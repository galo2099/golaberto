//! Bounded complete partition: child failure retains the parent.
use super::*;
use crate::rng::derive;
use std::time::Instant;
#[cfg(test)]
use crate::test_env as experiment_env;
#[cfg(not(test))]
use std::env as experiment_env;
mod hybrid_tree;
mod na_guidance;
mod pair_pressure;
mod path_priority;
pub(super) mod rival_path;
mod tree_guidance;

#[derive(Clone)]
struct Leaf {
    root: Vec<u8>,
    root_index: usize,
    above: bool,
    mask: u32,
    selected: u32,
    ambiguous: u32,
    max_exceptions: usize,
    pattern: Pattern,
    frozen: bool,
    score: f64,
    rank_hint_next_team: Option<usize>,
    original_parent_id: usize,
    rival_team_id: Option<usize>,
    path_id: Option<usize>,
    rival_fixed: Vec<(usize, u8)>,
    raw_path_prior: f64,
    parent_proxy_log: Option<f64>,
    child_proxy_log: Option<f64>,
    local_child_share: f64,
    guide_fraction: f64,
    hybrid_depth: usize,
    hybrid_path_count: usize,
}

const DEFAULT_TREE_TARGET_PATH_LIMIT: usize = 64;
const MIN_TREE_TARGET_PATH_LIMIT: usize = 64;
const MAX_TREE_TARGET_PATH_LIMIT: usize = 1024;
const TREE_TARGET_PATH_LIMIT_ENV: &str = "RUST_ODDS_EXPERIMENT_TREE_TARGET_PATH_LIMIT";
const TREE_MARGINAL_CACHE_LIMIT: usize = 4096;
const OVERFLOW_TREE_NODE_LIMIT: usize = 100_000;
const OVERFLOW_TREE_GUIDE_LIMIT: usize = 4_000_000;
pub(crate) const PATH_PRIORITY_WORK_LIMIT: usize = 8_000_000;
pub(crate) const LEAF_PILOT_WORK_LIMIT: usize = 1_900_000;
pub(crate) const LEAF_PILOT_TOTAL_WORK_LIMIT: usize = 2_000_000;
pub(crate) const PAIR_PRESSURE_WORK_LIMIT: usize = 8_000_000;
type MarginalKey = (usize, bool, i32, Vec<(usize, u8)>);

fn bounded_tree_target_path_limit(value: Option<&str>) -> usize {
    value
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_TREE_TARGET_PATH_LIMIT)
        .clamp(MIN_TREE_TARGET_PATH_LIMIT, MAX_TREE_TARGET_PATH_LIMIT)
}

fn tree_target_path_limit() -> usize {
    bounded_tree_target_path_limit(std::env::var(TREE_TARGET_PATH_LIMIT_ENV).ok().as_deref())
}

fn keys(tab: &TerminalTable, team: usize) -> std::result::Result<Vec<Vec<u8>>, String> {
    keys_with_limit(tab, team, tree_target_path_limit())
}

fn keys_with_limit(
    tab: &TerminalTable,
    team: usize,
    limit: usize,
) -> std::result::Result<Vec<Vec<u8>>, String> {
    keys_with_limit_measured(tab, team, limit, false).0
}

