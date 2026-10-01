# Broader joint conditioning with correct weighting in Rust

Started September 30; final allocation validated October 1, 2026.

## Decision

Enable the broader fallback by default, retaining the exact extreme-pattern fast path. The old extreme-target/ahead-set gate no longer prevents a joint proposal for other selected cells. No team or position constants are introduced.

Against the current Rust baseline, **30 paired requests gain 34 nonzero cell-runs**: seven snapshot cells, or **six distinct group/team/rank combinations** because team 22/17 appears in both snapshots of group 16653. No existing nonzero cell, reachability proof, or impossible classification disappears. All gains fill already proved reachable zeros; this change adds no impossibility proofs and does not resolve the remaining undecided cell-runs.

The baseline includes the preceding exact-cap production integration. These are incremental results; Chapecoense/3rd was already nonzero in that baseline. The user authorized shipping on October 1. This report describes the tested configuration; shipping does not restart a running deployment. No database mutation or Rails/UI change was made by this task.

## Generic proposal

1. Derive a safe union of target final totals. A total is allowed when independent minimum/maximum rival totals do not already force too many teams above or below the requested rank. Use packed points/wins only when wins immediately follow points in the phase sort. Allow packed ties; actual later tiebreakers decide them.
2. Compute per-team independent total PMFs. Approximate the requested rank's conditional likelihood with a Poisson-binomial calculation, assigning half of each packed tie to either side. This approximation selects an anchor and a soft target-total tilt; **it is never the final probability estimate**.
3. Prefer plausible rival ahead/below assignments around that anchor and select up to five tight rival floors/caps. Include the target. Enumerate selected-versus-selected fixture outcomes once; use backward DPs for each selected team's disjoint external fixture stream. Reduce the selected set until internal enumeration fits 20,000 states. Thus shared matches retain their dependence.
4. Mix 90% tilted joint sampling with 10% target-only sampling. The latter covers every allowed target total and all alternative rival assignments, including seasons outside the preferred joint region. Guide remaining fixtures toward the requested rank, correcting every proposal draw. A 1e-6 base component preserves residual outcome support when floating-point guidance rounds to zero; an empty guide falls back to the true fixture probabilities.
5. Sample actual scores conditional on outcomes and check the requested rank with the production sorter. Only verified hits contribute. Complete sampled seasons establish reachability; a witness alone supplies no probability estimate.

The model supports ordinary 3/1/0 scoring, zero bonus points, and points first. Wins are packed only if second. Unsupported, oversized, or numerically empty models skip this proposal and retain ordinary search. Setup also has a 4,096 packed-gain span limit. These limits and empty proposals never prove impossibility.

## Likelihood ratios

Let `s` denote the selected fixture outcomes, `P(s)` their true joint probability, `A` the necessary target-total union, and `M=P(A)`. Let `F(s)` be the soft target tilt multiplied by the rival floor/cap indicators, and `Z=E_P[F]`. The joint DP computes `Z` exactly for this selected model. **Z is a weighted normalizer, not a probability upper bound.**

On `A`, with `a=0.1`:

```text
q(s) = P(s) × [a/M + (1−a) F(s)/Z]
P(s)/q(s) = M × Z/[a Z + (1−a) M F(s)]
```

When `F=0`, the conditional weight is `1/a`, multiplied by `M`. The implementation uses the density of the entire overlapping mixture, regardless of which component generated the season. Using only the selected component's density, or reporting `Z × hit frequency`, would omit valid seasons or double-count overlap.

Each residual outcome adds its true/proposal probability ratio, including the defensive residual mixture. Scores are sampled from the unchanged conditional score model and need no further proposal ratio. Raw importance estimates therefore target the full rank event, not just the preferred rival configuration. Existing matrix reconciliation is retained afterward; that approximate balancing operation is distinct from raw likelihood-ratio correctness.

## Allocation and variants

