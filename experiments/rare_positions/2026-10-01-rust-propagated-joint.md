# Propagated joint conditioning and funded allocation (2026-10-01)

## Scope and baseline

Task: carry propagated constraints into the joint probability sampler and reconsider the allocation cutoff. Compare against the shipped **Rust** implementation at `65b71493e738ce434f071e717b0646a0e4c8a842`, not the earlier Go implementation. Preserve four workers, existing proof quotas and neighborhood budgets. No extra one-second allowance is assumed. No database mutation, commit or push was performed for this experiment.

The frozen baseline executable is `/private/tmp/golaberto-propagated-joint-baseline`; the candidate is the normal `golaberto-odds` binary with no enabling flags. Binary hashes and exact input hashes are recorded in the accompanying results JSON. Timings are full HTTP requests, including decoding, calculation, response construction and serialization. Persistent comparisons alternate request order on two idle servers: two warmup pairs, then eight measured pairs per snapshot. Only one request runs at a time; four calculation workers total. CPU and peak RSS come from each child process, including startup and warmups.

## Why the prior cutoff missed Athletico/20th

For group 16498, Athletico (team 5) starts with 49 points and 14 wins, with ten fixtures left. Chapecoense must win all eleven remaining fixtures, reaching 51 points and 14 wins. Once Chapecoense's wins are fixed, Remo must win its other nine fixtures, reaching 50 points and 14 wins. Athletico cannot win any fixture and can draw at most one. The draws against Chapecoense and Remo are excluded by their necessary wins.

Independent team bounds initially admit 56 target patterns (zero, one or two draws across ten games). Shared-fixture propagation excludes 47 patterns. The complete surviving union consists of nine cases: all losses, or a draw in one of the eight other fixtures. Each case fixes **28 of 103 fixtures**. These are necessary constraints, not a preferred outcome guessed from one witness.

The existing allocator excluded this cell at its `1e-11` upper-bound cutoff. Simply conditioning on the forced fixtures still produced no accepted estimate. Remaining rivals must jointly clear their rank floors, with dependencies through shared games; guiding those fixtures in chronological order also produced poor effective sample size.

## New sampler

`odds-rust/src/joint_caps/propagated.rs` implements a generic model for every team/rank, without team or position constants:

1. Enumerate the **complete** supported target W/D/L union allowed by necessary rank bounds.
2. Run existing `Domains::propagate` for each case, with shared fixtures, requested rank cardinality, and packed points/wins when applicable. Keep tied teams as potentially ahead or behind.
3. Apply every propagated fixture mask and required rival floor/cap. Compact forced fixtures out of the loop. Multiply their probabilities and the allowed-mask masses into the case probability.
4. Select up to six difficult rivals whose shared internal fixture enumeration fits the setup budget. Enumerate shared fixtures once, with safe external-table feasibility pruning. Sample each disjoint external fixture stream from its exact backward terminal table.
5. Guide remaining fixtures with their propagated rival bounds and rank cardinality. Visit fixtures involving difficult rivals first. Maintain cardinality incrementally by updating only the two fixture endpoints.
6. Sample actual scores with the existing conditional score distribution and verify the rank with the production sorter. Goal-difference and goals-scored ties are never decided by the points relaxation.

Guides and joint tables are cached across cases with identical masks and relative gain intervals. A reversed-gain DP evaluates rare upper tails directly; it avoids subtracting a rounded CDF from one. When the guidance weights a tail and ties equally, one inclusive-tail lookup replaces three separate lookups, for either direction.

### Weights and support

For disjoint target cases `c`, let `m_c` be the probability of the target outcomes, forced outcomes, allowed residual masks and selected rivals' necessary terminal bounds. Then `M = sum(m_c)` is the probability of a necessary event: the desired rank event is a subset of it. **M is an upper bound, not the estimate.** For Athletico/20th, `M = 1.2815375916403327e-25`.

Independent residual-tail probabilities only supply a soft case tilt `a_c`, floored at `1e-6`. Let `Z = sum(m_c * a_c)` and draw cases with probability `m_c * a_c / Z`. The initial normalized importance weight is `Z / (M * a_c)`. Multiply by every residual masked base-probability/proposal-probability ratio, then summarize rank hits and multiply by `M`. The sampler therefore estimates `E_Q[1(rank) * P/Q]`. The tilt is not treated as an independent probability bound.

Necessary prefix infeasibility can discard an outcome. A small base component retains every positive outcome within that necessary support when numerical guidance rounds to zero. Setup must include the complete bounded union or skip the proposal; it never silently truncates target cases or proves impossibility from an exhausted budget.

### Setup limits

