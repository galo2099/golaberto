#!/usr/bin/env python3
"""Run serialized, paired odds experiments for target path overflow cases.

Example:
  python3 scripts/run_overflow.py --baseline data/golaberto-odds-baseline \
    --candidate /tmp/golaberto-odds-candidate --output /tmp/overflow-run \
    --groups 16498 --seeds 808 --arm target:RARE_POSITION_EXPERIMENT=1

The baseline is run with the inherited RUST_ODDS_* and RARE_POSITION_* variables
removed. Each candidate arm starts from that same cleaned environment and adds
only its declared key/value pairs. Runs are serialized, with arm order rotated
between repetitions. The frozen baseline hash can be changed only by passing
--baseline-sha256 with the SHA-256 of the intended baseline executable.
"""
import argparse
import hashlib
import json
import os
import resource
import statistics
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
DEFAULT_REFERENCE_DIR = ROOT / "experiments/rare_positions/reference/2026-09-30-hundredfold/inputs"
ENV_PREFIXES = ("RUST_ODDS_", "RARE_POSITION_")
BASELINE_SHA256 = "87991033bdb81854cec91f3d29c32c66f5b089f51d2530590e48b2a0efc40085"


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def parse_int_list(value):
    try:
        values = [int(item.strip()) for item in value.split(",") if item.strip()]
    except ValueError as exc:
        raise argparse.ArgumentTypeError("expected comma-separated integers") from exc
    if not values:
        raise argparse.ArgumentTypeError("at least one integer is required")
    return values


def parse_sha256(value):
    normalized = value.lower()
    if len(normalized) != 64 or any(char not in "0123456789abcdef" for char in normalized):
        raise argparse.ArgumentTypeError("expected a 64-character SHA-256 hex digest")
    return normalized


def parse_arm(value):
    try:
        name, declarations = value.split(":", 1)
        flags = {}
        for declaration in declarations.split(","):
            key, setting = declaration.split("=", 1)
            if not key or not setting or key in flags:
                raise ValueError
            flags[key] = setting
        if not name or not flags:
            raise ValueError
        return {"name": name, "flags": flags}
    except ValueError as exc:
        raise argparse.ArgumentTypeError("arm must be NAME:KEY=VALUE[,KEY=VALUE]") from exc


def request_for(reference_dir, group):
    paths = list(reference_dir.glob(f"group-{group}-*.json"))
    if len(paths) != 1:
        raise ValueError(f"expected one tracked request for group {group}, found {len(paths)}")
    path = paths[0].resolve(strict=True)
    request = json.loads(path.read_text())
    if request.get("id") != group:
        raise ValueError(f"request id mismatch for group {group}: {path}")
    return path


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


def matrix(export):
    source = export.get("rare_position_estimates")
    if not isinstance(source, dict):
        raise ValueError("export lacks rare_position_estimates object")
    cells = {}
    for team, row in source.items():
        if not isinstance(row, dict):
            raise ValueError(f"malformed estimate row for team {team}")
        for rank, estimate in row.items():
            if not isinstance(estimate, dict) or not isinstance(estimate.get("probability"), (int, float)):
                raise ValueError(f"malformed estimate for team {team}, rank {rank}")
            cells[(str(team), str(rank))] = estimate
    return cells


def positive_changes(before, after):
    left, right = matrix(before), matrix(after)
    changes = []
    for key in sorted(left.keys() | right.keys()):
        old, new = left.get(key), right.get(key)
        if old and old.get("probability", 0) > 0:
            old_comp = {k: v for k, v in old.items() if k != "work_spent"}
            new_comp = {k: v for k, v in new.items() if k != "work_spent"} if new else None
            if old_comp != new_comp:
                changes.append({"team_id": key[0], "rank_index": key[1], "before": old, "after": new})
    return changes


