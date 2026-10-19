//! Bounded, opt-in final complete-tree rescue for weak championship rank cells.
use super::{accepted, budget, emit};
use crate::{
    conditioned::Result,
    joint_caps::propagated::lazy::{combine_branches, BranchSample, BranchStrata},
    model::Model,
    pool::Estimate,
    rng::derive,
    search::Cell,
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
pub(crate) use golaberto_odds_core::pilot_allocation::{
    frozen_hierarchical_pilot_budgets, frozen_pilot_counts, hierarchical_pilot_plan,
    hierarchical_pilot_plan_frozen, pilot_shares, quota_counts, HierarchicalPilotGroup,
    HierarchicalPilotPlan,
};
use std::time::Instant;

mod rank_bounds;
mod structural;

const ROOT_LIMIT: usize = 256;
const MAX_ATTEMPTS: usize = 4;
const ROOT_PROBE_ADMISSION_FLOOR: usize = 1_000_000;
const TREE_SETUP_ADMISSION_FLOOR: usize = 81_600_000;
const BOUNDED_OMISSION_WORK_DEFAULT: usize = 8_000_000;
const BOUNDED_OMISSION_WORK_MAX: usize = 512_000_000;
const RIVAL_BOUND_NODE_CAP: usize = 40_000;
const RIVAL_BOUND_EDGE_CAP: usize = 9;
const RIVAL_BOUND_WORK_CAP_PER_ROOT: usize = 80_000;
const NA_GUIDANCE_WORK_DEFAULT: usize = 8_000_000;
const NA_GUIDANCE_WORK_MAX: usize = 512_000_000;

#[derive(Clone, Copy)]
pub(crate) struct Settings {
    pub(crate) enabled: bool,
    reference_draws: usize,
    final_draws: usize,
    pilot_per_leaf: usize,
    max_attempts: usize,
    target_filter: Option<(i32, usize)>,
    target_filter_error: Option<&'static str>,
    path_priority: bool,
    path_sampling: bool,
    root_exploration: bool,
    dual_sigma: bool,
    pilot_support: bool,
    leaf_pilot: bool,
    pair_pilot: bool,
    hierarchical_pair_pilot: bool,
    rival_path: bool,
    large_budget: bool,
    goal_tiebreak: bool,
    shared_goal_tiebreak: bool,
    shared_goal_tiebreak_work_limit: usize,
    bounded_omission: bool,
    bounded_omission_work: usize,
    structural_recovery: bool,
    na_guidance: bool,
    na_guidance_work: usize,
    hybrid_tree_work: usize,
}

const SHARED_GOAL_TIEBREAK_WORK_DEFAULT: usize = 64_000_000;
const SHARED_GOAL_TIEBREAK_WORK_MAX: usize = 512_000_000;

fn setting(raw: Option<&str>, default: usize, low: usize, high: usize) -> usize {
    raw.and_then(|v| v.parse::<usize>().ok())
        .map_or(default, |v| v.clamp(low, high))
}
fn budget_caps(
    rescue_enabled: bool,
    large_requested: Option<&str>,
    work_requested: Option<&str>,
    final_requested: Option<&str>,
) -> (bool, usize, usize) {
    let large = rescue_enabled && enabled_value(large_requested);
    (
        large,
        setting(
            work_requested,
            90_000,
            90_000,
            if large { 20_000_000 } else { 2_000_000 },
        ),
        setting(
            final_requested,
            600_000,
            1,
            if large { 6_000_000 } else { 600_000 },
        ),
    )
}
fn enabled_value(raw: Option<&str>) -> bool {
    raw == Some("1")
}
fn bounded_omission_enabled(
    rescue_enabled: bool,
    path_sampling: bool,
    leaf_pilot: bool,
    raw: Option<&str>,
) -> bool {
    rescue_enabled && !path_sampling && leaf_pilot && enabled_value(raw)
}
fn omission_old_probability_valid(probability: f64) -> bool {
    probability.is_finite() && probability > 0.0
}
fn bounded_omission_work(enabled: bool, large_budget: bool, raw: Option<&str>) -> usize {
    if enabled && large_budget {
        setting(
            raw,
            BOUNDED_OMISSION_WORK_DEFAULT,
            BOUNDED_OMISSION_WORK_DEFAULT,
            BOUNDED_OMISSION_WORK_MAX,
        )
    } else {
        BOUNDED_OMISSION_WORK_DEFAULT
    }
}
fn path_priority_enabled(rescue_enabled: bool, raw: Option<&str>) -> bool {
    rescue_enabled && enabled_value(raw)
}
fn path_sampling_enabled(rescue_enabled: bool, path_priority: bool, raw: Option<&str>) -> bool {
    rescue_enabled && path_priority && enabled_value(raw)
}
fn root_exploration_enabled(
    rescue_enabled: bool,
    path_priority: bool,
    path_sampling: bool,
    raw: Option<&str>,
) -> bool {
    rescue_enabled && path_priority && !path_sampling && enabled_value(raw)
}
fn dual_sigma_enabled(
    rescue_enabled: bool,
    path_priority: bool,
    leaf_pilot: bool,
    path_sampling: bool,
    raw: Option<&str>,
) -> bool {
    rescue_enabled && path_priority && leaf_pilot && !path_sampling && enabled_value(raw)
}
fn pilot_support_enabled(
    dual_sigma: bool,
    root_exploration: bool,
    hierarchical_pair_pilot: bool,
    raw: Option<&str>,
) -> bool {
    dual_sigma && root_exploration && hierarchical_pair_pilot && enabled_value(raw)
}
fn leaf_pilot_enabled(rescue_enabled: bool, path_priority: bool, raw: Option<&str>) -> bool {
    rescue_enabled && path_priority && enabled_value(raw)
}
fn pair_pilot_enabled(
    rescue_enabled: bool,
    path_priority: bool,
    leaf_pilot: bool,
    raw: Option<&str>,
) -> bool {
    rescue_enabled && path_priority && leaf_pilot && enabled_value(raw)
}
fn hierarchical_pair_pilot_enabled(
    rescue_enabled: bool,
    path_priority: bool,
    leaf_pilot: bool,
    pair_pilot: bool,
    raw: Option<&str>,
) -> bool {
    rescue_enabled && path_priority && leaf_pilot && pair_pilot && enabled_value(raw)
}
fn rival_path_enabled(
    enabled: bool,
    path_priority: bool,
    leaf_pilot: bool,
    pair_pilot: bool,
    hierarchical_pair_pilot: bool,
    dual_sigma: bool,
    path_sampling: bool,
    raw: Option<&str>,
) -> bool {
    enabled
        && path_priority
        && leaf_pilot
        && pair_pilot
        && hierarchical_pair_pilot
        && dual_sigma
        && !path_sampling
        && enabled_value(raw)
}
fn goal_tiebreak_enabled(rival_path: bool, raw: Option<&str>) -> bool {
    rival_path && enabled_value(raw)
}
fn shared_goal_tiebreak_enabled(goal_tiebreak: bool, raw: Option<&str>) -> bool {
    goal_tiebreak && enabled_value(raw)
}
fn shared_goal_tiebreak_work_limit(
    shared_goal_tiebreak: bool,
    large_budget: bool,
    raw: Option<&str>,
) -> usize {
    if shared_goal_tiebreak && large_budget {
        setting(
            raw,
            SHARED_GOAL_TIEBREAK_WORK_DEFAULT,
            SHARED_GOAL_TIEBREAK_WORK_DEFAULT,
            SHARED_GOAL_TIEBREAK_WORK_MAX,
        )
    } else {
        SHARED_GOAL_TIEBREAK_WORK_DEFAULT
    }
}



fn next_up(value: f64) -> f64 {
    if !value.is_finite() || value == f64::INFINITY {
        return value;
    }
    if value == 0.0 {
        return f64::from_bits(1);
    }
    f64::from_bits(if value > 0.0 {
        value.to_bits() + 1
    } else {
        value.to_bits() - 1
    })
}

fn next_down(value: f64) -> f64 {
    if !value.is_finite() || value == f64::NEG_INFINITY {
        return value;
    }
    if value == 0.0 {
        return -f64::from_bits(1);
    }
    f64::from_bits(if value > 0.0 {
        value.to_bits() - 1
    } else {
        value.to_bits() + 1
    })
}

fn freeze_omissions(old_probability: f64, bounds: &[f64]) -> (Vec<bool>, f64, f64) {
    let mut omitted = vec![false; bounds.len()];
    if !old_probability.is_finite() || old_probability <= 0.0 || bounds.len() < 2 {
        return (omitted, 0.0, 0.0);
    }
    if bounds
        .iter()
        .any(|bound| !bound.is_finite() || *bound <= 0.0)
    {
        return (omitted, 0.0, 0.0);
    }
    let raw_allowance = (9.0 * old_probability).min(1.0);
    let allowance = if raw_allowance < 1.0 {
        next_down(raw_allowance).max(0.0)
    } else {
        1.0
    };
    let mut order = (0..bounds.len()).collect::<Vec<_>>();
    order.sort_by(|&a, &b| bounds[a].total_cmp(&bounds[b]).then_with(|| a.cmp(&b)));
    // Keep the largest bound unconditionally; tied bounds use the greatest
    // stable leaf index as that final retained leaf.
    let mut ledger = 0.0;
    for &leaf in order.iter().take(order.len() - 1) {
        let sum = next_up(ledger + bounds[leaf]);
        if sum <= allowance {
            omitted[leaf] = true;
            ledger = sum;
        }
    }
    (omitted, ledger, allowance)
}

fn coarse_bracket(probability: f64, std_err: f64, omitted_bound: f64) -> Option<(f64, f64)> {
    if !probability.is_finite()
        || probability < 0.0
        || !std_err.is_finite()
        || std_err < 0.0
        || !omitted_bound.is_finite()
        || omitted_bound < 0.0
    {
        return None;
    }
    let lower = (probability - 1.96 * std_err).max(0.0);
    let upper = (probability + 1.96 * std_err + omitted_bound).min(1.0);
    (lower.is_finite() && upper.is_finite() && upper >= lower).then_some((lower, upper))
}

fn omission_prefix_curve(bounds: &[f64], omitted: &[bool]) -> Vec<(usize, f64)> {
    let mut ids = (0..bounds.len())
        .filter(|&leaf| omitted.get(leaf).copied().unwrap_or(false))
        .collect::<Vec<_>>();
    ids.sort_by(|&a, &b| bounds[a].total_cmp(&bounds[b]).then_with(|| a.cmp(&b)));
    let mut ledger = 0.0;
    ids.into_iter()
        .map(|leaf| {
            ledger = next_up(ledger + bounds[leaf]);
            (leaf, ledger)
        })
        .collect()
}

fn omission_selection_work(leaves: usize) -> usize {
    let levels = usize::BITS as usize - leaves.max(1).leading_zeros() as usize;
    leaves.saturating_mul(levels.saturating_mul(2).saturating_add(6))
}

fn coarse_bracket_pair_accepted(main: &Result, check: &Result, omitted_bound: f64) -> bool {
    let Some((main_lower, main_upper)) =
        coarse_bracket(main.probability, main.std_err, omitted_bound)
    else {
        return false;
    };
    let Some((check_lower, check_upper)) =
        coarse_bracket(check.probability, check.std_err, omitted_bound)
    else {
        return false;
    };
    [(main_lower, main_upper), (check_lower, check_upper)]
        .iter()
        .all(|(lower, upper)| *lower > 0.0 && *upper / *lower <= 10.0)
        && accepted(main, check, true)
}

fn omission_slot<T>(omitted: bool, default: T, sample: impl FnOnce() -> T) -> T {
    if omitted {
        default
    } else {
        sample()
    }
}



fn leaf_plan_work(tree_work: usize, leaves: usize, roots: usize, fixtures: usize) -> Option<usize> {
    let work = leaves
        .saturating_add(roots)
        .saturating_mul(fixtures.saturating_add(64));
    (tree_work.saturating_add(work) <= BranchStrata::rank_rescue_leaf_pilot_limit()).then_some(work)
}

fn hierarchical_leaf_plan_extra_work(
    tree_work: usize,
    leaves: usize,
    roots: usize,
    fixtures: usize,
    teams: usize,
) -> Option<usize> {
    let base = leaf_plan_work(tree_work, leaves, roots, fixtures)?;
    let log_levels = usize::BITS as usize - leaves.max(1).leading_zeros() as usize;
    let extra = leaves
        .saturating_add(roots)
        .saturating_mul(fixtures.saturating_add(teams).saturating_add(32))
        .saturating_add(leaves.saturating_mul(log_levels).saturating_mul(4));
    (tree_work.saturating_add(base).saturating_add(extra)
        <= BranchStrata::rank_rescue_leaf_pilot_limit())
    .then_some(extra)
}









/// Immutable selected-set budgets from the base tree. Rival-path refinement
/// may redistribute within these groups, but never recomputes expanded totals.


/// Redistribute an immutable base group budget among refined leaves using the
/// same hierarchical 90/10 structural smoothing, Hamilton rounding, and floor.




fn forecast_pilot_counts(proposal: &BranchStrata, model: &Model, counts: &[usize]) -> usize {
    counts
        .iter()
        .enumerate()
        .map(|(index, count)| {
            count.saturating_mul(
                proposal
                    .draw_work(model, index, true)
                    .saturating_add(proposal.draw_work(model, index, false)),
            )
        })
        .sum()
}

fn shares_from_counts(root_groups: &[(Vec<u8>, Vec<usize>)], counts: &[usize]) -> Vec<Option<f64>> {
    let mut shares = vec![None; counts.len()];
    for (_, indexes) in root_groups {
        let total = indexes
            .iter()
            .map(|&index| counts.get(index).copied().unwrap_or(0))
            .sum::<usize>();
        if total > 0 {
            for &index in indexes {
                if let Some(count) = counts.get(index) {
                    shares[index] = Some(*count as f64 / total as f64);
                }
            }
        }
    }
    shares
}

fn parse_target_filter(
    raw: Option<&str>,
) -> std::result::Result<Option<(i32, usize)>, &'static str> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let Some((team, rank)) = raw.split_once(':') else {
        return Err("expected TEAM_ID:RANK");
    };
    let team_id = team
        .parse::<i32>()
        .map_err(|_| "team ID must be an integer")?;
    let rank = rank
        .parse::<usize>()
        .map_err(|_| "rank must be a positive integer")?;
    if rank == 0 {
        return Err("rank is one-based and must be positive");
    }
    Ok(Some((team_id, rank)))
}

fn target_cell(
    target: Option<(i32, usize)>,
    ids: &[i32],
    n: usize,
) -> std::result::Result<Option<usize>, &'static str> {
    let Some((team_id, rank)) = target else {
        return Ok(None);
    };
    if rank > n {
        return Err("rank is outside the model");
    }
    let team = ids
        .iter()
        .position(|id| *id == team_id)
        .ok_or("team ID is outside the model")?;
    Ok(Some(team * n + rank - 1))
}

impl Settings {
    pub(crate) fn from_env() -> Self {
        let raw_target = std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_TARGET").ok();
        let (target_filter, target_filter_error) = match parse_target_filter(raw_target.as_deref())
        {
            Ok(target) => (target, None),
            Err(reason) => (None, Some(reason)),
        };
        let enabled = enabled_value(
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE")
                .ok()
                .as_deref(),
        );
        let structural_recovery = enabled
            && enabled_value(
                std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_STRUCTURAL_RECOVERY")
                    .ok()
                    .as_deref(),
            );
        let path_priority = path_priority_enabled(
            enabled,
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_PATH_PRIORITY")
                .ok()
                .as_deref(),
        );
        let path_sampling = path_sampling_enabled(
            enabled,
            path_priority,
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_PATH_SAMPLING")
                .ok()
                .as_deref(),
        );
        let root_exploration = root_exploration_enabled(
            enabled,
            path_priority,
            path_sampling,
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_ROOT_EXPLORATION")
                .ok()
                .as_deref(),
        );
        let leaf_pilot = leaf_pilot_enabled(
            enabled,
            path_priority,
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_LEAF_PILOT")
                .ok()
                .as_deref(),
        );
        let pair_pilot = pair_pilot_enabled(
            enabled,
            path_priority,
            leaf_pilot,
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_PAIR_PILOT")
                .ok()
                .as_deref(),
        );
        let hierarchical_pair_pilot = hierarchical_pair_pilot_enabled(
            enabled,
            path_priority,
            leaf_pilot,
            pair_pilot,
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_HIERARCHICAL_PAIR_PILOT")
                .ok()
                .as_deref(),
        );
        let dual_sigma = dual_sigma_enabled(
            enabled,
            path_priority,
            leaf_pilot,
            path_sampling,
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_DUAL_SIGMA")
                .ok()
                .as_deref(),
        );
        let pilot_support = pilot_support_enabled(
            dual_sigma,
            root_exploration,
            hierarchical_pair_pilot,
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_PILOT_SUPPORT")
                .ok()
                .as_deref(),
        );
        let rival_path = rival_path_enabled(
            enabled,
            path_priority,
            leaf_pilot,
            pair_pilot,
            hierarchical_pair_pilot,
            dual_sigma,
            path_sampling,
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_RIVAL_PATH")
                .ok()
                .as_deref(),
        );
        let (large_budget, reference_draws, final_draws) = budget_caps(
            enabled,
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_LARGE_BUDGET")
                .ok()
                .as_deref(),
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_WORK_DRAWS")
                .ok()
                .as_deref(),
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_FINAL_DRAWS")
                .ok()
                .as_deref(),
        );
        let bounded_omission = bounded_omission_enabled(
            enabled,
            path_sampling,
            leaf_pilot,
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_BOUNDED_OMISSION")
                .ok()
                .as_deref(),
        );
        let bounded_omission_work = bounded_omission_work(
            bounded_omission,
            large_budget,
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_BOUNDED_OMISSION_WORK")
                .ok()
                .as_deref(),
        );
        let goal_tiebreak = goal_tiebreak_enabled(
            rival_path,
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_GOAL_TIEBREAK")
                .ok()
                .as_deref(),
        );
        let shared_goal_tiebreak = shared_goal_tiebreak_enabled(
            goal_tiebreak,
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_SHARED_GOAL_TIEBREAK")
                .ok()
                .as_deref(),
        );
        let shared_goal_tiebreak_work_limit = shared_goal_tiebreak_work_limit(
            shared_goal_tiebreak,
            large_budget,
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_SHARED_GOAL_TIEBREAK_WORK")
                .ok()
                .as_deref(),
        );
        let na_guidance = enabled
            && path_priority
            && leaf_pilot
            && pair_pilot
            && rival_path
            && std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_NA_GUIDANCE").as_deref() == Ok("1");
        let na_guidance_work = if na_guidance {
            if large_budget {
                setting(
                    std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_NA_WORK")
                        .ok()
                        .as_deref(),
                    NA_GUIDANCE_WORK_DEFAULT,
                    NA_GUIDANCE_WORK_DEFAULT,
                    NA_GUIDANCE_WORK_MAX,
                )
            } else {
                NA_GUIDANCE_WORK_DEFAULT
            }
        } else {
            0
        };
        let hybrid_mode = std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_HYBRID_TREE");
        let hybrid_tree = enabled
            && path_priority
            && leaf_pilot
            && pair_pilot
            && rival_path
            && matches!(hybrid_mode.as_deref(), Ok("paths") | Ok("mixed"));
        let hybrid_tree_work = if hybrid_tree {
            BranchStrata::rank_rescue_hybrid_tree_work_limit(large_budget)
        } else { 0 };
        Self {
            enabled,
            reference_draws,
            final_draws,
            pilot_per_leaf: setting(
                std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_PILOT")
                    .ok()
                    .as_deref(),
                25,
                5,
                5_000,
            ),
            max_attempts: setting(
                std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_MAX_CELLS")
                    .ok()
                    .as_deref(),
                2,
                1,
                MAX_ATTEMPTS,
            ),
            target_filter,
            target_filter_error,
            path_priority,
            path_sampling,
            root_exploration,
            dual_sigma,
            pilot_support,
            leaf_pilot,
            pair_pilot,
            hierarchical_pair_pilot,
            rival_path,
            large_budget,
            goal_tiebreak,
            shared_goal_tiebreak,
            shared_goal_tiebreak_work_limit,
            bounded_omission,
            bounded_omission_work,
            structural_recovery,
            na_guidance,
            na_guidance_work,
            hybrid_tree_work,
        }
    }
}

#[derive(Clone, Copy)]
struct Candidate {
    cell: Cell,
    roots: usize,
    root_work: usize,
    bound_pruned: usize,
    bound_work: usize,
    was_positive: bool,
}

#[derive(Default)]
struct RootBoundStats {
    examined: usize,
    certified: usize,
    pruned: usize,
    work: usize,
    unknown: usize,
}

fn structural_proxy_shares(setup: &serde_json::Value, leaves: usize) -> Option<Vec<f64>> {
    let recovery = setup.get("tree")?.get("structural_recovery")?;
    if recovery.get("enabled")?.as_bool()? != true
        || recovery.get("applied")?.as_bool()? != true
    {
        return None;
    }
    let rows = recovery.get("leaves")?.as_array()?;
    if rows.len() != leaves {
        return None;
    }
    let shares = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            (row.get("index")?.as_u64()? == index as u64)
                .then(|| row.get("structural_proxy_share")?.as_f64())?
        })
        .collect::<Option<Vec<_>>>()?;
    if shares.iter().any(|share| !share.is_finite() || *share < 0.0) {
        return None;
    }
    let sum = shares.iter().sum::<f64>();
    if !sum.is_finite() || sum <= 0.0 {
        return None;
    }
    Some(shares.into_iter().map(|share| share / sum).collect())
}

fn structural_final_shares(
    leaf_shares: &[f64],
    root_groups: Option<&[(Vec<u8>, f64, Vec<usize>)]>,
    structural_groups: Option<&[(Vec<u8>, Vec<usize>)]>,
    parent_support: Option<&[f64]>,
) -> Option<Vec<f64>> {
    if leaf_shares.is_empty()
        || leaf_shares
            .iter()
            .any(|share| !share.is_finite() || *share < 0.0)
    {
        return None;
    }
    let n = leaf_shares.len();
    let mut groups: Vec<(Vec<u8>, f64, Vec<usize>)> = match root_groups {
        Some(groups) => groups.to_vec(),
        None => structural_groups.map_or_else(
            || vec![(Vec::new(), 1.0, (0..n).collect())],
            |groups| {
                groups
                    .iter()
                    .map(|(root, indexes)| (root.clone(), 1.0, indexes.clone()))
                    .collect()
            },
        ),
    };
    let mut owner = vec![false; n];
    for (_, score, indexes) in &groups {
        if !score.is_finite() || *score < 0.0 || indexes.is_empty() {
            return None;
        }
        for &index in indexes {
            if index >= n || std::mem::replace(&mut owner[index], true) {
                return None;
            }
        }
    }
    if owner.iter().any(|seen| !seen) {
        return None;
    }
    let root_weights = normalize_weights(&groups.iter().map(|group| group.1).collect::<Vec<_>>())?;
    let mut shares = vec![0.0; n];
    for ((_, _, indexes), root_weight) in groups.drain(..).zip(root_weights) {
        let local = normalize_weights(&indexes.iter().map(|&index| leaf_shares[index]).collect::<Vec<_>>())?;
        for (&index, share) in indexes.iter().zip(local) {
            shares[index] = root_weight * share;
        }
    }
    if let Some(support) = parent_support {
        if support.len() == n
            && support.iter().all(|share| share.is_finite() && *share >= 0.0)
            && support.iter().sum::<f64>().is_finite()
            && support.iter().sum::<f64>() > 0.0
        {
            let support = normalize_weights(support)?;
            for (share, support) in shares.iter_mut().zip(support) {
                *share = 0.9 * *share + 0.1 * support;
            }
        }
    }
    normalize_weights(&shares)
}

fn standard_wdl_rules(m: &Model) -> bool {
    m.request.phase.bonus_points == 0
        && (m.request.phase.championship.point_win,
            m.request.phase.championship.point_draw,
            m.request.phase.championship.point_loss)
            == (3, 1, 0)
        && m.keys.first() == Some(&crate::model::Key::Pt)
        && m.keys.get(1) == Some(&crate::model::Key::W)
}

