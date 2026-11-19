//! Deterministic target-point-stratified recovery for guide-selected rare ranks.

use super::{parallel, Cell};
use crate::{
    conditioned::{Backward, Event},
    model::Model,
    pool::{Bounds, Estimate},
    rng::derive,
};

const DRAWS_PER_POINT: usize = 256;

fn stratum_variance(mass: f64, hits: usize, draws: usize) -> f64 {
    let q = hits as f64 / draws as f64;
    mass * mass * q * (1.0 - q) / (draws - 1) as f64
}

fn worst_case_variance(mass: f64, draws: usize) -> f64 {
    mass * mass / (4.0 * draws as f64)
}

fn draw_rank(
    model: &Model,
    team: usize,
    backward: &mut Backward<'_>,
    points: i32,
    outcomes: &mut [u8],
    campaigns: &mut [crate::model::Campaign],
    scores: &mut [[i32; 2]],
    order: &mut [usize],
    rng: &mut crate::rng::Rng,
) -> Option<usize> {
    outcomes.fill(0);
    if !backward.from_points(rng, outcomes, points) {
        return None;
    }
    campaigns.copy_from_slice(&model.base);
    for (i, fixture) in model.fixtures.iter().enumerate() {
        let target_game = fixture.home == team || fixture.away == team;
        let score = if target_game {
            fixture.scores.sample(outcomes[i] as usize, rng)
        } else {
            [
                fixture.home_sampler.sample(rng),
                fixture.away_sampler.sample(rng),
            ]
        };
        scores[fixture.request_index] = score;
        model.add(campaigns, i, score);
    }
    model.standings(order, campaigns, scores, rng);
    order.iter().position(|&t| t == team)
}

