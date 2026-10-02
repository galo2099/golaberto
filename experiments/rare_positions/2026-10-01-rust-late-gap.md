# Later gap rescue experiment — 2026-10-01

## Final disposition — 2026-10-02

Discarded both late-gap variants and their runtime flag. Neither rescuer
added a previously missing estimate, and both paired arms lost four cell-runs.
The historical results remain recorded; commands below require an isolated
archived candidate, not the current server.

Raw files are stored in `2026-10-01-late-gap/evidence.tar.gz`, with per-file
hashes in `manifest.json`. Extract that archive in its directory before using
the historical commands. Statements about flags, defaults and commit status
below refer to the experiment date. Current disposition is recorded in
[the decision report](2026-10-02-rust-experiment-decisions.md).

The late-gap archive also contains `cleanup-source.patch` against 31bc8b0a.
It preserves the local code at cleanup, including the inactive shorter-batch
option, and is **not** the earlier measured executable source snapshot.

## Implementation and decision

Extract both existing point-tilt gap paths and optionally execute them once after deferred joint and rare-tail publication, before reconciliation. Eligibility reads the newly published matrix. Existing acceptance gates, RNG streams and quotas remain; no interpolation or probability borrowing is introduced. The pilot variant skips a final proposal unless its independent training pilot has at least three hits and ESS 1.5. Skipping is a work decision, not an impossibility proof.

Keep this experimental, disabled by default. Enable with `RUST_ODDS_LATE_GAP_RESCUE=1`; `full` uses the unconditional moved stage. The baseline gap placement remains available with unset/0. Moving it alone did not demonstrate additional coverage, and latency increases are not assumed to fit the prior 50% allowance.

## Paired results

Four workers, alternating identical HTTP requests, three snapshots (16498, current 16653, historical 16653), seeds 801, 808, 1790864768586573000: nine pairs per variant. Both baseline and candidate use `RUST_ODDS_RARE_TAIL=coverage`. Baseline is the frozen current Rust coverage binary, not the old Go/native timing budget.

| Variant | Overall gain cell-runs | Overall loss cell-runs | Late stage ms | Full wall change | Summed CPU change |
|---|---:|---:|---:|---:|---:|
| move | 2 | 4 | 0.006–108.400 | -1.8–+13.1% | -2.2–+8.5% |
| pilot | 5 | 4 | 0.007–43.872 | -1.5–+27.8% | -0.7–+21.5% |

The unconditional move accepted no late estimates. The pilot variant accepted one (16498/team110/rank4), already positive in baseline: 1.45e-12 versus 1.35e-12 probability. Thus neither variant added a previously missing estimate through the gap rescuer. Overall gains/losses are rare-tail allocation differences; its time-funded allocation can vary even for fixed seeds. No impossible or reachability proof regressed. These runs cannot establish preservation of every baseline positive cell, so default adoption is not recommended.

## Reproduction

Frozen baseline SHA256: `4648ba0211c67ac455f1701aea6f9ab2085d1116c87390098a635a12f0cca899`.
The first move binary used flag `1` before the pilot guard existed; use `full` with the current source to reproduce that behavior. Per-arm binary and input hashes, paired matrices, stage costs and CPU measurements are saved in `2026-10-01-late-gap/{move,pilot}.json`.

The following reconstructs the **cleanup snapshot** against 31bc8b0a; it is a
new comparison, not an exact reproduction of the older measured cohort. Use
empty temporary directories:

```sh
mkdir -p /tmp/r40-late-base /tmp/r40-late-candidate /tmp/r40-late-evidence
git archive 31bc8b0a odds-rust stats/core | tar -x -C /tmp/r40-late-base
cp -R /tmp/r40-late-base/. /tmp/r40-late-candidate/
tar -xzf experiments/rare_positions/2026-10-01-late-gap/evidence.tar.gz \
  -C /tmp/r40-late-evidence
patch -d /tmp/r40-late-candidate -p1 < /tmp/r40-late-evidence/cleanup-source.patch
cargo build --release --offline --locked -j4 \
  --manifest-path /tmp/r40-late-base/odds-rust/Cargo.toml
cargo build --release --offline --locked -j4 \
  --manifest-path /tmp/r40-late-candidate/odds-rust/Cargo.toml
python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline /tmp/r40-late-base/odds-rust/target/release/golaberto-odds \
  --candidate /tmp/r40-late-candidate/odds-rust/target/release/golaberto-odds \
  --production-candidate --flag RUST_ODDS_LATE_GAP_RESCUE=1 \
  --cases 16498,16653 --seeds 801,808,1790864768586573000 \
  --output /tmp/r40-late-gap-pilot
```

No commit or push. This is a scheduling experiment, using the existing estimator; no new scientific method or external source claim is introduced.

Validation: Rust suite passes (79 tests; two database tests ignored). Added tests cover newly published neighbor eligibility, preserving impossible/positive cells, and pilot futility.
