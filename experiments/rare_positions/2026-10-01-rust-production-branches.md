# R36: production-path complete-branch experiment

## Adoption

After reviewing these results, the user authorized adopting the recommendation.
The ordinary complete-branch stage is now enabled by default in the coverage
profile. Set `RUST_ODDS_RARE_TAIL_BRANCHES=0` to opt out. The experimental
bound-based two-draw floor remains disabled. The experiment below records the
pre-adoption comparison; its archived source and results are unchanged.

Adoption verification: all 113 current-workspace Rust tests passed; two
database-write tests were ignored. The default-profile integration test checks
that the stage runs, preserves existing estimates, and remains absent when the
coverage portfolio is disabled. Unit tests cover explicit stage opt-out and
confirm that the experimental bound floor stays off.

## Recommendation and scope

Recommend the ordinary complete-branch rescue as an opt-in production candidate:
`RUST_ODDS_RARE_TAIL_BRANCHES=1`. It fills remaining zeros after native results
are frozen, using the same four workers and unchanged rough-estimate gates.
It is implemented in the actual `/odds` calculation, not an offline cell-only
example. Testing runs on local HTTP listeners; there is no access to the
production Xeon and no live deployment, database mutation, commit or push.

The final49 paired full requests across all seven snapshots gained five
cell-runs for **one distinct cell: Londrina95/3rd in group16653**. It was found
in5/7 seeds versus0/7 baseline. No nonzero losses or proof regressions. Aggregate
CPU+0.52%, complete HTTP wall+2.16%; current16653 wall+4.32%. This is an additional stage; it does not
replace existing work. Current defaults remain unchanged.

Two previously unused seeds were included in final validation. The five initial seeds were already used
offline in R34; the production validation on1847/1861 verifies integration,
and is not an unseen test of the sampling strategy itself.

## Production integration

Code: `odds-rust/src/rare_tail/branches.rs`, called at the end of the existing
rare-tail calculation only when the new flag is exactly1.

1. Finish existing shared, ordinary, extension and larger-confirmation work.
   Retain all positive estimates and all impossibility labels. Consider at most
   eight remaining non-impossible zero cells, using their generic table order.
2. Fully enumerate supported target W/D/L paths and strict exception cases.
   Use all-team points/applicable-wins propagation, forced fixture compaction,
   and the existing four-rival joint proposal. Refine one constrained rival's
   complete outcome paths as in R34. No team, group or rank is hardcoded.
3. Setup caps:64 target paths,256 attempted exception cases,100k cumulative joint
   setup nodes per proposal and four-million guide values. Each secondary rival
   has at most16 complete paths. Reject incomplete enumeration or setup failure;
   neither establishes impossibility. Proposals with more than150 strata cannot
   fit the200-draw floor in the30k budget and are skipped.
4. Compute the cheap fixed/domain/joint necessary-event bounds, without the
   six-rival or read-2 diagnostic DPs. Order branches by decreasing upper bound.
5. For each branch, run two independent500-draw pilots: rank guidance and interval
   guidance. Choose higher pilot ESS. Train allocation from contribution standard
   deviations, with200 draws per branch plus10% equal/90% variance allocation.
   Pilots do not contribute to the reported estimate.
6. Admit a fixed30k whole-cell main/check pair only if its pilot-based wall
   forecast fits the remaining stage allowance. Main and check have separate
   streams; the check is skipped if the main fails the existing gate. Every
   branch retains positive allocation. Sum branch means and variances, including
   actual branch draw counts, as tested against exhaustive small-fixture models.
7. Apply unchanged acceptance gates. Accepted results can fill only zero cells;
   their design is `matched_point_pool_complete_branches`. Preserve prior
   positive estimates and metadata inside the stage; subsequent matrix
   reconciliation can make normal adjustments to probabilities.

The allowance defaults to15% of the **preceding complete Rust calculation wall**,
not15% of an assumed one-second budget. Set
`RUST_ODDS_RARE_TAIL_BRANCH_BUDGET_FRACTION` to0–0.5; zero admits no work. The
deadline is soft: a constructor or fixed admitted pair can overrun the forecast.
No measured stage allowance overrun occurred in the49 main-arm pairs.

Logs include `rust_odds_rare_tail_branches`, `_skip` and `_summary`, with setup,
bounds, pilot, main/check times, quality metrics, draw counts, skips, work,
allowance and `preserved_existing_estimates`. Startup logs include the new flags.
There is no change to the HTTP response shape for this experiment.

## Why this arm keeps all branches

R35 tested omission with an explicit missing-mass bound. The production arm
keeps full event support instead, so its estimate does not require a new
omitted-probability field in the response. A zero pilot is not a pruning proof.