def compare_exports(before, after):
    left, right = matrix(before), matrix(after)
    gains, losses = [], []
    for key in sorted(left.keys() | right.keys()):
        old, new = left.get(key), right.get(key)
        old_p = old.get("probability", 0) if old else 0
        new_p = new.get("probability", 0) if new else 0
        if old_p == 0 and new_p > 0:
            gains.append({"team_id": key[0], "rank_index": key[1], "estimate": new})
        if old_p > 0 and new_p == 0:
            losses.append({"team_id": key[0], "rank_index": key[1], "before": old, "after": new})
    def zero_classification(cells):
        return {f"{team}:{rank}": sorted((str(k), str(v)) for k, v in estimate.items()
                if k != "work_spent" and any(word in k.lower() for word in ("status", "reason", "reach", "class", "available")))
                for (team, rank), estimate in cells.items() if estimate.get("probability", 0) == 0}
    return {
        "gains_from_zero": gains,
        "lost_positives": losses,
        "positive_metadata_changes_ignoring_work_spent": positive_changes(before, after),
        "impossible_or_undecided_reachability_changes": reachability_changes(before, after),
        "game_importance_equal": before.get("game_importance") == after.get("game_importance"),
        "all_zero_classifications_equal": zero_classification(left) == zero_classification(right),
        "all_zero_classification_changes": changed_mapping(zero_classification(left), zero_classification(right)),
    }


def changed_mapping(left, right):
    return [{"key": key, "before": left.get(key), "after": right.get(key)}
            for key in sorted(left.keys() | right.keys()) if left.get(key) != right.get(key)]


def reachability_changes(before, after):
    left, right = matrix(before), matrix(after)
    fields = ("reachable", "reachability", "impossible", "undecided", "status", "classification", "reason")
    changes = []
    for key in sorted(left.keys() | right.keys()):
        a, b = left.get(key, {}), right.get(key, {})
        old = {k: v for k, v in a.items() if any(part in k.lower() for part in fields)}
        new = {k: v for k, v in b.items() if any(part in k.lower() for part in fields)}
        if old != new:
            changes.append({"team_id": key[0], "rank_index": key[1], "before": old, "after": new})
    return changes


