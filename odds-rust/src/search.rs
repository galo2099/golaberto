use crate::{
    conditioned::{blockers, fast, Event, Result},
    joint_caps::JointProposal,
    lookahead::{self, Policy},
    model::{Key, Model},
    pool::{point_proofs_supported, zero_upper, Bounds, Estimate},
    rng::derive,
};
mod guided_points;
pub use golaberto_odds_core::search_support::{apply, crosscheck, enabled, parallel, policy, Cell};
use std::sync::Arc;
pub struct SearchResult {
    pub result: Result,
    /// Retained for callers; numeric Event support is no longer a proof.
    pub impossible: bool,
    pub event: Option<Arc<Event>>,
    pub witness: Option<Vec<u8>>,
}
pub fn initial(
    model: &Model,
    bounds: &Bounds,
    pmfs: &[Vec<(i32, f64)>],
    cell: Cell,
    seed: i64,
) -> SearchResult {
    let needed = (cell.rank + 1).min(model.n - cell.rank);
    let count = if needed < 3 { 4 - needed } else { 0 };
    let mut event = None;
    for count in (0..=count).rev() {
        let selected = blockers(model, bounds, pmfs, cell.team, cell.rank, count);
        if let Some(e) = Event::build(model, bounds, cell.team, cell.rank, &selected, 20000) {
            event = Some(Arc::new(e));
            break;
        }
    }
    let mut search = SearchResult {
        result: Result::default(),
        impossible: false,
        event,
        witness: None,
    };
    let Some(event) = &search.event else {
        return search;
    };
    if event.mass <= 0. {
        // A zero under the finite WDL kernel is a numeric support result, not
        // a certificate that the ranked cell is mathematically impossible.
        search.event = None;
        return search;
    }
    let samples = if event.mass < 1e-7 || event.mass > 1e-3 {
        1000
    } else {
        2000
    };
    let cell_seed = derive(
        seed,
        &format!("conditioned-zero-{}-{}", model.ids[cell.team], cell.rank),
    );
    let mut result = fast(model, event, cell.team, cell.rank, samples, cell_seed);
    result.blockers = event.teams.len() - 1;
    search.witness = result.witness.clone();
    search.result = result;
    // Guided and extra searches only revisit zero-hit cells. Completed cells
    // need their estimate and witness, not the large conditioning DP layers.
    if search.result.hits > 0 {
        search.event = None;
    }
    search
}
fn guided(
    model: &Model,
    bounds: &Bounds,
    pmfs: &[Vec<(i32, f64)>],
    cells: &[Cell],
    seed: i64,
    workers: usize,
    results: &mut [SearchResult],
) {
    if !enabled("RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD") {
        return;
    }
    let pilots = parallel(cells.len(), workers, |i| {
        let cell = cells[i];
        let search = &results[i];
        if search.result.samples == 0 || search.result.hits > 0 {
            return None;
        }
        let mut event = search.event.clone()?;
        if event.mass < 1e-9 {
            let selected = blockers(model, bounds, pmfs, cell.team, cell.rank, 4);
            if let Some(e) = Event::build(model, bounds, cell.team, cell.rank, &selected, 30000) {
                event = Arc::new(e);
            }
        }
        let pilot = if event.mass > 0. {
            lookahead::sample(
                model,
                &event,
                cell.team,
                cell.rank,
                bounds,
                policy(
                    model,
                    cell,
                    seed,
                    "conditioned-guided-pilot",
                    200,
                    1.,
                    0.,
                    enabled("RARE_POSITION_WIN_AWARE_STABLE_SELECTION"),
                    false,
                ),
            )
        } else {
            Result::default()
        };
        Some((event, pilot))
    });
    let mut selected = Vec::new();
    for (i, pilot) in pilots.into_iter().enumerate() {
        if let Some((event, pilot)) = pilot {
            results[i].result.work += pilot.work;
            if results[i].witness.is_none() {
                results[i].witness = pilot.witness.clone();
            }
            results[i].event = Some(event.clone());
            if event.mass <= 0. {
                results[i].event = None;
                continue;
            }
            if pilot.weighted && pilot.hits >= 20 && pilot.ess.is_finite() {
                selected.push((i, pilot));
            }
        }
    }
    selected.sort_by(|(a, x), (b, y)| {
        y.hits
            .cmp(&x.hits)
            .then(y.ess.total_cmp(&x.ess))
            .then(model.ids[cells[*a].team].cmp(&model.ids[cells[*b].team]))
            .then(cells[*a].rank.cmp(&cells[*b].rank))
    });
    selected.truncate(3);
    let confirmed = parallel(selected.len(), workers, |i| {
        let index = selected[i].0;
        let cell = cells[index];
        let event = results[index].event.as_ref().unwrap();
        let p = policy(
            model,
            cell,
            seed,
            "conditioned-guided-estimate",
            20000,
            1.,
            0.,
            false,
            false,
        );
        let mut result = lookahead::sample(model, event, cell.team, cell.rank, bounds, p);
        if !result.strict()
            && enabled("RARE_POSITION_WIN_AWARE_FALLBACK")
            && crate::conditioned::stride(model, "lookahead") > 1
        {
            let mut point = lookahead::sample(
                model,
                event,
                cell.team,
                cell.rank,
                bounds,
                Policy {
                    force_points: true,
                    ..p
                },
            );
            point.work += result.work;
            result = point;
        }
        (index, result)
    });
    for (i, mut result) in confirmed {
        let search = &mut results[i];
        if search.witness.is_none() {
            search.witness = result.witness.clone();
        }
        if result.strict() {
            result.work += search.result.work;
            result.blockers = search.event.as_ref().unwrap().teams.len() - 1;
            search.result = result;
            search.event = None;
        } else {
            search.result.work += result.work;
        }
    }
}
struct ExtraPlan {
    index: usize,
    event: Option<Arc<Event>>,
    gain: f64,
    native: Option<Arc<crate::joint_caps::propagated::PropagatedJoint>>,
    setup_ms: f64,
}
fn extra(
    model: &Model,
    bounds: &Bounds,
    pmfs: &[Vec<(i32, f64)>],
    cells: &[Cell],
    estimates: &[Estimate],
    seed: i64,
    workers: usize,
    results: &mut [SearchResult],
    log: Option<&crate::logging::RequestLog>,
) -> Vec<(Cell, Result)> {
    let allocation_mode =
        std::env::var("RUST_ODDS_JOINT_ALLOCATION").unwrap_or_else(|_| "transfer".into());
    let replace = matches!(allocation_mode.as_str(), "swap" | "transfer")
        && enabled("RUST_ODDS_JOINT_PROPAGATION")
        && enabled("RUST_ODDS_JOINT_CAP_CONDITIONING")
        && enabled("RUST_ODDS_JOINT_CAP_BROAD");
    let planning = std::time::Instant::now();
    let plans = parallel(cells.len(), workers, |i| {
        let cell = cells[i];
        let search = &results[i];
        let r = &search.result;
        if r.samples == 0 || r.hits > 0 || r.mass <= 0. {
            return None;
        }
        let mut upper = r.mass * zero_upper(r.samples);
        let prior = estimates[cell.index(model.n)].zero_hit_upper_95;
        if prior > 0. && prior < upper {
            upper = prior;
        }
        if upper <= 1e-11 {
            if !replace {
                return None;
            }
            let setup = std::time::Instant::now();
            let native = crate::joint_caps::propagated::PropagatedJoint::new(model, cell)?;
            let description = native.describe(model);
            let gain = description["forced_min"].as_u64()? as f64
                / (1. + description["variable_max"].as_u64()? as f64);
            return Some(ExtraPlan {
                index: i,
                event: None,
                gain,
                native: Some(Arc::new(native)),
                setup_ms: crate::logging::millis(setup),
            });
        }
        let mut mass = r.mass;
        let mut refined = None;
        let needed = (cell.rank + 1).min(model.n - cell.rank);
        if model.n > 3 && needed <= 3 && r.blockers < 3 && mass <= 1e-3 {
            let selected = blockers(model, bounds, pmfs, cell.team, cell.rank, 3);
            if let Some(e) = Event::build(model, bounds, cell.team, cell.rank, &selected, 120000) {
                if e.mass <= 0. {
                    return None;
                }
                if e.mass < mass * (1. - 1e-8) {
                    mass = e.mass;
                    refined = Some(Arc::new(e));
                }
            }
        }
        let next = (mass * zero_upper(50000)).max(1e-12);
        if next >= upper {
            return None;
        }
        let gain = (upper / next).ln();
        let event = refined.or_else(|| {
            Event::build(model, bounds, cell.team, cell.rank, &[], 120000).map(Arc::new)
        })?;
        if event.mass <= 0. {
            return None;
        }
        Some(ExtraPlan {
            index: i,
            event: Some(event),
            gain,
            native: None,
            setup_ms: 0.,
        })
    });
    if replace {
        if let Some(log) = log {
            log.event(
                "rust_odds_joint_planning",
                serde_json::json!({"elapsed_ms":crate::logging::millis(planning)}),
            );
        }
    }
    let mut candidates = Vec::new();
    let mut rare = Vec::new();
    for plan in plans.into_iter().flatten() {
        if plan.native.is_some() {
            rare.push(plan);
        } else {
            candidates.push(plan);
        }
    }
    let order = |a: &ExtraPlan, b: &ExtraPlan| {
        b.gain
            .total_cmp(&a.gain)
            .then(model.ids[cells[a.index].team].cmp(&model.ids[cells[b.index].team]))
            .then(cells[a.index].rank.cmp(&cells[b.index].rank))
    };
    candidates.sort_by(order);
    rare.sort_by(order);
    if candidates.len() >= 4 && !rare.is_empty() {
        candidates.truncate(if allocation_mode == "swap" { 3 } else { 4 });
        candidates.push(rare.remove(0));
    } else {
        candidates.truncate(4);
    }
    if replace {
        if let Some(log) = log {
            log.event("rust_odds_joint_allocation",serde_json::json!({"mode":allocation_mode,"selected":candidates.iter().map(|p|serde_json::json!({
            "team":model.ids[cells[p.index].team],"rank":cells[p.index].rank+1,"propagated":p.native.is_some(),"score":p.gain})).collect::<Vec<_>>() }));
        }
    }
    let transfer = allocation_mode == "transfer" && candidates.iter().any(|p| p.native.is_some());
    if transfer {
        candidates.sort_by_key(|p| p.native.is_none());
    }
    let joint_enabled = enabled("RUST_ODDS_JOINT_CAP_CONDITIONING");
    let mut extras = parallel(candidates.len(), workers, |i| {
        let allocation = &candidates[i];
        let index = allocation.index;
        let event = allocation.event.as_ref();
        let cell = cells[index];
        let mut pending = None;
        let mut joint_work = 0;
        let mut ordinary_draws = 50000;
        if joint_enabled && !(transfer && allocation.native.is_some()) {
            let start = std::time::Instant::now();
            // Use the cheap exact extreme case when its gate is proved;
            // otherwise cover all allowed target totals with a defensive mixture.
            let fallback = if allocation.native.is_none() {
                JointProposal::new(model, cell)
            } else {
                None
            };
            if allocation.native.is_some() || fallback.is_some() {
                let propagated = allocation.native.as_deref();
                let description = propagated.map_or_else(
                    || fallback.as_ref().unwrap().describe(model),
                    |p| p.describe(model),
                );
                let sample = |count, stream| {
                    propagated.map_or_else(
                        || {
                            fallback
                                .as_ref()
                                .unwrap()
                                .sample(model, count, stream, true)
                        },
                        |p| p.sample(model, count, stream, true),
                    )
                };
                let setup_ms = crate::logging::millis(start);
                let broad = !matches!(fallback, Some(JointProposal::Exact(_)));
                let main_draws = if propagated.is_some() {
                    3000
                } else if broad {
                    1500
                } else {
                    5000
                };
                let check_draws = if broad { 1500 } else { 2000 };
                let mut result = sample(
                    main_draws,
                    derive(
                        seed,
                        &format!("joint-cap-final-{}-{}", model.ids[cell.team], cell.rank),
                    ),
                );
                let check = sample(
                    check_draws,
                    derive(
                        seed,
                        &format!("joint-cap-check-{}-{}", model.ids[cell.team], cell.rank),
                    ),
                );
                let accepted = crate::joint_caps::confirmed(&result, &check);
                joint_work = result.work + check.work;
                // Replace existing draws, including the independent check. Do
                // not give unsuccessful attempts an extra simulation budget.
                // Forecast ordinary hits from the two independent joint streams.
                // This is allocation guidance, not a probability bound. Retain
                // more ordinary draws when they may contribute useful witnesses.
                let ordinary_hit_forecast = if broad && event.is_some() {
                    Some(50000. * result.probability.max(check.probability) / event.unwrap().mass)
                } else {
                    None
                };
                ordinary_draws = if propagated.is_some() {
                    0
                } else if broad {
                    if ordinary_hit_forecast.unwrap() >= 0.01 {
                        43000
                    } else if transfer {
                        25000
                    } else {
                        35000
                    }
                } else if accepted {
                    10000
                } else {
                    43000
                };
                if let Some(log) = log {
                    log.event("rust_odds_joint_caps", serde_json::json!({
                        "setup":description,"setup_ms":setup_ms,
                        "elapsed_ms":crate::logging::millis(start),
                        "probability":result.probability,"hits":result.hits,"ess":result.ess,
                        "relative_se":if result.probability>0. {Some(result.std_err/result.probability)} else {None},
                        "check_probability":check.probability,"check_hits":check.hits,"check_ess":check.ess,
                        "accepted":accepted,"publication":"deferred",
                        "joint_draws":result.samples,"check_draws":check.samples,
                        "ordinary_draws":ordinary_draws,
                        "ordinary_hit_forecast":ordinary_hit_forecast
                    }));
                }
                if accepted {
                    result.work = joint_work;
                    pending = Some(result);
                }
            }
        }
        let seed = derive(
            seed,
            &format!(
                "conditioned-zero-extra-{}-{}",
                model.ids[cell.team], cell.rank
            ),
        );
        let mut result = if ordinary_draws == 0 || transfer {
            let mut r = results[index].result.clone();
            r.work = 0;
            r
        } else {
            fast(
                model,
                event.unwrap(),
                cell.team,
                cell.rank,
                ordinary_draws,
                seed,
            )
        };
        if let Some(event) = event {
            result.blockers = event.teams.len() - 1;
        }
        result.work += joint_work;
        (index, result, pending, ordinary_draws)
    });
    if transfer {
        let phase_order = {
            let mut order = (0..candidates.len()).collect::<Vec<_>>();
            order.sort_by_key(|&i| {
                (
                    candidates[i].native.is_none(),
                    std::cmp::Reverse(extras[i].3),
                )
            });
            order
        };
        let saved_draws: usize = extras.iter().filter(|e| e.3 == 25000).count() * 10000;
        extras = parallel(candidates.len(), workers, |job| {
            let i = phase_order[job];
            let allocation = &candidates[i];
            let (index, placeholder, pending, draws) = &extras[i];
            let cell = cells[*index];
            if let Some(plan) = &allocation.native {
                if saved_draws < 8000 {
                    return (*index, placeholder.clone(), None, 0);
                }
                let start = std::time::Instant::now();
                let mut main = plan.sample(
                    model,
                    5000,
                    derive(
                        seed,
                        &format!("joint-cap-final-{}-{}", model.ids[cell.team], cell.rank),
                    ),
                    true,
                );
                let check = plan.sample(
                    model,
                    3000,
                    derive(
                        seed,
                        &format!("joint-cap-check-{}-{}", model.ids[cell.team], cell.rank),
                    ),
                    true,
                );
                let accepted = crate::joint_caps::confirmed(&main, &check);
                let work = main.work + check.work;
                if let Some(log) = log {
                    log.event("rust_odds_joint_caps",serde_json::json!({
                    "setup":plan.describe(model),"setup_ms":allocation.setup_ms,
                    "elapsed_ms":crate::logging::millis(start)+allocation.setup_ms,
                    "sampling_ms":crate::logging::millis(start),"saved_draws":saved_draws,
                    "probability":main.probability,"hits":main.hits,"ess":main.ess,"max_share":main.max_share,
                    "relative_se":if main.probability>0.{Some(main.std_err/main.probability)}else{None},
                    "check_probability":check.probability,"check_hits":check.hits,"check_ess":check.ess,
                    "accepted":accepted,"publication":"deferred","joint_draws":main.samples,"check_draws":check.samples,"ordinary_draws":0
                }));
                }
                main.work = work;
                // A verified hit and the necessary event mass remain useful
                // even when the probability fails the independent check.
                // Do not turn an IS zero-hit count into a binomial bound.
                if !accepted {
                    main.probability = 0.;
                    main.std_err = 0.;
                }
                let mut result = placeholder.clone();
                result.work += work;
                return (*index, result, Some(main), 0);
            }
            let event = allocation.event.as_ref().unwrap();
            let mut result = fast(
                model,
                event,
                cell.team,
                cell.rank,
                *draws,
                derive(
                    seed,
                    &format!(
                        "conditioned-zero-extra-{}-{}",
                        model.ids[cell.team], cell.rank
                    ),
                ),
            );
            result.blockers = event.teams.len() - 1;
            result.work += placeholder.work;
            (*index, result, pending.clone(), *draws)
        });
    }
    let mut pending_joint = Vec::new();
    for (i, mut extra, pending, _) in extras {
        if let Some(result) = pending {
            pending_joint.push((cells[i], result));
        }
        extra.work += results[i].result.work;
        if results[i].witness.is_none() {
            results[i].witness = extra.witness.clone();
        }
        results[i].result = extra;
    }
    pending_joint
}
pub fn run(model: &Model, seed: i64, workers: usize, estimates: &mut [Estimate]) -> u64 {
    run_logged(model, seed, workers, estimates, None)
}

