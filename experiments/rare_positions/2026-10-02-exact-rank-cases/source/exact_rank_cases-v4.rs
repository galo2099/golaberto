//! Offline exact weighted counting of certified points/wins rank cases.
//! This executable consumes a saved request and certified-path census. It does
//! not sample, mutate production behavior, or construct score-level outcomes.
use golaberto_odds::{
    domains::Domains,
    lookahead::RankGame,
    model::{Key, Model, Request},
    search::{parallel, Cell},
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    env, fs,
    time::Instant,
};

type Profile = BTreeMap<(usize, usize), f64>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Category {
    Zero,
    One,
    Two,
    ThreeDraw,
    ThreeWin,
    All,
}
impl Category {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "0" => Ok(Self::Zero),
            "1" => Ok(Self::One),
            "2" => Ok(Self::Two),
            "3draw" => Ok(Self::ThreeDraw),
            "3win" => Ok(Self::ThreeWin),
            "all" => Ok(Self::All),
            _ => Err(format!("unknown category {s:?}")),
        }
    }
    fn matches(self, points: i32, wins: i32) -> bool {
        match self {
            Self::Zero => points == 0 && wins == 0,
            Self::One => points == 1 && wins == 0,
            Self::Two => points == 2 && wins == 0,
            Self::ThreeDraw => points == 3 && wins == 0,
            Self::ThreeWin => points == 3 && wins == 1,
            Self::All => true,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Zero => "0",
            Self::One => "1",
            Self::Two => "2",
            Self::ThreeDraw => "3draw",
            Self::ThreeWin => "3win",
            Self::All => "all",
        }
    }
}
fn category_for(points: i32, wins: i32) -> Result<&'static str, String> {
    match (points, wins) {
        (0, 0) => Ok("0"),
        (1, 0) => Ok("1"),
        (2, 0) => Ok("2"),
        (3, 0) => Ok("3draw"),
        (3, 1) => Ok("3win"),
        other => Err(format!("unsupported target increment {other:?}")),
    }
}

#[derive(Clone, Copy)]
struct Limits {
    nodes: usize,
    memo_entries: usize,
}
#[derive(Clone)]
struct Edge {
    index: usize,
    home: usize,
    away: usize,
    prob: [f64; 3],
    hg: [i32; 3],
    ag: [i32; 3],
    mask: u8,
    track_home: bool,
    track_away: bool,
}
struct ReducedGraph {
    edges: Vec<Edge>,
    integrated_mass: f64,
    preclassified: Vec<bool>,
    fixed_profile: (usize, usize),
    reduced_rivals: usize,
    active_team_mask: Vec<bool>,
}

fn reduce_fixed_status(
    raw_edges: &[Edge],
    n: usize,
    target_team: usize,
    base: &[i32],
    lower: &[i32],
    upper: &[i32],
    target: i32,
    enabled: bool,
) -> ReducedGraph {
    let mut residual_min = vec![0i32; n];
    let mut residual_max = vec![0i32; n];
    for e in raw_edges {
        let os: Vec<_> = (0..3).filter(|&o| e.mask & (1 << o) != 0).collect();
        residual_min[e.home] += os.iter().map(|&o| e.hg[o]).min().unwrap();
        residual_max[e.home] += os.iter().map(|&o| e.hg[o]).max().unwrap();
        residual_min[e.away] += os.iter().map(|&o| e.ag[o]).min().unwrap();
        residual_max[e.away] += os.iter().map(|&o| e.ag[o]).max().unwrap();
    }
    let mut active = vec![true; n];
    active[target_team] = false;
    let mut preclassified = vec![false; n];
    let mut fixed_profile = (0usize, 0usize);
    let mut reduced_rivals = 0;
    if enabled {
        for t in 0..n {
            if t == target_team {
                continue;
            }
            let min_total = base[t] + residual_min[t];
            let max_total = base[t] + residual_max[t];
            if min_total < lower[t] || max_total > upper[t] {
                continue;
            }
            let status = if max_total < target {
                Some(0)
            } else if min_total > target {
                Some(1)
            } else if min_total == target && max_total == target {
                Some(2)
            } else {
                None
            };
            if let Some(status) = status {
                active[t] = false;
                preclassified[t] = true;
                reduced_rivals += 1;
                if status == 1 {
                    fixed_profile.0 += 1;
                }
                if status == 2 {
                    fixed_profile.1 += 1;
                }
            }
        }
    }
    let mut edges = Vec::new();
    let mut integrated_mass = 1.0;
    for raw in raw_edges {
        let track_home = active[raw.home];
        let track_away = active[raw.away];
        if !track_home && !track_away {
            integrated_mass *= (0..3)
                .filter(|&o| raw.mask & (1 << o) != 0)
                .map(|o| raw.prob[o])
                .sum::<f64>();
        } else {
            let mut e = raw.clone();
            e.track_home = track_home;
            e.track_away = track_away;
            edges.push(e);
        }
    }
    ReducedGraph {
        edges,
        integrated_mass,
        preclassified,
        fixed_profile,
        reduced_rivals,
        active_team_mask: active,
    }
}
#[derive(Clone)]
struct UnaryTeam {
    team: usize,
    gains: BTreeMap<i32, f64>,
    min_gain: i32,
    max_gain: i32,
    allowed_mass: f64,
}
#[derive(Clone)]
struct Component {
    edges: Vec<Edge>,
    unary: Vec<UnaryTeam>,
    teams: Vec<usize>,
    order_width: usize,
    fixed_profile: (usize, usize),
    domain_upper_mass: f64,
}
struct PreparedCase {
    id: usize,
    path_index: usize,
    case_index: usize,
    category: String,
    target_prior: f64,
    forced_mass: f64,
    integrated_mass: f64,
    base_weight: f64,
    target_total: i32,
    rank: usize,
    base: Vec<i32>,
    lower: Vec<i32>,
    upper: Vec<i32>,
    components: Vec<Component>,
    census: Value,
    estimated_cost: (usize, usize, usize, usize),
}
#[derive(Default)]
struct CountStats {
    nodes: usize,
    memo_hits: usize,
    memo_entries: usize,
    pruned: usize,
    cutoffs: usize,
    terminal_cache_hits: usize,
    terminal_cache_misses: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CutoffBound {
    Domain,
    Unary,
}
struct CountResult {
    profiles: Profile,
    unknown_upper: f64,
    stats: CountStats,
}

fn val_i64(v: &Value, name: &str) -> Result<i64, String> {
    v.get(name)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("missing/integer field {name}"))
}
fn val_usize(v: &Value, name: &str) -> Result<usize, String> {
    usize::try_from(val_i64(v, name)?).map_err(|_| format!("invalid nonnegative field {name}"))
}
fn val_f64(v: &Value, name: &str) -> Result<f64, String> {
    v.get(name)
        .and_then(Value::as_f64)
        .filter(|x| x.is_finite() && *x >= 0.)
        .ok_or_else(|| format!("missing/invalid probability field {name}"))
}
fn ids(v: &Value, name: &str) -> Result<Vec<i32>, String> {
    v.get(name)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing array field {name}"))?
        .iter()
        .map(|x| {
            x.as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .ok_or_else(|| format!("invalid team id in {name}"))
        })
        .collect()
}
fn outcomes(v: &Value) -> Result<Vec<u8>, String> {
    v.as_array()
        .ok_or_else(|| "target path is not an array".to_string())?
        .iter()
        .map(|x| {
            x.as_u64()
                .filter(|n| *n < 3)
                .map(|n| n as u8)
                .ok_or_else(|| "target path outcome must be 0, 1, or 2".to_string())
        })
        .collect()
}
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-10 * a.abs().max(b.abs()).max(1e-30)
}
fn case_upper_mass(target_prior: f64, forced_mass: f64, variable_allowed_mass: f64) -> f64 {
    target_prior * forced_mass * variable_allowed_mass
}
fn json_f64(v: &Value, name: &str) -> Result<f64, String> {
    v.get(name)
        .and_then(Value::as_f64)
        .filter(|x| x.is_finite() && *x >= 0.)
        .ok_or_else(|| format!("missing/invalid numeric field {name}"))
}

fn make_games(m: &Model, stride: i32) -> Vec<RankGame> {
    let use_wins = m.keys.get(1) == Some(&Key::W);
    let win = 3 * stride + i32::from(use_wins);
    m.fixtures
        .iter()
        .enumerate()
        .map(|(index, g)| RankGame {
            index,
            home: g.home,
            away: g.away,
            prob: g.prob,
            hg: [0, stride, win],
            ag: [win, stride, 0],
        })
        .collect()
}

