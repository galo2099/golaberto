#!/usr/bin/env python3
"""Sequential, alternating four-core full-request Rust/Rust comparisons."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import socket
import statistics
import subprocess
import time
import urllib.request

from build_golden_reference import canonical_bytes, normalized_request
from compare_golden_reference import compare

VARIANTS = {
    "legacy": {},
    "dual": {"RUST_ODDS_WITNESS_MODE": "dual"},
    "breadth": {"RUST_ODDS_NEIGHBOR_ORDER": "breadth"},
    "aggregate": {"RUST_ODDS_AGGREGATE_CUTS": "1"},
    "joint": {"RUST_ODDS_WITNESS_MODE": "joint"},
    "dual_breadth": {"RUST_ODDS_WITNESS_MODE": "dual", "RUST_ODDS_NEIGHBOR_ORDER": "breadth"},
    "late_dual": {"RUST_ODDS_REACHABILITY_ALLOCATION": "replace_walk"},
    "aggregate_late_dual": {"RUST_ODDS_AGGREGATE_CUTS": "1", "RUST_ODDS_REACHABILITY_ALLOCATION": "replace_walk"},
    "joint_reallocate": {"RUST_ODDS_WITNESS_MODE": "joint", "RUST_ODDS_REACHABILITY_ALLOCATION": "replace_neighborhood"},
    "reuse": {"RUST_ODDS_WITNESS_MODE": "reuse"},
    "spread": {"RUST_ODDS_NEIGHBOR_ORDER": "spread"},
    "aggregate_breadth": {"RUST_ODDS_AGGREGATE_CUTS": "1", "RUST_ODDS_NEIGHBOR_ORDER": "breadth"},
    "split_walk": {"RUST_ODDS_REACHABILITY_ALLOCATION": "split_walk"},
    "early_breadth": {"RUST_ODDS_AGGREGATE_CUTS": "early", "RUST_ODDS_NEIGHBOR_ORDER": "breadth"},
    "early": {"RUST_ODDS_AGGREGATE_CUTS": "early"},
    "adaptive": {"RUST_ODDS_WITNESS_MODE": "adaptive"},
    "adaptive_early": {"RUST_ODDS_WITNESS_MODE": "adaptive", "RUST_ODDS_AGGREGATE_CUTS": "early"},
    "adaptive_tail": {"RUST_ODDS_WITNESS_MODE": "adaptive", "RUST_ODDS_REACHABILITY_ALLOCATION": "adaptive_tail"},
    "adaptive_tail_early": {"RUST_ODDS_WITNESS_MODE": "adaptive", "RUST_ODDS_REACHABILITY_ALLOCATION": "adaptive_tail", "RUST_ODDS_AGGREGATE_CUTS": "early"},
    "raced_tail": {"RUST_ODDS_WITNESS_MODE": "adaptive", "RUST_ODDS_REACHABILITY_ALLOCATION": "adaptive_tail", "RUST_ODDS_JOINT_ROOT_SWEEP": "1"},
    "raced_tail_early": {"RUST_ODDS_WITNESS_MODE": "adaptive", "RUST_ODDS_REACHABILITY_ALLOCATION": "adaptive_tail", "RUST_ODDS_JOINT_ROOT_SWEEP": "1", "RUST_ODDS_AGGREGATE_CUTS": "early"},
    "budgeted": {"RUST_ODDS_WITNESS_MODE": "adaptive", "RUST_ODDS_AGGREGATE_CUTS": "early", "RUST_ODDS_PROOF_RECYCLE_CREDIT": "legacy"},
    "deferred": {"RUST_ODDS_WITNESS_MODE": "deferred", "RUST_ODDS_AGGREGATE_CUTS": "early", "RUST_ODDS_PROOF_RECYCLE_CREDIT": "legacy"},
    "goals": {"RUST_ODDS_WITNESS_MODE": "deferred", "RUST_ODDS_AGGREGATE_CUTS": "early", "RUST_ODDS_PROOF_RECYCLE_CREDIT": "legacy", "RUST_ODDS_GOAL_COMPLETION": "1"},
    "skip": {"RUST_ODDS_WITNESS_MODE": "skip", "RUST_ODDS_AGGREGATE_CUTS": "early", "RUST_ODDS_PROOF_RECYCLE_CREDIT": "legacy"},
}

# Pin historical variants now that production defaults select goal completion.
# This also makes old commands reproducible against a newly built executable.
LEGACY_FLAGS = {
    "RUST_ODDS_WITNESS_MODE": "legacy",
    "RUST_ODDS_AGGREGATE_CUTS": "0",
    "RUST_ODDS_PROOF_RECYCLE_CREDIT": "0",
    "RUST_ODDS_GOAL_COMPLETION": "0",
    "RUST_ODDS_RANK_PROOF": "0",
    "RUST_ODDS_DISCRETE_CUTS": "0",
}
VARIANTS = {name: dict(LEGACY_FLAGS, **flags) for name, flags in VARIANTS.items()}
VARIANTS["rank_proof"] = {"RUST_ODDS_RANK_PROOF": "1"}
VARIANTS["no_rank_proof"] = {"RUST_ODDS_RANK_PROOF": "0"}
VARIANTS["probe_draws_only"]={"RUST_ODDS_DOMAIN_PROBE_REALLOCATE":"1"}
VARIANTS["probe_local"]={"RUST_ODDS_DOMAIN_PROBES":"12","RUST_ODDS_DOMAIN_PROBE_NODES":"0"}
VARIANTS["probe_joint"]={"RUST_ODDS_DOMAIN_PROBES":"6","RUST_ODDS_DOMAIN_PROBE_NODES":"16"}
VARIANTS["probe_realloc"]={"RUST_ODDS_DOMAIN_PROBES":"6","RUST_ODDS_DOMAIN_PROBE_NODES":"16","RUST_ODDS_DOMAIN_PROBE_REALLOCATE":"1"}
for budget in (24, 96, 384):
    VARIANTS[f"probe_joint_{budget}"] = {
        "RUST_ODDS_DOMAIN_PROBES": str(budget),
        "RUST_ODDS_DOMAIN_PROBE_NODES": "16",
    }
VARIANTS["probe_realloc_96"] = dict(VARIANTS["probe_joint_96"], RUST_ODDS_DOMAIN_PROBE_REALLOCATE="1")
VARIANTS["goal_paths"] = {"RUST_ODDS_GOAL_PATHS": "1"}
VARIANTS["coalition_branching"] = {"RUST_ODDS_COALITION_BRANCHING": "1"}
VARIANTS["paths_coalitions"] = dict(VARIANTS["goal_paths"], **VARIANTS["coalition_branching"])
VARIANTS["defaults"] = {}
VARIANTS["discrete"] = {"RUST_ODDS_DISCRETE_CUTS": "1"}


def states(export):
    return {(int(t), int(r) + 1): ("positive" if e["probability"] > 0 else
            "impossible" if e.get("reachability", "").startswith("impossible") else
            "reachable_zero" if e.get("reachability") in ("witness", "reachable_by_construction") else
            "undecided") for t, ranks in export["rare_position_estimates"].items() for r, e in ranks.items()}


def run_http(binary, request, seed, flags, path, env):
    """Measure one complete request after the native HTTP listener is ready."""
    with socket.socket() as port_probe:
        port_probe.bind(("127.0.0.1", 0))
        port = port_probe.getsockname()[1]
    log_path = path.with_suffix(".log")
    started = time.monotonic()
    with log_path.open("w") as log:
        server = subprocess.Popen([str(binary), "serve", f"127.0.0.1:{port}"],
                                  env=dict(env, **flags, RARE_POSITION_RANDOM_SEED=str(seed)),
                                  stderr=log, stdout=subprocess.DEVNULL)
        try:
            url = f"http://127.0.0.1:{port}"
            while True:
                try:
                    with urllib.request.urlopen(url + "/health", timeout=1) as response:
                        response.read()
                    break
                except (OSError, urllib.error.URLError):
                    if server.poll() is not None or time.monotonic() - started > 10:
                        raise RuntimeError("HTTP estimator did not become ready")
                    time.sleep(0.005)
            startup_ms = (time.monotonic() - started) * 1000
            req = urllib.request.Request(url + "/odds", data=request.read_bytes(),
                                         headers={"Content-Type": "application/json"}, method="POST")
            timer = time.monotonic()
            with urllib.request.urlopen(req, timeout=30) as response:
                encoded = response.read()
            elapsed_ms = (time.monotonic() - timer) * 1000
            path.write_bytes(encoded)
        finally:
            server.terminate()
            server.wait(timeout=5)
    return elapsed_ms, startup_ms, (time.monotonic() - started) * 1000, log_path.read_text()


def run(binary, request, seed, flags, path, env, case, http=False):
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.monotonic()
    if http:
        elapsed, startup_ms, lifetime_ms, stderr = run_http(binary, request, seed, flags, path, env)
    else:
        result = subprocess.run([str(binary), "estimate", str(request), str(path), str(seed), "4"],
                                env=dict(env, **flags), capture_output=True, text=True, check=True)
        elapsed = (time.monotonic() - start) * 1000
        stderr = result.stderr
        startup_ms, lifetime_ms = None, elapsed
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    path.with_suffix(".log").write_text(stderr)
    export = json.loads(path.read_text())
    # Termination can interrupt the optional post-response HTTP log line.
    # Keep complete records; required estimator completion still must exist.
    events = [json.loads(line) for line in stderr.splitlines(keepends=True)
              if '"event":' in line and line.endswith("\n")]
    complete = next(e for e in events if e["event"] == "rust_odds_complete")
    cpu_ms = 1000 * (after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime)
    stages = {e["stage"]: e["elapsed_ms"] for e in events if e["event"] == "rust_odds_stage"}
    diagnostics = [e for e in events if e["event"].startswith("rust_reachability_") or e["event"]=="rust_odds_domain_probes"]
    http_events = [e for e in events if e["event"] == "rust_odds_http_complete"
                   and e.get("request_id") == complete["request_id"]]
    return export, {"timings": complete["timings"], "stages": stages, "cells": complete["cells"],
                    "process_ms": elapsed, "cpu_ms": cpu_ms, "average_cpu_cores": cpu_ms / lifetime_ms,
                    "protocol": "http" if http else "cli", "server_startup_ms": startup_ms,
                    "child_lifetime_ms": lifetime_ms,
                    "server_http_ms": http_events[-1].get("http_total_ms") if http_events else None,
                    "diagnostics": diagnostics, "reference_quality": compare(case, export["rare_position_estimates"])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--reference", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--seeds", default="801,804,808,817,911")
    parser.add_argument("--variants", default="dual,breadth,aggregate,joint,late_dual")
    parser.add_argument("--baseline-variant", default="legacy", choices=VARIANTS,
                        help="Configuration of the frozen Rust baseline")
    parser.add_argument("--cases", default="")
    parser.add_argument("--http", action="store_true", help="Measure HTTP request wall time separately from server startup")
    parser.add_argument("--no-trace", action="store_true", help="Disable experiment-only diagnostics for production timing")
    args = parser.parse_args()
    out = Path(args.output).resolve(); out.mkdir(parents=True, exist_ok=True)
    reference = Path(args.reference).resolve()
    cases = json.loads(reference.read_text())["cases"]
    inputs = sorted((reference.parent / "inputs").glob("*.json"))
    if args.cases:
        inputs = [p for p in inputs if any(s in p.name for s in args.cases.split(","))]
    env = {k: v for k, v in os.environ.items() if not k.startswith(("RARE_POSITION_", "RUST_ODDS_"))}
    env.update(RUST_ODDS_LOG="1", RUST_ODDS_REACHABILITY_TRACE="0" if args.no_trace else "1")
    rows = []
    for variant in args.variants.split(","):
        flags = VARIANTS[variant]
        for request in inputs:
            fingerprint = hashlib.sha256(canonical_bytes(normalized_request(json.loads(request.read_text())))).hexdigest()
            case = next(c for c in cases if c["semantic_input_sha256"] == fingerprint)
            for seed in map(int, args.seeds.split(",")):
                exports, measurements = {}, {}
                for name in (["baseline", "candidate"] if len(rows) % 2 == 0 else ["candidate", "baseline"]):
                    path = out / f"{variant}-{request.stem}-{seed}-{name}.json"
                    exports[name], measurements[name] = run(Path(getattr(args, name)).resolve(), request, seed,
                                                            flags if name == "candidate" else VARIANTS[args.baseline_variant], path, env, case, args.http)
                a, b = states(exports["baseline"]), states(exports["candidate"])
                probabilities_a = {(int(t), int(r) + 1): e["probability"]
                                   for t, ranks in exports["baseline"]["rare_position_estimates"].items()
                                   for r, e in ranks.items()}
                probabilities_b = {(int(t), int(r) + 1): e["probability"]
                                   for t, ranks in exports["candidate"]["rare_position_estimates"].items()
                                   for r, e in ranks.items()}
                delta = lambda test: [{"team": t, "position": r, "before": a[t, r], "after": b[t, r]}
                                      for t, r in a if test(a[t, r], b[t, r])]
                row = {"variant": variant, "flags": flags, "baseline_flags": VARIANTS[args.baseline_variant], "input": request.name, "input_sha256": fingerprint,
                       "seed": seed, "baseline": measurements["baseline"], "candidate": measurements["candidate"],
                       "additional_reachable": delta(lambda x, y: x == "undecided" and y in ("positive", "reachable_zero")),
                       "additional_impossible": delta(lambda x, y: x != "impossible" and y == "impossible"),
                       "lost_reachability": delta(lambda x, y: x in ("positive", "reachable_zero") and y == "undecided"),
                       "gained_nonzero": delta(lambda x, y: x != "positive" and y == "positive"),
                       "lost_nonzero": delta(lambda x, y: x == "positive" and y != "positive"),
                       "proof_conflicts": delta(lambda x, y: x in ("positive", "reachable_zero") and y == "impossible")}
                row["probability_changed_cells"] = sum(probabilities_a[c] != probabilities_b[c] for c in a)
                row["maximum_absolute_probability_delta"] = max(abs(probabilities_a[c] - probabilities_b[c]) for c in a)
                rows.append(row)
                (out / "summary.json").write_text(json.dumps(rows, indent=2) + "\n")
                old = row["baseline"]["timings"]["total_ms"]; new = row["candidate"]["timings"]["total_ms"]
                print(f"{variant} {request.name} seed={seed}: {old:.1f}->{new:.1f} ms "
                      f"({100*(new/old-1):+.1f}%) reachable+{len(row['additional_reachable'])} "
                      f"impossible+{len(row['additional_impossible'])} undecided={row['candidate']['cells']['undecided_zero']} "
                      f"nonzero +{len(row['gained_nonzero'])}/-{len(row['lost_nonzero'])}", flush=True)
    for variant in args.variants.split(","):
        subset = [r for r in rows if r["variant"] == variant]
        changes = [100 * (r["candidate"]["timings"]["total_ms"] / r["baseline"]["timings"]["total_ms"] - 1) for r in subset]
        if subset:
            print(variant, "median paired latency change", round(statistics.median(changes), 2), "%", flush=True)


if __name__ == "__main__":
    main()
