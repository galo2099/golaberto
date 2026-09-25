# Points-stratum hybrid: groups 16653 and 16982

Experiment date: 2026-09-25. The hypothesis is that a shallow, exact points
tail can guide conditional Monte Carlo toward rare finishing positions while
ordinary Monte Carlo estimates the full team-by-position matrix. The
comparison uses 20 matched master seeds (1001–1020), a 35,000,000 nominal-work
limit per estimator and run, and an independent five-million-season reference
per group. The reference is used only for offline scoring.

## Estimator and work accounting

For a target team, let `A` mean that it earns at least a chosen number of
points in its remaining fixtures. A dynamic program combines the exact
Poisson win/draw/loss probabilities for those fixtures to obtain `P(A)`.
Conditional Monte Carlo draws the target team's W/D/L pattern given `A`, then
draws scores given those results. All other matches use the original Poisson
model. For each selected target rank, two fresh production streams estimate

```text
P(rank) = P(rank, outside A) + P(A) × P(rank | A).
```

The ordinary stream supplies every other matrix cell. Scout, pure-conditional
discovery, and independent conditional validation are design-only. Production
sample counts are frozen before production starts. Each phase, including the
points dynamic program, is charged to the 35M limit. The selected conditional
stream receives 2% of remaining production work if validation observes at
least two target-rank hits. The group and target are explicit environment
settings; the general estimator remains the default.

## Group 16653, team 125

Team 125 had 17 points and nine remaining matches. Finishing 16th or higher
requires at least 15 more points, giving the simple exact upper bound
`P(points >= 15) = 0.0763297485`. The narrower production stratum was
`A = points >= 21`, with exact mass `0.00154226665`; selected ranks were
1st–16th (zero-based `MAX_RANK=15`). Scout, discovery, and validation consumed
110,000, 49,000, and 200,000 work; production consumed 34,641,000 work.

| 20-seed paired comparison | Coverage score | ESS >= 10 cells | ESS >= 25 cells |
| --- | ---: | ---: | ---: |
| Hybrid minus current diversified | +0.850 (SD 1.097) | +0.25 | +2.25 |
| Hybrid minus equal-work plain MC | -0.014 (SD 1.312) | — | — |

The target's 15th-place mean ESS was 5.01 for the hybrid versus 0.80 for
plain MC; its 16th-place mean ESS was 98.62 versus 17.00. Mean estimated
variance on common cells was about 3.1% above plain MC. A five-seed screen
giving only 1% of production work to the conditional stream lost 0.16 coverage
score versus 2%, so it was rejected. The hybrid improves the selected tail
but is essentially tied with plain MC over the full matrix.

## Group 16982, team 1526

The DB-exported fixture had 20 teams and 330 unplayed matches, 33 involving
the target. Team 1526 had three points. A table-based necessary condition for
top four was at least six additional points, but its exact probability was
`0.999994654`, so that bound was too loose to guide sampling. A design-only
target mass of 0.0015 selected `A = additional points >= 52` with exact mass
`0.00164752529`. The conditional validation stream saw 17–20 top-four hits
per 1,000 samples in the five-seed screen. Selected production ranks were
1st–4th (zero-based `MAX_RANK=3`). The 20-seed run spent 350,000 scout,
169,000 discovery, 680,000 validation, and 33,800,970 production work: a
total of 34,999,970. Production used 94,643 ordinary and 994 conditional
seasons per run.

| 20-seed mean, full 400-cell matrix | Current diversified | Points hybrid | Equal-work plain MC |
| --- | ---: | ---: | ---: |
| Coverage score, capped at ESS 10 per cell | 349.997 | 353.967 | 353.371 |
| Cells with ESS >= 10 | 339.70 | 344.55 | 344.35 |
| Cells with ESS >= 25 | 326.35 | 330.60 | 331.80 |

Paired hybrid-minus-current coverage gain was **+3.969** (SD 1.104), with
+4.85 ESS>=10 cells and +4.25 ESS>=25 cells. The gain by reference-probability
band was +0.850 for `[1e-4, 1e-3)`, +2.849 for `[1e-5, 1e-4)`, and +0.270
for `[1e-6, 1e-5)`. Paired hybrid-minus-plain coverage was **+0.596**
(SD 1.415), with +0.20 ESS>=10 cells and -1.20 ESS>=25 cells. That small
positive mean does not establish a dependable whole-table advantage over
plain MC. Mean estimated variance on common cells was 5.66% above plain MC.

The five-million-season reference saw team 1526 in fourth 105 times
(`2.1e-5`). Hybrid fourth-place estimates averaged `2.453e-5` and ESS 15.03;
equal-work plain MC averaged `2.45e-5` and ESS 2.45. All 20 hybrid runs met
ESS 10 for this cell; none of the plain runs did. The third-place reference
had 29 hits (`5.8e-6`); hybrid mean ESS was 2.56 versus 0.60 for plain MC.
Second place had only one reference hit (`2e-7`), so its reference value is
too uncertain for a strong calibration claim. These reference counts show
why coverage and uncertainty should be considered alongside point estimates.

## Decision and continuation

Keep the hybrid as an opt-in for these tested group/team combinations. It
improves the chosen rare cells and beats the current diversified estimator in
both groups. Group 16653 ties plain MC on the whole matrix; group 16982 has
only a small, noisy mean advantage and fewer ESS>=25 cells than plain MC.
Further group or threshold tests should keep the scout/discovery/validation
and production streams separate and use fresh 20-seed confirmation.

For group 16982, enable the tested configuration with:

```sh
RARE_POSITION_DIVERSIFIED_IS=1 \
RARE_POSITION_POINT_HYBRID_GROUP=16982 \
RARE_POSITION_POINT_HYBRID_TEAM=1526 \
RARE_POSITION_POINT_HYBRID_MAX_RANK=3 \
RARE_POSITION_POINT_HYBRID_TARGET_MASS=0.0015 \
RARE_POSITION_POINT_HYBRID_Q_PERCENT=2
```

For group 16653, use group `16653`, team `125`, max rank `15`, conditional
work percent `2`, and leave `TARGET_MASS` unset. The code then selects the
tested threshold `minimum + 6`.

To rerun, export the requests from the current DB with
`bin/rails runner script/export_rare_position_groups.rb /tmp/real-fixtures
16653 16982`. Build fresh five-million-season references with
`TestDiversifiedBuildReference` as described in
[`doc/rare_position_experiments.md`](../../doc/rare_position_experiments.md).
Then run `TestDiversifiedMatchedWorkBenchmark` twice with the same fixture,
reference directory, `RARE_POSITION_BENCHMARK_SEEDS=20`, and distinct output
directories: once without the hybrid variables and once with the settings
above. Compare the two `runs.jsonl` files using
`script/compare_rare_position_benchmarks.py`. This benchmark also records an
equal-work plain-MC arm in each run. The DB-derived requests, reference
matrices, and raw per-run files remain in ignored
`experiments/rare_positions/local/2026-09-24/` and
`experiments/rare_positions/local/2026-09-25/`; they are not in Git.

Fixture SHA-256 values for these results were
`2d1c1d6f70a64d5b86f5129b32cbf77bedc4c42e464cd849011ca0b3018b3d87`
(16653) and
`70e0d90c06c16c0529ec04dd9824f02e43a8cced244ec3c7b785293632e7e14a`
(16982). Both references used seed 72991. Re-exporting after the DB changes
can produce a different request hash and requires a new reference and
comparison.
