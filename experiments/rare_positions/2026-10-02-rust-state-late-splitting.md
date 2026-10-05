# R49, R48 and R5: state merging, later learned modes and fixed-level splitting

## Scope

The current Rust estimator is the baseline: ac92a8ab source,
with repository HEAD `4e812fd5` adding only AGENTS.md model roles. Frozen baseline
binary SHA256: `010a91e9d5f9432f4ddfdd165d13df3bfefed6de502732aa0e3f99d9cc6fbd46`.

Source prototypes are isolated under `/private/tmp/golaberto-r48/state-traced`,
`/private/tmp/golaberto-r48/later`, `/private/tmp/golaberto-r48/splitting` and
`/private/tmp/r48/splitting-fast`.
Luna owns all source edits; Sol owns design, coordination and review. No Pronto,
production changes, database mutations, commits or pushes.

Four estimator workers and serialized builds/tests/benchmarks. Identical requests
and fixed seeds, with actual CPU and full-request latency measured against the
current Rust baseline. Existing deterministic operation banks remain the funding
source. No additional time allowance is presumed.

## R49: merged internal joint states

The baseline `NecessaryJoint` retains each internal-fixture outcome history,
even when multiple histories have identical added packed points/applicable wins
for every selected team. The first arm merges those histories at the same
fixture step, sums their prior masses and retains weighted predecessor edges.
Backward sampling reconstructs complete fixture outcomes with their correct
conditional law. Original external TerminalTables then apply normally.

The first arm keeps the original selected-rival limits and 12,000-outcome-product
admission rule. Setup, hashing/storage and reverse reconstruction are charged.
Broader joint conditioning was deferred after this representation arm regressed.
Exhausted construction retains the existing complete fallback. The traced copy
adds constructor diagnostics only; its seed 808 export matches the original arm.

## R48: learn modes from already funded long batches

Collect bounded high-contribution seasons from ordinary LazyJoint main/check
batches. Retain at most three distinct weak packed-side masks per cached target
path, for at most eight already cached paths. Every stored contribution must
pass the production rank sorter and include the score likelihood correction.

After ordinary batches finish, only rejected cells with an existing later retry
may use this evidence. Freeze the modes, retain the native parent and evaluate
the complete overlapping proposal density. Charge collection, attempted setup,
training and replay against existing retry work. Earlier observations retain
their original weights and estimates. New estimates and checks use fresh fixed
batches; Union plans remain unchanged.

## R5: fixed-level binary branching

Sample the target's necessary point/win event with its exact normalizer `M`.
For residual fixtures, reject only outcomes that fail one-step necessary rank
counts. Ties remain viable. Sample allowed outcomes from their normalized original
probabilities and multiply the continuation weight by their allowed mass.

At distinct interior fixture indices `floor(G/4)`, `floor(G/2)` and `floor(3G/4)`,
each surviving particle splits into two independent children with half its
weight. There are at most eight terminal leaves per independent root. Complete
leaves use ordinary conditional scores and the actual phase sorter.

For root `i`, sum corrected leaf contributions as `X_i`. Report `M * mean(X_i)`.
Samples, hits, ESS, variance, maximum share and agreement diagnostics use roots,
not sibling leaves. Each root's worst-case cost is reserved before sampling;
admitted trees complete without truncation. This replaces an existing adaptive
retry slot and preserves independent fixed main/check gates.

The first version initialized the 607-word legacy RNG for every fixture
transition. That was unnecessary and incompletely represented by a single
derivation charge. A separate corrected version owns one boxed stream per
particle segment, initializes distinct streams only at branches, and continues
that stream into ordinary score sampling. It charges 1,841 seed steps per RNG
initialization. Root reservations cover all 15 possible initializations, 16
derivations and the existing worst-case transition, copy and ranking work.
Sibling RNG state is never cloned. Changed streams require fresh comparisons.

The principal modeling risk is weak checkpoint discrimination: a temporal
quartile is not necessarily a useful level of conditional rarity. Branching
cannot rescue particles that all encounter the same late constraint conflict.

## Paired coverage

Seven snapshots: groups 15902, 16413, 16498, 16653 (current and earlier), 16982 and
16983. Seeds 808, 1669, 1993, 2281, 2293 give 35 paired full HTTP requests per arm.
Runs alternate baseline/candidate order and clear unrelated estimator flags.
Each request uses four workers; heavy commands run sequentially.

A **rare cell-run** means that a team/rank had zero hits in the initial 100,000
MC seasons and a positive final estimate for that particular snapshot/seed.
The distinct-cell count is the union across the five seeds, with the two 16653
snapshots counted separately. It does not mean that a single request found all
those cells. Estimated probabilities below are fractions, not percentages.

