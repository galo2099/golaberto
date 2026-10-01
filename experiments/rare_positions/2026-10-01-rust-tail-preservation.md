# Ordinary-result preservation and rare-tail retries — 2026-10-01

## Default enablement update

After reviewing the coverage and latency results, the user requested that the
revision be turned on. The coverage profile now defaults to sequential `after`
retries; `overlap` remains experimental. No changes were made to the measured
algorithm, quality gates, four-worker usage or existing time allowance. Rebuild
and restart the server; no new flag is required. Set
`RUST_ODDS_RARE_TAIL_EXTENSION=0` to roll back only the retries. With the new
default, this opt-out is also needed to run the original R26 allocation
experiment described below.

The decisions below describe the initial opt-in shipment and its measurements.
Default/rollback tests and an HTTP profile check cover this promotion; the final
validation result is recorded in FUTURE_EXPERIMENTS.md.

## Decision

Keep production defaults unchanged. Implement an **opt-in sequential retry**
(`RUST_ODDS_RARE_TAIL_EXTENSION=after`) that fixes displacement within a request:
freeze and run ordinary coverage work first, then only fill remaining zeros.
The idle-worker variant (`overlap`) is also available for experiments, but its
limited slack makes it less useful. Neither is recommended as a new default.

The sequential arm has useful coverage gains, especially two historical16653
cells, but it is not a complete solution to run-to-run losses. Ordinary training
and funding still use measured elapsed time. A cold paired request can lose
ordinary funding despite having identical pilots. The report includes those
losses instead of labeling them solved.

Latency increases are explicit: warm total wall **+0.10%**, historical16653
median **+4.49%**, and fresh-seed full-cohort total wall **+2.09%**. No additional
allowance was assumed. These increases prevent unconditional default promotion
under the current latency constraint.

The user authorized shipment as an opt-in revision. Production defaults remain
unchanged. No database mutation, UI change or probability/proof relaxation was
performed.

## Baseline and method

Baseline: Rust master34000cc1b26ef4aa60d66d208be861c7d2f2765e, after the
[R26 regression rollback](2026-10-01-rust-tail-regressions.md).
Executable SHA256: `0fff218bd815131565b10663709b66dce970d5a019805c905ca4889d6eca961a`.
Final candidate SHA256: `786e7201067feff09f79991ada7d73a064c160114b264899eeb1df43052524bc`.
Development v1: `32e45f7074c3df36d5999638467c3f18d0aee7f901b6601abdc369fc5f70f9d0`.
Toolchain: rustc 1.93.1 (01f6ddf75 2026-02-11); cargo 1.93.1 (083ac5135 2025-12-15).

Identical saved JSON and fixed seeds, four estimator workers, serial full HTTP
requests, alternating arm order. No concurrent builds, tests, or benchmark
requests. Development seeds1301/1303/1307/1319/1321 cover16498 and both16653
snapshots. Warm runs use seed808, two warmups plus eight measured paired requests
per difficult snapshot. Fresh seeds1409/1423 cover all seven snapshots, including
16982,16983,15902 and16413. One generous-budget control is separate from all
production timing comparisons.

The binaries were built from an isolated source tree excluding pre-existing,
unrelated late-gap edits. The benchmark clears inherited estimator flags and
captures every `rust_odds_rare_tail*` event. Compressed summaries retain input
hashes, executable hashes, flags, stages, ordinary/extra evidence and comparisons.
Patches for v1 and final source are saved alongside them.

## Revised strategy

1. Restore ordinary complete-union cutoff256, existing witness/alternate retries,
   and ordinary batch allocation, even if the earlier64/roots/reserve opt-ins
   are present. Earlier pipeline stages and other explicit flags still apply.
2. Train, rank, truncate and fund ordinary proposals as before. Freeze ordinary
   main/check sample counts and their RNG stream names before extension work.
   Retain already trained but unfunded eligible finalists.
3. Run ordinary final jobs first on the same worker queue. Record acceptance.
4. Consider failed ordinary finals and unfunded eligible pilots, cheapest
   forecast first, only if their fresh main/check batches fit the remaining
   time forecast. Do not rebuild guides or start another model.
5. Sequential `after`: after ordinary jobs finish, target32 effective pilot
   samples (40 for strict mode), fixed main1000–6000 (strict2000–8000), independent
   check at least1000 and otherwise half the main batch. Reuse the frozen plan.
6. `overlap`: dispatch all ordinary jobs first; use spare workers only within
   the predicted ordinary final span. Skip a source still in flight or already
   accepted. Final revision targets10 effective pilot samples (strict20), with
   the same minimum/maximum batch sizes. The initial arm targeted32/40 and
   rarely fit in an idle-worker gap.
