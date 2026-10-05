//! Paired offline diagnostic for early target-path root ranking.
//!
//! Prints one JSON object per arm and seed. Pilot results are diagnostic only;
//! MAIN and CHECK use independent streams and determine the existing rough gate.
use golaberto_odds::{
    conditioned::Result as EstimateResult,
    joint_caps::propagated::lazy::LazyJoint,
    model::{Key, Model, Request},
    rare_tail,
    rng::derive,
    search::Cell,
    target_limits,
};
use serde_json::{json, Value};
use std::{env, fs, time::Instant};

fn parse_seeds(value: &str) -> std::result::Result<Vec<i64>, String> {
    let seeds: Vec<i64> = value
        .split(',')
        .map(str::trim)
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()
        .map_err(|error: std::num::ParseIntError| error.to_string())?;
    if seeds.is_empty() {
        return Err("SEEDS must contain at least one integer".into());
    }
    Ok(seeds)
}

fn sample_phase(
    plan: &LazyJoint,
    model: &Model,
    samples: usize,
    seed: i64,
) -> (Value, f64, EstimateResult) {
    let started = Instant::now();
    let result = plan.sample(model, samples, seed);
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.;
    let summary = json!({
        "samples": result.samples,
        "hits": result.hits,
        "probability": result.probability,
        "std_err": result.std_err,
        "relative_se": (result.probability > 0.).then_some(result.std_err / result.probability),
        "ess": result.ess,
        "max_share": result.max_share,
        "batch_gap": result.batch_gap,
        "mass": result.mass,
        "operations": {
            "fixtures": result.operations.fixtures,
            "guidance": result.operations.guidance,
            "ranking": result.operations.ranking,
            "units": result.operations.units(),
        },
    });
    (summary, elapsed_ms, result)
}

fn total_wall_ms(model_ms: f64, certificate_ms: f64, setup_ms: f64, sampling: &Value) -> f64 {
    model_ms
        + certificate_ms
        + setup_ms
        + sampling
            .get("total_ms")
            .and_then(Value::as_f64)
            .unwrap_or(0.)
}

