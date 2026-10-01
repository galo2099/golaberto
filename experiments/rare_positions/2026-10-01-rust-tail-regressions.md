# R26 regression follow-up — 2026-10-01

## Decision

Restore the three production defaults changed by af8dda53:

| Setting | R26 default | Corrected default |
| --- | --- | --- |
| `RUST_ODDS_RARE_TAIL_UNION_PATTERNS` |64|256|
| `RUST_ODDS_RARE_TAIL_RETRY` |roots|0|
| `RUST_ODDS_RARE_TAIL_BATCH_ALLOCATION` |reserve|0|

Default coverage, shared blocker sampling, earlier probability estimators,
proofs, acceptance gates, four workers and the existing35% tail allowance remain
as in2c87b146. R26 remains explicitly available by setting64/roots/reserve.
Unfitted root weights now also use the original floating-point operation order.
No database or UI changes are involved.

Corrected executable SHA256:
`0fff218bd815131565b10663709b66dce970d5a019805c905ca4889d6eca961a`.

The production complaint prompted this rollback. We have reproduced similar
losses on reference inputs, but the user's exact groups/cells/seeds have not yet
been supplied; this is not a claim to have reproduced every reported example.

## Root cause established by logs

The reserve-first allocator protects ordinary batches selected by the **new**
pilot population. It does not reserve the previous version's candidate set.
Faster construction allows additional pilots to start before the time cutoff;
root learning can improve other pilots and move them ahead in the final queue.
Those new finalists consume capacity ahead of previously successful cells.
Ordinary batches are reserved before enlargement, so turning off enlargement
alone cannot repair that change in candidate population and queue order.

For group16498 the fresh default screen lost these cells:

| Seed | Team/rank | Baseline estimate (fraction) | Baseline final evidence | New run |
| --- | --- | ---: | --- | --- |
|1303|Cruzeiro19|1.720e-12|246hits,ESS157.98,RSE7.30%|same pilot, no final|
|1307|Athletico18|1.855e-15|31hits,ESS11.20,RSE29.73%|same pilot, no final|
|1319|Chapecoense4|2.931e-19|77hits,ESS6.66,RSE38.62%|promising pilot, no final|

Pilot ESS was35.57/15.06/12.47 respectively; all retained reachable proof status.
The baseline estimates passed independent confirmation. The new run did not
prove them impossible or observe a numerical zero in its final sampler; it
never ran that sampler for those cells. This is a scheduling regression.

The original R26 campaign's aggregate gains were insufficient evidence for
safe default promotion. Its same-seed comparison was also affected by different
wall-clock funding. No guarantee that previous estimates survive was established.

## Measurements

Baseline: previous shipped default2c87b146, executable SHA256
`0f82dcaeaf9e6740a7b3b1fe041f8fa31f0e64291cdb4464e5f8e4a9928a56d0`.
R26 executable SHA256
`91404d9afcec70236e92ba6d253f2ba85a86acdc3a256e265afe76ed77880287`.
Fixed seeds1301/1303/1307/1319/1321 across16498 and both16653 snapshots.
Serial alternating full HTTP requests, four workers total; no compilation or
other benchmarks during timings. All counts below are **cell-runs**, not unique
cells. Separate arm cohorts have different baseline clock funding; do not add
or subtract their gain counts as if they shared a fixed baseline matrix.

| Arm vs2c87b146 | Pairs | Gained | Lost | Median paired wall change |
| --- | ---: | ---: | ---: | ---: |
|R26 shipped defaults|15|6|3|−1.75%|
|Disable root learning only|15|4|5|+1.25%|
|Restore union256 only|15|5|3|−1.05%|
|Disable upgrades only|15|8|3|+0.79%|
|Restore all three defaults + original weight operation order|15|6|1|+0.23%|

No partial rollback removed regressions in these screens. Default restoration
has the previous algorithm's selection logic; remaining gain/loss variation
comes from elapsed-time funding, which already existed before R26. The restored
screen's one loss is Athletico19/1319, with a promising pilot and no final batch.
It should not be presented as proof that all earlier estimates are guaranteed.
Cold total wall+1.24%, CPU+0.78%, max individual wall increase7.23%. We do not
interpret the6/1 count as a new estimator improvement.

