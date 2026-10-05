"""Warm and serialize paired baseline, parent-R59, and candidate-R60 timings."""
import argparse
import hashlib
import json
import os
import resource
import subprocess
import time
from pathlib import Path

DEFAULT_SEEDS = (808, 1669, 2293)
ENV_PREFIXES = ("RUST_ODDS_", "RARE_POSITION_")


def sha(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def parse_seed_list(value, parser):
    parts = value.split(",")
    try:
        seeds = tuple(int(part.strip(), 10) for part in parts)
    except ValueError:
        parser.error("--seed-list must be comma-separated decimal integers")
    if not value.strip() or any(not part.strip() for part in parts):
        parser.error("--seed-list must contain at least one seed with no empty entries")
    if any(seed < 0 or seed >= 2**64 for seed in seeds):
        parser.error("--seed-list values must be unsigned 64-bit integers")
    if len(set(seeds)) != len(seeds):
        parser.error("--seed-list must not contain duplicate seeds")
    return seeds


def parse_events(path):
    events = []
    for line in Path(path).read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(event, dict) and isinstance(event.get("event"), str):
            events.append(event)
    return events


def checked_screen_record(screen, arm, seed, binary, request, workers):
    record_path = screen / arm / f"seed{seed}" / "record.json"
    record = json.loads(record_path.read_text())
    if record.get("arm") != arm or record.get("seed") != seed or record.get("workers") != workers:
        raise ValueError(f"screen record identity/workers mismatch: {record_path}")
    if Path(record.get("binary", "")).resolve() != binary.resolve() or record.get("binary_sha256") != sha(binary):
        raise ValueError(f"screen binary provenance mismatch: {record_path}")
    if Path(record.get("request", "")).resolve() != request.resolve() or record.get("request_sha256") != sha(request):
        raise ValueError(f"screen request provenance mismatch: {record_path}")
    export = Path(record["export"])
    if not export.is_file() or not record.get("export_sha256") or sha(export) != record["export_sha256"]:
        raise ValueError(f"screen export hash mismatch: {record_path}")
    return record


def invoke(binary, request, folder, seed, arm, flags, workers, expected_hash=None, warmup=False, repeat=None):
    folder.mkdir(parents=True, exist_ok=False)
    export, stdout, stderr = folder / "export.json", folder / "stdout.log", folder / "stderr.log"
    env = {key: value for key, value in os.environ.items() if not key.startswith(ENV_PREFIXES)}
    env.update(flags)
    env["RUST_ODDS_LOG"] = "1"
    argv = [str(binary), "estimate", str(request), str(export), str(seed), str(workers)]
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.perf_counter()
    with stdout.open("wb") as so, stderr.open("wb") as se:
        code = subprocess.run(argv, env=env, stdout=so, stderr=se, check=False).returncode
    wall = time.perf_counter() - start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    actual_hash = sha(export) if export.is_file() else None
    if code or actual_hash is None or (expected_hash is not None and actual_hash != expected_hash):
        raise RuntimeError(f"{folder} failed or export hash mismatch; inspect {stderr}")
    return {
        "arm": arm, "seed": seed, "repeat": repeat, "warmup": warmup, "workers": workers,
        "binary": str(binary), "binary_sha256": sha(binary), "request": str(request), "request_sha256": sha(request),
        "argv": argv, "explicit_environment": {**flags, "RUST_ODDS_LOG": "1"}, "export": str(export),
        "export_sha256": actual_hash, "expected_export_sha256": expected_hash,
        "stdout": str(stdout), "stdout_sha256": sha(stdout), "stderr": str(stderr), "stderr_sha256": sha(stderr),
        "events": parse_events(stderr),
        "wall_seconds": wall, "child_user_seconds": after.ru_utime - before.ru_utime,
        "child_system_seconds": after.ru_stime - before.ru_stime,
        "child_cpu_seconds": after.ru_utime - before.ru_utime + after.ru_stime - before.ru_stime,
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--baseline", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--parent", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--candidate", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--request", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--screen", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--out", required=True, type=lambda s: Path(s).resolve())
    p.add_argument("--seed-list", help="Explicit comma-separated timing seeds; defaults to 808,1669,2293")
    p.add_argument("--repeats", type=int, choices=(3, 5), default=3)
    p.add_argument("--workers", type=int, default=4)
    args = p.parse_args()
    if args.workers != 4:
        p.error("the R60 timing protocol requires exactly four workers")
    if args.out.exists():
        p.error(f"refusing existing output directory: {args.out}")
    seeds = parse_seed_list(args.seed_list, p) if args.seed_list is not None else DEFAULT_SEEDS
    args.out.mkdir(parents=True, exist_ok=False)
    common = {
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK": "1",
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE": "all",
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE": "family",
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP": "3000",
        "RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING": "1",
        "RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING_FUNDING": "stage",
    }
    arms = (("baseline", args.baseline, {}), ("parent-r59", args.parent, common), ("candidate-r60", args.candidate, common))
    screen_arms = {"baseline": "baseline", "parent-r59": "parent-r59", "candidate-r60": "candidate-r60"}
    screen_summary = json.loads((args.screen / "summary.json").read_text())
    if Path(screen_summary.get("request", "")).resolve() != args.request or screen_summary.get("request_sha256") != sha(args.request):
        p.error("screen request path/hash does not match timing request")
    for arm, binary, _ in arms:
        metadata = screen_summary.get("binaries", {}).get(arm, {})
        if Path(metadata.get("path", "")).resolve() != binary or metadata.get("sha256") != sha(binary):
            p.error(f"screen summary binary provenance mismatch for {arm}")
    screen_records = {
        seed: {arm: checked_screen_record(args.screen, screen_arms[arm], seed, binary, args.request, args.workers)
               for arm, binary, _ in arms}
        for seed in seeds
    }
    warmups, records = [], []
    for seed in seeds:
        for arm, binary, flags in arms:
            warmups.append(invoke(binary, args.request, args.out / "warmup" / arm / f"seed{seed}", seed, arm, flags, args.workers, expected_hash=screen_records[seed][arm]["export_sha256"], warmup=True))
    initial_order = [arm for arm, _, _ in arms]
    for seed in seeds:
        for repeat in range(args.repeats):
            offset = repeat % len(initial_order)
            order = initial_order[offset:] + initial_order[:offset]
            for arm in order:
                binary, flags = next((binary, flags) for name, binary, flags in arms if name == arm)
                screen_record = screen_records[seed][arm]
                row = invoke(binary, args.request, args.out / "timed" / arm / f"seed{seed}" / f"repeat{repeat}", seed, arm, flags, args.workers, expected_hash=screen_record["export_sha256"], repeat=repeat)
                records.append(row)
    for row in [*warmups, *records]:
        (Path(row["export"]).parent / "record.json").write_text(json.dumps(row, indent=2) + "\n")
    alternation = [initial_order[i % len(initial_order):] + initial_order[:i % len(initial_order)] for i in range(args.repeats)]
    summary = {
        "seeds": list(seeds), "seed_selection": "explicit-list" if args.seed_list is not None else "default-808-1669-2293",
        "explicit_seed_list": list(seeds) if args.seed_list is not None else None, "repeats": args.repeats,
        "workers": args.workers, "serialized_processes": True, "warmups_separate_from_timed": True,
        "arms": [name for name, _, _ in arms], "alternation_by_repeat": alternation,
        "request": str(args.request), "request_sha256": sha(args.request),
        "binaries": {name: {"path": str(binary), "sha256": sha(binary)} for name, binary, _ in arms},
        "warmups": warmups, "records": records,
    }
    (args.out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    main()
