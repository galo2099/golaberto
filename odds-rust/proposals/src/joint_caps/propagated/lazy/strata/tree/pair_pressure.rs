use super::Leaf;
use crate::lookahead::RankGame;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(super) struct PairCorrection {
    pub(super) left: usize,
    pub(super) right: usize,
    pub(super) log_factor: f64,
}

pub(super) struct Result {
    pub(super) adjusted_log_scores: Option<Vec<f64>>,
    pub(super) independent_shares: Option<Vec<f64>>,
    pub(super) adjusted_shares: Option<Vec<f64>>,
    pub(super) work: usize,
    pub(super) reason: Option<&'static str>,
    pub(super) unique_private_pmfs: usize,
    pub(super) unique_root_pair_queries: usize,
    pub(super) leaves: Vec<PairLeafSummary>,
}

#[derive(Clone, Debug)]
pub(super) struct PairLeafSummary {
    pub(super) required_above_count: usize,
    pub(super) pair_count: usize,
    pub(super) shared_pair_count: usize,
    pub(super) sum_log_factor: f64,
}

struct PairPmfs {
    shared: Vec<(usize, RankGame)>,
    left_tails: Vec<f64>,
    right_tails: Vec<f64>,
}

fn actual_private_pmfs(cache: &HashMap<(usize, usize), PairPmfs>) -> usize {
    cache
        .values()
        .filter(|pair| !pair.shared.is_empty())
        .count()
        .saturating_mul(2)
}

fn actual_shared_queries(
    pairs: &HashMap<(usize, usize, usize, bool, bool), (f64, Option<&'static str>)>,
    cache: &HashMap<(usize, usize), PairPmfs>,
) -> usize {
    pairs
        .keys()
        .filter(|(_, left, right, _, _)| {
            cache
                .get(&(*left, *right))
                .is_some_and(|pair| !pair.shared.is_empty())
        })
        .count()
}

fn charge(work: &mut usize, limit: usize, amount: usize) -> bool {
    if work.saturating_add(amount) > limit {
        *work = limit;
        false
    } else {
        *work += amount;
        true
    }
}

fn log_probability(p: f64) -> f64 {
    if p > 0.0 && p.is_finite() {
        p.ln()
    } else {
        f64::NEG_INFINITY
    }
}

fn log_add(a: f64, b: f64) -> f64 {
    if a == f64::NEG_INFINITY {
        return b;
    }
    if b == f64::NEG_INFINITY {
        return a;
    }
    let m = a.max(b);
    m + ((a - m).exp() + (b - m).exp()).ln()
}

fn update_pmf(
    pmf: &[f64],
    gains: [i32; 3],
    probs: [f64; 3],
    work: &mut usize,
    limit: usize,
) -> Option<Vec<f64>> {
    if !charge(work, limit, pmf.len().saturating_mul(3)) {
        return None;
    }
    if gains.iter().any(|gain| *gain < 0)
        || probs
            .iter()
            .any(|probability| !probability.is_finite() || *probability < 0.0)
        || pmf.iter().any(|mass| !mass.is_finite() || *mass < 0.0)
    {
        return None;
    }
    let span = pmf
        .len()
        .saturating_add(gains.iter().copied().max().unwrap_or(0) as usize);
    let mut next = vec![0.0; span];
    for (score, mass) in pmf.iter().enumerate() {
        for outcome in 0..3 {
            let index = score.saturating_add(gains[outcome] as usize);
            if index < next.len() {
                next[index] += mass * probs[outcome];
            }
        }
    }
    Some(next)
}

fn suffix(pmf: &[f64], work: &mut usize, limit: usize) -> Option<Vec<f64>> {
    if !charge(work, limit, pmf.len()) {
        return None;
    }
    let mut tails = vec![0.0; pmf.len()];
    let mut sum = 0.0;
    for index in (0..pmf.len()).rev() {
        sum += pmf[index];
        if !sum.is_finite() {
            return None;
        }
        tails[index] = sum;
    }
    Some(tails)
}

fn tail_probability(tails: &[f64], threshold: i32, strict: bool) -> f64 {
    if threshold < 0 {
        return tails.first().copied().unwrap_or(1.0);
    }
    let index = (threshold as usize).saturating_add(usize::from(strict));
    tails.get(index).copied().unwrap_or(0.0)
}

fn log_pair_factor(
    log_joint: f64,
    q_left: f64,
    q_right: f64,
) -> std::result::Result<f64, &'static str> {
    if !(q_left > 0.0
        && q_right > 0.0
        && q_left.is_finite()
        && q_right.is_finite()
        && q_left <= 1.0 + 1e-12
        && q_right <= 1.0 + 1e-12)
    {
        return Err("zero or invalid pair marginal");
    }
    if log_joint.is_nan() {
        return Err("nonfinite pair joint");
    }
    if log_joint == f64::NEG_INFINITY {
        return Ok(f64::NEG_INFINITY);
    }
    let log_factor = log_joint - q_left.ln() - q_right.ln();
    if log_factor.is_finite() {
        Ok(log_factor)
    } else {
        Err("nonfinite pair factor")
    }
}

