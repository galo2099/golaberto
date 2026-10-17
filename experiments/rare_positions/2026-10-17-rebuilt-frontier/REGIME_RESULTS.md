# Conditional regime proposal recovery

The retained historical result is U/L6.386, with interval
`[6.865802419620651e-29,4.384498731654272e-28]` under one shared180-second
allowance. The earlier reconstructed U/L21.733 did not reproduce that result.
This stage restores actual score-conditioned proposals and remaining-score
integration before reassessing the scheduling policy.

## Restored proposal

A generic required-above rival partitions compatible WDL patterns into strict
and tied points/wins atoms. Each atom gets a fresh joint WDL plan retaining all
selected parent constraints. A tied atom samples every fixture incident to the
pivot or target from the native Poisson law conditional on WDL and rejects the
whole vector unless the necessary inclusive GD comparison holds. Its native
normalizer q enters the likelihood exactly once. Fixed goals are honored at
all affected endpoints, with no resampling or repeated private integration.
Strict atoms use ordinary scores. Exact integer-ticket mixture probabilities
enter both likelihood correction and the certified cap. Unsupported positive
atoms reject the whole candidate; only certified zeros may be omitted.

This recovery uses rejection, rather than the lost bridge sampler. Positive
q.lo<0.001 fails the preparation cost gate. The full native GD-equality/GF
residual integrator remains missing; strict/inclusive score brackets remain.
The fixture is the signature-matching surviving Palmeiras case documented in
README.md; original request byte identity remains unverified.

Validation before the first measured run:26 Rust tests,17 Python tests and
independent source review passed. Tests cover native conditioning, shared signs,
fixed-score integration against exhaustive score enumeration, nonuniform atom
mixture likelihood/support, successful installation, fresh epoch handling,
rejected proposal preservation and raw-plus-atom state accounting.

## Fresh measurements

Every run is one shared budget for40 cohorts, one native worker,99% simultaneous
confidence, initial k7/1000-state plans, adaptive growth through k11 and250000
states, depth9,128 discovery draws per live cohort. Builds precede the clock.
The conditional runs use180 seconds, one second reserved for output, and keep
sampling after first qualification. The prior reconstruction reserved10 seconds.

| Run | External wall | Draws | Lower | Upper | U/L | Qualified |
|---|---:|---:|---:|---:|---:|---|
| Prior reconstruction |170.290s|552096|4.648201090021483e-29|1.0101828577093011e-27|21.733|No|
| Conditional proposal, largest-width scheduler |179.166s|2277096|4.384224046031559e-29|6.1417542609684205e-28|14.009|No|
| Conditional proposal, cost-aware allocation and full setup |179.152s|1768096|6.147750115263105e-29|3.1342415010044333e-28|5.098|Yes|

The largest-width run installed eight regime proposals, rejected one, and
skipped23 cohorts as initially negligible. It took0.121 seconds for regime
setup and0.467 seconds for deterministic GD bounds. All32 live cohorts were
sampled and eight were proved impossible. There were no errors or confidence
conflicts, and all measured source/input/binary hashes were unchanged.

The run still missed both the target10 and historical6.386. It spent166.7 seconds
and2230128 draws on cohort23. Its width was8.06e-29, of which the empirical
score-bracket/dyadic gap was6.80e-29. This gap is a scheduling diagnostic, not a
certified lower limit on achievable interval width. Other major cohorts had
similar current width with much larger estimated sampling uncertainty but got
little time. This exposed the missing cost-aware frontier allocation behavior.

The next scheduling restoration estimates the part of each width attributable
to sampling, forecasts its reduction per unit cost, and uses that only to choose
the next action. All actual confidence bounds, observations and confidence
allocations remain unchanged. It also attempts initial regime setup for every
nonzero cohort within the existing20-second setup allowance, rather than making
a permanent early negligible-width decision.

Full receipts and timings stay local. Source and this aggregate report are
backed up to the recovery branch; raw artifact hashes are in LOCAL_ARTIFACTS.md.

## Recovered precision

The final run qualifies with U/L5.0981927408256364, improving on the retained
historical6.386 reference for this surviving signature-matching fixture. Its
interval midpoint estimate is `1.3881114346140374e-28` (geometric midpoint of
the confidence interval, not an unbiased empirical mean). The99% simultaneous
interval is `[6.147750115263105e-29,3.1342415010044333e-28]`.

External wall time179.151939541 seconds includes startup, setup, sampling and
output and stays below180 seconds. It completed1768096 draws, installed31
regime proposals, retained one ordinary joint proposal, proved eight cohorts
impossible, and sampled all32 live cohorts. Regime setup took0.256 seconds;
deterministic GD caps took0.479 seconds. No errors, confidence conflicts or
uncovered mass were reported, and all frozen hashes were unchanged.

The busiest cohort23 received54.6 seconds instead of166.7. Cohort24 received
34.7 seconds, cohort19 received22.9 seconds, cohort13 received16.7 seconds, and
cohort3 received17.5 seconds. The global upper bound fell49.0% and the lower
bound rose40.2% relative to the largest-width conditional run. Total draws fell,
while more useful observations tightened the aggregate interval.

Both cost-aware sampling and proposal setup coverage changed in this comparison.
It measures their combined treatment and does not isolate their individual
effects. This is one fresh seed and one fixture, not a regression suite across
all saved requests. Full native GD-equality integration and the old bridge
sampler remain missing; they were not required for this measured result.

Final validation:26 Rust tests and20 Python tests passed. Statistical/numerical
contracts and the independent review are documented in CONDITIONAL_GD_REVIEW.md.
The standard SciPy floating-point beta-inverse assumption still applies.

Binary SHA256:
`61ec2e29f5c28d035251e9e7a2050dc8b2649a760886ae89fe128d4eb99d0b13`.
Source aggregate SHA256:
`62383b24cd078c08d5a08bb61f7a3f1c832dde983329f0c192a89a0edf3d462a`.
Request SHA256:
`b8d78c18e6ab00b20888dbb36f5f80004879549bd5307fbd8146bdc4c44cccaa`.
