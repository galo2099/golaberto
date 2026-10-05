# R63: spend fewer draws on low-weight complete-tree leaves

2026-10-04. Experiment complete. The initial recommendation was opt-in;
the user subsequently authorized default enablement. See production adoption below.
Follow-up to R61 and R62; baseline is the same frozen
current Rust binary, SHA256
`09da95531b96998a5de5da7659729b5bfbe31b36648e6eac95abdf2a71f1dbf8`.
Use identical requests, fixed seeds and four workers. No additional work or
latency allowance, commit, push or database mutation.

## Hypothesis

R61's complete tree resolves Palmeiras/14 in group 16498, but its final allocator
starts each of up to 256 leaves with ten draws. Reduce this floor to two, keeping
every stratum represented, and let the existing variance allocation favor useful
families. Compare lower existing bound-pilot draw counts as a separate factor.
No stratum is discarded; full support, production sorting, exact stratum weights,
independent MAIN/CHECK and publication gates are retained. Pilot counts and final
admission remain deterministic. Charge setup and samples against the same bank.

Luna added a default-preserving bounded experiment flag:
`RUST_ODDS_EXPERIMENT_TARGET_TREE_DRAW_FLOOR=2` (default 10, range 2–10).
Source: `odds-rust/src/rare_tail/overflow_trees.rs`. Tests cover positive coverage,
total draw conservation and exact default allocation.

## Paired allocation screen: group 16498, seed 808

Frozen candidate: `2026-10-04-early-rank/data/golaberto-odds-tree-allocation-v2`,
SHA256 `a1767b2e437f7a3ef2f5678ae81408430df033fff51f49cde5a30d179d757790`.
All variants retain three message refinements and 6,000 nominal final draws;
actual paired admission sizes the final batch to the residual bank.

| Leaf floor | Pilot draws/leaf | Audited late credit | Added cells | Modeled added work | Published Palmeiras/14 |
| ---: | ---: | --- | ---: | ---: | ---: |
| 10 | 25 | yes (R61 control) | 1 | 120,572,748 | `3.207e-25` |
| 2 | 25 | yes | 1 | 134,007,276 | `4.298e-25` |
| 2 | 10 | yes | 1 | 119,365,375 | `6.553e-25` |
| 2 | 5 | yes | 0 | 74,323,505 | MAIN gate failed |
| 2 | 2 | yes | 1 | 109,729,501 | `4.420e-25` |
| **2** | **5** | **no** | **1** | **64,791,890** | **`7.542e-25`** |
| 2 | 2 | no | 0 | 48,665,726 | MAIN gate failed |

Reducing the floor alone increases work: draws move to more expensive useful
branches. Reducing pilots frees enough work for the useful family in the original
bank. The selected no-credit variant fits **65,847,660 available units**, leaving
1,055,770 unused. It spends **46.3% less** modeled work than the successful R61
control, without changing the original confirmation capacity or credit policy.

It runs 2,425 final draws per stream. Raw MAIN is `7.541558829774547e-25`, CHECK
`6.149052934441037e-25`, ratio 1.226; ESS 8.775/6.270, MAIN maximum contribution
share 0.232, MAIN relative SE 0.335. Reconciled publication is
`7.541559084745398e-25` probability (`7.542e-23%`). This is a rough estimate, not
a certified probability interval. Full-tree coverage retains 186 target roots
and the complete leaf partition.

The larger same-pilot credited batch fails due to a dominant later MAIN
observation. Fewer draws do not generally improve accuracy. This nonmonotonic
publication result is why the selected variant needs holdouts and retains the
independent check; it is not evidence that a larger sample should be discarded
after inspecting its result.

## Reference and holdout coverage

Six reference snapshots at seed 808 plus group-16498 seeds 1669, 1993, 2281,
2293 preserve all existing positives and their metadata except request work.
Game importance and reachability classifications agree. There are no lost
positive cells, no new undecideds and no changed proofs. All repeated seed-808
runs produce the same new probability and work counts.

| Group, seed 808 | Baseline zeros | Lean zeros |
| --- | ---: | ---: |
| 15902 | 0 | 0 |
| 16413 | 0 | 0 |
| 16498 | 18 | **17** |
| 16653 | 48 | 48 |
| 16982 | 0 | 0 |
| 16983 | 0 | 0 |

The 17 remaining group-16498 default-seed zeros and 48 group-16653 zeros are
proven impossible. The four holdout seeds add no cells; their residual admission
remains a limitation. This experiment gains **one unique cell**, not a count of
repeated cell-runs.

## Timing and CPU: four cores

Five warmed paired rotations, serialized full CLI requests, include one warmup
per arm. Do not combine overlapping parent/child stage timings.

