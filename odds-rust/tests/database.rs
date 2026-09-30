use golaberto_odds::{database, ratings::HistoricalRating};
use mysql::{prelude::Queryable, Conn};

#[test]
#[ignore = "set MYSQL_TEST_URL; all writes use a connection-local temporary table"]
fn historical_upserts_round_and_roll_back_atomically() {
    let url = std::env::var("MYSQL_TEST_URL").expect("MYSQL_TEST_URL");
    let mut conn = Conn::new(url.as_str()).unwrap();
    conn.query_drop("SET SESSION sql_mode='TRADITIONAL'")
        .unwrap();
    conn.query_drop("CREATE TEMPORARY TABLE historical_ratings (team_id INT NOT NULL,off_rating DOUBLE NOT NULL,def_rating DOUBLE NOT NULL,rating DOUBLE NOT NULL,measure_date DATE NOT NULL,UNIQUE KEY(team_id,measure_date)) ENGINE=InnoDB").unwrap();
    let row = HistoricalRating {
        team_id: 1,
        off_rating: 1.23456789,
        def_rating: 2.34567891,
        rating: 56.78912345,
        measure_date: "2026-09-30".into(),
    };
    database::persist_history(&mut conn, std::slice::from_ref(&row)).unwrap();
    let values: (f64, f64, f64) = conn
        .query_first("SELECT off_rating,def_rating,rating FROM historical_ratings")
        .unwrap()
        .unwrap();
    assert_eq!(values, (1.234568, 2.345679, 56.789123));
    let mut updated = row.clone();
    updated.rating = 70.;
    database::persist_history(&mut conn, &[updated]).unwrap();
    assert_eq!(
        conn.query_first::<f64, _>("SELECT rating FROM historical_ratings")
            .unwrap(),
        Some(70.)
    );
    conn.query_drop("DELETE FROM historical_ratings").unwrap();
    let mut rows: Vec<_> = (1..=1000)
        .map(|id| {
            let mut r = row.clone();
            r.team_id = id;
            r
        })
        .collect();
    let mut invalid = row;
    invalid.measure_date = "invalid-date".into();
    rows.push(invalid);
    assert!(database::persist_history(&mut conn, &rows).is_err());
    assert_eq!(
        conn.query_first::<u64, _>("SELECT COUNT(*) FROM historical_ratings")
            .unwrap(),
        Some(0)
    );
    database::persist_history(&mut conn, &[]).unwrap();
}
