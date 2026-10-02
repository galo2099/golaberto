//! Deterministic logical operation costs. These are scheduling units, not CPU
//! cycles or probability weights. Pilots refine costs without consulting time.
//! Ordinary pairs are reserved before sampling. Confirmation also admits spare
//! mains and refunds skipped checks after a deterministic barrier; main-only
//! results never publish. Constructor node/cell caps bound setup attempts.
#[derive(Clone, Copy, Default, Debug)]
pub struct Operations {
    pub fixtures: u64,
    pub guidance: u64,
    pub ranking: u64,
}
impl Operations {
    pub fn units(self) -> usize {
        (8 * self.fixtures + self.guidance + self.ranking) as usize
    }
    pub fn rank(&mut self, m: &crate::model::Model) {
        // Conditional scores, campaign updates, and a comparison-sort allowance.
        self.ranking += (8 * m.fixtures.len()
            + m.n * (m.n.max(2).ilog2() as usize + 1) * m.keys.len().max(1))
            as u64;
    }
    pub fn add(&mut self, other: Self) {
        self.fixtures += other.fixtures;
        self.guidance += other.guidance;
        self.ranking += other.ranking;
    }
}

pub(crate) fn modeled() -> bool {
    deterministic() && std::env::var("RUST_ODDS_WORK_MODEL").as_deref() != Ok("draws")
}
pub(crate) fn mode() -> &'static str {
    if modeled() {
        "operations"
    } else if deterministic() {
        "draws"
    } else {
        "wall"
    }
}
pub(crate) fn capacity(m: &crate::model::Model, draws: usize) -> usize {
    draws.saturating_mul(if modeled() { reference_cost(m) } else { 1 })
}
pub(crate) fn draw_cost(r: &crate::conditioned::Result) -> usize {
    if modeled() {
        per_draw(r)
    } else {
        1
    }
}
pub(crate) fn reference_cost(m: &crate::model::Model) -> usize {
    64 * m.fixtures.len() + 32 * m.n + 1
}
pub(crate) fn per_draw(r: &crate::conditioned::Result) -> usize {
    // Round upward. Counters include early exits and density replays; final
    // observations never feed back into admission or batch sizes.
    r.operations.units().div_ceil(r.samples.max(1)).max(1)
}
pub(crate) fn setup_cost(nodes: usize, guide_values: usize) -> usize {
    16 * nodes + 2 * guide_values
}

/// Run expensive admitted jobs first to reduce the four-worker tail. Return
/// results in admission order so scheduling cannot change aggregation.
pub(crate) fn parallel<T: Send, F: Fn(usize) -> T + Sync>(
    costs: &[usize],
    workers: usize,
    f: F,
) -> Vec<T> {
    if !modeled() {
        return crate::search::parallel(costs.len(), workers, f);
    }
    let mut order: Vec<_> = (0..costs.len()).collect();
    order.sort_by(|&a, &b| costs[b].cmp(&costs[a]).then(a.cmp(&b)));
    let mut results = crate::search::parallel(order.len(), workers, |slot| {
        let i = order[slot];
        (i, f(i))
    });
    results.sort_by_key(|r| r.0);
    results.into_iter().map(|r| r.1).collect()
}

pub(crate) const REFERENCE_DRAWS: usize = 100_000;

pub(crate) fn deterministic() -> bool {
    crate::search::enabled("RUST_ODDS_DETERMINISTIC_WORK")
}

pub(crate) fn scaled(base: usize, fraction: f64) -> usize {
    if !fraction.is_finite() || fraction <= 0. {
        return 0;
    }
    (base as f64 * fraction).floor() as usize
}

pub(crate) fn confirmation_limit(fraction: f64) -> usize {
    // Normalize the existing 1.5 default to 200k requested main/check draws.
    scaled(2 * REFERENCE_DRAWS, fraction / 1.5)
}

