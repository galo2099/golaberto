#!/usr/bin/env python3
"""Analyze an R68 run_matrix.py output directory against matched controls.

Probability values and provisional references are fractions. R67 references
are descriptive diagnostic means, not exact probabilities or confidence bounds.
"""
from __future__ import annotations

import argparse
import json
import hashlib
import math
import statistics
import sys
from pathlib import Path

FLAMENGO_ID = 17
PALMEIRAS_ID = 16
PROVISIONAL = {
    "original-16498": {
        "Flamengo / 13th": 7.56e-34,
        "Palmeiras / 15th": 4.86e-31,
        "Palmeiras / 16th": 3.01e-40,
    },
    "updated-16498": {
        "Flamengo / 13th": 8.82e-34,
        "Palmeiras / 15th": 5.43e-31,
        "Palmeiras / 16th": 3.01e-40,
    },
}
TARGETS = (
    ("Flamengo / 13th", FLAMENGO_ID, 13),
    ("Palmeiras / 15th", PALMEIRAS_ID, 15),
    ("Palmeiras / 16th", PALMEIRAS_ID, 16),
)


def load_record(path: Path) -> dict:
    record = json.loads(path.read_text(encoding="utf-8"))
    record["record_path"] = str(path)
    record["export_data"] = json.loads(Path(record["export"]).read_text(encoding="utf-8"))
    return record


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def stable_hash(value: object) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(payload).hexdigest()


def matrix(export: dict) -> dict[tuple[str, str], dict]:
    rows = export.get("rare_position_estimates")
    if not isinstance(rows, dict):
        raise ValueError("export lacks rare_position_estimates")
    result = {}
    for team, ranks in rows.items():
        if not isinstance(ranks, dict):
            continue
        for rank, estimate in ranks.items():
            if isinstance(estimate, dict):
                result[(str(team), str(rank))] = estimate
    return result


def comparable_estimate(value: dict | None) -> dict | None:
    if value is None:
        return None
    return {key: item for key, item in value.items() if key != "work_spent"}


def reachability(value: dict | None) -> dict:
    if value is None:
        return {}
    fields = ("reachability", "status", "classification", "reason", "available", "impossible", "undecided")
    return {key: val for key, val in value.items() if any(name in key.lower() for name in fields)}


def compare_exports(before: dict, after: dict) -> dict:
    left, right = matrix(before), matrix(after)
    gains, losses, changed_nonzeros, classifications = [], [], [], []
    for key in sorted(left.keys() | right.keys()):
        old, new = left.get(key), right.get(key)
        old_p = float(old.get("probability", 0.0)) if old else 0.0
        new_p = float(new.get("probability", 0.0)) if new else 0.0
        cell = {"team_id": key[0], "rank_index_zero_based": key[1]}
        if old_p == 0.0 and new_p > 0.0:
            gains.append({**cell, "after": new})
        if old_p > 0.0 and new_p == 0.0:
            losses.append({**cell, "before": old, "after": new})
        if old_p > 0.0 and new_p > 0.0 and comparable_estimate(old) != comparable_estimate(new):
            changed_nonzeros.append({**cell, "before": old, "after": new})
        if reachability(old) != reachability(new):
            classifications.append({**cell, "before": reachability(old), "after": reachability(new)})
    return {
        "positive_gains_from_zero": gains,
        "positive_losses_to_zero": losses,
        "reachability_changes": classifications,
        "changed_existing_positive_estimates_ignoring_work_spent": changed_nonzeros,
    }


def without_work_spent(value: object) -> object:
    if isinstance(value, dict):
        return {key: without_work_spent(item) for key, item in value.items() if key != "work_spent"}
    if isinstance(value, list):
        return [without_work_spent(item) for item in value]
    return value


def estimate_for(export: dict, team_id: int, rank_label: int) -> dict | None:
    return matrix(export).get((str(team_id), str(rank_label - 1)))


def rel_error(probability: object, reference: float | None) -> float | None:
    if reference is None or not isinstance(probability, (int, float)):
        return None
    value = float(probability)
    if not math.isfinite(value):
        return None
    return abs(value - reference) / reference


def raw_events(path: Path) -> list[dict]:
    events = []
    for line in path.read_text(errors="replace").splitlines():
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and isinstance(value.get("event"), str):
            events.append(value)
    return events


def record_events(record: dict) -> list[dict]:
    """Read raw, hash-verified event streams and de-duplicate repeated lines."""
    unique = {}
    for field in ("stdout", "stderr"):
        path = Path(record[field])
        if file_sha256(path) != record.get(f"{field}_sha256"):
            raise ValueError(f"{field} hash changed after run validation: {path}")
        for event in raw_events(path):
            unique[json.dumps(event, sort_keys=True, separators=(",", ":"))] = event
    return list(unique.values())


