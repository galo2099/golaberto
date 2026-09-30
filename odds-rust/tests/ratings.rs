use golaberto_odds::{
    http,
    logging::RequestLog,
    ratings::{self, Game, Request, TeamRating},
};
use serde_json::{json, Value};

fn fixture() -> Request {
    Request {
        games: (0..360)
            .map(|i| Game {
                home_id: 1 + i % 3,
                away_id: 1 + (i + 1) % 3,
                phase_id: if i < 300 { 1 } else { 2 },
                home_score: (i % 5) as f64,
                away_score: (i * 7 % 4) as f64,
                timestamp: 1500000000 + i as i64 * 5 * 86400,
                length: if i % 17 == 0 { 4. / 3. } else { 1. },
                advantage: [0., 0.16133676871779334, -0.16133676871779334][i as usize % 3],
            })
            .collect(),
        ratings: (1..=4)
            .map(|id| TeamRating {
                id,
                offense: if id % 2 == 0 { 1.4 } else { 0. },
                defense: if id % 2 == 0 { 1.2 } else { 0. },
                team: 0.,
            })
            .collect(),
        phases_to_eval: vec![2],
    }
}
fn assert_close(actual: &Value, expected: &Value, tolerance: f64) {
    match (actual, expected) {
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (k, v) in a {
                assert_close(v, &b[k], tolerance);
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                assert_close(a, b, tolerance);
            }
        }
        (Value::Number(a), Value::Number(b)) => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= tolerance,
            "{a} != {b}"
        ),
        _ => assert_eq!(actual, expected),
    }
}

#[test]
fn spi_eval_and_historical_match_go_oracle() {
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/ratings-go-oracle.json")).unwrap();
    let request = fixture();
    request.validate().unwrap();
    let spi = ratings::spi(&request.games, &request.initial()).unwrap();
    assert!(spi[&4].is_none());
    assert_close(&serde_json::to_value(spi).unwrap(), &expected["spi"], 1e-10);
    let eval = ratings::evaluate(&request).unwrap();
    assert_close(
        &serde_json::to_value(eval).unwrap(),
        &expected["eval"],
        1e-12,
    );
    let mut rows = ratings::historical(&request).unwrap();
    rows.sort_by(|a, b| (a.team_id, &a.measure_date).cmp(&(b.team_id, &b.measure_date)));
    assert_close(
        &serde_json::to_value(rows).unwrap(),
        &expected["historic"],
        0.500001e-6,
    );
}

#[test]
fn rails_null_ratings_and_go_field_names_are_accepted() {
    let request: Request = serde_json::from_value(json!({"Games":[],"Ratings":[
        {"id":1,"offense":null,"defense":null}, {"Id":2,"Offense":1.2,"Defense":1.3,"Team":null}
    ],"Phases_To_Eval":null}))
    .unwrap();
    request.validate().unwrap();
    assert_eq!(request.ratings[0].offense, 0.);
    assert_eq!(request.ratings[1].defense, 1.3);
    assert_eq!(
        serde_json::to_value(ratings::spi(&[], &request.initial()).unwrap()).unwrap(),
        json!({"1":null,"2":null})
    );
}

#[test]
fn invalid_rating_requests_are_rejected_before_computation() {
    let mut request = fixture();
    request.games[0].home_id = 999;
    assert!(request.validate().is_err());
    request = fixture();
    request.games.swap(0, 1);
    assert!(request.validate().is_err());
    request = fixture();
    request.games[0].length = 0.;
    assert!(request.validate().is_err());
    request = fixture();
    request.ratings.push(request.ratings[0].clone());
    assert!(request.validate().is_err());
    request = fixture();
    request.phases_to_eval.clear();
    assert!(ratings::evaluate(&request).is_err());
}

#[test]
fn empty_historical_request_needs_no_database_and_deprecated_route_is_absent() {
    let log = RequestLog::new();
    let response =
        http::execute("/historic_ratings", br#"{"games":[],"ratings":[]}"#, &log).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&response).unwrap(),
        json!({"ratings":{},"offense":{},"defense":{},"dates":[]})
    );
    assert_eq!(
        http::execute("/player_ratings", b"", &log)
            .unwrap_err()
            .status,
        404
    );
    assert_eq!(
        http::execute("/spi", b"not json", &log).unwrap_err().status,
        400
    );
}
