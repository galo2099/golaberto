use crate::{
    model::{Campaign, Key, Model},
    pool::{work_per_sample, Bounds},
    rng::Rng,
};
use std::{
    collections::{BTreeMap, HashMap},
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
        Self::build_impl(model, bounds, target, rank, blockers, limit, None)
    }

    /// Build the same point event using explicit fixture WDL probabilities.
    /// These probabilities describe necessary point events; callers remain
    /// responsible for any final standings predicate that makes events disjoint.
    pub(crate) fn build_with_probabilities(
        model: &Model,
        bounds: &Bounds,
        target: usize,
        rank: usize,
        blockers: &[usize],
        limit: usize,
        probabilities: &[[f64; 3]],
    ) -> Option<Self> {
        if probabilities.len() != model.fixtures.len()
            || probabilities.iter().any(|row| {
                row.iter().any(|p| !p.is_finite() || *p < 0.)
                    || (row.iter().sum::<f64>() - 1.).abs() > 1e-9
            })
        {
            return None;
        }
        Self::build_impl(
            model,
            bounds,
            target,
            rank,
            blockers,
            limit,
            Some(probabilities),
        )
    }

    fn build_impl(
        model: &Model,
        bounds: &Bounds,
        target: usize,
        rank: usize,
        blockers: &[usize],
        limit: usize,
        probabilities: Option<&[[f64; 3]]>,
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
                prob: probabilities.map_or(f.prob, |p| p[i]),
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
    pub operations: crate::rare_tail::budget::Operations,
    pub weighted: bool,
    pub probability: f64,
    pub std_err: f64,
    pub ess: f64,
    pub max_share: f64,
    pub batch_gap: f64,
    pub witness: Option<Vec<u8>>,
    pub omitted_draws: u64,
    pub score_integral: ScoreIntegralStats,
    pub score_integral_snapshot: Option<std::sync::Arc<ScoreIntegralSnapshot>>,
}
#[derive(Clone, Default, Debug)]
pub struct ScoreIntegralStats {
    pub calls: usize,
    pub positive: usize,
    pub fallbacks: usize,
    pub cache_builds: usize,
    pub cache_hits: usize,
    pub prep_work: usize,
    pub transitions: usize,
    pub endpoint_scans: usize,
    pub backward_work: usize,
    pub query_work: usize,
    pub query_work_reserved: usize,
    pub index_cells: usize,
    pub index_build_work: usize,
    pub cache_paths: usize,
    pub min_integrated_fixtures: Option<usize>,
    pub max_integrated_fixtures: usize,
    pub cohort_size_hist: BTreeMap<String, usize>,
    pub cohort_calls: usize,
    pub cohort_uses: usize,
    pub cohort_shared_sampled: usize,
    pub cohort_private_eligible: usize,
    pub cohort_private_integrated: usize,
    pub cohort_cache_paths: usize,
    pub cohort_cache_cells: usize,
    pub cohort_private_by_team: BTreeMap<String, (usize, usize)>,
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
    score_integral_preparation_work: usize,
    score_integral_last_query_work: usize,
    score_integral_last_query_reserved: usize,
    score_integral_last_index_build_work: usize,
    score_integral_last_endpoint_scans: usize,
    cohort_gd_cache: ScoreIntegralCache,
    cohort_gd_cache_cells: usize,
    cohort_gd_preparation_work: usize,
    cohort_gd_last_size: usize,
    cohort_gd_last_shared: usize,
    cohort_gd_last_eligible: usize,
    cohort_gd_last_integrated: usize,
    cohort_gd_last_team_counts: Vec<(i32, usize, usize)>,
    cohort_gd_last_strict_mass: f64,
    cohort_gd_last_residual_mass: f64,
    cohort_gd_last_residual_rank_hit: bool,
}
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct ScoreIntegralKey {
    target: usize,
    caps: crate::score_integral::BuildCaps,
    eligible: Vec<(usize, u8)>,
}
type ScoreIntegralCache =
    std::collections::HashMap<ScoreIntegralKey, std::sync::Arc<CachedScoreIntegral>>;
#[derive(Clone, Debug)]
pub struct ScoreIntegralSnapshot {
    model_identity: std::sync::Arc<()>,
    cohort_gd_entries: ScoreIntegralCache,
    cohort_gd_cells: usize,
}
#[derive(Debug)]
struct CachedScoreIntegral {
    kernels: Vec<crate::score_integral::JointKernel>,
    dp: crate::score_integral::PrefixDp,
    index: crate::score_integral::EndpointIndex,
    index_build_work: usize,
    endpoint_mass_scan: usize,
}

fn relative_fixture_score(
    fixture: &crate::model::Fixture,
    target: usize,
    score: [i32; 2],
    canonical: [i32; 2],
) -> crate::score_integral::ScorePair {
    let team_delta = |team: usize, score: [i32; 2]| -> (i32, i32) {
        if team == fixture.home {
            (score[0] - score[1], score[0])
        } else if team == fixture.away {
            (score[1] - score[0], score[1])
        } else {
            (0, 0)
        }
    };
    let (target_gd, target_gf) = team_delta(target, score);
    let (canonical_target_gd, canonical_target_gf) = team_delta(target, canonical);
    crate::score_integral::ScorePair {
        gd: target_gd - canonical_target_gd,
        gf: target_gf - canonical_target_gf,
        mass: 0.0,
    }
}

