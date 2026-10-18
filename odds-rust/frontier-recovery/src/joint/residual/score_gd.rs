//! Optional conditional goal-difference moment bounds for packed-score ties.
//!
//! The bound is an expectation bound over the source's residual outcome law.
//! It is deliberately never used as a pointwise likelihood-ratio cap.

use super::ResidualSource;
use crate::joint::{Interval, Law, Plan};
use golaberto_odds::model::{Key, Model};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

const BASES: [(u64, u64); 3] = [(9, 8), (5, 4), (3, 2)];
const SUPPORT_LIMIT: usize = 2_000;
const WORK_LIMIT: usize = 100_000_000;

#[derive(Clone, Debug, Default, Serialize)]
pub struct ScoreDiagnostics {
    pub enabled: bool,
    pub setup_ms: f64,
    pub sources_total: usize,
    pub prepared_sources: usize,
    pub skipped_sources: usize,
    pub budget_exhausted: bool,
    pub work_limit_hit: bool,
    pub support_limit_hit: bool,
    pub invalid_sources: usize,
    pub evaluations: usize,
    pub forced_tie_evaluations: usize,
    pub tightened_bounds: usize,
    pub min_factor: Option<f64>,
    pub selected_rivals: BTreeMap<i32, usize>,
    pub setup_work: usize,
}

#[derive(Clone)]
pub(super) struct Prepared {
    pub(super) rival: usize,
    pub(super) above: bool,
    pub(super) packed_target: i64,
    pub(super) rival_lower: i64,
    pub(super) rival_upper: i64,
    pub(super) suffix_min: Vec<Option<i64>>,
    pub(super) suffix_max: Vec<Option<i64>>,
    pub(super) steps: Vec<Option<[[f64; 3]; 3]>>,
    pub(super) suffix_moments: Vec<[f64; 3]>,
    pub(super) initial: [f64; 3],
    pub(super) support_hit: bool,
    pub(super) score_expectation_eligible: bool,
}

/// Immutable score-moment data borrowed by the score-aware expectation cache.
#[derive(Clone, Copy, Debug)]
pub(super) struct MomentKernel<'a> {
    pub(super) rival: usize,
    pub(super) above: bool,
    pub(super) packed_target: i64,
    pub(super) initial: [f64; 3],
    pub(super) steps: &'a [Option<[[f64; 3]; 3]>],
    pub(super) suffix_moments: &'a [[f64; 3]],
    pub(super) identity: usize,
}

pub(super) struct ScoreBounder {
    pub(super) rows: Vec<Option<Prepared>>,
    diagnostics: ScoreDiagnostics,
}

#[path = "selector.rs"]
mod selector;
pub(super) use selector::{BranchSelector, Selection};
pub use selector::SelectorDiagnostics;

impl ScoreBounder {
    pub fn disabled() -> Self {
        Self {
            rows: Vec::new(),
            diagnostics: ScoreDiagnostics::default(),
        }
    }