fn run_tail_fallback(
    model: &Model,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    log: Option<&crate::logging::RequestLog>,
    intervals: &[(usize, f64, f64)],
    bounds: &[(usize, f64, f64)],
) -> u64 {
    let selected = std::env::var("RUST_ODDS_TAIL_ALGORITHM").ok();
    let profile = crate::rare_tail::profile();
    let Some(mode) = tail_fallback_mode(selected.as_deref(), &profile) else {
        return 0;
    };
    if !tail_fallback_supported(model)
        || model.fixtures.is_empty()
        || estimates.len() != model.n.saturating_mul(model.n)
        || !estimates.iter().any(|estimate| {
            estimate.probability == 0. && !estimate.reachability.starts_with("impossible")
        })
    {
        return 0;
    }
    let baseline = estimates.to_vec();
    let (conditioned_accepted, conditioned_work) = if mode != "particles" {
        crate::tail_conditioned::run(model, seed, workers, estimates, log)
    } else {
        (0, 0)
    };
    let (particle_accepted, particle_work) = if mode != "conditioned"
        && estimates.iter().any(|estimate| {
            estimate.probability == 0. && !estimate.reachability.starts_with("impossible")
        }) {
        crate::tail_particles::run(model, seed, workers, estimates, log)
    } else {
        (0, 0)
    };
    let accepted = conditioned_accepted.saturating_add(particle_accepted);
    let spent = conditioned_work.saturating_add(particle_work);
    let mut retained = accepted;
    let mut rolled_back = false;
    if accepted > 0 {
        let valid = reconcile(model, estimates, &baseline)
            && crate::pool::valid(
                &estimates.iter().map(|e| e.probability).collect::<Vec<_>>(),
                model.n,
            )
            && baseline
                .iter()
                .zip(estimates.iter())
                .all(|(before, after)| {
                    (before.probability <= 0. || after.probability > 0.)
                        && (!before.reachability.starts_with("impossible")
                            || (after.probability == 0.
                                && after.reachability.starts_with("impossible")))
                })
            && intervals.iter().all(|(index, lower, upper)| {
                let p = estimates[*index].probability;
                let tolerance =
                    16. * f64::EPSILON * lower.abs().max(upper.abs()).max(f64::MIN_POSITIVE);
                p.is_finite() && p >= *lower - tolerance && p <= *upper + tolerance
            })
            && bounds.iter().all(|(index, floor, upper)| {
                let p = estimates[*index].probability;
                p.is_finite()
                    && p + floor.abs().max(f64::MIN_POSITIVE) * 1e-10 >= *floor
                    && p <= *upper + upper.abs().max(f64::MIN_POSITIVE) * 1e-10
            });
        if !valid {
            estimates.clone_from_slice(&baseline);
            retained = 0;
            rolled_back = true;
        }
    }
    if let Some(log) = log {
        log.event(
            "rust_odds_tail_fallback_reconcile",
            serde_json::json!({"algorithm":mode,"conditioned_accepted":conditioned_accepted,
                "particle_accepted":particle_accepted,"accepted":retained,
                "rolled_back":rolled_back,"work":spent}),
        );
    }
    spent
}

fn tail_fallback_mode(selected: Option<&str>, profile: &str) -> Option<&'static str> {
    match selected {
        Some("particles") => Some("particles"),
        Some("conditioned") => Some("conditioned"),
        Some(_) => None,
        None if profile == "coverage" => Some("conditioned_then_particles"),
        None => None,
    }
}

pub(crate) fn tail_fallback_supported(model: &Model) -> bool {
    let rules = &model.request.phase.championship;
    (1..=64).contains(&model.n)
        && model.ids.len() == model.n
        && model.request.team_groups.len() == model.n
        && model.fixtures.len() <= 256
        && rules.point_win == 3
        && rules.point_draw == 1
        && rules.point_loss == 0
        && model.request.phase.bonus_points == 0
        && model.keys.first() == Some(&Key::Pt)
        && model.fixtures.iter().all(|fixture| {
            fixture.home < model.n
                && fixture.away < model.n
                && fixture.home != fixture.away
                && fixture.home_sampler.mean.is_finite()
                && fixture.away_sampler.mean.is_finite()
        })
}

pub fn run_logged(
    model: &Model,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    log: Option<&crate::logging::RequestLog>,
) -> u64 {
    let rank_rescue = crate::rare_tail::rank_rescue::Settings::from_env();
    let mut work = run_logged_inner(model, seed, workers, estimates, log);
    // A saturated Monte Carlo cell is not itself a proof.  For points-first
    // requests, certify it only when every possible full-season points total
    // lies strictly on one side of the target's interval and the resulting
    // count fixes the rank.  Ties and all score-dependent keys remain open.
    prove_saturated_by_points(model, estimates);
    let complement_baseline = estimates.to_vec();
    let mut complement = crate::native_complement::run(model, seed, estimates);
    let complemented = complement.components;
    let complement_work = complement.work;
    work = work.saturating_add(complement_work);
    let mut complement_rolled_back = false;
    if complemented > 0 {
        if !reconcile(model, estimates, &complement_baseline)
            || !crate::pool::valid(
                &estimates.iter().map(|e| e.probability).collect::<Vec<_>>(),
                model.n,
            )
            || complement_baseline
                .iter()
                .zip(estimates.iter())
                .any(|(before, after)| before.probability > 0. && after.probability <= 0.)
            || complement.intervals.iter().any(|(index, lower, upper)| {
                let p = estimates[*index].probability;
                let tolerance =
                    16. * f64::EPSILON * lower.abs().max(upper.abs()).max(f64::MIN_POSITIVE);
                p < *lower - tolerance || p > *upper + tolerance
            })
        {
            restore_baseline_preserving_reachability(estimates, &complement_baseline);
            complement_rolled_back = true;
            complement.intervals.clear();
        }
    }
    if let Some(log) = log {
        log.event(
            "rust_odds_native_full_world_complement",
            serde_json::json!({
                "components": complemented, "work": complement_work,
                "rolled_back": complement_rolled_back,
                "reason": complement.reason,
                "accepted_targets": complement.accepted_targets,
                "reserved_prep": complement.reserved_prep,
                "epsilon_interval": complement.epsilon_interval
            }),
        );
    }
    let exact_caps_baseline = estimates.to_vec();
    let mut exact_caps = crate::rare_tail::exact_caps::run(model, seed, workers, estimates, log);
    work = work.saturating_add(exact_caps.work);
    let mut exact_caps_rolled_back = false;
    if exact_caps.accepted > 0 {
        let reconciled = reconcile(model, estimates, &exact_caps_baseline);
        let matrix_valid = crate::pool::valid(
            &estimates.iter().map(|e| e.probability).collect::<Vec<_>>(),
            model.n,
        );
        let positives_preserved = exact_caps_baseline
            .iter()
            .zip(estimates.iter())
            .all(|(before, after)| before.probability <= 0. || after.probability > 0.);
        let intervals_preserved = complement.intervals.iter().all(|(index, lower, upper)| {
            let p = estimates[*index].probability;
            let tolerance =
                16. * f64::EPSILON * lower.abs().max(upper.abs()).max(f64::MIN_POSITIVE);
            p >= *lower - tolerance && p <= *upper + tolerance
        });
        let bounds_preserved = exact_caps.bounds.iter().all(|(index, floor, upper)| {
            let probability = estimates[*index].probability;
            let lower_tolerance = floor.abs().max(f64::MIN_POSITIVE) * 1e-10;
            let upper_tolerance = upper.abs().max(f64::MIN_POSITIVE) * 1e-10;
            probability + lower_tolerance >= *floor && probability <= *upper + upper_tolerance
        });
        if !reconciled
            || !matrix_valid
            || !positives_preserved
            || !intervals_preserved
            || !bounds_preserved
        {
            estimates.clone_from_slice(&exact_caps_baseline);
            exact_caps_rolled_back = true;
            exact_caps.accepted = 0;
            exact_caps.bounds.clear();
        }
    }
    if let Some(log) = log {
        log.event(
            "rust_odds_exact_caps_reconcile",
            serde_json::json!({
                "accepted":exact_caps.accepted,
                "rolled_back":exact_caps_rolled_back,
                "preserved_positive_count":exact_caps_baseline.iter().zip(estimates.iter()).filter(|(before,after)|before.probability>0.&&after.probability>0.).count(),
                "valid_matrix":crate::pool::valid(&estimates.iter().map(|e|e.probability).collect::<Vec<_>>(),model.n),
                "bounds_preserved":exact_caps.bounds.iter().all(|(index,floor,upper)|{let p=estimates[*index].probability;p+floor.abs().max(f64::MIN_POSITIVE)*1e-10>=*floor&&p<=*upper+upper.abs().max(f64::MIN_POSITIVE)*1e-10})
            }),
        );
    }
    if !rank_rescue.enabled {
        work = work.saturating_add(run_tail_fallback(
            model,
            seed,
            workers,
            estimates,
            log,
            &complement.intervals,
            &exact_caps.bounds,
        ));
        work = work.saturating_add(recover_remaining_gd_reconciled(
            model,
            seed,
            workers,
            estimates,
            log,
            &complement.intervals,
            &exact_caps.bounds,
        ));
        let packed_phase = std::time::Instant::now();
        let packed = packed_witness_fallback(model, estimates);
        if let Some(log) = log {
            log.stage(
                "search.packed_witness",
                packed_phase,
                serde_json::json!(packed),
            );
        }
        return work;
    }
    let rescue_baseline = estimates.to_vec();
    let (rescue_added, rescue_work) =
        crate::rare_tail::rank_rescue::run(model, seed, workers, estimates, log, rank_rescue);
    let provisional_changes = estimates
        .iter()
        .zip(&rescue_baseline)
        .filter(|(after, before)| {
            serde_json::to_value(after).ok() != serde_json::to_value(before).ok()
        })
        .count();
    let mut rolled_back = false;
    if rescue_added > 0 {
        rolled_back = !reconcile(model, estimates, &rescue_baseline);
        if !rolled_back {
            rolled_back = complement.intervals.iter().any(|(index, lower, upper)| {
                let p = estimates[*index].probability;
                let tolerance =
                    16. * f64::EPSILON * lower.abs().max(upper.abs()).max(f64::MIN_POSITIVE);
                p < *lower - tolerance || p > *upper + tolerance
            });
        }
        if !rolled_back
            && rescue_baseline
                .iter()
                .zip(estimates.iter())
                .any(|(before, after)| before.probability > 0. && after.probability <= 0.)
        {
            restore_baseline_preserving_reachability(estimates, &rescue_baseline);
            rolled_back = true;
        }
        if !rolled_back
            && exact_caps.bounds.iter().any(|(index, floor, upper)| {
                let probability = estimates[*index].probability;
                probability + floor.abs().max(f64::MIN_POSITIVE) * 1e-10 < *floor
                    || probability > *upper + upper.abs().max(f64::MIN_POSITIVE) * 1e-10
            })
        {
            restore_baseline_preserving_reachability(estimates, &rescue_baseline);
            rolled_back = true;
        }
    }
    let valid_matrix = crate::pool::valid(
        &estimates.iter().map(|e| e.probability).collect::<Vec<_>>(),
        model.n,
    );
    if !valid_matrix && rescue_added > 0 {
        restore_baseline_preserving_reachability(estimates, &rescue_baseline);
        rolled_back = true;
    }
    if rolled_back && rescue_added > 0 {
        restore_baseline_preserving_reachability(estimates, &rescue_baseline);
    }
    let final_changes = estimates
        .iter()
        .zip(&rescue_baseline)
        .filter(|(after, before)| {
            serde_json::to_value(after).ok() != serde_json::to_value(before).ok()
        })
        .count();
    if let Some(log) = log {
        log.event(
            "rust_odds_rare_tail_rank_rescue_reconcile",
            serde_json::json!({
                "rolled_back":rolled_back,
                "provisional_changes":provisional_changes,
                "final_changes":final_changes,
                "preserved_positive_count":rescue_baseline.iter().zip(estimates.iter()).filter(|(before,after)|before.probability>0.&&after.probability>0.).count(),
                "valid_matrix":crate::pool::valid(&estimates.iter().map(|e|e.probability).collect::<Vec<_>>(),model.n)
            }),
        );
    }
    work = work.saturating_add(rescue_work);
    work = work.saturating_add(run_tail_fallback(
        model,
        seed,
        workers,
        estimates,
        log,
        &complement.intervals,
        &exact_caps.bounds,
    ));
    work = work.saturating_add(recover_remaining_gd_reconciled(
        model,
        seed,
        workers,
        estimates,
        log,
        &complement.intervals,
        &exact_caps.bounds,
    ));
    let packed_phase = std::time::Instant::now();
    let packed = packed_witness_fallback(model, estimates);
    if let Some(log) = log {
        log.stage(
            "search.packed_witness",
            packed_phase,
            serde_json::json!(packed),
        );
    }
    work
}

