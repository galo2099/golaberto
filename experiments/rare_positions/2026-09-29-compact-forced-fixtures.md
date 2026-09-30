# Compact forced fixtures out of rare-position sampling (2026-09-29)

## Adopted change

After rank-domain propagation, cache a compact simulation plan for each
sampled target fixture assignment. Pre-apply the forced WDL outcomes and their
points/wins increments. Multiply their original probability product once per
draw. Iterate only the unresolved fixtures in the WDL proposal loop.

The compact plan also caches each unresolved fixture's marginal lookahead
CDF references and necessary min/max feasibility limits. This removes repeated
suffix indexing and domain-array lookups from the inner loop.

For current group 16653, Fortaleza / 18th has eight forced Londrina wins.
The target's nine fixtures were already handled by exact point conditioning;
the remaining WDL loop now handles **68 fixtures instead of 76**.

The selected sampler workloads are **15–31% faster than commit `d62c0625`**.
The full current-16653 request saves a median paired **28 ms / 2.6%** on four
cores. No additional work is allocated, and no cells gain or lose estimates.

This is enabled by default in the propagated-domain sampler. Existing
matched-point-pool production flags are sufficient. Set
`RARE_POSITION_COMPACT_FORCED_FIXTURES=0` to disable the compact execution path.
Startup diagnostics include the raw flag and `effective_compact_forced_fixtures`.
Rebuild and restart the Go service to deploy. No Ruby, JavaScript, or API JSON
shapes changed.

## Preserving the probability model and seeded searches

- Forced fixtures have proposal probability one. Their original WDL
  probabilities remain in the importance weight; they are never treated as
  probability-one events under the original model.
- Pre-applying future forced gains must not change an unresolved fixture's
  proposal cap. Each compact entry stores those future gains so its proposal
  sees the same prefix as before compaction.
- The proposal continues to use the original marginal suffix CDF. Removing
  forced games from that CDF would change the proposal and seeded allocation.
- The old RNG value for each forced fixture is consumed at the corresponding
  boundary, without looking up the fixture or evaluating a proposal score.
  Early rejection consumes the same prefix of random values. This preserves
  later draws, hits, pilots, finalist selection, and search allocation.
- Zero guide coefficients and an underflowing initial forced probability
  product retain the original loop and its early-rejection behavior.
- Forced WDL outcomes can still require sampled goals to resolve later
  tiebreakers. That full standings evaluation remains unchanged.
- `WorkSpent` retains its existing conservative season-equivalent accounting.
  It is not a timing counter; this optimization does not increase any budget.

The first implementation put compact-mode branches inside every fixture
iteration. It saved only roughly 1–4% within selected workloads and introduced
overhead relative to the previous commit. The adopted version separates the
original and compact loops and caches the unresolved fixtures' feasibility
data. Inputs without forced fixtures retain the original loop.

## Sampler benchmarks

Apple M2 Pro, darwin/arm64, `GOMAXPROCS=4`. Each benchmark operation constructs
the sampler and runs 15,000 draws with seed 7011 and rank tilt 6. Point tilt is
selected from the same climb/drop rule used in production. Values are medians
of three measurements, each containing ten operations. Measurements were run
serially, without concurrent Go test or benchmark jobs.

`HEAD` is an isolated archive of `d62c0625` with only the benchmark harness
added. Disabled/enabled are ablation arms within the final implementation.
The disabled arm still constructs cached compact metadata, so the previous
commit is included to measure the complete change as well.

| Input | Team / rank | Previous commit (ms) | Compact disabled (ms) | Compact enabled (ms) | Faster than previous commit |
|---|---|---:|---:|---:|---:|
| Current 16653 | Fortaleza 22 / 18 | 52.50 | 54.62 | 37.56 | 28.5% |
| Current 16653 | Sport 77 / 19 | 82.62 | 85.18 | 61.03 | 26.1% |
| Current 16653 | Juventude 12 / 18 | 52.79 | 54.43 | 37.86 | 28.3% |
| Current 16653 | Atlético-GO 279 / 18 | 50.08 | 51.29 | 42.37 | 15.4% |
| Earlier 16653 | Fortaleza 22 / 18 | 38.67 | 38.99 | 38.78 | -0.3% |
| Earlier 16653 | Sport 77 / 19 | 87.28 | 88.71 | 63.83 | 26.9% |
| Earlier 16653 | Juventude 12 / 18 | 41.67 | 41.63 | 41.53 | 0.3% |
| Earlier 16653 | Atlético-GO 279 / 18 | 51.21 | 50.73 | 50.55 | 1.3% |
| Earlier 16653 | Ponte Preta 9 / 14 | 42.30 | 43.26 | 29.36 | 30.6% |

The earlier inputs with negligible differences are controls, not evidence
of a useful speed gain. Selected cases with forced domains improve substantially.
These are sampler benchmarks, not whole-request improvements.

