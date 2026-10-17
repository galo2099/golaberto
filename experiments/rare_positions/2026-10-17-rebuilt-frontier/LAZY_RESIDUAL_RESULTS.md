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

The12-second smoke finished in11.142seconds with28048 physical draws and no
errors/confidence conflicts. All14024 residual campaigns had exactly zero score
upper bounds, so no residual guide replay was needed. Core sampling took1.615
seconds and baseline/residual sampling0.536seconds; setup consumed most of this
short run. This smoke establishes integration behavior, not a latency or
qualification result.

Frozen source aggregate SHA256:
`42b70933ff6d41728ee3638786fcc78ea7dce311bbf523f878820b4bb3816b57`.
Implementation backup commit:
`87667bce6880c55178490a1291cdea754ac6be99`.

After timing, fixed-proposal checks on the actual fixture reproduced the final
cohort5 forest/GD proposal and cohort9 joint proposal, matching their benchmark
caps. For20000 physical draws per arm per cohort at the same seed, all three
core/residual/baseline receipts matched exactly between eager and lazy modes.
Cohort5 skipped10000 residual guide evaluations; cohort9 evaluated239 and
skipped9761. These validation draws are excluded from the benchmark intervals
and timings below. Thus the parity check covers both zero and positive score
upper bounds on the actual request, as well as the unit-test models.

## Matched180-second comparison

| Hybrid mode | Recorded seconds | Physical draws | Simultaneous99% lower | Simultaneous99% upper | Exponent halfwidth / allowance | Qualified |
|---|---:|---:|---:|---:|---:|---|
|Eager residual|179.069|1543048|2.2692850827270483e-41|1.0723681167162897e-36|2.337227 /1.915344|No|
|Lazy residual|179.026|1951048|2.589898304031079e-41|8.60297386511054e-37|2.260683 /1.916302|No|

Both arms reached their usable wall budget and neither qualified. Both report
unchanged request/source/binary hashes, no confidence conflicts, no sample
errors and no HiGHS dependence. This is one matched seed/request comparison;
the adaptive scheduler allocates different amounts of work to its cohorts.

Lazy evaluation completed26.44% more physical draws. Its upper endpoint was
19.78% lower and its lower endpoint14.13% higher. Native sampling time per
physical draw fell from95.96microseconds to73.59microseconds, a23.32% reduction.
These endpoint changes are observed outcomes under adaptive allocation, not
fixed-sample precision guarantees.

## Where time was saved

| Native phase/counter | Eager | Lazy |
|---|---:|---:|
|Guided core elapsed seconds|90.608|111.873|
|Baseline/residual elapsed seconds|57.230|31.427|
|Baseline/residual campaigns|771524|975524|
|Residual guide evaluations|771524|868|
|Residual guide evaluations skipped|0|974656|
|Actual lazy replay seconds|0|0.0511|

Lazy mode skipped99.911% of residual guide evaluations. Baseline/residual work
per campaign fell from74.18microseconds to32.22microseconds. Guided core time
increased because the run completed more core draws. It now accounts for78.07%
of measured core-plus-baseline phase time. Replay time is already included in
baseline time, and native receipt overhead is outside these two phase timers.

## Remaining statistical bottleneck

Cohort5 still dominates the upper endpoint. Its final lazy epoch has875000
draws in each stream. The guided core has58531 nonzero contributions, while
the baseline comparator has five and the residual none. Its final-epoch core
interval is `[1.4410141183323697e-41,1.1463390418238275e-39]`, but its residual
upper remains7.049822405827536e-37 under the unchanged cap1.335714790055119e-32.
Nonzero contributions are integrated weighted-score observations, not ordinary
Bernoulli final-position hits.

The optimization therefore saves real computation and tightens the final
interval, while leaving the central unseen-tail problem intact. Retaining it
inside the optional hybrid is justified as an exact computational improvement;
this result does not justify replacing ordinary sampling with the hybrid.
The historical ordinary control still qualified where these hybrid arms did
not, though it was not rerun in this two-arm panel.

## Local raw artifacts

Raw JSON remains local. The source and this aggregate report are backed up.

```text
lazy-hybrid-smoke-pal16.json       056e126cc2c9f6b216728e4132890a0871f3e105c119a6fe7710c87e133c308d
lazy-paired-eager-pal16-180s.json   b27998d16b49ca7f468f43226eb4eae5f929ef6e4388d190f14bcf1bd05cb27c
lazy-paired-pal16-180s.json         6e6434b9e89217c70eb6e70a74371e9f5981370a20767204591f6905fe983573
lazy-fixed-final-plan-parity.json   99cbceafed3a52011b3e95dc1a52a832e357e3309a3481215787b6e9dd0f3f4a
```
