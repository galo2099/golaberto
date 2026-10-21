//! Opt-in primary-tree WDL-path refinements.
use super::*;
#[cfg(any(test, feature = "test-support"))]
use crate::test_env as experiment_env;
#[cfg(not(any(test, feature = "test-support")))]
use std::env as experiment_env;

pub(super) const FLAG: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_HYBRID_TREE";
pub(super) const WORK_ENV: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_HYBRID_TREE_WORK";
pub(super) const LEAVES_ENV: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_HYBRID_TREE_LEAVES";
pub(super) const DEFAULT_WORK: usize = 8_000_000;
const LARGE_DEFAULT_WORK: usize = 128_000_000;
const MIN_WORK: usize = 8_000_000;
const MAX_WORK: usize = 512_000_000;
const PATH_LIMIT: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Paths,
    Mixed,
}

pub(super) fn mode() -> Option<Mode> {
    match experiment_env::var(FLAG).ok().as_deref() {
        Some("paths") => Some(Mode::Paths),
        Some("mixed") => Some(Mode::Mixed),
        _ => None,
    }
}

pub(super) fn work_cap(large: bool) -> usize {
    if !large {
        return DEFAULT_WORK;
    }
    let default = if large {
        LARGE_DEFAULT_WORK
    } else {
        DEFAULT_WORK
    };
    std::env::var(WORK_ENV)
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .map_or(default, |value| value.clamp(MIN_WORK, MAX_WORK))
}

pub(super) fn leaf_limit(large: bool, requested: usize) -> usize {
    if !large {
        return requested;
    }
    requested.max(
        std::env::var(LEAVES_ENV)
            .ok()
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(256)
            .clamp(256, 1024),
    )
}

pub(super) fn charge(work: &mut usize, cap: usize, fee: usize) -> bool {
    charge_with_denial_policy(
        work,
        cap,
        fee,
        experiment_env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_HYBRID_REPAIR").as_deref() == Ok("1"),
    )
}

fn charge_with_denial_policy(
    work: &mut usize,
    cap: usize,
    fee: usize,
    preserve_denied: bool,
) -> bool {
    match work.checked_add(fee) {
        Some(next) if next <= cap => {
            *work = next;
            true
        }
        _ => {
            if !preserve_denied {
                *work = cap;
            }
            false
        }
    }
}

#[cfg(test)]
mod budget_tests {
    use super::charge_with_denial_policy;

    #[test]
    fn denied_preview_reservations_preserve_repair_fallback_budget_only_when_opted_in() {
        let mut repaired = 7;
        assert!(!charge_with_denial_policy(&mut repaired, 10, 4, true));
        assert_eq!(repaired, 7);
        let mut legacy = 7;
        assert!(!charge_with_denial_policy(&mut legacy, 10, 4, false));
        assert_eq!(legacy, 10);
    }
}

/// Conservative budget for fixed-aware binary fallback work, bounded by the
/// remaining global tree node and guide-value limits.
pub(super) fn fallback_work_cap(
    builder: &LazyJoint,
    fixed_count: usize,
    children: usize,
    node_limit: usize,
    guide_limit: usize,
) -> usize {
    let g = builder.remaining.len();
    let n = builder.base.len();
    let per_pass = g.saturating_mul(4).saturating_add(n.saturating_mul(3));
    let one_propagation = g
        .saturating_mul(2)
        .saturating_add(1)
        .saturating_mul(per_pass)
        .saturating_add(g.saturating_mul(n).saturating_mul(4));
    let propagation = one_propagation
        .saturating_mul(3)
        .saturating_add(g.saturating_mul(3))
        .saturating_add(n.saturating_mul(3));
    let fixed_lookup = fixed_count
        .saturating_mul(2)
        .saturating_add(fixed_count.saturating_mul(g.saturating_add(n).saturating_add(1)))
        .saturating_add(builder.target.games.len())
        .saturating_add(n)
        .saturating_add(fixed_count.saturating_mul(g.saturating_add(1)))
        .saturating_add(g)
        .saturating_add(builder.target.games.len());
    let node_fee = node_limit.saturating_sub(builder.nodes).saturating_mul(16);
    let guide_fee = guide_limit
        .saturating_sub(builder.guide_values)
        .saturating_mul(2);
    propagation
        .saturating_mul(children)
        .saturating_add(node_fee)
        .saturating_add(guide_fee)
        .saturating_add(fixed_lookup.saturating_mul(children))
}

