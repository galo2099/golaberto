use crate::{
    rng::Rng,
    sampling::{Poisson, Scores},
};
use serde::{Deserialize, Deserializer, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Mutex;
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
    Name,
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
    pub(crate) target_limits: Mutex<HashMap<usize, crate::target_limits::Certified>>,
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
                "name" => Key::Name,
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
            target_limits: Mutex::new(HashMap::new()),
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
        if !self.keys[start..].contains(&Key::Head) {
            return self.compare_from(&c[a], &c[b], Some((a, b)), scores, rng, start);
        }
        self.compare_standing(a, b, c, scores, rng, start, None, None, &self.keys)
    }
    fn compare_standing(
        &self,
        a: usize,
        b: usize,
        c: &[Campaign],
        scores: &[[i32; 2]],
        rng: &mut Rng,
        start: usize,
        cohort: Option<&[usize]>,
        game_scope: Option<&[usize]>,
        keys: &[Key],
    ) -> bool {
        for key in &keys[start..] {
            let (x, y) = match key {
                Key::Pt => (c[a].points as f64, c[b].points as f64),
                Key::W => (c[a].wins as f64, c[b].wins as f64),
                Key::Gd => ((c[a].gf - c[a].ga) as f64, (c[b].gf - c[b].ga) as f64),
                Key::Gf => (c[a].gf as f64, c[b].gf as f64),
                Key::Average => (
                    c[a].gf as f64 / (c[a].ga as f64 + 1e-13),
                    c[b].gf as f64 / (c[b].ga as f64 + 1e-13),
                ),
                Key::Away => (c[a].away as f64, c[b].away as f64),
                Key::Bias => (c[a].bias as f64, c[b].bias as f64),
                Key::Aet | Key::Gp => (0., 0.),
                Key::Random | Key::Name => return rng.float() < 0.5,
                Key::Head => {
                    let cohort_len = cohort.map_or(self.n, |teams| teams.len());
                    let includes_pair =
                        cohort.map_or(true, |teams| teams.contains(&a) && teams.contains(&b));
                    let all_equal = if let Some(teams) = cohort {
                        teams.first().map_or(true, |&first| {
                            teams.iter().all(|&t| c[t].points == c[first].points)
                        })
                    } else {
                        (0..self.n).all(|t| c[t].points == c[0].points)
                    };
                    if cohort_len > 2 && includes_pair && !all_equal {
                        match self.context_head_order(a, b, cohort, c, scores, rng, game_scope) {
                            Ordering::Less => return true,
                            Ordering::Greater => return false,
                            Ordering::Equal => {}
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
    fn context_head_order(
        &self,
        a: usize,
        b: usize,
        cohort: Option<&[usize]>,
        c: &[Campaign],
        scores: &[[i32; 2]],
        rng: &mut Rng,
        game_scope: Option<&[usize]>,
    ) -> Ordering {
        let cohort_len = cohort.map_or(self.n, |teams| teams.len());
        let tied: Vec<_> = (0..cohort_len)
            .filter_map(|index| {
                let team = cohort.map_or(index, |teams| teams[index]);
                (c[team].points == c[a].points).then_some(team)
            })
            .collect();
        if tied.len() == cohort_len || tied.len() <= 1 || !tied.contains(&b) {
            return Ordering::Equal;
        }
        let mut mini = vec![Campaign::default(); c.len()];
        for &t in &tied {
            let team = &self.request.team_groups[t];
            mini[t].points = team.add_sub;
            mini[t].bias = team.bias;
        }
        let in_tied: std::collections::HashSet<_> = tied.iter().copied().collect();
        let mut selected = Vec::new();
        let mut last_by_pair = HashMap::new();
        let scope_len = game_scope.map_or(self.request.games.len(), |games| games.len());
        // Model::new applies played games first and future games afterward,
        // matching the Go service's campaign construction even when the
        // request interleaves those categories.
        for played in [true, false] {
            for index in 0..scope_len {
                let gi = game_scope.map_or(index, |games| games[index]);
                let game = &self.request.games[gi];
                if game.played != played {
                    continue;
                }
                let Some(&h) = self.indices.get(&game.home_id) else {
                    continue;
                };
                let Some(&v) = self.indices.get(&game.away_id) else {
                    continue;
                };
                if in_tied.contains(&h)
                    && in_tied.contains(&v)
                    && (h == a || h == b || v == a || v == b)
                {
                    last_by_pair.insert((h, v), gi);
                }
            }
        }
        selected.extend(last_by_pair.into_values());
        selected.sort_unstable();
        for &gi in &selected {
            let game = &self.request.games[gi];
            let h = self.indices[&game.home_id];
            let v = self.indices[&game.away_id];
            let [hs, as_] = scores[gi];
            mini[h].add(hs, as_, true, &self.request.phase.championship);
            mini[v].add(as_, hs, false, &self.request.phase.championship);
        }
        // Ruby removes the name key before comparing the mini table.
        let keys: Vec<_> = self
            .keys
            .iter()
            .copied()
            .filter(|key| *key != Key::Name && *key != Key::Random)
            .collect();
        let ordering = if self.compare_standing(
            a,
            b,
            &mini,
            scores,
            rng,
            0,
            Some(&tied),
            Some(&selected),
            &keys,
        ) {
            Ordering::Less
        } else if self.compare_standing(
            b,
            a,
            &mini,
            scores,
            rng,
            0,
            Some(&tied),
            Some(&selected),
            &keys,
        ) {
            Ordering::Greater
        } else {
            Ordering::Equal
        };
        ordering
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
    /// Compare the raw mini campaigns for a pair. Small-group score buckets
    /// use this only when a standings tie narrows to exactly two teams;
    /// standings themselves use the full tied-cohort comparator above.
    pub fn head_order(
        &self,
        home: usize,
        away: usize,
        scores: &[[i32; 2]],
        rng: &mut Rng,
    ) -> Ordering {
        let mut h = Campaign {
            points: self.request.team_groups[home].add_sub,
            bias: self.request.team_groups[home].bias,
            ..Default::default()
        };
        let mut a = Campaign {
            points: self.request.team_groups[away].add_sub,
            bias: self.request.team_groups[away].bias,
            ..Default::default()
        };
        let (mut forward, mut reverse): (Option<usize>, Option<usize>) = (None, None);
        for &(game, home_first) in &self.pair_games[home * self.n + away] {
            let slot = if home_first {
                &mut forward
            } else {
                &mut reverse
            };
            if slot.map_or(true, |previous| {
                let previous_played = self.request.games[previous].played;
                let current_played = self.request.games[game].played;
                (!current_played, game) > (!previous_played, previous)
            }) {
                *slot = Some(game);
            }
        }
        for (g, home_first) in forward
            .into_iter()
            .map(|g| (g, true))
            .chain(reverse.into_iter().map(|g| (g, false)))
        {
            let [hs, as_] = scores[g];
            if home_first {
                h.add(hs, as_, true, &self.request.phase.championship);
                a.add(as_, hs, false, &self.request.phase.championship);
            } else {
                h.add(as_, hs, false, &self.request.phase.championship);
                a.add(hs, as_, true, &self.request.phase.championship);
            }
        }
        // No pair means Random keys are skipped, exactly as in Key::Head.
        if self.compare(&a, &h, None, scores, rng) {
            Ordering::Less
        } else if self.compare(&h, &a, None, scores, rng) {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
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
                Key::Random | Key::Name => {
                    if pair.is_some() {
                        return rng.float() < 0.5;
                    } else {
                        continue;
                    }
                }
                Key::Head => {
                    if let Some((i, j)) = pair {
                        match self.head_order(i, j, scores, rng) {
                            Ordering::Greater => return true,
                            Ordering::Less => return false,
                            Ordering::Equal => {}
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

#[cfg(test)]
mod contextual_head_tests {
    use super::*;

    fn model(
        n: usize,
        sort: &str,
        groups: &[(i32, i32)],
        games: &[(usize, usize, i32, i32)],
    ) -> Model {
        Model::new(Request {
            id: 1,
            zones: vec![],
            phase: Phase {
                sort: sort.into(),
                championship: Championship {
                    point_win: 3,
                    point_draw: 1,
                    point_loss: 0,
                },
                bonus_points: 0,
                bonus_points_threshold: 0,
            },
            games: games
                .iter()
                .enumerate()
                .map(|(i, &(home, away, hs, as_))| Game {
                    id: i as i32 + 1,
                    home_id: home as i32 + 1,
                    away_id: away as i32 + 1,
                    home_score: hs,
                    away_score: as_,
                    home_power: 0.,
                    away_power: 0.,
                    played: true,
                })
                .collect(),
            team_groups: (0..n)
                .map(|i| Team {
                    team_id: i as i32 + 1,
                    add_sub: groups[i].0,
                    bias: groups[i].1,
                })
                .collect(),
        })
        .unwrap()
    }

    fn table(points: &[i32], gd: &[i32]) -> Vec<Campaign> {
        points
            .iter()
            .zip(gd)
            .map(|(&points, &gd)| Campaign {
                points,
                gf: gd,
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn partial_three_team_cohort_uses_all_tied_teams() {
        let m = model(
            4,
            "pt,head,gd",
            &[(0, 0); 4],
            &[(0, 1, 0, 0), (0, 2, 1, 0), (1, 2, 0, 1)],
        );
        let c = table(&[10, 10, 10, 2], &[0, 100, 0, 0]);
        assert!(m.less(0, 1, &c, &m.empty_scores(), &mut Rng::new(1)));
    }

    #[test]
    fn partial_four_team_cohort_uses_games_against_other_tied_teams() {
        let m = model(5, "pt,head,gd", &[(0, 0); 5], &[(0, 2, 1, 0), (1, 2, 0, 1)]);
        let c = table(&[10, 10, 10, 10, 2], &[0, 100, 0, 0, 0]);
        assert!(m.less(0, 1, &c, &m.empty_scores(), &mut Rng::new(6)));
    }

    #[test]
    fn ruby_partial_four_way_case_matches_add_sub_bias_and_latest_game_rules() {
        let m = model(
            5,
            "pt,head,gd,gf,g_away,w,bias,name",
            &[(2, 9), (2, 2), (4, 7), (1, 5), (0, 0)],
            &[
                (0, 1, 5, 0),
                (0, 1, 0, 0),
                (1, 0, 0, 0),
                (0, 2, 1, 0),
                (1, 2, 1, 0),
                (0, 3, 0, 0),
                (1, 3, 0, 0),
            ],
        );
        let mut c = table(&[10, 10, 10, 10, 0], &[0; 5]);
        for (campaign, group) in c.iter_mut().zip(&m.request.team_groups) {
            campaign.bias = group.bias;
        }
        assert!(m.less(0, 1, &c, &m.empty_scores(), &mut Rng::new(8)));
    }

    #[test]
    fn mini_campaign_keeps_the_latest_game_per_direction() {
        let m = model(
            4,
            "pt,head,gd",
            &[(0, 0); 4],
            &[(0, 2, 1, 0), (0, 2, 0, 1), (1, 2, 1, 0)],
        );
        let c = table(&[10, 10, 10, 2], &[100, 0, 0, 0]);
        assert!(m.less(1, 0, &c, &m.empty_scores(), &mut Rng::new(7)));
    }

    #[test]
    fn future_results_override_played_history_when_request_interleaves_them() {
        let m = Model::new(Request {
            id: 1,
            zones: vec![],
            phase: Phase {
                sort: "pt,head,gd".into(),
                championship: Championship {
                    point_win: 3,
                    point_draw: 1,
                    point_loss: 0,
                },
                bonus_points: 0,
                bonus_points_threshold: 0,
            },
            games: vec![
                Game {
                    id: 1,
                    home_id: 1,
                    away_id: 2,
                    home_score: 1,
                    away_score: 0,
                    home_power: 0.,
                    away_power: 0.,
                    played: false,
                },
                Game {
                    id: 2,
                    home_id: 1,
                    away_id: 2,
                    home_score: 0,
                    away_score: 1,
                    home_power: 0.,
                    away_power: 0.,
                    played: true,
                },
            ],
            team_groups: (1..=3)
                .map(|team_id| Team {
                    team_id,
                    add_sub: 0,
                    bias: 0,
                })
                .collect(),
        })
        .unwrap();
        let mut campaigns = m.base.clone();
        let mut scores = m.empty_scores();
        scores[0] = [1, 0];
        m.add(&mut campaigns, 0, scores[0]);

        assert!(m.less(0, 1, &campaigns, &scores, &mut Rng::new(9)));
        assert_eq!(
            m.head_order(0, 1, &scores, &mut Rng::new(9)),
            Ordering::Greater
        );
    }

    #[test]
    fn two_team_tie_inside_larger_table_uses_mini_campaign() {
        let m = model(4, "pt,head,gd", &[(0, 0); 4], &[(0, 1, 1, 0)]);
        let c = table(&[10, 10, 7, 3], &[0, 100, 0, 0]);
        assert!(m.less(0, 1, &c, &m.empty_scores(), &mut Rng::new(2)));
    }

    #[test]
    fn recursive_tie_restarts_within_narrowed_cohort() {
        let m = model(
            4,
            "pt,head,gd,gf",
            &[(0, 0); 4],
            &[(0, 1, 1, 0), (1, 2, 2, 0), (2, 0, 3, 0)],
        );
        let c = table(&[10, 10, 10, 2], &[-20, 20, 0, 0]);
        assert!(m.less(1, 0, &c, &m.empty_scores(), &mut Rng::new(3)));
    }

    #[test]
    fn mini_campaign_uses_add_sub_and_bias_initialization() {
        let m = model(
            3,
            "pt,head,bias,gd",
            &[(1, 5), (1, 0), (0, 0)],
            &[(0, 1, 0, 0)],
        );
        let c = table(&[10, 10, 2], &[0, 100, 0]);
        assert!(m.less(0, 1, &c, &m.empty_scores(), &mut Rng::new(4)));
    }

    #[test]
    fn two_team_group_skips_head_to_head() {
        let m = model(2, "pt,head,gd", &[(0, 0); 2], &[(0, 1, 1, 0)]);
        let c = table(&[5, 5], &[0, 10]);
        assert!(m.less(1, 0, &c, &m.empty_scores(), &mut Rng::new(5)));
    }
}
