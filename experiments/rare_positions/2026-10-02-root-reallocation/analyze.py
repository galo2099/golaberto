#!/usr/bin/env python3
"""Summarize paired propagated-joint exports and all custom arm log events."""

import argparse
import hashlib
import json
import statistics
from collections import Counter
from pathlib import Path


def digest_json(value: object) -> str:
    canonical = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    return hashlib.sha256(canonical).hexdigest()


def initial_mc(estimate: dict) -> dict:
    return {key: estimate.get(key) for key in ("samples", "hits")}


def iter_estimates(export: dict):
    for team, ranks in (export.get("rare_position_estimates") or {}).items():
        for rank, estimate in ranks.items():
            yield str(team), str(rank), estimate


def diagnostic_paths(value: object, prefix: str = "") -> dict[str, object]:
    found = {}
    if isinstance(value, dict):
        for key, child in value.items():
            path = f"{prefix}.{key}" if prefix else str(key)
            lowered = str(key).casefold()
            if any(token in lowered for token in ("dag", "state", "reconstruct", "late", "split")):
                found[path] = child
            found.update(diagnostic_paths(child, path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            found.update(diagnostic_paths(child, f"{prefix}[{index}]"))
    return found


def load_exports(root: Path, row: dict) -> dict[str, dict]:
    exports = {}
    for arm in ("baseline", "candidate"):
        path = root / f"{Path(row['input']).stem}-{row['seed']}-{arm}.json"
        exports[arm] = json.loads(path.read_text())
    return exports


def analyze_root(root: Path) -> dict:
    summary_path = root / "summary.json"
    summary = json.loads(summary_path.read_text())
    result = {
        "source_directory": str(root),
        "pairs": len(summary.get("pairs", [])),
        "flags": summary.get("flags", {}),
        "baseline_flags": summary.get("baseline_flags", {}),
        "binary_sha256": {
            arm: summary.get(f"{arm}_sha256")
            for arm in ("baseline", "candidate")
        },
        "gains": [],
        "losses": [],
        "groups": {},
        "cell_runs": {"baseline": 0, "candidate": 0},
        "distinct_cells": {"baseline": set(), "candidate": set()},
        "rare_cell_runs": {"baseline": 0, "candidate": 0},
        "rare_distinct_cells": {"baseline": set(), "candidate": set()},
        "distinct_gain_identities": set(),
        "distinct_loss_identities": set(),
        "newly_covered_rare_identities": [],
        "lost_rare_identities": [],
        "_positive_support": {"baseline": {}, "candidate": {}},
        "initial_mc_identical": True,
        "initial_mc_mismatches": 0,
        "initial_mc_samples_100k": True,
        "initial_mc": {
            arm: {"cells": 0, "total_samples": 0, "total_hits": 0}
            for arm in ("baseline", "candidate")
        },
        "initial_mc_hashes_identical": True,
        "game_importance_identical": True,
        "identical_exports": 0,
        "rare_positive_initial_mc_zero": {"baseline": 0, "candidate": 0},
        "reachability": {
            arm: {"by_status": {}, "reachable_zero": 0, "undecided_zero": 0, "undecided_total": 0}
            for arm in ("baseline", "candidate")
        },
        "proof_changes": [],
        "custom_logs": [],
        "custom_log_events": {},
        "joint_state_diagnostics": [],
        "late_split_statistics": [],
    }

    for row in summary.get("pairs", []):
        name, seed = row["input"], row["seed"]
        exports = load_exports(root, row)
        group = result["groups"].setdefault(name, {
            "cell_runs": {"baseline": 0, "candidate": 0},
            "distinct_cells": {"baseline": 0, "candidate": 0},
            "rare_cell_runs": {"baseline": 0, "candidate": 0},
            "rare_distinct_cells": {"baseline": 0, "candidate": 0},
            "gains": 0,
            "losses": 0,
            "distinct_gain_identities": 0,
            "distinct_loss_identities": 0,
            "newly_covered_rare_identities": 0,
            "lost_rare_identities": 0,
            "baseline_cpu_ms": 0.0,
            "candidate_cpu_ms": 0.0,
            "http_ms": {"baseline": [], "candidate": []},
            "rare_tail_ms": {"baseline": [], "candidate": []},
            "_distinct_cell_ids": {"baseline": set(), "candidate": set()},
            "_rare_cell_ids": {"baseline": set(), "candidate": set()},
            "_gain_ids": set(),
            "_loss_ids": set(),
            "_positive_support": {"baseline": set(), "candidate": set()},
        })

        for arm, export in exports.items():
            estimates = list(iter_estimates(export))
            result["cell_runs"][arm] += len(estimates)
            group["cell_runs"][arm] += len(estimates)
            for team, rank, estimate in estimates:
                cell = (name, team, rank)
                result["distinct_cells"][arm].add(cell)
                group["_distinct_cell_ids"][arm].add((team, rank))
                is_initial_mc_rare = (
                    estimate.get("samples") == 100_000
                    and estimate.get("hits") == 0
                    and estimate.get("probability", 0.0) > 0.0
                )
                if is_initial_mc_rare:
                    result["rare_cell_runs"][arm] += 1
                    result["rare_distinct_cells"][arm].add(cell)
                    group["rare_cell_runs"][arm] += 1
                    group["_rare_cell_ids"][arm].add((team, rank))
                    group["_positive_support"][arm].add((team, rank))
                    result["_positive_support"][arm][(name, team, rank)] = True
                stats = result["initial_mc"][arm]
                stats["cells"] += 1
                samples, hits = estimate.get("samples", 0), estimate.get("hits", 0)
                stats["total_samples"] += samples or 0
                stats["total_hits"] += hits or 0
                result["initial_mc_samples_100k"] &= samples == 100_000
                result["rare_positive_initial_mc_zero"][arm] += int(
                    samples == 100_000 and hits == 0 and estimate.get("probability", 0.0) > 0.0
                )
                status = estimate.get("reachability", "missing")
                arm_reach = result["reachability"][arm]
                arm_reach["by_status"][status] = arm_reach["by_status"].get(status, 0) + 1
                probability = estimate.get("probability", 0.0)
                if probability == 0.0 and status == "reachable":
                    arm_reach["reachable_zero"] += 1
                if probability == 0.0 and status == "undecided":
                    arm_reach["undecided_zero"] += 1
                if status == "undecided":
                    arm_reach["undecided_total"] += 1

        comparison = row.get("comparison") or {}
        for field, destination in (("gains", "gains"), ("lost", "losses")):
            for change in comparison.get(field, []):
                item = {"input": name, "seed": seed, **change}
                identity = (name, str(change["team"]), str(change["position"] - 1))
                if field == "lost":
                    item["baseline_probability"] = exports["baseline"]["rare_position_estimates"][
                        str(change["team"])
                    ][str(change["position"] - 1)]["probability"]
                    result["distinct_loss_identities"].add(identity)
                    group["_loss_ids"].add(identity)
                else:
                    result["distinct_gain_identities"].add(identity)
                    group["_gain_ids"].add(identity)
                result[destination].append(item)
                group["gains" if field == "gains" else "losses"] += 1

        baseline, candidate = exports["baseline"], exports["candidate"]
        result["identical_exports"] += int(baseline == candidate)
        result["game_importance_identical"] &= (
            baseline.get("game_importance") == candidate.get("game_importance")
        )
        for team, ranks in baseline.get("rare_position_estimates", {}).items():
            for rank, before in ranks.items():
                after = candidate.get("rare_position_estimates", {}).get(team, {}).get(rank, {})
                before_mc, after_mc = initial_mc(before), initial_mc(after)
                same = before_mc == after_mc
                result["initial_mc_identical"] &= same
                result["initial_mc_mismatches"] += int(not same)
                result["initial_mc_hashes_identical"] &= digest_json(before_mc) == digest_json(after_mc)
                if before.get("reachability") != after.get("reachability"):
                    result["proof_changes"].append({
                        "input": name, "seed": seed, "team": team, "rank": rank,
                        "before": before.get("reachability"), "after": after.get("reachability"),
                        "baseline": before, "candidate": after,
                    })

        for arm, timing in (row.get("timing") or {}).items():
            group[f"{arm}_cpu_ms"] += (timing or {}).get("cpu_ms", 0.0)
            group["http_ms"][arm].append((timing or {}).get("http_ms", 0.0))
            tail = ((timing or {}).get("stages") or {}).get("search.rare_tail")
            if tail is not None:
                group["rare_tail_ms"][arm].append(tail)

    for identity in sorted(result["_positive_support"]["candidate"]):
        if identity not in result["_positive_support"]["baseline"]:
            result["newly_covered_rare_identities"].append({
                "input": identity[0], "team": identity[1], "rank": identity[2],
            })
    for identity in sorted(result["_positive_support"]["baseline"]):
        if identity not in result["_positive_support"]["candidate"]:
            result["lost_rare_identities"].append({
                "input": identity[0], "team": identity[1], "rank": identity[2],
            })
    result["distinct_gain_identities"] = [
        {"input": input_name, "team": team, "rank": rank}
        for input_name, team, rank in sorted(result["distinct_gain_identities"])
    ]
    result["distinct_loss_identities"] = [
        {"input": input_name, "team": team, "rank": rank}
        for input_name, team, rank in sorted(result["distinct_loss_identities"])
    ]
    result["distinct_gain_identity_count"] = len(result["distinct_gain_identities"])
    result["distinct_loss_identity_count"] = len(result["distinct_loss_identities"])
    result["newly_covered_rare_identity_count"] = len(result.get("newly_covered_rare_identities", []))
    result["lost_rare_identity_count"] = len(result.get("lost_rare_identities", []))
    for arm in ("baseline", "candidate"):
        result["distinct_cells"][arm] = len(result["distinct_cells"][arm])
        result["rare_distinct_cells"][arm] = len(result["rare_distinct_cells"][arm])
    result["all_cell_runs"] = result["cell_runs"].copy()
    result["all_distinct_cells"] = result["distinct_cells"].copy()
    result.pop("_positive_support")
    result["cell_runs_per_distinct_cell"] = {
        arm: result["cell_runs"][arm] / result["distinct_cells"][arm]
        if result["distinct_cells"][arm]
        else None
        for arm in ("baseline", "candidate")
    }
    result["rare_cell_runs_per_rare_distinct_cell"] = {
        arm: result["rare_cell_runs"][arm] / result["rare_distinct_cells"][arm]
        if result["rare_distinct_cells"][arm]
        else None
        for arm in ("baseline", "candidate")
    }
    for name, group in result["groups"].items():
        baseline_support = group.pop("_positive_support")
        group["newly_covered_rare_identities"] = [
            {"team": team, "rank": rank}
            for team, rank in sorted(baseline_support["candidate"] - baseline_support["baseline"])
        ]
        group["lost_rare_identities"] = [
            {"team": team, "rank": rank}
            for team, rank in sorted(baseline_support["baseline"] - baseline_support["candidate"])
        ]
        group["newly_covered_rare_identity_count"] = len(group["newly_covered_rare_identities"])
        group["lost_rare_identity_count"] = len(group["lost_rare_identities"])
        group["distinct_cells"] = {
            arm: len(cell_ids) for arm, cell_ids in group.pop("_distinct_cell_ids").items()
        }
        group["rare_distinct_cells"] = {
            arm: len(cell_ids) for arm, cell_ids in group.pop("_rare_cell_ids").items()
        }
        group["distinct_gain_identities"] = len(group.pop("_gain_ids"))
        group["distinct_loss_identities"] = len(group.pop("_loss_ids"))
        group["cell_runs_per_distinct_cell"] = {
            arm: group["cell_runs"][arm] / group["distinct_cells"][arm]
            if group["distinct_cells"][arm]
            else None
            for arm in ("baseline", "candidate")
        }
        group["all_cell_runs"] = group["cell_runs"].copy()
        group["all_distinct_cells"] = group["distinct_cells"].copy()
        group["rare_cell_runs_per_rare_distinct_cell"] = {
            arm: group["rare_cell_runs"][arm] / group["rare_distinct_cells"][arm]
            if group["rare_distinct_cells"][arm]
            else None
            for arm in ("baseline", "candidate")
        }
        group["median_http_ms"] = {
            arm: statistics.median(values) if values else None
            for arm, values in group.pop("http_ms").items()
        }
        group["median_rare_tail_ms"] = {
            arm: statistics.median(values) if values else None
            for arm, values in group.pop("rare_tail_ms").items()
        }
        before, after = group["baseline_cpu_ms"], group["candidate_cpu_ms"]
        group["cpu_change_pct"] = 100 * (after / before - 1) if before else None
        b, c = group["median_http_ms"]["baseline"], group["median_http_ms"]["candidate"]
        group["http_change_pct"] = 100 * (c / b - 1) if b else None

    for log_file in sorted(root.glob("*.log")):
        arm = "candidate" if "candidate" in log_file.name else "baseline" if "baseline" in log_file.name else "unknown"
        for line_number, line in enumerate(log_file.read_text(errors="replace").splitlines(), 1):
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            event_name = event.get("event", "")
            if not isinstance(event_name, str) or not event_name.startswith("rust_odds_"):
                continue
            if event_name in ("rust_odds_stage", "rust_odds_complete"):
                continue
            record = {"file": log_file.name, "line": line_number, "arm": arm, "event": event}
            result["custom_logs"].append(record)
            result["custom_log_events"][event_name] = result["custom_log_events"].get(event_name, 0) + 1
            fields = diagnostic_paths(event)
            if any(
                token in key.casefold()
                for key in fields
                for token in ("dag", "state", "reconstruct")
            ):
                result["joint_state_diagnostics"].append({**record, "fields": fields})
            if "late" in event_name.casefold() or "split" in event_name.casefold() or any(
                token in key.casefold() for key in fields for token in ("late", "split")
            ):
                result["late_split_statistics"].append({**record, "fields": fields})

    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path, help="benchmark_propagated_joint.py output directory")
    parser.add_argument("--output", type=Path, help="write path; defaults to ROOT/analysis.json")
    args = parser.parse_args()
    root = args.root.resolve(strict=True)
    result = analyze_root(root)
    output = args.output or root / "analysis.json"
    output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({key: value for key, value in result.items() if key not in ("custom_logs", "groups", "proof_changes", "joint_state_diagnostics", "late_split_statistics")}, indent=2))
    print(f"analysis: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