#[derive(Default)]
pub(super) struct Report {
    pub(super) mode: Option<&'static str>,
    pub(super) work: usize,
    pub(super) cap: usize,
    pub(super) group_splits: usize,
    pub(super) path_splits: usize,
    pub(super) depths: Vec<usize>,
    pub(super) forced_fixtures: usize,
    pub(super) rollbacks: usize,
    pub(super) fallbacks: usize,
    pub(super) denied_reservations: usize,
    pub(super) fallback_splits: usize,
    pub(super) fallback_work: usize,
    pub(super) initial_hint_work: usize,
    pub(super) refuted_paths: usize,
    pub(super) records: Vec<serde_json::Value>,
}

/// Allocation-only rectangle-envelope tightening proxy. This is not an event
/// probability bound: it measures how much each child's already-built packed
/// interval narrows relative to its parent, weighted by the existing guide
/// proxy. The caller compares path cost per added leaf against the binary group
/// alternative.
pub(super) fn tightening_score(parent: &Leaf, children: &[Leaf]) -> f64 {
    if children.is_empty() {
        return 0.0;
    }
    let mut total = 0.0;
    for (p, (&lo, &hi)) in parent
        .pattern
        .lower
        .iter()
        .zip(&parent.pattern.upper)
        .enumerate()
    {
        let plo = lo as i64;
        let phi = hi as i64;
        let width = phi - plo;
        if width <= 0 || width >= (i32::MAX as i64 / 4) {
            continue;
        }
        for child in children {
            let clo = child.pattern.lower[p] as i64;
            let chi = child.pattern.upper[p] as i64;
            let narrowed = (clo - plo).max(0) + (phi - chi).max(0);
            total += (narrowed as f64 / width as f64).clamp(0.0, 1.0);
        }
    }
    let denom = (parent.pattern.lower.len().max(1) * children.len()).max(1) as f64;
    let proxy_mass = children
        .iter()
        .map(|child| (child.pattern.mass * child.pattern.joint.residual_hint).max(0.0))
        .sum::<f64>();
    let proxy_mass = if proxy_mass.is_finite() && proxy_mass > 0.0 {
        proxy_mass
    } else {
        1.0
    };
    (total / denom) * proxy_mass
}

struct Context {
    remaining: Vec<crate::lookahead::RankGame>,
    fixed: Vec<(usize, u8)>,
    points: Vec<i32>,
    domains: Domains,
}