| Arm | Baseline rare cell-runs | Candidate | Gains / losses | Distinct rare cells |
|---|---:|---:|---:|---:|
| R49 state merging |2,166|2,164|3 /5|503→502|
| R48 later modes |2,166|2,165|0 /1|503→503|
| R5 initial splitting |2,166|2,157|2 /11|503→502|
| R5 corrected splitting |2,166|2,157|2 /11|503→502|

Proof classifications, initial MC observations and game importance were
unchanged. Across the 35 requests, baseline had 16 reachable cell-runs still zero,
zero undecided cell-runs and 540 proven-impossible cell-runs. R49 increased reachable
zeros to 18; R48 to 17; both R5 versions to 25. Proof coverage therefore was not the
remaining bottleneck in this cohort.

Identical seeds do not imply identical samples after changing reconstruction or
retry proposals: RNG consumption and later work allocations change. A correct
sampling law can still lose finite-sample hits or fail a publication gate. These
regressions are why coverage and full latency are tested together; no proven
impossibility or reachability classification regressed.

### R49 findings

The three gains were Palmeiras/16th (seed 2281, 1.04e-40), Flamengo/12th
(seed 2293, 2.18e-25) in 16498, and Londrina/3rd (seed 2281, 1.70e-34) in current
16653. Five lost cell-runs included Palmeiras/16th twice, Flamengo/12th,
Palmeiras/14th and Athletico/19th. Palmeiras/14th disappeared from the union
across five seeds; no new distinct cell appeared.

Successful logged Lazy constructors contained 904 merged profiles across 154
plans. History transitions fell 92,567→89,910 (2.87%), with 4,177 merges,
61,272 unique states, 64,545 retained edges and 374,986 constructor setup units.
Those setup units incur a 16×node charge: 5,999,776 scheduling work units, before
other setup and guidance costs.
Maximum frontier width was 461. This small reduction did not pay for hashing,
state storage and reverse reconstruction. These counters describe logged
successful cached constructors, not every failed setup attempt.

Recommendation: **keep disabled**. A possible future arm is a cheap internal
fixture-cycle screen: forest components cannot coalesce distinct outcome
histories under the usual 3/1/0 points rule. Test that inference before using it
as a production shortcut. Broader certified joint selection remains untested
by this arm.

### R48 findings

Collection logged 239 positive records and 14,151,796 charged work units. Only
six failed ordinary Lazy jobs were eligible for learning; two produced piloted
modes, three modes each: old 16653 Londrina/3rd (seed 808) and current 16653
team 125/11th (seed 1993). Neither was a difficult Flamengo job. The only coverage
change was losing Flamengo/12th at seed 1993, whose baseline estimate was
1.03e-25. This loss occurred through work redistribution; there was no learned
Flamengo proposal.

Scope limits: first eight cached roots only, three weak packed-side masks per
root, no adaptation of Union plans or existing alternate/bias/case parents,
and a fresh bounded pilot funded before drawing. The native parent retains 50%
of the mixture; learned components share 50% and full overlapping densities
are evaluated. Learned modes are cleared before unrelated confirmations so
their samplers still match their pilots.

Recommendation: **keep disabled**. The useful follow-up is to gather weighted
contributions from the final native branch sampler and compose its complete
parent. The current arm's limited exposure does not establish that later
learning is ineffective in general.

### R5 initial findings

There were 77 splitting pilots, 32,483 independent roots and **zero completed
rankings or positive roots**. They charged 43,946,016 actual work units against
756,248,527 reserved worst-case units; summed pilot sampling time was 9,208ms.
None could become a funded final splitting plan. The two gains, Palmeiras/13th
at seeds 1669 and 2293 (9.96e-20 and 7.70e-20), came from redistribution of existing
native estimation work. They are not successful splitting estimates.

Losses: Flamengo/12th in three seeds, Athletico/19th in four, Londrina/3rd in
two and Londrina/4th in two. This motivated the separately measured RNG repair;
the first version's timings are not evidence for an efficiently implemented
splitting sampler.

### R5 corrected findings

The matched 35-pair correction produced the same coverage gains/losses. There
were 77 pilots and 14,908 roots, still **zero completed rankings or positive
roots**. Summed pilot sampling time fell 9,208→385ms. Fewer roots were admitted
after correcting seed initialization charges, so this is not a comparison of
equal draw counts. The corrected sampler charged 84,995,754 work units against
755,094,387 reserved units. The initial version's 43,946,016 counter omitted
seed initialization work and is not comparable as an actual CPU work measure.

An additional 42-pair audit used the driver's six default seeds
(801,804,808,817,818,911) accidentally before the explicit five-seed rerun. It is
retained separately: no gains, 11 losses, 2,586→2,575 rare cell-runs and zero
completed rankings across 95 pilots. It is not pooled into the main results.

