# R43: certified limits and messages inside lazy proposals

## Result

**Reject this revision; keep the shipped algorithm.** Five focused variants
were screened. The most informative variant recovered Flamengo/12 for seed808,
but the seven-snapshot, ten-seed validation gained **3 cell-runs and lost5**.
Flamengo/12 coverage fell **6/10 →3/10**. Two warm-server comparisons measured
**CPU+9.75–14.67%** and **median HTTP latency+10.46–12.21%**.

The prototype uses the existing work allocations, but does not preserve actual
full-request cost. No additional allowance is assumed. Production source and
its defaults are unchanged; nothing was committed or pushed.

## Baseline and protocol

Baseline: Rust commit `ac92a8ab88656c2e20a24b37c1f3c3d3dcfdb4cd`, including the
newly shipped complete-tree policy. The existing release binary was frozen
before editing. Four estimator workers were used throughout, with requests,
builds and tests serialized. Measurements are local macOS/ARM results; no
production Xeon timing is claimed. Older Go timings were not used.

Inputs: six hundredfold-reference snapshots (16498,16653,16982,16983,15902,16413),
plus the older16653 fixture. Development seeds:808,1669,1993,2281,2293. Holdout
seeds:801,804,817,911,1861. The benchmark alternates binary order and clears
estimator flags. Snapshots, request hashes, binary hashes and raw exports/logs
are archived. No DB reads or mutations were needed.

## What was implemented

The experiment is generic, with no team or finishing-position special cases.
Only unresolved, already-funded lazy proposals qualify: their cached certificate
must remove a previously allowed target total, pilot ESS must be below12, and
the existing independent pair must provide at least60m modeled work units.
Certificates are reused; no proof budget is added.

The first variant rebuilt the proposal after certification. Later variants
instead retained the existing root cache, witness alternatives and guides,
intersected the target dynamic program with the certified limits, removed
excluded cached roots and rebuilt their selection CDF. This avoids discarding
useful witness guidance or repeating marginal setup.

An independent500-draw training batch identifies target paths contributing the
largest weighted second moments. Three fixture/team message passes retune up
to the requested number of paths. On a selected path, the revised sampler mixes
its original broad conditional proposal with a retuned witness alternative.
A separate500-draw pilot compares ESS per estimated operation against the
original training result. Selected proposals get fresh fixed main/check streams.

Preparation is charged against the original pair:

```
allowance = 2 × original_draws × original_estimated_cost
new_draws = min(30000, floor((allowance − preparation_work)
                           / (2 × selected_estimated_cost)))
```

A failed refinement retains the original proposal, with shorter final batches
because preparation was still spent. Earlier committed ordinary/tree estimates
are frozen. Estimates which the baseline would discover later in confirmation
can consequently be lost; preserving earlier committed estimates is not a
promise to preserve the entire baseline output.

### Probability correctness

Messages define a proposal, not a probability or reachability proof. Active
fixture rows retain10% original probability. Exact joint normalization is
rebuilt underQ and every active joint/guided fixture gets its P/Q correction.
Omitted fixtures retain their original conditional law and need no additional
tilt correction. Target-path selection keeps its10% full-support component.

Within a selected target path, the final variant uses
`Q = 0.5 Q_primary + 0.5 Q_message`. Both conditional densities are evaluated,
including the density of the component which did not generate the draw.
Complete outcomes/scores are checked with the production sorter, and score
proposal correction remains in place. Final batches pass the existing gates;
no ESS, maximum-weight or consistency threshold was relaxed.

