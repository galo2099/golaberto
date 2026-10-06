use chrono::{NaiveDate, NaiveDateTime};
use player_ratings::{Game, Goal, HistoricalRating, PlayerGame, PlayerGamePos};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};

pub struct Fixture {
    pub games: Vec<Game>,
    pub goals: HashMap<i32, smallvec::SmallVec<[Goal; 4]>>,
    pub ratings: HashMap<i32, VecDeque<HistoricalRating>>,
    pub players: HashMap<i32, Vec<PlayerGamePos>>,
    pub now: i64,
}
fn integer(v: &Value, key: &str) -> i32 {
    v[key].as_i64().unwrap() as i32
}
pub fn read(v: &Value) -> Fixture {
    let games = v["games"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| Game {
            id: integer(g, "id"),
            home_id: integer(g, "home_id"),
            away_id: integer(g, "away_id"),
            date: NaiveDateTime::parse_from_str(g["date"].as_str().unwrap(), "%Y-%m-%d %H:%M:%S")
                .unwrap(),
            home_field: integer(g, "home_field"),
            home_aet: g["home_aet"].as_i64().map(|i| i as i32),
        })
        .collect();
    let mut goals: HashMap<i32, smallvec::SmallVec<[Goal; 4]>> = HashMap::new();
    for g in v["goals"].as_array().unwrap() {
        goals.entry(integer(g, "game_id")).or_default().push(Goal {
            player_id: integer(g, "player_id"),
            team_id: integer(g, "team_id"),
            time: integer(g, "time"),
            penalty: g["penalty"].as_bool().unwrap(),
            own_goal: g["own_goal"].as_bool().unwrap(),
        });
    }
    let mut ratings: HashMap<i32, VecDeque<HistoricalRating>> = HashMap::new();
    for r in v["ratings"].as_array().unwrap() {
        let team_id = integer(r, "team_id");
        ratings
            .entry(team_id)
            .or_default()
            .push_back(HistoricalRating {
                team_id,
                measure_date: NaiveDate::parse_from_str(
                    r["measure_date"].as_str().unwrap(),
                    "%Y-%m-%d",
                )
                .unwrap(),
                off_rating: r["off_rating"].as_f64().unwrap() as f32,
                def_rating: r["def_rating"].as_f64().unwrap() as f32,
            });
    }
    let mut players: HashMap<i32, Vec<PlayerGamePos>> = HashMap::new();
    for p in v["appearances"].as_array().unwrap() {
        let game_id = integer(p, "game_id");
        players.entry(game_id).or_default().push(PlayerGamePos {
            pg: PlayerGame {
                id: integer(p, "id"),
                game_id,
                player_id: integer(p, "player_id"),
                team_id: integer(p, "team_id"),
                on: integer(p, "on"),
                off: integer(p, "off"),
                red: p["red"].as_bool().unwrap(),
            },
            pos: p["pos"].as_str().unwrap().to_string(),
        });
    }
    Fixture {
        games,
        goals,
        ratings,
        players,
        now: v["now"].as_i64().unwrap(),
    }
}
pub fn output(result: &player_ratings::Computed, now: i64) -> Value {
    let mut players: Vec<_> = result
        .player_ratings
        .iter()
        .map(|(id, r)| (*id, r.off.to_bits(), r.def.to_bits(), r.minutes.to_bits()))
        .collect();
    players.sort_by_key(|r| r.0);
    let mut appearances: Vec<_> = result
        .player_game_ratings
        .iter()
        .map(|((id, game), r)| {
            (
                *id,
                *game,
                r.off.to_bits(),
                r.def.to_bits(),
                r.minutes.to_bits(),
            )
        })
        .collect();
    appearances.sort_by_key(|r| r.0);
    json!({"now":now,"players":players,"appearances":appearances})
}