/// Estimate direct-zero cells with a positive finite private guide.
/// Candidate ranks share each target team's simulated seasons.
pub(super) fn estimate(
    model: &Model,
    bounds: &Bounds,
    pmfs: &[Vec<(i32, f64)>],
    cells: &[Cell],
    estimates: &mut [Estimate],
    seed: i64,
    workers: usize,
) -> (usize, u64) {
    if pmfs.len() < model.n || model.request.phase.bonus_points != 0 {
        return (0, 0);
    }
    let mut by_team = vec![Vec::<usize>::new(); model.n];
    for (cell_index, cell) in cells.iter().enumerate() {
        let index = cell.index(model.n);
        if cell.team < model.n
            && index < estimates.len()
            && estimates[index].hits == 0
            && estimates[index].probability == 0.0
            && estimates[index].pool_guide.is_finite()
            && estimates[index].pool_guide > 0.0
        {
            by_team[cell.team].push(cell_index);
        }
    }
    let teams: Vec<_> = by_team
        .iter()
        .enumerate()
        .filter_map(|(team, candidates)| (!candidates.is_empty()).then_some(team))
        .collect();
    let runs = parallel(teams.len(), workers, |team_index| {
        let team = teams[team_index];
        if model
            .fixtures
            .iter()
            .any(|f| f.home == team && f.away == team)
        {
            return (Vec::new(), 0u64);
        }
        // The one-team forward DP contains all point states; from_points does
        // not consult the rank-filtered terminal list.
        let Some(event) = Event::build(model, bounds, team, model.n - 1, &[], 30000) else {
            return (Vec::new(), 0u64);
        };
        if event.teams.len() != 1
            || event.games.len()
                != model
                    .fixtures
                    .iter()
                    .filter(|f| f.home == team || f.away == team)
                    .count()
        {
            return (Vec::new(), 0u64);
        }
        let candidates = &by_team[team];
        let strata: Vec<(i32, f64)> = pmfs[team]
            .iter()
            .filter_map(|&(points, mass)| {
                if !mass.is_finite() || mass <= 0.0 {
                    return None;
                }
                let has_allowed_rank = candidates
                    .iter()
                    .any(|&cell_index| bounds.allowed(team, cells[cell_index].rank, points));
                has_allowed_rank.then_some((points, mass))
            })
            .collect();
        if strata.is_empty() {
            return (Vec::new(), 0u64);
        }
        // If a potentially rank-feasible PMF point is absent from the exact
        // forward DP, abandon this team rather than turning failed closure
        // attempts into misses.
        if strata.iter().any(|(points, _)| {
            !(0..=255).contains(points)
                || event
                    .forward
                    .last()
                    .and_then(|layer| layer.get(&(*points as u64)))
                    .is_none_or(|mass| *mass <= 0.0)
        }) {
            return (Vec::new(), 0u64);
        }
        let mut backward = Backward::new(&event);
        let mut outcomes = vec![0u8; model.fixtures.len()];
        let mut campaigns = model.base.clone();
        let mut scores = model.empty_scores();
        let mut order = vec![0usize; model.n];
        let mut rng = crate::rng::Rng::new(derive(
            seed,
            &format!("guided-target-points-{}", model.ids[team]),
        ));
        let mut stratum_hits = vec![vec![[0usize; 2]; strata.len()]; candidates.len()];
        let mut attempted = 0usize;
        for (stratum, &(points, _mass)) in strata.iter().enumerate() {
            for draw in 0..DRAWS_PER_POINT {
                attempted += 1;
                let Some(rank) = draw_rank(
                    model,
                    team,
                    &mut backward,
                    points,
                    &mut outcomes,
                    &mut campaigns,
                    &mut scores,
                    &mut order,
                    &mut rng,
                ) else {
                    return (
                        Vec::new(),
                        attempted as u64 * crate::pool::work_per_sample(model),
                    );
                };
                for (candidate, &cell_index) in candidates.iter().enumerate() {
                    if rank == cells[cell_index].rank
                        && bounds.allowed(team, cells[cell_index].rank, points)
                    {
                        stratum_hits[candidate][stratum]
                            [usize::from(draw >= DRAWS_PER_POINT / 2)] += 1;
                    }
                }
            }
        }
        let mut updates = Vec::new();
        for (candidate, &cell_index) in candidates.iter().enumerate() {
            let index = cells[cell_index].index(model.n);
            let mut probability = 0.0;
            let mut variance = 0.0;
            let mut hits = 0;
            let mut active_mass = 0.0;
            let mut active_strata = 0usize;
            let mut fallback_variance = 0.0;
            let mut squared_weights = 0.0;
            let mut max_observed_weight = 0.0f64;
            let mut first_half = 0.0;
            let mut second_half = 0.0;
            for (stratum, &(_, mass)) in strata.iter().enumerate() {
                if !bounds.allowed(team, cells[cell_index].rank, strata[stratum].0) {
                    continue;
                }
                active_mass += mass;
                active_strata += 1;
                fallback_variance += worst_case_variance(mass, DRAWS_PER_POINT);
                let split = stratum_hits[candidate][stratum];
                let stratum_hits = split[0] + split[1];
                let q = stratum_hits as f64 / DRAWS_PER_POINT as f64;
                probability += mass * q;
                variance += stratum_variance(mass, stratum_hits, DRAWS_PER_POINT);
                if stratum_hits > 0 {
                    let event_weight = mass / DRAWS_PER_POINT as f64;
                    squared_weights += stratum_hits as f64 * event_weight * event_weight;
                    max_observed_weight = max_observed_weight.max(event_weight);
                }
                hits += stratum_hits;
                first_half += mass * split[0] as f64 / (DRAWS_PER_POINT / 2) as f64;
                second_half += mass * split[1] as f64 / (DRAWS_PER_POINT / 2) as f64;
            }
            let se = if variance > 0.0 {
                variance.sqrt()
            } else {
                fallback_variance.sqrt()
            };
            let ess = if squared_weights > 0.0 {
                probability * probability / squared_weights
            } else {
                0.0
            };
            let half_difference = (first_half - second_half).abs();
            if ess >= 20.0
                && probability.is_finite()
                && probability > 0.0
                && se.is_finite()
                && se / probability <= 0.5
                && half_difference <= 6.0 * se
            {
                let max_share = max_observed_weight / probability;
                updates.push((
                    index,
                    probability,
                    se,
                    DRAWS_PER_POINT * active_strata,
                    hits,
                    ess,
                    max_share,
                    active_mass,
                ));
            }
        }
        (
            updates,
            (DRAWS_PER_POINT * strata.len()) as u64 * crate::pool::work_per_sample(model),
        )
    });
    let mut changed = 0;
    let mut work = 0;
    for (updates, team_work) in runs {
        work += team_work;
        for (index, probability, se, samples, hits, ess, max_share, active_mass) in updates {
            let estimate = &mut estimates[index];
            estimate.probability = probability;
            estimate.std_err = se;
            estimate.relative_se = Some(se / probability);
            estimate.ess = ess;
            estimate.max_event_weight_share = max_share;
            estimate.available = true;
            estimate.meets_precision_goal = se / probability <= 0.5;
            estimate.zero_hit_upper_95 = 0.0;
            estimate.design = "target_point_stratified".into();
            estimate.reachability = "witness".into();
            estimate.conditional_mass = active_mass;
            estimate.conditional_samples = samples;
            estimate.conditional_hits = hits;
            changed += 1;
        }
    }
    (changed, work)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Championship, Game, Phase, Request, Team};

    #[test]
    fn nondegenerate_small_stratum_uses_stratified_se_without_mass_floor() {
        let mass = 9.435e-6;
        let se = stratum_variance(mass, 86, DRAWS_PER_POINT).sqrt();
        let worst_case = worst_case_variance(mass, DRAWS_PER_POINT).sqrt();
        assert!(se > 0.0);
        assert!(se < mass / 20.0);
        assert!((worst_case / mass - 1.0 / 32.0).abs() < 1e-12);
    }

    #[test]
    fn no_feasible_stratum_leaves_zero_estimate_metadata_untouched() {
        let model = Model::new(Request {
            id: 1,
            zones: vec![],
            phase: Phase {
                sort: "pt".into(),
                championship: Championship {
                    point_win: 3,
                    point_draw: 1,
                    point_loss: 0,
                },
                bonus_points: 0,
                bonus_points_threshold: 0,
            },
            games: vec![Game {
                id: 1,
                home_id: 1,
                away_id: 3,
                home_score: 0,
                away_score: 0,
                home_power: 0.1,
                away_power: 8.0,
                played: false,
            }],
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
                Team {
                    team_id: 3,
                    add_sub: 2,
                    bias: 0,
                },
            ],
        })
        .unwrap();
        let bounds = Bounds::new(&model);
        let cells = [Cell { team: 0, rank: 0 }];
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        estimates[0].pool_guide = 1.0;
        estimates[0].samples = 100_000;
        estimates[0].work_spent = 17;
        let pmfs = vec![vec![(0, 1.0)]; model.n];

        let changed = estimate(&model, &bounds, &pmfs, &cells, &mut estimates, 99, 1);

        assert_eq!(changed, (0, 0));
        assert_eq!(estimates[0].probability, 0.0);
        assert_eq!(estimates[0].samples, 100_000);
        assert_eq!(estimates[0].hits, 0);
        assert_eq!(estimates[0].work_spent, 17);
        assert!(estimates[0].design.is_empty());
    }

    #[test]
    fn positive_estimate_uses_full_pmf_mass_and_keeps_scout_fields() {
        let model = Model::new(Request {
            id: 2,
            zones: vec![],
            phase: Phase {
                sort: "pt".into(),
                championship: Championship {
                    point_win: 3,
                    point_draw: 1,
                    point_loss: 0,
                },
                bonus_points: 0,
                bonus_points_threshold: 0,
            },
            games: vec![Game {
                id: 1,
                home_id: 1,
                away_id: 3,
                home_score: 0,
                away_score: 0,
                home_power: 0.1,
                away_power: 8.0,
                played: false,
            }],
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
                Team {
                    team_id: 3,
                    add_sub: 2,
                    bias: 0,
                },
            ],
        })
        .unwrap();
        let bounds = Bounds::new(&model);
        let cell = Cell { team: 0, rank: 0 };
        assert!(!bounds.allowed(0, 0, 0));
        assert!(!bounds.allowed(0, 0, 1));
        assert!(bounds.allowed(0, 0, 3));
        let probabilities = model.fixtures[0].prob;
        assert!(
            probabilities[2] < 0.001,
            "expected a small target-point stratum"
        );
        let pmfs = vec![
            vec![
                (0, probabilities[0]),
                (1, probabilities[1]),
                (3, probabilities[2])
            ];
            model.n
        ];
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        estimates[0].pool_guide = 0.25;
        estimates[0].samples = 100_000;
        estimates[0].work_spent = 19;

        let (changed, work) = estimate(&model, &bounds, &pmfs, &[cell], &mut estimates, 317, 1);

        assert_eq!(changed, 1);
        assert!(
            (estimates[0].probability - probabilities[2]).abs() < 1e-12,
            "estimated {} but expected win mass {} (probabilities {:?})",
            estimates[0].probability,
            probabilities[2],
            probabilities
        );
        assert_eq!(estimates[0].design, "target_point_stratified");
        assert_eq!(estimates[0].conditional_mass, probabilities[2]);
        assert_eq!(estimates[0].conditional_samples, DRAWS_PER_POINT);
        assert_eq!(estimates[0].conditional_hits, DRAWS_PER_POINT);
        assert_eq!(estimates[0].samples, 100_000);
        assert_eq!(estimates[0].hits, 0);
        assert_eq!(estimates[0].work_spent, 19);
        assert_eq!(estimates[0].pool_guide, 0.25);
        assert_eq!(
            work,
            DRAWS_PER_POINT as u64 * crate::pool::work_per_sample(&model)
        );
    }
}
