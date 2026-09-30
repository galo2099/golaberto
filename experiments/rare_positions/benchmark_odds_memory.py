#!/usr/bin/env python3
"""Paired Rust HTTP requests: identical outputs, elapsed time and peak process RSS."""
import argparse
import json
import os
from pathlib import Path
import statistics

from benchmark_rust_service import Server


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--input", action="append", type=Path)
    parser.add_argument("--seed", type=int, default=808)
    parser.add_argument("--iterations", type=int, default=7)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    assert args.iterations > 0
    inputs = args.input or sorted((Path(__file__).parent /
        "reference/2026-09-30-hundredfold/inputs").glob("*.json"))
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    env = {k: v for k, v in os.environ.items() if not k.startswith("RARE_POSITION_")}
    env.update(RUST_ODDS_LOG="0", RUST_ODDS_PROFILE="0", TZ="UTC",
               RARE_POSITION_RANDOM_SEED=str(args.seed))
    summary = dict(seed=args.seed, workers=4, warmups=2, iterations=args.iterations, scenarios=[])
    for path in inputs:
        label = path.stem
        body = path.read_bytes()
        servers = {}
        times = {"baseline": [], "candidate": []}
        resources = {}
        try:
            for name, binary in [("baseline", args.baseline), ("candidate", args.candidate)]:
                servers[name] = Server(name, binary.resolve(), env, out, label)
            for index in range(args.iterations + 2):
                results = {}
                for name in (["baseline", "candidate"] if index % 2 == 0 else ["candidate", "baseline"]):
                    elapsed, result = servers[name].request("/odds", body)
                    results[name] = result
                    if index >= 2:
                        times[name].append(elapsed)
                # Covers every probability, uncertainty, proof and work counter,
                # including the original game-importance scout.
                assert results["baseline"] == results["candidate"], (label, args.seed, index)
        finally:
            for name, server in servers.items():
                resources[name] = server.stop()
        row = dict(input=path.name, identical=True, latency_ms=times,
                   median_ms={k: statistics.median(v) for k, v in times.items()}, resources=resources)
        summary["scenarios"].append(row)
        (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
        print(json.dumps(row), flush=True)


if __name__ == "__main__":
    main()
