//! Conditional score importance proposals on the existing model's score
//! support. Outcome probabilities are unchanged; all goal tilts use P/Q.
use crate::rare_tail::budget::Operations;
use crate::{
    conditioned::canonical,
    model::{Campaign, Key, Model},
    rng::Rng,
    sampling::masses,
    search::Cell,
};
use std::collections::HashMap;
const BETAS: [f64; 9] = [-3., -2., -1., -0.5, 0., 0.5, 1., 2., 3.];
const MAX_JOINT_GUIDES: usize = 6;
const MAX_JOINT_SELECTED_FIXTURES: usize = 96;
const MAX_JOINT_PATH_CACHE: usize = 256;
const MAX_JOINT_FIT_WORK: usize = 1_000_000;
const MAX_JOINT_TOTAL_WORK: usize = 10_000_000;
const MAX_JOINT_SCORE_ENTRIES: usize = 60_000;
const MAX_JOINT_CACHED_SCORE_ENTRIES: usize = 300_000;
const MAX_JOINT_MODEL_FIXTURES: usize = 128;
pub(crate) const JOINT_PREPARATION_ALLOWANCE_PER_DRAW: usize = 1_024;
pub(crate) const JOINT_DENSITY_WORK_PER_FIXTURE: usize =
    2 * (MAX_JOINT_GUIDES + 1) + 2 * MAX_JOINT_GUIDES + 17;
#[derive(Clone)]
struct Table {
    values: [Vec<([i32; 2], f64)>; 3],
    log_z: [f64; 3],
    mean: [f64; 3],
}
pub struct Context {
    campaign: Vec<Campaign>,
    scores: Vec<[i32; 2]>,
    order: Vec<usize>,
    ties: Vec<i32>,
    joint: HashMap<Vec<u8>, Option<JointProposal>>,
    joint_work: usize,
    joint_score_entries: usize,
    joint_exhausted: bool,
    joint_work_limit: usize,
}
impl Context {
    pub fn new(m: &Model) -> Self {
        Self {
            campaign: m.base.clone(),
            scores: m.empty_scores(),
            order: vec![0; m.n],
            ties: Vec::with_capacity(m.n),
            joint: HashMap::new(),
            joint_work: 0,
            joint_score_entries: 0,
            joint_exhausted: false,
            joint_work_limit: MAX_JOINT_TOTAL_WORK,
        }
    }
    pub fn with_sample_budget(m: &Model, samples: usize) -> Self {
        let mut context = Self::new(m);
        context.joint_work_limit = samples
            .saturating_mul(JOINT_PREPARATION_ALLOWANCE_PER_DRAW)
            .min(MAX_JOINT_TOTAL_WORK);
        context
    }
}
#[derive(Clone)]
struct JointProposal {
    fixtures: Vec<JointFixture>,
    lambdas: Vec<f64>,
    coefficients: Vec<Vec<[f64; 2]>>,
}
#[derive(Clone)]
struct JointFixture {
    values: [Vec<([i32; 2], f64)>; 3],
    log_z: [f64; 3],
}
impl JointProposal {
    fn score_entries(&self) -> usize {
        self.fixtures
            .iter()
            .map(|fixture| fixture.values.iter().map(Vec::len).sum::<usize>())
            .sum()
    }
}
#[derive(Clone)]
pub struct GoalTilt {
    cell: Cell,
    tables: Vec<Vec<Table>>,
    joint_enabled: bool,
}
impl GoalTilt {
    pub(crate) fn guidance_eligible(m: &Model) -> bool {
        m.keys.first() == Some(&Key::Pt)
            && m.keys.contains(&Key::Gd)
            && !m
                .fixtures
                .iter()
                .any(|f| f.home_sampler.mean > 32. || f.away_sampler.mean > 32.)
    }
    fn globally_guidable_key(m: &Model) -> Option<Key> {
        let mut start = 1;
        if m.keys.get(1) == Some(&Key::W) {
            start = 2;
        }
        for key in &m.keys[start..] {
            match key {
                Key::Gd | Key::Gf | Key::Away => return Some(*key),
                // A head-to-head key can select a contextual mini-table and a
                // different fixture scope; keep those models eligible.
                Key::Head | Key::Average | Key::Random | Key::Name => return None,
                _ => {}
            }
        }
        None
    }
    fn joint_guidance_may_fit(m: &Model, target: Option<usize>) -> bool {
        if !Self::guidance_eligible(m) {
            return false;
        }
        let Some(_) = Self::globally_guidable_key(m) else {
            return true;
        };
        // A later unsupported key can be reached when an earlier GD/GF
        // comparison is constant under the WDL path, so only use this proof
        // when the remaining comparator is globally score-based.
        let mut start = if m.keys.get(1) == Some(&Key::W) { 2 } else { 1 };
        while start < m.keys.len() && !matches!(m.keys[start], Key::Gd | Key::Gf | Key::Away) {
            start += 1;
        }
        if m.keys[start..]
            .iter()
            .any(|key| matches!(key, Key::Head | Key::Average))
        {
            return true;
        }
        // Same-team fixtures can cancel from GD and GF comparisons in
        // different ways; keep such models on the ordinary bounded fitter.
        if m.fixtures.iter().any(|f| f.home == f.away) {
            return true;
        }
        let support = m.joint_score_support_costs();
        let mut best_raw = usize::MAX;
        let mut consider = |key: Key, a: usize, b: usize| {
            let raw = if key == Key::Away {
                support.away[a].saturating_add(support.away[b])
            } else {
                let pair = if a < b { (a, b) } else { (b, a) };
                support.incident[a]
                    .saturating_add(support.incident[b])
                    .saturating_sub(*support.shared.get(&pair).unwrap_or(&0))
            };
            best_raw = best_raw.min(raw);
        };
        let keys: Vec<Key> = m.keys[start..]
            .iter()
            .copied()
            .filter(|key| matches!(key, Key::Gd | Key::Gf | Key::Away))
            .collect();
        match target {
            Some(target) => {
                for rival in 0..m.n {
                    if rival != target {
                        for &key in &keys {
                            consider(key, target, rival);
                        }
                    }
                }
            }
            None => {
                for a in 0..m.n {
                    for b in a + 1..m.n {
                        for &key in &keys {
                            consider(key, a, b);
                        }
                    }
                }
            }
        }
        best_raw.saturating_mul(200) <= MAX_JOINT_FIT_WORK
    }
    pub(crate) fn joint_guidance_may_fit_for_any_target(m: &Model) -> bool {
        Self::joint_guidance_may_fit(m, None)
    }
    pub(crate) fn clone_work(&self) -> usize {
        self.tables
            .iter()
            .flatten()
            .map(|table| table.values.iter().map(Vec::len).sum::<usize>() + 6)
            .sum()
    }
    pub fn new(m: &Model, cell: Cell) -> Option<Self> {
        if !Self::guidance_eligible(m) {
            return None;
        }
        let joint_enabled = Self::joint_guidance_may_fit(m, Some(cell.team));
        let tables = m
            .fixtures
            .iter()
            .filter(|f| f.home == cell.team || f.away == cell.team)
            .map(|f| {
                let h = masses(f.home_sampler.mean);
                let a = masses(f.away_sampler.mean);
                let sign = if f.home == cell.team { 1. } else { -1. };
                BETAS
                    .iter()
                    .map(|&beta| {
                        let mut values: [Vec<([i32; 2], f64)>; 3] =
                            std::array::from_fn(|_| Vec::new());
                        let (mut raw, mut z, mut mean) = ([0.; 3], [0.; 3], [0.; 3]);
                        for (hs, &hp) in h.iter().enumerate() {
                            for (as_, &ap) in a.iter().enumerate() {
                                let o = if hs < as_ {
                                    0
                                } else if hs == as_ {
                                    1
                                } else {
                                    2
                                };
                                let p = hp * ap;
                                let d = sign * (hs as f64 - as_ as f64);
                                let q = p * (beta * d).exp();
                                raw[o] += p;
                                z[o] += q;
                                mean[o] += q * d;
                                values[o].push(([hs as i32, as_ as i32], z[o]));
                            }
                        }
                        let log_z = std::array::from_fn(|o| {
                            if raw[o] > 0. {
                                (z[o] / raw[o]).ln()
                            } else {
                                0.
                            }
                        });
                        for o in 0..3 {
                            if z[o] > 0. {
                                mean[o] /= z[o];
                                for (_, c) in &mut values[o] {
                                    *c /= z[o];
                                }
                                values[o].last_mut().unwrap().1 = 1.;
                            }
                        }
                        Table {
                            values,
                            log_z,
                            mean,
                        }
                    })
                    .collect()
            })
            .collect();
        Some(Self {
            cell,
            tables,
            joint_enabled,
        })
    }

