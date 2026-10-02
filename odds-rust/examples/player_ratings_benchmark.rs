//! Benchmark real inputs while writing only connection-local temporary tables.
//! Usage: player_ratings_benchmark OUTPUT.json [NOW_UNIX] [ITERATIONS] [stored|empty|unchanged]
use chrono::{DateTime, Utc};
use golaberto_odds::{database, player_ratings};
use mysql::{prelude::Queryable, Params, Value};
use serde_json::json;
use std::{env, fs, time::Instant};

fn main() -> Result<(), database::Error> {
    let args: Vec<_> = env::args().collect();
    let output = args.get(1).ok_or(
        "usage: player_ratings_benchmark OUTPUT.json [NOW_UNIX] [ITERATIONS] [stored|empty|unchanged]",
    )?;
    let now = match args.get(2) {
        Some(s) => DateTime::from_timestamp(s.parse()?, 0).ok_or("invalid timestamp")?,
        None => Utc::now(),
    };
    let iterations: usize = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(3);
    if iterations == 0 {
        return Err("iterations must be positive".into());
    }
    let mode = args.get(4).map(String::as_str).unwrap_or("stored");
    if !["stored", "empty", "unchanged"].contains(&mode) {
        return Err("mode must be stored, empty or unchanged".into());
    }
    let mut conn = database::connect()?;
    let mut observations = Vec::new();
    for iteration in 0..iterations {
        // Previous shadow tables must be removed before loading real input.
        conn.query_drop("DROP TEMPORARY TABLE IF EXISTS players, player_games")?;
        let start = Instant::now();
        let input = player_ratings::load_parallel(&mut conn, now.naive_utc())?;
        let load_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let computed = ::player_ratings::calculate(
            &input.games,
            &input.goals,
            input.ratings,
            &input.players,
            now.timestamp(),
        )?;
        let calculate_ms = start.elapsed().as_secs_f64() * 1000.;
        // These names shadow the application tables on this connection only.
        // Do not call persist unless both CREATE statements have succeeded.
        conn.query_drop("CREATE TEMPORARY TABLE players(id INT PRIMARY KEY, off_rating FLOAT, def_rating FLOAT, rating FLOAT, updated_at DATETIME) ENGINE=InnoDB")?;
        conn.query_drop("CREATE TEMPORARY TABLE player_games(id INT PRIMARY KEY, off_rating FLOAT, def_rating FLOAT) ENGINE=InnoDB")?;
        let mut player_ids: Vec<_> = computed.player_ratings.keys().copied().collect();
        player_ids.sort_unstable();
        let mut appearance_ids: Vec<_> = computed
            .player_game_ratings
            .keys()
            .map(|(id, _)| *id)
            .collect();
        appearance_ids.sort_unstable();
        for batch in player_ids.chunks(5000) {
            conn.exec_drop(
                format!(
                    "INSERT INTO players(id) VALUES {}",
                    vec!["(?)"; batch.len()].join(",")
                ),
                Params::Positional(batch.iter().map(|id| Value::from(*id)).collect()),
            )?;
        }
        let existing = match mode {
            "stored" => input.appearance_ratings,
            "unchanged" => computed
                .player_game_ratings
                .iter()
                .map(|((id, _), r)| (*id, (Some(r.off), Some(r.def))))
                .collect(),
            _ => std::collections::HashMap::new(),
        };
        for batch in appearance_ids.chunks(5000) {
            let values: Vec<_> = batch
                .iter()
                .flat_map(|id| {
                    let (off, def) = existing.get(id).copied().unwrap_or((None, None));
                    [Value::from(*id), Value::from(off), Value::from(def)]
                })
                .collect();
            conn.exec_drop(
                format!(
                    "INSERT INTO player_games VALUES {}",
                    vec!["(?,?,?)"; batch.len()].join(",")
                ),
                Params::Positional(values),
            )?;
        }
        let mut pending = ::player_ratings::Computed {
            player_ratings: computed.player_ratings.clone(),
            player_game_ratings: computed.player_game_ratings.clone(),
        };
        let start = Instant::now();
        player_ratings::retain_changed_appearances(&mut pending, &existing);
        player_ratings::persist(&mut conn, &pending, now.naive_utc())?;
        let persist_ms = start.elapsed().as_secs_f64() * 1000.;
        // Compare every stored float with the calculation, including normalization.
        let stored: Vec<(i32, f32, f32, f32)> =
            conn.exec("SELECT id,off_rating,def_rating,rating FROM players", ())?;
        assert_eq!(stored.len(), computed.player_ratings.len());
        for (id, off, def, rating) in stored {
            let expected = &computed.player_ratings[&id];
            assert_eq!(
                off.to_bits(),
                (expected.off / expected.minutes * 90.).to_bits()
            );
            assert_eq!(
                def.to_bits(),
                (expected.def / expected.minutes * 90.).to_bits()
            );
            assert_eq!(
                rating.to_bits(),
                ((expected.off + expected.def) / expected.minutes
                    * 90.
                    * ::player_ratings::squash_rating(expected.minutes))
                .to_bits()
            );
        }
        let stored: Vec<(i32, f32, f32)> =
            conn.exec("SELECT id,off_rating,def_rating FROM player_games", ())?;
        assert_eq!(stored.len(), computed.player_game_ratings.len());
        let by_id: std::collections::HashMap<_, _> = computed
            .player_game_ratings
            .iter()
            .map(|((id, _), rating)| (*id, rating))
            .collect();
        for (id, off, def) in stored {
            assert_eq!(off.to_bits(), by_id[&id].off.to_bits());
            assert_eq!(def.to_bits(), by_id[&id].def.to_bits());
        }
        let observation = json!({"iteration": iteration, "games": input.games.len(), "players": player_ids.len(), "appearances": appearance_ids.len(), "load_ms": load_ms, "calculate_ms": calculate_ms, "persist_ms": persist_ms, "total_ms": load_ms + calculate_ms + persist_ms, "stored_float_bits_match": true, "written_appearances": pending.player_game_ratings.len()});
        eprintln!("{observation}");
        observations.push(observation);
    }
    fs::write(
        output,
        serde_json::to_vec_pretty(
            &json!({"now": now.timestamp(), "mode": mode, "application_database_writes": 0, "destination": "connection-local temporary InnoDB tables", "observations": observations}),
        )?,
    )?;
    Ok(())
}
