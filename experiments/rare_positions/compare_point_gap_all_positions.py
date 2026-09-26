#!/usr/bin/env python3
"""Summarize the saved five-group point-gap scouts against their references."""

import csv
import json
import statistics
from collections import defaultdict
from pathlib import Path


ROOT = Path(__file__).resolve().parent
RUNS = ROOT / "local/2026-09-25/point-gap-smoothed-fivegroups-final/point-gap-runs.jsonl"
REFERENCES = ROOT / "local/2026-09-24/references"
CSV_OUT = ROOT / "2026-09-25-point-gap-all-cells.csv"
MD_OUT = ROOT / "2026-09-25-point-gap-all-positions.md"
METHODS = ("team_only", "observed", "gap")
MIN_REFERENCE_HITS = 25


def fmt(value):
    return f"{value:.4g}"


def mean_error(rows, method):
    return statistics.fmean(row[f"{method}_absolute_error"] for row in rows)


def read_runs():
    runs = defaultdict(dict)
    with RUNS.open() as source:
        for line in source:
            run = json.loads(line)
            group, seed = run["group"], run["seed"]
            if seed in runs[group]:
                raise ValueError(f"duplicate run: {group}/{seed}")
            runs[group][seed] = run
    if len(runs) != 5 or any(len(seeds) != 20 for seeds in runs.values()):
        raise ValueError("expected five groups and 20 seeds per group")
    return runs


def summarize_cells(runs):
    output = []
    for group, seeds in sorted(runs.items()):
        with (REFERENCES / f"group-{group}.reference.json").open() as source:
            reference = json.load(source)
        if reference["samples"] != 5_000_000:
            raise ValueError(f"unexpected reference size for {group}")
        reference_cells = {(cell["team"], cell["position"]): cell for cell in reference["cells"]}
        by_cell = defaultdict(list)
        for seed, run in sorted(seeds.items()):
            if run["scout_samples"] != 1000 or run["reference_samples"] != reference["samples"]:
                raise ValueError(f"unexpected run size for {group}/{seed}")
            if len(run["cells"]) != len(reference_cells):
                raise ValueError(f"incomplete cells for {group}/{seed}")
            for cell in run["cells"]:
                key = cell["team"], cell["position"]
                ref = reference_cells[key]
                if cell["reference_count"] != ref["count"] or cell["reference_probability"] != ref["p"]:
                    raise ValueError(f"reference mismatch for {group}/{seed}/{key}")
                by_cell[key].append(cell)
        if len(by_cell) != 400 or any(len(cells) != 20 for cells in by_cell.values()):
            raise ValueError(f"incomplete group {group}")
        for (team, position), cells in sorted(by_cell.items()):
            ref = reference_cells[team, position]
            row = {
                "group": group,
                "team": team,
                "position": position + 1,
                "reference_probability": ref["p"],
                "reference_hits": ref["count"],
            }
            for method in METHODS:
                estimates = [cell[f"{method}_estimate"] for cell in cells]
                average = statistics.fmean(estimates)
                row[f"{method}_mean"] = average
                row[f"{method}_seed_sd"] = statistics.stdev(estimates)
                row[f"{method}_absolute_error"] = abs(average - ref["p"])
                row[f"{method}_zero_seeds"] = sum(estimate == 0 for estimate in estimates)
            output.append(row)
    return output


