//! Sequential particle filters for rare outer-rank cells.
//!
//! The target team's incident fixtures are drawn from a defensive mixture
//! tilted toward a favorable points path. Their native/proposal likelihood
//! ratio is held until the terminal rank event. Other fixtures are sampled
//! once per particle, in rival blocks ordered to expose impossible ranks early.

use crate::{
    conditioned::Result,
    logging::RequestLog,
    model::{Campaign, Key, Model},
    pool::Estimate,
    rng::{derive, Rng},
};
use serde_json::json;
use std::cmp::Ordering;

const ISLANDS: usize = 128;
const PARTICLES: usize = 256;
const LOOKAHEAD_FLOOR: f64 = 1e-12;

#[derive(Clone)]
struct Particle {
    campaign: Vec<Campaign>,
    scores: Vec<[i32; 2]>,
    target_ratio: f64,
    ahead: usize,
    below: usize,
    lookahead: f64,
}

#[derive(Clone, Copy)]
struct TargetWdlProposal {
    native: [f64; 3],
    proposal: [f64; 3],
}

impl TargetWdlProposal {
    fn new(native: [f64; 3], target_is_home: bool, direction: f64, model: &Model) -> Self {
        let rules = &model.request.phase.championship;
        let utility: [f64; 3] = std::array::from_fn(|home_outcome| {
            let own_outcome = if target_is_home {
                home_outcome
            } else {
                2 - home_outcome
            };
            let points = match own_outcome {
                0 => rules.point_loss,
                1 => rules.point_draw,
                _ => rules.point_win,
            };
            let win_bonus = if own_outcome == 2 { 0.2 } else { 0. };
            direction * (points as f64 + win_bonus)
        });
        let mild = tilted(native, utility, 0.45);
        let strong = tilted(native, utility, 1.1);
        let proposal =
            std::array::from_fn(|i| 0.05 * native[i] + 0.25 * mild[i] + 0.70 * strong[i]);
        Self { native, proposal }
    }

    fn sample(self, fixture: usize, model: &Model, rng: &mut Rng) -> ([i32; 2], f64) {
        let outcome = categorical(self.proposal, rng);
        let likelihood = self.native[outcome] / self.proposal[outcome];
        let score = model.fixtures[fixture].scores.sample(outcome, rng);
        (score, likelihood)
    }
}

fn tilted(native: [f64; 3], utility: [f64; 3], strength: f64) -> [f64; 3] {
    let max_log = (0..3)
        .filter(|&i| native[i] > 0.)
        .map(|i| native[i].ln() + strength * utility[i])
        .fold(f64::NEG_INFINITY, f64::max);
    let mut result = [0.; 3];
    let mut total = 0.;
    for i in 0..3 {
        if native[i] > 0. {
            result[i] = (native[i].ln() + strength * utility[i] - max_log).exp();
            total += result[i];
        }
    }
    if total > 0. {
        for p in &mut result {
            *p /= total;
        }
    }
    result
}

fn categorical(probabilities: [f64; 3], rng: &mut Rng) -> usize {
    let u = rng.float();
    let mut cumulative = 0.;
    let mut last_positive = None;
    for (outcome, &probability) in probabilities.iter().enumerate() {
        if probability > 0. {
            last_positive = Some(outcome);
            cumulative += probability;
            if u < cumulative {
                return outcome;
            }
        }
    }
    // Handles accumulated roundoff at the upper endpoint without selecting a
    // zero-mass outcome.
    last_positive.expect("categorical distribution has positive mass")
}

fn supported_sort(model: &Model) -> bool {
    matches!(
        model.keys.as_slice(),
        [Key::Pt, Key::Gd, ..] | [Key::Pt, Key::W, Key::Gd, ..]
    )
}

fn candidate_cells(model: &Model, estimates: &[Estimate]) -> Vec<(usize, usize)> {
    if !crate::search::tail_fallback_supported(model)
        || model
            .fixtures
            .iter()
            .any(|fixture| fixture.home_sampler.mean > 32. || fixture.away_sampler.mean > 32.)
        || estimates.len() < model.n * model.n
        || !supported_sort(model)
    {
        return Vec::new();
    }
    let edge = (model.n / 4).max(1);
    (0..model.request.team_groups.len())
        .flat_map(|team| {
            (0..model.n).filter_map(move |rank| {
                let outer = rank < edge || rank >= model.n - edge;
                let estimate = &estimates[team * model.n + rank];
                (outer
                    && estimate.probability == 0.
                    && !estimate.reachability.starts_with("impossible"))
                .then_some((team, rank))
            })
        })
        .collect()
}

