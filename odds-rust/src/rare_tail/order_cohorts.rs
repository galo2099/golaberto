//! Separately budgeted expected-points cohort proposals for still-zero cells.
use super::{budget, emit};
use crate::{
    conditioned::Result,
    joint_caps::propagated::lazy::LazyJoint,
    model::Model,
    pool::Estimate,
    rng::derive,
    search::{apply, Cell},
};
use serde_json::json;

const MAX_CELLS: usize = 128;
const PILOT_DRAWS: usize = 1_024;
const MIN_PILOT_DRAWS: usize = 32;
const MAX_FINAL_DRAWS: usize = 20_000;
const MIN_FINAL_DRAWS: usize = 1_000;
const NATIVE_FINAL_CAPACITY_NUMERATOR: usize = 2;
const NATIVE_FINAL_CAPACITY_DENOMINATOR: usize = 5;
const TRAINING_CAP_NUMERATOR: usize = 2;
const TRAINING_CAP_DENOMINATOR: usize = 3;
const MAX_NODES: usize = 60_000;
const MAX_GUIDES: usize = 4_000_000;
const MAX_SELECTION_WORK: usize = 2_000_000;
const MAX_ARM_NODE_GUIDE_WORK: usize = 9_000_000;
const MAX_FEASIBLE_NODES_PER_ROOT: usize = 256;
const MAX_FEASIBLE_ROOTS: usize = 32;
const MAX_FEASIBLE_MEMBERSHIPS_PER_ROOT: usize = 4;

pub(crate) fn enabled() -> bool {
    resolve_enabled(
        std::env::var("RUST_ODDS_EXPECTED_COHORTS").ok().as_deref(),
        crate::rare_tail::profile() == "coverage",
    )
}

fn cohort_score_mode() -> &'static str {
    resolve_cohort_score_mode(
        std::env::var("RUST_ODDS_COHORT_SCORE_MODE").ok().as_deref(),
        crate::rare_tail::profile() == "coverage",
    )
}

pub(crate) fn positive_refinement_enabled(m: &Model) -> bool {
    enabled()
        && budget::modeled()
        && cohort_score_mode() == "analytic_gd_cohort"
        && crate::score_integral::supported(m)
}

fn resolve_cohort_score_mode(setting: Option<&str>, coverage: bool) -> &'static str {
    match setting {
        Some("sampled" | "0" | "native") => "sampled",
        Some("analytic_gd_cohort") => "analytic_gd_cohort",
        Some(_) => "sampled",
        // Analytic integration belongs to this late proposal stage only.
        None if coverage => "analytic_gd_cohort",
        None => "sampled",
    }
}

fn relative_se(estimate: &Estimate) -> f64 {
    if estimate.probability > 0.0 && estimate.probability.is_finite() {
        let value = estimate.std_err / estimate.probability;
        if value.is_finite() && value >= 0.0 {
            value
        } else {
            f64::INFINITY
        }
    } else {
        f64::INFINITY
    }
}

fn weak_positive(estimate: &Estimate) -> bool {
    estimate.probability.is_finite()
        && estimate.probability > 0.0
        && !estimate.reachability.starts_with("impossible")
        && estimate.conditional_samples > 0
        && (!estimate.meets_precision_goal
            || estimate.ess < 25.0
            || relative_se(estimate) > 0.25
            || !estimate.max_event_weight_share.is_finite()
            || estimate.max_event_weight_share > 0.1)
}

fn weak_reason(estimate: &Estimate) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if !estimate.meets_precision_goal {
        reasons.push("precision_goal_not_met");
    }
    if estimate.ess < 25.0 {
        reasons.push("ess_below_25");
    }
    if relative_se(estimate) > 0.25 {
        reasons.push("relative_se_above_0_25");
    }
    if !estimate.max_event_weight_share.is_finite() || estimate.max_event_weight_share > 0.1 {
        reasons.push("max_event_weight_share_above_0_1");
    }
    reasons
}

fn admit_candidate(remaining: usize, minimum_work: usize) -> Option<usize> {
    (minimum_work > 0 && remaining >= minimum_work).then_some(minimum_work)
}

fn pilot_is_healthy(pilot: &Result) -> bool {
    let relative_se = if pilot.probability > 0.0 && pilot.probability.is_finite() {
        pilot.std_err / pilot.probability
    } else {
        f64::INFINITY
    };
    pilot.probability.is_finite()
        && pilot.probability > 0.0
        && pilot.ess.is_finite()
        && pilot.ess >= 25.0
        && relative_se.is_finite()
        && relative_se >= 0.0
        && relative_se <= 0.25
        && pilot.max_share.is_finite()
        && pilot.max_share <= 0.1
}

/// Native arm comparisons use a common, fixed pilot stream length. A bounded
/// prefix may be useful diagnostically, but it cannot compete for selection.
fn native_pilot_complete(pilot: &Result) -> bool {
    pilot.samples == PILOT_DRAWS
}

fn feasible_fallback_trigger(arm_health: &[bool], analytic_gd: bool) -> Option<&'static str> {
    if arm_health.is_empty() {
        Some("empty_expected_arms")
    } else if analytic_gd && arm_health.iter().all(|healthy| !healthy) {
        Some("all_expected_pilots_weak")
    } else {
        None
    }
}

pub(crate) fn has_weak_eligible_cells(
    m: &Model,
    estimates: &[Estimate],
    positive_baselines: Option<&[(usize, Estimate)]>,
) -> bool {
    if cohort_score_mode() != "analytic_gd_cohort" || !crate::score_integral::supported(m) {
        return false;
    }
    if estimates.iter().any(weak_positive) {
        return true;
    }
    positive_baselines.is_some_and(|baselines| {
        baselines.iter().any(|(index, baseline)| {
            let current = estimates.get(*index).unwrap_or(baseline);
            weak_positive(if dominates_positive(current, baseline) {
                current
            } else {
                baseline
            })
        })
    })
}

fn dominates_positive(candidate: &Estimate, baseline: &Estimate) -> bool {
    candidate.probability.is_finite()
        && candidate.probability > 0.0
        && candidate.ess.is_finite()
        && candidate.ess > baseline.ess
        && relative_se(candidate) < relative_se(baseline)
}

fn restore_positive_baselines(
    estimates: &mut [Estimate],
    baselines: &[(usize, Estimate)],
) -> (usize, usize) {
    let mut restored = 0;
    let mut kept_better = 0;
    for (index, baseline) in baselines {
        if let Some(current) = estimates.get_mut(*index) {
            if dominates_positive(current, baseline) {
                kept_better += 1;
            } else {
                *current = baseline.clone();
                restored += 1;
            }
        }
    }
    (restored, kept_better)
}

fn candidate_cells(
    estimates: &[Estimate],
    n: usize,
    refine_positives: bool,
    ids: &[i32],
    seed: i64,
) -> Vec<Cell> {
    let mut zeros = Vec::new();
    let mut positives = Vec::new();
    for (index, estimate) in estimates.iter().enumerate() {
        if estimate.probability == 0.0 && !estimate.reachability.starts_with("impossible") {
            zeros.push(Cell {
                team: index / n,
                rank: index % n,
            });
        } else if refine_positives && weak_positive(estimate) {
            positives.push((estimate.probability, index / n, index % n));
        }
    }
    // Keep the established zero-first ordering. Weak positive cells then run
    // from rarest upward, with stable team/rank tie breakers.
    positives.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut cells = super::allocation::interleave(zeros, ids, seed, "expected-cohort-zero");
    cells.extend(
        positives
            .into_iter()
            .map(|(_, team, rank)| Cell { team, rank }),
    );
    cells.truncate(MAX_CELLS);
    cells
}

fn refinement_is_better(old: &Estimate, main: &Result, check: &Result) -> bool {
    fn quality(result: &Result) -> Option<(f64, f64)> {
        let p = result.probability;
        let rse = if p > 0.0 {
            result.std_err / p
        } else {
            f64::INFINITY
        };
        if !p.is_finite() || p <= 0.0 || !result.ess.is_finite() || !rse.is_finite() {
            return None;
        }
        (result.ess >= 10.0 && rse <= 0.5).then_some((result.ess, rse))
    }
    let (Some((main_ess, main_rse)), Some((check_ess, check_rse))) =
        (quality(main), quality(check))
    else {
        return false;
    };
    let old_rse = relative_se(old);
    main_rse < old_rse && check_rse < old_rse && main_ess > old.ess && check_ess > old.ess
}

fn should_replace_positive(
    old: &Estimate,
    main: &Result,
    check: &Result,
    publication_gate: bool,
) -> bool {
    publication_gate && weak_positive(old) && refinement_is_better(old, main, check)
}

fn pair_is_publishable(
    main: &Result,
    check: &Result,
    draws: usize,
    actual_work: usize,
    forecast: usize,
) -> bool {
    main.samples == draws && check.samples == draws && actual_work <= forecast
}

fn resolve_enabled(setting: Option<&str>, coverage: bool) -> bool {
    match setting {
        Some("1") => true,
        Some("0") => false,
        Some(_) => false,
        None => coverage,
    }
}

fn feasible_cohorts_enabled() -> bool {
    resolve_enabled(
        std::env::var("RUST_ODDS_FEASIBLE_COHORTS").ok().as_deref(),
        crate::rare_tail::profile() == "coverage",
    )
}

pub(super) struct Outcome {
    pub(super) accepted: usize,
    pub(super) legacy_sample_work: u64,
}

#[derive(Default)]
struct PassTotals {
    accepted: usize,
    accepted_weak_positives: usize,
    legacy_sample_work: u64,
    actual_work: usize,
    setup_pilot_work: usize,
    actual_final_work: usize,
    forecast_pair_work: usize,
    selected: usize,
    overrun: bool,
}

