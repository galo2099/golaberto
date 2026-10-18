//! Snapshot-specific diagnostic for group 16498 on 2026-10-18, when
//! Chapecoense (team 318) had 18 points and 3 wins. It examines a ninth-place
//! packed-points/WDL relaxation; it does not change production behavior.
use golaberto_odds::{
    domains::Domains,
    lookahead::RankGame,
    model::{Key, Model, Request},
    rng::{derive, Rng},
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap},
    env, fs,
    time::Instant,
};

const TARGET_ID: i32 = 318;
const TARGET_RANK: usize = 9;
const STRIDE: i32 = 28;
const TARGET_PACKED: i32 = 42 * STRIDE + 11;
const CAPPED_IDS: [i32; 11] = [7, 225, 24, 10, 79, 20, 584, 110, 14, 18, 67];
const SHARED_NODE_QUOTA: usize = 20_000_000;
const SHARED_LEAF_QUOTA: usize = 2_000_000;

#[path = "restricted_rank_analysis/conditioned.rs"]
mod conditioned;
#[path = "restricted_rank_analysis/gd.rs"]
mod gd;

#[derive(Clone)]
struct SharedEdge {
    a: usize,
    b: usize,
    mask: u8,
    probability: [f64; 3],
    gain_a: [i32; 3],
    gain_b: [i32; 3],
}

struct CountResult {
    nodes: usize,
    weighted_mass: Option<f64>,
}

#[derive(Clone)]
struct WeightedLeaf {
    packed_outcomes: u64,
    remaining: Vec<i32>,
    weight: f64,
}

struct EnumerationResult {
    nodes: usize,
    leaves: Option<Vec<WeightedLeaf>>,
    mass: Option<f64>,
}

struct Counter<'a> {
    edges: &'a [SharedEdge],
    unary_cdf: &'a [Vec<f64>],
    remaining: Vec<i32>,
    nodes: usize,
    quota: usize,
    exhausted: bool,
    weighted_mass: f64,
    collect_leaves: bool,
    leaf_quota: usize,
    leaves: Vec<WeightedLeaf>,
    packed_outcomes: u64,
}

impl Counter<'_> {
    fn visit(&mut self, edge_index: usize, weight: f64) {
        if self.exhausted {
            return;
        }
        self.nodes += 1;
        if self.nodes > self.quota {
            self.exhausted = true;
            return;
        }
        for team in 0..self.remaining.len() {
            let remaining = self.remaining[team];
            if remaining < 0
                || self.unary_cdf[team]
                    .get(remaining as usize)
                    .copied()
                    .unwrap_or(0.0)
                    == 0.0
            {
                return;
            }
        }
        if edge_index == self.edges.len() {
            let mut mass = weight;
            for team in 0..self.remaining.len() {
                mass *= self.unary_cdf[team][self.remaining[team] as usize];
            }
            self.weighted_mass += mass;
            if self.collect_leaves && mass > 0.0 {
                if self.leaves.len() >= self.leaf_quota {
                    self.exhausted = true;
                    return;
                }
                self.leaves.push(WeightedLeaf {
                    packed_outcomes: self.packed_outcomes,
                    remaining: self.remaining.clone(),
                    weight: mass,
                });
            }
            return;
        }

        let edge = &self.edges[edge_index];
        for outcome in 0..3 {
            if edge.mask & (1 << outcome) == 0 {
                continue;
            }
            let gain_a = edge.gain_a[outcome];
            let gain_b = edge.gain_b[outcome];
            if gain_a > self.remaining[edge.a] || gain_b > self.remaining[edge.b] {
                continue;
            }
            self.remaining[edge.a] -= gain_a;
            self.remaining[edge.b] -= gain_b;
            self.packed_outcomes |= (outcome as u64) << (2 * edge_index);
            self.visit(edge_index + 1, weight * edge.probability[outcome]);
            self.packed_outcomes &= !(3u64 << (2 * edge_index));
            self.remaining[edge.a] += gain_a;
            self.remaining[edge.b] += gain_b;
            if self.exhausted {
                return;
            }
        }
    }
}

fn count_cap_mass(
    edges: &[SharedEdge],
    unary_pmfs: &[BTreeMap<i32, f64>],
    base_fixed: &[i32],
    packed_cap: i32,
    quota: usize,
) -> CountResult {
    let unary_cdf = unary_pmfs
        .iter()
        .zip(base_fixed)
        .map(|(pmf, &base)| {
            let maximum_gain = packed_cap - base;
            if maximum_gain < 0 {
                return vec![0.0];
            }
            let mut cdf = vec![0.0; maximum_gain as usize + 1];
            for (&score, &mass) in pmf {
                let gain = score - base;
                if gain >= 0 && gain <= maximum_gain {
                    cdf[gain as usize] += mass;
                }
            }
            for index in 1..cdf.len() {
                cdf[index] += cdf[index - 1];
            }
            cdf
        })
        .collect::<Vec<_>>();
    let mut counter = Counter {
        edges,
        unary_cdf: &unary_cdf,
        remaining: base_fixed.iter().map(|base| packed_cap - base).collect(),
        nodes: 0,
        quota,
        exhausted: false,
        weighted_mass: 0.0,
        collect_leaves: false,
        leaf_quota: 0,
        leaves: Vec::new(),
        packed_outcomes: 0,
    };
    counter.visit(0, 1.0);
    CountResult {
        nodes: counter.nodes,
        weighted_mass: (!counter.exhausted).then_some(counter.weighted_mass),
    }
}

