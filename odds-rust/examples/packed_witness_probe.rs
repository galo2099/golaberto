use golaberto_odds::{
    conditioned::{canonical, canonical_ranks},
    goal_completion::{self, Certificate},
    model::{Model, Request},
    rank_proof::{Limits, Options, RankProof, WorkLimits},
    search::Cell,
};
use serde_json::{json, Value};
use std::{env, fs};

fn number<T: std::str::FromStr>(arg: Option<String>, default: T, name: &str) -> Result<T, String> {
    match arg {
        Some(value) => value
            .parse()
            .map_err(|_| format!("invalid {name}: {value}")),
        None => Ok(default),
    }
}

fn cert_json(model: &Model, cert: &Certificate) -> Value {
    json!({
        "outcomes": cert.outcomes,
        "adjustments": cert.adjustments.iter().map(|a| json!({
            "fixture": a.fixture, "margin": a.margin, "equal_goals": a.equal_goals
        })).collect::<Vec<_>>(),
        "ranks": cert.ranks,
        "canonical_outcomes": cert.outcomes.iter().map(|&o| canonical(o)).collect::<Vec<_>>(),
        "canonical_ranks_zero_based": canonical_ranks(model, &cert.outcomes),
        "native_score_support": goal_completion::has_native_score_support(model, cert),
    })
}