// This is an approximate two-standard-error consistency guard for sampled
// importance tails, not a rigorous confidence certificate.
#[derive(Clone, Copy, Debug)]
struct NativeEvidence {
    probability: f64,
    std_err: f64,
}

impl NativeEvidence {
    fn from_result(result: &Result) -> Option<Self> {
        (result.probability.is_finite()
            && result.probability > 0.0
            && result.std_err.is_finite()
            && result.std_err >= 0.0
            && result.ess.is_finite()
            && result.ess >= 3.0
            && result.hits >= 30)
            .then_some(Self {
                probability: result.probability,
                std_err: result.std_err,
            })
    }

    fn two_se_lower(self) -> f64 {
        (self.probability - 2.0 * self.std_err).max(0.0)
    }
}

#[derive(Clone, Copy, Debug)]
struct NativeGdDisagreement {
    native_lower: f64,
    gd_upper: f64,
}

fn native_gd_disagreement(
    native: &[Option<NativeEvidence>; 2],
    gd_main: &Result,
    gd_check: &Result,
) -> Option<NativeGdDisagreement> {
    let gd_upper = [gd_main, gd_check]
        .iter()
        .filter(|r| r.probability.is_finite() && r.std_err.is_finite() && r.std_err >= 0.0)
        .map(|r| r.probability + 2.0 * r.std_err)
        .reduce(f64::max)?;
    native
        .iter()
        .flatten()
        .copied()
        .map(NativeEvidence::two_se_lower)
        .filter(|lower| *lower > gd_upper)
        .map(|native_lower| NativeGdDisagreement {
            native_lower,
            gd_upper,
        })
        .max_by(|a, b| a.native_lower.total_cmp(&b.native_lower))
}

pub(crate) fn log_status(
    log: Option<&crate::logging::RequestLog>,
    group: i32,
    is_enabled: bool,
    budget_mode: &str,
) {
    emit(
        log,
        "rust_odds_expected_cohorts_summary",
        json!({"group":group,"enabled":is_enabled,"budget_mode":budget_mode,"status":if is_enabled {"skipped"} else {"disabled"}}),
    );
}

pub(super) fn run(
    m: &Model,
    request_seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    log: Option<&crate::logging::RequestLog>,
    positive_baselines: Option<&[(usize, Estimate)]>,
) -> Outcome {
    let capacity = budget::capacity(m, budget::scaled(2 * budget::REFERENCE_DRAWS, 0.45));
    let rough = crate::rare_tail::value("RUST_ODDS_RARE_TAIL_QUALITY") == "order";
    let analytic_gd = positive_refinement_enabled(m);
    let (restored_baselines, kept_better_baselines) = if analytic_gd {
        positive_baselines.map_or((0, 0), |baselines| {
            restore_positive_baselines(estimates, baselines)
        })
    } else {
        (0, 0)
    };
    let native_cells = candidate_cells(estimates, m.n, false, &m.ids, request_seed);
    let native_candidate_count = native_cells.len();
    let mut native_evidence = vec![[None; 2]; estimates.len()];
    let mut totals = run_pass(
        m,
        request_seed,
        workers,
        estimates,
        log,
        capacity,
        native_cells,
        false,
        rough,
        &mut native_evidence,
    );
    let mut gd_totals = PassTotals::default();
    let mut gd_candidate_count = 0;
    let mut gd_weak_candidate_count = 0;
    if analytic_gd {
        let gd_cells = candidate_cells(estimates, m.n, true, &m.ids, request_seed);
        gd_candidate_count = gd_cells.len();
        gd_weak_candidate_count = gd_cells
            .iter()
            .filter(|cell| estimates[cell.index(m.n)].probability > 0.0)
            .count();
        let remaining = capacity.saturating_sub(totals.actual_work);
        if remaining > 0 && !gd_cells.is_empty() {
            gd_totals = run_pass(
                m,
                request_seed,
                workers,
                estimates,
                log,
                remaining,
                gd_cells,
                true,
                rough,
                &mut native_evidence,
            );
        }
    }
    totals.accepted += gd_totals.accepted;
    totals.accepted_weak_positives += gd_totals.accepted_weak_positives;
    totals.legacy_sample_work = totals
        .legacy_sample_work
        .saturating_add(gd_totals.legacy_sample_work);
    totals.actual_work = totals.actual_work.saturating_add(gd_totals.actual_work);
    totals.setup_pilot_work = totals
        .setup_pilot_work
        .saturating_add(gd_totals.setup_pilot_work);
    totals.actual_final_work = totals
        .actual_final_work
        .saturating_add(gd_totals.actual_final_work);
    totals.forecast_pair_work = totals
        .forecast_pair_work
        .saturating_add(gd_totals.forecast_pair_work);
    totals.selected += gd_totals.selected;
    totals.overrun |= gd_totals.overrun;
    emit(
        log,
        "rust_odds_expected_cohorts_summary",
        json!({"enabled":true,"budget_mode":budget::mode(),"score_mode":if analytic_gd {"analytic_gd_cohort"} else {"sampled"},"gd_eligible":crate::score_integral::supported(m),"restored_donor_baselines":restored_baselines,"kept_better_donor_estimates":kept_better_baselines,"selected_weak_positive_candidates":gd_weak_candidate_count,"replaced_weak_positives":totals.accepted_weak_positives,"preserved_weak_positives":gd_weak_candidate_count.saturating_sub(totals.accepted_weak_positives),"preserved_reason":"failed_publication_or_improvement_gate_or_budget","candidate_cells":native_candidate_count+gd_candidate_count,"selected":totals.selected,"accepted":totals.accepted,"reserve":capacity,"forecast_capacity":capacity,"setup_pilot_work":totals.setup_pilot_work,"actual_work":totals.actual_work,"actual_final_work":totals.actual_final_work,"forecast_pair_work":totals.forecast_pair_work,"overrun":totals.overrun,"capacity_exceeded":totals.actual_work>capacity}),
    );
    Outcome {
        accepted: totals.accepted,
        legacy_sample_work: totals.legacy_sample_work,
    }
}