fn expected_points(model: &Model) -> Vec<f64> {
    let rules = &model.request.phase.championship;
    let mut points: Vec<_> = model.base.iter().map(|c| c.points as f64).collect();
    for fixture in &model.fixtures {
        let home = fixture.prob[0] * rules.point_loss as f64
            + fixture.prob[1] * rules.point_draw as f64
            + fixture.prob[2] * rules.point_win as f64;
        let away = fixture.prob[0] * rules.point_win as f64
            + fixture.prob[1] * rules.point_draw as f64
            + fixture.prob[2] * rules.point_loss as f64;
        points[fixture.home] += home;
        points[fixture.away] += away;
    }
    points
}

fn rival_blocks(model: &Model, target: usize, rank: usize) -> Vec<(usize, Vec<usize>)> {
    let expected = expected_points(model);
    let favor_high_first = rank <= model.n - 1 - rank;
    let mut rivals: Vec<_> = (0..model.n).filter(|&team| team != target).collect();
    rivals.sort_by(|&a, &b| {
        let order = expected[a].total_cmp(&expected[b]);
        let order = if favor_high_first {
            order.reverse()
        } else {
            order
        };
        order.then_with(|| a.cmp(&b))
    });
    let mut position = vec![usize::MAX; model.n];
    for (i, &team) in rivals.iter().enumerate() {
        position[team] = i;
    }
    let mut blocks = vec![Vec::new(); rivals.len()];
    for (i, fixture) in model.fixtures.iter().enumerate() {
        if fixture.home == target || fixture.away == target {
            continue;
        }
        let home = position[fixture.home];
        let away = position[fixture.away];
        let assigned = home.min(away);
        if assigned != usize::MAX {
            blocks[assigned].push(i);
        }
    }
    rivals.into_iter().zip(blocks).collect()
}

fn future_point_pmfs(model: &Model, blocks: &[(usize, Vec<usize>)]) -> Vec<Vec<Vec<(i32, f64)>>> {
    let mut result = Vec::with_capacity(blocks.len());
    for stage in 0..blocks.len() {
        let mut stage_pmfs = vec![Vec::new(); model.n];
        for team in 0..model.n {
            if blocks[stage..]
                .iter()
                .any(|(remaining, _)| *remaining == team)
            {
                let mut distribution = vec![(0, 1.)];
                for (_, fixtures) in &blocks[stage + 1..] {
                    for &fixture_index in fixtures {
                        let fixture = &model.fixtures[fixture_index];
                        if fixture.home != team && fixture.away != team {
                            continue;
                        }
                        let rules = &model.request.phase.championship;
                        let own_points = if fixture.home == team {
                            [rules.point_loss, rules.point_draw, rules.point_win]
                        } else {
                            [rules.point_win, rules.point_draw, rules.point_loss]
                        };
                        let mut next = std::collections::BTreeMap::<i32, f64>::new();
                        for &(current, mass) in &distribution {
                            for outcome in 0..3 {
                                *next.entry(current + own_points[outcome]).or_default() +=
                                    mass * fixture.prob[outcome];
                            }
                        }
                        distribution = next.into_iter().collect();
                    }
                }
                stage_pmfs[team] = distribution;
            }
        }
        result.push(stage_pmfs);
    }
    result
}

fn wrong_side_probability(
    distribution: &[(i32, f64)],
    current_points: i32,
    target_points: i32,
    target_is_high: bool,
) -> f64 {
    distribution
        .iter()
        .map(|&(gain, mass)| {
            let final_points = current_points + gain;
            let wrong = if target_is_high {
                match final_points.cmp(&target_points) {
                    Ordering::Greater => 1.,
                    Ordering::Equal => 0.5,
                    Ordering::Less => 0.,
                }
            } else {
                match final_points.cmp(&target_points) {
                    Ordering::Less => 1.,
                    Ordering::Equal => 0.5,
                    Ordering::Greater => 0.,
                }
            };
            mass * wrong
        })
        .sum::<f64>()
        .clamp(0., 1.)
}

