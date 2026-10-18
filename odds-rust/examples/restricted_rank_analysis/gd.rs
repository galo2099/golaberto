//! Private goal-difference integration given a WDL path and sampled shared scores.
//!
//! Fixtures shared by two members of the tied points/wins cohort are sampled
//! jointly. Fixtures with exactly one cohort endpoint are integrated into
//! that team's private goal-difference distribution.

use golaberto_odds::{model::Model, rng::Rng, sampling::masses};
use std::collections::HashMap;

const MAX_CONVOLUTION_PAIRS: usize = 2_000_000;
const MAX_CACHED_KERNELS: usize = 100_000;

#[derive(Clone, Copy, Debug, Default)]
pub struct Projection {
    pub strict: f64,
    pub weak: f64,
    pub tied_rivals: usize,
    pub shared_games: usize,
}

/// Integrates goal difference conditional on a fixture-indexed WDL path.
pub struct Integrator {
    /// Conditional home goal-difference kernels, keyed by (fixture, outcome).
    /// Away-endpoint kernels are the reflected home kernels.
    kernels: HashMap<(usize, u8), Vec<(i32, f64)>>,
}

#[derive(Clone, Debug)]
struct Distribution {
    min: i32,
    mass: Vec<f64>,
    cdf: Vec<f64>,
}

impl Distribution {
    fn point(value: i32) -> Self {
        Self {
            min: value,
            mass: vec![1.0],
            cdf: vec![1.0],
        }
    }

    fn from_mass(min: i32, mass: Vec<f64>) -> Self {
        let mut cdf = Vec::with_capacity(mass.len());
        let mut sum = 0.0;
        for &value in &mass {
            sum += value;
            cdf.push(sum);
        }
        Self { min, mass, cdf }
    }

    fn less_than(&self, threshold: i32) -> f64 {
        let count =
            (i64::from(threshold) - i64::from(self.min)).clamp(0, self.mass.len() as i64) as usize;
        if count == 0 {
            0.0
        } else {
            self.cdf[count - 1]
        }
    }

    fn less_equal(&self, threshold: i32) -> f64 {
        let count = (i64::from(threshold) - i64::from(self.min) + 1)
            .clamp(0, self.mass.len() as i64) as usize;
        if count == 0 {
            0.0
        } else {
            self.cdf[count - 1]
        }
    }

    fn convolve(&self, kernel: &[(i32, f64)], work: &mut usize) -> Result<Self, String> {
        let Some(kernel_min) = kernel.iter().map(|entry| entry.0).min() else {
            return Err("conditional score kernel has empty support".into());
        };
        let kernel_max = kernel.iter().map(|x| x.0).max().unwrap_or(kernel_min);
        let min = self.min.saturating_add(kernel_min);
        let max = (self.min + self.mass.len() as i32 - 1).saturating_add(kernel_max);
        let len = (i64::from(max) - i64::from(min) + 1) as usize;
        let mut next = vec![0.0; len];
        for (i, &left) in self.mass.iter().enumerate() {
            if left == 0.0 {
                continue;
            }
            for &(delta, right) in kernel {
                *work = work.saturating_add(1);
                if *work > MAX_CONVOLUTION_PAIRS {
                    return Err(format!("goal-difference convolution exceeded {MAX_CONVOLUTION_PAIRS} score-pair operations"));
                }
                if right > 0.0 {
                    let index = i64::from(self.min) + i as i64 + i64::from(delta) - i64::from(min);
                    next[index as usize] += left * right;
                }
            }
        }
        Ok(Self::from_mass(min, next))
    }
}

impl Integrator {
    pub fn new(_model: &Model) -> Result<Self, String> {
        Ok(Self {
            kernels: HashMap::new(),
        })
    }

