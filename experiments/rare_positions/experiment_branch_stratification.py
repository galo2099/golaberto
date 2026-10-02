#!/usr/bin/env python3
"""Run the R34 offline cell experiment serially; the Rust example uses four workers."""
import argparse
import hashlib
import json
from pathlib import Path
import resource
import subprocess
import time


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("request", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--team", type=int, default=95)
    parser.add_argument("--rank", type=int, default=3)
    parser.add_argument("--seeds", default="808,1669,1993,1847,1861")
    parser.add_argument("--cohort", choices=["plain", "refined", "validation"], default="validation")
    args = parser.parse_args()
    arms = []
    if args.cohort == "plain":
        arms = [
            ("equal30", 30000, "equal", "rank", "tilt"),
            ("bounds30", 30000, "equal", "bounds", "tilt"),
            ("adaptive30", 30000, "trained", "adaptive", "tilt"),
            ("equal100", 100000, "equal", "rank", "tilt"),
            ("adaptive100", 100000, "trained", "adaptive", "tilt"),
            ("native100", 100000, "equal", "rank", "native"),
        ]
    elif args.cohort == "refined":
        arms = [
            ("equal30", 30000, "equal", "rank", "tilt"),
            ("adaptive30", 30000, "trained", "adaptive", "tilt"),
            ("equal100", 100000, "equal", "rank", "tilt"),
            ("adaptive100", 100000, "trained", "adaptive", "tilt"),
            ("native100", 100000, "equal", "rank", "native"),
        ]
    else:
        arms = [
            ("plain_equal", 30000, "equal", "rank", "tilt"),
            ("refined_adaptive", 30000, "trained", "adaptive", "tilt"),
        ]
    args.output.mkdir(parents=True, exist_ok=False)
    summary = {"workers": 4, "binary_sha256": sha256(args.binary),
               "request_sha256": sha256(args.request), "arms": []}
    for name, draws, allocation, guide, score in arms:
        refined = args.cohort == "refined" or name == "refined_adaptive"
        command = [str(args.binary.resolve()), str(args.request.resolve()),
                   str(args.team), str(args.rank), str(draws), args.seeds,
                   allocation, guide, "4", score, "refine" if refined else "plain",
                   "500" if refined else "2000", "200" if refined else "2000"]
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        start = time.monotonic()
        with (args.output / f"{name}.jsonl").open("w") as out, \
                (args.output / f"{name}.stderr").open("w") as err:
            subprocess.run(command, stdout=out, stderr=err, check=True)
        wall = 1000 * (time.monotonic() - start)
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        cpu = 1000 * (after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime)
        records = [json.loads(line) for line in
                   (args.output / f"{name}.jsonl").read_text().splitlines()]
        results = [r for r in records if r["event"] == "strata_result"]
        setup = next(r for r in records if r["event"] == "strata_setup")
        summary["arms"].append({"name": name, "command": command, "cpu_ms": cpu,
                                "process_wall_ms": wall, "setup": setup, "results": results})
        print(f"{name}: {sum(r['accepted'] for r in results)}/{len(results)} accepted; "
              f"CPU {cpu / len(results):.2f} ms/seed", flush=True)
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    main()
