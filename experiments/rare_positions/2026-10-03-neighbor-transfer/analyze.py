#!/usr/bin/env python3
"""Analyze paired R54 estimates, per-cell changes, timing, and native invariants."""
from __future__ import annotations
import argparse
import csv
import hashlib
import json
from pathlib import Path
import statistics

TIMING_KEYS = {"elapsed_ms", "request_elapsed_ms", "timestamp_unix_ms", "setup_ms", "pilot_ms", "main_ms", "check_ms", "request_id"}
CAPTURE_TOKENS = ("neighbor", "transfer", "donor", "capture", "captured")
CONFIRM_TOKENS = ("confirm", "confirmation")


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def digest(value: object) -> str:
    return hashlib.sha256(canonical(value)).hexdigest()


def initial_mc(estimate: dict) -> dict:
    return {key: estimate.get(key) for key in ("samples", "hits")}


def scrub_diagnostic(value: object) -> object:
    if isinstance(value, dict):
        kept = {}
        for key, child in value.items():
            lower = str(key).casefold()
            if key in TIMING_KEYS or lower.endswith("_ms") or "duration" in lower:
                continue
            kept[key] = scrub_diagnostic(child)
        return kept
    if isinstance(value, list):
        return [scrub_diagnostic(x) for x in value]
    return value


def events_from_run(run: dict, pair_dir: Path) -> list[dict]:
    path = pair_dir / run["stderr_log"]
    events = []
    for line in path.read_text(errors="replace").splitlines():
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(item, dict) and isinstance(item.get("event"), str):
            events.append(item)
    return events


def contains_accounting_status(value: object) -> bool:
    if isinstance(value, dict):
        for key, child in value.items():
            lower = str(key).casefold()
            if any(token in lower for token in ("declin", "unfund", "overrun", "overdraft", "accounting_over")):
                if child not in (None, False, 0, "", [], {}):
                    return True
            if contains_accounting_status(child):
                return True
    elif isinstance(value, list):
        return any(contains_accounting_status(child) for child in value)
    return False


def event_semantic_key(event: dict) -> tuple[str, str, str, str]:
    plan = event.get("pilot", {}).get("plan", {}) if isinstance(event.get("pilot"), dict) else {}
    team = event.get("team", event.get("team_id", plan.get("team", plan.get("team_id", ""))))
    rank = event.get("rank", plan.get("rank", ""))
    stage = event.get("stage", "")
    return str(event.get("event", "")), str(team), str(rank), str(stage)


def events_preconfirmation(events: list[dict]) -> list[dict]:
    # Parallel jobs can emit in different completion order. Compare a stable
    # multiset keyed by event/team/rank/stage, retaining duplicate occurrences.
    grouped: dict[tuple[str, str, str, str], list[dict]] = {}
    for event in events:
        name = event.get("event", "").casefold()
        if any(token in name for token in (*CONFIRM_TOKENS, *CAPTURE_TOKENS)):
            break
        normalized = scrub_diagnostic(event)
        grouped.setdefault(event_semantic_key(normalized), []).append(normalized)
    result = []
    for key in sorted(grouped):
        # Sort only within an identical semantic key to remove scheduling noise;
        # duplicates remain as separate occurrence rows.
        rows = sorted(grouped[key], key=canonical)
        for occurrence, row in enumerate(rows, 1):
            result.append({"key": {"event": key[0], "team": key[1], "rank": key[2], "stage": key[3]},
                           "occurrence": occurrence, "record": row})
    return result


def first_differences(left: object, right: object, path: str = "$", limit: int = 12) -> list[dict]:
    out: list[dict] = []
    def walk(a: object, b: object, p: str) -> None:
        if len(out) >= limit or a == b:
            return
        if isinstance(a, dict) and isinstance(b, dict):
            for key in sorted(set(a) | set(b)):
                if len(out) >= limit:
                    break
                if key not in a or key not in b:
                    out.append({"path": f"{p}.{key}", "baseline": a.get(key), "candidate": b.get(key)})
                else:
                    walk(a[key], b[key], f"{p}.{key}")
        elif isinstance(a, list) and isinstance(b, list):
            if len(a) != len(b):
                out.append({"path": f"{p}.length", "baseline": len(a), "candidate": len(b)})
            for i, (av, bv) in enumerate(zip(a, b)):
                if len(out) >= limit:
                    break
                walk(av, bv, f"{p}[{i}]")
        else:
            out.append({"path": p, "baseline": a, "candidate": b})
    walk(left, right, path)
    return out


