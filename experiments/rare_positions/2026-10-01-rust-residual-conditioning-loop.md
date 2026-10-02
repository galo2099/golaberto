# R33: residual-difficulty conditioning loop

## Final disposition — 2026-10-02

Discarded the active residual-loop implementation and runtime flag. The five
variants did not justify their cost. This historical report and its source patch
are retained as negative evidence; reproduction uses the isolated historical
candidate, not the current production server.

Raw files are stored in `2026-10-01-residual-loop/evidence.tar.gz`, with per-file
hashes in `manifest.json`. Extract that archive in its directory before using
the historical commands. Statements about flags, defaults and commit status
below refer to the experiment date. Current disposition is recorded in
[the decision report](2026-10-02-rust-experiment-decisions.md).

## Result and recommendation

**Keep this experimental and disabled.** The loop detects why target-only
conditioning is inadequate, and can rescue estimates that fluctuate between
seeds. It does not yet provide a reliable Londrina/3rd estimate.

Five variants were compared against the shipped Rust baseline. The final
adaptive variant was also tested on two fresh seeds across all seven reference
snapshots. In its combined 23 paired requests:

- Four gained cell-runs, three distinct cells, no lost nonzero cells or proof
  regressions. Reachable-zero cell-runs fell from 30 to 26; undecideds remained
  zero. All three rescued cells had baseline estimates in other seeds: no new
  cell was discovered across the combined baseline coverage.
- Aggregate CPU increased **18.42%** and full-request wall time **34.81%**.
- Londrina/3rd remained zero in all five seeds. The proposal finds real matching
  seasons, but independent batches have concentrated, unstable weights.
- Default production flags, four workers and acceptance gates are unchanged.
  No commit, push, database writes or default enablement were performed.

The useful next experiment is to improve proposal allocation and the joint
sampler's handling of points awarded to *opponents* of conditioned teams.
Increasing the number of conditioned rivals alone made performance worse.
Do not ship this loop as an additional production stage in its current form.

## Baseline and experimental design

Baseline: current Rust `2290fe044a86fed57c20a5c586ca1ffe87e74e23`, including
coverage and confirmation1.5 defaults. Baseline executable SHA256:
`01a2737525c254896ff78c0f3836ca59ed5382f928bbec1199745a57af04b5ce`.
An isolated copy of that source plus the focused experiment patch was used;
other workspace experiments were excluded from the tested build.

All requests used four workers and identical saved JSON, probabilities, seeds
and model means. Requests ran serially, alternating candidate/baseline order
within each snapshot. Builds, tests and benchmarks did not run concurrently.
Measurements were on the local ARM Mac, Rust1.93.1. There is no production Xeon
measurement. CPU is child-process user+system time, including server startup;
wall is the complete HTTP request. Stage logs measure wall time, not stage CPU.

Development cohort: seeds808,1669,1993 on current16498, current16653 and the
historical16653 fixture: nine pairs per variant,45 pairs total. Final validation:
seeds1847,1861 on all seven snapshots:14 further pairs. **59 total paired
requests**, but final recommendation statistics use only the final variant's
23 pairs, not a sum of gains from repeated variants.

The seven snapshots are15902,16413,16498,16982,16983 and two16653 snapshots.
Request hashes, flags, executable hashes and individual results are archived.

## What the loop does

1. Complete the current estimator and freeze its positive estimates.
2. For up to eight remaining non-impossible zero cells, construct a generic
   target-path proposal from available verified seasons and independently
   generated attainable target paths. No team or position is hardcoded.
3. For up to two cached target paths, choose the direction with fewer remaining
   strict rank exceptions: teams above, or teams below. Applicable wins are
   packed with points as in the existing sampler.
4. Order ambiguous rivals by the one-team probability of satisfying the required
   side of the target total. This order is guidance, not a reachability proof.
5. Compute an exact necessary rank-cardinality probability on a small subset.
   Enumerate shared internal fixtures; convolve each team's external fixtures;
   sum the probability of at most the allowed number of strict exceptions.
   This avoids treating shared-game rivals as independent.
