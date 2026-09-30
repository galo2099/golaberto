# Rust reachability through goal-difference and goals-scored completion

## Implementation

The initially opt-in `RUST_ODDS_GOAL_COMPLETION=1` extends the previous best native Rust
`deferred` configuration. It does not introduce team IDs or position-specific
rules, extra MC seasons, an external solver or another thread pool.

For a complete fixed win/draw/loss assignment, construct the points/wins prefix
required by the phase. Goal adjustments cannot change that prefix. A team's
remaining match against a team outside this prefix tie supplies an independent
unbounded goal-difference direction: a projected win permits increase, a loss
permits decrease, and both permit either direction.

Use those directions to find the required number of rivals above the target.
Only a bounded list of integer values at current score boundaries is considered;
this is not random score search. Equal values can proceed to the next tiebreaker.
For the resulting points/wins/goal-difference tie, any remaining fixture against
a team outside it allows goals scored to increase without changing its margin:
add the same number of goals to both endpoints, including draws and losses.

Every proposed assignment accounts for both endpoints and is checked by the
actual production sorter. Accepted certificates retain W/D/L outcomes, compact
fixture adjustments and actual verified ranks. Full score arrays are temporary.
There is no arbitrary goal cap: arithmetic that cannot be represented safely
causes this heuristic to decline, never to certify impossibility.

Supported score prefixes are `pt,[w,]gd,...`, with a goals-scored second stage
when `gf` immediately follows `gd`, and `pt,[w,]gf,...`. When wins follows goal
difference, it is not incorrectly included in the preceding prefix. Later keys
are checked by the full sorter. Nonzero goal-related bonus configuration is
excluded. Both endpoints within the relevant cohort require coupled reasoning;
this cheap independent completion leaves those unresolved when necessary.

## Placement and work allocation

The hook runs on complete assignments rejected by canonical sorting, at most
two attempts per cell, sixteen algebraic layouts per attempt (shared across both
score stages). Successful minimum-path completion stops otherwise unproductive
branching within the same 100-node per-cell / 3,200-total ceiling. Maximum-path
search retains its original traversal and sampler seeds. New compact certificates
are published only after the existing probability estimators finish. Each actual
season is reused across all its verified ranks, without substituting a witness
for a probability estimate.

The cap/floor ceilings, neighborhood candidates, probability draw quotas,
acceptance gates and four-worker limit are unchanged. Goal-completion time is
included in construction; deferred publication is included in reachability stage
cost. Actual time savings are measured, not assumed from lower node counts.

## Baseline and reproducibility

The baseline is the previous best CURRENT Rust experimental executable, frozen
before this implementation as
`/private/tmp/golaberto-rust-reachability-2026-09-30/goal-baseline`, SHA-256
`ffe6298d77c148bfa39d6fdab8bb00fa1c367ea42db9724e47c7a57ba1a05fcc`.
It uses `WITNESS_MODE=deferred`, `AGGREGATE_CUTS=early` and
`PROOF_RECYCLE_CREDIT=legacy`. This baseline already contains the earlier native
reachability improvements; comparisons here are incremental over those results,
not over Go or the older unmodified Rust commit.

Frozen inputs are the same five snapshots of four groups: 16498, two versions of
16653, 16982 and 16983. Four workers total, sequential alternating comparisons,
identical input fingerprints and ten seeds (801,804,808,817,911,802,805,809,818,912).
Inherited estimator flags are stripped. No database writes or concurrent
estimation/compilation during timing. Final timing uses normal-logging complete
HTTP requests, with listener startup separate and experiment tracing disabled.

```sh
cargo build --release --locked --manifest-path odds-rust/Cargo.toml
python3 experiments/rare_positions/compare_rust_reachability.py \
  --baseline /private/tmp/golaberto-rust-reachability-2026-09-30/goal-baseline \
  --baseline-variant deferred \
  --candidate odds-rust/target/release/golaberto-odds \
  --reference experiments/rare_positions/reference/2026-09-30-hundredfold/reference.json \
  --output /tmp/rust-goal-http --variants goals \
  --seeds 801,804,808,817,911,802,805,809,818,912 --http --no-trace
```

If the frozen prior executable is unavailable, use the current executable for
both sides with `--baseline-variant deferred`; that isolates flag-on versus
flag-off behavior but is a new measurement, not reproduction of the older binary.
The prior algorithm's implementation/report is described in
[the reachability report](2026-09-30-rust-reachability.md).

