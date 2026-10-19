//! Bounded, fail-open GD guidance for refined rival paths.
use super::super::{path_priority, Leaf};
use crate::{
    model::{Key, Model},
    sampling::masses,
    search::Cell,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

pub(super) const WORK_LIMIT: usize = 8_000_000;
const MAX_MEAN: f64 = 32.0;
const MAX_POISSON_SUPPORT: usize = 128;
const MAX_KERNEL_CACHE_ENTRIES: usize = 2048;
const MAX_MARGIN_KERNEL_CACHE_ENTRIES: usize = 8192;
const MAX_QUERY_CACHE_ENTRIES: usize = 4096;
const MAX_CACHED_ATOMS: usize = 1_000_000;
const MAX_CACHED_MARGIN_BINS: usize = 1_000_000;
const MAX_WIDTH: usize = 4096;
const MIN_FACTOR: f64 = 1e-12;

#[derive(Default)]
pub(super) struct Report {
    pub(super) enabled: bool,
    pub(super) applied: bool,
    pub(super) reason: Option<&'static str>,
    pub(super) work: usize,
    pub(super) cap: usize,
    pub(super) query_cache_hits: usize,
    pub(super) kernel_cache_hits: usize,
    pub(super) query_rows: Vec<Value>,
}

#[derive(Clone, Copy)]
struct Tails {
    p_gt: f64,
    p_eq: f64,
    p_lt: f64,
}

fn charge(report: &mut Report, units: usize) -> bool {
    let Some(next) = report.work.checked_add(units) else {
        report.work = report.cap;
        return false;
    };
    if next > report.cap {
        report.work = report.cap;
        false
    } else {
        report.work = next;
        true
    }
}

fn outcome(home: i32, away: i32) -> u8 {
    if home < away {
        0
    } else if home == away {
        1
    } else {
        2
    }
}

pub(super) fn kernel(
    model: &Model,
    fixture_index: usize,
    fixed_outcome: Option<u8>,
) -> Option<Vec<([i32; 2], f64)>> {
    let fixture = model.fixtures.get(fixture_index)?;
    if !fixture.home_sampler.mean.is_finite()
        || !fixture.away_sampler.mean.is_finite()
        || fixture.home_sampler.mean < 0.0
        || fixture.away_sampler.mean < 0.0
        || fixture.home_sampler.mean > MAX_MEAN
        || fixture.away_sampler.mean > MAX_MEAN
    {
        return None;
    }
    let home = masses(fixture.home_sampler.mean);
    let away = masses(fixture.away_sampler.mean);
    if home.len() > MAX_POISSON_SUPPORT || away.len() > MAX_POISSON_SUPPORT {
        return None;
    }
    let mut atoms = Vec::new();
    let mut total = 0.0;
    for (h, hp) in home.iter().enumerate() {
        for (a, ap) in away.iter().enumerate() {
            let w = hp * ap;
            if w <= 0.0 || fixed_outcome.is_some_and(|want| outcome(h as i32, a as i32) != want) {
                continue;
            }
            atoms.push(([h as i32, a as i32], w));
            total += w;
        }
    }
    if !total.is_finite() || total <= 0.0 {
        return None;
    }
    Some(atoms.into_iter().map(|(sum, p)| (sum, p / total)).collect())
}

pub(super) fn signed_kernel(
    atoms: &[([i32; 2], f64)],
    coefficient: [f64; 2],
) -> Option<Vec<(i32, f64)>> {
    let home_coefficient = coefficient[0] as i32;
    let away_coefficient = coefficient[1] as i32;
    if coefficient[0] != home_coefficient as f64
        || coefficient[1] != away_coefficient as f64
        || !(-2..=2).contains(&home_coefficient)
        || !(-2..=2).contains(&away_coefficient)
    {
        return None;
    }
    let mut values = BTreeMap::<i32, f64>::new();
    for &([home, away], p) in atoms {
        let value = home_coefficient
            .saturating_mul(home)
            .saturating_add(away_coefficient.saturating_mul(away));
        *values.entry(value).or_default() += p;
    }
    Some(values.into_iter().collect())
}

#[derive(Default)]
struct Caches {
    kernels: HashMap<(usize, Option<u8>), Arc<Vec<([i32; 2], f64)>>>,
    margin_kernels: HashMap<(usize, Option<u8>, i32, i32), Arc<Vec<(i32, f64)>>>,
    queries: HashMap<(usize, usize, Vec<(usize, u8)>), Tails>,
    cached_atoms: usize,
    cached_margin_bins: usize,
}

fn tails(
    offset: i32,
    terms: &[(usize, [f64; 2])],
    fixed: &HashMap<usize, u8>,
    model: &Model,
    caches: &mut Caches,
    report: &mut Report,
) -> Result<Tails, &'static str> {
    let mut distribution = BTreeMap::from([(offset, 1.0)]);
    for &(fixture_index, coefficient) in terms {
        if fixture_index >= model.fixtures.len() {
            return Err("invalid GD fixture term");
        }
        let condition = fixed.get(&fixture_index).copied();
        let key = (fixture_index, condition);
        let raw = if let Some(cached) = caches.kernels.get(&key) {
            if !charge(report, 1) {
                return Err("GD kernel lookup work cap");
            }
            report.kernel_cache_hits += 1;
            Arc::clone(cached)
        } else {
            let atoms = MAX_POISSON_SUPPORT
                .saturating_mul(MAX_POISSON_SUPPORT)
                .saturating_add(2 * 1000);
            if !charge(report, atoms) {
                return Err("GD score atom work cap");
            }
            let support = kernel(model, fixture_index, condition)
                .ok_or("invalid or unsupported fixture score support")?;
            let support = Arc::new(support);
            let atom_count = support.len();
            if caches.kernels.len() < MAX_KERNEL_CACHE_ENTRIES
                && caches.cached_atoms.saturating_add(atom_count) <= MAX_CACHED_ATOMS
                && charge(report, 1)
            {
                caches.cached_atoms += atom_count;
                caches.kernels.insert(key, Arc::clone(&support));
            }
            support
        };
        let coefficient_home = coefficient[0] as i32;
        let coefficient_away = coefficient[1] as i32;
        let margin_key = (fixture_index, condition, coefficient_home, coefficient_away);
        let increments = if let Some(cached) = caches.margin_kernels.get(&margin_key) {
            if !charge(report, 1) {
                return Err("GD margin kernel lookup work cap");
            }
            report.kernel_cache_hits += 1;
            Arc::clone(cached)
        } else {
            if !charge(report, raw.len()) {
                return Err("GD signed kernel aggregation cap");
            }
            let values = signed_kernel(&raw, coefficient).ok_or("unsupported GD coefficient")?;
            let values = Arc::new(values);
            if caches.margin_kernels.len() < MAX_MARGIN_KERNEL_CACHE_ENTRIES
                && caches.cached_margin_bins.saturating_add(values.len()) <= MAX_CACHED_MARGIN_BINS
                && charge(report, 1)
            {
                caches.cached_margin_bins += values.len();
                caches
                    .margin_kernels
                    .insert(margin_key, Arc::clone(&values));
            }
            values
        };
        if !charge(report, increments.len().saturating_mul(2)) {
            return Err("GD width scan work cap");
        }
        let next_width = distribution
            .keys()
            .next()
            .zip(distribution.keys().next_back())
            .zip(
                increments
                    .iter()
                    .map(|(x, _)| *x)
                    .min()
                    .zip(increments.iter().map(|(x, _)| *x).max()),
            )
            .map(|((lo, hi), (inc_lo, inc_hi))| {
                (*hi as i64 + inc_hi as i64 - *lo as i64 - inc_lo as i64 + 1) as usize
            })
            .ok_or("empty GD score support")?;
        if next_width > MAX_WIDTH {
            return Err("GD support width cap");
        }
        let operations = distribution.len().saturating_mul(increments.len());
        if !charge(report, operations) {
            return Err("GD convolution work cap");
        }
        let mut next = BTreeMap::<i32, f64>::new();
        for (&current, &mass) in &distribution {
            for &(increment, probability) in increments.iter() {
                let Some(margin) = current.checked_add(increment) else {
                    return Err("GD integer margin overflow");
                };
                *next.entry(margin).or_default() += mass * probability;
            }
        }
        distribution = next;
    }
    if !charge(report, distribution.len().saturating_mul(2)) {
        return Err("GD tail scan work cap");
    }
    let total = distribution.values().sum::<f64>();
    if !total.is_finite() || total <= 0.0 {
        return Err("invalid GD margin normalizer");
    }
    let mut p_gt = 0.0;
    let mut p_eq = 0.0;
    let mut p_lt = 0.0;
    for (margin, mass) in distribution {
        match margin.cmp(&0) {
            std::cmp::Ordering::Greater => p_gt += mass / total,
            std::cmp::Ordering::Equal => p_eq += mass / total,
            std::cmp::Ordering::Less => p_lt += mass / total,
        }
    }
    Ok(Tails { p_gt, p_eq, p_lt })
}

