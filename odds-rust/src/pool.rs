use crate::{
    logging::RequestLog,
    model::{Key, Model},
    rng::{derive, Rng},
};
use serde::Serialize;
use serde_json::json;
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

#[derive(Clone, Serialize, Default)]
pub struct Estimate {
    pub probability: f64,
    pub std_err: f64,
    pub samples: usize,
    pub hits: usize,
    pub ess: f64,
    pub mean_weight: f64,
    pub work_spent: u64,
    pub available: bool,
    pub meets_precision_goal: bool,
    pub relative_se: Option<f64>,
    pub max_event_weight_share: f64,
    pub zero_hit_upper_95: f64,
    pub design: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub reachability: String,
    #[serde(skip_serializing_if = "is_zero")]
    pub conditional_mass: f64,
    #[serde(skip_serializing_if = "is_zero_usize")]
    pub conditional_samples: usize,
    #[serde(skip_serializing_if = "is_zero_usize")]
    pub conditional_hits: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<crate::confidence::Certificate>,
}
fn is_zero(v: &f64) -> bool {
    *v == 0.
}
fn is_zero_usize(v: &usize) -> bool {
    *v == 0
}
#[derive(Clone)]
pub struct Scout {
    pub samples: usize,
    pub ranks: Vec<usize>,
    pub joint: Vec<usize>,
    pub span: usize,
    pub offset: i32,
}
impl Scout {
    pub fn new(model: &Model, samples: usize) -> Self {
        let c = &model.request.phase.championship;
        let lo = c.point_win.min(c.point_draw).min(c.point_loss);
        let hi = c.point_win.max(c.point_draw).max(c.point_loss);
        let mut counts = vec![0; model.ids.len()];
        for f in &model.fixtures {
            counts[f.home] += 1;
            counts[f.away] += 1;
        }
        let max = *counts.iter().max().unwrap_or(&0);
        let offset = (max * lo).min(0);
        let span = (max * hi - offset + 1).max(1) as usize;
        Self {
            samples,
            ranks: vec![0; model.n * model.n],
            joint: vec![0; model.n * span * model.n],
            span,
            offset,
        }
    }
    #[inline]
    pub fn row(&self, t: usize, added: i32, n: usize) -> &[usize] {
        let p = added - self.offset;
        if p < 0 || p as usize >= self.span {
            return &[];
        }
        let start = (t * self.span + p as usize) * n;
        &self.joint[start..start + n]
    }
    pub fn subtract(&self, part: &Self) -> Self {
        Self {
            samples: self.samples - part.samples,
            span: self.span,
            offset: self.offset,
            ranks: self
                .ranks
                .iter()
                .zip(&part.ranks)
                .map(|(a, b)| a - b)
                .collect(),
            joint: self
                .joint
                .iter()
                .zip(&part.joint)
                .map(|(a, b)| a - b)
                .collect(),
        }
    }
}
pub fn scout(model: &Model, samples: usize, seed: i64) -> Scout {
    let n = model.n;
    let mut scout = Scout::new(model, samples);
    let mut rng = Rng::new(seed);
    let mut campaign = model.base.clone();
    let mut scores = model.empty_scores();
    let mut order = vec![0; n];
    for _ in 0..samples {
        campaign.copy_from_slice(&model.base);
        for (i, f) in model.fixtures.iter().enumerate() {
            let s = [
                f.home_sampler.sample(&mut rng),
                f.away_sampler.sample(&mut rng),
            ];
            scores[f.request_index] = s;
            model.add(&mut campaign, i, s);
        }
        model.standings(&mut order, &campaign, &scores, &mut rng);
        for (r, &t) in order.iter().enumerate() {
            scout.ranks[t * n + r] += 1;
            let p = (campaign[t].points - model.base[t].points - scout.offset) as usize;
            scout.joint[(t * scout.span + p) * n + r] += 1;
        }
    }
    scout
}
pub fn batched_scout(
    model: &Model,
    samples: usize,
    seed: i64,
    workers: usize,
) -> (Scout, Vec<Scout>) {
    let mut rng = Rng::new(seed);
    let seeds: Vec<_> = (0..10).map(|_| rng.int63()).collect();
    let next = AtomicUsize::new(0);
    let mut results = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..workers.clamp(1, 4) {
            let next = &next;
            let seeds = &seeds;
            handles.push(scope.spawn(move || {
                let mut local = Vec::new();
                loop {
                    let batch = next.fetch_add(1, Ordering::Relaxed);
                    if batch >= 10 {
                        break;
                    }
                    let count = (batch + 1) * samples / 10 - batch * samples / 10;
                    local.push((batch, scout(model, count, seeds[batch])));
                }
                local
            }));
        }
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    results.sort_by_key(|(i, _)| *i);
    let parts: Vec<_> = results.into_iter().map(|(_, r)| r).collect();
    let mut total = Scout::new(model, samples);
    for part in &parts {
        for (a, b) in total.ranks.iter_mut().zip(&part.ranks) {
            *a += b;
        }
        for (a, b) in total.joint.iter_mut().zip(&part.joint) {
            *a += b;
        }
    }
    (total, parts)
}
pub struct Bounds {
    pub current: Vec<i32>,
    pub min: Vec<i32>,
    pub max: Vec<i32>,
}
impl Bounds {
    pub fn new(model: &Model) -> Self {
        let n = model.n;
        let current: Vec<_> = model.base[..n].iter().map(|c| c.points).collect();
        let mut min = current.clone();
        let mut max = current.clone();
        let c = &model.request.phase.championship;
        let w = if c.point_win < 0 { 3 } else { c.point_win };
        let d = if c.point_draw < 0 { 1 } else { c.point_draw };
        let l = if c.point_loss < 0 { 0 } else { c.point_loss };
        for f in &model.fixtures {
            for t in [f.home, f.away] {
                if t < n {
                    min[t] += w.min(d).min(l);
                    max[t] += w.max(d).max(l);
                }
            }
        }
        Self { current, min, max }
    }
    #[inline]
    pub fn allowed(&self, t: usize, r: usize, added: i32) -> bool {
        let p = self.current[t] + added;
        let mut above = 0;
        let mut below = 0;
        for i in 0..self.current.len() {
            if i != t {
                above += usize::from(self.min[i] > p);
                below += usize::from(self.max[i] < p);
            }
        }
        r >= above && r <= self.current.len() - 1 - below
    }
    pub fn hard_bound(&self, t: usize, r: usize, pmf: &[(i32, f64)]) -> (f64, Vec<i32>) {
        let mut p = 0.;
        let mut totals = Vec::new();
        for &(added, mass) in pmf {
            if mass > 0. && self.allowed(t, r, added) {
                p += mass;
                totals.push(added);
            }
        }
        (p, totals)
    }
}
pub fn point_pmfs(model: &Model) -> Option<Vec<Vec<(i32, f64)>>> {
    if model.keys.first() != Some(&Key::Pt) {
        return None;
    }
    if model
        .fixtures
        .iter()
        .any(|f| f.prob.iter().any(|p| !p.is_finite()))
    {
        return None;
    }
    let c = &model.request.phase.championship;
    if c.point_win < 0 || c.point_draw < 0 || c.point_loss < 0 {
        return None;
    }
    let mut pmfs = Vec::new();
    for t in 0..model.n {
        let mut pmf = BTreeMap::from([(0, 1.)]);
        let mut remaining = 0;
        for f in &model.fixtures {
            if f.home != t && f.away != t {
                continue;
            }
            remaining += 1;
            if remaining > 39 {
                return None;
            }
            let gains = if f.home == t {
                [c.point_loss, c.point_draw, c.point_win]
            } else {
                [c.point_win, c.point_draw, c.point_loss]
            };
            let mut next = BTreeMap::new();
            for (added, p) in pmf {
                for o in 0..3 {
                    if f.prob[o] > 0. {
                        *next.entry(added + gains[o]).or_insert(0.) += p * f.prob[o];
                    }
                }
            }
            pmf = next;
        }
        pmfs.push(pmf.into_iter().collect());
    }
    Some(pmfs)
}
pub fn normalize(matrix: &mut [f64], n: usize) {
    for t in 0..n {
        let row = &mut matrix[t * n..(t + 1) * n];
        let sum: f64 = row.iter().sum();
        if sum > 0. {
            for p in row {
                *p /= sum;
            }
        }
    }
    for r in 0..n {
        let sum: f64 = (0..n).map(|t| matrix[t * n + r]).sum();
        if sum > 0. {
            for t in 0..n {
                matrix[t * n + r] /= sum;
            }
        }
    }
}
pub fn valid(matrix: &[f64], n: usize) -> bool {
    if matrix.iter().any(|p| !p.is_finite() || *p < 0. || *p > 1.) {
        return false;
    }
    (0..n).all(|t| (matrix[t * n..(t + 1) * n].iter().sum::<f64>() - 1.).abs() <= 1e-12)
        && (0..n).all(|r| ((0..n).map(|t| matrix[t * n + r]).sum::<f64>() - 1.).abs() <= 1e-12)
}

