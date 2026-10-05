"""Analyze R59 matrices, observer events, and preservation against baseline and R58."""
import argparse
import json
import math
from collections import Counter
import hashlib
from pathlib import Path


def read(path):
    return json.loads(Path(path).read_text())


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def clean(value):
    """Ignore only top-level estimate work_spent, as the R58 export contract does."""
    return {k: v for k, v in value.items() if k != "work_spent"}


def matrix(export):
    if "rare_position_estimates" not in export or "game_importance" not in export:
        raise ValueError("export lacks rare_position_estimates or game_importance")
    if not isinstance(export["game_importance"], dict):
        raise ValueError("game_importance must be a JSON object")
    return export["rare_position_estimates"]


def cells(export):
    seen = set()
    for team, ranks in matrix(export).items():
        for rank, estimate in ranks.items():
            identity = (str(team), str(rank))
            if identity in seen or not isinstance(estimate, dict):
                raise ValueError(f"duplicate or malformed estimate identity {identity}")
            probability = estimate.get("probability")
            if not isinstance(probability, (int, float)) or not math.isfinite(probability):
                raise ValueError(f"missing or invalid probability for {identity}")
            seen.add(identity)
            yield str(team), str(rank), estimate


def classifications(export):
    counts = Counter()
    identities = set()
    for team, rank, estimate in cells(export):
        identities.add((team, rank))
        probability = estimate.get("probability")
        reachability = str(estimate.get("reachability", "unknown")).lower()
        if probability == 0:
            if reachability in ("reachable", "witness"): counts["reachable_zeros"] += 1
            elif reachability.startswith("impossible") or reachability == "unreachable": counts["impossible"] += 1
            else: counts["undecided"] += 1
    counts["distinct_identities"] = len(identities)
    return dict(counts)


def gains_losses(base, candidate):
    old = {(t, r): e for t, r, e in cells(base)}
    new = {(t, r): e for t, r, e in cells(candidate)}
    gains, losses = [], []
    for key in sorted(old.keys() & new.keys()):
        before, after = old[key].get("probability", 0), new[key].get("probability", 0)
        item = {"team_id": key[0], "rank_index": key[1], "before": before, "after": after}
        if before == 0 and isinstance(after, (int, float)) and after > 0: gains.append(item)
        if isinstance(before, (int, float)) and before > 0 and after == 0: losses.append(item)
    return gains, losses


def team_odds_pos_changes(base, candidate):
    before, after = base.get("team_odds"), candidate.get("team_odds")
    if not isinstance(before, dict) or not isinstance(after, dict) or set(before) != set(after):
        raise ValueError("team_odds identities are missing or differ")
    changed, positive_changes, losses, gains = [], [], [], []
    for team in sorted(before):
        old_pos = before[team].get("Pos") if isinstance(before[team], dict) else None
        new_pos = after[team].get("Pos") if isinstance(after[team], dict) else None
        if not isinstance(old_pos, list) or not isinstance(new_pos, list) or len(old_pos) != len(new_pos):
            raise ValueError(f"team_odds Pos shape missing or changed for team {team}")
        for position, (old, new) in enumerate(zip(old_pos, new_pos)):
            if old != new:
                item = {"team_id": str(team), "rank_index": position, "before": old, "after": new}
                changed.append(item)
                if isinstance(old, (int, float)) and old > 0:
                    positive_changes.append(item)
                    if new == 0:
                        losses.append(item)
                elif old == 0 and isinstance(new, (int, float)) and new > 0:
                    gains.append(item)
    return {"changed_cells": changed, "baseline_positive_value_changes": positive_changes, "baseline_positive_losses_to_zero": losses, "zero_to_positive_gains": gains, "identities_and_shapes_exact": True}


CELL_METRICS = (
    "pilot_hits", "pilot_ess", "main_draws", "main_hits", "main_ess", "main_max_share",
    "probability", "check_draws", "check_hits", "check_ess", "check_probability",
    "cost_per_draw", "modeled_main_work", "modeled_check_work", "main_only",
)
SUMMARY_METRICS = (
    "funded", "completed", "main_only", "reclaimed_check_work", "waves",
    "native_reserved_after_waves", "native_reclaimed_check_work",
    "native_family_training_native_reserved_after_waves",
)


def identity(event):
    team = event.get("team_id", event.get("team"))
    rank = event.get("rank_index", event.get("rank"))
    if team is None or rank is None:
        raise ValueError(f"native cell metric event lacks team/rank identity: {event.get('event')}")
    return str(team), str(rank)


def native_cell_metrics(events):
    result = {}
    for event in events:
        if event.get("event") != "rust_odds_rare_tail_confirm_more":
            continue
        key = identity(event)
        missing = set(CELL_METRICS) - set(event)
        if missing:
            raise ValueError(f"incomplete native cell metrics for {key}; missing {sorted(missing)}")
        values = {metric: event[metric] for metric in CELL_METRICS}
        for pkey in ("probability", "check_probability"):
            if not isinstance(values[pkey], (int, float)) or not math.isfinite(values[pkey]):
                raise ValueError(f"null or invalid {pkey} for native cell {key}")
        if key in result:
            raise ValueError(f"duplicate native cell metric identity {key}")
        result[f"{key[0]}:{key[1]}"] = values
    return result


def native_summary_metrics(events):
    result = []
    for event in events:
        name = str(event.get("event", "")).lower()
        if name != "rust_odds_rare_tail_confirm_more_summary":
            continue
        values = {key: event[key] for key in SUMMARY_METRICS if key in event}
        values.update({key: value for key, value in event.items() if key.startswith(("wave1_", "wave2_", "first_wave_", "second_wave_"))})
        if values:
            result.append(values)
    return result


