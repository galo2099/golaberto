#!/bin/sh
# Run from the Golaberto repository. Uses isolated sources; no commits/DB writes.
set -eu
task_repo=$(pwd)
task_root=${1:-/tmp/golaberto-r42-reproduction}
task_artifact="$task_repo/experiments/rare_positions/2026-10-02-flamengo-messages"
test ! -e "$task_root/baseline"
test ! -e "$task_root/candidate"
mkdir -p "$task_root/baseline" "$task_root/candidate"
git archive f0a332b847bd7bea24b00fd63e724850da4fa065 odds-rust stats/core |
  tar -x -C "$task_root/baseline"
git archive f0a332b847bd7bea24b00fd63e724850da4fa065 odds-rust stats/core |
  tar -x -C "$task_root/candidate"
ln -s "$task_repo/experiments" "$task_root/baseline/experiments"
ln -s "$task_repo/experiments" "$task_root/candidate/experiments"
(cd "$task_root/candidate" && git apply "$task_artifact/prototype.patch")
cargo build --release -j 4 --manifest-path "$task_root/baseline/odds-rust/Cargo.toml" \
  --target-dir "$task_root/baseline-target" --bin golaberto-odds
cargo build --release -j 4 --manifest-path "$task_root/candidate/odds-rust/Cargo.toml" \
  --target-dir "$task_root/candidate-target" --bin golaberto-odds --example branch_stratification
cargo test --all-targets -j 4 --manifest-path "$task_root/candidate/odds-rust/Cargo.toml" \
  --target-dir "$task_root/candidate-target" -- --test-threads=1

unset TREE_MODE TREE_MESSAGES TREE_TRAINING TREE_LEAVES TREE_GUIDE_VALUES
unset TREE_MESSAGE_ITERATIONS TREE_MESSAGE_SHARE TREE_RESIDUAL_POWER TREE_PRIORITY
task_baseline="$task_root/baseline-target/release/golaberto-odds"
task_candidate="$task_root/candidate-target/release/golaberto-odds"
task_bench="experiments/rare_positions/benchmark_propagated_joint.py"

python3 "$task_bench" --baseline "$task_baseline" --candidate "$task_candidate" \
  --production-candidate --flag RUST_ODDS_EXPERIMENT_TREE=1 \
  --flag RUST_ODDS_EXPERIMENT_TREE_PILOT=25 \
  --flag RUST_ODDS_EXPERIMENT_CONFIRM_PREFIX=1 \
  --flag RUST_ODDS_EXPERIMENT_RARE_BONUS_UNITS=16000000 \
  --cases 16498,16653,16982,16983,15902,16413 --seeds 808,1669,1993,2281,2293 \
  --output "$task_root/development"
python3 "$task_bench" --baseline "$task_baseline" --candidate "$task_candidate" \
  --production-candidate --flag RUST_ODDS_EXPERIMENT_TREE=1 \
  --flag RUST_ODDS_EXPERIMENT_TREE_PILOT=25 \
  --flag RUST_ODDS_EXPERIMENT_CONFIRM_PREFIX=1 \
  --flag RUST_ODDS_EXPERIMENT_RARE_BONUS_UNITS=16000000 \
  --cases 16498,16653,16982,16983,15902,16413 --seeds 801,804,817,911,1861 \
  --output "$task_root/holdout"
python3 "$task_bench" --baseline "$task_baseline" --candidate "$task_candidate" \
  --production-candidate --persistent --flag RUST_ODDS_EXPERIMENT_TREE=1 \
  --flag RUST_ODDS_EXPERIMENT_TREE_PILOT=25 \
  --flag RUST_ODDS_EXPERIMENT_CONFIRM_PREFIX=1 \
  --flag RUST_ODDS_EXPERIMENT_RARE_BONUS_UNITS=16000000 \
  --cases 16498 --output "$task_root/persistent"
python3 experiments/rare_positions/summarize_rare_cell_budget.py "$task_root/development" \
  --output "$task_root/development/rare-budget.json"
python3 experiments/rare_positions/summarize_rare_cell_budget.py "$task_root/holdout" \
  --output "$task_root/holdout/rare-budget.json"

# Single-cell screen; this command is not a full-request timing comparison.
env TREE_MODE=1 TREE_LEAVES=64 TREE_TRAINING=0 TREE_MESSAGES=1 \
  TREE_MESSAGE_ITERATIONS=3 "$task_root/candidate-target/release/examples/branch_stratification" \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  17 13 3000 808,1669,1993,2281,2293 trained bounds 4 tilt plain 25 10 \
  > "$task_root/offline-screen.jsonl"

python3 -m unittest discover -s experiments/rare_positions -p test_rare_cell_budget.py
python3 -m unittest discover -s experiments/rare_positions -p test_compare_rust_reachability.py
