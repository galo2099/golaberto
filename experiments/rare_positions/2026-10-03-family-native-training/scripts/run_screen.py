"""Serialized R59 native-training screen. This script never chooses its output directory implicitly."""
import argparse
import hashlib
import json
import os
import resource
import subprocess
import time
from pathlib import Path

SEEDS = (808, 1669, 1993, 2281, 2293)
PREFIXES = ("RUST_ODDS_", "RARE_POSITION_")


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


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


def parse_seed_list(value, parser):
    try:
        seeds = [int(item.strip(), 10) for item in value.split(",")]
    except ValueError:
        parser.error("--seed-list must be comma-separated decimal integers")
    if not value.strip() or not seeds or any(not item.strip() for item in value.split(",")):
        parser.error("--seed-list must contain at least one seed with no empty entries")
    if any(seed < 0 or seed >= 2**64 for seed in seeds):
        parser.error("--seed-list values must be unsigned 64-bit integers")
    if len(set(seeds)) != len(seeds):
        parser.error("--seed-list must not contain duplicate seeds")
    return tuple(seeds)


def summarize_capture_validation(records):
    summaries = []
    for record in records:
        observer_events = [event for event in record.get("events", [])
                           if event.get("event") == "rust_odds_family_native_training"]
        summaries.append({
            "arm": record["arm"], "seed": record["seed"],
            "observer_event_count": len(observer_events),
            "originally_paired_true": sum(event.get("originally_paired") is True for event in observer_events),
            "originally_paired_false": sum(event.get("originally_paired") is False for event in observer_events),
            "originally_paired_missing": sum("originally_paired" not in event for event in observer_events),
            "prefix_caps": sorted({event["prefix_cap"] for event in observer_events if "prefix_cap" in event}),
            "positive_observation_caps": sorted({event["positive_observation_cap"] for event in observer_events if "positive_observation_cap" in event}),
            "native_samples": sum(event.get("native_samples", 0) for event in observer_events),
            "recorded_prefix_draws": sum(event.get("recorded_prefix_draws", 0) for event in observer_events),
            "recorded_positive_observations": sum(event.get("recorded_positive_observations", 0) for event in observer_events),
            "recorded_positive_observations_considered": sum(event.get("recorded_positive_observations_considered", 0) for event in observer_events),
            "skipped_key_observations": sum(event.get("skipped_key_observations", 0) for event in observer_events),
            "observer_actual_work": sum(event.get("observer_actual_work", 0) for event in observer_events),
            "observer_bound": sum(event.get("observer_bound", 0) for event in observer_events),
        })
    return summaries