def walk_mappings(value: object, path: str = "", inherited: dict | None = None):
    if inherited is None:
        inherited = {}
    if isinstance(value, dict):
        context = dict(inherited)
        for key in ("team_id", "team", "rank", "rank_index", "rank_index_zero_based", "position"):
            if key in value:
                context[key] = value[key]
        yield path, value, context
        for key, item in value.items():
            if isinstance(item, (dict, list)):
                yield from walk_mappings(item, f"{path}.{key}" if path else key, context)
    elif isinstance(value, list):
        for index, item in enumerate(value):
            if isinstance(item, (dict, list)):
                yield from walk_mappings(item, f"{path}[{index}]", inherited)


def find_target_diagnostics(events: list[dict], team_id: int, rank_label: int) -> list[dict]:
    matches = []
    for event in events:
        for path, node, context in walk_mappings(event):
            team = context.get("team_id", context.get("team"))
            rank = context.get("rank", context.get("position"))
            if rank is None and isinstance(context.get("rank_index"), (int, float)):
                rank = int(context["rank_index"]) + 1
            if rank is None and isinstance(context.get("rank_index_zero_based"), (int, float)):
                rank = int(context["rank_index_zero_based"]) + 1
            if str(team) != str(team_id) or not isinstance(rank, (int, float)) or int(rank) != rank_label:
                continue
            has_pair_values = any(key in node for key in ("main", "check", "main_probability", "check_probability"))
            is_overflow_stage = (
                isinstance(node.get("stage"), str)
                and ("setup_stages" in path or "sampling_stages" in path)
            )
            if has_pair_values or is_overflow_stage:
                matches.append({"path": path, **node})
    return matches


def modeled_work(record: dict, events: list[dict] | None = None) -> dict:
    cells = matrix(record["export_data"]).values()
    exported = [cell.get("work_spent") for cell in cells if isinstance(cell.get("work_spent"), (int, float))]
    events = events if events is not None else record_events(record)
    grants = []
    overruns = []
    actuals = []
    for event in events:
        for path, node, _context in walk_mappings(event):
            grant = node.get(
                "grant",
                node.get("reserved_work", node.get("work_limit", node.get("fallback_grant", node.get("base_fallback_grant")))),
            )
            actual = node.get("actual", node.get("charged_work", node.get("work_spent")))
            if isinstance(grant, (int, float)):
                grants.append({"path": path, "grant": grant})
                if isinstance(actual, (int, float)) and actual > grant:
                    overruns.append({"path": path, "event": event.get("event"), "grant": grant, "actual": actual})
            if node.get("overrun") is True:
                overruns.append({"path": path, "event": event.get("event"), "field": "overrun", "value": True})
            for key in ("within_grant", "settlement_within_grant"):
                if node.get(key) is False:
                    overruns.append({"path": path, "event": event.get("event"), "field": key, "value": False})
            if isinstance(actual, (int, float)):
                actuals.append({"path": path, "event": event.get("event"), "actual": actual})
            for key in (
                "actual_work", "added_actual_work", "added_actual_work_including_stage_observer",
                "actual_work_including_stage_observer", "charged_total", "setup_work",
                "sampling_work", "validation_work", "fit_work",
            ):
                if isinstance(node.get(key), (int, float)):
                    actuals.append({"path": path, "event": event.get("event"), "field": key, "actual": node[key]})
    return {
        "export_work_spent_max": max(exported) if exported else None,
        "export_work_spent_min": min(exported) if exported else None,
        "grant_values_from_summary_events": grants,
        "actual_values_from_summary_events": actuals,
        "overruns_or_within_grant_failures": overruns,
    }


def expected_runs(config: dict) -> dict[tuple[str, int, int, str], dict]:
    expected = {}
    for fixture, source in config["fixtures"].items():
        for seed in config["seeds"]:
            for repeat in range(config["repeats"]):
                for arm in config["arms"]:
                    key = (fixture, seed, repeat, arm["name"])
                    job_data = {
                        "config_hash": None, "fixture": fixture, "seed": seed,
                        "arm": arm["name"], "flags": arm["flags"], "repeat": repeat,
                        "warmup": False, "binary": arm["binary"],
                        "binary_sha256": arm["binary_sha256"],
                        "request": source["path"], "request_sha256": source["sha256"],
                    }
                    expected[key] = job_data
    return expected


