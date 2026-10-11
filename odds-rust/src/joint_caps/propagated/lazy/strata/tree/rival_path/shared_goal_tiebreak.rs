//! Bounded per-rival goal guidance using the target's conditional GD law.
use super::super::{path_priority, Leaf};
use crate::{
    joint_caps::propagated::lazy::strata::Pattern,
    lookahead::RankGame,
    model::{Key, Model},
    search::Cell,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

pub(super) const WORK_LIMIT: usize = 64_000_000;
const MAX_WORK_LIMIT: usize = 512_000_000;
const MAX_STATES: usize = 50_000;
const MAX_WIDTH: i32 = 4096;
const MAX_QUERY_CACHE: usize = 1024;
const MAX_TARGET_CACHE: usize = 256;
const MAX_KERNEL_CACHE: usize = 16_384;
const MAX_CACHED_KERNEL_ATOMS: usize = 1_000_000;
const MAX_CACHED_PMF_BINS: usize = 1_000_000;
const MAX_CACHED_QUERY_BINS: usize = 1_000_000;
const MIN_FACTOR: f64 = 1e-12;

#[derive(Default)]
pub(super) struct Report {
    pub(super) enabled: bool,
    pub(super) applied: bool,
    pub(super) reason: Option<&'static str>,
    pub(super) work: usize,
    pub(super) cap: usize,
    pub(super) query_cache_hits: usize,
    pub(super) target_cache_hits: usize,
    pub(super) kernel_cache_hits: usize,
    pub(super) eligible_unrefined: usize,
    pub(super) applied_unrefined: usize,
    pub(super) root_coverage: Vec<Value>,
    pub(super) query_rows: Vec<Value>,
}

type Pmf = BTreeMap<i32, f64>;
type Joint = BTreeMap<(i32, i32), f64>;

#[derive(Hash, PartialEq, Eq, Clone)]
struct KernelKey(usize, u8);

#[derive(Hash, PartialEq, Eq, Clone)]
struct QueryKey {
    root: Vec<u8>,
    target: usize,
    rival: usize,
    target_packed: i32,
    lower: i32,
    upper: i32,
    stride: i32,
    fixed: Vec<(usize, u8)>,
    domains: Vec<(usize, [u64; 3])>,
}

#[derive(Clone)]
struct RivalCalc {
    denominator: f64,
    strict_mass: f64,
    below_mass: f64,
    tie_by_z: Pmf,
}

#[derive(Default)]
struct Caches {
    kernels: HashMap<KernelKey, Arc<Vec<([i32; 2], f64)>>>,
    target_pmfs: HashMap<Vec<u8>, Arc<Pmf>>,
    target_splits: HashMap<(Vec<u8>, usize), Arc<(Pmf, Pmf)>>,
    queries: HashMap<QueryKey, Arc<RivalCalc>>,
    cached_kernel_atoms: usize,
    cached_pmf_bins: usize,
    cached_query_bins: usize,
    cached_query_key_items: usize,
}

fn charge(report: &mut Report, work: usize) -> bool {
    let Some(next) = report.work.checked_add(work) else {
        report.work = report.cap;
        return false;
    };
    if next > report.cap {
        report.work = report.cap;
        return false;
    }
    report.work = next;
    true
}

fn kernel(
    model: &Model,
    fixture: usize,
    wdl: u8,
    caches: &mut Caches,
    report: &mut Report,
) -> Result<Arc<Vec<([i32; 2], f64)>>, &'static str> {
    let key = KernelKey(fixture, wdl);
    if let Some(found) = caches.kernels.get(&key) {
        if !charge(report, found.len().saturating_add(1)) {
            return Err("kernel cache lookup cap");
        }
        report.kernel_cache_hits += 1;
        return Ok(Arc::clone(found));
    }
    if !charge(report, 128usize.saturating_mul(128).saturating_add(2000)) {
        return Err("conditional score kernel cap");
    }
    let atoms = super::goal_tiebreak::kernel(model, fixture, Some(wdl))
        .ok_or("unsupported conditional score kernel")?;
    if caches.kernels.len() < MAX_KERNEL_CACHE
        && caches.cached_kernel_atoms.saturating_add(atoms.len()) <= MAX_CACHED_KERNEL_ATOMS
    {
        if charge(report, atoms.len().saturating_add(1)) {
            caches.cached_kernel_atoms += atoms.len();
            caches.kernels.insert(key, Arc::new(atoms.clone()));
        }
    }
    Ok(Arc::new(atoms))
}

fn margin_pmf(
    model: &Model,
    fixture: usize,
    team: usize,
    wdl: u8,
    caches: &mut Caches,
    report: &mut Report,
) -> Result<Pmf, &'static str> {
    let game = model.fixtures.get(fixture).ok_or("invalid fixture index")?;
    let coefficient = if game.home == team {
        [1.0, -1.0]
    } else if game.away == team {
        [-1.0, 1.0]
    } else {
        return Err("fixture is not incident to team");
    };
    let atoms = kernel(model, fixture, wdl, caches, report)?;
    if !charge(report, atoms.len()) {
        return Err("margin aggregation cap");
    }
    let increments = super::goal_tiebreak::signed_kernel(&atoms, coefficient)
        .ok_or("unsupported GD coefficient")?;
    Ok(increments
        .into_iter()
        .filter(|(_, mass)| *mass > 0.0)
        .collect())
}

fn convolve(left: &Pmf, right: &Pmf, report: &mut Report) -> Result<Pmf, &'static str> {
    if left.is_empty() || right.is_empty() {
        return Err("empty goal margin support");
    }
    let work = left
        .len()
        .checked_mul(right.len())
        .ok_or("convolution overflow")?;
    if !charge(report, work) {
        return Err("goal margin convolution cap");
    }
    let width = left
        .keys()
        .next()
        .zip(left.keys().next_back())
        .zip(right.keys().next().zip(right.keys().next_back()))
        .map(|((lmin, lmax), (rmin, rmax))| {
            i64::from(*lmax) + i64::from(*rmax) - i64::from(*lmin) - i64::from(*rmin) + 1
        })
        .ok_or("empty goal margin range")?;
    if width > i64::from(MAX_WIDTH) {
        return Err("goal margin width cap");
    }
    let mut result = Pmf::new();
    for (&a, &pa) in left {
        for (&b, &pb) in right {
            let value = a.checked_add(b).ok_or("goal margin integer overflow")?;
            let mass = pa * pb;
            if !mass.is_finite() || mass < 0.0 {
                return Err("invalid goal margin convolution mass");
            }
            if mass > 0.0 {
                *result.entry(value).or_default() += mass;
            }
        }
    }
    if result.len() > MAX_STATES {
        return Err("goal margin state cap");
    }
    if result
        .values()
        .any(|mass| !mass.is_finite() || *mass <= 0.0)
    {
        return Err("invalid goal margin aggregation mass");
    }
    Ok(result)
}

fn shifted(pmf: &Pmf, offset: i32) -> Result<Pmf, &'static str> {
    let mut shifted = Pmf::new();
    for (&value, &mass) in pmf {
        if !mass.is_finite() || mass < 0.0 {
            return Err("invalid shifted goal margin mass");
        }
        if mass > 0.0 {
            let value = value
                .checked_add(offset)
                .ok_or("goal margin offset overflow")?;
            shifted.insert(value, mass);
        }
    }
    Ok(shifted)
}

fn packed_stride(model: &Model) -> Option<i32> {
    let has_wins = model.keys.get(1) == Some(&Key::W);
    if !has_wins {
        return Some(1);
    }
    let mut degree = vec![0i32; model.n];
    for fixture in &model.fixtures {
        *degree.get_mut(fixture.home)? = degree[fixture.home].checked_add(1)?;
        *degree.get_mut(fixture.away)? = degree[fixture.away].checked_add(1)?;
    }
    model
        .base
        .iter()
        .zip(degree)
        .map(|(base, remaining)| base.wins.checked_add(remaining))
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .max()?
        .checked_add(1)
        .filter(|stride| *stride > 0)
}

