//! Shared clipped-expectation DP split by strict score-GD success and ties.

use super::{expectation, score_gd::MomentKernel, suffix, ResidualSource};
use crate::joint::{outcome_gain, Plan};
use crate::law::{categorical_probabilities, Interval, Law};
use golaberto_odds::model::Model;
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
pub(super) struct ScoreProbe {
    pub bound: expectation::ExpectationBound,
    pub plain_upper: f64,
    pub strict_upper: f64,
    pub tie_upper: f64,
    pub tie_adjusted_upper: f64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ScoreExpectationDiagnostics {
    pub score_enabled: bool,
    pub supported_sources: usize,
    pub unsupported_sources: usize,
    pub selected_rivals: BTreeMap<i32, usize>,
    pub moment_overflows: usize,
    pub tightened_queries: usize,
    pub absorbed_prefixes: usize,
    pub plain_absorbed_bound_sum: f64,
    pub score_absorbed_bound_sum: f64,
    pub strict_upper_sum: f64,
    pub tie_upper_sum: f64,
    pub tie_adjusted_upper_sum: f64,
    pub absorbed_plain_upper: f64,
    pub absorbed_score_upper: f64,
    pub absorbed_raw_strict_upper: f64,
    pub absorbed_raw_tie_upper: f64,
    pub absorbed_adjusted_tie_upper: f64,
}

#[derive(Clone, Debug)]
struct Channels {
    strict: f64,
    tie: f64,
    tie_moment: [f64; 3],
    complete: bool,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct ScoreKey {
    suffix: suffix::Key,
    threshold: u64,
    source: usize,
    kernel_identity: usize,
    rival: usize,
    above: bool,
    packed_target: i64,
    rival_total: i32,
}

pub(super) struct Cache {
    rows: HashMap<ScoreKey, Channels>,
    in_progress: HashSet<ScoreKey>,
    deadline: Instant,
    budget: Duration,
    spent: Duration,
    options: suffix::SuffixOptions,
    diagnostics: expectation::ExpectationDiagnostics,
    score_diagnostics: ScoreExpectationDiagnostics,
    source_support: HashMap<usize, bool>,
}

impl Cache {
    pub fn new(options: &suffix::SuffixOptions, started: Instant, deadline: Instant) -> Self {
        let duration = deadline.saturating_duration_since(started);
        let fraction = if options.expectation_budget_fraction.is_finite() {
            options.expectation_budget_fraction.clamp(0.0, 1.0)
        } else {
            0.0
        };
        Self {
            rows: HashMap::new(),
            in_progress: HashSet::new(),
            deadline,
            budget: Duration::from_secs_f64(duration.as_secs_f64() * fraction),
            spent: Duration::ZERO,
            options: options.clone(),
            diagnostics: expectation::ExpectationDiagnostics {
                enabled: options.shared_expectation,
                threshold_bits: options.expectation_bits,
                ..Default::default()
            },
            score_diagnostics: ScoreExpectationDiagnostics {
                score_enabled: options.shared_expectation,
                ..Default::default()
            },
            source_support: HashMap::new(),
        }
    }

    pub fn diagnostics(&self) -> expectation::ExpectationDiagnostics {
        let mut out = self.diagnostics.clone();
        out.rows = self.rows.len();
        let mut unique_states = HashSet::new();
        out.completed_rows = 0;
        for (key, row) in &self.rows {
            unique_states.insert(&key.suffix);
            out.completed_rows += usize::from(row.complete);
        }
        out.unique_states = unique_states.len();
        out.incomplete_rows = out.rows.saturating_sub(out.completed_rows);
        out.spent_ms = self.spent.as_secs_f64() * 1000.0;
        out
    }

    pub fn score_diagnostics(&self) -> ScoreExpectationDiagnostics {
        self.score_diagnostics.clone()
    }

    pub fn max_remaining(&self) -> usize {
        self.options.expectation_max_remaining
    }

