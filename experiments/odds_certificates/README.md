# Odds certificate implementation and measurement

This branch adds automatic, simultaneous confidence intervals to odds responses.
The statistical contract and supported evidence are documented in
[`CONFIDENCE_CERTIFICATES.md`](../../odds-rust/CONFIDENCE_CERTIFICATES.md).

Production probabilities, standard errors, counts, effective sample sizes,
reachability labels, and match importance remain the existing outputs. The new
metadata gives absolute probability bounds, contributing evidence sources, the
adaptive precision decision, a separate certified estimate when qualified, and
whether the production estimate lies inside the interval. Matrix reconciliation
does not change confidence endpoints.

## Saved-request comparison, 2026-10-18

The evaluation contains seven distinct snapshots and 23 seeded runs, totaling
9,048 cell-runs. Both arms use the same default release binary, four workers,
the existing simulation budgets, and logging disabled. The family size is
explicitly set to 9,048, so the 99% simultaneous guarantee covers this whole
predeclared evaluation family. Production's default family is one request.

The runner alternates certificate-off/on order, excludes one warmup per arm,
and verifies input, source, and binary hashes. All 46 measured processes
succeeded. Raw requests, responses, logs, and detailed analyses remain outside
the repository.

| Evidence available | Qualified positive cell-runs | Proved impossible | Qualified or proved |
| --- | ---: | ---: | ---: |
| Native certificates | 3,620 | 540 | 4,160 |
| Native plus own-points matched strata | 3,620 | 540 | 4,160 |
| All supported certificates | 3,630 | 540 | 4,170 |

All 8,508 nonproof cells have bounds. Conditional-sampling evidence contributes
to 356 cells and adds ten newly qualified cell-runs: three distinct cells in
snapshot `16498-44eabb47` across repeated seeds. These are team 7 first, team
584 second, and team 10 first. Matched-strata evidence adds no qualified cells
and reduces aggregate interval width by only 0.00110%.

The independent audit includes proof cells: there are zero confidence conflicts
and zero production estimates outside their intervals. After removing the new
`confidence` field, all 23 paired outputs are exactly identical. The off arm also
matches all 23 archived production outputs exactly.

| Runtime across 23 requests | Certificates off | Certificates on | Change |
| --- | ---: | ---: | ---: |
| Total wall time | 32.075 s | 34.892 s | +8.78% |
| Total child CPU | 67.052 s | 71.672 s | +6.89% |
| Maximum individual wall time | 2.84 s | 2.99 s | |

The median paired request wall-time increase is 11.95%; the largest relative
increase is 37.63%. Mean added wall time is about 0.122 s per request.
The seven-snapshot seed-1847 subset has 1,162 qualified positive cells plus
108 impossibility proofs, out of 2,724 cells.

## Comparison with the earlier acceptance assessment

The earlier native-only Clopper–Pearson assessment qualified 3,902 positive
cell-runs and had the same 540 impossibility proofs: 4,442 completed cells
(49.09%). This implementation completes 4,170 (46.09%), leaving 4,878 below
the precision requirement. It reserves error allowance for matched and
conditional methods and uses conservative tail enclosures rather than exact
Clopper–Pearson inversion. Therefore the ten-cell conditional gain does not
offset the native-only qualification loss in this first implementation.

The cross-team matched production heuristic itself is not an exchangeable
sample and does not acquire a confidence guarantee from its jackknife error.
Its accompanying certificate uses the team's own observations. Broader guided,
shared-mixture, exact-cap, and partial-cohort samplers still need method-specific
likelihood bounds. Their existing estimates can carry bounds supplied by the
supported evidence; that does not certify the unsupported sampler itself.

The next useful changes are tighter own-points uncertainty aggregation and
complete weight bounds for the main guided proposals. Confidence-budget tuning
and exact interval-result caching are also worth measuring on fresh runs.

## Validation and provenance

- Full default release suite: 530 passed, five intentionally ignored. The HTTP
  tests passed after enabling local networking in the execution sandbox.
- Manual release request-performance test: passed; seven measured endpoint
  calls on the 218-team fixture had median 6,500.661 ms. This is an absolute
  measurement, not a paired regression comparison.
- Targeted tests include finite binomial-tail enumeration, finite-ticket score
  enumeration, unseen strata, allocation enforcement, proof conflicts,
  unsupported scopes, model identity, event mutation, and enabled/disabled
  parity of the complete fixed conditional sampling result.
- Production base: `77a3da2e24ff0fe51282326c44471a8768776e70`.
- Implementation commit: `21cc910c`.
- Binary SHA-256:
  `e6ccad2c787db183d7bab077c256ed22afff1434e50ddb4efd3144c4aa4d73bc`.
- Production-source SHA-256:
  `3bb13154f3f0907309e8d66f681383660d2a178e13a15ecff0e8ecbcb37a473b`.
- Private results: `/workspace/rare-odds-certificates-2026-10-18/paired`.

To reproduce the private evaluation, run `run_benchmark.py --binary PATH --run`
with the archived manifest and inputs available. The script is inert without
`--run` and rejects an output directory inside the repository.
