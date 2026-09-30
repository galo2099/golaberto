# Hundredfold rare-position search and partial golden reference

## Answer

The earlier 10× run did not resolve most remaining group 16498 zeros: it
added a median of six estimates and left a median of 18 non-impossible zeros.

The new 100× experiment adds **7 estimates in the median group 16498 run**
(range 6–11). Across three independent seeds it estimates 12 distinct cells
that were zero in at least one baseline. Further independent proposal checks
recover additional evidence. The resulting group 16498 reference contains
**13 repeatable tiny-event estimates, two provisional tail estimates, and
15 proven-impossible cells**. **Eleven baseline-zero cells remain undecided.**
The zero sets differ by seed; these combined counts are not a breakdown of a
particular live 38-zero request.

100× sampling therefore helps, but does not provide a complete probability
matrix or settle every remaining reachability question. The saved reference
explicitly retains its unknown entries. It is suitable for coverage and
order-of-magnitude comparisons on its accepted reference subset, not exact ground truth.

## Artifact

Portable dataset:

`reference/2026-09-30-hundredfold/reference.json`

It contains all 2,000 cells across five snapshots, with probabilities as
fractions, source fingerprints, seeds, MC counts, proof labels, baseline
evidence and held-out importance confirmation diagnostics. The matching
`inputs/` directory contains normalized requests. They preserve every field
consumed by Go's `GroupType`, including fixture/team order and powers, while
omitting Rails timestamps and other unused request metadata. Go tests compare
each portable request with its original saved export and confirm equality
after decoding into `GroupType`.

| Reference input | Source SHA-256 prefix |
|---|---|
| 16498 current | `44eabb47` |
| 16653 earlier, 90 unplayed fixtures | `2d1c1d6f` |
| 16653 current, 85 unplayed fixtures | `71d4fea8` |
| 16982 current | `9327edcd` |
| 16983 saved | `43969b02` |

These are five inputs, including two different snapshots of group 16653.
Full source and semantic input fingerprints are stored in the dataset.

## Search experiment

Estimator commit: `64fa19c9`. Four cores throughout. Production starts with
20,000 scout seasons and the 100,000-season matched-point pool. Existing
conditioned searches and production defaults remain enabled; legacy CEM
importance sampling remains disabled, matching preceding experiments.

Seeds: `801`, `808`, `1790746108560577000`.

For each request:

1. Retain its full production result.
2. Select every zero not already proved impossible.
3. Pilot the existing six rank/point-tilt combinations, up to 5,000 draws
   each, with the existing points-only pilot screen.
4. Divide remaining work equally among eligible cells. Use two thirds for a
   fresh selected-proposal confirmation and one third for a fresh rank-tilt-3
   confirmation. Final sampling uses wins when the ordering allows it.
5. Apply the existing hit/ESS/relative-error/weight-share/batch-agreement
   acceptance gate and retain the existing independent contradiction check.
6. Preserve all existing nonzero values. Pilot draws never enter estimates.

The extra simulation work is capped at **99 times the modeled full request
work**, including scout work in the reference denominator. Inputs with no
zeros stop immediately at 1×. This is directed extra work on missing cells,
not a 100-fold increase in every production loop. Integer rounding leaves a
small unused allowance. No production sampling default was increased.

| Input | Baseline zeros, range | New estimates, median (range) | Non-impossible zeros after 100×, median | Median total runtime |
|---|---:|---:|---:|---:|
| 16498 | 36–41 | **7 (6–11)** | 16 | **62.6 s** |
| 16653 earlier | 54–56 | 9 (8–9) | 4 | 47.8 s |
| 16653 current | 60–63 | 9 (6–12) | 3 | 49.5 s |
| 16982 | 0 | 0 | 0 | 0.95 s |
| 16983 | 0 | 0 | 0 | 0.97 s |

The 15-request search took 497 seconds. It performed **624,692,621 additional
conditioned fixture assignments** and charged approximately **71.56 billion
total modeled fixture/team work units**, including baseline requests. There
were no losses of baseline nonzero estimates, by construction and assertion.

