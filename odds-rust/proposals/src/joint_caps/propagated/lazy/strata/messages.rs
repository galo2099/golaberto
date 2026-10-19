//! Factor-message proposal. Messages guide Q only. They neither prove
//! infeasibility nor replace the exact joint normalizer and final P/Q correction.
use super::*;

fn consume_work(work: &mut usize, amount: usize, limit: usize) -> Option<()> {
    let next = work.checked_add(amount)?;
    if next > limit {
        return None;
    }
    *work = next;
    Some(())
}

fn sum_counted_slice(values: &[f64], work: &mut usize, work_limit: usize) -> Option<f64> {
    consume_work(work, values.len(), work_limit)?;
    Some(values.iter().sum())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GuideEstimate {
    forward_span: usize,
    reverse_span: usize,
    charge: usize,
}

// Mirror Guide::new and Suffix::new's logical row lengths without allocating
// either table. Invalid inputs fail closed before Guide::new can panic or wrap.
fn estimate_guide_charge(games: &[RankGame], teams: usize) -> Option<GuideEstimate> {
    let row_count = games.len().checked_add(1)?.checked_mul(teams)?;
    if row_count > isize::MAX as usize / std::mem::size_of::<usize>() {
        return None;
    }
    let mut forward = vec![0usize; teams];
    let mut reverse = vec![0usize; teams];
    for game in games {
        if game.home >= teams || game.away >= teams || game.home == game.away {
            return None;
        }
        if game.prob.iter().any(|p| !p.is_finite() || *p < 0.0)
            || !game.prob.iter().any(|p| *p > 0.0)
        {
            return None;
        }
        for (team, gains) in [(game.home, game.hg), (game.away, game.ag)] {
            if gains.iter().any(|&gain| gain < 0) || gains.iter().any(|&gain| gain > i32::MAX - 1) {
                return None;
            }
            let supported_max = (0..3)
                .filter(|&outcome| game.prob[outcome] > 0.0)
                .map(|outcome| gains[outcome])
                .max()?;
            forward[team] = forward[team].checked_add(supported_max as usize)?;
            reverse[team] = reverse[team].checked_add(*gains.iter().max()? as usize)?;
            // Guide::new accumulates these values in i32 vectors.
            if forward[team] > (i32::MAX - 1) as usize || reverse[team] > (i32::MAX - 1) as usize {
                return None;
            }
        }
    }
    let forward_span = *forward.iter().max().unwrap_or(&0);
    let reverse_span = *reverse.iter().max().unwrap_or(&0);
    let charge = row_count
        .checked_mul(forward_span.checked_add(1)?)?
        .checked_add(row_count.checked_mul(reverse_span.checked_add(1)?)?)?
        .checked_mul(2)?;
    Some(GuideEstimate {
        forward_span,
        reverse_span,
        charge,
    })
}

// Conservative preflight for the coarse bounded experimental path: every row in both tables may
// span the sum of all per-game gains, and each table charge counts two slots.
fn coarse_guide_charge(games: &[RankGame], teams: usize) -> Option<usize> {
    let span_bound = games.iter().try_fold(0usize, |sum, g| {
        let game_max = g.hg.iter().chain(&g.ag).copied().max()?.max(0) as usize;
        sum.checked_add(game_max)
    })?;
    games
        .len()
        .checked_add(1)
        .and_then(|count| count.checked_mul(teams))
        .and_then(|rows| rows.checked_mul(span_bound.checked_add(1)?))
        .and_then(|charge| charge.checked_mul(4))
}

fn guide_charge_fits(work: usize, charge: usize, work_limit: usize) -> Option<()> {
    (work.checked_add(charge)? <= work_limit).then_some(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RetuneFailure {
    Omitted,
    InvalidMessage,
    WorkAllowance,
    NodeAllowance,
    NodeCapExhausted,
    InvalidMass,
    GuideBound,
}

impl RetuneFailure {
    fn label(self) -> &'static str {
        match self {
            Self::Omitted => "omitted",
            Self::InvalidMessage => "invalid_message",
            Self::WorkAllowance => "work_allowance",
            Self::NodeAllowance => "node_allowance",
            Self::NodeCapExhausted => "node_cap_exhausted",
            Self::InvalidMass => "invalid_mass",
            Self::GuideBound => "guide_bound",
        }
    }
}

fn bounded_retuner_enabled(
    tree_discovery: Option<&str>,
    family_structure: crate::rare_tail::family_config::FamilyStructure,
) -> bool {
    tree_discovery.is_some_and(|mode| matches!(mode, "interval" | "rank" | "strict"))
        || family_structure != crate::rare_tail::family_config::FamilyStructure::Full
}

fn retry_bounded_candidates<T>(
    reserved_unseen: &[usize],
    ranked_eligible: &[usize],
    count: usize,
    max_attempts: usize,
    work: &mut usize,
    work_limit: usize,
    mut retune_candidate: impl FnMut(usize, &mut usize, usize) -> std::result::Result<T, RetuneFailure>,
) -> Vec<(usize, usize, std::result::Result<T, RetuneFailure>)> {
    let mut attempts = Vec::new();
    let mut successes = 0;
    let mut attempted = std::collections::HashSet::new();
    for &index in reserved_unseen.iter().take(max_attempts) {
        if successes >= count {
            break;
        }
        attempted.insert(index);
        let before = *work;
        let result = retune_candidate(index, work, work_limit);
        let delta = (*work).saturating_sub(before);
        if result.is_ok() {
            successes += 1;
        }
        let stop = matches!(
            &result,
            Err(RetuneFailure::WorkAllowance | RetuneFailure::NodeAllowance)
        );
        attempts.push((index, delta, result));
        if stop {
            break;
        }
        if successes > 0 {
            break;
        }
    }
    if !attempts.last().is_some_and(|(_, _, result)| {
        matches!(
            result,
            Err(RetuneFailure::WorkAllowance | RetuneFailure::NodeAllowance)
        )
    }) {
        for &index in ranked_eligible {
            if successes >= count || attempts.len() >= max_attempts {
                break;
            }
            if !attempted.insert(index) {
                continue;
            }
            let before = *work;
            let result = retune_candidate(index, work, work_limit);
            let delta = (*work).saturating_sub(before);
            if result.is_ok() {
                successes += 1;
            }
            let stop = matches!(
                &result,
                Err(RetuneFailure::WorkAllowance | RetuneFailure::NodeAllowance)
            );
            attempts.push((index, delta, result));
            if stop {
                break;
            }
        }
    }
    attempts
}

#[cfg(test)]
fn settle_node_usage<T>(
    result: Option<T>,
    node_limit: usize,
    remaining_nodes: usize,
    work: &mut usize,
    work_limit: usize,
) -> Option<T> {
    let used_nodes = node_limit.checked_sub(remaining_nodes)?;
    consume_work(work, used_nodes.saturating_mul(16), work_limit)?;
    result
}

pub(super) struct TiltedPattern {
    pub pattern: Pattern,
    pub nominal: Arc<Vec<[f64; 3]>>,
}
impl TiltedPattern {
    pub fn correction(&self, out: &[u8], ops: &mut crate::rare_tail::budget::Operations) -> f64 {
        self.pattern
            .joint
            .games
            .iter()
            .chain(&self.pattern.guide.games)
            .fold(1., |v, g| {
                ops.guidance += 3;
                let o = out[g.index] as usize;
                v * self.nominal[g.index][o] / g.prob[o]
            })
    }
}

fn normalized(mut row: [f64; 3]) -> Option<[f64; 3]> {
    let z = row.iter().sum::<f64>();
    if z <= 0. || !z.is_finite() {
        return None;
    }
    for p in &mut row {
        *p /= z;
    }
    Some(row)
}
fn proposal_rows(
    p: &Pattern,
    iterations: usize,
    work: &mut usize,
    work_limit: usize,
) -> std::result::Result<Vec<RankGame>, RetuneFailure> {
    let mut games: Vec<_> = p
        .joint
        .games
        .iter()
        .chain(&p.guide.games)
        .copied()
        .collect();
    games.sort_by_key(|g| g.index);
    let mut incident = vec![Vec::new(); p.base.len()];
    for (i, g) in games.iter().enumerate() {
        incident[g.home].push((i, 0, g.hg));
        incident[g.away].push((i, 1, g.ag));
    }
    let mut messages = vec![[[1.; 3]; 2]; games.len()];
    for _ in 0..iterations {
        let mut next = messages.clone();
        for (t, edges) in incident.iter().enumerate() {
            for &(excluded, side, gains) in edges {
                let mut distribution = vec![1.];
                for &(i, s, gain) in edges {
                    if i == excluded {
                        continue;
                    }
                    let row = normalized(std::array::from_fn(|o| {
                        games[i].prob[o] * messages[i][1 - s][o]
                    }))
                    .ok_or(RetuneFailure::InvalidMessage)?;
                    let mut added =
                        vec![0.; distribution.len() + *gain.iter().max().unwrap() as usize];
                    for (v, &mass) in distribution.iter().enumerate() {
                        if mass == 0. {
                            continue;
                        }
                        for o in 0..3 {
                            if row[o] > 0. {
                                consume_work(work, 1, work_limit)
                                    .ok_or(RetuneFailure::WorkAllowance)?;
                                added[v + gain[o] as usize] += mass * row[o];
                            }
                        }
                    }
                    distribution = added;
                }
                let mut row = [0.; 3];
                for o in 0..3 {
                    let lo = (p.lower[t] - p.base[t] - gains[o]).max(0) as usize;
                    let hi = p.upper[t] - p.base[t] - gains[o];
                    if hi >= 0 && lo < distribution.len() {
                        let hi = (hi as usize).min(distribution.len() - 1);
                        if lo <= hi {
                            row[o] = sum_counted_slice(&distribution[lo..=hi], work, work_limit)
                                .ok_or(RetuneFailure::WorkAllowance)?;
                        }
                    }
                }
                let max = row.iter().copied().fold(0_f64, f64::max);
                if max > 0. && max.is_finite() {
                    for o in 0..3 {
                        next[excluded][side][o] =
                            0.5 * messages[excluded][side][o] + 0.5 * row[o] / max;
                    }
                }
                // A vanished floating point message is not an impossibility proof.
            }
        }
        messages = next;
    }
    let share = 0.9; // Defensive original probability preserves event support.
    for (i, g) in games.iter_mut().enumerate() {
        let tilted = normalized(std::array::from_fn(|o| {
            g.prob[o] * messages[i][0][o] * messages[i][1][o]
        }));
        if let Some(q) = tilted {
            g.prob = std::array::from_fn(|o| (1. - share) * g.prob[o] + share * q[o]);
        }
    }
    Ok(games)
}
#[cfg(test)]
fn retune(
    p: &Pattern,
    fixture_count: usize,
    iterations: usize,
    work: &mut usize,
    work_limit: usize,
) -> Option<TiltedPattern> {
    retune_bounded(p, fixture_count, iterations, work, work_limit).ok()
}

#[cfg(test)]
fn retune_bounded(
    p: &Pattern,
    fixture_count: usize,
    iterations: usize,
    work: &mut usize,
    work_limit: usize,
) -> std::result::Result<TiltedPattern, RetuneFailure> {
    retune_bounded_mode(p, fixture_count, iterations, work, work_limit, false, None)
}

fn retune_bounded_mode(
    p: &Pattern,
    fixture_count: usize,
    iterations: usize,
    work: &mut usize,
    work_limit: usize,
    tight_guide_bound: bool,
    guide_charge: Option<&mut Option<usize>>,
) -> std::result::Result<TiltedPattern, RetuneFailure> {
    if !p.omitted.is_empty() {
        return Err(RetuneFailure::Omitted);
    }
    let games = proposal_rows(p, iterations, work, work_limit)?;
    let mut nominal = vec![[0.; 3]; fixture_count];
    for g in p.joint.games.iter().chain(&p.guide.games) {
        nominal[g.index] = g.prob;
    }
    let node_limit = 10_000.min(work_limit.saturating_sub(*work) / 16);
    let mut nodes = node_limit;
    if nodes == 0 {
        return Err(RetuneFailure::NodeAllowance);
    }
    let joint_result = NecessaryJoint::metered(&games, &p.base, &p.lower, &p.upper, 4, &mut nodes);
    let used_nodes = node_limit
        .checked_sub(nodes)
        .ok_or(RetuneFailure::NodeCapExhausted)?;
    consume_work(work, used_nodes.saturating_mul(16), work_limit)
        .ok_or(RetuneFailure::WorkAllowance)?;
    let (joint, remaining) = joint_result.ok_or(RetuneFailure::NodeCapExhausted)?;
    if joint.mass <= 0. || !joint.mass.is_finite() {
        return Err(RetuneFailure::InvalidMass);
    }
    let mut predicted_guide_charge = None;
    let guide_bound = if tight_guide_bound {
        // Charge the estimator traversal before doing it, so the admission
        // check itself remains inside the same deterministic grant.
        // Proxy charges one visit per fixture and four slots per team,
        // covering initialization of both span vectors and their scans.
        let estimate_work = remaining
            .len()
            .checked_add(
                p.base
                    .len()
                    .checked_mul(4)
                    .ok_or(RetuneFailure::GuideBound)?,
            )
            .ok_or(RetuneFailure::GuideBound)?;
        consume_work(work, estimate_work, work_limit).ok_or(RetuneFailure::GuideBound)?;
        let estimate =
            estimate_guide_charge(&remaining, p.base.len()).ok_or(RetuneFailure::GuideBound)?;
        predicted_guide_charge = Some(estimate.charge);
        if let Some(slot) = guide_charge {
            *slot = Some(estimate.charge);
        }
        estimate.charge
    } else {
        coarse_guide_charge(&remaining, p.base.len()).ok_or(RetuneFailure::GuideBound)?
    };
    guide_charge_fits(*work, guide_bound, work_limit).ok_or(RetuneFailure::GuideBound)?;
    let guide = Guide::new(remaining, p.base.len());
    let guide_work = 2 * guide
        .cdf
        .values
        .iter()
        .chain(&guide.reverse.values)
        .map(|v| v.len())
        .sum::<usize>();
    if tight_guide_bound {
        debug_assert_eq!(predicted_guide_charge, Some(guide_work));
    }
    if work
        .checked_add(guide_work)
        .ok_or(RetuneFailure::GuideBound)?
        > work_limit
    {
        return Err(RetuneFailure::GuideBound);
    }
    consume_work(work, guide_work, work_limit).ok_or(RetuneFailure::WorkAllowance)?;
    let mut q = p.clone();
    q.mass = p.mass / p.joint.mass * joint.mass;
    if q.mass <= 0. || !q.mass.is_finite() {
        return Err(RetuneFailure::InvalidMass);
    }
    q.joint = Arc::new(joint);
    q.guide = Arc::new(guide);
    Ok(TiltedPattern {
        pattern: q,
        nominal: Arc::new(nominal),
    })
}

// Keep the default message path byte-for-byte aligned with the pre-discovery
// retuner. In particular, its fixed joint-node allowance and its Guide build
// happen independently of the experimental discovery grant.
fn retune_legacy(
    p: &Pattern,
    fixture_count: usize,
    iterations: usize,
    work: &mut usize,
) -> Option<TiltedPattern> {
    if !p.omitted.is_empty() {
        return None;
    }
    let games = proposal_rows_legacy(p, iterations, work)?;
    let mut nominal = vec![[0.; 3]; fixture_count];
    for g in p.joint.games.iter().chain(&p.guide.games) {
        nominal[g.index] = g.prob;
    }
    let mut nodes = 10_000;
    let (joint, remaining) =
        NecessaryJoint::metered(&games, &p.base, &p.lower, &p.upper, 4, &mut nodes)?;
    *work += 16 * (10_000 - nodes);
    if joint.mass <= 0. || !joint.mass.is_finite() {
        return None;
    }
    let guide = Guide::new(remaining, p.base.len());
    *work += 2 * guide
        .cdf
        .values
        .iter()
        .chain(&guide.reverse.values)
        .map(|v| v.len())
        .sum::<usize>();
    let mut q = p.clone();
    q.mass = p.mass / p.joint.mass * joint.mass;
    if q.mass <= 0. || !q.mass.is_finite() {
        return None;
    }
    q.joint = Arc::new(joint);
    q.guide = Arc::new(guide);
    Some(TiltedPattern {
        pattern: q,
        nominal: Arc::new(nominal),
    })
}

fn proposal_rows_legacy(p: &Pattern, iterations: usize, work: &mut usize) -> Option<Vec<RankGame>> {
    let mut games: Vec<_> = p
        .joint
        .games
        .iter()
        .chain(&p.guide.games)
        .copied()
        .collect();
    games.sort_by_key(|g| g.index);
    let mut incident = vec![Vec::new(); p.base.len()];
    for (i, g) in games.iter().enumerate() {
        incident[g.home].push((i, 0, g.hg));
        incident[g.away].push((i, 1, g.ag));
    }
    let mut messages = vec![[[1.; 3]; 2]; games.len()];
    for _ in 0..iterations {
        let mut next = messages.clone();
        for (t, edges) in incident.iter().enumerate() {
            for &(excluded, side, gains) in edges {
                let mut distribution = vec![1.];
                for &(i, s, gain) in edges {
                    if i == excluded {
                        continue;
                    }
                    let row = normalized(std::array::from_fn(|o| {
                        games[i].prob[o] * messages[i][1 - s][o]
                    }))?;
                    let mut added =
                        vec![0.; distribution.len() + *gain.iter().max().unwrap() as usize];
                    for (v, &mass) in distribution.iter().enumerate() {
                        if mass == 0. {
                            continue;
                        }
                        for o in 0..3 {
                            if row[o] > 0. {
                                *work += 1;
                                if *work > 20_000_000 {
                                    return None;
                                }
                                added[v + gain[o] as usize] += mass * row[o];
                            }
                        }
                    }
                    distribution = added;
                }
                let mut row = [0.; 3];
                for o in 0..3 {
                    let lo = (p.lower[t] - p.base[t] - gains[o]).max(0) as usize;
                    let hi = p.upper[t] - p.base[t] - gains[o];
                    if hi >= 0 && lo < distribution.len() {
                        let hi = (hi as usize).min(distribution.len() - 1);
                        if lo <= hi {
                            row[o] = distribution[lo..=hi].iter().sum();
                            *work += hi + 1 - lo;
                        }
                    }
                }
                let max = row.iter().copied().fold(0_f64, f64::max);
                if max > 0. && max.is_finite() {
                    for o in 0..3 {
                        next[excluded][side][o] =
                            0.5 * messages[excluded][side][o] + 0.5 * row[o] / max;
                    }
                }
            }
        }
        messages = next;
    }
    let share = 0.9;
    for (i, g) in games.iter_mut().enumerate() {
        let tilted = normalized(std::array::from_fn(|o| {
            g.prob[o] * messages[i][0][o] * messages[i][1][o]
        }));
        if let Some(q) = tilted {
            g.prob = std::array::from_fn(|o| (1. - share) * g.prob[o] + share * q[o]);
        }
    }
    Some(games)
}

const DISCOVERY_LIMIT: usize = 20_000_000;

fn tight_guide_bound_enabled() -> bool {
    std::env::var("RUST_ODDS_EXPERIMENT_TIGHT_GUIDE_BOUND").as_deref() == Ok("1")
}

fn discovery_mode() -> Option<&'static str> {
    match std::env::var("RUST_ODDS_EXPERIMENT_TREE_DISCOVERY")
        .ok()?
        .as_str()
    {
        "interval" => Some("interval"),
        "rank" => Some("rank"),
        "strict" => Some("strict"),
        _ => None,
    }
}

fn add_game_marginals(
    p: &Pattern,
    work: &mut usize,
    work_limit: usize,
) -> Option<Vec<Vec<RankGame>>> {
    let possible_games = p
        .joint
        .games
        .len()
        .checked_add(p.guide.games.len())?
        .checked_add(p.omitted.len())?;
    consume_work(work, possible_games.saturating_mul(2), work_limit)?;
    let mut by_team = vec![Vec::new(); p.base.len()];
    let mut games = p
        .joint
        .games
        .iter()
        .chain(&p.guide.games)
        .chain(&p.omitted)
        .copied()
        .collect::<Vec<_>>();
    games.sort_by_key(|g| g.index);
    games.dedup_by_key(|g| g.index);
    for g in games {
        if g.home >= by_team.len()
            || g.away >= by_team.len()
            || g.prob.iter().any(|v| !v.is_finite() || *v < 0.)
            || g.prob.iter().sum::<f64>() <= 0.
        {
            return None;
        }
        by_team[g.home].push(g);
        by_team[g.away].push(g);
    }
    Some(by_team)
}

fn marginal(
    team: usize,
    games: &[RankGame],
    work: &mut usize,
    work_limit: usize,
) -> Option<Vec<f64>> {
    let mut dist = vec![1.];
    for g in games {
        let gains = if g.home == team { g.hg } else { g.ag };
        let max_gain = *gains.iter().max()?;
        if gains.iter().any(|&v| v < 0) {
            return None;
        }
        let mut next = vec![0.; dist.len().checked_add(max_gain as usize)?];
        let z = g.prob.iter().sum::<f64>();
        if !z.is_finite() || z <= 0. {
            return None;
        }
        for (score, mass) in dist.iter().enumerate() {
            if *mass == 0. {
                continue;
            }
            for o in 0..3 {
                consume_work(work, 1, work_limit)?;
                let prob = g.prob[o] / z;
                if prob > 0. {
                    next[score + gains[o] as usize] += mass * prob;
                }
            }
        }
        let scale = next.iter().copied().fold(0.0_f64, f64::max);
        if !scale.is_finite() || scale <= 0. {
            return None;
        }
        for value in &mut next {
            *value /= scale;
        }
        dist = next;
    }
    let z = dist.iter().sum::<f64>();
    if !z.is_finite() || z <= 0. {
        return None;
    }
    for value in &mut dist {
        *value /= z;
    }
    Some(dist)
}

fn interval_probability(
    dist: &[f64],
    base: i32,
    lo: i32,
    hi: i32,
    work: &mut usize,
    work_limit: usize,
) -> Option<f64> {
    let lo = (lo - base).max(0) as usize;
    let hi = hi.checked_sub(base)?;
    if hi < 0 || lo > hi as usize || lo >= dist.len() {
        return Some(0.);
    }
    let hi = (hi as usize).min(dist.len() - 1);
    let mut sum = 0.;
    for value in &dist[lo..=hi] {
        consume_work(work, 1, work_limit)?;
        sum += value;
    }
    Some(sum)
}

fn structural_scores(
    mode: &str,
    p: &Pattern,
    cell: Cell,
    above: bool,
    work: &mut usize,
    work_limit: usize,
) -> Option<f64> {
    let by_team = add_game_marginals(p, work, work_limit)?;
    if by_team[cell.team].len() != 0 || !p.mass.is_finite() || p.mass <= 0. {
        return None;
    }
    let mut distributions = Vec::with_capacity(p.base.len());
    for (team, games) in by_team.iter().enumerate() {
        distributions.push(marginal(team, games, work, work_limit)?);
    }
    let target = p.base[cell.team];
    let mut logs = Vec::with_capacity(p.base.len());
    let mut categories = Vec::with_capacity(p.base.len());
    for team in 0..p.base.len() {
        if team == cell.team {
            continue;
        }
        let dist = &distributions[team];
        let lo = interval_probability(
            dist,
            p.base[team],
            p.lower[team],
            p.upper[team],
            work,
            work_limit,
        )?;
        if !lo.is_finite() || lo <= 0. {
            return None;
        }
        logs.push(lo.ln());
        let mut interval_below = 0.;
        let mut interval_tied = 0.;
        let mut interval_higher = 0.;
        let lo_gain = (p.lower[team] - p.base[team]).max(0) as usize;
        let hi_gain = p.upper[team].checked_sub(p.base[team])?;
        if hi_gain >= 0 && lo_gain <= hi_gain as usize {
            for (gain, prob) in dist
                .iter()
                .enumerate()
                .take((hi_gain as usize).min(dist.len() - 1) + 1)
                .skip(lo_gain)
            {
                consume_work(work, 1, work_limit)?;
                let points = p.base[team] + gain as i32;
                if points < target {
                    interval_below += prob;
                } else if points > target {
                    interval_higher += prob;
                } else {
                    interval_tied += prob;
                }
            }
        }
        categories.push([
            interval_below / lo,
            interval_tied / lo,
            interval_higher / lo,
        ]);
    }
    let interval_log = p.mass.ln() + logs.iter().sum::<f64>();
    let score = match mode {
        "interval" => interval_log,
        "rank" => {
            let rivals = categories.len();
            let mut dp = vec![vec![0.; rivals + 1]; rivals + 1];
            dp[0][0] = 1.;
            for cat in categories {
                let mut next = vec![vec![0.; rivals + 1]; rivals + 1];
                for a in 0..=rivals {
                    for t in 0..=rivals {
                        if dp[a][t] == 0. {
                            continue;
                        }
                        next[a][t] += dp[a][t] * cat[0];
                        if t < rivals {
                            next[a][t + 1] += dp[a][t] * cat[1];
                        }
                        if a < rivals {
                            next[a + 1][t] += dp[a][t] * cat[2];
                        }
                        consume_work(work, 1, work_limit)?;
                    }
                }
                dp = next;
            }
            let rank_mass = (0..=rivals)
                .flat_map(|a| (0..=rivals).map(move |t| (a, t)))
                .filter(|(a, t)| *a <= cell.rank && cell.rank <= *a + *t)
                .map(|(a, t)| dp[a][t])
                .sum::<f64>();
            interval_log + rank_mass.max(f64::MIN_POSITIVE).ln()
        }
        "strict" => {
            let exact = if above {
                cell.rank
            } else {
                categories.len().saturating_sub(cell.rank)
            };
            let mut dp = vec![0.; categories.len() + 1];
            dp[0] = 1.;
            for cat in categories {
                let strict = if above { cat[2] } else { cat[0] };
                let other = 1. - strict;
                let mut next = vec![0.; dp.len()];
                for k in 0..dp.len() {
                    next[k] += dp[k] * other;
                    if k + 1 < next.len() {
                        next[k + 1] += dp[k] * strict;
                    }
                    consume_work(work, 1, work_limit)?;
                }
                dp = next;
            }
            interval_log
                + dp.get(exact)
                    .copied()
                    .unwrap_or(0.)
                    .max(f64::MIN_POSITIVE)
                    .ln()
        }
        _ => return None,
    };
    if !score.is_finite() {
        return None;
    }
    Some(score)
}

impl BranchStrata {
    pub fn clear_messages(&mut self) {
        self.retuned.clear();
        self.message_work = 0;
    }
    pub fn prepare_messages(
        &mut self,
        m: &Model,
        pilots: &[Result],
        count: usize,
        work_limit: usize,
    ) -> serde_json::Value {
        self.prepare_messages_config(
            m,
            pilots,
            count,
            work_limit,
            discovery_mode(),
            crate::rare_tail::family_structure(),
        )
    }

    fn prepare_messages_config(
        &mut self,
        m: &Model,
        pilots: &[Result],
        count: usize,
        work_limit: usize,
        mode: Option<&'static str>,
        family_structure: crate::rare_tail::family_config::FamilyStructure,
    ) -> serde_json::Value {
        self.clear_messages();
        let iterations = 3;
        let bounded = bounded_retuner_enabled(mode, family_structure);
        let work_limit = work_limit.min(DISCOVERY_LIMIT);
        let mut diagnostics = vec![None; self.strata.len()];
        let mut discovery_work = 0usize;
        if let Some(mode) = mode {
            let mut failed = false;
            for (i, stratum) in self.strata.iter().enumerate() {
                match structural_scores(
                    mode,
                    &stratum.pattern,
                    self.cell,
                    stratum.above,
                    &mut discovery_work,
                    work_limit,
                ) {
                    Some(score) => diagnostics[i] = Some(score),
                    None => {
                        failed = true;
                        break;
                    }
                }
            }
            if failed {
                self.message_work = discovery_work;
                return json!({"mode":mode,"attempted":true,"declined":true,"reason":"structural discovery exhausted or produced invalid score","selected":[],"work":self.message_work,"scores":diagnostics});
            }
        }
        let mut order: Vec<_> = (0..self.strata.len())
            .filter(|&i| mode.is_some() || pilots[i].probability > 0.)
            .collect();
        order.sort_by(|&a, &b| {
            if mode.is_some() {
                diagnostics[b]
                    .unwrap_or(f64::NEG_INFINITY)
                    .total_cmp(&diagnostics[a].unwrap_or(f64::NEG_INFINITY))
                    .then(a.cmp(&b))
            } else {
                pilots[b]
                    .std_err
                    .total_cmp(&pilots[a].std_err)
                    .then(a.cmp(&b))
            }
        });
        let (reserved_unseen, ranked_eligible) = if mode.is_some() {
            let unseen: Vec<_> = order
                .iter()
                .copied()
                .filter(|&i| pilots[i].hits == 0 && diagnostics[i].is_some_and(f64::is_finite))
                .collect();
            (unseen, order.clone())
        } else {
            (Vec::new(), order.clone())
        };
        self.message_work = discovery_work;
        let mut attempts = Vec::new();
        if bounded {
            let tight_guide_bound = tight_guide_bound_enabled();
            let mut predicted_guide_charges = vec![None; self.strata.len()];
            let mut diagnostic_work = self.message_work;
            let max_attempts = if mode.is_some() {
                order.len()
            } else {
                count.min(order.len())
            };
            let results = {
                let strata = &self.strata;
                let work = &mut self.message_work;
                retry_bounded_candidates(
                    &reserved_unseen,
                    &ranked_eligible,
                    count,
                    max_attempts,
                    work,
                    work_limit,
                    |i, work, limit| {
                        retune_bounded_mode(
                            &strata[i].pattern,
                            m.fixtures.len(),
                            iterations,
                            work,
                            limit,
                            tight_guide_bound,
                            Some(&mut predicted_guide_charges[i]),
                        )
                    },
                )
            };
            for (i, work_delta, result) in results {
                diagnostic_work = diagnostic_work.saturating_add(work_delta);
                let remaining_grant = work_limit.saturating_sub(diagnostic_work);
                let unseen = pilots[i].hits == 0;
                match result {
                    Ok(q) => {
                        self.retuned.insert(i, q);
                        let mut attempt = json!({"index":i,"unseen":unseen,"work_delta":work_delta,"rejection":null});
                        if tight_guide_bound {
                            attempt["predicted_guide_charge"] = json!(predicted_guide_charges[i]);
                            attempt["guide_bound_mode"] = json!("tight");
                            attempt["remaining_grant"] = json!(remaining_grant);
                        }
                        attempts.push(attempt);
                    }
                    Err(reason) => {
                        let mut attempt = json!({"index":i,"unseen":unseen,"work_delta":work_delta,"rejection":reason.label()});
                        if tight_guide_bound {
                            attempt["predicted_guide_charge"] = json!(predicted_guide_charges[i]);
                            attempt["guide_bound_mode"] = json!("tight");
                            attempt["remaining_grant"] = json!(remaining_grant);
                        }
                        attempts.push(attempt);
                    }
                }
            }
        } else {
            for &i in order.iter().take(count.min(order.len())) {
                if let Some(q) = retune_legacy(
                    &self.strata[i].pattern,
                    m.fixtures.len(),
                    iterations,
                    &mut self.message_work,
                ) {
                    self.retuned.insert(i, q);
                }
            }
        }
        let mut selected: Vec<_> = self.retuned.keys().copied().collect();
        selected.sort_unstable();
        if mode.is_none() {
            if bounded {
                return json!({"selected":selected,"iterations":iterations,"work":self.message_work,"attempts":attempts,"patterns":selected.iter().map(|&i|json!({"index":i,"mass":self.retuned[&i].pattern.mass,"joint_teams":self.retuned[&i].pattern.joint.selected.iter().map(|&t|m.ids[t]).collect::<Vec<_>>(),"guided":self.retuned[&i].pattern.guide.games.len()})).collect::<Vec<_>>()});
            }
            return json!({"selected":selected,"iterations":iterations,"work":self.message_work,"patterns":selected.iter().map(|&i|json!({"index":i,"mass":self.retuned[&i].pattern.mass,"joint_teams":self.retuned[&i].pattern.joint.selected.iter().map(|&t|m.ids[t]).collect::<Vec<_>>(),"guided":self.retuned[&i].pattern.guide.games.len()})).collect::<Vec<_>>()});
        }
        let max_log = diagnostics
            .iter()
            .flatten()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        json!({"mode":mode,"attempted":true,"selected":selected,"iterations":iterations,"work":self.message_work,"attempts":attempts,"scores":diagnostics.iter().enumerate().map(|(i,s)|json!({"index":i,"log_score":s,"relative_score":s.map(|v|(v-max_log).exp())})).collect::<Vec<_>>(),"patterns":selected.iter().map(|&i|json!({"index":i,"mass":self.retuned[&i].pattern.mass,"joint_teams":self.retuned[&i].pattern.joint.selected.iter().map(|&t|m.ids[t]).collect::<Vec<_>>(),"guided":self.retuned[&i].pattern.guide.games.len()})).collect::<Vec<_>>()})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{conditioned::canonical_ranks, model::Request};

    #[test]
    fn tight_guide_estimate_matches_forward_and_reverse_table_rows() {
        let games = [
            RankGame {
                index: 0,
                home: 0,
                away: 1,
                prob: [1., 0., 0.],
                hg: [1, 90, 2],
                ag: [2, 3, 4],
            },
            RankGame {
                index: 1,
                home: 2,
                away: 3,
                prob: [0., 1., 0.],
                hg: [80, 2, 70],
                ag: [1, 1, 1],
            },
        ];
        let estimate = estimate_guide_charge(&games, 5).unwrap();
        let guide = Guide::new(games.to_vec(), 5);
        let actual = 2 * guide
            .cdf
            .values
            .iter()
            .chain(&guide.reverse.values)
            .map(|row| row.len())
            .sum::<usize>();
        assert_eq!(estimate.forward_span, 2);
        assert_eq!(estimate.reverse_span, 90);
        assert_eq!(estimate.charge, actual);
        let coarse_span = games
            .iter()
            .map(|g| *g.hg.iter().chain(&g.ag).max().unwrap() as usize)
            .sum::<usize>();
        let coarse = 2 * (games.len() + 1) * 5 * (coarse_span + 1);
        assert!(estimate.charge < coarse);
    }

    #[test]
    fn tight_guide_estimate_handles_empty_and_rejects_invalid_inputs() {
        let empty = estimate_guide_charge(&[], 4).unwrap();
        assert_eq!(empty.charge, 16);
        let invalid = [RankGame {
            index: 0,
            home: 0,
            away: 1,
            prob: [1., 0., 0.],
            hg: [0, -1, 0],
            ag: [0; 3],
        }];
        assert!(estimate_guide_charge(&invalid, 2).is_none());
        let bad_index = [RankGame {
            index: 0,
            home: 0,
            away: 2,
            prob: [1., 0., 0.],
            hg: [0; 3],
            ag: [0; 3],
        }];
        assert!(estimate_guide_charge(&bad_index, 2).is_none());
        let oversized_gain = [RankGame {
            index: 0,
            home: 0,
            away: 1,
            prob: [1., 0., 0.],
            hg: [i32::MAX, 0, 0],
            ag: [0; 3],
        }];
        assert!(estimate_guide_charge(&oversized_gain, 2).is_none());
        let large = i32::MAX / 2 + 1;
        let overflowing_span = [
            RankGame {
                index: 0,
                home: 0,
                away: 1,
                prob: [1., 0., 0.],
                hg: [large, 0, 0],
                ag: [0; 3],
            },
            RankGame {
                index: 1,
                home: 0,
                away: 2,
                prob: [1., 0., 0.],
                hg: [large, 0, 0],
                ag: [0; 3],
            },
        ];
        assert!(estimate_guide_charge(&overflowing_span, 3).is_none());
        assert!(estimate_guide_charge(&[], usize::MAX).is_none());
    }

    #[test]
    fn coarse_guide_preflight_covers_both_tables_before_construction() {
        let teams = 5;
        let games = (0..4)
            .map(|i| RankGame {
                index: i,
                home: 0,
                away: i + 1,
                prob: [1., 1., 1.],
                hg: [10, 10, 10],
                ag: [10, 10, 10],
            })
            .collect::<Vec<_>>();
        let row_count = (games.len() + 1) * teams;
        let span_bound = games.len() * 10;
        let old_bound = row_count * (span_bound + 1) * 2;
        let corrected_bound = coarse_guide_charge(&games, teams).unwrap();
        let guide = Guide::new(games.clone(), teams);
        let actual_charge = 2 * guide
            .cdf
            .values
            .iter()
            .chain(&guide.reverse.values)
            .map(|row| row.len())
            .sum::<usize>();

        assert!(guide_charge_fits(0, old_bound, old_bound).is_some());
        assert!(guide_charge_fits(0, corrected_bound, old_bound).is_none());
        assert!(actual_charge > old_bound);
        assert!(actual_charge <= corrected_bound);
    }

    #[test]
    fn work_allowance_refuses_overshoot_without_charging_unperformed_work() {
        let mut work = 0;
        assert!(consume_work(&mut work, 8, 8).is_some());
        assert_eq!(work, 8);
        assert!(consume_work(&mut work, 1, 8).is_none());
        assert_eq!(work, 8);
        assert!(consume_work(&mut work, 1, 0).is_none());
        assert_eq!(work, 8);
    }

    #[test]
    fn counted_slice_sum_declines_when_the_scan_exceeds_its_allowance() {
        let values = [1., 2., 3., 4.];
        let mut work = 7;
        assert_eq!(sum_counted_slice(&values, &mut work, 10), None);
        assert_eq!(work, 7);

        assert_eq!(sum_counted_slice(&values, &mut work, 11), Some(10.));
        assert_eq!(work, 11);
    }

    #[test]
    fn bounded_retuner_selection_preserves_full_default_and_enables_partial_or_tree_modes() {
        use crate::rare_tail::family_config::FamilyStructure::{Full, Skeleton};
        assert!(!bounded_retuner_enabled(None, Full));
        assert!(!bounded_retuner_enabled(Some("invalid"), Full));
        assert!(bounded_retuner_enabled(None, Skeleton));
        assert!(bounded_retuner_enabled(Some("interval"), Full));
    }

    #[test]
    fn bounded_candidate_retry_fills_success_count_without_overspending() {
        let mut work = 0;
        let attempts = retry_bounded_candidates(
            &[10],
            &[10, 11, 12],
            1,
            3,
            &mut work,
            10,
            |index, work, limit| {
                if index == 10 {
                    consume_work(work, 2, limit).ok_or(RetuneFailure::WorkAllowance)?;
                    Err(RetuneFailure::InvalidMessage)
                } else {
                    consume_work(work, 3, limit).ok_or(RetuneFailure::WorkAllowance)?;
                    Ok(index)
                }
            },
        );
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0].0, 10);
        assert_eq!(attempts[0].1, 2);
        assert_eq!(attempts[0].2, Err(RetuneFailure::InvalidMessage));
        assert_eq!(attempts[1].0, 11);
        assert_eq!(attempts[1].1, 3);
        assert_eq!(attempts[1].2, Ok(11));
        assert_eq!(work, 5);

        let mut capped_work = 0;
        let capped = retry_bounded_candidates(
            &[],
            &[20, 21],
            1,
            2,
            &mut capped_work,
            4,
            |index, work, limit| {
                if index == 20 {
                    consume_work(work, 4, limit).ok_or(RetuneFailure::WorkAllowance)?;
                    Err(RetuneFailure::InvalidMass)
                } else {
                    consume_work(work, 1, limit).ok_or(RetuneFailure::WorkAllowance)?;
                    Ok(index)
                }
            },
        );
        assert_eq!(capped.len(), 2);
        assert_eq!(capped[0].2, Err(RetuneFailure::InvalidMass));
        assert_eq!(capped[1].2, Err(RetuneFailure::WorkAllowance));
        assert_eq!(capped_work, 4);
        assert!(capped_work <= 4);
    }

    #[test]
    fn failed_metered_setup_still_charges_consumed_nodes() {
        let mut work = 12;
        let result: Option<()> = settle_node_usage(None, 5, 2, &mut work, 60);
        assert!(result.is_none());
        assert_eq!(work, 60);
    }
    fn model() -> Model {
        let r:Request=serde_json::from_value(json!({"id":1,"phase":{"sort":"pt,w,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..4).map(|t|json!({"team_id":t,"add_sub":t,"bias":t})).collect::<Vec<_>>(),
            "games": ([(0,1),(0,2),(1,3),(2,3)]).iter().enumerate().map(|(i,&(h,a))|json!({"id":i,"home_id":h,"away_id":a,"home_power":1.1,"away_power":0.9})).collect::<Vec<_>>() })).unwrap();
        Model::new(r).unwrap()
    }
    #[test]
    fn prepared_messages_spend_only_reserved_work_and_report_partial_attempts() {
        let m = model();
        let mut p =
            BranchStrata::with_tree(&m, Cell { team: 0, rank: 1 }, 808, 1, 32, 0, 4_000_000)
                .unwrap();
        let mut pilots = vec![Result::default(); p.len()];
        let guided = p
            .strata
            .iter()
            .position(|stratum| {
                !stratum.pattern.guide.games.is_empty() && stratum.pattern.omitted.is_empty()
            })
            .unwrap();
        pilots[guided].probability = 1.0;
        let info = p.prepare_messages_config(
            &m,
            &pilots,
            1,
            1,
            None,
            crate::rare_tail::family_config::FamilyStructure::Skeleton,
        );
        assert_eq!(info["work"].as_u64(), Some(1));
        assert!(p.message_work <= 1);
        assert!(info["attempts"][0]["work_delta"].as_u64().unwrap() <= 1);

        let info = p.prepare_messages_config(
            &m,
            &pilots,
            1,
            0,
            None,
            crate::rare_tail::family_config::FamilyStructure::Skeleton,
        );
        assert_eq!(info["work"].as_u64(), Some(0));
        assert_eq!(p.message_work, 0);
    }

    #[test]
    fn retune_stays_within_grant_when_node_setup_exhausts() {
        let m = model();
        let p = BranchStrata::with_tree(&m, Cell { team: 0, rank: 1 }, 808, 1, 32, 0, 4_000_000)
            .unwrap();
        let pattern = &p
            .strata
            .iter()
            .map(|stratum| &stratum.pattern)
            .find(|pattern| !pattern.guide.games.is_empty() && pattern.omitted.is_empty())
            .unwrap();
        let mut proposal_work = 0;
        assert!(proposal_rows(pattern, 3, &mut proposal_work, DISCOVERY_LIMIT).is_ok());

        let limit = proposal_work + 32;
        let mut spent = 0;
        assert!(retune(pattern, m.fixtures.len(), 3, &mut spent, limit).is_none());
        assert!(spent <= limit);
    }

    #[test]
    fn legacy_retuner_matches_the_grant_bounded_retuner_with_ample_budget() {
        let m = model();
        let p = BranchStrata::with_tree(&m, Cell { team: 0, rank: 1 }, 808, 1, 32, 0, 4_000_000)
            .unwrap();
        let pattern = p
            .strata
            .iter()
            .map(|stratum| &stratum.pattern)
            .find(|pattern| !pattern.guide.games.is_empty() && pattern.omitted.is_empty())
            .unwrap();
        let mut legacy_work = 0;
        let legacy = retune_legacy(pattern, m.fixtures.len(), 3, &mut legacy_work).unwrap();
        let mut bounded_work = 0;
        let mut predicted = None;
        let tight = retune_bounded_mode(
            pattern,
            m.fixtures.len(),
            3,
            &mut bounded_work,
            DISCOVERY_LIMIT,
            true,
            Some(&mut predicted),
        )
        .unwrap();
        assert_eq!(
            predicted,
            Some(
                2 * tight
                    .pattern
                    .guide
                    .cdf
                    .values
                    .iter()
                    .chain(&tight.pattern.guide.reverse.values)
                    .map(|row| row.len())
                    .sum::<usize>()
            )
        );
        assert!(bounded_work > legacy_work);
        assert_eq!(legacy.pattern.mass.to_bits(), tight.pattern.mass.to_bits());
        assert_eq!(
            legacy.pattern.joint.mass.to_bits(),
            tight.pattern.joint.mass.to_bits()
        );
        assert_eq!(
            legacy.pattern.guide.games.len(),
            tight.pattern.guide.games.len()
        );
        assert_eq!(legacy.nominal.as_ref(), tight.nominal.as_ref());
        for (legacy, bounded) in legacy
            .pattern
            .joint
            .games
            .iter()
            .chain(&legacy.pattern.guide.games)
            .zip(
                tight
                    .pattern
                    .joint
                    .games
                    .iter()
                    .chain(&tight.pattern.guide.games),
            )
        {
            assert_eq!(
                legacy.prob.map(f64::to_bits),
                bounded.prob.map(f64::to_bits)
            );
        }
        let exact_limit = bounded_work;
        let mut coarse_work = 0;
        let coarse = retune_bounded_mode(
            pattern,
            m.fixtures.len(),
            3,
            &mut coarse_work,
            exact_limit,
            false,
            None,
        );
        let mut exact_work = 0;
        let exact = retune_bounded_mode(
            pattern,
            m.fixtures.len(),
            3,
            &mut exact_work,
            exact_limit,
            true,
            None,
        );
        assert!(exact.is_ok());
        assert_eq!(exact_work, exact_limit);
        let _coarse_outcome = coarse;
        assert!(coarse_work <= exact_limit);
        let mut short_work = 0;
        assert!(retune_bounded_mode(
            pattern,
            m.fixtures.len(),
            3,
            &mut short_work,
            exact_limit - 1,
            true,
            None,
        )
        .is_err());
        assert!(short_work <= exact_limit - 1);
    }

    #[test]
    fn tight_guide_bound_admits_when_coarse_preflight_refuses() {
        let teams = 6;
        let games = (0..3)
            .map(|i| RankGame {
                index: i,
                home: 2 * i,
                away: 2 * i + 1,
                prob: [1., 0., 0.],
                hg: [1, 20, 0],
                ag: [1, 20, 0],
            })
            .collect::<Vec<_>>();
        let base = vec![0; teams];
        let lower = vec![i32::MIN / 8; teams];
        let upper = vec![i32::MAX / 8; teams];
        let mut node_budget = 10_000;
        let (joint, remaining) =
            NecessaryJoint::metered(&games, &base, &lower, &upper, 4, &mut node_budget).unwrap();
        assert!(joint.selected.is_empty());
        let pattern = Pattern {
            fixed: Vec::new(),
            bias: None,
            omitted: Vec::new(),
            base,
            lower,
            upper,
            guide: Arc::new(Guide::new(remaining, teams)),
            joint: Arc::new(joint),
            mass: 1.,
            tilt: 0.,
            cumulative: 1.,
            bound_guidance: false,
        };
        let mut tight_work = 0;
        let tight = retune_bounded_mode(
            &pattern,
            games.len(),
            1,
            &mut tight_work,
            DISCOVERY_LIMIT,
            true,
            None,
        );
        assert!(tight.is_ok());
        let exact_grant = tight_work;
        let mut coarse_work = 0;
        assert!(retune_bounded_mode(
            &pattern,
            games.len(),
            1,
            &mut coarse_work,
            exact_grant,
            false,
            None,
        )
        .is_err());
        assert!(coarse_work <= exact_grant);
        let mut exact_work = 0;
        assert!(retune_bounded_mode(
            &pattern,
            games.len(),
            1,
            &mut exact_work,
            exact_grant,
            true,
            None,
        )
        .is_ok());
        assert_eq!(exact_work, exact_grant);
        let mut short_work = 0;
        assert!(retune_bounded_mode(
            &pattern,
            games.len(),
            1,
            &mut short_work,
            exact_grant - 1,
            true,
            None,
        )
        .is_err());
        assert!(short_work <= exact_grant - 1);
    }

    #[test]
    fn tilted_joint_and_guide_exactly_restore_original_rank_mass() {
        let m = model();
        for rank in 0..m.n {
            let cell = Cell { team: 0, rank };
            let p = BranchStrata::with_tree(&m, cell, 808, 1, 32, 0, 4_000_000).unwrap();
            let mut expected = 0.;
            for mut code in 0..3usize.pow(m.fixtures.len() as u32) {
                let mut out = vec![0; m.fixtures.len()];
                let mut raw = 1.;
                for (i, g) in m.fixtures.iter().enumerate() {
                    let o = code % 3;
                    code /= 3;
                    out[i] = o as u8;
                    raw *= g.prob[o];
                }
                if canonical_ranks(&m, &out)[cell.team] == rank {
                    expected += raw;
                }
            }
            for bounds in [false, true] {
                let mut estimate = 0.;
                for s in &p.strata {
                    let mut work = 0;
                    let mut q = retune(&s.pattern, m.fixtures.len(), 6, &mut work, DISCOVERY_LIMIT)
                        .unwrap();
                    q.pattern.bound_guidance = bounds;
                    let uncorrected = q.pattern.clone();
                    for g in q.pattern.joint.games.iter().chain(&q.pattern.guide.games) {
                        for o in 0..3 {
                            assert!(g.prob[o] >= 0.099999 * q.nominal[g.index][o]);
                        }
                    }
                    for mut code in 0..3usize.pow(m.fixtures.len() as u32) {
                        let mut out = vec![0; m.fixtures.len()];
                        let mut raw = 1.;
                        for (i, g) in m.fixtures.iter().enumerate() {
                            let o = code % 3;
                            code /= 3;
                            out[i] = o as u8;
                            raw *= g.prob[o];
                        }
                        if canonical_ranks(&m, &out)[cell.team] != rank {
                            continue;
                        }
                        let mut points = vec![0; m.n];
                        let mut rng = Rng::new(123);
                        let w = q.pattern.draw(
                            cell,
                            &mut rng,
                            &mut out,
                            &mut points,
                            true,
                            true,
                            true,
                            0,
                            &mut Default::default(),
                        );
                        if w == 0. {
                            continue;
                        }
                        assert!(w.is_finite());
                        let w = w * q.correction(&out, &mut Default::default());
                        assert!(w.is_finite());
                        let uncorrected_weight = uncorrected.draw(
                            cell,
                            &mut rng,
                            &mut out,
                            &mut points,
                            true,
                            true,
                            true,
                            0,
                            &mut Default::default(),
                        );
                        let density = q
                            .pattern
                            .joint
                            .games
                            .iter()
                            .chain(&q.pattern.guide.games)
                            .fold(1., |v, g| v * g.prob[out[g.index] as usize])
                            / q.pattern.joint.mass
                            / uncorrected_weight;
                        estimate += density * w * q.pattern.mass;
                        let _ = raw;
                    }
                }
                assert!(
                    (estimate - expected).abs() < 1e-11,
                    "rank {rank} bounds {bounds}: {estimate} vs {expected}"
                );
            }
        }
    }
    #[test]
    fn message_work_exhaustion_retains_original_pattern() {
        let m = model();
        let cell = Cell { team: 0, rank: 1 };
        let p = BranchStrata::with_tree(&m, cell, 808, 1, 32, 0, 4_000_000).unwrap();
        let original = p
            .strata
            .iter()
            .find(|s| !s.pattern.guide.games.is_empty())
            .unwrap();
        let mut work = 20_000_000;
        assert!(retune(
            &original.pattern,
            m.fixtures.len(),
            6,
            &mut work,
            DISCOVERY_LIMIT
        )
        .is_none());
        assert!(!original.pattern.guide.games.is_empty());
    }
}
