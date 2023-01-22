use crate::models::{Game, Goal, HistoricalRating, PlayerGame};
use crate::schema::games;
use crate::schema::phases;
use crate::schema::{championships, goals, players};
use chrono::{Duration, NaiveDate};
use diesel::connection::DefaultLoadingMode;
use diesel::dsl::sql;
use diesel::mysql::{Mysql, MysqlConnection};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sql_types::Bool;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::{debug_query, sql_query, ExpressionMethods, JoinOnDsl, SelectableHelper};
use dotenv::dotenv;
use itertools::Itertools;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use std::{env, thread};

pub mod models;
pub mod schema;

const AVG_BASE: f32 = 1.335_025_8;
const HOME_ADV: f32 = 0.161_336_76;

struct PlayerGamePos {
    pg: PlayerGame,
    pos: String,
}

fn establish_connection() -> Pool<ConnectionManager<MysqlConnection>> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
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

fn load_goals(conn: &mut MysqlConnection, games: &[Game]) -> HashMap<i32, Vec<Goal>> {
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
        .fold(HashMap::<i32, Vec<Goal>>::new(), |mut h, x| {
            let x = x.unwrap();
            match h.get_mut(&x.game_id.unwrap()) {
                Some(v) => v.push(x),
                None => {
                    h.insert(x.game_id.unwrap(), vec![x]);
                }
            };
            h
        });
    println!("goals {:?}", s.elapsed());
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
    println!("ratings {:?}", s.elapsed());
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
    println!("players {:?}", s.elapsed());
    x1
}

fn squash_date(timestamp: i64, now: i64) -> f32 {
    use std::f32::consts::E;
    let x = (timestamp - now) as f32 / (730.0 * 24.0 * 60.0 * 60.0);
    1.0 + (E.powf(x) - E.powf(-x)) / (E.powf(x) + E.powf(-x))
}

