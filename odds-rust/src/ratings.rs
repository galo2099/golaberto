//! Native implementation of the Go SPI, evaluation and historical-rating models.
//! Keep the legacy formulas (including evaluation's advantage convention) intact.
use chrono::{DateTime, Datelike, Local, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, HashMap};

pub const AVG_BASE: f64 = 1.3350257653834494;
pub const HOME_ADV: f64 = 0.16133676871779334;
const FOUR_YEARS: i64 = 4 * 365 * 24 * 3600;

fn null_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Game {
    #[serde(alias = "Phase_Id", deserialize_with = "null_default")]
    pub phase_id: i32,
    #[serde(alias = "Home_Id", deserialize_with = "null_default")]
    pub home_id: i32,
    #[serde(alias = "Away_Id", deserialize_with = "null_default")]
    pub away_id: i32,
    #[serde(alias = "Home_Score", deserialize_with = "null_default")]
    pub home_score: f64,
    #[serde(alias = "Away_Score", deserialize_with = "null_default")]
    pub away_score: f64,
    #[serde(alias = "Timestamp", deserialize_with = "null_default")]
    pub timestamp: i64,
    #[serde(alias = "Length", deserialize_with = "null_default")]
    pub length: f64,
    #[serde(alias = "Advantage", deserialize_with = "null_default")]
    pub advantage: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct TeamRating {
    #[serde(rename = "Id", alias = "id", deserialize_with = "null_default")]
    pub id: i32,
    #[serde(
        rename = "Offense",
        alias = "offense",
        deserialize_with = "null_default"
    )]
    pub offense: f64,
    #[serde(
        rename = "Defense",
        alias = "defense",
        deserialize_with = "null_default"
    )]
    pub defense: f64,
    #[serde(rename = "Team", alias = "team", deserialize_with = "null_default")]
    pub team: f64,
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(default)]
pub struct Request {
    #[serde(alias = "Games", deserialize_with = "null_default")]
    pub games: Vec<Game>,
    #[serde(alias = "Ratings", deserialize_with = "null_default")]
    pub ratings: Vec<TeamRating>,
    #[serde(alias = "Phases_To_Eval", deserialize_with = "null_default")]
    pub phases_to_eval: Vec<i32>,
}

pub type Ratings = BTreeMap<i32, Option<TeamRating>>;

impl Request {
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        for r in &self.ratings {
            if !ids.insert(r.id) {
                return Err(format!("duplicate rating for team {}", r.id));
            }
            if ![r.offense, r.defense, r.team].iter().all(|v| v.is_finite()) {
                return Err("ratings must be finite".into());
            }
        }
        let mut previous = i64::MIN;
        for g in &self.games {
            if !ids.contains(&g.home_id) || !ids.contains(&g.away_id) {
                return Err("every game team must appear in ratings".into());
            }
            if ![g.home_score, g.away_score, g.length, g.advantage]
                .iter()
                .all(|v| v.is_finite())
                || g.length <= 0.
                || g.home_score < 0.
                || g.away_score < 0.
            {
                return Err("game scores must be nonnegative and length must be positive".into());
            }
            if g.timestamp < previous || DateTime::from_timestamp(g.timestamp, 0).is_none() {
                return Err("games must have valid timestamps in chronological order".into());
            }
            previous = g.timestamp;
        }
        Ok(())
    }

    pub fn initial(&self) -> Ratings {
        self.ratings
            .iter()
            .map(|r| (r.id, Some(r.clone())))
            .collect()
    }
}

pub fn squash_date(timestamp: i64, now: i64) -> f64 {
    let x = (timestamp as f64 - now as f64) / (730. * 24. * 60. * 60.);
    let (a, b) = (x.exp(), (-x).exp());
    1. + (a - b) / (a + b)
}
pub fn power(offense: f64, defense: f64, advantage: f64) -> f64 {
    ((offense - AVG_BASE) / (AVG_BASE * 0.424 + 0.548) * ((defense + advantage) * 0.424 + 0.548)
        + defense
        + advantage)
        .clamp(0.01, 10.)
}
pub fn game_odds(home: f64, away: f64) -> [f64; 3] {
    let mut h = [0.; 20];
    let mut a = [0.; 20];
    h[0] = (-home).exp();
    a[0] = (-away).exp();
    for i in 1..20 {
        h[i] = h[i - 1] * home / i as f64;
        a[i] = a[i - 1] * away / i as f64;
    }
    let mut odds = [0.; 3];
    for (i, hp) in h.iter().enumerate() {
        for (j, ap) in a.iter().enumerate() {
            odds[if i > j {
                0
            } else if i == j {
                1
            } else {
                2
            }] += hp * ap;
        }
    }
    odds
}
pub fn team_rating(r: &TeamRating) -> f64 {
    let [h, d, _] = game_odds(
        power(r.offense, AVG_BASE, 0.),
        power(AVG_BASE, r.defense, 0.),
    );
    (h * 3. + d) / 3. * 100.
}
fn lower(avg: f64, samples: f64) -> f64 {
    avg / (1. + (-samples / 4.).exp())
}
fn upper(avg: f64, samples: f64) -> f64 {
    avg * (1. + (-samples / 4.).exp())
}