fn base_packed(model: &Model, team: usize, stride: i32) -> Option<i32> {
    let campaign = model.base.get(team)?;
    i64::from(campaign.points)
        .checked_mul(i64::from(stride))?
        .checked_add(if model.keys.get(1) == Some(&Key::W) {
            i64::from(campaign.wins)
        } else {
            0
        })?
        .try_into()
        .ok()
}

fn packed_gains(model: &Model, stride: i32, home: bool, outcome: u8) -> Option<i32> {
    let has_wins = model.keys.get(1) == Some(&Key::W);
    let rules = &model.request.phase.championship;
    if (rules.point_win, rules.point_draw, rules.point_loss) != (3, 1, 0)
        || model.request.phase.bonus_points != 0
    {
        return None;
    }
    let points = match outcome {
        0 if home => rules.point_loss,
        1 => rules.point_draw,
        2 if home => rules.point_win,
        0 if !home => rules.point_win,
        2 if !home => rules.point_loss,
        _ => return None,
    };
    let win = match (home, outcome) {
        (true, 2) | (false, 0) => 1,
        _ => 0,
    };
    points
        .checked_mul(stride)?
        .checked_add(if has_wins { win } else { 0 })
}

fn fixed_map(
    model: &Model,
    leaf: &Leaf,
    target: usize,
) -> Result<HashMap<usize, u8>, &'static str> {
    let target_fixtures = model
        .fixtures
        .iter()
        .enumerate()
        .filter_map(|(index, fixture)| {
            (fixture.home == target || fixture.away == target).then_some(index)
        })
        .collect::<Vec<_>>();
    if target_fixtures.len() != leaf.root.len() {
        return Err("target root outcome mapping unavailable");
    }
    let mut fixed = HashMap::new();
    for (fixture, &wdl) in target_fixtures.into_iter().zip(&leaf.root) {
        if wdl > 2 {
            return Err("invalid root WDL");
        }
        fixed.insert(fixture, wdl);
    }
    for &(fixture, wdl) in &leaf.pattern.fixed {
        if model.fixtures.get(fixture).is_none() {
            return Err("fixed WDL references an unknown fixture");
        }
        if wdl > 2 || fixed.insert(fixture, wdl).is_some_and(|old| old != wdl) {
            return Err("inconsistent fixed WDL mapping");
        }
    }
    Ok(fixed)
}

fn rank_games(model: &Model, pattern: &Pattern) -> Result<HashMap<usize, RankGame>, &'static str> {
    let mut games = HashMap::new();
    for game in pattern
        .guide
        .games
        .iter()
        .chain(pattern.joint.games.iter())
        .chain(pattern.omitted.iter())
    {
        let fixture = model
            .fixtures
            .get(game.index)
            .ok_or("rank domain references an unknown fixture")?;
        if fixture.home != game.home || fixture.away != game.away {
            return Err("rank domain fixture endpoints disagree with model");
        }
        let probability_sum = game.prob.iter().sum::<f64>();
        if game.prob.iter().any(|p| !p.is_finite() || *p < 0.0)
            || !probability_sum.is_finite()
            || (probability_sum - 1.0).abs() > 1e-9
        {
            return Err("invalid conditional WDL domain");
        }
        if let Some(previous) = games.get(&game.index) {
            let previous: &RankGame = previous;
            if previous.home != game.home
                || previous.away != game.away
                || previous.hg != game.hg
                || previous.ag != game.ag
                || previous.prob.map(f64::to_bits) != game.prob.map(f64::to_bits)
            {
                return Err("inconsistent duplicate fixture domain");
            }
        } else {
            games.insert(game.index, *game);
        }
    }
    Ok(games)
}

fn target_split_pmfs(
    model: &Model,
    target: usize,
    root: &[u8],
    rival: usize,
    caches: &mut Caches,
    report: &mut Report,
) -> Result<(Pmf, Pmf), &'static str> {
    if !charge(report, model.fixtures.len()) {
        return Err("target split fixture scan cap");
    }
    let target_games = model
        .fixtures
        .iter()
        .enumerate()
        .filter_map(|(index, fixture)| {
            (fixture.home == target || fixture.away == target).then_some(index)
        })
        .collect::<Vec<_>>();
    if target_games.len() != root.len() {
        return Err("target root does not cover every incident WDL");
    }
    let mut shared = Pmf::from([(0, 1.0)]);
    let mut private = Pmf::from([(0, 1.0)]);
    for (fixture, &wdl) in target_games.into_iter().zip(root) {
        let game = &model.fixtures[fixture];
        let current = margin_pmf(model, fixture, target, wdl, caches, report)?;
        if game.home == rival || game.away == rival {
            shared = convolve(&shared, &current, report)?;
        } else {
            private = convolve(&private, &current, report)?;
        }
    }
    Ok((shared, private))
}

fn canonical_target_pmf(
    model: &Model,
    target: usize,
    root: &[u8],
    caches: &mut Caches,
    report: &mut Report,
) -> Result<Arc<Pmf>, &'static str> {
    if !charge(report, model.fixtures.len()) {
        return Err("target canonical fixture scan cap");
    }
    if let Some(cached) = caches.target_pmfs.get(root) {
        report.target_cache_hits += 1;
        if !charge(report, 1) {
            return Err("target PMF lookup cap");
        }
        return Ok(Arc::clone(cached));
    }
    let target_games = model
        .fixtures
        .iter()
        .enumerate()
        .filter_map(|(index, fixture)| {
            (fixture.home == target || fixture.away == target).then_some(index)
        })
        .collect::<Vec<_>>();
    if target_games.len() != root.len() {
        return Err("target root does not cover every incident WDL");
    }
    if !charge(report, target_games.len().saturating_mul(2)) {
        return Err("target root scan cap");
    }
    let mut pmf = Pmf::from([(0, 1.0)]);
    for (fixture, &wdl) in target_games.into_iter().zip(root) {
        let kernel = margin_pmf(model, fixture, target, wdl, caches, report)?;
        pmf = convolve(&pmf, &kernel, report)?;
    }
    let base_gd = model.base[target]
        .gf
        .checked_sub(model.base[target].ga)
        .ok_or("target GD overflow")?;
    pmf = shifted(&pmf, base_gd)?;
    if !charge(report, pmf.len()) {
        return Err("target GD normalizer scan cap");
    }
    let total = pmf.values().sum::<f64>();
    if !total.is_finite() || total <= 0.0 {
        return Err("invalid target GD normalizer");
    }
    for mass in pmf.values_mut() {
        *mass /= total;
    }
    if pmf.is_empty() || pmf.len() > MAX_CACHED_PMF_BINS {
        return Err("target GD cache-bin cap");
    }
    if caches.target_pmfs.len() < MAX_TARGET_CACHE
        && caches.cached_pmf_bins.saturating_add(pmf.len()) <= MAX_CACHED_PMF_BINS
    {
        if !charge(report, pmf.len().saturating_add(1)) {
            return Err("target PMF cache work cap");
        }
        caches.cached_pmf_bins += pmf.len();
        caches
            .target_pmfs
            .insert(root.to_vec(), Arc::new(pmf.clone()));
    }
    Ok(Arc::new(pmf))
}

fn fixed_packed_for_rival(
    model: &Model,
    rival: usize,
    fixed: &HashMap<usize, u8>,
    stride: i32,
) -> Option<i32> {
    let mut packed = base_packed(model, rival, stride)?;
    for (&fixture_index, &wdl) in fixed {
        let fixture = model.fixtures.get(fixture_index)?;
        if fixture.home == rival {
            packed = packed.checked_add(packed_gains(model, stride, true, wdl)?)?;
        } else if fixture.away == rival {
            packed = packed.checked_add(packed_gains(model, stride, false, wdl)?)?;
        }
    }
    Some(packed)
}

