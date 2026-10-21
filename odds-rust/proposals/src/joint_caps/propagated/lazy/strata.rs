//! Offline complete case stratification. Each requested branch is a disjoint
//! target path and strict exception mask; budgets never truncate the sum.
use super::*;
mod messages;
mod tree;
pub(super) const RIVAL_PATH_ENUMERATION_WORK_LIMIT: usize = 8_000_000;

struct Stratum {
    root: Vec<u8>,
    exceptions: Vec<usize>,
    above: bool,
    secondary: Option<usize>,
    pattern: Pattern,
}

pub struct ShortlistShares {
    pub shares: Vec<f64>,
    pub all_loss_leaf_ids: Vec<usize>,
    pub compatible_counts: Vec<usize>,
    pub unmatched_weight: f64,
    pub work_units: usize,
}

fn independent_pilot_shares_from_diagnostics(
    pair: &serde_json::Value,
    leaf_pilot: &serde_json::Value,
) -> Option<Vec<f64>> {
    let independent = pair
        .get("independent_normalized_shares")
        .and_then(serde_json::Value::as_array)
        .and_then(|values| {
            values
                .iter()
                .map(serde_json::Value::as_f64)
                .collect::<Option<Vec<_>>>()
        });
    let shares = match independent {
        Some(shares) => shares,
        None if pair.get("rolled_back")?.as_bool()? => leaf_pilot
            .get("shares")?
            .as_array()?
            .iter()
            .map(|value| value.as_f64())
            .collect::<Option<Vec<_>>>()?,
        None => return None,
    };
    if shares
        .iter()
        .any(|share| !share.is_finite() || *share < 0.0)
    {
        return None;
    }
    Some(shares)
}
pub struct BranchStrata {
    cell: Cell,
    retuned: HashMap<usize, messages::TiltedPattern>,
    message_work: usize,
    strata: Vec<Stratum>,
    goals: Option<crate::goal_tilt::GoalTilt>,
    setup_nodes: usize,
    secondary_fallbacks: usize,
    secondary_nodes: usize,
    tree_training_work: usize,
    tree_diagnostics: Option<serde_json::Value>,
}
pub struct BranchSample {
    pub result: Result,
    pub left: f64,
    pub right: f64,
}
impl Default for BranchSample {
    fn default() -> Self {
        Self {
            result: Result::default(),
            left: 0.0,
            right: 0.0,
        }
    }
}
#[derive(Clone, Copy)]
pub struct BranchBound {
    pub fixed_prior: f64,
    pub domain_prior: f64,
    pub normalizer_upper: f64,
    pub six_rival_upper: Option<f64>,
    pub read_two_upper: f64,
    pub upper_bound: f64,
}
// Probability factors are nonnegative and at most one. Flooring products is
// conservative and prevents numerical underflow from becoming a zero proof.
fn bound_product(a: f64, b: f64) -> f64 {
    (a * b).max(f64::MIN_POSITIVE)
}
fn read_two_upper(domain_prior: f64, event_masses: &[f64]) -> Option<f64> {
    if !domain_prior.is_finite()
        || domain_prior <= 0.0
        || event_masses
            .iter()
            .any(|mass| !mass.is_finite() || *mass <= 0.0)
    {
        return None;
    }
    let log_mass = event_masses
        .iter()
        .map(|mass| mass.min(1.0).ln())
        .sum::<f64>();
    if !log_mass.is_finite() {
        return None;
    }
    let intersection = (0.5 * log_mass).exp();
    let raw = domain_prior * intersection;
    if !intersection.is_finite() || intersection <= 0.0 || !raw.is_finite() || raw <= 0.0 {
        return None;
    }
    Some(bound_product(domain_prior, intersection))
}
/// Refine a complete exception branch by every legal outcome path of one
/// tightly capped/floored rival. Fix the path before rerunning all-team
/// propagation. Raw fixture masses enter the stratum normalizer exactly once.
fn secondary(
    builder: &mut LazyJoint,
    key: &[u8],
    points: &[i32],
    rivals: &[usize],
    initial: &Domains,
    above: bool,
    mask: u64,
    selected: u64,
) -> std::result::Result<Option<(usize, Vec<Pattern>)>, String> {
    let mut work = 0;
    secondary_with_options(
        builder,
        key,
        points,
        rivals,
        initial,
        above,
        mask,
        selected,
        16,
        true,
        usize::MAX,
        &mut work,
    )
}

pub(super) fn rival_path_partition(
    builder: &mut LazyJoint,
    key: &[u8],
    points: &[i32],
    rivals: &[usize],
    initial: &Domains,
    above: bool,
    mask: u64,
    selected: u64,
    work_limit: usize,
) -> (
    std::result::Result<Option<(usize, Vec<Pattern>)>, String>,
    usize,
) {
    let mut work = 0;
    let result = secondary_with_options(
        builder, key, points, rivals, initial, above, mask, selected, 32, false, work_limit,
        &mut work,
    );
    (result, work)
}

