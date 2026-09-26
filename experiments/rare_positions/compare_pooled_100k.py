#!/usr/bin/env python3
"""Screen point/rank pooling against equal-sample ordinary MC scouts."""

import argparse
import csv
import json
import math
import statistics
from collections import defaultdict
from pathlib import Path


def references(directory):
    result = {}
    for path in Path(directory).glob("group-*.reference.json"):
        ref = json.loads(path.read_text())
        result[ref["group"]] = {
            "hash": ref["input_sha256"],
            "samples": ref["samples"],
            "cells": {(c["team"], c["position"]): c for c in ref["cells"]},
        }
    return result


def rank_curves(run):
    teams = run["teams"]
    positions = len(teams)
    samples = run["scout_samples"]
    group_counts = defaultdict(lambda: [0] * positions)
    group_totals = defaultdict(int)
    prepared = []
    for team in teams:
        pmf = {int(s): p for s, p in team["additional_pmf"].items()}
        point_counts = {int(s): count for s, count in team["point_counts"].items()}
        point_ranks = {int(s): counts for s, counts in team["point_rank_counts"].items()}
        mean_points = team["current_points"] + sum(s * p for s, p in pmf.items())
        prepared.append((team, pmf, point_counts, point_ranks, mean_points))
        for added, counts in point_ranks.items():
            final = team["current_points"] + added
            group_totals[final] += point_counts[added]
            for rank, count in enumerate(counts):
                group_counts[final][rank] += count

    predictions = {}
    for team, pmf, point_counts, point_ranks, mean_points in prepared:
        methods = {name: [0.0] * positions for name in (
            "plain", "pooled", "matched_3", "matched_6", "matched_12",
            "shrink_1", "shrink_5", "shrink_20", "calibrated", "blend",
            "hybrid_10", "hybrid_25",
        )}
        for rank, count in enumerate(team["rank_counts"]):
            methods["plain"][rank] = count / samples

        for added, mass in pmf.items():
            if mass <= 0:
                continue
            final = team["current_points"] + added
            group_total = group_totals[final]
            if group_total == 0:
                continue
            own_count = point_counts.get(added, 0)
            own_ranks = point_ranks.get(added, [0] * positions)
            matched = {}
            for width in (3, 6, 12):
                total = 0.0
                counts = [0.0] * positions
                for donor, _, donor_points, donor_ranks, donor_mean in prepared:
                    delta = final - donor["current_points"]
                    n = donor_points.get(delta, 0)
                    if n == 0:
                        continue
                    weight = math.exp(-abs(donor_mean - mean_points) / width)
                    total += n * weight
                    for rank, hits in enumerate(donor_ranks[delta]):
                        counts[rank] += hits * weight
                matched[width] = (counts, total)
            for rank in range(positions):
                group_q = group_counts[final][rank] / group_total
                methods["pooled"][rank] += mass * group_q
                for width in (3, 6, 12):
                    counts, total = matched[width]
                    methods[f"matched_{width}"][rank] += mass * counts[rank] / total
                for prior in (1, 5, 20):
                    q = (own_ranks[rank] + prior * group_q) / (own_count + prior)
                    methods[f"shrink_{prior}"][rank] += mass * q

        mode = max(range(positions), key=lambda rank: team["rank_counts"][rank])
        for rank in range(positions):
            if rank == mode:
                continue
            direction = 1 if rank < mode else -1
            neighbors = [rank + direction * step for step in (1, 2)
                         if 0 <= rank + direction * step < positions]
            actual = sum(team["rank_counts"][neighbor] for neighbor in neighbors)
            expected = samples * sum(methods["pooled"][neighbor] for neighbor in neighbors)
            scale = (actual + 10) / (expected + 10)
            calibrated = min(1.0, methods["pooled"][rank] * scale)
            methods["calibrated"][rank] = calibrated
            methods["blend"][rank] = (methods["plain"][rank] + calibrated) / 2
        methods["calibrated"][mode] = methods["pooled"][mode]
        methods["blend"][mode] = methods["plain"][mode]
        for rank, count in enumerate(team["rank_counts"]):
            methods["hybrid_10"][rank] = (
                methods["matched_3"][rank] if count <= 10 else methods["shrink_5"][rank]
            )
            methods["hybrid_25"][rank] = (
                methods["matched_3"][rank] if count <= 25 else methods["shrink_5"][rank]
            )
        predictions[team["id"]] = methods
    team_ids = list(predictions)
    for team_id in team_ids:
        predictions[team_id]["hybrid_10_reconciled"] = predictions[team_id]["hybrid_10"].copy()
    for _ in range(100):
        for team_id in team_ids:
            values = predictions[team_id]["hybrid_10_reconciled"]
            total = sum(values)
            if total > 0:
                predictions[team_id]["hybrid_10_reconciled"] = [value / total for value in values]
        for rank in range(positions):
            total = sum(predictions[team_id]["hybrid_10_reconciled"][rank] for team_id in team_ids)
            if total > 0:
                for team_id in team_ids:
                    predictions[team_id]["hybrid_10_reconciled"][rank] /= total
    return predictions


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scouts", required=True)
    parser.add_argument("--reference-dir", required=True)
    parser.add_argument("--selection-reference-dir")
    parser.add_argument("--output", required=True)
    parser.add_argument("--cell-csv")
    args = parser.parse_args()
    truth = references(args.reference_dir)
    selection = references(args.selection_reference_dir or args.reference_dir)
    rows = []
    seen_runs = set()
    with open(args.scouts) as source:
        for line in source:
            run = json.loads(line)
            group = run["group"]
            key = group, run["seed"]
            if key in seen_runs:
                raise ValueError(f"duplicate scout run: {key}")
            seen_runs.add(key)
            if run["scout_samples"] != 100000:
                raise ValueError(f"expected 100,000 scout seasons for {key}")
            if run["input_sha256"] != truth[group]["hash"] or run["input_sha256"] != selection[group]["hash"]:
                raise ValueError(f"stale fixture for group {group}")
            for team, methods in rank_curves(run).items():
                for method, probabilities in methods.items():
                    for rank, estimate in enumerate(probabilities):
                        cell = team, rank
                        rows.append({
                            "group": group, "seed": run["seed"], "team": team,
                            "position": rank + 1, "method": method,
                            "estimate": estimate,
                            "reference_probability": truth[group]["cells"][cell]["p"],
                            "reference_hits": truth[group]["cells"][cell]["count"],
                            "reference_samples": truth[group]["samples"],
                            "selection_probability": selection[group]["cells"][cell]["p"],
                        })
    bands = {
        "all": lambda p: True,
        "below_5e-7": lambda p: p < 5e-7,
        "near_1e-6": lambda p: 5e-7 <= p < 2e-6,
        "1e-6_to_1e-5": lambda p: 1e-6 <= p < 1e-5,
        "5e-6_to_1e-4": lambda p: 5e-6 <= p < 1e-4,
        "1e-4_to_1e-3": lambda p: 1e-4 <= p < 1e-3,
        "common_ge_1e-3": lambda p: p >= 1e-3,
    }
    summary = {}
    for band, select in bands.items():
        summary[band] = {}
        for method in sorted({row["method"] for row in rows}):
            chosen = [row for row in rows if row["method"] == method and select(row["selection_probability"])]
            errors = [row["estimate"] - row["reference_probability"] for row in chosen]
            summary[band][method] = {
                "cells": len({(r["group"], r["team"], r["position"]) for r in chosen}),
                "runs": len(chosen),
                "mae": statistics.fmean(map(abs, errors)),
                "rmse": math.sqrt(statistics.fmean(e * e for e in errors)),
                "bias": statistics.fmean(errors),
                "nonzero_fraction": sum(r["estimate"] > 0 for r in chosen) / len(chosen),
            }
    by_group = {}
    for group in sorted({row["group"] for row in rows}):
        by_group[group] = {}
        for method in ("plain", "pooled", "matched_3", "shrink_5", "hybrid_10"):
            chosen = [row for row in rows if row["group"] == group and row["method"] == method
                      and bands["near_1e-6"](row["selection_probability"])]
            errors = [row["estimate"] - row["reference_probability"] for row in chosen]
            if errors:
                by_group[group][method] = {
                    "cells": len({(r["team"], r["position"]) for r in chosen}),
                    "rmse": math.sqrt(statistics.fmean(e * e for e in errors)),
                    "mae": statistics.fmean(map(abs, errors)),
                    "bias": statistics.fmean(errors),
                }
    by_group_all = {}
    for group in sorted({row["group"] for row in rows}):
        by_group_all[group] = {}
        for method in ("plain", "hybrid_10", "hybrid_10_reconciled"):
            chosen = [row for row in rows if row["group"] == group and row["method"] == method]
            errors = [row["estimate"] - row["reference_probability"] for row in chosen]
            by_group_all[group][method] = {
                "rmse": math.sqrt(statistics.fmean(e * e for e in errors)),
                "mae": statistics.fmean(map(abs, errors)),
                "bias": statistics.fmean(errors),
            }
    output = {"scouts": args.scouts, "reference_dir": args.reference_dir,
              "selection_reference_dir": args.selection_reference_dir or args.reference_dir,
              "summary": summary, "by_group_near_1e-6": by_group,
              "by_group_all": by_group_all}
    Path(args.output).write_text(json.dumps(output, indent=2) + "\n")
    if args.cell_csv:
        grouped = defaultdict(lambda: defaultdict(list))
        for row in rows:
            key = row["group"], row["team"], row["position"]
            grouped[key][row["method"]].append(row["estimate"])
        columns = ["group", "team", "position", "scout_seeds", "selection_reference_hits",
                   "selection_reference_probability", "holdout_reference_hits",
                   "holdout_reference_probability"]
        for method in ("plain", "pooled", "matched_3", "shrink_5", "hybrid_10_reconciled"):
            columns += [f"{method}_mean", f"{method}_seed_sd"]
        with open(args.cell_csv, "w", newline="") as destination:
            writer = csv.DictWriter(destination, fieldnames=columns, lineterminator="\n")
            writer.writeheader()
            for group, team, position in sorted(grouped):
                ref_key = team, position - 1
                selected = selection[group]["cells"][ref_key]
                actual = truth[group]["cells"][ref_key]
                methods = grouped[group, team, position]
                cell_row = {
                    "group": group, "team": team, "position": position,
                    "scout_seeds": len(methods["plain"]),
                    "selection_reference_hits": selected["count"],
                    "selection_reference_probability": selected["p"],
                    "holdout_reference_hits": actual["count"],
                    "holdout_reference_probability": actual["p"],
                }
                for method in ("plain", "pooled", "matched_3", "shrink_5", "hybrid_10_reconciled"):
                    values = methods[method]
                    cell_row[f"{method}_mean"] = statistics.fmean(values)
                    cell_row[f"{method}_seed_sd"] = statistics.stdev(values) if len(values) > 1 else 0
                writer.writerow(cell_row)
    for band, methods in summary.items():
        print(band, "cells", next(iter(methods.values()))["cells"])
        for method, metrics in sorted(methods.items(), key=lambda pair: pair[1]["rmse"]):
            print(f"  {method:12s} RMSE={metrics['rmse']:.3g} MAE={metrics['mae']:.3g} "
                  f"bias={metrics['bias']:.3g} nonzero={metrics['nonzero_fraction']:.2f}")
    print("near_1e-6 by group")
    for group, methods in by_group.items():
        print(group, "cells", methods["plain"]["cells"],
              "plain", f"{methods['plain']['rmse']:.3g}",
              "matched_3", f"{methods['matched_3']['rmse']:.3g}")
    print("all cells by group")
    for group, methods in by_group_all.items():
        print(group, "plain", f"{methods['plain']['rmse']:.3g}",
              "hybrid_reconciled", f"{methods['hybrid_10_reconciled']['rmse']:.3g}")


if __name__ == "__main__":
    main()
