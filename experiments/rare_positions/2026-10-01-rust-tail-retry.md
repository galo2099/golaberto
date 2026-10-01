# Rust tail allocation and Fluminense last — 2026-10-01

## Status and objective

**Production regression follow-up:** the three R26 defaults are restored
to their pre-release settings; the new strategy remains opt-in. See the
[regression follow-up](2026-10-01-rust-tail-regressions.md). The results below
are the original R26 campaign, not evidence that every production cell improves.

Implement a generic improvement for high-variance rare cells within the existing
Rust coverage allowance. No team IDs or ranks appear in allocation/search rules.
The previous shipped default (`2c87b146`) is the baseline. All probabilities in
this report are fractions; multiply by 100 for percentages.

The selected changes were enabled in af8dda53 and are now opt-in. Shipping was authorized
on 2026-10-01. Separate uncommitted late-gap changes are excluded from the
measured release tree and this release.

## Why a constrained event could still produce zero

In the group 16498 snapshot, Fluminense (team 8) starts at 48 points/13 wins with
10 fixtures left. Chapecoense can reach at most 51 points. A necessary condition
for Fluminense last is therefore at most three additional points. There are
186 target win/draw/loss paths: 1 all-loss path, 10 one-draw paths, 45 two-draw
paths, 120 three-draw paths, and 10 one-win paths. Rivals must also pass
Fluminense, with actual points, applicable wins and goal tiebreakers.

Constraints make hits plentiful once a good proposal exists. They do not make
importance weights uniform. The old seed 1229 main batch had 469 hits but its
largest weight supplied 50.58% of the estimate; ESS was only 3.80. It correctly
failed publication. Other seeds did not fund a final batch at all.

Complete branch construction was also wasted work here. Six-rival enumeration
exhausted its joint node budget (19.0ms). Four/three/two/one-rival alternatives
exhausted guide memory in 33.7/20.7/17.7/22.1ms. None built the full union.

## Implementation

1. **Cheap complete-union cutoff.** The coverage target-path enumeration cap is
   64 instead of 256. Oversized events route to the existing full-support lazy
   sampler. This does not truncate a probability event or establish impossibility.
   Other complete-union limits and six-rival configuration remain unchanged.
   Fluminense setup fell from approximately 32–53ms to 19–31ms in development.
2. **Pilot branch learning.** Record weighted second moments by target outcome
   path in the existing 1,000-draw pilot. For ESS below the existing threshold 12,
   try learned root allocation using the existing 1,000-draw adaptive retry slot.
   Keep it only when the independent retry pilot improves ESS; otherwise restore
   the original allocation. No extra pilot batch is introduced.
3. **Reserve ordinary batches first.** Fund existing main/check forecasts in
   descending pilot ESS order before enlarging any batch. Weak pilots can grow
   toward forecast ESS 32 only using spare capacity in their predicted worker
   bin. Main batches remain bounded at 6,000 and checks remain at least 1,000.
   Enlarging a batch does not remove another already-funded batch.
4. **Sparse pilots retain witness retries.** Branch learning requires at least
   30 matching pilot seasons. Sparser pilots use the existing alternate/witness
   proposal retry. This protects a useful mode when moments have too little
   information to identify root allocation.

The total tail allowance remains 35% of elapsed native calculation time;
shared sampling keeps its existing 50% share. Training stops starting jobs at
65% of the allowance. At most four estimator workers run. Pilot cost forecasts
use the existing 1.2 safety factor. This is not a hard real-time deadline.

### Weighting and correctness

Let `q_i` be the original marginal probability of sampling target root `i`, and
`S_i` the sum of squared successful importance contributions observed there.
The learned root score is proportional to `sqrt(q_i * S_i)`: multiplying by
`q_i` removes the original root allocation from the second-moment objective.
This is an empirical variance-allocation rule, not an exact oracle.

Blend the normalized learned cached-root allocation 50:50 with its original
allocation. Preserve the 10% original target-event defensive component.
Use the actual fitted marginal root probability in the importance denominator,
including all existing internal proposal-mixture and goal-tilt corrections.
Unobserved cached roots keep half their original cached allocation; uncached
roots retain defensive support. Invalid/empty moment fits are skipped.

