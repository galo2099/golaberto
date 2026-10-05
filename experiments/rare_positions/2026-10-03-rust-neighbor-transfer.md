# R54: transfer neighboring-rank proposals

Status: completed. Isolated experiment; production Rust source, defaults and
binary are unchanged. No commit, push or database mutation.

## Question

Can successful Flamengo/11th and Flamengo/13th searches in group16498 supply
useful proposal training for the missing 12th-place estimate? Test generic
neighbor transfer, with no team or position constants in the estimator.

The current pipeline reuses full seasons in neighborhood search, but lazy
proposal seeding selects seasons already at the requested rank. Its complete
branch phase runs after the ordinary lazy attempts, so successful 13th-place
branches cannot seed the earlier 12th-place attempt without a later retry or
reordering work.

## Baseline

- HEAD: `4e812fd506b39a504dc3f771c26e8d027cff2311`.
- Production Rust source revision: `ac92a8ab88656c2e20a24b37c1f3c3d3dcfdb4cd`.
- Binary SHA-256:
  `010a91e9d5f9432f4ddfdd165d13df3bfefed6de502732aa0e3f99d9cc6fbd46`.
- Request: `reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json`.
- Request SHA-256:
  `2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625`.
- Four workers total. Current native Rust timing is the baseline; historical Go
  timings and a one-second allowance are not used.

A fresh baseline seed808 export is byte-identical to the R52 export. It returns
380 positive cells,17 impossible zeros and3 reachable zeros, with no undecided
zeros. Flamengo11th is `4.01148e-20`,12th is zero and13th is `1.17298e-33`, as
probabilities rather than percentages. Its estimator time is1469.96ms and
rare-tail time917.74ms; process1.48s and user+system3.33s. This is one cold
diagnostic observation, not a paired timing cohort.

## Experiment and correctness contract

Compare donor guidance from the better neighboring rank, worse neighboring
rank and both. Capture bounded unique fixture patterns from already funded
weighted hits without consuming RNG values or changing donor results. Lazy
contributions include the target-event normalizer and goal likelihood ratio;
branch contributions also include the stratum normalizer and draw allocation.
Normalize each donor direction separately before combining them equally.

The training set retains the largest individual corrected contribution per
unique outcome pattern, with a deterministic top32 limit. It is a bounded
proposal-training heuristic, not an unbiased sample of the donor event. Existing
verified seasons may supply explicitly labeled unweighted fallback training.

Rebuild the recipient's own target law and propagated domains. Valid neighbor
target patterns can seed its root cache; invalid patterns are excluded by its
own terminal filter. Donor constraint normalizers, infeasibility caches and rank
probabilities are not copied. Soft fixture guidance uses32 pseudo-observations
and one prior pseudo-observation. The native conditional component remains in
the50:50 mixture, with defensive target-root sampling and both densities replayed.
Fresh independent pilot/main/check batches use the unchanged publication gates.

Replace an originally paired additional-confirmation job within its existing
grant, including capture, fitting, setup and pilot work. Failed training must
also be charged. Preserve other initial grants and quarantine replacement
refunds; this can reduce later native admissions if the original job would have
skipped its check. Measure that effect instead of claiming every native
confirmation remains unchanged. Compare actual operation costs with the grant
before publishing; an overrun disqualifies the arm from promotion.

Reachability witnesses train proposals only. Neighboring ranks do not establish
intermediate-rank reachability or justify interpolating probabilities. Complete
seasons use the production score sampler and sorter; no score cap proves
unrestricted impossibility.

## Results and reproduction

### Current-budget screen

Fifteen paired full requests: five fixed seeds (808,1669,1993,2281,2293) for
each donor arm. Processes run sequentially with four estimator workers. Baseline
and candidate use identical requests and ordinary MC allocations.

