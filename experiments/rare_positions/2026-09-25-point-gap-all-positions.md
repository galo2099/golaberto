# Point/rank proxy comparison at every position

Date: 2026-09-25. This reanalyzes the saved point-gap experiment for
groups 16498, 16653, 16982, 16983, and 16986. Each group has 20
independent 1,000-season scouts (seeds 1001–1020). Each reference is
an independent 5,000,000-season MC run under the same fixture model.
All 2,000 team-position cells, including reference zeros, are in
[`2026-09-25-point-gap-all-cells.csv`](2026-09-25-point-gap-all-cells.csv).

`team_only` uses each team's own observed rank frequencies conditional
on its points; `observed` pools actual point/rank outcomes from all 20
teams; `gap` hypothetically moves every team to each point total in a
frozen simulated final table. Each reweights its conditional rank curve
with that target team's exact additional-points distribution. The
table values below are mean absolute errors of the **20-seed mean**
estimate against the independent reference, averaged over cells.
They measure bias plus residual scout error and are not standard errors.

We summarize cells with at least 25 reference hits (1,624 of 2,000).
With fewer hits, the reference has at least about 20% relative MC
standard error; a zero-hit reference is not proof of impossibility.
All cells remain in the CSV. Errors are probabilities, not percentages.

## Across all positions

| Group | Cells ≥25 hits | Team-only MAE | Actual pooled MAE | Frozen gap MAE | Actual pooled beats gap |
| --- | ---: | ---: | ---: | ---: | ---: |
| 16498 | 297 | 0.0009396 | 0.009155 | 0.01655 | 187/297 |
| 16653 | 296 | 0.0009497 | 0.007428 | 0.01575 | 200/296 |
| 16982 | 369 | 0.001026 | 0.004851 | 0.007323 | 172/369 |
| 16983 | 317 | 0.001057 | 0.007911 | 0.01451 | 217/317 |
| 16986 | 345 | 0.001022 | 0.006125 | 0.009167 | 192/345 |

## By reference probability

The rare band is still heterogeneous: a pooled proxy can help a
specific unseen outcome while biasing other cells in the same band.

| Reference probability | Cells ≥25 hits | Team-only MAE | Actual pooled MAE | Frozen gap MAE |
| --- | ---: | ---: | ---: | ---: |
| <0.001 | 317 | 0.0001286 | 0.0001346 | 0.0005224 |
| 0.001–<0.1 | 944 | 0.0009258 | 0.004274 | 0.007963 |
| ≥0.1 | 363 | 0.00196 | 0.01998 | 0.03404 |

For the narrower case that motivated pooling—reference probability
below 0.001, at least 25 reference hits, and **zero team-only estimates
in all 20 scouts**—the errors are:

| Cells | Team-only MAE (reported zero) | Actual pooled MAE | Frozen gap MAE |
| ---: | ---: | ---: | ---: |
| 94 | 2.791e-05 | 1.539e-05 | 0.0006944 |

## Every group and position

Each row averages across the teams whose reference cell has at least
25 hits. A position may therefore have fewer than 20 included teams.
The full per-team reference, 20-seed means, seed SDs, absolute errors,
and zero-seed counts are in the CSV. Positions here are one-based.

### Group 16498

| Position | Teams ≥25 hits | Team-only MAE | Actual pooled MAE | Frozen gap MAE |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 6 | 0.001594 | 0.03727 | 0.06701 |
| 2 | 8 | 0.001438 | 0.04165 | 0.06549 |
| 3 | 16 | 0.0007646 | 0.01172 | 0.05304 |
| 4 | 17 | 0.0004634 | 0.005089 | 0.008695 |
| 5 | 18 | 0.0008776 | 0.003074 | 0.003932 |
| 6 | 17 | 0.0009928 | 0.00587 | 0.0098 |
| 7 | 16 | 0.0009766 | 0.006661 | 0.01048 |
| 8 | 16 | 0.001077 | 0.005457 | 0.009523 |
| 9 | 17 | 0.00114 | 0.00492 | 0.007279 |
| 10 | 17 | 0.001288 | 0.003718 | 0.00539 |
| 11 | 17 | 0.0006718 | 0.002387 | 0.004241 |
| 12 | 17 | 0.0007998 | 0.002296 | 0.004308 |
| 13 | 18 | 0.001025 | 0.003143 | 0.004009 |
| 14 | 17 | 0.0006261 | 0.004011 | 0.004365 |
| 15 | 16 | 0.001072 | 0.004546 | 0.005947 |
| 16 | 15 | 0.0009997 | 0.004707 | 0.007949 |
| 17 | 14 | 0.0009026 | 0.005094 | 0.01267 |
| 18 | 14 | 0.001138 | 0.01882 | 0.04035 |
| 19 | 13 | 0.0009136 | 0.03373 | 0.04211 |
| 20 | 8 | 0.0005456 | 0.03394 | 0.05294 |

