# Reduced fixture simulation — 2026-09-30

## Result and decision

Enable marginalization in the existing domain-constrained rescue samplers.
Across **35 paired requests**, this gained **five nonzero cell-runs across
three distinct cells**, with **zero lost nonzero cell-runs**. The additional
estimates agree reasonably with the independent-stream IS reference.

Whole-request runtime was essentially unchanged: summed elapsed time was
38.261 s without the reduction and 38.160 s with it (**−0.26%**). This is
not evidence of a meaningful whole-request speedup. Some individual
constrained kernels improved by about 4–9%; others became slightly slower.

Keep application to the main directed sampler experimental. Its final
15-pair comparison gained four cell-runs and lost five, and the late
snapshots became about 7–8% slower.

**None of the 11 previously undecided group 16498 cells gained an estimate.**
This change eliminates irrelevant fixtures; it does not implement the new
Grêmio/Internacional disjunction or an exhaustive fixture traversal.

## Implementation

For each cached target-result assignment:

1. Compute unrestricted attainable minimum and maximum ordered scores for
   every rival. The score is points, or points followed by wins when that
   prefix is enabled and allowed by the phase.
2. Mark rivals whose entire interval lies strictly above or strictly below
   the target. Equality is retained for later tiebreakers.
3. Omit a fixture only when **both endpoints are marked**. A fixture against
   an unresolved team remains relevant even if its other team cannot reach
   the target.
4. Combine omissions with the existing forced-fixture compact plan.
   Forced results retain their W/D/L probability. Omitted fixtures integrate
   over every outcome and contribute mass **one**.
5. Keep a representative supported outcome for a complete fixture witness.
   When the standings sorter is needed, use a representative legal score
   for an omitted fixture instead of drawing its goals. Its endpoints cannot
   tie the target's ordered prefix. All relevant scorelines and the full
   standings sorter remain in use.

Plans are cached for at most 64 target assignments. Cache exhaustion uses
the original proposal. Non-points-first phases do not use this reduction.
There are no team, group, or finishing-position identifiers in the logic.

The existing sample budgets, acceptance criteria and JSON contract are
preserved. Work accounting still uses the existing season-equivalent model;
the kernel benchmark separately reports planned omitted fixtures per draw.
This experiment does not redistribute saved CPU time to additional searches.

Removing unnecessary goal draws changes subsequent seeded draws. Therefore
these comparisons are paired by request/root seed, but are not identical
realized simulation paths. Gains and losses describe measured seed outcomes,
not a guarantee that marginalization improves expected discovery rates.

## Full requests: adopted scope

Base estimator: `eae67fce`, plus this change. Four cores (`GOMAXPROCS=4`).
Five saved inputs represent **four distinct groups**, including two snapshots
of group 16653. Inputs are the portable files in
`reference/2026-09-30-hundredfold/inputs/`; their Go-consumed fields match the
original saved requests. These are not a new live DB snapshot.

Seeds: `801`, `804`, `808`, `817`, `911`, `1790746108560577000`,
`1790744713633556000`. Arm order alternates by seed parity. Scout 20,000,
matched pool 100,000, four pool workers; legacy CEM off, existing conditioned
searches and rescues on. Compare reduction `0` against `1`.

| Input | Median baseline ms | Median reduced ms | Summed runtime change | Gained cell-runs | Lost cell-runs |
|---|---:|---:|---:|---:|---:|
| 16498 | 1,392.9 | 1,407.1 | −1.66% | 1 | 0 |
| 16653 current, 85 remaining games | 1,128.0 | 1,122.6 | +0.67% | 4 | 0 |
| 16653 earlier, 90 remaining games | 1,138.7 | 1,121.5 | −1.09% | 0 | 0 |
| 16982 | 912.2 | 913.4 | +0.70% | 0 | 0 |
| 16983 | 913.2 | 916.3 | +0.80% | 0 | 0 |

The two early-phase groups have no baseline zeros, so they do not exercise
this rescue optimization; their timing differences are measurement noise.

### Additional estimates

All probabilities below are fractions, not percentages. The IS reference
requires independent accepted streams; it remains an approximate reference.

