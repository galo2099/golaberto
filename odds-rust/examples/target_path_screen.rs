//! Offline diagnostic: screen target W/D/L paths with terminal bounds, then
//! test the surviving roots with the shared-fixture Domains relaxation.
//! Feasibility is only necessary, not proof that the rank event is reachable.
//! This example does not change runtime proof updates or production APIs.
use golaberto_odds::{
    domains::Domains,
    lookahead::RankGame,
    model::{Key, Model, Request},
    search::Cell,
    target_limits::{self, Certified},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    env, fs,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Counts {
    visited_leaves: usize,
    accepted_roots: usize,
    cap_hit: bool,
}

fn terminal_allowed(
    total: i32,
    base: &[i32],
    left: &[i32],
    win: i32,
    cell: Cell,
    certified: Certified,
) -> bool {
    let above = (0..base.len())
        .filter(|&team| team != cell.team && base[team] > total)
        .count();
    let below = (0..base.len())
        .filter(|&team| {
            team != cell.team
                && i64::from(base[team]) + i64::from(win) * i64::from(left[team]) < i64::from(total)
        })
        .count();
    above <= cell.rank && below <= base.len() - 1 - cell.rank && certified.allows(i64::from(total))
}

fn enumerate(
    step: usize,
    gain: usize,
    target: &[RankGame],
    terminal_suffix: &[Vec<bool>],
    cell: Cell,
    terminal_gain: &[bool],
    cap: usize,
    points: &mut Vec<i32>,
    path: &mut Vec<u8>,
    mass: f64,
    roots: &mut Vec<(Vec<i32>, Vec<u8>, f64)>,
    counts: &mut Counts,
) {
    if counts.cap_hit {
        return;
    }
    if !terminal_suffix[step][gain] {
        return;
    }
    if step == target.len() {
        if counts.visited_leaves >= cap {
            counts.cap_hit = true;
            return;
        }
        counts.visited_leaves += 1;
        if terminal_gain[gain] {
            roots.push((points.clone(), path.clone(), mass));
        }
        return;
    }
    let game = &target[step];
    for outcome in 0..3 {
        let probability = game.prob[outcome];
        if probability <= 0. {
            continue;
        }
        let own_gain = if game.home == cell.team {
            game.hg[outcome]
        } else {
            game.ag[outcome]
        };
        let next_gain = gain + own_gain as usize;
        if !terminal_suffix[step + 1][next_gain] {
            continue;
        }
        points[game.home] += game.hg[outcome];
        points[game.away] += game.ag[outcome];
        path.push(outcome as u8);
        enumerate(
            step + 1,
            next_gain,
            target,
            terminal_suffix,
            cell,
            terminal_gain,
            cap,
            points,
            path,
            mass * probability,
            roots,
            counts,
        );
        path.pop();
        points[game.home] -= game.hg[outcome];
        points[game.away] -= game.ag[outcome];
        if counts.cap_hit {
            break;
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 4 || args.len() > 5 {
        return Err("target_path_screen REQUEST.json TEAM_ID RANK [PATH_CAP]".into());
    }
    let model_started = Instant::now();
    let request: Request = serde_json::from_slice(&fs::read(&args[1])?)?;
    let model = Model::new(request)?;
    let model_elapsed = model_started.elapsed();
    let rules = &model.request.phase.championship;
    if model.keys.first() != Some(&Key::Pt)
        || model.request.phase.bonus_points != 0
        || (rules.point_win, rules.point_draw, rules.point_loss) != (3, 1, 0)
        || model.ids.len() != model.n
        || model.n > 32
        || model.fixtures.len() > 256
        || model.fixtures.iter().any(|game| game.home == game.away)
    {
        return Err("diagnostic supports LazyJoint packed points-first 3/1/0 models with at most 32 teams, 256 fixtures, and no self-fixtures".into());
    }
    let team_id: i32 = args[2].parse()?;
    let team = *model
        .indices
        .get(&team_id)
        .ok_or("TEAM_ID is not in the model")?;
    let rank_arg: usize = args[3].parse()?;
    if rank_arg == 0 || rank_arg > model.n {
        return Err("RANK must be a one-based rank within the group".into());
    }
    let cell = Cell {
        team,
        rank: rank_arg - 1,
    };
    let path_cap = args.get(4).map(|s| s.parse()).transpose()?.unwrap_or(4096);
    if path_cap == 0 {
        return Err("PATH_CAP must be positive".into());
    }

    let mut left = vec![0i32; model.n];
    for game in &model.fixtures {
        left[game.home] += 1;
        left[game.away] += 1;
    }
    let uses_wins = model.keys.get(1) == Some(&Key::W);
    let stride = if uses_wins {
        model
            .base
            .iter()
            .zip(&left)
            .map(|(campaign, &remaining)| campaign.wins + remaining)
            .max()
            .ok_or("empty team set")?
            + 1
    } else {
        1
    };
    if stride <= 0
        || left
            .iter()
            .any(|&remaining| (3 * stride + i32::from(uses_wins)) * remaining > 4096)
        || left[team] == 0
        || model.base.iter().any(|campaign| {
            (i64::from(campaign.points).abs() + 3 * model.fixtures.len() as i64 + 1)
                * i64::from(stride)
                + i64::from(campaign.wins)
                >= i64::from(i32::MAX / 4)
        })
    {
        return Err(
            "model exceeds LazyJoint packed-score limits or target has no remaining fixtures"
                .into(),
        );
    }
    let win = 3 * stride + i32::from(uses_wins);
    let base: Vec<_> = model
        .base
        .iter()
        .map(|campaign| campaign.points * stride + if uses_wins { campaign.wins } else { 0 })
        .collect();
    let all: Vec<_> = model
        .fixtures
        .iter()
        .enumerate()
        .map(|(index, game)| RankGame {
            index,
            home: game.home,
            away: game.away,
            prob: game.prob,
            hg: [0, stride, win],
            ag: [win, stride, 0],
        })
        .collect();
    let (target, remaining): (Vec<_>, Vec<_>) = all
        .into_iter()
        .partition(|game| game.home == team || game.away == team);

    let certification_started = Instant::now();
    let certified = target_limits::certify(&model, cell, 16000);
    let certification_elapsed = certification_started.elapsed();

    let max_gain = target.len() * win as usize;
    let terminal_gain: Vec<bool> = (0..=max_gain)
        .map(|gain| terminal_allowed(base[team] + gain as i32, &base, &left, win, cell, certified))
        .collect();
    let mut terminal_suffix = vec![vec![false; max_gain + 1]; target.len() + 1];
    for gain in 0..=max_gain {
        terminal_suffix[target.len()][gain] = terminal_gain[gain];
    }
    for step in (0..target.len()).rev() {
        let game = target[step];
        for gain in 0..=max_gain {
            for outcome in 0..3 {
                if game.prob[outcome] <= 0. {
                    continue;
                }
                let increment = if game.home == team {
                    game.hg[outcome]
                } else {
                    game.ag[outcome]
                } as usize;
                if gain + increment <= max_gain && terminal_suffix[step + 1][gain + increment] {
                    terminal_suffix[step][gain] = true;
                    break;
                }
            }
        }
    }

    let enumeration_started = Instant::now();
    let mut roots = Vec::new();
    let mut counts = Counts::default();
    let mut points = base.clone();
    // The DFS consults the terminal suffix table before descending. Keep the
    // table indexed by step and current target gain to prune unsafe prefixes.
    enumerate(
        0,
        0,
        &target,
        &terminal_suffix,
        cell,
        &terminal_gain,
        path_cap,
        &mut points,
        &mut Vec::new(),
        1.,
        &mut roots,
        &mut counts,
    );
    let enumeration_elapsed = enumeration_started.elapsed();

    let propagation_started = Instant::now();
    let rivals: Vec<_> = (0..model.n).filter(|&other| other != team).collect();
    let mut feasible = 0usize;
    let mut refuted = 0usize;
    let mut prior_feasible = 0.0;
    let mut prior_refuted = 0.0;
    let mut by_target: BTreeMap<i32, (usize, f64, usize, f64)> = BTreeMap::new();
    for (totals, _path, prior) in &roots {
        let target_total = totals[team];
        let result = Domains::propagate(&remaining, totals, &rivals, cell.rank, target_total);
        let entry = by_target.entry(target_total).or_default();
        if result.feasible {
            feasible += 1;
            prior_feasible += prior;
            entry.0 += 1;
            entry.1 += prior;
        } else {
            refuted += 1;
            prior_refuted += prior;
            entry.2 += 1;
            entry.3 += prior;
        }
    }
    let propagation_elapsed = propagation_started.elapsed();
    counts.accepted_roots = roots.len();

    let mut target_rows = Vec::new();
    for (total, (feasible_count, feasible_prior, refuted_count, refuted_prior)) in by_target {
        target_rows.push(json!({
            "packed_target_total": total,
            "target_points": total.div_euclid(stride),
            "target_wins": if uses_wins { Some(total.rem_euclid(stride)) } else { None },
            "feasible_roots": feasible_count,
            "feasible_raw_prior_sum": feasible_prior,
            "refuted_roots": refuted_count,
            "refuted_raw_prior_sum": refuted_prior,
        }));
    }
    let elapsed = |duration: Duration| duration.as_secs_f64() * 1000.0;
    let report: Value = json!({
        "diagnostic": "necessary_relaxation_only",
        "runtime_proof_updates": false,
        "model": {
            "team_id": team_id,
            "rank": rank_arg,
            "teams": model.n,
            "fixtures": model.fixtures.len(),
            "target_fixtures": target.len(),
            "remaining_fixtures": remaining.len(),
            "packed_wins_key": uses_wins,
            "stride": stride,
        },
        "certified_limits": {
            "lower": certified.lower,
            "upper": certified.upper,
            "nodes": certified.nodes,
            "work": certified.work,
        },
        "enumeration": {
            "complete": !counts.cap_hit,
            "path_cap": path_cap,
            "visited_target_leaves": counts.visited_leaves,
            "terminal_allowed_roots": counts.accepted_roots,
            "count_semantics": if counts.cap_hit { "lower_bound_truncated" } else { "complete_count" },
        },
        "shared_fixture_relaxation": {
            "feasible_roots": feasible,
            "refuted_roots": refuted,
            "feasible_raw_prior_sum": prior_feasible,
            "refuted_raw_prior_sum": prior_refuted,
            "by_target_points": target_rows,
        },
        "elapsed_ms": {
            "model": elapsed(model_elapsed),
            "certificate": elapsed(certification_elapsed),
            "enumeration": elapsed(enumeration_elapsed),
            "propagation": elapsed(propagation_elapsed),
        },
        "warning": "positive outcomes are selected by per-fixture probability > 0; accumulated f64 prior underflow is not treated as impossibility, and feasible relaxations do not establish reachability",
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