    pub fn evaluate(
        &mut self,
        model: &Model,
        outcomes: &[u8],
        target: usize,
        rng: &mut Rng,
    ) -> Result<Projection, String> {
        if target >= model.n {
            return Err(format!(
                "target index {target} is outside the {}-team group",
                model.n
            ));
        }
        if outcomes.len() != model.fixtures.len() {
            return Err(format!(
                "expected {} fixture outcomes, received {}",
                model.fixtures.len(),
                outcomes.len()
            ));
        }
        for (fixture_index, (&outcome, fixture)) in outcomes.iter().zip(&model.fixtures).enumerate()
        {
            if outcome > 2 {
                return Err(format!(
                    "fixture {fixture_index} has invalid WDL outcome {outcome}"
                ));
            }
            if fixture.prob[outcome as usize] <= 0.0 {
                return Err(format!(
                    "fixture {fixture_index} has zero mass for WDL outcome {outcome}"
                ));
            }
        }

        let mut campaigns = model.base.clone();
        for (fixture_index, &outcome) in outcomes.iter().enumerate() {
            let score = match outcome {
                0 => [0, 1],
                1 => [0, 0],
                2 => [1, 0],
                _ => unreachable!(),
            };
            model.add(&mut campaigns, fixture_index, score);
        }
        let cohort = (0..model.n)
            .filter(|&team| {
                campaigns[team].points == campaigns[target].points
                    && campaigns[team].wins == campaigns[target].wins
            })
            .collect::<Vec<_>>();
        if cohort.len() == 1 {
            return Ok(Projection::default());
        }

        let mut cohort_slot = vec![None; campaigns.len()];
        for (slot, &team) in cohort.iter().enumerate() {
            cohort_slot[team] = Some(slot);
        }
        let target_slot = cohort_slot[target].expect("target is in its own cohort");
        let mut shared_gd = vec![0i32; cohort.len()];
        let mut private_fixtures = vec![Vec::<(usize, bool)>::new(); cohort.len()];
        let mut shared_games = 0usize;

        for (fixture_index, fixture) in model.fixtures.iter().enumerate() {
            let home = cohort_slot.get(fixture.home).copied().flatten();
            let away = cohort_slot.get(fixture.away).copied().flatten();
            match (home, away) {
                (Some(h), Some(a)) => {
                    let score = fixture.scores.sample(outcomes[fixture_index] as usize, rng);
                    shared_gd[h] += score[0] - score[1];
                    shared_gd[a] += score[1] - score[0];
                    shared_games += 1;
                }
                (Some(slot), None) => private_fixtures[slot].push((fixture_index, true)),
                (None, Some(slot)) => private_fixtures[slot].push((fixture_index, false)),
                (None, None) => {}
            }
        }

        let mut work = 0usize;
        let mut private = Vec::with_capacity(cohort.len());
        for fixtures in &private_fixtures {
            let mut distribution = Distribution::point(0);
            for &(fixture_index, is_home) in fixtures {
                let outcome = outcomes[fixture_index];
                let home_kernel = self.kernel(model, fixture_index, outcome)?;
                if is_home {
                    distribution = distribution.convolve(&home_kernel, &mut work)?;
                } else {
                    let reflected = home_kernel
                        .iter()
                        .map(|&(gd, p)| (-gd, p))
                        .collect::<Vec<_>>();
                    distribution = distribution.convolve(&reflected, &mut work)?;
                }
            }
            private.push(distribution);
        }

        let base_gd = cohort
            .iter()
            .map(|&team| model.base[team].gf - model.base[team].ga)
            .collect::<Vec<_>>();
        let target_distribution = &private[target_slot];
        let mut strict = 0.0;
        let mut weak = 0.0;
        for (delta_index, &target_mass) in target_distribution.mass.iter().enumerate() {
            if target_mass <= 0.0 {
                continue;
            }
            let target_total = base_gd[target_slot]
                + shared_gd[target_slot]
                + target_distribution.min
                + delta_index as i32;
            let mut strict_mass = 1.0;
            let mut weak_mass = 1.0;
            for rival_slot in 0..cohort.len() {
                if rival_slot == target_slot {
                    continue;
                }
                let threshold = target_total - base_gd[rival_slot] - shared_gd[rival_slot];
                strict_mass *= private[rival_slot].less_than(threshold);
                weak_mass *= private[rival_slot].less_equal(threshold);
                if strict_mass == 0.0 && weak_mass == 0.0 {
                    break;
                }
            }
            strict += target_mass * strict_mass;
            weak += target_mass * weak_mass;
        }
        Ok(Projection {
            strict: clamp_probability(strict)?,
            weak: clamp_probability(weak)?,
            tied_rivals: cohort.len() - 1,
            shared_games,
        })
    }

    fn kernel(
        &mut self,
        model: &Model,
        fixture_index: usize,
        outcome: u8,
    ) -> Result<Vec<(i32, f64)>, String> {
        let key = (fixture_index, outcome);
        if let Some(kernel) = self.kernels.get(&key) {
            return Ok(kernel.clone());
        }
        let fixture = &model.fixtures[fixture_index];
        let cdf = conditional_score_cdf(
            fixture.home_sampler.mean,
            fixture.away_sampler.mean,
            outcome,
        )?;
        let mut grouped = HashMap::<i32, f64>::new();
        let mut previous = 0.0;
        for ([home, away], cumulative) in cdf {
            let mass = cumulative - previous;
            previous = cumulative;
            if mass > 0.0 {
                *grouped.entry(home - away).or_default() += mass;
            }
        }
        let mut kernel = grouped.into_iter().collect::<Vec<_>>();
        kernel.sort_unstable_by_key(|entry| entry.0);
        if self.kernels.len() < MAX_CACHED_KERNELS {
            self.kernels.insert(key, kernel.clone());
        }
        Ok(kernel)
    }
}