fn run_pass(
    m: &Model,
    request_seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    log: Option<&crate::logging::RequestLog>,
    capacity: usize,
    cells: Vec<Cell>,
    analytic_gd: bool,
    rough: bool,
    native_evidence: &mut [[Option<NativeEvidence>; 2]],
) -> PassTotals {
    let pass_name = if analytic_gd {
        "gd_refinement"
    } else {
        "native_recovery"
    };
    let initial_capacity = capacity;
    let mut actual_total = 0usize;
    let mut setup_pilot_work = 0usize;
    let mut actual_final_work = 0usize;
    let mut forecast_pair_work = 0usize;
    let mut legacy_sample_work = 0u64;
    let mut accepted_count = 0usize;
    let mut accepted_weak_positive_count = 0usize;
    let mut selected_count = 0usize;
    let mut overrun = false;
    // Base construction only needs a small deterministic pilot floor. Each
    // learned proposal below gets its own component-aware draw reservation.
    let draw_upper = if analytic_gd {
        budget::reference_cost(m).max(1)
    } else {
        budget::ordered_draw_upper_cost(m).max(1)
    };
    let goals = crate::rare_tail::value("RUST_ODDS_LAZY_GOALS") == "1"
        && crate::goal_tilt::GoalTilt::joint_guidance_may_fit_for_any_target(m);
    let goal_preparation = if goals {
        (if analytic_gd {
            MIN_PILOT_DRAWS
        } else {
            PILOT_DRAWS
        })
        .saturating_mul(crate::goal_tilt::JOINT_PREPARATION_ALLOWANCE_PER_DRAW)
    } else {
        0
    };
    for (cell_index, cell) in cells.iter().copied().enumerate() {
        let prior = &estimates[cell.index(m.n)];
        if prior.probability > 0.0 {
            emit(
                log,
                "rust_odds_cohort_refinement_candidate",
                json!({
                    "team":m.ids[cell.team],"rank":cell.rank+1,"probability":prior.probability,
                    "relative_se":relative_se(prior),"ess":prior.ess,
                    "weak_positive_reasons":weak_reason(prior),"action":"evaluate_gd_cohort_refinement"
                }),
            );
        }
        let remaining = initial_capacity.saturating_sub(actual_total);
        let positive_cell = prior.probability > 0.0;
        let constructor_clone = LazyJoint::order_base_constructor_and_clone_allowance(m, cell);
        let minimum_arm = one_arm_minimum(constructor_clone, draw_upper, goal_preparation);
        let pilot_reserve = MIN_PILOT_DRAWS.saturating_mul(draw_upper.saturating_add(if goals {
            crate::goal_tilt::JOINT_PREPARATION_ALLOWANCE_PER_DRAW
        } else {
            0
        }));
        let minimum_grant = minimum_arm.saturating_add(pilot_reserve);
        let Some(minimum_admission) = admit_candidate(remaining, minimum_grant) else {
            emit(
                log,
                "rust_odds_expected_cohorts_skip",
                json!({"team":m.ids[cell.team],"rank":cell.rank+1,"reason":"cost_admission","remaining_work":remaining,"required_work":minimum_grant,"constructor_clone_allowance":constructor_clone,"draw_upper":draw_upper,"goal_preparation":goal_preparation}),
            );
            continue;
        };
        // Training gets at most two thirds of the current residual bank, with
        // the conservative minimum raised when needed. Actual work is charged
        // immediately, so unused training capacity remains available to final
        // pairs and later candidates.
        let training_share =
            remaining.saturating_mul(TRAINING_CAP_NUMERATOR) / TRAINING_CAP_DENOMINATOR;
        let training_cap = remaining.min(minimum_admission.max(training_share));

        let base_seed = derive(
            request_seed,
            &format!(
                "rare-tail-expected-cohort-base-{}-{}",
                m.ids[cell.team], cell.rank
            ),
        );
        let base_start = std::time::Instant::now();
        let (base, base_work, base_report) =
            LazyJoint::new_order_base(m, cell, base_seed, training_cap);
        let mut stage_ms = base_start.elapsed().as_secs_f64() * 1000.0;
        let Some(base) = base else {
            if base_work > training_cap {
                overrun = true;
                setup_pilot_work = setup_pilot_work.saturating_add(base_work);
                actual_total = actual_total.saturating_add(base_work);
                emit(
                    log,
                    "rust_odds_expected_cohorts_skip",
                    json!({"team":m.ids[cell.team],"rank":cell.rank+1,"reason":"base_overrun","base":base_report,"base_work":base_work,"training_cap":training_cap}),
                );
                break;
            }
            setup_pilot_work = setup_pilot_work.saturating_add(base_work);
            actual_total = actual_total.saturating_add(base_work);
            emit(
                log,
                "rust_odds_expected_cohorts_skip",
                json!({"team":m.ids[cell.team],"rank":cell.rank+1,"reason":"base_declined","base":base_report,"base_work":base_work,"training_cap":training_cap}),
            );
            continue;
        };
        if base_work > training_cap {
            overrun = true;
            setup_pilot_work = setup_pilot_work.saturating_add(base_work);
            actual_total = actual_total.saturating_add(base_work);
            emit(
                log,
                "rust_odds_expected_cohorts_skip",
                json!({"team":m.ids[cell.team],"rank":cell.rank+1,"reason":"base_overrun","base_work":base_work,"training_cap":training_cap}),
            );
            break;
        }
        setup_pilot_work = setup_pilot_work.saturating_add(base_work);
        actual_total = actual_total.saturating_add(base_work);
        let clone_work = base.clone_work();
        let one_arm_remaining = arm_training_minimum(clone_work, draw_upper, goal_preparation);
        let mut widths = vec![4usize, 6usize];
        if training_cap.saturating_sub(base_work) < one_arm_remaining.saturating_mul(2) {
            widths.truncate(1);
        }
        let mut training_used = base_work;
        let mut arms = Vec::new();
        let mut candidate_legacy_work = 0u64;
        let mut candidate_overrun = false;

        for (arm_index, width) in widths.iter().copied().enumerate() {
            let arms_left = widths.len() - arm_index;
            let remaining_training = training_cap.saturating_sub(training_used);
            let arm_grant = remaining_training / arms_left.max(1);
            if arm_grant < one_arm_remaining {
                break;
            }
            let arm_start = std::time::Instant::now();
            let mut proposal = base.clone();
            if analytic_gd {
                proposal.set_order_score_mode("analytic_gd_cohort");
            }
            let setup_before = proposal.order_cohort_setup_work();
            let native_before = proposal.setup_work();
            let cohorts =
                proposal.add_expected_cohorts(m, width, MAX_NODES, MAX_GUIDES, MAX_SELECTION_WORK);
            let setup_delta = proposal
                .order_cohort_setup_work()
                .saturating_sub(setup_before);
            let node_guide_delta = proposal.setup_work().saturating_sub(native_before);
            let selection_work = cohorts["selection_work"].as_u64().unwrap_or(0) as usize;
            let setup_work = clone_work.saturating_add(setup_delta);
            let proposal_draw_upper = proposal.order_native_draw_upper_work(m).max(1);
            stage_ms += arm_start.elapsed().as_secs_f64() * 1000.0;
            if node_guide_delta > MAX_ARM_NODE_GUIDE_WORK
                || selection_work > MAX_SELECTION_WORK
                || setup_work > arm_grant
            {
                training_used = training_used.saturating_add(setup_work);
                setup_pilot_work = setup_pilot_work.saturating_add(setup_work);
                actual_total = actual_total.saturating_add(setup_work);
                candidate_overrun = setup_work > arm_grant || training_used > training_cap;
                emit(
                    log,
                    "rust_odds_expected_cohorts_arm",
                    json!({"team":m.ids[cell.team],"rank":cell.rank+1,"width":width,"status":if candidate_overrun {"training_overrun"} else {"setup_cap"},"clone_work":clone_work,"node_guide_work":node_guide_delta,"selection_work":selection_work,"cohorts":cohorts,"arm_grant":arm_grant,"training_cap":training_cap}),
                );
                if candidate_overrun {
                    break;
                }
                continue;
            }
            training_used = training_used.saturating_add(setup_work);
            setup_pilot_work = setup_pilot_work.saturating_add(setup_work);
            actual_total = actual_total.saturating_add(setup_work);
            if cohorts["cohorts_added"].as_u64().unwrap_or(0) == 0 {
                emit(
                    log,
                    "rust_odds_expected_cohorts_arm",
                    json!({"team":m.ids[cell.team],"rank":cell.rank+1,"width":width,"status":"no_cohorts","setup_work":setup_work,"cohorts":cohorts}),
                );
                continue;
            }
            let pilot_allowance = arm_grant.saturating_sub(setup_work);
            let goal_draw_reserve = if goals {
                crate::goal_tilt::JOINT_PREPARATION_ALLOWANCE_PER_DRAW
            } else {
                0
            };
            let native_allowance = if analytic_gd { 0 } else { pilot_allowance };
            let gd_allowance = if analytic_gd { pilot_allowance } else { 0 };
            let native_draw_cap = if analytic_gd {
                0
            } else {
                native_allowance.saturating_sub(if goals {
                    PILOT_DRAWS
                        .saturating_mul(crate::goal_tilt::JOINT_PREPARATION_ALLOWANCE_PER_DRAW)
                } else {
                    0
                }) / proposal_draw_upper.max(1)
            };
            let gd_draws = if analytic_gd {
                bounded_pilot_draws(
                    gd_allowance,
                    proposal_draw_upper
                        .saturating_add(goal_draw_reserve)
                        .saturating_add(gd_attempt_overhead(m)),
                    true,
                )
            } else {
                0
            };
            // Native recovery retains its fixed 1,024-draw request and gives
            // the pilot the full arm allowance; the sampler meters actual work
            // and can return fewer observations when the grant runs out.
            let native_possible = !analytic_gd;
            let gd_possible = analytic_gd && gd_draws >= MIN_PILOT_DRAWS;
            if !native_possible && !gd_possible {
                emit(
                    log,
                    "rust_odds_expected_cohorts_arm",
                    json!({"team":m.ids[cell.team],"rank":cell.rank+1,"width":width,"status":"pilot_budget_preflight","pilot_allowance":pilot_allowance,"native_allowance":native_allowance,"gd_allowance":gd_allowance,"native_draws":native_draw_cap,"gd_draws":gd_draws,"proposal_draw_upper":proposal_draw_upper,"analytic_gd":analytic_gd}),
                );
                continue;
            }
            let pilot_seed = derive(
                request_seed,
                &format!(
                    "rare-tail-expected-cohort-pilot-{}-{}-width-{}",
                    m.ids[cell.team], cell.rank, width
                ),
            );
            let pilot_start = std::time::Instant::now();
            let mut native_pilot = None;
            let mut native_pilot_work = 0usize;
            if native_possible {
                proposal.set_order_score_mode("sampled");
                let result =
                    proposal.sample_order_pilot(m, PILOT_DRAWS, pilot_seed, native_allowance);
                native_pilot_work = result.operations.units();
                training_used = training_used.saturating_add(native_pilot_work);
                setup_pilot_work = setup_pilot_work.saturating_add(native_pilot_work);
                actual_total = actual_total.saturating_add(native_pilot_work);
                candidate_legacy_work = candidate_legacy_work.saturating_add(result.work);
                if training_used > training_cap {
                    candidate_overrun = true;
                    break;
                }
                native_pilot = Some(result);
            }
            let mut gd_pilot = None;
            let mut gd_pilot_work = 0usize;
            if gd_possible {
                proposal.set_order_score_mode("analytic_gd_cohort");
                let gd_seed = derive(
                    request_seed,
                    &format!(
                        "rare-tail-expected-cohort-pilot-{}-{}-width-{}-gd",
                        m.ids[cell.team], cell.rank, width
                    ),
                );
                let result = proposal.sample_order_pilot(m, gd_draws, gd_seed, gd_allowance);
                gd_pilot_work = result.operations.units();
                training_used = training_used.saturating_add(gd_pilot_work);
                setup_pilot_work = setup_pilot_work.saturating_add(gd_pilot_work);
                actual_total = actual_total.saturating_add(gd_pilot_work);
                candidate_legacy_work = candidate_legacy_work.saturating_add(result.work);
                if training_used > training_cap {
                    candidate_overrun = true;
                    break;
                }
                gd_pilot = Some(result);
            }
            let pilot_ms = pilot_start.elapsed().as_secs_f64() * 1000.0;
            stage_ms += pilot_ms;
            let gd_usable = gd_pilot.as_ref().is_some_and(|result| {
                result.samples >= MIN_PILOT_DRAWS
                    && result.hits >= 3
                    && result.ess > 0.0
                    && result.score_integral.cohort_private_integrated > 0
            });
            let gd_predicted_final_paths = gd_pilot.as_ref().map(predicted_final_gd_paths);
            let use_gd = analytic_gd;
            let gd_cache_pressure = false;
            let pilot_selection_reason = if use_gd {
                if positive_cell {
                    "weak_positive_requires_gd"
                } else {
                    "zero_recovery_gd_refinement_pass"
                }
            } else {
                "native_recovery_pass"
            };
            let pilot = if use_gd {
                if !gd_usable {
                    continue;
                }
                proposal.set_order_score_mode("analytic_gd_cohort");
                gd_pilot.expect("GD pilot selected only when usable")
            } else if native_pilot.as_ref().is_some_and(|result| {
                native_pilot_complete(result) && result.hits >= 3 && result.ess > 0.0
            }) {
                proposal.set_order_score_mode("sampled");
                native_pilot.expect("native pilot selected only when usable")
            } else {
                if let Some(result) = native_pilot.as_ref().filter(|r| !native_pilot_complete(r)) {
                    emit(
                        log,
                        "rust_odds_expected_cohorts_arm",
                        json!({"team":m.ids[cell.team],"rank":cell.rank+1,"width":width,
                            "status":"incomplete_native_pilot_discarded",
                            "requested_draws":PILOT_DRAWS,"completed_draws":result.samples,
                            "pilot_work":native_pilot_work,"arm_grant":arm_grant}),
                    );
                }
                continue;
            };
            if use_gd {
                emit(
                    log,
                    "rust_odds_score_cohort_gd_pilot",
                    json!({
                        "team":m.ids[cell.team],"rank":cell.rank+1,"mode":"analytic_gd_cohort",
                        "calls":pilot.score_integral.calls,"fallbacks":pilot.score_integral.fallbacks,
                        "private_eligible":pilot.score_integral.cohort_private_eligible,
                        "private_integrated":pilot.score_integral.cohort_private_integrated,
                        "shared_sampled":pilot.score_integral.cohort_shared_sampled,
                        "snapshot_paths":pilot.score_integral.cache_paths,
                        "snapshot_cells":pilot.score_integral.index_cells
                    }),
                );
            }
            let score = normalized_efficiency(&pilot);
            let pilot_mode = if use_gd {
                "analytic_gd_cohort"
            } else {
                "sampled"
            };
            emit(
                log,
                "rust_odds_expected_cohorts_arm",
                json!({"team":m.ids[cell.team],"rank":cell.rank+1,"width":width,"setup_work":setup_work,"pilot_mode":pilot_mode,"pilot_selection_reason":pilot_selection_reason,"gd_cache_pressure":gd_cache_pressure,"gd_predicted_final_paths":gd_predicted_final_paths,"gd_cache_path_cap":crate::score_integral::MAX_COHORT_GD_PATHS,"native_pilot_work":native_pilot_work,"gd_pilot_work":gd_pilot_work,"pilot_work":native_pilot_work.saturating_add(gd_pilot_work),"pilot_draws":pilot.samples,"pilot_hits":pilot.hits,"pilot_ess":pilot.ess,"efficiency_score":score,"pilot_ms":pilot_ms,"cohorts":cohorts,"arm_grant":arm_grant,"training_cap":training_cap}),
            );
            arms.push((width, proposal, pilot, pilot_ms, score, use_gd));
        }
        let fallback_trigger = if feasible_cohorts_enabled() && !candidate_overrun {
            let health = arms
                .iter()
                .map(|arm| pilot_is_healthy(&arm.2))
                .collect::<Vec<_>>();
            feasible_fallback_trigger(&health, analytic_gd)
        } else {
            None
        };
        'feasible_fallback: {
            let Some(trigger) = fallback_trigger else {
                break 'feasible_fallback;
            };
            let best_pilot = arms
                .iter()
                .max_by(|a, b| a.4.total_cmp(&b.4))
                .map(|arm| json!({
                    "width":arm.0,
                    "score":arm.4,
                    "probability":arm.2.probability,
                    "ess":arm.2.ess,
                    "relative_se":if arm.2.probability > 0.0 {Some(arm.2.std_err / arm.2.probability)} else {None},
                    "max_event_weight_share":arm.2.max_share,
                    "healthy":pilot_is_healthy(&arm.2)
                }));
            emit(
                log,
                "rust_odds_feasible_cohort_trigger",
                json!({"team":m.ids[cell.team],"rank":cell.rank+1,"stage":pass_name,"trigger":trigger,
                    "existing_expected_arms":arms.len(),"best_pilot":best_pilot}),
            );
            let fallback_grant = training_cap.saturating_sub(training_used);
            let clone_work = base.clone_work();
            let positive_cell = prior.probability > 0.0;
            let native_needed = !analytic_gd;
            let native_minimum = if native_needed {
                native_fallback_pilot_minimum(draw_upper, goals)
            } else {
                0
            };
            let gd_per_draw = draw_upper
                .saturating_add(if goals {
                    crate::goal_tilt::JOINT_PREPARATION_ALLOWANCE_PER_DRAW
                } else {
                    0
                })
                .saturating_add(gd_attempt_overhead(m));
            let gd_minimum = if analytic_gd {
                MIN_PILOT_DRAWS
                    .saturating_mul(gd_per_draw)
                    .saturating_mul(5)
                    .div_ceil(4)
            } else {
                0
            };
            let pilot_reserve = match (native_needed, analytic_gd) {
                (true, true) => native_minimum.min(gd_minimum),
                (true, false) => native_minimum,
                (false, true) => gd_minimum,
                (false, false) => 0,
            };
            let fallback_minimum = clone_work.saturating_add(pilot_reserve);
            let mut fallback_report = json!({"status":"declined","reason":"remaining_training_preflight",
                "remaining_training":fallback_grant,"minimum":fallback_minimum});
            if fallback_grant >= fallback_minimum {
                let mut proposal = base.clone();
                if analytic_gd {
                    proposal.set_order_score_mode("analytic_gd_cohort");
                }
                let setup_before = proposal.order_cohort_setup_work();
                let native_before = proposal.setup_work();
                let available_discovery = fallback_grant
                    .saturating_sub(clone_work)
                    .saturating_sub(pilot_reserve)
                    / 2;
                let selection_limit = MAX_SELECTION_WORK.min(available_discovery / 4);
                let node_guide_limit = available_discovery.saturating_sub(selection_limit);
                let max_nodes = MAX_NODES.min(node_guide_limit / 32);
                let max_guides = MAX_GUIDES.min(node_guide_limit / 4);
                let feasible = proposal.add_feasible_cohorts_bounded(
                    m,
                    MAX_FEASIBLE_ROOTS,
                    MAX_FEASIBLE_NODES_PER_ROOT,
                    MAX_FEASIBLE_MEMBERSHIPS_PER_ROOT,
                    max_nodes,
                    max_guides,
                    selection_limit,
                );
                let setup_delta = proposal
                    .order_cohort_setup_work()
                    .saturating_sub(setup_before);
                let node_guide_delta = proposal.setup_work().saturating_sub(native_before);
                let fallback_setup = clone_work.saturating_add(setup_delta);
                let feasible_added = feasible["cohorts_added"].as_u64().unwrap_or(0) as usize;
                fallback_report = json!({"status":if feasible_added>0 {"added"} else {"no_families"},
                    "method":"feasible_cohorts","feasible":feasible,
                    "clone_work":clone_work,"setup_work":fallback_setup,
                    "node_guide_work":node_guide_delta,
                    "remaining_training":fallback_grant,"pilot_reserve":pilot_reserve});
                if feasible_added > 0
                    && node_guide_delta <= MAX_ARM_NODE_GUIDE_WORK
                    && fallback_setup.saturating_add(pilot_reserve) <= fallback_grant
                {
                    training_used = training_used.saturating_add(fallback_setup);
                    setup_pilot_work = setup_pilot_work.saturating_add(fallback_setup);
                    actual_total = actual_total.saturating_add(fallback_setup);
                    let pilot_seed = derive(
                        request_seed,
                        &format!(
                            "rare-tail-feasible_cohorts-pilot-{}-{}",
                            m.ids[cell.team], cell.rank
                        ),
                    );
                    let pilot_allowance = fallback_grant.saturating_sub(fallback_setup);
                    let proposal_draw_upper = proposal.order_native_draw_upper_work(m).max(1);
                    let goal_draw = if goals {
                        crate::goal_tilt::JOINT_PREPARATION_ALLOWANCE_PER_DRAW
                    } else {
                        0
                    };
                    let native_goal_reserve = if goals {
                        PILOT_DRAWS
                            .saturating_mul(crate::goal_tilt::JOINT_PREPARATION_ALLOWANCE_PER_DRAW)
                    } else {
                        0
                    };
                    let (mut native_allowance, mut gd_allowance) = if analytic_gd && native_needed {
                        (pilot_allowance / 2, pilot_allowance - pilot_allowance / 2)
                    } else if analytic_gd {
                        (0, pilot_allowance)
                    } else {
                        (pilot_allowance, 0)
                    };
                    let mut native_cap = if native_needed {
                        native_allowance.saturating_sub(native_goal_reserve)
                            / proposal_draw_upper.max(1)
                    } else {
                        0
                    };
                    let mut gd_draws = if analytic_gd {
                        bounded_pilot_draws(
                            gd_allowance,
                            proposal_draw_upper
                                .saturating_add(goal_draw)
                                .saturating_add(gd_attempt_overhead(m)),
                            true,
                        )
                    } else {
                        0
                    };
                    if analytic_gd && native_needed {
                        if native_cap < MIN_PILOT_DRAWS || gd_draws < MIN_PILOT_DRAWS {
                            native_allowance = pilot_allowance;
                            native_cap = native_allowance.saturating_sub(native_goal_reserve)
                                / proposal_draw_upper.max(1);
                            if native_cap >= MIN_PILOT_DRAWS {
                                gd_allowance = 0;
                                gd_draws = 0;
                            } else {
                                native_allowance = 0;
                                gd_allowance = pilot_allowance;
                                gd_draws = bounded_pilot_draws(
                                    gd_allowance,
                                    proposal_draw_upper
                                        .saturating_add(goal_draw)
                                        .saturating_add(gd_attempt_overhead(m)),
                                    true,
                                );
                            }
                        }
                    }
                    let native_possible = native_needed;
                    let gd_possible = analytic_gd && gd_draws >= MIN_PILOT_DRAWS;
                    if !native_possible && !gd_possible {
                        break 'feasible_fallback;
                    }
                    let mut native_pilot = None;
                    let mut native_work = 0usize;
                    if native_possible {
                        proposal.set_order_score_mode("sampled");
                        let result = proposal.sample_order_pilot(
                            m,
                            PILOT_DRAWS,
                            pilot_seed,
                            native_allowance,
                        );
                        native_work = result.operations.units();
                        training_used = training_used.saturating_add(native_work);
                        setup_pilot_work = setup_pilot_work.saturating_add(native_work);
                        actual_total = actual_total.saturating_add(native_work);
                        candidate_legacy_work = candidate_legacy_work.saturating_add(result.work);
                        native_pilot = Some(result);
                    }
                    let mut gd_pilot = None;
                    let mut gd_work = 0usize;
                    if gd_possible {
                        proposal.set_order_score_mode("analytic_gd_cohort");
                        let gd_seed = derive(
                            request_seed,
                            &format!(
                                "rare-tail-feasible_cohorts-pilot-{}-{}-gd",
                                m.ids[cell.team], cell.rank
                            ),
                        );
                        let result =
                            proposal.sample_order_pilot(m, gd_draws, gd_seed, gd_allowance);
                        gd_work = result.operations.units();
                        training_used = training_used.saturating_add(gd_work);
                        setup_pilot_work = setup_pilot_work.saturating_add(gd_work);
                        actual_total = actual_total.saturating_add(gd_work);
                        candidate_legacy_work = candidate_legacy_work.saturating_add(result.work);
                        gd_pilot = Some(result);
                    }
                    let native_usable = native_pilot.as_ref().is_some_and(|result| {
                        native_pilot_complete(result) && result.hits >= 3 && result.ess > 0.0
                    });
                    let gd_usable = gd_pilot.as_ref().is_some_and(|result| {
                        result.samples >= MIN_PILOT_DRAWS
                            && result.hits >= 3
                            && result.ess > 0.0
                            && result.score_integral.cohort_private_integrated > 0
                    });
                    let gd_cache_pressure = !positive_cell
                        && native_usable
                        && gd_pilot
                            .as_ref()
                            .is_some_and(gd_pilot_exceeds_final_cache_capacity);
                    let gd_predicted_final_paths = gd_pilot.as_ref().map(predicted_final_gd_paths);
                    let use_gd = if positive_cell {
                        gd_usable
                    } else {
                        gd_usable
                            && !gd_cache_pressure
                            && (!native_usable
                                || normalized_efficiency(gd_pilot.as_ref().unwrap())
                                    > normalized_efficiency(native_pilot.as_ref().unwrap()))
                    };
                    let pilot_selection_reason = if gd_cache_pressure {
                        "native_cache_pressure"
                    } else if positive_cell {
                        "weak_positive_requires_gd"
                    } else if use_gd {
                        "gd_pilot_efficiency"
                    } else {
                        "native_pilot_efficiency_or_unusable_gd"
                    };
                    let pilot = if use_gd {
                        proposal.set_order_score_mode("analytic_gd_cohort");
                        gd_pilot.expect("usable GD fallback pilot")
                    } else if native_usable {
                        proposal.set_order_score_mode("sampled");
                        native_pilot.expect("usable native fallback pilot")
                    } else {
                        break 'feasible_fallback;
                    };
                    let score = normalized_efficiency(&pilot);
                    emit(
                        log,
                        "rust_odds_order_fallback_arm",
                        json!({"team":m.ids[cell.team],"rank":cell.rank+1,
                        "method":"feasible_cohorts","setup_work":fallback_setup,
                        "pilot_work":native_work.saturating_add(gd_work),"pilot_mode":if use_gd {"analytic_gd_cohort"} else {"sampled"},"pilot_selection_reason":pilot_selection_reason,"gd_cache_pressure":gd_cache_pressure,"gd_predicted_final_paths":gd_predicted_final_paths,"gd_cache_path_cap":crate::score_integral::MAX_COHORT_GD_PATHS,"native_pilot_work":native_work,"gd_pilot_work":gd_work,"pilot_draws":pilot.samples,
                        "pilot_hits":pilot.hits,"pilot_ess":pilot.ess,
                        "efficiency_score":score,"feasible":feasible,
                        "fallback":fallback_report,"arm_grant":fallback_grant,
                        "training_cap":training_cap}),
                    );
                    if pilot.samples >= MIN_PILOT_DRAWS && pilot.hits >= 3 && pilot.ess > 0.0 {
                        let pilot_is_gd =
                            use_gd && pilot.score_integral.cohort_private_integrated > 0;
                        arms.push((0, proposal, pilot, 0.0, score, pilot_is_gd));
                    }
                } else {
                    emit(
                        log,
                        "rust_odds_order_fallback_declined",
                        json!({"team":m.ids[cell.team],"rank":cell.rank+1,
                        "fallback":fallback_report}),
                    );
                    training_used = training_used.saturating_add(fallback_setup);
                    setup_pilot_work = setup_pilot_work.saturating_add(fallback_setup);
                    actual_total = actual_total.saturating_add(fallback_setup);
                }
            } else {
                emit(
                    log,
                    "rust_odds_order_fallback_declined",
                    json!({"team":m.ids[cell.team],"rank":cell.rank+1,
                    "fallback":fallback_report}),
                );
            }
        }
        if training_used > training_cap {
            candidate_overrun = true;
        }
        legacy_sample_work = legacy_sample_work.saturating_add(candidate_legacy_work);
        if candidate_overrun {
            overrun = true;
            break;
        }
        if arms.is_empty() {
            continue;
        }
        arms.sort_by(|a, b| arm_order(a.4, a.0, b.4, b.0));
        let (winner_width, mut proposal, pilot, pilot_ms, score, pilot_is_gd) = arms.remove(0);
        // A bounded GD pilot that never integrated private paths should not
        // move the final proposal's random stream away from the native path.
        let analytic_gd = pilot_is_gd && pilot.score_integral.cohort_private_integrated > 0;
        if !analytic_gd {
            proposal.set_order_score_mode("sampled");
        }
        let winner_method = if winner_width == 0 {
            "feasible_cohorts"
        } else {
            "expected_cohorts"
        };
        let cold_score_prep_per_stream = if analytic_gd {
            crate::score_integral::MAX_WORK
        } else {
            0
        };
        let native_upper = proposal.order_native_draw_upper_work(m).max(1);
        let goal_per_draw = if goals {
            crate::goal_tilt::JOINT_PREPARATION_ALLOWANCE_PER_DRAW
        } else {
            0
        };
        let final_per_draw = if analytic_gd {
            final_forecast_per_draw(analytic_recurring_per_draw(&pilot)).max(
                native_upper
                    .saturating_add(goal_per_draw)
                    .saturating_add(gd_attempt_overhead(m)),
            )
        } else {
            proposal.sample_work_upper_bound(m, 1)
        };
        let available_final = initial_capacity.saturating_sub(actual_total);
        let next_setup_reserve = cells.get(cell_index + 1).map_or(0, |next| {
            LazyJoint::order_base_constructor_and_clone_allowance(m, *next).saturating_add(
                MIN_PILOT_DRAWS.saturating_mul(draw_upper.saturating_add(if goals {
                    crate::goal_tilt::JOINT_PREPARATION_ALLOWANCE_PER_DRAW
                } else {
                    0
                })),
            )
        });
        let available_for_pair = available_final.saturating_sub(next_setup_reserve);
        let final_remaining = if analytic_gd {
            let desired = final_per_draw
                .saturating_mul(2)
                .saturating_mul((2 * MIN_FINAL_DRAWS).min(4_000))
                .saturating_add(cold_score_prep_per_stream.saturating_mul(2));
            desired.min(available_for_pair)
        } else {
            let minimum_pair = proposal
                .sample_work_upper_bound(m, MIN_FINAL_DRAWS)
                .saturating_mul(2);
            let desired = initial_capacity
                .saturating_mul(NATIVE_FINAL_CAPACITY_NUMERATOR)
                .checked_div(NATIVE_FINAL_CAPACITY_DENOMINATOR)
                .unwrap_or(0)
                .max(minimum_pair);
            desired.min(available_for_pair)
        };
        let allocation = if analytic_gd {
            fixed_pair_allocation_with_setup(
                final_remaining,
                final_per_draw,
                cold_score_prep_per_stream,
            )
        } else {
            let draws = native_final_draw_target(&pilot, final_remaining);
            let minimum_pair = proposal
                .sample_work_upper_bound(m, MIN_FINAL_DRAWS)
                .saturating_mul(2);
            (final_remaining >= minimum_pair).then_some((draws, final_remaining, 0))
        };
        let Some((draws, pair_forecast, query_per_stream)) = allocation else {
            emit(
                log,
                "rust_odds_expected_cohorts_skip",
                json!({"team":m.ids[cell.team],"rank":cell.rank+1,"reason":"final_pair_budget","training_work":training_used,"remaining_cell_grant":final_remaining,"per_draw_forecast":final_per_draw}),
            );
            continue;
        };
        // Main and check use distinct, fixed streams with the same requested
        // count. Bounded native streams may stop early; only complete pairs
        // are eligible for publication.
        let score_integral_snapshot = if analytic_gd {
            pilot.score_integral_snapshot.clone()
        } else {
            None
        };
        let pair = crate::search::parallel(2, workers, |index| {
            let stream = if winner_width >= 2 {
                if index == 0 {
                    "rare-tail-expected-cohort-main"
                } else {
                    "rare-tail-expected-cohort-check"
                }
            } else if index == 0 {
                "rare-tail-feasible-cohort-main"
            } else {
                "rare-tail-feasible-cohort-check"
            };
            let seed = derive(
                request_seed,
                &format!("{}-{}-{}", stream, m.ids[cell.team], cell.rank),
            );
            if analytic_gd {
                proposal.sample_order_limited_with_snapshot(
                    m,
                    draws,
                    seed,
                    pair_forecast / 2,
                    score_integral_snapshot.clone(),
                )
            } else {
                proposal.sample_order_native_bounded(m, draws, seed, pair_forecast / 2)
            }
        });
        let mut pair = pair.into_iter();
        let main = pair.next().expect("parallel main pair job");
        let check = pair.next().expect("parallel check pair job");
        let actual_pair = main
            .operations
            .units()
            .saturating_add(check.operations.units());
        let pair_legacy = main.work.saturating_add(check.work);
        let pair_is_complete =
            pair_is_publishable(&main, &check, draws, actual_pair, pair_forecast);
        let quality_ok = pair_is_complete && super::accepted(&main, &check, rough);
        let estimate = &mut estimates[cell.index(m.n)];
        let old_probability = estimate.probability;
        if !analytic_gd && pair_is_complete {
            native_evidence[cell.index(m.n)] = [
                NativeEvidence::from_result(&main),
                NativeEvidence::from_result(&check),
            ];
        }
        let disagreement = if analytic_gd && old_probability == 0.0 && quality_ok {
            native_gd_disagreement(&native_evidence[cell.index(m.n)], &main, &check)
        } else {
            None
        };
        let ok = quality_ok && disagreement.is_none();
        let candidate_kind = if old_probability > 0.0 {
            "weak_positive_refinement"
        } else {
            "zero_recovery"
        };
        let committed = if old_probability == 0. {
            if !estimate.reachability.starts_with("impossible") && ok {
                let design = match winner_method {
                    "feasible_cohorts" if analytic_gd => {
                        "matched_point_pool_feasible_cohorts_gd_cohort"
                    }
                    "feasible_cohorts" => "matched_point_pool_feasible_cohorts",
                    _ if analytic_gd => "matched_point_pool_expected_cohorts_gd_cohort",
                    _ => "matched_point_pool_expected_cohorts",
                };
                apply(estimate, &main, design);
                accepted_count += 1;
                true
            } else {
                false
            }
        } else if analytic_gd && should_replace_positive(estimate, &main, &check, ok) {
            let design = match winner_method {
                "feasible_cohorts" => "matched_point_pool_feasible_cohorts_gd_refinement",
                _ => "matched_point_pool_expected_cohorts_gd_refinement",
            };
            apply(estimate, &main, design);
            accepted_count += 1;
            accepted_weak_positive_count += 1;
            true
        } else {
            false
        };
        selected_count += 1;
        forecast_pair_work = forecast_pair_work.saturating_add(pair_forecast);
        actual_final_work = actual_final_work.saturating_add(actual_pair);
        actual_total = actual_total.saturating_add(actual_pair);
        legacy_sample_work = legacy_sample_work.saturating_add(pair_legacy);
        let pair_overrun = actual_pair > pair_forecast;
        overrun |= pair_overrun;
        if analytic_gd {
            let stats = |result: &Result| {
                json!({
                    "calls":result.score_integral.calls,"fallbacks":result.score_integral.fallbacks,
                    "private_eligible":result.score_integral.cohort_private_eligible,
                    "private_integrated":result.score_integral.cohort_private_integrated,
                    "shared_sampled":result.score_integral.cohort_shared_sampled,
                    "query_work":result.score_integral.query_work,"query_reserved":result.score_integral.query_work_reserved,
                    "prep_work":result.score_integral.prep_work,"cache_paths":result.score_integral.cache_paths,
                    "index_cells":result.score_integral.index_cells,"integrated_max":result.score_integral.max_integrated_fixtures
                })
            };
            emit(
                log,
                "rust_odds_score_cohort_gd_pair",
                json!({
                    "team":m.ids[cell.team],"rank":cell.rank+1,"mode":"analytic_gd_cohort",
                    "main":stats(&main),"check":stats(&check),"cold_setup_per_stream":cold_score_prep_per_stream,
                    "forecast_pair_work":pair_forecast,"query_allowance_per_stream":query_per_stream,"actual_pair_work":actual_pair
                }),
            );
        }
        let replacement_reason = if committed {
            "both_streams_improved"
        } else if disagreement.is_some() {
            "native_gd_disagreement"
        } else if old_probability > 0.0 && !ok {
            "failed_independent_publication_gate"
        } else if old_probability > 0.0 {
            "did_not_improve_both_precision_and_ess"
        } else {
            "zero_recovery_not_accepted"
        };
        emit(
            log,
            "rust_odds_expected_cohorts_pair",
            json!({"team":m.ids[cell.team],"rank":cell.rank+1,"stage":pass_name,"method":winner_method,"score_mode":if analytic_gd {"analytic_gd_cohort"} else {"sampled"},"candidate_kind":candidate_kind,"old_probability":if old_probability>0.0 {Some(old_probability)} else {None},"replacement_reason":replacement_reason,"native_gd_disagreement":disagreement.map(|d|json!({"native_lower_two_se":d.native_lower,"gd_upper_two_se":d.gd_upper})),"width":winner_width,"pilot_hits":pilot.hits,"pilot_ess":pilot.ess,"efficiency_score":score,"requested_draws":draws,"execution_cap":pair_forecast,"main_draws":main.samples,"main_hits":main.hits,"main_ess":main.ess,"check_draws":check.samples,"check_hits":check.hits,"check_ess":check.ess,"probability":main.probability,"check_probability":check.probability,"accepted":committed,"publication_gate":ok,"forecast_pair_work":pair_forecast,"actual_pair_work":actual_pair,"overrun":pair_overrun,"setup_ms":stage_ms,"pilot_ms":pilot_ms}),
        );
        if actual_total > initial_capacity {
            break;
        }
    }
    emit(
        log,
        "rust_odds_expected_cohorts_pass_summary",
        json!({"stage":pass_name,"candidate_cells":cells.len(),"selected":selected_count,"accepted":accepted_count,"reserve":initial_capacity,"setup_pilot_work":setup_pilot_work,"actual_work":actual_total,"actual_final_work":actual_final_work,"forecast_pair_work":forecast_pair_work,"overrun":overrun,"capacity_exceeded":actual_total>initial_capacity}),
    );
    PassTotals {
        accepted: accepted_count,
        accepted_weak_positives: accepted_weak_positive_count,
        legacy_sample_work,
        actual_work: actual_total,
        setup_pilot_work,
        actual_final_work,
        forecast_pair_work,
        selected: selected_count,
        overrun,
    }
}

