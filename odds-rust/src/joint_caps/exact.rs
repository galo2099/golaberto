//! Proof-gated exact packed-points caps for a narrow points/wins regime.
//!
//! This proposal is admitted only after bounded propagation proves that one
//! exceptional-above-cap set is the sole feasible set. The subsequent WDL
//! mass and sampler retain raw fixture probabilities and decline atomically
//! whenever a proof, work, memory, or support bound is incomplete.

use crate::{
    conditioned::{Result, ScoreContext},
    domains::{DomainWork, Domains},
    lookahead::RankGame,
    model::{Key, Model},
    rng::Rng,
    search::Cell,
};
use serde_json::{json, Value};
use std::mem::size_of;

const MAX_TEAMS: usize = 32;
const MAX_FIXTURES: usize = 128;
const MAX_CAP_TEAMS: usize = 16;
const MAX_SHARED_EDGES: usize = 32;
const MAX_SHARED_NODES: usize = 4_000_000;
const MAX_LEAVES: usize = 600_000;
const MAX_STRICT_NODES: usize = 200_000;
const MAX_PLAN_BYTES: usize = 32 * 1024 * 1024;
const META_BYTES: usize = 256 * 1024;

#[derive(Clone)]
struct Geometry {
    target: usize,
    rank: usize,
    stride: i32,
    cap: i32,
    target_mass: f64,
    base_packed: Vec<i32>,
    base_fixed: Vec<i32>,
    target_fixed: Vec<(usize, u8)>,
    guaranteed: Vec<usize>,
    possible_caps: Vec<usize>,
    gap: usize,
    rivals: Vec<usize>,
    games: Vec<RankGame>,
    seed_masks: Vec<u8>,
}

#[derive(Clone)]
struct SharedEdge {
    fixture_index: usize,
    a: usize,
    b: usize,
    mask: u8,
    probability: [f64; 3],
    gain_a: [i32; 3],
    gain_b: [i32; 3],
}

#[derive(Clone)]
struct UnaryGame {
    fixture_index: usize,
    mask: u8,
    probability: [f64; 3],
    gain: [i32; 3],
}

struct UnaryTable {
    games: Vec<UnaryGame>,
    cdf: Vec<Vec<f64>>,
    cap: usize,
}

impl UnaryTable {
    fn build(games: Vec<UnaryGame>, cap: usize) -> Self {
        let mut cdf = vec![vec![1.0; cap + 1]; games.len() + 1];
        for index in (0..games.len()).rev() {
            for allowance in 0..=cap {
                cdf[index][allowance] = (0..3)
                    .filter(|&outcome| {
                        games[index].mask & (1 << outcome) != 0
                            && games[index].gain[outcome] >= 0
                            && games[index].gain[outcome] as usize <= allowance
                    })
                    .map(|outcome| {
                        let gain = games[index].gain[outcome] as usize;
                        games[index].probability[outcome] * cdf[index + 1][allowance - gain]
                    })
                    .sum();
            }
        }
        Self { games, cdf, cap }
    }

    fn mass(&self, cap: i32) -> f64 {
        if cap < 0 {
            0.0
        } else {
            self.cdf[0][(cap as usize).min(self.cap)]
        }
    }

    fn sample(&self, mut cap: i32, rng: &mut Rng, outcomes: &mut [u8]) -> bool {
        for (index, game) in self.games.iter().enumerate() {
            let mut weights = [0.0; 3];
            for outcome in 0..3 {
                let gain = game.gain[outcome];
                if game.mask & (1 << outcome) != 0 && gain >= 0 && gain <= cap {
                    weights[outcome] = game.probability[outcome]
                        * self.cdf[index + 1][cap as usize - gain as usize];
                }
            }
            let total = weights.iter().sum::<f64>();
            if !total.is_finite() || total <= 0.0 {
                return false;
            }
            let mut draw = rng.float() * total;
            let mut selected = None;
            for outcome in 0..3 {
                if weights[outcome] > 0.0 && draw < weights[outcome] {
                    selected = Some(outcome);
                    break;
                }
                draw -= weights[outcome];
            }
            let Some(selected) = selected.or_else(|| (0..3).rev().find(|&o| weights[o] > 0.0))
            else {
                return false;
            };
            outcomes[game.fixture_index] = selected as u8;
            cap -= game.gain[selected];
        }
        true
    }
}

#[derive(Clone, Copy)]
struct Leaf {
    code: u64,
    weight: f64,
}

struct Counter<'a> {
    edges: &'a [SharedEdge],
    tables: &'a [UnaryTable],
    node_limit: usize,
    leaf_limit: usize,
    node_charge: usize,
    leaf_charge: usize,
    nodes: usize,
    work: &'a mut DomainWork,
    exhausted: bool,
    leaves: Vec<Leaf>,
    mass: f64,
    save_leaves: bool,
}