An alternative flag, `RUST_ODDS_RARE_TAIL_BRANCH_BOUND_FLOOR=1`, gives branches
whose upper bounds are tiny relative to training estimates a two-draw floor,
then reallocates the saved draws to other branches. The bound threshold is
`0.001 × combined pilot estimate / branch count`. It also retains every branch,
freezes allocations before independent final draws, and preserves unbiased
branch means conditional on training. It is not enabled in the main arm.

## Paired full-request design

Baseline: shipped Rust2290fe044a86fed57c20a5c586ca1ffe87e74e23; executable SHA256
`01a2737525c254896ff78c0f3836ca59ed5382f928bbec1199745a57af04b5ce`.
Candidate: isolated baseline source plus the focused R33/R34/R35/R36 dependency
patch. The R33 optional residual-loop stage remains disabled. Unrelated late-gap
and concurrently edited public-reachability response changes are excluded.

All requests use identical saved snapshots, model probabilities and seeds, with
all other Rust/Go experiment flags removed from the environment. Default coverage
and confirmation1.5 remain in place. Four cores total; runs/builds/tests are
serialized. Baseline/candidate order alternates by seed. Each request uses a
fresh server bound to a temporary loopback port, with listener startup excluded
from HTTP latency. CPU is complete child process user+system time, including
startup. The service performs20k scout and100k pool seasons in every full request.

Local platform: ARM Mac, Rust1.93.1. CPU fractions are local measurements;
production uses four Xeon E5-2697 cores and has not been measured.

Seven snapshots:15902,16413,16498,16982,16983, current16653-71d4fea8, and historical
16653-2d1c1d6f. Development seeds808,1669,1993:21 pairs per variant. Integration
validation1847,1861:14 more pairs per variant. A no-flag candidate control uses
seed808 on all seven snapshots:7 pairs. Raw responses, flags, input/executable
hashes and timings are archived.

Two genuinely held-out seeds2281,2293 add14 main-arm pairs on the final release.
Total experiment:49 main-arm pairs,35 alternative-allocation pairs and7 no-flag
controls, **91 paired requests /182 full HTTP calculations**. Different arms
have their own baseline cohorts; do not sum gains across repeated configurations.

## Final main arm: seven seeds

Held-out seed2293 accepts Londrina/3rd at3.885e-34, main ESS14.36 and check ESS7.12.
Seed2281 has main ESS4.25 but maximum weight share0.461>0.35; it is rejected
without a check. Held-out14 pairs gain one cell-run, zero losses, CPU+0.30%,
HTTP wall+3.18%. No other new cell is discovered.

| Snapshot | Pairs | Baseline mean HTTP ms | Candidate mean HTTP ms | Gains | Losses |
|---|---:|---:|---:|---:|---:|
| 15902 | 7 | 165.76 | 171.65 | 0 | 0 |
| 16413 | 7 | 155.43 | 155.57 | 0 | 0 |
| 16498 | 7 | 1168.36 | 1197.76 | 0 | 0 |
| 16653 current | 7 | 1068.96 | 1115.15 | **5** | 0 |
| 16653 historical | 7 | 683.94 | 678.00 | 0 | 0 |
| 16982 | 7 | 262.29 | 270.43 | 0 | 0 |
| 16983 | 7 | 263.36 | 261.01 | 0 | 0 |
| All | **49** | **538.30** | **549.94** | **5** | **0** |

Reachable-zero cell-runs41→36; undecided-zero counts remain zero. No nonzero or
proof regressions and all in-stage preservation assertions passed. Londrina/3rd
remains zero in seeds1993 and2281. Group16498's reachable zeros remain unresolved
by this stage. Full-request timing includes native timing-allocation variation;
the target group's46.19ms average increase exceeds the direct branch work alone.
We report that observed increase rather than assigning all of it to new sampling.

## Main arm: first five seeds

| Snapshot | Pairs | Baseline mean HTTP ms | Candidate mean HTTP ms | Gains | Losses |
|---|---:|---:|---:|---:|---:|
| 15902 | 5 | 165.49 | 173.49 | 0 | 0 |
| 16413 | 5 | 156.73 | 156.82 | 0 | 0 |
| 16498 | 5 | 1110.54 | 1137.64 | 0 | 0 |
| 16653 current | 5 | 1091.38 | 1117.03 | **4** | 0 |
| 16653 historical | 5 | 693.45 | 689.41 | 0 | 0 |
| 16982 | 5 | 263.99 | 274.82 | 0 | 0 |
| 16983 | 5 | 264.66 | 262.47 | 0 | 0 |
| All | **35** | **535.18** | **544.53** | **4** | **0** |

