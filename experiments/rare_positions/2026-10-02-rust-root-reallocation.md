# R51: learn native root allocation from long confirmations

Status: implemented and measured; rejected for production promotion. Production
unchanged; no commit or push. Isolated source patch and raw evidence retained.

## Hypothesis

R50's captured Flamengo/12th contributions concentrated in one native root.
Learning additional fixture modes did not remove the weight concentration and
cost too much latency. Reweight existing roots before rebuilding any conditional
fixture or score proposal. This separates root-selection inefficiency from
variance inside a root.

A root is one outcome pattern for the target team's remaining fixtures. Each
root has a conditional sampler for the other fixtures.

The current implementation already learns root frequencies from short pilots
when explicitly enabled. This experiment uses the much longer, already funded
native confirmation mains. It preserves their original samples and independent
native checks before funding any refinement.

## Statistical design

Observe a fixed prefix chosen before the native main, including zero
contributions and early exits. For each root record its original selection
probability `q0`, draw count, corrected contribution sum and sum of squares,
maximum contribution, and instrumented sampling cost.

For random-root sampling, the allocation score uses raw second moments:

```
moment score = sqrt(q0 × sum_of_squared_contributions)
cost score   = sqrt(q0 × sum_of_squared_contributions / smoothed_root_cost)
```

The quantity `q0 × sum_of_squared_contributions` estimates the root's squared
numerator contribution up to the common fixed training length; the moment score
takes its square root. Subtracting the squared conditional mean would allocate
no work to a deterministic productive root. Within-root variance is useful as a
diagnostic, not as the allocation score for this random-mixture estimator.
Cost adjustment is a heuristic; it is not claimed to solve the exact
variance-per-unit-work optimization.

Normalize scores, mix them equally with the current guided root frequencies,
and retain the existing 10% defensive target-root law. Freeze the resulting
allocation before fresh independent pilot/main/check draws. Their importance
weights use the actual new root-selection probability. Existing conditional
fixture densities, overlapping alternate modes, goal likelihood correction and
the production sorter remain in force.