fn parent_context(
    builder: &LazyJoint,
    leaf: &Leaf,
    work: &mut usize,
    cap: usize,
) -> std::result::Result<Context, String> {
    let n = builder.base.len();
    if !charge(
        work,
        cap,
        leaf.pattern
            .fixed
            .len()
            .saturating_add(builder.target.games.len())
            .saturating_add(n),
    ) {
        return Err("hybrid work cap".into());
    }
    if leaf.pattern.lower.len() != n || leaf.pattern.upper.len() != n {
        return Err("parent interval vector unavailable".into());
    }
    let target_games = builder
        .target
        .games
        .iter()
        .map(|game| game.index)
        .collect::<std::collections::HashSet<_>>();
    let mut fixed_map = std::collections::BTreeMap::<usize, u8>::new();
    for &(index, outcome) in &leaf.pattern.fixed {
        if !charge(work, cap, 2) {
            return Err("hybrid work cap".into());
        }
        if target_games.contains(&index) {
            continue;
        }
        if let Some(old) = fixed_map.insert(index, outcome) {
            if old != outcome {
                return Err("conflicting inherited fixture outcomes".into());
            }
        }
    }
    let fixed = fixed_map.into_iter().collect::<Vec<_>>();
    let mut base = builder.base.clone();
    let mut fixed_prior = 1.0;
    for &(index, outcome) in &fixed {
        if !charge(
            work,
            cap,
            builder.remaining.len().saturating_add(n).saturating_add(1),
        ) {
            return Err("hybrid work cap".into());
        }
        let game = builder
            .remaining
            .iter()
            .find(|game| game.index == index)
            .ok_or("inherited fixed fixture absent from builder")?;
        let probability = game.prob[outcome as usize];
        if !probability.is_finite() || probability <= 0.0 {
            return Err("inherited fixed outcome has no original mass".into());
        }
        base[game.home] += game.hg[outcome as usize];
        base[game.away] += game.ag[outcome as usize];
        fixed_prior *= probability;
    }
    let fixed_indexes = fixed
        .iter()
        .map(|(i, _)| *i)
        .collect::<std::collections::HashSet<_>>();
    let remaining = builder
        .remaining
        .iter()
        .copied()
        .filter(|game| !fixed_indexes.contains(&game.index))
        .collect::<Vec<_>>();
    let g = remaining.len();
    let per_pass = g.saturating_mul(4).saturating_add(n.saturating_mul(3));
    let one_propagation = g
        .saturating_mul(2)
        .saturating_add(1)
        .saturating_mul(per_pass)
        .saturating_add(g.saturating_mul(n).saturating_mul(4));
    let propagation_fee = one_propagation
        .saturating_mul(3)
        .saturating_add(g.saturating_mul(3))
        .saturating_add(n.saturating_mul(3));
    if !charge(work, cap, propagation_fee) {
        return Err("hybrid work cap".into());
    }
    let mut points = base.clone();
    for (game, &outcome) in builder.target.games.iter().zip(&leaf.root) {
        points[game.home] += game.hg[outcome as usize];
        points[game.away] += game.ag[outcome as usize];
    }
    let rivals = (0..n)
        .filter(|&team| team != builder.cell.team)
        .collect::<Vec<_>>();
    let target = points[builder.cell.team];
    let mut domains = Domains::propagate(&remaining, &points, &rivals, builder.cell.rank, target);
    if !domains.feasible {
        return Err("parent propagation failed".into());
    }
    domains = Domains::condition(
        &remaining,
        &points,
        &rivals,
        builder.cell.rank,
        target,
        &leaf.pattern.lower,
        &leaf.pattern.upper,
        &domains.domains,
    );
    if !domains.feasible {
        return Err("parent interval is unsupported".into());
    }
    for team in rivals
        .iter()
        .copied()
        .filter(|&t| leaf.selected & (1 << t) != 0)
    {
        let strict = leaf.mask & (1 << team) != 0;
        if leaf.above == strict {
            domains.lower[team] = domains.lower[team].max(target + i32::from(strict));
        } else {
            domains.upper[team] = domains.upper[team].min(target - i32::from(strict));
        }
    }
    domains = Domains::condition(
        &remaining,
        &points,
        &rivals,
        builder.cell.rank,
        target,
        &domains.lower,
        &domains.upper,
        &domains.domains,
    );
    if !domains.feasible || !fixed_prior.is_finite() || fixed_prior <= 0.0 {
        return Err("parent case propagation failed".into());
    }
    Ok(Context {
        remaining,
        fixed,
        points,
        domains,
    })
}

