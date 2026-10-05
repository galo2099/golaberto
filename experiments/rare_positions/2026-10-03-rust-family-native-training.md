# R59: Train a family fallback without replaying native draws

Started: 2026-10-03; completed 2026-10-04. Status: isolated experiment completed.
Production Rust source is unchanged. No commit, push or database mutation.

## Result

Native training reuse works, with successful native estimates preserved across
three groups and nine request/seed pairs. The final version retains R58's four
zero-to-positive results across five group-16498 runs, involving three distinct
cells. It also fixes R58's later-native preemption in group 16653.

Do not enable it by default yet. In warm group-16498 timing, wall time averaged
**10.4% above current Rust** and CPU time **3.4% above current Rust**. It met the
preservation goal but not the latency constraint. Small accepted
Palmeiras/15th estimates also remain uncalibrated.

## Objective

Preserve native proposal selection, first- and second-wave admissions, draw
counts, random streams, successful estimates and publication gates. Learn
corrected rival families during those existing main draws. Fit and confirm a
fallback only for an originally paired Lazy proposal whose main fails the
publication gate and whose cell remains zero. A publishable main with a failed
check remains outside fallback eligibility.

R58's family fallback recovered cells but duplicated native sampling during
training. Its broad cap-3k version gained four cell-runs across five seeds while
adding 12–26% request wall time. R59 tests removal of that replay. Recording,
funding checks, fitting and confirmation remain actual work.

## Funding before outcomes are known

A skipped native check cannot retrospectively fund a recorder used during its
main. First settle the earlier ordinary rare-tail stage, including extensions,
using its existing configured logical operation allowance. Pay a bounded
admission certificate from audited spare capacity in that stage. Include actual
main/check operation counters, reservation totals, training and setup. Decline
credit when accounting is incomplete or arithmetic overflows.

Completed final reservations can be settled to actual operation counters once
all ordinary and extension jobs have finished. Retaining the larger reservation
as a diagnostic is useful, but it no longer represents future work. This
settlement changes neither native admissions nor samples.

After the original confirmation planner reserves its jobs, enumerate every
possible first-wave main publication outcome for a bounded number of jobs.
Simulate the original skipped-check refunds and stable second-wave reservation
rules. Recording may reserve at most the smaller of initial free capacity and
minimum residual capacity across all those outcomes. Thus collection cannot
change an original admission. Charge certificate work even if it declines
recording; hold collection reservations until the original waves finish.

Bound the observed draw prefix and positive observations, retain a bounded
number of keys, and charge collection on native successes and failures alike.
Native draw operations are counted once. In the certificate arm, the failed
cell's recording charge is also deducted from its original remaining grant. Paid replay remains the
fallback when no recorder can be funded.

These are bounds in the existing logical operation model, not a hard CPU or
wall-time bound. No extra production allowance is assumed.

## Statistical assumptions

The recorder observes corrected positive contributions after actual score
sampling and production sorting. It consumes no random numbers and changes no
native weights. A learned family consists of a target outcome path and each
rival's below/equal/above status under points and applicable wins. Family
constraints guide a support-preserving mixture, with the full mixture density
used in importance weights. Reachability alone is never reported as probability.

Fit only after the native attempt fails; freeze the proposal before fresh
validation and independent main/check streams. Training contributions do not
enter the final reported mean. Publication still uses the existing hit, ESS,
relative error, largest weight, batch and check agreement gates. Those gates do
not establish precise magnitude for a small accepted batch.