def empty_rare_tail_search_evidence(events):
    summaries = [e for e in events if e.get("event") == "rust_odds_rare_tail_summary"]
    if summaries:
        proven = len(summaries) == 1 and int(summaries[0].get("finalists", -1)) == 0 and int(summaries[0].get("accepted", -1)) == 0
        return {"proven": proven, "source": "rare-tail-summary", "summary_count": len(summaries), "finalists": summaries[0].get("finalists") if len(summaries) == 1 else None, "accepted": summaries[0].get("accepted") if len(summaries) == 1 else None}

    stages = [e for e in events if e.get("event") == "rust_odds_stage"]
    screen = [e for e in stages if e.get("stage") == "search.screen"]
    search = [e for e in stages if e.get("stage") == "search"]
    early = [e for e in stages if e.get("stage") == "search.early_proofs"]
    if len(screen) != 1 or len(search) != 1 or len(early) != 1:
        return {"proven": False, "source": "stage-search-events", "screen_count": len(screen), "search_count": len(search), "early_proof_count": len(early)}
    screen_cells = screen[0].get("cells")
    search_cells = search[0].get("cells")
    scope_keys = ("group", "seed", "request_id")
    same_scope = all(screen[0].get(key) is not None and screen[0].get(key) == search[0].get(key) == early[0].get(key) for key in scope_keys)
    counts_match = isinstance(screen_cells, dict) and screen_cells == search_cells
    no_candidates = screen[0].get("candidate_cells") == 0 and search[0].get("candidate_cells", 0) == 0
    no_zero_cells = isinstance(screen_cells, dict) and all(screen_cells.get(key) == 0 for key in ("reachable_zero", "undecided_zero", "impossible_zero", "zeros"))
    no_search_work = search[0].get("work") == 0
    no_proofs = early[0].get("proofs") == 0
    proven = same_scope and counts_match and no_candidates and no_zero_cells and no_search_work and no_proofs
    return {"proven": proven, "source": "matching-empty-search-screen-and-final-counts", "same_request_scope": same_scope, "screen_candidate_cells": screen[0].get("candidate_cells"), "search_candidate_cells": search[0].get("candidate_cells"), "screen_cell_counts": screen_cells, "search_cell_counts": search_cells, "screen_final_cell_counts_match": counts_match, "no_reachable_undecided_or_other_zero_cells": no_zero_cells, "search_work": search[0].get("work"), "early_proofs": early[0].get("proofs")}


def no_native_confirmation_work_evidence(events_a, events_b):
    native_event_names = {"rust_odds_rare_tail_confirm_more", "rust_odds_family_fallback", "rust_odds_family_native_training"}
    unexpected_native_events = {
        label: [event.get("event") for event in events if event.get("event") in native_event_names]
        for label, events in (("flag_off", events_a), ("capture", events_b))
    }
    if any(unexpected_native_events.values()):
        return {"proven": False, "source": "unexpected-native-work-events", "events": unexpected_native_events}
    a_conf = [e for e in events_a if e.get("event") == "rust_odds_rare_tail_confirm_more_summary"]
    b_conf = [e for e in events_b if e.get("event") == "rust_odds_rare_tail_confirm_more_summary"]
    if a_conf or b_conf:
        a_eligible = int(a_conf[0].get("eligible", -1)) if len(a_conf) == 1 else None
        b_eligible = int(b_conf[0].get("eligible", -1)) if len(b_conf) == 1 else None
        proven = len(a_conf) == len(b_conf) == 1 and a_eligible == b_eligible == 0
        return {"proven": proven, "source": "confirmation-summary-eligible-count", "flag_off_eligible": a_eligible, "capture_eligible": b_eligible, "flag_off_summary_count": len(a_conf), "capture_summary_count": len(b_conf)}
    a_evidence = empty_rare_tail_search_evidence(events_a)
    b_evidence = empty_rare_tail_search_evidence(events_b)
    return {"proven": bool(a_evidence["proven"] and b_evidence["proven"]), "source": "matching-empty-stage-search-evidence", "flag_off": a_evidence, "capture": b_evidence}


def no_native_confirmation_work_proven(events_a, events_b):
    return no_native_confirmation_work_evidence(events_a, events_b)["proven"]


