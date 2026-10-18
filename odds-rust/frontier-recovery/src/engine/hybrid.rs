use super::Engine;
use crate::joint::{HybridDraw, Plan as JointPlan};
use crate::law::Interval;
use crate::score;
use golaberto_odds::rng::Rng;
use serde_json::{json, Value};
use std::time::Instant;

fn down(x: f64) -> f64 {
    if x <= 0.0 || !x.is_finite() {
        x.max(0.0)
    } else {
        f64::from_bits(x.to_bits() - 1)
    }
}
fn up(x: f64) -> f64 {
    if x == 0.0 {
        f64::from_bits(1)
    } else if !x.is_finite() {
        x
    } else {
        f64::from_bits(x.to_bits() + 1)
    }
}
fn sub_nonnegative(a: Interval, b: Interval, cap: f64) -> Interval {
    Interval {
        lo: down(a.lo - b.hi).max(0.0),
        hi: up(a.hi - b.lo).max(0.0).min(cap),
    }
}
fn core_coefficient(v: Interval, l: Interval, h: f64) -> Result<Interval, String> {
    if v.hi == 0.0 {
        return Ok(Interval::point(0.0));
    }
    if l.lo <= 0.0 {
        return Err("guided proposal has no positive lower likelihood ratio".into());
    }
    let q = v.div(l);
    Ok(Interval {
        lo: q.lo.max(0.0).min(h),
        hi: q.hi.max(0.0).min(h),
    })
}
fn validate_weight(v: Interval, cap: f64, source: &str) -> Result<(), String> {
    if !v.lo.is_finite() || !v.hi.is_finite() || v.lo < 0.0 || v.lo > v.hi {
        return Err(format!(
            "{source} draw returned an invalid likelihood interval"
        ));
    }
    if v.lo > cap {
        return Err(format!(
            "{source} likelihood lower bound exceeds certified cap: {} > {cap}",
            v.lo
        ));
    }
    Ok(())
}
fn validate_guide_ratio(l: Interval) -> Result<(), String> {
    if !l.lo.is_finite() || !l.hi.is_finite() || l.lo < 0.0 || l.lo > l.hi {
        return Err("hybrid draw returned an invalid guide-ratio interval".into());
    }
    Ok(())
}
fn receipt(n: usize, epoch: u64, cap: f64, stats: Stats) -> Value {
    json!({"n":n,"epoch":epoch,"B":cap,"sum_lower":stats.lo,"sum_upper":stats.hi,
        "sum_lower_sq":stats.lo_sq,"sum_upper_sq":stats.hi_sq,"lower_exceedances":stats.lower,
        "upper_exceedances":stats.upper,"lower_exceedances241":stats.lower,
        "upper_exceedances241":stats.upper,"positive_draws":stats.positive,
        "normalized":true,"absolute_stream":false})
}
#[derive(Default)]
struct Stats {
    lo: f64,
    hi: f64,
    lo_sq: f64,
    hi_sq: f64,
    lower: Vec<u64>,
    upper: Vec<u64>,
    positive: u64,
}
impl Stats {
    fn new() -> Self {
        Self {
            lower: vec![0; 241],
            upper: vec![0; 241],
            ..Self::default()
        }
    }
    fn push(&mut self, value: Interval, cap: f64) {
        let y = value.div(Interval::point(cap)).clamp_probability();
        self.lo += y.lo;
        self.hi += y.hi;
        self.lo_sq += y.lo * y.lo;
        self.hi_sq += y.hi * y.hi;
        if y.hi > 0.0 {
            self.positive += 1;
        }
        for j in 0..=240 {
            let t = 2f64.powi(-(j as i32));
            if y.lo >= t {
                self.lower[j] += 1;
            }
            if y.hi >= t {
                self.upper[j] += 1;
            }
        }
    }
}

impl Engine {
    pub fn residual_certificate(
        &self,
        id: usize,
        clip_fraction: f64,
        node_limit: usize,
        time_ms: u64,
    ) -> Result<Value, String> {
        self.residual_certificate_with_options(id, clip_fraction, node_limit, time_ms, None)
    }