fn lower_tail(probabilities: &[f64], allowed: usize) -> f64 {
    if allowed >= probabilities.len() {
        return 1.;
    }
    let mut dp = vec![0.; allowed + 1];
    dp[0] = 1.;
    let mut count = 0;
    for &probability in probabilities {
        let p = probability.clamp(0., 1.);
        let ceiling = (count + 1).min(allowed);
        for wrong in (1..=ceiling).rev() {
            dp[wrong] = dp[wrong] * (1. - p) + dp[wrong - 1] * p;
        }
        dp[0] *= 1. - p;
        count += 1;
    }
    dp.iter().sum::<f64>().clamp(0., 1.)
}

fn lookahead(
    model: &Model,
    target: usize,
    rank: usize,
    completed_stage: usize,
    particle: &Particle,
    blocks: &[(usize, Vec<usize>)],
    future_pmfs: &[Vec<Vec<(i32, f64)>>],
) -> f64 {
    let target_is_high = rank <= model.n - 1 - rank;
    let wrong_count = if target_is_high {
        particle.ahead
    } else {
        particle.below
    };
    let allowed = if target_is_high {
        rank
    } else {
        model.n - 1 - rank
    };
    if particle.ahead > rank || particle.below > model.n - 1 - rank || wrong_count > allowed {
        return 0.;
    }
    let target_points = particle.campaign[target].points;
    let future_wrong: Vec<_> = blocks[completed_stage + 1..]
        .iter()
        .map(|(team, _)| {
            wrong_side_probability(
                &future_pmfs[completed_stage][*team],
                particle.campaign[*team].points,
                target_points,
                target_is_high,
            )
        })
        .collect();
    lower_tail(&future_wrong, allowed - wrong_count).max(LOOKAHEAD_FLOOR)
}