## Paired full requests

Five saved inputs, five seeds each: `801,804,808,817,911`. Four cores and four
matched-pool workers. Each request uses the existing 20,000 initial MC scout,
100,000 matched-pool seasons, conditioned searches, and propagated-domain pass.
Only the compaction flag differs. Arm order alternates by seed parity.

| Input | Pairs | Median paired time saved | Median paired reduction | Gained / lost nonzero cell-runs |
|---|---:|---:|---:|---:|
| 16498, `44eabb47` | 5 | 20.1 ms | 1.4% | 0 / 0 |
| Current 16653, `71d4fea8` | 5 | 27.7 ms | 2.6% | 0 / 0 |
| Earlier 16653, `2d1c1d6f` | 5 | 10.7 ms | 0.9% | 0 / 0 |
| 16982, `9327edcd` | 5 | 5.8 ms | 0.6% | 0 / 0 |
| 16983, `43969b02` | 5 | 1.2 ms | 0.1% | 0 / 0 |

Timing remains noisy at this scale. Current-16653 paired changes include one
slower request; use the paired median rather than comparing unrelated seeds
or subtracting the independent arm medians. Groups 16982 and 16983 have no
zero targets, so their tiny timing differences are control noise.

Across **10,000 paired cells**, probabilities agree within a maximum relative
difference of **1.50e-14**, from reordered floating-point multiplication.
Reachability, hit/sample counts, selected estimator design, precision/availability
decisions, and work allocation are unchanged. Differences in floating-point
standard errors, ESS and derived bounds are similarly numerical.

## Saved inputs and reproducibility

| Input path | SHA-256 |
|---|---|
| `/private/tmp/golaberto-group-16498-current.json` | `44eabb47ee55477c77025c3de4e839c6ea913b8569dd29694d7178bdb8bcdc89` |
| `/private/tmp/group16653-live-20260927.json` | `71d4fea8502c3086b8787c764a94b5f8a5caa6549300f2a7687ead8897e11aec` |
| `/private/tmp/golaberto-real-fixtures/group-16653.json` | `2d1c1d6f70a64d5b86f5129b32cbf77bedc4c42e464cd849011ca0b3018b3d87` |
| `/private/tmp/golaberto-group-16982-current.json` | `9327edcdfcaa5cdc0f975771e9150b2436750231ea4b417a279fb372564e2aeb` |
| `/private/tmp/golaberto-real-fixtures/group-16983.json` | `43969b02fe7138b991b9d74215223efbc9fc16bc8f4ebe6d0a5e274126f8af36` |

Raw outputs are intentionally ignored under
`experiments/rare_positions/local/2026-09-29-compact-forced/`:
`full-final/`, `kernels-adopted.txt`, and `head-kernels.txt` are the adopted results.
Earlier prototype measurements are retained there for comparison.

Run from the repository root; use an absolute output directory:

```sh
GOCACHE=/private/tmp/golaberto-go-cache GOMAXPROCS=4 \
RARE_POSITION_COMPACT_EXPERIMENT_REQUESTS=/private/tmp/golaberto-group-16498-current.json,/private/tmp/group16653-live-20260927.json,/private/tmp/golaberto-real-fixtures/group-16653.json,/private/tmp/golaberto-group-16982-current.json,/private/tmp/golaberto-real-fixtures/group-16983.json \
RARE_POSITION_COMPACT_EXPERIMENT_OUTPUT=/Users/robsonaraujo/work/golaberto/experiments/rare_positions/local/2026-09-29-compact-forced/full-final \
go test ./go -run '^TestCompactForcedFullRequestExperiment$' -count=1 -v

GOCACHE=/private/tmp/golaberto-go-cache GOMAXPROCS=4 \
RARE_POSITION_COMPACT_EXPERIMENT_REQUESTS=/private/tmp/group16653-live-20260927.json,/private/tmp/golaberto-real-fixtures/group-16653.json \
go test ./go -run '^$' -bench '^BenchmarkCompactForcedFixtures$' -benchtime=10x -count=3
```

The benchmark contains named reference cells only in test code. Production
compaction is generic across teams, ranks and groups.

## Correctness checks

- Exhaustive compact-prefix/final-score comparison for a fixture graph with
  forced fixtures before, between and after unresolved fixtures; verifies the
  forced probability product and fixture identities.
- Fixed-seed sampler equivalence with wins as a ranking prefix and with wins
  later in the tiebreaker order.
- Current Fortaleza regression checks that all eight forced Londrina wins are
  absent from the unresolved plan.
- All 25 paired full requests passed, preserving estimates and reachability.
- Full Go suite passed.
- Targeted race check includes the compact unit tests, current Fortaleza
  regression, and a paired full current-16653 request at seed 808.