fn one_arm_minimum(constructor_clone: usize, draw_upper: usize, goal_preparation: usize) -> usize {
    arm_training_minimum(constructor_clone, draw_upper, goal_preparation)
}

fn native_fallback_pilot_minimum(draw_upper: usize, goals: bool) -> usize {
    MIN_PILOT_DRAWS
        .saturating_mul(draw_upper)
        .saturating_add(if goals {
            PILOT_DRAWS.saturating_mul(crate::goal_tilt::JOINT_PREPARATION_ALLOWANCE_PER_DRAW)
        } else {
            0
        })
}

fn arm_training_minimum(clone_work: usize, draw_upper: usize, goal_preparation: usize) -> usize {
    clone_work
        .saturating_add(MAX_ARM_NODE_GUIDE_WORK)
        .saturating_add(MAX_SELECTION_WORK)
        .saturating_add(draw_upper)
        .saturating_add(goal_preparation)
}

fn final_forecast_per_draw(pilot_per_draw: usize) -> usize {
    pilot_per_draw.saturating_mul(5).div_ceil(4).max(1)
}

fn native_final_draw_target(pilot: &Result, execution_cap: usize) -> usize {
    let pair_per_draw = final_forecast_per_draw(budget::per_draw(pilot)).saturating_mul(2);
    (execution_cap / pair_per_draw.max(1)).clamp(MIN_FINAL_DRAWS, MAX_FINAL_DRAWS)
}

