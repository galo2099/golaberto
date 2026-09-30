#!/usr/bin/env python3
"""Build a portable, explicitly partial rare-position reference dataset.

Only exact impossibility proofs, well-observed independent MC cells and
repeatable held-out importance estimates are eligible for scoring. Missing
probability evidence stays null; a numerical zero is never a substitute for it.
"""

import argparse
import hashlib
import json
import math
import statistics
from collections import Counter, defaultdict
from pathlib import Path


def digest(data):
    return hashlib.sha256(data).hexdigest()


def normalized_request(raw):
    """Fields read by Go's GroupType, retaining team/game/zone order."""
    phase = raw["phase"]
    integer = lambda value: 0 if value is None else int(value)
    return {
        "id": integer(raw["id"]),
        "zones": [{"position": z["position"]} for z in raw.get("zones", [])],
        "phase": {
            "sort": phase["sort"],
            "bonus_points": integer(phase.get("bonus_points", 0)),
            "bonus_points_threshold": integer(phase.get("bonus_points_threshold", 0)),
            "championship": {
                field: integer(phase["championship"].get(field, 0))
                for field in ("point_win", "point_draw", "point_loss")
            },
        },
        "team_groups": [
            {field: integer(team.get(field, 0)) for field in ("team_id", "add_sub", "bias")}
            for team in raw["team_groups"]
        ],
        "games": [
            {
                **{field: integer(game.get(field, 0)) for field in
                   ("id", "home_id", "away_id", "home_score", "away_score")},
                "home_power": float(game.get("home_power") or 0),
                "away_power": float(game.get("away_power") or 0),
                "played": bool(game.get("played", False)),
            }
            for game in raw["games"]
        ],
    }


