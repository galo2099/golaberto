#!/usr/bin/env python3
"""Run serialized current-source and family-conditioning screens."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import time


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run(binary: Path, request: Path, outdir: Path, name: str,
        flags: dict[str, str], seed: int) -> dict:
    export = outdir / f"{name}.json"
    stdout_path = outdir / f"{name}.stdout.log"
    stderr_path = outdir / f"{name}.stderr.log"
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(("RUST_ODDS_", "RARE_POSITION_"))}
    env.update(flags)
    env["RUST_ODDS_LOG"] = "1"
    command = [str(binary), "estimate", str(request), str(export), str(seed), "4"]
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.perf_counter()
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        result = subprocess.run(command, env=env, stdout=stdout, stderr=stderr)
    elapsed = time.perf_counter() - start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    if result.returncode or not export.is_file():
        raise RuntimeError(f"{name} failed ({result.returncode}); inspect {stderr_path}")
    events = []
    for line in stderr_path.read_text(errors="replace").splitlines():
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and isinstance(value.get("event"), str):
            events.append(value)
    stages = {event.get("stage"): event.get("elapsed_ms") for event in events
              if event.get("event") == "rust_odds_stage"}
    record = {
        "name": name, "seed": seed, "binary": str(binary), "binary_sha256": sha256(binary),
        "request": str(request), "request_sha256": sha256(request),
        "argv": command, "explicit_environment": {**flags, "RUST_ODDS_LOG": "1"},
        "returncode": result.returncode, "export": export.name,
        "export_sha256": sha256(export), "stdout_log": stdout_path.name,
        "stdout_sha256": sha256(stdout_path), "stderr_log": stderr_path.name,
        "stderr_sha256": sha256(stderr_path), "wall_seconds": elapsed,
        "child_user_seconds": after.ru_utime - before.ru_utime,
        "child_system_seconds": after.ru_stime - before.ru_stime,
        "child_cpu_seconds": (after.ru_utime - before.ru_utime)
        + (after.ru_stime - before.ru_stime),
        "effective_average_cpu_cores": ((after.ru_utime - before.ru_utime)
        + (after.ru_stime - before.ru_stime)) / elapsed if elapsed else None,
        "stages_ms": stages,
    }
    (outdir / f"{name}.command.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


def main() -> int:
    if len(sys.argv) not in (4, 5):
        raise SystemExit("usage: run_seed808_screen.py CURRENT_BASELINE R57_BINARY REQUEST [SEEDS]")
    baseline, candidate, request = (Path(value).resolve(strict=True) for value in sys.argv[1:4])
    seeds = [int(value) for value in sys.argv[4].split(",")] if len(sys.argv) == 5 else [808]
    common = {
        "RUST_ODDS_EXPERIMENT_NEIGHBOR_TRANSFER": "native",
        "RUST_ODDS_EXPERIMENT_NEIGHBOR_CELL": "17:12",
    }
    root = Path(__file__).resolve().parents[1] / "logs"
    for seed in seeds:
        outdir = root / "cohort" / f"seed{seed}"
        outdir.mkdir(parents=True, exist_ok=True)
        baseline_record = run(baseline, request, outdir, "current-source-baseline", {}, seed)
        records = [baseline_record]
        if seed == 808:
            flagoff_record = run(candidate, request, outdir, "r57-flag-off", {}, seed)
            records.append(flagoff_record)
            parity = baseline_record["export_sha256"] == flagoff_record["export_sha256"]
            (outdir / "flag-off-parity.json").write_text(json.dumps({
                "current_source_baseline": baseline_record["export_sha256"],
                "r57_flag_off": flagoff_record["export_sha256"],
                "byte_identical": parity,
            }, indent=2) + "\n")
            if not parity:
                raise RuntimeError("flag-off full-request exports differ; inspect saved outputs")
        for multiplier, suffix in ((1, "1x"), (10, "10x")):
            native = {**common,
                      "RUST_ODDS_EXPERIMENT_NEIGHBOR_BUDGET_MULTIPLIER": str(multiplier)}
            records.append(run(candidate, request, outdir, f"r57-native-{suffix}", native, seed))
            family = {**native, "RUST_ODDS_EXPERIMENT_FAMILY": "1"}
            records.append(run(candidate, request, outdir, f"r57-family-{suffix}", family, seed))
        summary = {
            "seed": seed, "workers": 4, "schedule": [item["name"] for item in records],
            "request_sha256": sha256(request), "records": records,
        }
        (outdir / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