The earlier 10× median runtime for 16498 was 6.6 seconds. Comparing its six
median gains with the new seven is descriptive: production code and seed sets
differ, so this is not a controlled marginal-speed or marginal-accuracy result.
The additional budget alone did not eliminate the difficult tail.

## Separate verification work

### Independent Monte Carlo

Run **5,000,000 fresh plain-MC seasons per input**, totaling 25 million. Each
input uses four private worker streams derived from base seed `947117`, its
original request SHA-256 and worker index. Total runtime: 86.6 seconds.

Use MC cells with at least 25 hits as reference estimates. This corresponds
to approximately 20% relative sampling error at the minimum hit count, adequate
for these order-of-magnitude comparisons. Counts and standard errors remain
available for every cell. Zero hits are accompanied by their 95% MC upper bound,
approximately `5.99e-7`, and are never treated as an impossibility proof.

### Independent tail proposals

Nineteen previously zero, proved-reachable or provisionally estimated cells
receive three independent 300,000-draw proposals at rank tilts 4, 6 and 10,
with point-tilt magnitudes 0.5, 0.75 and 1. Direction follows the target rank
relative to current standing. Streams derive from base seed `21831`; the
actual stream seed is stored for each result. Each proposal must pass the full
production validity gate before entering the reference.

This adds **17.1 million draws**, or **1.9521 billion modeled work units**,
and takes 15.6 seconds. It is verification work **outside the primary 100×
coverage budget**, as is the independent MC reference.

### Necessary fixture capacity proofs

The offline solver considers the maximum-points and minimum-points relaxations,
encoding wins in the ordered scalar only for an allowed points/wins prefix.
For each possible allocation of rank-side exemptions, it applies individual
domain propagation and necessary subset capacity inequalities: the sum of
minimum contributions from shared fixtures must fit the sum of the subset's
caps. Negating scores applies the same test to minimum requirements.

Only exhaustive rejection of all necessary exemption allocations proves
impossibility. A consistent relaxation or exhausted budget remains undecided.
The allowance is 200,000 nodes per direction/cell; actual searches here need at
most 17 nodes. All five inputs' proof passes together take under 0.3 seconds.
Exhaustive small-fixture tests verify that the cuts and exemption search retain
every feasible assignment, including negative gains.

This finds **one additional impossibility in group 16498: team 17 / 15th**.
The production baseline proves 14 cells impossible; the offline reference proves
15. The new proof is saved as reference evidence and has not been enabled as a
new production proof stage.

## Reference confidence rules

| Status | Meaning | Used for accuracy/coverage scoring |
|---|---|---|
| `impossible` | Necessary fixture constraints certify the rank unreachable; probability 0 | Yes |
| `reference_mc` | At least 25 hits in independent 5M plain MC | Yes |
| `reference_is` | At least two accepted, independently streamed held-out confirmations; maximum/minimum probability ratio <=3 | Yes |
| `provisional_is` | Accepted probability evidence without sufficient repetition/agreement | No |
| `reachable_no_reference` | Simulation/construction proves reachability, but no reference-quality probability was produced | No |
| `estimated_unverified` | Production has a pooled nonzero value without a recorded hit/witness or independent reference estimate | No |
| `undecided` | No reachability proof or adequate probability evidence | No |

`reference_is` uses the median of accepted confirmations and stores their
empirical range. That range is not a confidence interval. Repetition does not
rule out a shared proposal bias. References estimate this service's stochastic
model, not the real-world calibration of its fixture powers.

Unresolved reference probabilities are `null`. Some known-reachable cells with
production estimates consequently have a null reference probability; their
original values are retained separately. A positive pooled value alone is
not treated as a reachability witness.

