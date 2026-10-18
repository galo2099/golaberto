#!/usr/bin/env python3
"""Collect rare-tail phase timings for the frozen release binary.

Execution requires --run. Requests, responses, logs, and summaries stay private.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
ARCHIVE = Path("/workspace/rare-odds-production-2026-10-18")
MANIFEST = ARCHIVE / "runs.json"
OUTPUT = Path("/workspace/rare-odds-heuristic-breakdown-2026-10-18")
WORKERS = 4
TIMEOUT_SECONDS = 90


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def source_sha256() -> str:
    paths = [ROOT / "odds-rust/Cargo.toml", ROOT / "odds-rust/Cargo.lock"]
    paths.extend(sorted((ROOT / "odds-rust/src").rglob("*.rs")))
    digest = hashlib.sha256()
    for path in paths:
        if path.is_file():
            digest.update(path.relative_to(ROOT).as_posix().encode())
            digest.update(b"\0")
            digest.update(path.read_bytes())
            digest.update(b"\0")
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path, help="frozen release CLI binary")
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument("--run", action="store_true", help="execute warmup and all manifest runs")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    output = args.output.resolve()
    if output == ROOT or ROOT in output.parents:
        parser.error("output must be outside the repository")
    manifest = json.loads(MANIFEST.read_text())
    cases = []
    for index, item in enumerate(manifest):
        source = ARCHIVE / "inputs" / item["input"]
        if not source.is_file() or sha256(source) != item["input_sha256"]:
            parser.error(f"missing input or SHA-256 mismatch: {source}")
        archived = ARCHIVE / f"{index:02d}-{item['input'].removesuffix('.json')}-{item['seed']}.json"
        if not archived.is_file():
            parser.error(f"missing archived response: {archived}")
        cases.append({**item, "index": index, "input_path": source, "baseline_path": archived})

    initial = {"binary_sha256": sha256(binary), "source_sha256": source_sha256(),
               "manifest_sha256": sha256(MANIFEST),
               "inputs": [{"name": c["input"], "seed": c["seed"], "sha256": c["input_sha256"]} for c in cases],
               "workers": WORKERS, "timeout_seconds": TIMEOUT_SECONDS, "cases": len(cases)}
    if not args.run:
        print(json.dumps({**initial, "output": str(output), "execution": "inert; pass --run to execute"}, indent=2))
        return 0

    output.mkdir(parents=True, exist_ok=True)
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(("RUST_ODDS_", "RARE_POSITION_"))}
    env["RUST_ODDS_LOG"] = "1"

    def execute(case: dict, stem: str, warmup: bool) -> dict:
        response = output / f"{stem}.json"
        stdout_path, stderr_path = output / f"{stem}.stdout", output / f"{stem}.stderr"
        command = [str(binary), "estimate", str(case["input_path"]), str(response),
                   str(case["seed"]), str(WORKERS)]
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        child_before = before.ru_utime + before.ru_stime
        started = time.perf_counter()
        try:
            with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
                completed = subprocess.run(command, env=env, stdout=stdout, stderr=stderr,
                                            timeout=TIMEOUT_SECONDS, check=False)
            error = None if completed.returncode == 0 else f"exit_{completed.returncode}"
            returncode = completed.returncode
        except subprocess.TimeoutExpired:
            error, returncode = "timeout", None
        wall = time.perf_counter() - started
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        child_cpu = after.ru_utime + after.ru_stime - child_before
        result = {"case": case["index"], "input": case["input"], "seed": case["seed"],
                  "family": case["family"], "warmup": warmup, "returncode": returncode,
                  "error": error, "wall_seconds": wall, "child_cpu_seconds": child_cpu,
                  "response_path": str(response), "stdout_path": str(stdout_path),
                  "stderr_path": str(stderr_path)}
        if error is None:
            produced = response.read_bytes()
            baseline_bytes = case["baseline_path"].read_bytes()
            try:
                result["exact_json_equal"] = json.loads(produced) == json.loads(baseline_bytes)
            except (json.JSONDecodeError, UnicodeDecodeError):
                result["exact_json_equal"] = False
            result["byte_equal"] = produced == baseline_bytes
        else:
            result["exact_json_equal"] = False
            result["byte_equal"] = False
        return result

    records = [execute(cases[0], "warmup", True)]
    records.extend(execute(case, f"{case['index']:02d}-{case['input'].removesuffix('.json')}-{case['seed']}", False)
                   for case in cases)
    final = {"binary_sha256": sha256(binary), "source_sha256": source_sha256(),
             "manifest_sha256": sha256(MANIFEST),
             "input_sha256": {c["input"] + ":" + str(c["seed"]): sha256(c["input_path"]) for c in cases}}
    summary = {"initial_hashes": initial, "final_hashes": final, "runs": records,
               "measured_runs": len(cases),
               "successful_runs": sum(r["returncode"] == 0 for r in records if not r["warmup"]),
               "exact_json_matches": sum(r["exact_json_equal"] for r in records if not r["warmup"]),
               "byte_matches": sum(r["byte_equal"] for r in records if not r["warmup"]),
               "wall_seconds_total": sum(r["wall_seconds"] for r in records if not r["warmup"]),
               "child_cpu_seconds_total": sum(r["child_cpu_seconds"] for r in records if not r["warmup"])}
    changed = (initial["binary_sha256"] != final["binary_sha256"] or
               initial["source_sha256"] != final["source_sha256"] or
               initial["manifest_sha256"] != final["manifest_sha256"] or
               any(final["input_sha256"].get(c["input"] + ":" + str(c["seed"])) != c["input_sha256"] for c in cases))
    summary["hashes_unchanged"] = not changed
    (output / "summary.json").write_text(json.dumps(summary, indent=2, allow_nan=False) + "\n")
    (output / "summary.md").write_text(
        "# Rare-tail heuristic timing profile\n\n"
        f"Measured runs: {summary['successful_runs']}/{len(cases)} successful.\n\n"
        f"Exact JSON matches to saved responses: {summary['exact_json_matches']}/{len(cases)}; "
        f"byte matches: {summary['byte_matches']}/{len(cases)}.\n\n"
        f"Total wall: {summary['wall_seconds_total']:.3f}s; child CPU: "
        f"{summary['child_cpu_seconds_total']:.3f}s. Hashes unchanged: {not changed}.\n"
    )
    if changed:
        raise RuntimeError("binary, source, manifest, or inputs changed during profiling")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