### Warm default-restoration check

Seed808, two warmups and eight timed comparisons per snapshot:

| Snapshot | Gains/losses across eight pairs | Baseline median ms | Corrected median ms | Median change | CPU change |
| --- | ---: | ---: | ---: | ---: | ---: |
|16498|5/0|836.41|847.63|+1.34%|+4.18%|
|16653 current|0/0|870.93|853.73|−1.97%|−2.53%|
|16653 historical|0/0|805.66|800.76|−0.61%|+0.11%|

All24 warm pairs:5 gains/zero losses; total wall−0.33%, CPU+0.57%, median paired
wall−0.75%. Maximum individual increase7.52%. Tail-stage medians205.49→211.42,
194.66→193.41,166.30→163.35ms. CPU includes warmups; wall summaries exclude them.
The differences reflect clock funding/runtime variation between implementations
of the same default algorithm, not a newly claimed coverage optimization.

### Allocation-control diagnostic

One group16498/1303 pair used `RUST_ODDS_RARE_TAIL_BUDGET_FRACTION=2` in **both**
arms to remove the final-allocation bottleneck. This is an offline correctness
control, not a proposed production time allowance or a performance comparison.
Both runs returned376 positive/17 impossible/7 reachable-zero cells, with no
gained/lost cells, exactly matching existing probabilities/standard errors,
unchanged reachability and game importance. Parsed complete response objects
were exactly equal (`baseline == corrected`). The normal production allowance
remains0.35.

## Reproduction

Build the previous executable from an isolated archive/checkout of2c87b146 and
save it separately. Build the corrected release from this commit. No database
refresh is needed; the benchmark uses the saved JSON snapshots verbatim.

```sh
cargo build --release --manifest-path odds-rust/Cargo.toml
PYTHONPYCACHEPREFIX=/tmp/golaberto-r27-pycache \
python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline /tmp/golaberto-r26-baseline \
  --candidate odds-rust/target/release/golaberto-odds \
  --production-candidate --cases 16498,16653 \
  --seeds 1301,1303,1307,1319,1321 \
  --output /tmp/golaberto-r27-restored
```

Add `--persistent` for the warm check. For the allocation control use
`--cases 16498 --seeds 1303` and both
`--baseline-flag RUST_ODDS_RARE_TAIL_BUDGET_FRACTION=2` and
`--flag RUST_ODDS_RARE_TAIL_BUDGET_FRACTION=2`.

For a running R26 server, these overrides restore the old profile immediately
after restart without waiting for the corrected binary:

```sh
RUST_ODDS_RARE_TAIL_UNION_PATTERNS=256 \
RUST_ODDS_RARE_TAIL_RETRY=0 \
RUST_ODDS_RARE_TAIL_BATCH_ALLOCATION=0 \
./odds-rust/target/release/golaberto-odds serve
```

Preserve your normal server arguments/database configuration. Corrected defaults
need no flags. Explicit R26 overrides must be removed or changed to256/0/0.

Raw summaries in `2026-10-01-tail-regressions/*.json.gz` include input/binary
hashes, flags, stage/final evidence and comparison details. `metrics.json`
contains per-arm timing and coverage. They are readable using Python'sgzip/json
modules. The measured tree excludes the separate uncommitted late-gap work.

## Verification and next work

Full Rust suite: **88 passed, zero failed, two database tests ignored**, run
with `--test-threads=1` to serialize full requests and preserve four workers.
Formatting and whitespace checks passed. Config tests check the restored defaults and
explicit opt-in. Existing exhaustive rank tests cover learned and unfitted
importance weights; the original operation order is retained when unfitted.

A future allocation experiment should run and publish the ordinary production
pipeline first, then use actual unused tail capacity for new proposals, with
fresh pilot/main/check draws. That would protect estimates already obtained
within the same request. It would still not eliminate the existing clock-based
variation across runs; deterministic work quotas need separate measurement.
No new impossibility claim or witness-as-probability fallback is introduced.
