#!/usr/bin/env python3
"""Write a compact, portable summary of hash-verified R71 matrices."""
from __future__ import annotations

import argparse
import importlib.util
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
ANALYZER_PATH = HERE / "analyze_matrix.py"
SPEC = importlib.util.spec_from_file_location("analyze_matrix", ANALYZER_PATH)
ANALYZER = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader
SPEC.loader.exec_module(ANALYZER)


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def request_key(value: dict) -> tuple:
    return value.get("fixture"), value.get("seed"), value.get("repeat")


def portable_failure(failure: dict) -> dict:
    """Keep analyzer failure evidence while excluding host paths and values."""
    item = {"kind": failure.get("kind")}
    if "message" in failure:
        message = str(failure["message"])
        item["message"] = re.sub(r"(?:/[^\s,;:'\"]+)+", "<path>", message)
    if "fields" in failure:
        item["fields"] = failure["fields"]
    if "detail" in failure:
        item["detail"] = failure["detail"]
    runs = failure.get("runs", [])
    if runs:
        unique = {request_key(run) for run in runs}
        item["affected_requests"] = [dict(fixture=f, seed=s, repeat=r) for f, s, r in sorted(unique, key=str)]
        item["request_count"] = len(unique)
    return item


def compact_estimate(estimate: dict | None) -> dict | None:
    if not isinstance(estimate, dict):
        return None
    keys = ("probability", "reachability", "status", "classification", "reason",
            "available", "impossible", "undecided", "gate", "main", "check",
            "main_probability", "check_probability")
    return {key: estimate[key] for key in keys if key in estimate}


def build_summary(run_dir: Path) -> dict:
    manifest = read_json(run_dir / "run_manifest.json")
    report = ANALYZER.analyze(run_dir, ANALYZER.FLAMENGO_ID, ANALYZER.PALMEIRAS_ID)
    config = manifest["config"]
    coverage = defaultdict(lambda: {"matched_requests": 0, "positive_gains_from_zero": 0,
                                    "positive_losses_to_zero": 0, "changed_existing_positive_estimates": 0,
                                    "reachability_changes": 0})
    for comparison in report["candidate_arm_comparisons"]:
        if comparison["baseline_arm"] != "control":
            continue
        arm = comparison["arm"]
        totals = coverage[arm]
        totals["matched_requests"] += 1
        changes = comparison["changes_vs_baseline"]
        totals["positive_gains_from_zero"] += len(changes["positive_gains_from_zero"])
        totals["positive_losses_to_zero"] += len(changes["positive_losses_to_zero"])
        totals["changed_existing_positive_estimates"] += len(changes["changed_existing_positive_estimates_ignoring_work_spent"])
        totals["reachability_changes"] += len(changes["reachability_changes"])

    failures = Counter()
    failed_requests = defaultdict(set)
    for failure in report["validation_failures"]:
        kind = failure["kind"]
        if kind == "modeled_work_overrun_or_settlement_failure":
            for run in failure.get("runs", []):
                key = request_key(run)
                for detail in run.get("failures", []):
                    field = detail.get("field", detail.get("event", "unspecified"))
                    group = (run.get("arm", "unknown"), kind, str(field))
                    failed_requests[group].add(key)
        else:
            keys = {request_key(run) for run in failure.get("runs", [])}
            group = ("all", kind, "")
            if keys:
                failed_requests[group].update(keys)
            else:
                failures[group] += 1
    work_failures = [{"arm": arm, "kind": kind, "field": field,
                      "request_count": len(keys)}
                     for (arm, kind, field), keys in sorted(failed_requests.items())]
    validation_failures = [portable_failure(item) for item in report["validation_failures"]]

    timing = report["paired_timing_by_arm"]
    paired_totals = defaultdict(lambda: {"pairs": 0, "wall_delta_seconds": 0.0, "cpu_delta_seconds": 0.0})
    for pair in report["paired_wall_cpu_deltas_vs_control"]:
        values = paired_totals[pair["arm"]]
        values["pairs"] += 1
        values["wall_delta_seconds"] += pair["wall_delta_seconds"]
        values["cpu_delta_seconds"] += pair["cpu_delta_seconds"]

    diagnostics = []
    for comparison in report["candidate_arm_comparisons"]:
        if comparison["fixture"] not in ("original-16498", "updated-16498"):
            continue
        if comparison["baseline_arm"] != "control":
            continue
        cells = [{"cell": cell["cell"], "baseline": compact_estimate(cell["baseline_estimate"]),
                 "candidate": compact_estimate(cell["candidate_estimate"]),
                 "provisional_reference_probability": cell["provisional_reference_probability"]}
                for cell in comparison["diagnostic_cells"]]
        diagnostics.append({"fixture": comparison["fixture"], "seed": comparison["seed"],
                            "repeat": comparison["repeat"], "arm": comparison["arm"],
                            "cells": cells})

    status = report["incomplete_status"]
    incomplete = {key: status[key] for key in ("complete", "expected_run_count", "valid_run_count",
                                                "invalid_record_count", "missing_or_invalid_keys")}
    if status["invalid_records"]:
        incomplete["invalid_records"] = [{"reason": re.sub(r"(?:/[^\s,;:'\"]+)+", "<path>", row.get("reason", "")),
                                            "key": row.get("key"),
                                            "mismatch_fields": sorted(row.get("mismatches", {}))}
                                           for row in status["invalid_records"]]
    return {
        "run_dir": run_dir.name,
        "config_hash": manifest["config_hash"],
        "candidate_sha256": config.get("candidate_sha256"),
        "frozen_sha256": config.get("frozen_sha256"),
        "protocol": {"fixtures": {name: value["sha256"] for name, value in config["fixtures"].items()},
                     "seeds": config["seeds"], "repeats": config["repeats"],
                     "warmups": config["warmups"], "workers": config["workers"],
                     "serialized": config["serialized"],
                     "arms": [{"name": arm["name"], "flags": arm["flags"],
                               "binary_sha256": arm["binary_sha256"]} for arm in config["arms"]]},
        "records": {"count": report["run_record_count"], "expected": status["expected_run_count"],
                    "valid": status["valid_run_count"], "complete": status["complete"]},
        "valid": report["valid"],
        "default_parity": {"matched_requests": len(report["control_vs_frozen_parity_excluding_work_spent"]),
                           "equal_requests": sum(p["equal_excluding_work_spent"] for p in report["control_vs_frozen_parity_excluding_work_spent"])},
        "coverage_changes_vs_control": dict(coverage),
        "work_failures": work_failures,
        "timing_by_arm": timing,
        "paired_timing_delta_totals_vs_control": dict(paired_totals),
        "group_16498_diagnostics": diagnostics,
        "incomplete_status": incomplete,
        "validation_failures": validation_failures,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-dir", action="append", type=Path, default=[], help="matrix directory (repeatable)")
    parser.add_argument("--run-dirs", nargs="+", type=Path, default=[], help="matrix directories")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()
    dirs = args.run_dir + args.run_dirs
    if not dirs:
        parser.error("provide at least one --run-dir or --run-dirs")
    output = args.output.resolve()
    if output.exists() and not args.force:
        parser.error(f"output exists; pass --force to replace it: {output}")
    summaries = [build_summary(path.resolve(strict=True)) for path in dirs]
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps({"schema_version": 1, "matrices": summaries}, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 2 if any(not summary["valid"] for summary in summaries) else 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"summarize_validation: {exc}", file=sys.stderr)
        raise
