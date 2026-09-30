use crate::models::{Game, Goal, HistoricalRating, PlayerGame};
use crate::schema::games;
use crate::schema::phases;
use crate::schema::{championships, goals, players};
use chrono::Duration;
use diesel::connection::DefaultLoadingMode;
use diesel::dsl::sql;
use diesel::mysql::MysqlConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error as DieselError;
use diesel::sql_types::Bool;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::{sql_query, ExpressionMethods, JoinOnDsl, SelectableHelper};
use dotenv::dotenv;
use itertools::Itertools;
#[cfg(test)]
use player_ratings::red_card_penalty_remaining_per90;
use player_ratings::squash_rating;
use smallvec::{smallvec, SmallVec};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};
use std::{env, thread};
use tiny_http::{Header, Method, Response, Server};

pub mod models;
pub mod schema;

const WRITE_MAX_RETRIES: usize = 2;
const WRITE_RETRY_SLEEP: StdDuration = StdDuration::from_secs(1);
const PLAYER_UPSERT_BATCH_SIZE: usize = 1_000;
const PLAYER_GAME_UPSERT_BATCH_SIZE: usize = 5_000;
const MAX_UPSERT_WORKERS: usize = 4;

struct PlayerGamePos {
    pg: PlayerGame,
    pos: String,
}

fn establish_connection() -> Pool<ConnectionManager<MysqlConnection>> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://localhost/GolAberto_production".to_string());
    let manager = ConnectionManager::<MysqlConnection>::new(database_url);
    Pool::builder()
        .connection_timeout(std::time::Duration::from_secs(300))
        .build(manager)
        .expect("Could not build connection pool")
}

fn load_games(conn: &mut MysqlConnection) -> Vec<Game> {
    let start = chrono::Utc::now() - Duration::weeks(4 * 52);
    championships::table
        .inner_join(phases::table.on(phases::dsl::championship_id.eq(championships::dsl::id)))
        .inner_join(games::table.on(phases::dsl::id.eq(games::dsl::phase_id)))
        .select(Game::as_select())
        .filter(championships::category_id.eq(1))
        .filter(games::date.gt(start.naive_utc()))
        .filter(games::played.eq(true))
        .order(games::date)
        .load::<Game>(conn)
        .expect("champ")
}

