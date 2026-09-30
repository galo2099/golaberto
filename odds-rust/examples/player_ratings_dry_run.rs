//! Read application data and compute ratings without any persistent writes.
use chrono::{DateTime, Utc};
use golaberto_odds::{database, player_ratings::load};
use serde_json::json;
use std::{env, fs, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<_> = env::args().collect();
    let output = args
        .get(1)
        .ok_or("usage: player_ratings_dry_run OUTPUT.json [NOW_UNIX] [INPUT_EXPORT.json]")?;
    let now = match args.get(2) {
        Some(s) => DateTime::from_timestamp(s.parse()?, 0).ok_or("invalid timestamp")?,
        None => Utc::now(),
    };
    let start = Instant::now();
    let mut conn = database::connect()?;
    let input = load(&mut conn, now.naive_utc())?;
    let load_ms = start.elapsed().as_secs_f64() * 1000.;
    if let Some(path) = args.get(3) {
        fs::write(
            path,
            serde_json::to_vec(&json!({
                "now":now.timestamp(),
                "games":input.games.iter().map(|g| json!({"id":g.id,"home_id":g.home_id,"away_id":g.away_id,"date":g.date.to_string(),"home_field":g.home_field,"home_aet":g.home_aet})).collect::<Vec<_>>(),
                "goals":input.goals.iter().flat_map(|(id, goals)| goals.iter().map(move |g| json!({"game_id":id,"player_id":g.player_id,"team_id":g.team_id,"time":g.time,"penalty":g.penalty,"own_goal":g.own_goal}))).collect::<Vec<_>>(),
                "ratings":input.ratings.values().flat_map(|history| history.iter().map(|r| json!({"team_id":r.team_id,"measure_date":r.measure_date.to_string(),"off_rating":r.off_rating,"def_rating":r.def_rating}))).collect::<Vec<_>>(),
                "appearances":input.players.values().flat_map(|entries| entries.iter().map(|e| json!({"id":e.pg.id,"game_id":e.pg.game_id,"player_id":e.pg.player_id,"team_id":e.pg.team_id,"on":e.pg.on,"off":e.pg.off,"red":e.pg.red,"pos":e.pos}))).collect::<Vec<_>>()
            }))?,
        )?;
    }
    let start = Instant::now();
    let result = player_ratings::calculate(
        &input.games,
        &input.goals,
        input.ratings,
        &input.players,
        now.timestamp(),
    )?;
    let calculate_ms = start.elapsed().as_secs_f64() * 1000.;
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
    fs::write(
        output,
        serde_json::to_vec(
            &json!({"now":now.timestamp(),"players":players,"appearances":appearances}),
        )?,
    )?;
    eprintln!(
        "{}",
        json!({"games":input.games.len(),"players":players.len(),"appearances":appearances.len(),"load_ms":load_ms,"calculate_ms":calculate_ms,"writes":0})
    );
    Ok(())
}
