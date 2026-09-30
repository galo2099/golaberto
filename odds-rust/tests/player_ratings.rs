use chrono::NaiveDate;
use golaberto_odds::player_ratings;
use mysql::{prelude::Queryable, Conn};
use std::collections::HashMap;

#[test]
#[ignore = "set MYSQL_TEST_URL; every write targets connection-local temporary tables"]
fn player_input_filters_updates_and_rollback_use_only_temporary_tables() {
    let mut conn = Conn::new(
        std::env::var("MYSQL_TEST_URL")
            .expect("MYSQL_TEST_URL")
            .as_str(),
    )
    .unwrap();
    conn.query_drop("SET SESSION sql_mode='TRADITIONAL'")
        .unwrap();
    for sql in [
        "CREATE TEMPORARY TABLE championships(id INT PRIMARY KEY,category_id INT NOT NULL) ENGINE=InnoDB",
        "CREATE TEMPORARY TABLE phases(id INT PRIMARY KEY,championship_id INT NOT NULL) ENGINE=InnoDB",
        "CREATE TEMPORARY TABLE games(id INT PRIMARY KEY,home_id INT NOT NULL,away_id INT NOT NULL,date DATETIME NOT NULL,home_field INT NOT NULL,home_aet INT NULL,phase_id INT NOT NULL,played BOOL NOT NULL) ENGINE=InnoDB",
        "CREATE TEMPORARY TABLE goals(game_id INT,player_id INT,team_id INT,time INT,penalty BOOL,own_goal BOOL) ENGINE=InnoDB",
        "CREATE TEMPORARY TABLE players(id INT PRIMARY KEY,name VARCHAR(64) NOT NULL,position VARCHAR(3),off_rating FLOAT,def_rating FLOAT,rating FLOAT,updated_at DATETIME NOT NULL) ENGINE=InnoDB",
        "CREATE TEMPORARY TABLE player_games(id INT PRIMARY KEY,game_id INT NOT NULL,player_id INT NOT NULL,team_id INT NOT NULL,`on` INT NOT NULL,`off` INT NOT NULL,red BOOL NOT NULL,off_rating FLOAT,def_rating FLOAT) ENGINE=InnoDB",
        "CREATE TEMPORARY TABLE historical_ratings(team_id INT,measure_date DATE,off_rating FLOAT,def_rating FLOAT) ENGINE=InnoDB",
        "INSERT INTO championships VALUES(1,1),(2,2)",
        "INSERT INTO phases VALUES(1,1),(2,2)",
        "INSERT INTO games VALUES(1,1,2,'2026-09-29 12:00:00',0,NULL,1,1),(2,1,2,'2026-09-29 12:00:00',0,NULL,2,1),(3,1,2,'2026-09-29 12:00:00',0,NULL,1,0),(4,1,2,'2020-09-29 12:00:00',0,NULL,1,1)",
        "INSERT INTO players VALUES(1,'original name','fw',0,0,0,'2026-01-01'),(2,'other name','dc',0,0,0,'2026-01-01')",
        "INSERT INTO player_games VALUES(1,1,1,1,0,90,0,0,0),(2,1,2,2,0,90,0,0,0),(3,1,2,2,0,0,0,0,0),(4,2,2,2,0,90,0,0,0)",
        "INSERT INTO historical_ratings VALUES(1,'2026-09-28',1.3,1.4),(2,'2026-09-28',1.2,1.5),(1,'2020-01-01',0,0)",
        "INSERT INTO goals VALUES(1,1,1,45,0,0)",
    ] { conn.query_drop(sql).unwrap(); }
    let now = NaiveDate::from_ymd_opt(2026, 9, 30)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let input = player_ratings::load(&mut conn, now).unwrap();
    assert_eq!(input.games.len(), 1);
    assert_eq!(input.players[&1].len(), 2);
    assert_eq!(input.goals[&1].len(), 1);
    assert_eq!(input.ratings[&1].len(), 1);
    let computed = ::player_ratings::calculate(
        &input.games,
        &input.goals,
        input.ratings,
        &input.players,
        now.and_utc().timestamp(),
    )
    .unwrap();
    player_ratings::persist(&mut conn, &computed, now).unwrap();
    let row: (String, f32, f32, f32) = conn
        .exec_first(
            "SELECT name,off_rating,def_rating,rating FROM players WHERE id=1",
            (),
        )
        .unwrap()
        .unwrap();
    let raw = &computed.player_ratings[&1];
    assert_eq!(row.0, "original name");
    assert_eq!(row.1.to_bits(), (raw.off / raw.minutes * 90.).to_bits());
    assert_eq!(row.2.to_bits(), (raw.def / raw.minutes * 90.).to_bits());
    assert_eq!(
        row.3.to_bits(),
        ((raw.off + raw.def) / raw.minutes * 90. * ::player_ratings::squash_rating(raw.minutes))
            .to_bits()
    );
    let raw = &computed.player_game_ratings[&(1, 1)];
    let appearance: (i32, i32, i32, f32, f32) = conn
        .exec_first(
            "SELECT game_id,player_id,team_id,off_rating,def_rating FROM player_games WHERE id=1",
            (),
        )
        .unwrap()
        .unwrap();
    assert_eq!(appearance, (1, 1, 1, raw.off, raw.def));
    let before = row.1;
    let mut invalid = ::player_ratings::Computed {
        player_ratings: HashMap::from([(
            1,
            ::player_ratings::PlayerRating {
                off: 10.,
                def: 20.,
                minutes: 90.,
            },
        )]),
        player_game_ratings: HashMap::from([(
            (1, 1),
            ::player_ratings::PlayerRating {
                off: f32::INFINITY,
                def: 0.,
                minutes: 90.,
            },
        )]),
    };
    assert!(player_ratings::persist(&mut conn, &invalid, now).is_err());
    assert_eq!(
        conn.exec_first::<f32, _, _>("SELECT off_rating FROM players WHERE id=1", ())
            .unwrap(),
        Some(before)
    );
    invalid.player_game_ratings.clear();
    invalid.player_ratings.clear();
    player_ratings::persist(&mut conn, &invalid, now).unwrap();
    // Cross both batching ceilings with existing identities intact.
    conn.query_drop(format!(
        "INSERT INTO players(id,name,updated_at) VALUES {}",
        (3..=1003)
            .map(|id| format!("({id},'name','2026-01-01')"))
            .collect::<Vec<_>>()
            .join(",")
    ))
    .unwrap();
    conn.query_drop(format!(
        "INSERT INTO player_games(id,game_id,player_id,team_id,`on`,`off`,red) VALUES {}",
        (5..=5005)
            .map(|id| format!("({id},1,1,1,0,90,0)"))
            .collect::<Vec<_>>()
            .join(",")
    ))
    .unwrap();
    let rating = ::player_ratings::PlayerRating {
        off: 1.,
        def: 2.,
        minutes: 90.,
    };
    let large = ::player_ratings::Computed {
        player_ratings: (3..=1003).map(|id| (id, rating.clone())).collect(),
        player_game_ratings: (5..=5005).map(|id| ((id, 1), rating.clone())).collect(),
    };
    player_ratings::persist(&mut conn, &large, now).unwrap();
    assert_eq!(
        conn.query_first::<u64, _>(
            "SELECT COUNT(*) FROM players WHERE off_rating=1 AND def_rating=2"
        )
        .unwrap(),
        Some(1001)
    );
    assert_eq!(
        conn.query_first::<u64, _>(
            "SELECT COUNT(*) FROM player_games WHERE off_rating=1 AND def_rating=2"
        )
        .unwrap(),
        Some(5001)
    );
}