fn load_goals(conn: &mut MysqlConnection, games: &[Game]) -> HashMap<i32, SmallVec<[Goal; 4]>> {
    let s = Instant::now();
    let x1 = goals::table
        .select(Goal::as_select())
        .filter(sql::<Bool>(&format!(
            "game_id in ({})",
            games
                .iter()
                .map(|g| g.id.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )))
        .load_iter::<Goal, DefaultLoadingMode>(conn)
        .expect("goal")
        .fold(HashMap::<i32, SmallVec<[_; 4]>>::new(), |mut h, x| {
            let x = x.unwrap();
            match h.get_mut(&x.game_id.unwrap()) {
                Some(v) => v.push(x),
                None => {
                    h.insert(x.game_id.unwrap(), smallvec![x]);
                }
            };
            h
        });
    println!("Loaded goals grouped by game in {:?}", s.elapsed());
    x1
}

fn dedup<T: Copy + Eq + std::hash::Hash>(iter: impl Iterator<Item = T>) -> impl Iterator<Item = T> {
    let mut seen = HashSet::new();
    iter.filter(move |&x| seen.insert(x))
}

fn load_ratings(
    conn: &mut MysqlConnection,
    games: &[Game],
) -> HashMap<i32, VecDeque<HistoricalRating>> {
    use self::schema::historical_ratings::dsl::*;
    let s = Instant::now();
    let start = chrono::Utc::now() - Duration::weeks(4 * 52 + 1);
    let x1 = historical_ratings
        .select(HistoricalRating::as_select())
        .filter(measure_date.gt(start.date_naive()))
        .filter(sql::<Bool>(&format!(
            "team_id in ({})",
            dedup(games.iter().flat_map(|g| [g.home_id, g.away_id]))
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )))
        .order(team_id)
        .then_order_by(measure_date)
        .load_iter::<HistoricalRating, DefaultLoadingMode>(conn)
        .expect("ratings")
        .fold(
            HashMap::<i32, VecDeque<HistoricalRating>>::new(),
            |mut h, x| {
                let x = x.unwrap();
                match h.get_mut(&x.team_id) {
                    Some(v) => v.push_back(x),
                    None => {
                        h.insert(x.team_id, VecDeque::from([x]));
                    }
                }
                h
            },
        );
    println!("Loaded historical team ratings in {:?}", s.elapsed());
    x1
}

fn load_players(conn: &mut MysqlConnection, games: &[Game]) -> HashMap<i32, Vec<PlayerGamePos>> {
    use self::schema::player_games::dsl::*;
    let s = Instant::now();
    let x2 = player_games
        .inner_join(players::table.on(players::dsl::id.eq(player_id)))
        .select((PlayerGame::as_select(), players::dsl::position))
        .filter(sql::<Bool>(&format!(
            "game_id in ({})",
            games
                .iter()
                .map(|g| g.id.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )))
        .filter(off.gt(0));
    // println!("{}", debug_query::<Mysql, _>(&x2));
    let x1 = x2
        .load_iter::<(PlayerGame, Option<String>), DefaultLoadingMode>(conn)
        .expect("ratings")
        .fold(HashMap::<i32, Vec<PlayerGamePos>>::new(), |mut h, x| {
            let x = x.unwrap();
            match h.get_mut(&x.0.game_id) {
                Some(v) => v.push(PlayerGamePos {
                    pg: x.0,
                    pos: x.1.unwrap_or_default(),
                }),
                None => {
                    h.insert(
                        x.0.game_id,
                        vec![PlayerGamePos {
                            pg: x.0,
                            pos: x.1.unwrap_or_default(),
                        }],
                    );
                }
            }
            h
        });
    println!(
        "Loaded player game records (with positions) in {:?}",
        s.elapsed()
    );
    x1
}

fn compute_ratings(pool: &Pool<ConnectionManager<MysqlConnection>>) {
    let start = Instant::now();
    let games = Arc::new(load_games(&mut pool.get().unwrap()));
    println!("Loaded games from DB in {:?}", start.elapsed());

    let (goals, ratings, players) = thread::scope(|s| {
        let g1 = games.clone();
        let p1 = pool.clone();
        let goals = s.spawn(move || load_goals(&mut p1.get().unwrap(), &g1));

        let g1 = games.clone();
        let p1 = pool.clone();
        let ratings = s.spawn(move || load_ratings(&mut p1.get().unwrap(), &g1));

        let g1 = games.clone();
        let p1 = pool.clone();
        let players = s.spawn(move || load_players(&mut p1.get().unwrap(), &g1));

        (
            goals.join().unwrap(),
            ratings.join().unwrap(),
            players.join().unwrap(),
        )
    });

    println!(
        "Loaded goals, ratings, and player records in parallel in {:?}",
        start.elapsed()
    );
    println!("Loaded {} games for player rating computation", games.len());
    println!("Loaded goal maps for {} games", goals.len());
    println!("Loaded rating histories for {} teams", ratings.len());
    println!("Loaded player entries for {} games", players.len());

    let core_games: Vec<_> = games
        .iter()
        .map(|g| player_ratings::Game {
            id: g.id,
            home_id: g.home_id,
            away_id: g.away_id,
            date: g.date,
            home_field: g.home_field,
            home_aet: g.home_aet,
        })
        .collect();
    let core_goals = goals
        .into_iter()
        .map(|(id, entries)| {
            (
                id,
                entries
                    .into_iter()
                    .map(|g| player_ratings::Goal {
                        player_id: g.player_id,
                        team_id: g.team_id,
                        time: g.time,
                        penalty: g.penalty,
                        own_goal: g.own_goal,
                    })
                    .collect(),
            )
        })
        .collect();
    let core_ratings = ratings
        .into_iter()
        .map(|(id, entries)| {
            (
                id,
                entries
                    .into_iter()
                    .map(|r| player_ratings::HistoricalRating {
                        team_id: r.team_id,
                        measure_date: r.measure_date,
                        off_rating: r.off_rating,
                        def_rating: r.def_rating,
                    })
                    .collect(),
            )
        })
        .collect();
    let core_players = players
        .into_iter()
        .map(|(id, entries)| {
            (
                id,
                entries
                    .into_iter()
                    .map(|entry| player_ratings::PlayerGamePos {
                        pg: player_ratings::PlayerGame {
                            id: entry.pg.id,
                            game_id: entry.pg.game_id,
                            player_id: entry.pg.player_id,
                            team_id: entry.pg.team_id,
                            on: entry.pg.on,
                            off: entry.pg.off,
                            red: entry.pg.red,
                        },
                        pos: entry.pos,
                    })
                    .collect(),
            )
        })
        .collect();
    let computed = player_ratings::calculate(
        &core_games,
        &core_goals,
        core_ratings,
        &core_players,
        chrono::Utc::now().timestamp(),
    )
    .expect("player-rating calculation failed");
    let player_ratings = computed.player_ratings;
    let player_game_ratings = computed.player_game_ratings;

    println!(
        "Computed player and player-game ratings in {:?}",
        start.elapsed()
    );

    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
    let player_upsert_jobs = player_ratings
        .iter()
        .chunks(PLAYER_UPSERT_BATCH_SIZE)
        .into_iter()
        .map(|c| {
            "INSERT INTO players (id,off_rating,def_rating,rating,created_at,updated_at) VALUES "
                .to_owned()
                + &c.map(|(k, v)| {
                    format!(
                        "({}, {}, {}, {}, \"{}\", \"{}\")",
                        k,
                        v.off / v.minutes * 90.0,
                        v.def / v.minutes * 90.0,
                        (v.off + v.def) / v.minutes * 90.0 * squash_rating(v.minutes),
                        now,
                        now
                    )
                })
                .collect::<Vec<String>>()
                .join(",")
                + " ON DUPLICATE KEY UPDATE off_rating=VALUES(off_rating),def_rating=VALUES(def_rating),rating=VALUES(rating),updated_at=VALUES(updated_at)"
        })
        .collect::<Vec<_>>();

    println!(
        "Prepared {} player_game rating rows for upsert",
        player_game_ratings.len()
    );
    let player_game_upsert_jobs = player_game_ratings
        .iter()
        .chunks(PLAYER_GAME_UPSERT_BATCH_SIZE)
        .into_iter()
        .map(|c| {
            "INSERT INTO player_games (id, game_id, off_rating, def_rating) VALUES ".to_owned()
                + &c.map(|((id, game_id), v)| format!("({}, {}, {}, {})", id, game_id, v.off, v.def))
                    .collect::<Vec<String>>()
                    .join(",")
                + " ON DUPLICATE KEY UPDATE off_rating=VALUES(off_rating),def_rating=VALUES(def_rating)"
        })
        .collect::<Vec<_>>();

    println!(
        "Prepared {} player upsert jobs and {} player_game upsert jobs",
        player_upsert_jobs.len(),
        player_game_upsert_jobs.len()
    );

    let failed_jobs = execute_upsert_jobs(pool, player_upsert_jobs, "players")
        + execute_upsert_jobs(pool, player_game_upsert_jobs, "player_games");

    if failed_jobs > 0 {
        eprintln!(
            "Finished with {} failed upsert job(s); continuing without crashing",
            failed_jobs
        );
    }

    println!("Upserted ratings for {} players", player_ratings.len());
    println!("Finished compute_ratings in {:?}", start.elapsed());
}

fn execute_upsert_jobs(
    pool: &Pool<ConnectionManager<MysqlConnection>>,
    jobs: Vec<String>,
    table_name: &str,
) -> usize {
    if jobs.is_empty() {
        return 0;
    }

    let worker_count = jobs.len().min(MAX_UPSERT_WORKERS).max(1);
    println!(
        "Running {} {} upsert job(s) with {} worker(s)",
        jobs.len(),
        table_name,
        worker_count
    );

    let jobs = Arc::new(Mutex::new(VecDeque::from(jobs)));
    let mut handles = Vec::with_capacity(worker_count);

    for _ in 0..worker_count {
        let pool = pool.clone();
        let jobs = jobs.clone();
        let table_name = table_name.to_string();
        handles.push(thread::spawn(move || {
            let mut failed_jobs = 0;
            loop {
                let statement = {
                    let mut jobs = jobs.lock().unwrap();
                    jobs.pop_front()
                };

                match statement {
                    Some(statement) => {
                        if !run_upsert_with_retry(pool.clone(), statement, &table_name) {
                            failed_jobs += 1;
                        }
                    }
                    None => return failed_jobs,
                }
            }
        }));
    }

    let mut failed_jobs = 0;
    for h in handles {
        match h.join() {
            Ok(worker_failed_jobs) => failed_jobs += worker_failed_jobs,
            Err(_) => {
                failed_jobs += 1;
                eprintln!("A rating upsert worker panicked");
            }
        }
    }

    failed_jobs
}

fn run_upsert_with_retry(
    pool: Pool<ConnectionManager<MysqlConnection>>,
    statement: String,
    table_name: &str,
) -> bool {
    let query = statement;
    let mut attempt = 0;
    loop {
        attempt += 1;
        match pool.get() {
            Ok(mut conn) => match sql_query(query.clone()).execute(&mut conn) {
                Ok(_) => return true,
                Err(err) => {
                    if should_retry_write(&err) && attempt <= WRITE_MAX_RETRIES {
                        eprintln!(
                            "{} upsert attempt {} failed with retryable DB error: {}",
                            table_name, attempt, err
                        );
                        thread::sleep(WRITE_RETRY_SLEEP);
                        continue;
                    }
                    eprintln!(
                        "{} upsert failed after {} attempt(s): {}",
                        table_name, attempt, err
                    );
                    return false;
                }
            },
            Err(err) => {
                if attempt <= WRITE_MAX_RETRIES {
                    eprintln!(
                        "{} upsert attempt {} failed to acquire DB connection: {}",
                        table_name, attempt, err
                    );
                    thread::sleep(WRITE_RETRY_SLEEP);
                    continue;
                }
                eprintln!(
                    "{} upsert failed after {} attempt(s) due to connection checkout errors: {}",
                    table_name, attempt, err
                );
                return false;
            }
        }
    }
}

fn should_retry_write(err: &DieselError) -> bool {
    match err {
        DieselError::DatabaseError(_, info) => info
            .message()
            .to_ascii_lowercase()
            .contains("lost connection to mysql server"),
        _ => false,
    }
}

fn json_header() -> Header {
    "Content-Type: application/json".parse().unwrap()
}

fn main() {
    let pool = establish_connection();

    let port = env::var("STATS_PORT").unwrap_or_else(|_| "6578".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let server = Server::http(&addr).expect("Failed to start HTTP server");
    println!("Stats server listening on {}", addr);

    for request in server.incoming_requests() {
        match (request.method(), request.url()) {
            (&Method::Post, "/player_ratings") => {
                println!("Received player_ratings request, computing...");
                compute_ratings(&pool);
                let _ = request.respond(
                    Response::from_string("{\"status\":\"ok\"}").with_header(json_header()),
                );
            }
            _ => {
                let _ = request.respond(
                    Response::from_string("{\"error\":\"not found\"}")
                        .with_status_code(404)
                        .with_header(json_header()),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::red_card_penalty_remaining_per90;

    #[test]
    fn red_card_penalty_uses_regular_time_remaining_per90() {
        assert_eq!(red_card_penalty_remaining_per90(90.0, 60), 30.0 / 90.0);
    }

    #[test]
    fn red_card_penalty_includes_overtime_remaining_per90() {
        assert_eq!(red_card_penalty_remaining_per90(120.0, 60), 60.0 / 90.0);
    }

    #[test]
    fn red_card_penalty_handles_cards_during_overtime_per90() {
        assert_eq!(red_card_penalty_remaining_per90(120.0, 105), 15.0 / 90.0);
    }

    #[test]
    fn red_card_penalty_per90_never_rewards_late_cards() {
        assert_eq!(red_card_penalty_remaining_per90(90.0, 105), 0.0);
    }
}
