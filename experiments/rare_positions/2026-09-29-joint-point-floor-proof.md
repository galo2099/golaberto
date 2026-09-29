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

The pass runs after probability estimation so it cannot redirect simulation
work or displace an existing nonzero estimate. It uses the existing 500-node
per-cell and 10,000-node per-request limits. It is enabled by default and can
be disabled with `RARE_POSITION_JOINT_POINT_FLOOR=0`.

## Results

Saved group 16498 request, 103 unplayed fixtures, four cores:

| Fixed seed | Undecided zeros before | After | Newly proved impossible |
| --- | ---: | ---: | --- |
| 808 | 25 | 23 | Team 17, 16th; team 16, 18th |
| 811 | 27 | 25 | Same two cells |

The isolated proof took about 0.49 ms over the 25 cells at seed 808, using
144 propagation nodes. Five-request full-request benchmarks gave 0.960 s with
the pass off and 0.953 s with it on (means of three runs); the difference is
within timing noise. Allocations rose by about 1,300 per request.

At seed 811, groups 16653 and 16982 retained the same nonzero cells and
reachability statuses. Group 16653 had no new floor proof; group 16982 had no
zero cells. Group 16498 also retained exactly the same nonzero cells and
reported simulation work at both seeds.

The full Go suite and race detector passed. An exhaustive small-schedule test
checks that the dual solver never rules out a rank attainable under any
points tie order. The saved group 16498 request has a regression test for the
two new proofs.