fn secondary_with_options(
    builder: &mut LazyJoint,
    key: &[u8],
    points: &[i32],
    rivals: &[usize],
    initial: &Domains,
    above: bool,
    mask: u64,
    selected: u64,
    path_limit: usize,
    require_low_conditional_mass: bool,
    work_limit: usize,
    work: &mut usize,
) -> std::result::Result<Option<(usize, Vec<Pattern>)>, String> {
    let charge = |work: &mut usize, n: usize| -> bool {
        let Some(next) = work.checked_add(n) else {
            return false;
        };
        if next > work_limit {
            *work = work_limit;
            return false;
        }
        *work = next;
        true
    };
    if !charge(
        work,
        builder
            .remaining
            .len()
            .saturating_mul(builder.base.len().saturating_add(1)),
    ) {
        return Err("rival-path domain work cap".into());
    }
    let target = points[builder.cell.team];
    let (mut lower, mut upper) = (initial.lower.clone(), initial.upper.clone());
    for &t in rivals {
        if selected & (1 << t) == 0 {
            continue;
        }
        let strict = mask & (1 << t) != 0;
        if above == strict {
            lower[t] = lower[t].max(target + i32::from(strict));
        } else {
            upper[t] = upper[t].min(target - i32::from(strict));
        }
    }
    let d = Domains::condition(
        &builder.remaining,
        points,
        rivals,
        builder.cell.rank,
        target,
        &lower,
        &upper,
        &initial.domains,
    );
    if !d.feasible {
        return Err("parent branch propagation disagrees".into());
    }
    type Paths = Vec<(Vec<(usize, u8)>, f64)>;
    fn paths(
        gs: &[(RankGame, u8)],
        t: usize,
        i: usize,
        value: i32,
        lo: i32,
        hi: i32,
        suffix: &[(i32, i32)],
        key: &mut Vec<(usize, u8)>,
        mass: f64,
        out: &mut Paths,
        work: &mut usize,
        work_limit: usize,
        path_limit: usize,
    ) -> bool {
        *work = work.saturating_add(1);
        if *work > work_limit {
            return false;
        }
        if value + suffix[i].0 > hi || value + suffix[i].1 < lo {
            return true;
        }
        if i == gs.len() {
            out.push((key.clone(), mass));
            return out.len() <= path_limit;
        }
        let (g, mask) = gs[i];
        let gains = if g.home == t { g.hg } else { g.ag };
        for o in 0..3 {
            *work = work.saturating_add(1);
            if *work > work_limit {
                return false;
            }
            if mask & (1 << o) == 0 || g.prob[o] <= 0. {
                continue;
            }
            key.push((g.index, o as u8));
            if !paths(
                gs,
                t,
                i + 1,
                value + gains[o],
                lo,
                hi,
                suffix,
                key,
                mass * g.prob[o],
                out,
                work,
                work_limit,
                path_limit,
            ) {
                return false;
            }
            key.pop();
        }
        true
    }
    let mut chosen: Option<(usize, Paths, f64)> = None;
    for &t in rivals {
        if !charge(work, builder.remaining.len()) {
            return Err("rival-path scan work cap".into());
        }
        let gs: Vec<_> = builder
            .remaining
            .iter()
            .zip(&d.domains)
            .filter(|(g, _)| g.home == t || g.away == t)
            .map(|(&g, &mask)| (g, mask))
            .collect();
        if gs.is_empty() || gs.iter().all(|(_, mask)| mask.count_ones() == 1) {
            continue;
        }
        let mut suffix = vec![(0, 0); gs.len() + 1];
        let mut domain_mass = 1.;
        for i in (0..gs.len()).rev() {
            if !charge(work, 4) {
                return Err("rival-path domain work cap".into());
            }
            let (g, mask) = gs[i];
            let gains = if g.home == t { g.hg } else { g.ag };
            let options: Vec<_> = (0..3)
                .filter(|&o| mask & (1 << o) != 0 && g.prob[o] > 0.)
                .collect();
            if options.is_empty() {
                return Err("zero numerical fixture domain".into());
            }
            suffix[i] = (
                suffix[i + 1].0 + options.iter().map(|&o| gains[o]).min().unwrap(),
                suffix[i + 1].1 + options.iter().map(|&o| gains[o]).max().unwrap(),
            );
            domain_mass *= options.iter().map(|&o| g.prob[o]).sum::<f64>();
        }
        let mut out = Vec::new();
        if !paths(
            &gs,
            t,
            0,
            points[t],
            d.lower[t],
            d.upper[t],
            &suffix,
            &mut Vec::new(),
            1.,
            &mut out,
            work,
            work_limit,
            path_limit,
        ) || out.len() < 2
        {
            if *work >= work_limit {
                return Err("rival-path enumeration work cap".into());
            }
            continue;
        }
        let conditional = out.iter().map(|(_, p)| p).sum::<f64>() / domain_mass;
        if !charge(work, out.len().saturating_add(gs.len())) {
            return Err("rival-path enumeration work cap".into());
        }
        if !conditional.is_finite() || (require_low_conditional_mass && conditional >= 0.9) {
            continue;
        }
        if chosen.as_ref().is_none_or(|(_, v, p)| {
            out.len() < v.len() || (out.len() == v.len() && conditional < *p)
        }) {
            chosen = Some((t, out, conditional));
        }
    }
    let Some((team, paths, _)) = chosen else {
        return Ok(None);
    };
    // Charge fixed-path filtering, base cloning, fixture scans, propagation
    // and guide reconstruction before mutating the builder. This fee applies
    // to failed builds too; node/guide counters account for builder search.
    let games = builder.remaining.len();
    let teams = builder.base.len();
    let max_fixed_len = games.min(32);
    let rebuild_fee = paths.len().saturating_mul(
        games
            .saturating_mul(2usize.saturating_mul(teams.saturating_add(1)))
            .saturating_add(games.saturating_mul(max_fixed_len).saturating_mul(2))
            .saturating_add(teams.saturating_mul(4))
            .saturating_add(64),
    );
    if !charge(work, rebuild_fee) {
        return Err("rival-path child rebuild work cap".into());
    }
    let original_base = builder.base.clone();
    let original_games = builder.remaining.clone();
    let mut patterns = Vec::new();
    let mut failed = false;
    for (fixed, mass) in paths {
        builder.base = original_base.clone();
        builder.remaining = original_games
            .iter()
            .copied()
            .filter(|g| !fixed.iter().any(|(i, _)| *i == g.index))
            .collect();
        for &(index, o) in &fixed {
            let g = original_games.iter().find(|g| g.index == index).unwrap();
            builder.base[g.home] += g.hg[o as usize];
            builder.base[g.away] += g.ag[o as usize];
        }
        match builder.build(key, builder.base.len(), None, Some((above, mask, selected))) {
            Ok(Some(mut p)) => {
                p.mass *= mass;
                if p.mass <= 0. || !p.mass.is_finite() {
                    failed = true;
                    break;
                }
                p.fixed.extend(fixed);
                patterns.push(p);
            }
            Ok(None) => {}
            Err(()) => {
                failed = true;
                break;
            }
        }
    }
    builder.base = original_base;
    builder.remaining = original_games;
    if failed || patterns.is_empty() {
        return Err("secondary setup incomplete; union skipped".into());
    }
    Ok(Some((team, patterns)))
}
impl BranchStrata {
    /// Map frozen packed shortlist regions onto existing final leaves. This only
    /// returns allocation weights; it does not change patterns or their masses.
    pub fn shortlist_guidance_shares(
        &self,
        m: &Model,
        candidates: &[crate::shortlist_guidance::Candidate],
    ) -> Option<ShortlistShares> {
        use crate::model::Key;
        let pal = *m.indices.get(&16)?;
        let mir = *m.indices.get(&584)?;
        if self.cell.team != pal || self.cell.rank != 13 || m.keys.get(1) != Some(&Key::W) { return None; }
        let base = self.leaf_pilot_independent_shares().unwrap_or_else(|| vec![1.0; self.strata.len()]);
        if base.len() != self.strata.len() { return None; }
        let pal_fixtures = m.fixtures.iter().filter(|fixture| fixture.home == pal || fixture.away == pal).collect::<Vec<_>>();
        let mut left = vec![0i32; m.n];
        for fixture in &m.fixtures { left[fixture.home] += 1; left[fixture.away] += 1; }
        let stride = m.base.iter().zip(&left).map(|(base, remaining)| base.wins + remaining).max()? + 1;
        if stride <= 0 { return None; }
        let packed_target = 57 * stride + 16;
        let all_loss = |root: &[u8]| {
            root.len() == pal_fixtures.len() && root.iter().zip(&pal_fixtures).all(|(&outcome, fixture)| {
                if fixture.home == pal { outcome == 0 }
                else if fixture.away == pal { outcome == 2 }
                else { true }
            })
        };
        let all_loss_indexes = self.strata.iter().enumerate()
            .filter_map(|(i, s)| all_loss(&s.root).then_some(i)).collect::<Vec<_>>();
        if all_loss_indexes.is_empty() { return None; }
        let mut shares = vec![0.0; self.strata.len()];
        let mut compatible_counts = Vec::with_capacity(candidates.len());
        let mut unmatched_weight = 0.0;
        let mut work_units = 0usize;
        for candidate in candidates {
            let mut below = candidate.below_ids.iter().filter_map(|id|m.indices.get(id).copied()).collect::<Vec<_>>();
            below.extend([*m.indices.get(&110)?, *m.indices.get(&318)?]);
            if candidate.mir_band == "any" && !below.contains(&mir) { return None; }
            let compatible = all_loss_indexes.iter().copied().filter(|&i| {
                work_units = work_units.saturating_add(m.n);
                let pattern = &self.strata[i].pattern;
                if pattern.lower.len() != m.n || pattern.upper.len() != m.n { return false; }
                for team in 0..m.n {
                    if team == pal { continue; }
                    let lo = pattern.lower[team];
                    let hi = pattern.upper[team];
                    if below.contains(&team) {
                        if lo > packed_target { return false; }
                    } else if hi < packed_target { return false; }
                }
                let band_value = match candidate.mir_band.as_str() {
                    "57" => Some(57 * stride + 16), "58" => Some(58 * stride + 16), "60" => Some(60 * stride + 17), _ => None,
                };
                band_value.is_none_or(|packed| pattern.lower[mir] <= packed && packed <= pattern.upper[mir])
            }).collect::<Vec<_>>();
            compatible_counts.push(compatible.len());
            if compatible.is_empty() {
                unmatched_weight += candidate.weight;
                continue;
            }
            let sum = compatible.iter().map(|&i|base[i].max(0.0)).sum::<f64>();
            if sum.is_finite() && sum > 0.0 {
                for &i in &compatible { shares[i] += candidate.weight * base[i].max(0.0) / sum; }
            } else {
                for &i in &compatible { shares[i] += candidate.weight / compatible.len() as f64; }
            }
        }
        let total = shares.iter().sum::<f64>();
        if !total.is_finite() || total <= 0.0 { return None; }
        for share in &mut shares { *share /= total; }
        Some(ShortlistShares { shares, all_loss_leaf_ids: all_loss_indexes, compatible_counts, unmatched_weight, work_units })
    }

