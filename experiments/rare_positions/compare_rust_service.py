#!/usr/bin/env python3
"""Archived differential checks using a historical Go ratings oracle binary.

The historical Go source is no longer in this checkout; build the oracle from
a historical checkout. Go writes use temporary tables only.
"""
import argparse
import http.client
import json
import os
from pathlib import Path
import socket
import statistics
import subprocess
import time


def fixture():
    return {
        "games": [dict(home_id=1 + i % 3, away_id=1 + (i + 1) % 3,
                       phase_id=1 if i < 300 else 2, home_score=float(i % 5),
                       away_score=float(i * 7 % 4), timestamp=1500000000 + i * 5 * 86400,
                       length=4 / 3 if i % 17 == 0 else 1,
                       advantage=[0, .16133676871779334, -.16133676871779334][i % 3])
                  for i in range(360)],
        "ratings": [dict(id=i, offense=None if i % 2 else 1.4,
                         defense=None if i % 2 else 1.2) for i in range(1, 5)],
        "phases_to_eval": [2],
    }


def compare(a, b):
    if isinstance(a, dict):
        assert a.keys() == b.keys()
        return max((compare(a[k], b[k]) for k in a), default=0)
    if isinstance(a, list):
        assert len(a) == len(b)
        return max((compare(x, y) for x, y in zip(a, b)), default=0)
    if isinstance(a, (float, int)):
        return abs(a - b)
    assert a == b, (a, b)
    return 0


def run(command, env, log):
    start = time.perf_counter()
    result = subprocess.run(command, env=env, capture_output=True, text=True,
                            timeout=300, check=True)
    log.write_text(result.stdout + result.stderr)
    return time.perf_counter() - start, result.stderr


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust", required=True, type=Path)
    parser.add_argument("--go", required=True, type=Path, help="compiled Go test oracle")
    parser.add_argument("--baseline", type=Path, help="Rust binary from the starting revision")
    parser.add_argument("--db-request", type=Path, help="optional read-only exported SPI request")
    parser.add_argument("--iterations", type=int, default=3)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    assert args.iterations > 0
    args.rust = args.rust.resolve()
    args.go = args.go.resolve()
    if args.baseline:
        args.baseline = args.baseline.resolve()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    env = {k: v for k, v in os.environ.items() if not k.startswith("RARE_POSITION_")}
    env.update(RUST_ODDS_LOG="0", RUST_ODDS_PROFILE="0", TZ="UTC")
    request = out / "synthetic.json"
    request.write_text(json.dumps(fixture()))
    summary = {"ratings": {}, "odds": []}
    for label, path, modes in [("synthetic", request, ["spi", "eval", "historic"])] + (
            [("database_export", args.db_request.resolve(), ["spi"])] if args.db_request else []):
        for mode in modes:
            outputs = {}
            times = {}
            for name in ["go", "rust"]:
                output = out / f"{label}-{name}-{mode}.json"
                if name == "go":
                    command = [str(args.go), "-test.run", "^TestRustRatingsOracle$"]
                    extra = dict(RUST_RATINGS_ORACLE_INPUT=str(path),
                                 RUST_RATINGS_ORACLE_OUTPUT=str(output),
                                 RUST_RATINGS_ORACLE_MODE=mode)
                else:
                    command = [str(args.rust), mode, str(path), str(output)]
                    extra = {}
                times[name], _ = run(command, dict(env, **extra),
                                     out / f"{label}-{name}-{mode}.log")
                outputs[name] = json.loads(output.read_text())
                if mode == "historic":
                    outputs[name].sort(key=lambda r: (r["team_id"], r["measure_date"]))
            error = compare(outputs["go"], outputs["rust"])
            assert error <= (0.500001e-6 if mode == "historic" else 1e-10), error
            summary["ratings"][f"{label}-{mode}"] = dict(max_difference=error, process_seconds=times)

    if args.baseline:
        inputs = Path(__file__).parent / "reference/2026-09-30-hundredfold/inputs"
        for path in sorted(inputs.glob("*.json")):
            timings = {"baseline": [], "replacement": []}
            for i in range(args.iterations):
                responses = {}
                names = ["baseline", "replacement"] if i % 2 == 0 else ["replacement", "baseline"]
                for name in names:
                    binary = args.baseline if name == "baseline" else args.rust
                    output = out / f"{path.stem}-{name}-{i}.json"
                    _, stderr = run([str(binary), "estimate", str(path), str(output), "808", "4"],
                                    env, out / f"{path.stem}-{name}-{i}.log")
                    timings[name].append(json.loads(stderr.strip().splitlines()[-1])["total_ms"])
                    responses[name] = json.loads(output.read_text())
                assert responses["baseline"] == responses["replacement"], path
            summary["odds"].append(dict(input=path.name, identical=True,
                timings_ms=timings, median_ms={k: statistics.median(v) for k, v in timings.items()}))

    # Check the full-sized HTTP odds contract against CLI output from the same binary.
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    server = subprocess.Popen([str(args.rust), "serve", f"127.0.0.1:{port}"],
                              env=dict(env, RARE_POSITION_RANDOM_SEED="808"),
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        deadline = time.monotonic() + 10
        while True:
            try:
                with socket.create_connection(("127.0.0.1", port), timeout=.1):
                    break
            except OSError:
                assert time.monotonic() < deadline
                time.sleep(.01)
        path = Path(__file__).parent / "reference/2026-09-30-hundredfold/inputs/group-16653-71d4fea8.json"
        cli = out / "http-odds-cli.json"
        run([str(args.rust), "estimate", str(path), str(cli), "808", "4"], env, out / "http-odds-cli.log")
        conn = http.client.HTTPConnection("127.0.0.1", port, timeout=30)
        start = time.perf_counter()
        conn.request("POST", "/odds", path.read_bytes(), {"Content-Type": "application/json"})
        response = conn.getresponse()
        assert response.status == 200
        body = response.read()
        assert json.loads(body) == json.loads(cli.read_text())
        summary["http_odds"] = dict(identical_to_cli=True, wall_ms=(time.perf_counter()-start)*1000)
        conn.close()
    finally:
        server.terminate()
        server.wait(timeout=10)
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
