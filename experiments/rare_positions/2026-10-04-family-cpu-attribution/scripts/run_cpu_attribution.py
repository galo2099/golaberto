"""Run serialized warmed CPU attribution arms; does not build binaries."""
import argparse
import hashlib
import json
import os
import resource
import subprocess
import time
from pathlib import Path

SEEDS = (808, 1669, 2293)
PREFIXES = ("RUST_ODDS_", "RARE_POSITION_")
FAMILY_FLAGS = {
    "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK": "1",
    "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE": "all",
    "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE": "family",
    "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP": "3000",
    "RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING": "1",
    "RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING_FUNDING": "stage",
}


def sha(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def parse_events(path):
    events = []
    for line in Path(path).read_text(errors="replace").splitlines():
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(item, dict) and isinstance(item.get("event"), str):
            events.append(item)
    return events


def screen_hashes(screen, request):
    summary_path = screen / "summary.json"
    summary = json.loads(summary_path.read_text())
    if Path(summary["request"]).resolve() != request or summary["request_sha256"] != sha(request):
        raise ValueError("reference screen request provenance mismatch")
    reference_binaries = summary.get("binaries", {})
    for screen_arm, name in (("baseline", "baseline"), ("candidate-r60", "frozen-r60"), ("candidate-flag-off", "frozen-r60")):
        meta = reference_binaries.get(screen_arm, {})
        if not meta.get("path") or not meta.get("sha256") or sha(meta["path"]) != meta["sha256"]:
            raise ValueError(f"reference screen binary provenance invalid for {screen_arm}")
        # The concrete baseline/frozen binary argument is verified in main.
    expected = {}
    for seed in SEEDS:
        records = {row["arm"]: row for row in summary["records"] if row["seed"] == seed}
        for arm in ("baseline", "candidate-r60", "candidate-flag-off"):
            if arm not in records:
                raise ValueError(f"reference screen lacks {arm} seed {seed}")
        baseline = records["baseline"]["export_sha256"]
        candidate = records["candidate-r60"]["export_sha256"]
        off = records["candidate-flag-off"]["export_sha256"]
        for arm in ("baseline", "candidate-r60", "candidate-flag-off"):
            record = records[arm]
            binary_meta = reference_binaries[arm]
            if Path(record["binary"]).resolve() != Path(binary_meta["path"]).resolve() or record.get("binary_sha256") != binary_meta["sha256"]:
                raise ValueError(f"reference screen {arm} seed {seed} has binary provenance mismatch")
            if Path(record["request"]).resolve() != request or record.get("request_sha256") != sha(request) or record.get("workers") != 4:
                raise ValueError(f"reference screen {arm} seed {seed} has request or worker mismatch")
            for key in ("export", "stdout", "stderr"):
                path = Path(record[key])
                if not path.is_file() or sha(path) != record.get(f"{key}_sha256"):
                    raise ValueError(f"reference screen {arm} seed {seed} has invalid {key} artifact hash")
        if baseline != off:
            raise ValueError(f"reference screen flag-off differs from baseline seed {seed}")
        expected[seed] = {"family_on": candidate, "family_off": baseline}
    return summary, expected


def run_one(binary, request, out, arm, seed, flags, expected_hash, workers, warmup=False, repeat=None):
    folder = out / ("warmup" if warmup else "timed") / arm / f"seed{seed}"
    if not warmup:
        folder = folder / f"repeat{repeat}"
    folder.mkdir(parents=True, exist_ok=False)
    export, stdout, stderr = folder / "export.json", folder / "stdout.log", folder / "stderr.log"
    env = {key: value for key, value in os.environ.items() if not key.startswith(PREFIXES)}
    env.update(flags)
    env["RUST_ODDS_LOG"] = "1"
    argv = [str(binary), "estimate", str(request), str(export), str(seed), str(workers)]
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.perf_counter()
    with stdout.open("wb") as so, stderr.open("wb") as se:
        code = subprocess.run(argv, env=env, stdout=so, stderr=se, check=False).returncode
    wall = time.perf_counter() - start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    if code != 0 or not export.is_file():
        raise RuntimeError(f"{arm} seed {seed} failed (exit {code}); inspect {stderr}")
    export_sha = sha(export)
    if export_sha != expected_hash:
        raise RuntimeError(f"{arm} seed {seed} export hash differs from reference screen")
    events = parse_events(stderr)
    scopes = [e for e in events if e.get("event") == "rust_odds_cpu_scope"]
    if arm.startswith("diagnostic-"):
        labels = {e.get("label") for e in scopes}
        required = {"api.setup", "api.scout", "api.pool", "api.search", "api.response", "rare_tail.total", "confirmation.total"}
        if arm == "diagnostic-r60-family-on":
            required.add("fallback.total")
        missing = required - labels
        if missing or not any(e.get("event") == "rust_odds_complete" for e in events):
            raise RuntimeError(f"diagnostic scopes incomplete for {arm} seed {seed}: missing={sorted(missing)}")
        for event in scopes:
            for field in ("cpu_ms", "user_ms", "system_ms", "wall_ms"):
                if not isinstance(event.get(field), (int, float)) or event[field] < 0:
                    raise RuntimeError(f"invalid {field} in CPU scope {event.get('label')} for {arm} seed {seed}")
    record = {
        "arm": arm, "seed": seed, "repeat": repeat, "warmup": warmup, "workers": workers,
        "binary": str(binary), "binary_sha256": sha(binary), "request": str(request), "request_sha256": sha(request),
        "argv": argv, "explicit_environment": {**flags, "RUST_ODDS_LOG": "1"},
        "export": str(export), "export_sha256": export_sha, "expected_export_sha256": expected_hash,
        "stdout": str(stdout), "stdout_sha256": sha(stdout), "stderr": str(stderr), "stderr_sha256": sha(stderr),
        "wall_seconds": wall, "child_user_seconds": after.ru_utime - before.ru_utime,
        "child_system_seconds": after.ru_stime - before.ru_stime,
        "child_cpu_seconds": after.ru_utime - before.ru_utime + after.ru_stime - before.ru_stime,
        "events": events, "cpu_scope_events": scopes,
    }
    (folder / "record.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--baseline", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--frozen-r60", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--diagnostic-r60", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--request", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--reference-screen", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--out", required=True, type=lambda s: Path(s).resolve())
    p.add_argument("--repeats", type=int, choices=(5,), default=5)
    p.add_argument("--workers", type=int, default=4)
    args = p.parse_args()
    if args.workers != 4:
        p.error("CPU attribution protocol requires exactly four workers")
    if args.out.exists():
        p.error(f"refusing existing output directory: {args.out}")
    screen, expected = screen_hashes(args.reference_screen, args.request)
    metadata = screen.get("binaries", {})
    for screen_arm, binary in (("baseline", args.baseline), ("candidate-r60", args.frozen_r60), ("candidate-flag-off", args.frozen_r60)):
        meta = metadata[screen_arm]
        if Path(meta["path"]).resolve() != binary or sha(binary) != meta["sha256"]:
            p.error(f"reference screen binary identity does not match --{screen_arm}")
    bins = {"production-baseline": args.baseline, "frozen-r60": args.frozen_r60, "diagnostic-r60": args.diagnostic_r60}
    arms = (
        ("production-baseline", args.baseline, {}, "family_off"),
        ("frozen-r60-flag-off", args.frozen_r60, {}, "family_off"),
        ("diagnostic-r60-flag-off", args.diagnostic_r60, {"RUST_ODDS_DIAGNOSTIC_CPU": "1"}, "family_off"),
        ("diagnostic-r60-family-on", args.diagnostic_r60, {**FAMILY_FLAGS, "RUST_ODDS_DIAGNOSTIC_CPU": "1"}, "family_on"),
        ("frozen-r60-family-on", args.frozen_r60, FAMILY_FLAGS, "family_on"),
    )
    args.out.mkdir(parents=True, exist_ok=False)
    warmups, runs = [], []
    order = [row[0] for row in arms]
    for seed in SEEDS:
        for arm, binary, flags, parity in arms:
            warmups.append(run_one(binary, args.request, args.out, arm, seed, flags, expected[seed][parity], args.workers, warmup=True))
    for seed in SEEDS:
        for repeat in range(args.repeats):
            offset = (repeat + SEEDS.index(seed)) % len(order)
            rotated = order[offset:] + order[:offset]
            for arm in rotated:
                _, binary, flags, parity = next(row for row in arms if row[0] == arm)
                runs.append(run_one(binary, args.request, args.out, arm, seed, flags, expected[seed][parity], args.workers, repeat=repeat))
    alternation = [(order[(i + SEEDS.index(seed)) % len(order):] + order[:(i + SEEDS.index(seed)) % len(order)]) for seed in SEEDS for i in range(args.repeats)]
    summary = {
        "seeds": list(SEEDS), "repeats": args.repeats, "workers": args.workers,
        "serialized_processes": True, "warmups_excluded_from_timing": True,
        "request": str(args.request), "request_sha256": sha(args.request),
        "reference_screen_summary": str(args.reference_screen / "summary.json"),
        "reference_screen_summary_sha256": sha(args.reference_screen / "summary.json"),
        "reference_screen_binaries": screen.get("binaries"),
        "expected_screen_export_sha256": expected,
        "binaries": {name: {"path": str(binary), "sha256": sha(binary)} for name, binary, _, _ in arms},
        "arms": [row[0] for row in arms], "alternation_by_seed_repeat": alternation,
        "warmups": warmups, "runs": runs,
    }
    (args.out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    main()
