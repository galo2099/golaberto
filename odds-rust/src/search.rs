use crate::{
    conditioned::{blockers, fast, Event, Result},
    joint_caps::JointProposal,
    lookahead::{self, Policy},
    model::Model,
    pool::{zero_upper, Bounds, Estimate},
    rng::derive,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Cell {
    pub team: usize,
    pub rank: usize,
}
impl Cell {
    pub fn index(self, n: usize) -> usize {
        self.team * n + self.rank
    }
}
pub fn enabled(name: &str) -> bool {
    std::env::var(name).as_deref() != Ok("0")
}
pub fn parallel<T: Send, F: Fn(usize) -> T + Sync>(count: usize, workers: usize, f: F) -> Vec<T> {
    let next = AtomicUsize::new(0);
    let mut results = std::thread::scope(|scope| {
        let f = &f;
        let next = &next;
        let mut handles = Vec::new();
        for _ in 0..workers.clamp(1, 4).min(count) {
            handles.push(scope.spawn(move || {
                let mut result = Vec::new();
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= count {
                        break;
                    }
                    result.push((i, f(i)));
                }
                result
            }));
        }
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    results.sort_by_key(|(i, _)| *i);
    results.into_iter().map(|(_, r)| r).collect()
}
pub struct SearchResult {
    pub result: Result,
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
        search.impossible = event.terminals.is_empty();
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
pub fn policy(
    model: &Model,
    cell: Cell,
    seed: i64,
    stream: &str,
    samples: usize,
    tilt: f64,
    point_tilt: f64,
    force_points: bool,
    propagate: bool,
) -> Policy {
    Policy {
        samples,
        seed: derive(
            seed,
            &format!("{stream}-{}-{}", model.ids[cell.team], cell.rank),
        ),
        tilt,
        point_tilt,
        force_points,
        propagate,
    }
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
        if search.impossible || search.result.samples == 0 || search.result.hits > 0 {
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
                results[i].impossible = event.terminals.is_empty();
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
        if search.impossible || r.samples == 0 || r.hits > 0 || r.mass <= 0. {
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
                    return Some(ExtraPlan {
                        index: i,
                        event: Some(Arc::new(e)),
                        gain: f64::INFINITY,
                        native: None,
                        setup_ms: 0.,
                    });
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
        if plan.gain.is_infinite() {
            results[plan.index].impossible = true;
        } else if plan.native.is_some() {
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
pub fn apply(est: &mut Estimate, result: &Result, design: &str) {
    est.probability = result.probability;
    est.std_err = result.std_err;
    est.relative_se = Some(result.std_err / result.probability);
    est.ess = result.ess;
    est.max_event_weight_share = result.max_share;
    est.conditional_mass = result.mass;
    est.conditional_samples = result.samples;
    est.conditional_hits = result.hits;
    est.design = design.into();
    est.meets_precision_goal = result.ess >= 10. && result.std_err / result.probability <= 0.5;
    est.reachability = "witness".into();
    est.zero_hit_upper_95 = 0.;
}
pub fn crosscheck(result: Result, check: Result) -> (Result, bool, bool) {
    if check.probability <= 0.
        || check.hits < 30
        || check.ess < 5.
        || check.std_err / check.probability > 0.6
    {
        return (result, true, false);
    }
    let ratio = result.probability / check.probability;
    if (1. / 30. ..=30.).contains(&ratio) {
        return (result, true, false);
    }
    if check.coarse() {
        (check, true, true)
    } else {
        (result, false, false)
    }
}
pub fn run(model: &Model, seed: i64, workers: usize, estimates: &mut [Estimate]) -> u64 {
    run_logged(model, seed, workers, estimates, None)
}
pub fn run_logged(
    model: &Model,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    log: Option<&crate::logging::RequestLog>,
) -> u64 {
    if !enabled("RARE_POSITION_CONDITIONED_ZERO") {
        if let Some(log) = log {
            log.event(
                "rust_odds_search_skipped",
                serde_json::json!({"reason":"disabled"}),
            );
        }
        return 0;
    }
    let search_start = std::time::Instant::now();
    let baseline = estimates.to_vec();
    let donors = borrowed_donors(&baseline);
    if !donors.is_empty() && estimates.iter().all(|e| e.probability > 0.) {
        let selection = select_donors(model, &donors);
        if selection.admitted.is_empty() {
            report_donor_selection(log, &selection, 0, false);
            return 0;
        }
    }
    let Some(pmfs) = crate::pool::point_pmfs(model) else {
        return 0;
    };
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
    let mut cells = Vec::new();
    for t in 0..model.n {
        for r in 0..model.n {
            let est = &mut estimates[t * model.n + r];
            if est.probability != 0. {
                continue;
            }
            if est.reachability.starts_with("impossible") {
                continue;
            }
            if bounds.hard_bound(t, r, &pmfs[t]).0 <= 0. {
                est.reachability = "impossible_by_points".into();
            } else {
                est.reachability = "undecided".into();
                cells.push(Cell { team: t, rank: r });
            }
        }
    }
    cells.sort_by_key(|c| (c.rank.min(model.n - 1 - c.rank), model.ids[c.team], c.rank));
    report(
        "screen",
        serde_json::json!({"candidate_cells":cells.len(),"cells":crate::logging::cell_counts(estimates)}),
    );
    let proof_report = crate::proof::early_report(model, &cells, estimates);
    let proofs = proof_report.proofs;
    cells.retain(|c| estimates[c.index(model.n)].reachability == "undecided");
    report(
        "early_proofs",
        serde_json::json!({"proofs":proofs,"candidate_cells":cells.len()}),
    );
    if cells.is_empty() {
        if donors.is_empty() {
            return 0;
        }
        let (found, spent) = run_tail_with_donors(
            model,
            seed,
            workers,
            estimates,
            &donors,
            log,
            &[],
            search_start.elapsed().as_secs_f64() * 1000.,
        );
        if found > 0 {
            reconcile(model, estimates, &baseline);
        }
        return spent;
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
    let mut work = 0;
    let mut witnesses = 0;
    for (cell, search) in cells.iter().zip(&results) {
        let est = &mut estimates[cell.index(model.n)];
        let result = &search.result;
        if search.impossible {
            work += result.work;
            est.reachability = "impossible_by_joint_points".into();
            est.zero_hit_upper_95 = 0.;
            continue;
        }
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
                let frequency = result.hits as f64 / result.samples as f64;
                est.probability = result.mass * frequency;
                est.std_err =
                    result.mass * (frequency * (1. - frequency) / result.samples as f64).sqrt();
                est.design = "matched_point_pool_conditioned".into();
                est.relative_se = Some(est.std_err / est.probability);
                est.reachability = "witness".into();
                est.zero_hit_upper_95 = 0.;
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
    let (found, spent) = run_tail_with_donors(
        model,
        seed,
        workers,
        estimates,
        &donors,
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

fn reset_donor_for_tail(est: &mut Estimate) {
    est.probability = 0.;
    est.std_err = 0.;
    est.ess = 0.;
    est.meets_precision_goal = false;
    est.relative_se = None;
    est.max_event_weight_share = 0.;
    est.zero_hit_upper_95 = 0.;
    est.design.clear();
    est.reachability = "undecided".into();
    est.conditional_mass = 0.;
    est.conditional_samples = 0;
    est.conditional_hits = 0;
}

struct DonorSelection {
    admitted: Vec<(usize, Estimate)>,
    deferred: Vec<(usize, Estimate)>,
    candidate_count: usize,
    forecast_per_donor: usize,
    forecasted_total: usize,
    forecast_cap: usize,
}

const BORROWED_RETRY_BUDGET_REFERENCE_DRAWS: usize = 20_000;

fn donor_draw_floors_for_profile(profile: &str) -> (usize, usize, usize) {
    if profile == "union" {
        (1500, 8000, 4000)
    } else {
        // Default lazy/portfolio work uses a 1,000-draw pilot and a fresh
        // 2,000/1,000 main/check pair. Keep this tied to the current minima.
        (1000, 2000, 1000)
    }
}

fn select_donors(model: &Model, donors: &[(usize, Estimate)]) -> DonorSelection {
    select_donors_for_profile(model, donors, &crate::rare_tail::profile())
}

fn select_donors_for_profile(
    model: &Model,
    donors: &[(usize, Estimate)],
    profile: &str,
) -> DonorSelection {
    let reference = crate::rare_tail::budget::reference_cost(model).max(1);
    let family = crate::rare_tail::budget::family_validation_screen_cost(model).max(reference);
    let (pilot, main, check) = donor_draw_floors_for_profile(profile);
    let draws = pilot.saturating_add(main).saturating_add(check);
    let forecast_per_donor = family.saturating_mul(draws);
    // This is a deterministic feasibility screen, not an execution reservation:
    // it excludes bounded batch joint-proposal preparation and caps borrowed
    // retries at 20% of 100,000 reference draws. Actual preparation is charged
    // in pilots and work counters; family validation retains its full bound.
    let forecast_cap = reference.saturating_mul(BORROWED_RETRY_BUDGET_REFERENCE_DRAWS);
    let mut prioritized = donors.to_vec();
    prioritized.sort_by(|a, b| {
        a.1.probability
            .total_cmp(&b.1.probability)
            .then(a.0.cmp(&b.0))
    });
    let mut admitted = Vec::new();
    let mut deferred = Vec::new();
    let mut forecasted_total = 0usize;
    for donor in prioritized {
        if forecasted_total.saturating_add(forecast_per_donor) <= forecast_cap {
            forecasted_total += forecast_per_donor;
            admitted.push(donor);
        } else {
            deferred.push(donor);
        }
    }
    DonorSelection {
        candidate_count: donors.len(),
        admitted,
        deferred,
        forecast_per_donor,
        forecasted_total,
        forecast_cap,
    }
}

fn borrowed_donors(estimates: &[Estimate]) -> Vec<(usize, Estimate)> {
    estimates
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e.design == "matched_point_pool"
                && e.probability > 0.
                && e.hits == 0
                && e.conditional_mass == 0.
                && e.conditional_samples == 0
                && e.conditional_hits == 0
        })
        .map(|(i, e)| (i, e.clone()))
        .collect()
}

fn report_donor_selection(
    log: Option<&crate::logging::RequestLog>,
    selection: &DonorSelection,
    true_zero_candidates: usize,
    reserve_true_zeros: bool,
) {
    if selection.candidate_count == 0 {
        return;
    }
    let reserved = reserve_true_zeros && true_zero_candidates > 0;
    let admitted = if reserved {
        0
    } else {
        selection.admitted.len()
    };
    let deferred = selection.candidate_count - admitted;
    if let Some(log) = log {
        log.event(
            "rust_odds_search_donors",
            serde_json::json!({
                "donor_candidates": selection.candidate_count,
                "candidate_count": selection.candidate_count,
                "admitted": admitted,
                "deferred": deferred,
                "true_zero_candidates": true_zero_candidates,
                "reason": if reserved {"zero_recovery_budget_reserved"} else if deferred > 0 {"forecast_cap"} else {"within_forecast_cap"},
                "forecast_per_donor": selection.forecast_per_donor,
                "forecast_cap": selection.forecast_cap,
                "forecasted_total": if reserved {0} else {selection.forecasted_total}
            }),
        );
    }
}

fn run_tail_with_donors(
    model: &Model,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    donors: &[(usize, Estimate)],
    log: Option<&crate::logging::RequestLog>,
    seasons: &[(Vec<u8>, Vec<usize>)],
    elapsed_ms: f64,
) -> (usize, u64) {
    let selection = select_donors(model, donors);
    let true_zero_candidates = estimates
        .iter()
        .filter(|e| e.probability == 0. && !e.reachability.starts_with("impossible"))
        .count();
    if !donors.is_empty() && true_zero_candidates > 0 {
        for (index, donor) in donors {
            estimates[*index] = donor.clone();
        }
        report_donor_selection(log, &selection, true_zero_candidates, true);
        return crate::rare_tail::run(model, seed, workers, estimates, log, seasons, elapsed_ms);
    }
    for (index, donor) in &selection.deferred {
        estimates[*index] = donor.clone();
    }
    report_donor_selection(log, &selection, 0, false);
    for (index, donor) in &selection.admitted {
        estimates[*index] = donor.clone();
        reset_donor_for_tail(&mut estimates[*index]);
    }
    let (found, spent) =
        crate::rare_tail::run(model, seed, workers, estimates, log, seasons, elapsed_ms);
    for (index, donor) in &selection.admitted {
        if estimates[*index].probability <= 0. {
            estimates[*index] = donor.clone();
        }
    }
    (found, spent)
}

fn reconcile(model: &Model, estimates: &mut [Estimate], baseline: &[Estimate]) -> bool {
    let mut matrix: Vec<_> = estimates.iter().map(|e| e.probability).collect();
    if !crate::pool::balance(&mut matrix, model.n) {
        estimates.clone_from_slice(baseline);
        return false;
    }
    for (e, p) in estimates.iter_mut().zip(matrix) {
        if (e.reachability == "witness" || e.design == "outcome_path_stratified")
            && e.probability > 0.
        {
            e.std_err *= p / e.probability;
            e.relative_se = Some(e.std_err / p);
        }
        e.probability = p;
    }
    true
}

#[cfg(test)]
mod donor_preflight_tests {
    use super::select_donors_for_profile;
    use crate::{
        model::{Model, Request},
        pool::Estimate,
    };

    fn model(source: &str) -> Model {
        Model::new(serde_json::from_str::<Request>(source).unwrap()).unwrap()
    }

    fn donors(count: usize) -> Vec<(usize, Estimate)> {
        (0..count)
            .map(|index| {
                (
                    index,
                    Estimate {
                        probability: 0.001,
                        ..Estimate::default()
                    },
                )
            })
            .collect()
    }

    fn assert_large_group_is_deferred(source: &str) {
        let model = model(source);
        let selection = select_donors_for_profile(&model, &donors(50), "coverage");
        assert_eq!(selection.candidate_count, 50);
        assert!(selection.admitted.is_empty());
        assert_eq!(selection.deferred.len(), 50);
        assert!(selection.forecast_per_donor > selection.forecast_cap);
    }

    #[test]
    fn expensive_reference_groups_defer_borrowed_retries() {
        assert_large_group_is_deferred(include_str!(
            "../../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16413-a2eef0a8.json"
        ));
        assert_large_group_is_deferred(include_str!(
            "../../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16983-43969b02.json"
        ));
    }

    #[test]
    fn expensive_donor_only_request_short_circuits_without_mutating_estimates() {
        let model = model(include_str!(
            "../../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16413-a2eef0a8.json"
        ));
        let n = model.n;
        let mut estimates = vec![Estimate::default(); n * n];
        for estimate in &mut estimates {
            estimate.probability = 1. / n as f64;
            estimate.std_err = 0.003;
            estimate.samples = 100_000;
            estimate.hits = 0;
            estimate.ess = 0.;
            estimate.mean_weight = 1.;
            estimate.available = true;
            estimate.meets_precision_goal = false;
            estimate.relative_se = Some(0.012);
            estimate.max_event_weight_share = 0.01;
            estimate.design = "matched_point_pool".into();
            estimate.reachability = "pool_positive".into();
        }
        let before = serde_json::to_value(&estimates).unwrap();
        assert_eq!(super::run(&model, 809, 1, &mut estimates), 0);
        assert_eq!(serde_json::to_value(&estimates).unwrap(), before);
    }

    #[test]
    fn b3_rare_low_probability_candidate_is_admitted_first() {
        let model = model(include_str!("../tests/fixtures/group-17064.json"));
        let target = model.ids[..model.n]
            .iter()
            .position(|&team| team == 2689)
            .unwrap()
            * model.n;
        let candidates = vec![
            (
                target,
                Estimate {
                    probability: 5.2576e-5,
                    ..Estimate::default()
                },
            ),
            (
                target + 1,
                Estimate {
                    probability: 0.01,
                    ..Estimate::default()
                },
            ),
        ];
        let selection = select_donors_for_profile(&model, &candidates, "coverage");
        let reference = crate::rare_tail::budget::reference_cost(&model).max(1);
        let validation_draws = 1_000 + 2_000 + 1_000;
        let full_forecast = crate::rare_tail::budget::family_validation_upper_cost(&model)
            .max(reference)
            .saturating_mul(validation_draws);
        let screened_forecast = crate::rare_tail::budget::family_validation_screen_cost(&model)
            .max(reference)
            .saturating_mul(validation_draws);
        assert!(screened_forecast <= selection.forecast_cap);
        assert!(full_forecast > selection.forecast_cap);
        assert_eq!(selection.forecast_per_donor, screened_forecast);
        assert_eq!(selection.admitted.len(), 1);
        assert_eq!(selection.admitted[0].0, target);
        assert_eq!(selection.deferred.len(), 1);
        assert!(selection.forecasted_total <= selection.forecast_cap);
    }

    #[test]
    fn many_teams_with_one_fixture_fit_the_forecast_cap() {
        let request = serde_json::json!({
            "id": 2,
            "phase": {"sort":"pt,head,gd,bias", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups": (0..20).map(|team| serde_json::json!({"team_id":team,"add_sub":0,"bias":0})).collect::<Vec<_>>(),
            "games": [{"id":1,"home_id":0,"away_id":1,"home_power":1.0,"away_power":1.0}]
        });
        let model = Model::new(serde_json::from_value::<Request>(request).unwrap()).unwrap();
        let selection = select_donors_for_profile(&model, &donors(4), "coverage");
        assert_eq!(selection.admitted.len(), 1);
        assert_eq!(selection.deferred.len(), 3);
        assert!(selection.forecast_per_donor <= selection.forecast_cap);
        assert!(selection.forecasted_total <= selection.forecast_cap);
    }
}
