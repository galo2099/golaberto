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
}
