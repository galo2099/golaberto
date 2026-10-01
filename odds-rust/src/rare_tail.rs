//! Fixed-batch coverage portfolio on cells that remain zero after the
//! existing pipeline. Pilot selection never contributes to reported estimates.
use crate::{
    conditioned::Result,
    joint_caps::{
        confirmed,
        propagated::{
            lazy::{LazyJoint, SharedGuides},
            Limits, PropagatedJoint,
        },
    },
    model::Model,
    pool::Estimate,
    rng::derive,
    search::{apply, parallel, Cell},
};
use serde_json::json;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

// Coverage is the default profile, with explicit environment overrides. Reading it does
// not mutate the process environment or interfere with concurrent requests.
pub fn profile() -> String {
    resolve_profile(std::env::var("RUST_ODDS_RARE_TAIL").ok())
}
fn resolve_profile(explicit: Option<String>) -> String {
    explicit.unwrap_or_else(|| "coverage".into())
}
pub fn value(name: &str) -> String {
    resolve_value(name, std::env::var(name).ok(), profile() == "coverage")
}
fn resolve_value(name: &str, explicit: Option<String>, coverage: bool) -> String {
    explicit.unwrap_or_else(|| {
        if coverage {
            profile_default(name).into()
        } else {
            String::new()
        }
    })
}
fn profile_default(name: &str) -> &'static str {
    match name {
        "RUST_ODDS_LAZY_GUIDE" => "cardinality",
        "RUST_ODDS_LAZY_GOALS" | "RUST_ODDS_LAZY_OMIT" | "RUST_ODDS_RARE_TAIL_SHARE" => "1",
        "RUST_ODDS_LAZY_SUBSET" => "adaptive",
        "RUST_ODDS_RARE_TAIL_FINALISTS" => "16",
        "RUST_ODDS_LAZY_RIVALS" => "4",
        "RUST_ODDS_RARE_TAIL_QUALITY" => "order",
        // Faster native stages otherwise reduce funding for the same rare
        // plans. Reallocate part of their CPU savings to preserve coverage.
        "RUST_ODDS_RARE_TAIL_BUDGET_FRACTION" => "0.45",
        "RUST_ODDS_RARE_TAIL_CONFIRM_MORE" => "1.5",
        "RUST_ODDS_RARE_TAIL_EXTENSION" => "after",
        "RUST_ODDS_RARE_TAIL_UNION_PATTERNS" => "256",
        "RUST_ODDS_RARE_TAIL_RETRY" | "RUST_ODDS_RARE_TAIL_BATCH_ALLOCATION" => "0",
        "RUST_ODDS_SHARED_CONSTRAINTS" => "guided",
        "RUST_ODDS_SHARED_CONSTRAINTS_BLOCKERS" => "2",
        "RUST_ODDS_SHARED_CONSTRAINTS_RELATIVE" => "1",
        "RUST_ODDS_SHARED_CONSTRAINTS_CONFIRMATION" => "parallel",
        "RUST_ODDS_SHARED_CONSTRAINTS_FRACTION" => "0.5",
        _ => "",
    }
}