    pub fn residual_certificate_with_options(
        &self,
        id: usize,
        clip_fraction: f64,
        node_limit: usize,
        time_ms: u64,
        suffix_options: Option<crate::joint::SuffixOptions>,
    ) -> Result<Value, String> {
        let started = Instant::now();
        if id >= self.regions.len() {
            return Err("unknown region id".into());
        }
        if !clip_fraction.is_finite() || clip_fraction <= 0.0 || clip_fraction > 1.0 {
            return Err("clip_fraction must be finite and in (0,1]".into());
        }
        if node_limit == 0 {
            return Err("node_limit must be positive".into());
        }
        if let Some(options) = suffix_options.as_ref() {
            if options.state_cap == 0 || options.max_remaining == 0
                || options.probe_nodes == 0 || options.probe_ms == 0
                || options.score_setup_ms == 0
            {
                return Err("suffix state_cap, max_remaining, probe_nodes and probe_ms must be positive".into());
            }
            if !options.budget_fraction.is_finite()
                || options.budget_fraction <= 0.0 || options.budget_fraction > 1.0
            {
                return Err("suffix budget_fraction must be finite and in (0,1]".into());
            }
            if options.expectation_score_gd && !(options.shared_expectation && options.score_gd) {
                return Err("expectation_score_gd requires shared_expectation and score_gd".into());
            }
            if options.expectation_branch_selector && !options.expectation_score_gd {
                return Err("expectation_branch_selector requires expectation_score_gd".into());
            }
        }
        let region = &self.regions[id];
        let regime = region.regime.as_ref();
        let b = regime.map_or_else(|| region.plan.as_ref().map(|p| p.bound), |p| Some(p.bound))
            .ok_or("residual certificate requires an installed plan")?;
        let h = b * clip_fraction;
        if !b.is_finite() || b <= 0.0 || !h.is_finite() || h <= 0.0 {
            return Err("clip_fraction produces an invalid residual certificate cap".into());
        }
        let deadline = started + std::time::Duration::from_millis(time_ms);
        let certificate = match (regime, suffix_options.clone()) {
            (Some(plan), Some(options)) => {
                let score_ranking = options.score_gd.then_some((self.target, region.above.as_slice()));
                plan.residual_certificate_with_options(
                    &self.model, &self.laws, b, h, node_limit, deadline,
                    Some(options), score_ranking,
                )?
            },
            (Some(plan), None) => plan.residual_certificate(
                &self.model, &self.laws, b, h, node_limit, deadline,
            )?,
            (None, Some(options)) => {
                let mut source = region.plan.as_ref().unwrap().residual_source(
                    Interval::point(1.0), Interval::point(1.0),
                )?;
                if options.score_gd {
                    source.set_score_ranking(self.target, &region.above, None);
                }
                crate::joint::residual_certificate_with_options(
                    &[source], &self.model, &self.laws, b, h, node_limit, deadline, Some(options),
                )?
            }
            (None, None) => {
                let source = region.plan.as_ref().unwrap().residual_source(
                    Interval::point(1.0), Interval::point(1.0),
                )?;
                crate::joint::residual_certificate(
                    &[source], &self.model, &self.laws, b, h, node_limit, deadline,
                )?
            }
        };
        let snapshots: Vec<Value> = certificate.snapshots.iter().map(|s| json!({
            "expanded":s.expanded,"frontier":s.frontier,
            "expectation_upper":s.expectation_upper,"max_residual_cap":s.max_residual_cap
        })).collect();
        let depth_stats: Vec<Value> = certificate.depth_stats.iter().map(|s| json!({
            "depth":s.depth,"branches":s.branches,"expectation_upper":s.expectation_upper
        })).collect();
        let top_pending: Vec<Value> = certificate.top_pending.iter().map(|b| json!({
            "source_index":b.source,"depth":b.depth,
            "expectation_upper":b.expectation_upper,"residual_cap":b.residual_cap,
            "remaining_touched_teams":b.remaining_touched_teams
        })).collect();
        let active_width = certificate.depth_stats.iter().map(|s| s.branches).max().unwrap_or(0);
        let largest_pending = top_pending.first().cloned();
        let suffix_diagnostics = &certificate.suffix;
        let suffix = json!({
            "enabled":suffix_diagnostics.enabled,
            "options":suffix_options.as_ref().map(|o| json!({
                "state_cap":o.state_cap,"max_remaining":o.max_remaining,
                "probe_nodes":o.probe_nodes,"probe_ms":o.probe_ms,
                "budget_fraction":o.budget_fraction,
                "canonicalize_certain":o.canonicalize_certain,
                "joint_ratio":o.joint_ratio,"score_gd":o.score_gd,
                "score_setup_ms":o.score_setup_ms,
                "shared_expectation":o.shared_expectation,
                "expectation_bits":o.expectation_bits,
                "expectation_max_remaining":o.expectation_max_remaining,
                "expectation_budget_fraction":o.expectation_budget_fraction,
                "expectation_score_gd":o.expectation_score_gd,
                "expectation_branch_selector":o.expectation_branch_selector
            })),
            "calls":suffix_diagnostics.calls,"visits":suffix_diagnostics.visits,
            "cache_hits":suffix_diagnostics.cache_hits,"entries":suffix_diagnostics.entries,
            "completed_rows":suffix_diagnostics.completed_rows,
            "positive_bounds":suffix_diagnostics.positive_bounds,
            "impossible":suffix_diagnostics.impossible,"unknown":suffix_diagnostics.unknown,
            "terminals":suffix_diagnostics.terminals,
            "elapsed_ms":suffix_diagnostics.elapsed_ms,
            "cache_capacity_hit":suffix_diagnostics.cache_capacity_hit,
            "time_budget_hit":suffix_diagnostics.time_budget_hit,
            "probe_node_limit_hit":suffix_diagnostics.probe_node_limit_hit,
            "suffix_feasibility_prunes":suffix_diagnostics.suffix_feasibility_prunes,
            "guide_zero_prunes":suffix_diagnostics.guide_zero_prunes,
            "guide_prunes":suffix_diagnostics.guide_prunes,
            "tightened_branches":suffix_diagnostics.tightened_branches,
            "canonicalization_enabled":suffix_diagnostics.canonicalization_enabled,
            "canonicalized_components":suffix_diagnostics.canonicalized_components,
            "exact_components":suffix_diagnostics.exact_components,
            "canonicalized_key_calls":suffix_diagnostics.canonicalized_key_calls,
            "canonicalized_cache_hits":suffix_diagnostics.canonicalized_cache_hits,
            "max_active_components":suffix_diagnostics.max_active_components,
            "max_exact_components":suffix_diagnostics.max_exact_components,
            "unsupported_plans":suffix_diagnostics.unsupported_plans,
            "min_positive_g":suffix_diagnostics.min_positive_g,
            "max_positive_g":suffix_diagnostics.max_positive_g,
            "joint_ratio_enabled":suffix_diagnostics.joint_ratio_enabled,
            "joint_ratio_finite_bounds":suffix_diagnostics.joint_ratio_finite_bounds,
            "min_joint_ratio_cap":suffix_diagnostics.min_joint_ratio_cap,
            "max_joint_ratio_cap":suffix_diagnostics.max_joint_ratio_cap,
            "joint_ratio_tightenings":suffix_diagnostics.joint_ratio_tightenings,
            "joint_ratio_zero_bounds":suffix_diagnostics.joint_ratio_zero_bounds
        });
        let mut receipt = json!({
            "operation":"residual_certificate","id":id,"epoch":region.epoch,"B":b,"H":h,
            "mode":if regime.is_some(){"regime"}else{"joint"},
            "source_count":certificate.source_count,
            "expectation_upper":certificate.expectation_upper,
            "max_residual_cap":certificate.max_residual_cap,
            "initial_bounds":certificate.initial_bounds,
            "complete":certificate.complete,
            "expanded":certificate.expanded,"frontier":certificate.frontier,
            "retained_unknown_nodes":certificate.frontier,
            "frontier_min_depth":certificate.frontier_min_depth,
            "frontier_max_depth":certificate.frontier_max_depth,
            "prunes":certificate.prunes,"leaves":certificate.leaves,
            "zero_pw_prunes":certificate.zero_pw_prunes,
            "score_gd":certificate.score_gd,
            "score_zero_prunes":certificate.score_zero_prunes,
            "shared_expectation":certificate.shared_expectation,
            "zero_residual_prunes":certificate.zero_residual_prunes,
            "final_refresh_applied":certificate.final_refresh_applied,
            "final_refresh_skipped":certificate.final_refresh_skipped,
            "final_refresh_ms":certificate.final_refresh_ms,
            "aggregation_ms":certificate.aggregation_ms,
            "zero_residual_leaves":certificate.zero_residual_leaves,
            "feasible_leaves":certificate.feasible_leaves,
            "memory_limited":certificate.memory_limited,
            "pmf_cache_limited":certificate.pmf_limited,
            "depth_stats":depth_stats,"top_pending":top_pending,
            "active_width":active_width,"largest_pending":largest_pending,
            "suffix":suffix,
            "node_limit":node_limit,"time_ms":time_ms,
            "elapsed_ms":certificate.elapsed.as_secs_f64()*1000.0,
            "audit_elapsed_ms":started.elapsed().as_secs_f64()*1000.0,
            "snapshots":snapshots
        });
        receipt["score_shared_expectation"] = json!(certificate.score_shared_expectation);
        receipt["score_selector"] = json!(certificate.score_selector);
        Ok(receipt)
    }

