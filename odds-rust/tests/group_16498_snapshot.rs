use golaberto_odds::model::Model;
use golaberto_odds::{
    api,
    pool::Bounds,
    rank_proof::{Options, RankProof},
    rng::Rng,
    search::Cell,
};
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
    serde_json::from_str(include_str!("fixtures/group-16498-2026-10-10.json")).unwrap()
}

fn palmeiras_witness() -> Witness {
    serde_json::from_str(include_str!(
        "fixtures/group-16498-2026-10-10-palmeiras-13-witness.json"
    ))
    .unwrap()
}

#[test]
fn october_10_group_snapshot_passes_integrity_checks() {
    let request = snapshot_request();

    assert_eq!(request.id, 16498);
    assert_eq!(request.phase.sort, "pt,w,gd,gf,gp,name");
    assert_eq!(request.team_groups.len(), 20);
    assert_eq!(request.games.len(), 380);
    assert_eq!(request.games.iter().filter(|game| !game.played).count(), 90);

    let game_ids = request
        .games
        .iter()
        .map(|game| game.id)
        .collect::<HashSet<_>>();
    assert_eq!(game_ids.len(), request.games.len());
    assert!(request.games.iter().all(|game| {
        game.home_power.is_finite()
            && game.home_power > 0.0
            && game.away_power.is_finite()
            && game.away_power > 0.0
    }));

    let model = Model::new(request).unwrap();
    assert_eq!(model.n, 20);
    assert_eq!(model.fixtures.len(), 90);
}

#[test]
fn score_witness_places_palmeiras_thirteenth_and_rank_proof_does_not_refute_it() {
    let request = snapshot_request();
    let witness = palmeiras_witness();
    let model = Model::new(request).unwrap();

    assert_eq!(witness.team_id, 16);
    assert_eq!(witness.rank, 13);
    assert_eq!(witness.points, 60);
    assert_eq!(witness.wins, 17);
    assert_eq!(witness.goal_difference, -9);
    assert_eq!(witness.scores.len(), model.fixtures.len());
    assert!(witness
        .scores
        .iter()
        .all(|result| result.home_score >= 0 && result.away_score >= 0));

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
    assert_eq!(campaigns[team].points, witness.points);
    assert_eq!(campaigns[team].wins, witness.wins);
    assert_eq!(
        campaigns[team].gf - campaigns[team].ga,
        witness.goal_difference
    );

    let mut order = vec![0; model.n];
    model.standings(&mut order, &campaigns, &scores, &mut Rng::new(808));
    assert_eq!(order[witness.rank - 1], team);

    let report = RankProof::new(&model).unwrap().prove(
        Cell {
            team,
            rank: witness.rank - 1,
        },
        witness.rank - 1,
        100_000,
        Options::default(),
    );
    assert!(
        !report.impossible,
        "rank proof contradicted a concrete score witness"
    );
}

#[test]
fn default_calculation_recovers_palmeiras_thirteenth_for_one_and_four_workers() {
    let request = snapshot_request();
    let (one_worker, _) = api::calculate(request.clone(), 808, 1, 20_000).unwrap();
    let (four_workers, _) = api::calculate(request, 808, 4, 20_000).unwrap();

    for response in [&one_worker, &four_workers] {
        let estimate = &response.rare_position_estimates[&16][&12];
        assert_eq!(estimate.reachability, "reachable");
        assert!(
            estimate.probability.is_finite()
                && estimate.probability > 0.
                && estimate.probability <= 1.
        );
        assert!(estimate.std_err.is_finite() && estimate.std_err > 0.);
        assert!(estimate.meets_precision_goal);
    }
    assert_eq!(
        serde_json::to_value(&one_worker).unwrap(),
        serde_json::to_value(&four_workers).unwrap()
    );
}

fn november_19_request() -> golaberto_odds::model::Request {
    serde_json::from_str(include_str!("fixtures/group-16498-2026-11-19.json")).unwrap()
}

#[test]
fn november_19_cruzeiro_odds_and_point_win_bounds_preserve_tiebreak_cases() {
    const CRUZEIRO: i32 = 15;
    let request = november_19_request();
    let (response, _) = api::calculate(request.clone(), 808, 1, 100).unwrap();
    let estimates = &response.rare_position_estimates[&CRUZEIRO];

    for place in 0..20 {
        if [2, 3].contains(&place) {
            assert!(
                response.team_odds[&CRUZEIRO].pos[place] > 0.0,
                "place {}",
                place + 1
            );
            assert_eq!(
                estimates[&place].reachability,
                "reachable",
                "place {}",
                place + 1
            );
        } else {
            assert_eq!(
                response.team_odds[&CRUZEIRO].pos[place],
                0.0,
                "place {}",
                place + 1
            );
            assert_eq!(estimates[&place].probability, 0.0, "place {}", place + 1);
            assert_eq!(
                estimates[&place].reachability,
                "impossible",
                "place {}",
                place + 1
            );
        }
    }

    for place in 0..20 {
        let total: f64 = response
            .team_odds
            .values()
            .map(|odds| odds.pos[place])
            .sum();
        assert!(
            (total - 100.0).abs() < 1e-10,
            "place {} totals {total}",
            place + 1
        );
    }
    for (&team, odds) in &response.team_odds {
        assert_eq!(odds.pos.len(), 20, "team {team}");
        assert!(
            (odds.pos.iter().sum::<f64>() - 100.0).abs() < 1e-10,
            "team {team}"
        );
    }

    let model = Model::new(request).unwrap();
    let team = model.indices[&CRUZEIRO];
    assert_eq!((model.base[team].points, model.base[team].wins), (63, 19));
    let bounds = Bounds::new(&model);
    assert!(
        bounds.allowed(team, 3, 0),
        "Fluminense can still pass 63 points"
    );
    for rival_id in [5, 74] {
        let rival = model.indices[&rival_id];
        let max_added = bounds.max[rival] - bounds.current[rival];
        assert!(
            !bounds.allowed(rival, 2, max_added),
            "team {rival_id} cannot finish 3rd"
        );
    }
}

#[test]
fn point_goal_difference_bounds_keep_fifth_place_open_on_equal_points() {
    let mut request = november_19_request();
    request.phase.sort = "pt,gd,w,gf".into();
    let model = Model::new(request).unwrap();
    let cruzeiro = model.indices[&15];
    assert!(Bounds::new(&model).allowed(cruzeiro, 4, 0));

    // Athletico starts at 54 points and 16 wins in this controlled state.
    // Three wins reach 63/19, exactly Cruzeiro's current points and wins;
    // goal difference remains available to decide the tie.
    let mut model = Model::new(november_19_request()).unwrap();
    let cruzeiro = model.indices[&15];
    let athletico = model.indices[&5];
    model.base[athletico].wins = 16;
    let bounds = Bounds::new(&model);
    assert!(bounds.allowed(cruzeiro, 4, 0));
}
