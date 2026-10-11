//! Opt-in negative-association/Chernoff allocation guidance.
//!
//! The returned values are upper-bound score shapes for allocation. They are
//! never used to alter Pattern masses, frozen quotas, or sampled weights.
use super::*;
use crate::{lookahead::RankGame, model::Model};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, VecDeque};

pub(super) const FLAG: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_NA_GUIDANCE";
pub(super) const BLEND: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_NA_BLEND";
pub(super) const WORK: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_NA_WORK";
const DEFAULT_WORK: usize = 8_000_000;
const MIN_WORK: usize = 8_000_000;
const MAX_WORK: usize = 512_000_000;
const CACHE_LIMIT: usize = 64;
const LOG_Z: [f64; 11] = [0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 20.0, 40.0];

pub(super) fn enabled() -> bool {
    std::env::var(FLAG).as_deref() == Ok("1")
}
pub(super) fn blend() -> f64 {
    std::env::var(BLEND)
        .ok()
        .and_then(|x| x.parse::<f64>().ok())
        .filter(|x| x.is_finite())
        .unwrap_or(0.5)
        .clamp(0.0, 1.0)
}
pub(super) fn cap() -> usize {
    let large =
        std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_LARGE_BUDGET").as_deref() == Ok("1");
    if !large {
        return DEFAULT_WORK;
    }
    std::env::var(WORK)
        .ok()
        .and_then(|x| x.parse::<usize>().ok())
        .unwrap_or(DEFAULT_WORK)
        .clamp(MIN_WORK, MAX_WORK)
}

#[derive(Default)]
pub(super) struct Report {
    pub(super) enabled: bool,
    pub(super) root_applied: bool,
    pub(super) leaf_applied: bool,
    pub(super) reason: Option<String>,
    pub(super) blend: f64,
    pub(super) work: usize,
    pub(super) cap: usize,
    pub(super) elapsed_ms: f64,
    pub(super) cache_hits: usize,
    pub(super) root_rows: Vec<Value>,
    pub(super) leaf_rows: Vec<Value>,
}

type Pmf = Vec<(i32, f64)>;
type Pmfs = Vec<Pmf>;
struct PmfCache {
    values: HashMap<Vec<u64>, Pmfs>,
    order: VecDeque<Vec<u64>>,
}
impl PmfCache {
    fn new() -> Self {
        Self {
            values: HashMap::new(),
            order: VecDeque::new(),
        }
    }
    fn get(&mut self, key: &[u64]) -> Option<Pmfs> {
        self.values.get(key).cloned()
    }
    fn insert(&mut self, key: Vec<u64>, value: Pmfs) {
        if self.values.contains_key(&key) {
            return;
        }
        self.order.push_back(key.clone());
        self.values.insert(key, value);
        while self.values.len() > CACHE_LIMIT {
            if let Some(old) = self.order.pop_front() {
                self.values.remove(&old);
            }
        }
    }
}

fn charge(work: &mut usize, cap: usize, amount: usize) -> bool {
    match work.checked_add(amount) {
        Some(next) if next <= cap => {
            *work = next;
            true
        }
        _ => {
            *work = cap;
            false
        }
    }
}

fn countermonotone(hg: [i32; 3], ag: [i32; 3], supported: &[usize]) -> bool {
    supported.iter().enumerate().all(|(i, &a)| {
        supported.iter().skip(i + 1).all(|&b| {
            let home_delta = i64::from(hg[a]) - i64::from(hg[b]);
            let away_delta = i64::from(ag[a]) - i64::from(ag[b]);
            home_delta == 0 || away_delta == 0 || home_delta.signum() != away_delta.signum()
        })
    })
}

fn combine_upper(logs: [f64; 4]) -> f64 {
    logs.into_iter().fold(0.0f64, f64::min)
}

