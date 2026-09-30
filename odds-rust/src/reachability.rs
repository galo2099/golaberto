//! Experimental bounded joint fixture solver. Feasible point/win intervals
//! are a relaxation, not a witness. Only the production sorter verifies one.
use crate::{
    conditioned::canonical_ranks,
    model::{Key, Model},
    pool::Estimate,
    search::Cell,
};
use serde::Serialize;
use std::{collections::HashSet, time::Instant};

#[derive(Clone)]
struct Game {
    home: usize,
    away: usize,
    home_gain: [i64; 3],
    away_gain: [i64; 3],
    prob: [f64; 3],
}
pub struct JointProblem {
    base: Vec<i64>,
    games: Vec<Game>,
    ranked: usize,
}
#[derive(Clone)]
pub struct Bounds {
    pub lower: Vec<i64>,
    pub upper: Vec<i64>,
}
fn minimum(d: u8, gains: [i64; 3]) -> i64 {
    (0..3)
        .filter(|o| d & (1 << o) != 0)
        .map(|o| gains[o])
        .min()
        .unwrap()
}
fn maximum(d: u8, gains: [i64; 3]) -> i64 {
    (0..3)
        .filter(|o| d & (1 << o) != 0)
        .map(|o| gains[o])
        .max()
        .unwrap()
}