## Paired results

**Thirty additional reachable cell-runs / three distinct cells**, consistently
across all ten seeds:

- 16498 / Palmeiras (16) / position 15.
- 16498 / Palmeiras (16) / position 16.
- Current 16653 / team 70 / position 17.

No additional impossible cells, no lost known proofs, no nonzero gains/losses,
no proof conflicts, and **all 400 probability values bit-identical in every pair**.
Recycled draws and point-tilt work counts also match exactly. These gains reduce
undecideds, not the number of zero probabilities.

| Input | Impossible zeros | Reachable zeros without estimates | Undecided zeros |
|---|---:|---:|---:|
| 16498-44eabb47 | 15 → 15 | 18.8 → 20.8 | 4.0 → 2.0 |
| 16653-2d1c1d6f | 43 → 43 | 11.4 → 11.4 | 0.1 → 0.1 |
| 16653-71d4fea8 | 48 → 48 | 11.1 → 12.1 | 1.0 → 0.0 |
| 16982-9327edcd | 0 → 0 | 0.0 → 0.0 | 0.0 → 0.0 |
| 16983-43969b02 | 0 → 0 | 0.0 → 0.0 | 0.0 → 0.0 |

16498's remaining two cells are Palmeiras/17 and Flamengo/14. Current 16653 has
no undecideds in any tested seed. Earlier 16653 still has team 95/5 at seed 818;
the other nine seeds have none. Dense controls already have every cell positive.

### Full HTTP latency: close to flat, with an explicit measured increase

Ten fixed seeds per snapshot; milliseconds. Paired change is the median of
individual ratios, not the ratio of the two medians. Summed change aggregates
request time per input.

| Input | Baseline median ms | Candidate median ms | Median paired change | Summed change |
|---|---:|---:|---:|---:|
| 16498-44eabb47 | 819.141 | 792.565 | -1.198% | -0.261% |
| 16653-2d1c1d6f | 648.968 | 645.718 | -0.206% | +0.054% |
| 16653-71d4fea8 | 664.651 | 669.837 | +0.082% | +0.480% |
| 16982-9327edcd | 291.827 | 292.830 | +0.141% | +0.476% |
| 16983-43969b02 | 285.099 | 289.188 | +1.289% | +1.226% |

Total client HTTP time: **26.834 → 26.898 seconds (+0.239%)**.
Total child CPU: **65.203 → 66.018 seconds (+1.250%)**.
The four-worker ceiling remains unchanged. No one-second target or additional
allowance is inferred. This build is **not recommended for default production
adoption under the original no-latency-increase constraint**.

The first normal HTTP build measured +1.963% aggregate over 50 pairs. A follow-up
using the same executable with the flag off/on measured +1.024% over 25 pairs;
dense controls also fluctuated although completion never runs there. Isolating
helper verification/completion with `cold` / `inline(never)` did not remove the
full-request regression (+2.187%). A focused adjacent-CDF lookup optimization
then reduced the final comparison to +0.239%. It resolves the shared CDF row once
and retains floating-point arithmetic order exactly. All earlier results are
retained, not discarded in favor of the final timing sample. Code layout and host variation are
possible contributors; causality was not established. Probability workload and
results were unchanged, but that does not waive the latency constraint.

### Actual reachability / completion cost

Reachability sums early proofs, combined neighborhood/construction, later walk
and deferred publication. Point-tilt's embedded reuse remains inside point tilt.

| Input | Baseline reachability median ms | Candidate reachability median ms |
|---|---:|---:|
| 16498-44eabb47 | 19.818 | 19.525 |
| 16653-2d1c1d6f | 8.111 | 8.253 |
| 16653-71d4fea8 | 12.287 | 12.170 |
| 16982-9327edcd | 0.013 | 0.014 |
| 16983-43969b02 | 0.015 | 0.015 |

Separate traced seed-808 diagnostics of the isolated-helper build before the CDF
optimization (not the final HTTP latency gate):

| Input | Constructor nodes before → after | Constructor ms before → after | Goal attempts | Goal completion ms |
|---|---:|---:|---:|---:|
| 16498 | 2,573 → 2,507 | 4.223 → 3.867 | 10 | 0.037 |
| 16653 earlier | 1,211 → 1,211 | 2.031 → 2.071 | 8 | 0.016 |
| 16653 current | 1,751 → 1,725 | 3.854 → 3.744 | 15 | 0.030 |

