"""Aggregate completed paired and warm experiment summaries without losing schema detail."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import statistics


EXPERIMENT_DIRS = (
    "portfolio",
    "pair",
    "pair-cached",
    "marginal",
    "pair-bounded",
    "controls",
    "controls-v3",
    "warm-pair-bounded",
    "warm-portfolio",
    "warm-pair-cached",
    "warm-marginal",
)
TEST_RESULT = re.compile(
    r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; "
    r"(\d+) ignored; (\d+) measured; (\d+) filtered out"
)


def read_json(path: Path):
    return json.loads(path.read_text())


def source_hashes(summary: dict) -> dict:
    return {
        key: summary[key]
        for key in ("baseline_sha256", "candidate_sha256", "inputs")
        if key in summary
    }


def run_settings(summary: dict) -> dict:
    return {
        key: summary[key]
        for key in ("workers", "baseline_flags", "flags")
        if key in summary
    }


def paired_summary(directory: Path, summary: dict) -> dict:
    analysis_path = directory / "analysis.json"
    if not analysis_path.is_file():
        raise RuntimeError(f"paired experiment has no generated analysis: {analysis_path}")
    analysis = read_json(analysis_path)
    required = ("pairs", "gains", "losses", "proof_changes", "groups")
    missing = [key for key in required if key not in analysis]
    if missing:
        raise RuntimeError(f"{analysis_path} is missing fields: {', '.join(missing)}")

    result = {
        "kind": "paired",
        "pairs": analysis["pairs"],
        "gains": analysis["gains"],
        "losses": analysis["losses"],
        "reachability_classification_invariant": len(analysis["proof_changes"]) == 0,
        "reachability_changes": analysis["proof_changes"],
        "full_request_costs_by_input": analysis["groups"],
        "hashes": source_hashes(summary),
        "run_settings": run_settings(summary),
    }
    if "initial_mc_identical" in analysis:
        result["initial_mc_identical"] = analysis["initial_mc_identical"]
    if "initial_mc_mismatches" in analysis:
        result["initial_mc_mismatches"] = analysis["initial_mc_mismatches"]
    if "initial_mc_samples_100k" in analysis:
        result["initial_mc_samples_100k"] = analysis["initial_mc_samples_100k"]
    if "initial_mc" in analysis:
        result["initial_mc_counts"] = analysis["initial_mc"]
    if "importance_identical" in analysis:
        result["game_importance_identical"] = analysis["importance_identical"]
    if "identical_exports" in analysis:
        result["identical_exports"] = analysis["identical_exports"]
    if "rare_positive" in analysis:
        result["rare_positive"] = analysis["rare_positive"]
    return result


def warm_summary(summary: dict) -> dict:
    pairs = summary["pairs"]
    if pairs and not all("latency_ms" in row and "resources" in row for row in pairs):
        raise RuntimeError("warm summary rows do not match the observed latency/resources schema")
    costs = []
    comparisons = []
    gains = []
    losses = []
    reachability_changes = []
    importance = []
    for row in pairs:
        record = {"input": row["input"]}
        if "median_ms" in row:
            record["full_request_median_ms"] = row["median_ms"]
        if "latency_ms" in row:
            record["full_request_latency_ms"] = row["latency_ms"]
        cpu = {
            arm: resource["cpu_seconds"]
            for arm, resource in row["resources"].items()
            if "cpu_seconds" in resource
        }
        if cpu:
            record["cpu_seconds"] = cpu
        tail = {}
        for arm, resource in row["resources"].items():
            stages = resource.get("stages")
            if stages is not None and "search.rare_tail" in stages:
                samples = stages["search.rare_tail"]
                if samples:
                    tail[arm] = statistics.median(samples)
        if tail:
            record["median_rare_tail_ms"] = tail
        costs.append(record)
        if "comparison" in row:
            comparison = row["comparison"]
            comparisons.append({"input": row["input"], **comparison})
            gains.extend({"input": row["input"], **item} for item in comparison.get("gains", []))
            losses.extend({"input": row["input"], **item} for item in comparison.get("lost", []))
            if "lost_reachability" in comparison:
                reachability_changes.extend(
                    {"input": row["input"], **item}
                    for item in comparison["lost_reachability"]
                )
            if "game_importance_identical" in comparison:
                importance.append(comparison["game_importance_identical"])

    result = {
        "kind": "warm",
        "pairs": len(pairs),
        "costs_by_input": costs,
        "hashes": source_hashes(summary),
        "run_settings": run_settings(summary),
    }
    if comparisons:
        result["comparisons"] = comparisons
        result["gains"] = gains
        result["losses"] = losses
    if any("lost_reachability" in row["comparison"] for row in pairs if "comparison" in row):
        result["reachability_classification_invariant"] = not reachability_changes
        result["reachability_changes"] = reachability_changes
    if importance:
        result["game_importance_identical"] = all(importance)
    return result


def test_counts(log_paths: list[Path]) -> list[dict]:
    counts = []
    for path in log_paths:
        if not path.is_file():
            raise FileNotFoundError(path)
        found = []
        for line in path.read_text(errors="replace").splitlines():
            match = TEST_RESULT.search(line)
            if match:
                found.append(
                    {
                        "status": match.group(1),
                        "passed": int(match.group(2)),
                        "failed": int(match.group(3)),
                        "ignored": int(match.group(4)),
                        "measured": int(match.group(5)),
                        "filtered_out": int(match.group(6)),
                    }
                )
        if found:
            counts.append({"log": str(path), "test_results": found})
    return counts


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("r45_root", type=Path, help="/private/tmp/golaberto-r45")
    parser.add_argument(
        "--output",
        type=Path,
        default=Path(__file__).resolve().parent / "final-summary.json",
    )
    parser.add_argument(
        "--test-log", action="append", type=Path, default=[],
        help="optional cargo test log; may be supplied more than once",
    )
    args = parser.parse_args()

    root = args.r45_root.resolve(strict=True)
    experiments = {}
    for name in EXPERIMENT_DIRS:
        directory = root / name
        summary_path = directory / "summary.json"
        if not summary_path.is_file():
            continue
        summary = read_json(summary_path)
        if "pairs" not in summary:
            raise RuntimeError(f"{summary_path} is missing its pairs array")
        rows = summary["pairs"]
        if rows and "timing" in rows[0]:
            experiments[name] = paired_summary(directory, summary)
        elif rows and "latency_ms" in rows[0] and "resources" in rows[0]:
            experiments[name] = warm_summary(summary)
        else:
            raise RuntimeError(f"unrecognized summary schema in {summary_path}")

    document = {"experiments": experiments}
    invariant_root = Path(__file__).resolve().parent / "invariants"
    if invariant_root.is_dir():
        reports = []
        for path in sorted(invariant_root.rglob("invariants.json")):
            report = read_json(path)
            reports.append(
                {
                    "path": path.relative_to(invariant_root.parent).as_posix(),
                    "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                    "experiment": report["experiment"],
                    "request": report["request"],
                    "all_parsed_exports_equal": report["all_parsed_exports_equal"],
                    "run_count": len(report["runs"]),
                }
            )
        if reports:
            document["invariance_reports"] = reports
    counts = test_counts([path.resolve(strict=True) for path in args.test_log])
    if counts:
        document["test_counts"] = counts
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(document, indent=2) + "\n")
    print(
        json.dumps(
            {
                "output": str(output),
                "experiments": sorted(experiments),
                "test_log_count": len(counts),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
