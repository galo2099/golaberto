# Shared blocker conditioning and multi-cell sampling — October 1, 2026

## Recommendation

Adopt the strongest tested configuration in the existing
`RUST_ODDS_RARE_TAIL=coverage` profile. The user authorized shipment after
reviewing the seven gained cell-runs versus one lost across 39 cold paired
requests. The initial recommendation to keep this arm opt-in was too
conservative for the stated goal of reducing zeros with rough estimates.
Eight warm pairs gained one and lost one, so the benefit remains modest and
sensitive to the existing wall-time-funded allocator.

There are six distinct gained cells, all in group16498, rather than seven new
positions in one matrix. One of the seven gains came directly from the shared
sampler. The other six came from changing which existing individual searches
were funded. All reachability/impossibility labels and game importance results
were preserved. Database data was not changed.

### Release configuration

Coverage is now the default when `RUST_ODDS_RARE_TAIL` is unset. Its shared-sampler defaults are
`guided`, two blockers, relative final-total constraints, parallel independent
confirmation, and 0.5 of the existing tail allowance. Explicit overrides take
precedence. `RUST_ODDS_SHARED_CONSTRAINTS=0` restores individual coverage only.
No new overall allowance or cores are added; native mode remains available by
setting `RUST_ODDS_RARE_TAIL=0`. Earlier experiment results below remain unchanged.

## Baseline and comparison

The baseline is the **current Rust coverage implementation**, including the
disabled late-gap experiment, frozen before this work:

* Binary: `/private/tmp/golaberto-r25-baseline`.
* SHA256: `95b1c6c43bd9b84dabf0428f87fb89ac8628f71b7384e065b6d199c29e73bdbd`.
* Both sides: `RUST_ODDS_RARE_TAIL=coverage`, four estimator workers.
* Strongest measured candidate SHA256:
  `1d65d3dd39ea02fd6a5034a74c85d199e7384feca76bc85eaa31bc72fc3fb353`.

Requests were identical saved JSON inputs. Simulations did not use the DB.
A read-only `mysql -u root` query verified team labels; no DB data was changed. Development seeds801/808/818 used current16653, historical16653
and16498. Validation804/817/911 used all seven reference snapshots, including
16982. Reserved1201/1213/1229 used the three difficult snapshots. Servers were
called serially, with alternating baseline/candidate order and no concurrent
compilation or tests. Persistent testing used seed808, ten rounds and eight
measured rounds after two warmups.

This compares against the current coverage profile, not Go, the original native
no-tail timings, or an assumed one-second allowance. The shared stage consumes
the existing tail allowance:35% of calculation time before that stage. Its
clock is inside the original tail clock. Training still stops starting work at
65% of the original allowance; remaining individual work is funded from the
time left. The experiment does not create an additional allowance.

## Implementation

New code: `odds-rust/src/shared_constraints.rs`; integration:
`odds-rust/src/rare_tail.rs`; offline audit:
`odds-rust/examples/shared_constraints.rs`. Existing compact DP layers and native
suffix guidance are reused through crate-local visibility changes. Outside the coverage profile, shared sampling remains disabled unless explicitly
enabled. No proof algorithm is replaced.

1. Collect cells still at zero after existing searches and joint publication.
2. Derive each cell's necessary attainable target-total range and rival
   caps/floors from rank cardinality. Pack wins only when wins immediately
   follow points in the actual phase sort. All ranks and teams use the same rule.
3. Build an exact shared-fixture DP for the target plus one or two blockers.
   Match outcomes are represented once and credited to both endpoints.
4. Group up to four cells sharing their rarest blocker and direction, with
   conditioning masses within a factor of ten. This is a difficulty proxy,
   not an assertion that their actual rank probabilities are similar.
5. Run fresh750-draw pilots. At least two members need three hits and ESS>=2
   before funding the group. Use predicted hit/ESS rates and actual pilot cost
   to choose fixed main/check sizes and decide whether they fit.
