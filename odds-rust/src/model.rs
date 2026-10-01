use crate::{
    rng::Rng,
    sampling::{Poisson, Scores},
};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Request {
    pub id: i32,
    #[serde(default)]
    pub zones: Vec<Zone>,
    pub phase: Phase,
    pub games: Vec<Game>,
    pub team_groups: Vec<Team>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Zone {
    pub position: Vec<usize>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Phase {
    pub sort: String,
    pub championship: Championship,
    #[serde(default, deserialize_with = "null_default")]
    pub bonus_points: i32,
    #[serde(default, deserialize_with = "null_default")]
    pub bonus_points_threshold: i32,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Championship {
    #[serde(default, deserialize_with = "null_default")]
    pub point_win: i32,
    #[serde(default, deserialize_with = "null_default")]
    pub point_draw: i32,
    #[serde(default, deserialize_with = "null_default")]
    pub point_loss: i32,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Team {
    pub team_id: i32,
    #[serde(default, deserialize_with = "null_default")]
    pub add_sub: i32,
    #[serde(default, deserialize_with = "null_default")]
    pub bias: i32,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Game {
    pub id: i32,
    pub home_id: i32,
    pub away_id: i32,
    #[serde(default, deserialize_with = "null_default")]
    pub home_score: i32,
    #[serde(default, deserialize_with = "null_default")]
    pub away_score: i32,
    #[serde(default, deserialize_with = "null_default")]
    pub home_power: f64,
    #[serde(default, deserialize_with = "null_default")]
    pub away_power: f64,
    #[serde(default, deserialize_with = "null_default")]
    pub played: bool,
}
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Campaign {
    pub points: i32,
    pub wins: i32,
    pub losses: i32,
    pub draws: i32,
    pub gf: i32,
    pub ga: i32,
    pub away: i32,
    pub bias: i32,
}
impl Campaign {
    #[inline]
    pub fn add(&mut self, own: i32, other: i32, home: bool, rules: &Championship) {
        if own > other {
            self.wins += 1;
            self.points += rules.point_win;
        } else if own < other {
            self.losses += 1;
            self.points += rules.point_loss;
        } else {
            self.draws += 1;
            self.points += rules.point_draw;
        }
        self.gf += own;
        self.ga += other;
        if !home {
            self.away += own;
        }
    }
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Key {
    Pt,
    W,
    Gd,
    Gf,
    Average,
    Away,
    Bias,
    Aet,
    Gp,
    Head,
    Random,
}
pub struct Fixture {
    pub home: usize,
    pub away: usize,
    pub request_index: usize,
    pub home_sampler: Poisson,
    pub away_sampler: Poisson,
    pub prob: [f64; 3],
    pub scores: Scores,
}
pub struct Model {
    pub request: Request,
    pub ids: Vec<i32>,
    pub indices: HashMap<i32, usize>,
    pub base: Vec<Campaign>,
    pub fixtures: Vec<Fixture>,
    pub keys: Vec<Key>,
    pub pair_games: Vec<Vec<(usize, bool)>>,
    pub n: usize,
}
impl Model {
    pub fn new(request: Request) -> Result<Self, String> {
        let n = request.team_groups.len();
        if n == 0 {
            return Err("empty group".into());
        }
        let mut ids: Vec<i32> = request.team_groups.iter().map(|t| t.team_id).collect();
        let mut indices: HashMap<_, _> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        if indices.len() != n {
            return Err("duplicate team ID".into());
        }
        for g in &request.games {
            if !g.home_power.is_finite()
                || !g.away_power.is_finite()
                || g.home_power < 0.
                || g.away_power < 0.
            {
                return Err("invalid Poisson mean".into());
            }
            for id in [g.home_id, g.away_id] {
                if !indices.contains_key(&id) {
                    indices.insert(id, ids.len());
                    ids.push(id);
                }
            }
        }
        let mut base = vec![Campaign::default(); ids.len()];
        for (i, t) in request.team_groups.iter().enumerate() {
            base[i].points = t.add_sub;
            base[i].bias = t.bias;
        }
        let rules = &request.phase.championship;
        let mut fixtures = Vec::new();
        for (request_index, g) in request.games.iter().enumerate() {
            let home = indices[&g.home_id];
            let away = indices[&g.away_id];
            if g.played {
                base[home].add(g.home_score, g.away_score, true, rules);
                base[away].add(g.away_score, g.home_score, false, rules);
            } else {
                let scores = Scores::new(g.home_power, g.away_power);
                fixtures.push(Fixture {
                    home,
                    away,
                    request_index,
                    home_sampler: Poisson::new(g.home_power),
                    away_sampler: Poisson::new(g.away_power),
                    prob: scores.prob,
                    scores,
                });
            }
        }
        let keys = request
            .phase
            .sort
            .split(',')
            .map(|s| match s.trim() {
                "pt" => Key::Pt,
                "w" => Key::W,
                "gd" => Key::Gd,
                "gf" => Key::Gf,
                "g_average" => Key::Average,
                "g_away" => Key::Away,
                "bias" => Key::Bias,
                "g_aet" => Key::Aet,
                "gp" => Key::Gp,
                "head" => Key::Head,
                _ => Key::Random,
            })
            .collect();
        let mut pair_games = vec![Vec::new(); n * n];
        for (i, g) in request.games.iter().enumerate() {
            let h = indices[&g.home_id];
            let a = indices[&g.away_id];
            if h < n && a < n {
                pair_games[h * n + a].push((i, true));
                pair_games[a * n + h].push((i, false));
            }
        }
        Ok(Self {
            request,
            ids,
            indices,
            base,
            fixtures,
            keys,
            pair_games,
            n,
        })
    }
    #[inline]
    pub fn add(&self, c: &mut [Campaign], f: usize, score: [i32; 2]) {
        let game = &self.fixtures[f];
        let rules = &self.request.phase.championship;
        c[game.home].add(score[0], score[1], true, rules);
        c[game.away].add(score[1], score[0], false, rules);
    }
    pub fn empty_scores(&self) -> Vec<[i32; 2]> {
        self.request
            .games
            .iter()
            .map(|g| [g.home_score, g.away_score])
            .collect()
    }
    #[inline]
    pub fn less(
        &self,
        a: usize,
        b: usize,
        c: &[Campaign],
        scores: &[[i32; 2]],
        rng: &mut Rng,
    ) -> bool {
        // Resolve the common leading integer keys before entering the generic
        // (possibly recursive head-to-head) comparator. i32 values convert
        // exactly to f64, so ordering and tie RNG consumption are unchanged.
        let mut start = 0;
        if self.keys.first() == Some(&Key::Pt) {
            if c[a].points != c[b].points {
                return c[a].points > c[b].points;
            }
            start = 1;
            if self.keys.get(1) == Some(&Key::W) {
                if c[a].wins != c[b].wins {
                    return c[a].wins > c[b].wins;
                }
                start = 2;
            }
        }
        self.compare_from(&c[a], &c[b], Some((a, b)), scores, rng, start)
    }
    fn compare(
        &self,
        a: &Campaign,
        b: &Campaign,
        pair: Option<(usize, usize)>,
        scores: &[[i32; 2]],
        rng: &mut Rng,
    ) -> bool {
        self.compare_from(a, b, pair, scores, rng, 0)
    }
    fn compare_from(
        &self,
        a: &Campaign,
        b: &Campaign,
        pair: Option<(usize, usize)>,
        scores: &[[i32; 2]],
        rng: &mut Rng,
        start: usize,
    ) -> bool {
        for key in &self.keys[start..] {
            let (x, y) = match key {
                Key::Pt => (a.points as f64, b.points as f64),
                Key::W => (a.wins as f64, b.wins as f64),
                Key::Gd => ((a.gf - a.ga) as f64, (b.gf - b.ga) as f64),
                Key::Gf => (a.gf as f64, b.gf as f64),
                Key::Average => (
                    a.gf as f64 / (a.ga as f64 + 1e-13),
                    b.gf as f64 / (b.ga as f64 + 1e-13),
                ),
                Key::Away => (a.away as f64, b.away as f64),
                Key::Bias => (a.bias as f64, b.bias as f64),
                Key::Aet | Key::Gp => (0., 0.),
                Key::Random => {
                    if pair.is_some() {
                        return rng.float() < 0.5;
                    } else {
                        continue;
                    }
                }
                Key::Head => {
                    if let Some((i, j)) = pair {
                        let mut h = Campaign::default();
                        let mut v = Campaign::default();
                        for &(g, home) in &self.pair_games[i * self.n + j] {
                            let [hs, as_] = scores[g];
                            if home {
                                h.add(hs, as_, true, &self.request.phase.championship);
                                v.add(as_, hs, false, &self.request.phase.championship);
                            } else {
                                h.add(as_, hs, false, &self.request.phase.championship);
                                v.add(hs, as_, true, &self.request.phase.championship);
                            }
                        }
                        if self.compare(&v, &h, None, scores, rng) {
                            return false;
                        }
                        if self.compare(&h, &v, None, scores, rng) {
                            return true;
                        }
                    }
                    continue;
                }
            };
            if x > y {
                return true;
            }
            if x < y {
                return false;
            }
        }
        false
    }
    pub fn standings(
        &self,
        order: &mut [usize],
        c: &[Campaign],
        scores: &[[i32; 2]],
        rng: &mut Rng,
    ) {
        for (i, t) in order.iter_mut().enumerate() {
            *t = i;
        }
        crate::sort::sort(order, &mut |a, b| self.less(a, b, c, scores, rng));
    }
}