pub fn spi(games: &[Game], ratings: &Ratings) -> Result<Ratings, String> {
    if games.is_empty() {
        return Ok(ratings.keys().map(|&id| (id, None)).collect());
    }
    let ids: Vec<_> = ratings.keys().copied().collect();
    let index: HashMap<_, _> = ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
    let n = ids.len();
    let mut counts = vec![0f64; n];
    let now = games.last().unwrap().timestamp;
    let mut indexed = Vec::with_capacity(games.len());
    let mut weights = Vec::with_capacity(games.len());
    for g in games {
        let h = *index.get(&g.home_id).ok_or("unknown home team")?;
        let a = *index.get(&g.away_id).ok_or("unknown away team")?;
        indexed.push((h, a));
        let w = g.length * squash_date(g.timestamp, now);
        weights.push(w);
        counts[h] += w;
        counts[a] += w;
    }
    let team_weights: Vec<_> = counts
        .iter()
        .map(|c| 2. / (1. + (-c / 4.).exp()) - 1.)
        .collect();
    let penalties: Vec<_> = counts
        .iter()
        .map(|c| 1. / (1. + (-c / 4.).exp()) / 2.5 + 0.6)
        .collect();
    counts.fill(0.);
    for (i, &(h, a)) in indexed.iter().enumerate() {
        weights[i] *= team_weights[h] * team_weights[a];
        counts[h] += weights[i];
        counts[a] += weights[i];
    }
    for c in &mut counts {
        *c += c.min(1.);
    }
    let mut offense = vec![0.; n];
    let mut defense = vec![0.; n];
    for (i, id) in ids.iter().enumerate() {
        if let Some(r) = &ratings[id] {
            offense[i] = upper(r.offense, counts[i]);
            defense[i] = lower(r.defense, counts[i]);
        }
    }
    let mut scored = vec![0.; n];
    let mut allowed = vec![0.; n];
    let mut good = 0;
    for _ in 0..100000 {
        for i in 0..n {
            scored[i] = AVG_BASE * (counts[i] / 2.).min(1.);
            allowed[i] = scored[i];
        }
        for (i, g) in games.iter().enumerate() {
            let (h, a) = indexed[i];
            let adv = g.advantage;
            let w = weights[i];
            let scale = AVG_BASE * 0.424 + 0.548;
            scored[h] += penalties[h]
                * w
                * ((g.home_score - (defense[a] + adv)) / ((defense[a] + adv) * 0.424 + 0.548)
                    * scale
                    + AVG_BASE);
            allowed[h] += (1. / penalties[h])
                * w
                * ((g.away_score - (offense[a] - adv)) / ((offense[a] - adv) * 0.424 + 0.548)
                    * scale
                    + AVG_BASE);
            scored[a] += penalties[a]
                * w
                * ((g.away_score - (defense[h] - adv)) / ((defense[h] - adv) * 0.424 + 0.548)
                    * scale
                    + AVG_BASE);
            allowed[a] += (1. / penalties[a])
                * w
                * ((g.home_score - (offense[h] + adv)) / ((offense[h] + adv) * 0.424 + 0.548)
                    * scale
                    + AVG_BASE);
        }
        let mut error = 0f64;
        for i in 0..n {
            if counts[i] == 0. {
                continue;
            }
            let old = offense[i];
            offense[i] = scored[i] / counts[i];
            defense[i] = allowed[i] / counts[i];
            if !offense[i].is_finite() || !defense[i].is_finite() {
                return Err("SPI iteration produced a nonfinite rating".into());
            }
            error = error.max((old - offense[i]).abs());
        }
        good = if error < 0.0001 { good + 1 } else { 0 };
        if good > 10 {
            break;
        }
    }
    Ok(ids
        .iter()
        .enumerate()
        .map(|(i, &id)| {
            let rating = if counts[i] == 0. {
                None
            } else {
                let mut r = TeamRating {
                    id,
                    offense: lower(offense[i], counts[i]),
                    defense: upper(defense[i], counts[i]),
                    team: 0.,
                };
                r.team = team_rating(&r);
                Some(r)
            };
            (id, rating)
        })
        .collect())
}

