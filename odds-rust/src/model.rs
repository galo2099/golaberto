use crate::{
    rng::Rng,
    sampling::{Poisson, Scores},
};
use serde::{Deserialize, Deserializer, Serialize};
use smallvec::SmallVec;
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
#[derive(Clone, Copy)]
struct DirectedGame {
    home: usize,
    away: usize,
    request_index: usize,
}
pub struct Model {
    pub request: Request,
    pub ids: Vec<i32>,
    pub indices: HashMap<i32, usize>,
    pub base: Vec<Campaign>,
    pub fixtures: Vec<Fixture>,
    pub keys: Vec<Key>,
    pub pair_games: Vec<Vec<(usize, bool)>>,
    // Latest game for each directed in-group pair, indexed by (home, away).
    latest_directed: Vec<Option<DirectedGame>>,
    pub n: usize,
    pub(crate) target_limits: Mutex<HashMap<usize, crate::target_limits::Certified>>,
}

#[derive(Clone, Debug)]
pub struct DiscreteReachability {
    /// Row-major team/rank mask, with zero-based ranks.
    pub possible: Vec<bool>,
    pub completed: bool,
    pub replays: usize,
}

#[derive(Default)]
struct SortTrace {
    decisions: Vec<bool>,
    cursor: usize,
    missing: bool,
}

