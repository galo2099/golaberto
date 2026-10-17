use crate::{
    joint::Plan as JointPlan,
    law::{Interval, Law},
    score::{self, ScoreCache},
};
use golaberto_odds::{
    domains::{DomainWork, Domains},
    lookahead::RankGame,
    model::{Model, Request},
    rng::Rng,
    search::Cell,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Instant;

/// A support-preserving recovery engine. The fallback proposal is the native
/// product law itself, so its likelihood ratio is identically one. This keeps
/// every fixture and every score outcome in support even when no rare-event
/// proposal has been prepared.
pub struct Engine {
    model: Model,
    laws: Vec<Law>,
    target: usize,
    rank: usize,
    regions: Vec<Region>,
    score_cache: ScoreCache,
}

struct Region {
    root_id: usize,
    target_points: i32,
    target_wins: i32,
    above: Vec<usize>,
    upper: f64,
    impossible: bool,
    masks: Vec<u8>,
    lower: Vec<i32>,
    upper_bounds: Vec<i32>,
    plan: Option<JointPlan>,
    epoch: u64,
}

fn up(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        x
    } else {
        f64::from_bits(x.to_bits() + 1)
    }
}
fn mul_up_positive(a: f64, b: f64) -> f64 {
    let x = a * b;
    if x == 0.0 && a > 0.0 && b > 0.0 {
        f64::from_bits(1)
    } else {
        up(x)
    }
}
fn mask_mass_upper(laws: &[Law], masks: &[u8]) -> f64 {
    let mut mass = 1.0;
    for (i, law) in laws.iter().enumerate() {
        let p = (0..3)
            .filter(|&o| masks[i] & (1 << o) != 0)
            .map(|o| law.outcomes[o].hi)
            .sum::<f64>();
        mass = mul_up_positive(mass, p);
    }
    mass.min(1.0)
}

