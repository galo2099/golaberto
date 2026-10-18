//! Branch-specific packed-score rival selection for score-aware expectation bounds.

use super::super::super::{Interval, Law};
use super::{
    incidence, prepare_one, safe_upper_product, MomentKernel, PrepareFailure, Prepared,
};
use super::super::ResidualSource;
use golaberto_odds::model::Model;
use serde::Serialize;
use std::{
    cmp::Ordering,
    collections::BTreeMap,
    time::{Duration, Instant},
};

const SELECTOR_STATE_CAP: usize = 500_000;
const SELECTOR_WORK_LIMIT: usize = 5_000_000;

#[derive(Clone, Copy)]
pub struct Selection<'a> {
    pub kernel: MomentKernel<'a>,
    pub prefix_moments: [f64; 3],
    pub tie_share: f64,
    pub forced_tie: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SelectorDiagnostics {
    pub enabled: bool,
    pub setup_ms: f64,
    pub candidate_count: usize,
    pub prefix_cells: usize,
    pub suffix_cells: usize,
    pub setup_work: usize,
    pub budget_exhausted: bool,
    pub state_cap_hit: bool,
    pub work_limit_hit: bool,
    pub queries: usize,
    pub no_candidate: usize,
    pub skipped_settled: usize,
    pub selected_rivals: BTreeMap<i32, usize>,
    pub forced_tie_selections: usize,
}

struct SuffixMassRow {
    gains: Vec<i64>,
    masses: Vec<Interval>,
    mass_mid_prefix: Vec<f64>,
    mass_mid_suffix: Vec<f64>,
}

struct Candidate {
    prepared: Prepared,
    prefix: Vec<BTreeMap<i64, [f64; 3]>>,
    suffix: Vec<SuffixMassRow>,
}

pub struct BranchSelector {
    rows: Vec<Vec<Candidate>>,
    sources: Vec<SourceMeta>,
    ids: Vec<i32>,
    diagnostics: SelectorDiagnostics,
}

#[derive(Clone)]
struct SourceMeta {
    totals: Vec<i32>,
}

#[derive(Clone, Copy)]
struct Choice {
    index: usize,
    forced_tie: bool,
    tie_share: f64,
    cost: f64,
    id: i32,
}

#[derive(Clone, Copy, Debug)]
enum BuildFailure {
    StateCap,
    Deadline,
    WorkLimit,
    Unsafe,
}

