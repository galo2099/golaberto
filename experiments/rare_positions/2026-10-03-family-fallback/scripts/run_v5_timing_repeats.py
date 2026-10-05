"""Reproduce the authorized V5 alternating family-vs-baseline timing pairs."""
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


def main():
    if len(sys.argv) != 4:
        raise SystemExit("usage: run_v5_timing_repeats.py V5_BINARY CURRENT_RUST_BINARY REQUEST")
    v5, current, request = (Path(x).resolve(strict=True) for x in sys.argv[1:])
    root = Path(__file__).resolve().parents[1] / "logs" / "v5-timing"
    root.mkdir(parents=True, exist_ok=False)
    records = []
    for seed in (808, 2293):
        for rep in (1, 2, 3):
            order = ("baseline", "family") if rep % 2 else ("family", "baseline")
            for arm in order:
                binary = current if arm == "baseline" else v5
                flags = {} if arm == "baseline" else {
                    "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK": "1",
                    "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE": "all",
                    "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE": "family",
                }
                out = root / f"seed{seed}" / f"{arm}-rep{rep}"
                out.mkdir(parents=True)
                export, stdout, stderr = out / "export.json", out / "stdout.log", out / "stderr.log"
                env = {k: v for k, v in os.environ.items() if not k.startswith(("RUST_ODDS_", "RARE_POSITION_"))}
                env.update(flags)
                env["RUST_ODDS_LOG"] = "1"
                argv = [str(binary), "estimate", str(request), str(export), str(seed), "4"]
                before = resource.getrusage(resource.RUSAGE_CHILDREN)
                started = time.perf_counter()
                with stdout.open("wb") as so, stderr.open("wb") as se:
                    subprocess.run(argv, env=env, stdout=so, stderr=se, check=True)
                wall = time.perf_counter() - started
                after = resource.getrusage(resource.RUSAGE_CHILDREN)
                expected = root.parent / "v5-screen" / ("all-family-1x" if arm == "family" else "flag-off" if seed == 808 else "baseline-reference") / f"seed{seed}" / "export.json"
                if arm == "baseline":
                    expected = root.parent / "v3-cohort" / f"seed{seed}" / "current-rust.json"
                if export.read_bytes() != expected.read_bytes():
                    raise RuntimeError(f"nonidentical repeated export: {seed}/{arm}/rep{rep}")
                events = []
                for line in stderr.read_text(errors="replace").splitlines():
                    try:
                        event = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    if isinstance(event, dict) and event.get("event"):
                        events.append(event)
                stages = {e.get("stage"): e.get("elapsed_ms") for e in events if e.get("event") == "rust_odds_stage"}
                fallback = [e.get("fallback") for e in events if e.get("event") == "rust_odds_family_fallback"]
                records.append({
                    "seed": seed,
                    "rep": rep,
                    "arm": arm,
                    "order": list(order),
                    "binary": str(binary),
                    "binary_sha256": sha(binary),
                    "request": str(request),
                    "request_sha256": sha(request),
                    "argv": argv,
                    "environment": {**flags, "RUST_ODDS_LOG": "1"},
                    "export": str(export),
                    "export_sha256": sha(export),
                    "matches_saved_exact_export": True,
                    "stdout": str(stdout),
                    "stdout_sha256": sha(stdout),
                    "stderr": str(stderr),
                    "stderr_sha256": sha(stderr),
                    "wall_seconds": wall,
                    "child_user_seconds": after.ru_utime - before.ru_utime,
                    "child_system_seconds": after.ru_stime - before.ru_stime,
                    "child_cpu_seconds": after.ru_utime - before.ru_utime + after.ru_stime - before.ru_stime,
                    "stages_ms": stages,
                    "fallback_events": fallback,
                })
    (root / "summary.json").write_text(json.dumps({
        "seeds": [808, 2293],
        "paired_repetitions": 3,
        "alternating_order": True,
        "serialized_processes": True,
        "workers": 4,
        "all_exports_match_saved_v5_or_current_exact_exports": True,
        "records": records,
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