def collect_runs(run_dir: Path, manifest: dict) -> tuple[list[dict], dict]:
    config = manifest["config"]
    expected = expected_runs(config)
    records = []
    invalid = []
    observed = {}
    for path in sorted(run_dir.glob("runs/**/record.json")):
        try:
            record = json.loads(path.read_text(encoding="utf-8"))
            key = (record.get("fixture"), record.get("seed"), record.get("repeat"), record.get("arm"))
            if key not in expected:
                invalid.append({"record": str(path), "reason": "unexpected run key", "key": list(key)})
                continue
            if key in observed:
                invalid.append({"record": str(path), "reason": "duplicate run key", "key": list(key)})
                continue
            observed[key] = path
            wanted = dict(expected[key])
            wanted["config_hash"] = manifest["config_hash"]
            mismatches = {field: {"expected": value, "actual": record.get(field)}
                          for field, value in wanted.items() if record.get(field) != value}
            job_data = {**wanted}
            if record.get("job_id") != stable_hash(job_data):
                mismatches["job_id"] = {"expected": stable_hash(job_data), "actual": record.get("job_id")}
            for field in ("export", "stdout", "stderr"):
                file_path = Path(record.get(field, ""))
                hash_field = f"{field}_sha256"
                if not file_path.is_file():
                    mismatches[field] = {"expected": "existing file", "actual": str(file_path)}
                elif file_sha256(file_path) != record.get(hash_field):
                    mismatches[hash_field] = {"expected": record.get(hash_field), "actual": file_sha256(file_path)}
            if file_sha256(Path(wanted["binary"])) != wanted["binary_sha256"]:
                mismatches["binary_sha256"] = {"expected": wanted["binary_sha256"], "actual": "current binary hash mismatch"}
            if file_sha256(Path(wanted["request"])) != wanted["request_sha256"]:
                mismatches["request_sha256"] = {"expected": wanted["request_sha256"], "actual": "current request hash mismatch"}
            if mismatches:
                invalid.append({"record": str(path), "reason": "record metadata/hash validation failed",
                                "key": list(key), "mismatches": mismatches})
                continue
            record["record_path"] = str(path)
            record["export_data"] = json.loads(Path(record["export"]).read_text(encoding="utf-8"))
            records.append(record)
        except Exception as exc:
            invalid.append({"record": str(path), "reason": f"could not validate/read record: {exc}"})
    missing = sorted((list(key) for key in expected.keys() - observed.keys()), key=str)
    invalid_keys = {tuple(item["key"]) for item in invalid if "key" in item}
    missing.extend(sorted((list(key) for key in invalid_keys if key in expected), key=str))
    return records, {
        "expected_run_count": len(expected),
        "valid_run_count": len(records),
        "missing_or_invalid_keys": missing,
        "invalid_record_count": len(invalid),
        "invalid_records": invalid,
        "complete": len(records) == len(expected) and not invalid,
    }


def timing_report(records: list[dict], arms: list[str]) -> tuple[dict, list[dict]]:
    timing = {}
    keyed = {}
    for record in records:
        keyed[(record["fixture"], record["seed"], record["repeat"], record["arm"])] = record
    for arm in arms:
        runs = [record for record in records if record["arm"] == arm]
        walls = [record["wall_seconds"] for record in runs]
        cpus = [record["child_cpu_seconds"] for record in runs]
        timing[arm] = {
            "runs": len(runs),
            "wall_mean_seconds": statistics.mean(walls) if walls else None,
            "wall_median_seconds": statistics.median(walls) if walls else None,
            "cpu_mean_seconds": statistics.mean(cpus) if cpus else None,
            "cpu_median_seconds": statistics.median(cpus) if cpus else None,
        }
    paired = []
    for key, control in sorted(keyed.items()):
        fixture, seed, repeat, arm = key
        if arm != "control":
            continue
        for candidate_arm in arms:
            candidate = keyed.get((fixture, seed, repeat, candidate_arm))
            if not candidate or candidate_arm == "control":
                continue
            paired.append({
                "fixture": fixture, "seed": seed, "repeat": repeat, "arm": candidate_arm,
                "wall_delta_seconds": candidate["wall_seconds"] - control["wall_seconds"],
                "cpu_delta_seconds": candidate["child_cpu_seconds"] - control["child_cpu_seconds"],
            })
    return timing, paired