impl JointProblem {
    pub fn new(model: &Model) -> Option<Self> {
        if model.n < 2
            || model.keys.first() != Some(&Key::Pt)
            || model.fixtures.iter().any(|g| g.home == g.away)
        {
            return None;
        }
        let wins = model.keys.get(1) == Some(&Key::W);
        let mut remaining = vec![0; model.ids.len()];
        for g in &model.fixtures {
            remaining[g.home] += 1;
            remaining[g.away] += 1;
        }
        // All possible final win counts are smaller than this stride.
        let stride = if wins {
            (0..model.ids.len())
                .map(|t| i64::from(model.base[t].wins) + remaining[t])
                .max()?
                + 1
        } else {
            1
        };
        let c = &model.request.phase.championship;
        let hp = [c.point_loss, c.point_draw, c.point_win];
        let ap = [c.point_win, c.point_draw, c.point_loss];
        Some(Self {
            base: model
                .base
                .iter()
                .map(|c| i64::from(c.points) * stride + if wins { i64::from(c.wins) } else { 0 })
                .collect(),
            ranked: model.n,
            games: model
                .fixtures
                .iter()
                .map(|g| Game {
                    home: g.home,
                    away: g.away,
                    prob: g.prob,
                    home_gain: std::array::from_fn(|o| {
                        i64::from(hp[o]) * stride + i64::from(wins && o == 2)
                    }),
                    away_gain: std::array::from_fn(|o| {
                        i64::from(ap[o]) * stride + i64::from(wins && o == 0)
                    }),
                })
                .collect(),
        })
    }
    pub fn initial_domains(&self) -> Vec<u8> {
        // Reachability is a fixture-outcome question. A numerically zero mass
        // in the truncated score table cannot refute an unrestricted outcome.
        vec![7; self.games.len()]
    }
    fn possible(&self, b: &Bounds, target: usize, rank: usize) -> bool {
        let above = (0..self.ranked)
            .filter(|t| *t != target && b.lower[*t] > b.upper[target])
            .count();
        let below = (0..self.ranked)
            .filter(|t| *t != target && b.upper[*t] < b.lower[target])
            .count();
        above <= rank && below <= self.ranked - 1 - rank
    }
    /// Necessary rank screen for a fixed maximum/minimum target-point pattern.
    /// Rejection skips only that pattern, never proves the cell impossible.
    pub fn extreme_possible(&self, model: &Model, cell: Cell, maximum_points: bool) -> bool {
        let c = &model.request.phase.championship;
        let hp = [c.point_loss, c.point_draw, c.point_win];
        let ap = [c.point_win, c.point_draw, c.point_loss];
        // This screen skips the *existing constructive proposal*, which uses
        // positive numerical outcome mass. It never declares impossibility.
        let mut domains: Vec<_> = self
            .games
            .iter()
            .map(|g| (0..3).fold(0, |d, o| if g.prob[o] > 0. { d | (1 << o) } else { d }))
            .collect();
        let mut b = Bounds {
            lower: self.base.clone(),
            upper: self.base.clone(),
        };
        for (i, g) in self.games.iter().enumerate() {
            let points = if g.home == cell.team {
                Some(hp)
            } else if g.away == cell.team {
                Some(ap)
            } else {
                None
            };
            if let Some(points) = points {
                let feasible = (0..3)
                    .filter(|o| domains[i] & (1 << o) != 0)
                    .map(|o| points[o]);
                let extreme = if maximum_points {
                    feasible.max()
                } else {
                    feasible.min()
                };
                let Some(extreme) = extreme else {
                    return false;
                };
                domains[i] &= (0..3).fold(0, |d, o| {
                    if points[o] == extreme {
                        d | (1 << o)
                    } else {
                        d
                    }
                });
            }
            if domains[i] == 0 {
                return false;
            }
            b.lower[g.home] += minimum(domains[i], g.home_gain);
            b.upper[g.home] += maximum(domains[i], g.home_gain);
            b.lower[g.away] += minimum(domains[i], g.away_gain);
            b.upper[g.away] += maximum(domains[i], g.away_gain);
        }
        self.possible(&b, cell.team, cell.rank)
    }
    pub fn propagate(&self, target: usize, rank: usize, domains: &mut [u8]) -> Option<Bounds> {
        let n = self.base.len();
        let m = self.games.len();
        let mut b = Bounds {
            lower: self.base.clone(),
            upper: self.base.clone(),
        };
        let mut lo = vec![i64::MIN / 4; n];
        let mut hi = vec![i64::MAX / 4; n];
        let mut hm = vec![0; m];
        let mut hx = hm.clone();
        let mut am = hm.clone();
        let mut ax = hm.clone();
        loop {
            b.lower.copy_from_slice(&self.base);
            b.upper.copy_from_slice(&self.base);
            for (i, g) in self.games.iter().enumerate() {
                if domains[i] == 0 {
                    return None;
                }
                hm[i] = minimum(domains[i], g.home_gain);
                hx[i] = maximum(domains[i], g.home_gain);
                am[i] = minimum(domains[i], g.away_gain);
                ax[i] = maximum(domains[i], g.away_gain);
                b.lower[g.home] += hm[i];
                b.upper[g.home] += hx[i];
                b.lower[g.away] += am[i];
                b.upper[g.away] += ax[i];
            }
            let raw_lower = b.lower.clone();
            let raw_upper = b.upper.clone();
            for t in 0..n {
                b.lower[t] = b.lower[t].max(lo[t]);
                b.upper[t] = b.upper[t].min(hi[t]);
                if b.lower[t] > b.upper[t] {
                    return None;
                }
            }
            if !self.possible(&b, target, rank) {
                return None;
            }
            let above = (0..self.ranked)
                .filter(|t| *t != target && b.lower[*t] > b.upper[target])
                .count();
            let below = (0..self.ranked)
                .filter(|t| *t != target && b.upper[*t] < b.lower[target])
                .count();
            let mut changed = false;
            for t in 0..self.ranked {
                if t == target {
                    continue;
                }
                if above == rank && b.lower[t] <= b.upper[target] {
                    let v = hi[t].min(b.upper[target]);
                    changed |= v != hi[t];
                    hi[t] = v;
                    let v = lo[target].max(b.lower[t]);
                    changed |= v != lo[target];
                    lo[target] = v;
                }
                if below == self.ranked - 1 - rank && b.upper[t] >= b.lower[target] {
                    let v = lo[t].max(b.lower[target]);
                    changed |= v != lo[t];
                    lo[t] = v;
                    let v = hi[target].min(b.upper[t]);
                    changed |= v != hi[target];
                    hi[target] = v;
                }
            }
            // Test each game's shared outcome against both endpoints and rank
            // cardinality. Bounds are conservative, so rejection is sound.
            for (i, g) in self.games.iter().enumerate() {
                let mut d = domains[i];
                for o in 0..3 {
                    if d & (1 << o) == 0 {
                        continue;
                    }
                    // Intersect global bounds after subtracting the fixture;
                    // otherwise the hypothetical outcome bounds can be too tight.
                    let raw_hl = raw_lower[g.home] - hm[i] + g.home_gain[o];
                    let raw_hu = raw_upper[g.home] - hx[i] + g.home_gain[o];
                    let raw_al = raw_lower[g.away] - am[i] + g.away_gain[o];
                    let raw_au = raw_upper[g.away] - ax[i] + g.away_gain[o];
                    if raw_hl > hi[g.home]
                        || raw_hu < lo[g.home]
                        || raw_al > hi[g.away]
                        || raw_au < lo[g.away]
                    {
                        d &= !(1 << o);
                        changed = true;
                        continue;
                    }
                    let old = [
                        b.lower[g.home],
                        b.upper[g.home],
                        b.lower[g.away],
                        b.upper[g.away],
                    ];
                    b.lower[g.home] = raw_hl.max(lo[g.home]);
                    b.upper[g.home] = raw_hu.min(hi[g.home]);
                    b.lower[g.away] = raw_al.max(lo[g.away]);
                    b.upper[g.away] = raw_au.min(hi[g.away]);
                    if !self.possible(&b, target, rank) {
                        d &= !(1 << o);
                        changed = true;
                    }
                    [
                        b.lower[g.home],
                        b.upper[g.home],
                        b.lower[g.away],
                        b.upper[g.away],
                    ] = old;
                }
                if d == 0 {
                    return None;
                }
                domains[i] = d;
            }
            if !changed {
                return Some(b);
            }
        }
    }
    fn completion(&self, domains: &[u8], b: &Bounds, cell: Cell) -> Vec<u8> {
        let mut outcomes: Vec<_> = self
            .games
            .iter()
            .zip(domains)
            .map(|(g, d)| {
                (0..3)
                    .filter(|o| d & (1 << o) != 0)
                    .max_by(|a, b| g.prob[*a].total_cmp(&g.prob[*b]))
                    .unwrap() as u8
            })
            .collect();
        let mut scores = self.base.clone();
        for (g, o) in self.games.iter().zip(&outcomes) {
            scores[g.home] += g.home_gain[*o as usize];
            scores[g.away] += g.away_gain[*o as usize];
        }
        let mut rivals: Vec<_> = (0..self.ranked)
            .filter(|t| *t != cell.team)
            .map(|t| scores[t])
            .collect();
        rivals.sort_unstable_by(|a, b| b.cmp(a));
        let goal = if cell.rank == 0 {
            rivals[0] + 1
        } else if cell.rank == self.ranked - 1 {
            rivals[self.ranked - 2] - 1
        } else {
            (rivals[cell.rank - 1] + rivals[cell.rank]) / 2
        };
        let goal = goal.clamp(b.lower[cell.team], b.upper[cell.team]);
        for (i, g) in self.games.iter().enumerate() {
            let gains = if g.home == cell.team {
                g.home_gain
            } else if g.away == cell.team {
                g.away_gain
            } else {
                continue;
            };
            let old = outcomes[i] as usize;
            let best = (0..3)
                .filter(|o| domains[i] & (1 << o) != 0)
                .min_by_key(|o| {
                    (
                        (scores[cell.team] - gains[old] + gains[*o] - goal).abs(),
                        *o,
                    )
                })
                .unwrap();
            scores[cell.team] += gains[best] - gains[old];
            outcomes[i] = best as u8;
        }
        outcomes
    }
}

