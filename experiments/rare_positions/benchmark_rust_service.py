#!/usr/bin/env python3
"""Archived paired service benchmark using a historical Go service binary.

Build the Go benchmark host from a historical checkout. The benchmark uses
disposable MySQL writes and measures HTTP latency and macOS peak RSS.
"""
import argparse
import http.client
import json
import os
from pathlib import Path
import re
import signal
import socket
import statistics
import subprocess
import time
import uuid

from compare_rust_service import compare


def mysql(sql):
    return subprocess.check_output(["mysql", "-u", "root", "--batch", "--skip-column-names",
                                    "-e", sql], text=True)


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


class Server:
    def __init__(self, name, binary, env, out, scenario):
        self.name = name
        self.port = free_port()
        address = f"127.0.0.1:{self.port}"
        command = ([str(binary), "-test.run=^TestRustServiceBenchmarkServer$", "-test.timeout=10m"]
                   if name == "go" else [str(binary), "serve", address])
        self.time_path = out / f"{scenario}-{name}-resource.txt"
        self.log = open(out / f"{scenario}-{name}.log", "wb")
        self.process = subprocess.Popen(["/usr/bin/time", "-l", "-o", str(self.time_path), *command],
            env=dict(env, RUST_SERVICE_BENCHMARK_ADDRESS=address),
            stdout=self.log, stderr=self.log)
        deadline = time.monotonic() + 10
        while True:
            try:
                with socket.create_connection(("127.0.0.1", self.port), timeout=.1):
                    break
            except OSError:
                if self.process.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError(f"{name} failed to start: {self.log.name}")
                time.sleep(.01)
        processes = subprocess.check_output(["ps", "-axo", "pid=,ppid="], text=True)
        self.child = next(int(line.split()[0]) for line in processes.splitlines()
                          if len(line.split()) == 2 and int(line.split()[1]) == self.process.pid)
        self.idle_kib = int(subprocess.check_output(["ps", "-o", "rss=", "-p", str(self.child)]))
        self.connection = http.client.HTTPConnection("127.0.0.1", self.port, timeout=180)

    def request(self, endpoint, body):
        start = time.perf_counter()
        self.connection.request("POST", endpoint, body, {"Content-Type": "application/json"})
        response = self.connection.getresponse()
        result = response.read()
        elapsed = (time.perf_counter() - start) * 1000
        assert response.status == 200, (self.name, endpoint, response.status, result[:500])
        return elapsed, json.loads(result)

    def stop(self):
        self.connection.close()
        os.kill(self.child, signal.SIGTERM)
        self.process.wait(timeout=10)
        self.log.close()
        resource = self.time_path.read_text()
        peak = re.search(r"(\d+)\s+maximum resident set size", resource)
        assert peak, resource
        return dict(idle_rss_mib=self.idle_kib / 1024,
                    peak_rss_mib=int(peak.group(1)) / 1024**2)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust", required=True, type=Path)
    parser.add_argument("--go", required=True, type=Path, help="compiled Go benchmark test host")
    parser.add_argument("--ratings-request", required=True, type=Path)
    parser.add_argument("--iterations", type=int, default=7)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    assert args.iterations > 0
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    env = {k: v for k, v in os.environ.items() if not k.startswith("RARE_POSITION_")}
    schema = "golaberto_rust_bench_" + uuid.uuid4().hex[:12]
    env.update(GOMAXPROCS="4", RARE_POSITION_MATCHED_POINT_POOL="1",
               RARE_POSITION_IMPORTANCE_SAMPLING="0", RARE_POSITION_RANDOM_SEED="808",
               RARE_POSITION_BENCHMARK_ITERATIONS="20000", RUST_ODDS_LOG="0",
               RUST_ODDS_PROFILE="0", TZ="UTC",
               DATABASE_URL=f"mysql://root@127.0.0.1:3306/{schema}",
               RUST_SERVICE_BENCHMARK_DSN=f"root@tcp(127.0.0.1:3306)/{schema}?parseTime=true")
    ratings = json.loads(args.ratings_request.read_bytes())
    evaluation = json.loads(args.ratings_request.read_bytes())
    # Evaluate the last 40 real matches, retaining all previous training games.
    # A synthetic phase tag selects only these matches without changing ratings.
    for index, game in enumerate(evaluation["games"]):
        game["phase_id"] = 2 if index >= len(evaluation["games"]) - 40 else 1
    evaluation["phases_to_eval"] = [2]
    inputs = Path(__file__).parent / "reference/2026-09-30-hundredfold/inputs"
    scenarios = [("odds-16653", "/odds", (inputs / "group-16653-71d4fea8.json").read_bytes()),
                 ("odds-16982", "/odds", (inputs / "group-16982-9327edcd.json").read_bytes()),
                 ("spi", "/spi", args.ratings_request.read_bytes()),
                 ("eval", "/eval", json.dumps(evaluation).encode()),
                 ("historic", "/historic_ratings", args.ratings_request.read_bytes())]
    summary = dict(iterations=args.iterations, warmups=2, cores=4, seed=808,
                   ratings_games=len(ratings["games"]), ratings_teams=len(ratings["ratings"]),
                   evaluation_matches=40, scenarios=[])
    mysql(f"CREATE DATABASE `{schema}`; CREATE TABLE `{schema}`.historical_ratings ("
          "team_id INT NOT NULL, off_rating FLOAT NOT NULL, def_rating FLOAT NOT NULL,"
          "rating FLOAT NOT NULL, measure_date DATE NOT NULL,"
          "UNIQUE KEY (team_id,measure_date)) ENGINE=InnoDB")
    try:
        for label, endpoint, body in scenarios:
            servers = {}
            times = {"go": [], "rust": []}
            resources = {}
            snapshots = {}
            responses = {}
            try:
                for name, binary in [("go", args.go), ("rust", args.rust)]:
                    servers[name] = Server(name, binary.resolve(), env, out, label)
                for index in range(args.iterations + 2):
                    for name in (["go", "rust"] if index % 2 == 0 else ["rust", "go"]):
                        elapsed, result = servers[name].request(endpoint, body)
                        if index >= 2:
                            times[name].append(elapsed)
                        if index == 0:
                            responses[name] = result
                            (out / f"{label}-{name}-response.json").write_text(json.dumps(result))
                            if endpoint == "/historic_ratings":
                                snapshots[name] = mysql(f"SELECT team_id,off_rating,def_rating,rating,"
                                    f"measure_date FROM `{schema}`.historical_ratings ORDER BY team_id,measure_date")
                if endpoint == "/odds":
                    # The initial game-importance scout can differ when tied
                    # teams are visited in Go map order. Compare position odds
                    # and the actual rare-position work, rather than requiring
                    # identical noisy game-importance values.
                    error = compare(responses["go"]["team_odds"], responses["rust"]["team_odds"])
                    for team, estimates in responses["go"]["rare_position_estimates"].items():
                        for rank, estimate in estimates.items():
                            other = responses["rust"]["rare_position_estimates"][team][rank]
                            for field in ("samples", "conditional_samples", "work_spent", "design", "reachability"):
                                assert estimate.get(field) == other.get(field), (label, team, rank, field)
                else:
                    error = compare(responses["go"], responses["rust"])
                assert error <= 1e-10, (label, error)
                if snapshots:
                    assert snapshots["go"] == snapshots["rust"], "stored historical rows differ"
            finally:
                for name, server in servers.items():
                    resources[name] = server.stop()
            row = dict(scenario=label, endpoint=endpoint, payload_bytes=len(body),
                       latency_ms=times, median_ms={k: statistics.median(v) for k, v in times.items()},
                       resources=resources, max_response_difference=error,
                       response_comparison="team_odds" if endpoint == "/odds" else "full_response")
            if snapshots:
                row["historical_rows"] = len(snapshots["go"].splitlines())
            summary["scenarios"].append(row)
            (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
            print(json.dumps(row), flush=True)
    finally:
        mysql(f"DROP DATABASE `{schema}`")


if __name__ == "__main__":
    main()
