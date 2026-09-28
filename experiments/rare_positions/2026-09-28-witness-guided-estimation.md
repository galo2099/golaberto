# Witness-guided estimates for reachable zero cells

## Question

Can the complete fixture assignments already found for zero-probability cells
guide importance sampling well enough to replace zero with a useful estimate?
The eight reference cells are Avaí 2nd/3rd, team 11 19th, Londrina 3rd,
team 125 10th (group 16653), and teams 20 2nd, 110 3rd, and 318 3rd
(group 16498).

## Inputs and checks

I used the saved group 16653 request from 2026-09-27, the saved group
16498 fixture request, seed 808, and four Go cores. Every starting
win/draw/loss vector was checked by `canonicalReachabilityRank`. The
five group 16653 vectors were captured from the production neighborhood
and joint-point witness searches. The three group 16498 vectors came
from the joint-point witness search. Each chosen fixture outcome has
positive probability under the match model.

I tried four related proposals:

1. Fix the target's remaining outcomes and then increasingly many
   rank-sensitive fixture outcomes from its witness. Draw all other
   outcomes from the original match model.
2. Add the existing rank-directed lookahead to this fixed-outcome event.
   The event mass is calculated exactly; the lookahead likelihood ratio
   corrects its changed outcome probabilities.
3. Condition on one exact target point total and strengthen the
   lookahead tilt. This replaces a full witness pattern with a larger
   points event.
4. Use the root fixture domains from the joint point-cap witness solver.
   Sample each remaining outcome inside its allowed domain, then apply
   lookahead with the exact domain-mass and `P/Q` corrections. In the
   hardest cases, additionally fix selected witness outcomes.

Each resulting number estimates **P(target rank AND the chosen
conditioning event)**. It is an unbiased estimate of that subevent
under the Poisson match model. It is *not* a full-cell estimate unless
the proposal also covers the complement. A successful weighted sample
therefore supplies a positive contribution, not permission to replace
the production zero with the displayed number.

## Results

The following are representative independent 20,000-draw guided runs.
`ESS` is the effective sample size of event-weighted hits. The values
are probabilities, not percentages.

| Group | Team | Finish | Conditioning event | Rank hits | Estimated subevent probability | ESS |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| 16653 | 68 | 2nd | Exactly 57 points, rank tilt 3 | 1,499 | `1.83e-14` | 416 |
| 16653 | 68 | 3rd | Exactly 57 points, rank tilt 3 | 2,696 | `2.90e-11` | 1,565 |
| 16653 | 11 | 19th | Exactly 42 points, rank tilt 8 | 1,042 | `1.12e-11` | 809 |
| 16653 | 95 | 3rd | Point-cap domains plus 24 locked fixtures, tilt 8 | 421 | `2.23e-36` | 254 |
| 16653 | 125 | 10th | Point-cap domains, tilt 8 | 842 | `1.34e-24` | 724 |
| 16498 | 20 | 2nd | Point-cap domains, tilt 4 | 1,117 | `2.28e-16` | 949 |
| 16498 | 110 | 3rd | Exactly 53 points, rank tilt 4 | 936 | `1.23e-15` | 655 |
| 16498 | 318 | 3rd | Point-cap domains plus 24 locked fixtures, tilt 8 | 348 | `2.75e-29` | 245 |

The 57-point Avaí runs are notably different from the default
lookahead pilot, which saw zero 2nd/3rd-place hits in 200 draws each.
For Avaí 2nd, tilt 1 with a three-rival points event found only 4
hits in 20,000 draws; tilt 4 found 1,880 with a similar weighted
probability (`1.60e-14`). For Avaí 3rd at 57 points, tilts 3 and 4
gave `2.90e-11` and `2.82e-11`, respectively. For team 11 19th at
42 points, tilts 8, 12, and 16 gave `1.12e-11`, `1.12e-11`, and
`1.09e-11`.

The two rows requiring extra locked fixtures are **not useful full-cell
odds**. For Londrina 3rd, changing the number of extra locks from 24
to 32 changed the measured subevent from `2.23e-36` to `8.67e-37`;
for team 318 3rd, from `2.75e-29` to `1.57e-31`. The shrinking event
mass dominates the improved hit rate. A stable ESS only says that the
specified narrow subevent was measured reliably.

Fixing witness outcomes without lookahead was ineffective. Avaí 2nd
needed 48 locked fixtures before 19 of 10,000 draws hit, at event mass
`4.30e-26`. Avaí 3rd first hit at 32 locks, with just 6 of 10,000
draws. Directly projecting witness outcomes into Poisson means had
already produced zero hits in earlier 100,000-draw validations. Greedy
single-fixture witness optimization increased complete-vector
likelihood by factors up to `3.70e12`, but did not solve the narrow
coverage problem.

## Cost and decision

The 20,000-draw guided batches in this local test generally took
roughly 60–110 ms each; point-event construction ranged from under a
millisecond to hundreds of milliseconds when the dynamic program
reached its state limit. Eight additional batches would likely exceed
the current roughly 0.95-second four-core group 16653 request budget
unless they replace existing extra-search allocations and run in
parallel. This was an exploratory test, not a full-request benchmark.

The result supports rank tilt and exact point conditioning as useful
proposal components. It does not support putting the eight subevent
numbers into the production odds matrix. The next complete estimator
should partition all feasible target point totals, use multiple
joint-domain or witness components where needed, and calculate exact
mixture likelihoods. It should promote a cell only after held-out
sampling gives sufficient ESS and the uncovered component is measured
or bounded tightly enough for the requested order of magnitude.
