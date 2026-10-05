"""Compare combined V5 matrices with baseline and individual V4 outputs."""
import json
from pathlib import Path

ART = Path(__file__).resolve().parents[1]
ROOT = ART / "logs" / "v5-final-screen"
V3 = ART / "logs" / "v3-cohort"
V4B = ART / "logs" / "v4-breadth"
OUT = ART / "data" / "v5-final-screen-analysis.json"
SEEDS = [808, 1669, 1993, 2281, 2293]


def strip_work(value):
    if isinstance(value, dict):
        return {k: strip_work(v) for k, v in value.items() if k != "work_spent"}
    if isinstance(value, list):
        return [strip_work(v) for v in value]
    return value


def matrix(export):
    return export["rare_position_estimates"]


def new_positive_cells(base, candidate):
    result = []
    for team, ranks in matrix(base).items():
        for rank, old in ranks.items():
            now = matrix(candidate)[team][rank]
            if old.get("probability", 0) == 0 and now.get("probability", 0) > 0:
                result.append({"team_id": team, "rank_index": rank, "probability": now["probability"], "design": now.get("design")})
    return result


def v4_individual_gains():
    result = set()
    for seed in (808, 2293):
        old = json.loads((V3 / f"seed{seed}" / "r58-fallback.json").read_text())
        base = json.loads((V3 / f"seed{seed}" / "current-rust.json").read_text())
        result.update((seed, row["team_id"], row["rank_index"]) for row in new_positive_cells(base, old))
    for selector in ("16-14", "16-15"):
        for seed in SEEDS:
            path = V4B / selector / "family-1x" / f"seed{seed}" / "export.json"
            cand = json.loads(path.read_text())
            base = json.loads((V3 / f"seed{seed}" / "current-rust.json").read_text())
            result.update((seed, row["team_id"], row["rank_index"]) for row in new_positive_cells(base, cand))
    return result


def event_rows(record):
    return [entry["fallback"] for entry in record["fallback_events"]]


def main():
    rows = []
    preserved = True
    separate = v4_individual_gains()
    for arm in ("all-family-1x", "all-native-retry-1x"):
        for seed in SEEDS:
            d = ROOT / arm / f"seed{seed}"
            record = json.loads((d / "record.json").read_text())
            candidate = json.loads((d / "export.json").read_text())
            baseline = json.loads((V3 / f"seed{seed}" / "current-rust.json").read_text())
            changed, positive_changed = [], []
            for team, ranks in matrix(baseline).items():
                for rank, old in ranks.items():
                    now = matrix(candidate)[team][rank]
                    if strip_work(old) != strip_work(now):
                        key = f"{team}:{rank}"
                        changed.append(key)
                        if old.get("probability", 0) > 0:
                            positive_changed.append(key)
            same_importance = baseline["game_importance"] == candidate["game_importance"]
            if positive_changed or not same_importance:
                preserved = False
            gains = new_positive_cells(baseline, candidate)
            gain_keys = {(seed, cell["team_id"], cell["rank_index"]) for cell in gains}
            fallback = event_rows(record)
            full_stages = {}
            for line in Path(record["stderr"]).read_text(errors="replace").splitlines():
                try:
                    log_event = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if log_event.get("event") == "rust_odds_stage":
                    full_stages[log_event.get("stage")] = log_event.get("elapsed_ms")
            setup_work = sum(sum(event.get(key, 0) for key in ("training_operation_work", "family_collection_work", "clone_work", "fit_work", "validation_work")) for event in fallback)
            final_work = sum(event.get("final_work", 0) for event in fallback)
            stages = {}
            for event in fallback:
                for stage, elapsed in event.get("stage_ms", {}).items():
                    stages[stage] = stages.get(stage, 0) + elapsed
            remaining_reachable_zeros = [
                {"team_id": team, "rank_index": rank}
                for team, ranks in matrix(candidate).items()
                for rank, estimate in ranks.items()
                if estimate.get("probability", 0) == 0 and estimate.get("reachability") == "reachable"
            ]
            confirms = record["confirmation_summaries"][0] if record["confirmation_summaries"] else {}
            row = {
                "arm": arm,
                "seed": seed,
                "export_sha256": record["export_sha256"],
                "wall_seconds": record["wall_seconds"],
                "child_user_seconds": record["child_user_seconds"],
                "child_system_seconds": record["child_system_seconds"],
                "child_cpu_seconds": record["child_cpu_seconds"],
                "new_positive_cells": gains,
                "new_positive_cell_count": len(gains),
                "new_unique_team_ids": sorted({cell["team_id"] for cell in gains}),
                "gains_already_seen_in_individual_v4_runs": [cell for cell in gains if (seed, cell["team_id"], cell["rank_index"]) in separate],
                "baseline_positive_cells_changed_excluding_work_spent": positive_changed,
                "changed_estimate_cells_excluding_work_spent": changed,
                "game_importance_identical": same_importance,
                "remaining_reachable_zero_count": len(remaining_reachable_zeros),
                "remaining_reachable_zero_cells": remaining_reachable_zeros,
                "initial_post_native_bank_free": confirms.get("family_fallback_initial_post_native_bank_free"),
                "attempts": confirms.get("family_fallback_attempts"),
                "quality_declines": confirms.get("family_fallback_quality_declines"),
                "budget_declines": confirms.get("family_fallback_budget_declines"),
                "unsupported_skips": confirms.get("family_fallback_unsupported_skips"),
                "actual_fallback_work": confirms.get("family_fallback_added_work"),
                "published_charged_work": confirms.get("family_fallback_published_charged_work"),
                "final_bank_free": confirms.get("family_fallback_final_bank_free"),
                "final_bank_reserved": confirms.get("family_fallback_final_bank_reserved"),
                "stopped_after_overrun": confirms.get("family_fallback_stopped_after_overrun"),
                "overrun_count": sum(bool(event.get("overrun")) for event in fallback),
                "fallback_events": fallback,
                "fallback_stage_ms_sum": stages,
                "full_pipeline_stage_ms": full_stages,
                "fallback_setup_operation_work": setup_work,
                "fallback_final_operation_work": final_work,
            }
            rows.append(row)
    all_gains = {(row["seed"], cell["team_id"], cell["rank_index"]) for row in rows if row["arm"] == "all-family-1x" for cell in row["new_positive_cells"]}
    unique_combined = sorted(all_gains - separate)
    OUT.write_text(json.dumps({
        "baseline_positive_and_game_importance_preserved_all_runs": preserved,
        "v4_individual_unique_gains": [list(x) for x in sorted(separate)],
        "combined_family_all_scope_unique_gains_vs_individual_v4": [list(x) for x in unique_combined],
        "combined_family_new_positive_total": sum(row["new_positive_cell_count"] for row in rows if row["arm"] == "all-family-1x"),
        "combined_family_new_positive_by_seed": {str(seed): sum(row["new_positive_cell_count"] for row in rows if row["arm"] == "all-family-1x" and row["seed"] == seed) for seed in SEEDS},
        "runs": rows,
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