Pilot/retry observations never contribute to published probabilities. The
proposal and main/check sizes are frozen before fresh estimation and separate
confirmation. Publication keeps the existing rough gates: main hits >=30,
ESS >=4, RSE <=60%, max weight share <=35%, batch gap <=1.5; check hits >=30,
ESS >=3, RSE <=75%, and estimates agree within a factor of five. Reachability
witnesses and upper bounds are not substituted for probabilities. Failed setup
or exhausted budgets do not establish impossibility.

## Paired experiment design

Freeze the shipped baseline executable, SHA256
`0f82dcaeaf9e6740a7b3b1fe041f8fa31f0e64291cdb4464e5f8e4a9928a56d0`.
The benchmark clears inherited estimator environment flags, alternates baseline
and candidate order, and submits full HTTP requests serially. Both use four
workers; no compilation/tests run during timings. Input SHA256 and executable
SHA256 are included in every archived summary.

Development: seeds 801/808/1229 on group 16498 and both group 16653 snapshots.
Validation: 804/817/911 on all seven snapshots (15902,16413,16498,16653,16982,
16983, historical16653). Reserved seeds: 1201/1213/1237 on all seven snapshots.
The sparse-pilot guard was motivated by the first validation; its retest is a
follow-up on already observed seeds, not a new held-out evaluation.

A gain/loss is a **cell-run**: one team/rank in one request. Repeated seeds or
warm requests can count the same cell repeatedly. Different arms have different
time-dependent baseline funding; their gains must not be added together.

### Development screens (nine pairs each)

| Arm | Gains | Losses | Decision |
| --- | ---: | ---: | --- |
| Root learning only | 5 | 2 | Combine with allocation/construction changes |
| Complete-union cap 64 only | 3 | 0 | Useful construction saving |
| Cap 64 + forecast ESS 20 for all | 3 | 0 | Fluminense1229 still fails weight-share gate |
| Cap 64 + forecast ESS 32 for all | 6 | 4 | Reject: displaces stronger pilots |
| Cap 64 + ESS32 only below ESS12 | 3 | 1 | Reject after validation |
| Previous row + root learning | 10 | 2 | Reject after validation |
| Cap 64 + reserve-first, without learning | 4 | 0 | Less effective; Fluminense1229 still zero |
| Cap 64 + reserve-first + learning (v1) | 9 | 1 | Strongest combined development arm |

The concentrated allocator, with or without learning, gained one and lost four
in each 21-pair validation. This prompted reserve-first allocation.

### Reserve-first v1 across 42 validation/reserved pairs

Gains16/losses3, covering13 distinct gained cells and2 distinct lost cells.
The original21 validation pairs contributed5/2; the reserved21 pairs11/1.
All four easy snapshots stayed completely positive (400 or324 cells).
No reachability, impossibility, or game-importance regressions occurred.

All-request median paired latency change: +0.004%; total wall +0.27%, CPU -0.45%.
On the18 difficult requests: median paired wall +0.51%, total wall +0.64%,
CPU -0.08%. The largest individual increase was **6.12%**, explicitly retained
in the archive. Difficult tail-stage medians were161.61→157.99ms.

Losses: Athletico-PR19 at817/911 and Fluminense20 at1213. The first two replaced
an effective witness/alternate retry with unhelpful root learning after only
3/2 initial hits. Their pilot ESS fell from12.16/11.50 to1.00/1.92, so they were
not eligible for finals. This motivated the sparse-pilot guard. Fluminense1213
had exactly the same strong pilot but no final funding; reserve-first preserves
batches within its current plan set, not the baseline's clock-dependent plan set.

### Warm v1 check (seed808, two warmups + eight timed rounds per snapshot)

| Snapshot | Gains/losses over eight pairs | Baseline median ms | Candidate median ms | Change | CPU change |
| --- | ---: | ---: | ---: | ---: | ---: |
| 16498 | 19/8 |840.59|833.72|−0.82%|−3.08%|
| 16653 current |0/0|847.39|849.05|+0.20%|−0.46%|
| 16653 historical |8/0|797.10|812.47|+1.93%|−0.16%|

