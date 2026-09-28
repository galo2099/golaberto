# Reusing simulated seasons for reachability proofs

## Question

Can a bounded search reuse the full seasons already found by conditioned sampling to settle nearby zero-probability cells, such as Avaí finishing 2nd or 3rd in group 16653, without materially increasing request time?

## Method

The conditioned samplers retain the outcome vector of their first successful season. After the existing probability calculations, a new general-purpose search starts from each distinct vector and tests the original season, every single-fixture change, and two-fixture changes. It spreads a default 10,000-candidate budget across all starting seasons. The first pass uses final points and wins to exclude ranks a candidate cannot reach. A rank settled by that prefix is a proof; ambiguous ties are checked with the full standings sorter using positive-probability scorelines. The search makes no position-specific assumptions.

New proofs set `Reachability` to `reachable_by_construction`. They do **not** create a probability estimate: `Probability` stays zero, and the existing 95% upper bound remains available. Point-impossible cells remain classified as such. `RARE_POSITION_NEIGHBORHOOD_SEARCH=0` disables the search; `RARE_POSITION_NEIGHBORHOOD_BUDGET` can set a positive budget up to 250,000 candidates (default 10,000).

## Results

For the saved phase 4489 / group 16653 request, seed 808, matched point pool, lookahead on, and four cores, 26 cells were undecided after the existing searches. The neighborhood search used ten distinct successful seasons and 10,000 candidate seasons. It proved three additional cells reachable:

| Team | Finish | Result |
| --- | ---: | --- |
| Avaí (68) | 2nd | Reachable by construction |
| Avaí (68) | 3rd | Reachable by construction |
| Team 11 | 19th | Reachable by construction |

The remaining 23 cells are still undecided. The algorithm has not proved them impossible. The Avaí results agree with the independently saved one- and two-fixture witnesses in `2026-09-27-group-16653-team-68-top3-witness.json`.

Full-request Go benchmark on an Apple M2 Pro with `GOMAXPROCS=4`, five measured iterations at the chosen 10,000-candidate budget: 957.1 ms/request with the search off and 964.2 ms/request with it on, an increase of 7.1 ms (0.7%). A separate five-run comparison at 60,000 candidates measured 929.9 ms off and 957.4 ms on, a 27.6 ms (3.0%) increase. Run-to-run variation in the baseline is visible in these measurements. Budgets of 60,000, 120,000, and 240,000 candidates found no additional proof. The 240,000 budget exhausted all 144,510 distinct one- and two-fixture candidates from those seasons. A separate rank-directed beam experiment examined about 24,000 candidates and found no extra proof; it was removed.

Group 16982 had no zero cells under this configuration, so the new search had no candidates. The existing real-group regression tests passed for groups 16498, 16982, 16983, and 16986. The full Go test suite passed.

## Limits and next useful step

The search proves existence only when a nearby concrete season is found. It cannot certify impossibility beyond the existing point bounds and does not supply odds for the newly reachable cells. Resolving the other 23 cells would require a deeper constructive search or a bounded exact constraint solver; both need a separate runtime and proof-quality comparison before use on the request path.