fn run() -> Result<Value, String> {
    let mut args = env::args().skip(1);
    let path = args.next().ok_or(
        "usage: packed_witness_probe REQUEST TEAM ONE_BASED_RANK [MAX_COHORTS=100000] [NODES=1000] [MAX_WORK]",
    )?;
    let team_id: i32 = number(args.next(), 0, "TEAM")?;
    let one_rank: usize = number(args.next(), 0, "ONE_BASED_RANK")?;
    let max_cohorts: usize = number(args.next(), 100_000, "MAX_COHORTS")?;
    let node_budget: usize = number(args.next(), 1_000, "NODES")?;
    let max_work: Option<usize> = args
        .next()
        .map(|v| v.parse().map_err(|_| format!("invalid MAX_WORK: {v}")))
        .transpose()?;
    if args.next().is_some() || one_rank == 0 {
        return Err("usage: packed_witness_probe REQUEST TEAM ONE_BASED_RANK [MAX_COHORTS=100000] [NODES=1000] [MAX_WORK]".into());
    }
    let request: Request = serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let model = Model::new(request)?;
    if model.fixtures.iter().any(|f| {
        !f.home_sampler.mean.is_finite()
            || f.home_sampler.mean <= 0.
            || !f.away_sampler.mean.is_finite()
            || f.away_sampler.mean <= 0.
    }) {
        return Err("future fixtures must have positive finite Poisson means".into());
    }
    let team = *model
        .indices
        .get(&team_id)
        .ok_or("TEAM is absent from request")?;
    let rank = one_rank - 1;
    if rank >= model.n {
        return Err("rank is outside the group".into());
    }
    let cell = Cell { team, rank };
    let proof = RankProof::new(&model).ok_or("rank proof unavailable for this model")?;
    let opposite = proof.negated();
    let use_opposite = rank < model.n / 2;
    let (preferred, pref_required, alternate, alt_required, pref_name, alt_name) = if use_opposite {
        (
            &opposite,
            model.n - 1 - rank,
            &proof,
            rank,
            "negated",
            "primary",
        )
    } else {
        (
            &proof,
            rank,
            &opposite,
            model.n - 1 - rank,
            "primary",
            "negated",
        )
    };
    let limits = Limits {
        max_cohorts,
        max_depth: 512,
    };
    let mut attempts = Vec::new();
    let mut total_nodes = 0usize;
    let mut total_work = 0usize;
    let mut witness: Option<Certificate> = None;
    let mut impossible = false;
    for (direction, p, required) in [
        (pref_name, preferred, pref_required),
        (alt_name, alternate, alt_required),
    ] {
        let remaining = node_budget.saturating_sub(total_nodes);
        let remaining_work = max_work.map(|cap| cap.saturating_sub(total_work));
        if remaining == 0
            || remaining_work == Some(0)
            || (attempts.len() > 0 && !attempts.iter().any(|a: &Value| a["result"] == "unknown"))
        {
            break;
        }
        let found = if let Some(work) = remaining_work {
            p.feasible_domains_with_work_limits(
                cell,
                required,
                remaining,
                Options::default(),
                WorkLimits {
                    max_work: work,
                    max_depth: 512,
                },
            )
        } else {
            p.feasible_domains_with_limits(cell, required, remaining, Options::default(), limits)
        };
        let search_stats = serde_json::to_value(&found.stats).unwrap_or(Value::Null);
        total_nodes += found.stats.nodes;
        total_work += found.stats.cohort_work;
        let cohorts = found.stats.cohorts;
        if found.impossible {
            attempts.push(json!({"direction": direction, "nodes": found.stats.nodes, "cohorts": cohorts, "search_stats": search_stats, "domains_present": false, "completion_ok": false, "result": "impossible"}));
            impossible = true;
            break;
        }
        let Some(domains) = found.domains.as_deref() else {
            attempts.push(json!({"direction": direction, "nodes": found.stats.nodes, "cohorts": cohorts, "search_stats": search_stats, "domains_present": false, "completion_ok": false, "result": "unknown"}));
            continue;
        };
        if domains.len() != model.fixtures.len() {
            attempts.push(json!({"direction": direction, "nodes": found.stats.nodes, "cohorts": cohorts, "search_stats": search_stats, "domains_present": false, "completion_ok": false, "result": "unknown"}));
            continue;
        }
        let mut outcomes = Vec::with_capacity(domains.len());
        let mut supported = true;
        for (fixture, &domain) in model.fixtures.iter().zip(domains) {
            let selected = (0..3)
                .filter(|o| domain & (1 << o) != 0)
                .map(|o| (o, fixture.prob[o]))
                .filter(|(_, p)| p.is_finite() && *p > 0.)
                .fold(None, |best: Option<(usize, f64)>, candidate| {
                    if best.is_none_or(|(_, probability)| candidate.1 > probability) {
                        Some(candidate)
                    } else {
                        best
                    }
                })
                .map(|(o, _)| o as u8);
            if let Some(o) = selected {
                outcomes.push(o);
            } else {
                supported = false;
                break;
            }
        }
        let cert = if supported {
            goal_completion::complete_with_paths(&model, &outcomes, cell, true)
                .filter(|c| goal_completion::verify(&model, c, cell))
                .or_else(|| {
                    goal_completion::complete_with_supported_margins(&model, &outcomes, cell)
                        .filter(|c| goal_completion::verify(&model, c, cell))
                })
        } else {
            None
        };
        let completion_ok = cert.is_some();
        let ranks = cert.as_ref().map(|c| canonical_ranks(&model, &c.outcomes));
        attempts.push(json!({"direction": direction, "nodes": found.stats.nodes, "cohorts": cohorts, "search_stats": search_stats, "domains_present": true, "completion_ok": completion_ok, "canonical_rank": ranks.as_ref().map(|r| r[team]), "result": if completion_ok {"witness"} else {"unknown"}}));
        if cert.is_some() {
            witness = cert;
            break;
        }
    }
    let status = if witness.is_some() {
        "witness"
    } else if impossible {
        "impossible"
    } else {
        "unknown"
    };
    Ok(json!({
        "request": path, "team": team_id, "one_based_rank": one_rank,
        "limits": {"max_cohorts": if max_work.is_none() {Some(max_cohorts)} else {None}, "nodes": node_budget, "max_work": max_work, "max_depth": 512},
        "attempts": attempts, "status": status,
        "certificate": witness.as_ref().map(|c| cert_json(&model, c)),
    }))
}

fn main() {
    match run() {
        Ok(value) => println!("{}", value),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}
