//! Bounded complete partition: child failure retains the parent.
use super::*;
use crate::rng::derive;
use std::time::Instant;

struct Leaf {
    root: Vec<u8>,
    above: bool,
    mask: u32,
    selected: u32,
    ambiguous: u32,
    max_exceptions: usize,
    pattern: Pattern,
    frozen: bool,
    score: f64,
    rank_hint_next_team: Option<usize>,
}

const DEFAULT_TREE_TARGET_PATH_LIMIT: usize = 64;
const MIN_TREE_TARGET_PATH_LIMIT: usize = 64;
const MAX_TREE_TARGET_PATH_LIMIT: usize = 1024;
const TREE_TARGET_PATH_LIMIT_ENV: &str = "RUST_ODDS_EXPERIMENT_TREE_TARGET_PATH_LIMIT";
const TREE_MARGINAL_CACHE_LIMIT: usize = 4096;
const OVERFLOW_TREE_NODE_LIMIT: usize = 100_000;
const OVERFLOW_TREE_GUIDE_LIMIT: usize = 4_000_000;
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
fn stratum(l: &Leaf) -> Stratum {
    Stratum {
        root: l.root.clone(),
        exceptions: (0..32).filter(|&t| l.mask & (1 << t) != 0).collect(),
        above: l.above,
        secondary: None,
        pattern: l.pattern.clone(),
    }
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
) -> usize {
    crate::rare_tail::budget::setup_cost(builder.nodes, builder.guide_values)
        .saturating_add(bootstrap_work)
        .saturating_add(enumeration_work)
        .saturating_add(training_work)
        .saturating_add(rank_hint_work)
}

fn sync_modeled_tree_work(
    measured: bool,
    consumed: &mut usize,
    builder: &LazyJoint,
    bootstrap_work: usize,
    enumeration_work: usize,
    training_work: usize,
    rank_hint_work: usize,
) {
    if measured {
        *consumed = modeled_tree_work(
            builder,
            bootstrap_work,
            enumeration_work,
            training_work,
            rank_hint_work,
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
        consumed_work: &mut usize,
    ) -> std::result::Result<Self, String> {
        let clock = Instant::now();
        let goal_tilt_requested = crate::rare_tail::value("RUST_ODDS_LAZY_GOALS") == "1";
        let Some(mut builder) = LazyJoint::with_mode(m, cell, seed, &[], 0, false, false, None)
        else {
            if measured {
                *consumed_work = lazy_bootstrap_work_estimate(m, cell, goal_tilt_requested);
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
        let (roots_result, enumeration_work) =
            keys_with_limit_measured(&builder.target, cell.team, target_path_limit, measured);
        sync_modeled_tree_work(
            measured,
            consumed_work,
            &builder,
            bootstrap_work,
            enumeration_work,
            0,
            0,
        );
        let roots = roots_result?;
        if roots.len() > leaf_limit {
            return Err("complete roots exceed leaf budget".into());
        }
        let mut leaves = Vec::new();
        let (mut training_work, mut training_draws, mut training_ms) = (0, 0, 0.);
        let mut rank_hint_work = 0usize;
        let mut marginal_cache = rank_priority.then(HashMap::<MarginalKey, f64>::new);
        let mut serial = 0;
        for root in roots {
            let mut points = builder.base.clone();
            for (g, &o) in builder.target.games.iter().zip(&root) {
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
            let p = match builder.build(&root, m.n, None, Some((above, 0, 0))) {
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
            );
            if let Some(pattern) = p {
                let mut l = Leaf {
                    root,
                    above,
                    mask: 0,
                    selected: 0,
                    ambiguous,
                    max_exceptions,
                    pattern,
                    frozen: false,
                    score: 0.,
                    rank_hint_next_team: None,
                };
                let timer = Instant::now();
                let (w, n) = train(m, cell, &mut builder.goals, &mut l, seed, serial, pilot_n);
                serial += 1;
                training_ms += timer.elapsed().as_secs_f64() * 1000.;
                training_work += w;
                training_draws += n;
                if rank_priority {
                    let (next, hint, work) =
                        next_team_with_work(&builder, &l, m.n, true, marginal_cache.as_mut());
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
                );
                leaves.push(l);
            }
        }
        builder.node_limit = 100000.min(builder.nodes.saturating_add(split_quota));
        let initial = leaves.len();
        let (mut splits, mut failed, mut refuted) = (0, 0, 0);
        let mut attempts = 0;
        while leaves.len() < leaf_limit && attempts < 2 * leaf_limit {
            let chosen = leaves
                .iter()
                .enumerate()
                .filter(|(_, l)| !l.frozen)
                .max_by(|(a, x), (b, y)| {
                    x.score
                        .total_cmp(&y.score)
                        .then(x.pattern.mass.total_cmp(&y.pattern.mass))
                        .then(b.cmp(a))
                })
                .map(|(i, _)| i);
            let Some(index) = chosen else { break };
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
                );
                match build_result {
                    Ok(Some(pattern)) => children.push(Leaf {
                        root: root.clone(),
                        above,
                        mask: child_mask,
                        selected,
                        ambiguous: leaves[index].ambiguous,
                        max_exceptions: leaves[index].max_exceptions,
                        pattern,
                        frozen: false,
                        score: 0.,
                        rank_hint_next_team: None,
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
            );
            return Err("tree has no usable leaves; not a cell impossibility claim".into());
        }
        sync_modeled_tree_work(
            measured,
            consumed_work,
            &builder,
            bootstrap_work,
            enumeration_work,
            training_work,
            rank_hint_work,
        );
        let charged_tree_work = accounted_tree_work(training_work, rank_hint_work, rank_priority);
        let modeled_setup_work = modeled_tree_work(
            &builder,
            bootstrap_work,
            enumeration_work,
            training_work,
            rank_hint_work,
        );
        let metadata = json!({"rank_hint_priority":rank_priority,"bootstrap_work_units":bootstrap_work,"root_enumeration_work_units":enumeration_work,"rank_hint_work_units":rank_hint_work,"training_work":training_work,"modeled_setup_work":modeled_setup_work,"initial_leaves":initial,"leaf_limit":leaf_limit,"leaves":leaves.len(),"splits":splits,"attempts":attempts,"failed_splits":failed,"refuted_children":refuted,"joint_nodes":builder.nodes,"guide_limit":guide_limit,"guide_values":builder.guide_values,"training_draws":training_draws,"training_ms":training_ms,"setup_ms":clock.elapsed().as_secs_f64()*1000.,"selected":leaves.iter().map(|l|json!({"root":l.root,"selected":(0..m.n).filter(|&t|l.selected&(1<<t)!=0).map(|t|m.ids[t]).collect::<Vec<_>>(),"strict":(0..m.n).filter(|&t|l.mask&(1<<t)!=0).map(|t|m.ids[t]).collect::<Vec<_>>() })).collect::<Vec<_>>()});
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
        let goal_tilt_requested = crate::rare_tail::value("RUST_ODDS_LAZY_GOALS") == "1";
        let Some(builder) = LazyJoint::with_mode(m, cell, seed, &[], 0, false, false, None) else {
            let work = lazy_bootstrap_work_estimate(m, cell, goal_tilt_requested);
            return (Err("unsupported tree model".into()), work);
        };
        let bootstrap_work = lazy_bootstrap_work_estimate(m, cell, goal_tilt_requested);
        let (roots, enumeration_work) =
            keys_with_limit_measured(&builder.target, cell.team, limit, true);
        let work = crate::rare_tail::budget::setup_cost(builder.nodes, builder.guide_values)
            .saturating_add(bootstrap_work)
            .saturating_add(enumeration_work);
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