6. Generate one season for every group draw, calculate every member's rank,
   and keep separate weighted hit/SE/ESS diagnostics. Sample true conditional
   scores and run the production sorter whenever a requested rank involves a
   points/wins tie. Unique prefix ranks permit integrating scores out.
7. Publish only confirmed estimates into zeros. Pilot observations are excluded
   from published probabilities. Existing reconciliation runs afterward.

Setup is capped at150,000 DP transitions across the shared stage,20,000 states
per layer, three constrained teams and eight pilot groups. Oversized,
unsupported or numerically vanishing proposals are skipped, never declared
impossible. These are experiment limits, not increased native proof budgets.

### The useful strengthening: relative totals

Static bounds such as `blocker >= target minimum` are necessary but weak. They
often produce a target finishing above that minimum and a blocker still below
the target. The stronger arm additionally requires, in the same joint DP:

* For a necessary cap: `blocker final prefix <= target final prefix`.
* For a necessary floor: `blocker final prefix >= target final prefix`.

Suffix min/max intervals prune only states that cannot meet these comparisons.
Complete terminal masses enforce them exactly. The rule is sound: when other
teams already fill the guaranteed-above/below slots at the target's most
permissive bound, the selected blocker cannot occupy an additional such slot
at the target's actual final total. Equality retains later tiebreaker outcomes.

### Correct mixture weighting

Let `F` be the shared blocker envelope, `F_i` each member's necessary joint
event, and `M`, `M_i` their exact DP probabilities. Every queried rank event
`E_i` implies `F_i`, and `F_i` implies `F`. The proposal is10% `P(season|F)`
and90% an equal mixture of the recipient proposals.

Without residual guidance, the complete-season density relative to `P` is:

```
q(s)/P(s) = 0.1/M + 0.9/k * sum[1{s in F_i}/M_i]
```

With guidance, each term also contains the replayed residual-fixture density
`q_i/P`. Guidance uses native suffix rank-cardinality chances and retains a
0.0001 true-distribution share per residual fixture. Independence in that
heuristic is not used as a probability estimate. Every overlapping component
is replayed, rather than using only the component that generated the draw.
True conditional score kernels are shared by all components and cancel.

Each cell's pre-gate estimator is `mean[1{E_i}/(q/P)]`. Implementation stores
the common mass separately and uses the equivalent conditional likelihood
ratio. Estimates are not self-normalized. Fixed independent main/check streams
avoid first-hit stopping; publication gates and final reconciliation can still
affect the distribution of reported estimates, as in the current pipeline.

The unchanged rough gates require main>=30hits, ESS>=4, RSE<=0.6,
max-share<=0.35 and batch-gap<=1.5; check>=30hits, ESS>=3, RSE<=0.75 and
main/check probability ratio0.2--5. Strict confirmation is retained outside
the existing order-of-magnitude quality profile.

## Development screens

Each row has nine paired full requests on three difficult snapshots. Positive
gains/losses count a team/rank/seed occurrence, not distinct cells.

| Proposal | Shared accepted cell-runs | Matrix gains/losses | Active shared stage wall time | Median paired full-request change |
| --- | ---: | ---: | ---: | ---: |
| Common blocker only | 0 | 1 / 2 | 1.6--1.9ms | -2.3% |
| Target + one static blocker mixture | 0 | 0 / 1 | 1.6--2.1ms | -2.9% |
| Same with residual cardinality guidance | 0 | 1 / 1 | 8.0--9.4ms | +1.7% |
| Two static blockers + guidance | 0 | 0 / 0 | 9.1--10.2ms | -0.3% |
| Two relative blockers + guidance,25% of tail | 3 | 0 / 1 | 8.3--29.1ms | +1.5% |
| Relative blockers,50% of tail, concurrent main/check | 9 | 3 / 0 | 8.3--74.2ms | +1.2% |