Mixture design follows the existing R57/R58 implementation. Background:
[He and Owen (2014), Optimal mixture weights in multiple importance sampling](https://arxiv.org/abs/1411.3954).
The paper motivates learning mixture allocation from samples; it does not
validate this endpoint's particular publication thresholds.

## Protocol

- Baseline: current Rust, production HEAD
  `4e812fd506b39a504dc3f771c26e8d027cff2311`.
- Parent experiment: frozen R58 V6 source and binary, never overwritten.
- First request: saved group 16498, SHA256
  `2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625`.
- First paired seeds: 808 and 2293; then 1669, 1993 and 2281.
- Four Rust workers. Builds, tests and estimator processes serialized.
- Compare full matrices, every baseline-positive field except request-total
  `work_spent`, exact game importance, native sampling and admission evidence,
  collection/settlement ledgers, full-request wall and child user/system CPU.
- Timings are local macOS arm64 full-request CLI measurements, including startup,
  decode and export. They are not production Xeon or direct HTTP measurements.
- Preserve default-off output parity. Separate experiment source, scripts and raw
  evidence under `2026-10-03-family-native-training/`.

## Initial strict accounting control

The first version withheld collection credit if any attempted constructor failed.
It passed 110 Rust tests and paired full-request controls for seeds 808 and 2293.
Default-off exports were byte identical to current Rust; native per-cell metrics,
wave admissions, successful estimate metadata and game importance matched.
Fresh R58 replay exports also reproduced the frozen R58 exports exactly.

Collection was declined in both requests: each had 14 failed constructors. Before
accounting for those failed attempts, the conservative ordinary-stage unused
allowance was 30,374,917 logical units for seed 808 and 6,495,374 for seed 2293.
Those figures are not available credit until the missing constructor costs have
been audited. The strict control therefore retained the R58 replay and its
existing gains; it did not yet test native-trained families.

Evidence: `2026-10-03-family-native-training/logs/v1-strict-analysis.json`.
Tested source archive: `data/r59-v1-strict-tested-source.tar.gz`, SHA256
`2002b319fd8ab1ea3d8d73db70c5c69182226e68e577081167342cbed75b4d64`.
Strict binary SHA256:
`bf905a0dba5d142f6c85cf78b245ed90076eec553444486de9ea9129720c4142`.
An earlier uncompiled archive was overwritten before any benchmarks; it is not
the source artifact for these results.

## Accounting controls

The metered v2 control passed 113 Rust tests and all five paired group-16498
seeds. Native estimates, native metrics, admissions, default-off exports and
game importance were preserved. However, collection was still declined in all
five requests. Auditing failed attempts and actual constructed Guide storage
added 54,762,240 modeled setup units in four requests and 23,544,640 in seed 2293
to previously reported setup accounting. Keeping completed final reservations
at their projected amounts then left no spare capacity. All four gained R58
cell-runs were retained, using paid replay.

Settling completed reservations to actual counters yields 19,417,076 unused
ordinary-stage units for seed 2293. The other four requests remain at or above
the ordinary-stage cap even after settlement and must retain replay. These
overages expose limits of the existing native operation accounting; recording
did not cause them. The next arm uses only that verified settled surplus.

R58's Palmeiras/15th outputs remain uncalibrated: larger controls failed ESS or
independent-check agreement. Additional nonzero outputs must not be described
as validated estimates of that cell's magnitude.

### Rejected collection allocations

V3 passed 115 Rust tests and all five fixed-seed preservation checks. Settled
credit enabled recording on seed 2293. Its protected confirmation residual was
3,450,648 units. With a 256-positive cap, that funded only the first recorder:
Flamengo/11th, whose native attempt succeeded. No failed fallback reused data.
Both a 3,000-draw window and a full-native window therefore retained four R58
gains, with no coverage improvement and an extra 771,904 collection units.

Full-native recording with a 100-positive cap funded four recorders, including
failed Flamengo/12th and Palmeiras/13th attempts. Flamengo/12th published
`5.836e-26` (raw probability), but Palmeiras/13th failed confirmation and lost
its R58 nonzero. All original native positives remained unchanged. This arm
retained only three of the four R58 gained cell-runs across the five seeds.

Six additional seed-2293 controls used caps 80, 64 and 32 with both native and
grant fallback order. They all recorded five originally paired native runs,
and all work ledgers and native preservation checks passed. None improved
coverage. Native-order caps 80 and 64 lost both R58 gains; cap 32 retained
Palmeiras/13th but lost Flamengo/12th. Grant order sometimes found
Palmeiras/13th instead of Palmeiras/15th, while losing Flamengo/12th. Those
different outputs are not simultaneous gains from one configuration.

These controls are rejected. Their final gates were unchanged; learning from
truncated or different training data changes the proposal, and fresh batches
can fail even when a previous proposal published. The remaining test funds
complete bounded records from the completed earlier-stage surplus, with an
explicit transfer and no confirmation-bank debit.

## Stage-funded collection

V4 instead pays the bounded observer from unused capacity in the completed
ordinary/extension stage. Before any native MAIN outcome is known, it charges
`64 * job_count` dispatch units and requires the **sum of all eligible observer
bounds** to fit the settled surplus. It either funds the complete bounded set
or declines. Native confirmation reservations, admissions and draw counts are
unchanged. Observer actual work is charged once to the earlier stage; unused
advances remain unspent.

Fresh fallback fitting and sampling still fit the original cell's remaining
grant, `G - A`, where `A` is native actual work. The observer advance does not
fund more fresh draws. Per-cell accounting also checks
`A + observer_actual + fresh <= G + observer_bound`. These accounting limits
are deterministic model units, not a measured latency allowance.

V4.1 corrects an unsupported-configuration edge: this funding path activates
only when native recording is requested, family mode at 1x is compatible with
operation accounting, and stage funding is selected. Disabled or incompatible
recording retains the earlier grant/settlement path and logs its decline. All
15 supported output comparisons (five seeds, stage/certificate/default-off)
were byte-identical to frozen V4 after this guard fix.

With a 3,000-draw prefix and 256-positive cap, group 16498 seed 2293 recorded
five native MAINs: two successful and three failed. Recording successes is
charged too. It retained 733 observations over 13,843 inspected prefix draws,
with 2,317,112 observer units plus 384 dispatch units, against a 10,246,960-unit
advance and 19,417,076 settled spare units. There was no confirmation-bank
observer debit. Both successful fallback fits reused their records with zero
replay sampling or duplicate collection charge:

| Cell | R58 replay training units | R59 fresh MAIN/CHECK draws each | R58 fresh draws each |
|---|---:|---:|---:|
| Flamengo/12th | 17,370,797 | 2,785 | 1,751 |
| Palmeiras/13th | 40,176,067 | 1,832 | 1,057 |

The four other group-16498 seeds had no audited surplus and kept paid replay.
Consequently this is a limited reuse opportunity, not removal of replay from
every request. A full-native prefix on seed 2293 lost Palmeiras/13th, and grant
ordering with the short prefix lost the earlier grant-order Palmeiras/15th.
Both preserved native confirmation results but reduced fallback coverage;
reject these two controls.

## Preservation across later native stages

The breadth test found an inherited R58 defect outside the five-seed group-16498
screen. On group 16653 seed 2293, the baseline's final complete-branch sampler
estimates team 95/3rd at `3.884987101554226e-34`. An immediately published
fallback makes the cell nonzero before that native sampler selects its jobs,
so it skips the native estimate. R58 instead produced `8.355e-35`; V4.1's
stage-funded result was `9.978e-35`. Native confirmation metrics alone did not
detect this later-stage preemption.

The V5 correction stages an already accepted fallback result, leaves the cell
zero during the remaining original native stages, and publishes only after
those stages finish if the cell is still zero and not proved impossible.
Publication applies to the current estimate rather than replacing its saved
metadata. All fallback work remains charged even if a later native result
supersedes it. Accepted candidates and actual publication have distinct log
events and counts. This preservation guarantee concerns the current modeled
budgets; elapsed-time budgets cannot promise unchanged admissions after extra
work.

## Final V5 paired results

The final arm uses native fallback order, a 3,000-draw observation prefix,
256-positive cap, stage funding where audited surplus exists, and paid replay
otherwise. Across **three groups and nine request/seed pairs**, every current
Rust positive estimate retains all fields except request-total `work_spent`.
Game importance is exact. Default-off exports are byte-identical to current
Rust. Native confirmation metrics and admissions match wherever that stage
actually runs; group 16982 has no eligible zero cells and correctly reports
those checks as not applicable.

| Group | Seeds | Zero→positive results vs current Rust | Positive→zero regressions | Reachable zeros remaining | Impossible zeros | Undecideds |
|---|---|---:|---:|---|---|---:|
| 16498 | 808, 1669, 1993, 2281, 2293 | 4 | 0 | 1, 4, 2, 3, 1 | 17 per run | 0 |
| 16653 | 808, 2293 | 0 | 0 | 0 in both | 48 per run | 0 |
| 16982 | 808, 2293 | 0 | 0 | 0 in both | 0 | 0 |

The four gains concern **three distinct cells**, not four simultaneous cells
in one matrix. They equal R58's native-order coverage, with these final raw
probabilities (multiply by 100 for percent):

| Seed | Cell | V5 probability | R58 probability |
|---|---|---:|---:|
| 808 | Flamengo/12th | 3.451e-26 | 3.451e-26 |
| 808 | Palmeiras/15th | 1.000e-33 | 1.000e-33 |
| 2293 | Flamengo/12th | 5.657e-26 | 2.466e-26 |
| 2293 | Palmeiras/13th | 7.594e-21 | 6.972e-21 |

Group 16498 therefore goes from 15 to 11 reachable-zero results across its five
runs, a 26.7% reduction. Positive results go from 1,900 to 1,904 out of 2,000.
There are no additional reachability or impossibility proofs: this experiment
changes probability estimation, not the proof algorithm. Palmeiras is the
only team still carrying reachable zeros: 14th on 808; 13th–16th on 1669;
14th–15th on 1993; 14th–16th on 2281; and 15th on 2293.

V5 restores group 16653 team 95/3rd to the exact original complete-branch result
`3.884987101554226e-34`. Its staged fallback is explicitly logged as
`superseded_by_later_native_stage`; all 141,440,918 fallback units remain
charged. This is a correctness improvement over R58/V4, with no extra nonzero
cell in that group.

All seven runs with native confirmation pass the observer, dispatch, fallback
grant and actual/reservation arithmetic audits. The two group-16982 runs prove
no native confirmation work from matching zero-candidate stage logs and are
not reported as having exercised that funding path. Group 16498's existing
ordinary-stage accounting exceeds its configured cap on several seeds, even
without recording; combined ordinary/confirmation deficits are 22,902,808 on
1669 and 25,376,953 on 1993. Native recording is declined there. Preserving
existing reservations does not establish a universal hard operation or CPU cap.

The historical summary `work` uses legacy sample-cost units. Actual modeled
operation totals use `actual_operation_work` plus the separately charged
setup/fallback/observer components. The report and final analyzer distinguish
these units and do not use legacy summary `work` as actual headroom.

### Probability quality

The recorded native samples train the proposal; none enter its final mean.
The proposal is frozen before fresh validation, MAIN and CHECK streams.
The existing acceptance gates were retained, including independent check
agreement. Source review and tests cover density accounting and native result
parity; passing these gates is not a convergence proof.

Flamengo/12th remains in the same rough range as the larger R57 native controls
(`1.189e-25` and `1.314e-25`). Palmeiras/13th's larger R58 controls are
`6.451e-20` and `6.655e-20`, roughly nine times above these small accepted
estimates. Palmeiras/15th remains uncalibrated: the expanded controls failed
ESS/check agreement and exposed dominant contributions absent from the small
accepted batches. The nonzero count is therefore coverage evidence, not proof
that every recovered magnitude is accurate.

## Validation and frozen artifacts

- Final V5: `cargo fmt --all -- --check`, release check/build with `-j4`,
  and full Rust tests with `--test-threads=1` passed: **170 passed, 4 ignored**.
  The four ignored tests require MySQL. The two HTTP tests passed after allowing
  their temporary loopback listeners; the first sandboxed attempt denied bind.
- Relevant tests cover full native result/RNG parity, bounded prefix/positive
  recording, overflow/declined funding, configuration activation, stage and
  confirmation-bank settlement, deferred publication, impossible-cell skip,
  later native precedence, metadata retention and work charged once.
- The Python harness passed syntax, CLI/parser, synthetic ledger and deliberate
  mismatch checks. Heavy builds, tests and estimator runs were serialized with
  at most four jobs/workers. No production source or database was changed.
- Final source archive: `data/r59-v5-deferred-source.tar.gz`, SHA256
  `09146759cd29d12eeeb7bbab3fcbd25408a98815d7d3bf44ab8d1775eb657306`.
  Includes the unchanged integration tests, required reference fixtures and
  `stats/core` dependency. Member manifest SHA256:
  `c3ceb3f63d37beb24a94362d77d323aaf20b302f7d553f4bd63272ccb4396c25`.
- Final binary: `data/golaberto-odds-r59-v5-deferred`, SHA256
  `1098fa98e657023af7a3df7d1dbb64b3b453f8c2dfdf038130e440f52d1246e0`.
- Provenance: `data/r59-v5-deferred-provenance.json`, SHA256
  `9d7948290cacd33448f8768ab52f6412cc46f6751c1d2d253e1a02cb7975c2a1`.
- Portable patch chain: `r59-v4-full-vs-production-head.patch`, then
  `r59-v4.1-activation-vs-v4.patch`, then `r59-v5-deferred-vs-v4.1.patch`.
  Dry runs passed. Prefer the complete V5 archive for reproduction.
  The earlier V2 full-production patch includes unrelated deletions from a
  partial source clone and must not be applied. Older analysis artifacts are
  historical; the authoritative final analyses begin with `v5-`.

All artifact paths above are relative to
`experiments/rare_positions/2026-10-03-family-native-training/`.

## Warm full-request timing

Each arm received a separate warmup per seed, followed by three repeats in
alternating forward/reverse arm order. All processes were serialized with four
workers. Every timed export matched its screen hash. Group 16498 has 36 timed
requests and 12 warmups; group 16653 has 24 timed requests and 8 warmups.
The fourth arm is stage funding; the third, reported in the raw logs, uses
confirmation-bank certificate funding. Warmups are excluded below.

Values are means of three full-request CLI runs, in seconds. CPU time is child
user plus system CPU time, not instantaneous CPU utilization.

| Group/seed | Current Rust wall | R58 replay wall | V5 stage wall | V5 wall vs current | Current CPU | V5 CPU | V5 CPU vs current |
|---|---:|---:|---:|---:|---:|---:|---:|
| 16498/808 | 1.498 | 1.759 | 1.715 | +14.51% | 3.535 | 3.731 | +5.53% |
| 16498/1669 | 1.342 | 1.537 | 1.424 | +6.12% | 3.599 | 3.686 | +2.41% |
| 16498/2293 | 1.613 | 1.762 | 1.777 | +10.16% | 3.836 | 3.927 | +2.39% |
| 16653/808 | 1.520 | 1.636 | 1.432 | −5.82% | 3.155 | 2.953 | −6.39% |
| 16653/2293 | 1.123 | 1.196 | 1.235 | +9.96% | 2.675 | 2.754 | +2.96% |

For group 16498, current Rust averages 1.484 s wall / 3.657 s CPU, versus
V5's 1.639 s / 3.781 s: **+154 ms / +10.4% wall and +3.4% CPU**. V5 averages
2.8% less wall time and 4.5% less CPU time than R58 replay in this sample.
However, on seed 2293—the only group-16498 request that actually reuses failed
native training—wall time is **0.83% above R58**, while CPU time is 1.54% below.
The unfunded seeds keep replay. Do not attribute their apparent timing changes
to removal of replay, or claim a general speedup from this small sample.

Group 16653 has no added coverage. Seed 2293 still spends a fallback batch later
superseded by the native complete-branch result, and costs 112 ms / 9.96% more
wall time than current Rust. Seed 808's observed decrease does not establish a
free collection path. Source layout, runtime variation and scheduling can affect
these local measurements. Production Intel Xeon and direct HTTP latency were
not measured; no CPU-specific optimization was introduced.

No one-second or older Go budget is used. Logical spare capacity is not a
permission to increase production latency. These timings fail the requested
preservation constraint despite staying within the audited observer advance.

## Recommendation and next experiment

1. Retain V5 as an isolated, reproducible experiment. Keep production unchanged.
   Native training reuse and final publication precedence are sound directions,
   with measured preservation and unchanged statistical gates.
2. Reject the small-record certificate allocations, longer prefix and grant
   order tested here: each loses an earlier fallback gain without gaining
   coverage. The short-prefix stage arm retains the best measured coverage.
3. Next, defer **fallback fitting and sampling**, as well as publication, until
   the final native stages finish. Keep native family observations and the
   original frozen grants/results, then fit only cells still at zero. This
   should avoid the 141.44M-unit superseded group-16653 batch. It is a generic
   eligibility change, not a team/rank exception; it has **not** been implemented
   or measured here. Charge storage/dispatch and preserve all original native
   work. Compare full requests before treating it as a performance gain.
4. Continue the separate tail audit for Palmeiras/15th before trusting its
   recovered magnitude. More nonzero outputs alone cannot validate it.

## Reproduction

From the repository root, rebuild the frozen source in a separate directory:

```bash
r59_repro_dir="$(mktemp -d /private/tmp/golaberto-r59-repro.XXXXXX)"
tar -xzf experiments/rare_positions/2026-10-03-family-native-training/data/r59-v5-deferred-source.tar.gz -C "$r59_repro_dir"
cargo build --release -j4 --manifest-path "$r59_repro_dir/odds-rust/Cargo.toml"
cargo test --release -j4 --manifest-path "$r59_repro_dir/odds-rust/Cargo.toml" -- --test-threads=1
```

The source contains its required test inputs; database tests remain gated.
The HTTP tests require permission to bind temporary loopback listeners.
Use Rust/Cargo 1.93.1 for the recorded build; other toolchains may produce a
different binary hash. Set `r59_repro_bin` to that rebuilt release binary.
The saved macOS binary can instead be used on a compatible host:

```bash
r59_repro_bin="$PWD/experiments/rare_positions/2026-10-03-family-native-training/data/golaberto-odds-r59-v5-deferred"
env RUST_ODDS_LOG=1 \
  RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK=1 \
  RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE=all \
  RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE=family \
  RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP=3000 \
  RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING=1 \
  RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING_FUNDING=stage \
  "$r59_repro_bin" estimate \
  experiments/rare_positions/2026-10-03-family-conditioning/data/group-16498-44eabb47.json \
  /private/tmp/r59-group16498-seed2293.json 2293 4
```

Clear other inherited `RUST_ODDS_*` and `RARE_POSITION_*` flags for a standalone
invocation. The provided harness does this automatically. Repeat the paired
screen with a **new** output directory; it refuses existing output directories:

```bash
python3 experiments/rare_positions/2026-10-03-family-native-training/scripts/run_screen.py \
  --baseline /private/tmp/golaberto-r57/baseline/odds-rust/target/release/golaberto-odds \
  --baseline-reference experiments/rare_positions/2026-10-03-family-fallback/logs/v3-cohort \
  --replay /private/tmp/golaberto-r58-v6/odds-rust/target/release/golaberto-odds \
  --capture "$r59_repro_bin" \
  --request experiments/rare_positions/2026-10-03-family-conditioning/data/group-16498-44eabb47.json \
  --out /private/tmp/r59-reproduced-screen --seeds all --workers 4 --order native \
  --capture-only-flags RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK=1,RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE=all,RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE=family,RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP=3000,RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING=1,RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING_FUNDING=stage
python3 experiments/rare_positions/2026-10-03-family-native-training/scripts/analyze_screen.py \
  --screen /private/tmp/r59-reproduced-screen \
  --r58 experiments/rare_positions/2026-10-03-family-fallback/logs/v6-screen \
  --out /private/tmp/r59-reproduced-analysis.json --capture-arm capture-only-control
python3 experiments/rare_positions/2026-10-03-family-native-training/scripts/run_timing.py \
  --baseline /private/tmp/golaberto-r57/baseline/odds-rust/target/release/golaberto-odds \
  --replay /private/tmp/golaberto-r58-v6/odds-rust/target/release/golaberto-odds \
  --capture "$r59_repro_bin" \
  --request experiments/rare_positions/2026-10-03-family-conditioning/data/group-16498-44eabb47.json \
  --screen /private/tmp/r59-reproduced-screen --out /private/tmp/r59-reproduced-timing \
  --workers 4 --order native \
  --capture-only-flags RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING_FUNDING=stage
```

Frozen baseline/R58 binaries, source and their hashes are documented in
`2026-10-03-rust-family-fallback.md`; rebuild or use those artifacts if the
temporary paths no longer exist. The recorded baseline-reference screen requires
the exact recorded baseline binary hash. For other builds use `--fresh-baseline`
and `--generic-r58`. Breadth runs use the reference `inputs/group-16653-71d4fea8.json`
and `inputs/group-16982-9327edcd.json`, `--seeds first` (808/2293), and fresh
baseline; breadth timing uses `--seed-list 808,2293`.

Authoritative raw evidence: `logs/v5-stage-native`,
`logs/v5-stage-breadth-{16653,16982}` and their analyses, plus
`logs/v5-stage-timing-{16498,16653}/summary.json`. Every run retains request,
binary/export hashes, exact arguments/flags, stage logs, wall time and child
user/system CPU. Frozen V4/V4.1 and rejected control logs are retained separately.