Recommendation: **keep disabled**. This is a concentration failure of the
tested proposal, not evidence of impossibility. The next splitting arm would
need the existing native residual guidance or stronger shared-fixture viability
checks, with corrected P/Q weighting, before choosing useful fixed branching
levels. All sampled leaves died before the sorter; no death-depth histogram was
recorded. These observations do not establish that more particles would fail,
but provide no evidence to fund them under the current latency constraint.

## Warm latency and CPU

Persistent comparisons use seed 808, two warm-up requests and eight measured
requests per binary and snapshot. CPU includes all ten requests plus startup.
Full-request HTTP medians exclude warm-ups. These local ARM64 Rust 1.93.1 release
measurements compare each arm against its own paired baseline, with four
workers. They are not exact forecasts for the production Intel Xeon. Separate
cold cohorts varied in absolute latency; do not compare different cohorts as
though they had a common timing baseline.

| Arm / snapshot | HTTP baseline→candidate(ms) | HTTP change | CPU change |
|---|---:|---:|---:|
| R49 /16498 |1,377.69→1,603.54|+16.39%|+7.73%|
| R49 /16653 current |1,331.71→1,389.58|+4.35%|+2.84%|
| R49 /16653 earlier |781.58→777.90|−0.47%|−3.48%|
| R48 /16498 |1,388.44→1,355.01|−2.41%|−3.01%|
| R48 /16653 current |1,518.78→1,396.00|−8.08%|−4.96%|
| R48 /16653 earlier |843.34→853.79|+1.24%|+3.51%|
| R5 corrected /16498 |1,445.55→1,598.81|+10.60%|−1.90%|
| R5 corrected /16653 current |1,373.60→1,034.96|−24.65%|−14.61%|
| R5 corrected /16653 earlier |777.20→1,226.58|+57.82%|+20.30%|

R49's rare-tail stage medians increased in all three warm cases, even when
noise or changes in other stages masked this in full-request latency. Setup
and replay overhead are included in that stage. Deterministic operation charges
control allocation; they are not a guarantee of unchanged CPU or latency.

The corrected R5 current 16653 warm run loses Londrina/4th at seed 808. Its speed
gain therefore does not preserve coverage. The earlier 16653 case increases
latency substantially despite the faster splitting pilot: the replacement of
native alternate-mode retries also changes downstream finalists and work.
In its first warm request, finalists changed 9→8 and reported final work
4,819,870→10,868,770; the rare-tail stage took 185→619ms. Full-request costs
must be measured rather than inferred from the pilot's speed.

For the 35 cold pairs, aggregate rare coverage per process CPU second was:

| Arm | CPU seconds baseline→candidate | Rare cell-runs /CPU second |
|---|---:|---:|
| R49 |51.74→53.06|41.87→40.78|
| R48 |48.94→48.23|44.26→44.89|
| R5 corrected |50.49→50.58|42.90→42.65|

This ratio includes all retained rare cells, not just recoveries, and startup
CPU in cold runs. R48's small ratio increase comes with a coverage regression;
it earns no new-cell budget credit. Across warm cases, peak RSS ranges were
R49 baseline 237–426MiB/candidate 230–452MiB, R48 baseline 220–415MiB/candidate
235–412MiB, R5 baseline 246–416MiB/candidate 247–353MiB. Full per-stage times,
RSS and resource records are retained in the artifacts.

## Correctness gates

Exact small-fixture normalizers/outcome laws, sampler expectations and real
score-tie behavior; exhausted-budget fallback; all event paths retained;
state reconstruction; overlapping densities; frozen independent training and
confirmation; ancestry-aware uncertainty; actual operations within reservations;
flag-off identity and worker/repeat/logging invariance. A witness is not a
probability estimate or an unrestricted impossibility proof.

The experiments retain the production sorting rules. Points and applicable wins
provide necessary screens; their feasibility alone does not prove a final rank.
Every contributing complete season receives conditional scores and is sorted
normally. No arbitrary score cap proves impossibility. All existing reachability
budgets remain unchanged; these arms change probability construction or retries.

The state-merging arm passed 88 library tests and the full Rust suite: 138 passed,
four database tests ignored. The full suite used the debug library/test harness;
long endpoint tests used the equivalent optimized isolated state executable,
copied into that harness's executable path. It was not a wholly debug endpoint
run. Later modes, initial splitting and corrected splitting each passed 90
library tests. Relevant added checks cover exact joint mass/outcome laws,
goal-sensitive sorter expectations, overlapping same-root learned components,
empty supports, exhausted budgets, all-surviving branches, independent root
sum/sum-of-squares/ESS/SE, and charged work within worst-case reservations.

Separate Sol review checked weighting, full parent support, independent fixed
pilot/main/check phases and all reservation paths, including confirmations.
Compilation/test-fixture issues were corrected through Luna before measurement.
All heavy validation was serialized; no Rust or Rails production source changed.
Four flag-off paired controls per final binary reproduce the baseline export
byte for byte. All six worker/repeat/logging exports per final arm are identical
after canonical parsing. Initial MC observations are identical in those checks
and in all 35 paired requests. Focused formatting and patch-application checks
pass. No production enablement, commit or push was performed.

