use super::Engine;
use crate::{
    gd_bounds::GdCache,
    joint::{residual_certificate_with_options as certify_residual_with_options, Forest, Plan as JointPlan, ResidualCertificate, ResidualSource, SuffixOptions},
    law::{categorical, categorical_probabilities, Interval, Law},
    pair::PairPlan,
};
use golaberto_odds::{
    domains::{DomainWork, Domains},
    lookahead::RankGame,
    model::Model,
    rng::Rng,
};
use serde_json::{json, Value};
use std::time::Instant;

pub(super) struct RegimePlan {
    pub bound: f64,
    pub states: usize,
    pub complete: bool,
    pub pivot: usize,
    pub selected: Vec<usize>,
    weights: Vec<f64>,
    atoms: Vec<Atom>,
    forest: Option<Forest>,
    forest_atoms: Vec<ForestAtom>,
    pub pattern_count: usize,
}

struct Atom {
    plan: JointPlan,
    pair: Option<PairPlan>,
    q: Interval,
}

pub(super) enum HybridAtomContext {
    Atom(usize),
    ForestRoot(usize),
}

struct ForestAtom {
    root_index: usize,
    pair: Option<PairPlan>,
    q: Interval,
}

impl RegimePlan {
    fn atom_count(&self) -> usize {
        if self.forest.is_some() {
            self.forest_atoms.len()
        } else {
            self.atoms.len()
        }
    }
}

impl RegimePlan {
    pub fn draw(
        &self,
        model: &Model,
        laws: &[Law],
        rng: &mut Rng,
    ) -> Result<(Vec<u8>, Interval, Vec<Option<[i32; 2]>>), String> {
        let (index, pi) = categorical(&self.weights, rng)?;
        if pi.lo <= 0.0 {
            return Err("regime mixture ticket has zero lower probability".into());
        }
        if let Some(forest) = &self.forest {
            let atom = &self.forest_atoms[index];
            let root = forest
                .roots
                .get(atom.root_index)
                .ok_or("regime forest root missing")?;
            let (outcomes, suffix_weight) = forest.draw_root(atom.root_index, laws, rng)?;
            if suffix_weight.hi == 0.0 {
                return Ok((
                    outcomes,
                    Interval::point(0.0),
                    vec![None; model.fixtures.len()],
                ));
            }
            let fixed_scores = if let Some(pair) = &atom.pair {
                pair.draw(model, rng)?
            } else {
                vec![None; model.fixtures.len()]
            };
            let weight = root.prefix_mass.div(pi).mul(suffix_weight).mul(atom.q);
            return Ok((outcomes, weight, fixed_scores));
        }
        let atom = self.atoms.get(index).ok_or("regime atom missing")?;
        let (outcomes, wdl_weight) = atom.plan.draw(laws, rng)?;
        if wdl_weight.hi == 0.0 {
            return Ok((
                outcomes,
                Interval::point(0.0),
                vec![None; model.fixtures.len()],
            ));
        }
        let fixed_scores = if let Some(pair) = &atom.pair {
            pair.draw(model, rng)?
        } else {
            vec![None; model.fixtures.len()]
        };
        let weight = wdl_weight.mul(atom.q).div(pi);
        Ok((outcomes, weight, fixed_scores))
    }

    pub(super) fn residual_certificate(
        &self,
        model: &Model,
        laws: &[Law],
        cap_b: f64,
        clip_h: f64,
        node_limit: usize,
        deadline: Instant,
    ) -> Result<ResidualCertificate, String> {
        self.residual_certificate_with_options(model, laws, cap_b, clip_h, node_limit, deadline, None)
    }

    pub(super) fn residual_certificate_with_options(
        &self,
        model: &Model,
        laws: &[Law],
        cap_b: f64,
        clip_h: f64,
        node_limit: usize,
        deadline: Instant,
        suffix_options: Option<SuffixOptions>,
    ) -> Result<ResidualCertificate, String> {
        let tickets = categorical_probabilities(&self.weights)?;
        let mut sources: Vec<ResidualSource<'_>> = Vec::new();
        if let Some(forest) = &self.forest {
            for (index, atom) in self.forest_atoms.iter().enumerate() {
                if tickets[index].hi <= 0.0 { continue; }
                sources.push(forest.residual_source(
                    atom.root_index, model, atom.q, tickets[index],
                )?);
            }
        } else {
            for (index, atom) in self.atoms.iter().enumerate() {
                if tickets[index].hi <= 0.0 { continue; }
                sources.push(atom.plan.residual_source(atom.q, tickets[index])?);
            }
        }
        certify_residual_with_options(&sources, model, laws, cap_b, clip_h, node_limit, deadline, suffix_options)
    }

    /// Draw through the same atom and pair mixture while retaining its
    /// likelihood under the baseline proposal and the guide likelihood ratio.
    pub fn draw_hybrid(
        &self,
        model: &Model,
        laws: &[Law],
        rng: &mut Rng,
        guided: bool,
    ) -> Result<(Vec<u8>, Interval, Interval, Vec<Option<[i32; 2]>>), String> {
        let (outcomes, weight, ratio, fixed_scores, _) = self.draw_hybrid_with_context(model, laws, rng, guided)?;
        Ok((outcomes, weight, ratio, fixed_scores))
    }

    pub(super) fn replay_guide_ratio(
        &self,
        context: HybridAtomContext,
        model: &Model,
        laws: &[Law],
        outcomes: &[u8],
    ) -> Result<Interval, String> {
        match context {
            HybridAtomContext::Atom(index) => self.atoms.get(index)
                .ok_or("regime atom missing")?.plan.guide_ratio(model, laws, outcomes),
            HybridAtomContext::ForestRoot(root_index) => self.forest.as_ref()
                .ok_or("regime forest missing")?.root_guide_ratio(root_index, model, laws, outcomes),
        }
    }

