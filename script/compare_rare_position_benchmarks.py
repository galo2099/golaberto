#!/usr/bin/env python3
"""Compare paired equal-work benchmark JSONL files and save a record."""

import argparse
import json
import statistics
from pathlib import Path


METRICS = ("coverage_score", "nonzero", "ess_ge_4", "ess_ge_10", "ess_ge_25", "median_rel_se", "p90_rel_se")


def probability_band(p):
    if p >= 1e-3:
        return "ge_1e-3"
    if p >= 1e-4:
        return "1e-4_to_1e-3"
    if p >= 1e-5:
        return "1e-5_to_1e-4"
    if p >= 1e-6:
        return "1e-6_to_1e-5"
    return "lt_1e-6"


def cell_band_metrics(row):
    """Recompute bands from cells so older JSONL versions stay comparable."""
    bands = {}
    for cell in row["cells"]:
        band = bands.setdefault(probability_band(cell["reference_probability"]), [])
        band.append(cell)
    result = {}
    for name, cells in bands.items():
        reliable = [c for c in cells if c["reference_count"] >= 25]
        result[name] = {
            "reference_count_ge_25": len(reliable),
            "coverage_score": sum(min(c["ess"] / 10, 1) for c in cells),
            "ess_ge_4": sum(c["ess"] >= 4 for c in cells),
            "ess_ge_10": sum(c["ess"] >= 10 for c in cells),
            "ess_ge_25": sum(c["ess"] >= 25 for c in cells),
            "mean_estimated_variance": statistics.mean(c["std_err"] ** 2 for c in cells),
        }
        if reliable:
            result[name]["mean_estimated_relative_variance_reliable"] = statistics.mean(
                c["std_err"] ** 2 / c["reference_probability"] ** 2 for c in reliable
            )
    return result


def quantile(values, fraction):
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    index = fraction * (len(ordered) - 1)
    low = int(index)
    high = min(low + 1, len(ordered) - 1)
    return ordered[low] * (high - index) + ordered[high] * (index - low)


def summary(values):
    return {
        "mean": statistics.mean(values),
        "median": statistics.median(values),
        "stddev": statistics.stdev(values) if len(values) > 1 else 0.0,
        "p10": quantile(values, 0.1),
        "p90": quantile(values, 0.9),
    }


def load(path):
    rows = {}
    for line in path.read_text().splitlines():
        if not line.strip():
            continue
        row = json.loads(line)
        key = (row["group"], row["seed"], row["method"])
        if key in rows:
            raise ValueError(f"duplicate run {key} in {path}")
        work = row["work"]
        if work["total"] > work["limit"] or not 0 < work["limit"] <= 35_000_000:
            raise ValueError(f"work budget violation {key} in {path}")
        rows[key] = row
    return rows


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--hypothesis", required=True)
    parser.add_argument("--decision", choices=("accept", "reject", "inconclusive"), default="inconclusive")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    base, cand = load(args.baseline), load(args.candidate)
    if base.keys() != cand.keys():
        raise ValueError("baseline and candidate must contain identical group/seed/method keys")
    for key in base:
        if base[key]["reference_samples"] != cand[key]["reference_samples"]:
            raise ValueError(f"reference mismatch for {key}")
        if base[key]["work"]["limit"] != cand[key]["work"]["limit"]:
            raise ValueError(f"work limit mismatch for {key}")
    report = {
        "hypothesis": args.hypothesis,
        "decision": args.decision,
        "baseline_commit": sorted({row["commit"] for row in base.values()}),
        "candidate_commit": sorted({row["commit"] for row in cand.values()}),
        "groups": sorted({key[0] for key in base}),
        "seeds": sorted({key[1] for key in base}),
        "paired": {},
        "against_plain": {},
    }
    base_bands = {key: cell_band_metrics(row) for key, row in base.items()}
    cand_bands = {key: cell_band_metrics(row) for key, row in cand.items()}
    for method in ("diversified", "plain_mc"):
        keys = sorted(key for key in base if key[2] == method)
        if not keys:
            raise ValueError(f"missing {method} runs")
        report["paired"][method] = {
            metric: summary([cand[key]["quality"][metric] - base[key]["quality"][metric] for key in keys])
            for metric in METRICS
        }
        band_names = sorted({name for key in keys for name in base[key]["truth_by_band"]})
        report["paired"][method]["bands"] = {}
        for band in band_names:
            report["paired"][method]["bands"][band] = {}
            for metric in ("coverage_score", "ess_ge_4", "ess_ge_10", "ess_ge_25", "mean_estimated_variance", "mean_estimated_relative_variance_reliable", "rmse", "mean_absolute_error", "mean_z", "z_variance", "coverage_95"):
                diffs = []
                needs_reference_hits = metric in (
                    "mean_estimated_relative_variance_reliable", "rmse", "mean_absolute_error",
                    "mean_z", "z_variance", "coverage_95"
                )
                for key in keys:
                    if band not in cand_bands[key] or band not in base_bands[key]:
                        continue
                    if needs_reference_hits and base_bands[key][band]["reference_count_ge_25"] == 0:
                        continue
                    cand_value = cand_bands[key][band].get(metric)
                    base_value = base_bands[key][band].get(metric)
                    if cand_value is None:
                        cand_value = cand[key]["truth_by_band"].get(band, {}).get(metric)
                    if base_value is None:
                        base_value = base[key]["truth_by_band"].get(band, {}).get(metric)
                    if cand_value is not None and base_value is not None:
                        diffs.append(cand_value - base_value)
                if diffs:
                    report["paired"][method]["bands"][band][metric] = summary(diffs)
    for arm_name, rows in (("baseline", base), ("candidate", cand)):
        group_seeds = sorted({(group, seed) for group, seed, _ in rows})
        for group, seed in group_seeds:
            if rows[(group, seed, "diversified")]["work"]["limit"] != rows[(group, seed, "plain_mc")]["work"]["limit"]:
                raise ValueError(f"method work limit mismatch for {(group, seed)} in {arm_name}")
        report["against_plain"][arm_name] = {
            metric: summary([
                rows[(group, seed, "diversified")]["quality"][metric] - rows[(group, seed, "plain_mc")]["quality"][metric]
                for group, seed in group_seeds
            ]) for metric in METRICS
        }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({
        "paired_coverage_score": report["paired"]["diversified"]["coverage_score"],
        "paired_ess_ge_10": report["paired"]["diversified"]["ess_ge_10"],
        "paired_ess_ge_25": report["paired"]["diversified"]["ess_ge_25"],
        "candidate_vs_plain_coverage_score": report["against_plain"]["candidate"]["coverage_score"],
    }, indent=2))


if __name__ == "__main__":
    main()