impl Engine {
    fn packed_stride(&self) -> i32 {
        (0..self.model.n)
            .map(|t| {
                self.model.base[t].wins
                    + self
                        .model
                        .fixtures
                        .iter()
                        .filter(|f| f.home == t || f.away == t)
                        .count() as i32
            })
            .max()
            .unwrap_or(0)
            + 1
    }
    pub fn new(request: Request, team_id: i32, position: usize) -> Result<Self, String> {
        if position == 0 {
            return Err("position must be one-based and positive".into());
        }
        if request.phase.bonus_points != 0 || request.phase.bonus_points_threshold != 0 {
            return Err("recovery engine supports no bonus points".into());
        }
        let model = Model::new(request)?;
        if model.request.phase.championship.point_win != 3
            || model.request.phase.championship.point_draw != 1
            || model.request.phase.championship.point_loss != 0
        {
            return Err("recovery engine requires ordinary 3/1/0 scoring".into());
        }
        let supported = [
            golaberto_odds::model::Key::Pt,
            golaberto_odds::model::Key::W,
            golaberto_odds::model::Key::Gd,
            golaberto_odds::model::Key::Gf,
            golaberto_odds::model::Key::Gp,
            golaberto_odds::model::Key::Name,
        ];
        if model.keys.as_slice() != supported {
            return Err("recovery engine requires ranking pt,w,gd,gf,gp,name".into());
        }
        let target = *model
            .indices
            .get(&team_id)
            .ok_or("target team is not in group")?;
        let rank = position - 1;
        if rank >= model.n {
            return Err("position exceeds group size".into());
        }
        let mut laws = Vec::with_capacity(model.fixtures.len());
        for f in &model.fixtures {
            laws.push(Law::new(f.home_sampler.mean, f.away_sampler.mean)?);
        }
        let stride = (0..model.n)
            .map(|t| {
                model.base[t].wins
                    + model
                        .fixtures
                        .iter()
                        .filter(|f| f.home == t || f.away == t)
                        .count() as i32
            })
            .max()
            .unwrap_or(0)
            + 1;
        let initial: Vec<u8> = laws
            .iter()
            .map(|l| {
                (0..3).fold(0, |m, o| {
                    if l.outcomes[o].hi > 0.0 {
                        m | (1 << o)
                    } else {
                        m
                    }
                })
            })
            .collect();
        let mut target_fixtures = Vec::new();
        for (i, f) in model.fixtures.iter().enumerate() {
            if f.home == target || f.away == target {
                target_fixtures.push(i)
            }
        }
        let mut roots = BTreeMap::<(i32, i32), f64>::new();
        fn enumerate_roots(
            pos: usize,
            games: &[usize],
            model: &Model,
            laws: &[Law],
            target: usize,
            stride: i32,
            points: i32,
            wins: i32,
            mass: f64,
            roots: &mut BTreeMap<(i32, i32), f64>,
        ) {
            if pos == games.len() {
                let slot = roots.entry((points, wins)).or_default();
                *slot = up(*slot + mass);
                return;
            }
            let fi = games[pos];
            let f = &model.fixtures[fi];
            for o in 0..3 {
                let p = laws[fi].outcomes[o].hi;
                if p <= 0.0 {
                    continue;
                }
                let own_win = if f.home == target { o == 2 } else { o == 0 };
                let draw = o == 1;
                let pg = if own_win {
                    model.request.phase.championship.point_win
                } else if draw {
                    model.request.phase.championship.point_draw
                } else {
                    model.request.phase.championship.point_loss
                };
                enumerate_roots(
                    pos + 1,
                    games,
                    model,
                    laws,
                    target,
                    stride,
                    points + pg,
                    wins + i32::from(own_win),
                    mul_up_positive(mass, p),
                    roots,
                );
            }
            let _ = stride;
        }
        enumerate_roots(
            0,
            &target_fixtures,
            &model,
            &laws,
            target,
            stride,
            model.base[target].points,
            model.base[target].wins,
            1.0,
            &mut roots,
        );
        let cert =
            golaberto_odds::target_limits::certify(&model, Cell { team: target, rank }, 16_000);
        let left: Vec<i32> = (0..model.n)
            .map(|t| {
                model
                    .fixtures
                    .iter()
                    .filter(|f| f.home == t || f.away == t)
                    .count() as i32
            })
            .collect();
        let packed_base: Vec<i32> = model
            .base
            .iter()
            .map(|c| c.points * stride + c.wins)
            .collect();
        let games: Vec<RankGame> = model
            .fixtures
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let mut hg = [0; 3];
                let mut ag = [0; 3];
                for o in 0..3 {
                    let (h, a) = if o == 0 {
                        (
                            model.request.phase.championship.point_loss,
                            model.request.phase.championship.point_win,
                        )
                    } else if o == 1 {
                        (
                            model.request.phase.championship.point_draw,
                            model.request.phase.championship.point_draw,
                        )
                    } else {
                        (
                            model.request.phase.championship.point_win,
                            model.request.phase.championship.point_loss,
                        )
                    };
                    hg[o] = h * stride + i32::from(o == 2);
                    ag[o] = a * stride + i32::from(o == 0);
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
            .collect();
        let mut regions = Vec::new();
        let mut root_id = 0usize;
        for (&(target_points, target_wins), &root_mass) in &roots {
            let packed = target_points * stride + target_wins;
            if !cert.allows(i64::from(packed)) {
                continue;
            }
            let mut required = Vec::new();
            let mut eligible = Vec::new();
            for t in 0..model.n {
                if t == target {
                    continue;
                }
                let lo = packed_base[t];
                let hi = lo + left[t] * (3 * stride + 1);
                if lo > packed {
                    required.push(t);
                    eligible.push(t)
                } else if hi >= packed {
                    eligible.push(t)
                }
            }
            if required.len() > rank || eligible.len() < rank {
                root_id += 1;
                continue;
            }
            let mut choices = Vec::new();
            fn choose(
                at: usize,
                need: usize,
                items: &[usize],
                current: &mut Vec<usize>,
                out: &mut Vec<Vec<usize>>,
            ) {
                if need == 0 {
                    out.push(current.clone());
                    return;
                }
                if items.len().saturating_sub(at) < need {
                    return;
                }
                for i in at..items.len() {
                    current.push(items[i]);
                    choose(i + 1, need - 1, items, current, out);
                    current.pop();
                }
            }
            let optional: Vec<_> = eligible
                .iter()
                .copied()
                .filter(|t| !required.contains(t))
                .collect();
            choose(
                0,
                rank - required.len(),
                &optional,
                &mut Vec::new(),
                &mut choices,
            );
            let masks = initial.clone();
            for above in choices {
                let mut exact = required.clone();
                exact.extend(above);
                exact.sort_unstable();
                let mut lower = vec![i32::MIN / 4; model.n];
                let mut upper_bounds = vec![i32::MAX / 4; model.n];
                lower[target] = packed;
                upper_bounds[target] = packed;
                for &t in &exact {
                    lower[t] = packed
                }
                for t in 0..model.n {
                    if t != target && !exact.contains(&t) {
                        upper_bounds[t] = packed
                    }
                }
                let mut work = DomainWork::new(250_000);
                let final_masks = if let Some(bounds) = Domains::condition_bounds_only_bounded(
                    &games,
                    &packed_base,
                    &(0..model.n).filter(|&t| t != target).collect::<Vec<_>>(),
                    rank,
                    packed,
                    &lower,
                    &upper_bounds,
                    &masks,
                    &mut work,
                ) {
                    if !bounds.feasible {
                        continue;
                    }
                    bounds.domains
                } else {
                    masks.clone()
                };
                // The score-ranked above set is checked against the full native
                // standings at draw time, including GD ties and random sorters.
                let region_upper = root_mass.min(mask_mass_upper(&laws, &final_masks));
                regions.push(Region {
                    root_id,
                    target_points,
                    target_wins,
                    above: exact,
                    upper: region_upper,
                    impossible: false,
                    masks: final_masks,
                    lower,
                    upper_bounds,
                    plan: None,
                    epoch: 0,
                });
            }
            root_id += 1;
        }
        if regions.is_empty() {
            return Err("no target roots/cohorts survived conservative enumeration".into());
        }
        Ok(Self {
            model,
            laws,
            target,
            rank,
            regions,
            score_cache: ScoreCache::new(2_000_000),
        })
    }

    pub fn describe(&self) -> Value {
        json!({
            "target":{"team_id":self.model.ids[self.target],"position":self.rank+1,"rank":self.rank},
            "fixture_count":self.model.fixtures.len(),
            "root_count":self.regions.iter().map(|r|r.root_id).max().unwrap_or(0)+1,
            "candidate_slots":self.regions.len(),
            "uncovered_upper":0.0,
            "regions":self.regions.iter().enumerate().map(|(id,r)|{let b=r.plan.as_ref().map_or(1.0,|p|p.bound);json!({"id":id,"root_id":r.root_id,"target_points":r.target_points,"target_wins":r.target_wins,"opponent_ids":r.above.iter().map(|&t|self.model.ids[t]).collect::<Vec<_>>(),"impossible":r.impossible,"upper":r.upper,
                "plan":{"prepared":r.plan.is_some(),"B":b,"states":r.plan.as_ref().map_or(0,|p|p.states),
                    "k":r.plan.as_ref().map_or(0,|p|p.selected.iter().filter(|&&t|t!=self.target).count()),"complete":r.plan.as_ref().is_some_and(|p|p.complete),"epoch":r.epoch}})}).collect::<Vec<_>>()
        })
    }

    /// Stage a full-support native-law proposal. Preparation is transactional:
    /// all validation precedes replacing the current plan.
    pub fn prepare(
        &mut self,
        id: usize,
        k: usize,
        state_cap: usize,
        time_ms: u64,
    ) -> Result<Value, String> {
        let start = Instant::now();
        if id >= self.regions.len() {
            return Err("unknown region id".into());
        }
        if state_cap == 0 {
            return Err("state_cap must be positive".into());
        }
        if k > self.model.n.saturating_sub(1) {
            return Err("k exceeds rival count".into());
        }
        let region = &self.regions[id];
        let stride = self.packed_stride();
        let mut candidates = region.above.clone();
        let target_packed = region.lower[self.target];
        let rarity = |team: usize| {
            let mut dist = BTreeMap::from([(
                self.model.base[team].points * stride + self.model.base[team].wins,
                1.0f64,
            )]);
            for (i, f) in self.model.fixtures.iter().enumerate() {
                if f.home != team && f.away != team {
                    continue;
                }
                let raw: Vec<f64> = (0..3)
                    .map(|o| {
                        if region.masks[i] & (1 << o) != 0 {
                            self.laws[i].outcomes[o].mid()
                        } else {
                            0.0
                        }
                    })
                    .collect();
                let z = raw.iter().sum::<f64>();
                if z <= 0.0 {
                    return 0.0;
                }
                let mut next = BTreeMap::new();
                for (&value, &mass) in &dist {
                    for o in 0..3 {
                        let q = raw[o] / z;
                        if q == 0.0 {
                            continue;
                        }
                        let gain = if f.home == team {
                            if o == 0 {
                                0
                            } else if o == 1 {
                                stride
                            } else {
                                3 * stride + 1
                            }
                        } else if o == 0 {
                            3 * stride + 1
                        } else if o == 1 {
                            stride
                        } else {
                            0
                        };
                        *next.entry(value + gain).or_insert(0.0) += mass * q;
                    }
                }
                dist = next;
            }
            dist.iter()
                .filter(|(score, _)| **score >= target_packed)
                .map(|(_, p)| *p)
                .sum::<f64>()
        };
        candidates.sort_by(|&a, &b| rarity(a).total_cmp(&rarity(b)));
        candidates.truncate(k);
        // The target's exact packed root total is part of every region event;
        // include it in the joint state so its root mass is sampled and capped
        // once along with the selected rival prefix constraints.
        candidates.push(self.target);
        let deadline = Instant::now() + std::time::Duration::from_millis(time_ms);
        let candidate = JointPlan::build(
            &self.model,
            &self.laws,
            &region.masks,
            &region.lower,
            &region.upper_bounds,
            &candidates,
            (0..self.model.n)
                .map(|t| {
                    self.model.base[t].wins
                        + self
                            .model
                            .fixtures
                            .iter()
                            .filter(|f| f.home == t || f.away == t)
                            .count() as i32
                })
                .max()
                .unwrap_or(0)
                + 1,
            state_cap,
            deadline,
        )?;
        if !candidate.bound.is_finite() || candidate.bound < 0.0 {
            return Err("candidate has an invalid certified bound".into());
        }
        if candidate.bound == 0.0 {
            if !candidate.complete {
                return Err("incomplete candidate cannot certify a zero event".into());
            }
            self.regions[id].epoch = self.regions[id].epoch.saturating_add(1);
            self.regions[id].impossible = true;
            self.regions[id].upper = 0.0;
            let receipt = json!({"accepted":true,"B":0.0,"states":candidate.states,"selected_rival_ids":candidate.selected.iter().filter(|&&t|t!=self.target).map(|&t|self.model.ids[t]).collect::<Vec<_>>(),"setup_ms":start.elapsed().as_secs_f64()*1000.0,"complete":true,"epoch":self.regions[id].epoch,"impossible":true});
            self.regions[id].plan = Some(candidate);
            return Ok(receipt);
        }
        if let Some(old) = self.regions[id].plan.as_ref() {
            if candidate.bound >= old.bound * (1.0 - 1e-9) {
                return Ok(
                    json!({"accepted":false,"B":old.bound,"candidate_B":candidate.bound,"states":candidate.states,"selected_rival_ids":old.selected.iter().filter(|&&t|t!=self.target).map(|&t|self.model.ids[t]).collect::<Vec<_>>(),"setup_ms":start.elapsed().as_secs_f64()*1000.0,"complete":candidate.complete,"epoch":self.regions[id].epoch}),
                );
            }
        }
        self.regions[id].epoch = self.regions[id].epoch.saturating_add(1);
        let selected_rival_ids = candidate
            .selected
            .iter()
            .filter(|&&t| t != self.target)
            .map(|&t| self.model.ids[t])
            .collect::<Vec<_>>();
        let receipt = json!({"accepted":true,"B":candidate.bound,"states":candidate.states,
            "selected_rival_ids":selected_rival_ids,"setup_ms":start.elapsed().as_secs_f64()*1000.0,
            "complete":candidate.complete,"epoch":self.regions[id].epoch});
        self.regions[id].plan = Some(candidate);
        Ok(receipt)
    }

    /// Draw exactly n complete joint proposals and integrate their native
    /// conditional score event. Each stream estimates the absolute cohort mass.
    pub fn sample(
        &mut self,
        id: usize,
        n: usize,
        seed: i64,
        depth: usize,
    ) -> Result<Value, String> {
        let start = Instant::now();
        if id >= self.regions.len() {
            return Err("unknown region id".into());
        }
        if n == 0 {
            return Err("n must be positive".into());
        }
        if self.regions[id].impossible {
            return Ok(
                json!({"id":id,"n":n,"epoch":self.regions[id].epoch,"B":0.0,"sum_lower":0.0,"sum_upper":0.0,"sum_lower_sq":0.0,"sum_upper_sq":0.0,"lower_exceedances":vec![0u64;241],"upper_exceedances":vec![0u64;241],"positive_draws":0,"elapsed_ms":start.elapsed().as_secs_f64()*1000.0,"normalized":true,"absolute_stream":false}),
            );
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
        let plan = self.regions[id].plan.as_ref().unwrap();
        let b = plan.bound;
        let mut rng = Rng::new(seed);
        let mut sum_lower = 0.0;
        let mut sum_upper = 0.0;
        let mut sum_lower_sq = 0.0;
        let mut sum_upper_sq = 0.0;
        let mut lower_counts = vec![0u64; 241];
        let mut upper_counts = vec![0u64; 241];
        let mut positives = 0u64;
        for _ in 0..n {
            let region = &self.regions[id];
            let (outcomes, weight) = plan.draw(&self.laws, &mut rng)?;
            if weight.lo < 0.0 || !weight.hi.is_finite() {
                return Err("proposal returned an invalid likelihood interval".into());
            }
            let mut root_campaigns = self.model.base.clone();
            for (i, &o) in outcomes.iter().enumerate() {
                self.model.add(
                    &mut root_campaigns,
                    i,
                    match o {
                        0 => [0, 1],
                        1 => [0, 0],
                        2 => [1, 0],
                        _ => return Err("proposal returned invalid outcome".into()),
                    },
                );
            }
            let target_campaign = &root_campaigns[self.target];
            let root_matches = target_campaign.points == region.target_points
                && target_campaign.wins == region.target_wins;
            let score = if root_matches {
                score::integrate(
                    &self.model,
                    &self.laws,
                    &outcomes,
                    &region.above,
                    self.target,
                    depth,
                    &mut rng,
                    &mut self.score_cache,
                )?
            } else {
                Interval::point(0.0)
            };
            let normalized = weight.div(Interval::point(b));
            if weight.lo > b {
                return Err(format!(
                    "observed likelihood lower bound exceeds certified cap: {} > {b}",
                    weight.lo
                ));
            }
            // `weight.hi` may exceed B by a rounding ulp because the sample's
            // interval product is wider than the direct recurrence cap. Since
            // the cap certifies the true ratio and weight.lo<=B, tighten only
            // that enclosure to the proven [0,1] normalized range.
            let normalized = Interval {
                lo: normalized.lo.max(0.0),
                hi: normalized.hi.min(1.0),
            };
            let y = normalized.mul(score).clamp_probability();
            sum_lower += y.lo;
            sum_upper += y.hi;
            sum_lower_sq += y.lo * y.lo;
            sum_upper_sq += y.hi * y.hi;
            if y.hi > 0.0 {
                positives += 1
            }
            for j in 0..=240 {
                let threshold = 2f64.powi(-(j as i32));
                if y.lo >= threshold {
                    lower_counts[j] += 1
                }
                if y.hi >= threshold {
                    upper_counts[j] += 1
                }
            }
        }
        Ok(json!({"id":id,"n":n,"epoch":self.regions[id].epoch,"B":b,
            "sum_lower":sum_lower,"sum_upper":sum_upper,"sum_lower_sq":sum_lower_sq,"sum_upper_sq":sum_upper_sq,
            "lower_exceedances":lower_counts,"upper_exceedances":upper_counts,"positive_draws":positives,
            "score_cache_states":self.score_cache.states(),"event_lower":null,"absolute_stream":false,"normalized":true,
            "elapsed_ms":start.elapsed().as_secs_f64()*1000.0}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pal12_fixture_certifies_one_loss_root_and_exact_cohorts() {
        let request: Request = serde_json::from_str(include_str!(
            "../../tests/fixtures/group-16498-2026-10-10.json"
        ))
        .unwrap();
        let engine = Engine::new(request, 16, 12).unwrap();
        assert_eq!(engine.regions.len(), 40);
        assert!(engine
            .regions
            .iter()
            .all(|r| r.root_id == 0 && r.target_points == 60 && r.target_wins == 17));
        let target_fixtures: Vec<_> = engine
            .model
            .fixtures
            .iter()
            .enumerate()
            .filter(|(_, f)| f.home == engine.target || f.away == engine.target)
            .collect();
        assert_eq!(target_fixtures.len(), 9);
        for region in &engine.regions {
            for &(i, f) in &target_fixtures {
                let loss = if f.home == engine.target { 0 } else { 2 };
                assert_eq!(
                    region.masks[i],
                    1 << loss,
                    "target-root propagation must force each loss"
                );
            }
        }
    }
}