| Donor arm | Flamengo12 positive runs | New zero estimates / lost estimates | Median full process wall, baseline→candidate | Median CPU, baseline→candidate |
|---|---:|---:|---:|---:|
| Native baseline |3/5|—|—|—|
|11th only|2/5|0 /1|1.371→1.457s|3.237→3.330s|
|13th only|3/5|0 /0|1.407→1.396s|3.251→3.234s|
| Both equally |2/5|0 /1|1.371→1.371s|3.238→3.213s|

Both losses are the same identity, Flamengo12/seed2281, in separate arms.
Positive-to-positive probability changes are not new coverage. The three arms
contain nine baseline positive cell-runs and seven candidate positive cell-runs
for this cell. No previously zero cell becomes positive anywhere in these pairs.
The displayed `meets_precision_goal` field uses a stricter precision criterion
than the active rough-estimate acceptance policy; it is not the coverage count.

Transferred proposals pass the short pilot gate in3/5 better-direction runs,
0/5 worse-direction runs and1/5 combined runs. Where pilots fail, training is
charged and the native fallback has fewer draws. Seed1993 has no originally
paired eligible grant, so no transfer occurs;299,565 capture units remain
unfunded in each arm and explicitly disqualify that allocation policy from
promotion. Numerical reachability classifications, initial MC samples/hits and
game-importance output agree across all6,000 compared cells.

Seed808 combines30 unique corrected patterns from11th and2 from13th. Its
500-draw pilot ESS is1.078, below1.5. The paired grant is147,120,462 units;
capture368,685, scan/fit88,573, setup1,965,132 and pilot7,241,531 are charged.
Its native fallback shrinks8,817→8,237 draws per stream, spends79,825,486
operation units including training, and publishes no12th-place estimate.

The5-seed timing rows are diagnostic, not warm repeated performance benchmarks.
The11th-only median wall increase is6.3% and CPU increase2.9%, without coverage
gain. The other differences are small and do not establish a speed improvement.
Do not adopt any current-budget arm from these results.

The user subsequently requested ten-fold budgets for experiments. Retain the
current-budget screen, then compare native retry and transfer with an equal
ten-fold recipient search grant and pilot allocation. Scout and ordinary MC
pool remain unchanged. Expanded-budget costs are measured separately and do
not imply a production latency allowance or a new default.

### Ten-fold learning arm

The multiplier expands one originally funded recipient confirmation grant,
not the whole request. The ordinary 20,000-season scout and 100,000-season pool
are unchanged. Use the same experimental binary for both controls and treatments:
`native` reuses the selected native proposal; `better`, `worse`, and `both` pay
for fresh neighbor-guided setup and training. Each arm gets a fresh 5,000-draw
pilot and at most ten times its original main/check draw cap, capped at 100,000
per stream. Extra work stays outside the shared native allocation bank and is
logged separately. Other initially funded jobs retain their reservations.

An initial seed808 diagnostic exceeded the expanded grant by 0.64% for native
and 0.15% for combined guidance. Those pre-settlement successes were rejected.
The final learning version reserves 2% draw-cost headroom in both enlarged arms,
before sampling, and still checks actual work before publishing. The multiplier
one path keeps its original allocation. This headroom reduces observed cost
overruns; it does not guarantee an operation bound in advance.

Fresh independent main/check streams use the existing acceptance rules. A pilot
hit, a reachability witness, or a sampling gate pass with excess work is not a
published probability estimate. Report accepted coverage and actual settlement
separately. Selected native controls need an originally funded pair; this arm
does not test a universal policy that grants extra work to every missing cell.

This comparison grants equal maximum work, rather than equal draws or equal
realized CPU. Native allocation uses its original pilot cost; transfer uses its
fresh pilot cost with the native cost as a floor. The intervention includes
root reconstruction, fitting, capture and allocation, so it does not isolate
the effect of fixture fitting alone. Later branch capture can occur after the
recipient confirmation; it is diagnostic overhead that cannot retrospectively
be charged to that job. Unfunded capture, constructor failures with unavailable
setup accounting, and post-sampling overruns prevent a production budget claim.
Physical process and CPU measurements include all executed overhead.

