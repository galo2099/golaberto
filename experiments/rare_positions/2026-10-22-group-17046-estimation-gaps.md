# Group 17046: reachable finishes without probability estimates

This records the baseline before cost-based recovery admission. Implementation
and follow-up measurements are recorded in
`2026-10-22-cost-based-recovery-admission.md`.

The uploaded Europa League request is preserved byte-for-byte in
`odds-rust/tests/fixtures/group-17046-2026-10-22.json`. It has 36 teams,
144 matches, 54 played and 90 remaining. Every team has played three of its
eight matches. The actual `pt,gd,gf,g_away,w,name` ranking order and five
overlapping zones are retained. No production estimator code was changed for this baseline measurement.

The baseline source, based on commit `b377c7d0`, was rebuilt before measurement;
the executable already present in the environment was older than the source.
All runs below use the production defaults, four workers, and the normal
20,000-season importance scout. Positions below are one-based; response keys
are zero-based. Probabilities are fractions unless stated otherwise.

## Reproduction and coverage

```sh
cargo build --release --locked --manifest-path odds-rust/Cargo.toml
odds-rust/target/release/golaberto-odds estimate \
  odds-rust/tests/fixtures/group-17046-2026-10-22.json \
  /tmp/group-17046-808.json 808 4 2>/tmp/group-17046-808.log
cargo test --release --locked --manifest-path odds-rust/Cargo.toml --test group_17046
```

Each response contains 1,296 team/position cells.

| Seed | Zeros after 100,000-season pool | Zeros entering rare-tail portfolio | Final zeros | Reachable zeros | Undecided zeros |
| --- | ---: | ---: | ---: | ---: | ---: |
| 808 | 49 | 43 | 23 | 23 | 0 |
| 809 | 44 | 38 | 19 | 19 | 0 |
| 810 | 52 | 45 | 22 | 21 | 1 |
| 811 | 44 | 37 | 18 | 18 | 0 |
| 812 | 53 | 46 | 24 | 22 | 2 |

None of these zeros is certified impossible. Seed 810 leaves team 1554/34
undecided; seed 812 leaves 639/33 and 1554/33 undecided. The other zeros have
witnesses or constructive reachability evidence. A witness establishes that
a finish can occur, without establishing its probability.

Seed 808's remaining cells are:

| Team ID | Positions without estimates | Current points | Current goal difference |
| --- | --- | ---: | ---: |
| 173 | 36 | 7 | +2 |
| 176 | 36 | 7 | +6 |
| 394 | 32, 34–36 | 9 | +8 |
| 402 | 33–36 | 9 | +3 |
| 639 | 32–36 | 9 | +8 |
| 1130 | 35–36 | 9 | +6 |
| 1554 | 31–36 | 9 | +7 |

Fourteen cells are zero in all five runs: 176/36; 394/34–36; 402/35–36;
639/34–36; 1130/35–36; and 1554/34–36. These are primarily already successful
teams falling behind almost every rival. Their own losses alone are
insufficient: many rival results and score tiebreaks must align.

The same rebuilt binary gives Champions League group 17045, seed 808,
25 pool zeros, 18 pre-portfolio zeros, and two final zeros. This Europa League
snapshot therefore presents substantially more rare cells to the same bounded
recovery system. That comparison does not identify a single schedule statistic
as the cause of the different probabilities.

## Where recovery runs out of coverage

The point PMFs, matched matrix, and all ten jackknife matrices succeed.
There is no fallback to plain Monte Carlo, no matrix-reconciliation rollback,
and no obsolete 32-team rejection causing this result. Current native proposals
support this 36-team request.

At seed 808, ordinary conditioning accepts no new estimates, point tilting
adds four, peer rescue adds one, and domain rescue adds one. The rare-tail
portfolio then recovers 20 of the remaining 43 cells: three initial final
pairs, 14 extra confirmations, and three expected-cohort pairs.

The native training limit is `floor(64 * 0.45) = 28`
(`odds-rust/src/rare_tail.rs:297`). All 28 constructors and pilots complete;
constructor rejection does not explain this seed. Nevertheless, 15 of the
43 cells receive no initial native pilot. These are exactly positions 34–36
for the five nine-point teams 394, 402, 639, 1130, and 1554. Team interleaving
preserves ascending position order within each team, so positions 31–33
consume their earlier slots. Expected cohorts later recover 1130/34, but the
other 14 omitted cells remain zero in this run.

For leagues above 32 teams, native training uses equal fixed grants through
`src/rare_tail/large_training.rs`, rather than the smaller-league adaptive
proposal alternatives. Setup and pilots consume 523,352,756 of 622,170,000
operation units, or 84.1% of that stage's capacity. Only three ordinary final
pairs are admitted. Extra confirmation separately funds 15 of 25 eligible
candidates and accepts 14; ten eligible candidates remain unfunded.

Expected cohorts select only four cells and accept three. The late complete
tree also selects at most four cells, from its remaining confirmation bank.
Thus existing limits favor bounded work over complete rare-cell coverage.