    pub(super) fn draw_baseline_hybrid_context(
        &self,
        model: &Model,
        laws: &[Law],
        rng: &mut Rng,
    ) -> Result<(Vec<u8>, Interval, Vec<Option<[i32; 2]>>, HybridAtomContext), String> {
        let (index, pi) = categorical(&self.weights, rng)?;
        if pi.lo <= 0.0 {
            return Err("regime mixture ticket has zero lower probability".into());
        }
        if let Some(forest) = &self.forest {
            let atom = &self.forest_atoms[index];
            let root = forest.roots.get(atom.root_index).ok_or("regime forest root missing")?;
            let (outcomes, suffix_weight) = forest.draw_root(atom.root_index, laws, rng)?;
            let fixed_scores = if let Some(pair) = &atom.pair {
                pair.draw(model, rng)?
            } else { vec![None; model.fixtures.len()] };
            let weight = root.prefix_mass.div(pi).mul(suffix_weight).mul(atom.q);
            return Ok((outcomes, weight, fixed_scores, HybridAtomContext::ForestRoot(atom.root_index)));
        }
        let atom = self.atoms.get(index).ok_or("regime atom missing")?;
        let (outcomes, wdl_weight) = atom.plan.draw(laws, rng)?;
        let fixed_scores = if let Some(pair) = &atom.pair {
            pair.draw(model, rng)?
        } else { vec![None; model.fixtures.len()] };
        let weight = wdl_weight.mul(atom.q).div(pi);
        Ok((outcomes, weight, fixed_scores, HybridAtomContext::Atom(index)))
    }

    pub(super) fn draw_hybrid_with_context(
        &self,
        model: &Model,
        laws: &[Law],
        rng: &mut Rng,
        guided: bool,
    ) -> Result<(Vec<u8>, Interval, Interval, Vec<Option<[i32; 2]>>, HybridAtomContext), String> {
        let (index, pi) = categorical(&self.weights, rng)?;
        if pi.lo <= 0.0 {
            return Err("regime mixture ticket has zero lower probability".into());
        }
        if let Some(forest) = &self.forest {
            let atom = &self.forest_atoms[index];
            let root = forest
                .roots
                .get(atom.root_index)
                .ok_or("regime forest root missing")?;
            let draw = forest.draw_root_hybrid(atom.root_index, model, laws, rng, guided)?;
            let fixed_scores = if let Some(pair) = &atom.pair {
                pair.draw(model, rng)?
            } else {
                vec![None; model.fixtures.len()]
            };
            let base_weight = root
                .prefix_mass
                .div(pi)
                .mul(draw.baseline_weight)
                .mul(atom.q);
            let guide_ratio = draw.guide_over_baseline;
            return Ok((draw.outcomes, base_weight, guide_ratio, fixed_scores, HybridAtomContext::ForestRoot(atom.root_index)));
        }
        let atom = self.atoms.get(index).ok_or("regime atom missing")?;
        let draw = atom.plan.draw_hybrid(model, laws, rng, guided)?;
        let fixed_scores = if let Some(pair) = &atom.pair {
            pair.draw(model, rng)?
        } else {
            vec![None; model.fixtures.len()]
        };
        let base_weight = draw.baseline_weight.mul(atom.q).div(pi);
        Ok((
            draw.outcomes,
            base_weight,
            draw.guide_over_baseline,
            fixed_scores,
            HybridAtomContext::Atom(index),
        ))
    }
}

fn up(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        x
    } else {
        f64::from_bits(x.to_bits() + 1)
    }
}
fn mul_up(a: f64, b: f64) -> f64 {
    if a == 0.0 || b == 0.0 {
        0.0
    } else {
        let x = a * b;
        if x == 0.0 {
            f64::from_bits(1)
        } else {
            up(x)
        }
    }
}

fn make_games(model: &Model, laws: &[Law], stride: i32) -> Vec<RankGame> {
    model
        .fixtures
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let mut hg = [0; 3];
            let mut ag = [0; 3];
            for o in 0..3 {
                let (hp, ap) = match o {
                    0 => (0, 3),
                    1 => (1, 1),
                    _ => (3, 0),
                };
                hg[o] = hp * stride + i32::from(o == 2);
                ag[o] = ap * stride + i32::from(o == 0);
            }
            RankGame {
                index: i,
                home: f.home,
                away: f.away,
                prob: std::array::from_fn(|o| laws[i].outcomes[o].mid()),
                hg,
                ag,
            }
        })
        .collect()
}