The same four existing extra-search candidates and four workers are used. For a broad proposal, forecast ordinary hits as `50000 × max(main_probability, check_probability) / ordinary_event_mass`. Keep 43,000 ordinary draws if that forecast is at least 0.01; otherwise keep 35,000. The 0.01 threshold is a workload heuristic, not a claimed bound: it protects potentially useful ordinary hits while shortening batches forecast to provide almost none. The independent main/check sample counts are fixed; there is no adaptive stopping of the reported importance estimate. Budgets remain bounded; no one-second allowance or older Go timing is used.

| Proposal | Main | Independent check | Ordinary | Total draws |
|---|---:|---:|---:|---:|
| Broad, ordinary-hit forecast ≥0.01 | 1,500 | 1,500 | 43,000 | 46,000 |
| Broad, ordinary-hit forecast <0.01 | 1,500 | 1,500 | 35,000 | 38,000 |
| Exact fast path, accepted | 5,000 | 2,000 | 10,000 | 17,000 |
| Exact fast path, rejected | 5,000 | 2,000 | 43,000 | 50,000 |
| Unsupported proposal | 0 | 0 | 50,000 | 50,000 |

Draw counts are not interchangeable CPU costs. Actual setup, stage, request, CPU and RSS measurements are reported below. Main acceptance retains the existing coarse checks: at least 30 hits, ESS at least 8, relative SE at most 35%, maximum event-weight share at most 25%, and batch gap at most 1. The independent confirmation requires at least 30 hits, ESS at least 5, relative SE at most 60%, and estimates within a factor of ten. No threshold was weakened to obtain the compact result.

| Variant | Observation | Decision |
|---|---|---|
| Allowed target union without a target tilt | No extra estimates in 30 full-request pairs | Reject |
| Improved anchor without a tilt | No accepted standalone estimates across six difficult cells × six seeds | Reject |
| Soft tilt, 5,000+2,000 draws, ordinary 10,000 on acceptance | 34 gains but two lost existing estimates | Reject |
| Same, with a separate planning-priority proxy | Both regressions persisted | Reject |
| Soft tilt, 5,000+2,000 draws, ordinary 43,000 | 34 gains, no losses; persistent latency rose 3.52% for 16498 and 3.46% for historical 16653 | Reject allocation |
| Soft tilt, 1,500+1,500 draws, ordinary 43,000, full-support residual weights | Same 34 gains, no losses; two timing repeats pooled to +1.28% wall/+1.53% CPU for 16498 | Reject fixed allocation |
| Same, ordinary 35,000 for every broad cell | Lost current-16653 team 12/17 at seed 818 | Reject |
| Same, ordinary 35,000 or 43,000 using the generic hit forecast | 34 gains, no losses, lower active-snapshot CPU; final timing below | Adopt |

The shortened ordinary batches lost late hits and useful seeds, changing later neighborhood and point-tilt allocation. A scheduling score alone did not preserve that information. Selective retention of 43,000 ordinary draws fixes both measured regressions: historical group 16653, seed 808, team 95/4th; current group 16653, seed 818, team 12/17th. Joint estimates are still published after existing witness/probability searches and before reconciliation.

## Paired coverage

Seeds: `801, 804, 808, 817, 818, 911`. All probability values below are **fractions, not percentages**. Historical group 16653 is a separate request snapshot, not an extra group.

| Snapshot | Team ID | Rank | Newly nonzero runs | Probability range |
|---|---:|---:|---:|---|
| 16498 | 17 | 9 | 4/6 | 2.041e-14–2.365e-14 |
| 16498 | 8 | 19 | 6/6 | 1.577e-17–2.017e-17 |
| 16498 | 17 | 10 | 2/6 | 8.886e-17–1.025e-16 |
| 16653 historical | 12 | 18 | 5/6 | 7.019e-14–7.725e-14 |
| 16653 historical | 22 | 17 | 5/6 | 1.136e-12–1.290e-12 |
| 16653 historical | 95 | 2 | 6/6 | 1.397e-20–1.960e-20 |
| 16653 current | 22 | 17 | 6/6 | 3.026e-12–3.761e-12 |