fn pair_pmfs(
    left: usize,
    right: usize,
    remaining: &[RankGame],
    work: &mut usize,
    limit: usize,
) -> std::result::Result<PairPmfs, &'static str> {
    if !charge(work, limit, remaining.len().saturating_mul(2)) {
        return Err("pair fixture scan work cap");
    }
    let shared = remaining
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, game)| {
            (game.home == left && game.away == right) || (game.home == right && game.away == left)
        })
        .collect::<Vec<_>>();
    if shared.is_empty() {
        return Ok(PairPmfs {
            shared,
            left_tails: vec![1.0],
            right_tails: vec![1.0],
        });
    }
    if shared.len() > 2 {
        return Err("pair has more than two shared remaining fixtures");
    }
    let shared_indexes = shared.iter().map(|(index, _)| *index).collect::<Vec<_>>();
    let mut left_pmf = vec![1.0];
    let mut right_pmf = vec![1.0];
    for (index, game) in remaining.iter().enumerate() {
        if shared_indexes.contains(&index) {
            continue;
        }
        if game.home == left {
            left_pmf = update_pmf(&left_pmf, game.hg, game.prob, work, limit)
                .ok_or("pair PMF work cap or invalid gains")?;
        } else if game.away == left {
            left_pmf = update_pmf(&left_pmf, game.ag, game.prob, work, limit)
                .ok_or("pair PMF work cap or invalid gains")?;
        }
        if game.home == right {
            right_pmf = update_pmf(&right_pmf, game.hg, game.prob, work, limit)
                .ok_or("pair PMF work cap or invalid gains")?;
        } else if game.away == right {
            right_pmf = update_pmf(&right_pmf, game.ag, game.prob, work, limit)
                .ok_or("pair PMF work cap or invalid gains")?;
        }
    }
    Ok(PairPmfs {
        shared,
        left_tails: suffix(&left_pmf, work, limit).ok_or("pair tail work cap")?,
        right_tails: suffix(&right_pmf, work, limit).ok_or("pair tail work cap")?,
    })
}

fn joint_log_probability(
    pair: &PairPmfs,
    left: usize,
    right: usize,
    left_base: i32,
    right_base: i32,
    target_score: i32,
    left_strict: bool,
    right_strict: bool,
    work: &mut usize,
    limit: usize,
) -> std::result::Result<f64, &'static str> {
    if pair.shared.is_empty() {
        return Ok(f64::NEG_INFINITY);
    }
    let assignments = 3usize
        .checked_pow(pair.shared.len() as u32)
        .ok_or("shared outcome count overflow")?;
    let mut log_joint = f64::NEG_INFINITY;
    for mut code in 0..assignments {
        if !charge(work, limit, pair.shared.len().saturating_add(1)) {
            return Err("pair root-query work cap");
        }
        let mut log_term = 0.0;
        let mut left_shared = 0i32;
        let mut right_shared = 0i32;
        for (_, game) in &pair.shared {
            let outcome = code % 3;
            code /= 3;
            log_term += log_probability(game.prob[outcome]);
            if game.home == left {
                left_shared += game.hg[outcome];
            } else {
                left_shared += game.ag[outcome];
            }
            if game.home == right {
                right_shared += game.hg[outcome];
            } else {
                right_shared += game.ag[outcome];
            }
        }
        if log_term == f64::NEG_INFINITY {
            continue;
        }
        let left_threshold = target_score - left_base - left_shared;
        let right_threshold = target_score - right_base - right_shared;
        let left_tail = tail_probability(&pair.left_tails, left_threshold, left_strict);
        let right_tail = tail_probability(&pair.right_tails, right_threshold, right_strict);
        log_term += log_probability(left_tail) + log_probability(right_tail);
        log_joint = log_add(log_joint, log_term);
    }
    Ok(log_joint)
}