[Owen's importance sampling chapter](https://artowen.su.domains/mc/Ch-var-is.pdf)
gives the support requirement and second-moment variance identity underlying
this construction. The per-root allocation formula above is a derivation for
this estimator. [He and Owen (2014)](https://arxiv.org/abs/1411.3954) study optimal
mixture weights and sequential adaptation; this experiment does not implement
their control-variate optimization or claim its variance guarantees.

## Funding and correctness contract

All native first- and second-wave admissions, streams, results, checks and
accepted estimates remain unchanged. Passive bookkeeping is bounded and
charged separately from the native sampler's counters. Its ledger is settled
only after native second-wave work, so it cannot displace a native check.
This is retrospective accounting, not a strict prefunding guarantee. Any ledger
overdraft must be explicit and decline refinement; such a request is not
eligible for promotion evidence.

After that barrier, remaining modeled capacity and actually skipped native
second-wave checks may fund fitting and fresh independent batches. Training
observations never become a reported estimate. Existing acceptance gates remain
unchanged. Modeled costs are scheduling proxies; actual CPU and full-request
latency are measured separately, with no additional allowance.

The `native` control follows the same observation, eligibility, setup and
remaining-bank policy but uses the unchanged native proposal for its fresh
500-draw pilot and fixed main/check. It distinguishes reweighting gains from
another native attempt. All three arms use the same new pilot/main/check seed
tags, with separate independent streams for the three phases. Their pilot
observations can lead to different admitted final lengths, so final draw counts
and actual work are compared too.

Flags are opt-in: `RUST_ODDS_EXPERIMENT_ROOT_REALLOCATION=diagnostic|native|moment|cost`.
Unset or unsupported budget modes retain production behavior. Collection uses
a fixed prefix of `min(native_main_draws, floor(200000 / (8 + 2 × key_length)))`
draws. The per-draw allowance is modeled bookkeeping work, not CPU cycles.
New fitting charges `16 × feasible_roots` units. Pilot reservations use 500
draws at 1.25 times the larger of the native pilot's mean cost and the captured
maximum draw cost. Finals reserve 1.25 times the independently measured pilot
cost, then settle actual counters after the fixed batch completes. These are
forecasts rather than structural worst-case CPU guarantees. An unfunded
overrun stops further refinements and prevents publication of that new pair.

## Baseline and measurement plan

- Git baseline: `4e812fd506b39a504dc3f771c26e8d027cff2311`.
- Rust source revision: `ac92a8ab88656c2e20a24b37c1f3c3d3dcfdb4cd`.
- Frozen baseline binary SHA-256:
  `010a91e9d5f9432f4ddfdd165d13df3bfefed6de502732aa0e3f99d9cc6fbd46`.
- Isolated source: `/private/tmp/golaberto-r51/native`.
- Local ARM64 release build, four workers; production is a four-core Xeon
  E5-2697. Local paired ratios are evidence, not production timing guarantees.
- Development screen: group 16498, seeds `808,1669,1993,2281,2293`.
- Full comparisons: seven snapshots (15902,16413,16498,current/earlier16653,
  16982,16983) and the same five seeds. Screens remain separate cohorts.
- Rare cell-run: zero observations in the initial 100,000 MC draws, but a
  positive final estimate. Also count distinct identities, losses, reachable
  zeros, undecideds and impossible classifications.
- Warm timings: alternating binaries, seed 808, two warmups and eight measured
  requests per snapshot. Record process user+system CPU and stage timing.

## Paired coverage

Each arm completed 35 paired full HTTP requests: seven snapshots times five
fixed seeds, using four workers. The four five-seed group16498 development
screens are separate evidence and are not added to these totals.

| Metric | Rust baseline | Native retry | Root second moment | Root moment / cost |
|---|---:|---:|---:|---:|
| Rare positive cell-runs | 2,166 | 2,166 | 2,166 | 2,166 |
| Distinct rare identities across seeds | 503 | 503 | 503 | 503 |
| Gains / losses against paired baseline | — | 0 / 0 | 0 / 0 | 0 / 0 |
| Reachable cell-runs still at zero | 16 | 16 | 16 | 16 |
| Undecided cells | 0 | 0 | 0 | 0 |
| Impossible classifications | 540 | 540 | 540 | 540 |

A cell-run is one snapshot/team/rank/seed. Distinct identities include the
snapshot, so the earlier and current group16653 requests are separate cases.
Rare means the initial 100,000 ordinary MC seasons had zero observations while
the final estimate was positive. These counts are coverage, not an accuracy or
calibration comparison. Initial MC samples/hits and game-importance output are
identical in all paired runs.

All 105 pairs retain numerically identical native confirmation logs after
excluding only times, timestamps and request IDs. In each arm, 31/35 exports
are byte-identical. The four differing exports are group16498 seeds808,1669,
2281,2293; removing `work_spent` makes them identical as well. Probability,
precision, availability and reachability fields are unchanged. No pilot or
accounting overdraft occurs in the measured cohorts.

The 16 remaining reachable zeros span six identities:

| Snapshot | Team | Rank | Seeds still zero |
|---|---|---:|---|
| 16498 /44eabb47 | Palmeiras (16) | 13 | 1669,2293 |
| 16498 /44eabb47 | Palmeiras (16) | 14 | 808,1669,1993,2281 |
| 16498 /44eabb47 | Palmeiras (16) | 15 | All five |
| 16498 /44eabb47 | Palmeiras (16) | 16 | 1669,2281 |
| 16498 /44eabb47 | Flamengo (17) | 12 | 808,2293 |
| Current16653 /71d4fea8 | Londrina (95) | 3 | 2281 |

## Exposure and accounting

Each arm passively captures 33 native first-wave mains across 15 requests,
charging 5,600,594 modeled collector units. Each runs eight fresh 500-draw
pilots. The native control runs one fresh final (Flamengo12/seed808): 14,826
main draws, ESS4.042, no check, rejected. Moment and cost each run two finals:
Flamengo12/seed2293 has 4,157 main draws, ESS2.100, no check; seed808 runs the
main/check pair described below. None publishes an estimate.

| Decline reason | Native retry | Moment | Cost |
|---|---:|---:|---:|
| Empty/nonfinite training (including insufficient root choices) | 3 | 3 | 3 |
| Pilot not funded | 1 | 1 | 1 |
| Final pair not funded | 7 | 6 | 6 |

Fitting/setup and observation costs are charged even when no estimate follows.
Reclaimed modeled reservations permit extra physical work; they do not make
that work free. Fresh refinements run sequentially after the native parallel
barrier. This preserves native estimates but can add to request tail latency.

## What the root measurements show

Group16498, seed808, Flamengo/12th captures a fixed 7,142-draw prefix. One root
receives 4,670 of those draws and accounts for 99.9999999983% of the squared
contributions. Its complete proposal probability rises from 0.64561 to 0.77281;
the empirical training second moment predicts a 16.46% reduction. However, its
conditional ESS is only 3.299. Within-root variance accounts for 99.9755% of the
captured sample's centered variance. Increasing root exposure leaves the large
conditional weight dispersion intact.

For Palmeiras/15th in the same request, one root receives 6,070 of 7,142 draws,
with conditional ESS 5.243. Complete root probability changes 0.85023 to 0.87512,
predicting only a 2.84% training second-moment reduction. The empirical
within-root variance fraction is 99.9870%.

These are finite training-window diagnostics, not population variance estimates
or guaranteed reductions. Uncached patterns are combined into one bucket, so
the reported within/between decomposition uses coarsened root categories.

The moment arm's fresh Flamengo/12th main runs 5,856 draws, finds 288 hits and
has ESS 6.489, with estimated probability 3.8599e-26 (fraction). Its independent
5,856-draw check finds 267 hits, but ESS 2.380 rejects the pair; its estimate is
6.5784e-26. Similar order of magnitude alone does not bypass the existing
acceptance gates. Both reservations are funded; actual combined modeled work
is 97,426,369 units versus a 122,068,320-unit reserve.

Londrina/3rd in the current group16653 request has one feasible lazy root.
This arm cannot improve its target-root allocation. Residual-rival conditioning
and other production branch proposals remain separate, unchanged mechanisms.

## Validation

The isolated candidate passes all 88 Rust library tests (23.70s, test threads=1)
and a four-job release build. Added/extended tests cover passive result/RNG and
operation-counter parity, a fixed bounded prefix, zero-hit and uncached draws,
positive support and normalization, an already learned native allocation,
small enumerated points/wins cases, the native-control gate, and reservation
overflow/refusal. Existing goal-tilt tests also pass.

Four flag-off paired HTTP controls (16498,16982 × seeds808,1669) have
byte-identical exports. Each active arm also passes six group16498/seed808
invariance exports: four repeats with four workers, one run with one worker,
and one with quiet logging. Parsed exports and initial MC hashes match within
each arm. Changing worker count here is a validation control; all comparative
coverage/timing measurements use four workers.

There is no new exact score-sensitive oracle for this overlay, nor an end-to-end
forced-overdraft test. Reservation arithmetic, code review, and measured raw
accounting are the evidence for those paths. Complete outcomes still undergo
production goal sampling and sorting; no witness becomes a probability, no
score cap proves impossibility, and exhausted searches remain undecided.

## Warm full-request latency and CPU

Separate persistent HTTP cohorts alternate baseline/candidate request order,
using seed808, two warmups and eight measured requests per snapshot. Each arm
has its own paired baseline; do not compare absolute times across cohorts.
CPU is process user+system seconds over all ten requests and service lifetime,
whereas the wall column is the median of the eight measured requests.

| Arm | Snapshot | Full request ms, baseline → arm | Change | CPU seconds, baseline → arm | CPU change |
|---|---|---:|---:|---:|---:|
| Native retry | 16498 | 1,549.62 → 1,747.72 | +12.78% | 36.55 → 37.47 | +2.52% |
| Native retry | Current16653 | 1,433.38 → 1,382.59 | −3.54% | 30.11 → 29.05 | −3.52% |
| Native retry | Earlier16653 | 782.50 → 797.14 | +1.87% | 18.23 → 18.29 | +0.33% |
| Moment | 16498 | 1,436.67 → 1,520.93 | +5.86% | 36.03 → 35.67 | −1.00% |
| Moment | Current16653 | 1,400.18 → 1,414.67 | +1.03% | 30.61 → 30.07 | −1.76% |
| Moment | Earlier16653 | 822.73 → 807.22 | −1.89% | 19.58 → 19.09 | −2.50% |
| Cost | 16498 | 1,445.97 → 1,520.23 | +5.14% | 35.51 → 36.06 | +1.55% |
| Cost | Current16653 | 1,565.28 → 1,584.84 | +1.25% | 32.23 → 30.98 | −3.88% |
| Cost | Earlier16653 | 810.24 → 834.96 | +3.05% | 18.15 → 18.50 | +1.93% |

Group16498's median rare-tail stage increases 890.54→1,023.39ms for the native
control, 830.21→983.13ms for moment, and 856.85→955.71ms for cost. Thus the stage
where refinement runs grows by about133,153,99ms respectively. Other stage
variation partly offsets or amplifies that in the full request. CPU differences
are small and mixed, and baseline times vary between cohorts; these measurements
do not establish a CPU improvement or a precise production slowdown. The
measured full-request latency increases are explicit, with no added allowance.
Four workers are retained and heavy benchmark processes run sequentially.

Model fitting uses existing patterns and makes no conditional-model/goal-model
rebuild. Setup costs `16 × feasible_roots` modeled units; no separately isolated
wall-time or CPU measurement for that small fit is available. Observation and
sampling overhead are included in the complete stage/request measurements.
There are zero additional covered cells per added unit of measured work/time.

## Recommendation

Keep R51 disabled. None of its three strategies fills a zero in the measured
cohort. The useful result is the variance diagnosis: existing root selection is
already concentrated where contributions occur, while rare, large weights
remain inside those roots. The next useful proposal experiment should change
residual fixture conditioning within a productive root, with complete density
correction and independent confirmation. Repeating root allocation alone has
little measured headroom in these difficult cases.

The group16498 latency increase also fails the preservation constraint.
Remaining modeled bank capacity cannot be treated as an extra physical-time
allowance. Retain the experiment patch for diagnosis and future comparison;
do not enable or merge it.

## Retained evidence and reproduction

Artifact directory: `2026-10-02-root-reallocation/` beside this report. It
contains a self-contained `candidate.patch`, per-source SHA metadata, compact
cohort analyses/summaries, invariance reports, validation logs, a numeric audit,
and a verified raw-log/export archive with a per-member SHA-256 manifest.
All arms use the same candidate binary, SHA-256
`b4c6e576ac241713886252447c373b740e218ea80b9b8e9fd8e2610f24291b79`.
The four artifact helpers are unmodified copies of R50's helpers.

The verified archive contains 617 members, is 10,200,019 bytes, and has SHA-256
`9a2e016228c4db80f38876cb99ea0db71526d130edd8213a6f9f39d2ad3e4c60`.
`git apply --check` accepts the source patch against the baseline. Production
HEAD and binary SHA remain unchanged; pre-existing user changes are preserved.

Run from the repository root with a current Rust toolchain and a fresh scratch
directory. Serialize builds and requests. This reconstructs isolated source;
the checkout's Rust source and production binary are untouched.

```sh
artifact_dir="$PWD/experiments/rare_positions/2026-10-02-root-reallocation"
work_dir=/private/tmp/golaberto-r51-repro
mkdir -p "$work_dir/base" "$work_dir/candidate"
git archive 4e812fd506b39a504dc3f771c26e8d027cff2311 odds-rust stats/core AGENTS.md | tar -x -C "$work_dir/base"
cp -R "$work_dir/base/." "$work_dir/candidate/"
ln -s "$PWD/experiments" "$work_dir/candidate/experiments"
git -C "$work_dir/candidate" apply "$artifact_dir/candidate.patch"
cargo build --release --locked -j4 --manifest-path "$work_dir/base/odds-rust/Cargo.toml" --target-dir "$work_dir/base-target"
cargo build --release --locked -j4 --manifest-path "$work_dir/candidate/odds-rust/Cargo.toml" --target-dir "$work_dir/candidate-target"
cargo test --locked --manifest-path "$work_dir/candidate/odds-rust/Cargo.toml" --target-dir "$work_dir/candidate-target" --lib -j4 -- --test-threads=1

python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline "$work_dir/base-target/release/golaberto-odds" \
  --candidate "$work_dir/candidate-target/release/golaberto-odds" \
  --output "$work_dir/moment-pairs" --production-candidate \
  --flag RUST_ODDS_EXPERIMENT_ROOT_REALLOCATION=moment \
  --seeds 808,1669,1993,2281,2293
python3 "$artifact_dir/analyze.py" "$work_dir/moment-pairs"

python3 "$artifact_dir/invariants.py" \
  "$work_dir/candidate-target/release/golaberto-odds" \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  --output-dir "$work_dir/moment-invariants" --arm moment --seed 808 \
  --flag RUST_ODDS_EXPERIMENT_ROOT_REALLOCATION=moment
```

Repeat the paired/invariance commands with `native` and `cost`, using distinct
output directories. For the four screens, use `--cases 16498` and each mode,
including `diagnostic`. For warm timing use `--persistent --cases 16498,16653`;
that mode always uses seed808 and includes both16653 snapshots. For flag-off
controls, omit the experiment flag and use
`--cases 16498,16982 --seeds 808,1669`. No database access or mutation is needed.
