#!/bin/sh
# Run from the repository. Isolated sources, four workers, no database writes.
set -eu
task_repo=$(pwd)
task_root=${1:-/private/tmp/golaberto-r43-reproduction}
task_artifact="$task_repo/experiments/rare_positions/2026-10-02-lazy-messages"
test ! -e "$task_root/baseline"
test ! -e "$task_root/candidate"
mkdir -p "$task_root/baseline" "$task_root/candidate"
for task_arm in baseline candidate; do
  git archive ac92a8ab88656c2e20a24b37c1f3c3d3dcfdb4cd odds-rust stats/core |
    tar -x -C "$task_root/$task_arm"
  ln -s "$task_repo/experiments" "$task_root/$task_arm/experiments"
done
(cd "$task_root/candidate" && git apply "$task_artifact/prototype.patch")
cargo build --release -j 4 --manifest-path "$task_root/baseline/odds-rust/Cargo.toml" \
  --target-dir "$task_root/baseline-target" --bin golaberto-odds
cargo build --release -j 4 --manifest-path "$task_root/candidate/odds-rust/Cargo.toml" \
  --target-dir "$task_root/candidate-target" --bin golaberto-odds
cargo test --all-targets -j 4 --manifest-path "$task_root/candidate/odds-rust/Cargo.toml" \
  --target-dir "$task_root/candidate-target" -- --test-threads=1

task_baseline="$task_root/baseline-target/release/golaberto-odds"
task_candidate="$task_root/candidate-target/release/golaberto-odds"
task_bench=experiments/rare_positions/benchmark_propagated_joint.py
python3 "$task_bench" --baseline "$task_baseline" --candidate "$task_candidate" \
  --production-candidate --flag RUST_ODDS_EXPERIMENT_LAZY_MESSAGES=1 \
  --flag RUST_ODDS_EXPERIMENT_LAZY_MESSAGE_ROOTS=1 \
  --cases 16498,16653,16982,16983,15902,16413 --seeds 808,1669,1993,2281,2293 \
  --output "$task_root/development"
python3 "$task_bench" --baseline "$task_baseline" --candidate "$task_candidate" \
  --production-candidate --flag RUST_ODDS_EXPERIMENT_LAZY_MESSAGES=1 \
  --flag RUST_ODDS_EXPERIMENT_LAZY_MESSAGE_ROOTS=1 \
  --cases 16498,16653,16982,16983,15902,16413 --seeds 801,804,817,911,1861 \
  --output "$task_root/holdout"
python3 "$task_bench" --baseline "$task_baseline" --candidate "$task_candidate" \
  --production-candidate --persistent --flag RUST_ODDS_EXPERIMENT_LAZY_MESSAGES=1 \
  --flag RUST_ODDS_EXPERIMENT_LAZY_MESSAGE_ROOTS=1 --cases 16498 \
  --output "$task_root/persistent"
# Other screened settings with the final prototype: ROOTS=0 (certificates only),
# or ROOTS=1 plus GUIDE=bounds (interval guidance). Earlier rebuilding and10%
# conditional-native variants are archived as patches and have separate results.
python3 "$task_bench" --baseline "$task_baseline" --candidate "$task_candidate" \
  --production-candidate --flag RUST_ODDS_EXPERIMENT_LAZY_MESSAGES=0 \
  --cases 16498,16653,16982 --seeds 808 --output "$task_root/flag-off"
python3 "$task_artifact/invariance.py" "$task_candidate" \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  "$task_root/invariance"
# Optional weight diagnostics do not participate in timing or work admission.
(cd "$task_root/candidate" && git apply "$task_artifact/diagnostic.patch")
cargo build --release -j 4 --manifest-path "$task_root/candidate/odds-rust/Cargo.toml" \
  --target-dir "$task_root/candidate-target" --bin golaberto-odds
python3 "$task_bench" --baseline "$task_candidate" --candidate "$task_candidate" \
  --baseline-flag RUST_ODDS_EXPERIMENT_LAZY_WEIGHT_LOG=1 \
  --production-candidate --flag RUST_ODDS_EXPERIMENT_LAZY_MESSAGES=1 \
  --flag RUST_ODDS_EXPERIMENT_LAZY_MESSAGE_ROOTS=1 \
  --flag RUST_ODDS_EXPERIMENT_LAZY_WEIGHT_LOG=1 --cases 16498 --seeds 808,2281,2293 \
  --output "$task_root/diagnostics"
python3 -m unittest discover -s experiments/rare_positions -p test_rare_cell_budget.py
python3 -m unittest discover -s experiments/rare_positions -p test_compare_rust_reachability.py
