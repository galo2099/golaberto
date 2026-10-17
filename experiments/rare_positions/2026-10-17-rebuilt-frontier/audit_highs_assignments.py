#!/usr/bin/env python3
"""Audit HiGHS feasibility for the saved cohort-5 pivot-prefix assignments."""
from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import math
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
DEFAULT_REQUEST = REPO / "experiments/rare_positions/reference/2026-10-04-two-results/inputs/group-16498-two-results.json"
DEFAULT_RAW = Path("/tmp/pal16-root-tree-audit-raw.json")
DEFAULT_REFERENCE = Path("/tmp/trusted-highs-control-pal16-180s.json")
DEFAULT_BINARY = Path("/tmp/frontier-recovery-trusted-highs")
DEFAULT_OUTPUT = HERE / "highs-cohort5-assignments.json"


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def native_model(binary, request, team_id, position, region_id, deadline):
    proc = subprocess.Popen([str(binary), str(request), str(team_id), str(position)],
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, text=True, bufsize=1)
    try:
        for command in ({"op": "describe"}, {"op": "feasibility_model", "id": region_id}):
            if time.monotonic() >= deadline:
                raise TimeoutError("shared audit budget exhausted during native export")
            proc.stdin.write(json.dumps(command, separators=(",", ":")) + "\n")
            proc.stdin.flush()
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("shared audit budget exhausted during native export")
            import selectors
            sel = selectors.DefaultSelector()
            sel.register(proc.stdout, selectors.EVENT_READ)
            if not sel.select(remaining):
                raise TimeoutError("native model export exceeded shared audit budget")
            line = proc.stdout.readline()
            if not line:
                raise RuntimeError("native estimator exited during model export")
            result = json.loads(line)
            if "error" in result:
                raise RuntimeError(result["error"])
            if command["op"] == "describe":
                desc = result
            else:
                model = result
        return desc, model
    finally:
        try:
            proc.stdin.write('{"op":"quit"}\n')
            proc.stdin.flush()
            proc.wait(timeout=1)
        except Exception:
            proc.kill()
            proc.wait()


def validate_binding(raw, request_path, reference_path, desc, model, team_id, position):
    request_hash = sha256(request_path)
    reference = json.loads(reference_path.read_text())
    if reference.get("request_sha256") != request_hash:
        raise ValueError("request SHA256 does not match the trusted control artifact")
    if reference.get("fixture_signature") != {"team_id": team_id, "position": position}:
        raise ValueError("reference artifact target does not match requested target")
    if Path(raw["input"]).resolve() != request_path.resolve():
        raise ValueError("raw pivot audit names a different input request")
    if raw["signature"].get("team_id") != team_id or raw["signature"].get("position") != position:
        raise ValueError("raw pivot audit signature does not match requested target")
    audit = raw["cohort5_pivot584_mixture"]
    if audit["region_id"] != 5 or audit["root_id"] != 0 or audit["pivot_id"] != 584:
        raise ValueError("raw pivot audit does not describe cohort 5 / root 0 / pivot 584")
    if desc["target"]["team_id"] != team_id or desc["target"]["position"] != position:
        raise ValueError("native estimator target does not match requested target")
    if len(desc["regions"]) <= audit["region_id"]:
        raise ValueError("native estimator is missing cohort 5")
    native_region = desc["regions"][audit["region_id"]]
    if native_region["root_id"] != audit["root_id"]:
        raise ValueError("native cohort root differs from raw audit")
    if set(native_region["opponent_ids"]) != set(audit["above_ids"]):
        raise ValueError("native cohort opponent set differs from raw audit")
    stride = int(model["stride"])
    team_index = {tid: i for i, tid in enumerate(model["team_ids"])}
    for bound in audit["score_bounds"]:
        tid = int(bound["team_id"])
        if tid not in team_index:
            raise ValueError(f"native model missing audited team {tid}")
        i = team_index[tid]
        if int(model["base_packed"][i]) != int(bound["base_points"]) * stride + int(bound["base_wins"]):
            raise ValueError(f"native base packed total differs for team {tid}")
        if int(model["lower_packed"][i]) != int(bound["lower_packed"]) or int(model["upper_packed"][i]) != int(bound["upper_packed"]):
            raise ValueError(f"native packed bounds differ for team {tid}")
    if len(model["fixtures"]) != desc["fixture_count"]:
        raise ValueError("native feasibility model fixture count mismatch")
    if len(audit["prefixes"]) != audit["pattern_count"]:
        raise ValueError("raw prefix count differs from its declared pattern count")
    return request_hash