fn main() {
    let pool = establish_connection();
    let start = Instant::now();
    let games = Arc::new(load_games(&mut pool.get().unwrap()));
    println!("{:?}", start.elapsed());

    let (goals, mut ratings, players) = thread::scope(|s| {
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

    println!("{:?}", start.elapsed());
    println!("{:?}", games.len());
    println!("{:?}", goals.len());
    println!("{:?}", ratings.len());
    println!("{:?}", players.len());

    struct PlayerRating {
        off: f32,
        def: f32,
        minutes: f32,
    }
    let mut player_ratings = HashMap::<i32, PlayerRating>::new();
    let mut player_game_ratings = HashMap::<i32, PlayerRating>::new();
    for game in games.iter() {
        if players.get(&game.id).is_none() {
            continue;
        }

        let home_adv: f32 = match game.home_field {
            0 => HOME_ADV,
            1 => 0.0,
            2 => -HOME_ADV,
            _ => panic!("invalid home_adv"),
        };

        let now = chrono::offset::Utc::now().timestamp();
        let weight = squash_date(game.date.timestamp(), now);

        let length = match game.home_aet {
            Some(_) => 120.0,
            None => 90.0,
        };

        get_rating(&mut ratings, game.date.date(), game.home_id);
        get_rating(&mut ratings, game.date.date(), game.away_id);

        let home_rating: &HistoricalRating = ratings[&game.home_id].front().unwrap();
        let away_rating: &HistoricalRating = ratings[&game.away_id].front().unwrap();

        let home_for_zero_per90 = -(away_rating.def_rating + home_adv)
            / ((away_rating.def_rating + home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548)
            / length
            / 11.0;
        let home_for_goal_weight = 1.0 / ((away_rating.def_rating + home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548);
        let home_agg_zero_per90 = (away_rating.off_rating - home_adv)
            / ((away_rating.off_rating - home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548)
            / length
            / 11.0;
        let home_agg_goal_weight = -1.0 / ((away_rating.off_rating - home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548);

        let away_for_zero_per90 = -(home_rating.def_rating - home_adv)
            / ((home_rating.def_rating - home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548)
            / length
            / 11.0;
        let away_for_goal_weight = 1.0 / ((home_rating.def_rating - home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548);
        let away_agg_zero_per90 = (home_rating.off_rating + home_adv)
            / ((home_rating.off_rating + home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548)
            / length
            / 11.0;
        let away_agg_goal_weight = -1.0 / ((home_rating.off_rating + home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548);

        let home_players = players[&game.id]
            .iter()
            .filter(|x| x.pg.team_id == game.home_id)
            .collect::<Vec<_>>();
        let away_players = players[&game.id]
            .iter()
            .filter(|x| x.pg.team_id == game.away_id)
            .collect::<Vec<_>>();

        let empty_vec = Vec::new();
        let home_goals = goals
            .get(&game.id)
            .unwrap_or(&empty_vec)
            .iter()
            .filter(|&goal| {
                (goal.team_id == game.away_id && goal.own_goal)
                    || (goal.team_id == game.home_id && !goal.own_goal)
            })
            .collect::<Vec<_>>();
        let away_goals = goals
            .get(&game.id)
            .unwrap_or(&empty_vec)
            .iter()
            .filter(|&goal| {
                (goal.team_id == game.home_id && goal.own_goal)
                    || (goal.team_id == game.away_id && !goal.own_goal)
            })
            .collect::<Vec<_>>();

        let intervals = home_players
            .iter()
            .flat_map(|x| [x.pg.on, x.pg.off])
            .unique()
            .sorted();
        for (from, to) in intervals.tuple_windows() {
            let hp = home_players
                .iter()
                .filter(|x| std::cmp::max(from, x.pg.on) < std::cmp::min(to, x.pg.off))
                .collect::<Vec<_>>();
            let pos = hp.iter().fold(
                HashMap::from(["g", "dc", "cm", "fw", ""].map(|x| (x, 0.0))),
                |mut h, x| {
                    *h.entry(&x.pos).or_default() += 1.0;
                    h
                },
            );
            let off_windividual = hp.len() as f32
                / (pos["g"] * 0.3 + pos["dc"] * 0.7 + pos["cm"] + pos["fw"] * 1.0 + pos[""]);
            let off_w = HashMap::from([
                ("g", off_windividual * 0.3),
                ("dc", off_windividual * 0.7),
                ("cm", off_windividual),
                ("fw", off_windividual * 1.0),
                ("", off_windividual),
            ]);
            let def_windividual = hp.len() as f32
                / (pos["g"] * 4.0 + pos["dc"] * 2.0 + pos["cm"] + pos["fw"] * 0.5 + pos[""]);
            let def_w = HashMap::from([
                ("g", def_windividual * 4.0),
                ("dc", def_windividual * 2.0),
                ("cm", def_windividual),
                ("fw", def_windividual * 0.5),
                ("", def_windividual),
            ]);

            let home_goals_interval = home_goals
                .iter()
                .filter(|g| g.time > from && g.time <= to)
                .collect::<Vec<_>>();
            let away_goals_interval = away_goals
                .iter()
                .filter(|g| g.time > from && g.time <= to)
                .count();
            let home_goals_own = home_goals_interval.iter().filter(|g| g.own_goal).count();
            let home_goals_regular = home_goals_interval
                .iter()
                .filter(|g| !g.own_goal && !g.penalty)
                .collect::<Vec<_>>();
            let home_goals_penalty = home_goals_interval
                .iter()
                .filter(|g| !g.own_goal && g.penalty)
                .collect::<Vec<_>>();

            for v in &hp {
                let player_rating = player_ratings
                    .entry(v.pg.player_id)
                    .or_insert(PlayerRating {
                        off: 0.0,
                        def: 0.0,
                        minutes: 0.0,
                    });
                let player_game_rating =
                    player_game_ratings.entry(v.pg.id).or_insert(PlayerRating {
                        off: 0.0,
                        def: 0.0,
                        minutes: 0.0,
                    });
                let minutes = (to - from) as f32;
                player_rating.minutes += minutes * weight;
                player_game_rating.minutes += minutes;
                let off_player_weight = off_w[&*v.pos];
                let regular_goals = home_goals_regular
                    .iter()
                    .filter(|x| x.player_id == v.pg.player_id)
                    .count();
                let penalty_goals = home_goals_penalty
                    .iter()
                    .filter(|x| x.player_id == v.pg.player_id)
                    .count();

                let off = minutes * home_for_zero_per90 * off_player_weight
                    + home_goals_own as f32 * home_for_goal_weight * off_player_weight
                        / (hp.len() as f32)
                    + (home_goals_regular.len() as f32) * home_for_goal_weight * off_player_weight
                        / (hp.len() as f32)
                        / 4.0
                        * 3.0
                    + (home_goals_penalty.len() as f32) * home_for_goal_weight * off_player_weight
                        / (hp.len() as f32)
                        / 6.0
                        * 5.0
                    + regular_goals as f32 * home_for_goal_weight / 4.0
                    + penalty_goals as f32 * home_for_goal_weight / 6.0;
                let def = (minutes * home_agg_zero_per90
                    + away_goals_interval as f32 * home_agg_goal_weight / (hp.len() as f32))
                    * def_w[&*v.pos];
                player_rating.off += off * weight;
                player_rating.def += def * weight;
                player_game_rating.off += off;
                player_game_rating.def += def;
            }
        }

        let intervals = away_players
            .iter()
            .flat_map(|x| [x.pg.on, x.pg.off])
            .unique()
            .sorted();
        for (from, to) in intervals.tuple_windows() {
            let ap = away_players
                .iter()
                .filter(|x| std::cmp::max(from, x.pg.on) < std::cmp::min(to, x.pg.off))
                .collect::<Vec<_>>();
            let pos = ap.iter().fold(
                HashMap::from(["g", "dc", "cm", "fw", ""].map(|x| (x, 0.0))),
                |mut h, x| {
                    *h.entry(&x.pos).or_default() += 1.0;
                    h
                },
            );
            let off_windividual = ap.len() as f32
                / (pos["g"] * 0.3 + pos["dc"] * 0.7 + pos["cm"] + pos["fw"] * 1.0 + pos[""]);
            let off_w = HashMap::from([
                ("g", off_windividual * 0.3),
                ("dc", off_windividual * 0.7),
                ("cm", off_windividual),
                ("fw", off_windividual * 1.0),
                ("", off_windividual),
            ]);
            let def_windividual = ap.len() as f32
                / (pos["g"] * 4.0 + pos["dc"] * 2.0 + pos["cm"] + pos["fw"] * 0.5 + pos[""]);
            let def_w = HashMap::from([
                ("g", def_windividual * 4.0),
                ("dc", def_windividual * 2.0),
                ("cm", def_windividual),
                ("fw", def_windividual * 0.5),
                ("", def_windividual),
            ]);

            let away_goals_interval = away_goals
                .iter()
                .filter(|g| g.time > from && g.time <= to)
                .collect::<Vec<_>>();
            let home_goals_interval = home_goals
                .iter()
                .filter(|g| g.time > from && g.time <= to)
                .count();
            let away_goals_own = away_goals_interval.iter().filter(|g| g.own_goal).count();
            let away_goals_regular = away_goals_interval
                .iter()
                .filter(|g| !g.own_goal && !g.penalty)
                .collect::<Vec<_>>();
            let away_goals_penalty = away_goals_interval
                .iter()
                .filter(|g| !g.own_goal && g.penalty)
                .collect::<Vec<_>>();

            for v in &ap {
                let player_rating = player_ratings
                    .entry(v.pg.player_id)
                    .or_insert(PlayerRating {
                        off: 0.0,
                        def: 0.0,
                        minutes: 0.0,
                    });
                let player_game_rating =
                    player_game_ratings.entry(v.pg.id).or_insert(PlayerRating {
                        off: 0.0,
                        def: 0.0,
                        minutes: 0.0,
                    });
                let minutes = (to - from) as f32;
                player_rating.minutes += minutes * weight;
                player_game_rating.minutes += minutes;
                let off_player_weight = off_w[&*v.pos];
                let regular_goals = away_goals_regular
                    .iter()
                    .filter(|x| x.player_id == v.pg.player_id)
                    .count();
                let penalty_goals = away_goals_penalty
                    .iter()
                    .filter(|x| x.player_id == v.pg.player_id)
                    .count();

                let off = minutes * away_for_zero_per90 * off_player_weight
                    + away_goals_own as f32 * away_for_goal_weight * off_player_weight
                        / (ap.len() as f32)
                    + (away_goals_regular.len() as f32) * away_for_goal_weight * off_player_weight
                        / (ap.len() as f32)
                        / 4.0
                        * 3.0
                    + (away_goals_penalty.len() as f32) * away_for_goal_weight * off_player_weight
                        / (ap.len() as f32)
                        / 6.0
                        * 5.0
                    + regular_goals as f32 * away_for_goal_weight / 4.0
                    + penalty_goals as f32 * away_for_goal_weight / 6.0;
                let def = (minutes * away_agg_zero_per90
                    + home_goals_interval as f32 * away_agg_goal_weight / (ap.len() as f32))
                    * def_w[&*v.pos];
                player_rating.off += off * weight;
                player_rating.def += def * weight;
                player_game_rating.off += off;
                player_game_rating.def += def;
            }
        }
    }

    println!("{:?}", start.elapsed());

    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
    let mut handles = Vec::new();
    for c in &player_ratings.iter().chunks(1000) {
        let pool = pool.clone();
        let statement = sql_query("INSERT INTO players (id,off_rating,def_rating,rating,created_at,updated_at) VALUES ".to_owned() +
                &c.map(
                    |(k, v)|
                        format!("({}, {}, {}, {}, \"{}\", \"{}\")",
                                k,
                                v.off / v.minutes * 90.0,
                                v.def / v.minutes * 90.0,
                                (v.off + v.def) / (v.minutes + 900.0) * 90.0, now, now))
                    .collect::<Vec<String>>()
                    .join(",") +
                " ON DUPLICATE KEY UPDATE off_rating=VALUES(off_rating),def_rating=VALUES(def_rating),rating=VALUES(rating),updated_at=VALUES(updated_at)");
        // let sql = debug_query::<Mysql, _>(&statement).to_string();
        // println!("{}", sql);
        handles.push(thread::spawn(move || {
            // println!("thread");
            statement.execute(&mut pool.get().unwrap()).unwrap();
            // println!("thread done");
        }));
    }

    println!("{}", player_game_ratings.len());
    for c in &player_game_ratings.iter().chunks(50000) {
        let pool = pool.clone();
        let statement = sql_query("INSERT INTO player_games (id, off_rating, def_rating) VALUES ".to_owned() +
            &c.map(
                |(k, v)|
                    format!("({}, {}, {})",
                            k,
                            v.off,
                            v.def))
                .collect::<Vec<String>>()
                .join(",") +
            " ON DUPLICATE KEY UPDATE off_rating=VALUES(off_rating),def_rating=VALUES(def_rating)");
        // let sql = debug_query::<Mysql, _>(&statement).to_string();
        // println!("{}", sql);
        handles.push(thread::spawn(move || {
            // println!("thread pgr");
            statement.execute(&mut pool.get().unwrap()).unwrap();
            // println!("thread pgr done");
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
    // let mut x = player_ratings.iter().collect::<Vec<_>>();
    // x.sort_by(|a,b| b.1.off.partial_cmp(&a.1.off).unwrap());
    // println!("{} {} {} {}", x[0].0, x[0].1.off, x[0].1.def, x[0].1.minutes);
    // println!("{} {} {} {}", x[1].0, x[1].1.off, x[1].1.def, x[1].1.minutes);
    // println!("{} {} {} {}", x[2].0, x[2].1.off, x[2].1.def, x[2].1.minutes);
    println!("{:?}", player_ratings.len());
    println!("{:?}", start.elapsed());
}

fn get_rating(ratings: &mut HashMap<i32, VecDeque<HistoricalRating>>, date: NaiveDate, id: i32) {
    let r = ratings.get_mut(&id).expect("");
    while r.len() > 1 && date > r[1].measure_date {
        r.pop_front();
    }
}
