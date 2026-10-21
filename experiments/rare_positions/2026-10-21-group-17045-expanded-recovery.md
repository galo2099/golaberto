# Group 17045 after expanding rare-position recovery

This follows the original investigation in
`2026-10-20-group-17045-estimation-gaps.md`. The request fixture remains
byte-identical to the upload, including its `pt,gd,gf,g_away,w,name` order.

The [extended remaining-cell analysis](2026-10-21-group-17045-remaining-cells.md)
adds ten seeds. It identifies persistent sampling and allocation gaps plus a
small initial final-sampling budget overrun at seed 817 that the five runs
below did not exercise.

## Implementation

Selected-team, ambiguous-team, and exception masks now use `u64` throughout
lazy proposals, complete strata, tree refinement, path guidance, and sampling
density evaluation. Their capacity is 64 teams; the full-width mask avoids
shifting by 64, and requests above that capacity decline before team shifts.
Outcome masks and packed outcome sequences retain their existing types.

The target-limit and native order-cohort gates also admit up to 64 teams.
Their proof node, root enumeration, guide, leaf, sampling, and work limits are
unchanged. Constructors can still decline when those bounds are exhausted.

Modeled leagues above 32 teams also use a bounded native training path. Each
cell receives its share of the existing training capacity before construction;
constructor charges, failed attempts, and capped pilot draws all consume that
share. Unbounded competing Union fits and adaptive retries are bypassed on
this path. Smaller leagues retain their previous training behavior. Late-tree
final pairs use the conservative branch draw-work bound rather than relying
only on pilot averages when admitting larger leagues.

Exact-cap recovery separately supports goal-difference-first requests. It
packs only points with stride one and no win bonus for `pt,gd,gf,...`; the
existing wins-second mode retains its points/wins packing. Consequently,
equal points with different wins never become a strict rank certificate in
the new mode. Conditional scores still pass through the production sorter,
including away goals and later wins. Unsupported analytic score integration
remains disabled. The legacy wins-second admission path is preserved.

Exact caps retain their all-target-wins necessity proof, unique-exception,
graph, memory, and work requirements. This particular fixture still has no
eligible exact-cap cell. Its recovered cells come from the newly admitted
lazy, tree, and order-cohort proposals.

## Independent-seed comparison

The before executable was saved from the rebuilt source before these changes.
Both versions used default settings, 20,000 importance-scout draws, and four
workers. Positions below are one-based. Each response has 1,296 cells.

| Seed | Missing before | Missing after | Newly estimated |
| --- | ---: | ---: | ---: |
| 808 | 14 | 2 | 12 |
| 809 | 12 | 1 | 11 |
| 810 | 12 | 2 | 10 |
| 811 | 18 | 3 | 15 |
| 812 | 12 | 0 | 12 |

Every previously positive cell remains positive, every public reachability
status remains `reachable`, and the probability matrices satisfy row and
column normalization. Remaining cells are:

- Seeds 808 and 810: team 855 at 35; team 1455 at 1.
- Seed 809: team 855 at 35.
- Seed 811: team 855 at 35; team 1455 at 1–2.

The two especially small newly recovered cells are positive in all five runs:

| Cell | Probability range | Relative standard error range |
| --- | --- | --- |
| Team 36, position 36 | 1.14e-22–4.15e-22 | 0.21–0.37 |
| Team 855, position 36 | 1.19e-24–1.63e-24 | 0.25–0.35 |

The existing publication gates still require independent main/check streams,
enough event hits and effective samples, bounded weight concentration, and
agreement. Main-only results cannot publish. These are approximate rare-event
estimates, and the existing precision flag is retained: a published coarse
estimate can still have `meets_precision_goal: false`. Some other tail cells
vary more between seeds or remain unestimated. Existing weak positive
estimates are not upgraded merely because the masks now support this league.

## Cost evidence

