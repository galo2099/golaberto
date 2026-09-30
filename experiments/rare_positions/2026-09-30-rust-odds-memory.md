# Reducing Rust `/odds` memory without reducing estimator work

## Scope

Work is isolated on `codex/rust-service-replacement`. The baseline is the native
Rust replacement at `69f820a5`, copied before the memory changes. The candidate
is the implementation accompanying this report. No live service was replaced.

The goal is lower memory while preserving request performance, all probability
estimates, uncertainty metadata, reachability proofs, search budgets and seeds.
The 20,000-season initial scout, 100,000-season pool, four estimator workers,
conditional sample allocations, DP limits and acceptance gates are unchanged.
No new environment flags are required.

## Diagnosis and implementation

An optional diagnostic executable wraps Rust's system allocator with live-byte
and high-water counters. Sampling those counters every two milliseconds alongside
existing stage logs located the peak in deeper conditioning searches. The
baseline current 16653 calculation reached **289.9 MiB of requested live heap**;
the final candidate reached **74.8 MiB**, a 74.2% reduction. These counts exclude
allocator overhead and thread/OS memory, and are separate from process RSS.

Three changes address the measured allocations:

1. **Release obsolete conditioning events.** Cells that already have successful
   initial or confirmed guided estimates no longer retain their DP layers.
   Before extra search, all earlier events are released: extra search builds
   fresh events from the saved result metadata. Estimates and witnesses remain.
2. **Freeze completed DP layers.** Growing hash maps are still used to construct
   each layer with the original capacities and insertion behavior. Once complete,
   a layer stores its `(state, mass)` entries compactly in their original iteration
   order, preserving floating-point summation order. Wide states use a compact
   `u32` lookup index. One-byte point-total states use direct mass lookup with a
   presence bitset, preserving absent entries and zero-valued masses exactly.
   Only the currently growing layer needs oversized mutable hash buckets.
3. **Use a dedicated HTTP calculation worker.** Four body readers retain upload
   concurrency, and computation remains serialized with four estimator workers.
   Reusing one calculation thread avoids rotating its working allocations among
   readers. A rendezvous queue bounds prepared requests. Results return to the
   readers for response I/O, keeping slow downloads outside the calculator.
   Health remains independent, and the incoming queue remains bounded at 16.

Releasing obsolete events alone reduced peak process RSS by about 21–27% in
search-heavy snapshots, with identical outputs and similar speed. The first
generic compact-index experiment reduced RSS by about 50–60%, but introduced
a roughly 5% slowdown in two snapshots. Direct lookup for small point states
and a dedicated calculation worker removed that measured regression. Only
the final candidate is retained as production code.

## Final paired HTTP results

Measured on the same local macOS machine with two persistent loopback servers,
alternating requests, seed 808, two warmups and nine timed requests per snapshot.
Only one request runs at a time; each estimator has four workers. Logs are
disabled. Latency includes upload, calculation and response transfer. Each
snapshot uses fresh processes. `/usr/bin/time -l` records the whole-process
maximum resident set size over all eleven requests, including warmups.

| Snapshot | Baseline median | Candidate median | Latency change | Baseline peak RSS | Candidate peak RSS | RSS reduction |
|---|---:|---:|---:|---:|---:|---:|
| 16498 / 44eabb47 | 652.8 ms | 654.8 ms | +0.30% | 373.3 MiB | 188.2 MiB | 49.6% |
| 16653 / earlier 2d1c1d6f | 640.4 ms | 640.4 ms | +0.01% | 434.8 MiB | 190.3 MiB | 56.2% |
| 16653 / current 71d4fea8 | 645.3 ms | 648.5 ms | +0.50% | 483.3 MiB | 198.1 MiB | 59.0% |
| 16982 / 9327edcd | 288.3 ms | 291.1 ms | +0.98% | 25.9 MiB | 25.9 MiB | 0.2% |
| 16983 / 43969b02 | 291.1 ms | 288.3 ms | -0.97% | 25.5 MiB | 25.4 MiB | 0.6% |

The measured latency changes are within 1%, with no material regression.
16982 and 16983 do not need the large conditioning searches in this run, so
their memory is essentially unchanged. The reductions target DP memory rather
than reduced sampling. These are local macOS results; Linux allocator behavior
and peak RSS require measurements on the production host. No concurrent-load
throughput claim is made.

**All 55 HTTP pairs have identical complete parsed responses**, including game
importance, every position probability, uncertainty, reachability and work
counter. This compares the candidate against the prior Rust service, not Go.
The complete raw timing series is in `2026-09-30-rust-odds-memory.json` beside
this report. Raw logs and allocation traces remain under
`/private/tmp/golaberto-rust-service` for this session.

## Verification

- Full Rust release suite: 23 passing tests, one unchanged opt-in MySQL test
  skipped. Exhaustive tests still cover conditional event support/mass, weighted
  probability estimates, rank-domain propagation and cap/floor proofs.
- New storage tests verify original iteration order and exact float bits,
  packed keys through 120,000 entries, collision handling, absent keys,
  key zero, wide keys and direct lookup of zero/subnormal masses.
- HTTP tests cover active routes, chunked JSON, errors, database failures,
  continued health and a new incomplete-upload case that permits another
  calculation while the upload remains open.
- The paired HTTP comparison asserts complete response equality on every
  warmup and timed request, so the memory savings preserve reported work.
- Ten additional CLI pairs (all five snapshots, seeds 42 and 2026) also have
  identical complete responses. Combined with HTTP seed 808, this covers three
  distinct search selections without changing estimates or proof metadata.
- Rechecked all four active HTTP endpoints against Go with two warmups and
  three timed requests. SPI differs by at most 1.1369e-13, evaluation by
  4.4409e-16; historical responses and 2,230 stored rows match exactly. Historical
  writes used a disposable MySQL schema, which was dropped afterward. The paired
  current 16653 run measured Go at 1,123.0 ms / 302.1 MiB peak RSS and Rust at
  657.6 ms / 190.4 MiB. This shorter cross-language check is separate from the
  nine-request Rust-to-Rust table above.

## Reproduction

Save a baseline executable before building the candidate, then run from the
repository root:

```sh
cargo build --release --locked --manifest-path odds-rust/Cargo.toml
cargo test --release --locked --manifest-path odds-rust/Cargo.toml
PYTHONDONTWRITEBYTECODE=1 python3 experiments/rare_positions/benchmark_odds_memory.py \
  --baseline /path/to/baseline-golaberto-odds \
  --candidate odds-rust/target/release/golaberto-odds \
  --iterations 9 --output /tmp/odds-memory-paired
```

The default benchmark covers all five saved snapshots. Add `--input REQUEST.json`
to select a snapshot or `--seed 42` for another stream. It uses macOS process
resource accounting and makes no database writes.

For an allocation trace, independently of latency measurements:

```sh
RUST_ODDS_PROFILE=1 cargo run --release --locked \
  --manifest-path odds-rust/Cargo.toml --example profile_memory -- REQUEST.json \
  > /tmp/odds-heap.csv 2> /tmp/odds-heap-stages.log
```

The diagnostic adds allocation-counter overhead and is not installed in the
ordinary service executable. CSV columns are elapsed milliseconds, live requested
bytes and maximum requested bytes. The production service needs only a normal
release rebuild and restart to use these changes.