fn candidate_for_pivot(
    model: &Model,
    laws: &[Law],
    gd_cache: &mut GdCache,
    masks: &[u8],
    lower: &[i32],
    upper: &[i32],
    rivals: &[usize],
    rank: usize,
    target: usize,
    target_packed: i32,
    pivot: usize,
    selected: &[usize],
    games: &[RankGame],
    packed_base: &[i32],
    stride: i32,
    state_cap: usize,
    deadline: Instant,
) -> Result<Option<RegimePlan>, String> {
    let incident: Vec<usize> = model
        .fixtures
        .iter()
        .enumerate()
        .filter_map(|(i, f)| (f.home == pivot || f.away == pivot).then_some(i))
        .collect();
    let options: Vec<Vec<u8>> = incident
        .iter()
        .map(|&fi| {
            (0..3)
                .filter(|&o| masks[fi] & (1 << o) != 0 && laws[fi].outcomes[o].hi > 0.0)
                .map(|o| o as u8)
                .collect()
        })
        .collect();
    if options.iter().any(Vec::is_empty) {
        return Ok(None);
    }
    let mut suffix_max = vec![0i32; incident.len() + 1];
    for pos in (0..incident.len()).rev() {
        let fi = incident[pos];
        let f = &model.fixtures[fi];
        suffix_max[pos] = suffix_max[pos + 1]
            + options[pos]
                .iter()
                .map(|&o| {
                    if f.home == pivot {
                        games[fi].hg[o as usize]
                    } else {
                        games[fi].ag[o as usize]
                    }
                })
                .max()
                .unwrap_or(0);
    }
    fn visit(
        pos: usize,
        value: i32,
        pivot: usize,
        incident: &[usize],
        options: &[Vec<u8>],
        games: &[RankGame],
        suffix: &[i32],
        target: i32,
        prefix: &mut Vec<u8>,
        atoms: &mut Vec<(Vec<u8>, i32)>,
        overflow: &mut bool,
        deadline: Instant,
    ) {
        if *overflow || Instant::now() >= deadline {
            *overflow = true;
            return;
        }
        if value + suffix[pos] < target {
            return;
        }
        if pos == incident.len() {
            atoms.push((prefix.clone(), value));
            if atoms.len() > 64 {
                *overflow = true;
            }
            return;
        }
        let f = &games[incident[pos]];
        for &o in &options[pos] {
            prefix.push(o);
            let gain = if f.home == pivot {
                f.hg[o as usize]
            } else {
                f.ag[o as usize]
            };
            visit(
                pos + 1,
                value + gain,
                pivot,
                incident,
                options,
                games,
                suffix,
                target,
                prefix,
                atoms,
                overflow,
                deadline,
            );
            prefix.pop();
            if *overflow {
                return;
            }
        }
    }
    let mut atoms = Vec::new();
    let mut overflow = false;
    visit(
        0,
        packed_base[pivot],
        pivot,
        &incident,
        &options,
        games,
        &suffix_max,
        target_packed,
        &mut Vec::new(),
        &mut atoms,
        &mut overflow,
        deadline,
    );
    if overflow || atoms.is_empty() || !atoms.iter().any(|(_, pw)| *pw == target_packed) {
        return Ok(None);
    }

    let mut retained = Vec::<Atom>::new();
    let mut scales = Vec::<f64>::new();
    let mut states = 0usize;
    for (atom, pw) in atoms {
        if Instant::now() >= deadline {
            return Ok(None);
        }
        let mut child_masks = masks.to_vec();
        for (&fi, &o) in incident.iter().zip(&atom) {
            child_masks[fi] = 1 << o;
        }
        let mut work = DomainWork::new(state_cap.saturating_sub(states));
        let Some(domains) = Domains::condition_bounded(
            games,
            packed_base,
            rivals,
            rank,
            target_packed,
            lower,
            upper,
            &child_masks,
            &mut work,
        ) else {
            return Ok(None);
        };
        if !domains.feasible {
            continue;
        }
        child_masks = domains.domains;
        let remaining = state_cap.saturating_sub(states);
        let child = JointPlan::build(
            model,
            laws,
            &child_masks,
            lower,
            upper,
            selected,
            stride,
            remaining,
            deadline,
        )?;
        if child.bound == 0.0 {
            if child.complete {
                continue;
            }
            return Ok(None);
        }
        states = states.saturating_add(child.states);
        if states > state_cap {
            return Ok(None);
        }
        let (pair, q, scale) = if pw == target_packed {
            let Some(pair) =
                PairPlan::build(model, laws, &child_masks, pivot, target, deadline, gd_cache)?
            else {
                return Ok(None);
            };
            let q = pair.normalizer;
            if q.hi == 0.0 {
                continue;
            }
            if q.lo < 0.001 {
                return Ok(None);
            }
            (Some(pair), q, mul_up(child.bound, q.hi))
        } else {
            (None, Interval::point(1.0), child.bound)
        };
        if scale == 0.0 {
            continue;
        }
        retained.push(Atom {
            plan: child,
            pair,
            q,
        });
        scales.push(scale);
    }
    if retained.is_empty() {
        return Ok(Some(RegimePlan {
            bound: 0.0,
            states,
            complete: true,
            pivot,
            selected: selected.to_vec(),
            weights: Vec::new(),
            atoms: retained,
            forest: None,
            forest_atoms: Vec::new(),
            pattern_count: 0,
        }));
    }
    let probabilities = categorical_probabilities(&scales)?;
    let mut bound = 0.0f64;
    for (scale, pi) in scales.iter().zip(&probabilities) {
        if pi.lo <= 0.0 {
            return Ok(None);
        }
        let atom_cap = up(*scale / pi.lo);
        bound = bound.max(atom_cap);
    }
    Ok(Some(RegimePlan {
        bound,
        states,
        complete: true,
        pivot,
        selected: selected.to_vec(),
        weights: scales,
        atoms: retained,
        forest: None,
        forest_atoms: Vec::new(),
        pattern_count: 0,
    }))
}

const COMPACT_PATTERN_CAP: usize = 4096;

#[derive(Clone)]
struct PivotPatterns {
    pivot: usize,
    incident: Vec<usize>,
    patterns: Vec<Vec<u8>>,
    count: usize,
    tied: usize,
    strict: usize,
    complete: bool,
}

fn enumerate_pivot_patterns(
    model: &Model,
    laws: &[Law],
    masks: &[u8],
    games: &[RankGame],
    packed_base: &[i32],
    target_packed: i32,
    pivot: usize,
    deadline: Instant,
) -> PivotPatterns {
    let incident: Vec<usize> = model
        .fixtures
        .iter()
        .enumerate()
        .filter_map(|(i, f)| (f.home == pivot || f.away == pivot).then_some(i))
        .collect();
    let options: Vec<Vec<u8>> = incident
        .iter()
        .map(|&fi| {
            (0..3)
                .filter(|&o| masks[fi] & (1 << o) != 0 && laws[fi].outcomes[o].hi > 0.0)
                .map(|o| o as u8)
                .collect()
        })
        .collect();
    let mut out = PivotPatterns {
        pivot,
        incident,
        patterns: Vec::new(),
        count: 0,
        tied: 0,
        strict: 0,
        complete: true,
    };
    if options.iter().any(Vec::is_empty) {
        return out;
    }
    let mut suffix = vec![0i32; out.incident.len() + 1];
    for pos in (0..out.incident.len()).rev() {
        let fi = out.incident[pos];
        let f = &model.fixtures[fi];
        suffix[pos] = suffix[pos + 1]
            + options[pos]
                .iter()
                .map(|&o| {
                    if f.home == pivot {
                        games[fi].hg[o as usize]
                    } else {
                        games[fi].ag[o as usize]
                    }
                })
                .max()
                .unwrap_or(0);
    }
    fn visit(
        pos: usize,
        value: i32,
        pivot: usize,
        incident: &[usize],
        options: &[Vec<u8>],
        games: &[RankGame],
        suffix: &[i32],
        target: i32,
        prefix: &mut Vec<u8>,
        out: &mut PivotPatterns,
        deadline: Instant,
    ) {
        if Instant::now() >= deadline {
            out.complete = false;
            return;
        }
        if value + suffix[pos] < target {
            return;
        }
        if pos == incident.len() {
            out.count += 1;
            if value == target {
                out.tied += 1;
            } else {
                out.strict += 1;
            }
            if out.patterns.len() < COMPACT_PATTERN_CAP {
                out.patterns.push(prefix.clone());
            }
            if out.count > COMPACT_PATTERN_CAP {
                out.complete = false;
            }
            return;
        }
        let fi = incident[pos];
        let f = &games[fi];
        for &o in &options[pos] {
            prefix.push(o);
            let gain = if f.home == pivot {
                f.hg[o as usize]
            } else {
                f.ag[o as usize]
            };
            visit(
                pos + 1,
                value + gain,
                pivot,
                incident,
                options,
                games,
                suffix,
                target,
                prefix,
                out,
                deadline,
            );
            prefix.pop();
            if !out.complete {
                return;
            }
        }
    }
    let incident = out.incident.clone();
    visit(
        0,
        packed_base[pivot],
        pivot,
        &incident,
        &options,
        games,
        &suffix,
        target_packed,
        &mut Vec::new(),
        &mut out,
        deadline,
    );
    out
}

