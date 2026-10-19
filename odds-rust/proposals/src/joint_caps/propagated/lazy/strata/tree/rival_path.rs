//! Bounded, all-or-parent partition of a completed primary leaf by one rival.
use super::*;
pub(super) mod goal_tiebreak;
pub(super) mod shared_goal_tiebreak;
pub(super) const GOAL_TIEBREAK_WORK_LIMIT: usize = goal_tiebreak::WORK_LIMIT;
pub(super) const SHARED_GOAL_TIEBREAK_WORK_LIMIT: usize = shared_goal_tiebreak::WORK_LIMIT;

pub(super) fn apply_goal_tiebreak(
    model: &Model,
    cell: Cell,
    leaves: &[Leaf],
    corrected_logs: &mut [f64],
    screen: Option<&path_priority::Screen>,
) -> (serde_json::Value, usize) {
    let report = goal_tiebreak::apply(
        model,
        cell,
        leaves,
        corrected_logs,
        screen,
        GOAL_TIEBREAK_WORK_LIMIT,
    );
    let work = report.work;
    (
        json!({
            "enabled":report.enabled,
            "applied":report.applied,
            "reason":report.reason,
            "work":report.work,
            "cap":report.cap,
            "query_cache_hits":report.query_cache_hits,
            "kernel_cache_hits":report.kernel_cache_hits,
            "query_rows":report.query_rows,
            "margin_semantics":"rival GD minus target GD, conditional on incident WDL; optimistic equality counts with rival"
        }),
        work,
    )
}

pub(super) fn apply_shared_goal_tiebreak(
    model: &Model,
    cell: Cell,
    leaves: &[Leaf],
    corrected_logs: &mut [f64],
    screen: Option<&path_priority::Screen>,
) -> (serde_json::Value, usize) {
    apply_shared_goal_tiebreak_with_cap(
        model,
        cell,
        leaves,
        corrected_logs,
        screen,
        SHARED_GOAL_TIEBREAK_WORK_LIMIT,
    )
}

pub(super) fn apply_shared_goal_tiebreak_with_cap(
    model: &Model,
    cell: Cell,
    leaves: &[Leaf],
    corrected_logs: &mut [f64],
    screen: Option<&path_priority::Screen>,
    cap: usize,
) -> (serde_json::Value, usize) {
    let report = shared_goal_tiebreak::apply(model, cell, leaves, corrected_logs, screen, cap);
    let work = report.work;
    (
        json!({
            "enabled":report.enabled,
            "applied":report.applied,
            "reason":report.reason,
            "work":report.work,
            "cap":report.cap,
            "query_cache_hits":report.query_cache_hits,
            "kernel_cache_hits":report.kernel_cache_hits,
            "target_cache_hits":report.target_cache_hits,
            "eligible_unrefined":report.eligible_unrefined,
            "applied_unrefined":report.applied_unrefined,
            "root_coverage":report.root_coverage,
            "query_rows":report.query_rows,
            "probability_semantics":"per-rival conditional goal tie factor; opponent factors multiply as a conditional-independence approximation; full multi-team Pattern box dependence is not modeled"
        }),
        work,
    )
}

pub(super) const FINAL_LEAF_CAP: usize = 512;
pub(super) const ADDITIONAL_NODE_CAP: usize = 150_000;
pub(super) const ADDITIONAL_GUIDE_CAP: usize = 40_000_000;
pub(super) const ENUMERATION_WORK_CAP: usize = super::super::RIVAL_PATH_ENUMERATION_WORK_LIMIT;
pub(super) const PER_PARENT_NODE_CAP: usize = 20_000;

#[derive(Default)]
pub(super) struct Report {
    pub(super) enabled: bool,
    pub(super) applied: bool,
    pub(super) reason: Option<&'static str>,
    pub(super) scan_work: usize,
    pub(super) additional_nodes: usize,
    pub(super) additional_guide_values: usize,
    pub(super) parent_rows: Vec<serde_json::Value>,
    pub(super) parent_local_shares: Vec<f64>,
    pub(super) frozen_group_budgets: Vec<(Vec<u8>, Vec<i32>, usize)>,
    pub(super) pilot_counts: Option<Vec<usize>>,
    pub(super) base_pilot_counts: Option<Vec<usize>>,
    pub(super) original_base_pilot_counts: Option<Vec<usize>>,
    pub(super) original_leaf_count: usize,
}

