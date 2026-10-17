//! Native score rejection plans for rare pairwise goal-difference events.
//!
//! WDL outcomes incident to either team are fixed by the enclosing region.
//! Scores are then drawn from the native law conditional on those outcomes,
//! and the complete incident score vector is rejected until the pairwise GD
//! event holds.  The plan's normalizer is the probability of that rejection
//! event under the conditional native score law.

use crate::{
    gd_bounds::GdCache,
    law::{Interval, Law},
};
use golaberto_odds::{model::Model, rng::Rng};
use std::time::Instant;

const MAX_VECTOR_TRIALS: usize = 1_000_000;

pub struct PairPlan {
    pub normalizer: Interval,
    pub positive: usize,
    pub negative: usize,
    fixed_incident: Vec<(usize, u8)>,
}

impl PairPlan {
    pub fn build(
        model: &Model,
        laws: &[Law],
        masks: &[u8],
        positive: usize,
        negative: usize,
        deadline: Instant,
        cache: &mut GdCache,
    ) -> Result<Option<Self>, String> {
        if laws.len() != model.fixtures.len() || masks.len() != laws.len() {
            return Err("pair plan fixture length mismatch".into());
        }
        if positive >= model.n || negative >= model.n || positive == negative {
            return Err("pair plan team index out of range".into());
        }
        let mut fixed_incident = Vec::new();
        for (i, f) in model.fixtures.iter().enumerate() {
            if f.home != positive && f.away != positive && f.home != negative && f.away != negative
            {
                continue;
            }
            if masks[i] & !0b111 != 0 {
                return Err("pair plan WDL mask has unsupported bits".into());
            }
            if masks[i].count_ones() != 1 {
                return Ok(None);
            }
            let outcome = masks[i].trailing_zeros() as u8;
            if laws[i].outcomes[outcome as usize].hi <= 0.0 {
                return Ok(None);
            }
            fixed_incident.push((i, outcome));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        let Some(normalizer) =
            cache.probability(model, laws, masks, positive, negative, deadline)?
        else {
            return Ok(None);
        };
        // A zero upper endpoint is a certified zero-event plan and is useful
        // to callers doing support accounting. A positive but expensive
        // rejection plan is left to the ordinary proposal.
        if normalizer.hi > 0.0 && normalizer.lo < 0.001 {
            return Ok(None);
        }
        Ok(Some(Self {
            normalizer,
            positive,
            negative,
            fixed_incident,
        }))
    }

    /// Draw the native conditional score vector given the fixed incident WDLs
    /// and the inclusive GD event. Entries are indexed by model fixture;
    /// nonincident fixtures remain None for the downstream score integrator.
    pub fn draw(&self, model: &Model, rng: &mut Rng) -> Result<Vec<Option<[i32; 2]>>, String> {
        if self.normalizer.hi == 0.0 {
            return Err("cannot draw from certified zero pair event".into());
        }
        let mut fixed = vec![None; model.fixtures.len()];
        for &(i, o) in &self.fixed_incident {
            if i >= fixed.len() || o > 2 {
                return Err("pair plan does not match model fixtures".into());
            }
            fixed[i] = Some(o);
        }
        if self
            .fixed_incident
            .iter()
            .any(|&(i, _)| i >= model.fixtures.len())
        {
            return Err("pair plan does not match model fixtures".into());
        }
        if self.positive >= model.n || self.negative >= model.n || self.positive == self.negative {
            return Err("pair plan does not match model teams".into());
        }
        let mut recorded = vec![false; model.fixtures.len()];
        for &(i, _) in &self.fixed_incident {
            if recorded[i] {
                return Err("duplicate fixture in pair plan".into());
            }
            recorded[i] = true;
        }
        if model.fixtures.iter().enumerate().any(|(i, f)| {
            recorded[i]
                != (f.home == self.positive
                    || f.away == self.positive
                    || f.home == self.negative
                    || f.away == self.negative)
        }) {
            return Err("pair plan does not match model fixtures".into());
        }
        let delta0 = model.base[self.positive].gf
            - model.base[self.positive].ga
            - (model.base[self.negative].gf - model.base[self.negative].ga);
        for (trial, _) in (0..MAX_VECTOR_TRIALS).enumerate() {
            let mut scores = vec![None; model.fixtures.len()];
            let mut delta = delta0;
            for &(i, outcome) in &self.fixed_incident {
                let f = &model.fixtures[i];
                // Rejection from the native independent Poissons is required
                // here: the legacy conditional CDF can differ by ulps from
                // the native Poisson CDF used by the q normalizer.
                let score = loop {
                    let h = f.home_sampler.sample(rng);
                    let a = f.away_sampler.sample(rng);
                    let observed = if h < a {
                        0
                    } else if h == a {
                        1
                    } else {
                        2
                    };
                    if observed == outcome {
                        break [h, a];
                    }
                };
                scores[i] = Some(score);
                let gd = score[0] - score[1];
                if f.home == self.positive {
                    delta += gd;
                }
                if f.away == self.positive {
                    delta -= gd;
                }
                if f.home == self.negative {
                    delta -= gd;
                }
                if f.away == self.negative {
                    delta += gd;
                }
            }
            if delta >= 0 {
                return Ok(scores);
            }
            if trial + 1 == MAX_VECTOR_TRIALS {
                return Err(
                    "pair rejection sampler exceeded one million complete score vectors".into(),
                );
            }
        }
        unreachable!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use golaberto_odds::model::{Model, Request};
    use std::time::Duration;

    fn model() -> Model {
        let request: Request = serde_json::from_str(
            r#"{"id":1,"phase":{"sort":"pt,w,gd,gf,name","championship":{"point_win":3,"point_draw":1,"point_loss":0}},"games":[{"id":1,"home_id":2,"away_id":3,"home_score":2,"away_score":0,"played":true},{"id":2,"home_id":1,"away_id":2,"home_power":1.0,"away_power":1.0,"played":false},{"id":3,"home_id":1,"away_id":4,"home_power":1.0,"away_power":1.0,"played":false},{"id":4,"home_id":3,"away_id":4,"home_power":1.0,"away_power":1.0,"played":false}],"team_groups":[{"team_id":1},{"team_id":2},{"team_id":3},{"team_id":4}]}"#,
        ).unwrap();
        Model::new(request).unwrap()
    }

    #[test]
    fn shared_fixture_is_counted_with_opposite_signs_and_q_matches_cache() {
        let m = model();
        let laws = vec![Law::new(1., 1.).unwrap(); 3];
        let masks = [1 << 0, 1 << 2, 7];
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut cache = GdCache::new(8);
        let q = cache
            .probability(&m, &laws, &masks, 0, 1, deadline)
            .unwrap()
            .unwrap();
        let plan = PairPlan::build(&m, &laws, &masks, 0, 1, deadline, &mut cache)
            .unwrap()
            .unwrap();
        assert_eq!(plan.normalizer, q);
        // The initial GD gap is -2; this shared fixture contributes twice
        // the signed goal difference because its endpoints are the pair.
        let mut exact = Interval::point(0.);
        for shared in &laws[0].pairs[0] {
            for private in &laws[1].pairs[2] {
                let gd = -2 + 2 * (shared.home - shared.away) + private.home - private.away;
                if gd >= 0 {
                    exact = exact.add(
                        shared
                            .mass
                            .div(laws[0].outcomes[0])
                            .mul(private.mass.div(laws[1].outcomes[2])),
                    );
                }
            }
        }
        assert!(
            q.lo <= exact.mid() && q.hi >= exact.mid(),
            "q={q:?}, exact={exact:?}"
        );
        let mut rng = Rng::new(71);
        let draw = plan.draw(&m, &mut rng).unwrap();
        assert_eq!(draw.iter().filter(|x| x.is_some()).count(), 2);
        let shared_score = draw[0].unwrap();
        let private_score = draw[1].unwrap();
        assert!(draw[2].is_none(), "nonincident score must remain absent");
        assert!(
            -2 + 2 * (shared_score[0] - shared_score[1]) + private_score[0] - private_score[1] >= 0
        );
        for &(i, outcome) in &plan.fixed_incident {
            let score = draw[i].unwrap();
            let sampled_outcome = if score[0] < score[1] {
                0
            } else if score[0] == score[1] {
                1
            } else {
                2
            };
            assert_eq!(sampled_outcome, outcome);
        }
    }

    #[test]
    fn unfixed_and_expensive_events_are_not_planned() {
        let m = model();
        let laws = vec![Law::new(1., 1.).unwrap(); 3];
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut cache = GdCache::new(8);
        assert!(
            PairPlan::build(&m, &laws, &[7, 4, 7], 0, 1, deadline, &mut cache)
                .unwrap()
                .is_none()
        );

        // Base GD is -2. Fixed masks below make the chance of recovering the
        // gap small enough to trigger the documented rejection-cost cutoff.
        let impossibleish = [1 << 0, 1 << 0, 7];
        let mut cache = GdCache::new(8);
        let q = cache
            .probability(&m, &laws, &impossibleish, 0, 1, deadline)
            .unwrap()
            .unwrap();
        let p = PairPlan::build(&m, &laws, &impossibleish, 0, 1, deadline, &mut cache).unwrap();
        if q.hi == 0.0 || q.lo < 0.001 {
            assert!(p.is_none() || p.as_ref().unwrap().normalizer.hi == 0.0);
        } else {
            assert!(p.is_some());
        }
        assert!(
            PairPlan::build(&m, &laws, &[1 << 0, 1 << 2, 7], 0, 1, deadline, &mut cache)
                .unwrap()
                .is_some()
        );
        assert!(PairPlan::build(&m, &laws, &[8, 1 << 2, 7], 0, 1, deadline, &mut cache).is_err());
    }
}