fn build_with_fixed(
    builder: &mut LazyJoint,
    parent: &Leaf,
    case: (bool, u64, u64),
    new_fixed: &[(usize, u8)],
    work: &mut usize,
    cap: usize,
    charged_builder_work: &mut usize,
) -> std::result::Result<Option<Pattern>, String> {
    let mut context = parent_context(builder, parent, work, cap)?;
    let mut all_fixed = context
        .fixed
        .iter()
        .copied()
        .collect::<std::collections::BTreeMap<_, _>>();
    for &(index, outcome) in new_fixed {
        if let Some(old) = all_fixed.insert(index, outcome) {
            if old != outcome {
                return Err("path conflicts with inherited fixed result".into());
            }
        }
    }
    let target_indexes = builder
        .target
        .games
        .iter()
        .map(|game| game.index)
        .collect::<std::collections::HashSet<_>>();
    if !charge(
        work,
        cap,
        all_fixed
            .len()
            .saturating_mul(builder.remaining.len().saturating_add(1))
            .saturating_add(builder.remaining.len())
            .saturating_add(target_indexes.len()),
    ) {
        return Err("hybrid work cap".into());
    }
    let mut base = builder.base.clone();
    let mut fixed_prior = 1.0;
    for (&index, &outcome) in &all_fixed {
        if target_indexes.contains(&index) {
            return Err("extra fixed fixture overlaps root path".into());
        }
        let game = builder
            .remaining
            .iter()
            .find(|game| game.index == index)
            .ok_or("fixed fixture absent from builder")?;
        let probability = game.prob[outcome as usize];
        if !probability.is_finite() || probability <= 0.0 {
            return Err("fixed outcome has no original mass".into());
        }
        base[game.home] += game.hg[outcome as usize];
        base[game.away] += game.ag[outcome as usize];
        fixed_prior *= probability;
    }
    let fixed_indexes = all_fixed
        .keys()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let new_remaining = builder
        .remaining
        .iter()
        .copied()
        .filter(|g| !fixed_indexes.contains(&g.index))
        .collect();
    let original_base = std::mem::replace(&mut builder.base, base);
    let original_remaining = std::mem::replace(&mut builder.remaining, new_remaining);
    let old_nodes = builder.nodes;
    let old_guides = builder.guide_values;
    let old_node_limit = builder.node_limit;
    let old_guide_limit = builder.guide_limit;
    let phase_remaining = cap.saturating_sub(*work);
    builder.node_limit = old_nodes
        .saturating_add(phase_remaining / 32)
        .min(old_node_limit);
    builder.guide_limit = old_guides
        .saturating_add(phase_remaining / 4)
        .min(old_guide_limit);
    let child_result = builder.build_bounded(
        &parent.root,
        original_base.len(),
        None,
        Some(case),
        Some((&parent.pattern.lower, &parent.pattern.upper)),
        false,
    );
    builder.base = original_base;
    builder.remaining = original_remaining;
    builder.node_limit = old_node_limit;
    builder.guide_limit = old_guide_limit;
    let build_work = builder
        .nodes
        .saturating_sub(old_nodes)
        .saturating_mul(16)
        .saturating_add(
            builder
                .guide_values
                .saturating_sub(old_guides)
                .saturating_mul(2),
        );
    if !charge(work, cap, build_work) {
        return Err("hybrid work cap".into());
    }
    *charged_builder_work = charged_builder_work.saturating_add(build_work);
    context.fixed = all_fixed.into_iter().collect();
    match child_result {
        Ok(Some(mut pattern)) => {
            if !charge(
                work,
                cap,
                pattern.fixed.len().saturating_add(context.fixed.len()),
            ) {
                return Err("hybrid work cap".into());
            }
            pattern.mass *= fixed_prior;
            if !pattern.mass.is_finite() || pattern.mass <= 0.0 {
                return Err("child mass invalid after inherited fixed prior".into());
            }
            pattern.fixed.extend(context.fixed);
            pattern.fixed.sort_unstable();
            pattern.fixed.dedup();
            Ok(Some(pattern))
        }
        Ok(None) => Ok(None),
        Err(()) => Err("bounded child build exhausted".into()),
    }
}

type Path = (Vec<(usize, u8)>, f64);