def flatten_cell_rows(export: dict) -> dict[tuple[str, str], dict]:
    result = {}
    for team, ranks in (export.get("rare_position_estimates") or {}).items():
        for rank, estimate in ranks.items():
            result[(str(team), str(rank))] = estimate
    return result


def summarize_estimate(e: dict | None) -> dict:
    e = e or {}
    return {key: e.get(key) for key in (
        "probability", "std_err", "samples", "hits", "ess", "mean_weight", "work_spent",
        "available", "meets_precision_goal", "relative_se", "max_event_weight_share",
        "zero_hit_upper_95", "design", "reachability", "conditional_mass",
        "conditional_samples", "conditional_hits",
    )}


def target_event(event: dict, team_id: str, rank_1_based: int) -> bool:
    name = str(event.get("event", ""))
    if name == "rust_odds_rare_tail_confirm_more_summary":
        return True
    selector = event.get("selector") if isinstance(event.get("selector"), dict) else {}
    row_team = event.get("team_id", event.get("team", selector.get("team_id")))
    row_rank = event.get("rank", selector.get("rank"))
    try:
        team_match = str(int(row_team)) == team_id
    except (TypeError, ValueError):
        team_match = str(row_team) == team_id
    if row_rank is None:
        return False
    try:
        rank_match = int(row_rank) == rank_1_based
    except (TypeError, ValueError):
        rank_match = False
    return team_match and rank_match


def target_settlement(events: list[dict], estimate: dict, team_id: str, rank_1_based: int) -> dict:
    names = {
        "rust_odds_neighbor_native_control",
        "rust_odds_neighbor_transfer",
        "rust_odds_neighbor_transfer_decline",
        "rust_odds_rare_tail_confirm_more",
        "rust_odds_rare_tail_confirm_more_summary",
    }
    selected = [event for event in events
                if event.get("event") in names and target_event(event, team_id, rank_1_based)]
    controls = [x for x in selected if x.get("event") == "rust_odds_neighbor_native_control"]
    transfers = [x for x in selected if str(x.get("event", "")).startswith("rust_odds_neighbor_transfer")]
    final = [x for x in selected if x.get("event") == "rust_odds_rare_tail_confirm_more"]
    summaries = [x for x in selected if x.get("event") == "rust_odds_rare_tail_confirm_more_summary"]
    event_grant = controls[-1] if controls else (transfers[-1] if transfers else {})
    outcome = final[-1] if final else {}
    expanded = event_grant.get("expanded_grant_work", event_grant.get("grant_work"))
    original = event_grant.get("original_grant_work", event_grant.get("baseline_grant_work"))
    added = event_grant.get("added_experiment_allowance", event_grant.get("additional_allowance_work"))
    actual = outcome.get("actual_operation_work")
    transfer_result = next((x for x in reversed(transfers) if x.get("event") == "rust_odds_neighbor_transfer"), {})
    transfer_pilot_ess = transfer_result.get("pilot_ess", next((x.get("pilot_ess") for x in reversed(transfers) if x.get("pilot_ess") is not None), None))
    return {
        "control_events": controls,
        "transfer_events": transfers,
        "final_confirmation_events": final,
        "group_summary_events": summaries,
        "baseline_grant_work": original,
        "expanded_grant_work": expanded,
        "separately_funded_added_allowance_work": added,
        "pilot_ess": event_grant.get("pilot_ess"),
        "pilot_hits": event_grant.get("pilot_hits"),
        "transfer_pilot_ess": transfer_pilot_ess,
        "transfer_pilot_ess_gate_passed": (transfer_pilot_ess >= 1.5) if isinstance(transfer_pilot_ess, (int, float)) else None,
        "transfer_plan_replaced": transfer_result.get("plan_replaced"),
        "sampling_gates_passed": outcome.get("sampling_gates_passed"),
        "within_expanded_grant": outcome.get("within_grant"),
        "accepted_after_settlement": outcome.get("accepted"),
        "actual_operation_work": actual,
        "unspent_expanded_grant_work": (expanded - actual) if isinstance(expanded, (int, float)) and isinstance(actual, (int, float)) else None,
        "overrun_work": max(0, actual - expanded) if isinstance(expanded, (int, float)) and isinstance(actual, (int, float)) else None,
        "published_in_final_export": estimate.get("design") == "matched_point_pool_rare_tail_confirm_more",
        "final_export_estimate": summarize_estimate(estimate),
    }


