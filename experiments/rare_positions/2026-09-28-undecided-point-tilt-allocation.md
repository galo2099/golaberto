# Spending a small point-tilt budget on undecided zero cells

## Decision

Extend the existing point-tilt pass to cells whose reachability is still
undecided. Keep the four confirmation slots for constructed-reachable
cells and allow two additional, independently confirmed undecided cells.
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

## Paired 20-seed results

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

## Work and limits

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
latency guarantee. Groups with many eligible zeros can use the full
twelve-cell pilot and six-cell final allowance. Some newly estimated
cells have low but accepted ESS and should be read as order-of-magnitude
estimates.

The full Go test suite passed. Optional real-request regressions passed
for groups 16653 and 16498, and the group 16653 regression passed with
the Go race detector. A three-team test confirms that an undecided cell
can receive a full-cell estimate and that both opt-out flags prevent
that work.