Total27 gains/eight losses over24 pairs; total wall +0.24%, CPU -1.32%.
Largest individual wall increase2.40%. Tail medians209.34→205.70,
196.66→208.57,166.72→183.92ms respectively. Peak process RSS357.16→311.38,
246.58→246.20,254.27→254.77MiB; RSS/CPU include warmups. Gains from repeated
seed808 requests are allocator consistency observations, not independent seeds.

## Fluminense seed1229

| Metric | Shipped baseline | Reserve-first |
| --- | ---: | ---: |
| Main draws |1,000|2,932|
| Matching seasons |469|1,331|
| Main ESS |3.80|14.22|
| Largest weight share |50.58%|22.30%|
| Main RSE |51.25%|26.46%|
| Main estimate |8.176e-26 (rejected)|6.325e-26|
| Independent check estimate |not run|4.122e-26|
| Check ESS |0|16.97|
| Check RSE |—|24.15%|
| Published |zero|6.325e-26|

This is **6.325e-24%** in percentage units. No root fit was retained for this
cell; cheaper setup and a larger fixed main/check allocation made the difference.
Published values across the successful candidate seeds remain approximately
3.5e-26–6.3e-26. This is not an exact/golden probability or a guarantee of success
for every seed. In v1 Fluminense was positive8/9 development+validation seeds
versus7/9 in the baseline; candidate1213 still lacked funding.

## Reproduction and rollback

Build current code with `cargo build --release --manifest-path odds-rust/Cargo.toml`.
Build the baseline from an isolated checkout/archive of2c87b146, with a separate
Cargo target directory, and save its executable. No database refresh is required;
the saved input requests are used verbatim.

```sh
PYTHONPYCACHEPREFIX=/tmp/golaberto-r26-pycache \
python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline /tmp/golaberto-r26-baseline \
  --candidate odds-rust/target/release/golaberto-odds \
  --production-candidate \
  --seeds 804,817,911,1201,1213,1237 \
  --output /tmp/golaberto-r26-validation
```

The commands above describe the original af8dda53 defaults. On the corrected
branch, reproducing the R26 strategy requires three candidate flags:
`--flag RUST_ODDS_RARE_TAIL_UNION_PATTERNS=64`,
`--flag RUST_ODDS_RARE_TAIL_RETRY=roots`, and
`--flag RUST_ODDS_RARE_TAIL_BATCH_ALLOCATION=reserve`. For the exact archived
implementation, build af8dda53 in an isolated checkout. To reproduce legacy construction,
retry and allocation with the same binary, set all three:

```sh
RUST_ODDS_RARE_TAIL_UNION_PATTERNS=256 \
RUST_ODDS_RARE_TAIL_RETRY=0 \
RUST_ODDS_RARE_TAIL_BATCH_ALLOCATION=0 \
./odds-rust/target/release/golaberto-odds serve
```

For warm benchmarks add `--persistent --cases 16498,16653`. To compare experimental profiles, pass repeated
`--flag NAME=VALUE`; `reserve`, `concentrated`, `robust`, `robust32`, and legacy0
allocation modes remain available. Rejected modes are not enabled by default.

Archives in `2026-10-01-tail-retry/*.json.gz` contain paired summaries, final
quality evidence, raw stage costs, gain/loss lists, seeds and input/binary hashes.
Read with `json.load(gzip.open(path,'rt'))`. `metrics.json` summarizes the first
experiment arms. Timing affects funding even for fixed seeds; exact nonzero
counts need not be bit-identical across machines or repetitions.

## Sources and verification

- [`rare_tail.rs`](../../odds-rust/src/rare_tail.rs): budget, pilot selection,
  frozen batches, quality gates, explicit profile settings.
- [`joint_caps/propagated/lazy.rs`](../../odds-rust/src/joint_caps/propagated/lazy.rs):
  root moments, defensive mixture, fitted denominator and rollback.
- [`joint_caps/propagated.rs`](../../odds-rust/src/joint_caps/propagated.rs):
  full-union budget failure skips the model without truncating support.
- [`benchmark_propagated_joint.py`](benchmark_propagated_joint.py): serial paired
  HTTP measurement and four-worker harness.
