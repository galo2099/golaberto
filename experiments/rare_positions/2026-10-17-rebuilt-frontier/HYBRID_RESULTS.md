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
