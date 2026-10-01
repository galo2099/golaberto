#!/usr/bin/env python3
"""Build a four-core Rust odds executable using local reference-request profiles.

This is an optional build experiment. It does not change Monte Carlo budgets,
RNG streams, probability gates, or database contents. Profiles must be rebuilt
with the same compiler and source used for the final executable.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]


def run(command, **kwargs):
    subprocess.run([str(v) for v in command], check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--budget-fraction", default="0.5")
    args = parser.parse_args()
    source = args.repo.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="pgo-", dir=output))
    raw = work / "raw"
    raw.mkdir()
    sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
    version = subprocess.check_output(["rustc", "-Vv"], text=True)
    host = next(line.split(": ", 1)[1] for line in version.splitlines() if line.startswith("host:"))
    profdata = sysroot / "lib" / "rustlib" / host / "bin" / "llvm-profdata"
    if not profdata.is_file():
        raise SystemExit("Matching LLVM tools missing. Run: rustup component add llvm-tools")
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("RUST_ODDS_", "RARE_POSITION_", "LLVM_PROFILE_"))}
    env["CARGO_TARGET_DIR"] = str(work / "target")
    cargo = ["cargo", "build", "--release", "--locked", "-j", "4", "--target", host,
             "--manifest-path", source / "odds-rust" / "Cargo.toml"]
    env["RUSTFLAGS"] = f"-Cprofile-generate={raw}"
    run(cargo, env=env)
    executable = work / "target" / host / "release" / "golaberto-odds"
    requests = sorted((source / "experiments/rare_positions/reference/2026-09-30-hundredfold/inputs").glob("group-*.json"))
    requests.append(source / "odds-rust/tests/fixtures/group-16653-2d1c1d6f.json")
    if len(requests) != 7 or not all(p.is_file() for p in requests):
        raise SystemExit("Expected the seven tracked reference request snapshots")
    training_env = dict(env)
    training_env["LLVM_PROFILE_FILE"] = str(raw / "%m-%p.profraw")
    training_env["RUST_ODDS_RARE_TAIL_BUDGET_FRACTION"] = args.budget_fraction
    for request in requests:
        for seed in (1103, 1109):
            print(f"Training {request.name}, seed {seed}", flush=True)
            with (work / f"{request.stem}-{seed}.log").open("w") as log:
                run([executable, "estimate", request, work / "training-response.json", seed, 4],
                    env=training_env, stdout=subprocess.DEVNULL, stderr=log)
    merged = work / "merged.profdata"
    run([profdata, "merge", "-o", merged, *sorted(raw.glob("*.profraw"))])
    env["RUSTFLAGS"] = f"-Cprofile-use={merged} -Cllvm-args=-pgo-warn-missing-function"
    run(cargo, env=env)
    (work / "build.json").write_text(json.dumps({
        "compiler": version, "source": str(source), "seeds": [1103, 1109],
        "requests": [str(p) for p in requests], "workers": 4,
        "budget_fraction_during_training": args.budget_fraction,
        "executable": str(executable), "profile": str(merged),
    }, indent=2) + "\n")
    print(executable)


if __name__ == "__main__":
    main()