fn supported_prefix(keys: &[Key]) -> bool {
    keys.first() == Some(&Key::Pt)
        && match keys.get(1) {
            Some(Key::W) => keys.get(2) == Some(&Key::Gd),
            Some(Key::Gd) => true,
            _ => false,
        }
}

fn eligibility_reason(
    above: bool,
    selected: u32,
    mask: u32,
    max_exceptions: usize,
    ambiguous: u32,
    rival: usize,
) -> Option<&'static str> {
    if above {
        Some("not on the required nonstrict-above side")
    } else if selected & (1 << rival) == 0 || mask & (1 << rival) != 0 {
        Some("rival is not selected nonstrict")
    } else if mask.count_ones() as usize != max_exceptions {
        Some("rank constraint is not saturated")
    } else if selected & ambiguous != ambiguous {
        Some("leaf is not fully classified")
    } else {
        None
    }
}

fn exact_pair_points_wins(
    model: &Model,
    target: usize,
    rival: usize,
    terms: &[(usize, [f64; 2])],
    fixed: &HashMap<usize, u8>,
) -> Option<(i32, i32, i32, i32)> {
    let mut campaigns = model.base.clone();
    for &(fixture_index, _) in terms {
        let outcome = *fixed.get(&fixture_index)?;
        let score = match outcome {
            0 => [0, 1],
            1 => [0, 0],
            2 => [1, 0],
            _ => return None,
        };
        model.add(&mut campaigns, fixture_index, score);
    }
    Some((
        campaigns[target].points,
        campaigns[target].wins,
        campaigns[rival].points,
        campaigns[rival].wins,
    ))
}