The audit initially exposed two existing admission weaknesses now exercised by
the larger league: parallel setup could exceed the initial stage allocation,
and a late-tree main/check pair could exceed a grant based on pilot averages.
The fixed training quotas and conservative late-tree draw forecast address
those cases without increasing any stage capacity.

All five final runs were checked for stage overruns and matrix reconciliation.
Initial training charged 276,047,701–400,047,898 units, and training plus actual
native final operations used 610,800,004–658,064,945 units against the unchanged
674,010,000-unit capacity. Native final reservations also fit that capacity.
The expected-cohort stage used 620,916,416–642,540,774 units against its separate
674,010,000-unit capacity. Late complete-tree recovery used
43,111,662–191,823,910 units; each run stayed within the actual remaining
confirmation bank, with exact before-minus-work accounting. Confirmation
reservations stayed within their bank limits. Exact-cap scanning used 2,799,360
units against a 748,900,000-unit capacity and admitted no plans.

The newly supported paths consume more of the existing work allocation.
A separate benchmark used seed 808, four workers, identical default settings
with logging disabled, one warmup per binary, and five alternating before/after
pairs without concurrent compilation. Median wall time increased from 2.03 to
4.09 seconds (2.02 times as long, about 2.06 seconds extra). The observed ranges
were 1.90–2.53 seconds before and 3.85–4.32 seconds after. Median recovery search
time increased from 1.74 to 3.85 seconds; setup, scout, and pool costs were
similar. This measures the 36-team fixture, not general service performance.
Logical work limits, rather than timing thresholds, are enforced by the
regression tests.

### Other requests

Eight other saved requests were compared with seed 808, four workers, logging
disabled, one warmup per binary, and three alternating before/after pairs per
request. Runs were sequential with no concurrent compilation. Median wall
times include CLI startup and JSON output.

| Fixture | Teams | Before | After | Change |
| --- | ---: | ---: | ---: | ---: |
| 17064 | 4 | 17.5 ms | 17.9 ms | +2.6% |
| 17058 | 4 | 46.5 ms | 51.1 ms | +10.0% |
| 16498, October 18 | 20 | 2.401 s | 2.422 s | +0.9% |
| 16653, October 11 | 20 | 2.768 s | 2.948 s | +6.5% |
| Synthetic tight | 20 | 0.223 s | 0.239 s | +7.1% |
| Synthetic dominant | 20 | 2.929 s | 2.972 s | +1.5% |
| Synthetic blockers | 20 | 1.271 s | 1.223 s | -3.8% |
| 16687, October 12 | 218 | 5.645 s | 5.757 s | +2.0% |

The seven smaller requests returned identical complete responses. The
218-team request returned identical probabilities, importance, and estimate
quality fields; only its reported `work_spent` changed. These timings do not
show a general doubling of request cost. Small differences include runtime
noise and are not statistically established regressions. Other requests in the
newly supported 33–64-team range, or goal-difference-first requests that newly
qualify for exact-cap recovery, can perform additional recovery work; this
sample does not establish their cost.

## Validation

Behavioral tests exhaustively enumerate a padded small league at 36 and
64 teams, verify disjoint proposal membership and raw mass, and compare
sampling with independent enumeration. Additional tests reject 65 teams,
exercise points-only exact-cap masses and strict floors, preserve existing
wins-second behavior, and test the actual away-goal/wins tiebreak sequence.
The group fixture regression checks independent seeds, accepted stream quality,
budget accounting, and identical seed-808 responses with one/four workers and
logging off/on.

Rebuilt before/after responses also match exactly for the four-team
`group-17064.json` and twenty-team `group-16498-2026-10-18.json` fixtures.

The full release workspace suite passed: 543 tests passed and five were ignored
by their existing test annotations. The command was
`cargo test --release --locked --manifest-path odds-rust/Cargo.toml`.
HTTP integration tests required local networking in the execution sandbox;
their initial port-binding restriction was resolved before the successful
full-suite run. `git diff --check` also passed.
