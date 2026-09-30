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
pub fn balance(matrix: &mut [f64], n: usize) -> bool {
    for i in 0..20000 {
        normalize(matrix, n);
        if i % 100 == 0 && valid(matrix, n) {
            return true;
        }
    }
    valid(matrix, n)
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
        let mut matrices = Vec::new();
        for part in parts {
            let mut m = matched(model, &scout.subtract(&part), pmfs, &bounds);
            if !balance(&mut m, model.n) {
                matrix = None;
                fallback_reason = Some("invalid_jackknife_matrix");
                break;
            }
            matrices.push(m);
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
