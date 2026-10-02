#!/bin/sh
# Run from the repository root. All HTTP requests use four estimator workers,
# serialized; do not overlap these measurements with builds or other runs.
set -eu

# Historical prototype/baseline builds can be reconstructed with ../commands.sh.
# These variables name those unchanged binaries, not a new production budget.
: "${R42_PROTOTYPE_BINARY:?Path to the validated R42 prototype binary}"
: "${R42_BASELINE_BINARY:?Path to the previous production binary}"
: "${R42_ADOPTION_OUTPUT:=/private/tmp/r42-adoption-reproduction}"

cargo test --manifest-path odds-rust/Cargo.toml --all-targets --jobs 4 -- --test-threads=1
cargo build --manifest-path odds-rust/Cargo.toml --release --jobs 4
python3 -m unittest discover -s experiments/rare_positions -p 'test_rare_cell_budget.py'
python3 -m unittest discover -s experiments/rare_positions -p 'test_compare_rust_reachability.py'

python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline "$R42_PROTOTYPE_BINARY" \
  --candidate odds-rust/target/release/golaberto-odds --production-candidate \
  --baseline-flag RUST_ODDS_EXPERIMENT_TREE=1 \
  --baseline-flag RUST_ODDS_EXPERIMENT_TREE_PILOT=25 \
  --baseline-flag RUST_ODDS_EXPERIMENT_CONFIRM_PREFIX=1 \
  --baseline-flag RUST_ODDS_EXPERIMENT_RARE_BONUS_UNITS=16000000 \
  --cases 16498,16653,16982,16983,15902,16413 \
  --seeds 808,1669,1993,2281,2293,801,804,817,911,1861 \
  --output "$R42_ADOPTION_OUTPUT/equivalence"

python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline "$R42_BASELINE_BINARY" \
  --candidate odds-rust/target/release/golaberto-odds --production-candidate \
  --flag RUST_ODDS_RARE_TAIL_TREE=0 --cases 16498,16653,16982 --seeds 808 \
  --output "$R42_ADOPTION_OUTPUT/disable"

for round in 1 2; do
  python3 experiments/rare_positions/benchmark_propagated_joint.py \
    --baseline "$R42_BASELINE_BINARY" \
    --candidate odds-rust/target/release/golaberto-odds --production-candidate \
    --persistent --cases 16498 --output "$R42_ADOPTION_OUTPUT/warm-$round"
done

python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline "$R42_PROTOTYPE_BINARY" \
  --candidate odds-rust/target/release/golaberto-odds --production-candidate \
  --baseline-flag RUST_ODDS_EXPERIMENT_TREE=1 \
  --baseline-flag RUST_ODDS_EXPERIMENT_TREE_PILOT=25 \
  --baseline-flag RUST_ODDS_EXPERIMENT_CONFIRM_PREFIX=1 \
  --baseline-flag RUST_ODDS_EXPERIMENT_RARE_BONUS_UNITS=16000000 \
  --persistent --cases 16498 --output "$R42_ADOPTION_OUTPUT/warm-prototype"

python3 - "$R42_ADOPTION_OUTPUT" <<'PY'
from pathlib import Path
import json, sys
root = Path(sys.argv[1])
for name, expected_count in [('equivalence', 70), ('disable', 4)]:
    directory = root / name
    pairs = json.loads((directory / 'summary.json').read_text())['pairs']
    assert len(pairs) == expected_count
    for pair in pairs:
        stem = f"{Path(pair['input']).stem}-{pair['seed']}"
        assert (directory / f'{stem}-baseline.json').read_bytes() == \
               (directory / f'{stem}-candidate.json').read_bytes(), stem
    print(f'{name}: {len(pairs)} byte-identical paired exports')
PY