impl BranchSelector {
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
            rows: (0..sources.len()).map(|_| Vec::new()).collect(),
            sources: (0..sources.len())
                .map(|_| SourceMeta { totals: Vec::new() })
                .collect(),
            ids: model.ids.clone(),
            diagnostics: SelectorDiagnostics {
                enabled: true,
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
        let mut cells = 0usize;
        'sources: for (source_index, source) in priority {
            if Instant::now() >= budget_deadline {
                this.diagnostics.budget_exhausted = true;
                break;
            }
            let Some(ranking) = source.score_ranking.as_ref() else {
                this.diagnostics.skipped_settled += 1;
                continue;
            };
            let plan = source.graph.plan();
            this.sources[source_index] = SourceMeta {
                totals: source.totals.clone(),
            };
            if ranking.pair_condition.is_some()
                || ranking.target >= model.n
                || source.totals.len() != model.n
            {
                this.diagnostics.skipped_settled += 1;
                continue;
            }
            let target = ranking.target;
            let packed_target = i64::from(plan.lower[target]);
            let mut had_eligible = false;
            for rival in 0..model.n {
                if Instant::now() >= budget_deadline {
                    this.diagnostics.budget_exhausted = true;
                    break 'sources;
                }
                if rival == target {
                    continue;
                }
                let above = ranking.above.contains(&rival);
                let is_tie_eligible = if above {
                    i64::from(plan.lower[rival]) == packed_target
                        && i64::from(plan.upper[rival]) >= packed_target
                } else {
                    i64::from(plan.upper[rival]) == packed_target
                        && i64::from(plan.lower[rival]) <= packed_target
                };
                if !is_tie_eligible {
                    continue;
                }
                had_eligible = true;
                let prepared = match prepare_one(
                    source,
                    model,
                    laws,
                    &mut work,
                    budget_deadline,
                    Some(rival),
                ) {
                    Ok(Some(prepared)) => prepared,
                    Ok(None) => continue,
                    Err(PrepareFailure::Budget) => {
                        this.diagnostics.budget_exhausted = true;
                        break 'sources;
                    }
                    Err(PrepareFailure::Work) => {
                        this.diagnostics.work_limit_hit = true;
                        break 'sources;
                    }
                    Err(PrepareFailure::Support | PrepareFailure::Invalid) => continue,
                };
                if work > SELECTOR_WORK_LIMIT {
                    this.diagnostics.work_limit_hit = true;
                    break 'sources;
                }
                if !prepared.score_expectation_eligible {
                    continue;
                }
                let remaining_cells = SELECTOR_STATE_CAP.saturating_sub(cells);
                let prefix = match build_prefix_dp(
                    source,
                    &prepared,
                    model,
                    laws,
                    remaining_cells,
                    &mut work,
                    budget_deadline,
                ) {
                    Ok(prefix) => prefix,
                    Err(BuildFailure::StateCap) => {
                        this.diagnostics.state_cap_hit = true;
                        continue;
                    }
                    Err(BuildFailure::Deadline) => {
                        this.diagnostics.budget_exhausted = true;
                        break 'sources;
                    }
                    Err(BuildFailure::WorkLimit) => {
                        this.diagnostics.work_limit_hit = true;
                        break 'sources;
                    }
                    Err(BuildFailure::Unsafe) => continue,
                };
                let prefix_cells = prefix.iter().map(BTreeMap::len).sum::<usize>();
                cells = cells.saturating_add(prefix_cells);
                let remaining_cells = SELECTOR_STATE_CAP.saturating_sub(cells);
                let suffix = match build_suffix_mass(
                    source,
                    &prepared,
                    model,
                    laws,
                    remaining_cells,
                    &mut work,
                    budget_deadline,
                ) {
                    Ok(suffix) => suffix,
                    Err(BuildFailure::StateCap) => {
                        this.diagnostics.state_cap_hit = true;
                        cells = cells.saturating_sub(prefix_cells);
                        continue;
                    }
                    Err(BuildFailure::Deadline) => {
                        this.diagnostics.budget_exhausted = true;
                        break 'sources;
                    }
                    Err(BuildFailure::WorkLimit) => {
                        this.diagnostics.work_limit_hit = true;
                        break 'sources;
                    }
                    Err(BuildFailure::Unsafe) => {
                        cells = cells.saturating_sub(prefix_cells);
                        continue;
                    }
                };
                let suffix_cells = suffix.iter().map(|row| row.gains.len()).sum::<usize>();
                cells = cells.saturating_add(suffix_cells);
                this.diagnostics.prefix_cells += prefix_cells;
                this.diagnostics.suffix_cells += suffix_cells;
                this.rows[source_index].push(Candidate {
                    prepared,
                    prefix,
                    suffix,
                });
                this.diagnostics.candidate_count += 1;
            }
            if had_eligible && this.rows[source_index].is_empty() {
                this.diagnostics.skipped_settled += 1;
            }
        }
        this.diagnostics.setup_work = work;
        this.diagnostics.setup_ms = started.elapsed().as_secs_f64() * 1000.0;
        this
    }

    pub fn select(
        &mut self,
        source: usize,
        pos: usize,
        totals: &[i32],
    ) -> Option<Selection<'_>> {
        self.diagnostics.queries += 1;
        let Some(candidates) = self.rows.get(source) else {
            self.diagnostics.no_candidate += 1;
            return None;
        };
        let Some(metadata) = self.sources.get(source) else {
            self.diagnostics.no_candidate += 1;
            return None;
        };
        let mut best: Option<Choice> = None;
        for (index, candidate) in candidates.iter().enumerate() {
            let prepared = &candidate.prepared;
            let (Some(&current), Some(prefix), Some(suffix)) = (
                totals.get(prepared.rival),
                candidate.prefix.get(pos),
                candidate.suffix.get(pos),
            ) else {
                continue;
            };
            let Some(source_total) = metadata.totals.get(prepared.rival).copied() else {
                continue;
            };
            let gain = i64::from(current) - i64::from(source_total);
            let Some(prefix_moments) = prefix.get(&gain).copied() else {
                continue;
            };
            let Some(stats) = suffix_stats(
                suffix,
                current,
                prepared.rival_lower,
                prepared.rival_upper,
                prepared.packed_target,
                prepared.above,
            ) else {
                continue;
            };
            let Some(suffix_max) = prepared.suffix_moments.get(pos) else {
                continue;
            };
            let mut cost = f64::INFINITY;
            for j in 0..3 {
                let value = safe_upper_product(
                    safe_upper_product(prepared.initial[j], prefix_moments[j]),
                    suffix_max[j],
                );
                if value.is_finite() && value >= 0.0 {
                    cost = cost.min(value);
                }
            }
            let choice = Choice {
                index,
                forced_tie: stats.forced_tie,
                tie_share: stats.tie_share,
                cost,
                id: *self.ids.get(prepared.rival).unwrap_or(&i32::MAX),
            };
            if best.is_none_or(|old| choice_cmp(choice, old) == Ordering::Less) {
                best = Some(choice);
            }
        }
        let Some(best) = best else {
            self.diagnostics.no_candidate += 1;
            return None;
        };
        let candidate = &self.rows[source][best.index];
        let prepared = &candidate.prepared;
        let source_total = self.sources[source].totals.get(prepared.rival).copied()?;
        let prefix_gain = i64::from(totals[prepared.rival]) - i64::from(source_total);
        let prefix_moments = *candidate.prefix.get(pos)?.get(&prefix_gain)?;
        *self
            .diagnostics
            .selected_rivals
            .entry(best.id)
            .or_default() += 1;
        if best.forced_tie {
            self.diagnostics.forced_tie_selections += 1;
        }
        Some(Selection {
            kernel: MomentKernel {
                rival: prepared.rival,
                above: prepared.above,
                packed_target: prepared.packed_target,
                initial: prepared.initial,
                steps: &prepared.steps,
                suffix_moments: &prepared.suffix_moments,
                identity: prepared.steps.as_ptr() as usize,
            },
            prefix_moments,
            tie_share: best.tie_share,
            forced_tie: best.forced_tie,
        })
    }

    pub fn diagnostics(&self) -> SelectorDiagnostics {
        self.diagnostics.clone()
    }
}