fn components(
    edges: Vec<Edge>,
    n: usize,
    target_team: usize,
    base: &[i32],
    target: i32,
    preclassified: &[bool],
    reduced_profile: (usize, usize),
    integrate_unary: bool,
) -> Vec<Component> {
    let mut shared_edges = Vec::new();
    let mut unary_fixtures: BTreeMap<usize, Vec<Edge>> = BTreeMap::new();
    for edge in edges {
        if integrate_unary && edge.track_home ^ edge.track_away {
            let team = if edge.track_home {
                edge.home
            } else {
                edge.away
            };
            unary_fixtures.entry(team).or_default().push(edge);
        } else {
            shared_edges.push(edge);
        }
    }
    let mut unary_by_team = BTreeMap::<usize, UnaryTeam>::new();
    for (team, fixtures) in unary_fixtures {
        let mut gains = BTreeMap::from([(0i32, 1.0)]);
        for e in fixtures {
            let home_side = e.track_home;
            let mut next = BTreeMap::<i32, f64>::new();
            for (&prior_gain, &prior_mass) in &gains {
                for o in 0..3 {
                    if e.mask & (1 << o) == 0 || e.prob[o] == 0. {
                        continue;
                    }
                    let gain = if home_side { e.hg[o] } else { e.ag[o] };
                    *next.entry(prior_gain + gain).or_default() += prior_mass * e.prob[o];
                }
            }
            gains = next;
        }
        let min_gain = gains.keys().next().copied().unwrap_or(0);
        let max_gain = gains.keys().next_back().copied().unwrap_or(0);
        let allowed_mass = gains.values().sum();
        unary_by_team.insert(
            team,
            UnaryTeam {
                team,
                gains,
                min_gain,
                max_gain,
                allowed_mass,
            },
        );
    }
    let mut team_edges = vec![Vec::<usize>::new(); n];
    for (i, edge) in shared_edges.iter().enumerate() {
        if edge.track_home {
            team_edges[edge.home].push(i);
        }
        if edge.track_away {
            team_edges[edge.away].push(i);
        }
    }
    let mut seen_edge = vec![false; shared_edges.len()];
    let mut seen_team = vec![false; n];
    let mut result = Vec::new();
    for first in 0..shared_edges.len() {
        if seen_edge[first] {
            continue;
        }
        let first_edge = &shared_edges[first];
        let mut queue = VecDeque::new();
        if first_edge.track_home {
            queue.push_back(first_edge.home);
        }
        if first_edge.track_away {
            queue.push_back(first_edge.away);
        }
        let mut edge_ids = HashSet::new();
        let mut teams = HashSet::new();
        while let Some(team) = queue.pop_front() {
            if !teams.insert(team) {
                continue;
            }
            for &ei in &team_edges[team] {
                if edge_ids.insert(ei) {
                    let e = &shared_edges[ei];
                    if e.track_home {
                        queue.push_back(e.home);
                    }
                    if e.track_away {
                        queue.push_back(e.away);
                    }
                }
            }
        }
        for &ei in &edge_ids {
            seen_edge[ei] = true;
        }
        for &team in &teams {
            seen_team[team] = true;
        }
        let mut edge_ids: Vec<_> = edge_ids.into_iter().collect();
        edge_ids.sort_by_key(|&i| shared_edges[i].index);
        let part: Vec<_> = edge_ids
            .into_iter()
            .map(|i| shared_edges[i].clone())
            .collect();
        let (ordered, width) = greedy_order(part, n);
        let mut team_ids: Vec<_> = teams.into_iter().collect();
        team_ids.sort_unstable();
        let unary: Vec<_> = team_ids
            .iter()
            .filter_map(|t| unary_by_team.get(t).cloned())
            .collect();
        let fixed_profile = (0, 0);
        let shared_mass: f64 = ordered
            .iter()
            .map(|e| {
                (0..3)
                    .filter(|&o| e.mask & (1 << o) != 0)
                    .map(|o| e.prob[o])
                    .sum::<f64>()
            })
            .product();
        let upper_mass = shared_mass * unary.iter().map(|u| u.allowed_mass).product::<f64>();
        result.push(Component {
            edges: ordered,
            unary,
            teams: team_ids,
            order_width: width,
            fixed_profile,
            domain_upper_mass: upper_mass,
        });
    }
    for (team, unary) in unary_by_team {
        if !seen_team[team] {
            seen_team[team] = true;
            result.push(Component {
                edges: Vec::new(),
                teams: vec![team],
                order_width: 0,
                fixed_profile: (0, 0),
                domain_upper_mass: unary.allowed_mass,
                unary: vec![unary],
            });
        }
    }
    // Teams without variable fixtures have a fully fixed comparison status.
    let mut isolated = reduced_profile;
    for t in 0..n {
        if t != target_team && !seen_team[t] && !preclassified[t] {
            let value = base[t];
            if value > target {
                isolated.0 += 1;
            } else if value == target {
                isolated.1 += 1;
            }
        }
    }
    if isolated != (0, 0) {
        result.push(Component {
            edges: Vec::new(),
            unary: Vec::new(),
            teams: Vec::new(),
            order_width: 0,
            fixed_profile: isolated,
            domain_upper_mass: 1.,
        });
    }
    result.sort_by_key(|c| c.edges.first().map_or(usize::MAX, |e| e.index));
    result
}

fn greedy_order(mut edges: Vec<Edge>, n: usize) -> (Vec<Edge>, usize) {
    let mut ordered = Vec::with_capacity(edges.len());
    let mut used = vec![false; n];
    let mut max_frontier = 0;
    while !edges.is_empty() {
        let mut best = 0;
        let mut best_width = usize::MAX;
        for (candidate, edge) in edges.iter().enumerate() {
            let mut after = used.clone();
            if edge.track_home {
                after[edge.home] = true;
            }
            if edge.track_away {
                after[edge.away] = true;
            }
            let mut incident_left = vec![false; n];
            for (i, other) in edges.iter().enumerate() {
                if i == candidate {
                    continue;
                }
                if other.track_home {
                    incident_left[other.home] = true;
                }
                if other.track_away {
                    incident_left[other.away] = true;
                }
            }
            let width = (0..n).filter(|&t| after[t] && incident_left[t]).count();
            if (width, edge.index) < (best_width, edges[best].index) {
                best_width = width;
                best = candidate;
            }
        }
        let edge = edges.remove(best);
        if edge.track_home {
            used[edge.home] = true;
        }
        if edge.track_away {
            used[edge.away] = true;
        }
        let frontier = (0..n)
            .filter(|&t| {
                used[t]
                    && edges
                        .iter()
                        .any(|e| (e.track_home && e.home == t) || (e.track_away && e.away == t))
            })
            .count();
        max_frontier = max_frontier.max(frontier);
        ordered.push(edge);
    }
    (ordered, max_frontier)
}