7. Use cell-specific **new main and independent check streams**. Ordinary
   observations, including rejection evidence, never enter the new estimate.
   Commit all ordinary results first, then apply retries only where probability
   remains exactly zero. Matrix reconciliation may change values slightly.

The existing35% tail allowance is unchanged. Predicted cost uses measured pilot
sampling throughput with the existing1.2 multiplier. Deadlines are forecasts,
not hard real-time bounds; retaining proposals can also change memory lifetime.
There is no additional solver/model construction: setup and shared-guide work
are ordinary work already incurred. Logs record retry forecast and actual sample
cost, main/check evidence, effective settings and final-phase duration.

No root learning or cheaper union rebuilding is performed during these retries.
This revision trades that part of R26 for a fixed ordinary candidate set and
preserved accepted estimates.

## Paired results

Counts are **cell-runs**, not distinct newly covered cells. Repeated warm gains
for one cell count once per request. Direct retries exclude gains caused only
by ordinary funding variation. CPU for warm runs includes all ten requests;
warm wall comparisons use eight measured requests, so do not divide those totals
to infer worker occupancy.

|Cohort|Paired requests|Gained cell-runs|Lost cell-runs|Accepted retries / attempts|Total wall change|CPU change|
|---|---:|---:|---:|---:|---:|---:|
|after|15|9|4|7/11|+1.24%|+0.45%|
|overlap|15|6|2|2/2|+0.94%|+2.13%|
|overlap-small|15|10|2|5/5|+1.81%|+3.00%|
|after-warm|24|18|1|16/16|+0.10%|-0.44%|
|overlap-small-warm|24|8|6|8/8|-0.57%|-1.30%|
|after-holdout|14|1|0|1/3|+2.09%|+2.26%|

All comparisons retain impossibility proofs and reachability and have identical
game importance. Reconciliation can rescale earlier positive estimates. The
warm `after` gains are18 cell-runs, of which16 are directly accepted retries:
**two distinct historical16653 cells**, team95/ranks1 and3, repeated across eight
measured requests. Their fractions are about1.60e-27 and3.60e-15; they are not
18 distinct new cells. Cold sequential retries also confirm16498/team8/rank20,
current16653/team95/rank5 and team125/rank11. The cold accepted-retry probabilities
span9.58e-28 to3.93e-15. These are weighted estimates passing the same confirmation
gates; this experiment does not establish external ground-truth accuracy at
those magnitudes.

### Sequential warm per-snapshot costs

|Snapshot|Full-request median ms|Change|Rare-tail median ms|CPU change|Peak RSS MiB|
|---|---:|---:|---:|---:|---:|
|group-16498-44eabb47.json|854.07 → 845.56|-1.00%|212.16 → 211.88|-0.94%|354.4 → 353.2|
|group-16653-71d4fea8.json|862.99 → 834.99|-3.24%|192.43 → 192.97|-2.79%|246.4 → 243.2|
|group-16653-2d1c1d6f.json|796.69 → 832.45|+4.49%|167.15 → 202.85|+3.28%|254.7 → 253.9|

Maximum individual paired full-request increase is6.01% for sequential warm,
5.55% in its development cold screen, and4.90% in its fresh-seed cohort. Low
aggregate cost does not erase the historical16653 median increase. Its tail
median grows about35.70ms, corresponding to useful additional confirmations.
Peak RSS did not increase in the sequential warm cohort; this is a measured
observation, not a memory bound.

## Remaining losses and the verified guarantee

Sequential cold seed1307/16498 loses team15/rank19, team74/ranks19 and20,
and team8/rank20 relative to its separate baseline process. Their pilots are
bit-for-bit the same. The baseline has37.54ms remaining after training and funds
four finals; candidate has19.59ms remaining and funds none. Their allowances are
224.09ms versus210.75ms, reflecting different earlier elapsed time. Thus no
extension runs and no accepted estimate is overwritten. The same current
baseline seed1307 returns365 versus369 positive cells in separate development
campaigns, illustrating the wall-clock dependence directly.

The sequential warm loss is16498/team318/rank4 in measured round7. Smaller
overlap cold losses are16498/1303/team318/rank5 and team5/rank18; again identical
pilots but missing ordinary final allocation. Aggregate net gains alone do not
prove that every formerly positive cell survives across processes.

The guarantee actually tested is narrower and useful: **every ordinary accepted
estimate survives extension processing within that request**. Unit tests verify
exact metadata preservation and reject missing/inconsistent independent evidence.
All91 ordinary accepted finals in each initial cold extension cohort remain
positive in their saved response after reconciliation.

