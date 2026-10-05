#!/bin/sh
# Run from the repository; isolated builds, serialized requests, four workers.
set -eu
task_repo=$(pwd)
task_root=${1:-/private/tmp/golaberto-r44-reproduction}
task_artifact="$task_repo/experiments/rare_positions/2026-10-02-lazy-partitions"
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
cargo test --release --all-targets -j 4 --manifest-path "$task_root/candidate/odds-rust/Cargo.toml" \
  --target-dir "$task_root/candidate-target" -- --test-threads=1

task_baseline="$task_root/baseline-target/release/golaberto-odds"
task_candidate="$task_root/candidate-target/release/golaberto-odds"
task_bench=experiments/rare_positions/benchmark_propagated_joint.py
# V2: 10% native support, two guidance pilots per region.
python3 "$task_bench" --baseline "$task_baseline" --candidate "$task_candidate" \
  --production-candidate --flag RUST_ODDS_EXPERIMENT_LAZY_PARTITIONS=1 \
  --cases 16498 --seeds 808,1669,1993,2281,2293 --output "$task_root/screen-v2"
# V3: 50% native support.
python3 "$task_bench" --baseline "$task_baseline" --candidate "$task_candidate" \
  --production-candidate --flag RUST_ODDS_EXPERIMENT_LAZY_PARTITIONS=1 \
  --flag RUST_ODDS_EXPERIMENT_LAZY_PARTITION_NATIVE=0.5 \
  --cases 16498 --seeds 808,1669,1993,2281,2293 --output "$task_root/screen-v3"
# V4: also offer cardinality guidance, compare development and holdout cohorts.
for task_cohort in development holdout; do
  if [ "$task_cohort" = development ]; then task_seeds=808,1669,1993,2281,2293
  else task_seeds=801,804,817,911,1861; fi
  python3 "$task_bench" --baseline "$task_baseline" --candidate "$task_candidate" \
    --production-candidate --flag RUST_ODDS_EXPERIMENT_LAZY_PARTITIONS=1 \
    --flag RUST_ODDS_EXPERIMENT_LAZY_PARTITION_NATIVE=0.5 \
    --flag RUST_ODDS_EXPERIMENT_LAZY_PARTITION_CARDINALITY=1 \
    --cases 16498,16653,16982,16983,15902,16413 --seeds "$task_seeds" \
    --output "$task_root/$task_cohort"
done
python3 "$task_bench" --baseline "$task_baseline" --candidate "$task_candidate" \
  --production-candidate --persistent --flag RUST_ODDS_EXPERIMENT_LAZY_PARTITIONS=1 \
  --flag RUST_ODDS_EXPERIMENT_LAZY_PARTITION_NATIVE=0.5 \
  --flag RUST_ODDS_EXPERIMENT_LAZY_PARTITION_CARDINALITY=1 \
  --cases 16498 --output "$task_root/persistent"
python3 "$task_bench" --baseline "$task_baseline" --candidate "$task_candidate" \
  --production-candidate --flag RUST_ODDS_EXPERIMENT_LAZY_PARTITIONS=0 \
  --cases 16498,16653,16982 --seeds 808 --output "$task_root/flag-off"
python3 "$task_artifact/invariance.py" "$task_candidate" \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  "$task_root/invariance"
# V1 source: apply v1-overlay.patch after prototype.patch, rebuild to a separate
# target directory, then run the V2 screen command against that binary. This
# reproduces the conservative caps and serial setup (all attempts fell back).
# v2-overlay.patch reproduces the intermediate fixed-10% native version.