fn choice_cmp(a: Choice, b: Choice) -> Ordering {
    b.forced_tie
        .cmp(&a.forced_tie)
        .then_with(|| b.tie_share.total_cmp(&a.tie_share))
        .then_with(|| a.cost.total_cmp(&b.cost))
        .then_with(|| a.id.cmp(&b.id))
}

struct SuffixStats {
    tie_share: f64,
    forced_tie: bool,
}

fn suffix_stats(
    row: &SuffixMassRow,
    current: i32,
    lower: i64,
    upper: i64,
    target: i64,
    above: bool,
) -> Option<SuffixStats> {
    let min_gain = lower.checked_sub(i64::from(current))?;
    let max_gain = upper.checked_sub(i64::from(current))?;
    if min_gain > max_gain {
        return None;
    }
    let start = row.gains.partition_point(|&gain| gain < min_gain);
    let end = row.gains.partition_point(|&gain| gain <= max_gain);
    if start >= end {
        return None;
    }
    let tie_gain = target.checked_sub(i64::from(current))?;
    let tie_index = row.gains.binary_search(&tie_gain).ok();
    let tie_possible = tie_index.is_some_and(|index| index >= start && index < end);
    if !tie_possible {
        return None;
    }
    let strict_cut = target.checked_sub(i64::from(current))?;
    let strict_start = if above {
        row.gains
            .partition_point(|&gain| gain <= strict_cut)
            .max(start)
    } else {
        start
    };
    let strict_end = if above {
        end
    } else {
        row.gains
            .partition_point(|&gain| gain < strict_cut)
            .min(end)
    };
    let has_strict = strict_start < strict_end;
    let denom = if above {
        row.mass_mid_suffix[start] - row.mass_mid_suffix[end]
    } else {
        row.mass_mid_prefix[end] - row.mass_mid_prefix[start]
    };
    let tie_index = tie_index?;
    let tie_mass = row.masses[tie_index].mid().max(0.0);
    Some(SuffixStats {
        tie_share: if denom > 0.0 { (tie_mass / denom).clamp(0.0, 1.0) } else { 0.0 },
        forced_tie: !has_strict,
    })
}

