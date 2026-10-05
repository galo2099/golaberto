//! Offline complete-branch experiment. No server or database writes.
use golaberto_odds::{
    conditioned::Result,
    joint_caps::propagated::lazy::{combine_branches, BranchSample, BranchStrata},
    model::{Model, Request},
    rng::derive,
    search::{parallel, Cell},
};
use serde_json::json;
use std::{env, fs, time::Instant};
fn info(r: &Result) -> serde_json::Value {
    json!({"samples":r.samples,"hits":r.hits,"probability":r.probability,"std_err":r.std_err,"ess":r.ess,"max_share":r.max_share,"batch_gap":r.batch_gap,
        "relative_se":if r.probability>0.{Some(r.std_err/r.probability)}else{None},
        "operations":{"fixtures":r.operations.fixtures,"guidance":r.operations.guidance,"ranking":r.operations.ranking,"units":r.operations.units()}})
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    if a.len() < 5 {
        return Err("REQUEST.json TEAM RANK TOTAL_DRAWS [SEEDS] [equal|trained] [rank|bounds|adaptive] [RIVALS] [tilt|native] [plain|refine] [PILOT_DRAWS] [ALLOCATION_FLOOR] [off|prior|joint|strong] [REFERENCE_PROBABILITY] [RELATIVE_TOLERANCE]".into());
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
        BranchStrata::with_tree(&m, cell, 808, rivals, leaves, training, values)?
    } else {
        BranchStrata::with_secondary(&m, cell, 808, rivals, refine)?
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
        json!({"event":"strata_setup","model_ms":model_ms,"setup_ms":setup_ms,"modeled_setup_work_units":setup_work_units,"bound_setup_ms":bound_setup_ms,"bound_mode":bound_mode,
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
        p.clear_messages();
        let clock = Instant::now();
        let pilots = parallel(p.len(), 4, |i| {
            let start = Instant::now();
            if production_profile {
                let bounded = p.sample(
                    &m,
                    i,
                    pilot_n,
                    derive(seed, &format!("branch-bound-pilot-{i}")),
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
                derive(seed, &format!("branch-pilot-{i}")),
                false,
                goals,
            );
            let bound = if guide != "rank" {
                Some(p.sample(
                    &m,
                    i,
                    pilot_n,
                    derive(seed, &format!("branch-bound-pilot-{i}")),
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
            let info = p.prepare_messages(&m, &rs, messages);
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
                        derive(seed, &format!("message-pilot-{i}")),
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
                        derive(seed, &format!("message-pilot-{i}")),
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
        let scores: Vec<_> = pilots
            .iter()
            .map(|(r, _, _, _)| r.std_err * (r.samples as f64).sqrt())
            .collect();
        let score_sum: f64 = scores.iter().sum();
        let mut ns = vec![floor; p.len()];
        let left = total - floor * p.len();
        for i in 0..p.len() {
            let share = if allocation == "trained" && score_sum > 0. {
                0.1 / p.len() as f64 + 0.9 * scores[i] / score_sum
            } else {
                1. / p.len() as f64
            };
            ns[i] += (left as f64 * share).floor() as usize;
        }
        let assigned: usize = ns.iter().sum();
        for i in 0..total - assigned {
            ns[i % p.len()] += 1;
        }
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
            json!({"event":"strata_result","seed":seed,"allocation":allocation,"guide":guide,"production_profile":production_profile,"goal_tilt":goals,"rivals":rivals,"refined":refine,"pilot_draws":pilot_n,"allocation_floor":floor,
            "modeled_cost_units":{"setup":setup_work_units,"pilots_all_sampled":pilot_work_units,"messages":message_work_units,"main":main_work_units,"check":check_work_units,"total":setup_work_units.map(|setup|setup as usize+pilot_work_units+message_work_units+main_work_units+check_work_units)},
            "bound_mode":bound_mode,"omitted_upper":omitted_upper,"skipped":skipped.iter().filter(|&&s|s).count(),
            "message_ms":message_ms,"messages":message_info,"pilot_ms":pilot_ms,"main_ms":main_ms,"check_ms":check_ms,"elapsed_ms":clock.elapsed().as_secs_f64()*1000.,
            "main":info(&main),"check":info(&check),"accepted":golaberto_odds::rare_tail::accepted(&main,&check,true),
            "branches":(0..p.len()).map(|i|json!({"index":i,"draws":if skipped[i]{0}else{ns[i]},"planned_draws":ns[i],"skipped":skipped[i],"bounds":pilots[i].1,"pilot":info(&pilots[i].0),"pilot_ms":pilots[i].2,"main":info(&mains[i].result),"check":info(&checks[i].result)})).collect::<Vec<_>>() })
        );
    }
    Ok(())
}
