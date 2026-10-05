use crate::{model::Model, rng::Rng, sampling::masses};
use std::cmp::Ordering;

/// A categorical score distribution with an associated probability mass.
/// `scores` stores cumulative probabilities conditional on this table.
#[derive(Clone)]
pub(crate) struct ScoreTable {
    pub mass: f64,
    pub representative: [i32; 2],
    scores: Vec<([i32; 2], f64)>,
}

impl ScoreTable {
    fn from_weights(mass: f64, weights: Vec<([i32; 2], f64)>) -> Option<Self> {
        if mass <= 0.0 || weights.is_empty() {
            return None;
        }
        let weight_sum: f64 = weights.iter().map(|(_, p)| p).sum();
        if weight_sum <= 0.0 {
            return None;
        }
        let representative = weights[0].0;
        let mut cumulative = 0.0;
        let mut scores = Vec::with_capacity(weights.len());
        for (score, weight) in weights {
            cumulative += weight / weight_sum;
            scores.push((score, cumulative));
        }
        // Absorb accumulated floating point error in the final interval.
        scores.last_mut().unwrap().1 = 1.0;
        Some(Self {
            mass,
            representative,
            scores,
        })
    }

    pub(crate) fn sample(&self, rng: &mut Rng) -> [i32; 2] {
        let u = rng.float();
        let i = self
            .scores
            .partition_point(|(_, cumulative)| *cumulative < u);
        self.scores[i.min(self.scores.len() - 1)].0
    }
}

pub(crate) struct FixtureBuckets {
    /// Away win, draw, home win, in that order.
    pub outcomes: [Option<ScoreTable>; 3],
    /// For each outcome, head-to-head ordering buckets in Less, Equal, Greater order.
    pub head: [Vec<(Ordering, ScoreTable)>; 3],
}