    pub fn new(
        m: &Model,
        cell: Cell,
        seed: i64,
        rivals: usize,
    ) -> std::result::Result<Self, String> {
        Self::with_secondary(m, cell, seed, rivals, false)
    }
    pub fn with_secondary(
        m: &Model,
        cell: Cell,
        seed: i64,
        rivals: usize,
        refine: bool,
    ) -> std::result::Result<Self, String> {
        Self::with_secondary_quota(m, cell, seed, rivals, refine, 10000, 2000)
    }
    fn with_secondary_quota(
        m: &Model,
        cell: Cell,
        seed: i64,
        rivals: usize,
        refine: bool,
        secondary_quota: usize,
        per_parent_quota: usize,
    ) -> std::result::Result<Self, String> {
        let mut builder = LazyJoint::with_mode(m, cell, seed, &[], 0, false, false, None)
            .ok_or("unsupported model")?;
        builder.rival_limit = rivals.clamp(1, 6);
        builder.node_limit = 100000;
        // Enumerate every positive target-table path. Cached or witness paths
        // alone are insufficient to assert that strata cover the whole cell.
        fn roots(
            tab: &TerminalTable,
            t: usize,
            i: usize,
            added: usize,
            key: &mut Vec<u8>,
            out: &mut Vec<Vec<u8>>,
        ) -> std::result::Result<(), String> {
            if i == tab.games.len() {
                out.push(key.clone());
                return Ok(());
            }
            let g = tab.games[i];
            let gains = if g.home == t { g.hg } else { g.ag };
            for o in 0..3 {
                let next = added + gains[o] as usize;
                if g.prob[o] > 0. && tab.rows[i + 1].get(next).is_some_and(|&p| p > 0.) {
                    if out.len() >= 64 {
                        return Err("target path budget exhausted".into());
                    }
                    key.push(o as u8);
                    roots(tab, t, i + 1, next, key, out)?;
                    key.pop();
                }
            }
            Ok(())
        }
        let mut keys = Vec::new();
        roots(&builder.target, cell.team, 0, 0, &mut Vec::new(), &mut keys)?;
        let mut strata = Vec::new();
        let mut attempted = 0;
        for key in keys {
            let mut points = builder.base.clone();
            for (g, &o) in builder.target.games.iter().zip(&key) {
                points[g.home] += g.hg[o as usize];
                points[g.away] += g.ag[o as usize];
            }
            let target = points[cell.team];
            let ts: Vec<_> = (0..m.n).filter(|&t| t != cell.team).collect();
            let d = Domains::propagate(&builder.remaining, &points, &ts, cell.rank, target);
            if !d.feasible {
                continue;
            }
            let fixed_above = ts
                .iter()
                .filter(|&&t| points[t] + d.min[0][t] > target)
                .count();
            let fixed_below = ts
                .iter()
                .filter(|&&t| points[t] + d.max[0][t] < target)
                .count();
            let above = cell.rank - fixed_above <= m.n - 1 - cell.rank - fixed_below;
            let left = if above {
                cell.rank - fixed_above
            } else {
                m.n - 1 - cell.rank - fixed_below
            };
            let ambiguous: Vec<_> = ts
                .iter()
                .copied()
                .filter(|&t| points[t] + d.min[0][t] <= target && points[t] + d.max[0][t] >= target)
                .collect();
            fn masks(
                ts: &[usize],
                i: usize,
                left: usize,
                mask: u64,
                out: &mut Vec<u64>,
            ) -> std::result::Result<(), String> {
                if out.len() >= 256 {
                    return Err("case enumeration budget exhausted".into());
                }
                if i == ts.len() {
                    out.push(mask);
                    return Ok(());
                }
                masks(ts, i + 1, left, mask, out)?;
                if left > 0 {
                    masks(ts, i + 1, left - 1, mask | (1 << ts[i]), out)?;
                }
                Ok(())
            }
            let mut cases = Vec::new();
            masks(&ambiguous, 0, left, 0, &mut cases)?;
            let selected = ambiguous.iter().fold(0u64, |s, &t| s | (1 << t));
            for mask in cases {
                attempted += 1;
                if attempted > 256 {
                    return Err("total case budget exhausted".into());
                }
                match builder.build(&key, m.n, None, Some((above, mask, selected))) {
                    Ok(Some(pattern)) => {
                        let exceptions: Vec<_> = ambiguous
                            .iter()
                            .copied()
                            .filter(|&t| mask & (1 << t) != 0)
                            .collect();
                        strata.push(Stratum {
                            root: key.clone(),
                            exceptions,
                            above,
                            secondary: None,
                            pattern,
                        });
                    }
                    Ok(None) => {}
                    Err(()) => {
                        return Err(
                            "case setup/numerical budget exhausted; incomplete union skipped"
                                .into(),
                        )
                    }
                }
            }
        }
        if strata.is_empty() {
            return Err("no usable strata; not an impossibility proof".into());
        }
        // Finish the entire primary union before optional refinement can spend
        // its setup quota. A failed split must never erase an existing parent.
        let mut secondary_fallbacks = 0;
        let mut secondary_nodes = 0;
        if refine {
            let fallback = crate::search::enabled("RUST_ODDS_COMPLETE_PARENT_FALLBACK");
            let mut refined = Vec::new();
            for parent in &strata {
                if fallback && secondary_nodes >= secondary_quota {
                    secondary_fallbacks += 1;
                    break;
                }
                let mut points = builder.base.clone();
                for (g, &o) in builder.target.games.iter().zip(&parent.root) {
                    points[g.home] += g.hg[o as usize];
                    points[g.away] += g.ag[o as usize];
                }
                let rivals: Vec<_> = (0..m.n).filter(|&t| t != cell.team).collect();
                let d = Domains::propagate(
                    &builder.remaining,
                    &points,
                    &rivals,
                    cell.rank,
                    points[cell.team],
                );
                let mask = parent.exceptions.iter().fold(0, |v, &t| v | (1 << t));
                let target = points[cell.team];
                let selected = rivals
                    .iter()
                    .filter(|&&t| {
                        points[t] + d.min[0][t] <= target && points[t] + d.max[0][t] >= target
                    })
                    .fold(0, |v, &t| v | (1 << t));
                let limit = builder.node_limit;
                let before = builder.nodes;
                if fallback {
                    builder.node_limit =
                        limit.min(before + per_parent_quota.min(secondary_quota - secondary_nodes));
                }
                let split = secondary(
                    &mut builder,
                    &parent.root,
                    &points,
                    &rivals,
                    &d,
                    parent.above,
                    mask,
                    selected,
                );
                builder.node_limit = limit;
                secondary_nodes += builder.nodes - before;
                match split {
                    Ok(Some((team, patterns))) => {
                        for pattern in patterns {
                            refined.push(Stratum {
                                root: parent.root.clone(),
                                exceptions: parent.exceptions.clone(),
                                above: parent.above,
                                secondary: Some(team),
                                pattern,
                            });
                        }
                    }
                    Ok(None) => refined.push(Stratum {
                        root: parent.root.clone(),
                        exceptions: parent.exceptions.clone(),
                        above: parent.above,
                        secondary: None,
                        pattern: parent.pattern.clone(),
                    }),
                    Err(reason) if !fallback => return Err(reason),
                    Err(_) => {
                        secondary_fallbacks += 1;
                        break;
                    }
                }
            }
            // Keep the complete primary proposal if any optional split fails.
            // Mixing only the successful children changes allocation across
            // parents and can make an otherwise useful proposal less reliable.
            if secondary_fallbacks == 0 {
                strata = refined;
            }
        }
        Ok(Self {
            retuned: HashMap::new(),
            message_work: 0,
            cell,
            strata,
            goals: builder.goals,
            setup_nodes: builder.nodes,
            secondary_fallbacks,
            secondary_nodes,
            tree_training_work: 0,
            tree_diagnostics: None,
        })
    }
    pub fn len(&self) -> usize {
        self.strata.len()
    }
    pub fn is_empty(&self) -> bool {
        self.strata.is_empty()
    }
    pub fn setup_diagnostics(&self) -> serde_json::Value {
        json!({"nodes":self.setup_nodes,"secondary_nodes":self.secondary_nodes,"tree":self.tree_diagnostics,
            "complete_parent_fallback":self.secondary_fallbacks>0})
    }
    pub fn leaf_pilot_work_units(&self) -> Option<usize> {
        self.tree_diagnostics
            .as_ref()?
            .get("leaf_pilot")?
            .get("work_units")?
            .as_u64()
            .and_then(|work| usize::try_from(work).ok())
    }
    /// Root shares and leaf indexes for the optional final-sampling allocator.
    /// Missing or incomplete screen diagnostics make the caller use its legacy plan.
    pub fn path_priority_sampling_groups(&self) -> Option<Vec<(Vec<u8>, f64, Vec<usize>)>> {
        let path = self.tree_diagnostics.as_ref()?.get("path_priority")?;
        if path.get("enabled")?.as_bool() != Some(true) || !path.get("fallback_reason")?.is_null() {
            return None;
        }
        let roots = path.get("roots")?.as_array()?;
        let mut groups = Vec::new();
        let mut root_index = std::collections::HashMap::new();
        for root in roots {
            let outcomes = root
                .get("outcomes")?
                .as_array()?
                .iter()
                .map(|value| u8::try_from(value.as_u64()?).ok())
                .collect::<Option<Vec<_>>>()?;
            let share = root.get("operational_share")?.as_f64()?;
            let index = groups.len();
            if root_index.insert(outcomes.clone(), index).is_some() {
                return None;
            }
            groups.push((outcomes, share, Vec::new()));
        }
        for (leaf, stratum) in self.strata.iter().enumerate() {
            let index = *root_index.get(&stratum.root)?;
            groups[index].2.push(leaf);
        }
        groups.retain(|(_, _, leaves)| !leaves.is_empty());
        (!groups.is_empty()).then_some(groups)
    }
    /// Raw screen scores and leaf indexes for the opt-in root exploration allocator.
    /// Scores remain the diagnostic proxy values; the operational-share accessor above
    /// intentionally retains its existing allocation semantics.
    pub fn path_priority_raw_sampling_groups(
        &self,
    ) -> Option<Vec<(Vec<u8>, f64, Vec<usize>)>> {
        let path = self.tree_diagnostics.as_ref()?.get("path_priority")?;
        if path.get("enabled")?.as_bool() != Some(true) || !path.get("fallback_reason")?.is_null() {
            return None;
        }
        let roots = path.get("roots")?.as_array()?;
        let mut groups = Vec::with_capacity(roots.len());
        let mut root_index = std::collections::HashMap::new();
        for root in roots {
            let outcomes = root
                .get("outcomes")?
                .as_array()?
                .iter()
                .map(|value| u8::try_from(value.as_u64()?).ok())
                .collect::<Option<Vec<_>>>()?;
            let score = root.get("raw_score")?.as_f64()?;
            if !score.is_finite() || score < 0.0 {
                return None;
            }
            let index = groups.len();
            if root_index.insert(outcomes.clone(), index).is_some() {
                return None;
            }
            groups.push((outcomes, score, Vec::new()));
        }
        for (leaf, stratum) in self.strata.iter().enumerate() {
            let index = *root_index.get(&stratum.root)?;
            groups[index].2.push(leaf);
        }
        groups.retain(|(_, _, leaves)| !leaves.is_empty());
        let mut owners = vec![false; self.strata.len()];
        for (_, _, leaves) in &groups {
            for &leaf in leaves {
                if leaf >= owners.len() || std::mem::replace(&mut owners[leaf], true) {
                    return None;
                }
            }
        }
        (groups.len() >= 2 && owners.iter().all(|owner| *owner)).then_some(groups)
    }
    /// Per-root structural shares aligned with the current final strata order.
    pub fn leaf_pilot_groups(&self) -> Option<Vec<(Vec<u8>, Vec<usize>, Vec<f64>)>> {
        let diagnostics = self.tree_diagnostics.as_ref()?.get("leaf_pilot")?;
        if diagnostics.get("enabled")?.as_bool() != Some(true)
            || !diagnostics.get("fallback_reason")?.is_null()
        {
            return None;
        }
        let shares = diagnostics
            .get("shares")?
            .as_array()?
            .iter()
            .map(|value| value.as_f64())
            .collect::<Option<Vec<_>>>()?;
        if shares.len() != self.strata.len()
            || shares
                .iter()
                .any(|share| !share.is_finite() || *share < 0.0)
        {
            return None;
        }
        let mut groups: Vec<(Vec<u8>, Vec<usize>, Vec<f64>)> = Vec::new();
        let mut indexes = std::collections::HashMap::<Vec<u8>, usize>::new();
        for (leaf, stratum) in self.strata.iter().enumerate() {
            let index = if let Some(index) = indexes.get(&stratum.root) {
                *index
            } else {
                let index = groups.len();
                indexes.insert(stratum.root.clone(), index);
                groups.push((stratum.root.clone(), Vec::new(), Vec::new()));
                index
            };
            groups[index].1.push(leaf);
            groups[index].2.push(shares[leaf]);
        }
        if groups
            .iter()
            .any(|(_, _, weights)| (weights.iter().sum::<f64>() - 1.0).abs() > 1e-8)
        {
            return None;
        }
        Some(groups)
    }
    pub fn leaf_pilot_root_groups(&self) -> Option<Vec<(Vec<u8>, Vec<usize>)>> {
        let diagnostics = self.tree_diagnostics.as_ref()?.get("leaf_pilot")?;
        if diagnostics.get("enabled")?.as_bool() != Some(true) {
            return None;
        }
        let mut groups: Vec<(Vec<u8>, Vec<usize>)> = Vec::new();
        let mut indexes = std::collections::HashMap::<Vec<u8>, usize>::new();
        for (leaf, stratum) in self.strata.iter().enumerate() {
            let index = if let Some(index) = indexes.get(&stratum.root) {
                *index
            } else {
                let index = groups.len();
                indexes.insert(stratum.root.clone(), index);
                groups.push((stratum.root.clone(), Vec::new()));
                index
            };
            groups[index].1.push(leaf);
        }
        Some(groups)
    }
    /// Pair-corrected log scores and original independent shares aligned with
    /// final strata order; selected IDs define the hierarchical pilot groups.
    pub fn hierarchical_pair_pilot_data(&self) -> Option<(Vec<Option<f64>>, Vec<Vec<i32>>)> {
        let leaf_pilot = self.tree_diagnostics.as_ref()?.get("leaf_pilot")?;
        let pair = leaf_pilot.get("pair_pilot")?;
        if pair.get("enabled")?.as_bool() != Some(true)
            || pair.get("rolled_back")?.as_bool() != Some(false)
            || pair.get("adjusted_log_scores_valid")?.as_bool() != Some(true)
        {
            return None;
        }
        let logs = pair
            .get("adjusted_log_scores")?
            .as_array()?
            .iter()
            .map(|value| match value {
                serde_json::Value::Null => Some(None),
                _ => value.as_f64().filter(|score| score.is_finite()).map(Some),
            })
            .collect::<Option<Vec<_>>>()?;
        let leaf_rows = leaf_pilot.get("leaves")?.as_array()?;
        let selected = leaf_rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                if row.get("index")?.as_u64()? != index as u64 {
                    return None;
                }
                let mut ids = row
                    .get("selected")?
                    .as_array()?
                    .iter()
                    .map(|value| i32::try_from(value.as_i64()?).ok())
                    .collect::<Option<Vec<_>>>()?;
                ids.sort_unstable();
                Some(ids)
            })
            .collect::<Option<Vec<_>>>()?;
        if logs.len() != self.strata.len() || selected.len() != self.strata.len() {
            return None;
        }
        Some((logs, selected))
    }
    /// Original base-parent IDs and proxy shares for splitting each parent's
    /// uniform exploration mass over its refined children.
    pub fn rival_path_parent_map(&self) -> Option<(Vec<usize>, Vec<f64>)> {
        let diagnostics = self.tree_diagnostics.as_ref()?.get("leaf_pilot")?;
        if diagnostics.get("pair_pilot")?.get("enabled")?.as_bool() != Some(true) {
            return None;
        }
        let partition = self
            .tree_diagnostics
            .as_ref()?
            .get("rival_path_partition")?;
        if partition.get("enabled")?.as_bool() != Some(true) {
            return None;
        }
        let rows = diagnostics.get("leaves")?.as_array()?;
        if rows.len() != self.strata.len() {
            return None;
        }
        let mut parents = Vec::with_capacity(rows.len());
        let mut shares = Vec::with_capacity(rows.len());
        let mut totals = std::collections::HashMap::<usize, f64>::new();
        for (index, row) in rows.iter().enumerate() {
            if row.get("index")?.as_u64()? != index as u64 {
                return None;
            }
            let parent = usize::try_from(row.get("original_parent_id")?.as_u64()?).ok()?;
            let share = row.get("local_child_share")?.as_f64()?;
            if !share.is_finite() || share <= 0.0 {
                return None;
            }
            parents.push(parent);
            shares.push(share);
            *totals.entry(parent).or_default() += share;
        }
        if totals
            .values()
            .any(|sum| !sum.is_finite() || (*sum - 1.0).abs() > 1e-8)
        {
            return None;
        }
        Some((parents, shares))
    }
    pub fn rival_path_pilot_budgets(
        &self,
    ) -> Option<(bool, Vec<(Vec<u8>, Vec<i32>, usize)>)> {
        let partition = self
            .tree_diagnostics
            .as_ref()?
            .get("rival_path_partition")?;
        if partition.get("enabled")?.as_bool() != Some(true) {
            return None;
        }
        let applied = partition.get("applied")?.as_bool()?;
        let budgets = partition
            .get("frozen_group_budgets")?
            .as_array()?
            .iter()
            .map(|row| {
                let root = row
                    .get("root")?
                    .as_array()?
                    .iter()
                    .map(|value| u8::try_from(value.as_u64()?).ok())
                    .collect::<Option<Vec<_>>>()?;
                let selected = row
                    .get("selected_ids")?
                    .as_array()?
                    .iter()
                    .map(|value| i32::try_from(value.as_i64()?).ok())
                    .collect::<Option<Vec<_>>>()?;
                let quota = usize::try_from(row.get("draws")?.as_u64()?).ok()?;
                Some((root, selected, quota))
            })
            .collect::<Option<Vec<_>>>()?;
        Some((applied, budgets))
    }
    pub fn rival_path_base_pilot_counts(&self) -> Option<Vec<usize>> {
        if !self.rival_path_partition_applied() {
            return None;
        }
        let values = self
            .tree_diagnostics
            .as_ref()?
            .get("rival_path_partition")?
            .get("base_leaf_pilot")?
            .as_array()?;
        let counts = values
            .iter()
            .map(|value| usize::try_from(value.as_u64()?).ok())
            .collect::<Option<Vec<_>>>()?;
        (counts.len() == self.strata.len() && counts.iter().all(|count| *count >= 5))
            .then_some(counts)
    }
    pub fn rival_path_partition_applied(&self) -> bool {
        self.tree_diagnostics
            .as_ref()
            .and_then(|diagnostics| diagnostics.get("rival_path_partition"))
            .and_then(|event| event.get("applied"))
            .and_then(serde_json::Value::as_bool)
            == Some(true)
    }
    pub fn rival_path_sampling_keys(&self, pilot: bool) -> Option<Vec<String>> {
        let rows = self
            .tree_diagnostics
            .as_ref()?
            .get("leaf_pilot")?
            .get("leaves")?
            .as_array()?;
        if rows.len() != self.strata.len()
            || self
                .tree_diagnostics
                .as_ref()?
                .get("rival_path_partition")?
                .get("enabled")?
                .as_bool()
                != Some(true)
        {
            return None;
        }
        rows.iter()
            .enumerate()
            .map(|(index, row)| {
                if row.get("index")?.as_u64()? != index as u64 {
                    return None;
                }
                let parent = row.get("original_parent_id")?.as_u64()?;
                let path = row.get("path_id").and_then(serde_json::Value::as_u64);
                Some(path.map_or_else(
                    || {
                        if pilot {
                            parent.to_string()
                        } else {
                            format!("leaf-{parent}")
                        }
                    },
                    |path| format!("parent-{parent}-path-{path}"),
                ))
            })
            .collect()
    }
    /// Independent pair-pilot shares, or the ordinary leaf guide shares when
    /// the pair pass rolled back, aligned with final strata order.
    pub fn leaf_pilot_independent_shares(&self) -> Option<Vec<f64>> {
        let leaf_pilot = self.tree_diagnostics.as_ref()?.get("leaf_pilot")?;
        let pair = leaf_pilot.get("pair_pilot")?;
        let shares = independent_pilot_shares_from_diagnostics(pair, leaf_pilot)?;
        if shares.len() != self.strata.len()
            || shares
                .iter()
                .any(|share| !share.is_finite() || *share < 0.0)
        {
            return None;
        }
        Some(shares)
    }
    /// Bounds under independent original fixture outcomes, not proposal Q.
    /// Six-rival setup is offline only. Failed setup leaves the existing bound.
    pub fn bounds(&self, m: &Model, strengthen: bool) -> Vec<BranchBound> {
        self.strata
            .iter()
            .map(|s| {
                let p = &s.pattern;
                let fixed_prior: f64 = p
                    .fixed
                    .iter()
                    .map(|&(i, o)| m.fixtures[i].prob[o as usize])
                    .fold(1., bound_product);
                let games: Vec<_> = p
                    .joint
                    .games
                    .iter()
                    .chain(&p.guide.games)
                    .chain(&p.omitted)
                    .copied()
                    .collect();
                let domain_prior = games.iter().fold(fixed_prior, |mass, g| {
                    bound_product(
                        mass,
                        (0..3)
                            .filter(|&o| g.prob[o] > 0.)
                            .map(|o| m.fixtures[g.index].prob[o])
                            .sum::<f64>(),
                    )
                });
                let six_rival_upper = if strengthen {
                    NecessaryJoint::new(&games, &p.base, &p.lower, &p.upper, 6, 20000)
                        .filter(|(j, _)| j.mass > 0. && j.mass.is_finite())
                        .map(|(j, _)| bound_product(domain_prior, j.mass))
                } else {
                    None
                };
                // The joint-selected interval event and each remaining team's
                // interval form a read-2 family: a fixture affects at most two
                // events. Generalized Holder bounds the intersection by the
                // square root of the product of these event probabilities.
                // The plain residual_hint product is NOT an upper bound.
                let mut log_mass = p.joint.mass.ln();
                if strengthen {
                    for t in 0..m.n {
                        if p.joint.selected.contains(&t) {
                            continue;
                        }
                        let gs: Vec<_> = games
                            .iter()
                            .copied()
                            .filter(|g| g.home == t || g.away == t)
                            .collect();
                        let span: i32 = gs
                            .iter()
                            .map(|g| {
                                *if g.home == t { &g.hg } else { &g.ag }
                                    .iter()
                                    .max()
                                    .unwrap()
                            })
                            .sum();
                        let tab = TerminalTable::new(
                            gs,
                            t,
                            (0..=span)
                                .map(|gain| {
                                    if p.base[t] + gain >= p.lower[t]
                                        && p.base[t] + gain <= p.upper[t]
                                    {
                                        1.
                                    } else {
                                        0.
                                    }
                                })
                                .collect(),
                        );
                        let mass = tab.mass(0);
                        // A numerical zero must not become a proof; retain a
                        // looser bound on this event instead.
                        if mass > 0. && mass.is_finite() {
                            log_mass += mass.min(1.).ln();
                        }
                    }
                }
                let read_two_upper = if strengthen {
                    bound_product(domain_prior, (0.5 * log_mass).exp())
                } else {
                    1.
                };
                let raw = fixed_prior
                    .min(p.mass)
                    .min(six_rival_upper.unwrap_or(1.))
                    .min(read_two_upper);
                // Ordinary f64 DP arithmetic, not interval-certified arithmetic.
                // Retain headroom and never turn underflow into a zero proof.
                let upper_bound = if raw > 0. && raw.is_finite() {
                    (raw * (1. + 1e-9)).min(1.)
                } else {
                    fixed_prior
                };
                BranchBound {
                    fixed_prior,
                    domain_prior,
                    normalizer_upper: p.mass,
                    six_rival_upper,
                    read_two_upper,
                    upper_bound,
                }
            })
            .collect()
    }

    /// Read-2-only bounds for the bounded omission experiment. Unlike
    /// `bounds(..., true)`, this never runs the six-rival setup. Work is
    /// charged conservatively before each terminal table is allocated.
    pub fn bounded_omission_bounds(
        &self,
        m: &Model,
        work_cap: usize,
    ) -> (Option<Vec<BranchBound>>, usize, Vec<usize>, Option<&'static str>) {
        let mut work;
        let mut fallback_leaves = Vec::new();

        // Charge the false-bound pass first: bounds(false) still scans each
        // leaf's fixed outcomes and all original pattern games.
        let preflight_work = self.strata.len();
        let baseline_work = self.strata.iter().try_fold(preflight_work, |total, stratum| {
            let p = &stratum.pattern;
            let games = p
                .joint
                .games
                .len()
                .checked_add(p.guide.games.len())?
                .checked_add(p.omitted.len())?;
            total.checked_add(
                p.fixed
                    .len()
                    .checked_add(games.checked_mul(8)?)?
                    .checked_add(8)?,
            )
        });
        let Some(baseline_work) = baseline_work else {
            return (
                None,
                preflight_work,
                fallback_leaves,
                Some("work accounting overflow"),
            );
        };
        if baseline_work > work_cap {
            return (
                None,
                preflight_work,
                fallback_leaves,
                Some("work cap exhausted during false bounds"),
            );
        }
        work = baseline_work;
        let mut bounds = self.bounds(m, false);

        for (leaf, stratum) in self.strata.iter().enumerate() {
            let p = &stratum.pattern;
            if !p.omitted.is_empty() {
                fallback_leaves.push(leaf);
                continue;
            }
            let scan_charge = p
                .joint
                .games
                .len()
                .checked_add(p.guide.games.len())
                .and_then(|games| {
                    p.fixed
                        .len()
                        .checked_add(games.checked_mul(m.n.saturating_add(8))?)?
                        .checked_add(m.n.checked_mul(m.n)?)
                });
            let Some(scan_charge) = scan_charge else {
                return (Some(bounds), work, fallback_leaves, Some("work accounting overflow"));
            };
            let Some(next_work) = work.checked_add(scan_charge) else {
                return (Some(bounds), work, fallback_leaves, Some("work accounting overflow"));
            };
            if next_work > work_cap {
                return (Some(bounds), work, fallback_leaves, Some("work cap exhausted"));
            }
            work = next_work;
            let games: Vec<_> = p
                .joint
                .games
                .iter()
                .chain(&p.guide.games)
                .copied()
                .collect();
            let fixed_prior = p
                .fixed
                .iter()
                .map(|&(i, o)| m.fixtures[i].prob[o as usize])
                .fold(1., bound_product);
            let mut domain_prior = fixed_prior;
            for game in &games {
                domain_prior = bound_product(
                    domain_prior,
                    (0..3)
                        .filter(|&o| game.prob[o] > 0.)
                        .map(|o| m.fixtures[game.index].prob[o])
                        .sum::<f64>(),
                );
            }
            let mut event_masses = vec![p.joint.mass];
            let mut failed = !p.joint.mass.is_finite() || p.joint.mass <= 0.0;
            for team in 0..m.n {
                if failed {
                    break;
                }
                if p.joint.selected.contains(&team) {
                    continue;
                }
                let gs: Vec<_> = games
                    .iter()
                    .copied()
                    .filter(|g| g.home == team || g.away == team)
                    .collect();
                let span = gs
                    .iter()
                    .map(|g| {
                        *if g.home == team { &g.hg } else { &g.ag }
                            .iter()
                            .max()
                            .unwrap()
                    })
                    .try_fold(0i32, |sum, value| sum.checked_add(value));
                let Some(span) = span
                    .and_then(|span| usize::try_from(span).ok())
                    .and_then(|span| span.checked_add(1))
                else {
                    return (Some(bounds), work, fallback_leaves, Some("work accounting overflow"));
                };
                let Some(table_work) = gs
                    .len()
                    .checked_add(1)
                    .and_then(|rows| rows.checked_mul(span))
                    .and_then(|states| {
                        gs.len()
                            .checked_mul(span)
                            .and_then(|transitions| transitions.checked_mul(3))
                            .and_then(|transitions| states.checked_add(transitions))
                    })
                    .and_then(|table| table.checked_add(span))
                else {
                    return (Some(bounds), work, fallback_leaves, Some("work accounting overflow"));
                };
                let Some(next_work) = work.checked_add(table_work) else {
                    return (Some(bounds), work, fallback_leaves, Some("work accounting overflow"));
                };
                if next_work > work_cap {
                    return (Some(bounds), work, fallback_leaves, Some("work cap exhausted"));
                }
                work = next_work;
                let terminal = (0..span)
                    .map(|gain| {
                        if p.base[team] + gain as i32 >= p.lower[team]
                            && p.base[team] + gain as i32 <= p.upper[team]
                        {
                            1.
                        } else {
                            0.
                        }
                    })
                    .collect();
                let mass = TerminalTable::new(gs, team, terminal).mass(0);
                if !mass.is_finite() || mass <= 0. {
                    failed = true;
                    break;
                }
                event_masses.push(mass);
            }
            let read_two = if failed {
                fallback_leaves.push(leaf);
                continue;
            } else {
                let Some(read_two) = read_two_upper(domain_prior, &event_masses) else {
                    fallback_leaves.push(leaf);
                    continue;
                };
                read_two
            };
            let raw = bounds[leaf].upper_bound.min(read_two);
            if !read_two.is_finite() || read_two <= 0. || !raw.is_finite() || raw <= 0. {
                fallback_leaves.push(leaf);
                continue;
            }
            bounds[leaf].read_two_upper = read_two;
            bounds[leaf].upper_bound = (raw * (1. + 1e-9)).min(1.);
        }
        (Some(bounds), work, fallback_leaves, None)
    }
    pub fn draw_work(&self, m: &Model, index: usize, bounds: bool) -> usize {
        let p = self
            .retuned
            .get(&index)
            .map(|q| &q.pattern)
            .unwrap_or(&self.strata[index].pattern);
        let needed = self.cell.rank.min(m.n - 1 - self.cell.rank);
        3 * usize::from(self.retuned.contains_key(&index))
            * (p.joint.games.len() + p.guide.games.len())
            + 8 * (p.omitted.len() + p.joint.games.len())
            + p.fixed.len()
            + 8 * m.n
            + p.guide.games.len()
                * (8 + 36
                    + 54
                    + if bounds {
                        0
                    } else {
                        m.n.saturating_sub(3) * (needed + 2)
                    })
            + 8 * m.fixtures.len()
            + m.n * (m.n.max(2).ilog2() as usize + 1) * m.keys.len().max(1)
    }
    pub fn setup_work(&self) -> usize {
        if let Some(tree) = &self.tree_diagnostics {
            return self.tree_training_work
                + crate::rare_tail::budget::setup_cost(
                    self.setup_nodes,
                    tree["guide_values"].as_u64().unwrap() as usize,
                );
        }
        self.tree_training_work
            + crate::rare_tail::budget::setup_cost(
                self.setup_nodes,
                self.strata
                    .iter()
                    .map(|s| {
                        s.pattern
                            .guide
                            .cdf
                            .values
                            .iter()
                            .chain(&s.pattern.guide.reverse.values)
                            .map(|v| v.len())
                            .sum::<usize>()
                    })
                    .sum(),
            )
    }
    pub fn describe(&self, m: &Model) -> serde_json::Value {
        json!({"team":m.ids[self.cell.team],"rank":self.cell.rank+1,"setup_nodes":self.setup_nodes,"tree":self.tree_diagnostics,"secondary_fallbacks":self.secondary_fallbacks,"secondary_nodes":self.secondary_nodes,"strata":self.strata.iter().map(|s|json!({
            "target_outcomes":s.root,"direction":if s.above{"above"}else{"below"},
            "secondary_team":s.secondary.map(|t|m.ids[t]),
            "exceptions":s.exceptions.iter().map(|&t|m.ids[t]).collect::<Vec<_>>(),
            "forced":s.pattern.fixed.iter().map(|&(i,o)|json!({"game":m.request.games[m.fixtures[i].request_index].id,"home":m.ids[m.fixtures[i].home],"away":m.ids[m.fixtures[i].away],"outcome":o})).collect::<Vec<_>>(),
            "forced_count":s.pattern.fixed.len(),"guided_fixtures":s.pattern.guide.games.len(),"joint_fixtures":s.pattern.joint.games.len(),
            "joint_teams":s.pattern.joint.selected.iter().map(|&t|m.ids[t]).collect::<Vec<_>>(),
            "proposal_normalizer":s.pattern.mass,"allocation_hint":s.pattern.mass*s.pattern.joint.residual_hint.max(1e-80)
        })).collect::<Vec<_>>()})
    }
    pub fn sample(
        &self,
        m: &Model,
        index: usize,
        n: usize,
        seed: i64,
        bounds: bool,
        goals: bool,
    ) -> BranchSample {
        assert!(n >= 2);
        let mut pattern = self
            .retuned
            .get(&index)
            .map(|q| &q.pattern)
            .unwrap_or(&self.strata[index].pattern)
            .clone();
        pattern.bound_guidance = bounds;

        let mut rng = Rng::new(seed);
        let mut out = vec![0; m.fixtures.len()];
        let mut points = vec![0; m.n];
        let mut scores = ScoreContext::new(m);
        let mut goal_context = crate::goal_tilt::Context::new(m);
        let mut r = Result {
            mass: pattern.mass,
            samples: n,
            ..Result::default()
        };
        let (mut sum, mut sum2, mut max, mut batches) = (0., 0., 0_f64, [0.; 2]);
        for draw in 0..n {
            let mut weight = pattern.draw(
                self.cell,
                &mut rng,
                &mut out,
                &mut points,
                true,
                true,
                false,
                0,
                &mut r.operations,
            );
            if weight > 0. {
                if let Some(q) = self.retuned.get(&index) {
                    weight *= q.correction(&out, &mut r.operations);
                }
                r.operations.rank(m);

                let (rank, ratio) = if let Some(g) = self.goals.as_ref().filter(|_| goals) {
                    g.rank(m, &out, &mut rng, &mut goal_context)
                } else {
                    (scores.rank(self.cell.team, &out, &mut rng, None), 1.)
                };
                if rank != self.cell.rank {
                    weight = 0.;
                } else {
                    weight *= ratio;
                }
            }
            if weight > 0. {
                r.hits += 1;
                if r.witness.is_none() {
                    r.witness = Some(out.clone());
                }
                sum += weight;
                sum2 += weight * weight;
                max = max.max(weight);
                batches[usize::from(draw >= n / 2)] += weight;
            }
        }
        r.work = n as u64 * crate::pool::work_per_sample(m);
        r.summarize(sum, sum2, max, batches);
        BranchSample {
            left: pattern.mass * batches[0] / (n / 2) as f64,
            right: pattern.mass * batches[1] / (n - n / 2) as f64,
            result: r,
        }
    }
}
/// Independent disjoint stratum means sum, and their estimated variances sum.
/// ESS and max share use contributions scaled by each stratum's sample count.
pub fn combine(samples: &[BranchSample]) -> Result {
    let probability: f64 = samples.iter().map(|s| s.result.probability).sum();
    let std_err = samples.iter().fold(0_f64, |v, s| v.hypot(s.result.std_err));
    let squared: f64 = if probability > 0. {
        samples
            .iter()
            .filter(|s| s.result.ess > 0.)
            .map(|s| (s.result.probability / probability).powi(2) / s.result.ess)
            .sum()
    } else {
        0.
    };
    let max_share = if probability > 0. {
        samples
            .iter()
            .map(|s| s.result.max_share * (s.result.probability / probability))
            .fold(0., f64::max)
    } else {
        0.
    };
    Result {
        mass: 1.,
        samples: samples.iter().map(|s| s.result.samples).sum(),
        hits: samples.iter().map(|s| s.result.hits).sum(),
        work: samples.iter().map(|s| s.result.work).sum(),
        operations: samples.iter().fold(
            crate::rare_tail::budget::Operations::default(),
            |mut sum, s| {
                sum.add(s.result.operations);
                sum
            },
        ),
        weighted: probability > 0.,
        probability,
        std_err,
        ess: if squared > 0. { 1. / squared } else { 0. },
        max_share,
        batch_gap: if probability > 0. {
            (samples.iter().map(|s| s.left).sum::<f64>()
                - samples.iter().map(|s| s.right).sum::<f64>())
            .abs()
                / probability
        } else {
            0.
        },
        witness: samples.iter().find_map(|s| s.result.witness.clone()),
        ..Result::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn padded_three_team_model(team_count: usize) -> Model {
        let active = [0usize, 1, team_count - 1];
        let games = [
            (active[0], active[1]),
            (active[0], active[2]),
            (active[1], active[2]),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (home, away))| {
            json!({
                "id":i,"home_id":home,"away_id":away,"home_power":1.1,"away_power":0.9
            })
        })
        .collect::<Vec<_>>();
        let request: Request = serde_json::from_value(json!({
            "id": 938,
            "phase": {"sort":"pt,w,bias", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups": (0..team_count).map(|team| {
                let add_sub = match team {
                    t if t == active[0] => 0,
                    t if t == active[1] => 1,
                    t if t == active[2] => 2,
                    _ => 100,
                };
                json!({"team_id":team,"add_sub":add_sub,"bias":team})
            }).collect::<Vec<_>>(),
            "games": games
        })).unwrap();
        Model::new(request).unwrap()
    }

    #[test]
    fn high_team_masks_partition_and_sample_the_padded_small_model() {
        for team_count in [36usize, 64] {
            let m = padded_three_team_model(team_count);
            let active = [0usize, 1, team_count - 1];
            let mut exact_by_rank = vec![0.0; team_count];
            for mut code in 0..27 {
                let mut out = vec![0; m.fixtures.len()];
                let mut mass = 1.0;
                for (i, game) in m.fixtures.iter().enumerate() {
                    let outcome = code % 3;
                    code /= 3;
                    out[i] = outcome as u8;
                    mass *= game.prob[outcome];
                }
                exact_by_rank[canonical_ranks(&m, &out)[0]] += mass;
            }
            let target_ranks = exact_by_rank
                .iter()
                .enumerate()
                .filter_map(|(rank, &mass)| (mass > 0.0).then_some(rank))
                .collect::<Vec<_>>();
            let mut exercised_high_exception = false;
            for rank in target_ranks {
                let plan = BranchStrata::new(
                    &m,
                    Cell {
                        team: active[0],
                        rank,
                    },
                    8801,
                    4,
                )
                .unwrap();
                let desc = plan.describe(&m);
                exercised_high_exception |= desc["strata"].as_array().unwrap().iter().any(|s| {
                    s["exceptions"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|id| id.as_i64() == Some(m.ids[active[2]] as i64))
                });

                let mut covered = 0.0;
                for mut code in 0..27 {
                    let mut out = vec![0; m.fixtures.len()];
                    let mut mass = 1.0;
                    for (i, game) in m.fixtures.iter().enumerate() {
                        let outcome = code % 3;
                        code /= 3;
                        out[i] = outcome as u8;
                        mass *= game.prob[outcome];
                    }
                    if canonical_ranks(&m, &out)[active[0]] != rank {
                        continue;
                    }
                    let mut memberships = 0;
                    for stratum in &plan.strata {
                        let pattern = &stratum.pattern;
                        let games = pattern
                            .joint
                            .games
                            .iter()
                            .chain(&pattern.guide.games)
                            .collect::<Vec<_>>();
                        if pattern.fixed.iter().any(|&(i, want)| out[i] != want)
                            || games
                                .iter()
                                .any(|game| game.prob[out[game.index] as usize] <= 0.)
                        {
                            continue;
                        }
                        let mut totals = pattern.base.clone();
                        for game in games {
                            let outcome = out[game.index] as usize;
                            totals[game.home] += game.hg[outcome];
                            totals[game.away] += game.ag[outcome];
                        }
                        if (0..m.n).all(|team| {
                            totals[team] >= pattern.lower[team]
                                && totals[team] <= pattern.upper[team]
                        }) {
                            memberships += 1;
                        }
                    }
                    assert_eq!(
                        memberships, 1,
                        "{team_count} teams rank {rank}, outcome {out:?}"
                    );
                    covered += mass;
                }
                assert!((covered - exact_by_rank[rank]).abs() < 1e-12);

                let samples = (0..plan.len())
                    .map(|i| plan.sample(&m, i, 4000, 9000 + i as i64 * 31, false, false))
                    .collect::<Vec<_>>();
                let estimate = combine(&samples);
                assert!(
                    (estimate.probability - exact_by_rank[rank]).abs()
                        < 6.0 * estimate.std_err + 1e-5,
                    "{team_count} teams rank {rank}: {} vs {}",
                    estimate.probability,
                    exact_by_rank[rank]
                );
            }
            assert!(
                exercised_high_exception,
                "no high-index exception exercised at {team_count} teams"
            );
        }
    }

    #[test]
    fn team_mask_capacity_rejects_sixty_five_teams() {
        let m = padded_three_team_model(65);
        assert!(LazyJoint::new(&m, Cell { team: 0, rank: 0 }, 1, &[], 1).is_none());
        assert!(BranchStrata::new(&m, Cell { team: 0, rank: 0 }, 1, 1).is_err());
    }

    use crate::{conditioned::canonical_ranks, model::Request};

    #[test]
    fn hierarchy_independent_share_fallback_requires_explicit_pair_rollback() {
        let leaf = json!({"shares":[0.7,0.3]});
        assert_eq!(
            independent_pilot_shares_from_diagnostics(&json!({"rolled_back":false}), &leaf),
            None
        );
        assert_eq!(
            independent_pilot_shares_from_diagnostics(&json!({"rolled_back":true}), &leaf),
            Some(vec![0.7, 0.3])
        );
        assert_eq!(
            independent_pilot_shares_from_diagnostics(
                &json!({"independent_normalized_shares":[0.4,0.6],"rolled_back":false}),
                &leaf
            ),
            Some(vec![0.4, 0.6])
        );
    }

    #[test]
    fn tiny_bound_products_remain_positive() {
        assert_eq!(bound_product(1e-200, 1e-200), f64::MIN_POSITIVE);
        assert_eq!(bound_product(f64::MIN_POSITIVE, 1e-200), f64::MIN_POSITIVE);
        assert_eq!(bound_product(0.5, 0.25), 0.125);
    }
    #[test]
    fn read_two_upper_covers_every_tiny_two_fixture_event_intersection() {
        let p = [0.25, 0.75];
        let support = (0..4)
            .map(|code| {
                let first = code & 1;
                let second = (code >> 1) & 1;
                (code, p[first] * p[second])
            })
            .collect::<Vec<_>>();
        for first_event in 0u8..=3 {
            for second_event in 0u8..=15 {
                let mut pa = 0.0;
                let mut pb = 0.0;
                let mut intersection = 0.0;
                for &(code, mass) in &support {
                    let first = code & 1;
                    let a = first_event & (1 << first) != 0;
                    let b = second_event & (1 << code) != 0;
                    pa += if a { mass } else { 0.0 };
                    pb += if b { mass } else { 0.0 };
                    intersection += if a && b { mass } else { 0.0 };
                }
                if pa > 0.0 && pb > 0.0 {
                    let upper = read_two_upper(1.0, &[pa, pb]).unwrap();
                    assert!(intersection <= upper + 1e-15);
                } else {
                    assert_eq!(read_two_upper(1.0, &[pa, pb]), None);
                }
            }
        }
        assert_eq!(read_two_upper(1.0, &[0.0, 0.5]), None);
        assert_eq!(read_two_upper(1.0, &[f64::NAN]), None);
    }
    #[test]
    fn incomplete_target_enumeration_is_an_error_and_not_a_partial_estimate() {
        let request:Request=serde_json::from_value(json!({"id":1,
            "phase":{"sort":"pt,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..8).map(|t|json!({"team_id":t,"add_sub":if t==0{0}else{100}})).collect::<Vec<_>>(),
            "games":(1..8).map(|a|json!({"id":a,"home_id":0,"away_id":a,"home_power":1.1,"away_power":0.9})).collect::<Vec<_>>() })).unwrap();
        let m = Model::new(request).unwrap();
        assert!(
            matches!(BranchStrata::new(&m,Cell{team:0,rank:7},801,4),Err(e) if e.contains("target path budget"))
        );
    }
    #[test]
    fn combining_tiny_strata_avoids_squaring_unconditional_probabilities() {
        let samples: Vec<_> = [1e-240, 2e-240]
            .iter()
            .map(|&p| BranchSample {
                result: Result {
                    probability: p,
                    std_err: p * 0.1,
                    ess: 100.,
                    max_share: 0.01,
                    samples: 100,
                    hits: 100,
                    ..Result::default()
                },
                left: p,
                right: p,
            })
            .collect();
        let r = combine(&samples);
        assert!((r.probability / 3e-240 - 1.).abs() < 1e-12);
        assert!((r.std_err / 2.23606797749979e-241 - 1.).abs() < 1e-12);
        assert!((r.ess - 180.).abs() < 1e-10);
        assert_eq!(r.batch_gap, 0.);
    }
    #[test]
    fn secondary_paths_preserve_the_complete_union_and_raw_fixture_mass() {
        let request:Request=serde_json::from_value(json!({"id":1,
            "phase":{"sort":"pt,w,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..5).map(|t|json!({"team_id":t,"add_sub":if t==1{2}else{0},"bias":if t==0{100}else{0}})).collect::<Vec<_>>(),
            "games":([(0,4),(1,2),(1,3),(2,3)]).iter().enumerate().map(|(i,(h,a))|json!({"id":i,"home_id":h,"away_id":a,"home_power":1.1,"away_power":0.9})).collect::<Vec<_>>() })).unwrap();
        let m = Model::new(request).unwrap();
        let cell = Cell { team: 0, rank: 1 };
        let mut exact = 0.;
        for mut code in 0..81 {
            let mut out = vec![0; 4];
            let mut mass = 1.;
            for (i, g) in m.fixtures.iter().enumerate() {
                let o = code % 3;
                code /= 3;
                out[i] = o as u8;
                mass *= g.prob[o];
            }
            if canonical_ranks(&m, &out)[0] == cell.rank {
                exact += mass;
            }
        }
        for (quota, parent_quota) in [(0, 0), (10000, 0), (10000, 2000)] {
            let p = BranchStrata::with_secondary_quota(&m, cell, 801, 4, true, quota, parent_quota)
                .unwrap();
            if parent_quota > 0 {
                assert!(p.strata.iter().any(|s| s.secondary.is_some()));
            } else {
                assert!(p.secondary_fallbacks > 0);
            }
            let bounds = p.bounds(&m, true);
            let mut covered = 0.;
            for (s, bound) in p.strata.iter().zip(&bounds) {
                let pattern = &s.pattern;
                let games: Vec<_> = pattern
                    .joint
                    .games
                    .iter()
                    .chain(&pattern.guide.games)
                    .collect();
                let mut seen = std::collections::HashSet::new();
                assert!(pattern.fixed.iter().all(|&(i, _)| seen.insert(i)));
                let mut branch_mass = 0.;
                for mut code in 0..81 {
                    let mut out = vec![0; 4];
                    let mut mass = 1.;
                    for (i, g) in m.fixtures.iter().enumerate() {
                        let o = code % 3;
                        code /= 3;
                        out[i] = o as u8;
                        mass *= g.prob[o];
                    }
                    if pattern.fixed.iter().any(|&(i, o)| out[i] != o)
                        || games.iter().any(|g| g.prob[out[g.index] as usize] <= 0.)
                        || canonical_ranks(&m, &out)[0] != cell.rank
                    {
                        continue;
                    }
                    let mut totals = pattern.base.clone();
                    for g in &games {
                        totals[g.home] += g.hg[out[g.index] as usize];
                        totals[g.away] += g.ag[out[g.index] as usize];
                    }
                    if (0..m.n)
                        .all(|t| totals[t] >= pattern.lower[t] && totals[t] <= pattern.upper[t])
                    {
                        branch_mass += mass;
                    }
                }
                assert!(branch_mass <= bound.upper_bound + 1e-12);
                assert!(bound.normalizer_upper <= bound.domain_prior * (1. + 1e-10));
                assert!(bound.domain_prior <= bound.fixed_prior * (1. + 1e-10));
                covered += branch_mass;
            }
            assert!((covered - exact).abs() < 1e-12);
            for bounds in [false, true] {
                let samples: Vec<_> = (0..p.len())
                    .map(|i| {
                        p.sample(
                            &m,
                            i,
                            10000 + i * 127,
                            derive(808, &format!("secondary-test-{i}")),
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
    fn complete_case_stratification_matches_exhaustive_rank_probabilities() {
        let request:Request=serde_json::from_value(json!({"id":1,"phase":{"sort":"pt,w,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..3).map(|t|json!({"team_id":t,"add_sub":t,"bias":t})).collect::<Vec<_>>(),
            "games":(0..3).flat_map(|h|(h+1..3).map(move|a|json!({"id":h*3+a,"home_id":h,"away_id":a,"home_power":1.1,"away_power":0.9}))).collect::<Vec<_>>() })).unwrap();
        let m = Model::new(request).unwrap();
        for rank in 0..m.n {
            let mut exact = 0.;
            for mut code in 0..27 {
                let mut out = vec![0; m.fixtures.len()];
                let mut mass = 1.;
                for (i, g) in m.fixtures.iter().enumerate() {
                    let o = code % 3;
                    code /= 3;
                    out[i] = o as u8;
                    mass *= g.prob[o];
                }
                if canonical_ranks(&m, &out)[0] == rank {
                    exact += mass;
                }
            }
            let p = BranchStrata::new(&m, Cell { team: 0, rank }, 801, 4).unwrap();
            for bounds in [false, true] {
                let samples: Vec<_> = (0..p.len())
                    .map(|i| {
                        p.sample(
                            &m,
                            i,
                            10000 + i * 127,
                            derive(808, &format!("test-{i}")),
                            bounds,
                            false,
                        )
                    })
                    .collect();
                let r = combine(&samples);
                assert!(
                    (r.probability - exact).abs() < 6. * r.std_err + 1e-4,
                    "{rank}: {} vs {exact}",
                    r.probability
                );
                assert_eq!(
                    r.samples,
                    samples.iter().map(|s| s.result.samples).sum::<usize>()
                );
            }
        }
    }
}