- [Original Rust report](2026-09-30-rust.md) and
  [coverage campaign](2026-10-01-rust-rare50.md): underlying model/proof assumptions.

Tests exhaustively enumerate a small shared-fixture league and compare fitted
sampling against exact rank probabilities at all ranks with two cache sizes and
two independent seeds. Tests also verify training matches ordinary draws, empty
fits are skipped, allocation stays normalized/positive, rollback restores exact
CDF/sample bits, and budget upgrades require spare capacity while preserving
main/check reservations and limits. Full final-suite outcome is recorded below.

## Final sparse-pilot guard follow-up

The guard preserves the existing witness retry when there are fewer than 30
pilot hits. It is included in the default coverage configuration. The final executable
SHA256 is `91404d9afcec70236e92ba6d253f2ba85a86acdc3a256e265afe76ed77880287`;
`final-validation.json.gz`/`final-warm.json.gz` used **no candidate flags**.

### Final cold follow-up (49 pairs, all seven snapshots × seven seeds)

- Gains11/losses2: nine distinct gained cells, two distinct lost cells. Net nine
  fewer zero cell-runs. All proof labels and game importance remain valid.
- Athletico19 at817 and911 survives in both arms. The new losses are
  Chapecoense5 and Athletico17 at1213. Their strong pilots are unchanged, but
  their finals were not funded. Allocation remains timing-dependent.
- Fluminense is positive in six of seven candidate requests versus four of
  seven baseline requests. Seed1229 still fails the ordinary final gate in
  this cohort: reserve-first did not find enough spare capacity for its full
  2,932-draw upgrade. The earlier successful1229 experiment remains evidence
  that a larger independently confirmed batch works, **not a guarantee that
  this allocator funds it every time**.
- Overall median paired wall−0.70%, total wall−0.29%, CPU−1.21%. On the21
  difficult pairs: median paired wall−0.31%, total wall−0.07%, CPU−1.01%; tail
  stage medians155.51→156.97ms. The largest difficult-pair increase was4.10%.
- Maximum cold-pair wall increase **11.59%** (group15902/804,193.22→215.61ms),
  retained without exclusion. That group has no zero cells and no tail work.
  Timing variation is not attributed entirely to the new algorithm.

### Final warm follow-up (24 pairs at808)

| Snapshot | Gains/losses across eight pairs | Baseline median ms | Candidate median ms | Change | CPU change |
| --- | ---: | ---: | ---: | ---: | ---: |
|16498|5/0|841.79|833.24|−1.02%|−4.12%|
|16653 current|0/0|851.54|842.71|−1.04%|−1.58%|
|16653 historical|8/0|807.37|811.79|+0.55%|−3.08%|

Total13 gains/zero losses. Total wall−1.15%, CPU−2.93%; largest individual
increase1.96%. Tail medians208.17→204.98,195.64→201.74,161.54→180.16ms.
Peak RSS358.39→330.98,246.25→245.23,256.13→254.06MiB. The final-round
comparison alone has only one gained cell; the full13 count includes every
round, where deadline-based funding differs.

### Recommendation and remaining limitations

Enable bounded setup, guarded pilot root allocation and reserve-first upgrades:
coverage improves overall, aggregate full-request latency stays near the
current baseline, CPU does not increase, and weighting/quality checks remain
intact. Keep the four-worker and35% allowance unchanged. Do not enable the
concentrated/all-cell larger-batch allocators. No universally guaranteed
per-cell coverage improvement was established.

The next unresolved allocation issue is that reserving every ordinary finalist
can leave too little spare capacity for a promising weak pilot; deadlines can
also reject a slower strong pilot while funding a cheaper lower-ESS one. The
current change does not invent an estimate when that happens.

Final full-suite outcome: **88 passed, zero failed, two database tests ignored**.
Ran the final guarded release source tree with `cargo test -- --test-threads=1`,
serializing the four-worker full-request tests. `rustfmt` and `git diff --check`
also passed. The stored `candidate.patch.gz` contains the two Rust source changes
against the shipped baseline, excluding unrelated working-tree edits.
No DB mutations were performed.
