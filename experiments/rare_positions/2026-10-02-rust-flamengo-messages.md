# Flamengo / 13th: constraint messages and rare-cell work accounting

## Result and recommendation

The isolated Rust prototype estimates Flamengo/team17 finishing13th in
group16498 in **10/10 fixed seeds**, including five holdout seeds. Final
probabilities range from **4.50e-34 to1.17e-33**. These are probabilities,
not percentages or upper bounds; multiply by100 for percentages. This is a
rough order-of-magnitude result, not a golden reference probability.

Across **70 paired full requests / seven snapshots**, the final prototype
gains **10 cell-runs, one distinct cell**, loses no estimates, and preserves
all reachability/impossibility classifications. Existing probabilities,
standard errors and game importance are identical to the baseline. There
are no additional reachable or impossible cells: the gain is a probability
estimate for an already verified reachable cell.

**Initial recommendation: retain offline pending approval of the cost tradeoff.**
The repeated warm-server check for group16498 measures CPU **+2.48%** and median full HTTP
latency **+4.58%**. The recovered rare-cell gain in group16498 permits only
about **1.18%** more work. Configured operation credit fits that allowance,
but the measured CPU/latency increase does not. Four workers were retained;
no older Go timing or blanket one-second allowance was used.

The user subsequently authorized adoption on 2026-10-02. Production now enables
the tested complete-tree/message policy by default in the coverage profile;
`RUST_ODDS_RARE_TAIL_TREE=0` restores the previous allocation policy. The accepted
CPU and latency increases above remain part of the adoption decision. The
historical prototype is saved as [a patch](2026-10-02-flamengo-messages/prototype.patch).
The user authorized commit and push on 2026-10-02 after adoption validation.
No deployment or DB mutation was performed.

## Production adoption

The selected policy keeps the validated 64-leaf/4m-guide-value tree, 25 pilots
per leaf, 3k independent main/check allocations, three message passes on one
pilot-selected leaf, 10% defensive original outcomes and bounded 16m operation
credit. Priority-prefix confirmations and unused native-check refunds ship as
part of the same switch. Rejected preparation pipelines and experimental runtime
knobs were removed; acceptance gates, stream seeds and weighting are preserved.
The exact memoized initial-mass setup calculation also ships. New startup logs
expose the effective settings and `rust_odds_rare_tail_tree_credit` exposes the
credit cap and award. The archived prototype and raw evidence are immutable.

Adoption validation:

- Release build succeeds. **143 Rust tests pass**, four DB tests are ignored;
  five Python audit/comparison tests pass. Exhaustive small-league tests check
  complete support, failed-split parent retention and exact probability
  restoration after joint/guide message tilting.
- **70 paired HTTP requests / seven snapshots / ten seeds** produce
  byte-identical exports to the validated prototype. Flamengo13 remains positive
  in10/10 seeds with the same4.50e-34–1.17e-33 probability range. There are no
  estimate, error, game-importance or proof-classification differences.
- Four flag-off controls (16498, both16653 snapshots,16982; seed808) are
  byte-identical to the previous production binary. The default is also byte
  reproducible across one/four workers, repeats and logging on/off.
- Reduced confirmation funding passes: reservations stay within the available
  bank. Credit requires an accepted additional cell and cannot exceed the tree's
  reservation,16m units or the conservative proportional cap.

New warm timing checks, group16498/seed808, four workers,10 requests per binary
with two warmups and eight timed requests, serialized with no concurrent builds:

| Comparison | Baseline/candidate CPU seconds | CPU change | Baseline/candidate median HTTP ms | HTTP change |
|---|---:|---:|---:|---:|
| Previous production → adopted default, run1 |31.58 /33.73|+6.81%|1,293.41 /1,397.55|+8.05%|
| Previous production → adopted default, run2 |30.54 /32.12|+5.17%|1,244.29 /1,327.14|+6.66%|
| Validated prototype → adopted default |33.35 /32.22|−3.39%|1,267.19 /1,279.42|+0.97%|

CPU includes all ten requests and server startup; HTTP medians exclude warmups.
The integrated policy preserves the tested work and output, and the prototype
comparison does not show increased CPU from integration. Nevertheless, **the
current observations against previous production are CPU+5.17–6.81% and
HTTP+6.66–8.05%**, higher than the original experiment's+2.48%/+4.58%. Record
this variability explicitly; neither measurement establishes production Xeon
latency or compliance with the original1.18% proportional allowance. No further
proposal/batch budget was added during adoption. Publication was subsequently
authorized by the user on 2026-10-02.

Commands and verified adoption evidence:

