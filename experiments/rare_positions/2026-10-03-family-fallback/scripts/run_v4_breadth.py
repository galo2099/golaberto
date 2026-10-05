#!/usr/bin/env python3
"""Run the authorized V4 equal-grant breadth screen, sequentially."""
import hashlib
import json
import os
import resource
import subprocess
import sys
import time
from pathlib import Path


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(binary, request, root, seed, selector, mode):
    arm = "family-1x" if mode == "family" else "native-retry-1x"
    out = root / selector.replace(":", "-") / arm / f"seed{seed}"
    out.mkdir(parents=True)
    export, stdout, stderr = out / "export.json", out / "stdout.log", out / "stderr.log"
    flags = {
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK": "1",
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE": "native_retry" if mode == "native_retry" else "family",
        "RUST_ODDS_EXPERIMENT_NEIGHBOR_TRANSFER": "native",
        "RUST_ODDS_EXPERIMENT_NEIGHBOR_CELL": selector,
    }
    env = {k: v for k, v in os.environ.items() if not k.startswith(("RUST_ODDS_", "RARE_POSITION_"))}
    env.update(flags)
    env["RUST_ODDS_LOG"] = "1"
    argv = [str(binary), "estimate", str(request), str(export), str(seed), "4"]
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.perf_counter()
    with stdout.open("wb") as so, stderr.open("wb") as se:
        code = subprocess.run(argv, env=env, stdout=so, stderr=se).returncode
    wall = time.perf_counter() - start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    if code:
        raise RuntimeError(f"{selector} {arm} seed{seed} failed: {stderr}")
    events = []
    for line in stderr.read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(event, dict) and isinstance(event.get("event"), str):
            events.append(event)
    fallback = [e for e in events if e.get("event") == "rust_odds_family_fallback"]
    return {
        "selector": selector,
        "mode": mode,
        "arm": arm,
        "seed": seed,
        "workers": 4,
        "binary": str(binary),
        "binary_sha256": sha(binary),
        "request": str(request),
        "request_sha256": sha(request),
        "argv": argv,
        "explicit_environment": {**flags, "RUST_ODDS_LOG": "1"},
        "returncode": code,
        "export": str(export),
        "export_sha256": sha(export),
        "stdout": str(stdout),
        "stdout_sha256": sha(stdout),
        "stderr": str(stderr),
        "stderr_sha256": sha(stderr),
        "wall_seconds": wall,
        "child_user_seconds": after.ru_utime - before.ru_utime,
        "child_system_seconds": after.ru_stime - before.ru_stime,
        "child_cpu_seconds": after.ru_utime - before.ru_utime + after.ru_stime - before.ru_stime,
        "fallback_events": fallback,
    }


def main():
    if len(sys.argv) != 4:
        raise SystemExit("usage: run_v4_breadth.py V4_BINARY REQUEST V3_LOG_ROOT")
    binary, request, _v3 = (Path(value).resolve(strict=True) for value in sys.argv[1:])
    root = Path(__file__).resolve().parents[1] / "logs" / "v4-breadth"
    root.mkdir(parents=True, exist_ok=False)
    records = []
    for selector in ("16:14", "16:15"):
        for seed in (808, 1669, 1993, 2281, 2293):
            for mode in ("family", "native_retry"):
                record = run(binary, request, root, seed, selector, mode)
                records.append(record)
                path = root / selector.replace(":", "-") / record["arm"] / f"seed{seed}" / "record.json"
                path.write_text(json.dumps(record, indent=2) + "\n")
    (root / "summary.json").write_text(json.dumps({
        "selectors": ["16:14", "16:15"],
        "seeds": [808, 1669, 1993, 2281, 2293],
        "modes": ["family-1x", "native-retry-1x"],
        "serialized_processes": True,
        "workers": 4,
        "records": records,
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
