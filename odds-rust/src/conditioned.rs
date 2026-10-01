use crate::{
    model::{Campaign, Key, Model},
    pool::{work_per_sample, Bounds},
    rng::Rng,
};
use std::{
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
};
// Packed integer DP keys only. Request strings continue using the standard hasher.
#[derive(Default)]
pub struct IntegerHasher(u64);
impl Hasher for IntegerHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = self.0.rotate_left(5) ^ u64::from(*b);
            self.0 = self.0.wrapping_mul(0x517cc1b727220a95);
        }
    }
    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x517cc1b727220a95);
    }
}
pub type IntMap<V> = HashMap<u64, V, BuildHasherDefault<IntegerHasher>>;

/// Immutable DP masses in their original hash-map iteration order. Only the
/// active construction layer needs a growing hash table; saved layers use
/// compact entries plus a u32 lookup index instead of oversized hash buckets.
pub struct ForwardLayer {
    entries: Box<[(u64, f64)]>,
    index: ForwardIndex,
}
enum ForwardIndex {
    // One-team layers fit in one point-total byte. Direct mass lookup avoids
    // hashing on the frequent point-tilt and backward-transition paths.
    Dense {
        values: Box<[f64]>,
        present: [u64; 4],
    },
    Sparse(Box<[u32]>),
}

impl ForwardLayer {
    const EMPTY: u32 = u32::MAX;

    pub(crate) fn from_map(map: IntMap<f64>) -> Self {
        // Keep this order: subsequent mass summations must remain bit-for-bit
        // identical to iteration over the original construction hash table.
        let entries: Box<[_]> = map.into_iter().collect();
        assert!(entries.len() < Self::EMPTY as usize);
        if entries.iter().all(|(key, _)| *key < 256) {
            let span = entries
                .iter()
                .map(|(key, _)| *key as usize + 1)
                .max()
                .unwrap_or(0);
            let mut values = vec![0.; span].into_boxed_slice();
            let mut present = [0; 4];
            for &(key, mass) in &entries {
                values[key as usize] = mass;
                present[key as usize / 64] |= 1 << (key % 64);
            }
            return Self {
                entries,
                index: ForwardIndex::Dense { values, present },
            };
        }
        let buckets = (entries.len() * 4 / 3 + 1).next_power_of_two();
        let mut index = vec![Self::EMPTY; buckets].into_boxed_slice();
        for (entry, &(key, _)) in entries.iter().enumerate() {
            let mut slot = Self::slot(key, buckets);
            while index[slot] != Self::EMPTY {
                slot = (slot + 1) & (buckets - 1);
            }
            index[slot] = entry as u32;
        }
        Self {
            entries,
            index: ForwardIndex::Sparse(index),
        }
    }

    #[inline]
    fn slot(key: u64, buckets: usize) -> usize {
        // Multiplicative hashing uses the high product bits so every packed
        // point-total byte participates, rather than masking the low key byte.
        (key.wrapping_mul(0x9e3779b97f4a7c15) >> 32) as usize & (buckets - 1)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u64, &f64)> {
        self.entries.iter().map(|(key, mass)| (key, mass))
    }

    #[inline]
    pub fn get(&self, key: &u64) -> Option<&f64> {
        let index = match &self.index {
            ForwardIndex::Dense { values, present } => {
                if *key >= values.len() as u64
                    || present[*key as usize / 64] & (1 << (*key % 64)) == 0
                {
                    return None;
                }
                return Some(&values[*key as usize]);
            }
            ForwardIndex::Sparse(index) => index,
        };
        let mut slot = Self::slot(*key, index.len());
        loop {
            let entry = index[slot];
            if entry == Self::EMPTY {
                return None;
            }
            let (found, mass) = &self.entries[entry as usize];
            if found == key {
                return Some(mass);
            }
            slot = (slot + 1) & (index.len() - 1);
        }
    }
}

