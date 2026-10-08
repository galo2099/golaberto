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

const MAX_CELLS: usize = 4;
const PILOT_DRAWS: usize = 1_024;
const MIN_PILOT_DRAWS: usize = 32;
const MAX_FINAL_DRAWS: usize = 20_000;
const MIN_FINAL_DRAWS: usize = 1_000;
const MAX_NODES: usize = 60_000;
const MAX_GUIDES: usize = 4_000_000;
const MAX_SELECTION_WORK: usize = 2_000_000;
const MAX_ARM_NODE_GUIDE_WORK: usize = 9_000_000;

pub(crate) fn enabled() -> bool {
    resolve_enabled(
        std::env::var("RUST_ODDS_EXPECTED_COHORTS").ok().as_deref(),
        crate::rare_tail::profile() == "coverage",
    )
}

fn resolve_enabled(setting: Option<&str>, coverage: bool) -> bool {
    match setting {
        Some("1") => true,
        Some("0") => false,
        Some(_) => false,
        None => coverage,
    }
}

pub(super) struct Outcome {
    pub(super) accepted: usize,
    pub(super) legacy_sample_work: u64,
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
) -> Outcome {
    let capacity = budget::capacity(m, budget::scaled(2 * budget::REFERENCE_DRAWS, 0.45));
    let rough = crate::rare_tail::value("RUST_ODDS_RARE_TAIL_QUALITY") == "order";
    let cells: Vec<_> = estimates
        .iter()
        .enumerate()
        .filter_map(|(index, estimate)| {
            (estimate.probability == 0. && !estimate.reachability.starts_with("impossible"))
                .then_some(Cell {
                    team: index / m.n,
                    rank: index % m.n,
                })
        })
        .take(MAX_CELLS)
        .collect();
    let initial_capacity = capacity;
    let mut actual_total = 0usize;
    let mut setup_pilot_work = 0usize;
    let mut actual_final_work = 0usize;
    let mut forecast_pair_work = 0usize;
    let mut legacy_sample_work = 0u64;
    let mut accepted_count = 0usize;
    let mut selected_count = 0usize;
    let mut overrun = false;
    let draw_upper = budget::ordered_draw_upper_cost(m).max(1);
    let goals = crate::rare_tail::value("RUST_ODDS_LAZY_GOALS") == "1"
        && crate::goal_tilt::GoalTilt::joint_guidance_may_fit_for_any_target(m);
    let goal_preparation = if goals {
        PILOT_DRAWS.saturating_mul(crate::goal_tilt::JOINT_PREPARATION_ALLOWANCE_PER_DRAW)
    } else {
        0
    };

    for (position, cell) in cells.iter().copied().enumerate() {
        let slots_left = cells.len() - position;
        let remaining = initial_capacity.saturating_sub(actual_total);
        let grant = remaining / slots_left.max(1);
        let training_cap = grant / 3;
        let constructor_clone = LazyJoint::order_base_constructor_and_clone_allowance(m, cell);
        let minimum_arm = one_arm_minimum(constructor_clone, draw_upper, goal_preparation);
        if training_cap < minimum_arm {
            emit(
                log,
                "rust_odds_expected_cohorts_skip",
                json!({"team":m.ids[cell.team],"rank":cell.rank+1,"reason":"training_preflight","cell_grant":grant,"training_cap":training_cap,"required":minimum_arm,"constructor_clone_allowance":constructor_clone,"draw_upper":draw_upper,"goal_preparation":goal_preparation}),
            );
            continue;
        }

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
            let pilot_seed = derive(
                request_seed,
                &format!(
                    "rare-tail-expected-cohort-pilot-{}-{}-width-{}",
                    m.ids[cell.team], cell.rank, width
                ),
            );
            let pilot_start = std::time::Instant::now();
            let pilot = proposal.sample_order_pilot(
                m,
                PILOT_DRAWS,
                pilot_seed,
                arm_grant.saturating_sub(setup_work),
            );
            let pilot_ms = pilot_start.elapsed().as_secs_f64() * 1000.0;
            stage_ms += pilot_ms;
            let pilot_work = pilot.operations.units();
            candidate_legacy_work = candidate_legacy_work.saturating_add(pilot.work);
            if training_used.saturating_add(pilot_work) > training_cap {
                training_used = training_used.saturating_add(pilot_work);
                setup_pilot_work = setup_pilot_work.saturating_add(pilot_work);
                actual_total = actual_total.saturating_add(pilot_work);
                candidate_overrun = true;
                emit(
                    log,
                    "rust_odds_expected_cohorts_arm",
                    json!({"team":m.ids[cell.team],"rank":cell.rank+1,"width":width,"status":"pilot_training_overrun","pilot_draws":pilot.samples,"pilot_work":pilot_work,"training_cap":training_cap}),
                );
                break;
            }
            training_used = training_used.saturating_add(pilot_work);
            setup_pilot_work = setup_pilot_work.saturating_add(pilot_work);
            actual_total = actual_total.saturating_add(pilot_work);
            let score = normalized_efficiency(&pilot);
            emit(
                log,
                "rust_odds_expected_cohorts_arm",
                json!({"team":m.ids[cell.team],"rank":cell.rank+1,"width":width,"setup_work":setup_work,"pilot_work":pilot_work,"pilot_draws":pilot.samples,"pilot_hits":pilot.hits,"pilot_ess":pilot.ess,"efficiency_score":score,"pilot_ms":pilot_ms,"cohorts":cohorts,"arm_grant":arm_grant,"training_cap":training_cap}),
            );
            if pilot.samples >= MIN_PILOT_DRAWS && pilot.hits >= 3 && pilot.ess > 0.0 {
                arms.push((width, proposal, pilot, pilot_ms, score));
            }
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
        let (winner_width, proposal, pilot, pilot_ms, score) = arms.remove(0);
        let final_per_draw = final_forecast_per_draw(budget::per_draw(&pilot));
        let final_remaining = grant.saturating_sub(training_used);
        let Some((draws, pair_forecast)) = fixed_pair_allocation(final_remaining, final_per_draw)
        else {
            emit(
                log,
                "rust_odds_expected_cohorts_skip",
                json!({"team":m.ids[cell.team],"rank":cell.rank+1,"reason":"final_pair_budget","training_work":training_used,"remaining_cell_grant":final_remaining,"per_draw_forecast":final_per_draw}),
            );
            continue;
        };
        // Main and check use distinct, fixed streams. Both always complete even
        // when MAIN fails the publication gate.
        let pair = crate::search::parallel(2, workers, |index| {
            let stream = if index == 0 {
                "rare-tail-expected-cohort-main"
            } else {
                "rare-tail-expected-cohort-check"
            };
            proposal.sample(
                m,
                draws,
                derive(
                    request_seed,
                    &format!("{}-{}-{}", stream, m.ids[cell.team], cell.rank),
                ),
            )
        });
        let mut pair = pair.into_iter();
        let main = pair.next().expect("parallel main pair job");
        let check = pair.next().expect("parallel check pair job");
        let actual_pair = main
            .operations
            .units()
            .saturating_add(check.operations.units());
        let pair_legacy = main.work.saturating_add(check.work);
        let ok = super::accepted(&main, &check, rough);
        let estimate = &mut estimates[cell.index(m.n)];
        let committed =
            if estimate.probability == 0. && !estimate.reachability.starts_with("impossible") && ok
            {
                apply(estimate, &main, "matched_point_pool_expected_cohorts");
                accepted_count += 1;
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
        emit(
            log,
            "rust_odds_expected_cohorts_pair",
            json!({"team":m.ids[cell.team],"rank":cell.rank+1,"width":winner_width,"pilot_hits":pilot.hits,"pilot_ess":pilot.ess,"efficiency_score":score,"main_draws":main.samples,"main_hits":main.hits,"main_ess":main.ess,"check_draws":check.samples,"check_hits":check.hits,"check_ess":check.ess,"probability":main.probability,"check_probability":check.probability,"accepted":committed,"publication_gate":ok,"forecast_pair_work":pair_forecast,"actual_pair_work":actual_pair,"overrun":pair_overrun,"setup_ms":stage_ms,"pilot_ms":pilot_ms}),
        );
        if actual_total > initial_capacity {
            break;
        }
    }
    let capacity_exceeded = actual_total > initial_capacity;
    emit(
        log,
        "rust_odds_expected_cohorts_summary",
        json!({"enabled":true,"budget_mode":budget::mode(),"candidate_cells":cells.len(),"selected":selected_count,"accepted":accepted_count,"reserve":initial_capacity,"forecast_capacity":initial_capacity,"setup_pilot_work":setup_pilot_work,"actual_work":actual_total,"actual_final_work":actual_final_work,"forecast_pair_work":forecast_pair_work,"overrun":overrun,"capacity_exceeded":capacity_exceeded}),
    );
    Outcome {
        accepted: accepted_count,
        legacy_sample_work,
    }
}

fn one_arm_minimum(constructor_clone: usize, draw_upper: usize, goal_preparation: usize) -> usize {
    arm_training_minimum(constructor_clone, draw_upper, goal_preparation)
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

fn fixed_pair_allocation(remaining: usize, per_draw: usize) -> Option<(usize, usize)> {
    let pair_cost = per_draw.saturating_mul(2);
    let draws = (remaining / pair_cost.max(1)).min(MAX_FINAL_DRAWS);
    if draws < MIN_FINAL_DRAWS {
        None
    } else {
        Some((draws, draws.saturating_mul(pair_cost)))
    }
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
