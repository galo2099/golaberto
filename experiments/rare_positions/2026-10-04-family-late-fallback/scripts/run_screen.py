"""Run serialized baseline, parent-R59, and late-fallback R60 comparisons."""
import argparse
import hashlib
import json
import os
import resource
import subprocess
import time
from pathlib import Path

DEFAULT_SEEDS = (808, 1669, 1993, 2281, 2293)
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
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and isinstance(value.get("event"), str):
            events.append(value)
    return events


def run(binary, request, output, seed, arm, flags, workers):
    folder = output / arm / f"seed{seed}"
    folder.mkdir(parents=True, exist_ok=False)
    export = folder / "export.json"
    stdout, stderr = folder / "stdout.log", folder / "stderr.log"
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
    if code or not export.is_file():
        raise RuntimeError(f"{arm}/seed{seed} failed (exit {code}); inspect {stderr}")
    record = {
        "arm": arm, "seed": seed, "workers": workers, "binary": str(binary), "binary_sha256": sha(binary),
        "request": str(request), "request_sha256": sha(request), "argv": argv,
        "explicit_environment": {**flags, "RUST_ODDS_LOG": "1"}, "export": str(export), "export_sha256": sha(export),
        "stdout": str(stdout), "stdout_sha256": sha(stdout), "stderr": str(stderr), "stderr_sha256": sha(stderr),
        "wall_seconds": wall, "child_user_seconds": after.ru_utime - before.ru_utime,
        "child_system_seconds": after.ru_stime - before.ru_stime,
        "child_cpu_seconds": after.ru_utime - before.ru_utime + after.ru_stime - before.ru_stime,
        "events": parse_events(stderr),
    }
    (folder / "record.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--baseline", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--parent", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--candidate", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--request", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--out", required=True, type=lambda s: Path(s).resolve())
    p.add_argument("--seed-list", help="Explicit comma-separated seeds; defaults to the 16498 five-seed cohort")
    p.add_argument("--workers", type=int, default=4)
    args = p.parse_args()
    if args.workers != 4:
        p.error("the R60 comparison protocol requires exactly four workers")
    if args.out.exists():
        p.error(f"refusing existing output directory: {args.out}")
    seeds = parse_seed_list(args.seed_list, p) if args.seed_list is not None else DEFAULT_SEEDS
    common = {
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK": "1",
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE": "all",
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE": "family",
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP": "3000",
        "RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING": "1",
        "RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING_FUNDING": "stage",
    }
    args.out.mkdir(parents=True, exist_ok=False)
    arms = (("baseline", args.baseline, {}), ("parent-r59", args.parent, common), ("candidate-r60", args.candidate, common), ("candidate-flag-off", args.candidate, {}))
    records = []
    for seed in seeds:
        for arm, binary, flags in arms:
            records.append(run(binary, args.request, args.out, seed, arm, flags, args.workers))
    summary = {
        "seeds": list(seeds), "seed_selection": "explicit-list" if args.seed_list is not None else "default-16498-five-seed-cohort",
        "explicit_seed_list": list(seeds) if args.seed_list is not None else None,
        "workers": args.workers, "serialized_processes": True, "arms": [name for name, _, _ in arms],
        "request": str(args.request), "request_sha256": sha(args.request),
        "binaries": {name: {"path": str(binary), "sha256": sha(binary)} for name, binary, _ in arms},
        "experiment_flags": {name: flags for name, _, flags in arms}, "records": records,
    }
    (args.out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    main()