fn rectangle(
    model: &Model,
    pattern: &Pattern,
    work: &mut usize,
    cap: usize,
) -> std::result::Result<(Vec<RankGame>, Vec<u64>, f64), String> {
    if pattern.base.len() != model.n
        || pattern.lower.len() != model.n
        || pattern.upper.len() != model.n
    {
        return Err("rectangle team-vector alignment unavailable".into());
    }
    let mut fixed = HashMap::<usize, u8>::new();
    let mut signature = Vec::<u64>::new();
    for &(index, outcome) in &pattern.fixed {
        if !charge(work, cap, 4) {
            return Err("NA guidance work cap".into());
        }
        let p = model
            .fixtures
            .get(index)
            .and_then(|g| g.prob.get(outcome as usize))
            .copied()
            .unwrap_or(0.0);
        if !p.is_finite() || p <= 0.0 {
            return Err("fixed outcome has no original probability mass".into());
        }
        if fixed.insert(index, outcome).is_some() {
            return Err("duplicate fixed WDL fixture".into());
        }
        signature.extend([index as u64, outcome as u64, p.to_bits()]);
    }
    let mut games = HashMap::<usize, RankGame>::new();
    for game in pattern
        .joint
        .games
        .iter()
        .chain(&pattern.guide.games)
        .chain(&pattern.omitted)
    {
        if !charge(work, cap, 3) {
            return Err("NA guidance work cap".into());
        }
        if game.index >= model.fixtures.len()
            || game.home >= model.n
            || game.away >= model.n
            || game.home == game.away
        {
            return Err("invalid variable fixture geometry".into());
        }
        if let Some(old) = games.get(&game.index) {
            if old.hg != game.hg
                || old.ag != game.ag
                || old.home != game.home
                || old.away != game.away
                || old.prob.map(f64::to_bits) != game.prob.map(f64::to_bits)
            {
                return Err("inconsistent duplicate WDL fixture".into());
            }
        } else {
            games.insert(game.index, *game);
        }
    }
    if fixed.keys().any(|i| games.contains_key(i)) {
        return Err("fixture is both fixed and variable".into());
    }
    if (0..model.fixtures.len()).any(|i| !fixed.contains_key(&i) && !games.contains_key(&i)) {
        return Err("incomplete fixture partition for NA rectangle".into());
    }
    let mut domain_log = 0.0;
    for &(index, outcome) in &pattern.fixed {
        domain_log += model.fixtures[index].prob[outcome as usize].ln();
    }
    let mut games: Vec<_> = games.into_values().collect();
    games.sort_by_key(|g| g.index);
    for game in &games {
        let original = model.fixtures[game.index].prob;
        let supported: Vec<_> = (0..3)
            .filter(|&o| original[o] > 0.0 && game.prob[o] > 0.0)
            .collect();
        if supported.is_empty() {
            return Err("empty supported WDL mask".into());
        }
        if !countermonotone(game.hg, game.ag, &supported) {
            return Err("unsupported non-countermonotone packed-score gains".into());
        }
        let retained: f64 = supported.iter().map(|&o| original[o]).sum();
        if !retained.is_finite() || retained <= 0.0 {
            return Err("empty retained outcome probability".into());
        }
        domain_log += retained.ln();
        signature.extend([game.index as u64, game.home as u64, game.away as u64]);
        for o in 0..3 {
            signature.extend([
                original[o].to_bits(),
                game.prob[o].to_bits(),
                game.hg[o] as u64,
                game.ag[o] as u64,
            ]);
        }
    }
    signature.push(u64::MAX);
    signature.extend(pattern.base.iter().map(|x| *x as u64));
    Ok((games, signature, domain_log))
}

