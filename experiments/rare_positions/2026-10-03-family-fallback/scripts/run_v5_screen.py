"""Run V5 parity and the authorized combined-budget 1x screens serially."""
import hashlib
import json
import os
import resource
import subprocess
import sys
import time
from pathlib import Path


def sha(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def run(binary, request, root, seed, arm, flags):
    out = root / arm / f"seed{seed}"
    out.mkdir(parents=True)
    export, stdout, stderr = out / "export.json", out / "stdout.log", out / "stderr.log"
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
    if code or not export.exists():
        raise RuntimeError(f"{arm}/seed{seed} failed; inspect {stderr}")
    events = []
    for line in stderr.read_text(errors="replace").splitlines():
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and isinstance(value.get("event"), str):
            events.append(value)
    return {
        "arm": arm,
        "seed": seed,
        "workers": 4,
        "binary": str(binary),
        "binary_sha256": sha(binary),
        "request": str(request),
        "request_sha256": sha(request),
        "argv": argv,
        "explicit_environment": {**flags, "RUST_ODDS_LOG": "1"},
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
        "fallback_events": [e for e in events if e.get("event") == "rust_odds_family_fallback"],
        "conflict_events": [e for e in events if e.get("event") == "rust_odds_family_fallback_scope_conflict"],
        "unsupported_events": [e for e in events if e.get("event") == "rust_odds_family_fallback_skip"],
        "confirmation_summaries": [e for e in events if e.get("event") == "rust_odds_rare_tail_confirm_more_summary"],
    }


def main():
    if len(sys.argv) != 5:
        raise SystemExit("usage: run_v5_screen.py V5_BINARY CURRENT_RUST_BINARY REQUEST V3_LOG_ROOT")
    binary, current, request, v3_root = (Path(x).resolve(strict=True) for x in sys.argv[1:])
    root = Path(__file__).resolve().parents[1] / "logs" / "v5-screen"
    root.mkdir(parents=True, exist_ok=False)
    records = []
    selector = {
        "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK": "1",
        "RUST_ODDS_EXPERIMENT_NEIGHBOR_TRANSFER": "native",
        "RUST_ODDS_EXPERIMENT_NEIGHBOR_CELL": "17:12",
    }
    for seed in (808, 1669, 1993, 2281, 2293):
        for arm, mode in (("all-family-1x", "family"), ("all-native-retry-1x", "native_retry")):
            flags = {
                "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK": "1",
                "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE": "all",
                "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE": mode,
            }
            records.append(run(binary, request, root, seed, arm, flags))
    for seed in (808, 2293):
        records.append(run(binary, request, root, seed, "selected-family-1x", selector))
    records.append(run(binary, request, root, 808, "flag-off", {}))
    for row in records:
        path = root / row["arm"] / f"seed{row['seed']}" / "record.json"
        path.write_text(json.dumps(row, indent=2) + "\n")
        if row["arm"] == "selected-family-1x":
            old = v3_root / f"seed{row['seed']}" / "r58-fallback.json"
            if (root / row["arm"] / f"seed{row['seed']}" / "export.json").read_bytes() != old.read_bytes():
                raise RuntimeError(f"V5 selected default export differs from V4 for seed {row['seed']}")
        if row["arm"] == "flag-off":
            base = v3_root / "seed808" / "current-rust.json"
            if (root / row["arm"] / "seed808" / "export.json").read_bytes() != base.read_bytes():
                raise RuntimeError("V5 flag-off export differs from current Rust seed808")
    v4_comparators = []
    baseline_comparators = []
    for seed in (808, 1669, 1993, 2281, 2293):
        v3dir = v3_root / f"seed{seed}"
        v4 = v3dir / "r58-fallback.json"
        baseline = v3dir / "current-rust.json"
        v4_comparators.append({"seed": seed, "path": str(v4), "sha256": sha(v4)})
        baseline_comparators.append({"seed": seed, "path": str(baseline), "sha256": sha(baseline)})
    (root / "summary.json").write_text(json.dumps({
        "seeds": [808, 1669, 1993, 2281, 2293],
        "workers": 4,
        "serialized_processes": True,
        "v3_baseline_current_rust_comparators": baseline_comparators,
        "v4_selected_comparators": v4_comparators,
        "selected_v4_exact_parity_seeds": [808, 2293],
        "flag_off_current_rust_exact_parity_seed": 808,
        "records": records,
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
