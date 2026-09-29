# Rare-position heuristic time allocation (2026-09-29)

## Question and method

Where does a full odds request spend time, and can it move work between heuristics to turn more unresolved zero cells into estimates?

I profiled the complete `calculate_odds` path with `RARE_POSITION_MATCHED_POINT_POOL=1`, 20,000 initial Monte Carlo seasons, the 100,000-season matched pool, and `GOMAXPROCS=4`. Stage timers and experimental budget controls were temporary and were removed after the runs. Each comparison used the same input and seed on both sides. The difficult inputs were the current saved group 16653 request (85 unplayed fixtures), the saved group 16498 request (103), and an earlier group 16653 request (90). Seeds: 808, 811, 817, 900, 911. Times below are medians of five in-process full requests; process startup and compilation are excluded. Work units are the service's existing simulation-work measure, not elapsed time.

The saved group 16982, 16983, and 16986 requests had all 400 cells nonzero at seed 808. Their conditioned zero searches exited immediately, so these requests offer no zero-cell search budget to redistribute. Their full request times were roughly 878, 885, and 913 ms, respectively.

## Baseline time profile

All times are milliseconds. `Other` includes the 20,000-season initial scout and work outside the instrumented matched-pool stages. The subrows of `Conditioned total` are contained within that total.

| Stage | 16653 current | 16498 | 16653 earlier |
| --- | ---: | ---: | ---: |
| Full request | 933 | 1,023 | 938 |
| Other, including initial scout | ~125 | ~142 | ~134 |
| 100,000-season matched-pool scout | 133 | 147 | 139 |
| Matched-pool matrix and leave-one-batch-out matrices | 89 | 95 | 95 |
| Conditioned total | 586 | 639 | 570 |
| └ Direct conditioned search | 58 | 77 | 58 |
| └ Guided pilot and confirmation | 128 | 19 | 93 |
| └ Extra zero-hit extension | **227** | **260** | **246** |
| └ Neighborhood reachability | 6 | 10 | 4 |
| └ Joint-point witness search | 10 | 9 | 6 |
| └ Point-tilt pilots and confirmations | 150 | 254 | 176 |

The 200,000-draw extra zero-hit extension is the largest single zero-cell stage, about 24–25% of a difficult full request. Its selection score rewards reduction of a 95% zero-hit upper bound; it is not a direct estimate of the chance of producing a usable nonzero estimate. For current 16653 at seed 808, it used four 50,000-draw jobs, 21 million work units, and 244 ms.

The extension has a large fixed cost. A direct timer on current 16653 measured 104 ms of event planning before any of the four jobs ran. Four 50,000-draw jobs completed at 243 ms from the start of that stage; four 10,000-draw jobs completed at 142 ms. Halving the total budget to two 50,000-draw jobs saved almost no wall time because the same planning runs and the jobs use fewer of the four cores.

The cheap proof stages are good value. Neighborhood search and joint-point witness search together take about 10–20 ms on these inputs and produce reachability witnesses or impossibility proofs. The 100,000-season pool and its leave-one-batch-out matrices support the broad position matrix and uncertainty estimates; their cost is not interchangeable with a cell-focused search without changing that base estimator.

## Paired allocation experiments

The coverage-first variant removed the extra zero-hit extension, expanded point-tilt pilots from 12 to 24 cells, and allowed up to five proved-cell and seven undecided-cell confirmations (formerly four and three). The same acceptance gates still required fresh confirmation hits, effective sample size, relative error, and bounded weight concentration. There is no inference from adjacent positions or fabricated probability for a zero-hit cell.

Each delta is the change in nonzero cells in one complete 400-cell request. A cell-run is one team/position in one fixed-seed run; the same cell appearing under five seeds counts five times.

| Input | Baseline median | Coverage-first median | Nonzero delta by seed | Baseline → variant matched-pool work |
| --- | ---: | ---: | --- | ---: |
| 16653 current | 933 ms | **799 ms** | +4, +4, +3, +3, +3 | 57.4M → 50.4M |
| 16498 | 1,023 ms | **856 ms** | +1, +1, +1, +1, +1 | 67.4M → 58.1M |
| 16653 earlier | 938 ms | **785 ms** | +4, +6, +3, +4, +2 | 61.2M → 48.9M |

That is 41 gained nonzero cell-runs and zero lost nonzero cell-runs across 15 paired requests, with median full-request times 14–16% lower. Across the saved inputs, gains involved five distinct cells in current 16653, two in 16498, and seven in earlier 16653; not 41 distinct cells. Examples include current 16653 team 22 in 17th, team 95 in 5th and 6th, and 16498 team 318 in 5th. These are very small, independently confirmed importance-sampling estimates. This experiment measures coverage and cost, not accuracy against an exhaustive reference probability.

Simply deleting the extension saved 200–250 ms but did not reliably improve coverage, and on earlier 16653 it lost two nonzero cells for seed 808. Increasing only the number of final confirmations also did not help. The wider *pilot* set is what exposed useful proposals; adding one proved-cell confirmation slot prevented a regression for team 68 in 1st on the earlier 16653 request. Its pilot had one hit, but the original four proved-cell slots were taken by higher-scoring candidates after the extension was removed.

## Upper-bound tradeoff

The extension still provides valuable information for zeros it cannot estimate. Removing it weakened the 95% upper bound for one remaining zero cell in current 16653 and four in 16498 consistently across the five seeds. At seed 808:

| Remaining zero cell | Baseline upper bound | Coverage-first upper bound |
| --- | ---: | ---: |
| Current 16653, team 95 / 3rd | 3.47e-12 | 2.12e-7 |
| 16498, team 318 / 3rd | 1.37e-14 | 9.26e-10 |
| 16498, team 16 / 12th | 1.20e-7 | 6.01e-6 |

These are probabilities on a 0–1 scale, not percentages. The bounds remain valid but materially less informative. Keeping four shortened 10,000- or 20,000-draw extension jobs did not recover the strongest bounds for these cells. The 10,000-draw version still spent about 130–160 ms in the extension because event planning costs about 100 ms. It also did not improve coverage over the coverage-first variant and sometimes displaced a gain.

## Conclusion and next experiment

There is a clear opportunity to spend less on general zero-hit upper-bound tightening and more on scouting distinct unresolved cells. A 24-cell point-tilt pilot with one extra proved confirmation improved coverage in every paired run while reducing elapsed time. It is **not** a drop-in production replacement if the current strong upper bounds for still-zero cells are important.

The next engineering experiment should make extra-event planning incremental: rank cells using cheap existing bounds and pilot evidence, build the expensive deeper point events only for the few selected cells, and reuse events already built by direct search where possible. Measure the planning time separately, then test whether one or two bound-focused jobs can run alongside the wider point-tilt pilots while staying under one second. Compare both nonzero coverage and each remaining zero's upper bound; aggregate nonzero counts alone can hide regressions.
