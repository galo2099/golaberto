# Rust service replacement merge verification

## Scope

Merge `codex/rust-service-replacement` at `8efab685` into the current odds branch
at `f9e763d9`. This integrates native `/spi`, `/eval`, `/historic_ratings`, the
Rails historical-response compatibility fix, bounded HTTP I/O and the branch's
compact conditioning DP storage. The current deferred witnesses, aggregate
cuts, recycling policy and goal completion remain enabled by default.

The only text conflict was the library module list; retain both ratings and
reachability modules. No production process was restarted. Application database
rating data was not changed by verification.

## Verification

- Full Rust release suite: **40 passed**, one opt-in temporary-table MySQL test
  ignored. Includes Go differential fixtures for all three rating calculations,
  HTTP routing/chunked uploads/errors/health checks, and all current estimator tests.
- Full Rails suite: **119 runs, 348 assertions, zero failures/errors, one skip**.
- Targeted historical endpoint test: three runs, nine assertions, no failures.
- Release build, Rust formatting, Ruby syntax and merge diff checks passed.
- **47 complete paired HTTP responses match exactly** across the two batches,
  including warmups, all probabilities, game importance, uncertainty, work
  counters and reachability labels. Five frozen snapshots, seed 808, four
  estimator workers; requests alternate and calculate sequentially.

## HTTP latency and memory

Baseline is the current Rust odds service, not Go. Two warmup pairs precede
each snapshot; the initial batch has three timed pairs and the confirmation
has nine. Latency is full HTTP round trip. RSS is whole-process maximum over
warmups plus timed requests, measured with macOS `/usr/bin/time -l`.

| Batch / snapshot | Baseline median ms | Merged median ms | Change | Baseline peak MiB | Merged peak MiB |
|---|---:|---:|---:|---:|---:|
| initial / group-16498-44eabb47.json | 633.1 | 668.4 | +5.58% | 368.7 | 179.4 |
| initial / group-16653-2d1c1d6f.json | 618.2 | 620.6 | +0.39% | 390.8 | 189.4 |
| initial / group-16653-71d4fea8.json | 631.7 | 655.4 | +3.75% | 476.8 | 190.0 |
| initial / group-16982-9327edcd.json | 290.7 | 290.9 | +0.07% | 19.4 | 25.7 |
| initial / group-16983-43969b02.json | 281.5 | 288.3 | +2.42% | 16.3 | 25.0 |
| confirmation / group-16498-44eabb47.json | 645.7 | 669.7 | +3.73% | 368.0 | 189.3 |
| confirmation / group-16653-71d4fea8.json | 638.9 | 649.8 | +1.70% | 490.6 | 194.8 |

The confirmation measures **+3.73%** latency for 16498 and **+1.70%** for
current 16653. Their peak RSS falls approximately **49%** and **60%**. These
are explicit latency increases; the merge does not establish preservation of
the earlier full-request latency. No extra computational allowance is assumed.
The dense snapshots gain dependency/server overhead without appreciable DP
storage savings; their short initial timing series is not a precise latency
estimate. This merge retains the requested branch's implementation; output
equivalence does not imply equal runtime.

These local results are separate from the source branch's earlier benchmark
against a different Rust baseline and do not replace that historical evidence.
The frozen source-branch rating tests validate Go contract parity; this merge
did not rerun actual application-table historical writes.

## Reproduce

```sh
cargo test --release --locked --manifest-path odds-rust/Cargo.toml
bin/rails test
cargo build --release --locked --manifest-path odds-rust/Cargo.toml
PYTHONDONTWRITEBYTECODE=1 python3 experiments/rare_positions/benchmark_odds_memory.py \
  --baseline /private/tmp/golaberto-rust-before-endpoint-merge \
  --candidate odds-rust/target/release/golaberto-odds --iterations 3 \
  --output /tmp/rust-service-merge-paired
```

Use `--iterations 9` and repeated `--input` arguments for the confirmation.
The baseline executable must be built from `f9e763d9`; copy it before building
the merged release. Private temporary paths identify this session's evidence,
not production configuration. Raw paired timing/RSS data and binary hashes are
in [the merge results](2026-09-30-rust-service-merge.json).

Deployment requires rebuilding and restarting the Rust service on port 6577
and setting `DATABASE_URL` to the Rails database for historical persistence.
The separate `stats` player-rating service remains on 6578.