| Input | MC reference cells | Repeatable IS cells | Impossible | Provisional IS | Reachable without reference | Pooled estimate unverified | Undecided baseline zeros |
|---|---:|---:|---:|---:|---:|---:|---:|
| 16498 | 296 | 13 | 15 | 2 | 37 | 26 | **11** |
| 16653 earlier | 294 | 11 | 43 | 2 | 36 | 14 | 0 |
| 16653 current | 289 | 12 | 48 | 1 | 39 | 11 | 0 |
| 16982 | 368 | 0 | 0 | 0 | 18 | 14 | 0 |
| 16983 | 317 | 0 | 0 | 0 | 30 | 53 | 0 |
| Total | **1,564** | **36** | **106** | **5** | **160** | **118** | **11** |

There are **1,706 scorable cells**, including 106 known zeros. Uncertified
cells are explicitly excluded, not scored as probability zero.

### Group 16498 tail examples

| Team / rank | Reference fraction | Accepted confirmations | Status |
|---|---:|---:|---|
| Athletico-PR (5) / 18 | `3.32e-15` | 2 | Repeatable |
| Cruzeiro (15) / 19 | `1.68e-12` | 3 | Repeatable |
| Cruzeiro (15) / 20 | `2.01e-18` | 3 | Repeatable |
| Chapecoense (318) / 3 | `6.34e-24` | 2 | Repeatable |
| Fluminense (8) / 20 | `5.95e-26` | 1 | Provisional |
| Flamengo (17) / 9 | `2.76e-13` | 1 | Provisional |

The 11 still-undecided baseline-zero cells are:

- Team 5: 19th and 20th.
- Team 16: 13th through 17th.
- Team 17: 11th through 14th.

For the reported seed's 36 baseline zeros, the combined reference classifies
15 impossible, nine repeatably estimated, one provisionally estimated, and 11
undecided. The live 38-zero run needs its seed/snapshot for an exact paired
breakdown.

## Comparing a future estimator run

The comparison tool checks the request's semantic fingerprint. It reports
missing known-positive cells, positive estimates for proved-impossible cells,
MC RMSE, and log10 error for nonzero importance references. It excludes all
uncertified reference entries. It never feeds reference probabilities into
production estimation.

```sh
python3 experiments/rare_positions/compare_golden_reference.py \
  --reference experiments/rare_positions/reference/2026-09-30-hundredfold/reference.json \
  --request path/to/exact-request.json \
  --estimates path/to/saved-response.json
```

The default matrix key is `rare_position_estimates`; for saved experiment
exports use `--matrix-key baseline` or `--matrix-key extended`. Either the
original request or its committed normalized counterpart can identify a case.

## Regeneration and tests

The primary search harness is `TestRarePositionHundredfoldWorkExperiment`.
It uses the existing `RARE_POSITION_BUDGET_EXPERIMENT_REQUESTS`, `_OUTPUT` and
`_SEEDS` variables. The tenfold harness remains available unchanged in scope.

The independent reference harnesses are `TestRarePositionReferencePlainMC`
with `RARE_POSITION_REFERENCE_MC_OUTPUT`, and `TestRarePositionReferenceDeepProof`
with `RARE_POSITION_REFERENCE_PROOF_OUTPUT`. They share the request-path variable.
Tail checks use `TestRarePositionBudgetProposalCheck`, with `_CHECK_CELLS`,
`_CHECK_SEED=21831` and `_CHECK_SAMPLES=300000`.

Use the builder on their raw outputs:

```sh
python3 experiments/rare_positions/build_golden_reference.py \
  --runs-dir path/to/100x-runs \
  --proof-dir path/to/proofs \
  --mc-dir path/to/5m-references \
  --checks-dir path/to/proposal-checks \
  --output-dir path/to/generated-reference \
  --estimator-commit 64fa19c9
```

Full Go tests, portable-input equivalence, reference-data invariants, exhaustive
small-fixture proof tests and Python confidence/comparison tests pass. Raw
per-run matrices and logs remain in ignored `local/2026-09-30-hundredfold-*`
directories. The normalized inputs, compact evidence dataset, tooling and this
report are committed. Production runtime and response shapes are unchanged.
