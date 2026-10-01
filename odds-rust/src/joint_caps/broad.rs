//! A defensive mixture of joint rival conditioning and target-only conditioning.
//! Target conditioning removes only totals excluded by safe independent bounds.
//! Rival constraints are proposal choices, never reachability proofs.
use super::choose;
use crate::{
    conditioned::{Result, ScoreContext},
    lookahead::{RankGame, Suffix},
    model::{Key, Model},
    pool::work_per_sample,
    rng::Rng,
    search::Cell,
};
use serde_json::json;

fn indicator(on: bool) -> f64 {
    if on {
        1.
    } else {
        0.
    }
}

const BASE_SHARE: f64 = 0.1;
const RESIDUAL_BASE_SHARE: f64 = 1e-6;
const STATE_LIMIT: usize = 20000;
const SPAN_LIMIT: usize = 4096;

/// Backward DP for an arbitrary terminal mask, indexed by points/wins already
/// accumulated. It supports unions of totals, upper caps and lower floors.
pub(super) struct TerminalTable {
    pub(super) games: Vec<RankGame>,
    team: usize,
    pub(super) rows: Vec<Vec<f64>>,
}
impl TerminalTable {
    pub(super) fn new(games: Vec<RankGame>, team: usize, terminal: Vec<f64>) -> Self {
        let span = terminal.len();
        let mut rows = vec![vec![0.; span]; games.len() + 1];
        rows[games.len()] = terminal;
        for i in (0..games.len()).rev() {
            let g = &games[i];
            let gain = if g.home == team { g.hg } else { g.ag };
            for added in 0..span {
                for o in 0..3 {
                    let next = added + gain[o] as usize;
                    if next < span {
                        rows[i][added] += g.prob[o] * rows[i + 1][next];
                    }
                }
            }
        }
        Self { games, team, rows }
    }
    pub(super) fn mass(&self, added: usize) -> f64 {
        self.rows[0].get(added).copied().unwrap_or(0.)
    }
    pub(super) fn sample(&self, mut added: usize, rng: &mut Rng, out: &mut [u8]) {
        for (i, g) in self.games.iter().enumerate() {
            let gain = if g.home == self.team { g.hg } else { g.ag };
            let weights = std::array::from_fn(|o| {
                g.prob[o]
                    * self.rows[i + 1]
                        .get(added + gain[o] as usize)
                        .copied()
                        .unwrap_or(0.)
            });
            let total = weights.iter().sum();
            let o = choose(weights, total, rng);
            out[g.index] = o as u8;
            added += gain[o] as usize;
        }
    }
}
struct Pattern {
    outcomes: Vec<u8>,
    added: Vec<usize>,
    cumulative: f64,
}
pub struct BroadJoint {
    cell: Cell,
    stride: i32,
    anchor: i32,
    base: Vec<i32>,
    selected: Vec<usize>,
    ahead: Vec<bool>,
    terminal: Vec<Vec<f64>>,
    target_table: TerminalTable,
    tables: Vec<TerminalTable>,
    internal: Vec<(RankGame, usize, usize)>,
    selected_games: Vec<RankGame>,
    remaining: Vec<RankGame>,
    suffix: Suffix,
    patterns: Vec<Pattern>,
    target_mass: f64,
    joint_mass: f64,
    states: usize,
    target_totals: usize,
}
impl BroadJoint {
    pub fn new(m: &Model, cell: Cell) -> Option<Self> {
        Self::with_rivals(m, cell, 5)
    }
    fn with_rivals(m: &Model, cell: Cell, limit: usize) -> Option<Self> {
        let rules = &m.request.phase.championship;
        if m.keys.first() != Some(&Key::Pt)
            || m.request.phase.bonus_points != 0
            || (rules.point_win, rules.point_draw, rules.point_loss) != (3, 1, 0)
            || m.ids.len() != m.n
            || cell.team >= m.n
            || cell.rank >= m.n
            || m.fixtures.iter().any(|g| g.home == g.away)
        {
            return None;
        }
        let wins = m.keys.get(1) == Some(&Key::W);
        let mut left = vec![0; m.n];
        for g in &m.fixtures {
            left[g.home] += 1;
            left[g.away] += 1;
        }
        if left[cell.team] == 0 {
            return None;
        }
        let max_wins = m
            .base
            .iter()
            .zip(&left)
            .map(|(c, n)| i64::from(c.wins) + i64::from(*n))
            .max()?;
        let stride64 = if wins { max_wins + 1 } else { 1 };
        if stride64 <= 0 || stride64 > i64::from(i32::MAX / 4) {
            return None;
        }
        let stride = stride64 as i32;
        let base = m
            .base
            .iter()
            .map(|c| i64::from(c.points) * stride64 + if wins { i64::from(c.wins) } else { 0 })
            .collect::<Vec<_>>();
        let maximum = (3 * stride64 + i64::from(wins)) * i64::from(*left.iter().max()?);
        if maximum > SPAN_LIMIT as i64
            || base
                .iter()
                .any(|v| v.abs() + maximum >= i64::from(i32::MAX / 4))
        {
            return None;
        }
        let base = base.into_iter().map(|v| v as i32).collect::<Vec<_>>();
        let hg = [0, stride, 3 * stride + i32::from(wins)];
        let ag = [hg[2], hg[1], hg[0]];
        let all = m
            .fixtures
            .iter()
            .enumerate()
            .map(|(i, g)| RankGame {
                index: i,
                home: g.home,
                away: g.away,
                prob: g.prob,
                hg,
                ag,
            })
            .collect::<Vec<_>>();
        let spans = left
            .iter()
            .map(|n| (hg[2] * n) as usize + 1)
            .collect::<Vec<_>>();
        let max = (0..m.n)
            .map(|t| base[t] + spans[t] as i32 - 1)
            .collect::<Vec<_>>();
        let target = cell.team;
        // These independent bounds are necessary, including packed point/win
        // ties. No extreme target result or fixed ahead set is required.
        let target_terminal = (0..spans[target])
            .map(|added| {
                let total = base[target] + added as i32;
                let above = (0..m.n).filter(|&t| t != target && base[t] > total).count();
                let below = (0..m.n).filter(|&t| t != target && max[t] < total).count();
                indicator(above <= cell.rank && below <= m.n - 1 - cell.rank)
            })
            .collect::<Vec<_>>();
        let target_games = all
            .iter()
            .copied()
            .filter(|g| g.home == target || g.away == target)
            .collect();
        let target_table = TerminalTable::new(target_games, target, target_terminal.clone());
        let target_mass = target_table.mass(0);
        if !target_mass.is_finite() || target_mass <= 0. {
            return None;
        }
        // Independent point/win distributions select an anchor only. The final
        // estimate always uses exact joint normalization and mixture weights.
        let pmfs = (0..m.n)
            .map(|t| {
                let mut pmf = vec![0.; spans[t]];
                pmf[0] = 1.;
                for g in all.iter().filter(|g| g.home == t || g.away == t) {
                    let gain = if g.home == t { g.hg } else { g.ag };
                    let mut next = vec![0.; pmf.len()];
                    for (p, &mass) in pmf.iter().enumerate() {
                        if mass > 0. {
                            for o in 0..3 {
                                let to = p + gain[o] as usize;
                                if to < next.len() {
                                    next[to] += mass * g.prob[o];
                                }
                            }
                        }
                    }
                    pmf = next;
                }
                pmf
            })
            .collect::<Vec<_>>();
        let cdfs = pmfs
            .iter()
            .map(|pmf| {
                let mut sum = 0.;
                pmf.iter()
                    .map(|&p| {
                        sum += p;
                        sum
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let cdf = |t: usize, cap: i32| -> f64 {
            if cap < 0 {
                0.
            } else {
                cdfs[t]
                    .get(cap as usize)
                    .copied()
                    .unwrap_or(1.)
                    .clamp(0., 1.)
            }
        };
        let rank_likelihood = |added: usize| {
            let total = base[target] + added as i32;
            let mut ranks = vec![0.; cell.rank + 2];
            ranks[0] = 1.;
            for t in 0..m.n {
                if t != target {
                    let less = cdf(t, total - base[t] - 1);
                    let at = cdf(t, total - base[t]);
                    let above = 1. - 0.5 * (less + at);
                    for r in (0..ranks.len()).rev() {
                        ranks[r] =
                            ranks[r] * (1. - above) + if r > 0 { ranks[r - 1] * above } else { 0. };
                    }
                }
            }
            ranks[cell.rank]
        };
        // Calculate only attainable allowed totals, once each. Packed wins
        // leave many unused DP indices; they cannot affect this proposal.
        let likelihood = (0..spans[target])
            .map(|i| {
                if target_terminal[i] > 0. && pmfs[target][i] > 0. {
                    rank_likelihood(i)
                } else {
                    0.
                }
            })
            .collect::<Vec<_>>();
        let allowed = (0..spans[target])
            .filter(|&i| target_terminal[i] > 0. && pmfs[target][i] > 0.)
            .collect::<Vec<_>>();
        let added = *allowed.iter().max_by(|&&a, &&b| {
            (pmfs[target][a] * likelihood[a])
                .total_cmp(&(pmfs[target][b] * likelihood[b]))
                .then(a.cmp(&b))
        })?;
        let target_totals = allowed.len();
        let max_likelihood = likelihood.iter().copied().fold(0., f64::max);
        if max_likelihood <= 0. {
            return None;
        }
        let tilted_target = likelihood
            .iter()
            .map(|p| p / max_likelihood)
            .collect::<Vec<_>>();
        let anchor = base[target] + added as i32;
        let mut ahead = vec![false; m.n];
        let mut marginals = Vec::new();
        for t in 0..m.n {
            if t != target {
                let below = cdf(t, anchor - base[t]);
                let above = 1. - cdf(t, anchor - base[t] - 1);
                let expected = base[t] as f64
                    + pmfs[t]
                        .iter()
                        .enumerate()
                        .map(|(i, &p)| i as f64 * p)
                        .sum::<f64>();
                marginals.push((t, below, above, expected));
                ahead[t] = base[t] > anchor;
            }
        }
        let mandatory = ahead.iter().filter(|&&v| v).count();
        if mandatory > cell.rank {
            return None;
        }
        let mut order = marginals
            .iter()
            .filter(|v| !ahead[v.0] && v.2 > 0.)
            .collect::<Vec<_>>();
        order.sort_by(|a, b| {
            b.2.total_cmp(&a.2)
                .then(b.3.total_cmp(&a.3))
                .then(m.ids[a.0].cmp(&m.ids[b.0]))
        });
        for v in order.into_iter().take(cell.rank - mandatory) {
            ahead[v.0] = true;
        }
        let mut candidates = marginals
            .into_iter()
            .filter_map(|(t, below, above, _)| {
                let mass = if ahead[t] { above } else { below };
                (mass > 0. && mass < 1. - 1e-12).then_some((t, mass))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| a.1.total_cmp(&b.1).then(m.ids[a.0].cmp(&m.ids[b.0])));
        if candidates.is_empty() {
            return None;
        }
        let mut selected = vec![target];
        selected.extend(candidates.iter().take(limit.min(5)).map(|v| v.0));
        loop {
            let internal = all
                .iter()
                .filter(|g| selected.contains(&g.home) && selected.contains(&g.away))
                .count();
            if 3usize
                .checked_pow(internal as u32)
                .is_some_and(|n| n <= STATE_LIMIT)
            {
                break;
            }
            if selected.len() <= 2 {
                return None;
            }
            selected.pop();
        }
        let terminal = selected
            .iter()
            .map(|&t| {
                if t == target {
                    tilted_target.clone()
                } else {
                    (0..spans[t])
                        .map(|p| {
                            indicator(if ahead[t] {
                                base[t] + p as i32 >= anchor
                            } else {
                                base[t] + p as i32 <= anchor
                            })
                        })
                        .collect()
                }
            })
            .collect::<Vec<Vec<f64>>>();
        let mut internal = Vec::new();
        let mut ext = vec![Vec::new(); selected.len()];
        let mut selected_games = Vec::new();
        let mut remaining = Vec::new();
        for g in all {
            let h = selected.iter().position(|&t| t == g.home);
            let a = selected.iter().position(|&t| t == g.away);
            match (h, a) {
                (Some(h), Some(a)) => {
                    internal.push((g, h, a));
                    selected_games.push(g);
                }
                (Some(h), None) => {
                    ext[h].push(g);
                    selected_games.push(g);
                }
                (None, Some(a)) => {
                    ext[a].push(g);
                    selected_games.push(g);
                }
                _ => remaining.push(g),
            }
        }
        let tables = ext
            .into_iter()
            .enumerate()
            .map(|(i, g)| TerminalTable::new(g, selected[i], terminal[i].clone()))
            .collect::<Vec<_>>();
        let states = 3usize.checked_pow(internal.len() as u32)?;
        let mut patterns = Vec::new();
        let mut joint_mass = 0.;
        for mut code in 0..states {
            let mut added = vec![0; selected.len()];
            let mut outcomes = Vec::new();
            let mut mass = 1.;
            for (g, h, a) in &internal {
                let o = code % 3;
                code /= 3;
                outcomes.push(o as u8);
                added[*h] += g.hg[o] as usize;
                added[*a] += g.ag[o] as usize;
                mass *= g.prob[o];
            }
            for (table, &p) in tables.iter().zip(&added) {
                mass *= table.mass(p);
            }
            if mass > 0. {
                joint_mass += mass;
                patterns.push(Pattern {
                    outcomes,
                    added,
                    cumulative: joint_mass,
                });
            }
        }
        if !joint_mass.is_finite() || joint_mass <= 0. {
            return None;
        }
        let max_cap = max[target] - *base.iter().min()?;
        if !(0..=SPAN_LIMIT as i32).contains(&max_cap) {
            return None;
        }
        let suffix = Suffix::new(&remaining, m.n, max_cap);
        Some(Self {
            cell,
            stride,
            anchor,
            base,
            selected,
            ahead,
            terminal,
            target_table,
            tables,
            internal,
            selected_games,
            remaining,
            suffix,
            patterns,
            target_mass,
            joint_mass,
            states,
            target_totals,
        })
    }
    pub fn describe(&self, m: &Model) -> serde_json::Value {
        json!({"team":m.ids[self.cell.team],"rank":self.cell.rank+1,"mode":"target_total_mixture",
            "base_share":BASE_SHARE,"residual_base_share":RESIDUAL_BASE_SHARE,"target_mass":self.target_mass,"joint_normalizer":self.joint_mass,
            "anchor_points":self.anchor/self.stride,"anchor_wins":if self.stride>1{Some(self.anchor%self.stride)}else{None},
            "selected":self.selected.iter().map(|&t|m.ids[t]).collect::<Vec<_>>(),
            "ahead":self.selected.iter().skip(1).map(|&t|self.ahead[t]).collect::<Vec<_>>(),
            "target_totals":self.target_totals,
            "internal_games":self.internal.len(),"enumerated_states":self.states,
            "selected_games":self.selected_games.len(),"residual_games":self.remaining.len()})
    }
    fn joint_factor(&self, points: &[i32]) -> f64 {
        self.selected
            .iter()
            .enumerate()
            .map(|(i, &t)| {
                let p = points[t] - self.base[t];
                if p < 0 {
                    0.
                } else {
                    self.terminal[i].get(p as usize).copied().unwrap_or(0.)
                }
            })
            .product()
    }
    // Density of the entire mixture, including its target-total tilt and
    // overlapping component support. A component-only denominator is biased.
    fn mixture_weight(&self, factor: f64) -> f64 {
        if factor > 0. {
            self.joint_mass
                / (BASE_SHARE * self.joint_mass + (1. - BASE_SHARE) * self.target_mass * factor)
        } else {
            1. / BASE_SHARE
        }
    }
    pub fn sample(&self, m: &Model, samples: usize, seed: i64, guided: bool) -> Result {
        let mut result = Result {
            mass: self.target_mass,
            ..Default::default()
        };
        let mut rng = Rng::new(seed);
        let mut scores = ScoreContext::new(m);
        let mut out = vec![0; m.fixtures.len()];
        let mut points = self.base.clone();
        let (mut sum, mut sum2, mut max, mut batches) = (0., 0., 0_f64, [0.; 2]);
        for draw in 0..samples {
            if rng.float() < BASE_SHARE {
                self.target_table.sample(0, &mut rng, &mut out);
                for g in &self.selected_games {
                    if g.home != self.cell.team && g.away != self.cell.team {
                        out[g.index] = choose(g.prob, 1., &mut rng) as u8;
                    }
                }
            } else {
                let u = rng.float() * self.joint_mass;
                let p = &self.patterns[self
                    .patterns
                    .partition_point(|p| p.cumulative < u)
                    .min(self.patterns.len() - 1)];
                for ((g, _, _), &o) in self.internal.iter().zip(&p.outcomes) {
                    out[g.index] = o;
                }
                for (table, &added) in self.tables.iter().zip(&p.added) {
                    table.sample(added, &mut rng, &mut out);
                }
            }
            points.copy_from_slice(&self.base);
            for g in &self.selected_games {
                let o = out[g.index] as usize;
                points[g.home] += g.hg[o];
                points[g.away] += g.ag[o];
            }
            let mut weight = self.mixture_weight(self.joint_factor(&points));
            let target = points[self.cell.team];
            let mut expected_above = 0.;
            let mut expected_below = 0.;
            for t in 0..m.n {
                if t != self.cell.team {
                    expected_above += 1. - self.suffix.cdf(0, t, target - points[t]);
                    expected_below += self.suffix.cdf(0, t, target - points[t] - 1);
                }
            }
            let above = if expected_above > 0. {
                (self.cell.rank as f64 / expected_above).min(1.).powi(6)
            } else {
                1.
            };
            let below = if expected_below > 0. {
                ((m.n - 1 - self.cell.rank) as f64 / expected_below)
                    .min(1.)
                    .powi(6)
            } else {
                1.
            };
            for (step, g) in self.remaining.iter().enumerate() {
                let mut q = g.prob;
                if guided {
                    for o in 0..3 {
                        q[o] *= self
                            .suffix
                            .factor(
                                step + 1,
                                g.home,
                                target - points[g.home] - g.hg[o],
                                below,
                                above,
                            )
                            .max(0.)
                            * self
                                .suffix
                                .factor(
                                    step + 1,
                                    g.away,
                                    target - points[g.away] - g.ag[o],
                                    below,
                                    above,
                                )
                                .max(0.);
                    }
                }
                if guided {
                    let guide_mass = q.iter().sum::<f64>();
                    if guide_mass > 0. {
                        for o in 0..3 {
                            q[o] = (1. - RESIDUAL_BASE_SHARE) * q[o]
                                + RESIDUAL_BASE_SHARE * guide_mass * g.prob[o];
                        }
                    } else {
                        q = g.prob;
                    }
                }
                let total: f64 = q.iter().sum();
                if total <= 0. {
                    weight = 0.;
                    break;
                }
                let o = choose(q, total, &mut rng);
                weight *= g.prob[o] * total / q[o];
                out[g.index] = o as u8;
                points[g.home] += g.hg[o];
                points[g.away] += g.ag[o];
            }
            result.samples += 1;
            if weight <= 0. {
                continue;
            }
            let above_count = (0..m.n)
                .filter(|&t| t != self.cell.team && points[t] > target)
                .count();
            let ties = (0..m.n)
                .filter(|&t| t != self.cell.team && points[t] == target)
                .count();
            if above_count > self.cell.rank || above_count + ties < self.cell.rank {
                continue;
            }
            if scores.rank(self.cell.team, &out, &mut rng, None) != self.cell.rank {
                continue;
            }
            result.hits += 1;
            if result.witness.is_none() {
                result.witness = Some(out.clone());
            }
            sum += weight;
            sum2 += weight * weight;
            max = max.max(weight);
            batches[usize::from(draw >= samples / 2)] += weight;
        }
        result.work = result.samples as u64 * work_per_sample(m);
        result.summarize(sum, sum2, max, batches);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{conditioned::canonical_ranks, model::Request};
    fn league(points: [i32; 4], sort: &str) -> Model {
        let mut games = Vec::new();
        for h in 0..4 {
            for a in h + 1..4 {
                games.push(json!({"id":games.len()+1,"home_id":h,"away_id":a,
                "home_power":1.2+0.1*h as f64,"away_power":0.9+0.1*a as f64}));
            }
        }
        let request: Request=serde_json::from_value(json!({"id":1,
            "phase":{"sort":sort,"championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":points.into_iter().enumerate().map(|(t,p)|json!({"team_id":t,"add_sub":p,"bias":t})).collect::<Vec<_>>(),
            "games":games})).unwrap();
        Model::new(request).unwrap()
    }
    fn enumerate(m: &Model, p: &BroadJoint) -> (f64, f64, f64, f64, f64, f64) {
        let (mut target_mass, mut joint_mass, mut q_mass, mut true_rank, mut weighted_rank) =
            (0., 0., 0., 0., 0.);
        let mut outside_joint = 0.;
        for mut code in 0..3usize.pow(m.fixtures.len() as u32) {
            let mut points = p.base.clone();
            let mut out = Vec::new();
            let mut mass = 1.;
            for g in &m.fixtures {
                let o = code % 3;
                code /= 3;
                out.push(o as u8);
                mass *= g.prob[o];
                let hg = [0, p.stride, 3 * p.stride + i32::from(p.stride > 1)];
                points[g.home] += hg[o];
                points[g.away] += hg[2 - o];
            }
            let hit = canonical_ranks(m, &out)[p.cell.team] == p.cell.rank;
            if hit {
                true_rank += mass;
            }
            let target_added = (points[p.cell.team] - p.base[p.cell.team]) as usize;
            let allowed = p.target_table.rows.last().unwrap()[target_added] > 0.;
            assert!(!hit || allowed, "a valid rank path lost target support");
            if !allowed {
                continue;
            }
            target_mass += mass;
            let factor = p.joint_factor(&points);
            joint_mass += mass * factor;
            if hit && factor == 0. {
                outside_joint += mass;
            }
            let q = mass / p.target_mass / p.mixture_weight(factor);
            q_mass += q;
            if hit {
                weighted_rank += q * p.target_mass * p.mixture_weight(factor);
            }
        }
        (
            target_mass,
            joint_mass,
            q_mass,
            true_rank,
            weighted_rank,
            outside_joint,
        )
    }
    #[test]
    fn mixture_is_normalized_and_covers_rank_hits_outside_joint_constraints() {
        let mut observed_outside = false;
        for sort in ["pt,w,bias", "pt,bias"] {
            let m = league([0, 1, 2, 3], sort);
            for team in 0..m.n {
                for rank in 0..m.n {
                    let Some(p) = BroadJoint::new(&m, Cell { team, rank }) else {
                        continue;
                    };
                    let (t, j, q, exact, weighted, outside) = enumerate(&m, &p);
                    assert!((t - p.target_mass).abs() < 1e-12);
                    assert!((j - p.joint_mass).abs() < 1e-12);
                    assert!((q - 1.).abs() < 1e-12, "q={q}");
                    assert!((weighted - exact).abs() < 1e-12);
                    // A component-only estimate would be smaller in these cases.
                    if outside > 0. {
                        observed_outside = true;
                    }
                }
            }
        }
        assert!(
            observed_outside,
            "test must cover hits excluded by the joint component"
        );
    }
    #[test]
    fn broader_draws_and_residual_weights_match_exhaustive_rank_probability() {
        for sort in ["pt,w,bias", "pt,bias"] {
            let m = league([0, 1, 2, 3], sort);
            for rank in 1..=2 {
                let p = BroadJoint::with_rivals(&m, Cell { team: 0, rank }, 1).unwrap();
                let (_, _, _, exact, _, _) = enumerate(&m, &p);
                for guided in [false, true] {
                    let r = p.sample(&m, 10000, 808, guided);
                    assert!(r.hits > 0);
                    assert!(
                        (r.probability - exact).abs() < 6. * r.std_err + 1e-12,
                        "guided={guided} sort={sort} rank={rank} estimate={} exact={exact}",
                        r.probability
                    );
                    assert_eq!(r.work, r.samples as u64 * work_per_sample(&m));
                }
            }
        }
    }
    #[test]
    fn score_tiebreakers_match_independent_sampling() {
        // A packed point/win tie is not itself a rank hit. Exercise real score
        // sampling and the production goal-based sorter on both prefixes.
        for sort in ["pt,w,gd,gf,bias", "pt,gd,gf,bias"] {
            let m = league([0, 1, 2, 3], sort);
            let cell = Cell { team: 0, rank: 1 };
            let p = BroadJoint::with_rivals(&m, cell, 1).unwrap();
            let draws = 20000;
            let r = p.sample(&m, draws, 808, true);
            let mut rng = Rng::new(1123);
            let mut scores = ScoreContext::new(&m);
            let mut outcomes = vec![0; m.fixtures.len()];
            let mut hits = 0;
            for _ in 0..draws {
                for (i, fixture) in m.fixtures.iter().enumerate() {
                    outcomes[i] = choose(fixture.prob, 1., &mut rng) as u8;
                }
                if scores.rank(cell.team, &outcomes, &mut rng, None) == cell.rank {
                    hits += 1;
                }
            }
            let probability = hits as f64 / draws as f64;
            let se = (probability * (1. - probability) / draws as f64).sqrt();
            assert!(r.hits > 0);
            assert!(
                (r.probability - probability).abs() < 6. * (r.std_err.powi(2) + se.powi(2)).sqrt(),
                "sort={sort} weighted={} independent={probability}",
                r.probability
            );
        }
    }
    #[test]
    fn unsupported_scoring_and_oversized_arithmetic_skip_without_proof() {
        let mut m = league([0, 1, 2, 3], "gd,pt");
        assert!(BroadJoint::new(&m, Cell { team: 0, rank: 1 }).is_none());
        m.keys = vec![Key::Pt, Key::W];
        m.base[0].points = i32::MAX;
        assert!(BroadJoint::new(&m, Cell { team: 0, rank: 1 }).is_none());
    }
}