fn bounded_pilot_draws(allowance: usize, per_draw: usize, analytic_gd: bool) -> usize {
    let draw_budget = if analytic_gd {
        allowance.saturating_mul(4) / 5
    } else {
        allowance
    };
    (draw_budget / per_draw.max(1)).min(PILOT_DRAWS)
}

fn gd_attempt_overhead(m: &Model) -> usize {
    m.fixtures
        .len()
        .saturating_mul(3)
        .saturating_add(m.n.saturating_mul(8))
}

fn analytic_recurring_per_draw(pilot: &Result) -> usize {
    pilot
        .operations
        .units()
        .saturating_sub(pilot.score_integral.prep_work)
        .div_ceil(pilot.samples.max(1))
        .max(1)
}

fn predicted_final_gd_paths(pilot: &Result) -> usize {
    if pilot.samples == 0 {
        return usize::MAX;
    }
    pilot
        .score_integral
        .cache_paths
        .saturating_mul(MIN_FINAL_DRAWS)
        .saturating_add(pilot.samples - 1)
        / pilot.samples
}

fn gd_pilot_exceeds_final_cache_capacity(pilot: &Result) -> bool {
    predicted_final_gd_paths(pilot) > crate::score_integral::MAX_COHORT_GD_PATHS
}

#[cfg(test)]
fn fixed_pair_allocation(remaining: usize, per_draw: usize) -> Option<(usize, usize)> {
    let pair_cost = per_draw.saturating_mul(2);
    let draws = (remaining / pair_cost.max(1)).min(MAX_FINAL_DRAWS);
    if draws < MIN_FINAL_DRAWS {
        None
    } else {
        Some((draws, draws.saturating_mul(pair_cost)))
    }
}

