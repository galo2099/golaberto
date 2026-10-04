"""Run serialized production-default parity, opt-out, and R60 comparisons."""
import argparse
import hashlib
import json
import os
import resource
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
DEFAULT_REFERENCE_DIR = ROOT / "experiments/rare_positions/reference/2026-09-30-hundredfold/inputs"
DEFAULT_R60_SCREEN_ROOT = ROOT / "experiments/rare_positions/2026-10-04-family-late-fallback/logs"
CORE = {16498: (808, 1669, 1993, 2281, 2293), 16653: (808, 2293), 16982: (808, 2293)}
BROAD = {15902: (808,), 16413: (808,), 16983: (808,)}
ENV_PREFIXES = ("RUST_ODDS_", "RARE_POSITION_")
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


def request_for(reference_dir, group):
    paths = sorted(reference_dir.glob(f"group-{group}-*.json"))
    if len(paths) != 1:
        raise ValueError(f"expected one tracked request snapshot for group {group}, got {len(paths)}")
    return paths[0].resolve(strict=True)


def read_json(path):
    return json.loads(Path(path).read_text())


def reference_export(screen_root, group, arm, seed, request):
    screen = screen_root / f"v2-screen-{group}"
    summary_path = screen / "summary.json"
    if not summary_path.is_file():
        raise ValueError(f"missing R60 screen summary for core group {group}: {summary_path}")
    summary = read_json(summary_path)
    if Path(summary["request"]).resolve() != request and summary["request_sha256"] != sha(request):
        raise ValueError(f"R60 screen request mismatch for group {group}")
    rows = [r for r in summary.get("records", []) if r.get("arm") == arm and r.get("seed") == seed]
    if len(rows) != 1:
        raise ValueError(f"expected one R60 {arm} record group {group} seed {seed}")
    row = rows[0]
    binary = Path(row["binary"])
    binary_available = binary.is_file()
    if not row.get("binary_sha256") or (binary_available and sha(binary) != row["binary_sha256"]):
        raise ValueError(f"frozen screen binary provenance/hash invalid: group {group} seed {seed}")
    if row.get("request_sha256") != sha(request) or row.get("workers") != 4:
        raise ValueError(f"frozen screen request/workers mismatch: group {group} seed {seed}")
    export = Path(row["export"])
    if not export.is_file() or sha(export) != row.get("export_sha256"):
        raise ValueError(f"frozen screen export hash mismatch: group {group} seed {seed}")
    for key in ("stdout", "stderr"):
        file = Path(row[key])
        if not file.is_file() or sha(file) != row.get(f"{key}_sha256"):
            raise ValueError(f"frozen screen {key} hash mismatch: group {group} seed {seed}")
    parsed_events = parse_event_file(row["stderr"])
    if parsed_events != row.get("events", []):
        raise ValueError(f"frozen screen event record differs from stderr: group {group} seed {seed}")
    return {"export": str(export), "export_sha256": row["export_sha256"], "events": parsed_events, "record": row, "binary_path": str(binary), "binary_file_available": binary_available}


