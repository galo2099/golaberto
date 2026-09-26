# Pooling points and standings gaps from the initial MC scout

Date: 2026-09-25. The hypothesis was that each simulated final table can
inform the probability of a rank at nearby point totals, even when no team
finished with exactly those points in that season. A team-specific exact
additional-points distribution then weights this group-level points/rank curve.

## Method

For every one of 1,000 ordinary scout seasons, record the final points and
rank of all teams. At each integer point total in the feasible group range,
also move each team to that total against the *frozen* final table and record
its resulting rank. When a queried total lies in a standings gap, the team's
rank remains constant across that gap. Hypothetical ties use the original
sorted order, so the resulting curve is a model-based proxy, not an exact
conditional probability. One simulated season remains one independent season;
the additional placements do not create independent evidence.

For target team `t`, exact dynamic programming supplies `P_t(S=s)` for added
points `s`. The model estimate for rank `r` is

```text
sum_s P_t(S=s) * group_rank_probability(r | current_points_t + s)
```

The tested diagnostic uses the group's *actually observed* points/rank counts
where available and the frozen-table gap curve only when a point total had no
actual observation. This avoids treating every hypothetical placement as a
new independent data point. The diagnostic is clamped by the exact points
upper bound and set to zero for proven-impossible cells. It does not replace
the fresh-production probability or standard error.

Set `RARE_POSITION_POINT_GAP=1` to collect the diagnostic and charge its
analysis work. Also set `RARE_POSITION_POINT_GAP_USE_PRIORITY=1` to mix the
pooled conditional-rank signal equally with the existing scout heuristic
when ordering points-tail proposals. Independent conditional validation and
fresh production still decide the official cell estimates. Both switches are
off by default.

## Offline model check

Five DB-exported groups (16498, 16653, 16982, 16983, 16986) were evaluated
with 20 scout seeds (1001–1020), 1,000 seasons per scout, and independent
five-million-season references. Fixtures and raw JSONL runs are local under
ignored `experiments/rare_positions/local/2026-09-25/point-gap-smoothed-fivegroups-final/`.
The requests and references are the same as in the
[`adaptive points review`](2026-09-25-adaptive-points-review.md).

| Target cell | Reference probability | Team-only exact-point scout | Group observed-point proxy | Full frozen-table gap proxy |
| --- | ---: | ---: | ---: | ---: |
| 16653, team 125, 15th | 0.0000016 (8 hits) | mean 0.00000149; zero in 19/20 scouts | mean 0.00000211 | mean 0.00002271 |
| 16653, team 125, 16th | 0.0000642 (321 hits) | zero in 20/20 scouts | mean 0.00007710 | mean 0.00036606 |
| 16982, team 1526, 4th | 0.0000210 (105 hits) | zero in 20/20 scouts | mean 0.00002385 | mean 0.00002218 |

The group observed-point proxy gives a useful nonzero value for these cells.
The unrestricted frozen-table gap proxy overestimates team 125's 16th-place
probability by about 5.7 times. It changes the target's points without
changing the opponents' results in its nine remaining fixtures, which is a
material mismatch for this group. Treating the gap curve as 1, 5, 20, or 100
pseudo-observations at every point total raised aggregate error over the five
groups relative to using actual pooled observations. Thus the implementation
uses it only when the group had no observation at the queried total.

At the common 35M work limit, among 320 group-seed cells with a zero
equal-work plain-MC estimate,
reference probability below `1e-3`, and at least 25 reference hits, the
pooled observed-point proxy had mean absolute error `6.61e-6`, versus
`10.74e-6` for reporting zero. The improvement was uneven: the proxy's mean
absolute error exceeded zero's in groups 16498 (`9.73e-6` versus `7.16e-6`)
and 16983 (`14.90e-6` versus `10.94e-6`). This comparison is diagnostic;
it is not a calibrated uncertainty estimate or a reason to replace zeros in
the official matrix.

## Matched-work proposal check

The opt-in priority signal was compared with the previous adaptive default
using the same 20 seeds and work per run. It can change which conditional
proposal is validated or funded, but final probabilities still come from
fresh simulations.

| Group and limit | Candidate minus default whole-matrix coverage | Target stratum selected | Target mean ESS |
| --- | ---: | ---: | ---: |
| 16653, 11M | -0.009 (paired SD 0.200) | team 125: 7/20 vs 6/20 | 16th: 8.13 vs 7.62 |
| 16982, 35M | +0.035 (paired SD 0.141) | team 1526: 7/20 vs 6/20 | 4th: 2.97 vs 2.36 |

The target-cell improvements are small and the whole-matrix scores remain
essentially tied with the prior default. Raw matched-work arms and comparison
JSON are under ignored `local/2026-09-25/point-gap-priority-16653-11m-20/`
and `local/2026-09-25/point-gap-priority-16982-35m-20/`.

## Decision

Retain the pooled point/rank curve and standings-gap fallback as an opt-in
design diagnostic and proposal-ordering experiment. Do not promote its
model-based estimate to the official probability matrix. A broader
out-of-group calibration test and a way to account for the effect of the
target's own results on opponents are needed before that would be justified.