6. If that necessary event would yield fewer than one hit in30,000 ordinary
   target-conditioned draws, build partial, disjoint exception cases. Run the
   existing all-team propagation, compact forced fixtures and construct the
   existing weighted joint proposal. Expand the constrained rival set after
   weak independent pilots: widths3,4,5,6,8,12,20 in the expanded variants.
7. Final adaptive mode compares the existing rank guidance with guidance toward
   each rival's propagated point/win interval. Each uses fresh2,000-draw pilots.
   Keep the best measured pilot ESS; stop expansion at ESS>=12 and hits>=30.
8. Freeze the selected proposal. Size a new main batch from training only:
   `ceil(max(120*N/hits, 32*N/ESS))`, clamped to2,000–100,000 in the larger variant.
   Run an independent same-size check only if the main passes the existing gate.
   Neither pilot values nor failed batches are pooled into a reported estimate.

Important limit: the screen is under the original model P after the fixed target
path. It does **not** predict the hit frequency under the existing guided
proposal Q. It motivates escalation together with weak pilot evidence.

Native caps remain:32 target roots, two roots for case expansion,32 case masks
per root,20,000 additional joint setup nodes per depth, existing4-million guide
value cap, and12,000 internal screen states. Screening uses at most six rivals,
while the partial case constraint can include more rivals. Incomplete cases
retain the defensive proposal and are logged as incomplete.

## Development results

Each row uses its own nine paired baseline requests. Percentages are ratios of
summed CPU/wall across that row, not averages of per-request percentages.

| Variant | Gained cell-runs | Lost | CPU change | Full wall change |
|---|---:|---:|---:|---:|
| V1: widths3–6, four-rival joint block,30k cap | 1 | 0 | +3.73% | +10.74% |
| V2: widths3–6, six-rival joint block,30k cap | 0 | 0 | +5.23% | +9.83% |
| V3: expanded cases, four-rival block,30k cap | 1 | 0 | +14.07% | +18.95% |
| V4: propagated interval guidance,30k cap | 0 | 0 | +7.57% | +10.50% |
| V5: adaptive guidance, training-sized batches up to100k | 1 | 0 | +17.64% | +40.83% |

The development gain in V1,V3,V5 is the **same**16498 team17/12th cell at
seed808. These are three observations of one rescue, not three distinct cells.
V1 estimate7.30e-26; V3 estimate1.81e-25; V5 estimate1.68e-25 (fractions).

Six-rival conditioning removed more choices from the remaining guided portion,
including games whose other endpoint receives points. The exact joint block
conditions selected-team intervals, but does not jointly condition all those
opponents' intervals. Londrina main hits fell from32–43 in V1 to0–3 in V2;
two V2 pilots had no hits, so no final batch was funded. More conditioning is
not automatically a better importance proposal.

## Final adaptive validation

Fresh14-pair validation gained three cell-runs, no losses, with+19.34% CPU and
+27.69% wall. Combined with its development cohort:

| Snapshot | Pairs | Baseline mean full ms | Loop mean full ms | Gains |
|---|---:|---:|---:|---:|
|15902|2|154.92|151.52|0|
|16413|2|149.31|144.12|0|
|16498|5|1031.41|1919.25|4|
|16653 current|5|993.36|1141.54|0|
|16653 historical|5|628.91|634.82|0|
|16982|2|250.84|251.13|0|
|16983|2|249.00|241.59|0|

Small changes in cohorts where there are no unresolved cells are timing noise;
the loop adds no sampling work there.16498 mean CPU2943.69→4137.37ms;
current16653 mean CPU2080.75→2213.14ms. Averages dilute the affected-request
cost:16498 wall increased86%, including one full request2.568seconds.

Accepted rescues, probabilities as fractions:

| Group | Seed | Team/rank | Main estimate | Independent check | Main/check ESS |
|---|---:|---|---:|---:|---:|
|16498|808|17/12|1.68e-25|2.29e-25|13.83/6.34|
|16498|1847|17/12|1.60e-25|1.07e-25|18.66/21.50|
|16498|1861|318/4|3.61e-19|4.22e-19|16.16/15.62|
|16498|1861|5/19|7.00e-22|7.39e-22|12.34/37.23|

