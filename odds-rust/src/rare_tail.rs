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
use std::time::Instant;

// A single opt-in profile, with explicit environment overrides. Reading it does
// not mutate the process environment or interfere with concurrent requests.
pub fn value(name: &str) -> String {
    resolve_value(
        name,
        std::env::var(name).ok(),
        std::env::var("RUST_ODDS_RARE_TAIL").as_deref() == Ok("coverage"),
    )
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
        "RUST_ODDS_RARE_TAIL_BUDGET_FRACTION" => "0.35",
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
    let mode = std::env::var("RUST_ODDS_RARE_TAIL").unwrap_or_default();
    let mode = if mode == "coverage" {
        "portfolio".to_string()
    } else {
        mode
    };
    if !["union", "lazy", "portfolio"].contains(&mode.as_str()) {
        return (0, 0);
    }
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
    let allowance_ms = log.map_or(unlogged_search_ms, |l| l.calculation_elapsed_ms()) * fraction;
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
                    rivals: 6,
                    target_patterns: 256,
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
                let mut r = p.sample(m, 1000, derive(stream, "lazy"));
                let mut pilot_ms = clock.elapsed().as_secs_f64() * 1000.;
                pilot_work += r.work;
                if value("RUST_ODDS_LAZY_SUBSET") == "adaptive" && r.ess < 12. {
                    p.add_alternates(m, &seeds);
                    let clock = Instant::now();
                    let alt = p.sample(m, 1000, derive(stream, "multimode"));
                    let alt_ms = clock.elapsed().as_secs_f64() * 1000.;
                    pilot_work += alt.work;
                    if alt.ess > r.ess {
                        r = alt;
                        pilot_ms = alt_ms;
                    } else {
                        p.clear_alternates();
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
    plans.retain(|p| p.2.hits >= 3 && p.2.ess >= if rough { 1.5 } else { 2. });
    plans.truncate(finalists);
    let remaining_ms = (allowance_ms - start.elapsed().as_secs_f64() * 1000.).max(0.);
    let mut bins = vec![0_f64; workers.clamp(1, 4)];
    let mut funded = Vec::new();
    for p in plans {
        let n = if mode == "union" {
            8000
        } else {
            (((if rough { 40. } else { 60. }) * p.2.samples as f64 / p.2.hits.max(1) as f64)
                .max((if rough { 10. } else { 20. }) * p.2.samples as f64 / p.2.ess.max(0.1))
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
        }
    }
    let plans = funded;
    let results = parallel(plans.len(), workers, |i| {
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
        emit(
            log,
            "rust_odds_rare_tail",
            json!({"group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,"setup_ms":setup_ms,"pilot_ess":pilot.ess,"pilot_hits":pilot.hits,"main_draws":main.samples,"main_hits":main.hits,"main_ess":main.ess,"main_max_share":main.max_share,"main_batch_gap":main.batch_gap,"check_relative_se":if check.probability>0.{Some(check.std_err/check.probability)}else{None},"quality":if rough{"order"}else{"current"},"probability":main.probability,"relative_se":if main.probability>0.{Some(main.std_err/main.probability)}else{None},"check_probability":check.probability,"check_ess":check.ess,"accepted":accepted,"sampling_ms":start.elapsed().as_secs_f64()*1000.,"plan":plan.describe(m)}),
        );
        (*cell, main, check.work, accepted)
    });
    let mut found = shared_found;
    for (cell, r, check_work, accepted) in results {
        work += r.work + check_work;
        if accepted {
            apply(
                &mut estimates[cell.index(m.n)],
                &r,
                "matched_point_pool_rare_tail",
            );
            found += 1;
        }
    }
    emit(
        log,
        "rust_odds_rare_tail_summary",
        json!({"group":m.request.id,"cells":cells.len(),"finalists":plans.len(),"accepted":found,"elapsed_ms":start.elapsed().as_secs_f64()*1000.,"allowance_ms":allowance_ms,"remaining_after_training_ms":remaining_ms,"work":work,"shared_guides":shared.as_ref().map(|s|s.describe())}),
    );
    (found, work)
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
            "0.35"
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
