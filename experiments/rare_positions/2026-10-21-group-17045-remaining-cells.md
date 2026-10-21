# Group 17045: remaining estimation gaps

This extends the [recovery expansion report](2026-10-21-group-17045-expanded-recovery.md).
The production source was unchanged during this investigation. The actual
`pt,gd,gf,g_away,w,name` request and default estimation settings were retained.
Ten new independent seeds, 813–822, supplement the original 808–812 runs;
each used four workers and the default 20,000-draw importance scout. Positions
below are one-based. Probabilities are fractions, not percentages.

## Frequency and quality

Seven cells were unestimated in at least one of the 15 runs. Every unestimated
cell has public reachability `reachable`; zero is not an impossibility proof.
The first two rows are the persistent problems.

| Team | Position | Unestimated runs | Positive runs meeting precision goal |
| --- | ---: | ---: | ---: |
| 855 | 35 | 9 / 15 | 2 / 15 |
| 1455 | 1 | 9 / 15 | 2 / 15 |
| 1455 | 2 | 2 / 15 | 9 / 15 |
| 855 | 34 | 3 / 15 | 6 / 15 |
| 36 | 35 | 1 / 15 | 11 / 15 |
| 36 | 36 | 2 / 15 | 7 / 15 |
| 855 | 36 | 1 / 15 | 9 / 15 |

Total unestimated counts for seeds 808 through 822 were
`2, 1, 2, 3, 0, 5, 1, 1, 1, 2, 2, 1, 1, 1, 4`.
Seed 812's complete coverage therefore does not imply consistently complete
coverage. These frequencies describe fixed seeded runs, not confidence bounds
on the true probabilities.

When accepted, 855/35 estimates range from 8.39e-20 to 1.76e-19 across six
runs, using native confirmation, expected cohorts, and complete target trees.
Accepted 1455/1 estimates range from 1.00e-16 to 2.60e-16 across six runs,
using native recovery or confirmation. Only two runs for each cell meet the
existing precision goal. Those ranges are empirical consistency evidence,
not replacement estimates or confidence intervals.

## Why 855 finishing 35th is difficult

Team 855 already has nine points and goal difference +9, with five games left.
It needs 34 rivals ahead while keeping at most one rival behind. The certified
target-point limit is 12, but the 16,000-node proof search does not certify a
lower limit. The main difficulty is the joint rival ordering and score
tiebreaks, not merely generating a losing target campaign.

The native proposal and expected cohorts find event hits, but their importance
weights can concentrate on a few outcomes:

- Seed 808's cohort pair has 287/298 hits, but ESS 3.24/18.52. Its main/check
  estimates differ by 11.3 times, so publication fails.
- Seed 810's cohort pair has 362/416 hits, but main ESS 3.47 falls below the
  rough publication minimum of four even though the two estimates agree.
- Seed 812's cohort pair succeeds: 333/301 hits, ESS 12.91/10.27, and estimates
  1.15e-19/1.30e-19.

Complete trees can have the same problem. Seed 808's final main stream has
125 hits but ESS 1.76; its largest event weight contributes 74.3% of the total.
The main stream fails before a check stream is drawn. Seed 810 has 351 main
hits and ESS 1.76, likewise without a check stream. A logged check probability
of zero in these cases means no check was drawn, not an independent zero-hit
experiment.

## Why 1455 finishing first is difficult

Team 1455 has zero points and goal difference -12, with five games left and a
maximum of 15 points. It needs a very favorable combination of its own
results, rival point caps, and score margins. Native/cohort attempts commonly
have only one or two effective samples despite dozens of event hits. For
example, seed 808's title cohort pair has 64/83 hits but ESS 1.11/2.33; seed
810 has ESS 5.22/2.06, so its check stream fails the minimum of three.

The late complete tree exposes a specific discovery/allocation weakness. Its
five-draw-per-branch pilots guide 90% of the residual allocation; a uniform
component and a two-draw floor preserve support but need not discover valuable
rare regions. Mapping logged tree leaves to final draw counts gives:

| Seed | Final draws per stream | All-five-wins branch draws | Draws assigned to 13-point target paths |
| --- | ---: | ---: | ---: |
| 808 | 725 | 4 | 597 |
| 810 | 2,050 | 7 | 1,806 |