fn pmfs(
    model: &Model,
    pattern: &Pattern,
    games: &[RankGame],
    key: &[u64],
    teams: usize,
    work: &mut usize,
    cap: usize,
    cache: &mut PmfCache,
    hits: &mut usize,
) -> std::result::Result<Pmfs, String> {
    if let Some(value) = cache.get(key) {
        let clone_cost = value.iter().map(Vec::len).sum::<usize>();
        if !charge(work, cap, clone_cost) {
            return Err("NA guidance work cap".into());
        }
        *hits += 1;
        return Ok(value);
    }
    let mut all = Vec::with_capacity(teams);
    for team in 0..teams {
        let mut dist = BTreeMap::<i32, f64>::from([(pattern.base[team], 1.0)]);
        for game in games.iter().filter(|g| g.home == team || g.away == team) {
            let gains = if game.home == team { game.hg } else { game.ag };
            let original_probs = model
                .fixtures
                .get(game.index)
                .ok_or("rectangle fixture index out of range")?
                .prob;
            let normalizer: f64 = (0..3)
                .filter(|&o| original_probs[o] > 0.0 && game.prob[o] > 0.0)
                .map(|o| original_probs[o])
                .sum();
            if normalizer <= 0.0 || !normalizer.is_finite() {
                return Err("invalid rectangle WDL normalizer".into());
            }
            let outcomes: Vec<_> = (0..3)
                .filter(|&o| original_probs[o] > 0.0 && game.prob[o] > 0.0)
                .collect();
            let fee = dist.len().saturating_mul(outcomes.len());
            if !charge(work, cap, fee) {
                return Err("NA guidance work cap".into());
            }
            let mut next = BTreeMap::<i32, f64>::new();
            for (&value, &state_prob) in &dist {
                for &o in &outcomes {
                    *next.entry(value + gains[o]).or_default() +=
                        state_prob * original_probs[o] / normalizer;
                }
            }
            dist = next;
        }
        let mut values: Vec<_> = dist.into_iter().filter(|(_, p)| *p > 0.0).collect();
        values.sort_by_key(|x| x.0);
        all.push(values);
    }
    let store_cost = key
        .len()
        .saturating_add(all.iter().map(Vec::len).sum::<usize>());
    if !charge(work, cap, store_cost) {
        return Err("NA guidance work cap".into());
    }
    cache.insert(key.to_vec(), all.clone());
    Ok(all)
}

fn cdf_tail(
    pmf: &[(i32, f64)],
    threshold: i32,
    above: bool,
    work: &mut usize,
    cap: usize,
) -> std::result::Result<f64, String> {
    if !charge(work, cap, pmf.len()) {
        return Err("NA guidance work cap".into());
    }
    Ok(pmf
        .iter()
        .filter(|(value, _)| {
            if above {
                *value >= threshold
            } else {
                *value <= threshold
            }
        })
        .map(|(_, p)| *p)
        .sum::<f64>()
        .clamp(0.0, 1.0))
}

fn chernoff(
    q: &[f64],
    needed: usize,
    work: &mut usize,
    cap: usize,
) -> std::result::Result<f64, String> {
    if needed == 0 {
        return Ok(0.0);
    }
    if needed > q.len() {
        return Ok(f64::NEG_INFINITY);
    }
    let mut best = 0.0f64;
    for z in LOG_Z {
        if !charge(work, cap, q.len()) {
            return Err("NA guidance work cap".into());
        }
        let mut value = -(needed as f64) * z;
        for &p in q {
            value += (z.exp_m1() * p.clamp(0.0, 1.0)).ln_1p();
        }
        best = best.min(value);
    }
    Ok(best.min(0.0))
}

