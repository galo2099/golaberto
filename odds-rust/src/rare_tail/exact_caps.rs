//! Bounded recovery for cells with an exactly enumerable points/wins cap event.
use crate::{
    conditioned::Result,
    joint_caps::exact::ExactCaps,
    logging::RequestLog,
    model::Model,
    pool::Estimate,
    rng::derive,
    search::{self, Cell},
};
use serde_json::{json, Value};

const DRAW_CAP: usize = 65_536;
const MIN_DRAW_COUNT: usize = 32_768;
const MAX_ATTEMPTS: usize = 8;
const STAGE_DRAWS: usize = 100_000;
const DESIGN: &str = "exact_caps_conditioned";

#[derive(Default)]
pub(crate) struct Outcome {
    pub accepted: usize,
    pub work: u64,
    pub bounds: Vec<(usize, f64, f64)>,
    pub log: Value,
}

#[derive(Clone)]
struct Candidate {
    cell: Cell,
    index: usize,
    probability: f64,
    priority: usize,
}

fn estimate_rse(estimate: &Estimate) -> f64 {
    estimate.relative_se.unwrap_or_else(|| {
        if estimate.probability > 0.0 {
            estimate.std_err / estimate.probability
        } else {
            f64::INFINITY
        }
    })
}

fn weak_estimate(estimate: &Estimate) -> bool {
    let relative_se = estimate_rse(estimate);
    !estimate.ess.is_finite()
        || estimate.ess < 0.0
        || !estimate.std_err.is_finite()
        || estimate.std_err < 0.0
        || !estimate.max_event_weight_share.is_finite()
        || estimate.max_event_weight_share < 0.0
        || !estimate.meets_precision_goal
        || estimate.ess < 25.0
        || !relative_se.is_finite()
        || relative_se < 0.0
        || relative_se > 0.25
        || estimate.max_event_weight_share > 0.1
}

fn rse(result: &Result) -> f64 {
    if result.probability > 0.0 {
        result.std_err / result.probability
    } else {
        f64::INFINITY
    }
}

fn finite_result(result: &Result) -> bool {
    result.probability.is_finite()
        && result.std_err.is_finite()
        && result.std_err >= 0.0
        && result.ess.is_finite()
        && result.max_share.is_finite()
        && rse(result).is_finite()
}

fn certified_underestimate(old_probability: f64, floor: f64) -> bool {
    old_probability > 0.0 && floor > 0.0 && old_probability < floor * (1.0 - 1e-10)
}

fn draws_per_stream(remaining: usize, per_draw: usize) -> usize {
    DRAW_CAP.min(remaining / per_draw.max(1) / 2)
}

fn improves_positive(
    old: &Estimate,
    main: &Result,
    check: &Result,
    certified_underestimate: bool,
) -> bool {
    certified_underestimate
        || (main.ess > old.ess
            && check.ess > old.ess
            && rse(main) < estimate_rse(old)
            && rse(check) < estimate_rse(old))
}

fn within_exact_bounds(value: f64, floor: f64, upper: f64) -> bool {
    if !value.is_finite() || !floor.is_finite() || !upper.is_finite() || floor > upper {
        return false;
    }
    let tolerance = 1e-10 * floor.abs().max(upper.abs()).max(f64::MIN_POSITIVE);
    value >= floor - tolerance && value <= upper + tolerance
}

fn resolve_enabled(
    explicit: Option<&str>,
    profile: &str,
    modeled: bool,
    conditioned_zero_enabled: bool,
) -> bool {
    modeled
        && conditioned_zero_enabled
        && explicit != Some("0")
        && (explicit == Some("1") || profile == "coverage")
}

fn enabled() -> bool {
    resolve_enabled(
        std::env::var("RUST_ODDS_EXACT_CAP_RECOVERY")
            .ok()
            .as_deref(),
        &super::profile(),
        super::budget::modeled(),
        search::enabled("RARE_POSITION_CONDITIONED_ZERO"),
    )
}