fn fixed_incident(
    leaf: &Leaf,
    terms: &[(usize, [f64; 2])],
    report: &mut Report,
) -> Option<HashMap<usize, u8>> {
    if !charge(
        report,
        leaf.pattern
            .fixed
            .len()
            .saturating_add(terms.len())
            .saturating_add(4),
    ) {
        return None;
    }
    let mut fixed = HashMap::new();
    for &(fixture, result) in &leaf.pattern.fixed {
        if fixed
            .insert(fixture, result)
            .is_some_and(|old| old != result)
        {
            return None;
        }
    }
    Some(fixed)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply(
    model: &Model,
    cell: Cell,
    leaves: &[Leaf],
    corrected_logs: &mut [f64],
    screen: Option<&path_priority::Screen>,
    cap: usize,
) -> Report {
    let mut report = Report {
        enabled: true,
        cap,
        ..Report::default()
    };
    if !supported_prefix(&model.keys) {
        report.reason = Some("unsupported championship comparator prefix");
        return report;
    }
    if corrected_logs.len() != leaves.len() {
        report.reason = Some("corrected guide alignment unavailable");
        return report;
    }
    let Some(screen) = screen else {
        report.reason = Some("root priority unavailable");
        return report;
    };
    let mut order = (0..leaves.len()).collect::<Vec<_>>();
    let sort_fee = leaves
        .len()
        .saturating_mul(usize::BITS as usize - leaves.len().max(1).leading_zeros() as usize);
    if !charge(&mut report, sort_fee) {
        report.reason = Some("GD query ordering work cap");
        return report;
    }
    order.sort_by(|&a, &b| {
        let la = &leaves[a];
        let lb = &leaves[b];
        let sa = screen.roots.get(la.root_index).map_or(0.0, |r| r.raw_score);
        let sb = screen.roots.get(lb.root_index).map_or(0.0, |r| r.raw_score);
        sb.total_cmp(&sa)
            .then_with(|| la.root_index.cmp(&lb.root_index))
            .then_with(|| la.original_parent_id.cmp(&lb.original_parent_id))
            .then_with(|| la.path_id.cmp(&lb.path_id))
            .then_with(|| a.cmp(&b))
    });
    let mut caches = Caches::default();
    let mut stopped = false;
    for index in order {
        let leaf = &leaves[index];
        let Some(rival) = leaf.rival_team_id else {
            continue;
        };
        let Some(path_id) = leaf.path_id else {
            continue;
        };
        let not_eligible = eligibility_reason(
            leaf.above,
            leaf.selected,
            leaf.mask,
            leaf.max_exceptions,
            leaf.ambiguous,
            rival,
        );
        if not_eligible.is_some() {
            continue;
        }
        if stopped {
            report.query_rows.push(json!({"leaf_index":index,"original_parent_id":leaf.original_parent_id,"root_index":leaf.root_index,"rival_team_id":model.ids[rival],"path_id":leaf.path_id,"reason":"GD work cap reached; guide unchanged"}));
            continue;
        }
        let parent_id = leaf.original_parent_id;
        if !charge(
            &mut report,
            model
                .fixtures
                .len()
                .saturating_add(model.request.games.len()),
        ) {
            stopped = true;
            report.query_rows.push(json!({"leaf_index":index,"original_parent_id":parent_id,"rival_team_id":model.ids[rival],"path_id":path_id,"reason":"GD support scan work cap"}));
            continue;
        }
        let Some((offset, terms)) = model.goal_difference_support(rival, cell.team) else {
            report
                .query_rows
                .push(json!({"leaf_index":index,"reason":"GD support unavailable"}));
            continue;
        };
        if !charge(&mut report, terms.len().saturating_add(8)) {
            stopped = true;
            report.query_rows.push(json!({"leaf_index":index,"original_parent_id":parent_id,"rival_team_id":model.ids[rival],"path_id":path_id,"reason":"GD term scan work cap"}));
            continue;
        }
        let Some(fixed) = fixed_incident(leaf, &terms, &mut report) else {
            report.query_rows.push(json!({"leaf_index":index,"original_parent_id":parent_id,"rival_team_id":model.ids[rival],"path_id":path_id,"reason":"fixed incident WDL unavailable or scan cap"}));
            stopped |= report.work >= report.cap;
            continue;
        };
        let Some((target_points, target_wins, rival_points, rival_wins)) = ({
            if !charge(
                &mut report,
                model.n.saturating_add(terms.len().saturating_mul(2)),
            ) {
                stopped = true;
                report.query_rows.push(json!({"leaf_index":index,"original_parent_id":parent_id,"rival_team_id":model.ids[rival],"path_id":path_id,"reason":"earlier-key campaign work cap"}));
                continue;
            }
            exact_pair_points_wins(model, cell.team, rival, &terms, &fixed)
        }) else {
            report.query_rows.push(json!({"leaf_index":index,"original_parent_id":parent_id,"rival_team_id":model.ids[rival],"path_id":path_id,"reason":"earlier comparator totals unavailable"}));
            continue;
        };
        if target_points != rival_points
            || (model.keys.get(1) == Some(&Key::W) && target_wins != rival_wins)
        {
            report.query_rows.push(json!({"leaf_index":index,"original_parent_id":parent_id,"root_index":leaf.root_index,"root":leaf.root,"rival_team_id":model.ids[rival],"path_id":path_id,"reason":"earlier comparator key decisive","earlier_keys":"decisive","target_points":target_points,"target_wins":target_wins,"rival_points":rival_points,"rival_wins":rival_wins,"p_gt":null,"p_eq":null,"p_lt":null,"guide_factor":1.0}));
            continue;
        }
        let query_key = (
            cell.team,
            rival,
            terms
                .iter()
                .filter_map(|(fixture, _)| fixed.get(fixture).map(|outcome| (*fixture, *outcome)))
                .collect::<Vec<_>>(),
        );
        let mut query_cache_hit = false;
        let tails = if let Some(cached) = caches.queries.get(&query_key).copied() {
            if !charge(&mut report, query_key.2.len().saturating_add(1)) {
                stopped = true;
                report.query_rows.push(json!({"leaf_index":index,"original_parent_id":parent_id,"rival_team_id":model.ids[rival],"path_id":path_id,"reason":"GD query cache work cap"}));
                continue;
            }
            report.query_cache_hits += 1;
            query_cache_hit = true;
            cached
        } else {
            if !offset.is_finite()
                || offset.fract() != 0.0
                || offset < i32::MIN as f64
                || offset > i32::MAX as f64
            {
                report.query_rows.push(json!({"leaf_index":index,"original_parent_id":parent_id,"rival_team_id":model.ids[rival],"path_id":path_id,"reason":"GD offset outside integer support"}));
                continue;
            }
            match tails(
                offset as i32,
                &terms,
                &fixed,
                model,
                &mut caches,
                &mut report,
            ) {
                Ok(tails) => {
                    if caches.queries.len() < MAX_QUERY_CACHE_ENTRIES {
                        if charge(&mut report, 1) {
                            caches.queries.insert(query_key.clone(), tails);
                        }
                    }
                    tails
                }
                Err(reason) => {
                    report.query_rows.push(json!({"leaf_index":index,"original_parent_id":parent_id,"root_index":leaf.root_index,"rival_team_id":model.ids[rival],"path_id":path_id,"reason":reason}));
                    stopped |= report.work >= report.cap;
                    continue;
                }
            }
        };
        let raw_factor = tails.p_gt + tails.p_eq;
        if !raw_factor.is_finite() || raw_factor < 0.0 {
            report.query_rows.push(json!({"leaf_index":index,"original_parent_id":parent_id,"rival_team_id":model.ids[rival],"path_id":path_id,"reason":"invalid GD tail"}));
            continue;
        }
        let factor = raw_factor.clamp(MIN_FACTOR, 1.0);
        let clamped = factor != raw_factor;
        if corrected_logs[index].is_finite() {
            corrected_logs[index] += factor.ln();
            report.applied = true;
        } else {
            report.query_rows.push(json!({"leaf_index":index,"original_parent_id":parent_id,"rival_team_id":model.ids[rival],"path_id":path_id,"reason":"invalid corrected guide"}));
            continue;
        }
        report.query_rows.push(json!({"leaf_index":index,"original_parent_id":parent_id,"root_index":leaf.root_index,"root":leaf.root,"rival_team_id":model.ids[rival],"path_id":path_id,"incident_wdl":query_key.2,"earlier_keys":"tied","p_gt":tails.p_gt,"p_eq":tails.p_eq,"p_lt":tails.p_lt,"guide_factor":factor,"factor_clamped":clamped,"cache_hit":query_cache_hit,"reason":if clamped {Some("positive guide floor applied")} else {None}}));
    }
    if stopped && report.reason.is_none() {
        report.reason = Some("GD work cap reached; remaining guides unchanged");
    } else if !report.applied && report.reason.is_none() {
        report.reason = Some("no eligible tied saturated rival paths");
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Request;
    use serde_json::json;

    fn tiny_model(shared_only: bool) -> Model {
        let games = if shared_only {
            vec![(1, 0)]
        } else {
            vec![(1, 0), (1, 2)]
        };
        let request: Request = serde_json::from_value(json!({
            "id": 91,
            "phase":{"sort":"pt,w,gd,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..3).map(|team|json!({"team_id":team,"bias":0})).collect::<Vec<_>>(),
            "games":games.iter().enumerate().map(|(i,(home,away))|json!({"id":i,"home_id":home,"away_id":away,"home_power":1.2,"away_power":0.9})).collect::<Vec<_>>()
        })).unwrap();
        Model::new(request).unwrap()
    }

    #[test]
    fn shared_fixture_uses_double_goal_difference_coefficient_and_optimistic_equality() {
        let model = tiny_model(true);
        let (offset, terms) = model.goal_difference_support(1, 0).unwrap();
        assert_eq!(offset, 0.0);
        assert_eq!(terms, vec![(0, [2.0, -2.0])]);
        let fixed = HashMap::from([(0, 1u8)]);
        let mut report = Report {
            enabled: true,
            cap: WORK_LIMIT,
            ..Report::default()
        };
        let tails = tails(
            offset as i32,
            &terms,
            &fixed,
            &model,
            &mut Caches::default(),
            &mut report,
        )
        .unwrap();
        assert_eq!(tails.p_gt, 0.0);
        assert_eq!(tails.p_eq, 1.0);
        assert_eq!(tails.p_lt, 0.0);
        assert_eq!(tails.p_gt + tails.p_eq, 1.0);
    }

    #[test]
    fn two_fixture_conditional_margin_tails_match_direct_atom_enumeration() {
        let model = tiny_model(false);
        let (offset, terms) = model.goal_difference_support(1, 0).unwrap();
        assert_eq!(terms[0].1, [2.0, -2.0]);
        assert_eq!(terms[1].1, [1.0, -1.0]);
        // The rival loses at home in fixture 0 and wins at home in fixture 1.
        // Both clubs finish on 3 points / 1 win, while GD has mixed signed
        // contributions and therefore exercises all three margin tails.
        let fixed = HashMap::from([(0, 0u8), (1, 2u8)]);
        let mut report = Report {
            enabled: true,
            cap: WORK_LIMIT,
            ..Report::default()
        };
        let actual = tails(
            offset as i32,
            &terms,
            &fixed,
            &model,
            &mut Caches::default(),
            &mut report,
        )
        .unwrap();

        let mut atoms = Vec::new();
        for fixture_index in 0..2 {
            let fixture = &model.fixtures[fixture_index];
            let hp = masses(fixture.home_sampler.mean);
            let ap = masses(fixture.away_sampler.mean);
            let mut values = Vec::new();
            let mut total = 0.0;
            for (home, ph) in hp.iter().enumerate() {
                for (away, pa) in ap.iter().enumerate() {
                    if outcome(home as i32, away as i32) == fixed[&fixture_index] {
                        let prob = ph * pa;
                        total += prob;
                        values.push(([home as i32, away as i32], prob));
                    }
                }
            }
            for value in &mut values {
                value.1 /= total;
            }
            atoms.push(values);
        }
        let mut oracle = [0.0; 3];
        for &([h0, a0], p0) in &atoms[0] {
            for &([h1, a1], p1) in &atoms[1] {
                let margin = offset as i32 + 2 * (h0 - a0) + (h1 - a1);
                oracle[if margin > 0 {
                    0
                } else if margin == 0 {
                    1
                } else {
                    2
                }] += p0 * p1;
            }
        }
        assert!((actual.p_gt - oracle[0]).abs() < 1e-12);
        assert!((actual.p_eq - oracle[1]).abs() < 1e-12);
        assert!((actual.p_lt - oracle[2]).abs() < 1e-12);
        assert!(actual.p_gt > 0.0);
        assert!(actual.p_eq > 0.0);
        assert!(actual.p_lt > 0.0);
        assert!((actual.p_gt + actual.p_eq + actual.p_lt - 1.0).abs() < 1e-12);
    }

    #[test]
    fn tiny_positive_tail_matches_direct_atoms_without_cdf_subtraction() {
        let model = tiny_model(false);
        let terms = [(0, [2.0, -2.0]), (1, [1.0, -1.0])];
        let max_increments = terms
            .iter()
            .map(|(fixture_index, coefficient)| {
                let fixture = &model.fixtures[*fixture_index];
                let home_max = masses(fixture.home_sampler.mean).len() as i32 - 1;
                let away_max = masses(fixture.away_sampler.mean).len() as i32 - 1;
                (coefficient[0] as i32 * home_max).max(coefficient[0] as i32 * 0)
                    + (coefficient[1] as i32 * away_max).max(coefficient[1] as i32 * 0)
            })
            .sum::<i32>();
        // Only the simultaneous maximum home-score atoms produce a positive
        // margin, making this a deliberately tiny direct product tail.
        let offset = 1 - max_increments;
        let mut report = Report {
            enabled: true,
            cap: WORK_LIMIT,
            ..Report::default()
        };
        let actual = tails(
            offset,
            &terms,
            &HashMap::new(),
            &model,
            &mut Caches::default(),
            &mut report,
        )
        .unwrap();

        let mut oracle = [0.0; 3];
        let mut total = 0.0;
        let atoms = terms
            .iter()
            .map(|(fixture_index, _)| {
                let fixture = &model.fixtures[*fixture_index];
                (
                    masses(fixture.home_sampler.mean),
                    masses(fixture.away_sampler.mean),
                )
            })
            .collect::<Vec<_>>();
        for (h0, ph0) in atoms[0].0.iter().enumerate() {
            for (a0, pa0) in atoms[0].1.iter().enumerate() {
                for (h1, ph1) in atoms[1].0.iter().enumerate() {
                    for (a1, pa1) in atoms[1].1.iter().enumerate() {
                        let probability = ph0 * pa0 * ph1 * pa1;
                        let margin = offset + 2 * (h0 as i32 - a0 as i32) + (h1 as i32 - a1 as i32);
                        oracle[if margin > 0 {
                            0
                        } else if margin == 0 {
                            1
                        } else {
                            2
                        }] += probability;
                        total += probability;
                    }
                }
            }
        }
        for probability in &mut oracle {
            *probability /= total;
        }
        assert!(oracle[0] > 0.0 && oracle[0] < 1e-20);
        assert!((actual.p_gt - oracle[0]).abs() / oracle[0] < 1e-9);
        assert!((actual.p_eq - oracle[1]).abs() < 1e-12);
        assert!((actual.p_lt - oracle[2]).abs() < 1e-12);
    }

    #[test]
    fn exhausted_cap_fails_open_before_constructing_score_support() {
        let model = tiny_model(true);
        let mut caches = Caches::default();
        let mut report = Report {
            enabled: true,
            cap: 0,
            ..Report::default()
        };
        assert!(matches!(
            tails(
                0,
                &[(0, [2.0, -2.0])],
                &HashMap::new(),
                &model,
                &mut caches,
                &mut report,
            ),
            Err("GD score atom work cap")
        ));
        assert_eq!(report.work, report.cap);
        assert!(caches.kernels.is_empty());
    }

    #[test]
    fn comparator_prefix_and_saturation_guards_are_narrow() {
        assert!(supported_prefix(&[Key::Pt, Key::W, Key::Gd, Key::Bias]));
        assert!(supported_prefix(&[Key::Pt, Key::Gd, Key::Bias]));
        assert!(!supported_prefix(&[Key::Pt, Key::Bias, Key::Gd]));
        assert!(!supported_prefix(&[Key::W, Key::Gd]));
        assert_eq!(eligibility_reason(false, 0b11, 0b01, 1, 0b10, 1), None);
        assert_eq!(
            eligibility_reason(true, 0b11, 0b01, 1, 0b10, 1),
            Some("not on the required nonstrict-above side")
        );
        assert_eq!(
            eligibility_reason(false, 0b11, 0, 1, 0b10, 1),
            Some("rank constraint is not saturated")
        );
        assert_eq!(
            eligibility_reason(false, 0b01, 0b01, 1, 0b10, 1),
            Some("rival is not selected nonstrict")
        );
    }
}
