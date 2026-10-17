//! Cached exact suffix lower bounds for the guided/native likelihood ratio.

use super::{widen_if_invalid, ResidualSource};
use crate::joint::{outcome_gain, Plan};
use crate::law::{categorical_probabilities, Interval, Law};
use golaberto_odds::model::Model;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct SuffixOptions {
    pub state_cap: usize,
    pub max_remaining: usize,
    pub probe_nodes: usize,
    pub probe_ms: u64,
    pub budget_fraction: f64,
}

impl Default for SuffixOptions {
    fn default() -> Self {
        Self {
            state_cap: 1_000_000,
            max_remaining: 24,
            probe_nodes: 25_000,
            probe_ms: 25,
            budget_fraction: 0.15,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SuffixDiagnostics {
    pub enabled: bool,
    pub calls: usize,
    pub visits: usize,
    pub cache_hits: usize,
    pub entries: usize,
    pub completed_rows: usize,
    pub positive_bounds: usize,
    pub impossible: usize,
    pub unknown: usize,
    pub terminals: usize,
    pub elapsed: Duration,
    pub cache_capacity_hit: bool,
    pub time_budget_hit: bool,
    pub probe_node_limit_hit: bool,
    pub suffix_feasibility_prunes: usize,
    pub guide_zero_prunes: usize,
    pub elapsed_ms: f64,
    pub guide_prunes: usize,
    pub tightened_branches: usize,
    pub min_positive_g: Option<f64>,
    pub max_positive_g: Option<f64>,
}

#[derive(Clone, Debug)]
pub(super) enum Bound {
    Unknown,
    Impossible,
    Feasible(f64),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct Key {
    plan: usize,
    pos: usize,
    node: Option<usize>,
    totals: Vec<i32>,
}

#[derive(Clone)]
struct Meta {
    active: Vec<Vec<usize>>,
    min_gain: Vec<Vec<i32>>,
    max_gain: Vec<Vec<i32>>,
}

pub(super) struct Cache {
    rows: HashMap<Key, Bound>,
    by_node: HashSet<(usize, usize, Option<usize>)>,
    metadata: HashMap<usize, Meta>,
    options: SuffixOptions,
    budget: Duration,
    spent: Duration,
    diagnostics: SuffixDiagnostics,
}

impl Cache {
    pub fn new(
        options: SuffixOptions,
        started: Instant,
        deadline: Instant,
    ) -> Result<Self, String> {
        if options.state_cap == 0
            || !options.budget_fraction.is_finite()
            || !(0.0..=1.0).contains(&options.budget_fraction)
        {
            return Err("invalid suffix certificate options".into());
        }
        let remaining = deadline.saturating_duration_since(started);
        let budget = Duration::from_secs_f64(remaining.as_secs_f64() * options.budget_fraction);
        Ok(Self {
            rows: HashMap::new(),
            by_node: HashSet::new(),
            metadata: HashMap::new(),
            options,
            budget,
            spent: Duration::ZERO,
            diagnostics: SuffixDiagnostics {
                enabled: true,
                ..Default::default()
            },
        })
    }

    pub fn diagnostics(&self) -> SuffixDiagnostics {
        let mut out = self.diagnostics.clone();
        out.entries = self.rows.len();
        out
    }

    pub fn max_remaining(&self) -> usize {
        self.options.max_remaining
    }

    pub fn record_guide_zero(&mut self) {
        self.diagnostics.guide_zero_prunes += 1;
    }
    pub fn record_guide_prune(&mut self) {
        self.diagnostics.guide_prunes += 1;
    }

    pub fn record_tightened(&mut self) {
        self.diagnostics.tightened_branches += 1;
    }

    pub fn finish(&mut self) {
        self.diagnostics.elapsed = self.spent;
        self.diagnostics.elapsed_ms = self.spent.as_secs_f64() * 1000.0;
    }

    fn key(
        &mut self,
        plan: &Plan,
        model: &Model,
        laws: &[Law],
        pos: usize,
        node: Option<usize>,
        totals: &[i32],
    ) -> Result<Option<Key>, String> {
        let meta = self
            .metadata
            .entry(plan as *const Plan as usize)
            .or_insert_with(|| make_meta(plan, model, laws));
        if pos > plan.order.len() {
            return Ok(None);
        }
        // Finalized teams are checked before the cache lookup so entries can
        // safely omit their exact totals from the key.
        for team in 0..model.n {
            if !meta.active[pos].contains(&team)
                && (totals[team] < plan.lower[team] || totals[team] > plan.upper[team])
            {
                return Ok(None);
            }
            if totals[team].saturating_add(meta.min_gain[pos][team]) > plan.upper[team]
                || totals[team].saturating_add(meta.max_gain[pos][team]) < plan.lower[team]
            {
                return Ok(None);
            }
        }
        let active_totals = meta.active[pos].iter().map(|&t| totals[t]).collect();
        Ok(Some(Key {
            plan: plan as *const Plan as usize,
            pos,
            node,
            totals: active_totals,
        }))
    }

    pub fn lookup(
        &mut self,
        source: &ResidualSource<'_>,
        pos: usize,
        node: Option<usize>,
        totals: &[i32],
        model: &Model,
        laws: &[Law],
    ) -> Result<Bound, String> {
        let plan = source.graph.plan();
        let group = (plan as *const Plan as usize, pos, node);
        if !self.by_node.contains(&group) {
            return Ok(Bound::Unknown);
        }
        let Some(key) = self.key(plan, model, laws, pos, node, totals)? else {
            self.diagnostics.suffix_feasibility_prunes += 1;
            return Ok(Bound::Impossible);
        };
        if let Some(v) = self.rows.get(&key) {
            self.diagnostics.cache_hits += 1;
            return Ok(v.clone());
        }
        Ok(Bound::Unknown)
    }

    pub fn probe(
        &mut self,
        source: &ResidualSource<'_>,
        pos: usize,
        node: Option<usize>,
        totals: &[i32],
        model: &Model,
        laws: &[Law],
        local_deadline: Instant,
    ) -> Result<Bound, String> {
        self.diagnostics.calls += 1;
        let plan = source.graph.plan();
        let allowance = self.budget.saturating_sub(self.spent);
        if allowance.is_zero() {
            self.diagnostics.time_budget_hit = true;
            self.diagnostics.unknown += 1;
            return Ok(Bound::Unknown);
        }
        let deadline = local_deadline.min(Instant::now() + allowance);
        let mut visited = 0usize;
        let probe_started = Instant::now();
        let result = self.solve(
            plan,
            pos,
            node,
            totals.to_vec(),
            model,
            laws,
            deadline,
            &mut visited,
        )?;
        self.spent += probe_started.elapsed();
        self.diagnostics.elapsed = self.spent;
        self.diagnostics.elapsed_ms = self.spent.as_secs_f64() * 1000.0;
        self.diagnostics.visits += visited;
        match result {
            Bound::Unknown => self.diagnostics.unknown += 1,
            Bound::Impossible => {
                self.diagnostics.impossible += 1;
                self.diagnostics.suffix_feasibility_prunes += 1;
            }
            Bound::Feasible(g) if g > 0.0 => {
                self.diagnostics.positive_bounds += 1;
                self.diagnostics.min_positive_g =
                    Some(self.diagnostics.min_positive_g.map_or(g, |x| x.min(g)));
                self.diagnostics.max_positive_g =
                    Some(self.diagnostics.max_positive_g.map_or(g, |x| x.max(g)));
            }
            Bound::Feasible(_) => {}
        }
        Ok(result)
    }

    fn solve(
        &mut self,
        plan: &Plan,
        pos: usize,
        node: Option<usize>,
        totals: Vec<i32>,
        model: &Model,
        laws: &[Law],
        deadline: Instant,
        visited: &mut usize,
    ) -> Result<Bound, String> {
        if Instant::now() >= deadline {
            self.diagnostics.time_budget_hit = true;
            return Ok(Bound::Unknown);
        }
        let Some(key) = self.key(plan, model, laws, pos, node, &totals)? else {
            self.diagnostics.suffix_feasibility_prunes += 1;
            return Ok(Bound::Impossible);
        };
        if let Some(v) = self.rows.get(&key) {
            self.diagnostics.cache_hits += 1;
            return Ok(v.clone());
        }
        if self.rows.len() >= self.options.state_cap {
            self.diagnostics.cache_capacity_hit = true;
            return Ok(Bound::Unknown);
        }
        if *visited >= self.options.probe_nodes {
            self.diagnostics.probe_node_limit_hit = true;
            return Ok(Bound::Unknown);
        }
        *visited += 1;
        if pos == plan.order.len() {
            self.diagnostics.terminals += 1;
            let mut feasible = true;
            for team in 0..model.n {
                if totals[team] < plan.lower[team] || totals[team] > plan.upper[team] {
                    feasible = false;
                    break;
                }
            }
            let row = if feasible {
                Bound::Feasible(1.0)
            } else {
                Bound::Impossible
            };
            self.rows.insert(key, row.clone());
            self.by_node
                .insert((plan as *const Plan as usize, pos, node));
            self.diagnostics.completed_rows += 1;
            return Ok(row);
        }
        let fi = plan.order[pos];
        let (base, edges) = if let Some(id) = node {
            let Some(n) = plan.nodes.get(id) else {
                return Ok(Bound::Unknown);
            };
            if n.terminal == Some(false) {
                return Ok(Bound::Impossible);
            }
            if n.terminal == Some(true) {
                let feasible =
                    (0..model.n).all(|t| totals[t] >= plan.lower[t] && totals[t] <= plan.upper[t]);
                let row = if feasible {
                    Bound::Feasible(1.0)
                } else {
                    Bound::Impossible
                };
                self.rows.insert(key, row.clone());
                self.by_node
                    .insert((plan as *const Plan as usize, pos, node));
                self.diagnostics.completed_rows += 1;
                return Ok(row);
            }
            (n.weights.to_vec(), n.edges.clone())
        } else {
            (
                (0..3)
                    .map(|o| {
                        if plan.masks[fi] & (1 << o) != 0 {
                            laws[fi].outcomes[o].mid()
                        } else {
                            0.0
                        }
                    })
                    .collect(),
                Vec::new(),
            )
        };
        let q = match categorical_probabilities(&base) {
            Ok(x) => x,
            Err(_) => return Ok(Bound::Unknown),
        };
        let guided = match plan.guided_weights(model, laws, fi, pos, &totals, &base) {
            Ok(x) => x,
            Err(_) => return Ok(Bound::Unknown),
        };
        let gq = match categorical_probabilities(&guided) {
            Ok(x) => x,
            Err(_) => return Ok(Bound::Unknown),
        };
        let mut min_g = f64::INFINITY;
        let mut any_feasible = false;
        let mut unknown = false;
        for o in 0..3 {
            if q[o].hi <= 0.0 || laws[fi].outcomes[o].hi <= 0.0 {
                continue;
            }
            let edge = edges.iter().find(|e| e.outcome as usize == o);
            if node.is_some() && edge.is_none() {
                continue;
            }
            let (hg, ag) = outcome_gain(model, fi, o, plan.stride);
            let f = &model.fixtures[fi];
            let mut next_totals = totals.clone();
            next_totals[f.home] = next_totals[f.home].saturating_add(hg);
            next_totals[f.away] = next_totals[f.away].saturating_add(ag);
            let next_node = if node.is_some() {
                edge.and_then(|e| e.child)
            } else {
                None
            };
            let child = self.solve(
                plan,
                pos + 1,
                next_node,
                next_totals,
                model,
                laws,
                deadline,
                visited,
            )?;
            match child {
                Bound::Impossible => {}
                Bound::Unknown => unknown = true,
                Bound::Feasible(suffix) => {
                    any_feasible = true;
                    let ratio = widen_if_invalid(gq[o].div(q[o]));
                    let lower = Interval::point(ratio.lo).mul(Interval::point(suffix)).lo;
                    min_g = min_g.min(lower);
                }
            }
            if Instant::now() >= deadline {
                self.diagnostics.time_budget_hit = true;
                unknown = true;
                break;
            }
            if *visited >= self.options.probe_nodes {
                self.diagnostics.cache_capacity_hit = true;
                unknown = true;
                break;
            }
        }
        let row = if unknown {
            Bound::Unknown
        } else if !any_feasible {
            Bound::Impossible
        } else if min_g.is_finite() && min_g >= 0.0 {
            Bound::Feasible(min_g)
        } else {
            Bound::Unknown
        };
        if !matches!(row, Bound::Unknown) {
            self.rows.insert(key, row.clone());
            self.by_node
                .insert((plan as *const Plan as usize, pos, node));
            self.diagnostics.completed_rows += 1;
        }
        Ok(row)
    }
}

fn make_meta(plan: &Plan, model: &Model, laws: &[Law]) -> Meta {
    let nf = plan.order.len();
    let mut active = vec![Vec::new(); nf + 1];
    let mut min_gain = vec![vec![0i32; model.n]; nf + 1];
    let mut max_gain = vec![vec![0i32; model.n]; nf + 1];
    for pos in (0..nf).rev() {
        min_gain[pos] = min_gain[pos + 1].clone();
        max_gain[pos] = max_gain[pos + 1].clone();
        let fi = plan.order[pos];
        let f = &model.fixtures[fi];
        for team in [f.home, f.away] {
            let mut lo = i32::MAX;
            let mut hi = i32::MIN;
            for o in 0..3 {
                if plan.masks[fi] & (1 << o) == 0 || laws[fi].outcomes[o].hi <= 0.0 {
                    continue;
                }
                let (hg, ag) = outcome_gain(model, fi, o, plan.stride);
                let gain = if team == f.home { hg } else { ag };
                lo = lo.min(gain);
                hi = hi.max(gain);
            }
            if lo != i32::MAX {
                min_gain[pos][team] = min_gain[pos][team].saturating_add(lo);
                max_gain[pos][team] = max_gain[pos][team].saturating_add(hi);
            }
        }
    }
    let mut touched = vec![false; model.n];
    for pos in (0..nf).rev() {
        let f = &model.fixtures[plan.order[pos]];
        touched[f.home] = true;
        touched[f.away] = true;
        active[pos] = touched
            .iter()
            .enumerate()
            .filter_map(|(t, &v)| v.then_some(t))
            .collect();
    }
    Meta {
        active,
        min_gain,
        max_gain,
    }
}

pub(super) fn minimum(
    source: &ResidualSource<'_>,
    pos: usize,
    node: Option<usize>,
    totals: &[i32],
    model: &Model,
    laws: &[Law],
    cache: &mut Cache,
    deadline: Instant,
) -> Result<Bound, String> {
    let known = cache.lookup(source, pos, node, totals, model, laws)?;
    if !matches!(known, Bound::Unknown) {
        return Ok(known);
    }
    let remaining = source.graph.plan().order.len().saturating_sub(pos);
    if remaining > cache.options.max_remaining {
        return Ok(Bound::Unknown);
    }
    if Instant::now() >= deadline {
        cache.diagnostics.time_budget_hit = true;
        return Ok(Bound::Unknown);
    }
    if cache.spent >= cache.budget {
        cache.diagnostics.time_budget_hit = true;
        return Ok(Bound::Unknown);
    }
    let local = (Instant::now() + Duration::from_millis(cache.options.probe_ms)).min(deadline);
    cache.probe(source, pos, node, totals, model, laws, local)
}

pub(super) fn cached(
    source: &ResidualSource<'_>,
    pos: usize,
    node: Option<usize>,
    totals: &[i32],
    model: &Model,
    laws: &[Law],
    cache: &mut Cache,
) -> Result<Bound, String> {
    cache.lookup(source, pos, node, totals, model, laws)
}