#[derive(Clone)]
struct Score {
    log_upper: f64,
    lower_product: f64,
    upper_product: f64,
    up_chernoff: f64,
    down_chernoff: f64,
}
fn score(
    pattern: &Pattern,
    target: usize,
    rank: usize,
    n: usize,
    pmfs: &[Pmf],
    work: &mut usize,
    cap: usize,
) -> std::result::Result<Score, String> {
    let target_score = pattern.base[target];
    let mut lower_product = 0.0f64;
    let mut upper_product = 0.0f64;
    let mut up_q = Vec::new();
    let mut down_q = Vec::new();
    for team in 0..n {
        if team == target {
            continue;
        }
        let lower = pattern.lower[team];
        let upper = pattern.upper[team];
        if lower > i32::MIN / 8 {
            let q = cdf_tail(&pmfs[team], lower, true, work, cap)?;
            if q <= 0.0 || !q.is_finite() {
                return Err("numeric-zero lower-tail guidance".into());
            }
            lower_product += q.ln();
        }
        if upper < i32::MAX / 8 {
            let q = cdf_tail(&pmfs[team], upper, false, work, cap)?;
            if q <= 0.0 || !q.is_finite() {
                return Err("numeric-zero upper-tail guidance".into());
            }
            upper_product += q.ln();
        }
        up_q.push(cdf_tail(&pmfs[team], target_score, true, work, cap)?);
        down_q.push(cdf_tail(&pmfs[team], target_score, false, work, cap)?);
        if !charge(work, cap, 8) {
            return Err("NA guidance work cap".into());
        }
    }
    let up_chernoff = chernoff(&up_q, rank, work, cap)?;
    let down_chernoff = chernoff(&down_q, n.saturating_sub(1 + rank), work, cap)?;
    let log_upper = combine_upper([lower_product, upper_product, up_chernoff, down_chernoff]);
    if !log_upper.is_finite() {
        return Err("nonfinite NA upper score".into());
    }
    Ok(Score {
        log_upper,
        lower_product,
        upper_product,
        up_chernoff,
        down_chernoff,
    })
}

fn normalized_log_shares(logs: &[f64]) -> Vec<f64> {
    let max = logs
        .iter()
        .copied()
        .filter(|x| x.is_finite())
        .fold(f64::NEG_INFINITY, f64::max);
    if !max.is_finite() {
        return vec![1.0 / logs.len().max(1) as f64; logs.len()];
    }
    let mut values: Vec<_> = logs
        .iter()
        .map(|x| if x.is_finite() { (*x - max).exp() } else { 0.0 })
        .collect();
    let sum: f64 = values.iter().sum();
    if sum <= 0.0 || !sum.is_finite() {
        return vec![1.0 / logs.len().max(1) as f64; logs.len()];
    }
    for x in &mut values {
        *x /= sum;
    }
    values
}
fn blend_logs(old: &[f64], guide: &[f64], exponent: f64) -> Vec<f64> {
    if exponent == 0.0 {
        return old.to_vec();
    }
    if exponent == 1.0 {
        return guide.to_vec();
    }
    let old_share = normalized_log_shares(old);
    let guide_share = normalized_log_shares(guide);
    let result: Vec<_> = old_share
        .iter()
        .zip(guide_share)
        .map(|(&a, b)| {
            if a <= 0.0 || b <= 0.0 {
                f64::NEG_INFINITY
            } else {
                (1.0 - exponent) * a.ln() + exponent * b.ln()
            }
        })
        .collect();
    result
}

