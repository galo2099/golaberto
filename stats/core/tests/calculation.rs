mod support;
use serde_json::Value;
fn fixture() -> support::Fixture {
    support::read(&serde_json::from_str::<Value>(include_str!("fixtures/input.json")).unwrap())
}
#[test]
fn full_player_calculation_matches_frozen_legacy_float_bits() {
    let f = fixture();
    let result =
        player_ratings::calculate(&f.games, &f.goals, f.ratings, &f.players, f.now).unwrap();
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/legacy-output.json")).unwrap();
    assert_eq!(support::output(&result, f.now), expected);
}
#[test]
fn invalid_history_positions_and_order_are_errors_before_writes() {
    let mut f = fixture();
    f.ratings.remove(&1);
    assert!(player_ratings::calculate(&f.games, &f.goals, f.ratings, &f.players, f.now).is_err());
    let mut f = fixture();
    f.players.get_mut(&1).unwrap()[0].pos = "unknown".into();
    assert!(player_ratings::calculate(&f.games, &f.goals, f.ratings, &f.players, f.now).is_err());
    let mut f = fixture();
    f.games.swap(0, 1);
    assert!(player_ratings::calculate(&f.games, &f.goals, f.ratings, &f.players, f.now).is_err());
}
#[test]
fn no_eligible_games_is_an_empty_update() {
    let f = fixture();
    let result = player_ratings::calculate(&[], &f.goals, f.ratings, &f.players, f.now).unwrap();
    assert!(result.player_ratings.is_empty());
    assert!(result.player_game_ratings.is_empty());
}