fn outcome_transition(
    model: &Model,
    game: Option<&RankGame>,
    fixture_index: usize,
    team: usize,
    fixed_wdl: Option<u8>,
    caches: &mut Caches,
    report: &mut Report,
) -> Result<Vec<((i32, i32), f64)>, &'static str> {
    let fixture = model
        .fixtures
        .get(fixture_index)
        .ok_or("invalid variable fixture")?;
    if fixture.home != team && fixture.away != team {
        return Err("nonincident rival fixture in transition");
    }
    let mut transitions = BTreeMap::<(i32, i32), f64>::new();
    for wdl in 0..3u8 {
        if fixed_wdl.is_some_and(|fixed| fixed != wdl) {
            continue;
        }
        let domain_mass = if fixed_wdl.is_some() {
            1.0
        } else {
            game.ok_or("variable fixture lacks rank domain")?.prob[wdl as usize]
        };
        if domain_mass <= 0.0 {
            continue;
        }
        let packed = if fixed_wdl.is_some() {
            0
        } else {
            let game = game.ok_or("variable fixture lacks rank domain")?;
            if fixture.home == team {
                game.hg[wdl as usize]
            } else {
                game.ag[wdl as usize]
            }
        };
        let atoms = kernel(model, fixture_index, wdl, caches, report)?;
        let coefficient = if fixture.home == team {
            [1.0, -1.0]
        } else {
            [-1.0, 1.0]
        };
        if !charge(report, atoms.len()) {
            return Err("rival transition aggregation cap");
        }
        for &(scores, probability) in atoms.iter() {
            let margin = coefficient[0] as i32 * scores[0] + coefficient[1] as i32 * scores[1];
            *transitions.entry((packed, margin)).or_default() += domain_mass * probability;
        }
    }
    if transitions.is_empty() {
        return Err("empty rival WDL transition");
    }
    Ok(transitions.into_iter().collect())
}

fn private_rival_dp(
    model: &Model,
    leaf: &Leaf,
    target: usize,
    rival: usize,
    target_packed: i32,
    fixed: &HashMap<usize, u8>,
    games: &HashMap<usize, RankGame>,
    stride: i32,
    caches: &mut Caches,
    report: &mut Report,
) -> Result<RivalCalc, &'static str> {
    if !charge(
        report,
        model
            .fixtures
            .len()
            .saturating_add(fixed.len().saturating_mul(2))
            .saturating_add(games.len()),
    ) {
        return Err("rival fixture-map scan cap");
    }
    let lower = *leaf
        .pattern
        .lower
        .get(rival)
        .ok_or("missing rival lower bound")?;
    let upper = *leaf
        .pattern
        .upper
        .get(rival)
        .ok_or("missing rival upper bound")?;
    let mut packed_dp = BTreeMap::<i32, f64>::from([(0, 1.0)]);
    let mut joint_dp = Joint::from([((0, 0), 1.0)]);
    let mut stages = Vec::new();
    for (fixture_index, fixture) in model.fixtures.iter().enumerate() {
        if fixture.home != rival && fixture.away != rival {
            continue;
        }
        if fixture.home == target || fixture.away == target {
            continue;
        }
        let game = games.get(&fixture_index);
        let fixed_wdl = fixed.get(&fixture_index).copied();
        if fixed_wdl.is_none() && game.is_none() {
            return Err("unfixed rival fixture lacks rank domain");
        }
        let transitions =
            outcome_transition(model, game, fixture_index, rival, fixed_wdl, caches, report)?;
        let mut scalar_transitions = BTreeMap::<i32, f64>::new();
        if !charge(report, transitions.len()) {
            return Err("packed transition scan cap");
        }
        for &((packed_gain, _), probability) in &transitions {
            *scalar_transitions.entry(packed_gain).or_default() += probability;
        }
        let min_gain = scalar_transitions
            .keys()
            .next()
            .copied()
            .ok_or("empty packed transition")?;
        let max_gain = scalar_transitions
            .keys()
            .next_back()
            .copied()
            .ok_or("empty packed transition")?;
        stages.push((
            fixture_index,
            transitions,
            scalar_transitions,
            min_gain,
            max_gain,
        ));
    }
    let mut suffix_min = vec![0i64; stages.len() + 1];
    let mut suffix_max = vec![0i64; stages.len() + 1];
    for index in (0..stages.len()).rev() {
        suffix_min[index] = suffix_min[index + 1]
            .checked_add(i64::from(stages[index].3))
            .ok_or("suffix minimum overflow")?;
        suffix_max[index] = suffix_max[index + 1]
            .checked_add(i64::from(stages[index].4))
            .ok_or("suffix maximum overflow")?;
    }
    let base_packed = fixed_packed_for_rival(model, rival, fixed, stride)
        .ok_or("invalid fixed rival packed total")?;
    for (stage_index, (_fixture_index, transitions, scalar_transitions, _, _)) in
        stages.iter().enumerate()
    {
        let scalar_work = packed_dp
            .len()
            .checked_mul(scalar_transitions.len())
            .ok_or("scalar packed work overflow")?;
        if !charge(report, scalar_work) {
            return Err("scalar packed DP cap");
        }
        let mut scalar_next = BTreeMap::<i32, f64>::new();
        for (&packed, &mass) in &packed_dp {
            for (&gain, &probability) in scalar_transitions {
                let total = packed
                    .checked_add(gain)
                    .ok_or("scalar packed state overflow")?;
                let entry = scalar_next.entry(total).or_default();
                *entry += mass * probability;
                if scalar_next.len() > MAX_STATES {
                    return Err("scalar packed state cap");
                }
            }
        }
        packed_dp = scalar_next;

        let joint_work = joint_dp
            .len()
            .checked_mul(transitions.len())
            .ok_or("joint packed work overflow")?;
        if !charge(report, joint_work) {
            return Err("joint packed/GD DP cap");
        }
        let mut joint_next = Joint::new();
        let suffix_low = suffix_min[stage_index + 1];
        let suffix_high = suffix_max[stage_index + 1];
        for (&(packed, gd), &mass) in &joint_dp {
            for &((gain, gd_gain), probability) in transitions {
                let next_packed = packed
                    .checked_add(gain)
                    .ok_or("joint packed state overflow")?;
                let possible_min = i64::from(base_packed) + i64::from(next_packed) + suffix_low;
                let possible_max = i64::from(base_packed) + i64::from(next_packed) + suffix_high;
                if i64::from(target_packed) < possible_min
                    || i64::from(target_packed) > possible_max
                {
                    continue;
                }
                let next_gd = gd.checked_add(gd_gain).ok_or("joint GD state overflow")?;
                let entry = joint_next.entry((next_packed, next_gd)).or_default();
                *entry += mass * probability;
                if joint_next.len() > MAX_STATES {
                    return Err("joint packed/GD state cap");
                }
            }
        }
        joint_dp = joint_next;
    }
    if !charge(report, packed_dp.len().saturating_add(joint_dp.len())) {
        return Err("final packed scan cap");
    }
    let mut denominator = 0.0;
    let mut strict_mass = 0.0;
    let mut scalar_tie_mass = 0.0;
    let mut below_mass = 0.0;
    for (&packed_delta, &mass) in &packed_dp {
        let packed = base_packed
            .checked_add(packed_delta)
            .ok_or("rival packed overflow")?;
        if packed >= lower && packed <= upper {
            denominator += mass;
            if packed > target_packed {
                strict_mass += mass;
            } else if packed == target_packed {
                scalar_tie_mass += mass;
            } else {
                below_mass += mass;
            }
        }
    }
    if !denominator.is_finite() || denominator <= 0.0 {
        return Err("zero rival packed-interval mass");
    }
    let mut tie_by_z = Pmf::new();
    for (&(packed_delta, gd), &mass) in &joint_dp {
        let packed = base_packed
            .checked_add(packed_delta)
            .ok_or("joint rival packed overflow")?;
        if packed == target_packed && packed >= lower && packed <= upper {
            *tie_by_z.entry(gd).or_default() += mass;
        }
    }
    let joint_tie_mass = tie_by_z.values().sum::<f64>();
    let tolerance = 1e-9 * scalar_tie_mass.abs().max(f64::MIN_POSITIVE) + 8.0 * f64::MIN_POSITIVE;
    if (joint_tie_mass - scalar_tie_mass).abs() > tolerance {
        return Err("joint tie mass disagrees with scalar packed DP");
    }
    let _ = target;
    Ok(RivalCalc {
        denominator,
        strict_mass,
        below_mass,
        tie_by_z,
    })
}