    pub fn sample_hybrid(
        &mut self,
        id: usize,
        n_total: usize,
        seed: i64,
        depth: usize,
        clip_fraction: f64,
    ) -> Result<Value, String> {
        self.sample_hybrid_with_options(id, n_total, seed, depth, clip_fraction, true)
    }

    pub fn sample_hybrid_with_options(
        &mut self,
        id: usize,
        n_total: usize,
        seed: i64,
        depth: usize,
        clip_fraction: f64,
        lazy_residual: bool,
    ) -> Result<Value, String> {
        let started = Instant::now();
        if id >= self.regions.len() {
            return Err("unknown region id".into());
        }
        if n_total < 2 || n_total % 2 != 0 {
            return Err("n_total must be even and at least 2".into());
        }
        if !clip_fraction.is_finite() || clip_fraction <= 0.0 || clip_fraction > 1.0 {
            return Err("clip_fraction must be finite and in (0,1]".into());
        }
        let m = n_total / 2;
        if self.regions[id].impossible {
            return Err("hybrid sampling requires a positive installed proposal cap".into());
        }
        if self.regions[id].plan.is_none() {
            let region = &self.regions[id];
            let stride = self.packed_stride();
            let deadline = Instant::now();
            self.regions[id].plan = Some(JointPlan::build(
                &self.model,
                &self.laws,
                &region.masks,
                &region.lower,
                &region.upper_bounds,
                &[self.target],
                stride,
                0,
                deadline,
            )?);
        }
        let b = self.regions[id].regime.as_ref().map_or_else(
            || self.regions[id].plan.as_ref().unwrap().bound,
            |p| p.bound,
        );
        let h = b * clip_fraction;
        if !b.is_finite() || b <= 0.0 || !h.is_finite() || h <= 0.0 {
            return Err("clip_fraction produces an invalid hybrid cap".into());
        }
        let mut rng = Rng::new(seed);
        let mut core = Stats::new();
        let mut residual = Stats::new();
        let mut baseline = Stats::new();
        let mut guide_ratio_min = f64::INFINITY;
        let mut guide_ratio_max: f64 = 0.0;
        let core_started = Instant::now();
        for _ in 0..m {
            let (outcomes, v, l, fixed_scores) = {
                let plan = self.regions[id].plan.as_ref().unwrap();
                if let Some(r) = self.regions[id].regime.as_ref() {
                    r.draw_hybrid(&self.model, &self.laws, &mut rng, true)?
                } else {
                    let HybridDraw {
                        outcomes,
                        baseline_weight,
                        guide_over_baseline,
                    } = plan.draw_hybrid(&self.model, &self.laws, &mut rng, true)?;
                    (
                        outcomes,
                        baseline_weight,
                        guide_over_baseline,
                        vec![None; self.model.fixtures.len()],
                    )
                }
            };
            validate_weight(v, b, "guided")?;
            validate_guide_ratio(l)?;
            guide_ratio_min = guide_ratio_min.min(l.lo);
            guide_ratio_max = guide_ratio_max.max(l.hi);
            let base_result = self.score_draw(id, depth, &mut rng, &outcomes, v, &fixed_scores)?;
            let coeff = core_coefficient(v, l, h)?;
            core.push(coeff.mul(base_result), h);
        }
        let core_elapsed_ms = core_started.elapsed().as_secs_f64() * 1000.0;
        let baseline_started = Instant::now();
        let mut replay_elapsed = std::time::Duration::ZERO;
        let mut guide_evaluations = 0usize;
        let mut guide_skips = 0usize;
        for _ in 0..m {
            let (outcomes, v, l, fixed_scores, atom_context) = {
                let plan = self.regions[id].plan.as_ref().unwrap();
                if let Some(r) = self.regions[id].regime.as_ref() {
                    if lazy_residual {
                        let (o, v, fixed, context) = r.draw_baseline_hybrid_context(&self.model, &self.laws, &mut rng)?;
                        (o, v, Interval::point(0.0), fixed, Some(context))
                    } else {
                        let (o, v, l, fixed, context) = r.draw_hybrid_with_context(&self.model, &self.laws, &mut rng, false)?;
                        (o, v, l, fixed, Some(context))
                    }
                } else {
                    if lazy_residual {
                        let (outcomes, baseline_weight) = plan.draw(&self.laws, &mut rng)?;
                        (outcomes, baseline_weight, Interval::point(0.0), vec![None; self.model.fixtures.len()], None)
                    } else {
                        let d = plan.draw_hybrid(&self.model, &self.laws, &mut rng, false)?;
                        (d.outcomes, d.baseline_weight, d.guide_over_baseline, vec![None; self.model.fixtures.len()], None)
                    }
                }
            };
            validate_weight(v, b, "baseline")?;
            let score = self.score_draw(id, depth, &mut rng, &outcomes, v, &fixed_scores)?;
            let l = if lazy_residual && score.hi == 0.0 {
                guide_skips += 1;
                Interval::point(0.0)
            } else {
                let ratio = if !lazy_residual {
                    l
                } else if let Some(context) = atom_context {
                    let replay_started = Instant::now();
                    let ratio = self.regions[id].regime.as_ref().unwrap().replay_guide_ratio(
                        context, &self.model, &self.laws, &outcomes,
                    )?;
                    replay_elapsed += replay_started.elapsed();
                    ratio
                } else {
                    let replay_started = Instant::now();
                    let ratio = self.regions[id].plan.as_ref().unwrap().guide_ratio(&self.model, &self.laws, &outcomes)?;
                    replay_elapsed += replay_started.elapsed();
                    ratio
                };
                guide_evaluations += 1;
                validate_guide_ratio(ratio)?;
                guide_ratio_min = guide_ratio_min.min(ratio.lo);
                guide_ratio_max = guide_ratio_max.max(ratio.hi);
                ratio
            };
            let residual_value = if lazy_residual && score.hi == 0.0 {
                Interval::point(0.0)
            } else {
                sub_nonnegative(v, l.mul(Interval::point(h)), b).mul(score)
            };
            residual.push(residual_value, b);
            baseline.push(v.mul(score), b);
        }
        let baseline_elapsed_ms = baseline_started.elapsed().as_secs_f64() * 1000.0;
        let core_r = receipt(m, self.regions[id].epoch, h, core);
        let residual_r = receipt(m, self.regions[id].epoch, b, residual);
        let baseline_r = receipt(m, self.regions[id].epoch, b, baseline);
        let mut out = json!({"id":id,"n":n_total,"epoch":self.regions[id].epoch,"B":b,"H":h,"clip_fraction":clip_fraction,"mode":"hybrid","elapsed_ms":started.elapsed().as_secs_f64()*1000.0,"normalized":true,"absolute_stream":false,"core":core_r,"residual":residual_r,"baseline":baseline_r,"core_positive_draws":core_r["positive_draws"],"residual_positive_draws":residual_r["positive_draws"],"guide_ratio_min":if guide_ratio_min.is_finite(){json!(guide_ratio_min)}else{Value::Null},"guide_ratio_max":guide_ratio_max,"guide_ratio_scope":"evaluated_core_and_residual_draws","lazy_residual":lazy_residual,"residual_guide_evaluations":guide_evaluations,"residual_guide_skips":guide_skips,"hybrid_core_elapsed_ms":core_elapsed_ms,"hybrid_baseline_elapsed_ms":baseline_elapsed_ms,"hybrid_replay_elapsed_ms":replay_elapsed.as_secs_f64()*1000.0,"sum_lower":baseline_r["sum_lower"],"sum_upper":baseline_r["sum_upper"],"sum_lower_sq":baseline_r["sum_lower_sq"],"sum_upper_sq":baseline_r["sum_upper_sq"],"lower_exceedances":baseline_r["lower_exceedances"],"upper_exceedances":baseline_r["upper_exceedances"],"positive_draws":baseline_r["positive_draws"]});
        if let Some(obj) = out.as_object_mut() {
            obj.insert(
                "command_elapsed_ms".into(),
                json!(started.elapsed().as_secs_f64() * 1000.0),
            );
        }
        Ok(out)
    }
    fn score_draw(
        &mut self,
        id: usize,
        depth: usize,
        rng: &mut Rng,
        outcomes: &[u8],
        weight: Interval,
        fixed_scores: &[Option<[i32; 2]>],
    ) -> Result<Interval, String> {
        let region = &self.regions[id];
        let mut campaigns = self.model.base.clone();
        for (i, &o) in outcomes.iter().enumerate() {
            self.model.add(
                &mut campaigns,
                i,
                match o {
                    0 => [0, 1],
                    1 => [0, 0],
                    2 => [1, 0],
                    _ => return Err("proposal returned invalid outcome".into()),
                },
            );
        }
        let target = &campaigns[self.target];
        if target.points != region.target_points
            || target.wins != region.target_wins
            || weight.hi == 0.0
        {
            return Ok(Interval::point(0.0));
        }
        score::integrate_fixed(
            &self.model,
            &self.laws,
            outcomes,
            &region.above,
            self.target,
            depth,
            rng,
            &mut self.score_cache,
            fixed_scores,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use golaberto_odds::model::Request;

    fn tiny_engine() -> Engine {
        let request: Request = serde_json::from_str(
            r#"{"id":1,"phase":{"sort":"pt,w,gd,gf,gp,name","championship":{"point_win":3,"point_draw":1,"point_loss":0}},"games":[{"id":1,"home_id":1,"away_id":2,"home_power":1.0,"away_power":1.0,"played":false}],"team_groups":[{"team_id":1},{"team_id":2}]}"#,
        )
        .unwrap();
        Engine::new(request, 1, 1).unwrap()
    }

    #[test]
    fn clipping_decomposition_encloses_baseline_weight() {
        let b = 0.8;
        let h = 0.1;
        let v = Interval::point(0.6);
        let l = Interval::point(2.0);
        let core = core_coefficient(v, l, h).unwrap();
        let residual = sub_nonnegative(v, l.mul(Interval::point(h)), b);
        assert!(core.lo <= h && core.hi <= h);
        assert!(residual.lo >= 0.0 && residual.hi <= b);
        let sum = core.mul(l).add(residual);
        assert!(sum.lo <= v.lo && sum.hi >= v.hi);
        assert!(sum.lo.is_finite() && sum.hi.is_finite() && sum.lo <= sum.hi);
    }

    #[test]
    fn normalized_stats_keep_threshold_enclosure() {
        let threshold = 2f64.powi(-144);
        let cap = 1.1;
        let value = Interval::point(4.932570594423356e-44);
        assert_eq!(value.hi / cap, threshold);
        let mut stats = Stats::new();
        stats.push(value, cap);
        assert_eq!(stats.lower[144], 0);
        assert_eq!(stats.upper[144], 1);
    }

    #[test]
    fn cap_guard_rejects_a_regime_weight_above_its_certified_bound() {
        assert!(validate_weight(Interval::point(0.81), 0.8, "regime").is_err());
        assert!(validate_weight(Interval::point(0.81), 0.8, "guided").is_err());
        assert!(validate_weight(Interval::point(0.81), 0.8, "baseline").is_err());
        assert!(validate_weight(Interval::point(0.8), 0.8, "regime").is_ok());
        assert!(validate_weight(Interval { lo: 0.9, hi: 0.8 }, 1.0, "regime").is_err());
    }

    #[test]
    fn hybrid_rejects_invalid_sample_counts_and_clip_fractions() {
        let mut engine = tiny_engine();
        assert!(engine.sample_hybrid(0, 1, 1, 0, 0.01).is_err());
        assert!(engine.sample_hybrid(0, 3, 1, 0, 0.01).is_err());
        assert!(engine.sample_hybrid(0, 2, 1, 0, 0.0).is_err());
        assert!(engine.sample_hybrid(0, 2, 1, 0, f64::NAN).is_err());
        assert!(engine.sample_hybrid(0, 2, 1, 0, 1.01).is_err());
    }

    #[test]
    fn lazy_and_eager_hybrid_keep_the_same_fixed_stream_receipts() {
        let mut eager = tiny_engine();
        let mut lazy = tiny_engine();
        let eager = eager.sample_hybrid_with_options(0, 64, 8123, 0, 0.2, false).unwrap();
        let lazy = lazy.sample_hybrid_with_options(0, 64, 8123, 0, 0.2, true).unwrap();
        for key in ["core", "residual", "baseline"] {
            assert_eq!(eager[key], lazy[key], "receipt mismatch in {key}");
        }
        let evaluations = lazy["residual_guide_evaluations"].as_u64().unwrap();
        let skips = lazy["residual_guide_skips"].as_u64().unwrap();
        assert_eq!(evaluations + skips, 32);
        assert!(evaluations > 0 && skips > 0);
    }

    #[test]
    fn residual_certificate_is_read_only_and_validates_limits() {
        let mut engine = tiny_engine();
        engine.prepare(0, 1, 100, 100).unwrap();
        let epoch = engine.regions[0].epoch;
        let bound = engine.regions[0].plan.as_ref().unwrap().bound;
        assert!(engine.residual_certificate(0, 0.0, 10, 10).is_err());
        assert!(engine.residual_certificate(0, 0.1, 0, 10).is_err());
        assert!(engine.residual_certificate(usize::MAX, 0.1, 10, 10).is_err());
        let result = engine.residual_certificate(0, 0.1, 100, 100).unwrap();
        assert!(result["expectation_upper"].as_f64().unwrap() >= 0.0);
        assert!(result["max_residual_cap"].as_f64().unwrap() <= bound);
        assert_eq!(result["suffix"]["enabled"], false);
        let bad_options = crate::joint::SuffixOptions { state_cap: 0, ..Default::default() };
        assert!(engine.residual_certificate_with_options(0, 0.1, 100, 100, Some(bad_options)).is_err());
        let bad_expectation_score = crate::joint::SuffixOptions {
            expectation_score_gd: true,
            ..Default::default()
        };
        assert!(engine.residual_certificate_with_options(0, 0.1, 100, 100, Some(bad_expectation_score)).is_err());
        let epoch = engine.regions[0].epoch;
        let options = crate::joint::SuffixOptions {
            state_cap: 10_000,
            max_remaining: 2,
            probe_nodes: 100,
            probe_ms: 5,
            budget_fraction: 0.1,
            canonicalize_certain: true,
            joint_ratio: true,
            score_gd: true,
            ..Default::default()
        };
        let enabled = engine.residual_certificate_with_options(0, 0.1, 100, 100, Some(options)).unwrap();
        assert_eq!(enabled["suffix"]["enabled"], true);
        assert_eq!(enabled["suffix"]["options"]["max_remaining"], 2);
        assert_eq!(enabled["suffix"]["options"]["canonicalize_certain"], true);
        assert_eq!(enabled["suffix"]["options"]["joint_ratio"], true);
        assert_eq!(enabled["suffix"]["options"]["score_gd"], true);
        assert_eq!(enabled["score_gd"]["enabled"], true);
        assert_eq!(enabled["suffix"]["joint_ratio_enabled"], true);
        assert_eq!(enabled["suffix"]["canonicalization_enabled"], true);
        assert_eq!(engine.regions[0].epoch, epoch);
        assert_eq!(engine.regions[0].plan.as_ref().unwrap().bound, bound);
    }
}
