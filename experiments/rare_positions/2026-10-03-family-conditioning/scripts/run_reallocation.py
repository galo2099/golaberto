#!/usr/bin/env python3
"""Run matched native/family screens with a shared 5,000-draw pilot."""
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


def run(binary: Path, request: Path, outdir: Path, seed: int, family: bool) -> dict:
    name = "family-5000-h08" if family else "native-5000-v1headroom"
    export, stdout_path, stderr_path = [outdir / f"{name}.{suffix}"
                                        for suffix in ("json", "stdout.log", "stderr.log")]
    flags = {
        "RUST_ODDS_EXPERIMENT_NEIGHBOR_TRANSFER": "native",
        "RUST_ODDS_EXPERIMENT_NEIGHBOR_CELL": "17:12",
        "RUST_ODDS_EXPERIMENT_NEIGHBOR_BUDGET_MULTIPLIER": "1",
        "RUST_ODDS_EXPERIMENT_FAMILY_PILOT_DRAWS": "5000",
    }
    if family:
        flags.update({
            "RUST_ODDS_EXPERIMENT_FAMILY": "1",
            "RUST_ODDS_EXPERIMENT_FAMILY_HEADROOM": "0.08",
        })
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(("RUST_ODDS_", "RARE_POSITION_"))}
    env.update(flags)
    env["RUST_ODDS_LOG"] = "1"
    command = [str(binary), "estimate", str(request), str(export), str(seed), "4"]
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    started = time.perf_counter()
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        result = subprocess.run(command, env=env, stdout=stdout, stderr=stderr)
    wall = time.perf_counter() - started
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    if result.returncode != 0 or not export.is_file():
        raise RuntimeError(f"{name}/seed{seed} failed; inspect {stderr_path}")
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
        "name": name, "seed": seed, "family": family, "workers": 4,
        "binary": str(binary), "binary_sha256": sha256(binary),
        "request": str(request), "request_sha256": sha256(request),
        "argv": command, "explicit_environment": {**flags, "RUST_ODDS_LOG": "1"},
        "returncode": result.returncode, "export": export.name,
        "export_sha256": sha256(export), "stdout_log": stdout_path.name,
        "stdout_sha256": sha256(stdout_path), "stderr_log": stderr_path.name,
        "stderr_sha256": sha256(stderr_path), "wall_seconds": wall,
        "child_user_seconds": after.ru_utime - before.ru_utime,
        "child_system_seconds": after.ru_stime - before.ru_stime,
        "child_cpu_seconds": ((after.ru_utime - before.ru_utime)
                              + (after.ru_stime - before.ru_stime)),
        "effective_average_cpu_cores": (((after.ru_utime - before.ru_utime)
                                         + (after.ru_stime - before.ru_stime)) / wall
                                        if wall else None),
        "stages_ms": stages,
    }
    (outdir / f"{name}.command.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


def main() -> int:
    if len(sys.argv) not in (3, 4):
        raise SystemExit("usage: run_reallocation.py R57_REALLOCATED_BINARY REQUEST [SEEDS]")
    binary, request = (Path(value).resolve(strict=True) for value in sys.argv[1:3])
    seeds = [int(value) for value in sys.argv[3].split(",")] if len(sys.argv) == 4 else [808]
    root = Path(__file__).resolve().parents[1] / "logs" / "reallocation" / "cohort"
    for seed in seeds:
        outdir = root / f"seed{seed}"
        outdir.mkdir(parents=True, exist_ok=True)
        records = [run(binary, request, outdir, seed, family=False),
                   run(binary, request, outdir, seed, family=True)]
        (outdir / "summary.json").write_text(json.dumps({
            "seed": seed, "workers": 4, "request_sha256": sha256(request),
            "records": records,
        }, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
