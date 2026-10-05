# Independent Sol review: R45–R47

Reviewer: `gpt-6.1-sol`; implementation corrections: `gpt-6-luna`.
Scope: four isolated Rust files, their tests, likelihoods, admission limits and
work accounting. The reviewer made no edits and ran no builds or benchmarks.

## Findings and corrections

1. R46 initially allocated dense pair suffix tables before checking the guide
   limit. V3 estimates the same forward/reverse support dimensions first,
   charges the estimation traversal and rejects the pair before allocation
   when it does not fit. The native Pattern remains available.
2. R46 initially selected pairs by rescanning fixtures for every candidate,
   under a linear work charge. V3 gathers pair metadata in one fixture pass,
   caches team uncertainty and charges map traversal and candidate scoring.
   Its multiplication order and lexicographic tie rule preserve the old
   chooser. A separate old-rescan oracle checks equivalence.
3. A combined root-allocation/portfolio retry could clear only root allocation
   after losing. V3 clears both changed structures. This extra experimental
   root flag was absent from the measured default arms.

The reviewer accepted all three implementation corrections. Pair-table reuse
and per-draw kernel invalidation remain consistent with the residual fixtures.

## Likelihood reasoning

R45 evaluates the entire overlapping mixture, retaining half the native
conditional parent. R46's pair count kernel follows the normalized residual
fixture law; remaining independence and half-tie assumptions are guidance
approximations, with actual P/Q correction and production sorting afterward.

R47 sums original fixture probabilities, including outcomes forbidden by a
particular proposal component. If `d_o = Q(z,o)/(P(z)P(o))`, its collapsed
denominator is `sum_o P(o)d_o = Q(z)/P(z)`. Independent conditional score
corrections belong in the corresponding rank numerator. Null draws remain
null atoms. No additional likelihood bias was found in this derivation.

## Validation limits and integration decision

The R47 density test partly checks algebraic cancellation. It is not an
independent enumeration of forward proposal Q, and the small-league tests do
not cover stochastic score-tie integration. Those tests would be required
before promotion. The test-fixture equality failure and its repair are recorded
in the raw test logs, rather than silently removing the failing run.

Full-request coverage, latency and CPU results govern the integration decision.
No rare-cell golden probability is established by this experiment. Retain the
shipped baseline unless a measured arm satisfies the coverage and cost goals.
