# R50: learn fixture modes from complete native proposals

Status: implemented and measured; both variants rejected for production promotion.
Production is unchanged. No commit or push.

## Question

Can the long native confirmation batches teach a better proposal for cells that
already have many hits but too much weight concentration? R48 did not train the
alternate-root parents used by Flamengo/12th. This experiment supports that
complete parent and reallocates its existing confirmation work.

The saved group 16498 / seed 808 baseline has 95 ordinary Flamengo/12th main hits
(ESS 2.884, largest contribution 56.55%). Its longer confirmation has 366 hits
(ESS 3.984, largest contribution 41.79%) and no accepted independent pair.
Learning more winning patterns is useful only if fresh batches reduce that
concentration; additional raw hits alone are insufficient.

## Baseline and experiment

- Baseline Git HEAD: `4e812fd506b39a504dc3f771c26e8d027cff2311`.
- Rust source baseline: `ac92a8ab88656c2e20a24b37c1f3c3d3dcfdb4cd`.
- Frozen baseline executable SHA-256:
  `010a91e9d5f9432f4ddfdd165d13df3bfefed6de502732aa0e3f99d9cc6fbd46`.
- Baseline and isolated source: `/private/tmp/golaberto-r50/base` and
  `/private/tmp/golaberto-r50/native`; production source remains untouched.
- Flags: `RUST_ODDS_EXPERIMENT_PARENT_MODES=diagnostic` observes native batches;
  `patterns` additionally fits learned proposals; unset retains baseline.
- Rust 1.93.1, local ARM64, four estimator workers, serialized heavy commands.
  Production uses four Xeon E5-2697 cores; local ratios are the available evidence.

## Allocation and correctness

Passively record a bounded number of winning outcome patterns from native
first-wave confirmation mains. Their sample counts, seed streams, results and
existing checks retain the native law. Observation work is reserved only after
first-wave admissions, from the same confirmation bank. Contribution diagnostics
separate outcome weights and goal weights.

Fit a few contribution-weighted modes across all supported cached roots, freeze
them, and keep the complete native parent as half of the proposal. Evaluate the
full overlapping density, including the native alternate component. Root
selection remains native. Apply the original score likelihood correction once
and verify the actual rank with the production sorter. Unsupported case/dynamic
parents remain sampled with the native proposal; declined fitting and capture
costs still enter the shared bank.

Fresh independent pilot/main/check batches spend reclaimed skipped-check work
before native second-wave admissions. Already accepted first-wave estimates are
preserved. This can displace counterfactual second-wave work, so paired coverage
losses must be counted. No publication gate, reachability rule or probability
floor changes. Learned witnesses and training observations are not estimates.

Capture bookkeeping has a prefunded ceiling. Setup admission combines charged
selection/copy work, an estimated preparation allowance, and the inherited
constructor node/guide counters, capped at 200,000 scheduling units. The
preparation allowance is not a worst-case bound on domain propagation or DP
execution. Pilot funding uses a structural bound on instrumented operation
counters. Final fixed draw counts use pilot operation costs with a conservative
margin. None of these costs guarantees a bound on CPU cycles or latency. Actual
CPU and full-request latency are measured separately; no extra allowance is
assumed.

## Measurement plan

First screen group 16498 using seeds `808,1669,1993,2281,2293`. Then compare all
seven reference snapshots: 15902, 16413, 16498, current and earlier 16653, 16982,
and 16983. Count rare cell-runs (initial 100,000 MC draws have zero hits, final
probability is positive) and distinct rare identities separately. Report gains,
losses, reachable zeros, undecided cells, exposure, outcome/goal weight
concentration, construction cost, full-request latency and CPU.

Small-fixture tests cover native alternate overlap, support, likelihood weighting,
passive capture equivalence and modeled work limits. The points/wins probability
oracle is exact enumeration. Goal-sensitive validation compares independent
native and learned batches statistically, together with the existing GoalTilt
unit tests; it is not a new exact score-sum oracle.

## Related research