def run_one(binary, request, folder, group, seed, arm, flags, repeat, warmup):
    folder.mkdir(parents=True, exist_ok=False)
    export, stdout, stderr = folder / "export.json", folder / "stdout.log", folder / "stderr.log"
    env = {key: value for key, value in os.environ.items() if not key.startswith(ENV_PREFIXES)}
    env.update(flags)
    env["RUST_ODDS_LOG"] = "1"
    argv = [str(binary), "estimate", str(request), str(export), str(seed), "4"]
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    started = time.perf_counter()
    with stdout.open("wb") as out, stderr.open("wb") as err:
        completed = subprocess.run(argv, env=env, stdout=out, stderr=err, check=False)
    wall = time.perf_counter() - started
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    if completed.returncode or not export.is_file():
        raise RuntimeError(f"{arm} failed for group {group}, seed {seed}, exit {completed.returncode}; see {stderr}")
    events = parse_events(stderr)
    record = {
        "group": group, "seed": seed, "arm": arm, "repeat": repeat, "warmup": warmup,
        "workers": 4, "binary": str(binary), "binary_sha256": sha(binary),
        "request": str(request), "request_sha256": sha(request), "argv": argv,
        "explicit_environment": {**flags, "RUST_ODDS_LOG": "1"}, "exit_code": completed.returncode,
        "export": str(export), "export_sha256": sha(export), "stdout": str(stdout),
        "stdout_sha256": sha(stdout), "stderr": str(stderr), "stderr_sha256": sha(stderr),
        "events": events, "wall_seconds": wall,
        "child_cpu_seconds": (after.ru_utime - before.ru_utime) + (after.ru_stime - before.ru_stime),
    }
    (folder / "record.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--baseline", type=lambda x: Path(x).resolve(strict=True), required=True)
    parser.add_argument("--baseline-sha256", type=parse_sha256, default=BASELINE_SHA256,
                        help="expected baseline executable SHA-256 (default: frozen R61 hash)")
    parser.add_argument("--candidate", type=lambda x: Path(x).resolve(strict=True), required=True)
    parser.add_argument("--reference-dir", type=lambda x: Path(x).resolve(strict=True), default=DEFAULT_REFERENCE_DIR)
    parser.add_argument("--output", type=lambda x: Path(x).resolve(), required=True)
    parser.add_argument("--groups", type=parse_int_list, default=[16498])
    parser.add_argument("--seeds", type=parse_int_list, default=[808])
    parser.add_argument("--repeats", type=int, default=1)
    parser.add_argument("--warmups", type=int, default=0)
    parser.add_argument("--arm", type=parse_arm, action="append", default=[])
    args = parser.parse_args()
    if args.repeats < 1 or args.warmups < 0:
        parser.error("--repeats must be >= 1 and --warmups must be >= 0")
    if args.output.exists() and any(args.output.iterdir()):
        parser.error(f"output directory must be empty: {args.output}")
    if args.baseline == args.candidate:
        parser.error("baseline and candidate binaries must be distinct paths")
    if sha(args.baseline) != args.baseline_sha256:
        parser.error("baseline binary hash does not match --baseline-sha256")
    arms = [{"name": "baseline", "binary": args.baseline, "flags": {}}]
    names = {"baseline"}
    for arm in args.arm:
        if arm["name"] in names:
            parser.error(f"duplicate arm name: {arm['name']}")
        if any(not key.startswith(ENV_PREFIXES) for key in arm["flags"]):
            parser.error("arm keys must begin with RUST_ODDS_ or RARE_POSITION_")
        names.add(arm["name"])
        arms.append({**arm, "binary": args.candidate})
    args.output.mkdir(parents=True, exist_ok=True)
    records, comparisons = [], []
    pair_runs = {}
    for group in args.groups:
        request = request_for(args.reference_dir, group)
        for seed in args.seeds:
            for warmup_index in range(args.warmups):
                order = arms[warmup_index % len(arms):] + arms[:warmup_index % len(arms)]
                for arm in order:
                    folder = args.output / "warmups" / f"group-{group}-seed-{seed}-warmup-{warmup_index}" / arm["name"]
                    record = run_one(arm["binary"], request, folder, group, seed, arm["name"], arm["flags"], warmup_index, True)
                    records.append(record)
            for repeat in range(args.repeats):
                offset = repeat % len(arms)
                order = arms[offset:] + arms[:offset]
                current = {}
                for arm in order:
                    folder = args.output / "runs" / f"group-{group}-seed-{seed}-repeat-{repeat}" / arm["name"]
                    record = run_one(arm["binary"], request, folder, group, seed, arm["name"], arm["flags"], repeat, False)
                    records.append(record)
                    current[arm["name"]] = record
                pair_runs[(group, seed, repeat)] = current
                base_export = json.loads(Path(current["baseline"]["export"]).read_text())
                for arm in arms[1:]:
                    candidate_export = json.loads(Path(current[arm["name"]]["export"]).read_text())
                    comparisons.append({"group": group, "seed": seed, "repeat": repeat, "arm": arm["name"],
                                        "comparison": compare_exports(base_export, candidate_export)})
    timings = {}
    for arm in arms:
        walls, cpus = [], []
        for run in pair_runs.values():
            walls.append(run[arm["name"]]["wall_seconds"])
            cpus.append(run[arm["name"]]["child_cpu_seconds"])
        timings[arm["name"]] = {"paired_runs": len(walls), "wall_mean_seconds": statistics.mean(walls),
                                 "cpu_mean_seconds": statistics.mean(cpus)}
    baseline_wall = timings["baseline"]["wall_mean_seconds"]
    baseline_cpu = timings["baseline"]["cpu_mean_seconds"]
    for name, timing in timings.items():
        timing["wall_delta_vs_baseline_seconds"] = timing["wall_mean_seconds"] - baseline_wall
        timing["cpu_delta_vs_baseline_seconds"] = timing["cpu_mean_seconds"] - baseline_cpu
    summary = {
        "baseline_expected_sha256": args.baseline_sha256,
        "binaries": {arm["name"]: {"path": str(arm["binary"]), "sha256": sha(arm["binary"])} for arm in arms},
        "reference_dir": str(args.reference_dir), "groups": args.groups, "seeds": args.seeds,
        "repeats": args.repeats, "warmups": args.warmups, "workers": 4, "serialized": True,
        "arm_order_rotated_by_repeat": True, "environment_prefixes_cleared": list(ENV_PREFIXES),
        "records": records, "comparisons": comparisons, "paired_timing": timings,
    }
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps({"records": len(records), "comparisons": len(comparisons), "summary": str(args.output / "summary.json")}))


if __name__ == "__main__":
    main()