- Standard 3/1/0 scoring, no bonus points, points first. Pack wins only when wins immediately follow points.
- At most 256 remaining fixtures; 1–32 target fixtures; per-team packed gain span at most 4096; checked integer arithmetic.
- At most 64 initial target patterns and 16 feasible propagated cases.
- Every feasible case must force at least four **non-target** fixtures.
- Up to six selected rivals, at most 12,000 raw shared internal states, 20,000 enumeration nodes per selected-set attempt and a hard 60,000-node budget across unique cached tables, including unsuccessful retries. If a selected set is too dense, reduce it and retain exact support. A four-million-value guard bounds the newly allocated forward/reverse guide rows (about 32 MB of floating values per model).
- Unsupported, zero numerical mass or exhausted models return no proposal. They cannot supply an unrestricted impossibility proof.

## Replacing work and the cutoff

Keep the old four selected candidates and their exact/broad joint streams. For a cell below the old `1e-11` cutoff, attempt the bounded structural model. Prioritize eligible cells by `forced_min / (1 + residual_variable_max)` and select at most one extra cell. This score predicts tractability, not probability.

Funding comes from the existing ordinary batches. Preserve 43,000 ordinary draws when the two broad streams forecast at least 0.01 ordinary hits in 50,000 draws. For a less promising broad candidate, retain 25,000 ordinary draws instead of 35,000, releasing 10,000 draws. Fund the new **5,000 main + 3,000 independent confirmation** only when at least 8,000 draws were released. Unsupported old candidates and exact candidates retain their original budgets. This is draw accounting; measured CPU time, not draw counts alone, determines its performance.

Run old joint attempts first; then schedule the new joint batch and longest ordinary batches before short ones, using four workers. Deferred publication preserves the original neighborhood and witness-seed allocation. Publish accepted estimates only into cells still zero after existing estimators. If confirmation rejects a propagated estimate, retain its verified witness and analytical `M` bound. **Do not apply a binomial zero-hit bound to biased importance samples.**

The acceptance thresholds are unchanged: main at least 30 hits, ESS >= 8, relative SE <= 35%, maximum event-weight share <= 25%, batch-gap check; independent confirmation at least 30 hits, ESS >= 5, relative SE <= 60%, and a main/check ratio between 0.1 and 10. Finite-sample diagnostics do not certify rare-event accuracy.

## Variants considered

| Variant | Gains / losses across 18 paired cell-runs | Cost/result | Decision |
|---|---:|---|---|
| Replace the fourth candidate and also replace existing joint proposals | +4 / -10 | Lost six Flamengo 9th/10th cell-runs and four Fortaleza/17th cell-runs | Reject |
| Transfer draws, retain old proposals, single phase | +4 / 0 | Group 16498 median cold request 833.86 -> 852.49 ms (**+2.23%**) despite lower CPU | Reject scheduling |
| Two phases, small independent check, short ordinary job too early | Earlier cohort +4 / 0; warm seed 808 no gain | Group 16498 warm 672.39 -> 676.77 ms (**+0.65%**) | Replace ordering and larger check |
| Long batches first, 5,000 + 3,000 new draws | +5 / 0 in development cohort | Earlier warm group 16498 675.53 -> 663.41 ms (-1.79%) | Validate default on complete cohort below |
| Default before inclusive-tail optimization | +5 / 0 across all 42 pairs | Warm group 16498 676.12 -> 680.56 ms (**+0.66%**); controls up to **+2.22%** | Optimize the remaining lookup cost and remeasure |

Root forced fixtures alone, additional joint conditioning alone, case tilting alone and chronological fixture ordering were insufficient for reliable acceptance. Exact rival conditioning plus difficulty ordering, cached guides, and the larger independent check produced the useful result.

## Final paired results

Across **42 paired requests** (seven snapshots × six fixed seeds), the final candidate gained **five nonzero cell-runs, representing one unique cell: Athletico/20th**. It lost no positive cell, reachability classification or impossibility proof. Every pre-existing positive probability and standard error was **bit-for-bit unchanged**, and game importance was identical. No new impossibility proofs were produced. No previously undecided cell was newly resolved.

### Warm full-request wall time and CPU

| Snapshot | Baseline ms | Candidate ms | Wall change | CPU change | Peak RSS baseline -> candidate MiB |
|---|---:|---:|---:|---:|---:|
| 15902 | 205.29 | 202.23 | -1.50% | -2.32% | 18.59 -> 18.42 |
| 16413 | 190.67 | 194.12 | +1.81% | +1.99% | 17.52 -> 18.19 |
| 16498 | 677.32 | 678.62 | +0.19% | -4.85% | 169.48 -> 171.50 |
| 16653 current | 721.52 | 705.41 | -2.23% | -2.36% | 177.45 -> 172.11 |
| 16982 | 318.90 | 320.75 | +0.58% | +3.47% | 23.61 -> 23.97 |
| 16983 | 311.83 | 312.00 | +0.05% | -0.15% | 23.14 -> 22.41 |
| 16653 historical | 692.13 | 684.46 | -1.11% | -3.71% | 164.39 -> 163.38 |