fn build_prefix_dp(
    source: &ResidualSource<'_>,
    prepared: &Prepared,
    model: &Model,
    laws: &[Law],
    cell_limit: usize,
    work: &mut usize,
    deadline: Instant,
) -> Result<Vec<BTreeMap<i64, [f64; 3]>>, BuildFailure> {
    let plan = source.graph.plan();
    let mut rows = Vec::with_capacity(plan.order.len() + 1);
    rows.push(BTreeMap::from([(0i64, [1.0; 3])]));
    let mut allocated = 1usize;
    for (pos, &fi) in plan.order.iter().enumerate() {
        if Instant::now() >= deadline {
            return Err(BuildFailure::Deadline);
        }
        let mut next = BTreeMap::<i64, [f64; 3]>::new();
        let fixture = &model.fixtures[fi];
        let involves_rival = fixture.home == prepared.rival || fixture.away == prepared.rival;
        let step = prepared.steps.get(pos).and_then(Option::as_ref);
        if involves_rival {
            for (&gain, &prefix) in rows.last().ok_or(BuildFailure::Unsafe)? {
                for outcome in 0..3 {
                    if plan.masks[fi] & (1 << outcome) == 0 || laws[fi].outcomes[outcome].hi <= 0.0 {
                        continue;
                    }
                    *work = work.checked_add(1).ok_or(BuildFailure::WorkLimit)?;
                    if *work > SELECTOR_WORK_LIMIT {
                        return Err(BuildFailure::WorkLimit);
                    }
                    poll_setup(*work, deadline)?;
                    let delta = super::packed_gain_checked(
                        model,
                        fi,
                        outcome,
                        plan.stride,
                        fixture.home == prepared.rival,
                    )
                    .ok_or(BuildFailure::Unsafe)?;
                    let next_gain = gain
                        .checked_add(delta)
                        .ok_or(BuildFailure::Unsafe)?;
                    let moment = step.map_or([1.0; 3], |row| row[outcome]);
                    let value: [f64; 3] =
                        std::array::from_fn(|j| safe_upper_product(prefix[j], moment[j]));
                    let entry = next.entry(next_gain).or_insert([0.0; 3]);
                    for j in 0..3 {
                        entry[j] = entry[j].max(value[j]);
                    }
                }
            }
        } else {
            let max_step: [f64; 3] = std::array::from_fn(|j| {
                (0..3)
                    .filter(|&o| plan.masks[fi] & (1 << o) != 0 && laws[fi].outcomes[o].hi > 0.0)
                    .map(|o| step.map_or(1.0, |row| row[o][j]))
                    .fold(0.0, f64::max)
            });
            for (&gain, &prefix) in rows.last().ok_or(BuildFailure::Unsafe)? {
                *work = work.checked_add(1).ok_or(BuildFailure::WorkLimit)?;
                if *work > SELECTOR_WORK_LIMIT {
                    return Err(BuildFailure::WorkLimit);
                }
                poll_setup(*work, deadline)?;
                let value = std::array::from_fn(|j| safe_upper_product(prefix[j], max_step[j]));
                next.insert(gain, value);
            }
        }
        if next.is_empty() {
            return Err(BuildFailure::Unsafe);
        }
        if allocated.saturating_add(next.len()) > cell_limit {
            return Err(BuildFailure::StateCap);
        }
        allocated += next.len();
        if next.keys().any(|gain| {
            i64::from(source.totals[prepared.rival])
                .checked_add(*gain)
                .and_then(|total| i32::try_from(total).ok())
                .is_none()
        }) {
            return Err(BuildFailure::Unsafe);
        }
        rows.push(next);
    }
    Ok(rows)
}