Aggregate reachable-zero cell-runs30→26; undecided-zero cell-runs remain zero.
All impossible labels and existing positive cells are retained in paired outputs.
Every emitted in-stage preservation check is true.

Londrina/3rd uses the same22 branches as R34. Main estimates before reconciliation:

| Seed | Accepted | Main fraction | Main ESS | Check ESS |
|---:|:---:|---:|---:|---:|
| 808 | Yes | 4.935e-34 | 6.06 | 8.31 |
| 1669 | Yes | 5.237e-34 | 9.53 | 7.76 |
| 1993 | No | 1.092e-33 | 3.31 | — |
| 1847 | Yes | 3.350e-34 | 6.34 | 4.75 |
| 1861 | Yes | 3.402e-34 | 7.57 | 12.06 |

Seed1993 fails main ESS and weight-share gates; no check is run. Retained positive
values are roughly e-34 fractions, or e-32%. These are order-of-magnitude estimates
meeting current finite-batch gates, not independently certified exact probabilities.

Mean measured branch work for this cell is27.32ms (setup+bounds+pilots+main/check).
Accepted runs draw22k training plus30k main plus30k check seasons; the rejected
main saves the check. Four workers sample branch jobs without nested parallelism.

Group16498's remaining zero cells—team17 ranks12/13 and team16 ranks13–16,
depending on seed—exceed64 complete target paths. They are skipped in roughly
0.65–0.97ms per setup; whole added stage averages a few milliseconds. No branch
estimate is produced for them. Other snapshots have no remaining eligible zeros
in this cohort. This is generic code with a limited useful regime, not a cure
for all remaining zeros.

## Allocation variant and no-flag control

The bound-floor variant's35 paired requests show six raw gains and zero losses,
CPU+3.45%, HTTP wall+3.48%. Only four gains are branch-stage estimates, all the
same Londrina/3rd seeds. The other two are16498 team16/11th and12th at seed1669,
from the pre-existing ordinary tail and larger-confirmation stages. Baseline
time-based allocation differed in that cohort. Do not credit these two gains to
the new branch allocation. It yields no additional branch-stage coverage.

The no-flag control has0 gains/losses over7 pairs, CPU+1.73%, HTTP wall+1.98%.
This demonstrates that small end-to-end differences also arise from build/layout
and native timing variation. Do not interpret0.60% as a precise causal CPU cost
of the new stage, or subtract the limited control cohort from the headline.
The dedicated measured branch-stage timings describe its direct added work.

Keep the ordinary200-draw floor: the smaller floor has not demonstrated better
coverage in these production comparisons. Reusing cheap bounds is sufficient;
the costly six-rival bounding pass from R35 is not part of either production arm.

## Tests and reproducible commands

The focused full Rust suite passes **110 tests,2 DB-write tests ignored**, with
serial test harnesses and at most four calculation cores. Library tests include
complete branch enumeration/probability bounds, shared-fixture weighting,
underflow safety, positive allocations, fixed budgets, opt-in defaults and
preservation of earlier estimates. A transient test failure occurred when a
concurrent response-label change was copied into the isolated source; removing
that unrelated change restored the matched source/test contract. Workspace edits
from that other task were preserved.

```sh
cargo build --release --locked -j4 --manifest-path odds-rust/Cargo.toml
cargo test --locked -j4 --manifest-path odds-rust/Cargo.toml --tests -- --test-threads=1

RUST_ODDS_RARE_TAIL_BRANCHES=1 \
  odds-rust/target/release/golaberto-odds estimate \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16653-71d4fea8.json \
  /private/tmp/branches-response.json 808 4

python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline /private/tmp/golaberto-r32-default \
  --candidate odds-rust/target/release/golaberto-odds \
  --production-candidate --flag RUST_ODDS_RARE_TAIL_BRANCHES=1 \
  --seeds 808,1669,1993 --output /private/tmp/r36-full
```

Use1847,1861 for the second cohort, and2281,2293 for held-out validation. Add
`--flag RUST_ODDS_RARE_TAIL_BRANCH_BOUND_FLOOR=1` to reproduce the alternate arm.
The no-flag control omits both flags. Compile the frozen baseline separately
from2290fe04; use the archived source patch to reproduce the focused candidate.
Running the whole shared workspace can also include unrelated pending changes.

Artifacts: `2026-10-01-production-branches/`, including compressed summaries,
request responses/logs, source patch, flags, hashes, tests and final release smoke.
Measured binaries precede final diagnostic-log fields and invalid-allocation
guards; sampled allocation paths in these cohorts are unaffected. The final
release is checked with full HTTP requests before recommendations are finalized.
