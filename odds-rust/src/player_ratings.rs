//! DB adapter for the shared stats player-rating calculation.
use crate::{database, logging::RequestLog};
use chrono::{Duration, NaiveDate, NaiveDateTime, Utc};
use mysql::{prelude::Queryable, Conn, Params, TxOpts, Value};
use player_ratings::{Computed, Game, Goal, HistoricalRating, PlayerGame, PlayerGamePos};
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::time::Instant;

pub struct Input {
    pub games: Vec<Game>,
    pub goals: HashMap<i32, smallvec::SmallVec<[Goal; 4]>>,
    pub ratings: HashMap<i32, VecDeque<HistoricalRating>>,
    pub players: HashMap<i32, Vec<PlayerGamePos>>,
    pub appearance_ratings: HashMap<i32, (Option<f32>, Option<f32>)>,
}
fn placeholders(count: usize) -> String {
    vec!["?"; count].join(",")
}
fn load_games(conn: &mut Conn, now: NaiveDateTime) -> Result<Vec<Game>, database::Error> {
    let cutoff = (now - Duration::weeks(4 * 52))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    type GameRow = (i32, i32, i32, NaiveDateTime, i32, Option<i32>);
    let rows: Vec<GameRow> = conn.exec("SELECT g.id,g.home_id,g.away_id,g.date,g.home_field,g.home_aet FROM championships c INNER JOIN phases p ON p.championship_id=c.id INNER JOIN games g ON g.phase_id=p.id WHERE c.category_id=1 AND g.date>? AND g.played=1 ORDER BY g.date", (cutoff,))?;
    Ok(rows
        .into_iter()
        .map(|(id, home_id, away_id, date, home_field, home_aet)| Game {
            id,
            home_id,
            away_id,
            date,
            home_field,
            home_aet,
        })
        .collect())
}
fn load_goals(
    conn: &mut Conn,
    ids: &[i32],
) -> Result<HashMap<i32, smallvec::SmallVec<[Goal; 4]>>, database::Error> {
    let mut goals: HashMap<i32, smallvec::SmallVec<[Goal; 4]>> = HashMap::new();
    for batch in ids.chunks(1000) {
        type GoalRow = (i32, i32, i32, i32, bool, bool);
        let rows: Vec<GoalRow> = conn.exec(format!("SELECT game_id,player_id,team_id,time,penalty,own_goal FROM goals WHERE game_id IN ({})", placeholders(batch.len())), Params::Positional(batch.iter().map(|id| Value::from(*id)).collect()))?;
        for (game_id, player_id, team_id, time, penalty, own_goal) in rows {
            goals.entry(game_id).or_default().push(Goal {
                player_id,
                team_id,
                time,
                penalty,
                own_goal,
            });
        }
    }
    Ok(goals)
}
type AppearanceRatings = HashMap<i32, (Option<f32>, Option<f32>)>;
fn load_players(
    conn: &mut Conn,
    ids: &[i32],
) -> Result<(HashMap<i32, Vec<PlayerGamePos>>, AppearanceRatings), database::Error> {
    let mut players: HashMap<i32, Vec<PlayerGamePos>> = HashMap::new();
    let mut appearance_ratings = HashMap::new();
    if ids.is_empty() {
        return Ok((players, appearance_ratings));
    }
    // Fetch each position once rather than joining and decoding it for every
    // historical appearance. Missing players still have inner-join semantics.
    let positions: HashMap<i32, Option<String>> = conn
        .query::<(i32, Option<String>), _>("SELECT id,position FROM players")?
        .into_iter()
        .collect();
    for batch in ids.chunks(1000) {
        type AppearanceRow = (i32, i32, i32, i32, i32, i32, bool, Option<f32>, Option<f32>);
        let rows: Vec<AppearanceRow> = conn.exec(format!("SELECT id,game_id,player_id,team_id,`on`,`off`,red,off_rating,def_rating FROM player_games WHERE game_id IN ({}) AND `off`>0", placeholders(batch.len())), Params::Positional(batch.iter().map(|id| Value::from(*id)).collect()))?;
        for (id, game_id, player_id, team_id, on, off, red, off_rating, def_rating) in rows {
            let Some(position) = positions.get(&player_id) else {
                continue;
            };
            appearance_ratings.insert(id, (off_rating, def_rating));
            players.entry(game_id).or_default().push(PlayerGamePos {
                pg: PlayerGame {
                    id,
                    game_id,
                    player_id,
                    team_id,
                    on,
                    off,
                    red,
                },
                pos: position.clone().unwrap_or_default(),
            });
        }
    }
    Ok((players, appearance_ratings))
}
fn load_ratings(
    conn: &mut Conn,
    games: &[Game],
    now: NaiveDateTime,
) -> Result<HashMap<i32, VecDeque<HistoricalRating>>, database::Error> {
    let teams: Vec<_> = games
        .iter()
        .flat_map(|g| [g.home_id, g.away_id])
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let cutoff = (now - Duration::weeks(4 * 52 + 1))
        .date()
        .format("%Y-%m-%d")
        .to_string();
    let mut ratings: HashMap<i32, VecDeque<HistoricalRating>> = HashMap::new();
    for batch in teams.chunks(1000) {
        let mut values: Vec<Value> = batch.iter().map(|id| Value::from(*id)).collect();
        values.push(Value::from(cutoff.clone()));
        type RatingRow = (i32, NaiveDate, f32, f32);
        let rows: Vec<RatingRow> = conn.exec(format!("SELECT team_id,measure_date,off_rating,def_rating FROM historical_ratings WHERE team_id IN ({}) AND measure_date>? ORDER BY team_id,measure_date", placeholders(batch.len())), Params::Positional(values))?;
        for (team_id, measure_date, off_rating, def_rating) in rows {
            ratings
                .entry(team_id)
                .or_default()
                .push_back(HistoricalRating {
                    team_id,
                    measure_date,
                    off_rating,
                    def_rating,
                });
        }
    }
    Ok(ratings)
}
/// Serial reader for connection-local temporary fixtures and read-only comparisons.
/// Preserve category, time windows, chronological order and appearance filters.
pub fn load(conn: &mut Conn, now: NaiveDateTime) -> Result<Input, database::Error> {
    let games = load_games(conn, now)?;
    let ids: Vec<_> = games.iter().map(|g| g.id).collect();
    let goals = load_goals(conn, &ids)?;
    let (players, appearance_ratings) = load_players(conn, &ids)?;
    let ratings = load_ratings(conn, &games, now)?;
    Ok(Input {
        games,
        goals,
        ratings,
        players,
        appearance_ratings,
    })
}
/// Use at most four DB connections while independent input reads overlap.
/// The caller's connection remains the sole writer for the atomic update.
pub fn load_parallel(conn: &mut Conn, now: NaiveDateTime) -> Result<Input, database::Error> {
    let games = load_games(conn, now)?;
    let ids: Vec<_> = games.iter().map(|g| g.id).collect();
    let (goals, (players, appearance_ratings), ratings) =
        std::thread::scope(|scope| -> Result<_, database::Error> {
            let goals = scope.spawn(|| load_goals(&mut database::connect()?, &ids));
            let players = scope.spawn(|| load_players(&mut database::connect()?, &ids));
            let ratings = scope.spawn(|| load_ratings(&mut database::connect()?, &games, now));
            Ok((
                goals.join().map_err(|_| "player goals reader panicked")??,
                players
                    .join()
                    .map_err(|_| "player appearances reader panicked")??,
                ratings
                    .join()
                    .map_err(|_| "player history reader panicked")??,
            ))
        })?;
    Ok(Input {
        games,
        goals,
        ratings,
        players,
        appearance_ratings,
    })
}