The all-wins branch is present, not removed by the team masks or target-path
enumeration. Nevertheless, the final allocation scarcely samples it. Seed
808's tree estimates are 3.41e-33 and 5.82e-32, failing the fivefold main/check
agreement limit. Seed 810's main estimate is 5.42e-31 with ESS 1.16 and no
check stream. These magnitudes are many orders below the successful native
estimates. The logs establish poor exploration and estimator disagreement;
they do not establish a missing support region or identify the true title
probability. Per-target-path contribution diagnostics are the next useful
investigation before trusting this tree for the title cell.

## Scheduling and unsupported fallbacks

These are additional constraints, separate from weight variance:

- The ordinary complete-strata path declines 855/35 with `total case budget
  exhausted` and 1455/1 with `branch allocation floor exceeds draw budget`.
- Late trees run at most four cells. Their candidate ordering prioritizes the
  older 65-plus-root cases before newly admitted smaller root counts. Seed
  811 selects other cells, leaving all three persistent target cells without
  late-tree final pairs. Cohorts also cap their candidate list at four;
  1455/2 is not selected in that seed.
- Residual funding can stop otherwise supported attempts. Seed 808 cannot
  admit 1455/2's tree setup once only 3.64 million units remain, below its
  11 million-unit admission floor. Seed 809 builds 855/35's tree but cannot
  fund complete pilots afterward.
- Exact caps support the score-key prefix now, but require an all-wins
  necessity proof. For 1455 the all-wins cap is 15 and second-best is 13;
  no rival already has more than 13 points. For 855 those values are 24 and
  22, also with no rival already above the latter. The required proof fails
  for the relevant ranks. There are 99 future fixtures, within the 128-fixture
  guard. Raising team, fixture, or work limits does not supply this proof.
- Analytic score integration rejects the actual suffix containing away goals
  and late wins (`core/src/score_integral.rs:384`). Cohorts therefore sample
  scores and their ties, retaining that variance. Packed-witness recovery
  also rejects this suffix (`src/search.rs:1212`). Native full-world
  complement reports `no_saturated_title_or_target_cap`. Supporting these
  paths requires preserving the full sorting semantics.

## Adjacent weak positives

Nonzero cells are not uniformly reliable. At seed 809, 1455/2 retains a
pre-existing pool estimate of 3.48e-11 with ESS zero and relative standard
error approximately one. Other positive runs place it at 1.46e-14–7.44e-14.
This baseline is not evidence that a recovery method confirmed the larger
number. Its weak status must remain visible. Similarly, 36/35 has a roughly
37-fold positive range across runs despite usually being estimated. Independent
seed checks should include precision and method provenance, not only zero
counts.

## Additional cost finding

The broader check found a reproducible initial-stage overrun at seed 817,
which the first five seeds did not exercise. Training charges 384,509,074
units. Final sampling reserves 289,246,702 but actually uses 289,832,499.
Training plus actual final sampling is therefore 674,341,573 against the
674,010,000-unit capacity: 331,573 units, or 0.0492%, over capacity.
A second seed-817 run reproduces the accounting and identical response.

Initial final-batch admission still forecasts draw cost from the pilot's
average operations per draw (`src/rare_tail.rs:647` and
`core/src/budget.rs:139`). This is not an upper bound on fresh draws. The
`admission_valid` diagnostic means complete auditable accounting here; it
does not enforce actual work below capacity. Spare credit saturates at zero,
so the overrun does not generate extra downstream credit. This is separate
from the already conservative late-tree draw forecast and should be closed
before describing initial sampling as strictly bounded for arbitrary seeds.

The 15 runs have no logged late-tree grant violation or cohort overrun.
All probability matrices reconcile: maximum row and column errors are below
7.2e-14 percentage points. The newly observed initial-stage exception must
not be hidden by those successful checks.

## Recommended next work

1. Enforce conservative work admission for initial final batches, and add
   seed 817 to the cost regression coverage.
2. Improve target-path discovery and defensive allocation before increasing
   draw counts. Log per-path contribution, weight concentration, and explicit
   publication rejection reasons; verify against independent native/cohort
   streams. A high hit count alone is insufficient.
3. Improve bounded proposal coverage for rival ordering and score tiebreaks,
   then review four-cell scheduling and residual-bank fairness. Keep complete
   support and independent final/check streams.
4. Evaluate full-rule analytic or packed-witness extensions separately. Do
   not drop away goals, move wins earlier, relax publication gates, or treat
   a witness as a probability estimate.

No production probabilities or code were changed by this analysis.
