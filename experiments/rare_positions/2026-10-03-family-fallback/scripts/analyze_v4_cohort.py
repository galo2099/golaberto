#!/usr/bin/env python3
"""Check V4 cell preservation and summarize each authorized control."""
import json
from pathlib import Path

ARTIFACTS = Path(__file__).resolve().parents[1]
ROOT = ARTIFACTS / "logs" / "v4-cohort"
V3 = ARTIFACTS / "logs" / "v3-cohort"
OUT = ARTIFACTS / "data" / "v4-cohort-analysis.json"
SEEDS = [808, 1669, 1993, 2281, 2293]
ARMS = ["native-retry-1x", "native-retry-10x", "family-10x-cap5000", "family-10x-default50k", "family-1x"]


def strip_work(value):
    if isinstance(value, dict):
        return {key: strip_work(item) for key, item in value.items() if key != "work_spent"}
    if isinstance(value, list):
        return [strip_work(item) for item in value]
    return value


def main():
    rows = []
    all_preserved = True
    for arm in ARMS:
        for seed in SEEDS:
            run_dir = ROOT / arm / f"seed{seed}"
            if not run_dir.exists():
                continue
            record = json.loads((run_dir / "record.json").read_text())
            candidate = json.loads((run_dir / "export.json").read_text())
            baseline_dir = V3 / f"seed{seed}"
            baseline = json.loads((baseline_dir / "current-rust.json").read_text())
            changed = []
            positive_changed = []
            for team, positions in baseline["rare_position_estimates"].items():
                for rank, estimate in positions.items():
                    if strip_work(estimate) != strip_work(candidate["rare_position_estimates"][team][rank]):
                        cell = f"{team}:{rank}"
                        changed.append(cell)
                        if estimate.get("probability", 0) > 0:
                            positive_changed.append(cell)
            if positive_changed:
                all_preserved = False
            if baseline["game_importance"] != candidate["game_importance"]:
                all_preserved = False
            event = record["fallback_events"][0]["fallback"] if record["fallback_events"] else None
            target = candidate["rare_position_estimates"].get("17", {}).get("11", {})
            quality = None
            if event:
                summary = event.get("family_summary", {})
                quality = summary.get("final") if isinstance(summary, dict) else None
                if quality is None and isinstance(summary, dict) and isinstance(summary.get("fit"), list):
                    quality = summary.get("final")
            if not event:
                gate_detail = "no fallback event; selector did not have an eligible originally paired failed-main zero-estimate proposal"
            elif event.get("overrun"):
                gate_detail = "actual fallback work exceeded the grant; publication rejected"
            elif not event.get("settlement_within_grant"):
                gate_detail = "native plus fallback actual work exceeded the configured cell limit; publication rejected"
            elif quality and not quality.get("main_publishable"):
                gate_detail = (
                    "fresh main failed unchanged publishability gate; independent check was skipped"
                )
            elif quality and not quality.get("accepted"):
                gate_detail = "fresh main/check pair failed unchanged accepted() gate"
            elif event.get("published"):
                gate_detail = "fresh main/check pair passed publication and actual-work gates"
            else:
                gate_detail = event.get("reason")
            v3_baseline_run = json.loads((baseline_dir / "summary.json").read_text())["records"][0]
            row = {
                "arm": arm,
                "seed": seed,
                "export_sha256": record["export_sha256"],
                "wall_seconds": record["wall_seconds"],
                "child_user_seconds": record["child_user_seconds"],
                "child_system_seconds": record["child_system_seconds"],
                "child_cpu_seconds": record["child_cpu_seconds"],
                "stages_ms": record["stages_ms"],
                "v3_baseline_wall_seconds": v3_baseline_run["wall_seconds"],
                "v3_baseline_child_cpu_seconds": v3_baseline_run["child_cpu_seconds"],
                "probability_team17_rank12": target.get("probability"),
                "target_estimate": target,
                "fallback": event,
                "publication_gate_detail": gate_detail,
                "baseline_positive_cells_changed_excluding_work_spent": positive_changed,
                "changed_estimate_cells_excluding_work_spent": changed,
                "game_importance_identical": baseline["game_importance"] == candidate["game_importance"],
            }
            rows.append(row)
    OUT.write_text(json.dumps({
        "baseline_positive_and_game_importance_preserved_all_runs": all_preserved,
        "v3_comparator": str(V3),
        "runs": rows,
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