impl<'a> ScoreContext<'a> {
    pub fn new(model: &'a Model) -> Self {
        Self::with_score_integral_snapshot(model, None)
    }
    pub(crate) fn with_score_integral_snapshot(
        model: &'a Model,
        snapshot: Option<std::sync::Arc<ScoreIntegralSnapshot>>,
    ) -> Self {
        Self::with_score_integral_snapshot_and_cap(
            model,
            snapshot,
            crate::score_integral::MAX_TOTAL_CELLS,
        )
    }
    pub(crate) fn with_score_integral_snapshot_and_cap(
        model: &'a Model,
        snapshot: Option<std::sync::Arc<ScoreIntegralSnapshot>>,
        max_cache_cells: usize,
    ) -> Self {
        Self::with_score_integral_snapshot_and_caps(
            model,
            snapshot,
            max_cache_cells,
            crate::score_integral::MAX_COHORT_GD_PATHS,
        )
    }
    pub(crate) fn with_score_integral_snapshot_and_caps(
        model: &'a Model,
        snapshot: Option<std::sync::Arc<ScoreIntegralSnapshot>>,
        max_cache_cells: usize,
        max_cache_paths: usize,
    ) -> Self {
        let mut cohort_gd_cache = std::collections::HashMap::new();
        let mut cohort_gd_cache_cells = 0;
        if let Some(snapshot) = snapshot.as_ref() {
            let identity = model.cache_identity();
            if std::sync::Arc::ptr_eq(&snapshot.model_identity, &identity)
                && snapshot.cohort_gd_entries.len() <= max_cache_paths
                && snapshot.cohort_gd_cells <= max_cache_cells
            {
                cohort_gd_cache.clone_from(&snapshot.cohort_gd_entries);
                cohort_gd_cache_cells = snapshot.cohort_gd_cells;
            }
        }
        Self {
            model,
            campaign: model.base.clone(),
            scores: model.empty_scores(),
            order: vec![0; model.n],
            score_integral_preparation_work: 0,
            score_integral_last_query_work: 0,
            score_integral_last_query_reserved: 0,
            score_integral_last_index_build_work: 0,
            score_integral_last_endpoint_scans: 0,
            cohort_gd_cache,
            cohort_gd_cache_cells,
            cohort_gd_preparation_work: 0,
            cohort_gd_last_size: 0,
            cohort_gd_last_shared: 0,
            cohort_gd_last_eligible: 0,
            cohort_gd_last_integrated: 0,
            cohort_gd_last_team_counts: Vec::new(),
            cohort_gd_last_strict_mass: 0.0,
            cohort_gd_last_residual_mass: 0.0,
            cohort_gd_last_residual_rank_hit: false,
        }
    }
    pub(crate) fn score_integral_snapshot(&self) -> Option<std::sync::Arc<ScoreIntegralSnapshot>> {
        (!self.cohort_gd_cache.is_empty()).then(|| {
            std::sync::Arc::new(ScoreIntegralSnapshot {
                model_identity: self.model.cache_identity(),
                cohort_gd_entries: self.cohort_gd_cache.clone(),
                cohort_gd_cells: self.cohort_gd_cache_cells,
            })
        })
    }
    pub(crate) fn score_integral_preparation_work(&self) -> usize {
        self.score_integral_preparation_work
            .saturating_add(self.cohort_gd_preparation_work)
    }
    pub(crate) fn score_integral_last_query_work(&self) -> usize {
        self.score_integral_last_query_work
    }
    pub(crate) fn score_integral_last_query_reserved(&self) -> usize {
        self.score_integral_last_query_reserved
    }
    pub(crate) fn score_integral_last_index_build_work(&self) -> usize {
        self.score_integral_last_index_build_work
    }
    pub(crate) fn score_integral_last_endpoint_scans(&self) -> usize {
        self.score_integral_last_endpoint_scans
    }
    pub(crate) fn score_integral_index_cells(&self) -> usize {
        self.cohort_gd_cache
            .values()
            .map(|cached| cached.index.retained_cells())
            .sum()
    }
    pub(crate) fn score_integral_cache_paths(&self) -> usize {
        self.cohort_gd_cache.len()
    }
    pub(crate) fn score_integral_cohort_stats(&self) -> (usize, usize, usize, usize) {
        (
            self.cohort_gd_last_size,
            self.cohort_gd_last_shared,
            self.cohort_gd_last_eligible,
            self.cohort_gd_last_integrated,
        )
    }
    pub(crate) fn cohort_gd_cache_stats(&self) -> (usize, usize) {
        (self.cohort_gd_cache.len(), self.cohort_gd_cache_cells)
    }
    pub(crate) fn cohort_gd_team_counts(&self) -> &[(i32, usize, usize)] {
        &self.cohort_gd_last_team_counts
    }
    pub(crate) fn cohort_gd_projection_stats(&self) -> (f64, f64, bool) {
        (
            self.cohort_gd_last_strict_mass,
            self.cohort_gd_last_residual_mass,
            self.cohort_gd_last_residual_rank_hit,
        )
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

    /// Integrate the complete tied points/wins cohort over private fixture GD
    /// prefixes. Cohort-cohort fixtures stay native and are sampled once;
    /// each cohort member's disjoint private prefix is projected independently.
    pub(crate) fn fraction_cohort_gd(
        &mut self,
        target: usize,
        rank: usize,
        outcomes: &[u8],
        rng: &mut Rng,
        operations: &mut crate::rare_tail::budget::Operations,
        remaining_work: usize,
        caps: crate::score_integral::BuildCaps,
    ) -> Option<(f64, usize, usize, usize, usize, usize, bool)> {
        self.fraction_cohort_gd_inner(
            target,
            rank,
            outcomes,
            rng,
            operations,
            remaining_work,
            None,
            false,
            caps,
        )
    }

    /// Bounded-query form used by the sampler's shared work pool. Prefix
    /// preparation may populate reusable caches, but the guard runs before
    /// any score RNG is consumed and before a scoreline is drawn.
    pub(crate) fn fraction_cohort_gd_bounded(
        &mut self,
        target: usize,
        rank: usize,
        outcomes: &[u8],
        rng: &mut Rng,
        operations: &mut crate::rare_tail::budget::Operations,
        remaining_prep: usize,
        query_allowance: usize,
        caps: crate::score_integral::BuildCaps,
    ) -> Option<(f64, usize, usize, usize, usize, usize, bool)> {
        self.fraction_cohort_gd_inner(
            target,
            rank,
            outcomes,
            rng,
            operations,
            remaining_prep,
            Some(query_allowance),
            false,
            caps,
        )
    }

    /// Bounded-query form backed by one shared preparation/query allowance.
    /// Preparation is admitted first, then the residual query reserve must
    /// fit in the allowance left by that preparation.
    pub(crate) fn fraction_cohort_gd_bounded_total(
        &mut self,
        target: usize,
        rank: usize,
        outcomes: &[u8],
        rng: &mut Rng,
        operations: &mut crate::rare_tail::budget::Operations,
        remaining_prep: usize,
        remaining_total_work: usize,
        caps: crate::score_integral::BuildCaps,
    ) -> Option<(f64, usize, usize, usize, usize, usize, bool)> {
        self.fraction_cohort_gd_inner(
            target,
            rank,
            outcomes,
            rng,
            operations,
            remaining_prep.min(remaining_total_work),
            Some(remaining_total_work),
            true,
            caps,
        )
    }

    fn fraction_cohort_gd_inner(
        &mut self,
        target: usize,
        rank: usize,
        outcomes: &[u8],
        rng: &mut Rng,
        operations: &mut crate::rare_tail::budget::Operations,
        remaining_work: usize,
        query_allowance: Option<usize>,
        query_allowance_includes_prep: bool,
        caps: crate::score_integral::BuildCaps,
    ) -> Option<(f64, usize, usize, usize, usize, usize, bool)> {
        use crate::model::Key;
        use crate::score_integral::{JointKernel, PrefixDp, ScorePair};
        self.cohort_gd_last_size = 0;
        self.cohort_gd_last_shared = 0;
        self.cohort_gd_last_eligible = 0;
        self.cohort_gd_last_integrated = 0;
        self.cohort_gd_last_team_counts.clear();
        self.cohort_gd_last_strict_mass = 0.0;
        self.cohort_gd_last_residual_mass = 0.0;
        self.cohort_gd_last_residual_rank_hit = false;
        self.score_integral_last_query_work = 0;
        self.score_integral_last_query_reserved = 0;
        self.score_integral_last_index_build_work = 0;
        self.score_integral_last_endpoint_scans = 0;
        let model = self.model;
        if outcomes.len() != model.fixtures.len()
            || target >= model.n
            || !crate::score_integral::supported(model)
        {
            return None;
        }
        let gd_at = model.keys.iter().position(|key| *key == Key::Gd)?;
        if model.keys.get(gd_at + 1) != Some(&Key::Gf) {
            return None;
        }
        let prefix = &model.keys[..gd_at];
        if prefix != [Key::Pt] && prefix != [Key::Pt, Key::W] {
            return None;
        }
        if model.keys[gd_at + 2..].iter().any(|key| {
            !matches!(
                key,
                Key::Gp | Key::Aet | Key::Bias | Key::Name | Key::Random | Key::Head
            )
        }) {
            return None;
        }
        if outcomes.iter().any(|outcome| *outcome > 2) {
            return None;
        }

        // Points and wins are determined by the common WDL path. This fixes
        // the full tied cohort before any conditional goal score is sampled.
        let mut cohort_stats = model.base.clone();
        for (index, &outcome) in outcomes.iter().enumerate() {
            model.add(&mut cohort_stats, index, canonical(outcome));
        }
        let use_wins = prefix.contains(&Key::W);
        let anchor = (cohort_stats[target].points, cohort_stats[target].wins);
        let tied: Vec<bool> = cohort_stats
            .iter()
            .map(|campaign| campaign.points == anchor.0 && (!use_wins || campaign.wins == anchor.1))
            .collect();
        let cohort: Vec<usize> = (0..model.n).filter(|&team| tied[team]).collect();
        self.cohort_gd_last_size = cohort.len();
        let classes_above = cohort_stats
            .iter()
            .filter(|campaign| {
                campaign.points > anchor.0
                    || (use_wins && campaign.points == anchor.0 && campaign.wins > anchor.1)
            })
            .count();
        // Charge construction of the canonical points/wins state and the
        // tied-cohort/rank scans even when a singleton or out-of-range rank
        // returns before GD endpoint work starts.
        let cohort_classification_work = outcomes.len().saturating_add(model.n.saturating_mul(4));
        operations.guidance = operations
            .guidance
            .saturating_add(cohort_classification_work as u64);
        if cohort.len() == 1 {
            return Some((f64::from(classes_above == rank), 0, 0, 0, 0, 0, true));
        }
        if rank < classes_above || rank >= classes_above + cohort.len() {
            return Some((0.0, 0, 0, 0, 0, 0, true));
        }

        let mut private_by_team = vec![Vec::<(usize, u8)>::new(); model.n];
        let mut shared_fixtures = 0usize;
        for (index, fixture) in model.fixtures.iter().enumerate() {
            let home_tied = tied[fixture.home];
            let away_tied = tied[fixture.away];
            match (home_tied, away_tied) {
                (true, false) => private_by_team[fixture.home].push((index, outcomes[index])),
                (false, true) => private_by_team[fixture.away].push((index, outcomes[index])),
                (true, true) => shared_fixtures += 1,
                (false, false) => {}
            }
        }
        self.cohort_gd_last_shared = shared_fixtures;
        self.cohort_gd_last_eligible = cohort.iter().map(|&team| private_by_team[team].len()).sum();

        // Prepare every cohort member before consuming score RNG. When any
        // prefix is bounded by support/work/cell caps, the remaining private
        // fixtures remain native for that member; the shared score path is
        // still sampled only once after all preflight work.
        let mut prepared_by_team = vec![None::<std::sync::Arc<CachedScoreIntegral>>; model.n];
        let mut all_cache_hits = true;
        let mut prep_delta = 0usize;
        let mut transition_delta = 0usize;
        let mut query_work = model.fixtures.len();
        for &team in &cohort {
            let eligible = &private_by_team[team];
            if eligible.is_empty() {
                continue;
            }
            let cache_key = ScoreIntegralKey {
                target: team,
                caps,
                eligible: eligible.to_vec(),
            };
            if let Some(cached) = self.cohort_gd_cache.get(&cache_key).cloned() {
                prepared_by_team[team] = Some(cached);
                continue;
            }
            all_cache_hits = false;
            if self.cohort_gd_cache.len() >= crate::score_integral::MAX_COHORT_GD_PATHS {
                continue;
            }
            let cells_left = caps
                .total_cells
                .min(crate::score_integral::MAX_TOTAL_CELLS)
                .saturating_sub(self.cohort_gd_cache_cells);
            if cells_left == 0 {
                continue;
            }
            let mut candidate_kernels = Vec::<JointKernel>::new();
            let mut kernel_work = 0usize;
            let team_work_limit = remaining_work.min(caps.work).saturating_sub(prep_delta);
            let mut remaining = team_work_limit;
            for &(fixture_index, outcome) in eligible {
                if candidate_kernels.len() >= crate::score_integral::MAX_KERNELS {
                    break;
                }
                if kernel_work > remaining {
                    break;
                }
                let fixture = &model.fixtures[fixture_index];
                let scan_budget = remaining.saturating_sub(kernel_work);
                if scan_budget == 0 {
                    break;
                }
                // Leave room for the support scan, grouping pass, and worst-
                // case materialization before consuming the support iterator.
                // The exact charge below is 2*S+K, where K is the number of
                // distinct grouped GD values; 3*S is a safe preflight bound.
                let support_limit = scan_budget
                    .saturating_sub(1)
                    .checked_div(3)
                    .unwrap_or(0)
                    .min(crate::score_integral::MAX_KERNEL_SUPPORT);
                let support = fixture
                    .scores
                    .conditional_scores(outcome as usize)
                    .take(support_limit.saturating_add(1))
                    .collect::<Vec<_>>();
                let scan_work = support.len();
                if scan_work > support_limit
                    || support.is_empty()
                    || support.len() > crate::score_integral::MAX_KERNEL_SUPPORT
                {
                    kernel_work = kernel_work.saturating_add(scan_work);
                    break;
                }
                // Grouping revisits every support row, then materializes K
                // grouped endpoints. Reserve the full 3*S upper bound first.
                if kernel_work.saturating_add(scan_work.saturating_mul(3)) > remaining {
                    kernel_work = kernel_work.saturating_add(scan_work);
                    break;
                }
                kernel_work = kernel_work.saturating_add(scan_work.saturating_mul(2));
                let canonical_score = canonical(outcome);
                let mut grouped = std::collections::BTreeMap::<i32, f64>::new();
                for (score, mass) in support {
                    let contribution =
                        relative_fixture_score(fixture, team, score, canonical_score);
                    *grouped.entry(contribution.gd).or_default() += mass;
                }
                let values = grouped
                    .into_iter()
                    .map(|(gd, mass)| ScorePair { gd, gf: 0, mass })
                    .collect::<Vec<_>>();
                kernel_work = kernel_work.saturating_add(values.len());
                candidate_kernels.push(JointKernel {
                    fixture: fixture_index,
                    values,
                });
            }
            remaining = remaining.saturating_sub(kernel_work);
            let Some(dp) = PrefixDp::build_with_caps(
                &candidate_kernels,
                remaining,
                cells_left,
                caps.layer_cells
                    .min(crate::score_integral::MAX_LAYER_CELLS)
                    .min(crate::score_integral::MAX_COHORT_GD_ENDPOINTS),
                caps.convolution
                    .min(crate::score_integral::DEFAULT_BUILD_CAPS.convolution),
            ) else {
                prep_delta = prep_delta.saturating_add(kernel_work);
                continue;
            };
            // PrefixDp may stop at a bounded prefix. Do not retain/build cache
            // entries for kernels that were never represented in its layers.
            candidate_kernels.truncate(dp.integrated);
            transition_delta = transition_delta.saturating_add(dp.work);
            if dp.integrated == 0 {
                prep_delta = prep_delta.saturating_add(kernel_work + dp.work);
                continue;
            }
            let prefix_cells = dp.layers.iter().map(|layer| layer.len()).sum::<usize>();
            let endpoint_layer = dp.layers.last()?;
            let endpoint_base = endpoint_layer.len().checked_next_power_of_two()?.max(1);
            let estimated_index_work = endpoint_layer
                .len()
                .saturating_add(endpoint_base.saturating_mul(2));
            let endpoint_mass_scan = endpoint_layer.len();
            let estimated_total = kernel_work
                .saturating_add(dp.work)
                .saturating_add(estimated_index_work)
                .saturating_add(endpoint_mass_scan);
            if estimated_total > team_work_limit {
                prep_delta = prep_delta.saturating_add(kernel_work + dp.work);
                continue;
            }
            let Some((index, index_work)) =
                crate::score_integral::EndpointIndex::build(endpoint_layer)
            else {
                prep_delta = prep_delta.saturating_add(kernel_work + dp.work);
                continue;
            };
            let retained_cells = prefix_cells.saturating_add(index.retained_cells());
            if retained_cells > cells_left {
                prep_delta = prep_delta.saturating_add(kernel_work + dp.work + index_work);
                continue;
            }
            let total_work = kernel_work
                .saturating_add(dp.work)
                .saturating_add(index_work)
                .saturating_add(endpoint_mass_scan);
            let mut total_mass = 0.0;
            for mass in endpoint_layer.values() {
                total_mass += *mass;
            }
            if total_work > team_work_limit
                || !total_mass.is_finite()
                || (total_mass - 1.0).abs() > 1e-10
            {
                prep_delta = prep_delta.saturating_add(total_work);
                continue;
            }
            let cached = std::sync::Arc::new(CachedScoreIntegral {
                kernels: candidate_kernels,
                dp,
                index,
                index_build_work: index_work,
                endpoint_mass_scan,
            });
            self.cohort_gd_cache_cells += retained_cells;
            self.cohort_gd_cache.insert(cache_key, cached.clone());
            prepared_by_team[team] = Some(cached);
            prep_delta = prep_delta.saturating_add(total_work);
        }
        self.cohort_gd_preparation_work =
            self.cohort_gd_preparation_work.saturating_add(prep_delta);
        operations.guidance = operations.guidance.saturating_add(prep_delta as u64);

        let integrated_fixtures: std::collections::HashSet<usize> = cohort
            .iter()
            .filter_map(|&team| prepared_by_team[team].as_ref())
            .flat_map(|prepared| prepared.kernels[..prepared.dp.integrated].iter())
            .map(|kernel| kernel.fixture)
            .collect();
        self.cohort_gd_last_integrated = integrated_fixtures.len();
        if integrated_fixtures.is_empty() {
            return None;
        }

        if query_allowance.is_some() {
            let target_position = cohort
                .iter()
                .position(|&team| team == target)
                .expect("target belongs to its own points/wins cohort");
            let target_prepared = prepared_by_team[cohort[target_position]].as_ref();
            let target_support = target_prepared
                .and_then(|prepared| prepared.dp.layers.last())
                .map_or(1, |layer| layer.len());
            let rival_supports = cohort
                .iter()
                .enumerate()
                .filter(|(position, _)| *position != target_position)
                .map(|(_, &team)| {
                    prepared_by_team[team]
                        .as_ref()
                        .map_or(1, |prepared| prepared.index.len())
                })
                .collect::<Vec<_>>();
            let integrated_kernel_costs = cohort
                .iter()
                .filter_map(|&team| prepared_by_team[team].as_ref())
                .flat_map(|prepared| prepared.kernels[..prepared.dp.integrated].iter())
                .map(|kernel| {
                    kernel.values.len().saturating_mul(2).saturating_add(
                        model.fixtures[kernel.fixture]
                            .scores
                            .conditional_support_len(outcomes[kernel.fixture] as usize)
                            .saturating_mul(3),
                    )
                })
                .collect::<Vec<_>>();
            // This allowance also meters the per-query vector traversal used
            // to form the bound; cap it using only the realized fixture/cohort
            // sizes and charge it before deciding whether sampling may begin.
            let bookkeeping_work = model
                .fixtures
                .len()
                .saturating_mul(2)
                .saturating_add(cohort.len().saturating_mul(4));
            let reserved = crate::score_integral::cohort_gd_prepared_query_work_upper_bound(
                target_support,
                &rival_supports,
                model.fixtures.len(),
                &integrated_kernel_costs,
                bookkeeping_work,
            );
            operations.guidance = operations.guidance.saturating_add(bookkeeping_work as u64);
            let prep_charge = if query_allowance_includes_prep {
                prep_delta
            } else {
                0
            };
            if reserved.saturating_add(prep_charge)
                > query_allowance.expect("bounded query allowance was supplied")
            {
                return None;
            }
            self.score_integral_last_query_reserved = reserved;
        }
        for &team in &cohort {
            let integrated = prepared_by_team[team]
                .as_ref()
                .map_or(0, |prepared| prepared.dp.integrated);
            self.cohort_gd_last_team_counts.push((
                model.ids[team],
                private_by_team[team].len(),
                integrated,
            ));
        }

        // Draw every common/shared and nonintegrated score once. Integrated
        // private fixtures get their canonical placeholder contribution.
        self.campaign.clone_from(&model.base);
        for (index, fixture) in model.fixtures.iter().enumerate() {
            let score = if integrated_fixtures.contains(&index) {
                canonical(outcomes[index])
            } else {
                operations.fixtures += 1;
                fixture.scores.sample(outcomes[index] as usize, rng)
            };
            self.scores[fixture.request_index] = score;
            model.add(&mut self.campaign, index, score);
        }

        // Reuse each prepared team's cached positive GD index. A nonintegrated
        // team has the identity private projection, represented by one shared
        // endpoint/index for this query. Strict and residual weights share
        // one set of interval classifications per target endpoint.
        let identity_layer = std::collections::BTreeMap::from([((0, 0), 1.0)]);
        let (identity_index, identity_index_work) =
            crate::score_integral::EndpointIndex::build(&identity_layer)
                .expect("one-cell identity endpoint index is representable");
        let target_position = cohort
            .iter()
            .position(|&team| team == target)
            .expect("target belongs to its own points/wins cohort");
        let target_team = cohort[target_position];
        let target_base_gd = self.campaign[target_team].gf - self.campaign[target_team].ga;
        let target_layer = prepared_by_team[target_team]
            .as_ref()
            .and_then(|prepared| prepared.dp.layers.last())
            .unwrap_or(&identity_layer);
        let mut rivals = Vec::with_capacity(cohort.len().saturating_sub(1));
        for (position, &team) in cohort.iter().enumerate() {
            if position == target_position {
                continue;
            }
            let base_gd = self.campaign[team].gf - self.campaign[team].ga;
            let index = prepared_by_team[team]
                .as_ref()
                .map(|prepared| &prepared.index)
                .unwrap_or(&identity_index);
            rivals.push((base_gd, index));
        }
        let strict_count = rank
            .checked_sub(classes_above)
            .expect("target rank is inside its points/wins cohort");
        let projection = crate::score_integral::cohort_gd::project_indexed(
            target_layer,
            target_base_gd,
            &rivals,
            strict_count,
        );
        let residual_total = projection.residual;
        self.cohort_gd_last_strict_mass = projection.strict;
        self.cohort_gd_last_residual_mass = residual_total;
        let residual_choices = projection.residual_choices;
        query_work = query_work
            .saturating_add(identity_index_work)
            .saturating_add(projection.query_work);
        operations.guidance = operations.guidance.saturating_add(query_work as u64);
        self.score_integral_last_query_work = query_work;

        let mut backward_work = 0usize;
        let mut residual_indicator = 0.0;
        if residual_total > 0.0 {
            let mut draw = rng.float() * residual_total;
            let (target_gd, first_equal) = residual_choices
                .iter()
                .find(|(_, _, weight)| {
                    draw -= *weight;
                    draw < 0.0
                })
                .map(|(gd, rival, _)| (*gd, *rival))
                .or_else(|| residual_choices.last().map(|(gd, rival, _)| (*gd, *rival)))
                .expect("positive residual mass has a first-equal configuration");
            let choice_scan_work = residual_choices.len();
            query_work = query_work.saturating_add(choice_scan_work);
            self.score_integral_last_query_work = query_work;
            operations.guidance = operations.guidance.saturating_add(choice_scan_work as u64);
            let mut desired_gd = vec![0i32; cohort.len()];
            desired_gd[target_position] = target_gd;
            let mut residual_rng_work = 0usize;
            for position in 0..cohort.len() {
                if position == target_position {
                    continue;
                }
                let rival_pos = if position < target_position {
                    position
                } else {
                    position - 1
                };
                let team = cohort[position];
                let base_gd = self.campaign[team].gf - self.campaign[team].ga;
                let gd_layer = prepared_by_team[team]
                    .as_ref()
                    .and_then(|prepared| prepared.dp.layers.last())
                    .unwrap_or(&identity_layer);
                let condition = if rival_pos < first_equal {
                    0 // any GD except target
                } else if rival_pos == first_equal {
                    1 // equal GD
                } else {
                    2 // unrestricted
                };
                let mut total = 0.0;
                for (&(delta_gd, _), &mass) in gd_layer {
                    let gd = base_gd + delta_gd;
                    if (condition == 0 && gd != target_gd)
                        || (condition == 1 && gd == target_gd)
                        || condition == 2
                    {
                        total += mass;
                    }
                }
                if total <= 0.0 {
                    panic!("selected residual configuration has positive conditional mass");
                }
                let mut value = rng.float() * total;
                let mut selected = None;
                let mut selection_scan_work = 0usize;
                for (&(delta_gd, _), &mass) in gd_layer {
                    let gd = base_gd + delta_gd;
                    selection_scan_work = selection_scan_work.saturating_add(1);
                    if (condition == 0 && gd != target_gd)
                        || (condition == 1 && gd == target_gd)
                        || condition == 2
                    {
                        value -= mass;
                        if value < 0.0 {
                            selected = Some(gd);
                            break;
                        }
                    }
                }
                desired_gd[position] = selected.unwrap_or_else(|| {
                    gd_layer
                        .iter()
                        .rev()
                        .find_map(|(&(delta_gd, _), _)| {
                            let gd = base_gd + delta_gd;
                            ((condition == 0 && gd != target_gd)
                                || (condition == 1 && gd == target_gd)
                                || condition == 2)
                                .then_some(gd)
                        })
                        .expect("positive conditional GD mass has a support value")
                });
                residual_rng_work = residual_rng_work
                    .saturating_add(gd_layer.len())
                    .saturating_add(selection_scan_work);
            }
            for (position, &team) in cohort.iter().enumerate() {
                let Some(prepared) = prepared_by_team[team].as_ref() else {
                    continue;
                };
                let base_gd = self.campaign[team].gf - self.campaign[team].ga;
                let endpoint = (desired_gd[position] - base_gd, 0);
                if prepared.dp.integrated == 0 {
                    continue;
                }
                let used = &prepared.kernels[..prepared.dp.integrated];
                let path = prepared
                    .dp
                    .sample_path(used, endpoint, rng)
                    .expect("positive cohort GD endpoint has a DP backtrack path");
                backward_work = backward_work.saturating_add(
                    used.iter()
                        .map(|kernel| kernel.values.len().saturating_mul(2))
                        .sum::<usize>(),
                );
                for (kernel, &(gd, _)) in used.iter().zip(path.iter()) {
                    let fixture = &model.fixtures[kernel.fixture];
                    let canonical_score = canonical(outcomes[kernel.fixture]);
                    let mut support = Vec::new();
                    let mut support_scan_work = 0usize;
                    for (score, mass) in fixture
                        .scores
                        .conditional_scores(outcomes[kernel.fixture] as usize)
                    {
                        support_scan_work = support_scan_work.saturating_add(1);
                        let contribution =
                            relative_fixture_score(fixture, team, score, canonical_score);
                        if contribution.gd == gd {
                            support.push((score, mass));
                        }
                    }
                    let total: f64 = support.iter().map(|(_, mass)| *mass).sum();
                    if total <= 0.0 {
                        panic!("positive cohort GD backtrack contribution has score support");
                    }
                    let mut value = rng.float() * total;
                    let mut selection_scan_work = 0usize;
                    let score = support
                        .iter()
                        .find(|(_, mass)| {
                            selection_scan_work = selection_scan_work.saturating_add(1);
                            value -= *mass;
                            value < 0.0
                        })
                        .or_else(|| support.last())
                        .map(|(score, _)| *score)
                        .expect("positive cohort GD backtrack has a scoreline");
                    self.scores[fixture.request_index] = score;
                    backward_work = backward_work
                        .saturating_add(support_scan_work)
                        .saturating_add(support.len())
                        .saturating_add(selection_scan_work);
                }
            }
            self.campaign.clone_from(&model.base);
            for (index, fixture) in model.fixtures.iter().enumerate() {
                model.add(
                    &mut self.campaign,
                    index,
                    self.scores[fixture.request_index],
                );
            }
            operations.guidance = operations
                .guidance
                .saturating_add(model.fixtures.len() as u64);
            model.standings(&mut self.order, &self.campaign, &self.scores, rng);
            let actual_rank = self
                .order
                .iter()
                .position(|&team| team == target)
                .expect("target appears in native standings");
            if actual_rank == rank {
                residual_indicator = 1.0;
                self.cohort_gd_last_residual_rank_hit = true;
            }
            backward_work = backward_work.saturating_add(residual_rng_work);
            operations.guidance = operations.guidance.saturating_add(backward_work as u64);
        }
        self.score_integral_last_query_work = query_work;
        let all_cache_hit = all_cache_hits
            && cohort
                .iter()
                .all(|&team| private_by_team[team].is_empty() || prepared_by_team[team].is_some());
        Some((
            projection.strict + residual_total * residual_indicator,
            prep_delta,
            self.cohort_gd_last_integrated,
            transition_delta,
            query_work,
            backward_work,
            all_cache_hit,
        ))
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

#[cfg(test)]
mod analytic_score_tests {
    use super::*;
    use crate::model::{Championship, Game, Phase, Request, Team};

    fn fixture_model(sort: &str, target_points: i32, rival_points: i32) -> Model {
        Model::new(Request {
            id: 42,
            zones: vec![],
            phase: Phase {
                sort: sort.into(),
                championship: Championship {
                    point_win: 3,
                    point_draw: 1,
                    point_loss: 0,
                },
                bonus_points: 0,
                bonus_points_threshold: 0,
            },
            games: vec![Game {
                id: 1,
                home_id: 1,
                away_id: 3,
                home_score: 0,
                away_score: 0,
                home_power: 1.2,
                away_power: 0.9,
                played: false,
            }],
            team_groups: vec![
                Team {
                    team_id: 1,
                    add_sub: target_points,
                    bias: 0,
                },
                Team {
                    team_id: 2,
                    add_sub: rival_points,
                    bias: 0,
                },
                Team {
                    team_id: 3,
                    add_sub: 0,
                    bias: 0,
                },
            ],
        })
        .unwrap()
    }

    #[test]
    fn explicit_wdl_probabilities_drive_event_mass_and_backward_samples() {
        let model = fixture_model("points", 0, 0);
        let bounds = Bounds::new(&model);
        let native = Event::build(&model, &bounds, 0, 0, &[2], 100).unwrap();
        let supplied = [[0., 0., 1.]];
        let overridden =
            Event::build_with_probabilities(&model, &bounds, 0, 0, &[2], 100, &supplied).unwrap();
        assert_eq!(overridden.games[0].prob, supplied[0]);
        assert_ne!(native.mass.to_bits(), overridden.mass.to_bits());
        assert_eq!(overridden.mass, 1.);

        let mut backward = Backward::new(&overridden);
        let mut outcomes = vec![0; model.fixtures.len()];
        backward.sample(&mut Rng::new(19), &mut outcomes);
        assert_eq!(outcomes[0], 2);
    }

    #[test]
    fn explicit_probabilities_validate_shape_and_probability_law() {
        let model = fixture_model("points", 0, 0);
        let bounds = Bounds::new(&model);
        for invalid in [
            vec![],
            vec![[0., 0., 0.]],
            vec![[1., -0.1, 0.1]],
            vec![[f64::NAN, 0., 1.]],
            vec![[0.2, 0.2, 0.2]],
        ] {
            assert!(
                Event::build_with_probabilities(&model, &bounds, 0, 0, &[2], 100, &invalid,)
                    .is_none()
            );
        }
    }

    #[test]
    fn explicit_model_probabilities_reproduce_regular_event_build() {
        let model = fixture_model("points", 0, 0);
        let bounds = Bounds::new(&model);
        let regular = Event::build(&model, &bounds, 0, 0, &[2], 100).unwrap();
        let probabilities: Vec<_> = model.fixtures.iter().map(|f| f.prob).collect();
        let supplied =
            Event::build_with_probabilities(&model, &bounds, 0, 0, &[2], 100, &probabilities)
                .unwrap();
        assert_eq!(regular.mass.to_bits(), supplied.mass.to_bits());
        assert_eq!(
            regular
                .games
                .iter()
                .map(|g| (g.index, g.prob, g.delta))
                .collect::<Vec<_>>(),
            supplied
                .games
                .iter()
                .map(|g| (g.index, g.prob, g.delta))
                .collect::<Vec<_>>(),
        );
    }

    fn cohort_caps() -> crate::score_integral::BuildCaps {
        crate::score_integral::DEFAULT_BUILD_CAPS
    }

    fn cohort_snapshot_model() -> Model {
        Model::new(Request {
            id: 43,
            zones: vec![],
            phase: Phase {
                sort: "pt,gd,gf,bias".into(),
                championship: Championship {
                    point_win: 3,
                    point_draw: 1,
                    point_loss: 0,
                },
                bonus_points: 0,
                bonus_points_threshold: 0,
            },
            games: vec![
                Game {
                    id: 1,
                    home_id: 1,
                    away_id: 3,
                    home_score: 0,
                    away_score: 0,
                    home_power: 1.2,
                    away_power: 0.9,
                    played: false,
                },
                Game {
                    id: 2,
                    home_id: 4,
                    away_id: 2,
                    home_score: 0,
                    away_score: 0,
                    home_power: 0.8,
                    away_power: 1.0,
                    played: false,
                },
            ],
            team_groups: vec![
                Team {
                    team_id: 1,
                    add_sub: 0,
                    bias: 0,
                },
                Team {
                    team_id: 2,
                    add_sub: 0,
                    bias: 0,
                },
                Team {
                    team_id: 3,
                    add_sub: 0,
                    bias: 0,
                },
                Team {
                    team_id: 4,
                    add_sub: -100,
                    bias: 0,
                },
            ],
        })
        .unwrap()
    }

    fn cohort_two_private_fixtures_per_team_model() -> Model {
        Model::new(Request {
            id: 44,
            zones: vec![],
            phase: Phase {
                sort: "pt,gd,gf,bias".into(),
                championship: Championship {
                    point_win: 3,
                    point_draw: 1,
                    point_loss: 0,
                },
                bonus_points: 0,
                bonus_points_threshold: 0,
            },
            games: vec![(1, 3), (1, 4), (2, 5), (2, 6)]
                .into_iter()
                .enumerate()
                .map(|(index, (home_id, away_id))| Game {
                    id: index as i32 + 1,
                    home_id,
                    away_id,
                    home_score: 0,
                    away_score: 0,
                    home_power: if home_id == 1 || home_id == 2 {
                        1.2
                    } else {
                        0.9
                    },
                    away_power: if home_id == 1 || home_id == 2 {
                        0.9
                    } else {
                        1.2
                    },
                    played: false,
                })
                .collect(),
            team_groups: (1..=6)
                .map(|team_id| Team {
                    team_id,
                    add_sub: if team_id >= 3 { -100 } else { 0 },
                    bias: 0,
                })
                .collect(),
        })
        .unwrap()
    }

    #[test]
    fn head_is_supported_only_after_adjacent_gd_gf() {
        let supported = fixture_model("pt,w,gd,gf,head", 0, 3);
        assert!(crate::score_integral::supported(&supported));
        for sort in ["pt,head,gd,gf", "pt,gd,head,gf", "pt,gf,gd,head"] {
            let unsupported = fixture_model(sort, 0, 3);
            assert!(
                !crate::score_integral::supported(&unsupported),
                "unsupported Head placement {sort}"
            );
        }
    }

    #[test]
    fn head_suffix_cohort_matches_exhaustive_contextual_points_grouping() {
        fn exact_probability(model: &Model, outcomes: &[u8]) -> f64 {
            fn visit(
                model: &Model,
                outcomes: &[u8],
                index: usize,
                scores: &mut [[i32; 2]],
                mass: f64,
                total: &mut f64,
            ) {
                if index == model.fixtures.len() {
                    let mut campaign = model.base.clone();
                    for (fixture_index, fixture) in model.fixtures.iter().enumerate() {
                        model.add(&mut campaign, fixture_index, scores[fixture.request_index]);
                    }
                    let mut order = vec![0; model.n];
                    model.standings(&mut order, &campaign, scores, &mut Rng::new(17));
                    if order[0] == 0 {
                        *total += mass;
                    }
                    return;
                }
                let fixture = &model.fixtures[index];
                for (score, probability) in
                    fixture.scores.conditional_scores(outcomes[index] as usize)
                {
                    scores[fixture.request_index] = score;
                    visit(
                        model,
                        outcomes,
                        index + 1,
                        scores,
                        mass * probability,
                        total,
                    );
                }
            }

            let mut scores = model.empty_scores();
            let mut total = 0.0;
            visit(model, outcomes, 0, &mut scores, 1.0, &mut total);
            total
        }

        let model = Model::new(Request {
            id: 45,
            zones: vec![],
            phase: Phase {
                sort: "pt,w,gd,gf,head".into(),
                championship: Championship {
                    point_win: 3,
                    point_draw: 1,
                    point_loss: 0,
                },
                bonus_points: 0,
                bonus_points_threshold: 0,
            },
            games: vec![
                Game {
                    id: 1,
                    home_id: 1,
                    away_id: 3,
                    home_score: 0,
                    away_score: 0,
                    home_power: 0.8,
                    away_power: 0.8,
                    played: false,
                },
                Game {
                    id: 2,
                    home_id: 2,
                    away_id: 3,
                    home_score: 0,
                    away_score: 0,
                    home_power: 0.8,
                    away_power: 0.8,
                    played: false,
                },
                Game {
                    id: 3,
                    home_id: 1,
                    away_id: 4,
                    home_score: 0,
                    away_score: 0,
                    home_power: 0.8,
                    away_power: 0.8,
                    played: false,
                },
                Game {
                    id: 4,
                    home_id: 2,
                    away_id: 4,
                    home_score: 0,
                    away_score: 0,
                    home_power: 0.8,
                    away_power: 0.8,
                    played: false,
                },
            ],
            team_groups: vec![
                Team {
                    team_id: 1,
                    add_sub: 0,
                    bias: 0,
                },
                Team {
                    team_id: 2,
                    add_sub: 0,
                    bias: 0,
                },
                Team {
                    team_id: 3,
                    add_sub: 4,
                    bias: 0,
                },
                Team {
                    team_id: 4,
                    add_sub: -10,
                    bias: 0,
                },
            ],
        })
        .unwrap();
        let outcomes = [2, 2, 1, 1];
        let mut canonical_campaign = model.base.clone();
        for (index, &outcome) in outcomes.iter().enumerate() {
            model.add(&mut canonical_campaign, index, canonical(outcome));
        }
        assert_eq!(canonical_campaign[0].points, canonical_campaign[1].points);
        assert_eq!(canonical_campaign[0].wins, canonical_campaign[1].wins);
        assert_eq!(canonical_campaign[2].points, canonical_campaign[0].points);
        assert_ne!(canonical_campaign[2].wins, canonical_campaign[0].wins);

        // This finite scoreline makes the leading Pt/W/GD/GF keys tie A and
        // B. The points-only Head mini-table also includes C, whose wins differ
        // from the integrated cohort, and ranks B above A on its head GF.
        let mut witness_scores = model.empty_scores();
        for (fixture, score) in [[1, 0], [2, 1], [1, 1], [0, 0]].into_iter().enumerate() {
            witness_scores[model.fixtures[fixture].request_index] = score;
        }
        let mut witness_campaign = model.base.clone();
        for (fixture_index, fixture) in model.fixtures.iter().enumerate() {
            model.add(
                &mut witness_campaign,
                fixture_index,
                witness_scores[fixture.request_index],
            );
        }
        assert_eq!(witness_campaign[0].points, witness_campaign[1].points);
        assert_eq!(witness_campaign[0].wins, witness_campaign[1].wins);
        assert_eq!(
            witness_campaign[0].gf - witness_campaign[0].ga,
            witness_campaign[1].gf - witness_campaign[1].ga
        );
        assert_eq!(witness_campaign[0].gf, witness_campaign[1].gf);
        let mut head_order = vec![0; model.n];
        model.standings(
            &mut head_order,
            &witness_campaign,
            &witness_scores,
            &mut Rng::new(18),
        );
        let mut no_head_request = model.request.clone();
        no_head_request.phase.sort = "pt,w,gd,gf".into();
        let no_head = Model::new(no_head_request).unwrap();
        let mut no_head_order = vec![0; no_head.n];
        no_head.standings(
            &mut no_head_order,
            &witness_campaign,
            &witness_scores,
            &mut Rng::new(18),
        );
        assert_eq!(head_order[0], 1, "contextual Head ranks B over A");
        assert_eq!(
            no_head_order[0], 0,
            "without Head the stable tie keeps A first"
        );

        let exact = exact_probability(&model, &outcomes);
        let trials = 2_000;
        let mut estimate = 0.0;
        let mut integrated = 0;
        let mut context = ScoreContext::new(&model);
        for seed in 0..trials {
            let mut operations = crate::rare_tail::budget::Operations::default();
            let sample = context
                .fraction_cohort_gd_bounded(
                    0,
                    0,
                    &outcomes,
                    &mut Rng::new(seed + 80_000),
                    &mut operations,
                    crate::score_integral::MAX_WORK,
                    usize::MAX,
                    cohort_caps(),
                )
                .expect("Head after GD/GF is supported");
            integrated = sample.2;
            estimate += sample.0;
            let classification_work = outcomes.len().saturating_add(model.n.saturating_mul(4));
            let query_side_work = operations
                .units()
                .saturating_sub(sample.1)
                .saturating_sub(classification_work);
            assert!(
                query_side_work <= context.score_integral_last_query_reserved(),
                "query/backtrack/rebuild work {query_side_work} exceeds reserved {} at seed {seed}",
                context.score_integral_last_query_reserved()
            );
        }
        estimate /= trials as f64;
        assert!(integrated > 0, "private goal scores should be integrated");
        assert!(
            (estimate - exact).abs() < 0.06,
            "contextual Head estimate={estimate}, exhaustive oracle={exact}"
        );
    }

    #[test]
    fn cohort_snapshot_reuse_preserves_rng_and_uses_separate_path_cap() {
        let model = cohort_snapshot_model();
        let outcomes = [1, 1];
        let mut pilot = ScoreContext::new(&model);
        let mut pilot_ops = crate::rare_tail::budget::Operations::default();
        pilot
            .fraction_cohort_gd(
                0,
                0,
                &outcomes,
                &mut Rng::new(301),
                &mut pilot_ops,
                crate::score_integral::MAX_WORK,
                cohort_caps(),
            )
            .expect("pilot cohort projection is supported");
        let snapshot = pilot.score_integral_snapshot().unwrap_or_else(|| {
            panic!(
                "cohort cache snapshot missing; stats={:?}, team_counts={:?}, cache={:?}",
                pilot.score_integral_cohort_stats(),
                pilot.cohort_gd_team_counts(),
                pilot.cohort_gd_cache_stats()
            )
        });
        assert_eq!(snapshot.cohort_gd_entries.len(), 1);

        let mut cold = ScoreContext::new(&model);
        let mut cold_ops = crate::rare_tail::budget::Operations::default();
        let mut cold_rng = Rng::new(302);
        let cold_fraction = cold
            .fraction_cohort_gd(
                0,
                0,
                &outcomes,
                &mut cold_rng,
                &mut cold_ops,
                crate::score_integral::MAX_WORK,
                cohort_caps(),
            )
            .unwrap();
        let cold_next = cold_rng.float();

        let mut hot = ScoreContext::with_score_integral_snapshot_and_caps(
            &model,
            Some(snapshot.clone()),
            crate::score_integral::MAX_TOTAL_CELLS,
            crate::score_integral::MAX_COHORT_GD_PATHS,
        );
        let mut hot_ops = crate::rare_tail::budget::Operations::default();
        let mut hot_rng = Rng::new(302);
        let hot_fraction = hot
            .fraction_cohort_gd(
                0,
                0,
                &outcomes,
                &mut hot_rng,
                &mut hot_ops,
                crate::score_integral::MAX_WORK,
                cohort_caps(),
            )
            .unwrap();
        assert!(hot_fraction.6, "imported cohort projections should hit");
        assert_eq!(hot_fraction.0.to_bits(), cold_fraction.0.to_bits());
        assert_eq!(hot_rng.float().to_bits(), cold_next.to_bits());

        let (key, entry) = snapshot.cohort_gd_entries.iter().next().unwrap();
        let mut too_many_paths = (*snapshot).clone();
        for added in 0..=crate::score_integral::MAX_COHORT_GD_PATHS {
            too_many_paths.cohort_gd_entries.insert(
                ScoreIntegralKey {
                    target: key.target + 10_000 + added,
                    caps: key.caps,
                    eligible: key.eligible.clone(),
                },
                entry.clone(),
            );
        }
        let rejected = ScoreContext::with_score_integral_snapshot_and_caps(
            &model,
            Some(std::sync::Arc::new(too_many_paths)),
            crate::score_integral::MAX_TOTAL_CELLS,
            crate::score_integral::MAX_COHORT_GD_PATHS,
        );
        assert!(rejected.cohort_gd_cache.is_empty());
    }

    #[test]
    fn model_replacement_rejects_snapshot_even_at_same_storage_address() {
        let mut model = cohort_snapshot_model();
        let outcomes = [1, 1];
        let mut source = ScoreContext::new(&model);
        let mut source_ops = crate::rare_tail::budget::Operations::default();
        source
            .fraction_cohort_gd(
                0,
                0,
                &outcomes,
                &mut Rng::new(303),
                &mut source_ops,
                crate::score_integral::MAX_WORK,
                cohort_caps(),
            )
            .expect("source model admits cohort integration");
        let snapshot = source.score_integral_snapshot().unwrap();

        let mut request = model.request.clone();
        request.games[0].home_power += 0.25;
        model = Model::new(request).unwrap();
        let cold = ScoreContext::with_score_integral_snapshot_and_caps(
            &model,
            Some(snapshot),
            crate::score_integral::MAX_TOTAL_CELLS,
            crate::score_integral::MAX_COHORT_GD_PATHS,
        );
        assert!(cold.cohort_gd_cache.is_empty());
    }

    #[test]
    fn zero_integrated_private_prefix_returns_before_consuming_score_rng() {
        let model = cohort_two_private_fixtures_per_team_model();
        let outcomes = [2, 0, 0, 2];
        let mut caps = cohort_caps();
        caps.work = 0;
        let mut context = ScoreContext::new(&model);
        let mut operations = crate::rare_tail::budget::Operations::default();
        let mut rng = Rng::new(304);
        let mut untouched = Rng::new(304);
        assert!(context
            .fraction_cohort_gd(0, 0, &outcomes, &mut rng, &mut operations, 0, caps)
            .is_none());
        assert_eq!(rng.float().to_bits(), untouched.float().to_bits());
    }

    #[test]
    fn cohort_zero_setup_budget_returns_to_native_fallback_without_rng_use() {
        let model = fixture_model("pt,gd,gf,bias", 0, 1);
        let outcomes = [1];
        let mut context = ScoreContext::new(&model);
        let mut operations = crate::rare_tail::budget::Operations::default();
        let mut rng = Rng::new(991);
        let mut untouched = Rng::new(991);
        assert!(context
            .fraction_cohort_gd(0, 0, &outcomes, &mut rng, &mut operations, 0, cohort_caps())
            .is_none());
        assert_eq!(rng.float().to_bits(), untouched.float().to_bits());
        assert_eq!(operations.fixtures, 0);
    }

    #[test]
    fn cohort_small_setup_budget_returns_to_native_fallback_without_rng_use() {
        let model = cohort_snapshot_model();
        let outcomes = [1, 1];
        let mut context = ScoreContext::new(&model);
        let mut operations = crate::rare_tail::budget::Operations::default();
        let mut rng = Rng::new(992);
        let mut untouched = Rng::new(992);
        assert!(context
            .fraction_cohort_gd(0, 0, &outcomes, &mut rng, &mut operations, 5, cohort_caps())
            .is_none());
        assert_eq!(rng.float().to_bits(), untouched.float().to_bits());
        assert_eq!(operations.fixtures, 0);
        assert!(context.score_integral_preparation_work() <= 5);
    }

    #[test]
    fn bounded_cohort_query_rejects_before_rng_and_bounds_partial_cache_backtracking() {
        let model = cohort_two_private_fixtures_per_team_model();
        let outcomes = [2, 0, 0, 2];
        for fixture in &model.fixtures {
            for outcome in 0..3 {
                assert_eq!(
                    fixture.scores.conditional_support_len(outcome),
                    fixture.scores.conditional_scores(outcome).count()
                );
            }
        }
        let mut context = ScoreContext::new(&model);
        let mut operations = crate::rare_tail::budget::Operations::default();
        let mut denied_rng = Rng::new(3041);
        let mut untouched = Rng::new(3041);
        assert!(context
            .fraction_cohort_gd_bounded(
                0,
                0,
                &outcomes,
                &mut denied_rng,
                &mut operations,
                crate::score_integral::MAX_WORK,
                0,
                cohort_caps(),
            )
            .is_none());
        assert_eq!(denied_rng.float().to_bits(), untouched.float().to_bits());
        assert_eq!(context.score_integral_last_query_reserved(), 0);
        assert_eq!(context.score_integral_last_query_work(), 0);
        assert!(context.score_integral_cache_paths() > 0);

        let mut accepted_rng = Rng::new(3041);
        let fixture_units_before = operations.fixtures;
        let (.., backward_work, _) = context
            .fraction_cohort_gd_bounded(
                0,
                0,
                &outcomes,
                &mut accepted_rng,
                &mut operations,
                crate::score_integral::MAX_WORK,
                usize::MAX,
                cohort_caps(),
            )
            .expect("cached partial prefixes fit an unbounded query allowance");
        let reserved = context.score_integral_last_query_reserved();
        let bookkeeping = model
            .fixtures
            .len()
            .saturating_mul(2)
            .saturating_add(context.cohort_gd_last_size.saturating_mul(4));
        assert!(
            context
                .score_integral_last_query_work()
                .saturating_add(backward_work)
                .saturating_add(model.fixtures.len())
                .saturating_add((operations.fixtures - fixture_units_before) as usize * 8)
                .saturating_add(bookkeeping)
                <= reserved,
            "actual query/backtrack work plus bound bookkeeping must fit the reservation"
        );
    }

    #[test]
    fn shared_cohort_allowance_spends_prep_before_query_and_reuses_it() {
        let model = cohort_two_private_fixtures_per_team_model();
        let outcomes = [2, 0, 0, 2];
        let mut pilot = ScoreContext::new(&model);
        let mut pilot_operations = crate::rare_tail::budget::Operations::default();
        let pilot_result = pilot
            .fraction_cohort_gd_bounded(
                0,
                0,
                &outcomes,
                &mut Rng::new(4091),
                &mut pilot_operations,
                crate::score_integral::MAX_WORK,
                usize::MAX,
                cohort_caps(),
            )
            .expect("pilot measures the cold preparation and query reserve");
        let cold_prep = pilot_result.1;
        let query_reserve = pilot.score_integral_last_query_reserved();
        let total_allowance = cold_prep.saturating_add(query_reserve);
        assert!(cold_prep > 0 && query_reserve > 0);

        let mut denied = ScoreContext::new(&model);
        let mut denied_ops = crate::rare_tail::budget::Operations::default();
        let mut denied_rng = Rng::new(4092);
        let mut untouched = Rng::new(4092);
        assert!(denied
            .fraction_cohort_gd_bounded_total(
                0,
                0,
                &outcomes,
                &mut denied_rng,
                &mut denied_ops,
                crate::score_integral::MAX_WORK,
                total_allowance.saturating_sub(1),
                cohort_caps(),
            )
            .is_none());
        assert_eq!(denied_rng.float().to_bits(), untouched.float().to_bits());
        assert!(denied.score_integral_cache_paths() > 0);
        assert_eq!(denied.score_integral_last_query_reserved(), 0);

        let mut accepted = ScoreContext::new(&model);
        let mut accepted_ops = crate::rare_tail::budget::Operations::default();
        let accepted_result = accepted
            .fraction_cohort_gd_bounded_total(
                0,
                0,
                &outcomes,
                &mut Rng::new(4092),
                &mut accepted_ops,
                crate::score_integral::MAX_WORK,
                total_allowance,
                cohort_caps(),
            )
            .expect("unused preparation allowance is available to the query");
        assert!(
            accepted_result
                .1
                .saturating_add(accepted.score_integral_last_query_reserved())
                <= total_allowance
        );
    }

    #[test]
    fn cohort_grouping_charge_counts_scan_group_and_materialization_exactly() {
        let model = cohort_snapshot_model();
        let outcomes = [1, 1];
        let mut context = ScoreContext::new(&model);
        let mut operations = crate::rare_tail::budget::Operations::default();
        let fraction = context
            .fraction_cohort_gd(
                0,
                0,
                &outcomes,
                &mut Rng::new(994),
                &mut operations,
                crate::score_integral::MAX_WORK,
                cohort_caps(),
            )
            .expect("full private prefix setup succeeds");

        let mut expected = 0usize;
        for (key, prepared) in &context.cohort_gd_cache {
            for &(fixture_index, outcome) in &key.eligible {
                let fixture = &model.fixtures[fixture_index];
                let support = fixture
                    .scores
                    .conditional_scores(outcome as usize)
                    .collect::<Vec<_>>();
                let mut grouped = std::collections::BTreeSet::new();
                for (score, _) in &support {
                    grouped.insert(
                        relative_fixture_score(fixture, key.target, *score, canonical(outcome)).gd,
                    );
                }
                // One support enumeration, one grouping visit per support
                // row, and one materialized value per distinct GD endpoint.
                expected += 2 * support.len() + grouped.len();
            }
            expected += prepared.dp.work + prepared.index_build_work + prepared.endpoint_mass_scan;
        }
        assert_eq!(context.cohort_gd_cache.len(), 1);
        assert_eq!(fraction.1, expected, "setup ledger must charge exact 2*S+K");
        assert!(operations.units() >= fraction.1 + fraction.4 + fraction.5);
    }

    #[test]
    fn cohort_partial_and_zero_prefixes_do_not_retain_unused_kernels() {
        let model = cohort_two_private_fixtures_per_team_model();
        let outcomes = [2, 0, 0, 2];
        let mut full_caps = cohort_caps();
        let kernel_sizes = (0..2)
            .map(|team| {
                let first_fixture = team * 2;
                let mut sizes = [0; 2];
                for (offset, outcome) in [
                    if team == 0 { 2usize } else { 0usize },
                    if team == 0 { 0usize } else { 2usize },
                ]
                .into_iter()
                .enumerate()
                {
                    let fixture = &model.fixtures[first_fixture + offset];
                    let mut endpoints = std::collections::BTreeSet::new();
                    for (score, _) in fixture.scores.conditional_scores(outcome) {
                        endpoints.insert(
                            relative_fixture_score(fixture, team, score, canonical(outcome as u8))
                                .gd,
                        );
                    }
                    sizes[offset] = endpoints.len();
                }
                sizes
            })
            .collect::<Vec<_>>();
        assert!(kernel_sizes
            .iter()
            .all(|sizes| sizes.iter().all(|size| *size > 1)));
        full_caps.convolution = kernel_sizes.iter().map(|sizes| sizes[0]).max().unwrap();
        assert!(kernel_sizes
            .iter()
            .all(|sizes| sizes[0] * sizes[1] > full_caps.convolution));
        let mut partial_context = ScoreContext::new(&model);
        let partial_result = partial_context
            .fraction_cohort_gd(
                0,
                0,
                &outcomes,
                &mut Rng::new(995),
                &mut crate::rare_tail::budget::Operations::default(),
                crate::score_integral::MAX_WORK,
                full_caps,
            )
            .expect("bounded partial prefix keeps a native conditional draw");
        assert!(
            partial_result.2 > 0,
            "the first private fixture fits the prefix"
        );
        assert!(
            partial_result.2 < 4,
            "the second private fixture exceeds convolution cap"
        );
        assert_eq!(partial_context.score_integral_cohort_stats(), (2, 0, 4, 2));
        assert_eq!(partial_context.cohort_gd_cache.len(), 2);
        for prepared in partial_cache_values(&partial_context) {
            assert_eq!(prepared.dp.integrated, 1);
            assert_eq!(prepared.kernels.len(), 1);
            assert_eq!(prepared.kernels.len(), prepared.dp.integrated);
        }

        let mut zero_caps = cohort_caps();
        zero_caps.layer_cells = 1;
        let mut zero = ScoreContext::new(&model);
        let mut zero_rng = Rng::new(996);
        let mut untouched_rng = Rng::new(996);
        assert!(zero
            .fraction_cohort_gd(
                0,
                0,
                &outcomes,
                &mut zero_rng,
                &mut crate::rare_tail::budget::Operations::default(),
                crate::score_integral::MAX_WORK,
                zero_caps,
            )
            .is_none());
        assert_eq!(zero_rng.float().to_bits(), untouched_rng.float().to_bits());
        assert_eq!(zero.score_integral_cohort_stats(), (2, 0, 4, 0));
        assert_eq!(zero.cohort_gd_cache.len(), 0);
    }

    fn partial_cache_values<'a, 'b>(context: &'a ScoreContext<'b>) -> Vec<&'a CachedScoreIntegral> {
        context
            .cohort_gd_cache
            .values()
            .map(std::sync::Arc::as_ref)
            .collect()
    }

    #[test]
    fn cohort_early_rank_returns_charge_canonical_cohort_scans() {
        let model = fixture_model("pt,gd,gf,bias", 100, -100);
        let mut context = ScoreContext::new(&model);
        let mut operations = crate::rare_tail::budget::Operations::default();
        let result = context
            .fraction_cohort_gd(
                0,
                1,
                &[1],
                &mut Rng::new(993),
                &mut operations,
                crate::score_integral::MAX_WORK,
                cohort_caps(),
            )
            .expect("singleton tied cohort has a definite rank");
        assert_eq!(result.0, 0.0);
        assert!(operations.guidance >= (model.fixtures.len() + model.n * 4) as u64);
    }
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