struct CompactAttempt {
    plan: Option<RegimePlan>,
    reason: String,
    pattern_count: usize,
    tied: usize,
    strict: usize,
    forest_states: usize,
    forest_complete: bool,
    gd_plans_built: usize,
    gd_normalizers_computed: usize,
    gd_zero: usize,
    q_floor_rejected: usize,
    q_min: Option<f64>,
    q_max: Option<f64>,
    pair_setup_ms: f64,
}

#[allow(clippy::too_many_arguments)]
fn compact_candidate_for_pivot(
    model: &Model,
    laws: &[Law],
    gd_cache: &mut GdCache,
    masks: &[u8],
    lower: &[i32],
    upper: &[i32],
    selected: &[usize],
    stride: i32,
    target: usize,
    target_packed: i32,
    mut patterns: PivotPatterns,
    state_cap: usize,
    deadline: Instant,
) -> Result<CompactAttempt, String> {
    let pivot = patterns.pivot;
    let count = patterns.count;
    let tied = patterns.tied;
    let strict = patterns.strict;
    let mut gd_plans_built = 0usize;
    let mut gd_normalizers_computed = 0usize;
    let mut gd_zero = 0usize;
    let mut q_floor_rejected = 0usize;
    let mut q_min: Option<f64> = None;
    let mut q_max: Option<f64> = None;
    let mut pair_setup_ms = 0.0;
    macro_rules! reject {
        ($reason:expr,$states:expr,$complete:expr) => {
            return Ok(CompactAttempt {
                plan: None,
                reason: $reason.into(),
                pattern_count: count,
                tied,
                strict,
                forest_states: $states,
                forest_complete: $complete,
                gd_plans_built,
                gd_normalizers_computed,
                gd_zero,
                q_floor_rejected,
                q_min,
                q_max,
                pair_setup_ms,
            })
        };
    }
    if !patterns.complete || count > COMPACT_PATTERN_CAP {
        reject!("pattern_limit_or_deadline", 0, false);
    }
    if count == 0 {
        reject!("no_compatible_pattern", 0, true);
    }
    if tied == 0 {
        reject!("no_pw_tied_atom", 0, true);
    }
    if Instant::now() >= deadline {
        reject!("deadline_before_pair_setup", 0, false);
    }

    let mut prepared_prefixes = Vec::with_capacity(patterns.patterns.len());
    let mut pair_plans = Vec::with_capacity(patterns.patterns.len());
    let mut qs = Vec::with_capacity(patterns.patterns.len());
    let pair_started = Instant::now();
    for prefix in patterns.patterns.drain(..) {
        if Instant::now() >= deadline {
            pair_setup_ms = pair_started.elapsed().as_secs_f64() * 1000.0;
            reject!("deadline_during_pair_setup", 0, false);
        }
        let pw = model.base[pivot].points * stride
            + model.base[pivot].wins
            + patterns
                .incident
                .iter()
                .zip(&prefix)
                .map(|(&fi, &o)| {
                    let f = &model.fixtures[fi];
                    if f.home == pivot {
                        games_gain(model, fi, o as usize, true, stride)
                    } else {
                        games_gain(model, fi, o as usize, false, stride)
                    }
                })
                .sum::<i32>();
        let (pair, q) = if pw == target_packed {
            let mut child_masks = masks.to_vec();
            for (&fi, &o) in patterns.incident.iter().zip(&prefix) {
                child_masks[fi] = 1 << o;
            }
            let Some(q) =
                gd_cache.probability(model, laws, &child_masks, pivot, target, deadline)?
            else {
                pair_setup_ms = pair_started.elapsed().as_secs_f64() * 1000.0;
                reject!("pair_gd_normalizer_unsupported", 0, false);
            };
            gd_normalizers_computed += 1;
            if q.hi == 0.0 {
                gd_zero += 1;
                continue;
            }
            q_min = Some(q_min.map_or(q.lo, |v| v.min(q.lo)));
            q_max = Some(q_max.map_or(q.hi, |v| v.max(q.hi)));
            if q.lo < 0.001 {
                q_floor_rejected += 1;
                pair_setup_ms = pair_started.elapsed().as_secs_f64() * 1000.0;
                reject!("pair_q_cost_floor", 0, false);
            }
            let Some(pair) =
                PairPlan::build(model, laws, &child_masks, pivot, target, deadline, gd_cache)?
            else {
                pair_setup_ms = pair_started.elapsed().as_secs_f64() * 1000.0;
                reject!("pair_plan_unsupported_after_normalizer", 0, false);
            };
            gd_plans_built += 1;
            (Some(pair), q)
        } else {
            (None, Interval::point(1.0))
        };
        prepared_prefixes.push(prefix);
        pair_plans.push(pair);
        qs.push(q);
    }
    pair_setup_ms = pair_started.elapsed().as_secs_f64() * 1000.0;
    if prepared_prefixes.is_empty() {
        let mut forest_selected = selected.to_vec();
        if !forest_selected.contains(&pivot) {
            forest_selected.push(pivot);
        }
        let plan = RegimePlan {
            bound: 0.0,
            states: 0,
            complete: true,
            pivot,
            selected: forest_selected,
            weights: Vec::new(),
            atoms: Vec::new(),
            forest: None,
            forest_atoms: Vec::new(),
            pattern_count: count,
        };
        return Ok(CompactAttempt {
            plan: Some(plan),
            reason: "all_tied_atoms_certified_zero".into(),
            pattern_count: count,
            tied,
            strict,
            forest_states: 0,
            forest_complete: true,
            gd_plans_built,
            gd_normalizers_computed,
            gd_zero,
            q_floor_rejected,
            q_min,
            q_max,
            pair_setup_ms,
        });
    }

    let mut forest_selected = selected.to_vec();
    if !forest_selected.contains(&pivot) {
        forest_selected.push(pivot);
    }
    let target_incident = model
        .fixtures
        .iter()
        .filter(|f| f.home == target || f.away == target)
        .count();
    let pair_fixture_cost = patterns.incident.len().saturating_add(target_incident);
    let pair_cost = pair_plans
        .iter()
        .filter(|p| p.is_some())
        .count()
        .saturating_mul(pair_fixture_cost);
    let prefix_cost = prepared_prefixes
        .len()
        .saturating_mul(patterns.incident.len().saturating_add(1))
        .saturating_add(pair_cost);
    if prefix_cost >= state_cap {
        reject!("prefix_metadata_state_budget", 0, false);
    }
    let forest_cap = state_cap - prefix_cost;
    let forest = match Forest::build(
        model,
        laws,
        masks,
        lower,
        upper,
        &forest_selected,
        stride,
        &patterns.incident,
        &prepared_prefixes,
        forest_cap,
        deadline,
    ) {
        Ok(f) => f,
        Err(e) => reject!(format!("forest_build_error:{e}"), 0, false),
    };
    if forest.roots.len() != prepared_prefixes.len() {
        reject!("forest_root_count_mismatch", forest.states, forest.complete);
    }
    let mut forest_atoms = Vec::new();
    let mut scales = Vec::new();
    for (i, root) in forest.roots.iter().enumerate() {
        if root.zero {
            continue;
        }
        if root.prefix_mass.hi == 0.0 {
            continue;
        }
        let q = qs[i];
        if q.hi == 0.0 {
            continue;
        }
        let nominal = root.prefix_mass.mid() * q.mid() * root.potential;
        let nominal =
            if nominal == 0.0 && root.prefix_mass.hi > 0.0 && q.hi > 0.0 && root.potential > 0.0 {
                f64::from_bits(1)
            } else {
                nominal
            };
        if nominal <= 0.0 {
            reject!(
                "positive_root_has_no_mixture_ticket",
                forest.states,
                forest.complete
            );
        }
        forest_atoms.push(ForestAtom {
            root_index: i,
            pair: pair_plans[i].take(),
            q,
        });
        scales.push(nominal);
    }
    if forest_atoms.is_empty() {
        let forest_states = forest.states;
        let plan = RegimePlan {
            bound: 0.0,
            states: prefix_cost.saturating_add(forest_states),
            complete: true,
            pivot,
            selected: forest_selected,
            weights: Vec::new(),
            atoms: Vec::new(),
            forest: Some(forest),
            forest_atoms,
            pattern_count: count,
        };
        return Ok(CompactAttempt {
            plan: Some(plan),
            reason: "all_roots_certified_zero".into(),
            pattern_count: count,
            tied,
            strict,
            forest_states,
            forest_complete: true,
            gd_plans_built,
            gd_normalizers_computed,
            gd_zero,
            q_floor_rejected,
            q_min,
            q_max,
            pair_setup_ms,
        });
    }
    let probs = match categorical_probabilities(&scales) {
        Ok(p) => p,
        Err(e) => reject!(
            format!("mixture_ticket_unsupported:{e}"),
            forest.states,
            forest.complete
        ),
    };
    let mut bound = 0.0f64;
    for ((atom, pi), scale) in forest_atoms.iter().zip(&probs).zip(&scales) {
        if pi.lo <= 0.0 {
            reject!("zero_mixture_ticket_lower", forest.states, forest.complete);
        }
        let root = &forest.roots[atom.root_index];
        let cap = root
            .prefix_mass
            .mul(atom.q)
            .mul(Interval::point(root.cap))
            .div(*pi)
            .hi;
        if !cap.is_finite() {
            reject!("nonfinite_mixture_cap", forest.states, forest.complete);
        }
        bound = bound.max(cap);
        let _ = scale;
    }
    let states = prefix_cost.saturating_add(forest.states);
    if states > state_cap {
        reject!("shared_state_budget", forest.states, forest.complete);
    }
    let forest_complete = forest.complete;
    let forest_states = forest.states;
    let plan = RegimePlan {
        bound,
        states,
        complete: true,
        pivot,
        selected: forest_selected,
        weights: scales,
        atoms: Vec::new(),
        forest: Some(forest),
        forest_atoms,
        pattern_count: count,
    };
    Ok(CompactAttempt {
        plan: Some(plan),
        reason: "complete_partition".into(),
        pattern_count: count,
        tied,
        strict,
        forest_states,
        forest_complete,
        gd_plans_built,
        gd_normalizers_computed,
        gd_zero,
        q_floor_rejected,
        q_min,
        q_max,
        pair_setup_ms,
    })
}