    fn joint_proposal(
        &self,
        m: &Model,
        out: &[u8],
        operations: &mut Operations,
        work_budget: usize,
    ) -> Option<JointProposal> {
        if out.len() != m.fixtures.len() || m.fixtures.len() > MAX_JOINT_MODEL_FIXTURES {
            return None;
        }
        let start_units = operations.units();
        let sort_work =
            8 * m.fixtures.len() + m.n * (m.n.max(2).ilog2() as usize + 1) * m.keys.len().max(1);
        let copy_work = m.ids.len() + m.request.games.len() + 8 * m.fixtures.len();
        if sort_work.saturating_add(copy_work) > work_budget {
            return None;
        }
        operations.guidance = operations.guidance.saturating_add(copy_work as u64);
        let mut campaigns = m.base.clone();
        let mut scores = m.empty_scores();
        for (i, (&outcome, fixture)) in out.iter().zip(&m.fixtures).enumerate() {
            let score = canonical(outcome);
            scores[fixture.request_index] = score;
            m.add(&mut campaigns, i, score);
        }
        let wins = m.keys.get(1) == Some(&Key::W);
        let target = self.cell.team;
        let primary_cmp = |a: usize, b: usize| {
            campaigns[a].points.cmp(&campaigns[b].points).then_with(|| {
                if wins {
                    campaigns[a].wins.cmp(&campaigns[b].wins)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
        };
        let above = (0..m.n)
            .filter(|&t| t != target && primary_cmp(t, target).is_gt())
            .count();
        let tied: Vec<_> = (0..m.n)
            .filter(|&t| t != target && primary_cmp(t, target).is_eq())
            .collect();
        let slot = self.cell.rank.checked_sub(above)?;
        if tied.is_empty() || slot > tied.len() {
            return None;
        }
        let mut order = vec![0; m.n];
        let mut rng = Rng::new(0);
        operations.rank(m);
        m.standings(&mut order, &campaigns, &scores, &mut rng);
        if operations.units().saturating_sub(start_units) > work_budget {
            return None;
        }
        let ordered_ties: Vec<_> = order
            .into_iter()
            .filter(|&t| t != target && primary_cmp(t, target).is_eq())
            .collect();
        if ordered_ties.len() > MAX_JOINT_GUIDES {
            return None;
        }
        let mut guides = Vec::new();
        for (position, &rival) in ordered_ties.iter().enumerate() {
            let scan_work = m.n * (m.keys.len() + m.fixtures.len() + 1)
                + m.ids.len()
                + m.request.games.len()
                + 8 * m.fixtures.len();
            if operations
                .units()
                .saturating_sub(start_units)
                .saturating_add(scan_work)
                > work_budget
            {
                return None;
            }
            operations.guidance = operations.guidance.saturating_add(scan_work as u64);
            let Some(mut guide) = m.score_guidance(out, target, rival) else {
                return None;
            };
            let direction = if position < slot { -1. } else { 1. };
            guide.offset *= direction;
            for (_, coefficient) in &mut guide.terms {
                coefficient[0] *= direction;
                coefficient[1] *= direction;
            }
            guides.push(guide);
        }
        if guides.is_empty() || guides.len() > MAX_JOINT_GUIDES {
            return None;
        }
        let mut coefficients = vec![vec![[0.; 2]; m.fixtures.len()]; guides.len()];
        let mut used = vec![false; m.fixtures.len()];
        for (j, guide) in guides.iter().enumerate() {
            for &(fixture, coefficient) in &guide.terms {
                coefficients[j][fixture] = coefficient;
                used[fixture] = true;
            }
        }
        let selected: Vec<_> = used
            .iter()
            .enumerate()
            .filter_map(|(i, &x)| x.then_some(i))
            .collect();
        if selected.is_empty() || selected.len() > MAX_JOINT_SELECTED_FIXTURES {
            return None;
        }
        let mut raw = Vec::with_capacity(selected.len());
        let mut work = 0usize;
        for &i in &selected {
            let fixture = &m.fixtures[i];
            if fixture.home_sampler.mean > 32. || fixture.away_sampler.mean > 32. {
                return None;
            }
            let h = masses(fixture.home_sampler.mean);
            let a = masses(fixture.away_sampler.mean);
            let fixture_work = h
                .len()
                .saturating_mul(a.len())
                .saturating_add(h.len())
                .saturating_add(a.len());
            if operations
                .units()
                .saturating_sub(start_units)
                .saturating_add(fixture_work)
                > work_budget
            {
                return None;
            }
            work = work.saturating_add(fixture_work);
            operations.guidance = operations.guidance.saturating_add(fixture_work as u64);
            if work > MAX_JOINT_SCORE_ENTRIES {
                return None;
            }
            let mut by_outcome: [Vec<([i32; 2], f64)>; 3] = std::array::from_fn(|_| Vec::new());
            let wanted = out[i] as usize;
            for (hs, &hp) in h.iter().enumerate() {
                for (as_, &ap) in a.iter().enumerate() {
                    let outcome = if hs < as_ {
                        0
                    } else if hs == as_ {
                        1
                    } else {
                        2
                    };
                    if outcome != wanted {
                        continue;
                    }
                    let p = hp * ap;
                    if p > 0. {
                        by_outcome[outcome].push(([hs as i32, as_ as i32], p));
                    }
                }
            }
            let z: f64 = by_outcome[wanted].iter().map(|x| x.1).sum();
            if z <= 0. {
                return None;
            }
            for entry in &mut by_outcome[wanted] {
                entry.1 /= z;
            }
            raw.push(by_outcome);
        }
        let fit_work = work
            .saturating_mul(guides.len())
            .saturating_mul(guides.len())
            .saturating_mul(200);
        if fit_work > MAX_JOINT_FIT_WORK {
            return None;
        }
        let variance_work = work.saturating_mul(guides.len()).saturating_mul(2);
        if operations
            .units()
            .saturating_sub(start_units)
            .saturating_add(fit_work)
            .saturating_add(variance_work)
            .saturating_add(work)
            > work_budget
        {
            return None;
        }
        operations.guidance = operations
            .guidance
            .saturating_add((fit_work + variance_work + work) as u64);
        for feature in 0..guides.len() {
            let mut variance = 0.;
            for (local, &i) in selected.iter().enumerate() {
                let o = out[i] as usize;
                let coefficient = coefficients[feature][i];
                let mean: f64 = raw[local][o]
                    .iter()
                    .map(|&(score, p)| {
                        p * (coefficient[0] * score[0] as f64 + coefficient[1] * score[1] as f64)
                    })
                    .sum();
                variance += raw[local][o]
                    .iter()
                    .map(|&(score, p)| {
                        let value =
                            coefficient[0] * score[0] as f64 + coefficient[1] * score[1] as f64;
                        p * (value - mean).powi(2)
                    })
                    .sum::<f64>();
            }
            if variance <= f64::EPSILON {
                return None;
            }
        }
        let mut lambdas = vec![0.; guides.len()];
        let evaluate = |feature: usize, candidate: f64, lambdas: &[f64]| -> f64 {
            let mut total = guides[feature].offset;
            for (local, &i) in selected.iter().enumerate() {
                let o = out[i] as usize;
                let mut max_log = f64::NEG_INFINITY;
                for &(score, p) in &raw[local][o] {
                    let mut logw = p.ln();
                    for j in 0..guides.len() {
                        let lambda = if j == feature { candidate } else { lambdas[j] };
                        logw += lambda
                            * (coefficients[j][i][0] * score[0] as f64
                                + coefficients[j][i][1] * score[1] as f64);
                    }
                    max_log = max_log.max(logw);
                }
                let (mut z, mut moment) = (0., 0.);
                for &(score, p) in &raw[local][o] {
                    let mut logw = p.ln();
                    for j in 0..guides.len() {
                        let lambda = if j == feature { candidate } else { lambdas[j] };
                        logw += lambda
                            * (coefficients[j][i][0] * score[0] as f64
                                + coefficients[j][i][1] * score[1] as f64);
                    }
                    let w = (logw - max_log).exp();
                    z += w;
                    moment += w
                        * (coefficients[feature][i][0] * score[0] as f64
                            + coefficients[feature][i][1] * score[1] as f64);
                }
                if z > 0. {
                    total += moment / z;
                }
            }
            total
        };
        for _ in 0..4 {
            for feature in 0..guides.len() {
                if evaluate(feature, 0., &lambdas) >= 0. {
                    lambdas[feature] = 0.;
                    continue;
                }
                let mut high = 1.;
                while high < 64. && evaluate(feature, high, &lambdas) < 0. {
                    high *= 2.;
                }
                if evaluate(feature, high, &lambdas) < 0. {
                    return None;
                }
                let mut low = 0.;
                for _ in 0..16 {
                    let mid = (low + high) * 0.5;
                    if evaluate(feature, mid, &lambdas) < 0. {
                        low = mid;
                    } else {
                        high = mid;
                    }
                }
                lambdas[feature] = (low + high) * 0.5;
                if !lambdas[feature].is_finite() {
                    return None;
                }
            }
        }
        // Charge bounded optimization passes and materialization.
        let mut fixtures = Vec::with_capacity(selected.len());
        for (local, &i) in selected.iter().enumerate() {
            let mut values: [Vec<([i32; 2], f64)>; 3] = std::array::from_fn(|_| Vec::new());
            let mut log_z = [0.; 3];
            for o in 0..3 {
                if o != out[i] as usize {
                    continue;
                }
                let max_log = raw[local][o]
                    .iter()
                    .map(|&(score, p)| {
                        p.ln()
                            + (0..guides.len())
                                .map(|j| {
                                    lambdas[j]
                                        * (coefficients[j][i][0] * score[0] as f64
                                            + coefficients[j][i][1] * score[1] as f64)
                                })
                                .sum::<f64>()
                    })
                    .fold(f64::NEG_INFINITY, f64::max);
                let z: f64 = raw[local][o]
                    .iter()
                    .map(|&(score, p)| {
                        (p.ln()
                            + (0..guides.len())
                                .map(|j| {
                                    lambdas[j]
                                        * (coefficients[j][i][0] * score[0] as f64
                                            + coefficients[j][i][1] * score[1] as f64)
                                })
                                .sum::<f64>()
                            - max_log)
                            .exp()
                    })
                    .sum();
                log_z[o] = max_log + z.ln();
                for &(score, p) in &raw[local][o] {
                    let logw = p.ln()
                        + (0..guides.len())
                            .map(|j| {
                                lambdas[j]
                                    * (coefficients[j][i][0] * score[0] as f64
                                        + coefficients[j][i][1] * score[1] as f64)
                            })
                            .sum::<f64>();
                    values[o].push((score, (logw - log_z[o]).exp()));
                }
                let mut cumulative = 0.;
                for (_, probability) in &mut values[o] {
                    cumulative += *probability;
                    *probability = cumulative;
                }
                if let Some((_, last)) = values[o].last_mut() {
                    *last = 1.;
                }
            }
            fixtures.push(JointFixture { values, log_z });
        }
        Some(JointProposal {
            fixtures,
            lambdas,
            coefficients,
        })
    }

    pub fn rank_with_operations(
        &self,
        m: &Model,
        out: &[u8],
        rng: &mut Rng,
        context: &mut Context,
        operations: &mut Operations,
    ) -> (usize, f64) {
        if !self.joint_enabled {
            return self.rank_marginal(m, out, rng, context);
        }
        let key = out.to_vec();
        if !context.joint.contains_key(&key) {
            if context.joint_exhausted
                || context.joint.len() >= MAX_JOINT_PATH_CACHE
                || context.joint_work >= context.joint_work_limit
                || context.joint_score_entries >= MAX_JOINT_CACHED_SCORE_ENTRIES
            {
                context.joint_exhausted = true;
                operations.guidance = operations
                    .guidance
                    .saturating_add((m.fixtures.len() * (MAX_JOINT_GUIDES + 1)) as u64);
                return self.rank_marginal(m, out, rng, context);
            }
            let before = operations.units();
            let remaining = context.joint_work_limit.saturating_sub(context.joint_work);
            let proposal = self.joint_proposal(m, out, operations, remaining);
            let spent = operations.units().saturating_sub(before);
            let entries = proposal.as_ref().map_or(0, JointProposal::score_entries);
            if context.joint_work.saturating_add(spent) > context.joint_work_limit
                || context.joint_work.saturating_add(spent) > MAX_JOINT_TOTAL_WORK
                || context.joint_score_entries.saturating_add(entries)
                    > MAX_JOINT_CACHED_SCORE_ENTRIES
            {
                context.joint_exhausted = true;
                if context.joint.len() < MAX_JOINT_PATH_CACHE {
                    context.joint.insert(key.clone(), None);
                }
                return self.rank_marginal(m, out, rng, context);
            }
            context.joint_work = context.joint_work.saturating_add(spent);
            context.joint_score_entries = context.joint_score_entries.saturating_add(entries);
            context.joint.insert(key.clone(), proposal);
        } else {
            operations.guidance = operations
                .guidance
                .saturating_add((m.fixtures.len() * (MAX_JOINT_GUIDES + 1)) as u64);
        }
        let Some(proposal) = context.joint.get(&key).and_then(Option::as_ref) else {
            operations.guidance = operations
                .guidance
                .saturating_add((m.fixtures.len() * (MAX_JOINT_GUIDES + 1)) as u64);
            return self.rank_marginal(m, out, rng, context);
        };
        let c = &mut context.campaign;
        c.copy_from_slice(&m.base);
        let scores = &mut context.scores;
        let mut selected = 0usize;
        let mut log_ratio = 0.;
        let sample_q = rng.float() >= 0.1;
        for (i, fixture) in m.fixtures.iter().enumerate() {
            let outcome = out[i] as usize;
            let score_guide_count = proposal.coefficients.len();
            let tilted = proposal.coefficients.iter().any(|v| v[i] != [0., 0.]);
            operations.guidance = operations.guidance.saturating_add(score_guide_count as u64);
            let (score, logz) = if tilted {
                let table = &proposal.fixtures[selected];
                selected += 1;
                if sample_q {
                    let values = &table.values[outcome];
                    let u = rng.float();
                    operations.guidance = operations
                        .guidance
                        .saturating_add(values.len().max(1).ilog2() as u64 + 1);
                    let ix = values
                        .partition_point(|(_, p)| *p < u)
                        .min(values.len() - 1);
                    (values[ix].0, table.log_z[outcome])
                } else {
                    (fixture.scores.sample(outcome, rng), table.log_z[outcome])
                }
            } else {
                (fixture.scores.sample(outcome, rng), 0.)
            };
            if tilted {
                log_ratio -= logz;
                operations.guidance = operations.guidance.saturating_add(score_guide_count as u64);
                for j in 0..proposal.lambdas.len() {
                    log_ratio += proposal.lambdas[j]
                        * (proposal.coefficients[j][i][0] * score[0] as f64
                            + proposal.coefficients[j][i][1] * score[1] as f64);
                }
            }
            scores[fixture.request_index] = score;
            m.add(c, i, score);
        }
        let mut order = &mut context.order;
        m.standings(&mut order, c, scores, rng);
        let rank = order.iter().position(|&t| t == self.cell.team).unwrap();
        operations.guidance = operations.guidance.saturating_add(selected as u64);
        let ratio = 1. / (0.1 + 0.9 * log_ratio.exp());
        (rank, ratio)
    }
    fn rank_marginal(
        &self,
        m: &Model,
        out: &[u8],
        rng: &mut Rng,
        context: &mut Context,
    ) -> (usize, f64) {
        let c = &mut context.campaign;
        c.copy_from_slice(&m.base);
        let scores = &mut context.scores;
        let order = &mut context.order;
        for (i, _) in m.fixtures.iter().enumerate() {
            m.add(c, i, canonical(out[i]));
        }
        let wins = m.keys.get(1) == Some(&Key::W);
        let target = self.cell.team;
        let compare = |a: &Campaign, b: &Campaign| {
            a.points.cmp(&b.points).then_with(|| {
                if wins {
                    a.wins.cmp(&b.wins)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
        };
        let above = (0..m.n)
            .filter(|&t| t != target && compare(&c[t], &c[target]).is_gt())
            .count();
        let ties = &mut context.ties;
        ties.clear();
        ties.extend(
            (0..m.n)
                .filter(|&t| t != target && compare(&c[t], &c[target]).is_eq())
                .map(|t| c[t].gf - c[t].ga),
        );
        if ties.is_empty() {
            return (above, 1.);
        }
        if self.cell.rank < above || self.cell.rank > above + ties.len() {
            return (above, 1.);
        }
        ties.sort_by(|a, b| b.cmp(a));
        let k = self.cell.rank - above;
        let desired = if k == 0 {
            ties[0] as f64 + 1.
        } else if k == ties.len() {
            ties[k - 1] as f64 - 1.
        } else {
            (ties[k - 1] + ties[k]) as f64 * 0.5
        };
        let mut means = [(m.base[target].gf - m.base[target].ga) as f64; 9];
        let mut ix = 0;
        for (i, f) in m.fixtures.iter().enumerate() {
            if f.home == target || f.away == target {
                for b in 0..9 {
                    means[b] += self.tables[ix][b].mean[out[i] as usize];
                }
                ix += 1;
            }
        }
        let chosen = (0..9)
            .min_by(|&a, &b| {
                (means[a] - desired)
                    .abs()
                    .total_cmp(&(means[b] - desired).abs())
            })
            .unwrap();
        let drawn = if rng.float() < 0.1 { 4 } else { chosen };
        c.copy_from_slice(&m.base);
        ix = 0;
        let (mut log_z, mut gain) = (0., 0.);
        for (i, f) in m.fixtures.iter().enumerate() {
            let o = out[i] as usize;
            let score = if f.home == target || f.away == target {
                let table = &self.tables[ix][drawn];
                let v = &table.values[o];
                let u = rng.float();
                let score = v[v.partition_point(|(_, p)| *p < u).min(v.len() - 1)].0;
                log_z += self.tables[ix][chosen].log_z[o];
                gain += if f.home == target {
                    score[0] - score[1]
                } else {
                    score[1] - score[0]
                } as f64;
                ix += 1;
                score
            } else {
                f.scores.sample(o, rng)
            };
            scores[f.request_index] = score;
            m.add(c, i, score);
        }
        m.standings(order, c, scores, rng);
        let density = (BETAS[chosen] * gain - log_z).exp();
        (
            order.iter().position(|&t| t == target).unwrap(),
            1. / (0.1 + 0.9 * density),
        )
    }

    pub fn rank(
        &self,
        m: &Model,
        out: &[u8],
        rng: &mut Rng,
        context: &mut Context,
    ) -> (usize, f64) {
        self.rank_with_operations(m, out, rng, context, &mut Operations::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Request;
    use serde_json::json;

    #[test]
    fn production_comparator_guidance_captures_recursive_head_goal_coefficients() {
        let request: Request =
            serde_json::from_str(include_str!("../tests/fixtures/group-17059-head-gd.json"))
                .unwrap();
        let model = Model::new(request).unwrap();
        let team = |id| model.indices[&id];
        let outcomes = [2, 2];
        let cb = model
            .score_guidance(&outcomes, team(2694), team(2688))
            .unwrap();
        assert_eq!(cb.offset, -18.);
        let by_id: HashMap<_, _> = cb
            .terms
            .iter()
            .map(|(i, coefficient)| {
                (
                    model.request.games[model.fixtures[*i].request_index].id,
                    *coefficient,
                )
            })
            .collect();
        assert_eq!(by_id[&370691], [1., -1.]);
        assert_eq!(by_id[&370700], [1., -1.]);

        let ca = model
            .score_guidance(&outcomes, team(2694), team(2676))
            .unwrap();
        assert_eq!(ca.offset, -9.);
        let by_id: HashMap<_, _> = ca
            .terms
            .iter()
            .map(|(i, coefficient)| {
                (
                    model.request.games[model.fixtures[*i].request_index].id,
                    *coefficient,
                )
            })
            .collect();
        assert_eq!(by_id[&370691], [-1., 1.]);
        assert_eq!(by_id[&370700], [2., -2.]);
    }

    #[test]
    fn unaffordable_global_goal_guidance_uses_exact_marginal_path() {
        let request: Request = serde_json::from_str(include_str!(
            "../../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json"
        ))
        .unwrap();
        let model = Model::new(request).unwrap();
        assert!(GoalTilt::guidance_eligible(&model));
        assert!(!GoalTilt::joint_guidance_may_fit_for_any_target(&model));

        let cell = Cell { team: 0, rank: 0 };
        let proposal = GoalTilt::new(&model, cell).unwrap();
        assert!(!proposal.joint_enabled);
        let outcomes = vec![2; model.fixtures.len()];
        let mut joint_rng = Rng::new(808);
        let mut marginal_rng = Rng::new(808);
        let mut joint_context = Context::new(&model);
        let mut marginal_context = Context::new(&model);
        let mut operations = Operations::default();
        let actual = proposal.rank_with_operations(
            &model,
            &outcomes,
            &mut joint_rng,
            &mut joint_context,
            &mut operations,
        );
        let expected =
            proposal.rank_marginal(&model, &outcomes, &mut marginal_rng, &mut marginal_context);
        assert_eq!(actual.0, expected.0);
        assert_eq!(actual.1.to_bits(), expected.1.to_bits());
        assert_eq!(joint_rng.float().to_bits(), marginal_rng.float().to_bits());
        assert_eq!(operations.units(), 0);
        assert!(joint_context.joint.is_empty());
        assert_eq!(joint_context.joint_work, 0);
    }

    #[test]
    fn constant_draw_goal_difference_continues_to_goal_for_and_random_first_falls_back() {
        let req = |sort: &str| {
            serde_json::from_value::<Request>(json!({
                "id": 1,
                "phase": {"sort": sort, "championship": {"point_win": 3, "point_draw": 1, "point_loss": 0}},
                "team_groups": (0..4).map(|t| json!({"team_id":t,"add_sub":0,"bias":0})).collect::<Vec<_>>(),
                "games": [
                    {"id":1,"home_id":0,"away_id":2,"home_power":1.2,"away_power":0.8},
                    {"id":2,"home_id":1,"away_id":3,"home_power":0.7,"away_power":1.1}
                ]
            })).unwrap()
        };
        let model = Model::new(req("pt,gd,gf,name")).unwrap();
        let guidance = model.score_guidance(&[1, 1], 0, 1).unwrap();
        assert_eq!(guidance.terms[0].1, [1., 0.]);
        assert_eq!(guidance.terms[1].1, [-1., 0.]);
        let random_first = Model::new(req("pt,name,gd,gf")).unwrap();
        assert!(random_first.score_guidance(&[1, 1], 0, 1).is_none());
    }

    #[test]
    fn joint_mixture_likelihood_is_normalized_and_matches_independent_c2_probability() {
        let request: Request =
            serde_json::from_str(include_str!("../tests/fixtures/group-17059-head-gd.json"))
                .unwrap();
        let model = Model::new(request.clone()).unwrap();
        let target = model.indices[&2694];
        let proposal = GoalTilt::new(
            &model,
            Cell {
                team: target,
                rank: 1,
            },
        )
        .unwrap();
        let outcomes = [2, 2];
        let a_b = request.games.iter().find(|g| g.id == 370691).unwrap();
        let c_a = request.games.iter().find(|g| g.id == 370700).unwrap();
        let ah = masses(a_b.home_power);
        let ba = masses(a_b.away_power);
        let ch = masses(c_a.home_power);
        let aa = masses(c_a.away_power);
        let mut oracle = 0.;
        for (h1, &p1) in ah.iter().enumerate() {
            for (a1, &p2) in ba.iter().enumerate() {
                let x = h1 as i32 - a1 as i32;
                if x <= 0 {
                    continue;
                }
                for (h2, &p3) in ch.iter().enumerate() {
                    for (a2, &p4) in aa.iter().enumerate() {
                        let y = h2 as i32 - a2 as i32;
                        if y > 0
                            && 2 * y - x > 9
                            && (x + y > 18 || (x + y == 18 && 1 + h2 as i32 > 10 + a1 as i32))
                        {
                            oracle += p1 * p2 * p3 * p4;
                        }
                    }
                }
            }
        }
        let wdl_mass = model.fixtures[0].prob[2] * model.fixtures[1].prob[2];
        let expected = oracle / wdl_mass;
        let mut rng = Rng::new(8_081);
        let mut context = Context::new(&model);
        let mut operations = Operations::default();
        let (mut sum, mut sum2, mut weights, mut weights2) = (0., 0., 0., 0.);
        let n = 40_000;
        for _ in 0..n {
            let (rank, weight) = proposal.rank_with_operations(
                &model,
                &outcomes,
                &mut rng,
                &mut context,
                &mut operations,
            );
            weights += weight;
            weights2 += weight * weight;
            if rank == 1 {
                sum += weight;
                sum2 += weight * weight;
            }
        }
        let normalization = weights / n as f64;
        let norm_se =
            ((weights2 / n as f64 - normalization * normalization) / (n - 1) as f64).sqrt();
        let mean = sum / n as f64;
        let se = ((sum2 / n as f64 - mean * mean) / (n - 1) as f64).sqrt();
        assert!(
            (normalization - 1.).abs() < 6. * norm_se,
            "{normalization} ± {norm_se}"
        );
        assert!(
            (mean - expected).abs() < 6. * se,
            "{mean} vs {expected} ± {se}"
        );
        assert!(operations.guidance > 0);
    }

    #[test]
    fn failed_joint_fit_is_cached_and_exhausted_budget_stays_bounded() {
        let request: Request =
            serde_json::from_str(include_str!("../tests/fixtures/group-17059-head-gd.json"))
                .unwrap();
        let model = Model::new(request).unwrap();
        let target = model.indices[&2694];
        let proposal = GoalTilt::new(
            &model,
            Cell {
                team: target,
                rank: 1,
            },
        )
        .unwrap();
        let mut context = Context::with_sample_budget(&model, 1);
        let mut rng = Rng::new(5);
        let mut operations = Operations::default();
        let outcomes = [2, 2];
        let _ = proposal.rank_with_operations(
            &model,
            &outcomes,
            &mut rng,
            &mut context,
            &mut operations,
        );
        let spent = context.joint_work;
        let cached = context.joint.len();
        assert!(cached <= MAX_JOINT_PATH_CACHE);
        let _ = proposal.rank_with_operations(
            &model,
            &outcomes,
            &mut rng,
            &mut context,
            &mut operations,
        );
        assert_eq!(context.joint_work, spent);
        assert_eq!(context.joint.len(), cached);
        assert!(context.joint_work <= context.joint_work_limit);

        let mut exhausted = Context::with_sample_budget(&model, 0);
        for path in [[0, 2], [2, 2], [2, 0], [1, 1]] {
            let _ = proposal.rank_with_operations(
                &model,
                &path,
                &mut rng,
                &mut exhausted,
                &mut operations,
            );
        }
        assert!(exhausted.joint_exhausted);
        assert!(exhausted.joint.is_empty());
        assert_eq!(exhausted.joint_work, 0);
    }

    #[test]
    fn tiny_budget_falls_back_before_copying_large_played_history() {
        let mut request: Request =
            serde_json::from_str(include_str!("../tests/fixtures/group-17059-head-gd.json"))
                .unwrap();
        for id in 0..1_200 {
            request.games.push(crate::model::Game {
                id: 50_000 + id,
                home_id: 80_001,
                away_id: 80_002,
                home_score: 0,
                away_score: 0,
                home_power: 0.,
                away_power: 0.,
                played: true,
            });
        }
        let model = Model::new(request).unwrap();
        let target = model.indices[&2694];
        let proposal = GoalTilt::new(
            &model,
            Cell {
                team: target,
                rank: 1,
            },
        )
        .unwrap();
        let mut context = Context::with_sample_budget(&model, 1);
        let mut rng = Rng::new(12);
        let mut operations = Operations::default();
        for _ in 0..2 {
            let _ = proposal.rank_with_operations(
                &model,
                &[2, 2],
                &mut rng,
                &mut context,
                &mut operations,
            );
        }
        assert_eq!(context.joint_work, 0);
        assert_eq!(context.joint.len(), 1);
        assert!(context.joint_work <= context.joint_work_limit);
    }
    #[test]
    fn tilted_goal_mixture_matches_exact_conditional_rare_goal_difference_probability() {
        let request:Request=serde_json::from_value(json!({"id":1,"phase":{"sort":"pt,w,gd,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..4).map(|t|json!({"team_id":t,"add_sub":0,"bias":if t==0{10}else{0}})).collect::<Vec<_>>(),
            "games":[{"id":1,"home_id":0,"away_id":1,"home_power":1.2,"away_power":0.8},{"id":2,"home_id":2,"away_id":3,"home_power":1.2,"away_power":0.8}]})).unwrap();
        let mut m = Model::new(request).unwrap();
        m.base[0].ga = 8;
        let mut d = vec![0.; 40];
        let mut z = 0.;
        for (h, &hp) in masses(1.2).iter().enumerate() {
            for (a, &ap) in masses(0.8).iter().enumerate() {
                if h > a {
                    d[h - a] += hp * ap;
                    z += hp * ap;
                }
            }
        }
        for p in &mut d {
            *p /= z;
        }
        let mut expected = 0.;
        for h in 1..d.len() {
            for a in 1..d.len() {
                if h as i32 - 8 >= a as i32 {
                    expected += d[h] * d[a];
                }
            }
        }
        assert!(expected > 0. && expected < 1e-5);
        let p = GoalTilt::new(&m, Cell { team: 0, rank: 0 }).unwrap();
        let mut rng = Rng::new(808);
        let mut context = Context::new(&m);
        let (mut sum, mut sum2, mut weight_sum, mut weight_sum2) = (0., 0., 0., 0.);
        let n = 40000;
        for _ in 0..n {
            let (rank, w) = p.rank(&m, &[2, 2], &mut rng, &mut context);
            weight_sum += w;
            weight_sum2 += w * w;
            if rank == 0 {
                sum += w;
                sum2 += w * w;
            }
        }
        let mean = sum / n as f64;
        let se = ((sum2 / n as f64 - mean * mean) / (n - 1) as f64).sqrt();
        let weight_mean = weight_sum / n as f64;
        let weight_se =
            ((weight_sum2 / n as f64 - weight_mean * weight_mean) / (n - 1) as f64).sqrt();
        assert!(
            (weight_mean - 1.).abs() < 6. * weight_se,
            "conditional likelihood weights average to {weight_mean}, se={weight_se}"
        );
        assert!(
            (mean - expected).abs() < 6. * se,
            "{mean} vs {expected}, se={se}"
        );
    }

    fn exact_conditional_rank(m: &Model, outcomes: &[u8], target: usize, rank: usize) -> f64 {
        let options: Vec<Vec<([i32; 2], f64)>> = m
            .fixtures
            .iter()
            .zip(outcomes)
            .map(|(fixture, &outcome)| {
                let home = masses(fixture.home_sampler.mean);
                let away = masses(fixture.away_sampler.mean);
                let mut scores = Vec::new();
                let mut total = 0.;
                for (h, &hp) in home.iter().enumerate() {
                    for (a, &ap) in away.iter().enumerate() {
                        let observed = if h < a {
                            0
                        } else if h == a {
                            1
                        } else {
                            2
                        };
                        if observed == outcome {
                            let probability = hp * ap;
                            total += probability;
                            scores.push(([h as i32, a as i32], probability));
                        }
                    }
                }
                for (_, probability) in &mut scores {
                    *probability /= total;
                }
                scores
            })
            .collect();
        let mut favorable = 0.;
        for &(first, first_p) in &options[0] {
            for &(second, second_p) in &options[1] {
                let mut campaign = m.base.clone();
                let mut scores = m.empty_scores();
                for (i, score) in [first, second].into_iter().enumerate() {
                    let fixture = &m.fixtures[i];
                    scores[fixture.request_index] = score;
                    m.add(&mut campaign, i, score);
                }
                let mut order = vec![0; m.n];
                m.standings(&mut order, &campaign, &scores, &mut Rng::new(808));
                if order[rank] == target {
                    favorable += first_p * second_p;
                }
            }
        }
        favorable
    }

    #[test]
    fn tilted_goal_mixture_matches_head_to_head_sorter_reference() {
        let request: Request = serde_json::from_value(json!({"id":1,"phase":{"sort":"pt,head,gd,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..4).map(|t|json!({"team_id":t,"add_sub":if t==1{3}else{0},"bias":10-t})).collect::<Vec<_>>(),
            "games":[{"id":1,"home_id":0,"away_id":1,"home_power":1.2,"away_power":0.8},{"id":2,"home_id":2,"away_id":3,"home_power":1.2,"away_power":0.8}]})).unwrap();
        let m = Model::new(request).unwrap();
        let outcomes = [2, 2];
        let expected = exact_conditional_rank(&m, &outcomes, 0, 0);
        let proposal = GoalTilt::new(&m, Cell { team: 0, rank: 0 }).unwrap();
        let mut rng = Rng::new(809);
        let mut context = Context::new(&m);
        let (mut sum, mut sum2) = (0., 0.);
        let n = 40000;
        for _ in 0..n {
            let (rank, weight) = proposal.rank(&m, &outcomes, &mut rng, &mut context);
            if rank == 0 {
                sum += weight;
                sum2 += weight * weight;
            }
        }
        let mean = sum / n as f64;
        let se = ((sum2 / n as f64 - mean * mean) / (n - 1) as f64).sqrt();
        assert!(
            (mean - expected).abs() < 6. * se,
            "{mean} vs {expected}, se={se}"
        );
    }

    #[test]
    fn goal_tilt_accepts_goal_difference_after_head_to_head_and_requires_points_first() {
        let request = |sort: &str| {
            serde_json::from_value::<Request>(json!({"id":1,"phase":{"sort":sort,"championship":{"point_win":3,"point_draw":1,"point_loss":0}},
                "team_groups":(0..4).map(|t|json!({"team_id":t,"add_sub":0,"bias":0})).collect::<Vec<_>>(),
                "games":[{"id":1,"home_id":0,"away_id":1,"home_power":1.2,"away_power":0.8},{"id":2,"home_id":2,"away_id":3,"home_power":1.2,"away_power":0.8}]})).unwrap()
        };
        let head_before_gd = Model::new(request("pt,head,w,gd")).unwrap();
        assert!(GoalTilt::new(&head_before_gd, Cell { team: 0, rank: 0 }).is_some());
        let gd_before_head = Model::new(request("pt,gd,head")).unwrap();
        assert!(GoalTilt::new(&gd_before_head, Cell { team: 0, rank: 0 }).is_some());
        let no_points_first = Model::new(request("head,pt,gd")).unwrap();
        assert!(GoalTilt::new(&no_points_first, Cell { team: 0, rank: 0 }).is_none());
        let no_goal_difference = Model::new(request("pt,head,w")).unwrap();
        assert!(GoalTilt::new(&no_goal_difference, Cell { team: 0, rank: 0 }).is_none());
    }
}
