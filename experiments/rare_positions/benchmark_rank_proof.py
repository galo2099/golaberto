#!/usr/bin/env python3
"""Paired persistent-server checks of the generic explained rank proof."""
import argparse
import json
import os
from pathlib import Path
import re
import statistics

from benchmark_rust_service import Server
from compare_rust_reachability import states


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--iterations", type=int, default=8)
    parser.add_argument("--seed", type=int, default=808)
    parser.add_argument("--input", action="append", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    assert args.iterations > 0
    inputs = args.input or sorted((Path(__file__).parent /
        "reference/2026-09-30-hundredfold/inputs").glob("*.json"))
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("RARE_POSITION_", "RUST_ODDS_"))}
    env.update(RUST_ODDS_LOG="1", RUST_ODDS_PROFILE="0", TZ="UTC",
               RARE_POSITION_RANDOM_SEED=str(args.seed))
    summary = dict(seed=args.seed, workers=4, warmups=2, iterations=args.iterations,
                   baseline_flags={"RUST_ODDS_RANK_PROOF": "0"},
                   candidate_flags={}, scenarios=[])
    for path in inputs:
        label = path.stem
        servers, resources, responses = {}, {}, {}
        times = {"baseline": [], "candidate": []}
        try:
            for name, binary in [("baseline", args.baseline), ("candidate", args.candidate)]:
                flags = dict(env)
                if name == "baseline":
                    flags["RUST_ODDS_RANK_PROOF"] = "0"
                servers[name] = Server(name, binary.resolve(), flags, out, label)
            for index in range(args.iterations + 2):
                results = {}
                for name in (["baseline", "candidate"] if index % 2 == 0
                             else ["candidate", "baseline"]):
                    elapsed, result = servers[name].request("/odds", path.read_bytes())
                    results[name] = result
                    if index >= 2:
                        times[name].append(elapsed)
                a, b = results["baseline"], results["candidate"]
                assert a["team_odds"] == b["team_odds"], (label, "odds changed")
                assert a["game_importance"] == b["game_importance"], (label, "scout changed")
                sa, sb = states(a), states(b)
                for cell, previous in sa.items():
                    if previous in ("positive", "reachable_zero"):
                        assert sb[cell] in ("positive", "reachable_zero"), (label, cell, previous, sb[cell])
                    if previous == "impossible":
                        assert sb[cell] == "impossible", (label, cell, previous, sb[cell])
                for team, estimates in a["rare_position_estimates"].items():
                    for rank, estimate in estimates.items():
                        other = b["rare_position_estimates"][team][rank]
                        for field in ("probability", "std_err"):
                            assert estimate[field] == other[field], (label, team, rank, field)
                if index == 0:
                    responses = results
                    for name, response in results.items():
                        (out / f"{label}-{name}-response.json").write_text(json.dumps(response))
        finally:
            for name, server in servers.items():
                resources[name] = server.stop()
        proof_times = {}
        for name in resources:
            raw = (out / f"{label}-{name}-resource.txt").read_text()
            cpu = re.search(r"([\d.]+)\s+user\s+([\d.]+)\s+sys", raw)
            assert cpu, raw
            resources[name]["cpu_seconds"] = sum(map(float, cpu.groups()))
            events = [json.loads(line) for line in
                      (out / f"{label}-{name}.log").read_text().splitlines()
                      if line.startswith("{")]
            proof_times[name] = [e["elapsed_ms"] for e in events
                                 if e.get("stage") == "search.early_proofs"]
        a, b = states(responses["baseline"]), states(responses["candidate"])
        row = dict(input=path.name, latency_ms=times,
                   median_ms={k: statistics.median(v) for k, v in times.items()},
                   resources=resources, proof_stage_ms=proof_times,
                   probabilities_and_uncertainties_identical=True,
                   additional_impossible=[dict(team=t, position=p) for (t, p), before in a.items()
                                          if before != "impossible" and b[t, p] == "impossible"],
                   undecided={"baseline": sum(v == "undecided" for v in a.values()),
                              "candidate": sum(v == "undecided" for v in b.values())})
        row["median_change_percent"] = 100 * (row["median_ms"]["candidate"] /
                                               row["median_ms"]["baseline"] - 1)
        summary["scenarios"].append(row)
        (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
        print(json.dumps(row), flush=True)


if __name__ == "__main__":
    main()