#[derive(Clone)]
pub struct PointGame {
    pub index: usize,
    pub prob: [f64; 3],
    pub delta: [u64; 3],
}
pub struct Event {
    pub teams: Vec<usize>,
    pub games: Vec<PointGame>,
    pub forward: Vec<ForwardLayer>,
    pub terminals: Vec<(u64, f64)>,
    pub mass: f64,
}
#[inline]
fn byte(s: u64, slot: usize) -> i32 {
    ((s >> (slot * 8)) & 255) as i32
}
impl Event {
    pub fn build(
        model: &Model,
        bounds: &Bounds,
        target: usize,
        rank: usize,
        blockers: &[usize],
        limit: usize,
    ) -> Option<Self> {
        let mut teams = vec![target];
        teams.extend_from_slice(blockers);
        if teams.len() > 6 {
            return None;
        }
        let rules = &model.request.phase.championship;
        if [rules.point_loss, rules.point_draw, rules.point_win]
            .iter()
            .any(|g| *g < 0 || *g > 255)
        {
            return None;
        }
        let hg = [rules.point_loss, rules.point_draw, rules.point_win];
        let ag = [rules.point_win, rules.point_draw, rules.point_loss];
        // Each team's added points occupy one byte. Unsupported totals must
        // skip conditioning, rather than silently dropping feasible seasons.
        let max_gain = *hg.iter().max()? as usize;
        if teams.iter().any(|&t| {
            model
                .fixtures
                .iter()
                .filter(|f| f.home == t || f.away == t)
                .count()
                .saturating_mul(max_gain)
                > 255
        }) {
            return None;
        }
        let mut games = Vec::new();
        for (i, f) in model.fixtures.iter().enumerate() {
            let h = teams.iter().position(|t| *t == f.home);
            let a = teams.iter().position(|t| *t == f.away);
            if h.is_none() && a.is_none() {
                continue;
            }
            let mut delta = [0; 3];
            for o in 0..3 {
                if let Some(h) = h {
                    delta[o] += (hg[o] as u64) << (8 * h);
                }
                if let Some(a) = a {
                    delta[o] += (ag[o] as u64) << (8 * a);
                }
            }
            games.push(PointGame {
                index: i,
                prob: f.prob,
                delta,
            });
        }
        // Prefix cardinality can never exceed the number of blockers. When
        // pruning is possible, prepare byte thresholds once rather than loading
        // campaign bounds and subtracting them for every DP transition.
        let mut guaranteed_above = 0;
        let caps: Vec<_> = if rank < teams.len() - 1 {
            teams[1..]
                .iter()
                .enumerate()
                .filter_map(|(slot, &t)| {
                    let cap = i64::from(bounds.max[target]) - i64::from(bounds.current[t]);
                    if cap < 0 {
                        guaranteed_above += 1;
                        None
                    } else if cap >= 255 {
                        None
                    } else {
                        Some(((slot + 1) * 8, cap as u64))
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        // Accumulate sufficiently small point lattices by direct index. The
        // hash table still receives first insertions in the original order and
        // with its original capacity, preserving every saved layer's order.
        let mut dense_strides = Vec::new();
        let mut dense_size = 1usize;
        for slot in 0..teams.len() {
            dense_strides.push(dense_size);
            let width = games
                .iter()
                .map(|g| g.delta.iter().map(|&d| byte(d, slot)).max().unwrap())
                .sum::<i32>() as usize
                + 1;
            dense_size = dense_size.saturating_mul(width);
        }
        let dense = dense_size <= 65536;
        let index = |state: u64| -> usize {
            dense_strides
                .iter()
                .enumerate()
                .map(|(slot, &stride)| byte(state, slot) as usize * stride)
                .sum()
        };
        let mut dense_mass = vec![0.; if dense { dense_size } else { 0 }];
        let mut dense_seen = vec![0usize; dense_mass.len()];
        let mut generation = 0usize;
        let mut start = IntMap::default();
        start.insert(0, 1.);
        let mut forward = vec![ForwardLayer::from_map(start)];
        for g in &games {
            generation += 1;
            let dense_delta = if dense { g.delta.map(index) } else { [0; 3] };
            let mut next = IntMap::default();
            next.reserve((forward.last()?.len() * 3).min(limit + 1));
            for (&state, &mass) in forward.last()?.iter() {
                let dense_state = if dense { index(state) } else { 0 };
                for o in 0..3 {
                    if g.prob[o] <= 0. {
                        continue;
                    }
                    // The fixture-count maximum above guarantees every total
                    // fits its byte, including all intermediate prefixes.
                    let new = state + g.delta[o];
                    let above = guaranteed_above
                        + caps
                            .iter()
                            .filter(|&&(shift, cap)| ((new >> shift) & 255) > cap)
                            .count();
                    if above > rank {
                        continue;
                    }
                    if dense {
                        let slot = dense_state + dense_delta[o];
                        if dense_seen[slot] != generation {
                            dense_seen[slot] = generation;
                            dense_mass[slot] = 0.;
                            next.insert(new, 0.);
                        }
                        dense_mass[slot] += mass * g.prob[o];
                    } else {
                        *next.entry(new).or_default() += mass * g.prob[o];
                    }
                    // A failed event is discarded entirely. Stop as soon as
                    // its state limit is exceeded instead of finishing the layer.
                    if next.len() > limit {
                        return None;
                    }
                }
            }
            if next.len() > limit {
                return None;
            }
            if dense {
                for (&state, mass) in &mut next {
                    *mass = dense_mass[index(state)];
                }
            }
            forward.push(ForwardLayer::from_map(next));
        }
        let mut terminals = Vec::new();
        for (&state, &mass) in forward.last()?.iter() {
            if !bounds.allowed(target, rank, byte(state, 0)) {
                continue;
            }
            let target_points = bounds.current[target] + byte(state, 0);
            let mut above = 0;
            let mut below = 0;
            for (slot, t) in teams[1..].iter().enumerate() {
                let p = bounds.current[*t] + byte(state, slot + 1);
                above += usize::from(p > target_points);
                below += usize::from(p < target_points);
            }
            if above <= rank && below <= model.n - 1 - rank {
                terminals.push((state, mass));
            }
        }
        terminals.sort_unstable_by_key(|(s, _)| s.swap_bytes());
        let mut mass = 0.;
        for (_, p) in &mut terminals {
            mass += *p;
            *p = mass;
        }
        Some(Self {
            teams,
            games,
            forward,
            terminals,
            mass,
        })
    }
    pub fn point_tilt(&self, tilt: f64) -> (Vec<f64>, Vec<f64>) {
        if tilt == 0. || self.mass <= 0. {
            return (Vec::new(), Vec::new());
        }
        let probabilities: Vec<_> = self
            .terminals
            .iter()
            .enumerate()
            .map(|(i, (_, cdf))| cdf - if i == 0 { 0. } else { self.terminals[i - 1].1 })
            .collect();
        let max = self
            .terminals
            .iter()
            .zip(&probabilities)
            .filter(|(_, p)| **p > 0.)
            .map(|((s, _), _)| tilt * byte(*s, 0) as f64)
            .fold(f64::NEG_INFINITY, f64::max);
        let weights: Vec<_> = self
            .terminals
            .iter()
            .zip(&probabilities)
            .map(|((s, _), p)| p * (tilt * byte(*s, 0) as f64 - max).exp())
            .collect();
        let sum: f64 = weights.iter().sum();
        let mut cdf = Vec::new();
        let mut ratios = Vec::new();
        let mut cumulative = 0.;
        for (&p, w) in probabilities.iter().zip(weights) {
            let q = 0.02 * p / self.mass + 0.98 * w / sum;
            ratios.push(if q > 0. { p / self.mass / q } else { 0. });
            cumulative += q;
            cdf.push(cumulative);
        }
        *cdf.last_mut().unwrap() = 1.;
        (cdf, ratios)
    }
}
#[derive(Clone, Copy, Default)]
struct Transition {
    previous: [u64; 3],
    weights: [f64; 3],
    total: f64,
}
pub struct Backward<'a> {
    pub event: &'a Event,
    cache: Vec<IntMap<Transition>>,
    dense: Option<Vec<[Transition; 256]>>,
}
impl<'a> Backward<'a> {
    pub fn new(event: &'a Event) -> Self {
        let dense = if event.teams.len() == 1 {
            let mut steps = vec![[Transition::default(); 256]; event.games.len()];
            for (i, g) in event.games.iter().enumerate() {
                for (&state, _) in event.forward[i + 1].iter() {
                    let mut tr = Transition::default();
                    for o in 0..3 {
                        if state >= g.delta[o] {
                            tr.previous[o] = state - g.delta[o];
                            tr.weights[o] =
                                event.forward[i].get(&tr.previous[o]).copied().unwrap_or(0.)
                                    * g.prob[o];
                        }
                    }
                    tr.total = tr.weights.iter().sum();
                    steps[i][state as usize] = tr;
                }
            }
            Some(steps)
        } else {
            None
        };
        Self {
            event,
            cache: (0..event.games.len()).map(|_| IntMap::default()).collect(),
            dense,
        }
    }
    pub fn sample(&mut self, rng: &mut Rng, outcomes: &mut [u8]) {
        let u = rng.float() * self.event.mass;
        let terminal = self
            .event
            .terminals
            .partition_point(|(_, c)| *c < u)
            .min(self.event.terminals.len() - 1);
        self.from_terminal(rng, outcomes, terminal);
    }
    pub fn from_terminal(&mut self, rng: &mut Rng, outcomes: &mut [u8], terminal: usize) {
        let mut state = self.event.terminals[terminal].0;
        for i in (0..self.event.games.len()).rev() {
            let g = &self.event.games[i];
            let tr = if let Some(dense) = &self.dense {
                dense[i][state as usize]
            } else if let Some(tr) = self.cache[i].get(&state) {
                *tr
            } else {
                let mut tr = Transition::default();
                for o in 0..3 {
                    if (0..self.event.teams.len()).all(|s| byte(state, s) >= byte(g.delta[o], s)) {
                        tr.previous[o] = state - g.delta[o];
                        tr.weights[o] = self.event.forward[i]
                            .get(&tr.previous[o])
                            .copied()
                            .unwrap_or(0.)
                            * g.prob[o];
                    }
                }
                tr.total = tr.weights.iter().sum();
                if self.cache[i].len() < 4096 {
                    self.cache[i].insert(state, tr);
                }
                tr
            };
            let u = rng.float() * tr.total;
            let o = if u < tr.weights[0] {
                0
            } else if u < tr.weights[0] + tr.weights[1] {
                1
            } else {
                2
            };
            outcomes[g.index] = o as u8;
            state = tr.previous[o];
        }
    }
}
#[derive(Clone, Default, Debug)]
pub struct Result {
    pub mass: f64,
    pub samples: usize,
    pub hits: usize,
    pub blockers: usize,
    pub work: u64,
    pub weighted: bool,
    pub probability: f64,
    pub std_err: f64,
    pub ess: f64,
    pub max_share: f64,
    pub batch_gap: f64,
    pub witness: Option<Vec<u8>>,
    pub omitted_draws: u64,
}
impl Result {
    pub fn strict(&self) -> bool {
        self.weighted
            && self.probability > 0.
            && self.hits >= 100
            && self.ess >= 200.
            && self.std_err / self.probability <= 0.1
            && self.max_share <= 0.03
            && self.batch_gap <= 0.2
    }
    pub fn coarse(&self) -> bool {
        self.weighted
            && self.probability > 0.
            && self.hits >= 30
            && self.ess >= 8.
            && self.std_err / self.probability <= 0.35
            && self.max_share <= 0.25
            && self.batch_gap <= 1.
    }
    pub fn summarize(&mut self, sum: f64, sum2: f64, max: f64, batches: [f64; 2]) {
        if sum <= 0. || sum2 <= 0. || !sum.is_finite() {
            return;
        }
        let n = self.samples as f64;
        let mean = sum / n;
        let variance = ((sum2 / n - mean * mean) / (n - 1.)).max(0.);
        self.weighted = true;
        self.probability = self.mass * mean;
        self.std_err = self.mass * variance.sqrt();
        self.ess = sum * sum / sum2;
        self.max_share = max / sum;
        let left = batches[0] / (self.samples / 2) as f64;
        let right = batches[1] / (self.samples - self.samples / 2) as f64;
        self.batch_gap = (left - right).abs() / mean;
    }
}
pub struct ScoreContext<'a> {
    model: &'a Model,
    campaign: Vec<Campaign>,
    scores: Vec<[i32; 2]>,
    order: Vec<usize>,
}
impl<'a> ScoreContext<'a> {
    pub fn new(model: &'a Model) -> Self {
        Self {
            model,
            campaign: model.base.clone(),
            scores: model.empty_scores(),
            order: vec![0; model.n],
        }
    }
    pub fn rank(
        &mut self,
        target: usize,
        outcomes: &[u8],
        rng: &mut Rng,
        omitted: Option<&[bool]>,
    ) -> usize {
        self.campaign.copy_from_slice(&self.model.base);
        for (i, f) in self.model.fixtures.iter().enumerate() {
            let score = if omitted.is_some_and(|v| v.get(i).copied().unwrap_or(false)) {
                canonical(outcomes[i])
            } else {
                f.scores.sample(outcomes[i] as usize, rng)
            };
            self.scores[f.request_index] = score;
            self.model.add(&mut self.campaign, i, score);
        }
        self.model
            .standings(&mut self.order, &self.campaign, &self.scores, rng);
        self.order.iter().position(|t| *t == target).unwrap()
    }
}
pub fn canonical(outcome: u8) -> [i32; 2] {
    match outcome {
        0 => [0, 1],
        1 => [0, 0],
        _ => [1, 0],
    }
}
pub fn canonical_ranks(model: &Model, outcomes: &[u8]) -> Vec<usize> {
    let mut c = model.base.clone();
    let mut scores = model.empty_scores();
    let mut order = vec![0; model.n];
    for (i, f) in model.fixtures.iter().enumerate() {
        let s = canonical(outcomes[i]);
        scores[f.request_index] = s;
        model.add(&mut c, i, s);
    }
    let mut rng = Rng::new(1);
    model.standings(&mut order, &c, &scores, &mut rng);
    let mut ranks = vec![0; model.n];
    for (r, t) in order.into_iter().enumerate() {
        ranks[t] = r;
    }
    ranks
}
pub fn stride(model: &Model, stage: &str) -> i32 {
    let mode = std::env::var("RARE_POSITION_WIN_AWARE_RANK").unwrap_or("lookahead".into());
    if (mode != "1" && mode != stage)
        || model.keys.len() < 2
        || model.keys[..2] != [Key::Pt, Key::W]
    {
        return 1;
    }
    let mut maximum = 0;
    let mut left = vec![0; model.ids.len()];
    for f in &model.fixtures {
        left[f.home] += 1;
        left[f.away] += 1;
    }
    for t in 0..model.n {
        maximum = maximum.max(model.base[t].wins + left[t]);
    }
    maximum + 1
}
pub fn blockers(
    model: &Model,
    bounds: &Bounds,
    pmfs: &[Vec<(i32, f64)>],
    target: usize,
    rank: usize,
    count: usize,
) -> Vec<usize> {
    let top = rank + 1 <= model.n - rank;
    let mut chosen = Vec::new();
    let scores: Vec<_> = (0..model.n)
        .map(|t| {
            let mean =
                bounds.current[t] as f64 + pmfs[t].iter().map(|(p, m)| *p as f64 * m).sum::<f64>();
            (mean + 0.04 * bounds.current[t] as f64) * if top { 1. } else { -1. }
        })
        .collect();
    while chosen.len() < count.min(model.n - 1) {
        let mut best = None;
        let mut best_score = f64::NEG_INFINITY;
        for t in 0..model.n {
            if t == target || chosen.contains(&t) {
                continue;
            }
            let mut score = scores[t];
            if chosen.len() >= 2 {
                for f in &model.fixtures {
                    for &s in &chosen {
                        if (f.home == t && f.away == s) || (f.away == t && f.home == s) {
                            score -= 1.;
                        }
                    }
                }
            }
            if score > best_score
                || score == best_score && best.is_none_or(|b| model.ids[t] < model.ids[b])
            {
                best = Some(t);
                best_score = score;
            }
        }
        chosen.push(best.unwrap());
    }
    chosen
}
pub fn fast(
    model: &Model,
    event: &Event,
    target: usize,
    rank: usize,
    samples: usize,
    seed: i64,
) -> Result {
    let mut result = Result {
        mass: event.mass,
        ..Default::default()
    };
    if event.mass <= 0. || samples == 0 {
        return result;
    }
    let mut rng = Rng::new(seed);
    let mut backward = Backward::new(event);
    let mut outcomes = vec![0; model.fixtures.len()];
    let mut selected = vec![false; model.fixtures.len()];
    for g in &event.games {
        selected[g.index] = true;
    }
    let rules = &model.request.phase.championship;
    let stride = stride(model, "screen");
    let hg = [
        rules.point_loss * stride,
        rules.point_draw * stride,
        rules.point_win * stride + if stride > 1 { 1 } else { 0 },
    ];
    let ag = [hg[2], hg[1], hg[0]];
    // Sampling uses only these compact fields. Keep the unconditioned games
    // in fixture order so the random stream is identical, and avoid touching
    // the larger score-sampler objects or checking selection on every draw.
    let remaining: Vec<_> = model
        .fixtures
        .iter()
        .enumerate()
        .filter(|(i, _)| !selected[*i])
        .map(|(i, f)| (i, f.home, f.away, f.prob))
        .collect();
    let fixed: Vec<_> = model
        .fixtures
        .iter()
        .enumerate()
        .filter(|(i, _)| selected[*i])
        .map(|(i, f)| (i, f.home, f.away))
        .collect();
    let base: Vec<_> = model
        .base
        .iter()
        .map(|c| c.points * stride + if stride > 1 { c.wins } else { 0 })
        .collect();
    let mut points = base.clone();
    let mut score = ScoreContext::new(model);
    for _ in 0..samples {
        backward.sample(&mut rng, &mut outcomes);
        points.copy_from_slice(&base);
        for &(i, home, away) in &fixed {
            let o = outcomes[i] as usize;
            points[home] += hg[o];
            points[away] += ag[o];
        }
        for &(i, home, away, prob) in &remaining {
            let u = rng.float();
            let o = if u < prob[0] {
                0
            } else if u < prob[0] + prob[1] {
                1
            } else {
                2
            };
            outcomes[i] = o as u8;
            points[home] += hg[o];
            points[away] += ag[o];
        }
        let p = points[target];
        let above = (0..model.n)
            .filter(|t| *t != target && points[*t] > p)
            .count();
        let tied = (0..model.n)
            .filter(|t| *t != target && points[*t] == p)
            .count();
        let mut hit = above <= rank && above + tied >= rank;
        if hit && tied > 0 {
            hit = score.rank(target, &outcomes, &mut rng, None) == rank;
        }
        if hit {
            result.hits += 1;
            if result.witness.is_none() {
                result.witness = Some(outcomes.clone());
            }
        }
    }
    result.samples = samples;
    result.work = samples as u64 * work_per_sample(model);
    result
}

#[cfg(test)]
mod forward_layer_tests {
    use super::{ForwardLayer, IntMap};

    #[test]
    fn frozen_layers_preserve_iteration_bits_and_packed_key_lookups() {
        for count in [0, 1, 3, 31, 1000, 12000, 120000] {
            let mut map = IntMap::default();
            map.reserve(count * 3);
            for i in 0..count as u64 {
                // Exercise low and high point-total bytes, including key zero.
                map.insert((i << 32) | (i << 8) | (i % 251), (i as f64 + 1.) * 1e-200);
            }
            map.insert(u64::MAX, -0.0);
            let expected: Vec<_> = map.iter().map(|(&k, &v)| (k, v.to_bits())).collect();
            let layer = ForwardLayer::from_map(map);
            assert_eq!(layer.len(), expected.len());
            assert_eq!(
                layer
                    .iter()
                    .map(|(&k, &v)| (k, v.to_bits()))
                    .collect::<Vec<_>>(),
                expected
            );
            for (key, bits) in expected {
                assert_eq!(layer.get(&key).unwrap().to_bits(), bits);
            }
            for key in [1, 252, 1 << 63, u64::MAX - 1] {
                assert_eq!(layer.get(&key), None);
            }
        }
        let empty = ForwardLayer::from_map(IntMap::default());
        assert!(empty.is_empty());
        assert_eq!(empty.get(&0), None);
    }

    #[test]
    fn dense_layers_preserve_zero_masses_and_absent_point_totals() {
        let map: IntMap<f64> = IntMap::from_iter([(0, -0.0), (3, 0.25), (255, 1e-200)]);
        let expected: Vec<_> = map.iter().map(|(&k, &v)| (k, v.to_bits())).collect();
        let layer = ForwardLayer::from_map(map);
        assert!(matches!(layer.index, super::ForwardIndex::Dense { .. }));
        assert_eq!(
            layer
                .iter()
                .map(|(&k, &v)| (k, v.to_bits()))
                .collect::<Vec<_>>(),
            expected
        );
        for (key, bits) in expected {
            assert_eq!(layer.get(&key).unwrap().to_bits(), bits);
        }
        for key in [1, 254, 256, u64::MAX] {
            assert_eq!(layer.get(&key), None);
        }
    }

    #[test]
    fn collisions_find_every_mass_and_terminate_on_missing_keys() {
        let mut map = IntMap::default();
        for key in 0..10000 {
            map.insert(key, key as f64);
        }
        let layer = ForwardLayer::from_map(map);
        let super::ForwardIndex::Sparse(index) = &layer.index else {
            panic!("wide keys need the sparse index");
        };
        // At this load, multiple entries must share initial slots.
        assert!((0..10000).any(|key| {
            let entry = index[ForwardLayer::slot(key, index.len())];
            layer.entries[entry as usize].0 != key
        }));
        for key in 0..10000 {
            assert_eq!(layer.get(&key), Some(&(key as f64)));
        }
        for key in 10000..11000 {
            assert_eq!(layer.get(&key), None);
        }
    }
}

#[cfg(test)]
mod transition_accumulation_tests {
    use super::*;
    #[test]
    fn direct_accumulation_preserves_hash_order_and_every_mass_bit() {
        for rules in [[0, 1, 3], [1, 2, 4], [0, 0, 2]] {
            let games: Vec<_> = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]
                .into_iter()
                .enumerate()
                .map(|(i, (h, a))| {
                    serde_json::json!({
                        "id":i,"home_id":h,"away_id":a,"home_power":1.23,"away_power":0.87
                    })
                })
                .collect();
            let request = serde_json::from_value(serde_json::json!({
                "id":0,"phase":{"sort":"pt,w,gd,gf","championship":{
                    "point_loss":rules[0],"point_draw":rules[1],"point_win":rules[2]
                }},"team_groups":[{"team_id":0},{"team_id":1},{"team_id":2},{"team_id":3}],
                "games":games
            }))
            .unwrap();
            let model = Model::new(request).unwrap();
            let bounds = Bounds::new(&model);
            for target in 0..4 {
                let rivals: Vec<_> = (0..4).filter(|&t| t != target).collect();
                for rank in 0..4 {
                    for count in 0..=3 {
                        let event =
                            Event::build(&model, &bounds, target, rank, &rivals[..count], 20000)
                                .unwrap();
                        let mut original = IntMap::default();
                        original.insert(0, 1f64);
                        for (step, game) in event.games.iter().enumerate() {
                            let mut next = IntMap::<f64>::default();
                            next.reserve((original.len() * 3).min(20001));
                            for (&state, &mass) in &original {
                                for o in 0..3 {
                                    if game.prob[o] <= 0. {
                                        continue;
                                    }
                                    let new = state + game.delta[o];
                                    let above = event.teams[1..]
                                        .iter()
                                        .enumerate()
                                        .filter(|(slot, t)| {
                                            bounds.current[**t] + byte(new, slot + 1)
                                                > bounds.max[target]
                                        })
                                        .count();
                                    if above > rank {
                                        continue;
                                    }
                                    *next.entry(new).or_default() += mass * game.prob[o];
                                }
                            }
                            assert_eq!(
                                event.forward[step + 1].iter().map(|(&s, &m)| (s, m.to_bits())).collect::<Vec<_>>(),
                                next.iter().map(|(&s, &m)| (s, m.to_bits())).collect::<Vec<_>>(),
                                "rules={rules:?}, target={target}, rank={rank}, count={count}, step={step}"
                            );
                            original = next;
                        }
                    }
                }
            }
        }
    }
}