pub(super) fn blend_root_raw_scores(
    model: &Model,
    rank: usize,
    all_games: &[RankGame],
    screen: &mut path_priority::Screen,
    exponent: f64,
    work: &mut usize,
    cap: usize,
) -> std::result::Result<Vec<Value>, String> {
    if exponent == 0.0 {
        return Ok(Vec::new());
    }
    if all_games.iter().any(|g| {
        let supported: Vec<_> = (0..3).filter(|&o| g.prob[o] > 0.0).collect();
        !countermonotone(g.hg, g.ag, &supported)
    }) {
        return Err("unsupported non-countermonotone packed-score gains".into());
    }
    let comparators = screen
        .leaf_comparators
        .as_ref()
        .ok_or("root comparator PMFs unavailable")?;
    if comparators.len() != screen.roots.len() {
        return Err("root comparator alignment unavailable".into());
    }
    let mut guide = vec![f64::NEG_INFINITY; screen.roots.len()];
    let old: Vec<_> = screen
        .roots
        .iter()
        .map(|r| {
            if r.raw_score > 0.0 {
                r.raw_score.ln()
            } else {
                f64::NEG_INFINITY
            }
        })
        .collect();
    let mut rows = Vec::with_capacity(screen.roots.len());
    for (i, (root, comparator)) in screen.roots.iter().zip(comparators).enumerate() {
        let up_q: Vec<_> = comparator
            .iter()
            .map(|x| (x[1] + x[2]).clamp(0.0, 1.0))
            .collect();
        let down_q: Vec<_> = comparator
            .iter()
            .map(|x| (x[0] + x[1]).clamp(0.0, 1.0))
            .collect();
        let up = chernoff(&up_q, rank, work, cap)?;
        let down = chernoff(&down_q, model.n.saturating_sub(1 + rank), work, cap)?;
        let log_domain = if root.prior > 0.0 {
            root.prior.ln()
        } else {
            f64::NEG_INFINITY
        };
        let log_upper = 0.0f64.min(up).min(down);
        guide[i] = log_domain + log_upper;
        if !guide[i].is_finite() {
            return Err("numeric-zero root NA score".into());
        }
        rows.push(json!({"root_index":i,"log_domain_mass":log_domain,"log_upper":log_upper,"log_na_score":guide[i],"old_log_score":old[i],"up_count_log_chernoff":up,"down_count_log_chernoff":down}));
    }
    let blended = blend_logs(&old, &guide, exponent);
    for (i, (root, value)) in screen.roots.iter_mut().zip(blended).enumerate() {
        root.raw_score = if value.is_finite() { value.exp() } else { 0.0 };
        if let Some(row) = rows.get_mut(i) {
            row["blended_raw_score"] = json!(root.raw_score);
        }
    }
    let normalized =
        path_priority::normalized(&screen.roots.iter().map(|r| r.raw_score).collect::<Vec<_>>());
    for (root, share) in screen.roots.iter_mut().zip(normalized) {
        root.normalized_share = share;
    }
    Ok(rows)
}

pub(super) fn blend_leaf_corrected_logs(
    model: &Model,
    target: usize,
    rank: usize,
    leaves: &[Leaf],
    logs: &mut [f64],
    exponent: f64,
    work: &mut usize,
    cap: usize,
) -> std::result::Result<(Vec<Value>, usize), String> {
    if exponent == 0.0 {
        return Ok((Vec::new(), 0));
    }
    if logs.len() != leaves.len() {
        return Err("leaf NA guidance alignment unavailable".into());
    }
    let mut cache = PmfCache::new();
    let mut hits = 0;
    let mut guide = vec![f64::NEG_INFINITY; leaves.len()];
    let mut rows = Vec::with_capacity(leaves.len());
    for (i, leaf) in leaves.iter().enumerate() {
        let (games, key, log_domain) = rectangle(model, &leaf.pattern, work, cap)?;
        if games.iter().any(|g| g.home == target || g.away == target) {
            return Err("target has an unconditioned fixture in NA rectangle".into());
        }
        let pmf = pmfs(
            model,
            &leaf.pattern,
            &games,
            &key,
            model.n,
            work,
            cap,
            &mut cache,
            &mut hits,
        )?;
        let bound = score(&leaf.pattern, target, rank, model.n, &pmf, work, cap)?;
        guide[i] = log_domain + bound.log_upper;
        rows.push(json!({"leaf_index":i,"root_index":leaf.root_index,"log_domain_mass":log_domain,"log_upper":bound.log_upper,"log_na_score":guide[i],"lower_interval_log_product":bound.lower_product,"upper_interval_log_product":bound.upper_product,"up_count_log_chernoff":bound.up_chernoff,"down_count_log_chernoff":bound.down_chernoff}));
    }
    let mut by_root: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, leaf) in leaves.iter().enumerate() {
        by_root.entry(leaf.root_index).or_default().push(i);
    }
    let mut candidate = logs.to_vec();
    for indexes in by_root.values() {
        let old = normalized_log_shares(&indexes.iter().map(|&i| logs[i]).collect::<Vec<_>>());
        let na = normalized_log_shares(&indexes.iter().map(|&i| guide[i]).collect::<Vec<_>>());
        for (local, &i) in indexes.iter().enumerate() {
            if na[local] <= 0.0 {
                return Err("numeric-zero leaf guidance share".into());
            }
            if exponent < 1.0 && old[local] <= 0.0 {
                candidate[i] = f64::NEG_INFINITY;
                continue;
            }
            if exponent == 1.0 {
                candidate[i] = guide[i];
            } else {
                candidate[i] += exponent * (na[local].ln() - old[local].ln());
            }
        }
    }
    // Keep each root's old corrected total intact; only redistribute inside that root.
    for indexes in by_root.values() {
        let old_total = log_sum(indexes.iter().map(|&i| logs[i]));
        let new_total = log_sum(indexes.iter().map(|&i| candidate[i]));
        if !old_total.is_finite() || !new_total.is_finite() {
            return Err("invalid root-local NA log normalizer".into());
        }
        let shift = old_total - new_total;
        for &i in indexes {
            candidate[i] += shift;
        }
    }
    logs.copy_from_slice(&candidate);
    Ok((rows, hits))
}

