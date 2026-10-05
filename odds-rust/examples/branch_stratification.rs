//! Offline complete-branch experiment. No server or database writes.
#![recursion_limit = "256"]
use golaberto_odds::{
    conditioned::Result,
    joint_caps::propagated::lazy::{combine_branches, BranchSample, BranchStrata},
    model::{Model, Request},
    rng::derive,
    search::{parallel, Cell},
};
use serde_json::json;
use std::{env, fs, time::Instant};

const SETUP_SEED: i64 = 808;

fn prior_scores(empirical_sd: &[f64], samples: &[usize], bounds: &[f64]) -> Option<Vec<f64>> {
    if empirical_sd.is_empty()
        || empirical_sd.len() != samples.len()
        || empirical_sd.len() != bounds.len()
        || empirical_sd.iter().any(|v| !v.is_finite() || *v < 0.0)
        || bounds.iter().any(|v| !v.is_finite())
    {
        return None;
    }
    let max_bound = bounds.iter().map(|v| v.max(0.0)).fold(0.0, f64::max);
    let max_sd = empirical_sd.iter().copied().fold(0.0, f64::max);
    let scores = if max_sd > 0.0 && max_bound > 0.0 {
        empirical_sd
            .iter()
            .zip(samples)
            .zip(bounds)
            .map(|((&sd, &n), &bound)| {
                let n = n.max(1) as f64;
                let empirical = sd * (n / (n + 20.0)).sqrt();
                let prior = max_sd * (bound.max(0.0) / max_bound) * (20.0 / (n + 20.0)).sqrt();
                empirical.hypot(prior)
            })
            .collect()
    } else if max_sd == 0.0 && max_bound > 0.0 {
        bounds.iter().map(|b| b.max(0.0) / max_bound).collect()
    } else {
        empirical_sd.to_vec()
    };
    scores
        .iter()
        .all(|v: &f64| v.is_finite() && *v >= 0.0)
        .then_some(scores)
}

fn resolve_pilot_seed(row_seed: i64, override_seed: Option<i64>) -> i64 {
    override_seed.unwrap_or(row_seed)
}

fn parse_pilot_seed(
    raw: Option<&str>,
) -> std::result::Result<Option<i64>, std::num::ParseIntError> {
    raw.map(str::parse).transpose()
}

fn allocation_counts(
    scores: &[f64],
    total: usize,
    floor: usize,
    allocation: &str,
) -> Option<Vec<usize>> {
    if scores.is_empty()
        || scores.iter().any(|v| !v.is_finite() || *v < 0.0)
        || total < scores.len().checked_mul(floor)?
    {
        return None;
    }
    let sum: f64 = scores.iter().sum();
    let left = total - floor * scores.len();
    let mut counts = vec![floor; scores.len()];
    let use_scores = allocation != "equal" && sum > 0.0;
    for (i, count) in counts.iter_mut().enumerate() {
        let share = if use_scores {
            0.1 / scores.len() as f64 + 0.9 * scores[i] / sum
        } else {
            1.0 / scores.len() as f64
        };
        *count += (left as f64 * share).floor() as usize;
    }
    let assigned: usize = counts.iter().sum();
    for i in 0..total - assigned {
        counts[i % scores.len()] += 1;
    }
    Some(counts)
}

