#!/usr/bin/env python3
"""Validate and summarize paired Set 4 GD split regimes-only responses."""
from __future__ import annotations

import argparse
import csv
import gzip
import hashlib
import json
import math
from pathlib import Path


def sha(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def finite(x):
    return isinstance(x, (int, float)) and not isinstance(x, bool) and math.isfinite(x)


def proof_zero(row):
    diagnostic = row.get("joint_points_diagnostic") or {}
    return row.get("status") in ("joint_integer_refuted", "integer_refuted") or diagnostic.get("status") in ("joint_integer_refuted", "integer_refuted")


def arm_of(root):
    return root.get("regimes", root)


def interval(row):
    lo, hi = row.get("lower"), row.get("upper")
    return [float(lo), float(hi)] if finite(lo) and finite(hi) and 0 <= lo <= hi <= 1 else None


def first_finite(row, names):
    for key in names:
        if finite(row.get(key)):
            return float(row[key])
    return None


def response(label, path, saved, seed, workers, native_budget, expected_zeros):
    root = json.loads(path.read_text())
    arm = arm_of(root)
    errors = []
    if root.get("schema_version") != "target_points_frontier_regimes_v1":
        errors.append("response schema is not target_points_frontier_regimes_v1")
    if root.get("target") != saved.get("target"):
        errors.append("response target differs from saved input")
    if root.get("seed", arm.get("seed")) != seed:
        errors.append("response seed mismatch")
    if root.get("workers", arm.get("workers")) != workers:
        errors.append("response worker count mismatch")
    reported_budget = root.get("budget_ms_per_arm", arm.get("budget_ms"))
    if reported_budget != native_budget:
        errors.append(f"native budget mismatch: response={reported_budget}, receipt={native_budget}")
    rows = arm.get("cohorts") if isinstance(arm.get("cohorts"), list) else []
    expected_rows = saved.get("cohorts", [])
    if len(rows) != len(expected_rows):
        errors.append(f"response has {len(rows)} cohorts; input has {len(expected_rows)}")
    zeros = [i for i, row in enumerate(expected_rows) if proof_zero(row)]
    if len(zeros) != expected_zeros:
        errors.append(f"input has {len(zeros)} zero rows; expected {expected_zeros}")
    for i, row in enumerate(rows):
        if not isinstance(row, dict):
            errors.append(f"response cohort {i} is not an object")
            continue
        if row.get("cohort_index", i) != i:
            errors.append(f"response cohort ordering mismatch at {i}")
        if i >= len(expected_rows) or row.get("root_path_index") != expected_rows[i].get("root_path_index"):
            errors.append(f"response cohort {i} root path differs from input")
        if interval(row) is None:
            errors.append(f"response cohort {i} has invalid bounds")
        if i in zeros and row.get("status") != "impossible" and not (
                isinstance(row.get("nodes"), list) and row["nodes"] and
                all(isinstance(n, dict) and n.get("status") == "impossible" for n in row["nodes"])):
            errors.append(f"zero proof at cohort {i} was not preserved")
    global_interval = arm.get("global_interval")
    if not (isinstance(global_interval, list) and len(global_interval) == 2 and
            all(finite(x) for x in global_interval) and 0 <= global_interval[0] <= global_interval[1] <= 1):
        errors.append("global interval is absent or invalid")
        global_interval = None
    complete_receipts = []
    node_rows, batch_rows = [], []
    total_draws = 0
    for i, cohort in enumerate(rows):
        nodes = cohort.get("nodes", []) if isinstance(cohort, dict) else []
        cohort_draws = sum(n.get("completed_draws", 0) for n in nodes if isinstance(n, dict) and isinstance(n.get("completed_draws", 0), int))
        total_draws += cohort_draws
        for node in nodes:
            if not isinstance(node, dict):
                continue
            rec = node.get("prepare_receipt") if isinstance(node.get("prepare_receipt"), dict) else {}
            node_rows.append({"arm": label, "cohort_index": i, "root_path_index": node.get("root_path_index"),
                "node_id": node.get("node_id"), "parent_node_id": node.get("parent_node_id"),
                "depth": node.get("depth"), "proposal_kind": node.get("proposal_kind"), "status": node.get("status"),
                "lower": node.get("lower"), "upper": node.get("upper"), "completed_draws": node.get("completed_draws"),
                "plan_states": rec.get("plan_states"), "B": rec.get("proposal_weight_cap"),
                "alpha_node": rec.get("alpha_node"), "setup_ms": rec.get("setup_ms"),
                "selected_rival_ids": rec.get("selected_rival_ids"), "joint_status": rec.get("joint_status")})
            for b in node.get("batch_receipts", []) if isinstance(node.get("batch_receipts"), list) else []:
                if isinstance(b, dict):
                    batch_rows.append({"arm": label, "cohort_index": i, "node_id": node.get("node_id"), **b})
                    if b.get("complete") is True:
                        complete_receipts.append(b)
    batches = arm.get("total_completed_batches", arm.get("completed_batches"))
    if isinstance(batches, int) and total_draws != batches * 128:
        errors.append("reported draw total differs from 128 times batch count")
    if isinstance(batches, int) and len(complete_receipts) != batches:
        errors.append("complete batch receipt count differs from response batch count")
    for i, rec in enumerate(complete_receipts):
        if rec.get("draws") != 128:
            errors.append(f"complete batch receipt {i} does not report 128 draws")
        if not finite(rec.get("alpha_node")) or rec["alpha_node"] <= 0 or not finite(rec.get("alpha_look")) or rec["alpha_look"] <= 0:
            errors.append(f"complete batch receipt {i} has invalid confidence allocation")
    summary = {"arm": label, "response_path": str(path), "response_sha256": sha(path),
        "schema_version": root.get("schema_version"), "seed": root.get("seed", arm.get("seed")),
        "workers": root.get("workers", arm.get("workers")), "native_budget_ms": native_budget,
        "global_interval": global_interval,
        "global_width": global_interval[1] - global_interval[0] if global_interval else None,
        "global_upper_lower_ratio": global_interval[1] / global_interval[0] if global_interval and global_interval[0] > 0 else None,
        "qualifies_upper_lower_ratio_le_10": global_interval[1] / global_interval[0] <= 10 if global_interval and global_interval[0] > 0 else None,
        "cohort_count": len(rows), "expected_zero_count": len(zeros),
        "total_completed_draws": arm.get("total_completed_draws", arm.get("total_draws")),
        "total_completed_batches": batches, "setup_ms": arm.get("setup_ms"),
        "sampling_ms": arm.get("sampling_ms"), "elapsed_ms": arm.get("elapsed_ms"),
        "first_qualification_ms": arm.get("first_qualification_ms"),
        "growth_attempt_count": arm.get("growth_attempt_count"), "growth_accepted_count": arm.get("growth_accepted_count"),
        "growth_setup_ms": arm.get("growth_setup_ms"), "retained_states_final": arm.get("retained_states_final"),
        "retained_states_peak": arm.get("retained_states_peak"), "validation_errors": errors}
    return summary, rows, node_rows, batch_rows


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--control-response", type=Path, required=True)
    p.add_argument("--treatment-response", type=Path, required=True)
    p.add_argument("--saved", type=Path, default=Path(__file__).resolve().parents[3] / "inputs" / "target-16-rank-15-separator-saved.json")
    p.add_argument("--control-input", type=Path, required=True)
    p.add_argument("--treatment-input", type=Path, required=True)
    p.add_argument("--control-receipt", type=Path, required=True)
    p.add_argument("--treatment-receipt", type=Path, required=True)
    p.add_argument("--output-dir", type=Path, required=True)
    p.add_argument("--seed", type=int, default=2370)
    p.add_argument("--workers", type=int, default=4)
    p.add_argument("--budget-ms", type=int, default=180000)
    a = p.parse_args(argv)
    if a.output_dir.exists():
        raise SystemExit(f"refusing to overwrite {a.output_dir}")
    saved = json.loads(a.saved.read_text())
    cin, tin = json.loads(a.control_input.read_text()), json.loads(a.treatment_input.read_text())
    cr, tr = json.loads(a.control_receipt.read_text()), json.loads(a.treatment_receipt.read_text())
    if cr.get("conditional_on_highs_correct") is not True or tr.get("conditional_on_highs_correct") is not True:
        raise SystemExit("both receipts must mark the HiGHS prescreen as conditional")
    mapping = tr.get("partition", {})
    partition_rows = mapping.get("partition", []) if isinstance(mapping, dict) else []
    start = int(mapping.get("original_cohort_index", 4)) if isinstance(mapping, dict) else 4
    k = len(partition_rows)
    if not k or len(tin.get("cohorts", [])) != len(saved.get("cohorts", [])) - 1 + k:
        raise SystemExit("treatment input row count does not match its partition receipt")
    child_indices = mapping.get("treatment_cohort_indices") if isinstance(mapping, dict) else None
    if not isinstance(child_indices, list):
        # The first child replaces Set 4 at its original position; remaining
        # children are appended so all other original cohort indices stay fixed.
        child_indices = [start] + list(range(len(saved.get("cohorts", [])),
                                             len(saved.get("cohorts", [])) + k - 1))
    if len(child_indices) != k or any(not isinstance(i, int) or i < 0 or i >= len(tin.get("cohorts", [])) for i in child_indices):
        raise SystemExit("partition receipt has an invalid treatment cohort index mapping")
    expected_child_indices = [start] + list(range(len(saved.get("cohorts", [])),
                                                  len(saved.get("cohorts", [])) + k - 1))
    if child_indices != expected_child_indices:
        raise SystemExit("partition must replace original Set 4 in place and append remaining children")
    labels = [row.get("label") for row in partition_rows]
    if labels.count("10W") != 1 or labels.count("9W1D") != k - 1:
        raise SystemExit("partition receipt must contain one 10W child and all one-draw children")
    for old_i in range(len(cin.get("cohorts", []))):
        if old_i != start and cin["cohorts"][old_i] != tin["cohorts"][old_i]:
            raise SystemExit(f"treatment changed unpartitioned original cohort {old_i}")
    # Fresh prescreening has 22 infeasible statuses in each arm; retain the
    # count from each actual receipt to keep the conditional claim explicit.
    cpre, tpre = cr.get("prescreen", {}), tr.get("prescreen", {})
    czeros = 76 + int(cpre.get("infeasible_count", -1))
    tzeros = 76 + int(tpre.get("infeasible_count", -1))
    cb, tb = int(cr.get("native_budget_ms", -1)), int(tr.get("native_budget_ms", -1))
    if cb <= 0 or tb <= 0:
        raise SystemExit("arm receipt is missing a positive native budget")
    c, crows, cnodes, cbatches = response("control", a.control_response, cin, a.seed, a.workers, cb, czeros)
    t, trows, tnodes, tbatches = response("treatment", a.treatment_response, tin, a.seed, a.workers, tb, tzeros)
    if len(saved.get("cohorts", [])) != 108 or len(crows) != 108:
        c["validation_errors"].append("control does not contain the frozen 108 original cohorts")
    # Compare untouched original rows through the insertion mapping.
    differences = []
    for old_i in range(len(saved.get("cohorts", []))):
        if old_i == start:
            continue
        new_i = old_i
        old, new = interval(crows[old_i]), interval(trows[new_i])
        if old is None or new is None:
            continue
        differences.append({"original_index": old_i, "control_index": old_i, "treatment_index": new_i,
                            "control": old, "treatment": new, "upper_delta": new[1] - old[1]})
    children = []
    for offset, item in enumerate(partition_rows):
        row_index = child_indices[offset]
        bounds = interval(trows[row_index])
        children.append({"cohort_index": row_index, "label": item.get("label"),
            "draw_fixture_id": item.get("draw_fixture_id"), "lower": bounds[0] if bounds else None,
            "upper": bounds[1] if bounds else None,
            "estimate": first_finite(trows[row_index], ("estimate", "probability_estimate", "point_estimate", "mean")),
            "status": trows[row_index].get("status")})
    child_bounds = {"lower": sum(x["lower"] for x in children) if all(x["lower"] is not None for x in children) else None,
                    "upper": sum(x["upper"] for x in children) if all(x["upper"] is not None for x in children) else None,
                    "estimate_sum": sum(x["estimate"] for x in children) if all(x["estimate"] is not None for x in children) else None}
    old_set4 = interval(crows[start]) if start < len(crows) else None
    set4_comparison = {"control_original_row": old_set4, "treatment_child_count": k,
        "children": children, "summed_child_confidence_bounds": child_bounds,
        "control_original_event_prior_upper": (saved["cohorts"][start].get("joint_points_diagnostic") or {}).get("cohort_event_prior_upper"),
        "interpretation": "Bounds are confidence bounds from the sampler. Any estimate_sum is reported only when every child has an explicit point estimate."}
    a.output_dir.mkdir(parents=True)
    summaries = [c, t]
    with (a.output_dir / "per-cohort.csv").open("w", newline="") as f:
        fields = ("arm", "cohort_index", "root_path_index", "status", "lower", "upper", "completed_draws", "plan", "B", "states", "errors")
        w = csv.DictWriter(f, fieldnames=fields); w.writeheader()
        for label, rows in (("control", crows), ("treatment", trows)):
            for i, row in enumerate(rows):
                if not isinstance(row, dict):
                    continue
                b = interval(row)
                w.writerow({"arm": label, "cohort_index": i, "root_path_index": row.get("root_path_index"),
                    "status": row.get("status"), "lower": b[0] if b else None, "upper": b[1] if b else None,
                    "completed_draws": sum(n.get("completed_draws", 0) for n in row.get("nodes", []) if isinstance(n, dict)),
                    "plan": row.get("plan", row.get("selected_plan")), "B": row.get("B", row.get("b_value")),
                    "states": row.get("retained_states", row.get("states")), "errors": row.get("errors", row.get("error_count"))})
    with (a.output_dir / "per-node.csv").open("w", newline="") as f:
        rows = cnodes + tnodes
        fields = sorted({key for row in rows for key in row}) if rows else ["arm", "cohort_index"]
        w = csv.DictWriter(f, fieldnames=fields); w.writeheader(); w.writerows(rows)
    with gzip.open(a.output_dir / "per-batch.csv.gz", "wt", newline="") as f:
        fields = sorted({key for row in cbatches + tbatches for key in row}) if cbatches or tbatches else ["arm", "cohort_index"]
        w = csv.DictWriter(f, fieldnames=fields); w.writeheader(); w.writerows(cbatches + tbatches)
    comparison = {"schema_version": "set4_gd_split_analysis_v1", "conditional_on_highs_correct": True,
        "seed": a.seed, "workers": a.workers, "nominal_budget_ms_per_arm": a.budget_ms,
        "actual_native_budget_ms": {"control": cb, "treatment": tb}, "arms": summaries,
        "proof_counts": {"control_zero_rows": czeros, "treatment_zero_rows": tzeros,
            "control_rows": len(crows), "control_live_rows": len(crows) - czeros,
            "treatment_rows": len(trows), "treatment_live_rows": len(trows) - tzeros},
        "original_cohort_mapping": {"partition_start_index": start, "child_count": k,
            "control_set4_index": start, "treatment_children": child_indices,
            "original_indices_unchanged_except_partitioned_set4": True},
        "set4_partition_bounds": set4_comparison,
        "other_original_live_cohort_comparisons": [r for r in differences if r["original_index"] != start],
        "global_width_delta_treatment_minus_control": (t["global_width"] - c["global_width"] if c["global_width"] is not None and t["global_width"] is not None else None),
        "input_hashes": {"request": sha(REQUEST_PATH(a.saved)), "saved": sha(a.saved),
            "control_filtered": sha(a.control_input), "treatment_filtered": sha(a.treatment_input)},
        "validation_errors": c["validation_errors"] + t["validation_errors"]}
    (a.output_dir / "comparison.json").write_text(json.dumps(comparison, indent=2) + "\n")
    (a.output_dir / "audit.json").write_text(json.dumps({"schema_version": "set4_gd_split_audit_v1",
        "conditional_on_highs_correct": True, "control_response_sha256": c["response_sha256"],
        "treatment_response_sha256": t["response_sha256"], "valid": not comparison["validation_errors"],
        "validation_errors": comparison["validation_errors"], "global_intervals": {"control": c["global_interval"], "treatment": t["global_interval"]}}, indent=2) + "\n")
    return 0 if not comparison["validation_errors"] else 1


def REQUEST_PATH(saved_path):
    # request.json sits beside the inputs directory containing the saved file.
    return saved_path.parent / "request.json"


if __name__ == "__main__":
    raise SystemExit(main())