| Arm | Mean full-process wall s | Mean child CPU s | Median tree-stage ms |
| --- | ---: | ---: | ---: |
| Frozen current Rust baseline | 2.103 | 4.097 | absent |
| Candidate, tree disabled | 1.838 | 3.904 | absent |
| R61 tree control | 2.003 | 4.096 | 98.19 |
| Lean tree | 1.969 | 3.920 | **72.45** |

The lean tree is 26.2% faster within the stage. Full-process means are 6.4% lower
wall and 4.3% lower CPU than the frozen Rust baseline in this sample. Relative
to the **same candidate with tree disabled**, enabling lean adds 7.1% mean wall
and 0.4% mean CPU. The baseline/candidate disabled difference is unexplained;
do not attribute it to the heuristic. Individual full-process wall samples range
from about 1.67 to 2.64 seconds, so these means cannot establish a portable
speedup or unchanged endpoint latency.

An additional six-request, single-process check with logs disabled uses the same
seed 808 and four workers each time. The CLI's upper-median calculation reports
1,801 ms baseline and 1,772 ms lean. Total child CPU over six requests is
23.31 s baseline versus 22.11 s lean; total process wall is 10.77 versus 10.61 s.
These sequential batches also remain sensitive to machine load and run order.
No production Xeon was available. No extra latency allowance is assumed.

## Correctness and validation

The floor is selected before sampling. Every stratum keeps at least two draws;
exact outcome prior, stratum likelihood, score likelihood and production sorter
are unchanged. Lower pilots make allocation noisier, but cannot establish a
proof or become final evidence. Fresh MAIN/CHECK batches and original gates
remain mandatory. All setup, pilots, messages and samples are charged. Actual
operation overruns stop the stage and reject publication. Setup and pilot draw
costs remain deterministic accounting proxies rather than CPU upper bounds.

- Full Rust suite: **184 passed, four DB-gated tests ignored**; tests serial,
  Cargo at four jobs. HTTP tests permitted localhost binding; no DB mutations.
- Python paired-harness helpers: nine passed.
- Edited-file rustfmt and `git diff --check`: pass.
- Independent Sol review: no support/default-parity issue; notes lower per-leaf
  sample sizes can increase variance. Luna made every source/script edit.

## Reproduction and evidence

```sh
cargo build --offline --locked --release -j4 --manifest-path odds-rust/Cargo.toml
RUST_ODDS_EXPERIMENT_TARGET_OVERFLOW_TREE=1 \
RUST_ODDS_EXPERIMENT_TARGET_TREE_DRAW_FLOOR=2 \
RUST_ODDS_EXPERIMENT_TARGET_TREE_PILOT_DRAWS=5 \
RUST_ODDS_EXPERIMENT_TARGET_TREE_RECLAIM=0 \
  odds-rust/target/release/golaberto-odds estimate \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  /tmp/lean-tree.json 808 4
```

Keep `RUST_ODDS_EXPERIMENT_EARLY_RANK` unset: enabling it replaces the tree with
the unsuccessful R62 ranked proposal. For the paired harness, use R62's baseline
path/hash and candidate V2 above. Pass the four flags above through
`--arm lean:KEY=VALUE,...`. All exact commands, binary/request/export hashes and
wall/child-CPU measurements are saved in `2026-10-04-early-rank/`:
`allocation-screen/`, `allocation-reference/`, `allocation-holdouts/`,
`allocation-timing/`, and `probes/*-bench.*`.

Primary implementation sources: `rare_tail/overflow_trees.rs` for admission and
allocation; `joint_caps/propagated/lazy/strata/tree.rs` for the complete outcome
partition; `strata.rs` for weighted combination; `sort.rs` for actual rankings.
The R61 report documents the parent-retention and complete-support assumptions.

## Initial experiment recommendation

Use the lean variant for further target-overflow experiments. It recovers this
case inside the original bank with lower stage cost and no measured coverage
regressions. Keep it **opt-in**: full-request timing preservation is not established
well enough for automatic production adoption, and holdout reachability coverage
has not improved. Do not adopt R62 in place of the tree. No commit, push or
production-default change.

## Production adoption after user authorization

The user subsequently requested default enablement. Coverage now enables the
lean fallback with pilot 5, leaf floor 2, three messages, nominal final cap 6,000
and reclaim disabled. There is no increase to confirmation-bank capacity.
`RUST_ODDS_TARGET_OVERFLOW_TREE=0` opts out; the production setting takes
precedence over the legacy experiment toggle. Other profiles remain opt-in.
The unsuccessful R62 replacement remains off. Startup and request logs expose
the effective settings, and branch draw auditing uses the same effective toggle.

Production validation uncovered two existing setup overruns in the measured
lean configuration. At holdout seeds 1669 and 1993, construction consumed
12,681,719 units against respective grants of 6,450,146 and 5,680,871; neither
published an estimate. A generic admission floor now requires the measured
per-cell root-count work plus the configured 100,000-node/4,000,000-guide quota
cost (9,600,000 units) before construction. These cases need 12,391,031 units
and are skipped before building. This is an allocation policy, **not a rigorous
setup upper bound**: rank-hint work is additional and remains subject to the
existing measured overrun checks. Exhaustion remains undecided.

