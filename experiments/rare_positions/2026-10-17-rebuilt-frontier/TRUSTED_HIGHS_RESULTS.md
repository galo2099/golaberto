# Trusted HiGHS cohort prescreen

The optional `--trusted-highs` experiment discards a whole cohort when
SciPy/HiGHS reports status 2 (infeasible). This explicitly relies on HiGHS
being correct; it does not independently verify an infeasibility certificate.
The production endpoint and default experimental path are unchanged.

The native `feasibility_model` operation exports the actual cohort masks,
packed points/wins bounds, base totals and home/away outcome coefficients.
Each fixture has exactly one binary W/D/L choice. Inclusive rival bounds
retain ties for later GD analysis. Solver feasibility establishes only
necessary points/wins feasibility, not full score-ranked reachability.
Timeouts, errors and all statuses other than 2 retain the cohort. A native
protocol timeout stops further estimation calls while preserving remaining
upper bounds. Pruned rows remain in the output with zero bounds and status
`trusted_highs_infeasible`; original candidate slots and confidence allocation
are preserved. Results that rely on pruning carry
`conditional_on_highs_correct: true`.

## Historical Palmeiras 16th comparison

Both fresh runs use historical `group-16498-two-results.json`, target team 16
at position 16, seed 20261017, one native worker, shared 180-second wall budget,
one-second cleanup reserve, depth 9, initial k7/1000 states, maximum k11/250000
states, 1000 ms setup allowance, 128 discovery draws and 4000000 total draws
per cohort. Both continue sampling for the full budget. Setup and solver
imports are charged inside the budget; compilation finishes before measurement.
The HiGHS arm allows 10 seconds total prescreen time and 0.25 seconds per
cohort, using one HiGHS thread. Neither limit was reached.

| Run | Recorded wall seconds | Draws | Lower | Upper | U/L | Qualified |
|---|---:|---:|---:|---:|---:|---|
| Frozen control | 179.063 | 4599920 | 2.782948583945796e-41 | 1.7685902268417837e-37 | 6355.095 | No |
| Trusted HiGHS | 179.053 | 4724152 | 2.9660965290803897e-41 | 1.7311453326110821e-37 | 5836.443 | No |

These are simultaneous 99% confidence intervals, conditional on HiGHS
correctness for the treatment. No qualified point estimate is reported.
Both runs report unchanged measured hashes and no confidence conflicts.
This is a single-seed comparison on one request; small runtime and interval
changes should not be treated as a general performance guarantee.

## What was removed

HiGHS completed all 16 calls in 0.478564 seconds, reporting six infeasible
cohorts and ten points/wins-feasible cohorts. There were no limits or errors.
Teams 318 and 110 are always below Palmeiras in these cohorts; the table
lists the other two required-below teams.

| Cohort | Other two below Palmeiras | Control status |
|---:|---|---|
| 6 | 20, 8 | Unresolved |
| 7 | 20, 15 | Unresolved |
| 10 | 24, 79 | Already proved impossible |
| 11 | 20, 225 | Unresolved |
| 13 | 20, 74 | Unresolved |
| 15 | 20, 5 | Unresolved |

The five additional discarded cohorts contribute only 5.0922833296427884e-42
to the control upper, about 0.0029% of its total. They consumed about 2.125
seconds of reported native setup and sampling in the control. Earlier pruning
saves this work, but does not directly remove the dominant uncertainty.

Cohort 5 is identical in both runs: 4000000 draws, interval
`[3.834775699752718e-45, 1.5645417326444608e-37]`, and the same nine-rival,
team-584 GD proposal with cap `1.335714790055119e-32`. It supplies 90.38%
of the treatment upper. This experiment does not use HiGHS to prune individual
W/D/L prefixes within that reachable cohort. Such finer pruning would be a
separate experiment.

## Validation and provenance

31 optimized Rust tests and 35 Python tests pass. Tests cover shared-fixture
infeasibility, feasible ties, home/away masks, retaining non-infeasible solver
statuses, and an all-pruned driver flow preserving original slots and exact
zero totals. Independent review found no remaining model or driver blockers.

- SciPy: 1.17.0; bundled HiGHS: 1.8.0.
- Request SHA256: `cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee`.
- Control binary SHA256: `637b7e7726fe14e1520cae06b598141215b2111c9bbaac876c5bb030d2cf4aa1`.
- Treatment binary SHA256: `71f5e224ece3ce04fe80ab5c558198fd165b262f1619793ebdc9af471e736a75`.
- Treatment source aggregate: `9cd2fba8366e11edcc5912271505c908ec3a685547d10a69099ce1d0f955dd19`.
- Local control artifact: `trusted-highs-control-pal16-180s.json`, SHA256 `eaf6aa097174f53ef0a4af738639fa1add6684439bcb6f7cd7a5e85c72869c4f`.
- Local treatment artifact: `trusted-highs-pal16-180s.json`, SHA256 `399bc12e110464ca5ad6649590242c175d5975c18d7ab3cd41cafb1641075405`.

Raw experiment JSON remains local. Only source and aggregate prose are included
in the recovery branch backup.