Groups 16982 and 16983 already have every cell nonzero in these requests and gain none. Game importance is identical in every pair. Existing positive probabilities and standard errors are not all bit-for-bit identical: smaller ordinary batches can alter later allocation, and newly populated cells affect balancing. The two regression tests check preservation of positives/proofs and changes within the stated uncertainty tolerance; the prior exact fast-path test still checks its prior estimates and SEs exactly.

There is no independent high-budget ground truth for the newly estimated e-20/e-17 events. Independent streams and repeated seeds support rough orders of magnitude, not certified tiny-event accuracy. Defensive support makes the likelihood ratio correct; it does not guarantee that a finite run observes every rare, high-weight contribution. Failed acceptance leaves the cell to its existing estimators.

## Timing and resource usage

Final normal binary versus the frozen Rust baseline; serial requests, four estimator workers, alternating baseline/candidate order. Persistent measurements use two warmups and eight timed pairs per snapshot. CPU is process user+system time across all ten requests, startup and warmups included. Peak RSS is whole-process peak, not incremental sampler memory.

| Snapshot | Baseline median ms | Candidate median ms | Wall change | CPU seconds | CPU change | Peak RSS MiB |
|---|---:|---:|---:|---|---:|---|
| 16498 | 668.03 | 640.01 | -4.19% | 18.94 → 18.04 | -4.75% | 172.2 → 176.6 |
| 16653 historical | 702.51 | 689.04 | -1.92% | 15.68 → 15.18 | -3.19% | 171.5 → 163.8 |
| 16653 current | 720.65 | 728.83 | +1.14% | 21.18 → 20.43 | -3.54% | 181.7 → 185.7 |
| 16982 | 310.10 | 316.00 | +1.90% | 6.56 → 6.59 | +0.46% | 24.8 → 24.6 |
| 16983 | 311.71 | 305.82 | -1.89% | 6.47 → 6.41 | -0.93% | 23.6 → 24.5 |

Final persistent stage medians exclude warmups. Joint setup/sampling ranges include every candidate attempt in the ten requests; their sampling time includes setup and both streams. Parallel per-cell durations must not be summed as stage wall time.

| Snapshot | Extra stage ms | Point-tilt stage ms | Broad setup range ms | Broad setup+sampling range ms |
|---|---|---|---|---|
| 16498 | 93.83 → 94.26 | 350.15 → 325.65 | 0.91–2.97 | 6.07–15.31 |
| 16653 historical | 188.76 → 184.92 | 238.58 → 234.72 | 0.66–1.55 | 6.57–12.04 |
| 16653 current | 178.82 → 181.97 | 187.92 → 192.73 | 0.48–2.35 | 4.02–15.44 |

All three active snapshots use less process CPU in the final comparison. Group 16498 and historical group 16653 are faster; current group 16653 has a **1.14% wall increase**, despite lower CPU. The inactive 16982 control has a **1.90% wall increase**; 16983 is faster. Thus exact no-increase latency in every snapshot is not established. No extra allowance is assumed: the algorithm replaces existing work and reduces total draws, and these observed increases remain explicit limitations. Current-16653 paired differences include large scheduling variation (−267 to +121 ms). A fixed-seed 10,000-resample bootstrap of its eight paired differences gives a 95% median-difference interval of roughly −108 to +71 ms; this is uncertainty, not proof of zero overhead.

Some full-request savings come from changed downstream allocation, not just from faster joint sampling. Performance is not a universal speedup guarantee. **Cold six-seed request medians also show increases**, as recorded explicitly below; their timings vary with process startup, request seed and scheduling. Inactive-control increases are included, not hidden as an assumed time allowance.