/// Adds exact two-rival dependence corrections to independent leaf scores.
/// A cap/error returns no adjusted scores, so the caller can roll back every
/// attempted pair factor while retaining the already computed independent guide.
#[allow(clippy::too_many_arguments)]
pub(super) fn adjust(
    team: usize,
    base: &[i32],
    target_games: &[RankGame],
    remaining: &[RankGame],
    roots: &[Vec<u8>],
    root_comparators: Option<&[Vec<[f64; 3]>]>,
    leaves: &[Leaf],
    independent_log_scores: &[f64],
    work_limit: usize,
) -> Result {
    let mut work = 0usize;
    let mut private_cache = HashMap::<(usize, usize), PairPmfs>::new();
    let mut factor_cache =
        HashMap::<(usize, usize, usize, bool, bool), (f64, Option<&'static str>)>::new();
    let mut root_fixed = Vec::with_capacity(roots.len());
    let mut root_target = Vec::with_capacity(roots.len());
    for root in roots {
        if !charge(
            &mut work,
            work_limit,
            target_games.len().saturating_add(base.len()),
        ) {
            return failed(
                work,
                "pair root setup work cap",
                actual_private_pmfs(&private_cache),
                actual_shared_queries(&factor_cache, &private_cache),
                leaves.len(),
            );
        }
        let mut fixed = vec![0i32; base.len()];
        for (game, &outcome) in target_games.iter().zip(root) {
            let outcome = outcome as usize;
            fixed[game.home] += game.hg[outcome];
            fixed[game.away] += game.ag[outcome];
        }
        root_target.push(base[team] + fixed[team]);
        root_fixed.push(fixed);
    }
    let mut adjustments = vec![0.0; leaves.len()];
    let mut leaf_rows = Vec::with_capacity(leaves.len());
    for (leaf_index, leaf) in leaves.iter().enumerate() {
        if !charge(&mut work, work_limit, base.len()) {
            return failed(
                work,
                "pair selected-rival scan cap",
                actual_private_pmfs(&private_cache),
                actual_shared_queries(&factor_cache, &private_cache),
                leaves.len(),
            );
        }
        let mut required = Vec::new();
        for rival in 0..base.len() {
            let bit = 1u64 << rival;
            let strict_above = leaf.above && leaf.mask & bit != 0;
            let nonstrict_above = !leaf.above && leaf.mask & bit == 0;
            if rival != team && leaf.selected & bit != 0 && (strict_above || nonstrict_above) {
                required.push((rival, strict_above));
            }
        }
        let mut corrections = Vec::new();
        for first in 0..required.len() {
            for second in first + 1..required.len() {
                if !charge(&mut work, work_limit, 1) {
                    return failed(
                        work,
                        "pair enumeration cap",
                        actual_private_pmfs(&private_cache),
                        actual_shared_queries(&factor_cache, &private_cache),
                        leaves.len(),
                    );
                }
                let ((left, left_strict), (right, right_strict)) =
                    (required[first], required[second]);
                let (left, right, left_strict, right_strict) = if left < right {
                    (left, right, left_strict, right_strict)
                } else {
                    (right, left, right_strict, left_strict)
                };
                let key = (leaf.root_index, left, right, left_strict, right_strict);
                let (log_factor, _reason) = if let Some(cached) = factor_cache.get(&key) {
                    if cached.1.is_some() && independent_log_scores[leaf_index].is_finite() {
                        return failed(
                            work,
                            "zero or invalid marginal for finite independent leaf score",
                            actual_private_pmfs(&private_cache),
                            actual_shared_queries(&factor_cache, &private_cache),
                            leaves.len(),
                        );
                    }
                    *cached
                } else {
                    let Some(root_comparators) = root_comparators else {
                        return failed(
                            work,
                            "root comparator marginals unavailable",
                            actual_private_pmfs(&private_cache),
                            actual_shared_queries(&factor_cache, &private_cache),
                            leaves.len(),
                        );
                    };
                    let Some(comparators) = root_comparators.get(leaf.root_index) else {
                        return failed(
                            work,
                            "missing root comparator marginals",
                            actual_private_pmfs(&private_cache),
                            actual_shared_queries(&factor_cache, &private_cache),
                            leaves.len(),
                        );
                    };
                    let marginal = |rival: usize, strict: bool| {
                        let index = if rival < team { rival } else { rival - 1 };
                        comparators.get(index).map(|parts| {
                            if strict {
                                parts[2]
                            } else {
                                parts[1] + parts[2]
                            }
                        })
                    };
                    let Some(q_left) = marginal(left, left_strict) else {
                        return failed(
                            work,
                            "missing left comparator marginal",
                            actual_private_pmfs(&private_cache),
                            actual_shared_queries(&factor_cache, &private_cache),
                            leaves.len(),
                        );
                    };
                    let Some(q_right) = marginal(right, right_strict) else {
                        return failed(
                            work,
                            "missing right comparator marginal",
                            actual_private_pmfs(&private_cache),
                            actual_shared_queries(&factor_cache, &private_cache),
                            leaves.len(),
                        );
                    };
                    if log_pair_factor(0.0, q_left, q_right).is_err() {
                        if independent_log_scores[leaf_index].is_finite() {
                            return failed(
                                work,
                                "zero or invalid marginal for finite independent leaf score",
                                actual_private_pmfs(&private_cache),
                                actual_shared_queries(&factor_cache, &private_cache),
                                leaves.len(),
                            );
                        }
                        factor_cache
                            .insert(key, (0.0, Some("impossible marginal; neutral correction")));
                        (0.0, Some("impossible marginal; neutral correction"))
                    } else {
                        if !private_cache.contains_key(&(left, right)) {
                            match pair_pmfs(left, right, remaining, &mut work, work_limit) {
                                Ok(pmfs) => {
                                    private_cache.insert((left, right), pmfs);
                                }
                                Err(reason) => {
                                    return failed(
                                        work,
                                        reason,
                                        actual_private_pmfs(&private_cache),
                                        actual_shared_queries(&factor_cache, &private_cache),
                                        leaves.len(),
                                    )
                                }
                            }
                        }
                        let pair = private_cache.get(&(left, right)).unwrap();
                        if pair.shared.is_empty() {
                            factor_cache.insert(key, (0.0, None));
                            (0.0, None)
                        } else {
                            let fixed = &root_fixed[leaf.root_index];
                            let left_base = base[left] + fixed[left];
                            let right_base = base[right] + fixed[right];
                            let log_joint = match joint_log_probability(
                                pair,
                                left,
                                right,
                                left_base,
                                right_base,
                                root_target[leaf.root_index],
                                left_strict,
                                right_strict,
                                &mut work,
                                work_limit,
                            ) {
                                Ok(value) => value,
                                Err(reason) => {
                                    return failed(
                                        work,
                                        reason,
                                        actual_private_pmfs(&private_cache),
                                        actual_shared_queries(&factor_cache, &private_cache),
                                        leaves.len(),
                                    )
                                }
                            };
                            let log_factor = match log_pair_factor(log_joint, q_left, q_right) {
                                Ok(value) => value,
                                Err(reason) => {
                                    return failed(
                                        work,
                                        reason,
                                        actual_private_pmfs(&private_cache),
                                        actual_shared_queries(&factor_cache, &private_cache),
                                        leaves.len(),
                                    )
                                }
                            };
                            factor_cache.insert(key, (log_factor, None));
                            (log_factor, None)
                        }
                    }
                };
                adjustments[leaf_index] += log_factor;
                corrections.push(PairCorrection {
                    left,
                    right,
                    log_factor,
                });
            }
        }
        leaf_rows.push(PairLeafSummary {
            required_above_count: required.len(),
            pair_count: corrections.len(),
            shared_pair_count: corrections
                .iter()
                .filter(|pair| {
                    private_cache
                        .get(&(pair.left, pair.right))
                        .is_some_and(|pmfs| !pmfs.shared.is_empty())
                })
                .count(),
            sum_log_factor: corrections.iter().map(|pair| pair.log_factor).sum(),
        });
    }
    if !charge(
        &mut work,
        work_limit,
        leaves.len().saturating_mul(4).saturating_add(roots.len()),
    ) {
        return failed(
            work,
            "pair adjustment normalization cap",
            actual_private_pmfs(&private_cache),
            actual_shared_queries(&factor_cache, &private_cache),
            leaves.len(),
        );
    }
    let adjusted_log_scores = independent_log_scores
        .iter()
        .zip(adjustments)
        .map(|(&score, correction)| score + correction)
        .collect::<Vec<_>>();
    let mut by_root = vec![Vec::new(); roots.len()];
    for (index, leaf) in leaves.iter().enumerate() {
        let Some(group) = by_root.get_mut(leaf.root_index) else {
            return failed(
                work,
                "invalid leaf root index during pair normalization",
                actual_private_pmfs(&private_cache),
                actual_shared_queries(&factor_cache, &private_cache),
                leaves.len(),
            );
        };
        group.push(index);
    }
    let normalize = |scores: &[f64]| {
        let mut shares = vec![0.0; leaves.len()];
        for indexes in &by_root {
            if !indexes.is_empty() {
                let local = super::path_priority::normalized_log_shares(
                    &indexes
                        .iter()
                        .map(|&index| scores[index])
                        .collect::<Vec<_>>(),
                );
                for (&index, share) in indexes.iter().zip(local) {
                    shares[index] = share;
                }
            }
        }
        shares
    };
    let independent_shares = normalize(independent_log_scores);
    let adjusted_shares = normalize(&adjusted_log_scores);
    Result {
        adjusted_log_scores: Some(adjusted_log_scores),
        independent_shares: Some(independent_shares),
        adjusted_shares: Some(adjusted_shares),
        work,
        reason: None,
        unique_private_pmfs: actual_private_pmfs(&private_cache),
        unique_root_pair_queries: actual_shared_queries(&factor_cache, &private_cache),
        leaves: leaf_rows,
    }
}

fn failed(
    work: usize,
    reason: &'static str,
    unique_private_pmfs: usize,
    unique_root_pair_queries: usize,
    leaf_count: usize,
) -> Result {
    Result {
        adjusted_log_scores: None,
        independent_shares: None,
        adjusted_shares: None,
        work,
        reason: Some(reason),
        unique_private_pmfs,
        unique_root_pair_queries,
        leaves: vec![
            PairLeafSummary {
                required_above_count: 0,
                pair_count: 0,
                shared_pair_count: 0,
                sum_log_factor: 0.0,
            };
            leaf_count
        ],
    }
}

pub(super) fn unavailable(reason: &'static str, leaf_count: usize) -> Result {
    failed(0, reason, 0, 0, leaf_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_pairs_are_exactly_neutral() {
        let a = RankGame {
            index: 0,
            home: 0,
            away: 2,
            prob: [0.3, 0.4, 0.3],
            hg: [3, 1, 0],
            ag: [0, 1, 3],
        };
        let b = RankGame {
            index: 1,
            home: 1,
            away: 3,
            prob: [0.4, 0.4, 0.2],
            hg: [3, 1, 0],
            ag: [0, 1, 3],
        };
        let mut work = 0;
        let result = pair_pmfs(0, 1, &[a, b], &mut work, 1000).unwrap();
        assert!(result.shared.is_empty());
    }

    #[test]
    fn cap_failure_returns_no_partial_adjustment_and_keeps_paid_work() {
        let original = Vec::<f64>::new();
        let result = adjust(0, &[0], &[], &[], &[vec![]], None, &[], &original, 1);
        assert!(result.adjusted_log_scores.is_none());
        assert_eq!(result.work, 1);
        assert!(result.reason.is_some());
        assert!(original.is_empty());
    }

    #[test]
    fn direct_suffix_keeps_rare_tails_and_strict_ties_distinct() {
        let pmf = [1.0 - 1e-20, 0.0, 1e-20];
        let mut work = 0;
        let tail = suffix(&pmf, &mut work, 100).unwrap();
        assert!((tail_probability(&tail, 1, true) - 1e-20).abs() < 1e-30);
        let tied_tails = suffix(&[0.2, 0.3, 0.5], &mut work, 100).unwrap();
        assert!((tail_probability(&tied_tails, 1, false) - 0.8).abs() < 1e-12);
        assert!((tail_probability(&tied_tails, 1, true) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn exact_pair_joint_handles_one_and_two_shared_fixtures_and_tie_rules() {
        let game = RankGame {
            index: 0,
            home: 1,
            away: 2,
            prob: [0.3, 0.4, 0.3],
            hg: [3, 1, 0],
            ag: [0, 1, 3],
        };
        let mut work = 0;
        let one = pair_pmfs(1, 2, &[game], &mut work, 1000).unwrap();
        let joint_ge = joint_log_probability(&one, 1, 2, 0, 0, 1, false, false, &mut work, 1000)
            .unwrap()
            .exp();
        assert!((joint_ge - 0.4).abs() < 1e-12);
        let factor = log_pair_factor(joint_ge.ln(), 0.7, 0.7).unwrap().exp();
        assert!((factor - 0.4 / 0.49).abs() < 1e-12);
        assert!((log_pair_factor(0.8f64.ln(), 0.5, 0.5).unwrap().exp() - 3.2).abs() < 1e-12);
        let joint_gt =
            joint_log_probability(&one, 1, 2, 0, 0, 1, true, true, &mut work, 1000).unwrap();
        assert_eq!(joint_gt, f64::NEG_INFINITY);

        let game2 = RankGame { index: 1, ..game };
        let two = pair_pmfs(1, 2, &[game, game2], &mut work, 10_000).unwrap();
        assert_eq!(two.shared.len(), 2);
        let joint = joint_log_probability(&two, 1, 2, 0, 0, 2, false, false, &mut work, 10_000)
            .unwrap()
            .exp();
        // Two draws (0.16) or one home and one away win (0.18) put both at 2+.
        assert!((joint - 0.34).abs() < 1e-12);
    }
}