// A sparse balancing workspace keeps active cells in their original row-major
// order. That preserves the additions performed by normalize: row values are
// still added from left to right, and each column's values are added from top
// to bottom. Zeros can be omitted because adding either signed zero does not
// change a sum containing nonnegative values.
struct SparseBalance {
    indices: Vec<usize>,
    columns: Vec<usize>,
    row_offsets: Vec<usize>,
    column_sums: Vec<f64>,
    values: Vec<f64>,
}

impl SparseBalance {
    fn new(matrix: &[f64], n: usize) -> Option<Self> {
        let len = n.checked_mul(n)?;
        if n < 32 || matrix.len() != len || matrix.iter().any(|p| !p.is_finite() || *p < 0.) {
            return None;
        }

        let nonzero_count = matrix.iter().filter(|&&value| value != 0.).count();
        if nonzero_count > len / 4 {
            return None;
        }

        let mut indices = Vec::with_capacity(nonzero_count);
        let mut columns = Vec::with_capacity(nonzero_count);
        let mut row_offsets = Vec::with_capacity(n + 1);
        let mut values = Vec::with_capacity(nonzero_count);
        row_offsets.push(0);
        for row in 0..n {
            for column in 0..n {
                let index = row * n + column;
                if matrix[index] != 0. {
                    indices.push(index);
                    columns.push(column);
                    values.push(matrix[index]);
                }
            }
            row_offsets.push(indices.len());
        }

        Some(Self {
            indices,
            columns,
            row_offsets,
            column_sums: vec![0.; n],
            values,
        })
    }

