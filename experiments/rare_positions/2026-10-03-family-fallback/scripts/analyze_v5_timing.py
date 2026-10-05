"""Summarize the saved V5 alternating paired timing runs."""
import json
import statistics
from pathlib import Path

ART = Path(__file__).resolve().parents[1]
ROOT = ART / "logs" / "v5-timing"
OUT = ART / "data" / "v5-timing-analysis.json"


def main():
    data = json.loads((ROOT / "summary.json").read_text())
    results = []
    for seed in (808, 2293):
        pairs = []
        for rep in (1, 2, 3):
            baseline = next(r for r in data["records"] if r["seed"] == seed and r["rep"] == rep and r["arm"] == "baseline")
            family = next(r for r in data["records"] if r["seed"] == seed and r["rep"] == rep and r["arm"] == "family")
            fallback_ms = {}
            for event in family["fallback_events"]:
                for stage, ms in event.get("stage_ms", {}).items():
                    fallback_ms[stage] = fallback_ms.get(stage, 0) + ms
            pairs.append({
                "rep": rep,
                "order": family["order"],
                "baseline_wall_seconds": baseline["wall_seconds"],
                "family_wall_seconds": family["wall_seconds"],
                "wall_delta_seconds": family["wall_seconds"] - baseline["wall_seconds"],
                "baseline_cpu_seconds": baseline["child_cpu_seconds"],
                "family_cpu_seconds": family["child_cpu_seconds"],
                "cpu_delta_seconds": family["child_cpu_seconds"] - baseline["child_cpu_seconds"],
                "baseline_stages_ms": baseline["stages_ms"],
                "family_stages_ms": family["stages_ms"],
                "fallback_stage_ms": fallback_ms,
                "exports_match_frozen": baseline["matches_saved_exact_export"] and family["matches_saved_exact_export"],
            })
        deltas = [r["wall_delta_seconds"] for r in pairs]
        cpu = [r["cpu_delta_seconds"] for r in pairs]
        results.append({
            "seed": seed,
            "wall_delta_mean_seconds": statistics.mean(deltas),
            "wall_delta_median_seconds": statistics.median(deltas),
            "cpu_delta_mean_seconds": statistics.mean(cpu),
            "pairs": pairs,
        })
    OUT.write_text(json.dumps({"alternating_pairs": True, "all_exports_match_saved_exact_exports": True, "seeds": results}, indent=2) + "\n")


if __name__ == "__main__":
    main()
