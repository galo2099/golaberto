# Replacing the late cohort cap with a traversal-work budget

This experiment replaces the late packed witness search's completed-cohort cap
with a deterministic budget for recursive traversal and cached-conflict
comparisons. It also prunes entire cohort subtrees when their partial rival set
already contains a learned conflict. Measurements were performed in the isolated
checkout `/workspace/golaberto-cohort-pruning-17047/`.

## Candidate and baseline

Both arms start from production commit
`6d7b7da0e714ffb84e8e0a4f4820aae52ea884ad`. Earlier uncommitted training
experiments and the separate fixture-analysis checkout remain untouched.

The frozen baseline executable SHA-256 is
`5eac0e0c71c521971731d8c17888cafe095deddccd0942f9eae2671fcf679dfa`.
The frozen candidate executable SHA-256 is
`d3862d803402d169a12557f3822b391a948d4c9dadf0d1fd8c2d2dbd1ac6eb72`;
its diagnostic probe SHA-256 is
`5b83e8e37fa43942fb331839bdfc875e12115db026cf430e03dd48871e68a28f`.
The candidate build used a separate build directory; its exact command and log
are retained with the frozen binaries. `Cargo.lock` is unchanged.

Only the late packed search uses the new work mode. Earlier exhaustive proof
searches retain their combinatorial preflight, node budgets and traversal.
The legacy cohort-limited capture API also retains its original behavior.
Training, sampling, outcome selection, score completion and final sorter
verification remain unchanged.

## What the budget measures

A traversal-work unit is charged before each recursive cohort entry and before
each comparison against a cached conflict core. Internal prunes and leaf cache
hits both consume work. The new mode has no completed-cohort quota.

Each cell receives `100_000 * (team_count + 1)` units, shared across the preferred
and alternate orientations. The request receives six times that amount. For
these 36-team fixtures this means 3,700,000 units per cell and 22,200,000 per
request. These are deterministic operation counters, rather than wall-clock
deadlines or a count of all CPU instructions.

Fixture-search limits remain 1,000 nodes per cell and 6,000 per request, with
depth at most 512. Existing goal-completion and reuse bounds remain in force.
Conflict-cache maintenance is bounded by the fixture-node allowance and is not
included in the traversal counter.

When another charge is denied, the incomplete search returns unknown. A proof
that finishes exactly at its allowance remains valid. Work consumed by the first
orientation cannot be spent again by the second, and work consumed by one cell
cannot be spent again by a later cell.

## Complete production responses

On the new Conference League request, all four seeds (808, 809, 1669 and 2293)
give the same classifications across warmup and measured repeats:

| Arm | Reachable | Impossible | Undecided |
|---|---:|---:|---:|
| Production baseline | 1,258 | 26 | 12 |
| Work-budget candidate | 1,262 | 34 | 0 |

The changed cells are teams 367, 809, 848 and 1223 at positions 29–31. Each 29th
position has a verified scored witness; each 30th and 31st position is refuted.
All 1,296 probability estimates, team odds, game importance and other estimate
metadata match the baseline. The only permitted numerical changes are the eight
`zero_hit_upper_95` fields becoming zero for newly proved impossible cells.
Earlier pipeline stage counts/work and native training audits match exactly.

At seed 808, the candidate's late search uses 6,710,998 traversal-work units and 2,665 fixture
nodes, with no denied work charges. It visits 756,559 recursive cohort branches
and prunes 275,502 internal subtrees. The complete seed-808 response is
byte-identical with one worker/logging disabled and four workers/logging enabled.

## Paired runtime comparison

The comparison covers the November 4 group-17045, November 5 group-17046 and
November 6 group-17047 fixtures at seeds 808, 809, 1669 and 2293, using four
workers. Each arm receives one warmup and two measured runs per fixture/seed:
72 requests in total, with 48 measured requests forming 24 pairs. Arm order
alternates. Requests run serially after compilation and testing have finished.
Inherited `RUST_ODDS_*` and `RARE_POSITION_*` settings are removed; only logging
is enabled.