def canonical_bytes(request):
    return json.dumps(request, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def probability_reference(evidence):
    """Each seed contributes its preselected held-out confirmation once."""
    accepted = [sample for sample in evidence if sample["accepted"]]
    values = [sample["p"] for sample in accepted]
    if not values:
        return None, None, None, 0
    ratio = max(values) / min(values)
    # Agreement is empirical, not a confidence interval or a bias guarantee.
    status = "reference_is" if len(values) >= 2 and ratio <= 3 else "provisional_is"
    return status, statistics.median(values), [min(values), max(values)], len(values)


def make_case(runs, proof, mc, output, estimator_commit, checks=()):
    first = runs[0]
    source = Path(first["input"]).read_bytes()
    source_hash = digest(source)
    if source_hash != first["input_sha256"]:
        raise ValueError("saved source input changed")
    request = normalized_request(json.loads(source))
    semantic_hash = digest(canonical_bytes(request))
    case_id = f"group-{first['group']}-{source_hash[:8]}"
    request_path = f"inputs/{case_id}.json"
    (output / request_path).write_text(json.dumps(request, ensure_ascii=False, indent=2) + "\n")
    proof_cells = {(c["team"], c["position"]): c for c in proof["cells"]}
    mc_cells = {(c["team"], c["position"]): c for c in mc["cells"]}
    searches = [{(c["team"], c["position"]): c for c in (r["cells"] or [])} for r in runs]
    records = []
    for team in sorted(int(team) for team in first["baseline"]):
        for position in range(1, len(request["team_groups"]) + 1):
            key = team, position
            baseline = [r["baseline"][str(team)][str(position - 1)] for r in runs]
            evidence = []
            reachable = any(c["hits"] > 0 or c.get("conditional_hits", 0) > 0 or
                            c.get("reachability", "").startswith(("reachable", "witness")) for c in baseline)
            for run, searched in zip(runs, searches):
                cell = searched.get(key)
                if cell is None:
                    continue
                reachable |= cell["sampled_witness"]
                chosen = cell["gentle"] if cell["gentle"]["valid"] else cell["selected"]
                evidence.append({
                    "seed": run["seed"], "accepted": cell["accepted"],
                    **chosen, "selected": cell["selected"], "gentle": cell["gentle"],
                    "sampled_witness": cell["sampled_witness"],
                    "check_conclusive": cell["check_conclusive"],
                    "check_contradicts": cell["check_contradicts"],
                })
            for check in checks:
                if (check["team"], check["position"]) != key:
                    continue
                for sample in check["pilots"]:
                    reachable |= sample["hits"] > 0
                    evidence.append({**sample, "seed": sample["stream_seed"],
                                     "accepted": sample["valid"], "source": "independent_proposal_check"})
            proven = proof_cells[key]["impossible"] or any(c.get("reachability", "").startswith("impossible") for c in baseline)
            count = mc_cells[key]["count"]
            if proven and (reachable or count > 0):
                raise ValueError(f"impossibility contradicts a witness: {case_id}/{key}")
            reachable |= count > 0
            status, probability, interval, confirmations = probability_reference(evidence)
            method = "held_out_importance_sampling" if probability is not None else None
            if proven:
                status, probability, interval, method = "impossible", 0.0, [0.0, 0.0], "necessary_fixture_constraints"
            elif count >= 25:
                probability = count / mc["samples"]
                status, interval, method = "reference_mc", None, "independent_plain_mc"
            elif status is None:
                status = "reachable_no_reference" if reachable else "estimated_unverified" if any(c["probability"] > 0 for c in baseline) else "undecided"
            records.append({
                "team": team, "position": position, "status": status,
                "scorable": status in ("impossible", "reference_mc", "reference_is"),
                "reachability": "impossible" if proven else "reachable" if reachable else "undecided",
                "probability": probability, "method": method,
                "is_empirical_range": interval, "independent_is_confirmations": confirmations,
                "plain_mc": {"samples": mc["samples"], "hits": count,
                             "probability": count / mc["samples"],
                             "std_err": math.sqrt((count / mc["samples"]) * (1 - count / mc["samples"]) / mc["samples"]),
                             "zero_hit_upper_95": -math.expm1(math.log(.05) / mc["samples"]) if count == 0 else None},
                "proof": proof_cells[key] if proven else None,
                "baseline_probabilities": [c["probability"] for c in baseline],
                "baseline_evidence": [{k: c.get(k) for k in ("hits", "conditional_hits", "reachability", "design")} for c in baseline],
                "search_evidence": evidence,
            })
    return {
        "case": case_id, "group": first["group"], "source_input_sha256": source_hash,
        "semantic_input_sha256": semantic_hash, "request": request_path,
        "estimator_commit": estimator_commit, "seeds": [r["seed"] for r in runs],
        "plain_mc_seed": mc["seed"], "plain_mc_workers": 4,
        "counts": dict(Counter(c["status"] for c in records)),
        "runs": [{k: r[k] for k in ("seed", "baseline_ms", "extra_ms", "baseline_work", "extra_work", "work_limit")} for r in runs],
        "proof_elapsed_ms": proof["elapsed_ms"], "proof_nodes_per_case": proof["nodes_per_case"],
        "cells": records,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("runs-dir", "proof-dir", "mc-dir", "output-dir", "estimator-commit"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--checks-dir")
    args = parser.parse_args()
    output = Path(args.output_dir)
    (output / "inputs").mkdir(parents=True, exist_ok=True)
    grouped = defaultdict(list)
    for path in sorted(Path(args.runs_dir).glob("group-*.json")):
        run = json.loads(path.read_text())
        grouped[run["input_sha256"]].append(run)
    indexed = lambda directory, prefix: {
        record["input_sha256"]: record
        for path in Path(directory).glob(prefix + "-*.json")
        for record in [json.loads(path.read_text())]
    }
    proofs, mc = indexed(args.proof_dir, "proof"), indexed(args.mc_dir, "mc")
    cases = []
    for fingerprint, runs in grouped.items():
        runs.sort(key=lambda r: r["seed"])
        if len({r["seed"] for r in runs}) != len(runs):
            raise ValueError("duplicate independent seed")
        checks = []
        if args.checks_dir:
            path = Path(args.checks_dir) / f"proposal-check-{runs[0]['group']}-{fingerprint[:8]}.json"
            if path.exists():
                checks = json.loads(path.read_text())
        cases.append(make_case(runs, proofs[fingerprint], mc[fingerprint], output, args.estimator_commit, checks))
    result = {
        "schema_version": 1, "probability_units": "fraction", "modeled_work_multiplier": 100,
        "description": "Partial golden reference: empirical MC and repeated importance estimates are not exact truth. Null probabilities remain unresolved.",
        "is_agreement_maximum_ratio": 3, "mc_minimum_hits": 25,
        "source_separation": "Independent MC streams and held-out importance confirmations; pilot draws excluded.",
        "cases": cases,
    }
    (output / "reference.json").write_text(json.dumps(result, ensure_ascii=False, indent=2, allow_nan=False) + "\n")
    print(json.dumps({c["case"]: c["counts"] for c in cases}, indent=2))


if __name__ == "__main__":
    main()
