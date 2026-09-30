#!/usr/bin/env python3
"""Paired full-request Go/Rust comparison. Build both release binaries first.

Each pair runs sequentially; this avoids competing for the four-core budget.
Raw matrices and logs are kept in the user-selected output directory.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import time

from compare_golden_reference import compare
from build_golden_reference import canonical_bytes, normalized_request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--go", required=True, help="go test -c binary")
    parser.add_argument("--rust", required=True, help="cargo release binary")
    parser.add_argument("--reference", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--seeds", default="801,804,808,817,911")
    parser.add_argument("--cases", default="", help="Optional comma-separated input filename fragments")
    args = parser.parse_args()
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    reference_path = Path(args.reference).resolve()
    reference = json.loads(reference_path.read_text())
    inputs = sorted((reference_path.parent / "inputs").glob("*.json"))
    if args.cases:
        inputs = [p for p in inputs if any(s in p.name for s in args.cases.split(","))]
    env = {k: v for k, v in os.environ.items() if not k.startswith("RARE_POSITION_")}
    env.update(GOMAXPROCS="4", RARE_POSITION_MATCHED_POINT_POOL="1", RARE_POSITION_IMPORTANCE_SAMPLING="0",
               RUST_ODDS_LOG="0", RUST_ODDS_PROFILE="0")
    rows = []
    for request in inputs:
        data = json.loads(request.read_text())
        fingerprint = hashlib.sha256(canonical_bytes(normalized_request(data))).hexdigest()
        case = next(c for c in reference["cases"] if c["semantic_input_sha256"] == fingerprint)
        for seed in args.seeds.split(","):
            matrices, durations, scores = {}, {}, {}
            for language in (["go", "rust"] if len(rows) % 2 == 0 else ["rust", "go"]):
                name = f"{request.stem}-{seed}-{language}"
                dest = output / f"{name}.json"
                run_env = dict(env, RARE_POSITION_RANDOM_SEED=seed)
                if language == "go":
                    run_env.update(RUST_ODDS_ORACLE_REQUEST=str(request), RUST_ODDS_ORACLE_OUTPUT=str(dest))
                    command = [str(Path(args.go).resolve()), "-test.run=^TestRustEstimatorFullBaseline$", "-test.count=1", "-test.v"]
                else:
                    command = [str(Path(args.rust).resolve()), "estimate", str(request), str(dest), seed, "4"]
                start = time.monotonic()
                run = subprocess.run(command, env=run_env, capture_output=True, text=True, check=True)
                process_ms = (time.monotonic() - start) * 1000
                log = run.stdout + run.stderr
                (output / f"{name}.log").write_text(log)
                if language == "go":
                    durations[language] = float(re.search(r"rust-comparison-go elapsed_ms=([\d.]+)", log)[1])
                else:
                    timing = next(json.loads(line) for line in run.stderr.splitlines() if line.startswith('{"setup_ms"'))
                    durations[language] = timing["total_ms"]
                export = json.loads(dest.read_text())
                matrices[language] = export["rare_position_estimates"]
                scores[language] = {**compare(case, matrices[language]), "process_ms": process_ms}
            gained, lost, ratios = [], [], []
            for t, ranks in matrices["go"].items():
                for r, a in ranks.items():
                    x, y = a["probability"], matrices["rust"][t][r]["probability"]
                    if x == 0 < y:
                        gained.append({"team": int(t), "position": int(r) + 1, "p": y})
                    if y == 0 < x:
                        lost.append({"team": int(t), "position": int(r) + 1, "p": x})
                    if x > 0 and y > 0:
                        ratios.append(max(x / y, y / x))
            row = {"request": request.name, "seed": int(seed), "duration_ms": durations,
                   "speedup": durations["go"] / durations["rust"], "gained": gained, "lost": lost,
                   "largest_paired_ratio": max(ratios, default=1), "scores": scores}
            rows.append(row)
            (output / "summary.json").write_text(json.dumps(rows, indent=2) + "\n")
            print(f"{request.name} seed={seed} Go={durations['go']:.1f} ms Rust={durations['rust']:.1f} ms "
                  f"speedup={row['speedup']:.2f}x gain={len(gained)} loss={len(lost)}", flush=True)
    if rows:
        print(f"median speedup={statistics.median(r['speedup'] for r in rows):.2f}x", flush=True)


if __name__ == "__main__":
    main()
