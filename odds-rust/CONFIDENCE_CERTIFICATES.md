# Odds confidence certificates

The odds response includes an optional `confidence` object on each
`rare_position_estimates` entry. It bounds the underlying cell probability under
the native finite score sampler. It does not treat the production point estimate,
jackknife error, effective sample size, or matrix balancing as evidence of
coverage. Existing point estimates and their standard errors are retained.

## Supported evidence

- `native_mc`: a conservative fixed-sample binomial interval for the native
  100,000-draw pool scout.
- `matched_own_points_strata`: condition the same native observations on up to
  eight predetermined contiguous bins of the team's own added points. Compute
  each bin's probability from the actual finite-ticket native score law, bound
  its conditional finish probability, and sum all disjoint contributions
  outward. Unobserved bins retain their full possible contribution. Other
  teams' pooled observations and production pseudocounts are excluded.
- `conditioned_fast_is_native_bridge`: fixed conditional importance sampling in
  a necessary points event. A directed dynamic program encloses the event mass.
  Exact finite-ticket kernel probabilities bound conditional-sampler rounding
  error. A likelihood-ratio and conditional-score total-variation bridge converts
  the result to the native sampler's law. Unsupported event construction,
  event mutation, mismatched model identity, incomplete support, or excessive
  setup size suppresses this source. The model must remain fixed after its
  construction, as it does during production requests.

The binomial routine uses conservative Chernoff and Robbins/geometric-tail
inequalities with outward arithmetic. It is not an exact Clopper–Pearson
implementation. The generic bounded-mean helper is available, but the broader
guided, shared-mixture, and partial-cohort importance samplers do not yet emit
method-specific certificates. A cell estimated by one of those samplers can
still have a certificate supported by the sources above.

## Confidence allocation and interpretation

The default family is every team-position cell in one request. The total error
allowance is 0.01: native evidence receives 0.004, own-points evidence receives
0.002, and conditional evidence receives 0.004. Each family share is divided
across cells. Conditional evidence has 1,024 equal predeclared tickets per cell;
a ticket is consumed before sampling and is not reclaimed after rejection.
Own-points evidence further divides its cell allowance across its fixed bins.
The union bound does not require independence between evidence sources.

Thus, under the sampling model, the probability that any reported interval in
the declared family misses its true cell probability is at most 0.01. This is a
frequentist coverage statement over repeated draws. It assumes independent
uniform random tickets, as ordinary simulation confidence calculations do.
It does not describe uncertainty about the underlying football score model.

Set `RUST_ODDS_CERTIFICATE_FAMILY_CELLS` to at least the number of cell-runs in a
larger, predeclared evaluation family. Running indefinitely many requests with
the default does not give 99% simultaneous coverage across all those requests.
Set `RUST_ODDS_CERTIFICATES=0` to skip certificate calculations and omit the field.

## Response fields

`lower` and `upper` enclose the absolute cell probability. Source intervals are
intersected; conflicting evidence is flagged and cannot qualify. An impossibility
proof gives `[0,0]`, unless it contradicts a positive evidence lower bound.
Intervals are attached after production reconciliation, which never rescales
their endpoints.

`interval_qualifies` applies the adopted adaptive precision rule. For positive
endpoints, let `m=(log10(lower)+log10(upper))/2` and
`h=(log10(upper)-log10(lower))/2`. Qualification requires both
`upper-lower <= 0.01` and `h <= 0.05*max(1,-m)`, with conservative numerical
guards. A zero lower bound cannot qualify a positive estimate.
`certified_estimate` is the geometric midpoint when qualified.

`reported_estimate_inside` checks the existing production point estimate against
the interval; it does not certify that estimate merely because another source
qualified. `source_bounds` identifies each contributing method and its interval,
record count, alpha spent, and sample uses. Native and matched sources reuse
the same scout draws, so `sample_uses` and `evidence_samples` must not be read as
counts of independent or unique simulations. `alpha_reserved` also includes
unused claimed conditional tickets, while `alpha_spent` counts accepted records.

## Validation and measurement

Run the service tests with
`cargo test --release --locked --manifest-path odds-rust/Cargo.toml`.
Tests cover finite binomial tails, empty strata, native score-law enumeration,
budget enforcement, evidence conflicts, event-scope guards, and native-law
conversion. The private saved-request comparison runner is
`experiments/odds_certificates/run_benchmark.py`; raw requests and outputs must
remain outside the repository.
