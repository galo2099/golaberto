use golaberto_odds::{
    api::initial_scout,
    model::{Championship, Game, Model, Phase, Request, Team, Zone},
};
use std::collections::BTreeMap;

fn request(games: Vec<Game>, zones: Vec<Zone>) -> Request {
    Request {
        id: 17,
        zones,
        phase: Phase {
            sort: "pt,w,gd,gf,name".into(),
            championship: Championship {
                point_win: 3,
                point_draw: 1,
                point_loss: 0,
            },
            bonus_points: 0,
            bonus_points_threshold: 0,
        },
        games,
        team_groups: [1, 2]
            .into_iter()
            .map(|team_id| Team {
                team_id,
                add_sub: 0,
                bias: 0,
            })
            .collect(),
    }
}

fn game(
    id: i32,
    home_id: i32,
    away_id: i32,
    played: bool,
    home_power: f64,
    away_power: f64,
) -> Game {
    Game {
        id,
        home_id,
        away_id,
        home_score: 2,
        away_score: 1,
        home_power,
        away_power,
        played,
    }
}

#[test]
fn empty_zones_return_legacy_zero_importance_for_unplayed_in_group_sides_only() {
    let model = Model::new(request(
        vec![
            game(70, 1, 2, true, 0.0, 0.0),
            game(11, 1, 2, false, 1.0, 1.0),
            game(12, 90, 2, false, 1.0, 1.0),
            game(13, 90, 91, false, 1.0, 1.0),
            // Duplicate request IDs retain BTreeMap's legacy last-fixture-wins behavior.
            game(11, 1, 90, false, 1.0, 1.0),
        ],
        vec![],
    ))
    .unwrap();

    let importance = initial_scout(&model, 20_000, 808);
    let expected = BTreeMap::from([(11, [Some(0.0), None]), (12, [None, Some(0.0)])]);
    assert_eq!(importance, expected);
}

#[test]
fn nonempty_zones_still_use_sampled_game_importance() {
    let model = Model::new(request(
        vec![game(21, 1, 2, false, 2.0, 2.0)],
        vec![Zone { position: vec![1] }],
    ))
    .unwrap();

    let importance = initial_scout(&model, 5_000, 808);
    let sides = importance[&21];
    assert!(sides[0].is_some_and(|value| value > 0.0), "{sides:?}");
    assert!(sides[1].is_some_and(|value| value > 0.0), "{sides:?}");
}
