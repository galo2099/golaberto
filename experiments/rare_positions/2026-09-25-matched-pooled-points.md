# Matched pooling for probabilities near 1e-6

Date: 2026-09-25. The question was whether actual points/rank outcomes from
other teams can give a more useful estimate of a roughly `1e-6` position than
100,000 ordinary MC seasons. The previous full-group pool was too biased for
many cells, and frozen-table counterfactuals were worse. This experiment uses
only actual simulated finishes and gives more weight to teams with a similar
expected final point total.

## Design and evaluation

For each target team, an exact points dynamic program gives its marginal
probability of every final point total `x`. For each `x`, the scout pools
actual point/rank observations from the 20 teams. Donor team `d` receives
weight `exp(-abs(expected_final_points_d - expected_final_points_target)/3)`.
The weighted fraction of donors with `x` points and rank `r` estimates the
target's conditional rank probability at `x`; the exact target points PMF
then weights those fractions. The target's own actual finishes are included.
This is called `matched_3` in the analysis script.

Every arm uses **the same 100,000 simulated seasons per group and seed**.
Plain MC estimates each cell directly from its rank count. The combined
matrix uses `matched_3` when plain MC observed at most 10 hits for that cell.
For other cells, it uses the target's own points/rank counts with five pooled
actual observations as a prior at each point total (`shrink_5`). Finally,
100 rounds of row/column normalization reconcile the matrix. The ten-hit
switch is based only on the scout, not on reference probabilities.

Five DB-exported groups (16498, 16653, 16982, 16983, 16986) and 20 scout
seeds (1001–1020) produced 100 runs and 2,000 team-position cells per run.
The original independent 5M-season references (seed 72991) were used to
screen the candidate and **define** the near-`1e-6` band
`[5e-7, 2e-6)`. A fresh independent 5M-season reference for each group
(seed 84321) scored the frozen candidate. This band has 43 cells across the
five groups. RMSE below averages squared error over the 20 scout seeds and
the selected cells, then takes the square root. The estimate and reference
use the same fixture model; neither represents observed real-world odds.
The [per-cell CSV](2026-09-25-matched-pooled-points-cells.csv) records the
reference counts and 20-seed mean and spread of each principal estimate for
all 2,000 cells. Positions in that file are one-based.

| Group | Near-`1e-6` cells | 100k plain MC RMSE | Matched pool RMSE | Whole 400-cell plain RMSE | Reconciled hybrid RMSE |
| --- | ---: | ---: | ---: | ---: | ---: |
| 16498 | 11 | 3.16e-6 | 0.703e-6 | 0.000625 | 0.000542 |
| 16653 | 10 | 3.46e-6 | 0.653e-6 | 0.000633 | 0.000542 |
| 16982 | 3 | 3.25e-6 | 0.772e-6 | 0.000652 | 0.000568 |
| 16983 | 9 | 3.22e-6 | 1.01e-6 | 0.000632 | 0.000554 |
| 16986 | 10 | 3.18e-6 | 0.413e-6 | 0.000641 | 0.000557 |
| **All** | **43** | **3.25e-6** | **0.722e-6** | **0.000636** | **0.000553** |

The matched pool had a nonzero estimate in all 860 near-band cell/run cases;
plain MC did so in 10%. On the `[1e-6, 1e-5)` band (88 cells), the matched
pool's RMSE was `2.05e-6` versus `6.17e-6` for plain MC. On the
`[5e-6, 1e-4)` band (146 cells), the hybrid's RMSE was `1.29e-5` versus
`1.84e-5`. For common cells (`>=1e-3`), the reconciled hybrid's RMSE was
`0.000683` versus `0.000787`. Thus the rare gain was not purchased by a
measured whole-matrix loss on these fixtures. The unrestricted matched pool
alone was inaccurate on common positions (RMSE `0.00649`); the hit-count
switch is essential.

Before reconciliation, the hybrid matrix's mean absolute row-sum deviation
was `1.65e-5` (maximum `0.000150`) and mean absolute column-sum deviation
was `0.00103` (maximum `0.00523`). Row/column normalization improved the
whole-matrix RMSE from `0.000565` to `0.000553` while changing near-band
RMSE only from `0.723e-6` to `0.722e-6`.

## Sharper check: group 16653, team 125

A third independent reference (20M seasons, seed 91873) gives 30M combined
seasons for group 16653. Across the ten cells selected near `1e-6` by the
original reference, matched pooling still has RMSE `0.574e-6`, versus
`3.45e-6` for 100k plain MC.

Team 125 finished 15th in 80 of the 30M reference seasons, giving
`2.667e-6` with an approximate binomial standard error of `0.298e-6`.
Across 20 scouts, the matched estimate averaged `1.574e-6` with a seed
standard deviation of `0.104e-6`. Plain MC returned zero in 17/20 scouts;
its mean was `1.5e-6` with standard deviation `3.66e-6`. The matched
estimate is much steadier and has lower squared error, but **underestimates
this specific cell by about 41%**. It is not yet a calibrated per-cell
uncertainty interval.

## Decision and limits

This meets the experimental goal **on the five tested fixtures**: at the
same number of simulated seasons, a pooled point/rank estimator substantially
reduces measured error for cells near `1e-6`, and the hit-count hybrid also
reduces whole-matrix error. The point model adds analysis work beyond the 100,000 seasons;
the comparison controls simulation count, not wall-clock time. The reference
at `1e-6` has only a few hits per 5M seasons, so one cell's result remains
noisy; the 30M team-125 check shows a material remaining bias. The five
fixture groups were used for candidate screening, so an unseen-group check
and a calibrated standard error are still needed before a default switch.

