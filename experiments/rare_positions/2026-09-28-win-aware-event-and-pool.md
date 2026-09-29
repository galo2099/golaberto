# Point-and-win conditioning and matched-pool experiment

## Candidate designs

For phases sorted `pt,w,...`, I tested two independent changes to the
production-sized request. Both used flags during the experiment and were
removed after evaluation.

1. **Conditioned event:** encode each tracked team's `(additional points,
   additional wins)` jointly in the fixture-outcome DP. Compare tracked teams
   lexicographically at terminal states. The event remains a necessary condition
   for the target rank, and samples retain their exact conditional mass. If the
   DP exceeds its existing state limit, fall back to the points event.
2. **Matched pool:** record `(additional points, additional wins, rank)` in the
   existing 100,000-season scout. Compute the target's exact joint point/win PMF
   from its remaining W/D/L fixtures. Pool teams that actually finished with
   the same final point and win totals, requiring at least 20 observations for
   a joint state. Blend this estimate 50/50 with the existing point pool and
   use the point pool for unobserved joint states. This retains the original
   matrix's nonzero support.

I tested the event and pool separately, then together. Group 16653 and 16498
sort by points then wins. Group 16982 sorts by points then goal difference, so
the candidates correctly left its estimates unchanged in the tested seed.
The event full-request runs used the September 28 group 16653 snapshot with
85 unplayed games; the reference comparison used the earlier saved group
16653 request paired with its 5M-season reference. Group 16498 used its saved
reference request throughout.

## Correctness checks

On a small `pt,w,...` phase, exhaustive W/D/L enumeration confirmed that the
joint event contains every finish at the requested rank and that its mass
matches the enumerated outcome mass. The exact joint PMF's point marginal
matched the existing point PMF. Joint scout counts summed to the original
point and rank counts. The full Go test suite passed.

## Results

The joint event exceeded the existing 20,000-state cap for almost every
two-rival build in a diagnostic that used the first two other teams as
blockers. On group 16653, only **3 of 400** cell builds kept the
point-and-win event; the rest fell back to points. None of the three had a
smaller event mass. On group 16498, the point-only two-rival event itself fit
the cap for only 12 cells; just 2 of those retained wins, and neither narrowed
the mass. With one tracked rival chosen the same way, all 400 builds succeeded,
but wins narrowed only 4 cells in 16653 and 17 in 16498. Production selects
blockers by expected points, so these counts characterize the state-space
problem rather than the exact production fallback rate. Across seeds
808, 811, and 812, it found no new nonzero cells in either group; one 16498
cell was lost in a changed stochastic search. The loss is not a proof of
impossibility.

For the matched pool, I compared seeds 808, 811, and 812 against the saved
5-million-season references. The requests and references had matching SHA-256
hashes. Each group has 400 cells; the near-`1e-6` subset contains cells with
reference probability from `5e-7` to `2e-6`.

| Group | Near-`1e-6` cells | Baseline RMSE | Joint-pool RMSE | Nonzero-cell change | Approximate time change |
| --- | ---: | ---: | ---: | ---: | ---: |
| 16653 | 10 | `0.721e-6` | `1.266e-6` | none | +4% |
| 16498 | 11 | `0.859e-6` | `1.181e-6` | none | +6% |

RMSE combines the three seeds for each group. The 5M-season references have
appreciable sampling uncertainty at `1e-6`, so these figures are evidence
against this particular blend, not an exact measurement of its bias. Full
matrix RMSE changed little. The pool alone did not gain or lose a nonzero cell
in these six paired runs.

Combining the event and pool did not resolve the tradeoff. In group 16653 at
seed 808 it gained team 70 in 17th at about `2.97e-11` while losing team 95 in
5th at about `1.90e-9`, because the changed pilots allocated their limited
confirmation slots differently. It also cost more time.

## Decision

Do not enable or retain either candidate in the production path. The event's
joint state space is too large under the current cap, and the exact joint
matched pool fragments the 100,000-season scout without improving the tested
`1e-6` estimates. A narrower next experiment would use wins only to resolve
point ties after the point-based candidate selection, keeping the broad point
pool and its support intact.
