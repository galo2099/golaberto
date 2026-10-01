# Increasing the conditional fixture-probe budget

Date: 2026-09-30. Follow-up to the
[initial proof-guided sampler experiment](2026-09-30-rust-domain-probes.md).
All variants remain disabled by default. No native code was changed for this
budget sweep; the existing experimental sampler already supports these limits.
No commit, push, or database mutation was made.

## Result

Increasing the hypothesis budget from **6 to 24, 96, and 384 per sampler call**
produced more verified fixture pruning, but **zero additional estimates**.
The 90 proof-only full responses matched the current Rust baseline exactly,
including probabilities, uncertainty, matrix, reachability, game importance,
and work accounting. There were no nonzero-cell regressions.

**Recommendation: do not enable any of these larger budgets.** The measured
pruning is not improving published probability coverage under this allocation.

Across the same five snapshots and six seeds:

| Hypotheses per call | Removed outcomes | Newly forced fixtures | New estimated cell-runs | New reachable / impossible cells |
|---|---:|---:|---:|---:|
| 6, prior experiment | 22 | 10 | 0 | 0 / 0 |
| 24 | 50 | 22 | 0 | 0 / 0 |
| 96 | 90 | 38 | 0 | 0 / 0 |
| 384 | 374 | 115 | 0 | 0 / 0 |

Removed/forced counts sum repeated sampler calls, conditional states, and seeds;
they are not counts of distinct fixtures or team/rank cells. The 64-fold increase
from 6 to 384 allowed more outcomes to be screened, not a deeper per-outcome
proof search. The exemption-search limit remains **16 nodes per direction**.

## Protocol

- Current Rust baseline, already including default discrete cuts; four workers.
- Five frozen reference snapshots: 16498 `44eabb47`, earlier 16653 `2d1c1d6f`,
  current 16653 `71d4fea8`, 16982 `9327edcd`, 16983 `43969b02`.
- Identical requests and fixed seeds **801, 804, 808, 817, 818, 911**.
- 30 alternating complete HTTP pairs per variant, four variants: 120 pairs.
- One active request at a time, no simultaneous tests or builds.
- Two further persistent-server measurements at 384 probes, seed 808: compare
  the frozen baseline and candidate, then use the same candidate binary for
  both arms with probes disabled versus enabled.
- Persistent measurements cover the three snapshots with domain-rescue work.
  Each server has two warmups; frozen comparisons have six timed requests,
  same-binary comparisons have eight.
- Existing scout/pool and all other search limits are retained. The proof-only
  variants keep pilot/final/check draws at 1,000/15,000/5,000.

The budget is shared across cached conditional states **per sampler call**,
not applied separately to every season or cache entry. Existing cache size
remains 64; domain rescue still limits candidate cells and finalists as before.
Some calls use less than their configured budget when eligible outcomes or
cached states run out.

The baseline executable SHA-256 is `94283bee7d957029779cc148563671bdc29aa6b9251a661adb6c0e253cf559fe`; candidate SHA-256 is `e5e8ceaa858863f6b200928ad86f6a478bab1b789f2d6bd811dcc948aa96de4b`.
No Go timings or one-second allowance are used.

## Pruning and actual proof work

| Variant | Hypotheses tried | Exemption nodes | Removed outcomes | Newly forced fixtures | Entire conditional states rejected |
| 24 probes | 2,880 | 12,030 | 50 | 22 | 11 |
| 96 probes | 11,520 | 53,503 | 90 | 38 | 11 |
| 384 probes | 42,831 | 216,033 | 374 | 115 | 45 |
| 96 + fewer draws | 11,904 | 54,655 | 90 | 38 | 11 |

Conditional-state rejection does not establish global impossibility of a cell.
These counts refer only to the fixed target-outcome pattern being sampled.
Model construction and local propagation are included in the probe timers;
exemption-node counts do not measure graph operations or CPU directly.

