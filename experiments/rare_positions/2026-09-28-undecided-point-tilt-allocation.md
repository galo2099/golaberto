# Spending a small point-tilt budget on undecided zero cells

## Initial decision

Extend the existing point-tilt pass to cells whose reachability is still
undecided. Keep the four confirmation slots for constructed-reachable
cells and initially allow two additional, independently confirmed undecided cells.
The ordinary conditional search and its four 50,000-draw extra batches
remain in place. The extension is on by default in matched-point-pool
mode; `RARE_POSITION_POINT_TILT_UNDECIDED=0` restores the previous
selection, and `RARE_POSITION_CONDITIONED_POINT_TILT=0` disables the
whole point-tilt pass.

This is a bounded extension of the current allocator, not a complete
replacement of all proposal-selection code. Pilot and final draws are
separate seeded streams. An undecided cell needs no constructed witness
to enter a pilot, but receives `witness` status only after a weighted
final sample actually finishes at that rank. The target-only point
event and terminal/fixture likelihood ratios still cover the full
rank cell.

## Experiments rejected

Moving joint point-cap and constructive witness searches before
conditional sampling produced effectively identical estimates across
five paired seeds of groups 16653 and 16498. It saved no meaningful
request time or gained coverage, so the production order stays as it
was. The neighborhood proof search still uses sampled scenarios.

A matched-draw trial removed one 50,000-draw conditional extra batch to
pay for the new pilots and confirmations. It found more positive cells,
but loosened some still-zero cells' reported 95% upper bounds by about
50 times. That trade-off was rejected. The chosen version retains the
extra batch and measures its added wall time directly.

## Initial paired 20-seed results

The inputs were the five saved requests in
`/tmp/golaberto-real-fixtures`, each checked against its independent
5-million-season reference file in `/tmp/golaberto-real-reference`.
Master seeds were 808–827, `GOMAXPROCS=4`, on an Apple M2 Pro. Each
pair used the same input and seed, with only
`RARE_POSITION_POINT_TILT_UNDECIDED` changed. Zero-count reference
cells were treated as unresolved, not impossible.

| Group | Extra nonzero cell-runs across 20 pairs | Pairs with a gain | Lost baseline nonzero cell-runs | Mean request time before → after | Time increase |
| --- | ---: | ---: | ---: | ---: | ---: |
| 16653 | 31 | 18 | 0 | 1.062 → 1.122 s | 5.7% |
| 16498 | 9 | 7 | 0 | 1.059 → 1.170 s | 10.5% |
| 16982 | 0 | 0 | 0 | 0.927 → 0.923 s | within noise |
| 16983 | 0 | 0 | 0 | 0.929 → 0.931 s | within noise |
| 16986 | 0 | 0 | 0 | 0.977 → 0.980 s | within noise |

The last three requests had no reachable or undecided zero cells in
these runs, so the new pilot loop did no work. Across reference cells
with at least one observed finish, whole-matrix, common-cell, and
rare-cell RMSE changes were below `3e-13` in the affected groups.
The new cells had zero occurrences in the 5M references; those files
cannot check probabilities around `1e-9` to `1e-12`.

At seed 808, the additional 15,000-draw estimates were:

| Group | Team | Rank | Estimate | Confirmation ESS | Two independent 300,000-draw estimates |
| --- | ---: | ---: | ---: | ---: | ---: |
| 16653 | 12 | 17th | `5.58e-10` | 70.8 | `5.00e-10`, `5.25e-10` |
| 16653 | 95 | 5th | `1.90e-9` | 38.8 | `1.53e-9`, `1.47e-9` |
| 16498 | 8 | 18th | `4.17e-12` | 8.51 | `3.35e-12`, `2.76e-12` |
| 16498 | 74 | 19th | `7.51e-12` | 17.9 | `4.18e-12`, `3.50e-12` |

The longer runs had ESS between 133 and 1,415, depending on the cell.
All four short estimates were within about a factor of 2.2 of their
longer checks. Those checks used the same full-cell proposal family but
independent seeds, so they test sampling stability rather than provide
an external exact truth.

## Initial work and limits

The pass now logs attempted and built point events, pilot and final
draws, accepted estimates, work units, and elapsed time. At seed 808:

| Group | Before pilot/final draws | After pilot/final draws | Pass time before/after |
| --- | ---: | ---: | ---: |
| 16653 | 24,000 / 60,000 | 36,000 / 90,000 | 144 / 221 ms |
| 16498 | 9,000 / 30,000 | 36,000 / 60,000 | 153 / 269 ms |

