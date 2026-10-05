#!/usr/bin/env python3
"""Validate and summarize generic rare-position follow-up panels."""

import argparse
import hashlib
import json
import math
from pathlib import Path
import sys


FROZEN_BINARY = "1c191c1fe9a7d6a07a652748545443f6cb8a4c61e49781fc9a15292ed27ec4f1"
REQUEST_HASHES = {
    "before": "2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625",
    "after": "cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee",
}
SEEDS = {"screen": [60013, 60017, 60029], "holdout": [60101, 60103, 60107]}
PILOT_SEEDS = {"screen": 60031, "holdout": 60109}
EXPECTED_PANEL_NAMES = {
    "screen": {
        f"screen-{snapshot}-pal15-pilot5000" for snapshot in ("before", "after")
    } | {
        f"screen-{snapshot}-pal16-{variant}"
        for snapshot in ("before", "after")
        for variant in ("equal", "pilot50000", "bounds", "native")
    },
    "holdout": {
        f"holdout-{snapshot}-pal15-{variant}"
        for snapshot in ("before", "after")
        for variant in ("pilot500", "pilot5000", "messages4")
    } | {
        f"holdout-{snapshot}-pal16-{variant}"
        for snapshot in ("before", "after")
        for variant in ("baseline", "bounds")
    },
}


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def read_json(path):
    with path.open(encoding="utf-8") as stream:
        return json.load(stream)


def close(a, b):
    return math.isclose(float(a), float(b), rel_tol=1e-9, abs_tol=1e-300)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def panel_phase(name):
    lowered = name.lower()
    if "holdout" in lowered:
        return "holdout"
    if "screen" in lowered:
        return "screen"
    raise ValueError(f"panel name must identify screen or holdout phase: {name}")


def validate_named_settings(name, args):
    lowered = name.lower()
    rank = args.get("rank")
    require(rank in (15, 16), f"{name}: panel name/settings must identify rank 15 or 16")
    require(args.get("team") is not None, f"{name}: missing target team argument")
    require(args.get("floor") == (2 if rank == 15 else 200), f"{name}: allocation floor does not match rank protocol")
    if rank == 15:
        require(args.get("tree_leaves") == 256, f"{name}: P15 requires the 256-leaf tree")
        require(args.get("production_profile") is True, f"{name}: P15 requires production-profile pilots")
        require(args.get("rank_priority") is True, f"{name}: P15 requires generic rank priority")
        allowed_pilots = (5000,) if "screen" in lowered else (500, 5000)
        require(args.get("pilot_draws") in allowed_pilots, f"{name}: P15 pilot count is outside protocol")
        require(args.get("messages") in ((3,) if "screen" in lowered else (3, 4)), f"{name}: P15 message count is outside protocol")
    else:
        require(args.get("refine") is True, f"{name}: P16 requires secondary refinement")
    if "pal15" in lowered or "p15" in lowered:
        require(rank == 15 and args.get("draws") == 600000, f"{name}: P15 screening/holdout settings mismatch")
    if "pal16" in lowered or "p16" in lowered:
        require(rank == 16 and args.get("draws") == 2500000, f"{name}: P16 screening/holdout settings mismatch")
    if "pilot50000" in lowered:
        expected = (50000, "trained", "adaptive", "tilt")
    elif "pilot5000" in lowered and rank == 15:
        expected = (5000, "trained", "bounds", "tilt")
    elif "equal" in lowered:
        expected = (5000, "equal", "adaptive", "tilt")
    elif "bounds" in lowered:
        expected = (5000, "trained", "bounds", "tilt")
    elif "native" in lowered:
        expected = (5000, "trained", "adaptive", "native")
    else:
        expected = None
    if expected:
        pilot, allocation, guide, goals = expected
        require(args.get("pilot_draws") == pilot, f"{name}: pilot draw setting does not match panel name")
        require(args.get("allocation") == allocation and args.get("guide") == guide, f"{name}: allocation/guide do not match panel name")
        require(args.get("goals", "tilt") == goals, f"{name}: goal mode does not match panel name")


