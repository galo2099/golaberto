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
}
fn placeholders(count: usize) -> String {
    vec!["?"; count].join(",")
}
/// Read the same category, time windows, played games and appearance filters as stats.
pub fn load(conn: &mut Conn, now: NaiveDateTime) -> Result<Input, database::Error> {
    let cutoff = (now - Duration::weeks(4 * 52))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    type GameRow = (i32, i32, i32, String, i32, Option<i32>);
    let rows: Vec<GameRow> = conn.exec("SELECT g.id,g.home_id,g.away_id,DATE_FORMAT(g.date,'%Y-%m-%d %H:%i:%s'),g.home_field,g.home_aet FROM championships c INNER JOIN phases p ON p.championship_id=c.id INNER JOIN games g ON g.phase_id=p.id WHERE c.category_id=1 AND g.date>? AND g.played=1 ORDER BY g.date", (cutoff,))?;
    let games: Vec<Game> = rows
        .into_iter()
        .map(|(id, home_id, away_id, date, home_field, home_aet)| {
            Ok(Game {
                id,
                home_id,
                away_id,
                date: NaiveDateTime::parse_from_str(&date, "%Y-%m-%d %H:%M:%S")?,
                home_field,
                home_aet,
            })
        })
        .collect::<Result<_, database::Error>>()?;
    let ids: Vec<_> = games.iter().map(|g| g.id).collect();
    let mut goals: HashMap<i32, smallvec::SmallVec<[Goal; 4]>> = HashMap::new();
    let mut players: HashMap<i32, Vec<PlayerGamePos>> = HashMap::new();
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
        type AppearanceRow = (i32, i32, i32, i32, i32, i32, bool, Option<String>);
        let rows: Vec<AppearanceRow> = conn.exec(format!("SELECT pg.id,pg.game_id,pg.player_id,pg.team_id,pg.`on`,pg.`off`,pg.red,p.position FROM player_games pg INNER JOIN players p ON p.id=pg.player_id WHERE pg.game_id IN ({}) AND pg.`off`>0", placeholders(batch.len())), Params::Positional(batch.iter().map(|id| Value::from(*id)).collect()))?;
        for (id, game_id, player_id, team_id, on, off, red, position) in rows {
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
                pos: position.unwrap_or_default(),
            });
        }
    }
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
        type RatingRow = (i32, String, f32, f32);
        let rows: Vec<RatingRow> = conn.exec(format!("SELECT team_id,DATE_FORMAT(measure_date,'%Y-%m-%d'),off_rating,def_rating FROM historical_ratings WHERE team_id IN ({}) AND measure_date>? ORDER BY team_id,measure_date", placeholders(batch.len())), Params::Positional(values))?;
        for (team_id, date, off_rating, def_rating) in rows {
            ratings
                .entry(team_id)
                .or_default()
                .push_back(HistoricalRating {
                    team_id,
                    measure_date: NaiveDate::parse_from_str(&date, "%Y-%m-%d")?,
                    off_rating,
                    def_rating,
                });
        }
    }
    Ok(Input {
        games,
        goals,
        ratings,
        players,
    })
}

/// Update existing records in bounded batches, committing players and appearances
/// together. Existing names/identities remain intact; no placeholder rows are inserted.
pub fn persist(
    conn: &mut Conn,
    computed: &Computed,
    now: NaiveDateTime,
) -> Result<(), database::Error> {
    let mut players: Vec<_> = computed.player_ratings.iter().collect();
    players.sort_by_key(|(id, _)| **id);
    let mut appearances: Vec<_> = computed.player_game_ratings.iter().collect();
    appearances.sort_by_key(|((id, _), _)| *id);
    let mut tx = conn.start_transaction(TxOpts::default())?;
    for batch in players.chunks(1000) {
        let mut values = Vec::new();
        let mut columns = Vec::new();
        for field in 0..3 {
            let mut cases = Vec::new();
            for &(id, rating) in batch {
                let value = match field {
                    0 => rating.off / rating.minutes * 90.,
                    1 => rating.def / rating.minutes * 90.,
                    _ => {
                        (rating.off + rating.def) / rating.minutes
                            * 90.
                            * player_ratings::squash_rating(rating.minutes)
                    }
                };
                if !value.is_finite() {
                    return Err("non-finite normalized player rating".into());
                }
                cases.push("WHEN ? THEN ?");
                values.extend([Value::from(*id), Value::from(value)]);
            }
            columns.push(format!(
                "{}=CASE id {} END",
                ["off_rating", "def_rating", "rating"][field],
                cases.join(" ")
            ));
        }
        values.push(Value::from(now.format("%Y-%m-%d %H:%M:%S").to_string()));
        values.extend(batch.iter().map(|(id, _)| Value::from(**id)));
        tx.exec_drop(
            format!(
                "UPDATE players SET {},updated_at=? WHERE id IN ({})",
                columns.join(","),
                placeholders(batch.len())
            ),
            Params::Positional(values),
        )?;
    }
    for batch in appearances.chunks(5000) {
        let mut values = Vec::new();
        let mut columns = Vec::new();
        for field in 0..2 {
            let mut cases = Vec::new();
            for &((id, _), rating) in batch {
                let value = if field == 0 { rating.off } else { rating.def };
                if !value.is_finite() {
                    return Err("non-finite player appearance rating".into());
                }
                cases.push("WHEN ? THEN ?");
                values.extend([Value::from(*id), Value::from(value)]);
            }
            columns.push(format!(
                "{}=CASE id {} END",
                ["off_rating", "def_rating"][field],
                cases.join(" ")
            ));
        }
        values.extend(batch.iter().map(|((id, _), _)| Value::from(*id)));
        tx.exec_drop(
            format!(
                "UPDATE player_games SET {} WHERE id IN ({})",
                columns.join(","),
                placeholders(batch.len())
            ),
            Params::Positional(values),
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn run(log: &RequestLog) -> Result<(), database::Error> {
    let now = Utc::now().naive_utc();
    let start = Instant::now();
    let mut conn = database::connect()?;
    let input = load(&mut conn, now)?;
    log.stage(
        "players.load",
        start,
        serde_json::json!({"games":input.games.len(),"appearance_games":input.players.len()}),
    );
    let start = Instant::now();
    let computed = player_ratings::calculate(
        &input.games,
        &input.goals,
        input.ratings,
        &input.players,
        now.and_utc().timestamp(),
    )?;
    log.stage("players.calculate", start, serde_json::json!({"players":computed.player_ratings.len(),"appearances":computed.player_game_ratings.len()}));
    let start = Instant::now();
    persist(&mut conn, &computed, now)?;
    log.stage("players.persist", start, serde_json::json!({"players":computed.player_ratings.len(),"appearances":computed.player_game_ratings.len()}));
    Ok(())
}
