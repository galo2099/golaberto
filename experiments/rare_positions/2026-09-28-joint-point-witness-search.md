# Complete fixture witnesses from joint point domains

## Goal and method

The joint point-cap pass can leave a set of locally consistent outcomes without proving that the target finishes at the requested rank. This experiment extends those domains to a **complete assignment of every unplayed fixture**, then checks the resulting rank with the same Go standings sorter used by the odds service.

For an unresolved team/rank cell, the search gives the target its maximum attainable points, allows enough other teams to exceed that total for the requested rank, and applies the existing point-cap propagation to the remaining teams. It branches on an unassigned fixture with the smallest domain and least point slack. Each branch is a home loss, draw, or home win with positive probability under that fixture's Poisson model. At a complete assignment, it builds canonical 0–1, 0–0, or 1–0 scores and calls `canonicalReachabilityRank`, which applies the full standings sort, including tiebreakers. A matching rank changes the cell to `reachable_by_construction`; a failed or exhausted search leaves it undecided. The odds estimate and its existing upper bound do not change.

The search has a limit of 100 propagation nodes per cell and 3,200 per request. It runs by default after the impossibility pass when points are the first standings criterion. `RARE_POSITION_JOINT_POINT_WITNESS=0` disables it. The search is general across teams and positions; it has no rank-specific targets.

## Group 16653: complete Londrina 3rd-place assignment

In the saved phase 4489 request, 85 fixtures are unplayed. An independent CSP construction supplies one outcome for **all 85**. This string follows the unplayed fixtures in the request's order; `0` means home loss, `1` draw, and `2` home win:

```text
0122010121210002112000222010000022020222202202110200100121210202022002010202002002112
```

The regression test checks that the string has exactly one digit per remaining fixture, that each selected outcome has positive probability, and that the Go standings sorter produces these ranks:

| Team | Verified finish |
| --- | ---: |
| Vila Nova (70) | 1st |
| Fortaleza (22) | 2nd |
| Londrina (95) | 3rd |
| Novorizontino (2064) | 4th |
| Operário (588) | 5th |

Londrina wins all eight of its remaining games and reaches 52 points. The fixture domains also accommodate Fortaleza above it while Novorizontino and Operário finish below it after tiebreakers. The full standings check is essential: point caps alone cannot establish the ordering of teams tied on 52 points.

The production search independently found a complete Londrina 3rd-place witness in 65–68 nodes. It also found team 125's 10th-place witness in 58 nodes. Both were previously undecided zero cells. Its own witness may differ from the explicit assignment above. It examined all 21 remaining undecided cells, used 2,023 nodes, and logged about 11–12 ms. A group 16498 run examined all 31 undecided cells, used 3,081 nodes, logged about 11 ms, and proved team 20's 2nd place, team 110's 3rd place, and team 318's 3rd place reachable.

## Cost and limits

On an Apple M2 Pro with four Go cores, a paired five-iteration full-request benchmark for group 16653 measured 961 ms with this pass disabled and 981 ms enabled. Other observed pairs varied by tens of milliseconds, so the direct pass logs are a more useful measure of its work. The measured full-request mean remains below one second, though individual requests can cross one second due to normal runtime variation.

The complete schedule is a **reachability proof**, not a probability estimate. Canonical scorelines may miss witnesses that need a different winning score or tiebreaker. Fixing the target to its maximum points and the bounded node count can also leave reachable cells undecided. The implementation never declares impossibility from a failed witness search.

## How long would “all cases” take?

There is no finite timeout that makes this particular search complete. It fixes the target at its maximum attainable points and only explores the resulting point-cap domain. A valid finish at a lower point total, or one requiring a different exemption order, remains outside that search regardless of the node budget.

As a measured sensitivity check, I reran every remaining zero cell independently with 100, 1,000, and 10,000 nodes per cell. Group 16653 had 19 cells remaining after the production pass; 190,000 extra nodes took about 2.4 seconds and found no additional witness. Group 16498 had 28 cells; 280,000 nodes took about 3.2 seconds and also found no additional witness. These times exclude the roughly one-second odds calculation itself. They show the cost of a larger bounded search, not a proof that the cells are impossible.

An exact reachability solver would have to enumerate target point totals, which teams are allowed above each target total, all surviving fixture outcomes, and the final tiebreaker order. With 85 fixtures, the unpruned outcome space is `3^85`, about `3.5e40` assignments. Point-cap propagation reduces that dramatically in typical cases, but the worst case is still exponential. A practical exact mode should therefore use a separate time budget and return `reachable`, `impossible`, or `undecided` when the budget expires; it should not be treated as a request-time operation with a fixed guarantee.

Validation: synthetic standings-sorter test, saved group 16653 and 16498 regression tests with seed 808, and the full Go test suite.
