#!/usr/bin/env python3
"""Run the authorized R58 V4 controls serially with four sampler workers."""
from __future__ import annotations

import hashlib
import json
import os
import resource
import subprocess
import sys
import time
from pathlib import Path


def sha(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def run(binary: Path, request: Path, root: Path, seed: int, arm: str, flags: dict[str, str]) -> dict:
    out = root / arm / f"seed{seed}"
    out.mkdir(parents=True)
    export = out / "export.json"
    stdout = out / "stdout.log"
    stderr = out / "stderr.log"
    env = {k: v for k, v in os.environ.items() if not k.startswith(("RUST_ODDS_", "RARE_POSITION_"))}
    env.update(flags)
    env["RUST_ODDS_LOG"] = "1"
    argv = [str(binary), "estimate", str(request), str(export), str(seed), "4"]
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    started = time.perf_counter()
    with stdout.open("wb") as out_stream, stderr.open("wb") as err_stream:
        code = subprocess.run(argv, env=env, stdout=out_stream, stderr=err_stream).returncode
    wall = time.perf_counter() - started
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    if code or not export.is_file():
        raise RuntimeError(f"{arm}/seed{seed} failed; inspect {stderr}")
    events = []
    for line in stderr.read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(event, dict) and isinstance(event.get("event"), str):
            events.append(event)
    stages = {e.get("stage"): e.get("elapsed_ms") for e in events if e.get("event") == "rust_odds_stage"}
    fallback = [e for e in events if e.get("event") == "rust_odds_family_fallback"]
    summary = [e for e in events if e.get("event") == "rust_odds_rare_tail_confirm_more_summary"]
    return {
        "arm": arm,
        "seed": seed,
        "workers": 4,
        "binary": str(binary),
        "binary_sha256": sha(binary),
        "request": str(request),
        "request_sha256": sha(request),
        "argv": argv,
        "explicit_environment": {**flags, "RUST_ODDS_LOG": "1"},
        "returncode": code,
        "export": str(export),
        "export_sha256": sha(export),
        "stdout": str(stdout),
        "stdout_sha256": sha(stdout),
        "stderr": str(stderr),
        "stderr_sha256": sha(stderr),
        "wall_seconds": wall,
        "child_user_seconds": after.ru_utime - before.ru_utime,
        "child_system_seconds": after.ru_stime - before.ru_stime,
        "child_cpu_seconds": after.ru_utime - before.ru_utime + after.ru_stime - before.ru_stime,
        "stages_ms": stages,
        "fallback_events": fallback,
        "confirmation_summary_events": summary,
    }


def main() -> None:
    if len(sys.argv) != 4:
        raise SystemExit("usage: run_v4_cohort.py V4_BINARY REQUEST V3_LOG_ROOT")
    binary, request, v3_root = (Path(value).resolve(strict=True) for value in sys.argv[1:])
    root = Path(__file__).resolve().parents[1] / "logs" / "v4-cohort"
    root.mkdir(parents=True, exist_ok=False)
    selector = {
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK": "1",
        "RUST_ODDS_EXPERIMENT_NEIGHBOR_TRANSFER": "native",
        "RUST_ODDS_EXPERIMENT_NEIGHBOR_CELL": "17:12",
    }
    seeds = [808, 1669, 1993, 2281, 2293]
    records: list[dict] = []
    schedule = []
    for seed in seeds:
        for multiplier in (1, 10):
            schedule.append((seed, f"native-retry-{multiplier}x", {**selector,
                "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE": "native_retry",
                "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MULTIPLIER": str(multiplier)}))
        schedule.append((seed, "family-10x-cap5000", {**selector,
            "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MULTIPLIER": "10",
            "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP": "5000"}))
        schedule.append((seed, "family-10x-default50k", {**selector,
            "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MULTIPLIER": "10"}))
    for seed in (808, 2293):
        schedule.append((seed, "family-1x", selector))
    for seed, arm, flags in schedule:
        record = run(binary, request, root, seed, arm, flags)
        records.append(record)
        (root / arm / f"seed{seed}" / "record.json").write_text(json.dumps(record, indent=2) + "\n")
        if arm == "family-1x":
            old = v3_root / f"seed{seed}" / "r58-fallback.json"
            if (root / arm / f"seed{seed}" / "export.json").read_bytes() != old.read_bytes():
                raise RuntimeError(f"V4 family 1x differs from frozen V3 export for seed {seed}")
    (root / "summary.json").write_text(json.dumps({
        "seeds": seeds,
        "serialized_processes": True,
        "workers": 4,
        "schedule": [f"{arm}/seed{seed}" for seed, arm, _ in schedule],
        "family_1x_exact_v3_export_parity_seeds": [808, 2293],
        "records": records,
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