### Group 16653

| Position | Teams ≥25 hits | Team-only MAE | Actual pooled MAE | Frozen gap MAE |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 14 | 0.001065 | 0.005763 | 0.02101 |
| 2 | 15 | 0.001662 | 0.005428 | 0.008767 |
| 3 | 15 | 0.0008932 | 0.005415 | 0.00581 |
| 4 | 15 | 0.0009389 | 0.004447 | 0.007675 |
| 5 | 15 | 0.0009443 | 0.005118 | 0.009216 |
| 6 | 16 | 0.0009728 | 0.005607 | 0.008277 |
| 7 | 17 | 0.001163 | 0.00478 | 0.006108 |
| 8 | 18 | 0.001064 | 0.003232 | 0.004565 |
| 9 | 18 | 0.001148 | 0.003465 | 0.005032 |
| 10 | 18 | 0.001123 | 0.003775 | 0.005796 |
| 11 | 18 | 0.001465 | 0.004038 | 0.005669 |
| 12 | 18 | 0.0007936 | 0.004398 | 0.006057 |
| 13 | 18 | 0.0008553 | 0.00507 | 0.007541 |
| 14 | 18 | 0.0006075 | 0.005719 | 0.009588 |
| 15 | 16 | 0.0009298 | 0.009018 | 0.0179 |
| 16 | 15 | 0.000581 | 0.007455 | 0.008613 |
| 17 | 12 | 0.0003836 | 0.007691 | 0.01469 |
| 18 | 12 | 0.0004717 | 0.01325 | 0.06974 |
| 19 | 6 | 0.0002963 | 0.05752 | 0.1593 |
| 20 | 2 | 0.0008109 | 0.1269 | 0.2389 |

### Group 16982

| Position | Teams ≥25 hits | Team-only MAE | Actual pooled MAE | Frozen gap MAE |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 13 | 0.0007649 | 0.006152 | 0.02501 |
| 2 | 16 | 0.0009283 | 0.01316 | 0.009202 |
| 3 | 19 | 0.0009919 | 0.01296 | 0.02427 |
| 4 | 19 | 0.000824 | 0.005448 | 0.008727 |
| 5 | 19 | 0.0006366 | 0.002974 | 0.004801 |
| 6 | 20 | 0.00115 | 0.002472 | 0.003384 |
| 7 | 20 | 0.001277 | 0.002268 | 0.002397 |
| 8 | 20 | 0.001024 | 0.002272 | 0.002387 |
| 9 | 20 | 0.0008746 | 0.002157 | 0.002527 |
| 10 | 20 | 0.0009196 | 0.002153 | 0.002695 |
| 11 | 20 | 0.001173 | 0.002152 | 0.002938 |
| 12 | 20 | 0.000957 | 0.001957 | 0.00325 |
| 13 | 19 | 0.00115 | 0.002471 | 0.003695 |
| 14 | 19 | 0.0008983 | 0.003732 | 0.004256 |
| 15 | 18 | 0.001466 | 0.005413 | 0.007093 |
| 16 | 18 | 0.001492 | 0.005315 | 0.008523 |
| 17 | 18 | 0.001042 | 0.004722 | 0.006751 |
| 18 | 18 | 0.00085 | 0.007463 | 0.007308 |
| 19 | 17 | 0.001169 | 0.006451 | 0.008836 |
| 20 | 16 | 0.0008491 | 0.009035 | 0.01778 |

### Group 16983