    fn normalize(&mut self, n: usize) {
        for row in 0..n {
            let start = self.row_offsets[row];
            let end = self.row_offsets[row + 1];
            let sum: f64 = self.values[start..end].iter().sum();
            if sum > 0. {
                for value in &mut self.values[start..end] {
                    *value /= sum;
                }
            }
        }

        self.column_sums.fill(0.);
        for (&value, &column) in self.values.iter().zip(&self.columns) {
            self.column_sums[column] += value;
        }
        for (value, &column) in self.values.iter_mut().zip(&self.columns) {
            let sum = self.column_sums[column];
            if sum > 0. {
                *value /= sum;
            }
        }
    }

    fn copy_to_dense(&self, matrix: &mut [f64]) {
        for (&index, &value) in self.indices.iter().zip(&self.values) {
            matrix[index] = value;
        }
    }
}

pub fn balance(matrix: &mut [f64], n: usize) -> bool {
    if let Some(mut sparse) = SparseBalance::new(matrix, n) {
        for i in 0..20000 {
            sparse.normalize(n);
            if i % 100 == 0 {
                sparse.copy_to_dense(matrix);
                if valid(matrix, n) {
                    return true;
                }
            }
        }
        sparse.copy_to_dense(matrix);
        valid(matrix, n)
    } else {
        for i in 0..20000 {
            normalize(matrix, n);
            if i % 100 == 0 && valid(matrix, n) {
                return true;
            }
        }
        valid(matrix, n)
    }
}