fn witness_summary(m: &Model, witness: Option<&[u8]>) -> serde_json::Value {
    let Some(outcomes) = witness else {
        return serde_json::Value::Null;
    };
    if outcomes.len() != m.fixtures.len() {
        return serde_json::Value::Null;
    }
    let mut campaign = m.base.clone();
    let mut mapped = Vec::with_capacity(outcomes.len());
    for (i, &outcome) in outcomes.iter().enumerate() {
        let g = &m.fixtures[i];
        if outcome > 2 {
            return serde_json::Value::Null;
        }
        let request_game = &m.request.games[g.request_index];
        mapped.push(json!({"fixture_id":request_game.id,"home_team_id":m.ids[g.home],"away_team_id":m.ids[g.away],"outcome":outcome}));
        m.add(
            &mut campaign,
            i,
            golaberto_odds::conditioned::canonical(outcome),
        );
    }
    json!({"kind":"first_hit_outcome_witness_outcomes_only_actual_goal_scores_not_retained","fixtures":mapped,"teams":(0..m.n).map(|i|json!({"team_id":m.ids[i],"points":campaign[i].points,"wins":campaign[i].wins})).collect::<Vec<_>>()})
}
fn info(r: &Result) -> serde_json::Value {
    json!({"samples":r.samples,"hits":r.hits,"probability":r.probability,"std_err":r.std_err,"ess":r.ess,"max_share":r.max_share,"batch_gap":r.batch_gap,
        "relative_se":if r.probability>0.{Some(r.std_err/r.probability)}else{None},
        "operations":{"fixtures":r.operations.fixtures,"guidance":r.operations.guidance,"ranking":r.operations.ranking,"units":r.operations.units()}})
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    if a.len() < 5 {
        return Err("REQUEST.json TEAM RANK TOTAL_DRAWS [SEEDS] [equal|trained|prior] [rank|bounds|adaptive] [RIVALS] [tilt|native] [plain|refine] [PILOT_DRAWS] [ALLOCATION_FLOOR] [off|prior|joint|strong] [REFERENCE_PROBABILITY] [RELATIVE_TOLERANCE]; TREE_PILOT_SEED optionally fixes only pilot/message training streams".into());
    }
    let request = fs::read(&a[1])?;
    let model_start = Instant::now();
    let m = Model::new(serde_json::from_slice::<Request>(&request)?)?;
    let model_ms = model_start.elapsed().as_secs_f64() * 1000.;
    let cell = Cell {
        team: m.indices[&a[2].parse()?],
        rank: a[3].parse::<usize>()? - 1,
    };
    let total = a[4].parse::<usize>()?;
    let seeds: Vec<i64> = a
        .get(5)
        .map_or("808,1669,1993,1847,1861", |s| s)
        .split(',')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()?;
    let allocation = a.get(6).map_or("equal", |s| s.as_str());
    if !["equal", "trained", "prior"].contains(&allocation) {
        return Err("allocation must be equal, trained, or prior".into());
    }
    let guide = a.get(7).map_or("rank", |s| s.as_str());
    let production_profile = env::var("TREE_PRODUCTION_PROFILE").as_deref() == Ok("1");
    let rivals = a.get(8).map_or(Ok(4), |s| s.parse::<usize>())?;
    let goals = a.get(9).map_or(true, |s| s != "native");
    let refine = a.get(10).is_some_and(|s| s == "refine");
    let pilot_n = a.get(11).map_or(Ok(2000), |s| s.parse::<usize>())?;
    let floor = a
        .get(12)
        .map_or(Ok(if refine { 200 } else { 2000 }), |s| s.parse::<usize>())?;
    if pilot_n < 2 || floor < 2 {
        return Err("pilot and branch draw floors must be at least two".into());
    }
    let setup_clock = Instant::now();
    let cert = golaberto_odds::target_limits::certify(&m, cell, 16000);
    let leaves = std::env::var("TREE_LEAVES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(32);
    let training = std::env::var("TREE_TRAINING")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);
    let values = std::env::var("TREE_GUIDE_VALUES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4000000);
    let mut p = if std::env::var("TREE_MODE").as_deref() == Ok("1") {
        BranchStrata::with_tree(&m, cell, SETUP_SEED, rivals, leaves, training, values)?
    } else {
        BranchStrata::with_secondary(&m, cell, SETUP_SEED, rivals, refine)?
    };
    let _ = cert;
    let setup_ms = setup_clock.elapsed().as_secs_f64() * 1000.;
    let proposal = p.describe(&m);
    let setup_work_units = proposal
        .get("tree")
        .and_then(|tree| tree.get("modeled_setup_work"))
        .and_then(serde_json::Value::as_u64);
    let bound_mode = a.get(13).map_or("off", |s| s.as_str());
    let reference = a.get(14).map_or(Ok(4e-34), |s| s.parse::<f64>())?;
    let tolerance = a.get(15).map_or(Ok(0.01), |s| s.parse::<f64>())?;
    if !["off", "prior", "joint", "strong"].contains(&bound_mode)
        || reference <= 0.
        || !reference.is_finite()
        || tolerance < 0.
        || !tolerance.is_finite()
    {
        return Err("invalid bound mode/reference/tolerance".into());
    }
    let bound_clock = Instant::now();
    let bounds = (bound_mode != "off").then(|| p.bounds(&m, bound_mode == "strong"));
    let bound_setup_ms = bound_clock.elapsed().as_secs_f64() * 1000.;
    let mut order: Vec<_> = (0..p.len()).collect();
    let mut skipped = vec![false; p.len()];
    let mut omitted_upper = 0.;
    if let Some(bs) = &bounds {
        let upper = |i: usize| {
            if bound_mode == "prior" {
                bs[i].fixed_prior * (1. + 1e-9)
            } else {
                bs[i].upper_bound
            }
        };
        order.sort_by(|&a, &b| upper(b).total_cmp(&upper(a)).then(a.cmp(&b)));
        for &i in order.iter().rev() {
            if omitted_upper + upper(i) > reference * tolerance {
                break;
            }
            omitted_upper += upper(i);
            skipped[i] = true;
        }
    }
    println!(
        "{}",
        json!({"event":"strata_setup","model_ms":model_ms,"setup_ms":setup_ms,"modeled_setup_work_units":setup_work_units,"setup_seed":SETUP_SEED,"bound_setup_ms":bound_setup_ms,"bound_mode":bound_mode,
        "omitted_upper":omitted_upper,"skipped":skipped,"order":order,"reference_probability":reference,"relative_tolerance":tolerance,
        "workers":4,"proposal":proposal})
    );
    if total == 0 {
        return Ok(());
    }
    if total < p.len() * floor {
        return Err("total draws must cover every stratum at the allocation floor".into());
    }
    for seed in seeds {
        let pilot_seed_value = env::var("TREE_PILOT_SEED").ok();
        let pilot_seed_override = parse_pilot_seed(pilot_seed_value.as_deref())?;
        let pilot_seed = resolve_pilot_seed(seed, pilot_seed_override);
        p.clear_messages();
        let clock = Instant::now();
        let pilots = parallel(p.len(), 4, |i| {
            let start = Instant::now();
            if production_profile {
                let bounded = p.sample(
                    &m,
                    i,
                    pilot_n,
                    derive(pilot_seed, &format!("branch-bound-pilot-{i}")),
                    true,
                    goals,
                );
                let work = bounded.result.operations.units();
                return (
                    bounded.result,
                    true,
                    start.elapsed().as_secs_f64() * 1000.,
                    work,
                );
            }
            let rank = p.sample(
                &m,
                i,
                pilot_n,
                derive(pilot_seed, &format!("branch-pilot-{i}")),
                false,
                goals,
            );
            let bound = if guide != "rank" {
                Some(p.sample(
                    &m,
                    i,
                    pilot_n,
                    derive(pilot_seed, &format!("branch-bound-pilot-{i}")),
                    true,
                    goals,
                ))
            } else {
                None
            };
            let pilot_work_units = rank.result.operations.units().saturating_add(
                bound
                    .as_ref()
                    .map_or(0, |sample| sample.result.operations.units()),
            );
            let use_bounds = guide == "bounds"
                || (guide == "adaptive"
                    && bound
                        .as_ref()
                        .is_some_and(|b| b.result.ess > rank.result.ess));
            let result = if use_bounds {
                bound.unwrap().result
            } else {
                rank.result
            };
            (
                result,
                use_bounds,
                start.elapsed().as_secs_f64() * 1000.,
                pilot_work_units,
            )
        });
        let mut pilots = pilots;
        let mut pilot_work_units: usize = pilots.iter().map(|pilot| pilot.3).sum();
        let message_clock = Instant::now();
        let messages = std::env::var("TREE_MESSAGES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let message_info = if messages > 0 {
            let rs: Vec<_> = pilots.iter().map(|s| s.0.clone()).collect();
            let info = p.prepare_messages(&m, &rs, messages, 20_000_000);
            if production_profile {
                let selected = info["selected"]
                    .as_array()
                    .ok_or("message preparation omitted selected pilot branches")?
                    .iter()
                    .map(|index| index.as_u64().ok_or("invalid selected pilot index"))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                let revised = parallel(selected.len(), 4, |slot| {
                    let i = selected[slot] as usize;
                    let bounded = p.sample(
                        &m,
                        i,
                        pilot_n,
                        derive(pilot_seed, &format!("message-pilot-{i}")),
                        true,
                        goals,
                    );
                    let work = bounded.result.operations.units();
                    (i, (bounded.result, true, 0., work))
                });
                for (i, row) in revised {
                    pilot_work_units = pilot_work_units.saturating_add(row.3);
                    pilots[i] = row;
                }
            } else {
                // Training chooses the proposal; all main/check draws stay fresh.
                pilots = parallel(p.len(), 4, |i| {
                    let bounds = guide != "rank";
                    let r = p.sample(
                        &m,
                        i,
                        pilot_n,
                        derive(pilot_seed, &format!("message-pilot-{i}")),
                        bounds,
                        goals,
                    );
                    let work = r.result.operations.units();
                    (r.result, bounds, 0., work)
                });
                pilot_work_units = pilot_work_units
                    .saturating_add(pilots.iter().map(|pilot| pilot.3).sum::<usize>());
            }
            info
        } else {
            serde_json::Value::Null
        };
        let message_ms = message_clock.elapsed().as_secs_f64() * 1000.;
        let message_work_units = message_info
            .get("work")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
        let pilot_ms = clock.elapsed().as_secs_f64() * 1000.;
        let empirical_scores: Vec<_> = pilots
            .iter()
            .map(|(r, _, _, _)| r.std_err * (r.samples as f64).sqrt())
            .collect();
        let allocation_bounds_clock = Instant::now();
        let allocation_bounds = if allocation == "prior" {
            Some(p.bounds(&m, false))
        } else {
            None
        };
        let allocation_bounds_ms = allocation_bounds_clock.elapsed().as_secs_f64() * 1000.;
        let prior_scores = allocation_bounds.as_ref().and_then(|bs| {
            let upper: Vec<_> = bs.iter().map(|b| b.upper_bound).collect();
            prior_scores(
                &empirical_scores,
                &pilots.iter().map(|p| p.0.samples).collect::<Vec<_>>(),
                &upper,
            )
        });
        let scores = if allocation == "prior" {
            prior_scores
                .clone()
                .ok_or("could not form finite prior allocation scores")?
        } else {
            empirical_scores.clone()
        };
        let ns = allocation_counts(&scores, total, floor, allocation)
            .ok_or("invalid allocation or insufficient draw budget")?;
        let bounds_metadata = allocation_bounds.as_ref().map(|bs| bs.iter().enumerate().map(|(i,b)| json!({"index":i,"fixed_prior":b.fixed_prior,"domain_prior":b.domain_prior,"normalizer_upper":b.normalizer_upper,"read_two_upper":b.read_two_upper,"upper_bound":b.upper_bound})).collect::<Vec<_>>());
        let main_clock = Instant::now();
        let mut mains = parallel(p.len(), 4, |slot| {
            let i = order[slot];
            (
                i,
                if skipped[i] {
                    BranchSample {
                        result: Result::default(),
                        left: 0.,
                        right: 0.,
                    }
                } else {
                    p.sample(
                        &m,
                        i,
                        ns[i],
                        derive(seed, &format!("branch-main-{i}")),
                        pilots[i].1,
                        goals,
                    )
                },
            )
        });
        mains.sort_by_key(|s| s.0);
        let mains: Vec<_> = mains.into_iter().map(|s| s.1).collect();
        let main_work_units: usize = mains
            .iter()
            .map(|sample| sample.result.operations.units())
            .sum();
        let main_ms = main_clock.elapsed().as_secs_f64() * 1000.;
        let check_clock = Instant::now();
        let mut checks = parallel(p.len(), 4, |slot| {
            let i = order[slot];
            (
                i,
                if skipped[i] {
                    BranchSample {
                        result: Result::default(),
                        left: 0.,
                        right: 0.,
                    }
                } else {
                    p.sample(
                        &m,
                        i,
                        ns[i],
                        derive(seed, &format!("branch-check-{i}")),
                        pilots[i].1,
                        goals,
                    )
                },
            )
        });
        checks.sort_by_key(|s| s.0);
        let checks: Vec<_> = checks.into_iter().map(|s| s.1).collect();
        let check_work_units: usize = checks
            .iter()
            .map(|sample| sample.result.operations.units())
            .sum();
        let check_ms = check_clock.elapsed().as_secs_f64() * 1000.;
        let main = combine_branches(&mains);
        let check = combine_branches(&checks);
        println!(
            "{}",
            json!({"event":"strata_result","seed":seed,"pilot_seed":pilot_seed,"pilot_seed_overridden":pilot_seed_override.is_some(),"setup_seed":SETUP_SEED,"allocation":allocation,"guide":guide,"production_profile":production_profile,"goal_tilt":goals,"rivals":rivals,"refined":refine,"pilot_draws":pilot_n,"allocation_floor":floor,"allocation_scores":scores,"empirical_sd_scores":empirical_scores,"allocation_bounds":bounds_metadata,"allocation_bounds_ms":allocation_bounds_ms,"allocation_bounds_work_units":null,"allocation_bounds_method":"p.bounds(m,false) upper_bound; work-unit counter unavailable",
            "modeled_cost_units":{"setup":setup_work_units,"pilots_all_sampled":pilot_work_units,"messages":message_work_units,"main":main_work_units,"check":check_work_units,"total":setup_work_units.map(|setup|setup as usize+pilot_work_units+message_work_units+main_work_units+check_work_units)},
            "bound_mode":bound_mode,"omitted_upper":omitted_upper,"skipped":skipped.iter().filter(|&&s|s).count(),
            "message_ms":message_ms,"messages":message_info,"pilot_ms":pilot_ms,"main_ms":main_ms,"check_ms":check_ms,"elapsed_ms":clock.elapsed().as_secs_f64()*1000.,
            "main":info(&main),"check":info(&check),"accepted":golaberto_odds::rare_tail::accepted(&main,&check,true),
            "branches":(0..p.len()).map(|i|json!({"index":i,"draws":if skipped[i]{0}else{ns[i]},"planned_draws":ns[i],"skipped":skipped[i],"bounds":pilots[i].1,"pilot":info(&pilots[i].0),"pilot_ms":pilots[i].2,"allocation_score":scores[i],"allocation_bound":allocation_bounds.as_ref().map(|b|b[i].upper_bound),"main":info(&mains[i].result),"main_outcome_witness":witness_summary(&m,mains[i].result.witness.as_deref()),"check":info(&checks[i].result),"check_outcome_witness":witness_summary(&m,checks[i].result.witness.as_deref())})).collect::<Vec<_>>() })
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prior_scores_handle_tiny_values_without_squaring() {
        let scores = prior_scores(&[1.0e-200, 2.0e-200], &[1, 1], &[1.0e-200, 2.0e-200]).unwrap();
        assert!(scores.iter().all(|v| v.is_finite() && *v > 0.0));
        assert!(scores[1] > scores[0]);
    }

    #[test]
    fn prior_scores_reject_invalid_inputs_and_use_bounds_when_all_hits_are_zero() {
        assert_eq!(
            prior_scores(&[0.0, 0.0], &[100, 100], &[0.1, 0.4]),
            Some(vec![0.25, 1.0])
        );
        assert!(prior_scores(&[f64::NAN], &[1], &[0.5]).is_none());
    }

    #[test]
    fn allocations_conserve_draws_and_respect_floors() {
        for mode in ["equal", "trained", "prior"] {
            let counts = allocation_counts(&[0.1, 0.9, 0.0], 1001, 17, mode).unwrap();
            assert_eq!(counts.iter().sum::<usize>(), 1001);
            assert!(counts.iter().all(|n| *n >= 17));
        }
        assert_eq!(
            allocation_counts(&[1.0, 2.0], 100, 10, "equal").unwrap(),
            vec![50, 50]
        );
    }

    #[test]
    fn pilot_seed_override_changes_training_seed_only() {
        assert_eq!(parse_pilot_seed(None).unwrap(), None);
        assert_eq!(parse_pilot_seed(Some("1669")).unwrap(), Some(1669));
        assert!(parse_pilot_seed(Some("bad")).is_err());
        assert_eq!(resolve_pilot_seed(808, None), 808);
        assert_eq!(resolve_pilot_seed(808, Some(1669)), 1669);
    }
}