def run_one(binary, request, folder, group, seed, arm, flags, workers=4, expected_hash=None, timing=False, repeat=None, warmup=False):
    folder.mkdir(parents=True, exist_ok=False)
    export, stdout, stderr = folder / "export.json", folder / "stdout.log", folder / "stderr.log"
    env = {key: value for key, value in os.environ.items() if not key.startswith(ENV_PREFIXES)}
    env.update(flags)
    env["RUST_ODDS_LOG"] = "1"
    argv = [str(binary), "estimate", str(request), str(export), str(seed), str(workers)]
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.perf_counter()
    with stdout.open("wb") as out, stderr.open("wb") as err:
        code = subprocess.run(argv, env=env, stdout=out, stderr=err, check=False).returncode
    wall = time.perf_counter() - start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    if code != 0 or not export.is_file():
        raise RuntimeError(f"{arm} group {group} seed {seed} failed with exit {code}; inspect {stderr}")
    export_hash = sha(export)
    record = {
        "group": group, "seed": seed, "arm": arm, "workers": workers, "timing": timing,
        "repeat": repeat, "warmup": warmup, "binary": str(binary), "binary_sha256": sha(binary),
        "request": str(request), "request_sha256": sha(request), "argv": argv,
        "explicit_environment": {**flags, "RUST_ODDS_LOG": "1"}, "exit_code": code,
        "export": str(export), "export_sha256": export_hash, "expected_export_sha256": expected_hash,
        "expected_export_hash_matches": export_hash == expected_hash if expected_hash is not None else None,
        "stdout": str(stdout), "stdout_sha256": sha(stdout), "stderr": str(stderr), "stderr_sha256": sha(stderr),
        "wall_seconds": wall, "child_user_seconds": after.ru_utime - before.ru_utime,
        "child_system_seconds": after.ru_stime - before.ru_stime,
        "child_cpu_seconds": after.ru_utime - before.ru_utime + after.ru_stime - before.ru_stime,
    }
    (folder / "record.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


def display_values(value):
    return str(value).encode()


def first_diff(left, right, pointer=""):
    if type(left) is not type(right):
        return {"pointer": pointer or "/", "left": left, "right": right}
    if isinstance(left, dict):
        for key in sorted(left.keys() | right.keys()):
            if key not in left or key not in right:
                return {"pointer": f"{pointer}/{key}", "left": left.get(key, "<missing>"), "right": right.get(key, "<missing>")}
            found = first_diff(left[key], right[key], f"{pointer}/{key}")
            if found:
                return found
        return None
    if isinstance(left, list):
        if len(left) != len(right):
            return {"pointer": pointer, "left_length": len(left), "right_length": len(right)}
        for index, (a, b) in enumerate(zip(left, right)):
            found = first_diff(a, b, f"{pointer}/{index}")
            if found:
                return found
        return None
    return None if left == right else {"pointer": pointer or "/", "left": left, "right": right}


def estimate_matrix(export):
    matrix = export.get("rare_position_estimates")
    if not isinstance(matrix, dict):
        raise ValueError("export lacks rare_position_estimates object")
    cells = {}
    for team, ranks in matrix.items():
        if not isinstance(ranks, dict):
            raise ValueError(f"malformed estimate row {team}")
        for rank, estimate in ranks.items():
            if not isinstance(estimate, dict) or not isinstance(estimate.get("probability"), (int, float)):
                raise ValueError(f"malformed estimate {team}:{rank}")
            cells[(str(team), str(rank))] = estimate
    return cells


def positive_differences(base, other):
    before, after = estimate_matrix(base), estimate_matrix(other)
    changes = []
    for key in sorted(before.keys() | after.keys()):
        old, new = before.get(key), after.get(key)
        if old and old.get("probability", 0) > 0:
            old_cmp = {k: v for k, v in old.items() if k != "work_spent"}
            new_cmp = {k: v for k, v in new.items() if k != "work_spent"} if new else None
            if old_cmp != new_cmp:
                changes.append({"team_id": key[0], "rank_index": key[1], "before": old, "after": new})
    return changes


def work_counter_deltas(before, after, field="work_spent"):
    a, b = estimate_matrix(before), estimate_matrix(after)
    rows = []
    for key in sorted(a.keys() | b.keys()):
        old = a.get(key, {}).get(field)
        new = b.get(key, {}).get(field)
        delta = new - old if isinstance(old, (int, float)) and isinstance(new, (int, float)) else None
        rows.append({"team_id": key[0], "rank_index": key[1], "before": old, "after": new, "delta": delta})
    return rows


def gain_loss(before, after):
    a, b = estimate_matrix(before), estimate_matrix(after)
    gain, loss = [], []
    for key in sorted(a.keys() & b.keys()):
        x, y = a[key]["probability"], b[key]["probability"]
        if x == 0 and y > 0:
            gain.append({"team_id": key[0], "rank_index": key[1], "estimate": b[key]})
        if x > 0 and y == 0:
            loss.append({"team_id": key[0], "rank_index": key[1], "before": a[key], "after": b[key]})
    return gain, loss


def native_cells(events):
    fields = ("pilot_hits", "pilot_ess", "main_draws", "main_hits", "main_ess", "main_max_share", "probability", "check_draws", "check_hits", "check_ess", "check_probability", "cost_per_draw", "modeled_main_work", "modeled_check_work", "main_only", "accepted")
    result = {}
    for event in events:
        if event.get("event") != "rust_odds_rare_tail_confirm_more":
            continue
        team, rank = event.get("team", event.get("team_id")), event.get("rank")
        if team is None or rank is None:
            raise ValueError("native event has no team/rank identity")
        key = f"{team}:{rank}"
        missing = [field for field in fields if field not in event]
        if missing:
            raise ValueError(f"native event {key} missing essential fields {missing}")
        if key in result:
            raise ValueError(f"duplicate native event {key}")
        result[key] = {field: event[field] for field in fields}
    return result


def native_summary(events):
    events = [e for e in events if e.get("event") == "rust_odds_rare_tail_confirm_more_summary"]
    if len(events) != 1:
        return None
    event = events[0]
    result = {field: event.get(field) for field in ("eligible", "funded", "completed", "accepted", "main_only", "waves", "work", "work_limit", "reserved_work", "family_fallback_enabled", "family_fallback_handoff_fee", "family_fallback_handoff_fee_funded") if field in event}
    observer_actual = event.get("native_family_observer_work", event.get("native_family_training_observer_actual"))
    observer_bound = event.get("native_family_observer_bound", event.get("native_family_training_stage_observer_bound"))
    observer_within = event.get("native_family_observer_within_bound")
    if observer_within is None and observer_actual is not None and observer_bound is not None:
        observer_within = observer_actual <= observer_bound
    if observer_actual is not None:
        result["observer_actual"] = observer_actual
    if observer_bound is not None:
        result["observer_bound"] = observer_bound
    if observer_within is not None:
        result["observer_within_bound"] = observer_within
    return result


def fallback_cells(events):
    fields = ("native_training_funding", "native_training_observer_bound", "native_training_observer_work", "base_fallback_grant", "fallback_grant", "added_actual_work", "added_actual_work_including_stage_observer", "native_plus_added_work", "unused_fallback_grant", "settlement_within_grant", "overrun", "published", "training_draws", "training_replayed_draws", "training_new_draws", "training_cap", "training_hits", "training_ess", "training_operation_work", "family_collection_work", "clone_work", "fit_work", "validation_draws", "validation_hits", "validation_ess", "validation_work", "final_draws_per_stream", "final_work")
    result = {}
    for event in events:
        if event.get("event") != "rust_odds_family_fallback":
            continue
        team = event.get("team_id", event.get("team"))
        rank = event.get("rank")
        fallback = event.get("fallback")
        if team is None or rank is None or not isinstance(fallback, dict):
            raise ValueError("fallback event lacks identity or nested record")
        key = f"{team}:{rank}"
        if key in result:
            raise ValueError(f"duplicate fallback event {key}")
        missing = [field for field in fields if field not in fallback]
        if missing:
            raise ValueError(f"fallback event {key} missing charge/settlement fields {missing}")
        result[key] = {field: fallback[field] for field in fields}
    return result


def fallback_ledger(events):
    summary_events = [e for e in events if e.get("event") == "rust_odds_family_fallback_late_summary"]
    attempts = [e for e in events if e.get("event") == "rust_odds_family_fallback"]
    skips = [e for e in events if e.get("event") in ("rust_odds_family_fallback_late_skip", "rust_odds_family_fallback_skip")]
    if not summary_events:
        return {"applicable": not attempts and not skips, "checks": {}, "reason": "no fallback summary"}
    if len(summary_events) != 1:
        return {"applicable": False, "checks": {}, "reason": "expected one late fallback summary"}
    summary = summary_events[0]
    summary_values = {
        "attempted": summary.get("attempted", summary.get("attempts")),
        "skips": summary.get("skipped_superseded_or_impossible", summary.get("skips", summary.get("skipped"))),
        "added_actual_work": summary.get("added_actual_work"),
        "granted_work": summary.get("granted_work"),
        "settled_fallback_work": summary.get("settled_fallback_work"),
        "unused_grant_work": summary.get("unused_grant_work"),
        "published_charged_work": summary.get("published_charged_work"),
        "published": summary.get("published", summary.get("added")),
        "handoff_fee": summary.get("handoff_fee", summary.get("certificate_fee", summary.get("family_fallback_handoff_fee"))),
    }
    cells = fallback_cells(events)
    nested = [e["fallback"] for e in attempts]
    order_work = int(summary.get("order_setup_work", 0))
    charged = sum(int(row.get("added_actual_work", -1)) for row in nested)
    grants = sum(int(row.get("fallback_grant", -1)) for row in nested)
    settled = sum(int(row.get("added_actual_work", -1)) for row in nested)
    unused = sum(int(row.get("unused_fallback_grant", -1)) for row in nested)
    published = [row for row in nested if row.get("published") is True]
    component_checks = {}
    cell_violations = []
    components = ("training_operation_work", "family_collection_work", "clone_work", "fit_work", "validation_work", "final_work")
    for event in attempts:
        cell = f"{event.get('team_id', event.get('team'))}:{event.get('rank')}"
        row = event["fallback"]
        component_sum = sum(int(row.get(name, 0)) for name in components)
        if component_sum != row.get("added_actual_work"):
            cell_violations.append({"cell": cell, "component_sum": component_sum, "added_actual_work": row.get("added_actual_work")})
        if row.get("overrun") is True or row.get("settlement_within_grant") is not True or row.get("added_actual_work", -1) > row.get("fallback_grant", -1) or row.get("fallback_grant") != row.get("base_fallback_grant"):
            cell_violations.append({"cell": cell, "reason": "grant/settlement/overrun guard failed"})
        stage_observer = int(row.get("native_training_observer_work", 0)) if row.get("native_training_funding") == "stage" else 0
        if row.get("added_actual_work_including_stage_observer") != row.get("added_actual_work", 0) + stage_observer:
            cell_violations.append({"cell": cell, "reason": "stage observer work was omitted or double-charged"})
    expected_summary_fields = tuple(summary_values)
    missing = [field for field, value in summary_values.items() if value is None]
    checks = {
        "summary_attempts_equal_per_cell_records": summary_values["attempted"] == len(attempts),
        "summary_skips_equal_skip_events": summary_values["skips"] == len(skips),
        "actual_work_total_includes_order_fee_once": summary_values["added_actual_work"] == charged + order_work,
        "granted_work_matches_cell_grants": summary_values["granted_work"] == grants,
        "settled_work_matches_cell_actuals": summary_values["settled_fallback_work"] == settled,
        "unused_work_matches_cell_grant_remainder": summary_values["unused_grant_work"] == unused,
        "published_count_matches_cells": summary_values["published"] == len(published),
        "published_charged_work_matches_published_cells": summary_values["published_charged_work"] == sum(int(row.get("added_actual_work", 0)) for row in published),
        "fit_train_main_check_charges_reconcile_once": not cell_violations,
    }
    return {"applicable": True, "schema_complete": not missing, "missing_summary_fields": missing,
            "checks": checks, "all_checks_pass": not missing and all(checks.values()),
            "late_summary": summary_values,
            "attempt_cell_count": len(cells), "attempts": cells, "cell_violations": cell_violations,
            "late_added_actual_work": summary.get("added_actual_work"), "cell_actual_work_sum": charged,
            "order_setup_work": order_work}


def family_late_signature(events):
    summaries = [e for e in events if e.get("event") == "rust_odds_family_fallback_late_summary"]
    skips = [e for e in events if e.get("event") in ("rust_odds_family_fallback_late_skip", "rust_odds_family_fallback_skip")]
    cells = []
    for event in skips:
        cells.append({"team_id": event.get("team_id", event.get("team")), "rank": event.get("rank"), "reason": event.get("reason")})
    if not summaries:
        return {"summary_count": 0, "skip_cells": sorted(cells, key=lambda x: (str(x["team_id"]), str(x["rank"])))}
    if len(summaries) != 1:
        raise ValueError("expected at most one late fallback summary")
    event = summaries[0]
    return {"summary_count": 1, "attempted": event.get("attempted", event.get("attempts")), "skipped_superseded_or_impossible": event.get("skipped_superseded_or_impossible", event.get("skips", event.get("skipped"))), "published": event.get("published", event.get("added")), "added_actual_work": event.get("added_actual_work"), "granted_work": event.get("granted_work"), "settled_fallback_work": event.get("settled_fallback_work"), "unused_grant_work": event.get("unused_grant_work"), "published_charged_work": event.get("published_charged_work"), "handoff_fee": event.get("handoff_fee", event.get("certificate_fee", event.get("family_fallback_handoff_fee"))), "skip_cells": sorted(cells, key=lambda x: (str(x["team_id"]), str(x["rank"]))) }


def analyze_pair(base_export, candidate_export, optout_export=None, r60_export=None, strict_r60=False):
    base = read_json(base_export)
    candidate = read_json(candidate_export)
    base_cells, candidate_cells = estimate_matrix(base), estimate_matrix(candidate)
    same_cells = base_cells.keys() == candidate_cells.keys()
    positive_changes = positive_differences(base, candidate)
    gains, losses = gain_loss(base, candidate)
    game_keys_exact = set(base.get("game_importance", {})) == set(candidate.get("game_importance", {}))
    games_exact = base.get("game_importance") == candidate.get("game_importance")
    team_matrix_shapes_equal = set(base.get("team_odds", {})) == set(candidate.get("team_odds", {})) and all(set(base["team_odds"][k]) == set(candidate["team_odds"][k]) for k in set(base.get("team_odds", {})) & set(candidate.get("team_odds", {})))
    comparisons = {
        "matrix_identity_equal": same_cells,
        "old_positive_estimates_exact_except_work_spent": not positive_changes,
        "game_id_set_equal": game_keys_exact,
        "game_importance_exact": games_exact,
        "team_odds_shape_equal": team_matrix_shapes_equal,
        "gains_from_old_zero": gains,
        "losses_from_old_positive": losses,
        "positive_estimate_changes": positive_changes,
        "per_cell_work_spent_deltas": work_counter_deltas(base, candidate),
        "full_export_bytes_equal": Path(base_export).read_bytes() == Path(candidate_export).read_bytes(),
    }
    if optout_export is not None:
        comparisons["optout_byte_equal_old_baseline"] = Path(base_export).read_bytes() == Path(optout_export).read_bytes()
    if strict_r60 and r60_export is not None:
        comparisons["candidate_default_byte_equal_frozen_r60"] = Path(candidate_export).read_bytes() == Path(r60_export).read_bytes()
    return comparisons


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--production", required=True, type=lambda s: Path(s).resolve(strict=True))
    parser.add_argument("--old-production", required=True, type=lambda s: Path(s).resolve(strict=True))
    parser.add_argument("--frozen-r60", required=True, type=lambda s: Path(s).resolve(strict=True))
    parser.add_argument("--reference-dir", type=lambda s: Path(s).resolve(strict=True), default=DEFAULT_REFERENCE_DIR)
    parser.add_argument("--r60-screen-root", type=lambda s: Path(s).resolve(strict=True), default=DEFAULT_R60_SCREEN_ROOT)
    parser.add_argument("--out", required=True, type=lambda s: Path(s).resolve())
    parser.add_argument("--timing-repeats", type=int, choices=(0, 3), default=0)
    parser.add_argument("--workers", type=int, default=4)
    args = parser.parse_args()
    if args.workers != 4:
        parser.error("production adoption protocol uses exactly four workers")
    if args.out.exists():
        parser.error(f"refusing existing output directory: {args.out}")
    binaries = {"old-production": args.old_production, "production": args.production, "frozen-r60": args.frozen_r60}
    args.out.mkdir(parents=True, exist_ok=False)
    records, pairs, failures, refs = [], [], [], {}

    def run(group, seed, arm, binary, request, flags, expected_hash=None, timing=False, repeat=None, warmup=False):
        suffix = "warmup" if warmup else f"repeat{repeat}" if timing else "screen"
        folder = args.out / suffix / str(group) / f"seed{seed}" / arm
        row = run_one(binary, request, folder, group, seed, arm, flags, args.workers, expected_hash, timing, repeat, warmup)
        records.append(row)
        return row

    group_seeds = {**CORE, **BROAD}
    for group, seeds in group_seeds.items():
        request = request_for(args.reference_dir, group)
        for seed in seeds:
            if group in CORE:
                r60 = reference_export(args.r60_screen_root, group, "candidate-r60", seed, request)
                r60_off = reference_export(args.r60_screen_root, group, "baseline", seed, request)
                refs[(group, seed)] = {"r60": r60, "baseline": r60_off}
                if r60["export_sha256"] == r60_off["export_sha256"] and group != 16982:
                    # R60 may legitimately be byte-identical when all candidate cells were already resolved.
                    pass
            base = run(group, seed, "old-production", args.old_production, request, {}, refs.get((group, seed), {}).get("baseline", {}).get("export_sha256"))
            default = run(group, seed, "production-default", args.production, request, {}, refs.get((group, seed), {}).get("r60", {}).get("export_sha256"))
            optout = run(group, seed, "production-optout", args.production, request, {"RUST_ODDS_FAMILY_FALLBACK": "0"}, refs.get((group, seed), {}).get("baseline", {}).get("export_sha256"))
            r60_run = None
            if group in CORE:
                r60_run = run(group, seed, "frozen-r60-family-on", args.frozen_r60, request, FAMILY_FLAGS, refs[(group, seed)]["r60"]["export_sha256"])
            precedence = None
            if group == 16498 and seed == 2293:
                precedence = run(group, seed, "production-optout-precedence", args.production, request, {**FAMILY_FLAGS, "RUST_ODDS_FAMILY_FALLBACK": "0"}, refs[(group, seed)]["baseline"]["export_sha256"])
            base_export, default_export, optout_export = base["export"], default["export"], optout["export"]
            r60_export = r60_run["export"] if r60_run else refs[(group, seed)]["r60"]["export"] if group in CORE else None
            default_checks = analyze_pair(base_export, default_export, optout_export, r60_export, strict_r60=group in CORE)
            optout_checks = {"byte_equal_old_baseline": Path(base_export).read_bytes() == Path(optout_export).read_bytes()}
            if precedence:
                optout_checks["legacy_flags_do_not_override_explicit_zero"] = Path(base_export).read_bytes() == Path(precedence["export"]).read_bytes()
            native = {}
            if group in CORE:
                old_events = refs[(group, seed)]["r60"]["events"]
                new_events = parse_event_file(default["stderr"])
                old_cells, new_cells = native_cells(old_events), native_cells(new_events)
                old_summary, new_summary = native_summary(old_events), native_summary(new_events)
                native = {"cell_metrics_exact_to_frozen_r60": old_cells == new_cells,
                          "cell_count_frozen_r60": len(old_cells), "cell_count_production": len(new_cells),
                          "cell_first_diff": first_diff(old_cells, new_cells),
                          "summary_metrics_exact_to_frozen_r60": old_summary == new_summary,
                          "frozen_r60_summary": old_summary, "production_summary": new_summary,
                          "late_fallback_signature": family_late_signature(new_events)}
                work_fields = ("actual_operation_work", "modeled_main_work", "modeled_check_work", "original_grant_work", "expanded_grant_work")
                native_work_deltas = []
                for identity in sorted(old_cells.keys() | new_cells.keys()):
                    before_cell, after_cell = old_cells.get(identity, {}), new_cells.get(identity, {})
                    native_work_deltas.append({"cell": identity, "counters": {field: {"frozen_r60": before_cell.get(field), "production": after_cell.get(field), "delta": after_cell.get(field) - before_cell.get(field) if isinstance(after_cell.get(field), (int, float)) and isinstance(before_cell.get(field), (int, float)) else None} for field in work_fields}})
                native["per_cell_native_work_counter_deltas"] = native_work_deltas
                old_fallback = fallback_cells(old_events)
                new_fallback = fallback_cells(new_events)
                old_late = family_late_signature(old_events)
                new_late = family_late_signature(new_events)
                native.update({"fallback_cell_charges_exact_to_frozen_r60": old_fallback == new_fallback,
                               "fallback_cell_count_frozen_r60": len(old_fallback), "fallback_cell_count_production": len(new_fallback),
                               "fallback_cell_first_diff": first_diff(old_fallback, new_fallback),
                               "frozen_r60_late_fallback_signature": old_late,
                               "late_fallback_signature_exact_to_frozen_r60": old_late == new_late})
                candidate_ledger = fallback_ledger(new_events)
                reference_ledger = fallback_ledger(old_events)
                new_native_summary = native_summary(new_events)
                fee_once = (candidate_ledger.get("late_summary", {}).get("handoff_fee") == (new_native_summary or {}).get("family_fallback_handoff_fee")) if candidate_ledger.get("applicable") and candidate_ledger.get("schema_complete", True) else not candidate_ledger.get("applicable")
                native.update({"production_fallback_ledger": candidate_ledger,
                               "frozen_r60_fallback_ledger_passes": reference_ledger.get("all_checks_pass", True),
                               "production_fallback_ledger_passes": candidate_ledger.get("all_checks_pass", True),
                               "handoff_fee_matches_native_summary_once": fee_once})
                if group == 16653 and seed == 2293:
                    expected_skip = {"team_id": 95, "rank": 3, "reason": "superseded_by_later_native_stage"}
                    new_sig = native["late_fallback_signature"]
                    old_sig = family_late_signature(old_events)
                    native["team95_rank3_skip_preserved"] = expected_skip in new_sig["skip_cells"] and new_sig.get("attempted") == 0
                    native["team95_rank3_has_no_fallback_attempt"] = "95:3" not in new_fallback
                    native["skip_signature_exact_to_frozen_r60"] = new_sig == old_sig
            pair = {"group": group, "seed": seed, "request": str(request), "request_sha256": sha(request), "default_checks": default_checks, "optout_checks": optout_checks, "native_checks": native}
            pairs.append(pair)
            for row, reference in ((base, refs.get((group, seed), {}).get("baseline")),
                                   (default, refs.get((group, seed), {}).get("r60")),
                                   (optout, refs.get((group, seed), {}).get("baseline")),
                                   (r60_run, refs.get((group, seed), {}).get("r60")),
                                   (precedence, refs.get((group, seed), {}).get("baseline"))):
                if row and reference and not row["expected_export_hash_matches"]:
                    failures.append({"group": group, "seed": seed, "arm": row["arm"], "check": "matches_frozen_screen_hash", "first_diff": first_diff(read_json(reference["export"]), read_json(row["export"]))})
            if not default_checks["matrix_identity_equal"]:
                failures.append({"group": group, "seed": seed, "arm": "production-default", "check": "matrix_identity_equal", "first_diff": first_diff(sorted(estimate_matrix(read_json(base_export))), sorted(estimate_matrix(default_export)))})
            if not default_checks["game_id_set_equal"]:
                failures.append({"group": group, "seed": seed, "arm": "production-default", "check": "game_id_set_equal", "first_diff": first_diff(sorted(read_json(base_export).get("game_importance", {})), sorted(read_json(default_export).get("game_importance", {})))})
            if not default_checks["team_odds_shape_equal"]:
                left = {team: sorted(row) for team, row in read_json(base_export).get("team_odds", {}).items()}
                right = {team: sorted(row) for team, row in read_json(default_export).get("team_odds", {}).items()}
                failures.append({"group": group, "seed": seed, "arm": "production-default", "check": "team_odds_shape_equal", "first_diff": first_diff(left, right)})
            required_checks = ["matrix_identity_equal", "old_positive_estimates_exact_except_work_spent", "game_id_set_equal", "game_importance_exact", "team_odds_shape_equal"]
            if group in CORE:
                required_checks.append("candidate_default_byte_equal_frozen_r60")
            for check in required_checks:
                if not default_checks.get(check, False):
                    if check == "old_positive_estimates_exact_except_work_spent":
                        positive = default_checks["positive_estimate_changes"]
                        difference = positive[0] if positive else None
                    elif check == "game_importance_exact":
                        difference = first_diff(read_json(base_export).get("game_importance"), read_json(default_export).get("game_importance"))
                    else:
                        difference = first_diff(read_json(default_export), read_json(r60_export)) if check == "candidate_default_byte_equal_frozen_r60" and r60_export else None
                    failures.append({"group": group, "seed": seed, "arm": "production-default", "check": check, "first_diff": difference})
            if not optout_checks["byte_equal_old_baseline"]:
                failures.append({"group": group, "seed": seed, "arm": "production-optout", "check": "byte_equal_old_baseline", "first_diff": first_diff(read_json(base_export), read_json(optout_export))})
            if precedence and not optout_checks["legacy_flags_do_not_override_explicit_zero"]:
                failures.append({"group": group, "seed": seed, "arm": "production-optout-precedence", "check": "legacy_flags_do_not_override_explicit_zero", "first_diff": first_diff(read_json(base_export), read_json(precedence["export"]))})
            if native and not native["cell_metrics_exact_to_frozen_r60"]:
                failures.append({"group": group, "seed": seed, "arm": "production-default", "check": "native_cell_metrics_exact_to_frozen_r60", "first_diff": native["cell_first_diff"]})
            if native and not native["summary_metrics_exact_to_frozen_r60"]:
                failures.append({"group": group, "seed": seed, "arm": "production-default", "check": "native_summary_metrics_exact_to_frozen_r60", "first_diff": first_diff(native["frozen_r60_summary"], native["production_summary"])})
            if native and not native["fallback_cell_charges_exact_to_frozen_r60"]:
                failures.append({"group": group, "seed": seed, "arm": "production-default", "check": "fallback_cell_charges_exact_to_frozen_r60", "first_diff": native["fallback_cell_first_diff"]})
            if native and not native["late_fallback_signature_exact_to_frozen_r60"]:
                failures.append({"group": group, "seed": seed, "arm": "production-default", "check": "late_fallback_signature_exact_to_frozen_r60", "first_diff": first_diff(native["late_fallback_signature"], native["frozen_r60_late_fallback_signature"])})
            if native and not native["frozen_r60_fallback_ledger_passes"]:
                failures.append({"group": group, "seed": seed, "arm": "frozen-r60-family-on", "check": "frozen_r60_fallback_ledger_consistent", "first_diff": native["production_fallback_ledger"]})
            if native and not native["production_fallback_ledger_passes"]:
                failures.append({"group": group, "seed": seed, "arm": "production-default", "check": "production_fallback_ledger_consistent", "first_diff": native["production_fallback_ledger"]})
            if native and not native["handoff_fee_matches_native_summary_once"]:
                failures.append({"group": group, "seed": seed, "arm": "production-default", "check": "handoff_fee_matches_native_summary_once", "first_diff": native["production_fallback_ledger"]})
            if group == 16653 and seed == 2293:
                if not native.get("team95_rank3_skip_preserved") or not native.get("team95_rank3_has_no_fallback_attempt") or not native.get("skip_signature_exact_to_frozen_r60"):
                    failures.append({"group": group, "seed": seed, "arm": "production-default", "check": "team95_rank3_skip_preserved_without_fit_replay_validation_or_final_draws", "first_diff": native.get("late_fallback_signature")})

    timing = {"enabled": bool(args.timing_repeats), "records": []}
    if args.timing_repeats:
        for seed in (808, 1669, 2293):
            request = request_for(args.reference_dir, 16498)
            base_record = next(r for r in records if r["group"] == 16498 and r["seed"] == seed and r["arm"] == "old-production")
            default_record = next(r for r in records if r["group"] == 16498 and r["seed"] == seed and r["arm"] == "production-default")
            warm_base = run(16498, seed, "old-production-timing-warmup", args.old_production, request, {}, base_record["export_sha256"], timing=True, warmup=True)
            warm_default = run(16498, seed, "production-default-timing-warmup", args.production, request, {}, default_record["export_sha256"], timing=True, warmup=True)
            timing["records"].extend([warm_base, warm_default])
            for repeat in range(args.timing_repeats):
                order = ("old-production", "production-default") if repeat % 2 == 0 else ("production-default", "old-production")
                for arm in order:
                    binary = args.old_production if arm == "old-production" else args.production
                    expected_hash = base_record["export_sha256"] if arm == "old-production" else default_record["export_sha256"]
                    timed = run(16498, seed, f"{arm}-timing", binary, request, {}, expected_hash, timing=True, repeat=repeat)
                    timing["records"].append(timed)
                    if not timed["expected_export_hash_matches"]:
                        source = base_record if arm == "old-production" else default_record
                        failures.append({"group": 16498, "seed": seed, "arm": timed["arm"], "check": "timing_export_matches_screen_run", "first_diff": first_diff(read_json(source["export"]), read_json(timed["export"]))})
        timing["paired_seed_means"] = {}
        for seed in (808, 1669, 2293):
            values = [r for r in timing["records"] if r["seed"] == seed and not r["warmup"]]
            old = [r for r in values if r["arm"] == "old-production-timing"]
            new = [r for r in values if r["arm"] == "production-default-timing"]
            timing["paired_seed_means"][str(seed)] = {
                "old_baseline_wall_seconds_mean": sum(r["wall_seconds"] for r in old) / len(old),
                "candidate_default_wall_seconds_mean": sum(r["wall_seconds"] for r in new) / len(new),
                "old_baseline_child_cpu_seconds_mean": sum(r["child_cpu_seconds"] for r in old) / len(old),
                "candidate_default_child_cpu_seconds_mean": sum(r["child_cpu_seconds"] for r in new) / len(new),
                "paired_wall_delta_seconds_mean": sum(n["wall_seconds"] - o["wall_seconds"] for o, n in zip(old, new)) / len(old),
                "paired_cpu_delta_seconds_mean": sum(n["child_cpu_seconds"] - o["child_cpu_seconds"] for o, n in zip(old, new)) / len(old),
            }

    gain_rows = []
    for pair in pairs:
        for gain in pair["default_checks"]["gains_from_old_zero"]:
            gain_rows.append({"group": pair["group"], "seed": pair["seed"], **gain})
    gain_cells = {}
    for row in gain_rows:
        key = f"{row['team_id']}:{row['rank_index']}"
        entry = gain_cells.setdefault(key, {"team_id": row["team_id"], "rank_index": row["rank_index"], "runs": []})
        entry["runs"].append({"group": row["group"], "seed": row["seed"], "estimate": row["estimate"]})
    summary = {"binaries": {name: {"path": str(path), "sha256": sha(path)} for name, path in binaries.items()},
               "reference_dir": str(args.reference_dir), "r60_screen_root": str(args.r60_screen_root),
               "workers": args.workers, "serialized": True, "environment_prefixes_cleared": list(ENV_PREFIXES),
               "pair_count": len(pairs), "records": records, "pairs": pairs,
               "gain_coverage": {"gained_cell_runs": len(gain_rows), "distinct_gained_cells": len(gain_cells), "cells": list(gain_cells.values())},
               "failures": failures, "timing": timing,
               "passed": not failures}
    (args.out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps({"passed": not failures, "pair_count": len(pairs), "failure_count": len(failures), "summary": str(args.out / "summary.json")}))
    if failures:
        sys.exit(1)


def parse_event_file(path):
    events = []
    for line in Path(path).read_text(errors="replace").splitlines():
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and isinstance(value.get("event"), str):
            events.append(value)
    return events


if __name__ == "__main__":
    main()
