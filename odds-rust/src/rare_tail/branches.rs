//! Complete branch refinement after native results are frozen.
//! Every branch retains positive final sampling allocation; bounds only order
//! work or reduce a tiny branch's draw floor, never truncate event support.
use super::*;
use crate::joint_caps::propagated::lazy::{combine_branches, BranchSample, BranchStrata};

const TOTAL: usize = 30000;
const PILOT: usize = 500;
const FLOOR: usize = 200;
const POSITIVE_BRANCH_LIMIT: usize = 2;
const POSITIVE_DEFENSIVE_PERCENT: usize = 50;

fn job_defensive_percent(positive_refinement: bool, configured: usize) -> usize {
    if positive_refinement {
        POSITIVE_DEFENSIVE_PERCENT
    } else {
        configured
    }
}

fn positive_branches_enabled() -> bool {
    value("RUST_ODDS_RARE_TAIL_POSITIVE_BRANCHES") == "1"
}

fn positive_setup_fee(m: &Model) -> usize {
    let n = m.n;
    let g = m.fixtures.len();
    budget::setup_cost(100_000, 4_000_000)
        .saturating_add(
            4usize
                .saturating_mul(g.saturating_add(1))
                .saturating_mul(4097),
        )
        .saturating_add(
            64usize
                .saturating_mul(n.saturating_add(g).saturating_add(1))
                .saturating_mul(n.saturating_add(4)),
        )
        .saturating_add(
            256usize
                .saturating_mul(n.saturating_add(g).saturating_add(1))
                .saturating_mul(n.saturating_add(8)),
        )
}

fn reserve_positive_setup_fee(budget: &mut WorkBudget, m: &Model) -> Option<usize> {
    let fee = positive_setup_fee(m);
    budget.reserve(fee).then_some(fee)
}

fn positive_skip_fields(
    old: &Estimate,
    reason: &str,
    prepaid_setup_work: usize,
) -> serde_json::Value {
    serde_json::json!({
        "positive_refinement":true,
        "positive_accepted":false,
        "positive_rejection_reason":reason,
        "positive_old_probability":old.probability,
        "positive_old_quality":{"ess":old.ess,"rse":old.relative_se,"max_share":old.max_event_weight_share,"meets_precision_goal":old.meets_precision_goal},
        "positive_new_probability":null,
        "positive_setup_fee_required":prepaid_setup_work,
        "positive_prepaid_setup_work":if reason=="positive_setup_fee_unfundable" {0} else {prepaid_setup_work},
        "branch_coverage":null
    })
}

fn log_positive_skip(
    log: Option<&crate::logging::RequestLog>,
    m: &Model,
    cell: Cell,
    old: &Estimate,
    reason: &str,
    prepaid_setup_work: usize,
) {
    let mut entry = serde_json::json!({
        "group":m.request.id,
        "team":m.ids[cell.team],
        "rank":cell.rank+1,
        "reason":reason
    });
    if let Some(fields) = positive_skip_fields(old, reason, prepaid_setup_work).as_object() {
        for (key, value) in fields {
            entry[key] = value.clone();
        }
    }
    emit(log, "rust_odds_rare_tail_branches_skip", entry);
}

fn weak_positive_estimate(estimate: &Estimate) -> bool {
    let rse = if estimate.probability > 0.0 {
        estimate.std_err / estimate.probability
    } else {
        f64::NAN
    };
    !estimate.meets_precision_goal
        || !estimate.ess.is_finite()
        || estimate.ess < 25.0
        || !rse.is_finite()
        || rse < 0.0
        || rse > 0.25
        || !estimate.max_event_weight_share.is_finite()
        || estimate.max_event_weight_share < 0.0
        || estimate.max_event_weight_share > 0.1
}

fn positive_candidates(m: &Model, estimates: &[Estimate], excluded: &[Cell]) -> Vec<Cell> {
    let mut candidates = estimates
        .iter()
        .enumerate()
        .filter_map(|(index, estimate)| {
            (estimate.probability.is_finite()
                && estimate.probability > 0.0
                && estimate.probability <= 1.0
                && estimate.conditional_samples > 0
                && !estimate.reachability.starts_with("impossible")
                && weak_positive_estimate(estimate))
            .then_some(Cell {
                team: index / m.n,
                rank: index % m.n,
            })
        })
        .filter(|cell| !excluded.contains(cell))
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| {
        estimates[a.index(m.n)]
            .probability
            .total_cmp(&estimates[b.index(m.n)].probability)
            .then(m.ids[a.team].cmp(&m.ids[b.team]))
            .then(a.rank.cmp(&b.rank))
    });

    candidates
}

fn select_positive_candidates(candidates: &[Cell]) -> Vec<Cell> {
    // Give each selected team one turn before considering a second cell from
    // any team. The current bounded pass admits at most two teams.
    let mut selected = Vec::with_capacity(POSITIVE_BRANCH_LIMIT);
    let mut teams = std::collections::BTreeSet::new();
    for &cell in candidates {
        if teams.insert(cell.team) {
            selected.push(cell);
            if selected.len() == POSITIVE_BRANCH_LIMIT {
                break;
            }
        }
    }
    selected
}

#[derive(Clone, Debug)]
struct PositiveBranchCoverage {
    accepted: bool,
    supported: Vec<usize>,
    unsupported: Vec<usize>,
    unsupported_upper_sum: f64,
    unsupported_allowance: f64,
    branches: Vec<serde_json::Value>,
    rejection: Option<&'static str>,
}

fn positive_branch_coverage(
    bounds: &[crate::joint_caps::propagated::lazy::BranchBound],
    main: &[BranchSample],
    check: &[BranchSample],
    allocated: &[usize],
    main_probability: f64,
    check_probability: f64,
) -> PositiveBranchCoverage {
    let count = allocated.len();
    let mut coverage = PositiveBranchCoverage {
        accepted: false,
        supported: Vec::new(),
        unsupported: Vec::new(),
        unsupported_upper_sum: 0.0,
        unsupported_allowance: 0.0,
        branches: Vec::with_capacity(count),
        rejection: None,
    };
    if count == 0 {
        coverage.rejection = Some("empty_branch_set");
        return coverage;
    }
    if bounds.len() != count || main.len() != count || check.len() != count {
        coverage.rejection = Some("branch_count_mismatch");
        return coverage;
    }
    if bounds
        .iter()
        .any(|bound| !bound.upper_bound.is_finite() || bound.upper_bound < 0.0)
    {
        coverage.rejection = Some("invalid_branch_bound");
        return coverage;
    }
    if !main_probability.is_finite()
        || main_probability <= 0.0
        || main_probability > 1.0
        || !check_probability.is_finite()
        || check_probability <= 0.0
        || check_probability > 1.0
    {
        coverage.rejection = Some("invalid_combined_probability");
        return coverage;
    }
    if (0..count).any(|index| {
        allocated[index] == 0
            || main[index].result.samples != allocated[index]
            || check[index].result.samples != allocated[index]
    }) {
        coverage.rejection = Some("final_branch_draw_count_mismatch");
        return coverage;
    }

    for index in 0..count {
        let main_result = &main[index].result;
        let check_result = &check[index].result;
        let ratio = if main_result.probability.is_finite()
            && check_result.probability.is_finite()
            && main_result.probability > 0.0
            && check_result.probability > 0.0
        {
            Some(main_result.probability / check_result.probability)
        } else {
            None
        };
        let supported = branch_result_valid(main_result)
            && branch_result_valid(check_result)
            && main_result.coarse()
            && check_result.coarse()
            && ratio.is_some_and(|ratio| (0.2..=5.0).contains(&ratio));
        if supported {
            coverage.supported.push(index);
        } else {
            coverage.unsupported.push(index);
            coverage.unsupported_upper_sum =
                (coverage.unsupported_upper_sum + bounds[index].upper_bound).next_up();
        }
        coverage.branches.push(serde_json::json!({
            "branch_index":index,
            "allocated_draws":allocated[index],
            "bound_upper":bounds[index].upper_bound,
            "main":{"samples":main_result.samples,"hits":main_result.hits,"ess":main_result.ess,"probability":main_result.probability,"rse":if main_result.probability>0.0 {Some(main_result.std_err/main_result.probability)} else {None},"max_share":main_result.max_share},
            "check":{"samples":check_result.samples,"hits":check_result.hits,"ess":check_result.ess,"probability":check_result.probability,"rse":if check_result.probability>0.0 {Some(check_result.std_err/check_result.probability)} else {None},"max_share":check_result.max_share},
            "main_batch_gap":main_result.batch_gap,
            "check_batch_gap":check_result.batch_gap,
            "pair_ratio":ratio,
            "supported":supported
        }));
    }

    coverage.unsupported_allowance = 0.01 * main_probability.min(check_probability);
    coverage.accepted = coverage.unsupported_upper_sum.is_finite()
        && coverage.unsupported_upper_sum <= coverage.unsupported_allowance;
    if !coverage.accepted {
        coverage.rejection = Some("unsupported_branch_bounds_exceed_allowance");
    }
    coverage
}

fn positive_allocation_floors(pilots: &[(BranchSample, bool, u64, usize)]) -> Option<Vec<usize>> {
    pilots
        .iter()
        .map(|(pilot, _, _, _)| {
            let result = &pilot.result;
            let probability = result.probability;
            let rse = result.std_err / probability;
            if !probability.is_finite()
                || probability <= 0.0
                || probability > 1.0
                || result.samples == 0
                || result.hits == 0
                || result.hits > result.samples
                || !result.ess.is_finite()
                || result.ess <= 0.0
                || !result.std_err.is_finite()
                || result.std_err < 0.0
                || !rse.is_finite()
                || rse < 0.0
                || !result.max_share.is_finite()
                || !(0.0..=1.0).contains(&result.max_share)
                || !result.batch_gap.is_finite()
                || result.batch_gap < 0.0
            {
                return None;
            }

            let samples = result.samples as f64;
            let required = [
                200.0,
                samples * 100.0 / result.hits as f64,
                samples * 50.0 / result.ess,
                samples * (rse / 0.10).powi(2),
                samples * result.max_share / 0.10,
            ];
            let floor = required.into_iter().fold(200.0_f64, f64::max).ceil();
            if !floor.is_finite() || floor > usize::MAX as f64 {
                return None;
            }
            Some(floor as usize)
        })
        .collect()
}