/// Conservatively repairs an approximately doubly-stochastic candidate by
/// moving probability mass additively within bounded per-cell adjustments.
/// Exact zeros stay locked at zero, and very small positive cells stay fixed.
/// The factor bounds are numerical adjustment guards, not confidence bounds;
/// upstream estimator and uncertainty semantics remain unchanged.
pub(crate) fn balance_additive(matrix: &[f64], n: usize) -> Option<Vec<f64>> {
    let len = n.checked_mul(n)?;
    if n == 0 || matrix.len() != len || matrix.iter().any(|p| !p.is_finite() || *p < 0. || *p > 1.)
    {
        return None;
    }
    if valid(matrix, n) {
        return Some(matrix.to_vec());
    }
    // Dense residual flow is intended for the small finalist matrices where
    // multiplicative scaling stalls; bound both graph size and augmentation work.
    if n > 64 {
        return None;
    }

    #[derive(Clone)]
    struct Edge {
        to: usize,
        rev: usize,
        cap: f64,
        cost: f64,
    }
    fn add_edge(graph: &mut [Vec<Edge>], from: usize, to: usize, cap: f64, cost: f64) -> usize {
        let forward = graph[from].len();
        let reverse = graph[to].len();
        graph[from].push(Edge {
            to,
            rev: reverse,
            cap,
            cost,
        });
        graph[to].push(Edge {
            to: from,
            rev: forward,
            cap: 0.,
            cost: -cost,
        });
        forward
    }

    let source = 2 * n;
    let sink = source + 1;
    let mut graph = vec![Vec::<Edge>::new(); sink + 1];
    let mut balances = vec![0.; 2 * n];
    for row in 0..n {
        let sum: f64 = matrix[row * n..(row + 1) * n].iter().sum();
        balances[row] = 1. - sum;
    }
    for col in 0..n {
        let sum: f64 = (0..n).map(|row| matrix[row * n + col]).sum();
        balances[n + col] = sum - 1.;
    }

    let mut operations = Vec::new();
    for row in 0..n {
        for col in 0..n {
            let index = row * n + col;
            let value = matrix[index];
            if value == 0. || value <= 1e-15 || value * 0.5 == 0. {
                continue;
            }
            let lower = value * 0.5;
            let upper = (value * 2.).min(1.);
            let cost = 1. - value.ln();
            let add_cap = upper - value;
            if add_cap > 0. {
                let edge = add_edge(&mut graph, row, n + col, add_cap, cost);
                operations.push((index, row, edge, add_cap, 1.));
            }
            let remove_cap = value - lower;
            if remove_cap > 0. {
                let edge = add_edge(&mut graph, n + col, row, remove_cap, cost);
                operations.push((index, n + col, edge, remove_cap, -1.));
            }
        }
    }

    let mut required = 0.;
    for (node, &balance) in balances.iter().enumerate() {
        if balance > 1e-15 {
            add_edge(&mut graph, source, node, balance, 0.);
            required += balance;
        } else if balance < -1e-15 {
            add_edge(&mut graph, node, sink, -balance, 0.);
        }
    }
    if !required.is_finite() || required == 0. {
        return None;
    }

    let mut sent = 0.;
    let node_count = graph.len();
    let mut augmentations = 0usize;
    let mut edge_visits = 0usize;
    while required - sent > 1e-15 {
        augmentations += 1;
        if augmentations > 20_000 {
            return None;
        }
        let mut distance = vec![f64::INFINITY; node_count];
        let mut previous = vec![None::<(usize, usize)>; node_count];
        distance[source] = 0.;
        for _ in 0..node_count {
            let mut changed = false;
            for from in 0..node_count {
                if !distance[from].is_finite() {
                    continue;
                }
                for (edge_index, edge) in graph[from].iter().enumerate() {
                    edge_visits += 1;
                    if edge_visits > 8_000_000 {
                        return None;
                    }
                    if edge.cap <= 1e-15 {
                        continue;
                    }
                    let next = distance[from] + edge.cost;
                    if next < distance[edge.to] - 1e-14 {
                        distance[edge.to] = next;
                        previous[edge.to] = Some((from, edge_index));
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        if previous[sink].is_none() {
            return None;
        }
        let mut amount = required - sent;
        let mut at = sink;
        let mut hops = 0usize;
        while at != source {
            hops += 1;
            if hops > node_count {
                return None;
            }
            let (from, edge) = previous[at]?;
            amount = amount.min(graph[from][edge].cap);
            at = from;
        }
        if !amount.is_finite() || amount <= 1e-15 {
            return None;
        }
        at = sink;
        hops = 0;
        while at != source {
            hops += 1;
            if hops > node_count {
                return None;
            }
            let (from, edge_index) = previous[at]?;
            let to = graph[from][edge_index].to;
            let reverse = graph[from][edge_index].rev;
            graph[from][edge_index].cap -= amount;
            graph[to][reverse].cap += amount;
            at = from;
        }
        sent += amount;
    }

    let mut result = matrix.to_vec();
    for (index, from, edge, original_cap, sign) in operations {
        let used = (original_cap - graph[from][edge].cap).max(0.);
        result[index] += sign * used;
    }
    if valid(&result, n)
        && result.iter().zip(matrix).all(|(after, before)| {
            if *before == 0. || *before <= 1e-15 || *before * 0.5 == 0. {
                after.to_bits() == before.to_bits()
            } else {
                let lower = *before * 0.5;
                let upper = (*before * 2.).min(1.);
                let slack = 4. * f64::EPSILON * before;
                *after > 0. && *after + slack >= lower && *after <= upper + slack
            }
        })
    {
        Some(result)
    } else {
        None
    }
}
pub fn matched(
    model: &Model,
    scout: &Scout,
    pmfs: &[Vec<(i32, f64)>],
    bounds: &Bounds,
) -> Vec<f64> {
    let n = model.n;
    let means: Vec<f64> = pmfs
        .iter()
        .enumerate()
        .map(|(t, p)| bounds.current[t] as f64 + p.iter().map(|(s, p)| *s as f64 * p).sum::<f64>())
        .collect();
    let weights: Vec<f64> = (0..n * n)
        .map(|i| (-(means[i / n] - means[i % n]).abs() / 3.).exp())
        .collect();
    let mut raw = vec![0.; n * n];
    for t in 0..n {
        let mut matched = vec![0.; n];
        let mut shrunk = vec![0.; n];
        for &(added, mass) in &pmfs[t] {
            if mass <= 0. {
                continue;
            }
            let final_points = bounds.current[t] + added;
            let mut group = vec![0; n];
            let mut weighted = vec![0.; n];
            let mut group_total = 0;
            let mut weight_total = 0.;
            for donor in 0..n {
                let row = scout.row(donor, final_points - bounds.current[donor], n);
                let count: usize = row.iter().sum();
                if count == 0 {
                    continue;
                }
                let w = weights[t * n + donor];
                group_total += count;
                weight_total += count as f64 * w;
                for (r, &hits) in row.iter().enumerate() {
                    group[r] += hits;
                    weighted[r] += hits as f64 * w;
                }
            }
            if group_total == 0 {
                continue;
            }
            let own = scout.row(t, added, n);
            let own_count: usize = own.iter().sum();
            for r in 0..n {
                if !bounds.allowed(t, r, added) {
                    continue;
                }
                if weight_total > 0. {
                    matched[r] += mass * weighted[r] / weight_total;
                }
                let hits = own.get(r).copied().unwrap_or(0);
                shrunk[r] += mass * (hits as f64 + 5. * group[r] as f64 / group_total as f64)
                    / (own_count + 5) as f64;
            }
        }
        for r in 0..n {
            raw[t * n + r] = if scout.ranks[t * n + r] <= 10 {
                matched[r]
            } else {
                shrunk[r]
            };
        }
    }
    for _ in 0..100 {
        normalize(&mut raw, n);
    }
    raw
}

fn jackknife_matrices(
    model: &Model,
    scout: &Scout,
    parts: &[Scout],
    pmfs: &[Vec<(i32, f64)>],
    bounds: &Bounds,
    workers: usize,
) -> (Vec<Vec<f64>>, bool) {
    let workers = workers.clamp(1, 4);
    if workers == 1 {
        let mut matrices = Vec::with_capacity(parts.len());
        for part in parts {
            let mut matrix = matched(model, &scout.subtract(part), pmfs, bounds);
            if !balance(&mut matrix, model.n) {
                return (matrices, false);
            }
            matrices.push(matrix);
        }
        return (matrices, true);
    }

    let next = AtomicUsize::new(0);
    let results = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            let next = &next;
            handles.push(scope.spawn(move || {
                let mut local = Vec::new();
                loop {
                    let batch = next.fetch_add(1, Ordering::Relaxed);
                    if batch >= parts.len() {
                        break;
                    }
                    let mut matrix = matched(model, &scout.subtract(&parts[batch]), pmfs, bounds);
                    let balanced = balance(&mut matrix, model.n);
                    local.push((batch, matrix, balanced));
                }
                local
            }));
        }
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    ordered_jackknife_prefix(results)
}

fn ordered_jackknife_prefix(mut results: Vec<(usize, Vec<f64>, bool)>) -> (Vec<Vec<f64>>, bool) {
    results.sort_by_key(|(batch, _, _)| *batch);
    let mut matrices = Vec::with_capacity(results.len());
    for (_, matrix, balanced) in results {
        if !balanced {
            return (matrices, false);
        }
        matrices.push(matrix);
    }
    (matrices, true)
}

pub fn zero_upper(samples: usize) -> f64 {
    if samples == 0 {
        1.
    } else {
        -((0.05f64).ln() / samples as f64).exp_m1()
    }
}
pub fn work_per_sample(model: &Model) -> u64 {
    if model.fixtures.is_empty() {
        (model.n + 1) as u64
    } else {
        (model.fixtures.len() + model.n) as u64
    }
}
pub fn production(model: &Model, seed: i64, workers: usize) -> Vec<Estimate> {
    production_logged(model, seed, workers, None)
}
pub fn production_logged(
    model: &Model,
    seed: i64,
    workers: usize,
    log: Option<&RequestLog>,
) -> Vec<Estimate> {
    let samples = 100000;
    let phase = Instant::now();
    let (scout, parts) = batched_scout(model, samples, derive(seed, "pooled-point-scout"), workers);
    if model.confidence.enabled() {
        let native_alpha = model.confidence.native_alpha();
        for index in 0..model.n * model.n {
            if let Ok(interval) =
                crate::confidence::binomial_kl_interval(scout.ranks[index], samples, native_alpha)
            {
                model.confidence.record(
                    index,
                    crate::confidence::Evidence {
                        interval,
                        method: "native_mc".into(),
                        alpha: native_alpha,
                        samples,
                        contribution_cap: None,
                    },
                );
            }
        }
        for (index, evidence) in crate::confidence::matched::own_points_evidence(model, &scout) {
            model.confidence.record(index, evidence);
        }
    }
    if let Some(log) = log {
        log.stage(
            "pool.mc",
            phase,
            json!({"samples":samples,"batches":10,"workers":workers,
            "work":samples as u64 * work_per_sample(model),
            "stream_seed":derive(seed,"pooled-point-scout")}),
        );
    }
    let phase = Instant::now();
    let mut estimates = Vec::new();
    let work = work_per_sample(model) * samples as u64;
    let bounds = Bounds::new(model);
    let pmfs = point_pmfs(model);
    let mut fallback_reason = if pmfs.is_none() {
        Some("unsupported_point_pmfs")
    } else {
        None
    };
    if let Some(log) = log {
        log.stage(
            "pool.point_pmfs",
            phase,
            json!({"supported":pmfs.is_some()}),
        );
    }
    let phase = Instant::now();
    let mut matrix = pmfs
        .as_ref()
        .map(|pmfs| matched(model, &scout, pmfs, &bounds));
    if let Some(m) = &mut matrix {
        if !balance(m, model.n) {
            matrix = None;
            fallback_reason = Some("invalid_balanced_matrix");
        }
    }
    if let Some(log) = log {
        log.stage(
            "pool.matched_matrix",
            phase,
            json!({"matched":matrix.is_some()}),
        );
    }
    let phase = Instant::now();
    let leaveout = if let (Some(pmfs), Some(_)) = (&pmfs, &matrix) {
        let (matrices, balanced) =
            jackknife_matrices(model, &scout, &parts, pmfs, &bounds, workers);
        if !balanced {
            matrix = None;
            fallback_reason = Some("invalid_jackknife_matrix");
        }
        matrices
    } else {
        Vec::new()
    };
    if let Some(log) = log {
        log.stage("pool.jackknife", phase, json!({"batches":leaveout.len()}));
        if let Some(reason) = fallback_reason {
            log.event(
                "rust_odds_fallback",
                json!({"design":"plain_mc","reason":reason}),
            );
        }
    }
    for t in 0..model.n {
        for r in 0..model.n {
            let index = t * model.n + r;
            let hits = scout.ranks[index];
            let (p, se, design) = if let Some(m) = &matrix {
                let p = m[index];
                let mean = leaveout.iter().map(|m| m[index]).sum::<f64>() / 10.;
                let se = (0.9
                    * leaveout
                        .iter()
                        .map(|m| (m[index] - mean).powi(2))
                        .sum::<f64>())
                .sqrt();
                (p, se, "matched_point_pool")
            } else {
                let p = hits as f64 / samples as f64;
                (p, (p * (1. - p) / (samples - 1) as f64).sqrt(), "plain_mc")
            };
            let mut upper = if hits == 0 { zero_upper(samples) } else { 0. };
            if hits == 0 {
                if let Some(pmfs) = pmfs.as_ref().filter(|_| matrix.is_some()) {
                    upper = upper.min(bounds.hard_bound(t, r, &pmfs[t]).0);
                }
            }
            estimates.push(Estimate {
                probability: p,
                std_err: se,
                samples,
                hits,
                ess: if matrix.is_none() { hits as f64 } else { 0. },
                meets_precision_goal: matrix.is_none() && hits >= 10 && p > 0. && se / p <= 0.5,
                max_event_weight_share: if matrix.is_none() && hits > 0 {
                    1. / hits as f64
                } else {
                    0.
                },
                mean_weight: 1.,
                work_spent: work,
                available: true,
                relative_se: if p > 0. { Some(se / p) } else { None },
                zero_hit_upper_95: upper,
                design: design.into(),
                ..Default::default()
            });
        }
    }
    estimates
}

#[cfg(test)]
mod tests {
    use super::{balance_additive, ordered_jackknife_prefix, valid};

    #[test]
    fn ordered_jackknife_results_keep_only_the_successful_batch_prefix() {
        let (matrices, balanced) = ordered_jackknife_prefix(vec![
            (2, vec![2.], true),
            (1, vec![1.], false),
            (0, vec![0.], true),
        ]);
        assert!(!balanced);
        assert_eq!(matrices, vec![vec![0.]]);
    }

    #[test]
    fn additive_balance_repairs_small_ill_conditioned_matrix() {
        let matrix = vec![1. - 1e-6, 1e-6, 2e-6, 1. - 2e-6];
        let mut multiplicative = matrix.clone();
        assert!(!super::balance(&mut multiplicative, 2));
        let balanced = balance_additive(&matrix, 2).expect("bounded additive repair");
        assert!(valid(&balanced, 2));
        assert!(balanced.iter().all(|p| (0.0..=1.0).contains(p)));
        for (&before, &after) in matrix.iter().zip(&balanced) {
            assert!(after >= before * 0.5 && after <= (before * 2.).min(1.));
            assert!(after > 0.);
        }
    }

    #[test]
    fn additive_balance_leaves_already_valid_matrix_bitwise_unchanged() {
        let matrix = vec![0.8, 0.2, 0.2, 0.8];
        let balanced = balance_additive(&matrix, 2).unwrap();
        assert_eq!(
            balanced
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            matrix
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn additive_balance_keeps_zeros_and_fails_atomically_without_support() {
        let matrix = vec![1., 0., 0., 0.];
        assert_eq!(balance_additive(&matrix, 2), None);
        assert_eq!(matrix, vec![1., 0., 0., 0.]);
        let incompatible = vec![1. - 1e-6, 1e-6, 1e-9, 1. - 1e-9];
        assert_eq!(balance_additive(&incompatible, 2), None);
    }

    #[test]
    fn additive_balance_locks_tiny_cells_and_checks_bad_inputs() {
        let tiny = vec![
            1. - 3e-6,
            3e-6,
            1e-40,
            1e-6,
            1. - 4e-6,
            3e-6,
            2e-6,
            2e-6,
            1. - 3e-6,
        ];
        let balanced = balance_additive(&tiny, 3).expect("repair while retaining tiny mass");
        assert_eq!(balanced[2].to_bits(), tiny[2].to_bits());
        assert!(valid(&balanced, 3));
        assert_eq!(balance_additive(&[f64::NAN, 0., 0., 1.], 2), None);
        assert_eq!(balance_additive(&[-0.1, 0., 0., 1.], 2), None);
        assert_eq!(balance_additive(&[0., 1.], 2), None);
    }

    #[test]
    fn additive_balance_repairs_nontrivial_near_deterministic_matrix() {
        let matrix = vec![
            1. - 3e-6,
            3e-6,
            0.,
            1e-6,
            1. - 4e-6,
            3e-6,
            2e-6,
            2e-6,
            1. - 3e-6,
        ];
        let balanced = balance_additive(&matrix, 3).expect("bounded repair");
        assert!(valid(&balanced, 3));
        assert_eq!(balanced[2].to_bits(), 0f64.to_bits());
        assert!(matrix.iter().zip(&balanced).all(|(before, after)| {
            if *before == 0. {
                *after == 0.
            } else {
                *after >= before * 0.5 && *after <= (before * 2.).min(1.)
            }
        }));
    }
}