impl Counter<'_> {
    fn visit(&mut self, edge_index: usize, remaining: &mut [i32], code: u64, weight: f64) {
        if self.exhausted {
            return;
        }
        if !charge(self.work, self.node_charge) {
            self.exhausted = true;
            return;
        }
        if self.nodes >= self.node_limit {
            self.exhausted = true;
            return;
        }
        self.nodes += 1;
        for (team, &cap) in remaining.iter().enumerate() {
            if cap < 0 || self.tables[team].mass(cap) == 0.0 {
                return;
            }
        }
        if edge_index == self.edges.len() {
            let mut leaf_weight = weight;
            for (team, &cap) in remaining.iter().enumerate() {
                leaf_weight *= self.tables[team].mass(cap);
            }
            if !leaf_weight.is_finite() {
                self.exhausted = true;
                return;
            }
            if leaf_weight > 0.0 {
                if self.save_leaves {
                    if self.leaves.len() >= self.leaf_limit || !charge(self.work, self.leaf_charge)
                    {
                        self.exhausted = true;
                        return;
                    }
                    self.leaves.push(Leaf {
                        code,
                        weight: leaf_weight,
                    });
                }
                self.mass += leaf_weight;
            }
            return;
        }
        let edge = &self.edges[edge_index];
        for outcome in 0..3 {
            if edge.mask & (1 << outcome) == 0 || edge.probability[outcome] <= 0.0 {
                continue;
            }
            let ga = edge.gain_a[outcome];
            let gb = edge.gain_b[outcome];
            if ga > remaining[edge.a] || gb > remaining[edge.b] {
                continue;
            }
            remaining[edge.a] -= ga;
            remaining[edge.b] -= gb;
            self.visit(
                edge_index + 1,
                remaining,
                code | ((outcome as u64) << (2 * edge_index)),
                weight * edge.probability[outcome],
            );
            remaining[edge.a] += ga;
            remaining[edge.b] += gb;
            if self.exhausted {
                return;
            }
        }
    }
}

pub(crate) struct Build {
    pub plan: Option<ExactCaps>,
    pub work: usize,
    pub reason: &'static str,
}

pub(crate) struct ExactCaps {
    target: usize,
    rank: usize,
    stride: i32,
    cap: i32,
    target_mass: f64,
    joint_mass: f64,
    guaranteed_mass: f64,
    target_fixed: Vec<(usize, u8)>,
    base_packed: Vec<i32>,
    base_fixed: Vec<i32>,
    cap_teams: Vec<usize>,
    shared: Vec<SharedEdge>,
    unary: Vec<UnaryTable>,
    irrelevant: Vec<(usize, [f64; 3])>,
    leaves: Vec<Leaf>,
    tree: Vec<f64>,
    strict_cv: bool,
    guaranteed_ids: Vec<i32>,
    exceptional_id: Option<i32>,
    domain_case_count: usize,
    domain_work: usize,
    shared_nodes: usize,
    strict_nodes: usize,
    memory_bytes: usize,
}

impl ExactCaps {
    pub(crate) fn eligible(model: &Model, cell: Cell) -> bool {
        geometry(model, cell).is_some_and(|g| {
            candidate_cases(model, &g)
                .iter()
                .any(|exception| within_graph_limits(model, &g, *exception))
        })
    }

