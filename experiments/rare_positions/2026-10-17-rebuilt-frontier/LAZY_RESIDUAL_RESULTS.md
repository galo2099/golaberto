# Lazy guide correction for residual draws

This follow-up optimizes the hybrid sampler's baseline/residual half. The
guided core, clip threshold, joint conditioning, GD score kernels, confidence
allocation and adaptive qualification rule remain the same as in
[HYBRID_RESULTS.md](HYBRID_RESULTS.md).

## Exact optimization

Residual campaigns are now drawn using the existing fast baseline W/D/L
sampler, retaining the actual sampled regime atom/root and fixed scores. After
the conditional score bracket is computed, a score upper bound of exactly zero
makes the entire residual contribution exactly zero. Such a campaign needs no
guide likelihood calculation. It still counts in the sample denominator and
the full baseline comparator stream.

For a positive score upper bound, deterministic replay computes `qg/q0` on the
same sampled W/D/L path. Replay consumes no random numbers, uses actual
integer-ticket probabilities, includes fixed-prefix gains, and follows sticky
fallbacks. The selected atom/root is carried from sampling; it is not inferred
from the outcomes. Residual weighting remains `max(V-H*L,0)*S`.

The previous eager path remains available through `--hybrid-eager-residual`.
Default `--hybrid` uses lazy residual correction. Ordinary sampling is
unaffected. Native counters record residual evaluations and skips, with their
sum equal to the number of baseline/residual draws. Core and baseline elapsed
times are separate; replay time is part of baseline time and must not be added
again. Guide-ratio extrema cover only evaluated ratios.

## Measurement protocol

Compare the eager and lazy hybrid on the same frozen binary and Python source,
serially, using historical279-played/101-remaining Palmeiras16th. Request SHA256:
`cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee`.

Both arms use seed20261017, one worker, a180-second wall budget with one-second
cleanup reserve, depth9, initial k7/1000states, maximum k11/250000states,
1000ms per setup attempt,128 initial physical draws,4000000 maximum physical
draws per cohort and clip fraction1/1024. HiGHS pruning is disabled. Both stop
on the same adaptive qualification rule. All setup, sampling and confidence
work is charged inside the budget; compilation finishes before timing.

The earlier ordinary control qualified in159.231seconds with interval
`[2.664044018339886e-41,1.9487416972334806e-37]`. This is a historical reference,
not a new ordinary-control timing in the present two-arm comparison.

## Validation and frozen implementation

All39 optimized Rust tests and50 Python experiment tests pass. Replay matches
the eager guide ratio for full and partial plans and fixed forest prefixes.
End-to-end tests establish exact equality of the core, residual and baseline
receipts in eager and lazy modes, including an installed forest regime with
tied/strict atoms and conditional score-pair plans. Both zero-score skips and
positive-score evaluations are covered. An independent review found no blockers.

Frozen binary SHA256:
`369eae5388513cb8ed1e56aca99731c2c31dfa9e7111e1e0eb9a1c37f9c52768`.

Measured results and source/raw artifact hashes are recorded below.
