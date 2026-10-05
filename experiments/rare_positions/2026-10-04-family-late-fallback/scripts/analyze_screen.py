"""Compare full exports, native work, and deferred-family events for R60."""
import argparse
import hashlib
import json
from collections import Counter
from pathlib import Path

CELL_METRICS = (
    "pilot_hits", "pilot_ess", "main_draws", "main_hits", "main_ess", "main_max_share",
    "probability", "check_draws", "check_hits", "check_ess", "check_probability",
    "cost_per_draw", "modeled_main_work", "modeled_check_work", "main_only",
)
CELL_ADMISSIONS = (
    "accepted", "main_publishable", "sampling_gates_passed", "within_grant",
    "neighbor_transfer_attempted", "neighbor_transfer_used", "actual_operation_work",
    "original_grant_work", "expanded_grant_work", "original_draw_cap", "expanded_draw_cap",
    "draw_cap_scale", "budget_multiplier", "added_experiment_allowance",
)
SUMMARY_METRICS = (
    "funded", "completed", "accepted", "main_only", "reclaimed_check_work", "waves",
    "native_reserved_after_waves", "native_reclaimed_check_work", "neighbor_transfer_used",
)


def read(path):
    return json.loads(Path(path).read_text())


def sha(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def matrix(export):
    value = export.get("rare_position_estimates")
    if not isinstance(value, dict) or not isinstance(export.get("game_importance"), dict):
        raise ValueError("export must contain rare_position_estimates and object game_importance")
    result = {}
    for team, ranks in value.items():
        if not isinstance(ranks, dict):
            raise ValueError(f"malformed estimate row for team {team}")
        for rank, estimate in ranks.items():
            key = (str(team), str(rank))
            if key in result or not isinstance(estimate, dict):
                raise ValueError(f"duplicate or malformed rare estimate {key}")
            if "probability" not in estimate or not isinstance(estimate["probability"], (int, float)):
                raise ValueError(f"rare estimate {key} lacks a numeric probability")
            result[key] = estimate
    return result


def normalize_export(export):
    normalized = json.loads(json.dumps(export))
    for ranks in normalized["rare_position_estimates"].values():
        for estimate in ranks.values():
            estimate.pop("work_spent", None)
    return normalized


def canonical_hash(value):
    raw = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    return hashlib.sha256(raw).hexdigest()


def positive_differences(before, after):
    a, b = matrix(before), matrix(after)
    changes = []
    for key in sorted(a.keys() | b.keys()):
        old = a.get(key)
        new = b.get(key)
        if old is not None and new is not None and old.get("probability", 0) > 0 and {k: v for k, v in old.items() if k != "work_spent"} != {k: v for k, v in new.items() if k != "work_spent"}:
            changes.append({"team_id": key[0], "rank_index": key[1], "before": old, "after": new})
    return changes


def gains_losses(before, after):
    a, b = matrix(before), matrix(after)
    gains, losses = [], []
    for key in sorted(a.keys() & b.keys()):
        old, new = a[key]["probability"], b[key]["probability"]
        if old == 0 and new > 0:
            gains.append({"team_id": key[0], "rank_index": key[1], "before": old, "after": new, "estimate_before": a[key], "estimate_after": b[key]})
        elif old > 0 and new == 0:
            losses.append({"team_id": key[0], "rank_index": key[1], "before": old, "after": new, "estimate_before": a[key], "estimate_after": b[key]})
    return gains, losses


def cell_summary(export):
    estimates = matrix(export)
    result = {"cells": len(estimates), "zeros": 0, "positive": 0, "zero_reachability": {}, "zero_cells": []}
    for (team, rank), estimate in estimates.items():
        probability = estimate["probability"]
        if probability > 0:
            result["positive"] += 1
        elif probability == 0:
            result["zeros"] += 1
            reachability = str(estimate.get("reachability", "unknown")).lower()
            result["zero_reachability"][reachability] = result["zero_reachability"].get(reachability, 0) + 1
            result["zero_cells"].append({"team_id": team, "rank_index": rank, "reachability": reachability})
    return result


def changed_positive_metadata(entries):
    statistical_fields = {"probability", "std_err", "samples", "hits", "ess", "mean_weight", "relative_se", "max_event_weight_share", "zero_hit_upper_95"}
    result = []
    for entry in entries:
        before, after = entry["before"], entry["after"]
        changed = sorted(key for key in (before.keys() | after.keys()) if key != "work_spent" and before.get(key) != after.get(key))
        metadata = [key for key in changed if key not in statistical_fields]
        result.append({"team_id": entry["team_id"], "rank_index": entry["rank_index"], "changed_fields_except_work_spent": changed, "metadata_changed_fields": metadata})
    return result


def event_identity(event):
    team = event.get("team_id", event.get("team"))
    rank = event.get("rank_index", event.get("rank"))
    if team is None or rank is None:
        raise ValueError(f"native confirmation event lacks cell identity: {event.get('event')}")
    return f"{team}:{rank}"


def native_metrics(events, require_admissions=False):
    cells, summaries, cell_sampling_ms = {}, [], {}
    for event in events:
        if event.get("event") == "rust_odds_rare_tail_confirm_more":
            key = event_identity(event)
            missing = [name for name in CELL_METRICS if name not in event]
            if require_admissions:
                missing.extend(name for name in CELL_ADMISSIONS if name not in event)
            if missing:
                raise ValueError(f"native confirmation {key} lacks metrics {missing}")
            if key in cells:
                raise ValueError(f"duplicate native confirmation identity {key}")
            cells[key] = {name: event[name] for name in CELL_METRICS}
            cells[key].update({name: event[name] for name in CELL_ADMISSIONS if name in event})
            cell_sampling_ms[key] = event.get("sampling_ms")
        elif event.get("event") == "rust_odds_rare_tail_confirm_more_summary":
            if require_admissions:
                missing = [name for name in SUMMARY_METRICS if name not in event]
                if missing:
                    raise ValueError(f"native confirmation summary lacks admission/budget metrics {missing}")
            values = {name: event[name] for name in SUMMARY_METRICS if name in event}
            values.update({name: value for name, value in event.items() if name.startswith(("wave1_", "wave2_", "first_wave_", "second_wave_"))})
            summaries.append(values)
    return {"cells": cells, "summaries": summaries, "cell_sampling_ms": cell_sampling_ms}


def empty_search_evidence(events):
    if any(e.get("event") in ("rust_odds_rare_tail_confirm_more", "rust_odds_family_fallback", "rust_odds_family_fallback_late_skip", "rust_odds_family_native_training") for e in events):
        return {"proven": False, "reason": "native confirmation or fallback events are present"}
    confirmations = [e for e in events if e.get("event") == "rust_odds_rare_tail_confirm_more_summary"]
    if confirmations:
        proven = len(confirmations) == 1 and confirmations[0].get("eligible") == 0
        return {"proven": proven, "source": "confirmation-summary-eligible-count", "summary_count": len(confirmations), "eligible": confirmations[0].get("eligible") if len(confirmations) == 1 else None}
    screens = [e for e in events if e.get("event") == "rust_odds_stage" and e.get("stage") == "search.screen"]
    finals = [e for e in events if e.get("event") == "rust_odds_stage" and e.get("stage") == "search"]
    early = [e for e in events if e.get("event") == "rust_odds_stage" and e.get("stage") == "search.early_proofs"]
    if not (len(screens) == len(finals) == len(early) == 1):
        return {"proven": False, "source": "search-stage-events", "screen_count": len(screens), "final_count": len(finals), "early_proof_count": len(early)}
    screen, final, early_event = screens[0], finals[0], early[0]
    scope = ("group", "seed", "request_id")
    same_scope = all(screen.get(key) is not None and screen.get(key) == final.get(key) == early_event.get(key) for key in scope)
    counts_match = isinstance(screen.get("cells"), dict) and screen.get("cells") == final.get("cells")
    counts = screen.get("cells", {})
    zero_counts = all(counts.get(key) == 0 for key in ("zeros", "reachable_zero", "undecided_zero", "impossible_zero"))
    no_candidates = screen.get("candidate_cells") == 0 and final.get("candidate_cells", 0) == 0
    no_work = final.get("work") == 0 and early_event.get("proofs") == 0
    return {"proven": same_scope and counts_match and zero_counts and no_candidates and no_work,
            "source": "matching-empty-search-screen-and-final-counts", "same_request_scope": same_scope,
            "screen_candidate_cells": screen.get("candidate_cells"), "final_candidate_cells": final.get("candidate_cells"),
            "screen_cell_counts": counts, "final_cell_counts": final.get("cells"), "screen_final_cell_counts_match": counts_match,
            "no_zero_cells": zero_counts, "final_work": final.get("work"), "early_proofs": early_event.get("proofs")}


def no_native_work_evidence(events_by_arm):
    evidence = {arm: empty_search_evidence(events) for arm, events in events_by_arm.items()}
    return {"proven": all(item.get("proven") for item in evidence.values()), "by_arm": evidence}


def verify_record(record, request, binary):
    for key, path in (("binary", binary), ("request", request), ("export", Path(record["export"])), ("stdout", Path(record["stdout"])), ("stderr", Path(record["stderr"]))):
        if not path.is_file():
            raise ValueError(f"record {record.get('arm')}/seed{record.get('seed')} missing {key} file: {path}")
        expected = record.get(f"{key}_sha256")
        if not expected or sha(path) != expected:
            raise ValueError(f"record {record.get('arm')}/seed{record.get('seed')} {key} hash mismatch: {path}")
    if record.get("binary_sha256") != sha(binary) or Path(record["binary"]).resolve() != binary.resolve():
        raise ValueError(f"record binary identity/hash mismatch for {record.get('arm')} seed {record.get('seed')}")
    if record.get("request_sha256") != sha(request) or Path(record["request"]).resolve() != request.resolve():
        raise ValueError(f"record request identity/hash mismatch for {record.get('arm')} seed {record.get('seed')}")


def deferred_fallback_audit(events, no_work_proven=False):
    confirm = [e for e in events if e.get("event") == "rust_odds_rare_tail_confirm_more_summary"]
    late = [e for e in events if e.get("event") == "rust_odds_family_fallback_late_summary"]
    skips = [e for e in events if e.get("event") == "rust_odds_family_fallback_late_skip"]
    fallbacks = [e for e in events if e.get("event") == "rust_odds_family_fallback"]
    if len(late) != 1:
        if no_work_proven and not late and not skips and not fallbacks:
            return {"schema_complete": True, "status": "not_applicable_no_native_or_late_fallback_work", "all_accounting_checks_match": None, "late_summary_count": 0}
        return {"schema_complete": False, "reason": "expected one late fallback summary", "late_summary_count": len(late), "skip_events": skips, "fallback_events": fallbacks}
    late_summary = late[0]
    skip_reasons = Counter(str(e.get("reason", "missing")) for e in skips)
    charged, granted, published_charges = [], [], []
    fallback_contract_violations = []
    for event in fallbacks:
        record = event.get("fallback", {})
        amount = record.get("added_actual_work")
        if not isinstance(amount, int) or amount < 0:
            return {"schema_complete": False, "reason": "fallback record lacks nonnegative added_actual_work", "late_summary": late_summary, "skip_events": skips, "fallback_events": fallbacks}
        charged.append(amount)
        grant = record.get("fallback_grant")
        if not isinstance(grant, int) or grant < 0:
            return {"schema_complete": False, "reason": "fallback record lacks nonnegative fallback_grant", "late_summary": late_summary, "skip_events": skips, "fallback_events": fallbacks}
        granted.append(grant)
        if record.get("published") is True:
            published_charges.append(amount)
        if record.get("multiplier") != 1 or record.get("expanded_allowance") != 0 or grant != record.get("base_fallback_grant") or amount > grant or record.get("settlement_within_grant") is not True:
            fallback_contract_violations.append({"team_id": event.get("team_id"), "rank": event.get("rank"), "multiplier": record.get("multiplier"), "expanded_allowance": record.get("expanded_allowance"), "base_fallback_grant": record.get("base_fallback_grant"), "fallback_grant": grant, "added_actual_work": amount, "settlement_within_grant": record.get("settlement_within_grant")})
    summary_added = late_summary.get("added_actual_work")
    handoff = late_summary.get("handoff_fee")
    attempted = late_summary.get("attempted")
    skipped = late_summary.get("skipped_superseded_or_impossible")
    required = ("funding_source", "native_training_funding", "native_training_observer_reserved", "native_training_observer_actual", "native_training_fee_already_charged", "native_training_stage_spare_before_funding", "native_training_observer_bound", "inherited_bank_limit", "inherited_bank_reserved_after_native", "inherited_bank_free_before_handoff_fee", "bank_reserved_at_late_entry", "bank_free_at_late_entry", "attempted", "skipped_superseded_or_impossible", "unsupported_skips", "quality_declines", "budget_declines", "order_setup_work", "granted_work", "settled_fallback_work", "unused_grant_work", "published_charged_work", "published", "multiplier", "scope", "mode", "elapsed_ms", "final_bank_limit", "final_bank_reserved", "final_bank_free")
    missing = [key for key in required if key not in late_summary]
    if summary_added is None or handoff is None or attempted is None or skipped is None:
        missing.extend(key for key, val in (("added_actual_work", summary_added), ("handoff_fee", handoff), ("attempted", attempted), ("skipped_superseded_or_impossible", skipped)) if val is None)
    if missing:
        return {"schema_complete": False, "reason": "late fallback summary missing contract fields", "missing_fields": sorted(set(missing)), "late_summary": late_summary, "skip_events": skips, "fallback_events": fallbacks}
    native = confirm[0] if len(confirm) == 1 else {}
    initial_fallback = native.get("family_fallback_added_work")
    initial_attempts = native.get("family_fallback_attempts")
    fee_required = native.get("family_fallback_handoff_fee_required")
    fee_native = native.get("family_fallback_handoff_fee")
    deferred_states = native.get("family_fallback_late_deferred_states")
    order_events = [e for e in events if e.get("event") == "rust_odds_family_fallback_order"]
    order_event_work = sum(int(e.get("order_setup_work", 0)) for e in order_events)
    fallback_actual = sum(charged)
    fallback_grants = sum(granted)
    order_work = int(late_summary["order_setup_work"])
    checks = {
        "native_phase_fallback_work_is_zero": initial_fallback == 0,
        "native_phase_fallback_attempts_are_zero": initial_attempts == 0,
        "native_handoff_fee_is_funded_and_nonnegative": native.get("family_fallback_handoff_fee_funded") is True and isinstance(fee_native, int) and fee_native >= 0,
        "late_observer_funding_is_stage_based": late_summary.get("native_training_funding") == "stage" and native.get("native_family_training_funding") == "stage",
        "late_observer_actual_matches_native_summary": late_summary.get("native_training_observer_actual") == native.get("native_family_training_observer_actual"),
        "late_observer_is_not_double_reserved_in_confirmation_bank": late_summary.get("native_training_observer_reserved") == 0 and native.get("native_family_training_confirmation_observer_reserved") == 0,
        "late_stage_fee_matches_native_stage_dispatch_charge": late_summary.get("native_training_fee_already_charged") == native.get("native_family_training_stage_dispatch_fee_charged"),
        "deferred_state_count_is_positive_when_handoff_funded": isinstance(deferred_states, int) and deferred_states > 0,
        "late_handoff_fee_matches_native_confirmation_fee": handoff == fee_native == fee_required,
        "late_attempt_count_matches_fallback_records": int(attempted) == len(fallbacks),
        "late_skip_count_matches_skip_events": int(skipped) == len(skips),
        "skip_reason_counts_match_skip_events": sum(skip_reasons.values()) == len(skips),
        "unsupported_skip_count_is_nonnegative": isinstance(late_summary.get("unsupported_skips"), int) and late_summary["unsupported_skips"] >= 0,
        "quality_and_budget_declines_are_nonnegative": all(isinstance(late_summary.get(key), int) and late_summary[key] >= 0 for key in ("quality_declines", "budget_declines")),
        "late_actual_matches_fallback_records_and_order_setup": int(summary_added) == fallback_actual + order_work,
        "late_order_setup_matches_order_event": order_work == order_event_work,
        "settled_fallback_work_matches_attempt_charges": int(late_summary["settled_fallback_work"]) == fallback_actual,
        "granted_work_matches_record_grants": int(late_summary["granted_work"]) == fallback_grants,
        "unused_grant_matches_granted_minus_settled": int(late_summary["unused_grant_work"]) == fallback_grants - fallback_actual,
        "published_charged_work_matches_published_records": int(late_summary["published_charged_work"]) == sum(published_charges),
        "one_x_original_grants_respected": not fallback_contract_violations,
        "late_attempt_scope_is_all": late_summary.get("scope") == "all",
        "late_mode_is_family": late_summary.get("mode") == "family",
        "late_multiplier_is_one": late_summary.get("multiplier") == 1,
        "late_funding_source_is_inherited_bank_and_original_grants": late_summary.get("funding_source") == "existing_confirmation_bank_and_original_cell_grants",
    }
    checks["late_inherited_limit_matches_native_limit"] = len(confirm) == 1 and late_summary["inherited_bank_limit"] == native.get("work_limit")
    checks["late_inherited_reservation_matches_native_reservation"] = len(confirm) == 1 and late_summary["inherited_bank_reserved_after_native"] == native.get("native_family_training_full_bank_reserved_after_waves")
    checks["free_before_fee_matches_inherited_bank"] = late_summary["inherited_bank_free_before_handoff_fee"] == max(0, late_summary["inherited_bank_limit"] - late_summary["inherited_bank_reserved_after_native"])
    checks["late_entry_reservation_includes_handoff_once"] = late_summary["bank_reserved_at_late_entry"] == late_summary["inherited_bank_reserved_after_native"] + handoff
    checks["late_entry_free_matches_limit_minus_reservation"] = late_summary["bank_free_at_late_entry"] == max(0, late_summary["inherited_bank_limit"] - late_summary["bank_reserved_at_late_entry"])
    checks["late_bank_limit_unchanged_at_one_x"] = late_summary["final_bank_limit"] == late_summary["inherited_bank_limit"]
    checks["final_bank_free_matches_limit_minus_reservation"] = late_summary["final_bank_free"] == max(0, late_summary["final_bank_limit"] - late_summary["final_bank_reserved"])
    checks["final_reservation_increases_by_late_actual_work_only"] = late_summary["final_bank_reserved"] == late_summary["bank_reserved_at_late_entry"] + int(summary_added)
    for field in ("inherited_bank_limit", "inherited_bank_reserved_after_native", "inherited_bank_free_before_handoff_fee", "bank_reserved_at_late_entry", "bank_free_at_late_entry", "final_bank_limit", "final_bank_reserved", "final_bank_free"):
        checks[f"{field}_is_nonnegative_integer"] = isinstance(late_summary[field], int) and late_summary[field] >= 0
    return {
        "schema_complete": True, "checks": checks, "all_accounting_checks_match": all(checks.values()),
        "native_confirmation_summary": native if len(confirm) == 1 else None,
        "late_summary": late_summary, "skip_count_by_reason": dict(skip_reasons),
        "skip_events": skips, "fallback_event_count": len(fallbacks), "fallback_added_actual_work_sum": fallback_actual,
        "fallback_granted_work_sum": fallback_grants, "fallback_contract_violations": fallback_contract_violations,
        "handoff_fee": handoff, "late_added_actual_work": summary_added,
        "once_only_components": {"native_confirmation_summary_fallback_work": initial_fallback, "native_confirmation_summary_attempts": initial_attempts, "native_deferred_state_count": deferred_states, "native_handoff_fee": fee_native, "late_handoff_fee": handoff, "late_fallback_actual_work": summary_added},
    }


def cross_phase_work_audit(parent_events, candidate_events, late_audit):
    parent_native = [e for e in parent_events if e.get("event") == "rust_odds_rare_tail_confirm_more_summary"]
    candidate_native = [e for e in candidate_events if e.get("event") == "rust_odds_rare_tail_confirm_more_summary"]
    parent_final = [e for e in parent_events if e.get("event") == "rust_odds_rare_tail_summary"]
    candidate_final = [e for e in candidate_events if e.get("event") == "rust_odds_rare_tail_summary"]
    if not (len(parent_native) == len(candidate_native) == len(parent_final) == len(candidate_final) == 1):
        return {"schema_complete": False, "reason": "expected one native and one final rare-tail summary for both active arms"}
    p, c = parent_native[0], candidate_native[0]
    pf, cf = parent_final[0], candidate_final[0]
    late = late_audit.get("late_summary") if isinstance(late_audit, dict) else None
    if not isinstance(late, dict):
        return {"schema_complete": False, "reason": "late summary unavailable for cross-phase accounting"}
    p_work, c_work = p.get("work"), c.get("work")
    p_fallback, c_fallback = p.get("family_fallback_added_work"), c.get("family_fallback_added_work")
    c_fee = c.get("family_fallback_handoff_fee")
    p_final_work, c_final_work = pf.get("work"), cf.get("work")
    late_work = late.get("added_actual_work")
    checks = {
        "candidate_native_phase_fallback_work_is_zero": c_fallback == 0,
        "parent_candidate_native_legacy_work_matches_after_fallback_or_handoff": isinstance(p_work, int) and isinstance(c_work, int) and isinstance(p_fallback, int) and isinstance(c_fee, int) and p_work - p_fallback == c_work - c_fee,
        "late_fee_matches_candidate_native_fee": isinstance(c_fee, int) and c_fee == late.get("handoff_fee"),
        "parent_candidate_final_rare_tail_work_delta_matches_fallback_reallocation": all(isinstance(v, int) for v in (p_final_work, c_final_work, p_fallback, late_work, c_fee)) and p_final_work - c_final_work == p_fallback - late_work - c_fee,
    }
    return {"schema_complete": True, "checks": checks, "all_checks_match": all(checks.values()),
            "parent_native_work": p_work, "candidate_native_work": c_work,
            "parent_native_fallback_work": p_fallback, "candidate_handoff_fee": c_fee,
            "parent_final_rare_tail_work": p_final_work, "candidate_final_rare_tail_work": c_final_work,
            "candidate_late_fallback_work": late_work}
def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--screen", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--out", required=True, type=lambda s: Path(s).resolve())
    p.add_argument("--parent-arm", default="parent-r59")
    p.add_argument("--candidate-arm", default="candidate-r60")
    args = p.parse_args()
    if args.out.exists():
        p.error(f"refusing existing output file: {args.out}")
    summary = read(args.screen / "summary.json")
    request = Path(summary["request"]).resolve(strict=True)
    arm_binaries = {arm: Path(value["path"]).resolve(strict=True) for arm, value in summary["binaries"].items()}
    required_arms = ("baseline", args.parent_arm, args.candidate_arm, "candidate-flag-off")
    if any(arm not in arm_binaries for arm in required_arms):
        p.error(f"screen summary must contain arms {required_arms}")
    rows, all_events = [], []
    for seed in summary["seeds"]:
        records = {arm: read(args.screen / arm / f"seed{seed}" / "record.json") for arm in required_arms}
        for arm, record in records.items():
            verify_record(record, request, arm_binaries[arm])
        if records["baseline"].get("workers") != records[args.parent_arm].get("workers") or records["baseline"].get("workers") != records[args.candidate_arm].get("workers"):
            raise ValueError(f"worker count differs across active arms for seed {seed}")
        exports = {arm: read(record["export"]) for arm, record in records.items()}
        matrices = {arm: matrix(export) for arm, export in exports.items()}
        identities = {arm: set(value) for arm, value in matrices.items()}
        if any(not value for value in identities.values()):
            raise ValueError(f"empty rare-position matrix for seed {seed}")
        identity_parity = identities["baseline"] == identities[args.parent_arm] == identities[args.candidate_arm]
        if not identity_parity:
            raise ValueError(f"matrix identities differ across arms for seed {seed}")
        event_sets = {arm: records[arm].get("events", []) for arm in records}
        empty_evidence = no_native_work_evidence(event_sets)
        native = {arm: native_metrics(records[arm].get("events", []), require_admissions=arm in (args.parent_arm, args.candidate_arm)) for arm in records}
        native_content = {arm: {"cells": data["cells"], "summaries": data["summaries"]} for arm, data in native.items()}
        all_native_empty = all(not native_content[arm]["cells"] and not native_content[arm]["summaries"] for arm in required_arms)
        native_not_applicable = all_native_empty and empty_evidence["proven"]
        if all_native_empty and not native_not_applicable:
            raise ValueError(f"native confirmation metrics absent without proof of empty search for seed {seed}: {empty_evidence}")
        native_parent_candidate_exact = None if native_not_applicable else native_content[args.parent_arm] == native_content[args.candidate_arm]
        native_baseline_parent_exact = None if native_not_applicable else native_content["baseline"] == native_content[args.parent_arm]
        if not native_parent_candidate_exact:
            if native_parent_candidate_exact is False:
                raise ValueError(f"native confirmation metrics/admissions differ between parent and candidate for seed {seed}")
        gains_parent, losses_parent = gains_losses(exports["baseline"], exports[args.parent_arm])
        gains_candidate, losses_candidate = gains_losses(exports["baseline"], exports[args.candidate_arm])
        gains_vs_parent, losses_vs_parent = gains_losses(exports[args.parent_arm], exports[args.candidate_arm])
        positive_parent = positive_differences(exports["baseline"], exports[args.parent_arm])
        positive_candidate = positive_differences(exports["baseline"], exports[args.candidate_arm])
        exact_importance = {arm: exports[arm]["game_importance"] == exports["baseline"]["game_importance"] for arm in (args.parent_arm, args.candidate_arm)}
        raw_equal = {arm: Path(records[arm]["export"]).read_bytes() == Path(records["baseline"]["export"]).read_bytes() for arm in (args.parent_arm, args.candidate_arm)}
        normalized_hashes = {arm: canonical_hash(normalize_export(exports[arm])) for arm in exports}
        flag_off_byte_equal = Path(records["candidate-flag-off"]["export"]).read_bytes() == Path(records["baseline"]["export"]).read_bytes()
        if not flag_off_byte_equal:
            raise ValueError(f"candidate flag-off export differs byte-for-byte from current Rust for seed {seed}")
        late_audit = deferred_fallback_audit(records[args.candidate_arm].get("events", []), no_work_proven=empty_evidence["by_arm"].get(args.candidate_arm, {}).get("proven", False))
        if late_audit.get("schema_complete") and late_audit.get("status") != "not_applicable_no_native_or_late_fallback_work" and not late_audit.get("all_accounting_checks_match", False):
            raise ValueError(f"late fallback accounting failed for seed {seed}: {late_audit.get('checks')}")
        if late_audit.get("status") == "not_applicable_no_native_or_late_fallback_work":
            cross_phase = {"schema_complete": True, "status": "not_applicable_proven_empty_search", "all_checks_match": None}
        else:
            cross_phase = cross_phase_work_audit(records[args.parent_arm].get("events", []), records[args.candidate_arm].get("events", []), late_audit)
            if not cross_phase.get("schema_complete") or not cross_phase.get("all_checks_match"):
                raise ValueError(f"cross-phase work accounting failed for seed {seed}: {cross_phase}")
        timing = {}
        for arm in (args.parent_arm, args.candidate_arm):
            native_summaries = [e for e in records[arm].get("events", []) if e.get("event") == "rust_odds_rare_tail_confirm_more_summary"]
            timing[arm] = native_summaries[0].get("elapsed_ms") if len(native_summaries) == 1 else None
        late_summaries = [e for e in records[args.candidate_arm].get("events", []) if e.get("event") == "rust_odds_family_fallback_late_summary"]
        late_elapsed = late_summaries[0].get("elapsed_ms") if len(late_summaries) == 1 else None
        candidate_combined_elapsed = timing[args.candidate_arm] + late_elapsed if isinstance(timing[args.candidate_arm], (int, float)) and isinstance(late_elapsed, (int, float)) else None
        native_sampling = {arm: data["cell_sampling_ms"] for arm, data in native.items()}
        positive_metadata = {"parent": changed_positive_metadata(positive_parent), "candidate": changed_positive_metadata(positive_candidate)}
        for arm in (args.parent_arm, args.candidate_arm):
            all_events.extend({"seed": seed, "arm": arm, **event} for event in records[arm].get("events", []))
        rows.append({
            "seed": seed, "matrix_identity_parity": identity_parity,
            "record_artifacts_verified": True,
            "normalized_full_export_sha256": normalized_hashes,
            "raw_full_export_byte_equality_to_baseline": raw_equal,
            "candidate_flag_off_byte_identical_to_current_rust": flag_off_byte_equal,
            "normalized_full_export_equal_to_baseline": {arm: normalized_hashes[arm] == normalized_hashes["baseline"] for arm in (args.parent_arm, args.candidate_arm)},
            "game_importance_exact_to_baseline": exact_importance,
            "baseline_positive_estimates_exact_except_work_spent": {"parent": not positive_parent, "candidate": not positive_candidate},
            "parent_positive_differences": positive_parent, "candidate_positive_differences": positive_candidate,
            "baseline_positive_metadata_differences": positive_metadata,
            "parent_gains_vs_baseline": gains_parent, "parent_losses_vs_baseline": losses_parent,
            "candidate_gains_vs_baseline": gains_candidate, "candidate_losses_vs_baseline": losses_candidate,
            "candidate_gains_vs_parent": gains_vs_parent, "candidate_losses_vs_parent": losses_vs_parent,
            "matrix_summary": {arm: cell_summary(exports[arm]) for arm in exports},
            "native_confirmation_parent_candidate_exact": native_parent_candidate_exact,
            "native_confirmation_baseline_parent_exact": native_baseline_parent_exact,
            "native_confirmation_status": "not_applicable_proven_empty_search" if native_not_applicable else "compared",
            "native_no_work_evidence": empty_evidence,
            "native_confirmation_metrics": native,
            "native_cell_sampling_ms_by_arm": native_sampling,
            "native_and_late_stage_timing_ms": {"parent_native_summary_elapsed_ms_including_immediate_fallback": timing[args.parent_arm], "candidate_native_summary_elapsed_ms_before_late_fallback": timing[args.candidate_arm], "candidate_late_fallback_summary_elapsed_ms": late_elapsed, "candidate_native_plus_late_elapsed_ms": candidate_combined_elapsed, "candidate_combined_minus_parent_ms": candidate_combined_elapsed - timing[args.parent_arm] if candidate_combined_elapsed is not None and isinstance(timing[args.parent_arm], (int, float)) else None, "native_final_branch_cell_sampling_ms_by_arm": native_sampling},
            "deferred_fallback_audit": late_audit,
            "cross_phase_work_audit": cross_phase,
            "raw_event_diagnostics": {arm: records[arm].get("events", []) for arm in (args.parent_arm, args.candidate_arm)},
        })
    result = {
        "seeds": summary["seeds"], "arms": summary["arms"], "screen_summary": str(args.screen / "summary.json"),
        "native_metrics_compare_only_reported_not_statistical_calibration": True,
        "all_matrix_identity_parity": all(row["matrix_identity_parity"] for row in rows),
        "all_game_importance_exact_to_baseline": all(all(row["game_importance_exact_to_baseline"].values()) for row in rows),
        "all_baseline_positive_estimates_preserved_parent": all(row["baseline_positive_estimates_exact_except_work_spent"]["parent"] for row in rows),
        "all_baseline_positive_estimates_preserved_candidate": all(row["baseline_positive_estimates_exact_except_work_spent"]["candidate"] for row in rows),
        "all_native_confirmation_parent_candidate_exact": None if all(row["native_confirmation_parent_candidate_exact"] is None for row in rows) else all(row["native_confirmation_parent_candidate_exact"] is True for row in rows),
        "all_deferred_fallback_ledgers_complete_and_consistent": all(row["deferred_fallback_audit"].get("status") == "not_applicable_no_native_or_late_fallback_work" or row["deferred_fallback_audit"].get("all_accounting_checks_match", False) for row in rows),
        "all_cross_phase_work_ledgers_complete_and_consistent": all(row["cross_phase_work_audit"].get("status") == "not_applicable_proven_empty_search" or row["cross_phase_work_audit"].get("all_checks_match", False) for row in rows),
        "runs": rows, "parent_and_candidate_event_diagnostics": all_events,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
