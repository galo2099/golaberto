//! Bounded extreme-rank relaxation with explained fixture conflicts.
//! A feasible P/W assignment is not a reachability witness. Only exhaustion of
//! every required-rival cohort can prove impossibility, without restricting goals.
use crate::{
    model::{Key, Model},
    search::Cell,
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Reason<const WORDS: usize> {
    decisions: [u64; WORDS],
    teams: u64,
}
impl<const WORDS: usize> Default for Reason<WORDS> {
    fn default() -> Self {
        Self {
            decisions: [0; WORDS],
            teams: 0,
        }
    }
}
impl<const WORDS: usize> Reason<WORDS> {
    fn insert(&mut self, bit: usize) {
        self.decisions[bit / 64] |= 1_u64 << (bit % 64);
    }
    fn contains(&self, bit: usize) -> bool {
        self.decisions[bit / 64] & (1_u64 << (bit % 64)) != 0
    }
    fn remove(&mut self, bit: usize) {
        self.decisions[bit / 64] &= !(1_u64 << (bit % 64));
    }
    fn is_empty(&self) -> bool {
        self.decisions.iter().all(|word| *word == 0)
    }
    fn merge(&mut self, other: Self) {
        for (left, right) in self.decisions.iter_mut().zip(other.decisions) {
            *left |= right;
        }
        self.teams |= other.teams;
    }
}
struct Game {
    home: usize,
    away: usize,
    gain: [[i64; 3]; 2],
    maximum: [[i64; 8]; 2],
}
pub struct RankProof {
    base: Vec<i64>,
    games: Vec<Game>,
    adjacent: Vec<Vec<(usize, usize)>>,
    ranked: usize,
}
#[derive(Default, Debug, Serialize)]
pub struct Stats {
    pub nodes: usize,
    pub cohorts: usize,
    pub cohort_cache_hits: usize,
    pub backjumps: usize,
}
#[derive(Clone, Copy)]
pub struct Options {
    pub explain: bool,
    pub internal_order: bool,
    pub reuse_cohorts: bool,
    /// Avoid deep search unless an eligible rival has a tight fixture outcome.
    pub require_root_pressure: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_cohorts: usize,
    pub max_depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_cohorts: 512,
            max_depth: 128,
        }
    }
}
impl Default for Options {
    fn default() -> Self {
        Self {
            explain: true,
            internal_order: true,
            reuse_cohorts: true,
            require_root_pressure: false,
        }
    }
}
#[derive(Debug, PartialEq)]
enum Answer<const WORDS: usize> {
    Conflict(Reason<WORDS>),
    Feasible,
    Unknown,
}
#[derive(Default, Serialize)]
pub struct Report {
    pub impossible: bool,
    pub stats: Stats,
}
#[derive(Default)]
pub struct FeasibleDomains {
    pub domains: Option<Vec<u8>>,
    pub stats: Stats,
}
impl RankProof {
    pub fn new(model: &Model) -> Option<Self> {
        let c = &model.request.phase.championship;
        if model.ids.len() > 64
            || model.keys.first() != Some(&Key::Pt)
            || model.request.phase.bonus_points != 0
            || (c.point_win, c.point_draw, c.point_loss) != (3, 1, 0)
            || model.fixtures.iter().any(|g| g.home == g.away)
        {
            return None;
        }
        let wins = model.keys.get(1) == Some(&Key::W);
        let mut remaining = vec![0_i64; model.ids.len()];
        for g in &model.fixtures {
            remaining[g.home] += 1;
            remaining[g.away] += 1;
        }
        let stride = if wins {
            model
                .base
                .iter()
                .enumerate()
                .map(|(t, c)| i64::from(c.wins) + remaining[t])
                .max()?
                + 1
        } else {
            1
        };
        let base = model
            .base
            .iter()
            .map(|c| {
                i64::from(c.points)
                    .checked_mul(stride)?
                    .checked_add(if wins { i64::from(c.wins) } else { 0 })
            })
            .collect::<Option<Vec<_>>>()?;
        let win = 3 * stride + i64::from(wins);
        let mut adjacent = vec![Vec::new(); model.ids.len()];
        let games = model
            .fixtures
            .iter()
            .enumerate()
            .map(|(i, g)| {
                adjacent[g.home].push((i, 0));
                adjacent[g.away].push((i, 1));
                let gain = [[0, stride, win], [win, stride, 0]];
                let maximum = std::array::from_fn(|end| {
                    std::array::from_fn(|d| {
                        (0..3)
                            .filter(|o| d & (1 << o) != 0)
                            .map(|o| gain[end][o])
                            .max()
                            .unwrap_or(i64::MIN / 4)
                    })
                });
                Game {
                    home: g.home,
                    away: g.away,
                    gain,
                    maximum,
                }
            })
            .collect();
        Some(Self {
            base,
            games,
            adjacent,
            ranked: model.n,
        })
    }
    /// Negating the packed ordering provides the corresponding best-rank proof.
    pub fn negated(&self) -> Self {
        let games = self
            .games
            .iter()
            .map(|g| {
                let gain = g.gain.map(|v| v.map(|x| -x));
                let maximum = std::array::from_fn(|end| {
                    std::array::from_fn(|d| {
                        (0..3)
                            .filter(|o| d & (1 << o) != 0)
                            .map(|o| gain[end][o])
                            .max()
                            .unwrap_or(i64::MIN / 4)
                    })
                });
                Game {
                    home: g.home,
                    away: g.away,
                    gain,
                    maximum,
                }
            })
            .collect();
        Self {
            base: self.base.iter().map(|v| -v).collect(),
            games,
            adjacent: self.adjacent.clone(),
            ranked: self.ranked,
        }
    }
    pub fn prove(
        &self,
        cell: Cell,
        required_count: usize,
        budget: usize,
        options: Options,
    ) -> Report {
        self.prove_floor(cell, required_count, budget, options, None)
    }
    /// Necessary relaxation for seasons whose target packed total is at least
    /// `floor` in this proof's orientation. Extreme target results generously
    /// improve every rival; equality is counted in the rivals' favor. A false
    /// result, including quota exhaustion, supplies no restriction.
    pub fn prove_floor(
        &self,
        cell: Cell,
        required_count: usize,
        budget: usize,
        options: Options,
        requested_floor: Option<i64>,
    ) -> Report {
        self.prove_floor_with_limits(
            cell,
            required_count,
            budget,
            options,
            requested_floor,
            Limits::default(),
        )
    }
    pub fn prove_floor_with_limits(
        &self,
        cell: Cell,
        required_count: usize,
        budget: usize,
        options: Options,
        requested_floor: Option<i64>,
        limits: Limits,
    ) -> Report {
        if cell.team >= self.ranked || required_count >= self.ranked {
            return Report::default();
        }
        let mut initial = vec![7_u8; self.games.len()];
        let mut floor = self.base[cell.team];
        for (i, end) in &self.adjacent[cell.team] {
            let gain = self.games[*i].gain[*end];
            let minimum = *gain.iter().min().unwrap();
            initial[*i] = (0..3).fold(0, |d, o| if gain[o] == minimum { d | (1 << o) } else { d });
            floor += minimum;
        }
        if let Some(requested) = requested_floor {
            floor = floor.max(requested);
        }
        let mut maximum = self.base.clone();
        for (i, g) in self.games.iter().enumerate() {
            maximum[g.home] += g.maximum[0][initial[i] as usize];
            maximum[g.away] += g.maximum[1][initial[i] as usize];
        }
        let eligible: Vec<_> = (0..self.ranked)
            .filter(|t| *t != cell.team && maximum[*t] >= floor)
            .collect();
        if eligible.len() < required_count {
            return Report {
                impossible: true,
                stats: Stats::default(),
            };
        }
        // Difficulty signal only: an individually tight outcome indicates that
        // shared assignments may matter. Absence of this signal merely skips the
        // fallback; it does not establish feasibility or impossibility.
        if options.require_root_pressure
            && !eligible.iter().any(|&t| {
                self.adjacent[t].iter().any(|&(i, end)| {
                    let g = &self.games[i];
                    let minimum = (0..3)
                        .filter(|o| initial[i] & (1 << o) != 0)
                        .map(|o| g.gain[end][o])
                        .min()
                        .unwrap();
                    maximum[t] - g.maximum[end][initial[i] as usize] + minimum < floor
                })
            })
        {
            return Report::default();
        }
        // Bound cohort enumeration independently of the fixture-node budget.
        let k = required_count.min(eligible.len() - required_count);
        let mut count = 1_usize;
        for i in 0..k {
            count = match count.checked_mul(eligible.len() - i) {
                Some(v) => v / (i + 1),
                None => return Report::default(),
            };
            if count > limits.max_cohorts {
                return Report::default();
            }
        }
        if count > limits.max_cohorts {
            return Report::default();
        }
        let max_depth = limits.max_depth.min(512);
        if max_depth <= 128 {
            let mut solver: Solver<'_, 2> = Solver {
                p: self,
                floor,
                initial,
                initial_maximum: maximum,
                budget,
                options,
                max_depth,
                stats: Stats::default(),
                cores: Vec::new(),
                capture_feasible_domains: false,
                feasible_domains: None,
            };
            let impossible = solver.cohorts(&eligible, 0, required_count, 0);
            return Report {
                impossible,
                stats: solver.stats,
            };
        }
        let mut solver: Solver<'_, 8> = Solver {
            p: self,
            floor,
            initial,
            initial_maximum: maximum,
            budget,
            options,
            max_depth,
            stats: Stats::default(),
            cores: Vec::new(),
            capture_feasible_domains: false,
            feasible_domains: None,
        };
        let impossible = solver.cohorts(&eligible, 0, required_count, 0);
        Report {
            impossible,
            stats: solver.stats,
        }
    }

    /// Find one feasible point/win relaxation assignment for witness
    /// construction. Feasibility is only a candidate: callers must complete
    /// scores and verify the actual standings before publishing it.
    pub fn feasible_domains(
        &self,
        cell: Cell,
        required_count: usize,
        budget: usize,
        options: Options,
    ) -> FeasibleDomains {
        self.feasible_domains_with_limits(
            cell,
            required_count,
            budget,
            options,
            Limits::default(),
        )
    }
    pub fn feasible_domains_with_limits(
        &self,
        cell: Cell,
        required_count: usize,
        budget: usize,
        options: Options,
        limits: Limits,
    ) -> FeasibleDomains {
        if cell.team >= self.ranked || required_count >= self.ranked || budget == 0 {
            return FeasibleDomains::default();
        }
        let mut initial = vec![7_u8; self.games.len()];
        let mut floor = self.base[cell.team];
        for (i, end) in &self.adjacent[cell.team] {
            let gain = self.games[*i].gain[*end];
            let minimum = *gain.iter().min().unwrap();
            initial[*i] = (0..3).fold(0, |d, o| if gain[o] == minimum { d | (1 << o) } else { d });
            floor += minimum;
        }
        let mut maximum = self.base.clone();
        for (i, g) in self.games.iter().enumerate() {
            maximum[g.home] += g.maximum[0][initial[i] as usize];
            maximum[g.away] += g.maximum[1][initial[i] as usize];
        }
        let eligible: Vec<_> = (0..self.ranked)
            .filter(|t| *t != cell.team && maximum[*t] >= floor)
            .collect();
        if eligible.len() < required_count {
            return FeasibleDomains::default();
        }
        if options.require_root_pressure
            && !eligible.iter().any(|&t| {
                self.adjacent[t].iter().any(|&(i, end)| {
                    let g = &self.games[i];
                    let minimum = (0..3)
                        .filter(|o| initial[i] & (1 << o) != 0)
                        .map(|o| g.gain[end][o])
                        .min()
                        .unwrap();
                    maximum[t] - g.maximum[end][initial[i] as usize] + minimum < floor
                })
            })
        {
            return FeasibleDomains::default();
        }
        let k = required_count.min(eligible.len() - required_count);
        let mut count = 1_usize;
        for i in 0..k {
            count = match count.checked_mul(eligible.len() - i) {
                Some(v) => v / (i + 1),
                None => return FeasibleDomains::default(),
            };
            if count > limits.max_cohorts {
                return FeasibleDomains::default();
            }
        }
        if count > limits.max_cohorts {
            return FeasibleDomains::default();
        }
        let max_depth = limits.max_depth.min(512);
        if max_depth <= 128 {
            let mut solver: Solver<'_, 2> = Solver {
                p: self,
                floor,
                initial,
                initial_maximum: maximum,
                budget,
                options,
                max_depth,
                stats: Stats::default(),
                cores: Vec::new(),
                capture_feasible_domains: true,
                feasible_domains: None,
            };
            solver.cohorts(&eligible, 0, required_count, 0);
            return FeasibleDomains {
                domains: solver.feasible_domains,
                stats: solver.stats,
            };
        }
        let mut solver: Solver<'_, 8> = Solver {
            p: self,
            floor,
            initial,
            initial_maximum: maximum,
            budget,
            options,
            max_depth,
            stats: Stats::default(),
            cores: Vec::new(),
            capture_feasible_domains: true,
            feasible_domains: None,
        };
        solver.cohorts(&eligible, 0, required_count, 0);
        FeasibleDomains {
            domains: solver.feasible_domains,
            stats: solver.stats,
        }
    }
}
struct Solver<'a, const WORDS: usize> {
    p: &'a RankProof,
    floor: i64,
    initial: Vec<u8>,
    initial_maximum: Vec<i64>,
    budget: usize,
    options: Options,
    max_depth: usize,
    stats: Stats,
    cores: Vec<u64>,
    capture_feasible_domains: bool,
    feasible_domains: Option<Vec<u8>>,
}
impl<const WORDS: usize> Solver<'_, WORDS> {
    // Only restrictions that lower this endpoint's maximum can contribute to
    // its bound. A hypothetical outcome replaces its own fixture, so omit that
    // fixture's restriction when explaining why the outcome was removed.
    fn explanation(
        &self,
        t: usize,
        ds: &[u8],
        reasons: &[Reason<WORDS>],
        exclude: usize,
    ) -> Reason<WORDS> {
        let mut r = Reason::<WORDS> {
            decisions: [0; WORDS],
            teams: 1 << t,
        };
        for &(i, end) in &self.p.adjacent[t] {
            if i != exclude
                && self.p.games[i].maximum[end][ds[i] as usize]
                    < self.p.games[i].maximum[end][self.initial[i] as usize]
            {
                r.merge(reasons[i]);
            }
        }
        r
    }
    fn propagate(
        &self,
        required: u64,
        ds: &mut [u8],
        reasons: &mut [Reason<WORDS>],
    ) -> Result<Vec<i64>, Reason<WORDS>> {
        let mut maximum = self.initial_maximum.clone();
        loop {
            maximum.copy_from_slice(&self.initial_maximum);
            // Root maxima include forced target results. Unchanged fixture
            // domains need no score-table loads or endpoint updates.
            for (i, g) in self.p.games.iter().enumerate() {
                if ds[i] == self.initial[i] {
                    continue;
                }
                maximum[g.home] +=
                    g.maximum[0][ds[i] as usize] - g.maximum[0][self.initial[i] as usize];
                maximum[g.away] +=
                    g.maximum[1][ds[i] as usize] - g.maximum[1][self.initial[i] as usize];
            }
            for (t, &v) in maximum.iter().enumerate() {
                if required & (1 << t) != 0 && v < self.floor {
                    return Err(self.explanation(t, ds, reasons, usize::MAX));
                }
            }
            let mut changed = false;
            for (i, g) in self.p.games.iter().enumerate() {
                let domain = ds[i];
                // Singleton support is checked by the endpoint maxima above.
                // Games outside the required cohort cannot constrain its floor.
                if domain.is_power_of_two() || required & ((1 << g.home) | (1 << g.away)) == 0 {
                    continue;
                }
                for o in 0..3 {
                    if domain & (1 << o) == 0 {
                        continue;
                    }
                    for (t, end) in [(g.home, 0), (g.away, 1)] {
                        if required & (1 << t) != 0
                            && maximum[t] - g.maximum[end][domain as usize] + g.gain[end][o]
                                < self.floor
                        {
                            let reason = self.explanation(t, ds, reasons, i);
                            reasons[i].merge(reason);
                            ds[i] &= !(1 << o);
                            changed = true;
                            break;
                        }
                    }
                }
                if ds[i] == 0 {
                    return Err(reasons[i]);
                }
            }
            if !changed {
                return Ok(maximum);
            }
        }
    }
    fn visit(
        &mut self,
        required: u64,
        mut ds: Vec<u8>,
        mut reasons: Vec<Reason<WORDS>>,
        depth: usize,
    ) -> Answer<WORDS> {
        if self.stats.nodes >= self.budget || depth >= self.max_depth {
            return Answer::Unknown;
        }
        self.stats.nodes += 1;
        let maximum = match self.propagate(required, &mut ds, &mut reasons) {
            Ok(v) => v,
            Err(r) => return Answer::Conflict(r),
        };
        let best = self
            .p
            .games
            .iter()
            .enumerate()
            .filter(|(i, g)| {
                ds[*i].count_ones() > 1 && required & ((1 << g.home) | (1 << g.away)) != 0
            })
            .min_by_key(|(i, g)| {
                let h = required & (1 << g.home) != 0;
                let a = required & (1 << g.away) != 0;
                let sum = if h { maximum[g.home] - self.floor } else { 0 }
                    + if a { maximum[g.away] - self.floor } else { 0 };
                let slack = [(g.home, h), (g.away, a)]
                    .into_iter()
                    .filter(|(_, r)| *r)
                    .map(|(t, _)| maximum[t] - self.floor)
                    .min()
                    .unwrap();
                if self.options.internal_order {
                    (
                        ds[*i].count_ones(),
                        2 - usize::from(h) - usize::from(a),
                        sum,
                        *i,
                    )
                } else {
                    (ds[*i].count_ones(), 0, slack, *i)
                }
            })
            .map(|(i, _)| i);
        let Some(i) = best else {
            if self.capture_feasible_domains {
                self.feasible_domains = Some(ds);
            }
            return Answer::Feasible;
        };
        let g = &self.p.games[i];
        let mut options: Vec<_> = (0..3).filter(|o| ds[i] & (1 << o) != 0).collect();
        options.sort_by_key(|o| {
            -(if required & (1 << g.home) != 0 {
                g.gain[0][*o]
            } else {
                0
            } + if required & (1 << g.away) != 0 {
                g.gain[1][*o]
            } else {
                0
            })
        });
        // The parent domain may already exclude values. Those explanations must
        // be retained when resolving child conflicts over only its allowed values.
        let mut union = reasons[i];
        for o in options {
            let mut next = ds.clone();
            next[i] = 1 << o;
            let mut why = reasons.clone();
            why[i].insert(depth);
            match self.visit(required, next, why, depth + 1) {
                Answer::Conflict(r) => {
                    if self.options.explain && !r.contains(depth) {
                        self.stats.backjumps += 1;
                        return Answer::Conflict(r);
                    }
                    union.merge(r);
                }
                other => return other,
            }
        }
        union.remove(depth);
        Answer::Conflict(union)
    }
    fn cohorts(&mut self, candidates: &[usize], start: usize, left: usize, required: u64) -> bool {
        if left == 0 {
            self.stats.cohorts += 1;
            if self.options.reuse_cohorts && self.cores.iter().any(|core| core & required == *core)
            {
                self.stats.cohort_cache_hits += 1;
                return true;
            }
            if self.stats.nodes >= self.budget {
                return false;
            }
            match self.visit(
                required,
                self.initial.clone(),
                vec![Reason::<WORDS>::default(); self.p.games.len()],
                0,
            ) {
                Answer::Conflict(r) => {
                    debug_assert!(r.is_empty());
                    debug_assert_eq!(r.teams & required, r.teams);
                    if self.options.reuse_cohorts
                        && !self.cores.iter().any(|core| core & r.teams == *core)
                    {
                        self.cores.retain(|core| r.teams & *core != r.teams);
                        self.cores.push(r.teams);
                    }
                    true
                }
                _ => false,
            }
        } else {
            for i in start..=candidates.len() - left {
                if !self.cohorts(candidates, i + 1, left - 1, required | (1 << candidates[i])) {
                    return false;
                }
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Request;
    use serde_json::json;
    #[test]
    fn decision_reasons_merge_and_clear_across_the_full_depth_range() {
        let mut a = Reason::<8>::default();
        for bit in [127, 128, 255, 511] {
            a.insert(bit);
            assert!(a.contains(bit));
        }
        let mut b = Reason::<8>::default();
        b.insert(128);
        b.insert(300);
        a.merge(b);
        assert!(a.contains(127) && a.contains(128) && a.contains(255));
        assert!(a.contains(300) && a.contains(511));
        for bit in [127, 128, 255, 300, 511] {
            a.remove(bit);
        }
        assert!(a.is_empty());
    }
    fn snapshot() -> Model {
        let request:Request=serde_json::from_str(include_str!("../../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json")).unwrap();
        Model::new(request).unwrap()
    }
    #[test]
    fn threshold_refutations_never_remove_a_legal_outcome_in_either_direction() {
        for sort in ["pt,w,gd,gf", "pt,gd,gf,w"] {
            let request:Request=serde_json::from_value(json!({"id":1,"phase":{"sort":sort,"championship":{"point_win":3,"point_draw":1,"point_loss":0}},
                "team_groups":[{"team_id":1,"add_sub":2},{"team_id":2},{"team_id":3},{"team_id":4}],
                "games":[{"id":1,"home_id":1,"away_id":2,"played":true,"home_score":1,"away_score":0},
                {"id":2,"home_id":1,"away_id":3},{"id":3,"home_id":2,"away_id":4},{"id":4,"home_id":2,"away_id":3},{"id":5,"home_id":3,"away_id":4}]})).unwrap();
            let m = Model::new(request).unwrap();
            let p = RankProof::new(&m).unwrap();
            let neg = p.negated();
            for proof in [&p, &neg] {
                let seasons: Vec<_> = (0..81)
                    .map(|mut code| {
                        let mut totals = proof.base.clone();
                        for g in &proof.games {
                            let o = code % 3;
                            code /= 3;
                            totals[g.home] += g.gain[0][o];
                            totals[g.away] += g.gain[1][o];
                        }
                        totals
                    })
                    .collect();
                for team in 0..m.n {
                    let floors: std::collections::BTreeSet<_> = seasons
                        .iter()
                        .flat_map(|s| [s[team], s[team] + 1])
                        .collect();
                    for floor in floors {
                        let ahead = seasons
                            .iter()
                            .filter(|s| s[team] >= floor)
                            .map(|s| (0..m.n).filter(|&t| t != team && s[t] >= s[team]).count())
                            .max();
                        for rank in 0..m.n {
                            let r = proof.prove_floor(
                                Cell { team, rank },
                                rank,
                                10000,
                                Options::default(),
                                Some(floor),
                            );
                            assert!(!r.impossible || ahead.is_none_or(|a|a<rank),"false refutation: {sort}, target {team}, floor {floor}, rank {rank}");
                            let expanded = proof.prove_floor_with_limits(
                                Cell { team, rank },
                                rank,
                                10000,
                                Options::default(),
                                Some(floor),
                                Limits { max_cohorts: 100_000, max_depth: 512 },
                            );
                            assert!(
                                !expanded.impossible || ahead.is_none_or(|a| a < rank),
                                "false expanded refutation: {sort}, target {team}, floor {floor}, rank {rank}"
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn explained_search_refutes_snapshot_with_existing_cell_quota() {
        let m = snapshot();
        let p = RankProof::new(&m).unwrap();
        let cell = Cell {
            team: m.indices[&17],
            rank: 13,
        };
        let result = p.prove(cell, 13, 500, Options::default());
        assert!(result.impossible);
        assert!(result.stats.nodes <= 500);
        assert!(result.stats.cohort_cache_hits > 0);
        assert!(result.stats.backjumps > 0);
        let budgeted = p.prove(
            cell,
            13,
            500,
            Options {
                require_root_pressure: true,
                ..Options::default()
            },
        );
        assert!(budgeted.impossible);
        let short = p.prove(cell, 13, 100, Options::default());
        assert!(!short.impossible);
        let empty = p.prove(cell, 13, 0, Options::default());
        assert!(!empty.impossible);
        assert_eq!(empty.stats.nodes, 0);
    }
    #[test]
    fn feasible_domain_capture_finds_palmeiras_thirteenth_within_small_budget() {
        let request: Request = serde_json::from_str(include_str!(
            "../tests/fixtures/group-16498-2026-10-10.json"
        ))
        .unwrap();
        let model = Model::new(request).unwrap();
        let proof = RankProof::new(&model).unwrap();
        let cell = Cell {
            team: model.indices[&16],
            rank: 12,
        };
        let found = proof.feasible_domains(cell, 12, 100, Options::default());
        assert!(found.domains.is_some(), "nodes={}", found.stats.nodes);
        assert!(found.stats.nodes <= 100);
        assert!(found.stats.cohorts > 0);
        assert!(found
            .domains
            .unwrap()
            .iter()
            .all(|domain| (1..=7).contains(domain)));

        let short = proof.feasible_domains(cell, 12, 1, Options::default());
        assert!(short.domains.is_none());
        assert!(short.stats.nodes <= 1);
        let empty = proof.feasible_domains(cell, 12, 0, Options::default());
        assert!(empty.domains.is_none());
        assert_eq!(empty.stats.nodes, 0);
    }
    #[test]
    fn expanded_depth_finds_a_feasible_domain_after_128_decisions() {
        let request = json!({
            "id": 99005,
            "phase": {"sort":"pt,gd,gf,bias", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups": (0..3).map(|team| json!({"team_id":team,"add_sub":0,"bias":team})).collect::<Vec<_>>(),
            "games": (0..160).map(|id| json!({"id":id+1,"home_id":1,"away_id":2,"home_power":1.0,"away_power":1.0})).collect::<Vec<_>>()
        });
        let model = Model::new(serde_json::from_value(request).unwrap()).unwrap();
        let proof = RankProof::new(&model).unwrap();
        let cell = Cell { team: 0, rank: 1 };
        let default = proof.feasible_domains(cell, 1, 1000, Options::default());
        assert!(default.domains.is_none());

        let expanded = proof.feasible_domains_with_limits(
            cell,
            1,
            1000,
            Options::default(),
            Limits {
                max_cohorts: 100_000,
                max_depth: 512,
            },
        );
        assert!(expanded.domains.is_some());
        assert!(expanded.stats.nodes > 128, "{:?}", expanded.stats);
        assert!(expanded.stats.nodes <= 1000);
    }
    #[test]
    fn learned_conflicts_and_both_directions_match_exhaustive_relaxation() {
        for adjustment in [0, 1, 3, 6] {
            for sort in ["pt,w,gd,gf", "pt,gd,gf,w"] {
                let request:Request=serde_json::from_value(json!({"id":1,"phase":{"sort":sort,"championship":{"point_win":3,"point_draw":1,"point_loss":0}},
                "team_groups":[{"team_id":1,"add_sub":adjustment},{"team_id":2},{"team_id":3},{"team_id":4}],
                "games":[{"id":1,"home_id":1,"away_id":2,"played":true,"home_score":1,"away_score":0},
                {"id":2,"home_id":1,"away_id":3},{"id":3,"home_id":2,"away_id":4},{"id":4,"home_id":2,"away_id":3},{"id":5,"home_id":3,"away_id":4}]})).unwrap();
                let m = Model::new(request).unwrap();
                let p = RankProof::new(&m).unwrap();
                let neg = p.negated();
                for (proof, direction) in [(&p, 1), (&neg, -1)] {
                    let mut maximum_ahead = vec![0; m.n];
                    for mut code in 0..3_usize.pow(m.fixtures.len() as u32) {
                        let mut totals = proof.base.clone();
                        for g in &proof.games {
                            let o = code % 3;
                            code /= 3;
                            totals[g.home] += g.gain[0][o];
                            totals[g.away] += g.gain[1][o];
                        }
                        for t in 0..m.n {
                            maximum_ahead[t] = maximum_ahead[t].max(
                                (0..m.n)
                                    .filter(|other| *other != t && totals[*other] >= totals[t])
                                    .count(),
                            );
                        }
                    }
                    for (team, &ahead) in maximum_ahead.iter().enumerate() {
                        for rank in 0..m.n {
                            for options in [
                                Options {
                                    explain: false,
                                    internal_order: false,
                                    reuse_cohorts: false,
                                    require_root_pressure: false,
                                },
                                Options::default(),
                            ] {
                                let r = proof.prove(Cell { team, rank }, rank, 100000, options);
                                assert_eq!(r.impossible,ahead<rank,"adjustment={adjustment} sort={sort} direction={direction} team={team} required={rank}");
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn rejects_unsupported_scoring_and_bonus_points() {
        let m = snapshot();
        let mut r = m.request.clone();
        r.phase.bonus_points = 1;
        assert!(RankProof::new(&Model::new(r).unwrap()).is_none());
        let mut r = m.request.clone();
        r.phase.championship.point_win = 2;
        assert!(RankProof::new(&Model::new(r).unwrap()).is_none());
    }
}