def capture_accounting(events: list[dict]) -> dict:
    transfer_capture = sum(
        int(event.get("capture_work", 0) or 0)
        for event in events
        if str(event.get("event", "")).startswith("rust_odds_neighbor_transfer")
    )
    transfer_unfunded = sum(
        int(event.get("unfunded_capture_work", 0) or 0)
        for event in events
        if str(event.get("event", "")).startswith("rust_odds_neighbor_transfer")
    )
    confirmation = [event for event in events if event.get("event") == "rust_odds_rare_tail_confirm_more_summary"]
    post = [event for event in events if event.get("event") == "rust_odds_rare_tail_summary" and event.get("extension") == "after"]
    confirmation_capture = sum(int(event.get("capture_work", 0) or 0) for event in confirmation)
    post_capture = sum(int(event.get("neighbor_capture_work", 0) or 0) for event in post)
    return {
        "transfer_event_capture_work": transfer_capture,
        "transfer_event_unfunded_capture_work": transfer_unfunded,
        "confirm_more_summary_capture_work": confirmation_capture,
        "post_confirmation_rare_tail_capture_work": post_capture,
        "post_confirmation_capture_delta_vs_confirm_more_summary": post_capture - confirmation_capture,
        "confirm_more_summary_events": confirmation,
        "post_confirmation_rare_tail_summaries": post,
    }


