"""Run V6 grant-order, cap-only, joint family arms and exact parity controls."""
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
        "arm": arm, "seed": seed, "workers": 4,
        "binary": str(binary), "binary_sha256": sha(binary),
        "request": str(request), "request_sha256": sha(request),
        "argv": argv, "explicit_environment": {**flags, "RUST_ODDS_LOG": "1"},
        "export": str(export), "export_sha256": sha(export),
        "stdout": str(stdout), "stdout_sha256": sha(stdout),
        "stderr": str(stderr), "stderr_sha256": sha(stderr),
        "wall_seconds": wall,
        "child_user_seconds": after.ru_utime - before.ru_utime,
        "child_system_seconds": after.ru_stime - before.ru_stime,
        "child_cpu_seconds": after.ru_utime - before.ru_utime + after.ru_stime - before.ru_stime,
        "fallback_events": [e for e in events if e.get("event") == "rust_odds_family_fallback"],
        "order_events": [e for e in events if e.get("event") == "rust_odds_family_fallback_order"],
        "order_declines": [e for e in events if e.get("event") == "rust_odds_family_fallback_order_declined"],
        "conflict_events": [e for e in events if e.get("event") == "rust_odds_family_fallback_scope_conflict"],
        "unsupported_events": [e for e in events if e.get("event") == "rust_odds_family_fallback_skip"],
        "confirmation_summaries": [e for e in events if e.get("event") == "rust_odds_rare_tail_confirm_more_summary"],
    }


def main():
    if len(sys.argv) != 6:
        raise SystemExit("usage: run_v6_screen.py V6_BINARY CURRENT_RUST_BINARY REQUEST V3_LOG_ROOT V5_FINAL_LOG_ROOT")
    binary, current, request, v3_root, v5_root = (Path(x).resolve(strict=True) for x in sys.argv[1:])
    root = Path(__file__).resolve().parents[1] / "logs" / "v6-screen"
    root.mkdir(parents=True, exist_ok=False)
    records = []
    arms = (
        ("grant-order-cap5000", {"RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_ORDER": "grant", "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP": "5000"}),
        ("native-order-cap3000", {"RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP": "3000"}),
        ("grant-order-cap3000", {"RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_ORDER": "grant", "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP": "3000"}),
    )
    for seed in (808, 1669, 1993, 2281, 2293):
        for arm, extra in arms:
            flags = {
                "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK": "1",
                "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE": "all",
                "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE": "family",
                **extra,
            }
            records.append(run(binary, request, root, seed, arm, flags))
    for seed in (808, 2293):
        flags = {
            "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK": "1",
            "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE": "all",
            "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE": "family",
            "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP": "5000",
        }
        row = run(binary, request, root, seed, "native-order-v5-parity", flags)
        expected = v5_root / "all-family-1x" / f"seed{seed}" / "export.json"
        if (root / row["arm"] / f"seed{seed}" / "export.json").read_bytes() != expected.read_bytes():
            raise RuntimeError(f"V6 native ordering differs from corrected V5 seed {seed}")
        row["parity_comparator"] = str(expected)
        row["parity_comparator_sha256"] = sha(expected)
        records.append(row)
    row = run(binary, request, root, 808, "flag-off", {})
    expected = v3_root / "seed808" / "current-rust.json"
    if (root / row["arm"] / "seed808" / "export.json").read_bytes() != expected.read_bytes():
        raise RuntimeError("V6 flag-off export differs from current Rust seed808")
    row["parity_comparator"] = str(expected)
    row["parity_comparator_sha256"] = sha(expected)
    records.append(row)
    for row in records:
        (root / row["arm"] / f"seed{row['seed']}" / "record.json").write_text(json.dumps(row, indent=2) + "\n")
    comparators = {}
    for seed in (808, 1669, 1993, 2281, 2293):
        v3dir = v3_root / f"seed{seed}"
        for label, path in (("baseline", v3dir / "current-rust.json"), ("v4_selected", v3dir / "r58-fallback.json")):
            comparators.setdefault(label, []).append({"seed": seed, "path": str(path), "sha256": sha(path)})
    (root / "summary.json").write_text(json.dumps({
        "source_root": str(binary.parents[2]), "binary": str(binary), "binary_sha256": sha(binary),
        "seeds": [808, 1669, 1993, 2281, 2293], "workers": 4, "serialized_processes": True,
        "comparators": comparators, "records": records,
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