These costs include interval construction and sorter verification, not just
symbolic arithmetic. No external model/setup cost is introduced. Earlier 16653
has no new proof and a small reachability-cost increase; work is not universally
saved. Full-request measurements remain the gate despite the cheap direct step.

Coverage/time (`400 - undecided` over complete request seconds) also includes
previously known cells. Across the final suite, 30 new certificate cell-runs over
26.898 candidate request-seconds give **1.12 additional certificates per request
second**; this is not an estimate-coverage rate and does not justify extra latency.

Machine-readable evidence: [all 183 paired requests](results/2026-09-30-rust-goal-completion.json).
Raw requests, outputs and logs remain in the temporary experiment directory.

### Enable only for experimental use

Use the previous best configuration plus the new flag:

```sh
RUST_ODDS_WITNESS_MODE=deferred \
RUST_ODDS_AGGREGATE_CUTS=early \
RUST_ODDS_PROOF_RECYCLE_CREDIT=legacy \
RUST_ODDS_GOAL_COMPLETION=1 \
odds-rust/target/release/golaberto-odds estimate \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  /tmp/rust-goal-16498.json 808 4
```

Without the new flag, the existing algorithm remains selected. Implemented proof
completion is useful and correct on these tests, but the full-request latency
gate remains unmet. No production default was changed.

## Correctness tests and limitations

**29 Rust tests passed (2 unit + 27 integration).** Tests cover the saved Palmeiras candidate, exact intermediate rank construction,
refusal to move a rank beyond the points/wins cohort, equal goals on a draw,
shared-endpoint coupling, and deferred labels/probabilities. Exhaustively enumerate
all 729 W/D/L assignments for a four-team league under three phase orders;
independently replay every returned compact certificate through `Model::add`
and the production sorter to verify all actual ranks.

This is a sufficient constructive heuristic, not a complete goal-assignment
solver. Its failures, unsupported prefixes, exhausted algebraic trials and unsafe
integer sizes remain undecided. A feasible score interval alone is never accepted
as a proof. Witnesses do not imply useful sampling probability: extreme scores
are certificate material, not importance proposals.

The original scientific sources and their limits are documented in the
[reachability research report](2026-09-30-rust-reachability.md#scientific-context).
This implementation needs no new external solver or scientific performance claim.

```sh
cargo test --offline --release --manifest-path odds-rust/Cargo.toml
cargo check --offline --manifest-path odds-rust/Cargo.toml
cargo fmt --manifest-path odds-rust/Cargo.toml -- --check
git diff --check
```

Nothing was committed or pushed. Production defaults remain unchanged.


## Release decision (2026-09-30)

After reviewing the +0.24% wall-time / +1.25% CPU result, the user authorized
“Enable and ship it.” Deferred witnesses, early aggregate cuts, legacy recycling
credit and verified goal completion now run by default. This authorization
accepts the reported tradeoff; it does not change the measurements above or
establish strict latency non-increase. Four workers and existing quotas remain.

Rebuild with `cargo build --release --locked --manifest-path odds-rust/Cargo.toml`
and restart the Rust service. No new flags or JavaScript build are required.
`RUST_ODDS_GOAL_COMPLETION=0` disables score completion; the README documents the
complete original-pipeline rollback. Historical harness variants now explicitly
pin their options, and `--variants defaults` tests the new no-flag behavior.


Release verification: all 29 Rust tests, release build, `cargo check`, formatting
and diff checks passed. Five complete four-worker CLI pairs (all frozen snapshots,
seed 808) compared the frozen explicit `goals` configuration with the new no-flag
defaults: complete response JSONs were identical, including probabilities and
reachability labels. This smoke comparison is not a new latency claim; the
50-pair HTTP results above remain the performance evidence.
[Release verification results](results/2026-09-30-rust-release-defaults.json).

```sh
python3 experiments/rare_positions/compare_rust_reachability.py \
  --baseline /private/tmp/golaberto-rust-reachability-2026-09-30/pre-defaults \
  --baseline-variant goals --candidate odds-rust/target/release/golaberto-odds \
  --reference experiments/rare_positions/reference/2026-09-30-hundredfold/reference.json \
  --output /tmp/rust-release-defaults --variants defaults --seeds 808 --no-trace
```
