# R44: partition lazy witness subsets and their complements

## Protocol

Baseline is shipped Rust `ac92a8ab88656c2e20a24b37c1f3c3d3dcfdb4cd`.
Sources and frozen binaries are isolated under `/private/tmp/golaberto-r44`.
The production source and defaults are unchanged. No commit or push is authorized.
Four estimator workers; full HTTP requests alternate binary order and run
serially. Local macOS/ARM timings do not represent measured production Xeon
performance. No older Go timing or additional allowance is used.

A preflight accidentally started before compilation finished and used the
copied R43 binary. That cohort overlapped compilation, did not exercise R44,
and is excluded from all reported measurements. Subsequent benchmarks use
frozen, completed binaries and do not overlap builds or tests.

## Construction and weighting

Select an already-funded unresolved lazy confirmation pair when its training
ESS is below12, a witness alternative exists, and cached certification excludes
a target total still allowed by its old dynamic program. The pair must have
at least60m logical work units. This is generic: no team or rank special cases.
Certification selects experiments; the old target dynamic program is retained.

Choose the cached witness root with the largest existing prior/rank-guidance
score. Compare its primary and alternate intervals to extract up to six
one-sided integer conditions. For conditions C1…Ck, construct:

- C1 violated; later conditions free.
- C1 met, C2 violated; later conditions free.
- Continue through the first violated condition.
- All conditions met.

These k+1 regions are disjoint and cover the parent. Complements use exact
integer thresholds (`>=a` becomes `<=a-1`; `<=b` becomes `>=b+1`). Each region
reruns shared-fixture domain propagation and constructs exact joint sampling
normalizers. Remaining interval/rank checks can produce null draws. A
propagation refutation removes only that region; numerical failure or a setup
quota retains the complete parent and discards the attempted partition.

Region pilots compare guidance modes and learn allocation from weighted second
moments, with a prior floor. Final sampling mixes that partition with the
primary proposal. Both densities enter the full mixture denominator. Native
draws identify their region using actual fixture outcomes, including omitted
games. The target-path mixture still covers uncached paths. Score correction
and the production standings sorter are retained.