def analyze(run_dir: Path, flamingo_id: int, palmeiras_id: int) -> dict:
    manifest = json.loads((run_dir / "run_manifest.json").read_text(encoding="utf-8"))
    records, completeness = collect_runs(run_dir, manifest)
    events_by_job = {record["job_id"]: record_events(record) for record in records}
    config = manifest["config"]
    arm_names = [arm["name"] for arm in config["arms"]]
    by_key = {(r["fixture"], r["seed"], r["repeat"], r["arm"]): r for r in records}
    comparisons = []
    parity = []
    for record in records:
        if record["arm"] != "control":
            continue
        key_base = (record["fixture"], record["seed"], record["repeat"])
        control_export = record["export_data"]
        frozen = by_key.get((*key_base, "frozen"))
        if frozen:
            frozen_export = frozen["export_data"]
            a, b = without_work_spent(control_export), without_work_spent(frozen_export)
            parity.append({
                "fixture": record["fixture"], "seed": record["seed"], "repeat": record["repeat"],
                "equal_excluding_work_spent": a == b,
                "changed_cells": compare_exports(control_export, frozen_export),
            })
        for arm in arm_names:
            if arm in ("control", "frozen"):
                continue
            candidate = by_key.get((*key_base, arm))
            if not candidate:
                continue
            export = candidate["export_data"]
            changes = compare_exports(control_export, export)
            diagnostic_cells = []
            fixture_reference = PROVISIONAL.get(record["fixture"])
            control_events = events_by_job[record["job_id"]]
            candidate_events = events_by_job[candidate["job_id"]]
            for label, numeric_id, rank_label in (
                ("Flamengo / 13th", flamingo_id, 13),
                ("Palmeiras / 15th", palmeiras_id, 15),
                ("Palmeiras / 16th", palmeiras_id, 16),
            ):
                reference = fixture_reference.get(label) if fixture_reference else None
                before = estimate_for(control_export, numeric_id, rank_label)
                after = estimate_for(export, numeric_id, rank_label)
                control_diagnostics = find_target_diagnostics(control_events, numeric_id, rank_label)
                diagnostics = find_target_diagnostics(candidate_events, numeric_id, rank_label)
                diagnostic_cells.append({
                    "cell": label, "team_id": numeric_id, "rank_label_one_based": rank_label,
                    "rank_index_zero_based": rank_label - 1,
                    "control_estimate": before, "candidate_estimate": after,
                    "provisional_reference_probability": reference,
                    "reference_is_provisional_not_truth": True,
                    "control_relative_absolute_error": rel_error(before.get("probability") if before else None, reference),
                    "candidate_relative_absolute_error": rel_error(after.get("probability") if after else None, reference),
                    "control_raw_main_check_diagnostic_events": control_diagnostics,
                    "raw_main_check_diagnostic_events": diagnostics,
                })
            comparisons.append({
                "fixture": record["fixture"], "seed": record["seed"], "repeat": record["repeat"],
                "arm": arm, "changes_vs_matched_control": changes,
                "diagnostic_cells": diagnostic_cells,
                "control_modeled_work": modeled_work(record, control_events),
                "candidate_modeled_work": modeled_work(candidate, candidate_events),
                "control_wall_seconds": record["wall_seconds"],
                "candidate_wall_seconds": candidate["wall_seconds"],
                "control_cpu_seconds": record["child_cpu_seconds"],
                "candidate_cpu_seconds": candidate["child_cpu_seconds"],
            })
    timing, paired_deltas = timing_report(records, arm_names)
    return {
        "schema_version": 1,
        "probability_units": "fraction",
        "provisional_reference_notice": "R67 values are descriptive means, not exact probabilities or certified bounds.",
        "run_dir": str(run_dir.resolve()), "config_hash": manifest["config_hash"],
        "diagnostic_team_ids": {"Flamengo": flamingo_id, "Palmeiras": palmeiras_id},
        "paired_timing_by_arm": timing,
        "paired_wall_cpu_deltas_vs_control": paired_deltas,
        "control_vs_frozen_parity_excluding_work_spent": parity,
        "candidate_arm_comparisons": comparisons,
        "run_record_count": len(records),
        "incomplete_status": completeness,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-dir", required=True, type=lambda value: Path(value).resolve(strict=True))
    parser.add_argument("--output", type=Path)
    parser.add_argument("--flamengo-team-id", type=int, default=FLAMENGO_ID)
    parser.add_argument("--palmeiras-team-id", type=int, default=PALMEIRAS_ID)
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()
    output = args.output.resolve() if args.output else args.run_dir / "analysis.json"
    if output.exists() and not args.force:
        parser.error(f"analysis output exists; pass --force to replace it: {output}")
    report = analyze(args.run_dir, args.flamengo_team_id, args.palmeiras_team_id)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"analysis": str(output), "comparisons": len(report["candidate_arm_comparisons"]),
                      "run_records": report["run_record_count"]}))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"analyze_matrix: {exc}", file=sys.stderr)
        raise
