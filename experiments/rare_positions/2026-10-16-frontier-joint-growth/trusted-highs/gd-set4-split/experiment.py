#!/usr/bin/env python3
"""Run one arm of the Set 4 team 79 GD partition experiment."""
from __future__ import annotations

import time
PROCESS_START = time.monotonic()

import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ARTIFACT = HERE.parents[1]
PRESCREEN = ARTIFACT / "trusted-highs" / "prescreen.py"
AUDIT = ARTIFACT / "validation" / "gd-branch-audit" / "pal16-gd-branch-analysis.py"
EXPECTED_BINARY_SHA256 = "eb013be81c343762952f6a18abe7f45dda9f37a17dd33064877206c0c4fc60ab"
EXPECTED_PRESCREEN_SHA256 = "4f609a32582dd2ae24d46d74eb4384b6ee21eb2554c5f013d2ff58298fbe87df"
EXPECTED_AUDIT_SHA256 = "6554a84c4b9eed28116d51f2360b2969fcd11842329cc4e4afbdc81e8c2b75a7"
RUN_ROOT = Path("/tmp/frontier-trusted-highs-runs/target16-set4-gd-primary-2370")
TEAM_ID = 79
TARGET_POINTS = 57
TARGET_WINS = 16
SET4_INDEX = 4