An offline control uses fraction2 in both arms to remove ordinary funding
scarcity for16498/1307. All15 ordinary final logs have identical main/check draw
counts, probabilities, ESS, weight statistics and acceptance;11 are accepted.
The candidate retains every baseline positive and adds team318/rank4, about
3.50e-19, ESS10.14, independently checked at2.21e-19, ESS4.86. Failed retries stay
zero. This control is a correctness diagnostic and **not** a production budget
proposal; wall1109.80→1160.27ms is excluded from the production tables.

## Correctness and quality

The ordinary weighted probability models, defensive support, normalization and
production sorter are unchanged. Main and confirmation use distinct derived
RNG streams and fixed sample counts selected without those batches' outcomes.
Pilot/rejection-based selection never supplies a published probability. No
witness, upper bound or intermediate-rank inference becomes an estimate.

Rough gates remain: weighted positive main, ≥30 hits, ESS≥4, RSE≤60%, maximum
weight share≤35%, batch gap≤1.5; independent check ≥30 hits, ESS≥3, RSE≤75%, and
main/check ratio between0.2 and5. Strict mode keeps its existing stronger gates.
A rejected batch does not prove impossibility. Budget exhaustion leaves cells
as zero with their existing reachability metadata.

New tests cover exact preservation of an existing probability plus metadata,
zero-only application, opt-out default, and required consistent confirmation.
The existing suite additionally covers exhaustive weighted sampler probabilities,
defensive support, actual goal tiebreakers and sound impossibility constraints.
Full Rust suite: **90 passed, zero failed, two DB tests ignored**, with
`--test-threads=1`. Formatting and diff whitespace checks passed. No Ruby request/response shapes or Rails call sites changed.

## Reproduction

Build a baseline at34000cc1 and freeze its binary before building the candidate.
Apply the compressed `2026-10-01-tail-preservation/candidate.patch.gz` to that
source for the final candidate; v1 uses `candidate-v1.patch.gz`. Rust release builds use the locked
manifest and four workers in the harness. All reference paths/hashes are in the
compressed summaries. Exact funding/counts can vary with elapsed time.

```sh
cargo build --release --locked --manifest-path odds-rust/Cargo.toml
cp odds-rust/target/release/golaberto-odds /tmp/golaberto-r28-baseline
gzip -dc experiments/rare_positions/2026-10-01-tail-preservation/candidate.patch.gz | git apply
# Rebuild the release binary after applying the patch.
python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline /tmp/golaberto-r28-baseline \
  --candidate odds-rust/target/release/golaberto-odds \
  --production-candidate --flag RUST_ODDS_RARE_TAIL_EXTENSION=after \
  --cases 16498,16653 --seeds 1301,1303,1307,1319,1321 \
  --output /tmp/golaberto-r28-after
# Warm: replace --seeds with --persistent. Idle-worker arm: change after to overlap.
# Fresh full cohort: omit --cases and use --seeds 1409,1423.
cargo test --manifest-path odds-rust/Cargo.toml -- --test-threads=1
```

The actual measurement build used
`/private/tmp/golaberto-shared-release/odds-rust/Cargo.toml` with the repository's
`odds-rust/target` as CARGO_TARGET_DIR to exclude unrelated working-tree edits.

### Opt in

The already shipped R26 experiment is still selectable:

```sh
RUST_ODDS_RARE_TAIL=coverage \
RUST_ODDS_RARE_TAIL_UNION_PATTERNS=64 \
RUST_ODDS_RARE_TAIL_RETRY=roots \
RUST_ODDS_RARE_TAIL_BATCH_ALLOCATION=reserve \
  odds-rust/target/release/golaberto-odds serve
```

It can displace ordinary estimates, as established in the regression report.
After rebuilding with the safer revision, use instead:

```sh
RUST_ODDS_RARE_TAIL=coverage RUST_ODDS_RARE_TAIL_EXTENSION=after \
  odds-rust/target/release/golaberto-odds serve
```

Preserve your normal server arguments and database configuration. Unset the new
flag or set it to0 to return to the unchanged default. Older shipped binaries
do not implement this new flag. The revision is shipped as an opt-in; normal defaults remain unchanged.

## Recommendation and next experiment

Keep `after` opt-in for users willing to accept measured small latency increases
for more tiny-event coverage. Do not enable `overlap` by default: it leaves less
useful coverage and its net result is too dependent on ordinary time funding.
Do not re-enable64/roots/reserve globally.

The next useful allocation experiment is a **deterministic ordinary work
reservation**, calibrated to current native costs across all snapshots. Reserve
sampling work before starting more proposal setup, account for in-flight setup
jobs, and separate ordinary work quotas from elapsed-time admission for optional
extensions. Measure losses with the extension off as a control, then reintroduce
root learning only after ordinary work is reserved. This requires a measured
tradeoff; it cannot promise invariant coverage and unchanged latency simply by
spending whatever a nominal allowance leaves unused.
