# Guided core and certified residual experiment

This optional experiment keeps the existing joint points/wins proposal,
regime atom probabilities, fixed-prefix assignments and conditional GD score
kernels. It adds sequential W/D/L guidance on the same proposal graph. It does
not replace a joint constraint with independent rival probability estimates.
Production defaults are outside this experiment.

## Probability decomposition

Let `q0` denote the installed baseline proposal, `qg` its guided version,
`V=p/q0` the baseline importance weight, and `L=qg/q0`. Here `p` denotes the
native model measure represented by the installed proposal, including its
existing prefix and score-conditioning factors. Let `S` denote the remaining
conditional score/ranking event probability. With a fixed positive cap `H`:

```text
core     = E_qg[min(V/L, H) * S]
residual = E_q0[max(V - H*L, 0) * S]
core + residual = probability of the exact cohort event
```

Clipping applies to the likelihood before multiplying by the score probability.
The residual accounts for every contribution removed by clipping. It is an
overflow expectation, rather than just the proposal probability of overflow.
No contribution is discarded based on the largest observed weight.

The core observation is bounded by `H`. The residual observation is bounded by
the existing certified baseline cap `B`. This first experiment therefore retains
the baseline's conservative residual cap; it does not claim to have solved the
structural high-weight-tail problem.

The clip cap is fixed within each proposal epoch as `H=B/1024`. Proposal growth
or an accepted regime rebuild starts a fresh epoch, while retaining previously
established full-event bounds. There is no pilot-based choice of the clip cap.

## Guidance and confidence

Guidance uses remaining packed points/wins distributions for fixture endpoints
outside the jointly selected rival set. These marginal probabilities are a
proposal heuristic. Shared fixtures continue to have one actual W/D/L outcome,
and likelihood corrections use the actual integer-ticket categorical laws.
Positive defensive weights retain the baseline support. Fixed forest prefixes
are included in current team totals and excluded from remaining suffix tables.

Each hybrid batch spends half its physical draws on the guided core and half
on the baseline residual. The latter draws also produce full baseline event
observations without an additional draw. Conditional score integration is
reused for the residual and baseline observations on the same campaign.

Confidence allocation is divided across three streams: core, residual and full
baseline. Their dependence is allowed by the union bound. The sum of the core
and residual intervals is intersected with the baseline interval and retained
analytic/full-event bounds. Confidence conflicts prevent qualification.
Existing simultaneous99% allocation and the adaptive qualification policy are
preserved. `--max-draws` counts physical draws, not paired iterations.

This initial implementation guides W/D/L outcomes and keeps the existing score
integrator; it does not restore the old goal-tilt sampler. Any measured benefit
or regression therefore concerns this specified hybrid increment.

## Comparison protocol

The comparison uses the historical279-played/101-remaining-match input,
Palmeiras team16 at position16. Request SHA256:
`cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee`.

Both arms use the same frozen native binary and Python sources, one worker,
seed20261017, a180-second total wall budget with one-second cleanup reserve,
depth9, initial k7/1000states, maximum k11/250000states,1000ms per setup attempt,
128 initial physical draws and4000000 maximum physical draws per cohort.
HiGHS pruning is disabled. Setup, failed attempts, guidance construction,
sampling and confidence calculations are charged inside the shared budget.
Compilation completes before measurement and timed jobs run serially.

The control uses ordinary sampling and the treatment adds `--hybrid` with
clip fraction1/1024. Both use the adopted adaptive qualification rule and stop
when it is met. Raw result JSON stays local; hashes and aggregate results are
recorded below after validation and measurement.

## Validation and frozen implementation

All37 optimized standalone Rust tests and49 Python tests pass. New checks cover
actual categorical likelihood ratios, preserved baseline paths and fixed
prefixes, core/residual decomposition, outward normalization at confidence
thresholds, full regime-adjusted cap validation, transactional three-stream
receipts and physical draw accounting. Independent native and statistical
reviews found no remaining blockers before measurement.

Sparse guide PMFs accumulate in deterministic gain order. Cached suffix rows
are shared across a forest's roots, but this first implementation rebuilds each
requested team/suffix PMF independently and scans its values for endpoint
probabilities. It does not restore the old sampler's optimized suffix-CDF lookup
architecture. Guide work is included in sampling time; its separate CPU cost
has not been profiled.

The12-second integration smoke finished in11.026seconds with31048 physical
draws, no errors/conflicts and unchanged hashes. In cohort5's final proposal
epoch, the guided core had206 nonzero contributions in3000 draws, versus none
in3000 baseline draws. Nonzero integrated contributions are not ordinary
Bernoulli hit counts, and this observation does not establish a qualified
estimate.