/// A safe points/wins upper bound for one terminal target-game root. Any
/// unsupported shape or exhausted bound returns `None`, which keeps the root.
fn root_maximum_possible_above(m: &Model, cell: Cell, root: &[u8]) -> (Option<usize>, usize) {
    let scan_levels = (usize::BITS as usize - m.fixtures.len().max(1).leading_zeros() as usize)
        .saturating_add(4);
    let base_work = m
        .fixtures
        .len()
        .saturating_mul(scan_levels)
        .saturating_add(m.n.saturating_mul(RIVAL_BOUND_EDGE_CAP.saturating_add(16)));
    if !standard_wdl_rules(m) || m.n > 64 || m.base.iter().take(m.n).any(|team| team.wins < 0) {
        return (None, base_work.max(1));
    }
    let target_games = m
        .fixtures
        .iter()
        .enumerate()
        .filter(|(_, fixture)| fixture.home == cell.team || fixture.away == cell.team)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if target_games.len() != root.len() || root.iter().any(|outcome| *outcome > 2) {
        return (None, base_work.max(1));
    }

    let mut points = m.base.iter().map(|campaign| campaign.points).collect::<Vec<_>>();
    let mut wins = m.base.iter().map(|campaign| campaign.wins.max(0) as usize).collect::<Vec<_>>();
    for (&fixture_index, &outcome) in target_games.iter().zip(root) {
        let fixture = &m.fixtures[fixture_index];
        let (home_points, home_wins, away_points, away_wins) = match outcome {
            0 => (0, 0, 3, 1),
            1 => (1, 0, 1, 0),
            2 => (3, 1, 0, 0),
            _ => unreachable!(),
        };
        points[fixture.home] = points[fixture.home].saturating_add(home_points);
        points[fixture.away] = points[fixture.away].saturating_add(away_points);
        wins[fixture.home] = wins[fixture.home].saturating_add(home_wins);
        wins[fixture.away] = wins[fixture.away].saturating_add(away_wins);
    }
    let target = (points[cell.team], wins[cell.team]);
    let target_set = target_games.into_iter().collect::<BTreeSet<_>>();
    let rival_ids = (0..m.n).filter(|&team| team != cell.team).collect::<Vec<_>>();
    let mut rival_slot = vec![None; m.n];
    for (slot, &team) in rival_ids.iter().enumerate() {
        rival_slot[team] = Some(slot);
    }
    let mut degrees = vec![0usize; rival_ids.len()];
    for (fixture_index, fixture) in m.fixtures.iter().enumerate() {
        if target_set.contains(&fixture_index) {
            continue;
        }
        let home = rival_slot.get(fixture.home).copied().flatten();
        let away = rival_slot.get(fixture.away).copied().flatten();
        if let Some(slot) = home {
            degrees[slot] = degrees[slot].saturating_add(1);
        }
        if let Some(slot) = away {
            degrees[slot] = degrees[slot].saturating_add(1);
        }
    }
    let individually_eligible: Vec<bool> = rival_ids
        .iter()
        .enumerate()
        .map(|(slot, &team)| {
            (points[team].saturating_add(3 * degrees[slot] as i32),
                wins[team].saturating_add(degrees[slot])) >= target
        })
        .collect();
    let slack = |slot: usize| {
        let score = (
            points[rival_ids[slot]].saturating_add(3 * degrees[slot] as i32),
            wins[rival_ids[slot]].saturating_add(degrees[slot]),
        );
        (score.0 as i64 - target.0 as i64) * (m.n as i64 + 1)
            + score.1 as i64 - target.1 as i64
    };
    let mut candidate_edges = Vec::new();
    for (fixture_index, fixture) in m.fixtures.iter().enumerate() {
        if target_set.contains(&fixture_index) {
            continue;
        }
        let home = rival_slot.get(fixture.home).copied().flatten();
        let away = rival_slot.get(fixture.away).copied().flatten();
        if let (Some(a), Some(b)) = (home, away) {
            if a != b && individually_eligible[a] && individually_eligible[b] {
                candidate_edges.push((slack(a).saturating_add(slack(b)), fixture_index, a, b));
            }
        }
    }
    candidate_edges.sort_by_key(|edge| (edge.0, edge.1));
    let work_per_node = m.n.saturating_mul(2).saturating_add(1).max(1);
    let node_cap = RIVAL_BOUND_WORK_CAP_PER_ROOT
        .saturating_sub(base_work)
        .checked_div(work_per_node)
        .unwrap_or(0)
        .min(RIVAL_BOUND_NODE_CAP);
    let edge_cap_for_budget = (0..=RIVAL_BOUND_EDGE_CAP)
        .take_while(|edges| {
            (0..=*edges).try_fold(0usize, |nodes, _| nodes.checked_mul(3).and_then(|n| n.checked_add(1)))
                .is_some_and(|nodes| nodes <= node_cap)
        })
        .last()
        .unwrap_or(0);
    candidate_edges.truncate(edge_cap_for_budget);
    let mut selected_degree = vec![0usize; rival_ids.len()];
    let mut games = Vec::with_capacity(candidate_edges.len());
    for (_, _, a, b) in &candidate_edges {
        selected_degree[*a] = selected_degree[*a].saturating_add(1);
        selected_degree[*b] = selected_degree[*b].saturating_add(1);
        games.push(rank_bounds::Game { a: *a, b: *b });
    }
    let rivals = rival_ids
        .iter()
        .enumerate()
        .map(|(slot, &team)| rank_bounds::Rival {
            points: points[team],
            wins: wins[team],
            outside_fixtures: degrees[slot].saturating_sub(selected_degree[slot]),
        })
        .collect::<Vec<_>>();
    if node_cap == 0 {
        return (None, base_work.max(1));
    }
    let (above, visited) = match rank_bounds::maximum_possible_above(
        &rivals,
        &games,
        target.0,
        target.1,
        node_cap,
    ) {
        Ok(result) => result,
        Err(_) => return (None, RIVAL_BOUND_WORK_CAP_PER_ROOT),
    };
    let work = base_work.saturating_add(visited.saturating_mul(m.n.saturating_mul(2).saturating_add(1)));
    if work > RIVAL_BOUND_WORK_CAP_PER_ROOT {
        return (None, RIVAL_BOUND_WORK_CAP_PER_ROOT);
    }
    (Some(above), work)
}

fn root_bound_filter(m: &Model, cell: Cell, root: &[u8], stats: &mut RootBoundStats) -> (bool, usize) {
    stats.examined = stats.examined.saturating_add(1);
    let (maximum, work) = root_maximum_possible_above(m, cell, root);
    stats.work = stats.work.saturating_add(work);
    if let Some(maximum) = maximum {
        stats.certified = stats.certified.saturating_add(1);
        if maximum < cell.rank {
            stats.pruned = stats.pruned.saturating_add(1);
            return (false, work);
        }
    } else {
        stats.unknown = stats.unknown.saturating_add(1);
    }
    (true, work)
}