## Sampling quality also prevents publication

High hit counts can still produce only one or two effective samples when a
few importance weights dominate. Concrete seed-808 failures are:

| Attempt | Main hits | Main ESS | Largest weight share | Outcome |
| --- | ---: | ---: | ---: | --- |
| Extra confirmation, 394/32 | 240 | 3.14 | 55.8% | Main fails; check not drawn |
| Complete tree, 2436/1 | 2,532 | 1.59 | 75.3% | Main fails; check not drawn |
| Complete tree, 639/32 | 405 | 1.96 | 71.0% | Main fails; check not drawn |

The rough main gate requires at least 30 hits, ESS at least four, relative
standard error at most 0.6, maximum weight share at most 0.35, and bounded
batch disagreement. Publication also requires an independent check with
at least 30 hits, ESS at least three, relative error at most 0.75, and main/check
probabilities within a factor of five (`src/rare_tail.rs:1351–1379`). Logged
zero check probabilities for these main failures mean no check was drawn.

The expected-cohort pair for 1554/31 has 235 main hits and 266 check hits,
but check ESS 2.978 fails the minimum of three. Its estimates near 7e-13 are
rejected candidate results, not established probabilities to fill into the API.

The complete tree recovers nothing in any of the five runs. At seed 808 it
counts roots successfully for 26 cells, spending 36,671,540 units before
choosing its four candidates. Two final main streams fail as above. A third
candidate consumes setup and pilot work but cannot fund its final pair;
the fourth cannot fund setup. Total actual work is 114,900,950 of the
121,948,625 residual units, leaving 7,047,675. Better proposal weights and
allocation matter as well as the nominal number of draws.

The family observer is unfunded in all five runs, limiting the evidence
available to the late family fallback. Exact-cap recovery finds no eligible
cells; native full-world complement finds no saturated title or target cap.
Analytic score integration and packed-witness recovery do not support this
away-goal/late-wins suffix. Dropping those ranking keys would change the
requested model rather than solve the estimation gap.

## Additional defect: root counting can exceed its residual grant

Three runs reveal an actual modeled-work overrun in late root counting:

| Seed | Grant for failing root count | Actual root-count work | Excess | Whole overflow stage: actual / available |
| --- | ---: | ---: | ---: | --- |
| 809 | 1,023,519 | 1,416,657 | 393,138 | 17,081,398 / 16,688,260 |
| 810 | 826,468 | 1,451,081 | 624,613 | 1,451,081 / 826,468 |
| 811 | 728,688 | 1,434,515 | 705,827 | 4,304,117 / 3,598,290 |

Each reports `within_grant: false` and stops with
`root-count work exceeded residual-bank grant`. These are operation units,
not milliseconds. Seeds 808 and 812 fit their overflow grants.

`src/rare_tail/overflow_trees.rs:346–393` reserves any positive residual and
calls `BranchStrata::overflow_root_count` without passing an operation limit.
The callee in `proposals/src/joint_caps/propagated/lazy/strata/tree.rs:2697`
limits root count, but performs setup and enumeration before returning its
work total. Overspending is detected afterward. `settle_stage` leaves the
full grant reserved on failure, making `bank_free_after: 0` insufficient to
show the excess. This defect also stops remaining late-tree work; fixing it
alone would enforce admission without supplying the missing probabilities.

## Follow-up priorities and verification

1. Make root counting accept an enforceable operation budget, or provide a
   conservative preflight bound, and add seeds 809–811 to cost regression tests.
2. Review the 28-cell native limit and allocation across positions, plus the
   setup/pilot share and unfunded confirmations. Preserve deterministic work
   accounting and independent final streams.
3. Improve proposals for joint rival ordering and score tiebreaks. Preserve
   full support and publication gates; more raw hits alone do not solve
   concentrated importance weights.
4. Extend full-rule analytic or witness paths only with separate correctness
   checks. Do not replace reachable zeros with arbitrary positive floors.

Coverage is also not a probability-accuracy guarantee. For example, 1554/31
is zero in seeds 808 and 812; other seeds retain weak matched-pool estimates
with ESS zero and relative errors about 0.60–1.00. Those positives were not
independently confirmed. The five seeds characterize this snapshot, without
establishing the true probabilities of its hardest cells.

The new regression tests check the fixture's identity and complete schedule,
the full API matrix and normalization, zero-estimate metadata, and a baseline
floor of 1,273 positive cells for seed 808 while allowing future improvements.
The API test reduces only the game-importance scout to 100 draws; the
production probability pool remains 100,000 seasons. The full seed-808 CLI
response is byte-identical with one and four workers.

Validation: the release build and both new targeted tests pass. The fixture
matches the uploaded bytes, formatting and `git diff --check` pass, and all
five CLI responses have maximum row/column errors below 5.7e-14 percentage
points. These tests characterize coverage; they do not certify the accuracy
of the tiny probabilities. Existing compiler warnings remain. The full
workspace suite was not rerun because this change adds only a fixture, tests,
and this analysis.
