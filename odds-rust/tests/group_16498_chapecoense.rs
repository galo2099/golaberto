use golaberto_odds::{api, model::Model, rng::Rng, sampling::masses};
use std::collections::{HashMap, HashSet};

#[derive(serde::Deserialize)]
struct Witness {
    team_id: i32,
    rank: usize,
    scores: Vec<WitnessScore>,
    standings: Vec<WitnessStanding>,
}

#[derive(serde::Deserialize)]
struct WitnessScore {
    game_id: i32,
    home_score: i32,
    away_score: i32,
}

#[derive(serde::Deserialize)]
struct WitnessStanding {
    rank: usize,
    team_id: i32,
    points: i32,
    wins: i32,
    gd: i32,
}

fn snapshot_request() -> golaberto_odds::model::Request {
    serde_json::from_str(include_str!("fixtures/group-16498-2026-10-18.json")).unwrap()
}

fn chapecoense_ninth_witness() -> Witness {
    serde_json::from_str(include_str!(
        "fixtures/group-16498-2026-10-18-chapecoense-9-witness.json"
    ))
    .unwrap()
}

#[test]
fn october_18_group_snapshot_preserves_chapecoense_campaign_and_fixtures() {
    let request = snapshot_request();

    assert_eq!(request.id, 16498);
    assert_eq!(request.phase.sort, "pt,w,gd,gf,gp,name");
    assert_eq!(request.team_groups.len(), 20);
    assert_eq!(request.games.len(), 380);
    assert_eq!(request.games.iter().filter(|game| game.played).count(), 302);

    let model = Model::new(request.clone()).unwrap();
    assert_eq!(model.n, 20);
    assert_eq!(model.fixtures.len(), 78);

    // Chapecoense-SC is team 318 in this group snapshot.
    let team_index = model.indices[&318];
    let campaign = model.base[team_index];
    assert_eq!(campaign.points, 18);
    assert_eq!((campaign.wins, campaign.draws, campaign.losses), (3, 9, 18));
    assert_eq!((campaign.gf, campaign.ga, campaign.away), (30, 62, 11));
    assert_eq!(campaign.gf - campaign.ga, -32);

    let remaining_fixtures = request
        .games
        .iter()
        .filter(|game| !game.played && (game.home_id == 318 || game.away_id == 318))
        .map(|game| (game.id, game.home_id, game.away_id))
        .collect::<Vec<_>>();
    assert_eq!(
        remaining_fixtures,
        vec![
            (364001, 318, 18),
            (364109, 318, 8),
            (364125, 225, 318),
            (364131, 318, 584),
            (364145, 110, 318),
            (364153, 318, 15),
            (364161, 318, 16),
            (364172, 17, 318),
        ]
    );

    let mut scores = model.empty_scores();
    for (index, game) in model.request.games.iter().enumerate() {
        if game.played {
            scores[index] = [game.home_score, game.away_score];
        }
    }
    let mut order = vec![0; model.n];
    model.standings(&mut order, &model.base, &scores, &mut Rng::new(808));
    assert_eq!(
        order[19], team_index,
        "Chapecoense is 20th after 302 matches"
    );
}

#[test]
fn only_eight_chapecoense_wins_leave_at_most_eight_rivals_ahead_on_points_and_wins() {
    let model = Model::new(snapshot_request()).unwrap();
    let team = model.indices[&318];
    let mut paths_with_at_most_eight_rivals_ahead = Vec::new();
    let outcomes = [
        (3, true),  // win
        (1, false), // draw
        (0, false), // loss
    ];

    for mut encoded_path in 0..3usize.pow(8) {
        let mut points = model.base[team].points;
        let mut wins = model.base[team].wins;
        let mut is_all_wins = true;
        for _ in 0..8 {
            let (earned_points, is_win) = outcomes[encoded_path % 3];
            encoded_path /= 3;
            points += earned_points;
            wins += usize::from(is_win) as i32;
            is_all_wins &= is_win;
        }

        let guaranteed_ahead = model
            .base
            .iter()
            .enumerate()
            .filter(|(index, rival)| {
                *index != team
                    && (rival.points > points || (rival.points == points && rival.wins > wins))
            })
            .count();
        if guaranteed_ahead <= 8 {
            paths_with_at_most_eight_rivals_ahead.push((points, wins, is_all_wins));
        }
    }

    assert_eq!(paths_with_at_most_eight_rivals_ahead.len(), 1);
    assert_eq!(paths_with_at_most_eight_rivals_ahead[0], (42, 11, true));
}

#[test]
fn scored_witness_places_chapecoense_ninth_with_native_positive_score_mass() {
    let model = Model::new(snapshot_request()).unwrap();
    let witness = chapecoense_ninth_witness();
    assert_eq!(witness.team_id, 318);
    assert_eq!(witness.rank, 9);
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
    assert!(witness
        .scores
        .iter()
        .all(|result| seen.contains(&result.game_id)));
    for (request_index, game) in model.request.games.iter().enumerate() {
        if game.played {
            assert_eq!(scores[request_index], [game.home_score, game.away_score]);
        }
    }

    let team = model.indices[&witness.team_id];
    let campaign = campaigns[team];
    assert_eq!(campaign.points, 42);
    assert_eq!(campaign.wins, 11);
    assert_eq!(campaign.gf - campaign.ga, -24);

    let mut order = vec![0; model.n];
    model.standings(&mut order, &campaigns, &scores, &mut Rng::new(808));
    assert_eq!(order[witness.rank - 1], team);

    for rival in campaigns
        .iter()
        .enumerate()
        .filter_map(|(index, campaign)| {
            (index != team && campaign.points == 42).then_some(campaign)
        })
    {
        assert!(rival.wins < campaign.wins);
    }

    assert_eq!(witness.standings.len(), model.n);
    let target_standing = witness
        .standings
        .iter()
        .find(|standing| standing.team_id == witness.team_id)
        .unwrap();
    assert_eq!(
        (
            target_standing.rank,
            target_standing.points,
            target_standing.wins
        ),
        (9, 42, 11)
    );
    assert_eq!(target_standing.gd, -24);
}

#[test]
fn seed_808_reports_chapecoense_ninth_as_reachable_for_one_and_four_workers() {
    let request = snapshot_request();
    let (one_worker, _) = api::calculate(request.clone(), 808, 1, 100).unwrap();
    let (four_workers, _) = api::calculate(request, 808, 4, 100).unwrap();

    for response in [&one_worker, &four_workers] {
        let estimate = &response.rare_position_estimates[&318][&8];
        assert_eq!(estimate.reachability, "reachable");
        assert!(estimate.probability.is_finite());
        assert!((0.0..=1.0).contains(&estimate.probability));
        assert_eq!(
            response.team_odds[&318].pos[8],
            estimate.probability * 100.0
        );
        if estimate.probability == 0.0 {
            assert!(!estimate.meets_precision_goal);
            assert!(estimate.zero_hit_upper_95.is_finite() && estimate.zero_hit_upper_95 > 0.0);
        }
    }

    assert_eq!(
        serde_json::to_value(&one_worker.rare_position_estimates[&318][&8]).unwrap(),
        serde_json::to_value(&four_workers.rare_position_estimates[&318][&8]).unwrap()
    );
}