The tracked benchmark harness is
[`go/rare_position_pooled_estimator_benchmark_test.go`](../../go/rare_position_pooled_estimator_benchmark_test.go),
and the analysis is
[`compare_pooled_100k.py`](compare_pooled_100k.py). The former saves the
point/rank counts and exact points PMFs; the latter computes all candidates,
the ten-hit hybrid, reconciliation, and error by band. Fresh reference seeds
are supported by `RARE_POSITION_REFERENCE_SEED` in the reference builder.
Raw DB-derived fixtures, scouts, and references remain ignored under
`experiments/rare_positions/local/2026-09-24/` and
`local/2026-09-25/`. Their seed and fixture hashes are recorded in the
reference JSON files.

## Production implementation (2026-09-25)

The Go odds service offers the tested hybrid through
`RARE_POSITION_MATCHED_POINT_POOL=1`. The flag enables rare-position processing
on its own and takes precedence over the CEM and diversified flags. It runs
100,000 ordinary seasons in one joint scout, computes each team's exact
additional-points distribution, applies the ten-hit `matched_3`/`shrink_5`
rule, and replaces the full `team_odds` position matrix with its reconciled
result. The original 20,000-season request scout still supplies game-score
odds and game importance. The response retains the existing JSON shape and
includes one `rare_position_estimates` entry per team and position with
`design: "matched_point_pool"`.

Production continues row/column normalization until each sum differs from
one by at most `1e-6`. The 100 rounds used in offline screening were not
sufficient for the sparse matrices of three reference groups. The extra
rounds keep structural zeros from the points feasibility check. If the
standings are not points-led, a team has more than 39 remaining fixtures,
the exact points model is unavailable, or reconciliation fails, the request
uses a 100,000-season ordinary MC estimate and logs the reason. When
reconciliation fails after the joint scout, its own rank counts supply this
fallback; unsupported standings draw a separate ordinary-MC batch.

Ten equal batches of the *same* 100,000 seasons provide delete-one-batch
jackknife `std_err` and `relative_se`. They describe sampling variability
only. They do not include donor mismatch or model bias; therefore
`meets_precision_goal` is false for all pooled cells and `ess` is left at
zero (not applicable). `hits` reports ordinary scout rank hits, while
`zero_hit_upper_95` is the smaller of the ordinary-MC zero-hit bound and the
exact points-feasibility bound. A nonzero pooled estimate remains a rough
order-of-magnitude result, especially for an individual rare cell such as
team 125 finishing 15th. Do not interpret the jackknife error as a calibrated
confidence interval.

The production path was scored against the independent 5M-season holdout
references with the original reference fixing the 43 near-`1e-6` cells.
Across 20 seeds per group (100 production runs, 860 selected cell/runs), its
near-band RMSE was `0.722e-6` versus `3.25e-6` for 100k plain MC.
Whole-matrix RMSE over 40,000 cell/runs was `0.000553` versus `0.000636`.
All five groups used the pooled path with no fallback. The full benchmark
took 287 seconds locally, averaging 2.9 seconds for the 100,000-season
pooled calculation including batch jackknifing; the ordinary initial request
scout is additional work. The regression test
`TestMatchedPointPoolReferenceBenchmark` reproduces this check using the
five saved fixture paths, the original selection-reference directory, the
independent holdout-reference directory, and
`RARE_POSITION_BENCHMARK_SEEDS=20`. The feature remains **opt-in** because
these groups were used to select the method and per-cell bias remains
uncalibrated.

## Runtime optimization (2026-09-26)

I profiled the complete odds request for group 16653 (20 teams, 90 unplayed
fixtures) on an Apple M2 Pro with Go 1.26. The benchmark keeps the production
20,000-season request scout and 100,000-season matched pool, uses random seed
808, and runs five requests per measurement. The request JSON was captured
from the local database. Times below are median wall time per complete request
across three measurements of five requests each:

| Implementation | Time/request | Relative to 1.03 s baseline |
| --- | ---: | ---: |
| Before the first profiling pass | 1.995 s | 1.94× slower |
| Reused campaigns and games, Poisson CDF, dense full scout | 1.029 s | Baseline |
| Dense batch counters and dense matrix balancing, one worker | 0.662 s | 36% faster |
| Same code, two matched-pool workers (default on at least two cores) | 0.447 s | 57% faster |
| Same code, four matched-pool workers | 0.357 s | 65% faster |

The CPU profiles led to four changes: reuse per-season campaign and game
storage; precompute each fixture's Poisson CDF with a small lookup table;
record the full scout and its ten jackknife batches in dense arrays before
converting them to response maps; and balance each probability matrix in a
dense array. Updating both teams' standings together also reduced work per
fixture. The one-worker timing reflects CPU improvements. Multiple workers
reduce request latency by using more cores; they do not divide CPU work by
the worker count.

For groups with at least 20 unfinished fixtures, the matched pool runs its
ten existing batches independently using up to two workers by default, or
one worker when only one is available. Set
`RARE_POSITION_MATCHED_POINT_POOL_WORKERS=1` for serial execution or `=4`
for lower latency on a machine with spare cores (capped at ten workers).
Smaller groups use one worker. The total remains 100,000 sampled seasons,
with ten batches for the same jackknife estimator. The independent worker
streams change seeded results, while the sampled probability model remains
the same.

The Go package tests pass, including the Poisson distribution check and the
parallel batch aggregation test. The latter also passes under Go's race
detector. These benchmarks describe one representative group on one machine;
throughput with simultaneous odds requests depends on available cores.
