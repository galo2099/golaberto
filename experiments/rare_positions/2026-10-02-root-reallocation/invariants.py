#!/usr/bin/env python3
"""Run sequential four-repeat, one-worker, and quiet-log export invariants."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


RUNS = (
    ("fourrepeat", 4, True, 4),
    ("oneworker", 1, True, 1),
    ("quietlog", 4, False, 1),
)


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def initial_mc(export: dict) -> dict:
    return {
        team: {
            rank: {key: estimate.get(key) for key in ("samples", "hits")}
            for rank, estimate in ranks.items()
        }
        for team, ranks in sorted((export.get("rare_position_estimates") or {}).items())
    }


def clean_environment(logging: bool, flags: dict[str, str]) -> dict[str, str]:
    env = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("RARE_POSITION_", "RUST_ODDS_"))
    }
    env.update(flags)
    env["RUST_ODDS_LOG"] = "1" if logging else "0"
    return env


def parse_flags(raw_flags: list[str]) -> dict[str, str]:
    flags = {}
    for raw in raw_flags:
        if "=" not in raw:
            raise ValueError(f"flag must be NAME=VALUE: {raw!r}")
        name, value = raw.split("=", 1)
        if not name:
            raise ValueError("flag name cannot be empty")
        flags[name] = value
    return flags


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path, help="built golaberto-odds executable")
    parser.add_argument("request", type=Path, help="serialized request JSON")
    parser.add_argument("--flag", action="append", default=[], help="arbitrary NAME=VALUE experiment flag")
    parser.add_argument("--seed", default="808")
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--arm", default="candidate", help="label recorded in invariants.json")
    args = parser.parse_args()

    binary = args.binary.resolve(strict=True)
    request = args.request.resolve(strict=True)
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    flags = parse_flags(args.flag)
    runs = []
    reference_export_hash = None
    reference_initial_hash = None
    all_exports_identical = True
    all_initial_mc_identical = True

    for label, workers, logging, repetitions in RUNS:
        for repetition in range(1, repetitions + 1):
            stem = f"{label}-{repetition:02d}"
            export_path = output_dir / f"{stem}.json"
            stdout_path = output_dir / f"{stem}.stdout.log"
            stderr_path = output_dir / f"{stem}.stderr.log"
            command = [str(binary), "estimate", str(request), str(export_path), str(args.seed), str(workers)]
            with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
                completed = subprocess.run(
                    command,
                    env=clean_environment(logging, flags),
                    stdout=stdout,
                    stderr=stderr,
                    check=False,
                )
            record = {
                "name": stem,
                "argv": command,
                "arm": args.arm,
                "seed": int(args.seed),
                "workers": workers,
                "logging": logging,
                "flags": flags,
                "returncode": completed.returncode,
                "stdout_log": stdout_path.name,
                "stderr_log": stderr_path.name,
            }
            (output_dir / f"{stem}.command.json").write_text(json.dumps(record, indent=2) + "\n")
            if completed.returncode != 0:
                raise RuntimeError(f"{stem} failed ({completed.returncode}); see {stderr_path}")
            export = json.loads(export_path.read_text())
            export_hash = sha256_bytes(canonical(export))
            initial = initial_mc(export)
            initial_hash = sha256_bytes(canonical(initial))
            all_exports_identical &= reference_export_hash in (None, export_hash)
            all_initial_mc_identical &= reference_initial_hash in (None, initial_hash)
            reference_export_hash = reference_export_hash or export_hash
            reference_initial_hash = reference_initial_hash or initial_hash
            record.update({
                "export": export_path.name,
                "export_file_sha256": sha256_bytes(export_path.read_bytes()),
                "parsed_export_sha256": export_hash,
                "initial_mc_sha256": initial_hash,
                "parsed_export_matches_first": export_hash == reference_export_hash,
                "initial_mc_matches_first": initial_hash == reference_initial_hash,
            })
            runs.append(record)

    report = {
        "arm": args.arm,
        "binary": str(binary),
        "binary_sha256": sha256_bytes(binary.read_bytes()),
        "request": str(request),
        "request_sha256": sha256_bytes(request.read_bytes()),
        "seed": int(args.seed),
        "flags": flags,
        "runs": runs,
        "all_parsed_exports_identical": all_exports_identical,
        "all_initial_mc_identical": all_initial_mc_identical,
        "initial_mc_sha256": reference_initial_hash,
    }
    report_path = output_dir / "invariants.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: value for key, value in report.items() if key != "runs"}, indent=2))
    print(f"runs: {len(runs)}; details: {report_path}")
    return 0 if all_exports_identical and all_initial_mc_identical else 1


if __name__ == "__main__":
    raise SystemExit(main())
