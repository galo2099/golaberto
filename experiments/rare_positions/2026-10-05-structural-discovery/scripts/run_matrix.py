#!/usr/bin/env python3
"""Run a serialized, resumable R68 full-request arm matrix.

Example:
  python3 scripts/run_matrix.py --candidate-binary /tmp/golaberto-odds-r68 \
    --frozen-production-binary /tmp/golaberto-odds-prod \
    --out-dir logs/r68 --fixtures original-16498,updated-16498 \
    --seeds 808,1669,1993 --arms control,tree-rank,family-skeleton,combined-rank-skeleton

All runs use four workers. A non-empty output directory is accepted only with
--resume and a matching run manifest. Completed records are reused only after
their binary, request, export and log hashes are rechecked.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import resource
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
REFERENCE = ROOT / "experiments/rare_positions/reference/2026-09-30-hundredfold/inputs"
UPDATED_16498 = ROOT / "experiments/rare_positions/reference/2026-10-04-two-results/inputs/group-16498-two-results.json"
HISTORICAL_16653 = ROOT / "odds-rust/tests/fixtures/group-16653-2d1c1d6f.json"
CLEAR_PREFIXES = ("TREE_", "RUST_ODDS_", "RARE_POSITION_")

FIXTURES = {
    "original-16498": REFERENCE / "group-16498-44eabb47.json",
    "updated-16498": UPDATED_16498,
    "ref-15902": REFERENCE / "group-15902-31d66705.json",
    "ref-16413": REFERENCE / "group-16413-a2eef0a8.json",
    "ref-16653-current": REFERENCE / "group-16653-71d4fea8.json",
    "ref-16982": REFERENCE / "group-16982-9327edcd.json",
    "ref-16983": REFERENCE / "group-16983-43969b02.json",
    "historical-16653": HISTORICAL_16653,
}
DEFAULT_FIXTURES = list(FIXTURES)

ARMS = {
    "control": {},
    "frozen": {},
    "tree-interval": {"RUST_ODDS_EXPERIMENT_TREE_DISCOVERY": "interval"},
    "tree-rank": {"RUST_ODDS_EXPERIMENT_TREE_DISCOVERY": "rank"},
    "tree-strict": {"RUST_ODDS_EXPERIMENT_TREE_DISCOVERY": "strict"},
    "family-ties": {"RUST_ODDS_EXPERIMENT_FAMILY_STRUCTURE": "ties"},
    "family-skeleton": {"RUST_ODDS_EXPERIMENT_FAMILY_STRUCTURE": "skeleton"},
    "family-relax": {"RUST_ODDS_EXPERIMENT_FAMILY_STRUCTURE": "relax"},
    "combined-rank-skeleton": {
        "RUST_ODDS_EXPERIMENT_TREE_DISCOVERY": "rank",
        "RUST_ODDS_EXPERIMENT_FAMILY_STRUCTURE": "skeleton",
    },
    "combined-strict-relax": {
        "RUST_ODDS_EXPERIMENT_TREE_DISCOVERY": "strict",
        "RUST_ODDS_EXPERIMENT_FAMILY_STRUCTURE": "relax",
    },
}
DEFAULT_ARMS = [name for name in ARMS if name != "frozen"]
SUMMARY_EVENTS = ("summary", "fallback", "branches", "overflow_tree")
EVENT_FIELDS = {
    "event", "group", "team", "team_id", "rank", "rank_index", "rank_index_zero_based",
    "position", "status", "reason", "mode", "stage",
    "work", "work_limit", "reserved_work", "actual", "grant", "within_grant",
    "settlement_within_grant", "charged_work", "accepted", "probability",
    "main_probability", "check_probability", "work_spent", "family_structure",
    "selection_work", "selection_allowance", "message_work", "overflow_work",
    "main", "check", "fallback", "final", "stages", "stage_rows",
    "setup_stages", "sampling_stages", "rows",
    "overflow", "setup", "sampling",
}
METRIC_FIELDS = {
    "probability", "std_err", "samples", "hits", "ess", "max_share",
    "batch_gap", "work", "work_spent", "operations", "weighted", "available",
    "accepted", "check_samples", "check_hits", "check_ess", "check_probability",
}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def stable_hash(value: object) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(payload).hexdigest()


def csv_values(value: str, choices: set[str] | None = None) -> list[str]:
    values = [part.strip() for part in value.split(",") if part.strip()]
    if not values or len(values) != len(set(values)):
        raise argparse.ArgumentTypeError("expected a non-empty comma-separated list without duplicates")
    if choices is not None:
        unknown = sorted(set(values) - choices)
        if unknown:
            raise argparse.ArgumentTypeError(f"unknown value(s): {', '.join(unknown)}")
    return values


def int_values(value: str) -> list[int]:
    try:
        values = [int(part.strip()) for part in value.split(",") if part.strip()]
    except ValueError as exc:
        raise argparse.ArgumentTypeError("expected comma-separated integers") from exc
    if not values or len(values) != len(set(values)):
        raise argparse.ArgumentTypeError("expected a non-empty comma-separated list without duplicates")
    return values


def fixture_path(name: str) -> Path:
    try:
        path = FIXTURES[name].resolve(strict=True)
    except KeyError as exc:
        raise ValueError(f"unknown fixture {name!r}; choose from {', '.join(FIXTURES)}") from exc
    request = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(request, dict) or not isinstance(request.get("id"), int):
        raise ValueError(f"fixture lacks integer request id: {path}")
    return path


def event_rows(*paths: Path) -> list[dict]:
    rows = []
    for path in paths:
        for line in path.read_text(errors="replace").splitlines():
            try:
                item = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(item, dict) and isinstance(item.get("event"), str):
                rows.append(item)
    return rows


def compact(value: object, depth: int = 0) -> object:
    if depth > 6:
        return "<nested>"
    if isinstance(value, dict):
        output = {}
        for key, item in value.items():
            if key in EVENT_FIELDS or key in METRIC_FIELDS or key in {
                "main", "check", "summary", "settings", "fallback", "final",
                "stages", "stage_rows", "setup_stages", "sampling_stages", "rows",
                "overflow", "setup", "sampling",
            }:
                output[key] = compact(item, depth + 1)
            elif isinstance(item, dict) and any(k in METRIC_FIELDS for k in item):
                output[key] = compact(item, depth + 1)
        return output
    if isinstance(value, list):
        if len(value) > 64:
            return {"count": len(value), "sample": [compact(x, depth + 1) for x in value[:3]]}
        return [compact(x, depth + 1) for x in value]
    return value


def summary_events(rows: list[dict]) -> list[dict]:
    selected = []
    for row in rows:
        name = row.get("event", "")
        if any(token in name for token in SUMMARY_EVENTS):
            selected.append(compact(row))
    return selected


def make_environment(flags: dict[str, str]) -> tuple[dict[str, str], list[str], dict[str, str]]:
    env = os.environ.copy()
    removed = sorted(key for key in env if key.startswith(CLEAR_PREFIXES))
    for key in removed:
        env.pop(key, None)
    env.update(flags)
    env["RUST_ODDS_LOG"] = "1"
    effective = {key: value for key, value in env.items() if key.startswith(CLEAR_PREFIXES)}
    return env, removed, effective


def verify_reusable(folder: Path, job_id: str, binary: Path, request: Path) -> dict | None:
    if not folder.exists():
        return None
    record_path = folder / "record.json"
    if not record_path.is_file():
        raise RuntimeError(f"refusing to overwrite incomplete or unrecorded output: {folder}")
    record = json.loads(record_path.read_text(encoding="utf-8"))
    if record.get("job_id") != job_id:
        raise RuntimeError(f"existing record arguments differ; refusing resume: {record_path}")
    if record.get("binary_sha256") != sha256(binary) or record.get("request_sha256") != sha256(request):
        raise RuntimeError(f"existing record input hashes differ; refusing resume: {record_path}")
    for key in ("export", "stdout", "stderr"):
        path = Path(record[key])
        if not path.is_file() or sha256(path) != record.get(f"{key}_sha256"):
            raise RuntimeError(f"existing {key} missing or hash mismatch: {path}")
    return record


def run_one(
    binary: Path,
    request: Path,
    folder: Path,
    *,
    fixture: str,
    seed: int,
    arm: str,
    flags: dict[str, str],
    repeat: int,
    warmup: bool,
    config_hash: str,
    resume: bool,
) -> dict:
    binary_hash, request_hash = sha256(binary), sha256(request)
    job_data = {
        "config_hash": config_hash, "fixture": fixture, "seed": seed, "arm": arm,
        "flags": flags, "repeat": repeat, "warmup": warmup,
        "binary": str(binary), "binary_sha256": binary_hash,
        "request": str(request), "request_sha256": request_hash,
    }
    job_id = stable_hash(job_data)
    if folder.exists():
        if not resume:
            raise RuntimeError(f"output exists; use --resume to verify and reuse: {folder}")
        existing = verify_reusable(folder, job_id, binary, request)
        if existing:
            return existing
    else:
        folder.mkdir(parents=True, exist_ok=False)
    export = folder / "export.json"
    stdout = folder / "stdout.log"
    stderr = folder / "stderr.log"
    argv = [str(binary), "estimate", str(request), str(export), str(seed), "4"]
    env, removed, effective = make_environment(flags)
    environment_fingerprint = stable_hash(sorted(env.items()))
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    started = time.perf_counter()
    with stdout.open("wb") as out, stderr.open("wb") as err:
        completed = subprocess.run(argv, env=env, stdout=out, stderr=err, check=False)
    wall = time.perf_counter() - started
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    if completed.returncode != 0 or not export.is_file():
        failure = {
            **job_data, "job_id": job_id, "argv": argv, "exit_code": completed.returncode,
            "stdout": str(stdout), "stderr": str(stderr),
        }
        (folder / "failure.json").write_text(json.dumps(failure, indent=2) + "\n", encoding="utf-8")
        raise RuntimeError(f"{arm} failed for {fixture}, seed {seed}, exit {completed.returncode}; see {stderr}")
    rows = event_rows(stdout, stderr)
    record = {
        **job_data, "job_id": job_id, "workers": 4, "argv": argv,
        "cleared_inherited_environment_keys": removed,
        "effective_experiment_environment": effective,
        "effective_environment_sha256": environment_fingerprint,
        "exit_code": completed.returncode,
        "export": str(export), "export_sha256": sha256(export),
        "stdout": str(stdout), "stdout_sha256": sha256(stdout),
        "stderr": str(stderr), "stderr_sha256": sha256(stderr),
        "summary_events": summary_events(rows),
        "wall_seconds": wall,
        "child_cpu_seconds": (after.ru_utime - before.ru_utime) + (after.ru_stime - before.ru_stime),
    }
    temp = folder / "record.json.tmp"
    temp.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temp.replace(folder / "record.json")
    return record


def write_json(path: Path, value: object, overwrite_same_config: bool, config_hash: str) -> None:
    if path.exists():
        existing = json.loads(path.read_text(encoding="utf-8"))
        if not overwrite_same_config or existing.get("config_hash") != config_hash:
            raise RuntimeError(f"refusing to overwrite output with different configuration: {path}")
    temp = path.with_suffix(path.suffix + ".tmp")
    temp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temp.replace(path)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--candidate-binary", required=True, type=lambda x: Path(x).resolve(strict=True))
    parser.add_argument("--frozen-production-binary", type=lambda x: Path(x).resolve(strict=True))
    parser.add_argument("--frozen-production-sha256")
    parser.add_argument("--out-dir", required=True, type=lambda x: Path(x).resolve())
    parser.add_argument("--fixtures", default=",".join(DEFAULT_FIXTURES))
    parser.add_argument("--seeds", type=int_values, default=[808])
    parser.add_argument("--arms", default=None, help="comma-separated arm names; frozen requires its binary")
    parser.add_argument("--repeats", type=int, default=1)
    parser.add_argument("--warmups", type=int, default=0)
    parser.add_argument("--resume", action="store_true")
    args = parser.parse_args()
    try:
        fixtures = csv_values(args.fixtures, set(FIXTURES))
        default_arms = DEFAULT_ARMS + (["frozen"] if args.frozen_production_binary else [])
        arms = csv_values(args.arms, set(ARMS)) if args.arms else default_arms
    except argparse.ArgumentTypeError as exc:
        parser.error(str(exc))
    if args.repeats < 1 or args.warmups < 0:
        parser.error("--repeats must be >= 1 and --warmups must be >= 0")
    if "frozen" in arms and not args.frozen_production_binary:
        parser.error("arm 'frozen' requires --frozen-production-binary")
    if args.candidate_binary == args.frozen_production_binary:
        parser.error("candidate and frozen production binaries must be distinct paths")
    for binary in [args.candidate_binary, args.frozen_production_binary]:
        if binary and not os.access(binary, os.X_OK):
            parser.error(f"binary is not executable: {binary}")
    if args.frozen_production_binary and args.frozen_production_sha256:
        if sha256(args.frozen_production_binary) != args.frozen_production_sha256.lower():
            parser.error("frozen production binary hash does not match --frozen-production-sha256")
    resolved_fixtures = {name: fixture_path(name) for name in fixtures}
    arm_specs = []
    for arm in arms:
        binary = args.frozen_production_binary if arm == "frozen" else args.candidate_binary
        arm_specs.append({"name": arm, "binary": str(binary), "binary_sha256": sha256(binary), "flags": ARMS[arm]})
    config = {
        "candidate_binary": str(args.candidate_binary),
        "candidate_sha256": sha256(args.candidate_binary),
        "frozen_production_binary": str(args.frozen_production_binary) if args.frozen_production_binary else None,
        "frozen_sha256": sha256(args.frozen_production_binary) if args.frozen_production_binary else None,
        "fixtures": {name: {"path": str(path), "sha256": sha256(path)} for name, path in resolved_fixtures.items()},
        "seeds": args.seeds, "arms": arm_specs, "repeats": args.repeats,
        "warmups": args.warmups, "workers": 4, "serialized": True,
        "cleared_prefixes": list(CLEAR_PREFIXES),
    }
    config_hash = stable_hash(config)
    output = args.out_dir
    if output.exists() and any(output.iterdir()):
        if not args.resume:
            parser.error(f"output directory is non-empty; use --resume only for the same matrix: {output}")
        manifest = output / "run_manifest.json"
        if not manifest.is_file() or json.loads(manifest.read_text()).get("config_hash") != config_hash:
            parser.error("existing output manifest is missing or does not match the requested matrix")
    output.mkdir(parents=True, exist_ok=True)
    write_json(output / "run_manifest.json", {"config_hash": config_hash, "config": config}, args.resume, config_hash)
    records: list[dict] = []
    request_index = 0
    for fixture in fixtures:
        request = resolved_fixtures[fixture]
        for seed_index, seed in enumerate(args.seeds):
            for warmup_index in range(args.warmups):
                shift = (request_index + warmup_index) % len(arm_specs)
                order = arm_specs[shift:] + arm_specs[:shift]
                for arm in order:
                    folder = output / "warmups" / fixture / f"seed-{seed}" / f"warmup-{warmup_index}" / arm["name"]
                    record = run_one(Path(arm["binary"]), request, folder, fixture=fixture, seed=seed,
                                     arm=arm["name"], flags=arm["flags"], repeat=warmup_index, warmup=True,
                                     config_hash=config_hash, resume=args.resume)
                    records.append(record)
                    request_index += 1
                    print(json.dumps({"completed": request_index, "fixture": fixture, "seed": seed,
                                      "arm": arm["name"], "warmup": True}), flush=True)
            for repeat in range(args.repeats):
                shift = (request_index + seed_index + repeat) % len(arm_specs)
                order = arm_specs[shift:] + arm_specs[:shift]
                for arm in order:
                    folder = output / "runs" / fixture / f"seed-{seed}" / f"repeat-{repeat}" / arm["name"]
                    record = run_one(Path(arm["binary"]), request, folder, fixture=fixture, seed=seed,
                                     arm=arm["name"], flags=arm["flags"], repeat=repeat, warmup=False,
                                     config_hash=config_hash, resume=args.resume)
                    records.append(record)
                    request_index += 1
                    print(json.dumps({"completed": request_index, "fixture": fixture, "seed": seed,
                                      "repeat": repeat, "arm": arm["name"], "wall_seconds": record["wall_seconds"]}), flush=True)
    summary = {
        "config_hash": config_hash,
        "config": config,
        "records": [str(Path(record["export"]).parent / "record.json") for record in records],
        "record_count_including_warmups": len(records),
        "heavy_runs_serialized": True,
        "arm_order_rotated": True,
    }
    write_json(output / "summary.json", summary, args.resume, config_hash)
    print(json.dumps({"done": True, "records": len(records), "summary": str(output / "summary.json")}))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"run_matrix: {exc}", file=sys.stderr)
        raise
