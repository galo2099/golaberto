# Adaptive points-stratum review and confirmation

Date: 2026-09-25. Reviewed source: the adaptive estimator introduced through
`965a5e2` on `adaptive-point-stratified-rare-positions-11626508236358776789`.
All candidate and plain-MC arms used the same 35,000,000 nominal-work limit,
master seeds 1001–1020, and independent five-million-season references. The
reference matrices were used for offline scoring only.

## Review findings and changes

1. The adaptive production path returned estimates but populated only the
   feasibility field of the benchmark diagnostics. The harness consequently
   recorded **zero work** for adaptive runs. It now reports charged scout,
   discovery, validation, and production work, along with actual production
   batches and validation cells.
2. A hard-coded team-125 conditional diagnostic ran outside the work budget.
   It was removed. Exact points DP and repeated rank-bound analysis now carry
   nonzero nominal discovery work. Rank bounds are cached across cells so the
   charged analysis no longer repeats a fixture scan for every point/rank.
3. The scheduler could allocate production to several strata for the same
   team, although production retained only one stream per team. It now keeps
   the first independently admitted proposal in the frozen utility order.
4. The profile generator favored broad point sets with mass near one. The
   reviewed candidate also builds exact upper and lower points tails with
   target masses 0.0015, 0.005, and 0.02. Design-only neighboring-rank
   evidence orders candidates; independent validation still gates each
   production cell. The conditional variance scheduler uses the disjoint
   outside-stratum scout count plus validated inside-stratum probability.
5. A zero-probability conditional DP path now fails explicitly rather than
   silently selecting a result outside its stratum. Per-cell debug logs are
   available through `RARE_POSITION_ADAPTIVE_DEBUG=1`.

The tested default **inside the opt-in diversified estimator** uses 1,000
scout seasons, tail-only proposals, and a 97.5% minimum plain share of
production work. `RARE_POSITION_DIVERSIFIED_IS=1` is still required to use
that estimator. Set `RARE_POSITION_ADAPTIVE_TAIL_ONLY=0` to screen the broader
profile groups again; the other tested settings can also be overridden with
`RARE_POSITION_ADAPTIVE_SCOUT_SAMPLES` and
`RARE_POSITION_ADAPTIVE_MIN_PLAIN_PRODUCTION_FRACTION`.

## Experiment sequence

Group 16982 exposed the original work-reporting bug. With the work recorded,
the pre-review adaptive path spent 5.25M on scout and 14.06M on repeated
discovery, admitted no conditional stream, and scored 344.00 against 354.00
for plain MC in the first seed. Narrow tail proposals, cached rank bounds,
and corrected allocation raised its five-seed mean to 353.38, just above
plain MC. The 20-seed tail-only arm with the former 80% plain-production
floor gained 2.01 whole-matrix coverage points over plain MC, but common-cell
estimated variance was 21.6% higher. That arm failed the variance guard and
was rejected.

The 97.5% plain-production floor reduced the group 16982 whole-matrix gain
to +0.487 in 20 seeds while keeping common-cell variance 9.67% above plain
MC. This guarded setting was then confirmed across all five real groups.

| Group | Coverage score minus plain MC | ESS>=10 cells | ESS>=25 cells | Common-cell variance ratio |
| --- | ---: | ---: | ---: | ---: |
| 16498 | -0.075 | +0.80 | +0.60 | 1.045 |
| 16653 | -0.135 | +0.30 | +1.10 | 1.038 |
| 16982 | +0.487 | +0.70 | +1.00 | 1.097 |
| 16983 | +0.287 | +0.85 | +0.65 | 1.083 |
| 16986 | -0.331 | +0.60 | +0.70 | 1.095 |
| **Mean over 100 group-seed pairs** | **+0.047** | **+0.65** | **+0.81** | — |

The standard deviation of paired whole-matrix coverage differences over the
100 group-seed pairs was 1.491. Thus the aggregate coverage result is
effectively a tie with equal-work plain MC, although the ESS threshold counts
improve. In the earlier five-seed comparison against the preceding
diversified estimator, the guarded candidate gained 2.465 coverage points
and 3.12 ESS>=10 cells per group-seed pair. The five-group result does not
establish a general whole-matrix win over plain MC.