fn prepare_case(
    m: &Model,
    target_games: &[RankGame],
    remaining: &[RankGame],
    cell: Cell,
    stride: i32,
    row: &Value,
    path_index: usize,
    case_index: usize,
    case_ids: &[i32],
    id: usize,
    category_name: &str,
    reduction_fixed: bool,
    integrate_unary: bool,
) -> Result<PreparedCase, String> {
    let path = outcomes(row.get("path").ok_or("row has no path")?)?;
    if path.len() != target_games.len() {
        return Err(format!("row {path_index}: target path length mismatch"));
    }
    let mut points: Vec<i32> = m
        .base
        .iter()
        .map(|c| {
            c.points * stride
                + if m.keys.get(1) == Some(&Key::W) {
                    c.wins
                } else {
                    0
                }
        })
        .collect();
    let mut added_points = 0;
    let mut added_wins = 0;
    let mut target_prior = 1.0;
    for (g, &o) in target_games.iter().zip(&path) {
        if g.prob[o as usize] <= 0. {
            return Err(format!(
                "row {path_index}: path uses zero-prior target outcome"
            ));
        }
        target_prior *= g.prob[o as usize];
        points[g.home] += g.hg[o as usize];
        points[g.away] += g.ag[o as usize];
        if g.home == cell.team {
            added_points += [0, 1, 3][o as usize];
            added_wins += i32::from(o == 2);
        } else {
            added_points += [3, 1, 0][o as usize];
            added_wins += i32::from(o == 0);
        }
    }
    let target_total = points[cell.team];
    if points[cell.team] / stride != val_i64(row, "target_points")? as i32
        || (m.keys.get(1) == Some(&Key::W)
            && points[cell.team] % stride != val_i64(row, "target_wins")? as i32)
    {
        return Err(format!(
            "row {path_index}: recomputed target points/wins disagree"
        ));
    }
    let row_prior = json_f64(row, "prior")?;
    if !close(row_prior, target_prior) {
        return Err(format!("row {path_index}: target prior mismatch"));
    }
    let comp = Category::parse(category_name)?;
    if !comp.matches(added_points, added_wins) {
        return Err(format!("row {path_index}: category mismatch"));
    }

    let rivals: Vec<_> = (0..m.n).filter(|&t| t != cell.team).collect();
    let initial = Domains::propagate(remaining, &points, &rivals, cell.rank, target_total);
    if !initial.feasible {
        return Err(format!(
            "row {path_index}: source case claims infeasible path"
        ));
    }
    let fixed_above: Vec<_> = rivals
        .iter()
        .copied()
        .filter(|&t| points[t] + initial.min[0][t] > target_total)
        .collect();
    let fixed_below: Vec<_> = rivals
        .iter()
        .copied()
        .filter(|&t| points[t] + initial.max[0][t] < target_total)
        .collect();
    let ambiguous: Vec<_> = rivals
        .iter()
        .copied()
        .filter(|&t| {
            points[t] + initial.min[0][t] <= target_total
                && points[t] + initial.max[0][t] >= target_total
        })
        .collect();
    let src_ambiguous = ids(row, "ambiguous")?;
    let actual_ambiguous: Vec<_> = ambiguous.iter().map(|&t| m.ids[t]).collect();
    if src_ambiguous != actual_ambiguous
        || val_usize(row, "fixed_above")? != fixed_above.len()
        || val_usize(row, "fixed_below")? != fixed_below.len()
        || row.get("above").and_then(Value::as_bool) != Some(false)
    {
        return Err(format!(
            "row {path_index}: source comparison partition mismatch"
        ));
    }
    let selected: HashSet<i32> = case_ids.iter().copied().collect();
    if selected.len() != case_ids.len() || case_ids.iter().any(|x| !src_ambiguous.contains(x)) {
        return Err(format!(
            "row {path_index} case {case_index}: strict-below IDs not a unique ambiguous subset"
        ));
    }
    let mut lower = initial.lower.clone();
    let mut upper = initial.upper.clone();
    for &t in &fixed_above {
        lower[t] = lower[t].max(target_total + 1);
    }
    for &t in &fixed_below {
        upper[t] = upper[t].min(target_total - 1);
    }
    for &t in &ambiguous {
        if selected.contains(&m.ids[t]) {
            upper[t] = upper[t].min(target_total - 1);
        } else {
            lower[t] = lower[t].max(target_total);
        }
    }
    let d = Domains::condition(
        remaining,
        &points,
        &rivals,
        cell.rank,
        target_total,
        &lower,
        &upper,
        &initial.domains,
    );
    if !d.feasible {
        return Err(format!(
            "row {path_index} case {case_index}: source case is infeasible"
        ));
    }
    let lower = d.lower.clone();
    let upper = d.upper.clone();
    let mut forced_mass = 1.0;
    let mut fixed_points = points.clone();
    let mut variable_games = Vec::new();
    let mut forced_fixture_count = 0;
    let mut variable_allowed_product = 1.0;
    for (i, g) in remaining.iter().enumerate() {
        let mask = d.domains[i];
        let allowed_mass: f64 = (0..3)
            .filter(|&o| mask & (1 << o) != 0)
            .map(|o| g.prob[o])
            .sum();
        if mask.count_ones() == 1 {
            let o = mask.trailing_zeros() as usize;
            forced_mass *= g.prob[o];
            forced_fixture_count += 1;
            fixed_points[g.home] += g.hg[o];
            fixed_points[g.away] += g.ag[o];
        } else {
            variable_allowed_product *= allowed_mass;
            variable_games.push((g, mask, allowed_mass));
        }
    }
    let variable_fixture_count = variable_games.len();
    let raw_edges: Vec<_> = variable_games
        .iter()
        .map(|(g, mask, _)| Edge {
            index: g.index,
            home: g.home,
            away: g.away,
            prob: g.prob,
            hg: g.hg,
            ag: g.ag,
            mask: *mask,
            track_home: true,
            track_away: true,
        })
        .collect();
    let reduced = reduce_fixed_status(
        &raw_edges,
        m.n,
        cell.team,
        &fixed_points,
        &lower,
        &upper,
        target_total,
        reduction_fixed,
    );
    let integrated_mass = reduced.integrated_mass;
    let reduced_rivals = reduced.reduced_rivals;
    let active_team_mask = reduced.active_team_mask;
    let retained_fixture_count = reduced.edges.len();
    let integrated_fixture_count = variable_fixture_count.saturating_sub(retained_fixture_count);
    let unary_fixture_count = reduced
        .edges
        .iter()
        .filter(|e| e.track_home ^ e.track_away)
        .count();
    let shared_fixture_count = retained_fixture_count - unary_fixture_count;
    let parts = components(
        reduced.edges,
        m.n,
        cell.team,
        &fixed_points,
        target_total,
        &reduced.preclassified,
        reduced.fixed_profile,
        integrate_unary,
    );
    let constrained_teams = rivals
        .iter()
        .filter(|&&t| lower[t] != i32::MIN / 4 || upper[t] != i32::MAX / 4)
        .count();
    let component_sizes: Vec<_> = parts.iter().map(|c| c.teams.len()).collect();
    let widths: Vec<_> = parts.iter().map(|c| c.order_width).collect();
    let branch_fixture_count: usize = parts.iter().map(|c| c.edges.len()).sum();
    let integrated_unary_count: usize = parts.iter().map(|c| c.unary.len()).sum();
    let unary_gain_table_entries: usize = parts
        .iter()
        .flat_map(|c| &c.unary)
        .map(|u| u.gains.len())
        .sum();
    let base_weight = target_prior * forced_mass * integrated_mass;
    let census_upper = case_upper_mass(target_prior, forced_mass, variable_allowed_product);
    let census = json!({
        "case_id":id,"path_index":path_index,"case_index":case_index,"category":category_name,
        "case_ids":case_ids,"target_prior":target_prior,"forced_mass":forced_mass,
        "variable_allowed_probability_product":variable_allowed_product,"case_upper_mass":census_upper,
        "fixed_fixtures":forced_fixture_count,"variable_fixtures":variable_fixture_count,
        "integrated_probability_mass":integrated_mass,"reduction_fixed":reduction_fixed,"reduced_rivals":reduced_rivals,
        "active_team_mask":active_team_mask,"domain_masks":d.domains,
        "retained_fixture_count":retained_fixture_count,"shared_fixture_count":shared_fixture_count,
        "unary_fixture_count":unary_fixture_count,"integrated_fixture_count":integrated_fixture_count,
        "unary_integrated_team_count":integrated_unary_count,"branch_fixture_count":branch_fixture_count,
        "unary_gain_table_entries":unary_gain_table_entries,
        "unary_mode":if integrate_unary {"integrate"} else {"enumerate"},
        "constrained_teams":constrained_teams,"component_count":component_sizes.len(),
        "component_team_counts":component_sizes,"frontier_widths":widths
    });
    Ok(PreparedCase {
        id,
        path_index,
        case_index,
        category: category_name.to_string(),
        target_prior,
        forced_mass,
        integrated_mass,
        base_weight,
        target_total,
        rank: cell.rank,
        base: fixed_points,
        lower,
        upper,
        components: parts,
        census,
        estimated_cost: (
            branch_fixture_count,
            widths.iter().copied().max().unwrap_or(0),
            constrained_teams,
            id,
        ),
    })
}

fn classify_profile((above, equal): (usize, usize), rank: usize) -> &'static str {
    let pos0 = rank;
    if equal == 0 && above == pos0 {
        "rank_settled"
    } else if equal > 0 && above <= pos0 && pos0 <= above + equal {
        "tie_possible"
    } else {
        "rank_impossible"
    }
}

type MemoKey = (usize, Vec<i32>);
fn unary_terminal_profile(
    team: usize,
    current_total: i32,
    unary_by_team: &HashMap<usize, &UnaryTeam>,
    lower: &[i32],
    upper: &[i32],
    target: i32,
    cache: &mut HashMap<(usize, i32), Profile>,
    stats: &mut CountStats,
) -> Profile {
    let key = (team, current_total);
    if let Some(profile) = cache.get(&key) {
        stats.terminal_cache_hits += 1;
        return profile.clone();
    }
    stats.terminal_cache_misses += 1;
    let mut profile = Profile::new();
    if let Some(unary) = unary_by_team.get(&team) {
        for (&gain, &mass) in &unary.gains {
            let total = current_total + gain;
            if total < lower[team] || total > upper[team] {
                continue;
            }
            let status = if total > target {
                (1, 0)
            } else if total == target {
                (0, 1)
            } else {
                (0, 0)
            };
            *profile.entry(status).or_default() += mass;
        }
    } else if current_total >= lower[team] && current_total <= upper[team] {
        let status = if current_total > target {
            (1, 0)
        } else if current_total == target {
            (0, 1)
        } else {
            (0, 0)
        };
        profile.insert(status, 1.);
    }
    cache.insert(key, profile.clone());
    profile
}