fn build_suffix_mass(
    source: &ResidualSource<'_>,
    prepared: &Prepared,
    model: &Model,
    laws: &[Law],
    cell_limit: usize,
    work: &mut usize,
    deadline: Instant,
) -> Result<Vec<SuffixMassRow>, BuildFailure> {
    let plan = source.graph.plan();
    let mut rows: Vec<Option<SuffixMassRow>> = (0..=plan.order.len()).map(|_| None).collect();
    let terminal = BTreeMap::from([(0i64, Interval::point(1.0))]);
    if cell_limit == 0 {
        return Err(BuildFailure::StateCap);
    }
    rows[plan.order.len()] = Some(to_suffix_row(terminal));
    let mut allocated = 1usize;
    for pos in (0..plan.order.len()).rev() {
        if Instant::now() >= deadline {
            return Err(BuildFailure::Deadline);
        }
        let fi = plan.order[pos];
        let fixture = &model.fixtures[fi];
        let involves_rival = fixture.home == prepared.rival || fixture.away == prepared.rival;
        let child = rows[pos + 1].as_ref().ok_or(BuildFailure::Unsafe)?;
        let mut next = BTreeMap::<i64, Interval>::new();
        if involves_rival {
            for (&gain, &mass) in child.gains.iter().zip(&child.masses) {
                for outcome in 0..3 {
                    if plan.masks[fi] & (1 << outcome) == 0 || laws[fi].outcomes[outcome].hi <= 0.0 {
                        continue;
                    }
                    *work = work.checked_add(1).ok_or(BuildFailure::WorkLimit)?;
                    if *work > SELECTOR_WORK_LIMIT {
                        return Err(BuildFailure::WorkLimit);
                    }
                    poll_setup(*work, deadline)?;
                    let delta = super::packed_gain_checked(
                        model,
                        fi,
                        outcome,
                        plan.stride,
                        fixture.home == prepared.rival,
                    )
                    .ok_or(BuildFailure::Unsafe)?;
                    let new_gain = gain
                        .checked_add(delta)
                        .ok_or(BuildFailure::Unsafe)?;
                    let term = mass.mul(laws[fi].outcomes[outcome]);
                    let old = next.get(&new_gain).copied().unwrap_or_else(|| Interval::point(0.0));
                    next.insert(new_gain, old.add(term));
                }
            }
        } else {
            let mut fixture_mass = Interval::point(0.0);
            for outcome in 0..3 {
                if plan.masks[fi] & (1 << outcome) != 0 && laws[fi].outcomes[outcome].hi > 0.0 {
                    fixture_mass = fixture_mass.add(laws[fi].outcomes[outcome]);
                }
            }
            for (&gain, &mass) in child.gains.iter().zip(&child.masses) {
                next.insert(gain, mass.mul(fixture_mass));
            }
        }
        if next.is_empty() {
            return Err(BuildFailure::Unsafe);
        }
        if allocated.saturating_add(next.len()) > cell_limit {
            return Err(BuildFailure::StateCap);
        }
        allocated += next.len();
        rows[pos] = Some(to_suffix_row(next));
    }
    rows.into_iter().map(|row| row.ok_or(BuildFailure::Unsafe)).collect()
}

fn poll_setup(work: usize, deadline: Instant) -> Result<(), BuildFailure> {
    if work % 1024 == 0 && Instant::now() >= deadline {
        Err(BuildFailure::Deadline)
    } else {
        Ok(())
    }
}