The equally weighted sum of these seven medians changed by -0.64%; total process CPU across their ten request pairs changed by -2.34%. This is a corpus summary, not a claim that every request is faster.

**Latency increases are explicit:** group 16498 was 1.30 ms slower (+0.19%), despite 4.85% lower CPU; group 16413 was 3.45 ms slower (+1.81%); 16982 was 1.85 ms slower (+0.58%); 16983 was 0.17 ms slower (+0.05%). The control increases occur in the ordinary scout/pool stages; they ran no propagated sampler. Attribution to system variance or binary code layout is not established. Earlier paired runs changed direction. These measurements do not demonstrate strictly equal latency for every group. No additional latency allowance is assumed.

For group 16498, median `search.extra` increased **96.36 -> 125.27 ms**, while other search-stage costs fell and CPU decreased. New model setup is about **10.2 ms**, sampling about **47.6 ms** for 8,000 draws, versus about 56.6 ms before inclusive-tail optimization. The whole request remains about 0.68 s on the measured machine; a new-stage cost cannot be mistaken for the full-request increment. Unselected/unsupported models also consume planning work, included in the measured stage.

### Cold requests across seeds

These include a fresh server per request and show substantially more scheduling variability. They are included rather than replaced by the warm result.

| Snapshot | Median baseline ms | Median candidate ms | Wall change | CPU change | Gains / losses |
|---|---:|---:|---:|---:|---:|
| 15902 | 201.35 | 203.89 | +1.27% | +1.75% | +0 / -0 |
| 16413 | 192.02 | 193.66 | +0.85% | -0.33% | +0 / -0 |
| 16498 | 849.39 | 875.02 | +3.02% | -1.60% | +5 / -0 |
| 16653 current | 734.95 | 731.07 | -0.53% | -1.39% | +0 / -0 |
| 16982 | 317.57 | 319.86 | +0.72% | +0.17% | +0 / -0 |
| 16983 | 310.77 | 310.19 | -0.19% | -0.70% | +0 / -0 |
| 16653 historical | 773.54 | 713.99 | -7.70% | -4.17% | +0 / -0 |

### Athletico/20th estimates and remaining zeros

| Seed | Published probability | Main ESS | Check ESS | Decision |
|---:|---:|---:|---:|---|
| 801 | 1.4635e-34 | 62.54 | 14.79 | accepted |
| 804 | 1.6730e-34 | 41.73 | 45.94 | accepted |
| 808 | 2.0709e-34 | 17.95 | 10.36 | accepted |
| 817 | 1.8856e-34 | 41.33 | 20.33 | accepted |
| 818 | 0.0000e+00 | 12.69 | 3.76 | rejected: independent ESS < 5 |
| 911 | 1.6443e-34 | 27.24 | 23.69 | accepted |

All probabilities here are fractions, not percentages. Multiply by 100 for percentage values. Seed 818 remains zero: its main estimate passes, but its independent check has ESS 3.76. Retain its verified witness and tighten the stored upper bound from about `7.94e-13` to the analytical necessary-event bound `1.2815375916403327e-25`. This is not promoted to a nonzero estimate.

At seed 808: group 16498 zeros fall **35 -> 34** (17 impossible, 17 reachable without estimates); current 16653 stays at 61 (48 impossible, 13 reachable); historical 16653 stays at 53 (43 impossible, 10 reachable). The four other snapshots remain entirely positive. Historical 16653 seed 818 still has one undecided cell, Londrina/5th. No inference of intermediate rank reachability was used.

In current 16653, propagated attempts often succeed for cells subsequently estimated by the existing stages, so they add no final nonzero cell. Deferred publication preserves those existing values. In group 16498, the three low-forecast broad candidates release 30,000 draws; the extra propagated candidate spends 8,000, reducing the corresponding total draw allocation by 22,000. This is a reallocation, not an added 8,000-draw allowance.

### Larger offline check

The six independent 100,000-draw targeted streams estimate **2.027e-34 to 2.275e-34**, with ESS 344–694 and relative SE 3.8–5.4%. All pass the coarse gate. Setup took 12.70 ms; the six-stream four-worker batch took 1.23 s. The smaller accepted production estimates are about 1.46e-34 to 2.07e-34, roughly 0.70–0.99 times the larger-run median. This supports the requested order of magnitude. It is an offline workload, never part of the production allocation, and uses the same proposal family: it is not an independent ground-truth estimator.

### Recommendation