Frozen adoption binary:
`2026-10-04-early-rank/data/golaberto-odds-tree-production-v2`, SHA256
`ae0e68a5d5986436656ca34aa6b6b6a9f54befeaaf96049dfbc0df96ab6bbdf1`.
The frozen pre-adoption default is the allocation V2 binary above, SHA256
`a1767b2e437f7a3ef2f5678ae81408430df033fff51f49cde5a30d179d757790`.

### Paired adoption results

All six seed-808 default exports are byte-for-byte identical to the previously
measured explicit lean configuration. All ten opt-out exports (six references
and four holdouts) are byte-for-byte identical to the frozen pre-adoption default.
Existing positive metadata, proofs and game importance are preserved. The only
gain is Palmeiras/14th; all five warmed repetitions reproduce the same estimate
and 64,791,890 units of tree work. No new holdout estimates are claimed.

The two admission skips preserve all non-work output while removing 12,681,719
units of wasted setup per request. Every observed tree stage now fits its
residual bank, with no reclaimed credit or capacity increase.

| Group 16498 seed | Tree work before guard | Tree work with guard | Added cells |
| ---: | ---: | ---: | ---: |
| 808 | 64,791,890 | 64,791,890 | 1 |
| 1669 | 23,842,661 | 11,160,942 | 0 |
| 1993 | 18,262,631 | 5,580,912 | 0 |
| 2281 | 53,466,468 | 53,466,468 | 0 |
| 2293 | 2,789,881 | 2,789,881 | 0 |

Five warmed, rotated full-request timing pairs, four workers, one warmup per arm:

| Arm | Mean full-process wall s | Mean child CPU s |
| --- | ---: | ---: |
| Frozen pre-adoption default | 3.531 | 5.840 |
| New default | 3.937 | 6.020 |
| Same new binary, opt-out | 3.495 | 5.839 |

Enabling the new default adds **12.7% mean wall time and 3.1% mean CPU** versus
its opt-out in this sample. Median tree-stage wall time is 152.77 ms. Default
full-process samples range from 3.11 to 5.87 s, and the machine experienced heavy
memory compression during release validation. The single-pass reference and
holdout timings also vary substantially. These measurements do not establish
unchanged production latency or a portable slowdown. The earlier 7.1%/0.4%
sample remains historical evidence; neither sample licenses an extra work-budget
allowance. The user authorized adoption, and this report explicitly records the
observed overhead.

Raw commands, request/binary/export hashes, operation logs and timing records
are in `2026-10-04-early-rank/production-reference-v2/`,
`production-holdouts-v2/` and `production-timing-v2/`. The first validation
without the admission guard is retained in `production-reference/` and
`production-holdouts/` rather than overwritten.

```sh
python3 experiments/rare_positions/2026-10-04-target-overflow/scripts/run_overflow.py \
  --baseline experiments/rare_positions/2026-10-04-early-rank/data/golaberto-odds-tree-allocation-v2 \
  --baseline-sha256 a1767b2e437f7a3ef2f5678ae81408430df033fff51f49cde5a30d179d757790 \
  --candidate experiments/rare_positions/2026-10-04-early-rank/data/golaberto-odds-tree-production-v2 \
  --output /tmp/tree-adoption-reference \
  --groups 15902,16413,16498,16653,16982,16983 --seeds 808 \
  --arm default:RUST_ODDS_LOG=1 --arm optout:RUST_ODDS_TARGET_OVERFLOW_TREE=0
```

Use a fresh output directory per invocation. For holdouts pass group 16498 and
seeds `1669,1993,2281,2293`; for the timing run pass group 16498, seed 808,
`--repeats 5 --warmups 1`. Logging is the only explicit setting in the default
arm; there are no enabling or tuning flags.

The full Rust suite passes **188 tests**, with four DB tests ignored. It covers
HTTP default enablement, a production opt-out overriding legacy opt-in,
repeatability, native-estimate preservation, admission boundaries and saturated
accounting. Build concurrency was reduced after local memory compression slowed
release linking; endpoint checks and experiments retain four workers.

```sh
cargo test --offline --locked --release -j1 \
  --manifest-path odds-rust/Cargo.toml -- --test-threads=1
cargo build --offline --locked --release -j1 \
  --manifest-path odds-rust/Cargo.toml --bin golaberto-odds
odds-rust/target/release/golaberto-odds estimate \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  /tmp/default-tree.json 808 4
```

Remove earlier experimental tuning overrides to use these defaults. Rebuild and
restart the Rust service to activate the change in a deployed endpoint.
After validation, the user authorized committing and pushing the implementation,
reproduction tools and reports. Frozen executables and raw run directories remain
local artifacts and are excluded from publication. No database mutation was performed.