fn cutoff_upper_mass(
    i: usize,
    totals: &[i32],
    active: &[Vec<usize>],
    domain_suffix_mass: &[f64],
    shared_suffix_mass: &[f64],
    shared_suffix_min: &[Vec<i32>],
    shared_suffix_max: &[Vec<i32>],
    unary_by_team: &HashMap<usize, &UnaryTeam>,
    lower: &[i32],
    upper: &[i32],
    mode: CutoffBound,
) -> f64 {
    if mode == CutoffBound::Domain {
        return domain_suffix_mass[i];
    }
    let mut mass = shared_suffix_mass[i];
    for &team in &active[i] {
        let minimum_shared = shared_suffix_min[i][team];
        let maximum_shared = shared_suffix_max[i][team];
        let minimum_unary = lower[team] - totals[team] - maximum_shared;
        let maximum_unary = upper[team] - totals[team] - minimum_shared;
        let allowed_unary = if minimum_unary > maximum_unary {
            0.0
        } else if let Some(unary) = unary_by_team.get(&team) {
            unary
                .gains
                .range(minimum_unary..=maximum_unary)
                .map(|(_, mass)| *mass)
                .sum::<f64>()
        } else if minimum_unary <= 0 && 0 <= maximum_unary {
            1.0
        } else {
            0.0
        };
        mass *= allowed_unary;
        if mass == 0.0 {
            break;
        }
    }
    mass
}

fn count_component(
    c: &Component,
    n: usize,
    base: &[i32],
    lower: &[i32],
    upper: &[i32],
    target: i32,
    limits: Limits,
    cutoff_bound: CutoffBound,
    stats: &mut CountStats,
) -> (Profile, f64, f64) {
    let unary_by_team: HashMap<usize, &UnaryTeam> = c.unary.iter().map(|u| (u.team, u)).collect();
    let mut terminal_cache = HashMap::<(usize, i32), Profile>::new();
    if c.edges.is_empty() {
        let mut p = BTreeMap::from([(c.fixed_profile, 1.)]);
        for u in &c.unary {
            let team_profile = unary_terminal_profile(
                u.team,
                base[u.team],
                &unary_by_team,
                lower,
                upper,
                target,
                &mut terminal_cache,
                stats,
            );
            p = convolve(&p, &team_profile);
        }
        let solved = p.values().sum::<f64>();
        return (p, 0., c.domain_upper_mass.max(solved));
    }
    let edges = &c.edges;
    let mut last = vec![None; n];
    for (i, e) in edges.iter().enumerate() {
        if e.track_home {
            last[e.home] = Some(i);
        }
        if e.track_away {
            last[e.away] = Some(i);
        }
    }
    let mut suffix_min = vec![vec![0i32; n]; edges.len() + 1];
    let mut suffix_max = suffix_min.clone();
    let mut suffix_mass = vec![1.0; edges.len() + 1];
    for i in (0..edges.len()).rev() {
        suffix_min[i] = suffix_min[i + 1].clone();
        suffix_max[i] = suffix_max[i + 1].clone();
        let e = &edges[i];
        let outcomes: Vec<_> = (0..3).filter(|&o| e.mask & (1 << o) != 0).collect();
        if e.track_home {
            suffix_min[i][e.home] += outcomes.iter().map(|&o| e.hg[o]).min().unwrap();
            suffix_max[i][e.home] += outcomes.iter().map(|&o| e.hg[o]).max().unwrap();
        }
        if e.track_away {
            suffix_min[i][e.away] += outcomes.iter().map(|&o| e.ag[o]).min().unwrap();
            suffix_max[i][e.away] += outcomes.iter().map(|&o| e.ag[o]).max().unwrap();
        }
        suffix_mass[i] = suffix_mass[i + 1] * outcomes.iter().map(|&o| e.prob[o]).sum::<f64>();
    }
    let shared_suffix_min = suffix_min.clone();
    let shared_suffix_max = suffix_max.clone();
    let shared_suffix_mass = suffix_mass.clone();
    for u in &c.unary {
        for i in 0..=edges.len() {
            suffix_min[i][u.team] += u.min_gain;
            suffix_max[i][u.team] += u.max_gain;
        }
    }
    let active: Vec<Vec<usize>> = (0..=edges.len())
        .map(|step| {
            (0..n)
                .filter(|&t| last[t].map_or(false, |x| x >= step))
                .collect()
        })
        .collect();
    for i in 0..=edges.len() {
        suffix_mass[i] *= active[i]
            .iter()
            .filter_map(|t| unary_by_team.get(t).map(|u| u.allowed_mass))
            .product::<f64>();
    }
    let mut memo: HashMap<MemoKey, (Profile, f64)> = HashMap::new();
    let mut totals = base.to_vec();
    fn recurse(
        i: usize,
        edges: &[Edge],
        active: &[Vec<usize>],
        last: &[Option<usize>],
        totals: &mut [i32],
        lower: &[i32],
        upper: &[i32],
        target: i32,
        suffix_min: &[Vec<i32>],
        suffix_max: &[Vec<i32>],
        domain_suffix_mass: &[f64],
        shared_suffix_mass: &[f64],
        shared_suffix_min: &[Vec<i32>],
        shared_suffix_max: &[Vec<i32>],
        limits: Limits,
        cutoff_bound: CutoffBound,
        stats: &mut CountStats,
        memo: &mut HashMap<MemoKey, (Profile, f64)>,
        unary_by_team: &HashMap<usize, &UnaryTeam>,
        terminal_cache: &mut HashMap<(usize, i32), Profile>,
    ) -> (Profile, f64) {
        if i == edges.len() {
            return (BTreeMap::from([((0, 0), 1.)]), 0.);
        }
        let key = (i, active[i].iter().map(|&t| totals[t]).collect());
        if let Some(v) = memo.get(&key) {
            stats.memo_hits += 1;
            return v.clone();
        }
        if stats.nodes >= limits.nodes
            || stats.nodes >= limits.memo_entries
            || memo.len() >= limits.memo_entries
            || limits.nodes == 0
            || limits.memo_entries == 0
        {
            stats.cutoffs += 1;
            let cutoff_mass = cutoff_upper_mass(
                i,
                totals,
                active,
                domain_suffix_mass,
                shared_suffix_mass,
                shared_suffix_min,
                shared_suffix_max,
                unary_by_team,
                lower,
                upper,
                cutoff_bound,
            );
            return (Profile::new(), cutoff_mass);
        }
        stats.nodes += 1;
        for t in &active[i] {
            if totals[*t] + suffix_min[i][*t] > upper[*t]
                || totals[*t] + suffix_max[i][*t] < lower[*t]
            {
                stats.pruned += 1;
                return (Profile::new(), 0.);
            }
        }
        let e = &edges[i];
        let mut profiles = Profile::new();
        let mut unknown = 0.0;
        for o in 0..3 {
            if e.mask & (1 << o) == 0 || e.prob[o] == 0. {
                continue;
            }
            if e.track_home {
                totals[e.home] += e.hg[o];
            }
            if e.track_away {
                totals[e.away] += e.ag[o];
            }
            let mut valid = true;
            for t in &active[i + 1] {
                if totals[*t] + suffix_min[i + 1][*t] > upper[*t]
                    || totals[*t] + suffix_max[i + 1][*t] < lower[*t]
                {
                    valid = false;
                    break;
                }
            }
            if valid {
                let mut closure_profile = BTreeMap::from([((0usize, 0usize), 1.0)]);
                let mut closure_valid = true;
                for t in [e.home, e.away] {
                    if (t == e.home && !e.track_home) || (t == e.away && !e.track_away) {
                        continue;
                    }
                    if last[t] == Some(i) {
                        let team_profile = unary_terminal_profile(
                            t,
                            totals[t],
                            unary_by_team,
                            lower,
                            upper,
                            target,
                            terminal_cache,
                            stats,
                        );
                        if team_profile.is_empty() {
                            closure_valid = false;
                            break;
                        }
                        closure_profile = convolve(&closure_profile, &team_profile);
                    }
                }
                let (sub, sub_unknown) = if closure_valid {
                    recurse(
                        i + 1,
                        edges,
                        active,
                        last,
                        totals,
                        lower,
                        upper,
                        target,
                        suffix_min,
                        suffix_max,
                        domain_suffix_mass,
                        shared_suffix_mass,
                        shared_suffix_min,
                        shared_suffix_max,
                        limits,
                        cutoff_bound,
                        stats,
                        memo,
                        unary_by_team,
                        terminal_cache,
                    )
                } else {
                    (Profile::new(), 0.)
                };
                let closure_mass = closure_profile.values().sum::<f64>();
                for ((a, equal), mass) in &sub {
                    for ((ca, ce), cmass) in &closure_profile {
                        *profiles.entry((a + ca, equal + ce)).or_default() +=
                            e.prob[o] * mass * cmass;
                    }
                }
                unknown += e.prob[o] * closure_mass * sub_unknown;
            } else {
                stats.pruned += 1;
            }
            if e.track_home {
                totals[e.home] -= e.hg[o];
            }
            if e.track_away {
                totals[e.away] -= e.ag[o];
            }
        }
        if memo.len() < limits.memo_entries {
            memo.insert(key, (profiles.clone(), unknown));
            stats.memo_entries += 1;
        }
        (profiles, unknown)
    }
    let (profiles, unknown) = recurse(
        0,
        edges,
        &active,
        &last,
        &mut totals,
        lower,
        upper,
        target,
        &suffix_min,
        &suffix_max,
        &suffix_mass,
        &shared_suffix_mass,
        &shared_suffix_min,
        &shared_suffix_max,
        limits,
        cutoff_bound,
        stats,
        &mut memo,
        &unary_by_team,
        &mut terminal_cache,
    );
    (profiles, unknown, c.domain_upper_mass)
}