Use the funded structural exception instead of globally removing the probability cutoff or displacing one of the four existing candidates. Keep complete support, independent confirmation, analytical bounds on rejected estimates, and the hard setup guards. The local implementation enables this strategy by default; it has not been committed, pushed or deployed. Four-core CPU work decreases on the difficult groups and warm target latency is nearly unchanged, but the small positive latency changes above must remain visible when reviewing this against a strict no-increase requirement. Finite-seed preservation does not guarantee absence of regressions for every future random seed.

## Correctness and tests

The new unit tests enumerate all 729 seasons of a four-team league, for every target/rank, with points-only and points/wins ordering and multiple joint-selection limits. They check the necessary event normalizer and preservation of every rank hit. Incremental endpoint cardinality updates are checked against a full-team recomputation for every candidate outcome at supported prefixes. Guided and unguided weighted estimates agree with exact enumeration. A separate test checks an upper tail of `1e-20` and actual goal tiebreakers against independent score sampling. Exceeding the complete target-pattern limit or joint enumeration budget skips the model instead of truncating it.

A binary integration test compares normal defaults with propagation disabled at seeds 808 and 818. It checks preservation of all old positive probabilities and standard errors, impossibility labels and game importance; 56 -> 9 patterns; 28 forced fixtures; funding; acceptance of seed 808; rejection of seed 818 and its analytical bound. The previous broad regression test explicitly isolates the old allocation to keep its 35,000/43,000 draw assertions meaningful.

Full Rust suite: **70 passed, 2 database-dependent tests ignored**. HTTP tests need permission to bind local ports. No database writes were run. Rails/UI and request/response shapes were unchanged.

## Reproduction

```sh
# Rebuild the shipped baseline in a separate temporary directory.
mkdir -p /private/tmp/golaberto-propagated-baseline-src
git archive 65b71493e738ce434f071e717b0646a0e4c8a842 | tar -x -C /private/tmp/golaberto-propagated-baseline-src
cargo build --release --offline --locked \
  --manifest-path /private/tmp/golaberto-propagated-baseline-src/odds-rust/Cargo.toml \
  --bin golaberto-odds -j4
cp /private/tmp/golaberto-propagated-baseline-src/odds-rust/target/release/golaberto-odds \
  /private/tmp/golaberto-propagated-joint-baseline

cargo build --release --offline --locked --manifest-path odds-rust/Cargo.toml \
  --bin golaberto-odds --example joint_cap_conditioning -j4
cargo test --release --offline --locked --manifest-path odds-rust/Cargo.toml \
  -j4 -- --test-threads=1

python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline /private/tmp/golaberto-propagated-joint-baseline \
  --candidate odds-rust/target/release/golaberto-odds \
  --output /private/tmp/propagated-tail-pairs --production-candidate
python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline /private/tmp/golaberto-propagated-joint-baseline \
  --candidate odds-rust/target/release/golaberto-odds \
  --output /private/tmp/propagated-tail-persistent --production-candidate --persistent

odds-rust/target/release/examples/joint_cap_conditioning \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  5 20 6 100000 propagated
```

To reconstruct the baseline later, build commit `65b71493e738ce434f071e717b0646a0e4c8a842` in a separate checkout, then pass its executable to `--baseline`. Build and benchmarks must run sequentially to respect four cores. Snapshot group 16653 includes both the current canonical request and historical `group-16653-2d1c1d6f.json`. Fixed seeds are 801, 804, 808, 817, 818 and 911. Persistent timing uses seed 808.

The new path is enabled locally by default. `RUST_ODDS_JOINT_PROPAGATION=0` or `RUST_ODDS_JOINT_ALLOCATION=legacy` restores the shipped allocation. `RUST_ODDS_JOINT_CAP_CONDITIONING=0` disables all joint conditioning. Startup logs report effective toggles; planner and sampler logs report setup, sampling, credit and confirmation costs. `swap` remains an experimental allocation mode for inspecting the rejected slot-replacement approach; it is not recommended.

## Sources

- [Existing shared-fixture propagation](../../odds-rust/src/domains.rs), [production sorter](../../odds-rust/src/sort.rs), [previous broad sampler](../../odds-rust/src/joint_caps/broad.rs).
- [New propagated sampler](../../odds-rust/src/joint_caps/propagated.rs), [allocation and deferred publication](../../odds-rust/src/search.rs), [binary regression tests](../../odds-rust/tests/joint_caps.rs).
- [Prior shipped broad-conditioning experiment](2026-09-30-rust-broad-joint.md), [reference snapshots](reference/2026-09-30-hundredfold/inputs).
- [Paired benchmark harness](benchmark_propagated_joint.py), [machine-readable final results](2026-10-01-rust-propagated-joint-results.json).