| Position | Teams ≥25 hits | Team-only MAE | Actual pooled MAE | Frozen gap MAE |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 7 | 0.001998 | 0.03547 | 0.0571 |
| 2 | 14 | 0.001171 | 0.02629 | 0.04039 |
| 3 | 17 | 0.00103 | 0.02175 | 0.02784 |
| 4 | 18 | 0.001077 | 0.01769 | 0.02783 |
| 5 | 18 | 0.001114 | 0.007264 | 0.01396 |
| 6 | 17 | 0.001111 | 0.00485 | 0.006579 |
| 7 | 17 | 0.001124 | 0.002698 | 0.002974 |
| 8 | 17 | 0.001229 | 0.001921 | 0.002225 |
| 9 | 17 | 0.0008514 | 0.001492 | 0.002637 |
| 10 | 16 | 0.001107 | 0.001558 | 0.004274 |
| 11 | 16 | 0.001101 | 0.001548 | 0.004252 |
| 12 | 17 | 0.0008662 | 0.001506 | 0.003547 |
| 13 | 17 | 0.00106 | 0.001471 | 0.003268 |
| 14 | 17 | 0.000833 | 0.001411 | 0.002144 |
| 15 | 16 | 0.001195 | 0.001923 | 0.002091 |
| 16 | 17 | 0.0008068 | 0.00219 | 0.003915 |
| 17 | 17 | 0.001115 | 0.002368 | 0.006331 |
| 18 | 17 | 0.00143 | 0.01081 | 0.04472 |
| 19 | 16 | 0.0005972 | 0.01889 | 0.03181 |
| 20 | 9 | 0.0007144 | 0.01864 | 0.04432 |

### Group 16986

| Position | Teams ≥25 hits | Team-only MAE | Actual pooled MAE | Frozen gap MAE |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 13 | 0.001048 | 0.01788 | 0.02441 |
| 2 | 15 | 0.0005773 | 0.01223 | 0.01996 |
| 3 | 16 | 0.0008267 | 0.006786 | 0.01068 |
| 4 | 17 | 0.0007245 | 0.004153 | 0.006895 |
| 5 | 17 | 0.0009091 | 0.003126 | 0.005999 |
| 6 | 18 | 0.001015 | 0.004785 | 0.006777 |
| 7 | 19 | 0.001145 | 0.00655 | 0.007081 |
| 8 | 19 | 0.0009124 | 0.005982 | 0.008082 |
| 9 | 20 | 0.001031 | 0.004945 | 0.008825 |
| 10 | 20 | 0.001413 | 0.004925 | 0.006274 |
| 11 | 20 | 0.0008834 | 0.003316 | 0.004228 |
| 12 | 19 | 0.001041 | 0.002275 | 0.003265 |
| 13 | 19 | 0.001109 | 0.001943 | 0.003007 |
| 14 | 19 | 0.0007855 | 0.001987 | 0.003332 |
| 15 | 18 | 0.001157 | 0.002871 | 0.00414 |
| 16 | 18 | 0.0008829 | 0.004208 | 0.005778 |
| 17 | 16 | 0.0009338 | 0.005571 | 0.00869 |
| 18 | 16 | 0.001211 | 0.006877 | 0.01286 |
| 19 | 14 | 0.001504 | 0.01458 | 0.02252 |
| 20 | 12 | 0.001453 | 0.01904 | 0.02816 |

## Interpretation

The actual pooled curve improves on the frozen gap curve in most
groups, but neither is accurate enough to replace team-specific
estimates across the matrix. Pooling actual finishes changes the
conditioning from a particular team's points to any team's points.
For common positions that team-identity mismatch causes sizable bias.
The gap curve adds counterfactual results that do not update opponents'
points in the target team's fixtures. These comparisons are diagnostics,
not calibrated uncertainty intervals or official probability outputs.

For the previously discussed cell (16653, team 125, 16th), the
reference is 0.0000642 (321 hits); team-only is zero in all 20 scouts,
actual pooled is 0.0000771, and frozen gap is 0.0003661. The broader
comparison shows that this useful pooled result is not representative
of every team-position cell.

Reproduce from the saved local runs with
`python3 experiments/rare_positions/compare_point_gap_all_positions.py`.