fn recover_remaining_gd_reconciled(
    model: &Model,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    log: Option<&crate::logging::RequestLog>,
    intervals: &[(usize, f64, f64)],
    bounds: &[(usize, f64, f64)],
) -> u64 {
    if !estimates.iter().any(|estimate| {
        estimate.probability == 0. && !estimate.reachability.starts_with("impossible")
    }) {
        return 0;
    }
    let baseline = estimates.to_vec();
    let (accepted, work) =
        crate::rare_tail::recover_remaining_gd(model, seed, workers, estimates, log);
    let mut rolled_back = false;
    if accepted > 0 {
        let valid = reconcile(model, estimates, &baseline)
            && crate::pool::valid(
                &estimates
                    .iter()
                    .map(|estimate| estimate.probability)
                    .collect::<Vec<_>>(),
                model.n,
            )
            && baseline
                .iter()
                .zip(estimates.iter())
                .all(|(before, after)| {
                    (before.probability <= 0. || after.probability > 0.)
                        && (!before.reachability.starts_with("impossible")
                            || (after.probability == 0.
                                && after.reachability.starts_with("impossible")))
                })
            && intervals.iter().all(|(index, lower, upper)| {
                let probability = estimates[*index].probability;
                let tolerance =
                    16. * f64::EPSILON * lower.abs().max(upper.abs()).max(f64::MIN_POSITIVE);
                probability >= *lower - tolerance && probability <= *upper + tolerance
            })
            && bounds.iter().all(|(index, floor, upper)| {
                let probability = estimates[*index].probability;
                probability + floor.abs().max(f64::MIN_POSITIVE) * 1e-10 >= *floor
                    && probability <= *upper + upper.abs().max(f64::MIN_POSITIVE) * 1e-10
            });
        if !valid {
            restore_baseline_preserving_reachability(estimates, &baseline);
            rolled_back = true;
        }
    }
    if let Some(log) = log {
        log.event(
            "rust_odds_remaining_gd_reconcile",
            serde_json::json!({
                "accepted": if rolled_back { 0 } else { accepted },
                "rolled_back": rolled_back,
                "work": work,
                "remaining_zeros": estimates.iter().filter(|estimate| {
                    estimate.probability == 0. && !estimate.reachability.starts_with("impossible")
                }).count()
            }),
        );
    }
    work
}

fn publish_fixed_mass_result(est: &mut Estimate, result: &Result) {
    let frequency = result.hits as f64 / result.samples as f64;
    est.probability = result.mass * frequency;
    est.std_err = result.mass * (frequency * (1. - frequency) / result.samples as f64).sqrt();
    est.design = "matched_point_pool_conditioned".into();
    est.relative_se = Some(est.std_err / est.probability);
    est.reachability = "witness".into();
    est.zero_hit_upper_95 = 0.;
    est.ess = result.hits as f64;
    est.max_event_weight_share = 1. / result.hits as f64;
    est.meets_precision_goal = result.hits >= 10 && est.std_err / est.probability <= 0.5;
}

#[cfg(test)]
fn structurally_possible_cells(
    model: &Model,
    bounds: &Bounds,
    estimates: &mut [Estimate],
) -> Vec<Cell> {
    structurally_possible_cells_with_proofs(
        model,
        bounds,
        estimates,
        &golaberto_odds_core::rank_support::impossible_ranks(model),
    )
}

fn structurally_possible_cells_with_proofs(
    model: &Model,
    bounds: &Bounds,
    estimates: &mut [Estimate],
    impossible_ranks: &[bool],
) -> Vec<Cell> {
    let mut cells = Vec::new();
    for team in 0..model.n {
        for rank in 0..model.n {
            let estimate = &mut estimates[team * model.n + rank];
            let rank_proven_impossible = impossible_ranks
                .get(team * model.n + rank)
                .copied()
                .unwrap_or(false);
            if rank_proven_impossible {
                let warranted = has_direct_evidence(estimate);
                if !warranted {
                    estimate.probability = 0.;
                    estimate.reachability = "impossible_by_rank_support".into();
                    estimate.zero_hit_upper_95 = 0.;
                    estimate.pool_guide = 0.;
                    continue;
                }
            }
            if estimate.probability != 0. {
                continue;
            }
            if estimate.reachability.starts_with("impossible") {
                estimate.zero_hit_upper_95 = 0.;
                estimate.pool_guide = 0.;
                continue;
            }
            if point_proofs_supported(model) && !bounds.rank_possible(team, rank) {
                estimate.reachability = "impossible_by_points".into();
                estimate.zero_hit_upper_95 = 0.;
                estimate.pool_guide = 0.;
            } else {
                estimate.reachability = "undecided".into();
                cells.push(Cell { team, rank });
            }
        }
    }
    cells
}

fn prove_saturated_by_points(model: &Model, estimates: &mut [Estimate]) {
    if model.n == 0 || !point_proofs_supported(model) {
        return;
    }
    let bounds = Bounds::new(model);
    for target in 0..model.n {
        for rank in 0..model.n {
            let index = target * model.n + rank;
            let estimate = &mut estimates[index];
            if estimate.probability != 1. {
                continue;
            }
            let mut above = 0;
            let mut below = 0;
            let mut separated = true;
            for rival in 0..model.n {
                if rival == target {
                    continue;
                }
                if bounds.max[target] < bounds.min[rival] {
                    above += 1;
                } else if bounds.min[target] > bounds.max[rival] {
                    below += 1;
                } else {
                    separated = false;
                    break;
                }
            }
            if separated && above == rank && below == model.n - 1 - rank {
                estimate.design = "certain_by_points".into();
                estimate.reachability = "certain_by_points".into();
                estimate.meets_precision_goal = true;
                estimate.std_err = 0.;
                estimate.relative_se = Some(0.);
                estimate.zero_hit_upper_95 = 0.;
            }
        }
    }
}

