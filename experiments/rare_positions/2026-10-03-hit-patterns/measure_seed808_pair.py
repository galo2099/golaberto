#!/usr/bin/env python3
"""Run one serialized observer-off/on seed 808 timing pair and record child CPU."""
import argparse
import hashlib
import json
import os
import resource
import subprocess
import time
from pathlib import Path


FLAGS = {
    "RUST_ODDS_EXPERIMENT_NEIGHBOR_TRANSFER": "native",
    "RUST_ODDS_EXPERIMENT_NEIGHBOR_CELL": "17:12",
    "RUST_ODDS_EXPERIMENT_NEIGHBOR_BUDGET_MULTIPLIER": "10",
}


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--request", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--audit-dir", required=True, type=Path)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    args.audit_dir.mkdir(parents=True, exist_ok=True)
    results = []
    for arm in ("observer-off", "observer-on"):
        output = args.output_dir / f"seed808-timed-{arm}.json"
        stderr = args.output_dir / f"seed808-timed-{arm}.stderr"
        env = {key: value for key, value in os.environ.items()
               if not key.startswith(("RUST_ODDS_", "RARE_POSITION_"))}
        env.update(FLAGS)
        if arm == "observer-on":
            env["RUST_ODDS_LAZY_HIT_AUDIT_CELL"] = "17:12"
            env["RUST_ODDS_LAZY_HIT_AUDIT_DIR"] = str(args.audit_dir.resolve())
        command = [str(args.binary.resolve()), "estimate", str(args.request.resolve()),
                   str(output.resolve()), "808", "4"]
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        started = time.monotonic()
        with stderr.open("wb") as log:
            subprocess.run(command, env=env, cwd=args.binary.resolve().parent.parent,
                           stdout=subprocess.DEVNULL, stderr=log, check=True)
        wall = time.monotonic() - started
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        results.append({"arm": arm, "command": command, "explicit_environment":
            {**FLAGS, **({"RUST_ODDS_LAZY_HIT_AUDIT_CELL": "17:12",
                          "RUST_ODDS_LAZY_HIT_AUDIT_DIR": str(args.audit_dir.resolve())}
                         if arm == "observer-on" else {})},
            "wall_seconds": wall, "child_user_seconds": after.ru_utime - before.ru_utime,
            "child_system_seconds": after.ru_stime - before.ru_stime,
            "export": str(output.resolve()), "export_sha256": sha256(output),
            "stderr": str(stderr.resolve()), "stderr_sha256": sha256(stderr)})
    if results[0]["export_sha256"] != results[1]["export_sha256"]:
        raise SystemExit("observer changed the seed 808 result export")
    (args.output_dir / "seed808-pair-resource-usage.json").write_text(
        json.dumps({"workers": 4, "sequential": True, "results": results}, indent=2) + "\n")
    print(json.dumps({"results": results}, indent=2))


if __name__ == "__main__":
    main()
