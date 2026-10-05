# R55: isolate Flamengo's three-point cases

Status: completed analysis and offline numerical verification, October 3, 2026.
No production source changes, new code, commit, push or database mutation.

## Scope and distinction

Saved group16498 request, team17 (Flamengo), rank12. Flamengo begins with
60 points and18 wins and has10 remaining matches. The exact target-path
distribution in the existing numeric fixture model is:

| Added points | Result patterns | Probability |
|---|---|---:|
|0|Ten losses|1.502e-8|
|1|One draw, nine losses|2.127e-7|
|2|Two draws, eight losses|1.335e-6|
|3|Three draws, seven losses **or** one win, nine losses|5.581e-6|

The three-point total contains two different wins totals:

- Three draws: final63 points,18 wins; prior4.8947426345113675e-6.
- One win: final63 points,19 wins; prior6.861541253954544e-7.

The single rival grouping applies to the second subset. In the certified
partition, nine of the ten possible single-win paths retain one common
strict-below mask: Botafogo (7) and Vitória (67). Six other rivals are already
fixed below by propagation; the remaining eleven must finish at least level
with Flamengo on points/wins and ultimately above it in the actual sorter.
One target win path has no retained rival case. A mask describes a constrained
region containing many fixture assignments, rather than one complete season.

Three-draw paths have337 retained rival cases across120 target patterns and
multiple strict-below masks. These are candidates in the points/wins relaxation;
retained masks alone do not prove their actual rank reachability. The one-win
upper bound below must not be applied to the entire three-point total.

## Exact outcome calculation

The R52 standalone counter has already fully counted the nine single-win cases.
It propagates shared fixtures, multiplies forced-result probabilities once,
integrates independent unary fixtures, and exactly sums the remaining shared
outcomes. Repeating with ten times the per-case/global node limits reproduces
every numerical profile. Five cases have zero mass, and four have positive mass:

| Flamengo's sole win | Joint points/wins-feasible probability |
|---|---:|
|Palmeiras (16)|3.2604104624728206e-39|
|Chapecoense (318)|9.004519105449279e-38|
|Athletico-PR (5)|6.327474013088511e-39|
|Grêmio (79)|4.170994453736957e-38|
| **Total M** | **1.413430200674237e-37** |

These probabilities include Flamengo's target result pattern and every required
rival result. They are not conditional on Flamengo earning the three points.
Conditioned on the one-win target event alone, the summed rival requirements
have probability2.059931068489355e-31; multiplying by6.861541253954544e-7 gives M.

The remaining exact profiles are:

| Rivals strictly above on points/wins | Rivals equal on points/wins | Joint mass |
|---:|---:|---:|
|8|3|1.2073043514499406e-37|
|9|2|2.0612584922429624e-38|

In either profile, **every tied rival must beat Flamengo on later tiebreakers**
to put Flamengo12th. Only Bragantino (225), São Paulo (14), and Cruzeiro (15)
can finish exactly63 points/19 wins in this snapshot. The three-tie profile
therefore ties all three; the aggregate two-tie profile does not identify which
pair without further fixture analysis. Sorting uses goal difference, goals
scored and the production sorter's final tie policy after points and wins.

No resolved profile fixes rank12 before goals. Let E be the fully counted
points/wins event and let alpha=P(rank12 given E). Then:

\[
P(\text{one win and rank12})=M\alpha,\qquad
0\leq P(\text{one win and rank12})\leq1.413430200674237\times10^{-37}.
\]

This is a numerical model upper bound, rather than a rank probability estimate.
Reachability through large winning margins does not establish the likelihood
of those margins. There is no unresolved points/wins search mass in this subset.

## Do goals require simulation?

Simulation is not mathematically necessary. A further deterministic calculation
could sum the conditional score distributions while tracking the target's and
tied rivals' goal differences/goals scored and reproducing the actual final
sorter. Its cost depends on the joint goal state, including fixtures shared by
those teams. The present outcome counter has not performed that calculation.
Independent products of rival goal probabilities would generally be wrong.

