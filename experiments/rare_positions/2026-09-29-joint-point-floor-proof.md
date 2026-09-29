# Minimum-points impossibility proof, 2026-09-29

## Question

Can a small sound proof pass classify some of the zero-probability cells that
remain undecided after the current point-cap and witness searches?

## Method

The existing point-cap solver asks whether at most `r` rivals can exceed the
target's maximum attainable points. The dual question is useful near the
bottom: a team finishing at zero-based rank `r` can have at most `n-1-r` rivals
strictly below it. Since the target finishes with at least its minimum
attainable points, at most that many rivals can finish below that minimum.

Negating current points and per-fixture point gains turns this into the same
joint cap problem. The solver then propagates shared fixture domains and may
prove that every allocation of below-target slots is inconsistent. Tied teams
are allowed in the relaxation, so a proof is sound regardless of later
tiebreakers. A rival is counted as *necessarily* below only when its **maximum
attainable final points** are below the target's minimum. Using its current
points here would be incorrect, because it can still gain points; an initial
trial exposed this error and its results were discarded.

The pass runs after the initial scout and hard point screen, but before
conditioned searches. Proven-impossible cells leave the targeted search list.
The broad 100,000-season matched pool still evaluates every position. The
proof uses the existing 500-node per-cell and 10,000-node per-request limits.
It is enabled by default and can be disabled with
`RARE_POSITION_JOINT_POINT_FLOOR=0`.

## Results

Saved group 16498 request, 103 unplayed fixtures, four cores:

| Fixed seed | Undecided zeros before | After | Newly proved impossible |
| --- | ---: | ---: | --- |
| 808 | 25 | 23 | Team 17, 16th; team 16, 18th |
| 811 | 27 | 25 | Same two cells |

The isolated proof took about 0.49 ms over the 25 cells at seed 808, using
144 propagation nodes. Moving it before targeted searches removes the two
cells' 2,000 conditioned draws and 200 guided pilot draws each. This saves
541,200 reported work units, equivalent to 4,400 plain seasons for this
request. Five-request full-request benchmarks gave 0.957 s with the pass off
and 0.914 s with the early pass on (means of three runs), a 4.5% reduction.
Allocated bytes fell from about 367 MB to 339 MB per request.

At seed 811, groups 16653 and 16982 retained the same nonzero cells and
reachability statuses. Group 16653 had no new floor proof; group 16982 had no
zero cells. Group 16498 retained exactly the same nonzero cells at seeds 801,
808, 811, 817, and 900. Its probabilities on common nonzero cells were
unchanged to floating-point precision.

The full Go suite and race detector passed. An exhaustive small-schedule test
checks that the dual solver never rules out a rank attainable under any
points tie order. The saved group 16498 request has a regression test for the
two new proofs, unchanged nonzero coverage, and reduced reported search work.