fn best_paths(
    builder: &LazyJoint,
    context: &Context,
    ids: &[i32],
    work: &mut usize,
    cap: usize,
) -> std::result::Result<Option<(usize, Vec<Path>)>, String> {
    fn dfs(
        games: &[(crate::lookahead::RankGame, u8)],
        team: usize,
        i: usize,
        total: i32,
        lower: i32,
        upper: i32,
        suffix: &[(i32, i32)],
        path: &mut Vec<(usize, u8)>,
        mass: f64,
        out: &mut Vec<Path>,
        work: &mut usize,
        cap: usize,
    ) -> std::result::Result<(), String> {
        if !charge(work, cap, 1) {
            return Err("hybrid work cap".into());
        }
        if total + suffix[i].0 > upper || total + suffix[i].1 < lower {
            return Ok(());
        }
        if i == games.len() {
            out.push((path.clone(), mass));
            return Ok(());
        }
        let (game, mask) = games[i];
        let gains = if game.home == team { game.hg } else { game.ag };
        for outcome in 0..3 {
            if mask & (1 << outcome) == 0 || game.prob[outcome] <= 0.0 {
                continue;
            }
            if !charge(work, cap, 1) {
                return Err("hybrid work cap".into());
            }
            path.push((game.index, outcome as u8));
            dfs(
                games,
                team,
                i + 1,
                total + gains[outcome],
                lower,
                upper,
                suffix,
                path,
                mass * game.prob[outcome],
                out,
                work,
                cap,
            )?;
            path.pop();
            if out.len() > PATH_LIMIT {
                break;
            }
        }
        Ok(())
    }

    let mut rivals = (0..builder.base.len())
        .filter(|&t| t != builder.cell.team)
        .collect::<Vec<_>>();
    rivals.sort_by_key(|&t| ids[t]);
    let mut chosen: Option<(usize, Vec<Path>, f64)> = None;
    for team in rivals {
        let games = context
            .remaining
            .iter()
            .zip(&context.domains.domains)
            .filter(|(g, _)| g.home == team || g.away == team)
            .map(|(&g, &mask)| (g, mask))
            .collect::<Vec<_>>();
        if games.is_empty() || games.iter().all(|(_, mask)| mask.count_ones() == 1) {
            continue;
        }
        let mut suffix = vec![(0, 0); games.len() + 1];
        let mut domain_mass = 1.0;
        for i in (0..games.len()).rev() {
            if !charge(work, cap, 4) {
                return Err("hybrid work cap".into());
            }
            let (g, mask) = games[i];
            let gains = if g.home == team { g.hg } else { g.ag };
            let choices = (0..3)
                .filter(|&o| mask & (1 << o) != 0 && g.prob[o] > 0.0)
                .collect::<Vec<_>>();
            if choices.is_empty() {
                return Err("empty incident outcome domain".into());
            }
            suffix[i] = (
                suffix[i + 1].0 + choices.iter().map(|&o| gains[o]).min().unwrap(),
                suffix[i + 1].1 + choices.iter().map(|&o| gains[o]).max().unwrap(),
            );
            domain_mass *= choices.iter().map(|&o| g.prob[o]).sum::<f64>();
        }
        let mut paths = Vec::new();
        dfs(
            &games,
            team,
            0,
            context.points[team],
            context.domains.lower[team],
            context.domains.upper[team],
            &suffix,
            &mut Vec::new(),
            1.0,
            &mut paths,
            work,
            cap,
        )?;
        if paths.is_empty() || paths.len() > PATH_LIMIT || domain_mass <= 0.0 {
            continue;
        }
        let represented = paths.iter().map(|(_, p)| *p).sum::<f64>();
        let local_mass = represented / domain_mass;
        if chosen.as_ref().is_none_or(|(_, old, mass)| {
            paths.len() < old.len() || (paths.len() == old.len() && local_mass < *mass)
        }) {
            chosen = Some((team, paths, local_mass));
        }
    }
    Ok(chosen.map(|(team, paths, _)| (team, paths)))
}