fn shared_rival_factor_by_target_gd(
    model: &Model,
    cell: Cell,
    leaf: &Leaf,
    rival: usize,
    target_packed: i32,
    fixed: &HashMap<usize, u8>,
    games: &HashMap<usize, RankGame>,
    stride: i32,
    caches: &mut Caches,
    report: &mut Report,
) -> Result<(Pmf, Arc<Pmf>, Value), &'static str> {
    if !charge(
        report,
        fixed.len().saturating_add(games.len()).saturating_add(4),
    ) {
        return Err("rival query key scan cap");
    }
    let target_pmf = canonical_target_pmf(model, cell.team, &leaf.root, caches, report)?;
    let split_key = (leaf.root.clone(), rival);
    let split = if let Some(value) = caches.target_splits.get(&split_key) {
        report.target_cache_hits += 1;
        if !charge(report, 1) {
            return Err("target split cache lookup cap");
        }
        Arc::clone(value)
    } else {
        let value = target_split_pmfs(model, cell.team, &leaf.root, rival, caches, report)?;
        let bins = value.0.len().saturating_add(value.1.len());
        if caches.target_splits.len() < MAX_TARGET_CACHE
            && caches.cached_pmf_bins.saturating_add(bins) <= MAX_CACHED_PMF_BINS
        {
            if !charge(report, bins.saturating_add(1)) {
                return Err("target split cache work cap");
            }
            caches.cached_pmf_bins += bins;
            caches
                .target_splits
                .insert(split_key, Arc::new(value.clone()));
        }
        Arc::new(value)
    };
    let (shared_u, private_v) = split.as_ref();
    let lower = leaf.pattern.lower[rival];
    let upper = leaf.pattern.upper[rival];
    let mut key = QueryKey {
        root: leaf.root.clone(),
        target: cell.team,
        rival,
        target_packed,
        lower,
        upper,
        stride,
        fixed: fixed
            .iter()
            .filter_map(|(index, wdl)| {
                let fixture = model.fixtures.get(*index)?;
                (fixture.home == rival || fixture.away == rival).then_some((*index, *wdl))
            })
            .collect(),
        domains: games
            .iter()
            .filter_map(|(index, game)| {
                let fixture = model.fixtures.get(*index)?;
                (fixture.home == rival || fixture.away == rival)
                    .then_some((*index, game.prob.map(f64::to_bits)))
            })
            .collect(),
    };
    key.fixed.sort_unstable();
    key.domains.sort_unstable();
    let calc = if let Some(value) = caches.queries.get(&key) {
        report.query_cache_hits += 1;
        if !charge(report, 1) {
            return Err("rival query cache lookup cap");
        }
        Arc::clone(value)
    } else {
        let value = private_rival_dp(
            model,
            leaf,
            cell.team,
            rival,
            target_packed,
            fixed,
            games,
            stride,
            caches,
            report,
        )?;
        let bins = value.tie_by_z.len();
        let key_items = key.fixed.len().saturating_add(key.domains.len());
        if caches.queries.len() < MAX_QUERY_CACHE
            && caches.cached_query_bins.saturating_add(bins) <= MAX_CACHED_QUERY_BINS
            && caches.cached_query_key_items.saturating_add(key_items) <= MAX_CACHED_QUERY_BINS
        {
            if !charge(report, bins.saturating_add(key_items).saturating_add(1)) {
                return Err("rival query cache work cap");
            }
            caches.cached_query_bins += bins;
            caches.cached_query_key_items += key_items;
            caches.queries.insert(key, Arc::new(value.clone()));
        }
        Arc::new(value)
    };

    let base_target_gd = model.base[cell.team]
        .gf
        .checked_sub(model.base[cell.team].ga)
        .ok_or("target GD overflow")?;
    let base_rival_gd = model.base[rival]
        .gf
        .checked_sub(model.base[rival].ga)
        .ok_or("rival GD overflow")?;
    let mut q_by_x = Pmf::new();
    for (&x, &target_mass) in target_pmf.iter() {
        if !target_mass.is_finite() || target_mass <= 0.0 {
            continue;
        }
        if !charge(report, shared_u.len()) {
            return Err("shared GD conditional scan cap");
        }
        let mut posterior_terms = Vec::<(i32, f64)>::new();
        for (&u, &u_mass) in shared_u {
            let v_key = x
                .checked_sub(base_target_gd)
                .and_then(|v| v.checked_sub(u))
                .ok_or("target GD conditional margin overflow")?;
            let v_mass = private_v.get(&v_key).copied().unwrap_or(0.0);
            if u_mass > 0.0 && v_mass > 0.0 {
                posterior_terms.push((u, u_mass.ln() + v_mass.ln()));
            }
        }
        if posterior_terms.is_empty() {
            return Err("positive target GD atom has no shared/private decomposition support");
        }
        let log_normalizer = logsumexp(
            &posterior_terms
                .iter()
                .map(|(_, value)| *value)
                .collect::<Vec<_>>(),
        );
        if !log_normalizer.is_finite() {
            return Err("invalid shared/private target GD normalizer");
        }
        if (log_normalizer - target_mass.ln()).abs() > 1e-8 {
            return Err("shared/private conditional target GD mass mismatch");
        }
        let tail_work = posterior_terms
            .len()
            .checked_mul(calc.tie_by_z.len())
            .and_then(|work| work.checked_mul(2))
            .and_then(|work| work.checked_add(posterior_terms.len()))
            .ok_or("conditional GD tail work overflow")?;
        if !charge(report, tail_work) {
            return Err("conditional GD tail scan cap");
        }
        let mut tie_probability = 0.0;
        for (u, log_weight) in posterior_terms {
            let threshold = x
                .checked_sub(base_rival_gd)
                .and_then(|v| v.checked_add(u))
                .ok_or("rival GD threshold overflow")?;
            let tail = calc
                .tie_by_z
                .range(threshold..)
                .map(|(_, mass)| *mass)
                .sum::<f64>();
            tie_probability += (log_weight - log_normalizer).exp() * tail / calc.denominator;
        }
        let q = calc.strict_mass / calc.denominator + tie_probability;
        if !q.is_finite() || q < -1e-12 || q > 1.0 + 1e-10 {
            return Err("invalid conditional rival pass probability");
        }
        q_by_x.insert(x, q.clamp(0.0, 1.0));
    }
    if !charge(report, q_by_x.len().saturating_mul(2)) {
        return Err("rival q summary scan cap");
    }
    let q_marginal = target_pmf
        .iter()
        .map(|(x, mass)| mass * q_by_x.get(x).copied().unwrap_or(0.0))
        .sum::<f64>();
    let q_min = q_by_x.values().copied().fold(f64::INFINITY, f64::min);
    let q_max = q_by_x.values().copied().fold(f64::NEG_INFINITY, f64::max);
    let tie_mass = calc.tie_by_z.values().sum::<f64>();
    let row = json!({
        "rival_team_id":model.ids[rival], "status":"conditional_tiebreak",
        "denominator":calc.denominator, "strict_mass":calc.strict_mass,
        "tie_mass":tie_mass, "below_mass":calc.below_mass,
        "marginal_q":q_marginal,"q_min":q_min,"q_max":q_max,
        "target_gd_values":target_pmf.len(), "shared_margin_values":shared_u.len(),
        "private_tie_gd_values":calc.tie_by_z.len()
    });
    Ok((q_by_x, target_pmf, row))
}