fn count_case(c: &PreparedCase, n: usize, limits: Limits, cutoff_bound: CutoffBound) -> Value {
    let started = Instant::now();
    let mut stats = CountStats::default();
    let mut profile: Profile = BTreeMap::from([((0, 0), 1.)]);
    let mut unknown = 0.0;
    for component in &c.components {
        let (next, component_unknown, component_upper) = count_component(
            component,
            n,
            &c.base,
            &c.lower,
            &c.upper,
            c.target_total,
            limits,
            cutoff_bound,
            &mut stats,
        );
        let solved_before: f64 = profile.values().sum();
        let solved_component: f64 = next.values().sum();
        unknown =
            unknown * (solved_component + component_unknown) + solved_before * component_unknown;
        profile = convolve(&profile, &next);
        let _ = component_upper;
    }
    let mut rank_mass = 0.;
    let mut tie_mass = 0.;
    let mut impossible_mass = 0.;
    let mut profiles_json = Vec::new();
    for (&(above, equal), &mass) in &profile {
        let class = classify_profile((above, equal), c.rank);
        match class {
            "rank_settled" => rank_mass += mass,
            "tie_possible" => tie_mass += mass,
            _ => impossible_mass += mass,
        }
        profiles_json.push(json!({"strict_above":above,"equal":equal,"class":class,"allowed_mass":mass,"case_mass":mass*c.base_weight}));
    }
    json!({
        "case_id":c.id,"path_index":c.path_index,"case_index":c.case_index,"category":c.category,
        "target_prior":c.target_prior,"forced_mass":c.forced_mass,"integrated_mass":c.integrated_mass,"base_weight":c.base_weight,
        "complete":stats.cutoffs == 0 && unknown == 0.,
        "rank_settled_mass":rank_mass*c.base_weight,"tie_possible_mass":tie_mass*c.base_weight,
        "rank_impossible_mass":impossible_mass*c.base_weight,"unknown_upper_mass":unknown*c.base_weight,
        "profiles":profiles_json,"nodes":stats.nodes,"memo_hits":stats.memo_hits,
        "memo_entries":stats.memo_entries,"pruned_states":stats.pruned,"cutoff_states":stats.cutoffs,
        "terminal_profile_cache_hits":stats.terminal_cache_hits,"terminal_profile_cache_misses":stats.terminal_cache_misses,
        "terminal_profile_cache_entries":stats.terminal_cache_misses,
        "elapsed_ms":started.elapsed().as_secs_f64()*1000.
    })
}

fn convolve(a: &Profile, b: &Profile) -> Profile {
    let mut out = Profile::new();
    for (&(aa, ae), &am) in a {
        for (&(ba, be), &bm) in b {
            *out.entry((aa + ba, ae + be)).or_default() += am * bm;
        }
    }
    out
}

