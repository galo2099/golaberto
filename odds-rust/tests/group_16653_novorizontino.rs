use golaberto_odds::{api, model::Model, rng::Rng, sampling::masses};
use std::collections::{HashMap, HashSet};

#[derive(serde::Deserialize)]
struct Witness {
    team_id: i32,
    rank: usize,
    points: i32,
    wins: i32,
    goal_difference: i32,
    scores: Vec<WitnessScore>,
}

#[derive(serde::Deserialize)]
struct WitnessScore {
    game_id: i32,
    home_score: i32,
    away_score: i32,
}

fn snapshot_request() -> golaberto_odds::model::Request {
    serde_json::from_str(include_str!("fixtures/group-16653-2026-10-11.json")).unwrap()
}

fn novorizontino_fifteenth_witness() -> Witness {
    serde_json::from_str(include_str!(
        "fixtures/group-16653-2026-10-11-novorizontino-15-witness.json"
    ))
    .unwrap()
}

#[test]
fn october_11_group_snapshot_preserves_novorizontino_campaign_and_fixtures() {
    let request = snapshot_request();
    let model = Model::new(request.clone()).unwrap();

    assert_eq!(request.id, 16653);
    assert_eq!(request.phase.sort, "pt,w,gd,gf,head,name");
    assert_eq!(request.team_groups.len(), 20);
    assert_eq!(request.games.len(), 380);
    assert_eq!(request.games.iter().filter(|game| game.played).count(), 325);
    assert_eq!(model.n, 20);
    assert_eq!(model.fixtures.len(), 55);

    let team_index = model.indices[&2064];
    let campaign = model.base[team_index];
    assert_eq!(campaign.points, 57);
    assert_eq!((campaign.wins, campaign.draws, campaign.losses), (16, 9, 7));
    assert_eq!((campaign.gf, campaign.ga, campaign.away), (54, 27, 28));

    let remaining_opponents = request
        .games
        .iter()
        .filter(|game| !game.played && (game.home_id == 2064 || game.away_id == 2064))
        .map(|game| {
            if game.home_id == 2064 {
                game.away_id
            } else {
                game.home_id
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(remaining_opponents, vec![9, 70, 279, 588, 22, 73]);
}

#[test]
fn scored_witness_places_novorizontino_fifteenth_with_positive_score_mass() {
    let model = Model::new(snapshot_request()).unwrap();
    let witness = novorizontino_fifteenth_witness();
    assert_eq!(witness.team_id, 2064);
    assert_eq!(witness.rank, 15);
    assert_eq!(witness.scores.len(), model.fixtures.len());

    let scores_by_id = witness
        .scores
        .iter()
        .map(|score| (score.game_id, score))
        .collect::<HashMap<_, _>>();
    assert_eq!(scores_by_id.len(), witness.scores.len());

    let mut campaigns = model.base.clone();
    let mut scores = model.empty_scores();
    let mut seen = HashSet::new();
    for (fixture_index, fixture) in model.fixtures.iter().enumerate() {
        let game = &model.request.games[fixture.request_index];
        assert!(!game.played, "fixture {} is already played", game.id);
        assert!(seen.insert(game.id), "duplicate fixture id {}", game.id);
        let result = scores_by_id
            .get(&game.id)
            .unwrap_or_else(|| panic!("witness is missing future game {}", game.id));
        assert!(
            (0..=3).contains(&result.home_score) && (0..=3).contains(&result.away_score),
            "witness score for game {} exceeds three goals",
            game.id
        );
        assert!(masses(fixture.home_sampler.mean)[result.home_score as usize] > 0.0);
        assert!(masses(fixture.away_sampler.mean)[result.away_score as usize] > 0.0);
        scores[fixture.request_index] = [result.home_score, result.away_score];
        model.add(
            &mut campaigns,
            fixture_index,
            [result.home_score, result.away_score],
        );
    }
    assert_eq!(seen.len(), witness.scores.len());
    let team = model.indices[&witness.team_id];
    let campaign = campaigns[team];
    assert_eq!(campaign.points, witness.points);
    assert_eq!(campaign.wins, witness.wins);
    assert_eq!(campaign.gf - campaign.ga, witness.goal_difference);

    let mut order = vec![0; model.n];
    model.standings(&mut order, &campaigns, &scores, &mut Rng::new(808));
    assert_eq!(order[witness.rank - 1], team);
}

#[test]
fn seed_808_reports_reachable_fifteenth_place_even_without_a_sampled_hit() {
    let request = snapshot_request();
    let (response, _) = api::calculate(request, 808, 1, 100).unwrap();
    let estimate = &response.rare_position_estimates[&2064][&14];

    assert_eq!(estimate.reachability, "reachable");
    assert!(estimate.probability > 0.0);
    assert_eq!(
        response.team_odds[&2064].pos[14],
        estimate.probability * 100.0
    );
    assert!(estimate.meets_precision_goal);
    assert!(estimate.relative_se.unwrap_or(f64::INFINITY) <= 0.5);
    assert!(estimate.ess >= 10.0);
    assert!(estimate.design.contains("gd_refinement") || estimate.design.contains("gd_cohort"));
}

#[test]
fn seed_809_reports_positive_reachable_fifteenth_place_deterministically() {
    let request = snapshot_request();
    let (one_worker, _) = api::calculate(request.clone(), 809, 1, 100).unwrap();
    let (four_workers, _) = api::calculate(request, 809, 4, 100).unwrap();

    for response in [&one_worker, &four_workers] {
        let estimate = &response.rare_position_estimates[&2064][&14];
        assert_eq!(estimate.reachability, "reachable");
        assert!(estimate.probability.is_finite() && estimate.probability > 0.0);
        assert!(estimate.meets_precision_goal);
        assert!(estimate.relative_se.unwrap_or(f64::INFINITY) <= 0.5);
        assert!(estimate.ess >= 10.0);
        assert!(estimate.design.contains("gd_refinement") || estimate.design.contains("gd_cohort"));
        assert_eq!(
            response.team_odds[&2064].pos[14],
            estimate.probability * 100.0
        );
    }
    assert_eq!(
        serde_json::to_value(&one_worker.rare_position_estimates[&2064][&14]).unwrap(),
        serde_json::to_value(&four_workers.rare_position_estimates[&2064][&14]).unwrap()
    );
}