Witnesses and proposal normalizers are not final probabilities. Fresh independent
main/check batches must pass the unchanged publication gates. Pilot zeros are
not reachability or impossibility proofs. These density and support requirements
follow [Owen, importance sampling, §§9.1 and9.11](https://artowen.su.domains/mc/Ch-var-is.pdf).

All attempted construction and pilot work is charged to the original pair.
New batch lengths cannot exceed its original draws or remaining work:

```
new_draws = min(old_draws, floor((old_pair_allowance - preparation_work)
                              / (2 * selected_estimated_draw_cost)))
```

Successful checks can still increase actual cost if the baseline would skip
its check. Logical allocation limits do not by themselves establish equal CPU
or latency; those are measured separately.

## Four development screens

Each screen uses five full group16498 request pairs with seeds
808,1669,1993,2281,2293. CPU is summed child-process user+system time; HTTP is
the median of five alternating fresh-process requests. These timings are noisy
and are diagnostic; the warm comparison below is the cost check.

| Variant | Gained / lost cell-runs | CPU change | Median HTTP change | Recommendation |
|---|---:|---:|---:|---|
|V1: existing constructor ceilings;10% native support; serial setup|0 /0|−2.37%|+0.04%|Reject: all10 partition attempts fall back|
|V2: funded constructor reallocation;10% native support; parallel setup|0 /1|−1.65%|−3.67%|Reject: loses Flamengo12/1669|
|V3: same construction,50% native support|0 /1|−2.11%|−0.24%|Reject: loses Flamengo12/2281|
|V4:50% native support plus cardinality pilot option|1 /2|+3.74%|+5.67%|Reject: one recovery, two losses|

V1 clips setup to the existing node/guide ceilings, allowing at most20k more
nodes and1m more guide values if space remains. V2–V4 instead permit at most
60k additional joint-constructor nodes and4m additional guide values per
attempt, charged within that pair's existing confirmation allocation. These
are proposal-construction limits, separate from reachability proof budgets.
No global operation-bank increase is introduced. Setup runs on the existing
four workers and is applied in stable admission order.

Each constructed region receives50 rank-guidance and50 interval-guidance pilot
draws. V4 offers another50 cardinality-guidance draws per region. Choose the
mode with higher observed ESS per logical operation. Allocate regions by
estimated weighted second moment with a10% prior floor, then use a separate
300-draw mixture pilot. A refined proposal is selected only with at least10
pilot hits and improved ESS per operation. Failed attempts still shorten the
original pair because their work was spent.

## Seven-snapshot validation of V4

The development cohort combines the V4 group16498 screen with30 pairs across
the other six snapshots. Holdout adds35 pairs with seeds801,804,817,911,1861.
Total: **70 paired requests, seven snapshots, ten seeds**.

- **One gained cell-run, two lost cell-runs.** All changes are Flamengo/team17,
  finishing12th in group16498. This is one distinct recovered cell across seeds.
- Flamengo12 coverage **6/10 →5/10**. Flamengo13 stays **10/10**.
- Gained seed2293: **P=2.80e-26**; independent check **4.79e-26**.
- Lost seed1669: baseline **1.61e-25**. Revised main maximum-weight share38.27%
  exceeds the unchanged35% gate.
- Lost seed2281: baseline **1.10e-25**. Revised main passes, but check ESS2.54
  misses the unchanged minimum3.
- The other six snapshots produce **60 byte-identical exports**.
- Initial100k MC hits/samples, game importance and every reachability
  classification are unchanged. No additional reachable or impossible cells;
  no undecided/proof regressions. Group16498 remains fully classified, with17
  proven impossible zeros and2–5 reachable zeros depending on seed/arm.
- Positive rare cell-runs (initial MC hits zero, final P positive):
  **4,329 →4,328** across the complete cohort.

All probabilities above are fractions per season, **not percentages**. This
experiment supplies no new golden estimate or accuracy certification for
Flamengo12. Coverage alone does not establish accuracy for an unobserved event.

### Current Rust full-request costs

Fresh-process V4 cohort; medians across ten seeds. Groups16653 use two different
snapshots. CPU changes aggregate ten child lifetimes for each snapshot.

| Snapshot | HTTP baseline → candidate (ms) | Rare-tail baseline → candidate (ms) | CPU change |
|---|---:|---:|---:|
|16498|1555.91 →1580.84|904.32 →906.94|−0.33%|
|16653 /71d4fea8|1202.78 →1206.68|649.80 →677.26|+1.35%|
|16653 /2d1c1d6f|683.55 →685.02|127.39 →129.64|−1.90%|
|16982|262.02 →264.80|stage absent|+3.52%|
|16983|258.14 →261.22|stage absent|+2.50%|
|15902|170.51 →177.39|stage absent|+7.87%|
|16413|162.49 →161.58|stage absent|−0.10%|

The partition hook runs only for group16498 in this cohort. Changes in the
other groups' timings cannot be attributed to partition work; their outputs
are identical. Fresh timings include an outlier in15902, so they do not
establish speed gains or regressions for an inactive feature. Summed CPU
across all70 pairs is96.53 →97.04 seconds, **+0.52%**.

V4 attempts17 preparations, selects6 and completes all attempted partitions.
Summed preparation wall times248.49ms, **14.62ms per attempted cell** or
**24.85ms per affected request**. Parallel timings are sums of task durations,
not extra request latency. Construction and training are measured together;
constructor-only time is not separately instrumented. Modeled preparation
work is206.32m units,20.63m per affected request. Early proof-stage wall time
and proof budgets are unchanged: warm median2.114 →2.139ms. Existing native
proof limits remain100 nodes/cell and3,200 total for constructive witnesses,
and500/cell and10,000 total per joint proof direction (`proof.rs`).

### Warm-server cost and why the bank limit is insufficient

Ten requests per binary, two warmups, eight timed requests; alternating binary
order, serialized requests, four workers. Seed808/group16498:

| Metric | Current Rust | V4 | Change |
|---|---:|---:|---:|
|Median HTTP wall time|1312.50ms|1468.71ms|**+11.90%**|
|Median rare-tail stage|771.76ms|931.82ms|+160.06ms|
|CPU, user+system, all ten requests/startup|31.65s|34.73s|**+9.73%**|
|Peak RSS|425.42MiB|428.61MiB|+3.19MiB|

The full outputs for this warm seed have the same positive coverage. The fixed
bank limit is1,019.22m modeled units in both arms, but actual reserved work is
678.52m →911.21m. Skipped-check refunds fall300.84m →68.15m.

For Palmeiras/team16 finishing15th, the refinement pilot is rejected and its
original sampler retained. Charging setup shortens its main27,060 →26,411
draws. That changes the two-half consistency statistic and makes the revised
main pass publication. It triggers a26,411-draw check which the baseline skips;
that check fails with ESS2.97. Sampling for that cell rises230.51 →466.66ms in
the first warm request, with no new estimate. This directly demonstrates why
preserving configured ceilings does not preserve baseline full-request cost.

## What the experiment teaches

A complete seven-region partition is correct and cheap enough to construct,
but its early complement regions are still broad. For Flamengo12's all-loss
root, their necessary-event normalizers range roughly1.09e-8 down to4.22e-18,
while the all-six-conditions witness region is1.72e-17. These are proposal
normalizers, not rank probabilities; the rank event is much rarer.

Across the five development seeds, rank/interval pilots for the six complement
regions mostly see no hits. Adding cardinality guidance does not rescue those
pilots: their selected modes generally remain rank guidance. The all-met
witness region produces the useful pilot observations. For seed2293, its
cardinality pilot sees10 hits/ESS3.39 and eventually recovers the cell. For
seed808, the final main still has a56.60% maximum-weight share and fails.

Inference: naming complementary modes does not sufficiently condition their
remaining rival outcomes. The measured pilot evidence and failed final gates
support rejecting this partition/allocation policy. They do not prove the
complementary regions impossible or establish their true probability masses.

## Tests and reproducibility

- Full Rust release/all-target suite: **146 passed, four DB tests ignored**.
  Ignored DB tests require `MYSQL_TEST_URL`; no DB reads or mutations were needed.
- Three added tests: exhaustive integer-partition coverage; complete likelihood
  recovery enumerating all81 outcomes in24 scenarios (two ranks, two standings
  layouts, omissions on/off and all three guidance modes); exhausted-child complete-parent fallback.
- Five existing Python benchmark/accounting tests pass.
- Four flag-off exports are byte-identical to current production.
- Enabled4-worker repeat and1-worker/logging-off CLI exports have identical SHA:
  `bc3a0b9454172cb5bab2c732dc548bb090b1e218c411336d415e9f90aac4278a`.
- The six-file prototype patch reconstructs its recorded source hashes exactly;
  earlier-cap overlays apply cleanly. Incidental Rustfmt changes are excluded.

Artifacts: [`2026-10-02-lazy-partitions/`](2026-10-02-lazy-partitions/), including
`prototype.patch`, earlier variant overlays, `commands.sh`, `summary.json`,
source hashes, invariance helper and verified raw logs/exports/test archive.
Use `commands.sh` for isolated builds and paired commands. Reference requests
are the committed hundredfold snapshots plus the older16653 fixture; benchmark
summaries record every input hash and measured binary hash.

## Recommendation and next hypothesis

**Reject all four variants and keep production unchanged.** V4 reduces total
coverage and exceeds current warm request cost. No commit or push was made.

Next test: retain a few distinct **largest-weight verified pilot seasons** per
cached target path and build a small portfolio from their different rival-side
patterns. Use observations from the existing pilots, avoiding new pilot draws,
and evaluate the full overlapping mixture density. Existing R4/R14 already use
witness proposals and rival masks; the additional hypothesis is choosing several
modes by their observed weighted contribution rather than the first matching
witness. It must replace existing work and pass paired coverage/cost checks.
This next hypothesis is recorded in FUTURE_EXPERIMENTS.md and is not implemented.