def write_csv(rows):
    with CSV_OUT.open("w", newline="") as destination:
        writer = csv.DictWriter(destination, fieldnames=list(rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def write_markdown(rows):
    eligible = [row for row in rows if row["reference_hits"] >= MIN_REFERENCE_HITS]
    lines = [
        "# Point/rank proxy comparison at every position",
        "",
        "Date: 2026-09-25. This reanalyzes the saved point-gap experiment for",
        "groups 16498, 16653, 16982, 16983, and 16986. Each group has 20",
        "independent 1,000-season scouts (seeds 1001–1020). Each reference is",
        "an independent 5,000,000-season MC run under the same fixture model.",
        "All 2,000 team-position cells, including reference zeros, are in",
        "[`2026-09-25-point-gap-all-cells.csv`](2026-09-25-point-gap-all-cells.csv).",
        "",
        "`team_only` uses each team's own observed rank frequencies conditional",
        "on its points; `observed` pools actual point/rank outcomes from all 20",
        "teams; `gap` hypothetically moves every team to each point total in a",
        "frozen simulated final table. Each reweights its conditional rank curve",
        "with that target team's exact additional-points distribution. The",
        "table values below are mean absolute errors of the **20-seed mean**",
        "estimate against the independent reference, averaged over cells.",
        "They measure bias plus residual scout error and are not standard errors.",
        "",
        "We summarize cells with at least 25 reference hits (1,624 of 2,000).",
        "With fewer hits, the reference has at least about 20% relative MC",
        "standard error; a zero-hit reference is not proof of impossibility.",
        "All cells remain in the CSV. Errors are probabilities, not percentages.",
        "",
        "## Across all positions",
        "",
        "| Group | Cells ≥25 hits | Team-only MAE | Actual pooled MAE | Frozen gap MAE | Actual pooled beats gap |",
        "| --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for group in sorted({row["group"] for row in rows}):
        subset = [row for row in eligible if row["group"] == group]
        wins = sum(row["observed_absolute_error"] < row["gap_absolute_error"] for row in subset)
        lines.append(
            f"| {group} | {len(subset)} | {fmt(mean_error(subset, 'team_only'))} | "
            f"{fmt(mean_error(subset, 'observed'))} | {fmt(mean_error(subset, 'gap'))} | "
            f"{wins}/{len(subset)} |"
        )
    lines += [
        "",
        "## By reference probability",
        "",
        "The rare band is still heterogeneous: a pooled proxy can help a",
        "specific unseen outcome while biasing other cells in the same band.",
        "",
        "| Reference probability | Cells ≥25 hits | Team-only MAE | Actual pooled MAE | Frozen gap MAE |",
        "| --- | ---: | ---: | ---: | ---: |",
    ]
    bands = (
        ("<0.001", lambda p: p < 0.001),
        ("0.001–<0.1", lambda p: 0.001 <= p < 0.1),
        ("≥0.1", lambda p: p >= 0.1),
    )
    for label, select in bands:
        subset = [row for row in eligible if select(row["reference_probability"])]
        lines.append(
            f"| {label} | {len(subset)} | {fmt(mean_error(subset, 'team_only'))} | "
            f"{fmt(mean_error(subset, 'observed'))} | {fmt(mean_error(subset, 'gap'))} |"
        )
    unseen = [
        row for row in eligible
        if row["reference_probability"] < 0.001 and row["team_only_zero_seeds"] == 20
    ]
    lines += [
        "",
        "For the narrower case that motivated pooling—reference probability",
        "below 0.001, at least 25 reference hits, and **zero team-only estimates",
        "in all 20 scouts**—the errors are:",
        "",
        "| Cells | Team-only MAE (reported zero) | Actual pooled MAE | Frozen gap MAE |",
        "| ---: | ---: | ---: | ---: |",
        f"| {len(unseen)} | {fmt(mean_error(unseen, 'team_only'))} | "
        f"{fmt(mean_error(unseen, 'observed'))} | {fmt(mean_error(unseen, 'gap'))} |",
    ]
    lines += [
        "",
        "## Every group and position",
        "",
        "Each row averages across the teams whose reference cell has at least",
        "25 hits. A position may therefore have fewer than 20 included teams.",
        "The full per-team reference, 20-seed means, seed SDs, absolute errors,",
        "and zero-seed counts are in the CSV. Positions here are one-based.",
    ]
    for group in sorted({row["group"] for row in rows}):
        lines += [
            "",
            f"### Group {group}",
            "",
            "| Position | Teams ≥25 hits | Team-only MAE | Actual pooled MAE | Frozen gap MAE |",
            "| ---: | ---: | ---: | ---: | ---: |",
        ]
        for position in range(1, 21):
            subset = [row for row in eligible if row["group"] == group and row["position"] == position]
            if subset:
                lines.append(
                    f"| {position} | {len(subset)} | {fmt(mean_error(subset, 'team_only'))} | "
                    f"{fmt(mean_error(subset, 'observed'))} | {fmt(mean_error(subset, 'gap'))} |"
                )
            else:
                lines.append(f"| {position} | 0 | — | — | — |")
    lines += [
        "",
        "## Interpretation",
        "",
        "The actual pooled curve improves on the frozen gap curve in most",
        "groups, but neither is accurate enough to replace team-specific",
        "estimates across the matrix. Pooling actual finishes changes the",
        "conditioning from a particular team's points to any team's points.",
        "For common positions that team-identity mismatch causes sizable bias.",
        "The gap curve adds counterfactual results that do not update opponents'",
        "points in the target team's fixtures. These comparisons are diagnostics,",
        "not calibrated uncertainty intervals or official probability outputs.",
        "",
        "For the previously discussed cell (16653, team 125, 16th), the",
        "reference is 0.0000642 (321 hits); team-only is zero in all 20 scouts,",
        "actual pooled is 0.0000771, and frozen gap is 0.0003661. The broader",
        "comparison shows that this useful pooled result is not representative",
        "of every team-position cell.",
        "",
        "Reproduce from the saved local runs with",
        "`python3 experiments/rare_positions/compare_point_gap_all_positions.py`.",
        "",
    ]
    MD_OUT.write_text("\n".join(lines))


if __name__ == "__main__":
    cells = summarize_cells(read_runs())
    write_csv(cells)
    write_markdown(cells)
    print(f"Wrote {len(cells)} cells to {CSV_OUT.name} and {MD_OUT.name}")
