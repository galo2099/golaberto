use crate::{
    logging::{cell_counts, RequestLog},
    model::{Model, Request},
    pool::{self, Estimate},
    rng::{derive, Rng},
};
use serde::Serialize;
use serde_json::json;
use std::{collections::BTreeMap, time::Instant};
#[derive(Serialize)]
pub struct Odds {
    #[serde(rename = "Pos")]
    pub pos: Vec<f64>,
}
#[derive(Serialize)]
pub struct Response {
    pub team_odds: BTreeMap<i32, Odds>,
    pub game_importance: BTreeMap<i32, [Option<f64>; 2]>,
    pub rare_position_estimates: BTreeMap<i32, BTreeMap<usize, Estimate>>,
}
#[derive(Serialize)]
pub struct Timings {
    pub setup_ms: f64,
    pub scout_ms: f64,
    pub pool_ms: f64,
    pub search_ms: f64,
    pub total_ms: f64,
}
fn entropy(v: &[f64]) -> f64 {
    v.iter().filter(|p| **p > 0.).map(|p| -p * p.log2()).sum()
}
fn importance(counts: &[u32], n: usize, zones: &[Vec<usize>], samples: usize) -> f64 {
    let mut average = vec![0.; zones.len() + 1];
    let mut divergence = 0.;
    for bin in 0..100 {
        let row = &counts[bin * n..(bin + 1) * n];
        let total: u32 = row.iter().sum();
        if total == 0 {
            continue;
        }
        let mut probs = vec![0.; zones.len() + 1];
        for (r, &count) in row.iter().enumerate() {
            let mut found = false;
            for (z, positions) in zones.iter().enumerate() {
                for p in positions {
                    if r + 1 == *p {
                        probs[z] += count as f64 / total as f64;
                        found = true;
                    }
                }
            }
            if !found {
                probs[zones.len()] += count as f64 / total as f64;
            }
        }
        let weight = total as f64 / samples as f64;
        divergence -= weight * entropy(&probs);
        for (mean, p) in average.iter_mut().zip(probs) {
            *mean += weight * p;
        }
    }
    let h = entropy(&average);
    divergence += h;
    if divergence <= 0. || average.iter().filter(|p| **p > 0.).count() == 1 {
        0.
    } else {
        (divergence / h.max(1.)).sqrt()
    }
}
pub fn initial_scout(model: &Model, samples: usize, seed: i64) -> BTreeMap<i32, [Option<f64>; 2]> {
    let n = model.n;
    let mut counts = vec![0u32; model.fixtures.len() * 2 * 100 * n];
    let mut c = model.base.clone();
    let mut scores = model.empty_scores();
    let mut order = vec![0; n];
    let mut ranks = vec![0; n];
    let mut rng = Rng::new(derive(seed, "pipeline-scout"));
    for _ in 0..samples {
        c.copy_from_slice(&model.base);
        for (i, f) in model.fixtures.iter().enumerate() {
            let s = [
                f.home_sampler.sample(&mut rng),
                f.away_sampler.sample(&mut rng),
            ];
            scores[f.request_index] = s;
            model.add(&mut c, i, s);
        }
        model.standings(&mut order, &c, &scores, &mut rng);
        for (r, &t) in order.iter().enumerate() {
            ranks[t] = r;
        }
        for (i, f) in model.fixtures.iter().enumerate() {
            let s = scores[f.request_index];
            let bin = (s[0].min(9) * 10 + s[1].min(9)) as usize;
            if f.home < n {
                counts[((i * 2) * 100 + bin) * n + ranks[f.home]] += 1;
            }
            if f.away < n {
                counts[((i * 2 + 1) * 100 + bin) * n + ranks[f.away]] += 1;
            }
        }
    }
    let zones: Vec<_> = model
        .request
        .zones
        .iter()
        .map(|z| z.position.clone())
        .collect();
    let mut result = BTreeMap::new();
    for (i, f) in model.fixtures.iter().enumerate() {
        let mut sides = [None; 2];
        for (s, t) in [f.home, f.away].into_iter().enumerate() {
            if t < n {
                let start = (i * 2 + s) * 100 * n;
                sides[s] = Some(importance(
                    &counts[start..start + 100 * n],
                    n,
                    &zones,
                    samples,
                ));
            }
        }
        if sides != [None, None] {
            result.insert(model.request.games[f.request_index].id, sides);
        }
    }
    result
}
pub fn calculate(
    request: Request,
    seed: i64,
    workers: usize,
    scout_samples: usize,
) -> Result<(Response, Timings), String> {
    let log = RequestLog::new().context(request.id, seed);
    calculate_logged(request, seed, workers, scout_samples, &log)
}