The total pilot queue is capped at twelve cells. Constructed-reachable
cells enter first; remaining slots go to undecided zeros by their
existing zero-hit upper bound. Proposal choice uses pilot ESS, with a
gentler tilt preferred when pilot evidence is too sparse to compare
tails. Up to four proved and two undecided cells receive fresh final
draws. The existing acceptance checks, precision metadata, and matrix
reconciliation are unchanged. Zero-hit upper bounds were unchanged to
floating-point precision in the final paired runs for cells that
remained zero.

The extra time is measured on these reference requests, not a universal
latency guarantee. In this initial two-slot experiment, groups with many
eligible zeros could use the full twelve-cell pilot and six-cell final
allowance. Some newly estimated
cells have low but accepted ESS and should be read as order-of-magnitude
estimates.

The full Go test suite passed. Optional real-request regressions passed
for groups 16653 and 16498, and the group 16653 regression passed with
the Go race detector. A three-team test confirms that an undecided cell
can receive a full-cell estimate and that both opt-out flags prevent
that work.

## Follow-up: Criciúma 18th place disappeared between runs

The saved live request for group 16653 at
`/tmp/group16653-live-20260927.json` has a different fixture and power
snapshot from the reference request above. Its Criciúma cell is team 73,
rank 18. With seed 816, both the old allocation and the new two-slot
allocation report `1.95e-10` from the matched point pool. This has no
direct season hits and a `1.39e-10` standard error (72% relative error).
With seed 817, **both** allocations report zero. Thus changing the
allocation did not remove a nonzero estimate; the separate runs used
different seeds. The default seed is based on the current time unless
`RARE_POSITION_RANDOM_SEED` is set.

At seed 817, the targeted point-tilt pass did pilot Criciúma 18th and
recorded 121 hits with pilot ESS 24.2. It ranked third among undecided
candidates, behind team 95 rank 7 and team 588 rank 18, so the two
confirmation slots excluded it. We increased only the undecided final
allowance from two to three; the four constructed-reachable slots and
twelve-cell pilot cap stay the same. Criciúma then receives an independent
15,000-draw confirmation at seed 817 and is estimated at `1.83e-9`
(ESS 365). Seed 815 independently estimates `1.65e-9` (ESS 322).

Paired 20-seed comparisons of two versus three final slots, using the
same request and seed on each side with four cores:

| Request | Extra positive cell-runs | Lost positive cell-runs | Criciúma 18th positive runs | Mean request time, two → three slots |
| --- | ---: | ---: | ---: | ---: |
| Live group 16653 | 20 | 0 | 16 → 20 of 20 | 1.111 → 1.122 s (+1.1%) |
| Reference group 16653 | 7 | 0 | 20 → 20 of 20 | 1.099 → 1.104 s (+0.5%) |
| Reference group 16498 | 0 | 0 | not applicable | 1.103 → 1.124 s (+1.9%) |

The new confirmation is a bounded improvement in coverage, not a
guarantee that every future seed or updated fixture snapshot will have
a positive estimate. Paired same-seed tests separate this allocation
change from ordinary Monte Carlo variation.

## Follow-up: Avaí second place fails the final-sample quality gate

A live group 16653 run with time seed `1790644074581708000` proved
Avaí (team 68) could finish second but published zero. The saved live
request above has the same simulation inputs as the database request at
the time of this investigation. The ordinary conditional search found
no rank hits. The point-tilt pass selected Avaí for confirmation using
the gentle `(rank tilt 3, point tilt 0.5)` proposal. Its 1,000-draw pilot
had four hits and ESS 2.59. The 15,000-draw confirmation found 47 hits
and estimated `9.91e-15`, but its largest hit carried 28.7% of the total
weight, exceeding the 25% publication limit. Keeping zero was the
correct action under that quality rule.

We now use 30,000 confirmation draws for any constructed-reachable
candidate whose selected pilot has at least three hits but ESS below
ten. This uses the same unbiased proposal and acceptance gates; it
allocates more samples only where the pilot suggests a usable but noisy
proposal. With the reported seed, Avaí second had 98 final hits, ESS
24.4, a largest weight share of 12.4%, and an accepted probability
`1.15e-14`. The result remained positive after matrix reconciliation.

Paired comparisons of 15,000 versus adaptive confirmation draws used
the reported seed plus seeds 808–827, with four Go cores:

| Request | Extra positive cell-runs | Lost positive cell-runs | Avaí second positive runs | Mean request time before → after |
| --- | ---: | ---: | ---: | ---: |
| Live group 16653 | 1 | 0 | 19 → 20 of 21 | 1.138 → 1.171 s (+2.9%) |
| Reference group 16653 | 3 | 0 | 16 → 17 of 21 | 1.125 → 1.169 s (+3.9%) |
| Reference group 16498 | 8 | 0 | not applicable | 1.158 → 1.180 s (+1.9%) |

The rule does not guarantee a positive result for every seed. Candidates
can still miss the pilot, receive no final allocation, or fail the
independent quality check. The fixed random seed in the regression test
replays this particular failure and verifies its recovery.

## Follow-up: prioritize proven gaps between estimated ranks

A zero between two positive ranks is a useful search cue, but it does
not prove that the middle rank is reachable. Across 41 seeds per saved
request, the live group 16653 request had five gap cell-runs for Avaí
across ranks 2–4. The earlier group 16653 request had ten gap cell-runs
across five cells. In group 16498, team 110 rank 4 was a gap in 40 of
41 runs, yet its reachability remained undecided. That case should not
be filled merely by interpolating its neighbors.

The new bounded pass considers only a still-zero cell with a
constructed reachability proof, positive estimates on both neighboring
ranks, and an existing point-tilt pilot candidate. It runs after the
ordinary point-tilt confirmations, so a neighbor found in that same
pass can establish the gap. At most one gap per request receives a
fresh 60,000-draw full-cell confirmation. It uses the gentle rank tilt
3 and point tilt ±0.5, with exact importance weights and the unchanged
publication gates. The pilot's strongest tilt was unsuitable here:
despite passing the gates, it produced estimates far below gentler
independent runs because rare high-weight tails were poorly sampled.

Paired four-core measurements with the gap pass off versus on used the
reported time seed plus seeds 808–827 (808–817 for group 16498):

| Request | Additional positive cell-runs | Lost positive cell-runs | Remaining gaps | Mean time before → after |
| --- | ---: | ---: | ---: | ---: |
| Live group 16653 | 1 | 0 | 2 → 1 | 1.212 → 1.220 s (+0.6%) |
| Reference group 16653 | 3 | 0 | 7 → 4 | 1.177 → 1.217 s (+3.4%) |
| Reference group 16498 | 0 | 0 | 11 → 11 | 1.203 → 1.195 s (timing noise) |

At live seed 816, the pass estimated Avaí second at `1.89e-14` from
246 weighted rank hits (ESS 67.7). Five independent confirmation
streams for that same cell ranged from `1.43e-14` to `1.98e-14`.
On the older group 16653 request it also filled Avaí third and
Botafogo-SP second. Five independent streams for those cells ranged
from `9.42e-11` to `1.20e-10` and from `7.94e-10` to `9.97e-10`,
respectively. These checks show proposal stability, not exact truth;
the 5-million-season references have too few observations at this
scale. `RARE_POSITION_POINT_TILT_GAP_RESCUE=0` disables the extra pass.

## Follow-up: sample gaps whose reachability is undecided

The group 16498 gap for team 110 rank 4 remained undecided under the
constructive solver, but that status only meant the proof budget ended.
Direct point-conditioned importance samples found rank-4 finishes in
three independent 60,000-draw moderate-tilt runs. Their estimates were
`1.45e-12`, `1.63e-12`, and `1.70e-12`, each with effective sample size
above 400. Thus the gap is reachable, and leaving it at zero missed an
event that the weighted sampler can measure.

The new pass checks at most one undecided, still-zero gap per request.
It selects a gap with positive immediate neighbors and the largest
existing zero-hit upper bound. It constructs the full target-point event,
then runs independent 1,000-draw pilots at gentle and moderate rank
tilts. A moderate proposal runs first only when the gentle pilot gets
at most two hits and the moderate pilot gets at least 25 hits and ESS at
least three. A fresh 15,000-draw confirmation supplies the estimate;
the other proposal gets one fresh 15,000-draw fallback if needed.
Both proposals retain exact likelihood correction. The moderate
confirmation has stricter weight-concentration and ESS gates because
the aggressive proposal was unstable for Avaí rank 4. A sampled rank
hit is required before reachability becomes `witness`; neighboring
positive ranks alone never establish it.

Paired four-core requests used the reported time seed plus seeds
808–827, with this pass off and on for each saved request:

| Request | Extra positive cell-runs | Lost positive cell-runs | Main gap, before → after | Mean request time before → after |
| --- | ---: | ---: | ---: | ---: |
| Group 16498 reference | 21 | 0 | Team 110 rank 4: 0 → 21 of 21 | 1.194 → 1.266 s (+6.0%) |
| Group 16653 live | 1 | 0 | Avaí rank 4: 20 → 21 of 21 | 1.171 → 1.163 s (timing noise) |
| Group 16653 reference | 3 | 0 | Team 22 rank 17 twice; team 95 rank 5 once | 1.120 → 1.138 s (+1.6%) |

Team 110's 21 accepted estimates ranged from `1.36e-12` to `2.04e-12`.
Independent 60,000-draw checks for Avaí rank 4 with the gentle proposal
gave `5.21e-9`, `5.81e-9`, and `6.49e-9`; the live paired gain was
`5.11e-9`. Independent moderate checks for team 22 rank 17 gave
`4.47e-12` to `5.18e-12`; the paired gains were `5.11e-12` and
`5.20e-12`. These are proposal-stability checks, not exact truth.

The pass is enabled by default when the undecided point-tilt search is
enabled. `RARE_POSITION_POINT_TILT_UNDECIDED_GAP=0` disables only this
extra gap check. Its maximum additional sample budget is 2,000 pilot
draws plus 30,000 confirmation draws for one cell. The measured cost is
below the requested 20% limit on these three requests; other fixture
sets may cost more when both confirmations are needed.

## Follow-up: share the proved-gap budget

Two remaining gaps showed that the single-cell proved-gap pass could
spend its 60,000 draws on one cell while skipping another, or skip a
proved gap entirely because it missed the ordinary point-tilt final
shortlist. On the older group 16653 request at seed 816, team 457 rank
2 received the 60,000-draw rescue; Avaí rank 2 remained zero. On group
16498 at seed 825, team 74 rank 19 had a proof and positive neighbors
but no gap rescue. Its ordinary conditional search used 1,000 draws.

Direct proposal checks gave a reason to split the budget. Three
independent 15,000-draw gentle-tilt estimates for both team 457 and
Avaí rank 2 passed the publication gate. Team 74 rank 19 needed the
moderate tilt: two of three 15,000-draw checks passed, and three of
three 60,000-draw checks passed with estimates `3.21e-12` to
`3.86e-12`. The moderate proposal is selected only if a short gentle
pilot has at most two hits and the moderate pilot has at least ten hits
and ESS at least 1.5. Pilot draws do not enter the published estimate.

The revised pass scans all proved, still-zero gaps after the ordinary
point-tilt confirmations. It orders them by their existing zero-hit
upper bound and considers at most two. Each cell gets two independent
1,000-draw pilots and a fresh 15,000-draw confirmation, with one fresh
15,000-draw alternative if the first fails. All draws count against the
same 60,000-draw cap used by the earlier one-cell pass. The estimator
still needs an accepted weighted sample to assign a positive value.

Paired full requests used four cores, the reported time seed plus
seeds 808–827 for the three affected snapshots, and five seeds for
each other reference group. Each pair differed only in the proved-gap
allocator; the matched-point-pool, ordinary conditional, undecided,
and reconciliation passes were otherwise identical.

| Saved request | Paired runs | Extra positive cell-runs | Lost positive cell-runs | Zero-hit upper bounds worsened | Remaining gaps before → after | Mean time before → after |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Live group 16653 | 21 | 0 | 0 | 0 | 0 → 0 | 1.246 → 1.245 s |
| Reference group 16653 | 21 | 1 | 0 | 0 | 1 → 0 | 1.273 → 1.263 s |
| Reference group 16498 | 21 | 1 | 0 | 0 | 1 → 0 | 1.357 → 1.374 s (+1.2%) |
| Reference group 16982 | 5 | 0 | 0 | 0 | 0 → 0 | 1.193 → 1.181 s |
| Reference group 16983 | 5 | 0 | 0 | 0 | 0 → 0 | 1.169 → 1.168 s |
| Reference group 16986 | 5 | 0 | 0 | 0 | 0 → 0 | 1.219 → 1.224 s |

At seed 816 the new Avaí rank-2 estimate was `1.51e-13` (ESS 22.1),
while team 457 rank 2 retained a positive estimate. At seed 825 team
74 rank 19 was estimated at `2.81e-12` (ESS 8.69). The independent
longer runs support their order of magnitude, but do not establish
exact probabilities. No upper bound grew on a still-zero cell in the
paired runs. The measured time differences are small enough that only
the +1.2% group 16498 result should be treated as a cost signal;
slightly negative differences are timing noise.