def enumerate_prefix_coverage(model, audit, team_id):
    """Enumerate all mask-allowed pivot assignments passing necessary packed bounds."""
    fixtures = model["fixtures"]
    prefix_fixtures = audit["prefix_fixtures"]
    indices = [int(x["fixture_index"]) for x in prefix_fixtures]
    if len(indices) != len(set(indices)):
        raise ValueError("duplicate fixture in pivot prefix declaration")
    if any(i < 0 or i >= len(fixtures) for i in indices):
        raise ValueError("pivot prefix fixture index outside native model")
    pivot = int(audit["pivot_id"])
    native_incident = {i for i, f in enumerate(fixtures)
                       if pivot in (int(f["home_id"]), int(f["away_id"]))}
    if set(indices) != native_incident:
        raise ValueError("declared prefix fixtures do not cover every native pivot fixture")
    for item in prefix_fixtures:
        f = fixtures[item["fixture_index"]]
        if f["home_id"] != item["home"] or f["away_id"] != item["away"]:
            raise ValueError("pivot fixture orientation differs from native model")
        if pivot not in (f["home_id"], f["away_id"]):
            raise ValueError("declared pivot fixture is not incident to pivot team")
    team_index = {tid: i for i, tid in enumerate(model["team_ids"])}
    if pivot not in team_index or team_id not in team_index:
        raise ValueError("pivot or target team missing from native model")
    assignments = []
    allowed = [[o for o in range(3) if int(fi["mask"]) & (1 << o)]
               for fi in (fixtures[i] for i in indices)]
    for combo in itertools.product(*allowed):
        totals = list(map(int, model["base_packed"]))
        for fi, outcome in zip(indices, combo):
            f = fixtures[fi]
            totals[team_index[f["home_id"]]] += int(f["outcomes"][outcome]["home"])
            totals[team_index[f["away_id"]]] += int(f["outcomes"][outcome]["away"])
        pivot_total = totals[team_index[pivot]]
        if int(model["lower_packed"][team_index[pivot]]) <= pivot_total <= int(model["upper_packed"][team_index[pivot]]):
            assignments.append(tuple(combo))
    listed = {tuple(map(int, item["prefix"])) for item in audit["prefixes"]}
    enumerated = set(assignments)
    if len(listed) != len(audit["prefixes"]):
        raise ValueError("raw prefix audit has duplicate assignments")
    if enumerated != listed:
        raise ValueError(f"prefix coverage mismatch: enumerated {len(enumerated)}, listed {len(listed)}")
    return indices, assignments


def wdl_for_prefix(prefix, prefix_fixtures, model, pivot_id):
    wdl = {"W": 0, "D": 0, "L": 0}
    loss_opponent = None
    for outcome, item in zip(prefix, prefix_fixtures):
        pivot_home = int(item["home"]) == pivot_id
        if outcome == 1:
            result = "D"
        elif (outcome == 2) == pivot_home:
            result = "W"
        else:
            result = "L"
        wdl[result] += 1
        if result == "L":
            loss_opponent = int(item["away"] if pivot_home else item["home"])
    return wdl, loss_opponent


def verify_witness(model, prefix_indices, prefix, witness):
    choices = witness.get("outcome_indices") if isinstance(witness, dict) else None
    if not isinstance(choices, list) or len(choices) != len(model["fixtures"]):
        return False, "missing or wrong-length assignment"
    try:
        original_choices = choices
        choices = [int(x) for x in original_choices]
        if any(float(x) != int(x) for x in original_choices):
            return False, "noninteger outcome index"
    except (TypeError, ValueError, OverflowError):
        return False, "noninteger outcome index"
    if any(o not in (0, 1, 2) for o in choices):
        return False, "outcome index outside W/D/L domain"
    for fi, (f, o) in enumerate(zip(model["fixtures"], choices)):
        if not int(f["mask"]) & (1 << o):
            return False, f"outcome violates native WDL mask at fixture {fi}"
    for fi, expected in zip(prefix_indices, prefix):
        if choices[fi] != int(expected):
            return False, f"assignment does not preserve fixed prefix at fixture {fi}"
    totals = list(map(int, model["base_packed"]))
    team_index = {tid: i for i, tid in enumerate(model["team_ids"])}
    for f, o in zip(model["fixtures"], choices):
        totals[team_index[f["home_id"]]] += int(f["outcomes"][o]["home"])
        totals[team_index[f["away_id"]]] += int(f["outcomes"][o]["away"])
    for t, total in enumerate(totals):
        if not int(model["lower_packed"][t]) <= total <= int(model["upper_packed"][t]):
            return False, f"packed bound violation for team {model['team_ids'][t]}"
    return True, "verified packed WDL assignment"