def ledger_audit(events):
    """Audit native observer, confirmation, fallback, and earlier-stage ledgers."""
    summaries = [e for e in events if e.get("event") == "rust_odds_rare_tail_confirm_more_summary"]
    stages = [e for e in events if e.get("event") == "rust_odds_rare_tail_summary"]
    observers = [e for e in events if e.get("event") == "rust_odds_family_native_training"]
    fallback_events = []
    for e in events:
        if e.get("event") == "rust_odds_family_fallback":
            record = dict(e.get("fallback", {}))
            record["team_id"] = e.get("team_id")
            record["rank"] = e.get("rank")
            fallback_events.append(record)
    if not summaries and not stages and not observers and not fallback_events:
        evidence = empty_rare_tail_search_evidence(events)
        if evidence.get("proven"):
            return {"schema_complete": True, "status": "not_applicable_no_native_confirmation_work", "no_native_confirmation_work_evidence": evidence, "checks": {}, "all_arithmetic_checks_match": None, "observer_events": [], "fallback_events": []}
    if len(summaries) != 1 or len(stages) != 1:
        return {"schema_complete": False, "reason": "expected one confirmation summary and one ordinary rare-tail summary", "observer_events": observers, "fallback_events": fallback_events}
    summary = summaries[0]
    funding_mode = summary.get("native_family_training_funding", "confirmation_bank_certificate")
    stage_funded = funding_mode == "stage"
    if funding_mode not in ("stage", "confirmation_bank_certificate"):
        return {"schema_complete": False, "reason": f"unknown native training funding mode: {funding_mode}", "observer_events": observers, "fallback_events": fallback_events}
    stage = stages[0].get("native_family_recording_stage")
    if not isinstance(stage, dict):
        return {"schema_complete": False, "reason": "rare-tail summary lacks native_family_recording_stage", "observer_events": observers, "fallback_events": fallback_events}
    required_summary = (
        "native_family_training_certificate_fee", "native_family_training_certificate_residual",
        "native_family_training_observer_reserved", "native_family_training_observer_actual",
        "native_family_training_observer_overrun", "native_family_training_reused_draws",
        "native_family_training_recorded_observations", "native_family_training_skipped_key_observations",
        "native_family_training_added_work", "native_family_training_requested", "family_fallback_added_work", "family_fallback_order_setup_work",
        "work", "work_limit", "reserved_work",
    )
    required_stage = (
        "capacity", "audit_enabled", "training_work", "setup_audit_extra_work", "setup_audit_complete", "unmetered_constructor_failures", "ordinary_actual_final_operations",
        "extension_actual_final_operations", "actual_final_operations_total",
        "reserved_final_work", "conservative_used", "available_spare", "constructor_failures",
    )
    missing = [f"confirmation.{key}" for key in required_summary if key not in summary]
    if stage_funded:
        required_stage_funding = (
            "native_family_training_stage_spare_before_funding",
            "native_family_training_stage_dispatch_fee_required",
            "native_family_training_stage_dispatch_fee_charged",
            "native_family_training_stage_observer_bound",
            "native_family_training_stage_observer_actual",
            "native_family_training_stage_overrun",
            "native_family_training_stage_added_work",
            "native_family_training_confirmation_observer_reserved",
            "native_family_training_confirmation_observer_actual",
            "native_family_training_native_reserved_after_waves",
            "native_reserved_after_waves",
            "native_reclaimed_check_work",
        )
        missing.extend(f"confirmation.{key}" for key in required_stage_funding if key not in summary)
    missing.extend(f"stage.{key}" for key in required_stage if key not in stage)
    observer_fields = ("observer_bound", "observer_actual_work", "recorded_prefix_draws", "recorded_positive_observations", "skipped_key_observations")
    for i, event in enumerate(observers):
        missing.extend(f"observer[{i}].{key}" for key in observer_fields if key not in event)
        if stage_funded:
            missing.extend(f"observer[{i}].{key}" for key in ("funding_source", "observer_reservation_in_confirmation_bank") if key not in event)
    native_operation_events = [e for e in events if e.get("event") == "rust_odds_rare_tail_confirm_more"]
    for i, event in enumerate(native_operation_events):
        if "actual_operation_work" not in event:
            missing.append(f"native_confirmation[{i}].actual_operation_work")
    if missing:
        return {"schema_complete": False, "reason": "missing ledger fields", "missing_fields": missing, "observer_events": observers, "fallback_events": fallback_events}

    observer_actual = sum(int(e["observer_actual_work"]) for e in observers)
    observer_bound = sum(int(e["observer_bound"]) for e in observers)
    reused_draws = sum(int(e["recorded_prefix_draws"]) for e in observers)
    recorded_observations = sum(int(e["recorded_positive_observations"]) for e in observers)
    skipped_observations = sum(int(e["skipped_key_observations"]) for e in observers)
    checks = {
        "observer_actual_sum_matches_confirmation": observer_actual == summary["native_family_training_observer_actual"],
        "observer_bound_sum_matches_funding_reservation": observer_bound == (summary["native_family_training_stage_observer_bound"] if stage_funded else summary["native_family_training_observer_reserved"]),
        "observer_actual_within_sum_of_cell_bounds": observer_actual <= observer_bound,
        "observer_reused_draws_sum_matches_confirmation": reused_draws == summary["native_family_training_reused_draws"],
        "observer_recorded_observations_sum_matches_confirmation": recorded_observations == summary["native_family_training_recorded_observations"],
        "observer_skipped_observations_sum_matches_confirmation": skipped_observations == summary["native_family_training_skipped_key_observations"],
        "observer_overrun_flag_matches_actual_vs_reservation": bool(summary["native_family_training_observer_overrun"]) == (observer_actual > observer_bound if stage_funded else observer_actual > int(summary["native_family_training_observer_reserved"])),
    }
    if stage_funded:
        checks["all_capture_events_are_stage_funded"] = all(
            e.get("funding_source") == "stage" and e.get("observer_reservation_in_confirmation_bank") is False for e in observers
        )
        checks["confirmation_bank_observer_reservation_is_zero"] = int(summary["native_family_training_confirmation_observer_reserved"]) == 0
        checks["confirmation_bank_observer_actual_is_zero"] = int(summary["native_family_training_confirmation_observer_actual"]) == 0
        checks["stage_observer_actual_summary_matches_events"] = observer_actual == int(summary["native_family_training_stage_observer_actual"])
        checks["stage_observer_overrun_matches_actual_vs_bound"] = bool(summary["native_family_training_stage_overrun"]) == (observer_actual > observer_bound)
        checks["native_reserved_after_waves_alias_matches"] = summary["native_family_training_native_reserved_after_waves"] == summary["native_reserved_after_waves"]
    observer_bounds = [{"team_id": e.get("team_id"), "rank": e.get("rank"), "bound": e["observer_bound"], "actual": e["observer_actual_work"], "deficit": max(0, int(e["observer_actual_work"]) - int(e["observer_bound"]))} for e in observers]
    native_confirmation_actual = sum(int(e["actual_operation_work"]) for e in native_operation_events)

    observer_by_cell = {f"{e.get('team_id')}:{e.get('rank')}": e for e in observers}
    fallback_rows = []
    fallback_records_actual = 0
    replay_violations = []
    cell_ledger_violations = []
    stage_cell_ledger_violations = []
    for event in fallback_events:
        team_rank = f"{event.get('team_id')}:{event.get('rank')}"
        observer = observer_by_cell.get(team_rank, {})
        observer_work = int(event.get("native_training_observer_work", 0))
        observer_work_matches = observer_work == int(observer.get("observer_actual_work", 0))
        native_work = int(event.get("native_actual_work", 0))
        added_work = int(event.get("added_actual_work", 0))
        combined = int(event.get("native_plus_added_work", 0))
        combined_matches = combined == native_work + observer_work + added_work
        fallback_records_actual += added_work
        reused = int(event.get("native_training_reused_draws", 0)) > 0
        replay = {key: int(event.get(key, 0)) for key in ("training_replayed_draws", "training_new_draws", "training_operation_work", "family_collection_work")}
        no_duplicate_replay = not reused or all(value == 0 for value in replay.values())
        if not no_duplicate_replay:
            replay_violations.append({"cell": team_rank, **replay})
        if not observer_work_matches or not combined_matches:
            cell_ledger_violations.append({"cell": team_rank, "observer_work_matches_capture": observer_work_matches, "combined_actual_matches_components": combined_matches})
        stage_cell_result = None
        if stage_funded:
            event_bound = int(event.get("native_training_observer_bound", -1))
            actual_with_observer = int(event.get("added_actual_work_including_stage_observer", -1))
            cell_grant = int(event.get("cell_grant", -1))
            fresh_grant = int(event.get("fallback_grant", -1))
            cell_limit = event.get("expanded_cell_limit")
            expected_cell_limit = cell_grant + event_bound
            stage_cell_result = {
                "funding_mode_matches": event.get("native_training_funding") == "stage",
                "observer_bound_matches_capture": event_bound == int(observer.get("observer_bound", 0)),
                "added_work_including_observer_matches": actual_with_observer == observer_work + added_work,
                "fresh_grant_does_not_include_advance": fresh_grant >= 0 and fresh_grant <= max(0, cell_grant - native_work),
                "cell_limit_equals_original_grant_plus_observer_bound": cell_limit == expected_cell_limit,
                "total_actual_within_advanced_cell_limit": combined <= expected_cell_limit,
            }
            if not all(stage_cell_result.values()):
                stage_cell_ledger_violations.append({"cell": team_rank, **stage_cell_result})
        fallback_rows.append({"cell": team_rank, "observer_work": observer_work, "native_actual_work": native_work, "fallback_added_actual_work": added_work, "native_plus_added_work": combined, "combined_matches_components": combined_matches, "stage_funded_ledger": stage_cell_result, "settlement_within_grant": event.get("settlement_within_grant"), "reused_draws": event.get("native_training_reused_draws", 0), "replay_and_collection_work": replay, "no_duplicate_replay_or_collection": no_duplicate_replay})

    family_work_expected = fallback_records_actual + int(summary["family_fallback_order_setup_work"])
    fee = int(summary["native_family_training_certificate_fee"])
    observer_reserved = int(summary["native_family_training_observer_reserved"])
    observer_actual_reported = int(summary["native_family_training_observer_actual"])
    stage_dispatch_fee = int(summary.get("native_family_training_stage_dispatch_fee_charged", 0)) if stage_funded else 0
    stage_observer_bound = int(summary.get("native_family_training_stage_observer_bound", 0)) if stage_funded else 0
    stage_observer_actual = int(summary.get("native_family_training_stage_observer_actual", 0)) if stage_funded else 0
    stage_added_reported = int(summary.get("native_family_training_stage_added_work", 0)) if stage_funded else 0
    native_added_expected = (stage_dispatch_fee + stage_observer_actual) if stage_funded else (fee + observer_actual_reported)
    checks["fallback_actual_sum_matches_summary_including_order_setup"] = family_work_expected == int(summary["family_fallback_added_work"])
    checks["native_added_work_equals_fee_plus_observer_actual"] = native_added_expected == int(summary["native_family_training_added_work"])
    if stage_funded:
        required_dispatch_fee = summary["native_family_training_stage_dispatch_fee_required"]
        checks["stage_dispatch_fee_required_matches_charge"] = (stage_dispatch_fee == 0 if required_dispatch_fee is None else stage_dispatch_fee in (0, int(required_dispatch_fee)))
        checks["stage_added_work_equals_dispatch_fee_plus_observer_actual"] = stage_added_reported == stage_dispatch_fee + stage_observer_actual
        checks["stage_observer_actual_summary_matches_generic_total"] = stage_observer_actual == observer_actual_reported
    checks["reported_work_includes_certificate_fee"] = int(summary["work"]) >= fee
    checks["fallback_never_replays_or_recollects_reused_prefix"] = not replay_violations
    checks["fallback_cell_ledgers_include_observer_and_actual_work"] = not cell_ledger_violations
    checks["stage_funded_fallback_cells_respect_advance_and_fresh_grant"] = not stage_cell_ledger_violations

    training_work = int(stage["training_work"])
    setup_audit_extra = int(stage["setup_audit_extra_work"])
    audited_training_work = training_work + setup_audit_extra
    actual_final = int(stage["ordinary_actual_final_operations"]) + int(stage["extension_actual_final_operations"])
    reserved_final = int(stage["reserved_final_work"])
    conservative_expected = audited_training_work + max(actual_final, reserved_final)
    settled_expected = audited_training_work + actual_final
    stage_capacity = int(stage["capacity"])
    constructor_failures = int(stage["constructor_failures"])
    unmetered_constructor_failures = int(stage["unmetered_constructor_failures"])
    stage_credit_guard = bool(stage["audit_enabled"]) and bool(stage["setup_audit_complete"]) and unmetered_constructor_failures == 0
    available_spare = stage.get("available_spare")
    v3_stage_schema = "stage_complete" in stage or "settled_available_spare" in stage or "conservative_available_spare" in stage
    stage_math_matches = int(stage["actual_final_operations_total"]) == actual_final
    if v3_stage_schema:
        for key in ("stage_complete", "settled_used", "settled_available_spare", "conservative_available_spare"):
            if key not in stage:
                missing.append(f"stage.{key}")
        if missing:
            return {"schema_complete": False, "reason": "missing v3 settled-stage fields", "missing_fields": missing, "observer_events": observers, "fallback_events": fallback_events}
        any_capture_charge = bool(summary["native_family_training_requested"]) or fee > 0 or observer_actual > 0 or observer_reserved > 0 or stage_added_reported > 0 or stage_observer_bound > 0
        stage_complete_ok = not any_capture_charge or stage.get("stage_complete") is True
        reported_settled = stage.get("settled_used")
        reported_conservative = stage.get("conservative_used")
        settled_available = stage.get("settled_available_spare")
        conservative_available = stage.get("conservative_available_spare")
        settlement_credit_available = stage_credit_guard and reported_settled is not None
        if settlement_credit_available:
            stage_math_matches = stage_math_matches and reported_settled == settled_expected and reported_conservative == conservative_expected
            stage_math_matches = stage_math_matches and settled_available == max(0, stage_capacity - settled_expected)
            stage_math_matches = stage_math_matches and conservative_available == max(0, stage_capacity - conservative_expected)
            stage_math_matches = stage_math_matches and available_spare == settled_available
        else:
            stage_math_matches = stage_math_matches and all(v is None for v in (reported_settled, reported_conservative, settled_available, conservative_available, available_spare))
        funded_amount = stage_added_reported if stage_funded else fee + observer_actual
        funded_bound = stage_observer_bound if stage_funded else observer_reserved
        checks["no_certificate_or_observer_funding_without_stage_credit"] = settlement_credit_available or (funded_amount == 0 and funded_bound == 0)
        stage_math_matches = stage_math_matches and stage_complete_ok
    else:
        reported_settled = None
        settled_available = None
        conservative_available = None
        if stage_credit_guard:
            stage_math_matches = stage_math_matches and int(stage["conservative_used"]) == conservative_expected and available_spare == max(0, stage_capacity - conservative_expected)
        else:
            stage_math_matches = stage_math_matches and available_spare is None
    checks["ordinary_stage_actual_reserve_and_spare_arithmetic_matches"] = stage_math_matches
    checks["stage_complete_before_capture_credit"] = not (bool(summary["native_family_training_requested"]) or fee > 0 or observer_actual > 0 or observer_reserved > 0) or (not v3_stage_schema or stage.get("stage_complete") is True)
    settled_spare_for_fee = settled_available if v3_stage_schema else available_spare
    fee_guard = settlement_credit_available if v3_stage_schema else stage_credit_guard
    if stage_funded:
        stage_spare_before_funding = summary.get("native_family_training_stage_spare_before_funding")
        advance_required = stage_dispatch_fee + stage_observer_bound
        checks["stage_spare_before_funding_matches_stage_credit"] = stage_spare_before_funding == settled_spare_for_fee
        checks["stage_dispatch_fee_and_observer_advance_fit_settled_spare"] = advance_required == 0 or (fee_guard and settled_spare_for_fee is not None and advance_required <= int(settled_spare_for_fee))
        checks["stage_funding_has_valid_stage_credit"] = fee_guard or (stage_added_reported == 0 and stage_observer_bound == 0)
    else:
        checks["certificate_fee_fits_reported_settled_stage_spare"] = fee == 0 or (fee_guard and settled_spare_for_fee is not None and fee <= int(settled_spare_for_fee))

    legacy_summary_work = int(summary["work"])
    confirmation_limit = int(summary["work_limit"])
    confirmation_reserved = int(summary["reserved_work"])
    fallback_actual = int(summary["family_fallback_added_work"])
    confirmation_observer_actual = int(summary["native_family_training_confirmation_observer_actual"]) if stage_funded else observer_actual
    confirmation_actual_without_stage_fee = native_confirmation_actual + confirmation_observer_actual + fallback_actual
    confirmation_actual = confirmation_actual_without_stage_fee + fee
    stage_actual = audited_training_work + actual_final + (stage_added_reported if stage_funded else 0)
    combined_actual = stage_actual + confirmation_actual
    combined_capacity = stage_capacity + confirmation_limit
    confirmation_actual_deficit = max(0, confirmation_actual_without_stage_fee - confirmation_limit)
    conservative_stage_plus_fee_deficit = max(0, conservative_expected + (stage_dispatch_fee if stage_funded else fee) - stage_capacity)
    settled_stage_plus_fee_deficit = max(0, settled_expected + (stage_dispatch_fee + stage_observer_actual if stage_funded else fee) - stage_capacity)
    settled_spare = max(0, stage_capacity - settled_expected)
    checks["actual_confirmation_components_match_fallback_and_observer_ledgers"] = confirmation_actual == native_confirmation_actual + confirmation_observer_actual + fallback_actual + fee
    checks["stage_actual_components_include_stage_funding_once"] = stage_actual == audited_training_work + actual_final + (stage_added_reported if stage_funded else 0)
    checks["stage_actual_capture_work_fits_capacity"] = stage_added_reported == 0 or (fee_guard and audited_training_work + actual_final + stage_added_reported <= stage_capacity)
    return {
        "schema_complete": True, "checks": checks, "all_arithmetic_checks_match": all(checks.values()),
        "observer": {"event_count": len(observers), "actual_sum": observer_actual, "funding_source": funding_mode, "funding_bound_sum": observer_bound, "actual_over_funding_bound": max(0, observer_actual - observer_bound), "confirmation_bank_reserved_bound": observer_reserved if not stage_funded else None, "confirmation_bank_actual_over_reserved": max(0, observer_actual - observer_reserved) if not stage_funded else None, "summary_overrun": summary["native_family_training_observer_overrun"], "cells": observer_bounds},
        "certificate": {"fee": fee, "residual": summary["native_family_training_certificate_residual"], "stage_available_spare": available_spare, "stage_capacity": stage_capacity},
        "fallback": {"record_added_actual_sum": fallback_records_actual, "order_setup_work": summary["family_fallback_order_setup_work"], "expected_summary_added_work": family_work_expected, "reported_summary_added_work": summary["family_fallback_added_work"], "cells": fallback_rows, "duplicate_replay_violations": replay_violations, "cell_ledger_violations": cell_ledger_violations},
        "stage_budget": {"training_work": training_work, "setup_audit_extra_work": setup_audit_extra, "audited_training_work": audited_training_work, "ordinary_actual_final_operations": stage["ordinary_actual_final_operations"], "extension_actual_final_operations": stage["extension_actual_final_operations"], "actual_final_operations_total": actual_final, "reserved_final_work": reserved_final, "conservative_used_calculated": conservative_expected, "conservative_used_reported": reported_conservative, "settled_used_calculated": settled_expected, "settled_used_reported": reported_settled, "settled_actual_spare_calculated": settled_spare, "settled_available_spare_reported": settled_available, "conservative_available_spare_reported": conservative_available, "v3_stage_schema": v3_stage_schema, "settlement_credit_available": settlement_credit_available if v3_stage_schema else None, "available_spare_basis": ("settled-actual" if settlement_credit_available else "unavailable-v3") if v3_stage_schema else "v2-conservative-before-settlement-field", "available_spare_matches_settled_actual": (available_spare == settled_available) if settlement_credit_available else None, "stage_complete": stage.get("stage_complete"), "capacity": stage_capacity, "audit_enabled": stage["audit_enabled"], "setup_audit_complete": stage["setup_audit_complete"], "constructor_failures": constructor_failures, "unmetered_constructor_failures": unmetered_constructor_failures, "credit_guard_satisfied": stage_credit_guard, "available_spare": available_spare, "funding_mode": funding_mode, "dispatch_fee_required": summary.get("native_family_training_stage_dispatch_fee_required"), "dispatch_fee_charged": stage_dispatch_fee, "observer_advance_bound": stage_observer_bound, "observer_actual_work": stage_observer_actual, "stage_added_work": stage_added_reported, "spare_before_funding": summary.get("native_family_training_stage_spare_before_funding"), "actual_stage_usage_including_capture": stage_actual, "certificate_fee_plus_conservative_used": conservative_expected + (stage_dispatch_fee if stage_funded else fee), "certificate_fee_conservative_projection_deficit": conservative_stage_plus_fee_deficit, "certificate_fee_plus_settled_used": settled_expected + (stage_dispatch_fee + stage_observer_actual if stage_funded else fee), "certificate_fee_settled_actual_deficit": settled_stage_plus_fee_deficit},
        "confirmation_budget": {"native_actual_operation_sum": native_confirmation_actual, "observer_actual_sum": confirmation_observer_actual, "all_observer_actual_sum": observer_actual, "fallback_added_actual_work": fallback_actual, "certificate_fee_transfer": fee, "actual_confirmation_work_excluding_stage_fee": confirmation_actual_without_stage_fee, "actual_confirmation_work_including_stage_fee": confirmation_actual, "legacy_summary_work_not_actual_counter": legacy_summary_work, "legacy_summary_work_minus_actual_counter": legacy_summary_work - confirmation_actual, "raw_actual_including_fee_over_limit": max(0, confirmation_actual - confirmation_limit), "actual_deficit_excluding_stage_fee": confirmation_actual_deficit, "work_limit": confirmation_limit, "reserved_work": confirmation_reserved, "reservation_free": max(0, confirmation_limit - confirmation_reserved), "actual_free_excluding_stage_fee": max(0, confirmation_limit - confirmation_actual_without_stage_fee), "actual_over_cap": confirmation_actual_deficit > 0},
        "combined_budget": {"actual_stage_plus_confirmation_work": combined_actual, "stage_plus_confirmation_capacity": combined_capacity, "actual_deficit": max(0, combined_actual - combined_capacity), "inherited_reservation_projection_is_not_an_actual_work_guarantee": True},
        "native_added_work_reported": summary["native_family_training_added_work"],
        "native_added_work_calculated": native_added_expected,
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--screen", required=True, type=lambda x: Path(x).resolve(strict=True))
    p.add_argument("--r58", type=lambda x: Path(x).resolve(strict=True), help="Frozen R58 V6 screen root, with ORDER-order-cap3000/seedN/export.json")
    p.add_argument("--out", required=True, type=lambda x: Path(x).resolve())
    p.add_argument("--order", choices=("native", "grant"), default="native")
    p.add_argument("--capture-arm", help="Capture arm directory name; defaults to capture-ORDER-order-cap3000")
    p.add_argument("--generic-r58", action="store_true", help="Use this screen's fresh R58 replay export as the comparator instead of requiring a frozen V6 cohort record")
    args = p.parse_args()
    if args.out.exists(): p.error(f"refusing existing output file: {args.out}")
    if not args.generic_r58 and not args.r58:
        p.error("--r58 is required unless --generic-r58 is set")
    summary = read(args.screen / "summary.json")
    r58_summary = None if args.generic_r58 else read(args.r58 / "summary.json")
    seeds = summary["seeds"]
    rows, all_events = [], []
    replay_arm = f"replay-{args.order}-order-cap3000"
    capture_arm = args.capture_arm or f"capture-{args.order}-order-cap3000"
    for seed in seeds:
        base_rec = read(args.screen / "baseline" / f"seed{seed}" / "record.json")
        capture_rec = read(args.screen / capture_arm / f"seed{seed}" / "record.json")
        base, capture = read(base_rec["export"]), read(capture_rec["export"])
        replay_rec = read(args.screen / replay_arm / f"seed{seed}" / "record.json")
        if sha(replay_rec["export"]) != replay_rec.get("export_sha256"):
            raise ValueError(f"fresh R58 replay export record/hash mismatch for seed {seed}")
        if args.generic_r58:
            r58_path = Path(replay_rec["export"]).resolve(strict=True)
        else:
            r58_path = args.r58 / f"{args.order}-order-cap3000" / f"seed{seed}" / "export.json"
            r58_record = next((r for r in r58_summary["records"] if r.get("seed") == seed and r.get("arm") == f"{args.order}-order-cap3000"), None)
            if not r58_record or Path(r58_record["export"]) != r58_path.resolve() or sha(r58_path) != r58_record["export_sha256"]:
                raise ValueError(f"R58 frozen export record/hash mismatch for seed {seed}: {r58_path}")
            if Path(replay_rec["export"]).read_bytes() != r58_path.read_bytes():
                raise ValueError(f"fresh R58 replay export differs from frozen same-order cap-3000 export for seed {seed}")
        for record in (base_rec, capture_rec):
            if sha(record["export"]) != record.get("export_sha256"):
                raise ValueError(f"R59 screen export record/hash mismatch for {record.get('arm')} seed {seed}")
        r58 = read(r58_path)
        base_cells = {(t, r) for t, r, _ in cells(base)}
        capture_cells = {(t, r) for t, r, _ in cells(capture)}
        r58_cells = {(t, r) for t, r, _ in cells(r58)}
        if not base_cells or base_cells != capture_cells or base_cells != r58_cells:
            raise ValueError(f"incomplete or differing matrix identities for seed {seed}")
        gains, losses = gains_losses(base, capture)
        r58_gains, r58_losses = gains_losses(base, r58)
        capture_vs_r58_gains, capture_vs_r58_losses = gains_losses(r58, capture)
        changed_positive, changed_cells = [], []
        bmat, cmat = matrix(base), matrix(capture)
        for team, ranks in bmat.items():
            for rank, before in ranks.items():
                after = cmat[team][rank]
                if clean(before) != clean(after):
                    changed_cells.append(f"{team}:{rank}")
                    if before.get("probability", 0) > 0:
                        changed_positive.append(f"{team}:{rank}")
        importance_exact = base["game_importance"] == capture["game_importance"]
        events = capture_rec.get("events", [])
        ledger = ledger_audit(events)
        replay_events = replay_rec.get("events", [])
        flag_off_rec = read(args.screen / "capture-flag-off" / f"seed{seed}" / "record.json")
        if sha(flag_off_rec["export"]) != flag_off_rec.get("export_sha256"):
            raise ValueError(f"R59 flag-off export hash mismatch for seed {seed}")
        flag_off_events = flag_off_rec.get("events", [])
        flag_off_confirm = next((e for e in flag_off_events if e.get("event") == "rust_odds_rare_tail_confirm_more_summary"), {})
        flag_off_native_operations = [e for e in flag_off_events if e.get("event") == "rust_odds_rare_tail_confirm_more"]
        native_operation_values = [e.get("actual_operation_work") for e in flag_off_native_operations]
        native_actual_operation_work = sum(native_operation_values) if all(isinstance(value, int) and value >= 0 for value in native_operation_values) else None
        confirmation_limit = flag_off_confirm.get("work_limit")
        confirmation_reserved = flag_off_confirm.get("reserved_work")
        team_odds_changes = team_odds_pos_changes(base, capture)
        event_counts = Counter(e.get("event") for e in events)
        observer_events = [e for e in events if any(s in e.get("event", "").lower() for s in ("native_training", "training_capture", "certificate"))]
        flag_off_cell_metrics = native_cell_metrics(flag_off_events)
        capture_cell_metrics = native_cell_metrics(events)
        flag_off_summary_metrics = native_summary_metrics(flag_off_events)
        capture_summary_metrics = native_summary_metrics(events)
        no_native_evidence = no_native_confirmation_work_evidence(flag_off_events, events)
        no_native_work = no_native_evidence["proven"]
        empty_cell_metrics = not flag_off_cell_metrics and not capture_cell_metrics
        if empty_cell_metrics and no_native_work:
            cell_metric_parity = None
            cell_metric_status = "not_applicable_no_native_confirmation_work"
        else:
            cell_metric_parity = bool(flag_off_cell_metrics) and flag_off_cell_metrics == capture_cell_metrics
            cell_metric_status = "compared"
        if cell_metric_parity is False:
            raise ValueError(f"native per-cell metrics are missing or differ from flag-off control for seed {seed}")
        if not flag_off_summary_metrics and not capture_summary_metrics and no_native_work:
            summary_metric_parity = None
            summary_metric_status = "not_applicable_no_native_confirmation_work"
        else:
            summary_metric_parity = bool(flag_off_summary_metrics) and flag_off_summary_metrics == capture_summary_metrics
            summary_metric_status = "compared"
        if summary_metric_parity is False:
            raise ValueError(f"native funding/wave summary metrics are missing or differ from flag-off control for seed {seed}")
        flag_off_bytes_match = Path(flag_off_rec["export"]).read_bytes() == Path(base_rec["export"]).read_bytes()
        if not flag_off_bytes_match:
            raise ValueError(f"capture flag-off export differs byte-for-byte from baseline for seed {seed}")
        reference_workers = base_rec.get("reference_record", {}).get("workers", base_rec.get("workers"))
        worker_parity_comparable = reference_workers == summary.get("workers")
        all_events.extend({"seed": seed, **e} for e in events)
        rows.append({
            "seed": seed, "baseline_export_sha256": base_rec["export_sha256"],
            "capture_export_sha256": capture_rec["export_sha256"], "r58_export": str(r58_path),
            "r58_comparison_mode": "generic-fresh-replay" if args.generic_r58 else "frozen-v6",
            "baseline_positive_estimates_and_importance_match_baseline": not changed_positive and importance_exact,
            "changed_estimate_cells_ignoring_only_work_spent": changed_cells,
            "baseline_positive_cells_changed": changed_positive,
            "game_importance_exact": importance_exact,
            "team_odds_Pos_changes_vs_baseline": team_odds_changes,
            "team_odds_positive_losses_to_zero": team_odds_changes["baseline_positive_losses_to_zero"],
            "native_confirm_actual_and_reservation_headroom": {
                "native_actual_operation_work": native_actual_operation_work,
                "legacy_reported_summary_work": flag_off_confirm.get("work"),
                "legacy_summary_work_minus_native_actual_operation_work": flag_off_confirm.get("work") - native_actual_operation_work if isinstance(flag_off_confirm.get("work"), int) and native_actual_operation_work is not None else None,
                "work_limit": confirmation_limit,
                "reserved_work": confirmation_reserved,
                "actual_headroom": confirmation_limit - native_actual_operation_work if isinstance(confirmation_limit, int) and native_actual_operation_work is not None else None,
                "reservation_headroom": confirmation_limit - confirmation_reserved if isinstance(confirmation_limit, int) and isinstance(confirmation_reserved, int) else None,
                "actual_work_exceeds_limit": native_actual_operation_work > confirmation_limit if native_actual_operation_work is not None and isinstance(confirmation_limit, int) else None,
            },
            "native_training_ledger": ledger,
            "capture_flag_off_byte_identical_to_baseline": flag_off_bytes_match,
            "baseline_reference_workers": reference_workers,
            "baseline_worker_parity_comparable": worker_parity_comparable,
            "capture_gains": gains, "capture_losses": losses,
            "r58_gains": r58_gains, "r58_losses": r58_losses,
            "capture_vs_r58_gains": capture_vs_r58_gains, "capture_vs_r58_losses": capture_vs_r58_losses,
            "capture_gains_vs_baseline_count": len(gains), "capture_losses_vs_baseline_count": len(losses),
            "capture_gains_vs_r58_count": len(capture_vs_r58_gains), "capture_losses_vs_r58_count": len(capture_vs_r58_losses),
            "capture_matrix_classification": classifications(capture),
            "r58_matrix_classification": classifications(r58),
            "capture_events_by_name": dict(event_counts),
            "native_training_or_certificate_events": observer_events,
            "native_cell_metrics_complete": bool(flag_off_cell_metrics and capture_cell_metrics),
            "native_cell_metrics_status": cell_metric_status,
            "native_cell_metrics_flag_off": flag_off_cell_metrics,
            "native_cell_metrics_capture": capture_cell_metrics,
            "native_cell_metrics_exact_parity": cell_metric_parity,
            "native_confirmation_work_evidence": no_native_evidence,
            "native_summary_metrics_status": summary_metric_status,
            "native_summary_metrics_flag_off": flag_off_summary_metrics,
            "native_summary_metrics_capture": capture_summary_metrics,
            "native_summary_metrics_exact_parity": summary_metric_parity,
            "whole_event_diagnostics_replay": replay_events,
            "whole_event_diagnostics_flag_off": flag_off_events,
            "whole_event_diagnostics_capture": events,
            "all_capture_events": events,
        })
    result = {
        "seeds": seeds, "native_preservation_coverage_only": True,
        "P15_magnitude_or_calibration_validated": False,
        "capture_arm": capture_arm,
        "markdown_counters": {
            "capture_gains_vs_baseline": sum(r["capture_gains_vs_baseline_count"] for r in rows),
            "capture_losses_vs_baseline": sum(r["capture_losses_vs_baseline_count"] for r in rows),
            "capture_gains_vs_r58": sum(r["capture_gains_vs_r58_count"] for r in rows),
            "capture_losses_vs_r58": sum(r["capture_losses_vs_r58_count"] for r in rows),
            "team_odds_positive_losses_to_zero": sum(len(r["team_odds_positive_losses_to_zero"]) for r in rows),
            "native_observer_actual_work": sum(r["native_training_ledger"].get("observer", {}).get("actual_sum", 0) for r in rows),
            "native_confirmation_actual_work": sum(r["native_training_ledger"].get("confirmation_budget", {}).get("actual_confirmation_work_excluding_stage_fee", 0) for r in rows),
        },
        "screen_binary_metadata": summary.get("binaries"), "runs": rows,
        "all_native_capture_events": all_events,
        "all_baseline_positive_estimates_and_importance_match_baseline": all(r["baseline_positive_estimates_and_importance_match_baseline"] for r in rows),
        "all_capture_flag_off_exports_byte_identical_to_baseline": all(r["capture_flag_off_byte_identical_to_baseline"] for r in rows),
        "all_baseline_worker_parity_comparable": all(r["baseline_worker_parity_comparable"] for r in rows),
        "all_native_cell_metrics_exact_parity": None if all(r["native_cell_metrics_status"] == "not_applicable_no_native_confirmation_work" for r in rows) else (None if any(r["native_cell_metrics_status"] == "not_applicable_no_native_confirmation_work" for r in rows) else all(r["native_cell_metrics_exact_parity"] is True for r in rows)),
        "all_compared_native_cell_metrics_exact_parity": all(r["native_cell_metrics_exact_parity"] is True for r in rows if r["native_cell_metrics_status"] == "compared") if any(r["native_cell_metrics_status"] == "compared" for r in rows) else None,
        "all_native_summary_metrics_exact_parity": None if all(r["native_summary_metrics_status"] == "not_applicable_no_native_confirmation_work" for r in rows) else (None if any(r["native_summary_metrics_status"] == "not_applicable_no_native_confirmation_work" for r in rows) else all(r["native_summary_metrics_exact_parity"] is True for r in rows)),
        "all_compared_native_summary_metrics_exact_parity": all(r["native_summary_metrics_exact_parity"] is True for r in rows if r["native_summary_metrics_status"] == "compared") if any(r["native_summary_metrics_status"] == "compared" for r in rows) else None,
        "native_metrics_not_applicable_runs": sum(r["native_cell_metrics_status"] == "not_applicable_no_native_confirmation_work" for r in rows),
        "native_confirmation_work_proven_absent_runs": sum(r["native_confirmation_work_evidence"]["proven"] for r in rows),
        "charged_ledger_audit": "See per-run native_training_ledger; actual counter and reservation deficits are reported separately and are not treated as hard CPU guarantees.",
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
