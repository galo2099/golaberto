//! Joint conditioning on a complete, bounded union of propagated target paths.
//! Every restriction is necessary for the requested packed rank. Exhausting a
//! setup budget skips this proposal; it never establishes impossibility.
use super::{broad::TerminalTable, choose};
use crate::{
    conditioned::{Result, ScoreContext},
    domains::Domains,
    lookahead::{RankGame, Suffix},
    model::{Key, Model},
    pool::work_per_sample,
    rng::Rng,
    search::Cell,
};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

pub mod lazy;

const PATTERN_LIMIT: usize = 64;

#[derive(Clone, Copy)]
pub struct Limits {
    pub minimum_forced: usize,
    pub rivals: usize,
    pub target_patterns: usize,
    pub feasible_cases: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            minimum_forced: 4,
            rivals: 6,
            target_patterns: PATTERN_LIMIT,
            feasible_cases: 16,
        }
    }
}
const BASE_SHARE: f64 = 1e-6;

struct JointPattern {
    outcomes: Vec<u8>,
    added: Vec<usize>,
    cumulative: f64,
}
struct NecessaryJoint {
    games: Vec<RankGame>,
    internal: Vec<(RankGame, usize, usize)>,
    tables: Vec<TerminalTable>,
    patterns: Vec<JointPattern>,
    mass: f64,
    selected: Vec<usize>,
    states: usize,
    residual_hint: f64,
}
impl NecessaryJoint {
    fn new(
        games: &[RankGame],
        base: &[i32],
        lower: &[i32],
        upper: &[i32],
        limit: usize,
        budget: usize,
    ) -> Option<(Self, Vec<RankGame>)> {
        let mut budget = budget;
        Self::metered(games, base, lower, upper, limit, &mut budget)
    }
    fn metered(
        games: &[RankGame],
        base: &[i32],
        lower: &[i32],
        upper: &[i32],
        limit: usize,
        budget: &mut usize,
    ) -> Option<(Self, Vec<RankGame>)> {
        let terminal = |t: usize| {
            let team_games = games.iter().filter(|g| g.home == t || g.away == t);
            let span: i32 = team_games
                .map(|g| {
                    *if g.home == t { &g.hg } else { &g.ag }
                        .iter()
                        .max()
                        .unwrap()
                })
                .sum();
            (0..=span)
                .map(|gain| {
                    if base[t] + gain >= lower[t] && base[t] + gain <= upper[t] {
                        1.
                    } else {
                        0.
                    }
                })
                .collect::<Vec<_>>()
        };
        let mut candidates = (0..base.len())
            .filter_map(|t| {
                if lower[t] <= i32::MIN / 8 && upper[t] >= i32::MAX / 8 {
                    return None;
                }
                let table = TerminalTable::new(
                    games
                        .iter()
                        .copied()
                        .filter(|g| g.home == t || g.away == t)
                        .collect(),
                    t,
                    terminal(t),
                );
                let mass = table.mass(0);
                (mass < 1. - 1e-12).then_some((t, mass))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        let mut selected = Vec::new();
        for &(t, _) in &candidates {
            if selected.len() >= limit {
                break;
            }
            selected.push(t);
            let states = games
                .iter()
                .filter(|g| selected.contains(&g.home) && selected.contains(&g.away))
                .try_fold(1usize, |n, g| {
                    n.checked_mul(g.prob.iter().filter(|&&p| p > 0.).count())
                });
            if states.is_none_or(|n| n > 12000) {
                selected.pop();
            }
        }
        let residual_hint = candidates
            .iter()
            .filter(|(t, _)| !selected.contains(t))
            .map(|(_, p)| *p)
            .product();
        let mut internal = Vec::new();
        let mut external = vec![Vec::new(); selected.len()];
        let mut remaining = Vec::new();
        let mut fixed = Vec::new();
        for &g in games {
            match (
                selected.iter().position(|&t| t == g.home),
                selected.iter().position(|&t| t == g.away),
            ) {
                (Some(h), Some(a)) => {
                    internal.push((g, h, a));
                    fixed.push(g);
                }
                (Some(t), None) | (None, Some(t)) => {
                    external[t].push(g);
                    fixed.push(g);
                }
                _ => remaining.push(g),
            }
        }
        let tables = selected
            .iter()
            .zip(external)
            .map(|(&t, gs)| TerminalTable::new(gs, t, terminal(t)))
            .collect::<Vec<_>>();
        let mut future_min = vec![vec![0usize; selected.len()]; internal.len() + 1];
        let mut future_max = future_min.clone();
        for i in (0..internal.len()).rev() {
            future_min[i] = future_min[i + 1].clone();
            future_max[i] = future_max[i + 1].clone();
            let (g, h, a) = &internal[i];
            for (t, gain) in [(*h, g.hg), (*a, g.ag)] {
                future_min[i][t] += (0..3)
                    .filter(|&o| g.prob[o] > 0.)
                    .map(|o| gain[o] as usize)
                    .min()
                    .unwrap();
                future_max[i][t] += (0..3)
                    .filter(|&o| g.prob[o] > 0.)
                    .map(|o| gain[o] as usize)
                    .max()
                    .unwrap();
            }
        }
        let mut nodes = 0usize;
        let mut states = vec![(Vec::new(), vec![0usize; selected.len()], 1.)];
        for (step, (g, h, a)) in internal.iter().enumerate() {
            let mut next = Vec::new();
            for (outcomes, added, mass) in states {
                for o in 0..3 {
                    if g.prob[o] <= 0. {
                        continue;
                    }
                    if *budget == 0 {
                        return None;
                    }
                    *budget -= 1;
                    nodes += 1;
                    if nodes > 20000 {
                        let (mut joint, remaining) = Self::metered(
                            games,
                            base,
                            lower,
                            upper,
                            selected.len().saturating_sub(1),
                            budget,
                        )?;
                        joint.states += nodes;
                        return Some((joint, remaining));
                    }
                    let mut added = added.clone();
                    added[*h] += g.hg[o] as usize;
                    added[*a] += g.ag[o] as usize;
                    let possible = selected.iter().enumerate().all(|(i, &t)| {
                        let lo = added[i] + future_min[step + 1][i];
                        let hi = added[i] + future_max[step + 1][i];
                        if upper[t] >= i32::MAX / 8 {
                            tables[i].mass(hi) > 0.
                        } else if lower[t] <= i32::MIN / 8 {
                            tables[i].mass(lo) > 0.
                        } else {
                            (lo..=hi).any(|a| tables[i].mass(a) > 0.)
                        }
                    });
                    if !possible {
                        continue;
                    }
                    let mut outcomes = outcomes.clone();
                    outcomes.push(o as u8);
                    next.push((outcomes, added, mass * g.prob[o]));
                }
            }
            states = next;
        }
        let count = nodes;
        if std::env::var("RUST_ODDS_PROPAGATED_ORDER").as_deref() != Ok("fixture") {
            let difficulty = |g: &RankGame| {
                let mass = |t| candidates.iter().find(|v| v.0 == t).map_or(1., |v| v.1);
                mass(g.home) * mass(g.away)
            };
            remaining.sort_by(|a, b| {
                difficulty(a)
                    .total_cmp(&difficulty(b))
                    .then(a.index.cmp(&b.index))
            });
        }
        let mut mass = 0.;
        let mut patterns = Vec::new();
        for (outcomes, added, probability) in states {
            let weight = tables
                .iter()
                .zip(&added)
                .fold(probability, |p, (t, &a)| p * t.mass(a));
            if weight > 0. {
                mass += weight;
                patterns.push(JointPattern {
                    outcomes,
                    added,
                    cumulative: mass,
                });
            }
        }
        Some((
            Self {
                games: fixed,
                internal,
                tables,
                patterns,
                mass,
                selected,
                states: count,
                residual_hint,
            },
            remaining,
        ))
    }
    fn sample(&self, rng: &mut Rng, out: &mut [u8]) {
        let u = rng.float() * self.mass;
        let p = &self.patterns[self
            .patterns
            .partition_point(|p| p.cumulative < u)
            .min(self.patterns.len() - 1)];
        for ((g, _, _), &o) in self.internal.iter().zip(&p.outcomes) {
            out[g.index] = o;
        }
        for (table, &added) in self.tables.iter().zip(&p.added) {
            table.sample(added, rng, out);
        }
    }
}

pub(crate) struct Guide {
    pub(crate) games: Vec<RankGame>,
    cdf: Suffix,
    reverse: Suffix,
    pub(crate) min: Vec<Vec<i32>>,
    pub(crate) max: Vec<Vec<i32>>,
    origin: Vec<Vec<i32>>,
    chance_rows: [OnceLock<Vec<Arc<[f64]>>>; 2],
}
impl Guide {
    pub(crate) fn new(games: Vec<RankGame>, teams: usize) -> Self {
        let mut min = vec![vec![0; teams]; games.len() + 1];
        let mut max = min.clone();
        let mut reversed = games.clone();
        for i in (0..games.len()).rev() {
            min[i] = min[i + 1].clone();
            max[i] = max[i + 1].clone();
            let g = &games[i];
            for (t, gains) in [(g.home, g.hg), (g.away, g.ag)] {
                let lo = (0..3)
                    .filter(|&o| g.prob[o] > 0.)
                    .map(|o| gains[o])
                    .min()
                    .unwrap();
                let hi = (0..3)
                    .filter(|&o| g.prob[o] > 0.)
                    .map(|o| gains[o])
                    .max()
                    .unwrap();
                min[i][t] += lo;
                max[i][t] += hi;
                // Zero-mass options can have gains above hi. Keep arithmetic
                // nonnegative even though those options do not contribute.
                let top = *gains.iter().max().unwrap();
                if t == g.home {
                    reversed[i].hg = gains.map(|v| top - v);
                } else {
                    reversed[i].ag = gains.map(|v| top - v);
                }
            }
        }
        let span = max[0].iter().copied().max().unwrap_or(0);
        // Reverse gains use the unconditional per-fixture maximum. Store that
        // complement's origin separately from the actual attainable maximum.
        let mut origin = vec![vec![0; teams]; games.len() + 1];
        for i in (0..games.len()).rev() {
            origin[i] = origin[i + 1].clone();
            origin[i][games[i].home] += *games[i].hg.iter().max().unwrap();
            origin[i][games[i].away] += *games[i].ag.iter().max().unwrap();
        }
        let reverse_span = origin[0].iter().copied().max().unwrap_or(0);
        let cdf = Suffix::new(&games, teams, span);
        let reverse = Suffix::new(&reversed, teams, reverse_span);
        Self {
            games,
            cdf,
            reverse,
            min,
            max,
            origin,
            chance_rows: std::array::from_fn(|_| OnceLock::new()),
        }
    }
    fn prepare_chances(&self, below: bool) -> &[Arc<[f64]>] {
        self.chance_rows[usize::from(below)].get_or_init(|| {
            let terminal: Arc<[f64]> = vec![0.5].into();
            let teams = self.cdf.teams;
            let mut rows = vec![terminal; (self.games.len() + 1) * teams];
            for step in (0..self.games.len()).rev() {
                let split = (step + 1) * teams;
                let (first, last) = rows.split_at_mut(split);
                first[step * teams..split].clone_from_slice(&last[..teams]);
                let g = &self.games[step];
                for t in [g.home, g.away] {
                    let span = (self.max[step][t] as usize + 1).min(self.cdf.span);
                    first[step * teams + t] = (0..span)
                        .map(|cap| self.chance(step, t, cap as i32, below))
                        .collect::<Vec<_>>()
                        .into();
                }
            }
            rows
        })
    }
    #[inline]
    fn cached_chance(
        &self,
        rows: &[Arc<[f64]>],
        step: usize,
        t: usize,
        cap: i32,
        below: bool,
    ) -> f64 {
        if cap >= 0 {
            if let Some(&value) = rows[step * self.cdf.teams + t].get(cap as usize) {
                return value;
            }
        }
        self.chance(step, t, cap, below)
    }
    #[inline]
    pub(crate) fn chance(&self, step: usize, t: usize, cap: i32, below: bool) -> f64 {
        if cap < 0 {
            // Both forward CDF terms are zero, so the original tie term is
            // exactly zero. Preserve the reverse tail's rounding residue.
            return if below {
                0.
            } else {
                self.reverse.cdf(step, t, self.origin[step][t] - cap - 1) + 0.
            };
        }
        let low_row = &self.cdf.values[step * self.cdf.teams + t];
        let high_row = &self.reverse.values[step * self.reverse.teams + t];
        let origin = self.origin[step][t];
        let low = Suffix::cdf_row(low_row, cap - 1);
        let high = Suffix::cdf_row(high_row, origin - cap - 1);
        let tie = if low <= 0.5 {
            Suffix::cdf_row(low_row, cap) - low
        } else {
            Suffix::cdf_row(high_row, origin - cap) - high
        };
        (if below { low } else { high }) + tie.max(0.) * 0.5
    }
    fn factor(&self, step: usize, team: usize, cap: i32, below: f64, above: f64) -> f64 {
        let origin = self.origin[step][team];
        // When guidance weights one side and ties equally, the whole factor
        // is one inclusive tail. Avoid three separate DP lookups and retain
        // tiny upper tails without CDF subtraction.
        if below == 0. && above == 1. {
            return self.reverse.cdf(step, team, origin - cap);
        }
        if below == 1. && above == 0. {
            return self.cdf.cdf(step, team, cap);
        }
        let low = self.cdf.cdf(step, team, cap - 1);
        // Evaluate a rare upper tail directly, never as 1 - rounded CDF.
        let high = self.reverse.cdf(step, team, origin - cap - 1);
        let tie = if low <= 0.5 {
            self.cdf.cdf(step, team, cap) - low
        } else {
            self.reverse.cdf(step, team, origin - cap) - high
        };
        below * low + tie.max(0.) + above * high
    }
}
/// Exact shortcuts for deterministic Bernoulli terms in the rank DP.
/// DP entries and clamped chances are finite and nonnegative, so multiplying
/// by zero/one adds no rounding and consumes no randomness.
#[inline]
fn update_cardinality(values: &mut [f64], p: f64, support: &mut (usize, usize)) {
    if p == 0. || support.0 >= values.len() {
        return;
    }
    let (lo, hi) = *support;
    let next_hi = (hi + 1).min(values.len() - 1);
    if p == 1. {
        for j in (lo + 1..=next_hi).rev() {
            values[j] = values[j - 1];
        }
        values[lo] = 0.;
        *support = (lo + 1, next_hi);
        return;
    }
    for j in (lo..=next_hi).rev() {
        values[j] = values[j] * (1. - p) + if j > 0 { values[j - 1] * p } else { 0. };
    }
    support.1 = next_hi;
}

#[derive(Clone)]
struct Pattern {
    fixed: Vec<(usize, u8)>,
    bias: Option<Arc<Vec<[f64; 3]>>>,
    omitted: Vec<RankGame>,
    base: Vec<i32>,
    lower: Vec<i32>,
    upper: Vec<i32>,
    guide: Arc<Guide>,
    joint: Arc<NecessaryJoint>,
    mass: f64,
    tilt: f64,
    cumulative: f64,
}
impl Pattern {
    fn draw(
        &self,
        cell: Cell,
        rng: &mut Rng,
        out: &mut [u8],
        points: &mut [i32],
        guided: bool,
        cardinality: bool,
        replay: bool,
        dynamic: usize,
    ) -> f64 {
        for &(i, o) in &self.fixed {
            if replay && out[i] != o {
                return 0.;
            }
            out[i] = o;
        }
        for g in &self.omitted {
            if replay {
                if g.prob[out[g.index] as usize] <= 0. {
                    return 0.;
                }
            } else {
                out[g.index] = choose(g.prob, g.prob.iter().sum(), rng) as u8;
            }
        }
        points.copy_from_slice(&self.base);
        if !replay {
            self.joint.sample(rng, out);
        }
        for g in &self.joint.games {
            let o = out[g.index] as usize;
            if g.prob[o] <= 0. {
                return 0.;
            }
            points[g.home] += g.hg[o];
            points[g.away] += g.ag[o];
        }
        if self
            .joint
            .selected
            .iter()
            .any(|&t| points[t] < self.lower[t] || points[t] > self.upper[t])
        {
            return 0.;
        }
        let target = points[cell.team];
        let Some(mut counts) = self.counts(0, points, cell) else {
            return 0.;
        };
        let (above, below) = if guided && !cardinality {
            let mut ea = 0.;
            let mut eb = 0.;
            for t in 0..points.len() {
                if t != cell.team {
                    ea += self.guide.factor(0, t, target - points[t], 0., 1.);
                    eb += self.guide.factor(0, t, target - points[t], 1., 0.);
                }
            }
            let above = if ea > 0. {
                (cell.rank as f64 / ea).min(1.).powi(6)
            } else {
                1.
            };
            let below = if eb > 0. {
                ((points.len() - 1 - cell.rank) as f64 / eb).min(1.).powi(6)
            } else {
                1.
            };
            (above, below)
        } else {
            (1., 1.)
        };
        let mut weight = 1.;
        let count_below = cell.rank > (points.len() - 1) / 2;
        let needed = if count_below {
            points.len() - 1 - cell.rank
        } else {
            cell.rank
        };
        let chance_rows = if cardinality {
            self.guide.prepare_chances(count_below)
        } else {
            &[]
        };
        let chance = |step: usize, t: usize, value: i32| {
            self.guide
                .cached_chance(chance_rows, step, t, target - value, count_below)
        };
        let mut chances = if cardinality {
            (0..points.len())
                .map(|t| chance(0, t, points[t]).clamp(0., 1.))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mut other = vec![0.; if cardinality { needed + 1 } else { 0 }];
        let rivals: Vec<_> = if dynamic > 0 {
            (0..points.len()).filter(|&t| t != cell.team).collect()
        } else {
            Vec::new()
        };
        let mut masks = vec![
            7u8;
            if dynamic > 0 {
                self.guide.games.len()
            } else {
                0
            }
        ];
        for (step, g) in self.guide.games.iter().enumerate() {
            if dynamic > 0 && step % dynamic == 0 {
                let games: Vec<_> = self.guide.games[step..]
                    .iter()
                    .enumerate()
                    .map(|(i, &g)| {
                        let mut g = g;
                        for o in 0..3 {
                            if masks[step + i] & (1 << o) == 0 {
                                g.prob[o] = 0.;
                            }
                        }
                        g
                    })
                    .collect();
                let d = Domains::propagate(&games, points, &rivals, cell.rank, target);
                if !d.feasible {
                    return 0.;
                }
                for (i, &mask) in d.domains.iter().enumerate() {
                    masks[step + i] &= mask;
                }
            }
            if cardinality {
                other.fill(0.);
                other[0] = 1.;
                let mut support = (0, 0);
                for (t, &p) in chances.iter().enumerate() {
                    if t == cell.team || t == g.home || t == g.away {
                        continue;
                    }
                    update_cardinality(&mut other, p, &mut support);
                }
            }
            let mut q = [0.; 3];
            let mut support = 0.;
            let mut endpoint_chances = [[0.; 2]; 3];
            let next: [Option<(usize, usize)>; 3] =
                std::array::from_fn(|o| self.next_counts(step, points, cell, o, counts));
            let allowed: [bool; 3] = std::array::from_fn(|o| {
                g.prob[o] > 0. && next[o].is_some() && (dynamic == 0 || masks[step] & (1 << o) != 0)
            });
            for o in 0..3 {
                if !allowed[o] {
                    continue;
                }
                support += g.prob[o];
                q[o] = g.prob[o]
                    * if cardinality {
                        let ph = chance(step + 1, g.home, points[g.home] + g.hg[o]).clamp(0., 1.);
                        let pa = chance(step + 1, g.away, points[g.away] + g.ag[o]).clamp(0., 1.);
                        endpoint_chances[o] = [ph, pa];
                        other[needed] * (1. - ph) * (1. - pa)
                            + if needed > 0 {
                                other[needed - 1] * (ph * (1. - pa) + (1. - ph) * pa)
                            } else {
                                0.
                            }
                            + if needed > 1 {
                                other[needed - 2] * ph * pa
                            } else {
                                0.
                            }
                    } else if guided {
                        self.guide.factor(
                            step + 1,
                            g.home,
                            target - points[g.home] - g.hg[o],
                            below,
                            above,
                        ) * self.guide.factor(
                            step + 1,
                            g.away,
                            target - points[g.away] - g.ag[o],
                            below,
                            above,
                        )
                    } else {
                        1.
                    };
            }
            if let Some(bias) = &self.bias {
                for o in 0..3 {
                    q[o] *= bias[g.index][o];
                }
            }
            let guide_mass = q.iter().sum::<f64>();
            for o in 0..3 {
                if !allowed[o] {
                    continue;
                }
                q[o] = if guide_mass > 0. {
                    (1. - BASE_SHARE) * q[o] + BASE_SHARE * guide_mass * g.prob[o] / support
                } else {
                    g.prob[o]
                };
            }
            let total = q.iter().sum();
            if total <= 0. {
                weight = 0.;
                break;
            }
            let o = if replay {
                out[g.index] as usize
            } else {
                choose(q, total, rng)
            };
            if !allowed[o] || q[o] <= 0. {
                return 0.;
            }
            counts = next[o].unwrap();
            weight *= g.prob[o] * total / q[o];
            out[g.index] = o as u8;
            points[g.home] += g.hg[o];
            points[g.away] += g.ag[o];
            if cardinality {
                chances[g.home] = endpoint_chances[o][0];
                chances[g.away] = endpoint_chances[o][1];
            }
        }
        weight
    }

    fn counts(&self, step: usize, points: &[i32], cell: Cell) -> Option<(usize, usize)> {
        let target = points[cell.team];
        let mut above = 0;
        let mut below = 0;
        for (t, &value) in points.iter().enumerate() {
            if t != cell.team {
                let lo = value + self.guide.min[step][t];
                let hi = value + self.guide.max[step][t];
                if lo > self.upper[t] || hi < self.lower[t] {
                    return None;
                }
                above += usize::from(lo > target);
                below += usize::from(hi < target);
            }
        }
        (above <= cell.rank && below <= points.len() - 1 - cell.rank).then_some((above, below))
    }
    fn next_counts(
        &self,
        step: usize,
        points: &[i32],
        cell: Cell,
        o: usize,
        counts: (usize, usize),
    ) -> Option<(usize, usize)> {
        let g = &self.guide.games[step];
        let target = points[cell.team];
        let (mut above, mut below) = counts;
        for (t, gain) in [(g.home, g.hg[o]), (g.away, g.ag[o])] {
            above -= usize::from(points[t] + self.guide.min[step][t] > target);
            below -= usize::from(points[t] + self.guide.max[step][t] < target);
            let lo = points[t] + gain + self.guide.min[step + 1][t];
            let hi = points[t] + gain + self.guide.max[step + 1][t];
            if lo > self.upper[t] || hi < self.lower[t] {
                return None;
            }
            above += usize::from(lo > target);
            below += usize::from(hi < target);
        }
        (above <= cell.rank && below <= points.len() - 1 - cell.rank).then_some((above, below))
    }
    #[cfg(test)]
    fn allows(&self, step: usize, points: &[i32], cell: Cell, outcome: usize) -> bool {
        let g = &self.guide.games[step];
        let target = points[cell.team];
        let mut above = 0;
        let mut below = 0;
        for (t, &value) in points.iter().enumerate() {
            if t == cell.team {
                continue;
            }
            let value = value
                + if t == g.home {
                    g.hg[outcome]
                } else if t == g.away {
                    g.ag[outcome]
                } else {
                    0
                };
            let lo = value + self.guide.min[step + 1][t];
            let hi = value + self.guide.max[step + 1][t];
            if lo > self.upper[t] || hi < self.lower[t] {
                return false;
            }
            above += usize::from(lo > target);
            below += usize::from(hi < target);
        }
        above <= cell.rank && below <= points.len() - 1 - cell.rank
    }
}
pub struct PropagatedJoint {
    cell: Cell,
    patterns: Vec<Pattern>,
    mass: f64,
    tilted_mass: f64,
    target_patterns: usize,
    guides: usize,
}
impl PropagatedJoint {
    pub fn new(m: &Model, cell: Cell) -> Option<Self> {
        Self::with_limits(m, cell, 4, 6)
    }
    fn with_limits(
        m: &Model,
        cell: Cell,
        minimum_forced: usize,
        rival_limit: usize,
    ) -> Option<Self> {
        Self::configured(
            m,
            cell,
            Limits {
                minimum_forced,
                rivals: rival_limit,
                target_patterns: PATTERN_LIMIT,
                feasible_cases: if minimum_forced > 0 { 16 } else { usize::MAX },
            },
        )
        .ok()
    }
    pub fn configured(
        m: &Model,
        cell: Cell,
        limits: Limits,
    ) -> std::result::Result<Self, &'static str> {
        let minimum_forced = limits.minimum_forced;
        let rival_limit = limits.rivals;
        let pattern_limit = limits.target_patterns;
        let rules = &m.request.phase.championship;
        if m.keys.first() != Some(&Key::Pt)
            || m.request.phase.bonus_points != 0
            || (rules.point_win, rules.point_draw, rules.point_loss) != (3, 1, 0)
            || m.ids.len() != m.n
            || cell.team >= m.n
            || cell.rank >= m.n
            || m.fixtures.iter().any(|g| g.home == g.away)
        {
            return Err("unsupported_rules");
        }
        let mut left = vec![0; m.n];
        for g in &m.fixtures {
            left[g.home] += 1;
            left[g.away] += 1;
        }
        let wins = m.keys.get(1) == Some(&Key::W);
        let stride64 = if wins {
            m.base
                .iter()
                .zip(&left)
                .map(|(c, n)| i64::from(c.wins) + i64::from(*n))
                .max()
                .ok_or("empty_model")?
                + 1
        } else {
            1
        };
        if !(1..=4096).contains(&stride64) {
            return Err("packed_stride_limit");
        }
        let stride = stride64 as i32;
        if m.fixtures.len() > 256
            || left[cell.team] == 0
            || left[cell.team] > 32
            || left
                .iter()
                .any(|&n| (3 * stride64 + i64::from(wins)) * i64::from(n) > 4096)
            || m.base.iter().any(|c| {
                (i64::from(c.points).abs() + 3 * m.fixtures.len() as i64 + 1) * stride64
                    + i64::from(c.wins)
                    >= i64::from(i32::MAX / 4)
            })
        {
            return Err("fixture_or_gain_limit");
        }
        let base = m
            .base
            .iter()
            .map(|c| c.points * stride + if wins { c.wins } else { 0 })
            .collect::<Vec<_>>();
        let hg = [0, stride, 3 * stride + i32::from(wins)];
        let all = m
            .fixtures
            .iter()
            .enumerate()
            .map(|(index, g)| RankGame {
                index,
                home: g.home,
                away: g.away,
                prob: g.prob,
                hg,
                ag: [hg[2], hg[1], hg[0]],
            })
            .collect::<Vec<_>>();
        let (target_games, remaining): (Vec<_>, Vec<_>) = all
            .into_iter()
            .partition(|g| g.home == cell.team || g.away == cell.team);
        let span = (hg[2] * left[cell.team]) as usize + 1;
        let terminal = (0..span)
            .map(|gain| {
                let total = base[cell.team] + gain as i32;
                let above = (0..m.n)
                    .filter(|&t| t != cell.team && base[t] > total)
                    .count();
                let below = (0..m.n)
                    .filter(|&t| t != cell.team && base[t] + hg[2] * left[t] < total)
                    .count();
                if above <= cell.rank && below <= m.n - 1 - cell.rank {
                    1.
                } else {
                    0.
                }
            })
            .collect::<Vec<_>>();
        let table = TerminalTable::new(target_games, cell.team, terminal);
        // Count every supported target assignment, with saturation. The model
        // is used only if the entire union fits; no prefix is silently retained.
        let mut counts = vec![vec![0usize; span]; table.games.len() + 1];
        for (i, &p) in table
            .rows
            .last()
            .ok_or("empty_target_table")?
            .iter()
            .enumerate()
        {
            counts[table.games.len()][i] = usize::from(p > 0.);
        }
        for i in (0..table.games.len()).rev() {
            let g = &table.games[i];
            let gains = if g.home == cell.team { g.hg } else { g.ag };
            for added in 0..span {
                for o in 0..3 {
                    if g.prob[o] > 0. {
                        counts[i][added] = (counts[i][added]
                            + counts[i + 1]
                                .get(added + gains[o] as usize)
                                .copied()
                                .unwrap_or(0))
                        .min(pattern_limit + 1);
                    }
                }
            }
        }
        let target_patterns = counts[0][0];
        if target_patterns == 0 {
            return Err("empty_target_support");
        }
        if target_patterns > pattern_limit {
            return Err("target_pattern_limit");
        }
        let mut assignments = vec![(0usize, Vec::new(), 1., base.clone())];
        for (step, g) in table.games.iter().enumerate() {
            let mut next = Vec::new();
            let gain = if g.home == cell.team { g.hg } else { g.ag };
            for (added, fixed, mass, points) in assignments {
                for o in 0..3 {
                    let to = added + gain[o] as usize;
                    if g.prob[o] <= 0. || counts[step + 1].get(to).copied().unwrap_or(0) == 0 {
                        continue;
                    }
                    let mut fixed = fixed.clone();
                    fixed.push((g.index, o as u8));
                    let mut points = points.clone();
                    points[g.home] += g.hg[o];
                    points[g.away] += g.ag[o];
                    next.push((to, fixed, mass * g.prob[o], points));
                }
            }
            assignments = next;
        }
        let rivals = (0..m.n).filter(|&t| t != cell.team).collect::<Vec<_>>();
        let mut patterns = Vec::new();
        let mut cache: HashMap<(Vec<u8>, Vec<(i32, i32)>), (Arc<NecessaryJoint>, Arc<Guide>)> =
            HashMap::new();
        let mut setup_nodes = 0usize;
        let mut guide_values = 0usize;
        let mut mass = 0.;
        for (_, mut fixed, mut weight, points) in assignments {
            let d = Domains::propagate(&remaining, &points, &rivals, cell.rank, points[cell.team]);
            if !d.feasible {
                continue;
            }
            let mut variables = Vec::new();
            for (g, &mask) in remaining.iter().zip(&d.domains) {
                if mask.count_ones() == 1 {
                    let o = mask.trailing_zeros() as usize;
                    fixed.push((g.index, o as u8));
                    weight *= g.prob[o];
                } else {
                    let total: f64 = (0..3)
                        .filter(|o| mask & (1 << o) != 0)
                        .map(|o| g.prob[o])
                        .sum();
                    if total <= 0. {
                        return Err("zero_mask_mass");
                    }
                    weight *= total;
                    let mut g = *g;
                    g.prob = std::array::from_fn(|o| {
                        if mask & (1 << o) != 0 {
                            g.prob[o] / total
                        } else {
                            0.
                        }
                    });
                    variables.push(g);
                }
            }
            if fixed.len() < table.games.len() + minimum_forced {
                return Err("minimum_forced_fixtures");
            }
            let propagated_base = if d.base.is_empty() { points } else { d.base };
            let constraint_key = (0..m.n)
                .map(|t| {
                    let span: i32 = variables
                        .iter()
                        .filter(|g| g.home == t || g.away == t)
                        .map(|g| {
                            *if g.home == t { &g.hg } else { &g.ag }
                                .iter()
                                .max()
                                .unwrap()
                        })
                        .sum();
                    (
                        (d.lower[t] - propagated_base[t]).clamp(0, span + 1),
                        (d.upper[t] - propagated_base[t]).clamp(-1, span),
                    )
                })
                .collect::<Vec<_>>();
            if patterns.len() >= limits.feasible_cases {
                return Err("feasible_case_limit");
            }
            let key = (d.domains.clone(), constraint_key);
            let (joint, guide) = if let Some(pair) = cache.get(&key) {
                pair.clone()
            } else {
                let (joint, variables) = NecessaryJoint::new(
                    &variables,
                    &propagated_base,
                    &d.lower,
                    &d.upper,
                    rival_limit,
                    60000 - setup_nodes,
                )
                .ok_or("joint_node_limit")?;
                setup_nodes += joint.states;
                let span = (0..m.n)
                    .map(|t| {
                        variables
                            .iter()
                            .filter(|g| g.home == t || g.away == t)
                            .map(|g| {
                                *if g.home == t { &g.hg } else { &g.ag }
                                    .iter()
                                    .max()
                                    .unwrap()
                            })
                            .sum::<i32>()
                    })
                    .max()
                    .unwrap_or(0) as usize
                    + 1;
                // Suffix rows share unchanged teams through Arc. Two
                // endpoint rows per fixture, in both gain directions.
                guide_values += (2 * variables.len() + 1) * span * 2;
                if guide_values > 4000000 {
                    return Err("guide_memory_limit");
                }
                let pair = (Arc::new(joint), Arc::new(Guide::new(variables, m.n)));
                cache.insert(key, pair.clone());
                pair
            };
            if joint.mass <= 0. {
                continue;
            }
            weight *= joint.mass;
            mass += weight;
            patterns.push(Pattern {
                bias: None,
                omitted: Vec::new(),
                fixed,
                base: propagated_base,
                lower: d.lower,
                upper: d.upper,
                guide,
                joint,
                mass: weight,
                tilt: 1.,
                cumulative: mass,
            });
        }
        if mass <= 0.
            || !mass.is_finite()
            || patterns.is_empty()
            || patterns
                .iter()
                .any(|p| p.fixed.len() < table.games.len() + minimum_forced)
        {
            return Err("empty_conditioning_mass");
        }
        let max_hint = patterns
            .iter()
            .map(|p| p.joint.residual_hint)
            .fold(0., f64::max);
        let mut tilted_mass = 0.;
        for p in &mut patterns {
            p.tilt = if max_hint > 0. {
                (p.joint.residual_hint / max_hint).max(1e-6)
            } else {
                1.
            };
            tilted_mass += p.mass * p.tilt;
            p.cumulative = tilted_mass;
        }
        Ok(Self {
            tilted_mass,
            guides: cache.len(),
            cell,
            patterns,
            mass,
            target_patterns,
        })
    }
    pub fn describe(&self, m: &Model) -> serde_json::Value {
        json!({"mode":"propagated_target_union","team":m.ids[self.cell.team],"rank":self.cell.rank+1,
            "target_patterns":self.target_patterns,"feasible_patterns":self.patterns.len(),"guides":self.guides,
            "forced_min":self.patterns.iter().map(|p|p.fixed.len()).min(),"forced_max":self.patterns.iter().map(|p|p.fixed.len()).max(),
            "conditioning_mass":self.mass,"variable_max":self.patterns.iter().map(|p|p.guide.games.len()).max(),
            "joint_states":self.patterns.iter().map(|p|p.joint.states).sum::<usize>(),
            "case_tilts":self.patterns.iter().map(|p|json!({"total":p.base[self.cell.team],"mass":p.mass,"tilt":p.tilt,"hint":p.joint.residual_hint})).collect::<Vec<_>>(),
            "joint_teams":self.patterns.iter().map(|p|p.joint.selected.iter().map(|&t|m.ids[t]).collect::<Vec<_>>()).collect::<Vec<_>>()})
    }
    pub fn sample(&self, m: &Model, samples: usize, seed: i64, guided: bool) -> Result {
        let mut result = Result {
            mass: self.mass,
            ..Default::default()
        };
        let mut rng = Rng::new(seed);
        let mut scores = ScoreContext::new(m);
        let mut out = vec![0; m.fixtures.len()];
        let mut points = vec![0; m.n];
        let (mut sum, mut sum2, mut largest, mut batches) = (0., 0., 0_f64, [0.; 2]);
        for draw in 0..samples {
            let u = rng.float() * self.tilted_mass;
            let p = &self.patterns[self
                .patterns
                .partition_point(|p| p.cumulative < u)
                .min(self.patterns.len() - 1)];
            let weight = self.tilted_mass / self.mass / p.tilt
                * p.draw(
                    self.cell,
                    &mut rng,
                    &mut out,
                    &mut points,
                    guided,
                    false,
                    false,
                    0,
                );
            result.samples += 1;
            if weight <= 0. || scores.rank(self.cell.team, &out, &mut rng, None) != self.cell.rank {
                continue;
            }
            result.hits += 1;
            if result.witness.is_none() {
                result.witness = Some(out.clone());
            }
            sum += weight;
            sum2 += weight * weight;
            largest = largest.max(weight);
            batches[usize::from(draw >= samples / 2)] += weight;
        }
        result.work = result.samples as u64 * work_per_sample(m);
        result.summarize(sum, sum2, largest, batches);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{conditioned::canonical_ranks, model::Request};
    fn league(sort: &str) -> Model {
        let games = (0..4)
            .flat_map(|h| {
                (h + 1..4).map(move |a| {
                    json!({"id":h*4+a,"home_id":h,"away_id":a,
            "home_power":1.1+0.1*h as f64,"away_power":0.8+0.1*a as f64})
                })
            })
            .collect::<Vec<_>>();
        let request: Request = serde_json::from_value(json!({"id":1,
            "phase":{"sort":sort,"championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..4).map(|t|json!({"team_id":t,"add_sub":t,"bias":t})).collect::<Vec<_>>(),
            "games":games})).unwrap();
        Model::new(request).unwrap()
    }
    fn exact(m: &Model, p: &PropagatedJoint) -> (f64, f64) {
        let stride = if m.keys.get(1) == Some(&Key::W) { 4 } else { 1 };
        let mut event = 0.;
        let mut rank = 0.;
        for mut code in 0..3usize.pow(m.fixtures.len() as u32) {
            let mut out = vec![0; m.fixtures.len()];
            let mut mass = 1.;
            let mut points = m
                .base
                .iter()
                .map(|c| c.points * stride + if stride > 1 { c.wins } else { 0 })
                .collect::<Vec<_>>();
            for (i, g) in m.fixtures.iter().enumerate() {
                let o = code % 3;
                code /= 3;
                out[i] = o as u8;
                mass *= g.prob[o];
                let gains = [0, stride, 3 * stride + i32::from(stride > 1)];
                points[g.home] += gains[o];
                points[g.away] += gains[2 - o];
            }
            let hit = canonical_ranks(m, &out)[p.cell.team] == p.cell.rank;
            if hit {
                rank += mass;
            }
            let pattern = p
                .patterns
                .iter()
                .find(|p| p.fixed.iter().all(|&(i, o)| out[i] == o));
            let Some(pattern) = pattern else {
                assert!(!hit, "lost target/forced support");
                continue;
            };
            let supported = pattern
                .joint
                .games
                .iter()
                .chain(&pattern.guide.games)
                .all(|g| g.prob[out[g.index] as usize] > 0.)
                && pattern
                    .joint
                    .selected
                    .iter()
                    .all(|&t| points[t] >= pattern.lower[t] && points[t] <= pattern.upper[t]);
            assert!(!hit || supported, "lost necessary joint support");
            if !supported {
                continue;
            }
            event += mass;
            if hit {
                let mut prefix = pattern.base.clone();
                for g in &pattern.joint.games {
                    let o = out[g.index] as usize;
                    prefix[g.home] += g.hg[o];
                    prefix[g.away] += g.ag[o];
                }
                for (step, g) in pattern.guide.games.iter().enumerate() {
                    let o = out[g.index] as usize;
                    let counts = pattern.counts(step, &prefix, p.cell).unwrap();
                    for candidate in 0..3 {
                        let mut after = prefix.clone();
                        after[g.home] += g.hg[candidate];
                        after[g.away] += g.ag[candidate];
                        assert_eq!(
                            pattern.next_counts(step, &prefix, p.cell, candidate, counts),
                            pattern.counts(step + 1, &after, p.cell),
                            "incremental rank cardinality changed support"
                        );
                    }
                    assert!(
                        pattern.allows(step, &prefix, p.cell, o),
                        "lost prefix rank support"
                    );
                    prefix[g.home] += g.hg[o];
                    prefix[g.away] += g.ag[o];
                }
            }
        }
        (event, rank)
    }
    #[test]
    fn complete_propagated_union_matches_exhaustive_mass_and_preserves_every_rank_hit() {
        for sort in ["pt,w,bias", "pt,bias"] {
            let m = league(sort);
            for team in 0..4 {
                for rank in 0..4 {
                    for limit in [0, 1, 3] {
                        let Some(p) =
                            PropagatedJoint::with_limits(&m, Cell { team, rank }, 0, limit)
                        else {
                            continue;
                        };
                        let (event, _) = exact(&m, &p);
                        assert!(
                            (p.mass - event).abs() < 1e-12,
                            "sort={sort} team={team} rank={rank} mass={} exact={event}",
                            p.mass
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn propagated_joint_weights_match_exact_ranks_with_and_without_guidance() {
        for sort in ["pt,w,bias", "pt,bias"] {
            let m = league(sort);
            for rank in 0..4 {
                let p = PropagatedJoint::with_limits(&m, Cell { team: 0, rank }, 0, 1).unwrap();
                let (_, expected) = exact(&m, &p);
                for guided in [false, true] {
                    let r = p.sample(&m, 10000, 808, guided);
                    assert!(
                        (r.probability - expected).abs() < 6. * r.std_err + 1e-12,
                        "sort={sort} rank={rank} guided={guided} estimate={} exact={expected}",
                        r.probability
                    );
                }
            }
        }
    }
    #[test]
    fn reversed_guidance_retains_tiny_upper_tail_and_real_goal_tiebreakers() {
        let g = RankGame {
            index: 0,
            home: 0,
            away: 1,
            prob: [1. - 1e-20, 0., 1e-20],
            hg: [0, 1, 3],
            ag: [3, 1, 0],
        };
        let guide = Guide::new(vec![g], 2);
        assert_eq!(guide.factor(0, 0, 2, 0., 1.), 1e-20);
        let m = league("pt,w,gd,gf,bias");
        let p = PropagatedJoint::with_limits(&m, Cell { team: 0, rank: 1 }, 0, 1).unwrap();
        let r = p.sample(&m, 20000, 808, true);
        let mut rng = Rng::new(1123);
        let mut scores = ScoreContext::new(&m);
        let mut out = vec![0; m.fixtures.len()];
        let mut hits = 0;
        for _ in 0..20000 {
            for (i, g) in m.fixtures.iter().enumerate() {
                out[i] = choose(g.prob, 1., &mut rng) as u8;
            }
            hits += usize::from(scores.rank(0, &out, &mut rng, None) == 1);
        }
        let estimate = hits as f64 / 20000.;
        let se = (estimate * (1. - estimate) / 20000.).sqrt();
        assert!((r.probability - estimate).abs() < 6. * (r.std_err * r.std_err + se * se).sqrt());
    }
    #[test]
    fn exceeding_complete_pattern_budget_skips_instead_of_truncating() {
        let mut m = league("pt,w,bias");
        let games = m
            .fixtures
            .iter()
            .enumerate()
            .map(|(index, g)| RankGame {
                index,
                home: g.home,
                away: g.away,
                prob: g.prob,
                hg: [0, 1, 3],
                ag: [3, 1, 0],
            })
            .collect::<Vec<_>>();
        let base = m.base.iter().map(|c| c.points).collect::<Vec<_>>();
        let lower = base.iter().map(|p| p + 1).collect::<Vec<_>>();
        let upper = base.iter().map(|p| p + 9).collect::<Vec<_>>();
        assert!(NecessaryJoint::new(&games, &base, &lower, &upper, 3, 0).is_none());
        let (joint, _) = NecessaryJoint::new(&games, &base, &lower, &upper, 3, 10000).unwrap();
        assert!(joint.states > 0 && joint.states <= 10000);
        m.request.games.extend(m.request.games.clone());
        m = Model::new(m.request).unwrap();
        assert!(PropagatedJoint::with_limits(&m, Cell { team: 0, rank: 1 }, 0, 1).is_none());
        m.keys = vec![Key::Gd, Key::Pt];
        assert!(PropagatedJoint::new(&m, Cell { team: 0, rank: 1 }).is_none());
    }
}

#[cfg(test)]
mod cardinality_fast_tests {
    use super::update_cardinality;
    #[test]
    fn deterministic_rank_terms_preserve_every_dp_bit() {
        let probabilities = [0., 1., 1e-30, 0.3, 1. - f64::EPSILON];
        for code in 0..5usize.pow(6) {
            for length in 1..8 {
                let mut original = vec![0.; length];
                let mut fast = original.clone();
                original[0] = 1.;
                fast[0] = 1.;
                let mut sequence = code;
                let mut support = (0, 0);
                for _ in 0..6 {
                    let p = probabilities[sequence % 5];
                    sequence /= 5;
                    for j in (0..length).rev() {
                        original[j] =
                            original[j] * (1. - p) + if j > 0 { original[j - 1] * p } else { 0. };
                    }
                    update_cardinality(&mut fast, p, &mut support);
                    assert_eq!(
                        original.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                        fast.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod chance_table_tests {
    use super::*;
    #[test]
    fn prepared_chances_preserve_bits_in_both_directions_and_outside_the_table() {
        for stride in [1, 19] {
            let hg = [0, stride, 3 * stride + i32::from(stride > 1)];
            let games = (0..12)
                .map(|i| RankGame {
                    index: i,
                    home: i % 4,
                    away: (i + 1) % 4,
                    prob: if i % 3 == 0 {
                        [0.9999999999999999, 0., 1e-20]
                    } else {
                        [0.31, 0.27, 0.42]
                    },
                    hg,
                    ag: [hg[2], hg[1], hg[0]],
                })
                .collect();
            let guide = Guide::new(games, 4);
            for below in [false, true] {
                let rows = guide.prepare_chances(below);
                for step in 0..=guide.games.len() {
                    for t in 0..4 {
                        for cap in -10..=guide.cdf.span as i32 + 10 {
                            let low = guide.cdf.cdf(step, t, cap - 1);
                            let high = guide.reverse.cdf(step, t, guide.origin[step][t] - cap - 1);
                            let tie = if low <= 0.5 {
                                guide.cdf.cdf(step, t, cap) - low
                            } else {
                                guide.reverse.cdf(step, t, guide.origin[step][t] - cap) - high
                            };
                            let original = (if below { low } else { high }) + tie.max(0.) * 0.5;
                            assert_eq!(
                                guide.cached_chance(rows, step, t, cap, below).to_bits(),
                                original.to_bits()
                            );
                            assert_eq!(
                                guide.chance(step, t, cap, below).to_bits(),
                                original.to_bits()
                            );
                        }
                    }
                }
            }
        }
    }
}