All are consistent in order of magnitude with baseline estimates in other seeds.
The loop's preservation log is true on every final-variant request. The final
matrix reconciliation still runs after this stage; pre-loop clock-dependent
funding can differ between separate requests. Preservation describes estimates
inside the added stage, not bit identity of separately timed entire requests.

### Work and setup

Current16653 proposal construction was0.46–0.47ms; loop training35.92–37.36ms.
Its complete added stage took86.83–181.25ms across the five seeds.16498 sums of
per-cell construction were6.02–18.51ms; summed cell training163.08–433.46ms.
Cells train concurrently, so those sums are not request wall time. Its complete
added stage took171.80–1457.27ms, dominated by unsuccessful final batches.

The experimental admission deadline is1.5 times measured pre-tail time. It is a
**soft** between-task deadline: admitted fixed main/check batches can overrun it.
The1457ms stage demonstrates that it is not a production latency guarantee.
No additional latency allowance was assumed in interpreting these measurements.
Replacing current successful confirmation work with this loop is not supported
by the observed coverage per time. No reallocation variant was tested in R33;
all measured gains here come from additional work.

## Londrina/3rd: what the screen learns and what remains

Group16653 team95 must win all eight remaining games to reach52points/15wins.
The probability of that target outcome path is1.416045169745338e-4. Vila Nova is
already guaranteed ahead, leaving at most one additional team strictly ahead.

| Screened rivals | Conditional necessary-event probability | After target path |
|---|---:|---:|
|Fortaleza, Novorizontino, Juventude|4.3923e-8|6.2197e-12|
|Those three + Atlético-GO|1.9793e-11|2.8027e-15|
|Those four + Operário|1.9159e-14|2.7130e-18|

These are model-based upper bounds on the full requested event through that
necessary subset condition, not position estimates or impossibility proofs.
Ties on packed points/wins remain eligible; the production sorter decides actual
ranks. The loop constructs three surviving exception cases and finds matching
seasons. It still fails to concentrate enough independent weighted evidence.

Final adaptive batches:

| Seed | Main draws/hits | Main ESS | Check draws/hits | Check ESS | Outcome |
|---:|---:|---:|---:|---:|---|
|808|48000/57|3.90|0/0|0|Main rejected|
|1669|60000/68|10.53|60000/68|1.67|Check rejected|
|1993|40000/47|3.82|0/0|0|Main rejected|
|1847|48000/53|7.49|48000/54|2.62|Check rejected|
|1861|80000/109|3.45|0/0|0|Main rejected|

For1669, main1.306e-34 versus check7.742e-34: ratio5.93, and check ESS1.67.
For1847, main3.707e-34 versus check1.173e-33: ratio3.16, but check ESS2.62.
The main/check gates were not relaxed. These rejected numbers are diagnostic
only; the API cell stays zero. No accurate golden probability at this scale was
established by this experiment.

## Correctness and tests

- Full fixtures share one outcome per game; applicable wins and points use the
  existing packed totals. Shared internal games are explicitly enumerated.
- Partial exception cases are disjoint on the selected rivals. Mixture density
  replay uses that selected mask, including seasons drawn by the native mode.
- Native defensive support remains even for a complete case union. Exhaustion,
  zero numerical mass, a score-assignment failure, or a weak pilot do not prove
  impossibility or prune an unrestricted event.
- Every simulated rank uses the production score sampler/sorter. Witnesses are
  candidate generators, never probability estimates. No score cap is introduced.
- Interval guidance uses direct lower/upper tails, and stable interval subtraction
  for two-sided bounds. It changes Q only; every draw retains the full importance
  ratio and full mixture density. Approximate marginal guidance is not treated as
  an exact multi-team event probability.
- Proposal/depth selection and batch sizes use training only. Main and check use
  independently derived streams; no estimate is formed by multiplying the screen
  bound and a zero-observation confidence limit.

102 Rust tests passed; two DB-write tests were ignored. Added exhaustive tests
for shared-fixture cardinality screens and interval guidance. Extended the
existing exhaustive weighted-rank test with complete/incomplete partial case
mixtures and interval-guided mixtures. Batch sizing and default-disabled flags
are tested. No DB access or mutations were needed for these saved requests.