pub fn calculate_logged(
    request: Request,
    seed: i64,
    workers: usize,
    scout_samples: usize,
    log: &RequestLog,
) -> Result<(Response, Timings), String> {
    let scoped_log = log.calculating();
    let log = &scoped_log;
    log.event(
        "rust_odds_start",
        json!({"teams":request.team_groups.len(),
        "games":request.games.len(),"workers":workers,"scout_samples":scout_samples,
        "pool_samples":100000,"sort":request.phase.sort,
        "rare_tail_profile":crate::rare_tail::profile(),
        "rare_tail_quality":crate::rare_tail::value("RUST_ODDS_RARE_TAIL_QUALITY"),
        "rare_tail_extension":crate::rare_tail::value("RUST_ODDS_RARE_TAIL_EXTENSION"),
        "rare_tail_confirm_more":crate::rare_tail::value("RUST_ODDS_RARE_TAIL_CONFIRM_MORE"),
        "rare_tail_shared_guides":crate::rare_tail::value("RUST_ODDS_RARE_TAIL_SHARE")=="1",
        "shared_constraints":crate::rare_tail::value("RUST_ODDS_SHARED_CONSTRAINTS"),
        "shared_constraints_blockers":crate::rare_tail::value("RUST_ODDS_SHARED_CONSTRAINTS_BLOCKERS"),
        "shared_constraints_relative":crate::rare_tail::value("RUST_ODDS_SHARED_CONSTRAINTS_RELATIVE")=="1",
        "shared_constraints_confirmation":crate::rare_tail::value("RUST_ODDS_SHARED_CONSTRAINTS_CONFIRMATION"),
        "shared_constraints_fraction":crate::rare_tail::value("RUST_ODDS_SHARED_CONSTRAINTS_FRACTION"),
        "joint_cap_conditioning":crate::search::enabled("RUST_ODDS_JOINT_CAP_CONDITIONING"),
        "joint_cap_broad":crate::search::enabled("RUST_ODDS_JOINT_CAP_CONDITIONING")
            && crate::search::enabled("RUST_ODDS_JOINT_CAP_BROAD"),
        "joint_propagation":crate::search::enabled("RUST_ODDS_JOINT_CAP_CONDITIONING")
            && crate::search::enabled("RUST_ODDS_JOINT_CAP_BROAD")
            && crate::search::enabled("RUST_ODDS_JOINT_PROPAGATION"),
        "joint_allocation":std::env::var("RUST_ODDS_JOINT_ALLOCATION")
            .unwrap_or_else(|_| "transfer".into())}),
    );
    if workers == 0 || workers > 4 || scout_samples == 0 {
        return Err("workers must be 1..4 and scout samples positive".into());
    }
    let start = Instant::now();
    let model = Model::new(request)?;
    let setup_ms = start.elapsed().as_secs_f64() * 1000.;
    log.stage(
        "setup",
        start,
        json!({"remaining_fixtures":model.fixtures.len()}),
    );
    let phase = Instant::now();
    let game_importance = initial_scout(&model, scout_samples, seed);
    let scout_ms = phase.elapsed().as_secs_f64() * 1000.;
    log.stage(
        "scout",
        phase,
        json!({"samples":scout_samples,"stream_seed":derive(seed,"pipeline-scout")}),
    );
    let phase = Instant::now();
    let mut estimates = pool::production_logged(&model, seed, workers, Some(log));
    let pool_ms = phase.elapsed().as_secs_f64() * 1000.;
    log.stage("pool", phase, json!({"cells":cell_counts(&estimates)}));
    let phase = Instant::now();
    let work = crate::search::run_logged(&model, seed, workers, &mut estimates, Some(log));
    for e in &mut estimates {
        e.work_spent += work;
    }
    let search_ms = phase.elapsed().as_secs_f64() * 1000.;
    log.stage(
        "search",
        phase,
        json!({"work":work,"cells":cell_counts(&estimates)}),
    );
    let phase = Instant::now();
    let mut team_odds = BTreeMap::new();
    let mut rare_position_estimates = BTreeMap::new();
    for t in 0..model.n {
        let row = estimates[t * model.n..(t + 1) * model.n].to_vec();
        team_odds.insert(
            model.ids[t],
            Odds {
                pos: row.iter().map(|e| 100. * e.probability).collect(),
            },
        );
        rare_position_estimates.insert(
            model.ids[t],
            row.into_iter()
                .enumerate()
                .map(|(rank, mut estimate)| {
                    estimate.reachability = public_reachability(&estimate).into();
                    (rank, estimate)
                })
                .collect(),
        );
    }
    let total_ms = start.elapsed().as_secs_f64() * 1000.;
    log.stage("response", phase, json!({}));
    let timings = Timings {
        setup_ms,
        scout_ms,
        pool_ms,
        search_ms,
        total_ms,
    };
    log.event(
        "rust_odds_complete",
        json!({"timings":timings,"cells":cell_counts(&estimates),
        "work_spent":estimates.first().map(|e|e.work_spent).unwrap_or(0)}),
    );
    Ok((
        Response {
            team_odds,
            game_importance,
            rare_position_estimates,
        },
        timings,
    ))
}

// Normalize only response copies; search and diagnostic labels stay detailed.
fn public_reachability(estimate: &Estimate) -> &'static str {
    if estimate.reachability.starts_with("impossible") {
        "impossible"
    } else if estimate.probability > 0.
        || matches!(
            estimate.reachability.as_str(),
            "witness" | "reachable_by_construction"
        )
    {
        "reachable"
    } else {
        "undecided"
    }
}

#[cfg(test)]
mod response_tests {
    use super::*;

    #[test]
    fn public_status_has_three_values_including_unobserved_and_plain_mc_cells() {
        for (label, probability, expected) in [
            ("impossible_by_points", 0., "impossible"),
            ("impossible_by_joint_points", 0., "impossible"),
            ("impossible_by_joint_rank", 0., "impossible"),
            ("reachable_by_construction", 0., "reachable"),
            ("witness", 0., "reachable"),
            ("undecided", 0., "undecided"),
            ("", 0., "undecided"),
            ("", 0.25, "reachable"),
        ] {
            let estimate = Estimate {
                reachability: label.into(),
                probability,
                ..Estimate::default()
            };
            assert_eq!(public_reachability(&estimate), expected);
            assert_eq!(estimate.reachability, label);
        }
    }
}
