"""Summarize paired exports and logs for the isolated R45-R47 experiments."""

import json
import statistics
import sys
from pathlib import Path


root = Path(sys.argv[1])
summary = json.loads((root / "summary.json").read_text())
out = {
    "pairs": len(summary["pairs"]),
    "gains": [],
    "losses": [],
    "proof_changes": [],
    "groups": {},
    "initial_mc_identical": True,
    "initial_mc_mismatches": 0,
    "initial_mc_samples_100k": True,
    "initial_mc": {
        "baseline": {"cells": 0, "total_samples": 0, "total_hits": 0},
        "candidate": {"cells": 0, "total_samples": 0, "total_hits": 0},
    },
    "importance_identical": True,
    "identical_exports": 0,
    "rare_positive": {"baseline": 0, "candidate": 0},
    "training": [],
}

for row in summary["pairs"]:
    name = row["input"]
    seed = row["seed"]
    group = out["groups"].setdefault(
        name,
        {
            "gains": 0,
            "losses": 0,
            "baseline_cpu_ms": 0.0,
            "candidate_cpu_ms": 0.0,
            "http_ms": {"baseline": [], "candidate": []},
            "tail_ms": {"baseline": [], "candidate": []},
        },
    )
    exports = {}
    for arm in ("baseline", "candidate"):
        exports[arm] = json.loads(
            (root / f"{Path(name).stem}-{seed}-{arm}.json").read_text()
        )
        timing = row["timing"][arm]
        group[f"{arm}_cpu_ms"] += timing["cpu_ms"]
        group["http_ms"][arm].append(timing["http_ms"])
        stages = timing.get("stages") or {}
        group["tail_ms"][arm].append(stages.get("search.rare_tail", 0.0))

        for estimates in exports[arm]["rare_position_estimates"].values():
            # Count any positive final estimate whose initial 100k pool had no hits.
            # There is deliberately no probability floor here.
            out["rare_positive"][arm] += sum(
                estimate.get("hits") == 0 and estimate["probability"] > 0.0
                for estimate in estimates.values()
            )
            for estimate in estimates.values():
                stats = out["initial_mc"][arm]
                stats["cells"] += 1
                samples = estimate.get("samples", 0)
                hits = estimate.get("hits", 0)
                stats["total_samples"] += samples
                stats["total_hits"] += hits
                out["initial_mc_samples_100k"] &= samples == 100_000

    comparison = row.get("comparison") or {}
    for key, destination in (("gains", "gains"), ("lost", "losses")):
        for change in comparison.get(key, []):
            item = dict(input=name, seed=seed, **change)
            if key == "lost":
                item["probability"] = exports["baseline"][
                    "rare_position_estimates"
                ][str(change["team"])][str(change["position"] - 1)]["probability"]
            out[destination].append(item)
            group[destination] += 1

    baseline = exports["baseline"]
    candidate = exports["candidate"]
    out["identical_exports"] += baseline == candidate
    out["importance_identical"] &= (
        baseline.get("game_importance") == candidate.get("game_importance")
    )

    for team, estimates in baseline["rare_position_estimates"].items():
        for rank, before in estimates.items():
            after = candidate["rare_position_estimates"][team][rank]
            if before.get("reachability") != after.get("reachability"):
                out["proof_changes"].append(
                    {
                        "input": name,
                        "seed": seed,
                        "team": team,
                        "rank": rank,
                        "before": before,
                        "after": after,
                    }
                )

            same_initial_mc = (
                before.get("samples") == after.get("samples")
                and before.get("hits") == after.get("hits")
            )
            out["initial_mc_identical"] &= same_initial_mc
            out["initial_mc_mismatches"] += not same_initial_mc

for group in out["groups"].values():
    group["median_http_ms"] = {
        arm: statistics.median(values) for arm, values in group.pop("http_ms").items()
    }
    group["median_tail_ms"] = {
        arm: statistics.median(values) for arm, values in group.pop("tail_ms").items()
    }
    group["cpu_change_pct"] = 100 * (
        group["candidate_cpu_ms"] / group["baseline_cpu_ms"] - 1
    )
    group["http_change_pct"] = 100 * (
        group["median_http_ms"]["candidate"] / group["median_http_ms"]["baseline"]
        - 1
    )

for log_file in sorted(root.glob("*candidate.log")):
    for line in log_file.read_text().splitlines():
        if not line.startswith("{"):
            continue
        event = json.loads(line)
        if event.get("event") != "rust_odds_rare_tail_training":
            continue
        pilot = event.get("pilot")
        plan = (pilot or {}).get("plan", {})
        if plan.get("experiment"):
            out["training"].append(
                {
                    "file": log_file.name,
                    "team": event["team"],
                    "rank": event["rank"],
                    "training_ms": event["training_ms"],
                    "training_work": event["training_work_units"],
                    "pilot": pilot,
                }
            )

(root / "analysis.json").write_text(json.dumps(out, indent=2) + "\n")
print(json.dumps({key: value for key, value in out.items() if key not in ("training", "groups")}, indent=2))
for name, group in out["groups"].items():
    print(name, json.dumps(group))
print("training:", len(out["training"]))