def focus_estimate(export_path: Path, team_id: str, rank_key: str) -> dict:
    export = json.loads(export_path.read_text())
    return ((export.get("rare_position_estimates") or {}).get(team_id) or {}).get(rank_key) or {}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run_dir", type=Path)
    parser.add_argument("--request", type=Path, required=True)
    parser.add_argument("--output-prefix", type=Path)
    parser.add_argument("--compare-run-dir", type=Path,
                        help="compare matching seeds/modes with an existing paired cohort")
    parser.add_argument("--focus-team-id", type=str, default="17")
    parser.add_argument("--focus-rank-1-based", type=int, default=12)
    args = parser.parse_args()
    run_dir = args.run_dir.resolve(strict=True)
    request = json.loads(args.request.read_text())
    ids = [int(t["team_id"]) for t in request.get("team_groups", [])]
    focus_index = next((i for i, team_id in enumerate(ids) if str(team_id) == args.focus_team_id), None)
    if focus_index is None:
        raise SystemExit(f"team ID {args.focus_team_id} absent from request.team_groups")
    focus_rank_key = str(args.focus_rank_1_based - 1)
    summary = json.loads((run_dir / "summary.json").read_text())
    pairs_summary = []
    cell_changes = []
    focus_rows = []
    native_checks = []
    admission_events = []
    custom_events = []
    positive_initial_zero = {"baseline": 0, "candidate": 0}
    reachability = {arm: {} for arm in ("baseline", "candidate")}
    initial_parity = {"cells": 0, "mismatches": []}
    importance_parity = []
    stage_rows = []
    coverage_gain_ids, coverage_loss_ids = set(), set()
    probability_increase_ids, probability_decrease_ids = set(), set()
    coverage_gain_cells, coverage_loss_cells = set(), set()
    probability_increase_cells, probability_decrease_cells = set(), set()
    run_rows = []
    capture_rows = []
    capture_pair_rows = []

    for pair in summary.get("pairs", []):
        pair_dir = run_dir / pair["pair_id"]
        exports = {}
        event_lists = {}
        for arm, run in pair["runs"].items():
            export_path = pair_dir / run["export"]
            exports[arm] = json.loads(export_path.read_text())
            event_lists[arm] = events_from_run(run, pair_dir)
            for stage, elapsed in run.get("stages_ms", {}).items():
                stage_rows.append({"pair_id": pair["pair_id"], "seed": pair["seed"], "transfer": pair["transfer"], "arm": arm, "stage": stage, "elapsed_ms": elapsed})
            for event in event_lists[arm]:
                ename = event.get("event", "")
                if any(token in ename.casefold() for token in CAPTURE_TOKENS):
                    custom_events.append({"pair_id": pair["pair_id"], "arm": arm, "event": event})
                if any(token in ename.casefold() for token in ("decline", "unfunded", "overrun", "overdraft", "accounting")) or contains_accounting_status(event):
                    admission_events.append({"pair_id": pair["pair_id"], "arm": arm, "event": event})
            capture_rows.append({"pair_id": pair["pair_id"], "seed": pair["seed"],
                                 "transfer": pair["transfer"], "arm": arm,
                                 **capture_accounting(event_lists[arm])})
            run_rows.append({"pair_id": pair["pair_id"], "arm": arm, "seed": pair["seed"], "transfer": pair["transfer"], "workers": pair["workers"], "wall_seconds": run["wall_seconds"], "child_cpu_seconds": run["child_cpu_seconds"], "returncode": run["returncode"], "binary_sha256": run["binary_sha256"]})

        base_capture = capture_accounting(event_lists["baseline"])
        candidate_capture = capture_accounting(event_lists["candidate"])
        capture_pair_rows.append({
            "pair_id": pair["pair_id"], "seed": pair["seed"], "transfer": pair["transfer"],
            "baseline_post_confirmation_capture_work": base_capture["post_confirmation_rare_tail_capture_work"],
            "candidate_post_confirmation_capture_work": candidate_capture["post_confirmation_rare_tail_capture_work"],
            "candidate_minus_baseline_post_confirmation_capture_work": candidate_capture["post_confirmation_rare_tail_capture_work"] - base_capture["post_confirmation_rare_tail_capture_work"],
            "baseline_internal_capture_accounting_delta": base_capture["post_confirmation_capture_delta_vs_confirm_more_summary"],
            "candidate_internal_capture_accounting_delta": candidate_capture["post_confirmation_capture_delta_vs_confirm_more_summary"],
            "candidate_unfunded_capture_work": candidate_capture["transfer_event_unfunded_capture_work"],
        })

        baseline, candidate = exports["baseline"], exports["candidate"]
        before = flatten_cell_rows(baseline)
        after = flatten_cell_rows(candidate)
        all_cells = sorted(set(before) | set(after))
        coverage_gains = coverage_losses = 0
        probability_increases = probability_decreases = 0
        for team, rank in all_cells:
            b, c = before.get((team, rank), {}), after.get((team, rank), {})
            bp = float(b.get("probability", 0.0) or 0.0)
            cp = float(c.get("probability", 0.0) or 0.0)
            delta = cp - bp
            if delta > 0:
                probability_increases += 1
                probability_increase_ids.add((str(pair["seed"]), pair["transfer"], team, rank))
                probability_increase_cells.add((team, rank))
            elif delta < 0:
                probability_decreases += 1
                probability_decrease_ids.add((str(pair["seed"]), pair["transfer"], team, rank))
                probability_decrease_cells.add((team, rank))
            if bp == 0.0 and cp > 0.0:
                coverage_gains += 1
                coverage_gain_ids.add((str(pair["seed"]), pair["transfer"], team, rank))
                coverage_gain_cells.add((team, rank))
            elif bp > 0.0 and cp == 0.0:
                coverage_losses += 1
                coverage_loss_ids.add((str(pair["seed"]), pair["transfer"], team, rank))
                coverage_loss_cells.add((team, rank))
            initial_parity["cells"] += 1
            if initial_mc(b) != initial_mc(c):
                initial_parity["mismatches"].append({"pair_id": pair["pair_id"], "team": team, "rank0": rank, "baseline": initial_mc(b), "candidate": initial_mc(c)})
            if int(b.get("samples", 0) or 0) >= 100_000 and bp > 0 and int(b.get("hits", 0) or 0) == 0:
                positive_initial_zero["baseline"] += 1
            if int(c.get("samples", 0) or 0) >= 100_000 and cp > 0 and int(c.get("hits", 0) or 0) == 0:
                positive_initial_zero["candidate"] += 1
            for arm, e in (("baseline", b), ("candidate", c)):
                status = str(e.get("reachability", "missing"))
                reachability[arm][status] = reachability[arm].get(status, 0) + 1
            cell_changes.append({
                "pair_id": pair["pair_id"], "seed": pair["seed"], "transfer": pair["transfer"],
                "team_id": team, "rank_0_based": rank,
                "baseline_probability": bp, "candidate_probability": cp, "delta": delta,
                "coverage_gain": bp == 0.0 and cp > 0.0,
                "coverage_loss": bp > 0.0 and cp == 0.0,
                "baseline": summarize_estimate(b), "candidate": summarize_estimate(c),
            })
        importance_parity.append({"pair_id": pair["pair_id"], "game_importance_identical": baseline.get("game_importance") == candidate.get("game_importance")})
        norm_base = events_preconfirmation(event_lists["baseline"])
        norm_candidate = events_preconfirmation(event_lists["candidate"])
        event_diffs = first_differences(norm_base, norm_candidate)
        native_checks.append({
            "pair_id": pair["pair_id"], "pre_experiment_events_identical_after_timing_and_request_id_normalization": norm_base == norm_candidate,
            "baseline_event_hash": digest(norm_base), "candidate_event_hash": digest(norm_candidate),
            "baseline_event_count": len(norm_base), "candidate_event_count": len(norm_candidate),
            "first_differences": event_diffs,
            "first_difference_context": (norm_base[int(event_diffs[0]["path"].split("[")[1].split("]")[0])] if event_diffs else None),
        })
        focus_id = str(ids[focus_index])
        focus_b = before.get((focus_id, focus_rank_key), {})
        focus_c = after.get((focus_id, focus_rank_key), {})
        focus_rows.append({
            "pair_id": pair["pair_id"], "seed": pair["seed"], "transfer": pair["transfer"],
            "team_index": focus_index, "team_id": focus_id,
            "rank_1_based": args.focus_rank_1_based,
            "baseline": summarize_estimate(focus_b), "candidate": summarize_estimate(focus_c),
            "delta_probability": float(focus_c.get("probability", 0.0) or 0.0) - float(focus_b.get("probability", 0.0) or 0.0),
            "neighbor_events": [e for e in event_lists["candidate"] if any(token in e.get("event", "").casefold() for token in CAPTURE_TOKENS)],
            "baseline_settlement": target_settlement(event_lists["baseline"], focus_b, focus_id, args.focus_rank_1_based),
            "candidate_settlement": target_settlement(event_lists["candidate"], focus_c, focus_id, args.focus_rank_1_based),
        })
        pairs_summary.append({
            "pair_id": pair["pair_id"], "seed": pair["seed"], "transfer": pair["transfer"],
            "workers": pair["workers"], "cell_count": len(all_cells),
            "coverage_gains": coverage_gains, "coverage_losses": coverage_losses,
            "probability_increases": probability_increases, "probability_decreases": probability_decreases,
            "identical_exports": baseline == candidate,
            "game_importance_identical": baseline.get("game_importance") == candidate.get("game_importance"),
        })

    times_by_arm = {arm: [r["wall_seconds"] for r in run_rows if r["arm"] == arm] for arm in ("baseline", "candidate")}
    cpu_by_arm = {arm: [r["child_cpu_seconds"] for r in run_rows if r["arm"] == arm] for arm in ("baseline", "candidate")}
    result = {
        "request_sha256": hashlib.sha256(args.request.read_bytes()).hexdigest(),
        "production_sha256": summary.get("production_sha256"), "candidate_sha256": summary.get("candidate_sha256"),
        "selector": {"team_index": focus_index, "team_id": focus_id, "rank_1_based": args.focus_rank_1_based, "rank_export_key": focus_rank_key},
        "pairs": pairs_summary,
        "focus_cell": focus_rows,
        "coverage_gains": {"cell_runs": len(coverage_gain_ids), "distinct_cells": len(coverage_gain_cells)},
        "coverage_losses": {"cell_runs": len(coverage_loss_ids), "distinct_cells": len(coverage_loss_cells)},
        "probability_increases": {"cell_runs": len(probability_increase_ids), "distinct_cells": len(probability_increase_cells)},
        "probability_decreases": {"cell_runs": len(probability_decrease_ids), "distinct_cells": len(probability_decrease_cells)},
        "positive_initial_mc_zero_hit_counts": positive_initial_zero,
        "initial_mc_parity": {"cells": initial_parity["cells"], "mismatches": len(initial_parity["mismatches"]), "mismatch_rows": initial_parity["mismatches"]},
        "game_importance_parity": importance_parity,
        "reachability_counts": reachability,
        "native_preconfirmation_diagnostics": native_checks,
        "candidate_admission_events": admission_events,
        "neighbor_capture_events": custom_events,
        "capture_accounting": capture_rows,
        "capture_pair_comparison": capture_pair_rows,
        "median_wall_seconds": {arm: statistics.median(v) if v else None for arm, v in times_by_arm.items()},
        "median_child_cpu_seconds": {arm: statistics.median(v) if v else None for arm, v in cpu_by_arm.items()},
        "run_timings": run_rows,
        "stage_timings": stage_rows,
    }
    if args.compare_run_dir:
        compare_dir = args.compare_run_dir.resolve(strict=True)
        compare_summary = json.loads((compare_dir / "summary.json").read_text())
        matched_rows = []
        for pair in summary.get("pairs", []):
            matching = next((p for p in compare_summary.get("pairs", [])
                             if p.get("seed") == pair.get("seed") and p.get("transfer") == pair.get("transfer")
                             and p.get("label") == pair.get("label")), None)
            if matching is None:
                continue
            target_key = str(args.focus_rank_1_based - 1)
            row = {"pair_id": pair["pair_id"], "seed": pair["seed"], "transfer": pair["transfer"]}
            for arm in ("baseline", "candidate"):
                current_run = matching["runs"].get("baseline" if arm == "baseline" else "candidate")
                high_run = pair["runs"].get(arm)
                if not current_run or not high_run:
                    continue
                current_pair_dir = compare_dir / matching["pair_id"]
                high_pair_dir = run_dir / pair["pair_id"]
                current_est = focus_estimate(current_pair_dir / current_run["export"], focus_id, target_key)
                high_est = focus_estimate(high_pair_dir / high_run["export"], focus_id, target_key)
                row[arm] = {
                    "current_budget": summarize_estimate(current_est),
                    "ten_x": summarize_estimate(high_est),
                    "probability_delta": float(high_est.get("probability", 0.0) or 0.0) - float(current_est.get("probability", 0.0) or 0.0),
                }
            matched_rows.append(row)
        native_by_seed: dict[str, list[str]] = {}
        for pair in summary.get("pairs", []):
            run = pair["runs"].get("baseline")
            if run:
                native_by_seed.setdefault(str(pair["seed"]), []).append(run["export_sha256"])
        result["cross_cohort_comparison"] = {
            "compare_run_dir": str(compare_dir),
            "matching_rows": matched_rows,
            "native_control_repeated_export_identity_by_seed": {
                seed: {"count": len(hashes), "identical": len(set(hashes)) == 1, "hashes": hashes}
                for seed, hashes in sorted(native_by_seed.items())
            },
        }
    prefix = args.output_prefix or (run_dir / "analysis")
    prefix.parent.mkdir(parents=True, exist_ok=True)
    (prefix.with_suffix(".json")).write_text(json.dumps(result, indent=2) + "\n")
    with prefix.with_suffix(".cells.csv").open("w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=["pair_id", "seed", "transfer", "team_id", "rank_0_based", "baseline_probability", "candidate_probability", "delta", "coverage_gain", "coverage_loss"])
        writer.writeheader()
        for row in cell_changes:
            writer.writerow({key: row[key] for key in writer.fieldnames})
    with prefix.with_suffix(".stages.csv").open("w", newline="") as f:
        cols = ["pair_id", "seed", "transfer", "arm", "stage", "elapsed_ms"]
        writer = csv.DictWriter(f, fieldnames=cols); writer.writeheader(); writer.writerows(stage_rows)
    pre = result["native_preconfirmation_diagnostics"]
    pre_summary = {
        "pairs": len(pre),
        "identical": sum(x["pre_experiment_events_identical_after_timing_and_request_id_normalization"] for x in pre),
        "first_mismatch": next(({
            "pair_id": x["pair_id"], "context": x["first_difference_context"],
            "differences": x["first_differences"][:4],
        } for x in pre if x["first_differences"]), None),
    }
    print(json.dumps({**{k: result[k] for k in ("selector", "coverage_gains", "coverage_losses", "probability_increases", "probability_decreases", "positive_initial_mc_zero_hit_counts", "initial_mc_parity", "median_wall_seconds", "median_child_cpu_seconds")}, "native_preconfirmation_summary": pre_summary}, indent=2))
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