fn run_logged_inner(
    model: &Model,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    log: Option<&crate::logging::RequestLog>,
) -> u64 {
    let search_start = std::time::Instant::now();
    let baseline = estimates.to_vec();
    let bounds = Bounds::new(model);
    let profile = std::env::var("RUST_ODDS_PROFILE").as_deref() == Ok("1");
    let mut phase = std::time::Instant::now();
    let mut report = |label: &str, fields: serde_json::Value| {
        if let Some(log) = log {
            log.stage(&format!("search.{label}"), phase, fields);
        } else if profile {
            eprintln!(
                "rust-search-stage {label} ms={:.3}",
                phase.elapsed().as_secs_f64() * 1000.
            );
        }
        phase = std::time::Instant::now();
    };
    // Compute the conservative full-world support proof once per search. It is
    // deliberately independent of pool guides and numeric zero-hit outcomes.
    let rank_support = golaberto_odds_core::rank_support::impossible_ranks(model);
    let mut cells =
        structurally_possible_cells_with_proofs(model, &bounds, estimates, &rank_support);
    cells.sort_by(|a, b| {
        let guide = |cell: &Cell| estimates[cell.index(model.n)].pool_guide;
        let ga = guide(a);
        let gb = guide(b);
        let a_valid = ga.is_finite() && ga > 0.;
        let b_valid = gb.is_finite() && gb > 0.;
        b_valid
            .cmp(&a_valid)
            .then_with(|| {
                if a_valid && b_valid {
                    gb.total_cmp(&ga)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then(
                a.rank
                    .min(model.n - 1 - a.rank)
                    .cmp(&b.rank.min(model.n - 1 - b.rank)),
            )
            .then(model.ids[a.team].cmp(&model.ids[b.team]))
            .then(a.rank.cmp(&b.rank))
    });
    report(
        "screen",
        serde_json::json!({"candidate_cells":cells.len(),"cells":crate::logging::cell_counts(estimates)}),
    );
    let proof_report = if point_proofs_supported(model) {
        crate::proof::early_report(model, &cells, estimates)
    } else {
        Default::default()
    };
    let proofs = proof_report.proofs;
    cells.retain(|c| estimates[c.index(model.n)].reachability == "undecided");
    report(
        "early_proofs",
        serde_json::json!({"proofs":proofs,"candidate_cells":cells.len()}),
    );
    if !enabled("RARE_POSITION_CONDITIONED_ZERO") {
        if let Some(log) = log {
            log.event(
                "rust_odds_search_skipped",
                serde_json::json!({"reason":"disabled"}),
            );
        }
        return 0;
    }
    let Some(pmfs) = crate::pool::point_pmfs(model) else {
        return 0;
    };
    let (guided_count, guided_work) =
        guided_points::estimate(model, &bounds, &pmfs, &cells, estimates, seed, workers);
    report(
        "guided_points",
        serde_json::json!({"count":guided_count,"work":guided_work}),
    );
    let mut witnesses = guided_count;
    let mut work = guided_work;
    cells.retain(|cell| {
        let estimate = &estimates[cell.index(model.n)];
        estimate.probability == 0. && estimate.reachability == "undecided"
    });
    if cells.is_empty() {
        if witnesses > 0 && !reconcile(model, estimates, &baseline) {
            return work;
        }
        let baseline = estimates.to_vec();
        let (found, spent) =
            crate::rare_tail::run_late_positive_refinement(model, seed, workers, estimates, log);
        let reconciled = found > 0 && reconcile(model, estimates, &baseline);
        if let Some(log) = log {
            log.event(
                "rust_odds_search_positive_cohort_refinement",
                serde_json::json!({"found":found,"reconciled":reconciled,"work":spent}),
            );
        }
        return work.saturating_add(spent);
    }
    let mut results = parallel(cells.len(), workers, |i| {
        initial(model, &bounds, &pmfs, cells[i], seed)
    });
    report(
        "initial_conditioning",
        serde_json::json!({"candidate_cells":cells.len()}),
    );
    guided(model, &bounds, &pmfs, &cells, seed, workers, &mut results);
    report("guided", serde_json::json!({}));
    // Extra search builds fresh conditioning events from the result metadata.
    // Release the earlier layers before those larger refinements are allocated.
    for search in &mut results {
        search.event = None;
    }
    let pending_joint = extra(
        model,
        &bounds,
        &pmfs,
        &cells,
        estimates,
        seed,
        workers,
        &mut results,
        log,
    );
    report("extra", serde_json::json!({}));
    for (cell, search) in cells.iter().zip(&results) {
        let est = &mut estimates[cell.index(model.n)];
        let result = &search.result;
        if result.samples == 0 {
            continue;
        }
        work += result.work;
        est.conditional_mass = result.mass;
        est.conditional_samples = result.samples;
        est.conditional_hits = result.hits;
        if result.hits > 0 {
            if result.weighted {
                apply(est, result, "matched_point_pool_conditioned_lookahead");
            } else {
                publish_fixed_mass_result(est, result);
            }
            witnesses += 1;
        } else {
            est.zero_hit_upper_95 = est
                .zero_hit_upper_95
                .min(result.mass * zero_upper(result.samples));
        }
    }
    let seeds: Vec<_> = results.into_iter().filter_map(|r| r.witness).collect();
    report(
        "apply",
        serde_json::json!({"accepted":witnesses,"distinct_seeds":seeds.len(),"work":work,
        "cells":crate::logging::cell_counts(estimates)}),
    );
    let budget = std::env::var("RARE_POSITION_NEIGHBORHOOD_BUDGET")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|b| *b > 0 && *b <= 250000)
        .unwrap_or(10000);
    let allocation = std::env::var("RUST_ODDS_REACHABILITY_ALLOCATION").unwrap_or_default();
    let neighbor = if allocation == "replace_neighborhood" {
        Vec::new()
    } else {
        crate::neighbors::search(model, &cells, &seeds, estimates, budget)
    };
    crate::neighbors::mark(model, &neighbor, estimates);
    let mut assignments: Vec<_> = neighbor.into_iter().map(|p| p.outcomes).collect();
    if std::env::var("RUST_ODDS_REACHABILITY_TRACE").as_deref() == Ok("1") {
        report(
            "neighborhood",
            serde_json::json!({"assignments":assignments.len(),"budget":budget}),
        );
    }
    let witness_mode = crate::proof::witness_mode();
    let construction = crate::proof::witnesses_report(model, &cells, estimates, &witness_mode);
    assignments.extend(construction.outcomes);
    report(
        "witnesses",
        serde_json::json!({"assignments":assignments.len(),"neighborhood_budget":budget,
        "cells":crate::logging::cell_counts(estimates)}),
    );
    let recycled = if enabled("RARE_POSITION_RECYCLE_PROOF_WORK") {
        (1000 * proof_report.recycle_credit).min(4000)
    } else {
        0
    };
    let (found, spent) =
        crate::tilt::run(model, &cells, &bounds, seed, workers, estimates, recycled);
    witnesses += found;
    work += spent;
    report(
        "point_tilt",
        serde_json::json!({"accepted":found,"work":spent,"recycled_draws":recycled,
        "cells":crate::logging::cell_counts(estimates)}),
    );
    if allocation == "replace_walk" {
        crate::proof::witnesses_mode(model, &cells, estimates, "dual");
    } else if allocation == "split_walk" {
        crate::neighbors::walk_passes(model, &cells, &assignments, estimates, 1);
    } else {
        crate::neighbors::walk(model, &cells, &assignments, estimates);
    }
    report(
        "neighbor_walk",
        serde_json::json!({"cells":crate::logging::cell_counts(estimates)}),
    );
    let (found, spent) = crate::rescue::peers(model, &bounds, seed, workers, estimates);
    witnesses += found;
    work += spent;
    report(
        "peers",
        serde_json::json!({"accepted":found,"work":spent,"cells":crate::logging::cell_counts(estimates)}),
    );
    let (found, spent) = crate::rescue::domains(model, &bounds, seed, workers, estimates);
    witnesses += found;
    work += spent;
    report(
        "domains",
        serde_json::json!({"accepted":found,"work":spent,"cells":crate::logging::cell_counts(estimates)}),
    );
    if allocation == "split_walk" {
        // Replace the second 4,000-candidate walk with a small joint search.
        // Run after estimators so proof discovery cannot reshuffle their quotas.
        crate::reachability::run(
            model,
            &cells,
            estimates,
            25,
            400.min(3200 - construction.nodes),
        );
        report(
            "joint_tail",
            serde_json::json!({"cells":crate::logging::cell_counts(estimates)}),
        );
    } else if allocation == "adaptive_tail" {
        // Experimental: fund a small joint search by the work skipped by the
        // adaptive constructive screen; preserve both original walk passes.
        crate::reachability::run(
            model,
            &cells,
            estimates,
            100,
            200.min(3200 - construction.nodes),
        );
        report(
            "joint_tail",
            serde_json::json!({"cells":crate::logging::cell_counts(estimates)}),
        );
    }
    if !construction.deferred.is_empty() || !construction.goal_witnesses.is_empty() {
        for outcomes in &construction.deferred {
            for (team, rank) in crate::conditioned::canonical_ranks(model, outcomes)
                .into_iter()
                .enumerate()
            {
                let e = &mut estimates[team * model.n + rank];
                if e.probability == 0. && e.reachability == "undecided" {
                    e.reachability = "reachable_by_construction".into();
                }
            }
        }
        // Certificates have already materialized legal scores and passed the
        // production sorter. Keep only their compact adjustments and actual ranks.
        for proof in &construction.goal_witnesses {
            for (team, &rank) in proof.ranks.iter().enumerate() {
                let e = &mut estimates[team * model.n + rank];
                if e.probability == 0. && e.reachability == "undecided" {
                    e.reachability = "reachable_by_construction".into();
                }
            }
        }
        report(
            "deferred_witnesses",
            serde_json::json!({"seasons":construction.deferred.len(),
            "goal_certificates":construction.goal_witnesses.len(),
            "cells":crate::logging::cell_counts(estimates)}),
        );
    }
    // Publishing earlier removed neighborhood queries and their useful seasons.
    // Keep the original witness allocation, then fill only remaining zeros.
    let mut joint_found = 0;
    for (cell, result) in pending_joint {
        let est = &mut estimates[cell.index(model.n)];
        if est.probability == 0. && !est.reachability.starts_with("impossible") {
            if result.probability > 0. {
                apply(est, &result, "matched_point_pool_joint_caps");
                witnesses += 1;
                joint_found += 1;
            } else {
                if est.zero_hit_upper_95 <= 0. || result.mass < est.zero_hit_upper_95 {
                    est.zero_hit_upper_95 = result.mass;
                }
                if result.witness.is_some() {
                    est.reachability = "witness".into();
                }
            }
        }
    }
    report(
        "joint_caps_publish",
        serde_json::json!({"enabled":enabled("RUST_ODDS_JOINT_CAP_CONDITIONING"),
        "accepted":joint_found,"cells":crate::logging::cell_counts(estimates)}),
    );
    let tail_seasons: Vec<_> = if matches!(
        crate::rare_tail::profile().as_str(),
        "lazy" | "portfolio" | "coverage"
    ) {
        assignments
            .iter()
            .chain(&construction.deferred)
            .map(|o| (o.clone(), crate::conditioned::canonical_ranks(model, o)))
            .chain(
                construction
                    .goal_witnesses
                    .iter()
                    .map(|c| (c.outcomes.clone(), c.ranks.clone())),
            )
            .collect()
    } else {
        Vec::new()
    };
    let (found, spent) = crate::rare_tail::run(
        model,
        seed,
        workers,
        estimates,
        log,
        &tail_seasons,
        search_start.elapsed().as_secs_f64() * 1000.,
    );
    witnesses += found;
    work += spent;
    report(
        "rare_tail",
        serde_json::json!({"accepted":found,"work":spent,"cells":crate::logging::cell_counts(estimates)}),
    );
    if witnesses > 0 {
        if !reconcile(model, estimates, &baseline) {
            report(
                "reconcile",
                serde_json::json!({"rolled_back":true,"cells":crate::logging::cell_counts(estimates)}),
            );
            return work;
        }
    }
    report(
        "reconcile",
        serde_json::json!({"rolled_back":false,"cells":crate::logging::cell_counts(estimates)}),
    );
    work
}

const PACKED_WITNESS_NODE_BUDGET: usize = 1000;
const PACKED_WITNESS_TOTAL_NODES: usize = 6000;
const PACKED_REUSE_SEED_LIMIT: usize = 64;
const PACKED_REUSE_PER_CELL_LIMIT: usize = 8;
const PACKED_REUSE_TOTAL_LIMIT: usize = 128;

#[derive(Default, Debug, serde::Serialize)]
struct PackedWitnessStats {
    supported: bool,
    candidates: usize,
    attempts: usize,
    nodes: usize,
    cohorts: usize,
    cohort_work: usize,
    cohort_work_exhausted: usize,
    cohort_branch_visits: usize,
    cohort_pruned_branches: usize,
    per_cell_work_limit: usize,
    request_work_limit: usize,
    certificates: usize,
    labels: usize,
    refutations: usize,
    reuse_enabled: bool,
    reuse_seeds: usize,
    reuse_prefix_screened_pairs: usize,
    reuse_attempts: usize,
    reuse_certificates: usize,
    reuse_labels: usize,
    reuse_global_cap_reached: bool,
    reuse_cell_cap_reached: usize,
}

#[derive(Clone)]
struct PackedReuseSeed {
    outcomes: Vec<u8>,
    ranks: Vec<usize>,
    bands: Vec<(usize, usize)>,
}

fn packed_witness_supported(model: &Model) -> bool {
    use crate::model::Key;
    let championship = &model.request.phase.championship;
    if model.n > 64
        || model.ids.len() > 64
        || model.request.phase.bonus_points != 0
        || (
            championship.point_win,
            championship.point_draw,
            championship.point_loss,
        ) != (3, 1, 0)
        || model.keys.first() != Some(&Key::Pt)
        || model.fixtures.iter().any(|fixture| {
            !fixture.home_sampler.mean.is_finite()
                || !fixture.away_sampler.mean.is_finite()
                || fixture.home_sampler.mean <= 0.
                || fixture.away_sampler.mean <= 0.
                || fixture.home == fixture.away
        })
    {
        return false;
    }
    let score_start = 1 + usize::from(model.keys.get(1) == Some(&Key::W));
    matches!(model.keys.get(score_start), Some(Key::Gd | Key::Gf))
        && model.keys[score_start..]
            .iter()
            .enumerate()
            .all(|(offset, key)| {
                matches!(
                    key,
                    Key::Gd | Key::Gf | Key::Gp | Key::Away | Key::W | Key::Name
                ) || (*key == Key::Bias && score_start + offset + 1 == model.keys.len())
            })
}

fn choose_supported_outcomes(model: &Model, domains: &[u8]) -> Option<Vec<u8>> {
    if domains.len() != model.fixtures.len() {
        return None;
    }
    let mut outcomes = Vec::with_capacity(domains.len());
    for (fixture, &domain) in model.fixtures.iter().zip(domains) {
        let mut selected = None;
        for outcome in 0..3 {
            let probability = fixture.prob[outcome];
            if domain & (1 << outcome) != 0
                && probability.is_finite()
                && probability > 0.
                && selected.is_none_or(|(_, best)| probability > best)
            {
                selected = Some((outcome, probability));
            }
        }
        outcomes.push(selected?.0 as u8);
    }
    Some(outcomes)
}

enum PackedSearchResult {
    Witness(crate::goal_completion::Certificate),
    Impossible,
    Unknown,
}

fn verified_packed_certificate(
    model: &Model,
    proof: &crate::rank_proof::RankProof,
    cell: Cell,
    required_count: usize,
    budget: usize,
    work_budget: usize,
    stats: &mut PackedWitnessStats,
) -> PackedSearchResult {
    if budget == 0 {
        return PackedSearchResult::Unknown;
    }
    stats.attempts += 1;
    let search = proof.feasible_domains_with_work_limits(
        cell,
        required_count,
        budget,
        crate::rank_proof::Options::default(),
        crate::rank_proof::WorkLimits {
            max_work: work_budget,
            max_depth: 512,
        },
    );
    stats.nodes += search.stats.nodes;
    stats.cohorts += search.stats.cohorts;
    stats.cohort_work += search.stats.cohort_work;
    stats.cohort_work_exhausted += usize::from(search.stats.cohort_work_exhausted);
    stats.cohort_branch_visits += search.stats.cohort_branch_visits;
    stats.cohort_pruned_branches += search.stats.cohort_pruned_branches;
    if search.impossible {
        return PackedSearchResult::Impossible;
    }
    let Some(domains) = search.domains.as_deref() else {
        return PackedSearchResult::Unknown;
    };
    let Some(outcomes) = choose_supported_outcomes(model, domains) else {
        return PackedSearchResult::Unknown;
    };
    if let Some(certificate) =
        crate::goal_completion::complete_with_paths(model, &outcomes, cell, true)
    {
        // Reachability is mathematical: every nonnegative integer score has
        // positive probability under the positive finite Poisson means checked
        // by packed_witness_supported. Native CDF support only affects finite
        // sampling and must not decide whether the ranked cell is reachable.
        if crate::goal_completion::verify(model, &certificate, cell) {
            return PackedSearchResult::Witness(certificate);
        }
    }
    let certificate =
        crate::goal_completion::complete_with_supported_margins(model, &outcomes, cell);
    let Some(certificate) = certificate else {
        return PackedSearchResult::Unknown;
    };
    if !crate::goal_completion::verify(model, &certificate, cell) {
        return PackedSearchResult::Unknown;
    }
    PackedSearchResult::Witness(certificate)
}

fn packed_witness_candidate(estimate: &Estimate) -> bool {
    estimate.probability == 0.
        && (estimate.reachability.is_empty() || estimate.reachability == "undecided")
}

fn packed_witness_fallback(model: &Model, estimates: &mut [Estimate]) -> PackedWitnessStats {
    packed_witness_fallback_with_options(
        model,
        estimates,
        enabled("RUST_ODDS_PACKED_WITNESS"),
        enabled("RUST_ODDS_PACKED_WITNESS_REUSE"),
    )
}

#[cfg(test)]
fn packed_witness_fallback_with_enabled(
    model: &Model,
    estimates: &mut [Estimate],
    feature_enabled: bool,
) -> PackedWitnessStats {
    packed_witness_fallback_with_options(model, estimates, feature_enabled, true)
}

fn packed_witness_fallback_with_options(
    model: &Model,
    estimates: &mut [Estimate],
    feature_enabled: bool,
    reuse_enabled: bool,
) -> PackedWitnessStats {
    let per_cell_work_limit = 100_000usize.saturating_mul(model.n + 1);
    packed_witness_fallback_with_work_limits(
        model,
        estimates,
        feature_enabled,
        reuse_enabled,
        per_cell_work_limit,
        6usize.saturating_mul(per_cell_work_limit),
    )
}

fn packed_witness_fallback_with_work_limits(
    model: &Model,
    estimates: &mut [Estimate],
    feature_enabled: bool,
    reuse_enabled: bool,
    per_cell_work_limit: usize,
    request_work_limit: usize,
) -> PackedWitnessStats {
    let mut stats = PackedWitnessStats {
        reuse_enabled,
        ..PackedWitnessStats::default()
    };
    if !feature_enabled {
        return stats;
    }
    stats.supported = packed_witness_supported(model);
    stats.per_cell_work_limit = per_cell_work_limit;
    stats.request_work_limit = request_work_limit;
    if !stats.supported || estimates.len() != model.n * model.n {
        return stats;
    }
    let mut cells: Vec<_> = (0..model.n)
        .flat_map(|team| (0..model.n).map(move |rank| Cell { team, rank }))
        .filter(|cell| {
            let estimate = &estimates[cell.index(model.n)];
            packed_witness_candidate(estimate)
        })
        .collect();
    cells.sort_by_key(|cell| {
        (
            cell.rank.min(model.n - 1 - cell.rank),
            model.ids[cell.team],
            cell.rank,
        )
    });
    stats.candidates = cells.len();
    if cells.is_empty() {
        return stats;
    }
    let Some(primary) = crate::rank_proof::RankProof::new(model) else {
        return stats;
    };
    let opposite = primary.negated();
    let mut reuse_seeds: Vec<PackedReuseSeed> = Vec::new();
    for cell in cells {
        if stats.nodes >= PACKED_WITNESS_TOTAL_NODES
            || stats.cohort_work >= stats.request_work_limit
        {
            break;
        }
        let estimate = &estimates[cell.index(model.n)];
        if !packed_witness_candidate(estimate) {
            continue;
        }
        let cell_budget = PACKED_WITNESS_NODE_BUDGET.min(PACKED_WITNESS_TOTAL_NODES - stats.nodes);
        let cell_work_budget = stats
            .per_cell_work_limit
            .min(stats.request_work_limit.saturating_sub(stats.cohort_work));
        let mut remaining = cell_budget;
        let mut remaining_work = cell_work_budget;
        let use_opposite = cell.rank < model.n / 2;
        let preferred = if !use_opposite { &primary } else { &opposite };
        let preferred_required_count = if use_opposite {
            model.n - 1 - cell.rank
        } else {
            cell.rank
        };
        let alternate = if !use_opposite { &opposite } else { &primary };
        let alternate_required_count = if use_opposite {
            cell.rank
        } else {
            model.n - 1 - cell.rank
        };
        let nodes_before = stats.nodes;
        let work_before = stats.cohort_work;
        let mut result = verified_packed_certificate(
            model,
            preferred,
            cell,
            preferred_required_count,
            remaining,
            remaining_work,
            &mut stats,
        );
        remaining = cell_budget.saturating_sub(stats.nodes - nodes_before);
        remaining_work = cell_work_budget.saturating_sub(stats.cohort_work - work_before);
        if matches!(&result, PackedSearchResult::Unknown) && remaining > 0 && remaining_work > 0 {
            result = verified_packed_certificate(
                model,
                alternate,
                cell,
                alternate_required_count,
                remaining,
                remaining_work,
                &mut stats,
            );
        }
        match result {
            PackedSearchResult::Impossible => {
                let estimate = &mut estimates[cell.index(model.n)];
                estimate.reachability = "impossible_by_joint_rank".into();
                estimate.zero_hit_upper_95 = 0.;
                stats.refutations += 1;
            }
            PackedSearchResult::Unknown => {}
            PackedSearchResult::Witness(certificate) => {
                stats.certificates += 1;
                if reuse_enabled {
                    packed_reuse_seed_push(
                        model,
                        &mut reuse_seeds,
                        &certificate.outcomes,
                        &certificate.ranks,
                    );
                }
                for (team, &rank) in certificate.ranks.iter().enumerate() {
                    let estimate = &mut estimates[team * model.n + rank];
                    if packed_witness_candidate(estimate) {
                        estimate.reachability = "reachable_by_construction".into();
                        stats.labels += 1;
                    }
                }
            }
        }
    }
    if reuse_enabled && !reuse_seeds.is_empty() {
        stats.reuse_seeds = reuse_seeds.len();
        packed_witness_reuse(model, estimates, &reuse_seeds, &mut stats);
    }
    stats
}

fn packed_prefix_rank_bands(model: &Model, outcomes: &[u8]) -> Vec<(usize, usize)> {
    let mut campaigns = model.base.clone();
    for (index, &outcome) in outcomes.iter().enumerate() {
        model.add(
            &mut campaigns,
            index,
            crate::conditioned::canonical(outcome),
        );
    }
    let wins = model.keys.get(1) == Some(&Key::W);
    let key = |team: usize| {
        (
            campaigns[team].points,
            if wins { campaigns[team].wins } else { 0 },
        )
    };
    (0..model.n)
        .map(|team| {
            let own = key(team);
            let strictly_ahead = (0..model.n).filter(|&other| key(other) > own).count();
            let tied = (0..model.n).filter(|&other| key(other) == own).count();
            (strictly_ahead, strictly_ahead + tied - 1)
        })
        .collect()
}

fn packed_witness_reuse(
    model: &Model,
    estimates: &mut [Estimate],
    seeds: &[PackedReuseSeed],
    stats: &mut PackedWitnessStats,
) {
    packed_witness_reuse_with_limits(
        model,
        estimates,
        seeds,
        stats,
        PACKED_REUSE_PER_CELL_LIMIT,
        PACKED_REUSE_TOTAL_LIMIT,
    );
}

fn packed_witness_reuse_with_limits(
    model: &Model,
    estimates: &mut [Estimate],
    seeds: &[PackedReuseSeed],
    stats: &mut PackedWitnessStats,
    per_cell_limit: usize,
    total_limit: usize,
) {
    let mut cells: Vec<Cell> = (0..model.n)
        .flat_map(|team| (0..model.n).map(move |rank| Cell { team, rank }))
        .filter(|cell| packed_witness_candidate(&estimates[cell.index(model.n)]))
        .collect();
    cells.sort_by_key(|cell| {
        (
            cell.rank.min(model.n - 1 - cell.rank),
            model.ids[cell.team],
            cell.rank,
        )
    });
    let mut eligible: Vec<Vec<usize>> = Vec::with_capacity(cells.len());
    for cell in &cells {
        let mut order: Vec<_> = seeds
            .iter()
            .enumerate()
            .filter(|(_, seed)| {
                (seed.bands[cell.team].0..=seed.bands[cell.team].1).contains(&cell.rank)
            })
            .map(|(index, _)| index)
            .collect();
        stats.reuse_prefix_screened_pairs += seeds.len();
        order.sort_by_key(|&index| (seeds[index].ranks[cell.team].abs_diff(cell.rank), index));
        eligible.push(order);
    }
    let mut calls = vec![0usize; cells.len()];
    let mut positions = vec![0usize; cells.len()];
    let mut total = 0;
    loop {
        let mut progressed = false;
        for index in 0..cells.len() {
            if calls[index] >= per_cell_limit
                || !packed_witness_candidate(&estimates[cells[index].index(model.n)])
                || positions[index] >= eligible[index].len()
                || total >= total_limit
            {
                continue;
            }
            let seed = &seeds[eligible[index][positions[index]]];
            positions[index] += 1;
            calls[index] += 1;
            total += 1;
            progressed = true;
            stats.reuse_attempts += 1;
            if let Some(certificate) = crate::goal_completion::complete_with_paths(
                model,
                &seed.outcomes,
                cells[index],
                true,
            ) {
                if crate::goal_completion::verify(model, &certificate, cells[index]) {
                    stats.reuse_certificates += 1;
                    for (team, &rank) in certificate.ranks.iter().enumerate() {
                        let estimate = &mut estimates[team * model.n + rank];
                        if packed_witness_candidate(estimate) {
                            estimate.reachability = "reachable_by_construction".into();
                            stats.reuse_labels += 1;
                        }
                    }
                }
            }
        }
        if !progressed {
            break;
        }
    }
    let pending = |index: usize| {
        packed_witness_candidate(&estimates[cells[index].index(model.n)])
            && positions[index] < eligible[index].len()
    };
    // Attribute each pending cell to its per-cell cap first; the global flag
    // records only pending cells that still had per-cell budget available.
    stats.reuse_cell_cap_reached = (0..cells.len())
        .filter(|&index| calls[index] >= per_cell_limit && pending(index))
        .count();
    stats.reuse_global_cap_reached = total >= total_limit
        && (0..cells.len()).any(|index| pending(index) && calls[index] < per_cell_limit);
}

fn packed_reuse_seed_push(
    model: &Model,
    seeds: &mut Vec<PackedReuseSeed>,
    outcomes: &[u8],
    ranks: &[usize],
) -> bool {
    if seeds.len() >= PACKED_REUSE_SEED_LIMIT || seeds.iter().any(|seed| seed.outcomes == outcomes)
    {
        return false;
    }
    seeds.push(PackedReuseSeed {
        outcomes: outcomes.to_vec(),
        ranks: ranks.to_vec(),
        bands: packed_prefix_rank_bands(model, outcomes),
    });
    true
}

fn reconcile(model: &Model, estimates: &mut [Estimate], baseline: &[Estimate]) -> bool {
    // A rank-support proof remains authoritative through every reconciliation
    // path, including candidates whose baseline came from a borrowed donor.
    for estimate in estimates.iter_mut() {
        if estimate.reachability == "impossible_by_rank_support" {
            estimate.probability = 0.;
            estimate.zero_hit_upper_95 = 0.;
        }
    }
    let original: Vec<_> = estimates.iter().map(|e| e.probability).collect();
    let mut matrix = original.clone();
    if !crate::pool::balance(&mut matrix, model.n) {
        let Some(additive) = crate::pool::balance_additive(&original, model.n) else {
            restore_baseline_preserving_reachability(estimates, baseline);
            return false;
        };
        matrix = additive;
    }
    if !crate::pool::valid(&matrix, model.n) {
        restore_baseline_preserving_reachability(estimates, baseline);
        return false;
    }
    for (e, p) in estimates.iter_mut().zip(matrix) {
        if e.reachability == "impossible_by_rank_support" {
            e.probability = 0.;
            e.zero_hit_upper_95 = 0.;
            continue;
        }
        if (e.reachability == "witness"
            || e.design == "outcome_path_stratified"
            || e.design == "native_full_world_complement"
            || e.design
                .starts_with("matched_point_pool_rare_tail_rank_rescue"))
            && e.probability > 0.
        {
            e.std_err *= p / e.probability;
            e.relative_se = Some(e.std_err / p);
        }
        e.probability = p;
    }
    true
}

fn restore_baseline_preserving_reachability(estimates: &mut [Estimate], baseline: &[Estimate]) {
    let candidate = estimates.to_vec();
    estimates.clone_from_slice(baseline);
    for ((restored, candidate), original) in estimates.iter_mut().zip(candidate).zip(baseline) {
        restored.native_cohort_lower =
            [restored.native_cohort_lower, candidate.native_cohort_lower]
                .into_iter()
                .filter(|value| value.is_finite() && *value >= 0.)
                .fold(0.0, f64::max);
        // Preserve baseline proofs and direct observations. A new proof may
        // override a pooled positive whose own native stream saw no instance.
        if original.reachability.starts_with("impossible") {
            restored.probability = 0.;
            restored.zero_hit_upper_95 = 0.;
            restored.pool_guide = 0.;
            continue;
        }
        if has_direct_evidence(original) {
            continue;
        }
        if candidate.reachability.starts_with("impossible") {
            restored.probability = 0.;
            restored.reachability = candidate.reachability;
            restored.zero_hit_upper_95 = 0.;
            restored.pool_guide = 0.;
            restored.native_cohort_lower = 0.;
        } else if candidate.reachability == "witness"
            || candidate.reachability == "reachable_by_construction"
        {
            restored.reachability = candidate.reachability;
        }
    }
}

fn has_direct_evidence(estimate: &Estimate) -> bool {
    estimate.reachability == "witness"
        || estimate.reachability == "reachable_by_construction"
        || estimate.reachability == "certain_by_points"
        || estimate.design == "exact_caps_conditioned"
        || (estimate.samples > 0
            && estimate.hits > 0
            && matches!(estimate.design.as_str(), "plain_mc" | "matched_point_pool"))
}

#[cfg(test)]
mod tail_fallback_eligibility_tests {
    use super::{run_tail_fallback, tail_fallback_mode, tail_fallback_supported};
    use crate::model::{Model, Request};
    use crate::pool::Estimate;
    use serde_json::json;

    fn group(n: usize, games: Vec<serde_json::Value>, points: [i32; 3]) -> Model {
        let request: Request = serde_json::from_value(json!({
            "id": 7305,
            "phase": {"sort":"pt,gd", "championship":{"point_win":points[0],"point_draw":points[1],"point_loss":points[2]}},
            "team_groups": (0..n).map(|team| json!({"team_id":team,"add_sub":0})).collect::<Vec<_>>(),
            "games": games
        })).unwrap();
        Model::new(request).unwrap()
    }

    fn one_game() -> Vec<serde_json::Value> {
        vec![json!({"id":1,"home_id":0,"away_id":1,"home_power":0.4,"away_power":0.5})]
    }

    fn tiny_fallback_model() -> Model {
        let request: Request = serde_json::from_value(json!({
            "id": 7306,
            "phase": {"sort":"pt,gd", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups": [
                {"team_id":0,"add_sub":0},
                {"team_id":1,"add_sub":0},
                {"team_id":2,"add_sub":-100}
            ],
            "games": [{"id":1,"home_id":0,"away_id":1,"home_power":0.4,"away_power":0.5}]
        }))
        .unwrap();
        Model::new(request).unwrap()
    }

    #[test]
    fn fallback_support_covers_small_groups_and_32_team_groups() {
        for n in [2, 3, 32, 64] {
            assert!(
                tail_fallback_supported(&group(n, one_game(), [3, 1, 0])),
                "n={n}"
            );
        }
        assert!(tail_fallback_supported(&group(1, vec![], [3, 1, 0])));
        assert!(!tail_fallback_supported(&group(65, vec![], [3, 1, 0])));
    }

    #[test]
    fn fallback_modes_cover_default_and_explicit_algorithms() {
        assert_eq!(
            tail_fallback_mode(None, "coverage"),
            Some("conditioned_then_particles")
        );
        assert_eq!(
            tail_fallback_mode(Some("conditioned"), "default"),
            Some("conditioned")
        );
        assert_eq!(
            tail_fallback_mode(Some("particles"), "default"),
            Some("particles")
        );
        assert_eq!(tail_fallback_mode(None, "default"), None);
    }

    #[test]
    fn fallback_support_rejects_extra_teams_nonstandard_rules_and_oversized_schedules() {
        let external = group(
            2,
            vec![json!({"id":1,"home_id":0,"away_id":2,"home_power":0.4,"away_power":0.5})],
            [3, 1, 0],
        );
        assert!(!tail_fallback_supported(&external));
        assert!(!tail_fallback_supported(&group(2, one_game(), [2, 1, 0])));
        let schedule = (0..257)
            .map(|id| json!({"id":id,"home_id":0,"away_id":1,"home_power":0.4,"away_power":0.5}))
            .collect();
        assert!(!tail_fallback_supported(&group(2, schedule, [3, 1, 0])));
    }

    fn unresolved_matrix(model: &Model) -> Vec<Estimate> {
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        estimates[0].reachability = "undecided".into();
        estimates[1] = Estimate {
            probability: 1.,
            design: "prior_estimate".into(),
            reachability: "witness".into(),
            ..Estimate::default()
        };
        estimates[model.n] = Estimate {
            probability: 1.,
            design: "prior_estimate".into(),
            reachability: "witness".into(),
            ..Estimate::default()
        };
        estimates[model.n + 1].reachability = "undecided".into();
        estimates[2].reachability = "impossible:fixed_last".into();
        estimates[model.n + 2].reachability = "impossible:fixed_last".into();
        estimates[2 * model.n].reachability = "impossible:fixed_last".into();
        estimates[2 * model.n + 1].reachability = "impossible:fixed_last".into();
        estimates[2 * model.n + 2].probability = 1.;
        estimates
    }

    #[test]
    fn tiny_default_fallback_resolves_a_cell_and_rolls_back_prior_bound_violation() {
        assert_eq!(
            tail_fallback_mode(None, &crate::rare_tail::profile()),
            Some("conditioned_then_particles")
        );
        let model = tiny_fallback_model();
        let mut estimates = unresolved_matrix(&model);
        let original_positive_design = estimates[1].design.clone();
        let original_impossible = estimates[2].reachability.clone();
        run_tail_fallback(&model, 731, 1, &mut estimates, None, &[], &[]);
        assert!(estimates[0].probability > 0.);
        assert_eq!(estimates[1].design, original_positive_design);
        assert_eq!(estimates[2].reachability, original_impossible);

        let mut constrained = unresolved_matrix(&model);
        let baseline = constrained.clone();
        run_tail_fallback(
            &model,
            732,
            1,
            &mut constrained,
            None,
            &[],
            &[(1, 0.99, 1.0)],
        );
        assert_eq!(
            serde_json::to_value(&constrained).unwrap(),
            serde_json::to_value(&baseline).unwrap()
        );
    }
}

#[cfg(test)]
mod saturated_points_proof_tests {
    use super::prove_saturated_by_points;
    use crate::{
        model::{Model, Request},
        pool::Estimate,
    };

    fn model(sort: &str, points: [i32; 3], rules: [i32; 3]) -> Model {
        Model::new(serde_json::from_value::<Request>(serde_json::json!({
            "id": 71,
            "phase": {"sort":sort,"championship":{"point_loss":rules[0],"point_draw":rules[1],"point_win":rules[2]}},
            "team_groups":[
                {"team_id":1,"add_sub":points[0]},
                {"team_id":2,"add_sub":points[1]},
                {"team_id":3,"add_sub":points[2]}
            ], "games":[]
        })).unwrap()).unwrap()
    }

    #[test]
    fn proves_strictly_separated_middle_rank() {
        let m = model("pt,gd,gf", [0, 10, 20], [0, 1, 3]);
        let mut estimates = vec![Estimate::default(); m.n * m.n];
        estimates[m.n + 1].probability = 1.;
        prove_saturated_by_points(&m, &mut estimates);
        assert_eq!(estimates[m.n + 1].design, "certain_by_points");
        assert_eq!(estimates[m.n + 1].reachability, "certain_by_points");
    }

    #[test]
    fn overlapping_points_intervals_do_not_certify_saturated_cell() {
        let m = model("pt,gd,gf", [0, 0, 0], [0, 1, 3]);
        let mut estimates = vec![Estimate::default(); m.n * m.n];
        estimates[0].probability = 1.;
        prove_saturated_by_points(&m, &mut estimates);
        assert_ne!(estimates[0].design, "certain_by_points");
    }

    #[test]
    fn negative_points_rules_skip_interval_proof() {
        let m = model("pt,gd,gf", [0, 10, 20], [-1, -1, -1]);
        let mut estimates = vec![Estimate::default(); m.n * m.n];
        estimates[m.n + 1].probability = 1.;
        prove_saturated_by_points(&m, &mut estimates);
        assert_ne!(estimates[m.n + 1].design, "certain_by_points");
    }
}

#[cfg(test)]
mod unsupported_bonus_point_search_tests {
    use super::{point_proofs_supported, structurally_possible_cells, Cell};
    use crate::{
        model::{Model, Request},
        pool::{Bounds, Estimate},
    };

    #[test]
    fn reachable_bonus_points_cell_skips_point_proofs_and_guided_sampling() {
        let request: Request = serde_json::from_value(serde_json::json!({
            "id": 72,
            "phase": {
                "sort":"pt,gd,gf",
                "bonus_points":1,
                "bonus_points_threshold":0,
                "championship":{"point_win":3,"point_draw":1,"point_loss":0}
            },
            "team_groups":[
                {"team_id":0,"add_sub":0},
                {"team_id":1,"add_sub":0},
                {"team_id":2,"add_sub":3}
            ],
            "games":[{"id":1,"home_id":0,"away_id":1,"home_power":0.4,"away_power":0.5}]
        }))
        .unwrap();
        let model = Model::new(request).unwrap();
        let target = Cell { team: 0, rank: 2 };
        assert!(Bounds::new(&model).rank_possible(target.team, target.rank));
        assert!(!point_proofs_supported(&model));
        assert!(crate::pool::point_pmfs(&model).is_none());

        let mut estimates = vec![Estimate::default(); model.n * model.n];
        estimates[target.index(model.n)].pool_guide = 0.25;
        let result = super::guided_points::estimate(
            &model,
            &Bounds::new(&model),
            &[],
            &[target],
            &mut estimates,
            19,
            1,
        );
        assert_eq!(result, (0, 0));
        assert_eq!(estimates[target.index(model.n)].probability, 0.0);
    }

    #[test]
    fn structural_zero_hit_proof_is_independent_of_pmfs_and_clears_upper_bound() {
        let request: Request = serde_json::from_value(serde_json::json!({
            "id": 73,
            "phase": {"sort":"pt,gd,gf","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":[
                {"team_id":0,"add_sub":0},
                {"team_id":1,"add_sub":3},
                {"team_id":2,"add_sub":6}
            ],
            "games":[]
        })).unwrap();
        let model = Model::new(request).unwrap();
        let impossible = Cell { team: 0, rank: 0 };
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        estimates[impossible.index(model.n)].zero_hit_upper_95 = 0.25;

        let cells = structurally_possible_cells(&model, &Bounds::new(&model), &mut estimates);

        assert!(!cells.contains(&impossible));
        assert!(estimates[impossible.index(model.n)]
            .reachability
            .starts_with("impossible"));
        assert_eq!(estimates[impossible.index(model.n)].zero_hit_upper_95, 0.0);
        assert_eq!(estimates[impossible.index(model.n)].pool_guide, 0.0);
    }

    #[test]
    fn rank_support_proof_overrides_borrowed_positive_and_survives_rollback() {
        let request: Request = serde_json::from_value(serde_json::json!({
            "id": 74,
            "phase": {"sort":"pt,gd,gf","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":[
                {"team_id":0,"add_sub":0},
                {"team_id":1,"add_sub":3},
                {"team_id":2,"add_sub":6}
            ],
            "games":[]
        })).unwrap();
        let model = Model::new(request).unwrap();
        let impossible = Cell { team: 0, rank: 0 };
        let index = impossible.index(model.n);
        let baseline = vec![Estimate::default(); model.n * model.n];
        let mut candidate = baseline.clone();
        candidate[index].probability = 0.2; // unsupported borrowed donor mass
        candidate[index].pool_guide = 0.2;
        candidate[index].zero_hit_upper_95 = 0.2;

        let _ = structurally_possible_cells(&model, &Bounds::new(&model), &mut candidate);
        assert_eq!(candidate[index].probability, 0.0);
        assert_eq!(candidate[index].zero_hit_upper_95, 0.0);
        let mut baseline_with_donor = baseline;
        baseline_with_donor[index].probability = 0.2;
        super::restore_baseline_preserving_reachability(&mut baseline_with_donor, &candidate);
        assert_eq!(baseline_with_donor[index].probability, 0.0);
        assert_eq!(baseline_with_donor[index].zero_hit_upper_95, 0.0);
        assert_eq!(
            baseline_with_donor[index].reachability,
            "impossible_by_rank_support"
        );
    }
}

#[cfg(test)]
mod fixed_mass_result_tests {
    use super::{initial, publish_fixed_mass_result, Cell};
    use crate::{
        model::{Model, Request},
        pool::{Bounds, Estimate},
    };

    #[test]
    fn production_conditional_hits_publish_effective_sample_quality() {
        let request: Request = serde_json::from_value(serde_json::json!({
            "id": 74,
            "phase": {"sort":"pt,gd,gf","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":[{"team_id":0,"add_sub":0},{"team_id":1,"add_sub":0}],
            "games":[{"id":1,"home_id":0,"away_id":1,"home_power":1.0,"away_power":1.0}]
        })).unwrap();
        let model = Model::new(request).unwrap();
        let pmfs = crate::pool::point_pmfs(&model).unwrap();
        let bounds = Bounds::new(&model);
        let cell = Cell { team: 0, rank: 0 };
        let result = initial(&model, &bounds, &pmfs, cell, 123).result;
        assert!(!result.weighted);
        assert!(result.hits >= 10, "{result:?}");

        let mut estimate = Estimate {
            samples: 100_000,
            hits: 0,
            ..Estimate::default()
        };
        publish_fixed_mass_result(&mut estimate, &result);

        assert_eq!(estimate.samples, 100_000);
        assert_eq!(estimate.hits, 0);
        assert_eq!(estimate.ess, result.hits as f64);
        assert_eq!(estimate.max_event_weight_share, 1. / result.hits as f64);
        assert!(estimate.meets_precision_goal);
        assert!(estimate.probability > 0.0);
    }
}

#[cfg(test)]
mod packed_witness_tests {
    use super::*;
    use crate::model::Request;
    use serde_json::Value;

    fn snapshot() -> Model {
        let request: Request = serde_json::from_str(include_str!(
            "../tests/fixtures/group-16498-2026-10-10.json"
        ))
        .unwrap();
        Model::new(request).unwrap()
    }

    fn numeric_values(estimates: &[Estimate]) -> Value {
        let mut estimates = estimates.to_vec();
        for estimate in &mut estimates {
            estimate.reachability.clear();
        }
        serde_json::to_value(estimates).unwrap()
    }

    #[test]
    fn reuse_prefix_bands_contain_the_verified_full_sorter_ranks() {
        let model = snapshot();
        let outcomes = vec![1; model.fixtures.len()];
        let ranks = crate::conditioned::canonical_ranks(&model, &outcomes);
        let bands = packed_prefix_rank_bands(&model, &outcomes);
        for team in 0..model.n {
            assert!((bands[team].0..=bands[team].1).contains(&ranks[team]));
        }
    }

    #[test]
    fn reuse_seed_cache_deduplicates_and_stops_at_its_bound() {
        let model = snapshot();
        let outcomes = vec![1; model.fixtures.len()];
        let ranks = crate::conditioned::canonical_ranks(&model, &outcomes);
        let mut seeds = Vec::new();
        assert!(packed_reuse_seed_push(
            &model, &mut seeds, &outcomes, &ranks
        ));
        assert!(!packed_reuse_seed_push(
            &model, &mut seeds, &outcomes, &ranks
        ));
        for marker in 0..PACKED_REUSE_SEED_LIMIT {
            let mut distinct = outcomes.clone();
            if distinct.is_empty() {
                continue;
            }
            let mut value = marker;
            for outcome in &mut distinct {
                *outcome = (value % 3) as u8;
                value /= 3;
            }
            if !seeds.iter().any(|seed| seed.outcomes == distinct) {
                let canonical_ranks = crate::conditioned::canonical_ranks(&model, &distinct);
                packed_reuse_seed_push(&model, &mut seeds, &distinct, &canonical_ranks);
            }
        }
        assert_eq!(seeds.len(), PACKED_REUSE_SEED_LIMIT);
        let mut sixty_fifth = outcomes.clone();
        let mut marker = PACKED_REUSE_SEED_LIMIT;
        for outcome in &mut sixty_fifth {
            *outcome = (marker % 3) as u8;
            marker /= 3;
        }
        assert!(!seeds.iter().any(|seed| seed.outcomes == sixty_fifth));
        let canonical_ranks = crate::conditioned::canonical_ranks(&model, &sixty_fifth);
        assert!(!packed_reuse_seed_push(
            &model,
            &mut seeds,
            &sixty_fifth,
            &canonical_ranks
        ));
    }

    #[test]
    fn reuse_zero_limits_preserve_unknown_numeric_metadata() {
        let model = snapshot();
        let cell = Cell { team: 0, rank: 0 };
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        estimates[cell.index(model.n)] = Estimate {
            std_err: 0.125,
            samples: 17,
            reachability: "undecided".into(),
            ..Estimate::default()
        };
        let before = numeric_values(&estimates);
        let ranks = crate::conditioned::canonical_ranks(&model, &vec![1; model.fixtures.len()]);
        let outcomes = vec![1; model.fixtures.len()];
        let seed = PackedReuseSeed {
            bands: packed_prefix_rank_bands(&model, &outcomes),
            outcomes,
            ranks,
        };
        let mut stats = PackedWitnessStats::default();
        packed_witness_reuse_with_limits(&model, &mut estimates, &[seed], &mut stats, 0, 0);
        assert_eq!(stats.reuse_attempts, 0);
        assert_eq!(estimates[cell.index(model.n)].reachability, "undecided");
        assert_eq!(numeric_values(&estimates), before);
    }

    #[test]
    fn reuse_positive_limits_skip_prefix_excluded_cells() {
        let model = snapshot();
        let outcomes = vec![1; model.fixtures.len()];
        let ranks = crate::conditioned::canonical_ranks(&model, &outcomes);
        let bands = packed_prefix_rank_bands(&model, &outcomes);
        let (team, rank) = (0..model.n)
            .flat_map(|team| (0..model.n).map(move |rank| (team, rank)))
            .find(|&(team, rank)| !(bands[team].0..=bands[team].1).contains(&rank))
            .expect("snapshot should have a prefix-excluded rank");
        let cell = Cell { team, rank };
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        for estimate in &mut estimates {
            estimate.reachability = "impossible_by_points".into();
        }
        estimates[cell.index(model.n)].reachability = "undecided".into();
        let seed = PackedReuseSeed {
            bands,
            outcomes,
            ranks,
        };
        let mut stats = PackedWitnessStats::default();
        packed_witness_reuse_with_limits(&model, &mut estimates, &[seed], &mut stats, 2, 2);
        assert_eq!(stats.reuse_attempts, 0);
        assert!(!stats.reuse_global_cap_reached);
        assert_eq!(stats.reuse_cell_cap_reached, 0);
        assert_eq!(estimates[cell.index(model.n)].reachability, "undecided");
    }

    #[test]
    fn reuse_success_preserves_other_labels_and_numeric_metadata() {
        let model = snapshot();
        let outcomes = vec![1; model.fixtures.len()];
        let ranks = crate::conditioned::canonical_ranks(&model, &outcomes);
        let target = Cell {
            team: 0,
            rank: ranks[0],
        };
        let certificate =
            crate::goal_completion::complete_with_paths(&model, &outcomes, target, true)
                .expect("seed completion should construct the selected canonical rank");
        assert!(crate::goal_completion::verify(&model, &certificate, target));
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        for estimate in &mut estimates {
            estimate.reachability = "impossible_by_points".into();
        }
        estimates[target.index(model.n)] = Estimate {
            std_err: 0.125,
            samples: 17,
            relative_se: Some(0.25),
            reachability: "undecided".into(),
            ..Estimate::default()
        };
        let positive = Cell {
            team: 1,
            rank: ranks[1],
        };
        estimates[positive.index(model.n)] = Estimate {
            probability: 0.01,
            reachability: "pool_positive".into(),
            ..Estimate::default()
        };
        let witness = Cell {
            team: 2,
            rank: ranks[2],
        };
        estimates[witness.index(model.n)].reachability = "reachable_by_construction".into();
        let before = estimates.clone();
        let seed = PackedReuseSeed {
            bands: packed_prefix_rank_bands(&model, &outcomes),
            outcomes,
            ranks,
        };
        let mut stats = PackedWitnessStats::default();
        packed_witness_reuse_with_limits(&model, &mut estimates, &[seed], &mut stats, 8, 8);
        assert!(stats.reuse_attempts > 0);
        assert_eq!(
            estimates[target.index(model.n)].reachability,
            "reachable_by_construction"
        );
        assert_eq!(
            estimates[positive.index(model.n)].reachability,
            "pool_positive"
        );
        assert_eq!(
            estimates[witness.index(model.n)].reachability,
            "reachable_by_construction"
        );
        assert_eq!(numeric_values(&estimates), numeric_values(&before));
    }

    #[test]
    fn reuse_cap_stats_count_only_remaining_eligible_work() {
        let model = snapshot();
        let outcomes = vec![1; model.fixtures.len()];
        let ranks = crate::conditioned::canonical_ranks(&model, &outcomes);
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        let bands = packed_prefix_rank_bands(&model, &outcomes);
        for team in 0..model.n {
            let rank = (bands[team].0..=bands[team].1).find(|&rank| rank != ranks[team]);
            let Some(rank) = rank else { continue };
            let cell = Cell { team, rank };
            estimates[cell.index(model.n)].reachability = "undecided".into();
        }
        let seed = PackedReuseSeed {
            bands,
            outcomes,
            ranks,
        };
        let mut stats = PackedWitnessStats::default();
        packed_witness_reuse_with_limits(&model, &mut estimates, &[seed], &mut stats, 1, 1);
        assert_eq!(stats.reuse_attempts, 1);
        assert!(stats.reuse_global_cap_reached);
        assert_eq!(stats.reuse_cell_cap_reached, 0);
    }

    #[test]
    fn packed_witness_accepts_verified_palmeiras_completion_outside_native_support() {
        let model = snapshot();
        let team = model.indices[&16];
        let target_index = Cell { team, rank: 12 }.index(model.n);
        let positive_index = Cell { team, rank: 13 }.index(model.n);
        let impossible_index = Cell { team, rank: 14 }.index(model.n);
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        for estimate in &mut estimates {
            estimate.reachability = "impossible_by_points".into();
        }
        estimates[target_index] = Estimate {
            std_err: 0.002,
            samples: 1234,
            hits: 2,
            ess: 4.5,
            mean_weight: 1.25,
            work_spent: 9876,
            available: true,
            meets_precision_goal: false,
            relative_se: Some(0.2),
            max_event_weight_share: 0.33,
            zero_hit_upper_95: 0.14,
            design: "test_design".into(),
            reachability: "undecided".into(),
            conditional_mass: 0.5,
            conditional_samples: 5,
            conditional_hits: 1,
            ..Estimate::default()
        };
        estimates[positive_index] = Estimate {
            probability: 0.01,
            reachability: "pool_positive".into(),
            ..Estimate::default()
        };
        estimates[impossible_index].reachability = "impossible_by_joint_points".into();
        let original = estimates.clone();

        let proof = crate::rank_proof::RankProof::new(&model).unwrap();
        let search = proof.feasible_domains(
            Cell { team, rank: 12 },
            12,
            PACKED_WITNESS_NODE_BUDGET,
            crate::rank_proof::Options::default(),
        );
        let outcomes =
            choose_supported_outcomes(&model, search.domains.as_deref().unwrap()).unwrap();
        let raw = crate::goal_completion::complete_with_paths(
            &model,
            &outcomes,
            Cell { team, rank: 12 },
            true,
        )
        .unwrap();
        assert!(crate::goal_completion::verify(
            &model,
            &raw,
            Cell { team, rank: 12 }
        ));
        assert!(!crate::goal_completion::has_native_score_support(
            &model, &raw
        ));

        let mut certificate_stats = PackedWitnessStats::default();
        let accepted = verified_packed_certificate(
            &model,
            &proof,
            Cell { team, rank: 12 },
            12,
            PACKED_WITNESS_NODE_BUDGET,
            usize::MAX,
            &mut certificate_stats,
        );
        let accepted = match accepted {
            PackedSearchResult::Witness(certificate) => certificate,
            PackedSearchResult::Impossible => panic!("expected verified witness, got refutation"),
            PackedSearchResult::Unknown => panic!("expected verified witness, got unknown search"),
        };
        assert!(crate::goal_completion::verify(
            &model,
            &accepted,
            Cell { team, rank: 12 }
        ));
        assert!(!crate::goal_completion::has_native_score_support(
            &model, &accepted
        ));

        let disabled = packed_witness_fallback_with_enabled(&model, &mut estimates, false);
        assert_eq!(disabled.labels, 0);
        assert_eq!(
            serde_json::to_value(&estimates).unwrap(),
            serde_json::to_value(&original).unwrap()
        );

        let stats = packed_witness_fallback_with_enabled(&model, &mut estimates, true);
        assert!(stats.supported);
        assert_eq!(stats.candidates, 1);
        assert!(stats.attempts <= 2);
        assert!(stats.nodes <= PACKED_WITNESS_NODE_BUDGET);
        assert_eq!(stats.certificates, 1, "{stats:?}");
        assert_eq!(stats.labels, 1, "{stats:?}");
        assert_eq!(
            estimates[target_index].reachability,
            "reachable_by_construction"
        );
        assert_eq!(estimates[positive_index].reachability, "pool_positive");
        assert_eq!(
            estimates[impossible_index].reachability,
            "impossible_by_joint_points"
        );
        assert_eq!(numeric_values(&estimates), numeric_values(&original));
    }

    #[test]
    fn packed_witness_accepts_group_17045_rank_32_beyond_native_score_support() {
        let request: Request = serde_json::from_str(include_str!(
            "../tests/fixtures/group-17045-2026-11-04.json"
        ))
        .unwrap();
        let model = Model::new(request).unwrap();
        let team = model.indices[&290];
        let cell = Cell { team, rank: 32 };
        let proof = crate::rank_proof::RankProof::new(&model).unwrap();
        let search = proof.feasible_domains_with_limits(
            cell,
            32,
            1000,
            crate::rank_proof::Options::default(),
            crate::rank_proof::Limits {
                max_cohorts: 100_000,
                max_depth: 512,
            },
        );
        let outcomes = choose_supported_outcomes(&model, search.domains.as_deref().unwrap())
            .expect("all relaxed fixture domains should have positive-probability outcomes");
        let raw = crate::goal_completion::complete_with_paths(&model, &outcomes, cell, true)
            .expect("the relaxed assignment should admit an exact ranking completion");
        assert!(crate::goal_completion::verify(&model, &raw, cell));
        assert!(!crate::goal_completion::has_native_score_support(
            &model, &raw
        ));
        assert!(raw.adjustments.iter().any(|adjustment| {
            model.request.games[model.fixtures[adjustment.fixture].request_index].id == 370283
        }));

        let mut estimates = vec![Estimate::default(); model.n * model.n];
        for estimate in &mut estimates {
            estimate.reachability = "impossible_by_points".into();
        }
        let target_index = cell.index(model.n);
        estimates[target_index].reachability = "undecided".into();
        let original = estimates.clone();
        let stats = packed_witness_fallback_with_enabled(&model, &mut estimates, true);

        assert!(stats.supported);
        assert_eq!(stats.candidates, 1);
        assert_eq!(stats.certificates, 1, "{stats:?}");
        assert_eq!(stats.labels, 1, "{stats:?}");
        assert_eq!(
            estimates[target_index].reachability,
            "reachable_by_construction"
        );
        assert_eq!(numeric_values(&estimates), numeric_values(&original));
        assert!(estimates
            .iter()
            .enumerate()
            .all(|(index, estimate)| index == target_index
                || estimate.reachability == "impossible_by_points"));
    }

    #[test]
    fn packed_witness_publishes_only_exhaustive_group_17045_refutations() {
        let request: Request = serde_json::from_str(include_str!(
            "../tests/fixtures/group-17045-2026-11-04.json"
        ))
        .unwrap();
        let model = Model::new(request).unwrap();
        let cells = [(401, 32), (401, 33), (855, 32), (855, 33)];
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        for estimate in &mut estimates {
            estimate.reachability = "impossible_by_points".into();
        }
        let indices = cells.map(|(id, rank)| {
            Cell {
                team: model.indices[&id],
                rank,
            }
            .index(model.n)
        });
        for &index in &indices {
            estimates[index] = Estimate {
                probability: 0.,
                zero_hit_upper_95: 0.2,
                reachability: "undecided".into(),
                ..Estimate::default()
            };
        }
        let before = estimates.clone();
        let stats = packed_witness_fallback_with_enabled(&model, &mut estimates, true);
        assert_eq!(stats.refutations, 4, "{stats:?}");
        for &index in &indices {
            assert_eq!(estimates[index].reachability, "impossible_by_joint_rank");
            assert_eq!(estimates[index].zero_hit_upper_95, 0.);
            assert_eq!(estimates[index].probability, before[index].probability);
            assert_eq!(estimates[index].samples, before[index].samples);
            assert_eq!(estimates[index].work_spent, before[index].work_spent);
        }
        for (index, estimate) in estimates.iter().enumerate() {
            if !indices.contains(&index) {
                assert_eq!(estimate.reachability, before[index].reachability);
                assert_eq!(estimate.zero_hit_upper_95, before[index].zero_hit_upper_95);
            }
        }
    }

    #[test]
    fn packed_witness_constructs_group_17045_rank_24_within_node_budget() {
        let request: Request = serde_json::from_str(include_str!(
            "../tests/fixtures/group-17045-2026-11-04.json"
        ))
        .unwrap();
        let model = Model::new(request).unwrap();
        let cell = Cell {
            team: model.indices[&401],
            rank: 24,
        };
        let target = cell.index(model.n);
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        for estimate in &mut estimates {
            estimate.reachability = "impossible_by_points".into();
        }
        estimates[target].reachability = "undecided".into();
        let original = estimates.clone();
        let stats = packed_witness_fallback_with_enabled(&model, &mut estimates, true);
        assert_eq!(
            estimates[target].reachability, "reachable_by_construction",
            "{stats:?}"
        );
        assert_eq!(stats.certificates, 1, "{stats:?}");
        assert_eq!(stats.refutations, 0, "{stats:?}");
        assert!(stats.nodes <= PACKED_WITNESS_NODE_BUDGET, "{stats:?}");
        assert_eq!(numeric_values(&estimates), numeric_values(&original));
    }

    #[test]
    fn packed_witness_keeps_one_node_group_17045_rank_24_search_unknown() {
        let request: Request = serde_json::from_str(include_str!(
            "../tests/fixtures/group-17045-2026-11-04.json"
        ))
        .unwrap();
        let model = Model::new(request).unwrap();
        let cell = Cell {
            team: model.indices[&401],
            rank: 24,
        };
        let proof = crate::rank_proof::RankProof::new(&model).unwrap();
        let mut stats = PackedWitnessStats::default();
        let result =
            verified_packed_certificate(&model, &proof, cell, 24, 1, usize::MAX, &mut stats);

        assert!(matches!(result, PackedSearchResult::Unknown), "{stats:?}");
        assert!(stats.nodes <= 1, "{stats:?}");
        assert_eq!(stats.refutations, 0, "{stats:?}");
    }

    #[test]
    fn packed_witness_accepts_empty_unresolved_label_without_changing_numbers() {
        let model = tiny_points_goal_bias_model();
        let target = Cell { team: 0, rank: 0 }.index(model.n);
        let positive = Cell { team: 1, rank: 1 }.index(model.n);
        let impossible = Cell { team: 2, rank: 2 }.index(model.n);
        let witness = Cell { team: 1, rank: 0 }.index(model.n);
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        for estimate in &mut estimates {
            estimate.reachability = "impossible_by_points".into();
        }
        estimates[target] = Estimate::default();
        estimates[positive] = Estimate {
            probability: 0.01,
            reachability: "pool_positive".into(),
            ..Estimate::default()
        };
        estimates[impossible].reachability = "impossible_by_joint_points".into();
        estimates[witness].reachability = "witness".into();
        let before = numeric_values(&estimates);
        let baseline = estimates.clone();
        estimates[target].reachability = "undecided".into();
        restore_baseline_preserving_reachability(&mut estimates, &baseline);
        assert!(estimates[target].reachability.is_empty());

        let stats = packed_witness_fallback_with_enabled(&model, &mut estimates, true);

        assert!(stats.supported);
        assert!(stats.candidates > 0);
        assert!(stats.certificates > 0, "{stats:?}");
        assert_eq!(estimates[target].reachability, "reachable_by_construction");
        assert_eq!(estimates[positive].reachability, "pool_positive");
        assert_eq!(
            estimates[impossible].reachability,
            "impossible_by_joint_points"
        );
        assert_eq!(estimates[witness].reachability, "witness");
        assert_eq!(numeric_values(&estimates), before);
    }

    #[test]
    fn packed_witness_skips_unsupported_rules_and_zero_mean_fixtures() {
        let original = snapshot();
        let mut changed = original.request.clone();
        changed.phase.championship.point_win = 2;
        assert!(!packed_witness_supported(&Model::new(changed).unwrap()));

        let mut changed = original.request.clone();
        changed.phase.bonus_points = 1;
        assert!(!packed_witness_supported(&Model::new(changed).unwrap()));

        let mut changed = original.request.clone();
        changed.phase.sort = "pt,gd,gf,bias".into();
        assert!(packed_witness_supported(&Model::new(changed).unwrap()));

        let mut changed = original.request.clone();
        changed.phase.sort = "pt,gd,bias,gf".into();
        assert!(!packed_witness_supported(&Model::new(changed).unwrap()));

        let mut changed = original.request.clone();
        changed.phase.sort = "pt,head,gd,gf".into();
        assert!(!packed_witness_supported(&Model::new(changed).unwrap()));

        let mut changed = original.request.clone();
        changed.phase.sort = "pt,away,gd,gf".into();
        assert!(!packed_witness_supported(&Model::new(changed).unwrap()));

        let mut changed = original.request.clone();
        let future = changed.games.iter_mut().find(|game| !game.played).unwrap();
        future.home_power = 0.0;
        assert!(!packed_witness_supported(&Model::new(changed).unwrap()));
    }

    #[test]
    fn packed_witness_accepts_points_first_score_prefix_without_wins() {
        let original = snapshot();
        for sort in ["pt,gd,gf", "pt,gf,gd,bias", "pt,w,gd,gf,bias"] {
            let mut changed = original.request.clone();
            changed.phase.sort = sort.into();
            assert!(
                packed_witness_supported(&Model::new(changed).unwrap()),
                "{sort}"
            );
        }
    }

    #[test]
    fn packed_witness_accepts_goal_away_win_name_sort_and_rejects_away_prefix() {
        let request: Request = serde_json::from_str(include_str!(
            "../tests/fixtures/group-17045-2026-11-04.json"
        ))
        .unwrap();
        let model = Model::new(request).unwrap();
        assert_eq!(
            model.keys,
            vec![
                crate::model::Key::Pt,
                crate::model::Key::Gd,
                crate::model::Key::Gf,
                crate::model::Key::Away,
                crate::model::Key::W,
                crate::model::Key::Name,
            ]
        );
        assert!(packed_witness_supported(&model));

        let mut changed = model.request.clone();
        changed.phase.sort = "pt,g_away,gd,gf,w,name".into();
        assert!(!packed_witness_supported(&Model::new(changed).unwrap()));
    }

    #[test]
    fn points_goal_bias_fixture_builds_native_supported_full_world_without_numeric_changes() {
        let request: Request = serde_json::from_str(include_str!(
            "../tests/fixtures/rare_position_benchmarks/group-99003-dominant.json"
        ))
        .unwrap();
        let model = Model::new(request).unwrap();
        assert!(packed_witness_supported(&model));
        let outcomes = vec![1; model.fixtures.len()];
        let ranks = crate::conditioned::canonical_ranks(&model, &outcomes);
        let team = ranks.iter().position(|rank| *rank == 0).unwrap();
        let cell = Cell { team, rank: 0 };
        let certificate = crate::goal_completion::complete(&model, &outcomes, cell).unwrap();
        assert!(crate::goal_completion::verify(&model, &certificate, cell));
        assert!(crate::goal_completion::has_native_score_support(
            &model,
            &certificate
        ));
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        for estimate in &mut estimates {
            estimate.reachability = "undecided".into();
        }
        let original = estimates.clone();
        let stats = packed_witness_fallback_with_enabled(&model, &mut estimates, true);
        assert!(stats.supported, "{stats:?}");
        assert_eq!(numeric_values(&estimates), numeric_values(&original));
    }

    fn tiny_points_goal_bias_model() -> Model {
        let request = serde_json::json!({
            "id": 99004,
            "phase": {"sort":"pt,gd,gf,bias", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups": (0..3).map(|team| serde_json::json!({"team_id":team,"add_sub":0,"bias":team})).collect::<Vec<_>>(),
            "games": [
                {"id":1,"home_id":0,"away_id":1,"home_power":1.0,"away_power":1.0},
                {"id":2,"home_id":1,"away_id":0,"home_power":1.0,"away_power":1.0},
                {"id":3,"home_id":0,"away_id":2,"home_power":1.0,"away_power":1.0},
                {"id":4,"home_id":2,"away_id":0,"home_power":1.0,"away_power":1.0},
                {"id":5,"home_id":1,"away_id":2,"home_power":1.0,"away_power":1.0},
                {"id":6,"home_id":2,"away_id":1,"home_power":1.0,"away_power":1.0}
            ]
        });
        Model::new(serde_json::from_value(request).unwrap()).unwrap()
    }

    #[test]
    fn tiny_points_goal_bias_world_gets_a_bounded_native_supported_witness() {
        let model = tiny_points_goal_bias_model();
        assert!(packed_witness_supported(&model));
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        for estimate in &mut estimates {
            estimate.reachability = "undecided".into();
        }
        let original = estimates.clone();
        let stats = packed_witness_fallback_with_enabled(&model, &mut estimates, true);
        assert!(stats.certificates > 0, "{stats:?}");
        assert!(stats.nodes <= PACKED_WITNESS_TOTAL_NODES);
        assert_eq!(numeric_values(&estimates), numeric_values(&original));
        assert!(estimates
            .iter()
            .any(|estimate| estimate.reachability == "reachable_by_construction"));
    }

    #[test]
    fn real_goal_away_win_name_sort_builds_verified_witness_without_numeric_changes() {
        let mut model = tiny_points_goal_bias_model();
        model.request.phase.sort = "pt,gd,gf,g_away,w,name".into();
        let model = Model::new(model.request).unwrap();
        assert!(packed_witness_supported(&model));
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        for estimate in &mut estimates {
            estimate.reachability = "undecided".into();
        }
        let before = numeric_values(&estimates);

        let stats = packed_witness_fallback_with_enabled(&model, &mut estimates, true);

        assert!(stats.certificates > 0, "{stats:?}");
        assert!(stats.nodes <= PACKED_WITNESS_TOTAL_NODES, "{stats:?}");
        assert!(estimates
            .iter()
            .any(|estimate| estimate.reachability == "reachable_by_construction"));
        assert_eq!(numeric_values(&estimates), before);
        for estimate in estimates
            .iter()
            .filter(|estimate| estimate.reachability == "reachable_by_construction")
        {
            // Labels are assigned only after certificate verification.
            assert_eq!(estimate.probability, 0.);
        }
    }

    #[test]
    fn opposite_packed_orientation_can_build_a_verified_top_rank_certificate() {
        let model = snapshot();
        let team = model.indices[&16];
        let cell = Cell { team, rank: 0 };
        let proof = crate::rank_proof::RankProof::new(&model).unwrap().negated();
        let mut stats = PackedWitnessStats::default();
        let certificate =
            verified_packed_certificate(&model, &proof, cell, 19, 100, usize::MAX, &mut stats);
        assert!(stats.nodes <= PACKED_WITNESS_NODE_BUDGET);
        match certificate {
            PackedSearchResult::Witness(certificate) => assert_eq!(certificate.ranks[team], 0),
            PackedSearchResult::Impossible => panic!("expected witness, got refutation"),
            PackedSearchResult::Unknown => panic!("expected witness, got unknown search"),
        }
    }

    #[test]
    fn packed_witness_respects_the_shared_node_budget() {
        let model = snapshot();
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        for estimate in &mut estimates {
            estimate.reachability = "undecided".into();
        }
        let stats = packed_witness_fallback_with_enabled(&model, &mut estimates, true);
        assert_eq!(stats.candidates, model.n * model.n);
        assert!(stats.nodes <= PACKED_WITNESS_TOTAL_NODES);
        assert!(stats.attempts > 0);
    }

    #[test]
    fn packed_work_budget_is_shared_across_orientations_and_request_cells() {
        let request: crate::model::Request = serde_json::from_str(include_str!(
            "../tests/fixtures/group-17047-2026-11-06.json"
        ))
        .unwrap();
        let model = Model::new(request).unwrap();
        let mut estimates = vec![
            Estimate {
                reachability: "impossible_by_points".into(),
                ..Estimate::default()
            };
            model.n * model.n
        ];
        let target_ids = [367, 809, 848];
        for id in target_ids {
            let team = model.indices[&id];
            estimates[Cell { team, rank: 29 }.index(model.n)] = Estimate {
                std_err: 0.125,
                samples: 17,
                reachability: "undecided".into(),
                zero_hit_upper_95: 0.2,
                ..Estimate::default()
            };
        }
        let before = estimates.clone();
        let stats =
            packed_witness_fallback_with_work_limits(&model, &mut estimates, true, false, 1, 2);
        assert_eq!(stats.attempts, 2);
        assert_eq!(stats.cohort_work, 2);
        assert_eq!(stats.cohort_work_exhausted, 2);
        assert_eq!(stats.nodes, 0);
        assert_eq!(stats.per_cell_work_limit, 1);
        assert_eq!(stats.request_work_limit, 2);
        assert_eq!(stats.certificates, 0);
        assert_eq!(stats.refutations, 0);
        assert_eq!(stats.labels, 0);
        for id in target_ids {
            let index = Cell {
                team: model.indices[&id],
                rank: 29,
            }
            .index(model.n);
            assert_eq!(estimates[index].reachability, "undecided");
            assert_eq!(estimates[index].probability, before[index].probability);
            assert_eq!(estimates[index].std_err, before[index].std_err);
            assert_eq!(estimates[index].samples, before[index].samples);
            assert_eq!(
                estimates[index].zero_hit_upper_95,
                before[index].zero_hit_upper_95
            );
        }
    }
}

#[cfg(test)]
mod rollback_reachability_tests {
    use super::restore_baseline_preserving_reachability;
    use crate::pool::Estimate;

    #[test]
    fn rollback_restores_numbers_and_keeps_independent_zero_mass_facts() {
        let baseline = vec![
            Estimate {
                probability: 0.,
                samples: 3,
                reachability: "undecided".into(),
                zero_hit_upper_95: 0.2,
                native_cohort_lower: 0.25,
                ..Estimate::default()
            },
            Estimate {
                probability: 0.,
                reachability: "impossible_by_points".into(),
                native_cohort_lower: 0.1,
                ..Estimate::default()
            },
            Estimate {
                probability: 0.4,
                reachability: "pool_positive".into(),
                pool_guide: 0.2,
                zero_hit_upper_95: 0.15,
                native_cohort_lower: 0.1,
                ..Estimate::default()
            },
            Estimate {
                probability: 0.,
                reachability: "witness".into(),
                ..Estimate::default()
            },
            Estimate {
                probability: 0.,
                reachability: "undecided".into(),
                zero_hit_upper_95: 0.15,
                ..Estimate::default()
            },
            Estimate {
                probability: 0.1,
                samples: 1000,
                hits: 1,
                design: "matched_point_pool".into(),
                reachability: "pool_positive".into(),
                ..Estimate::default()
            },
        ];
        let mut candidate = vec![
            Estimate {
                probability: 0.1,
                samples: 99,
                reachability: "witness".into(),
                zero_hit_upper_95: 0.,
                native_cohort_lower: 0.75,
                ..Estimate::default()
            },
            Estimate {
                probability: 0.2,
                reachability: "witness".into(),
                native_cohort_lower: 0.8,
                ..Estimate::default()
            },
            Estimate {
                probability: 0.0,
                reachability: "impossible_conflict".into(),
                ..Estimate::default()
            },
            Estimate {
                probability: 0.3,
                reachability: "impossible_wrong".into(),
                ..Estimate::default()
            },
            Estimate {
                probability: 0.,
                reachability: "impossible_new".into(),
                ..Estimate::default()
            },
            Estimate {
                probability: 0.,
                reachability: "impossible_direct_conflict".into(),
                ..Estimate::default()
            },
        ];
        restore_baseline_preserving_reachability(&mut candidate, &baseline);
        assert_eq!(candidate[0].probability, 0.);
        assert_eq!(candidate[0].samples, 3);
        assert_eq!(candidate[0].reachability, "witness");
        assert_eq!(candidate[0].zero_hit_upper_95, 0.2);
        assert_eq!(candidate[0].native_cohort_lower, 0.75);
        assert_eq!(candidate[1].reachability, "impossible_by_points");
        assert_eq!(candidate[1].native_cohort_lower, 0.8);
        assert_eq!(candidate[2].probability, 0.);
        assert_eq!(candidate[2].reachability, "impossible_conflict");
        assert_eq!(candidate[2].zero_hit_upper_95, 0.);
        assert_eq!(candidate[2].pool_guide, 0.);
        assert_eq!(candidate[2].native_cohort_lower, 0.);
        assert_eq!(candidate[3].reachability, "witness");
        assert_eq!(candidate[4].reachability, "impossible_new");
        assert_eq!(candidate[4].zero_hit_upper_95, 0.);
        assert_eq!(candidate[5].probability, 0.1);
        assert_eq!(candidate[5].hits, 1);
        assert_eq!(candidate[5].reachability, "pool_positive");
    }

    #[test]
    fn reconciliation_failure_uses_rollback_fact_merge() {
        let request: crate::model::Request = serde_json::from_str(include_str!(
            "../tests/fixtures/group-16498-2026-10-10.json"
        ))
        .unwrap();
        let model = crate::model::Model::new(request).unwrap();
        let baseline = vec![Estimate::default(); model.n * model.n];
        let mut candidate = baseline.clone();
        candidate[0].reachability = "reachable_by_construction".into();
        candidate[1].probability = 0.5;
        assert!(!super::reconcile(&model, &mut candidate, &baseline));
        assert_eq!(candidate[0].probability, 0.);
        assert_eq!(candidate[0].reachability, "reachable_by_construction");
        assert_eq!(candidate[1].probability, 0.);
        assert!(candidate[1].reachability.is_empty());
    }

    #[test]
    fn synthetic_shared_collapse_api_keeps_new_positives_proofs_and_balanced_matrix() {
        let request: crate::model::Request = serde_json::from_str(include_str!(
            "../../experiments/rare_positions/2026-10-01-shared-constraints/synthetic-shared-collapse.json"
        ))
        .unwrap();
        let (response, _) = crate::api::calculate(request, 808, 4, 20_000).unwrap();
        let estimates = response.rare_position_estimates;
        let mut matrix = Vec::new();
        for team in 0..6 {
            for rank in 0..6 {
                matrix.push(estimates[&(team as i32)][&rank].probability);
            }
        }
        assert!(crate::pool::valid(&matrix, 6));
        for team in 1..6 {
            let first = &estimates[&(team as i32)][&0];
            assert!(
                first.probability > 0.,
                "team={team} first_probability={}",
                first.probability
            );
            assert_eq!(first.reachability, "reachable");
        }
        let second = &estimates[&0][&1];
        assert!(
            second.probability > 0.,
            "team=0 second_probability={}",
            second.probability
        );
        assert_eq!(second.reachability, "reachable");
        for rank in 2..6 {
            let cell = &estimates[&0][&rank];
            assert_eq!(
                cell.probability, 0.,
                "rank={rank} probability={}",
                cell.probability
            );
            assert_eq!(
                cell.reachability, "impossible",
                "rank={rank} reachability={}",
                cell.reachability
            );
        }
    }
}
