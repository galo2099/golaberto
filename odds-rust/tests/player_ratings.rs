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
    let input = player_ratings::load(&mut conn, now).unwrap();
    let mut recalculated = ::player_ratings::calculate(
        &input.games,
        &input.goals,
        input.ratings,
        &input.players,
        now.and_utc().timestamp(),
    )
    .unwrap();
    player_ratings::retain_changed_appearances(&mut recalculated, &input.appearance_ratings);
    assert!(recalculated.player_game_ratings.is_empty());
    assert_eq!(recalculated.player_ratings.len(), 2);
    // Edited match data must still update historical appearances on the next run.
    conn.query_drop("INSERT INTO goals VALUES(1,2,2,75,0,0)")
        .unwrap();
    let input = player_ratings::load(&mut conn, now).unwrap();
    let mut recalculated = ::player_ratings::calculate(
        &input.games,
        &input.goals,
        input.ratings,
        &input.players,
        now.and_utc().timestamp(),
    )
    .unwrap();
    player_ratings::retain_changed_appearances(&mut recalculated, &input.appearance_ratings);
    assert_eq!(recalculated.player_game_ratings.len(), 2);
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

#[test]
#[ignore = "set MYSQL_TEST_URL; every write targets connection-local temporary tables"]
fn staged_updates_rollback_all_batches_on_database_error_and_allow_retry() {
    let mut conn = Conn::new(
        std::env::var("MYSQL_TEST_URL")
            .expect("MYSQL_TEST_URL")
            .as_str(),
    )
    .unwrap();
    conn.query_drop("SET SESSION sql_mode='TRADITIONAL'")
        .unwrap();
    conn.query_drop("CREATE TEMPORARY TABLE players(id INT PRIMARY KEY,name VARCHAR(64) NOT NULL,off_rating FLOAT,def_rating FLOAT,rating FLOAT,updated_at DATETIME) ENGINE=InnoDB").unwrap();
    // A strict column limit forces a real write error in the second appearance batch.
    conn.query_drop("CREATE TEMPORARY TABLE player_games(id INT PRIMARY KEY,off_rating FLOAT,def_rating TINYINT) ENGINE=InnoDB").unwrap();
    conn.query_drop("INSERT INTO players VALUES(1,'preserved',0,0,0,'2020-01-01'),(2,'untouched',9,9,9,'2020-01-01')").unwrap();
    conn.query_drop(format!(
        "INSERT INTO player_games VALUES {}",
        (1..=5001)
            .map(|id| format!("({id},0,0)"))
            .collect::<Vec<_>>()
            .join(",")
    ))
    .unwrap();
    let now = NaiveDate::from_ymd_opt(2026, 9, 30)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let raw = ::player_ratings::PlayerRating {
        off: 1.,
        def: 2.,
        minutes: 90.,
    };
    let mut computed = ::player_ratings::Computed {
        player_ratings: HashMap::from([(1, raw.clone()), (9999, raw.clone())]),
        player_game_ratings: (1..=5002).map(|id| ((id, 1), raw.clone())).collect(),
    };
    computed
        .player_game_ratings
        .get_mut(&(5001, 1))
        .unwrap()
        .def = 1000.;
    assert!(player_ratings::persist(&mut conn, &computed, now).is_err());
    assert_eq!(
        conn.query_first::<u64, _>(
            "SELECT COUNT(*) FROM player_games WHERE off_rating<>0 OR def_rating<>0"
        )
        .unwrap(),
        Some(0)
    );
    let player: (f32, String) = conn
        .exec_first(
            "SELECT off_rating,CAST(updated_at AS CHAR) FROM players WHERE id=1",
            (),
        )
        .unwrap()
        .unwrap();
    assert_eq!(player, (0., "2020-01-01 00:00:00".into()));
    // Retry on the same connection verifies staging cleanup after rollback.
    computed
        .player_game_ratings
        .get_mut(&(5001, 1))
        .unwrap()
        .def = 2.;
    player_ratings::persist(&mut conn, &computed, now).unwrap();
    assert_eq!(
        conn.query_first::<u64, _>(
            "SELECT COUNT(*) FROM player_games WHERE off_rating=1 AND def_rating=2"
        )
        .unwrap(),
        Some(5001)
    );
    assert_eq!(
        conn.query_first::<u64, _>("SELECT COUNT(*) FROM players")
            .unwrap(),
        Some(2)
    );
    assert_eq!(
        conn.query_first::<u64, _>("SELECT COUNT(*) FROM player_games")
            .unwrap(),
        Some(5001)
    );
    let untouched: (String, f32, String) = conn
        .exec_first(
            "SELECT name,off_rating,CAST(updated_at AS CHAR) FROM players WHERE id=2",
            (),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        untouched,
        ("untouched".into(), 9., "2020-01-01 00:00:00".into())
    );
    let updated: (String, String) = conn
        .exec_first(
            "SELECT name,CAST(updated_at AS CHAR) FROM players WHERE id=1",
            (),
        )
        .unwrap()
        .unwrap();
    assert_eq!(updated, ("preserved".into(), "2026-09-30 00:00:00".into()));
    // Repeating a successful operation also leaves no staging table behind.
    player_ratings::persist(&mut conn, &computed, now).unwrap();
}