pub(super) fn path_children(
    builder: &mut LazyJoint,
    parent: &Leaf,
    ids: &[i32],
    work: &mut usize,
    cap: usize,
    charged_builder_work: &mut usize,
) -> std::result::Result<Option<(usize, Vec<Leaf>, usize, usize)>, String> {
    let context = parent_context(builder, parent, work, cap)?;
    let Some((team, paths)) = best_paths(builder, &context, ids, work, cap)? else {
        return Ok(None);
    };
    let mut built = Vec::with_capacity(paths.len());
    let mut refuted = 0usize;
    for (fixed, path_mass) in &paths {
        if !charge(
            work,
            cap,
            context
                .remaining
                .len()
                .saturating_mul(builder.base.len().saturating_add(1)),
        ) {
            return Err("hybrid work cap".into());
        }
        match build_with_fixed(
            builder,
            parent,
            (parent.above, parent.mask, parent.selected),
            fixed,
            work,
            cap,
            charged_builder_work,
        )? {
            Some(pattern) => built.push((pattern, fixed.clone(), *path_mass)),
            None => refuted += 1,
        }
    }
    if built.is_empty() || built.len() > PATH_LIMIT {
        return Ok(Some((team, Vec::new(), paths.len(), refuted)));
    }
    let proxy_logs = built
        .iter()
        .map(|(pattern, _, _)| {
            if pattern.mass > 0.0 && pattern.joint.residual_hint > 0.0 {
                Some(pattern.mass.ln() + pattern.joint.residual_hint.ln())
            } else {
                None
            }
        })
        .collect::<Option<Vec<_>>>();
    let Some(proxy_logs) = proxy_logs else {
        return Ok(None);
    };
    let max = proxy_logs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut shares = proxy_logs
        .iter()
        .map(|x| (*x - max).exp())
        .collect::<Vec<_>>();
    let sum = shares.iter().sum::<f64>();
    if !sum.is_finite() || sum <= 0.0 {
        return Ok(None);
    }
    shares.iter_mut().for_each(|x| *x /= sum);
    let mut children = Vec::with_capacity(built.len());
    for (path_id, ((pattern, fixed, raw_prior), share)) in built.into_iter().zip(shares).enumerate()
    {
        if !share.is_finite() || share <= 0.0 {
            return Ok(None);
        }
        let mut child = parent.clone();
        child.pattern = pattern;
        child.frozen = false;
        child.score = 0.0;
        child.rank_hint_next_team = None;
        child.original_parent_id = 0;
        child.rival_team_id = Some(team);
        child.path_id = Some(path_id);
        child.rival_fixed = fixed;
        child.raw_path_prior = raw_prior;
        child.parent_proxy_log = None;
        child.child_proxy_log = None;
        child.local_child_share = share;
        child.guide_fraction *= share;
        child.hybrid_depth += 1;
        child.hybrid_path_count += 1;
        children.push(child);
    }
    Ok(Some((team, children, paths.len(), refuted)))
}

