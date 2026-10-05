#!/usr/bin/env python3
"""Check flag-off identity and active worker/logging invariance for R54."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

PRODUCTION_SHA256 = "010a91e9d5f9432f4ddfdd165d13df3bfefed6de502732aa0e3f99d9cc6fbd46"
REQUEST_SHA256 = "2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625"
TRANSFER_FLAG = "RUST_ODDS_EXPERIMENT_NEIGHBOR_TRANSFER"
CELL_FLAG = "RUST_ODDS_EXPERIMENT_NEIGHBOR_CELL"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def initial_mc(export: dict) -> dict:
    return {team: {rank: {k: row.get(k) for k in ("samples", "hits")}
                   for rank, row in sorted(ranks.items())}
            for team, ranks in sorted((export.get("rare_position_estimates") or {}).items())}


def env_for(logging: bool, flags: dict[str, str]) -> dict[str, str]:
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("RUST_ODDS_", "RARE_POSITION_"))}
    env["RUST_ODDS_LOG"] = "1" if logging else "0"
    env.update(flags)
    return env


def launch(binary: Path, request: Path, root: Path, name: str, seed: int,
           workers: int, logging: bool, flags: dict[str, str]) -> dict:
    export = root / f"{name}.json"
    stdout_path = root / f"{name}.stdout.log"
    stderr_path = root / f"{name}.stderr.log"
    argv = [str(binary), "estimate", str(request), str(export), str(seed), str(workers)]
    start = time.perf_counter()
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        result = subprocess.run(argv, env=env_for(logging, flags), stdout=stdout, stderr=stderr, check=False)
    elapsed = time.perf_counter() - start
    record = {
        "name": name, "argv": argv, "flags": flags, "logging": logging,
        "workers": workers, "seed": seed, "returncode": result.returncode,
        "wall_seconds": elapsed, "export": export.name if export.exists() else None,
        "export_sha256": sha256(export) if export.exists() else None,
        "stdout_log": stdout_path.name, "stderr_log": stderr_path.name,
        "binary_sha256": sha256(binary),
    }
    (root / f"{name}.command.json").write_text(json.dumps(record, indent=2) + "\n")
    if result.returncode != 0 or not export.exists():
        raise RuntimeError(f"invariant run {name} failed; inspect {stderr_path}")
    return record


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("production_binary", type=Path)
    ap.add_argument("candidate_binary", type=Path)
    ap.add_argument("request", type=Path)
    ap.add_argument("--output-dir", type=Path, required=True)
    ap.add_argument("--seed", type=int, default=808)
    ap.add_argument("--cell-selector", default="17:12", help="candidate selector TEAM_ID:RANK_1_BASED")
    ap.add_argument("--execute", action="store_true", help="required before launching estimate jobs")
    args = ap.parse_args()
    if not args.execute:
        ap.error("no jobs launched: pass --execute only after root releases the serialized heavy-run slot")
    production = args.production_binary.resolve(strict=True)
    candidate = args.candidate_binary.resolve(strict=True)
    request = args.request.resolve(strict=True)
    if sha256(production) != PRODUCTION_SHA256:
        ap.error("production binary SHA256 does not match frozen baseline")
    if sha256(request) != REQUEST_SHA256:
        ap.error("request SHA256 does not match saved group 16498 input")
    out = args.output_dir.resolve(); out.mkdir(parents=True, exist_ok=True)
    matrix = [
        ("production-baseline", production, {}, 4, True),
        ("candidate-flagoff", candidate, {}, 4, True),
        ("candidate-active-w4-logon", candidate, {TRANSFER_FLAG: "both", CELL_FLAG: args.cell_selector}, 4, True),
        ("candidate-active-w1-logon", candidate, {TRANSFER_FLAG: "both", CELL_FLAG: args.cell_selector}, 1, True),
        ("candidate-active-w4-logoff", candidate, {TRANSFER_FLAG: "both", CELL_FLAG: args.cell_selector}, 4, False),
        ("candidate-active-w1-logoff", candidate, {TRANSFER_FLAG: "both", CELL_FLAG: args.cell_selector}, 1, False),
    ]
    runs = []
    for name, binary, flags, workers, logging in matrix:
        runs.append(launch(binary, request, out, name, args.seed, workers, logging, flags))
    exports = {r["name"]: json.loads((out / r["export"]).read_text()) for r in runs}
    baseline = exports["production-baseline"]
    flagoff = exports["candidate-flagoff"]
    active_names = [r[0] for r in matrix[2:]]
    baseline_identity = baseline == flagoff
    active_exports = [exports[n] for n in active_names]
    active_identity = all(x == active_exports[0] for x in active_exports[1:])
    mc_exports = [baseline, flagoff, *active_exports]
    mc_hashes = [hashlib.sha256(canonical(initial_mc(x))).hexdigest() for x in mc_exports]
    report = {
        "request_sha256": sha256(request), "production_sha256": sha256(production),
        "candidate_sha256": sha256(candidate), "seed": args.seed, "cell_selector": args.cell_selector,
        "matrix": runs, "flagoff_export_identical_to_production": baseline_identity,
        "active_export_identical_across_workers_and_logging": active_identity,
        "initial_mc_hashes": mc_hashes, "initial_mc_identical_all_arms": len(set(mc_hashes)) == 1,
    }
    (out / "invariants.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k != "matrix"}, indent=2))
    return 0 if baseline_identity and active_identity and len(set(mc_hashes)) == 1 else 1

if __name__ == "__main__":
    raise SystemExit(main())
