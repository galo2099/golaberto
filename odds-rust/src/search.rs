use crate::{
    conditioned::{blockers, fast, Event, Result},
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
        } else {
            search.result.work += result.work;
        }
    }
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
) {
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
            return None;
        }
        let mut mass = r.mass;
        let mut refined = None;
        let needed = (cell.rank + 1).min(model.n - cell.rank);
        if model.n > 3 && needed <= 3 && r.blockers < 3 && mass <= 1e-3 {
            let selected = blockers(model, bounds, pmfs, cell.team, cell.rank, 3);
            if let Some(e) = Event::build(model, bounds, cell.team, cell.rank, &selected, 120000) {
                if e.mass <= 0. {
                    return Some((i, Arc::new(e), f64::INFINITY));
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
        Some((i, event, gain))
    });
    let mut candidates = Vec::new();
    for (i, p) in plans.into_iter().enumerate() {
        if let Some((_, event, gain)) = p {
            if gain.is_infinite() {
                results[i].impossible = true;
            } else {
                candidates.push((i, event, gain));
            }
        }
    }
    candidates.sort_by(|(a, _, x), (b, _, y)| {
        y.total_cmp(x)
            .then(model.ids[cells[*a].team].cmp(&model.ids[cells[*b].team]))
            .then(cells[*a].rank.cmp(&cells[*b].rank))
    });
    candidates.truncate(4);
    let extras = parallel(candidates.len(), workers, |i| {
        let (index, event, _) = &candidates[i];
        let cell = cells[*index];
        let seed = derive(
            seed,
            &format!(
                "conditioned-zero-extra-{}-{}",
                model.ids[cell.team], cell.rank
            ),
        );
        let mut result = fast(model, event, cell.team, cell.rank, 50000, seed);
        result.blockers = event.teams.len() - 1;
        (*index, result)
    });
    for (i, mut extra) in extras {
        extra.work += results[i].result.work;
        if results[i].witness.is_none() {
            results[i].witness = extra.witness.clone();
        }
        results[i].result = extra;
    }
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
    if !enabled("RARE_POSITION_CONDITIONED_ZERO")
        || estimates.iter().all(|e| e.design == "plain_mc")
    {
        if let Some(log) = log {
            log.event("rust_odds_search_skipped", serde_json::json!({"reason":
                if !enabled("RARE_POSITION_CONDITIONED_ZERO") {"disabled"} else {"plain_mc_fallback"}}));
        }
        return 0;
    }
    let Some(pmfs) = crate::pool::point_pmfs(model) else {
        return 0;
    };
    let bounds = Bounds::new(model);
    let baseline = estimates.to_vec();
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
        return 0;
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
    extra(
        model,
        &bounds,
        &pmfs,
        &cells,
        estimates,
        seed,
        workers,
        &mut results,
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
    if witnesses > 0 {
        let mut matrix: Vec<_> = estimates.iter().map(|e| e.probability).collect();
        if !crate::pool::balance(&mut matrix, model.n) {
            estimates.clone_from_slice(&baseline);
            report(
                "reconcile",
                serde_json::json!({"rolled_back":true,"cells":crate::logging::cell_counts(estimates)}),
            );
            return work;
        }
        for (e, p) in estimates.iter_mut().zip(matrix) {
            if e.reachability == "witness" && e.probability > 0. {
                e.std_err *= p / e.probability;
                e.relative_se = Some(e.std_err / p);
            }
            e.probability = p;
        }
    }
    report(
        "reconcile",
        serde_json::json!({"rolled_back":false,"cells":crate::logging::cell_counts(estimates)}),
    );
    work
}
