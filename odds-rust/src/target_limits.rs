//! Per-request certified target-total limits. Only fully refuted relaxations
//! prune proposal support. Quota exhaustion, hints and witnesses do not.
use crate::{
    model::{Key, Model},
    rank_proof::{Options, RankProof},
    search::Cell,
};
pub(crate) const NODE_LIMIT: usize = 16000;

#[derive(Clone, Copy, Debug, Default)]
pub struct Certified {
    pub lower: Option<i64>,
    pub upper: Option<i64>,
    pub nodes: usize,
    pub work: usize,
}
impl Certified {
    pub fn allows(self, total: i64) -> bool {
        self.lower.is_none_or(|v| total >= v) && self.upper.is_none_or(|v| total <= v)
    }
}
pub(crate) fn cached(m: &Model, cell: Cell) -> Certified {
    m.target_limits
        .lock()
        .unwrap()
        .get(&cell.index(m.n))
        .copied()
        .unwrap_or_default()
}
/// Shared terminal predicate for the exhaustive and sampled-path builders.
pub(crate) fn terminal(m: &Model, cell: Cell, base: &[i32], left: &[i32], win: i32) -> Vec<f64> {
    let limits = cached(m, cell);
    (0..=win * left[cell.team])
        .map(|gain| {
            let total = base[cell.team] + gain;
            let above = (0..m.n)
                .filter(|&t| t != cell.team && base[t] > total)
                .count();
            let below = (0..m.n)
                .filter(|&t| t != cell.team && base[t] + win * left[t] < total)
                .count();
            if above <= cell.rank && below <= m.n - 1 - cell.rank && limits.allows(i64::from(total))
            {
                1.
            } else {
                0.
            }
        })
        .collect()
}
/// Tighten only missing proposals that would exceed complete path enumeration.
/// Results are cached on the immutable request, independent of seeds/workers.
pub fn certify(m: &Model, cell: Cell, budget: usize) -> Certified {
    certify_with_work(m, cell, budget).0
}
/// Cached certificates carry diagnostics, but their setup is charged only on
/// the call that computes them. Distinct cell jobs may run on four workers.
pub(crate) fn certify_with_work(m: &Model, cell: Cell, budget: usize) -> (Certified, usize) {
    if let Some(c) = m
        .target_limits
        .lock()
        .unwrap()
        .get(&cell.index(m.n))
        .copied()
    {
        return (c, 0);
    }
    let result = compute(m, cell, budget);
    let stored = *m
        .target_limits
        .lock()
        .unwrap()
        .entry(cell.index(m.n))
        .or_insert(result);
    (stored, result.work)
}
fn compute(m: &Model, cell: Cell, budget: usize) -> Certified {
    if cell.team >= m.n || cell.rank >= m.n {
        return Certified::default();
    }
    let Some(p) = RankProof::new(m) else {
        return Certified::default();
    };
    let mut left = vec![0; m.ids.len()];
    for g in &m.fixtures {
        left[g.home] += 1;
        left[g.away] += 1;
    }
    let wins = m.keys.get(1) == Some(&Key::W);
    let stride = if wins {
        m.base
            .iter()
            .zip(&left)
            .map(|(c, l)| c.wins + l)
            .max()
            .unwrap()
            + 1
    } else {
        1
    };
    if m.ids.len() != m.n
        || m.n > 32
        || m.fixtures.len() > 256
        || stride <= 0
        || left
            .iter()
            .any(|&l| (3 * i64::from(stride) + i64::from(wins)) * i64::from(l) > 4096)
        || m.base.iter().any(|c| {
            (i64::from(c.points).abs() + 3 * m.fixtures.len() as i64 + 1) * i64::from(stride)
                + i64::from(c.wins)
                >= i64::from(i32::MAX / 4)
        })
    {
        return Certified::default();
    }
    let win = 3 * stride + i32::from(wins);
    let base: Vec<_> = m
        .base
        .iter()
        .map(|c| c.points * stride + if wins { c.wins } else { 0 })
        .collect();
    let allowed = terminal(m, cell, &base, &left, win);
    let mut counts = vec![1usize];
    for _ in 0..left[cell.team] {
        let mut next = vec![0usize; counts.len() + win as usize];
        for (gain, &n) in counts.iter().enumerate() {
            for extra in [0, stride, win] {
                next[gain + extra as usize] = (next[gain + extra as usize] + n).min(65);
            }
        }
        counts = next;
    }
    let totals: Vec<_> = counts
        .iter()
        .enumerate()
        .filter(|(g, n)| **n > 0 && allowed[*g] > 0.)
        .map(|(g, _)| i64::from(base[cell.team]) + g as i64)
        .collect();
    if budget == 0
        || totals.len() < 2
        || counts
            .iter()
            .zip(&allowed)
            .filter(|(_, a)| **a > 0.)
            .map(|(n, _)| *n)
            .sum::<usize>()
            <= 64
    {
        return Certified::default();
    }
    let negative = p.negated();
    let mut c = Certified::default();
    // The direction requiring more rivals is the stronger first query, for
    // every rank. The second direction uses the same remaining node quota.
    for upper in if cell.rank >= m.n - 1 - cell.rank {
        [true, false]
    } else {
        [false, true]
    } {
        let values: Vec<_> = if upper {
            totals.clone()
        } else {
            totals.iter().rev().map(|v| -v).collect()
        };
        let proof = if upper { &p } else { &negative };
        let required = if upper {
            cell.rank
        } else {
            m.n - 1 - cell.rank
        };
        let (mut lo, mut hi) = (1, values.len());
        while lo < hi && c.nodes < budget {
            let mid = (lo + hi) / 2;
            let r = proof.prove_floor(
                cell,
                required,
                budget - c.nodes,
                Options::default(),
                Some(values[mid]),
            );
            c.nodes += r.stats.nodes;
            if r.impossible {
                if upper {
                    c.upper = Some(values[mid] - 1);
                } else {
                    c.lower = Some(-values[mid] + 1);
                }
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
    }
    // A conservative per-node fixture-scan allowance, charged to the existing
    // operation budget. Time is diagnostic and cannot change admission.
    c.work = c.nodes.saturating_mul(8 * m.fixtures.len() + 4 * m.n) + 16 * m.fixtures.len();
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Request;
    fn snapshot() -> Model {
        Model::new(serde_json::from_str::<Request>(include_str!("../../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json")).unwrap()).unwrap()
    }
    #[test]
    fn certified_zero_point_path_is_cached_and_filters_both_builders() {
        let m = snapshot();
        let cell = Cell {
            team: m.indices[&16],
            rank: 15,
        };
        let c = certify(&m, cell, 16000);
        assert!(c.upper.is_some());
        assert!(c.nodes <= 16000);
        // 29 exceeds every possible final win count in this request.
        let base = 57 * 29 + 16;
        assert!(c.allows(base));
        assert!(!c.allows(base + 29));
        let again = certify(&m, cell, 1);
        assert_eq!(again.nodes, c.nodes);
        assert_eq!(again.upper, c.upper);
        assert_eq!(certify_with_work(&m, cell, 16000).1, 0);
        let p =
            crate::joint_caps::propagated::lazy::LazyJoint::new(&m, cell, 808, &[], 32).unwrap();
        assert!(
            (p.describe(&m)["conditioning_mass"].as_f64().unwrap() / 2.0019063431519247e-8 - 1.)
                .abs()
                < 1e-10
        );
        let p = crate::joint_caps::propagated::PropagatedJoint::configured(
            &m,
            cell,
            crate::joint_caps::propagated::Limits {
                minimum_forced: 0,
                rivals: 4,
                target_patterns: 1,
                feasible_cases: 128,
            },
        );
        // Reaching the case constructor instead of failing target enumeration
        // establishes that both builders use the same certified terminal.
        assert!(!matches!(p, Err("target_pattern_limit")));
    }
    #[test]
    fn quota_exhaustion_does_not_remove_an_unproved_total() {
        let m = snapshot();
        let cell = Cell {
            team: m.indices[&16],
            rank: 15,
        };
        let c = certify(&m, cell, 1);
        assert!(c.nodes <= 1);
        assert!(c.allows(58 * 29 + 16));
        let empty = compute(&m, cell, 0);
        assert!(empty.lower.is_none() && empty.upper.is_none());
    }
}