| Input / cell | New probability | Reference probability | Gained runs |
|---|---:|---:|---:|
| 16498 / Cruzeiro (15), 20th | 1.517e-18 | 2.005e-18 | 1 |
| 16653 current / team 12, 18th | 3.886e-15 to 4.654e-15 | 4.285e-15 | 2 |
| 16653 current / team 279, 18th | 8.898e-13 to 1.160e-12 | 1.134e-12 | 2 |

Cruzeiro's gain occurred at seed 911. Team 12 gained at seeds 911 and
1790744713633556000; team 279 at 808 and 1790744713633556000.

## Kernel measurements

Thirty thousand requested draws per invocation, five repetitions per arm;
table values are medians. Domain-constrained proposal, rank tilt 6, point
tilt −0.5 for the displayed downward cells. Setup is included.

| Input / cell | Baseline ms | Reduced ms | Change |
|---|---:|---:|---:|
| 16653 current / Fortaleza (22), 18th | 75.01 | 72.19 | −3.76% |
| 16653 earlier / Fortaleza (22), 18th | 75.88 | 72.83 | −4.02% |
| 16653 current / Londrina (125), 10th | 69.61 | 63.21 | −9.19% |
| 16653 earlier / Londrina (125), 10th | 75.68 | 68.91 | −8.94% |
| 16498 / Palmeiras (16), 17th | 27.64 | 28.57 | +3.36% |

Londrina's kernel removes 18 fixtures per requested draw, but has zero hits
in this configuration. Faster execution there does not resolve reachability
or establish a probability. Palmeiras still needs stronger coupled-rival
constraints; omitting a few irrelevant games does not supply them.

## Broader scope: rejected for default use

The first all-stage implementation was tested on 35 pairs and lost six
nonzero cell-runs. After reducing unused bound setup, a final 15-pair test
used the same five inputs and seeds 801, 804, 808:

| Input | Summed runtime change | Gained cell-runs | Lost cell-runs |
|---|---:|---:|---:|
| 16498 | +7.66% | 2 | 4 |
| 16653 current | +6.76% | 1 | 0 |
| 16653 earlier | +8.26% | 1 | 1 |
| 16982 | −1.16% | 0 | 0 |
| 16983 | +0.82% | 0 | 0 |

The losses include Cruzeiro 19th and 20th at seed 808. Cache preparation and
extra proposal checks outweigh the savings for many short pilots. Changed
draws also affect pilot selection and final acceptance. Retain this scope
only as an explicit experimental mode.

## Controls and verification

- Unset or `RARE_POSITION_REDUCED_SIMULATION=1`: adopted domain-rescue scope.
- `RARE_POSITION_REDUCED_SIMULATION=0`: original sampler, for comparisons.
- `RARE_POSITION_REDUCED_SIMULATION=all`: experimental broader scope;
  not recommended for production.
- Requires the existing forced-fixture compaction to be enabled.
- New tests check marginalized mass, supported representative outcomes,
  strict points/wins bounds, equality retention, and agreement with ordinary
  conditional sampling using both phase ordering variants and both sampler
  policies. The pure directed mode adds no rival-domain restrictions.
- Full Go suite: PASS, 3.126 s. Targeted race tests: PASS, 4.845 s.

Reproduce full-request comparisons with
`TestReducedSimulationFullRequestExperiment`, setting
`RARE_POSITION_REDUCED_EXPERIMENT_REQUESTS` to comma-separated saved input
paths, `RARE_POSITION_REDUCED_EXPERIMENT_OUTPUT` to an absolute directory,
and `RARE_POSITION_REDUCED_EXPERIMENT_SEEDS` to the seed list above. Set
`RARE_POSITION_REDUCED_EXPERIMENT_MODE=all` for the broader comparison.
Kernel benchmarks are `BenchmarkReducedSimulation` and
`BenchmarkReducedDirectedSimulation` with the same requests variable,
`-benchtime=1x -count=5`.

Raw request matrices and timing logs are in the ignored local experiment
directories `2026-09-30-reduced-simulation-final` and
`2026-09-30-reduced-simulation-all-final`. The next useful search experiment
is bounded branching on mutually incompatible rival requirements, followed
by propagation and this reduction within each surviving branch.