fn proxy_log(pattern: &Pattern) -> Option<f64> {
    let mass = pattern.mass;
    let residual = pattern.joint.residual_hint;
    (mass.is_finite() && mass > 0.0 && residual.is_finite() && residual > 0.0)
        .then(|| mass.ln() + residual.ln())
        .filter(|value| value.is_finite())
}

fn normalize_logs(logs: &[f64]) -> Option<Vec<f64>> {
    let max = logs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !max.is_finite() {
        return None;
    }
    let mut values = logs
        .iter()
        .map(|value| {
            if value.is_finite() {
                (*value - max).exp()
            } else {
                0.0
            }
        })
        .collect::<Vec<_>>();
    let sum = values.iter().sum::<f64>();
    if !sum.is_finite() || sum <= 0.0 {
        return None;
    }
    values.iter_mut().for_each(|value| *value /= sum);
    Some(values)
}

fn selected_ids(leaf: &Leaf, ids: &[i32]) -> Vec<i32> {
    let mut selected = ids
        .iter()
        .enumerate()
        .filter_map(|(team, id)| (leaf.selected & (1 << team) != 0).then_some(*id))
        .collect::<Vec<_>>();
    selected.sort_unstable();
    selected
}

fn add_metered_work(report: &mut Report, amount: usize) -> bool {
    let Some(next) = report.scan_work.checked_add(amount) else {
        report.scan_work = ENUMERATION_WORK_CAP;
        return false;
    };
    if next > ENUMERATION_WORK_CAP {
        report.scan_work = ENUMERATION_WORK_CAP;
        return false;
    }
    report.scan_work = next;
    true
}

fn roots_and_groups(leaves: &[Leaf]) -> (Vec<Vec<u8>>, Vec<(Vec<u8>, Vec<usize>)>) {
    let roots = leaves
        .iter()
        .map(|leaf| leaf.root.clone())
        .collect::<Vec<_>>();
    let mut groups = Vec::<(Vec<u8>, Vec<usize>)>::new();
    let mut index = HashMap::<Vec<u8>, usize>::new();
    for (leaf_index, leaf) in leaves.iter().enumerate() {
        let group = *index.entry(leaf.root.clone()).or_insert_with(|| {
            groups.push((leaf.root.clone(), Vec::new()));
            groups.len() - 1
        });
        groups[group].1.push(leaf_index);
    }
    (roots, groups)
}