Early arms' matrix changes arose entirely from pre-existing tail funding.
Neither faster cold timings nor incidental extra positives establish a benefit
for an arm that produced no accepted shared estimates.

The concurrent arm predicts80main hits so its half-size check can plausibly
clear the unchanged30-hit check gate. Its main/check jobs share the same
four-worker executor. Forecasts charge both jobs to scheduling bins; there are
no nested four-core pools. Main sizes are1000--5000, with check=max(1000,main/2).

### Why the relative event matters

In a separate fixed30,000-draw diagnostic for group16498:

| Joint event | Cruzeiro20 hits / ESS | Bahia20 hits / ESS | Draw wall time |
| --- | ---: | ---: | ---: |
| Static floors | 7 / 2.79 | 9 / 2.42 | 211ms |
| Floors relative to actual target totals | 1731 / 655 | 1022 / 290 | 269ms |

The relative sampler produces roughly a hundred times more useful ESS per
millisecond here. The raw draw kernel is slower; its event is much better
conditioned. Static floors wasted almost all draws before even reaching the
requested prefix rank. Relative constraints also reduce DP setup transitions:
about49,000 to16,000 for the full group16498 shared setup.

These longer diagnostics are offline, unpublished and outside the production
request allocation. They explain proposal efficiency; they are not an extra
production time allowance.

## Strongest arm: validation and regressions

Development9 + validation21 + reserved9 = **39 cold paired requests**:

* Seven gained cell-runs, one lost: reachable/undecided zeros126→120.
* Six distinct gained cells; one distinct regressed cell.
* Sixteen directly accepted shared cell-runs. Fifteen already had estimates in
  the paired baseline. Bahia20 at seed1213 is the direct new shared gain.
* No impossibility, reachability or game-importance regressions; no remaining
  undecided zeros in either side of these reference requests.
* Group16982 and the other three complete control matrices stayed complete.

All matrix changes are in16498:

| Seed | Gains | Losses |
| --- | --- | --- |
| 801 | Cruzeiro19; Fluminense20 | none |
| 808 | Chapecoense4 | none |
| 911 | Fluminense20 | none |
| 1213 | Chapecoense5; Athletico17; Bahia20 | Fluminense20 |

The lost seed1213 Fluminense20 estimate was `3.484e-26`. That cell remains proved
reachable. It was estimated by the old individual tail, whose funding changed;
it was not rejected by the new constraint model or an impossibility proof.

Example independently confirmed shared estimates, in probability fractions:

| Snapshot / seed | Cell | Current coverage | Shared main |
| --- | --- | ---: | ---: |
| 16498 / 808 | Bahia20 | 3.595e-19 | 6.401e-19 |
| 16498 / 808 | Cruzeiro20 | 1.876e-18 | 1.941e-18 |
| Current16653 / 801 | Juventude18 | 4.223e-15 | 4.524e-15 |
| Current16653 / 801 | Novorizontino18 | 9.992e-20 | 1.444e-19 |
| Historical16653 / 801 | Fortaleza18 | 1.277e-16 | 1.211e-16 |
| 16498 / 1213 | Bahia20 | zero | 5.665e-19 |

All fifteen comparable accepted results were0.68--1.78 times the paired
baseline estimate. This is useful agreement between proposals, not validation
against a precise golden probability. Ordinary million-season references
cannot resolve probabilities at these magnitudes.

## Full-request cost and allocation stability

Across39 cold pairs, median paired wall change was **-0.55%**, with range
**-8.03% to+6.70%**. Median child-process CPU change was-2.48%; the largest
observed increase was+9.81%. Cold controls can fluctuate even when no useful
shared group exists. These ranges are reported explicitly; they do not show a
guaranteed full-request latency improvement.

Persistent16498/808, ten calls per side, eight measured rounds:

| Metric | Current coverage | Strongest shared arm | Change |
| --- | ---: | ---: | ---: |
| Median measured full request | 830.18ms | 825.83ms | -0.52% |
| Total process CPU, including warmups | 24.39s | 23.98s | -1.68% |
| Peak process RSS | 400.73MiB | 352.50MiB | -12.0% |

The warm matrices matched in six rounds. One gained Chapecoense4; one lost the
same cell. The direct shared Bahia/Cruzeiro probabilities remained reproducible
in these streams, while allocation of remaining old tail work fluctuated.
The eight-round net coverage change was zero. Peak memory can fall because
accepted shared cells do not build their old individual tail models; this is
one measured request, not a guaranteed memory reduction for every group.

The strong arm's coverage gain is6/126 eligible cold cell-runs, about4.8%,
with an observed regression and no warm net gain. Adoption accepts this measured
tradeoff; it does not establish a consistent gain for every seed or group.

## Controlled version of the user's example

The saved six-team synthetic case gives one leader a25--27point lead over five
rivals, with nine games remaining per team. No production team IDs are used.
Under3/1/0 scoring, leader gain<=2 has exactly46WDL paths: all losses,
one draw, or two draws. Fixture-specific probabilities weight those paths.

In two shared groups of three/two recipients,6000 draws each from the simple
recipient mixture produced hundreds or thousands of hits for all five cells,
with ESS435--1832. Their diagnostic estimates were4.06e-8--1.18e-6. Conditioning
only on the leader produced8/0/0 and1/0 hits. The mechanism works when recipient
requirements are sufficiently constraining; conditioning the leader alone can
still leave an unlikely recipient path unsampled. These are diagnostics, not
production-published synthetic estimates or a measured comparison against the
current native individual sampler.

## Tests and correctness limits

Five focused tests cover:

* Necessary absolute/relative constraints for every rank, both directions,
  points-only and applicable wins, on exhaustively enumerated small leagues.
* Exact joint normalizers with shared matches, overlapping mixture density
  normalization, weighted probabilities and independent sampled comparisons.
* Guided likelihood replay and two-blocker/relative proposals.
* The46 weighted nine-fixture paths for gain<=2.
* Real goal-difference/goals-scored sorting after equal final points, checked
  against an enumerated Poisson score matrix.
* Exhausted setup, unsupported scoring/bonus rules and self-fixtures skip
  proposals without proving impossibility.

The full suite passed83 tests with two DB-write tests ignored before the final
relative/concurrent refinements. Final targeted execution passed40 library and
34 estimator tests. No DB-write tests were enabled. This change adds probability
proposals, not reachability proofs; native proof quotas remain unchanged.
Numerical goal tables inherit production support, and no score cap becomes an
unrestricted impossibility certificate. The extra goal test was added after
timing; production sampling logic was unchanged.

## Reproduction and evidence

Portable summaries, exact input/source hashes, accepted-cell comparisons and
regression metadata are in `2026-10-01-shared-constraints/`. The baseline binary
is a local frozen artifact; if unavailable, use the current binary with the
new shared flag unset as the algorithmic baseline. The helper extraction uses
the same old acceptance gates; byte-for-byte timing reproduction still needs
the recorded toolchain, binary and machine conditions. Funding is wall-time
dependent, so fixed requests and seeds alone do not promise identical coverage.

