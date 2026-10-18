#!/usr/bin/env python3
"""Paired certificate-on/off benchmark for the frozen odds CLI binary.

Raw requests, responses, logs, and summaries are written outside the repository.
The script is deliberately inert unless --run is supplied.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import resource
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = Path("/workspace/rare-odds-production-2026-10-18/runs.json")
BASELINE = Path("/workspace/rare-odds-production-2026-10-18")
DEFAULT_OUT = Path("/workspace/rare-odds-certificates-2026-10-18")
FAMILY_CELLS = 9048
WORKERS = 4
TIMEOUT = 90


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def source_sha() -> str:
    h = hashlib.sha256()
    files = [ROOT / "odds-rust/Cargo.toml", ROOT / "odds-rust/Cargo.lock"]
    files += sorted((ROOT / "odds-rust/src").rglob("*.rs"))
    for path in files:
        if path.is_file():
            h.update(path.relative_to(ROOT).as_posix().encode())
            h.update(b"\0")
            h.update(path.read_bytes())
            h.update(b"\0")
    return h.hexdigest()


def qualify(lower: float, upper: float) -> bool:
    if not (math.isfinite(lower) and math.isfinite(upper) and 0 < lower <= upper <= 1):
        return False
    log_lo, log_hi = math.log10(lower), math.log10(upper)
    guard = 1.0e-12 * max(abs(log_lo), abs(log_hi), 1.0)
    mid_log_hi = math.nextafter((log_lo + log_hi) * 0.5 + guard, math.inf)
    half_log_hi = math.nextafter((log_hi - log_lo) * 0.5 + guard, math.inf)
    scale_lo = max(-mid_log_hi, 1.0)
    relative_bound_lo = max(math.nextafter(0.05 * scale_lo - guard, -math.inf), 0.0)
    width_hi = math.nextafter(upper - lower, math.inf)
    return width_hi <= 0.01 - 1e-14 and half_log_hi <= relative_bound_lo


def intersection(bounds: list[dict]) -> tuple[float, float] | None:
    if not bounds:
        return None
    lo, hi = max(float(x["lower"]) for x in bounds), min(float(x["upper"]) for x in bounds)
    return (lo, hi) if lo <= hi else None


def stripped(value):
    if isinstance(value, dict):
        return {k: stripped(v) for k, v in value.items() if k != "confidence"}
    if isinstance(value, list):
        return [stripped(v) for v in value]
    return value


def summarize_arm(records: list[dict], arm: str) -> dict:
    counts = {m: {"covered": 0, "qualified": 0, "width_sum": 0.0}
              for m in ("native_only", "native_plus_matched", "final")}
    states = {"proved_impossible": 0, "conflict": 0, "unavailable": 0, "point_outside": 0}
    source_cells = {"native_mc": 0, "matched_own_points_strata": 0,
                    "conditioned_fast_is_native_bridge": 0}
    timed = [r for r in records if r["arm"] == arm and not r.get("warmup") and r.get("wall_seconds") is not None]
    cells = 0
    for record in records:
        if record["arm"] != arm or record.get("warmup"):
            continue
        payload = record.get("payload", {})
        for ranks in payload.get("rare_position_estimates", {}).values():
            for estimate in ranks.values():
                cert = estimate.get("confidence") or {}
                status = cert.get("status")
                if status == "proved_impossible" or str(estimate.get("reachability", "")).startswith("impossible"):
                    states["proved_impossible"] += 1
                    continue
                cells += 1
                if status == "confidence_conflict" or cert.get("conflicts"):
                    states["conflict"] += 1
                if status in (None, "unavailable"):
                    states["unavailable"] += 1
                if cert.get("reported_estimate_inside") is False:
                    states["point_outside"] += 1
                sources = cert.get("source_bounds", [])
                for method in source_cells:
                    if any(x.get("method") == method for x in sources):
                        source_cells[method] += 1
                native = intersection([x for x in sources if x.get("method") == "native_mc"])
                matched = intersection([x for x in sources if x.get("method") == "matched_own_points_strata"])
                if native and matched:
                    lo, hi = max(native[0], matched[0]), min(native[1], matched[1])
                    native_matched = (lo, hi) if lo <= hi else None
                else:
                    native_matched = native
                final = (float(cert["lower"]), float(cert["upper"])) if "lower" in cert and "upper" in cert else None
                for key, interval in (("native_only", native), ("native_plus_matched", native_matched), ("final", final)):
                    if interval is not None:
                        c = counts[key]
                        c["covered"] += 1
                        qualified = cert.get("interval_qualifies", False) if key == "final" else qualify(*interval)
                        if key == "final" and (cert.get("conflicts") or status == "confidence_conflict"):
                            qualified = False
                        c["qualified"] += int(qualified)
                        c["width_sum"] += max(0.0, interval[1] - interval[0])
    wall = [r["wall_seconds"] for r in timed]
    cpu = [r["child_cpu_seconds"] for r in timed]
    return {"cells": cells, "methods": counts, "source_cells": source_cells, "states": states,
            "timing": {"runs": len(timed), "wall_seconds_total": sum(wall),
                       "wall_seconds_median": sorted(wall)[len(wall) // 2] if wall else None,
                       "child_cpu_seconds_total": sum(cpu)}}


def run_one(binary: Path, request: Path, seed: int, arm: str, stem: str,
            out: Path, dry_run: bool) -> dict:
    response, stdout_path, stderr_path = (out / f"{stem}.{suffix}" for suffix in ("json", "stdout", "stderr"))
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("RUST_ODDS_", "RARE_POSITION_"))}
    env.update({"RUST_ODDS_LOG": "0", "RUST_ODDS_CERTIFICATE_FAMILY_CELLS": str(FAMILY_CELLS),
                "RUST_ODDS_CERTIFICATES": "1" if arm == "on" else "0"})
    command = [str(binary), "estimate", str(request), str(response), str(seed), str(WORKERS)]
    if dry_run:
        return {"command": command, "arm": arm, "input": str(request)}
    before = resource.getrusage(resource.RUSAGE_CHILDREN).ru_utime + resource.getrusage(resource.RUSAGE_CHILDREN).ru_stime
    started = time.perf_counter()
    try:
        with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
            proc = subprocess.run(command, env=env, stdout=stdout, stderr=stderr, timeout=TIMEOUT, check=False)
        error = None if proc.returncode == 0 else f"exit_{proc.returncode}"
    except subprocess.TimeoutExpired:
        proc, error = None, "timeout"
    wall = time.perf_counter() - started
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    cpu = usage.ru_utime + usage.ru_stime - before
    payload = json.loads(response.read_text()) if error is None else None
    return {"arm": arm, "wall_seconds": wall, "child_cpu_seconds": cpu,
            "returncode": proc.returncode if proc else None, "error": error,
            "payload": payload, "response_path": str(response), "stderr_path": str(stderr_path)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True, help="frozen release binary")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUT)
    parser.add_argument("--limit", type=int, help="run only the first N manifest cases")
    parser.add_argument("--run", action="store_true", help="execute warmups and paired measurements")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    out = args.output.resolve()
    if out == ROOT or ROOT in out.parents:
        parser.error("output must be outside the repository")
    cases = json.loads(MANIFEST.read_text())
    if args.limit is not None:
        if args.limit < 1:
            parser.error("--limit must be positive")
        cases = cases[:args.limit]
    for case in cases:
        path = BASELINE / "inputs" / case["input"]
        if not path.is_file() or sha256(path) != case["input_sha256"]:
            parser.error(f"input missing or hash mismatch: {path}")
        case["_path"] = path
    if not args.run:
        print(json.dumps({"planned_cases": len(cases), "warmups": ["off", "on"],
                          "pair_order": "off/on, on/off alternating", "workers": WORKERS,
                          "family_cells": FAMILY_CELLS, "timeout_seconds": TIMEOUT,
                          "binary_sha256": sha256(binary), "source_sha256": source_sha(),
                          "output": str(out)}, indent=2))
        return 0
    out.mkdir(parents=True, exist_ok=True)
    metadata = {"binary": str(binary), "binary_sha256": sha256(binary), "source_sha256": source_sha(),
                "manifest_sha256": sha256(MANIFEST), "workers": WORKERS, "family_cells": FAMILY_CELLS,
                "timeout_seconds": TIMEOUT, "cases": [
                    {"input": c["input"], "input_sha256": c["input_sha256"], "seed": c["seed"], "family": c["family"]}
                    for c in cases]}
    records = []
    for arm in ("off", "on"):
        result = run_one(binary, cases[0]["_path"], cases[0]["seed"], arm,
                         f"warmup-{arm}", out, False)
        if result["error"]:
            raise RuntimeError(f"warmup {arm} failed: {result['error']}")
        result.update({"warmup": True, "case": 0})
        records.append(result)
    for i, case in enumerate(cases):
        arms = ("off", "on") if i % 2 == 0 else ("on", "off")
        for arm in arms:
            stem = f"{i:02d}-{case['input'].removesuffix('.json')}-{case['seed']}-{arm}"
            result = run_one(binary, case["_path"], case["seed"], arm, stem, out, False)
            result.update({"warmup": False, "case": i, "input": case["input"], "family": case["family"], "seed": case["seed"]})
            records.append(result)
    if sha256(binary) != metadata["binary_sha256"] or source_sha() != metadata["source_sha256"]:
        raise RuntimeError("binary or Rust sources changed during benchmark")
    for case in cases:
        if sha256(case["_path"]) != case["input_sha256"]:
            raise RuntimeError(f"input changed during benchmark: {case['input']}")
    comparisons = []
    for i in range(len(cases)):
        pair = [r for r in records if r.get("case") == i and not r.get("warmup")]
        by_arm = {r["arm"]: r for r in pair}
        if set(by_arm) == {"off", "on"} and all(by_arm[a].get("payload") is not None for a in by_arm):
            off_clean, on_clean = stripped(by_arm["off"]["payload"]), stripped(by_arm["on"]["payload"])
            baseline_path = BASELINE / f"{i:02d}-{cases[i]['input'].removesuffix('.json')}-{cases[i]['seed']}.json"
            baseline_equal = None
            if baseline_path.is_file():
                baseline_equal = off_clean == json.loads(baseline_path.read_text())
            comparisons.append({"case": i, "legacy_fields_equal": off_clean == on_clean,
                                "off_matches_saved_baseline": baseline_equal})
    summary = {"metadata": metadata, "runs": records,
               "arms": {a: summarize_arm(records, a) for a in ("off", "on")},
               "paired_legacy_output_equal": sum(x["legacy_fields_equal"] for x in comparisons),
               "off_matches_saved_baseline": sum(x["off_matches_saved_baseline"] is True for x in comparisons),
               "paired_comparisons": comparisons}
    off_by_case = {r["case"]: r for r in records if not r.get("warmup") and r["arm"] == "off"}
    on_by_case = {r["case"]: r for r in records if not r.get("warmup") and r["arm"] == "on"}
    paired = [(off_by_case[i]["wall_seconds"], on_by_case[i]["wall_seconds"],
               off_by_case[i]["child_cpu_seconds"], on_by_case[i]["child_cpu_seconds"])
              for i in range(len(cases)) if i in off_by_case and i in on_by_case
              and off_by_case[i].get("wall_seconds") is not None and on_by_case[i].get("wall_seconds") is not None]
    deltas = [(on - off) / off * 100.0 for off, on, _, _ in paired if off > 0]
    cpu_deltas = [(on - off) / off * 100.0 for _, _, off, on in paired if off > 0]
    summary["paired_timing"] = {"completed_pairs": len(paired),
                                "median_wall_change_percent": sorted(deltas)[len(deltas) // 2] if deltas else None,
                                "mean_wall_change_percent": sum(deltas) / len(deltas) if deltas else None,
                                "median_child_cpu_change_percent": sorted(cpu_deltas)[len(cpu_deltas) // 2] if cpu_deltas else None}
    (out / "summary.json").write_text(json.dumps(summary, indent=2, allow_nan=False) + "\n")
    lines = ["# Certificate benchmark", "", f"Binary SHA-256: `{metadata['binary_sha256']}`",
             f"Source SHA-256: `{metadata['source_sha256']}`", "",
             f"Paired legacy output identical (confidence removed): {summary['paired_legacy_output_equal']}/{len(comparisons)}.", ""]
    lines.insert(5, f"Off-arm legacy output matches saved baseline: {summary['off_matches_saved_baseline']}/{sum(x['off_matches_saved_baseline'] is not None for x in comparisons)}.")
    lines.insert(6, f"Paired median wall-time change: {summary['paired_timing']['median_wall_change_percent']}%; median child-CPU change: {summary['paired_timing']['median_child_cpu_change_percent']}%.")
    for arm in ("off", "on"):
        lines.append(f"## Certificates {arm}")
        lines.append("")
        data = summary["arms"][arm]
        lines.append(f"Cells: {data['cells']}; statuses: {json.dumps(data['states'], sort_keys=True)}.")
        lines.append(f"Timing: {data['timing']['wall_seconds_total']:.3f}s wall, {data['timing']['child_cpu_seconds_total']:.3f}s child CPU total.")
        lines.append(f"Source coverage: {json.dumps(data['source_cells'], sort_keys=True)}.")
        lines.append("")
        for method, values in data["methods"].items():
            average = values["width_sum"] / values["covered"] if values["covered"] else None
            lines.append(f"- {method}: coverage {values['covered']}, qualified {values['qualified']}, mean width {average}.")
        lines.append("")
    (out / "summary.md").write_text("\n".join(lines))
    if sha256(binary) != metadata["binary_sha256"]:
        raise RuntimeError("binary changed during benchmark")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
