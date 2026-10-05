#!/usr/bin/env python3
"""Run serialized, paired production/candidate odds estimates for R54."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import time

PRODUCTION_SHA256 = "010a91e9d5f9432f4ddfdd165d13df3bfefed6de502732aa0e3f99d9cc6fbd46"
REQUEST_SHA256 = "2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625"
DEFAULT_SEEDS = (808, 1669, 1993, 2281, 2293)
TRANSFER_FLAG = "RUST_ODDS_EXPERIMENT_NEIGHBOR_TRANSFER"
CELL_FLAG = "RUST_ODDS_EXPERIMENT_NEIGHBOR_CELL"
CELL_VALUE = "17:12"  # team ID:rank_1_based, resolved generically by candidate Model.indices


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def clean_env(extra: dict[str, str], log: bool) -> dict[str, str]:
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("RUST_ODDS_", "RARE_POSITION_"))}
    env["RUST_ODDS_LOG"] = "1" if log else "0"
    env.update(extra)
    return env


def parse_events(path: Path) -> list[dict]:
    events = []
    for line in path.read_text(errors="replace").splitlines():
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and isinstance(value.get("event"), str):
            events.append(value)
    return events


def run_one(binary: Path, request: Path, out_dir: Path, arm: str, transfer: str | None,
            seed: int, workers: int, logging: bool, label: str, cell_selector: str,
            baseline_flags: dict[str, str], candidate_flags: dict[str, str]) -> dict:
    stem = f"{label}-{arm}-seed{seed}-w{workers}"
    export_path = out_dir / f"{stem}.json"
    stdout_path = out_dir / f"{stem}.stdout.log"
    stderr_path = out_dir / f"{stem}.stderr.log"
    command = [str(binary), "estimate", str(request), str(export_path), str(seed), str(workers)]
    flags = dict(baseline_flags if arm == "baseline" else candidate_flags)
    if arm == "candidate":
        flags[TRANSFER_FLAG] = str(transfer)
        flags[CELL_FLAG] = cell_selector
    env = clean_env(flags, logging)
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    wall_start = time.perf_counter()
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        completed = subprocess.run(command, env=env, stdout=stdout, stderr=stderr, check=False)
    wall_seconds = time.perf_counter() - wall_start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    user_seconds = after.ru_utime - before.ru_utime
    sys_seconds = after.ru_stime - before.ru_stime
    events = parse_events(stderr_path)
    stages = {e.get("stage"): e.get("elapsed_ms") for e in events
              if e.get("event") == "rust_odds_stage" and isinstance(e.get("stage"), str)}
    complete = next((e for e in reversed(events) if e.get("event") == "rust_odds_complete"), None)
    status = "missing_export"
    export_sha = None
    if export_path.is_file():
        export_sha = sha256(export_path)
        status = "success" if completed.returncode == 0 else "process_error_with_export"
    record = {
        "name": stem, "label": label, "arm": arm, "transfer": transfer,
        "seed": seed, "workers": workers, "logging": logging,
        "binary": str(binary), "binary_sha256": sha256(binary),
        "request": str(request), "request_sha256": sha256(request),
        "argv": command, "explicit_environment": {**flags, "RUST_ODDS_LOG": "1" if logging else "0"},
        "scrubbed_environment_prefixes": ["RUST_ODDS_", "RARE_POSITION_"],
        "returncode": completed.returncode, "status": status,
        "export": export_path.name if export_sha else None,
        "export_sha256": export_sha,
        "stdout_log": stdout_path.name, "stdout_sha256": sha256(stdout_path),
        "stderr_log": stderr_path.name, "stderr_sha256": sha256(stderr_path),
        "wall_seconds": wall_seconds, "child_user_seconds": user_seconds,
        "child_system_seconds": sys_seconds, "child_cpu_seconds": user_seconds + sys_seconds,
        "stages_ms": stages, "complete_event": complete,
    }
    (out_dir / f"{stem}.command.json").write_text(json.dumps(record, indent=2) + "\n")
    if completed.returncode != 0:
        raise RuntimeError(f"{stem} exited {completed.returncode}; inspect {stderr_path}")
    if not export_sha:
        raise RuntimeError(f"{stem} produced no response; inspect {stderr_path}")
    return record


def parse_int_list(value: str) -> list[int]:
    try:
        result = [int(part) for part in value.split(",") if part.strip()]
    except ValueError as exc:
        raise argparse.ArgumentTypeError(str(exc)) from exc
    if not result or any(seed < 0 for seed in result):
        raise argparse.ArgumentTypeError("provide one or more nonnegative comma-separated seeds")
    return result


def read_env_json(path: Path | None) -> tuple[dict[str, str], dict | None]:
    if path is None:
        return {}, None
    resolved = path.resolve(strict=True)
    value = json.loads(resolved.read_text())
    if not isinstance(value, dict) or any(
        not isinstance(key, str) or not isinstance(item, str)
        for key, item in value.items()
    ):
        raise ValueError(f"{resolved} must contain a JSON object of string keys and values")
    if any(not key.startswith(("RUST_ODDS_EXPERIMENT_", "RARE_POSITION_")) for key in value):
        raise ValueError(f"{resolved} may only set experiment-prefixed environment variables")
    return value, {"path": str(resolved), "sha256": sha256(resolved), "values": value}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("production_binary", type=Path)
    parser.add_argument("candidate_binary", type=Path)
    parser.add_argument("request", type=Path)
    parser.add_argument("--baseline-binary", choices=("production", "candidate"), default="production",
                        help="use candidate to run a same-build native high-budget control cohort")
    parser.add_argument("--baseline-env-json", type=Path,
                        help="explicit baseline environment overrides as a JSON string map")
    parser.add_argument("--candidate-env-json", type=Path,
                        help="explicit candidate environment overrides as a JSON string map")
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--seeds", type=parse_int_list, default=list(DEFAULT_SEEDS))
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--transfers", default="better,worse,both")
    parser.add_argument("--cell-selector", default=CELL_VALUE, help="candidate selector TEAM_ID:RANK_1_BASED")
    parser.add_argument("--warmup-pairs", type=int, default=0)
    parser.add_argument("--warmup-seed", type=int, default=808)
    parser.add_argument("--repeat-pairs", type=int, default=0)
    parser.add_argument("--repeat-seed", type=int, default=808)
    parser.add_argument("--quiet-log", action="store_true", help="set RUST_ODDS_LOG=0")
    parser.add_argument("--execute", action="store_true", help="required to launch full production estimate processes")
    args = parser.parse_args()
    if not args.execute:
        parser.error("no jobs launched: pass --execute only after root releases the serialized heavy-run slot")
    if args.workers < 1 or args.workers > 4:
        parser.error("workers must be between 1 and 4")
    if args.warmup_pairs < 0 or args.repeat_pairs < 0:
        parser.error("pair counts cannot be negative")
    production = args.production_binary.resolve(strict=True)
    candidate = args.candidate_binary.resolve(strict=True)
    request = args.request.resolve(strict=True)
    if args.baseline_binary == "production" and sha256(production) != PRODUCTION_SHA256:
        parser.error("production binary SHA256 does not match frozen baseline")
    if sha256(request) != REQUEST_SHA256:
        parser.error("request SHA256 does not match saved group 16498 input")
    transfers = [x.strip() for x in args.transfers.split(",") if x.strip()]
    if not transfers or any(x not in {"better", "worse", "both"} for x in transfers):
        parser.error("transfers must be a comma-separated subset of better,worse,both")
    try:
        baseline_flags, baseline_env_source = read_env_json(args.baseline_env_json)
        candidate_flags, candidate_env_source = read_env_json(args.candidate_env_json)
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        parser.error(str(exc))
    out_dir = args.output_dir.resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    schedule = []
    for transfer in transfers:
        for i in range(args.warmup_pairs):
            schedule.append((f"warmup-{i+1:02d}", args.warmup_seed, transfer))
        for seed in args.seeds:
            schedule.append(("paired", seed, transfer))
        for i in range(args.repeat_pairs):
            schedule.append((f"repeat-{i+1:02d}", args.repeat_seed, transfer))
    run_records = []
    for pair_no, (label, seed, transfer) in enumerate(schedule):
        pair_dir = out_dir / f"{transfer}-{label}-seed{seed}"
        pair_dir.mkdir(parents=True, exist_ok=True)
        order = ("baseline", "candidate") if pair_no % 2 == 0 else ("candidate", "baseline")
        pair_records = {}
        for arm in order:
            binary = (production if args.baseline_binary == "production" else candidate) if arm == "baseline" else candidate
            record = run_one(binary, request, pair_dir, arm,
                             None if arm == "baseline" else transfer,
                             seed, args.workers, not args.quiet_log, label, args.cell_selector,
                             baseline_flags, candidate_flags)
            pair_records[arm] = record
        pair = {
            "pair_id": pair_dir.name, "label": label, "seed": seed, "transfer": transfer,
            "workers": args.workers, "arm_order": list(order), "runs": pair_records,
        }
        (pair_dir / "pair.json").write_text(json.dumps(pair, indent=2) + "\n")
        run_records.append(pair)
    summary = {
        "contract": "estimate REQUEST OUTPUT SEED WORKERS",
        "cohort": "production-vs-candidate" if args.baseline_binary == "production" else "candidate-control-vs-candidate-transfer",
        "baseline_binary_mode": args.baseline_binary,
        "production_sha256": sha256(production), "candidate_sha256": sha256(candidate),
        "request_sha256": sha256(request), "cell_selector": args.cell_selector,
        "baseline_env": baseline_env_source, "candidate_env": candidate_env_source,
        "transfer_modes": transfers, "workers": args.workers,
        "scrubbed_prefixes": ["RUST_ODDS_", "RARE_POSITION_"],
        "baseline_experiment_flags": baseline_flags,
        "candidate_flags": {**candidate_flags, TRANSFER_FLAG: "per-pair transfer", CELL_FLAG: args.cell_selector},
        "scheduled_pairs": len(schedule), "pairs": run_records,
    }
    (out_dir / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps({k: v for k, v in summary.items() if k != "pairs"}, indent=2))
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