fn run() -> Result<(), String> {
    let setup_started = Instant::now();
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        return Err("usage: exact_rank_cases REQUEST CASES_JSON OUTPUT [--category 0|1|2|3draw|3win|all] [--reduction none|fixed] [--unary enumerate|integrate] [--cutoff-bound domain|unary] [--max-cases N] [--nodes-per-case N] [--nodes-total N] [--memo-entries N] [--workers 1..4]".into());
    }
    let request: Request = serde_json::from_slice(&fs::read(&args[1]).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let m = Model::new(request).map_err(|e| format!("invalid request: {e}"))?;
    if m.ids.len() != m.n {
        return Err("model team ID count does not match group size".into());
    }
    let root: Value = serde_json::from_slice(&fs::read(&args[2]).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut category = Category::All;
    let mut max_cases = usize::MAX;
    let mut nodes_per_case = 20_000usize;
    let mut nodes_total = 100_000usize;
    let mut memo_entries = 100_000usize;
    let mut workers = 4usize;
    let mut reduction_fixed = true;
    let mut integrate_unary = false;
    let mut cutoff_bound = CutoffBound::Domain;
    let mut i = 4;
    while i < args.len() {
        if i + 1 >= args.len() {
            return Err(format!("missing value for {}", args[i]));
        }
        let value = &args[i + 1];
        match args[i].as_str() {
            "--category" => category = Category::parse(value)?,
            "--reduction" => {
                reduction_fixed = match value.as_str() {
                    "fixed" => true,
                    "none" => false,
                    _ => return Err("--reduction must be none or fixed".into()),
                }
            }
            "--unary" => {
                integrate_unary = match value.as_str() {
                    "enumerate" => false,
                    "integrate" => true,
                    _ => return Err("--unary must be enumerate or integrate".into()),
                }
            }
            "--cutoff-bound" => {
                cutoff_bound = match value.as_str() {
                    "domain" => CutoffBound::Domain,
                    "unary" => CutoffBound::Unary,
                    _ => return Err("--cutoff-bound must be domain or unary".into()),
                }
            }
            "--max-cases" => max_cases = value.parse().map_err(|_| "bad --max-cases")?,
            "--nodes-per-case" => {
                nodes_per_case = value.parse().map_err(|_| "bad --nodes-per-case")?
            }
            "--nodes-total" => nodes_total = value.parse().map_err(|_| "bad --nodes-total")?,
            "--memo-entries" => memo_entries = value.parse().map_err(|_| "bad --memo-entries")?,
            "--workers" => {
                workers = value.parse().map_err(|_| "bad --workers")?;
                if !(1..=4).contains(&workers) {
                    return Err("--workers must be 1..4".into());
                }
            }
            _ => return Err(format!("unknown argument {}", args[i])),
        }
        i += 2;
    }
    if m.keys.first() != Some(&Key::Pt)
        || m.keys.get(1) != Some(&Key::W)
        || (
            m.request.phase.championship.point_win,
            m.request.phase.championship.point_draw,
            m.request.phase.championship.point_loss,
        ) != (3, 1, 0)
        || m.request.phase.bonus_points != 0
    {
        return Err("exact rank cases currently requires points-first wins-second 3/1/0 with no bonus points".into());
    }
    let group = val_i64(&root, "group")?;
    if group != i64::from(m.request.id) {
        return Err("saved cases group does not match request".into());
    }
    let team_id = i32::try_from(val_i64(&root, "team")?).map_err(|_| "bad team id")?;
    let team = *m
        .indices
        .get(&team_id)
        .ok_or("saved target team missing from request")?;
    let rank1 = val_usize(&root, "rank")?;
    if rank1 == 0 || rank1 > m.n {
        return Err("saved rank outside group".into());
    }
    let cell = Cell {
        team,
        rank: rank1 - 1,
    };
    let rows = root
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("saved cases missing rows")?;
    let mut left = vec![0; m.ids.len()];
    for g in &m.fixtures {
        left[g.home] += 1;
        left[g.away] += 1;
    }
    let stride = m
        .base
        .iter()
        .zip(&left)
        .map(|(c, l)| c.wins + l)
        .max()
        .unwrap_or(0)
        + 1;
    if val_i64(&root, "stride")? != i64::from(stride) {
        return Err("saved cases stride does not match recomputed stride".into());
    }
    let all = make_games(&m, stride);
    let mut target_games = Vec::new();
    let mut remaining = Vec::new();
    for g in all {
        if g.home == cell.team || g.away == cell.team {
            target_games.push(g);
        } else {
            remaining.push(g);
        }
    }
    let mut prepared = Vec::new();
    let mut case_id = 0usize;
    let mut path_keys = HashSet::new();
    let mut case_keys = HashSet::new();
    let census_started = Instant::now();
    for (path_index, row) in rows.iter().enumerate() {
        if row.get("feasible").and_then(Value::as_bool) != Some(true)
            || row.get("complete").and_then(Value::as_bool) != Some(true)
        {
            return Err(format!(
                "row {path_index} is not a complete feasible census"
            ));
        }
        let path = outcomes(row.get("path").ok_or("row without path")?)?;
        if path.len() != target_games.len() {
            return Err(format!("row {path_index}: target path length mismatch"));
        }
        if !path_keys.insert(path.clone()) {
            return Err(format!("duplicate target path at row {path_index}"));
        }
        let mut added_points = 0;
        let mut added_wins = 0;
        let mut recomputed_target_points = m.base[cell.team].points;
        let mut recomputed_target_wins = m.base[cell.team].wins;
        let mut recomputed_prior = 1.0;
        for (g, &o) in target_games.iter().zip(&path) {
            if g.prob[o as usize] <= 0.0 {
                return Err(format!(
                    "row {path_index}: path uses zero-prior target outcome"
                ));
            }
            recomputed_prior *= g.prob[o as usize];
            if g.home == cell.team {
                added_points += [0, 1, 3][o as usize];
                added_wins += i32::from(o == 2);
            } else {
                added_points += [3, 1, 0][o as usize];
                added_wins += i32::from(o == 0);
            }
        }
        recomputed_target_points += added_points;
        recomputed_target_wins += added_wins;
        if val_i64(row, "target_points")? != i64::from(recomputed_target_points)
            || val_i64(row, "target_wins")? != i64::from(recomputed_target_wins)
            || !close(json_f64(row, "prior")?, recomputed_prior)
        {
            return Err(format!(
                "row {path_index}: target points/wins/prior mismatch"
            ));
        }
        let cat_name = category_for(added_points, added_wins)?;
        let cases = row
            .get("cases")
            .and_then(Value::as_array)
            .ok_or("row missing cases")?;
        let mut decoded_cases = Vec::with_capacity(cases.len());
        for (case_index, case) in cases.iter().enumerate() {
            let case_ids = ids_value(case)?;
            let mut mask = case_ids.clone();
            mask.sort_unstable();
            if !case_keys.insert((path.clone(), mask)) {
                return Err(format!(
                    "duplicate path/case mask at row {path_index} case {case_index}"
                ));
            }
            decoded_cases.push(case_ids);
        }
        if !category.matches(added_points, added_wins) {
            continue;
        }
        for (case_index, case_ids) in decoded_cases.iter().enumerate() {
            prepared.push(prepare_case(
                &m,
                &target_games,
                &remaining,
                cell,
                stride,
                row,
                path_index,
                case_index,
                case_ids,
                case_id,
                cat_name,
                reduction_fixed,
                integrate_unary,
            )?);
            case_id += 1;
        }
    }
    let census_elapsed_ms = census_started.elapsed().as_secs_f64() * 1000.;
    let setup_elapsed_ms = setup_started.elapsed().as_secs_f64() * 1000. - census_elapsed_ms;
    let mut order: Vec<usize> = (0..prepared.len()).collect();
    order.sort_by_key(|&i| prepared[i].estimated_cost);
    let census: Vec<_> = order.iter().map(|&i| prepared[i].census.clone()).collect();
    let selected_count = order.len().min(max_cases);
    let mut unselected_upper: BTreeMap<String, f64> = BTreeMap::new();
    for &i in &order[selected_count..] {
        let c = &prepared[i];
        *unselected_upper.entry(c.category.clone()).or_default() +=
            c.census["case_upper_mass"].as_f64().unwrap_or(0.);
    }
    order.truncate(selected_count);
    let per_case_quota = if selected_count == 0 {
        0
    } else {
        nodes_total / selected_count
    };
    let quotas: Vec<_> = order
        .iter()
        .map(|_| Limits {
            nodes: nodes_per_case.min(per_case_quota),
            memo_entries,
        })
        .collect();
    let run_started = Instant::now();
    let results = parallel(order.len(), workers, |slot| {
        count_case(&prepared[order[slot]], m.n, quotas[slot], cutoff_bound)
    });
    let measured_ms = run_started.elapsed().as_secs_f64() * 1000.;
    let mut sums: BTreeMap<String, [f64; 4]> = BTreeMap::new();
    for result in &results {
        let cat = result["category"].as_str().unwrap_or("all").to_string();
        let row = sums.entry(cat).or_default();
        row[0] += result["rank_settled_mass"].as_f64().unwrap_or(0.);
        row[1] += result["tie_possible_mass"].as_f64().unwrap_or(0.);
        row[2] += result["rank_impossible_mass"].as_f64().unwrap_or(0.);
        row[3] += result["unknown_upper_mass"].as_f64().unwrap_or(0.);
    }
    let aggregate: Vec<_> = sums.iter().map(|(k,v)| json!({"category":k,"rank_settled_lower_mass":v[0],"tie_possible_lower_mass":v[1],"rank_impossible_lower_mass":v[2],"unknown_upper_mass":v[3],"unselected_upper_mass":unselected_upper.get(k).copied().unwrap_or(0.)})).collect();
    let output = json!({
        "group":m.request.id,"team":team_id,"rank":rank1,"category_filter":category.label(),
        "reduction":if reduction_fixed {"fixed"} else {"none"},
        "unary_mode":if integrate_unary {"integrate"} else {"enumerate"},
        "cutoff_bound":if cutoff_bound == CutoffBound::Unary {"unary"} else {"domain"},
        "remaining_fixture_indices":remaining.iter().map(|g|g.index).collect::<Vec<_>>(),
        "cases_censused":census.len(),"cases_selected":selected_count,"workers":workers,
        "source_partition_assumption":"input rows are the complete certified path/case partition; all rows were validated before category filtering",
        "quota_policy":"deterministic equal node share across selected cases, capped per case; cases ordered by census variable-fixtures/frontier/constrained-teams/id",
        "nodes_per_case_limit":nodes_per_case,"nodes_total_limit":nodes_total,"memo_entries_per_component_limit":memo_entries,
        "setup_elapsed_ms":setup_elapsed_ms,"preparation_elapsed_ms":census_elapsed_ms,"counter_elapsed_ms":measured_ms,
        "unselected_upper_by_category":unselected_upper,
        "observed_timing_ms":measured_ms,"category_aggregates":aggregate,
        "census":census,"case_results":results
    });
    fs::write(
        &args[3],
        serde_json::to_vec_pretty(&output).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn ids_value(v: &Value) -> Result<Vec<i32>, String> {
    v.as_array()
        .ok_or_else(|| "case mask must be a team id array".to_string())?
        .iter()
        .map(|x| {
            x.as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .ok_or_else(|| "bad case team id".to_string())
        })
        .collect()
}

fn main() {
    if let Err(e) = run() {
        eprintln!("exact_rank_cases: {e}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(index: usize, home: usize, away: usize) -> Edge {
        Edge {
            index,
            home,
            away,
            prob: [0.25, 0.5, 0.25],
            hg: [0, 1, 3],
            ag: [3, 1, 0],
            mask: 7,
            track_home: true,
            track_away: true,
        }
    }
    fn exhaustive(edges: &[Edge], base: &[i32], target: i32) -> Profile {
        let mut out = Profile::new();
        for mut code in 0..3usize.pow(edges.len() as u32) {
            let mut totals = base.to_vec();
            let mut mass = 1.;
            for e in edges {
                let o = code % 3;
                code /= 3;
                totals[e.home] += e.hg[o];
                totals[e.away] += e.ag[o];
                mass *= e.prob[o];
            }
            let (mut above, mut equal) = (0, 0);
            for (t, &value) in totals.iter().enumerate() {
                if t == 0 {
                    continue;
                }
                if value > target {
                    above += 1;
                } else if value == target {
                    equal += 1;
                }
            }
            *out.entry((above, equal)).or_default() += mass;
        }
        out
    }
    fn exhaustive_bounded(
        edges: &[Edge],
        base: &[i32],
        lower: &[i32],
        upper: &[i32],
        target: i32,
    ) -> Profile {
        let mut out = Profile::new();
        for mut code in 0..3usize.pow(edges.len() as u32) {
            let mut totals = base.to_vec();
            let mut mass = 1.;
            let mut allowed = true;
            for e in edges {
                let o = code % 3;
                code /= 3;
                if e.mask & (1 << o) == 0 || e.prob[o] == 0. {
                    allowed = false;
                    break;
                }
                totals[e.home] += e.hg[o];
                totals[e.away] += e.ag[o];
                mass *= e.prob[o];
            }
            if !allowed || !(1..base.len()).all(|t| totals[t] >= lower[t] && totals[t] <= upper[t])
            {
                continue;
            }
            let above = (1..base.len()).filter(|&t| totals[t] > target).count();
            let equal = (1..base.len()).filter(|&t| totals[t] == target).count();
            *out.entry((above, equal)).or_default() += mass;
        }
        out
    }
    fn assert_profiles_close(a: &Profile, b: &Profile) {
        assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
        for (key, mass) in a {
            assert!((mass - b[key]).abs() <= 1e-12 * mass.abs().max(1.));
        }
    }
    fn graph_profile(
        edges: Vec<Edge>,
        n: usize,
        base: &[i32],
        target: i32,
        node_limit: usize,
        preclassified: &[bool],
        fixed_profile: (usize, usize),
        integrate_unary: bool,
    ) -> (Profile, f64) {
        let lower = vec![i32::MIN / 4; n];
        let upper = vec![i32::MAX / 4; n];
        graph_profile_bounded(
            edges,
            n,
            base,
            &lower,
            &upper,
            target,
            node_limit,
            preclassified,
            fixed_profile,
            integrate_unary,
        )
    }
    fn graph_profile_bounded(
        edges: Vec<Edge>,
        n: usize,
        base: &[i32],
        lower: &[i32],
        upper: &[i32],
        target: i32,
        node_limit: usize,
        preclassified: &[bool],
        fixed_profile: (usize, usize),
        integrate_unary: bool,
    ) -> (Profile, f64) {
        let parts = components(
            edges,
            n,
            0,
            base,
            target,
            preclassified,
            fixed_profile,
            integrate_unary,
        );
        let mut combined = BTreeMap::from([((0, 0), 1.)]);
        let mut unknown = 0.;
        let mut stats = CountStats::default();
        for c in parts {
            let (profile, partial, _) = count_component(
                &c,
                n,
                base,
                lower,
                upper,
                target,
                Limits {
                    nodes: node_limit,
                    memo_entries: 100_000,
                },
                CutoffBound::Domain,
                &mut stats,
            );
            let solved: f64 = combined.values().sum();
            let next_solved: f64 = profile.values().sum();
            unknown = unknown * (next_solved + partial) + solved * partial;
            combined = convolve(&combined, &profile);
        }
        (combined, unknown)
    }
    #[test]
    fn component_profiles_match_exhaustive_shared_fixture_enumeration() {
        let edges = vec![edge(0, 1, 2), edge(1, 2, 3)];
        let c = Component {
            edges,
            unary: vec![],
            teams: vec![1, 2, 3],
            order_width: 2,
            fixed_profile: (0, 0),
            domain_upper_mass: 1.,
        };
        let base = [2, 2, 0, 4];
        let lo = [i32::MIN / 4; 4];
        let hi = [i32::MAX / 4; 4];
        let mut stats = CountStats::default();
        let (actual, unknown, _) = count_component(
            &c,
            4,
            &base,
            &lo,
            &hi,
            2,
            Limits {
                nodes: 1000,
                memo_entries: 10000,
            },
            CutoffBound::Domain,
            &mut stats,
        );
        assert_eq!(unknown, 0.);
        assert_eq!(actual, exhaustive(&c.edges, &base, 2));
    }
    #[test]
    fn cutoff_unknown_upper_contains_all_bruteforce_mass_without_double_counting() {
        let edges = vec![edge(0, 1, 2), edge(1, 2, 3), edge(2, 3, 4)];
        let c = Component {
            edges: edges.clone(),
            unary: vec![],
            teams: vec![1, 2, 3, 4],
            order_width: 2,
            fixed_profile: (0, 0),
            domain_upper_mass: 1.,
        };
        let base = [2, 0, 1, 1, 3];
        let lo = [i32::MIN / 4; 5];
        let hi = [i32::MAX / 4; 5];
        let mut stats = CountStats::default();
        let (exact, unknown, upper) = count_component(
            &c,
            5,
            &base,
            &lo,
            &hi,
            2,
            Limits {
                nodes: 1,
                memo_entries: 1000,
            },
            CutoffBound::Domain,
            &mut stats,
        );
        let truth: f64 = exhaustive(&edges, &base, 2).values().sum();
        assert!(exact.values().sum::<f64>() <= truth + 1e-12);
        assert!(truth <= exact.values().sum::<f64>() + unknown + 1e-12);
        assert!(unknown <= upper + 1e-12);
    }
    #[test]
    fn component_convolution_adds_settled_comparison_counts() {
        let a = BTreeMap::from([((1, 0), 0.2), ((0, 1), 0.3)]);
        let b = BTreeMap::from([((0, 2), 0.4), ((1, 0), 0.1)]);
        let c = convolve(&a, &b);
        assert!((c[&(1, 2)] - 0.08).abs() < 1e-15);
        assert!((c[&(2, 0)] - 0.02).abs() < 1e-15);
    }
    #[test]
    fn zero_based_rank_slot_classification_is_not_decremented_twice() {
        assert_eq!(classify_profile((0, 0), 0), "rank_settled");
        assert_eq!(classify_profile((1, 0), 0), "rank_impossible");
        assert_eq!(classify_profile((0, 1), 0), "tie_possible");
    }
    #[test]
    fn no_variable_fixture_rivals_are_counted_once_and_target_is_excluded() {
        let fixed_components =
            components(vec![], 3, 0, &[100, 6, 5], 5, &[false; 3], (0, 0), false);
        assert_eq!(fixed_components.len(), 1);
        let prepared = PreparedCase {
            id: 0,
            path_index: 0,
            case_index: 0,
            category: "0".to_string(),
            target_prior: 1.,
            forced_mass: 1.,
            integrated_mass: 1.,
            base_weight: 1.,
            target_total: 5,
            rank: 0,
            base: vec![100, 6, 5],
            lower: vec![i32::MIN / 4; 3],
            upper: vec![i32::MAX / 4; 3],
            components: fixed_components,
            census: json!({}),
            estimated_cost: (0, 0, 0, 0),
        };
        let result = count_case(
            &prepared,
            3,
            Limits {
                nodes: 10,
                memo_entries: 10,
            },
            CutoffBound::Domain,
        );
        assert_eq!(result["complete"], true);
        assert_eq!(result["profiles"][0]["strict_above"], 1);
        assert_eq!(result["profiles"][0]["equal"], 1);
        assert_eq!(result["profiles"][0]["allowed_mass"], 1.0);
    }
    #[test]
    fn restrictive_bounds_are_checked_when_last_fixture_closes_team() {
        let edges = vec![edge(0, 1, 2), edge(1, 2, 3)];
        let c = Component {
            edges: edges.clone(),
            unary: vec![],
            teams: vec![1, 2, 3],
            order_width: 2,
            fixed_profile: (0, 0),
            domain_upper_mass: 1.,
        };
        let base = [2, 2, 0, 4];
        let mut lo = [i32::MIN / 4; 4];
        let mut hi = [i32::MAX / 4; 4];
        lo[1] = 3;
        hi[1] = 3;
        lo[2] = 1;
        hi[2] = 3;
        lo[3] = 1;
        hi[3] = 5;
        let mut expected = Profile::new();
        for mut code in 0..3usize.pow(edges.len() as u32) {
            let mut totals = base.to_vec();
            let mut mass = 1.;
            for e in &edges {
                let o = code % 3;
                code /= 3;
                totals[e.home] += e.hg[o];
                totals[e.away] += e.ag[o];
                mass *= e.prob[o];
            }
            if (1..4).all(|t| totals[t] >= lo[t] && totals[t] <= hi[t]) {
                let above = (1..4).filter(|&t| totals[t] > 2).count();
                let equal = (1..4).filter(|&t| totals[t] == 2).count();
                *expected.entry((above, equal)).or_default() += mass;
            }
        }
        let mut stats = CountStats::default();
        let (actual, unknown, _) = count_component(
            &c,
            4,
            &base,
            &lo,
            &hi,
            2,
            Limits {
                nodes: 1000,
                memo_entries: 10000,
            },
            CutoffBound::Domain,
            &mut stats,
        );
        assert_eq!(unknown, 0.);
        assert_eq!(actual, expected);
    }
    #[test]
    fn cutoff_composition_bound_contains_bruteforce_truth() {
        let solved_a = 0.3;
        let unknown_a = 0.2;
        let solved_b = 0.4;
        let unknown_b = 0.1;
        let composed_unknown = unknown_a * (solved_b + unknown_b) + solved_a * unknown_b;
        let lower = solved_a * solved_b;
        let true_joint = (solved_a + unknown_a) * (solved_b + unknown_b);
        assert!(true_joint + 1e-15 >= lower);
        assert!(true_joint <= lower + composed_unknown + 1e-15);
    }
    #[test]
    fn census_upper_multiplies_target_and_forced_probability_once() {
        assert!((case_upper_mass(0.2, 0.3, 0.5) - 0.03).abs() < 1e-15);
    }
    #[test]
    fn fixed_status_reduction_preserves_weighted_profiles_unary_edges_and_integrated_mass() {
        let mut raw = vec![
            edge(10, 1, 2),
            edge(11, 2, 1),
            edge(12, 1, 3),
            edge(13, 4, 2),
            edge(14, 3, 4),
        ];
        raw[0].prob = [0.2, 0.3, 0.5];
        raw[1].prob = [0.6, 0.1, 0.3];
        raw[2].prob = [0.1, 0.7, 0.2];
        raw[2].mask = 0b110;
        raw[3].prob = [0.4, 0.4, 0.2];
        raw[3].mask = 0b101;
        raw[4].prob = [0.2, 0.5, 0.3];
        raw[4].mask = 0b011;
        let base = [2, 0, 0, 10, 10];
        let lower = [i32::MIN / 4, 0, 0, 0, 0];
        let upper = [i32::MAX / 4, 20, 20, 20, 20];
        let (v1, _) = graph_profile(raw.clone(), 5, &base, 2, 10_000, &[false; 5], (0, 0), false);
        let reduced = reduce_fixed_status(&raw, 5, 0, &base, &lower, &upper, 2, true);
        assert_eq!(reduced.reduced_rivals, 2);
        assert!((reduced.integrated_mass - 0.7).abs() < 1e-15);
        assert_eq!(reduced.edges.len(), 4);
        assert_eq!(
            reduced
                .edges
                .iter()
                .filter(|e| e.track_home && e.track_away)
                .count(),
            2
        );
        assert_eq!(
            reduced
                .edges
                .iter()
                .filter(|e| e.track_home ^ e.track_away)
                .count(),
            2
        );
        let (mut v2, unknown) = graph_profile(
            reduced.edges.clone(),
            5,
            &base,
            2,
            10_000,
            &reduced.preclassified,
            reduced.fixed_profile,
            false,
        );
        assert_eq!(unknown, 0.);
        for mass in v2.values_mut() {
            *mass *= reduced.integrated_mass;
        }
        assert_profiles_close(&v1, &v2);
        let (mut v3, unknown) = graph_profile(
            reduced.edges.clone(),
            5,
            &base,
            2,
            10_000,
            &reduced.preclassified,
            reduced.fixed_profile,
            true,
        );
        assert_eq!(unknown, 0.);
        for mass in v3.values_mut() {
            *mass *= reduced.integrated_mass;
        }
        assert_profiles_close(&v1, &v3);
        let bounded_lower = [i32::MIN / 4, 0, 1, 0, 0];
        let bounded_upper = [i32::MAX / 4, 9, 6, 20, 20];
        let bounded_reduced =
            reduce_fixed_status(&raw, 5, 0, &base, &bounded_lower, &bounded_upper, 2, true);
        let (v1_bounded, _) = graph_profile_bounded(
            raw.clone(),
            5,
            &base,
            &bounded_lower,
            &bounded_upper,
            2,
            10_000,
            &[false; 5],
            (0, 0),
            false,
        );
        assert_profiles_close(
            &v1_bounded,
            &exhaustive_bounded(&raw, &base, &bounded_lower, &bounded_upper, 2),
        );
        let (mut v3_bounded, unknown_bounded) = graph_profile_bounded(
            bounded_reduced.edges.clone(),
            5,
            &base,
            &bounded_lower,
            &bounded_upper,
            2,
            10_000,
            &bounded_reduced.preclassified,
            bounded_reduced.fixed_profile,
            true,
        );
        assert_eq!(unknown_bounded, 0.);
        for mass in v3_bounded.values_mut() {
            *mass *= bounded_reduced.integrated_mass;
        }
        assert_profiles_close(&v1_bounded, &v3_bounded);
        let cutoff_reduced = reduce_fixed_status(&raw, 5, 0, &base, &lower, &upper, 2, true);
        let (partial, unknown) = graph_profile(
            cutoff_reduced.edges,
            5,
            &base,
            2,
            1,
            &cutoff_reduced.preclassified,
            cutoff_reduced.fixed_profile,
            true,
        );
        let truth: f64 = v2.values().sum();
        assert!(partial.values().sum::<f64>() * cutoff_reduced.integrated_mass <= truth + 1e-12);
        assert!(
            truth
                <= partial.values().sum::<f64>() * cutoff_reduced.integrated_mass
                    + unknown * cutoff_reduced.integrated_mass
                    + 1e-12
        );
    }
    #[test]
    fn pure_unary_component_matches_direct_weighted_gain_profile() {
        let unary = UnaryTeam {
            team: 1,
            gains: BTreeMap::from([(0, 0.25), (1, 0.5), (3, 0.25)]),
            min_gain: 0,
            max_gain: 3,
            allowed_mass: 1.,
        };
        let component = Component {
            edges: vec![],
            unary: vec![unary],
            teams: vec![1],
            order_width: 0,
            fixed_profile: (0, 0),
            domain_upper_mass: 1.,
        };
        let base = [9, 1];
        let lower = [i32::MIN / 4, 2];
        let upper = [i32::MAX / 4, 4];
        let mut stats = CountStats::default();
        let (profile, unknown, _) = count_component(
            &component,
            2,
            &base,
            &lower,
            &upper,
            2,
            Limits {
                nodes: 1,
                memo_entries: 1,
            },
            CutoffBound::Domain,
            &mut stats,
        );
        assert_eq!(unknown, 0.);
        assert_eq!(profile, BTreeMap::from([((0, 1), 0.5), ((1, 0), 0.25)]));
    }
    #[test]
    fn unary_cutoff_bound_is_tighter_and_contains_restrictive_exact_mass() {
        let mut shared = edge(0, 1, 2);
        shared.prob = [0.2, 0.3, 0.5];
        let unary = UnaryTeam {
            team: 1,
            gains: BTreeMap::from([(1, 0.99), (3, 0.01)]),
            min_gain: 1,
            max_gain: 3,
            allowed_mass: 1.,
        };
        let component = Component {
            edges: vec![shared],
            unary: vec![unary],
            teams: vec![1, 2],
            order_width: 1,
            fixed_profile: (0, 0),
            domain_upper_mass: 1.,
        };
        let base = [2, 0, 0];
        let lower = [i32::MIN / 4, 2, i32::MIN / 4];
        let upper = [i32::MAX / 4, 2, i32::MAX / 4];
        let mut domain_stats = CountStats::default();
        let (_, domain_unknown, _) = count_component(
            &component,
            3,
            &base,
            &lower,
            &upper,
            2,
            Limits {
                nodes: 0,
                memo_entries: 10,
            },
            CutoffBound::Domain,
            &mut domain_stats,
        );
        let mut unary_stats = CountStats::default();
        let (_, unary_unknown, _) = count_component(
            &component,
            3,
            &base,
            &lower,
            &upper,
            2,
            Limits {
                nodes: 0,
                memo_entries: 10,
            },
            CutoffBound::Unary,
            &mut unary_stats,
        );
        let truth = 0.3 * 0.99;
        assert!(unary_unknown <= domain_unknown);
        assert!(truth <= unary_unknown + 1e-15);
        assert!(unary_unknown < domain_unknown);
    }
    #[test]
    fn unary_cutoff_positive_sum_handles_packed_gain_gap() {
        let shared = Edge {
            index: 0,
            home: 1,
            away: 2,
            prob: [0.3, 0.3, 0.4],
            hg: [0, 2, 2],
            ag: [2, 0, 0],
            mask: 7,
            track_home: true,
            track_away: true,
        };
        let unary = UnaryTeam {
            team: 1,
            gains: BTreeMap::from([(0, 0.5), (4, 0.5)]),
            min_gain: 0,
            max_gain: 4,
            allowed_mass: 1.,
        };
        let component = Component {
            edges: vec![shared],
            unary: vec![unary],
            teams: vec![1, 2],
            order_width: 1,
            fixed_profile: (0, 0),
            domain_upper_mass: 1.,
        };
        let base = [2, 0, 0];
        let lower = [i32::MIN / 4, 3, i32::MIN / 4];
        let upper = [i32::MAX / 4, 3, i32::MAX / 4];
        let mut stats = CountStats::default();
        let (_, unknown, _) = count_component(
            &component,
            3,
            &base,
            &lower,
            &upper,
            2,
            Limits {
                nodes: 0,
                memo_entries: 10,
            },
            CutoffBound::Unary,
            &mut stats,
        );
        assert_eq!(unknown, 0.);
    }
    #[test]
    fn tiny_unknown_mass_survives_when_known_mass_rounds_to_one() {
        let mut first = edge(0, 1, 2);
        first.prob = [1.0, 1e-20, 0.0];
        first.mask = 0b011;
        let mut second = edge(1, 2, 3);
        second.prob = [1.0, 0.0, 0.0];
        second.mask = 0b001;
        let c = Component {
            edges: vec![first, second],
            unary: vec![],
            teams: vec![1, 2, 3],
            order_width: 2,
            fixed_profile: (0, 0),
            domain_upper_mass: 1.,
        };
        let base = [2, 0, 0, 0];
        let lower = [i32::MIN / 4; 4];
        let upper = [i32::MAX / 4; 4];
        let mut stats = CountStats::default();
        let (known, unknown, _) = count_component(
            &c,
            4,
            &base,
            &lower,
            &upper,
            2,
            Limits {
                nodes: 2,
                memo_entries: 100,
            },
            CutoffBound::Domain,
            &mut stats,
        );
        assert!(known.values().sum::<f64>() >= 0.99);
        assert!(unknown >= 1e-20 * 0.99);
    }
    #[test]
    fn terminal_profile_cache_preserves_exact_profile() {
        let unary = UnaryTeam {
            team: 1,
            gains: BTreeMap::from([(0, 0.2), (1, 0.3), (3, 0.5)]),
            min_gain: 0,
            max_gain: 3,
            allowed_mass: 1.,
        };
        let map = HashMap::from([(1usize, &unary)]);
        let lower = [i32::MIN / 4, 0];
        let upper = [i32::MAX / 4, 10];
        let mut cache = HashMap::new();
        let mut stats = CountStats::default();
        let first = unary_terminal_profile(1, 1, &map, &lower, &upper, 2, &mut cache, &mut stats);
        let second = unary_terminal_profile(1, 1, &map, &lower, &upper, 2, &mut cache, &mut stats);
        assert_eq!(first, second);
        assert_eq!(stats.terminal_cache_misses, 1);
        assert_eq!(stats.terminal_cache_hits, 1);
    }
}
