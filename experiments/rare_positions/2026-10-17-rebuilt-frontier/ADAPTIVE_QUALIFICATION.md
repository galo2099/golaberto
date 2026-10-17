# Adaptive interval qualification

The experimental frontier estimator now accepts wider multiplicative intervals
for smaller probabilities. The confidence level remains simultaneous99%, with
the existing allocation across cells, cohorts, proposal epochs and sampling
looks. Neither observations nor confidence endpoints change merely because the
qualification policy changes. The production endpoint is outside this change.

## Adopted rule

For a finite probability interval with `0 < L <= U <= 1`, define:

```text
log_midpoint = (log10(L) + log10(U)) / 2
exponent = -log_midpoint
exponent_halfwidth = (log10(U) - log10(L)) / 2
allowed_exponent_halfwidth = 0.05 * max(1, exponent)
```

Qualification requires both interval width at most0.01 and exponent halfwidth
at most the allowance, with no confidence conflict. A width of0.01 corresponds
to at most0.5 percentage points on either side of the arithmetic midpoint. At
small probabilities, the exponent condition dominates instead. The geometric
midpoint, used as the reported estimate, is computed from logarithms so that
the product of tiny endpoints cannot underflow.

| Probability scale | Illustrative interval | Upper/lower ratio |
|---|---|---:|
|0.5|[0.495,0.505]|1.02|
|1e-2|[7.94e-3,1.26e-2]|1.58|
|1e-6|[5.01e-7,2.00e-6]|3.98|
|1e-12|[2.51e-13,3.98e-12]|15.85|
|1e-20|[1e-21,1e-19]|100|
|1e-40|[1e-42,1e-38]|10000|

The displayed examples are rounded. At1e-40 this policy permits approximately
two decades on either side, substantially wider than the former total ratio10.
The interval must remain visible alongside its estimate.

An interval with `L=0<U` never qualifies as a positive estimate, even if its
absolute width is small. Exact zero still requires a proof. The new policy is
used consistently for early stopping and final qualification; `--run-full-budget`
continues sampling after qualification for controlled comparisons. Existing
confidence conflicts prevent positive qualification.

## Ordering

Cell A is established below cell B when `upper(A) < lower(B)`. Overlapping
intervals do not establish an ordering. Qualification alone does not promise a
complete ordering, and this change does not introduce an ordering-based
scheduler. It changes stopping and result classification only.

## Saved-run reassessment

The following analysis applies the new rule to existing confidence intervals.
It introduces no additional Monte Carlo observations and does not measure when
an adaptive run would first stop. The underlying historical source, input and
binary hashes remain recorded in the original reports and local receipts.

| Saved result | Previous qualification | Adaptive qualification | Exponent halfwidth | Allowed |
|---|---|---|---:|---:|
|primary-pal12-180s.json|No|Yes|0.7272|1.3803|
|gd-caps-pal12-180s.json|No|Yes|0.6927|1.3821|
|final-pal12-180s.json|No|Yes|0.6686|1.3832|
|regime-pal12-180s.json|No|Yes|0.5732|1.3892|
|regime-cost-pal12-180s.json|Yes|Yes|0.3537|1.3929|
|regime-cost-pal16-285played-180s.json|No|Yes|0.5862|2.0820|
|regime-cost-pal16-historical-180s.json|No|No|2.2327|1.9075|
|compact-pal16-historical-180s.json|No|No|2.2226|1.9075|
|gd-rebuild-pal16-historical-180s.json|No|Yes|1.8717|1.9318|
|trusted-highs-control-pal16-180s.json|No|Yes|1.9016|1.9327|
|trusted-highs-pal16-180s.json|No|Yes|1.8831|1.9322|
|smoke-pal12.json|No|No|12.8623|1.2234|
|smoke-v2-pal12.json|No|No|13.2221|1.1785|

Nine of11 saved long-run results now qualify, compared with one previously.
Including the two short smoke runs gives nine of13, with eight newly qualified
and no lost qualifications. These are repeated experiments on three target/input
combinations, not13 distinct cells and not a full-test-set coverage benchmark.
The later285-played Palmeiras16th input differs from the historical279-played
input; their probabilities must not be compared as if they were one fixture.

The latest control without trusted HiGHS pruning retains exactly its original
interval `[2.782948583945796e-41,1.7685902268417837e-37]`. Its geometric midpoint
is about `2.21853457655492e-39`. Qualification means this coarse interval meets
the new precision target; it does not validate that midpoint as an accurate
reference probability. The HiGHS treatment remains conditional on solver
correctness, as documented in TRUSTED_HIGHS_RESULTS.md.

The older compact and regime-cost historical Palmeiras16th runs still fail,
showing that the new rule does not accept every nonzero interval. Neither short
smoke run qualifies despite its tiny absolute width.

## Local result provenance

Raw JSON remains local and unchanged. The reassessment used these SHA256 values:

```text
primary-pal12-180s.json                 1657f07c19d3f8044979f65851039ea36dc39bd62c536a242fbd5a5f400ce90c
gd-caps-pal12-180s.json                 5def450447703cb96ae1eaf5e4a1432af838fdaa44ee78cb5a42c274767245ea
final-pal12-180s.json                   0ff5bdd0d921132b292ee36f73620936f2646bef9e8484459ed8a84d0eb109ce
regime-pal12-180s.json                  813745e2b18bf3ca3ecde8b3c2151f8d73443d53357bc0a53be5e67eff6de10c
regime-cost-pal12-180s.json             0aa2182546cce6de4172c913d9f0648056bc1ba9b327d19054d8bd6f8a4b2507
regime-cost-pal16-285played-180s.json    39705bae52c464bca42d7cf17e790e9efcb6fcce9ed6d31c97ebeb935f867956
regime-cost-pal16-historical-180s.json   1214affea3ce9f0f4796e7c05af6acf816e68be2021a9285adcaa7f104795632
compact-pal16-historical-180s.json       fd99e33bdc639cc55ee80343b1f6e2ee9843cab56741d7abb9f8f40914f6e892
gd-rebuild-pal16-historical-180s.json    79cdfc83ce7133e67885ffa3181b035e7c5e4fa4e87887cbf9cb2028e0eee8ac
trusted-highs-control-pal16-180s.json    eaf6aa097174f53ef0a4af738639fa1add6684439bcb6f7cd7a5e85c72869c4f
trusted-highs-pal16-180s.json            399bc12e110464ca5ad6649590242c175d5975c18d7ab3cd41cafb1641075405
smoke-pal12.json                        0fdb21e73a7cb9f671c2045758290c4b4a27bbe85f0e1551e588e0cc3d1fad79
smoke-v2-pal12.json                     ee8eb5e874653eaa3b2fabca380ccf8299a575c1d6d5d7081660e606f7c0e969
```

## Validation

All43 Python experiment tests pass, including the existing confidence and
solver-audit suites. New checks cover common-probability precision, tiny
probability intervals, zero lower bounds, invalid intervals, confidence
conflicts, exact positive points and logarithmic boundaries. Driver integration
tests verify that an interval with ratio above10 can trigger adaptive stopping,
that its geometric midpoint near1e-250 stays positive despite endpoint-product
underflow, and that `--run-full-budget` continues through additional sampling.
An independent source review found no blocking issues. No native Rust source,
production endpoint or confidence-bound construction changed.
