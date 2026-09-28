# Avaí second- and third-place reachability in group 16653

This experiment uses the saved group 16653, phase 4489 request captured on
2026-09-27. Team 68 is Avaí. The current matched-point-pool estimator has
a positive first- and fourth-place estimate but leaves second and third
at zero with `undecided` reachability. These observations concern this
snapshot; new results or fixtures require a new witness and estimate.

## Constructive reachability proof

The [self-contained outcome fixture](2026-09-27-group-16653-team-68-top3-witness.json)
specifies the played scores and one positive-probability outcome for each
of the 85 remaining fixtures. Every unplayed match is scored 1–0, 0–0, or
0–1, and all its Poisson scoring powers are positive. Avaí wins all nine
remaining matches and ends on 57 points. The Go standings sorter verifies:

| Changed outcomes from the base witness | First | Second | Third | Avaí's rank |
| --- | --- | --- | --- | ---: |
| None | Avaí | team 2064 | team 588 | 1 |
| Game 364721, team 70 v 96: draw → home win | team 70 (58 points) | Avaí (57) | team 2064 | 2 |
| Game 364721 and game 364727, team 22 v 80: draw → home win | team 70 (58) | team 22 (58) | Avaí (57) | 3 |

The second modification is cumulative: both games change for third
place. Neither game involves Avaí. All other scores remain as in the
base witness. The regression `TestGroup16653Team68TopThreeWitnesses`
replays the full standings with the actual point, win, goal-difference,
goals-for, and head-to-head sort order. Thus second and third are
reachable; zeros in the odds matrix are search misses.

## What the current search knows

At seed 808 with the matched-point pool enabled:

| Rank | Reported probability | Reachability | Last conditional search | 95% zero-hit upper limit |
| --- | ---: | --- | --- | ---: |
| 1 | 3.2566e-19 | witness | weighted lookahead | — |
| 2 | 0 | undecided | 0 / 50,000, point-event mass 5.8164e-8 | 3.4848e-12 |
| 3 | 0 | undecided | 0 / 1,000, point-event mass 0.014466 | 2.9957e-5 |
| 4 | 5.7297e-9 | pooled estimate | — | — |

The second-place upper limit is valid for its necessary points event.
The third-place limit is too loose to be useful. These limits do not
convert adjacent ranks into point estimates: finishing second and
finishing third can have very different sets of fixture outcomes.

## Directed sampling experiments

The rank-general lookahead was tested with additional draws, separate
from the production request. Conditioning on the points event used for
the first-place search and directing the proposal to ranks two or three
gave these *partial* probabilities:

| Event and rank | Weighted hits / draws | Estimated event-and-rank probability | Standard error |
| --- | ---: | ---: | ---: |
| First-place points event, rank 2 | 1,323 / 100,000 | 4.7532e-15 | 1.47e-16 |
| First-place points event, rank 3 | 3,619 / 100,000 | 2.0824e-13 | 3.58e-15 |

These are contributions from one event, **not total rank odds**. The
event is a subset of the possible second- and third-place seasons.

We also conditioned only on Avaí finishing with exactly 57 points,
which has probability 6.7763e-5 under the match outcome model:

| Event and rank | Weighted hits / draws | Estimated event-and-rank probability | Standard error |
| --- | ---: | ---: | ---: |
| Avaí 57 points, rank 2 | 2 / 200,000 | 1.4954e-14 | 1.08e-14 |
| Avaí 57 points, rank 3 | 45 / 200,000 | 3.9931e-11 | 6.00e-12 |

The rank-three 57-point contribution is roughly 190 times the
first-place-event contribution. This directly rules out treating the
latter as the complete third-place probability. The rank-two
57-point result is too noisy for a useful total estimate. No second-
or third-place probability is inferred from first and fourth alone.

## Recommendation

Keep the constructive witnesses as reachability evidence. For an odds
estimate, partition by Avaí's attainable final points and direct the
rank proposal within each stratum. Combine disjoint strata to avoid
double counting; use the current necessary-event searches to bound
unresolved strata. Allocate the existing request budget by expected
reduction in uncertainty. Simply enabling the million-draw deep flag
or increasing the ordinary conditional draw count is unlikely to help
second place efficiently: the unconditioned rank-two guided run found
only one finish in 100,000 draws, and the ordinary 50,000-draw run
found none.

This experiment changes reachability proof and documentation only.
The production estimator still reports zero for Avaí's second and
third places until a complete estimator for those cells is integrated.