fn logsumexp(terms: &[f64]) -> f64 {
    let max = terms.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !max.is_finite() {
        return max;
    }
    max + terms
        .iter()
        .map(|value| (*value - max).exp())
        .sum::<f64>()
        .ln()
}

fn integrate_shared_grid(target_pmf: &Pmf, q_by_rival: &[Pmf]) -> Result<f64, &'static str> {
    let mut terms = Vec::new();
    for (&x, &mass) in target_pmf {
        if !mass.is_finite() || mass < 0.0 {
            return Err("invalid target GD mass");
        }
        if mass == 0.0 {
            continue;
        }
        let mut product_log = 0.0;
        for q_pmf in q_by_rival {
            let q = q_pmf
                .get(&x)
                .copied()
                .ok_or("missing rival q on common target grid")?;
            if !q.is_finite() || !(0.0..=1.0).contains(&q) {
                return Err("invalid rival pass probability");
            }
            if q == 0.0 {
                product_log = f64::NEG_INFINITY;
                break;
            }
            product_log += q.ln();
        }
        if product_log.is_finite() {
            terms.push(mass.ln() + product_log);
        }
    }
    Ok(logsumexp(&terms))
}

fn log_factor_for_leaf(
    model: &Model,
    cell: Cell,
    leaf: &Leaf,
    fixed: &HashMap<usize, u8>,
    games: &HashMap<usize, RankGame>,
    stride: i32,
    caches: &mut Caches,
    report: &mut Report,
) -> Result<(f64, Vec<Value>, Value), &'static str> {
    if !charge(report, model.n) {
        return Err("selected rival scan cap");
    }
    let target_packed = fixed_packed_for_rival(model, cell.team, fixed, stride)
        .ok_or("invalid target packed score")?;
    let mut target_pmf: Option<Arc<Pmf>> = None;
    let mut q_by_rival = Vec::<Pmf>::new();
    let mut rival_rows = Vec::new();
    let mut has_factor_rival = false;
    for rival in 0..model.n {
        if rival == cell.team || leaf.selected & (1 << rival) == 0 {
            continue;
        }
        if leaf.mask & (1 << rival) != 0 {
            rival_rows.push(json!({"rival_team_id":model.ids[rival],"status":"strict_primary_below","factor":1.0}));
            continue;
        }
        let lower = leaf.pattern.lower[rival];
        if lower > target_packed {
            rival_rows.push(json!({"rival_team_id":model.ids[rival],"status":"interval_strictly_ahead","factor":1.0}));
            continue;
        }
        if lower < target_packed || leaf.pattern.upper[rival] < target_packed {
            return Err(
                "selected nonstrict rival interval is not saturated at target primary score",
            );
        }
        has_factor_rival = true;
        let (q_by_x, this_target_pmf, row) = shared_rival_factor_by_target_gd(
            model,
            cell,
            leaf,
            rival,
            target_packed,
            fixed,
            games,
            stride,
            caches,
            report,
        )?;
        if let Some(existing) = &target_pmf {
            if !charge(report, this_target_pmf.len()) {
                return Err("common target support validation cap");
            }
            if existing.as_ref().keys().ne(this_target_pmf.keys()) {
                return Err("target GD cache support mismatch");
            }
        } else {
            target_pmf = Some(Arc::clone(&this_target_pmf));
        }
        q_by_rival.push(q_by_x);
        rival_rows.push(row);
    }
    if !has_factor_rival {
        return Ok((0.0, rival_rows, Value::Null));
    }
    let target_pmf = target_pmf.ok_or("target GD distribution missing")?;
    let factor_count = q_by_rival.len();
    if !charge(
        report,
        target_pmf
            .len()
            .checked_mul(factor_count.saturating_add(1))
            .ok_or("shared target logsum work overflow")?,
    ) {
        return Err("shared target logsum cap");
    }
    let raw_log = integrate_shared_grid(&target_pmf, &q_by_rival)?;
    if raw_log.is_nan() || raw_log == f64::INFINITY || raw_log > 1e-10 {
        return Err("invalid shared target log-factor");
    }
    let raw_log = raw_log.min(0.0);
    if !charge(
        report,
        target_pmf
            .len()
            .checked_mul(factor_count.saturating_mul(2).saturating_add(4))
            .ok_or("target posterior work overflow")?,
    ) {
        return Err("target posterior telemetry cap");
    }
    let prior_mean = target_pmf
        .iter()
        .map(|(x, p)| f64::from(*x) * p)
        .sum::<f64>();
    let prior_mode = target_pmf
        .iter()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(x, _)| *x);
    let posterior_mean = if raw_log.is_finite() {
        target_pmf
            .iter()
            .filter_map(|(x, mass)| {
                if *mass <= 0.0 {
                    return None;
                }
                let mut lp = 0.0;
                for q_pmf in &q_by_rival {
                    let q = q_pmf.get(x).copied()?;
                    if q == 0.0 {
                        return None;
                    }
                    lp += q.ln();
                }
                Some(f64::from(*x) * (mass.ln() + lp - raw_log).exp())
            })
            .sum::<f64>()
    } else {
        f64::NAN
    };
    let posterior_mode = target_pmf
        .iter()
        .filter_map(|(x, mass)| {
            if *mass <= 0.0 {
                return None;
            }
            let mut lp = 0.0;
            for q_pmf in &q_by_rival {
                let q = q_pmf.get(x).copied()?;
                if q == 0.0 {
                    return None;
                }
                lp += q.ln();
            }
            Some((*x, mass.ln() + lp))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(x, _)| x);
    let target_summary = json!({
        "target_gd_support":target_pmf.len(),
        "prior_mean":prior_mean,"prior_mode":prior_mode,
        "posterior_mean":posterior_mean.is_finite().then_some(posterior_mean),
        "posterior_mode":posterior_mode,
        "raw_log_factor":raw_log.is_finite().then_some(raw_log)
    });
    Ok((raw_log, rival_rows, target_summary))
}