fn log_sum(values: impl Iterator<Item = f64>) -> f64 {
    let values: Vec<_> = values.filter(|x| x.is_finite()).collect();
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !max.is_finite() {
        return max;
    }
    max + values.iter().map(|x| (*x - max).exp()).sum::<f64>().ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chernoff_chooses_tightest_grid_bound_and_caps_at_one() {
        let q = [0.1, 0.2, 0.3];
        let mut work = 0;
        let value = chernoff(&q, 2, &mut work, 10_000).unwrap();
        assert!(value.is_finite() && value <= 0.0);
        let direct = q.iter().map(|x| x.ln()).sum::<f64>();
        assert!(value >= direct - 1e-12);
        let mut work = 0;
        assert_eq!(chernoff(&q, 0, &mut work, 10_000).unwrap(), 0.0);
        assert_eq!(
            chernoff(&q, 4, &mut work, 10_000).unwrap(),
            f64::NEG_INFINITY
        );
    }

    #[test]
    fn geometric_blend_has_exact_original_and_na_endpoints() {
        let original = [-4.0, -2.0, f64::NEG_INFINITY];
        let guide = [-8.0, -1.0, -3.0];
        assert_eq!(blend_logs(&original, &guide, 0.0), original);
        assert_eq!(blend_logs(&original, &guide, 1.0), guide);
        let middle = blend_logs(&original, &guide, 0.5);
        assert!(middle[0].is_finite() && middle[1].is_finite());
        assert_eq!(middle[2], f64::NEG_INFINITY);
    }

    #[test]
    fn packed_fixture_countermonotonicity_accepts_reversed_gain_order_only() {
        assert!(countermonotone([0, 100, 301], [301, 100, 0], &[0, 1, 2]));
        assert!(!countermonotone([0, 100, 301], [0, 100, 301], &[0, 1, 2]));
        // A restricted outcome mask is checked only on its supported pair.
        assert!(countermonotone([0, 100, 301], [301, 100, 0], &[0, 2]));
    }

    #[test]
    fn opposite_tail_families_are_minimized_separately_not_multiplied() {
        // A shared home win can make one rival above on one packed endpoint
        // and a different rival below on the other. The two bounds are not
        // independent events, so combine them with min rather than product.
        let upper = combine_upper([0.5f64.ln(), 0.5f64.ln(), 0.0, 0.0]);
        let false_product = 0.5f64.ln() + 0.5f64.ln();
        assert!((upper.exp() - 0.5).abs() < 1e-15);
        assert!(upper > false_product);
        let exact_shared_fixture_mass = 0.5;
        assert!(upper.exp() + 1e-15 >= exact_shared_fixture_mass);
    }

    #[test]
    fn work_cap_failure_is_charged_and_fails_closed() {
        let mut work = 3;
        assert!(!charge(&mut work, 5, 4));
        assert_eq!(work, 5);
        let mut work = 0;
        assert!(cdf_tail(&[(0, 0.5), (1, 0.5)], 1, true, &mut work, 1).is_err());
        assert_eq!(work, 1);
    }
}

#[cfg(test)]
#[path = "na_guidance/brute_tests.rs"]
mod brute_tests;
