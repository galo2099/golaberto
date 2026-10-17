#!/usr/bin/env python3
"""Prepare or run the serial paired Set 4 GD partition experiment."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
ARTIFACT = HERE.parents[2]
TRUSTED = ARTIFACT / "trusted-highs"
EXPERIMENT = TRUSTED / "gd-set4-split" / "experiment.py"
PRESCREEN = TRUSTED / "prescreen.py"
AUDIT = ARTIFACT / "validation" / "gd-branch-audit" / "pal16-gd-branch-analysis.py"
REQUEST = ARTIFACT / "inputs" / "request.json"
SAVED = ARTIFACT / "inputs" / "target-16-rank-15-separator-saved.json"
SOURCE_BINARY = Path("/tmp/frontier-trusted-highs-runs/target16-trusted-highs-primary-2360/bin/frontier-regimes")
EXPECTED_BINARY_SHA256 = "eb013be81c343762952f6a18abe7f45dda9f37a17dd33064877206c0c4fc60ab"
EXPECTED_REQUEST_SHA256 = "538859da91dc7825ab412a2da44893592f46d83feb1ee70351b49b199372cefa"
EXPECTED_SAVED_SHA256 = "ef80774b597cd60fbde8bffce51f10209631fc8db06254ffa78a04ea92aacb9c"
EXPECTED_PRESCREEN_SHA256 = "4f609a32582dd2ae24d46d74eb4384b6ee21eb2554c5f013d2ff58298fbe87df"
EXPECTED_AUDIT_SHA256 = "6554a84c4b9eed28116d51f2360b2969fcd11842329cc4e4afbdc81e8c2b75a7"
RUN_ROOT = Path("/tmp/frontier-trusted-highs-runs/target16-set4-gd-primary-2370")
VALIDATION = TRUSTED / "gd-set4-split" / "validation"
SEED, WORKERS, BUDGET_MS = 2370, 4, 180_000


def sha(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def sources() -> dict[str, str]:
    files = [Path(__file__).resolve(), EXPERIMENT, PRESCREEN, AUDIT, REQUEST, SAVED]
    return {str(p): sha(p) for p in files}


def validate_frozen() -> dict[str, str]:
    if sha(SOURCE_BINARY) != EXPECTED_BINARY_SHA256:
        raise SystemExit("pinned shared native binary hash mismatch")
    pinned = {REQUEST: EXPECTED_REQUEST_SHA256, SAVED: EXPECTED_SAVED_SHA256,
              PRESCREEN: EXPECTED_PRESCREEN_SHA256, AUDIT: EXPECTED_AUDIT_SHA256}
    for path, expected in pinned.items():
        if sha(path) != expected:
            raise SystemExit(f"frozen input/source hash mismatch: {path}")
    saved = json.loads(SAVED.read_text())
    if len(saved.get("cohorts", [])) != 108 or len(saved.get("root_points", [])) != 486:
        raise SystemExit("frozen saved input inventory changed")
    return sources()


def paths(run_dir: Path, arm: str) -> dict[str, Path]:
    base = run_dir / arm
    return {"filtered": base / "filtered.json", "receipt": base / "receipt.json",
            "response": base / "response.json", "stdout": run_dir / f"{arm}.stdout.txt",
            "stderr": run_dir / f"{arm}.stderr.txt"}


def main(argv=None) -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--run-name", default=RUN_ROOT.name)
    p.add_argument("--execute", action="store_true", help="run control and then treatment")
    p.add_argument("--prepare-only", action="store_true", help="only invoke each arm's prepare-only mode")
    args = p.parse_args(argv)
    if args.prepare_only and not args.execute:
        p.error("--prepare-only requires --execute")
    if args.run_name != RUN_ROOT.name:
        p.error(f"this frozen experiment requires --run-name {RUN_ROOT.name}")
    if RUN_ROOT.exists() or VALIDATION.exists():
        raise SystemExit("refusing to reuse existing raw run or validation directory")
    before = validate_frozen()
    binary = RUN_ROOT / "bin" / "frontier-regimes"
    arms = {arm: paths(RUN_ROOT, arm) for arm in ("control", "treatment")}
    plan = {"schema_version": "set4_gd_paired_plan_v1", "run_name": args.run_name,
            "seed": SEED, "workers": WORKERS, "nominal_budget_ms_per_arm": BUDGET_MS,
            "conditional_on_highs_correct": True, "binary_sha256": EXPECTED_BINARY_SHA256,
            "same_binary_for_both_arms": True, "invocation_order": ["control", "treatment"],
            "prescreen_policy": "each arm runs a fresh 32-row prescreen before native work; all wrapper and prescreen elapsed time reduces that arm's 180000 ms allowance",
            "sources_sha256_at_plan": before,
            "inputs": {"request": str(REQUEST), "saved": str(SAVED)},
            "arms": {arm: {key: str(value) for key, value in paths_.items()} for arm, paths_ in arms.items()},
            "binary_copy": str(binary), "mode": "execute" if args.execute else "dry-plan",
            "prepare_only": args.prepare_only}
    print(json.dumps(plan, indent=2))
    if not args.execute:
        return 0

    RUN_ROOT.mkdir(parents=True)
    binary.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(SOURCE_BINARY, binary)
    if sha(binary) != EXPECTED_BINARY_SHA256:
        raise SystemExit("copied binary hash mismatch")
    (RUN_ROOT / "plan.json").write_text(json.dumps(plan, indent=2) + "\n")
    statuses = []
    for arm in ("control", "treatment"):
        current = validate_frozen()
        if current != before:
            raise SystemExit(f"frozen sources or inputs changed before {arm}; refusing to run")
        if sha(binary) != EXPECTED_BINARY_SHA256:
            raise SystemExit("run-pinned native binary changed")
        out = arms[arm]
        command = [sys.executable, str(EXPERIMENT), "--arm", arm,
                   "--request", str(REQUEST), "--saved", str(SAVED),
                   "--filtered-output", str(out["filtered"]), "--receipt", str(out["receipt"]),
                   "--native-output", str(out["response"]), "--binary", str(binary),
                   "--seed", str(SEED), "--workers", str(WORKERS), "--budget-ms", str(BUDGET_MS)]
        if args.prepare_only:
            command.append("--prepare-only")
        out["stdout"].parent.mkdir(parents=True, exist_ok=False)
        start = time.monotonic()
        with out["stdout"].open("wb") as stdout, out["stderr"].open("wb") as stderr:
            proc = subprocess.run(command, stdout=stdout, stderr=stderr, check=False)
        status = {"arm": arm, "command": command, "returncode": proc.returncode,
                  "wall_ms": round((time.monotonic() - start) * 1000),
                  "source_hashes_before": current,
                  "prescreen_invocation_source_sha256": current[str(PRESCREEN)],
                  "response_exists": out["response"].is_file(),
                  "filtered_sha256": sha(out["filtered"]) if out["filtered"].is_file() else None,
                  "receipt_sha256": sha(out["receipt"]) if out["receipt"].is_file() else None}
        if out["receipt"].is_file():
            receipt = json.loads(out["receipt"].read_text())
            status.update({"native_budget_ms": receipt.get("native_budget_ms"),
                           "wrapper_elapsed_ms_before_native": receipt.get("wrapper_elapsed_ms_before_native"),
                           "prescreen_wall_ms": receipt.get("prescreen", {}).get("wall_ms"),
                           "fresh_infeasible_count": receipt.get("prescreen", {}).get("infeasible_count"),
                           "conditional_on_highs_correct": receipt.get("conditional_on_highs_correct")})
        statuses.append(status)
        if proc.returncode:
            break
    after = sources()
    result = {"plan": plan, "arm_statuses": statuses, "sources_sha256_after": after,
              "source_and_input_hashes_unchanged": after == before,
              "binary_source_sha256_after": sha(SOURCE_BINARY),
              "binary_copy_sha256_after": sha(binary),
              "conditional_on_highs_correct": True}
    (RUN_ROOT / "status.json").write_text(json.dumps(result, indent=2) + "\n")
    if after != before or sha(SOURCE_BINARY) != EXPECTED_BINARY_SHA256 or sha(binary) != EXPECTED_BINARY_SHA256:
        return 1
    if len(statuses) != 2 or any(s["returncode"] != 0 for s in statuses):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
