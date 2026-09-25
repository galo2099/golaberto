#!/usr/bin/env python3
"""Run one reproducible rare-position hypothesis against a fixed baseline."""

import argparse
import json
import math
import os
import subprocess
import sys
import tempfile
from pathlib import Path


def run_command(command, cwd, env, log_path):
    with log_path.open("w") as log:
        result = subprocess.run(command, cwd=cwd, env=env, stdout=log, stderr=subprocess.STDOUT)
    return result.returncode


def commit_label(path):
    result = subprocess.run(["git", "rev-parse", "HEAD"], cwd=path, capture_output=True, text=True)
    return result.stdout.strip() if result.returncode == 0 else "source-snapshot"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--fixtures", required=True, help="comma-separated GroupType JSON paths")
    parser.add_argument("--reference-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--hypothesis", required=True)
    parser.add_argument("--seeds", type=int, default=5)
    parser.add_argument("--baseline-label")
    parser.add_argument("--candidate-label")
    args = parser.parse_args()
    if args.seeds < 5:
        parser.error("use at least five matched seeds")
    args.output.mkdir(parents=True, exist_ok=True)
    for name in ("baseline", "candidate"):
        path = args.output / name
        path.mkdir(exist_ok=True)
        if (path / "runs.jsonl").exists():
            parser.error(f"{path}/runs.jsonl already exists; use a new experiment directory")
    config = {
        "hypothesis": args.hypothesis,
        "baseline_source": str(args.baseline.resolve()),
        "candidate_source": str(args.candidate.resolve()),
        "fixtures": args.fixtures.split(","),
        "reference_dir": str(args.reference_dir.resolve()),
        "seeds": args.seeds,
        "baseline_commit": args.baseline_label or commit_label(args.baseline),
        "candidate_commit": args.candidate_label or commit_label(args.candidate),
        "decision": "inconclusive",
    }
    cache = str(Path(tempfile.gettempdir()) / "golaberto-go-cache")
    env = os.environ.copy()
    env.setdefault("GOCACHE", cache)
    env["RARE_POSITION_BENCHMARK_FIXTURES"] = args.fixtures
    env["RARE_POSITION_BENCHMARK_REFERENCE_DIR"] = str(args.reference_dir)
    env["RARE_POSITION_BENCHMARK_SEEDS"] = str(args.seeds)

    # This gate runs before any expensive arm. The normal Go suite skips the
    # opt-in benchmark tests, but covers likelihood, budget, and freeze rules.
    rc = run_command(["go", "test", "./go", "-count=1"], args.candidate, env, args.output / "unit-tests.log")
    if rc:
        config.update(decision="reject", reason="candidate unit tests failed")
    else:
        for name, source, label in (
            ("baseline", args.baseline, config["baseline_commit"]),
            ("candidate", args.candidate, config["candidate_commit"]),
        ):
            arm_env = env.copy()
            arm_env["RARE_POSITION_BENCHMARK_OUTPUT"] = str(args.output / name)
            arm_env["RARE_POSITION_BENCHMARK_COMMIT"] = label
            arm_env["RARE_POSITION_BENCHMARK_VARIANT"] = name
            rc = run_command(
                ["go", "test", "./go", "-run", "^TestDiversifiedMatchedWorkBenchmark$", "-count=1", "-timeout=60m"],
                source, arm_env, args.output / f"{name}.log"
            )
            if rc:
                log = (args.output / f"{name}.log").read_text()
                hard = "invalid estimate" in log or "work exceeded" in log
                config.update(decision="reject" if hard else "inconclusive", reason=f"{name} benchmark failed", hard_constraint=hard)
                break
        else:
            comparison = args.output / "comparison.json"
            rc = run_command(
                [sys.executable, str(Path(__file__).resolve().with_name("compare_rare_position_benchmarks.py")),
                 str(args.output / "baseline/runs.jsonl"), str(args.output / "candidate/runs.jsonl"),
                 "--hypothesis", args.hypothesis, "--output", str(comparison)],
                args.candidate, env, args.output / "compare.log"
            )
            if rc:
                config.update(decision="inconclusive", reason="paired comparison failed")
            else:
                report = json.loads(comparison.read_text())
                paired = report["paired"]["diversified"]
                config["aggregate_metrics"] = report["paired"]
                config["against_plain"] = report["against_plain"]
                score = paired["coverage_score"]
                ess10 = paired["ess_ge_10"]["mean"]
                ess25 = paired["ess_ge_25"]["mean"]
                vs_plain = report["against_plain"]["candidate"]
                if score["mean"] < 0 and ess10 <= 0:
                    config.update(decision="reject", reason="paired whole-table coverage worsened")
                elif score["mean"] <= score["stddev"] / math.sqrt(args.seeds * len(report["groups"])) and vs_plain["coverage_score"]["mean"] < 0:
                    config.update(decision="reject", reason="no measurable paired coverage gain and still below equal-work plain MC")
                elif args.seeds < 20:
                    config.update(decision="inconclusive", reason="promising changes require 20+ matched seeds")
                elif score["mean"] > 2 * score["stddev"] / (args.seeds * len(report["groups"])) ** .5 and ess10 >= 0 and ess25 >= 0 and vs_plain["coverage_score"]["mean"] > 0 and vs_plain["ess_ge_10"]["mean"] >= 0:
                    config.update(decision="accept", reason="paired coverage gain passed confirmation screen")
                else:
                    config.update(decision="inconclusive", reason="confirmation did not establish a whole-table gain")
    (args.output / "experiment.json").write_text(json.dumps(config, indent=2) + "\n")
    print(json.dumps({"decision": config["decision"], "reason": config.get("reason"), "record": str(args.output / "experiment.json")}))


if __name__ == "__main__":
    main()