pub(crate) struct WorkBudget {
    pub limit: usize,
    pub reserved: usize,
}
impl WorkBudget {
    pub fn new(limit: usize) -> Self {
        Self { limit, reserved: 0 }
    }
    pub fn release(&mut self, units: usize) -> bool {
        let Some(next) = self.reserved.checked_sub(units) else {
            return false;
        };
        self.reserved = next;
        true
    }
    pub fn reserve(&mut self, units: usize) -> bool {
        let Some(next) = self.reserved.checked_add(units) else {
            return false;
        };
        if next > self.limit {
            return false;
        }
        self.reserved = next;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reservations_are_bounded_and_do_not_depend_on_completion_order() {
        let mut budget = WorkBudget::new(12_000);
        assert!(budget.reserve(8_000));
        assert!(!budget.reserve(6_000));
        assert!(budget.reserve(4_000));
        assert!(!budget.reserve(1));
        assert_eq!(budget.reserved, budget.limit);
        // Failed admissions do not consume capacity, including arithmetic overflow.
        let mut big = WorkBudget::new(usize::MAX);
        assert!(big.reserve(usize::MAX));
        assert!(!big.reserve(1));
        assert_eq!(big.reserved, usize::MAX);
    }

    #[test]
    fn pilot_cost_counts_fixtures_guidance_and_ranking_separately() {
        let mut pilot = crate::conditioned::Result {
            samples: 3,
            operations: Operations {
                fixtures: 10,
                guidance: 21,
                ranking: 20,
            },
            ..Default::default()
        };
        assert_eq!(pilot.operations.units(), 121);
        assert_eq!(per_draw(&pilot), 41); // upward rounding
        pilot.operations.add(Operations {
            fixtures: 1,
            guidance: 2,
            ranking: 3,
        });
        assert_eq!(pilot.operations.units(), 134);
        assert_eq!(setup_cost(100, 500), 2600);
    }

    #[test]
    fn identical_work_funds_more_draws_for_a_constrained_proposal() {
        let limit = 10000;
        let setup = 1000;
        let cheap = 30;
        let expensive = 90;
        assert_eq!((limit - setup) / cheap, 3 * ((limit - setup) / expensive));
        for cost in [cheap, expensive] {
            let mut budget = WorkBudget::new(limit);
            assert!(budget.reserve(setup));
            let pair_draws = (limit - setup) / cost;
            assert!(budget.reserve(pair_draws * cost));
            assert!(!budget.reserve(cost));
        }
    }

    #[test]
    fn execution_priority_preserves_admission_order_of_results() {
        let seen = std::sync::Mutex::new(Vec::new());
        let result = parallel(&[2, 8, 5], 1, |i| {
            seen.lock().unwrap().push(i);
            i
        });
        assert_eq!(result, vec![0, 1, 2]);
        if modeled() {
            assert_eq!(*seen.lock().unwrap(), vec![1, 2, 0]);
        }
    }

    #[test]
    fn skipped_check_capacity_is_reused_without_exceeding_the_original_limit() {
        let mut budget = WorkBudget::new(100);
        assert!(budget.reserve(80));
        assert!(!budget.reserve(40));
        assert!(budget.release(40));
        assert!(budget.reserve(40));
        assert_eq!(budget.reserved, 80);
        assert!(!budget.release(81));
        assert_eq!(budget.reserved, 80);
    }
    #[test]
    fn default_stage_limits_and_zero_rollback_are_explicit() {
        assert_eq!(scaled(REFERENCE_DRAWS, 0.45), 45_000);
        assert_eq!(scaled(REFERENCE_DRAWS, 0.45 * 0.5), 22_500);
        assert_eq!(confirmation_limit(1.5), 200_000);
        assert_eq!(confirmation_limit(0.), 0);
        assert_eq!(scaled(6 * REFERENCE_DRAWS, 0.15), 90_000);
        for fraction in [0., -1., f64::NAN, f64::INFINITY] {
            assert_eq!(scaled(REFERENCE_DRAWS, fraction), 0);
        }
    }
}