## Reproduction and sources

The final variant is opt-in:

```sh
RUST_ODDS_RARE_TAIL_RESIDUAL_LOOP=1 \
RUST_ODDS_RARE_TAIL_RESIDUAL_BOUND_GUIDE=adaptive \
RUST_ODDS_RARE_TAIL_RESIDUAL_MAX_SAMPLES=100000 \
/absolute/path/to/golaberto-odds
```

Without the first flag the new stage is disabled. Explicit bound guidance1 uses
interval guidance alone; unset uses existing rank guidance. Optional
`RUST_ODDS_RARE_TAIL_RESIDUAL_RIVALS=6` changes the block limit locally to the
experiment; it does not change native stages. Older V1/V2 also limited expansion
to widths3–6. The archived measurements identify the exact executables; the
focused source patch reproduces the final V5 implementation.

Build baseline and candidate from isolated copies, excluding unrelated workspace
changes. For example, from the repository root (directories must be empty):

```sh
mkdir -p /tmp/r33-baseline /tmp/r33-candidate
git archive 2290fe044a86fed57c20a5c586ca1ffe87e74e23 odds-rust stats/core | tar -x -C /tmp/r33-baseline
cp -R /tmp/r33-baseline/. /tmp/r33-candidate/
patch -d /tmp/r33-candidate -p1 < experiments/rare_positions/2026-10-01-residual-loop/source.patch
cargo build --release --locked -j4 --manifest-path /tmp/r33-baseline/odds-rust/Cargo.toml
cargo build --release --locked -j4 --manifest-path /tmp/r33-candidate/odds-rust/Cargo.toml

python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline /tmp/r33-baseline/odds-rust/target/release/golaberto-odds \
  --candidate /tmp/r33-candidate/odds-rust/target/release/golaberto-odds \
  --production-candidate \
  --flag RUST_ODDS_RARE_TAIL_RESIDUAL_LOOP=1 \
  --flag RUST_ODDS_RARE_TAIL_RESIDUAL_BOUND_GUIDE=adaptive \
  --flag RUST_ODDS_RARE_TAIL_RESIDUAL_MAX_SAMPLES=100000 \
  --seeds 1847,1861 --output /tmp/r33-fresh

python3 experiments/rare_positions/summarize_residual_loop.py \
  experiments/rare_positions/2026-10-01-residual-loop/v5-summary.json.gz \
  experiments/rare_positions/2026-10-01-residual-loop/fresh-summary.json.gz

cargo test --locked -j4 --manifest-path /tmp/r33-candidate/odds-rust/Cargo.toml -- --test-threads=1
```

The tests need the repository reference files: copy/symlink `experiments` into the
isolated copy before integration tests. Do not enable the ignored DB-write tests.
Cold startup and time-based allocation mean reruns need not reproduce exact
wall times or admission decisions, despite fixed RNG seeds.

Artifacts: `2026-10-01-residual-loop/`, containing six compressed summaries,
individual responses and structured odds logs, final analysis, and source patch.

Relevant prior evidence and code:

- `FUTURE_EXPERIMENTS.md`: R14/R21 case-conditioning overhead, R20 native proposal
  improvements, R32 successful existing-pilot confirmations; R33 tracked here.
- `2026-10-01-rust-more-pilot-confirmation.md`: baseline and previous Londrina
  pilot/weight concentration diagnosis.
- `odds-rust/src/joint_caps/propagated/lazy/residual.rs`: exact screen and loop.
- `odds-rust/src/joint_caps/propagated/lazy.rs`: case mixtures and density replay.
- `odds-rust/src/joint_caps/propagated.rs`: interval guidance and importance ratios.
- `odds-rust/src/rare_tail.rs`: frozen-result integration and confirmation gates.

Next priority: measure failures immediately after joint-block draws, then test
whether conditioning/tilting shared games can satisfy the selected rivals and
recipient opponents together. Train exception-case allocations from independent
weighted contributions rather than relying only on products of rival marginals.
Both should be tested by replacing existing work, with unchanged acceptance
checks, before considering a production residual loop.
