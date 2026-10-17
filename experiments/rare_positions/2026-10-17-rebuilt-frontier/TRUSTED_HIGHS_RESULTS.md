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

## HiGHS inside cohort 5: all 156 team-584 assignments

The follow-up audit fixes each of team 584's 156 native W/D/L prefixes in
turn and asks HiGHS for a completion satisfying every cohort points/wins
constraint, including all 15 required-above rivals, four required-below rivals
and Palmeiras's exact target total. Fixed outcomes intersect the native masks;
other fixtures retain their original support. This is an offline audit and
does not change the estimator's proposal or sampling path.

The saved prefix list matches an independent enumeration of the full pivot
incident fixture set using the pivot's packed bounds. Masses are matched by
the actual prefix tuple. The request hash, target, cohort membership, native
fixture indices and home/away orientation are checked before the audit.

| Team 584 results | Assignments | Verified points/wins-feasible | Infeasible | Unknown |
|---|---:|---:|---:|---:|
| 8W / 1D / 1L | 90 | 90 | 0 | 0 |
| 8W / 2D | 45 | 45 | 0 | 0 |
| 9W / 1L | 10 | 10 | 0 | 0 |
| 9W / 1D | 10 | 10 | 0 | 0 |
| 10W | 1 | 1 | 0 | 0 |
| Total | 156 | 156 | 0 | 0 |

All calls completed in 3.960071 seconds, including native model export,
independent coverage enumeration, solver setup and integer witness replay.
Each returned complete W/D/L witness is checked against every native mask,
the fixed prefix and every team's packed bounds. Thus feasibility is checked
independently of the solver's numerical status. This establishes necessary
points/wins feasibility only: no scorelines were constructed or verified, and
no GD, GF or final-ranking reachability claim is made.

The nine-wins/sole-loss-to-team-24 prefix is feasible. Its nominal relaxed mass
is 8.177088483052663e-33, about 61.22% of the full post-GD envelope and 89.03%
of the 9W/1L group's envelope. These remain relaxed masses, not full finishing
probabilities. No nominal or certified-envelope numerator mass can be removed
by this assignment-level points/wins test. The 1.335714790055119e-32 proposal
cap is unchanged; another Monte Carlo comparison would add no pruning effect.

Four new audit tests and all 39 Python tests pass; independent source review
found no remaining implementation blockers. Native estimator sources and
binary are unchanged. Run `audit_highs_assignments.py` with the saved pivot
audit and control result; it accepts request, binary, target, cohort and budget
arguments and retains uncalled/unknown assignments explicitly. The measured
audit used a 180-second shared allowance and 0.25 seconds per solver call.

Local raw artifact: `highs-cohort5-assignments.json`, SHA256
`c3032e32071d0f024a5db8941470040aafabc38345d0f1321cb2368c68aabdbf`.
Source and aggregate prose are backed up; the raw witness artifact stays local.
