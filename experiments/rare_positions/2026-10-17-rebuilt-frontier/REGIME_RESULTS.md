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