fn fixed_pair_allocation_with_setup(
    remaining: usize,
    recurring_per_draw: usize,
    cold_setup_per_stream: usize,
) -> Option<(usize, usize, usize)> {
    let setup_pair = cold_setup_per_stream.saturating_mul(2);
    let recurring_pair_per_draw = recurring_per_draw.max(1).saturating_mul(2);
    let minimum_draw_reserve = recurring_pair_per_draw.saturating_mul(MIN_FINAL_DRAWS);
    let slack = remaining.saturating_sub(setup_pair.saturating_add(minimum_draw_reserve));
    // Hold half of post-floor slack for GD query work and leave the other half
    // available for more fixed native draws. The per-stream sampler divides
    // its slack between capped preparation and queries before using RNG.
    let query_per_stream = slack / 4;
    let recurring_budget = remaining
        .saturating_sub(setup_pair)
        .saturating_sub(query_per_stream.saturating_mul(2));
    let draws = (recurring_budget / recurring_pair_per_draw).min(MAX_FINAL_DRAWS);
    if draws < MIN_FINAL_DRAWS {
        return None;
    }
    Some((
        draws,
        setup_pair
            .saturating_add(query_per_stream.saturating_mul(2))
            .saturating_add(draws.saturating_mul(recurring_pair_per_draw)),
        query_per_stream,
    ))
}

