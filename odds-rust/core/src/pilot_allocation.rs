//! Pure, deterministic leaf-pilot allocation shared across crate boundaries.
use std::collections::BTreeMap;

pub fn quota_counts(shares: &[f64], total: usize, floor: usize) -> Option<Vec<usize>> {
    if shares.is_empty() || total < shares.len().saturating_mul(floor) {
        return None;
    }
    if shares
        .iter()
        .any(|share| !share.is_finite() || *share < 0.0)
    {
        return None;
    }
    let sum: f64 = shares.iter().sum();
    if !sum.is_finite() || sum <= 0.0 {
        return None;
    }
    let remaining = total - shares.len() * floor;
    let mut counts = vec![floor; shares.len()];
    let mut fractions = Vec::with_capacity(shares.len());
    let mut used = 0usize;
    for (i, share) in shares.iter().enumerate() {
        let exact = remaining as f64 * (*share / sum);
        let whole = exact.floor() as usize;
        counts[i] = counts[i].saturating_add(whole);
        used = used.saturating_add(whole);
        fractions.push((exact - whole as f64, i));
    }
    fractions.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let excess = remaining.checked_sub(used)?;
    for &(_, index) in fractions.iter().take(excess) {
        counts[index] = counts[index].saturating_add(1);
    }
    (counts.iter().sum::<usize>() == total).then_some(counts)
}

pub fn pilot_shares(structural: &[f64]) -> Option<Vec<f64>> {
    if structural.is_empty()
        || structural
            .iter()
            .any(|share| !share.is_finite() || *share < 0.0)
    {
        return None;
    }
    let sum: f64 = structural.iter().sum();
    if !sum.is_finite() {
        return None;
    }
    let n = structural.len() as f64;
    if sum <= 0.0 {
        return Some(vec![1.0 / n; structural.len()]);
    }
    Some(
        structural
            .iter()
            .map(|share| 0.9 * (*share / sum) + 0.1 / n)
            .collect(),
    )
}

#[derive(Clone, Debug)]
pub struct HierarchicalPilotGroup {
    pub id: usize,
    pub root: Vec<u8>,
    pub selected_ids: Vec<i32>,
    pub indexes: Vec<usize>,
    pub original_quota: usize,
    pub final_quota: usize,
    pub status: &'static str,
}

#[derive(Clone, Debug)]
pub struct HierarchicalPilotPlan {
    pub counts: Vec<usize>,
    pub original_counts: Vec<usize>,
    pub original_shares: Vec<f64>,
    pub actual_shares: Vec<f64>,
    pub group_ids: Vec<usize>,
    pub groups: Vec<HierarchicalPilotGroup>,
}

pub fn normalized_log_weights(log_scores: &[Option<f64>]) -> Option<Vec<f64>> {
    let max = log_scores
        .iter()
        .flatten()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    if !max.is_finite() {
        return None;
    }
    let mut weights = log_scores
        .iter()
        .map(|score| score.map_or(0.0, |score| (score - max).exp()))
        .collect::<Vec<_>>();
    let sum: f64 = weights.iter().sum();
    if !sum.is_finite() || sum <= 0.0 {
        return None;
    }
    for weight in &mut weights {
        *weight /= sum;
    }
    Some(weights)
}