## Reproduction and artifacts

The self-contained patches, helper scripts, summaries and source hashes are in
[`2026-10-02-state-late-splitting/`](2026-10-02-state-late-splitting/).
`experiment.json` records the arms and flags; `source-patches.json` records every
Rust source hash. Each final patch passes `git apply --check` against the baseline.
`raw-evidence.tar.gz` preserves 876 exports, logs and resource records, including
test/build logs; every member was verified against the SHA256 manifest. Warm
summaries retain timing, CPU, RSS and stage arrays; verbose diagnostics remain in
the raw archive. Archive SHA256:
`10b66ed3c31d7e5c4aba258379a5c1d0cf92c4bc58d13d082e8281ab2f295e28`.
The initial and corrected splitting patches are separate to preserve the failed
first attempt. Production executable SHA256 still equals the frozen baseline.

Build a separate tree from repository HEAD `4e812fd506b39a504dc3f771c26e8d027cff2311`
containing `odds-rust/` and `stats/core/`. Apply one arm's patch to that tree,
link its `experiments/` directory to the repository's fixtures, and use a separate
Cargo target directory. Build/test sequentially:

```sh
cargo test --locked -j4 --lib --manifest-path /tmp/arm/odds-rust/Cargo.toml \
  --target-dir /tmp/arm-target -- --test-threads=1
cargo build --locked --release -j4 --manifest-path /tmp/arm/odds-rust/Cargo.toml \
  --target-dir /tmp/arm-target
```

After a successful build, freeze that executable before building another arm.
For example, the full 35-pair state experiment:

```sh
python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline /private/tmp/golaberto-r48/baseline/golaberto-odds \
  --candidate /private/tmp/golaberto-r48/candidate-state-traced \
  --production-candidate --flag RUST_ODDS_EXPERIMENT_JOINT_DAG=merge \
  --seeds 808,1669,1993,2281,2293 \
  --output /private/tmp/golaberto-r48/state-pairs
```

Other arms use `candidate-later` with `RUST_ODDS_EXPERIMENT_LATE_MODES=1`,
`candidate-splitting` (initial) or `candidate-splitting-fast` (corrected) with
`RUST_ODDS_EXPERIMENT_SPLITTING=1`. Pass the five seeds explicitly; the driver's
defaults differ. The warm command adds `--persistent --cases 16498,16653`; it automatically
includes the earlier 16653 snapshot. Flag-off controls omit `--flag` and use
`--cases 16498,16982 --seeds 808,1669`. Local HTTP binding may require sandbox
approval, without exposing the server publicly.

```sh
python3 experiments/rare_positions/2026-10-02-state-late-splitting/analyze.py \
  /private/tmp/golaberto-r48/state-pairs
python3 experiments/rare_positions/2026-10-02-state-late-splitting/invariants.py \
  /private/tmp/golaberto-r48/candidate-state-traced \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  --flag RUST_ODDS_EXPERIMENT_JOINT_DAG=merge --seed 808 \
  --output-dir /private/tmp/golaberto-r48/state-invariants --arm state
```

Initial MC and final probabilities are exported independently from timestamped
logs. The invariants helper runs six exports: four repetitions with four workers,
one with one worker, and one with quiet logging. It compares canonical parsed
exports and initial MC observations. Repeat per final arm with its own flag.

Coverage is a finite five-seed result, not a proof of universal dominance or
calibration at 1e-25. Exact small-fixture oracles check estimator laws; these full
requests do not provide an independent golden probability for every tiny cell.

## Recommendation

Keep the current production implementation. None of the three tested strategies
earned coverage credit; R49 and corrected R5 also violate latency preservation
on warm snapshots. The most useful next experiment is R48 evidence collection
from the native final branch sampler, including complete parent composition.
Its current two-job exposure missed the hard Flamengo jobs. That remains an
untested hypothesis, with no additional budget presumed.

## Sources

[Fichte et al. (2018)](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ESA.2018.28)
provide related dynamic-programming weighted-counting work; their GPU timings
do not establish performance for this Rust adaptation.
[Glasserman et al. (1999)](https://business.columbia.edu/sites/default/files-efs/pubfiles/4273/multilevel_splitting.pdf)
motivate fixed branching with corrected path weights and work-normalized
comparisons. [Walter (2015)](https://arxiv.org/abs/1507.00919) explains the need
to handle discrete outcomes explicitly. These are experimental adaptations,
not published football performance results.
[Owen, importance sampling](https://artowen.su.domains/mc/Ch-var-is.pdf) provides
background for likelihood corrections and complete mixture densities; the
collection and allocation policies here are our hypotheses.