#[test]
fn unchanged_appearances_require_exact_bits_and_complete_stored_ratings() {
    let raw = ::player_ratings::PlayerRating {
        off: 1.,
        def: 2.,
        minutes: 90.,
    };
    let mut computed = ::player_ratings::Computed {
        player_ratings: HashMap::from([(1, raw.clone())]),
        player_game_ratings: (1..=5).map(|id| ((id, 1), raw.clone())).collect(),
    };
    let existing = HashMap::from([
        (1, (Some(1.), Some(2.))),
        (2, (Some(f32::from_bits(1_f32.to_bits() + 1)), Some(2.))),
        (3, (Some(1.), None)),
        (4, (Some(1.), Some(f32::NAN))),
    ]);
    player_ratings::retain_changed_appearances(&mut computed, &existing);
    assert!(!computed.player_game_ratings.contains_key(&(1, 1)));
    for id in 2..=5 {
        assert!(computed.player_game_ratings.contains_key(&(id, 1)));
    }
    assert_eq!(computed.player_ratings.len(), 1);
}

#[test]
#[ignore = "set MYSQL_TEST_URL; every write targets connection-local temporary tables"]
fn staging_preserves_small_large_and_fractional_float_bits() {
    let mut conn = Conn::new(
        std::env::var("MYSQL_TEST_URL")
            .expect("MYSQL_TEST_URL")
            .as_str(),
    )
    .unwrap();
    conn.query_drop("CREATE TEMPORARY TABLE players(id INT PRIMARY KEY,off_rating FLOAT,def_rating FLOAT,rating FLOAT,updated_at DATETIME) ENGINE=InnoDB").unwrap();
    conn.query_drop("CREATE TEMPORARY TABLE player_games(id INT PRIMARY KEY,off_rating FLOAT,def_rating FLOAT) ENGINE=InnoDB").unwrap();
    let values = [
        f32::MIN_POSITIVE,
        1.0e-25,
        -1.0e-25,
        f32::from_bits(0x3f800001),
        -f32::from_bits(0x3f800001),
        12345678.,
        0.,
    ];
    let computed = ::player_ratings::Computed {
        player_ratings: HashMap::new(),
        player_game_ratings: values
            .iter()
            .enumerate()
            .map(|(i, &value)| {
                (
                    (i as i32, 1),
                    ::player_ratings::PlayerRating {
                        off: value,
                        def: -value,
                        minutes: 90.,
                    },
                )
            })
            .collect(),
    };
    conn.query_drop(format!(
        "INSERT INTO player_games(id) VALUES {}",
        (0..values.len())
            .map(|id| format!("({id})"))
            .collect::<Vec<_>>()
            .join(",")
    ))
    .unwrap();
    let now = NaiveDate::from_ymd_opt(2026, 9, 30)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    player_ratings::persist(&mut conn, &computed, now).unwrap();
    let stored: Vec<(i32, f32, f32)> = conn
        .exec(
            "SELECT id,off_rating,def_rating FROM player_games ORDER BY id",
            (),
        )
        .unwrap();
    for (id, off, def) in stored {
        assert_eq!(off.to_bits(), values[id as usize].to_bits());
        assert_eq!(def.to_bits(), (-values[id as usize]).to_bits());
    }
}

#[path = "../../stats/core/tests/support/mod.rs"]
mod calculation_fixture;

#[test]
fn advancing_the_clock_recomputes_player_totals_before_skipping_identical_appearances() {
    let fixture = calculation_fixture::read(
        &serde_json::from_str(include_str!("../../stats/core/tests/fixtures/input.json")).unwrap(),
    );
    let before = ::player_ratings::calculate(
        &fixture.games,
        &fixture.goals,
        fixture.ratings.clone(),
        &fixture.players,
        fixture.now,
    )
    .unwrap();
    let mut after = ::player_ratings::calculate(
        &fixture.games,
        &fixture.goals,
        fixture.ratings,
        &fixture.players,
        fixture.now + 30 * 24 * 60 * 60,
    )
    .unwrap();
    assert!(before.player_ratings.iter().any(|(id, raw)| {
        let next = &after.player_ratings[id];
        let rating =
            (raw.off + raw.def) / raw.minutes * 90. * ::player_ratings::squash_rating(raw.minutes);
        let next_rating = (next.off + next.def) / next.minutes
            * 90.
            * ::player_ratings::squash_rating(next.minutes);
        rating.to_bits() != next_rating.to_bits()
    }));
    let existing = before
        .player_game_ratings
        .iter()
        .map(|((id, _), raw)| (*id, (Some(raw.off), Some(raw.def))))
        .collect();
    player_ratings::retain_changed_appearances(&mut after, &existing);
    assert!(after.player_game_ratings.is_empty());
    assert_eq!(after.player_ratings.len(), before.player_ratings.len());
    // Compare both full raw outputs to the existing core fixture helper too.
    assert_ne!(
        calculation_fixture::output(&before, fixture.now)["players"],
        calculation_fixture::output(&after, fixture.now)["players"]
    );
}