| Snapshot | Outcomes removed at 24 | At 96 | At 384 |
| 16498 | 0 | 0 | 0 |
| Earlier 16653 | 50 | 60 | 218 |
| Current 16653 | 0 | 30 | 156 |
| 16982 | 0 | 0 | 0 |
| 16983 | 0 | 0 | 0 |

No domain rescue runs for 16982 or 16983 in these requests, because their matrices
already have estimates in all 400 cells. They remain controls. Even 384 probes
remove no outcomes in 16498's attempted calls. Its remaining undecided and
reachable-zero cells are unchanged. Current 16653 gains pruning at 96 and 384,
but still gains no accepted probability estimates.

## Reallocation and ablation

A fourth variant uses 96 probes while reducing existing domain-rescue draws:
**pilot 1,000→750; final 15,000→14,000; check stays 5,000**.

It gains **one distinct cell in two cell-runs**, team 9 finishing 14th in the
earlier 16653 snapshot, seeds 801 and 808. Every one of its 30 full responses is
identical to the previously measured draw-only ablation at the same requests
and seeds. These gains come from changed simulation allocation/acceptance, not
from the additional proofs. There are no losses or new reachability labels.

This reallocation is not guaranteed to reduce actual work: different pilot
lengths can change which finalists qualify. Larger proof budgets were not paid
for by assuming any new production time allowance.

## Wall time and CPU

Across 30 cold-server HTTP pairs per variant:

| Variant | Median paired full-request change | Change in summed full-request times | Change in summed process CPU |
| 24 probes | +0.51% | -0.32% | -2.12% |
| 96 probes | -1.00% | -1.15% | -1.53% |
| 384 probes | +2.13% | +2.53% | +2.15% |
| 96 + fewer draws | -1.14% | -1.84% | -1.51% |

The largest budget increases summed full-request wall time **2.53%**, and CPU
**2.15%**, in these cold comparisons. At lower budgets, variation in unchanged
stages is larger than the added proof time; negative totals are not evidence
that probing itself saves time.

Median domain-stage increases across the six seeds:

| Snapshot | 24 probes, added ms | 96 probes, added ms | 384 probes, added ms |
| 16498 | 1.12 | 4.57 | 15.40 |
| Earlier 16653 | 2.82 | 7.76 | 32.11 |
| Current 16653 | 2.60 | 9.33 | 37.94 |

### Persistent requests using the same binary

Both arms use the candidate executable. Only the hypothesis-budget flag differs:
0 versus 384. This removes executable-layout differences as a confound.

| Snapshot | Full HTTP median, probes off ms | Probes on ms | Full change | Domain stage off ms | Domain stage on ms | Process CPU change |
| 16498 | 715.96 | 705.28 | -1.49% | 4.07 | 22.71 | -3.96% |
| Earlier 16653 | 704.51 | 697.97 | -0.93% | 4.97 | 23.96 | +0.69% |
| Current 16653 | 720.12 | 714.31 | -0.81% | 55.64 | 86.80 | -0.05% |

The added domain stage is approximately **19, 19, and 31 ms**, respectively.
The summed full-request medians nevertheless decrease 1.08%. Earlier unchanged
point-tilt stage medians are about 30, 12, and 33 ms lower in the probe arm.
That stage runs before domain probes; these measurements do not establish a
reliable overall speedup caused by probing. Repeated requests also reuse process
and allocator state. Full-request variation masks the consistent additional
cost within the domain stage.

For completeness, the frozen-baseline persistent comparison is also in the
result JSON. Its full-request changes were −6.50%, −2.64%, and +4.94% for 16498,
earlier 16653, and current 16653. Current 16653 therefore did show a full-request
latency increase in that comparison. Neither measurement protocol showed a
coverage gain.

CPU figures are process user+system time, including startup and warmup requests.
Stage figures are wall time; summed parallel probe timers are not CPU readings.
Peak RSS, per-request latencies, and all stage/probe records are retained in
[the results JSON](results/2026-09-30-rust-domain-probes-larger.json).