enum Plan {
    Union(PropagatedJoint),
    Lazy(LazyJoint),
}
impl Plan {
    fn sample(&self, m: &Model, n: usize, seed: i64) -> Result {
        match self {
            Self::Union(p) => p.sample(m, n, seed, true),
            Self::Lazy(p) => p.sample(m, n, seed),
        }
    }
    fn describe(&self, m: &Model) -> serde_json::Value {
        match self {
            Self::Union(p) => p.describe(m),
            Self::Lazy(p) => p.describe(m),
        }
    }
}
pub fn run(
    m: &Model,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    log: Option<&crate::logging::RequestLog>,
    witnesses: &[(Vec<u8>, Vec<usize>)],
    unlogged_search_ms: f64,
) -> (usize, u64) {
    let mode = profile();
    let mode = if mode == "coverage" {
        "portfolio".to_string()
    } else {
        mode
    };
    if !["union", "lazy", "portfolio"].contains(&mode.as_str()) {
        return (0, 0);
    }
    let extension_mode = value("RUST_ODDS_RARE_TAIL_EXTENSION");
    let extension = mode == "portfolio" && ["after", "overlap"].contains(&extension_mode.as_str());
    let mut cells: Vec<_> = estimates
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            (e.probability == 0. && !e.reachability.starts_with("impossible")).then_some(Cell {
                team: i / m.n,
                rank: i % m.n,
            })
        })
        .collect();
    let rough = value("RUST_ODDS_RARE_TAIL_QUALITY") == "order";
    let start = Instant::now();
    let fraction = value("RUST_ODDS_RARE_TAIL_BUDGET_FRACTION")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0. && *v <= 2.)
        .unwrap_or(0.45);
    let native_ms = log.map_or(unlogged_search_ms, |l| l.calculation_elapsed_ms());
    let allowance_ms = native_ms * fraction;
    let more_fraction = confirmation_fraction(&value("RUST_ODDS_RARE_TAIL_CONFIRM_MORE"));
    let shared_fraction = value("RUST_ODDS_SHARED_CONSTRAINTS_FRACTION")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && (0. ..=0.8).contains(v))
        .unwrap_or(0.25);
    let (shared_found, shared_work) = crate::shared_constraints::run(
        m,
        &cells,
        seed,
        workers,
        estimates,
        log,
        allowance_ms * shared_fraction,
        rough,
    );
    cells.retain(|c| estimates[c.index(m.n)].probability == 0.);
    let shared = (value("RUST_ODDS_RARE_TAIL_SHARE") == "1")
        .then(|| std::sync::Arc::new(SharedGuides::default()));
    let plans = parallel(cells.len(), workers, |i| {
        let cell = cells[i];
        if start.elapsed().as_secs_f64() * 1000. >= allowance_ms * 0.65 {
            return None;
        }
        let start = Instant::now();
        let stream = derive(
            seed,
            &format!("rare-tail-pilot-{}-{}", m.ids[cell.team], cell.rank),
        );
        let union = if mode != "lazy" {
            PropagatedJoint::configured(
                m,
                cell,
                Limits {
                    minimum_forced: 0,
                    rivals: value("RUST_ODDS_RARE_TAIL_UNION_RIVALS")
                        .parse::<usize>()
                        .ok()
                        .filter(|&v| v >= 1 && v <= 6)
                        .unwrap_or(6),
                    target_patterns: if extension {
                        256
                    } else {
                        value("RUST_ODDS_RARE_TAIL_UNION_PATTERNS")
                            .parse::<usize>()
                            .ok()
                            .filter(|&v| v >= 1 && v <= 256)
                            .unwrap_or(256)
                    },
                    feasible_cases: 128,
                },
            )
            .ok()
        } else {
            None
        };
        let mut candidate = union.map(|p| {
            let clock = Instant::now();
            let r = p.sample(m, if mode == "union" { 1500 } else { 1000 }, stream, true);
            (Plan::Union(p), r, clock.elapsed().as_secs_f64() * 1000.)
        });
        let mut pilot_work = candidate.as_ref().map_or(0, |p| p.1.work);
        if mode != "union" && candidate.as_ref().is_none_or(|p| p.1.ess < 25.) {
            let seeds: Vec<_> = witnesses
                .iter()
                .filter(|(_, r)| r[cell.team] == cell.rank)
                .map(|(o, _)| o.clone())
                .collect();
            if let Some(mut p) = LazyJoint::new_shared(
                m,
                cell,
                derive(
                    seed,
                    &format!("rare-tail-setup-{}-{}", m.ids[cell.team], cell.rank),
                ),
                &seeds,
                32,
                shared.clone(),
            ) {
                let clock = Instant::now();
                let fit_roots = !extension && value("RUST_ODDS_RARE_TAIL_RETRY") == "roots";
                let (mut r, moments) = if fit_roots {
                    let (r, moments) = p.sample_training(m, 1000, derive(stream, "lazy"));
                    (r, Some(moments))
                } else {
                    (p.sample(m, 1000, derive(stream, "lazy")), None)
                };
                let mut pilot_ms = clock.elapsed().as_secs_f64() * 1000.;
                pilot_work += r.work;
                if value("RUST_ODDS_LAZY_SUBSET") == "adaptive" && r.ess < 12. {
                    // Sparse pilot hits contain too little branch information.
                    // Keep the existing witness/alternate-mode retry for them.
                    let fitted =
                        r.hits >= 30 && moments.as_ref().is_some_and(|s| p.fit_root_allocation(s));
                    if !fitted {
                        p.add_alternates(m, &seeds);
                    }
                    let clock = Instant::now();
                    let alt = p.sample(m, 1000, derive(stream, "multimode"));
                    let alt_ms = clock.elapsed().as_secs_f64() * 1000.;
                    pilot_work += alt.work;
                    if alt.ess > r.ess {
                        r = alt;
                        pilot_ms = alt_ms;
                    } else {
                        if fitted {
                            p.clear_root_allocation();
                        } else {
                            p.clear_alternates();
                        }
                    }
                }
                if std::env::var("RUST_ODDS_LAZY_FIXTURE_BIAS").as_deref() == Ok("1") && r.ess < 12.
                {
                    p.add_fixture_bias(m, &seeds);
                    let clock = Instant::now();
                    let biased = p.sample(m, 1000, derive(stream, "fixture-bias"));
                    let biased_ms = clock.elapsed().as_secs_f64() * 1000.;
                    pilot_work += biased.work;
                    if biased.ess > r.ess {
                        r = biased;
                        pilot_ms = biased_ms;
                    } else {
                        p.clear_bias();
                    }
                }
                if (std::env::var("RUST_ODDS_LAZY_CARDINALITY_CASES").as_deref() == Ok("1")
                    && r.ess < 12.)
                    || (std::env::var("RUST_ODDS_LAZY_CARDINALITY_CASES").as_deref() == Ok("hard")
                        && r.ess < 2.)
                {
                    p.add_cases(m, &seeds);
                    let clock = Instant::now();
                    let case = p.sample(m, 1000, derive(stream, "cardinality-cases"));
                    let case_ms = clock.elapsed().as_secs_f64() * 1000.;
                    pilot_work += case.work;
                    if case.ess > r.ess {
                        r = case;
                        pilot_ms = case_ms;
                    } else {
                        p.clear_cases();
                    }
                }
                if candidate.as_ref().is_none_or(|v| r.ess > v.1.ess) {
                    candidate = Some((Plan::Lazy(p), r, pilot_ms));
                }
            }
        }
        let setup_ms = start.elapsed().as_secs_f64() * 1000.;
        emit(
            log,
            "rust_odds_rare_tail_training",
            json!({"group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,"training_ms":setup_ms,"pilot_work":pilot_work,"pilot":candidate.as_ref().map(|(p,r,ms)|json!({"hits":r.hits,"ess":r.ess,"sampling_ms":ms,"plan":p.describe(m)}))}),
        );
        if let Some((plan, r, pilot_ms)) = candidate {
            Some((cell, plan, r, setup_ms, pilot_work, pilot_ms))
        } else {
            None
        }
    });
    let mut plans: Vec<_> = plans.into_iter().flatten().collect();
    if std::env::var("RUST_ODDS_RARE_TAIL_REUSE").as_deref() == Ok("1") {
        let seasons: Vec<_> = plans
            .iter()
            .filter_map(|p| p.2.witness.as_ref())
            .map(|o| (o.clone(), crate::conditioned::canonical_ranks(m, o)))
            .collect();
        let jobs: Vec<_> = plans
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let seeds: Vec<_> = seasons
                    .iter()
                    .filter(|(_, r)| r[p.0.team] == p.0.rank)
                    .map(|(o, _)| o.clone())
                    .collect();
                (p.2.ess < 5. && !seeds.is_empty()).then_some((i, seeds))
            })
            .take(4)
            .collect();
        let rescued = parallel(jobs.len(), workers, |j| {
            if start.elapsed().as_secs_f64() * 1000. >= allowance_ms * 0.65 {
                return None;
            }
            let (i, seeds) = &jobs[j];
            let cell = plans[*i].0;
            let clock = Instant::now();
            let p = LazyJoint::from_seasons(
                m,
                cell,
                derive(
                    seed,
                    &format!("rare-tail-reuse-{}-{}", m.ids[cell.team], cell.rank),
                ),
                seeds,
                8,
            )?;
            let sample = Instant::now();
            let r = p.sample(
                m,
                1000,
                derive(
                    seed,
                    &format!("rare-tail-reuse-pilot-{}-{}", m.ids[cell.team], cell.rank),
                ),
            );
            let ms = sample.elapsed().as_secs_f64() * 1000.;
            let elapsed = clock.elapsed().as_secs_f64() * 1000.;
            emit(
                log,
                "rust_odds_rare_tail_reuse",
                json!({"team":m.ids[cell.team],"rank":cell.rank+1,"training_ms":elapsed,"pilot_ess":r.ess,"pilot_hits":r.hits,"seeds":seeds.len()}),
            );
            Some((*i, p, r, ms, elapsed))
        });
        for (i, p, r, ms, elapsed) in rescued.into_iter().flatten() {
            let old = &mut plans[i];
            old.4 += r.work;
            old.3 += elapsed;
            if r.ess > old.2.ess {
                old.1 = Plan::Lazy(p);
                old.2 = r;
                old.5 = ms;
            }
        }
    }
    let mut work = shared_work + plans.iter().map(|p| p.4).sum::<u64>();
    for (cell, _, r, _, _, _) in &plans {
        if r.witness.is_some() && estimates[cell.index(m.n)].reachability == "undecided" {
            estimates[cell.index(m.n)].reachability = "witness".into();
        }
    }
    plans.sort_by(|a, b| {
        b.2.ess
            .total_cmp(&a.2.ess)
            .then(a.0.index(m.n).cmp(&b.0.index(m.n)))
    });
    let finalists = Some(value("RUST_ODDS_RARE_TAIL_FINALISTS"))
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(8)
        .min(32);
    let mut weak = Vec::new();
    if more_fraction > 0. {
        let (strong, rest): (Vec<_>, Vec<_>) = plans
            .into_iter()
            .partition(|p| p.2.hits >= 3 && p.2.ess >= if rough { 1.5 } else { 2. });
        plans = strong;
        weak = rest;
        if plans.len() > finalists {
            weak.extend(plans.split_off(finalists));
        }
    } else {
        plans.retain(|p| p.2.hits >= 3 && p.2.ess >= if rough { 1.5 } else { 2. });
        plans.truncate(finalists);
    }
    let remaining_ms = (allowance_ms - start.elapsed().as_secs_f64() * 1000.).max(0.);
    let mut bins = vec![0_f64; workers.clamp(1, 4)];
    let mut funded = Vec::new();
    let mut pending = Vec::new();
    let mut funded_bins = Vec::new();
    let reserve = !extension && value("RUST_ODDS_RARE_TAIL_BATCH_ALLOCATION") == "reserve";
    let pilot_ess_target = if extension {
        10.
    } else {
        match value("RUST_ODDS_RARE_TAIL_BATCH_ALLOCATION").as_str() {
            "robust" => 20.,
            "robust32" => 32.,
            _ => 10.,
        }
    };
    for p in plans {
        let ess_target = if !extension
            && value("RUST_ODDS_RARE_TAIL_BATCH_ALLOCATION") == "concentrated"
            && p.2.ess < 12.
        {
            32.
        } else {
            pilot_ess_target
        };
        let n = if mode == "union" {
            8000
        } else {
            (((if rough { 40. } else { 60. }) * p.2.samples as f64 / p.2.hits.max(1) as f64)
                .max((if rough { ess_target } else { 20. }) * p.2.samples as f64 / p.2.ess.max(0.1))
                .ceil() as usize)
                .clamp(
                    if rough { 1000 } else { 2000 },
                    if rough { 6000 } else { 8000 },
                )
        };
        let check = if mode == "union" {
            4000
        } else {
            (n / 2).max(1000)
        };
        let cost = 1.2 * p.5 / p.2.samples.max(1) as f64 * (n + check) as f64;
        let bin = (0..bins.len())
            .min_by(|&a, &b| bins[a].total_cmp(&bins[b]))
            .unwrap();
        if mode == "union" || bins[bin] + cost <= remaining_ms {
            bins[bin] += cost;
            funded.push((p, n, check));
            funded_bins.push(bin);
        } else if extension || more_fraction > 0. {
            pending.push(p);
        }
    }
    // Fund the ordinary batches first. Weak pilots may consume spare capacity,
    // but cannot displace a batch already funded for another cell.
    if reserve && mode != "union" {
        let mut loads = bins.clone();
        for ((p, n, check), bin) in funded.iter_mut().zip(funded_bins) {
            if !rough || p.2.ess >= 12. {
                continue;
            }
            if let Some((desired, desired_check, extra)) = reserve_extension(
                *n,
                *check,
                p.2.samples,
                p.2.ess,
                1.2 * p.5 / p.2.samples.max(1) as f64,
                remaining_ms - loads[bin],
            ) {
                loads[bin] += extra;
                *n = desired;
                *check = desired_check;
            }
        }
    }
    let plans = funded;
    // Original jobs and sampling streams are fixed before any extension work.
    // Status 0 is in flight, 1 accepted, 2 rejected; only rejected jobs may retry.
    let status: Vec<_> = (0..plans.len()).map(|_| AtomicU8::new(0)).collect();
    let final_start = Instant::now();
    let extension_deadline_ms = if extension_mode == "overlap" {
        bins.iter().copied().fold(0., f64::max).min(remaining_ms)
    } else {
        remaining_ms
    };
    let mut extension_jobs: Vec<_> = if extension {
        plans
            .iter()
            .enumerate()
            .map(|(i, (p, _, _))| (Some(i), p))
            .chain(pending.iter().map(|p| (None, p)))
            .map(|(source, p)| {
                // Overlap uses smaller fixed batches to fit idle-worker gaps;
                // the sequential arm targets more effective samples. Gates are identical.
                let ess_target = if extension_mode == "overlap" {
                    if rough {
                        10.
                    } else {
                        20.
                    }
                } else if rough {
                    32.
                } else {
                    40.
                };
                let n = (ess_target * p.2.samples as f64 / p.2.ess.max(0.1)).ceil() as usize;
                let n = n.clamp(
                    if rough { 1000 } else { 2000 },
                    if rough { 6000 } else { 8000 },
                );
                let check = (n / 2).max(1000);
                let cost = 1.2 * p.5 / p.2.samples.max(1) as f64 * (n + check) as f64;
                (source, p, n, check, cost)
            })
            .collect()
    } else {
        Vec::new()
    };
    extension_jobs.sort_by(|a, b| {
        a.4.total_cmp(&b.4)
            .then(a.1 .0.index(m.n).cmp(&b.1 .0.index(m.n)))
    });
    let ordinary = |i: usize| {
        let ((cell, plan, pilot, setup_ms, _, _), main_draws, check_draws) = &plans[i];
        let start = Instant::now();
        let main = plan.sample(
            m,
            *main_draws,
            derive(
                seed,
                &format!("rare-tail-main-{}-{}", m.ids[cell.team], cell.rank),
            ),
        );
        let check = if publishable(&main, rough) {
            plan.sample(
                m,
                *check_draws,
                derive(
                    seed,
                    &format!("rare-tail-check-{}-{}", m.ids[cell.team], cell.rank),
                ),
            )
        } else {
            Result::default()
        };
        let accepted = accepted(&main, &check, rough);
        status[i].store(if accepted { 1 } else { 2 }, Ordering::Release);
        emit(
            log,
            "rust_odds_rare_tail",
            json!({"group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,"setup_ms":setup_ms,"pilot_ess":pilot.ess,"pilot_hits":pilot.hits,"main_draws":main.samples,"main_hits":main.hits,"main_ess":main.ess,"main_max_share":main.max_share,"main_batch_gap":main.batch_gap,"check_relative_se":if check.probability>0.{Some(check.std_err/check.probability)}else{None},"quality":if rough{"order"}else{"current"},"probability":main.probability,"relative_se":if main.probability>0.{Some(main.std_err/main.probability)}else{None},"check_probability":check.probability,"check_ess":check.ess,"accepted":accepted,"sampling_ms":start.elapsed().as_secs_f64()*1000.,"plan":plan.describe(m)}),
        );
        (*cell, main, check.work, accepted, false)
    };
    let extra = |i: usize| {
        let (source, p, n, check_n, cost) = &extension_jobs[i];
        if source.is_some_and(|j| status[j].load(Ordering::Acquire) != 2)
            || final_start.elapsed().as_secs_f64() * 1000. + cost > extension_deadline_ms
        {
            return None;
        }
        let cell = p.0;
        let clock = Instant::now();
        // Retry selection can use an ordinary batch's failure, but none of its
        // observations contribute to these fresh main/check estimates.
        let main = p.1.sample(
            m,
            *n,
            derive(
                seed,
                &format!(
                    "rare-tail-extension-main-{}-{}",
                    m.ids[cell.team], cell.rank
                ),
            ),
        );
        let check = if publishable(&main, rough) {
            p.1.sample(
                m,
                *check_n,
                derive(
                    seed,
                    &format!(
                        "rare-tail-extension-check-{}-{}",
                        m.ids[cell.team], cell.rank
                    ),
                ),
            )
        } else {
            Result::default()
        };
        let ok = accepted(&main, &check, rough);
        emit(
            log,
            "rust_odds_rare_tail_extension",
            json!({"group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,"mode":extension_mode,"source":if source.is_some(){"rejected_final"}else{"unfunded_pilot"},"main_draws":main.samples,"main_hits":main.hits,"main_ess":main.ess,"main_max_share":main.max_share,"main_relative_se":if main.probability>0.{Some(main.std_err/main.probability)}else{None},"main_batch_gap":main.batch_gap,"probability":main.probability,"check_probability":check.probability,"check_hits":check.hits,"check_ess":check.ess,"check_relative_se":if check.probability>0.{Some(check.std_err/check.probability)}else{None},"accepted":ok,"forecast_ms":cost,"sampling_ms":clock.elapsed().as_secs_f64()*1000.}),
        );
        Some((cell, main, check.work, ok, true))
    };
    let results = if extension_mode == "overlap" && extension {
        // All ordinary jobs are dispatched ahead of extension jobs, on the same
        // worker queue. No extra worker or serial time allowance is introduced.
        parallel(plans.len() + extension_jobs.len(), workers, |i| {
            if i < plans.len() {
                Some(ordinary(i))
            } else {
                extra(i - plans.len())
            }
        })
    } else {
        let mut r = parallel(plans.len(), workers, |i| Some(ordinary(i)));
        if extension {
            r.extend(parallel(extension_jobs.len(), workers, extra));
        }
        r
    };
    let mut found = shared_found;
    let mut extra_found = 0;
    let mut extra_draws = 0;
    for (cell, r, check_work, accepted, extra) in results.into_iter().flatten() {
        work += r.work + check_work;
        if extra {
            extra_draws += r.samples;
        }
        // Results are in queue order: commit all ordinary estimates first.
        if accepted && commit_estimate(&mut estimates[cell.index(m.n)], &r, extra) {
            found += 1;
            extra_found += usize::from(extra);
        }
    }
    let ordinary_final_phase_ms = final_start.elapsed().as_secs_f64() * 1000.;
    // The additional allowance confirms already-built proposals only.
    // Ordinary results are frozen first; new independent streams can only fill zeros.
    if more_fraction > 0. {
        let before: Vec<_> = estimates
            .iter()
            .enumerate()
            .filter(|(_, e)| e.probability > 0.)
            .map(|(i, e)| (i, serde_json::to_value(e).unwrap()))
            .collect();
        let extra_start = Instant::now();
        let extra_allowance = native_ms * more_fraction;
        let mut jobs: Vec<_> = plans
            .iter()
            .map(|(p, _, _)| p)
            .chain(pending.iter())
            .chain(weak.iter())
            .filter(|p| {
                estimates[p.0.index(m.n)].probability == 0.
                    && !estimates[p.0.index(m.n)]
                        .reachability
                        .starts_with("impossible")
            })
            .filter_map(|p| {
                confirmation_batches(&p.2).map(|n| {
                    let cost = 1.2 * p.5 / p.2.samples as f64 * (2 * n) as f64;
                    (p, n, cost)
                })
            })
            .collect();
        jobs.sort_by(|a, b| {
            a.2.total_cmp(&b.2)
                .then(a.0 .0.index(m.n).cmp(&b.0 .0.index(m.n)))
        });
        let eligible = jobs.len();
        let mut loads = vec![0_f64; workers.clamp(1, 4)];
        jobs.retain(|(_, _, cost)| {
            let bin = (0..loads.len())
                .min_by(|&a, &b| loads[a].total_cmp(&loads[b]))
                .unwrap();
            if cost.is_finite() && *cost >= 0. && loads[bin] + cost <= extra_allowance {
                loads[bin] += cost;
                true
            } else {
                false
            }
        });
        let funded = jobs.len();
        let extra_results = parallel(jobs.len(), workers, |i| {
            let (p, n, cost) = jobs[i];
            if extra_start.elapsed().as_secs_f64() * 1000. + cost > extra_allowance {
                return None;
            }
            let clock = Instant::now();
            let cell = p.0;
            // Fixed batch sizes are selected solely from independent training data.
            let main = p.1.sample(
                m,
                n,
                derive(
                    seed,
                    &format!(
                        "rare-tail-confirm-more-main-{}-{}",
                        m.ids[cell.team], cell.rank
                    ),
                ),
            );
            let check = if publishable(&main, rough) {
                p.1.sample(
                    m,
                    n,
                    derive(
                        seed,
                        &format!(
                            "rare-tail-confirm-more-check-{}-{}",
                            m.ids[cell.team], cell.rank
                        ),
                    ),
                )
            } else {
                Result::default()
            };
            let ok = accepted(&main, &check, rough);
            emit(
                log,
                "rust_odds_rare_tail_confirm_more",
                json!({
                    "group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,
                    "pilot_hits":p.2.hits,"pilot_ess":p.2.ess,"main_draws":main.samples,
                    "main_hits":main.hits,"main_ess":main.ess,"main_max_share":main.max_share,
                    "probability":main.probability,"check_draws":check.samples,"check_hits":check.hits,
                    "check_ess":check.ess,"check_probability":check.probability,"accepted":ok,
                    "forecast_ms":cost,"sampling_ms":clock.elapsed().as_secs_f64()*1000.
                }),
            );
            Some((cell, main, check.work, ok))
        });
        let mut added = 0;
        let mut completed = 0;
        let mut extra_work = 0;
        for (cell, r, check_work, ok) in extra_results.into_iter().flatten() {
            completed += 1;
            extra_work += r.work + check_work;
            if ok && commit_estimate(&mut estimates[cell.index(m.n)], &r, true) {
                estimates[cell.index(m.n)].design =
                    "matched_point_pool_rare_tail_confirm_more".into();
                added += 1;
            }
        }
        work += extra_work;
        found += added;
        let preserved = before
            .iter()
            .all(|(i, e)| *e == serde_json::to_value(&estimates[*i]).unwrap());
        emit(
            log,
            "rust_odds_rare_tail_confirm_more_summary",
            json!({
                "group":m.request.id,"fraction":more_fraction,"allowance_ms":extra_allowance,
                "eligible":eligible,"funded":funded,"completed":completed,"accepted":added,
                "work":extra_work,"preserved_existing_estimates":preserved,
                "elapsed_ms":extra_start.elapsed().as_secs_f64()*1000.
            }),
        );
    }
    if extension {
        emit(
            log,
            "rust_odds_rare_tail_extension_summary",
            json!({"group":m.request.id,"mode":extension_mode,"ordinary_finalists":plans.len(),"unfunded_pilots":pending.len(),"accepted":extra_found,"main_draws":extra_draws,"final_phase_ms":ordinary_final_phase_ms,"deadline_ms":extension_deadline_ms}),
        );
    }
    emit(
        log,
        "rust_odds_rare_tail_summary",
        json!({"group":m.request.id,"cells":cells.len(),"finalists":plans.len(),"accepted":found,"elapsed_ms":start.elapsed().as_secs_f64()*1000.,"allowance_ms":allowance_ms,"remaining_after_training_ms":remaining_ms,"work":work,"batch_allocation":value("RUST_ODDS_RARE_TAIL_BATCH_ALLOCATION"),"root_retry":value("RUST_ODDS_RARE_TAIL_RETRY"),"union_patterns":value("RUST_ODDS_RARE_TAIL_UNION_PATTERNS"),"extension":extension_mode,"effective_union_patterns":if extension{"256".into()}else{value("RUST_ODDS_RARE_TAIL_UNION_PATTERNS")},"effective_root_retry":if extension{"0".into()}else{value("RUST_ODDS_RARE_TAIL_RETRY")},"effective_batch_allocation":if extension{"0".into()}else{value("RUST_ODDS_RARE_TAIL_BATCH_ALLOCATION")},"shared_guides":shared.as_ref().map(|s|s.describe())}),
    );
    (found, work)
}
// Ordinary results are committed first; extension results can only fill zeros.
fn commit_estimate(est: &mut Estimate, result: &Result, extension: bool) -> bool {
    if extension && est.probability != 0. {
        return false;
    }
    apply(
        est,
        result,
        if extension {
            "matched_point_pool_rare_tail_extension"
        } else {
            "matched_point_pool_rare_tail"
        },
    );
    true
}

fn confirmation_fraction(value: &str) -> f64 {
    value
        .parse::<f64>()
        .ok()
        .filter(|f| f.is_finite() && *f > 0. && *f <= 2.)
        .unwrap_or(0.)
}

fn confirmation_batches(pilot: &Result) -> Option<usize> {
    if pilot.samples == 0 || pilot.hits == 0 || !pilot.ess.is_finite() || pilot.ess <= 0. {
        return None;
    }
    // Both independent batches target enough hits and effective samples. Pilot
    // observations and earlier failures never enter the published estimate.
    let n = (120. * pilot.samples as f64 / pilot.hits as f64)
        .max(32. * pilot.samples as f64 / pilot.ess)
        .ceil() as usize;
    Some(n.clamp(2000, 30000))
}

fn reserve_extension(
    main: usize,
    check: usize,
    pilot_samples: usize,
    pilot_ess: f64,
    draw_ms: f64,
    spare_ms: f64,
) -> Option<(usize, usize, f64)> {
    if main > 6000 || pilot_ess >= 12. {
        return None;
    }
    let desired = (32. * pilot_samples as f64 / pilot_ess.max(0.1)).ceil() as usize;
    let desired = desired.clamp(main, 6000);
    let desired_check = (desired / 2).max(check);
    let extra = draw_ms * (desired + desired_check - main - check) as f64;
    (extra <= spare_ms).then_some((desired, desired_check, extra))
}

pub(crate) fn accepted(main: &Result, check: &Result, rough: bool) -> bool {
    if !rough {
        return confirmed(main, check);
    }
    publishable(main, true)
        && check.hits >= 30
        && check.ess >= 3.
        && check.probability > 0.
        && check.std_err / check.probability <= 0.75
        && (0.2..=5.).contains(&(main.probability / check.probability))
}
fn publishable(r: &Result, rough: bool) -> bool {
    if !rough {
        return r.coarse();
    }
    r.weighted
        && r.probability > 0.
        && r.hits >= 30
        && r.ess >= 4.
        && r.std_err / r.probability <= 0.6
        && r.max_share <= 0.35
        && r.batch_gap <= 1.5
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extension_preserves_existing_estimate_and_its_metadata() {
        let r = Result {
            probability: 2e-26,
            ess: 20.,
            hits: 100,
            samples: 2000,
            ..Result::default()
        };
        let mut est = Estimate {
            probability: 1e-25,
            std_err: 2e-26,
            design: "ordinary".into(),
            reachability: "witness".into(),
            ..Estimate::default()
        };
        let before = serde_json::to_value(&est).unwrap();
        assert!(!commit_estimate(&mut est, &r, true));
        assert_eq!(serde_json::to_value(&est).unwrap(), before);
        est.probability = 0.;
        assert!(commit_estimate(&mut est, &r, true));
        assert_eq!(est.probability, r.probability);
        assert_eq!(est.design, "matched_point_pool_rare_tail_extension");
        assert_eq!(est.reachability, "witness");
    }
    #[test]
    fn coverage_enables_larger_confirmation_with_explicit_overrides() {
        let flag = "RUST_ODDS_RARE_TAIL_CONFIRM_MORE";
        assert_eq!(resolve_value(flag, None, true), "1.5");
        assert_eq!(resolve_value(flag, None, false), "");
        for value in ["0", "0.25", "1.5", ""] {
            assert_eq!(resolve_value(flag, Some(value.into()), true), value);
        }
    }
    #[test]
    fn extra_confirmation_rejects_empty_pilots_and_sizes_fresh_fixed_batches() {
        let mut pilot = Result {
            samples: 1000,
            hits: 0,
            ess: 0.,
            ..Result::default()
        };
        assert_eq!(confirmation_batches(&pilot), None);
        pilot.hits = 260;
        pilot.ess = 23.;
        assert_eq!(confirmation_batches(&pilot), Some(2000));
        pilot.hits = 4;
        pilot.ess = 2.8;
        assert_eq!(confirmation_batches(&pilot), Some(30000));
        pilot.hits = 8;
        pilot.ess = 1.18;
        assert_eq!(confirmation_batches(&pilot), Some(27119));
        pilot.ess = f64::NAN;
        assert_eq!(confirmation_batches(&pilot), None);
        for invalid in ["", "0", "-1", "NaN", "inf", "2.1"] {
            assert_eq!(confirmation_fraction(invalid), 0.);
        }
        assert_eq!(confirmation_fraction("0.25"), 0.25);
        assert_eq!(confirmation_fraction("1.5"), 1.5);
    }
    #[test]
    fn coverage_defaults_to_sequential_retries_and_allows_rollback() {
        assert_eq!(
            resolve_value("RUST_ODDS_RARE_TAIL_EXTENSION", None, true),
            "after"
        );
        assert_eq!(
            resolve_value("RUST_ODDS_RARE_TAIL_EXTENSION", None, false),
            ""
        );
        for mode in ["0", "after", "overlap", ""] {
            assert_eq!(
                resolve_value("RUST_ODDS_RARE_TAIL_EXTENSION", Some(mode.into()), true),
                mode
            );
        }
    }
    #[test]
    fn retry_requires_independent_consistent_confirmation() {
        let main = Result {
            weighted: true,
            probability: 1e-26,
            std_err: 2e-27,
            hits: 100,
            ess: 20.,
            max_share: 0.1,
            batch_gap: 0.2,
            ..Result::default()
        };
        assert!(accepted(&main, &main, true));
        assert!(!accepted(&main, &Result::default(), true));
        let mut check = main.clone();
        check.probability = 1e-24;
        assert!(!accepted(&main, &check, true));
        check.probability = main.probability;
        check.hits = 29;
        assert!(!accepted(&main, &check, true));
    }
    #[test]
    fn spare_capacity_only_extends_fixed_main_and_confirmation_batches() {
        assert!(reserve_extension(1000, 1000, 1000, 10., 0.01, 27.).is_none());
        let (main, check, extra) = reserve_extension(1000, 1000, 1000, 10., 0.01, 31.).unwrap();
        assert_eq!((main, check), (3200, 1600));
        assert_eq!(extra, 28.);
        assert!(extra <= 31.);
        assert!(reserve_extension(1000, 1000, 1000, 20., 0.01, 100.).is_none());
        assert!(reserve_extension(8000, 4000, 1000, 2., 0.01, 100.).is_none());
        assert_eq!(
            reserve_extension(6000, 3000, 1000, 2., 0.01, 0.),
            Some((6000, 3000, 0.))
        );
    }
    #[test]
    fn coverage_retains_previous_tail_defaults_and_allows_experiment_opt_in() {
        for (flag, expected, experiment) in [
            ("RUST_ODDS_RARE_TAIL_UNION_PATTERNS", "256", "64"),
            ("RUST_ODDS_RARE_TAIL_RETRY", "0", "roots"),
            ("RUST_ODDS_RARE_TAIL_BATCH_ALLOCATION", "0", "reserve"),
        ] {
            assert_eq!(resolve_value(flag, None, true), expected);
            assert_eq!(resolve_value(flag, None, false), "");
            assert_eq!(
                resolve_value(flag, Some(experiment.into()), true),
                experiment
            );
        }
    }
    #[test]
    fn absent_profile_selects_coverage_and_preserves_explicit_modes() {
        assert_eq!(resolve_profile(None), "coverage");
        for mode in ["coverage", "0", "union", "lazy", "portfolio", ""] {
            assert_eq!(resolve_profile(Some(mode.into())), mode);
        }
        let coverage = resolve_profile(None) == "coverage";
        assert_eq!(
            resolve_value("RUST_ODDS_SHARED_CONSTRAINTS", None, coverage),
            "guided"
        );
    }
    #[test]
    fn coverage_enables_the_tested_shared_profile_with_explicit_rollback() {
        for (flag, expected) in [
            ("RUST_ODDS_SHARED_CONSTRAINTS", "guided"),
            ("RUST_ODDS_SHARED_CONSTRAINTS_BLOCKERS", "2"),
            ("RUST_ODDS_SHARED_CONSTRAINTS_RELATIVE", "1"),
            ("RUST_ODDS_SHARED_CONSTRAINTS_CONFIRMATION", "parallel"),
            ("RUST_ODDS_SHARED_CONSTRAINTS_FRACTION", "0.5"),
        ] {
            assert_eq!(resolve_value(flag, None, true), expected);
            assert_eq!(resolve_value(flag, None, false), "");
            assert_eq!(resolve_value(flag, Some("0".into()), true), "0");
        }
    }
    #[test]
    fn profile_defaults_leave_rejected_experiments_disabled() {
        assert_eq!(profile_default("RUST_ODDS_LAZY_RIVALS"), "4");
        assert_eq!(profile_default("RUST_ODDS_LAZY_GUIDE"), "cardinality");
        assert_eq!(profile_default("RUST_ODDS_RARE_TAIL_QUALITY"), "order");
        assert_eq!(
            profile_default("RUST_ODDS_RARE_TAIL_BUDGET_FRACTION"),
            "0.45"
        );
        for flag in [
            "RUST_ODDS_LAZY_FIXTURE_BIAS",
            "RUST_ODDS_LAZY_CARDINALITY_CASES",
            "RUST_ODDS_POINT_TILT_FINAL",
            "RUST_ODDS_LAZY_PROPAGATE",
        ] {
            assert_eq!(profile_default(flag), "");
        }
    }
    #[test]
    fn rough_gate_still_rejects_single_dominant_weight_and_missing_evidence() {
        let mut r = Result {
            weighted: true,
            probability: 1e-25,
            std_err: 5e-26,
            hits: 100,
            ess: 5.,
            max_share: 0.3,
            batch_gap: 1.2,
            ..Default::default()
        };
        assert!(publishable(&r, true));
        assert!(!publishable(&r, false));
        r.max_share = 0.6;
        assert!(!publishable(&r, true));
        r.max_share = 0.3;
        r.hits = 1;
        assert!(!publishable(&r, true));
        r.hits = 100;
        r.probability = 0.;
        assert!(!publishable(&r, true));
    }
}
fn emit(log: Option<&crate::logging::RequestLog>, event: &str, fields: serde_json::Value) {
    if let Some(log) = log {
        log.event(event, fields);
    }
}