    pub fn finish(&mut self) {
        self.diagnostics = self.diagnostics();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn probe(
        &mut self,
        suffix_cache: &mut suffix::Cache,
        source: &ResidualSource<'_>,
        model: &Model,
        laws: &[Law],
        pos: usize,
        node: Option<usize>,
        totals: &[i32],
        threshold: f64,
        source_index: usize,
        kernel: Option<MomentKernel<'_>>,
        prefix_moments: [f64; 3],
        deadline: Instant,
    ) -> Result<ScoreProbe, String> {
        let plan = source.graph.plan();
        let valid_kernel = kernel.is_some_and(|k| {
            k.rival < model.n
                && k.steps.len() == plan.order.len()
                && k.suffix_moments.len() >= plan.order.len() + 1
                && k.steps
                    .iter()
                    .flatten()
                    .flatten()
                    .all(|row| row.iter().all(|value| value.is_finite() && *value >= 0.0))
                && k.suffix_moments.iter().flatten().all(|value| *value >= 0.0)
        });
        if kernel.is_some() && !valid_kernel {
            self.diagnostics.invalid_states += 1;
        }
        let kernel = kernel.filter(|_| valid_kernel);
        if !self.source_support.contains_key(&source_index) {
            self.source_support.insert(source_index, kernel.is_some());
            if let Some(kernel) = kernel {
                self.score_diagnostics.supported_sources += 1;
                *self
                    .score_diagnostics
                    .selected_rivals
                    .entry(model.ids[kernel.rival])
                    .or_default() += 1;
            } else {
                self.score_diagnostics.unsupported_sources += 1;
            }
        }
        let started = Instant::now();
        let now = Instant::now();
        let allowance = self.budget.saturating_sub(self.spent);
        let budget_deadline = now.checked_add(allowance).unwrap_or(self.deadline);
        let probe_deadline = now
            .checked_add(Duration::from_millis(self.options.probe_ms))
            .unwrap_or(self.deadline);
        let local_deadline = deadline
            .min(self.deadline)
            .min(budget_deadline)
            .min(probe_deadline);
        let mut visits = 0usize;
        let result = if !self.options.shared_expectation
            || !(4..=20).contains(&self.options.expectation_bits)
            || pos > plan.order.len()
            || plan.order.len().saturating_sub(pos) > self.options.expectation_max_remaining
            || !threshold.is_finite()
            || threshold < 0.0
        {
            self.diagnostics.invalid_states += 1;
            Channels {
                strict: 1.0,
                tie: if kernel.is_some() { 1.0 } else { 0.0 },
                tie_moment: [f64::INFINITY; 3],
                complete: false,
            }
        } else {
            let grid = floor_grid(threshold, self.options.expectation_bits);
            let Some(suffix_key) = suffix_cache.key(plan, model, laws, pos, node, totals)? else {
                let channels = Channels {
                    strict: 0.0,
                    tie: 0.0,
                    tie_moment: [0.0; 3],
                    complete: true,
                };
                return Ok(finalize_channels(channels, kernel, prefix_moments));
            };
            let kernel = kernel;
            let rival_total = kernel
                .and_then(|k| totals.get(k.rival).copied())
                .unwrap_or(0);
            let key = score_key(suffix_key, grid, source_index, kernel, rival_total);
            self.solve(
                suffix_cache,
                plan,
                key,
                pos,
                node,
                totals.to_vec(),
                rival_total,
                grid,
                source_index,
                kernel,
                model,
                laws,
                local_deadline,
                &mut visits,
            )?
        };
        self.spent += started.elapsed();
        self.diagnostics.visits += visits;
        if result.complete {
            self.diagnostics.returned_complete_calls += 1;
        }
        let probe = finalize_channels(result, kernel, prefix_moments);
        if probe.bound.complete && probe.tie_adjusted_upper < probe.tie_upper {
            self.diagnostics.tightened_rows += 1;
            self.score_diagnostics.tightened_queries += 1;
        }
        Ok(probe)
    }

    #[allow(clippy::too_many_arguments)]
    fn solve(
        &mut self,
        suffix_cache: &mut suffix::Cache,
        plan: &Plan,
        key: ScoreKey,
        pos: usize,
        node: Option<usize>,
        totals: Vec<i32>,
        rival_total: i32,
        threshold: f64,
        source_index: usize,
        kernel: Option<MomentKernel<'_>>,
        model: &Model,
        laws: &[Law],
        deadline: Instant,
        visited: &mut usize,
    ) -> Result<Channels, String> {
        if let Some(row) = self.rows.get(&key) {
            self.diagnostics.cache_hits += 1;
            if row.complete {
                self.diagnostics.completed_cache_hits += 1;
                return Ok(row.clone());
            }
            self.diagnostics.partial_cache_hits += 1;
        }
        if self.in_progress.contains(&key) {
            return Ok(unknown_channels(pos, kernel));
        }
        let old = self.rows.get(&key).cloned();
        if Instant::now() >= deadline || self.spent >= self.budget {
            self.diagnostics.time_budget_hit = true;
            return Ok(old.unwrap_or_else(|| unknown_channels(pos, kernel)));
        }
        if old.is_none()
            && self.rows.len().saturating_add(self.in_progress.len()) >= self.options.state_cap
        {
            self.diagnostics.cap_hit = true;
            return Ok(unknown_channels(pos, kernel));
        }
        if *visited >= self.options.probe_nodes {
            self.diagnostics.node_limit_hit = true;
            return Ok(old.unwrap_or_else(|| unknown_channels(pos, kernel)));
        }
        *visited += 1;
        self.in_progress.insert(key.clone());
        let result = self.evaluate(
            suffix_cache,
            plan,
            pos,
            node,
            totals,
            rival_total,
            threshold,
            source_index,
            kernel,
            model,
            laws,
            deadline,
            visited,
        );
        self.in_progress.remove(&key);
        let mut channels = result?;
        if let Some(old) = old {
            channels.strict = channels.strict.min(old.strict);
            channels.tie = channels.tie.min(old.tie);
            for j in 0..3 {
                channels.tie_moment[j] = channels.tie_moment[j].min(old.tie_moment[j]);
            }
            channels.complete |= old.complete;
        }
        self.rows.insert(key, channels.clone());
        Ok(channels)
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate(
        &mut self,
        suffix_cache: &mut suffix::Cache,
        plan: &Plan,
        pos: usize,
        node: Option<usize>,
        totals: Vec<i32>,
        rival_total: i32,
        threshold: f64,
        source_index: usize,
        kernel: Option<MomentKernel<'_>>,
        model: &Model,
        laws: &[Law],
        deadline: Instant,
        visited: &mut usize,
    ) -> Result<Channels, String> {
        if pos == plan.order.len() {
            let feasible =
                (0..model.n).all(|t| totals[t] >= plan.lower[t] && totals[t] <= plan.upper[t]);
            if !feasible {
                return Ok(zero_channels());
            }
            let residual = terminal_upper(threshold);
            let Some(kernel) = kernel else {
                return Ok(Channels {
                    strict: residual,
                    tie: 0.0,
                    tie_moment: [0.0; 3],
                    complete: true,
                });
            };
            let strict = if relation_strict(rival_total, kernel) {
                residual
            } else {
                0.0
            };
            let tie = if rival_total as i64 == kernel.packed_target {
                residual
            } else {
                0.0
            };
            return Ok(Channels {
                strict,
                tie,
                tie_moment: [tie; 3],
                complete: true,
            });
        }
        let fi = plan.order[pos];
        let (base, edges) = if let Some(id) = node {
            let Some(n) = plan.nodes.get(id) else {
                self.diagnostics.invalid_states += 1;
                return Ok(unknown_channels(pos, kernel));
            };
            if n.terminal == Some(false) {
                return Ok(zero_channels());
            }
            if n.terminal == Some(true) {
                let feasible =
                    (0..model.n).all(|t| totals[t] >= plan.lower[t] && totals[t] <= plan.upper[t]);
                if !feasible {
                    return Ok(zero_channels());
                }
                let residual = terminal_upper(threshold);
                let Some(kernel) = kernel else {
                    return Ok(Channels {
                        strict: residual,
                        tie: 0.0,
                        tie_moment: [0.0; 3],
                        complete: true,
                    });
                };
                let strict = if relation_strict(rival_total, kernel) {
                    residual
                } else {
                    0.0
                };
                let tie = if rival_total as i64 == kernel.packed_target {
                    residual
                } else {
                    0.0
                };
                return Ok(Channels {
                    strict,
                    tie,
                    tie_moment: [tie; 3],
                    complete: true,
                });
            }
            (n.weights.to_vec(), n.edges.clone())
        } else {
            (
                (0..3)
                    .map(|o| {
                        if plan.masks[fi] & (1 << o) != 0 {
                            laws[fi].outcomes[o].mid()
                        } else {
                            0.0
                        }
                    })
                    .collect(),
                Vec::new(),
            )
        };
        let q0 = match categorical_probabilities(&base) {
            Ok(x) => x,
            Err(_) => return Ok(unknown_channels(pos, kernel)),
        };
        let guided = match plan.guided_weights(model, laws, fi, pos, &totals, &base) {
            Ok(x) => x,
            Err(_) => return Ok(unknown_channels(pos, kernel)),
        };
        let gq = match categorical_probabilities(&guided) {
            Ok(x) => x,
            Err(_) => return Ok(unknown_channels(pos, kernel)),
        };
        let step = kernel.and_then(|k| k.steps.get(pos).and_then(Option::as_ref));
        let mut strict_sum = Interval::point(0.0);
        let mut tie_sum = Interval::point(0.0);
        let mut moment_sums = [Interval::point(0.0); 3];
        let mut complete = true;
        for outcome in 0..3 {
            let p = laws[fi].outcomes[outcome];
            if q0[outcome].hi <= 0.0 || p.hi <= 0.0 {
                continue;
            }
            if !p.hi.is_finite() || !gq[outcome].lo.is_finite() || gq[outcome].lo < 0.0 {
                self.diagnostics.invalid_states += 1;
                return Ok(unknown_channels(pos, kernel));
            }
            let edge = edges.iter().find(|e| e.outcome as usize == outcome);
            if node.is_some() && edge.is_none() {
                continue;
            }
            let ratio = Interval::point(threshold)
                .mul(Interval::point(gq[outcome].lo))
                .div(Interval::point(p.hi));
            if !ratio.lo.is_finite() || ratio.lo < 0.0 {
                self.diagnostics.invalid_states += 1;
                return Ok(unknown_channels(pos, kernel));
            }
            let child_threshold = floor_grid(ratio.lo, self.options.expectation_bits);
            if !child_threshold.is_finite() {
                return Ok(unknown_channels(pos, kernel));
            }
            let (hg, ag) = outcome_gain(model, fi, outcome, plan.stride);
            let fixture = &model.fixtures[fi];
            let mut child_totals = totals.clone();
            child_totals[fixture.home] = child_totals[fixture.home].saturating_add(hg);
            child_totals[fixture.away] = child_totals[fixture.away].saturating_add(ag);
            let child_rival = if kernel.is_some_and(|k| k.rival == fixture.home) {
                child_totals[fixture.home]
            } else if kernel.is_some_and(|k| k.rival == fixture.away) {
                child_totals[fixture.away]
            } else {
                rival_total
            };
            let child_node = if node.is_some() {
                edge.and_then(|e| e.child)
            } else {
                None
            };
            let Some(child_suffix_key) =
                suffix_cache.key(plan, model, laws, pos + 1, child_node, &child_totals)?
            else {
                continue;
            };
            let child_key = score_key(
                child_suffix_key,
                child_threshold,
                source_index,
                kernel,
                child_rival,
            );
            let child = self.solve(
                suffix_cache,
                plan,
                child_key,
                pos + 1,
                child_node,
                child_totals,
                child_rival,
                child_threshold,
                source_index,
                kernel,
                model,
                laws,
                deadline,
                visited,
            )?;
            complete &= child.complete;
            let p_hi = Interval { lo: 0.0, hi: p.hi };
            strict_sum = strict_sum.add(p_hi.mul(Interval::point(child.strict)));
            tie_sum = tie_sum.add(p_hi.mul(Interval::point(child.tie)));
            for j in 0..3 {
                let moment = if let Some(row) = step {
                    row[outcome][j]
                } else {
                    1.0
                };
                if !moment.is_finite() || moment < 0.0 {
                    self.score_diagnostics.moment_overflows += 1;
                    moment_sums[j] = Interval {
                        lo: moment_sums[j].lo,
                        hi: f64::INFINITY,
                    };
                    continue;
                }
                if !child.tie_moment[j].is_finite() || child.tie_moment[j] < 0.0 {
                    self.score_diagnostics.moment_overflows += 1;
                    moment_sums[j].hi = f64::INFINITY;
                    continue;
                }
                let term = p_hi
                    .mul(Interval {
                        lo: 0.0,
                        hi: moment,
                    })
                    .mul(Interval::point(child.tie_moment[j]));
                moment_sums[j] = moment_sums[j].add(term);
            }
            if Instant::now() >= deadline {
                complete = false;
                self.diagnostics.time_budget_hit = true;
            }
        }
        Ok(Channels {
            strict: strict_sum.hi.min(1.0),
            tie: tie_sum.hi.min(1.0),
            tie_moment: std::array::from_fn(|j| moment_sums[j].hi),
            complete,
        })
    }
}

fn score_key(
    suffix: suffix::Key,
    threshold: f64,
    source: usize,
    kernel: Option<MomentKernel<'_>>,
    rival_total: i32,
) -> ScoreKey {
    ScoreKey {
        suffix,
        threshold: threshold.to_bits(),
        source,
        kernel_identity: kernel.map_or(0, |k| k.identity),
        rival: kernel.map_or(usize::MAX, |k| k.rival),
        above: kernel.is_some_and(|k| k.above),
        packed_target: kernel.map_or(i64::MIN, |k| k.packed_target),
        rival_total: if kernel.is_some() { rival_total } else { 0 },
    }
}

fn relation_strict(rival_total: i32, kernel: MomentKernel<'_>) -> bool {
    if kernel.above {
        rival_total as i64 > kernel.packed_target
    } else {
        (rival_total as i64) < kernel.packed_target
    }
}

fn unknown_channels(pos: usize, kernel: Option<MomentKernel<'_>>) -> Channels {
    let tie_moment = kernel.map_or([0.0; 3], |k| {
        k.suffix_moments
            .get(pos)
            .copied()
            .unwrap_or([f64::INFINITY; 3])
            .map(|x| {
                if x.is_finite() && x >= 0.0 {
                    x
                } else {
                    f64::INFINITY
                }
            })
    });
    Channels {
        strict: 1.0,
        tie: if kernel.is_some() { 1.0 } else { 0.0 },
        tie_moment,
        complete: false,
    }
}

fn zero_channels() -> Channels {
    Channels {
        strict: 0.0,
        tie: 0.0,
        tie_moment: [0.0; 3],
        complete: true,
    }
}

fn finalize_channels(
    channels: Channels,
    kernel: Option<MomentKernel<'_>>,
    prefix_moments: [f64; 3],
) -> ScoreProbe {
    let plain = Interval::point(channels.strict)
        .add(Interval::point(channels.tie))
        .hi
        .min(1.0);
    let adjusted_tie = if let Some(kernel) = kernel {
        let cap = (0..3)
            .map(|j| {
                safe_upper_product(
                    safe_upper_product(kernel.initial[j], prefix_moments[j]),
                    channels.tie_moment[j],
                )
            })
            .fold(f64::INFINITY, f64::min);
        channels.tie.min(cap)
    } else {
        0.0
    };
    let combined = Interval::point(channels.strict)
        .add(Interval::point(adjusted_tie))
        .hi
        .min(1.0);
    ScoreProbe {
        bound: expectation::ExpectationBound {
            upper: combined,
            complete: channels.complete,
        },
        plain_upper: plain,
        strict_upper: channels.strict,
        tie_upper: channels.tie,
        tie_adjusted_upper: adjusted_tie,
    }
}

fn safe_upper_product(a: f64, b: f64) -> f64 {
    if a == 0.0 || b == 0.0 {
        return 0.0;
    }
    if !a.is_finite() || !b.is_finite() || a < 0.0 || b < 0.0 {
        return f64::INFINITY;
    }
    let product = Interval::point(a).mul(Interval::point(b)).hi;
    if product.is_finite() && product >= 0.0 {
        product
    } else {
        f64::INFINITY
    }
}

fn terminal_upper(threshold: f64) -> f64 {
    if threshold >= 1.0 {
        0.0
    } else if threshold == 0.0 {
        1.0
    } else {
        let v = 1.0 - threshold;
        if v.is_finite() {
            f64::from_bits(v.to_bits() + 1)
        } else {
            1.0
        }
    }
}

fn floor_grid(value: f64, bits: u32) -> f64 {
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    let clear = 52u32.saturating_sub(bits.min(52));
    let mask = !((1u64 << clear).saturating_sub(1));
    f64::from_bits(value.to_bits() & mask)
}

#[cfg(test)]
mod tests {
    use super::super::Graph;
    use super::*;
    use crate::law::categorical_probabilities;
    use golaberto_odds::model::{Model, Request};

    const BASES: [f64; 3] = [9.0 / 8.0, 5.0 / 4.0, 3.0 / 2.0];

    fn model() -> Model {
        let request: Request = serde_json::from_str(r#"{"id":3,"phase":{"sort":"pt,w,gd,gf,name","championship":{"point_win":3,"point_draw":1,"point_loss":0}},"games":[{"id":1,"home_id":1,"away_id":2,"home_power":1.2,"away_power":0.8,"played":false},{"id":2,"home_id":2,"away_id":3,"home_power":0.9,"away_power":1.1,"played":false},{"id":3,"home_id":1,"away_id":3,"home_power":1.0,"away_power":0.9,"played":false}],"team_groups":[{"team_id":1},{"team_id":2},{"team_id":3}]}"#).unwrap();
        Model::new(request).unwrap()
    }

    fn make_plan(model: &Model, laws: &[Law]) -> Plan {
        let mut lower = vec![0; model.n];
        let mut upper = vec![100; model.n];
        lower[1] = 1;
        upper[0] = 4;
        Plan::build(
            model,
            laws,
            &vec![7; laws.len()],
            &lower,
            &upper,
            &[0],
            1,
            2000,
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap()
    }

    fn moment_kernel<'a>(
        model: &Model,
        plan: &Plan,
        laws: &[Law],
        rival: usize,
        target: usize,
        above: bool,
        steps: &'a mut Vec<Option<[[f64; 3]; 3]>>,
        suffix: &'a mut Vec<[f64; 3]>,
        identity: usize,
    ) -> MomentKernel<'a> {
        let direction = if above { 1.0 } else { -1.0 };
        let mut by_pos = vec![None; plan.order.len()];
        for (pos, &fi) in plan.order.iter().enumerate() {
            let fixture = &model.fixtures[fi];
            let incidence = |team| {
                if fixture.home == team {
                    1
                } else if fixture.away == team {
                    -1
                } else {
                    0
                }
            };
            let coefficient = (incidence(rival) - incidence(target)) as f64 * direction;
            let mut outcome_row = [[1.0; 3]; 3];
            for outcome in 0..3 {
                let mass = laws[fi].outcomes[outcome];
                if mass.lo <= 0.0 {
                    continue;
                }
                let mut sums = [Interval::point(0.0); 3];
                for pair in &laws[fi].pairs[outcome] {
                    let gd = pair.home - pair.away;
                    for j in 0..3 {
                        let weight = Interval::point(BASES[j].powf(coefficient * f64::from(gd)));
                        sums[j] = sums[j].add(pair.mass.div(mass).mul(weight));
                    }
                }
                for j in 0..3 {
                    outcome_row[outcome][j] = sums[j].hi;
                }
            }
            by_pos[pos] = Some(outcome_row);
        }
        *steps = by_pos;
        *suffix = vec![[1.0; 3]; plan.order.len() + 1];
        for pos in (0..plan.order.len()).rev() {
            let row = steps[pos].unwrap();
            for j in 0..3 {
                let max = (0..3).map(|o| row[o][j]).fold(0.0, f64::max);
                suffix[pos][j] = Interval::point(max)
                    .mul(Interval::point(suffix[pos + 1][j]))
                    .hi;
            }
        }
        MomentKernel {
            rival,
            above,
            packed_target: 4,
            initial: [1.0; 3],
            steps,
            suffix_moments: suffix,
            identity,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn exact_paths(
        plan: &Plan,
        model: &Model,
        laws: &[Law],
        pos: usize,
        node: Option<usize>,
        totals: &[i32],
        rival_total: i32,
        gd_difference: i32,
        required_offset: i32,
        native_path: f64,
        guided_path: f64,
        score_conditional: f64,
        threshold: f64,
        kernel: MomentKernel<'_>,
        result: &mut [f64; 6],
    ) {
        if pos == plan.order.len() {
            if !(0..model.n).all(|t| totals[t] >= plan.lower[t] && totals[t] <= plan.upper[t]) {
                return;
            }
            let residual = (native_path - threshold * guided_path).max(0.0) * score_conditional;
            let strict = relation_strict(rival_total, kernel);
            if strict {
                result[0] += residual;
            }
            if rival_total as i64 == kernel.packed_target {
                result[1] += residual;
                let oriented_gd = if kernel.above {
                    gd_difference
                } else {
                    -gd_difference
                };
                if oriented_gd + required_offset >= 0 {
                    result[2] += residual;
                }
                for j in 0..3 {
                    result[3 + j] += residual * BASES[j].powi(oriented_gd);
                }
            }
            return;
        }
        let fi = plan.order[pos];
        let (base, edges) = if let Some(id) = node {
            let n = &plan.nodes[id];
            if n.terminal == Some(false) {
                return;
            }
            if n.terminal == Some(true) {
                let residual = (native_path - threshold * guided_path).max(0.0) * score_conditional;
                if (0..model.n).all(|t| totals[t] >= plan.lower[t] && totals[t] <= plan.upper[t]) {
                    if relation_strict(rival_total, kernel) {
                        result[0] += residual;
                    }
                    if rival_total as i64 == kernel.packed_target {
                        result[1] += residual;
                        let oriented_gd = if kernel.above {
                            gd_difference
                        } else {
                            -gd_difference
                        };
                        if oriented_gd + required_offset >= 0 {
                            result[2] += residual;
                        }
                        for j in 0..3 {
                            result[3 + j] += residual * BASES[j].powi(oriented_gd);
                        }
                    }
                }
                return;
            }
            (n.weights.to_vec(), n.edges.clone())
        } else {
            (
                (0..3)
                    .map(|o| {
                        if plan.masks[fi] & (1 << o) != 0 {
                            laws[fi].outcomes[o].mid()
                        } else {
                            0.0
                        }
                    })
                    .collect(),
                Vec::new(),
            )
        };
        let q0 = categorical_probabilities(&base).unwrap();
        let guided = plan
            .guided_weights(model, laws, fi, pos, totals, &base)
            .unwrap();
        let gq = categorical_probabilities(&guided).unwrap();
        for outcome in 0..3 {
            let p = laws[fi].outcomes[outcome].mid();
            if q0[outcome].hi <= 0.0 || p <= 0.0 {
                continue;
            }
            let edge = edges.iter().find(|e| e.outcome as usize == outcome);
            if node.is_some() && edge.is_none() {
                continue;
            }
            let fixture = &model.fixtures[fi];
            let (hg, ag) = outcome_gain(model, fi, outcome, plan.stride);
            let mut next = totals.to_vec();
            next[fixture.home] += hg;
            next[fixture.away] += ag;
            let next_rival = if kernel.rival == fixture.home {
                next[fixture.home]
            } else if kernel.rival == fixture.away {
                next[fixture.away]
            } else {
                rival_total
            };
            let child_node = if node.is_some() {
                edge.and_then(|e| e.child)
            } else {
                None
            };
            for pair in &laws[fi].pairs[outcome] {
                let conditional = pair.mass.mid() / p;
                let gd = (pair.home - pair.away) as i32;
                let coefficient = if fixture.home == kernel.rival {
                    1
                } else if fixture.away == kernel.rival {
                    -1
                } else {
                    0
                } - (if fixture.home == 0 {
                    1
                } else if fixture.away == 0 {
                    -1
                } else {
                    0
                });
                exact_paths(
                    plan,
                    model,
                    laws,
                    pos + 1,
                    child_node,
                    &next,
                    next_rival,
                    gd_difference + coefficient * gd,
                    required_offset,
                    native_path * p,
                    guided_path * gq[outcome].mid(),
                    score_conditional * conditional,
                    threshold,
                    kernel,
                    result,
                );
            }
        }
    }

    #[test]
    fn exact_pair_enumeration_covers_strict_tie_and_adjusted_tie_for_both_orientations() {
        let model = model();
        let laws = vec![
            Law::new(1.1, 0.7).unwrap(),
            Law::new(0.8, 1.3).unwrap(),
            Law::new(0.9, 1.2).unwrap(),
        ];
        let plan = make_plan(&model, &laws);
        let options = suffix::SuffixOptions {
            state_cap: 20_000,
            probe_nodes: 20_000,
            probe_ms: 1000,
            shared_expectation: true,
            expectation_max_remaining: 8,
            expectation_budget_fraction: 1.0,
            ..Default::default()
        };
        let now = Instant::now();
        let deadline = now + Duration::from_secs(2);
        let mut suffix_cache = suffix::Cache::new(options.clone(), now, deadline).unwrap();
        let mut cache = Cache::new(&options, now, deadline);
        let mut any_tightened = false;
        for above in [true, false] {
            let mut steps = Vec::new();
            let mut suffix_moments = Vec::new();
            let kernel = moment_kernel(
                &model,
                &plan,
                &laws,
                1,
                0,
                above,
                &mut steps,
                &mut suffix_moments,
                if above { 11 } else { 12 },
            );
            let source = ResidualSource {
                graph: Graph::Plan(&plan),
                node: plan.root,
                totals: plan.base.clone(),
                v_prefix: Interval::point(1.0),
                g_prefix: Interval::point(1.0),
                q0_prefix: Interval::point(1.0),
                zero: false,
                fixed_outcomes: vec![None; model.fixtures.len()],
                score_ranking: None,
            };
            let mut exact = [0.0; 6];
            exact_paths(
                &plan,
                &model,
                &laws,
                0,
                plan.root,
                &source.totals,
                0,
                0,
                -1,
                1.0,
                1.0,
                1.0,
                0.271,
                kernel,
                &mut exact,
            );
            assert!(
                exact[0] > 0.0 && exact[1] > 0.0 && exact[2] > 0.0,
                "need mixed strict, tie, and GD-qualified tie mass: {exact:?}"
            );
            let prefix = BASES.map(|base| base.powi(-1));
            let base = plan.nodes[plan.root.unwrap()].weights;
            let guided_weights = plan
                .guided_weights(&model, &laws, plan.order[0], 0, &source.totals, &base)
                .unwrap();
            let q0 = categorical_probabilities(&base).unwrap();
            let gq = categorical_probabilities(&guided_weights).unwrap();
            assert!(
                (0..3).any(|o| (q0[o].mid() - gq[o].mid()).abs() > 1e-6),
                "test requires qg != q0"
            );
            let probe = cache
                .probe(
                    &mut suffix_cache,
                    &source,
                    &model,
                    &laws,
                    0,
                    plan.root,
                    &source.totals,
                    0.271,
                    above as usize,
                    Some(kernel),
                    prefix,
                    deadline,
                )
                .unwrap();
            assert!(probe.bound.complete);
            assert!(
                probe.plain_upper + 1e-12 >= exact[0] + exact[1],
                "plain {} < exact {}",
                probe.plain_upper,
                exact[0] + exact[1]
            );
            assert!(
                probe.bound.upper + 1e-12 >= exact[0] + exact[2],
                "score {} < exact necessary GD {}",
                probe.bound.upper,
                exact[0] + exact[2]
            );
            let expected_adjusted = exact[0]
                + (0..3)
                    .map(|j| prefix[j] * exact[3 + j])
                    .fold(f64::INFINITY, f64::min)
                    .min(exact[1]);
            assert!(probe.bound.upper + 1e-10 >= expected_adjusted);
            any_tightened |= probe.bound.upper + 1e-12 < probe.plain_upper;
        }
        assert!(
            any_tightened,
            "an adverse prefix must tighten at least one orientation"
        );
    }

    #[test]
    fn cache_keys_keep_raw_rival_and_source_kernel_identity() {
        let model = model();
        let laws = vec![
            Law::new(1.0, 1.0).unwrap(),
            Law::new(1.0, 1.0).unwrap(),
            Law::new(1.0, 1.0).unwrap(),
        ];
        let plan = make_plan(&model, &laws);
        let mut options = suffix::SuffixOptions::default();
        options.canonicalize_certain = true;
        let now = Instant::now();
        let deadline = now + Duration::from_secs(2);
        let mut suffix_cache = suffix::Cache::new(options, now, deadline).unwrap();
        let terminal = plan.order.len();
        let mut totals_a = plan.base.clone();
        let mut totals_b = plan.base.clone();
        totals_a[1] = totals_a[1].saturating_add(1);
        totals_b[1] = totals_b[1].saturating_add(2);
        let canonical_a = suffix_cache
            .key(&plan, &model, &laws, terminal, None, &totals_a)
            .unwrap()
            .unwrap();
        let canonical_b = suffix_cache
            .key(&plan, &model, &laws, terminal, None, &totals_b)
            .unwrap()
            .unwrap();

        // At the finalized suffix the canonical key has dropped the score
        // coordinates, so the score-specific key must retain the raw rival.
        assert_eq!(canonical_a, canonical_b);
        let mut steps = vec![None; plan.order.len()];
        let mut suffix_moments = vec![[1.0; 3]; plan.order.len() + 1];
        let kernel_a = moment_kernel(
            &model,
            &plan,
            &laws,
            1,
            0,
            true,
            &mut steps,
            &mut suffix_moments,
            101,
        );
        let key_a = score_key(canonical_a.clone(), 0.5, 7, Some(kernel_a), totals_a[1]);
        let key_b = score_key(canonical_b, 0.5, 7, Some(kernel_a), totals_b[1]);
        assert_ne!(key_a, key_b);
        assert_ne!(
            key_a,
            score_key(key_a.suffix.clone(), 0.5, 8, Some(kernel_a), totals_a[1])
        );

        let other_kernel = MomentKernel {
            identity: 202,
            ..kernel_a
        };
        assert_ne!(
            key_a,
            score_key(
                key_a.suffix.clone(),
                0.5,
                7,
                Some(other_kernel),
                totals_a[1]
            )
        );
    }

    #[test]
    fn cutoff_and_nonfinite_moments_remain_conservative() {
        let steps: Vec<Option<[[f64; 3]; 3]>> = vec![None; 2];
        let suffix_moments = vec![[4.0, 5.0, 6.0]; 3];
        let kernel = MomentKernel {
            rival: 1,
            above: true,
            packed_target: 4,
            initial: [1.0; 3],
            steps: &steps,
            suffix_moments: &suffix_moments,
            identity: 33,
        };
        let unknown = unknown_channels(1, Some(kernel));
        assert!(!unknown.complete);
        assert_eq!(unknown.strict, 1.0);
        assert_eq!(unknown.tie, 1.0);
        assert_eq!(unknown.tie_moment, [4.0, 5.0, 6.0]);

        let channels = Channels {
            strict: 0.2,
            tie: 0.3,
            tie_moment: [f64::INFINITY; 3],
            complete: true,
        };
        let result = finalize_channels(channels, Some(kernel), [f64::INFINITY; 3]);
        assert!(result.bound.upper.is_finite());
        assert!(result.bound.upper >= 0.2);
        assert!(result.bound.upper <= 0.5 + 1e-12);
        assert_eq!(result.tie_adjusted_upper, 0.3);
    }

    #[test]
    fn zero_state_cap_and_expired_deadline_return_incomplete_upper() {
        let model = model();
        let laws = vec![
            Law::new(1.0, 1.0).unwrap(),
            Law::new(1.0, 1.0).unwrap(),
            Law::new(1.0, 1.0).unwrap(),
        ];
        let plan = make_plan(&model, &laws);
        let source = ResidualSource {
            graph: Graph::Plan(&plan),
            node: plan.root,
            totals: plan.base.clone(),
            v_prefix: Interval::point(1.0),
            g_prefix: Interval::point(1.0),
            q0_prefix: Interval::point(1.0),
            zero: false,
            fixed_outcomes: vec![None; model.fixtures.len()],
            score_ranking: None,
        };
        let mut steps = vec![None; plan.order.len()];
        let mut suffix_moments = vec![[1.0; 3]; plan.order.len() + 1];
        let kernel = moment_kernel(
            &model,
            &plan,
            &laws,
            1,
            0,
            true,
            &mut steps,
            &mut suffix_moments,
            404,
        );
        let mut exact = [0.0; 6];
        exact_paths(
            &plan,
            &model,
            &laws,
            0,
            plan.root,
            &source.totals,
            0,
            0,
            -1,
            1.0,
            1.0,
            1.0,
            0.2,
            kernel,
            &mut exact,
        );
        let mut options = suffix::SuffixOptions {
            shared_expectation: true,
            expectation_budget_fraction: 1.0,
            expectation_max_remaining: 8,
            probe_ms: 1000,
            ..Default::default()
        };
        let now = Instant::now();
        let deadline = now + Duration::from_secs(2);
        options.state_cap = 1;
        let mut suffix_cache = suffix::Cache::new(options.clone(), now, deadline).unwrap();
        let mut cache = Cache::new(&options, now, deadline);
        let cap_result = cache
            .probe(
                &mut suffix_cache,
                &source,
                &model,
                &laws,
                0,
                plan.root,
                &source.totals,
                0.2,
                0,
                Some(kernel),
                [1.0; 3],
                deadline,
            )
            .unwrap();
        assert!(!cap_result.bound.complete);
        assert!(cap_result.bound.upper + 1e-12 >= exact[0] + exact[2]);
        assert!(cache.diagnostics().cap_hit);

        options.state_cap = 20_000;
        let mut suffix_cache = suffix::Cache::new(options.clone(), now, deadline).unwrap();
        let mut cache = Cache::new(&options, now, deadline);
        let expired = Instant::now()
            .checked_sub(Duration::from_millis(1))
            .unwrap();
        let timed_result = cache
            .probe(
                &mut suffix_cache,
                &source,
                &model,
                &laws,
                0,
                plan.root,
                &source.totals,
                0.2,
                0,
                Some(kernel),
                [1.0; 3],
                expired,
            )
            .unwrap();
        assert!(!timed_result.bound.complete);
        assert!(timed_result.bound.upper + 1e-12 >= exact[0] + exact[2]);
        assert!(cache.diagnostics().time_budget_hit);
    }
}
