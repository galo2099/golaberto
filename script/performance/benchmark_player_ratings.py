#!/usr/bin/env python3
"""Compare the actual HTTP endpoint on a disposable copy of local application data.

Every write targets a newly created schema. The source schema is only read.
The benchmark warms ratings first, then measures routine unchanged-history jobs.
"""
import argparse
import json
import os
from pathlib import Path
import re
import socket
import statistics
import subprocess
import time
import urllib.request
import uuid


def sql(statement):
    return subprocess.check_output(
        ["mysql", "-u", "root", "--batch", "--skip-column-names", "-e", statement],
        text=True,
    )


def port():
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--source-database", default="GolAberto_development")
    parser.add_argument("--iterations", type=int, default=3)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if not re.fullmatch(r"[A-Za-z0-9_]+", args.source_database):
        parser.error("invalid source database name")
    if args.iterations < 1:
        parser.error("iterations must be positive")
    source = args.source_database
    schema = "golaberto_player_bench_" + uuid.uuid4().hex[:12]
    servers = []
    created = False
    result = {}
    try:
        sql(f"CREATE DATABASE `{schema}`")
        created = True
        tables = ["championships", "phases", "games", "goals", "players", "player_games", "historical_ratings"]
        for table in tables:
            sql(f"CREATE TABLE `{schema}`.`{table}` LIKE `{source}`.`{table}`")
        # Copy only the game's eligible window, retaining actual column types,
        # indexes, player positions, and stored appearance ratings.
        queries = [
            f"INSERT INTO `{schema}`.games SELECT g.* FROM `{source}`.games g JOIN `{source}`.phases p ON p.id=g.phase_id JOIN `{source}`.championships c ON c.id=p.championship_id WHERE c.category_id=1 AND g.played=1 AND g.date>UTC_TIMESTAMP()-INTERVAL 1456 DAY",
            f"INSERT INTO `{schema}`.phases SELECT p.* FROM `{source}`.phases p WHERE p.id IN (SELECT DISTINCT phase_id FROM `{schema}`.games)",
            f"INSERT INTO `{schema}`.championships SELECT c.* FROM `{source}`.championships c WHERE c.id IN (SELECT DISTINCT championship_id FROM `{schema}`.phases)",
            f"INSERT INTO `{schema}`.players SELECT * FROM `{source}`.players",
            f"INSERT INTO `{schema}`.goals SELECT g.* FROM `{source}`.goals g JOIN `{schema}`.games m ON m.id=g.game_id",
            f"INSERT INTO `{schema}`.player_games SELECT pg.* FROM `{source}`.player_games pg JOIN `{schema}`.games g ON g.id=pg.game_id WHERE pg.`off`>0",
            f"INSERT INTO `{schema}`.historical_ratings SELECT r.* FROM `{source}`.historical_ratings r WHERE r.measure_date>UTC_DATE()-INTERVAL 1463 DAY AND r.team_id IN (SELECT home_id FROM `{schema}`.games UNION SELECT away_id FROM `{schema}`.games)",
        ]
        for query in queries:
            # Preserve legacy zero timestamps in unused columns while cloning.
            # This setting applies only to this disposable-copy connection.
            sql("SET SESSION sql_mode='NO_ENGINE_SUBSTITUTION';" + query)
        counts = {table: int(sql(f"SELECT COUNT(*) FROM `{schema}`.`{table}`")) for table in tables}
        print(json.dumps({"copied_rows": counts}), flush=True)
        endpoints = {}
        log_files = []
        for name, binary in [("baseline", args.baseline), ("candidate", args.candidate)]:
            server_port = port()
            log_path = args.output.with_name(args.output.stem + f"-{name}.log")
            log_path.parent.mkdir(parents=True, exist_ok=True)
            log_file = log_path.open("w")
            log_files.append(log_file)
            env = dict(os.environ, DATABASE_URL=f"mysql://root@127.0.0.1:3306/{schema}", RUST_ODDS_LOG="1")
            server = subprocess.Popen([str(binary.resolve()), "serve", f"127.0.0.1:{server_port}"], env=env, stdout=subprocess.DEVNULL, stderr=log_file)
            servers.append(server)
            endpoints[name] = f"http://127.0.0.1:{server_port}"
            deadline = time.monotonic() + 30
            while True:
                try:
                    with urllib.request.urlopen(endpoints[name] + "/health", timeout=1) as response:
                        assert json.load(response) == {"status": "ok"}
                    break
                except OSError:
                    if server.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError(f"{name} did not start")
                    time.sleep(.05)

        def update(name):
            start = time.perf_counter()
            request = urllib.request.Request(endpoints[name] + "/player_ratings", data=b"", method="POST")
            with urllib.request.urlopen(request, timeout=300) as response:
                assert response.status == 200
                assert json.load(response) == {"status": "ok"}
            elapsed = (time.perf_counter() - start) * 1000
            print(json.dumps({"implementation": name, "http_ms": elapsed}), flush=True)
            return elapsed

        # The first run brings the clone's historical appearance values current.
        # No application data is updated. Both timed implementations see exactly
        # the same unchanged appearances; player weights still use the real clock.
        first_refresh_ms = update("baseline")
        warmup_ms = {"baseline": update("baseline"), "candidate": update("candidate")}
        timings = {"baseline": [], "candidate": []}
        for iteration in range(args.iterations):
            order = ["baseline", "candidate"] if iteration % 2 == 0 else ["candidate", "baseline"]
            for name in order:
                timings[name].append(update(name))
        medians = {name: statistics.median(values) for name, values in timings.items()}
        result = {
            "workload": "routine refresh with already-rated history, actual HTTP requests and normal InnoDB destination tables",
            "source_database_writes": 0,
            "copied_rows": counts,
            "iterations": args.iterations,
            "initial_baseline_refresh_ms": first_refresh_ms,
            "warmup_ms": warmup_ms,
            "timings_ms": timings,
            "median_ms": medians,
            "latency_reduction_percent": 100 * (1 - medians["candidate"] / medians["baseline"]),
            "disposable_schema_cleaned": True,
        }
        for log_file in log_files:
            log_file.flush()
            log_file.close()
    finally:
        for server in servers:
            server.terminate()
            server.wait(timeout=10)
        if created:
            sql(f"DROP DATABASE `{schema}`")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result), flush=True)


if __name__ == "__main__":
    main()
