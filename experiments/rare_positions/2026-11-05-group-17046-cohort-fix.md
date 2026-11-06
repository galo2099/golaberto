# Bound visited cohorts when constructing reachability witnesses

The default production configuration now resolves every cell in the November 5
Europa League request: 1,286 reachable, ten impossible and zero undecided, at
all four tested seeds. The change preserves probability estimates and the
existing node, depth and cohort limits.

## Change

The [original analysis](2026-11-05-group-17046-undecided.md) found four reachable
cells blocked by a preflight check. It rejected a search whenever the total
number of possible rival cohorts exceeded 100,000, although the first cohort
was enough to construct a witness for each affected cell.

`RankProof::feasible_domains_with_limits` now starts the witness search and
charges its cohort quota as cohorts are visited. The shared solver checks the
quota before incrementing the counter and before looking up a learned conflict.
Cached cohorts therefore consume the quota too. Reaching the quota before
completion returns unknown and cannot publish an impossibility proof. Exhausting
all cohorts exactly at the quota can still establish a valid refutation.

Exhaustive impossibility searches retain their combinatorial preflight. They
need to refute every cohort, so that early check avoids spending their earlier
proof budgets on traversals that cannot finish. This keeps the probability
pipeline and its allocation unchanged.

The packed witness limits remain 100,000 cohorts per search, 1,000 fixture
nodes per cell, 6,000 fixture nodes per request, and depth 512. Witnesses still
need score completion and verification through the production sorter before
any reachability label changes. Only the witness-capture admission rule changes;
no larger-budget or training experiment is enabled.

## Before and after

Both executables use production commit
`91b1226c2d7f9c03301f946c6cd05d2bac2db8ad` as the base. The candidate includes
the cohort-quota change. Runs clear inherited `RUST_ODDS_*` and
`RARE_POSITION_*` variables, use four workers and retain normal defaults.

| Seed | Previous undecided cells | Fixed undecided | Fixed packed nodes |
|---|---|---:|---:|
| 808 | Team 3169, 30th | 0 | 1,600 |
| 809 | Team 402, 30th and 31st | 0 | 1,647 |
| 1669 | Team 402, 30th and 31st | 0 | 1,779 |
| 2293 | Team 402, 30th/31st; team 2436, 6th | 0 | 1,767 |

Every result has the same ten impossible cells. All 1,296 metadata entries
remain present. Team odds, game importance and all probability and uncertainty
fields match their baseline values exactly. The fix supplies missing
reachability proofs without inventing probability estimates; unestimated counts
remain 72, 72, 76 and 78.

The older November 4 group 17045 and October 22 group 17046 fixtures also retain
zero undecided cells across the same four seeds. The three-fixture comparison
contains twelve measured baseline/candidate pairs and discarded warmups.
All comparisons and native-stage budget audits pass, with no lost estimates,
changed impossible-cell sets or unexpected numerical-field differences.
The older fixtures' eight responses match baseline byte for byte. For the new
request, removing only the reachability labels makes each full response equal
to its baseline; exactly one, two, two and three labels change across seeds.

Direct packed construction now resolves the November 4 fixture without using
the later reuse pass. Its regression expectations change from 23 intermediate
undecided cells to zero; numerical equality, refutation and complete-grid
assertions remain in place. Reuse behavior retains its separate unit coverage.

## Bounded work and validation

The new request's packed passes use 1,600–1,779 fixture nodes, within the
unchanged 6,000-node limit. Aggregate cohort counters can exceed 100,000 because
that quota applies separately to each search, not to the entire request.
The counter includes cache hits, so a small fixture-node count cannot bypass
the independent cohort bound.

The serialized timing screen runs without overlapping Rust compilation. Across
the twelve measured pairs, summed child CPU is 96.28 seconds at baseline and
95.06 seconds for the candidate; the median paired CPU ratio is 0.992. No
material regression appears in this small screen. One measured repetition per
seed does not establish a speed improvement.

Focused tests cover first-cohort capture in a large hypothetical cohort space,
zero quota, exhaustion after a refuted prefix, exact-quota complete refutation,
and cache-hit quota accounting. Both supported depth widths are exercised.
The twelve focused rank-proof tests pass. A new default-configuration CLI
regression checks all four seeds and compares one and four workers at seed 808,
alongside the saved scored-witness tests.
An older unit case that now obtains a verified witness is tested as reachable;
a separate one-node test retains genuine exhaustion coverage. All twenty
focused packed-witness tests pass, with probability metadata preserved.

The full release suite passes: **581 passed, zero failed, five ignored**.
Formatting and whitespace checks pass. The full-suite result includes the
new default-configuration CLI regression and the existing fixture, exhaustive
refutation, native-support and worker-determinism checks.

## Reproduction

The baseline executable SHA-256 is
`a367e66936e7435966fc7955693f4ac55b1b442c88f70bdca5fa084f8e512556`;
the measured candidate is
`c5a6fc0c36c4a582ea19c6699cf3217e760959dc3f0ce08994aa4a7c7fa915a3`.
The [result artifact](results/2026-11-05-group-17046-cohort-fix.json) contains
request hashes, per-seed classifications, exact comparisons, packed-stage
counters, budget audits and descriptive timings. Raw files and the collector
are under `/workspace/work/production-17046-cohort-fix-2026-11-05/`.