    pub fn prepare(
        sources: &[ResidualSource<'_>],
        model: &Model,
        laws: &[Law],
        setup_ms: u64,
        deadline: Instant,
    ) -> Self {
        let started = Instant::now();
        let budget_deadline = started
            .checked_add(Duration::from_millis(setup_ms))
            .unwrap_or(deadline)
            .min(deadline);
        let mut this = Self {
            rows: vec![None; sources.len()],
            diagnostics: ScoreDiagnostics {
                enabled: true,
                sources_total: sources.len(),
                ..Default::default()
            },
        };
        let mut priority: Vec<_> = sources.iter().enumerate().collect();
        priority.sort_by(|(ia, a), (ib, b)| {
            b.q0_prefix
                .hi
                .total_cmp(&a.q0_prefix.hi)
                .then_with(|| ia.cmp(ib))
        });
        let mut work = 0usize;
        for (index, source) in priority {
            if Instant::now() >= budget_deadline {
                this.diagnostics.budget_exhausted = true;
                break;
            }
            match prepare_one(source, model, laws, &mut work, budget_deadline, None) {
                Ok(Some(row)) => {
                    this.diagnostics.support_limit_hit |= row.support_hit;
                    *this
                        .diagnostics
                        .selected_rivals
                        .entry(model.ids[row.rival])
                        .or_default() += 1;
                    this.rows[index] = Some(row);
                    this.diagnostics.prepared_sources += 1;
                }
                Ok(None) => this.diagnostics.skipped_sources += 1,
                Err(PrepareFailure::Budget) => {
                    this.diagnostics.budget_exhausted = true;
                    break;
                }
                Err(PrepareFailure::Work) => {
                    this.diagnostics.work_limit_hit = true;
                    break;
                }
                Err(PrepareFailure::Support) => {
                    this.diagnostics.support_limit_hit = true;
                    this.diagnostics.skipped_sources += 1;
                }
                Err(PrepareFailure::Invalid) => {
                    this.diagnostics.invalid_sources += 1;
                    this.diagnostics.skipped_sources += 1;
                }
            }
        }
        this.diagnostics.skipped_sources = sources
            .len()
            .saturating_sub(this.diagnostics.prepared_sources);
        this.diagnostics.setup_work = work;
        this.diagnostics.setup_ms = started.elapsed().as_secs_f64() * 1000.0;
        this
    }

    pub fn advance(&self, source: usize, pos: usize, outcome: usize, prefix: [f64; 3]) -> [f64; 3] {
        let Some(row) = self.rows.get(source).and_then(Option::as_ref) else {
            return prefix;
        };
        let Some(Some(step)) = row.steps.get(pos) else {
            return prefix;
        };
        if outcome >= 3 {
            return [f64::INFINITY; 3];
        }
        std::array::from_fn(|j| safe_upper_product(prefix[j], step[outcome][j]))
    }

    pub fn factor(&mut self, source: usize, pos: usize, totals: &[i32], prefix: [f64; 3]) -> f64 {
        self.diagnostics.evaluations += 1;
        let Some(row) = self.rows.get(source).and_then(Option::as_ref) else {
            return 1.0;
        };
        let Some((min, max)) = packed_suffix_range(row, pos, totals) else {
            return 1.0;
        };
        let forced = if row.above {
            max <= row.packed_target && row.rival_lower >= row.packed_target
        } else {
            min >= row.packed_target && row.rival_upper <= row.packed_target
        };
        if !forced {
            return 1.0;
        }
        self.diagnostics.forced_tie_evaluations += 1;
        let Some(suffix) = row.suffix_moments.get(pos) else {
            return 1.0;
        };
        let mut bound = 1.0f64;
        for j in 0..3 {
            let value = safe_upper_product(row.initial[j], prefix[j]);
            let value = safe_upper_product(value, suffix[j]);
            if !value.is_finite() || value < 0.0 {
                return 1.0;
            }
            bound = bound.min(value);
        }
        if bound < 1.0 {
            self.diagnostics.tightened_bounds += 1;
            self.diagnostics.min_factor = Some(
                self.diagnostics
                    .min_factor
                    .map_or(bound, |old| old.min(bound)),
            );
        }
        bound.max(0.0)
    }

    pub fn diagnostics(&self) -> ScoreDiagnostics {
        self.diagnostics.clone()
    }

    pub fn moment_kernel(&self, source: usize) -> Option<MomentKernel<'_>> {
        let row = self.rows.get(source)?.as_ref()?;
        if !row.score_expectation_eligible {
            return None;
        }
        Some(MomentKernel {
            rival: row.rival,
            above: row.above,
            packed_target: row.packed_target,
            initial: row.initial,
            steps: &row.steps,
            suffix_moments: &row.suffix_moments,
            identity: row.steps.as_ptr() as usize,
        })
    }
}