Frozen native binary SHA256:
`f3c4a36feb551ff9492dbd63e340edb3c5b059f96dbb518fae1c240c79b43fd1`.
Source aggregate SHA256:
`6f9fda9fb876639b7db2b431623b4c07af8a640b17b056d9f658e1378f895c03`.
The source implementation was backed up before the timed comparison in commit
`84f94cd0377b71a71d7ca8c51e35b9057875fa2a`.

## Measured comparison

| Arm | Recorded seconds | Physical draws | Simultaneous99% lower | Simultaneous99% upper | Exponent halfwidth / allowance | Adaptive qualification |
|---|---:|---:|---:|---:|---:|---|
|Ordinary control|159.231|4163048|2.664044018339886e-41|1.9487416972334806e-37|1.932106 /1.932118|Yes|
|Hybrid|179.084|1565048|2.2813552301789618e-41|1.0588593502036845e-36|2.333323 /1.915424|No|

The control stopped on qualification. The hybrid exhausted its usable budget
and did not qualify. Both runs report unchanged source/input/binary hashes,
no confidence conflicts, no sample errors, and no reliance on HiGHS. This is
one seed and one request, so it does not establish full-test-set coverage.
It is a regression for this measured case and is not adopted as the default.

The hybrid upper is5.43 times the control upper, while its lower is0.856 times
the control lower. Average native sampling work, including guidance and score
integration, was94.55microseconds per physical draw versus30.22microseconds
for the control, a3.13-fold increase. The hybrid completed2.66 times fewer
physical draws despite running longer. This timing does not isolate guidance,
additional useful score integration, or confidence-controller costs separately.

### Cohort5: stronger lower, unresolved residual upper

The hybrid's final cohort5 epoch has696500 guided core draws and696500 baseline
draws reused for the residual. There are46408 nonzero core contributions,
four nonzero baseline contributions and no nonzero residual contributions.
These are weighted conditional-score contributions, not ordinary final-rank
hit counts.

| Final-epoch stream | Certified lower | Certified upper | Empirical lower-score mean | Empirical upper-score mean |
|---|---:|---:|---:|---:|
|Core|1.2752209842024926e-41|1.2646138393859655e-39|6.008035207128631e-41|1.534926068423025e-40|
|Residual|0|8.769025355532755e-37|0|0|
|Baseline|6.240199850077303e-53|8.769118021047364e-37|5.396501239164411e-45|2.1872297988322403e-42|

The empirical columns are diagnostic means, not confidence bounds. The current
stream bounds use the existing cohort/epoch/look allocation, divided by three
for the streams, then by482 for the one-sided threshold bounds.

The retained full-cohort lower improved from2.3697181152131404e-45 in the
control to1.280702091337918e-41 in the hybrid. Guidance therefore supplied a
useful lower bound. However, the residual still has cap1.335714790055119e-32.
With zero observed contributions, the confidence upper must retain an unseen
tail allowance. The baseline comparator supplies the slightly smaller upper
than the sum of core and residual intervals. Cohort5 contributes82.82% of the
hybrid's aggregate upper.

### Cohort9: many small contributions do not cover its main mass

The final hybrid epoch has26500 draws per stream. The core has19785 nonzero
contributions, but its empirical score means are only
`[3.247272074740668e-42,8.071995292063857e-42]`. The residual empirical means
are `[7.119985815588563e-41,1.9365292126867346e-40]`. The fixed clip threshold
therefore leaves most of the observed contribution in the residual stream.
These are comparisons of diagnostic streams under different proposals, not
an exact probability decomposition obtained from their finite sample means.

The control's final epoch has232000 baseline draws for this cohort, versus
26500 in the hybrid. Its retained lower falls from2.663699274038179e-41 to
9.945087047640286e-42. This loss offsets cohort5's lower-bound gain; the global
lower is slightly worse even though the guided core finds many contributions.

## Interpretation and next target

The hybrid identity and confidence accounting work, and W/D/L guidance finds
useful schedules. The first implementation does not solve the bound on the
overflow expectation. Allocating half the physical draws to the core while
increasing per-draw cost leaves far fewer observations to certify that residual.

The next computational improvement is to draw residual campaigns through the
existing fast baseline path, calculate their score bracket, and evaluate the
guide likelihood only when the score upper is positive. Skipping it when the
score upper is exactly zero preserves the estimator. This requires retaining
the actual sampled regime atom for a subsequent deterministic path replay;
reconstructing an atom from its outcomes alone is insufficient.

Further improvements need residual-specific structural envelopes or a better
proposal/clip choice for cohorts such as9. Additional ordinary hits cannot
justify dropping the unseen-tail allowance. No such follow-up is included in
this measured result.

## Local raw artifacts

Only source and aggregate prose are backed up. Raw JSON stays local:

```text
hybrid-control-pal16-180s.json  4d62ab4024ee5679171996c9178ff379976189f8357b1689cad73de7b711a9c7
hybrid-pal16-180s.json          ea058f61acffb9116d15a063c31906c517222bf4b2091fad3a337534cc5d4c0a
```