## Interpretation and recommendation

More hypotheses find constraints that the six-probe budget misses, including
in the current 16653 snapshot. However, published estimates still do not change,
even when the larger budget compacts additional forced fixtures. Simply
increasing this budget is not effective for the tested requests and seeds.

One plausible limitation is allocation: the budget is consumed on early cache
states and ordered fixtures, whereas the event's useful seasons might lie in
other conditional states. Another is that W/D/L feasibility pruning does not
actively steer toward a complete target-rank season or favorable goal
conditions. These are hypotheses, not established diagnoses from this sweep.
The remaining proof budget and cache coverage are still finite; this is not an
exhaustive search or evidence that all stronger constraint approaches fail.

If pursuing this mechanism further, measure recurrence and failed-sample cost
of each conditional state, then replace low-value probes with checks in states
that actually dominate failed estimation. Keep the existing budgets until such
a change shows coverage gains within current full-request latency.

## Correctness and validation

Native implementation and acceptance checks are unchanged from the initial
experiment. Relaxation infeasibility may remove an outcome; feasibility and
budget exhaustion retain it. Fixed results are applied exactly once, raw point
decoding uses Euclidean division for negative adjustments, and nonstandard
scoring/bonus phases skip probing. Goal tiebreakers are left free in the necessary
relaxation. Actual witnesses are still checked by the production sorter.

The initial experiment's exhaustive support-retention and weighted-probability
tests still cover this implementation. No new Rust code required new Rust tests
in this flag-only sweep. Native suite from the prior experiment: 47 passed,
two database tests skipped. Additional validation here:

- 90 proof-only responses compared exactly with their paired Rust baselines.
- 30 reallocated responses compared exactly with the saved draw-only controls.
- Repeated fixed-seed responses required to stay identical within each
  persistent arm; game-importance output required identical across arms.
- Python scripts compiled successfully; `git diff --check` passed.

The first screening attempt stopped because the server shutdown truncated its
optional final HTTP log line. Its partial measurements are excluded. The
harness now skips unterminated log records while still requiring a complete
estimator completion event. No estimator request failed.

## Reproduce

Use the current baseline saved before adding the opt-in probe implementation
(the SHA above identifies the executable), and the candidate built for the
initial experiment. An alternative for isolating flag overhead is using the
same current candidate executable for both arms, as in the persistent command.

```sh
python3 experiments/rare_positions/compare_rust_reachability.py \
  --baseline /private/tmp/golaberto-domain-probe-baseline \
  --candidate odds-rust/target/release/golaberto-odds \
  --reference experiments/rare_positions/reference/2026-09-30-hundredfold/reference.json \
  --output /private/tmp/larger-probe-pairs --baseline-variant defaults \
  --variants probe_joint_24,probe_joint_96,probe_joint_384,probe_realloc_96 \
  --seeds 801,804,808,817,818,911 --http --no-trace

python3 experiments/rare_positions/benchmark_domain_probes.py \
  --baseline odds-rust/target/release/golaberto-odds \
  --candidate odds-rust/target/release/golaberto-odds \
  --variant probe_joint_384 --iterations 8 --seed 808 \
  --input experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  --input experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16653-2d1c1d6f.json \
  --input experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16653-71d4fea8.json \
  --output /private/tmp/larger-probe-persistent
```

Actual pair measurements were split into `808,818` and `801,804,817,911`.
For the frozen persistent comparison use the saved baseline instead of the
candidate in `--baseline`, and `--iterations 6`.

To reproduce a single experimental request:

```sh
RUST_ODDS_DOMAIN_PROBES=384 RUST_ODDS_DOMAIN_PROBE_NODES=16 \
  odds-rust/target/release/golaberto-odds estimate REQUEST.json OUTPUT.json 808 4
```

No new production flags are recommended. The proof mechanisms and scientific
background are documented in the initial report and
[discrete-cut report](2026-09-30-rust-discrete-cut.md).
