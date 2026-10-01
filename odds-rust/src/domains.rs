use crate::{
    conditioned::IntMap,
    lookahead::{RankGame, Suffix},
    proof::{max_gain, min_gain},
};
use std::sync::Arc;
pub struct Variable {
    pub step: usize,
    pub skipped: usize,
    pub hf: i32,
    pub af: i32,
}
pub struct Domains {
    pub domains: Vec<u8>,
    pub lower: Vec<i32>,
    pub upper: Vec<i32>,
    pub min: Vec<Vec<i32>>,
    pub max: Vec<Vec<i32>>,
    prefix_bounds: Vec<[i64; 4]>,
    pub feasible: bool,
    pub restricted: bool,
    pub base: Vec<i32>,
    pub forced: Vec<(usize, u8)>,
    pub variable: Vec<Variable>,
    pub after: usize,
    pub mass: f64,
    pub omitted: Vec<bool>,
    pub omitted_scores: Vec<bool>,
    pub omitted_count: usize,
}
impl Domains {
    fn new(games: &[RankGame], teams: usize, restricted: bool) -> Self {
        Self {
            domains: games
                .iter()
                .map(|g| (0..3).fold(0, |d, o| if g.prob[o] > 0. { d | (1 << o) } else { d }))
                .collect(),
            lower: vec![i32::MIN / 4; teams],
            upper: vec![i32::MAX / 4; teams],
            min: Vec::new(),
            max: Vec::new(),
            prefix_bounds: Vec::new(),
            feasible: true,
            restricted,
            base: Vec::new(),
            forced: Vec::new(),
            variable: Vec::new(),
            after: 0,
            mass: 1.,
            omitted: vec![false; games.len()],
            omitted_scores: Vec::new(),
            omitted_count: 0,
        }
    }
    pub fn propagate(
        games: &[RankGame],
        points: &[i32],
        rivals: &[usize],
        rank: usize,
        target: i32,
    ) -> Self {
        Self::seeded(games, points, rivals, rank, target, None, None)
    }
    /// Proposal-only intervals can tighten a branch without proving the whole
    /// rank event impossible. Callers must retain other branches or a fallback.
    pub fn condition(
        games: &[RankGame],
        points: &[i32],
        rivals: &[usize],
        rank: usize,
        target: i32,
        lower: &[i32],
        upper: &[i32],
        masks: &[u8],
    ) -> Self {
        Self::seeded(
            games,
            points,
            rivals,
            rank,
            target,
            Some(masks),
            Some((lower, upper)),
        )
    }
    fn seeded(
        games: &[RankGame],
        points: &[i32],
        rivals: &[usize],
        rank: usize,
        target: i32,
        seed: Option<&[u8]>,
        bounds: Option<(&[i32], &[i32])>,
    ) -> Self {
        let mut result = Self::new(games, points.len(), true);
        if let Some((lower, upper)) = bounds {
            result.lower.copy_from_slice(lower);
            result.upper.copy_from_slice(upper);
        }
        if let Some(seed) = seed {
            for (d, mask) in result.domains.iter_mut().zip(seed) {
                *d &= mask;
            }
        }
        let mut min = points.to_vec();
        let mut max = min.clone();
        let mut hm = vec![0; games.len()];
        let mut hx = hm.clone();
        let mut am = hm.clone();
        let mut ax = hm.clone();
        loop {
            min.copy_from_slice(points);
            max.copy_from_slice(points);
            for (i, g) in games.iter().enumerate() {
                let d = result.domains[i];
                if d == 0 {
                    result.feasible = false;
                    return result;
                }
                hm[i] = min_gain(d, g.hg);
                hx[i] = max_gain(d, g.hg);
                am[i] = min_gain(d, g.ag);
                ax[i] = max_gain(d, g.ag);
                min[g.home] += hm[i];
                min[g.away] += am[i];
                max[g.home] += hx[i];
                max[g.away] += ax[i];
            }
            let above = rivals.iter().filter(|t| min[**t] > target).count();
            let below = rivals.iter().filter(|t| max[**t] < target).count();
            if above > rank || below > rivals.len() - rank {
                result.feasible = false;
                return result;
            }
            for &t in rivals {
                if above == rank && min[t] <= target {
                    result.upper[t] = result.upper[t].min(target);
                }
                if below == rivals.len() - rank && max[t] >= target {
                    result.lower[t] = result.lower[t].max(target);
                }
                if min[t] > result.upper[t] || max[t] < result.lower[t] {
                    result.feasible = false;
                    return result;
                }
            }
            let mut changed = false;
            for (i, g) in games.iter().enumerate() {
                let mut d = result.domains[i];
                for o in 0..3 {
                    if d & (1 << o) == 0 {
                        continue;
                    }
                    if min[g.home] - hm[i] + g.hg[o] > result.upper[g.home]
                        || max[g.home] - hx[i] + g.hg[o] < result.lower[g.home]
                        || min[g.away] - am[i] + g.ag[o] > result.upper[g.away]
                        || max[g.away] - ax[i] + g.ag[o] < result.lower[g.away]
                    {
                        d &= !(1 << o);
                        changed = true;
                    }
                }
                result.domains[i] = d;
            }
            if !changed {
                break;
            }
        }
        result.compile(games, points);
        result
    }
    fn compile(&mut self, games: &[RankGame], points: &[i32]) {
        self.min = vec![vec![0; points.len()]; games.len() + 1];
        self.max = self.min.clone();
        for step in (0..games.len()).rev() {
            self.min[step] = self.min[step + 1].clone();
            self.max[step] = self.max[step + 1].clone();
            let g = &games[step];
            let d = self.domains[step];
            self.min[step][g.home] += min_gain(d, g.hg);
            self.min[step][g.away] += min_gain(d, g.ag);
            self.max[step][g.home] += max_gain(d, g.hg);
            self.max[step][g.away] += max_gain(d, g.ag);
        }
        // These bounds stay fixed for this compiled assignment. Loading one
        // endpoint record avoids walking six vectors on every proposal draw.
        self.prefix_bounds = games
            .iter()
            .enumerate()
            .map(|(step, g)| {
                [
                    i64::from(self.lower[g.home]) - i64::from(self.max[step + 1][g.home]),
                    i64::from(self.upper[g.home]) - i64::from(self.min[step + 1][g.home]),
                    i64::from(self.lower[g.away]) - i64::from(self.max[step + 1][g.away]),
                    i64::from(self.upper[g.away]) - i64::from(self.min[step + 1][g.away]),
                ]
            })
            .collect();
        self.compact(games, points);
    }
    fn compact(&mut self, games: &[RankGame], points: &[i32]) {
        self.mass = 1.;
        self.forced.clear();
        self.variable.clear();
        if !(0..games.len()).any(|i| self.domains[i].count_ones() == 1 || self.omitted[i]) {
            return;
        }
        let last = games.iter().map(|g| g.index).max().unwrap_or(0);
        self.omitted_scores = vec![false; last + 1];
        self.omitted_count = 0;
        for (i, g) in games.iter().enumerate() {
            self.omitted_scores[g.index] = self.omitted[i];
            self.omitted_count += usize::from(self.omitted[i]);
        }
        let mut future = vec![0; points.len()];
        for (step, g) in games.iter().enumerate().rev() {
            let d = self.domains[step];
            if d.count_ones() == 1 || self.omitted[step] {
                let o = d.trailing_zeros() as usize;
                future[g.home] += g.hg[o];
                future[g.away] += g.ag[o];
            } else {
                self.variable.push(Variable {
                    step,
                    skipped: 0,
                    hf: future[g.home],
                    af: future[g.away],
                });
            }
        }
        self.base = points.iter().zip(future).map(|(p, g)| p + g).collect();
        self.variable.reverse();
        let mut skipped = 0;
        let mut variable = 0;
        for (step, g) in games.iter().enumerate() {
            let d = self.domains[step];
            if d.count_ones() == 1 || self.omitted[step] {
                let o = d.trailing_zeros() as u8;
                self.forced.push((g.index, o));
                if !self.omitted[step] {
                    self.mass *= g.prob[o as usize];
                }
                skipped += 1;
            } else {
                self.variable[variable].skipped = skipped;
                variable += 1;
                skipped = 0;
            }
        }
        self.after = skipped;
    }
    #[inline]
    pub fn allows(&self, step: usize, g: &RankGame, o: usize, points: &[i32]) -> bool {
        self.allows_prefix(step, g, o, points[g.home], points[g.away])
    }
    #[inline]
    pub fn allows_prefix(&self, step: usize, g: &RankGame, o: usize, hp: i32, ap: i32) -> bool {
        if self.domains[step] & (1 << o) == 0 {
            return false;
        }
        if !self.restricted {
            return true;
        }
        let h = hp + g.hg[o];
        let a = ap + g.ag[o];
        h + self.min[step + 1][g.home] <= self.upper[g.home]
            && h + self.max[step + 1][g.home] >= self.lower[g.home]
            && a + self.min[step + 1][g.away] <= self.upper[g.away]
            && a + self.max[step + 1][g.away] >= self.lower[g.away]
    }
    /// Load the shared endpoint bounds once for the three outcome checks.
    #[inline]
    pub fn prefix_mask(&self, step: usize, g: &RankGame, hp: i32, ap: i32) -> u8 {
        let mut mask = self.domains[step];
        if self.restricted {
            let [hlo, hhi, alo, ahi] = self.prefix_bounds[step];
            let hlo = hlo - i64::from(hp);
            let hhi = hhi - i64::from(hp);
            let alo = alo - i64::from(ap);
            let ahi = ahi - i64::from(ap);
            for o in 0..3 {
                if mask & (1 << o) == 0 {
                    continue;
                }
                let h = i64::from(g.hg[o]);
                let a = i64::from(g.ag[o]);
                if !(h <= hhi && h >= hlo && a <= ahi && a >= alo) {
                    mask &= !(1 << o);
                }
            }
        }
        mask
    }
    pub fn zero_guide_safe(
        &self,
        games: &[RankGame],
        points: &[i32],
        target: i32,
        suffix: &Suffix,
        below: f64,
        above: f64,
    ) -> bool {
        for (step, &omitted) in self.omitted.iter().enumerate() {
            if !omitted {
                continue;
            }
            let g = &games[step];
            let factors = [g.home, g.away].map(|t| {
                if points[t] + self.min[0][t] > target {
                    above
                } else {
                    below
                }
            });
            if g.prob
                .iter()
                .any(|p| *p > 0. && !(*p * factors[0] * factors[1] > 0.))
            {
                return false;
            }
        }
        let mut min = points.to_vec();
        let mut max = min.clone();
        for (step, g) in games.iter().enumerate() {
            let d = self.domains[step];
            if d.count_ones() == 1 {
                let o = d.trailing_zeros() as usize;
                let mut factors = [0.; 2];
                for (side, (t, gain)) in [(g.home, g.hg[o]), (g.away, g.ag[o])]
                    .into_iter()
                    .enumerate()
                {
                    let mut lo = min[t];
                    let mut hi = max[t];
                    if self.restricted {
                        lo = lo.max(self.lower[t] - self.max[step][t]);
                        hi = hi.min(self.upper[t] - self.min[step][t]);
                    }
                    if lo > hi {
                        return false;
                    }
                    for prefix in lo..=hi {
                        let f = suffix.factor(step + 1, t, target - prefix - gain, below, above);
                        if !(f > 0.) {
                            return false;
                        }
                        if prefix == lo || f < factors[side] {
                            factors[side] = f;
                        }
                    }
                }
                if !(g.prob[o] * factors[0] * factors[1] > 0.) {
                    return false;
                }
            }
            min[g.home] += min_gain(d, g.hg);
            min[g.away] += min_gain(d, g.ag);
            max[g.home] += max_gain(d, g.hg);
            max[g.away] += max_gain(d, g.ag);
        }
        true
    }
}
#[derive(Clone, Copy, Default)]
pub struct ProbeConfig {
    pub checks: usize,
    pub nodes: usize,
}
#[derive(Default)]
pub struct ProbeStats {
    pub checks: usize,
    pub nodes: usize,
    pub removed: usize,
    pub forced: usize,
    pub infeasible: usize,
    pub elapsed_ms: f64,
}
impl Domains {
    fn probe(
        &mut self,
        games: &[RankGame],
        points: &[i32],
        rivals: &[usize],
        rank: usize,
        target: usize,
        stride: i32,
        budget: &mut usize,
        nodes: usize,
        stats: &mut ProbeStats,
    ) {
        if !self.feasible || *budget == 0 {
            return;
        }
        let started = std::time::Instant::now();
        let before = self.domains.iter().filter(|d| d.count_ones() == 1).count();
        let mut order: Vec<_> = (0..games.len()).collect();
        order.sort_by_key(|i| {
            let g = &games[*i];
            (
                self.domains[*i].count_ones(),
                (points[g.home] - points[target])
                    .abs()
                    .min((points[g.away] - points[target]).abs()),
                *i,
            )
        });
        for i in order {
            if self.domains[i].count_ones() < 2 {
                continue;
            }
            for o in 0..3 {
                if *budget == 0 || self.domains[i] & (1 << o) == 0 {
                    continue;
                }
                *budget -= 1;
                stats.checks += 1;
                let mut masks = self.domains.clone();
                masks[i] = 1 << o;
                let trial = Self::seeded(
                    games,
                    points,
                    rivals,
                    rank,
                    points[target],
                    Some(&masks),
                    None,
                );
                let mut impossible = !trial.feasible;
                if !impossible && nodes > 0 {
                    // Fixed W/D/L results contribute to the base exactly once.
                    // Residual domains are a relaxation; score tiebreakers stay free.
                    let mut base = points.to_vec();
                    let mut residual = Vec::new();
                    let mut ds = Vec::new();
                    for (g, d) in games.iter().zip(&trial.domains) {
                        if d.count_ones() == 1 {
                            let o = d.trailing_zeros() as usize;
                            base[g.home] += g.hg[o];
                            base[g.away] += g.ag[o];
                        } else {
                            residual.push(crate::proof::Game {
                                home: g.home,
                                away: g.away,
                                hg: g.hg,
                                ag: g.ag,
                                prob: g.prob,
                            });
                            ds.push(*d);
                        }
                    }
                    let encoded =
                        crate::proof::Problem::conditional(base.clone(), residual.clone());
                    if let Some(p) = encoded {
                        let (bad, n) =
                            p.impossible_domains(target, rank, points[target], &ds, nodes);
                        stats.nodes += n;
                        impossible = bad;
                        if !impossible {
                            let (bad, n) = p.negated().impossible_domains(
                                target,
                                rivals.len() - rank,
                                -points[target],
                                &ds,
                                nodes,
                            );
                            stats.nodes += n;
                            impossible = bad;
                        }
                    }
                    if !impossible && stride > 1 {
                        for b in &mut base {
                            *b = b.div_euclid(stride);
                        }
                        for g in &mut residual {
                            g.hg = g.hg.map(|v| v.div_euclid(stride));
                            g.ag = g.ag.map(|v| v.div_euclid(stride));
                        }
                        if let Some(p) = crate::proof::Problem::conditional(base, residual) {
                            let (bad, n) = p.negated().impossible_domains(
                                target,
                                rivals.len() - rank,
                                -points[target].div_euclid(stride),
                                &ds,
                                nodes,
                            );
                            stats.nodes += n;
                            impossible = bad;
                        }
                    }
                }
                if impossible {
                    stats.removed += 1;
                    let mut masks = self.domains.clone();
                    masks[i] &= !(1 << o);
                    *self = Self::seeded(
                        games,
                        points,
                        rivals,
                        rank,
                        points[target],
                        Some(&masks),
                        None,
                    );
                    if !self.feasible {
                        stats.infeasible += 1;
                        break;
                    }
                }
            }
            if !self.feasible {
                break;
            }
        }
        if self.feasible {
            stats.forced += self
                .domains
                .iter()
                .filter(|d| d.count_ones() == 1)
                .count()
                .saturating_sub(before);
        }
        stats.elapsed_ms += started.elapsed().as_secs_f64() * 1000.;
    }
}
pub struct Cache<'a> {
    games: &'a [RankGame],
    selected: &'a [RankGame],
    rivals: Vec<usize>,
    rank: usize,
    min: Vec<i32>,
    max: Vec<i32>,
    entries: IntMap<Option<Arc<Domains>>>,
    pub zero_safe: IntMap<bool>,
    reduce: bool,
    propagate: bool,
    target: usize,
    stride: i32,
    probes: ProbeConfig,
    pub stats: ProbeStats,
}
impl<'a> Cache<'a> {
    pub fn new(
        games: &'a [RankGame],
        selected: &'a [RankGame],
        target: usize,
        rank: usize,
        teams: usize,
        reduce: bool,
        propagate: bool,
    ) -> Option<Self> {
        if selected.len() > 32 {
            return None;
        }
        let mut min = vec![0; teams];
        let mut max = min.clone();
        for g in games {
            let d = (0..3).fold(0, |d, o| if g.prob[o] > 0. { d | (1 << o) } else { d });
            min[g.home] += min_gain(d, g.hg);
            min[g.away] += min_gain(d, g.ag);
            max[g.home] += max_gain(d, g.hg);
            max[g.away] += max_gain(d, g.ag);
        }
        Some(Self {
            games,
            selected,
            rivals: (0..teams).filter(|t| *t != target).collect(),
            rank,
            min,
            max,
            entries: IntMap::default(),
            zero_safe: IntMap::default(),
            reduce,
            propagate,
            target,
            stride: 1,
            probes: ProbeConfig::default(),
            stats: ProbeStats::default(),
        })
    }
    pub fn with_probes(mut self, stride: i32, probes: ProbeConfig) -> Self {
        self.stride = stride;
        self.probes = probes;
        self
    }
    pub fn get(
        &mut self,
        points: &[i32],
        target: i32,
        outcomes: &[u8],
    ) -> (u64, Option<Arc<Domains>>) {
        let key = self
            .selected
            .iter()
            .enumerate()
            .fold(0, |k, (i, g)| k | ((outcomes[g.index] as u64) << (2 * i)));
        if let Some(r) = self.entries.get(&key) {
            return (key, r.clone());
        }
        if self.entries.len() >= 64 {
            return (key, None);
        }
        let above = self
            .rivals
            .iter()
            .filter(|t| points[**t] + self.min[**t] > target)
            .count();
        let below = self
            .rivals
            .iter()
            .filter(|t| points[**t] + self.max[**t] < target)
            .count();
        let settled: Vec<_> = (0..points.len())
            .map(|t| {
                self.rivals.contains(&t)
                    && (points[t] + self.min[t] > target || points[t] + self.max[t] < target)
            })
            .collect();
        let omitted: Vec<_> = self
            .games
            .iter()
            .map(|g| self.reduce && settled[g.home] && settled[g.away])
            .collect();
        let has_omitted = omitted.iter().any(|o| *o);
        let result = if self.propagate
            && (above >= self.rank || below >= self.rivals.len() - self.rank)
            || has_omitted
            || self.probes.checks > 0
        {
            let mut d = if self.propagate {
                Domains::propagate(self.games, points, &self.rivals, self.rank, target)
            } else {
                let mut d = Domains::new(self.games, points.len(), false);
                d.compile(self.games, points);
                d
            };
            if self.probes.checks > 0 {
                d.probe(
                    self.games,
                    points,
                    &self.rivals,
                    self.rank,
                    self.target,
                    self.stride,
                    &mut self.probes.checks,
                    self.probes.nodes,
                    &mut self.stats,
                );
            }
            if d.feasible && has_omitted {
                d.omitted = omitted;
                d.compact(self.games, points);
            }
            Some(Arc::new(d))
        } else {
            None
        };
        self.entries.insert(key, result.clone());
        (key, result)
    }
}
