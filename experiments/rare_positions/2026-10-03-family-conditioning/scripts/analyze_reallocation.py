#!/usr/bin/env python3
"""Summarize allocation-only screens against version 1 controls."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SEEDS = (808, 1669, 1993, 2281, 2293)


def read_json(path: Path):
    return json.loads(path.read_text())


def parse_events(path: Path) -> list[dict]:
    events = []
    for line in path.read_text(errors="replace").splitlines():
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and isinstance(value.get("event"), str):
            events.append(value)
    return events


def find_selected(events: list[dict], name: str) -> dict | None:
    return next((event for event in reversed(events)
                 if event.get("event") == name
                 and (event.get("team") == 17 or event.get("team_id") == 17)
                 and event.get("rank") == 12), None)


def load_arm(seed: int, path: Path, arm: str) -> dict:
    export = read_json(path / f"{arm}.json")
    command = read_json(path / f"{arm}.command.json")
    events = parse_events(path / f"{arm}.stderr.log")
    confirm = find_selected(events, "rust_odds_rare_tail_confirm_more")
    fit = find_selected(events, "rust_odds_family_conditioning_fit")
    native = find_selected(events, "rust_odds_neighbor_native_control")
    complete = next((event for event in reversed(events)
                     if event.get("event") == "rust_odds_complete"), {})
    target = export["rare_position_estimates"]["17"]["11"]
    return {
        "seed": seed, "arm": arm, "probability": target["probability"],
        "std_err": target["std_err"], "relative_se": target.get("relative_se"),
        "ess": target["ess"], "samples": target["samples"], "hits": target["hits"],
        "design": target["design"], "reachability": target["reachability"],
        "conditional_samples": target.get("conditional_samples"),
        "conditional_hits": target.get("conditional_hits"),
        "conditional_mass": target.get("conditional_mass"),
        "request_work_spent": complete.get("work_spent"),
        "coverage": complete.get("cells"),
        "confirm": confirm, "fit": fit, "native_control": native,
        "wall_seconds": command["wall_seconds"],
        "child_user_seconds": command["child_user_seconds"],
        "child_system_seconds": command["child_system_seconds"],
        "child_cpu_seconds": command["child_cpu_seconds"],
        "effective_average_cpu_cores": command["effective_average_cpu_cores"],
        "stages_ms": command["stages_ms"],
        "export_sha256": command["export_sha256"],
        "export": export,
    }


def compare_exports(v1: dict, v2: dict) -> dict:
    changed_estimates = []
    changed_hits = []
    changed_samples = []
    changed_reachability = []
    for team in sorted(set(v1["rare_position_estimates"]) | set(v2["rare_position_estimates"])):
        left_team = v1["rare_position_estimates"].get(team, {})
        right_team = v2["rare_position_estimates"].get(team, {})
        for rank in sorted(set(left_team) | set(right_team), key=int):
            left, right = left_team.get(rank), right_team.get(rank)
            if left is None or right is None:
                changed_estimates.append({"team": team, "rank": rank, "left": left, "right": right})
                continue
            if left.get("probability") != right.get("probability"):
                changed_estimates.append({"team": team, "rank": rank,
                                          "v1": left.get("probability"),
                                          "v2": right.get("probability"),
                                          "v1_design": left.get("design"),
                                          "v2_design": right.get("design")})
            if left.get("hits") != right.get("hits"):
                changed_hits.append([team, rank, left.get("hits"), right.get("hits")])
            if left.get("samples") != right.get("samples"):
                changed_samples.append([team, rank, left.get("samples"), right.get("samples")])
            if left.get("reachability") != right.get("reachability"):
                changed_reachability.append([team, rank, left.get("reachability"),
                                             right.get("reachability")])
    v1_importance = v1.get("game_importance")
    v2_importance = v2.get("game_importance")
    importance_hash = lambda value: hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    v1_odds = v1.get("team_odds")
    v2_odds = v2.get("team_odds")
    return {
        "changed_rare_estimate_count": len(changed_estimates),
        "changed_rare_estimates": changed_estimates,
        "changed_mc_hit_count": len(changed_hits), "changed_mc_hits": changed_hits,
        "changed_sample_count": len(changed_samples), "changed_samples": changed_samples,
        "changed_reachability_count": len(changed_reachability),
        "changed_reachability": changed_reachability,
        "team_odds_identical": v1_odds == v2_odds,
        "game_importance_identical": v1_importance == v2_importance,
        "v1_game_importance_sha256": importance_hash(v1_importance),
        "v2_game_importance_sha256": importance_hash(v2_importance),
        "positive_to_positive_change_count": sum(
            row.get("v1", 0) > 0 and row.get("v2", 0) > 0 for row in changed_estimates),
        "zero_to_positive_gains": [row for row in changed_estimates
                                    if row.get("v1", 0) == 0 and row.get("v2", 0) > 0],
        "positive_to_zero_losses": [row for row in changed_estimates
                                    if row.get("v1", 0) > 0 and row.get("v2", 0) == 0],
    }


def main() -> None:
    rows, comparisons = [], []
    for seed in SEEDS:
        v1_dir = (ROOT / "logs" / "seed808" if seed == 808
                  else ROOT / "logs" / "cohort" / f"seed{seed}")
        v2_dir = ROOT / "logs" / "reallocation" / "cohort" / f"seed{seed}"
        for old_arm, new_arm in (("r57-native-1x", "native-5000-v1headroom"),
                                 ("r57-family-1x", "family-5000-h08")):
            old = load_arm(seed, v1_dir, old_arm)
            new = load_arm(seed, v2_dir, new_arm)
            comparisons.append({"seed": seed, "v1_arm": old_arm,
                                "reallocated_arm": new_arm,
                                "comparison": compare_exports(old["export"], new["export"]),
                                "latency_delta_v2_minus_v1": {
                                    "wall_seconds": new["wall_seconds"] - old["wall_seconds"],
                                    "child_user_seconds": new["child_user_seconds"] - old["child_user_seconds"],
                                    "child_system_seconds": new["child_system_seconds"] - old["child_system_seconds"],
                                    "child_cpu_seconds": new["child_cpu_seconds"] - old["child_cpu_seconds"],
                                    "stages_ms": {
                                        stage: new["stages_ms"].get(stage, 0) - old["stages_ms"].get(stage, 0)
                                        for stage in set(old["stages_ms"]) | set(new["stages_ms"])
                                    },
                                }})
            for row in (old, new):
                row.pop("export")
                rows.append(row)
    output = {"seeds": list(SEEDS), "selected_cell": {"team_id": 17, "rank_1_based": 12},
              "arms": rows, "v1_comparisons": comparisons}
    path = ROOT / "logs" / "reallocation" / "metrics.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(output, indent=2) + "\n")


if __name__ == "__main__":
    main()
