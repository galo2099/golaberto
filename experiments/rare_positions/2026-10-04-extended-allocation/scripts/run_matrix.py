#!/usr/bin/env python3
"""Run one reproducible branch-stratification row and archive its diagnostics."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import time


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--request", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--team", required=True, type=int)
    parser.add_argument("--rank", required=True, type=int)
    parser.add_argument("--draws", required=True, type=int)
    parser.add_argument("--seeds", required=True)
    parser.add_argument("--allocation", required=True, choices=("equal", "trained", "prior"))
    parser.add_argument("--guide", required=True, choices=("rank", "bounds", "adaptive"))
    parser.add_argument("--pilot-draws", required=True, type=int)
    parser.add_argument("--floor", required=True, type=int)
    parser.add_argument("--pilot-seed", type=int)
    parser.add_argument("--tree-leaves", type=int)
    parser.add_argument("--messages", type=int, default=0)
    parser.add_argument("--production-profile", action="store_true")
    parser.add_argument("--rank-priority", action="store_true", help="enable the generic tree rank-priority experiment control")
    parser.add_argument("--discovery", choices=("interval", "rank", "strict"), help="enable bounded structural message discovery")
    parser.add_argument("--refine", action="store_true")
    args = parser.parse_args()

    binary = args.binary.resolve(strict=True)
    request = args.request.resolve(strict=True)
    output = args.output.resolve()
    if output.exists() and any(output.iterdir()):
        parser.error("--output must be a new or empty directory")
    output.mkdir(parents=True, exist_ok=True)
    seeds = [int(value) for value in args.seeds.split(",")]
    if not seeds or args.rank < 1 or args.draws < 1 or args.pilot_draws < 2 or args.floor < 2:
        parser.error("seeds, rank, draws, pilot draws, and floor must be positive")
    if args.messages < 0:
        parser.error("--messages cannot be negative")

    child_env = os.environ.copy()
    cleared = sorted(key for key in child_env if key.startswith(("TREE_", "RUST_ODDS_", "RARE_POSITION_")))
    for key in cleared:
        child_env.pop(key, None)
    if args.pilot_seed is not None:
        child_env["TREE_PILOT_SEED"] = str(args.pilot_seed)
    if args.tree_leaves is not None:
        if args.tree_leaves < 1:
            parser.error("--tree-leaves must be positive")
        child_env.update({"TREE_MODE": "1", "TREE_LEAVES": str(args.tree_leaves), "TREE_TRAINING": "0", "TREE_GUIDE_VALUES": "4000000"})
    if args.messages:
        child_env["TREE_MESSAGES"] = str(args.messages)
    if args.production_profile:
        child_env["TREE_PRODUCTION_PROFILE"] = "1"
    if args.rank_priority:
        child_env["RUST_ODDS_EXPERIMENT_TREE_RANK_PRIORITY"] = "1"
    if args.discovery:
        child_env["RUST_ODDS_EXPERIMENT_TREE_DISCOVERY"] = args.discovery

    guide = args.guide
    command = [str(binary), str(request), str(args.team), str(args.rank), str(args.draws), args.seeds,
               args.allocation, guide, "4", "tilt", "refine" if args.refine else "plain",
               str(args.pilot_draws), str(args.floor), "off"]
    before_cpu = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.monotonic()
    process = subprocess.run(command, env=child_env, text=True, capture_output=True, check=False)
    wall_seconds = time.monotonic() - start
    after_cpu = resource.getrusage(resource.RUSAGE_CHILDREN)
    stdout_path = output / "stdout.jsonl"
    stderr_path = output / "stderr.txt"
    stdout_path.write_text(process.stdout, encoding="utf-8")
    stderr_path.write_text(process.stderr, encoding="utf-8")
    rows = []
    for line_number, line in enumerate(process.stdout.splitlines(), 1):
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError as exc:
            raise RuntimeError(f"non-JSON stdout at line {line_number}: {exc}") from exc
    results = [row for row in rows if row.get("event") == "strata_result"]
    summary = {
        "exit_code": process.returncode,
        "rows": len(results),
        "acceptance": [{"seed": row.get("seed"), "accepted": row.get("accepted"), "main_probability": row.get("main", {}).get("probability"), "check_probability": row.get("check", {}).get("probability")} for row in results],
    }
    (output / "summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    record = {
        "command": command,
        "arguments": {key: str(value) if isinstance(value, Path) else value for key, value in vars(args).items()},
        "environment_overrides": {key: child_env[key] for key in child_env if key.startswith(("TREE_", "RUST_ODDS_", "RARE_POSITION_"))},
        "cleared_inherited_environment_keys": cleared,
        "binary": str(binary), "binary_sha256": sha256(binary),
        "request": str(request), "request_sha256": sha256(request),
        "output_files": {path.name: sha256(path) for path in (stdout_path, stderr_path)},
        "wall_seconds": wall_seconds,
        "child_cpu_seconds": (after_cpu.ru_utime - before_cpu.ru_utime) + (after_cpu.ru_stime - before_cpu.ru_stime),
        "exit_code": process.returncode,
        "summary": summary,
    }
    (output / "record.json").write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(summary, sort_keys=True))
    return process.returncode


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"run_matrix: {error}", file=sys.stderr)
        raise