fn to_suffix_row(map: BTreeMap<i64, Interval>) -> SuffixMassRow {
    let mut gains = Vec::with_capacity(map.len());
    let mut masses = Vec::with_capacity(map.len());
    let mut prefix = Vec::with_capacity(map.len() + 1);
    prefix.push(0.0);
    for (gain, mass) in map {
        gains.push(gain);
        masses.push(mass);
        prefix.push(prefix.last().copied().unwrap_or(0.0) + mass.mid().max(0.0));
    }
    let mut suffix = vec![0.0; masses.len() + 1];
    for index in (0..masses.len()).rev() {
        suffix[index] = suffix[index + 1] + masses[index].mid().max(0.0);
    }
    SuffixMassRow {
        gains,
        masses,
        mass_mid_prefix: prefix,
        mass_mid_suffix: suffix,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use golaberto_odds::model::Request;
    use serde_json::json;

    fn tiny() -> (Model, Vec<Law>) {
        let request: Request = serde_json::from_value(json!({
            "id": 1,
            "phase": {"sort":"pt,w,gd,gf,gp,name", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":[{"team_id":10},{"team_id":20},{"team_id":30}],
            "games":[
                {"id":1,"home_id":10,"away_id":20,"home_power":1.1,"away_power":0.8,"played":false},
                {"id":2,"home_id":20,"away_id":30,"home_power":0.9,"away_power":1.0,"played":false},
                {"id":3,"home_id":30,"away_id":10,"home_power":1.0,"away_power":0.9,"played":false}
            ]
        })).unwrap();
        let model = Model::new(request).unwrap();
        let laws = model.fixtures.iter().map(|f| {
            Law::new(f.home_sampler.mean, f.away_sampler.mean).unwrap()
        }).collect();
        (model, laws)
    }

    #[test]
    fn selector_choice_order_prefers_forced_ties_then_tie_share() {
        let forced = Choice { index: 1, forced_tie: true, tie_share: 0.01, cost: 10.0, id: 99 };
        let strict = Choice { index: 0, forced_tie: false, tie_share: 0.99, cost: 0.0, id: 1 };
        assert_eq!(choice_cmp(forced, strict), Ordering::Less);
        let concentrated = Choice { index: 2, forced_tie: false, tie_share: 0.8, cost: 100.0, id: 9 };
        let diffuse = Choice { index: 3, forced_tie: false, tie_share: 0.2, cost: 0.0, id: 1 };
        assert_eq!(choice_cmp(concentrated, diffuse), Ordering::Less);
        let same = Choice { index: 4, forced_tie: false, tie_share: 0.2, cost: 1.0, id: 5 };
        let lower_cost = Choice { index: 5, forced_tie: false, tie_share: 0.2, cost: 0.5, id: 6 };
        assert_eq!(choice_cmp(lower_cost, same), Ordering::Less);
    }

    #[test]
    fn selector_switches_to_whichever_rival_is_forced_into_a_tie() {
        let candidate = |rival: usize| Candidate {
            prepared: Prepared {
                rival,
                above: true,
                packed_target: 100,
                rival_lower: 100,
                rival_upper: 102,
                suffix_min: vec![],
                suffix_max: vec![],
                steps: vec![None],
                suffix_moments: vec![[1.0; 3]],
                initial: [1.0; 3],
                support_hit: false,
                score_expectation_eligible: true,
            },
            prefix: vec![BTreeMap::from([(0, [0.8; 3]), (1, [0.8; 3])])],
            // At current total 100, only the zero gain is a tie and no strict
            // higher score is possible. At 101, -1 ties while zero is strict.
            suffix: vec![SuffixMassRow {
                gains: vec![-1, 0],
                masses: vec![Interval::point(0.5); 2],
                mass_mid_prefix: vec![0.0, 0.5, 1.0],
                mass_mid_suffix: vec![1.0, 0.5, 0.0],
            }],
        };
        let mut selector = BranchSelector {
            rows: vec![vec![candidate(1), candidate(2)]],
            sources: vec![SourceMeta { totals: vec![0, 100, 100] }],
            ids: vec![10, 20, 30],
            diagnostics: SelectorDiagnostics { enabled: true, ..Default::default() },
        };
        let selected_b = selector.select(0, 0, &[0, 101, 100]).unwrap();
        assert_eq!(selected_b.kernel.rival, 2);
        assert!(selected_b.forced_tie);
        assert_eq!(selected_b.prefix_moments, [0.8; 3]);
        let selected_a = selector.select(0, 0, &[0, 100, 101]).unwrap();
        assert_eq!(selected_a.kernel.rival, 1);
        assert!(selected_a.forced_tie);
        assert!(selector.select(0, 0, &[0, 102, 102]).is_none());
    }

    #[test]
    fn prefix_dp_encloses_every_tiny_history_in_both_fixture_orientations() {
        let (model, laws) = tiny();
        let lower = vec![i32::MIN / 4; model.n];
        let upper = vec![i32::MAX / 4; model.n];
        let plan = super::super::super::Plan::build(
            &model, &laws, &[7; 3], &lower, &upper, &[0, 1], 4, 10_000,
            Instant::now() + Duration::from_secs(2),
        ).unwrap();
        let source = plan.residual_source(Interval::point(1.0), Interval::point(1.0)).unwrap();
        let steps: Vec<Option<[[f64; 3]; 3]>> = (0..plan.order.len()).map(|pos| {
            // Distinct per-outcome factors make multiple WDL paths at the same
            // packed rival gain exercise the per-channel maximum operation.
            Some(std::array::from_fn(|o| std::array::from_fn(|j| {
                0.7 + ((pos + 2 * o + j) % 5) as f64 * 0.2
            })))
        }).collect();
        for rival in [0usize, 1usize] {
            let prepared = Prepared {
                rival,
                above: true,
                packed_target: 0,
                rival_lower: i64::from(plan.lower[rival]),
                rival_upper: i64::from(plan.upper[rival]),
                suffix_min: vec![None; plan.order.len() + 1],
                suffix_max: vec![None; plan.order.len() + 1],
                steps: steps.clone(),
                suffix_moments: vec![[1.0; 3]; plan.order.len() + 1],
                initial: [0.4, 0.8, 1.3],
                support_hit: false,
                score_expectation_eligible: true,
            };
            let mut work = 0;
            let rows = build_prefix_dp(
                &source, &prepared, &model, &laws, SELECTOR_STATE_CAP,
                &mut work, Instant::now() + Duration::from_secs(2),
            ).unwrap();
            assert_eq!(rows[0].get(&0), Some(&[1.0; 3]));
            // Prefix history is conditional on the current residual source;
            // its already-accounted initial factor is intentionally separate.
            assert_ne!(prepared.initial, [1.0; 3]);
            let mut histories = vec![(0i64, [1.0; 3])];
            for (pos, &fi) in plan.order.iter().enumerate() {
                let fixture = &model.fixtures[fi];
                let mut next = Vec::new();
                for &(gain, prefix) in &histories {
                    for outcome in 0..3 {
                        if plan.masks[fi] & (1 << outcome) == 0 || laws[fi].outcomes[outcome].hi <= 0.0 {
                            continue;
                        }
                        let delta = super::super::packed_gain_checked(
                            &model, fi, outcome, plan.stride, fixture.home == rival,
                        ).unwrap_or(0);
                        let gain = if fixture.home == rival || fixture.away == rival {
                            gain + delta
                        } else { gain };
                        let step = prepared.steps[pos].unwrap()[outcome];
                        let moments = std::array::from_fn(|j| prefix[j] * step[j]);
                        next.push((gain, moments));
                    }
                }
                for (gain, moments) in &next {
                    let bound = rows[pos + 1].get(gain).expect("reachable gain has a DP cell");
                    for j in 0..3 {
                        assert!(bound[j] >= moments[j]);
                    }
                }
                histories = next;
            }
        }
    }

    #[test]
    fn prefix_dp_deadline_and_cell_cap_decline_without_zero_bound() {
        let (model, laws) = tiny();
        let lower = vec![i32::MIN / 4; model.n];
        let upper = vec![i32::MAX / 4; model.n];
        let plan = super::super::super::Plan::build(
            &model, &laws, &[7; 3], &lower, &upper, &[0, 1], 4, 10_000,
            Instant::now() + Duration::from_secs(2),
        ).unwrap();
        let source = plan.residual_source(Interval::point(1.0), Interval::point(1.0)).unwrap();
        let prepared = Prepared {
            rival: 1, above: true, packed_target: 0,
            rival_lower: i64::from(plan.lower[1]), rival_upper: i64::from(plan.upper[1]),
            suffix_min: vec![], suffix_max: vec![], steps: vec![None; plan.order.len()],
            suffix_moments: vec![], initial: [1.0; 3], support_hit: false,
            score_expectation_eligible: true,
        };
        let mut work = 0;
        assert!(matches!(build_prefix_dp(
            &source, &prepared, &model, &laws, 100, &mut work, Instant::now() - Duration::from_secs(1)
        ), Err(BuildFailure::Deadline)));
        work = 0;
        assert!(matches!(build_prefix_dp(
            &source, &prepared, &model, &laws, 1, &mut work, Instant::now() + Duration::from_secs(1)
        ), Err(BuildFailure::StateCap)));
    }

    #[test]
    fn rival_only_checked_gain_does_not_overflow_for_opponent_win() {
        let (model, _) = tiny();
        let stride = i32::MAX;
        // Outcome zero is an away win. A checked home-rival loss is zero;
        // computing the unrelated away team's 3*stride first would overflow.
        assert_eq!(super::super::packed_gain_checked(&model, 0, 0, stride, true), Some(0));
        assert_eq!(super::super::packed_gain_checked(&model, 0, 0, stride, false), None);
    }
}