/// Reconstructs the conditional score CDF in exactly the same nested order as
/// `Scores::new`, including its conditional normalization and terminal one.
fn conditional_score_cdf(
    home_mean: f64,
    away_mean: f64,
    outcome: u8,
) -> Result<Vec<([i32; 2], f64)>, String> {
    let home_masses = masses(home_mean);
    let away_masses = masses(away_mean);
    let mut cdf = Vec::<([i32; 2], f64)>::new();
    let mut probability = 0.0;
    for (home, &home_mass) in home_masses.iter().enumerate() {
        for (away, &away_mass) in away_masses.iter().enumerate() {
            let selected = match outcome {
                0 => home < away,
                1 => home == away,
                2 => home > away,
                _ => false,
            };
            let score_mass = home_mass * away_mass;
            if selected && score_mass > 0.0 {
                probability += score_mass;
                cdf.push(([home as i32, away as i32], probability));
            }
        }
    }
    if probability <= 0.0 {
        return Err(format!(
            "WDL outcome {outcome} has no conditional score support"
        ));
    }
    for (_, cumulative) in &mut cdf {
        *cumulative /= probability;
    }
    cdf.last_mut().expect("positive outcome has support").1 = 1.0;
    Ok(cdf)
}

fn clamp_probability(value: f64) -> Result<f64, String> {
    if !value.is_finite() || value < -1e-12 || value > 1.0 + 1e-12 {
        return Err(format!(
            "GD projection produced invalid probability {value}"
        ));
    }
    Ok(value.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use golaberto_odds::{model::Request, rng::Rng, sampling::Scores};
    use serde_json::json;

    fn toy_model(games: serde_json::Value, teams: serde_json::Value) -> Model {
        let request: Request = serde_json::from_value(json!({
            "id": 1,
            "zones": [],
            "phase": {"sort":"pt,w,gd", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups": teams,
            "games": games
        })).unwrap();
        Model::new(request).unwrap()
    }

    #[test]
    fn reconstructed_conditional_score_sampler_matches_public_scores_sampler() {
        for (home_mean, away_mean) in [(0.0, 0.0), (1.2, 0.8), (3.7, 1.1)] {
            let scores = Scores::new(home_mean, away_mean);
            for outcome in 0..3u8 {
                if scores.prob[outcome as usize] == 0.0 {
                    continue;
                }
                let cdf = conditional_score_cdf(home_mean, away_mean, outcome).unwrap();
                for seed in 1..80 {
                    let mut reconstructed_rng = Rng::new(seed);
                    let mut public_rng = Rng::new(seed);
                    let u = reconstructed_rng.float();
                    let index = cdf.partition_point(|(_, cumulative)| *cumulative < u);
                    let reconstructed = cdf[index.min(cdf.len() - 1)].0;
                    assert_eq!(
                        reconstructed,
                        scores.sample(outcome as usize, &mut public_rng)
                    );
                }
            }
        }
    }

    #[test]
    fn reflected_away_kernel_convolves_all_support_points() {
        let mut work = 0;
        let distribution = Distribution::point(0)
            .convolve(&[(-1, 0.25), (-2, 0.75)], &mut work)
            .unwrap();
        assert_eq!(distribution.min, -2);
        assert_eq!(distribution.mass.len(), 2);
        assert!((distribution.mass[0] - 0.75).abs() < 1e-15);
        assert!((distribution.mass[1] - 0.25).abs() < 1e-15);
        assert!((distribution.less_than(-1) - 0.75).abs() < 1e-15);
    }

    #[test]
    fn tiny_private_gd_convolution_has_hand_computed_cdf() {
        let mut work = 0;
        let first = Distribution::point(0)
            .convolve(&[(-1, 0.25), (1, 0.75)], &mut work)
            .unwrap();
        let both = first.convolve(&[(-1, 0.5), (1, 0.5)], &mut work).unwrap();
        assert_eq!(both.min, -2);
        assert_eq!(both.mass.len(), 5);
        assert!((both.mass[0] - 0.125).abs() < 1e-15);
        assert!((both.mass[2] - 0.5).abs() < 1e-15);
        assert!((both.mass[4] - 0.375).abs() < 1e-15);
        assert!((both.less_than(0) - 0.125).abs() < 1e-15);
        assert!((both.less_equal(0) - 0.625).abs() < 1e-15);
    }

    #[test]
    fn shared_scoreline_moves_the_two_gd_totals_in_opposite_directions() {
        let model = toy_model(
            json!([
                {"id":1,"home_id":2,"away_id":3,"home_score":3,"away_score":0,"played":true},
                {"id":2,"home_id":1,"away_id":2,"home_power":1.4,"away_power":0.9,"played":false}
            ]),
            json!([{"team_id":1},{"team_id":2},{"team_id":3}]),
        );
        for seed in 1..20 {
            let mut expected_rng = Rng::new(seed);
            let score = model.fixtures[0].scores.sample(2, &mut expected_rng);
            let home_gd = score[0] - score[1];
            let away_gd = 3 + score[1] - score[0];
            let expected_next = expected_rng.float();
            let mut integrator = Integrator::new(&model).unwrap();
            let mut rng = Rng::new(seed);
            let projection = integrator.evaluate(&model, &[2], 0, &mut rng).unwrap();
            assert_eq!(projection.tied_rivals, 1);
            assert_eq!(projection.shared_games, 1);
            assert_eq!(projection.strict, if home_gd > away_gd { 1.0 } else { 0.0 });
            assert_eq!(projection.weak, if home_gd >= away_gd { 1.0 } else { 0.0 });
            assert_eq!(rng.float(), expected_next);
        }
    }
}