All 36 paired response comparisons, including warmups, pass every semantic gate.
The older fixtures' responses are byte-identical in all 24 pairs. The new
fixture changes only the twelve classifications and eight impossibility bounds
described above. Probability estimates, team odds, game importance, earlier
pipeline work and native training audits match across all pairs.

The table uses eight measured requests per arm for each fixture. CPU is total
child user plus system CPU; packed-pass time is the median elapsed time recorded
by the `search.packed_witness` stage.

| Fixture | Baseline CPU total (s) | Candidate CPU total (s) | Baseline packed median (ms) | Candidate packed median (ms) |
|---|---:|---:|---:|---:|
| 17045 | 53.85 | 53.47 | 53.63 | 54.93 |
| 17046 | 61.34 | 60.27 | 32.56 | 29.69 |
| 17047 | 49.90 | 50.58 | 61.20 | 38.79 |

Across the measured requests, total CPU is 165.08 seconds for baseline and
164.32 for candidate; the median paired CPU ratio is 1.0005. Total elapsed time
is 82.03 versus 81.84 seconds, with median paired ratio 0.9993. Overall runtime
is effectively unchanged in this small sample. Group 17047 uses about 1.4% more
total CPU, while its packed-pass median falls from 61.2 to 38.8 milliseconds.
These descriptive results do not establish an overall speed improvement.

No candidate search exhausts its traversal-work allowance in these runs. The
highest request consumption is 6,711,574 units, below the 22,200,000-unit cap;
the highest fixture-node count is 2,821, below 6,000. All native-stage admission,
setup-accounting and work-cap checks pass. The
[machine-readable results](results/2026-11-06-cohort-work-budget.json) retain
individual comparison gates, paired timings, bounds, hashes and provenance.

## Work-budget sensitivity and interruption check

The diagnostic probe runs all 12 previously undecided cells at each allowance,
retaining the same 1,000 fixture-node and 512-depth limits:

| Work allowance per cell | Verified witnesses | Completed refutations | Unknown |
|---|---:|---:|---:|
| 100,000 | 4 | 0 | 8 |
| 500,000 | 4 | 4 | 4 |
| 1,000,000 | 4 | 4 | 4 |
| 3,700,000 | 4 | 8 | 0 |

The witnesses need 28,824–33,109 units. The 31st-place refutations need
405,368–407,942 units, and the 30th-place refutations need
1,086,673–1,102,231. Thus the remaining failures under a one-million-unit budget
are genuine budget interruptions in this implementation.

The synthetic counterexample has a three-point target with no remaining games
and 62 zero-point rivals paired into 31 matches. Finishing 33rd would require
32 rivals to win, which is impossible. With a 6,400,000-unit allowance, the new
search returns unknown after exactly that
much work, with the exhaustion flag set, 212,855 recursive branch entries, 7,151
complete cohorts and 31 fixture nodes. It finishes within the enforced two-second
timeout. This preserves the work guard on an input whose uncapped baseline
traversal timed out.

Frozen binaries, hashes, commands, raw response/log exports, sensitivity results
and test logs are under
`/workspace/work/cohort-work-budget-17047-2026-11-06/`.

## Validation

All 591 distinct tests pass, with five existing ignored tests. The full command
was `cargo test --release --locked --offline --no-fail-fast --manifest-path odds-rust/Cargo.toml`.
That run passed 586 tests; five HTTP tests failed before starting their servers
because the default sandbox denied binding a localhost socket. Rerunning only
that target with local network access and `--test http -- --test-threads=1`
passed all five. Both transcripts are retained; there are no unresolved failures.

Focused checks cover zero work, denied cache comparisons at internal and leaf
nodes, completed refutation exactly at its work allowance, one-unit-short
interruption at both depth widths, and an exhaustive small-model comparison in
both orientations. A packed-search regression injects a one-unit cell allowance
and two-unit request allowance: only two primary attempts run, alternates cannot
reset the cell allowance, the third cell is not attempted, and existing labels
and numerical values remain intact.

Formatting and whitespace checks pass. A separate read-only review found no
correctness issue in pruning, interruption propagation, legacy behavior or
shared work accounting.