fn branch_result_valid(result: &Result) -> bool {
    result.probability.is_finite()
        && result.probability > 0.0
        && result.probability <= 1.0
        && result.std_err.is_finite()
        && result.std_err >= 0.0
        && result.ess.is_finite()
        && result.ess >= 0.0
        && result.max_share.is_finite()
        && (0.0..=1.0).contains(&result.max_share)
        && result.batch_gap.is_finite()
        && result.batch_gap >= 0.0
}

fn positive_replacement_quality(old: &Estimate, main: &Result, check: &Result) -> bool {
    let old_ess = if old.ess.is_finite() && old.ess >= 0.0 {
        old.ess
    } else {
        0.0
    };
    let old_rse = if old.probability.is_finite()
        && old.probability > 0.0
        && old.std_err.is_finite()
        && old.std_err >= 0.0
    {
        old.std_err / old.probability
    } else {
        f64::INFINITY
    };
    let old_rse = if old_rse.is_finite() && old_rse >= 0.0 {
        old_rse
    } else {
        f64::INFINITY
    };
    let old_share = if old.max_event_weight_share.is_finite()
        && (0.0..=1.0).contains(&old.max_event_weight_share)
    {
        old.max_event_weight_share
    } else {
        1.0
    };
    [main, check].into_iter().all(|result| {
        let rse = if result.probability > 0.0 {
            result.std_err / result.probability
        } else {
            f64::INFINITY
        };
        result.probability.is_finite()
            && result.probability > 0.0
            && result.ess.is_finite()
            && result.ess > old_ess
            && result.std_err.is_finite()
            && result.std_err >= 0.0
            && rse.is_finite()
            && rse < old_rse
            && result.max_share.is_finite()
            && (0.0..=1.0).contains(&result.max_share)
            && result.max_share <= old_share
    })
}

fn commit_positive_estimate(
    estimate: &mut Estimate,
    old: &Estimate,
    main: &Result,
    check: &Result,
    coverage: &PositiveBranchCoverage,
) -> bool {
    if !accepted(main, check, true)
        || !positive_replacement_quality(old, main, check)
        || !coverage.accepted
    {
        return false;
    }
    crate::search::apply(
        estimate,
        main,
        "matched_point_pool_complete_branches_positive",
    );
    true
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct DrawAudit {
    pub credit: usize,
    pub valid: bool,
    pub reserved_draw_units: usize,
    pub actual_draw_units: usize,
    pub already_released_draw_units: usize,
}

impl DrawAudit {
    fn record(&mut self, reserved: usize, actual: usize, released: usize) {
        if released > reserved || actual > reserved.saturating_sub(released) {
            self.valid = false;
        }
        self.reserved_draw_units = self.reserved_draw_units.saturating_add(reserved);
        self.actual_draw_units = self.actual_draw_units.saturating_add(actual);
        self.already_released_draw_units =
            self.already_released_draw_units.saturating_add(released);
    }

    fn finish(&mut self) {
        self.credit = if self.valid {
            self.reserved_draw_units
                .saturating_sub(self.already_released_draw_units)
                .saturating_sub(self.actual_draw_units)
        } else {
            0
        };
    }
}

fn tree_pilot_release(pilot_cost: usize, setup_work: usize, actual_draw_units: usize) -> usize {
    pilot_cost.saturating_sub(setup_work.saturating_add(actual_draw_units))
}

fn fraction(value: &str) -> f64 {
    value
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && (0. ..=0.5).contains(v))
        .unwrap_or(0.15)
}

#[cfg(test)]
fn allocation(scores: &[f64], small: &[bool], total: usize) -> Option<Vec<usize>> {
    allocation_floor(scores, small, total, FLOOR)
}
#[cfg(test)]
fn allocation_floor(
    scores: &[f64],
    small: &[bool],
    total: usize,
    floor: usize,
) -> Option<Vec<usize>> {
    allocation_with_defensive_percent(scores, small, total, floor, 10)
}

fn allocation_with_defensive_percent(
    scores: &[f64],
    small: &[bool],
    total: usize,
    floor: usize,
    defensive_percent: usize,
) -> Option<Vec<usize>> {
    allocation_with_branch_floors(
        scores,
        small,
        &vec![floor; scores.len()],
        total,
        defensive_percent,
    )
}

fn allocation_with_branch_floors(
    scores: &[f64],
    small: &[bool],
    floors: &[usize],
    total: usize,
    defensive_percent: usize,
) -> Option<Vec<usize>> {
    if scores.len() != small.len()
        || scores.len() != floors.len()
        || scores.is_empty()
        || scores.iter().any(|s| !s.is_finite() || *s < 0.)
        || !(10..=50).contains(&defensive_percent)
    {
        return None;
    }
    let mut ns: Vec<_> = small
        .iter()
        .zip(floors)
        .map(|(&is_small, &floor)| if is_small { 2 } else { floor })
        .collect();
    let assigned = ns.iter().try_fold(0usize, |sum, &n| sum.checked_add(n))?;
    if assigned > total {
        return None;
    }
    let mut active: Vec<_> = (0..ns.len()).filter(|&i| !small[i]).collect();
    if active.is_empty() {
        active.extend(0..ns.len());
    }
    let sum: f64 = active.iter().map(|&i| scores[i]).sum();
    let left = total - assigned;
    let uniform_share = defensive_percent as f64 / 100.0;
    for &i in &active {
        let share = if sum > 0. {
            uniform_share / active.len() as f64 + (1.0 - uniform_share) * scores[i] / sum
        } else {
            1. / active.len() as f64
        };
        ns[i] += (left as f64 * share).floor() as usize;
    }
    let remainder = total - ns.iter().sum::<usize>();
    for i in 0..remainder {
        ns[active[i % active.len()]] += 1;
    }
    Some(ns)
}

fn defensive_percent(raw: Option<&str>) -> usize {
    raw.and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10)
        .clamp(10, 50)
}

fn pilot_prior_enabled(raw: Option<&str>) -> bool {
    raw == Some("1")
}

fn skip_empty_pilot_final_enabled(raw: Option<&str>) -> bool {
    raw == Some("1")
}

fn structural_discovery_enabled() -> bool {
    matches!(
        std::env::var("RUST_ODDS_EXPERIMENT_TREE_DISCOVERY").as_deref(),
        Ok("interval" | "rank" | "strict")
    )
}

fn tree_pilot_no_evidence_skip(
    all_zero: bool,
    prior_enabled: bool,
    skip_empty_pilot_final: bool,
    maximum_finite_bound: f64,
) -> bool {
    all_zero
        && !(prior_enabled
            && !skip_empty_pilot_final
            && maximum_finite_bound.is_finite()
            && maximum_finite_bound > 0.0)
}

fn pilot_prior_scores(
    empirical_sd: &[f64],
    samples: &[usize],
    upper_bounds: &[f64],
) -> Option<(Vec<f64>, f64)> {
    if empirical_sd.is_empty()
        || empirical_sd.len() != samples.len()
        || empirical_sd.len() != upper_bounds.len()
        || empirical_sd
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        || upper_bounds.iter().any(|value| !value.is_finite())
    {
        return None;
    }
    let maximum_bound = upper_bounds
        .iter()
        .map(|value| value.max(0.0))
        .fold(0.0, f64::max);
    let maximum_sd = empirical_sd.iter().copied().fold(0.0, f64::max);
    let scores = if maximum_sd > 0.0 && maximum_bound > 0.0 {
        empirical_sd
            .iter()
            .zip(samples)
            .zip(upper_bounds)
            .map(|((&sd, &n), &bound)| {
                let n = n.max(1) as f64;
                let bound_ratio = bound.max(0.0) / maximum_bound;
                let empirical = sd * (n / (n + 20.0)).sqrt();
                let prior = maximum_sd * bound_ratio * (20.0 / (n + 20.0)).sqrt();
                empirical.hypot(prior)
            })
            .collect::<Vec<_>>()
    } else if maximum_sd == 0.0 && maximum_bound > 0.0 {
        upper_bounds
            .iter()
            .map(|bound| bound.max(0.0) / maximum_bound)
            .collect()
    } else {
        empirical_sd.to_vec()
    };
    scores
        .iter()
        .all(|score| score.is_finite() && *score >= 0.0)
        .then_some((scores, maximum_bound))
}

fn sample(
    p: &BranchStrata,
    m: &Model,
    ns: &[usize],
    guides: &[bool],
    order: &[usize],
    seed: i64,
    phase: &str,
    workers: usize,
) -> Vec<BranchSample> {
    let costs: Vec<_> = order
        .iter()
        .map(|&i| ns[i] * p.draw_work(m, i, guides[i]))
        .collect();
    let mut rows = budget::parallel(&costs, workers, |slot| {
        let i = order[slot];
        (
            i,
            p.sample(
                m,
                i,
                ns[i],
                derive(seed, &format!("branch-{phase}-{i}")),
                guides[i],
                true,
            ),
        )
    });
    rows.sort_by_key(|r| r.0);
    rows.into_iter().map(|r| r.1).collect()
}

// Early branch work is paid for by additional confirmations. A reduced
// confirmation override must also reduce the transferable branch allowance.
fn work_limit(configured: usize, available: Option<usize>) -> usize {
    available.map_or(configured, |funded| configured.min(funded))
}

