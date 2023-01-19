use std::collections::{HashSet, HashMap};
use diesel::mysql::{Mysql, MysqlConnection};
use dotenv::dotenv;
use std::{env, thread};
use std::time::Instant;
use chrono::{Duration, NaiveDateTime};
use diesel::{ExpressionMethods, JoinOnDsl, Selectable, Queryable, SelectableHelper, Identifiable};
use diesel::{Connection, QueryDsl, BelongingToDsl};
use diesel::connection::DefaultLoadingMode;
use diesel::dsl::sql;
use diesel::sql_types::{Bool, Integer};
use diesel_async::{AsyncConnection, AsyncMysqlConnection, RunQueryDsl};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::mobc::Pool;
use crate::models::{Championship, Game, Goal, HistoricalRating, Phase, PlayerGame};
use crate::schema::{championships, goals};
use crate::schema::phases;
use crate::schema::games;
use crate::schema::historical_ratings::dsl::historical_ratings;
use futures::executor::block_on;

pub mod models;
pub mod schema;

pub async fn establish_connection() -> Pool<AsyncMysqlConnection> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let config = AsyncDieselConnectionManager::new(database_url);
    Pool::new(config)
}

async fn load_games(pool: Pool<AsyncMysqlConnection>) -> Vec<Game> {
    let mut conn = pool.get()
        .await
        .expect("Failed to connect to database");
    let start = chrono::Utc::now() - Duration::weeks(4*52);
    championships::table
        .inner_join(phases::table.on(phases::dsl::championship_id.eq(championships::dsl::id)))
        .inner_join(games::table.on(phases::dsl::id.eq(games::dsl::phase_id)))
        .select(Game::as_select())
        .filter(championships::category_id.eq(1))
        .filter(games::date.gt(start.naive_utc()))
        .filter(games::played.eq(true))
        .order(games::date)
        .load::<Game>(&mut conn)
        .await
        .expect("champ")
}

async fn load_goals(pool: Pool<AsyncMysqlConnection>, games: &Vec<Game>) -> HashMap<i32, Vec<Goal>> {
    println!("goals before get");
    let mut conn = pool.get()
        .await
        .expect("Failed to connect to database");
    println!("goals after get");
    let s = Instant::now();
    let x1: HashMap<i32, Vec<Goal>> = goals::table
        .select(Goal::as_select())
        .filter(sql::<Bool>(
            &*format!("game_id in ({})",
                      games.iter().map(|g|
                          g.id.to_string()).collect::<Vec<_>>().join(",")
            )))
        .load::<Goal>(&mut conn)
        .await
        .expect("goal")
        .into_iter()
        .fold(HashMap::new(), |mut h, x| {
            match h.get_mut(&x.team_id) {
                Some(v) => v.push(x),
                None => { h.insert(x.team_id, vec![x]); },
            };
            h
        });
    println!("goals {:?}", s.elapsed());
    x1
}

fn dedup<T: Copy + Eq + std::hash::Hash>(iter: impl Iterator<Item=T>) -> impl Iterator<Item=T> {
    let mut seen = HashSet::new();
    iter.filter(move |&x| seen.insert(x))
}

async fn load_ratings(pool: Pool<AsyncMysqlConnection>, games: &Vec<Game>) -> HashMap<i32, Vec<HistoricalRating>> {
    println!("ratings before get");
    let mut conn = pool.get()
        .await
        .expect("Failed to connect to database");
    println!("ratings after get");
    use self::schema::historical_ratings::dsl::*;
    let s = Instant::now();
    let start = chrono::Utc::now() - Duration::weeks(4*52+1);
    let x1 = historical_ratings
        .select(HistoricalRating::as_select())
        .filter(measure_date.gt(start.date_naive()))
        .filter(sql::<Bool>(
            &*format!("team_id in ({})",
                      dedup(games.iter().map(|g|
                          [g.home_id, g.away_id]).flatten()).map(|x| x.to_string()).collect::<Vec<_>>().join(",")
            )))
        .order(team_id)
        .then_order_by(measure_date)
        .load::<HistoricalRating>(&mut conn)
        .await
        .expect("ratings")
        .into_iter()
        .fold(HashMap::<i32, Vec<HistoricalRating>>::new(), |mut h, x| {
            match h.get_mut(&x.team_id) {
                Some(v) => v.push(x),
                None => { h.insert(x.team_id, vec![x]); },
            }
            h
        })
        ;
    println!("ratings {:?}", s.elapsed());
    return x1;
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let pool = establish_connection().await;
    let start = Instant::now();
    let games = load_games(pool.clone()).await;
    println!("{:?}", start.elapsed());

    let goals = load_goals(pool.clone(), &games);
    let ratings = load_ratings(pool.clone(), &games);

    futures::join!(ratings, goals);
    println!("{:?}", start.elapsed());
}