pub(crate) fn run(
    model: &Model,
    request_seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    log: Option<&RequestLog>,
) -> Outcome {
    let started = std::time::Instant::now();
    let capacity = super::budget::capacity(model, STAGE_DRAWS);
    let mut outcome = Outcome::default();
    let mut reason = if enabled() {
        "no_eligible_cells"
    } else {
        "disabled_or_unmodeled"
    };
    let mut scan_work = 0usize;
    let mut scan_reservation = 0usize;
    let mut setup_work = 0usize;
    let mut final_work = 0usize;
    let mut reserved_work = 0usize;
    let mut attempts = 0usize;
    let mut certified_underestimates = 0usize;
    let mut events = Vec::<Value>::new();

    if enabled() && estimates.len() == model.n.saturating_mul(model.n) {
        let scan_cost = 16usize
            .saturating_mul(model.n)
            .saturating_mul(model.n)
            .saturating_mul(model.n.saturating_add(model.fixtures.len()));
        scan_reservation = scan_cost;
        if scan_cost > capacity {
            reason = "scan_budget_refusal";
        } else {
            scan_work = scan_cost;
            let mut candidates = Vec::<Candidate>::new();
            for team in 0..model.n {
                for rank in 0..model.n {
                    let cell = Cell { team, rank };
                    let index = cell.index(model.n);
                    let old = &estimates[index];
                    if old.reachability.starts_with("impossible")
                        || !ExactCaps::eligible(model, cell)
                    {
                        continue;
                    }
                    if old.probability == 0.0 {
                        candidates.push(Candidate {
                            cell,
                            index,
                            probability: 0.0,
                            priority: 0,
                        });
                    } else if old.probability <= 1e-8 {
                        candidates.push(Candidate {
                            cell,
                            index,
                            probability: old.probability,
                            priority: 1,
                        });
                    }
                }
            }
            candidates.sort_by(|a, b| {
                a.priority
                    .cmp(&b.priority)
                    .then(a.probability.total_cmp(&b.probability))
                    .then(model.ids[a.cell.team].cmp(&model.ids[b.cell.team]))
                    .then(a.cell.rank.cmp(&b.cell.rank))
            });
            if !candidates.is_empty() {
                reason = "attempts_exhausted";
            }
            let mut remaining = capacity.saturating_sub(scan_work);
            let mut published = false;
            for candidate in candidates.into_iter().take(MAX_ATTEMPTS) {
                if published {
                    break;
                }
                let old = estimates[candidate.index].clone();
                let grant = remaining.min(capacity / 3);
                attempts += 1;
                let build = ExactCaps::build(model, candidate.cell, grant);
                setup_work = setup_work.saturating_add(build.work);
                remaining = remaining.saturating_sub(build.work);
                if build.work > grant || scan_work.saturating_add(setup_work) > capacity {
                    events.push(json!({"team":model.ids[candidate.cell.team],"rank":candidate.cell.rank+1,"status":"build_budget_refusal","reason":"build_work_exceeded_grant","build_work":build.work,"grant":grant}));
                    reason = "build_work_exceeded_grant";
                    break;
                }
                let Some(plan) = build.plan else {
                    events.push(json!({"team":model.ids[candidate.cell.team],"rank":candidate.cell.rank+1,"status":"build_declined","reason":build.reason,"build_work":build.work,"grant":grant}));
                    reason = build.reason;
                    continue;
                };
                let floor = plan.guaranteed_mass().max(0.0);
                let upper = plan.mass();
                let certified_underestimate = certified_underestimate(old.probability, floor);
                if old.probability > 0.0 && !weak_estimate(&old) && !certified_underestimate {
                    events.push(json!({"team":model.ids[candidate.cell.team],"rank":candidate.cell.rank+1,"status":"preserved_healthy_positive","exact_floor":floor,"exact_mass":upper,"old_probability":old.probability,"build_work":build.work}));
                    reason = "healthy_positive_consistent_with_exact_bounds";
                    continue;
                }
                let per_draw = plan.draw_cost(model).max(1);
                let draws = draws_per_stream(remaining, per_draw);
                if draws < MIN_DRAW_COUNT {
                    events.push(json!({"team":model.ids[candidate.cell.team],"rank":candidate.cell.rank+1,"status":"sample_budget_refusal","draws_per_stream":draws,"per_draw_cost":per_draw,"remaining":remaining}));
                    reason = "insufficient_final_sample_budget";
                    continue;
                }
                let reserve = draws.saturating_mul(per_draw).saturating_mul(2);
                if reserve > remaining {
                    events.push(json!({"team":model.ids[candidate.cell.team],"rank":candidate.cell.rank+1,"status":"reservation_overflow"}));
                    reason = "reservation_overflow";
                    continue;
                }
                reserved_work = reserved_work.saturating_add(reserve);
                remaining -= reserve;
                let namespace = format!(
                    "exact-caps-team-{}-rank-{}",
                    model.ids[candidate.cell.team],
                    candidate.cell.rank + 1
                );
                let seeds = [
                    derive(request_seed, &format!("{namespace}-main")),
                    derive(request_seed, &format!("{namespace}-check")),
                ];
                let results = search::parallel(2, workers, |stream| {
                    plan.sample(model, draws, seeds[stream])
                });
                let main = results[0].clone();
                let check = results[1].clone();
                let actual = main
                    .operations
                    .units()
                    .saturating_add(check.operations.units());
                final_work = final_work.saturating_add(actual);
                outcome.work = outcome
                    .work
                    .saturating_add(main.work)
                    .saturating_add(check.work);
                let actual_within_reserve = actual <= reserve
                    && scan_work
                        .saturating_add(setup_work)
                        .saturating_add(final_work)
                        <= capacity;
                let bounds_ok = within_exact_bounds(main.probability, floor, upper)
                    && within_exact_bounds(check.probability, floor, upper);
                let pair_ok = crate::rare_tail::accepted(&main, &check, false);
                let improves_positive = candidate.probability == 0.0
                    || improves_positive(&old, &main, &check, certified_underestimate);
                let accepted = actual_within_reserve
                    && main.samples == draws
                    && check.samples == draws
                    && finite_result(&main)
                    && finite_result(&check)
                    && main.probability > 0.0
                    && check.probability > 0.0
                    && bounds_ok
                    && pair_ok
                    && improves_positive;
                if accepted {
                    search::apply(&mut estimates[candidate.index], &main, DESIGN);
                    outcome.accepted = 1;
                    outcome.bounds.push((candidate.index, floor, upper));
                    certified_underestimates += usize::from(certified_underestimate);
                    published = true;
                    reason = if certified_underestimate {
                        "certified_underestimate_recovered"
                    } else if candidate.probability > 0.0 {
                        "weak_positive_refined"
                    } else {
                        "zero_recovered"
                    };
                } else {
                    reason = if !actual_within_reserve {
                        "actual_work_exceeded_reservation"
                    } else if !bounds_ok {
                        "estimate_outside_exact_bounds"
                    } else if !pair_ok {
                        "independent_pair_not_publishable"
                    } else if !improves_positive {
                        "positive_not_improved"
                    } else {
                        "invalid_sample_result"
                    };
                }
                events.push(json!({
                    "team":model.ids[candidate.cell.team],"rank":candidate.cell.rank+1,
                    "status":if accepted {"published"} else {"sample_declined"},"reason":reason,
                    "old_probability":candidate.probability,"exact_floor":floor,"exact_mass":upper,
                    "certified_underestimate":certified_underestimate,"draws_per_stream":draws,
                    "main_probability":main.probability,"check_probability":check.probability,
                    "main_ess":main.ess,"check_ess":check.ess,"main_rse":rse(&main),"check_rse":rse(&check),
                    "main_hits":main.hits,"check_hits":check.hits,"main_samples":main.samples,"check_samples":check.samples,
                    "build_work":build.work,"reserved_work":reserve,"actual_work":actual,
                    "actual_within_reserve":actual_within_reserve,"bounds_ok":bounds_ok,"pair_ok":pair_ok,
                    "plan":plan.describe(model)
                }));
            }
        }
    } else if enabled() {
        reason = "estimate_matrix_shape_mismatch";
    }

    let actual_total = scan_work
        .saturating_add(setup_work)
        .saturating_add(final_work);
    let overrun = actual_total > capacity;
    outcome.log = json!({
        "enabled":enabled(),"reason":reason,"budget_mode":super::budget::mode(),"capacity":capacity,
        "scan_reservation":scan_reservation,"scan_work":scan_work,"setup_work":setup_work,"final_work":final_work,
        "reserved_final_work":reserved_work,"actual_work":actual_total,"overrun":overrun,
        "attempts":attempts,"accepted":outcome.accepted,"certified_underestimates":certified_underestimates,
        "elapsed_ms":started.elapsed().as_secs_f64()*1000.0,"events":events
    });
    if let Some(log) = log {
        log.event("rust_odds_exact_caps_summary", outcome.log.clone());
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_enable_resolver_respects_opt_out_profiles_and_required_gate() {
        assert!(resolve_enabled(None, "coverage", true, true));
        assert!(!resolve_enabled(None, "disabled", true, true));
        assert!(resolve_enabled(Some("1"), "disabled", true, true));
        assert!(!resolve_enabled(Some("1"), "disabled", false, true));
        assert!(!resolve_enabled(Some("1"), "coverage", true, false));
        assert!(!resolve_enabled(Some("0"), "coverage", true, true));
    }

    #[test]
    fn certified_underestimate_can_replace_an_apparently_healthy_positive() {
        let old = Estimate {
            probability: 4.0e-24,
            std_err: 1.0e-25,
            relative_se: Some(0.025),
            ess: 80.0,
            meets_precision_goal: true,
            ..Estimate::default()
        };
        let main = Result {
            probability: 6.0e-24,
            ess: 1.0,
            std_err: 2.0e-24,
            ..Result::default()
        };
        let check = main.clone();
        let floor = 5.0e-24;
        assert!(!weak_estimate(&old));
        assert!(certified_underestimate(old.probability, floor));
        assert!(improves_positive(&old, &main, &check, true));
        assert!(!certified_underestimate(5.1e-24, floor));
    }

    #[test]
    fn insufficient_final_budget_cannot_admit_partial_pair() {
        let per_draw = 17usize;
        let remaining = 2 * per_draw * (MIN_DRAW_COUNT - 1);
        let draws = draws_per_stream(remaining, per_draw);
        assert_eq!(draws, MIN_DRAW_COUNT - 1);
        assert!(draws < MIN_DRAW_COUNT);
        assert_eq!(
            draws_per_stream(2 * per_draw * MIN_DRAW_COUNT, per_draw),
            MIN_DRAW_COUNT
        );
    }
}