#[derive(Debug, Serialize)]
pub struct Evaluation {
    pub rps: f64,
    pub team_rps: BTreeMap<i32, f64>,
}
pub fn evaluate(request: &Request) -> Result<Evaluation, String> {
    request.validate()?;
    let mut ratings = request.initial();
    let mut start = 0;
    // Go's zero time has day-of-month 1. Preserve its day-only update policy.
    let mut last_day = 1;
    let mut total = 0.;
    let mut count = 0;
    let mut teams: BTreeMap<i32, (f64, usize)> = BTreeMap::new();
    for (i, g) in request.games.iter().enumerate() {
        if request.phases_to_eval.contains(&g.phase_id) {
            let day = DateTime::from_timestamp(g.timestamp, 0)
                .unwrap()
                .with_timezone(&Local)
                .day();
            if day != last_day {
                ratings = spi(&request.games[start..i], &ratings)?;
                last_day = day;
            }
            let h = ratings.get(&g.home_id).and_then(Option::as_ref);
            let a = ratings.get(&g.away_id).and_then(Option::as_ref);
            let hp = power(
                h.map_or(AVG_BASE / 2., |r| r.offense),
                a.map_or(AVG_BASE * 2., |r| r.defense),
                g.advantage,
            );
            let ap = power(
                a.map_or(AVG_BASE / 2., |r| r.offense),
                h.map_or(AVG_BASE * 2., |r| r.defense),
                g.advantage,
            );
            let [home, draw, _] = game_odds(hp, ap);
            let observed_home = if g.home_score > g.away_score { 1. } else { 0. };
            let observed_draw = if g.home_score == g.away_score { 1. } else { 0. };
            let rps = ((home - observed_home).powi(2)
                + (home + draw - observed_home - observed_draw).powi(2))
                / 2.;
            for id in [g.home_id, g.away_id] {
                let value = teams.entry(id).or_default();
                value.0 += rps;
                value.1 += 1;
            }
            total += rps;
            count += 1;
        }
        while request.games[start].timestamp < g.timestamp - FOUR_YEARS {
            start += 1;
        }
    }
    if count == 0 {
        return Err("no games belong to phases_to_eval".into());
    }
    Ok(Evaluation {
        rps: total / count as f64,
        team_rps: teams
            .into_iter()
            .map(|(id, (sum, count))| (id, sum / count as f64))
            .collect(),
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoricalRating {
    pub team_id: i32,
    pub off_rating: f64,
    pub def_rating: f64,
    pub rating: f64,
    pub measure_date: String,
}
pub fn historical(request: &Request) -> Result<Vec<HistoricalRating>, String> {
    request.validate()?;
    let Some(first) = request.games.first() else {
        return Ok(Vec::new());
    };
    let mut ratings = request.initial();
    let mut start = 0;
    let mut next = 4 * 52 * 7 * 24 * 60 * 60;
    let mut rows = Vec::new();
    for (i, g) in request.games.iter().enumerate() {
        while request.games[start].timestamp < g.timestamp - FOUR_YEARS {
            start += 1;
        }
        if g.timestamp - first.timestamp > next || i == request.games.len() - 1 {
            next += 24 * 60 * 60;
            ratings = spi(&request.games[start..=i], &ratings)?;
            let date = DateTime::<Utc>::from_timestamp(g.timestamp, 0)
                .unwrap()
                .format("%Y-%m-%d")
                .to_string();
            for r in ratings.values().flatten() {
                if r.team != 0. {
                    rows.push(HistoricalRating {
                        team_id: r.id,
                        off_rating: r.offense,
                        def_rating: r.defense,
                        rating: r.team,
                        measure_date: date.clone(),
                    });
                }
            }
        }
    }
    Ok(rows)
}