| Snapshot | Baseline cold median ms | Candidate cold median ms | Change |
|---|---:|---:|---:|
| 16498 | 832.71 | 819.84 | -1.55% |
| 16653 historical | 719.52 | 714.15 | -0.75% |
| 16653 current | 723.04 | 729.90 | +0.95% |
| 16982 | 304.17 | 310.58 | +2.11% |
| 16983 | 303.20 | 306.11 | +0.96% |

The earlier compact run, before numerical-support protection, measured active persistent changes of −2.88%, −2.80%, and −4.33%; inactive controls increased 0.66% and 0.62%. Its cold current-16653 median increased 5.78%. The 5,000+2,000/43,000 prototype increased historical-16653 CPU 5.54%. All variant measurements are retained in the results JSON. Adoption uses the final measured configuration rather than assuming those slower variants fit the production budget.

## Validation and reproduction

The full Rust release suite passes **65 tests**, with two database tests ignored. New tests exhaustively enumerate 729 shared-fixture seasons for four-team models, both points-only and points/wins sorts, every target/rank; check safe target support, exact normalizers, complete mixture normalization, and weighted rank expectation, including hits excluded by the joint component. Sampling tests cover guided residual weights and real goal difference/goals scored tiebreakers. Production regression tests reproduce both lost-cell seeds, test default enablement and actual draw counts, and preserve existing positive/proven cells and game importance.

No CP-SAT/SCIP dependency, proof budget increase, score-cap impossibility claim, or intermediate-rank inference is introduced. The broader fallback remains limited to the four candidates chosen by the existing extra scheduler; it does not search every zero. Standalone 5,000-draw probes also found promising estimates for Chapecoense/4th (~7e-20) and /5th (~1.7e-15), but those cells are not selected by this allocation. They are future allocation opportunities, not claimed production gains.

```sh
cargo build --release --offline --locked --manifest-path odds-rust/Cargo.toml \
  --bin golaberto-odds --example joint_cap_conditioning -j4
cargo test --release --offline --locked --manifest-path odds-rust/Cargo.toml \
  -j4 -- --test-threads=1

# Reproduce the cohort with the same executable, disabling broad conditioning
# for the baseline and leaving all sampler enable flags unset for the candidate.
python3 experiments/rare_positions/benchmark_joint_cap_requests.py \
  --baseline odds-rust/target/release/golaberto-odds \
  --candidate odds-rust/target/release/golaberto-odds \
  --broad-candidate --output /tmp/broad-joint-pairs
python3 experiments/rare_positions/benchmark_joint_cap_requests.py \
  --baseline odds-rust/target/release/golaberto-odds \
  --candidate odds-rust/target/release/golaberto-odds \
  --broad-candidate --persistent --output /tmp/broad-joint-persistent

# Exploratory probe: six fixed streams, four workers, no production confirmation.
odds-rust/target/release/examples/joint_cap_conditioning \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  318 4 5 5000 broad

# Normal service; no enabling flags needed.
odds-rust/target/release/golaberto-odds serve 127.0.0.1:6577
```

The benchmark clears inherited estimator flags. `RUST_ODDS_JOINT_CAP_BROAD=0` retains the prior exact sampler; `RUST_ODDS_JOINT_CAP_CONDITIONING=0` disables both. The cohort is pinned to the five requests above even as the canonical golden reference is refreshed. The historical request now lives in `odds-rust/tests/fixtures`; its SHA-256 matches the earlier experiment. Input/binary hashes, every paired result, rejected variants and resource data are saved in [machine-readable results](results/2026-09-30-rust-broad-joint.json).

Implementation and evidence: [broad model](../../odds-rust/src/joint_caps/broad.rs), [exact/fallback dispatcher and confirmation](../../odds-rust/src/joint_caps.rs), [search integration](../../odds-rust/src/search.rs), [production regression tests](../../odds-rust/tests/joint_caps.rs), [paired benchmark](benchmark_joint_cap_requests.py), and [preceding Rust baseline report](2026-09-30-rust-joint-caps.md).