[Hesterberg (1995), defensive mixtures](https://www.tandfonline.com/doi/abs/10.1080/00401706.1995.10484303)
provides the motivation for retaining support when adapting an importance
proposal. Here the defensive component is the complete native necessary-event
proposal rather than the unconditional original season law.
[Owen and Zhou (2000)](https://www.tandfonline.com/doi/abs/10.1080/01621459.2000.10473909)
study mixture sampling with control variates. This experiment does not implement
their control-variate estimator, so its variance guarantees are not claimed.

### Review corrections before measurement

Review required the learned law to retain the root-selection denominator, reuse
the already drawn component's density, reconstruct complete packed point/win
states for records, and account for fallback/cardinality pilot work. Fitting uses
local caches and immutable guide references. Selection and build preparation are
charged as modeled allowances before attempting construction. Diagnostic root
order is deterministic.

The first prototype additionally requires pilot ESS at least 12 before funding
fresh confirmations. An ablation sets
`RUST_ODDS_EXPERIMENT_PARENT_MODES_PILOT_ESS=0`, removing that extra threshold and
using existing confirmation sizing for positive finite ESS. Existing estimate
publication gates remain unchanged in both variants.

## Paired coverage results

Each arm completed 35 full-request pairs: seven snapshots × five fixed seeds.
Separate five-pair development screens and a diagnostic-only screen are retained
as separate cohorts; they are not added to the 35-pair coverage totals.

| Arm | Rare cell-runs | Gains / losses | Distinct rare identities | Reachable zero cell-runs |
|---|---:|---:|---:|---:|
| Native Rust baseline | 2,166 | — | 503 | 16 |
| Learned modes, pilot ESS ≥12 | 2,166 | 0 / 0 | 503 | 16 |
| Learned modes, extra ESS floor removed | 2,167 | 2 / 1 | 503 | 15 |

Both variants retain zero undecided cell-runs and the same 540 impossible
classifications. Initial MC observations and game importance match baseline.
These are cell-run counts, not additional unique positions. The net gain of one
rare cell-run is only 0.046% of baseline rare coverage.

| Snapshot | Baseline | ESS ≥12 | Extra floor removed |
|---|---:|---:|---:|
| 15902 | 274 | 274 | 274 |
| 16413 | 236 | 236 | 236 |
| 16498 | 432 | 432 | 433 |
| 16653 current | 322 | 322 | 322 |
| 16982 | 158 | 158 | 158 |
| 16983 | 428 | 428 | 428 |
| 16653 earlier | 316 | 316 | 316 |

The only coverage changes are in group 16498:

| Seed | Cell | Change | Final probability fraction |
|---:|---|---|---:|
| 808 | Palmeiras /15th | zero → estimate | 4.19e-33 |
| 2293 | Flamengo /12th | zero → estimate | 9.68e-26 |
| 2293 | Palmeiras /14th | estimate → zero | baseline 8.88e-25 |

These are probability fractions; percentage values would be 100 times larger.
Across the five seeds, Palmeiras/15th becomes a newly covered identity, while
Palmeiras/14th disappears from the candidate's covered union. Thus distinct
coverage stays 503 even though two individual cell-runs improve.

This is coverage over five fixed seeds per snapshot. It does not establish
calibration of the tiny estimates, including the new 4.19e-33 value; there is no
independent reference probability for that cell in this experiment.

### Regression mechanism

All accepted native first-wave estimates are preserved. The Palmeiras/14th loss
comes from second-wave allocation: its native main remains exactly the same
20,000 draws, 88 hits, ESS 7.138 and probability 8.88e-25. Its 20,000-draw
independent check costs 236,400,000 modeled units. After the learned Flamengo
pair, only 196,901,182 units remain in the 1,015,052,750-unit bank. The check is
not admitted, so the main alone cannot publish. This is a concrete work tradeoff,
not a failed reachability proof or a likelihood regression.

## Exposure, weights and stage work

In the 35-pair cohorts, capture records 33 native confirmation jobs across 15
requests. There are 12 fit attempts, 10 fitted plans and 10 fresh pilots. Capture
charges 4,994,104 units; modeled setup charges 1,728,031 units; pilots record
47,773,111 instrumented units. The 12-ESS variant admits no learned finals:
pilot ESS ranges from zero to 2.696, despite up to 54 hits.

Removing the extra floor admits five fixed fresh main jobs and publishes two
independently checked estimates. Initial final-job reservations total
1,284,090,086 estimated units. Skipped-check refunds return 483,682,418, leaving
800,407,668 net reserved units. Finals record 629,188,239 instrumented units.
All bank reservations remain within
their original limits. That does not imply unchanged actual latency.

| Group/seed/cell | Fresh main draws | Hits | ESS | Check draws | Accepted |
|---|---:|---:|---:|---:|---|
| 16498 /808 /Flamengo12 | 12,006 | 675 | 3.898 | 0 | no |
| 16498 /808 /Palmeiras15 | 7,500 | 88 | 6.948 | 7,500 | yes |
| 16498 /2293 /Flamengo12 | 8,165 | 433 | 6.557 | 8,165 | yes |
| 16653 current /2281 /Londrina3 | 30,000 | 24 | 14.014 | 0 | no |
| 16653 current /2293 /Londrina3 | 30,000 | 32 | 3.414 | 0 | no |

For Flamengo/12th at seed 808, the 200,000-unit capture ceiling records the first
166 positive observations out of 366 native hits, covering four supported cached
roots. In that captured window:

- Outcome-weight ESS: 1.829; goal-ratio ESS: 101.810; corrected-weight ESS: 4.636.
- One root contributes approximately 99.998% of the captured corrected sum.
- Three learned modes occupy three different roots, one mode per root.
- The fresh pilot still has ESS 1.333 and maximum contribution share 85.94%.
- The larger fresh main has 675 hits but ESS 3.898, maximum share 42.36%, and
  fails publication before running a check.

The weights identify fixture-outcome concentration as the main issue for this
captured window. Root contribution alone does not distinguish poor marginal root
allocation from variance within the dominant root. Native root frequencies and
second moments should be measured before choosing between those explanations.
Palmeiras/15th differs: its captured goal-ratio ESS is only 4.153, so score
weighting also matters there.

Gated logs contain placeholder zero maximum shares for separate outcome/goal
summaries; only their ESS fields and corrected maximum share are used here. The
ablation source fixes those diagnostics and tests their maxima. Capture timing
includes the existing native main/check batches; it is not isolated collection
overhead. Operation counters/model allowances are scheduling evidence, not CPU
cycle measurements.

## Warm full-request latency and CPU

Each snapshot uses two warmups and eight measured repetitions per binary,
alternating order, fixed seed 808 and four workers. CPU is process user+system
time over all ten requests and service lifecycle; wall values are medians of the
eight measured requests. Compare each arm with its own paired baseline.

| Arm /snapshot | Baseline→candidate wall ms | Wall change | CPU change |
|---|---:|---:|---:|
| ESS ≥12 /16498 | 1,302.91→1,287.68 | −1.17% | −3.70% |
| ESS ≥12 /16653 current | 1,321.15→1,264.13 | −4.32% | −7.71% |
| ESS ≥12 /16653 earlier | 744.98→727.32 | −2.37% | −4.83% |
| Extra floor removed /16498 | 1,258.95→1,515.82 | **+20.40%** | **+8.80%** |
| Extra floor removed /16653 current | 1,247.09→1,252.75 | +0.45% | −0.82% |
| Extra floor removed /16653 earlier | 741.79→721.62 | −2.72% | −6.35% |

In the useful seed-808 ablation, median `search.rare_tail` time rises
756.44→1,003.92ms. Learned fits/pilots/finals are sequential after the native
first-wave barrier. Their 247.48ms stage increase accounts for most of the
256.87ms full-request increase. Four cores are the maximum throughout; the
learner's one-worker tail is a latency cost even when the modeled bank fits.
Gated full-request reductions do not establish an optimization: its rare-tail
time on 16498 grows 761.57→782.77ms, and other stage/timing variability offsets
that cost. Cold cohort ratios are retained in the analyses but are noisier.

## Recommendation

**Keep both variants disabled.** The gated arm publishes no learned estimates. Removing the
extra floor demonstrates useful learning, but trades Palmeiras/14th for another
cell, does not increase distinct coverage, and violates the current full-request
latency constraint on group 16498. Existing unused modeled capacity is not free
CPU time.

The most useful follow-up is an allocation control using the long native batch's
per-root draw counts and corrected second moments. Test native-pattern-only
root reweighting before constructing more patterns; the existing initial-pilot
root-allocation code supplies a starting point. Preserve complete support, freeze
the learned allocation before fresh independent batches, use the new root
selection probability in P/Q, and reserve admitted native second-wave checks
before spending on refinements. Fund it by replacing failed confirmation work,
and compare its actual four-core latency. This is a proposed experiment, not a
conclusion that root reweighting will solve within-root or goal variance.

## Validation and retained evidence

- Final isolated Rust library tests: **90 passed**, zero failures, 22.65s,
  `-j4 -- --test-threads=1`.
- Each arm: six parsed exports identical across four repeated four-worker runs,
  one-worker execution and quiet logging. Initial MC observations also match.
- Final binary with flags unset: four paired exports match the production
  baseline exactly (groups 16498 and 16982, seeds 808 and 1669).
- Production Rust source, executable and Git HEAD remain unchanged. No DB
  mutations, commit, push or production enablement.

Artifacts are in [2026-10-02-parent-modes](2026-10-02-parent-modes/):

- `gated.patch` and `open.patch`, with complete source hashes in
  `source-patches.json`.
- Cohort `*.summary.json` and compact `*.analysis.json`; the latter omit
  duplicated log arrays, which remain in the raw archive.
- `gated.invariants.json`, `open.invariants.json` and `library-tests.log`.
- `raw-evidence.tar.gz`: **424 verified members**, 6,863,354 compressed bytes,
  SHA-256 `99f39e3b9d65d9788b47e07fc9fa5a56f22781d063f0ffd65a8598e9513c557c`.
  `raw-evidence.manifest.json` records each member's size and SHA-256; packaging
  verified every archived member against its source.
- Analysis, invariance, patch-generation and evidence-packaging helpers copied
  unchanged from the preceding experiment.

Frozen measured candidate executable hashes:

| Arm | SHA-256 |
|---|---|
| Gated | `05abade85dbbd51426846193f5cdfe91800f49c840346d3737737d50715291c1` |
| Extra floor removed | `4cb8881a56ca7e40e4aed01583e9e472032a9007dadd263dee29284dffe04564` |

## Reproduction

Run from the repository root. Use a fresh scratch directory and serialize all
builds and benchmarks. The benchmark runner always uses four workers and clears
unrelated odds flags. The patches restore isolated source without modifying the
working checkout:

```sh
artifact_dir="$PWD/experiments/rare_positions/2026-10-02-parent-modes"
work_dir=/private/tmp/golaberto-r50-repro
mkdir -p "$work_dir/base" "$work_dir/gated" "$work_dir/open"
git archive 4e812fd506b39a504dc3f771c26e8d027cff2311 odds-rust stats/core AGENTS.md | tar -x -C "$work_dir/base"
cp -R "$work_dir/base/." "$work_dir/gated/"
cp -R "$work_dir/base/." "$work_dir/open/"
git -C "$work_dir/gated" apply "$artifact_dir/gated.patch"
git -C "$work_dir/open" apply "$artifact_dir/open.patch"
cargo build --release -j4 --manifest-path "$work_dir/base/odds-rust/Cargo.toml" --target-dir "$work_dir/base-target"
cargo build --release -j4 --manifest-path "$work_dir/gated/odds-rust/Cargo.toml" --target-dir "$work_dir/gated-target"
cargo build --release -j4 --manifest-path "$work_dir/open/odds-rust/Cargo.toml" --target-dir "$work_dir/open-target"
cargo test --manifest-path "$work_dir/open/odds-rust/Cargo.toml" --target-dir "$work_dir/open-target" --lib -j4 -- --test-threads=1

python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline "$work_dir/base-target/release/golaberto-odds" \
  --candidate "$work_dir/gated-target/release/golaberto-odds" \
  --output "$work_dir/gated-pairs" --production-candidate \
  --flag RUST_ODDS_EXPERIMENT_PARENT_MODES=patterns \
  --seeds 808,1669,1993,2281,2293
python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline "$work_dir/base-target/release/golaberto-odds" \
  --candidate "$work_dir/open-target/release/golaberto-odds" \
  --output "$work_dir/open-pairs" --production-candidate \
  --flag RUST_ODDS_EXPERIMENT_PARENT_MODES=patterns \
  --flag RUST_ODDS_EXPERIMENT_PARENT_MODES_PILOT_ESS=0 \
  --seeds 808,1669,1993,2281,2293
python3 "$artifact_dir/analyze.py" "$work_dir/gated-pairs"
python3 "$artifact_dir/analyze.py" "$work_dir/open-pairs"
```

For warm timings, rerun each benchmark with a new output directory and
`--persistent --cases 16498,16653`. This mode uses seed **808**, two warmups and
eight measured requests, regardless of `--seeds`. For the development screen,
use `--cases 16498`; diagnostic capture uses `diagnostic` instead of `patterns`.
For flag-off controls, omit both experiment flags and use
`--cases 16498,16982 --seeds 808,1669`.

```sh
python3 "$artifact_dir/invariants.py" \
  "$work_dir/open-target/release/golaberto-odds" \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  --output-dir "$work_dir/open-invariants" --arm open --seed 808 \
  --flag RUST_ODDS_EXPERIMENT_PARENT_MODES=patterns \
  --flag RUST_ODDS_EXPERIMENT_PARENT_MODES_PILOT_ESS=0
```

Repeat the invariance command with the gated binary and output directory,
omitting the extra ESS override. Input hashes, raw result exports,
stage logs and process CPU measurements are retained with each cohort.