pub(crate) fn build(model: &Model, fixture_index: usize) -> FixtureBuckets {
    let fixture = &model.fixtures[fixture_index];
    let request_game = &model.request.games[fixture.request_index];
    let home_masses = masses(request_game.home_power);
    let away_masses = masses(request_game.away_power);
    let mut outcome_weights: [Vec<([i32; 2], f64)>; 3] = std::array::from_fn(|_| Vec::new());
    let mut head_weights: [Vec<(Ordering, Vec<([i32; 2], f64)>)>; 3] =
        std::array::from_fn(|_| Vec::new());
    let mut total_mass = 0.0;
    // This comparator cannot consume randomness when called without a pair,
    // but reuse one RNG to avoid initializing one for every supported score.
    let mut compare_rng = Rng::new(0);

    for (home_score, home_probability) in home_masses.iter().enumerate() {
        for (away_score, away_probability) in away_masses.iter().enumerate() {
            let probability = home_probability * away_probability;
            if probability <= 0.0 {
                continue;
            }
            let score = [home_score as i32, away_score as i32];
            let outcome = if score[0] < score[1] {
                0
            } else if score[0] == score[1] {
                1
            } else {
                2
            };
            let mut scores = model.empty_scores();
            scores[fixture.request_index] = score;
            let ordering = model.head_order(fixture.home, fixture.away, &scores, &mut compare_rng);

            outcome_weights[outcome].push((score, probability));
            let index = head_weights[outcome]
                .iter()
                .position(|(existing, _)| *existing == ordering);
            if let Some(index) = index {
                head_weights[outcome][index].1.push((score, probability));
            } else {
                head_weights[outcome].push((ordering, vec![(score, probability)]));
            }
            total_mass += probability;
        }
    }

    let outcomes = std::array::from_fn(|outcome| {
        let raw_mass: f64 = outcome_weights[outcome].iter().map(|(_, p)| p).sum();
        ScoreTable::from_weights(raw_mass / total_mass, outcome_weights[outcome].clone())
    });
    let head = std::array::from_fn(|outcome| {
        let outcome_mass: f64 = outcome_weights[outcome].iter().map(|(_, p)| p).sum();
        let mut buckets: Vec<_> = head_weights[outcome]
            .drain(..)
            .map(|(ordering, weights)| {
                let raw_mass: f64 = weights.iter().map(|(_, p)| p).sum();
                (
                    ordering,
                    ScoreTable::from_weights(raw_mass / outcome_mass, weights).unwrap(),
                )
            })
            .collect();
        buckets.sort_by_key(|(ordering, _)| match ordering {
            Ordering::Less => 0,
            Ordering::Equal => 1,
            Ordering::Greater => 2,
        });
        buckets
    });
    FixtureBuckets { outcomes, head }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Championship, Game, Phase, Request, Team};

    fn reverse_fixture_model(home_power: f64, away_power: f64) -> Model {
        Model::new(Request {
            id: 1,
            zones: vec![],
            phase: Phase {
                sort: "pt,head,gd,gf,g_away".into(),
                championship: Championship {
                    point_win: 3,
                    point_draw: 1,
                    point_loss: 0,
                },
                bonus_points: 0,
                bonus_points_threshold: 0,
            },
            games: vec![
                Game {
                    id: 1,
                    home_id: 1,
                    away_id: 2,
                    home_score: 3,
                    away_score: 0,
                    home_power: 0.0,
                    away_power: 0.0,
                    played: true,
                },
                Game {
                    id: 2,
                    home_id: 2,
                    away_id: 1,
                    home_score: 0,
                    away_score: 0,
                    home_power,
                    away_power,
                    played: false,
                },
            ],
            team_groups: vec![
                Team {
                    team_id: 1,
                    add_sub: 0,
                    bias: 0,
                },
                Team {
                    team_id: 2,
                    add_sub: 0,
                    bias: 0,
                },
            ],
        })
        .unwrap()
    }

    #[test]
    fn outcome_and_head_masses_are_normalized_and_samples_preserve_bucket() {
        let model = reverse_fixture_model(0.8, 1.2);
        let buckets = build(&model, 0);
        let total_outcome_mass: f64 = buckets
            .outcomes
            .iter()
            .flatten()
            .map(|table| table.mass)
            .sum();
        assert!((total_outcome_mass - 1.0).abs() < 1e-14);
        let mut rng = Rng::new(42);
        for outcome in 0..3 {
            let head_mass: f64 = buckets.head[outcome]
                .iter()
                .map(|(_, table)| table.mass)
                .sum();
            assert!((head_mass - 1.0).abs() < 1e-14);
            for (ordering, table) in &buckets.head[outcome] {
                assert!(table.mass > 0.0);
                for _ in 0..100 {
                    let score = table.sample(&mut rng);
                    let actual_outcome = if score[0] < score[1] {
                        0
                    } else if score[0] == score[1] {
                        1
                    } else {
                        2
                    };
                    assert_eq!(actual_outcome, outcome);
                    let mut scores = model.empty_scores();
                    scores[model.fixtures[0].request_index] = score;
                    assert_eq!(
                        model.head_order(
                            model.fixtures[0].home,
                            model.fixtures[0].away,
                            &scores,
                            &mut rng,
                        ),
                        *ordering
                    );
                }
            }
        }
    }

    #[test]
    fn rare_home_win_tail_splits_at_prior_three_goal_margin() {
        let model = reverse_fixture_model(0.05, 0.05);
        let buckets = build(&model, 0);
        let home_win = &buckets.head[2];
        assert!(home_win
            .iter()
            .any(|(order, table)| *order == Ordering::Less && table.mass > 0.0));
        assert!(home_win
            .iter()
            .any(|(order, table)| *order == Ordering::Equal && table.mass > 0.0));
        assert!(home_win
            .iter()
            .any(|(order, table)| *order == Ordering::Greater && table.mass > 0.0));
        let greater = home_win
            .iter()
            .find(|(order, _)| *order == Ordering::Greater)
            .unwrap();
        assert!(greater.1.mass > 0.0);
        assert!(greater.1.representative[0] - greater.1.representative[1] > 3);
        let equal = home_win
            .iter()
            .find(|(order, _)| *order == Ordering::Equal)
            .unwrap();
        assert_eq!(equal.1.representative, [3, 0]);
    }

    #[test]
    fn outcome_masses_match_independent_poisson_sum() {
        let model = reverse_fixture_model(0.8, 1.2);
        let buckets = build(&model, 0);
        let home = masses(0.8);
        let away = masses(1.2);
        let mut reference = [0.0; 3];
        for (h, hp) in home.iter().enumerate() {
            for (a, ap) in away.iter().enumerate() {
                reference[if h < a {
                    0
                } else if h == a {
                    1
                } else {
                    2
                }] += hp * ap;
            }
        }
        let normalizer: f64 = reference.iter().sum();
        for outcome in 0..3 {
            assert!(
                (buckets.outcomes[outcome].as_ref().unwrap().mass
                    - reference[outcome] / normalizer)
                    .abs()
                    < 1e-14
            );
        }
    }
}