pub fn hierarchical_pilot_plan(
    root_groups: &[(Vec<u8>, Vec<usize>)],
    independent_shares: &[f64],
    corrected_log_scores: &[Option<f64>],
    selected_ids: &[Vec<i32>],
    pilot_per_leaf: usize,
) -> Option<HierarchicalPilotPlan> {
    let leaves = independent_shares.len();
    if corrected_log_scores.len() != leaves || selected_ids.len() != leaves {
        return None;
    }
    let mut original_counts = vec![0usize; leaves];
    let mut original_shares = vec![0.0; leaves];
    let mut owners = vec![false; leaves];
    let mut root_indexes = Vec::with_capacity(root_groups.len());
    for (root, indexes) in root_groups {
        if indexes.is_empty() || indexes.iter().any(|&index| index >= leaves) {
            return None;
        }
        for &index in indexes {
            if std::mem::replace(&mut owners[index], true) {
                return None;
            }
        }
        let local = indexes
            .iter()
            .map(|&index| independent_shares[index])
            .collect::<Vec<_>>();
        let shares = pilot_shares(&local)?;
        let total = indexes.len().saturating_mul(pilot_per_leaf);
        let counts = quota_counts(&shares, total, 5)?;
        for ((&index, share), count) in indexes.iter().zip(shares).zip(counts) {
            original_shares[index] = share;
            original_counts[index] = count;
        }
        root_indexes.push((root.clone(), indexes.clone()));
    }
    if original_counts.iter().any(|count| *count == 0) || owners.iter().any(|owner| !owner) {
        return None;
    }

    let mut partition = BTreeMap::<(usize, Vec<i32>), Vec<usize>>::new();
    for (root_index, (_, indexes)) in root_indexes.iter().enumerate() {
        for &index in indexes {
            partition
                .entry((root_index, selected_ids[index].clone()))
                .or_default()
                .push(index);
        }
    }
    let mut counts = original_counts.clone();
    let mut actual_shares = vec![0.0; leaves];
    let mut group_ids = vec![usize::MAX; leaves];
    let mut groups = Vec::with_capacity(partition.len());
    for ((root_index, selected), indexes) in partition {
        let group_id = groups.len();
        let root = root_indexes[root_index].0.clone();
        let original_quota = indexes.iter().map(|&index| original_counts[index]).sum();
        let mut status = "singleton_kept";
        if indexes.len() > 1 {
            let local_logs = indexes
                .iter()
                .map(|&index| corrected_log_scores[index])
                .collect::<Vec<_>>();
            if let Some(weights) = normalized_log_weights(&local_logs) {
                if let Some(shares) = pilot_shares(&weights) {
                    if let Some(redistributed) = quota_counts(&shares, original_quota, 5) {
                        for (&index, count) in indexes.iter().zip(redistributed) {
                            counts[index] = count;
                        }
                        status = "redistributed";
                    } else {
                        status = "original_quota_kept_unaffordable";
                    }
                } else {
                    status = "original_quota_kept_invalid_scores";
                }
            } else {
                status = "original_quota_kept_zero_scores";
            }
        }
        let final_quota = indexes.iter().map(|&index| counts[index]).sum();
        for &index in &indexes {
            group_ids[index] = group_id;
            actual_shares[index] = counts[index] as f64 / final_quota as f64;
        }
        groups.push(HierarchicalPilotGroup {
            id: group_id,
            root,
            selected_ids: selected,
            indexes,
            original_quota,
            final_quota,
            status,
        });
    }
    Some(HierarchicalPilotPlan {
        counts,
        original_counts,
        original_shares,
        actual_shares,
        group_ids,
        groups,
    })
}

pub fn frozen_hierarchical_pilot_budgets(
    root_groups: &[(Vec<u8>, Vec<usize>)],
    independent_shares: &[f64],
    corrected_log_scores: &[Option<f64>],
    selected_ids: &[Vec<i32>],
    pilot_per_leaf: usize,
) -> Option<Vec<(Vec<u8>, Vec<i32>, usize)>> {
    let plan = hierarchical_pilot_plan(
        root_groups,
        independent_shares,
        corrected_log_scores,
        selected_ids,
        pilot_per_leaf,
    )?;
    Some(
        plan.groups
            .into_iter()
            .map(|group| (group.root, group.selected_ids, group.original_quota))
            .collect(),
    )
}

pub fn frozen_pilot_counts(
    roots: &[Vec<u8>],
    selected_ids: &[Vec<i32>],
    corrected_logs: &[Option<f64>],
    budgets: &[(Vec<u8>, Vec<i32>, usize)],
) -> Option<Vec<usize>> {
    if roots.len() != selected_ids.len() || roots.len() != corrected_logs.len() {
        return None;
    }
    let mut grouped = BTreeMap::<(Vec<u8>, Vec<i32>), Vec<usize>>::new();
    for index in 0..roots.len() {
        grouped
            .entry((roots[index].clone(), selected_ids[index].clone()))
            .or_default()
            .push(index);
    }
    let mut counts = vec![0; roots.len()];
    let mut seen = vec![false; roots.len()];
    let mut used_budgets = BTreeMap::<(Vec<u8>, Vec<i32>), bool>::new();
    for (root, selected, quota) in budgets {
        let key = (root.clone(), selected.clone());
        if used_budgets.insert(key.clone(), true).is_some() {
            return None;
        }
        let indexes = grouped.get(&key)?;
        if indexes.is_empty() || *quota < indexes.len().saturating_mul(5) {
            return None;
        }
        let logs = indexes
            .iter()
            .map(|&index| corrected_logs[index])
            .collect::<Vec<_>>();
        let weights = normalized_log_weights(&logs)?;
        let shares = pilot_shares(&weights)?;
        let group_counts = quota_counts(&shares, *quota, 5)?;
        for (&index, count) in indexes.iter().zip(group_counts) {
            counts[index] = count;
            seen[index] = true;
        }
    }
    (seen.iter().all(|owner| *owner) && used_budgets.len() == grouped.len()).then_some(counts)
}