/// Refines parents in deterministic raw-root/corrected-guide order. The
/// caller has already produced base-tree diagnostics and pilot guides.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply(
    m: &Model,
    cell: Cell,
    builder: &mut LazyJoint,
    screen: Option<&path_priority::Screen>,
    leaves: &mut Vec<Leaf>,
    independent_logs: &mut Vec<f64>,
    corrected_logs: &mut Vec<f64>,
    independent_shares: &[f64],
    pilot_per_leaf: usize,
) -> Report {
    let original_leaf_count = leaves.len();
    let mut report = Report {
        enabled: true,
        original_leaf_count,
        ..Report::default()
    };
    let Some(screen) = screen.filter(|screen| screen.reason.is_none()) else {
        report.reason = Some("valid root screen unavailable");
        return report;
    };
    if leaves.is_empty()
        || independent_logs.len() != leaves.len()
        || corrected_logs.len() != leaves.len()
        || independent_shares.len() != leaves.len()
    {
        report.reason = Some("base pilot guide metadata unavailable");
        return report;
    }
    for (index, leaf) in leaves.iter_mut().enumerate() {
        leaf.original_parent_id = index;
        // Each current primary frontier leaf is now its own secondary parent.
        // Earlier hybrid path shares have already entered the parent's root
        // guide fraction; carrying them here would make the legacy parent-map
        // totals sum below one and disable its allocated exploration draws.
        leaf.local_child_share = 1.0;
    }
    let (base_roots, root_groups) = roots_and_groups(leaves);
    let base_selected = leaves
        .iter()
        .map(|leaf| selected_ids(leaf, &m.ids))
        .collect::<Vec<_>>();
    let base_independent_logs = independent_logs.clone();
    let base_corrected_logs = corrected_logs.clone();
    let independent = independent_shares.to_vec();
    let log_levels = usize::BITS as usize - leaves.len().max(1).leading_zeros() as usize;
    let base_planning_fee = leaves
        .len()
        .saturating_mul(32 + log_levels.saturating_mul(4))
        .saturating_add(root_groups.len().saturating_mul(16));
    if !add_metered_work(&mut report, base_planning_fee) {
        report.reason = Some("base pilot planning scan cap reached");
        return report;
    }
    report.frozen_group_budgets =
        match crate::pilot_allocation::frozen_hierarchical_pilot_budgets(
            &root_groups,
            &independent,
            &corrected_logs.iter().copied().map(Some).collect::<Vec<_>>(),
            &base_selected,
            pilot_per_leaf,
        ) {
            Some(budgets) => budgets,
            None => {
                report.reason = Some("base selected-group pilot budgets unavailable");
                return report;
            }
        };
    let base_count_fee = leaves
        .len()
        .saturating_mul(16 + log_levels.saturating_mul(3))
        .saturating_add(report.frozen_group_budgets.len().saturating_mul(16));
    if !add_metered_work(&mut report, base_count_fee) {
        report.reason = Some("base pilot count scan cap reached");
        return report;
    }
    let mut baseline_counts = match crate::pilot_allocation::frozen_pilot_counts(
        &base_roots,
        &base_selected,
        &base_independent_logs
            .iter()
            .copied()
            .map(Some)
            .collect::<Vec<_>>(),
        &report.frozen_group_budgets,
    ) {
        Some(counts) => counts,
        None => {
            report.reason = Some("base independent pilot allocation invalid");
            return report;
        }
    };
    report.base_pilot_counts = Some(baseline_counts.clone());
    report.original_base_pilot_counts = report.base_pilot_counts.clone();
    let mut group_counts = std::collections::HashMap::<(Vec<u8>, Vec<i32>), usize>::new();
    for (root, indexes) in &root_groups {
        for &index in indexes {
            *group_counts
                .entry((root.clone(), base_selected[index].clone()))
                .or_default() += 1;
        }
    }
    let budget_by_group = report
        .frozen_group_budgets
        .iter()
        .map(|(root, selected, quota)| ((root.clone(), selected.clone()), *quota))
        .collect::<std::collections::HashMap<_, _>>();
    let mut roots = base_roots.clone();
    let mut selected = base_selected.clone();
    let mut parent_local = vec![1.0; leaves.len()];
    let mut order = (0..leaves.len()).collect::<Vec<_>>();
    order.sort_by(|&a, &b| {
        screen.roots[leaves[b].root_index]
            .raw_score
            .total_cmp(&screen.roots[leaves[a].root_index].raw_score)
            .then_with(|| base_corrected_logs[b].total_cmp(&base_corrected_logs[a]))
            .then_with(|| a.cmp(&b))
    });
    let original_nodes = builder.nodes;
    let original_guides = builder.guide_values;
    let original_node_limit = builder.node_limit;
    let original_guide_limit = builder.guide_limit;
    builder.node_limit = original_nodes.saturating_add(ADDITIONAL_NODE_CAP);
    builder.guide_limit = original_guides.saturating_add(ADDITIONAL_GUIDE_CAP);
    let mut stopped_by_cap = false;
    for parent_id in order {
        let Some(parent_index) = leaves.iter().position(|leaf| {
            leaf.original_parent_id == parent_id
                && leaf.rival_team_id.is_none()
                // Secondary refinement rebuilds from the root rectangle. A
                // primary WDL-path descendant has inherited fixed outcomes
                // this older builder cannot preserve. Group-only descendants
                // still match the legacy case and remain eligible.
                && leaf.hybrid_path_count == 0
        }) else {
            continue;
        };
        if leaves.len() >= FINAL_LEAF_CAP {
            report.reason = Some("final leaf cap reached");
            break;
        }
        if report.scan_work >= ENUMERATION_WORK_CAP
            || builder.nodes.saturating_sub(original_nodes) >= ADDITIONAL_NODE_CAP
            || builder.guide_values.saturating_sub(original_guides) >= ADDITIONAL_GUIDE_CAP
        {
            report.reason = Some("global rival-path work cap reached");
            stopped_by_cap = true;
            break;
        }
        let parent = &leaves[parent_index];
        if parent.selected & parent.ambiguous != parent.ambiguous {
            report.parent_rows.push(json!({"parent_id":parent_id,"applied":false,"reason":"parent not fully classified"}));
            continue;
        }
        let Some(parent_proxy) = proxy_log(&parent.pattern) else {
            report.parent_rows.push(
                json!({"parent_id":parent_id,"applied":false,"reason":"invalid parent proxy"}),
            );
            continue;
        };
        let root = parent.root.clone();
        let above = parent.above;
        let mask = parent.mask;
        let selected_bits = parent.selected;
        let root_index = parent.root_index;
        let parent_ambiguous = parent.ambiguous;
        let parent_max_exceptions = parent.max_exceptions;
        let points = {
            let mut points = builder.base.clone();
            for (game, &outcome) in builder.target.games.iter().zip(&root) {
                points[game.home] += game.hg[outcome as usize];
                points[game.away] += game.ag[outcome as usize];
            }
            points
        };
        let propagation_work = builder
            .remaining
            .len()
            .saturating_mul(m.n.saturating_add(1));
        let remaining_scan = ENUMERATION_WORK_CAP.saturating_sub(report.scan_work);
        if propagation_work > remaining_scan {
            report.reason = Some("global rival-path enumeration cap reached");
            stopped_by_cap = true;
            break;
        }
        report.scan_work = report.scan_work.saturating_add(propagation_work);
        let remaining_after_propagate = ENUMERATION_WORK_CAP.saturating_sub(report.scan_work);
        let mut rivals = (0..m.n)
            .filter(|&team| team != cell.team)
            .collect::<Vec<_>>();
        rivals.sort_by_key(|&team| m.ids[team]);
        let initial = Domains::propagate(
            &builder.remaining,
            &points,
            &rivals,
            cell.rank,
            points[cell.team],
        );
        if !initial.feasible {
            report.parent_rows.push(json!({"parent_id":parent_id,"applied":false,"reason":"parent propagation unavailable"}));
            continue;
        }
        let before_nodes = builder.nodes;
        let before_guides = builder.guide_values;
        let old_limit = builder.node_limit;
        builder.node_limit = builder
            .node_limit
            .min(before_nodes.saturating_add(PER_PARENT_NODE_CAP));
        let (result, scan) = super::super::rival_path_partition(
            builder,
            &root,
            &points,
            &rivals,
            &initial,
            above,
            mask,
            selected_bits,
            remaining_after_propagate,
        );
        builder.node_limit = old_limit;
        let added_nodes = builder.nodes.saturating_sub(before_nodes);
        let added_guides = builder.guide_values.saturating_sub(before_guides);
        report.additional_nodes = builder.nodes.saturating_sub(original_nodes);
        report.additional_guide_values = builder.guide_values.saturating_sub(original_guides);
        if scan > remaining_after_propagate {
            report.scan_work = ENUMERATION_WORK_CAP;
            report.reason = Some("global rival-path enumeration cap reached");
            stopped_by_cap = true;
            report.parent_rows.push(json!({"parent_id":parent_id,"applied":false,"reason":"enumeration work cap","scan_work":scan,"additional_nodes":added_nodes,"additional_guide_values":added_guides}));
            break;
        }
        report.scan_work = report.scan_work.saturating_add(scan);
        let result = match result {
            Ok(Some((team, patterns))) => Some((team, patterns, scan)),
            Ok(None) => None,
            Err(_) => {
                report.parent_rows.push(json!({"parent_id":parent_id,"applied":false,"reason":"incomplete rival enumeration or child build","scan_work":scan,"additional_nodes":added_nodes,"additional_guide_values":added_guides}));
                continue;
            }
        };
        let Some((team, patterns, scan)) = result else {
            report.parent_rows.push(json!({"parent_id":parent_id,"applied":false,"reason":"no rival with 2..32 feasible paths","scan_work":scan,"additional_nodes":added_nodes,"additional_guide_values":added_guides}));
            continue;
        };
        if patterns.len() < 2
            || leaves
                .len()
                .saturating_add(patterns.len())
                .saturating_sub(1)
                > FINAL_LEAF_CAP
        {
            report.parent_rows.push(json!({"parent_id":parent_id,"applied":false,"reason":"child count violates final leaf cap","scan_work":scan,"additional_nodes":added_nodes,"additional_guide_values":added_guides}));
            continue;
        }
        let candidate_planning_fee = leaves
            .len()
            .saturating_mul(8)
            .saturating_add(patterns.len().saturating_mul(32))
            .saturating_add(report.frozen_group_budgets.len().saturating_mul(2));
        if !add_metered_work(&mut report, candidate_planning_fee) {
            report.reason = Some("candidate pilot planning scan cap reached");
            stopped_by_cap = true;
            break;
        }
        let child_logs = patterns.iter().map(proxy_log).collect::<Option<Vec<_>>>();
        let Some(child_logs) = child_logs else {
            report.parent_rows.push(json!({"parent_id":parent_id,"applied":false,"reason":"invalid child proxy; parent retained","scan_work":scan,"additional_nodes":added_nodes,"additional_guide_values":added_guides}));
            continue;
        };
        let Some(local_shares) = normalize_logs(&child_logs) else {
            report.parent_rows.push(json!({"parent_id":parent_id,"applied":false,"reason":"child proxy normalization underflow","scan_work":scan,"additional_nodes":added_nodes,"additional_guide_values":added_guides}));
            continue;
        };
        let parent_ind_log = base_independent_logs[parent_id];
        let parent_corrected_log = base_corrected_logs[parent_id];
        if !parent_ind_log.is_finite() || !parent_corrected_log.is_finite() {
            report.parent_rows.push(json!({"parent_id":parent_id,"applied":false,"reason":"invalid parent guide log","scan_work":scan,"additional_nodes":added_nodes,"additional_guide_values":added_guides}));
            continue;
        }
        let group_key = (root.clone(), selected[parent_index].clone());
        let current_group_count = group_counts.get(&group_key).copied().unwrap_or(0);
        let group_quota = budget_by_group.get(&group_key).copied().unwrap_or(0);
        let patterns_len = patterns.len();
        let expanded_group_count =
            current_group_count.saturating_add(patterns_len.saturating_sub(1));
        if group_quota < expanded_group_count.saturating_mul(5) {
            report.parent_rows.push(json!({"parent_id":parent_id,"applied":false,"reason":"original selected-group quota cannot support child floors","scan_work":scan,"additional_nodes":added_nodes,"additional_guide_values":added_guides}));
            continue;
        }
        let mut replacements = Vec::new();
        for (path_id, (pattern, (proxy, local_share))) in patterns
            .into_iter()
            .zip(child_logs.into_iter().zip(local_shares.into_iter()))
            .enumerate()
        {
            let correction = proxy - parent_proxy;
            let independent_log = parent_ind_log + local_share.ln();
            let corrected_log = parent_corrected_log + correction;
            if !independent_log.is_finite() || !corrected_log.is_finite() {
                replacements.clear();
                break;
            }
            let fixed = pattern
                .fixed
                .iter()
                .copied()
                .filter(|(fixture, _)| {
                    m.fixtures.get(*fixture).is_some_and(|f| {
                        (f.home == team || f.away == team)
                            && builder.remaining.iter().any(|game| game.index == *fixture)
                    })
                })
                .collect::<Vec<_>>();
            let raw_prior = fixed.iter().fold(1.0, |mass, &(fixture, outcome)| {
                mass * m.fixtures[fixture].prob[outcome as usize]
            });
            if !raw_prior.is_finite() || raw_prior <= 0.0 {
                replacements.clear();
                break;
            }
            let mut child = Leaf {
                root: root.clone(),
                root_index,
                above,
                mask,
                selected: selected_bits,
                ambiguous: parent_ambiguous,
                max_exceptions: parent_max_exceptions,
                pattern,
                frozen: true,
                score: 0.0,
                rank_hint_next_team: None,
                original_parent_id: parent_id,
                rival_team_id: Some(team),
                path_id: Some(path_id),
                rival_fixed: fixed,
                raw_path_prior: raw_prior,
                parent_proxy_log: Some(parent_proxy),
                child_proxy_log: Some(proxy),
                local_child_share: local_share,
                guide_fraction: parent.guide_fraction,
                hybrid_depth: parent.hybrid_depth,
                hybrid_path_count: 0,
            };
            child.score = corrected_log.exp();
            replacements.push((child, independent_log, corrected_log, local_share));
        }
        let inserted = replacements.len();
        if inserted < 2 || inserted != patterns_len {
            report.parent_rows.push(json!({"parent_id":parent_id,"applied":false,"reason":"invalid child normalizer or prior","scan_work":scan,"additional_nodes":added_nodes,"additional_guide_values":added_guides}));
            continue;
        }
        let parent_selected_ids = selected[parent_index].clone();
        let mut trial_roots = roots.clone();
        let mut trial_selected = selected.clone();
        let mut trial_independent = independent_logs.clone();
        let mut trial_corrected = corrected_logs.clone();
        trial_roots.splice(
            parent_index..=parent_index,
            std::iter::repeat(root.clone()).take(inserted),
        );
        trial_selected.splice(
            parent_index..=parent_index,
            std::iter::repeat(parent_selected_ids.clone()).take(inserted),
        );
        trial_independent.splice(
            parent_index..=parent_index,
            replacements.iter().map(|(_, i, _, _)| *i),
        );
        trial_corrected.splice(
            parent_index..=parent_index,
            replacements.iter().map(|(_, _, c, _)| *c),
        );
        let trial_count_fee = trial_roots
            .len()
            .saturating_mul(
                16 + (usize::BITS as usize - trial_roots.len().max(1).leading_zeros() as usize)
                    .saturating_mul(3),
            )
            .saturating_add(report.frozen_group_budgets.len().saturating_mul(16));
        if !add_metered_work(&mut report, trial_count_fee) {
            report.reason = Some("candidate pilot count scan cap reached");
            stopped_by_cap = true;
            break;
        }
        let Some(trial_independent_counts) = crate::pilot_allocation::frozen_pilot_counts(
            &trial_roots,
            &trial_selected,
            &trial_independent
                .iter()
                .copied()
                .map(Some)
                .collect::<Vec<_>>(),
            &report.frozen_group_budgets,
        ) else {
            report.parent_rows.push(json!({"parent_id":parent_id,"applied":false,"reason":"original-group independent child allocation invalid","scan_work":scan,"additional_nodes":added_nodes,"additional_guide_values":added_guides}));
            continue;
        };
        leaves.splice(
            parent_index..=parent_index,
            replacements.iter().map(|(child, _, _, _)| child.clone()),
        );
        independent_logs.splice(
            parent_index..=parent_index,
            replacements.iter().map(|(_, i, _, _)| *i),
        );
        corrected_logs.splice(
            parent_index..=parent_index,
            replacements.iter().map(|(_, _, c, _)| *c),
        );
        baseline_counts = trial_independent_counts;
        roots.splice(
            parent_index..=parent_index,
            std::iter::repeat_n(root.clone(), inserted),
        );
        selected.splice(
            parent_index..=parent_index,
            std::iter::repeat_n(parent_selected_ids, inserted),
        );
        *group_counts.entry(group_key).or_default() = expanded_group_count;
        parent_local.splice(
            parent_index..=parent_index,
            replacements.iter().map(|(_, _, _, share)| *share),
        );
        report.applied = true;
        report.parent_rows.push(json!({"parent_id":parent_id,"applied":true,"rival_team_id":m.ids[team],"child_count":inserted,"scan_work":scan,"additional_nodes":added_nodes,"additional_guide_values":added_guides,"parent_proxy_log":parent_proxy,"child_proxy_logs":replacements.iter().map(|(child,_,_,_)|child.child_proxy_log).collect::<Vec<_>>(),"local_child_shares":replacements.iter().map(|(_,_,_,share)|share).collect::<Vec<_>>()}));
    }
    builder.node_limit = original_node_limit;
    builder.guide_limit = original_guide_limit;
    report.parent_local_shares = parent_local;
    report.base_pilot_counts = Some(baseline_counts.clone());
    let final_log_levels = usize::BITS as usize - leaves.len().max(1).leading_zeros() as usize;
    let final_count_fee = leaves
        .len()
        .saturating_mul(16 + final_log_levels.saturating_mul(3))
        .saturating_add(report.frozen_group_budgets.len().saturating_mul(16));
    if add_metered_work(&mut report, final_count_fee) {
        report.pilot_counts = crate::pilot_allocation::frozen_pilot_counts(
            &roots,
            &selected,
            &corrected_logs.iter().copied().map(Some).collect::<Vec<_>>(),
            &report.frozen_group_budgets,
        )
        .or(Some(baseline_counts.clone()));
    } else {
        report.pilot_counts = Some(baseline_counts);
        report.reason = Some("final pilot count scan cap reached");
    }
    if !report.applied && report.reason.is_none() {
        report.reason = Some(if stopped_by_cap {
            "rival-path work cap reached"
        } else {
            "no eligible complete rival partition"
        });
    }
    report
}