### Ten-fold paired results

Fifteen further paired requests compare the three transfer arms to a native
10× control, using the same five seeds, frozen v3 binary and four workers.
Repeated native controls give identical numerical outputs per seed. They
provide timing observations, rather than 15 independent coverage cases.

| Proposal at 10× | Flamengo12 positive runs | Gains / losses versus native10 | Median process wall, native10→transfer | Median CPU, native10→transfer |
|---|---:|---:|---:|---:|
| Native |5/5|—|—|—|
|11th only|5/5|0 /0|2.670→2.573s|4.809→4.740s|
|13th only|4/5|0 /1|2.654→2.698s|4.830→4.833s|
| Both equally |4/5|0 /1|2.941→2.781s|5.250→5.012s|

The 13th-only loss is seed2293; the combined loss is seed808. There are no
other coverage changes in these pairs. All executed expanded recipient jobs
fit their grants. Seed1993 remains ineligible for the new grant and
retains its existing positive estimate. Initial MC samples/hits, game importance,
reachability classifications and keyed native diagnostics before the experiment
agree for every pair. Across all three arms the native control has1,302 positive
initial-MC-zero-hit cell-runs and transfer1,300; these repeated cell-runs are not
distinct cells.

There are12 expanded treatment jobs (four eligible seeds in each arm), rather
than15. Actual proposal replacement occurs in0/4 better,1/4 worse and3/4 combined
jobs. The other eligible jobs pay for failed training and return to native.
Thus the5/5 better-arm result principally shows a successful native fallback.
The12 expanded jobs have no actual-work overrun. Seed1993 has299,565 unfunded
capture units per transfer run; late post-confirmation capture is zero in all
measured runs. The corrected offline source disables that unused late collection.

| Seed | Current native probability | Native10 probability | Native10 main ESS |
|---|---:|---:|---:|
|808|0|1.314e-25|23.13|
|1669|1.610e-25|1.037e-25|47.38|
|1993|1.030e-25|1.030e-25|7.97|
|2281|1.100e-25|1.049e-25|35.02|
|2293|0|1.189e-25|12.46|

The native retry recovers two missing cell-runs, both for the same cell,
Flamengo12. Its five estimates span1.030–1.314e-25. This is convergence evidence
for a rough scale, not a certified reference probability. The current-budget
controls and enlarged controls ran in separate cohorts; their timing comparison
must be read with that limitation. Expanded native medians are roughly2.65–2.94s
wall and4.81–5.25s CPU, versus1.37–1.41s and3.24–3.25s in the current-budget
controls. The experimental allowance therefore has a substantial real cost.

Seed808 native10 uses a1,471,204,620-unit grant (147,120,462 originally), spends
1,451,712,013 units and samples83,951 draws in each independent final stream.
Main/check probabilities are1.314e-25 and1.208e-25, with ESS23.13 and29.80.
The combined transfer spends762,461,436 units, draws46,949 main seasons and
fails the unchanged main publication gate, so it runs no check. Its2,758 hits
have ESS7.79. More event hits therefore do not necessarily provide more useful
weighted evidence. Transfer from11th at seed808 fails its fresh pilot and falls
back to native with82,320 draws per stream; its eventual positive estimate is
evidence for the native retry, rather than a successful transferred proposal.

The corrected diagnostic replay identifies the combined seed808 failure:
`main_batch_gap=1.7445`, exceeding the active maximum1.5. Its relative SE0.3582
passes the active0.6 threshold; ESS7.79 and maximum share0.1894 also pass their
respective thresholds4 and0.35. Do not confuse the stricter exported precision
flag with the active rough publication policy. The independent check is skipped
when the main batch fails this consistency rule.