fn sampling_times(pilot_ms: f64, main_ms: f64, check_ms: f64) -> Value {
    json!({
        "pilot_ms": pilot_ms,
        "main_ms": main_ms,
        "check_ms": check_ms,
        "total_ms": pilot_ms + main_ms + check_ms,
    })
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 9 {
        return Err("early_rank_roots REQUEST.json TEAM_ID ONE_BASED_RANK MODE(order|bias) ROOTS DRAWS SEEDS(comma) WORK_LIMIT".into());
    }
    let request_start = Instant::now();
    let request: Request = serde_json::from_slice(&fs::read(&args[1])?)?;
    let model = Model::new(request)?;
    let model_ms = request_start.elapsed().as_secs_f64() * 1000.;
    let team_id: i32 = args[2].parse()?;
    let team = *model
        .indices
        .get(&team_id)
        .ok_or("TEAM_ID is not in the model")?;
    let rank_arg: usize = args[3].parse()?;
    if rank_arg == 0 || rank_arg > model.n {
        return Err("ONE_BASED_RANK must be within the group".into());
    }
    let mode = args[4].as_str();
    if !matches!(mode, "order" | "bias") {
        return Err("MODE must be order or bias".into());
    }
    let roots: usize = args[5].parse()?;
    if !(1..=32).contains(&roots) {
        return Err("ROOTS must be between 1 and 32".into());
    }
    let draws: usize = args[6].parse()?;
    if draws == 0 {
        return Err("DRAWS must be positive".into());
    }
    let seeds = parse_seeds(&args[7])?;
    let work_limit: usize = args[8].parse()?;
    if work_limit == 0 {
        return Err("WORK_LIMIT must be positive".into());
    }
    let cell = Cell {
        team,
        rank: rank_arg - 1,
    };
    let rules = &model.request.phase.championship;
    if model.keys.first() != Some(&Key::Pt)
        || model.request.phase.bonus_points != 0
        || (rules.point_win, rules.point_draw, rules.point_loss) != (3, 1, 0)
        || model.ids.len() != model.n
        || model.n > 32
        || model.fixtures.len() > 256
        || model.fixtures.iter().any(|game| game.home == game.away)
    {
        return Err("requires LazyJoint-supported points-first 3/1/0 model with at most 32 teams, 256 fixtures, and no self-fixtures".into());
    }

    let certificate_start = Instant::now();
    let certified = target_limits::certify(&model, cell, 5000);
    let certificate_ms = certificate_start.elapsed().as_secs_f64() * 1000.;
    let current_campaign = model.base[team];
    for seed in seeds {
        let cell_seed = derive(seed, &format!("early-rank-cell-{team_id}-{rank_arg}"));
        let setup_start = Instant::now();
        let baseline = LazyJoint::new(&model, cell, cell_seed, &[], 32);
        let baseline_setup_ms = setup_start.elapsed().as_secs_f64() * 1000.;
        let sampling_seed = |phase: &str| {
            derive(
                cell_seed,
                &format!("early-rank-{phase}-{team_id}-{rank_arg}"),
            )
        };
        if let Some(plan) = baseline {
            let (pilot, pilot_ms, _) = sample_phase(&plan, &model, 500, sampling_seed("pilot"));
            let (main, main_ms, main_result) =
                sample_phase(&plan, &model, draws, sampling_seed("main"));
            let (check, check_ms, check_result) =
                sample_phase(&plan, &model, draws, sampling_seed("check"));
            let sampling_ms = sampling_times(pilot_ms, main_ms, check_ms);
            let total_ms = total_wall_ms(model_ms, certificate_ms, baseline_setup_ms, &sampling_ms);
            println!(
                "{}",
                json!({
                    "event": "early_rank_screen",
                    "arm": "lazy_32_roots",
                    "seed": seed,
                    "cell": {"team_id": team_id, "one_based_rank": rank_arg},
                    "mode": mode,
                    "model_ms": model_ms,
                    "certificate_ms": certificate_ms,
                    "certificate": {"node_quota": 5000, "nodes": certified.nodes, "work": certified.work,
                        "lower": certified.lower, "upper": certified.upper},
                    "constructor_ms": baseline_setup_ms,
                    "pilot": pilot,
                    "main": main,
                    "check": check,
                    "sampling_ms": sampling_ms,
                    "total_wall_ms": total_ms,
                    "accepted": rare_tail::accepted(&main_result, &check_result, true),
                    "campaign_totals": {"points": current_campaign.points, "wins": current_campaign.wins},
                    "early_rank_diagnostics": Value::Null,
                    "proof_work_units": certified.work,
                })
            );
        } else {
            println!(
                "{}",
                json!({
                    "event": "early_rank_screen",
                    "arm": "lazy_32_roots",
                    "seed": seed,
                    "cell": {"team_id": team_id, "one_based_rank": rank_arg},
                    "status": "unsupported",
                    "model_ms": model_ms,
                    "certificate_ms": certificate_ms,
                    "certificate": {"node_quota": 5000, "nodes": certified.nodes, "work": certified.work,
                        "lower": certified.lower, "upper": certified.upper},
                })
            );
        }

        let setup_start = Instant::now();
        let (built, setup_work) =
            LazyJoint::with_early_rank(&model, cell, cell_seed, roots, mode == "bias", work_limit);
        let setup_ms = setup_start.elapsed().as_secs_f64() * 1000.;
        match built {
            Ok(plan) => {
                let diagnostics = plan.early_rank_diagnostics();
                let selected_paths = diagnostics
                    .get("selected_paths")
                    .cloned()
                    .unwrap_or_else(|| json!([]));
                let global_domain_mass = selected_paths
                    .as_array()
                    .map(|paths| {
                        paths
                            .iter()
                            .map(|path| {
                                json!({
                                    "key": path.get("key"),
                                    "global_domain_mass": path.get("global_domain_mass"),
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .map(Value::Array)
                    .unwrap_or(Value::Null);
                let (pilot, pilot_ms, _) = sample_phase(&plan, &model, 500, sampling_seed("pilot"));
                let (main, main_ms, main_result) =
                    sample_phase(&plan, &model, draws, sampling_seed("main"));
                let (check, check_ms, check_result) =
                    sample_phase(&plan, &model, draws, sampling_seed("check"));
                let sampling_ms = sampling_times(pilot_ms, main_ms, check_ms);
                let total_ms = total_wall_ms(model_ms, certificate_ms, setup_ms, &sampling_ms);
                println!(
                    "{}",
                    json!({
                        "event": "early_rank_screen",
                        "arm": format!("early_rank_{mode}"),
                        "seed": seed,
                        "cell": {"team_id": team_id, "one_based_rank": rank_arg},
                        "mode": mode,
                        "roots": roots,
                        "draws_per_final_stream": draws,
                        "work_limit": work_limit,
                        "model_ms": model_ms,
                        "certificate_ms": certificate_ms,
                        "certificate": {"node_quota": 5000, "nodes": certified.nodes, "work": certified.work,
                            "lower": certified.lower, "upper": certified.upper},
                        "constructor_ms": setup_ms,
                        "setup_work": setup_work,
                        "pilot": pilot,
                        "main": main,
                        "check": check,
                        "sampling_ms": sampling_ms,
                        "total_wall_ms": total_ms,
                        "accepted": rare_tail::accepted(&main_result, &check_result, true),
                        "campaign_totals": {"points": current_campaign.points, "wins": current_campaign.wins},
                        "early_rank_diagnostics": diagnostics,
                        "selected_path_scores_and_totals": selected_paths,
                        "global_domain_mass": global_domain_mass,
                        "proof_work_units": certified.work,
                    })
                );
            }
            Err(reason) => println!(
                "{}",
                json!({
                    "event": "early_rank_screen",
                    "arm": format!("early_rank_{mode}"),
                    "seed": seed,
                    "cell": {"team_id": team_id, "one_based_rank": rank_arg},
                    "mode": mode,
                    "roots": roots,
                    "work_limit": work_limit,
                    "status": "declined",
                    "reason": reason,
                    "constructor_ms": setup_ms,
                    "setup_work": setup_work,
                    "model_ms": model_ms,
                    "certificate_ms": certificate_ms,
                    "certificate": {"node_quota": 5000, "nodes": certified.nodes, "work": certified.work,
                        "lower": certified.lower, "upper": certified.upper},
                })
            ),
        }
    }
    Ok(())
}