This follows the computable-density and event-support requirements of
[Owen's importance-sampling chapter, §§9.1 and9.11](https://artowen.su.domains/mc/Ch-var-is.pdf).
The particular messages, allocation and certificates are repository experiments.

## Target support: why64 paths are insufficient

For Flamengo/team17 finishing12th in group16498, the cached certificate has
packed upper bound1873. With points/wins stride29 and its existing18 wins,
that excludes64 points: Flamengo can add at most3 to its current60.

| Final points/wins | Target outcome paths |
|---|---:|
|60 /18: lose every match|1|
|61 /18: one draw|10|
|62 /18: two draws|45|
|63 /18: three draws|120|
|63 /19: one win|10|
|Total|186|

An exhaustive target-outcome test verifies this count and that the dynamic
program preserves every allowed path. These are target paths, not complete
league witnesses; rival fixtures still determine rank. The complete constructor
stops at64 target paths, while the lazy sampler keeps the full dynamic program.

## Five development screens

Each screen pairs five full group16498 requests against current production.
These fresh-process timings are diagnostic and were noisy; warm measurements
below are the cost evidence for the cross-group arm.

| Variant | Gained/lost cell-runs | CPU change | Median HTTP change |
|---|---:|---:|---:|
|Rebuild after certification; retune primary paths|0 /0|+1.72%|+6.97%|
|Reuse cache; retune witness alternatives;10% native conditional component|1 /2|+3.41%|+3.36%|
|Same, with50% native conditional component|1 /2|−5.31%|+3.83%|
|Reuse certified limits without messages|1 /2|+1.68%|+5.60%|
|50% native mixture with interval guidance|0 /2|−2.41%|−4.79%|

All are rejected. Rebuilding loses the helpful existing alternatives. Stronger
concentration increases rank hits but leaves rare, large weights poorly
represented. Certified limits alone change target allocation without solving
the main variance problem. Interval guidance does not rescue the failing cells.

## Paired validation across seven snapshots

The50% native/rank-guidance arm was carried to70 full request pairs, including
35 holdout pairs. Counts below are cell-runs across seeds, not distinct cells.

| Snapshot | Gain /loss | Export comparison |
|---|---:|---|
|16498-44eabb47|3 /5|Changed in all ten seeds|
|16653-71d4fea8|0 /0|All ten byte-identical|
|16653-2d1c1d6f|0 /0|All ten byte-identical|
|16982-9327edcd|0 /0|All ten byte-identical|
|16983-43969b02|0 /0|All ten byte-identical|
|15902-31d66705|0 /0|All ten byte-identical|
|16413-a2eef0a8|0 /0|All ten byte-identical|

Development:1 gain/2 losses. Holdout:2 gains/3 losses. Distinct gained cells:
Flamengo12 and Palmeiras14. Lost cells: Flamengo12 and Palmeiras13.
Palmeiras13's lost seed1861 estimate was6.85e-20, much larger than the recovered
roughly1e-25 events. There are **zero new reachable or impossible cells**, no
proof-classification regressions, and no change in undecideds. All initial MC
hits/sample counts and game importance are identical.

Recovered rare cell-runs, defined as positive final estimates with zero initial
100k-MC hits: group16498 **856→854**; whole cohort **4,329→4,327**. Fresh-process
coverage per CPU in16498 falls about1.95%; there is no proportional work credit
from the net result.

### Flamengo12 estimates

Values are final probabilities, not percentages or upper bounds.

| Seed | Baseline | Candidate |
|---|---:|---:|
|801|0|0|
|804|0|0|
|808|0|6.346e-26|
|817|1.624e-25|0|
|911|1.219e-25|4.130e-26|
|1669|1.610e-25|0|
|1861|1.264e-25|0|
|1993|1.030e-25|1.030e-25|
|2281|1.100e-25|0|
|2293|0|0|

Seed808 is a real success: its baseline main has366 hits, ESS3.984 and maximum
weight share41.79%, so no check runs. The candidate uses4,840 draws per batch,
gets714 main hits, ESS17.08 and maximum share16.17%; its independent check has
ESS6.16. Main/check probabilities are6.346e-26 and1.057e-25. The final matrix
reconciliation changes the last digits of the main estimate slightly.

Seed2281 explains a regression:24,301 baseline main/check draws become14,617.
The candidate main gets2,209 hits, but one weight supplies45.35% of the sum;
it fails the35% main threshold. Seed2293's candidate main passes with ESS9.29,
but its check has ESS1.42 and maximum share83.66%; it cannot publish.

These probabilities are working estimates. The hundredfold reference contains
no scorable positive Flamengo12 reference, so there is no independent golden
number against which to claim numerical accuracy.

## Where the heavy weights actually come from

Diagnostic logging is separate from the timing binaries and does not alter
probability output. For the inspected main/check batches, virtually all weighted
mass is in the **60-point/18-win, all-loss target path**. Higher target totals
are supported but are not the dominant observed contribution.

For the largest candidate weights in seed2281's main and seed2293's failed
check:

- Both belong to the retuned all-loss target path.
- Both lie **outside the retuned witness alternative's constraint support**.
- Their score likelihood correction is exactly1; the large correction comes
  from the outcome proposal.
- Maximum shares are45.35% and83.66%, respectively.

The native mixture component correctly preserves these rival-result modes.
Messages concentrate the selected witness subset; they do not improve its
complement. Small pilots can therefore look efficient while fresh batches
encounter an under-sampled complementary mode. More target paths or stronger
goal tilting would not address the dominant observed failure.

## Cost and allocation

Group16498 fresh paired results over ten seeds: CPU37.29→37.94 seconds (+1.75%);
median full HTTP1,549.24→1,653.45 ms (+6.73%). Median rare-tail stage
942.06→1,056.96 ms. Early proof stage2.177→2.175 ms: this is proposal/allocation
work, not added proof search.32 preparation attempts across those requests total
589.77 ms and622.23m modeled units, averaging58.98 ms and62.22m per request.

Two warm runs use10 requests per binary, two warmups and eight timed requests,
serialized and alternated at seed808:

| Run | Baseline/candidate CPU seconds | CPU change | Baseline/candidate median HTTP ms | HTTP change |
|---|---:|---:|---:|---:|
|1|30.37 /33.33|+9.75%|1,266.24 /1,420.83|+12.21%|
|2|32.99 /37.83|+14.67%|1,357.88 /1,499.90|+10.46%|

Peak RSS is411.11 /408.11 MiB and424.27 /401.80 MiB. CPU includes all ten
requests and startup; HTTP medians exclude warmups. No additional cores are used.

Every audited preparation plus final pair fits its original modeled allowance,
and every confirmation bank stays within its existing cap. That does **not**
make the new work free: successful mains run checks which production skipped;
unused checks and second-wave reservations also change. At seed808 the final
confirmation reservation rises678.52m→979.36m units under the unchanged cap.
Estimated draw costs are scheduling units, not measured CPU cycles. Reallocating
a maximum reservation is insufficient to preserve baseline actual latency.

## Tests, artifacts and next recommendation

-146 Rust tests pass; four DB tests are ignored. Three new tests cover exact
  probability restoration with omitted games, defensive support outside a
  witness subset, and the186 certified target paths. Existing score-tiebreaker,
  complete-parent and operation-budget tests pass.
- Five Python audit/comparison tests pass.
- Four flag-off controls are byte-identical to production.
- Ten diagnostic exports match their earlier expected exports byte-for-byte.
- The enabled variant is byte-identical across four/one workers, repeated runs
  and logging on/off. Worker changes are a correctness check, not a performance
  comparison.
- Source patches reconstruct their recorded hashes; the raw archive has a
  verified per-file manifest. Two incidental Rustfmt-only changes were excluded
  from the focused patch; they change no program logic.

**Next recommendation:** partition a promising target path's primary region
into the witness subset and its complement. For up to six witness-side interval
constraints, an ordered “first violated constraint” partition needs at most
seven disjoint regions: all constraints met, or the first one violated after
its predecessors were met. This covers the heavy-weight modes observed here
without enumerating all186 target paths. Each region needs its own exact
normalizer and fresh probability batches; ties still use the actual sorter.
If a split cannot be completed within setup limits, retain the complete parent.
Pay for this by replacing existing proposal/replay work, and measure actual
request cost. This complementary partition has **not** been implemented byR43.

Artifacts: [commands](2026-10-02-lazy-messages/commands.sh),
[compact results](2026-10-02-lazy-messages/summary.json),
[prototype patch](2026-10-02-lazy-messages/prototype.patch),
[weight diagnostics](2026-10-02-lazy-messages/diagnostic.patch),
[raw evidence](2026-10-02-lazy-messages/raw-evidence.tar.gz), and
[manifest](2026-10-02-lazy-messages/manifest.json).
Progress is recorded in `FUTURE_EXPERIMENTS.md`.
