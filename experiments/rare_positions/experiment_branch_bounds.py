#!/usr/bin/env python3
"""Paired R35 single-cell runs, four Rust workers, fresh setup per seed by default."""
import argparse
import hashlib
import json
from pathlib import Path
import resource
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("request", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--seeds", default="808,1669,1993,1847,1861,2111,2129,2141,2161,2179,2197")
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--amortize", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    baseline = args.baseline or args.binary
    seeds = [args.seeds] if args.amortize else args.seeds.split(",")
    summary = {"workers": 4, "reference_probability": 4e-34, "tolerance": .01,
               "amortized_setup": args.amortize,
               "request_sha256": hashlib.sha256(args.request.read_bytes()).hexdigest(),
               "binary_hashes": {str(p): hashlib.sha256(p.read_bytes()).hexdigest()
                                 for p in [baseline, args.binary]}, "arms": []}
    for round_ in range(args.rounds):
        for seed in seeds:
            arms = [("baseline", baseline, []),
                    ("prior_order", args.binary, ["prior", "4e-34", "0"]),
                    ("joint_skip", args.binary, ["joint", "4e-34", ".01"]),
                    ("strong_skip", args.binary, ["strong", "4e-34", ".01"])]
            if round_ % 2:
                arms.reverse()
            for name, binary, extra in arms:
                command = [str(binary.resolve()), str(args.request.resolve()), "95", "3",
                           "30000", seed, "trained", "adaptive", "4", "tilt",
                           "refine", "500", "200"] + extra
                stem = f"{round_}-{seed}-{name}"
                before = resource.getrusage(resource.RUSAGE_CHILDREN)
                start = time.monotonic()
                with (args.output / f"{stem}.jsonl").open("w") as out, \
                        (args.output / f"{stem}.stderr").open("w") as err:
                    subprocess.run(command, stdout=out, stderr=err, check=True)
                wall = 1000 * (time.monotonic() - start)
                after = resource.getrusage(resource.RUSAGE_CHILDREN)
                cpu = 1000 * (after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime)
                records = [json.loads(s) for s in
                           (args.output / f"{stem}.jsonl").read_text().splitlines()]
                summary["arms"].append({"round": round_, "seed": seed, "name": name,
                                        "command": command, "cpu_ms": cpu,
                                        "process_wall_ms": wall, "setup": records[0],
                                        "results": records[1:]})
        print(f"round {round_ + 1} complete", flush=True)
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    main()
