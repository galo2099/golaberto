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