- [Commands](2026-10-02-flamengo-messages/adoption/commands.sh)
- [Compact results and source/binary hashes](2026-10-02-flamengo-messages/adoption/summary.json)
- [Raw exports, logs and resource measurements](2026-10-02-flamengo-messages/adoption/raw-evidence.tar.gz)
- [Verified per-file manifest](2026-10-02-flamengo-messages/adoption/manifest.json)

## Count against rare cells

For this experiment, a recovered rare cell means:

```
final probability > 0 AND initial 100k Monte Carlo hits == 0
```

This isolates the estimator's work beyond ordinary Monte Carlo. It includes
point-pool and importance estimates, regardless of their final numerical
value. It excludes positive cells observed by MC and all remaining zeros.
Paired exports must have identical initial MC hits and sample counts.

For each request snapshot, use net gains after subtracting lost rare cells:

```
allowed additional fraction = max(0, (candidate recovered rare cells
                                  - baseline recovered rare cells)
                                 / baseline recovered rare cells)
```

| Group16498 cohort | Baseline rare cell-runs | Candidate | Gains / losses | Additional fraction |
|---|---:|---:|---:|---:|
|Development, five seeds|427|432|5 /0|1.171%|
|Holdout, five seeds|419|424|5 /0|1.193%|
|Combined|846|856|10 /0|1.182%|

There are82–88 recovered rare cells per development request, rather than
roughly380 total positive cells. Finding the same Flamengo13 cell in ten
seeds means one distinct cell, not ten additional positions. Across all
seven snapshots, rare cell-runs rise4,319→4,329; work credit is local to the
affected request, not funded by unrelated groups.

[The audit tool](summarize_rare_cell_budget.py) computes these counts from
paired exports. Its tests check the denominator, subtract regressions and
reject comparisons whose initial MC observations differ.

## Baseline, constraints and bottleneck

- Baseline: master `f0a332b847bd7bea24b00fd63e724850da4fa065`; production Rust
  matches released `31bc8b0ab91a9ae7a5e9493806b2f9943890d500`.
- Baseline binary SHA256:
  `f8db89014089e9db7f06a1f9ba15f4d48e4fc1a68c28127c42b6c64a4c75c1cb`.
- Saved identical requests: groups15902,16413,16498,16982,16983 and two
  group16653 snapshots. Input and candidate binary hashes are in
  [the compact summary](2026-10-02-flamengo-messages/summary.json).
- Local arm64 macOS, Rust1.93.1/LLVM21.1.8. Production Xeon was unavailable.
  No CPU-specific instructions were added.
- Development seeds:808,1669,1993,2281,2293. Holdout seeds:801,804,817,911,1861.
- Alternating, serialized HTTP comparisons, four estimator workers total.

The [preceding complete-tree experiment](2026-10-02-rust-flamengo-tree.md)
identified the difficult proposal. Flamengo's certificate allows at most
61 points/18 wins: all losses, or one draw/nine losses. The target-path mass
2.2772722599753912e-7 is a necessary-event upper bound, not the requested
rank probability. Full mask enumeration needs576 cases/path, exceeding256.

The bounded complete tree retains54 disjoint leaves under the existing
100k joint setup-node and4m guide-value limits. It uses776 nodes and
3,918,630 cumulative charged guide values. Exhausted child construction
retains the complete parent region. No supported event region is omitted.

The primary problem was variance within one fully classified leaf. In R41,
1,452 hits could have ESS4.29 and a43% largest contribution. The original
proposal conditioned a few rivals exactly while only guiding the remaining
rivals. More final draws or a different rank-side split did not reliably
remove these heavy weights.

## Proposal experiment

### Messages from every constrained team

The new proposal uses a fixture factor graph:

- Each active fixture has three outcome values.
- Each team has its leaf-specific final packed points/wins interval.
- A team sends a fixture a message measuring how compatible each outcome is
  with its interval, integrating its other incident fixtures under the
  current messages from their opponents.
- Three synchronous iterations, damping0.5. Final proposal rows are a
  defensive mixture: `Q = 0.1 * original + 0.9 * message tilt`.
- Retune one leaf selected by original pilot standard error; no team, rank
  or leaf ID is hardcoded. Only its pilot is rerun after retuning.