def validate_panel(record_path):
    panel_dir = record_path.parent
    name = panel_dir.name
    phase = panel_phase(name)
    record = read_json(record_path)
    require(record.get("exit_code") == 0, f"{name}: nonzero panel exit")
    require(record.get("binary_sha256") == FROZEN_BINARY, f"{name}: unexpected binary hash")
    binary_path = Path(record.get("binary", ""))
    require(binary_path.is_file() and sha256(binary_path) == FROZEN_BINARY, f"{name}: binary file hash mismatch or binary unavailable")
    required_files = {"stdout.jsonl", "stderr.txt"}
    output_hashes = record.get("output_files", {})
    require(required_files <= output_hashes.keys(), f"{name}: missing output hashes")
    for filename in required_files:
        path = panel_dir / filename
        require(path.is_file(), f"{name}: missing {filename}")
        require(sha256(path) == output_hashes[filename], f"{name}: {filename} hash mismatch")
    args = record.get("arguments", {})
    if "goals" not in args and len(record.get("command", [])) > 9:
        args["goals"] = record["command"][9]
    validate_named_settings(name, args)
    require(args.get("output") is not None, f"{name}: missing recorded output argument")
    request_path = Path(record.get("request", ""))
    require(request_path.is_file(), f"{name}: request file unavailable for hash verification")
    request_hash = sha256(request_path)
    require(record.get("request_sha256") == request_hash, f"{name}: request hash mismatch")
    expected_request = REQUEST_HASHES["before" if "before" in name else "after"]
    require(request_hash == expected_request, f"{name}: unexpected request hash")
    seeds = [int(x) for x in str(args.get("seeds", "")).split(",") if x]
    require(seeds == SEEDS[phase], f"{name}: final seeds do not match {phase} schedule")

    rows = []
    with (panel_dir / "stdout.jsonl").open(encoding="utf-8") as stream:
        for line_number, line in enumerate(stream, 1):
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError as error:
                raise ValueError(f"{name}: invalid JSON on stdout line {line_number}: {error}") from error
    setup_rows = [r for r in rows if r.get("event") == "strata_setup"]
    results = [r for r in rows if r.get("event") == "strata_result"]
    require(len(setup_rows) == 1 and len(results) == 3, f"{name}: expected one setup and three results")
    setup = setup_rows[0]
    require(setup.get("workers") == 4, f"{name}: setup did not use four workers")
    require(setup.get("setup_seed") == 808, f"{name}: setup seed must be 808")
    require(not any(setup.get("skipped", [])), f"{name}: setup reports skipped branches")
    require(sorted(r.get("seed") for r in results) == seeds, f"{name}: result seeds mismatch")
    require(all(r.get("exit_code", 0) == 0 for r in results), f"{name}: result exit failure")
    require(all(r.get("setup_seed") == 808 for r in results), f"{name}: setup seed/result mismatch")
    require(all(r.get("pilot_seed") == PILOT_SEEDS[phase] for r in results), f"{name}: pilot seed does not match {phase} schedule")
    require(args.get("pilot_seed") == PILOT_SEEDS[phase], f"{name}: recorded pilot seed does not match {phase} schedule")

    proposal = setup.get("proposal", {})
    strata = proposal.get("strata", [])
    branch_snapshots = []
    seed_summaries = []
    main_checks = []
    min_ess = None
    max_share = None
    for result in sorted(results, key=lambda r: r["seed"]):
        skipped = result.get("skipped", False)
        require(not (any(skipped) if isinstance(skipped, list) else skipped), f"{name}: result {result['seed']} reports skipped branches")
        require(result.get("allocation") == args.get("allocation") and result.get("guide") == args.get("guide"), f"{name}: result policy differs from recorded arguments")
        require(result.get("pilot_draws") == args.get("pilot_draws") and result.get("allocation_floor") == args.get("floor"), f"{name}: result pilot/floor differs from recorded arguments")
        require(result.get("production_profile") == bool(args.get("production_profile")), f"{name}: result production profile differs from recorded arguments")
        require(result.get("refined") == bool(args.get("refine")), f"{name}: result refinement differs from recorded arguments")
        require(result.get("goal_tilt") == (args.get("goals", "tilt") == "tilt"), f"{name}: result goal mode differs from recorded arguments")
        branches = result.get("branches", [])
        require(len(branches) == len(strata), f"{name}: branch/selection metadata count mismatch")
        require(sum(int(b.get("draws", 0)) for b in branches) == int(args["draws"]), f"{name}: branch draws do not sum to requested draws")
        require(sum(int(b.get("planned_draws", 0)) for b in branches) == int(args["draws"]), f"{name}: planned branch draws do not sum to requested draws")
        require(not any(b.get("skipped") for b in branches), f"{name}: skipped branch found")
        for metric in ("main", "check"):
            total = sum(float(b[metric]["probability"]) for b in branches)
            require(close(total, result[metric]["probability"]), f"{name}: {metric.upper()} branch probabilities fail conservation")
            fractions = [float(b[metric]["probability"]) / total for b in branches] if total > 0 else []
            require(not fractions or close(sum(fractions), 1.0), f"{name}: {metric.upper()} branch probability fractions fail conservation")
        main_checks.append(result["main"]["probability"])
        ess = result["main"].get("ess")
        share = result["main"].get("max_share")
        min_ess = ess if min_ess is None else min(min_ess, ess)
        max_share = share if max_share is None else max(max_share, share)
        top_branches = sorted(branches, key=lambda b: b["main"].get("probability", 0.0), reverse=True)[:3]
        main_total = float(result["main"]["probability"])
        top3 = [{"branch": b.get("index"), "main_probability_contribution": b["main"].get("probability"), "main_fraction": (b["main"].get("probability") / main_total if main_total > 0 else None), "selection": {key: strata[int(b.get("index", 0))].get(key) for key in ("direction", "target_outcomes", "exceptions", "joint_teams")}} for b in top_branches]
        seed_summaries.append({
            "seed": result["seed"], "main_probability": result["main"]["probability"],
            "check_probability": result["check"]["probability"], "accepted": result["accepted"],
            "ess": ess, "max_weight_share": share,
            "check_ess": result["check"].get("ess"),
            "check_max_weight_share": result["check"].get("max_share"),
            "check_hits": result["check"].get("hits"),
            "main_hits": result["main"].get("hits"),
            "batch_gap": {"main": result["main"].get("batch_gap"), "check": result["check"].get("batch_gap")},
            "draws": int(args["draws"]), "top3_main_branch_contributions": top3,
        })
        for expected_index, (branch, selection) in enumerate(zip(branches, strata)):
            require(branch.get("index") == expected_index, f"{name}: branch order differs from setup selection order")
            require(int(branch.get("draws", 0)) >= int(args["floor"]) and int(branch.get("planned_draws", 0)) >= int(args["floor"]), f"{name}: branch draw count is below allocation floor")
            require(branch["main"].get("samples") == branch.get("draws") and branch["check"].get("samples") == branch.get("draws"), f"{name}: branch sample counts do not match draws")
            branch_snapshots.append({
                "seed": result["seed"], "branch": branch.get("index"),
                "selection": {key: selection.get(key) for key in ("direction", "target_outcomes", "exceptions", "forced_count", "joint_teams", "joint_fixtures", "guided_fixtures")},
                "bounds": branch.get("bounds"),
                "allocation_bound": branch.get("allocation_bound"),
                "main_probability": branch["main"].get("probability"),
                "check_probability": branch["check"].get("probability"),
                "main_fraction": (branch["main"].get("probability") / result["main"]["probability"] if result["main"]["probability"] > 0 else None),
                "check_fraction": (branch["check"].get("probability") / result["check"]["probability"] if result["check"]["probability"] > 0 else None),
                "main_ess": branch["main"].get("ess"), "main_max_weight_share": branch["main"].get("max_share"),
                "main_hits": branch["main"].get("hits"), "check_ess": branch["check"].get("ess"),
                "check_max_weight_share": branch["check"].get("max_share"), "check_hits": branch["check"].get("hits"),
                "main_batch_gap": branch["main"].get("batch_gap"),
                "check_batch_gap": branch["check"].get("batch_gap"), "draws": branch.get("draws"),
                "planned_draws": branch.get("planned_draws"), "pilot_probability": (branch.get("pilot") or {}).get("probability"),
                "pilot_ess": (branch.get("pilot") or {}).get("ess"),
            })

    require(all(math.isfinite(float(s.get("proposal_normalizer", 0.0))) and float(s.get("proposal_normalizer", 0.0)) >= 0 for s in strata), f"{name}: invalid proposal normalizer")
    setup_work = setup.get("modeled_setup_work_units")
    messages = [r.get("messages") for r in results]
    message_summary = {
        "observed": messages,
        "modeled": [r.get("modeled_cost_units") for r in results],
        "charges": {
            "setup_ms": setup.get("setup_ms"),
            "pilot_ms": [r.get("pilot_ms") for r in results],
            "main_ms": [r.get("main_ms") for r in results],
            "check_ms": [r.get("check_ms") for r in results],
            "message_ms": [r.get("message_ms") for r in results],
        },
        "cost": "unavailable" if setup_work is None or any(r.get("modeled_cost_units") is None for r in results) else "available",
    }
    request = read_json(request_path)
    team = proposal.get("team", args.get("team"))
    target_fixture_ids = [game["id"] for game in request.get("games", []) if not game.get("played") and team in (game.get("home_id"), game.get("away_id"))]
    require(target_fixture_ids, f"{name}: no unplayed target-team fixtures found in request")
    semantic_roots = []
    for selection in strata:
        outcomes = selection.get("target_outcomes", [])
        require(len(outcomes) == len(target_fixture_ids), f"{name}: target outcome vector does not align with target fixtures")
        semantic_roots.append({
            "target_team_id": team,
            "target_fixture_ids": target_fixture_ids,
            "target_outcomes": outcomes,
            "classification": {
                "direction": selection.get("direction"),
                "exceptions": selection.get("exceptions", []),
                "joint_teams": selection.get("joint_teams", []),
            },
        })
    selection_team_ids = sorted({int(team_id) for selection in strata for team_id in (selection.get("joint_teams", []) + selection.get("exceptions", [])) if isinstance(team_id, (int, float))})
    settings = {key: args.get(key) for key in ("team", "rank", "draws", "allocation", "guide", "pilot_draws", "pilot_seed", "messages", "goals", "production_profile")}
    if args.get("goals", "tilt") == "native":
        settings["interpretation"] = "whole native-score policy arm; pilot scores, guidance selection, and allocation may all change"
    return {
        "name": name, "phase": phase,
        "settings": settings,
        "input": {"request_sha256": request_hash, "binary_sha256": record["binary_sha256"], "championship_id": request.get("id")},
        "target_and_rivals": {"target_team_id": team, "selection_team_ids_from_setup": selection_team_ids},
        "semantic_roots": semantic_roots,
        "selection_sets": strata,
        "seeds": seed_summaries,
        "mean_main_probability": sum(main_checks) / len(main_checks),
        "mean_check_probability": sum(r["check"]["probability"] for r in results) / len(results),
        "aggregate_diagnostics": {"main": {key: sum(float(r["main"].get(key, 0) or 0) for r in results) / len(results) for key in ("ess", "max_share", "hits", "batch_gap")}, "check": {key: sum(float(r["check"].get(key, 0) or 0) for r in results) / len(results) for key in ("ess", "max_share", "hits", "batch_gap")}},
        "mean_main_check": {"main": sum(main_checks) / len(main_checks), "check": sum(r["check"]["probability"] for r in results) / len(results)},
        "failed_seeds": sum(not bool(r["accepted"]) for r in results),
        "min_ess": min_ess, "max_weight_share": max_share,
        "min_main_check_ess": min(min(float(r[m].get("ess", 0) or 0) for r in results) for m in ("main", "check")),
        "max_main_check_weight_share": max(max(float(r[m].get("max_share", 0) or 0) for r in results) for m in ("main", "check")),
        "messages": message_summary,
        "pilot": [{"seed": r.get("pilot_seed"), "draws": r.get("pilot_draws"), "accepted": r.get("accepted"), "allocation_scores": r.get("allocation_scores")} for r in results],
        "timing": {"wall_seconds": record.get("wall_seconds"), "child_cpu_seconds": record.get("child_cpu_seconds")},
        "guide_choice_snapshots": branch_snapshots,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-dir", required=True, type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--baseline-dir", type=Path, help="optional prior campaign location; reported by reference only")
    parser.add_argument("--expected-prefix", default="screen", help="panel-name prefix counted toward completion; empty string counts all panels")
    parser.add_argument("--expected-panels", type=int, default=10, help="expected panel count for the selected prefix")
    args = parser.parse_args()
    run_dir = args.run_dir.resolve(strict=True)
    records = sorted(path for path in run_dir.glob("*/record.json") if path.parent != run_dir)
    require(records, f"no direct child panel records found in {run_dir}")
    panels = [validate_panel(path) for path in records]
    expected = [p for p in panels if p["name"].startswith(args.expected_prefix)]
    known_set = None
    if args.expected_prefix in EXPECTED_PANEL_NAMES and args.expected_panels == len(EXPECTED_PANEL_NAMES[args.expected_prefix]):
        known_set = EXPECTED_PANEL_NAMES[args.expected_prefix]
    elif args.expected_prefix == "" and args.expected_panels == 20:
        known_set = EXPECTED_PANEL_NAMES["screen"] | EXPECTED_PANEL_NAMES["holdout"]
    actual_names = {p["name"] for p in expected}
    if known_set is not None:
        complete = actual_names == known_set
        missing_names = sorted(known_set - actual_names)
        unexpected_names = sorted(actual_names - known_set)
    else:
        complete = len(expected) == args.expected_panels
        missing_names = []
        unexpected_names = []
    report = {"run_dir": str(run_dir), "panels": panels, "completion": {"prefix": args.expected_prefix, "completed": len(expected), "expected": args.expected_panels, "complete": complete, "missing_names": missing_names, "unexpected_names": unexpected_names}}
    if args.baseline_dir:
        report["baseline_reference"] = {"path": str(args.baseline_dir.resolve()), "included_in_new_cost": False}
    output = args.output
    if output:
        output = output.resolve()
        if output.exists() and not args.force:
            parser.error("--output exists; pass --force to replace it")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    else:
        print(json.dumps(report, separators=(",", ":"), sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"analyze_followups: {error}", file=sys.stderr)
        raise SystemExit(1)