#[derive(Default, Serialize)]
pub struct Stats {
    pub nodes: usize,
    pub cache_hits: usize,
    pub sorter_checks: usize,
    pub reachable: usize,
    pub impossible: usize,
    pub exhausted: usize,
    pub setup_ms: f64,
    pub elapsed_ms: f64,
}
pub struct Run {
    pub outcomes: Vec<Vec<u8>>,
    pub stats: Stats,
}
enum Answer {
    Witness,
    Impossible,
    Unknown,
}
struct Solver<'a> {
    problem: &'a JointProblem,
    model: &'a Model,
    cell: Cell,
    nodes: usize,
    budget: usize,
    conflicts: HashSet<Vec<u8>>,
    stats: &'a mut Stats,
    estimates: &'a mut [Estimate],
    found: &'a mut Vec<Vec<u8>>,
}
impl Solver<'_> {
    fn check(&mut self, outcomes: Vec<u8>) -> bool {
        self.stats.sorter_checks += 1;
        let ranks = canonical_ranks(self.model, &outcomes);
        let mut used = false;
        for (t, &rank) in ranks.iter().enumerate() {
            let e = &mut self.estimates[t * self.model.n + rank];
            if e.probability == 0.
                && !e.reachability.starts_with("impossible")
                && e.reachability != "reachable_by_construction"
                && e.reachability != "witness"
            {
                e.reachability = "reachable_by_construction".into();
                self.stats.reachable += 1;
                used = true;
            }
        }
        if used {
            self.found.push(outcomes);
        }
        ranks[self.cell.team] == self.cell.rank
    }
    fn visit(&mut self, mut domains: Vec<u8>, depth: usize) -> Answer {
        if self.nodes >= self.budget {
            return Answer::Unknown;
        }
        self.nodes += 1;
        self.stats.nodes += 1;
        let Some(b) = self
            .problem
            .propagate(self.cell.team, self.cell.rank, &mut domains)
        else {
            return Answer::Impossible;
        };
        if self.conflicts.contains(&domains) {
            self.stats.cache_hits += 1;
            return Answer::Impossible;
        }
        let best = self
            .problem
            .games
            .iter()
            .enumerate()
            .filter(|(i, g)| {
                domains[*i].count_ones() > 1
                    && [g.home, g.away].iter().any(|t| {
                        *t == self.cell.team
                            || *t < self.problem.ranked
                                && b.lower[*t] <= b.upper[self.cell.team]
                                && b.upper[*t] >= b.lower[self.cell.team]
                    })
            })
            .min_by_key(|(i, g)| {
                (
                    domains[*i].count_ones(),
                    usize::from(g.home != self.cell.team && g.away != self.cell.team),
                    *i,
                )
            })
            .map(|(i, _)| i);
        let completion = self.problem.completion(&domains, &b, self.cell);
        if (depth == 0 || best.is_none()) && self.check(completion.clone()) {
            return Answer::Witness;
        }
        let Some(i) = best else {
            // Failed canonical goals do not refute this outcome pattern.
            return Answer::Unknown;
        };
        let mut options: Vec<_> = (0..3).filter(|o| domains[i] & (1 << o) != 0).collect();
        options.sort_by_key(|o| usize::from(*o != completion[i] as usize));
        let mut all_impossible = true;
        for o in options {
            if self.nodes >= self.budget {
                return Answer::Unknown;
            }
            let mut next = domains.clone();
            next[i] = 1 << o;
            match self.visit(next, depth + 1) {
                Answer::Witness => return Answer::Witness,
                Answer::Unknown => all_impossible = false,
                Answer::Impossible => {}
            }
        }
        if all_impossible {
            self.conflicts.insert(domains);
            Answer::Impossible
        } else {
            Answer::Unknown
        }
    }
}
pub fn run(
    model: &Model,
    cells: &[Cell],
    estimates: &mut [Estimate],
    per_cell: usize,
    total: usize,
) -> Run {
    run_ordered(
        model,
        cells,
        estimates,
        per_cell,
        total,
        std::env::var("RUST_ODDS_JOINT_ROOT_SWEEP").as_deref() == Ok("1"),
    )
}
pub fn run_ordered(
    model: &Model,
    cells: &[Cell],
    estimates: &mut [Estimate],
    per_cell: usize,
    total: usize,
    root_sweep: bool,
) -> Run {
    let start = Instant::now();
    let mut stats = Stats::default();
    let mut outcomes = Vec::new();
    let Some(problem) = JointProblem::new(model) else {
        return Run { outcomes, stats };
    };
    stats.setup_ms = start.elapsed().as_secs_f64() * 1000.;
    let mut used = vec![0; cells.len()];
    let passes = if root_sweep {
        vec![1, per_cell]
    } else {
        vec![per_cell]
    };
    for quota in passes {
        for (index, &cell) in cells.iter().enumerate() {
            let e = &estimates[cell.index(model.n)];
            if e.probability != 0.
                || e.reachability != "undecided"
                || stats.nodes >= total
                || used[index] >= per_cell
            {
                continue;
            }
            let budget = quota.min(per_cell - used[index]).min(total - stats.nodes);
            let mut solver = Solver {
                problem: &problem,
                model,
                cell,
                nodes: 0,
                budget,
                conflicts: HashSet::new(),
                stats: &mut stats,
                estimates,
                found: &mut outcomes,
            };
            let answer = solver.visit(problem.initial_domains(), 0);
            let spent = solver.nodes;
            drop(solver);
            used[index] += spent;
            if matches!(answer, Answer::Impossible) {
                let e = &mut estimates[cell.index(model.n)];
                e.reachability = "impossible_by_joint_rank".into();
                e.zero_hit_upper_95 = 0.;
                stats.impossible += 1;
            }
            if quota == per_cell && spent >= budget && matches!(answer, Answer::Unknown) {
                stats.exhausted += 1;
            }
        }
    }
    stats.elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
    if std::env::var("RUST_ODDS_REACHABILITY_TRACE").as_deref() == Ok("1") {
        eprintln!(
            "{}",
            serde_json::json!({"event":"rust_reachability_joint","group":model.request.id,"stats":stats,"root_sweep":root_sweep,"cell_node_limit":per_cell,"node_limit":total})
        );
    }
    Run { outcomes, stats }
}