pub(super) fn run(
    m: &Model,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    log: Option<&crate::logging::RequestLog>,
    preceding_ms: f64,
    tightened_only: bool,
    excluded: &[Cell],
    available_work: Option<usize>,
    tree_mode: bool,
) -> (usize, u64, usize, Vec<Cell>, Vec<Cell>, DrawAudit) {
    let floor = if tree_mode { 10 } else { FLOOR };
    let nominal_total = if tree_mode {
        3000
    } else if tightened_only {
        std::env::var("RUST_ODDS_CERTIFIED_BRANCH_DRAWS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|n| (10000..=TOTAL).contains(n))
            .unwrap_or(25000)
    } else {
        TOTAL
    };
    let pilot_draws = if tree_mode { 25 } else { PILOT };
    let start = Instant::now();
    let deterministic = budget::deterministic();
    let defensive_percent_raw = std::env::var("RUST_ODDS_EXPERIMENT_BRANCH_DEFENSIVE_PERCENT").ok();
    let defensive_percent = defensive_percent(defensive_percent_raw.as_deref());
    let prior_raw = std::env::var("RUST_ODDS_EXPERIMENT_BRANCH_PILOT_PRIOR").ok();
    let prior_enabled = pilot_prior_enabled(prior_raw.as_deref());
    let skip_empty_pilot_final = skip_empty_pilot_final_enabled(
        std::env::var("RUST_ODDS_EXPERIMENT_BRANCH_SKIP_EMPTY_PILOT_FINAL")
            .ok()
            .as_deref(),
    );
    let experiment_controls_logged =
        defensive_percent_raw.is_some() || prior_enabled || skip_empty_pilot_final;
    let share = fraction(&value("RUST_ODDS_RARE_TAIL_BRANCH_BUDGET_FRACTION"));
    let allowance = preceding_ms * share;
    let positive_branches_on =
        !tightened_only && !tree_mode && budget::modeled() && positive_branches_enabled();
    let zero_work_limit = work_limit(
        budget::capacity(m, scaled(6 * REFERENCE_DRAWS, share)),
        available_work,
    );
    let positive_work_limit = if positive_branches_on {
        budget::capacity(m, scaled(2 * REFERENCE_DRAWS, share))
    } else {
        0
    };
    let mut zero_budget = WorkBudget::new(zero_work_limit);
    let mut positive_budget = WorkBudget::new(positive_work_limit);
    let audit_enabled = super::target_overflow_tree_enabled() && budget::modeled();
    let mut audit = DrawAudit {
        valid: audit_enabled,
        ..DrawAudit::default()
    };
    let mut cells: Vec<_> = estimates
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            (e.probability == 0. && !e.reachability.starts_with("impossible")).then_some(Cell {
                team: i / m.n,
                rank: i % m.n,
            })
        })
        .filter(|c| !excluded.contains(c))
        .collect();
    let zero_eligible = cells.len();
    let positive_pool = if positive_branches_on {
        positive_candidates(m, estimates, excluded)
    } else {
        Vec::new()
    };
    let positive_eligible = positive_pool.len();
    let positive_selected = select_positive_candidates(&positive_pool);
    cells.extend(positive_selected.iter().copied());
    if tree_mode {
        let mut counted: Vec<_> = cells
            .into_iter()
            .map(|c| (BranchStrata::tree_root_count(m, c, seed), c))
            .collect();
        counted.sort_by_key(|&(n, c)| (n, c.index(m.n)));
        cells = counted.into_iter().map(|(_, c)| c).collect();
    }
    let before: Vec<_> = estimates
        .iter()
        .enumerate()
        .filter(|(_, e)| e.probability > 0.)
        .map(|(i, e)| (i, serde_json::to_value(e).unwrap()))
        .collect();
    let reduce_floor = value("RUST_ODDS_RARE_TAIL_BRANCH_BOUND_FLOOR") == "1";
    let (mut found, mut work, mut attempted, mut skipped, mut setups_ms) = (0, 0, 0, 0, 0.);
    let (mut positive_attempted, mut positive_refined) = (0usize, 0usize);
    let mut positive_setup_fee_work = 0u64;
    let refund_checks = tree_mode || tree_enabled();
    let mut reclaimed_check_work = 0;
    let mut handled = Vec::new();
    let mut tree_eligible = Vec::new();
    for cell in cells.iter().copied() {
        if attempted >= 8 {
            break;
        }
        let positive_refinement = estimates[cell.index(m.n)].probability > 0.0;
        if deterministic && !positive_refinement && zero_budget.reserved >= zero_budget.limit {
            if positive_branches_on {
                continue;
            }
            break;
        }
        if deterministic && positive_refinement && positive_budget.reserved >= positive_budget.limit
        {
            break;
        }
        if !deterministic && start.elapsed().as_secs_f64() * 1000. >= allowance {
            break;
        }
        let job_budget = if positive_refinement {
            &mut positive_budget
        } else {
            &mut zero_budget
        };
        let job_defensive_percent = job_defensive_percent(positive_refinement, defensive_percent);
        attempted += 1;
        let old_estimate = positive_refinement.then(|| estimates[cell.index(m.n)].clone());
        let cell_pilot_draws = if positive_refinement {
            100
        } else {
            pilot_draws
        };
        if positive_refinement {
            positive_attempted += 1;
        }
        let positive_prepaid_setup = if positive_refinement {
            match reserve_positive_setup_fee(job_budget, m) {
                Some(fee) => fee,
                None => {
                    skipped += 1;
                    let fee = positive_setup_fee(m);
                    log_positive_skip(
                        log,
                        m,
                        cell,
                        old_estimate.as_ref().unwrap(),
                        "positive_setup_fee_unfundable",
                        fee,
                    );
                    break;
                }
            }
        } else {
            0
        };
        if positive_refinement {
            positive_setup_fee_work =
                positive_setup_fee_work.saturating_add(positive_prepaid_setup as u64);
        }
        let setup = Instant::now();
        if crate::search::enabled("RUST_ODDS_CERTIFIED_TARGET_LIMITS") {
            let cost = 8 * m.fixtures.len() + 4 * m.n;
            let remaining = job_budget.limit.saturating_sub(job_budget.reserved);
            let node_budget = if budget::modeled() {
                remaining.saturating_sub(16 * m.fixtures.len()) / cost.max(1)
            } else {
                crate::target_limits::NODE_LIMIT
            }
            .min(crate::target_limits::NODE_LIMIT);
            let (c, setup_work) = crate::target_limits::certify_with_work(m, cell, node_budget);
            let charge = if budget::modeled() {
                setup_work
            } else {
                setup_work.div_ceil(budget::reference_cost(m))
            };
            if deterministic && !job_budget.reserve(charge) {
                break;
            }
            emit(
                log,
                "rust_odds_certified_target_limits",
                json!({"team":m.ids[cell.team],"rank":cell.rank+1,"lower_packed":c.lower,"upper_packed":c.upper,"nodes":c.nodes,"reserved_work":charge,"elapsed_ms":setup.elapsed().as_secs_f64()*1000.}),
            );
            if tightened_only && c.lower.is_none() && c.upper.is_none() {
                continue;
            }
        }
        handled.push(cell);
        // Deterministic complete enumeration; no cached target path is omitted.
        let proposal = if tree_mode {
            BranchStrata::with_tree(m, cell, seed, 4, 64, 0, 4_000_000)
        } else {
            BranchStrata::with_secondary(m, cell, seed, 4, true)
        };
        let setup_ms = setup.elapsed().as_secs_f64() * 1000.;
        setups_ms += setup_ms;
        let mut p = match proposal {
            Ok(p) if p.len() * floor <= nominal_total => p,
            result => {
                skipped += 1;
                let reason = match result {
                    Err(reason) => {
                        if reason == "case enumeration budget exhausted" {
                            tree_eligible.push(cell);
                        }
                        reason
                    }
                    Ok(_) => "branch allocation floor exceeds draw budget".into(),
                };
                if positive_refinement {
                    log_positive_skip(
                        log,
                        m,
                        cell,
                        old_estimate.as_ref().unwrap(),
                        &reason,
                        positive_prepaid_setup,
                    );
                } else {
                    emit(
                        log,
                        "rust_odds_rare_tail_branches_skip",
                        json!({"group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,"setup_ms":setup_ms,"reason":reason}),
                    );
                }
                continue;
            }
        };
        let positive_setup_extra = if positive_refinement {
            p.setup_work().saturating_sub(positive_prepaid_setup)
        } else {
            0
        };
        if positive_refinement && positive_setup_extra > 0 {
            if !job_budget.reserve(positive_setup_extra) {
                skipped += 1;
                log_positive_skip(
                    log,
                    m,
                    cell,
                    old_estimate.as_ref().unwrap(),
                    "positive_setup_extra_unfundable",
                    positive_prepaid_setup,
                );
                break;
            }
            positive_setup_fee_work =
                positive_setup_fee_work.saturating_add(positive_setup_extra as u64);
        }
        if !deterministic && start.elapsed().as_secs_f64() * 1000. >= allowance {
            break;
        }
        // Reserve both pilots and complete main/check batches before sampling.
        // Branch count comes from complete enumeration, never pilot outcomes.
        let setup_component = if positive_refinement {
            p.setup_work()
                .saturating_sub(positive_prepaid_setup.saturating_add(positive_setup_extra))
        } else {
            p.setup_work()
        };
        let pilot_cost = if budget::modeled() {
            setup_component
                + (0..p.len())
                    .map(|i| {
                        cell_pilot_draws
                            * (if tree_mode {
                                p.draw_work(m, i, true)
                            } else {
                                p.draw_work(m, i, false) + p.draw_work(m, i, true)
                            })
                    })
                    .sum::<usize>()
        } else {
            2 * p.len() * cell_pilot_draws + 2 * nominal_total
        };
        if deterministic && !job_budget.reserve(pilot_cost) {
            skipped += 1;
            if positive_refinement {
                log_positive_skip(
                    log,
                    m,
                    cell,
                    old_estimate.as_ref().unwrap(),
                    "complete pilot pair exceeds remaining work budget",
                    positive_prepaid_setup,
                );
            } else {
                emit(
                    log,
                    "rust_odds_rare_tail_branches_skip",
                    json!({
                        "group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,
                        "setup_ms":setup_ms,"branches":p.len(),
                        "requested_draws":2 * p.len() * cell_pilot_draws + 2 * nominal_total,
                        "reason":"complete pilot pair exceeds remaining work budget"
                    }),
                );
            }
            continue;
        }
        let bound_clock = Instant::now();
        let bounds = p.bounds(m, false);
        let bounds_ms = bound_clock.elapsed().as_secs_f64() * 1000.;
        let mut order: Vec<_> = (0..p.len()).collect();
        order.sort_by(|&a, &b| {
            bounds[b]
                .upper_bound
                .total_cmp(&bounds[a].upper_bound)
                .then(a.cmp(&b))
        });
        let pilot_clock = Instant::now();
        let mut pilots = parallel(p.len(), workers, |i| {
            if tree_mode {
                let interval = p.sample(
                    m,
                    i,
                    cell_pilot_draws,
                    derive(seed, &format!("branch-bound-pilot-{i}")),
                    true,
                    true,
                );
                let work = interval.result.work;
                let draw_units = interval.result.operations.units();
                return (interval, true, work, draw_units);
            }
            let rank = p.sample(
                m,
                i,
                cell_pilot_draws,
                derive(seed, &format!("branch-pilot-{i}")),
                false,
                true,
            );
            let interval = p.sample(
                m,
                i,
                cell_pilot_draws,
                derive(seed, &format!("branch-bound-pilot-{i}")),
                true,
                true,
            );
            let use_interval = interval.result.ess > rank.result.ess;
            let work = rank.result.work + interval.result.work;
            let draw_units = rank
                .result
                .operations
                .units()
                .saturating_add(interval.result.operations.units());
            (
                if use_interval { interval } else { rank },
                use_interval,
                work,
                draw_units,
            )
        });
        work += pilots.iter().map(|r| r.2).sum::<u64>();
        let pilot_actual = pilots.iter().fold(0usize, |sum, r| sum.saturating_add(r.3));
        let pilot_release = if tree_mode && budget::modeled() {
            tree_pilot_release(pilot_cost, p.setup_work(), pilot_actual)
        } else {
            0
        };
        if tree_mode && budget::modeled() {
            let actual = p.setup_work().saturating_add(pilot_actual);
            if actual < pilot_cost {
                assert!(job_budget.release(pilot_cost - actual));
            }
        }
        if audit_enabled {
            audit.record(
                pilot_cost.saturating_sub(p.setup_work()),
                pilot_actual,
                pilot_release,
            );
        }
        let maximum_finite_bound = bounds
            .iter()
            .map(|bound| bound.upper_bound)
            .filter(|bound| bound.is_finite())
            .map(|bound| bound.max(0.0))
            .fold(0.0, f64::max);
        let defer_empty_skip = tree_mode && structural_discovery_enabled();
        if tree_mode
            && !defer_empty_skip
            && tree_pilot_no_evidence_skip(
                pilots.iter().all(|r| r.0.result.probability <= 0.),
                prior_enabled,
                skip_empty_pilot_final,
                maximum_finite_bound,
            )
        {
            skipped += 1;
            emit(
                log,
                "rust_odds_rare_tail_branches_skip",
                json!({"team":m.ids[cell.team],"rank":cell.rank+1,"reason":"no event evidence in complete tree pilots","pilot_draws":cell_pilot_draws*p.len()}),
            );
            continue;
        }
        let mut message_info = serde_json::Value::Null;
        if tree_mode {
            let bounded_messages = structural_discovery_enabled()
                || crate::rare_tail::family_structure()
                    != crate::rare_tail::family_config::FamilyStructure::Full;
            let re_pilot_guard = if bounded_messages {
                cell_pilot_draws.saturating_mul(
                    (0..p.len())
                        .map(|i| p.draw_work(m, i, true))
                        .max()
                        .unwrap_or(0),
                )
            } else {
                0
            };
            let free_before_messages = job_budget.limit.saturating_sub(job_budget.reserved);
            let message_allowance = if bounded_messages {
                20_000_000usize.min(free_before_messages.saturating_sub(re_pilot_guard))
            } else {
                20_000_000
            };
            if bounded_messages && !job_budget.reserve(message_allowance) {
                skipped += 1;
                continue;
            }
            let rs = pilots
                .iter()
                .map(|r| r.0.result.clone())
                .collect::<Vec<_>>();
            message_info = p.prepare_messages(m, &rs, 1, message_allowance);
            let message_work = message_info["work"].as_u64().unwrap() as usize;
            if bounded_messages {
                assert!(message_work <= message_allowance);
                assert!(job_budget.release(message_allowance - message_work));
            }
            let selected: Vec<_> = message_info["selected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as usize)
                .collect();
            let re_pilot_work = selected
                .iter()
                .map(|&i| cell_pilot_draws * p.draw_work(m, i, true))
                .sum::<usize>();
            let reserve_after_discovery = if bounded_messages {
                job_budget.reserve(re_pilot_work)
            } else {
                job_budget.reserve(message_work + re_pilot_work)
            };
            if !reserve_after_discovery {
                skipped += 1;
                continue;
            }
            let revised = parallel(selected.len(), workers, |slot| {
                let i = selected[slot];
                let r = p.sample(
                    m,
                    i,
                    cell_pilot_draws,
                    derive(seed, &format!("message-pilot-{i}")),
                    true,
                    true,
                );
                let work = r.result.work;
                let draw_units = r.result.operations.units();
                (i, (r, true, work, draw_units))
            });
            let revised_actual = revised
                .iter()
                .fold(0usize, |sum, row| sum.saturating_add(row.1 .3));
            if audit_enabled {
                audit.record(re_pilot_work, revised_actual, 0);
            }
            for (i, row) in revised {
                work += row.2;
                pilots[i] = row;
            }
            if defer_empty_skip && pilots.iter().all(|r| r.0.result.probability <= 0.) {
                skipped += 1;
                emit(
                    log,
                    "rust_odds_rare_tail_branches_skip",
                    json!({"team":m.ids[cell.team],"rank":cell.rank+1,"reason":"no event evidence after structural discovery re-pilot","pilot_draws":cell_pilot_draws*p.len(),"messages":message_info}),
                );
                continue;
            }
        }
        let pilot_ms = pilot_clock.elapsed().as_secs_f64() * 1000.;
        let pilot_probability: f64 = pilots.iter().map(|r| r.0.result.probability).sum();
        let small: Vec<_> = bounds
            .iter()
            .map(|b| {
                reduce_floor
                    && pilot_probability > 0.
                    && b.upper_bound <= pilot_probability * 0.001 / p.len() as f64
            })
            .collect();
        let empirical_sd: Vec<_> = pilots
            .iter()
            .map(|r| r.0.result.std_err * (r.0.result.samples as f64).sqrt())
            .collect();
        let pilot_counts: Vec<_> = pilots.iter().map(|r| r.0.result.samples).collect();
        let pilot_hits: Vec<_> = pilots.iter().map(|r| r.0.result.hits).collect();
        let positive_floors = if positive_refinement {
            match positive_allocation_floors(&pilots) {
                Some(floors) => Some(floors),
                None => {
                    skipped += 1;
                    log_positive_skip(
                        log,
                        m,
                        cell,
                        old_estimate.as_ref().unwrap(),
                        "invalid positive pilot diagnostics",
                        positive_prepaid_setup,
                    );
                    continue;
                }
            }
        } else {
            None
        };
        let (scores, prior_scores_used) = if prior_enabled {
            let samples: Vec<_> = pilots.iter().map(|r| r.0.result.samples).collect();
            let upper_bounds: Vec<_> = bounds.iter().map(|bound| bound.upper_bound).collect();
            match pilot_prior_scores(&empirical_sd, &samples, &upper_bounds) {
                Some((scores, max_bound)) if max_bound > 0.0 => (scores, true),
                _ => (empirical_sd.clone(), false),
            }
        } else {
            (empirical_sd.clone(), false)
        };
        let mut total = nominal_total;
        let costs: Vec<_> = pilots
            .iter()
            .map(|r| budget::draw_cost(&r.0.result))
            .collect();
        let allocation_floors = positive_floors
            .clone()
            .unwrap_or_else(|| vec![floor; p.len()]);
        let allocation_small = if positive_refinement {
            vec![false; p.len()]
        } else {
            small.clone()
        };
        if budget::modeled() {
            // Preserve the ordinary allocation and streams before spending
            // remaining units on a larger independent retry for cheap failures.
            let floor_total = allocation_floors
                .iter()
                .try_fold(0usize, |sum, &n| sum.checked_add(n))
                .unwrap_or(usize::MAX);
            let minimum_total = if positive_refinement {
                floor_total
            } else {
                p.len().saturating_mul(floor)
            };
            let mut allocation_funded = false;
            while total >= minimum_total {
                let Some(ns) = allocation_with_branch_floors(
                    &scores,
                    &allocation_small,
                    &allocation_floors,
                    total,
                    job_defensive_percent,
                ) else {
                    break;
                };
                let cost: usize = ns.iter().zip(&costs).map(|(n, c)| 2 * n * c).sum();
                if job_budget.reserve(cost) {
                    allocation_funded = true;
                    break;
                }
                if positive_refinement {
                    if total == minimum_total {
                        break;
                    }
                    total = total.saturating_sub(1000).max(minimum_total);
                } else {
                    total = total.saturating_sub(1000);
                }
            }
            if (positive_refinement && !allocation_funded) || total < minimum_total {
                skipped += 1;
                if positive_refinement {
                    log_positive_skip(
                        log,
                        m,
                        cell,
                        old_estimate.as_ref().unwrap(),
                        "branch allocation floors exceed remaining draw budget",
                        positive_prepaid_setup,
                    );
                }
                continue;
            }
        }
        let Some(ns) = allocation_with_branch_floors(
            &scores,
            &allocation_small,
            &allocation_floors,
            total,
            job_defensive_percent,
        ) else {
            if positive_refinement {
                log_positive_skip(
                    log,
                    m,
                    cell,
                    old_estimate.as_ref().unwrap(),
                    "branch allocation failed",
                    positive_prepaid_setup,
                );
            }
            continue;
        };
        let guides: Vec<_> = pilots.iter().map(|r| r.1).collect();
        // Soft admission deadline; the entire fixed main/check pair runs after
        // admission. Never stop a reported batch based on its own results.
        let forecast =
            1.3 * pilot_ms * (2 * nominal_total) as f64 / (2 * p.len() * cell_pilot_draws) as f64;
        if !deterministic && start.elapsed().as_secs_f64() * 1000. + forecast > allowance {
            if positive_refinement {
                log_positive_skip(
                    log,
                    m,
                    cell,
                    old_estimate.as_ref().unwrap(),
                    "final pair exceeds remaining admission allowance",
                    positive_prepaid_setup,
                );
            } else {
                emit(
                    log,
                    "rust_odds_rare_tail_branches_skip",
                    json!({"group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,"setup_ms":setup_ms,"pilot_ms":pilot_ms,"forecast_ms":if deterministic {None}else{Some(forecast)},"reason":"final pair exceeds remaining admission allowance"}),
                );
            }
            skipped += 1;
            continue;
        }
        let mut positive_coverage = None;
        let (main, check, main_ms, check_ms) = if tree_mode {
            let clock = Instant::now();
            let costs: Vec<_> = (0..2 * p.len())
                .map(|slot| {
                    let i = order[slot % p.len()];
                    ns[i] * p.draw_work(m, i, guides[i])
                })
                .collect();
            let mut rows = budget::parallel(&costs, workers, |slot| {
                let phase = if slot < p.len() { "main" } else { "check" };
                let i = order[slot % p.len()];
                (
                    slot / p.len(),
                    i,
                    p.sample(
                        m,
                        i,
                        ns[i],
                        derive(seed, &format!("branch-{phase}-{i}")),
                        guides[i],
                        true,
                    ),
                )
            });
            rows.sort_by_key(|r| (r.0, r.1));
            let mut rows: Vec<_> = rows.into_iter().map(|r| r.2).collect();
            let checks = rows.split_off(p.len());
            (
                combine_branches(&rows),
                combine_branches(&checks),
                clock.elapsed().as_secs_f64() * 1000.,
                0.,
            )
        } else {
            let main_clock = Instant::now();
            let mains = sample(&p, m, &ns, &guides, &order, seed, "main", workers);
            let main_ms = main_clock.elapsed().as_secs_f64() * 1000.;
            let main = combine_branches(&mains);
            let check_clock = Instant::now();
            let checks = if positive_refinement || publishable(&main, true) {
                Some(sample(&p, m, &ns, &guides, &order, seed, "check", workers))
            } else {
                None
            };
            let check = checks
                .as_ref()
                .map(|rows| combine_branches(rows))
                .unwrap_or_default();
            let check_ms = check_clock.elapsed().as_secs_f64() * 1000.;
            if positive_refinement {
                positive_coverage = Some(positive_branch_coverage(
                    &bounds,
                    &mains,
                    checks.as_deref().unwrap_or(&[]),
                    &ns,
                    main.probability,
                    check.probability,
                ));
            }
            if refund_checks && check.samples == 0 {
                let unused = ns.iter().zip(&costs).map(|(n, c)| n * c).sum();
                assert!(job_budget.release(unused));
                reclaimed_check_work += unused;
            }
            if audit_enabled {
                let reserved = ns.iter().zip(&costs).fold(0usize, |sum, (n, c)| {
                    sum.saturating_add(2usize.saturating_mul(*n).saturating_mul(*c))
                });
                let released = if refund_checks && check.samples == 0 {
                    ns.iter().zip(&costs).fold(0usize, |sum, (n, c)| {
                        sum.saturating_add(n.saturating_mul(*c))
                    })
                } else {
                    0
                };
                audit.record(
                    reserved,
                    main.operations
                        .units()
                        .saturating_add(check.operations.units()),
                    released,
                );
            }
            (main, check, main_ms, check_ms)
        };
        if tree_mode && audit_enabled {
            let reserved = ns.iter().zip(&costs).fold(0usize, |sum, (n, c)| {
                sum.saturating_add(2usize.saturating_mul(*n).saturating_mul(*c))
            });
            audit.record(
                reserved,
                main.operations
                    .units()
                    .saturating_add(check.operations.units()),
                0,
            );
        }
        work += main.work + check.work;
        let legacy_accepted = accepted(&main, &check, true);
        let positive_accepted = if positive_refinement {
            legacy_accepted
                && old_estimate
                    .as_ref()
                    .is_some_and(|old| positive_replacement_quality(old, &main, &check))
                && positive_coverage
                    .as_ref()
                    .is_some_and(|coverage| coverage.accepted)
        } else {
            false
        };
        let ok = if positive_refinement {
            positive_accepted
        } else {
            legacy_accepted
        };
        let committed = if positive_refinement {
            ok && old_estimate.as_ref().is_some_and(|old| {
                positive_coverage.as_ref().is_some_and(|coverage| {
                    commit_positive_estimate(
                        &mut estimates[cell.index(m.n)],
                        old,
                        &main,
                        &check,
                        coverage,
                    )
                })
            })
        } else {
            ok && commit_estimate(&mut estimates[cell.index(m.n)], &main, true)
        };
        if committed {
            if !positive_refinement {
                estimates[cell.index(m.n)].design = "matched_point_pool_complete_branches".into();
            } else {
                positive_refined += 1;
            }
            estimates[cell.index(m.n)].work_spent +=
                pilots.iter().map(|r| r.2).sum::<u64>() + main.work + check.work;
            found += 1;
        }
        let mut branch_log = json!({"group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,"branches":p.len(),"setup":p.setup_diagnostics(),"parallel_pair":tree_mode,"setup_ms":setup_ms,"bounds_ms":bounds_ms,"messages":message_info,"pilot_ms":pilot_ms,"main_ms":main_ms,"check_ms":check_ms,"pilot_probability":pilot_probability,"small_branch_floors":small.iter().filter(|&&s|s).count(),"allocated_main_draws":total,"estimated_cost_per_draw":costs,"main_draws":main.samples,"main_hits":main.hits,"main_ess":main.ess,"main_max_share":main.max_share,"main_batch_gap":main.batch_gap,"probability":main.probability,"check_probability":check.probability,"check_ess":check.ess,"accepted":ok,"full_support":true,"omitted_probability":0,"forecast_ms":if deterministic {None}else{Some(forecast)}});
        if positive_refinement {
            let coverage = positive_coverage.as_ref();
            let quality_improved = old_estimate
                .as_ref()
                .is_some_and(|old| positive_replacement_quality(old, &main, &check));
            let rejection = if !legacy_accepted {
                Some("main_check_not_accepted")
            } else if !quality_improved {
                Some("global_quality_not_improved")
            } else {
                coverage.and_then(|coverage| coverage.rejection)
            };
            branch_log["positive_refinement"] = json!(true);
            branch_log["positive_accepted"] = json!(positive_accepted);
            branch_log["positive_quality_improved"] = json!(quality_improved);
            branch_log["positive_old_probability"] =
                json!(old_estimate.as_ref().map(|old| old.probability));
            branch_log["positive_new_probability"] = json!(main.probability);
            branch_log["positive_rejection_reason"] = json!(rejection);
            branch_log["positive_prepaid_setup_work"] = json!(positive_prepaid_setup);
            branch_log["positive_setup_extra_work"] = json!(positive_setup_extra);
            branch_log["positive_pilot_draws_per_branch"] = json!(cell_pilot_draws);
            branch_log["positive_defensive_percent"] = json!(job_defensive_percent);
            branch_log["positive_allocation_floors"] = json!(positive_floors);
            branch_log["positive_pilot_branches"] = json!(pilots.iter().enumerate().map(|(index, (pilot, guided, _, _))| {
                let result = &pilot.result;
                json!({
                    "branch_index":index,
                    "guided":guided,
                    "samples":result.samples,
                    "hits":result.hits,
                    "ess":result.ess,
                    "probability":result.probability,
                    "rse":if result.probability > 0.0 {Some(result.std_err / result.probability)} else {None},
                    "max_share":result.max_share,
                    "batch_gap":result.batch_gap,
                    "cost_per_draw":budget::draw_cost(result)
                })
            }).collect::<Vec<_>>());
            branch_log["positive_old_quality"] = json!(old_estimate.as_ref().map(|old| json!({
                "probability":old.probability,"ess":old.ess,"rse":if old.probability>0.0 {Some(old.std_err/old.probability)} else {None},
                "max_share":old.max_event_weight_share,"meets_precision_goal":old.meets_precision_goal
            })));
            branch_log["branch_coverage"] = json!({
                "supported":coverage.map(|value| &value.supported),
                "unsupported":coverage.map(|value| &value.unsupported),
                "unsupported_upper_sum":coverage.map(|value| value.unsupported_upper_sum),
                "unsupported_allowance":coverage.map(|value| value.unsupported_allowance),
                "branches":coverage.map(|value| &value.branches),
                "rejection_reason":rejection
            });
        }
        if experiment_controls_logged {
            let actual_check_draws = if tree_mode || check.samples > 0 {
                ns.clone()
            } else {
                vec![0; p.len()]
            };
            branch_log["allocation_experiment"] = json!({
                "defensive_percent":job_defensive_percent,
                "pilot_prior_enabled":prior_enabled,
                "skip_empty_pilot_final":skip_empty_pilot_final,
                "pilot_prior_scores_used":prior_scores_used,
                "pilot_draws_per_branch":pilot_counts,
                "pilot_hits_per_branch":pilot_hits,
                "pilot_scores":scores,
                "actual_main_draws_per_branch":ns,
                "actual_check_draws_per_branch":actual_check_draws
            });
        }
        emit(log, "rust_odds_rare_tail_branches", branch_log);
        if !ok && budget::modeled() && !positive_refinement {
            let reference = budget::reference_cost(m);
            let average = costs.iter().sum::<usize>() / costs.len().max(1);
            let retry_total = (nominal_total * reference / average.max(reference / 3).max(1))
                .clamp(nominal_total, 3 * nominal_total);
            if let Some(retry_ns) = allocation_with_defensive_percent(
                &scores,
                &small,
                retry_total,
                floor,
                defensive_percent,
            ) {
                let retry_cost: usize = retry_ns.iter().zip(&costs).map(|(n, c)| 2 * n * c).sum();
                if job_budget.reserve(retry_cost) {
                    let clock = Instant::now();
                    let retry = combine_branches(&sample(
                        &p,
                        m,
                        &retry_ns,
                        &guides,
                        &order,
                        seed,
                        "retry-main",
                        workers,
                    ));
                    let check = if publishable(&retry, true) {
                        combine_branches(&sample(
                            &p,
                            m,
                            &retry_ns,
                            &guides,
                            &order,
                            seed,
                            "retry-check",
                            workers,
                        ))
                    } else {
                        Result::default()
                    };
                    let released = if refund_checks && check.samples == 0 {
                        retry_cost / 2
                    } else {
                        0
                    };
                    if refund_checks && check.samples == 0 {
                        assert!(job_budget.release(released));
                        reclaimed_check_work += retry_cost / 2;
                    }
                    if audit_enabled {
                        audit.record(
                            retry_cost,
                            retry
                                .operations
                                .units()
                                .saturating_add(check.operations.units()),
                            released,
                        );
                    }
                    work += retry.work + check.work;
                    let accepted = accepted(&retry, &check, true);
                    if accepted && commit_estimate(&mut estimates[cell.index(m.n)], &retry, true) {
                        estimates[cell.index(m.n)].design =
                            "matched_point_pool_complete_branches_retry".into();
                        found += 1;
                    }
                    let mut retry_log = json!({
                        "group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,
                        "allocated_main_draws":retry_total,"main_draws":retry.samples,
                        "check_draws":check.samples,"probability":retry.probability,
                        "check_probability":check.probability,"accepted":accepted,
                        "reserved_work":retry_cost,"elapsed_ms":clock.elapsed().as_secs_f64()*1000.
                    });
                    if experiment_controls_logged {
                        let actual_retry_check_draws = if check.samples > 0 {
                            retry_ns.clone()
                        } else {
                            vec![0; p.len()]
                        };
                        retry_log["allocation_experiment"] = json!({
                            "defensive_percent":defensive_percent,
                            "pilot_prior_enabled":prior_enabled,
                            "pilot_prior_scores_used":prior_scores_used,
                            "pilot_draws_per_branch":pilot_counts,
                            "pilot_hits_per_branch":pilot_hits,
                            "pilot_scores":scores,
                            "actual_main_draws_per_branch":retry_ns,
                            "actual_check_draws_per_branch":actual_retry_check_draws
                        });
                    }
                    emit(log, "rust_odds_rare_tail_branches_retry", retry_log);
                }
            }
        }
    }
    let total_work_limit = zero_budget.limit.saturating_add(positive_budget.limit);
    let total_reserved_work = zero_budget
        .reserved
        .saturating_add(positive_budget.reserved);
    let mut summary = json!({"group":m.request.id,"tree_mode":tree_mode,"pilot_draws_per_branch":pilot_draws,"tightened_only":tightened_only,"eligible":zero_eligible,"attempted":attempted,"skipped":skipped,"accepted":found,"elapsed_ms":start.elapsed().as_secs_f64()*1000.,"setup_ms":setups_ms,"allowance_ms":if deterministic {None}else{Some(allowance)},"budget_mode":budget::mode(),"work_limit":if deterministic {Some(total_work_limit)}else{None},"reclaimed_check_work":reclaimed_check_work,"reserved_work":if deterministic {Some(total_reserved_work)}else{None},"work":work,"bound_floor":reduce_floor,"preserved_existing_estimates":before.iter().all(|(i, e)| *e == serde_json::to_value(&estimates[*i]).unwrap())});
    summary["zero_work_limit"] = json!(if deterministic {
        Some(zero_budget.limit)
    } else {
        None
    });
    summary["zero_reserved_work"] = json!(if deterministic {
        Some(zero_budget.reserved)
    } else {
        None
    });
    summary["positive_work_limit"] = json!(if deterministic {
        Some(positive_budget.limit)
    } else {
        None
    });
    summary["positive_reserved_work"] = json!(if deterministic {
        Some(positive_budget.reserved)
    } else {
        None
    });
    summary["positive_branches_enabled"] = json!(positive_branches_on);
    summary["positive_eligible"] = json!(positive_eligible);
    summary["positive_selected"] = json!(positive_selected.len());
    summary["positive_attempted"] = json!(positive_attempted);
    summary["positive_refined"] = json!(positive_refined);
    summary["positive_rejected"] = json!(positive_attempted.saturating_sub(positive_refined));
    summary["positive_unselected"] =
        json!(positive_eligible.saturating_sub(positive_selected.len()));
    summary["positive_prepaid_setup_work"] = json!(positive_setup_fee_work);
    summary["positive_defensive_percent"] = json!(if positive_branches_on {
        Some(POSITIVE_DEFENSIVE_PERCENT)
    } else {
        None
    });
    if experiment_controls_logged {
        summary["allocation_experiment"] = json!({
            "defensive_percent":defensive_percent,
            "pilot_prior_enabled":prior_enabled,
            "skip_empty_pilot_final":skip_empty_pilot_final
        });
    }
    emit(log, "rust_odds_rare_tail_branches_summary", summary);
    if audit_enabled {
        audit.finish();
    }
    (
        found,
        work,
        total_reserved_work,
        handled,
        tree_eligible,
        audit,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Model {
        let request = serde_json::from_str(include_str!(
            "../../tests/fixtures/group-16498-2026-10-10.json"
        ))
        .unwrap();
        Model::new(request).unwrap()
    }

    fn result(
        samples: usize,
        probability: f64,
        hits: usize,
        ess: f64,
        rse: f64,
        share: f64,
    ) -> Result {
        Result {
            samples,
            probability,
            hits,
            ess,
            std_err: probability * rse,
            max_share: share,
            batch_gap: 0.0,
            weighted: true,
            ..Result::default()
        }
    }

    fn bound(upper_bound: f64) -> crate::joint_caps::propagated::lazy::BranchBound {
        crate::joint_caps::propagated::lazy::BranchBound {
            fixed_prior: upper_bound,
            domain_prior: upper_bound,
            normalizer_upper: upper_bound,
            six_rival_upper: None,
            read_two_upper: upper_bound,
            upper_bound,
        }
    }

    fn branch(result: Result) -> BranchSample {
        BranchSample {
            result,
            ..BranchSample::default()
        }
    }

    #[test]
    fn draw_audit_credits_discarded_pilot_surplus() {
        let mut audit = DrawAudit {
            valid: true,
            ..DrawAudit::default()
        };
        audit.record(100, 70, 0);
        audit.finish();
        assert_eq!(audit.credit, 30);
        assert!(audit.valid);
    }
    #[test]
    fn tree_pilot_release_subtracts_setup_and_actual_draw_work() {
        assert_eq!(tree_pilot_release(150, 20, 100), 30);
        assert_eq!(tree_pilot_release(150, 20, 140), 0);
    }
    #[test]
    fn draw_audit_subtracts_skipped_check_release_once() {
        let mut audit = DrawAudit {
            valid: true,
            ..DrawAudit::default()
        };
        audit.record(200, 100, 100);
        audit.finish();
        assert_eq!(audit.credit, 0);
        assert_eq!(audit.already_released_draw_units, 100);
    }
    #[test]
    fn settled_tree_pilot_has_no_surplus_credit() {
        let mut audit = DrawAudit {
            valid: true,
            ..DrawAudit::default()
        };
        // The tree runner has already released its pilot surplus before this
        // phase is audited, so the effective reservation equals actual work.
        audit.record(80, 80, 0);
        audit.finish();
        assert_eq!(audit.credit, 0);
    }
    #[test]
    fn draw_audit_overrun_invalidates_all_credit() {
        let mut audit = DrawAudit {
            valid: true,
            ..DrawAudit::default()
        };
        audit.record(50, 60, 0);
        audit.record(100, 50, 0);
        audit.finish();
        assert!(!audit.valid);
        assert_eq!(audit.credit, 0);
    }
    #[test]
    fn released_reservation_cannot_mask_phase_overrun() {
        let mut audit = DrawAudit {
            valid: true,
            ..DrawAudit::default()
        };
        audit.record(100, 60, 50);
        audit.record(200, 100, 0);
        audit.finish();
        assert!(!audit.valid);
        assert_eq!(audit.credit, 0);
    }
    #[test]
    fn failed_setup_without_measured_phase_has_no_credit() {
        let mut audit = DrawAudit {
            valid: true,
            ..DrawAudit::default()
        };
        audit.finish();
        assert_eq!(audit.credit, 0);
        assert_eq!(audit.reserved_draw_units, 0);
    }

    #[test]
    fn transferred_work_cannot_exceed_its_funding() {
        assert_eq!(work_limit(90000, Some(30000)), 30000);
        assert_eq!(work_limit(90000, Some(200000)), 90000);
        assert_eq!(work_limit(90000, Some(0)), 0);
        assert_eq!(work_limit(90000, None), 90000);
    }
    #[test]
    fn allocation_preserves_every_branch_and_fixed_budget() {
        let ns = allocation(&[1., 2., 3.], &[false, false, false], 1000).unwrap();
        assert_eq!(ns.iter().sum::<usize>(), 1000);
        assert!(ns.iter().all(|&n| n >= FLOOR));
        let ns = allocation(&[1., 2., 3.], &[true, false, false], 1000).unwrap();
        assert_eq!(ns[0], 2);
        assert_eq!(ns.iter().sum::<usize>(), 1000);
        assert!(allocation(&[1., 2.], &[false, false], 399).is_none());
        assert!(allocation(&[], &[], 1000).is_none());
        assert!(allocation(&[1.], &[false, true], 1000).is_none());
        let ns = allocation(&[1., 2.], &[true, true], 1000).unwrap();
        assert_eq!(ns.iter().sum::<usize>(), 1000);
        assert!(ns.iter().all(|&n| n >= 2));
        assert!(allocation(&[f64::NAN], &[false], 1000).is_none());
        assert!(allocation(&[-1.], &[false], 1000).is_none());
    }

    #[test]
    fn defensive_percent_defaults_and_clamps_without_changing_legacy_default() {
        assert_eq!(defensive_percent(None), 10);
        assert_eq!(defensive_percent(Some("bad")), 10);
        assert_eq!(defensive_percent(Some("10")), 10);
        assert_eq!(defensive_percent(Some("35")), 35);
        assert_eq!(defensive_percent(Some("5")), 10);
        assert_eq!(defensive_percent(Some("80")), 50);
        assert_eq!(pilot_prior_enabled(None), false);
        assert_eq!(pilot_prior_enabled(Some("0")), false);
        assert_eq!(pilot_prior_enabled(Some("1")), true);
        assert_eq!(skip_empty_pilot_final_enabled(None), false);
        assert_eq!(skip_empty_pilot_final_enabled(Some("0")), false);
        assert_eq!(skip_empty_pilot_final_enabled(Some("1")), true);
    }

    #[test]
    fn ten_percent_allocation_is_bit_exact_legacy_behavior() {
        assert_eq!(
            allocation_floor(&[1.0, 2.0, 3.0], &[false; 3], 1000, FLOOR),
            Some(vec![274, 333, 393])
        );
        assert_eq!(
            allocation_floor(&[0.1, 0.0, 2.0], &[false; 3], 999, FLOOR),
            Some(vec![231, 213, 555])
        );
        assert_eq!(
            allocation_floor(&[1.0, 2.0, 3.0], &[true, false, false], 1000, FLOOR),
            Some(vec![2, 446, 552])
        );
        assert_eq!(
            allocation_with_defensive_percent(&[1.0, 2.0, 3.0], &[false; 3], 1000, FLOOR, 10),
            Some(vec![274, 333, 393])
        );
    }

    #[test]
    fn defensive_allocation_conserves_draws_and_keeps_every_floor() {
        let scores = [1.0, 0.0, 3.0, 2.0];
        let small = [false, false, true, false];
        for percent in [10, 25, 50] {
            let ns =
                allocation_with_defensive_percent(&scores, &small, 5000, FLOOR, percent).unwrap();
            assert_eq!(ns.iter().sum::<usize>(), 5000);
            assert!(ns
                .iter()
                .zip(small)
                .all(|(&n, is_small)| n >= if is_small { 2 } else { FLOOR }));
        }
    }

    #[test]
    fn positive_jobs_use_coverage_defensive_surplus_without_changing_zero_policy() {
        assert_eq!(job_defensive_percent(true, 10), 50);
        assert_eq!(job_defensive_percent(false, 10), 10);
        let floors = [200, 200];
        let zero =
            allocation_with_branch_floors(&[1.0, 9.0], &[false; 2], &floors, 1200, 10).unwrap();
        let positive = allocation_with_branch_floors(
            &[1.0, 9.0],
            &[false; 2],
            &floors,
            1200,
            job_defensive_percent(true, 10),
        )
        .unwrap();
        assert_eq!(zero, vec![312, 888]);
        assert_eq!(positive, vec![440, 760]);
    }

    #[test]
    fn pilot_prior_allocates_to_zero_hit_strata_with_positive_bounds() {
        let (scores, maximum_bound) =
            pilot_prior_scores(&[0.0, 1.0], &[500, 500], &[0.1, 0.4]).unwrap();
        assert_eq!(maximum_bound, 0.4);
        assert!(scores[0] > 0.0);
        assert!(scores[1] > scores[0]);
        let ns =
            allocation_with_defensive_percent(&scores, &[false, false], 3000, FLOOR, 10).unwrap();
        assert!(ns[0] > FLOOR);
        assert_eq!(ns.iter().sum::<usize>(), 3000);
    }

    #[test]
    fn pilot_prior_uses_stable_hypot_for_tiny_probability_scales() {
        let (scores, _) =
            pilot_prior_scores(&[1.0e-200, 2.0e-200], &[1, 1], &[1.0e-200, 2.0e-200]).unwrap();
        assert!(scores.iter().all(|score| score.is_finite() && *score > 0.0));
        assert!(scores[1] > scores[0]);
    }

    #[test]
    fn missing_bounds_keep_empirical_scores_and_do_not_admit_empty_tree_pilots() {
        let (scores, maximum_bound) =
            pilot_prior_scores(&[0.0, 0.25], &[100, 100], &[0.0, -1.0]).unwrap();
        assert_eq!(scores, vec![0.0, 0.25]);
        assert_eq!(maximum_bound, 0.0);
        assert!(tree_pilot_no_evidence_skip(
            true,
            true,
            false,
            maximum_bound
        ));
        assert!(!tree_pilot_no_evidence_skip(true, true, false, 0.2));
        assert!(tree_pilot_no_evidence_skip(true, true, true, 0.2));
        assert!(tree_pilot_no_evidence_skip(true, false, false, 0.2));
        assert!(!tree_pilot_no_evidence_skip(false, true, false, 0.0));
        assert!(pilot_prior_scores(&[f64::NAN], &[100], &[0.5]).is_none());
        assert!(pilot_prior_scores(&[0.0], &[100], &[f64::INFINITY]).is_none());
    }
    #[test]
    fn bounded_fraction_has_explicit_zero_rollback() {
        assert_eq!(fraction("0"), 0.);
        assert_eq!(fraction("0.25"), 0.25);
        for value in ["", "NaN", "inf", "-1", "0.6"] {
            assert_eq!(fraction(value), 0.15);
        }
    }
    #[test]
    fn production_branch_stage_defaults_to_coverage_with_explicit_overrides() {
        assert_eq!(
            resolve_value("RUST_ODDS_RARE_TAIL_BRANCHES", None, true),
            "1"
        );
        assert_eq!(
            resolve_value("RUST_ODDS_RARE_TAIL_BRANCHES", Some("1".into()), true),
            "1"
        );
        assert_eq!(
            resolve_value("RUST_ODDS_RARE_TAIL_BRANCHES", Some("0".into()), true),
            "0"
        );
        assert_eq!(
            resolve_value("RUST_ODDS_RARE_TAIL_BRANCHES", None, false),
            ""
        );
        assert_eq!(
            resolve_value("RUST_ODDS_RARE_TAIL_BRANCH_BOUND_FLOOR", None, true),
            ""
        );
        assert_eq!(
            resolve_value("RUST_ODDS_RARE_TAIL_POSITIVE_BRANCHES", None, true),
            "1"
        );
        assert_eq!(
            resolve_value(
                "RUST_ODDS_RARE_TAIL_POSITIVE_BRANCHES",
                Some("0".into()),
                true
            ),
            "0"
        );
        assert_eq!(
            resolve_value("RUST_ODDS_RARE_TAIL_POSITIVE_BRANCHES", None, false),
            ""
        );
    }

    #[test]
    fn positive_setup_fee_is_paid_before_construction_and_small_grants_decline() {
        let m = snapshot();
        let fee = positive_setup_fee(&m);
        assert!(fee >= budget::setup_cost(100_000, 4_000_000));
        let mut short = WorkBudget::new(fee - 1);
        assert_eq!(reserve_positive_setup_fee(&mut short, &m), None);
        assert_eq!(short.reserved, 0);
        let mut funded = WorkBudget::new(fee);
        assert_eq!(reserve_positive_setup_fee(&mut funded, &m), Some(fee));
        // A later constructor decline does not refund already paid setup work.
        assert_eq!(funded.reserved, fee);
    }

    #[test]
    fn positive_work_grant_is_independent_of_the_zero_stage_budget() {
        let mut zero = WorkBudget::new(300);
        let mut positive = WorkBudget::new(100);
        assert!(zero.reserve(300));
        assert_eq!(zero.reserve(1), false);
        assert!(positive.reserve(100));
        assert_eq!(zero.reserved, 300);
        assert_eq!(positive.reserved, 100);
        assert!(positive.release(50));
        assert_eq!(zero.reserved, 300);
        assert_eq!(positive.reserved, 50);
    }

    #[test]
    fn weak_positive_candidates_exclude_strong_mc_and_impossible_cells() {
        let m = snapshot();
        let mut estimates = vec![Estimate::default(); m.n * m.n];
        let weak = Cell {
            team: m.indices[&16],
            rank: 17,
        };
        let strong = Cell {
            team: m.indices[&17],
            rank: 17,
        };
        let mc = Cell {
            team: m.indices[&18],
            rank: 17,
        };
        let impossible = Cell {
            team: m.indices[&20],
            rank: 17,
        };
        estimates[weak.index(m.n)] = Estimate {
            probability: 1e-30,
            conditional_samples: 20,
            ess: 2.0,
            relative_se: Some(1.0),
            max_event_weight_share: 0.5,
            ..Estimate::default()
        };
        estimates[strong.index(m.n)] = Estimate {
            probability: 0.01,
            conditional_samples: 100,
            meets_precision_goal: true,
            ess: 100.0,
            relative_se: Some(0.1),
            max_event_weight_share: 0.05,
            ..Estimate::default()
        };
        estimates[mc.index(m.n)] = Estimate {
            probability: 0.001,
            ess: 1.0,
            relative_se: Some(1.0),
            ..Estimate::default()
        };
        estimates[impossible.index(m.n)] = Estimate {
            probability: 0.001,
            conditional_samples: 100,
            ess: 1.0,
            relative_se: Some(1.0),
            reachability: "impossible:certificate".into(),
            ..Estimate::default()
        };
        assert_eq!(positive_candidates(&m, &estimates, &[]), vec![weak]);
    }

    #[test]
    fn positive_candidates_sort_by_mass_and_take_distinct_team_turns() {
        let m = snapshot();
        let first_team = m.indices[&16];
        let second_team = m.indices[&17];
        let mut estimates = vec![Estimate::default(); m.n * m.n];
        let first = Cell {
            team: first_team,
            rank: 16,
        };
        let same_team_next = Cell {
            team: first_team,
            rank: 17,
        };
        let second = Cell {
            team: second_team,
            rank: 18,
        };
        for (cell, probability) in [(first, 1e-27), (same_team_next, 1e-26), (second, 2e-26)] {
            estimates[cell.index(m.n)] = Estimate {
                probability,
                conditional_samples: 10,
                ess: 1.0,
                relative_se: Some(1.0),
                max_event_weight_share: 0.5,
                ..Estimate::default()
            };
        }
        let pool = positive_candidates(&m, &estimates, &[]);
        assert_eq!(pool, vec![first, same_team_next, second]);
        assert_eq!(select_positive_candidates(&pool), vec![first, second]);
        let zero_first = [Cell { team: 4, rank: 2 }];
        let appended = zero_first
            .into_iter()
            .chain(select_positive_candidates(&pool))
            .collect::<Vec<_>>();
        assert_eq!(appended[0], zero_first[0]);
        assert_eq!(appended.len(), 3);
    }

    #[test]
    fn positive_training_floors_cover_low_support_and_validate_pilots() {
        let pilots = vec![
            (branch(result(100, 0.2, 80, 80.0, 0.05, 0.05)), false, 0, 0),
            (branch(result(100, 1e-24, 10, 2.0, 0.5, 0.5)), true, 0, 0),
        ];
        let floors = positive_allocation_floors(&pilots).unwrap();
        assert_eq!(floors, vec![200, 2500]);

        let floors = [floors[0], floors[1]];
        let allocation =
            allocation_with_branch_floors(&[1.0, 10.0], &[false, false], &floors, 3000, 10)
                .unwrap();
        assert_eq!(allocation.iter().sum::<usize>(), 3000);
        assert!(allocation
            .iter()
            .zip(floors)
            .all(|(&draws, floor)| draws >= floor));
        assert!(
            allocation_with_branch_floors(&[1.0, 10.0], &[false, false], &floors, 2699, 10,)
                .is_none()
        );

        let invalid = vec![(branch(result(100, 0.0, 0, 0.0, 0.0, 0.0)), false, 0, 0)];
        assert!(positive_allocation_floors(&invalid).is_none());
    }

    #[test]
    fn invalid_positive_quality_diagnostics_are_weak_but_out_of_range_probability_is_excluded() {
        let m = snapshot();
        let negative_rse = Cell {
            team: m.indices[&16],
            rank: 15,
        };
        let negative_share = Cell {
            team: m.indices[&17],
            rank: 15,
        };
        let invalid_probability = Cell {
            team: m.indices[&18],
            rank: 15,
        };
        let mut estimates = vec![Estimate::default(); m.n * m.n];
        estimates[negative_rse.index(m.n)] = Estimate {
            probability: 0.01,
            std_err: -0.001,
            conditional_samples: 100,
            meets_precision_goal: true,
            ess: 100.0,
            max_event_weight_share: 0.05,
            ..Estimate::default()
        };
        estimates[negative_share.index(m.n)] = Estimate {
            probability: 0.02,
            std_err: 0.001,
            conditional_samples: 100,
            meets_precision_goal: true,
            ess: 100.0,
            max_event_weight_share: -0.1,
            ..Estimate::default()
        };
        estimates[invalid_probability.index(m.n)] = Estimate {
            probability: 1.1,
            conditional_samples: 100,
            meets_precision_goal: false,
            ess: 1.0,
            max_event_weight_share: 0.5,
            ..Estimate::default()
        };
        assert_eq!(
            positive_candidates(&m, &estimates, &[]),
            vec![negative_rse, negative_share]
        );
    }

    #[test]
    fn positive_coverage_rejects_a_hidden_large_bound() {
        let allocated = [100, 100];
        let main = [
            branch(result(100, 0.1, 40, 20.0, 0.1, 0.1)),
            branch(result(100, 0.0, 0, 0.0, 0.0, 0.0)),
        ];
        let check = [
            branch(result(100, 0.1, 40, 20.0, 0.1, 0.1)),
            branch(result(100, 0.0, 0, 0.0, 0.0, 0.0)),
        ];
        let coverage = positive_branch_coverage(
            &[bound(0.9), bound(0.05)],
            &main,
            &check,
            &allocated,
            0.1,
            0.1,
        );
        assert!(!coverage.accepted);
        assert_eq!(coverage.supported, vec![0]);
        assert_eq!(coverage.unsupported, vec![1]);
        assert_eq!(
            coverage.rejection,
            Some("unsupported_branch_bounds_exceed_allowance")
        );
    }

    #[test]
    fn positive_coverage_sums_all_small_unsupported_bounds_conservatively() {
        let allocated = [100, 100, 100];
        let main = [
            branch(result(100, 0.0997, 40, 20.0, 0.1, 0.1)),
            branch(result(100, 0.0, 0, 0.0, 0.0, 0.0)),
            branch(result(100, 0.0, 0, 0.0, 0.0, 0.0)),
        ];
        let check = [
            branch(result(100, 0.0997, 40, 20.0, 0.1, 0.1)),
            branch(result(100, 0.0, 0, 0.0, 0.0, 0.0)),
            branch(result(100, 0.0, 0, 0.0, 0.0, 0.0)),
        ];
        let coverage = positive_branch_coverage(
            &[bound(0.1), bound(0.0006), bound(0.0006)],
            &main,
            &check,
            &allocated,
            0.0997,
            0.0997,
        );
        assert!(!coverage.accepted);
        assert!(coverage.unsupported_upper_sum > coverage.unsupported_allowance);
        assert_eq!(coverage.unsupported, vec![1, 2]);
    }

    #[test]
    fn positive_coverage_allows_negligible_sampled_branches_with_small_total_bound() {
        let allocated = [100, 100];
        let main = [
            branch(result(100, 0.1, 40, 20.0, 0.1, 0.1)),
            branch(result(100, 0.0, 0, 0.0, 0.0, 0.0)),
        ];
        let check = [
            branch(result(100, 0.1, 40, 20.0, 0.1, 0.1)),
            branch(result(100, 0.0, 0, 0.0, 0.0, 0.0)),
        ];
        let coverage = positive_branch_coverage(
            &[bound(0.9), bound(0.0005)],
            &main,
            &check,
            &allocated,
            0.1,
            0.1,
        );
        assert!(coverage.accepted);
        assert_eq!(coverage.supported, vec![0]);
        assert_eq!(coverage.unsupported, vec![1]);
    }

    #[test]
    fn positive_coverage_rejects_missing_draws_or_invalid_bounds() {
        let samples = [branch(result(99, 0.1, 40, 20.0, 0.1, 0.1))];
        let check = [branch(result(100, 0.1, 40, 20.0, 0.1, 0.1))];
        let missing = positive_branch_coverage(&[bound(0.0)], &samples, &check, &[100], 0.1, 0.1);
        assert!(!missing.accepted);
        assert_eq!(missing.rejection, Some("final_branch_draw_count_mismatch"));
        let invalid =
            positive_branch_coverage(&[bound(f64::NAN)], &check, &check, &[100], 0.1, 0.1);
        assert!(!invalid.accepted);
        assert_eq!(invalid.rejection, Some("invalid_branch_bound"));
    }

    #[test]
    fn positive_replacement_requires_both_batches_to_improve_but_allows_probability_jumps() {
        let old = Estimate {
            probability: 1e-27,
            std_err: 5e-28,
            ess: 5.0,
            relative_se: Some(0.5),
            max_event_weight_share: 0.3,
            ..Estimate::default()
        };
        let main = result(100, 1e-23, 40, 10.0, 0.1, 0.2);
        let check = result(100, 1e-23, 40, 8.0, 0.2, 0.1);
        assert!(positive_replacement_quality(&old, &main, &check));
        let mut poor_check = check.clone();
        poor_check.ess = old.ess;
        assert!(!positive_replacement_quality(&old, &main, &poor_check));
    }

    #[test]
    fn rejected_positive_refinement_preserves_estimate_atomically() {
        let old = Estimate {
            probability: 1e-27,
            std_err: 2e-28,
            samples: 9000,
            hits: 40,
            ess: 5.0,
            mean_weight: 0.75,
            work_spent: 123,
            available: true,
            meets_precision_goal: false,
            relative_se: Some(0.2),
            max_event_weight_share: 0.3,
            zero_hit_upper_95: 0.0,
            design: "previous design".into(),
            reachability: "witness".into(),
            conditional_mass: 0.2,
            conditional_samples: 9000,
            conditional_hits: 40,
        };
        let mut estimate = old.clone();
        let good = result(100, 1e-23, 40, 10.0, 0.1, 0.2);
        let coverage = PositiveBranchCoverage {
            accepted: false,
            supported: vec![0],
            unsupported: vec![],
            unsupported_upper_sum: 0.0,
            unsupported_allowance: 0.0,
            branches: vec![],
            rejection: Some("unsupported_branch_bounds_exceed_allowance"),
        };
        assert!(!commit_positive_estimate(
            &mut estimate,
            &old,
            &good,
            &good,
            &coverage
        ));
        assert_eq!(
            serde_json::to_value(estimate).unwrap(),
            serde_json::to_value(old).unwrap()
        );
    }
}
