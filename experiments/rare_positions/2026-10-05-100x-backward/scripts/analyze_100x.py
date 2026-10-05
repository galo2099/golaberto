#!/usr/bin/env python3
"""Summarize archived branch-stratification rows for the 100x campaign."""
import argparse
import copy
import hashlib
import json
import math
from pathlib import Path
import sys

PANELS = [f"{phase}-{team}-100x{suffix}" for suffix in ("", "-pilot60031")
          for phase in ("before", "after") for team in ("flam13", "pal15", "pal16")]
SEEDS = (60013, 60017, 60029)
EXPECTED_CELL = {"flam13": (17, 13), "pal15": (16, 15), "pal16": (16, 16)}
FROZEN_BINARY_SHA256 = "1c191c1fe9a7d6a07a652748545443f6cb8a4c61e49781fc9a15292ed27ec4f1"
EXPECTED_REQUEST_SHA256 = {
    "before": "2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625",
    "after": "cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee",
}


def sha256(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def load_run(path):
    record_path = path / "record.json"
    stdout_path = path / "stdout.jsonl"
    record = json.loads(record_path.read_text())
    if not stdout_path.is_file():
        raise ValueError(f"missing {stdout_path}")
    if sha256(stdout_path) != record.get("output_files", {}).get("stdout.jsonl"):
        raise ValueError(f"stdout hash mismatch: {stdout_path}")
    for name, expected in record.get("output_files", {}).items():
        output_file = path / name
        if not output_file.is_file() or sha256(output_file) != expected:
            raise ValueError(f"output hash mismatch: {output_file}")
    for key in ("binary", "request"):
        source = Path(record[key])
        if not source.is_file() or sha256(source) != record.get(key + "_sha256"):
            raise ValueError(f"{key} missing or hash mismatch for {path}")
    rows = [json.loads(line) for line in stdout_path.read_text().splitlines() if line.strip()]
    setups = [r for r in rows if r.get("event") == "strata_setup"]
    results = [r for r in rows if r.get("event") == "strata_result"]
    if record.get("exit_code") != 0:
        raise ValueError(f"run exited {record.get('exit_code')}: {path}")
    if len(setups) != 1:
        raise ValueError(f"expected exactly one strata_setup row in {path}; found {len(setups)}")
    return {"path": str(path), "record": record, "setup": setups[0], "results": results}


def num(v):
    return v if isinstance(v, (int, float)) and math.isfinite(v) else None


def summary(values):
    values = [v for v in values if num(v) is not None]
    return {"n": len(values), "mean": sum(values) / len(values) if values else None,
            "min": min(values) if values else None, "max": max(values) if values else None}


def validate_results(results, seeds):
    actual = [r.get("seed") for r in results]
    if len(actual) != len(seeds) or len(set(actual)) != len(actual) or set(actual) != set(seeds):
        raise ValueError(f"expected exactly one result for each seed {seeds}; found {actual}")


def validate_pilot(run, expected):
    recorded = run["record"].get("arguments", {}).get("pilot_seed")
    if recorded is None or int(recorded) != expected:
        raise ValueError(f"{run['path']}: recorded pilot seed {recorded}, expected {expected}")
    for result in run["results"]:
        if result.get("pilot_seed") is None or int(result["pilot_seed"]) != expected:
            raise ValueError(f"{run['path']}: result pilot seed {result.get('pilot_seed')}, expected {expected}")


def validate_panel_identity(panel, run):
    phase, cell = panel.split("-", 1)[0], panel.split("-", 2)[1]
    team, rank = EXPECTED_CELL[cell]
    arguments = run["record"].get("arguments", {})
    proposal = (run.get("setup") or {}).get("proposal", {})
    if int(arguments.get("team", -1)) != team or int(arguments.get("rank", -1)) != rank:
        raise ValueError(f"{run['path']}: runner team/rank does not match {panel}")
    if int(proposal.get("team", -1)) != team or int(proposal.get("rank", -1)) != rank:
        raise ValueError(f"{run['path']}: setup proposal team/rank does not match {panel}")
    if run["record"].get("binary_sha256") != FROZEN_BINARY_SHA256:
        raise ValueError(f"{run['path']}: binary hash does not match frozen protocol binary")
    if run["record"].get("request_sha256") != EXPECTED_REQUEST_SHA256[phase]:
        raise ValueError(f"{run['path']}: request hash does not match {phase} panel fixture")


def result_contributions(result, branches, kind):
    total = num((result.get(kind) or {}).get("probability"))
    values = [num((b.get(kind) or {}).get("probability")) for b in branches]
    if any(v is None for v in values):
        raise ValueError(f"missing {kind} branch probability for seed {result.get('seed')}")
    branch_sum = sum(values)
    if total is None:
        raise ValueError(f"missing aggregate {kind} probability for seed {result.get('seed')}")
    if not math.isclose(branch_sum, total, rel_tol=1e-8, abs_tol=1e-300):
        raise ValueError(f"{kind} branch probabilities sum to {branch_sum}, aggregate is {total}")
    fractions = [v / total if total > 0 else None for v in values]
    if total > 0 and not math.isclose(sum(fractions), 1.0, rel_tol=1e-8, abs_tol=1e-10):
        raise ValueError(f"{kind} normalized branch contributions do not sum to one")
    return values, fractions


def witness_summary(witness, target_team, request):
    """Use Rust's phase-derived team totals; derive only fixture W/D/L from outcomes."""
    if not witness:
        return {"teams": {}, "target_wdl": None, "rival_classes": {"above": [], "equal": [], "below": []}}
    team_rows = {int(t["team_id"]): {"points": int(t["points"]), "wins": int(t["wins"])}
                 for t in witness.get("teams", [])}
    target = team_rows.get(target_team)
    classes = {"above": [], "equal": [], "below": []}
    if target is not None:
        target_score = (target["points"], target["wins"])
        for team_id, score in team_rows.items():
            if team_id == target_team:
                continue
            label = "above" if (score["points"], score["wins"]) > target_score else (
                "below" if (score["points"], score["wins"]) < target_score else "equal")
            classes[label].append(team_id)
    games = {int(g["id"]): g for g in request.get("games", [])}
    wdl = {"wins": 0, "draws": 0, "losses": 0}
    remaining_wdl = {"wins": 0, "draws": 0, "losses": 0}
    if target is not None:
        for game in games.values():
            hid, aid = int(game["home_id"]), int(game["away_id"])
            if target_team not in (hid, aid):
                continue
            if game.get("played"):
                hs, ass = int(game.get("home_score", 0)), int(game.get("away_score", 0))
                outcome = 2 if hs > ass else 1 if hs == ass else 0
            else:
                item = next((o for o in witness.get("fixtures", []) if int(o.get("fixture_id", -1)) == int(game["id"])), None)
                if item is None:
                    continue
                outcome = int(item["outcome"])
                hid, aid = int(game["home_id"]), int(game["away_id"])
            bucket = "draws" if outcome == 1 else "wins" if (outcome == 2 and hid == target_team) or (outcome == 0 and aid == target_team) else "losses"
            wdl[bucket] += 1
            if not game.get("played"):
                remaining_wdl[bucket] += 1
    return {"teams": {str(k): v for k, v in sorted(team_rows.items())}, "target_wdl": wdl,
            "target_remaining_wdl": remaining_wdl,
            "rival_classes": {k: sorted(v) for k, v in classes.items()}, "classification_label": "points_wins_only"}


def analyze_run(run):
    setup, results = run["setup"], run["results"]
    if not setup:
        raise ValueError(f"no strata_setup in {run['path']}")
    proposal_data = setup.get("proposal", {})
    proposal = proposal_data.get("strata", [])
    tree_selected = (proposal_data.get("tree") or {}).get("selected", [])
    request = json.loads(Path(run["record"]["request"]).read_text())
    target_team = int(run["record"].get("arguments", {}).get("team") or proposal_data.get("team"))
    target_fixture_ids = [int(g["id"]) for g in request.get("games", [])
                          if not g.get("played") and target_team in (int(g["home_id"]), int(g["away_id"]))]
    branch_rows, roots = [], {}
    for result in results:
        main_values, main_fractions = result_contributions(result, result.get("branches", []), "main")
        check_values, check_fractions = result_contributions(result, result.get("branches", []), "check")
        branches = result.get("branches", [])
        if len(branches) != len(proposal):
            raise ValueError(f"seed {result.get('seed')} has {len(branches)} branches but proposal has {len(proposal)}")
        indices = [b.get("index") for b in branches]
        if indices != list(range(len(proposal))):
            raise ValueError(f"seed {result.get('seed')} has missing or duplicate branch indices: {indices}")
        for b in branches:
            i = int(b.get("index", -1))
            prop = proposal[i] if 0 <= i < len(proposal) else {}
            root = prop.get("target_outcomes")
            if root is not None and len(root) != len(target_fixture_ids):
                raise ValueError(f"root {i} outcome count does not match unplayed target fixture list")
            semantic_root = {"target_team": target_team, "target_fixture_ids": target_fixture_ids, "target_outcomes": root}
            signature = json.dumps(semantic_root, sort_keys=True) if root is not None else None
            tree_leaf = tree_selected[i] if i < len(tree_selected) else None
            tree_meta = tree_leaf or {}
            strict_signature = {"target_team": target_team, "selected": tree_meta.get("selected"),
                                "strict": tree_meta.get("strict"), "strict_ids": prop.get("exceptions"),
                                "direction": prop.get("direction"), "secondary_team": prop.get("secondary_team"),
                                "forced": prop.get("forced"),
                                "selected_metadata_available": tree_leaf is not None}
            if tree_leaf is None:
                strict_signature["selected"] = None
                strict_signature["strict"] = None
            row = {"seed": result.get("seed"), "index": i, "proposal_normalizer": num(prop.get("proposal_normalizer")),
                   "branch_probability": main_values[i], "normalized_branch_contribution": main_fractions[i],
                   "check_branch_probability": check_values[i], "normalized_check_contribution": check_fractions[i],
                   "root_signature": semantic_root if root is not None else None, "root_signature_key": signature,
                   "direction": prop.get("direction"),
                   "strict_signature": strict_signature,
                   "draws": b.get("draws"), "pilot_hits": (b.get("pilot") or {}).get("hits"),
                   "message_selected": i in ((result.get("messages") or {}).get("selected") or []),
                   "main": b.get("main"), "check": b.get("check"), "main_witness": b.get("main_outcome_witness"),
                   "check_witness": b.get("check_outcome_witness"), "source": run["path"]}
            for kind in ("main", "check"):
                witness = row[kind + "_witness"]
                row[kind + "_witness_summary"] = witness_summary(witness, target_team, request)
            branch_rows.append(row)
            if signature is not None:
                roots.setdefault(signature, {"root_signature": semantic_root, "branches": []})["branches"].append(row)
    root_rows = []
    for item in roots.values():
        main_by_seed, check_by_seed = {}, {}
        classifications = {"main": {"above": 0, "equal": 0, "below": 0},
                           "check": {"above": 0, "equal": 0, "below": 0}}
        for b in item["branches"]:
            main_by_seed.setdefault(b["seed"], 0.0)
            check_by_seed.setdefault(b["seed"], 0.0)
            main_by_seed[b["seed"]] += b["normalized_branch_contribution"] or 0.0
            check_by_seed[b["seed"]] += b["normalized_check_contribution"] or 0.0
            for kind in ("main", "check"):
                for label in b[kind + "_witness_summary"]["rival_classes"]:
                    classifications[kind][label] += len(b[kind + "_witness_summary"]["rival_classes"][label])
        item["contribution_by_seed"] = main_by_seed
        item["check_contribution_by_seed"] = check_by_seed
        item["first_hit_rival_ids_by_points_wins_class"] = classifications
        item["classification_label"] = "points_wins_only"
        root_rows.append(item)
    return {"path": run["path"], "setup": setup, "result_count": len(results), "branches": branch_rows, "roots": root_rows,
            "result_summaries": [{"seed": r.get("seed"), "accepted": r.get("accepted"), "main": r.get("main"), "check": r.get("check"),
                "messages": r.get("messages"), "pilot_zero_hit_branches": sum(1 for b in r.get("branches", []) if (b.get("pilot") or {}).get("hits") == 0),
                "modeled_cost_units": r.get("modeled_cost_units"), "elapsed_ms": r.get("elapsed_ms"),
                "branches": [b for b in branch_rows if b.get("seed") == r.get("seed")],
                "main_witnesses": [b.get("main_outcome_witness") for b in r.get("branches", []) if b.get("main_outcome_witness")],
                "check_witnesses": [b.get("check_outcome_witness") for b in r.get("branches", []) if b.get("check_outcome_witness")]} for r in results]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-dir", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--output", type=Path)
    parser.add_argument("--panels", default=",".join(PANELS))
    parser.add_argument("--seeds", default=",".join(map(str, SEEDS)))
    parser.add_argument("--expected-pilot-seeds", default="808,60031", help="fixed proposal-training seeds expected across the two panel families")
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()
    panels, seeds = args.panels.split(","), [int(s) for s in args.seeds.split(",")]
    expected_pilots = {int(v) for v in args.expected_pilot_seeds.split(",")}
    if len(panels) != 12 or set(panels) != set(PANELS) or len(set(panels)) != len(panels):
        raise ValueError("panels must be exactly the six before/after panels and their pilot60031 counterparts")
    if seeds != list(SEEDS):
        raise ValueError(f"final seeds must be {list(SEEDS)} in the configured order")
    if expected_pilots != {808, 60031}:
        raise ValueError("expected proposal-training pilot seeds must be exactly 808 and 60031")
    analyzed, missing, completeness = {}, [], {}
    for panel in panels:
        analyzed[panel] = {}
        aggregate_path = args.run_dir / panel
        aggregate_run = load_run(aggregate_path) if (aggregate_path / "record.json").is_file() else None
        if aggregate_run:
            validate_panel_identity(panel, aggregate_run)
            validate_results(aggregate_run["results"], seeds)
            expected_pilot = 60031 if panel.endswith("-pilot60031") else 808
            validate_pilot(aggregate_run, expected_pilot)
            if expected_pilot not in expected_pilots:
                missing.append(f"{panel}: pilot seed {expected_pilot} absent from --expected-pilot-seeds")
            full = analyze_run(aggregate_run)
        else:
            full = None
        for seed in seeds:
            path = aggregate_path / str(seed)
            run = load_run(path) if (path / "record.json").is_file() else aggregate_run
            if run is None:
                missing.append(f"{path} (or aggregate {aggregate_path})")
                continue
            if run is not aggregate_run:
                validate_panel_identity(panel, run)
            expected_pilot = 60031 if panel.endswith("-pilot60031") else 808
            if expected_pilot not in expected_pilots:
                missing.append(f"{panel}: pilot seed {expected_pilot} absent from --expected-pilot-seeds")
            if run is not aggregate_run:
                validate_pilot(run, expected_pilot)
                validate_results(run["results"], [seed])
                result = analyze_run(run)
            else:
                result = copy.deepcopy(full)
            got = {r.get("seed") for r in result["result_summaries"]}
            if seed not in got:
                missing.append(f"{run['path']}: expected seed {seed}, found {sorted(got)}")
                continue
            result["result_summaries"] = [r for r in result["result_summaries"] if r.get("seed") == seed]
            result["branches"] = [b for b in result["branches"] if b.get("seed") == seed]
            for root in result["roots"]:
                root["branches"] = [b for b in root["branches"] if b.get("seed") == seed]
                root["contribution_by_seed"] = {str(seed): root["contribution_by_seed"].get(seed, root["contribution_by_seed"].get(str(seed), 0.0))}
                root["check_contribution_by_seed"] = {str(seed): root["check_contribution_by_seed"].get(seed, root["check_contribution_by_seed"].get(str(seed), 0.0))}
                root["first_hit_rival_ids_by_points_wins_class"] = {
                    kind: {label: sum(len(b[kind + "_witness_summary"]["rival_classes"][label]) for b in root["branches"])
                           for label in ("above", "equal", "below")} for kind in ("main", "check")}
            result["result_count"] = len(result["result_summaries"])
            analyzed[panel][str(seed)] = result
            completeness[f"{panel}/{seed}"] = {"result_seeds": sorted(got), "result_count": result["result_count"],
                "accepted_count": sum(bool(r.get("accepted")) for r in result["result_summaries"]), "record": str(Path(run["path"]) / "record.json"),
                "binary_sha256": run["record"].get("binary_sha256"), "request_sha256": run["record"].get("request_sha256")}
    if missing:
        raise ValueError("incomplete campaign:\n" + "\n".join(missing))

    messages4_names = ("before-pal15-100x-pilot60031-messages4", "after-pal15-100x-pilot60031-messages4")
    messages4_dirs = [args.run_dir / name for name in messages4_names]
    present = [path.exists() for path in messages4_dirs]
    if any(present) and not all(present):
        raise ValueError("both before/after Pal15 pilot60031 messages4 directories must exist together")
    messages4_analysis = None
    if all(present):
        messages4_analysis = {}
        for panel, path in zip(messages4_names, messages4_dirs):
            run = load_run(path)
            validate_panel_identity(panel, run)
            validate_results(run["results"], seeds)
            validate_pilot(run, 60031)
            arguments = run["record"].get("arguments", {})
            overrides = run["record"].get("environment_overrides", {})
            if int(arguments.get("draws", -1)) != 600000:
                raise ValueError(f"{path}: expected 600000 final draws, found {arguments.get('draws')}")
            if arguments.get("seeds") != ",".join(map(str, seeds)):
                raise ValueError(f"{path}: unexpected final seeds argument {arguments.get('seeds')}")
            if int(arguments.get("messages", -1)) != 4 or overrides.get("TREE_MESSAGES") != "4":
                raise ValueError(f"{path}: expected messages=4 and TREE_MESSAGES=4")
            full = analyze_run(run)
            base_panel = panel.replace("-messages4", "")
            base_path = args.run_dir / base_panel
            base_run = load_run(base_path) if (base_path / "record.json").is_file() else load_run(base_path / str(seeds[0]))
            validate_panel_identity(base_panel, base_run)
            validate_pilot(base_run, 60031)
            if int(base_run["record"].get("arguments", {}).get("messages", -1)) != 3:
                raise ValueError(f"{base_path}: paired primary baseline must use three messages")
            if int(base_run["record"].get("arguments", {}).get("draws", -1)) != 600000:
                raise ValueError(f"{base_path}: paired primary baseline must use 600000 final draws")
            validate_results(base_run["results"], seeds if base_path == Path(base_run["path"]) else [seeds[0]])
            baseline_arguments = dict(base_run["record"].get("arguments", {}))
            counterfactual_arguments = dict(run["record"].get("arguments", {}))
            for arguments_to_compare in (baseline_arguments, counterfactual_arguments):
                arguments_to_compare.pop("output", None)
                arguments_to_compare.pop("messages", None)
            if baseline_arguments != counterfactual_arguments:
                raise ValueError(f"{path}: messages4 runner arguments differ from paired messages3 baseline")
            baseline_environment = dict(base_run["record"].get("environment_overrides", {}))
            counterfactual_environment = dict(run["record"].get("environment_overrides", {}))
            baseline_environment.pop("TREE_MESSAGES", None)
            counterfactual_environment.pop("TREE_MESSAGES", None)
            if baseline_environment != counterfactual_environment:
                raise ValueError(f"{path}: messages4 environment differs from paired messages3 baseline")
            messages4_analysis[panel] = {"record": {"path": str(path / "record.json"),
                "binary_sha256": run["record"].get("binary_sha256"), "request_sha256": run["record"].get("request_sha256"),
                "wall_seconds": run["record"].get("wall_seconds"), "child_cpu_seconds": run["record"].get("child_cpu_seconds")},
                "by_seed": {}}
            for seed in seeds:
                row = next(r for r in full["result_summaries"] if r.get("seed") == seed)
                baseline = next(r for r in analyzed[base_panel][str(seed)]["result_summaries"] if r.get("seed") == seed)
                branch_by_index = {b["index"]: b for b in row["branches"]}
                base_branch_by_index = {b["index"]: b for b in baseline["branches"]}

                def selection(result_row, lookup):
                    selected = (result_row.get("messages") or {}).get("selected") or []
                    if len(set(selected)) != len(selected) or any(index not in lookup for index in selected):
                        raise ValueError(f"{panel} seed {seed}: message selection contains duplicate or invalid branch indices")
                    return {"indices": selected,
                            "semantic_branches": [{"index": index, "root_signature": lookup[index]["root_signature"],
                                "strict_signature": lookup[index]["strict_signature"]}
                                for index in selected if index in lookup]}

                def estimate(result_row, lookup):
                    return {"main_probability": (result_row.get("main") or {}).get("probability"),
                            "check_probability": (result_row.get("check") or {}).get("probability"),
                            "accepted": result_row.get("accepted"),
                            "main_ess": (result_row.get("main") or {}).get("ess"),
                            "main_max_share": (result_row.get("main") or {}).get("max_share"),
                            "check_ess": (result_row.get("check") or {}).get("ess"),
                            "check_max_share": (result_row.get("check") or {}).get("max_share"),
                            "branch_contributions": [{"index": b["index"], "root_signature": b["root_signature"],
                                "main_probability": b["branch_probability"], "main_fraction": b["normalized_branch_contribution"],
                                "check_probability": b["check_branch_probability"], "check_fraction": b["normalized_check_contribution"]}
                                for b in lookup.values()],
                            "modeled_cost_units": result_row.get("modeled_cost_units"),
                            "elapsed_ms": result_row.get("elapsed_ms")}

                messages4_analysis[panel]["by_seed"][str(seed)] = {
                    "baseline_messages3": {"selection": selection(baseline, base_branch_by_index),
                                            "estimate": estimate(baseline, base_branch_by_index)},
                    "messages4": {"selection": selection(row, branch_by_index),
                                  "estimate": estimate(row, branch_by_index)}}
    output = args.output or args.run_dir / "analysis-100x.json"
    if output.exists() and not args.force:
        raise ValueError(f"refusing to overwrite {output}; pass --force")
    compact = {}
    for panel, seeds_data in analyzed.items():
        for seed, data in seeds_data.items():
            compact[f"{panel}/{seed}"] = {"result_count": data["result_count"], "accepted_count": sum(bool(r.get("accepted")) for r in data["result_summaries"]),
                "main_probability": summary([(r.get("main") or {}).get("probability") for r in data["result_summaries"]]),
                "check_probability": summary([(r.get("check") or {}).get("probability") for r in data["result_summaries"]]),
                "ess": summary([(r.get("main") or {}).get("ess") for r in data["result_summaries"]]),
                "max_share": summary([(r.get("main") or {}).get("max_share") for r in data["result_summaries"]]),
                "check_ess": summary([(r.get("check") or {}).get("ess") for r in data["result_summaries"]]),
                "check_max_share": summary([(r.get("check") or {}).get("max_share") for r in data["result_summaries"]]),
                "branch_concentration": {"top1_mass": summary([max((b.get("normalized_branch_contribution") or 0 for b in r["branches"]), default=0) for r in data["result_summaries"]]),
                    "top3_mass": summary([sum(sorted((b.get("normalized_branch_contribution") or 0 for b in r["branches"]), reverse=True)[:3]) for r in data["result_summaries"]]),
                    "branch_ess": summary([b["main"]["ess"] for b in data["branches"] if b.get("main")]),
                    "pilot_zero_hit_branches": sum(r["pilot_zero_hit_branches"] for r in data["result_summaries"]),
                    "message_selected_branches": sum(len((r.get("messages") or {}).get("selected", [])) for r in data["result_summaries"])},
                "roots_top10": sorted(data["roots"], key=lambda r: sum(r["contribution_by_seed"].values()), reverse=True)[:10]}
    panel_summary = {}
    for panel in panels:
        rows = [compact[f"{panel}/{seed}"] for seed in seeds]
        panel_summary[panel] = {metric: summary([row[metric]["mean"] for row in rows])
                                for metric in ("main_probability", "check_probability", "ess", "max_share", "check_ess", "check_max_share")}
        panel_summary[panel]["accepted_count"] = sum(row["accepted_count"] for row in rows)
        panel_summary[panel]["result_count"] = sum(row["result_count"] for row in rows)
    pilot_pairs = {}
    for base in [f"{phase}-{team}-100x" for phase in ("before", "after") for team in ("flam13", "pal15", "pal16")]:
        alt = base + "-pilot60031"
        main_by_root, check_by_root = {}, {}
        for seed in seeds:
            for family, key, destination in ((base, "contribution_by_seed", main_by_root),
                                             (alt, "contribution_by_seed", main_by_root),
                                             (base, "check_contribution_by_seed", check_by_root),
                                             (alt, "check_contribution_by_seed", check_by_root)):
                data = analyzed[family][str(seed)]
                for root in data["roots"]:
                    signature = json.dumps(root["root_signature"], sort_keys=True)
                    pair = destination.setdefault(signature, {"root_signature": root["root_signature"], "by_seed": {}})
                    values = pair["by_seed"].setdefault(str(seed), {})
                    values["60031" if family == alt else "808"] = root[key].get(str(seed), 0.0)
        def deltas(groups):
            rows = []
            for item in groups.values():
                differences = {seed: values.get("60031", 0.0) - values.get("808", 0.0)
                               for seed, values in item["by_seed"].items()}
                rows.append({"root_signature": item["root_signature"], "delta_by_final_seed": differences,
                             "mean_delta": sum(differences.values()) / len(differences) if differences else 0.0})
            return sorted(rows, key=lambda row: abs(row["mean_delta"]), reverse=True)[:10]
        pilot_pairs[base] = {"main_probability_delta_60031_minus_808":
            (panel_summary[alt]["main_probability"]["mean"] - panel_summary[base]["main_probability"]["mean"])
            if panel_summary[alt]["main_probability"]["mean"] is not None and panel_summary[base]["main_probability"]["mean"] is not None else None,
            "main_probability_by_pilot_seed": {"808": panel_summary[base]["main_probability"]["mean"],
                                               "60031": panel_summary[alt]["main_probability"]["mean"]},
            "top_main_semantic_root_contribution_deltas": deltas(main_by_root),
            "top_check_semantic_root_contribution_deltas": deltas(check_by_root),
            "pairing_note": "final seeds are paired across the two fixed training-pilot seeds; these are descriptive deltas, not 36 independent configurations"}
    payload = {"schema": "100x-backward-analysis-v1", "panels": panels, "seeds": seeds, "completeness": completeness,
               "panel_summary_across_final_seeds": panel_summary, "pilot_seed_paired_comparison": pilot_pairs,
               "messages4_counterfactual": messages4_analysis,
               "summary": compact, "runs": analyzed,
               "interpretation_limits": ["Branch contribution is branch probability divided by aggregate probability, computed separately for main and check.",
                 "First-hit witnesses retain fixture outcomes but not actual goal scores. Points/wins classifications do not resolve goal-difference tie breakers and do not establish the full outcome distribution.",
                 "Pilot zero-hit, retuned-message selection, and witness correlations are descriptive diagnostics, not causal or confidence claims."]}
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"output": str(output), "panels": len(panels), "runs": len(completeness), "branches": sum(len(r["branches"]) for d in analyzed.values() for r in d.values())}, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"analyze_100x: {error}", file=sys.stderr)
        raise
