//! Bounded extreme-rank relaxation with explained fixture conflicts.
//! A feasible P/W assignment is not a reachability witness. Only exhaustion of
//! every required-rival cohort can prove impossibility, without restricting goals.
use crate::{
    model::{Key, Model},
    search::Cell,
};
use serde::Serialize;

#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct Reason {
    decisions: u128,
    teams: u64,
}
impl Reason {
    fn merge(&mut self, other: Self) {
        self.decisions |= other.decisions;
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
enum Answer {
    Conflict(Reason),
    Feasible,
    Unknown,
}
#[derive(Default, Serialize)]
pub struct Report {
    pub impossible: bool,
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
            if count > 512 {
                return Report::default();
            }
        }
        let mut solver = Solver {
            p: self,
            floor,
            initial,
            initial_maximum: maximum,
            budget,
            options,
            stats: Stats::default(),
            cores: Vec::new(),
        };
        let impossible = solver.cohorts(&eligible, 0, required_count, 0);
        Report {
            impossible,
            stats: solver.stats,
        }
    }
}
struct Solver<'a> {
    p: &'a RankProof,
    floor: i64,
    initial: Vec<u8>,
    initial_maximum: Vec<i64>,
    budget: usize,
    options: Options,
    stats: Stats,
    cores: Vec<u64>,
}
impl Solver<'_> {
    // Only restrictions that lower this endpoint's maximum can contribute to
    // its bound. A hypothetical outcome replaces its own fixture, so omit that
    // fixture's restriction when explaining why the outcome was removed.
    fn explanation(&self, t: usize, ds: &[u8], reasons: &[Reason], exclude: usize) -> Reason {
        let mut r = Reason {
            decisions: 0,
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
        reasons: &mut [Reason],
    ) -> Result<Vec<i64>, Reason> {
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
        mut reasons: Vec<Reason>,
        depth: usize,
    ) -> Answer {
        if self.stats.nodes >= self.budget || depth >= 128 {
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
            return Answer::Feasible;
        };
        let g = &self.p.games[i];
        let bit = 1_u128 << depth;
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
            why[i].decisions |= bit;
            match self.visit(required, next, why, depth + 1) {
                Answer::Conflict(r) => {
                    if self.options.explain && r.decisions & bit == 0 {
                        self.stats.backjumps += 1;
                        return Answer::Conflict(r);
                    }
                    union.merge(r);
                }
                other => return other,
            }
        }
        union.decisions &= !bit;
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
                vec![Reason::default(); self.p.games.len()],
                0,
            ) {
                Answer::Conflict(r) => {
                    debug_assert_eq!(r.decisions, 0);
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
    fn snapshot() -> Model {
        let request:Request=serde_json::from_str(include_str!("../../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json")).unwrap();
        Model::new(request).unwrap()
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