For seed808 native10, the matrix changes380→381 positive cells and3→2 reachable
zeros. The17 proven impossible zeros and zero undecideds remain unchanged.
The remaining reachable zeros are team16, Palmeiras, ranks14 and15. This arm
increases the selected Flamengo search only.

## Recommendation and learning

Keep the neighbor-transfer arms offline. They add no coverage at either budget,
and some arms lose estimates. The successful enlarged native retry shows that
the existing proposal contains usable paths: additional independent confirmation
can stabilize its rare weights without changing the target model. This result
supports investigating selective confirmation allocation before more donor
fitting. It does not establish a universal 10× budget policy.

The transfer experiment uses bounded warmstarts and soft fixture marginals.
It does not test hard rival-side changes between neighboring ranks. A future
variant could rebuild joint rival cases after changing one rival's side of the
target threshold, then measure the resulting variance within the same grant.
That variant remains untested; it cannot inherit a donor rank probability or
claim intermediate-rank reachability from neighboring estimates.

## Reproduction

Toolchain: `rustc1.93.1` and `cargo1.93.1`. The artifact directory is
`experiments/rare_positions/2026-10-03-neighbor-transfer/`.

The current-budget cohort uses `candidate-soft-v1.patch`. The enlarged-budget
cohort uses `candidate-highbudget-v3.patch`. Each patch is relative to the Rust
crate, with base HEAD4e812fd506b39a504dc3f771c26e8d027cff2311. Build each in its own
fresh source tree with the matching sibling `stats/core` dependency:

```bash
r54_tree=$(mktemp -d /private/tmp/golaberto-r54-repro.XXXXXX)
git archive 4e812fd506b39a504dc3f771c26e8d027cff2311 odds-rust stats/core |
  tar -x -C "$r54_tree"
git -C "$r54_tree" apply --directory=odds-rust \
  /Users/robsonaraujo/work/golaberto/experiments/rare_positions/2026-10-03-neighbor-transfer/candidate-highbudget-v3.patch
cargo build --manifest-path "$r54_tree/odds-rust/Cargo.toml" --release -j4
```

From the repository root, reproduce the enlarged comparison with the rebuilt
candidate binary as both native-control and treatment binary:

```bash
python3 experiments/rare_positions/2026-10-03-neighbor-transfer/run_pairs.py \
  odds-rust/target/release/golaberto-odds \
  "$r54_tree/odds-rust/target/release/golaberto-odds" \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  --output-dir /private/tmp/r54-repro-highbudget \
  --workers 4 --seeds 808,1669,1993,2281,2293 \
  --transfers better,worse,both --cell-selector 17:12 \
  --baseline-binary candidate \
  --baseline-env-json experiments/rare_positions/2026-10-03-neighbor-transfer/highbudget-native10.env.json \
  --candidate-env-json experiments/rare_positions/2026-10-03-neighbor-transfer/highbudget-transfer10.env.json \
  --execute
python3 experiments/rare_positions/2026-10-03-neighbor-transfer/analyze.py \
  /private/tmp/r54-repro-highbudget \
  --request experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  --output-prefix /private/tmp/r54-repro-highbudget/analysis
```

For the current-budget comparison, use a fresh tree with the soft-v1 patch and
omit the three high-budget baseline/environment arguments. The harness scrubs
inherited `RUST_ODDS_` and `RARE_POSITION_` settings, records explicit flags and
binary/request hashes, alternates pair order, and serializes processes. CPU is
child user plus system time from Python `resource`; wall time encloses the
complete CLI process. Request decoding, model setup, estimation and export are
included; these are not HTTP round-trip timings. The macOS `time -lp` diagnostic
footer cannot read `kern.clockrate` in the sandbox, so those development timings
are kept separate from the successful paired harness runs.

The selector is a generic team ID and one-based rank resolved through the model;
it is not the request's team array index. Development logs from the earlier
incorrect index selection are excluded from results. Source patches are
recoverable offline experiments, and production source/defaults are unchanged.