def run(args):
    started = time.monotonic()
    deadline = started + args.budget
    request = Path(args.request).resolve()
    raw = json.loads(Path(args.raw_audit).read_text())
    audit = raw["cohort5_pivot584_mixture"]
    team_id = int(args.team_id)
    position = int(args.position)
    desc, model = native_model(Path(args.binary).resolve(), request, team_id, position,
                               int(args.cohort_id), deadline)
    request_hash = validate_binding(raw, request, Path(args.reference_result), desc,
                                    model, team_id, position)
    if int(args.cohort_id) != int(audit["region_id"]):
        raise ValueError("requested cohort differs from saved raw audit")
    indices, assignments = enumerate_prefix_coverage(model, audit, int(audit["pivot_id"]))
    from trusted_highs import prescreen
    records = []
    source_by_prefix = {tuple(map(int, item["prefix"])): item for item in audit["prefixes"]}
    for serial, prefix in enumerate(assignments):
        if time.monotonic() >= deadline:
            break
        constrained = dict(model)
        constrained["fixtures"] = [dict(f) for f in model["fixtures"]]
        for fi, outcome in zip(indices, prefix):
            constrained["fixtures"][fi]["mask"] = int(constrained["fixtures"][fi]["mask"]) & (1 << outcome)
        rec = prescreen(constrained, args.per_prefix_limit, deadline,
                        node_limit=args.node_limit)
        witness = rec.get("feasible_witness")
        witness_valid, witness_message = (verify_witness(model, indices, prefix, witness)
                                           if witness is not None else (False, "no optimal witness returned"))
        status = rec.get("status")
        category = "infeasible" if status == 2 else (
            "feasible_verified" if status == 0 and witness_valid else "unknown")
        source = source_by_prefix[prefix]
        wdl, loss_opponent = wdl_for_prefix(prefix, audit["prefix_fixtures"], model,
                                            int(audit["pivot_id"]))
        records.append({"index": serial, "prefix": list(prefix), "wdl": wdl,
                        "loss_opponent_id": loss_opponent,
                        "nominal_prefix_gd_suffix_mass": source["nominal_prefix_gd_suffix_mass"],
                        "cert_numerator_hi": source["cert_numerator_hi"],
                        "solver_status": status, "solver_message": rec.get("message", ""),
                        "solver_runtime_seconds": rec.get("runtime_seconds", 0.0),
                        "category": category, "feasible_witness_present": witness is not None,
                        "feasible_witness_valid": witness_valid,
                        "feasible_witness_check": witness_message,
                        "solver_record": rec})
    if len(records) != len(assignments):
        # A partial audit is explicit: uncalled rows remain unknown and contribute no infeasible mass.
        done = {tuple(x["prefix"]) for x in records}
        for serial, prefix in enumerate(assignments):
            if prefix in done:
                continue
            source = source_by_prefix[prefix]
            wdl, loss_opponent = wdl_for_prefix(prefix, audit["prefix_fixtures"], model,
                                                int(audit["pivot_id"]))
            records.append({"index": serial, "prefix": list(prefix), "wdl": wdl,
                            "loss_opponent_id": loss_opponent,
                            "nominal_prefix_gd_suffix_mass": source["nominal_prefix_gd_suffix_mass"],
                            "cert_numerator_hi": source["cert_numerator_hi"],
                            "solver_status": "not_called_budget", "solver_message": "shared wall budget exhausted",
                            "solver_runtime_seconds": 0.0, "category": "unknown",
                            "feasible_witness_present": False, "feasible_witness_valid": False,
                            "feasible_witness_check": "not called", "solver_record": None})
        records.sort(key=lambda x: x["index"])
    groups = defaultdict(lambda: {"count": 0, "infeasible": 0, "feasible_verified": 0, "unknown": 0,
                                  "nominal_prefix_gd_suffix_mass": 0.0,
                                  "cert_numerator_hi": 0.0,
                                  "infeasible_nominal_mass": 0.0,
                                  "infeasible_cert_numerator_hi": 0.0})
    losses = defaultdict(lambda: {"count": 0, "infeasible": 0, "feasible_verified": 0, "unknown": 0,
                                  "nominal_prefix_gd_suffix_mass": 0.0,
                                  "cert_numerator_hi": 0.0,
                                  "infeasible_nominal_mass": 0.0,
                                  "infeasible_cert_numerator_hi": 0.0})
    for row in records:
        key = "".join(k * row["wdl"][k] for k in ("W", "D", "L"))
        buckets = [groups[key]]
        if row["wdl"] == {"W": 9, "D": 0, "L": 1}:
            buckets.append(losses[str(row["loss_opponent_id"])])
        for bucket in buckets:
            bucket["count"] += 1
            bucket[row["category"] if row["category"] != "feasible_verified" else "feasible_verified"] += 1
            bucket["nominal_prefix_gd_suffix_mass"] += float(row["nominal_prefix_gd_suffix_mass"])
            bucket["cert_numerator_hi"] += float(row["cert_numerator_hi"])
            if row["category"] == "infeasible":
                bucket["infeasible_nominal_mass"] += float(row["nominal_prefix_gd_suffix_mass"])
                bucket["infeasible_cert_numerator_hi"] += float(row["cert_numerator_hi"])
    totals = {"count": len(records), "infeasible": sum(x["category"] == "infeasible" for x in records),
              "feasible_verified": sum(x["category"] == "feasible_verified" for x in records),
              "unknown": sum(x["category"] == "unknown" for x in records),
              "nominal_prefix_gd_suffix_mass": math.fsum(float(x["nominal_prefix_gd_suffix_mass"]) for x in records),
              "cert_numerator_hi": math.fsum(float(x["cert_numerator_hi"]) for x in records),
              "infeasible_nominal_mass": math.fsum(float(x["nominal_prefix_gd_suffix_mass"]) for x in records if x["category"] == "infeasible"),
              "infeasible_cert_numerator_hi": math.fsum(float(x["cert_numerator_hi"]) for x in records if x["category"] == "infeasible")}
    return {"purpose": "audit only: cohort-5 pivot prefix feasibility under native packed WDL constraints",
            "claim_scope": "verified witnesses prove packed-WDL feasibility only; no GD reachability claim",
            "conditional_on_highs_correct": totals["infeasible"] > 0,
            "request_sha256": request_hash, "reference_request_sha256": json.loads(Path(args.reference_result).read_text())["request_sha256"],
            "binary_sha256": sha256(Path(args.binary)), "team_id": team_id, "position": position,
            "cohort_id": int(args.cohort_id), "pivot_id": int(audit["pivot_id"]),
            "above_ids": audit["above_ids"], "selected_ids": audit["selected_ids"],
            "prefix_fixture_indices": indices, "prefix_assignments_enumerated": len(assignments),
            "prefix_coverage_validated": True, "shared_wall_budget_seconds": args.budget,
            "elapsed_seconds": time.monotonic() - started, "totals": totals,
            "by_wdl": dict(groups), "by_loss_opponent": dict(losses), "assignments": records}


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--binary", default=str(DEFAULT_BINARY))
    p.add_argument("--request", default=str(DEFAULT_REQUEST))
    p.add_argument("--raw-audit", default=str(DEFAULT_RAW))
    p.add_argument("--reference-result", default=str(DEFAULT_REFERENCE))
    p.add_argument("--output", default=str(DEFAULT_OUTPUT))
    p.add_argument("--team-id", type=int, default=16)
    p.add_argument("--position", type=int, default=16)
    p.add_argument("--cohort-id", type=int, default=5)
    p.add_argument("--budget", type=float, default=180.0)
    p.add_argument("--per-prefix-limit", type=float, default=.25)
    p.add_argument("--node-limit", type=int)
    args = p.parse_args()
    if not math.isfinite(args.budget) or args.budget <= 0:
        p.error("--budget must be finite and positive")
    if not math.isfinite(args.per_prefix_limit) or args.per_prefix_limit <= 0:
        p.error("--per-prefix-limit must be finite and positive")
    if args.node_limit is not None and args.node_limit < 0:
        p.error("--node-limit must be nonnegative")
    result = run(args)
    Path(args.output).write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    summary = {k: result[k] for k in ("prefix_assignments_enumerated", "elapsed_seconds", "totals", "by_wdl", "by_loss_opponent")}
    print(json.dumps(summary, sort_keys=True))


if __name__ == "__main__":
    main()