pub fn hierarchical_pilot_plan_frozen(
    root_groups: &[(Vec<u8>, Vec<usize>)],
    independent_shares: &[f64],
    corrected_log_scores: &[Option<f64>],
    selected_ids: &[Vec<i32>],
    budgets: &[(Vec<u8>, Vec<i32>, usize)],
) -> Option<HierarchicalPilotPlan> {
    let leaves = independent_shares.len();
    if corrected_log_scores.len() != leaves || selected_ids.len() != leaves {
        return None;
    }
    let roots = vec![Vec::new(); leaves];
    let mut leaf_roots = roots;
    let mut owners = vec![false; leaves];
    for (root, indexes) in root_groups {
        for &index in indexes {
            if index >= leaves || std::mem::replace(&mut owners[index], true) {
                return None;
            }
            leaf_roots[index] = root.clone();
        }
    }
    if owners.iter().any(|owner| !owner)
        || independent_shares
            .iter()
            .any(|share| !share.is_finite() || *share < 0.0)
    {
        return None;
    }
    let independent_logs = independent_shares
        .iter()
        .map(|share| (*share > 0.0).then(|| share.ln()))
        .collect::<Vec<_>>();
    let original_counts =
        frozen_pilot_counts(&leaf_roots, selected_ids, &independent_logs, budgets)?;
    let budget_map = budgets
        .iter()
        .map(|(root, selected, quota)| ((root.clone(), selected.clone()), *quota))
        .collect::<BTreeMap<_, _>>();
    let mut partition = BTreeMap::<(Vec<u8>, Vec<i32>), Vec<usize>>::new();
    for index in 0..leaves {
        partition
            .entry((leaf_roots[index].clone(), selected_ids[index].clone()))
            .or_default()
            .push(index);
    }
    if partition.len() != budget_map.len() {
        return None;
    }
    let mut counts = original_counts.clone();
    let mut actual_shares = vec![0.0; leaves];
    let mut original_shares = vec![0.0; leaves];
    let mut group_ids = vec![usize::MAX; leaves];
    let mut groups = Vec::with_capacity(partition.len());
    for ((root, selected), indexes) in partition {
        let quota = *budget_map.get(&(root.clone(), selected.clone()))?;
        if quota < indexes.len().saturating_mul(5) {
            return None;
        }
        let local = indexes
            .iter()
            .map(|&i| corrected_log_scores[i])
            .collect::<Vec<_>>();
        let mut status = "original_quota_kept";
        if indexes.len() > 1 {
            if let Some(weights) = normalized_log_weights(&local) {
                if let Some(shares) = pilot_shares(&weights) {
                    if let Some(redistributed) = quota_counts(&shares, quota, 5) {
                        for (&index, count) in indexes.iter().zip(redistributed) {
                            counts[index] = count;
                        }
                        status = "redistributed_with_frozen_quota";
                    } else {
                        status = "independent_fallback_quota_too_small";
                    }
                } else {
                    status = "independent_fallback_invalid_scores";
                }
            } else {
                status = "independent_fallback_zero_scores";
            }
        }
        let group_id = groups.len();
        for &index in &indexes {
            group_ids[index] = group_id;
            original_shares[index] = original_counts[index] as f64 / quota as f64;
            actual_shares[index] = counts[index] as f64 / quota as f64;
        }
        groups.push(HierarchicalPilotGroup {
            id: group_id,
            root,
            selected_ids: selected,
            indexes,
            original_quota: quota,
            final_quota: quota,
            status,
        });
    }
    Some(HierarchicalPilotPlan {
        counts,
        original_counts: original_counts.clone(),
        original_shares,
        actual_shares,
        group_ids,
        groups,
    })
}