fn games_gain(model: &Model, _fi: usize, outcome: usize, home: bool, stride: i32) -> i32 {
    let pw = model.request.phase.championship.point_win;
    let pd = model.request.phase.championship.point_draw;
    let (points, win) = match (home, outcome) {
        (true, 0) => (model.request.phase.championship.point_loss, false),
        (true, 1) => (pd, false),
        (true, 2) => (pw, true),
        (false, 0) => (pw, true),
        (false, 1) => (pd, false),
        (false, 2) => (model.request.phase.championship.point_loss, false),
        _ => unreachable!(),
    };
    points * stride + i32::from(win)
}

impl Engine {
    pub fn regime(&mut self, id: usize, state_cap: usize, time_ms: u64) -> Result<Value, String> {
        let start = Instant::now();
        if id >= self.regions.len() {
            return Err("unknown region id".into());
        }
        if state_cap == 0 {
            return Err("state_cap must be positive".into());
        }
        let deadline = start + std::time::Duration::from_millis(time_ms);
        let effective_before = {
            let r = &self.regions[id];
            r.regime
                .as_ref()
                .map_or_else(|| r.plan.as_ref().map_or(1.0, |p| p.bound), |p| p.bound)
        };
        let raw_states = self.regions[id].plan.as_ref().map_or(0, |p| p.states);
        if raw_states > state_cap {
            return Ok(
                json!({"accepted":false,"B":effective_before,"candidate_B":null,"states":raw_states,
                "atoms":0,"atom_count":0,"pivot_id":null,"selected_rival_ids":[],"mode":"joint",
                "complete":false,"setup_ms":start.elapsed().as_secs_f64()*1000.0,"epoch":self.regions[id].epoch,"candidates":[]}),
            );
        }
        let child_state_cap = state_cap.saturating_sub(raw_states);
        let (candidate, candidate_receipts) = {
            let region = &self.regions[id];
            let model = &self.model;
            let laws = &self.laws;
            let stride = self.packed_stride();
            let packed_base: Vec<i32> = model
                .base
                .iter()
                .map(|c| c.points * stride + c.wins)
                .collect();
            let games = make_games(model, laws, stride);
            let rivals: Vec<usize> = (0..model.n).filter(|&t| t != self.target).collect();
            let selected = region
                .plan
                .as_ref()
                .map(|p| p.selected.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| vec![self.target]);
            let target_packed = region.lower[self.target];
            let mut best: Option<RegimePlan> = None;
            let mut receipts = Vec::new();
            let inventories: Vec<PivotPatterns> = region
                .above
                .iter()
                .map(|&pivot| {
                    enumerate_pivot_patterns(
                        model,
                        laws,
                        &region.masks,
                        &games,
                        &packed_base,
                        target_packed,
                        pivot,
                        deadline,
                    )
                })
                .collect();

            // Retain the established path and region order for partitions
            // within the legacy atom cap.
            for inv in inventories
                .iter()
                .filter(|p| p.complete && p.count <= 64 && p.tied > 0)
            {
                if Instant::now() >= deadline {
                    break;
                }
                let pivot = inv.pivot;
                match candidate_for_pivot(model,laws,&mut self.gd_cache,&region.masks,&region.lower,
                    &region.upper_bounds,&rivals,self.rank,self.target,target_packed,pivot,&selected,
                    &games,&packed_base,stride,child_state_cap,deadline) {
                    Ok(Some(mut plan)) => {
                        plan.pattern_count=inv.count;
                        receipts.push(json!({"pivot_id":model.ids[pivot],"B":plan.bound,
                            "states":raw_states.saturating_add(plan.states),"atom_count":plan.atom_count(),
                            "pattern_count":inv.count,"tie_count":inv.tied,"strict_count":inv.strict,
                            "complete":plan.complete,"forest_complete":true,"path":"per_atom"}));
                        if best.as_ref().map_or(true,|b|plan.bound<b.bound) { best=Some(plan); }
                    }
                    Ok(None) => receipts.push(json!({"pivot_id":model.ids[pivot],"pattern_count":inv.count,
                        "tie_count":inv.tied,"strict_count":inv.strict,"complete":false,"reason":"per_atom_setup_declined","path":"per_atom"})),
                    Err(e) => return Err(e),
                }
            }

            // The forest shares its suffix DP across all fixed incident WDL
            // prefixes. Try the lightest partitions first so a modest viable
            // pivot is not starved by a later high-degree pivot.
            let mut compact: Vec<PivotPatterns> = inventories
                .iter()
                .filter(|p| {
                    p.complete && p.count > 64 && p.count <= COMPACT_PATTERN_CAP && p.tied > 0
                })
                .cloned()
                .collect();
            compact.sort_by_key(|p| (p.count, model.ids[p.pivot]));
            for inv in compact {
                if Instant::now() >= deadline {
                    break;
                }
                let pivot = inv.pivot;
                let attempt = compact_candidate_for_pivot(
                    model,
                    laws,
                    &mut self.gd_cache,
                    &region.masks,
                    &region.lower,
                    &region.upper_bounds,
                    &selected,
                    stride,
                    self.target,
                    target_packed,
                    inv,
                    child_state_cap,
                    deadline,
                )?;
                if let Some(plan) = attempt.plan {
                    receipts.push(json!({"pivot_id":model.ids[pivot],"B":plan.bound,
                        "states":raw_states.saturating_add(plan.states),"atom_count":plan.atom_count(),
                        "pattern_count":attempt.pattern_count,"tie_count":attempt.tied,
                        "strict_count":attempt.strict,"forest_states":attempt.forest_states,
                        "forest_complete":attempt.forest_complete,"complete":plan.complete,
                        "gd_plans_built":attempt.gd_plans_built,"gd_normalizers_computed":attempt.gd_normalizers_computed,
                        "gd_zero":attempt.gd_zero,"q_floor_rejected":attempt.q_floor_rejected,
                        "q_min":attempt.q_min,"q_max":attempt.q_max,"pair_setup_ms":attempt.pair_setup_ms,
                        "reason":attempt.reason,"path":"shared_suffix_forest"}));
                    if best.as_ref().map_or(true, |b| plan.bound < b.bound) {
                        best = Some(plan);
                    }
                } else {
                    receipts.push(json!({"pivot_id":model.ids[pivot],"pattern_count":attempt.pattern_count,
                        "tie_count":attempt.tied,"strict_count":attempt.strict,"forest_states":attempt.forest_states,
                        "forest_complete":attempt.forest_complete,"complete":false,"reason":attempt.reason,
                        "gd_plans_built":attempt.gd_plans_built,"gd_normalizers_computed":attempt.gd_normalizers_computed,
                        "gd_zero":attempt.gd_zero,"q_floor_rejected":attempt.q_floor_rejected,
                        "q_min":attempt.q_min,"q_max":attempt.q_max,"pair_setup_ms":attempt.pair_setup_ms,
                        "path":"shared_suffix_forest"}));
                }
            }
            for inv in inventories.iter().filter(|p| {
                !p.complete || p.count == 0 || p.tied == 0 || p.count > COMPACT_PATTERN_CAP
            }) {
                let reason = if !inv.complete {
                    if inv.count > COMPACT_PATTERN_CAP {
                        "pattern_limit"
                    } else {
                        "pattern_scan_deadline"
                    }
                } else if inv.count == 0 {
                    "no_compatible_pattern"
                } else if inv.tied == 0 {
                    "no_pw_tied_atom"
                } else {
                    "pattern_limit"
                };
                receipts.push(json!({"pivot_id":model.ids[inv.pivot],
                    "pattern_count":if inv.count>COMPACT_PATTERN_CAP {format!(">{COMPACT_PATTERN_CAP}")} else {inv.count.to_string()},
                    "tie_count":inv.tied,"strict_count":inv.strict,"complete":false,"reason":reason,"path":"not_attempted"}));
            }
            (best, receipts)
        };
        let Some(candidate) = candidate else {
            return Ok(
                json!({"accepted":false,"B":effective_before,"candidate_B":null,"states":raw_states,"atoms":0,"atom_count":0,"pivot_id":null,"selected_rival_ids":[],"mode":if self.regions[id].regime.is_some(){"regime"}else{"joint"},"complete":false,"setup_ms":start.elapsed().as_secs_f64()*1000.0,"epoch":self.regions[id].epoch,"candidates":candidate_receipts}),
            );
        };
        let improved = candidate.bound <= 0.75 * effective_before;
        let states = raw_states.saturating_add(candidate.states);
        let atoms = candidate.atom_count();
        let pivot_id = self.model.ids[candidate.pivot];
        let selected_rival_ids = candidate
            .selected
            .iter()
            .filter(|&&t| t != self.target)
            .map(|&t| self.model.ids[t])
            .collect::<Vec<_>>();
        if improved {
            self.regions[id].epoch = self.regions[id].epoch.saturating_add(1);
            if candidate.bound == 0.0 {
                self.regions[id].impossible = true;
                self.regions[id].upper = 0.0;
            }
            self.regions[id].regime = Some(candidate);
        }
        Ok(
            json!({"accepted":improved,"B":if improved {self.regions[id].regime.as_ref().unwrap().bound} else {effective_before},
            "candidate_B":candidate_receipts.iter().map(|c|c["B"].as_f64().unwrap_or(f64::INFINITY)).fold(f64::INFINITY,f64::min),
            "states":states,"atoms":atoms,"atom_count":atoms,"pivot_id":pivot_id,"selected_rival_ids":selected_rival_ids,
            "mode":if improved {"regime"}else if self.regions[id].regime.is_some(){"regime"}else{"joint"},
            "complete":true,"setup_ms":start.elapsed().as_secs_f64()*1000.0,"epoch":self.regions[id].epoch,"candidates":candidate_receipts}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use golaberto_odds::model::{Model, Request};
    use std::time::Duration;

    fn tiny() -> Model {
        let request: Request = serde_json::from_str(
            r#"{"id":1,"phase":{"sort":"pt,w,gd,gf,name","championship":{"point_win":3,"point_draw":1,"point_loss":0}},"games":[{"id":1,"home_id":1,"away_id":2,"home_power":1.0,"away_power":1.0,"played":false}],"team_groups":[{"team_id":1},{"team_id":2}]}"#,
        ).unwrap();
        Model::new(request).unwrap()
    }

    #[test]
    fn nonuniform_atom_mixture_identity_and_cap() {
        let model = tiny();
        let law = Law::new(1.0, 1.0).unwrap();
        let laws = vec![law.clone()];
        let lower = vec![i32::MIN / 4; model.n];
        let upper = vec![i32::MAX / 4; model.n];
        let stride = 16;
        let deadline = Instant::now() + Duration::from_secs(1);
        let left = JointPlan::build(
            &model,
            &laws,
            &[1 << 1],
            &lower,
            &upper,
            &[],
            stride,
            0,
            deadline,
        )
        .unwrap();
        let right = JointPlan::build(
            &model,
            &laws,
            &[1 << 2],
            &lower,
            &upper,
            &[],
            stride,
            0,
            deadline,
        )
        .unwrap();
        let scales = vec![left.bound, right.bound];
        let pi = categorical_probabilities(&scales).unwrap();
        assert!(
            pi[0].mid() != pi[1].mid(),
            "the mixture fixture must be nonuniform"
        );
        let bound = up((0..2).map(|i| scales[i] / pi[i].lo).fold(0.0, f64::max));
        assert!(scales[0] / pi[0].lo <= bound && scales[1] / pi[1].lo <= bound);
        let plans = RegimePlan {
            bound,
            states: 0,
            complete: true,
            pivot: 0,
            selected: Vec::new(),
            weights: scales.clone(),
            atoms: vec![
                Atom {
                    plan: left,
                    pair: None,
                    q: Interval::point(1.0),
                },
                Atom {
                    plan: right,
                    pair: None,
                    q: Interval::point(1.0),
                },
            ],
            forest: None,
            forest_atoms: Vec::new(),
            pattern_count: 2,
        };
        // Exact mixture expectation for the union of the two atom events
        // recovers their native-law mass, with the ticket-probability interval
        // canceling between mixture selection and likelihood correction.
        let expected = pi[0]
            .mul(law.outcomes[1].div(pi[0]))
            .add(pi[1].mul(law.outcomes[2].div(pi[1])));
        let native = law.outcomes[1].add(law.outcomes[2]);
        assert!(expected.lo <= native.lo && expected.hi >= native.hi);
        let mut rng = Rng::new(701);
        for _ in 0..32 {
            let (outcomes, weight, scores) = plans.draw(&model, &laws, &mut rng).unwrap();
            assert!(weight.lo >= 0.0 && weight.lo <= bound);
            assert!(scores.iter().all(Option::is_none));
            assert!(outcomes[0] == 1 || outcomes[0] == 2);
        }
    }

    #[test]
    fn compact_forest_mixture_preserves_tied_and_strict_support() {
        let request: Request = serde_json::from_str(include_str!(
            "../../../../experiments/rare_positions/reference/2026-10-04-two-results/inputs/group-16498-two-results.json"
        )).unwrap();
        let mut engine = Engine::new(request, 16, 16).unwrap();
        let pivot = *engine.model.indices.get(&584).unwrap();
        let id = 5;
        assert!(engine.regions[id].above.contains(&pivot));
        engine.regions[id].above = vec![pivot];
        engine.prepare(id, 7, 1000, 100).unwrap();
        let receipt = engine.regime(id, 250_000, 10_000).unwrap();
        assert_eq!(receipt["accepted"], true, "{receipt}");
        let plan = engine.regions[id].regime.as_ref().unwrap();
        assert_eq!(plan.pivot, pivot);
        assert_eq!(plan.pattern_count, 156);
        assert_eq!(plan.atom_count(), 156);
        let forest = plan.forest.as_ref().unwrap();
        assert!(forest
            .roots
            .iter()
            .any(|r| r.prefix_mass.hi > 0.0 && r.potential > 0.0));
        let mut has_pair = false;
        let mut has_strict = false;
        for atom in &plan.forest_atoms {
            if atom.pair.is_some() {
                has_pair = true;
            } else {
                has_strict = true;
            }
        }
        assert!(
            has_pair && has_strict,
            "mixture must retain tied and strict PW branches"
        );
        let mut legacy_rng = Rng::new(9123);
        let mut hybrid_rng = Rng::new(9123);
        for _ in 0..24 {
            let (legacy_outcomes, legacy_weight, legacy_scores) = plan
                .draw(&engine.model, &engine.laws, &mut legacy_rng)
                .unwrap();
            let (hybrid_outcomes, hybrid_weight, guide_ratio, hybrid_scores) = plan
                .draw_hybrid(&engine.model, &engine.laws, &mut hybrid_rng, false)
                .unwrap();
            assert_eq!(hybrid_outcomes, legacy_outcomes);
            assert_eq!(hybrid_scores, legacy_scores);
            assert_eq!(hybrid_weight, legacy_weight);
            assert!(guide_ratio.lo >= 0.0 && guide_ratio.hi.is_finite());
        }

        // The ticket probabilities cancel once against the native prefix
        // mass, the suffix estimator and the pair-GD normalizer. This checks
        // the actual compact mixture scales and interval tickets, rather than
        // the independent per-atom proposal path.
        let tickets = categorical_probabilities(&plan.weights).unwrap();
        let mut mixture = Interval::point(0.0);
        let mut native = Interval::point(0.0);
        for (i, atom) in plan.forest_atoms.iter().enumerate() {
            let root = &forest.roots[atom.root_index];
            let contribution = root
                .prefix_mass
                .mul(Interval::point(root.potential))
                .mul(atom.q);
            native = native.add(contribution);
            mixture = mixture.add(
                tickets[i].mul(
                    root.prefix_mass
                        .div(tickets[i])
                        .mul(Interval::point(root.potential))
                        .mul(atom.q),
                ),
            );
        }
        assert!(mixture.lo <= native.lo && mixture.hi >= native.hi);
        let mut rng = Rng::new(584_156);
        for _ in 0..32 {
            let (outcomes, weight, fixed) =
                plan.draw(&engine.model, &engine.laws, &mut rng).unwrap();
            assert!(weight.lo >= 0.0 && weight.hi <= plan.bound);
            assert!(outcomes
                .iter()
                .enumerate()
                .all(|(fi, &o)| engine.regions[id].masks[fi] & (1 << o) != 0));
            if fixed.iter().any(Option::is_some) {
                for (fi, score) in fixed
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| s.map(|v| (i, v)))
                {
                    let observed = if score[0] < score[1] {
                        0
                    } else if score[0] == score[1] {
                        1
                    } else {
                        2
                    };
                    assert_eq!(outcomes[fi], observed);
                }
            }
        }

        // Exercise the complete core, residual and baseline loops with the
        // installed compact forest. Both modes must retain the same sampled
        // roots, prefix weights and fixed pair scores on a shared seed.
        let eager = engine
            .sample_hybrid_with_options(id, 16, 584_156, 0, 0.25, false)
            .unwrap();
        let lazy = engine
            .sample_hybrid_with_options(id, 16, 584_156, 0, 0.25, true)
            .unwrap();
        for stream in ["core", "residual", "baseline"] {
            assert_eq!(eager[stream], lazy[stream], "forest stream mismatch in {stream}");
        }
        assert_eq!(lazy["residual_guide_evaluations"].as_u64().unwrap()
            + lazy["residual_guide_skips"].as_u64().unwrap(), 8);

        let epoch = engine.regions[id].epoch;
        let audit = engine.residual_certificate(id, 0.25, 500, 1).unwrap();
        assert_eq!(audit["source_count"], 156);
        assert_eq!(audit["epoch"], epoch);
        assert!(audit["expectation_upper"].as_f64().unwrap() <= audit["B"].as_f64().unwrap());
        assert_eq!(engine.regions[id].epoch, epoch);
        let suffix_audit = engine.residual_certificate_with_options(
            id,
            0.25,
            500,
            1,
            Some(SuffixOptions {
                state_cap: 100,
                max_remaining: 1,
                probe_nodes: 50,
                probe_ms: 1,
                budget_fraction: 0.1,
                canonicalize_certain: true,
            }),
        ).unwrap();
        assert_eq!(suffix_audit["suffix"]["enabled"], true);
        assert_eq!(suffix_audit["suffix"]["canonicalization_enabled"], true);
        assert_eq!(suffix_audit["source_count"], 156);
        assert_eq!(engine.regions[id].epoch, epoch);
    }
}