This is a proposal heuristic inspired by message passing for constraint
satisfaction. It does not establish feasibility, infeasibility, convergence
or a football-specific success guarantee.
[Montanari, Ricci-Tersenghi & Semerjian, BP-guided decimation](https://arxiv.org/abs/0709.1667).

Message computations are capped at20m counted convolution/range operations,
with10k nodes for the rebuilt exact joint model. Those charges are additional
to the tree's setup charge and are debited from the same transferred bank.
If numerical construction or the quota fails, retain the original proposal.
A vanished message is never an impossibility proof.

### Correct likelihood ratio

Changing outcome probabilities also changes the exact joint normalizer and
which rivals are selected. The prototype rebuilds both underQ. If the old
leaf normalizer is `raw fixed/domain prior * original joint mass`, the new
one is `raw fixed/domain prior * Q joint mass`.

Every sampled active fixture, including exact joint fixtures, contributes
`original normalized outcome probability / Q outcome probability` to its
weight. Existing guided-draw and conditional-goal corrections are retained.
The production standings sorter determines the actual rank. Each retained
leaf receives positive allocation; main/check batches are fresh after
proposal selection and allocation are frozen.

This follows computable likelihood-ratio importance sampling with complete
event support. ESS and observed variance remain diagnostics, not guarantees
against unobserved heavy weights.
[Owen, Importance Sampling, §§9.1/9.3](https://artowen.su.domains/mc/Ch-var-is.pdf).

Constraint-aware sampling literature also highlights the rejection problem
and the difficulty of correctly weighting search/backtracking samples.
This prototype keeps a computableQ rather than introducing an unknown
backtracking normalizer.
[Gogate & Dechter, SampleSearch](https://proceedings.mlr.press/v2/gogate07a.html).

The estimator for each frozen proposal retains the original probability
measure. Publication still depends on the existing acceptance gates; no
claim of an unbiased estimate conditional on acceptance is made.

### Allocation and deterministic accounting

The complete tree is attempted only after native **case enumeration budget
exhaustion**, rather than repeating every failed or successful constructor.
It uses25 pilots/leaf, a10-draw floor and3,000 draws in each fresh final batch.
Main/check leaves are scheduled together across four workers with independent
deterministic streams. Existing quality gates are unchanged.

The allocation experiments also:

- Skip final tree draws when every pilot has zero event evidence; keep the
  cell reachable/undecided as appropriate.
- Refund unexecuted check reservations. A refund is unused operation capacity,
  not previously spent CPU time.
- Replace conservative first-pilot charges with counted pilot operations.
- Preserve higher-priority confirmation jobs before admitting lower-priority
  full pairs. The previous greedy policy could lose a higher-priority cell
  when a slightly larger bank admitted a lower-priority pair.
- Grant credit only after a tree final pair accepts an additional zero cell.
  Requested credit16m units is capped by tree reservations and a conservative
  proportional fraction of the configured rare-tail/confirmation capacity.

The credit denominator is the number of all initial zero-hit MC cells,
including still-zero/impossible cells. This is an upper bound on the number
of baseline recovered rare cells, so it grants less credit than the measured
denominator. Configured group16498 capacity is2,097,570,000 units; requested
credit is at most0.763%. Actual used reservations, modeled limits, CPU and
wall time are distinct measures. Some individual used-work totals can rise
more because the baseline left capacity unused.

## Screening results and discarded approaches

These screens reused development seeds; they are not independent validations.
All final estimates use the existing main/check acceptance gates.

| Experiment | Accepted seed pairs | Decision |
|---|---:|---|
|Original tree, rank guide + goal tilt,25k finals|3/5|Variance remains high|
|Interval guide + goal tilt,25k|4/5|Useful direction|
|Remove goal tilt|2–3/5|Discard|
|Power the interval weights by0.5/0.75/1.25/1.5/2|2/3/3/2/1 of5|Discard|
|Six message passes,25k finals|5/5|Good quality, too much work|
|Six passes,8k finals|5/5|Compact candidate|
|Three passes,8k/5k/3k finals|5/5 each|Retain3k for paired validation|
|Stronger0.99 mixture / more iterations|1–4/5|Discard; stronger is not uniformly better|
|32-leaf trees / cheaper priorities|0/5 in most arms|Discard|
|48 leaves, softened priority|4/5|Discard coverage loss|
|Thread-local marginal cache|5 gains /0 losses; CPU+6.63%|Discard and remove source|
|Short confirmations for strong point-tilt pilots|No eligible proposals|Discard and remove source|
|Messages on Palmeiras16,5k finals|0/5|Do not generalize the tuning blindly|

Full-request allocation development arms:

| Arm | Gain / loss cell-runs | CPU change,16498 | Median HTTP change | Decision |
|---|---:|---:|---:|---|
|Initial transfer|5 /2|−2.84%|−3.96%|Reject losses|
|Narrow native-setup eligibility|5 /3|−3.10%|−5.79%|Reject losses|
|3k finals|5 /3|−5.44%|−10.03%|Reject losses|
|Refunds / zero-hit skip|5 /2|−1.28%|−2.18%|Reject losses|
|Actual pilot work / priority prefix / credit|5 /0|+0.34%|+4.27%|Quality success; latency remains|
|Setup-only pipeline|5 /0|+2.05%|+4.14%|Reject as performance improvement|
|Warm pipeline / concurrent finals|5 /0|+2.79%|+2.81%|Reject as performance improvement|

A single preparation thread overlaps tree setup/pilots with native work,
temporarily reserving three native workers plus one preparation worker. The
candidate restores four native workers after joining. It is deterministic,
but did not provide a measured cost win. Final validation leaves it disabled.

## Exact setup optimization

The guide-size check already happens before guide construction. The remaining
setup optimization replaces calls that build an entire `TerminalTable` only
to read `mass(0)` with memoized evaluation of the same backward recurrence
from that initial state. It keeps the same outcome addition order and exact
floating-point bits. Full tables are retained wherever actual sampling needs
their rows. No new fixture restriction or probability approximation is used.

Development median Flamengo-tree setup falls **51.40→26.47 ms**, about48.5%.
All70 final candidate exports are exactly identical to the pre-optimization
exports. The modeled setup charge remains conservative; it was not reduced
to manufacture a budget win. This optimization is included in the archived
prototype but is not applied to production source.

## Final paired results and timing limits

Final candidate, seven snapshots and ten seeds:

- Flamengo13:10/10 accepted, probability4.50e-34–1.17e-33.
- Gains:10 cell-runs/one distinct cell; losses:0.
- No reachability or impossibility regressions; no changed existing estimate,
  standard error or game importance.
- Other six snapshots: no additional coverage. Their per-group timings are
  included in the compact summary and raw logs, including increases.
- No tiebreaker simplification, score cap, intermediate-rank inference or
  witness-to-probability substitution was introduced.

| Group16498 timing cohort | CPU baseline / candidate | CPU change | Median HTTP baseline / candidate | Median change |
|---|---:|---:|---:|---:|
|Development, five fresh-process pairs|16.250 /16.636 s|+2.38%|1,390.61 /1,337.34 ms|−3.83%|
|Holdout, five fresh-process pairs|20.748 /18.602 s|−10.34%|1,932.84 /1,620.88 ms|−16.14%|
|Warm servers, ten requests each, eight timed|31.010 /31.780 s|**+2.48%**|1,249.80 /1,307.07 ms|**+4.58%**|

The holdout and some other snapshot timings show pronounced run-to-run
variation: several unchanged requests had large wall-time excursions. Do
not interpret the holdout reduction as a demonstrated full-request speedup.
Warm-server CPU includes all ten requests plus startup/shutdown; the median
wall uses eight requests after two warmups. Both servers were idle except
for the one request being measured. Peak RSS395.91→409.77 MiB in that run.

Warm seed808 preserves all existing probabilities/standard errors exactly,
with one additional Flamengo13 estimate on every repetition. Its cost still
exceeds the rare-cell allowance. The useful result is a reliable proposal and
a cheaper exact setup primitive; a production allocation win remains unproven.

## Reproduction and evidence

The [artifact directory](2026-10-02-flamengo-messages/) contains a patch against
f0a332b8, compact results, commands, raw log/export archive and per-member hash
manifest. Applying the patch to an isolated copy passes `git apply --check`.
The archive is read back and verified against every original member.

See [commands.sh](2026-10-02-flamengo-messages/commands.sh) for source staging,
release builds, serialized four-worker paired runs, holdouts, warm-server
timings and the rare-cell count audit. The source includes experimental knobs
for rejected arms; none is a new production default. The exact lazy table
primitive is active in the isolated prototype even when the new tree is off.

Validation: **141 Rust tests passed, four DB tests ignored; five Python tests
passed**. Four flag-off HTTP controls (group16498, both16653 snapshots and
16982, seed808) return exactly identical exports to the baseline. The isolated
patch reconstructs all ten modified source files byte-for-byte from f0a332b8.
Results are recorded in the artifact summary and raw test logs.
New tests cover complete weighted rank mass on an exhaustively enumerated
small league, positive defensive support, message quota fallback, exact
backward-DP bits and rare-cell accounting. DB-dependent tests remain ignored.
An initial enumeration test tried to formP/Q for an outcome outside a leaf's
support; the harness now skips zero-density outcomes before forming the ratio,
and the corrected exhaustive check passes.

Next useful work: identify failed/low-yield search work that can fund the new
tree while preserving existing coverage; keep the exact setup optimization.
Do not enable a more aggressive generic message policy solely from this cell.