fn enumerate_cap_leaves(
    edges: &[SharedEdge],
    unary_pmfs: &[BTreeMap<i32, f64>],
    base_fixed: &[i32],
    packed_cap: i32,
    node_quota: usize,
    leaf_quota: usize,
) -> EnumerationResult {
    let unary_cdf = unary_pmfs
        .iter()
        .zip(base_fixed)
        .map(|(pmf, &base)| {
            let maximum_gain = packed_cap - base;
            if maximum_gain < 0 {
                return vec![0.0];
            }
            let mut cdf = vec![0.0; maximum_gain as usize + 1];
            for (&score, &mass) in pmf {
                let gain = score - base;
                if gain >= 0 && gain <= maximum_gain {
                    cdf[gain as usize] += mass;
                }
            }
            for index in 1..cdf.len() {
                cdf[index] += cdf[index - 1];
            }
            cdf
        })
        .collect::<Vec<_>>();
    let mut counter = Counter {
        edges,
        unary_cdf: &unary_cdf,
        remaining: base_fixed.iter().map(|base| packed_cap - base).collect(),
        nodes: 0,
        quota: node_quota,
        exhausted: false,
        weighted_mass: 0.0,
        collect_leaves: true,
        leaf_quota,
        leaves: Vec::new(),
        packed_outcomes: 0,
    };
    counter.visit(0, 1.0);
    let complete = !counter.exhausted;
    EnumerationResult {
        nodes: counter.nodes,
        mass: complete.then_some(counter.weighted_mass),
        leaves: complete.then_some(counter.leaves),
    }
}

fn absolute_mass(target_path_mass: f64, cap_mass: Option<f64>) -> Option<f64> {
    cap_mass.map(|mass| target_path_mass * mass)
}

fn score_gain(outcome: usize, home: bool) -> i32 {
    match (outcome, home) {
        (0, true) | (2, false) => 0,
        (1, _) => STRIDE,
        (2, true) | (0, false) => 3 * STRIDE + 1,
        _ => unreachable!(),
    }
}

fn outcome_name(outcome: usize) -> &'static str {
    ["home_loss", "draw", "home_win"][outcome]
}

fn game_domains(mask: u8, probability: [f64; 3]) -> Vec<Value> {
    (0..3)
        .filter(|outcome| mask & (1 << outcome) != 0)
        .map(|outcome| {
            json!({
                "outcome": outcome_name(outcome),
                "probability": probability[outcome]
            })
        })
        .collect()
}

fn min_fill_order(ids: &[i32], edges: &[(usize, usize)]) -> (Vec<i32>, usize) {
    let mut adjacent = vec![vec![false; ids.len()]; ids.len()];
    for &(a, b) in edges {
        adjacent[a][b] = true;
        adjacent[b][a] = true;
    }
    let mut alive = vec![true; ids.len()];
    let mut order = Vec::with_capacity(ids.len());
    let mut width = 0;
    for _ in 0..ids.len() {
        let pick = (0..ids.len())
            .filter(|&v| alive[v])
            .min_by_key(|&v| {
                let neighbors = (0..ids.len())
                    .filter(|&u| alive[u] && adjacent[v][u])
                    .collect::<Vec<_>>();
                let mut missing = 0usize;
                for i in 0..neighbors.len() {
                    for j in i + 1..neighbors.len() {
                        if !adjacent[neighbors[i]][neighbors[j]] {
                            missing += 1;
                        }
                    }
                }
                (missing, neighbors.len(), ids[v])
            })
            .unwrap();
        let neighbors = (0..ids.len())
            .filter(|&u| alive[u] && adjacent[pick][u])
            .collect::<Vec<_>>();
        width = width.max(neighbors.len());
        for i in 0..neighbors.len() {
            for j in i + 1..neighbors.len() {
                adjacent[neighbors[i]][neighbors[j]] = true;
                adjacent[neighbors[j]][neighbors[i]] = true;
            }
        }
        alive[pick] = false;
        order.push(ids[pick]);
    }
    (order, width)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().collect::<Vec<_>>();
    if args.len() < 2 {
        return Err("restricted_rank_analysis REQUEST.json [--gd-samples N --seed S]".into());
    }
    let mut gd_samples = None;
    let mut seed = 808i64;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--gd-samples" if index + 1 < args.len() => {
                let n = args[index + 1].parse::<usize>()?;
                if !(2..=1_000_000).contains(&n) {
                    return Err("--gd-samples must be in 2..=1000000".into());
                }
                gd_samples = Some(n);
                index += 2;
            }
            "--seed" if index + 1 < args.len() => {
                seed = args[index + 1].parse::<i64>()?;
                index += 2;
            }
            _ => return Err("expected --gd-samples N and/or --seed S".into()),
        }
    }
    let report = analyze(&args[1], gd_samples.map(|n| (n, seed)))?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}