impl SortTrace {
    fn unknown(&mut self) -> bool {
        if self.missing {
            return false;
        }
        if let Some(&decision) = self.decisions.get(self.cursor) {
            self.cursor += 1;
            decision
        } else {
            self.missing = true;
            false
        }
    }
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
        let keys: Vec<Key> = request
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
        let mut latest_directed = vec![None; n * n];
        for (i, g) in request.games.iter().enumerate() {
            let h = indices[&g.home_id];
            let a = indices[&g.away_id];
            if h < n && a < n {
                pair_games[h * n + a].push((i, true));
                pair_games[a * n + h].push((i, false));
                let slot = &mut latest_directed[h * n + a];
                let replace = slot.is_none_or(|previous: DirectedGame| {
                    let prev = &request.games[previous.request_index];
                    let current = &request.games[i];
                    (!current.played, i) > (!prev.played, previous.request_index)
                });
                if replace {
                    *slot = Some(DirectedGame {
                        home: h,
                        away: a,
                        request_index: i,
                    });
                }
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
            latest_directed,
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
        self.less_with_trace(a, b, c, scores, rng, &mut None)
    }
    #[inline]
    fn less_with_trace(
        &self,
        a: usize,
        b: usize,
        c: &[Campaign],
        scores: &[[i32; 2]],
        rng: &mut Rng,
        trace: &mut Option<&mut SortTrace>,
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
            return self.compare_from_with_trace(
                &c[a],
                &c[b],
                Some((a, b)),
                scores,
                rng,
                start,
                trace,
            );
        }
        self.compare_standing_with_trace(a, b, c, scores, rng, start, None, None, &self.keys, trace)
    }
    #[allow(dead_code)]
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
        self.compare_standing_with_trace(
            a, b, c, scores, rng, start, cohort, game_scope, keys, &mut None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn compare_standing_with_trace(
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
        trace: &mut Option<&mut SortTrace>,
    ) -> bool {
        for key in &keys[start..] {
            if matches!(key, Key::Gd | Key::Gf | Key::Average | Key::Away) {
                if let Some(t) = trace.as_deref_mut() {
                    return t.unknown();
                }
            }
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
                Key::Random | Key::Name => {
                    if cohort.is_some() {
                        continue;
                    }
                    if let Some(t) = trace.as_deref_mut() {
                        return t.unknown();
                    }
                    return rng.float() < 0.5;
                }
                Key::Head => {
                    let cohort_len = cohort.map_or(self.n, |teams| teams.len());
                    if cohort_len <= 2 {
                        continue;
                    }
                    let includes_pair =
                        cohort.map_or(true, |teams| teams.contains(&a) && teams.contains(&b));
                    let all_equal = if let Some(teams) = cohort {
                        teams.first().map_or(true, |&first| {
                            teams.iter().all(|&t| c[t].points == c[first].points)
                        })
                    } else {
                        (0..self.n).all(|t| c[t].points == c[0].points)
                    };
                    if includes_pair && !all_equal {
                        match self.context_head_order_with_trace(
                            a, b, cohort, c, scores, rng, game_scope, trace,
                        ) {
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
    #[allow(dead_code)]
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
        self.context_head_order_with_trace(a, b, cohort, c, scores, rng, game_scope, &mut None)
    }
    #[allow(clippy::too_many_arguments)]
    fn context_head_order_with_trace(
        &self,
        a: usize,
        b: usize,
        cohort: Option<&[usize]>,
        c: &[Campaign],
        scores: &[[i32; 2]],
        rng: &mut Rng,
        game_scope: Option<&[usize]>,
        trace: &mut Option<&mut SortTrace>,
    ) -> Ordering {
        let cohort_len = cohort.map_or(self.n, |teams| teams.len());
        let tied: SmallVec<[usize; 32]> = (0..cohort_len)
            .filter_map(|index| {
                let team = cohort.map_or(index, |teams| teams[index]);
                (c[team].points == c[a].points).then_some(team)
            })
            .collect();
        if tied.len() == cohort_len || tied.len() <= 1 || !tied.contains(&b) {
            return Ordering::Equal;
        }
        let mut mini: SmallVec<[Campaign; 32]> = std::iter::repeat(Campaign::default())
            .take(c.len())
            .collect();
        for &t in &tied {
            let team = &self.request.team_groups[t];
            mini[t].points = team.add_sub;
            mini[t].bias = team.bias;
        }
        // Model::new applies played games first and future games afterward,
        // matching the Go service's campaign construction even when the
        // request interleaves those categories.
        let mut selected_edges: SmallVec<[DirectedGame; 128]> = SmallVec::new();
        for &team in &tied {
            for edge in [
                self.latest_directed[a * self.n + team],
                self.latest_directed[team * self.n + a],
                self.latest_directed[b * self.n + team],
                self.latest_directed[team * self.n + b],
            ]
            .into_iter()
            .flatten()
            {
                let gi = edge.request_index;
                if game_scope.is_some_and(|scope| scope.binary_search(&gi).is_err()) {
                    continue;
                }
                selected_edges.push(edge);
            }
        }
        selected_edges.sort_unstable_by_key(|edge| edge.request_index);
        selected_edges.dedup_by_key(|edge| edge.request_index);
        let selected: SmallVec<[usize; 64]> = selected_edges
            .iter()
            .map(|edge| edge.request_index)
            .collect();
        for edge in selected_edges {
            let gi = edge.request_index;
            let [hs, as_] = scores[gi];
            mini[edge.home].add(hs, as_, true, &self.request.phase.championship);
            mini[edge.away].add(as_, hs, false, &self.request.phase.championship);
        }
        // Ruby removes name/random keys from mini-table comparisons. The
        // comparator applies that rule whenever it has a contextual cohort.
        let keys = &self.keys;
        let ordering = if self.compare_standing_with_trace(
            a,
            b,
            &mini,
            scores,
            rng,
            0,
            Some(&tied),
            Some(&selected),
            keys,
            trace,
        ) {
            Ordering::Less
        } else if self.compare_standing_with_trace(
            b,
            a,
            &mini,
            scores,
            rng,
            0,
            Some(&tied),
            Some(&selected),
            keys,
            trace,
        ) {
            Ordering::Greater
        } else {
            Ordering::Equal
        };
        ordering
    }
    #[allow(dead_code)]
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
        self.head_order_with_trace(home, away, scores, rng, &mut None)
    }
    fn head_order_with_trace(
        &self,
        home: usize,
        away: usize,
        scores: &[[i32; 2]],
        rng: &mut Rng,
        trace: &mut Option<&mut SortTrace>,
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
        if self.compare_from_with_trace(&a, &h, None, scores, rng, 0, trace) {
            Ordering::Less
        } else if self.compare_from_with_trace(&h, &a, None, scores, rng, 0, trace) {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
    #[allow(dead_code)]
    fn compare_from(
        &self,
        a: &Campaign,
        b: &Campaign,
        pair: Option<(usize, usize)>,
        scores: &[[i32; 2]],
        rng: &mut Rng,
        start: usize,
    ) -> bool {
        self.compare_from_with_trace(a, b, pair, scores, rng, start, &mut None)
    }
    fn compare_from_with_trace(
        &self,
        a: &Campaign,
        b: &Campaign,
        pair: Option<(usize, usize)>,
        scores: &[[i32; 2]],
        rng: &mut Rng,
        start: usize,
        trace: &mut Option<&mut SortTrace>,
    ) -> bool {
        for key in &self.keys[start..] {
            if matches!(key, Key::Gd | Key::Gf | Key::Average | Key::Away) {
                if let Some(t) = trace.as_deref_mut() {
                    return t.unknown();
                }
            }
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
                        if let Some(t) = trace.as_deref_mut() {
                            return t.unknown();
                        }
                        return rng.float() < 0.5;
                    } else {
                        continue;
                    }
                }
                Key::Head => {
                    if let Some((i, j)) = pair {
                        match self.head_order_with_trace(i, j, scores, rng, trace) {
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

    /// Over-approximate ranks for one fixed W/D/L path while leaving every
    /// score-dependent or random comparator decision unconstrained. The helper
    /// uses the production sorter and comparator; each replay fixes a prefix of
    /// unknown comparison results, then branches at the next unknown result.
    /// If the replay quota is exhausted, it returns the all-possible mask.
    pub fn discrete_reachability(
        &self,
        outcomes: &[usize],
        replay_limit: usize,
    ) -> DiscreteReachability {
        let all = || DiscreteReachability {
            possible: vec![true; self.n * self.n],
            completed: false,
            replays: 0,
        };
        if outcomes.len() != self.fixtures.len()
            || self.request.phase.bonus_points != 0
            || replay_limit == 0
        {
            return all();
        }
        let mut scores = self.empty_scores();
        let mut campaigns = self.base.clone();
        for (i, (&outcome, fixture)) in outcomes.iter().zip(&self.fixtures).enumerate() {
            let score = match outcome {
                0 => [0, 1],
                1 => [0, 0],
                2 => [1, 0],
                _ => return all(),
            };
            scores[fixture.request_index] = score;
            self.add(&mut campaigns, i, score);
        }

        let mut possible = vec![false; self.n * self.n];
        let mut pending = vec![Vec::<bool>::new()];
        let mut replays = 0;
        while let Some(decisions) = pending.pop() {
            if replays == replay_limit {
                return DiscreteReachability {
                    possible: vec![true; self.n * self.n],
                    completed: false,
                    replays,
                };
            }
            replays += 1;
            let mut trace = SortTrace {
                decisions: decisions.clone(),
                ..SortTrace::default()
            };
            let mut active = Some(&mut trace);
            let mut rng = Rng::new(0);
            let mut order: Vec<_> = (0..self.n).collect();
            crate::sort::sort(&mut order, &mut |a, b| {
                self.less_with_trace(a, b, &campaigns, &scores, &mut rng, &mut active)
            });
            if trace.missing {
                let mut yes = decisions.clone();
                yes.push(true);
                let mut no = decisions;
                no.push(false);
                pending.push(yes);
                pending.push(no);
            } else {
                for (rank, team) in order.into_iter().enumerate() {
                    possible[team * self.n + rank] = true;
                }
            }
        }
        DiscreteReachability {
            possible,
            completed: true,
            replays,
        }
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

    // Deliberately retain the old whole-scope algorithm as a test oracle.
    fn slow_head(
        m: &Model,
        a: usize,
        b: usize,
        cohort: Option<&[usize]>,
        c: &[Campaign],
        scores: &[[i32; 2]],
        rng: &mut Rng,
        scope: Option<&[usize]>,
    ) -> Ordering {
        let len = cohort.map_or(m.n, |teams| teams.len());
        let tied: Vec<_> = (0..len)
            .filter_map(|i| {
                let t = cohort.map_or(i, |teams| teams[i]);
                (c[t].points == c[a].points).then_some(t)
            })
            .collect();
        if tied.len() == len || tied.len() <= 1 || !tied.contains(&b) {
            return Ordering::Equal;
        }
        let mut mini = vec![Campaign::default(); c.len()];
        for &t in &tied {
            mini[t].points = m.request.team_groups[t].add_sub;
            mini[t].bias = m.request.team_groups[t].bias;
        }
        let count = scope.map_or(m.request.games.len(), |s| s.len());
        let mut latest = HashMap::new();
        for played in [true, false] {
            for i in 0..count {
                let gi = scope.map_or(i, |s| s[i]);
                let g = &m.request.games[gi];
                if g.played != played {
                    continue;
                }
                let (Some(&h), Some(&v)) = (m.indices.get(&g.home_id), m.indices.get(&g.away_id))
                else {
                    continue;
                };
                if tied.contains(&h) && tied.contains(&v) && (h == a || h == b || v == a || v == b)
                {
                    latest.insert((h, v), gi);
                }
            }
        }
        let mut selected: Vec<_> = latest.into_values().collect();
        selected.sort_unstable();
        for &gi in &selected {
            let g = &m.request.games[gi];
            let (h, v) = (m.indices[&g.home_id], m.indices[&g.away_id]);
            let [hs, as_] = scores[gi];
            mini[h].add(hs, as_, true, &m.request.phase.championship);
            mini[v].add(as_, hs, false, &m.request.phase.championship);
        }
        let keys: Vec<_> = m
            .keys
            .iter()
            .copied()
            .filter(|k| *k != Key::Name && *k != Key::Random)
            .collect();
        if slow_compare(m, a, b, &tied, &selected, &mini, scores, rng, &keys) {
            Ordering::Less
        } else if slow_compare(m, b, a, &tied, &selected, &mini, scores, rng, &keys) {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }

    fn slow_compare(
        m: &Model,
        a: usize,
        b: usize,
        cohort: &[usize],
        scope: &[usize],
        c: &[Campaign],
        scores: &[[i32; 2]],
        rng: &mut Rng,
        keys: &[Key],
    ) -> bool {
        for key in keys {
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
                Key::Name | Key::Random => return rng.float() < 0.5,
                Key::Head => {
                    let all_equal = cohort.iter().all(|&t| c[t].points == c[cohort[0]].points);
                    if cohort.len() > 2 && cohort.contains(&a) && cohort.contains(&b) && !all_equal
                    {
                        match slow_head(m, a, b, Some(cohort), c, scores, rng, Some(scope)) {
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

    fn slow_standing(
        m: &Model,
        a: usize,
        b: usize,
        c: &[Campaign],
        scores: &[[i32; 2]],
        rng: &mut Rng,
        cohort: Option<&[usize]>,
        scope: Option<&[usize]>,
        keys: &[Key],
        start: usize,
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
                Key::Random | Key::Name => {
                    if cohort.is_some() {
                        continue;
                    }
                    return rng.float() < 0.5;
                }
                Key::Head => {
                    let len = cohort.map_or(m.n, |teams| teams.len());
                    let all_equal = if let Some(teams) = cohort {
                        teams.first().map_or(true, |&first| {
                            teams.iter().all(|&t| c[t].points == c[first].points)
                        })
                    } else {
                        (0..m.n).all(|t| c[t].points == c[0].points)
                    };
                    if len > 2
                        && cohort.map_or(true, |teams| teams.contains(&a) && teams.contains(&b))
                        && !all_equal
                    {
                        match slow_head(m, a, b, cohort, c, scores, rng, scope) {
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

    fn slow_less(
        m: &Model,
        a: usize,
        b: usize,
        c: &[Campaign],
        scores: &[[i32; 2]],
        rng: &mut Rng,
    ) -> bool {
        let mut start = 0;
        if m.keys.first() == Some(&Key::Pt) {
            if c[a].points != c[b].points {
                return c[a].points > c[b].points;
            }
            start = 1;
            if m.keys.get(1) == Some(&Key::W) {
                if c[a].wins != c[b].wins {
                    return c[a].wins > c[b].wins;
                }
                start = 2;
            }
        }
        slow_standing(m, a, b, c, scores, rng, None, None, &m.keys, start)
    }

    fn slow_standings(m: &Model, c: &[Campaign], scores: &[[i32; 2]], rng: &mut Rng) -> Vec<usize> {
        let mut order: Vec<_> = (0..m.n).collect();
        crate::sort::sort(&mut order, &mut |a, b| slow_less(m, a, b, c, scores, rng));
        order
    }

    #[test]
    fn indexed_context_matches_slow_reference_and_rng_for_random_histories() {
        for seed in 1..40i64 {
            let n = 5;
            let mut games = Vec::new();
            for i in 0..24 {
                let h = (seed as usize * 7 + i * 3) % n;
                let mut a = (h + 1 + (i % 3)) % n;
                if a == h {
                    a = (a + 1) % n;
                }
                games.push((
                    h,
                    a,
                    (i % 4) as i32,
                    ((i + seed as usize) % 3) as i32,
                    i % 3 != 0,
                ));
            }
            // external opponents and interleaved played/future duplicates
            games.push((1, 8, 2, 0, true));
            games.push((2, 1, 0, 1, false));
            games.push((3, 3, 0, 0, true));
            let mut m = Model::new(Request {
                id: 1,
                zones: vec![],
                phase: Phase {
                    sort: "pt,head,gd,gf,g_away,w,bias,name".into(),
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
                    .map(|(i, &(h, a, hs, as_, played))| Game {
                        id: i as i32,
                        home_id: h as i32 + 1,
                        away_id: a as i32 + 1,
                        home_score: hs,
                        away_score: as_,
                        home_power: 0.,
                        away_power: 0.,
                        played,
                    })
                    .collect(),
                team_groups: (0..n)
                    .map(|i| Team {
                        team_id: i as i32 + 1,
                        add_sub: (i as i32 % 3) - 1,
                        bias: i as i32,
                    })
                    .collect(),
            })
            .unwrap();
            let scores = m.empty_scores();
            let c = table(&[9, 9, 9, 6, 6], [0; 5].as_slice());
            for &(a, b) in &[(0, 1), (1, 2), (3, 4)] {
                let mut fast = Rng::new(seed);
                let mut slow = fast.clone();
                let actual = m.context_head_order(a, b, None, &c, &scores, &mut fast, None);
                let expected = slow_head(&m, a, b, None, &c, &scores, &mut slow, None);
                assert_eq!(actual, expected, "seed={seed} pair={a},{b}");
                assert_eq!(fast.float(), slow.float(), "rng seed={seed} pair={a},{b}");
            }
            for keys in [
                vec![Key::Pt, Key::Head, Key::Gd, Key::Name],
                vec![Key::Pt, Key::Bias, Key::Head, Key::Gf, Key::Random],
                vec![Key::Pt, Key::Head, Key::Gf, Key::Away, Key::W],
            ] {
                m.keys = keys;
                let mut fast_rng = Rng::new(seed);
                let mut slow_rng = fast_rng.clone();
                let mut order = vec![0; n];
                m.standings(&mut order, &c, &scores, &mut fast_rng);
                let expected = slow_standings(&m, &c, &scores, &mut slow_rng);
                assert_eq!(order, expected, "standings seed={seed} keys={:?}", m.keys);
                assert_eq!(
                    fast_rng.float(),
                    slow_rng.float(),
                    "standings rng seed={seed} keys={:?}",
                    m.keys
                );
                let mut next_c = c.clone();
                for (team, campaign) in next_c.iter_mut().enumerate() {
                    campaign.points += (team % 3) as i32;
                }
                let mut fast_rng = Rng::new(seed + 100);
                let mut slow_rng = fast_rng.clone();
                let mut next_order = vec![0; n];
                m.standings(&mut next_order, &next_c, &scores, &mut fast_rng);
                let next_expected = slow_standings(&m, &next_c, &scores, &mut slow_rng);
                assert_eq!(next_order, next_expected, "changed-season seed={seed}");
                assert_eq!(
                    fast_rng.float(),
                    slow_rng.float(),
                    "changed-season rng seed={seed}"
                );
            }
        }
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