    pub(crate) fn build(model: &Model, cell: Cell, allowance: usize) -> Build {
        let Some(geometry) = geometry(model, cell) else {
            return declined(0, "structural_gate");
        };
        if !candidate_cases(model, &geometry)
            .iter()
            .any(|exception| within_graph_limits(model, &geometry, *exception))
        {
            return declined(0, "graph_gate");
        }
        let mut work = DomainWork::new(allowance);
        if !charge(&mut work, 16 * geometry.games.len() + 8 * model.n) {
            return declined(work.used, "work_budget");
        }

        let cases = candidate_cases(model, &geometry);
        let domain_case_count = cases.len();
        let mut feasible = Vec::<(Option<usize>, Domains)>::new();
        for exception in cases {
            let mut lower = vec![i32::MIN / 4; model.n];
            let mut upper = vec![i32::MAX / 4; model.n];
            lower[geometry.target] = geometry.cap;
            upper[geometry.target] = geometry.cap;
            for &team in &geometry.possible_caps {
                if Some(team) == exception {
                    lower[team] = geometry.cap.saturating_add(1);
                } else {
                    upper[team] = geometry.cap;
                }
            }
            let Some(domains) = Domains::condition_bounded(
                &geometry.games,
                &geometry.base_packed,
                &geometry.rivals,
                geometry.rank,
                geometry.cap,
                &lower,
                &upper,
                &geometry.seed_masks,
                &mut work,
            ) else {
                return declined(work.used, "domain_work_budget");
            };
            if domains.feasible {
                feasible.push((exception, domains));
                if feasible.len() > 1 {
                    return declined(work.used, "exceptional_cases_not_unique");
                }
            }
        }
        if feasible.len() != 1 {
            return declined(work.used, "exceptional_cases_not_unique");
        }
        let (exception, domains) = feasible.pop().unwrap();
        // If the unique feasible case has an exception, the empty-exception
        // case was enumerated and proved infeasible above, making this lower
        // bound redundant once every other rival stays at or below the cap.

        let cap_teams = geometry
            .possible_caps
            .iter()
            .copied()
            .filter(|team| Some(*team) != exception)
            .collect::<Vec<_>>();
        if cap_teams.len() > MAX_CAP_TEAMS {
            return declined(work.used, "cap_team_limit");
        }
        let local = cap_teams
            .iter()
            .enumerate()
            .map(|(i, &team)| (team, i))
            .collect::<std::collections::HashMap<_, _>>();
        let mut shared = Vec::new();
        let mut unary_games = vec![Vec::<UnaryGame>::new(); cap_teams.len()];
        let mut irrelevant = Vec::new();
        for (i, fixture) in model.fixtures.iter().enumerate() {
            let mask = domains.domains[i];
            if mask == 0 {
                return declined(work.used, "empty_domain");
            }
            if geometry.target_fixed.iter().any(|(fixed, _)| *fixed == i) {
                continue;
            }
            let home = local.get(&fixture.home).copied();
            let away = local.get(&fixture.away).copied();
            match (home, away) {
                (Some(a), Some(b)) => {
                    if shared.len() >= MAX_SHARED_EDGES {
                        return declined(work.used, "shared_edge_limit");
                    }
                    shared.push(SharedEdge {
                        fixture_index: i,
                        a,
                        b,
                        mask,
                        probability: fixture.prob,
                        gain_a: std::array::from_fn(|o| {
                            packed_gain(o, fixture.home == cap_teams[a], geometry.stride)
                        }),
                        gain_b: std::array::from_fn(|o| {
                            packed_gain(o, fixture.home == cap_teams[b], geometry.stride)
                        }),
                    });
                }
                (Some(local_team), None) => unary_games[local_team].push(UnaryGame {
                    fixture_index: i,
                    mask,
                    probability: fixture.prob,
                    gain: std::array::from_fn(|o| {
                        packed_gain(o, fixture.home == cap_teams[local_team], geometry.stride)
                    }),
                }),
                (None, Some(local_team)) => unary_games[local_team].push(UnaryGame {
                    fixture_index: i,
                    mask,
                    probability: fixture.prob,
                    gain: std::array::from_fn(|o| {
                        packed_gain(o, fixture.home == cap_teams[local_team], geometry.stride)
                    }),
                }),
                (None, None) => {
                    let support = positive_mask(fixture.prob);
                    let mass = (0..3)
                        .filter(|&o| mask & (1 << o) != 0)
                        .map(|o| fixture.prob[o])
                        .sum::<f64>();
                    if mask != support || !mass.is_finite() || (mass - 1.0).abs() > 1e-12 {
                        return declined(work.used, "free_fixture_restricted");
                    }
                    irrelevant.push((i, fixture.prob));
                }
            }
        }
        if shared.len() > MAX_SHARED_EDGES {
            return declined(work.used, "shared_edge_limit");
        }

        let mut unary_specs = Vec::with_capacity(cap_teams.len());
        let mut cdf_bytes = 0usize;
        let mut unary_work = 0usize;
        for (local_team, team_games) in unary_games.into_iter().enumerate() {
            let cap_remaining = geometry.cap - geometry.base_fixed[cap_teams[local_team]];
            if cap_remaining < 0 {
                return declined(work.used, "invalid_base_cap");
            }
            let cap = cap_remaining as usize;
            cdf_bytes = cdf_bytes.saturating_add(
                (team_games.len() + 1)
                    .saturating_mul(cap + 1)
                    .saturating_mul(size_of::<f64>())
                    .saturating_add((team_games.len() + 1) * size_of::<Vec<f64>>()),
            );
            unary_work = unary_work
                .saturating_add(team_games.len().saturating_mul(cap + 1).saturating_mul(4));
            unary_specs.push((team_games, cap));
        }
        let max_tree_base = MAX_LEAVES.next_power_of_two();
        let max_bytes = META_BYTES
            .saturating_add(MAX_LEAVES.saturating_mul(size_of::<Leaf>()))
            .saturating_add(
                2usize
                    .saturating_mul(max_tree_base)
                    .saturating_mul(size_of::<f64>()),
            )
            .saturating_add(cdf_bytes)
            .saturating_add(shared.len().saturating_mul(size_of::<SharedEdge>()))
            .saturating_add(cap_teams.len().saturating_mul(size_of::<usize>()));
        if max_bytes > MAX_PLAN_BYTES {
            return declined(work.used, "memory_limit");
        }
        if !charge(&mut work, unary_work) {
            return declined(work.used, "work_budget");
        }
        let mut unary = Vec::with_capacity(unary_specs.len());
        for (games, cap) in unary_specs {
            unary.push(UnaryTable::build(games, cap));
        }

        let remaining = cap_teams
            .iter()
            .map(|&team| geometry.cap - geometry.base_fixed[team])
            .collect::<Vec<_>>();
        let node_charge = 3usize.saturating_mul(cap_teams.len()).saturating_add(2);
        let leaf_charge = cap_teams.len().saturating_add(2);
        if !charge(&mut work, MAX_LEAVES.saturating_mul(size_of::<Leaf>() / 2)) {
            return declined(work.used, "work_budget");
        }
        let mut upper_counter = Counter {
            edges: &shared,
            tables: &unary,
            node_limit: MAX_SHARED_NODES,
            leaf_limit: MAX_LEAVES,
            node_charge,
            leaf_charge,
            nodes: 0,
            work: &mut work,
            exhausted: false,
            leaves: Vec::with_capacity(MAX_LEAVES),
            mass: 0.0,
            save_leaves: true,
        };
        upper_counter.visit(0, &mut remaining.clone(), 0, 1.0);
        let shared_nodes = upper_counter.nodes;
        if upper_counter.exhausted || upper_counter.leaves.is_empty() {
            return declined(work.used, "upper_counter_incomplete");
        }
        let joint_mass = upper_counter.mass;
        if !joint_mass.is_finite() || joint_mass <= 0.0 || geometry.target_mass * joint_mass == 0.0
        {
            return declined(work.used, "invalid_mass");
        }
        let leaves = upper_counter.leaves;
        let tree_base = leaves.len().next_power_of_two();
        let tree_len = 2usize.saturating_mul(tree_base);
        let tree_bytes = tree_len.saturating_mul(size_of::<f64>());
        if max_bytes - 2 * max_tree_base * size_of::<f64>() + tree_bytes > MAX_PLAN_BYTES {
            return declined(work.used, "memory_limit");
        }
        if !charge(&mut work, tree_len.saturating_mul(2)) {
            return declined(work.used, "work_budget");
        }
        let mut tree = vec![0.0; tree_len];
        for (index, leaf) in leaves.iter().enumerate() {
            tree[tree_base + index] = leaf.weight;
        }
        for index in (1..tree_base).rev() {
            tree[index] = tree[2 * index] + tree[2 * index + 1];
        }
        if !tree[1].is_finite() || tree[1] <= 0.0 || !relative_close(tree[1], joint_mass, 1e-12) {
            return declined(work.used, "invalid_leaf_mass");
        }

        let (guaranteed_mass, strict_nodes, strict_cv) = strict_mass(
            &geometry,
            &cap_teams,
            &shared,
            &unary,
            geometry.guaranteed.len() + usize::from(exception.is_some()) == geometry.rank,
            &mut work,
        );
        let guaranteed_mass = guaranteed_mass
            .map(|mass| geometry.target_mass * mass)
            .filter(|mass| mass.is_finite() && *mass > 0.0)
            .unwrap_or(0.0);
        let mass = geometry.target_mass * tree[1];
        let cv = strict_cv && guaranteed_mass <= mass * (1.0 + 1e-10);
        let guaranteed_mass = if cv { guaranteed_mass.min(mass) } else { 0.0 };
        let actual_bytes = META_BYTES
            .saturating_add(leaves.capacity().saturating_mul(size_of::<Leaf>()))
            .saturating_add(tree.capacity().saturating_mul(size_of::<f64>()))
            .saturating_add(cdf_bytes)
            .saturating_add(shared.capacity().saturating_mul(size_of::<SharedEdge>()))
            .saturating_add(unary.capacity().saturating_mul(size_of::<UnaryTable>()));
        if actual_bytes > MAX_PLAN_BYTES {
            return declined(work.used, "memory_limit");
        }
        let plan = ExactCaps {
            target: geometry.target,
            rank: geometry.rank,
            stride: geometry.stride,
            cap: geometry.cap,
            target_mass: geometry.target_mass,
            joint_mass: tree[1],
            guaranteed_mass,
            target_fixed: geometry.target_fixed,
            base_packed: geometry.base_packed,
            base_fixed: geometry.base_fixed,
            cap_teams: cap_teams.clone(),
            shared,
            unary,
            irrelevant,
            leaves,
            tree,
            strict_cv: cv,
            guaranteed_ids: geometry.guaranteed.iter().map(|&t| model.ids[t]).collect(),
            exceptional_id: exception.map(|t| model.ids[t]),
            domain_case_count,
            domain_work: work.used,
            shared_nodes,
            strict_nodes,
            memory_bytes: actual_bytes,
        };
        Build {
            plan: Some(plan),
            work: work.used,
            reason: "admitted",
        }
    }

