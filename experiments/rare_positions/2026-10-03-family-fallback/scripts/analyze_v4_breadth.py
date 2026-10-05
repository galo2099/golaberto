"""Verify V4 breadth preservation and summarize selector outcomes."""
import json
from pathlib import Path

ARTIFACTS = Path(__file__).resolve().parents[1]
ROOT = ARTIFACTS / "logs" / "v4-breadth"
V3 = ARTIFACTS / "logs" / "v3-cohort"
OUT = ARTIFACTS / "data" / "v4-breadth-analysis.json"


def strip_work(value):
    if isinstance(value, dict):
        return {k: strip_work(v) for k, v in value.items() if k != "work_spent"}
    if isinstance(value, list):
        return [strip_work(v) for v in value]
    return value


def main():
    runs = []
    baseline_zero_by_selector = {}
    preserved = True
    for selector in ("16:14", "16:15"):
        team, one_based_rank = selector.split(":")
        zero_key = (team, str(int(one_based_rank) - 1))
        baseline_zero_by_selector[selector] = 0
        for seed in (808, 1669, 1993, 2281, 2293):
            baseline = json.loads((V3 / f"seed{seed}" / "current-rust.json").read_text())
            selected_base = baseline["rare_position_estimates"].get(zero_key[0], {}).get(zero_key[1], {})
            if selected_base.get("probability", 0) == 0:
                baseline_zero_by_selector[selector] += 1
            for mode, arm in (("family", "family-1x"), ("native_retry", "native-retry-1x")):
                directory = ROOT / selector.replace(":", "-") / arm / f"seed{seed}"
                candidate = json.loads((directory / "export.json").read_text())
                record = json.loads((directory / "record.json").read_text())
                changed, positive_changed = [], []
                for tid, positions in baseline["rare_position_estimates"].items():
                    for rank, original in positions.items():
                        current = candidate["rare_position_estimates"][tid][rank]
                        if strip_work(original) != strip_work(current):
                            key = f"{tid}:{rank}"
                            changed.append(key)
                            if original.get("probability", 0) > 0:
                                positive_changed.append(key)
                same_importance = baseline["game_importance"] == candidate["game_importance"]
                if positive_changed or not same_importance:
                    preserved = False
                fallback = record["fallback_events"][0]["fallback"] if record["fallback_events"] else None
                current_target = candidate["rare_position_estimates"].get(zero_key[0], {}).get(zero_key[1], {})
                baseline_zeros = sum(
                    estimate.get("probability", 0) == 0
                    for positions in baseline["rare_position_estimates"].values()
                    for estimate in positions.values()
                )
                reachable_baseline_zeros = sum(
                    estimate.get("probability", 0) == 0 and estimate.get("reachability") == "reachable"
                    for positions in baseline["rare_position_estimates"].values()
                    for estimate in positions.values()
                )
                runs.append({
                    "selector": selector,
                    "seed": seed,
                    "mode": mode,
                    "wall_seconds": record["wall_seconds"],
                    "child_user_seconds": record["child_user_seconds"],
                    "child_system_seconds": record["child_system_seconds"],
                    "child_cpu_seconds": record["child_cpu_seconds"],
                    "export_sha256": record["export_sha256"],
                    "fallback": fallback,
                    "no_attempt_reason": None if fallback is not None else "no eligible originally paired failed-main zero-estimate proposal; no fallback event was emitted",
                    "baseline_zero_cell_count": baseline_zeros,
                    "baseline_reachable_zero_cell_count": reachable_baseline_zeros,
                    "fallback_setup_operation_work": 0 if fallback is None else sum(fallback.get(field, 0) for field in ("training_operation_work", "family_collection_work", "clone_work", "fit_work", "validation_work")),
                    "fallback_final_operation_work": 0 if fallback is None else fallback.get("final_work", 0),
                    "fallback_overrun": False if fallback is None else fallback.get("overrun", False),
                    "baseline_selected_estimate": selected_base,
                    "candidate_selected_estimate": current_target,
                    "new_positive_from_baseline_zero": selected_base.get("probability", 0) == 0 and current_target.get("probability", 0) > 0,
                    "baseline_positive_cells_changed_excluding_work_spent": positive_changed,
                    "changed_estimate_cells_excluding_work_spent": changed,
                    "game_importance_identical": same_importance,
                })
    OUT.write_text(json.dumps({
        "baseline_positive_and_game_importance_preserved_all_runs": preserved,
        "baseline_zero_selected_cells_by_selector": baseline_zero_by_selector,
        "baseline_reachable_zero_cell_count_by_seed": {
            str(seed): next(row["baseline_reachable_zero_cell_count"] for row in runs if row["seed"] == seed)
            for seed in (808, 1669, 1993, 2281, 2293)
        },
        "new_positive_results": sum(row["new_positive_from_baseline_zero"] for row in runs),
        "new_positive_by_selector_mode": {
            f"{selector}/{mode}": sum(row["new_positive_from_baseline_zero"] for row in runs if row["selector"] == selector and row["mode"] == mode)
            for selector in ("16:14", "16:15") for mode in ("family", "native_retry")
        },
        "runs": runs,
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
