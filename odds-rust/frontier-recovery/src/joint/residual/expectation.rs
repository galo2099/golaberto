//! Sparse memoized upper bounds for clipped native/guided suffix expectations.

use super::{suffix, ResidualSource};
use crate::joint::{outcome_gain, Plan};
use crate::law::{categorical_probabilities, Interval, Law};
use golaberto_odds::model::Model;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
pub struct ExpectationBound {
    pub upper: f64,
    pub complete: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ExpectationDiagnostics {
    pub enabled: bool,
    pub visits: usize,
    pub rows: usize,
    pub unique_states: usize,
    pub threshold_bits: u32,
    pub completed_rows: usize,
    pub incomplete_rows: usize,
    pub cache_hits: usize,
    pub completed_cache_hits: usize,
    pub partial_cache_hits: usize,
    pub returned_complete_calls: usize,
    pub tightened_rows: usize,
    pub cap_hit: bool,
    pub time_budget_hit: bool,
    pub node_limit_hit: bool,
    pub invalid_states: usize,
    pub spent_ms: f64,
    pub absorbed_prefixes: usize,
    pub saved_nonzero_depth: usize,
    pub absorbed_bound_sum: f64,
    pub absorbed_bound_before: f64,
    pub absorbed_bound_after: f64,
    pub zero_mean_archives: usize,
    pub zero_mean_max_cap: f64,
    pub partial_tightened_prefixes: usize,
}

#[derive(Clone, Debug)]
struct Row {
    upper: f64,
    complete: bool,
}

pub(super) struct Cache {
    rows: HashMap<(suffix::Key, u64), Row>,
    in_progress: HashSet<(suffix::Key, u64)>,
    deadline: Instant,
    budget: Duration,
    spent: Duration,
    options: suffix::SuffixOptions,
    diagnostics: ExpectationDiagnostics,
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
            diagnostics: ExpectationDiagnostics {
                enabled: options.shared_expectation,
                threshold_bits: options.expectation_bits,
                ..Default::default()
            },
        }
    }

    pub fn diagnostics(&self) -> ExpectationDiagnostics {
        let mut result = self.diagnostics.clone();
        result.rows = self.rows.len();
        let mut states = HashSet::new();
        result.completed_rows = 0;
        for ((key, _), row) in &self.rows {
            states.insert(key);
            result.completed_rows += usize::from(row.complete);
        }
        result.unique_states = states.len();
        result.incomplete_rows = result.rows.saturating_sub(result.completed_rows);
        result.spent_ms = self.spent.as_secs_f64() * 1000.0;
        result
    }

    pub fn max_remaining(&self) -> usize {
        self.options.expectation_max_remaining
    }

    pub fn finish(&mut self) {
        self.diagnostics.rows = self.rows.len();
        let mut states = HashSet::new();
        self.diagnostics.completed_rows = 0;
        for ((key, _), row) in &self.rows {
            states.insert(key);
            self.diagnostics.completed_rows += usize::from(row.complete);
        }
        self.diagnostics.unique_states = states.len();
        self.diagnostics.incomplete_rows = self.rows.len() - self.diagnostics.completed_rows;
        self.diagnostics.spent_ms = self.spent.as_secs_f64() * 1000.0;
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
        deadline: Instant,
    ) -> Result<ExpectationBound, String> {
        let plan = source.graph.plan();
        let start = Instant::now();
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
        let mut visited = 0usize;
        let result = if !self.options.shared_expectation
            || self.options.expectation_bits < 4
            || self.options.expectation_bits > 20
            || pos > plan.order.len()
            || plan.order.len().saturating_sub(pos) > self.options.expectation_max_remaining
        {
            ExpectationBound {
                upper: 1.0,
                complete: false,
            }
        } else if !threshold.is_finite() || threshold < 0.0 || threshold == f64::INFINITY {
            self.diagnostics.invalid_states += 1;
            ExpectationBound {
                upper: 1.0,
                complete: false,
            }
        } else {
            let grid = floor_grid(threshold, self.options.expectation_bits);
            if !grid.is_finite() {
                self.diagnostics.invalid_states += 1;
                ExpectationBound {
                    upper: 1.0,
                    complete: false,
                }
            } else {
                match suffix_cache.key(plan, model, laws, pos, node, totals)? {
                    None => ExpectationBound {
                        upper: 0.0,
                        complete: true,
                    },
                    Some(key) => self.solve(
                        suffix_cache,
                        plan,
                        key,
                        pos,
                        node,
                        totals.to_vec(),
                        grid,
                        model,
                        laws,
                        local_deadline,
                        &mut visited,
                    )?,
                }
            }
        };
        self.spent += start.elapsed();
        self.diagnostics.visits += visited;
        if result.complete {
            self.diagnostics.returned_complete_calls += 1;
        }
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn solve(
        &mut self,
        suffix_cache: &mut suffix::Cache,
        plan: &Plan,
        key: suffix::Key,
        pos: usize,
        node: Option<usize>,
        totals: Vec<i32>,
        threshold: f64,
        model: &Model,
        laws: &[Law],
        deadline: Instant,
        visited: &mut usize,
    ) -> Result<ExpectationBound, String> {
        let cell = (key.clone(), threshold.to_bits());
        if let Some(row) = self.rows.get(&cell) {
            self.diagnostics.cache_hits += 1;
            if row.complete {
                self.diagnostics.completed_cache_hits += 1;
                return Ok(ExpectationBound {
                    upper: row.upper,
                    complete: true,
                });
            }
            self.diagnostics.partial_cache_hits += 1;
        }
        if self.in_progress.contains(&cell) {
            return Ok(ExpectationBound {
                upper: 1.0,
                complete: false,
            });
        }
        let old = self.rows.get(&cell).cloned();
        if Instant::now() >= deadline || self.spent >= self.budget {
            self.diagnostics.time_budget_hit = true;
            return Ok(old.map_or(
                ExpectationBound {
                    upper: 1.0,
                    complete: false,
                },
                |r| ExpectationBound {
                    upper: r.upper,
                    complete: false,
                },
            ));
        }
        if old.is_none()
            && self.rows.len().saturating_add(self.in_progress.len()) >= self.options.state_cap
        {
            self.diagnostics.cap_hit = true;
            return Ok(old.map_or(
                ExpectationBound {
                    upper: 1.0,
                    complete: false,
                },
                |r| ExpectationBound {
                    upper: r.upper,
                    complete: false,
                },
            ));
        }
        if *visited >= self.options.probe_nodes {
            self.diagnostics.node_limit_hit = true;
            return Ok(old.map_or(
                ExpectationBound {
                    upper: 1.0,
                    complete: false,
                },
                |r| ExpectationBound {
                    upper: r.upper,
                    complete: false,
                },
            ));
        }
        *visited += 1;
        self.in_progress.insert(cell.clone());
        let answer = self.evaluate(
            suffix_cache,
            plan,
            pos,
            node,
            totals,
            threshold,
            model,
            laws,
            deadline,
            visited,
        );
        self.in_progress.remove(&cell);
        let mut answer = answer?;
        if let Some(previous) = old {
            let tightened = answer.upper < previous.upper;
            answer.upper = answer.upper.min(previous.upper);
            answer.complete |= previous.complete;
            if tightened {
                self.diagnostics.tightened_rows += 1;
            }
        }
        self.rows.insert(
            cell,
            Row {
                upper: answer.upper,
                complete: answer.complete,
            },
        );
        Ok(answer)
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate(
        &mut self,
        suffix_cache: &mut suffix::Cache,
        plan: &Plan,
        pos: usize,
        node: Option<usize>,
        totals: Vec<i32>,
        threshold: f64,
        model: &Model,
        laws: &[Law],
        deadline: Instant,
        visited: &mut usize,
    ) -> Result<ExpectationBound, String> {
        if pos == plan.order.len() {
            let feasible =
                (0..model.n).all(|t| totals[t] >= plan.lower[t] && totals[t] <= plan.upper[t]);
            return Ok(ExpectationBound {
                upper: if feasible {
                    terminal_upper(threshold)
                } else {
                    0.0
                },
                complete: true,
            });
        }
        let fi = plan.order[pos];
        let (base, edges) = if let Some(id) = node {
            let Some(n) = plan.nodes.get(id) else {
                self.diagnostics.invalid_states += 1;
                return Ok(ExpectationBound {
                    upper: 1.0,
                    complete: false,
                });
            };
            if n.terminal == Some(false) {
                return Ok(ExpectationBound {
                    upper: 0.0,
                    complete: true,
                });
            }
            if n.terminal == Some(true) {
                let feasible =
                    (0..model.n).all(|t| totals[t] >= plan.lower[t] && totals[t] <= plan.upper[t]);
                return Ok(ExpectationBound {
                    upper: if feasible {
                        terminal_upper(threshold)
                    } else {
                        0.0
                    },
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
        let q = match categorical_probabilities(&base) {
            Ok(q) => q,
            Err(_) => {
                return Ok(ExpectationBound {
                    upper: 1.0,
                    complete: false,
                })
            }
        };
        let guided = match plan.guided_weights(model, laws, fi, pos, &totals, &base) {
            Ok(w) => w,
            Err(_) => {
                return Ok(ExpectationBound {
                    upper: 1.0,
                    complete: false,
                })
            }
        };
        let gq = match categorical_probabilities(&guided) {
            Ok(q) => q,
            Err(_) => {
                return Ok(ExpectationBound {
                    upper: 1.0,
                    complete: false,
                })
            }
        };
        let mut sum = Interval::point(0.0);
        let mut complete = true;
        for o in 0..3 {
            // The native path mass is p; the baseline node vector determines
            // which paths are actually supported by q0. The guided ticket
            // probability is taken from the parent's exact guidance rule.
            let p = laws[fi].outcomes[o];
            if q[o].hi <= 0.0 || p.hi <= 0.0 {
                continue;
            }
            if !p.hi.is_finite()
                || p.hi <= 0.0
                || p.hi > 1.0
                || !gq[o].lo.is_finite()
                || gq[o].lo < 0.0
            {
                return Ok(ExpectationBound {
                    upper: 1.0,
                    complete: false,
                });
            }
            let edge = edges.iter().find(|e| e.outcome as usize == o);
            if node.is_some() && edge.is_none() {
                continue;
            }
            let ratio = Interval::point(threshold)
                .mul(Interval {
                    lo: gq[o].lo,
                    hi: gq[o].lo,
                })
                .div(Interval { lo: p.hi, hi: p.hi });
            if !ratio.lo.is_finite() || ratio.lo < 0.0 {
                self.diagnostics.invalid_states += 1;
                return Ok(ExpectationBound {
                    upper: 1.0,
                    complete: false,
                });
            }
            let child_threshold = floor_grid(ratio.lo, self.options.expectation_bits);
            if !child_threshold.is_finite() {
                return Ok(ExpectationBound {
                    upper: 1.0,
                    complete: false,
                });
            }
            let (hg, ag) = outcome_gain(model, fi, o, plan.stride);
            let f = &model.fixtures[fi];
            let mut child_totals = totals.clone();
            child_totals[f.home] = child_totals[f.home].saturating_add(hg);
            child_totals[f.away] = child_totals[f.away].saturating_add(ag);
            let next_node = if node.is_some() {
                edge.and_then(|e| e.child)
            } else {
                None
            };
            let child_key =
                match suffix_cache.key(plan, model, laws, pos + 1, next_node, &child_totals)? {
                    None => continue,
                    Some(k) => k,
                };
            let child = self.solve(
                suffix_cache,
                plan,
                child_key,
                pos + 1,
                next_node,
                child_totals,
                child_threshold,
                model,
                laws,
                deadline,
                visited,
            )?;
            complete &= child.complete;
            let term = Interval { lo: 0.0, hi: p.hi }.mul(Interval {
                lo: child.upper,
                hi: child.upper,
            });
            sum = sum.add(term);
            if !sum.hi.is_finite() {
                return Ok(ExpectationBound {
                    upper: 1.0,
                    complete: false,
                });
            }
            if Instant::now() >= deadline {
                complete = false;
                self.diagnostics.time_budget_hit = true;
            }
        }
        Ok(ExpectationBound {
            upper: sum.hi.min(1.0),
            complete,
        })
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

fn terminal_upper(threshold: f64) -> f64 {
    if threshold >= 1.0 {
        0.0
    } else if threshold == 0.0 {
        1.0
    } else {
        let value = 1.0 - threshold;
        if value.is_finite() {
            f64::from_bits(value.to_bits() + 1)
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{suffix, Graph, ResidualSource};
    use super::{floor_grid, Cache as ExpectationCache};
    use crate::{
        joint::{outcome_gain, Plan},
        law::{categorical_probabilities, Interval, Law},
    };
    use golaberto_odds::model::{Model, Request};
    use std::time::{Duration, Instant};

    fn fixture_model() -> Model {
        let request: Request = serde_json::from_str(r#"{"id":1,"phase":{"sort":"pt,w,gd,gf,name","championship":{"point_win":3,"point_draw":1,"point_loss":0}},"games":[{"id":1,"home_id":1,"away_id":2,"home_power":1.2,"away_power":0.8,"played":false},{"id":2,"home_id":1,"away_id":3,"home_power":0.9,"away_power":1.1,"played":false}],"team_groups":[{"team_id":1},{"team_id":2},{"team_id":3}]}"#).unwrap();
        Model::new(request).unwrap()
    }

    fn options() -> suffix::SuffixOptions {
        suffix::SuffixOptions {
            state_cap: 1000,
            max_remaining: 8,
            probe_nodes: 10_000,
            probe_ms: 1000,
            budget_fraction: 1.0,
            shared_expectation: true,
            expectation_bits: 8,
            expectation_max_remaining: 8,
            expectation_budget_fraction: 1.0,
            ..Default::default()
        }
    }

    fn exact_tree(
        plan: &Plan,
        model: &Model,
        laws: &[Law],
        pos: usize,
        node: Option<usize>,
        totals: &[i32],
        threshold: f64,
    ) -> f64 {
        fn walk(
            plan: &Plan,
            model: &Model,
            laws: &[Law],
            pos: usize,
            node: Option<usize>,
            totals: &[i32],
            threshold: f64,
            native_path: f64,
            guided_path: f64,
        ) -> f64 {
            if pos == plan.order.len() {
                let feasible =
                    (0..model.n).all(|t| totals[t] >= plan.lower[t] && totals[t] <= plan.upper[t]);
                return if feasible {
                    (native_path - threshold * guided_path).max(0.0)
                } else {
                    0.0
                };
            }
            let fi = plan.order[pos];
            let (base, edges) = if let Some(id) = node {
                let n = &plan.nodes[id];
                if n.terminal == Some(false) {
                    return 0.0;
                }
                if n.terminal == Some(true) {
                    let feasible = (0..model.n)
                        .all(|t| totals[t] >= plan.lower[t] && totals[t] <= plan.upper[t]);
                    return if feasible {
                        (native_path - threshold * guided_path).max(0.0)
                    } else {
                        0.0
                    };
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
            let mut sum = 0.0;
            for o in 0..3 {
                if q0[o].hi <= 0.0 || laws[fi].outcomes[o].hi <= 0.0 {
                    continue;
                }
                let edge = edges.iter().find(|e| e.outcome as usize == o);
                if node.is_some() && edge.is_none() {
                    continue;
                }
                let (hg, ag) = outcome_gain(model, fi, o, plan.stride);
                let fixture = &model.fixtures[fi];
                let mut next = totals.to_vec();
                next[fixture.home] += hg;
                next[fixture.away] += ag;
                let next_node = if node.is_some() {
                    edge.and_then(|e| e.child)
                } else {
                    None
                };
                sum += walk(
                    plan,
                    model,
                    laws,
                    pos + 1,
                    next_node,
                    &next,
                    threshold,
                    native_path * laws[fi].outcomes[o].mid(),
                    guided_path * gq[o].mid(),
                );
            }
            sum
        }
        walk(plan, model, laws, pos, node, totals, threshold, 1.0, 1.0)
    }

    fn prepared() -> (Model, Vec<Law>, Plan) {
        let model = fixture_model();
        let laws = vec![Law::new(1.2, 0.8).unwrap(), Law::new(0.9, 1.1).unwrap()];
        let mut lower = vec![0; model.n];
        let mut upper = vec![100; model.n];
        lower[0] = 5;
        upper[0] = 8;
        lower[1] = 1;
        let plan = Plan::build(
            &model,
            &laws,
            &[7, 7],
            &lower,
            &upper,
            &[0],
            1,
            1000,
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert!(plan.bound > 0.0);
        let root = plan.nodes[plan.root.unwrap()].weights;
        let base_probs = categorical_probabilities(&root).unwrap();
        let guided = plan
            .guided_weights(&model, &laws, plan.order[0], 0, &plan.base, &root)
            .unwrap();
        let guided_probs = categorical_probabilities(&guided).unwrap();
        assert!((0..3).any(|o| (base_probs[o].mid() - guided_probs[o].mid()).abs() > 1e-6));
        (model, laws, plan)
    }

    #[test]
    fn grid_rounds_down_and_is_stable() {
        for bits in 4..=20 {
            for x in [0.1, 0.5, 1.0, 1.25, 100.0] {
                let y = floor_grid(x, bits);
                assert!(y <= x);
                assert_eq!(floor_grid(y, bits), y);
            }
        }
    }

    #[test]
    fn exhaustive_tiny_tree_bounds_off_grid_thresholds_and_reuses_cells() {
        let (model, laws, plan) = prepared();
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
        let options = options();
        let now = Instant::now();
        let deadline = now + Duration::from_secs(2);
        let mut suffix_cache = suffix::Cache::new(options.clone(), now, deadline).unwrap();
        let mut cache = ExpectationCache::new(&options, now, deadline);
        let small_threshold_truth =
            exact_tree(&plan, &model, &laws, 0, plan.root, &source.totals, 1e-5);
        assert!(small_threshold_truth > 0.0);
        for threshold in [1e-5, 0.1234567, 0.4, 0.875] {
            let truth = exact_tree(
                &plan,
                &model,
                &laws,
                0,
                plan.root,
                &source.totals,
                threshold,
            );
            let bound = cache
                .probe(
                    &mut suffix_cache,
                    &source,
                    &model,
                    &laws,
                    0,
                    plan.root,
                    &source.totals,
                    threshold,
                    deadline,
                )
                .unwrap();
            assert!(bound.complete);
            assert!(
                bound.upper + 1e-13 >= truth,
                "bound {} < exact {} at {threshold}",
                bound.upper,
                truth
            );
        }
        let before = cache.diagnostics().completed_cache_hits;
        cache
            .probe(
                &mut suffix_cache,
                &source,
                &model,
                &laws,
                0,
                plan.root,
                &source.totals,
                0.4,
                deadline,
            )
            .unwrap();
        assert!(cache.diagnostics().completed_cache_hits > before);
        assert!(
            cache.diagnostics().rows > 1,
            "threshold-specific cells were not retained"
        );
    }

    #[test]
    fn tiny_node_and_cell_cutoffs_keep_unresolved_mass() {
        let (model, laws, plan) = prepared();
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
        let truth = exact_tree(&plan, &model, &laws, 0, plan.root, &source.totals, 0.37);
        for (nodes, states) in [(1, 100), (1000, 1)] {
            let mut opts = options();
            opts.probe_nodes = nodes;
            opts.state_cap = states;
            let now = Instant::now();
            let deadline = now + Duration::from_secs(1);
            let mut suffix_cache = suffix::Cache::new(opts.clone(), now, deadline).unwrap();
            let mut cache = ExpectationCache::new(&opts, now, deadline);
            let bound = cache
                .probe(
                    &mut suffix_cache,
                    &source,
                    &model,
                    &laws,
                    0,
                    plan.root,
                    &source.totals,
                    0.37,
                    deadline,
                )
                .unwrap();
            assert!(bound.upper + 1e-13 >= truth);
        }
    }

    #[test]
    fn canonicalization_and_overflow_fallback_preserve_upper_bound() {
        let (model, laws, plan) = prepared();
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
        let mut opts = options();
        opts.canonicalize_certain = true;
        let now = Instant::now();
        let deadline = now + Duration::from_secs(1);
        let mut suffix_cache = suffix::Cache::new(opts.clone(), now, deadline).unwrap();
        let mut cache = ExpectationCache::new(&opts, now, deadline);
        let truth = exact_tree(&plan, &model, &laws, 0, plan.root, &source.totals, 0.31);
        let bound = cache
            .probe(
                &mut suffix_cache,
                &source,
                &model,
                &laws,
                0,
                plan.root,
                &source.totals,
                0.31,
                deadline,
            )
            .unwrap();
        assert!(bound.complete);
        assert!(bound.upper + 1e-13 >= truth);
        let overflow = cache
            .probe(
                &mut suffix_cache,
                &source,
                &model,
                &laws,
                0,
                plan.root,
                &source.totals,
                f64::MAX,
                deadline,
            )
            .unwrap();
        assert!(!overflow.complete);
        assert_eq!(overflow.upper, 1.0);
    }
}
