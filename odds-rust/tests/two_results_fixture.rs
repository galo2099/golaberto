use golaberto_odds::model::{Model, Request};
use serde_json::Value;
use std::collections::HashMap;

const BASELINE: &str = include_str!(
    "../../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json"
);
const TWO_RESULTS: &str = include_str!(
    "../../experiments/rare_positions/reference/2026-10-04-two-results/inputs/group-16498-two-results.json"
);

#[test]
fn two_results_snapshot_refreshes_only_remaining_game_powers_and_model_base() {
    let baseline: Value = serde_json::from_str(BASELINE).unwrap();
    let actual: Value = serde_json::from_str(TWO_RESULTS).unwrap();
    let mut expected = baseline.clone();
    let actual_games: HashMap<_, _> = actual["games"]
        .as_array()
        .unwrap()
        .iter()
        .map(|game| (game["id"].as_i64().unwrap(), game))
        .collect();

    let scores = HashMap::from([(364007, (1, 2)), (363999, (1, 0))]);
    let mut changed = 0;
    let mut refreshed_means = 0;
    for game in expected["games"].as_array_mut().unwrap() {
        let id = game["id"].as_i64().unwrap() as i32;
        if let Some(&(home_score, away_score)) = scores.get(&id) {
            game["home_score"] = home_score.into();
            game["away_score"] = away_score.into();
            game["played"] = true.into();
            changed += 1;
        } else if game["played"].as_bool() == Some(false) {
            let updated = actual_games[&(id as i64)];
            for key in ["home_power", "away_power"] {
                let mean = updated[key].as_f64().unwrap();
                assert!(
                    mean.is_finite() && mean > 0.0,
                    "game {id} {key} must be finite and positive"
                );
                if game[key].as_f64() != Some(mean) {
                    refreshed_means += 1;
                }
                game[key] = updated[key].clone();
            }
        }
    }
    assert_eq!(changed, scores.len());
    assert_eq!(actual["games"].as_array().unwrap().len(), 380);
    assert_eq!(
        refreshed_means, 200,
        "expected refreshed means for remaining games"
    );
    assert_eq!(actual, expected, "snapshot has unexpected field changes");

    for (&id, &(home_score, away_score)) in &scores {
        let game = actual_games[&(id as i64)];
        assert_eq!(game["home_score"].as_i64(), Some(home_score));
        assert_eq!(game["away_score"].as_i64(), Some(away_score));
        assert_eq!(game["played"].as_bool(), Some(true));
    }

    let baseline_request: Request = serde_json::from_value(baseline).unwrap();
    let actual_request: Request = serde_json::from_value(actual).unwrap();
    let baseline_model = Model::new(baseline_request).unwrap();
    let actual_model = Model::new(actual_request).unwrap();
    assert_eq!(actual_model.fixtures.len(), 101);
    assert_eq!(baseline_model.fixtures.len(), 103);

    let baseline_fixture_ids: Vec<_> = baseline_model
        .fixtures
        .iter()
        .map(|fixture| baseline_model.request.games[fixture.request_index].id)
        .filter(|id| !scores.contains_key(id))
        .collect();
    let actual_fixture_ids: Vec<_> = actual_model
        .fixtures
        .iter()
        .map(|fixture| actual_model.request.games[fixture.request_index].id)
        .collect();
    assert_eq!(actual_fixture_ids, baseline_fixture_ids);

    let baseline_by_id: HashMap<_, _> = baseline_model
        .ids
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, baseline_model.base[index]))
        .collect();
    let actual_by_id: HashMap<_, _> = actual_model
        .ids
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, actual_model.base[index]))
        .collect();
    let expected_deltas = HashMap::from([
        (14, (0, 0, 1, 0, 1, 2, 0, 0)),
        (6, (3, 1, 0, 0, 2, 1, 2, 0)),
        (4, (3, 1, 0, 0, 1, 0, 0, 0)),
        (225, (0, 0, 1, 0, 0, 1, 0, 0)),
    ]);
    for (&team_id, &(points, wins, losses, draws, goals_for, goals_against, away, bias)) in
        &expected_deltas
    {
        let before = baseline_by_id[&team_id];
        let after = actual_by_id[&team_id];
        assert_eq!(
            after.points - before.points,
            points,
            "team {team_id} points"
        );
        assert_eq!(after.wins - before.wins, wins, "team {team_id} wins");
        assert_eq!(
            after.losses - before.losses,
            losses,
            "team {team_id} losses"
        );
        assert_eq!(after.draws - before.draws, draws, "team {team_id} draws");
        assert_eq!(after.gf - before.gf, goals_for, "team {team_id} goals for");
        assert_eq!(
            after.ga - before.ga,
            goals_against,
            "team {team_id} goals against"
        );
        assert_eq!(after.away - before.away, away, "team {team_id} away goals");
        assert_eq!(after.bias - before.bias, bias, "team {team_id} bias");
    }
    for team_id in baseline_model
        .ids
        .iter()
        .filter(|id| !expected_deltas.contains_key(id))
    {
        assert_eq!(
            baseline_by_id[team_id], actual_by_id[team_id],
            "unaffected team {team_id} campaign changed"
        );
    }
}