/// Compare only the supported, deterministic standings prefix. Equal prefixes
/// and keys involving future/unordered evidence remain ties for safe pruning.
fn compare_prefix(model: &Model, a: usize, b: usize, campaigns: &[Campaign]) -> Ordering {
    for key in &model.keys {
        let left = &campaigns[a];
        let right = &campaigns[b];
        let ordering = match key {
            Key::Pt => left.points.cmp(&right.points),
            Key::W => left.wins.cmp(&right.wins),
            Key::Gd => (left.gf - left.ga).cmp(&(right.gf - right.ga)),
            Key::Aet | Key::Gp => Ordering::Equal,
            Key::Gf
            | Key::Average
            | Key::Away
            | Key::Bias
            | Key::Head
            | Key::Random
            | Key::Name => return Ordering::Equal,
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
}

fn target_proposals(model: &Model, target: usize, rank: usize) -> Vec<Option<TargetWdlProposal>> {
    let direction = if rank <= (model.n - 1) / 2 { 1. } else { -1. };
    model
        .fixtures
        .iter()
        .map(|fixture| {
            (fixture.home == target || fixture.away == target).then(|| {
                TargetWdlProposal::new(fixture.prob, fixture.home == target, direction, model)
            })
        })
        .collect()
}

fn one_island(
    model: &Model,
    target: usize,
    rank: usize,
    proposals: &[Option<TargetWdlProposal>],
    blocks: &[(usize, Vec<usize>)],
    future_pmfs: &[Vec<Vec<(i32, f64)>>],
    seed: i64,
) -> (f64, u64) {
    let mut rng = Rng::new(seed);
    let mut particles: Vec<_> = (0..PARTICLES)
        .map(|_| {
            let mut particle = Particle {
                campaign: model.base.clone(),
                scores: model.empty_scores(),
                target_ratio: 1.,
                ahead: 0,
                below: 0,
                lookahead: 1.,
            };
            for (i, proposal) in proposals.iter().enumerate() {
                if let Some(proposal) = proposal {
                    let (score, likelihood) = proposal.sample(i, model, &mut rng);
                    particle.target_ratio *= likelihood;
                    particle.scores[model.fixtures[i].request_index] = score;
                    model.add(&mut particle.campaign, i, score);
                }
            }
            particle
        })
        .collect();
    let mut work = (PARTICLES * proposals.iter().filter(|p| p.is_some()).count()) as u64;
    let mut normalization = 1.;
    for (stage, (team, fixtures)) in blocks.iter().enumerate() {
        let active = particles.len();
        work = work.saturating_add((active * (fixtures.len() + blocks.len() - stage)) as u64);
        let mut weights = Vec::with_capacity(active);
        for particle in &mut particles {
            for &fixture_index in fixtures {
                let fixture = &model.fixtures[fixture_index];
                let outcome = categorical(fixture.prob, &mut rng);
                let score = fixture.scores.sample(outcome, &mut rng);
                particle.scores[fixture.request_index] = score;
                model.add(&mut particle.campaign, fixture_index, score);
            }
            match compare_prefix(model, target, *team, &particle.campaign) {
                Ordering::Greater => particle.below += 1,
                Ordering::Less => particle.ahead += 1,
                Ordering::Equal => {}
            }
            let next = lookahead(model, target, rank, stage, particle, blocks, future_pmfs);
            weights.push(next / particle.lookahead);
            particle.lookahead = next;
        }
        let sum_weights = weights.iter().sum::<f64>();
        if sum_weights <= 0. || !sum_weights.is_finite() {
            return (0., work);
        }
        normalization *= sum_weights / active as f64;
        let mut cumulative = Vec::with_capacity(active);
        let mut total = 0.;
        for weight in weights {
            total += weight;
            cumulative.push(total);
        }
        let ancestors = particles;
        let step = total / PARTICLES as f64;
        let mut threshold = rng.float() * step;
        let mut index = 0;
        particles = Vec::with_capacity(PARTICLES);
        for _ in 0..PARTICLES {
            while index + 1 < cumulative.len() && cumulative[index] <= threshold {
                index += 1;
            }
            particles.push(ancestors[index].clone());
            threshold += step;
        }
    }

    let mut order = vec![0; model.n];
    let mut terminal = 0.;
    for particle in &particles {
        model.standings(&mut order, &particle.campaign, &particle.scores, &mut rng);
        work = work.saturating_add(1);
        if order[rank] == target {
            terminal += particle.target_ratio;
        }
    }
    (normalization * terminal / particles.len() as f64, work)
}

fn summarize(values: &[f64], work: u64) -> Result {
    let half = values.len() / 2;
    let sum = values.iter().sum::<f64>();
    let sum2 = values.iter().map(|v| v * v).sum::<f64>();
    let result = Result {
        mass: 1.,
        samples: values.len(),
        hits: values.iter().filter(|&&v| v > 0.).count(),
        work,
        ..Result::default()
    };
    let max = values.iter().copied().fold(0., f64::max);
    let batches = [values[..half].iter().sum(), values[half..].iter().sum()];
    let mut result = result;
    result.summarize(sum, sum2, max, batches);
    result
}

fn stream(model: &Model, target: usize, rank: usize, seed: i64, stream_name: &str) -> Result {
    let proposals = target_proposals(model, target, rank);
    let blocks = rival_blocks(model, target, rank);
    let future = future_point_pmfs(model, &blocks);
    let mut values = Vec::with_capacity(ISLANDS);
    let mut work = 0u64;
    for island in 0..ISLANDS {
        let island_seed = derive(seed, &format!("tail-particles-{stream_name}-{island}"));
        let (value, spent) = one_island(
            model,
            target,
            rank,
            &proposals,
            &blocks,
            &future,
            island_seed,
        );
        values.push(value);
        work = work.saturating_add(spent);
    }
    summarize(&values, work)
}

/// Process unresolved outer-rank cells for large standard leagues.
/// Returns accepted cells and actual fixture/sort work from both streams.
pub(crate) fn run(
    model: &Model,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    log: Option<&RequestLog>,
) -> (usize, u64) {
    let cells = candidate_cells(model, estimates);
    if cells.is_empty() {
        return (0, 0);
    }
    let outcomes = crate::search::parallel(cells.len(), workers, |i| {
        let (target, rank) = cells[i];
        let cell_seed = derive(seed, &format!("tail-particle-cell-{target}-{rank}"));
        let main = stream(model, target, rank, derive(cell_seed, "main"), "main");
        let check = stream(model, target, rank, derive(cell_seed, "check"), "check");
        let work = main.work.saturating_add(check.work);
        let rough = crate::rare_tail::value("RUST_ODDS_RARE_TAIL_QUALITY") == "order";
        let ok = crate::rare_tail::accepted(&main, &check, rough)
            && main.probability > 0.
            && main.probability <= 1.
            && check.probability > 0.
            && check.probability <= 1.
            && [
                main.probability,
                check.probability,
                main.std_err,
                check.std_err,
                main.ess,
                check.ess,
                main.max_share,
                check.max_share,
                main.batch_gap,
                check.batch_gap,
            ]
            .iter()
            .all(|value| value.is_finite());
        (target, rank, main, check, work, ok)
    });
    let mut accepted = 0;
    let mut total_work = 0u64;
    for (target, rank, main, check, work, ok) in outcomes {
        total_work = total_work.saturating_add(work);
        if ok {
            let index = target * model.n + rank;
            crate::search::apply(&mut estimates[index], &main, "sequential_tail_particles");
            accepted += 1;
        }
        if let Some(log) = log {
            log.event(
                "rust_odds_tail_particles_cell",
                json!({
                    "group": model.request.id,
                    "team": model.ids[target],
                    "rank": rank + 1,
                    "main": {"samples": main.samples, "hits": main.hits, "ess": main.ess, "probability": main.probability, "rse": if main.probability > 0. {Some(main.std_err / main.probability)} else {None}},
                    "check": {"samples": check.samples, "hits": check.hits, "ess": check.ess, "probability": check.probability, "rse": if check.probability > 0. {Some(check.std_err / check.probability)} else {None}},
                    "accepted": ok,
                    "work": work
                }),
            );
        }
    }
    (accepted, total_work)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn triangle() -> Model {
        let request = serde_json::from_value(json!({
            "id": 7301,
            "phase": {
                "sort": "pt,gd,gf,bias",
                "championship": {"point_win": 3, "point_draw": 1, "point_loss": 0}
            },
            "team_groups": (0..3).map(|team| json!({"team_id": team, "add_sub": 0, "bias": team})).collect::<Vec<_>>(),
            "games": [
                {"id":1,"home_id":0,"away_id":1,"home_power":0.45,"away_power":0.0},
                {"id":2,"home_id":0,"away_id":2,"home_power":0.0,"away_power":0.75},
                {"id":3,"home_id":1,"away_id":2,"home_power":0.6,"away_power":0.0}
            ]
        }))
        .unwrap();
        Model::new(request).unwrap()
    }

    fn no_target_fixture_large_group() -> Model {
        let teams: Vec<_> = (0..33)
            .map(|team| {
                json!({
                    "team_id": team,
                    "add_sub": if team == 0 { 2 } else if team == 1 { 0 } else { -100 }
                })
            })
            .collect();
        let request = serde_json::from_value(json!({
            "id": 7302,
            "phase": {
                "sort": "pt,gd",
                "championship": {"point_win": 3, "point_draw": 1, "point_loss": 0}
            },
            "team_groups": teams,
            "games": [
                {"id": 1, "home_id": 1, "away_id": 2, "home_power": 0.8, "away_power": 0.3}
            ]
        }))
        .unwrap();
        Model::new(request).unwrap()
    }

    fn small_group(n: usize) -> Model {
        let request = serde_json::from_value(json!({
            "id": 7303,
            "phase": {"sort":"pt,gd", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups": (0..n).map(|team| json!({"team_id":team,"add_sub":0})).collect::<Vec<_>>(),
            "games": [{"id":1,"home_id":0,"away_id":1,"home_power":0.4,"away_power":0.5}]
        })).unwrap();
        Model::new(request).unwrap()
    }

    fn exact_rank_probability(model: &Model, target: usize, rank: usize) -> f64 {
        let distributions: Vec<_> = model
            .fixtures
            .iter()
            .map(|fixture| {
                let mut home = crate::sampling::masses(fixture.home_sampler.mean);
                let mut away = crate::sampling::masses(fixture.away_sampler.mean);
                if home.len() > 1 {
                    let prior = home[..home.len() - 1].iter().sum::<f64>();
                    *home.last_mut().unwrap() = (1. - prior).max(0.);
                }
                if away.len() > 1 {
                    let prior = away[..away.len() - 1].iter().sum::<f64>();
                    *away.last_mut().unwrap() = (1. - prior).max(0.);
                }
                home.iter()
                    .enumerate()
                    .flat_map(|(h, &hp)| {
                        away.iter().enumerate().filter_map(move |(a, &ap)| {
                            (hp * ap > 0.).then_some(([h as i32, a as i32], hp * ap))
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        fn visit(
            model: &Model,
            target: usize,
            rank: usize,
            distributions: &[Vec<([i32; 2], f64)>],
            fixture: usize,
            campaign: &mut [Campaign],
            scores: &mut [[i32; 2]],
            mass: f64,
            total: &mut f64,
        ) {
            if fixture == distributions.len() {
                let mut order = vec![0; model.n];
                model.standings(&mut order, campaign, scores, &mut Rng::new(4));
                if order[rank] == target {
                    *total += mass;
                }
                return;
            }
            let game = &model.fixtures[fixture];
            for &(score, probability) in &distributions[fixture] {
                scores[game.request_index] = score;
                let prior_campaign = campaign.to_vec();
                model.add(campaign, fixture, score);
                visit(
                    model,
                    target,
                    rank,
                    distributions,
                    fixture + 1,
                    campaign,
                    scores,
                    mass * probability,
                    total,
                );
                campaign.copy_from_slice(&prior_campaign);
            }
        }
        let mut total = 0.;
        let mut campaign = model.base.clone();
        let mut scores = model.empty_scores();
        visit(
            model,
            target,
            rank,
            &distributions,
            0,
            &mut campaign,
            &mut scores,
            1.,
            &mut total,
        );
        total
    }

    #[test]
    fn particle_filter_matches_exact_shared_rival_score_rank_oracle() {
        let model = triangle();
        assert_eq!(model.fixtures.len(), 3);
        // Rival teams 1 and 2 share a fixture, and GD/GF settle point ties.
        for rank in [0, 2] {
            let exact = exact_rank_probability(&model, 0, rank);
            let sampled = stream(&model, 0, rank, 991 + rank as i64, "oracle");
            assert!((0. ..1.).contains(&exact), "rank={rank}, exact={exact}");
            assert!(
                (sampled.probability - exact).abs() < (5. * sampled.std_err).max(0.01),
                "rank={rank} exact={exact} sampled={} se={}",
                sampled.probability,
                sampled.std_err
            );
        }
    }

    #[test]
    fn particle_fallback_resolves_target_without_remaining_fixtures() {
        let model = no_target_fixture_large_group();
        assert!(model
            .fixtures
            .iter()
            .all(|fixture| fixture.home != 0 && fixture.away != 0));
        let exact = 1. - model.fixtures[0].prob[2];
        let mut estimates = vec![
            Estimate {
                probability: 1.,
                ..Estimate::default()
            };
            model.n * model.n
        ];
        estimates[0] = Estimate {
            reachability: "undecided".into(),
            ..Estimate::default()
        };
        estimates[1] = Estimate {
            probability: 0.25,
            design: "prior_estimate".into(),
            reachability: "witness".into(),
            ..Estimate::default()
        };

        let (conditioned_count, _) =
            crate::tail_conditioned::run(&model, 73, 1, &mut estimates, None);
        assert_eq!(conditioned_count, 0);
        assert_eq!(estimates[0].probability, 0.);

        let (particle_count, _) = run(&model, 7302, 1, &mut estimates, None);
        assert_eq!(particle_count, 1);
        let estimate = &estimates[0];
        assert!(estimate.probability > 0.);
        assert!(
            (estimate.probability - exact).abs() < (5. * estimate.std_err).max(0.02),
            "expected={exact}, estimate={}, se={}",
            estimate.probability,
            estimate.std_err
        );
        assert_eq!(estimate.design, "sequential_tail_particles");
        assert_eq!(estimates[1].probability, 0.25);
        assert_eq!(estimates[1].design, "prior_estimate");
        assert_eq!(estimates[1].reachability, "witness");
    }

    #[test]
    fn particle_cells_include_both_edges_for_two_and_three_teams() {
        for n in [2, 3] {
            let model = small_group(n);
            let estimates = vec![Estimate::default(); model.n * model.n];
            let cells = candidate_cells(&model, &estimates);
            for team in 0..model.request.team_groups.len() {
                assert!(cells.contains(&(team, 0)), "n={n}, team={team}");
                assert!(cells.contains(&(team, n - 1)), "n={n}, team={team}");
            }
        }
    }

    #[test]
    fn particle_candidates_reject_extra_fixture_teams() {
        let request = serde_json::from_value(json!({
            "id": 7304,
            "phase": {"sort":"pt,gd", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups": [{"team_id":0,"add_sub":0},{"team_id":1,"add_sub":0}],
            "games": [{"id":1,"home_id":0,"away_id":2,"home_power":0.4,"away_power":0.5}]
        }))
        .unwrap();
        let model = Model::new(request).unwrap();
        assert_eq!(model.n, 2);
        assert_eq!(model.ids.len(), 3);
        assert!(candidate_cells(&model, &vec![Estimate::default(); model.n * model.n]).is_empty());
    }
}