pub(super) fn safe_upper_product(a: f64, b: f64) -> f64 {
    if a == 0.0 || b == 0.0 {
        return if a.is_finite() && b.is_finite() {
            0.0
        } else {
            f64::INFINITY
        };
    }
    if !a.is_finite() || !b.is_finite() || a < 0.0 || b < 0.0 {
        return f64::INFINITY;
    }
    let v = Interval::point(a).mul(Interval::point(b)).hi;
    if v.is_finite() && v >= 0.0 {
        v
    } else {
        f64::INFINITY
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum PrepareFailure {
    Budget,
    Work,
    Support,
    Invalid,
}

pub(super) fn prepare_one(
    source: &ResidualSource<'_>,
    model: &Model,
    laws: &[Law],
    work: &mut usize,
    deadline: Instant,
    requested_rival: Option<usize>,
) -> Result<Option<Prepared>, PrepareFailure> {
    if laws.len() != model.fixtures.len() || source.fixed_outcomes.len() != laws.len() {
        return Err(PrepareFailure::Invalid);
    }
    let Some(ranking) = source.score_ranking.as_ref() else {
        return Ok(None);
    };
    let plan = source.graph.plan();
    let target = ranking.target;
    let rules = &model.request.phase.championship;
    if target >= model.n
        || plan.stride <= 0
        || plan.lower[target] != plan.upper[target]
        || model.keys.get(..3) != Some(&[Key::Pt, Key::W, Key::Gd][..])
        || rules.point_win != 3
        || rules.point_draw != 1
        || rules.point_loss != 0
        || model.request.phase.bonus_points != 0
        || model.fixtures.iter().any(|f| f.home == f.away)
        || ranking
            .above
            .iter()
            .any(|&team| team >= model.n || team == target)
        || ranking.above.iter().collect::<BTreeSet<_>>().len() != ranking.above.len()
    {
        return Err(PrepareFailure::Invalid);
    }
    let packed_target = i64::from(plan.lower[target]);
    let pair_endpoints = ranking.pair_condition.map(|(a, b, _)| [a, b]);
    let mut candidates: Vec<usize> = (0..model.n)
        .filter(|&team| team != target)
        .filter(|team| !pair_endpoints.is_some_and(|pair| pair.contains(team)))
        .collect();
    candidates.sort_by(|&a, &b| {
        let (ap, an) = if ranking.above.contains(&a) {
            (a, target)
        } else {
            (target, a)
        };
        let (bp, bn) = if ranking.above.contains(&b) {
            (b, target)
        } else {
            (target, b)
        };
        let ga = i64::from(model.base[ap].gf)
            - i64::from(model.base[ap].ga)
            - i64::from(model.base[an].gf)
            + i64::from(model.base[an].ga);
        let gb = i64::from(model.base[bp].gf)
            - i64::from(model.base[bp].ga)
            - i64::from(model.base[bn].gf)
            + i64::from(model.base[bn].ga);
        ga.cmp(&gb).then_with(|| model.ids[a].cmp(&model.ids[b]))
    });
    let mut chosen = None;
    let mut candidate_support_hit = false;
    for &candidate in &candidates {
        if requested_rival.is_some_and(|requested| requested != candidate) {
            continue;
        }
        if ranking.above.contains(&candidate) {
            if i64::from(plan.lower[candidate]) != packed_target
                || i64::from(plan.upper[candidate]) < packed_target
            {
                continue;
            }
        } else if i64::from(plan.upper[candidate]) != packed_target
            || i64::from(plan.lower[candidate]) > packed_target
        {
            continue;
        }
        match packed_reachable(
            plan,
            model,
            candidate,
            *source
                .totals
                .get(candidate)
                .ok_or(PrepareFailure::Invalid)?,
            packed_target,
            work,
            deadline,
        )? {
            Some(true) => {
                chosen = Some(candidate);
                break;
            }
            Some(false) => continue,
            None => {
                candidate_support_hit = true;
                continue;
            }
        }
    }
    let Some(rival) = chosen else {
        return if candidate_support_hit {
            Err(PrepareFailure::Support)
        } else {
            Ok(None)
        };
    };
    let above = ranking.above.contains(&rival);
    let (b_positive, b_negative) = if above {
        (rival, target)
    } else {
        (target, rival)
    };
    let (mut initial, a, b, incident) = if let Some((aa, bb, q)) = ranking.pair_condition {
        if aa >= model.n || bb >= model.n || aa == bb || !valid_positive(q) {
            return Err(PrepareFailure::Invalid);
        }
        let a_team = aa;
        let b_team = bb;
        let mut incident = vec![false; model.fixtures.len()];
        for (fi, fixture) in model.fixtures.iter().enumerate() {
            if fixture.home == a_team
                || fixture.away == a_team
                || fixture.home == b_team
                || fixture.away == b_team
            {
                if source.fixed_outcomes[fi].is_none() {
                    return Ok(None);
                }
                incident[fi] = true;
            }
        }
        let gd_a = i64::from(model.base[a_team].gf) - i64::from(model.base[a_team].ga);
        let gd_b = i64::from(model.base[b_team].gf) - i64::from(model.base[b_team].ga);
        let base_state = gd_a.checked_sub(gd_b).ok_or(PrepareFailure::Invalid)?;
        let initial = pair_mgf(
            model, laws, source, a_team, b_team, b_positive, b_negative, base_state, &incident, q,
            work, deadline,
        )?;
        (initial, a_team, b_team, incident)
    } else {
        let base_b = (i64::from(model.base[b_positive].gf) - i64::from(model.base[b_positive].ga))
            .checked_sub(
                i64::from(model.base[b_negative].gf) - i64::from(model.base[b_negative].ga),
            )
            .ok_or(PrepareFailure::Invalid)?;
        let initial = std::array::from_fn(|j| pow_rational(BASES[j], base_b).hi);
        let incident = vec![false; model.fixtures.len()];
        (initial, b_positive, b_negative, incident)
    };
    if !multiply_known_outside_prefix(
        &mut initial,
        source,
        model,
        laws,
        plan,
        &incident,
        b_positive,
        b_negative,
        work,
        deadline,
    )? {
        return Ok(None);
    }
    let mut steps = vec![None; plan.order.len()];
    for (pos, &fi) in plan.order.iter().enumerate() {
        if incident[fi]
            || incidence(&model.fixtures[fi], b_positive)
                == incidence(&model.fixtures[fi], b_negative)
        {
            continue;
        }
        let f = &model.fixtures[fi];
        let mut row = [[1.0; 3]; 3];
        for o in 0..3 {
            if plan.masks[fi] & (1 << o) == 0 {
                row[o] = [0.0; 3];
            } else {
                row[o] =
                    conditional_moments(f, &laws[fi], b_positive, b_negative, o, work, deadline)?;
            }
        }
        steps[pos] = Some(row);
    }
    let mut suffix_moments = vec![[1.0; 3]; plan.order.len() + 1];
    for pos in (0..plan.order.len()).rev() {
        let mut row = suffix_moments[pos + 1];
        if let Some(Some(step)) = steps.get(pos) {
            for j in 0..3 {
                let allowed_max = (0..3)
                    .filter(|&o| plan.masks[plan.order[pos]] & (1 << o) != 0)
                    .map(|o| step[o][j])
                    .fold(0.0, f64::max);
                row[j] = safe_upper_product(row[j], allowed_max);
            }
        }
        suffix_moments[pos] = row;
    }
    let (suffix_min, suffix_max) = make_packed_suffix(plan, model, rival);
    let _ = (a, b);
    Ok(Some(Prepared {
        rival,
        above,
        packed_target,
        rival_lower: i64::from(plan.lower[rival]),
        rival_upper: i64::from(plan.upper[rival]),
        suffix_min,
        suffix_max,
        steps,
        suffix_moments,
        initial,
        support_hit: candidate_support_hit,
        score_expectation_eligible: ranking.pair_condition.is_none(),
    }))
}

fn multiply_known_outside_prefix(
    initial: &mut [f64; 3],
    source: &ResidualSource<'_>,
    model: &Model,
    laws: &[Law],
    plan: &Plan,
    incident_a: &[bool],
    b_positive: usize,
    b_negative: usize,
    work: &mut usize,
    deadline: Instant,
) -> Result<bool, PrepareFailure> {
    let in_plan: BTreeSet<_> = plan.order.iter().copied().collect();
    for (fi, outcome) in source.fixed_outcomes.iter().enumerate() {
        if in_plan.contains(&fi) || incident_a.get(fi).copied().unwrap_or(false) {
            continue;
        }
        let fixture = &model.fixtures[fi];
        if incidence(fixture, b_positive) == incidence(fixture, b_negative) {
            continue;
        }
        let Some(outcome) = outcome else {
            return Ok(false);
        };
        let moments = conditional_moments(
            fixture,
            &laws[fi],
            b_positive,
            b_negative,
            *outcome as usize,
            work,
            deadline,
        )?;
        for j in 0..3 {
            initial[j] = safe_upper_product(initial[j], moments[j]);
        }
    }
    Ok(true)
}

fn pair_mgf(
    model: &Model,
    laws: &[Law],
    source: &ResidualSource<'_>,
    a_team: usize,
    b_team: usize,
    b_positive: usize,
    b_negative: usize,
    base_state: i64,
    incident: &[bool],
    q: Interval,
    work: &mut usize,
    deadline: Instant,
) -> Result<[f64; 3], PrepareFailure> {
    let mut grouped = Vec::with_capacity(model.fixtures.len());
    for fi in 0..model.fixtures.len() {
        if !incident[fi] {
            grouped.push(None);
            continue;
        }
        let outcome = source.fixed_outcomes[fi].ok_or(PrepareFailure::Invalid)? as usize;
        if outcome >= 3 || !valid_positive(laws[fi].outcomes[outcome]) {
            return Err(PrepareFailure::Invalid);
        }
        let denom = laws[fi].outcomes[outcome];
        let mut groups = BTreeMap::<i64, Interval>::new();
        for pair in &laws[fi].pairs[outcome] {
            *work += 1;
            if *work > WORK_LIMIT {
                return Err(PrepareFailure::Work);
            }
            let gd = i64::from(pair.home)
                .checked_sub(i64::from(pair.away))
                .ok_or(PrepareFailure::Invalid)?;
            let mass = pair.mass.div(denom);
            let old = groups
                .get(&gd)
                .copied()
                .unwrap_or_else(|| Interval::point(0.0));
            groups.insert(gd, old.add(mass));
        }
        if groups.is_empty() {
            return Err(PrepareFailure::Invalid);
        }
        grouped.push(Some(groups));
    }
    let mut transitions: [Vec<Option<Vec<(i64, Interval)>>>; 3] =
        std::array::from_fn(|_| vec![None; model.fixtures.len()]);
    for j in 0..3 {
        for fi in 0..model.fixtures.len() {
            if !incident[fi] {
                continue;
            }
            if Instant::now() >= deadline {
                return Err(PrepareFailure::Budget);
            }
            let fixture = &model.fixtures[fi];
            let ca = incidence(fixture, a_team) - incidence(fixture, b_team);
            let cb = incidence(fixture, b_positive) - incidence(fixture, b_negative);
            let mut row =
                Vec::with_capacity(grouped[fi].as_ref().ok_or(PrepareFailure::Invalid)?.len());
            for (&gd, &mass) in grouped[fi].as_ref().ok_or(PrepareFailure::Invalid)? {
                *work += 1;
                if *work > WORK_LIMIT {
                    return Err(PrepareFailure::Work);
                }
                let delta_a = ca.checked_mul(gd).ok_or(PrepareFailure::Invalid)?;
                let delta_b = cb.checked_mul(gd).ok_or(PrepareFailure::Invalid)?;
                let weight = mass.mul(pow_rational(BASES[j], delta_b));
                if !weight.hi.is_finite() || weight.hi < 0.0 {
                    return Err(PrepareFailure::Invalid);
                }
                row.push((delta_a, weight));
            }
            transitions[j][fi] = Some(row);
        }
    }
    let mut result = [0.0; 3];
    for j in 0..3 {
        let z = BASES[j];
        let base_b = (i64::from(model.base[b_positive].gf) - i64::from(model.base[b_positive].ga))
            .checked_sub(
                i64::from(model.base[b_negative].gf) - i64::from(model.base[b_negative].ga),
            )
            .ok_or(PrepareFailure::Invalid)?;
        let mut dp = BTreeMap::<i64, Interval>::from([(base_state, pow_rational(z, base_b))]);
        for fi in 0..model.fixtures.len() {
            if !incident[fi] {
                continue;
            }
            if Instant::now() >= deadline {
                return Err(PrepareFailure::Budget);
            }
            let mut next = BTreeMap::<i64, Interval>::new();
            for (&state, &mass) in &dp {
                for &(delta_a, weight) in
                    transitions[j][fi].as_ref().ok_or(PrepareFailure::Invalid)?
                {
                    *work += 1;
                    if *work > WORK_LIMIT {
                        return Err(PrepareFailure::Work);
                    }
                    if *work % 4096 == 0 && Instant::now() >= deadline {
                        return Err(PrepareFailure::Budget);
                    }
                    let new_state = state.checked_add(delta_a).ok_or(PrepareFailure::Invalid)?;
                    let term = mass.mul(weight);
                    let old = next
                        .get(&new_state)
                        .copied()
                        .unwrap_or_else(|| Interval::point(0.0));
                    next.insert(new_state, old.add(term));
                    if next.len() > SUPPORT_LIMIT {
                        return Err(PrepareFailure::Support);
                    }
                }
            }
            dp = next;
        }
        let numerator = dp
            .iter()
            .filter(|(state, _)| **state >= 0)
            .fold(Interval::point(0.0), |sum, (_, mass)| sum.add(*mass));
        if numerator.hi <= 0.0 {
            return Err(PrepareFailure::Invalid);
        }
        let conditioned = numerator.div(q);
        result[j] = conditioned.hi;
        if !result[j].is_finite() || result[j] < 0.0 {
            return Err(PrepareFailure::Invalid);
        }
    }
    Ok(result)
}

fn pow_rational(base: (u64, u64), exponent: i64) -> Interval {
    let mut n = exponent.unsigned_abs();
    let atom = if exponent >= 0 {
        Interval::point(base.0 as f64).div(Interval::point(base.1 as f64))
    } else {
        Interval::point(base.1 as f64).div(Interval::point(base.0 as f64))
    };
    let mut factor = atom;
    let mut value = Interval::point(1.0);
    while n > 0 {
        if n & 1 == 1 {
            value = value.mul(factor);
        }
        n >>= 1;
        if n > 0 {
            factor = factor.mul(factor);
        }
        if !value.hi.is_finite() || !factor.hi.is_finite() {
            return Interval {
                lo: 0.0,
                hi: f64::INFINITY,
            };
        }
    }
    value
}

fn conditional_moments(
    fixture: &golaberto_odds::model::Fixture,
    law: &Law,
    positive: usize,
    negative: usize,
    outcome: usize,
    work: &mut usize,
    deadline: Instant,
) -> Result<[f64; 3], PrepareFailure> {
    Ok([
        conditional_moment(fixture, law, positive, negative, outcome, 0, work, deadline)?,
        conditional_moment(fixture, law, positive, negative, outcome, 1, work, deadline)?,
        conditional_moment(fixture, law, positive, negative, outcome, 2, work, deadline)?,
    ])
}

fn conditional_moment(
    fixture: &golaberto_odds::model::Fixture,
    law: &Law,
    positive: usize,
    negative: usize,
    outcome: usize,
    base: usize,
    work: &mut usize,
    deadline: Instant,
) -> Result<f64, PrepareFailure> {
    if outcome >= 3 || !valid_positive(law.outcomes[outcome]) {
        return Err(PrepareFailure::Invalid);
    }
    let coeff = incidence(fixture, positive) - incidence(fixture, negative);
    if coeff == 0 {
        return Ok(1.0);
    }
    let mut sum = Interval::point(0.0);
    for pair in &law.pairs[outcome] {
        *work += 1;
        if *work > WORK_LIMIT {
            return Err(PrepareFailure::Work);
        }
        if *work % 4096 == 0 && Instant::now() >= deadline {
            return Err(PrepareFailure::Budget);
        }
        let gd = i64::from(pair.home)
            .checked_sub(i64::from(pair.away))
            .ok_or(PrepareFailure::Invalid)?;
        let exponent = i64::from(coeff)
            .checked_mul(gd)
            .ok_or(PrepareFailure::Invalid)?;
        let mass = pair
            .mass
            .div(law.outcomes[outcome])
            .mul(pow_rational(BASES[base], exponent));
        sum = sum.add(mass);
    }
    if !sum.hi.is_finite() || sum.hi <= 0.0 {
        return Err(PrepareFailure::Invalid);
    }
    Ok(sum.hi)
}

pub(super) fn incidence(fixture: &golaberto_odds::model::Fixture, team: usize) -> i64 {
    (if fixture.home == team { 1 } else { 0 }) - (if fixture.away == team { 1 } else { 0 })
}

fn valid_positive(x: Interval) -> bool {
    x.lo.is_finite() && x.hi.is_finite() && x.lo > 0.0 && x.lo <= x.hi && x.hi <= 1.0
}

fn make_packed_suffix(
    plan: &Plan,
    model: &Model,
    rival: usize,
) -> (Vec<Option<i64>>, Vec<Option<i64>>) {
    let mut mins = vec![0i64; plan.order.len() + 1];
    let mut maxs = vec![0i64; plan.order.len() + 1];
    for pos in (0..plan.order.len()).rev() {
        let fi = plan.order[pos];
        let f = &model.fixtures[fi];
        let sign = if f.home == rival {
            1i64
        } else if f.away == rival {
            1i64
        } else {
            0
        };
        if sign == 0 {
            mins[pos] = mins[pos + 1];
            maxs[pos] = maxs[pos + 1];
            continue;
        }
        let mut gains = Vec::new();
        for o in 0..3 {
            if plan.masks[fi] & (1 << o) != 0 {
                let Some(gain) = packed_gain_checked(model, fi, o, plan.stride, f.home == rival)
                else {
                    return (vec![None; mins.len()], vec![None; maxs.len()]);
                };
                gains.push(gain);
            }
        }
        let lo = gains.iter().copied().min();
        let hi = gains.iter().copied().max();
        mins[pos] = match lo.and_then(|x| mins[pos + 1].checked_add(x)) {
            Some(x) => x,
            None => return (vec![None; mins.len()], vec![None; maxs.len()]),
        };
        maxs[pos] = match hi.and_then(|x| maxs[pos + 1].checked_add(x)) {
            Some(x) => x,
            None => return (vec![None; mins.len()], vec![None; maxs.len()]),
        };
    }
    (
        mins.into_iter().map(Some).collect(),
        maxs.into_iter().map(Some).collect(),
    )
}

/// Exact single-team packed-gain reachability is used only to choose a useful
/// rival. An unsupported set is unknown, never an impossibility proof.
fn packed_reachable(
    plan: &Plan,
    model: &Model,
    rival: usize,
    current: i32,
    target: i64,
    work: &mut usize,
    deadline: Instant,
) -> Result<Option<bool>, PrepareFailure> {
    let mut states = BTreeSet::from([0i64]);
    for &fi in &plan.order {
        if Instant::now() >= deadline {
            return Err(PrepareFailure::Budget);
        }
        let fixture = &model.fixtures[fi];
        if fixture.home != rival && fixture.away != rival {
            continue;
        }
        let mut gains = Vec::new();
        for o in 0..3 {
            if plan.masks[fi] & (1 << o) == 0 {
                continue;
            }
            let Some(gain) = packed_gain_checked(model, fi, o, plan.stride, fixture.home == rival)
            else {
                return Err(PrepareFailure::Invalid);
            };
            gains.push(gain);
        }
        let mut next = BTreeSet::new();
        for &old in &states {
            for &gain in &gains {
                *work += 1;
                if *work > WORK_LIMIT {
                    return Err(PrepareFailure::Work);
                }
                let Some(value) = old.checked_add(gain) else {
                    return Err(PrepareFailure::Invalid);
                };
                if i32::try_from(
                    i64::from(current)
                        .checked_add(value)
                        .ok_or(PrepareFailure::Invalid)?,
                )
                .is_err()
                {
                    return Err(PrepareFailure::Invalid);
                }
                next.insert(value);
                if next.len() > SUPPORT_LIMIT {
                    return Ok(None);
                }
            }
        }
        states = next;
    }
    let wanted = target
        .checked_sub(i64::from(current))
        .ok_or(PrepareFailure::Invalid)?;
    Ok(Some(states.contains(&wanted)))
}

fn packed_suffix_range(row: &Prepared, pos: usize, totals: &[i32]) -> Option<(i64, i64)> {
    let base = i64::from(*totals.get(row.rival)?);
    let lo = *row.suffix_min.get(pos)?.as_ref()?;
    let hi = *row.suffix_max.get(pos)?.as_ref()?;
    let lo = base.checked_add(lo)?;
    let hi = base.checked_add(hi)?;
    i32::try_from(lo).ok()?;
    i32::try_from(hi).ok()?;
    Some((lo, hi))
}

fn packed_gain_checked(
    model: &Model,
    _fi: usize,
    outcome: usize,
    stride: i32,
    home: bool,
) -> Option<i64> {
    if stride <= 0 || outcome >= 3 {
        return None;
    }
    let rules = &model.request.phase.championship;
    let (hp, ap, hw, aw) = match outcome {
        0 => (rules.point_loss, rules.point_win, false, true),
        1 => (rules.point_draw, rules.point_draw, false, false),
        _ => (rules.point_win, rules.point_loss, true, false),
    };
    let points = if home { hp } else { ap };
    let win = if home { hw } else { aw };
    let packed = i64::from(points)
        .checked_mul(i64::from(stride))?
        .checked_add(i64::from(win))?;
    i32::try_from(packed).ok()?;
    Some(packed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use golaberto_odds::model::Request;
    use serde_json::json;
    use std::time::Duration;

    fn prepared(above: bool, target: i64, lo: i64, hi: i64) -> Prepared {
        Prepared {
            rival: 1,
            above,
            packed_target: target,
            rival_lower: target,
            rival_upper: target,
            suffix_min: vec![Some(lo)],
            suffix_max: vec![Some(hi)],
            steps: vec![],
            suffix_moments: vec![[1.0; 3]],
            initial: [0.8, 0.9, 1.0],
            support_hit: false,
            score_expectation_eligible: true,
        }
    }

    #[test]
    fn factor_only_tightens_a_forced_tie() {
        let mut bounder = ScoreBounder {
            rows: vec![Some(prepared(true, 12, 0, 0))],
            diagnostics: ScoreDiagnostics::default(),
        };
        assert!(bounder.factor(0, 0, &[0, 12], [1.0; 3]) <= 0.800000000000001);
        assert_eq!(bounder.diagnostics.forced_tie_evaluations, 1);

        let mut strict = ScoreBounder {
            rows: vec![Some(prepared(true, 12, 1, 3))],
            diagnostics: ScoreDiagnostics::default(),
        };
        assert_eq!(strict.factor(0, 0, &[0, 10], [1.0; 3]), 1.0);
        assert_eq!(strict.diagnostics.forced_tie_evaluations, 0);
    }

    #[test]
    fn invalid_arithmetic_falls_back_to_one() {
        let mut row = prepared(false, 4, 0, 0);
        row.initial = [f64::INFINITY; 3];
        let mut bounder = ScoreBounder {
            rows: vec![Some(row)],
            diagnostics: ScoreDiagnostics::default(),
        };
        assert_eq!(bounder.factor(0, 0, &[0, 4], [1.0; 3]), 1.0);
    }

    #[test]
    fn dyadic_moments_use_integer_powers_and_enclose_values() {
        for (base, value) in BASES.into_iter().zip([1.125f64, 1.25, 1.5]) {
            let p = pow_rational(base, 7);
            assert!(p.lo <= value.powi(7) && value.powi(7) <= p.hi);
            let inverse = pow_rational(base, -7);
            assert!(inverse.lo <= value.powi(-7) && value.powi(-7) <= inverse.hi);
        }
        assert!(!pow_rational(BASES[2], i64::MAX).hi.is_finite());
    }

    #[test]
    fn disabled_and_expired_setup_are_neutral() {
        let disabled = ScoreBounder::disabled();
        assert_eq!(disabled.advance(0, 0, 1, [0.4; 3]), [0.4; 3]);
        assert_eq!(disabled.diagnostics().prepared_sources, 0);

        let (model, laws) = shared_tiny();
        let lower = vec![-1000; 4];
        let upper = vec![1000; 4];
        let plan = Plan::build(
            &model,
            &laws,
            &[7; 3],
            &lower,
            &upper,
            &[0, 1, 2, 3],
            4,
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut source = manual_source(&plan);
        source.set_score_ranking(0, &[], None);
        let mut expired = ScoreBounder::prepare(
            &[source],
            &model,
            &laws,
            5_000,
            Instant::now() - Duration::from_millis(1),
        );
        assert!(expired.diagnostics().budget_exhausted);
        assert_eq!(expired.factor(0, 0, &[0, 0, 0, 0], [1.0; 3]), 1.0);
    }

    #[test]
    fn no_pair_condition_uses_conditional_fixture_moment() {
        let (model, laws) = shared_tiny();
        let fixture = &model.fixtures[2];
        let outcome = 2;
        let mut work = 0;
        let moments = conditional_moments(
            fixture,
            &laws[2],
            2,
            3,
            outcome,
            &mut work,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        for j in 0..3 {
            let z = [1.125f64, 1.25, 1.5][j];
            let exact: f64 = laws[2].pairs[outcome]
                .iter()
                .map(|p| {
                    (p.mass.mid() / laws[2].outcomes[outcome].mid())
                        * z.powi((i64::from(p.home) - i64::from(p.away)) as i32 * 2)
                })
                .sum();
            assert!(moments[j] + 1e-10 >= exact);
        }
    }

    fn shared_tiny() -> (Model, Vec<Law>) {
        let request: Request = serde_json::from_value(json!({
            "id": 1,
            "phase": {"sort":"pt,w,gd,gf,gp,name", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":[{"team_id":10},{"team_id":20},{"team_id":30},{"team_id":40}],
            "games":[
                {"id":1,"home_id":10,"away_id":20,"home_power":1.1,"away_power":1.1,"played":false},
                {"id":2,"home_id":20,"away_id":10,"home_power":1.1,"away_power":1.1,"played":false},
                {"id":3,"home_id":30,"away_id":40,"home_power":1.1,"away_power":1.1,"played":false}
            ]
        })).unwrap();
        let model = Model::new(request).unwrap();
        let laws = model
            .fixtures
            .iter()
            .map(|f| Law::new(f.home_sampler.mean, f.away_sampler.mean).unwrap())
            .collect();
        (model, laws)
    }

    fn manual_source<'a>(plan: &'a Plan) -> ResidualSource<'a> {
        ResidualSource {
            graph: super::super::Graph::Plan(plan),
            node: plan.root,
            totals: plan.base.clone(),
            v_prefix: Interval::point(1.0),
            g_prefix: Interval::point(1.0),
            q0_prefix: Interval::point(1.0),
            zero: false,
            fixed_outcomes: vec![Some(2); 3],
            score_ranking: None,
        }
    }

    #[test]
    fn conditional_pair_mgf_matches_enumeration_with_shared_opposite_coefficients() {
        let (model, laws) = shared_tiny();
        let lower = vec![-1000; 4];
        let upper = vec![1000; 4];
        let plan = Plan::build(
            &model,
            &laws,
            &[7; 3],
            &lower,
            &upper,
            &[0, 1, 2, 3],
            4,
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut plan = plan;
        plan.order.retain(|&fi| fi != 2);
        let source = manual_source(&plan);
        let mut q = 0.0;
        let mut numerator = [0.0; 3];
        let mut unconditional = [1.0; 3];
        let mut per_fixture = Vec::new();
        for fi in 0..2 {
            let law = &laws[fi];
            let denom = law.outcomes[2].mid();
            let row: Vec<_> = law.pairs[2]
                .iter()
                .map(|p| (i64::from(p.home) - i64::from(p.away), p.mass.mid() / denom))
                .collect();
            per_fixture.push(row);
        }
        for &(gd0, p0) in &per_fixture[0] {
            for &(gd1, p1) in &per_fixture[1] {
                let mass = p0 * p1;
                let a_state = 2 * gd0 - 2 * gd1;
                if a_state >= 0 {
                    q += mass;
                    for j in 0..3 {
                        let z = [1.125f64, 1.25, 1.5][j];
                        numerator[j] += mass * z.powi(a_state as i32);
                    }
                }
            }
        }
        for j in 0..3 {
            let z = [1.125f64, 1.25, 1.5][j];
            unconditional[j] = per_fixture
                .iter()
                .enumerate()
                .map(|(fi, row)| {
                    row.iter()
                        .map(|(gd, mass)| {
                            mass * z.powi((if fi == 0 { 2 } else { -2 }) * *gd as i32)
                        })
                        .sum::<f64>()
                })
                .product();
        }
        assert!(q > 0.0 && q < 1.0);
        let q_interval = Interval { lo: q, hi: q };
        let mut work = 0;
        let bound = pair_mgf(
            &model,
            &laws,
            &source,
            0,
            1,
            0,
            1,
            0,
            &[true, true, false],
            q_interval,
            &mut work,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        for j in 0..3 {
            let exact = numerator[j] / q;
            assert!(
                bound[j] + 1e-10 >= exact,
                "basis {j}: bound {} < exact {exact}",
                bound[j]
            );
        }
        assert!(numerator[2] / q > unconditional[2]);

        // Reverse B while retaining the same shared score pairs: the DP must
        // preserve the negative coefficient on the reverse fixture too.
        let mut reverse_num = [0.0; 3];
        for &(gd0, p0) in &per_fixture[0] {
            for &(gd1, p1) in &per_fixture[1] {
                if 2 * gd0 - 2 * gd1 >= 0 {
                    let b = -2 * gd0 + 2 * gd1;
                    for j in 0..3 {
                        reverse_num[j] += p0 * p1 * [1.125f64, 1.25, 1.5][j].powi(b as i32);
                    }
                }
            }
        }
        let mut reverse_work = 0;
        let reverse = pair_mgf(
            &model,
            &laws,
            &source,
            0,
            1,
            1,
            0,
            0,
            &[true, true, false],
            q_interval,
            &mut reverse_work,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        for j in 0..3 {
            assert!(reverse[j] + 1e-10 >= reverse_num[j] / q);
        }

        let mut private_work = 0;
        let mut private_initial = pair_mgf(
            &model,
            &laws,
            &source,
            0,
            1,
            2,
            3,
            0,
            &[true, true, false],
            q_interval,
            &mut private_work,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let before = private_initial;
        let mut prefix_work = 0;
        assert!(multiply_known_outside_prefix(
            &mut private_initial,
            &source,
            &model,
            &laws,
            &plan,
            &[true, true, false],
            2,
            3,
            &mut prefix_work,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap());
        let expected_prefix = conditional_moments(
            &model.fixtures[2],
            &laws[2],
            2,
            3,
            2,
            &mut prefix_work,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        for j in 0..3 {
            assert!((before[j] - 1.0).abs() < 1e-9);
            let expected_once = safe_upper_product(before[j], expected_prefix[j]);
            assert!(
                private_initial[j] + 1e-10 >= expected_once,
                "basis {j}: got {}, before {}, moment {}, expected-once {}",
                private_initial[j],
                before[j],
                expected_prefix[j],
                expected_once
            );
            assert!(
                private_initial[j] < expected_once + 1e-9,
                "basis {j}: prefix multiplied more than once"
            );
        }
    }
}
