"""Check CLI export determinism across worker, repeat, and logging settings."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys


HERE = Path(__file__).resolve().parent
SEED = "808"
RUNS = {
    "4workers": (4, "1", 1),
    "1worker": (1, "1", 1),
    "4repeat": (4, "1", 4),
    "4quiet": (4, "0", 1),
}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def clean_environment(run_mode: str, experiment: str) -> dict[str, str]:
    environment = os.environ.copy()
    for key in tuple(environment):
        if key.startswith("RARE_POSITION_") or key.startswith("RUST_ODDS_"):
            environment.pop(key)
    environment["RUST_ODDS_LOG"] = "0" if run_mode == "4quiet" else "1"
    if experiment != "baseline":
        environment["RUST_ODDS_EXPERIMENT_LAZY"] = experiment
    return environment


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path, help="built golaberto-odds executable")
    parser.add_argument(
        "mode", choices=("baseline", "portfolio", "pair", "marginal"),
        help="isolated lazy experiment setting; baseline leaves it unset",
    )
    parser.add_argument("request", type=Path, help="serialized request JSON")
    args = parser.parse_args()

    binary = args.binary.resolve(strict=True)
    request = args.request.resolve(strict=True)
    output_dir = HERE / "invariants" / args.mode / request.stem
    output_dir.mkdir(parents=True, exist_ok=True)

    reference = None
    runs = []
    for label, (workers, logging, count) in RUNS.items():
        for repetition in range(1, count + 1):
            stem = f"{label}-{repetition:02d}"
            export_path = output_dir / f"{stem}.json"
            stdout_path = output_dir / f"{stem}.stdout.log"
            stderr_path = output_dir / f"{stem}.stderr.log"
            command = [
                str(binary), "estimate", str(request), str(export_path), SEED,
                str(workers),
            ]
            environment = clean_environment(label, args.mode)
            with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
                result = subprocess.run(
                    command,
                    env=environment,
                    stdout=stdout,
                    stderr=stderr,
                    check=False,
                )
            command_record = {
                "argv": command,
                "seed": int(SEED),
                "workers": workers,
                "logging": logging,
                "experiment": None if args.mode == "baseline" else args.mode,
                "returncode": result.returncode,
                "stdout_log": stdout_path.name,
                "stderr_log": stderr_path.name,
            }
            (output_dir / f"{stem}.command.json").write_text(
                json.dumps(command_record, indent=2) + "\n"
            )
            if result.returncode != 0:
                raise RuntimeError(
                    f"{label} run failed (exit {result.returncode}); see {stderr_path}"
                )
            exported = json.loads(export_path.read_text())
            canonical = json.dumps(
                exported, sort_keys=True, separators=(",", ":"), ensure_ascii=False
            ).encode()
            if reference is None:
                reference = canonical
            elif canonical != reference:
                raise AssertionError(
                    f"parsed export differs in {stem}; see {export_path}"
                )
            runs.append(
                {
                    **command_record,
                    "export": export_path.name,
                    "export_sha256": sha256(export_path),
                    "export_bytes": export_path.stat().st_size,
                    "canonical_export_sha256": hashlib.sha256(canonical).hexdigest(),
                    "matches_reference": canonical == reference,
                }
            )

    report = {
        "binary": str(binary),
        "binary_sha256": sha256(binary),
        "request": str(request),
        "request_sha256": sha256(request),
        "experiment": args.mode,
        "seed": int(SEED),
        "runs": runs,
        "all_parsed_exports_equal": True,
    }
    (output_dir / "invariants.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
