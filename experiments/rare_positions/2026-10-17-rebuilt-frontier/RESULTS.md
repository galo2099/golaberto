# Reconstructed Palmeiras 12th estimator

This is a fresh implementation recovered from retained design/review history,
not a restoration of the missing binary. The surviving fixture matches the
historical case's signature and reproduces its 40-cohort structure. Its identity
with the lost request bytes cannot be verified. Fixture SHA256:
`b8d78c18e6ab00b20888dbb36f5f80004879549bd5307fbd8146bdc4c44cccaa`.

All fresh primary runs use one native worker, a shared 180-second wall allowance,
10 seconds reserved for output/cleanup, depth9 private GD integration, initial
k7 joint conditioning with1000 states per candidate, adaptive growth through k11,
and250000 states per candidate. Builds finish before the clock starts. Startup,
setup, sampling, confidence calculations and final output are charged. The target
is a99% simultaneous interval with U/L<=10. Neither the original factor-two
criterion nor a qualified point estimate is implied by a positive lower bound.

| Fresh run | External wall | Draws | Lower | Upper | U/L | Qualified |
|---|---:|---:|---:|---:|---:|---|
| Rebuilt joint core |170.295s|1000096|4.648436909477114e-29|1.3232625740437402e-27|28.467|No|
| First GD cap layer |170.186s|1002096|4.630406876527246e-29|1.124551703936395e-27|24.286|No|
| Final reconstruction |170.290s|552096|4.648201090021483e-29|1.0101828577093011e-27|21.733|No|

All three runs preserve eight exact zero cohorts and sample32 live cohorts. They have
zero uncovered upper mass, no confidence conflicts, no reported errors, and
unchanged measured input/source/binary hashes. The first two runs' lower and upper empirical
score-stream means are approximately `[1.17e-28,2.06e-28]`; this is a pair of
means for two event brackets, not a confidence interval or a single point estimate.

The first GD pass took0.410 seconds in total. It adds deterministic necessary
GD comparison caps and disjoint WDL pivot partitions, including the generic
9W versus nine8W1D split. It preserves proposal B and existing observations.
Its very large reductions from the initial analytic caps include stronger joint
WDL propagation in child atoms; they cannot be attributed to GD alone.
The global upper bound fell about15%, but the result remains unqualified.

The final restoration checks every supported pivot within each deadline and
retains the best completed cap. It also distinguishes an unfinished rejected
candidate from the installed proposal, so further setup can increase that
candidate's state capacity before widening its selected rival set. The final
run installed stronger joint plans (up to10 selected rivals), spent0.280 seconds
on joint preparation and0.455 seconds on GD bounds, and lowered the global
upper bound23.7% against the first rebuilt run. Its empirical bracket means
were `[1.1226024397292e-28,2.0146754576608387e-28]`. Stronger plans cost more per
draw, so total draws fell; this is not evidence that fewer draws alone improve
precision. Three changing implementations give a reconstruction comparison,
not a replicated experiment isolating each mechanism.

The final interval does not meet U/L<=10. The reconstruction is executable and
validated, but the lost implementation's historical precision has not been
reproduced. No qualified point estimate is reported.

Final binary SHA256:
`3960444dac1b9e063070e3cec7911fdc959ad030a990db09cfa936beece9900f`.
Final source aggregate SHA256:
`a33b9c318831083c2ac33a13b9d9111ebb3cc2bb16fcf897bd0c8892811cfd3f`.
Final raw result SHA256 is recorded in the local artifact manifest.

## Evidence and limits

The independent source review is in `INDEPENDENT_SOURCE_REVIEW.md`. The final
source tests are21 Rust tests and15 Python tests. Confidence intervals use exact
binomial Clopper-Pearson constructions and simultaneous allocations across
cohorts, epochs, looks and predeclared thresholds. Their numerical beta inverses
use SciPy double precision with outward guards, not a separately certified
special-function implementation.

This reconstruction does not yet contain the lost score-conditioned regime
mixture or full native GD-equality residual integrator. Its variable-elimination
joint DP replaces the historical explicit separator representation. It supports
the canonical points/wins/GD ranking case measured here; it does not claim all
saved requests have been restored or retested. No production endpoint changed.

Retained historical treatment numbers were `[6.865802419620651e-29,
4.384498731654272e-28]`, U/L6.386 under a shared180-second allowance. They came
from the lost implementation and unverified old request identity. They are a
reference, not a fresh validation result or a timing-equivalent control.
Cohort indices also changed; compare exact above-team sets, not numeric labels.

Full JSON receipts, moment counts and timings are retained locally alongside
this report. The GitHub backup contains source, design/review notes and this
aggregate report. Raw receipt files are not part of the branch backup.