pub(super) fn next_team(
    builder: &LazyJoint,
    leaf: &Leaf,
    teams: usize,
    ids: &[i32],
    track_work: bool,
    cache: Option<&mut HashMap<super::MarginalKey, f64>>,
    cap: usize,
) -> (Option<usize>, f64, usize) {
    let mut work = 0usize;
    let Ok(context) = parent_context(builder, leaf, &mut work, cap) else {
        return (None, 0.0, work);
    };
    let target = context.points[builder.cell.team];
    let rivals = (0..teams)
        .filter(|&t| t != builder.cell.team)
        .collect::<Vec<_>>();
    let mut dist = vec![0.0; leaf.max_exceptions + 1];
    dist[0] = 1.0;
    let mut best: Option<(usize, f64)> = None;
    let mut cache = cache;
    for team in rivals {
        if leaf.selected & (1 << team) != 0 || leaf.ambiguous & (1 << team) == 0 {
            continue;
        }
        let lo =
            (context.points[team] + context.domains.min[0][team]).max(context.domains.lower[team]);
        let hi =
            (context.points[team] + context.domains.max[0][team]).min(context.domains.upper[team]);
        let cuts = if leaf.above {
            lo <= target && hi > target
        } else {
            lo < target && hi >= target
        };
        let games: Vec<crate::lookahead::RankGame> = context
            .remaining
            .iter()
            .zip(&context.domains.domains)
            .filter(|(g, _)| g.home == team || g.away == team)
            .map(|(&mut_game, &mask)| {
                let mut game = mut_game;
                let z = (0..3)
                    .filter(|&o| mask & (1 << o) != 0)
                    .map(|o| game.prob[o])
                    .sum::<f64>();
                game.prob = std::array::from_fn(|o| {
                    if mask & (1 << o) != 0 {
                        game.prob[o] / z
                    } else {
                        0.0
                    }
                });
                game
            })
            .collect();
        let span = games
            .iter()
            .map(|game| {
                let gains = if game.home == team { game.hg } else { game.ag };
                gains
                    .iter()
                    .map(|gain| (*gain).max(0) as usize)
                    .max()
                    .unwrap_or(0)
            })
            .sum::<usize>();
        let key = (
            team,
            leaf.above,
            target - context.points[team],
            games
                .iter()
                .map(|game| {
                    let mask = game
                        .prob
                        .iter()
                        .enumerate()
                        .fold(0u8, |mask, (o, p)| mask | if *p > 0.0 { 1 << o } else { 0 });
                    (game.index, mask)
                })
                .collect::<Vec<_>>(),
        );
        let key_work = games.len().saturating_add(1);
        let cache_hit = cache
            .as_deref()
            .is_some_and(|cache| cache.contains_key(&key));
        let preflight = context
            .remaining
            .len()
            .saturating_add(teams)
            .saturating_add(if cache_hit {
                key_work
            } else {
                key_work.saturating_add(games.len().saturating_mul(span.saturating_add(1)))
            });
        if !charge(&mut work, cap, preflight) {
            return (None, 0.0, work);
        }
        let (mass, marginal_work) = super::cached_marginal_mass(
            games,
            team,
            leaf.above,
            target - context.points[team],
            if track_work {
                cache.as_deref_mut()
            } else {
                None
            },
        );
        // The complete miss rectangle was reserved before entering the DP;
        // `marginal_work` is retained only as a consistency check signal.
        debug_assert!(marginal_work <= preflight);
        let allowed = leaf
            .max_exceptions
            .saturating_sub(leaf.mask.count_ones() as usize);
        for k in (0..=allowed).rev() {
            dist[k] = dist[k] * mass
                + if k > 0 {
                    dist[k - 1] * (1.0 - mass).max(0.0)
                } else {
                    0.0
                };
        }
        if cuts && best.is_none_or(|(old, p)| mass < p || (mass == p && ids[team] < ids[old])) {
            best = Some((team, mass));
        }
    }
    (
        best.map(|x| x.0),
        dist[..=leaf
            .max_exceptions
            .saturating_sub(leaf.mask.count_ones() as usize)]
            .iter()
            .sum(),
        work,
    )
}

pub(super) fn group_children(
    builder: &mut LazyJoint,
    parent: &Leaf,
    team: usize,
    work: &mut usize,
    cap: usize,
    charged_builder_work: &mut usize,
) -> std::result::Result<(Vec<Leaf>, usize), String> {
    let selected = parent.selected | (1 << team);
    let mut children = Vec::new();
    let mut empty = 0;
    for strict in [false, true] {
        if strict && parent.mask.count_ones() as usize >= parent.max_exceptions {
            empty += 1;
            continue;
        }
        let child_mask = parent.mask | if strict { 1 << team } else { 0 };
        let child_selected = if child_mask.count_ones() as usize == parent.max_exceptions {
            selected | parent.ambiguous
        } else {
            selected
        };
        match build_with_fixed(
            builder,
            parent,
            (parent.above, child_mask, child_selected),
            &[],
            work,
            cap,
            charged_builder_work,
        )? {
            Some(pattern) => {
                let mut child = parent.clone();
                child.mask = child_mask;
                child.selected = child_selected;
                child.pattern = pattern;
                child.frozen = false;
                child.score = 0.0;
                child.rank_hint_next_team = None;
                child.original_parent_id = 0;
                child.rival_team_id = None;
                child.path_id = None;
                child.rival_fixed.clear();
                child.raw_path_prior = 1.0;
                child.parent_proxy_log = None;
                child.child_proxy_log = None;
                child.local_child_share = 1.0;
                child.hybrid_depth += 1;
                child.hybrid_path_count = parent.hybrid_path_count;
                children.push(child);
            }
            None => empty += 1,
        }
    }
    Ok((children, empty))
}
