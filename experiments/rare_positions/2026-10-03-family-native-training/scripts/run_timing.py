"""Run warmups and three serialized alternating baseline/replay/capture timing triplets."""
import argparse
import hashlib
import json
import os
import resource
import subprocess
import time
from pathlib import Path


def sha(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def parse_seed_list(value, parser):
    parts = value.split(",")
    try:
        seeds = [int(part.strip(), 10) for part in parts]
    except ValueError:
        parser.error("--seed-list must be comma-separated decimal integers")
    if not value.strip() or any(not part.strip() for part in parts):
        parser.error("--seed-list must contain at least one seed with no empty entries")
    if any(seed < 0 or seed >= 2**64 for seed in seeds):
        parser.error("--seed-list values must be unsigned 64-bit integers")
    if len(set(seeds)) != len(seeds):
        parser.error("--seed-list must not contain duplicate seeds")
    return tuple(seeds)


def invoke(binary, request, folder, seed, flags, workers, expected_hash=None, warmup=False):
    folder.mkdir(parents=True, exist_ok=False)
    export, stdout, stderr = folder / "export.json", folder / "stdout.log", folder / "stderr.log"
    env = {k: v for k, v in os.environ.items() if not k.startswith(("RUST_ODDS_", "RARE_POSITION_"))}
    env.update(flags)
    env["RUST_ODDS_LOG"] = "1"
    argv = [str(binary), "estimate", str(request), str(export), str(seed), str(workers)]
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.perf_counter()
    with stdout.open("wb") as so, stderr.open("wb") as se:
        code = subprocess.run(argv, env=env, stdout=so, stderr=se, check=False).returncode
    wall = time.perf_counter() - start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    actual = sha(export) if export.is_file() else None
    if code or actual is None or (expected_hash and actual != expected_hash):
        raise RuntimeError(f"{folder} failed or export hash mismatch; inspect {stderr}")
    return {
        "warmup": warmup, "seed": seed, "binary": str(binary), "binary_sha256": sha(binary),
        "request": str(request), "request_sha256": sha(request), "argv": argv,
        "explicit_environment": {**flags, "RUST_ODDS_LOG": "1"}, "export": str(export),
        "export_sha256": actual, "expected_export_sha256": expected_hash,
        "stdout": str(stdout), "stdout_sha256": sha(stdout), "stderr": str(stderr), "stderr_sha256": sha(stderr),
        "wall_seconds": wall, "child_user_seconds": after.ru_utime - before.ru_utime,
        "child_system_seconds": after.ru_stime - before.ru_stime,
        "child_cpu_seconds": after.ru_utime - before.ru_utime + after.ru_stime - before.ru_stime,
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--baseline", required=True, type=lambda x: Path(x).resolve(strict=True))
    p.add_argument("--replay", required=True, type=lambda x: Path(x).resolve(strict=True))
    p.add_argument("--capture", required=True, type=lambda x: Path(x).resolve(strict=True))
    p.add_argument("--request", required=True, type=lambda x: Path(x).resolve(strict=True))
    p.add_argument("--screen", required=True, type=lambda x: Path(x).resolve(strict=True))
    p.add_argument("--out", required=True, type=lambda x: Path(x).resolve())
    p.add_argument("--workers", type=int, default=4)
    p.add_argument("--order", choices=("native", "grant"), default="native")
    p.add_argument("--seed-list", help="Explicit comma-separated decimal seed list; defaults to 808,1669,2293")
    p.add_argument("--capture-only-flags", default="", help="Optional comma-separated KEY=VALUE pairs applied only to a separate capture-binary arm")
    args = p.parse_args()
    if args.out.exists():
        p.error(f"refusing existing output directory: {args.out}")
    if not 1 <= args.workers <= 4:
        p.error("--workers must be between 1 and 4")
    seeds = parse_seed_list(args.seed_list, p) if args.seed_list is not None else (808, 1669, 2293)
    args.out.mkdir(parents=True, exist_ok=False)
    common = {"RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK": "1", "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE": "all", "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE": "family", "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP": "3000"}
    if args.order == "grant": common["RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_ORDER"] = "grant"
    replay_flags = dict(common)
    capture_flags = {**common, "RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING": "1"}
    capture_only_flags = {}
    for item in filter(None, args.capture_only_flags.split(",")):
        if "=" not in item:
            p.error(f"invalid capture-only flag {item!r}; expected KEY=VALUE")
        key, value = item.split("=", 1)
        if not key.startswith("RUST_ODDS_"):
            p.error(f"capture-only flag must start RUST_ODDS_: {key}")
        capture_only_flags[key] = value
    arms = {
        "baseline": (args.baseline, {}),
        "replay": (args.replay, replay_flags),
        "capture": (args.capture, capture_flags),
    }
    if capture_only_flags:
        arms["capture-only"] = (args.capture, {**capture_flags, **capture_only_flags})
    # One untimed-in-analysis warmup per binary and seed; warmup is recorded separately.
    warmups, records = [], []
    for seed in seeds:
        for arm, (binary, flags) in arms.items():
            warmups.append({"arm": arm, **invoke(binary, args.request, args.out / "warmup" / arm / f"seed{seed}", seed, flags, args.workers, warmup=True)})
    even_repeat_order = list(arms)
    odd_repeat_order = list(reversed(even_repeat_order))
    for seed in seeds:
        for repeat in range(3):
            order = even_repeat_order if repeat % 2 == 0 else odd_repeat_order
            for arm in order:
                binary, flags = arms[arm]
                folder_arm = {"baseline": "baseline", "replay": f"replay-{args.order}-order-cap3000", "capture": f"capture-{args.order}-order-cap3000", "capture-only": "capture-only-control"}[arm]
                record = json.loads((args.screen / folder_arm / f"seed{seed}" / "record.json").read_text())
                row = invoke(binary, args.request, args.out / "timed" / arm / f"seed{seed}" / f"repeat{repeat}", seed, flags, args.workers, record["export_sha256"])
                row.update({"arm": arm, "repeat": repeat})
                records.append(row)
    for row in [*warmups, *records]:
        folder = Path(row["export"]).parent
        (folder / "record.json").write_text(json.dumps(row, indent=2) + "\n")
    alternation = f"{'/'.join(even_repeat_order)} on even repeats; {'/'.join(odd_repeat_order)} on odd repeats"
    (args.out / "summary.json").write_text(json.dumps({"seeds": list(seeds), "seed_selection": "explicit-list" if args.seed_list is not None else "default", "explicit_seed_list": list(seeds) if args.seed_list is not None else None, "repeats": 3, "workers": args.workers, "serialized_processes": True, "warmups_separate": True, "alternation": alternation, "warmups": warmups, "records": records}, indent=2) + "\n")


if __name__ == "__main__":
    main()