The `[1e-5, 1e-4)` reference-probability band gained 0.146 coverage-score
points and 0.83 ESS>=10 cells on average; the `[1e-6, 1e-5)` band lost 0.021
score points. Common cells (`>=1e-3`) were already saturated at ESS 10, and
their mean estimated variance rose with the smaller ordinary production
stream. Reference cells with fewer than 25 hits are excluded from calibration
claims. Five-million-season references can still be noisy in the smallest
band.

For group 16653, team 125's reference probabilities were `1.6e-6` for 15th
and `6.42e-5` for 16th. The guarded automatic estimator raised mean ESS from
0.80 to 1.35 for 15th and from 17.00 to 86.59 for 16th, while whole-matrix
coverage stayed near plain MC. The team-125 stratum was selected in 18 of 20
runs. For group 16982, team 1526's fourth-place reference was `2.1e-5`;
the guarded estimator selected a team-1526 stratum in only six of 20 runs and
had mean ESS 2.36 versus 2.45 for plain MC. The earlier explicit group-16982
hybrid remains better for that particular cell.

## Production-budget check

The 35M comparison is the common research cap. In production,
`calculateMaxRareWork` currently sets the work limit to 100,000 ordinary
season equivalents. That equals 11.0M for group 16653 and 12.3M for group
16498 at these fixtures. Separate 20-seed matched comparisons at those limits
gave:

| Group | Production work limit | Coverage minus plain MC | ESS>=10 cells | Common-cell variance ratio |
| --- | ---: | ---: | ---: | ---: |
| 16653 | 11.0M | +0.277 (SD 1.374) | +1.25 | 1.085 |
| 16498 | 12.3M | +0.134 (SD 1.341) | -0.60 | 1.084 |

At 11M, team 125's 16th-place mean ESS was 7.62 versus 5.20 for plain MC;
the conditional stratum was selected in six of 20 runs. These production
budget results are more representative of the service for the two groups
than their 35M research results. Set
`RARE_POSITION_BENCHMARK_WORK_LIMIT` to the desired work limit to reproduce
them; both estimator arms in a benchmark run receive the same value.

## Reproduction and decision

The tracked benchmark procedure is in
[`doc/rare_position_experiments.md`](../../doc/rare_position_experiments.md).
The tested guarded settings are now defaults within the opt-in diversified
path. To reproduce explicitly, set:

```sh
RARE_POSITION_DIVERSIFIED_IS=1
RARE_POSITION_ADAPTIVE_SCOUT_SAMPLES=1000
RARE_POSITION_ADAPTIVE_TAIL_ONLY=1
RARE_POSITION_ADAPTIVE_MIN_PLAIN_PRODUCTION_FRACTION=0.975
```

Export the five real requests from the DB, build a fresh five-million-season
reference for each request, and run
`TestDiversifiedMatchedWorkBenchmark` with 20 seeds. The local runs and
comparisons for this review are under ignored
`experiments/rare_positions/local/2026-09-26/`. The 20-seed file
`adaptive-tailonly-plain975-fivegroups-20.jsonl` contains 200 arm rows
(candidate and plain MC for every group and seed). DB-derived requests,
references, and raw per-run output are not committed.

Reference seed: `72991`. Fixture SHA-256 by group:

| Group | Request SHA-256 |
| --- | --- |
| 16498 | `b37c82f317d5f0e412b2642101929cca778857c2792dc89c0fbb830e702e701e` |
| 16653 | `2d1c1d6f70a64d5b86f5129b32cbf77bedc4c42e464cd849011ca0b3018b3d87` |
| 16982 | `70e0d90c06c16c0529ec04dd9824f02e43a8cced244ec3c7b785293632e7e14a` |
| 16983 | `43969b02fe7138b991b9d74215223efbc9fc16bc8f4ebe6d0a5e274126f8af36` |
| 16986 | `8a887edd5fc88039ea2a43b4bc2f40df7916a956376a09c9fa3fad1cb1d2af88` |

Decision: retain the guarded automatic estimator inside the existing opt-in
path and keep the explicit group-specific hybrid available. Its full-matrix
score ties plain MC across the five tested groups while improving more rare
cells to ESS 10 and 25. Further work should target the missing 16982
fourth-place cell without raising common-cell variance above 10%.