def run(binary, request, out, seed, arm, flags, workers):
    folder = out / arm / f"seed{seed}"
    folder.mkdir(parents=True, exist_ok=False)
    export, stdout, stderr = folder / "export.json", folder / "stdout.log", folder / "stderr.log"
    env = {k: v for k, v in os.environ.items() if not k.startswith(PREFIXES)}
    env.update(flags)
    env["RUST_ODDS_LOG"] = "1"
    argv = [str(binary), "estimate", str(request), str(export), str(seed), str(workers)]
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.perf_counter()
    with stdout.open("wb") as so, stderr.open("wb") as se:
        completed = subprocess.run(argv, env=env, stdout=so, stderr=se, check=False)
    wall = time.perf_counter() - start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    if completed.returncode or not export.is_file():
        raise RuntimeError(f"{arm}/seed{seed} failed (exit {completed.returncode}); inspect {stderr}")
    events = parse_events(stderr)
    record = {
        "arm": arm, "seed": seed, "workers": workers, "binary": str(binary),
        "binary_sha256": sha(binary), "request": str(request), "request_sha256": sha(request),
        "argv": argv, "explicit_environment": {**flags, "RUST_ODDS_LOG": "1"},
        "export": str(export), "export_sha256": sha(export),
        "stdout": str(stdout), "stdout_sha256": sha(stdout),
        "stderr": str(stderr), "stderr_sha256": sha(stderr), "wall_seconds": wall,
        "child_user_seconds": after.ru_utime - before.ru_utime,
        "child_system_seconds": after.ru_stime - before.ru_stime,
        "child_cpu_seconds": (after.ru_utime - before.ru_utime) + (after.ru_stime - before.ru_stime),
        "events": events,
    }
    (folder / "record.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True, type=lambda x: Path(x).resolve(strict=True))
    parser.add_argument("--baseline-reference", type=lambda x: Path(x).resolve(strict=True), help="Recorded R57 controls root containing seedN/current-rust.json and summary.json")
    parser.add_argument("--fresh-baseline", action="store_true", help="Run the production baseline binary for each seed instead of reusing recorded reference exports")
    parser.add_argument("--replay", required=True, type=lambda x: Path(x).resolve(strict=True))
    parser.add_argument("--capture", required=True, type=lambda x: Path(x).resolve(strict=True))
    parser.add_argument("--request", required=True, type=lambda x: Path(x).resolve(strict=True))
    parser.add_argument("--out", required=True, type=lambda x: Path(x).resolve())
    parser.add_argument("--seeds", choices=("first", "all"), default="first")
    parser.add_argument("--seed-list", help="Explicit comma-separated decimal seed list; overrides --seeds")
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--order", choices=("native", "grant"), default="native")
    parser.add_argument("--capture-only-flags", default="", help="Optional comma-separated KEY=VALUE pairs for a future capture-only control")
    args = parser.parse_args()
    if not 1 <= args.workers <= 4:
        parser.error("--workers must be between 1 and 4")
    if args.out.exists():
        parser.error(f"refusing existing output directory: {args.out}")
    if not args.fresh_baseline and not args.baseline_reference:
        parser.error("--baseline-reference is required unless --fresh-baseline is set")
    seed_list = parse_seed_list(args.seed_list, parser) if args.seed_list is not None else (SEEDS if args.seeds == "all" else (SEEDS[0], SEEDS[-1]))
    seed_selection = "explicit-list" if args.seed_list is not None else args.seeds
    common_flags = {
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK": "1",
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE": "all",
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE": "family",
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP": "3000",
    }
    if args.order == "grant":
        common_flags["RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_ORDER"] = "grant"
    replay_flags = dict(common_flags)
    capture_flags = {**common_flags, "RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING": "1"}
    capture_only = {}
    for item in filter(None, args.capture_only_flags.split(",")):
        if "=" not in item:
            parser.error(f"invalid capture-only flag {item!r}; expected KEY=VALUE")
        key, value = item.split("=", 1)
        if not key.startswith("RUST_ODDS_"):
            parser.error(f"capture-only flag must start RUST_ODDS_: {key}")
        capture_only[key] = value
    expected_baseline_sha = sha(args.baseline)
    baseline_records = []
    if args.fresh_baseline:
        args.out.mkdir(parents=True, exist_ok=False)
        for seed in seed_list:
            row = run(args.baseline, args.request, args.out, seed, "baseline", {}, args.workers)
            row["baseline_worker_parity_comparable"] = True
            baseline_records.append(row)
    else:
        reference_summary = json.loads((args.baseline_reference / "summary.json").read_text())
        reference_rows = {int(r["seed"]): r for r in reference_summary["records"] if r.get("name") == "current-rust"}
        reference_payloads = []
        for seed in seed_list:
            row = reference_rows.get(seed)
            export = args.baseline_reference / f"seed{seed}" / "current-rust.json"
            if not row or row.get("binary_sha256") != expected_baseline_sha or row.get("request_sha256") != sha(args.request) or not export.is_file() or sha(export) != row.get("export_sha256"):
                parser.error(f"baseline reference does not validate for seed {seed}: {export}")
            reference_payloads.append({"arm": "baseline", "seed": seed, "workers": row.get("workers"), "requested_workers": args.workers,
                "binary": str(args.baseline), "binary_sha256": expected_baseline_sha,
                "request": str(args.request), "request_sha256": sha(args.request),
                "export": str(export), "export_sha256": sha(export), "reference_record": row,
                "reference_summary": str(args.baseline_reference / "summary.json"),
                "baseline_worker_parity_comparable": row.get("workers") == args.workers})
        args.out.mkdir(parents=True, exist_ok=False)
        for payload in reference_payloads:
            baseline_records.append(payload)
            seed = payload["seed"]
            baseline_folder = args.out / "baseline" / f"seed{seed}"
            baseline_folder.mkdir(parents=True, exist_ok=False)
            (baseline_folder / "record.json").write_text(json.dumps(payload, indent=2) + "\n")
    suffix = f"{args.order}-order-cap3000"
    specs = [
        (f"replay-{suffix}", args.replay, replay_flags),
        (f"capture-{suffix}", args.capture, capture_flags),
        ("capture-flag-off", args.capture, {}),
    ]
    if capture_only:
        specs.append(("capture-only-control", args.capture, {**capture_flags, **capture_only}))
    records = []
    records.extend(baseline_records)
    for seed in seed_list:
        for arm, binary, flags in specs:
            records.append(run(binary, args.request, args.out, seed, arm, flags, args.workers))
    summary = {
        "seeds": list(seed_list), "seed_selection": seed_selection,
        "explicit_seed_list": list(seed_list) if args.seed_list is not None else None,
        "workers": args.workers, "order": args.order,
        "baseline_mode": "fresh" if args.fresh_baseline else "recorded-reference",
        "serialized_processes": True, "request": str(args.request), "request_sha256": sha(args.request),
        "binaries": {"baseline": {"path": str(args.baseline), "sha256": expected_baseline_sha}, **{name: {"path": str(binary), "sha256": sha(binary)} for name, binary, _ in specs}},
        "records": records,
        "capture_validation": {
            "capture_only_control_enabled": bool(capture_only),
            "capture_only_flags": capture_only,
            "capture_only_prefix_cap_configured": capture_only.get("RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING_PREFIX"),
            "capture_only_positive_observation_cap_configured": capture_only.get("RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING_OBSERVATIONS"),
            "observer_events_by_arm_seed": summarize_capture_validation(
                [record for record in records if record.get("arm", "").startswith("capture-")]
            ),
        },
        "interpretation_limit": "Native preservation coverage does not validate the P15 magnitude or its statistical calibration.",
    }
    (args.out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    main()