fn fully_classified_saturated(leaf: &Leaf) -> bool {
    leaf.selected & leaf.ambiguous == leaf.ambiguous
        && leaf.mask.count_ones() as usize == leaf.max_exceptions
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
        cap: cap.min(MAX_WORK_LIMIT),
        ..Report::default()
    };
    if corrected_logs.len() != leaves.len() {
        report.reason = Some("corrected guide alignment unavailable");
        return report;
    }
    if !matches!(
        model.keys.as_slice(),
        [Key::Pt, Key::W, Key::Gd, ..] | [Key::Pt, Key::Gd, ..]
    ) {
        report.reason = Some("unsupported championship comparator prefix");
        return report;
    }
    let Some(screen) = screen else {
        report.reason = Some("root priority unavailable");
        return report;
    };
    if !charge(
        &mut report,
        model
            .fixtures
            .len()
            .saturating_mul(2)
            .saturating_add(model.n),
    ) {
        report.reason = Some("packed stride scan cap");
        return report;
    }
    let Some(stride) = packed_stride(model) else {
        report.reason = Some("packed stride unavailable");
        return report;
    };
    let mut order = (0..leaves.len()).collect::<Vec<_>>();
    let sorting_work = leaves.len().saturating_mul(usize::BITS as usize);
    if !charge(&mut report, sorting_work) {
        report.reason = Some("shared GD query ordering cap");
        return report;
    }
    order.sort_by(|&a, &b| {
        let ra = screen
            .roots
            .get(leaves[a].root_index)
            .map_or(0.0, |root| root.raw_score);
        let rb = screen
            .roots
            .get(leaves[b].root_index)
            .map_or(0.0, |root| root.raw_score);
        rb.total_cmp(&ra)
            .then_with(|| leaves[a].root_index.cmp(&leaves[b].root_index))
            .then_with(|| corrected_logs[b].total_cmp(&corrected_logs[a]))
            .then_with(|| a.cmp(&b))
    });
    let mut caches = Caches::default();
    let mut stopped = false;
    let mut root_coverage = BTreeMap::<usize, (usize, usize, usize)>::new();
    for (processing_rank, index) in order.into_iter().enumerate() {
        let leaf = &leaves[index];
        if leaf.above || !fully_classified_saturated(leaf) {
            continue;
        }
        root_coverage.entry(leaf.root_index).or_default().0 += 1;
        if leaf.rival_team_id.is_none() {
            report.eligible_unrefined += 1;
        }
        if stopped {
            report.query_rows.push(json!({"processing_rank":processing_rank,"leaf_index":index,"original_parent_id":leaf.original_parent_id,"root_index":leaf.root_index,"root":leaf.root,"reason":"shared GD cap reached; pre-GD guide retained"}));
            continue;
        }
        let domain_count = leaf
            .pattern
            .guide
            .games
            .len()
            .saturating_add(leaf.pattern.joint.games.len())
            .saturating_add(leaf.pattern.omitted.len());
        if !charge(
            &mut report,
            model
                .fixtures
                .len()
                .saturating_add(leaf.pattern.fixed.len())
                .saturating_add(domain_count.saturating_mul(4))
                .saturating_add(16),
        ) {
            stopped = true;
            report.query_rows.push(json!({"processing_rank":processing_rank,"leaf_index":index,"original_parent_id":leaf.original_parent_id,"reason":"fixed/domain mapping scan cap","applied":false}));
            continue;
        }
        let fixed = match fixed_map(model, leaf, cell.team) {
            Ok(fixed) => fixed,
            Err(reason) => {
                report.query_rows.push(json!({"processing_rank":processing_rank,"leaf_index":index,"original_parent_id":leaf.original_parent_id,"root_index":leaf.root_index,"root":leaf.root,"reason":reason,"applied":false}));
                continue;
            }
        };
        let games = match rank_games(model, &leaf.pattern) {
            Ok(games) => games,
            Err(reason) => {
                report.query_rows.push(json!({"processing_rank":processing_rank,"leaf_index":index,"original_parent_id":leaf.original_parent_id,"root_index":leaf.root_index,"root":leaf.root,"reason":reason,"applied":false}));
                continue;
            }
        };
        match fixed_packed_for_rival(model, cell.team, &fixed, stride) {
            Some(value)
                if value >= leaf.pattern.lower[cell.team]
                    && value <= leaf.pattern.upper[cell.team] => {}
            _ => {
                report.query_rows.push(json!({"processing_rank":processing_rank,"leaf_index":index,"original_parent_id":leaf.original_parent_id,"reason":"target packed score outside leaf interval","applied":false}));
                continue;
            }
        }
        let result = log_factor_for_leaf(
            model,
            cell,
            leaf,
            &fixed,
            &games,
            stride,
            &mut caches,
            &mut report,
        );
        match result {
            Ok((raw_log_factor, rivals, target_summary)) => {
                if !rivals
                    .iter()
                    .any(|rival| rival["status"] == "conditional_tiebreak")
                {
                    report.query_rows.push(json!({"processing_rank":processing_rank,"leaf_index":index,"original_parent_id":leaf.original_parent_id,"root_index":leaf.root_index,"root":leaf.root,"reason":"no tie-capable nonstrict rival","applied":false,"required_rivals":rivals}));
                    continue;
                }
                if !raw_log_factor.is_finite() && raw_log_factor != f64::NEG_INFINITY {
                    report.query_rows.push(json!({"processing_rank":processing_rank,"leaf_index":index,"original_parent_id":leaf.original_parent_id,"root_index":leaf.root_index,"root":leaf.root,"reason":"invalid raw shared GD log-factor","applied":false,"required_rivals":rivals}));
                    continue;
                }
                let clamped = raw_log_factor < MIN_FACTOR.ln();
                let log_factor = if clamped {
                    MIN_FACTOR.ln()
                } else {
                    raw_log_factor
                };
                if !corrected_logs[index].is_finite() {
                    report.query_rows.push(json!({"processing_rank":processing_rank,"leaf_index":index,"original_parent_id":leaf.original_parent_id,"root_index":leaf.root_index,"root":leaf.root,"reason":"invalid corrected guide","applied":false}));
                    continue;
                }
                corrected_logs[index] += log_factor;
                report.applied = true;
                root_coverage.entry(leaf.root_index).or_default().1 += 1;
                if leaf.rival_team_id.is_none() {
                    report.applied_unrefined += 1;
                }
                report.query_rows.push(json!({"processing_rank":processing_rank,"leaf_index":index,"original_parent_id":leaf.original_parent_id,"root_index":leaf.root_index,"root":leaf.root,"rival_team_id":leaf.rival_team_id.map(|team|model.ids[team]),"path_id":leaf.path_id,"factor":log_factor.exp(),"raw_log_factor":if raw_log_factor.is_finite(){Some(raw_log_factor)}else{None},"log_factor":log_factor,"factor_clamped":clamped,"target_goal_distribution":target_summary,"required_rivals":rivals,"applied":true}));
            }
            Err(reason) => {
                root_coverage.entry(leaf.root_index).or_default().2 += 1;
                report.query_rows.push(json!({"processing_rank":processing_rank,"leaf_index":index,"original_parent_id":leaf.original_parent_id,"root_index":leaf.root_index,"root":leaf.root,"rival_team_id":leaf.rival_team_id.map(|team|model.ids[team]),"path_id":leaf.path_id,"reason":reason,"applied":false}));
                stopped = report.work >= report.cap;
            }
        }
    }
    report.root_coverage = root_coverage
        .into_iter()
        .map(|(root_index, (eligible, applied, failed))| {
            json!({"root_index":root_index,"eligible":eligible,"applied":applied,"failed":failed})
        })
        .collect();
    if stopped {
        report.reason = Some("shared GD work cap reached; remaining guides unchanged");
    } else if !report.applied {
        report.reason = Some("no eligible shared GD factor");
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::joint_caps::propagated::{Guide, NecessaryJoint};

    #[test]
    fn shared_target_goal_decomposition_preserves_non_degenerate_conditional_scores() {
        // Conditional WDL fixes a loss but not the scoreline; the direct
        // target-margin kernel must retain multiple GD atoms.
        let request: crate::model::Request = serde_json::from_value(json!({
            "id":1,
            "phase":{"sort":"pt,w,gd,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":[{"team_id":0,"bias":0},{"team_id":1,"bias":0}],
            "games":[{"id":1,"home_id":0,"away_id":1,"home_power":1.1,"away_power":0.9}]
        })).unwrap();
        let model = Model::new(request).unwrap();
        let mut report = Report {
            cap: WORK_LIMIT,
            ..Report::default()
        };
        let mut caches = Caches::default();
        let pmf = margin_pmf(&model, 0, 0, 0, &mut caches, &mut report).unwrap();
        assert!(pmf.len() > 1);
        let total = pmf.values().sum::<f64>();
        assert!((total - 1.0).abs() < 1e-12);
    }

    #[test]
    fn common_target_grid_multiplies_rivals_before_integrating() {
        let target = Pmf::from([(0, 0.5), (1, 0.5)]);
        let low_only = Pmf::from([(0, 1.0), (1, 0.0)]);
        let log_joint = integrate_shared_grid(&target, &[low_only.clone(), low_only]).unwrap();
        assert!((log_joint.exp() - 0.5).abs() < 1e-12);
        assert!((log_joint.exp() - 0.25).abs() > 0.2);
    }

    #[test]
    fn fixed_primary_tie_factor_matches_conditional_score_atom_enumeration() {
        const TINY_TAIL_BASE_GD: i32 = 12;
        let request: crate::model::Request = serde_json::from_value(json!({
            "id":1,
            "phase":{"sort":"pt,gd","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":[{"team_id":0,"bias":0},{"team_id":1,"bias":0},{"team_id":2,"bias":0}],
            "games":[
                {"id":10,"home_id":0,"away_id":2,"home_power":1.4,"away_power":0.8},
                {"id":11,"home_id":1,"away_id":2,"home_power":1.2,"away_power":0.9}
            ]
        }))
        .unwrap();
        let mut model = Model::new(request).unwrap();
        let lower = vec![3, 3, 0];
        let upper = vec![3, 3, i32::MAX];
        let base = vec![0; 3];
        let unconstrained_lower = vec![0; 3];
        let unconstrained_upper = vec![i32::MAX; 3];
        let (joint, omitted) = NecessaryJoint::new(
            &[],
            &base,
            &unconstrained_lower,
            &unconstrained_upper,
            4,
            10_000,
        )
        .unwrap();
        let pattern = Pattern {
            fixed: vec![(1, 2)],
            bias: None,
            omitted,
            base,
            lower,
            upper,
            guide: Arc::new(Guide::new(Vec::new(), 3)),
            joint: Arc::new(joint),
            mass: 1.0,
            tilt: 1.0,
            cumulative: 1.0,
            bound_guidance: false,
        };
        let leaf = Leaf {
            root: vec![2],
            root_index: 0,
            above: false,
            mask: 0,
            selected: 1 << 1,
            ambiguous: 1 << 1,
            max_exceptions: 0,
            pattern,
            frozen: false,
            score: 1.0,
            rank_hint_next_team: None,
            original_parent_id: 0,
            rival_team_id: None,
            path_id: None,
            rival_fixed: vec![],
            raw_path_prior: 1.0,
            parent_proxy_log: None,
            child_proxy_log: None,
            local_child_share: 1.0,
            guide_fraction: 1.0,
            hybrid_depth: 0,
            hybrid_path_count: 0,
        };
        let fixed = fixed_map(&model, &leaf, 0).unwrap();
        let games = rank_games(&model, &leaf.pattern).unwrap();
        let mut report = Report {
            cap: WORK_LIMIT,
            ..Report::default()
        };
        let mut caches = Caches::default();
        let (log_factor, rivals, _) = log_factor_for_leaf(
            &model,
            Cell { team: 0, rank: 1 },
            &leaf,
            &fixed,
            &games,
            1,
            &mut caches,
            &mut report,
        )
        .unwrap();

        let target = margin_pmf(&model, 0, 0, 2, &mut caches, &mut report).unwrap();
        let rival = margin_pmf(&model, 1, 1, 2, &mut caches, &mut report).unwrap();
        let brute_force = target
            .iter()
            .map(|(target_gd, target_mass)| {
                target_mass * rival.range(*target_gd..).map(|(_, mass)| mass).sum::<f64>()
            })
            .sum::<f64>();
        assert!((log_factor.exp() - brute_force).abs() < 1e-10);
        assert!((rivals[0]["denominator"].as_f64().unwrap() - 1.0).abs() < 1e-12);
        assert!(rivals[0]["strict_mass"].as_f64().unwrap().abs() < 1e-12);
        assert!((brute_force - 0.5).abs() > 1e-3);

        let screen = path_priority::Screen {
            roots: vec![path_priority::RootShare {
                prior: 1.0,
                count_chance: 1.0,
                raw_score: 1.0,
                normalized_share: 1.0,
            }],
            work: 0,
            reason: None,
            leaf_comparators: None,
            leaf_work: 0,
            leaf_reason: None,
        };
        let mut corrected_logs = vec![0.0];
        let applied = apply(
            &model,
            Cell { team: 0, rank: 1 },
            &[leaf.clone()],
            &mut corrected_logs,
            Some(&screen),
            WORK_LIMIT,
        );
        assert!(applied.applied);
        assert_eq!(applied.applied_unrefined, 1);
        assert_eq!(applied.eligible_unrefined, 1);
        assert!((corrected_logs[0].exp() - brute_force).abs() < 1e-10);

        let mut capped_logs = vec![0.25];
        let capped = apply(
            &model,
            Cell { team: 0, rank: 1 },
            &[leaf.clone()],
            &mut capped_logs,
            Some(&screen),
            0,
        );
        assert!(!capped.applied);
        assert_eq!(capped_logs, vec![0.25]);
        assert!(capped.work <= capped.cap);

        // Shift only the target's already-earned GD so the direct conditional
        // atom tail is small; compare relatively to detect CDF cancellation.
        model.base[0].gf = TINY_TAIL_BASE_GD;
        let tiny_fixed = fixed_map(&model, &leaf, 0).unwrap();
        let mut tiny_report = Report {
            cap: WORK_LIMIT,
            ..Report::default()
        };
        let mut tiny_caches = Caches::default();
        let (tiny_log_factor, _, _) = log_factor_for_leaf(
            &model,
            Cell { team: 0, rank: 1 },
            &leaf,
            &tiny_fixed,
            &games,
            1,
            &mut tiny_caches,
            &mut tiny_report,
        )
        .unwrap();
        let tiny_target = margin_pmf(&model, 0, 0, 2, &mut tiny_caches, &mut tiny_report).unwrap();
        let tiny_rival = margin_pmf(&model, 1, 1, 2, &mut tiny_caches, &mut tiny_report).unwrap();
        let tiny_brute = tiny_target
            .iter()
            .map(|(target_gd, target_mass)| {
                let final_target_gd = target_gd.checked_add(TINY_TAIL_BASE_GD).unwrap();
                target_mass
                    * tiny_rival
                        .range(final_target_gd..)
                        .map(|(_, mass)| mass)
                        .sum::<f64>()
            })
            .sum::<f64>();
        assert!(tiny_brute > 0.0);
        assert!(tiny_brute < 1e-8);
        assert!(tiny_brute < brute_force * 0.01);
        assert!((tiny_log_factor.exp() / tiny_brute - 1.0).abs() < 1e-8);
    }

    #[test]
    fn shared_target_rival_fixture_uses_opposite_gd_sign() {
        let request: crate::model::Request = serde_json::from_value(json!({
            "id":1,
            "phase":{"sort":"pt,gd","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":[{"team_id":0,"bias":0},{"team_id":1,"bias":0}],
            "games":[{"id":10,"home_id":0,"away_id":1,"home_power":1.3,"away_power":0.8}]
        }))
        .unwrap();
        let mut model = Model::new(request).unwrap();
        // The target wins this fixture while the rival's three base points
        // offset the loss, so their primary totals tie and their GDs oppose.
        model.base[1].points = 3;
        model.base[1].gf = 2;
        model.base[1].ga = 0;
        let base = vec![0, 3];
        let lower = vec![3, 3];
        let upper = vec![3, 3];
        let unconstrained_lower = vec![0; 2];
        let unconstrained_upper = vec![i32::MAX; 2];
        let (joint, omitted) = NecessaryJoint::new(
            &[],
            &base,
            &unconstrained_lower,
            &unconstrained_upper,
            2,
            10_000,
        )
        .unwrap();
        let pattern = Pattern {
            fixed: vec![],
            bias: None,
            omitted,
            base,
            lower,
            upper,
            guide: Arc::new(Guide::new(Vec::new(), 2)),
            joint: Arc::new(joint),
            mass: 1.0,
            tilt: 1.0,
            cumulative: 1.0,
            bound_guidance: false,
        };
        let leaf = Leaf {
            root: vec![2],
            root_index: 0,
            above: false,
            mask: 0,
            selected: 1 << 1,
            ambiguous: 1 << 1,
            max_exceptions: 0,
            pattern,
            frozen: false,
            score: 1.0,
            rank_hint_next_team: None,
            original_parent_id: 0,
            rival_team_id: None,
            path_id: None,
            rival_fixed: vec![],
            raw_path_prior: 1.0,
            parent_proxy_log: None,
            child_proxy_log: None,
            local_child_share: 1.0,
            guide_fraction: 1.0,
            hybrid_depth: 0,
            hybrid_path_count: 0,
        };
        let fixed = fixed_map(&model, &leaf, 0).unwrap();
        let games = rank_games(&model, &leaf.pattern).unwrap();
        let mut report = Report {
            cap: WORK_LIMIT,
            ..Report::default()
        };
        let mut caches = Caches::default();
        let (log_factor, _, _) = log_factor_for_leaf(
            &model,
            Cell { team: 0, rank: 1 },
            &leaf,
            &fixed,
            &games,
            1,
            &mut caches,
            &mut report,
        )
        .unwrap();
        let target = margin_pmf(&model, 0, 0, 2, &mut caches, &mut report).unwrap();
        let brute_force = target
            .iter()
            .filter(|(gd, _)| **gd <= 1)
            .map(|(_, mass)| *mass)
            .sum::<f64>();
        assert!((log_factor.exp() - brute_force).abs() < 1e-10);
        assert!(brute_force > 0.0 && brute_force < 1.0);
    }

    #[test]
    fn variable_primary_interval_is_normalized_once_before_goal_tails() {
        let request: crate::model::Request = serde_json::from_value(json!({
            "id":1,
            "phase":{"sort":"pt,gd","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":[{"team_id":0,"bias":0},{"team_id":1,"bias":0},{"team_id":2,"bias":0},{"team_id":3,"bias":0}],
            "games":[
                {"id":10,"home_id":0,"away_id":3,"home_power":1.4,"away_power":0.8},
                {"id":11,"home_id":1,"away_id":2,"home_power":1.2,"away_power":0.9},
                {"id":12,"home_id":1,"away_id":3,"home_power":1.1,"away_power":0.7}
            ]
        })).unwrap();
        let model = Model::new(request).unwrap();
        let target_fixture = 0;
        let rival_fixtures = [1, 2];
        let base = vec![0; 4];
        let unconstrained_lower = vec![0; 4];
        let unconstrained_upper = vec![i32::MAX; 4];
        let (joint, omitted) = NecessaryJoint::new(
            &[],
            &base,
            &unconstrained_lower,
            &unconstrained_upper,
            4,
            10_000,
        )
        .unwrap();
        let lower = vec![3, 3, 0, 0];
        let upper = vec![3, 9, i32::MAX, i32::MAX];
        let pattern = Pattern {
            fixed: vec![],
            bias: None,
            omitted,
            base,
            lower,
            upper,
            guide: Arc::new(Guide::new(Vec::new(), 4)),
            joint: Arc::new(joint),
            mass: 1.0,
            tilt: 1.0,
            cumulative: 1.0,
            bound_guidance: false,
        };
        let leaf = Leaf {
            root: vec![2],
            root_index: 0,
            above: false,
            mask: 0,
            selected: 1 << 1,
            ambiguous: 1 << 1,
            max_exceptions: 0,
            pattern,
            frozen: false,
            score: 1.0,
            rank_hint_next_team: None,
            original_parent_id: 0,
            rival_team_id: None,
            path_id: None,
            rival_fixed: vec![],
            raw_path_prior: 1.0,
            parent_proxy_log: None,
            child_proxy_log: None,
            local_child_share: 1.0,
            guide_fraction: 1.0,
            hybrid_depth: 0,
            hybrid_path_count: 0,
        };

        for win_probability in [0.2, 1e-12] {
            let loss_probability = 1.0 - win_probability;
            let mut games = HashMap::new();
            for fixture in rival_fixtures {
                let model_fixture = &model.fixtures[fixture];
                games.insert(
                    fixture,
                    RankGame {
                        index: fixture,
                        home: model_fixture.home,
                        away: model_fixture.away,
                        prob: [loss_probability, 0.0, win_probability],
                        hg: [0, 1, 3],
                        ag: [3, 1, 0],
                    },
                );
            }
            let mut variable_leaf = leaf.clone();
            variable_leaf.pattern.guide =
                Arc::new(Guide::new(games.values().copied().collect(), 4));
            let fixed = fixed_map(&model, &variable_leaf, 0).unwrap();
            let mut report = Report {
                cap: WORK_LIMIT,
                ..Report::default()
            };
            let mut caches = Caches::default();
            let (log_factor, rivals, _) = log_factor_for_leaf(
                &model,
                Cell { team: 0, rank: 1 },
                &variable_leaf,
                &fixed,
                &games,
                1,
                &mut caches,
                &mut report,
            )
            .unwrap();

            let target_pmf =
                margin_pmf(&model, target_fixture, 0, 2, &mut caches, &mut report).unwrap();
            let outcome_probabilities = [loss_probability, 0.0, win_probability];
            let mut expected_denominator = 0.0;
            let mut expected_strict = 0.0;
            let mut expected_goal_mass = 0.0;
            for first in 0..3u8 {
                for second in 0..3u8 {
                    let mass = outcome_probabilities[first as usize]
                        * outcome_probabilities[second as usize];
                    if mass == 0.0 {
                        continue;
                    }
                    let packed_points = i32::from(games[&rival_fixtures[0]].hg[first as usize])
                        + i32::from(games[&rival_fixtures[1]].hg[second as usize]);
                    if packed_points < 3 {
                        continue;
                    }
                    expected_denominator += mass;
                    if packed_points > 3 {
                        expected_strict += mass;
                        expected_goal_mass += mass;
                        continue;
                    }
                    let first_margin = margin_pmf(
                        &model,
                        rival_fixtures[0],
                        1,
                        first,
                        &mut caches,
                        &mut report,
                    )
                    .unwrap();
                    let second_margin = margin_pmf(
                        &model,
                        rival_fixtures[1],
                        1,
                        second,
                        &mut caches,
                        &mut report,
                    )
                    .unwrap();
                    let rival_pmf = convolve(&first_margin, &second_margin, &mut report).unwrap();
                    let conditional_goal_pass = target_pmf
                        .iter()
                        .map(|(target_gd, target_mass)| {
                            target_mass
                                * rival_pmf
                                    .range(*target_gd..)
                                    .map(|(_, value)| value)
                                    .sum::<f64>()
                        })
                        .sum::<f64>();
                    expected_goal_mass += mass * conditional_goal_pass;
                }
            }
            assert!(
                (rivals[0]["denominator"].as_f64().unwrap() / expected_denominator - 1.0).abs()
                    < 1e-8
            );
            assert!(
                (rivals[0]["strict_mass"].as_f64().unwrap() / expected_strict.max(1e-300) - 1.0)
                    .abs()
                    < 1e-8
            );
            assert!(
                (log_factor.exp() / (expected_goal_mass / expected_denominator) - 1.0).abs() < 1e-8
            );
            assert!(expected_goal_mass > 0.0 && expected_goal_mass.is_finite());
        }
    }
}
