# Point-tilted estimates within the extra-time budget

## Goal

Spend at most about 20% more four-core request time to replace reachable
zero cells with rough order-of-magnitude probabilities. The previous
witness-guided experiment found several narrow subevent probabilities,
but those could not be used as full-cell odds because they omitted other
ways to finish at the same rank.

## Method

After the existing scout, conditioned searches, and reachability proofs,
the new pass visits zero cells proven reachable by construction. It
conditions only on the target team's **full feasible range** of final
point totals. It changes the distribution of those totals with an
exponential tilt and directs the remaining fixtures toward the requested
rank. Every feasible terminal point state retains at least 2% of its
original conditional proposal probability. The terminal and fixture
likelihood ratios correct both tilts, so a weighted hit estimates the
whole rank cell rather than a selected point slice.

Up to twelve proved cells, chosen by their existing zero-hit upper
bounds, each get three 1,000-draw pilots with generic tilt strengths.
Pilots select a proposal and at most four cells receive a separate
15,000-draw confirmation. A result is published only with at least 30
rank hits, effective sample size (ESS) at least 8, relative standard
error at most 35%, maximum event-weight share at most 25%, and a
half-batch gap at most 1. These deliberately loose gates suit an
order-of-magnitude estimate; the usual `MeetsPrecisionGoal` field is
still calculated separately. All cells that fail confirmation keep
their reachability proof and zero estimate.

The pass runs by default with `RARE_POSITION_MATCHED_POINT_POOL=1` and
can be disabled with `RARE_POSITION_CONDITIONED_POINT_TILT=0`.

## Results

Saved requests: group 16653 from 2026-09-27 and group 16498 from the
local real-fixtures set. Four Go cores, seed 808. Values are
probabilities, not percentages; post-reconciliation values are shown.

| Group | Team | Rank | Before | After | Confirmation ESS |
| --- | ---: | ---: | ---: | ---: | ---: |
| 16653 | 68 (Avaí) | 2nd | 0, reachable | `1.44e-14` | 15.9 |
| 16653 | 68 (Avaí) | 3rd | 0, reachable | `2.71e-11` | 46.9 |
| 16653 | 11 | 19th | 0, reachable | `1.88e-11` | 442 |
| 16498 | 20 | 2nd | 0, reachable | `2.39e-16` | 1,040 |
| 16498 | 110 | 3rd | 0, reachable | `1.14e-15` | 303 |

The other proven cells in these two groups, including Londrina 3rd,
team 125 10th, and team 318 3rd, still had insufficient full-cell
weighted hits. The sampler does not substitute the much smaller
witness-constrained subevent estimates for them.

Five independent group 16653 seeds (808–812) all produced a positive
Avaí 2nd estimate. Those estimates ranged from `8.10e-15` to
`1.73e-14`; Avaí 3rd ranged from `2.25e-11` to `3.29e-11`. These
ranges describe seed sensitivity in a small check, not a validated
confidence interval or external ground truth.

## Cost and checks

Paired `BenchmarkMatchedPointPoolFullRequest` runs, three requests per
configuration, `GOMAXPROCS=4`, Apple M2 Pro:

| Group | Pass disabled | Pass enabled | Increase |
| --- | ---: | ---: | ---: |
| 16653 | 0.945 s | 1.078 s | 14.1% |
| 16498 | 0.899 s | 1.012 s | 12.6% |

An earlier paired run measured 5.4% and 10.2%, respectively; the
three-request benchmark varies, but both runs stayed under 20%.

The full Go test suite passed. A three-team test checks that every
positive-mass terminal state keeps support, that its likelihood ratio
recovers the original distribution, and that the weighted rank estimate
agrees with independent direct conditioned sampling within sampling
error. Real-group regression tests check the new estimate metadata or
retain a reachability proof when confirmation fails.

These are measured costs for the two reference requests. The pilot
count is capped at twelve, but fixture counts differ, so this is not a
universal wall-time cap. Likewise, the new cells are rough
simulation estimates, not exact probabilities.