/// Compare only after recalculating with current inputs and the current clock.
/// Avoid rewriting identical appearance bits; always retain date-weighted player
/// totals. NULLs and edited input data still cause appearance updates.
pub fn retain_changed_appearances(computed: &mut Computed, existing: &AppearanceRatings) {
    computed.player_game_ratings.retain(|(id, _), rating| {
        !matches!(existing.get(id), Some((Some(off), Some(def)))
            if off.to_bits() == rating.off.to_bits() && def.to_bits() == rating.def.to_bits())
    });
}

/// Update existing records through a bounded, primary-keyed staging table.
/// Both target tables still commit together; missing identities are never inserted.
pub fn persist(
    conn: &mut Conn,
    computed: &Computed,
    now: NaiveDateTime,
) -> Result<(), database::Error> {
    if computed.player_ratings.is_empty() && computed.player_game_ratings.is_empty() {
        return Ok(());
    }
    let mut players: Vec<_> = computed.player_ratings.iter().collect();
    players.sort_by_key(|(id, _)| **id);
    let mut appearances: Vec<_> = computed.player_game_ratings.iter().collect();
    appearances.sort_by_key(|((id, _), _)| *id);
    // One connection-local table is reused for each batch. FLOAT preserves the
    // calculation's f32 values, while the key makes each target lookup constant
    // work instead of evaluating thousands of CASE branches per updated row.
    conn.query_drop("CREATE TEMPORARY TABLE golaberto_player_rating_updates(id INT PRIMARY KEY,off_rating FLOAT NOT NULL,def_rating FLOAT NOT NULL,rating FLOAT NOT NULL) ENGINE=MEMORY")?;
    let result = (|| -> Result<(), database::Error> {
        let mut tx = conn.start_transaction(TxOpts::default())?;
        let updated_at = now.format("%Y-%m-%d %H:%M:%S").to_string();
        for batch in players.chunks(1000) {
            let mut rows = Vec::with_capacity(batch.len());
            for &(id, raw) in batch {
                let off = raw.off / raw.minutes * 90.;
                let def = raw.def / raw.minutes * 90.;
                let rating = (raw.off + raw.def) / raw.minutes
                    * 90.
                    * player_ratings::squash_rating(raw.minutes);
                if !off.is_finite() || !def.is_finite() || !rating.is_finite() {
                    return Err("non-finite normalized player rating".into());
                }
                // Only typed IDs and validated finite floats enter this SQL.
                // Scientific notation round-trips f32, including tiny values.
                rows.push(format!("({id},{off:e},{def:e},{rating:e})"));
            }
            tx.query_drop(format!(
                "INSERT INTO golaberto_player_rating_updates VALUES {}",
                rows.join(",")
            ))?;
            // Start with the bounded batch even when the target table is large.
            tx.exec_drop("UPDATE golaberto_player_rating_updates r STRAIGHT_JOIN players p ON p.id=r.id SET p.off_rating=r.off_rating,p.def_rating=r.def_rating,p.rating=r.rating,p.updated_at=?", (&updated_at,))?;
            // TRUNCATE would implicitly commit the target updates in MySQL.
            tx.query_drop("DELETE FROM golaberto_player_rating_updates")?;
        }
        for batch in appearances.chunks(5000) {
            let mut rows = Vec::with_capacity(batch.len());
            for &((id, _), rating) in batch {
                if !rating.off.is_finite() || !rating.def.is_finite() {
                    return Err("non-finite player appearance rating".into());
                }
                rows.push(format!("({id},{:e},{:e},0)", rating.off, rating.def));
            }
            tx.query_drop(format!(
                "INSERT INTO golaberto_player_rating_updates VALUES {}",
                rows.join(",")
            ))?;
            tx.query_drop("UPDATE golaberto_player_rating_updates r STRAIGHT_JOIN player_games p ON p.id=r.id SET p.off_rating=r.off_rating,p.def_rating=r.def_rating")?;
            tx.query_drop("DELETE FROM golaberto_player_rating_updates")?;
        }
        tx.commit()?;
        Ok(())
    })();
    // Transaction drop rolls back failures before cleanup. Drop only our own
    // temporary table, including on failure, so the connection can be reused.
    let cleanup = conn.query_drop("DROP TEMPORARY TABLE golaberto_player_rating_updates");
    result?;
    cleanup?;
    Ok(())
}

pub fn run(log: &RequestLog) -> Result<(), database::Error> {
    let now = Utc::now().naive_utc();
    let start = Instant::now();
    let mut conn = database::connect()?;
    let input = load_parallel(&mut conn, now)?;
    log.stage(
        "players.load",
        start,
        serde_json::json!({"games":input.games.len(),"appearance_games":input.players.len()}),
    );
    let start = Instant::now();
    let mut computed = player_ratings::calculate(
        &input.games,
        &input.goals,
        input.ratings,
        &input.players,
        now.and_utc().timestamp(),
    )?;
    log.stage("players.calculate", start, serde_json::json!({"players":computed.player_ratings.len(),"appearances":computed.player_game_ratings.len()}));
    let start = Instant::now();
    retain_changed_appearances(&mut computed, &input.appearance_ratings);
    persist(&mut conn, &computed, now)?;
    log.stage("players.persist", start, serde_json::json!({"players":computed.player_ratings.len(),"appearances":computed.player_game_ratings.len()}));
    Ok(())
}