fn keys_with_limit_measured(
    tab: &TerminalTable,
    team: usize,
    limit: usize,
    measure: bool,
) -> (std::result::Result<Vec<Vec<u8>>, String>, usize) {
    fn walk(
        tab: &TerminalTable,
        t: usize,
        i: usize,
        added: usize,
        limit: usize,
        measure: bool,
        work: &mut usize,
        key: &mut Vec<u8>,
        out: &mut Vec<Vec<u8>>,
    ) -> std::result::Result<(), String> {
        if measure {
            *work = work.saturating_add(1);
        }
        if i == tab.games.len() {
            out.push(key.clone());
            return Ok(());
        }
        let g = tab.games[i];
        let gains = if g.home == t { g.hg } else { g.ag };
        for o in 0..3 {
            if measure {
                *work = work.saturating_add(1);
            }
            let next = added + gains[o] as usize;
            if g.prob[o] > 0. && tab.rows[i + 1].get(next).is_some_and(|&p| p > 0.) {
                if out.len() >= limit {
                    return Err(if limit == DEFAULT_TREE_TARGET_PATH_LIMIT {
                        "complete tree target-path budget exhausted".into()
                    } else {
                        format!("complete tree target-path budget exhausted (limit {limit})")
                    });
                }
                key.push(o as u8);
                walk(tab, t, i + 1, next, limit, measure, work, key, out)?;
                key.pop();
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    let mut work = 0usize;
    let result = walk(
        tab,
        team,
        0,
        0,
        limit,
        measure,
        &mut work,
        &mut Vec::new(),
        &mut out,
    )
    .map(|()| out);
    (result, work)
}

/// Apply a root filter only after the complete, disjoint target-path cover has
/// been enumerated. A capped enumeration still fails as a whole; it never
/// returns the retained subset as if it were a complete cover.
fn filter_complete_roots(
    roots: Vec<Vec<u8>>,
    filter: &mut dyn FnMut(&[u8]) -> (bool, usize),
) -> (Vec<Vec<u8>>, usize) {
    let mut retained = Vec::with_capacity(roots.len());
    let mut work = 0usize;
    for root in roots {
        let (keep, charged) = filter(&root);
        work = work.saturating_add(charged);
        if keep {
            retained.push(root);
        }
    }
    (retained, work)
}

fn branch_ready(leaf: &Leaf, target_team: usize, teams: usize) -> bool {
    let rival_mask = if teams >= u32::BITS as usize {
        u32::MAX
    } else {
        (1u32 << teams) - 1
    } & !(1u32 << target_team);
    leaf.ambiguous & !leaf.selected & rival_mask == 0
}

fn comparable_group_shares(raw: &[f64], ready: &[bool]) -> Vec<f64> {
    if raw.is_empty() || raw.len() != ready.len() {
        return Vec::new();
    }
    let total = raw
        .iter()
        .filter(|weight| weight.is_finite() && **weight > 0.0)
        .sum::<f64>();
    let base = if total.is_finite() && total > 0.0 {
        raw.iter()
            .map(|weight| {
                if weight.is_finite() && *weight > 0.0 {
                    *weight / total
                } else {
                    0.0
                }
            })
            .collect::<Vec<_>>()
    } else {
        vec![1.0 / raw.len() as f64; raw.len()]
    };
    let ready_indexes = ready
        .iter()
        .enumerate()
        .filter_map(|(index, &is_ready)| is_ready.then_some(index))
        .collect::<Vec<_>>();
    let unresolved_indexes = ready
        .iter()
        .enumerate()
        .filter_map(|(index, &is_ready)| (!is_ready).then_some(index))
        .collect::<Vec<_>>();
    let mut comparable = vec![0.0; raw.len()];
    if !ready_indexes.is_empty() && !unresolved_indexes.is_empty() {
        let ready_mass = ready_indexes.iter().map(|&index| base[index]).sum::<f64>();
        for &index in &ready_indexes {
            comparable[index] = 0.90
                * if ready_mass > 0.0 {
                    base[index] / ready_mass
                } else {
                    1.0 / ready_indexes.len() as f64
                };
        }
        for &index in &unresolved_indexes {
            comparable[index] = 0.10 / unresolved_indexes.len() as f64;
        }
    } else {
        comparable.clone_from(&base);
    }
    comparable
        .iter()
        .map(|share| 0.90 * share + 0.10 / raw.len() as f64)
        .collect()
}

/// Comparable pilot weights keep unresolved/coarse branches in the proposal
/// with positive exploration while limiting their aggregate share when a
/// root also has rank-ready siblings. This is allocation guidance only and
/// never refutes a branch or changes the sampled event.
fn comparable_structural_shares(
    leaves: &[Leaf],
    roots: usize,
    base_shares: Option<&[f64]>,
    teams: usize,
    target_team: usize,
) -> (Vec<f64>, Vec<f64>, Vec<bool>, usize, usize) {
    let mut output = vec![0.0; leaves.len()];
    let mut proxies = vec![0.0; leaves.len()];
    let readiness = leaves
        .iter()
        .map(|leaf| branch_ready(leaf, target_team, teams))
        .collect::<Vec<_>>();
    let mut unresolved_roots = 0usize;
    let mut by_root = vec![Vec::new(); roots];
    for (index, leaf) in leaves.iter().enumerate() {
        if let Some(group) = by_root.get_mut(leaf.root_index) {
            group.push(index);
        }
    }
    for indexes in by_root {
        if indexes.is_empty() {
            continue;
        }
        let raw = indexes
            .iter()
            .map(|&index| {
                base_shares
                    .and_then(|shares| shares.get(index).copied())
                    .filter(|share| share.is_finite() && *share > 0.0)
                    .unwrap_or_else(|| leaves[index].score.max(1e-80))
            })
            .collect::<Vec<_>>();
        let unresolved_local = indexes
            .iter()
            .enumerate()
            .filter_map(|(position, &index)| (!readiness[index]).then_some(position))
            .collect::<Vec<_>>();
        if !unresolved_local.is_empty() {
            unresolved_roots += 1;
        }
        let ready_local = indexes
            .iter()
            .map(|&index| readiness[index])
            .collect::<Vec<_>>();
        let comparable = comparable_group_shares(&raw, &ready_local);
        for (position, &index) in indexes.iter().enumerate() {
            output[index] = comparable[position];
            proxies[index] = output[index];
        }
    }
    let work = leaves.len().saturating_mul(8).saturating_add(roots);
    (output, proxies, readiness, unresolved_roots, work)
}
fn stratum(l: &Leaf) -> Stratum {
    Stratum {
        root: l.root.clone(),
        exceptions: (0..32).filter(|&t| l.mask & (1 << t) != 0).collect(),
        above: l.above,
        secondary: None,
        pattern: l.pattern.clone(),
    }
}

fn operational_root_shares(leaves: &[Leaf], screen: &path_priority::Screen) -> Vec<f64> {
    let mut retained = vec![false; screen.roots.len()];
    for leaf in leaves {
        if leaf.root_index < retained.len() {
            retained[leaf.root_index] = true;
        }
    }
    let indexes: Vec<_> = retained
        .iter()
        .enumerate()
        .filter_map(|(i, &yes)| yes.then_some(i))
        .collect();
    let scores: Vec<_> = indexes.iter().map(|&i| screen.roots[i].raw_score).collect();
    let shares = path_priority::mix_shares(&scores, 0.10);
    let mut out = vec![0.0; screen.roots.len()];
    for (i, share) in indexes.into_iter().zip(shares) {
        out[i] = share;
    }
    out
}

fn split_priorities(leaves: &[Leaf], root_shares: &[f64]) -> Vec<f64> {
    let mut maxima = vec![0.0f64; root_shares.len()];
    let mut counts = vec![0usize; root_shares.len()];
    for leaf in leaves {
        if leaf.root_index < maxima.len() && leaf.score.is_finite() {
            maxima[leaf.root_index] = maxima[leaf.root_index].max(leaf.score.max(0.0));
            counts[leaf.root_index] += 1;
        }
    }
    let mut sums = vec![0.0f64; root_shares.len()];
    for leaf in leaves {
        if leaf.root_index < sums.len() && maxima[leaf.root_index] > 0.0 {
            sums[leaf.root_index] += leaf.score.max(0.0) / maxima[leaf.root_index];
        }
    }
    leaves
        .iter()
        .map(|leaf| {
            let r = leaf.root_index;
            if r >= root_shares.len() {
                return 0.0;
            }
            let count = counts[r].max(1) as f64;
            let local = if maxima[r] > 0.0 && sums[r] > 0.0 && leaf.score.is_finite() {
                (leaf.score.max(0.0) / maxima[r]) / sums[r]
            } else {
                1.0 / count
            };
            root_shares[r] * local
        })
        .collect()
}
fn train(
    m: &Model,
    cell: Cell,
    goals: &mut Option<crate::goal_tilt::GoalTilt>,
    l: &mut Leaf,
    seed: i64,
    serial: usize,
    n: usize,
) -> (usize, usize) {
    if n < 2 {
        l.score = l.pattern.mass * l.pattern.joint.residual_hint.max(1e-80);
        return (0, 0);
    }
    let p = BranchStrata {
        retuned: HashMap::new(),
        message_work: 0,
        cell,
        strata: vec![stratum(l)],
        goals: goals.take(),
        setup_nodes: 0,
        secondary_fallbacks: 0,
        secondary_nodes: 0,
        tree_training_work: 0,
        tree_diagnostics: None,
    };
    let a = p.sample(
        m,
        0,
        n,
        derive(seed, &format!("tree-training-rank-{serial}")),
        false,
        true,
    );
    let b = p.sample(
        m,
        0,
        n,
        derive(seed, &format!("tree-training-interval-{serial}")),
        true,
        true,
    );
    *goals = p.goals;
    l.score = a.result.std_err.max(b.result.std_err);
    (
        a.result.operations.units() + b.result.operations.units(),
        2 * n,
    )
}
fn next_team(builder: &LazyJoint, l: &Leaf, teams: usize) -> (Option<usize>, f64) {
    let (team, hint, _) = next_team_with_work(builder, l, teams, false, None);
    (team, hint)
}

fn next_team_with_work(
    builder: &LazyJoint,
    l: &Leaf,
    teams: usize,
    track_work: bool,
    marginal_cache: Option<&mut HashMap<MarginalKey, f64>>,
) -> (Option<usize>, f64, usize) {
    let mut estimated_work = if track_work {
        builder
            .remaining
            .len()
            .saturating_mul(teams.saturating_add(1))
    } else {
        0
    };
    let mut points = builder.base.clone();
    for (g, &o) in builder.target.games.iter().zip(&l.root) {
        points[g.home] += g.hg[o as usize];
        points[g.away] += g.ag[o as usize];
    }
    let target = points[builder.cell.team];
    let rivals: Vec<_> = (0..teams).filter(|&t| t != builder.cell.team).collect();
    let d = Domains::propagate(
        &builder.remaining,
        &points,
        &rivals,
        builder.cell.rank,
        target,
    );
    let (mut lower, mut upper) = (d.lower.clone(), d.upper.clone());
    for &t in &rivals {
        if l.selected & (1 << t) == 0 {
            continue;
        }
        let strict = l.mask & (1 << t) != 0;
        if l.above == strict {
            lower[t] = lower[t].max(target + i32::from(strict));
        } else {
            upper[t] = upper[t].min(target - i32::from(strict));
        }
    }
    let d = Domains::condition(
        &builder.remaining,
        &points,
        &rivals,
        builder.cell.rank,
        target,
        &lower,
        &upper,
        &d.domains,
    );
    if !d.feasible {
        return (None, 0., estimated_work);
    }
    let mut dist = vec![0.; l.max_exceptions + 1];
    dist[0] = 1.;
    let mut best: Option<(usize, f64)> = None;
    let mut marginal_cache = marginal_cache;
    for &t in &rivals {
        if l.selected & (1 << t) != 0 {
            continue;
        }
        let lo = (points[t] + d.min[0][t]).max(d.lower[t]);
        let hi = (points[t] + d.max[0][t]).min(d.upper[t]);
        let cuts = if l.above {
            lo <= target && hi > target
        } else {
            lo < target && hi >= target
        };
        if l.ambiguous & (1 << t) == 0 {
            continue;
        }
        let games: Vec<_> = builder
            .remaining
            .iter()
            .zip(&d.domains)
            .filter(|(g, _)| g.home == t || g.away == t)
            .map(|(&g, &mask)| {
                let mut g = g;
                let z = (0..3)
                    .filter(|&o| mask & (1 << o) != 0)
                    .map(|o| g.prob[o])
                    .sum::<f64>();
                g.prob = std::array::from_fn(|o| {
                    if mask & (1 << o) != 0 {
                        g.prob[o] / z
                    } else {
                        0.
                    }
                });
                g
            })
            .collect();
        let (mass, work) = cached_marginal_mass(
            games,
            t,
            l.above,
            target - points[t],
            if track_work {
                marginal_cache.as_deref_mut()
            } else {
                None
            },
        );
        estimated_work = estimated_work.saturating_add(work);
        let allowed = l
            .max_exceptions
            .saturating_sub(l.mask.count_ones() as usize);
        for k in (0..=allowed).rev() {
            dist[k] = dist[k] * mass
                + if k > 0 {
                    dist[k - 1] * (1. - mass).max(0.)
                } else {
                    0.
                };
        }
        // Ordering only: independent marginal rank hint is not a probability estimate or proof.
        if !cuts {
            continue;
        }
        // Ordering only: even numerical zero does not refute an outcome region.
        if best.is_none_or(|(b, p)| mass < p || (mass == p && t < b)) {
            best = Some((t, mass));
        }
    }
    (
        best.map(|(t, _)| t),
        dist[..=l
            .max_exceptions
            .saturating_sub(l.mask.count_ones() as usize)]
            .iter()
            .sum(),
        estimated_work,
    )
}

fn rank_priority_score(score: f64, hint: f64) -> f64 {
    score * hint.max(1e-80)
}

fn train_hybrid_children(
    m: &Model,
    cell: Cell,
    builder: &mut LazyJoint,
    children: &mut [Leaf],
    seed: i64,
    serial: &mut usize,
    pilot_n: usize,
    rank_priority: bool,
    measured: bool,
    mut marginal_cache: Option<&mut HashMap<MarginalKey, f64>>,
    training_ms: &mut f64,
    training_work: &mut usize,
    training_draws: &mut usize,
    rank_hint_work: &mut usize,
    report: &mut hybrid_tree::Report,
    hybrid_work: &mut usize,
    repair_hints: bool,
) {
    for child in children {
        let timer = Instant::now();
        let (work, draws) = train(m, cell, &mut builder.goals, child, seed, *serial, pilot_n);
        *serial += 1;
        *training_ms += timer.elapsed().as_secs_f64() * 1000.0;
        *training_work += work;
        *training_draws += draws;
        if rank_priority {
            let (next, hint, work) = if repair_hints {
                next_team_with_work(builder, child, m.n, true, marginal_cache.as_deref_mut())
            } else {
                hybrid_tree::next_team(
                    builder,
                    child,
                    m.n,
                    &m.ids,
                    measured,
                    marginal_cache.as_deref_mut(),
                    report.cap.saturating_sub(*hybrid_work),
                )
            };
            child.rank_hint_next_team = next;
            child.score = rank_priority_score(child.score, hint);
            *rank_hint_work = rank_hint_work.saturating_add(work);
            if repair_hints {
                report.initial_hint_work = report.initial_hint_work.saturating_add(work);
            } else {
                *hybrid_work = hybrid_work.saturating_add(work);
            }
            report.work = *hybrid_work;
        }
        report.depths.push(child.hybrid_depth);
    }
}

fn children_increment(child_count: usize) -> f64 {
    child_count.saturating_sub(1).max(1) as f64
}

fn fallback_group_team(
    model: &Model,
    builder: &LazyJoint,
    parent: &Leaf,
    target_team: usize,
    ids: &[i32],
) -> Option<usize> {
    if parent.rank_hint_next_team.is_some_and(|team| {
        team < model.n && parent.selected & (1 << team) == 0 && parent.ambiguous & (1 << team) != 0
    }) {
        return parent.rank_hint_next_team;
    }
    let mut points = builder.base.clone();
    for (game, &outcome) in builder.target.games.iter().zip(&parent.root) {
        points[game.home] += game.hg[outcome as usize];
        points[game.away] += game.ag[outcome as usize];
    }
    let target = points[target_team];
    (0..model.n)
        .filter(|&team| {
            team != target_team
                && parent.selected & (1 << team) == 0
                && parent.ambiguous & (1 << team) != 0
                && parent
                    .pattern
                    .lower
                    .get(team)
                    .is_some_and(|lo| *lo <= target)
                && parent
                    .pattern
                    .upper
                    .get(team)
                    .is_some_and(|hi| target <= *hi)
        })
        .max_by(|&a, &b| {
            let alo = parent.pattern.lower.get(a).copied().unwrap_or(i32::MAX);
            let ahi = parent.pattern.upper.get(a).copied().unwrap_or(i32::MIN);
            let blo = parent.pattern.lower.get(b).copied().unwrap_or(i32::MAX);
            let bhi = parent.pattern.upper.get(b).copied().unwrap_or(i32::MIN);
            (i64::from(ahi) - i64::from(alo))
                .cmp(&(i64::from(bhi) - i64::from(blo)))
                .then_with(|| ids[b].cmp(&ids[a]))
        })
}

fn accounted_tree_work(training_work: usize, rank_hint_work: usize, enabled: bool) -> usize {
    if enabled {
        training_work.saturating_add(rank_hint_work)
    } else {
        training_work
    }
}

fn rank_priority_enabled(pilot_n: usize) -> bool {
    pilot_n == 0 && std::env::var("RUST_ODDS_EXPERIMENT_TREE_RANK_PRIORITY").as_deref() == Ok("1")
}

fn cached_marginal_mass(
    games: Vec<crate::lookahead::RankGame>,
    team: usize,
    above: bool,
    threshold: i32,
    cache: Option<&mut HashMap<MarginalKey, f64>>,
) -> (f64, usize) {
    let Some(cache) = cache else {
        let (mass, _) = calculate_marginal_mass(games, team, above, threshold);
        return (mass, 0);
    };
    let key = (
        team,
        above,
        threshold,
        games
            .iter()
            .map(|game| {
                let mask =
                    game.prob
                        .iter()
                        .enumerate()
                        .fold(0u8, |mask, (outcome, &probability)| {
                            mask | if probability > 0. { 1 << outcome } else { 0 }
                        });
                (game.index, mask)
            })
            .collect::<Vec<_>>(),
    );
    let key_work = games.len().saturating_add(1);
    if let Some(&mass) = cache.get(&key) {
        return (mass, key_work);
    }
    let (mass, span) = calculate_marginal_mass(games, team, above, threshold);
    // A miss pays the complete allocated DP rectangle; a hit pays only key
    // construction and lookup. Cache growth is bounded and has no eviction.
    let work = key_work.saturating_add((key.3.len()).saturating_mul(span + 1));
    if cache.len() < TREE_MARGINAL_CACHE_LIMIT {
        cache.insert(key, mass);
    }
    (mass, work)
}

fn calculate_marginal_mass(
    games: Vec<crate::lookahead::RankGame>,
    team: usize,
    above: bool,
    threshold: i32,
) -> (f64, usize) {
    let span: usize = games
        .iter()
        .map(|game| {
            *if game.home == team {
                &game.hg
            } else {
                &game.ag
            }
            .iter()
            .max()
            .unwrap() as usize
        })
        .sum();
    let terminal = (0..=span)
        .map(|added| {
            if if above {
                added as i32 <= threshold
            } else {
                added as i32 >= threshold
            } {
                1.
            } else {
                0.
            }
        })
        .collect();
    let mass = TerminalTable::initial_mass(games, team, terminal);
    (mass, span)
}

fn modeled_tree_work(
    builder: &LazyJoint,
    bootstrap_work: usize,
    enumeration_work: usize,
    training_work: usize,
    rank_hint_work: usize,
    path_priority_work: usize,
) -> usize {
    crate::rare_tail::budget::setup_cost(builder.nodes, builder.guide_values)
        .saturating_add(bootstrap_work)
        .saturating_add(enumeration_work)
        .saturating_add(training_work)
        .saturating_add(rank_hint_work)
        .saturating_add(path_priority_work)
}

fn sync_modeled_tree_work(
    measured: bool,
    consumed: &mut usize,
    builder: &LazyJoint,
    bootstrap_work: usize,
    enumeration_work: usize,
    training_work: usize,
    rank_hint_work: usize,
    path_priority_work: usize,
) {
    if measured {
        *consumed = modeled_tree_work(
            builder,
            bootstrap_work,
            enumeration_work,
            training_work,
            rank_hint_work,
            path_priority_work,
        );
    }
}

fn lazy_bootstrap_work_estimate(m: &Model, cell: Cell, goal_tilt_requested: bool) -> usize {
    let teams = m.n;
    let fixtures = m.fixtures.len();
    let scan_fee = fixtures.saturating_add(teams).max(1);
    let rules = &m.request.phase.championship;
    if m.keys.first() != Some(&Key::Pt)
        || m.request.phase.bonus_points != 0
        || (rules.point_win, rules.point_draw, rules.point_loss) != (3, 1, 0)
        || m.ids.len() != teams
        || teams == 0
        || teams > 32
        || fixtures > 256
        || cell.team >= teams
        || m.fixtures.iter().any(|game| game.home == game.away)
    {
        return scan_fee;
    }
    let mut degree = vec![0usize; teams];
    for game in &m.fixtures {
        degree[game.home] = degree[game.home].saturating_add(1);
        degree[game.away] = degree[game.away].saturating_add(1);
    }
    let has_wins = m.keys.get(1) == Some(&Key::W);
    let stride = if has_wins {
        m.base
            .iter()
            .zip(&degree)
            .map(|(base, &left)| base.wins.saturating_add(left as i32))
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    } else {
        1
    };
    if stride <= 0
        || degree
            .iter()
            .any(|&left| i64::from(3 * stride + i32::from(has_wins)) * left as i64 > 4096)
        || degree[cell.team] == 0
        || m.base.iter().any(|base| {
            (i64::from(base.points).abs() + 3 * fixtures as i64 + 1) * i64::from(stride)
                + i64::from(base.wins)
                >= i64::from(i32::MAX / 4)
        })
    {
        return scan_fee;
    }
    let max_gain = (3 * stride + i32::from(has_wins)) as usize;
    let target_span = degree[cell.team].saturating_mul(max_gain).saturating_add(1);
    // Measured callers construct LazyJoint with roots=0. Bootstrap therefore
    // builds only the target terminal table (and optional goal tilt); root
    // training PMFs and rank hints are not constructed. These are logical
    // modeled units, not elapsed-time bounds.
    let mut work = fixtures
        .saturating_mul(teams.saturating_add(4))
        .saturating_add(teams.saturating_mul(teams.saturating_add(1)));
    work = work.saturating_add(
        degree[cell.team]
            .saturating_add(1)
            .saturating_mul(target_span)
            .saturating_add(
                degree[cell.team]
                    .saturating_mul(target_span)
                    .saturating_mul(3),
            ),
    );

    if goal_tilt_requested {
        let goal_supported = m.keys.first() == Some(&Key::Pt)
            && m.keys.get(if has_wins { 2 } else { 1 }) == Some(&Key::Gd)
            && !m
                .fixtures
                .iter()
                .any(|fixture| fixture.home_sampler.mean > 32. || fixture.away_sampler.mean > 32.);
        if goal_supported {
            for fixture in m
                .fixtures
                .iter()
                .filter(|fixture| fixture.home == cell.team || fixture.away == cell.team)
            {
                let support = |mean: f64| {
                    if mean <= 0. {
                        1
                    } else {
                        ((mean.ceil() as usize).saturating_mul(4).saturating_add(64)).min(1000)
                    }
                };
                work = work.saturating_add(
                    9usize
                        .saturating_mul(support(fixture.home_sampler.mean))
                        .saturating_mul(support(fixture.away_sampler.mean))
                        .saturating_mul(6),
                );
            }
        } else {
            work = work.saturating_add(scan_fee);
        }
    }
    if std::env::var("RUST_ODDS_LAZY_FALLBACK").as_deref() == Ok("guided") {
        let remaining_fixtures = fixtures.saturating_sub(degree[cell.team]);
        let max_remaining_span = degree
            .iter()
            .map(|&count| count.saturating_mul(max_gain as usize))
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        work = work
            .saturating_add(
                remaining_fixtures
                    .saturating_mul(2)
                    .saturating_add(1)
                    .saturating_mul(max_remaining_span)
                    .saturating_mul(2),
            )
            .saturating_add(teams.saturating_mul(teams));
    }
    work.max(scan_fee)
}
impl BranchStrata {
    pub(crate) const fn rank_rescue_path_priority_limit() -> usize {
        PATH_PRIORITY_WORK_LIMIT
    }
    pub(crate) const fn rank_rescue_leaf_pilot_limit() -> usize {
        LEAF_PILOT_TOTAL_WORK_LIMIT
    }
    pub(crate) const fn rank_rescue_pair_pressure_limit() -> usize {
        PAIR_PRESSURE_WORK_LIMIT
    }
    pub(crate) const fn rank_rescue_goal_tiebreak_limit() -> usize {
        rival_path::GOAL_TIEBREAK_WORK_LIMIT
    }
    pub(crate) const fn rank_rescue_shared_goal_tiebreak_limit() -> usize {
        rival_path::SHARED_GOAL_TIEBREAK_WORK_LIMIT
    }
    pub(crate) fn rank_rescue_hybrid_tree_work_limit(large_budget: bool) -> usize {
        hybrid_tree::work_cap(large_budget)
    }

    /// Fixed measured profile for the opt-in final rank-rescue stage.
    pub(crate) fn with_rank_rescue_tree(
        m: &Model,
        cell: Cell,
        seed: i64,
        path_priority: bool,
        leaf_pilot: bool,
        pair_pilot: bool,
        hierarchical_pair_pilot: bool,
    ) -> (std::result::Result<Self, String>, usize) {
        Self::with_rank_rescue_tree_experiment(
            m,
            cell,
            seed,
            path_priority,
            leaf_pilot,
            pair_pilot,
            hierarchical_pair_pilot,
            false,
            25,
        )
    }

    pub(crate) fn with_rank_rescue_tree_experiment(
        m: &Model,
        cell: Cell,
        seed: i64,
        path_priority: bool,
        leaf_pilot: bool,
        pair_pilot: bool,
        hierarchical_pair_pilot: bool,
        rival_path: bool,
        pilot_per_leaf: usize,
    ) -> (std::result::Result<Self, String>, usize) {
        Self::with_rank_rescue_tree_goal_tiebreak(
            m,
            cell,
            seed,
            path_priority,
            leaf_pilot,
            pair_pilot,
            hierarchical_pair_pilot,
            rival_path,
            pilot_per_leaf,
            false,
        )
    }

    pub(crate) fn with_rank_rescue_tree_goal_tiebreak(
        m: &Model,
        cell: Cell,
        seed: i64,
        path_priority: bool,
        leaf_pilot: bool,
        pair_pilot: bool,
        hierarchical_pair_pilot: bool,
        rival_path: bool,
        pilot_per_leaf: usize,
        goal_tiebreak: bool,
    ) -> (std::result::Result<Self, String>, usize) {
        Self::with_rank_rescue_tree_shared_goal_tiebreak(
            m,
            cell,
            seed,
            path_priority,
            leaf_pilot,
            pair_pilot,
            hierarchical_pair_pilot,
            rival_path,
            pilot_per_leaf,
            goal_tiebreak,
            false,
        )
    }

    pub(crate) fn with_rank_rescue_tree_shared_goal_tiebreak(
        m: &Model,
        cell: Cell,
        seed: i64,
        path_priority: bool,
        leaf_pilot: bool,
        pair_pilot: bool,
        hierarchical_pair_pilot: bool,
        rival_path: bool,
        pilot_per_leaf: usize,
        goal_tiebreak: bool,
        shared_goal_tiebreak: bool,
    ) -> (std::result::Result<Self, String>, usize) {
        Self::with_rank_rescue_tree_shared_goal_tiebreak_work_limit(
            m,
            cell,
            seed,
            path_priority,
            leaf_pilot,
            pair_pilot,
            hierarchical_pair_pilot,
            rival_path,
            pilot_per_leaf,
            goal_tiebreak,
            shared_goal_tiebreak,
            rival_path::SHARED_GOAL_TIEBREAK_WORK_LIMIT,
        )
    }

    pub(crate) fn with_rank_rescue_tree_shared_goal_tiebreak_work_limit(
        m: &Model,
        cell: Cell,
        seed: i64,
        path_priority: bool,
        leaf_pilot: bool,
        pair_pilot: bool,
        hierarchical_pair_pilot: bool,
        rival_path: bool,
        pilot_per_leaf: usize,
        goal_tiebreak: bool,
        shared_goal_tiebreak: bool,
        shared_goal_work_limit: usize,
    ) -> (std::result::Result<Self, String>, usize) {
        Self::with_tree_profile_options(
            m,
            cell,
            seed,
            4,
            256,
            0,
            40_000_000,
            100_000,
            256,
            true,
            true,
            path_priority,
            leaf_pilot,
            pair_pilot,
            hierarchical_pair_pilot,
            rival_path,
            pilot_per_leaf,
            goal_tiebreak,
            shared_goal_tiebreak,
            shared_goal_work_limit,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_rank_rescue_tree_shared_goal_tiebreak_work_limit_recovery(
        m: &Model,
        cell: Cell,
        seed: i64,
        path_priority: bool,
        leaf_pilot: bool,
        pair_pilot: bool,
        hierarchical_pair_pilot: bool,
        rival_path: bool,
        pilot_per_leaf: usize,
        goal_tiebreak: bool,
        shared_goal_tiebreak: bool,
        shared_goal_work_limit: usize,
        structural_recovery: bool,
        root_filter: &mut dyn FnMut(&[u8]) -> (bool, usize),
    ) -> (std::result::Result<Self, String>, usize) {
        Self::with_tree_profile_options_recovery(
            m,
            cell,
            seed,
            4,
            256,
            0,
            40_000_000,
            100_000,
            256,
            true,
            true,
            path_priority,
            leaf_pilot,
            pair_pilot,
            hierarchical_pair_pilot,
            rival_path,
            pilot_per_leaf,
            goal_tiebreak,
            shared_goal_tiebreak,
            shared_goal_work_limit,
            structural_recovery,
            Some(root_filter),
        )
    }

    pub fn tree_root_count(m: &Model, cell: Cell, seed: i64) -> usize {
        LazyJoint::with_mode(m, cell, seed, &[], 0, false, false, None)
            .and_then(|b| keys(&b.target, cell.team).ok())
            .map_or(usize::MAX, |v| v.len())
    }
    pub fn with_tree(
        m: &Model,
        cell: Cell,
        seed: i64,
        rivals: usize,
        leaf_limit: usize,
        pilot_n: usize,
        guide_limit: usize,
    ) -> std::result::Result<Self, String> {
        Self::with_tree_quota(
            m,
            cell,
            seed,
            rivals,
            leaf_limit,
            pilot_n,
            guide_limit,
            100000,
        )
    }
    fn with_tree_quota(
        m: &Model,
        cell: Cell,
        seed: i64,
        rivals: usize,
        leaf_limit: usize,
        pilot_n: usize,
        guide_limit: usize,
        split_quota: usize,
    ) -> std::result::Result<Self, String> {
        Self::with_tree_profile(
            m,
            cell,
            seed,
            rivals,
            leaf_limit,
            pilot_n,
            guide_limit,
            split_quota,
            tree_target_path_limit(),
            rank_priority_enabled(pilot_n),
            false,
        )
        .0
    }

    fn with_tree_profile(
        m: &Model,
        cell: Cell,
        seed: i64,
        rivals: usize,
        leaf_limit: usize,
        pilot_n: usize,
        guide_limit: usize,
        split_quota: usize,
        target_path_limit: usize,
        rank_priority: bool,
        measured: bool,
    ) -> (std::result::Result<Self, String>, usize) {
        Self::with_tree_profile_options(
            m,
            cell,
            seed,
            rivals,
            leaf_limit,
            pilot_n,
            guide_limit,
            split_quota,
            target_path_limit,
            rank_priority,
            measured,
            false,
            false,
            false,
            false,
            false,
            0,
            false,
            false,
            rival_path::SHARED_GOAL_TIEBREAK_WORK_LIMIT,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn with_tree_profile_options(
        m: &Model,
        cell: Cell,
        seed: i64,
        rivals: usize,
        leaf_limit: usize,
        pilot_n: usize,
        guide_limit: usize,
        split_quota: usize,
        target_path_limit: usize,
        rank_priority: bool,
        measured: bool,
        path_priority: bool,
        leaf_pilot: bool,
        pair_pilot: bool,
        hierarchical_pair_pilot: bool,
        rival_path: bool,
        pilot_per_leaf: usize,
        goal_tiebreak: bool,
        shared_goal_tiebreak: bool,
        shared_goal_work_limit: usize,
    ) -> (std::result::Result<Self, String>, usize) {
        Self::with_tree_profile_options_recovery(
            m,
            cell,
            seed,
            rivals,
            leaf_limit,
            pilot_n,
            guide_limit,
            split_quota,
            target_path_limit,
            rank_priority,
            measured,
            path_priority,
            leaf_pilot,
            pair_pilot,
            hierarchical_pair_pilot,
            rival_path,
            pilot_per_leaf,
            goal_tiebreak,
            shared_goal_tiebreak,
            shared_goal_work_limit,
            false,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn with_tree_profile_options_recovery(
        m: &Model,
        cell: Cell,
        seed: i64,
        rivals: usize,
        leaf_limit: usize,
        pilot_n: usize,
        guide_limit: usize,
        split_quota: usize,
        target_path_limit: usize,
        rank_priority: bool,
        measured: bool,
        path_priority: bool,
        leaf_pilot: bool,
        pair_pilot: bool,
        hierarchical_pair_pilot: bool,
        rival_path: bool,
        pilot_per_leaf: usize,
        goal_tiebreak: bool,
        shared_goal_tiebreak: bool,
        shared_goal_work_limit: usize,
        structural_recovery: bool,
        root_filter: Option<&mut dyn FnMut(&[u8]) -> (bool, usize)>,
    ) -> (std::result::Result<Self, String>, usize) {
        let mut consumed_work = 0usize;
        let result = Self::build_tree_profile(
            m,
            cell,
            seed,
            rivals,
            leaf_limit,
            pilot_n,
            guide_limit,
            split_quota,
            target_path_limit,
            rank_priority,
            measured,
            path_priority,
            leaf_pilot,
            pair_pilot,
            hierarchical_pair_pilot,
            rival_path,
            pilot_per_leaf,
            goal_tiebreak,
            shared_goal_tiebreak,
            shared_goal_work_limit,
            structural_recovery,
            root_filter,
            &mut consumed_work,
        );
        (result, consumed_work)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_tree_profile(
        m: &Model,
        cell: Cell,
        seed: i64,
        rivals: usize,
        leaf_limit: usize,
        pilot_n: usize,
        guide_limit: usize,
        split_quota: usize,
        target_path_limit: usize,
        rank_priority: bool,
        measured: bool,
        path_priority: bool,
        leaf_pilot: bool,
        pair_pilot: bool,
        hierarchical_pair_pilot: bool,
        rival_path: bool,
        pilot_per_leaf: usize,
        goal_tiebreak: bool,
        shared_goal_tiebreak: bool,
        shared_goal_work_limit: usize,
        structural_recovery: bool,
        root_filter: Option<&mut dyn FnMut(&[u8]) -> (bool, usize)>,
        consumed_work: &mut usize,
    ) -> std::result::Result<Self, String> {
        let clock = Instant::now();
        let hybrid_started = Instant::now();
        let hybrid_requested = hybrid_tree::mode();
        let large_budget =
            std::env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_LARGE_BUDGET").as_deref() == Ok("1");
        let hybrid_active = hybrid_requested.is_some()
            && measured
            && path_priority
            && leaf_pilot
            && pair_pilot
            && rival_path;
        let repair_requested =
            experiment_env::var("RUST_ODDS_EXPERIMENT_RANK_RESCUE_HYBRID_REPAIR").as_deref() == Ok("1");
        let tree_repair_active = repair_requested && hybrid_active;
        let leaf_limit = if hybrid_active {
            hybrid_tree::leaf_limit(large_budget, leaf_limit)
        } else {
            leaf_limit
        };
        let guide_limit = if hybrid_active && large_budget && leaf_limit > 256 {
            guide_limit.max(40_000_000)
        } else {
            guide_limit
        };
        let hybrid_cap = if hybrid_active {
            hybrid_tree::work_cap(large_budget)
        } else {
            hybrid_tree::DEFAULT_WORK
        };
        let tree_guide_requested = experiment_env::var(tree_guidance::FLAG).unwrap_or_default();
        let tree_guide_scope =
            tree_repair_active && m.ids.get(cell.team).copied() == Some(16) && cell.rank == 13;
        let mut tree_guide = tree_guidance::State::new(m, tree_guide_scope, large_budget);
        let tree_guide_setup_work = tree_guide.work;
        let mut hybrid_report = hybrid_tree::Report {
            mode: hybrid_requested.map(|mode| match mode {
                hybrid_tree::Mode::Paths => "paths",
                hybrid_tree::Mode::Mixed => "mixed",
            }),
            cap: hybrid_cap,
            ..Default::default()
        };
        let mut hybrid_work = 0usize;
        let goal_tilt_requested = crate::rare_tail::value("RUST_ODDS_LAZY_GOALS") == "1";
        let Some(mut builder) = LazyJoint::with_mode(m, cell, seed, &[], 0, false, false, None)
        else {
            if measured {
                *consumed_work = lazy_bootstrap_work_estimate(m, cell, goal_tilt_requested)
                    .saturating_add(tree_guide_setup_work);
            }
            return Err("unsupported tree model".into());
        };
        let bootstrap_work = if measured {
            lazy_bootstrap_work_estimate(m, cell, goal_tilt_requested)
        } else {
            0
        };
        builder.node_limit = OVERFLOW_TREE_NODE_LIMIT;
        builder.rival_limit = rivals.clamp(1, 6);
        builder.guide_limit = guide_limit;
        let overall_node_limit = builder.node_limit;
        let overall_guide_limit = builder.guide_limit;
        let (roots_result, enumeration_work) =
            keys_with_limit_measured(&builder.target, cell.team, target_path_limit, measured);
        let root_filter = if structural_recovery {
            root_filter
        } else {
            None
        };
        let (roots_result, filter_work) = match (roots_result, root_filter) {
            (Ok(roots), Some(filter)) => {
                let (roots, work) = filter_complete_roots(roots, filter);
                (Ok(roots), work)
            }
            (roots, _) => (roots, 0),
        };
        let enumeration_work = enumeration_work.saturating_add(filter_work);
        sync_modeled_tree_work(
            measured,
            consumed_work,
            &builder,
            bootstrap_work,
            enumeration_work,
            0,
            0,
            tree_guide_setup_work,
        );
        let roots = roots_result?;
        if roots.len() > leaf_limit {
            return Err("complete roots exceed leaf budget".into());
        }
        let screen_started = Instant::now();
        let schedule_work_reserve = 16usize
            .saturating_mul(leaf_limit)
            .saturating_mul(leaf_limit);
        let mut screen = if path_priority {
            Some(path_priority::screen(
                cell.team,
                cell.rank,
                &builder.base,
                &builder.target.games,
                &builder.remaining,
                &roots,
                PATH_PRIORITY_WORK_LIMIT.saturating_sub(schedule_work_reserve),
                leaf_pilot,
                LEAF_PILOT_WORK_LIMIT,
            ))
        } else {
            None
        };
        let screen_elapsed_ms = screen_started.elapsed().as_secs_f64() * 1000.0;
        let na_requested = na_guidance::enabled();
        let na_active =
            na_requested && measured && path_priority && leaf_pilot && pair_pilot && rival_path;
        let na_blend = na_guidance::blend();
        let na_cap = na_guidance::cap();
        let mut na_report = na_guidance::Report {
            enabled: na_active,
            blend: na_blend,
            cap: na_cap,
            reason: if !na_requested {
                Some("flag off".into())
            } else if !na_active {
                Some("measured rank-rescue gating not satisfied".into())
            } else if na_blend == 0.0 {
                Some("blend exponent is zero".into())
            } else {
                None
            },
            ..Default::default()
        };
        let na_started = Instant::now();
        let mut na_work = 0usize;
        if na_active && na_blend > 0.0 {
            if let Some(s) = screen.as_mut().filter(|s| s.reason.is_none()) {
                let games: Vec<_> = builder
                    .target
                    .games
                    .iter()
                    .chain(&builder.remaining)
                    .copied()
                    .collect();
                match na_guidance::blend_root_raw_scores(
                    m,
                    cell.rank,
                    &games,
                    s,
                    na_blend,
                    &mut na_work,
                    na_cap,
                ) {
                    Ok(rows) => {
                        na_report.root_applied = true;
                        na_report.root_rows = rows;
                    }
                    Err(error) => na_report.reason = Some(format!("root stage fallback: {error}")),
                }
            } else {
                na_report.reason = Some("root comparator screen unavailable".into());
            }
        }
        na_report.work = na_work;
        na_report.elapsed_ms = na_started.elapsed().as_secs_f64() * 1000.0;
        let mut path_priority_work = screen
            .as_ref()
            .map_or(0, |s| {
                s.work
                    .saturating_add(if s.reason.is_none() {
                        schedule_work_reserve
                    } else {
                        0
                    })
                    .saturating_add(if leaf_pilot { s.leaf_work } else { 0 })
            })
            .saturating_add(na_work)
            .saturating_add(tree_guide_setup_work);
        let mut leaves = Vec::new();
        let (mut training_work, mut training_draws, mut training_ms) = (0, 0, 0.);
        let mut rank_hint_work = 0usize;
        let mut marginal_cache = rank_priority.then(HashMap::<MarginalKey, f64>::new);
        let mut serial = 0;
        for (root_index, root) in roots.iter().enumerate() {
            let mut points = builder.base.clone();
            for (g, &o) in builder.target.games.iter().zip(root.iter()) {
                points[g.home] += g.hg[o as usize];
                points[g.away] += g.ag[o as usize];
            }
            let target = points[cell.team];
            let ts: Vec<_> = (0..m.n).filter(|&t| t != cell.team).collect();
            let d = Domains::propagate(&builder.remaining, &points, &ts, cell.rank, target);
            if !d.feasible {
                continue;
            }
            let above_count = ts
                .iter()
                .filter(|&&t| points[t] + d.min[0][t] > target)
                .count();
            let below_count = ts
                .iter()
                .filter(|&&t| points[t] + d.max[0][t] < target)
                .count();
            let above = cell.rank - above_count <= m.n - 1 - cell.rank - below_count;
            let max_exceptions = if above {
                cell.rank - above_count
            } else {
                m.n - 1 - cell.rank - below_count
            };
            let ambiguous = ts
                .iter()
                .filter(|&&t| {
                    points[t] + d.min[0][t] <= target && points[t] + d.max[0][t] >= target
                })
                .fold(0, |bits, &t| bits | (1 << t));
            let p = match builder.build(root, m.n, None, Some((above, 0, 0))) {
                Ok(pattern) => pattern,
                Err(()) => {
                    sync_modeled_tree_work(
                        measured,
                        consumed_work,
                        &builder,
                        bootstrap_work,
                        enumeration_work,
                        training_work,
                        rank_hint_work,
                        path_priority_work,
                    );
                    return Err("complete tree root setup exhausted".into());
                }
            };
            sync_modeled_tree_work(
                measured,
                consumed_work,
                &builder,
                bootstrap_work,
                enumeration_work,
                training_work,
                rank_hint_work,
                path_priority_work,
            );
            if let Some(pattern) = p {
                let mut l = Leaf {
                    root: root.clone(),
                    root_index,
                    above,
                    mask: 0,
                    selected: 0,
                    ambiguous,
                    max_exceptions,
                    pattern,
                    frozen: false,
                    score: 0.,
                    rank_hint_next_team: None,
                    original_parent_id: 0,
                    rival_team_id: None,
                    path_id: None,
                    rival_fixed: Vec::new(),
                    raw_path_prior: 1.0,
                    parent_proxy_log: None,
                    child_proxy_log: None,
                    local_child_share: 1.0,
                    guide_fraction: 1.0,
                    hybrid_depth: 0,
                    hybrid_path_count: 0,
                };
                let timer = Instant::now();
                let (w, n) = train(m, cell, &mut builder.goals, &mut l, seed, serial, pilot_n);
                serial += 1;
                training_ms += timer.elapsed().as_secs_f64() * 1000.;
                training_work += w;
                training_draws += n;
                if rank_priority {
                    let (next, hint, work) = if tree_repair_active {
                        next_team_with_work(&builder, &l, m.n, true, marginal_cache.as_mut())
                    } else if hybrid_active {
                        hybrid_tree::next_team(
                            &builder,
                            &l,
                            m.n,
                            &m.ids,
                            measured,
                            marginal_cache.as_mut(),
                            hybrid_cap.saturating_sub(hybrid_work),
                        )
                    } else {
                        next_team_with_work(&builder, &l, m.n, true, marginal_cache.as_mut())
                    };
                    l.rank_hint_next_team = next;
                    l.score = rank_priority_score(l.score, hint);
                    rank_hint_work = rank_hint_work.saturating_add(work);
                    if hybrid_active && !tree_repair_active {
                        hybrid_work = hybrid_work.saturating_add(work);
                        hybrid_report.work = hybrid_work;
                    }
                    if tree_repair_active {
                        hybrid_report.initial_hint_work =
                            hybrid_report.initial_hint_work.saturating_add(work);
                    }
                }
                sync_modeled_tree_work(
                    measured,
                    consumed_work,
                    &builder,
                    bootstrap_work,
                    enumeration_work,
                    training_work,
                    rank_hint_work,
                    path_priority_work,
                );
                leaves.push(l);
            }
        }
        builder.node_limit = 100000.min(builder.nodes.saturating_add(split_quota));
        let initial = leaves.len();
        let operational_shares = screen
            .as_ref()
            .filter(|s| s.reason.is_none())
            .map(|s| operational_root_shares(&leaves, s));
        let (mut splits, mut failed, mut refuted) = (0, 0, 0);
        let mut attempts = 0;
        while leaves.len() < leaf_limit && attempts < 2 * leaf_limit {
            let mut priorities = operational_shares
                .as_ref()
                .map(|shares| split_priorities(&leaves, shares));
            if tree_guide_requested == "mass" || tree_guide_requested == "entropy" {
                let baseline = priorities.clone().unwrap_or_else(|| {
                    path_priority::normalized(
                        &leaves
                            .iter()
                            .map(|leaf| leaf.score.max(0.0))
                            .collect::<Vec<_>>(),
                    )
                });
                let target_games = builder
                    .target
                    .games
                    .iter()
                    .map(|game| (game.home, game.away))
                    .collect::<Vec<_>>();
                let guide_work_before = tree_guide.work;
                if let Some(guided) =
                    tree_guide.frontier_weights(m, &target_games, &leaves, &baseline)
                {
                    priorities = Some(guided);
                }
                path_priority_work = path_priority_work
                    .saturating_add(tree_guide.work.saturating_sub(guide_work_before));
            }
            let chosen = leaves
                .iter()
                .enumerate()
                .filter(|(_, l)| !l.frozen)
                .max_by(|(a, x), (b, y)| {
                    priorities
                        .as_ref()
                        .map_or(x.score, |p| p[*a])
                        .total_cmp(&priorities.as_ref().map_or(y.score, |p| p[*b]))
                        .then(x.score.total_cmp(&y.score))
                        .then(x.pattern.mass.total_cmp(&y.pattern.mass))
                        .then(b.cmp(a))
                })
                .map(|(i, _)| i);
            let Some(index) = chosen else { break };
            if hybrid_active {
                attempts += 1;
                let parent = leaves[index].clone();
                // Preview the complete group partition first. Its measured
                // children are reused if selected; path preview then competes
                // against the same parent without changing tree coverage.
                let (mut team, _, team_work) = if tree_repair_active {
                    let (team, hint, work) =
                        next_team_with_work(&builder, &parent, m.n, true, marginal_cache.as_mut());
                    rank_hint_work = rank_hint_work.saturating_add(work);
                    hybrid_report.initial_hint_work =
                        hybrid_report.initial_hint_work.saturating_add(work);
                    (team, hint, work)
                } else {
                    hybrid_tree::next_team(
                        &builder,
                        &parent,
                        m.n,
                        &m.ids,
                        measured,
                        marginal_cache.as_mut(),
                        hybrid_cap.saturating_sub(hybrid_work),
                    )
                };
                let team_preview_work = if tree_repair_active { 0 } else { team_work };
                if tree_guide_requested == "entropy" {
                    let guide_work_before = tree_guide.work;
                    let compatible = tree_guide.compatible_for_leaf(&parent);
                    if let Some(entropy_team) = compatible.as_deref().and_then(|compatible| {
                        tree_guide.entropy_team(m, &parent, cell.team, compatible, &m.ids)
                    }) {
                        team = Some(entropy_team);
                    }
                    path_priority_work = path_priority_work
                        .saturating_add(tree_guide.work.saturating_sub(guide_work_before));
                }
                let mut group_trial_work = 0usize;
                let mut group_charged_builder_work = 0usize;
                let group_old_nodes = builder.nodes;
                let group_old_guides = builder.guide_values;
                let mut group_denied = false;
                let group_candidate = if let Some(team) = team {
                    match hybrid_tree::group_children(
                        &mut builder,
                        &parent,
                        team,
                        &mut group_trial_work,
                        hybrid_cap
                            .saturating_sub(hybrid_work)
                            .saturating_sub(team_preview_work),
                        &mut group_charged_builder_work,
                    ) {
                        Ok((children, refuted)) => Some((team, children, refuted)),
                        Err(error) => {
                            group_denied = error.contains("work cap");
                            None
                        }
                    }
                } else {
                    None
                };
                let group_build_cost = builder
                    .nodes
                    .saturating_sub(group_old_nodes)
                    .saturating_mul(16)
                    .saturating_add(
                        builder
                            .guide_values
                            .saturating_sub(group_old_guides)
                            .saturating_mul(2),
                    );
                hybrid_work = hybrid_work
                    .saturating_add(team_preview_work)
                    .saturating_add(group_trial_work);
                path_priority_work = path_priority_work
                    .saturating_add(team_preview_work)
                    .saturating_add(group_trial_work.saturating_sub(if tree_repair_active {
                        group_charged_builder_work
                    } else {
                        group_build_cost
                    }));
                let group_score = group_candidate.as_ref().map_or(0.0, |(_, children, _)| {
                    hybrid_tree::tightening_score(&parent, children)
                });
                let old_nodes = builder.nodes;
                let old_guides = builder.guide_values;
                let mut path_trial_work = 0usize;
                let mut path_charged_builder_work = 0usize;
                let path_attempt = hybrid_tree::path_children(
                    &mut builder,
                    &parent,
                    &m.ids,
                    &mut path_trial_work,
                    hybrid_cap.saturating_sub(hybrid_work),
                    &mut path_charged_builder_work,
                );
                let path_build_cost = builder
                    .nodes
                    .saturating_sub(old_nodes)
                    .saturating_mul(16)
                    .saturating_add(
                        builder
                            .guide_values
                            .saturating_sub(old_guides)
                            .saturating_mul(2),
                    );
                hybrid_work = hybrid_work.saturating_add(path_trial_work);
                path_priority_work = path_priority_work.saturating_add(
                    path_trial_work.saturating_sub(if tree_repair_active {
                        path_charged_builder_work
                    } else {
                        path_build_cost
                    }),
                );
                hybrid_report.work = hybrid_work;
                let path_candidate = match path_attempt {
                    Ok(Some((rival, children, path_count, refuted))) => {
                        hybrid_report.refuted_paths =
                            hybrid_report.refuted_paths.saturating_add(refuted);
                        Some((rival, children, path_count, refuted))
                    }
                    Ok(None) => None,
                    Err(reason) => {
                        hybrid_report.rollbacks += 1;
                        hybrid_report.fallbacks += 1;
                        if reason.contains("work cap") {
                            hybrid_report.denied_reservations += 1;
                        }
                        hybrid_report.records.push(json!({
                            "kind":"path_preview", "parent":index, "depth":parent.hybrid_depth,
                            "applied":false, "reason":reason, "work":path_trial_work,
                            "nodes":builder.nodes.saturating_sub(old_nodes),
                            "guide_values":builder.guide_values.saturating_sub(old_guides)
                        }));
                        None
                    }
                };
                let room = leaf_limit.saturating_sub(leaves.len()).saturating_add(1);
                let path_score = path_candidate.as_ref().map_or(0.0, |(_, children, _, _)| {
                    hybrid_tree::tightening_score(&parent, children)
                });
                let path_ok = path_candidate.as_ref().is_some_and(|(_, children, _, _)| {
                    !children.is_empty() && children.len() <= room
                });
                let choose_path = path_ok
                    && match hybrid_requested {
                        Some(hybrid_tree::Mode::Paths) => true,
                        Some(hybrid_tree::Mode::Mixed) => {
                            let path_per_leaf = path_score
                                / children_increment(path_candidate.as_ref().unwrap().1.len());
                            let group_viable = group_candidate
                                .as_ref()
                                .is_some_and(|(_, children, _)| !children.is_empty());
                            !group_viable || path_per_leaf > group_score
                        }
                        None => false,
                    };
                if group_denied {
                    hybrid_report.denied_reservations += 1;
                }
                hybrid_report.records.push(json!({
                    "kind":"split_preview", "parent":index, "root":parent.root,
                    "depth":parent.hybrid_depth, "group_team_id":team.map(|t|m.ids[t]),
                    "group_child_count":group_candidate.as_ref().map_or(0, |(_,c,_)|c.len()),
                    "group_refuted":group_candidate.as_ref().map_or(0, |(_,_,r)|*r),
                    "group_envelope_score_per_incremental_leaf":group_score,
                    "path_rival_id":path_candidate.as_ref().map(|(t,_,_,_)|m.ids[*t]),
                    "path_count":path_candidate.as_ref().map_or(0, |(_,_,n,_)|*n),
                    "path_child_count":path_candidate.as_ref().map_or(0, |(_,c,_,_)|c.len()),
                    "path_refuted":path_candidate.as_ref().map_or(0, |(_,_,_,r)|*r),
                    "path_envelope_score_per_incremental_leaf":path_score / path_candidate.as_ref().map_or(1.0, |(_,c,_,_)|children_increment(c.len())),
                    "score_kind":"pattern_interval_envelope_proxy",
                    "path_selected":choose_path, "work":team_preview_work + group_trial_work + path_trial_work,
                    "rank_hint_work":if tree_repair_active { team_work } else { 0 },
                    "nodes":builder.nodes.saturating_sub(old_nodes),
                    "guide_values":builder.guide_values.saturating_sub(old_guides)
                }));
                if choose_path {
                    let (rival, mut children, path_count, refuted_count) = path_candidate.unwrap();
                    train_hybrid_children(
                        m,
                        cell,
                        &mut builder,
                        &mut children,
                        seed,
                        &mut serial,
                        pilot_n,
                        rank_priority,
                        measured,
                        marginal_cache.as_mut(),
                        &mut training_ms,
                        &mut training_work,
                        &mut training_draws,
                        &mut rank_hint_work,
                        &mut hybrid_report,
                        &mut hybrid_work,
                        tree_repair_active,
                    );
                    refuted += refuted_count;
                    splits += 1;
                    hybrid_report.path_splits += 1;
                    hybrid_report.forced_fixtures = hybrid_report.forced_fixtures.saturating_add(
                        children
                            .iter()
                            .map(|child| child.rival_fixed.len())
                            .sum::<usize>(),
                    );
                    let parent_depth = parent.hybrid_depth;
                    hybrid_report.records.push(json!({"kind":"path_split","parent":index,
                        "depth":parent_depth,"rival_id":m.ids[rival],"path_count":path_count,
                        "refuted_paths":refuted_count,"children":children.iter().map(|child|json!({
                            "raw_path_prior_proxy":child.raw_path_prior,
                            "guide_fraction":child.guide_fraction,
                            "fixed":child.rival_fixed.iter().map(|(fixture,outcome)|json!({"fixture_index":fixture,"outcome":outcome})).collect::<Vec<_>>()
                        })).collect::<Vec<_>>(),"applied":true}));
                    leaves.remove(index);
                    leaves.splice(index..index, children);
                    sync_modeled_tree_work(
                        measured,
                        consumed_work,
                        &builder,
                        bootstrap_work,
                        enumeration_work,
                        training_work,
                        rank_hint_work,
                        path_priority_work,
                    );
                    continue;
                }
                if let Some((team, mut children, empty)) =
                    group_candidate.filter(|(_, c, _)| !c.is_empty())
                {
                    train_hybrid_children(
                        m,
                        cell,
                        &mut builder,
                        &mut children,
                        seed,
                        &mut serial,
                        pilot_n,
                        rank_priority,
                        measured,
                        marginal_cache.as_mut(),
                        &mut training_ms,
                        &mut training_work,
                        &mut training_draws,
                        &mut rank_hint_work,
                        &mut hybrid_report,
                        &mut hybrid_work,
                        tree_repair_active,
                    );
                    refuted += empty;
                    splits += 1;
                    hybrid_report.group_splits += 1;
                    hybrid_report.records.push(json!({"kind":"group_split","parent":index,
                        "team_id":m.ids[team],"depth":parent.hybrid_depth,"child_count":children.len(),
                        "refuted_children":empty,"work":group_trial_work,"applied":true}));
                    leaves.remove(index);
                    leaves.splice(index..index, children);
                    sync_modeled_tree_work(
                        measured,
                        consumed_work,
                        &builder,
                        bootstrap_work,
                        enumeration_work,
                        training_work,
                        rank_hint_work,
                        path_priority_work,
                    );
                    continue;
                }
                if tree_repair_active {
                    let fallback_team = team
                        .filter(|&team| {
                            team != cell.team
                                && parent.selected & (1 << team) == 0
                                && parent.ambiguous & (1 << team) != 0
                        })
                        .or_else(|| fallback_group_team(m, &builder, &parent, cell.team, &m.ids));
                    if let Some(fallback_team) = fallback_team {
                        let old_node_limit = builder.node_limit;
                        let old_guide_limit = builder.guide_limit;
                        builder.node_limit = overall_node_limit;
                        builder.guide_limit = overall_guide_limit;
                        let fallback_cap = hybrid_tree::fallback_work_cap(
                            &builder,
                            parent.pattern.fixed.len(),
                            2,
                            overall_node_limit,
                            overall_guide_limit,
                        );
                        let mut fallback_used = 0usize;
                        let mut fallback_charged_builder_work = 0usize;
                        let fallback = hybrid_tree::group_children(
                            &mut builder,
                            &parent,
                            fallback_team,
                            &mut fallback_used,
                            fallback_cap,
                            &mut fallback_charged_builder_work,
                        );
                        builder.node_limit = old_node_limit;
                        builder.guide_limit = old_guide_limit;
                        path_priority_work = path_priority_work.saturating_add(
                            fallback_used.saturating_sub(fallback_charged_builder_work),
                        );
                        hybrid_report.fallback_work =
                            hybrid_report.fallback_work.saturating_add(fallback_used);
                        match fallback {
                            Ok((mut children, empty)) if !children.is_empty() => {
                                train_hybrid_children(
                                    m,
                                    cell,
                                    &mut builder,
                                    &mut children,
                                    seed,
                                    &mut serial,
                                    pilot_n,
                                    rank_priority,
                                    measured,
                                    marginal_cache.as_mut(),
                                    &mut training_ms,
                                    &mut training_work,
                                    &mut training_draws,
                                    &mut rank_hint_work,
                                    &mut hybrid_report,
                                    &mut hybrid_work,
                                    tree_repair_active,
                                );
                                refuted += empty;
                                splits += 1;
                                hybrid_report.group_splits += 1;
                                hybrid_report.fallback_splits += 1;
                                hybrid_report.records.push(json!({"kind":"fallback_group_split",
                                    "parent":index,"team_id":m.ids[fallback_team],
                                    "depth":parent.hybrid_depth,"child_count":children.len(),
                                    "refuted_children":empty,"work":fallback_used,
                                    "inherited_fixed_fixture_count":parent.pattern.fixed.iter()
                                        .filter(|(fixture, _)| !builder.target.games.iter().any(|game| game.index == *fixture)).count(),
                                    "applied":true}));
                                leaves.remove(index);
                                leaves.splice(index..index, children);
                                sync_modeled_tree_work(
                                    measured,
                                    consumed_work,
                                    &builder,
                                    bootstrap_work,
                                    enumeration_work,
                                    training_work,
                                    rank_hint_work,
                                    path_priority_work,
                                );
                                continue;
                            }
                            result => {
                                let denied = result
                                    .as_ref()
                                    .is_err_and(|error| error.contains("work cap"));
                                if denied {
                                    hybrid_report.denied_reservations += 1;
                                }
                                hybrid_report
                                    .records
                                    .push(json!({"kind":"fallback_group_split",
                                    "parent":index,"team_id":m.ids[fallback_team],
                                    "depth":parent.hybrid_depth,"work":fallback_used,
                                    "applied":false,"reason":match result {
                                        Err(error) => error,
                                        Ok(_) => "no supported child cases".into(),
                                    }}));
                            }
                        }
                    }
                }
                hybrid_report.fallbacks += 1;
                leaves[index].frozen = true;
                failed += 1;
                continue;
            }
            let next = if rank_priority {
                leaves[index].rank_hint_next_team
            } else {
                next_team(&builder, &leaves[index], m.n).0
            };
            let Some(t) = next else {
                leaves[index].frozen = true;
                continue;
            };
            attempts += 1;
            let (root, above, mask, selected) = {
                let l = &leaves[index];
                (l.root.clone(), l.above, l.mask, l.selected | (1 << t))
            };
            let mut children = Vec::new();
            let mut failure = false;
            let mut empty = 0;
            for strict in [false, true] {
                if strict && mask.count_ones() as usize >= leaves[index].max_exceptions {
                    empty += 1;
                    continue;
                }
                let child_mask = mask | if strict { 1 << t } else { 0 };
                let selected = if child_mask.count_ones() as usize == leaves[index].max_exceptions {
                    selected | leaves[index].ambiguous
                } else {
                    selected
                };
                let build_result =
                    builder.build(&root, m.n, None, Some((above, child_mask, selected)));
                sync_modeled_tree_work(
                    measured,
                    consumed_work,
                    &builder,
                    bootstrap_work,
                    enumeration_work,
                    training_work,
                    rank_hint_work,
                    path_priority_work,
                );
                match build_result {
                    Ok(Some(pattern)) => children.push(Leaf {
                        root: root.clone(),
                        root_index: leaves[index].root_index,
                        above,
                        mask: child_mask,
                        selected,
                        ambiguous: leaves[index].ambiguous,
                        max_exceptions: leaves[index].max_exceptions,
                        pattern,
                        frozen: false,
                        score: 0.,
                        rank_hint_next_team: None,
                        original_parent_id: 0,
                        rival_team_id: None,
                        path_id: None,
                        rival_fixed: Vec::new(),
                        raw_path_prior: 1.0,
                        parent_proxy_log: None,
                        child_proxy_log: None,
                        local_child_share: 1.0,
                        guide_fraction: leaves[index].guide_fraction,
                        hybrid_depth: leaves[index].hybrid_depth + 1,
                        hybrid_path_count: leaves[index].hybrid_path_count,
                    }),
                    Ok(None) => empty += 1,
                    Err(()) => {
                        failure = true;
                        break;
                    }
                }
            }
            if failure {
                leaves[index].frozen = true;
                failed += 1;
                continue;
            }
            // Both children together cover the parent. Replace only after both
            // completed or were refuted by necessary domain propagation.
            refuted += empty;
            splits += 1;
            for l in &mut children {
                let timer = Instant::now();
                let (w, n) = train(m, cell, &mut builder.goals, l, seed, serial, pilot_n);
                serial += 1;
                training_ms += timer.elapsed().as_secs_f64() * 1000.;
                training_work += w;
                training_draws += n;
                if rank_priority {
                    let (next, hint, work) =
                        next_team_with_work(&builder, l, m.n, true, marginal_cache.as_mut());
                    l.rank_hint_next_team = next;
                    l.score = rank_priority_score(l.score, hint);
                    rank_hint_work = rank_hint_work.saturating_add(work);
                }
                sync_modeled_tree_work(
                    measured,
                    consumed_work,
                    &builder,
                    bootstrap_work,
                    enumeration_work,
                    training_work,
                    rank_hint_work,
                    path_priority_work,
                );
            }
            leaves.remove(index);
            leaves.splice(index..index, children);
        }
        if leaves.is_empty() {
            sync_modeled_tree_work(
                measured,
                consumed_work,
                &builder,
                bootstrap_work,
                enumeration_work,
                training_work,
                rank_hint_work,
                path_priority_work,
            );
            return Err("tree has no usable leaves; not a cell impossibility claim".into());
        }
        let mut pair_pressure_result = None;
        let mut pair_pressure_elapsed_ms = 0.0;
        let (mut leaf_pilot_shares, leaf_pilot_work, leaf_pilot_reason) = if leaf_pilot {
            let mut used = screen.as_ref().map_or(0, |s| s.leaf_work);
            let mut reason = screen.as_ref().and_then(|s| s.leaf_reason);
            let mut log_scores = vec![f64::NEG_INFINITY; leaves.len()];
            if reason.is_none() {
                if let Some(comparators) = screen.as_ref().and_then(|s| s.leaf_comparators.as_ref())
                {
                    for (i, leaf) in leaves.iter().enumerate() {
                        let Some(root_comparators) = comparators.get(leaf.root_index) else {
                            reason = Some("missing root comparator PMFs");
                            break;
                        };
                        match path_priority::leaf_log_mass(
                            root_comparators,
                            cell.rank,
                            leaf.above,
                            leaf.mask,
                            leaf.selected,
                            cell.team,
                            &mut used,
                            LEAF_PILOT_WORK_LIMIT,
                        ) {
                            Some(score) => {
                                log_scores[i] = if hybrid_active {
                                    if leaf.guide_fraction > 0.0 && leaf.guide_fraction.is_finite()
                                    {
                                        score + leaf.guide_fraction.ln()
                                    } else {
                                        reason = Some("invalid inherited hybrid guide fraction");
                                        break;
                                    }
                                } else {
                                    score
                                };
                            }
                            None => {
                                reason = Some("leaf score work cap");
                                break;
                            }
                        }
                    }
                } else {
                    reason = Some("root comparator PMFs unavailable");
                }
            }
            if pair_pilot {
                if reason.is_none() {
                    let pair_started = Instant::now();
                    let independent = log_scores.clone();
                    let pair_result = pair_pressure::adjust(
                        cell.team,
                        &builder.base,
                        &builder.target.games,
                        &builder.remaining,
                        &roots,
                        screen.as_ref().and_then(|s| s.leaf_comparators.as_deref()),
                        &leaves,
                        &independent,
                        PAIR_PRESSURE_WORK_LIMIT,
                    );
                    pair_pressure_elapsed_ms = pair_started.elapsed().as_secs_f64() * 1000.0;
                    if let Some(adjusted) = pair_result.adjusted_log_scores.as_ref() {
                        log_scores.clone_from(adjusted);
                    }
                    path_priority_work = path_priority_work.saturating_add(pair_result.work);
                    pair_pressure_result = Some(pair_result);
                } else {
                    pair_pressure_result = Some(pair_pressure::unavailable(
                        "independent leaf scores unavailable",
                        leaves.len(),
                    ));
                }
            }
            let mut shares = vec![0.0; leaves.len()];
            let pair_shares = pair_pressure_result
                .as_ref()
                .and_then(|result| result.adjusted_shares.as_ref());
            if pair_pilot
                && pair_pressure_result
                    .as_ref()
                    .is_some_and(|r| r.adjusted_log_scores.is_some())
            {
                shares.clone_from(pair_shares.expect("successful pair pass provides shares"));
            }
            let mut indexes_by_root = vec![Vec::new(); roots.len()];
            if reason.is_none() && !(pair_pilot && pair_shares.is_some()) {
                for (i, leaf) in leaves.iter().enumerate() {
                    if !path_priority::increment(&mut used, LEAF_PILOT_WORK_LIMIT, 1) {
                        reason = Some("leaf grouping work cap");
                        break;
                    }
                    if let Some(indexes) = indexes_by_root.get_mut(leaf.root_index) {
                        indexes.push(i);
                    } else {
                        reason = Some("invalid retained root index");
                        break;
                    }
                }
            }
            if reason.is_none() && !(pair_pilot && pair_shares.is_some()) {
                for indexes in indexes_by_root {
                    if indexes.is_empty() {
                        continue;
                    }
                    if !path_priority::increment(&mut used, LEAF_PILOT_WORK_LIMIT, indexes.len()) {
                        reason = Some("leaf share normalization work cap");
                        break;
                    }
                    let local = path_priority::normalized_log_shares(
                        &indexes.iter().map(|&i| log_scores[i]).collect::<Vec<_>>(),
                    );
                    for (index, share) in indexes.into_iter().zip(local) {
                        shares[index] = share;
                    }
                }
            }
            let previously_accounted = screen.as_ref().map_or(0, |s| s.leaf_work);
            path_priority_work =
                path_priority_work.saturating_add(used.saturating_sub(previously_accounted));
            (Some(shares), used, reason)
        } else {
            (None, 0, None)
        };
        for (index, leaf) in leaves.iter_mut().enumerate() {
            leaf.original_parent_id = index;
            // Secondary refinement treats every current primary leaf as a
            // fresh parent; its root-level guide fraction already carries
            // primary path allocation, while local_child_share is normalized
            // within this new parent group.
            leaf.local_child_share = 1.0;
        }
        let mut rival_path_report = rival_path::Report::default();
        let mut goal_tiebreak_diag = json!({"enabled":goal_tiebreak,"applied":false,"reason":if goal_tiebreak {Some("rival partition unavailable")} else {Some("flag off or gating condition not met")},"work":0,"cap":rival_path::GOAL_TIEBREAK_WORK_LIMIT,"query_cache_hits":0,"kernel_cache_hits":0,"query_rows":[]});
        let mut shared_goal_tiebreak_diag = json!({"enabled":shared_goal_tiebreak,"applied":false,"reason":if shared_goal_tiebreak {Some("rival partition unavailable")} else {Some("flag off or gating condition not met")},"work":0,"cap":shared_goal_work_limit,"query_cache_hits":0,"kernel_cache_hits":0,"query_rows":[]});
        if rival_path {
            rival_path_report.enabled = true;
            if let Some((Some(base_independent), Some(base_corrected))) =
                pair_pressure_result.as_ref().map(|result| {
                    (
                        result.independent_shares.as_ref(),
                        result.adjusted_log_scores.as_ref(),
                    )
                })
            {
                let mut independent_logs = base_independent
                    .iter()
                    .map(|share| {
                        if *share > 0.0 {
                            share.ln()
                        } else {
                            f64::NEG_INFINITY
                        }
                    })
                    .collect::<Vec<_>>();
                let mut corrected_logs = base_corrected.clone();
                rival_path_report = rival_path::apply(
                    m,
                    cell,
                    &mut builder,
                    screen.as_ref(),
                    &mut leaves,
                    &mut independent_logs,
                    &mut corrected_logs,
                    base_independent,
                    pilot_per_leaf,
                );
                if na_active
                    && na_report.root_applied
                    && rival_path_report.applied
                    && na_blend > 0.0
                {
                    let before = corrected_logs.clone();
                    let leaf_started = Instant::now();
                    let leaf_cap = na_cap.saturating_sub(na_work);
                    let mut leaf_work = 0usize;
                    match na_guidance::blend_leaf_corrected_logs(
                        m,
                        cell.team,
                        cell.rank,
                        &leaves,
                        &mut corrected_logs,
                        na_blend,
                        &mut leaf_work,
                        leaf_cap,
                    ) {
                        Ok((rows, hits)) => {
                            na_report.leaf_applied = true;
                            na_report.leaf_rows = rows;
                            na_report.cache_hits = na_report.cache_hits.saturating_add(hits);
                            na_report.work = na_report.work.saturating_add(leaf_work);
                            path_priority_work = path_priority_work.saturating_add(leaf_work);
                            na_report.elapsed_ms += leaf_started.elapsed().as_secs_f64() * 1000.0;
                            for row in &mut na_report.leaf_rows {
                                if let Some(i) = row["leaf_index"].as_u64().map(|x| x as usize) {
                                    row["before_goal_log_score"] = before
                                        .get(i)
                                        .copied()
                                        .map_or(serde_json::Value::Null, |x| json!(x));
                                    row["after_na_log_score"] = corrected_logs
                                        .get(i)
                                        .copied()
                                        .map_or(serde_json::Value::Null, |x| json!(x));
                                }
                            }
                        }
                        Err(error) => {
                            corrected_logs = before;
                            na_report.work = na_report.work.saturating_add(leaf_work);
                            path_priority_work = path_priority_work.saturating_add(leaf_work);
                            na_report.reason = Some(format!("leaf stage rollback: {error}"));
                            na_report.elapsed_ms += leaf_started.elapsed().as_secs_f64() * 1000.0;
                        }
                    }
                } else if na_active && na_report.root_applied {
                    na_report.reason = Some(
                        "leaf NA guidance skipped because rival-path partition was unavailable"
                            .into(),
                    );
                }
                if shared_goal_tiebreak && goal_tiebreak {
                    goal_tiebreak_diag = json!({"enabled":true,"applied":false,"reason":"replaced by shared goal-tiebreak stage","work":0,"cap":rival_path::GOAL_TIEBREAK_WORK_LIMIT,"query_cache_hits":0,"kernel_cache_hits":0,"query_rows":[]});
                    let (diagnostics, work) = rival_path::apply_shared_goal_tiebreak_with_cap(
                        m,
                        cell,
                        &leaves,
                        &mut corrected_logs,
                        screen.as_ref(),
                        shared_goal_work_limit,
                    );
                    shared_goal_tiebreak_diag = diagnostics;
                    path_priority_work = path_priority_work.saturating_add(work);
                } else if goal_tiebreak && rival_path_report.applied {
                    let (diagnostics, work) = rival_path::apply_goal_tiebreak(
                        m,
                        cell,
                        &leaves,
                        &mut corrected_logs,
                        screen.as_ref(),
                    );
                    goal_tiebreak_diag = diagnostics;
                    path_priority_work = path_priority_work.saturating_add(work);
                }
                if rival_path_report.applied || shared_goal_tiebreak_diag["applied"] == true {
                    let local_shares = |logs: &[f64]| {
                        let mut by_root = vec![Vec::new(); roots.len()];
                        for (i, leaf) in leaves.iter().enumerate() {
                            by_root[leaf.root_index].push(i);
                        }
                        let mut output = vec![0.0; leaves.len()];
                        for indexes in by_root {
                            if !indexes.is_empty() {
                                let normalized = path_priority::normalized_log_shares(
                                    &indexes.iter().map(|&i| logs[i]).collect::<Vec<_>>(),
                                );
                                for (&index, share) in indexes.iter().zip(normalized) {
                                    output[index] = share;
                                }
                            }
                        }
                        output
                    };
                    let independent_local = local_shares(&independent_logs);
                    let adjusted_local = local_shares(&corrected_logs);
                    if let Some(result) = pair_pressure_result.as_mut() {
                        let old_rows = result.leaves.clone();
                        result.adjusted_log_scores = Some(corrected_logs.clone());
                        result.independent_shares = Some(independent_local);
                        result.adjusted_shares = Some(adjusted_local.clone());
                        result.leaves = leaves
                            .iter()
                            .map(|leaf| old_rows[leaf.original_parent_id].clone())
                            .collect();
                    }
                    leaf_pilot_shares = Some(adjusted_local);
                }
            } else {
                rival_path_report.reason = Some("pair-pilot corrected guide unavailable");
                rival_path_report.original_leaf_count = leaves.len();
            }
            path_priority_work = path_priority_work.saturating_add(rival_path_report.scan_work);
        }
        let structural_recovery_diag = if structural_recovery {
            let (shares, proxies, readiness, unresolved_roots, work) = comparable_structural_shares(
                &leaves,
                roots.len(),
                leaf_pilot_shares.as_deref(),
                m.n,
                cell.team,
            );
            path_priority_work = path_priority_work.saturating_add(work);
            // The comparable family becomes the allocation fallback. The
            // independent and corrected pilot score rows remain available in
            // their diagnostics and no goal/pair correction is discarded.
            leaf_pilot_shares = Some(shares.clone());
            Some(json!({
                "enabled": true,
                "applied": true,
                "unresolved_roots": unresolved_roots,
                "work_units": work,
                "work_semantics": "bounded leaf grouping and comparable-share normalization",
                "comparable_family": "within each target root, ready branches retain 90% in corrected-score proportions and unresolved branches share 10%; a 10% uniform mixture preserves positive exploration",
                "proxy_semantics": "allocation-only structural proxy; it never refutes or removes a branch",
                "shares": shares,
                "leaves": leaves.iter().enumerate().map(|(index, leaf)| json!({
                    "index": index,
                    "root_index": leaf.root_index,
                    "structural_proxy_share": proxies[index],
                    "branch_readiness": if readiness[index] { "ready" } else { "unresolved" },
                    "unresolved_ambiguous_rivals": (0..m.n)
                        .filter(|&team| team != cell.team
                            && leaf.ambiguous & (1 << team) != 0
                            && leaf.selected & (1 << team) == 0)
                        .map(|team| m.ids[team])
                        .collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            }))
        } else {
            None
        };
        sync_modeled_tree_work(
            measured,
            consumed_work,
            &builder,
            bootstrap_work,
            enumeration_work,
            training_work,
            rank_hint_work,
            path_priority_work,
        );
        let charged_tree_work = accounted_tree_work(training_work, rank_hint_work, rank_priority);
        let modeled_setup_work = modeled_tree_work(
            &builder,
            bootstrap_work,
            enumeration_work,
            training_work,
            rank_hint_work,
            path_priority_work,
        );
        let final_leaf_counts: Vec<usize> = (0..roots.len())
            .map(|i| leaves.iter().filter(|leaf| leaf.root_index == i).count())
            .collect();
        let path_diag = screen.as_ref().map(|s| json!({
            "enabled": true,
            "proxy": "optimistic packed-score comparator; >= ties count favorably; ordering only",
            "comparator_independence": "approximate independent rival marginals",
            "root_count": roots.len(),
            "work_units": s.work,
            "work_limit": PATH_PRIORITY_WORK_LIMIT,
            "elapsed_ms": screen_elapsed_ms,
            "fallback_reason": s.reason,
            "roots": roots.iter().enumerate().map(|(i, root)| {
                let row = s.roots.get(i);
                json!({
                    "outcomes": root,
                    "prior": row.map(|r| r.prior),
                    "count_chance_exact_rank": row.map(|r| r.count_chance),
                    "raw_score": row.map(|r| r.raw_score),
                    "normalized_initial_share": row.map(|r| r.normalized_share),
                    "operational_share": operational_shares.as_ref().and_then(|v| v.get(i)).copied(),
                    "final_leaf_count": final_leaf_counts[i],
                })
            }).collect::<Vec<_>>()
        })).unwrap_or_else(|| json!({"enabled":false}));
        let mut pair_pilot_diag = if pair_pilot {
            if let Some(result) = pair_pressure_result.as_ref() {
                json!({
                    "enabled":true,"work_units":result.work,"work_limit":PAIR_PRESSURE_WORK_LIMIT,
                    "elapsed_ms":pair_pressure_elapsed_ms,"fallback_reason":result.reason,
                    "rolled_back":result.adjusted_log_scores.is_none(),
                    "unique_private_pmfs":result.unique_private_pmfs,
                    "unique_shared_root_pair_queries":result.unique_root_pair_queries,
                    "independent_normalized_shares":result.independent_shares,
                    "adjusted_normalized_shares":result.adjusted_shares,
                    "leaves":result.leaves.iter().enumerate().map(|(index,row)|json!({
                        "index":index,"required_above_count":row.required_above_count,
                        "pair_count":row.pair_count,"shared_pair_count":row.shared_pair_count,
                        "sum_log_pair_factor":row.sum_log_factor,
                    })).collect::<Vec<_>>(),
                })
            } else {
                json!({"enabled":true,"work_units":0,"work_limit":PAIR_PRESSURE_WORK_LIMIT,"rolled_back":true,"fallback_reason":"independent leaf score unavailable"})
            }
        } else {
            json!({"enabled":false})
        };
        if hierarchical_pair_pilot {
            if let Some(scores) = pair_pressure_result
                .as_ref()
                .and_then(|result| result.adjusted_log_scores.as_ref())
            {
                let valid = scores
                    .iter()
                    .all(|score| score.is_finite() || *score == f64::NEG_INFINITY);
                pair_pilot_diag["adjusted_log_scores_valid"] = json!(valid);
                pair_pilot_diag["adjusted_log_scores"] = if valid {
                    json!(scores
                        .iter()
                        .map(|score| score.is_finite().then_some(*score))
                        .collect::<Vec<_>>())
                } else {
                    serde_json::Value::Null
                };
            } else {
                pair_pilot_diag["adjusted_log_scores_valid"] = json!(false);
                pair_pilot_diag["adjusted_log_scores"] = serde_json::Value::Null;
            }
        }
        let mut leaf_pilot_diag = if leaf_pilot {
            json!({"enabled":true,"work_units":leaf_pilot_work,"tree_work_limit":LEAF_PILOT_WORK_LIMIT,"total_work_limit":LEAF_PILOT_TOTAL_WORK_LIMIT,"fallback_reason":leaf_pilot_reason,"shares":leaf_pilot_shares,"leaves":leaves.iter().enumerate().map(|(i,l)|json!({"index":i,"root_index":l.root_index,"root":l.root,"selected":(0..m.n).filter(|&t|l.selected&(1<<t)!=0).map(|t|m.ids[t]).collect::<Vec<_>>(),"strict":(0..m.n).filter(|&t|l.mask&(1<<t)!=0).map(|t|m.ids[t]).collect::<Vec<_>>(),"original_parent_id":l.original_parent_id,"rival_team_id":l.rival_team_id.map(|team|m.ids[team]),"path_id":l.path_id,"fixed_fixture_results":l.rival_fixed.iter().map(|&(fixture,outcome)|json!({"game_id":m.request.games[m.fixtures[fixture].request_index].id,"outcome":outcome})).collect::<Vec<_>>(),"raw_path_prior":l.rival_team_id.map(|_|l.raw_path_prior),"child_proxy_log":l.child_proxy_log,"parent_proxy_log":l.parent_proxy_log,"local_child_share":l.local_child_share})).collect::<Vec<_>>()})
        } else {
            json!({"enabled":false})
        };
        if let Some(recovery) = structural_recovery_diag.as_ref() {
            leaf_pilot_diag["structural_recovery"] = recovery.clone();
            if let Some(rows) = leaf_pilot_diag["leaves"].as_array_mut() {
                if let Some(structural_rows) = recovery["leaves"].as_array() {
                    for (row, structural) in rows.iter_mut().zip(structural_rows) {
                        row["structural_proxy_share"] =
                            structural["structural_proxy_share"].clone();
                        row["branch_readiness"] = structural["branch_readiness"].clone();
                        row["unresolved_ambiguous_rivals"] =
                            structural["unresolved_ambiguous_rivals"].clone();
                    }
                }
            }
        }
        if hybrid_active {
            let target_indexes = builder
                .target
                .games
                .iter()
                .map(|game| game.index)
                .collect::<std::collections::HashSet<_>>();
            if let Some(rows) = leaf_pilot_diag["leaves"].as_array_mut() {
                for (row, leaf) in rows.iter_mut().zip(&leaves) {
                    row["hybrid_depth"] = json!(leaf.hybrid_depth);
                    row["hybrid_guide_fraction"] = json!(leaf.guide_fraction);
                    row["hybrid_path_count"] = json!(leaf.hybrid_path_count);
                    row["hybrid_inherited_fixed_fixture_results"] = json!(leaf
                        .pattern
                        .fixed
                        .iter()
                        .filter(|(fixture, _)| !target_indexes.contains(fixture))
                        .map(|&(fixture, outcome)| json!({
                            "fixture_index":fixture,
                            "game_id":m.request.games[m.fixtures[fixture].request_index].id,
                            "outcome":outcome
                        }))
                        .collect::<Vec<_>>());
                }
            }
        }
        if pair_pilot {
            leaf_pilot_diag["pair_pilot"] = pair_pilot_diag;
        }
        let mut metadata = json!({"rank_hint_priority":rank_priority,"path_priority":path_diag,"leaf_pilot":leaf_pilot_diag,"path_priority_work_units":path_priority_work,"bootstrap_work_units":bootstrap_work,"root_enumeration_work_units":enumeration_work,"rank_hint_work_units":rank_hint_work,"training_work":training_work,"modeled_setup_work":modeled_setup_work,"initial_leaves":initial,"leaf_limit":leaf_limit,"leaves":leaves.len(),"splits":splits,"attempts":attempts,"failed_splits":failed,"refuted_children":refuted,"joint_nodes":builder.nodes,"guide_limit":guide_limit,"guide_values":builder.guide_values,"training_draws":training_draws,"training_ms":training_ms,"setup_ms":clock.elapsed().as_secs_f64()*1000.,"selected":leaves.iter().map(|l|json!({"root":l.root,"selected":(0..m.n).filter(|&t|l.selected&(1<<t)!=0).map(|t|m.ids[t]).collect::<Vec<_>>(),"strict":(0..m.n).filter(|&t|l.mask&(1<<t)!=0).map(|t|m.ids[t]).collect::<Vec<_>>() })).collect::<Vec<_>>()});
        if let Some(recovery) = structural_recovery_diag {
            metadata["structural_recovery"] = recovery;
        }
        if hybrid_requested.is_some() {
            hybrid_report.work = hybrid_work;
            metadata["hybrid_tree"] = json!({
                "enabled":hybrid_active,
                "requested_mode":hybrid_report.mode,
                "applied":hybrid_report.path_splits + hybrid_report.group_splits > 0,
                "reason":if hybrid_active { None::<&str> } else { Some("requires measured path-priority, leaf-pilot, pair-pilot, and rival-path stack") },
                "work":hybrid_report.work,"cap":hybrid_report.cap,"leaf_limit":leaf_limit,
                "path_limit":32,"group_splits":hybrid_report.group_splits,
                "path_splits":hybrid_report.path_splits,"refuted_paths":hybrid_report.refuted_paths,
                "depths":hybrid_report.depths,"forced_fixtures":hybrid_report.forced_fixtures,
                "rollbacks":hybrid_report.rollbacks,"fallbacks":hybrid_report.fallbacks,
                "elapsed_ms":hybrid_started.elapsed().as_secs_f64()*1000.0,
                "score_semantics":"pattern interval envelope tightening weighted by Pattern.mass*residual_hint and divided by incremental leaf count; this is an allocation heuristic, not a probability bound",
                "guide_fraction_semantics":"normalized Pattern.mass*residual_hint proxy across complete path children; not an exact conditional path probability",
                "records":hybrid_report.records
            });
            if repair_requested {
                metadata["hybrid_tree"]["repair_enabled"] = json!(tree_repair_active);
                metadata["hybrid_tree"]["denied_reservations"] =
                    json!(hybrid_report.denied_reservations);
                metadata["hybrid_tree"]["fallback_splits"] = json!(hybrid_report.fallback_splits);
                metadata["hybrid_tree"]["fallback_work"] = json!(hybrid_report.fallback_work);
                metadata["hybrid_tree"]["repair_hint_work"] =
                    json!(hybrid_report.initial_hint_work);
            }
        }
        if !tree_guide_requested.is_empty() {
            metadata["shortlist_tree_guidance"] = tree_guide.diagnostics(&tree_guide_requested);
        }
        metadata["rival_path_partition"] = json!({"enabled":rival_path_report.enabled,"applied":rival_path_report.applied,"reason":rival_path_report.reason,"caps":{"final_leaves":rival_path::FINAL_LEAF_CAP,"additional_nodes":rival_path::ADDITIONAL_NODE_CAP,"additional_guide_values":rival_path::ADDITIONAL_GUIDE_CAP,"enumeration_scan_work":rival_path::ENUMERATION_WORK_CAP,"per_parent_build_nodes":rival_path::PER_PARENT_NODE_CAP},"work":{"scan_dfs_domain":rival_path_report.scan_work,"additional_nodes":rival_path_report.additional_nodes,"additional_guide_values":rival_path_report.additional_guide_values},"original_leaf_count":rival_path_report.original_leaf_count,"final_leaf_count":leaves.len(),"base_leaf_pilot":rival_path_report.base_pilot_counts,"original_base_leaf_pilot":rival_path_report.original_base_pilot_counts,"frozen_group_budgets":rival_path_report.frozen_group_budgets.iter().map(|(root,selected,quota)|json!({"root":root,"selected_ids":selected,"draws":quota})).collect::<Vec<_>>(),"pilot_counts":rival_path_report.pilot_counts,"parent_rows":rival_path_report.parent_rows,"goal_tiebreak":goal_tiebreak_diag,"proxy_semantics":"heuristic proxy log(Pattern.mass)+log(Pattern.joint.residual_hint); child proxy ratios transfer parent guide mass and are not conditional event probabilities; pair correction is inherited from the original parent"});
        if shared_goal_tiebreak {
            metadata["rival_path_partition"]["shared_goal_tiebreak"] = shared_goal_tiebreak_diag;
        }
        if na_requested {
            metadata["na_guidance"] = json!({
                "enabled":na_report.enabled,
                "root_applied":na_report.root_applied,
                "leaf_applied":na_report.leaf_applied,
                "reason":na_report.reason,
                "blend":na_report.blend,
                "work":na_report.work,
                "cap":na_report.cap,
                "elapsed_ms":na_report.elapsed_ms,
                "cache_hits":na_report.cache_hits,
                "root_rows":na_report.root_rows,
                "leaf_rows":na_report.leaf_rows,
                "semantics":"allocation-only upper-bound shape; fixed/domain masses use original fixture probabilities; separate increasing/decreasing families are minimized and never multiplied; estimator weights and proposal masses are unchanged"
            });
        }
        Ok(Self {
            retuned: HashMap::new(),
            message_work: 0,
            cell,
            strata: leaves.iter().map(stratum).collect(),
            goals: builder.goals,
            setup_nodes: builder.nodes,
            secondary_fallbacks: failed,
            secondary_nodes: 0,
            tree_training_work: charged_tree_work,
            tree_diagnostics: Some(metadata),
        })
    }

    pub(crate) fn overflow_root_count(
        m: &Model,
        cell: Cell,
        seed: i64,
        limit: usize,
    ) -> (std::result::Result<usize, String>, usize) {
        Self::overflow_root_count_filtered(m, cell, seed, limit, &mut |_| (true, 0))
    }

    pub(crate) fn overflow_root_count_filtered(
        m: &Model,
        cell: Cell,
        seed: i64,
        limit: usize,
        filter: &mut dyn FnMut(&[u8]) -> (bool, usize),
    ) -> (std::result::Result<usize, String>, usize) {
        let goal_tilt_requested = crate::rare_tail::value("RUST_ODDS_LAZY_GOALS") == "1";
        let Some(builder) = LazyJoint::with_mode(m, cell, seed, &[], 0, false, false, None) else {
            let work = lazy_bootstrap_work_estimate(m, cell, goal_tilt_requested);
            return (Err("unsupported tree model".into()), work);
        };
        let bootstrap_work = lazy_bootstrap_work_estimate(m, cell, goal_tilt_requested);
        let (roots, enumeration_work) =
            keys_with_limit_measured(&builder.target, cell.team, limit, true);
        let (roots, filter_work) = match roots {
            Ok(roots) => {
                let (retained, work) = filter_complete_roots(roots, filter);
                (Ok(retained), work)
            }
            Err(error) => (Err(error), 0),
        };
        let work = crate::rare_tail::budget::setup_cost(builder.nodes, builder.guide_values)
            .saturating_add(bootstrap_work)
            .saturating_add(enumeration_work)
            .saturating_add(filter_work);
        (roots.map(|roots| roots.len()), work)
    }

    pub(crate) fn with_overflow_tree(
        m: &Model,
        cell: Cell,
        seed: i64,
    ) -> (std::result::Result<Self, String>, usize) {
        Self::with_tree_profile(
            m,
            cell,
            seed,
            4,
            256,
            0,
            OVERFLOW_TREE_GUIDE_LIMIT,
            OVERFLOW_TREE_NODE_LIMIT,
            256,
            true,
            true,
        )
    }

    pub(crate) fn overflow_tree_setup_admission_floor(root_count_work: usize) -> usize {
        root_count_work.saturating_add(crate::rare_tail::budget::setup_cost(
            OVERFLOW_TREE_NODE_LIMIT,
            OVERFLOW_TREE_GUIDE_LIMIT,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{conditioned::canonical_ranks, model::Request};

    fn binary_path_table(game_count: usize) -> TerminalTable {
        let games = (0..game_count)
            .map(|index| crate::lookahead::RankGame {
                index,
                home: 0,
                away: 1,
                prob: [0.5, 0.5, 0.],
                hg: [0, 0, 0],
                ag: [0, 0, 0],
            })
            .collect::<Vec<_>>();
        TerminalTable::new(games, 0, vec![1.])
    }

    #[test]
    fn target_path_limit_defaults_to_64_and_is_bounded() {
        assert_eq!(bounded_tree_target_path_limit(None), 64);
        assert_eq!(bounded_tree_target_path_limit(Some("bad")), 64);
        assert_eq!(bounded_tree_target_path_limit(Some("32")), 64);
        assert_eq!(bounded_tree_target_path_limit(Some("256")), 256);
        assert_eq!(bounded_tree_target_path_limit(Some("2048")), 1024);
    }

    #[test]
    fn keys_accept_exact_complete_limit_and_reject_partial_union() {
        let exact = binary_path_table(6);
        let exact_keys = keys_with_limit(&exact, 0, 64).unwrap();
        assert_eq!(exact_keys.len(), 64);
        assert!(exact_keys
            .iter()
            .all(|key| key.iter().all(|&outcome| outcome < 2)));

        let over = binary_path_table(7);
        let error = keys_with_limit(&over, 0, 64).unwrap_err();
        assert_eq!(error, "complete tree target-path budget exhausted");
        assert_eq!(keys_with_limit(&over, 0, 128).unwrap().len(), 128);
    }

    fn root_has_exact_rank_completion(m: &Model, cell: Cell, root: &[u8]) -> (bool, usize) {
        let Some(builder) = LazyJoint::with_mode(m, cell, 808, &[], 0, false, false, None) else {
            return (true, 0);
        };
        if root.len() != builder.target.games.len() {
            return (true, 0);
        }
        let mut outcomes = vec![0u8; m.fixtures.len()];
        for (game, &outcome) in builder.target.games.iter().zip(root) {
            outcomes[game.index] = outcome;
        }
        let mut work = 0usize;
        for mut code in 0..3usize.pow(builder.remaining.len() as u32) {
            let mut positive = true;
            for game in &builder.remaining {
                let outcome = code % 3;
                code /= 3;
                outcomes[game.index] = outcome as u8;
                positive &= game.prob[outcome] > 0.0;
            }
            work = work.saturating_add(1);
            if positive && canonical_ranks(m, &outcomes)[cell.team] == cell.rank {
                return (true, work);
            }
        }
        (false, work)
    }

    #[test]
    fn filtered_roots_keep_the_exhaustive_rank_cover_and_caps_atomically() {
        let m = model();
        let cell = Cell { team: 0, rank: 1 };
        let builder = LazyJoint::with_mode(&m, cell, 808, &[], 0, false, false, None).unwrap();
        let roots = keys_with_limit(&builder.target, cell.team, 256).unwrap();
        let expected = roots
            .iter()
            .filter(|root| root_has_exact_rank_completion(&m, cell, root).0)
            .cloned()
            .collect::<Vec<_>>();
        let mut filter = |root: &[u8]| {
            let (keep, work) = root_has_exact_rank_completion(&m, cell, root);
            (keep, work)
        };
        let (count, work) =
            BranchStrata::overflow_root_count_filtered(&m, cell, 808, 256, &mut filter);
        assert_eq!(count.unwrap(), expected.len());
        assert!(work > 0);

        let mut tree_filter = |root: &[u8]| {
            let (keep, work) = root_has_exact_rank_completion(&m, cell, root);
            (keep, work)
        };
        let (tree, tree_work) = BranchStrata::with_tree_profile_options_recovery(
            &m,
            cell,
            808,
            1,
            256,
            0,
            4_000_000,
            100_000,
            256,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            0,
            false,
            false,
            rival_path::SHARED_GOAL_TIEBREAK_WORK_LIMIT,
            true,
            Some(&mut tree_filter),
        );
        let tree = tree.unwrap();
        assert!(tree_work > 0);
        assert!((coverage(&m, &tree) - exact_rank_mass_for_test(&m, cell)).abs() < 1e-12);

        let mut callbacks = 0usize;
        let mut capped_filter = |_: &[u8]| {
            callbacks += 1;
            (false, 1)
        };
        let (capped, capped_work) =
            BranchStrata::overflow_root_count_filtered(&m, cell, 808, 1, &mut capped_filter);
        assert!(
            matches!(capped, Err(error) if error.contains("complete tree target-path budget exhausted"))
        );
        assert_eq!(
            callbacks, 0,
            "a capped partial enumeration cannot be filtered"
        );
        assert!(capped_work > 0);

        let mut empty_filter = |_: &[u8]| (false, 1);
        let (empty, empty_work) = filter_complete_roots(Vec::new(), &mut empty_filter);
        assert!(empty.is_empty());
        assert_eq!(empty_work, 0);
    }

    #[test]
    fn comparable_structural_family_limits_coarse_sibling_and_keeps_exploration() {
        let shares = comparable_group_shares(&[0.05, 0.05, 0.90], &[true, true, false]);
        assert_eq!(shares.len(), 3);
        assert!(shares.iter().all(|share| *share > 0.0));
        assert!((shares.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(shares[2] < 0.20, "coarse sibling received {shares:?}");

        let all_coarse = comparable_group_shares(&[0.0, 0.0], &[false, false]);
        assert!(all_coarse.iter().all(|share| *share > 0.0));
        assert!((all_coarse[0] + all_coarse[1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn default_path_cap_keeps_the_original_64_root_behavior() {
        let exact = binary_path_table(6);
        assert_eq!(
            keys_with_limit(&exact, 0, DEFAULT_TREE_TARGET_PATH_LIMIT)
                .unwrap()
                .len(),
            64
        );
        let over = binary_path_table(7);
        assert_eq!(
            keys_with_limit(&over, 0, DEFAULT_TREE_TARGET_PATH_LIMIT).unwrap_err(),
            "complete tree target-path budget exhausted"
        );
    }

    #[test]
    fn overflow_setup_admission_floor_includes_setup_quota_and_saturates() {
        let quota = crate::rare_tail::budget::setup_cost(
            OVERFLOW_TREE_NODE_LIMIT,
            OVERFLOW_TREE_GUIDE_LIMIT,
        );
        assert_eq!(BranchStrata::overflow_tree_setup_admission_floor(0), quota);
        assert_eq!(
            BranchStrata::overflow_tree_setup_admission_floor(17),
            quota.saturating_add(17)
        );
        assert_eq!(
            BranchStrata::overflow_tree_setup_admission_floor(usize::MAX),
            usize::MAX
        );
    }

    #[test]
    fn rank_hint_priority_can_outweigh_a_larger_prior_score() {
        let high_prior = rank_priority_score(1e-4, 1e-4);
        let lower_prior = rank_priority_score(1e-6, 0.1);
        assert!(lower_prior > high_prior);
        assert_eq!(rank_priority_score(1., 0.), 1e-80);
    }

    #[test]
    fn rank_hint_work_is_charged_only_when_priority_is_enabled() {
        assert_eq!(accounted_tree_work(100, 25, true), 125);
        assert_eq!(accounted_tree_work(100, 25, false), 100);
        assert_eq!(accounted_tree_work(usize::MAX, 1, true), usize::MAX);
    }

    #[test]
    fn marginal_cache_preserves_mass_and_reduces_repeated_work() {
        let games = vec![crate::lookahead::RankGame {
            index: 7,
            home: 0,
            away: 1,
            prob: [0.2, 0.3, 0.5],
            hg: [0, 1, 3],
            ag: [3, 1, 0],
        }];
        let expected = calculate_marginal_mass(games.clone(), 0, true, 2).0;
        let mut cache = HashMap::new();
        let (first, miss_work) = cached_marginal_mass(games.clone(), 0, true, 2, Some(&mut cache));
        let (second, hit_work) = cached_marginal_mass(games, 0, true, 2, Some(&mut cache));
        assert_eq!(first, expected);
        assert_eq!(second, expected);
        assert!(hit_work < miss_work);
        assert_eq!(cache.len(), 1);
    }

    fn model() -> Model {
        let request:Request=serde_json::from_value(json!({"id":1,"phase":{"sort":"pt,w,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":(0..4).map(|t|json!({"team_id":t,"add_sub":t,"bias":t})).collect::<Vec<_>>(),
        "games": ([(0,1),(0,2),(1,3),(2,3)]).iter().enumerate().map(|(i,&(h,a))|json!({"id":i,"home_id":h,"away_id":a,"home_power":1.1,"away_power":0.9})).collect::<Vec<_>>() })).unwrap();
        Model::new(request).unwrap()
    }

    fn goal_tiebreak_model() -> Model {
        let request: Request = serde_json::from_value(json!({
            "id":2,
            "phase":{"sort":"pt,w,gd,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..4).map(|t|json!({"team_id":t,"add_sub":t,"bias":t})).collect::<Vec<_>>(),
            "games": ([(0,1),(0,2),(1,3),(2,3)]).iter().enumerate().map(|(i,&(h,a))|json!({"id":i,"home_id":h,"away_id":a,"home_power":1.1,"away_power":0.9})).collect::<Vec<_>>()
        })).unwrap();
        Model::new(request).unwrap()
    }
    fn coverage(m: &Model, p: &BranchStrata) -> f64 {
        let mut left = vec![0; m.n];
        for g in &m.fixtures {
            left[g.home] += 1;
            left[g.away] += 1;
        }
        let stride = m
            .base
            .iter()
            .zip(&left)
            .map(|(b, l)| b.wins + l)
            .max()
            .unwrap()
            + 1;
        let mut exact = 0.;
        for mut code in 0..3usize.pow(m.fixtures.len() as u32) {
            let mut out = vec![0; m.fixtures.len()];
            let mut mass = 1.;
            let mut totals: Vec<_> = m.base.iter().map(|b| b.points * stride + b.wins).collect();
            for (i, g) in m.fixtures.iter().enumerate() {
                let o = code % 3;
                code /= 3;
                out[i] = o as u8;
                mass *= g.prob[o];
                totals[g.home] += [0, stride, 3 * stride + 1][o];
                totals[g.away] += [3 * stride + 1, stride, 0][o];
            }
            let count = p
                .strata
                .iter()
                .filter(|s| {
                    let p = &s.pattern;
                    p.fixed.iter().all(|&(i, o)| out[i] == o)
                        && p.joint
                            .games
                            .iter()
                            .chain(&p.guide.games)
                            .chain(&p.omitted)
                            .all(|g| g.prob[out[g.index] as usize] > 0.)
                        && (0..m.n).all(|t| totals[t] >= p.lower[t] && totals[t] <= p.upper[t])
                })
                .count();
            assert!(count <= 1, "overlapping leaves at {out:?}");
            if canonical_ranks(m, &out)[p.cell.team] == p.cell.rank {
                assert_eq!(count, 1, "lost a rank event at {out:?}");
                exact += mass;
            }
        }
        exact
    }
    #[test]
    fn tree_partition_and_weights_preserve_every_enumerated_rank() {
        let m = model();
        for rank in 0..m.n {
            let cell = Cell { team: 0, rank };
            let p = BranchStrata::with_tree(&m, cell, 808, 1, 32, 0, 4000000).unwrap();
            let exact = coverage(&m, &p);
            for bounds in [false, true] {
                let samples: Vec<_> = (0..p.len())
                    .map(|i| {
                        p.sample(
                            &m,
                            i,
                            5000 + i * 17,
                            derive(808, &format!("tree-weight-test-{i}")),
                            bounds,
                            false,
                        )
                    })
                    .collect();
                let r = combine(&samples);
                assert!(
                    (r.probability - exact).abs() < 6. * r.std_err + 1e-4,
                    "{} vs {exact}",
                    r.probability
                );
            }
        }
    }
    #[test]
    fn rival_path_experiment_preserves_exact_union_and_frozen_pilot_totals() {
        let m = model();
        let mut multi_parent_applied = false;
        for rank in 0..m.n {
            let cell = Cell { team: 0, rank };
            let (proposal, _) = BranchStrata::with_rank_rescue_tree_experiment(
                &m, cell, 404, true, true, true, true, true, 25,
            );
            let proposal = proposal.unwrap();
            let diagnostics = proposal.setup_diagnostics();
            let partition = &diagnostics["tree"]["rival_path_partition"];
            if partition["applied"] != true {
                continue;
            }
            assert!(proposal.len() >= partition["original_leaf_count"].as_u64().unwrap() as usize);
            let counts = proposal.rival_path_base_pilot_counts().unwrap();
            assert_eq!(counts.len(), proposal.len());
            let budgets = proposal.rival_path_pilot_budgets().unwrap().1;
            let budget_total = budgets.iter().map(|(_, _, n)| *n).sum::<usize>();
            assert_eq!(counts.iter().sum::<usize>(), budget_total);
            assert!(counts.iter().all(|count| *count >= 5));
            assert!((coverage(&m, &proposal) - exact_rank_mass_for_test(&m, cell)).abs() < 1e-12);
            let applied_rows = partition["parent_rows"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["applied"] == true)
                .count();
            if applied_rows < 2 {
                continue;
            }
            let leaf_rows = diagnostics["tree"]["leaf_pilot"]["leaves"]
                .as_array()
                .unwrap();
            for (root, selected, quota) in &budgets {
                let root_json = json!(root);
                let selected_json = json!(selected);
                let group_total = leaf_rows
                    .iter()
                    .enumerate()
                    .filter(|(_, row)| row["root"] == root_json && row["selected"] == selected_json)
                    .map(|(index, _)| counts[index])
                    .sum::<usize>();
                assert_eq!(group_total, *quota);
            }
            multi_parent_applied = true;
            break;
        }
        assert!(
            multi_parent_applied,
            "tiny fixture should admit two bounded rival parent partitions"
        );
    }

    #[test]
    fn goal_tiebreak_changes_guidance_only_after_an_applied_partition() {
        let m = goal_tiebreak_model();
        let mut exercised = false;
        let mut shared_exercised = false;
        let mut saw_applied_partition = false;
        for team in 0..m.n {
            for rank in 0..m.n {
                let cell = Cell { team, rank };
                let build = |enabled| {
                    BranchStrata::with_rank_rescue_tree_goal_tiebreak(
                        &m, cell, 919, true, true, true, true, true, 25, enabled,
                    )
                    .0
                    .unwrap()
                };
                let baseline = build(false);
                let guided = build(true);
                let baseline_tree = &baseline.setup_diagnostics()["tree"];
                let guided_tree = &guided.setup_diagnostics()["tree"];
                let base_partition = &baseline_tree["rival_path_partition"];
                let goal_partition = &guided_tree["rival_path_partition"];
                if base_partition["applied"] != true || goal_partition["applied"] != true {
                    continue;
                }
                let test_shared_cap = rival_path::SHARED_GOAL_TIEBREAK_WORK_LIMIT + 1_000_000;
                let shared = BranchStrata::with_rank_rescue_tree_shared_goal_tiebreak_work_limit(
                    &m,
                    cell,
                    919,
                    true,
                    true,
                    true,
                    true,
                    true,
                    25,
                    true,
                    true,
                    test_shared_cap,
                )
                .0
                .unwrap();
                let shared_tree = &shared.setup_diagnostics()["tree"];
                let shared_partition = &shared_tree["rival_path_partition"];
                assert!(base_partition.get("shared_goal_tiebreak").is_none());
                assert_eq!(shared_partition["shared_goal_tiebreak"]["enabled"], true);
                assert_eq!(
                    shared_partition["shared_goal_tiebreak"]["cap"],
                    test_shared_cap
                );
                assert!(
                    shared_partition["shared_goal_tiebreak"]["work"]
                        .as_u64()
                        .unwrap()
                        <= test_shared_cap as u64
                );
                assert!(
                    shared_tree["path_priority_work_units"].as_u64().unwrap()
                        >= shared_partition["shared_goal_tiebreak"]["work"]
                            .as_u64()
                            .unwrap()
                );
                assert_eq!(
                    base_partition["original_leaf_count"],
                    shared_partition["original_leaf_count"]
                );
                assert_eq!(
                    base_partition["final_leaf_count"],
                    shared_partition["final_leaf_count"]
                );
                assert_eq!(
                    base_partition["frozen_group_budgets"],
                    shared_partition["frozen_group_budgets"]
                );
                assert_eq!(
                    base_partition["parent_rows"],
                    shared_partition["parent_rows"]
                );
                assert_eq!(
                    base_partition["base_leaf_pilot"],
                    shared_partition["base_leaf_pilot"]
                );
                assert_eq!(
                    base_partition["original_base_leaf_pilot"],
                    shared_partition["original_base_leaf_pilot"]
                );
                assert_eq!(
                    base_partition["pilot_counts"],
                    shared_partition["pilot_counts"]
                );
                assert_eq!(
                    baseline_tree["leaf_pilot"]["pair_pilot"]["independent_normalized_shares"],
                    shared_tree["leaf_pilot"]["pair_pilot"]["independent_normalized_shares"]
                );
                assert_eq!(baseline.len(), guided.len());
                assert_eq!(baseline.len(), shared.len());
                assert_eq!(
                    base_partition["frozen_group_budgets"],
                    goal_partition["frozen_group_budgets"]
                );
                assert_eq!(base_partition["parent_rows"], goal_partition["parent_rows"]);
                let goal_diag = &goal_partition["goal_tiebreak"];
                assert_eq!(goal_diag["enabled"], true);
                assert!(goal_diag["work"].as_u64().unwrap() <= goal_diag["cap"].as_u64().unwrap());
                saw_applied_partition = true;
                assert_eq!(
                    baseline_tree["leaf_pilot"]["leaves"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|row| (
                            row["root"].clone(),
                            row["original_parent_id"].clone(),
                            row["rival_team_id"].clone(),
                            row["path_id"].clone()
                        ))
                        .collect::<Vec<_>>(),
                    guided_tree["leaf_pilot"]["leaves"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|row| (
                            row["root"].clone(),
                            row["original_parent_id"].clone(),
                            row["rival_team_id"].clone(),
                            row["path_id"].clone()
                        ))
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    baseline_tree["leaf_pilot"]["leaves"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|row| (
                            row["root"].clone(),
                            row["original_parent_id"].clone(),
                            row["rival_team_id"].clone(),
                            row["path_id"].clone(),
                            row["local_child_share"].clone()
                        ))
                        .collect::<Vec<_>>(),
                    shared_tree["leaf_pilot"]["leaves"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|row| (
                            row["root"].clone(),
                            row["original_parent_id"].clone(),
                            row["rival_team_id"].clone(),
                            row["path_id"].clone(),
                            row["local_child_share"].clone()
                        ))
                        .collect::<Vec<_>>()
                );
                let base_logs = baseline_tree["leaf_pilot"]["pair_pilot"]["adjusted_log_scores"]
                    .as_array()
                    .unwrap();
                let guided_logs = guided_tree["leaf_pilot"]["pair_pilot"]["adjusted_log_scores"]
                    .as_array()
                    .unwrap();
                let shared_logs = shared_tree["leaf_pilot"]["pair_pilot"]["adjusted_log_scores"]
                    .as_array()
                    .unwrap();
                let shared_diag = &shared_partition["shared_goal_tiebreak"];
                for row in shared_diag["query_rows"].as_array().unwrap() {
                    if row["applied"] != true {
                        continue;
                    }
                    let Some(log_factor) = row["log_factor"].as_f64() else {
                        continue;
                    };
                    if log_factor.abs() < 1e-10 {
                        continue;
                    }
                    let index = row["leaf_index"].as_u64().unwrap() as usize;
                    let base_log = base_logs[index].as_f64().unwrap();
                    let shared_log = shared_logs[index].as_f64().unwrap();
                    assert!((shared_log - base_log - log_factor).abs() < 1e-9);
                    shared_exercised = true;
                }
                for row in goal_diag["query_rows"].as_array().unwrap() {
                    let Some(factor) = row["guide_factor"].as_f64() else {
                        continue;
                    };
                    if row["earlier_keys"] != "tied" || (factor - 1.0).abs() < 1e-14 {
                        continue;
                    }
                    let index = row["leaf_index"].as_u64().unwrap() as usize;
                    let base_log = base_logs[index].as_f64().unwrap();
                    let guided_log = guided_logs[index].as_f64().unwrap();
                    assert!((guided_log - base_log - factor.ln()).abs() < 1e-10);
                    exercised = true;
                }
                if exercised && shared_exercised {
                    break;
                }
            }
            if exercised && shared_exercised {
                break;
            }
        }
        assert!(
            saw_applied_partition,
            "fixture should admit a rival partition"
        );
        assert!(
            exercised,
            "fixture should produce an eligible tied rival query with a nonunit GD factor"
        );
        assert!(
            shared_exercised,
            "fixture should produce a nonunit shared-GD query wired into corrected child guides"
        );
    }

    fn exact_rank_mass_for_test(m: &Model, cell: Cell) -> f64 {
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
    fn path_priority_changes_ordering_without_losing_rank_support() {
        let m = model();
        for rank in 0..m.n {
            let cell = Cell { team: 0, rank };
            let (result, work) = BranchStrata::with_tree_profile_options(
                &m,
                cell,
                808,
                1,
                32,
                0,
                4_000_000,
                100_000,
                64,
                true,
                true,
                true,
                false,
                false,
                false,
                false,
                0,
                false,
                false,
                rival_path::SHARED_GOAL_TIEBREAK_WORK_LIMIT,
            );
            let proposal = result.unwrap();
            assert!(work > 0);
            assert!(coverage(&m, &proposal) > 0.0);
            let diagnostics = proposal.setup_diagnostics();
            let tree = &diagnostics["tree"];
            assert_eq!(tree["path_priority"]["enabled"], true);
            let roots = tree["path_priority"]["roots"].as_array().unwrap();
            assert!(!roots.is_empty());
            let initial: f64 = roots
                .iter()
                .filter_map(|r| r["normalized_initial_share"].as_f64())
                .sum();
            let operational: f64 = roots
                .iter()
                .filter_map(|r| r["operational_share"].as_f64())
                .sum();
            assert!((initial - 1.0).abs() < 1e-9);
            assert!((operational - 1.0).abs() < 1e-9);
            let exact = coverage(&m, &proposal);
            for bounds in [false, true] {
                let samples: Vec<_> = (0..proposal.len())
                    .map(|i| {
                        proposal.sample(
                            &m,
                            i,
                            5000 + i * 17,
                            derive(808, &format!("path-priority-weight-test-{rank}-{i}")),
                            bounds,
                            false,
                        )
                    })
                    .collect();
                let estimate = combine(&samples);
                assert!(
                    (estimate.probability - exact).abs() < 6.0 * estimate.std_err + 1e-4,
                    "rank {rank}: {} vs {exact}",
                    estimate.probability
                );
            }
        }
    }

    #[test]
    fn enabling_leaf_pilots_preserves_the_frozen_tree_partition() {
        let m = model();
        let cell = Cell { team: 0, rank: 1 };
        let build = |leaf_pilot, pair_pilot| {
            BranchStrata::with_tree_profile_options(
                &m,
                cell,
                808,
                1,
                32,
                0,
                4_000_000,
                100_000,
                64,
                true,
                true,
                true,
                leaf_pilot,
                pair_pilot,
                false,
                false,
                0,
                false,
                false,
                rival_path::SHARED_GOAL_TIEBREAK_WORK_LIMIT,
            )
            .0
            .unwrap()
        };
        let baseline = build(false, false);
        let guided = build(true, false);
        let paired = build(true, true);
        let baseline_diagnostics = baseline.setup_diagnostics();
        let guided_diagnostics = guided.setup_diagnostics();
        let paired_diagnostics = paired.setup_diagnostics();
        assert_eq!(
            baseline_diagnostics["tree"]["selected"],
            guided_diagnostics["tree"]["selected"]
        );
        assert_eq!(
            baseline_diagnostics["tree"]["path_priority"]["roots"],
            guided_diagnostics["tree"]["path_priority"]["roots"]
        );
        assert_eq!(baseline_diagnostics["tree"]["leaf_pilot"]["enabled"], false);
        assert_eq!(guided_diagnostics["tree"]["leaf_pilot"]["enabled"], true);
        assert_eq!(
            guided_diagnostics["tree"]["selected"],
            paired_diagnostics["tree"]["selected"]
        );
        assert_eq!(
            guided_diagnostics["tree"]["path_priority"]["roots"],
            paired_diagnostics["tree"]["path_priority"]["roots"]
        );
        let pair_diag = &paired_diagnostics["tree"]["leaf_pilot"]["pair_pilot"];
        assert_eq!(pair_diag["enabled"], true);
        assert_eq!(
            pair_diag["rolled_back"], false,
            "{:?}",
            pair_diag["fallback_reason"]
        );
        assert!(pair_diag["work_units"].as_u64().unwrap() > 0);
    }

    #[test]
    fn hybrid_primary_paths_and_groups_preserve_full_partition() {
        let m = model();
        let cell = Cell { team: 0, rank: 1 };
        let key = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_HYBRID_TREE";
        for mode in ["paths", "mixed"] {
            let _env = experiment_env::scoped(&[
                (key, Some(mode)),
                ("RUST_ODDS_EXPERIMENT_RANK_RESCUE_HYBRID_REPAIR", None),
                ("RUST_ODDS_EXPERIMENT_RANK_RESCUE_SHORTLIST_TREE", None),
                ("RUST_ODDS_EXPERIMENT_RANK_RESCUE_SHORTLIST_SOURCE", None),
                ("RUST_ODDS_EXPERIMENT_RANK_RESCUE_SHORTLIST_MODE", None),
            ]);
            let (built, _) = BranchStrata::with_tree_profile_options(
                &m,
                cell,
                404,
                1,
                32,
                0,
                4_000_000,
                100_000,
                64,
                true,
                true,
                true,
                true,
                true,
                false,
                true,
                25,
                false,
                false,
                rival_path::SHARED_GOAL_TIEBREAK_WORK_LIMIT,
            );
            let proposal = built.unwrap();
            let diagnostic = proposal.setup_diagnostics();
            let hybrid = &diagnostic["tree"]["hybrid_tree"];
            assert_eq!(hybrid["enabled"], true);
            assert!(hybrid["work"].as_u64().unwrap() <= hybrid["cap"].as_u64().unwrap());
            let (parents, shares) = proposal
                .rival_path_parent_map()
                .expect("secondary parent map should remain available after hybrid primary paths");
            let mut parent_totals = std::collections::HashMap::<usize, f64>::new();
            for (parent, share) in parents.into_iter().zip(shares) {
                *parent_totals.entry(parent).or_default() += share;
            }
            assert!(parent_totals
                .values()
                .all(|total| (*total - 1.0).abs() < 1e-8));
            let exact = exact_rank_mass_for_test(&m, cell);
            assert!((coverage(&m, &proposal) - exact).abs() < 1e-12);
            let samples = (0..proposal.len())
                .map(|i| {
                    proposal.sample(
                        &m,
                        i,
                        3_000 + i * 13,
                        derive(808, &format!("hybrid-law-{mode}-{i}")),
                        false,
                        false,
                    )
                })
                .collect::<Vec<_>>();
            let estimate = combine(&samples);
            assert!(
                (estimate.probability - exact).abs() < 6.0 * estimate.std_err + 1e-4,
                "hybrid {mode}: {} vs {exact}",
                estimate.probability
            );
            if mode == "paths" {
                assert!(
                    hybrid["path_splits"].as_u64().unwrap() > 0,
                    "path mode failed to apply a complete WDL-path split: {hybrid}"
                );
                let records = hybrid["records"].as_array().unwrap();
                assert!(
                    records
                        .iter()
                        .any(|row| row["kind"] == "path_split"
                            && row["depth"].as_u64().unwrap_or(0) > 0),
                    "path preview must remain eligible below an earlier split: {hybrid}"
                );
            } else {
                assert!(
                    hybrid["path_splits"].as_u64().unwrap() > 0,
                    "mixed mode never selected a path split: {hybrid}"
                );
                assert!(
                    hybrid["group_splits"].as_u64().unwrap() > 0,
                    "mixed mode never selected a group split: {hybrid}"
                );
                let records = hybrid["records"].as_array().unwrap();
                assert!(
                    records
                        .iter()
                        .any(|row| row["kind"] == "path_split"
                            && row["depth"].as_u64().unwrap_or(0) > 0),
                    "mixed mode did not try paths after an earlier split: {hybrid}"
                );
                assert!(
                    records.iter().any(|row| row["kind"] == "group_split"
                        && row["depth"].as_u64().unwrap_or(0) > 0),
                    "mixed mode did not apply group constraints below a split: {hybrid}"
                );
            }
            let initial_roots = diagnostic["tree"]["path_priority"]["roots"]
                .as_array()
                .unwrap();
            assert_eq!(
                initial_roots.len(),
                BranchStrata::tree_root_count(&m, cell, 808)
            );
        }
    }

    #[test]
    fn repaired_nested_fallback_preserves_inherited_fixed_fixtures_and_rank_mass() {
        const HYBRID: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_HYBRID_TREE";
        const REPAIR: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_HYBRID_REPAIR";
        let _env = experiment_env::scoped(&[
            (HYBRID, Some("mixed")),
            (REPAIR, Some("1")),
            ("RUST_ODDS_EXPERIMENT_RANK_RESCUE_SHORTLIST_TREE", None),
            ("RUST_ODDS_EXPERIMENT_RANK_RESCUE_SHORTLIST_SOURCE", None),
            ("RUST_ODDS_EXPERIMENT_RANK_RESCUE_SHORTLIST_MODE", None),
        ]);
        let m = model();
        let cell = Cell { team: 0, rank: 1 };
        let exact = exact_rank_mass_for_test(&m, cell);
        let mut exercised = None;
        for quota in [0, 1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024] {
            let (result, _) = BranchStrata::with_tree_profile_options(
                &m,
                cell,
                808,
                1,
                32,
                0,
                4_000_000,
                quota,
                64,
                true,
                true,
                true,
                true,
                true,
                false,
                true,
                0,
                false,
                false,
                rival_path::SHARED_GOAL_TIEBREAK_WORK_LIMIT,
            );
            let proposal = result.unwrap();
            let hybrid = &proposal.setup_diagnostics()["tree"]["hybrid_tree"];
            let records = hybrid["records"].as_array().unwrap();
            if records.iter().any(|row| {
                row["kind"] == "fallback_group_split"
                    && row["applied"] == true
                    && row["depth"].as_u64().unwrap_or(0) > 0
                    && row["inherited_fixed_fixture_count"].as_u64().unwrap_or(0) > 0
            }) {
                assert!(
                    (coverage(&m, &proposal) - exact).abs() < 1e-12,
                    "nested fixed-aware fallback changed exhaustive rank mass: {hybrid}"
                );
                assert!(hybrid["fallback_splits"].as_u64().unwrap_or(0) > 0);
                exercised = Some(quota);
                break;
            }
        }
        assert!(
            exercised.is_some(),
            "could not reach a nested fallback retaining inherited fixtures"
        );
    }
    #[test]
    fn exhausted_split_budget_keeps_complete_parent_regions() {
        let m = model();
        let cell = Cell { team: 0, rank: 1 };
        let p = BranchStrata::with_tree_quota(&m, cell, 808, 1, 32, 0, 4000000, 0).unwrap();
        assert!(
            p.tree_diagnostics.as_ref().unwrap()["failed_splits"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert!(coverage(&m, &p) > 0.);
    }
    #[test]
    fn insufficient_root_budget_cannot_return_a_partial_union() {
        let m = model();
        let cell = Cell { team: 0, rank: 1 };
        assert!(
            matches!(BranchStrata::with_tree(&m,cell,808,1,1,0,4000000),Err(e) if e.contains("complete roots exceed"))
        );
    }

    #[test]
    fn overflow_tree_helpers_charge_complete_and_failed_work_without_environment_changes() {
        let m = model();
        let cell = Cell { team: 0, rank: 1 };
        let (root_result, root_work) = BranchStrata::overflow_root_count(&m, cell, 808, 256);
        assert!(root_result.unwrap() > 0);
        assert!(root_work > 0);
        let (capped_roots, capped_root_work) = BranchStrata::overflow_root_count(&m, cell, 808, 1);
        assert!(
            matches!(capped_roots, Err(error) if error.contains("complete tree target-path budget exhausted"))
        );
        assert!(capped_root_work > 0);

        let (tree_result, tree_work) = BranchStrata::with_tree_profile(
            &m, cell, 808, 1, 0, 0, 4_000_000, 100_000, 256, true, true,
        );
        assert!(matches!(tree_result, Err(error) if error.contains("complete roots exceed")));
        assert!(tree_work > 0);
    }
}