    pub(crate) fn mass(&self) -> f64 {
        self.target_mass * self.joint_mass
    }

    pub(crate) fn guaranteed_mass(&self) -> f64 {
        self.guaranteed_mass
    }

    pub(crate) fn draw_cost(&self, model: &Model) -> usize {
        let g = model.fixtures.len();
        let n = model.n;
        let depth = self.leaves.len().next_power_of_two().ilog2() as usize + 1;
        let guidance = 4 * g + 2 * n + depth + 4 * self.cap_teams.len();
        let ranking = 8 * g + n * (n.max(2).ilog2() as usize + 1) * model.keys.len().max(1);
        8 * g + guidance + ranking
    }

    pub(crate) fn describe(&self, model: &Model) -> Value {
        json!({
            "mode":"proof_gated_exact_caps",
            "target":model.ids[self.target],
            "rank":self.rank+1,
            "stride":self.stride,
            "packed_cap":self.cap,
            "target_path_mass":self.target_mass,
            "joint_cap_mass":self.joint_mass,
            "conditioning_mass":self.mass(),
            "guaranteed_mass":self.guaranteed_mass,
            "guaranteed_above_ids":self.guaranteed_ids,
            "exceptional_above_id":self.exceptional_id,
            "capped_ids":self.cap_teams.iter().map(|&t|model.ids[t]).collect::<Vec<_>>(),
            "target_fixed_games":self.target_fixed.len(),
            "shared_edges":self.shared.len(),
            "unary_games":self.unary.iter().map(|table|table.games.len()).sum::<usize>(),
            "free_free_games":self.irrelevant.len(),
            "leaves":self.leaves.len(),
            "shared_dfs_nodes":self.shared_nodes,
            "strict_dfs_nodes":self.strict_nodes,
            "strict_cv_available":self.strict_cv,
            "domain_case_count":self.domain_case_count,
            "domain_work":self.domain_work,
            "memory_bytes":self.memory_bytes,
            "draw_cost":self.draw_cost(model)
        })
    }