fn arm_order(a_score: f64, a_width: usize, b_score: f64, b_width: usize) -> std::cmp::Ordering {
    b_score.total_cmp(&a_score).then(a_width.cmp(&b_width))
}

fn normalized_efficiency(result: &Result) -> f64 {
    if result.samples == 0 {
        return 0.0;
    }
    result.ess * 1000.0 / result.operations.units().max(1) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn estimate(probability: f64, conditional_samples: usize) -> Estimate {
        Estimate {
            probability,
            std_err: probability * 0.8,
            ess: 2.0,
            meets_precision_goal: false,
            conditional_samples,
            max_event_weight_share: 0.2,
            reachability: "reachable".into(),
            ..Default::default()
        }
    }

    #[test]
    fn native_fallback_reserves_full_goal_preparation_separately_from_draw_floor() {
        let draw_upper = 12_345;
        let preparation = crate::goal_tilt::JOINT_PREPARATION_ALLOWANCE_PER_DRAW;
        assert_eq!(
            native_fallback_pilot_minimum(draw_upper, true),
            MIN_PILOT_DRAWS * draw_upper + PILOT_DRAWS * preparation
        );
        assert_eq!(
            native_fallback_pilot_minimum(draw_upper, false),
            MIN_PILOT_DRAWS * draw_upper
        );
    }

    #[test]
    fn gd_cache_pressure_projects_final_working_set_from_pilot_rate() {
        let mut pilot = Result {
            samples: 162,
            ..Default::default()
        };
        pilot.score_integral.cache_paths = 152;
        assert_eq!(predicted_final_gd_paths(&pilot), 939);
        assert!(gd_pilot_exceeds_final_cache_capacity(&pilot));

        pilot.samples = 800;
        pilot.score_integral.cache_paths = 12;
        assert_eq!(predicted_final_gd_paths(&pilot), 15);
        assert!(!gd_pilot_exceeds_final_cache_capacity(&pilot));

        pilot.samples = 0;
        assert_eq!(predicted_final_gd_paths(&pilot), usize::MAX);
        assert!(gd_pilot_exceeds_final_cache_capacity(&pilot));
    }

    #[test]
    fn weak_positive_selection_excludes_impossible_nonconditional_and_strong_cells() {
        let mut cells = vec![
            estimate(0.0, 0),
            estimate(0.0, 0),
            estimate(0.001, 20),
            estimate(0.1, 20),
        ];
        cells[1].reachability = "impossible_bounds".into();
        cells[3].std_err = 0.001;
        cells[3].ess = 1_000.0;
        cells[3].meets_precision_goal = true;
        cells[3].max_event_weight_share = 0.01;
        let mut no_conditional = estimate(0.0001, 0);
        no_conditional.meets_precision_goal = false;
        cells.push(no_conditional);

        assert!(weak_positive(&cells[2]));
        assert!(!weak_positive(&cells[1]));
        let mut impossible_positive = cells[2].clone();
        impossible_positive.reachability = "impossible_proof".into();
        assert!(!weak_positive(&impossible_positive));
        assert!(!weak_positive(&cells[3]));
        assert!(!weak_positive(&cells[4]));
        assert_eq!(
            candidate_cells(&cells, 5, true, &[0, 1, 2, 3, 4], 17),
            vec![Cell { team: 0, rank: 0 }, Cell { team: 0, rank: 2 }]
        );
        assert_eq!(
            candidate_cells(&cells, 5, false, &[0, 1, 2, 3, 4], 17),
            vec![Cell { team: 0, rank: 0 }]
        );
    }

    #[test]
    fn weak_positive_cells_sort_rarest_first_after_zero_recovery_cells() {
        let values = vec![
            Estimate {
                probability: 0.0,
                reachability: "reachable".into(),
                ..Default::default()
            },
            estimate(0.01, 8),
            estimate(0.0001, 8),
            estimate(0.01, 8),
        ];
        assert_eq!(
            candidate_cells(&values, 2, true, &[11, 22], 8),
            vec![
                Cell { team: 0, rank: 0 },
                Cell { team: 1, rank: 0 },
                Cell { team: 0, rank: 1 },
                Cell { team: 1, rank: 1 },
            ]
        );
    }

    #[test]
    fn candidate_cap_spreads_zero_cells_across_teams_before_truncating() {
        let values = vec![
            Estimate {
                probability: 0.0,
                reachability: "reachable".into(),
                ..Default::default()
            };
            144
        ];
        let selected = candidate_cells(
            &values,
            12,
            false,
            &[10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120],
            123,
        );
        assert_eq!(selected.len(), MAX_CELLS);
        let teams: std::collections::BTreeSet<_> = selected.iter().map(|cell| cell.team).collect();
        assert_eq!(teams.len(), 12);
        assert!(selected.iter().filter(|cell| cell.team == 0).count() <= 11);
    }

    #[test]
    fn feasible_fallback_requires_empty_or_all_weak_expected_pilots() {
        let healthy = Result {
            probability: 0.01,
            std_err: 0.002,
            ess: 30.0,
            max_share: 0.05,
            ..Default::default()
        };
        assert!(pilot_is_healthy(&healthy));
        assert_eq!(
            feasible_fallback_trigger(&[], false),
            Some("empty_expected_arms")
        );
        assert_eq!(
            feasible_fallback_trigger(&[], true),
            Some("empty_expected_arms")
        );

        let weak = Result {
            ess: 24.9,
            ..healthy.clone()
        };
        assert!(!pilot_is_healthy(&weak));
        assert_eq!(
            feasible_fallback_trigger(&[false, false], true),
            Some("all_expected_pilots_weak")
        );
        assert_eq!(feasible_fallback_trigger(&[false, false], false), None);
        assert_eq!(feasible_fallback_trigger(&[false, true], true), None);
        assert_eq!(feasible_fallback_trigger(&[true], true), None);

        assert!(!pilot_is_healthy(&Result {
            probability: 0.0,
            ..healthy.clone()
        }));
        assert!(!pilot_is_healthy(&Result {
            std_err: f64::NAN,
            ..healthy.clone()
        }));
        assert!(!pilot_is_healthy(&Result {
            max_share: f64::INFINITY,
            ..healthy
        }));
    }

    #[test]
    fn incomplete_native_pilot_cannot_win_on_prefix_ess() {
        let complete = Result {
            samples: PILOT_DRAWS,
            ess: 1.3,
            ..Default::default()
        };
        let incomplete_prefix_with_high_ess = Result {
            samples: 863,
            ess: 10.11,
            ..Default::default()
        };
        assert!(native_pilot_complete(&complete));
        assert!(!native_pilot_complete(&incomplete_prefix_with_high_ess));
        // A partial native result is excluded before efficiency comparison,
        // regardless of an apparently favorable prefix ESS.
        let eligible = [complete.clone(), incomplete_prefix_with_high_ess]
            .into_iter()
            .filter(native_pilot_complete)
            .collect::<Vec<_>>();
        assert_eq!(eligible.len(), 1);
        assert_eq!(eligible[0].ess, 1.3);
    }

    #[test]
    fn native_gd_disagreement_uses_qualified_two_se_intervals() {
        let native = NativeEvidence::from_result(&Result {
            probability: 10.0,
            std_err: 1.0,
            hits: 30,
            ess: 3.0,
            ..Default::default()
        });
        assert!(native.is_some());
        let native = [native, None];
        let separated = native_gd_disagreement(
            &native,
            &Result {
                probability: 1.0,
                std_err: 1.0,
                ..Default::default()
            },
            &Result {
                probability: 2.0,
                std_err: 1.0,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(separated.native_lower, 8.0);
        assert_eq!(separated.gd_upper, 4.0);

        let overlapping = [
            NativeEvidence::from_result(&Result {
                probability: 10.0,
                std_err: 3.0,
                hits: 30,
                ess: 3.0,
                ..Default::default()
            }),
            None,
        ];
        assert!(native_gd_disagreement(
            &overlapping,
            &Result {
                probability: 1.0,
                std_err: 5.0,
                ..Default::default()
            },
            &Result {
                probability: 1.0,
                std_err: 5.0,
                ..Default::default()
            },
        )
        .is_none());
        assert!(native_gd_disagreement(
            &[None, None],
            &Result {
                probability: 1.0,
                std_err: 0.1,
                ..Default::default()
            },
            &Result {
                probability: 1.0,
                std_err: 0.1,
                ..Default::default()
            },
        )
        .is_none());
    }

    #[test]
    fn cost_admission_skips_expensive_cells_and_recycles_unused_work() {
        let bank = 1_000;
        let expensive = admit_candidate(bank, 1_200);
        assert_eq!(expensive, None);

        let first_grant = admit_candidate(bank, 400).unwrap();
        let free_while_reserved = bank - first_grant;
        assert_eq!(admit_candidate(free_while_reserved, 700), None);

        let actual_spend = 250;
        let unused_refund = first_grant - actual_spend;
        let recycled = free_while_reserved + unused_refund;
        assert_eq!(recycled, bank - actual_spend);
        assert_eq!(admit_candidate(recycled, 700), Some(700));
        assert_eq!(admit_candidate(recycled, 500), Some(500));
    }

    #[test]
    fn native_final_target_uses_execution_ceiling_with_a_floor_and_hard_cap() {
        let healthy = Result {
            samples: 1_024,
            ..Default::default()
        };
        assert_eq!(native_final_draw_target(&healthy, 2_000), MIN_FINAL_DRAWS);
        assert_eq!(native_final_draw_target(&healthy, 8_000), 2_000);
        assert_eq!(
            native_final_draw_target(&healthy, usize::MAX),
            MAX_FINAL_DRAWS
        );
    }

    #[test]
    fn positive_replacement_requires_two_finite_precision_improvements() {
        let old = Estimate {
            probability: 1e-8,
            std_err: 0.5e-8,
            ess: 15.0,
            conditional_samples: 500,
            ..Default::default()
        };
        let result = |probability, ess, rse| Result {
            probability,
            std_err: probability * rse,
            ess,
            ..Default::default()
        };
        let good_main = result(1e-8, 30.0, 0.2);
        let good_check = result(1.1e-8, 25.0, 0.3);
        assert!(refinement_is_better(&old, &good_main, &good_check));
        assert!(!refinement_is_better(
            &old,
            &good_main,
            &result(0.0, 100.0, 0.01)
        ));
        assert!(!refinement_is_better(
            &old,
            &good_main,
            &result(1e-8, 40.0, 0.6)
        ));
        assert!(!refinement_is_better(
            &old,
            &result(f64::NAN, 100.0, 0.01),
            &good_check
        ));
    }

    #[test]
    fn failed_publication_gate_preserves_all_existing_estimate_fields() {
        let old = Estimate {
            probability: 1e-8,
            std_err: 0.5e-8,
            ess: 15.0,
            samples: 300,
            hits: 7,
            work_spent: 9876,
            design: "old_positive_design".into(),
            conditional_samples: 200,
            ..Default::default()
        };
        let before = serde_json::to_value(&old).unwrap();
        let main = Result {
            probability: 1e-8,
            std_err: 0.1e-8,
            ess: 30.0,
            ..Default::default()
        };
        let check = Result {
            probability: 1.1e-8,
            std_err: 0.2e-8,
            ess: 25.0,
            ..Default::default()
        };
        assert!(!should_replace_positive(&old, &main, &check, false));
        assert_eq!(serde_json::to_value(&old).unwrap(), before);
        assert!(!should_replace_positive(
            &old,
            &main,
            &Result {
                ess: 9.0,
                ..check.clone()
            },
            true
        ));
        assert_eq!(serde_json::to_value(&old).unwrap(), before);
    }

    #[test]
    fn truncated_or_over_budget_pairs_cannot_be_published() {
        let complete = Result {
            samples: 1_000,
            ..Default::default()
        };
        let truncated = Result {
            samples: 999,
            ..complete.clone()
        };
        assert!(pair_is_publishable(&complete, &complete, 1_000, 100, 100));
        assert!(!pair_is_publishable(&truncated, &complete, 1_000, 100, 100));
        assert!(!pair_is_publishable(&complete, &truncated, 1_000, 100, 100));
        assert!(!pair_is_publishable(&complete, &complete, 1_000, 101, 100));

        let old = estimate(1e-8, 100);
        let before = serde_json::to_value(&old).unwrap();
        let publish = pair_is_publishable(&truncated, &complete, 1_000, 100, 100)
            && crate::rare_tail::accepted(&truncated, &complete, false);
        if publish {
            panic!("truncated pair unexpectedly passed its publication gate");
        }
        assert_eq!(serde_json::to_value(&old).unwrap(), before);
    }

    #[test]
    fn donor_baseline_is_restored_unless_temporary_tail_estimate_dominates_it() {
        let baseline = Estimate {
            probability: 1e-7,
            std_err: 0.5e-7,
            ess: 20.0,
            work_spent: 1234,
            design: "donor_baseline".into(),
            conditional_samples: 100,
            ..Default::default()
        };
        let mut estimates = vec![
            Estimate::default(),
            Estimate {
                probability: 0.9e-7,
                std_err: 0.6e-7,
                ess: 30.0,
                design: "weaker_tail_result".into(),
                ..Default::default()
            },
            Estimate {
                probability: 1.2e-7,
                std_err: 0.3e-7,
                ess: 25.0,
                design: "better_tail_result".into(),
                ..Default::default()
            },
        ];
        let (restored, kept) = restore_positive_baselines(
            &mut estimates,
            &[
                (0, baseline.clone()),
                (1, baseline.clone()),
                (2, baseline.clone()),
            ],
        );
        assert_eq!((restored, kept), (2, 1));
        assert_eq!(estimates[0].design, "donor_baseline");
        assert_eq!(estimates[1].design, "donor_baseline");
        assert_eq!(estimates[2].design, "better_tail_result");
    }

    #[test]
    fn invalid_or_missing_relative_error_is_conservatively_weak() {
        let mut value = estimate(0.01, 1);
        value.std_err = f64::NAN;
        assert!(weak_positive(&value));
        assert_eq!(relative_se(&value), f64::INFINITY);
    }

    #[test]
    fn cohort_gd_defaults_only_for_coverage_and_sampled_is_an_explicit_rollback() {
        assert_eq!(resolve_cohort_score_mode(None, true), "analytic_gd_cohort");
        assert_eq!(resolve_cohort_score_mode(None, false), "sampled");
        assert_eq!(resolve_cohort_score_mode(Some("sampled"), true), "sampled");
        assert_eq!(resolve_cohort_score_mode(Some("0"), true), "sampled");
        assert_eq!(
            resolve_cohort_score_mode(Some("analytic_gd_cohort"), false),
            "analytic_gd_cohort"
        );
        assert_eq!(resolve_cohort_score_mode(Some("typo"), true), "sampled");
    }

    #[test]
    fn setting_defaults_to_coverage_and_explicit_values_win() {
        assert!(resolve_enabled(None, true));
        assert!(!resolve_enabled(None, false));
        assert!(resolve_enabled(Some("1"), false));
        assert!(!resolve_enabled(Some("0"), true));
        assert!(!resolve_enabled(Some("bad"), true));
    }

    #[test]
    fn fixed_pairs_are_equal_capped_and_require_the_minimum() {
        assert_eq!(fixed_pair_allocation(20_000, 2), Some((5_000, 20_000)));
        assert_eq!(fixed_pair_allocation(1_999, 1), None);
        assert_eq!(fixed_pair_allocation(100_000, 1), Some((20_000, 40_000)));
    }

    #[test]
    fn gd_pair_reserves_native_floor_query_slack_and_never_exceeds_grant() {
        let (draws, forecast, query) =
            fixed_pair_allocation_with_setup(120_000, 10, 2_000).unwrap();
        assert!(draws >= MIN_FINAL_DRAWS);
        assert!(query > 0);
        assert!(forecast <= 120_000);
        assert_eq!(forecast % 2, 0);
        let (draws, forecast, query) = fixed_pair_allocation_with_setup(22_000, 10, 1_000).unwrap();
        assert_eq!(draws, MIN_FINAL_DRAWS);
        assert_eq!(query, 0);
        assert_eq!(forecast, 22_000);
        assert!(fixed_pair_allocation_with_setup(21_999, 10, 1_000).is_none());
    }

    #[test]
    fn gd_pilot_keeps_a_bounded_preparation_query_pool_inside_its_arm_grant() {
        let per_draw = 4_000;
        assert_eq!(bounded_pilot_draws(per_draw * 375, per_draw, true), 300);
        assert_eq!(bounded_pilot_draws(per_draw * 39, per_draw, true), 31);
        assert_eq!(
            bounded_pilot_draws(per_draw * PILOT_DRAWS, per_draw, false),
            PILOT_DRAWS
        );
    }

    #[test]
    fn final_forecast_adds_twenty_five_percent_headroom() {
        assert_eq!(final_forecast_per_draw(100), 125);
        assert_eq!(final_forecast_per_draw(101), 127);
    }

    #[test]
    fn width_four_wins_an_exact_score_tie() {
        assert_eq!(arm_order(1.25, 4, 1.25, 6), std::cmp::Ordering::Less);
        assert_eq!(arm_order(1.0, 6, 1.25, 4), std::cmp::Ordering::Greater);
    }

    #[test]
    fn cohort_score_normalizes_pilot_lengths_by_operations() {
        let native = Result {
            samples: 1000,
            ess: 20.0,
            operations: budget::Operations {
                fixtures: 100,
                ..Default::default()
            },
            ..Default::default()
        };
        let cohort = Result {
            samples: 512,
            ess: 10.2,
            operations: budget::Operations {
                fixtures: 51,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!((normalized_efficiency(&native) - normalized_efficiency(&cohort)).abs() < 0.01);
    }
}