fn analyze(
    path: &str,
    gd_options: Option<(usize, i64)>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let request: Request = serde_json::from_slice(&fs::read(path)?)?;
    let model = Model::new(request)?;
    if model.request.id != 16498 {
        return Err("diagnostic is specific to group 16498".into());
    }
    let target = model.indices[&TARGET_ID];
    if model.keys.first() != Some(&Key::Pt)
        || model.keys.get(1) != Some(&Key::W)
        || model.keys.get(2) != Some(&Key::Gd)
        || model.keys.get(3) != Some(&Key::Gf)
        || model.n != 20
        || model.request.phase.bonus_points != 0
        || (
            model.request.phase.championship.point_win,
            model.request.phase.championship.point_draw,
            model.request.phase.championship.point_loss,
        ) != (3, 1, 0)
    {
        return Err("diagnostic requires this 20-team points-then-wins 3/1/0 phase".into());
    }
    if (model.base[target].points, model.base[target].wins) != (18, 3) {
        return Err(
            "snapshot does not match Chapecoense's expected 18-point, 3-win campaign".into(),
        );
    }

    let cap_indices = CAPPED_IDS
        .iter()
        .map(|id| model.indices[id])
        .collect::<Vec<_>>();
    let cap_lookup = cap_indices
        .iter()
        .enumerate()
        .map(|(local, &team)| (team, local))
        .collect::<HashMap<_, _>>();
    let base = model
        .base
        .iter()
        .map(|campaign| campaign.points * STRIDE + campaign.wins)
        .collect::<Vec<_>>();
    let mut lower = vec![i32::MIN / 4; model.n];
    let mut upper = vec![i32::MAX / 4; model.n];
    lower[target] = TARGET_PACKED;
    upper[target] = TARGET_PACKED;
    for &team in &cap_indices {
        upper[team] = TARGET_PACKED;
    }

    let games = model
        .fixtures
        .iter()
        .enumerate()
        .map(|(index, fixture)| RankGame {
            index,
            home: fixture.home,
            away: fixture.away,
            prob: fixture.prob,
            hg: [0, STRIDE, 3 * STRIDE + 1],
            ag: [3 * STRIDE + 1, STRIDE, 0],
        })
        .collect::<Vec<_>>();
    let mut seed_masks = model
        .fixtures
        .iter()
        .map(|fixture| {
            (0..3).fold(0u8, |mask, outcome| {
                if fixture.prob[outcome] > 0.0 {
                    mask | (1 << outcome)
                } else {
                    mask
                }
            })
        })
        .collect::<Vec<_>>();
    let mut target_path_mass = 1.0;
    let mut target_games = Vec::new();
    for (index, fixture) in model.fixtures.iter().enumerate() {
        if fixture.home == target || fixture.away == target {
            let outcome = if fixture.home == target { 2 } else { 0 };
            target_path_mass *= fixture.prob[outcome];
            seed_masks[index] &= 1 << outcome;
            target_games.push(index);
        }
    }
    if target_games.len() != 8 || target_path_mass <= 0.0 {
        return Err("snapshot does not have eight positive-mass target fixtures".into());
    }

    let propagated = Domains::condition(
        &games,
        &base,
        &[],
        0,
        TARGET_PACKED,
        &lower,
        &upper,
        &seed_masks,
    );
    if !propagated.feasible {
        return Err("necessary packed-point constraints propagated infeasible".into());
    }

    let cap_ids = CAPPED_IDS.to_vec();
    let mut shared_edges = Vec::new();
    let mut shared_games = Vec::new();
    let mut unary_games = vec![Vec::<usize>::new(); cap_indices.len()];
    let mut irrelevant_games = Vec::new();
    let mut target_fixed_games = Vec::new();
    let mut domain_counts = [0usize; 4];
    let mut domain_mass_product = 1.0;
    let mut relevant_rows = Vec::new();
    let mut fixed_cap_gain = vec![0i32; cap_indices.len()];

    for (index, fixture) in model.fixtures.iter().enumerate() {
        let mask = propagated.domains[index];
        let count = mask.count_ones() as usize;
        domain_counts[count] += 1;
        let allowed_mass = (0..3)
            .filter(|&outcome| mask & (1 << outcome) != 0)
            .map(|outcome| fixture.prob[outcome])
            .sum::<f64>();
        domain_mass_product *= allowed_mass;
        let game = &model.request.games[fixture.request_index];
        let target_fixture = fixture.home == target || fixture.away == target;
        if target_fixture {
            target_fixed_games.push(index);
            let constrained = if fixture.home == target {
                cap_lookup
                    .get(&fixture.away)
                    .map(|&local| (fixture.away, false, local))
            } else {
                cap_lookup
                    .get(&fixture.home)
                    .map(|&local| (fixture.home, true, local))
            };
            if let Some((_, constrained_is_home, local)) = constrained {
                let outcome = if fixture.home == target { 2 } else { 0 };
                fixed_cap_gain[local] += score_gain(outcome, constrained_is_home);
            }
            continue;
        }

        let home_cap = cap_lookup.get(&fixture.home).copied();
        let away_cap = cap_lookup.get(&fixture.away).copied();
        match (home_cap, away_cap) {
            (None, None) => irrelevant_games.push(index),
            (Some(local), None) | (None, Some(local)) => unary_games[local].push(index),
            (Some(home_local), Some(away_local)) => {
                shared_edges.push((home_local, away_local));
                shared_games.push(index);
            }
        }
        if home_cap.is_some() || away_cap.is_some() {
            relevant_rows.push(json!({
                "game_id":game.id,
                "home_id":game.home_id,
                "away_id":game.away_id,
                "kind":match (home_cap.is_some(),away_cap.is_some()) {
                    (true,true) => "shared_cap_edge",
                    (true,false)|(false,true) => "one_cap_endpoint",
                    (false,false) => unreachable!(),
                },
                "domain_mask":mask,
                "domain_count":count,
                "allowed_mass":allowed_mass,
                "outcomes":game_domains(mask,fixture.prob)
            }));
        }
    }

    let mut pmfs = Vec::new();
    let mut unary_distributions = Vec::new();
    for (local, &team) in cap_indices.iter().enumerate() {
        let mut distribution = BTreeMap::from([(base[team] + fixed_cap_gain[local], 1.0)]);
        for &game_index in &unary_games[local] {
            let fixture = &model.fixtures[game_index];
            let mut next = BTreeMap::<i32, f64>::new();
            for (&score, &mass) in &distribution {
                for outcome in 0..3 {
                    if propagated.domains[game_index] & (1 << outcome) == 0 {
                        continue;
                    }
                    let own_is_home = fixture.home == team;
                    *next
                        .entry(score + score_gain(outcome, own_is_home))
                        .or_default() += mass * fixture.prob[outcome];
                }
            }
            distribution = next;
        }
        let total_mass = distribution.values().sum::<f64>();
        let within_cap = distribution
            .iter()
            .filter(|(score, _)| **score <= TARGET_PACKED)
            .map(|(_, mass)| *mass)
            .sum::<f64>();
        let strict_below = distribution
            .iter()
            .filter(|(score, _)| **score < TARGET_PACKED)
            .map(|(_, mass)| *mass)
            .sum::<f64>();
        let exactly_tied = distribution.get(&TARGET_PACKED).copied().unwrap_or(0.0);
        unary_distributions.push(distribution.clone());
        let support = distribution
            .iter()
            .map(|(score, mass)| json!({"packed_score":score,"probability_mass":mass}))
            .collect::<Vec<_>>();
        pmfs.push(json!({
            "team_id":model.ids[team],
            "role":"upper_capped_rival",
            "base_packed_score":base[team],
            "fixed_target_match_gain":fixed_cap_gain[local],
            "unary_game_count":unary_games[local].len(),
            "unary_game_ids":unary_games[local].iter().map(|&i| model.request.games[model.fixtures[i].request_index].id).collect::<Vec<_>>(),
            "pmf_support_count":distribution.len(),
            "pmf_total_mass":total_mass,
            "pmf_mass_le_target":within_cap,
            "pmf_mass_strictly_below_target":strict_below,
            "pmf_mass_tied_at_target":exactly_tied,
            "support":support
        }));
    }

    let (min_fill_ids, induced_width) = min_fill_order(&cap_ids, &shared_edges);
    let shared_edges_for_count = shared_games
        .iter()
        .map(|&index| {
            let fixture = &model.fixtures[index];
            let home_local = cap_lookup[&fixture.home];
            let away_local = cap_lookup[&fixture.away];
            let mut edge = SharedEdge {
                a: home_local,
                b: away_local,
                mask: propagated.domains[index],
                probability: fixture.prob,
                gain_a: [0; 3],
                gain_b: [0; 3],
            };
            for outcome in 0..3 {
                edge.gain_a[outcome] = score_gain(outcome, true);
                edge.gain_b[outcome] = score_gain(outcome, false);
            }
            edge
        })
        .collect::<Vec<_>>();
    let base_fixed = cap_indices
        .iter()
        .enumerate()
        .map(|(local, &team)| base[team] + fixed_cap_gain[local])
        .collect::<Vec<_>>();
    let upper_cap_result = count_cap_mass(
        &shared_edges_for_count,
        &unary_distributions,
        &base_fixed,
        TARGET_PACKED,
        SHARED_NODE_QUOTA,
    );
    let strict_cap_result = count_cap_mass(
        &shared_edges_for_count,
        &unary_distributions,
        &base_fixed,
        TARGET_PACKED - 1,
        SHARED_NODE_QUOTA,
    );

    let edges = shared_games
        .iter()
        .map(|&index| {
            let fixture = &model.fixtures[index];
            let game = &model.request.games[fixture.request_index];
            json!({
                "game_id":game.id,
                "home_id":game.home_id,
                "away_id":game.away_id,
                "domain_mask":propagated.domains[index],
                "outcomes":game_domains(propagated.domains[index],fixture.prob)
            })
        })
        .collect::<Vec<_>>();
    let target_forced_rows = target_fixed_games
        .iter()
        .map(|&index| {
            let fixture = &model.fixtures[index];
            let game = &model.request.games[fixture.request_index];
            let outcome = if fixture.home == target { 2 } else { 0 };
            json!({
                "game_id":game.id,
                "home_id":game.home_id,
                "away_id":game.away_id,
                "forced_outcome":outcome_name(outcome),
                "forced_mass":fixture.prob[outcome],
                "opponent_id":if fixture.home == target {game.away_id} else {game.home_id},
                "opponent_packed_gain":0
            })
        })
        .collect::<Vec<_>>();
    let shared_assignment_product = shared_games
        .iter()
        .map(|&index| propagated.domains[index].count_ones() as usize)
        .product::<usize>();

    let mut report = json!({
        "diagnostic":"restricted_rank_analysis",
        "snapshot":"group-16498-2026-10-18",
        "probability_law":"stored fixture.prob WDL probabilities",
        "group":model.request.id,
        "target":{"team_id":TARGET_ID,"rank":TARGET_RANK,"base_points":model.base[target].points,
            "base_wins":model.base[target].wins,"remaining_target_fixtures":target_games.len(),
            "target_final_packed_score":TARGET_PACKED,"target_all_wins_prior_mass":target_path_mass},
        "necessary_conditions":{
            "stride":STRIDE,
            "upper_cap_packed":TARGET_PACKED,
            "strict_packed_below_cap":TARGET_PACKED-1,
            "seven_existing_above_unconstrained":[17,16,5,8,4,74,15],
            "exception_team6":"Santos; its lower bound is implied because an all-loss path forces team225 above its cap",
            "upper_capped_team_ids":cap_ids,
            "exception_id_not_in_cap_graph":6
        },
        "domain_propagation":{
            "feasible":propagated.feasible,
            "domain_counts_by_popcount":domain_counts,
            "target_forced_games":target_fixed_games.len(),
            "domain_mass_product":domain_mass_product,
            "domain_mass_ratio_to_target_path":domain_mass_product/target_path_mass,
            "additional_singleton_games":propagated.domains.iter().filter(|mask| mask.count_ones()==1).count().saturating_sub(target_games.len())
        },
        "fixture_partition":{
            "total_remaining_fixtures":model.fixtures.len(),
            "target_forced_games":target_fixed_games.len(),
            "irrelevant_free_free_games":irrelevant_games.len(),
            "one_cap_endpoint_games":unary_games.iter().map(Vec::len).sum::<usize>(),
            "shared_cap_edges":shared_games.len(),
            "shared_edge_raw_assignment_product":shared_assignment_product,
            "irrelevant_game_ids":irrelevant_games.iter().map(|&i| model.request.games[model.fixtures[i].request_index].id).collect::<Vec<_>>()
        },
        "shared_graph":{
            "nodes":cap_ids,
            "edges":edges,
            "min_fill_order":min_fill_ids,
            "induced_width":induced_width
        },
        "unary_marginals":pmfs,
        "exact_wdl_mass":{
            "method":"bounded shared-edge DFS; unary fixtures integrated as independent raw-mass PMFs",
            "node_quota_per_run":SHARED_NODE_QUOTA,
            "upper_cap":{"packed_cap":TARGET_PACKED,"status":if upper_cap_result.weighted_mass.is_some(){"complete"}else{"unknown_node_quota"},
                "nodes":upper_cap_result.nodes,"conditional_mass":upper_cap_result.weighted_mass,
                "absolute_joint_mass":absolute_mass(target_path_mass,upper_cap_result.weighted_mass)},
            "strict_below":{"packed_cap":TARGET_PACKED-1,"status":if strict_cap_result.weighted_mass.is_some(){"complete"}else{"unknown_node_quota"},
                "nodes":strict_cap_result.nodes,"conditional_mass":strict_cap_result.weighted_mass,
                "absolute_joint_mass":absolute_mass(target_path_mass,strict_cap_result.weighted_mass)},
            "interpretation":"upper cap is a necessary-rank relaxation; strict packed-below mass puts seven baseline-ahead teams plus Santos above Chapecoense and all eleven capped rivals below, yielding ninth; exact score tiebreak boundary remains unresolved"
        },
        "target_forced_game_domains_and_masses":target_forced_rows,
        "relevant_game_domains_and_masses":relevant_rows,
        "scope_note":"Snapshot-specific WDL analysis uses stored fixture.prob masses. Score tiebreaks remain unresolved for packed-score ties; strict packed-below mass is a guaranteed-rank subset, and the all-at-cap mass is only a necessary-condition upper bound."
    });
    if let Some((samples, seed)) = gd_options {
        let upper_mass = upper_cap_result
            .weighted_mass
            .ok_or("upper cap counter did not complete")?;
        let strict_mass = strict_cap_result
            .weighted_mass
            .ok_or("strict cap counter did not complete")?;
        let gd_report = conditioned_gd_report(
            &model,
            target,
            &propagated.domains,
            &target_fixed_games,
            &shared_games,
            &shared_edges_for_count,
            &unary_games,
            &cap_indices,
            &base_fixed,
            &irrelevant_games,
            target_path_mass,
            strict_mass,
            upper_mass,
            samples,
            seed,
        )?;
        report["gd_monte_carlo"] = gd_report;
    }
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn conditioned_gd_report(
    model: &Model,
    target: usize,
    domains: &[u8],
    target_fixed_games: &[usize],
    shared_games: &[usize],
    edges: &[SharedEdge],
    unary_games: &[Vec<usize>],
    cap_indices: &[usize],
    base_fixed: &[i32],
    irrelevant_games: &[usize],
    target_path_mass: f64,
    strict_mass: f64,
    upper_mass: f64,
    sample_count: usize,
    seed: i64,
) -> Result<Value, Box<dyn std::error::Error>> {
    if model.keys.get(2) != Some(&Key::Gd) || model.keys.get(3) != Some(&Key::Gf) {
        return Err("GD diagnostic requires points/wins/GD/GF comparator".into());
    }
    let enumeration = enumerate_cap_leaves(
        edges,
        &unary_distributions_for(model, domains, cap_indices, base_fixed, unary_games),
        base_fixed,
        TARGET_PACKED,
        SHARED_NODE_QUOTA,
        SHARED_LEAF_QUOTA,
    );
    let leaf_mass = enumeration.mass.ok_or_else(|| {
        format!(
            "conditioned leaf enumeration incomplete (nodes={}, node_quota={}, leaf_quota={})",
            enumeration.nodes, SHARED_NODE_QUOTA, SHARED_LEAF_QUOTA
        )
    })?;
    if !relative_close(leaf_mass, upper_mass, 1e-10) {
        return Err(format!(
            "leaf mass {leaf_mass:.17e} disagrees with cap mass {upper_mass:.17e}"
        )
        .into());
    }
    let support_leaves = enumeration.leaves.as_ref().map_or(0, Vec::len);

    let mut unary_choices = vec![Vec::new(); cap_indices.len()];
    for (local, &team) in cap_indices.iter().enumerate() {
        for &fixture_index in &unary_games[local] {
            let fixture = &model.fixtures[fixture_index];
            unary_choices[local].push(conditioned::UnaryChoice {
                fixture_index,
                mask: domains[fixture_index],
                probability: fixture.prob,
                gain: std::array::from_fn(|outcome| score_gain(outcome, fixture.home == team)),
            });
        }
    }
    let irrelevant = irrelevant_games
        .iter()
        .map(|&i| {
            let fixture = &model.fixtures[i];
            if domains[i] != 0b111 || (fixture.prob.iter().sum::<f64>() - 1.0).abs() > 1e-12 {
                return Err(format!(
                    "free/free fixture {} is not an unrestricted unit-mass WDL law",
                    fixture.request_index
                ));
            }
            Ok((i, domains[i], fixture.prob))
        })
        .collect::<Result<Vec<_>, String>>()
        .map_err(boxed_error)?;
    let target_fixed = target_fixed_games
        .iter()
        .map(|&i| {
            (
                i,
                if model.fixtures[i].home == target {
                    2
                } else {
                    0
                },
            )
        })
        .collect::<Vec<_>>();
    let max_remaining = base_fixed
        .iter()
        .map(|&base| TARGET_PACKED - base)
        .collect::<Vec<_>>();
    let sampler = conditioned::Sampler::new(
        enumeration.leaves.unwrap(),
        shared_games.to_vec(),
        edges,
        unary_choices,
        &max_remaining,
        irrelevant,
        target_fixed,
        model.fixtures.len(),
    )
    .map_err(boxed_error)?;

    let mut integrator =
        gd::Integrator::new(model).map_err(|e| boxed_error(format!("GD integrator: {e}")))?;
    let mut gd_rng = Rng::new(derive(seed, "chapecoense-conditioned-gd"));
    let mut ref_wdl_rng = Rng::new(derive(seed, "chapecoense-conditioned-reference-wdl"));
    let mut ref_score_rng = Rng::new(derive(seed, "chapecoense-conditioned-reference-scores"));
    let mut gd_strict = Vec::with_capacity(sample_count);
    let mut gd_weak = Vec::with_capacity(sample_count);
    let mut tied_histogram = BTreeMap::<usize, usize>::new();
    let mut shared_game_total = 0usize;
    let mut reference_hits = 0usize;
    let mut reference_order = vec![0; model.n];
    let run_started = Instant::now();
    for _ in 0..sample_count {
        let outcomes = sampler.sample(&mut gd_rng).map_err(boxed_error)?;
        validate_conditioned_path(model, &outcomes, target, cap_indices).map_err(boxed_error)?;
        let projection = integrator
            .evaluate(model, &outcomes, target, &mut gd_rng)
            .map_err(|e| boxed_error(format!("GD projection: {e}")))?;
        if !projection.strict.is_finite()
            || !projection.weak.is_finite()
            || projection.strict < 0.0
            || projection.weak < projection.strict
            || projection.weak > 1.0 + 1e-12
        {
            return Err("GD projection returned invalid bounds".into());
        }
        gd_strict.push(projection.strict);
        gd_weak.push(projection.weak);
        *tied_histogram.entry(projection.tied_rivals).or_default() += 1;
        shared_game_total += projection.shared_games;

        let reference_outcomes = sampler.sample(&mut ref_wdl_rng).map_err(boxed_error)?;
        validate_conditioned_path(model, &reference_outcomes, target, cap_indices)
            .map_err(boxed_error)?;
        let mut campaigns = model.base.clone();
        let mut scores = model.empty_scores();
        for (i, (&outcome, fixture)) in reference_outcomes.iter().zip(&model.fixtures).enumerate() {
            let score = fixture.scores.sample(outcome as usize, &mut ref_score_rng);
            scores[fixture.request_index] = score;
            model.add(&mut campaigns, i, score);
        }
        model.standings(
            &mut reference_order,
            &campaigns,
            &scores,
            &mut ref_score_rng,
        );
        if reference_order.iter().position(|&team| team == target) == Some(TARGET_RANK - 1) {
            reference_hits += 1;
        }
    }
    let (strict_mean, strict_variance) = mean_variance(&gd_strict);
    let (weak_mean, weak_variance) = mean_variance(&gd_weak);
    let strict_standard_error = (strict_variance / sample_count as f64).sqrt();
    let weak_standard_error = (weak_variance / sample_count as f64).sqrt();
    let ln_factor = (2.0 / 0.005f64).ln();
    let strict_radius = empirical_bernstein_radius(strict_variance, sample_count, ln_factor);
    let weak_radius = empirical_bernstein_radius(weak_variance, sample_count, ln_factor);
    let exact_strict_absolute = target_path_mass * strict_mass;
    let upper_absolute = target_path_mass * upper_mass;
    let estimate_lower = (exact_strict_absolute + upper_absolute * strict_mean)
        .clamp(exact_strict_absolute, upper_absolute);
    let estimate_upper =
        (exact_strict_absolute + upper_absolute * weak_mean).clamp(estimate_lower, upper_absolute);
    let confidence_lower = (exact_strict_absolute
        + upper_absolute * (strict_mean - strict_radius).max(0.0))
    .clamp(exact_strict_absolute, upper_absolute);
    let confidence_upper = (exact_strict_absolute
        + upper_absolute * (weak_mean + weak_radius).min(1.0))
    .clamp(confidence_lower, upper_absolute);
    let (reference_low, reference_high) = wilson_interval(reference_hits, sample_count, 0.99);
    Ok(json!({
        "samples":sample_count,
        "seed":seed,
        "conditioned_sampler":{"support_leaves":support_leaves,"dfs_nodes":enumeration.nodes,"mass":sampler.total_mass(),"node_quota":SHARED_NODE_QUOTA,"leaf_quota":SHARED_LEAF_QUOTA},
        "gd_projection":{"mean_strict_correction":strict_mean,"mean_weak_correction":weak_mean,"strict_sample_variance":strict_variance,"weak_sample_variance":weak_variance,"strict_standard_error":strict_standard_error,"weak_standard_error":weak_standard_error,"strict_absolute_standard_error_scale":upper_absolute*strict_standard_error,"weak_absolute_standard_error_scale":upper_absolute*weak_standard_error,"strict_empirical_bernstein_radius_99_each":strict_radius,"weak_empirical_bernstein_radius_99_each":weak_radius,"tied_rival_histogram":tied_histogram,"mean_shared_games":shared_game_total as f64/sample_count as f64},
        "absolute_probability_bounds":{"known_strict_mass":exact_strict_absolute,"point_estimate_lower":estimate_lower,"point_estimate_upper":estimate_upper,"simultaneous_99_percent_lower":confidence_lower,"simultaneous_99_percent_upper":confidence_upper,"known_necessary_condition_upper":upper_absolute},
        "independent_native_score_reference":{"hits":reference_hits,"samples":sample_count,"conditional_probability":reference_hits as f64/sample_count as f64,"conditional_wilson_99_percent":[reference_low,reference_high],"absolute_probability_estimate":upper_absolute*reference_hits as f64/sample_count as f64,"absolute_wilson_99_percent":[upper_absolute*reference_low,upper_absolute*reference_high]},
        "runtime_seconds":run_started.elapsed().as_secs_f64(),
        "scope":"snapshot diagnostic only; GD projections and native reference are Monte Carlo estimates under the exact packed-cap WDL-conditioned sampler"
    }))
}

fn validate_conditioned_path(
    model: &Model,
    outcomes: &[u8],
    target: usize,
    capped: &[usize],
) -> Result<(), String> {
    if outcomes.len() != model.fixtures.len() {
        return Err("conditioned WDL path has wrong fixture count".into());
    }
    let mut campaigns = model.base.clone();
    for (i, &outcome) in outcomes.iter().enumerate() {
        let score = match outcome {
            0 => [0, 1],
            1 => [0, 0],
            2 => [1, 0],
            _ => return Err("conditioned WDL path contains invalid outcome".into()),
        };
        model.add(&mut campaigns, i, score);
    }
    let target_score = campaigns[target].points * STRIDE + campaigns[target].wins;
    if target_score != TARGET_PACKED {
        return Err("conditioned sample does not preserve target all-wins score".into());
    }
    if capped
        .iter()
        .any(|&team| campaigns[team].points * STRIDE + campaigns[team].wins > TARGET_PACKED)
    {
        return Err("conditioned sample exceeded a rival packed cap".into());
    }
    Ok(())
}

fn unary_distributions_for(
    model: &Model,
    domains: &[u8],
    cap_indices: &[usize],
    base_fixed: &[i32],
    unary_games: &[Vec<usize>],
) -> Vec<BTreeMap<i32, f64>> {
    cap_indices
        .iter()
        .enumerate()
        .map(|(local, &team)| {
            let mut distribution = BTreeMap::from([(base_fixed[local], 1.0)]);
            for &i in &unary_games[local] {
                let fixture = &model.fixtures[i];
                let mut next = BTreeMap::<i32, f64>::new();
                for (&score, &mass) in &distribution {
                    for outcome in 0..3 {
                        if domains[i] & (1 << outcome) != 0 {
                            *next
                                .entry(score + score_gain(outcome, fixture.home == team))
                                .or_default() += mass * fixture.prob[outcome];
                        }
                    }
                }
                distribution = next;
            }
            distribution
        })
        .collect()
}

fn relative_close(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= b.abs().max(f64::MIN_POSITIVE) * tolerance
}

fn boxed_error(message: String) -> Box<dyn std::error::Error> {
    std::io::Error::new(std::io::ErrorKind::Other, message).into()
}

fn mean_variance(values: &[f64]) -> (f64, f64) {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance =
        values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    (mean, variance)
}

fn empirical_bernstein_radius(variance: f64, n: usize, ln_factor: f64) -> f64 {
    (2.0 * variance * ln_factor / n as f64).sqrt() + 7.0 * ln_factor / (3.0 * (n - 1) as f64)
}

fn wilson_interval(hits: usize, n: usize, confidence: f64) -> (f64, f64) {
    let z = if confidence >= 0.99 {
        2.575_829_303_548_900_4
    } else {
        1.959_963_984_540_054
    };
    let p = hits as f64 / n as f64;
    let z2 = z * z;
    let denominator = 1.0 + z2 / n as f64;
    let center = (p + z2 / (2.0 * n as f64)) / denominator;
    let radius =
        z * ((p * (1.0 - p) / n as f64 + z2 / (4.0 * (n as f64).powi(2))).sqrt()) / denominator;
    ((center - radius).max(0.0), (center + radius).min(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close_relative(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= expected.abs() * tolerance,
            "{actual:.17e} differs from {expected:.17e}"
        );
    }

    #[test]
    fn tiny_weighted_graph_matches_exhaustive_enumeration() {
        let edges = [SharedEdge {
            a: 0,
            b: 1,
            mask: 0b111,
            probability: [0.2, 0.3, 0.5],
            gain_a: [0, 28, 85],
            gain_b: [85, 28, 0],
        }];
        let unary = [
            BTreeMap::from([(0, 0.5), (28, 0.5)]),
            BTreeMap::from([(0, 0.75), (28, 0.25)]),
        ];
        let counted = count_cap_mass(&edges, &unary, &[0, 0], 28, 100);
        let exhaustive = 0.3 * (0.5 * 0.75);
        close_relative(counted.weighted_mass.unwrap(), exhaustive, 1e-15);
        let quota_limited = count_cap_mass(&edges, &unary, &[0, 0], 28, 1);
        assert!(quota_limited.weighted_mass.is_none());
        let partial = enumerate_cap_leaves(&edges, &unary, &[0, 0], 28, 1, 10);
        assert!(partial.mass.is_none());
        assert!(partial.leaves.is_none());
    }

    #[test]
    fn snapshot_exact_wdl_masses_match_independent_counter() {
        let path = format!(
            "{}/tests/fixtures/group-16498-2026-10-18.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let report = analyze(&path, None).unwrap();
        let exact = &report["exact_wdl_mass"];
        assert_eq!(exact["upper_cap"]["status"], "complete");
        assert_eq!(exact["strict_below"]["status"], "complete");
        let upper = exact["upper_cap"]["conditional_mass"].as_f64().unwrap();
        let strict = exact["strict_below"]["conditional_mass"].as_f64().unwrap();
        let target = report["target"]["target_all_wins_prior_mass"]
            .as_f64()
            .unwrap();
        assert!(strict < upper);
        close_relative(upper, 4.110578532406328e-15, 1e-10);
        close_relative(strict, 3.394107318625757e-18, 1e-10);
        close_relative(
            exact["upper_cap"]["absolute_joint_mass"].as_f64().unwrap(),
            target * upper,
            1e-15,
        );
        close_relative(
            exact["strict_below"]["absolute_joint_mass"]
                .as_f64()
                .unwrap(),
            target * strict,
            1e-15,
        );
    }
}