```sh
cargo build --offline --locked --release --manifest-path odds-rust/Cargo.toml \
  --bin golaberto-odds --example shared_constraints -j4

python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline /private/tmp/golaberto-r25-baseline \
  --baseline-flag RUST_ODDS_RARE_TAIL=coverage \
  --candidate odds-rust/target/release/golaberto-odds --production-candidate \
  --flag RUST_ODDS_RARE_TAIL=coverage \
  --flag RUST_ODDS_SHARED_CONSTRAINTS=guided \
  --flag RUST_ODDS_SHARED_CONSTRAINTS_BLOCKERS=2 \
  --flag RUST_ODDS_SHARED_CONSTRAINTS_RELATIVE=1 \
  --flag RUST_ODDS_SHARED_CONSTRAINTS_CONFIRMATION=parallel \
  --flag RUST_ODDS_SHARED_CONSTRAINTS_FRACTION=0.5 \
  --seeds 804,817,911 --output /private/tmp/repeat-shared-validation

# Development/reserved: add --cases 16498,16653 and the corresponding seeds.
# Warm resource/coverage comparison: add --cases 16498 --persistent.
# Ablations: shared mode blocker / mixture / guided, relative flag unset,
# blocker count1 / 2, confirmation flag unset, fraction default0.25.

cargo test --offline --locked --manifest-path odds-rust/Cargo.toml \
  --lib --test estimator -j4

odds-rust/target/release/examples/shared_constraints \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  experiments/rare_positions/2026-10-01-shared-constraints/group-16498-44eabb47-grouped-cells.json \
  30000 guided 2 relative
```

Prior sources: the repository's `odds-rust/README.md`, native
`joint_caps/propagated.rs` and `propagated/lazy.rs`, the
`2026-10-01-rust-rare50.md` campaign, and the user hypothesis/Experiment4 in
`FUTURE_EXPERIMENTS.md`. All quantitative claims here come from the saved paired
experiments or enumerated tests; the likelihood formulas are derived above.

## Next useful experiment

Retain the exact relative DP and multi-cell likelihood implementation. Replace
the fixed25/50% allocation with a decision made after individual pilots exist:
compare actual per-cell ESS and forecast check cost against the corresponding
individual proposals, substitute only demonstrably cheaper groups, and protect
promising unrelated finalists from lost funding. Do not spend a shared batch
just because necessary-event masses look similar. A broader score-aware guide
could help low-ESS members, but its full mixture likelihood and setup/CPU cost
would need a separate measured experiment.

## Shipment verification

The shipped source excludes the separate late-gap experiment. Its disabled
state in the measured baseline did not alter the production path. The release
tree passed `cargo test`: **83 tests passed, two database tests ignored**.
The new pure configuration test checks every selected shared default, native
mode, and explicit disabling without changing global environment variables.
The effective shared configuration is included in the request start log.

Rebuild and run after pulling:

```sh
cargo build --release --manifest-path odds-rust/Cargo.toml
odds-rust/target/release/golaberto-odds serve
```

Retain the existing server address and database arguments/environment when
restarting. Shared-only rollback: add `RUST_ODDS_SHARED_CONSTRAINTS=0`.
The frozen baseline diff is archived as `baseline.patch.gz`; decompress it
with `gzip -dc` when reconstructing the measured source.

Release smoke check: seven paired HTTP requests (all snapshots, seed808, four
cores), measured explicit best configuration versus the new one-flag profile.
There were no positive/zero, reachability, impossibility or game-importance
regressions. The largest paired latency increase was **7.43%** (current16653);
this small verification cohort is not a replacement for the earlier 39 pairs.
A separate full16498 request with the rollback override emitted no shared
sampler events, had no losses, and gained one timing-funded individual cell.
Compact evidence and binary hashes: `2026-10-01-shared-constraints/release-validation.json`.

### Default coverage follow-up

The user requested coverage as the application default. A single profile resolver
now supplies `coverage` for an unset environment variable to sampling, witness
reuse, configuration defaults and request logging. Explicit `0` disables the
portfolio; explicit experimental modes and per-setting overrides are preserved.
The same measured profile and budgets apply; this changes selection, not work
within that profile. Existing joint-allocation regression tests explicitly
disable the independent portfolio to retain their isolated assertions. A new
child-process test verifies default shared sampling, effective settings, native
rollback, proof labels and game importance.

Default follow-up verification: the exact release tree passed the full Rust
suite (**85 passed, two database tests ignored**) with `--test-threads=1`,
serializing full-request tests to preserve the four-worker limit. The no-flags
default and explicit `0` opt-out both passed the child-process integration test.