def sha(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def load_audit():
    spec = importlib.util.spec_from_file_location("frozen_gd_partition_audit", AUDIT)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load frozen audit module: {AUDIT}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def team_outcome(game: dict, team_id: int, native_outcome: int) -> int:
    """Convert native home-perspective outcome to team-perspective W/D/L."""
    if native_outcome == 1:
        return 1
    team_home = game["home_id"] == team_id
    team_wins = (native_outcome == 2) if team_home else (native_outcome == 0)
    return 2 if team_wins else 0


def native_outcome(game: dict, team_id: int, team_result: int) -> int:
    if team_result == 1:
        return 1
    team_home = game["home_id"] == team_id
    team_wins = team_result == 2
    return (2 if team_wins else 0) if team_home else (0 if team_wins else 2)


def partition(saved: dict, request: dict, audit) -> tuple[dict, dict]:
    """Build the exhaustive disjoint PW partition for original cohort 4."""
    if len(saved.get("cohorts", [])) != 108 or len(saved.get("root_points", [])) != 486:
        raise ValueError("expected frozen 108-cohort, 486-root input")
    if saved.get("setup", {}).get("candidate_slots_before_integer_pruning") != 2425:
        raise ValueError("candidate slot inventory changed")
    original = saved["cohorts"][SET4_INDEX]
    if original["root_path_index"] != 0 or TEAM_ID not in original["opponent_ids"]:
        raise ValueError("cohort index 4 is not root 0 with team 79 in its rival set")

    # Use the exact frozen audit replay to identify the fixtures and initial
    # table totals; native outcome masks are in home-perspective 0/1/2 order.
    data = audit.load_data()
    fixtures = data[5]
    base = data[4]
    incident = [i for i, game in enumerate(fixtures)
                if TEAM_ID in (game["home_id"], game["away_id"])]
    if len(incident) != 10:
        raise ValueError(f"expected 10 remaining fixtures for team 79, found {len(incident)}")

    # Every 79 W/D/L assignment allowed by the original row masks is counted.
    # The other teams' fixtures do not affect 79's points or wins.
    from itertools import product
    choices = []
    for fi in incident:
        allowed = [outcome for outcome in range(3)
                   if original["fixture_masks"][fi] & (1 << outcome)]
        choices.append(allowed)
    patterns = []
    for result_tuple in product(*choices):
        points = int(base[TEAM_ID]["points"])
        wins = int(base[TEAM_ID]["wins"])
        for fi, result in zip(incident, result_tuple):
            converted = team_outcome(fixtures[fi], TEAM_ID, result)
            points += 3 if converted == 2 else 1 if converted == 1 else 0
            wins += int(converted == 2)
        if points > TARGET_POINTS or (points == TARGET_POINTS and wins >= TARGET_WINS):
            patterns.append(result_tuple)
    if len(patterns) != 11:
        raise ValueError(f"expected exactly 11 PW-valid assignments, found {len(patterns)}")

    def make_child(results: tuple[int, ...], label: str, draw_fixture=None) -> dict:
        row = copy.deepcopy(original)
        row["original_cohort_index"] = SET4_INDEX
        row["gd_partition"] = {"label": label, "draw_fixture_id": draw_fixture,
                               "gd_expected": draw_fixture is not None,
                               "team_id": TEAM_ID, "conditional_on_highs_correct": True}
        row["fixture_masks"] = list(row["fixture_masks"])
        for fi, result in zip(incident, results):
            row["fixture_masks"][fi] = 1 << result
        # The old MC interval and moment diagnostics describe a sampled parent,
        # not the fixed-mask child. Keep its event-prior upper diagnostic only.
        row.pop("lower", None)
        row.pop("upper", None)
        diagnostic = row.get("joint_points_diagnostic", {})
        prior_upper = diagnostic.get("cohort_event_prior_upper")
        row["status"] = "gd_partition_child"
        row["lower"] = 0.0
        if prior_upper is not None:
            row["upper"] = prior_upper
        row["joint_points_diagnostic"] = ({"status": "gd_partition_child",
            "cohort_event_prior_upper": prior_upper} if prior_upper is not None else
            {"status": "gd_partition_child"})
        return row

    ten_w = tuple(native_outcome(fixtures[fi], TEAM_ID, 2) for fi in incident)
    if ten_w in patterns:
        child_rows = [make_child(ten_w, "10W")]
    else:
        raise ValueError("10W assignment is not parent-mask/PW feasible")
    for pattern in patterns:
        team_results = tuple(team_outcome(fixtures[fi], TEAM_ID, result)
                             for fi, result in zip(incident, pattern))
        if team_results.count(1) == 1 and team_results.count(2) == 9:
            draw_at = team_results.index(1)
            draw_fi = incident[draw_at]
            child_rows.append(make_child(pattern, "9W1D", fixtures[draw_fi]["id"]))
    if len(child_rows) != 11 or sum(row["gd_partition"]["label"] == "9W1D" for row in child_rows) != 10:
        raise ValueError("PW-valid assignments did not yield one 10W and ten 9W1D children")

    # Verify exact disjoint coverage and every child is a subset of parent masks.
    child_patterns = set()
    for row in child_rows:
        p = tuple(next(outcome for outcome in range(3)
                       if row["fixture_masks"][fi] == (1 << outcome)) for fi in incident)
        if p in child_patterns or p not in patterns:
            raise ValueError("partition children are not disjoint allowed PW patterns")
        child_patterns.add(p)
        for fi in incident:
            if not (original["fixture_masks"][fi] & row["fixture_masks"][fi]):
                raise ValueError("child mask is outside parent mask")
    if child_patterns != set(patterns):
        raise ValueError("children do not exactly cover parent-mask PW-valid patterns")

    output = copy.deepcopy(saved)
    output["cohorts"] = [copy.deepcopy(row) for row in saved["cohorts"]]
    output["cohorts"][SET4_INDEX] = child_rows[0]
    output["cohorts"].extend(child_rows[1:])
    counts = {row["root_path_index"]: row["prepared"] for row in output["root_cohort_counts"]}
    counts[0] += 10
    output["root_cohort_counts"] = [dict(root_path_index=i, prepared=counts[i])
                                     for i in sorted(counts)]
    if len(output["cohorts"]) != 118 or counts[0] != 26:
        raise ValueError("partition row or root prepared counts mismatch")
    # Every other original cohort remains byte-for-byte JSON-equivalent.
    for old_i, old_row in enumerate(saved["cohorts"]):
        if old_i != SET4_INDEX and output["cohorts"][old_i] != old_row:
            raise ValueError(f"non-Set4 cohort {old_i} changed")
    receipt = {"schema_version": "set4_gd_partition_v1", "conditional_on_highs_correct": True,
        "original_cohort_index": SET4_INDEX, "root_path_index": 0,
        "team_id": TEAM_ID, "incident_fixture_indices": incident,
        "incident_fixture_ids": [fixtures[fi]["id"] for fi in incident],
        "pw_valid_parent_patterns": len(patterns),
        "partition": [{"label": row["gd_partition"]["label"],
                       "draw_fixture_id": row["gd_partition"]["draw_fixture_id"]} for row in child_rows],
        "original_cohort_count": 108, "partitioned_cohort_count": 118,
        "root0_prepared_before": 16, "root0_prepared_after": 26,
        "root_points_count": 486, "candidate_slots": 2425,
        "pw_threshold": {"points": TARGET_POINTS, "wins": TARGET_WINS}}
    return output, receipt


def run_prescreen(args, arm_dir: Path) -> tuple[dict, dict, Path, Path]:
    screen_dir = arm_dir / "prescreen"
    screen_filtered = screen_dir / "screen-filtered.json"
    screen_receipt = screen_dir / "screen-receipt.json"
    screen_dir.mkdir(parents=True, exist_ok=False)
    cmd = [sys.executable, str(PRESCREEN), str(args.request), str(args.saved),
           str(screen_filtered), str(screen_receipt), "--budget-ms", str(args.budget_ms),
           "--screen-only", "--binary", str(args.binary)]
    proc = subprocess.run(cmd, check=False)
    if proc.returncode:
        raise subprocess.CalledProcessError(proc.returncode, cmd)
    receipt = json.loads(screen_receipt.read_text())
    filtered = json.loads(screen_filtered.read_text())
    return filtered, receipt, screen_filtered, screen_receipt


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--arm", choices=("control", "treatment"), required=True)
    ap.add_argument("--request", type=Path, required=True)
    ap.add_argument("--saved", type=Path, required=True)
    ap.add_argument("--filtered-output", type=Path, required=True)
    ap.add_argument("--receipt", type=Path, required=True)
    ap.add_argument("--native-output", type=Path, required=True)
    ap.add_argument("--binary", type=Path, required=True)
    ap.add_argument("--seed", type=int, default=2370)
    ap.add_argument("--workers", type=int, default=4)
    ap.add_argument("--budget-ms", type=int, default=180000)
    ap.add_argument("--prepare-only", action="store_true")
    args = ap.parse_args(argv)
    if args.budget_ms <= 0:
        ap.error("--budget-ms must be positive")
    if sha(args.binary) != EXPECTED_BINARY_SHA256:
        raise ValueError("native binary SHA256 mismatch")
    if sha(PRESCREEN) != EXPECTED_PRESCREEN_SHA256 or sha(AUDIT) != EXPECTED_AUDIT_SHA256:
        raise ValueError("frozen prescreen or audit source hash mismatch")
    for path in (args.filtered_output, args.receipt, args.native_output):
        if path.exists():
            raise FileExistsError(f"refusing to overwrite {path}")
    arm_dir = RUN_ROOT / args.arm
    if arm_dir.exists():
        raise FileExistsError(f"refusing to overwrite arm directory {arm_dir}")
    arm_dir.mkdir(parents=True)
    filtered, screen_receipt, screen_filtered, screen_receipt_path = run_prescreen(args, arm_dir)
    audit = load_audit()
    prepared = copy.deepcopy(filtered)
    mapping = {"type": "identity", "rows": 108}
    if args.arm == "treatment":
        prepared, mapping = partition(filtered, json.loads(args.request.read_text()), audit)
    args.filtered_output.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.filtered_output.write_text(json.dumps(prepared, indent=2) + "\n")
    elapsed = int((time.monotonic() - PROCESS_START) * 1000)
    native_budget = max(0, args.budget_ms - elapsed)
    receipt = {"schema_version": "set4_gd_experiment_v1", "arm": args.arm,
        "conditional_on_highs_correct": True, "nominal_budget_ms": args.budget_ms,
        "native_budget_ms": native_budget, "wrapper_elapsed_ms_before_native": elapsed,
        "prescreen": screen_receipt, "prescreen_filtered_input": str(screen_filtered),
        "prescreen_receipt_path": str(screen_receipt_path),
        "partition": mapping, "binary_sha256": sha(args.binary),
        "prescreen_source_sha256": sha(PRESCREEN), "audit_source_sha256": sha(AUDIT),
        "request_sha256": sha(args.request), "saved_sha256": sha(args.saved),
        "source_cohort_mapping": "118 treatment rows map to 108 original rows; Set4 row 4 becomes 10W and 9W1D children append at indices 108..117"}
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    if args.prepare_only:
        print(json.dumps({"prepared": True, "arm": args.arm, "native_budget_ms": native_budget,
                          "cohort_count": len(prepared["cohorts"]), "receipt": str(args.receipt)}))
        return 0
    if native_budget <= 0:
        return 0
    command = [str(args.binary), str(args.request), str(args.filtered_output), str(args.native_output),
               str(args.seed), str(args.workers), str(native_budget), "--regimes-only"]
    completed = subprocess.run(command, check=False)
    receipt["native_returncode"] = completed.returncode
    receipt["native_wall_ms"] = int((time.monotonic() - PROCESS_START) * 1000) - elapsed
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
