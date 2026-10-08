#!/usr/bin/env python3
"""Run serialized Rust odds benchmarks and check exact parity or cell coverage."""

import argparse
import hashlib
import json
import math
import os
import resource
import statistics
import subprocess
import sys
import time
from pathlib import Path


STAGES = ("setup_ms", "scout_ms", "pool_ms", "search_ms", "total_ms")


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def resource_snapshot():
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    return usage.ru_utime, usage.ru_stime


def resource_delta(before, after):
    return {"user_seconds": after[0] - before[0], "system_seconds": after[1] - before[1]}


def parse_timings(stderr):
    for line in reversed(stderr.splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and all(key in value for key in STAGES):
            stages = {key: value[key] for key in STAGES}
            return stages if all(valid_number(number) and number >= 0 for number in stages.values()) else None
    return None


def valid_number(value):
    return not isinstance(value, bool) and isinstance(value, (int, float)) and math.isfinite(value)


def output_counts(path):
    try:
        data = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return {"valid_json": False}
    if not isinstance(data, dict):
        return {"valid_json": True, "json_type": type(data).__name__}
    teams = data.get("team_odds", {})
    rare = data.get("rare_position_estimates", {})
    estimates = [estimate for entries in rare.values() if isinstance(entries, dict)
                 for estimate in entries.values() if isinstance(estimate, dict)] if isinstance(rare, dict) else []
    return {
        "valid_json": True,
        "top_level_keys": sorted(data),
        "teams": len(teams) if isinstance(teams, dict) else None,
        "game_importance_entries": len(data.get("game_importance", {})) if isinstance(data.get("game_importance", {}), dict) else None,
        "rare_position_teams": len(rare) if isinstance(rare, dict) else None,
        "rare_position_estimates": len(estimates),
        "positive_probability_estimates": sum(item.get("probability", 0) > 0 for item in estimates),
        "zero_probability_estimates": sum(item.get("probability", 0) == 0 for item in estimates),
        "impossible_estimates": sum(str(item.get("reachability", "")).startswith("impossible") for item in estimates),
        "reachable_zero_estimates": sum(item.get("probability", 0) == 0 and not str(item.get("reachability", "")).startswith("impossible") for item in estimates),
    }


def load_export(path):
    data = json.loads(path.read_text())
    if not isinstance(data, dict):
        raise ValueError("export must be a JSON object")
    errors = []
    expected_keys = {"team_odds", "game_importance", "rare_position_estimates"}
    if set(data) != expected_keys:
        errors.append({"kind": "top_level_keys", "expected": sorted(expected_keys), "actual": sorted(data)})
    cells = {}
    rare = data.get("rare_position_estimates")
    if not isinstance(rare, dict):
        errors.append({"kind": "rare_position_estimates", "error": "expected object"})
        rare = {}
    for team, ranks in rare.items():
        if not isinstance(ranks, dict):
            errors.append({"kind": "rare_position_team", "team": team, "error": "expected rank object"})
            continue
        for rank, estimate in ranks.items():
            if not isinstance(estimate, dict):
                errors.append({"kind": "estimate_shape", "cell": f"{team}:{rank}", "error": "expected object"})
            else:
                cells[f"{team}:{rank}"] = estimate

    teams = data.get("team_odds")
    if not isinstance(teams, dict):
        errors.append({"kind": "team_odds", "error": "expected object"})
        teams = {}
    for team, odds in teams.items():
        if not isinstance(odds, dict) or not isinstance(odds.get("Pos"), list):
            errors.append({"kind": "team_odds_shape", "team": team, "error": "expected Pos array"})
            continue
        for rank, value in enumerate(odds["Pos"]):
            if not valid_number(value) or not 0 <= value <= 100:
                errors.append({"kind": "team_odds_value", "team": team, "rank": rank, "value": repr(value)})

    importance = data.get("game_importance")
    if not isinstance(importance, dict):
        errors.append({"kind": "game_importance", "error": "expected object"})
        importance = {}
    for game, values in importance.items():
        if not isinstance(values, (list, tuple)) or len(values) != 2:
            errors.append({"kind": "game_importance_shape", "game": game, "error": "expected two-element array"})
            continue
        for index, value in enumerate(values):
            if value is not None and not valid_number(value):
                errors.append({"kind": "game_importance_value", "game": game, "index": index, "value": repr(value)})
    if isinstance(rare, dict) and isinstance(teams, dict):
        if set(rare) != set(teams):
            errors.append({"kind": "team_identity_mismatch", "team_odds": sorted(teams),
                           "rare_position_estimates": sorted(rare)})
        for team in teams.keys() & rare.keys():
            odds = teams[team]
            ranks = rare[team]
            if isinstance(odds, dict) and isinstance(odds.get("Pos"), list) and isinstance(ranks, dict):
                if len(odds["Pos"]) != len(ranks):
                    errors.append({"kind": "team_rank_length_mismatch", "team": team,
                                   "team_odds": len(odds["Pos"]), "rare_position_estimates": len(ranks)})
    return data, cells, errors


def coverage_comparison(baseline_path, candidate_path):
    baseline_data, baseline, baseline_shape_errors = load_export(baseline_path)
    candidate_data, candidate, candidate_shape_errors = load_export(candidate_path)
    shape_errors = {"baseline": baseline_shape_errors, "candidate": candidate_shape_errors}
    for field, key in (("team_odds", "team"), ("game_importance", "game")):
        base_ids = set(baseline_data.get(field, {})) if isinstance(baseline_data.get(field), dict) else set()
        candidate_ids = set(candidate_data.get(field, {})) if isinstance(candidate_data.get(field), dict) else set()
        if base_ids != candidate_ids:
            shape_errors.setdefault("identity_mismatches", []).append({
                "field": field, "missing": sorted(base_ids - candidate_ids),
                "unexpected": sorted(candidate_ids - base_ids),
            })
    for field in ("team_odds", "game_importance"):
        base_values = baseline_data.get(field, {})
        candidate_values = candidate_data.get(field, {})
        if isinstance(base_values, dict) and isinstance(candidate_values, dict):
            for identity in sorted(base_values.keys() & candidate_values.keys()):
                if field == "team_odds":
                    base_pos = base_values[identity].get("Pos") if isinstance(base_values[identity], dict) else None
                    candidate_pos = candidate_values[identity].get("Pos") if isinstance(candidate_values[identity], dict) else None
                    if isinstance(base_pos, list) and isinstance(candidate_pos, list) and len(base_pos) != len(candidate_pos):
                        shape_errors.setdefault("position_length_mismatches", []).append({
                            "team": identity, "baseline": len(base_pos), "candidate": len(candidate_pos)})
    positive = lambda cell: valid_number(cell.get("probability")) and cell["probability"] > 0
    baseline_positive = {key for key, cell in baseline.items() if positive(cell)}
    candidate_positive = {key for key, cell in candidate.items() if positive(cell)}
    baseline_impossible = {key for key, cell in baseline.items()
                           if str(cell.get("reachability", "")).startswith("impossible")}
    lost = sorted(baseline_positive - candidate_positive)
    added = sorted(candidate_positive - baseline_positive)

    def invalid_estimates(cells):
        invalid = []
        for key, cell in cells.items():
            for field in ("probability", "std_err", "ess", "mean_weight",
                          "max_event_weight_share", "zero_hit_upper_95"):
                value = cell.get(field)
                if not valid_number(value):
                    invalid.append({"cell": key, "field": field, "value": repr(value)})
                    continue
                if value < 0 or (field == "probability" and value > 1):
                    invalid.append({"cell": key, "field": field, "value": value})
            for field in ("conditional_mass", "relative_se"):
                if field not in cell or (field == "relative_se" and cell[field] is None):
                    continue
                value = cell[field]
                if not valid_number(value) or value < 0:
                    invalid.append({"cell": key, "field": field, "value": repr(value)})
            for field in ("samples", "hits"):
                if field not in cell:
                    invalid.append({"cell": key, "field": field, "value": "<missing>"})
                    continue
                value = cell[field]
                if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                    invalid.append({"cell": key, "field": field, "value": repr(value)})
            for field in ("work_spent", "conditional_samples", "conditional_hits"):
                if field not in cell:
                    continue
                value = cell[field]
                if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                    invalid.append({"cell": key, "field": field, "value": repr(value)})
            if isinstance(cell.get("samples"), int) and isinstance(cell.get("hits"), int) and cell["hits"] > cell["samples"]:
                invalid.append({"cell": key, "field": "hits", "error": "hits exceeds samples"})
            if isinstance(cell.get("conditional_samples"), int) and isinstance(cell.get("conditional_hits"), int) and cell["conditional_hits"] > cell["conditional_samples"]:
                invalid.append({"cell": key, "field": "conditional_hits", "error": "conditional_hits exceeds conditional_samples"})
        return invalid

    invalid = invalid_estimates(candidate)
    invalid_baseline = invalid_estimates(baseline)
    missing_cells = sorted(baseline.keys() - candidate.keys())
    unexpected_cells = sorted(candidate.keys() - baseline.keys())

    reachability_differences = []
    for key in sorted(baseline.keys() | candidate.keys()):
        base_status = baseline.get(key, {}).get("reachability")
        cand_status = candidate.get(key, {}).get("reachability")
        if base_status != cand_status:
            reachability_differences.append({"cell": key, "baseline": base_status, "candidate": cand_status})
    contradictions = sorted(key for key in baseline_impossible & candidate_positive)

    ratios = []
    log_ratios = {}
    independent_precision_mismatches = []
    lost_precision_goal = [
        key for key in sorted(baseline.keys() & candidate.keys())
        if baseline[key].get("meets_precision_goal") is True
        and candidate[key].get("meets_precision_goal") is not True
    ]
    for key in sorted(baseline_positive & candidate_positive):
        base_probability = baseline[key]["probability"]
        candidate_probability = candidate[key]["probability"]
        if not valid_number(base_probability) or not valid_number(candidate_probability) or base_probability <= 0 or candidate_probability <= 0:
            continue
        log_ratio = math.log(candidate_probability) - math.log(base_probability)
        log_ratios[key] = log_ratio
        ratio_value = candidate_probability / base_probability
        ratio = ratio_value if math.isfinite(ratio_value) else None
        ratios.append({"cell": key, "baseline_probability": base_probability,
                       "candidate_probability": candidate_probability, "candidate_over_baseline": ratio,
                       "log_candidate_over_baseline": log_ratio})
        base, cand = baseline[key], candidate[key]
        qualified_reference = (
            base.get("meets_precision_goal") is True
            and valid_number(base.get("relative_se")) and base["relative_se"] <= 0.1
            and valid_number(base.get("max_event_weight_share")) and base["max_event_weight_share"] <= 0.03
            and valid_number(base.get("ess")) and base["ess"] >= 200
        )
        # Candidate ESS and goal flags do not qualify an independent reference:
        # a prototype may report uncertainty from only a small filter ensemble.
        if qualified_reference:
            combined_std_err = math.hypot(base["std_err"], cand["std_err"])
            difference = abs(candidate_probability - base_probability)
            if (difference > 6 * combined_std_err
                    and not math.log(0.8) <= log_ratio <= math.log(1.25)):
                independent_precision_mismatches.append({
                    "cell": key,
                    "baseline_probability": base_probability,
                    "candidate_probability": candidate_probability,
                    "candidate_over_baseline": ratio,
                    "log_candidate_over_baseline": log_ratio,
                    "absolute_difference": difference,
                    "combined_std_err": combined_std_err,
                    "six_combined_std_err": 6 * combined_std_err,
                    "baseline_relative_se": base["relative_se"],
                    "candidate_relative_se": cand["relative_se"],
                    "baseline_ess": base["ess"], "candidate_ess": cand["ess"],
                    "baseline_max_event_weight_share": base["max_event_weight_share"],
                    "candidate_max_event_weight_share": cand["max_event_weight_share"],
                })
    large_ratios = [item for item in ratios if log_ratios[item["cell"]] > math.log(100)]
    small_ratios = [item for item in ratios if log_ratios[item["cell"]] < math.log(0.01)]
    rare_baseline = {key for key in baseline_positive if baseline[key]["probability"] < 1e-8}
    rare_candidate = {key for key in candidate_positive if candidate[key]["probability"] < 1e-8}
    rare_ratios = [item for item in ratios if item["baseline_probability"] < 1e-8
                   or item["candidate_probability"] < 1e-8]
    return {
        "baseline_cells": len(baseline), "candidate_cells": len(candidate),
        "missing_cell_ids": missing_cells, "unexpected_cell_ids": unexpected_cells,
        "baseline_positive_cells": len(baseline_positive), "candidate_positive_cells": len(candidate_positive),
        "lost_positive_cells": lost, "added_positive_cells": added,
        "baseline_rare_positive_below_1e-8": sorted(rare_baseline),
        "candidate_rare_positive_below_1e-8": sorted(rare_candidate),
        "baseline_rare_positive_count_below_1e-8": len(rare_baseline),
        "candidate_rare_positive_count_below_1e-8": len(rare_candidate),
        "baseline_impossible_contradicted_as_positive": contradictions,
        "reachability_differences": reachability_differences,
        "lost_precision_goal": lost_precision_goal,
        "independent_precision_mismatches": independent_precision_mismatches,
        "invalid_candidate_estimates": invalid,
        "invalid_baseline_estimates": invalid_baseline,
        "response_shape_errors": shape_errors,
        "positive_probability_ratios": ratios, "rare_positive_ratios": rare_ratios,
        "ratios_over_100x": large_ratios, "ratios_under_0.01x": small_ratios,
        "precision_verified": False,
        "passed": not lost and not contradictions and not invalid and not invalid_baseline
                  and not missing_cells and not unexpected_cells
                  and not any(shape_errors.values()) and not independent_precision_mismatches,
    }


def execute(binary, request, output, stdout_path, stderr_path, seed, workers, env, cwd):
    command = [str(binary), "estimate", str(request), str(output), str(seed), str(workers)]
    before = resource_snapshot()
    start = time.perf_counter()
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        completed = subprocess.run(command, cwd=cwd, env=env, stdout=stdout, stderr=stderr)
    wall = time.perf_counter() - start
    cpu = resource_delta(before, resource_snapshot())
    stderr_text = stderr_path.read_text(errors="replace")
    return {
        "command": command,
        "returncode": completed.returncode,
        "wall_seconds": wall,
        "child_cpu": cpu,
        "timings_ms": parse_timings(stderr_text),
        "export_sha256": sha256(output) if output.is_file() else None,
    }


def summarize(values):
    if not values:
        return {"median": None, "samples": []}
    return {"median": statistics.median(values), "samples": values}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--request", type=Path, action="append", required=True,
                        help="request JSON; repeat to cover multiple fixture groups")
    parser.add_argument("--seeds", default="808,1669,2293")
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--include-one-worker", action="store_true",
                        help="also benchmark both binaries with one worker")
    parser.add_argument("--comparison", choices=("exact", "coverage"), default="exact",
                        help="compare baseline/candidate bytes exactly or compare cell coverage")
    parser.add_argument("--candidate-env", action="append", default=[], metavar="NAME=VALUE",
                        help="set a candidate-only RUST_ODDS_ or RARE_POSITION_ variable; repeatable")
    parser.add_argument("--out", type=Path, required=True,
                        help="new output directory; it must not already exist")
    args = parser.parse_args()

    if not 1 <= args.workers <= 4:
        parser.error("--workers must be between 1 and 4")
    if args.repeats < 1:
        parser.error("--repeats must be positive")
    try:
        seeds = [int(value.strip()) for value in args.seeds.split(",")]
    except ValueError:
        parser.error("--seeds must be a comma-separated list of integers")
    if not seeds or len(set(seeds)) != len(seeds):
        parser.error("--seeds must contain unique integers")
    if any(seed < -(2**63) or seed >= 2**63 for seed in seeds):
        parser.error("seeds must fit in a signed 64-bit integer")
    candidate_overrides = {}
    for assignment in args.candidate_env:
        name, separator, value = assignment.partition("=")
        if not separator or not (name.startswith("RUST_ODDS_") or name.startswith("RARE_POSITION_")):
            parser.error(f"candidate environment override must be NAME=VALUE with a RUST_ODDS_ or RARE_POSITION_ name: {assignment}")
        if name == "RUST_ODDS_LOG" and value != "1":
            parser.error("RUST_ODDS_LOG must remain 1 so stage timings are recorded")
        candidate_overrides[name] = value

    baseline = args.baseline.resolve()
    candidate = args.candidate.resolve()
    requests = [path.resolve() for path in args.request]
    for label, path in (("baseline", baseline), ("candidate", candidate),
                        *(("request", path) for path in requests)):
        if not path.is_file():
            parser.error(f"{label} file does not exist: {path}")
    out = args.out.resolve()
    if out.exists():
        parser.error(f"output path already exists: {out}")
    out.mkdir(parents=True)

    env = os.environ.copy()
    for key in list(env):
        if key.startswith("RUST_ODDS_") or key.startswith("RARE_POSITION_"):
            del env[key]
    env["RUST_ODDS_LOG"] = "1"
    arm_envs = {"baseline": env, "candidate": env.copy()}
    arm_envs["candidate"].update(candidate_overrides)
    arms = {"baseline": baseline, "candidate": candidate}
    worker_counts = [args.workers] + ([1] if args.include_one_worker and args.workers != 1 else [])
    report = {
        "config": {
            "baseline": str(baseline), "candidate": str(candidate),
            "binary_sha256": {"baseline": sha256(baseline), "candidate": sha256(candidate)},
            "requests": [{"path": str(path), "sha256": sha256(path)} for path in requests],
            "seeds": seeds, "workers": worker_counts, "repeats": args.repeats,
            "comparison": args.comparison, "candidate_environment_overrides": candidate_overrides,
            "environment": {"RUST_ODDS_LOG": "1", "cleared_prefixes": ["RUST_ODDS_", "RARE_POSITION_"]},
        },
        "runs": [], "parity": [], "coverage": [],
    }
    any_failure = False
    run_index = 0
    for request_index, request in enumerate(requests):
        for seed in seeds:
            for workers in worker_counts:
                # Each arm gets one discarded warmup. Reverse order per pair to reduce order bias.
                order = ("baseline", "candidate") if (request_index + seed + workers) % 2 == 0 else ("candidate", "baseline")
                pair_outputs = {"warmup": {}}
                for arm in order:
                    run_index += 1
                    directory = out / f"group-{request_index:02d}" / f"seed-{seed}" / f"workers-{workers}" / arm
                    directory.mkdir(parents=True, exist_ok=True)
                    warm_output = directory / "warmup.json"
                    warm = execute(arms[arm], request, warm_output, directory / "warmup.stdout",
                                   directory / "warmup.stderr", seed, workers, arm_envs[arm], Path.cwd())
                    warm["arm"] = arm
                    warm["request"] = str(request)
                    warm["seed"] = seed
                    warm["workers"] = workers
                    warm["warmup"] = True
                    report["runs"].append(warm)
                    pair_outputs["warmup"][arm] = warm_output
                    if warm["returncode"] != 0 or not warm_output.is_file() or warm["timings_ms"] is None:
                        any_failure = True
                # Rotate repeat order independently; still execute one child at a time.
                for repeat in range(args.repeats):
                    repeat_order = order if repeat % 2 == 0 else tuple(reversed(order))
                    for arm in repeat_order:
                        run_index += 1
                        directory = out / f"group-{request_index:02d}" / f"seed-{seed}" / f"workers-{workers}" / arm
                        stem = f"run-{repeat + 1:02d}"
                        export = directory / f"{stem}.json"
                        result = execute(arms[arm], request, export, directory / f"{stem}.stdout",
                                         directory / f"{stem}.stderr", seed, workers, arm_envs[arm], Path.cwd())
                        result.update(arm=arm, request=str(request), seed=seed,
                                      workers=workers, repeat=repeat + 1, warmup=False)
                        report["runs"].append(result)
                        pair_outputs.setdefault(repeat + 1, {})[arm] = export
                        if result["returncode"] != 0 or not export.is_file() or result["timings_ms"] is None:
                            any_failure = True
                for repeat, outputs in sorted(pair_outputs.items(), key=lambda item: str(item[0])):
                    if set(outputs) != set(arms):
                        continue
                    left, right = outputs["baseline"], outputs["candidate"]
                    if not left.is_file() or not right.is_file():
                        parity = {"request": str(request), "seed": seed, "workers": workers,
                                  "repeat": repeat, "equal": False, "error": "missing export"}
                        any_failure = True
                        report["parity"].append(parity)
                        continue
                    if args.comparison == "exact":
                        left_hash, right_hash = sha256(left), sha256(right)
                        parity = {"request": str(request), "seed": seed, "workers": workers,
                                  "repeat": repeat, "equal": left_hash == right_hash,
                                  "baseline_sha256": left_hash, "candidate_sha256": right_hash}
                        if left_hash != right_hash:
                            parity["baseline_counts"] = output_counts(left)
                            parity["candidate_counts"] = output_counts(right)
                            any_failure = True
                        report["parity"].append(parity)
                    else:
                        try:
                            comparison = coverage_comparison(left, right)
                        except (OSError, json.JSONDecodeError, ValueError) as error:
                            comparison = {"passed": False, "error": str(error)}
                        comparison.update(request=str(request), seed=seed, workers=workers, repeat=repeat)
                        report["coverage"].append(comparison)
                        any_failure |= not comparison["passed"]

                # Repeats and worker counts must also be deterministic for each binary.
                for arm in arms:
                    reference = pair_outputs["warmup"].get(arm)
                    for repeat in range(1, args.repeats + 1):
                        current = pair_outputs.get(repeat, {}).get(arm)
                        for label, left, right in ((f"{arm}:warmup-vs-run-1", reference, current),):
                            if left is None or right is None or not left.is_file() or not right.is_file():
                                continue
                            equal = sha256(left) == sha256(right)
                            report["parity"].append({"request": str(request), "seed": seed,
                                "workers": workers, "repeat": repeat, "comparison": label, "equal": equal})
                            any_failure |= not equal

    # With fixed input and seed, worker count must not change serialized results.
    if len(worker_counts) > 1:
        indexed = {}
        for run in report["runs"]:
            if run["warmup"]:
                continue
            key = (run["request"], run["seed"], run["arm"], run["repeat"])
            indexed.setdefault(key, {})[run["workers"]] = run
        for key, variants in indexed.items():
            if len(variants) != len(worker_counts):
                continue
            first_workers = worker_counts[0]
            first_dir = out / f"group-{requests.index(Path(key[0])):02d}" / f"seed-{key[1]}" / f"workers-{first_workers}" / key[2] / f"run-{key[3]:02d}.json"
            for workers in worker_counts[1:]:
                other_dir = out / f"group-{requests.index(Path(key[0])):02d}" / f"seed-{key[1]}" / f"workers-{workers}" / key[2] / f"run-{key[3]:02d}.json"
                if first_dir.is_file() and other_dir.is_file():
                    equal = sha256(first_dir) == sha256(other_dir)
                    report["parity"].append({"request": key[0], "seed": key[1], "arm": key[2],
                        "repeat": key[3], "comparison": f"workers:{first_workers}-vs-{workers}", "equal": equal})
                    any_failure |= not equal

    metrics = {}
    for arm in arms:
        selected = [run for run in report["runs"] if run["arm"] == arm and not run["warmup"]]
        metrics[arm] = {
            "wall_seconds": summarize([run["wall_seconds"] for run in selected]),
            "child_cpu_seconds": summarize([run["child_cpu"]["user_seconds"] + run["child_cpu"]["system_seconds"] for run in selected]),
            "total_cpu_seconds": summarize([run["child_cpu"]["user_seconds"] + run["child_cpu"]["system_seconds"] for run in selected]),
            "timings_ms": {stage: summarize([run["timings_ms"][stage] for run in selected if run["timings_ms"] is not None]) for stage in STAGES},
        }
    report["summary"] = metrics
    report["success"] = not any_failure and all(item["equal"] for item in report["parity"])
    summary_path = out / "summary.json"
    summary_path.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"success": report["success"], "runs": len(report["runs"]),
                      "parity_checks": len(report["parity"]), "coverage_checks": len(report["coverage"]),
                      "summary": str(summary_path)}, indent=2))
    return 0 if report["success"] else 1


if __name__ == "__main__":
    sys.exit(main())
