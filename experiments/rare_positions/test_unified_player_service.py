#!/usr/bin/env python3
"""Opt-in player endpoint smoke test; writes only a newly created disposable schema."""
import argparse
import json
import math
import os
from pathlib import Path
import socket
import subprocess
import time
import urllib.error
import urllib.request
import uuid


def sql(statement):
    return subprocess.check_output(["mysql", "-u", "root", "--batch", "--skip-column-names", "-e", statement], text=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    schema = "golaberto_player_test_" + uuid.uuid4().hex[:12]
    with socket.socket() as socket_probe:
        socket_probe.bind(("127.0.0.1", 0))
        port = socket_probe.getsockname()[1]
    server = None
    created = False
    try:
        sql(f"CREATE DATABASE `{schema}`")
        created = True
        sql(f"USE `{schema}`;" + ";".join([
            "CREATE TABLE championships(id INT PRIMARY KEY,category_id INT NOT NULL) ENGINE=InnoDB",
            "CREATE TABLE phases(id INT PRIMARY KEY,championship_id INT NOT NULL) ENGINE=InnoDB",
            "CREATE TABLE games(id INT PRIMARY KEY,home_id INT,away_id INT,date DATETIME,home_field INT,home_aet INT,phase_id INT,played BOOL) ENGINE=InnoDB",
            "CREATE TABLE goals(game_id INT,player_id INT,team_id INT,time INT,penalty BOOL,own_goal BOOL) ENGINE=InnoDB",
            "CREATE TABLE players(id INT PRIMARY KEY,name VARCHAR(64) NOT NULL,position VARCHAR(3),off_rating FLOAT,def_rating FLOAT,rating FLOAT,updated_at DATETIME) ENGINE=InnoDB",
            "CREATE TABLE player_games(id INT PRIMARY KEY,game_id INT,player_id INT,team_id INT,`on` INT,`off` INT,red BOOL,off_rating FLOAT,def_rating FLOAT) ENGINE=InnoDB",
            "CREATE TABLE historical_ratings(team_id INT,measure_date DATE,off_rating FLOAT,def_rating FLOAT) ENGINE=InnoDB",
            "INSERT INTO championships VALUES(1,1)",
            "INSERT INTO phases VALUES(1,1)",
            "INSERT INTO games VALUES(1,1,2,UTC_TIMESTAMP()-INTERVAL 1 DAY,0,NULL,1,1)",
            "INSERT INTO goals VALUES(1,1,1,45,0,0)",
            "INSERT INTO players VALUES(1,'home player','fw',0,0,0,'2020-01-01'),(2,'away player','dc',0,0,0,'2020-01-01')",
            "INSERT INTO player_games VALUES(1,1,1,1,0,90,0,0,0),(2,1,2,2,0,90,0,0,0)",
            "INSERT INTO historical_ratings VALUES(1,UTC_DATE()-INTERVAL 2 DAY,1.3,1.4),(2,UTC_DATE()-INTERVAL 2 DAY,1.2,1.5)",
        ]))
        env = dict(os.environ, DATABASE_URL=f"mysql://root@127.0.0.1:3306/{schema}", RUST_ODDS_LOG="0")
        server = subprocess.Popen([str(args.binary.resolve()), "serve", f"127.0.0.1:{port}"], env=env,
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        base = f"http://127.0.0.1:{port}"
        deadline = time.monotonic() + 10
        while True:
            try:
                with urllib.request.urlopen(base + "/health", timeout=1) as response:
                    assert json.load(response) == {"status": "ok"}
                break
            except OSError:
                if server.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError("unified service did not start")
                time.sleep(.01)
        def update():
            request = urllib.request.Request(base + "/player_ratings", data=b"", method="POST")
            with urllib.request.urlopen(request, timeout=30) as response:
                return response.status, json.load(response)
        status, body = update()
        assert (status, body) == (200, {"status": "ok"})
        stored = sql(f"SELECT id,name,off_rating,def_rating,rating FROM `{schema}`.players ORDER BY id")
        rows = [line.split("\t") for line in stored.splitlines()]
        assert len(rows) == 2 and rows[0][1] == "home player" and rows[1][1] == "away player"
        assert all(math.isfinite(float(value)) for row in rows for value in row[2:])
        assert any(float(row[2]) != 0 for row in rows)
        assert sql(f"SELECT COUNT(*) FROM `{schema}`.player_games WHERE off_rating<>0 OR def_rating<>0").strip() == "2"
        identities = sql(f"SELECT id,game_id,player_id,team_id FROM `{schema}`.player_games ORDER BY id")
        assert identities == "1\t1\t1\t1\n2\t1\t2\t2\n"
        sql(f"UPDATE `{schema}`.players SET position='bad' WHERE id=1")
        try:
            update()
            raise AssertionError("invalid player data was accepted")
        except urllib.error.HTTPError as error:
            assert error.code == 500
            assert json.load(error) == {"error": "rating database operation failed"}
        assert sql(f"SELECT id,name,off_rating,def_rating,rating FROM `{schema}`.players ORDER BY id") == stored
        with urllib.request.urlopen(base + "/health", timeout=1) as response:
            assert json.load(response) == {"status": "ok"}
        result = dict(endpoint="/player_ratings", success_status=status, response=body,
                      updated_players=2, updated_appearances=2, identities_preserved=True,
                      invalid_data_status=500, ratings_unchanged_on_failure=True, health_after_failure=True,
                      application_database_writes=0, disposable_schema_cleaned=True)
    finally:
        if server is not None:
            server.terminate()
            server.wait(timeout=5)
        if created:
            sql(f"DROP DATABASE `{schema}`")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
