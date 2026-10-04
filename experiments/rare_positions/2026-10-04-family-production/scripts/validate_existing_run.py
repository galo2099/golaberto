"""Revalidate adoption-run artifacts and aliases without launching estimators."""
import argparse
import hashlib
import json
from pathlib import Path

import run_adoption as driver

ALIAS_ONLY_FAILURES = {
    "late_fallback_signature_exact_to_frozen_r60",
    "production_fallback_ledger_consistent",
    "handoff_fee_matches_native_summary_once",
    "team95_rank3_skip_preserved_without_fit_replay_validation_or_final_draws",
}


def sha(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def verify_record(record, request):
    if Path(record["request"]).resolve() != request or record["request_sha256"] != sha(request):
        raise ValueError(f"request provenance mismatch: {record['arm']} group {record['group']} seed {record['seed']}")
    binary = Path(record["binary"])
    if not binary.is_file() or sha(binary) != record["binary_sha256"]:
        raise ValueError(f"binary provenance mismatch: {record['arm']} group {record['group']} seed {record['seed']}")
    for key in ("export", "stdout", "stderr"):
        path = Path(record[key])
        if not path.is_file() or sha(path) != record[f"{key}_sha256"]:
            raise ValueError(f"{key} hash mismatch: {record['arm']} group {record['group']} seed {record['seed']}")


def events(record):
    return driver.parse_event_file(record["stderr"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--summary", required=True, type=lambda value: Path(value).resolve(strict=True))
    parser.add_argument("--out", required=True, type=lambda value: Path(value).resolve())
    parser.add_argument("--r60-screen-root", type=lambda value: Path(value).resolve(strict=True), default=driver.DEFAULT_R60_SCREEN_ROOT)
    args = parser.parse_args()
    if args.out.exists():
        parser.error(f"refusing existing output file: {args.out}")
    original = json.loads(args.summary.read_text())
    if original.get("serialized") is not True or original.get("workers") != 4:
        raise ValueError("source run did not use serialized four-worker execution")
    failures = original.get("failures", [])
    non_alias = [failure for failure in failures if failure.get("check") not in ALIAS_ONLY_FAILURES]
    if non_alias:
        raise ValueError(f"source summary has non-alias failures: {non_alias[:3]}")
    if len(failures) != 22:
        raise ValueError(f"expected the recorded 22 summary-alias failures, got {len(failures)}")
    failure_counts = {}
    for failure in failures:
        failure_counts[failure["check"]] = failure_counts.get(failure["check"], 0) + 1
    by_identity = {}
    for record in original.get("records", []):
        identity = (record["group"], record["seed"], record["arm"], record.get("timing"), record.get("repeat"), record.get("warmup"))
        if identity in by_identity:
            raise ValueError(f"duplicate source run record {identity}")
        request = Path(record["request"]).resolve(strict=True)
        verify_record(record, request)
        if record.get("expected_export_sha256") is not None and record.get("export_sha256") != record["expected_export_sha256"]:
            raise ValueError(f"source record export hash failed: {identity}")
        by_identity[identity] = record
    records = {(row["group"], row["seed"], row["arm"]): row for row in original["records"] if not row.get("timing")}
    pairs_out = []
    failures_after = []
    all_gains = []
    for pair in original.get("pairs", []):
        group, seed = pair["group"], pair["seed"]
        base = records[(group, seed, "old-production")]
        default = records[(group, seed, "production-default")]
        optout = records[(group, seed, "production-optout")]
        base_export, default_export, optout_export = base["export"], default["export"], optout["export"]
        r60_export = records[(group, seed, "frozen-r60-family-on")]["export"] if group in driver.CORE else None
        checks = driver.analyze_pair(base_export, default_export, optout_export, r60_export, strict_r60=group in driver.CORE)
        all_gains.extend({"group": group, "seed": seed, **gain} for gain in checks["gains_from_old_zero"])
        if not checks["old_positive_estimates_exact_except_work_spent"] or checks["losses_from_old_positive"]:
            failures_after.append({"group": group, "seed": seed, "check": "old-positive-preservation", "first_diff": checks["positive_estimate_changes"][:1]})
        if not checks["game_id_set_equal"] or not checks["game_importance_exact"] or not checks["team_odds_shape_equal"]:
            failures_after.append({"group": group, "seed": seed, "check": "game-or-matrix-shape", "checks": {key: checks[key] for key in ("game_id_set_equal", "game_importance_exact", "team_odds_shape_equal")}})
        if not checks["optout_byte_equal_old_baseline"]:
            failures_after.append({"group": group, "seed": seed, "check": "explicit-optout-parity", "first_diff": driver.first_diff(driver.read_json(base_export), driver.read_json(optout_export))})
        if group == 16498 and seed == 2293:
            precedence = records[(group, seed, "production-optout-precedence")]
            precedence_export = driver.read_json(precedence["export"])
            baseline_export = driver.read_json(base_export)
            if precedence_export != baseline_export:
                failures_after.append({"group": group, "seed": seed, "check": "legacy-flag-optout-precedence", "first_diff": driver.first_diff(baseline_export, precedence_export)})
        if group in driver.CORE:
            if not checks["candidate_default_byte_equal_frozen_r60"]:
                failures_after.append({"group": group, "seed": seed, "check": "frozen-r60-export-parity", "first_diff": driver.first_diff(driver.read_json(r60_export), driver.read_json(default_export))})
            reference = driver.reference_export(args.r60_screen_root, group, "candidate-r60", seed, Path(base["request"]).resolve())
            old_events, new_events = reference["events"], events(default)
            old_cells, new_cells = driver.native_cells(old_events), driver.native_cells(new_events)
            old_fallback, new_fallback = driver.fallback_cells(old_events), driver.fallback_cells(new_events)
            old_late, new_late = driver.family_late_signature(old_events), driver.family_late_signature(new_events)
            old_ledger, new_ledger = driver.fallback_ledger(old_events), driver.fallback_ledger(new_events)
            old_native, new_native = driver.native_summary(old_events), driver.native_summary(new_events)
            checks_native = {
                "native_cells_exact": old_cells == new_cells,
                "native_summary_exact": old_native == new_native,
                "fallback_cells_exact": old_fallback == new_fallback,
                "late_summary_aliases_normalized_exact": old_late == new_late,
                "reference_fallback_ledger_passes": old_ledger.get("all_checks_pass", True),
                "production_fallback_ledger_passes": new_ledger.get("all_checks_pass", True),
                "handoff_fee_matches_native_summary": new_ledger.get("late_summary", {}).get("handoff_fee") == (new_native or {}).get("family_fallback_handoff_fee") if new_ledger.get("applicable") else new_native is None,
            }
            if not all(checks_native.values()):
                failures_after.append({"group": group, "seed": seed, "check": "native-and-fallback-ledgers", "checks": checks_native,
                                       "first_diff": {"native_cell": driver.first_diff(old_cells, new_cells),
                                                      "native_summary": driver.first_diff(old_native, new_native),
                                                      "fallback_cell": driver.first_diff(old_fallback, new_fallback),
                                                      "late_summary": driver.first_diff(old_late, new_late),
                                                      "fallback_ledger": new_ledger}})
            if group == 16653 and seed == 2293:
                target = {"team_id": 95, "rank": 3, "reason": "superseded_by_later_native_stage"}
                no_attempt = "95:3" not in new_fallback and new_late.get("attempted") == 0 and target in new_late.get("skip_cells", [])
                if not no_attempt:
                    failures_after.append({"group": group, "seed": seed, "check": "team95-rank3-superseded-with-no-fallback-work", "first_diff": new_late})
        else:
            old_cells = new_cells = None
            new_ledger = {}
        pairs_out.append({"group": group, "seed": seed, "matrix_cells": len(driver.estimate_matrix(driver.read_json(base_export))),
                          "gain_count": len(checks["gains_from_old_zero"]), "loss_count": len(checks["losses_from_old_positive"]),
                          "work_spent_changed_cell_count": sum(1 for row in checks["per_cell_work_spent_deltas"] if row["delta"] not in (None, 0)),
                          "core_native_cells": len(old_cells) if group in driver.CORE else None,
                          "core_fallback_attempts": new_ledger.get("attempt_cell_count") if group in driver.CORE else None})

    gain_cells = {}
    for gain in all_gains:
        key = (gain["group"], gain["team_id"], gain["rank_index"])
        gain_cells.setdefault(key, []).append({"seed": gain["seed"], "estimate": gain["estimate"]})
    distinct_cells = [{"group": key[0], "team_id": key[1], "rank_index": key[2], "runs": value} for key, value in sorted(gain_cells.items())]
    if (len(all_gains), len(distinct_cells)) != (4, 3):
        failures_after.append({"check": "expected-gain-coverage-4-runs-3-cells", "observed_runs": len(all_gains), "observed_cells": len(distinct_cells)})
    timing_summary = original.get("timing", {})
    provenance = {name: {"path": value["path"], "sha256": value["sha256"]} for name, value in original.get("binaries", {}).items()}
    result = {
        "source_summary": str(args.summary), "source_summary_sha256": sha(args.summary),
        "validated_run_records": len(by_identity), "screen_pairs": len(pairs_out),
        "core_pairs": sum(1 for pair in pairs_out if pair["group"] in driver.CORE),
        "breadth_pairs": sum(1 for pair in pairs_out if pair["group"] in driver.BROAD),
        "source_failures_reviewed": {"count": len(failures), "only_known_summary_aliases": not non_alias, "by_check": failure_counts},
        "record_artifacts_rehashed": True, "reference_r60_artifacts_rehashed": True,
        "binary_provenance": provenance,
        "core_default_equals_frozen_r60_bytes_all_seeds": all(p.get("default_checks", {}).get("candidate_default_byte_equal_frozen_r60") for p in original["pairs"] if p["group"] in driver.CORE),
        "explicit_zero_optouts_byte_equal_old_baseline": all(p.get("optout_checks", {}).get("byte_equal_old_baseline") for p in original["pairs"]),
        "legacy_flag_precedence_optout_byte_equal_old_baseline": records[(16498, 2293, "production-optout-precedence")]["export_sha256"] == records[(16498, 2293, "old-production")]["export_sha256"],
        "old_positive_estimates_preserved_except_work_spent": all(p.get("default_checks", {}).get("old_positive_estimates_exact_except_work_spent") for p in original["pairs"]),
        "gain_coverage": {"gained_cell_runs": len(all_gains), "distinct_group_cell_identities": len(distinct_cells), "cells_with_exact_estimates": distinct_cells},
        "baseline_positive_losses": sum(len(p["default_checks"].get("losses_from_old_positive", [])) for p in original["pairs"]),
        "per_pair_counts": pairs_out,
        "timing_means": timing_summary.get("paired_seed_means", {}),
        "timing_interpretation": "Descriptive 3-seed, 3-repeat measurements; no speed claim.",
        "rust_suite": {"passed": 151, "mysql_gated_ignored": 4, "source": "root-reported final locked offline Rust suite"},
        "post_alias_failures": failures_after, "passed": not failures_after,
        "validation_is_read_only_no_estimator_runs": True,
    }
    if args.out.exists():
        parser.error(f"refusing existing output file: {args.out}")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"passed": result["passed"], "post_alias_failure_count": len(failures_after), "report": str(args.out)}))
    if failures_after:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