In production, `Scores::new` uses finite Poisson mass tables with a1e-15 tail
cutoff and normalized outcome probabilities. "Exact" here means an exhaustive
weighted sum in that existing finite floating-point fixture law, under the
recorded certified-partition completeness assumption. It is not an
arbitrary-precision certificate for unrestricted Poisson score tails. A separate
unbounded-score analysis would need explicit residual-tail accounting; an
arbitrary goal cap cannot prove unrestricted impossibility.

For the current whole-cell estimate around1e-25, M is at most1.413e-12 of that
scale. Even alpha=1 cannot make this case material. We can safely defer goal
work here when the aim is the full cell's order of magnitude. That conclusion
uses a deterministic bound on this subset, not a failed Monte Carlo search.

There is also a model bound independent of the sampled1e-25 scale. Previously
resolved, goal-independent rank12 cases in R52 sum to a whole-cell lower bound
of3.995833147845772e-34. Consequently the one-win subset can contribute at most
`M / lower_bound = 0.000353726`, or **0.03537% of the whole-cell probability**.
Deferring it therefore has a small bounded relative effect even without trusting
the importance-sampling estimate. These remain numerical bounds under the same
partition and fixture-law assumptions.

## Bounds if we simulate this subset

An exact conditional simulator would:

1. Select a feasible target/rival assignment according to its original
   probability **conditioned on E**, using the exact outcome sums.
2. Draw original scores conditional on those outcomes.
3. Run the production sorter and count rank12 successes.

With N independent draws from that conditional law and k successes,
`p_hat=M*k/N`. A binomial confidence interval for alpha scales by M. If k=0,
the one-sided95% upper limit is obtained from `(1-alpha)^N=0.05`:

\[
\alpha_{95}=1-0.05^{1/N},\qquad p_{95}=M\alpha_{95}.
\]

| Conditional samples, assuming zero rank12 successes | Conditional95% upper | Whole-season contribution95% upper |
|---:|---:|---:|
|1,000|2.991e-3|4.228e-40|
|10,000|2.995e-4|4.234e-41|
|100,000|2.996e-5|4.234e-42|

These are prospective confidence bounds; no such score simulation was run.
They require exact independent sampling conditional on E. They do not apply
unchanged to weighted importance samples or to ordinary seasons conditioned
only on Flamengo's point total. M already provides an unconditional model
upper bound regardless of a score-simulation outcome.

## Verification and reproduction

The offline rerun uses four workers,1,000,000 nodes per case and9,000,000 total,
ten times the previous100,000/900,000 limits. It completes all nine cases after
221,565 nodes, with2.001ms preparation and38.032ms counting. Enlarging the limits
does not add work after completion. These are stage timings from one local run,
not CPU measurements or endpoint timings. Aggregates and per-case profiles
agree exactly with the previously archived four-worker result.

```sh
/private/tmp/golaberto-r52/exact_rank_cases-v4 \
  /private/tmp/golaberto-r52/request-16498.json \
  /private/tmp/golaberto-r52/certified-paths.json \
  /Users/robsonaraujo/work/golaberto/experiments/rare_positions/2026-10-03-flamengo-three-points/one-win-10x.json \
  --category 3win --reduction fixed --unary integrate --cutoff-bound unary \
  --nodes-per-case 1000000 --nodes-total 9000000 \
  --memo-entries 100000 --workers 4
```

Use R52's retained source/patch and reproduction instructions to rebuild the
temporary executable when its scratch path no longer exists. Existing R52
exhaustive tests validate the unchanged counter; no new source was written.
Team names were read with a local MySQL SELECT, without changing the snapshot.

- Request SHA256: `2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625`.
- Partition SHA256: `f74075fc29286834d2836738c5219622bf0abd014c5deabee0b1b899f8e17d80`.
- Counter SHA256: `4dfef11060e181179ef0c97d9db9aad054b951132ea0d5de3eeb5a7e432c1d66`.
- Rerun JSON SHA256: `7f35fcd48a09a3f94acec98b930c10be72f4e1c2f53ac444bca7f3a5b6c96ae9`.

Related evidence: `2026-10-02-rust-exact-rank-cases.md` and the R52 raw archive.
Recommendation: close the one-win points/wins calculation and defer its goal
correction; analyze the three-draw and lower-point cases separately.