    pub(crate) fn sample(&self, model: &Model, samples: usize, seed: i64) -> Result {
        let mut result = Result {
            mass: self.mass(),
            samples,
            ..Result::default()
        };
        if samples == 0 || self.mass() <= 0.0 {
            return result;
        }
        let mut rng = Rng::new(seed);
        let mut score_context = ScoreContext::new(model);
        let mut outcomes = vec![0u8; model.fixtures.len()];
        let mut points = self.base_packed.clone();
        let allow = self
            .cap_teams
            .iter()
            .map(|&team| self.cap - self.base_fixed[team])
            .collect::<Vec<_>>();
        let strict_fraction = if self.strict_cv {
            (self.guaranteed_mass / self.mass()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mut sum = 0.0;
        let mut sum2 = 0.0;
        let mut max = 0.0f64;
        let mut batches = [0.0; 2];
        for draw in 0..samples {
            outcomes.fill(0);
            for &(index, outcome) in &self.target_fixed {
                outcomes[index] = outcome;
            }
            let Some(leaf_index) = self.draw_leaf(&mut rng) else {
                result.samples = draw;
                result.weighted = false;
                self.charge_sample_work(model, &mut result, draw + 1);
                return result;
            };
            let code = self.leaves[leaf_index].code;
            let mut remaining = allow.clone();
            for (index, edge) in self.shared.iter().enumerate() {
                let outcome = ((code >> (2 * index)) & 3) as usize;
                outcomes[edge.fixture_index] = outcome as u8;
                remaining[edge.a] -= edge.gain_a[outcome];
                remaining[edge.b] -= edge.gain_b[outcome];
            }
            for (local, table) in self.unary.iter().enumerate() {
                if !table.sample(remaining[local], &mut rng, &mut outcomes) {
                    result.samples = draw;
                    result.weighted = false;
                    self.charge_sample_work(model, &mut result, draw + 1);
                    return result;
                }
            }
            for &(fixture, probability) in &self.irrelevant {
                outcomes[fixture] = sample_raw(probability, &mut rng) as u8;
            }
            points.copy_from_slice(&self.base_packed);
            for (index, fixture) in model.fixtures.iter().enumerate() {
                let outcome = outcomes[index] as usize;
                let rules = &model.request.phase.championship;
                let hg = [
                    rules.point_loss * self.stride,
                    rules.point_draw * self.stride,
                    rules.point_win * self.stride + 1,
                ];
                let ag = [hg[2], hg[1], hg[0]];
                points[fixture.home] += hg[outcome];
                points[fixture.away] += ag[outcome];
            }
            let within_caps = self.cap_teams.iter().all(|&team| points[team] <= self.cap);
            if !within_caps {
                result.samples = draw;
                result.weighted = false;
                self.charge_sample_work(model, &mut result, draw + 1);
                return result;
            }
            if points[self.target] != self.cap {
                result.samples = draw;
                result.weighted = false;
                self.charge_sample_work(model, &mut result, draw + 1);
                return result;
            }
            let strict_packed = self.cap_teams.iter().all(|&team| points[team] < self.cap);
            let rank_hit = score_context.rank(self.target, &outcomes, &mut rng, None) == self.rank;
            result.hits += usize::from(rank_hit);
            if rank_hit && result.witness.is_none() {
                result.witness = Some(outcomes.clone());
            }
            let weight = if self.strict_cv {
                strict_fraction + f64::from(rank_hit && !strict_packed)
            } else {
                f64::from(rank_hit)
            };
            if weight > 0.0 {
                sum += weight;
                sum2 += weight * weight;
                max = max.max(weight);
                batches[usize::from(draw >= samples / 2)] += weight;
            }
        }
        self.charge_sample_work(model, &mut result, samples);
        result.blockers = samples.saturating_sub(result.hits);
        let batches = batches;
        result.summarize(sum, sum2, max, batches);
        if sum == 0.0 {
            result.weighted = true;
            result.probability = 0.0;
            result.std_err = 0.0;
            result.ess = 0.0;
            result.max_share = 0.0;
            result.batch_gap = 1.0;
        }
        result
    }

    fn draw_leaf(&self, rng: &mut Rng) -> Option<usize> {
        let mut threshold = rng.float() * self.tree[1];
        let mut node = 1usize;
        let base = self.tree.len() / 2;
        while node < base {
            let left = self.tree[node * 2];
            if threshold < left {
                node *= 2;
            } else {
                threshold -= left;
                node = node * 2 + 1;
            }
        }
        let index = node - base;
        (index < self.leaves.len() && self.leaves[index].weight > 0.0).then_some(index)
    }

    fn charge_sample_work(&self, model: &Model, result: &mut Result, draws: usize) {
        let draws = draws as u64;
        result.work = draws * (model.fixtures.len() + model.n) as u64;
        result.operations.fixtures = draws * model.fixtures.len() as u64;
        result.operations.guidance = draws
            * (4 * model.fixtures.len()
                + 2 * model.n
                + self.leaves.len().next_power_of_two().ilog2() as usize
                + 1
                + 4 * self.cap_teams.len()) as u64;
        result.operations.ranking = draws
            * (8 * model.fixtures.len()
                + model.n * (model.n.max(2).ilog2() as usize + 1) * model.keys.len().max(1))
                as u64;
    }
}

fn geometry(model: &Model, cell: Cell) -> Option<Geometry> {
    if model.n == 0
        || model.n > MAX_TEAMS
        || model.ids.len() != model.n
        || model.fixtures.len() > MAX_FIXTURES
        || cell.team >= model.n
        || cell.rank >= model.n
        || model.keys.first() != Some(&Key::Pt)
        || model.keys.get(1) != Some(&Key::W)
        || model.request.phase.bonus_points != 0
        || (
            model.request.phase.championship.point_win,
            model.request.phase.championship.point_draw,
            model.request.phase.championship.point_loss,
        ) != (3, 1, 0)
        || model
            .fixtures
            .iter()
            .any(|f| f.home == f.away || f.home >= model.n || f.away >= model.n)
        || !valid_score_suffix(model)
    {
        return None;
    }
    let mut left = vec![0usize; model.n];
    for fixture in &model.fixtures {
        left[fixture.home] += 1;
        left[fixture.away] += 1;
    }
    let max_wins = model
        .base
        .iter()
        .enumerate()
        .map(|(team, base)| i64::from(base.wins) + left[team] as i64)
        .max()?;
    let stride64 = max_wins + 1;
    if stride64 <= 0 || stride64 > i64::from(i32::MAX / 4) {
        return None;
    }
    let stride = stride64 as i32;
    let base_packed = model
        .base
        .iter()
        .map(|base| i32::try_from(i64::from(base.points) * stride64 + i64::from(base.wins)).ok())
        .collect::<Option<Vec<_>>>()?;
    let max_future_gain = (3 * stride64 + 1).checked_mul(model.fixtures.len() as i64)?;
    if base_packed.iter().any(|value| {
        i64::from(*value).abs().saturating_add(max_future_gain) >= i64::from(i32::MAX / 4)
    }) {
        return None;
    }
    let target = cell.team;
    let target_games = model
        .fixtures
        .iter()
        .enumerate()
        .filter(|(_, f)| f.home == target || f.away == target)
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if target_games.is_empty() {
        return None;
    }
    let final_points = i64::from(model.base[target].points) + 3 * target_games.len() as i64;
    let final_wins = i64::from(model.base[target].wins) + target_games.len() as i64;
    let cap64 = final_points * stride64 + final_wins;
    let drop64 = 2 * stride64 + 1;
    if cap64.abs() >= i64::from(i32::MAX / 4) || drop64 > i64::from(i32::MAX / 4) {
        return None;
    }
    let cap = cap64 as i32;
    let second_best = cap64 - drop64;
    if base_packed
        .iter()
        .enumerate()
        .filter(|(i, value)| *i != target && i64::from(**value) > second_best)
        .count()
        <= cell.rank
    {
        return None;
    }

    let mut target_mass = 1.0;
    let mut target_fixed = Vec::with_capacity(target_games.len());
    let mut seed_masks = model
        .fixtures
        .iter()
        .map(|f| positive_mask(f.prob))
        .collect::<Vec<_>>();
    let mut games = Vec::with_capacity(model.fixtures.len());
    for (i, fixture) in model.fixtures.iter().enumerate() {
        let target_outcome = if fixture.home == target {
            Some(2usize)
        } else if fixture.away == target {
            Some(0usize)
        } else {
            None
        };
        if let Some(outcome) = target_outcome {
            if fixture.prob[outcome] <= 0.0 {
                return None;
            }
            target_mass *= fixture.prob[outcome];
            seed_masks[i] = 1 << outcome;
            target_fixed.push((i, outcome as u8));
        }
        let hg = [0, stride, 3 * stride + 1];
        games.push(RankGame {
            index: i,
            home: fixture.home,
            away: fixture.away,
            prob: fixture.prob,
            hg,
            ag: [hg[2], hg[1], hg[0]],
        });
    }
    if !target_mass.is_finite() || target_mass <= 0.0 {
        return None;
    }
    let mut base_fixed = base_packed.clone();
    for &(i, outcome) in &target_fixed {
        let game = &games[i];
        base_fixed[game.home] += game.hg[outcome as usize];
        base_fixed[game.away] += game.ag[outcome as usize];
    }
    if base_fixed[target] != cap {
        return None;
    }
    let guaranteed = (0..model.n)
        .filter(|&team| team != target && base_fixed[team] > cap)
        .collect::<Vec<_>>();
    if guaranteed.len() > cell.rank {
        return None;
    }
    let gap = cell.rank - guaranteed.len();
    if gap > 1 {
        return None;
    }
    let possible_caps = (0..model.n)
        .filter(|&team| team != target && base_fixed[team] <= cap)
        .collect::<Vec<_>>();
    let rivals = (0..model.n).filter(|&team| team != target).collect();
    Some(Geometry {
        target,
        rank: cell.rank,
        stride,
        cap,
        target_mass,
        base_packed,
        base_fixed,
        target_fixed,
        guaranteed,
        possible_caps,
        gap,
        rivals,
        games,
        seed_masks,
    })
}

fn valid_score_suffix(model: &Model) -> bool {
    let Some(gd) = model.keys.iter().position(|key| *key == Key::Gd) else {
        return false;
    };
    model.keys.get(gd + 1) == Some(&Key::Gf)
        && model.keys[gd + 2..].iter().all(|key| {
            matches!(
                key,
                Key::Gp | Key::Aet | Key::Bias | Key::Name | Key::Random | Key::Head
            )
        })
}

fn candidate_cases(model: &Model, geometry: &Geometry) -> Vec<Option<usize>> {
    let mut cases = vec![None];
    if geometry.gap == 1 {
        cases.extend(geometry.possible_caps.iter().copied().map(Some));
    }
    let _ = model;
    cases
}

fn within_graph_limits(model: &Model, geometry: &Geometry, exception: Option<usize>) -> bool {
    let caps = geometry
        .possible_caps
        .iter()
        .filter(|&&team| Some(team) != exception)
        .copied()
        .collect::<Vec<_>>();
    if caps.len() > MAX_CAP_TEAMS {
        return false;
    }
    let mut edges = 0;
    for fixture in &model.fixtures {
        if fixture.home != geometry.target
            && fixture.away != geometry.target
            && caps.contains(&fixture.home)
            && caps.contains(&fixture.away)
        {
            edges += 1;
        }
    }
    edges <= MAX_SHARED_EDGES
}

fn strict_mass(
    geometry: &Geometry,
    cap_teams: &[usize],
    shared: &[SharedEdge],
    unary: &[UnaryTable],
    strict_rank_complete: bool,
    work: &mut DomainWork,
) -> (Option<f64>, usize, bool) {
    if !strict_rank_complete {
        return (None, 0, false);
    }
    let cap = geometry.cap - 1;
    let base = cap_teams
        .iter()
        .map(|&team| geometry.base_fixed[team])
        .collect::<Vec<_>>();
    if base.iter().any(|&value| value > cap) {
        return (Some(0.0), 0, true);
    }
    let strict_tables = unary;
    let initial = base.iter().map(|&value| cap - value).collect::<Vec<_>>();
    let node_charge = 3 * cap_teams.len() + 2;
    let mut counter = Counter {
        edges: shared,
        tables: strict_tables,
        node_limit: MAX_STRICT_NODES,
        leaf_limit: 0,
        node_charge,
        leaf_charge: 0,
        nodes: 0,
        work,
        exhausted: false,
        leaves: Vec::new(),
        mass: 0.0,
        save_leaves: false,
    };
    let mut remaining = initial;
    counter.visit(0, &mut remaining, 0, 1.0);
    if counter.exhausted {
        (None, counter.nodes, false)
    } else {
        (Some(counter.mass), counter.nodes, true)
    }
}

fn packed_gain(outcome: usize, is_home: bool, stride: i32) -> i32 {
    match (outcome, is_home) {
        (0, true) | (2, false) => 0,
        (1, _) => stride,
        (2, true) | (0, false) => 3 * stride + 1,
        _ => unreachable!(),
    }
}

fn positive_mask(probability: [f64; 3]) -> u8 {
    (0..3).fold(0, |mask, outcome| {
        if probability[outcome] > 0.0 {
            mask | (1 << outcome)
        } else {
            mask
        }
    })
}

fn sample_raw(probability: [f64; 3], rng: &mut Rng) -> usize {
    let mut draw = rng.float();
    for outcome in 0..3 {
        if draw < probability[outcome] {
            return outcome;
        }
        draw -= probability[outcome];
    }
    (0..3).rev().find(|&o| probability[o] > 0.0).unwrap_or(2)
}

fn charge(work: &mut DomainWork, amount: usize) -> bool {
    let Some(next) = work.used.checked_add(amount) else {
        return false;
    };
    if next > work.limit {
        return false;
    }
    work.used = next;
    true
}

fn declined(work: usize, reason: &'static str) -> Build {
    Build {
        plan: None,
        work,
        reason,
    }
}

fn relative_close(actual: f64, expected: f64, tolerance: f64) -> bool {
    (actual - expected).abs() <= expected.abs().max(f64::MIN_POSITIVE) * tolerance
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Request;
    use serde_json::json;

    const TOY_GAMES: [(usize, usize); 5] = [(0, 1), (0, 2), (1, 2), (1, 3), (3, 4)];
    const IDS: [i32; 5] = [100, 200, 300, 400, 500];

    fn toy_model(ids: [i32; 5], b_points: i32) -> Model {
        let request: Request = serde_json::from_value(json!({
            "id": 1,
            "phase": {"sort":"pt,w,gd,gf", "championship":{
                "point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":ids.iter().map(|id| json!({"team_id":id,"add_sub":0})).collect::<Vec<_>>(),
            "games":TOY_GAMES.iter().enumerate().map(|(i,(home,away))| json!({
                "id":i,"home_id":IDS[*home],"away_id":IDS[*away],
                "home_power":1.0,"away_power":0.9
            })).collect::<Vec<_>>()
        }))
        .unwrap();
        let mut model = Model::new(request).unwrap();
        let campaigns = [
            (6, 0, 0),
            (12, 2, 3),
            (b_points, 2, 0),
            (9, 1, 0),
            (9, 2, 0),
        ];
        for (source, id) in IDS.into_iter().enumerate() {
            let index = model.indices[&id];
            model.base[index].points = campaigns[source].0;
            model.base[index].wins = campaigns[source].1;
            model.base[index].gf = campaigns[source].2;
            model.base[index].ga = 0;
        }
        model
    }

    fn empty_exception_model() -> Model {
        let request: Request = serde_json::from_value(json!({
            "id":2,
            "phase":{"sort":"pt,w,gd,gf","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":[{"team_id":0},{"team_id":1},{"team_id":2}],
            "games":[
                {"id":1,"home_id":0,"away_id":2,"home_power":1.0,"away_power":0.9},
                {"id":2,"home_id":0,"away_id":2,"home_power":1.1,"away_power":0.8}
            ]
        })).unwrap();
        let mut model = Model::new(request).unwrap();
        model.base[0].points = 0;
        model.base[0].wins = 0;
        model.base[1].points = 6;
        model.base[1].wins = 2;
        model.base[1].gf = 10;
        model.base[1].ga = 0;
        model.base[2].points = 5;
        model.base[2].wins = 0;
        model
    }

    fn brute_masses(model: &Model, plan: &ExactCaps) -> (f64, f64, usize, usize) {
        let exception = model.indices[&plan.exceptional_id.unwrap()];
        let target_fixtures = plan
            .target_fixed
            .iter()
            .map(|(index, _)| *index)
            .collect::<Vec<_>>();
        let mut upper = 0.0;
        let mut strict = 0.0;
        let mut upper_count = 0;
        let mut strict_count = 0;
        let combinations = 3usize.pow(model.fixtures.len() as u32);
        for mut code in 0..combinations {
            let mut outcomes = vec![0usize; model.fixtures.len()];
            let mut weight = 1.0;
            let mut packed = model
                .base
                .iter()
                .map(|base| base.points * plan.stride + base.wins)
                .collect::<Vec<_>>();
            for (index, fixture) in model.fixtures.iter().enumerate() {
                outcomes[index] = code % 3;
                code /= 3;
                let outcome = outcomes[index];
                weight *= fixture.prob[outcome];
                packed[fixture.home] += packed_gain(outcome, true, plan.stride);
                packed[fixture.away] += packed_gain(outcome, false, plan.stride);
            }
            if target_fixtures.iter().any(|&index| {
                let fixture = &model.fixtures[index];
                let expected = if fixture.home == plan.target { 2 } else { 0 };
                outcomes[index] != expected
            }) || packed[plan.target] != plan.cap
                || packed[exception] <= plan.cap
            {
                continue;
            }
            if plan.cap_teams.iter().all(|&team| packed[team] <= plan.cap) {
                upper += weight;
                upper_count += 1;
                if plan.cap_teams.iter().all(|&team| packed[team] < plan.cap) {
                    strict += weight;
                    strict_count += 1;
                }
            }
        }
        (upper, strict, upper_count, strict_count)
    }

    #[test]
    fn five_team_exact_cap_mass_matches_exhaustive_wdl_and_id_permutation() {
        let mut masses = Vec::new();
        for ids in [IDS, [500, 300, 100, 200, 400]] {
            let model = toy_model(ids, 10);
            let cell = Cell {
                team: model.indices[&100],
                rank: 1,
            };
            let built = ExactCaps::build(&model, cell, 50_000_000);
            let plan = built
                .plan
                .expect("toy has one proved exceptional-above case");
            assert_eq!(plan.exceptional_id, Some(200));
            let (brute_upper, brute_strict, upper_count, strict_count) =
                brute_masses(&model, &plan);
            assert_eq!((upper_count, strict_count), (6, 4));
            assert!(
                relative_close(plan.mass(), brute_upper, 1e-12),
                "{} != {}",
                plan.mass(),
                brute_upper
            );
            assert!(relative_close(plan.guaranteed_mass(), brute_strict, 1e-12));
            assert!(plan.strict_cv);
            let sampled = plan.sample(&model, 256, 19);
            assert_eq!(sampled.samples, 256);
            assert!(sampled.weighted);
            assert_eq!(sampled.mass.to_bits(), plan.mass().to_bits());
            masses.push((plan.mass(), plan.guaranteed_mass()));
        }
        assert!(relative_close(masses[0].0, masses[1].0, 1e-12));
        assert!(relative_close(masses[0].1, masses[1].1, 1e-12));
    }

    #[test]
    fn sampler_invariant_failure_returns_incomplete_and_charged_result() {
        let model = toy_model(IDS, 10);
        let cell = Cell {
            team: model.indices[&100],
            rank: 1,
        };
        let mut plan = ExactCaps::build(&model, cell, 50_000_000)
            .plan
            .expect("toy exact cap plan");
        assert!(!plan.unary[0].games.is_empty());
        plan.unary[0].cdf[1].fill(0.0);
        let result = plan.sample(&model, 8, 31);
        assert!(result.samples < 8);
        assert!(!result.weighted);
        assert!(result.work > 0);
        assert!(result.operations.units() > 0);
    }

    #[test]
    fn rejects_when_all_wins_are_not_proven_necessary() {
        let model = toy_model(IDS, 8);
        let cell = Cell {
            team: model.indices[&100],
            rank: 1,
        };
        assert!(!ExactCaps::eligible(&model, cell));
        assert_eq!(
            ExactCaps::build(&model, cell, 50_000_000).reason,
            "structural_gate"
        );
    }

    #[test]
    fn bounded_construction_declines_without_work_allowance() {
        let model = toy_model(IDS, 10);
        let cell = Cell {
            team: model.indices[&100],
            rank: 1,
        };
        let built = ExactCaps::build(&model, cell, 0);
        assert!(built.plan.is_none());
        assert_eq!(built.reason, "work_budget");
        assert_eq!(built.work, 0);
    }

    #[test]
    fn rejects_packed_campaigns_that_could_overflow_domain_propagation() {
        let mut model = toy_model(IDS, 10);
        model.base[1].points = 180_000_000;
        let cell = Cell {
            team: model.indices[&100],
            rank: 1,
        };
        assert!(ExactCaps::build(&model, cell, 50_000_000).plan.is_none());
    }

    #[test]
    fn unique_empty_exception_gap_does_not_claim_a_strict_rank_floor() {
        let model = empty_exception_model();
        let plan = ExactCaps::build(&model, Cell { team: 0, rank: 1 }, 10_000_000)
            .plan
            .expect("empty exceptional set is the only feasible case");
        assert_eq!(plan.exceptional_id, None);
        assert_eq!(plan.guaranteed_mass(), 0.0);
        assert!(!plan.strict_cv);
    }

    #[test]
    fn chapecoense_snapshot_exact_cap_masses_match_reproducible_counter() {
        let request: Request = serde_json::from_str(include_str!(
            "../../tests/fixtures/group-16498-2026-10-18.json"
        ))
        .unwrap();
        let model = Model::new(request).unwrap();
        let cell = Cell {
            team: model.indices[&318],
            rank: 8,
        };
        let built = ExactCaps::build(&model, cell, 220_000_000);
        let plan = built
            .plan
            .expect("snapshot exact cap proof should complete");
        assert!(relative_close(plan.mass(), 6.25841402859205e-21, 1e-10));
        assert!(relative_close(
            plan.guaranteed_mass(),
            5.167576459121802e-24,
            1e-10
        ));
        assert!(plan.guaranteed_mass() < plan.mass());
    }
}