fn selector_candidate_allowed(roots: usize, structural_recovery: bool) -> bool {
    roots <= ROOT_LIMIT && !(structural_recovery && roots == 0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProposalKind {
    Rank,
    Interval,
}
impl ProposalKind {
    fn bound_guidance(self) -> bool {
        matches!(self, Self::Interval)
    }
}

fn candidate(estimate: &Estimate) -> bool {
    if estimate.probability == 0. {
        return !estimate.reachability.starts_with("impossible");
    }
    if estimate.conditional_samples == 0 {
        return false;
    }
    let weak_rse = estimate_relative_se(estimate) > 0.25;
    let weak_share = estimate.max_event_weight_share > 0.1;
    estimate.ess < 25. || weak_rse || weak_share
}

fn estimate_relative_se(estimate: &Estimate) -> f64 {
    estimate.relative_se.unwrap_or_else(|| {
        if estimate.probability > 0. {
            estimate.std_err / estimate.probability
        } else {
            f64::INFINITY
        }
    })
}

fn interval_proposal_better(rank_ess: f64, interval_ess: f64) -> bool {
    interval_ess > rank_ess
}

fn allocation(scores: &[f64], total: usize, floor: usize) -> Option<Vec<usize>> {
    if scores.is_empty() || floor < 2 || scores.iter().any(|s| !s.is_finite() || *s < 0.) {
        return None;
    }
    let base = floor.checked_mul(scores.len())?;
    if base > total {
        return None;
    }
    let mut out = vec![floor; scores.len()];
    let remaining = total - base;
    let sum = scores.iter().sum::<f64>();
    for (i, score) in scores.iter().enumerate() {
        let share = if sum > 0. {
            0.1 / scores.len() as f64 + 0.9 * score / sum
        } else {
            1. / scores.len() as f64
        };
        out[i] += (remaining as f64 * share).floor() as usize;
    }
    let len = out.len();
    for i in 0..total - out.iter().sum::<usize>() {
        out[i % len] += 1;
    }
    Some(out)
}

fn allocation_from_shares(shares: &[f64], total: usize, floor: usize) -> Option<Vec<usize>> {
    if shares.is_empty()
        || floor < 2
        || shares
            .iter()
            .any(|share| !share.is_finite() || *share < 0.0)
    {
        return None;
    }
    let base = floor.checked_mul(shares.len())?;
    if base > total {
        return None;
    }
    let sum = shares.iter().sum::<f64>();
    if !sum.is_finite() || sum <= 0.0 {
        return None;
    }
    let remaining = total - base;
    let mut output = vec![floor; shares.len()];
    for (index, share) in shares.iter().enumerate() {
        output[index] =
            output[index].saturating_add((remaining as f64 * (*share / sum)).floor() as usize);
    }
    let residual = total.checked_sub(output.iter().sum::<usize>())?;
    let output_len = output.len();
    for index in 0..residual {
        output[index % output_len] += 1;
    }
    Some(output)
}

fn affordable_allocation(
    scores: &[f64],
    max_draws: usize,
    floor: usize,
    per_draw: &[usize],
    streams: usize,
    available: usize,
) -> Option<(Vec<usize>, usize)> {
    if per_draw.len() != scores.len() || streams == 0 {
        return None;
    }
    let mut low = floor.checked_mul(scores.len())?;
    let mut high = max_draws;
    let mut best = None;
    while low <= high {
        let mid = low + (high - low) / 2;
        if let Some(counts) = allocation(scores, mid, floor) {
            let forecast = counts
                .iter()
                .zip(per_draw)
                .map(|(n, cost)| n.saturating_mul(*cost))
                .sum::<usize>()
                .saturating_mul(streams);
            if forecast <= available {
                best = Some((counts, forecast));
                low = mid.saturating_add(1);
            } else if mid == 0 {
                break;
            } else {
                high = mid - 1;
            }
        } else {
            low = mid.saturating_add(1);
        }
    }
    best
}

fn affordable_allocation_measured(
    scores: &[f64],
    max_draws: usize,
    floor: usize,
    per_draw: &[usize],
    streams: usize,
    available: usize,
) -> (Option<(Vec<usize>, usize)>, usize) {
    if per_draw.len() != scores.len() || streams == 0 {
        return (None, 0);
    }
    let Some(mut low) = floor.checked_mul(scores.len()) else {
        return (None, 0);
    };
    let mut high = max_draws;
    let mut best = None;
    let mut iterations = 0usize;
    while low <= high {
        iterations = iterations.saturating_add(1);
        let mid = low + (high - low) / 2;
        if let Some(counts) = allocation(scores, mid, floor) {
            let forecast = counts
                .iter()
                .zip(per_draw)
                .map(|(n, cost)| n.saturating_mul(*cost))
                .sum::<usize>()
                .saturating_mul(streams);
            if forecast <= available {
                best = Some((counts, forecast));
                low = mid.saturating_add(1);
            } else if mid == 0 {
                break;
            } else {
                high = mid - 1;
            }
        } else {
            low = mid.saturating_add(1);
        }
    }
    let work = iterations.saturating_mul(scores.len().saturating_mul(6).saturating_add(8));
    (best, work)
}

fn affordable_allocation_from_shares(
    shares: &[f64],
    max_draws: usize,
    floor: usize,
    per_draw: &[usize],
    streams: usize,
    available: usize,
) -> (Option<(Vec<usize>, usize)>, usize) {
    if per_draw.len() != shares.len() || streams == 0 {
        return (None, 0);
    }
    let Some(mut low) = floor.checked_mul(shares.len()) else {
        return (None, 0);
    };
    let mut high = max_draws;
    let mut best = None;
    let mut iterations = 0usize;
    while low <= high {
        iterations = iterations.saturating_add(1);
        let mid = low + (high - low) / 2;
        if let Some(counts) = allocation_from_shares(shares, mid, floor) {
            let forecast = counts
                .iter()
                .zip(per_draw)
                .map(|(n, cost)| n.saturating_mul(*cost))
                .sum::<usize>()
                .saturating_mul(streams);
            if forecast <= available {
                best = Some((counts, forecast));
                low = mid.saturating_add(1);
            } else if mid == 0 {
                break;
            } else {
                high = mid - 1;
            }
        } else {
            low = mid.saturating_add(1);
        }
    }
    let work = iterations.saturating_mul(shares.len().saturating_mul(6).saturating_add(8));
    (best, work)
}

const HIERARCHICAL_SEARCH_STEPS: usize = 24;

fn normalize_weights(weights: &[f64]) -> Option<Vec<f64>> {
    if weights.is_empty() || weights.iter().any(|w| !w.is_finite() || *w < 0.0) {
        return None;
    }
    let max = weights.iter().copied().fold(0.0, f64::max);
    if max == 0.0 {
        return Some(vec![1.0 / weights.len() as f64; weights.len()]);
    }
    let scaled: Vec<_> = weights.iter().map(|w| *w / max).collect();
    let sum: f64 = scaled.iter().sum();
    if !sum.is_finite() || sum <= 0.0 {
        return None;
    }
    Some(scaled.into_iter().map(|w| w / sum).collect())
}

fn parent_uniform_final_shares(
    sigma: &[f64],
    parent_ids: &[usize],
    local_shares: &[f64],
) -> Option<Vec<f64>> {
    if sigma.is_empty()
        || parent_ids.len() != sigma.len()
        || local_shares.len() != sigma.len()
        || local_shares
            .iter()
            .any(|share| !share.is_finite() || *share <= 0.0)
    {
        return None;
    }
    let parents = parent_ids.iter().copied().collect::<BTreeSet<_>>();
    if parents.is_empty() {
        return None;
    }
    let max_sigma = sigma.iter().copied().fold(0.0, f64::max);
    let shares = if max_sigma == 0.0 {
        local_shares
            .iter()
            .map(|local| *local / parents.len() as f64)
            .collect::<Vec<_>>()
    } else {
        normalize_weights(sigma)?
            .iter()
            .zip(local_shares)
            .map(|(learned, local)| 0.9 * learned + 0.1 / parents.len() as f64 * local)
            .collect::<Vec<_>>()
    };
    let sum = shares.iter().sum::<f64>();
    (sum.is_finite() && sum > 0.0).then(|| shares.into_iter().map(|share| share / sum).collect())
}

/// Spread only the draws above the per-leaf floor according to the existing
/// root shares, then each root's pilot sigma shares with a within-root floor.
fn hierarchical_allocation(
    groups: &[(Vec<u8>, f64, Vec<usize>)],
    scores: &[f64],
    total: usize,
    floor: usize,
) -> Option<Vec<usize>> {
    if groups.is_empty()
        || scores.is_empty()
        || floor < 2
        || scores.iter().any(|s| !s.is_finite() || *s < 0.0)
        || floor.checked_mul(scores.len())? > total
    {
        return None;
    }
    let root_shares = normalize_weights(&groups.iter().map(|g| g.1).collect::<Vec<_>>())?;
    let mut owner = vec![false; scores.len()];
    let mut leaf_weights = vec![0.0; scores.len()];
    for (group_index, (_, _, leaves)) in groups.iter().enumerate() {
        if leaves.is_empty() {
            return None;
        }
        for &i in leaves {
            if i >= scores.len() || owner[i] {
                return None;
            }
            owner[i] = true;
        }
        let within = normalize_weights(&leaves.iter().map(|&i| scores[i]).collect::<Vec<_>>())?;
        for (&i, sigma_share) in leaves.iter().zip(within) {
            leaf_weights[i] =
                root_shares[group_index] * (0.9 * sigma_share + 0.1 / leaves.len() as f64);
        }
    }
    if owner.iter().any(|owned| !owned) {
        return None;
    }
    let excess = total - floor * scores.len();
    let mut counts = vec![floor; scores.len()];
    let mut remainders = Vec::with_capacity(scores.len());
    let mut used = 0usize;
    for (i, weight) in leaf_weights.iter().enumerate() {
        let exact = excess as f64 * weight;
        let whole = exact.floor() as usize;
        counts[i] = counts[i].checked_add(whole)?;
        used = used.checked_add(whole)?;
        remainders.push((i, exact - whole as f64));
    }
    remainders.sort_by(|(ia, ra), (ib, rb)| rb.total_cmp(ra).then_with(|| ia.cmp(ib)));
    let leftover = excess.checked_sub(used)?;
    if leftover > remainders.len() {
        return None;
    }
    for (i, _) in remainders.into_iter().take(leftover) {
        counts[i] += 1;
    }
    (counts.iter().sum::<usize>() == total).then_some(counts)
}

fn hierarchical_work_per_search(leaves: usize, roots: usize) -> usize {
    leaves
        .saturating_mul(32)
        .saturating_add(roots.saturating_mul(16))
        .saturating_add(64)
}

fn hierarchical_search_work_bound(leaves: usize, roots: usize) -> usize {
    hierarchical_work_per_search(leaves, roots).saturating_mul(HIERARCHICAL_SEARCH_STEPS)
}

fn forecast(counts: &[usize], per_draw: &[usize], streams: usize) -> Option<usize> {
    if counts.len() != per_draw.len() || streams == 0 {
        return None;
    }
    Some(
        counts
            .iter()
            .zip(per_draw)
            .map(|(n, cost)| n.saturating_mul(*cost))
            .sum::<usize>()
            .saturating_mul(streams),
    )
}

fn pilot_sigma(std_err: f64, samples: usize) -> f64 {
    std_err * (samples as f64).sqrt()
}

fn dual_sigma_scores(rank_sigma: &[f64], interval_sigma: &[f64]) -> Option<Vec<f64>> {
    if rank_sigma.len() != interval_sigma.len()
        || rank_sigma
            .iter()
            .chain(interval_sigma)
            .any(|sigma| !sigma.is_finite() || *sigma < 0.0)
    {
        return None;
    }
    Some(
        rank_sigma
            .iter()
            .zip(interval_sigma)
            .map(|(rank, interval)| rank.max(*interval))
            .collect(),
    )
}

fn pilot_guard_work_fee(leaves: usize, pilot_support: bool) -> usize {
    leaves
        .saturating_mul(if pilot_support { 12 } else { 8 })
        .saturating_add(16)
}

fn positive_fold(a: f64, b: f64) -> Option<f64> {
    if !a.is_finite() || !b.is_finite() || a <= 0.0 || b <= 0.0 {
        return None;
    }
    let fold = a.max(b) / a.min(b);
    fold.is_finite().then_some(fold)
}

fn finite_positive_ratio(numerator: f64, denominator: f64) -> Option<f64> {
    if !numerator.is_finite() || !denominator.is_finite() || numerator < 0.0 || denominator <= 0.0 {
        return None;
    }
    let ratio = numerator / denominator;
    ratio.is_finite().then_some(ratio)
}

fn telemetry_float(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

#[derive(Clone, Debug)]
struct RootExplorationPolicy {
    shares: Vec<f64>,
    learned_shares: Vec<f64>,
    exploration_shares: Vec<f64>,
    pilot_support_shares: Vec<f64>,
    dominant_index: usize,
    raw_share: f64,
    dominant_leaf_count: usize,
    other_leaf_count: usize,
    dominant_pilot_count: Option<usize>,
    pilot_support_applied: bool,
}

#[derive(Clone, Copy)]
struct PilotGuardRecord {
    rank_sigma: f64,
    interval_sigma: f64,
    chosen_sigma: f64,
    chosen_probability: f64,
    chosen_ess: f64,
    rank_probability: f64,
    interval_probability: f64,
    rank_ess: f64,
    interval_ess: f64,
}

fn root_exploration_policy_shares(
    groups: &[(Vec<u8>, f64, Vec<usize>)],
    sigma: &[f64],
    support_counts: Option<&[usize]>,
) -> Option<RootExplorationPolicy> {
    let parent_ids = (0..sigma.len()).collect::<Vec<_>>();
    let local_shares = vec![1.0; sigma.len()];
    root_exploration_policy_shares_parented(
        groups,
        sigma,
        support_counts,
        &parent_ids,
        &local_shares,
    )
}

fn root_exploration_policy_shares_parented(
    groups: &[(Vec<u8>, f64, Vec<usize>)],
    sigma: &[f64],
    support_counts: Option<&[usize]>,
    parent_ids: &[usize],
    local_shares: &[f64],
) -> Option<RootExplorationPolicy> {
    if groups.len() < 2
        || sigma.is_empty()
        || parent_ids.len() != sigma.len()
        || local_shares.len() != sigma.len()
        || local_shares
            .iter()
            .any(|share| !share.is_finite() || *share <= 0.0)
        || sigma.iter().any(|value| !value.is_finite() || *value < 0.0)
    {
        return None;
    }
    let mut owners = vec![false; sigma.len()];
    let mut root_scores = Vec::with_capacity(groups.len());
    let mut root_leaf_counts = Vec::with_capacity(groups.len());
    for (_, score, leaves) in groups {
        if !score.is_finite() || *score < 0.0 || leaves.is_empty() {
            return None;
        }
        for &leaf in leaves {
            if leaf >= owners.len() || std::mem::replace(&mut owners[leaf], true) {
                return None;
            }
        }
        root_scores.push(*score);
        root_leaf_counts.push(leaves.len());
    }
    if owners.iter().any(|owner| !owner) {
        return None;
    }
    let normalized_roots = normalize_weights(&root_scores)?;
    let dominant = (0..groups.len()).max_by(|&a, &b| {
        normalized_roots[a]
            .total_cmp(&normalized_roots[b])
            .then_with(|| b.cmp(&a))
    })?;
    let raw_share = normalized_roots[dominant];
    if raw_share < 0.90 {
        return None;
    }
    let other_leaves = sigma.len().saturating_sub(root_leaf_counts[dominant]);
    if root_leaf_counts[dominant] == 0 || other_leaves == 0 {
        return None;
    }
    if sigma.iter().copied().fold(0.0, f64::max) <= 0.0 {
        return None;
    }
    let normalized_sigma = normalize_weights(sigma)?;
    let dominant_parents = groups[dominant]
        .2
        .iter()
        .map(|&leaf| parent_ids[leaf])
        .collect::<BTreeSet<_>>();
    let all_parents = parent_ids.iter().copied().collect::<BTreeSet<_>>();
    let other_parent_count = all_parents.difference(&dominant_parents).count();
    if dominant_parents.is_empty() || other_parent_count == 0 {
        return None;
    }
    let mut shares = vec![0.0; sigma.len()];
    let mut learned_shares = vec![0.0; sigma.len()];
    let mut exploration_shares = vec![0.0; sigma.len()];
    let mut pilot_support_shares = vec![0.0; sigma.len()];
    let valid_support_counts = support_counts
        .filter(|counts| counts.len() == sigma.len() && counts.iter().all(|count| *count >= 5));
    let dominant_pilot_count =
        valid_support_counts.and_then(|counts| root_groups_count(groups, dominant, counts));
    let support_pilot_count = dominant_pilot_count.filter(|count| *count > 0);
    let use_support = support_pilot_count.is_some();
    for (root_index, (_, _, leaves)) in groups.iter().enumerate() {
        for &leaf in leaves {
            learned_shares[leaf] = 0.9 * normalized_sigma[leaf];
            if root_index == dominant {
                pilot_support_shares[leaf] = if let (Some(counts), Some(total)) =
                    (valid_support_counts, support_pilot_count)
                {
                    0.081 * counts[leaf] as f64 / total as f64
                } else {
                    0.0
                };
                let uniform = if use_support {
                    0.009 / dominant_parents.len() as f64 * local_shares[leaf]
                } else {
                    0.09 / dominant_parents.len() as f64 * local_shares[leaf]
                };
                exploration_shares[leaf] = pilot_support_shares[leaf] + uniform;
            } else {
                exploration_shares[leaf] = 0.01 / other_parent_count as f64 * local_shares[leaf];
            }
            shares[leaf] = learned_shares[leaf] + exploration_shares[leaf];
        }
    }
    Some(RootExplorationPolicy {
        shares,
        learned_shares,
        exploration_shares,
        pilot_support_shares,
        dominant_index: dominant,
        raw_share,
        dominant_leaf_count: root_leaf_counts[dominant],
        other_leaf_count: other_leaves,
        dominant_pilot_count: support_pilot_count,
        pilot_support_applied: use_support,
    })
}

fn root_groups_count(
    groups: &[(Vec<u8>, f64, Vec<usize>)],
    root_index: usize,
    counts: &[usize],
) -> Option<usize> {
    groups[root_index]
        .2
        .iter()
        .try_fold(0usize, |total, &leaf| total.checked_add(*counts.get(leaf)?))
}

fn root_exploration_allocation(shares: &[f64], total: usize, floor: usize) -> Option<Vec<usize>> {
    quota_counts(shares, total, floor)
}

fn affordable_root_exploration(
    shares: &[f64],
    max_draws: usize,
    floor: usize,
    per_draw: &[usize],
    streams: usize,
    available: usize,
) -> (Option<(Vec<usize>, usize)>, usize) {
    if shares.len() != per_draw.len() || streams == 0 {
        return (None, 0);
    }
    let Some(mut low) = floor.checked_mul(shares.len()) else {
        return (None, 0);
    };
    let mut high = max_draws;
    let mut best = None;
    let mut steps = 0usize;
    while low <= high && steps < HIERARCHICAL_SEARCH_STEPS {
        let mid = low + (high - low) / 2;
        steps += 1;
        let Some(counts) = root_exploration_allocation(shares, mid, floor) else {
            return (
                None,
                steps.saturating_mul(hierarchical_work_per_search(shares.len(), 2)),
            );
        };
        let Some(cost) = forecast(&counts, per_draw, streams) else {
            return (
                None,
                steps.saturating_mul(hierarchical_work_per_search(shares.len(), 2)),
            );
        };
        if cost <= available {
            best = Some((counts, cost));
            low = mid.saturating_add(1);
        } else if mid == 0 {
            break;
        } else {
            high = mid - 1;
        }
    }
    (
        best,
        steps.saturating_mul(hierarchical_work_per_search(shares.len(), 2)),
    )
}

fn affordable_hierarchical_allocation(
    groups: &[(Vec<u8>, f64, Vec<usize>)],
    scores: &[f64],
    max_draws: usize,
    floor: usize,
    per_draw: &[usize],
    streams: usize,
    available: usize,
) -> (Option<(Vec<usize>, usize)>, usize) {
    let Some(mut low) = floor.checked_mul(scores.len()) else {
        return (None, 0);
    };
    let mut high = max_draws;
    if low > high || per_draw.len() != scores.len() || streams == 0 {
        return (None, 0);
    }
    let mut best = None;
    let mut steps = 0usize;
    while low <= high && steps < HIERARCHICAL_SEARCH_STEPS {
        let mid = low + (high - low) / 2;
        steps += 1;
        if let Some(counts) = hierarchical_allocation(groups, scores, mid, floor) {
            let Some(cost) = forecast(&counts, per_draw, streams) else {
                return (
                    None,
                    steps.saturating_mul(hierarchical_work_per_search(scores.len(), groups.len())),
                );
            };
            if cost <= available {
                best = Some((counts, cost));
                low = mid.saturating_add(1);
            } else if mid == 0 {
                break;
            } else {
                high = mid - 1;
            }
        } else {
            return (
                None,
                steps.saturating_mul(hierarchical_work_per_search(scores.len(), groups.len())),
            );
        }
    }
    (
        best,
        steps.saturating_mul(hierarchical_work_per_search(scores.len(), groups.len())),
    )
}

fn rough_publishable(r: &Result) -> bool {
    r.weighted
        && r.probability > 0.
        && r.hits >= 30
        && r.ess >= 4.
        && r.std_err / r.probability <= 0.6
        && r.max_share <= 0.35
        && r.batch_gap <= 1.5
}

fn rse(r: &Result) -> f64 {
    if r.probability > 0. {
        r.std_err / r.probability
    } else {
        f64::INFINITY
    }
}

fn replacement_better(old: &Estimate, main: &Result, check: &Result) -> bool {
    rough_publishable(main)
        && rough_publishable(check)
        && accepted(main, check, true)
        && main.ess.min(check.ess) > old.ess
        && rse(main).max(rse(check)) < estimate_relative_se(old)
}

fn accepted_pair_for(old: &Estimate, main: &Result, check: &Result, positive: bool) -> bool {
    if positive { replacement_better(old, main, check) } else { accepted(main, check, true) }
}

fn replacement_or_new_accepts(old: &Estimate, main: &Result, check: &Result, positive: bool) -> bool {
    accepted_pair_for(old, main, check, positive)
}

fn commit_candidate(
    est: &mut Estimate,
    old: &Estimate,
    main: &Result,
    check: &Result,
    positive: bool,
) -> bool {
    let accepted = if positive {
        replacement_better(old, main, check)
    } else {
        accepted(main, check, true)
    };
    if accepted {
        crate::search::apply(
            est,
            main,
            if positive {
                "matched_point_pool_rare_tail_rank_rescue_upgrade"
            } else {
                "matched_point_pool_rare_tail_rank_rescue"
            },
        );
    }
    accepted
}

fn sample_stage(
    proposal: &BranchStrata,
    m: &Model,
    counts: &[usize],
    seed: i64,
    label: &str,
    proposal_kinds: &[ProposalKind],
    workers: usize,
    stable_keys: Option<&[String]>,
    skip_zero: bool,
) -> Vec<BranchSample> {
    let costs = counts
        .iter()
        .enumerate()
        .map(|(i, &n)| {
            n.saturating_mul(proposal.draw_work(m, i, proposal_kinds[i].bound_guidance()))
        })
        .collect::<Vec<_>>();
    let rows = budget::parallel(&costs, workers, |i| {
        let n = counts[i];
        if skip_zero && n == 0 {
            return BranchSample::default();
        }
        let key = stable_keys
            .and_then(|keys| keys.get(i))
            .map_or_else(|| format!("leaf-{i}"), |key| key.clone());
        proposal.sample(
            m,
            i,
            n,
            derive(seed, &format!("rank-rescue-{label}-{key}")),
            proposal_kinds[i].bound_guidance(),
            true,
        )
    });
    rows
}

fn charge(samples: &[BranchSample]) -> usize {
    samples.iter().map(|s| s.result.operations.units()).sum()
}

fn combine_group(samples: &[BranchSample], leaves: &[usize]) -> Result {
    let grouped = leaves
        .iter()
        .filter_map(|&i| samples.get(i))
        .map(|sample| BranchSample {
            result: sample.result.clone(),
            left: sample.left,
            right: sample.right,
        })
        .collect::<Vec<_>>();
    combine_branches(&grouped)
}

fn reserve(bank: &mut budget::WorkBudget, grant: usize) -> bool {
    grant > 0 && bank.reserve(grant)
}

pub(crate) fn run(
    m: &Model,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    log: Option<&crate::logging::RequestLog>,
    settings: Settings,
) -> (usize, u64) {
    let started = Instant::now();
    if let Some(reason) = settings.target_filter_error {
        emit(
            log,
            "rust_odds_rare_tail_rank_rescue_summary",
            json!({
                "enabled":true,"status":"skipped","reason":"invalid target filter","target_reason":reason,
                "eligible":0,"attempted":0,"found":0,"failed":0,"skipped":0,"work_limit":0,
                "elapsed_ms":started.elapsed().as_secs_f64()*1000.0
            }),
        );
        return (0, 0);
    }
    let target_index = match target_cell(settings.target_filter, &m.ids, m.n) {
        Ok(index) => index,
        Err(reason) => {
            emit(
                log,
                "rust_odds_rare_tail_rank_rescue_summary",
                json!({
                    "enabled":true,"status":"skipped","reason":"target filter outside model","target_reason":reason,
                    "target":settings.target_filter,"eligible":0,"attempted":0,"found":0,"failed":0,"skipped":0,
                    "work_limit":0,"elapsed_ms":started.elapsed().as_secs_f64()*1000.0
                }),
            );
            return (0, 0);
        }
    };
    let eligible = estimates
        .iter()
        .enumerate()
        .filter(|(index, e)| candidate(e) && target_index.map_or(true, |target| *index == target))
        .count();
    let mut added = 0usize;
    let mut root_probe_count = 0usize;
    let mut tree_attempted = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut upgrades = 0usize;
    let preserved_positives = estimates.iter().filter(|e| e.probability > 0.).count();
    let mut setup_actual = 0usize;
    let mut pilot_actual = 0usize;
    let mut final_actual = 0usize;
    let mut allocation_actual_total = 0usize;
    let mut cumulative_reserved = 0usize;
    let mut reserved_peak = 0usize;
    let mut rows = Vec::new();

    if !settings.enabled {
        emit(
            log,
            "rust_odds_rare_tail_rank_rescue_summary",
            json!({
                "enabled":false,"eligible":eligible,"attempted":0,"found":0,"new":0,"upgraded":0,
                "failed":0,"skipped":0,"preserved_positives":preserved_positives,
                "setup_actual":0,"pilot_actual":0,"final_actual":0,"reserved_work":0,
                "work_limit":0,"elapsed_ms":started.elapsed().as_secs_f64()*1000.0
            }),
        );
        return (0, 0);
    }
    if !budget::modeled() {
        emit(
            log,
            "rust_odds_rare_tail_rank_rescue_summary",
            json!({
                "enabled":true,"status":"skipped","reason":"modeled work budget required",
                "eligible":eligible,"attempted":0,"found":0,"new":0,"upgraded":0,
                "failed":0,"skipped":eligible,"preserved_positives":preserved_positives,
                "setup_actual":0,"pilot_actual":0,"final_actual":0,"reserved_work":0,
                "work_limit":0,"elapsed_ms":started.elapsed().as_secs_f64()*1000.0
            }),
        );
        return (0, 0);
    }

    let limit = budget::capacity(m, settings.reference_draws);
    let mut bank = budget::WorkBudget::new(limit);
    let mut candidates = Vec::new();
    for (index, estimate) in estimates.iter().enumerate() {
        if !candidate(estimate) || target_index.is_some_and(|target| target != index) {
            continue;
        }
        let cell = Cell {
            team: index / m.n,
            rank: index % m.n,
        };
        let grant = bank.limit.saturating_sub(bank.reserved);
        if grant < ROOT_PROBE_ADMISSION_FLOOR {
            skipped += estimates[index..].iter().filter(|e| candidate(e)).count();
            rows.push(json!({"team":m.ids[cell.team],"rank":cell.rank+1,"stage":"root_count","status":"skipped","reason":"remaining bank below root probe admission floor","available":grant,"admission_floor":ROOT_PROBE_ADMISSION_FLOOR}));
            break;
        }
        if !reserve(&mut bank, grant) {
            skipped += 1;
            continue;
        }
        root_probe_count += 1;
        cumulative_reserved = cumulative_reserved.saturating_add(grant);
        reserved_peak = reserved_peak.max(bank.reserved);
        let root_seed = derive(
            seed,
            &format!("rank-rescue-root-{}-{}", m.ids[cell.team], cell.rank),
        );
        let mut root_bound_stats = RootBoundStats::default();
        let (result, actual) = if settings.structural_recovery {
            let mut filter = |root: &[u8]| root_bound_filter(m, cell, root, &mut root_bound_stats);
            BranchStrata::overflow_root_count_filtered(
                m,
                cell,
                root_seed,
                ROOT_LIMIT,
                &mut filter,
            )
        } else {
            BranchStrata::overflow_root_count(m, cell, root_seed, ROOT_LIMIT)
        };
        setup_actual = setup_actual.saturating_add(actual);
        if actual > grant {
            failed += 1;
            rows.push(json!({"team":m.ids[cell.team],"rank":cell.rank+1,"stage":"root_count","roots":result.as_ref().ok(),"actual":actual,"grant":grant,"overrun":true,"actual_overrun_units":actual.saturating_sub(grant),"reason":"work overrun; stage stopped"}));
            break;
        }
        assert!(bank.release(grant - actual));
        match result {
            Ok(roots) => {
                rows.push(json!({"team":m.ids[cell.team],"rank":cell.rank+1,"stage":"root_count","roots":roots,"actual":actual,"grant":grant,
                    "structural_recovery":settings.structural_recovery,"bound_roots_examined":root_bound_stats.examined,
                    "bound_roots_certified":root_bound_stats.certified,"bound_roots_pruned":root_bound_stats.pruned,
                    "bound_unknown_roots":root_bound_stats.unknown,"bound_work":root_bound_stats.work}));
                if selector_candidate_allowed(roots, settings.structural_recovery) {
                    candidates.push(Candidate {
                        cell,
                        roots,
                        root_work: actual,
                        bound_pruned: root_bound_stats.pruned,
                        bound_work: root_bound_stats.work,
                        was_positive: estimate.probability > 0.,
                    });
                } else {
                    skipped += 1;
                    if settings.structural_recovery && roots == 0 {
                        rows.push(json!({"team":m.ids[cell.team],"rank":cell.rank+1,"stage":"root_count_selector","roots":0,"reason":"all roots certified impossible; excluded from attempt ordering"}));
                    }
                }
            }
            Err(error) => {
                failed += 1;
                rows.push(json!({"team":m.ids[cell.team],"rank":cell.rank+1,"stage":"root_count","actual":actual,"grant":grant,"error":error}));
            }
        }
    }
    candidates.sort_by_key(|c| (c.roots, c.cell.index(m.n)));
    let ranked_candidates = candidates.iter().map(|c|json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"root_count_work":c.root_work,"bound_roots_pruned":c.bound_pruned,"bound_work":c.bound_work,"positive":c.was_positive})).collect::<Vec<_>>();
    skipped = skipped.saturating_add(candidates.len().saturating_sub(settings.max_attempts));
    candidates.truncate(settings.max_attempts);

    for c in candidates {
        let index = c.cell.index(m.n);
        let old = estimates[index].clone();
        let free = bank.limit.saturating_sub(bank.reserved);
        let path_priority_floor = if settings.path_priority {
            BranchStrata::rank_rescue_path_priority_limit()
        } else {
            0
        };
        let leaf_pilot_floor = if settings.leaf_pilot {
            BranchStrata::rank_rescue_leaf_pilot_limit()
        } else {
            0
        };
        let pair_pilot_floor = if settings.pair_pilot {
            BranchStrata::rank_rescue_pair_pressure_limit()
        } else {
            0
        };
        let setup_admission_floor = TREE_SETUP_ADMISSION_FLOOR
            .saturating_add(path_priority_floor)
            .saturating_add(leaf_pilot_floor)
            .saturating_add(pair_pilot_floor)
            .saturating_add(
                if settings.bounded_omission && omission_old_probability_valid(old.probability) {
                    settings.bounded_omission_work
                } else {
                    0
                },
            )
            .saturating_add(if settings.goal_tiebreak {
                if settings.shared_goal_tiebreak {
                    settings.shared_goal_tiebreak_work_limit
                } else {
                    BranchStrata::rank_rescue_goal_tiebreak_limit()
                }
            } else {
                0
            })
            .saturating_add(if settings.na_guidance {
                settings.na_guidance_work
            } else {
                0
            });
        let setup_admission_floor = setup_admission_floor
            .saturating_add(settings.hybrid_tree_work);
        if free < setup_admission_floor {
            skipped += 1;
            rows.push(json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"stage":"setup","status":"skipped","reason":"remaining bank below tree setup admission floor","available":free,"admission_floor":setup_admission_floor}));
            continue;
        }
        if free == 0 {
            skipped += 1;
            continue;
        }
        if !reserve(&mut bank, free) {
            skipped += 1;
            continue;
        }
        cumulative_reserved = cumulative_reserved.saturating_add(free);
        reserved_peak = reserved_peak.max(bank.reserved);
        tree_attempted += 1;
        let tree_seed = derive(
            seed,
            &format!("rank-rescue-tree-{}-{}", m.ids[c.cell.team], c.cell.rank),
        );
        let mut tree_bound_stats = RootBoundStats::default();
        let (proposal_result, mut setup_work) = if settings.structural_recovery {
            let mut filter = |root: &[u8]| {
                root_bound_filter(m, c.cell, root, &mut tree_bound_stats)
            };
            BranchStrata::with_rank_rescue_tree_shared_goal_tiebreak_work_limit_recovery(
                m,
                c.cell,
                tree_seed,
                settings.path_priority,
                settings.leaf_pilot,
                settings.pair_pilot,
                settings.hierarchical_pair_pilot,
                settings.rival_path,
                settings.pilot_per_leaf,
                settings.goal_tiebreak,
                settings.shared_goal_tiebreak,
                settings.shared_goal_tiebreak_work_limit,
                true,
                &mut filter,
            )
        } else {
            BranchStrata::with_rank_rescue_tree_shared_goal_tiebreak_work_limit(
                m,
                c.cell,
                tree_seed,
                settings.path_priority,
                settings.leaf_pilot,
                settings.pair_pilot,
                settings.hierarchical_pair_pilot,
                settings.rival_path,
                settings.pilot_per_leaf,
                settings.goal_tiebreak,
                settings.shared_goal_tiebreak,
                settings.shared_goal_tiebreak_work_limit,
            )
        };
        setup_actual = setup_actual.saturating_add(setup_work);
        if setup_work > free {
            failed += 1;
            rows.push(json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"stage":"setup","actual":setup_work,"grant":free,"overrun":true,"actual_overrun_units":setup_work.saturating_sub(free),"reason":"work overrun; stage stopped","bound_roots_pruned":tree_bound_stats.pruned,"bound_work":tree_bound_stats.work}));
            break;
        }
        assert!(bank.release(free - setup_work));
        let proposal = match proposal_result {
            Ok(p) if !p.is_empty() => p,
            Ok(_) => {
                skipped += 1;
                continue;
            }
            Err(error) => {
                failed += 1;
                rows.push(json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"stage":"setup","error":error}));
                continue;
            }
        };
        let shortlist_requested = std::env::var(crate::shortlist_guidance::MODE_ENV)
            .ok().filter(|mode| !mode.is_empty());
        let mut shortlist_mode = "off".to_string();
        let mut shortlist_reason: Option<String> = None;
        let mut shortlist_guide = None;
        let mut shortlist_compatible_counts = Vec::new();
        let mut shortlist_all_loss_leaf_ids = Vec::new();
        let mut shortlist_comparison_work = 0usize;
        let mut shortlist_unmatched_weight = 0.0;
        let mut shortlist_scan_work = 0usize;
        let mut shortlist_pilot_applied = false;
        let mut shortlist_pilot_requested_counts: Option<Vec<usize>> = None;
        let mut shortlist_pilot_fallback: Option<String> = None;
        let mut shortlist_learning_fallback: Option<String> = None;
        let mut shortlist_learned_shares_applied = false;
        let target_shortlist_cell = m.ids.get(c.cell.team) == Some(&16) && c.cell.rank == 13;
        if target_shortlist_cell && shortlist_requested.is_some() {
            shortlist_mode = shortlist_requested.clone().unwrap();
            let work_cap = 4550usize.saturating_mul(128)
                .saturating_add(64usize.saturating_mul(proposal.len()).saturating_mul(m.n))
                .saturating_add(64usize.saturating_mul(128));
            if reserve(&mut bank, work_cap) {
                cumulative_reserved = cumulative_reserved.saturating_add(work_cap);
                reserved_peak = reserved_peak.max(bank.reserved);
                // The prepaid bound covers the full frozen-row scan, parsing,
                // leaf compatibility comparisons, and normalization. Charge it
                // on parse and compatibility fallbacks as well as success.
                shortlist_scan_work = work_cap;
                setup_work = setup_work.saturating_add(work_cap);
                setup_actual = setup_actual.saturating_add(work_cap);
                let loaded = crate::shortlist_guidance::load(m);
                shortlist_reason = loaded.fallback.clone();
                if loaded.fallback.is_none() {
                    if let Some(mapped) = proposal.shortlist_guidance_shares(m, &loaded.candidates) {
                        shortlist_compatible_counts = mapped.compatible_counts;
                        shortlist_all_loss_leaf_ids = mapped.all_loss_leaf_ids;
                        shortlist_comparison_work = mapped.work_units;
                        shortlist_unmatched_weight = mapped.unmatched_weight;
                        shortlist_guide = Some(mapped.shares);
                        shortlist_reason = None;
                    } else {
                        shortlist_reason = Some("no_compatible_all_loss_regions".into());
                    }
                }
            } else {
                shortlist_reason = Some("mapping_work_budget_unavailable".into());
            }
        }
        if settings.path_priority {
            let diagnostics = proposal.setup_diagnostics();
            emit(
                log,
                "rust_odds_rank_rescue_path_priority",
                json!({
                    "team":m.ids[c.cell.team],
                    "rank":c.cell.rank+1,
                    "elapsed_ms":diagnostics["tree"]["path_priority"]["elapsed_ms"],
                    "work_units":diagnostics["tree"]["path_priority_work_units"],
                    "setup_diagnostics":diagnostics,
                }),
            );
        }
        let leaves = proposal.len();
        let setup_diagnostics = proposal.setup_diagnostics();
        let structural_proxy = if settings.structural_recovery {
            structural_proxy_shares(&setup_diagnostics, leaves)
        } else {
            None
        };
        let structural_proxy_reason = if settings.structural_recovery && structural_proxy.is_none() {
            Some("structural proxy missing or invalid; uniform pilot fallback")
        } else {
            None
        };
        let structural_root_groups = if settings.structural_recovery {
            proposal.leaf_pilot_root_groups()
        } else {
            None
        };
        let shortlist_static_shares = shortlist_guide.as_ref().map(|guide| {
            guide.iter().map(|share| 0.95 * share + 0.05 / leaves.max(1) as f64).collect::<Vec<_>>()
        });
        let mut omitted = vec![false; leaves];
        let mut omission_bound_values = vec![0.0; leaves];
        let mut omission_ledger = 0.0;
        let mut omission_allowance = 0.0;
        let mut omission_work = 0usize;
        let mut omission_fallback_reason: Option<&'static str> = None;
        let mut omission_fallback_leaves = Vec::new();
        let mut omission_applied = false;
        let mut omission_bounds_available = false;
        let mut omission_prefix = Vec::new();
        if settings.bounded_omission {
            if !omission_old_probability_valid(old.probability) {
                omission_fallback_reason =
                    Some("old public probability is not finite and positive");
            } else if reserve(&mut bank, settings.bounded_omission_work) {
                cumulative_reserved =
                    cumulative_reserved.saturating_add(settings.bounded_omission_work);
                reserved_peak = reserved_peak.max(bank.reserved);
                let selection_work = omission_selection_work(leaves);
                if selection_work >= settings.bounded_omission_work {
                    omission_fallback_reason = Some("omission ledger work exceeds bound cap");
                } else {
                    let (bounds, work, fallbacks, reason) = proposal.bounded_omission_bounds(
                        m,
                        settings.bounded_omission_work - selection_work,
                    );
                    omission_work = work;
                    omission_fallback_reason = reason;
                    omission_fallback_leaves = fallbacks;
                    if let Some(bounds) = bounds {
                        omission_work = omission_work.saturating_add(selection_work);
                        omission_bounds_available = true;
                        omission_bound_values =
                            bounds.iter().map(|bound| bound.upper_bound).collect();
                        (omitted, omission_ledger, omission_allowance) =
                            freeze_omissions(old.probability, &omission_bound_values);
                        omission_applied = omitted.iter().any(|is_omitted| *is_omitted);
                        omission_prefix = omission_prefix_curve(&omission_bound_values, &omitted);
                    } else {
                        omission_bound_values.fill(1.0);
                    }
                }
                setup_actual = setup_actual.saturating_add(omission_work);
                setup_work = setup_work.saturating_add(omission_work);
                assert!(bank.release(settings.bounded_omission_work.saturating_sub(omission_work)));
            } else {
                omission_fallback_reason = Some("bound work reservation unavailable");
            }
        }
        if settings.bounded_omission {
            emit(
                log,
                "rust_odds_rank_rescue_bounded_omission",
                json!({
                    "team":m.ids[c.cell.team],"rank":c.cell.rank+1,
                    "enabled":true,"applied":omission_applied,
                    "bound_method":"false bound + read-2 only; original proposal leaves",
                    "bound_work_units":omission_work,"bound_work_cap":settings.bounded_omission_work,
                    "work_units_definition":"charged upper-bound scan/assembly iterations, DP state/storage units, transition evaluations, and omission sort/ledger allowance",
                    "bounds_available":omission_bounds_available,
                    "fallback_reason":omission_fallback_reason,
                    "fallback_leaf_ids":omission_fallback_leaves,
                    "old_probability":old.probability,"omission_allowance":omission_allowance,
                    "omitted_bound_ledger":omission_ledger,
                    "omission_prefix_ledger":omission_prefix.iter().map(|(leaf,upper)|json!({"leaf_id":leaf,"cumulative_upper_bound":upper})).collect::<Vec<_>>(),
                    "leaves":omission_bound_values.iter().enumerate().map(|(index,bound)|json!({"leaf_id":index,"upper_bound":if omission_bounds_available{Some(*bound)}else{None},"omitted":omitted[index]})).collect::<Vec<_>>(),
                }),
            );
        }
        let metadata_work = leaves.saturating_mul(64).saturating_add(128);
        let metadata_paid = settings.rival_path && reserve(&mut bank, metadata_work);
        if metadata_paid {
            cumulative_reserved = cumulative_reserved.saturating_add(metadata_work);
            reserved_peak = reserved_peak.max(bank.reserved);
            allocation_actual_total = allocation_actual_total.saturating_add(metadata_work);
        }
        let sampling_keys = if metadata_paid {
            proposal.rival_path_sampling_keys(true)
        } else {
            None
        };
        let final_sampling_keys = if metadata_paid {
            proposal.rival_path_sampling_keys(false)
        } else {
            None
        };
        let rival_partition_applied =
            settings.rival_path && metadata_paid && proposal.rival_path_partition_applied();
        let rival_cached_pilot_counts = if rival_partition_applied {
            proposal.rival_path_base_pilot_counts()
        } else {
            None
        };
        let mut pilot_ns = rival_cached_pilot_counts
            .clone()
            .unwrap_or_else(|| vec![settings.pilot_per_leaf; leaves]);
        let mut leaf_pilot_reason = None;
        let mut leaf_pilot_groups = None;
        let mut hierarchical_baseline: Option<HierarchicalPilotPlan> = None;
        let mut hierarchical_plan: Option<HierarchicalPilotPlan> = None;
        let mut hierarchical_applied = false;
        let mut hierarchical_reason: Option<&'static str> = None;
        let mut hierarchical_work = 0usize;
        let mut hierarchical_uniform_fallback = false;
        if settings.leaf_pilot {
            let tree_work = proposal.leaf_pilot_work_units().unwrap_or(0);
            // Conservative accounting for root-key lookup, JSON share decode,
            // grouping and deterministic within-root quota sorting.
            if let Some(plan_work) = leaf_plan_work(tree_work, leaves, c.roots, m.fixtures.len()) {
                if reserve(&mut bank, plan_work) {
                    setup_work = setup_work.saturating_add(plan_work);
                    cumulative_reserved = cumulative_reserved.saturating_add(plan_work);
                    reserved_peak = reserved_peak.max(bank.reserved);
                    setup_actual = setup_actual.saturating_add(plan_work);
                    if settings.hierarchical_pair_pilot {
                        leaf_pilot_groups = proposal.leaf_pilot_groups();
                        let rival_frozen = if settings.rival_path {
                            proposal.rival_path_pilot_budgets()
                        } else {
                            None
                        };
                        let root_groups = proposal.leaf_pilot_root_groups();
                        let independent = if settings.structural_recovery {
                            Some(structural_proxy.clone().unwrap_or_else(|| {
                                vec![1.0 / leaves.max(1) as f64; leaves]
                            }))
                        } else {
                            proposal.leaf_pilot_independent_shares()
                        };
                        let cached_pair_data = proposal.hierarchical_pair_pilot_data();
                        if let (Some(root_groups), Some(independent)) = (root_groups, independent) {
                            let no_scores = vec![None; leaves];
                            let no_selected = vec![Vec::new(); leaves];
                            let frozen_selected = cached_pair_data
                                .as_ref()
                                .map(|(_, selected)| selected.clone())
                                .unwrap_or_else(|| no_selected.clone());
                            hierarchical_baseline =
                                if let Some((true, budgets)) = rival_frozen.as_ref() {
                                    hierarchical_pilot_plan_frozen(
                                        &root_groups,
                                        &independent,
                                        &no_scores,
                                        &frozen_selected,
                                        budgets,
                                    )
                                } else {
                                    hierarchical_pilot_plan(
                                        &root_groups,
                                        &independent,
                                        &no_scores,
                                        &no_selected,
                                        settings.pilot_per_leaf,
                                    )
                                };
                            if let Some(baseline) = hierarchical_baseline.as_ref() {
                                pilot_ns.clone_from(&baseline.counts);
                            } else {
                                leaf_pilot_reason =
                                    Some("independent pilot shares unavailable; uniform fallback");
                            }
                            if hierarchical_baseline.is_some() {
                                if let Some(extra) = hierarchical_leaf_plan_extra_work(
                                    tree_work,
                                    leaves,
                                    c.roots,
                                    m.fixtures.len(),
                                    m.n,
                                ) {
                                    if reserve(&mut bank, extra) {
                                        setup_work = setup_work.saturating_add(extra);
                                        cumulative_reserved =
                                            cumulative_reserved.saturating_add(extra);
                                        reserved_peak = reserved_peak.max(bank.reserved);
                                        setup_actual = setup_actual.saturating_add(extra);
                                        hierarchical_work = extra;
                                        if let Some((logs, selected)) = cached_pair_data {
                                            hierarchical_plan = if let Some((true, budgets)) =
                                                rival_frozen.as_ref()
                                            {
                                                hierarchical_pilot_plan_frozen(
                                                    &root_groups,
                                                    &independent,
                                                    &logs,
                                                    &selected,
                                                    budgets,
                                                )
                                            } else {
                                                hierarchical_pilot_plan(
                                                    &root_groups,
                                                    &independent,
                                                    &logs,
                                                    &selected,
                                                    settings.pilot_per_leaf,
                                                )
                                            };
                                            if let Some(plan) = hierarchical_plan.as_ref() {
                                                pilot_ns.clone_from(&plan.counts);
                                                hierarchical_applied = true;
                                            } else {
                                                hierarchical_reason =
                                                    Some("invalid hierarchical pair pilot data; original quotas retained");
                                            }
                                        } else {
                                            hierarchical_reason = Some(
                                                "pair correction unavailable; original quotas retained",
                                            );
                                        }
                                    } else {
                                        hierarchical_reason =
                                            Some("hierarchical planning bank exhausted; original quotas retained");
                                    }
                                } else {
                                    hierarchical_reason =
                                        Some("hierarchical planning cap exceeded; original quotas retained");
                                }
                            }
                        } else {
                            leaf_pilot_reason =
                                Some("independent pilot metadata unavailable; uniform fallback");
                        }
                    } else {
                        leaf_pilot_groups = proposal.leaf_pilot_groups();
                    }
                } else {
                    leaf_pilot_reason =
                        Some("leaf pilot planning bank exhausted; uniform pilot fallback");
                }
            } else {
                leaf_pilot_reason =
                    Some("leaf pilot planning cap exceeded; uniform pilot fallback");
            }
            if !settings.hierarchical_pair_pilot {
                if let Some(groups) = leaf_pilot_groups.as_ref() {
                    for (_, indexes, weights) in groups {
                        let total = indexes.len().saturating_mul(settings.pilot_per_leaf);
                        let Some(shares) = pilot_shares(weights) else {
                            leaf_pilot_reason =
                                Some("invalid structural scores; uniform pilot fallback");
                            break;
                        };
                        let Some(counts) = quota_counts(&shares, total, 5) else {
                            leaf_pilot_reason =
                                Some("invalid structural shares; uniform pilot fallback");
                            break;
                        };
                        for (&index, count) in indexes.iter().zip(counts) {
                            pilot_ns[index] = count;
                        }
                    }
                } else if leaf_pilot_reason.is_none() {
                    leaf_pilot_reason =
                        Some("structural shares unavailable; uniform pilot fallback");
                }
                if leaf_pilot_reason.is_some() {
                    pilot_ns.fill(settings.pilot_per_leaf);
                }
            }
            if settings.hierarchical_pair_pilot && leaf_pilot_reason.is_some() {
                hierarchical_uniform_fallback = true;
            }
            if shortlist_mode == "pilot" {
                if let Some(shares) = shortlist_static_shares.as_ref() {
                    let total = pilot_ns.iter().sum::<usize>();
                    if omitted.iter().any(|is_omitted| *is_omitted) {
                        shortlist_pilot_fallback = Some("omitted_leaf_set_not_supported".into());
                    } else if let Some(counts) = quota_counts(shares, total, 5) {
                        pilot_ns = counts;
                        shortlist_pilot_requested_counts = Some(pilot_ns.clone());
                        if settings.hierarchical_pair_pilot {
                            hierarchical_applied = false;
                            hierarchical_reason = Some("shortlist pilot quotas superseded hierarchical plan");
                        }
                    } else {
                        shortlist_pilot_fallback = Some("guided_pilot_quota_invalid_or_floor_unaffordable".into());
                    }
                }
            }
            let guided_forecast = forecast_pilot_counts(&proposal, m, &pilot_ns);
            let uniform_forecast = if settings.rival_path {
                rival_cached_pilot_counts
                    .as_ref()
                    .map(|counts| forecast_pilot_counts(&proposal, m, counts))
                    .unwrap_or(usize::MAX)
            } else {
                (0..leaves)
                    .map(|i| {
                        settings.pilot_per_leaf.saturating_mul(
                            proposal
                                .draw_work(m, i, true)
                                .saturating_add(proposal.draw_work(m, i, false)),
                        )
                    })
                    .sum::<usize>()
            };
            let available = bank.limit.saturating_sub(bank.reserved);
            if settings.hierarchical_pair_pilot
                && guided_forecast > available
                && hierarchical_baseline.as_ref().is_some_and(|baseline| {
                    forecast_pilot_counts(&proposal, m, &baseline.counts) <= available
                })
            {
                pilot_ns.clone_from(&hierarchical_baseline.as_ref().unwrap().counts);
                hierarchical_applied = false;
                hierarchical_reason =
                    Some("hierarchical forecast exceeds bank; original quotas restored");
            }
            let selected_forecast = forecast_pilot_counts(&proposal, m, &pilot_ns);
            if selected_forecast > available && uniform_forecast <= available {
                if let Some(counts) = rival_cached_pilot_counts
                    .as_ref()
                    .filter(|_| settings.rival_path)
                {
                    pilot_ns.clone_from(counts);
                } else {
                    pilot_ns.fill(settings.pilot_per_leaf);
                }
                if settings.hierarchical_pair_pilot {
                    hierarchical_applied = false;
                    hierarchical_uniform_fallback = true;
                    hierarchical_reason =
                        Some("original and hierarchical forecasts exceed bank; uniform fallback");
                } else {
                    leaf_pilot_reason =
                        Some("guided pilot forecast exceeds bank; uniform pilot fallback");
                }
            }
            // Uniform or cached fallback restores only retained pilot quotas;
            // omitted slots remain zero and keep their original indexes.
            for (index, is_omitted) in omitted.iter().enumerate() {
                if *is_omitted {
                    pilot_ns[index] = 0;
                }
            }
            if let Some(requested_counts) = shortlist_pilot_requested_counts.as_ref() {
                shortlist_pilot_applied = pilot_ns == *requested_counts;
                if !shortlist_pilot_applied {
                    shortlist_pilot_fallback = Some("guided_pilot_quotas_restored_after_forecast".into());
                }
            }
        }
        let mut structural_pilot_repartition = json!({"enabled":settings.structural_recovery,"applied":false,"reason":if settings.structural_recovery {"comparable root groups unavailable"} else {"flag off"}});
        if settings.structural_recovery && omission_applied {
            structural_pilot_repartition = json!({"enabled":true,"applied":false,"reason":"bounded omission retains its explicit pilot set"});
        } else if settings.structural_recovery {
            if let (Some(shares), Some(groups)) = (structural_proxy.as_deref(), structural_root_groups.as_deref()) {
                let helper_work = leaves.saturating_mul((usize::BITS as usize - leaves.max(1).leading_zeros() as usize).saturating_add(4))
                    .saturating_add(groups.len().saturating_mul(8));
                if reserve(&mut bank, helper_work) {
                    setup_work = setup_work.saturating_add(helper_work);
                    setup_actual = setup_actual.saturating_add(helper_work);
                    cumulative_reserved = cumulative_reserved.saturating_add(helper_work);
                    reserved_peak = reserved_peak.max(bank.reserved);
                    let old_counts = pilot_ns.clone();
                    if let Some(repartitioned) = structural::repartition_pilot(groups, shares, &old_counts) {
                        let remaining = bank.limit.saturating_sub(bank.reserved);
                        if forecast_pilot_counts(&proposal, m, &repartitioned) <= remaining {
                            pilot_ns = repartitioned;
                            hierarchical_applied = false;
                            hierarchical_reason = Some("comparable structural shares repartitioned each root quota");
                            structural_pilot_repartition = json!({"enabled":true,"applied":true,"reason":"root totals conserved; normalized structural shares repartitioned within roots","helper_work":helper_work,"root_groups":groups.len(),"pilot_counts":pilot_ns});
                        } else {
                            structural_pilot_repartition = json!({"enabled":true,"applied":false,"reason":"repartitioned pilot forecast exceeds available bank; original quotas retained","helper_work":helper_work});
                        }
                    } else {
                        structural_pilot_repartition = json!({"enabled":true,"applied":false,"reason":"invalid shares, root coverage, or per-root floor; original quotas retained","helper_work":helper_work});
                    }
                } else {
                    structural_pilot_repartition = json!({"enabled":true,"applied":false,"reason":"repartition helper work unavailable"});
                }
            }
        }
        if settings.rival_path && !metadata_paid {
            skipped += 1;
            rows.push(json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"leaves":leaves,"stage":"pilot","reason":"rival-path metadata assessment work unavailable"}));
            continue;
        }
        if rival_partition_applied && rival_cached_pilot_counts.is_none() {
            skipped += 1;
            rows.push(json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"leaves":leaves,"stage":"pilot","reason":"frozen base-tree pilot quotas unavailable"}));
            continue;
        }
        let pilot_forecast = pilot_ns
            .iter()
            .enumerate()
            .map(|(i, n)| {
                n.saturating_mul(
                    proposal
                        .draw_work(m, i, ProposalKind::Rank.bound_guidance())
                        .saturating_add(proposal.draw_work(
                            m,
                            i,
                            ProposalKind::Interval.bound_guidance(),
                        )),
                )
            })
            .sum::<usize>();
        if pilot_forecast > bank.limit.saturating_sub(bank.reserved) {
            skipped += 1;
            rows.push(json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"leaves":leaves,"stage":"pilot","reason":"pilot forecast exceeds fresh bank"}));
            continue;
        }
        if !reserve(&mut bank, pilot_forecast) {
            skipped += 1;
            continue;
        }
        cumulative_reserved = cumulative_reserved.saturating_add(pilot_forecast);
        reserved_peak = reserved_peak.max(bank.reserved);
        let pilot_costs = (0..leaves)
            .map(|i| {
                pilot_ns[i].saturating_mul(
                    proposal
                        .draw_work(m, i, true)
                        .saturating_add(proposal.draw_work(m, i, false)),
                )
            })
            .collect::<Vec<_>>();
        let pilots = budget::parallel(&pilot_costs, workers, |i| {
            omission_slot(
                omitted[i],
                (
                    ProposalKind::Rank,
                    BranchSample::default(),
                    BranchSample::default(),
                ),
                || {
                    let sample_key = sampling_keys
                        .as_ref()
                        .and_then(|keys| keys.get(i))
                        .map_or_else(|| i.to_string(), |key| key.clone());
                    // Rank proposal disables bound guidance; interval enables it.
                    let rank_sample = proposal.sample(
                        m,
                        i,
                        pilot_ns[i],
                        derive(
                            seed,
                            &format!(
                                "rank-rescue-pilot-rank-{}-{}-{sample_key}",
                                m.ids[c.cell.team], c.cell.rank
                            ),
                        ),
                        ProposalKind::Rank.bound_guidance(),
                        true,
                    );
                    let interval_sample = proposal.sample(
                        m,
                        i,
                        pilot_ns[i],
                        derive(
                            seed,
                            &format!(
                                "rank-rescue-pilot-interval-{}-{}-{sample_key}",
                                m.ids[c.cell.team], c.cell.rank
                            ),
                        ),
                        ProposalKind::Interval.bound_guidance(),
                        true,
                    );
                    let chosen = if interval_proposal_better(
                        rank_sample.result.ess,
                        interval_sample.result.ess,
                    ) {
                        ProposalKind::Interval
                    } else {
                        ProposalKind::Rank
                    };
                    (chosen, rank_sample, interval_sample)
                },
            )
        });
        let mut proposal_kinds = Vec::with_capacity(leaves);
        let mut pilot_samples = Vec::with_capacity(leaves);
        let mut leaf_pilot_rows = Vec::with_capacity(leaves);
        let hierarchical_root_groups = if settings.hierarchical_pair_pilot {
            proposal.leaf_pilot_root_groups()
        } else {
            None
        };
        let fallback_pilot_shares = if settings.hierarchical_pair_pilot {
            hierarchical_root_groups
                .as_ref()
                .map(|groups| shares_from_counts(groups, &pilot_ns))
        } else {
            None
        };
        let pilot_cost = pilots.iter().fold(0usize, |total, (_, rank, interval)| {
            total.saturating_add(
                rank.result
                    .operations
                    .units()
                    .saturating_add(interval.result.operations.units()),
            )
        });
        pilot_actual = pilot_actual.saturating_add(pilot_cost);
        if pilot_cost > pilot_forecast {
            failed += 1;
            rows.push(json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"leaves":leaves,"stage":"pilot","actual":pilot_cost,"grant":pilot_forecast,"reason":"work overrun; stage stopped"}));
            break;
        }
        assert!(bank.release(pilot_forecast - pilot_cost));
        let pilot_guard_fee = if settings.dual_sigma {
            pilot_guard_work_fee(leaves, settings.pilot_support)
        } else {
            0
        };
        let pilot_guard_fee_reserved = pilot_guard_fee > 0 && reserve(&mut bank, pilot_guard_fee);
        if pilot_guard_fee_reserved {
            cumulative_reserved = cumulative_reserved.saturating_add(pilot_guard_fee);
            reserved_peak = reserved_peak.max(bank.reserved);
        }
        let pilot_guard_fee_actual = if pilot_guard_fee_reserved {
            pilot_guard_fee
        } else {
            0
        };
        let mut pilot_guard_records = Vec::with_capacity(leaves);
        for (i, (chosen, rank_sample, interval_sample)) in pilots.into_iter().enumerate() {
            if pilot_guard_fee_reserved {
                pilot_guard_records.push(Some(PilotGuardRecord {
                    rank_sigma: pilot_sigma(rank_sample.result.std_err, rank_sample.result.samples),
                    interval_sigma: pilot_sigma(
                        interval_sample.result.std_err,
                        interval_sample.result.samples,
                    ),
                    chosen_sigma: pilot_sigma(
                        if chosen == ProposalKind::Interval {
                            interval_sample.result.std_err
                        } else {
                            rank_sample.result.std_err
                        },
                        if chosen == ProposalKind::Interval {
                            interval_sample.result.samples
                        } else {
                            rank_sample.result.samples
                        },
                    ),
                    chosen_probability: if chosen == ProposalKind::Interval {
                        interval_sample.result.probability
                    } else {
                        rank_sample.result.probability
                    },
                    chosen_ess: if chosen == ProposalKind::Interval {
                        interval_sample.result.ess
                    } else {
                        rank_sample.result.ess
                    },
                    rank_probability: rank_sample.result.probability,
                    interval_probability: interval_sample.result.probability,
                    rank_ess: rank_sample.result.ess,
                    interval_ess: interval_sample.result.ess,
                }));
            } else {
                pilot_guard_records.push(None);
            }
            proposal_kinds.push(chosen);
            if settings.leaf_pilot {
                let global_pair_share = leaf_pilot_groups.as_ref().and_then(|groups| {
                    groups
                        .iter()
                        .find(|(_, indices, _)| indices.contains(&i))
                        .and_then(|(_, indices, weights)| {
                            indices
                                .iter()
                                .position(|index| *index == i)
                                .map(|j| weights[j])
                        })
                });
                let legacy_pilot_share = leaf_pilot_groups.as_ref().and_then(|groups| {
                    groups
                        .iter()
                        .find(|(_, indices, _)| indices.contains(&i))
                        .and_then(|(_, indices, weights)| {
                            indices.iter().position(|index| *index == i).and_then(|j| {
                                pilot_shares(weights).and_then(|shares| shares.get(j).copied())
                            })
                        })
                });
                let mut row = json!({
                    "leaf":i,
                    "pilot_count":pilot_ns[i],
                    "rank_probability":rank_sample.result.probability,
                    "rank_std_err":rank_sample.result.std_err,
                    "rank_ess":rank_sample.result.ess,
                    "interval_probability":interval_sample.result.probability,
                    "interval_std_err":interval_sample.result.std_err,
                    "interval_ess":interval_sample.result.ess,
                    "chosen":if chosen==ProposalKind::Rank {"rank"} else {"interval"},
                });
                if settings.hierarchical_pair_pilot {
                    let actual_share = hierarchical_plan
                        .as_ref()
                        .filter(|_| hierarchical_applied)
                        .map(|plan| plan.actual_shares[i])
                        .or_else(|| fallback_pilot_shares.as_ref().and_then(|shares| shares[i]));
                    let original_count = hierarchical_baseline
                        .as_ref()
                        .map(|plan| plan.original_counts[i]);
                    row["pilot_share"] = json!(actual_share);
                    row["pilot_share_basis"] = json!(if hierarchical_applied {
                        "selected_set_group_counts"
                    } else if hierarchical_uniform_fallback {
                        "uniform_fallback"
                    } else if hierarchical_baseline.is_some() {
                        "original_independent_root_counts"
                    } else {
                        "fallback_without_group_metadata"
                    });
                    row["global_pair_pilot_share_diagnostic"] = json!(global_pair_share);
                    row["original_pilot_count"] = json!(original_count);
                    row["original_pilot_share"] = json!(hierarchical_baseline
                        .as_ref()
                        .map(|plan| plan.original_shares[i]));
                    row["hierarchical_group_id"] =
                        json!(hierarchical_plan.as_ref().map(|plan| plan.group_ids[i]));
                } else {
                    row["structural_share"] = json!(global_pair_share);
                    row["pilot_share"] = json!(legacy_pilot_share);
                }
                leaf_pilot_rows.push(row);
            }
            pilot_samples.push(if chosen == ProposalKind::Interval {
                interval_sample
            } else {
                rank_sample
            });
        }
        if settings.leaf_pilot {
            let roots = if settings.hierarchical_pair_pilot {
                hierarchical_root_groups.as_ref().map(|groups| {
                    groups.iter().map(|(outcomes, indexes)| {
                        json!({
                            "outcomes":outcomes,
                            "leaf_count":indexes.len(),
                            "original_pilot_count":hierarchical_baseline.as_ref().map(|plan|indexes.iter().map(|&i|plan.original_counts[i]).sum::<usize>()),
                            "pilot_count":indexes.iter().map(|&i|pilot_ns[i]).sum::<usize>(),
                            "uniform_pilot_count":indexes.len().saturating_mul(settings.pilot_per_leaf),
                            "pilot_probability_sum":indexes.iter().map(|&i|pilot_samples.get(i).map_or(0.0,|s|s.result.probability)).sum::<f64>(),
                            "pilot_sigma_rss":indexes.iter().map(|&i|pilot_samples.get(i).map_or(0.0,|s|s.result.std_err.powi(2))).sum::<f64>().sqrt(),
                            "forecast_work":indexes.iter().map(|&i|pilot_ns[i].saturating_mul(proposal.draw_work(m,i,true).saturating_add(proposal.draw_work(m,i,false)))).sum::<usize>(),
                        })
                    }).collect::<Vec<_>>()
                })
            } else {
                leaf_pilot_groups.as_ref().map(|groups| {
                    groups.iter().map(|(outcomes, indexes, weights)| {
                        json!({
                            "outcomes":outcomes,
                            "leaf_count":indexes.len(),
                            "pilot_count":indexes.iter().map(|&i|pilot_ns[i]).sum::<usize>(),
                            "uniform_pilot_count":indexes.len().saturating_mul(settings.pilot_per_leaf),
                            "structural_share_sum":weights.iter().sum::<f64>(),
                            "pilot_probability_sum":indexes.iter().map(|&i|pilot_samples.get(i).map_or(0.0,|s|s.result.probability)).sum::<f64>(),
                            "pilot_sigma_rss":indexes.iter().map(|&i|pilot_samples.get(i).map_or(0.0,|s|s.result.std_err.powi(2))).sum::<f64>().sqrt(),
                            "forecast_work":indexes.iter().map(|&i|pilot_ns[i].saturating_mul(proposal.draw_work(m,i,true).saturating_add(proposal.draw_work(m,i,false)))).sum::<usize>(),
                        })
                    }).collect::<Vec<_>>()
                })
            };
            let mut event = json!({
                "team":m.ids[c.cell.team],"rank":c.cell.rank+1,"enabled":true,
                "fallback_reason":leaf_pilot_reason,"roots":roots,
                "leaves":leaf_pilot_rows,"pilot_actual_work":pilot_cost,"pilot_forecast_work":pilot_forecast,
            });
            if settings.hierarchical_pair_pilot {
                event["hierarchical_pair_pilot"] = json!({
                    "enabled":true,
                    "work_units":hierarchical_work,
                    "work_limit":BranchStrata::rank_rescue_leaf_pilot_limit(),
                    "fallback_reason":hierarchical_reason,
                    "redistributed":hierarchical_applied,
                    "groups":hierarchical_plan.as_ref().map(|plan|plan.groups.iter().map(|group|json!({
                        "group_id":group.id,"root":group.root,"selected_ids":group.selected_ids,
                        "leaf_count":group.indexes.len(),"original_quota":group.original_quota,
                        "proposed_quota":group.final_quota,
                        "final_quota":group.indexes.iter().map(|&i|pilot_ns[i]).sum::<usize>(),
                        "status":if hierarchical_applied {group.status} else if hierarchical_uniform_fallback {"uniform_quota_restored"} else {"original_quota_restored"},
                    })).collect::<Vec<_>>()).unwrap_or_default(),
                });
            }
            emit(log, "rust_odds_rank_rescue_leaf_pilot", event);
        }
        let pilot = combine_branches(&pilot_samples);
        let chosen_scores = pilot_samples
            .iter()
            .map(|s| pilot_sigma(s.result.std_err, s.result.samples))
            .collect::<Vec<_>>();
        let guard_metrics_available = settings.dual_sigma && pilot_guard_fee_reserved;
        let rank_sigmas = pilot_guard_records
            .iter()
            .map(|record| record.map(|record| record.rank_sigma))
            .collect::<Vec<_>>();
        let interval_sigmas = pilot_guard_records
            .iter()
            .map(|record| record.map(|record| record.interval_sigma))
            .collect::<Vec<_>>();
        let mut scores = chosen_scores.clone();
        let mut dual_sigma_applied = false;
        let mut pilot_guard_reason = if !settings.dual_sigma {
            Some("flag off or gating condition not met")
        } else if !pilot_guard_fee_reserved {
            Some("pilot guard assessment work exceeds available bank")
        } else {
            None
        };
        if guard_metrics_available {
            let risk_scores = dual_sigma_scores(
                &rank_sigmas.iter().flatten().copied().collect::<Vec<_>>(),
                &interval_sigmas
                    .iter()
                    .flatten()
                    .copied()
                    .collect::<Vec<_>>(),
            );
            if pilot_guard_records.iter().any(Option::is_none) {
                pilot_guard_reason = Some("pilot guard assessment records incomplete");
            } else if let Some(risk_scores) = risk_scores.filter(|scores| scores.len() == leaves) {
                scores = risk_scores;
                dual_sigma_applied = true;
                pilot_guard_reason = None;
            } else {
                pilot_guard_reason = Some("invalid dual sigma values; chosen sigma restored");
            }
        }
        let pilot_guard_event = json!({
            "team":m.ids[c.cell.team],
            "rank":c.cell.rank+1,
            "enabled":settings.dual_sigma,
            "applied":dual_sigma_applied,
            "fallback_reason":pilot_guard_reason,
            "work_units":pilot_guard_fee_actual,
            "per_leaf":(0..leaves).map(|i| {
                let record = pilot_guard_records.get(i).copied().flatten();
                let risk = record.and_then(|record| {
                    (record.rank_sigma.is_finite() && record.rank_sigma >= 0.0
                        && record.interval_sigma.is_finite() && record.interval_sigma >= 0.0)
                        .then_some(record.rank_sigma.max(record.interval_sigma))
                });
                let sigma_fold = record.and_then(|record| {
                    finite_positive_ratio(record.rank_sigma.max(record.interval_sigma), record.chosen_sigma)
                });
                json!({
                    "leaf":i,
                    "chosen_sigma":telemetry_float(chosen_scores[i]),
                    "rank_sigma":record.and_then(|record|telemetry_float(record.rank_sigma)),
                    "interval_sigma":record.and_then(|record|telemetry_float(record.interval_sigma)),
                    "max_sigma":risk.and_then(telemetry_float),
                    "used_risk":if dual_sigma_applied {risk.and_then(telemetry_float)} else {telemetry_float(chosen_scores[i])},
                    "rank_probability":record.and_then(|record|telemetry_float(record.rank_probability)),
                    "interval_probability":record.and_then(|record|telemetry_float(record.interval_probability)),
                    "chosen_probability":record.and_then(|record|telemetry_float(record.chosen_probability)),
                    "rank_ess":record.and_then(|record|telemetry_float(record.rank_ess)),
                    "interval_ess":record.and_then(|record|telemetry_float(record.interval_ess)),
                    "chosen_ess":record.and_then(|record|telemetry_float(record.chosen_ess)),
                    "mean_fold":record.and_then(|record|positive_fold(record.rank_probability, record.interval_probability)),
                    "sigma_fold":sigma_fold,
                })
            }).collect::<Vec<_>>(),
        });
        emit(log, "rust_odds_rank_rescue_pilot_guard", pilot_guard_event);
        let shortlist_final_shares = shortlist_static_shares.as_ref().map(|static_shares| {
            if shortlist_mode != "pilot" { return static_shares.clone(); }
            let total = scores.iter().sum::<f64>();
            if !dual_sigma_applied || !total.is_finite() || total <= 0.0 || scores.iter().any(|x|!x.is_finite() || *x < 0.0) {
                shortlist_learning_fallback = Some("dual_sigma_unavailable_or_invalid_static_fallback".into());
                return static_shares.clone();
            }
            shortlist_learned_shares_applied = true;
            scores.iter().zip(shortlist_guide.as_ref().unwrap()).map(|(sigma, guide)| {
                0.90 * sigma / total + 0.05 * guide + 0.05 / leaves.max(1) as f64
            }).collect()
        });
        let parent_map_work = if settings.rival_path {
            leaves.saturating_mul(32).saturating_add(32)
        } else {
            0
        };
        let parent_map_reserved = settings.rival_path && reserve(&mut bank, parent_map_work);
        if parent_map_reserved {
            cumulative_reserved = cumulative_reserved.saturating_add(parent_map_work);
            reserved_peak = reserved_peak.max(bank.reserved);
        }
        let parent_map_actual = if parent_map_reserved {
            parent_map_work
        } else {
            0
        };
        let root_mapping_work = (leaves.saturating_add(c.roots))
            .saturating_mul(m.fixtures.len())
            .saturating_add(leaves.saturating_add(c.roots).saturating_mul(16));
        let pilot_sigma_zero = scores.iter().all(|score| score.is_finite() && *score == 0.0);
        let mapping_reserved = if settings.path_sampling {
            if reserve(&mut bank, root_mapping_work) {
                cumulative_reserved = cumulative_reserved.saturating_add(root_mapping_work);
                reserved_peak = reserved_peak.max(bank.reserved);
                true
            } else {
                false
            }
        } else {
            false
        };
        let structural_root_mapping_reserved = settings.structural_recovery
            && pilot_sigma_zero
            && settings.path_priority
            && !settings.root_exploration
            && !mapping_reserved
            && reserve(&mut bank, root_mapping_work);
        if structural_root_mapping_reserved {
            cumulative_reserved = cumulative_reserved.saturating_add(root_mapping_work);
            reserved_peak = reserved_peak.max(bank.reserved);
        }
        let root_groups = if settings.path_priority && (!settings.path_sampling || mapping_reserved)
        {
            proposal.path_priority_sampling_groups()
        } else {
            None
        };
        let per_draw = (0..leaves)
            .map(|i| proposal.draw_work(m, i, proposal_kinds[i].bound_guidance()))
            .collect::<Vec<_>>();
        let parent_map = if settings.rival_path && parent_map_reserved {
            proposal.rival_path_parent_map()
        } else {
            None
        };
        let parent_map_valid = !settings.rival_path || parent_map.is_some();
        let (parent_ids, child_local_shares) =
            parent_map.unwrap_or_else(|| ((0..leaves).collect::<Vec<_>>(), vec![1.0; leaves]));
        let parent_fallback_shares = if settings.rival_path && parent_map_valid {
            parent_uniform_final_shares(&scores, &parent_ids, &child_local_shares)
        } else {
            None
        };
        let structural_fallback_needed = settings.structural_recovery && pilot_sigma_zero;
        let mut root_exploration_counts = None;
        let mut root_exploration_reason =
            if settings.root_exploration && settings.rival_path && !parent_map_valid {
                Some("original-parent exploration mapping work unavailable")
            } else if !settings.root_exploration {
                Some("flag off or gating condition not met")
            } else if !reserve(&mut bank, root_mapping_work) {
                Some("root exploration mapping work exceeds available bank")
            } else {
                cumulative_reserved = cumulative_reserved.saturating_add(root_mapping_work);
                reserved_peak = reserved_peak.max(bank.reserved);
                None
            };
        let root_exploration_mapping_charged =
            settings.root_exploration && root_exploration_reason.is_none();
        let mut root_exploration_actual = if root_exploration_mapping_charged {
            root_mapping_work
        } else {
            0
        };
        let raw_root_groups = if root_exploration_mapping_charged
            || structural_root_mapping_reserved
            || (settings.path_sampling && mapping_reserved)
        {
            proposal.path_priority_raw_sampling_groups()
        } else {
            None
        };
        let mut structural_fallback_reason = if !settings.structural_recovery {
            Some("flag off")
        } else if !pilot_sigma_zero {
            Some("pilot sigma is positive")
        } else if structural_proxy.is_none() {
            Some("structural proxy invalid; uniform fallback selected")
        } else if raw_root_groups.is_none() {
            Some("raw root proxy unavailable; equal root shares used")
        } else if parent_fallback_shares.is_none() {
            Some("parent support unavailable; structural proxy used alone")
        } else {
            None
        };
        let structural_fallback_shares = if structural_fallback_needed {
            let uniform = vec![1.0 / leaves.max(1) as f64; leaves];
            if let Some(proxy) = structural_proxy.as_ref() {
                structural_final_shares(
                    proxy,
                    raw_root_groups.as_deref(),
                    structural_root_groups.as_deref(),
                    parent_fallback_shares.as_deref(),
                )
                .or_else(|| {
                    structural_fallback_reason = Some("structural allocation shares invalid; uniform fallback selected");
                    Some(uniform)
                })
            } else {
                Some(uniform)
            }
        } else {
            None
        };
        let mut pilot_support_reason = if !settings.pilot_support {
            None
        } else if !pilot_guard_fee_reserved {
            Some("pilot support assessment work unavailable")
        } else if !dual_sigma_applied {
            Some("dual sigma unavailable; pilot support disabled with chosen-sigma fallback")
        } else if !hierarchical_applied {
            Some("hierarchical pair pilot plan unavailable or unapplied")
        } else if hierarchical_plan.is_none() {
            Some("cached hierarchical pilot counts unavailable")
        } else {
            None
        };
        let pilot_support_counts = if settings.pilot_support
            && pilot_guard_fee_reserved
            && dual_sigma_applied
            && hierarchical_applied
        {
            hierarchical_plan.as_ref().and_then(|plan| {
                (plan.counts.len() == leaves
                    && plan.counts.iter().enumerate().all(|(index, count)| {
                        if omitted[index] {
                            pilot_ns[index] == 0
                        } else {
                            *count >= 5 && *count == pilot_ns[index]
                        }
                    }))
                .then_some(pilot_ns.as_slice())
            })
        } else {
            None
        };
        if settings.pilot_support
            && pilot_guard_fee_reserved
            && dual_sigma_applied
            && hierarchical_applied
            && pilot_support_counts.is_none()
        {
            pilot_support_reason =
                Some("cached hierarchical pilot counts invalid; uniform root exploration retained");
        }
        let mut root_exploration_policy = None;
        if settings.root_exploration && root_exploration_reason.is_none() {
            if let Some(groups) = raw_root_groups.as_ref() {
                if scores.iter().copied().fold(0.0, f64::max) <= 0.0 {
                    root_exploration_reason = Some("final pilot sigma total is zero or invalid");
                } else if let Some(policy) = if settings.rival_path {
                    root_exploration_policy_shares_parented(
                        groups,
                        &scores,
                        pilot_support_counts,
                        &parent_ids,
                        &child_local_shares,
                    )
                } else {
                    root_exploration_policy_shares(groups, &scores, pilot_support_counts)
                } {
                    if settings.pilot_support && policy.pilot_support_applied {
                        pilot_support_reason = None;
                    } else if settings.pilot_support && pilot_support_reason.is_none() {
                        pilot_support_reason =
                            Some("dominant root policy unavailable; uniform exploration retained");
                    }
                    root_exploration_policy = Some(policy);
                } else {
                    let valid_partition = groups.len() >= 2
                        && groups.iter().all(|(_, score, indexes)| {
                            score.is_finite() && *score >= 0.0 && !indexes.is_empty()
                        });
                    root_exploration_reason = Some(if !valid_partition {
                        "missing or invalid raw screen grouping"
                    } else {
                        "raw normalized dominant root share below 0.90 or empty side"
                    });
                }
            } else {
                root_exploration_reason = Some("raw screen grouping unavailable");
            }
        }
        if let (Some(groups), Some(policy)) =
            (raw_root_groups.as_ref(), root_exploration_policy.as_ref())
        {
            let planning_grant = hierarchical_search_work_bound(leaves, groups.len());
            if reserve(&mut bank, planning_grant) {
                cumulative_reserved = cumulative_reserved.saturating_add(planning_grant);
                reserved_peak = reserved_peak.max(bank.reserved);
                let plan_available = bank.limit.saturating_sub(bank.reserved);
                let (plan, plan_work) = affordable_root_exploration(
                    &policy.shares,
                    settings.final_draws,
                    2,
                    &per_draw,
                    2,
                    plan_available,
                );
                let settled = plan_work.min(planning_grant);
                root_exploration_actual = root_exploration_actual.saturating_add(settled);
                assert!(bank.release(planning_grant - settled));
                if let Some((counts, _)) = plan {
                    root_exploration_counts = Some(counts);
                    root_exploration_reason = None;
                } else {
                    root_exploration_reason = Some("no affordable root exploration allocation");
                }
            } else {
                root_exploration_reason =
                    Some("root exploration planning work exceeds available bank");
            }
        }
        if settings.pilot_support
            && root_exploration_policy.is_none()
            && pilot_support_reason.is_none()
        {
            pilot_support_reason = Some(
                root_exploration_reason
                    .unwrap_or("dominant root policy unavailable; uniform exploration retained"),
            );
        }
        let mut hierarchical_counts = None;
        let mut path_plan_reason = if !settings.path_sampling {
            None
        } else if !mapping_reserved {
            Some("root allocation mapping work exceeds available bank")
        } else if root_groups.is_none() {
            Some("missing or invalid root allocation plan")
        } else {
            None
        };
        let mut allocation_actual = if mapping_reserved {
            root_mapping_work
        } else {
            0
        };
        if structural_root_mapping_reserved {
            allocation_actual = allocation_actual.saturating_add(root_mapping_work);
        }
        allocation_actual = allocation_actual
            .saturating_add(root_exploration_actual)
            .saturating_add(parent_map_actual)
            .saturating_add(pilot_guard_fee_actual);
        let mut structural_fallback_counts = None;
        if let Some(shares) = structural_fallback_shares.as_ref() {
            let root_count = structural_root_groups.as_ref().map_or(1, Vec::len);
            let planning_grant = hierarchical_search_work_bound(leaves, root_count);
            if reserve(&mut bank, planning_grant) {
                cumulative_reserved = cumulative_reserved.saturating_add(planning_grant);
                reserved_peak = reserved_peak.max(bank.reserved);
                let plan_available = bank.limit.saturating_sub(bank.reserved);
                let (plan, plan_work) = affordable_allocation_from_shares(
                    shares,
                    settings.final_draws,
                    2,
                    &per_draw,
                    2,
                    plan_available,
                );
                allocation_actual = allocation_actual.saturating_add(plan_work);
                assert!(bank.release(planning_grant.saturating_sub(plan_work)));
                if let Some((counts, _)) = plan {
                    structural_fallback_counts = Some(counts);
                    structural_fallback_reason = None;
                } else {
                    structural_fallback_reason = Some("no affordable structural allocation");
                }
            } else {
                structural_fallback_reason = Some("structural allocation planning work unavailable");
            }
        }
        if settings.path_sampling {
            if let Some(groups) = root_groups.as_ref() {
                let planning_grant = hierarchical_search_work_bound(leaves, groups.len());
                if reserve(&mut bank, planning_grant) {
                    cumulative_reserved = cumulative_reserved.saturating_add(planning_grant);
                    reserved_peak = reserved_peak.max(bank.reserved);
                    let plan_available = bank.limit.saturating_sub(bank.reserved);
                    let (plan, plan_work) = affordable_hierarchical_allocation(
                        groups,
                        &scores,
                        settings.final_draws,
                        2,
                        &per_draw,
                        2,
                        plan_available,
                    );
                    let settled_plan = plan_work.min(planning_grant);
                    allocation_actual = allocation_actual.saturating_add(settled_plan);
                    assert!(bank.release(planning_grant - settled_plan));
                    if let Some((counts, _)) = plan {
                        hierarchical_counts = Some(counts);
                        path_plan_reason = None;
                    } else {
                        path_plan_reason = Some("no affordable hierarchical allocation");
                    }
                } else {
                    path_plan_reason = Some("hierarchical allocation work exceeds available bank");
                }
            }
        }
        let mut parent_fallback_counts = None;
        let mut parent_fallback_reason = if !settings.rival_path {
            None
        } else if parent_fallback_shares.is_none() {
            Some("original-parent exploration shares unavailable")
        } else if root_exploration_counts.is_some() {
            Some("root policy allocation already available")
        } else {
            None
        };
        if settings.rival_path && root_exploration_counts.is_none() {
            if let Some(shares) = parent_fallback_shares.as_ref() {
                let parent_count = parent_ids.iter().copied().collect::<BTreeSet<_>>().len();
                let planning_grant = hierarchical_search_work_bound(leaves, parent_count);
                if reserve(&mut bank, planning_grant) {
                    cumulative_reserved = cumulative_reserved.saturating_add(planning_grant);
                    reserved_peak = reserved_peak.max(bank.reserved);
                    let plan_available = bank.limit.saturating_sub(bank.reserved);
                    let (plan, plan_work) = affordable_allocation_from_shares(
                        shares,
                        settings.final_draws,
                        2,
                        &per_draw,
                        2,
                        plan_available,
                    );
                    allocation_actual = allocation_actual.saturating_add(plan_work);
                    assert!(bank.release(planning_grant.saturating_sub(plan_work)));
                    if let Some((counts, _)) = plan {
                        parent_fallback_counts = Some(counts);
                        parent_fallback_reason = None;
                    } else {
                        parent_fallback_reason = Some("no affordable original-parent allocation");
                    }
                } else {
                    parent_fallback_reason =
                        Some("original-parent allocation planning work exceeds available bank");
                }
            }
        }
        allocation_actual_total = allocation_actual_total.saturating_add(allocation_actual);
        let mut shortlist_plan = None;
        if let Some(shares) = shortlist_final_shares.as_ref() {
            if !omission_applied {
                let planning_grant = hierarchical_search_work_bound(leaves, 1);
                if reserve(&mut bank, planning_grant) {
                    cumulative_reserved = cumulative_reserved.saturating_add(planning_grant);
                    reserved_peak = reserved_peak.max(bank.reserved);
                    let plan_available = bank.limit.saturating_sub(bank.reserved);
                    let (plan, plan_work) = affordable_allocation_from_shares(
                        shares, settings.final_draws, 2, &per_draw, 2, plan_available,
                    );
                    allocation_actual = allocation_actual.saturating_add(plan_work);
                    allocation_actual_total = allocation_actual_total.saturating_add(plan_work);
                    assert!(bank.release(planning_grant.saturating_sub(plan_work)));
                    if let Some((counts, grant)) = plan {
                        shortlist_plan = Some((counts, grant));
                        shortlist_reason = None;
                    } else { shortlist_reason = Some("no_affordable_guided_final_pair".into()); }
                } else { shortlist_reason = Some("guided_final_planning_work_unavailable".into()); }
            } else { shortlist_reason = Some("omission_incompatible_with_shortlist".into()); }
        }
        let available = bank.limit.saturating_sub(bank.reserved);
        let raw_root_diagnostics = raw_root_groups.as_ref().and_then(|groups| {
            let normalized =
                normalize_weights(&groups.iter().map(|group| group.1).collect::<Vec<_>>())?;
            let dominant = (0..groups.len()).max_by(|&a, &b| {
                normalized[a]
                    .total_cmp(&normalized[b])
                    .then_with(|| b.cmp(&a))
            })?;
            Some((dominant, normalized[dominant]))
        });
        let legacy_counts = || {
            if settings.rival_path {
                parent_fallback_counts.clone().and_then(|counts| {
                    forecast(&counts, &per_draw, 2)
                        .filter(|cost| *cost <= available)
                        .map(|cost| (counts, cost))
                })
            } else {
                affordable_allocation(&scores, settings.final_draws, 2, &per_draw, 2, available)
            }
        };
        let root_exploration_final = match root_exploration_counts {
            Some(counts) => match forecast(&counts, &per_draw, 2) {
                Some(cost) if cost <= available => Some((counts, cost)),
                _ => {
                    root_exploration_reason = Some("final pair forecast exceeds available bank");
                    None
                }
            },
            None => None,
        };
        let structural_fallback_final = match structural_fallback_counts {
            Some(counts) => match forecast(&counts, &per_draw, 2) {
                Some(cost) if cost <= available => Some((counts, cost)),
                _ => {
                    structural_fallback_reason = Some("structural pair forecast exceeds available bank");
                    None
                }
            },
            None => None,
        };
        let (counts, pair_grant, allocation_mode) = if omission_applied {
            let retained_indexes = omitted
                .iter()
                .enumerate()
                .filter_map(|(index, is_omitted)| (!*is_omitted).then_some(index))
                .collect::<Vec<_>>();
            let explicit_shares = if root_exploration_final.is_some() {
                root_exploration_policy
                    .as_ref()
                    .map(|policy| policy.shares.as_slice())
            } else if structural_fallback_final.is_some() {
                structural_fallback_shares.as_deref()
            } else if settings.rival_path && parent_fallback_counts.is_some() {
                parent_fallback_shares.as_deref()
            } else {
                None
            };
            let local_shares = retained_indexes
                .iter()
                .map(|&index| explicit_shares.map_or(scores[index], |shares| shares[index]))
                .collect::<Vec<_>>();
            let local_costs = retained_indexes
                .iter()
                .map(|&index| per_draw[index])
                .collect::<Vec<_>>();
            let planning_grant = hierarchical_search_work_bound(retained_indexes.len(), 1);
            if !reserve(&mut bank, planning_grant) {
                skipped += 1;
                rows.push(json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"leaves":leaves,"stage":"final_pair","reason":"retained-leaf allocation work exceeds available bank","available":available,"draw_cap":settings.final_draws}));
                continue;
            }
            cumulative_reserved = cumulative_reserved.saturating_add(planning_grant);
            reserved_peak = reserved_peak.max(bank.reserved);
            let planning_available = bank.limit.saturating_sub(bank.reserved);
            let (plan, plan_work) = if explicit_shares.is_some() {
                affordable_allocation_from_shares(
                    &local_shares,
                    settings.final_draws,
                    2,
                    &local_costs,
                    2,
                    planning_available,
                )
            } else {
                affordable_allocation_measured(
                    &local_shares,
                    settings.final_draws,
                    2,
                    &local_costs,
                    2,
                    planning_available,
                )
            };
            allocation_actual = allocation_actual.saturating_add(plan_work);
            assert!(bank.release(planning_grant.saturating_sub(plan_work)));
            let Some((local_counts, grant)) = plan else {
                skipped += 1;
                allocation_actual_total = allocation_actual_total.saturating_add(allocation_actual);
                rows.push(json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"leaves":leaves,"stage":"final_pair","reason":"no affordable retained-leaf allocation","available":planning_available,"draw_cap":settings.final_draws}));
                continue;
            };
            let mut counts = vec![0usize; leaves];
            for (&index, count) in retained_indexes.iter().zip(local_counts) {
                counts[index] = count;
            }
            (counts, grant, "bounded_omission_retained")
        } else if let Some((counts, grant)) = shortlist_plan {
            (counts, grant, "frozen_shortlist_guidance")
        } else {
            match root_exploration_final {
                Some((counts, grant)) => (counts, grant, "root_exploration"),
                None if structural_fallback_final.is_some() => {
                    let (counts, grant) = structural_fallback_final.unwrap();
                    (counts, grant, "structural_recovery_fallback")
                }
                None => match hierarchical_counts {
                    Some(counts) => match forecast(&counts, &per_draw, 2) {
                        Some(cost) if cost <= available => (counts, cost, "hierarchical"),
                        Some(_) => {
                            path_plan_reason =
                                Some("hierarchical pair forecast exceeds available bank");
                            let Some((counts, grant)) = legacy_counts() else {
                                skipped += 1;
                                rows.push(json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"leaves":leaves,"pilot_ess":pilot.ess,"stage":"final_pair","reason":"pair forecast exceeds available bank","available":available,"draw_cap":settings.final_draws,"path_sampling_fallback":path_plan_reason}));
                                continue;
                            };
                            (counts, grant, "legacy_fallback")
                        }
                        None => {
                            path_plan_reason = Some("invalid hierarchical work forecast");
                            let Some((counts, grant)) = legacy_counts() else {
                                skipped += 1;
                                rows.push(json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"leaves":leaves,"pilot_ess":pilot.ess,"stage":"final_pair","reason":"pair forecast exceeds available bank","available":available,"draw_cap":settings.final_draws,"path_sampling_fallback":path_plan_reason}));
                                continue;
                            };
                            (counts, grant, "legacy_fallback")
                        }
                    },
                    None => {
                        let Some((counts, grant)) = legacy_counts() else {
                            skipped += 1;
                            if settings.root_exploration {
                                emit(
                                    log,
                                    "rust_odds_rank_rescue_root_exploration",
                                    json!({
                                        "team":m.ids[c.cell.team],"rank":c.cell.rank+1,
                                        "enabled":true,"applied":false,
                                        "reason":root_exploration_reason.unwrap_or("legacy fallback forecast exceeds available bank"),
                                        "pilot_support_applied":false,
                                        "pilot_support_reason":if settings.pilot_support {pilot_support_reason.or(root_exploration_reason)} else {None},
                                        "dominant_outcomes":raw_root_groups.as_ref().and_then(|groups|raw_root_diagnostics.and_then(|(i,_)|groups.get(i).map(|g|g.0.clone()))),
                                        "dominant_raw_normalized_share":raw_root_diagnostics.map(|(_,share)|share),
                                        "allocation_planning_work":root_exploration_actual,
                                        "pilot_guard_work":pilot_guard_fee_actual,
                                        "mapping_work":if root_exploration_mapping_charged {root_mapping_work} else {0},
                                    }),
                                );
                            }
                            rows.push(json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"leaves":leaves,"pilot_ess":pilot.ess,"stage":"final_pair","reason":"pair forecast exceeds available bank","available":available,"draw_cap":settings.final_draws,"path_sampling_fallback":path_plan_reason}));
                            continue;
                        };
                        let mode = if settings.rival_path {
                            "rival_path_parent_uniform"
                        } else if settings.path_sampling || settings.root_exploration {
                            "legacy_fallback"
                        } else {
                            "legacy"
                        };
                        (counts, grant, mode)
                    }
                },
            }
        };
        let root_exploration_event = if settings.root_exploration {
            let groups = raw_root_groups.as_ref();
            let policy = root_exploration_policy.as_ref();
            let applied_policy = (allocation_mode == "root_exploration")
                .then_some(policy)
                .flatten();
            let dominant_index = policy
                .map(|policy| policy.dominant_index)
                .or_else(|| raw_root_diagnostics.map(|(index, _)| index));
            let support_target_groups = hierarchical_plan.as_ref().and_then(|plan| {
                if !applied_policy.is_some_and(|policy| policy.pilot_support_applied) {
                    return None;
                }
                let dominant_root = groups?.get(dominant_index?)?;
                Some(
                    plan.groups
                        .iter()
                        .filter(|group| group.root == dominant_root.0)
                        .map(|group| {
                            json!({
                                "group_id":group.id,
                                "selected_ids":group.selected_ids,
                                "pilot_count":group.indexes.iter().map(|&i|pilot_ns[i]).sum::<usize>(),
                            })
                        })
                        .collect::<Vec<_>>(),
                )
            });
            json!({
                "team":m.ids[c.cell.team],
                "rank":c.cell.rank+1,
                "enabled": settings.root_exploration,
                "applied": allocation_mode == "root_exploration",
                "reason": if allocation_mode == "root_exploration" {None} else {root_exploration_reason},
                "pilot_support_applied": applied_policy.is_some_and(|policy|policy.pilot_support_applied),
                "pilot_support_reason": if applied_policy.is_some_and(|policy|policy.pilot_support_applied) {None} else if settings.pilot_support {pilot_support_reason.or(root_exploration_reason)} else {None},
                "dominant_outcomes": groups.and_then(|groups| dominant_index.and_then(|i| groups.get(i).map(|g|g.0.clone()))),
                "dominant_raw_normalized_share": policy.map(|policy|policy.raw_share).or_else(||raw_root_diagnostics.map(|(_,share)|share)),
                "dominant_leaf_count": policy.map(|policy|policy.dominant_leaf_count).or_else(||groups.and_then(|groups|dominant_index.and_then(|i|groups.get(i).map(|g|g.2.len())))),
                "other_leaf_count": policy.map(|policy|policy.other_leaf_count).or_else(||groups.and_then(|groups|dominant_index.map(|i|groups.iter().enumerate().filter(|(j,_)|*j!=i).map(|(_,g)|g.2.len()).sum::<usize>()))),
                "total_draws": counts.iter().sum::<usize>(),
                "floor_per_leaf": 2,
                "learned_policy_share_total": applied_policy.map(|_|0.9),
                "exploration_policy_share_total": applied_policy.map(|_|0.1),
                "dominant_exploration_share": applied_policy.map(|_|0.09),
                "other_exploration_share": applied_policy.map(|_|0.01),
                "proposed_policy_shares": if applied_policy.is_none() {policy.map(|policy|policy.shares.clone())} else {None},
                "policy_shares": applied_policy.map(|policy|policy.shares.clone()),
                "learned_policy_shares": applied_policy.map(|policy|policy.learned_shares.clone()),
                "exploration_policy_shares": applied_policy.map(|policy|policy.exploration_shares.clone()),
                "pilot_support_shares": applied_policy.map(|policy|policy.pilot_support_shares.clone()),
                "pilot_support_count_base": if applied_policy.is_some_and(|policy|policy.pilot_support_applied) {hierarchical_plan.as_ref().map(|plan|plan.counts.clone())} else {None},
                "pilot_support_root_count_total": applied_policy.and_then(|policy|policy.dominant_pilot_count),
                "pilot_support_selected_set_groups": support_target_groups,
                "original_parent_count": if settings.rival_path {Some(parent_ids.iter().copied().collect::<BTreeSet<_>>().len())} else {None},
                "original_parent_ids": if settings.rival_path {Some(parent_ids.clone())} else {None},
                "parent_child_proxy_shares": if settings.rival_path {Some(child_local_shares.clone())} else {None},
                "parent_uniform_fallback_shares": if settings.rival_path {parent_fallback_shares.clone()} else {None},
                "parent_uniform_fallback_reason": parent_fallback_reason,
                "planned_draws_per_root": groups.map(|groups|groups.iter().map(|(outcomes,_,indexes)|json!({
                    "outcomes":outcomes,
                    "draws":indexes.iter().map(|&i|counts[i]).sum::<usize>(),
                    "leaves":indexes.len(),
                })).collect::<Vec<_>>()),
                "allocation_planning_work": root_exploration_actual,
                "pilot_guard_work": pilot_guard_fee_actual,
                "mapping_work": if root_exploration_mapping_charged {root_mapping_work} else {0},
            })
        } else {
            json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"enabled":false,"applied":false,"reason":"flag off or gating condition not met","allocation_planning_work":0})
        };
        emit(
            log,
            "rust_odds_rank_rescue_root_exploration",
            root_exploration_event,
        );
        if !reserve(&mut bank, pair_grant) {
            skipped += 1;
            continue;
        }
        cumulative_reserved = cumulative_reserved.saturating_add(pair_grant);
        reserved_peak = reserved_peak.max(bank.reserved);
        let main_rows = sample_stage(
            &proposal,
            m,
            &counts,
            derive(
                seed,
                &format!("rank-rescue-main-{}-{}", m.ids[c.cell.team], c.cell.rank),
            ),
            "main",
            &proposal_kinds,
            workers,
            final_sampling_keys.as_deref(),
            omission_applied,
        );
        let main = combine_branches(&main_rows);
        let main_charge = charge(&main_rows);
        final_actual = final_actual.saturating_add(main_charge);
        if main_charge > pair_grant {
            failed += 1;
            rows.push(json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"leaves":leaves,"stage":"final_main","actual":main_charge,"grant":pair_grant,"overrun":true,"actual_overrun_units":main_charge.saturating_sub(pair_grant),"reason":"work overrun; stopped before check"}));
            break;
        }
        let check_forecast = counts
            .iter()
            .zip(&per_draw)
            .map(|(n, cost)| n.saturating_mul(*cost))
            .sum::<usize>();
        let check_funded = check_forecast <= pair_grant.saturating_sub(main_charge);
        let check_eligible = if omission_applied {
            main.weighted && main.probability.is_finite() && main.probability > 0.0
        } else {
            rough_publishable(&main)
        };
        let check_rows = if check_eligible && check_funded {
            sample_stage(
                &proposal,
                m,
                &counts,
                derive(
                    seed,
                    &format!("rank-rescue-check-{}-{}", m.ids[c.cell.team], c.cell.rank),
                ),
                "check",
                &proposal_kinds,
                workers,
                final_sampling_keys.as_deref(),
                omission_applied,
            )
        } else {
            Vec::new()
        };
        let check = if check_rows.is_empty() {
            Result::default()
        } else {
            combine_branches(&check_rows)
        };
        let check_charge = charge(&check_rows);
        final_actual = final_actual.saturating_add(check_charge);
        let mut actual = main_charge.saturating_add(check_charge);
        if settings.leaf_pilot {
            let root_rows = root_groups.as_ref().map(|groups| {
                groups.iter().map(|(outcomes, _, indexes)| {
                    let main_root = combine_group(&main_rows, indexes);
                    let check_root = (!check_rows.is_empty()).then(|| combine_group(&check_rows, indexes));
                    json!({
                        "outcomes":outcomes,
                        "leaf_count":indexes.len(),
                        "planned_draws":indexes.iter().map(|&i|counts[i]).sum::<usize>(),
                        "forecast_work":indexes.iter().map(|&i|counts[i].saturating_mul(per_draw[i])).sum::<usize>().saturating_mul(2),
                        "main_probability":main_root.probability,
                        "main_ess":main_root.ess,
                        "check_probability":check_root.as_ref().map(|r|r.probability),
                        "check_ess":check_root.as_ref().map(|r|r.ess),
                        "check_ran":!check_rows.is_empty(),
                    })
                }).collect::<Vec<_>>()
            });
            let leaf_rows = (0..leaves)
                .map(|i| {
                    let mut row = json!({
                        "leaf":i,
                        "pilot_count":pilot_ns[i],
                        "final_count":counts[i],
                        "forecast_work":counts[i].saturating_mul(per_draw[i]).saturating_mul(2),
                        "main_probability":main_rows.get(i).map(|s|s.result.probability),
                        "main_ess":main_rows.get(i).map(|s|s.result.ess),
                        "check_probability":check_rows.get(i).map(|s|s.result.probability),
                        "check_ess":check_rows.get(i).map(|s|s.result.ess),
                        "check_ran":!check_rows.is_empty(),
                    });
                    if settings.hierarchical_pair_pilot {
                        let global_pair_share = leaf_pilot_groups.as_ref().and_then(|groups| {
                            groups
                                .iter()
                                .find(|(_, indexes, _)| indexes.contains(&i))
                                .and_then(|(_, indexes, weights)| {
                                    indexes
                                        .iter()
                                        .position(|index| *index == i)
                                        .map(|j| weights[j])
                                })
                        });
                        let actual_share = hierarchical_plan
                            .as_ref()
                            .filter(|_| hierarchical_applied)
                            .map(|plan| plan.actual_shares[i])
                            .or_else(|| {
                                fallback_pilot_shares.as_ref().and_then(|shares| shares[i])
                            });
                        row["pilot_share"] = json!(actual_share);
                        row["pilot_share_basis"] = json!(if hierarchical_applied {
                            "selected_set_group_counts"
                        } else if hierarchical_uniform_fallback {
                            "uniform_fallback"
                        } else if hierarchical_baseline.is_some() {
                            "original_independent_root_counts"
                        } else {
                            "fallback_without_group_metadata"
                        });
                        row["global_pair_pilot_share_diagnostic"] = json!(global_pair_share);
                        row["original_pilot_count"] = json!(hierarchical_baseline
                            .as_ref()
                            .map(|plan| plan.original_counts[i]));
                        row["original_pilot_share"] = json!(hierarchical_baseline
                            .as_ref()
                            .map(|plan| plan.original_shares[i]));
                        row["hierarchical_group_id"] =
                            json!(hierarchical_plan.as_ref().map(|plan| plan.group_ids[i]));
                    } else {
                        let structural_share = leaf_pilot_groups.as_ref().and_then(|groups| {
                            groups
                                .iter()
                                .find(|(_, indexes, _)| indexes.contains(&i))
                                .and_then(|(_, indexes, weights)| {
                                    indexes
                                        .iter()
                                        .position(|index| *index == i)
                                        .map(|j| weights[j])
                                })
                        });
                        let pilot_share = leaf_pilot_groups.as_ref().and_then(|groups| {
                            groups
                                .iter()
                                .find(|(_, indexes, _)| indexes.contains(&i))
                                .and_then(|(_, indexes, weights)| {
                                    indexes.iter().position(|index| *index == i).and_then(|j| {
                                        pilot_shares(weights)
                                            .and_then(|shares| shares.get(j).copied())
                                    })
                                })
                        });
                        row["structural_share"] = json!(structural_share);
                        row["pilot_share"] = json!(pilot_share);
                    }
                    row
                })
                .collect::<Vec<_>>();
            emit(
                log,
                "rust_odds_rank_rescue_leaf_pilot_final",
                json!({
                    "team":m.ids[c.cell.team],"rank":c.cell.rank+1,
                    "allocation_mode":allocation_mode,"roots":root_rows,"leaves":leaf_rows,
                    "main_actual_work":main_charge,"check_actual_work":check_charge,
                }),
            );
        }
        if actual > pair_grant {
            failed += 1;
            rows.push(json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"leaves":leaves,"stage":"final_pair","actual":actual,"grant":pair_grant,"overrun":true,"actual_overrun_units":actual.saturating_sub(pair_grant),"reason":"work overrun; stage stopped"}));
            break;
        }
        assert!(bank.release(pair_grant - actual));
        // A rejected original pair may inform one bounded retry allocation,
        // but none of its samples enter the retry estimate.
        let original_pair_accepted = !omission_applied
            && check_funded
            && replacement_or_new_accepts(&old, &main, &check, c.was_positive);
        let retry_trigger = settings.structural_recovery && !omission_applied && !original_pair_accepted;
        let mut retry_row = json!({"attempted":false,"triggered":retry_trigger,"reason":if retry_trigger {"original pair failed quality gates"} else {"original pair accepted or omission mode"}});
        let mut retry_overrun = false;
        let mut effective_main = main.clone();
        let mut effective_check = check.clone();
        let mut committed_from = "original";
        let mut effective_check_funded = check_funded;
        if retry_trigger {
            let main_remaining = settings.final_draws.saturating_sub(main.samples);
            let check_remaining = settings.final_draws.saturating_sub(check.samples);
            let retry_total = main_remaining.min(check_remaining);
            let mut retry_scores = (0..leaves)
                .map(|i| {
                    let first = scores.get(i).copied().unwrap_or(0.0);
                    let retry_sigma = main_rows.get(i).map_or(0.0, |row| {
                        if row.result.samples > 0 && row.result.std_err.is_finite() {
                            pilot_sigma(row.result.std_err, row.result.samples)
                        } else { 0.0 }
                    });
                    first.max(retry_sigma)
                })
                .collect::<Vec<_>>();
            let mut retry_share_source = "pilot_and_failed_main_sigma";
            if retry_scores.iter().all(|x| !x.is_finite() || *x <= 0.0) {
                if let Some(shares) = structural_fallback_shares.as_ref() {
                    retry_scores.clone_from(shares);
                    retry_share_source = "structural_proxy";
                } else {
                    retry_scores.fill(1.0);
                    retry_share_source = "uniform";
                }
            }
            let planning_grant = hierarchical_search_work_bound(leaves, 1);
            let mut retry_planning_reserved = false;
            let retry_plan = if retry_total >= leaves.saturating_mul(2) && reserve(&mut bank, planning_grant) {
                retry_planning_reserved = true;
                cumulative_reserved = cumulative_reserved.saturating_add(planning_grant);
                reserved_peak = reserved_peak.max(bank.reserved);
                let (planned, work) = affordable_allocation_from_shares(
                    &retry_scores,
                    retry_total,
                    2,
                    &per_draw,
                    2,
                    bank.limit.saturating_sub(bank.reserved),
                );
                (planned, work)
            } else { (None, 0) };
            let retry_plan_work = retry_plan.1.min(planning_grant);
            if retry_planning_reserved {
                allocation_actual = allocation_actual.saturating_add(retry_plan_work);
                allocation_actual_total = allocation_actual_total.saturating_add(retry_plan_work);
                assert!(bank.release(planning_grant.saturating_sub(retry_plan_work)));
            }
            if let (Some((retry_counts, _retry_forecast)), retry_plan_work) = retry_plan {
                let _ = retry_plan_work;
                let retry_forecast = retry_counts.iter().zip(&per_draw)
                    .map(|(n,c)| n.saturating_mul(*c)).sum::<usize>().saturating_mul(2);
                if reserve(&mut bank, retry_forecast) {
                    cumulative_reserved = cumulative_reserved.saturating_add(retry_forecast);
                    reserved_peak = reserved_peak.max(bank.reserved);
                    let retry_main_rows = sample_stage(
                        &proposal, m, &retry_counts,
                        derive(seed, &format!("rank-rescue-retry-main-{}-{}", m.ids[c.cell.team], c.cell.rank)),
                        "retry-main", &proposal_kinds, workers, final_sampling_keys.as_deref(), omission_applied,
                    );
                    let retry_main = combine_branches(&retry_main_rows);
                    let retry_main_charge = charge(&retry_main_rows);
                    let retry_check_forecast = retry_counts.iter().zip(&per_draw).map(|(n,c)| n.saturating_mul(*c)).sum::<usize>();
                    let retry_check_funded = retry_check_forecast <= retry_forecast.saturating_sub(retry_main_charge);
                    let retry_check_eligible = if omission_applied {
                        retry_main.weighted && retry_main.probability.is_finite() && retry_main.probability > 0.0
                    } else { rough_publishable(&retry_main) };
                    let retry_check_rows = if retry_main_charge <= retry_forecast && retry_check_eligible && retry_check_funded {
                        sample_stage(
                            &proposal, m, &retry_counts,
                            derive(seed, &format!("rank-rescue-retry-check-{}-{}", m.ids[c.cell.team], c.cell.rank)),
                            "retry-check", &proposal_kinds, workers, final_sampling_keys.as_deref(), omission_applied,
                        )
                    } else { Vec::new() };
                    let retry_check = if retry_check_rows.is_empty() { Result::default() } else { combine_branches(&retry_check_rows) };
                    let retry_check_charge = charge(&retry_check_rows);
                    let retry_actual = retry_main_charge.saturating_add(retry_check_charge);
                    retry_overrun = retry_actual > retry_forecast;
                    final_actual = final_actual.saturating_add(retry_actual);
                    actual = actual.saturating_add(retry_actual);
                    let retry_ok = retry_check_funded
                        && retry_main_charge <= retry_forecast
                        && retry_actual <= retry_forecast
                        && accepted_pair_for(&old, &retry_main, &retry_check, c.was_positive);
                    if !retry_overrun {
                        assert!(bank.release(retry_forecast - retry_actual));
                    }
                    if retry_ok {
                        effective_main = retry_main.clone();
                        effective_check = retry_check.clone();
                        committed_from = "retry";
                        effective_check_funded = retry_check_funded;
                    }
                    retry_row = json!({
                        "attempted":true,"triggered":true,"reason":if retry_ok {"fresh retry pair passed gates"} else {"fresh retry pair failed gates"},
                        "share_source":retry_share_source,"main_label":"retry-main","check_label":"retry-check",
                        "planned_draws_per_stream":retry_counts.iter().sum::<usize>(),"planned_counts":retry_counts,
                        "main_actual_draws":retry_main.samples,"check_actual_draws":retry_check.samples,
                        "original_main_draws":main.samples,"original_check_draws":check.samples,
                        "main_hits":retry_main.hits,"check_hits":retry_check.hits,
                        "main_probability":retry_main.probability,"check_probability":retry_check.probability,
                        "main_ess":retry_main.ess,"check_ess":retry_check.ess,
                        "main_rough_publishable":rough_publishable(&retry_main),"check_rough_publishable":rough_publishable(&retry_check),
                        "forecast_work":retry_forecast,"actual_work":retry_actual,"overrun":retry_overrun,"committed":retry_ok
                    });
                } else { retry_row["reason"] = json!("retry forecast exceeds available bank"); }
            } else if retry_total < leaves.saturating_mul(2) {
                retry_row["reason"] = json!("per-stream draw cap leaves fewer than two draws per leaf");
            } else { retry_row["reason"] = json!("retry allocation unavailable under remaining work bank"); }
        }
        if settings.path_priority {
            let total_draws = counts.iter().sum::<usize>();
            let root_rows = root_groups
                .as_ref()
                .map(|groups| {
                    groups
                        .iter()
                        .map(|(outcomes, proxy_share, indexes)| {
                            let root_draws = indexes.iter().map(|&i| counts[i]).sum::<usize>();
                            let root_forecast = indexes
                                .iter()
                                .map(|&i| counts[i].saturating_mul(per_draw[i]))
                                .sum::<usize>()
                                .saturating_mul(2);
                            let pilot_sigma_sum = indexes.iter().map(|&i| scores[i]).sum::<f64>();
                            let pilot_root = combine_group(&pilot_samples, indexes);
                            let main_draws = indexes
                                .iter()
                                .filter_map(|&i| main_rows.get(i))
                                .map(|s| s.result.samples)
                                .sum::<usize>();
                            let check_draws = indexes
                                .iter()
                                .filter_map(|&i| check_rows.get(i))
                                .map(|s| s.result.samples)
                                .sum::<usize>();
                            let main_work = indexes
                                .iter()
                                .filter_map(|&i| main_rows.get(i))
                                .map(|s| s.result.operations.units())
                                .sum::<usize>();
                            let check_work = indexes
                                .iter()
                                .filter_map(|&i| check_rows.get(i))
                                .map(|s| s.result.operations.units())
                                .sum::<usize>();
                            let main_root = combine_group(&main_rows, indexes);
                            let check_root = if check_rows.is_empty() {
                                None
                            } else {
                                Some(combine_group(&check_rows, indexes))
                            };
                            json!({
                                "outcomes":outcomes,
                                "leaf_count":indexes.len(),
                                "operational_share":proxy_share,
                                "pilot_sigma_sum":pilot_sigma_sum,
                                "pilot_probability":pilot_root.probability,
                                "pilot_std_err":pilot_root.std_err,
                                "planned_draws":root_draws,
                                "planned_draw_share":if total_draws>0{root_draws as f64/total_draws as f64}else{0.0},
                                "forecast_work":root_forecast,
                                "forecast_work_share":if pair_grant>0{root_forecast as f64/pair_grant as f64}else{0.0},
                                "actual_main_draws":main_draws,
                                "actual_main_work":main_work,
                                "actual_check_draws":check_draws,
                                "actual_check_work":check_work,
                                "main_probability":main_root.probability,
                                "main_ess":main_root.ess,
                                "check_probability":check_root.as_ref().map(|r|r.probability),
                                "check_ess":check_root.as_ref().map(|r|r.ess),
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            emit(
                log,
                "rust_odds_rank_rescue_path_sampling",
                json!({
                    "team":m.ids[c.cell.team],
                    "rank":c.cell.rank+1,
                    "path_sampling_enabled":settings.path_sampling,
                    "allocation_mode":allocation_mode,
                    "fallback_reason":path_plan_reason,
                    "allocation_work_units":allocation_actual,
                    "final_planned_draws":total_draws,
                    "forecast_work":pair_grant,
                    "check_status":if check_rows.is_empty(){"not_run"}else{"run"},
                    "roots":root_rows,
                }),
            );
        }
        let main_bracket = omission_applied
            .then(|| coarse_bracket(effective_main.probability, effective_main.std_err, omission_ledger));
        let check_bracket = omission_applied
            .then(|| coarse_bracket(effective_check.probability, effective_check.std_err, omission_ledger));
        let coarse_accepted = omission_applied
            && effective_check_funded
            && effective_main.weighted
            && effective_check.weighted
            && coarse_bracket_pair_accepted(&effective_main, &effective_check, omission_ledger);
        let quality = effective_check_funded
            && !omission_applied
            && commit_candidate(&mut estimates[index], &old, &effective_main, &effective_check, c.was_positive);
        let mut replaced = false;
        if quality {
            estimates[index].work_spent = estimates[index].work_spent.saturating_add(
                (c.root_work + setup_work + pilot_cost + allocation_actual + actual) as u64,
            );
            added += 1;
            if c.was_positive {
                upgrades += 1;
                replaced = true;
            }
        }
        let main_rse = (effective_main.probability > 0.).then(|| rse(&effective_main));
        let check_rse = (effective_check.probability > 0.).then(|| rse(&effective_check));
        let baseline_rse = estimate_relative_se(&old);
        let ess_improved = effective_main.ess.min(effective_check.ess) > old.ess;
        let rse_improved = rse(&effective_main).max(rse(&effective_check)) < baseline_rse;
        let main_rough = rough_publishable(&effective_main);
        let check_rough = rough_publishable(&effective_check);
        let accepted_pair = accepted(&effective_main, &effective_check, true);
        let mut final_row = json!({
            "team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots,"leaves":leaves,
            "main_samples":effective_main.samples,"main_hits":effective_main.hits,"main_ess":effective_main.ess,"main_rse":main_rse,
            "main_max_share":effective_main.max_share,"main_batch_gap":effective_main.batch_gap,"main_probability":effective_main.probability,
            "check_samples":effective_check.samples,"check_hits":effective_check.hits,"check_ess":effective_check.ess,"check_rse":check_rse,
            "check_max_share":effective_check.max_share,"check_batch_gap":effective_check.batch_gap,"check_probability":effective_check.probability,
            "main_rough_publishable":main_rough,"check_rough_publishable":check_rough,"accepted_pair":accepted_pair,
            "baseline_ess":old.ess,"baseline_rse":baseline_rse,"ess_improved":ess_improved,"rse_improved":rse_improved,
            "original_p":old.probability,"replacement":replaced,"committed":quality,"check_funded":effective_check_funded,
            "reason":if quality{"accepted"}else if c.was_positive{"quality_gate"}else{"pair_rejected"},
            "root_count_work":c.root_work,"setup_work":setup_work,"pilot_work":pilot_cost,"final_work":actual,
            "allocation_mode":allocation_mode,"allocation_work":allocation_actual,"path_sampling_fallback":path_plan_reason,
            "reserved_work":c.root_work+setup_work+pilot_cost+allocation_actual+pair_grant,
            "committed_from":if quality {committed_from}else{"none"}
        });
        final_row["original_pair"] = json!({"main_samples":main.samples,"check_samples":check.samples,"main_hits":main.hits,"check_hits":check.hits,"accepted":original_pair_accepted});
        final_row["retry"] = retry_row;
        final_row["structural_recovery"] = json!({
            "enabled":settings.structural_recovery,
            "proxy_valid":structural_proxy.is_some(),
            "proxy_reason":structural_proxy_reason,
            "pilot_repartition":structural_pilot_repartition,
            "zero_sigma_final_fallback_needed":structural_fallback_needed,
            "zero_sigma_final_fallback_applied":allocation_mode == "structural_recovery_fallback",
            "zero_sigma_final_fallback_reason":structural_fallback_reason,
            "final_fallback_shares_used":allocation_mode == "structural_recovery_fallback"
        });
        if shortlist_requested.is_some() && target_shortlist_cell {
            let guide_entropy = shortlist_guide.as_ref().map(|shares| {
                shares.iter().filter(|share| **share > 0.0).map(|share| -share * share.ln()).sum::<f64>()
            });
            final_row["shortlist_guidance"] = json!({
                "mode":shortlist_mode,
                "applied":allocation_mode == "frozen_shortlist_guidance",
                "pilot_quota_applied":shortlist_pilot_applied,
                "fallback_reason":shortlist_learning_fallback.or(shortlist_reason),
                "pilot_fallback_reason":shortlist_pilot_fallback,
                "learned_sigma_applied":shortlist_learned_shares_applied,
                "candidate_count":if shortlist_guide.is_some(){53}else{0},
                "compatible_leaf_counts":shortlist_compatible_counts,
                "all_loss_leaf_ids":shortlist_all_loss_leaf_ids,
                "compatibility_comparison_work":shortlist_comparison_work,
                "unmatched_proxy_weight":shortlist_unmatched_weight,
                "guide_entropy_nats":guide_entropy,
                "guide_by_leaf":shortlist_guide,
                "pilot_counts":pilot_ns,
                "final_counts":counts,
                "scan_and_mapping_work":shortlist_scan_work,
                "work_units_definition":"prepaid bound for scanning 4,550 frozen rows, decoding selected H5 scores, checking all-loss leaf packed intervals, and normalizing allocation shares"
            });
        }
        if settings.bounded_omission {
            final_row["bounded_omission"] = json!({
                "applied":omission_applied,
                "leaf_bounds":omission_bound_values.iter().enumerate().map(|(leaf,bound)|json!({"leaf_id":leaf,"upper_bound":if omission_bounds_available{Some(*bound)}else{None}})).collect::<Vec<_>>(),
                "omitted_leaf_ids":omitted.iter().enumerate().filter_map(|(leaf,drop_leaf)|(*drop_leaf).then_some(leaf)).collect::<Vec<_>>(),
                "omitted_bound_ledger":omission_ledger,"omission_allowance":omission_allowance,
                "omission_prefix_ledger":omission_prefix.iter().map(|(leaf,upper)|json!({"leaf_id":leaf,"cumulative_upper_bound":upper})).collect::<Vec<_>>(),
                "bound_work_units":omission_work,"bound_work_cap":settings.bounded_omission_work,
                "pilot_counts":pilot_ns,"final_counts":counts,
                "main_bracket":main_bracket,"check_bracket":check_bracket,
                "coarse_accepted":coarse_accepted,"committed":quality
            });
        }
        rows.push(final_row);
        if retry_overrun {
            failed += 1;
            break;
        }
    }
    let actual_work = setup_actual
        .saturating_add(pilot_actual)
        .saturating_add(final_actual)
        .saturating_add(allocation_actual_total);
    let mut summary = json!({
        "enabled":true,"target_filter":settings.target_filter,"eligible":eligible,"attempted":tree_attempted,"root_probe_count":root_probe_count,"tree_attempted":tree_attempted,"ranked_candidates":ranked_candidates,"found":added,"new":added.saturating_sub(upgrades),"upgraded":upgrades,"failed":failed,"skipped":skipped,"preserved_positives":preserved_positives,
        "setup_actual":setup_actual,"pilot_actual":pilot_actual,"final_actual":final_actual,"allocation_actual":allocation_actual_total,"actual_work":actual_work,"reserved_work":bank.reserved,"cumulative_reserved_work":cumulative_reserved,"peak_reserved_work":reserved_peak,"work_limit":limit,"stage_final_draw_cap":settings.final_draws,"reference_draws":settings.reference_draws,"pilot_per_leaf":settings.pilot_per_leaf,"max_attempts":settings.max_attempts,"path_priority":settings.path_priority,"path_sampling":settings.path_sampling,"leaf_pilot":settings.leaf_pilot,"dual_sigma":settings.dual_sigma,"pilot_support":settings.pilot_support,"rival_path":settings.rival_path,"goal_tiebreak":settings.goal_tiebreak,"large_budget":settings.large_budget,"cells":rows,"elapsed_ms":started.elapsed().as_secs_f64()*1000.0
    });
    if settings.shared_goal_tiebreak {
        summary["shared_goal_tiebreak"] = json!(true);
        if settings.shared_goal_tiebreak_work_limit != SHARED_GOAL_TIEBREAK_WORK_DEFAULT {
            summary["shared_goal_tiebreak_work_limit"] =
                json!(settings.shared_goal_tiebreak_work_limit);
        }
    }
    if settings.bounded_omission {
        summary["bounded_omission"] = json!(true);
        summary["bounded_omission_work_cap"] = json!(settings.bounded_omission_work);
        summary["bounded_omission_work_cap_units"] =
            json!("charged upper-bound scan/assembly iterations, DP state/storage units, transition evaluations, and omission sort/ledger allowance");
    }
    if settings.pair_pilot {
        summary["pair_pilot"] = json!(true);
    }
    if settings.hierarchical_pair_pilot {
        summary["hierarchical_pair_pilot"] = json!(true);
    }
    emit(log, "rust_odds_rare_tail_rank_rescue_summary", summary);
    (added, actual_work as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{conditioned::canonical_ranks, model::Request};
    use serde_json::json;

    fn tiny_model() -> Model {
        let request: Request = serde_json::from_value(json!({
            "id": 1,
            "phase": {"sort": "pt,w,bias", "championship": {"point_win": 3, "point_draw": 1, "point_loss": 0}},
            "team_groups": (0..4).map(|team| json!({"team_id": team, "add_sub": team, "bias": team})).collect::<Vec<_>>(),
            "games": ([(0, 1), (0, 2), (1, 3), (2, 3)].iter().enumerate().map(|(i, &(home, away))| json!({
                "id": i, "home_id": home, "away_id": away, "home_power": 1.1, "away_power": 0.9
            })).collect::<Vec<_>>())
        }))
        .unwrap();
        Model::new(request).unwrap()
    }

    fn exact_rank_mass(m: &Model, cell: Cell) -> f64 {
        let mut mass = 0.0;
        for mut code in 0..3usize.pow(m.fixtures.len() as u32) {
            let mut outcomes = vec![0u8; m.fixtures.len()];
            let mut probability = 1.0;
            for (i, fixture) in m.fixtures.iter().enumerate() {
                let outcome = code % 3;
                code /= 3;
                outcomes[i] = outcome as u8;
                probability *= fixture.prob[outcome];
            }
            if canonical_ranks(m, &outcomes)[cell.team] == cell.rank {
                mass += probability;
            }
        }
        mass
    }

    #[test]
    fn path_priority_is_inert_without_rank_rescue_and_off_by_default() {
        assert_eq!(
            budget_caps(false, Some("1"), Some("20000000"), Some("6000000")),
            (false, 2_000_000, 600_000)
        );
        assert_eq!(
            budget_caps(true, None, None, None),
            (false, 90_000, 600_000)
        );
        assert_eq!(
            budget_caps(true, Some("1"), None, None),
            (true, 90_000, 600_000)
        );
        assert_eq!(
            budget_caps(true, Some("1"), Some("20000000"), Some("6000000")),
            (true, 20_000_000, 6_000_000)
        );
        assert_eq!(
            shared_goal_tiebreak_work_limit(false, true, Some("512000000")),
            SHARED_GOAL_TIEBREAK_WORK_DEFAULT
        );
        assert_eq!(
            shared_goal_tiebreak_work_limit(true, false, Some("512000000")),
            SHARED_GOAL_TIEBREAK_WORK_DEFAULT
        );
        assert_eq!(
            shared_goal_tiebreak_work_limit(true, true, None),
            SHARED_GOAL_TIEBREAK_WORK_DEFAULT
        );
        assert_eq!(
            shared_goal_tiebreak_work_limit(true, true, Some("bad")),
            SHARED_GOAL_TIEBREAK_WORK_DEFAULT
        );
        assert_eq!(
            shared_goal_tiebreak_work_limit(true, true, Some("1")),
            SHARED_GOAL_TIEBREAK_WORK_DEFAULT
        );
        assert_eq!(
            shared_goal_tiebreak_work_limit(true, true, Some("64000000")),
            SHARED_GOAL_TIEBREAK_WORK_DEFAULT
        );
        assert_eq!(
            shared_goal_tiebreak_work_limit(true, true, Some("70000000")),
            70_000_000
        );
        assert_eq!(
            shared_goal_tiebreak_work_limit(true, true, Some("1")),
            SHARED_GOAL_TIEBREAK_WORK_DEFAULT
        );
        assert_eq!(
            shared_goal_tiebreak_work_limit(true, true, Some("900000000")),
            SHARED_GOAL_TIEBREAK_WORK_MAX
        );
        assert!(!path_priority_enabled(false, Some("1")));
        assert!(!path_priority_enabled(true, None));
        assert!(!path_priority_enabled(true, Some("true")));
        assert!(path_priority_enabled(true, Some("1")));
        assert!(!root_exploration_enabled(false, true, false, Some("1")));
        assert!(!root_exploration_enabled(true, false, false, Some("1")));
        assert!(!root_exploration_enabled(true, true, true, Some("1")));
        assert!(!root_exploration_enabled(true, true, false, None));
        assert!(root_exploration_enabled(true, true, false, Some("1")));
        assert!(!dual_sigma_enabled(false, true, true, false, Some("1")));
        assert!(!dual_sigma_enabled(true, false, true, false, Some("1")));
        assert!(!dual_sigma_enabled(true, true, false, false, Some("1")));
        assert!(!dual_sigma_enabled(true, true, true, true, Some("1")));
        assert!(!dual_sigma_enabled(true, true, true, false, None));
        assert!(dual_sigma_enabled(true, true, true, false, Some("1")));
        assert!(!pilot_support_enabled(false, true, true, Some("1")));
        assert!(!pilot_support_enabled(true, false, true, Some("1")));
        assert!(!pilot_support_enabled(true, true, false, Some("1")));
        assert!(pilot_support_enabled(true, true, true, Some("1")));
        assert!(!rival_path_enabled(
            false,
            true,
            true,
            true,
            true,
            true,
            false,
            Some("1")
        ));
        assert!(!rival_path_enabled(
            true,
            false,
            true,
            true,
            true,
            true,
            false,
            Some("1")
        ));
        assert!(!rival_path_enabled(
            true,
            true,
            false,
            true,
            true,
            true,
            false,
            Some("1")
        ));
        assert!(!rival_path_enabled(
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            Some("1")
        ));
        assert!(!rival_path_enabled(
            true,
            true,
            true,
            true,
            true,
            false,
            false,
            Some("1")
        ));
        assert!(!rival_path_enabled(
            true, true, true, true, true, true, false, None
        ));
        assert!(rival_path_enabled(
            true,
            true,
            true,
            true,
            true,
            true,
            false,
            Some("1")
        ));
        assert!(!goal_tiebreak_enabled(false, Some("1")));
        assert!(!goal_tiebreak_enabled(true, None));
        assert!(goal_tiebreak_enabled(true, Some("1")));
        assert!(!shared_goal_tiebreak_enabled(false, Some("1")));
        assert!(!shared_goal_tiebreak_enabled(true, None));
        assert!(shared_goal_tiebreak_enabled(true, Some("1")));
        assert!(!path_sampling_enabled(false, true, Some("1")));
        assert!(!path_sampling_enabled(true, false, Some("1")));
        assert!(!path_sampling_enabled(true, true, None));
        assert!(path_sampling_enabled(true, true, Some("1")));
        assert!(!leaf_pilot_enabled(false, true, Some("1")));
        assert!(!leaf_pilot_enabled(true, false, Some("1")));
        assert!(!leaf_pilot_enabled(true, true, None));
        assert!(leaf_pilot_enabled(true, true, Some("1")));
        assert!(!pair_pilot_enabled(false, true, true, Some("1")));
        assert!(!pair_pilot_enabled(true, false, true, Some("1")));
        assert!(!pair_pilot_enabled(true, true, false, Some("1")));
        assert!(!pair_pilot_enabled(true, true, true, None));
        assert!(pair_pilot_enabled(true, true, true, Some("1")));
        assert!(!hierarchical_pair_pilot_enabled(
            false,
            true,
            true,
            true,
            Some("1")
        ));
        assert!(!hierarchical_pair_pilot_enabled(
            true,
            false,
            true,
            true,
            Some("1")
        ));
        assert!(!hierarchical_pair_pilot_enabled(
            true,
            true,
            false,
            true,
            Some("1")
        ));
        assert!(!hierarchical_pair_pilot_enabled(
            true,
            true,
            true,
            false,
            Some("1")
        ));
        assert!(!hierarchical_pair_pilot_enabled(
            true, true, true, true, None
        ));
        assert!(hierarchical_pair_pilot_enabled(
            true,
            true,
            true,
            true,
            Some("1")
        ));
    }

    #[test]
    fn certified_empty_roots_are_not_attempt_candidates_only_under_recovery() {
        assert!(selector_candidate_allowed(0, false));
        assert!(!selector_candidate_allowed(0, true));
        assert!(selector_candidate_allowed(1, true));
    }

    #[test]
    fn palmeiras_rank_thirteen_root_bound_certifies_forty_five_of_forty_six() {
        let request: Request = serde_json::from_str(include_str!(
            "../../tests/fixtures/group-16498-2026-10-10.json"
        )).unwrap();
        let model = Model::new(request).unwrap();
        let team = model.ids.iter().position(|id| *id == 16).unwrap();
        let cell = Cell { team, rank: 12 };
        let mut raw_filter = |_: &[u8]| (true, 0);
        let raw = BranchStrata::overflow_root_count_filtered(
            &model, cell, 901, ROOT_LIMIT, &mut raw_filter,
        ).0.unwrap();
        assert_eq!(raw, 46);
        let mut stats = RootBoundStats::default();
        let mut filter = |root: &[u8]| root_bound_filter(&model, cell, root, &mut stats);
        let retained = BranchStrata::overflow_root_count_filtered(
            &model, cell, 901, ROOT_LIMIT, &mut filter,
        ).0.unwrap();
        assert_eq!(retained, 1);
        assert_eq!(stats.pruned, 45);
        let all_loss = model.fixtures.iter().filter(|fixture| fixture.home == team || fixture.away == team)
            .map(|fixture| if fixture.home == team { 0u8 } else { 2u8 }).collect::<Vec<_>>();
        assert_eq!(all_loss.len(), 9);
        assert!(root_bound_filter(&model, cell, &all_loss, &mut RootBoundStats::default()).0);
    }

    #[test]
    fn parent_uniform_exploration_preserves_parent_mass_including_zero_sigma() {
        let ids = [0, 0, 0, 1];
        let local = [0.2, 0.3, 0.5, 1.0];
        let zero = parent_uniform_final_shares(&[0.0; 4], &ids, &local).unwrap();
        assert!((zero[..3].iter().sum::<f64>() - 0.5).abs() < 1e-12);
        assert!((zero[3] - 0.5).abs() < 1e-12);
        let mixed = parent_uniform_final_shares(&[1.0; 4], &ids, &local).unwrap();
        let first_parent = mixed[..3].iter().sum::<f64>();
        assert!((first_parent - 0.9 * 0.75 - 0.05).abs() < 1e-12);
        assert!((mixed[3] - 0.9 * 0.25 - 0.05).abs() < 1e-12);
        assert!((mixed.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn root_exploration_reallocates_256_leaves_with_floor_and_conservation() {
        let dominant_leaves = (0..22).collect::<Vec<_>>();
        let other_leaves = (22..256).collect::<Vec<_>>();
        let groups = vec![
            (vec![0], 0.95, dominant_leaves.clone()),
            (vec![1], 0.05, other_leaves.clone()),
        ];
        let sigma = vec![1.0; 256];
        let policy = root_exploration_policy_shares(&groups, &sigma, None).unwrap();
        assert_eq!(
            (
                policy.dominant_index,
                policy.dominant_leaf_count,
                policy.other_leaf_count
            ),
            (0, 22, 234)
        );
        assert!((policy.raw_share - 0.95).abs() < 1e-12);
        assert!((policy.shares.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        let counts = root_exploration_allocation(&policy.shares, 100_000, 2).unwrap();
        assert_eq!(counts.iter().sum::<usize>(), 100_000);
        assert!(counts.iter().all(|count| *count >= 2));
        let dominant_share =
            dominant_leaves.iter().map(|&i| counts[i]).sum::<usize>() as f64 / 100_000.0;
        assert!(dominant_share > 0.16);
        assert!(dominant_share < 0.18);
        assert!(dominant_share > policy.dominant_leaf_count as f64 / 256.0);
    }

    #[test]
    fn root_exploration_rejects_threshold_invalid_partition_and_zero_sigma() {
        let sigma = vec![1.0, 1.0, 1.0];
        let below = vec![(vec![0], 0.899, vec![0]), (vec![1], 0.101, vec![1, 2])];
        assert!(root_exploration_policy_shares(&below, &sigma, None).is_none());
        let invalid = vec![(vec![0], 0.95, vec![0, 1]), (vec![1], 0.05, vec![1, 2])];
        assert!(root_exploration_policy_shares(&invalid, &sigma, None).is_none());
        assert!(root_exploration_policy_shares(&[], &sigma, None).is_none());
        let roots = vec![(vec![0], 0.95, vec![0]), (vec![1], 0.05, vec![1, 2])];
        assert!(root_exploration_policy_shares(&roots, &[0.0, 0.0, 0.0], None).is_none());
        assert_eq!(allocation(&[0.0, 0.0, 0.0], 30, 2), Some(vec![10, 10, 10]));
        assert!(root_exploration_policy_shares(&roots, &[1.0, f64::NAN, 1.0], None).is_none());
    }

    #[test]
    fn dual_sigma_uses_maximum_sigma_with_each_streams_own_sample_count() {
        let rank = pilot_sigma(0.2, 25);
        let interval = pilot_sigma(0.03, 100);
        assert!((rank - 1.0).abs() < 1e-12);
        assert!((interval - 0.3).abs() < 1e-12);
        assert_eq!(dual_sigma_scores(&[rank], &[interval]), Some(vec![rank]));
        assert_eq!(
            dual_sigma_scores(&[0.0, 0.0], &[0.0, 0.0]),
            Some(vec![0.0, 0.0])
        );
        assert_eq!(dual_sigma_scores(&[1.0], &[]), None);
        assert_eq!(dual_sigma_scores(&[f64::INFINITY], &[0.0]), None);
        assert_eq!(positive_fold(0.5, 0.1), Some(5.0));
        assert_eq!(positive_fold(0.5, 0.0), None);
        assert!((finite_positive_ratio(rank, interval).unwrap() - (1.0 / 0.3)).abs() < 1e-12);
        assert!((finite_positive_ratio(rank, interval.max(rank)).unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn pilot_support_blends_cached_dominant_counts_without_changing_them() {
        let groups = vec![(vec![0], 0.95, vec![0, 1]), (vec![1], 0.05, vec![2])];
        let dual = vec![6.0, 3.0, 1.0];
        let counts = vec![10, 30, 20];
        let original_counts = counts.clone();
        let policy = root_exploration_policy_shares(&groups, &dual, Some(&counts)).unwrap();
        assert!(policy.pilot_support_applied);
        assert_eq!(policy.dominant_pilot_count, Some(40));
        assert!((policy.learned_shares[0] - 0.54).abs() < 1e-12);
        assert!((policy.learned_shares[1] - 0.27).abs() < 1e-12);
        assert!((policy.learned_shares[2] - 0.09).abs() < 1e-12);
        assert!((policy.pilot_support_shares[0] - 0.02025).abs() < 1e-12);
        assert!((policy.pilot_support_shares[1] - 0.06075).abs() < 1e-12);
        assert!((policy.exploration_shares[0] - 0.02475).abs() < 1e-12);
        assert!((policy.exploration_shares[1] - 0.06525).abs() < 1e-12);
        assert!((policy.exploration_shares[2] - 0.01).abs() < 1e-12);
        assert!((policy.shares.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert_eq!(counts, original_counts);

        let invalid = root_exploration_policy_shares(&groups, &dual, Some(&[10, 0, 20])).unwrap();
        let uniform = root_exploration_policy_shares(&groups, &dual, None).unwrap();
        assert!(!invalid.pilot_support_applied);
        assert_eq!(invalid.shares, uniform.shares);
        assert_eq!(invalid.exploration_shares, uniform.exploration_shares);
    }

    #[test]
    fn pilot_guard_assessment_fee_is_bounded_linear_work() {
        assert_eq!(pilot_guard_work_fee(0, false), 16);
        assert_eq!(pilot_guard_work_fee(256, false), 2_064);
        assert_eq!(pilot_guard_work_fee(256, true), 3_088);
        assert!(pilot_guard_work_fee(256, true) < 256 * 16);
    }

    #[test]
    fn root_exploration_affordable_search_uses_unequal_draw_costs() {
        let groups = vec![(vec![0], 0.95, vec![0]), (vec![1], 0.05, vec![1, 2])];
        let shares = root_exploration_policy_shares(&groups, &[1.0, 2.0, 3.0], None)
            .unwrap()
            .shares;
        let per_draw = vec![1, 4, 9];
        let available = 10_000;
        let (allocation, work) =
            affordable_root_exploration(&shares, 10_000, 2, &per_draw, 2, available);
        let (counts, cost) = allocation.unwrap();
        assert!(work <= hierarchical_search_work_bound(3, 2));
        assert_eq!(cost, forecast(&counts, &per_draw, 2).unwrap());
        assert!(cost <= available);
        assert!(counts.iter().sum::<usize>() <= 10_000);
        assert!(counts.iter().all(|count| *count >= 2));
    }
    #[test]
    fn bounded_settings_clamp_and_defaults() {
        assert_eq!(setting(None, 25, 5, 5000), 25);
        assert_eq!(setting(Some("1"), 25, 5, 5000), 5);
        assert_eq!(setting(Some("9000"), 25, 5, 5000), 5000);
        assert_eq!(setting(Some("bad"), 25, 5, 5000), 25);
        assert!(!enabled_value(None));
        assert!(!enabled_value(Some("true")));
        assert!(enabled_value(Some("1")));
    }

    #[test]
    fn bounded_omission_gating_and_work_cap_are_opt_in() {
        assert!(!bounded_omission_enabled(false, false, true, Some("1")));
        assert!(!bounded_omission_enabled(true, true, true, Some("1")));
        assert!(!bounded_omission_enabled(true, false, false, Some("1")));
        assert!(!bounded_omission_enabled(true, false, true, None));
        assert!(bounded_omission_enabled(true, false, true, Some("1")));
        assert!(!omission_old_probability_valid(0.0));
        assert!(!omission_old_probability_valid(f64::NAN));
        assert!(!omission_old_probability_valid(f64::INFINITY));
        assert!(omission_old_probability_valid(0.01));
        assert_eq!(
            bounded_omission_work(false, true, Some("20000000")),
            8_000_000
        );
        assert_eq!(
            bounded_omission_work(true, false, Some("20000000")),
            8_000_000
        );
        assert_eq!(bounded_omission_work(true, true, None), 8_000_000);
        assert_eq!(bounded_omission_work(true, true, Some("bad")), 8_000_000);
        assert_eq!(bounded_omission_work(true, true, Some("1")), 8_000_000);
        assert_eq!(
            bounded_omission_work(true, true, Some("600000000")),
            512_000_000
        );
        assert_eq!(
            bounded_omission_work(true, true, Some("9000000")),
            9_000_000
        );
    }

    #[test]
    fn omission_freeze_respects_allowance_stable_order_and_no_support_leak() {
        let (omitted, ledger, allowance) = freeze_omissions(0.05, &[0.02, 0.01, 0.4]);
        assert_eq!(omitted, vec![true, true, false]);
        assert!(ledger <= allowance);
        assert!(ledger <= 9.0 * 0.05);

        // Equal bounds use stable indexes, and the final (largest-index) leaf
        // remains available even when every bound fits the ledger.
        let (tied, _, _) = freeze_omissions(0.5, &[0.01, 0.01, 0.01]);
        assert_eq!(tied, vec![true, true, false]);
        for probability in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                freeze_omissions(probability, &[0.01, 0.02]).0,
                vec![false, false]
            );
        }
        assert_eq!(
            freeze_omissions(0.5, &[0.01, f64::NAN]).0,
            vec![false, false]
        );
        assert_eq!(freeze_omissions(0.5, &[0.0, 0.2]).0, vec![false, false]);
        assert_eq!(coarse_bracket(0.2, 0.01, f64::NAN), None);
    }

    #[test]
    fn omitted_slots_keep_full_index_alignment_without_sampling() {
        let omitted = [false, true, false, true];
        let slots = omitted
            .iter()
            .enumerate()
            .map(|(index, &drop_leaf)| omission_slot(drop_leaf, 0, || index + 10))
            .collect::<Vec<_>>();
        assert_eq!(slots, vec![10, 0, 12, 0]);
        assert_eq!(slots.len(), omitted.len());
    }

    #[test]
    fn omitted_mass_is_added_to_both_frozen_stream_brackets() {
        let omitted_mass = 0.015;
        let (omitted, ledger, _) = freeze_omissions(0.01, &[omitted_mass, 0.5]);
        assert!(omitted[0]);
        assert!(ledger >= omitted_mass);
        let exact = 0.08;
        let retained_probability = exact - omitted_mass;
        let main = Result {
            weighted: true,
            probability: retained_probability,
            std_err: 0.005,
            ..Result::default()
        };
        let check = Result {
            weighted: true,
            probability: retained_probability,
            std_err: 0.005,
            ..Result::default()
        };
        let main_bracket = coarse_bracket(main.probability, main.std_err, ledger).unwrap();
        let check_bracket = coarse_bracket(check.probability, check.std_err, ledger).unwrap();
        assert!(main_bracket.0 <= exact && exact <= main_bracket.1);
        assert!(check_bracket.0 <= exact && exact <= check_bracket.1);
    }
    #[test]
    fn target_filter_parses_one_based_rank_and_rejects_invalid_ranges() {
        assert_eq!(parse_target_filter(None), Ok(None));
        assert_eq!(parse_target_filter(Some("550:14")), Ok(Some((550, 14))));
        assert_eq!(
            parse_target_filter(Some("550")),
            Err("expected TEAM_ID:RANK")
        );
        assert_eq!(
            parse_target_filter(Some("x:2")),
            Err("team ID must be an integer")
        );
        assert_eq!(
            parse_target_filter(Some("550:0")),
            Err("rank is one-based and must be positive")
        );
        assert_eq!(target_cell(Some((550, 2)), &[100, 550], 4), Ok(Some(5)));
        assert_eq!(
            target_cell(Some((550, 5)), &[100, 550], 4),
            Err("rank is outside the model")
        );
        assert_eq!(
            target_cell(Some((999, 2)), &[100, 550], 4),
            Err("team ID is outside the model")
        );
    }
    #[test]
    fn weak_positive_requires_a_real_conditional_estimate() {
        let mut e = Estimate {
            probability: 0.4,
            ess: 1.0,
            ..Estimate::default()
        };
        assert!(!candidate(&e));
        e.conditional_samples = 1;
        assert!(candidate(&e));
    }
    #[test]
    fn allocation_has_full_support_and_honors_floor() {
        let counts = allocation(&[0., 100., 1.], 100, 2).unwrap();
        assert!(counts.iter().all(|n| *n >= 2));
        assert_eq!(counts.iter().sum::<usize>(), 100);
        assert!(allocation(&[1., 1.], 3, 2).is_none());
    }

    #[test]
    fn retained_score_fallback_keeps_legacy_uniform_mix_and_zero_score_floor() {
        let (uniform, work) = affordable_allocation_measured(&[0.0, 0.0], 10, 2, &[1, 1], 2, 20);
        assert!(work > 0);
        assert_eq!(uniform, Some((vec![5, 5], 20)));
        // Legacy score allocation gives the 10% uniform exploration share
        // alongside 90% normalized pilot score.
        assert_eq!(allocation(&[9.0, 1.0], 102, 2), Some(vec![87, 15]));
    }

    #[test]
    fn structural_pilot_quotas_have_exact_totals_and_minimum_five() {
        let shares = pilot_shares(&[0.9, 0.1]).unwrap();
        assert!((shares[0] - 0.86).abs() < 1e-12);
        assert!((shares[1] - 0.14).abs() < 1e-12);
        let counts = quota_counts(&shares, 50, 5).unwrap();
        assert_eq!(counts.iter().sum::<usize>(), 50);
        assert!(counts.iter().all(|count| *count >= 5));
        assert!(counts[0] > counts[1]);
        assert_eq!(quota_counts(&[0.0, 0.0], 50, 5), None);
        assert_eq!(quota_counts(&[0.9, 0.1], 9, 5), None);
        assert_eq!(quota_counts(&[0.9, 0.1], 10, 5), Some(vec![5, 5]));
        assert_eq!(pilot_shares(&[0.0, 0.0]), Some(vec![0.5, 0.5]));
        assert_eq!(leaf_plan_work(1_900_000, 1, 1, 10), Some(148));
        assert_eq!(leaf_plan_work(1_999_999, 1, 1, 100), None);
    }

    #[test]
    fn hierarchical_pair_pilot_preserves_root_and_selected_group_quotas() {
        let roots = vec![(vec![0], vec![0, 1, 2, 3]), (vec![1], vec![4, 5])];
        let independent = vec![0.4, 0.3, 0.2, 0.1, 0.5, 0.5];
        // Leaves 0 and 1 share the selected set even though a strict mask
        // could differ. Leaves 2/3 are singleton families; 4/5 have all-zero
        // corrected support and keep their original root-local quota.
        let logs = vec![Some(0.0), Some(-2.0), Some(1.0), Some(-1.0), None, None];
        let selected = vec![
            vec![10, 20],
            vec![10, 20],
            vec![30],
            vec![40],
            vec![50],
            vec![50],
        ];
        let plan = hierarchical_pilot_plan(&roots, &independent, &logs, &selected, 10).unwrap();
        assert_eq!(plan.original_counts[0..4].iter().sum::<usize>(), 40);
        assert_eq!(plan.original_counts[4..6].iter().sum::<usize>(), 20);
        assert_eq!(plan.counts[0..4].iter().sum::<usize>(), 40);
        assert_eq!(plan.counts[4..6].iter().sum::<usize>(), 20);
        assert_eq!(plan.counts[2], plan.original_counts[2]);
        assert_eq!(plan.counts[3], plan.original_counts[3]);
        assert_eq!(plan.counts[4], plan.original_counts[4]);
        assert_eq!(plan.counts[5], plan.original_counts[5]);
        assert_ne!(plan.counts[0], plan.original_counts[0]);
        assert!(plan.counts.iter().all(|count| *count >= 5));
        assert_eq!(plan.groups.len(), 4);
        let family = plan
            .groups
            .iter()
            .find(|group| group.selected_ids == [10, 20])
            .unwrap();
        assert_eq!(
            family.original_quota,
            plan.original_counts[0] + plan.original_counts[1]
        );
        assert_eq!(family.final_quota, plan.counts[0] + plan.counts[1]);
        assert_eq!(family.status, "redistributed");
        let zero = plan
            .groups
            .iter()
            .find(|group| group.selected_ids == [50])
            .unwrap();
        assert_eq!(zero.status, "original_quota_kept_zero_scores");
        assert_eq!(plan.group_ids[0], plan.group_ids[1]);
    }

    #[test]
    fn hierarchical_pair_plan_work_is_bounded_by_existing_leaf_cap() {
        let base = leaf_plan_work(100, 100, 10, 8).unwrap();
        let extra = hierarchical_leaf_plan_extra_work(100, 100, 10, 8, 20).unwrap();
        assert!(100 + base + extra <= BranchStrata::rank_rescue_leaf_pilot_limit());
        assert!(hierarchical_leaf_plan_extra_work(1_990_000, 100, 10, 8, 20).is_none());
    }

    #[test]
    fn hierarchical_plan_preserves_root_share_independent_of_leaf_count() {
        let groups = vec![(vec![0], 0.8222, vec![0]), (vec![1], 0.1778, vec![1, 2, 3])];
        let counts = hierarchical_allocation(&groups, &[100.0, 0.0, 0.0, 0.0], 100, 2).unwrap();
        assert_eq!(counts.iter().sum::<usize>(), 100);
        assert!(counts.iter().all(|n| *n >= 2));
        let excess_share = (counts[0] - 2) as f64 / (100 - 8) as f64;
        assert!((excess_share - 0.8222).abs() < 0.01);
        assert!((counts[1] as isize - counts[2] as isize).abs() <= 1);
        assert!((counts[2] as isize - counts[3] as isize).abs() <= 1);
        assert!(hierarchical_allocation(&[(vec![0], 1.0, vec![0, 0])], &[1.0], 4, 2).is_none());
    }

    #[test]
    fn hierarchical_zero_sigma_has_uniform_leaf_support_and_cost_admission() {
        let groups = vec![(vec![4], 1.0, vec![0, 1, 2])];
        let counts = hierarchical_allocation(&groups, &[0.0, 0.0, 0.0], 10, 2).unwrap();
        assert_eq!(counts.iter().sum::<usize>(), 10);
        assert!(counts.iter().all(|n| *n >= 2));
        assert!(counts.iter().max().unwrap() - counts.iter().min().unwrap() <= 1);
        let per_draw = [10, 20, 20];
        assert!(affordable_hierarchical_allocation(
            &groups,
            &[0.0, 0.0, 0.0],
            10,
            2,
            &per_draw,
            2,
            199,
        )
        .0
        .is_none());
        let (plan, work) =
            affordable_hierarchical_allocation(&groups, &[0.0, 0.0, 0.0], 10, 2, &per_draw, 2, 200);
        assert!(plan.is_some());
        assert!(work > 0);
        let (counts, forecast) = plan.unwrap();
        assert_eq!(counts.iter().sum::<usize>(), 6);
        assert_eq!(forecast, 200);
    }

    #[test]
    fn hierarchical_plan_reservations_charge_mapping_and_search_once() {
        let mut bank = budget::WorkBudget::new(550);
        let mapping = 300;
        let search_grant = 200;
        let search_actual = 100;
        let final_pair = 150;
        assert!(bank.reserve(mapping));
        assert!(bank.reserve(search_grant));
        assert!(bank.release(search_grant - search_actual));
        assert_eq!(bank.reserved, mapping + search_actual);
        assert!(bank.reserve(final_pair));
        assert_eq!(bank.reserved, mapping + search_actual + final_pair);
        assert_eq!(bank.reserved, bank.limit);
    }

    #[test]
    fn unequal_branch_sample_counts_do_not_reweight_combined_probability() {
        let branches = vec![
            BranchSample {
                result: Result {
                    samples: 2,
                    probability: 0.02,
                    ess: 2.0,
                    weighted: true,
                    ..Result::default()
                },
                left: 0.01,
                right: 0.01,
            },
            BranchSample {
                result: Result {
                    samples: 200,
                    probability: 0.03,
                    ess: 50.0,
                    weighted: true,
                    ..Result::default()
                },
                left: 0.015,
                right: 0.015,
            },
        ];
        assert!((combine_branches(&branches).probability - 0.05).abs() < 1e-12);
    }

    #[test]
    fn unequal_guided_pilots_preserve_tiny_fixture_mass_with_legacy_final_allocation() {
        let m = tiny_model();
        let cell = Cell { team: 0, rank: 2 };
        let (proposal, _) =
            BranchStrata::with_rank_rescue_tree(&m, cell, 808, true, true, true, true);
        let proposal = proposal.unwrap();
        let tree = proposal.setup_diagnostics()["tree"].clone();
        let pair_diagnostics = &tree["leaf_pilot"]["pair_pilot"];
        assert_eq!(pair_diagnostics["rolled_back"], false, "{pair_diagnostics}");
        assert!(
            pair_diagnostics["leaves"]
                .as_array()
                .unwrap()
                .iter()
                .any(|leaf| {
                    leaf["shared_pair_count"].as_u64().unwrap_or(0) > 0
                        && leaf["sum_log_pair_factor"].as_f64().unwrap_or(0.0).abs() > 1e-10
                }),
            "{pair_diagnostics}"
        );
        assert!(pair_diagnostics["adjusted_log_scores_valid"]
            .as_bool()
            .unwrap());
        let root_groups = proposal.leaf_pilot_root_groups().unwrap();
        let independent = proposal.leaf_pilot_independent_shares().unwrap();
        let (corrected_logs, selected_sets) = proposal.hierarchical_pair_pilot_data().unwrap();
        let hierarchy = hierarchical_pilot_plan(
            &root_groups,
            &independent,
            &corrected_logs,
            &selected_sets,
            128,
        )
        .unwrap();
        let pilot_counts = hierarchy.counts.clone();
        assert!(hierarchy
            .groups
            .iter()
            .any(|group| group.status == "redistributed"));
        assert_ne!(hierarchy.counts, hierarchy.original_counts);
        for (_, indexes) in &root_groups {
            assert_eq!(
                indexes.iter().map(|&i| pilot_counts[i]).sum::<usize>(),
                indexes
                    .iter()
                    .map(|&i| hierarchy.original_counts[i])
                    .sum::<usize>()
            );
        }
        assert!(pilot_counts.iter().any(|count| *count != 128));
        let mut kinds = Vec::with_capacity(proposal.len());
        let mut scores = Vec::with_capacity(proposal.len());
        for i in 0..proposal.len() {
            let rank = proposal.sample(
                &m,
                i,
                pilot_counts[i],
                derive(808, &format!("hierarchical-test-pilot-rank-{i}")),
                false,
                true,
            );
            let interval = proposal.sample(
                &m,
                i,
                pilot_counts[i],
                derive(808, &format!("hierarchical-test-pilot-interval-{i}")),
                true,
                true,
            );
            let interval_wins = interval_proposal_better(rank.result.ess, interval.result.ess);
            kinds.push(interval_wins);
            let selected = if interval_wins { &interval } else { &rank };
            scores.push(selected.result.std_err * (selected.result.samples as f64).sqrt());
        }
        let counts = allocation(&scores, 20_000, 2).unwrap();
        assert_eq!(counts.iter().sum::<usize>(), 20_000);
        assert!(counts.iter().all(|&n| n >= 2));
        let sample = |label: &str| {
            let branches = (0..proposal.len())
                .map(|i| {
                    proposal.sample(
                        &m,
                        i,
                        counts[i],
                        derive(909, &format!("hierarchical-test-{label}-{i}")),
                        kinds[i],
                        true,
                    )
                })
                .collect::<Vec<_>>();
            combine_branches(&branches)
        };
        let main = sample("main");
        let check = sample("check");
        let exact = exact_rank_mass(&m, cell);
        assert!((main.probability - exact).abs() < 6.0 * main.std_err + 0.002);
        assert!((check.probability - exact).abs() < 6.0 * check.std_err + 0.002);
    }
    #[test]
    fn adaptive_pilot_selects_higher_ess_and_ties_favor_rank() {
        assert!(interval_proposal_better(4., 5.));
        assert!(!interval_proposal_better(5., 5.));
        assert!(!interval_proposal_better(6., 5.));
        assert!(!ProposalKind::Rank.bound_guidance());
        assert!(ProposalKind::Interval.bound_guidance());
    }
    #[test]
    fn unaffordable_pair_is_declined_before_any_reservation() {
        let affordable = affordable_allocation(&[1., 3.], 100, 2, &[100, 200], 2, 100);
        assert!(affordable.is_none());
        let bank = budget::WorkBudget::new(100);
        assert_eq!(bank.reserved, 0);
    }
    #[test]
    fn positive_replacement_requires_both_quality_improvements() {
        let old = Estimate {
            probability: 0.2,
            ess: 10.,
            relative_se: Some(0.5),
            conditional_samples: 1,
            ..Estimate::default()
        };
        let mut weak = Result::default();
        weak.weighted = true;
        weak.probability = 0.3;
        weak.hits = 40;
        weak.ess = 20.;
        weak.std_err = 0.1;
        weak.max_share = 0.1;
        weak.batch_gap = 1.;
        let mut poor_check = weak.clone();
        poor_check.std_err = 0.18;
        assert!(!replacement_better(&old, &weak, &poor_check));
        let mut current = old.clone();
        let before = current.clone();
        assert!(!commit_candidate(
            &mut current,
            &old,
            &weak,
            &poor_check,
            true
        ));
        assert_eq!(current.probability, before.probability);
        assert_eq!(current.ess, before.ess);
        let old = Estimate {
            ess: 5.,
            relative_se: Some(0.8),
            ..old
        };
        let check = weak.clone();
        assert!(replacement_better(&old, &weak, &check));
    }
}