## Validation and preserved versions

- Current-budget measured binary: soft-v1,
  SHA`98fecb8fa1480e2e6b9258abd985cf4fd7b52eec6428948f946eb8043bcd774f`.
- Enlarged-budget measured binary: v3,
  SHA`687d71a1eb522a065b7c357ccdaf9a5291b7cc9e4e6e296d381031e02fdff53d`.
- Corrected experimental source: `candidate-highbudget-v4-capture-phase.patch`,
  SHA`014aa1db4163e6abeee1ec4a0a97a9575f3be982f0292b1f3d8914e9d35e8b0e`.
  Its binary SHA is
  `47a4ac2fbd699a2e2673f94b1204a1e3c3a2327a68822ec357436e40f9d5c0be`.

V4 disables donor capture in the final post-confirmation branch pass and logs
the individual main sampling criteria. The measured v3 cohorts have zero late
capture, so this correction does not alter their ledger or scientific results.
V4 native10 and combined10 seed808 exports are byte-identical to v3; operation
counts and publication decisions agree. The archived v3 cohort was not rerun
under a differently labeled binary.

`cargo check` passes. The Rust suite passes138 tests:88 library,35 estimator,
and15 remaining integration tests. Three database tests are ignored because
they require `MYSQL_TEST_URL`. The two HTTP tests initially could not bind
localhost in the sandbox; a focused run with localhost permission passes2/2.
The existing worker/repeat/logging integration test passes. No DB mutation was
used. Exact commands and observed terminal counts are preserved in
`source-corrected-v4/validation.md`; Cargo stdout was not redirected to a
persistent raw log.

The separate six-run v4 invariant matrix confirms byte-identical flag-off output
versus production and identical active outputs with one/four workers and logging
off/on. Initial MC hashes agree throughout. Multiplier10 without a transfer mode,
and native10 without a cell selector, both preserve baseline output. Focused
tests cover donor direction normalization, bounded multiplier/overflow/draw caps,
phase-restricted capture and unchanged quality gates; exhaustive small-model
proposal tests cover support and replayed weighting.

Production `odds-rust` and `stats/core` remain unmodified. Experimental source is
preserved as patches and hash manifests. User changes in `db/schema.rb` and
unrelated files were preserved. No commit or push was performed.

### Evidence archives

The artifact directory preserves three separate `raw-evidence.tar.gz` bundles,
with external manifests and internal member checksums:

| Subdirectory | Contents | Archive SHA-256 |
|---|---|---|
| `current-budget` |15 soft-v1 paired requests, commands, logs, exports and analysis|`719f317bb5c0a4449651f57dfa6051d7fce21dde40e55b7bfe031c282ec13836`|
| `high-budget-v3` |15 native10/transfer10 pairs, configurations, commands, logs, exports and analysis|`1aa8ffb89a85dce6a34294e122ff5ca3ae2bc0f1b910ff353cf589e358db4429`|
| `source-corrected-v4` |Diagnostic replays, multiplier controls, six-run invariants and validation record|`2b693f587889201d566c7cdd20e8773864f8ddae957487dcea9afc16837aacf6`|

Archive packaging verifies every evidence member. Binary and source hashes,
request identity, flags and exact commands are recorded in provenance files.
The source manifests and versioned patches sit beside the archives. Development
v2 overruns are labeled separately from the final settled v3 measurements.

## Scientific basis

Importance sampling estimates an event using its original probability density
divided by the sampling density. The proposal must retain support wherever the
event has positive probability; a poorly fitted proposal can increase variance.
These requirements motivate rebuilding the recipient law, replaying both mixture
densities, and retaining the native component. See [Owen, importance sampling](https://artowen.su.domains/mc/Ch-var-is.pdf)
and [He and Owen, mixture importance sampling](https://arxiv.org/abs/1411.3954).
Neither source establishes that neighboring ranks will improve this estimator;
that is the empirical hypothesis tested here.
