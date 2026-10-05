# Future experiments: rare-position search efficiency

Status: active measured campaign; selected configuration remains opt-in and local.
Written October 1, 2026. The original proposals below are preserved; the active
50% campaign later in this file supersedes their earlier time constraint.

## Experimental learning budgets (October 3)

User requested ten-fold budgets when experimenting, to discover attainable
coverage and understand proposal failures. Include an enlarged learning arm
alongside the current-budget comparison. State which setup, pilot, search or
confirmation budgets are multiplied; distinguish a focused cell grant from
the entire request. Compare candidate and native control with equal grants,
fixed seeds and four cores, measure actual CPU and full-request wall time, and
retain acceptance gates. Enlarged experiments do not change production defaults
or establish an additional production latency allowance. R54 is the first
paired application of this instruction.

## Status of the three original proposals (clarified October 1)

The 26 configurations in the later campaign are not 26 completed arms of the
three original proposals. Their current mapping is:

| Original proposal | What was actually tested | Individual evidence | What remains untested |
|---|---|---|---|
| 1. Constraint-based fixture ordering | Related ordering variants in the NEW sampled-target joint proposal | Same nine difficult-snapshot requests: primary ordering 79 gained cell-runs; rare-first 73; uncertainty-first 76. Worst full-request increases 47.4%,47.4%,44.5% respectively versus frozen Rust. No baseline losses. Alternative orderings not selected. | Original lookahead-only A, broad-only B, exact-cap-only C, conditioning-DP ordering, and combined/cached ordering arms. The new sampler's result is not an isolated test of those components. |
| 2. Evidence-based stopping/allocation | Fresh fixed batches funded from pilot hits/ESS/cost in the new portfolio; guide/cache reuse changed at the same time. Existing point-tilt quota reallocations also tested. | Unfunded portfolio 63 gained cell-runs with 82.5% worst latency increase; funded/cache variant 66 with 38.1%, no losses. These are combined-profile results, not a pure stopping ablation. Corrected high-ESS point-tilt quota arm 82 gains but 56.2% worst increase; rejected. Removing point-tilt lost 3 old cell-runs; rejected. | Checkpoints/continuation inside the old samplers' batches, statistically justified sequential-estimation stopping S2, and a clean isolated stopping-only comparison. Main/check estimation batches still have predetermined sizes. |
| 3. W/D/L pool MC with deferred scores | Not tested in the ordinary 100k pool | No measured pool CPU, wall, tie-backfill or coverage result. Conditional goal sampling/tilting in new rare proposals is a different component. | L1 all-fixture backfill, L2 selective backfill, then isolated L3 extension. Original pool/scout goal sampling stays unchanged. |

Portable measurements: `experiments/rare_positions/2026-10-01-rare50/development-arms.json`.
The final 207 gains/63-pair result belongs to the combined rare-tail campaign;
it is not an individual result for any of these three original proposals.

## Objective

Test whether processing the most constrained fixtures first can produce more
accepted rare-position estimates within the current Rust full-request latency
and four-core usage. Replace or reorganize existing work; do not increase draw
budgets, weaken acceptance checks, or assume a one-second allowance.

Sequential importance proposals use approximate guidance. If fixtures involving
teams with tight points/wins requirements occur late, earlier choices can leave
incompatible requirements for a shared match. Processing that match earlier lets
later draws account for its outcome. Ordinary Monte Carlo does not gain event
frequency from this ordering; exact conditioning DP may instead gain smaller
intermediate state sets.

The preliminary propagated-joint experiment for Athletico finishing last in
group 16498 increased hits from roughly 22–37 to 360–405 per 1,500 draws after
ordering remaining fixtures by constraint difficulty. This motivates broader
testing. It does not establish a general improvement in accepted estimates,
effective sample size, or full-request cost.

## Current scope and priorities

| Component | Current ordering | Proposed experiment |
|---|---|---|
| Propagated joint proposal (`odds-rust/src/joint_caps/propagated.rs`) | Constraint difficulty | Retain as a reference; measure its setup and sampling separately |
| Shared lookahead sampler (`odds-rust/src/lookahead.rs`) | Input fixture order | **First priority:** reorder residual fixtures; this serves point-tilt, gap, peer and domain searches |
| Broad joint proposal (`odds-rust/src/joint_caps/broad.rs`) | Input order for residual fixtures | Apply the same ranking in a separate arm |
| Exact-cap joint proposal (`odds-rust/src/joint_caps.rs`) | Input order for residual fixtures | Apply the ranking using its necessary caps/floors in a separate arm |
| Conditioning DP (`odds-rust/src/conditioned.rs`) | Input fixture order | Separate experiment on setup time, peak state count and state-limit failures |
| Reachability solvers | Existing rules based on domain size and points/wins slack | Compare only after sampler results; keep existing branching as the control |
| Neighborhood witness search | Existing mutation strategy | Separate hypothesis: prioritize relevant mutations, rather than directly copying a simulation ordering |
| Scout and pool MC | Input fixture order | Leave out of the ordering experiment; there is no expected statistical coverage benefit |

## Candidate ordering

For each team, estimate the probability of satisfying the relevant remaining
points/wins cap or floor, conditional on information available at proposal setup.
Use wins only when the phase sorting rules permit that prefix. Respect the
remaining fixture outcome domains.

Use the product of the two endpoint probabilities as a fixture score and process
smaller products first. Equivalently, sum negative log probabilities and process
larger scores first. Break ties by original fixture index. These individual
probabilities are ranking heuristics; they are not joint finishing-position
probabilities or impossibility certificates.

Compare two bounded implementations:

1. **Static ordering:** compute one order per search cell/proposal setup.
2. **Cached conditional ordering:** compute an order per existing conditioning
   pattern or propagated-domain cache entry. Include every relevant input in the
   cache key and retain a bounded cache with a safe fallback.

Start with static ordering. Consider per-draw or per-prefix reordering only if
measurements identify a concrete benefit that pays for its additional cost.
Make the ranking helper reusable where the samplers share suitable inputs;
preserve each sampler's distinct conditioning and likelihood calculations.

## Correctness requirements

- Preserve original fixture IDs and indices when changing traversal order. Shared
  matches must remain a single outcome variable and contribute exactly once.
- Build suffix probability tables, prefix bounds, propagated-domain structures
  and compacted-fixture offsets **after** choosing the order. Any cache entry must
  correspond to that same order.
- Preserve every feasible rank path. Ranking alone must not remove outcomes.
  Remove an outcome only through an existing sound necessary constraint.
- Calculate likelihood ratios from the proposal actually used, including
  defensive mixtures, conditioning normalizers and forced-result probabilities.
- Verify complete seasons with the production sorter and actual score sampling.
  Ties on points/wins remain subject to subsequent phase rules.
- A witness is not a probability estimate. Budget exhaustion or a failed proposal
  remains undecided; score caps cannot prove unrestricted impossibility.
- Keep draw counts, independent confirmation streams and acceptance gates fixed
  in the initial comparisons. Compare raw estimates before reconciliation and
  final matrices afterward.

Add exhaustive small-league tests that compare event support and normalizers
across permutations, plus weighted sampling tests against enumerated rank
probabilities. Exercise points-only, points/wins and goal-based tiebreakers,
shared fixtures, forced outcomes and empty/unsupported proposals.

## Experimental arms

Freeze the current Rust executable and source/configuration before implementing
these arms. Use the implementation current at experiment start, including any
accepted changes from the ongoing propagated-joint work; do not silently use an
older baseline.

| Arm | Change |
|---|---|
| Control | Frozen current Rust defaults |
| A | Static ordering in shared lookahead only |
| B | Static ordering in broad joint residual sampling only |
| C | Static ordering in exact-cap joint residual sampling only |
| D | Combine individually successful sampler changes |
| E | Cached conditional ordering in the best-performing component |

Keep allocation and the rare-cell cutoff identical in all initial arms. Changing
those at the same time would obscure whether ordering helped. If ordering saves
measurable work, run a second comparison that reallocates those savings to
additional cells while preserving the measured full-request budget.

## Requests and seeds

Use all six active requests in
`experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/`:

- Group 16498: `group-16498-44eabb47.json`.
- Group 16653 current: `group-16653-71d4fea8.json`.
- Groups 16982 and 16983: `group-16982-9327edcd.json` and
  `group-16983-43969b02.json`.
- The added groups 15902 and 16413: `group-15902-31d66705.json` and
  `group-16413-a2eef0a8.json`. These have reopened results and serve as controls
  where the matched pool already populates every cell in the tested seeds.

Also include historical 16653 from
`odds-rust/tests/fixtures/group-16653-2d1c1d6f.json` for allocation regressions.

Use paired seeds `801, 804, 808, 817, 818, 911`, identical request bytes and four
workers. Before tuning, reserve an additional held-out seed set, for example
`1201, 1213, 1229`. Record input/source/binary hashes and effective flags. Clear
inherited estimator flags so the baseline is unambiguous. No database mutation
is needed.

## Measurements

Run complete requests serially, alternating control/candidate order. Do not run
two four-worker requests or a compilation alongside a timing measurement.
Measure cold requests and persistent HTTP servers separately. For persistent
timing, use two warmups and at least eight timed pairs per snapshot.

Report:

- Gained and lost nonzero cells for each seed; distinguish unique cells from
  repeated cell-runs. Identify reachable zeros and undecided zeros separately.
- New or lost reachability witnesses and impossible classifications.
- Accepted estimates, verified hits, ESS, relative SE, maximum event-weight
  share, batch agreement and independent-check outcomes.
- Changes in existing estimates relative to reported uncertainty, raw versus
  reconciled results, and quality against the available golden subset. Tiny
  events without independent reference evidence must remain explicitly unverified.
- Ordering/model setup, suffix/cache construction, sampling, each affected stage
  and full-request wall time. Do not sum overlapping parallel stage durations.
- Process CPU, peak RSS, cache usage and DP state counts. Draw counts are not a
  substitute for CPU measurements.
- Coverage per unit of full-request time, paired timing differences and their
  variability. Include inactive controls and any latency increases.

## Adoption rule and deliverables

Adopt each component independently only when paired and held-out runs demonstrate
useful additional coverage or a measurable cost reduction, without unexplained
losses of existing estimates or proofs. A higher hit count alone is insufficient.
Reject changes that improve one team by displacing another team's estimate.

Require evidence that the change fits the current full-request latency and
four-core budget. Report every observed increase explicitly; if neutrality is
uncertain, retain the arm as experimental rather than assuming extra allowance.
Reallocation must be backed by measured savings, not an unused nominal draw cap.

Save an experiment report under `experiments/rare_positions/`, machine-readable
paired results, effective configuration, reproducible commands and relevant
tests. Record rejected arms and regressions as well as improvements. Commit,
push or change production defaults only with the user's authorization.

## Experiment 2: stopping and allocation based on observed results

### Objective and current behavior

Test whether evidence-based stopping can release work from cells that already
have sufficient evidence, or from ineffective proposals, and redirect it to
unresolved cells within the current full-request latency and four-core budget.

The current implementation already adapts between completed batches: observed
hits can skip later zero-cell searches, pilot results select larger searches,
and acceptance or disagreement can change fallback allocation. The ordinary MC,
lookahead and joint-proposal sampling loops generally finish their allocated
draw counts before summarizing evidence. This experiment extends the existing
adaptation to checkpoints inside those allocations and evaluates the overall
continuation policy, not just a single sampler.

### Candidate policies and comparison arms

Freeze the implementation current at experiment start. First test this
independently of fixture ordering; combine successful changes afterward.

| Arm | Change |
|---|---|
| Control | Current full batches and existing continuation rules |
| S1 | Smaller pilot checkpoints; use evidence to select or abandon proposals, followed by a fresh fixed-size estimation batch |
| S2 | Checkpoints during estimation; stop on sufficient evidence using a statistically justified sequential procedure |
| S3 | Bounded futility stopping for proposals with poor observed progress; preserve fallback opportunities |
| S4 | Reallocate measured savings from successful stopping policies to additional unresolved cells |

Pre-register checkpoint sizes, minimum draws and continuation criteria before
tuning. Compare a small set of chunk sizes appropriate to each sampler's current
batch budget. Initially retain the same per-cell maximum draws, acceptance
requirements and existing allocation priorities. Evaluate hits, effective sample
size, relative standard error, maximum weight share and stability; hit count
alone must not trigger success. A futile proposal is not proof that a position
is impossible or that its probability is zero.

Keep broad scout/pool MC fixed in the initial experiment: it serves many cells
and the pooled point-position model simultaneously. Stopping it because some
cells are well estimated can damage rare-cell coverage elsewhere. Any later
scout/pool stopping arm must evaluate those shared outputs explicitly.

### Statistical and implementation requirements

- Repeatedly checking a fixed-sample standard-error gate does not automatically
  preserve its statistical properties. Stopping at the first favorable result
  can bias an estimate and invalidate nominal uncertainty.
- S1 separates adaptive proposal selection from estimation: conditional on the
  selected proposal, use fresh randomness and a predetermined estimation size.
  Measure any selection/publication effects across repeated requests.
- S2 requires a justified stopping procedure, such as a valid confidence
  sequence under established weight bounds, or an explicitly validated
  alternative. Do not apply an unweighted binomial rule to importance weights.
  If the required assumptions cannot be established, retain S1 as the safe
  comparison and keep S2 experimental.
- Retain independent confirmation. It can detect disagreement but does not by
  itself eliminate stopping or publication bias.
- Continue each cell's RNG stream and accumulated statistics across chunks.
  Keep the proposal fixed within an estimation run initially. Changing proposals
  requires correct likelihoods for every draw and a compatible variance method.
- Maintain stable per-cell streams so reallocation does not gratuitously change
  unrelated cells. Define checkpoint/batch-agreement statistics explicitly;
  the existing equal-half diagnostic cannot be copied blindly to arbitrary
  stopping times. Retain state across chunks and measure checkpoint overhead.
- Add focused tests for chunked versus uninterrupted fixed-size runs, correct
  accumulation including zero-weight draws, independent streams and maximum
  budgets. Validate stopping behavior against enumerable small leagues and
  available high-budget references across repeated independent seeds.

### Measurements and adoption

Use the requests, paired seeds, held-out evaluation and timing protocol above.
Run both a savings-only comparison and a savings-reallocation comparison. Report
draws actually avoided, checkpoint/setup overhead, CPU, stage and full-request
wall time, stopping reasons, new and lost estimates, and reachable/undecided
zeros. Compare estimate bias, error, uncertainty calibration and publication
frequency against the control and reference evidence; include rejected estimates
when analyzing selection effects. Preserve existing reachability classifications.

Adopt only if saved work produces useful coverage or lower cost without
unexplained regressions, while preserving statistically defensible estimates.
Report any latency increase explicitly. Savings in nominal draws are not an
additional wall-time allowance. Do not implement a first-hit stopping rule or
weaken reliability gates merely to increase the number of nonzero cells.

## Experiment 3: W/D/L Monte Carlo with deferred scoreline sampling

### Objective and current behavior

Test whether the ordinary 100,000-season pool can run faster by sampling fixture
W/D/L outcomes first, then generating scorelines only when later tiebreakers
require them. Preserve the existing probability model, sample count, pooled
point-position outputs and four-worker limit.

Currently `odds-rust/src/pool.rs::scout` samples two Poisson goal totals for every
remaining fixture in every season. The rare-position sampler in
`odds-rust/src/conditioned.rs::fast` already checks points/wins before sampling
conditional scorelines when the target rank needs a tiebreak. That existing
conditional-score infrastructure in `odds-rust/src/sampling.rs` can be reused.

The initial scout in `odds-rust/src/api.rs` also records exact scoreline bins for
game importance. Keep its full score sampling in the initial experiment;
preserving that output requires scores even when standings do not need them.

### Candidate implementation and correctness

1. Precompute each fixture's W/D/L probabilities from the same Poisson model.
2. Sample one shared outcome per fixture and accumulate points, wins and any
   other ranking information determined by that outcome.
3. Identify unresolved comparisons using the actual phase's ranking rules.
   Use wins only where those rules allow it, and do not skip an earlier
   score-dependent criterion to apply a later outcome-only criterion.
4. Backfill the fixtures needed for goal-based or head-to-head comparisons,
   sampling each scoreline **conditional on its previously sampled outcome**.
5. Complete the standings ordering and collect the same rank and point/rank
   counts as the current pool.

Backfill may require all remaining fixtures involving a tied team, including
matches against teams outside the tie. Winning and losing scorelines matter for
goal difference too. Head-to-head rules can require a different fixture subset;
follow the production sorter's semantics rather than assuming points/wins/GD.
Cache each generated scoreline for that season so both teams and repeated
comparisons see the same result. Preserve played scores and starting campaign
totals. Fall back to full score sampling for unsupported rule combinations.

Unconditional Poisson backfill, canonical 1–0/0–0 scores used in goal comparisons,
or independently sampled scores for the two teams would change the model.
Preserve importance weights when extending deferred sampling to rare proposals.
Use stable separate outcome and score RNG streams where practical. The new
sampler need not reproduce the old seed's exact seasons, but must preserve their
distribution and remain reproducible under its own fixed seeds.

### Expected savings and comparison arms

For group 16653's 85 remaining fixtures, 100,000 seasons currently require about
17 million random draws for goal sampling. W/D/L sampling needs 8.5 million;
conditional score backfill adds approximately one draw per backfilled fixture.
If fraction `f` of fixtures need scores, the total is approximately
`8.5 million × (1 + f)`. At `f = 0.2`, that is 40% fewer fixture-sampling draws,
**not a measured 40% CPU or full-request speedup**. The current Poisson lookup is
already fast, and conditional-score lookup, tie discovery and sorting add cost.

| Arm | Change |
|---|---|
| Control | Current ordinary MC with two Poisson samples per fixture |
| L1 | W/D/L pool MC; backfill all fixtures if any score-dependent tie remains |
| L2 | W/D/L pool MC; backfill only fixtures needed by unresolved comparisons |
| L3 | After pool results, reduce score backfill in existing rare samplers separately |

L1 isolates the basic implementation and distribution check; it may save little
if almost every season has a tie somewhere. L2 tests whether selective backfill
pays for its bookkeeping. Freeze the current implementation and test this
independently of ordering and adaptive stopping before combining successful arms.

### Validation, measurements and adoption

Use all six active golden requests, paired and held-out seeds, alternating timing
order and the four-worker full-request protocol above. Keep sample counts,
allocation and acceptance gates unchanged. Report:

- Fraction of seasons needing scores, fraction of fixtures backfilled and tie
  group sizes, separately for each phase's ranking rules.
- Outcome sampling, conditional score lookup, bookkeeping and sorting costs;
  pool-stage and full-request wall time, process CPU and peak RSS.
- Raw rank and point/rank distributions, matched-pool estimates, new/lost
  nonzero cells and downstream allocation or reachability changes.

Add small-league comparisons against enumerated W/D/L assignments with integrated
conditional scores or an independent high-budget scoreline reference. Exercise
points/wins, goal difference, goals scored, away goals and head-to-head rules,
shared fixtures and repeated comparisons. Check row/column sums, conditional
score distributions and deterministic replay. Statistical comparisons must allow
different individual seasons under the same numeric seed.

Adopt only if selective backfill preserves distributions and existing outputs
while reducing measured full-request cost across representative datasets. Record
tie-heavy regressions and keep full sampling as a fallback. Reallocate savings
to rare-cell searches only in a subsequent measured experiment.


# Active campaign: 50% additional full-request time (2026-10-01)

This section tracks the user's new authorization and supersedes the earlier
no-additional-time constraint **for this campaign**. Baseline is the current
Rust implementation, including propagated joint conditioning and funded work
transfer, frozen at experiment start. Four cores remain the limit. Candidate
full-request latency may be at most 1.5 times its paired baseline; measure
actual CPU and wall time rather than interpreting additional draws as free.
Preserve existing positive estimates and impossibility/reachability proofs.
The objective is to minimize **reachable/undecided cells without estimates**;
proven impossible cells must remain zero. No commit/push is authorized.

Baseline binary: `/private/tmp/golaberto-rare50-baseline`.
Baseline manifest: `/private/tmp/golaberto-rare50-baseline.json`.
Development seeds: 801, 804, 808, 817, 818, 911.
Reserved held-out seeds: 1201, 1213, 1229 (evaluate after selecting arms).
All six current snapshots plus historical 16653 remain the reference cohort.

## Progress ledger

| ID | Hypothesis / experiment | Status | Evidence / next action |
|---|---|---|---|
| R0 | Audit every final reachable/undecided zero: model eligibility, support limits and sampler quality | Complete (seed 808) | 40 residual zeros across three difficult snapshots: most 16498 cells exceed even 256 target paths. Relaxed union produces promising pilots for 12/18, 125/11–13, 2064/18, 70/17, 95/6 in current 16653, and 9/14, 95/1 in historical 16653. Pilot acceptance is not a published estimate. |
| R1 | Run new sampling on actual residual zeros after baseline stages, allocating multiple candidates within the +50% budget | Complete; superseded | Experimental `RUST_ODDS_RARE_TAIL=union`: 1,500 pilot draws, up to eight fresh 8,000-draw estimates plus independent 4,000-draw checks; baseline work unchanged. |
| R2 | Constraint-difficulty ordering in the shared lookahead sampler (original Experiment 1, Arm A) | Deferred | Rebuild all suffix/domain structures after ordering; exact-support tests; track gains and regressions |
| R3 | Stronger complete joint conditioning: larger target unions / reduced minimum forced fixtures, bounded setup | Complete; combined | Union cap 256, feasible roots 128, minimum forced fixtures zero. Audit rejects entire oversized unions. Next test sampled roots with an explicit defensive fallback and exact mixture weights. |
| R4 | Pilot-selected defensive mixtures of witness/product proposals, with fresh fixed estimation and confirmation | Complete; combined | Explore separated event modes instead of intensifying one proposal; whole-mixture P/Q |
| R5 | Fixed-level splitting / sequential Monte Carlo for a sequence of necessary rank constraints | Native temporal arm measured; rejected | No complete pilot seasons; needs stronger residual guidance before branching; independent root replications for uncertainty |
| R6 | Reallocate old work using evidence: smaller independent pilots, stop training ineffective proposals, retain fixed estimation batches | Complete; combined | Existing Experiment 2; avoid first-hit stopping and optional-stopping SE errors |
| R7 | Deferred score sampling in ordinary pool / rare proposals (original Experiment 3) | Deferred | Preserve broad pool and game importance; need statistical comparisons when RNG seasons change |

## Research notes

- Glasserman, Heidelberger, Shahabuddin & Zajic (1999),
  [Multilevel Splitting for Estimating Rare Event Probabilities](https://business.columbia.edu/sites/default/files-efs/pubfiles/4273/multilevel_splitting.pdf):
  reliability/queueing examples motivate spending work on promising partial
  paths and retaining independent descendants with their proper weights.
  Splitting factors must avoid population explosion; compare work-normalized
  variance and measured cost. Candidate adaptation here: a fixed sequence of
  necessary rival-rank constraints with independent particle-root replicates.
- Walter (2015),
  [Rare Event Simulation and Splitting for Discontinuous Random Variables](https://arxiv.org/abs/1507.00919):
  continuous-score splitting assumptions can fail on discrete or mixed scores;
  corrected estimators also cover SAT. Soccer points/wins and rank margins are
  discrete, so ordinary adaptive-quantile splitting cannot be copied directly.
- Botev, Chan & Kroese (2011),
  [The Cross-Entropy Method for Rare-Event Simulation](https://people.smp.uq.edu.au/DirkKroese/ps/WSC_BCK.pdf):
  flexible mixture proposals can represent multiple failure regions. Candidate
  adaptation: witness-derived fixture product components with a defensive base
  component; train on pilots, then freeze the whole mixture and estimate with
  fresh streams and exact mixture likelihoods.

Research-derived choices are hypotheses, not guarantees for football. Record
per-arm commands, configuration/source/binary/input hashes, cell lists,
acceptance diagnostics, timing increases and rejected approaches in the ledger
as results arrive. Record repeated cell-runs separately from distinct cells.

### R1/R3 first paired results

Nine four-core full-request pairs (seeds 801, 808, 818; three difficult snapshots),
`RUST_ODDS_RARE_TAIL=union`, frozen current Rust baseline. Current 16653 gains
4/5/4 cells; historical 16653 gains 1/2/1; 16498 gains 0/0/2. No lost positives
or impossibility changes. Paired latency increases 3.1–20.6%; raw evidence
`/private/tmp/rare50-union/summary.json`. These are 19 additional **cell-runs**,
not 19 distinct cells. Continue developing sampled-root conditioning for the
large unions; retain this arm as the first working coverage improvement.

- Blanchet & Lam (2012), [State-dependent importance sampling for rare-event simulation](https://www.columbia.edu/~khl2114/files/1-s2.0-S1876735411000250-main.pdf): queueing, random-walk, insurance and graph-counting examples show why matching a likely failure path alone can leave poor variance. Candidate adaptation: conditional fixture proposals updated from the remaining rank constraints, with a defensive component and independent confirmation. No asymptotic efficiency theorem is claimed for this estimator.

### Sampled-root prototype / R8

R3 sampled-root prototype retains a 10% exact target-event defensive component;
cached roots use propagated fixture domains and exact shared-rival conditioning.
The target-root mixture probability is evaluated across both components, and
uncached roots use ordinary residual simulation (never dropped). Fresh sampling
does not grow its cache. Three-team exhaustive probability checks pass for empty,
partial and full caches. Seed-808 3,000-draw pilots find 15/20 and 74/20 in 16498
with ESS 335 and 86; 8/20 has ESS 8.5 but fails max-weight-share. Current 16653
2064/17 and 70/16 also show useful pilots, excluded by the complete-union cap.
These are diagnostic pilots, not production estimates.

| R8 | Rank-cardinality completion guidance: approximate the probability of the required number ahead/behind using a Poisson-binomial distribution over rival packed standings; correct every fixture with P/Q | Complete; selected | Endpoint probabilities and remaining fixture marginals update after every draw. Independence and half-weighted ties are proposal heuristics only; production sorter decides hits, defensive support retained. |

### R3/R8 unrestricted portfolio result (rejected timing)

Nine paired full requests, seeds 801/808/818: 61 new cell-runs, no lost positives
(16498 8/7/3, current 16653 8/8/7, historical 16653 7/8/7). Most of the remaining
zeros are now concentrated in cells with weak/no hits. **Not eligible for adoption**:
16498 increased 49–82%, two runs over the allowed 50%; some 16653 pairs also
slightly exceeded it. Evidence: `/private/tmp/rare50-portfolio/summary.json`.
Next: reuse identical suffix guides across root patterns, rank-weight target roots,
and reserve fixed fresh main/check batches based on independent pilot hit/ESS rates
and measured cost, within 45% of baseline elapsed time (5% reconciliation margin).
R6 allocation is therefore Running. Counts chosen before final sampling; no sample
batch is stopped when it first obtains a desired estimate.

### R6 / reused-guide funded portfolio

The same nine paired requests now gain **66 cell-runs** (16498 8/7/4;
current 16653 8/8/8; historical 16653 7/8/8), with zero lost positives.
Paired full-request latency increases **17.0–38.1%**, within allowance in
every pair. Unchanged fixture-probability suffix guides are shared across roots;
point/win bases and constraints remain per root. Fixed final sample counts are
chosen from independent pilot hit/ESS rates and projected cost. Model training
and final allocation get 45% of baseline elapsed time, leaving reconciliation
margin. Evidence `/private/tmp/rare50-funded/summary.json`.

Next R4: witness-directed selection of difficult rivals even when their individual
point bounds are not logically forced. These restrictions are proposal-only. An
inner ordinary-residual defensive mixture retains every legal rank event and
evaluates the whole mixture density, including replaying guided likelihoods
when a base draw also belongs to the restricted proposal. Failed proposal paths
contribute zero; never reinterpret arbitrary proposal infeasibility as a proof.

| R9 | Goal-tiebreaker importance sampling and marginalization: tilt conditional score margins only when packed points/wins leave a tie, with exact exponential-tilt normalizers and a defensive goal mixture | Complete; selected | Same score support as existing finite numerical Poisson tables; production sorter decides every tied hit. No unbounded impossibility claim. Outcome likelihood and score likelihood both included. |

### R4 warning from cross-proposal comparison

The first witness-restricted defensive experiment gains 8 cells per snapshot in
seed 808, including 125/10, but **reject it as a sole proposal**: it reports
70/16 around 1e-17 whereas the broader sampler consistently sees ~1e-12, and
historical 95/3 around 1e-18 versus ~1e-15. Ordinary-MC defense has full support
but fails to see the common rare-event modes at this budget; standard errors
and same-proposal checks can miss those modes. This is a practical variance
problem, not permission to treat a witness as probability. Revised R4 uses a
50/50 mixture of the original propagated guided proposal and witness-restricted
proposal, evaluating **both** guided densities on every completed path.
Evidence `/private/tmp/rare50-subset/summary.json`.

R9 seed-808 run gains 8/9/7 cells (16498/current16653/historical16653), with
21–42% latency increases. Its score-tilt kernel passes an exact conditional
rare-GD comparison. Goal tilts improve 95/5 in current16653; outcomes still limit
95/3 and 125/10. Wider finalist eligibility is funded by the same time allocator.

### Corrected R4 guided mixture results

Nine paired requests: gains 4/5/6 in16498, 8/11/10 current16653, 8/10/8 historical16653
(total70 cell-runs), no baseline losses; full-request increases 28–46%. Historical
16653 seed808 has **zero reachable or undecided zeros** (all43 remaining zeros
proved impossible). Current 16653 seed808 has just two reachable zeros. The
mixture costs coverage in16498 relative to the primary-only funded portfolio,
so next test adds alternate modes only when a primary pilot has ESS below12;
retain the primary proposal when its alternate pilot performs worse. This
reallocates setup and replay cost to cells with measured difficulty.
Evidence `/private/tmp/rare50-multimode/summary.json`.

R2 related test in the NEW sampled-target joint proposal: rare-tail versus
uncertainty ordering of residual fixtures, with
all suffix/guide structures rebuilt after the change; no sorting-state or
likelihood reuse across mismatched orders. R10: compute small Bernoulli tails
directly rather than subtracting nearly-one CDFs, and reuse cardinality scratch
arrays. These are numerical/performance improvements to the same weighted
proposal, not changes to proof logic.

R2 related new-joint uncertainty-first ordering gains 76 cell-runs in nine pairs, within a 45%
latency increase, but again has no gains for 16498 seed 801. Do not select a
global fixture-order change from this result.

R13 repeated propagation fails its first full-request test: 16498 seed 808
increases **106%** and gains no estimates because its pilots consume the
allocation; current/historical 16653 gain only 3/5 instead of 10/8. Reject global
inside-draw propagation at this budget. Evidence:
`/private/tmp/rare50-dynamic-valid/summary.json`. The earlier run in
`/private/tmp/rare50-dynamic` is explicitly INVALID (old binary after a failed
build); exclude it from analysis.

| R11 | Replace ordinary residual fallback for uncached target paths with a complete rank-guided residual proposal | Complete; off | Exact target-event mixture weights unchanged; necessary prefix rank masks plus defensive fixture support, followed by conditional score weighting. No uncached path is omitted. |

R10 additionally reuses mathematically identical joint profiles across roots:
identical variable fixture probabilities and clipped relative point/win bounds
imply the same conditioning normalizer. Failed construction attempts are
charged to the shared node budget. Cache hits remain usable after exhausting
the new-construction budget.

### Adaptive mixture results

Nine paired requests add **79 cell-runs**, with no baseline losses:16498 gains
8/8/8,current16653 8/10/11,historical16653 8/8/10. All paired latency increases
21–48%; seed818 historical16653 has no reachable/undecided zeros remaining.
Evidence `/private/tmp/rare50-adaptive/summary.json`. This is currently the best
measured aggregate coverage within the allowance. Continue testing fixture order
and candidate prioritization before held-out validation. Construction-node
budgets now meter failed attempts too; goal/cardinality scratch buffers are
reused to reduce allocator cost.

R2 related new-joint rare-first ordering gains73 cell-runs in nine pairs, within47.4% latency
increase, but group16498 seed801 gains none. This is weaker than the adaptive
primary-order portfolio on aggregate. Uncertainty-first ordering is the next
paired control.

| R13 | Repropagate shared fixture domains after blocks of partial outcomes, carrying new forced results into the weighted proposal | Complete; rejected | Every eight variable fixtures, recompute necessary packed-rank domains from the current state and retain previous masks. Replays recompute the same masks for mixture densities. No model feasibility is interpreted as a reachability proof. Measure cost: propagation may outweigh saved rejected seasons. |


| R14 | Condition on disjoint sets of rivals strictly above/below the target; propagate the complement into shared fixture domains | Complete; off | Enumerate all small cardinality unions, otherwise use verified witness sets. Keep 10% primary guided defense for complete unions and 50% for partial unions; evaluate full mixture likelihoods. Tied points/wins remain in the weak complement. Cases are proposals, never probability estimates or new impossibility proofs. |

R14 targets the few exceptions to an otherwise highly constrained rank. It
can compact newly forced fixtures before simulation, rather than repeatedly
running domain propagation inside every draw. Exhaustive small-league tests
compare complete and deliberately incomplete case mixtures to enumerated
shared-fixture probabilities in both rank directions.


R14's first three full requests gain5/10/8, versus the adaptive portfolio's
8/10/8 for seed808. Extra setup consumes final allocation in16498; only one
selected case proposal improves a pilot (5/18). Defer broad cardinality-case
setup under the current budget. Evidence `/private/tmp/rare50-cases/summary.json`.

| R15 | Reallocate the expensive existing point-tilt stage to the final joint portfolio | Complete; rejected | Keep early MC/proofs/search/witness reuse, disable point-tilt estimates, and compare all existing positives for regressions. Any lost earlier estimate rejects global replacement. Budget measured against the frozen full Rust request, not against a one-second allowance. |

R0 stage accounting at seed808: point-tilt consumes333ms of the~665ms16498
request;193ms of current16653~690ms;230ms of historical16653~644ms. The
100k pool+scout costs~100–115ms. Reallocation of this stage is therefore a
larger opportunity than squeezing another few milliseconds from proofs.


R15 global replacement is rejected: nine pairs gain69 cell-runs but lose
Fluminense/18th in16498 seeds808 and818, and Bahia/19th in818. Faster timing
is insufficient when earlier positives regress. Keep the stage and test
reallocating its final samples instead. Evidence `/private/tmp/rare50-replace-tilt/summary.json`.

| R16 | Smaller exact rival block (four instead of six) to spread setup nodes across more target roots | Complete; selected | Conditional distribution and normalization remain exact for selected rivals; remaining bounds use full-support guided IS. Test whether setup savings exceed added sampling variance. |
| R17 | Allocate existing point-tilt final samples from pilot ESS, targeting40 effective samples, with fixed fresh batches | Complete; rejected | Clamp3000–30000; preserve independent crosscheck, same error/confirmation gates. Selection pilots are unchanged. Paired comparisons must detect any lost old cells. |


R16 four-rival setup gains77 cell-runs in nine pairs, no lost positives,
26–44% paired latency increases. Comparable coverage to the six-rival adaptive
arm with lower variance in setup time. Evidence `/private/tmp/rare50-four-rivals/summary.json`.

R17 first allocator gains85 cell-runs without losses but violates allowance in
16498 (+48–72%). Some low-ESS candidates received more final draws than
previously, and changed early estimates triggered115ms of peer rescue in seed808.
Reject that allocator; revised variant only reduces batches when pilotESS>=40,
keeps low-ESS batches unchanged, and returns to the45% tail budget.
Evidence `/private/tmp/rare50-funded-tilt/summary.json`.

| R18 | Smoothed fixture outcome frequencies from verified seasons, mixed50/50 with the original guided path proposal | Complete; off | Freeze fit after pilots; exact replay of both proposal likelihoods. Only retained if a separate pilot improvesESS. It changes proposal probabilities, never fixture/event probabilities. No setup enumeration is needed for this mode. |


| R19 | Remove rank-irrelevant fixtures from guided loops | Complete; selected | Omit a variable fixture only when both endpoints are strictly on a fixed side of the target for every remaining outcome and their interval constraints are slack for the full range. Sample omitted outcomes independently from their masked true distribution, preserving the complete verified season and exact mixture replay. Tied rivals and active bounds are never omitted. |

R19 treats guaranteed-above/below teams as fixed cardinality contributions.
Their exact final points are unnecessary inside the proposal loop, provided
no active interval constraint depends on the omitted results. Canonical
representatives maintain those strict classifications; final standings use
the independently sampled actual results. This is a proposal-loop optimization,
not a relaxation of event verification.


R18 witness-fixture fit gains74 cell-runs, no losses, but 16498 seed801 exceeds
allowance (+58%) and aggregate coverage is weaker than the four-rival arm.
Reject broad witness fitting in this budget; retain the experimental flag.
Evidence `/private/tmp/rare50-witness-fixtures/summary.json`.

| R20 | Spend final sampling on order-of-magnitude coverage rather than35% relativeSE | Complete; selected | Fresh fixed main/check batches. Main requires>=30 hits,ESS>=4,relativeSE<=60%,maxweight share<=35%,split-batch gap<=1.5; check>=30 hits,ESS>=3,relativeSE<=75%,main/check within a factor of 5. Not a confidence guarantee. Compare against independent golden IS; reject gross order errors. |

R20 targets10 pilot-predicted effective samples instead of20 and lowers
minimum main batch1000, maximum6000. Existing estimates keep their prior
quality gates. This experiment explicitly trades precision for coverage only
for final remaining zeros, reflecting the user's order-of-magnitude objective.
No upper bound or witness is converted into a probability estimate.


R19 omission gains78 cell-runs in nine pairs, no losses and18–43% latency
increases. Small exhaustive tests verify every shared-fixture rank probability,
including mixture replays. It changes draws/variance, so gains differ by seed;
judge coverage and latency over the full cohort, not a single new cell.
Evidence `/private/tmp/rare50-omit/summary.json`.


R20 adds82 cell-runs in nine pairs, no losses,18–48% latency increases.
It estimates318/4 and16/12 in some16498 runs that stricter batches leave zero.
Evidence `/private/tmp/rare50-order-quality/summary.json`. Validate rough
estimates on independent reference IS before recommending the weaker gate.

R14 with cheaper four-rival setup still gains only74 cell-runs:16498 seed801
uses the allocation on model/pilot setup and gains none. Reject global case
construction. Evidence `/private/tmp/rare50-order-cases/summary.json`.

| R21 | Small cardinality-case rescue only after pilotESS<2 | Complete; off | At most two roots,eight branches,20k additional setup nodes per difficult cell, full native guided defense. Meter all failed work and skip new training after65% of the measured tail allowance. Never turn setup failure into a proof. |


R17 at the45% tail allowance adds85 cell-runs with no losses, but
16498 seed808 still exceeds allowance (+62%). Earlier changes trigger additional
peer work; reduced final draws cannot be counted as a guaranteed time saving.
Reject global activation. Keep existing point-tilt quotas for final validation.
Evidence `/private/tmp/rare50-reallocate-safe/summary.json`.


| R22 | Share newly verified pilot seasons across cells before funding final batches | Complete; off | Canonical complete seasons are checked by the production sorter. Rebuild up to four low-ESS models from matching cross-team/rank seasons, using eight roots and native guided defensive mixture; retain only an improved independent pilot. Pilot seasons never contribute directly to reported probabilities. |

R22 differs from reusing the original scout/neighborhood seasons: the new
rare-event pilots can reveal coupled fixture assignments absent from that
bank. Freeze the expanded proposal before fresh main/check estimation. Both
favorable and unfavorable ranks use the same rule. Reuse stays within the
same measured tail allocation and training deadline.


R21 adds79 cell-runs with no losses and20–45% latency increases. It fails to
improve aggregate coverage over R20 (82). Extra rank-case setup still displaces
useful ordinary final batches. Defer rather than enable. Evidence
`/private/tmp/rare50-hard-cases/summary.json`.


R22 pilot-season reuse gains82 cell-runs, same as R20, with no losses and
16–48% latency increases. It did not improve the remaining difficult cells in
this cohort. Keep it off in the selected configuration rather than paying
unproven extra setup. Evidence `/private/tmp/rare50-pilot-reuse/summary.json`.

## Selection and validation checkpoint

Select R20 with four-rival blocks, conditional goal tilts, adaptive witness
mixtures, primary fixture ordering, and rank-irrelevant omission. Preserve the
existing point-tilt/scout/pool stages. Larger case models, fixture fitting,
dynamic propagation and global point-tilt reallocation are not selected.
Next: all seven snapshots with all six development seeds, then the three
reserved held-out seeds; paired warm CPU/RSS measurements and the full Rust
suite. Add one opt-in flag `RUST_ODDS_RARE_TAIL=coverage` for the selected profile;
explicit experimental flags may override its defaults. Unset keeps prior behavior.


Audit correction: the intended high-ESS-only edit was absent from the binary
used for `rare50-reallocate-safe`; that arm actually repeated the original
R17 policy with the45% tail allowance. Its paired observations are valid, but
it is not evidence about the revised policy. Correct the source and run the
actual high-ESS-only experiment separately before making that recommendation.
The selected/held-out coverage profile has this experimental flag disabled
and is unaffected. Its validated binary is frozen separately.


| R23 | Share identical frozen suffix probability guides across cells within one request | Complete; selected | Weak-reference cache keyed by full fixture order, endpoints, packed gains, probabilities and team count. No scores, base points or bounds shared. Keep local logical setup quotas unchanged. Build outside the cache lock; race results are mathematically identical. Measure RSS/setup savings and coverage. |

Warm selected16498 latency +39.3%, but process CPU +55.2% and peak RSS
194→448MiB. Four estimator workers are preserved. R23 targets duplicated
per-root/cell guide tables, especially consecutive ranks of the same team.
The unchanged-budget alternative remains frozen for comparison.


Actual corrected R17 high-ESS-only allocation adds82 cell-runs, no losses,
but 16498 seed801 is +56% (607.5→948.8ms). It neither improves coverage nor
reliably stays under allowance. Keep it disabled. Evidence
`/private/tmp/rare50-high-ess-tilt/summary.json`.


R23 adds81 cell-runs in nine pairs, no losses and16–45% paired latency
increases. Request-local guide reuse hits:16498 18–27; current16653 5–8;
historical16653 2–3. Coverage is essentially unchanged versus R20, so selection
will depend on warm setup/CPU/RSS rather than a single seed's extra cell.
Evidence `/private/tmp/rare50-shared-guides/summary.json`.


R23 warm candidate peak RSS:16498 448→400MiB,current16653 281→263MiB,
historical16653 271→266MiB. Warm wall increases versus frozen Rust baseline
32.7%/27.4%/17.7%; processCPU increases47.0%/30.7%/26.5%. Separate runs have
clock variation; claim memory savings and unchanged probability guides, not a
precise CPU speedup. Select shared guides in the one-flag coverage profile and
repeat full nine-seed validation on this final configuration. Original
unshared validation remains archived. Evidence `/private/tmp/rare50-shared-warm/summary.json`.


Budget safety correction before final validation: base the additional allowance
on native calculation time, excluding HTTP uploads/queue wait. A long queue
must not authorize extra CPU search. Direct unlogged search uses its own elapsed
search time conservatively; remove the arbitrary200ms fallback. Preserve HTTP
request IDs and total-time logs. Test this without sleeps by constructing an
older request start and a fresh calculation start. Start logs explicitly show
the selected profile, quality gate and shared-guide setting.


### Final-validation checkpoint and R24 latency headroom

The selected shared-guide profile completed63 paired requests:223 gained
cell-runs across41 distinct snapshot/team/rank cells,329→106 eligible zeros,
zero lost positives/proofs/reachability, no remaining undecideds. Established
golden comparisons cover108 new cell-runs; ratios0.463–1.48. Cold-pair maximum
wall increase45.6%. Warm 16498 median622.9→922.8ms (+48.1%), but one of eight
timed pairs exceeded the allowance (+52.8%). ProcessCPU increased64.5%;
peak RSS198→392MiB. This is NOT sufficient evidence for a strict per-request
50% wall ceiling. The work controller is predictive rather than real-time.

| R24 | Leave more controller headroom by reducing the additional allowance from45% to40% or35% of native calculation time | Complete; selected 35% | Compare coverage and warm CPU/wall costs. Preserve all proposals and confirmation gates. Report any lost new gains separately from regressions of baseline positives. One warm outlier cannot be hidden behind a median. |

Raw first-final evidence: `/private/tmp/rare50-final/summary.json` and
`/private/tmp/rare50-final-warm/summary.json`. Keep these results even if a
safer funding profile is selected. No commits or pushes authorized.


R24 cold screens:40% funding adds80 cell-runs, but 16498 seed801 is still
+55.1% (602.8→935.1ms).35% funding adds76 cell-runs, with a44.5% maximum
increase; no baseline estimate/proof/reachability losses in either. Compared
to the earlier shared-guide screen (81 gains), these sacrifice1 or5 new
cell-runs, not existing positives. Native pre-tail costs vary too, especially
point-tilt; the tail's own predicted fraction cannot guarantee a paired
full-request ceiling. Warm comparisons follow. Evidence
`/private/tmp/rare50-headroom40/summary.json` and
`/private/tmp/rare50-headroom35/summary.json`. Full Rust suite: 77 passed, 2 DB
tests ignored. No DB writes.


R24 warm16498:40% funding median625.7→852.6ms (+36.3%), largest
individual pair+37.6%, CPU+52.6%.35% funding median622.1→872.2ms (+40.2%),
largest pair+41.1%, CPU+54.2%. Lower nominal funding is not necessarily
faster end-to-end: measured pre-tail point-tilt changed from293ms to331ms
between these runs, despite identical point-tilt quotas. Do not infer a precise
CPU saving from these separate runs. Select35% for more latency headroom,
then repeat the full63 pairs and warm cohort using its one-flag default.
The50% allowance is assessed as paired request wall time with four workers;
summed CPU increases are reported separately and can exceed50%.
Evidence `/private/tmp/rare50-headroom40-warm/summary.json` and
`/private/tmp/rare50-headroom35-warm/summary.json`.


## Campaign completion — selected measured result

**Select `RUST_ODDS_RARE_TAIL=coverage`, with 35% native-calculation funding.**
The profile is opt-in; production defaults remain the earlier pipeline.
No commit or push performed. The larger 45% funding result is retained as an
experimental alternative, not silently discarded or described as meeting the
wall allowance.

Final 63 paired full requests, seven snapshots × nine seeds, four workers:

- **207** gained nonzero cell-runs across **39** distinct snapshot/team/rank
  cells; eligible zeros **329→122 (62.9% removed)**. These are repeated-seed
  observations, not207 extra positions in one table.
- Development 147 gains, reserved-seed safety validation 60. Funding was reduced
  after timing failures; the repeated reserved-seed checks are safety
  validation rather than a second untouched selection holdout.
- **Zero** lost baseline positives, impossible proofs or verified reachability;
  game importance identical. Final undecideds: **0** in this cohort. New
  impossibility proofs: **0**; the remaining eligible zeros are reachable.
- Largest cold-pair wall increase **46.7%**. Largest individual warm-pair
  increase **35.0%**, none above 50%. Warm 16498 median **623.1→828.0ms (+32.9%)**;
  summed CPU **+45.1%**, peak RSS **200.6→395.0MiB**. Current 16653 warm
  **661.8→771.6ms (+16.6%)**; CPU **+21.5%**. Same four-worker maximum.
  Earlier screens showed CPU increases above 50%; do not promise a strict
  CPU-time ceiling or hard real-time wall guarantee for future requests.
- Established golden probabilities cover 98 gained cell-runs;
  candidate/reference ratio **0.463–1.48**. The other new estimates do not
  acquire reference validation merely by passing internal checks.

| Snapshot | Eligible zeros before per request | After | Structural impossible zeros | Gained cell-runs | Distinct gained cells |
|---|---:|---:|---:|---:|---:|
| 16498 | 14–20 | 6–15 | 17 | 59 | 15 |
| Current 16653 | 8–13 | 2–4 | 48 | 71 | 11 |
| Historical 16653 | 6–12 | 0–2 | 43 | 77 | 13 |
| 16982,16983,15902,16413 | 0 | 0 | 0 | 0 | 0 |

Selected mechanisms: complete bounded unions, sampled target paths preserving
event support, rank-cardinality proposal guidance, exact four-rival shared
conditioning, conditional goal tilts, adaptive broader/witness mixtures,
strict-irrelevance fixture omission, request-local shared guides, and
pilot-cost/ESS funding of fixed fresh main/check batches. Existing scout, pool,
point-tilt and other accepted work retained. The new final gate targets rough
orders (main RSE<=60%, independent check<=75%, agreement within a factor of 5); it
does not fabricate probability estimates from upper bounds or witnesses.

Twenty-six development arms are archived, including timing and quality
rejections. Broad case construction, inside-draw propagation, smoothed fixture
fitting, cross-pilot witness retraining and point-tilt reallocation did not
improve valid coverage within this budget. Increasing nominal work alone is
not selected. We reached a measured plateau for this portfolio; no claim of
a theoretical global maximum.

Final report: [2026-10-01-rust-rare50.md](experiments/rare_positions/2026-10-01-rust-rare50.md).
Portable evidence: [final-results.json](experiments/rare_positions/2026-10-01-rare50/final-results.json),
[development-arms.json](experiments/rare_positions/2026-10-01-rare50/development-arms.json),
frozen baseline source patch and configuration in the same directory. Raw
final outputs: `/private/tmp/rare50-final35/` and
`/private/tmp/rare50-final35-warm/`. Validated binary:
`/private/tmp/golaberto-rare50-final35-validated`.

### Remaining experiments, ordered by evidence

1. **Discrete splitting/SMC with independent root replications.** Target
   dominant-weight and low-hit cells using necessary shared rank constraints
   as intermediate levels. Begin with exact toy probabilities and discrete
   weighting before full matrices. Charge transitions, genealogy and setup
   against the current native budget; do not assume population growth is free.
2. **Within-request memory reduction for guide tables.** Shared immutable guides
   help, but 16498 peak memory nearly doubles versus baseline. Compact suffix
   rows or evict provably unused roots without changing their defensive
   proposal support. Measure sampler replay cost and actual process RSS.
3. **Lookahead fixture ordering from the original proposal.** This remains
   untested in the earlier common sampler; do not relabel the new joint guide
   as a test of that component. Run independently with fixed old budgets and
   gates to avoid confounding order with allocation.
4. **Deferred score sampling in the broad pool.** Retained as a separate
   statistically intrusive optimization. It changes RNG seasons; require
   broad-position and game-importance comparisons, not just rare-cell hits.

Final Rust suite: 77 passed, 2 optional DB tests ignored. No DB mutations,
Rails/UI changes, commits or pushes.


## Experiment 4 / R25: shared blocker-conditioned matrix sampling (proposed)

User hypothesis: a leading team A is 25–27 points above several rivals with
nine games remaining. Condition on A's collapse once, then accumulate several
team/rank estimates from each complete season. Current rare samplers return
one target-cell result; shared tables and witness reuse do not implement this
probability reuse.

### Exact small conditioning event

Under 3/1/0 scoring, A gains exactly 2 points in 36 outcome patterns (two draws,
seven losses). A gains at most 2 points in 46 patterns (zero,one or two draws;
no wins). Their masses are products of fixture-specific outcome probabilities;
patterns must not be selected uniformly. DP can sum/sample the entire event
without enumerating all 3^9 outcomes. Sample conditional scorelines as needed
and carry every shared match's outcome to the opponent exactly once.

A rival with a 25-point deficit can tie A after earning 27 points only when
A adds<=2; deficits 26 and 27 require A adds<=1 and<=0 respectively. The broad
shared event A adds<=2 contains all these individual cases. Requiring all four
rivals to catch A simultaneously is a different, stronger event and can be
impossible when the rivals play each other. Actual wins and later tiebreakers
must be checked by the production sorter.

### Estimator and proposed comparison

For each selected cell E_i, first prove E_i implies the conditioning event F.
Sample full seasons from P(season | F), sort them once, and estimate
P(E_i)=P(F)*hits_i/N. An event that can also occur outside F needs a complement
stratum or a proposal mixture; its within-F estimate alone is incomplete.
Further rank-directed proposals must retain support for every selected cell
and use the whole proposal P/Q, including conditional goals. Existing
target-specific early rejection, skipped scores and hard proposal domains
cannot simply be reused as multi-cell observations.

- Cluster residual cells by common necessary blocker caps/floors, using points
  and applicable wins. No team IDs or particular ranks select eligibility.
- Share one exact event model, complete season generation and sorting; maintain
  separate per-cell hit/weight/SE diagnostics and independent confirmation.
- Compare shared-event-only sampling against existing per-cell work, then a
  full-support mixture of recipient-guided proposals for weak conditional hits.
- Reallocate existing final portfolio funding first. Keep four workers and the
  current 50% full-request wall allowance; measure setup, draw and sorter costs,
  old positive regressions, eligible zeros, CPU and memory.
- Test exact small leagues, shared A/rival and rival/rival games, equality on
  points/wins, and cells whose event is not wholly inside the proposed stratum.

Status: **Implemented and measured; not recommended for activation** (see the R25 shared-constraint results below). Common conditioning removes A's
collapse cost, but recipient teams may still need improbable result sequences,
so four recipients do not guarantee four accepted estimates. This is a more
direct bounded native experiment than introducing a full splitting engine.


## Run-specific gap diagnosis — group16498, seed1790864768586573000

Readonly DB inspection confirms Athletico-PR is team5 and Chapecoense-SC
team318 in16498. The earlier supplied16653 log belongs to a different group;
its two reproduced eligible zeros are Londrina-PR95/ranks3,4.

Reproduced16498 with the current coverage profile and identical seed:

- Athletico19th: pilot106 hits,ESS5.72; no funded final batch. The allocator
  sorts by pilotESS and estimates the fixed main/check cost; this cell does
  not fit the remaining worker bins. Reachability stays verified.
- Chapecoense3rd: estimate7.357e-24, confirmed and published. Main97 hits,
  ESS27.5; independent check41 hits,ESS9.57. Stored DB odds also contain a
  nonzero third-place probability.
- Chapecoense4th: main91 hits/1290 draws,ESS4.70,RSE46.0%, but max event
  weight share42.5% exceeds the35% rough gate. Reject rather than publish
  the unconfirmed2.481e-19 main estimate. Stored DB fourth-place odds arezero.
  Metadata rank key3 is zero-based and refers to fourth place.

Existing gap rescue occurs inside point-tilt, before deferred joint and new
rare-tail publication. It cannot react to neighboring estimates first added
by those later stages. A possible follow-up is final gap-aware funding or
shared-anchor multi-cell sampling, with a comparison against the current
allocation. No new allocation/acceptance change made in this diagnosis.

Reproduction outputs `/private/tmp/golaberto-16498-1790864768586573000.json`
and `.log`. Funded-cell counts are sensitive to measured preparation cost:
user summary8 finalists/7 accepted versus reproduction9/8; this is not
bit-identical allocation despite a fixed RNG seed. No DB mutations.


### R25 refinement: build per-cell constraints, then cluster compatible proposals

User refinement: group recipients when their separate requirements have similar
conditional difficulty after the shared blocker constraint. Build each cell's
necessary constraints first, then detect reusable simulation structure. Sharing
a leading team alone is insufficient evidence of a useful shared batch.

Proposed screening and validation:

1. Derive each unresolved cell's safe blocker caps/floors, target points/wins
   requirements, relevant fixtures and proposal domains independently. Compare
   actual fixture identities and allowed outcomes; similar numeric thresholds
   do not imply identical event support.
2. Find common necessary event F and estimate recipient conditional difficulty
   cheaply from point/win PMFs, including the different blocker-total strata
   and fixed results of blocker/recipient fixtures. These are screening proxies,
   not final rank estimates; shared rival games prohibit treating their marginal
   probabilities as independent joint probabilities.
3. Group similar conditional difficulty and sufficiently overlapping proposal
   support. If common-event sampling is adequate, one sorted season contributes
   to every eligible recipient. Otherwise sample a frozen mixture of recipient
   proposals plus a defensive common-event component. Use the entire mixture
   density for every recipient; never apply the generating component's weight
   to the whole batch.
4. Run a bounded independent pilot. Compare per-cell hits, ESS and predicted
   main/check cost per millisecond against separate proposals. Split recipients
   that make the group slower or receive insufficient useful sampling. Similar
   event probability is not enough: weight variance and likelihood replay cost
   also matter. Predetermine fresh final/check batches after selection.
5. Share setup and immutable conditioning/suffix structures exactly where their
   keys match. Full-season generation, score backfill and sorting must support
   all recipients. Existing one-target early rejection or target-only goal
   marginalization cannot provide other cells' observations without adaptation.

For samples s~q_mix on F, estimate each eligible event E_i using
P(F)/N * sum[P(s|F)/q_mix(s) * indicator(E_i)]. Require E_i subset F, or include
a correctly weighted complement stratum. Pooling observations across mutually
exclusive first-place events does not yield four hits per season: if recipient
proposals hardly overlap, sharing mainly saves setup, and mixture replay could
cost more than it saves. The pilot should measure this tradeoff.

Status: **Measured** in the bounded native shared-constraint experiment below. The tested grouping uses a tenfold necessary-event mass range, followed by independent actual rank-hit/ESS pilots. It does not assume those masses equal conditional rank probabilities.


## R25 — move existing gap rescue after final publication

User requests moving the gap rescuer later. Extract the existing proven and
undecided gap paths from point-tilt; run them once after deferred joint and
rare-tail publication, before reconciliation. Keep the same per-cell RNG
streams, draw quotas and acceptance gates for the first comparison. Temporary
`RUST_ODDS_LATE_GAP_RESCUE=1` selects the moved stage. Compare against frozen
current coverage binary, not the older native/no-tail baseline. Check both
full-request cost and the earlier50% allowance; any newly eligible work is
measured rather than assumed free. No commit/push. Status: running.


R25 direct-move screen, nine paired coverage requests: late gap stage gained
**zero** accepted gap estimates and cost40–108ms when active. Overall matrices
gained2 cell-runs and lost4, all differences from rare-tail funding rather
than successful late-gap estimates. Measured wall changes-1.8% to+13.1%.
Do not activate the unconditional moved stage globally. Next screen retains
full15k final batches only when the corresponding fresh1k pilot has>=3 hits
andESS>=1.5. Futility is a work decision, not an impossibility proof.
`RUST_ODDS_LATE_GAP_RESCUE=full` retains the initial direct-move arm;1 tests
pilot-guided late rescue. Final acceptance gates remain unchanged. Evidence
`/private/tmp/late-gap-move/summary.json`.

R25 completed: guarded late rescue accepts one already-positive cell, no additional gap coverage. Overall five gains/four losses originate from runtime-funded rare-tail differences. Keep opt-in, default placement unchanged. Paired costs, CPU and reproducible commands: `experiments/rare_positions/2026-10-01-rust-late-gap.md`.


### R25 experiment started

Freeze CURRENT Rust coverage profile (including disabled late-gap experiment)
as `/private/tmp/golaberto-r25-baseline`; both paired sides use
`RUST_ODDS_RARE_TAIL=coverage`, four workers. Start with exact points/wins
interval conditioning per cell, group cells by a shared necessary blocker and
similar conditional difficulty, and sample a correctly weighted mixture that
collects every recipient's rank. Common-blocker-only sampling is an ablation.
Use part of the existing native tail allowance; do not add an unmeasured stage
on top. Fresh fixed main/check batches, unchanged rough acceptance gates.
Test all cell implications and mixture probabilities against enumerated small
leagues before timing. Development801/808/818; validation804/817/911 plus
reserved1201/1213/1229 if an arm passes screening.

Status: **Completed; adopted in the coverage profile after user authorization**. Results are under
`experiments/rare_positions/2026-10-01-shared-constraints/`; final report
`experiments/rare_positions/2026-10-01-rust-shared-constraints.md`. Shipment authorized after reviewing the measured tradeoff.


### R25 shared-constraint development checkpoint

Do not confuse this experiment with the separate R25 late-gap entry above.
Three nine-pair screens (current16498/current16653/historical16653;
801/808/818), both sides current coverage/four cores:

| Shared proposal | Direct accepted cells | Matrix gains/losses | Active stage cost |
| --- | ---: | ---: | ---: |
| Exact recipient + one blocker mixture | 0 | 0 / 1 | 1.6--2.1ms |
| Same, cardinality-guided remaining fixtures | 0 | 1 / 1 | 8.0--9.4ms |
| Exact recipient + two blockers, cardinality guidance | 0 | 0 / 0 | 9.1--10.2ms |

No grouped batch qualified for fresh final/check funding: at least two useful
recipients (>=3pilot hits andESS>=2 each) are needed. Similar necessary-event
masses do not make actual rank probabilities similar. The historical16653
group produces occasional rank18 hits for team70 but none for2064;16498
has occasional team74/20 hits but none for15/20. Current16653 has no matched
group under the tested requirement rules. Matrix changes are runtime-dependent
funding differences in the pre-existing tail, not accepted shared estimates.
Next: common-blocker-only ablation, held-out seeds and prefix-versus-score
diagnostics; confirm model setup and CPU/RSS cost before closing the experiment.
Full Rust suite:83 passed, two DB-write tests ignored. No DB mutations.


### R25 shared constraints — final result and next allocation target

Completed six arms: common-blocker only; recipient + one blocker; guided one;
guided two; two blockers with exact relative final totals; relative model with
concurrent main/check and 50% of the EXISTING tail allocation. No new total
allowance. The first four arms had zero direct confirmations. Relative
constraints changed 16498/20 proposal efficiency by roughly 100x in ESS per ms
in a 30k offline diagnostic. Warm full-request speed stayed approximately equal.

Strongest arm: 39 cold pairs (development 9, held-out 21 over seven snapshots,
reserved 9). Seven gains across six distinct cells, one loss; eligible residual
cell-runs 126 → 120. Sixteen direct shared confirmations, mostly overlapping old
coverage; Bahia 20 / seed 1213 is one direct additional estimate (5.665e-19).
All changes are in 16498. No proof, reachability or game-importance regressions.
Current 16653, historical 16653 and 16982 gained no final cells on these runs.

Cold median paired wall -0.55%; largest observed increase +6.70%. Median CPU
-2.48%, largest increase +9.81%. Warm 16498/808: 830.18 → 825.83ms (-0.52%);
process CPU 24.39 → 23.98s (-1.68%); peak RSS 400.73 → 352.50MiB. Eight warm
comparisons: six same, one gain, one loss (Chapecoense 4). The cold loss is
Fluminense 20 at seed 1213, prior 3.484e-26. These losses are existing
individual searches whose time-based funding changed, not new impossibility
decisions. Do not claim every old positive is preserved under this allocation.

Release decision: **enable the strongest tested shared configuration in the
coverage profile** after the user accepted the seven-gain/one-loss tradeoff.
Defaults: guided, two relative blockers, parallel independent confirmation,
50% of the existing tail allowance. Disable with `RUST_ODDS_SHARED_CONSTRAINTS=0`.
The initial opt-in recommendation was revised to match the objective of overall
coverage; the warm net-zero result and allocation regressions remain recorded.
Next hypothesis: compare shared and individual
proposal pilot ESS/time after the individual pilots exist; replace only groups
that actually save predicted confirmed-estimation work, and protect promising
unrelated finalists. Necessary-event mass similarity alone is insufficient.
Native proof quotas and quality gates unchanged; no DB writes. Shipment authorized.
Full suite 83 passed / two ignored before final refinements; final targeted
40 library + 34 estimator tests passed, including actual goal tiebreakers,
exact WDL unions, both-direction implications and whole-mixture densities.


### R25 shared constraints — shipment

User authorized shipment. The coverage profile now selects guided sampling,
two relative blockers, parallel independent confirmation and half of the
existing tail allowance. Explicit overrides retain rollback and native mode.
No new total allowance, core count, proof quotas or acceptance gates. The
separate late-gap experiment remains outside this release. The exact release
tree passed 83 Rust tests; two DB tests ignored. Added configuration/override
coverage and effective settings in the request start log.

Release smoke check: all seven snapshots at seed808 match the explicit tested
profile's coverage and proof labels; no game-importance differences. A single
current16653 latency pair was +7.43%, explicitly retained. Shared-only rollback
was verified by a full16498 request with no shared events (one individual gain,
no losses from runtime allocation). Portable evidence: shared-constraint
`release-validation.json`.


### R25 coverage default follow-up

User requested coverage as the default. Unset `RUST_ODDS_RARE_TAIL` now resolves
to `coverage` everywhere, including witness reuse and effective request logging.
Explicit `0` restores native-only calculation; shared-only rollback remains
`RUST_ODDS_SHARED_CONSTRAINTS=0`. Configuration, budgets, four-core usage and
acceptance gates match the previously shipped explicit profile. Added a pure
resolver test and child-process default/rollback integration coverage.

Default follow-up verification: the exact release tree passed the full Rust
suite (**85 passed, two database tests ignored**) with `--test-threads=1`,
serializing full-request tests to preserve the four-worker limit. The no-flags
default and explicit `0` opt-out both passed the child-process integration test.


## R26 — bounded branch conditioning and high-variance pilot retries

Status: complete; R26 defaults shipped in af8dda53, then withdrawn by R27. Shipping authorized
on 2026-10-01. User requested the generic improvement after Fluminense
20/seed1229 produced 469 hits but failed ESS/max-weight-share checks. Freeze
current default-coverage Rust (2c87b146), including shared sampling, as baseline.
No additional time allowance: replace/reallocate existing tail work, four cores.

Experiments: (A) reduce expensive complete-union rival enumeration while
preserving support and actual sorting; (B) learn target-branch allocation from
existing pilot second moments and reuse the existing adaptive-pilot slot for
an independent retry; (C) allocate fixed independent confirmation to promising
weighted cells using pilot evidence. Training never supplies published estimates,
failed setup stays undecided, and no team/rank-specific rule is introduced.
Development801/808/1229; validation804/817/911 across all seven snapshots;
reserved1201/1213/1237. Compare coverage gains/losses, estimates, proof labels,
stage/full-request timing and CPU. Retain the current budget and quality gates.

R26 checkpoint: reducing complete-union rivals from6 to4/3/2/1 does not
build Fluminense20's full186-branch union: six exhaust joint nodes; smaller
blocks exhaust guide memory (17.7–33.7ms). Reject that construction-only arm.
Nine-pair development screens: root-moment pilot retry gains5/loses2; bounded
union64 gains3/loses0; bounded64 plus forecast ESS20 gains3/loses0. These are
cell-runs and separate timing-dependent cohorts, not cumulative improvements.
Root fitting was not retained for Fluminense/1229. Larger fixed batches improve
its ESS3.80→7.54, but max-weight share35.34% still misses the unchanged35%
gate. Test the generic ESS32 forecast next, with fixed independent confirmation.

R26 second checkpoint: bounded64 plus ESS32 finds Fluminense20/1229 at
6.325e-26 (main1331hits,ESS14.22,max-share22.30%), independently confirmed.
Nine development pairs gain6/lose4; broad ESS32 funding starves good pilots,
including Fluminense20/801. Refine allocation generically: forecast ESS32 only
for pilots below the existing12-ESS adaptive threshold, retaining the old
forecast for stronger pilots. Keep all acceptance gates unchanged.

R26 validation checkpoint: concentrated allocation with/without root learning
each gains1/loses4 across21 validation requests. Do not enable that allocator.
Reserve ordinary main/check batches first and enlarge weak-pilot batches only
from spare predicted worker capacity. With bounded64 and root learning, this
revision gains9/loses1 in nine development requests, then gains16/loses3 across
42 requests (seven snapshots ×804/817/911/1201/1213/1237). The three reserved
seeds contribute11 gains/1 loss. No reachability, impossibility or game-importance
regressions. Median paired full-request latency is unchanged; maximum increase
6.12%, so latency preservation is not asserted per request. Warm-server check
and final correctness suite pending. Fluminense/1229 independently confirmed
at6.325e-26; Fluminense/1213 was not funded in one validation request.


R26 final allocation checkpoint: retain witness/alternate retries for pilots
with fewer than30 matching seasons. Final no-flags defaults use union64,
guarded root learning and reserve-first allocation. Across49 paired full
requests:11 gains/2 losses (nine distinct gained cells/two lost); Athletico19
regressions at817/911 are resolved. Remaining losses at1213 are unfunded
Chapecoense5/Athletico17 pilots. Fluminense is positive6/7 versus4/7 requests;
1229 still lacks spare capacity for its larger final batch in this retest.
Across24 warm pairs:13 gains/0 losses, total wall−1.15%, CPU−2.93%; historical
16653 median wall+0.55%. Cold total wall−0.29%, CPU−1.21%, max increase11.59%
(15902/804, no tail work); max difficult increase4.10%. No proof/reachability or
game-importance regressions. No added allowance/fifth worker. Report and raw
summaries:experiments/rare_positions/2026-10-01-rust-tail-retry.md and
2026-10-01-tail-retry/*.json.gz.


R26 final verification:88 Rust tests passed, zero failed, two DB tests ignored,
using the final guarded source tree and `--test-threads=1`. Formatting and diff
whitespace checks passed. The local coverage defaults require no extra flags;
individual rollback flags and reproducible commands are in the report. No
further timing allowance was used. Cold/warm gains are reported separately and
are not independent counts of distinct new cells.


## R27 — R26 production regression follow-up

Status: complete; previous defaults restored and experimental settings opt-in. User reports multiple regressions after af8dda53. Retest
against2c87b146 on fixed fresh seeds1301/1303/1307/1319/1321, both16653
snapshots and16498, four workers with serial full requests. Initial15 pairs
have6 gains/3 losses. All three losses have unchanged/promising pilot evidence
but no final allocation: Cruzeiro19/1303, Athletico18/1307, Chapecoense4/1319.
They are not impossibility changes. Improved/new pilot candidates consume final
capacity ahead of older candidates; reserve-first does not protect the older
version's candidate set. Ablate root learning, union cutoff and batch upgrades
individually, and validate full legacy-profile rollback. Preserve acceptance
checks and existing native coverage/shared settings. Production examples/seeds
have been requested asynchronously; do not claim those exact runs reproduced.


R27 resolution: partial ablations (15 pairs each) do not preserve all prior
cells: root-off4 gains/5 losses; union256-only5/3; upgrade-off8/3. Restore all
three defaults together (256/0/0) and preserve original unfitted weight operation
order. Normal-budget correction screen15 pairs:6 gains/1 loss (elapsed-time
funding still varies);24 warm pairs5 gains/0 losses, total wall−0.33%, CPU+0.57%,
max individual increase7.52%. Group16498 warm median wall+1.34%, CPU+4.18%; do
not claim a speedup or exact cold-request coverage guarantee. A diagnostic pair
with fraction2 in both arms returns identical complete response objects; this
is an offline control, not additional production allowance. Full suite88 pass,
2 DB tests ignored, serialized requests. Reports and raw summaries:
experiments/rare_positions/2026-10-01-rust-tail-regressions.md and
2026-10-01-tail-regressions/*.json.gz. No DB/UI changes. Next allocation research
should publish ordinary baseline results before spending unused capacity on
new proposals; deterministic work quotas require separate measurement.

## R28 — Preserve ordinary estimates before retry allocation

Status: complete; shipped as an opt-in, extension remains off by default.
Baseline: current Rust master34000cc1, four workers. Freeze ordinary pilots, funding, sample counts
and RNG streams; commit ordinary estimates before new work can fill zeros.
Compare `RUST_ODDS_RARE_TAIL_EXTENSION=after` (leftover allowance after ordinary
batches) against `overlap` (idle workers within the predicted ordinary final
span). Reuse trained plans, fresh fixed main/check streams, unchanged acceptance
gates, no added budget. Measure full-request wall/CPU, extra confirmed estimates,
paired losses, and memory. A wall-clock admission forecast is not a hard latency
bound and cannot guarantee identical coverage across separate processes.


R28 results: sequential cold15 pairs9 gained/4 lost cell-runs, wall+1.24%,
CPU+0.45%; all four losses are identical pilots with no ordinary final funding.
Initial overlap15 pairs6/2, wall+0.94%, CPU+2.13%, only2 accepted retries.
Smaller fixed overlap batches15 pairs10/2, wall+1.81%, CPU+3.00%,5 retries.
Sequential warm24 pairs18/1, wall+0.10%, CPU−0.44%;16 direct retries are two
historical16653 cells repeated eight times, not18 distinct cells. Historical
16653 median wall+4.49% (tail+35.70ms). Smaller overlap warm24 pairs8/6,
wall−0.57%, CPU−1.30%,8 direct retries for one historical cell. Fresh sequential
holdout14 pairs/all7 snapshots:1/0, wall+2.09%, CPU+2.26%, max pair+4.90%.
No proof/reachability/game-importance regressions. No extra allowance/fifth worker.

Ordinary-result preservation is verified within requests; elapsed-time funding
still prevents identical coverage across separate processes. Offline fraction2
control/16498/1307 has all15 ordinary finals identical, retains all baseline
positives and confirms one extra (318/rank4). This is a correctness control,
not production timing.90 Rust tests pass,2 DB tests ignored. Report, source
patches, input/executable hashes and raw summaries:
experiments/rare_positions/2026-10-01-rust-tail-preservation.md and
2026-10-01-tail-preservation/. Recommend only optional `after`; keep defaults
unchanged under the latency constraint. Next experiment: deterministic ordinary
work reservation, accounting for in-flight proposal setup before optional root
learning or larger batches; include extension-off variation as a control.


## R29 — Enable sequential retries by default

Status: validated; default enablement authorized by the user after the R28 opt-in
shipment. Promote exactly the tested `after` mode in the coverage profile;
`overlap` remains experimental and explicit `RUST_ODDS_RARE_TAIL_EXTENSION=0`
disables retries. No changes to sampling, gates, four workers or allowance.
R28 sequential cohorts show28 gained/5 lost cell-runs across53 pairs (not28
unique cells); warm total wall+0.10%, fresh cohort+2.09%, historical16653 warm
median+4.49%. User accepts enabling the revision with these measured costs.
Add default/rollback tests and verify the default HTTP profile reports `after`.


R29 verification:91 Rust tests passed, zero failed, two DB tests ignored,
serialized with `--test-threads=1`. The default HTTP request logs `after` and an
extension summary; disabling the whole portfolio logs no extension work.
A unit test covers default sequential retries, explicit0/empty rollback,
explicitoverlap/after selection and non-coverage profiles. Sampling code is
unchanged; the production change is the coverage profile's default value.
Formatting and whitespace checks pass. Rebuild/restart needs no added flags.

R29 release build completed with the locked manifest. Executable SHA256:
`e695437bedb38f36ef2dc65c5316cc3e85c3f2bf58afd1bb94f6cf28f44a8334`. Default enablement shipped to master.

## R30 — CPU optimization without losing zero-cell coverage

Status: validated and approved for shipment. Experiment history follows.
Baseline e9cce7dc (default sequential retry), four cores; preserve existing
samples, proposals, gates and proof rules.
An8-second full-request native sample on16498 identifies lookahead proposal
loops, propagated rank proposals and event construction as the largest active
stacks. Do not count thread waits as useful CPU. First test exact memoization of
lookahead guide factors for repeated fixed point/win states; bound setup/memory,
retain original floating-point operation order and RNG draws. Compare identical
kernels and paired full requests across all seven snapshots. Faster earlier
stages shrink the elapsed-time funding allowance, which can lose coverage even
when a kernel is mathematically unchanged; explicitly audit this feedback.

R30 intermediate results: whole-state factor cache9 pairs CPU+2.32%, rejected;
weight-key cache9 pairs CPU+1.56%, rejected. Exact loop changes9 pairs CPU−17.91%,
wall−18.42%, but2 gained/10 lost cell-runs due elapsed-time tail funding.
Seven snapshot deterministic oracles were identical. Additional exact CDF-tail
and cardinality work9 pairs CPU−20.43%, wall−19.78%,1 gained/3 lost; an incorrectly
named fraction flag in that run had no effect (effective coverage budget remained
0.35). Correct fraction0.45 reallocates savings:21 fresh pairs/all7 snapshots,
CPU−14.14%, wall−12.92%,8 gained/0 lost cell-runs. This is a useful coverage-safe
candidate, not the25% goal. Next: prepared DP prefix caps and profile-guided
compiler optimization, preserving random draws and every probability gate.

R30 final portable candidate: compact fixture fields, separate score-index table,
prepared prefix caps and gain intervals, exact CDF/cardinality shortcuts, integer
leading comparison. Restore tail funding with coverage fraction0.5; no extra
absolute-time allowance, samples or relaxed gates. Fresh21 pairs/all7 snapshots,
seeds1567/1571/1579:CPU−14.41%, wall−13.89%,9 gained/0 lost cell-runs; no proof,
reachability-label or game-importance regressions. Native compact screen9 pairs
was−17.91% CPU/9 gained/0 lost. Sampler-mode specialization worsened to−15.56%
and was discarded. Intermediate PGO9 pairs CPU−22.11%/6 gained/1 lost, optional
only. Production is Intel Xeon E5-2697@2.7GHz/four cores; local ARM measurements
are not production verification. Approximately25% goal remains outstanding.
95 tests passed,2 DB tests ignored. Reproducible isolated native/PGO Xeon harness,
source patch, raw summaries, profile and report retained in
experiments/rare_positions/2026-10-01-rust-cpu.md and2026-10-01-cpu-optimization/.
No commit/push. Next: Xeon paired benchmark, then target its remaining hot loops.

R30 final validation:all7 complete deterministic oracle JSON outputs equal;
source patch applies cleanly to e9cce7dc and reproduces final code/tests exactly.
Read-only SSH discovery of the configured host failed(publickey); no production
load or changes were made. Await accessible Xeon test host or benchmark output.

R30 continuation:production access is unavailable; user authorizes treating
portable local CPU reductions as applicable to the Xeon unless CPU-specific.
Do not gate progress on production access or promote ARM-specific PGO claims.
Next exact native experiment removes unused non-cardinality guide weights and
per-draw root-key allocation, then evaluates bounded rank DP and compact suffix
storage. Retain four cores, candidate fraction0.5 and all zero-coverage checks.

R30 continuation screens (four workers, nine paired full requests each):
- Skip unused cardinality-mode guide weights and reuse borrowed/inline target
  keys: CPU −19.26%, wall −18.23%, 11 gained / 0 lost cell-runs.
- Compress suffix CDF storage: CPU −13.60%, wall −12.66%, 5 gained / 0 lost;
  rejected because extra hot-path indexing outweighed setup savings.
- Bound rank-DP updates to nonzero support: CPU −18.05%, wall −16.41%,
  8 gained / 0 lost. Exhaustive bitwise sequence test passed.
- Prepared endpoint bounds and explicit CDF boundary branches: CPU −19.10%,
  wall −17.56%, 6 gained / 1 lost. Loss is 16498/318/rank5 at seed1669:
  identical pilot, more training admitted, ordinary final no longer funded.
  Investigating allocation feedback before accepting this continuation.
- Next: branchless shared match-outcome updates, paired all-snapshot validation
  including the regression seed. Production access is not a prerequisite.


## R31 — Shared Poisson indices on a discretized mean grid

Status: completed; production changes rejected. Experiment history follows.
Compare against the retained R30
portable candidate, with four workers and unchanged tail fraction0.45. Test0.01
mean-grid sharing of 256-bin Poisson starting indices while retaining the actual
mean and exact CDF. Correct both upward and downward before returning a score;
keep the same RNG draws, zero-mean behavior and large-mean sampler. This tests
lookup reuse and smaller fixtures without changing the target distribution.
Boundary/RNG tests cover rounding on both sides, tiny positive means and means
above32. Actual mean rounding would change the model and can compound across
rare-event fixture combinations; assess separately if lookup reuse is useful.

R30 continuation validation: lazy prepared chance rows, dense small point DP,
exact bounded rank support and private RNG cursor bounds are retained. Final
default tail fraction0.45 reallocates savings. Twenty-eight cold paired requests
(all7 snapshots, seeds1669/1847/1861/1867): CPU−20.22%, wall−18.62%,14 gained/0 lost
cell-runs. Warm defaults:56 pairs/all7, CPU−22.01%, wall−19.72%,31 gained/0 lost
cell-runs. Warm CPU includes two warmups per arm; wall/coverage exclude them.
98 tests passed,2 DB tests ignored. Reject fused campaign updates (no benefit),
CDF-only Poisson paths (about0% change), and compact suffix rows (slower).
Production access is unavailable and not a gate under the user's instruction;
these are portable source changes, not ARM-specific compiler tuning. About25%
CPU target remains short. Next exact experiment removes redundant conditional
score bounds checks, using the existing CDF=1/u<1 invariant.

R31 results: nearest-grid index plus two-way correction12 pairs CPU+56.69%,
wall+52.72%, rejected. Conservative bucket indices and smaller fixture layout
initially saved13.37% CPU on four MC-heavy groups. Full28-pair/all7 comparison
against retained R30: CPU−1.31%, wall−1.31%,1 gained/1 lost (16498/318/rank5,
seed1669). Rare-heavy snapshots increased CPU2.46–4.07%; MC-heavy decreased
10.67–19.35%. Same-binary grid-sharing ablation14 pairs: CPU−2.18%, wall−1.74%,
0 gained/0 lost. This isolates sharing from moving lookup arrays out of fixtures.
Rejected as production default: weak full-request gain and coverage regression.
Grid patch and summaries archived; source reverted. Actual modeled means were
never rounded: only cache keys, and exact per-mean CDFs/RNG draws were retained.
Rounding actual means would be a distinct model approximation, not an established
speedup. No commits or pushes.

R30 final loop screen: omit redundant conditional-score terminal bounds/minimum
checks under existing CDF=1/u<1 invariant. Nine paired rare-heavy requests:
CPU+5.34%, wall+3.92%,2 gained/0 lost. Reverted because
CPU worsened. Retained R30 source remains the98-test/default0.45 candidate.

R30 retained final checks: all seven full deterministic oracle outputs exactly
equal to e9cce7dc; archived patch applies cleanly and reproduces all ten changed
source/test files byte for byte. Experimental Poisson grid and score-bounds
changes are reverted. Four-core defaults require no added flags.

R31 literal follow-up: rounded actual fixture means to nearest0.01, preserving
zero/positive support, without table reuse. Same retained binary,14 pairs/all7
snapshots at seeds1987/1993: CPU+1.17%, wall+1.15%,2 gained/4 lost cell-runs.
All losses16498/seed1993:16/r12,318/r4,5/r18,8/r20. Maximum existing-estimate
difference0.003123 (0.3123 percentage points). Median rounded/original ratio of
695 paired tiny positive estimates1.005, but wide individual estimator ratios
are not ground-truth errors; models, sample candidates, fits and time admission
all differ. Do not enable: no performance benefit and losses. Full rounded
distribution sharing is a separate potential cache experiment, not yet tested.
Reproducible script:experiments/rare_positions/experiment_poisson_mean_grid.py.

R30 shipment authorized by the user. Ship the validated portable candidate with
default tail fraction0.45 and no new required flags:20.22% cold/22.01% warm CPU
reduction,14/31 gained cell-runs and zero losses in the final cohorts.98 tests
passed,2 DB-write tests ignored; all7 full deterministic oracles identical.
R31 Poisson approximations and unrelated late-gap changes are excluded.

## R32 — More confirmation work for existing pilots

Status: default enabled and release validated; confirmation1.5 in the coverage
profile. User explicitly requested default enablement and shipping after the
experiments. Baseline ee334cfb, four cores, saved snapshots.

- Existing tail fraction0.75 versus0.45,9 pairs:11 gained cell-runs (10 distinct
  cells),0 lost,+5.95% CPU,+6.15% wall. All gains in16498; no Londrina rescue.
- Opt-in `RUST_ODDS_RARE_TAIL_CONFIRM_MORE=0.25`,9 pairs:7 gained cell-runs,
  0 lost,+3.56% CPU,+5.35% wall. Cheap confirmations of existing proposals.
- New confirmation1.5,9 pilot pairs plus14 fresh validation pairs across all7
  snapshots:34 gained cell-runs (14 distinct cells),0 lost,+27.01% CPU,
  +32.05% aggregate wall. Reachable-zero cell-runs64→30. Affected groups cost
  more:16498 mean681.61→1016.98ms; current16653 mean614.04→1002.20ms.
- Londrina/4 rescued5/5 seeds with30k independent main/check draws each,
  estimates7.12e-23–1.78e-22. Londrina/3 remains zero; concentrated weights and
  zero-hit pilots need better proposals. Weak Flamengo/12 rescued at seed1993.

Additional allowance is a fraction of measured pre-tail time, not a request
latency percentage. Reuse proposals; fixed fresh batches derive from training
hits/ESS, with unchanged quality gates and target means. Freeze ordinary
estimates before additional work; all larger runs logged preservation=true.
Clock-based ordinary funding still varies between paired requests. No new
proofs, no DB writes, no additional cores. Default confirmation1.5; explicit
0.25 selects less work and0 disables the additional stage. Combining a larger
ordinary tail allowance with the new default was not evaluated. Report and reproducible
artifacts: `experiments/rare_positions/2026-10-01-rust-more-pilot-confirmation.md`
and `2026-10-01-more-pilot-confirmation/`.
Full Rust suite:99 passed,2 DB-write tests ignored. Final release rebuilt after
timing-log correction;3 follow-up seed808 pairs had4 gains,0 losses. Those pairs
are separate from the23-request totals. `rustfmt --check` and diff checks passed.

Default enablement:100 Rust tests passed,2 DB-write tests ignored. Added coverage
default/rollback tests, runtime confirmation activation/preservation assertions,
and startup logging of the selected confirmation fraction. Existing tail0.45,
four-core cap and acceptance gates are retained.
Final no-flag release check across all7 snapshots,seed808:4 gained cell-runs,
0 losses or proof regressions; Londrina/4 accepted. Archived as `default-check`.

## R33 — Residual-difficulty conditioning loop

Status: discarded from active code on 2026-10-02; historical evidence committed.
Historical baseline Rust2290fe04, four workers. Five variants ×9 paired requests, plus14 fresh
validation pairs across all7 snapshots:59 total pairs. Final adaptive variant's
23 pairs gained4 cell-runs across3 distinct cells, lost0; reachable-zero
cell-runs30→26. All rescued cells were already found by baseline in other seeds;
Londrina/3 remained zero5/5. No new proofs or resolved undecideds.

- Small necessary-event screens handle shared fixtures exactly. Londrina's
  conditional bound falls4.39e-8→1.98e-11→1.92e-14 as3/4/5 rivals are included.
  These bounds motivate search and are not estimates or guided hit rates.
- Four-rival, widths3–6 loop:1 gain,0 losses,+3.73% CPU,+10.74% wall (9 pairs).
- Six-rival block:0 gains,+5.23% CPU,+9.83% wall; selected-team constraints fail
  to account jointly for the points their opponents receive.
- Expanded cases:1 gain,+14.07% CPU,+18.95% wall. Interval guidance alone:0 gains,
  +7.57% CPU,+10.50% wall. Each uses its own9-pair baseline cohort.
- Adaptive guidance with batches up to100k: final23 pairs,+18.42% CPU,+34.81%
  wall;16498 mean1031→1919ms, current16653993→1142ms. Added stage has a soft
  admission deadline and can overrun it. No reallocation variant was tested.
- Londrina had68/68 main/check hits at seed1669, but check ESS1.67 and estimate
  ratio5.93; rejected. More matching seasons still do not imply stable weights.

The residual-loop runtime flag and implementation were removed on 2026-10-02.
The historical candidate remains reproducible from the archived source patch;
production defaults/gates remain unchanged.102 historical Rust tests passed,
2 DB-write tests ignored. Report:`2026-10-01-rust-residual-conditioning-loop.md`;
artifacts:`2026-10-01-residual-loop/`. Next: instrument joint-draw recipient
constraint failures, test guidance for selected rivals plus opponents, and learn
case allocations from weighted training contributions. Replace existing work
before adding another production stage.

## R34 — Complete exception-branch stratification

Status: committed as offline diagnostics/evidence on 2026-10-02; successful
mechanism already integrated by R36. Confirmed Londrina/3's three
surviving exception cases: Fortaleza, Novorizontino and Atlético-GO. Three-case
equal sampling accepted2/11 seeds. Generic complete refinement of one tightly
capped rival gives22 subbranches; pilot-guided allocation/adaptive guidance at30k
draws per main/check batch accepted7/11 (4/5 development,3/6 fresh). Accepted
fractions1.67e-34–7.92e-34. Refined misses the two plain successes:7 gains,2 losses
relative to plain across these paired cell-runs. Do not replace accepted values.

- Eleven development arms ×5 seeds, plus two arms ×6 fresh seeds. Four workers;
  complete supported target paths/exception masks, original P/Q weighting,
  production sorter and unchanged acceptance gates. Pilots excluded from final.
- Refined adaptive30: stage wall~22.4ms plus7.7–12.5ms setup and~1ms model. CPU
  49.03ms/seed development,46.27ms/seed fresh (startup/setup amortized). Fresh
  comparison with plain:+36.9% stage wall,+21.3% CPU. Offline single-cell costs;
  full-request and other-group effects not measured. R32/R33 previously leave
  this cell zero5/5 development seeds; no new full-request baseline cohort here.
- Most constrained Atlético-GO branch contributes around e-40 in observed
  samples. Novorizontino branch usually dominates and retains wide weights.
  Increasing all batches to100k did not reliably improve acceptance.
- All60 library tests passed after the final guard, including four new complete
  enumeration/weighting/numerical tests. No database writes; the original run
made no commit/push. Offline evidence is now committed.

Report:`2026-10-01-rust-branch-stratification.md`;
artifacts:`2026-10-01-branch-stratification/`. Historical next step: opt-in full-request integration
that preserves earlier accepted estimates and refines remaining weak zeros;
reallocate existing tail work and test all7 snapshots with paired seeds. An
empirical plain+refined union covers9/11 here, but that portfolio is not yet a
measured or validated production configuration in that original cohort. R36
subsequently measured and integrated the successful mechanism.

## R35 — Fixed priors, joint upper bounds and branch ordering

Status: committed as offline diagnostics/evidence on 2026-10-02. Cheap ordering
is already integrated by R36; strong bounding and omission remain offline.
Audited all22 Londrina/3 branches. Fixed priors permit
skipping one negligible branch; existing joint normalizer tightens its bound to
3.854e-43. Six-rival necessary-event bounds permit skipping all3 Atlético-GO
subbranches, total omitted probability≤3.851e-36, below1% of a4e-34 reference.
Relative omission is against an estimate, not a proven true-probability floor.
Keep an explicit summed omitted-mass bound; skipped branches are not impossible.

- Eleven identical seeds ×3 repeats ×4 arms,132 cold single-cell runs, four
  workers. Same7/11 accepted seeds in every arm/repeat. Retained branch RNG and
  draw counts preserved; no reallocation. Ordering-only estimates bit-identical.
- Cheap bound omission: CPU61.18→59.64ms(−2.53%); complete offline cell wall
  31.78→31.07ms(−2.23%). Bound setup~0.013ms. Saved316 draws per final batch.
- Strong bounds: CPU67.26ms(+9.93%), complete cell wall39.09ms(+22.99%);
  bounds setup~7.53ms. Saved948 draws per final batch. Reject extra bounding
  as a speed optimization here. Amortizing setup across11 seeds reverses the
  apparent benefit; production cannot assume such reuse.
- Read-2/Holder diagnostic bounds improved0/22 over six-rival bounds. Remaining19
  branches cannot safely be omitted merely because pilots saw no hits.
- All61 library tests passed; exhaustive small-fixture branch bounds and underflow
  checks included. Final release smoke preserves bounds and accepted seed808.

Report:`2026-10-01-rust-branch-bounds.md`; artifacts:`2026-10-01-branch-bounds/`.
Recommend cheap existing bounds plus pilot allocation; use stronger bounds if
already paid for by another stage. Further integration must preserve estimates,
freeze any training-based retained union before final draws, carry omitted mass,
and measure full-request/all-group behavior. No production bound-omission
enablement; offline records/tools are now committed.

## R36 — Production-path complete-branch refinement

Status: adopted at the user's request. The ordinary arm is now enabled by
default in the coverage profile; `RUST_ODDS_RARE_TAIL_BRANCHES=0` opts out.
Adoption verification: 113 current-workspace Rust tests passed, two DB-write
tests ignored, including default-stage activation and estimate preservation.
`RUST_ODDS_RARE_TAIL_BRANCHES=1` appends generic
complete-branch refinement after native estimates are frozen. Four workers,
cheap bound ordering, independent500-draw rank/interval pilots, trained30k
main/check budgets. All branches retain positive allocation; no omitted mass.
Optional `RUST_ODDS_RARE_TAIL_BRANCH_BOUND_FLOOR=1` gives negligible branches a
two-draw floor and reallocates draws while retaining full support. Admission
allowance defaults to15% of preceding full calculation wall; deadline is soft
and overruns will be measured. The experimental two-draw floor remains off.

Compared against current Rust2290fe04 on all7 saved snapshots at7 fixed seeds,
using full `/odds` HTTP requests and four cores total.49 main-arm pairs:5 gained
cell-runs, one distinct new cell(Londrina95/3rd),0 losses or proof regressions.
Londrina baseline0/7→5/7; reachable-zero cell-runs41→36. Aggregate CPU+0.52%,
full HTTP wall+2.16%; current16653 mean1068.96→1115.15ms(+4.32%). This adds work;
it does not reallocate native work. Remaining16498 zeros exceed64 supported
target paths and skip complete enumeration after~1ms setup per cell.

- Initial5 seeds reproduce the offline arm. Two previously unused seeds2281/2293
  add1 gain in14 held-out full-request pairs, no losses. Seed2281 rejects on
  maximum weight share;1993 rejects on ESS/share. Gates unchanged.
- Alternative two-draw bound floor:35 pairs, same4 branch-stage gains on the
  initial5 seeds;2 extra native-stage gains are timing-allocation variation.
  No extra branch coverage, CPU+3.45%, wall+3.48%. Retain the ordinary200 floor.
- No-flag7-pair control:0 gains/losses, CPU+1.73%, wall+1.98%; small timing changes
  are noisy/build-sensitive.91 total pairs across all cohorts(182 calculations).
- All110 focused Rust tests passed,2 DB-write tests ignored. A concurrent
  response-format edit was preserved in the workspace and excluded from the
  isolated comparison/test patch. No commit/push, deployment or DB writes.

Report:`2026-10-01-rust-production-branches.md`;
artifacts:`2026-10-01-production-branches/`. Next: support wider target-path
families through a weighted complete-support proposal, without enumerating every
target season. Do not truncate their union. Any broader integration must retain
the measured full-request latency accounting and zero-only publication.


## R37 — deterministic proposal work budgets

Status: enabled by default; user accepts the measured 2.3% aggregate latency
increase and authorized integration into master on 2026-10-02. Server deployment
still requires a release build and restart.

Replace elapsed-time admission in the additional Rust coverage portfolio with
`setup + draws × estimated draw cost`. Independent pilots count active fixtures,
guidance/cardinality operations, density replays and ranking allowances. Setup
charges nodes/guide values; failed constructors retain deterministic node/cell
limits. Cheap proposals can fund additional independent draws within the same
modeled quota. Four cores, stable admission/aggregation and deterministic barriers.

- Draw-only prototype: 77 pairs, 8 gains / 0 losses, CPU +1.94%, HTTP +3.12%.
- Operation v1: 16 pairs, 0 gains / 2 losses; fixed larger-first branch design and
  weak-pilot priority. Preserve original 30k branch pairs before fresh retries.
- ESS/work priority plus pilot-hit reliability removed development regressions.
  Fresh seeds exposed skipped-check waste; two deterministic waves reuse it.
- Selected allocator: **98 full-request pairs**, all 7 snapshots / 14 fixed seeds,
  **7 gained cell-runs / 4 distinct cells, 0 losses or proof regressions**.
  Reachable-zero runs 72→65; undecided zeros remain 0. CPU **+1.62%**, HTTP
  **+2.27%**. Group 16498 latency +6.36%; this is explicitly an aggregate allowance.
- Reject all-mains-first (5 losses), 20% confirmation quota cut (2 losses),
  pilot-only counter specialization (same responses, slower), and 10% quota cut
  (worse wall time). Union setup cap128: 6 gains /0 losses, CPU +0.22%, HTTP +0.90%;
  keep256 for the seventh gain under the accepted allowance.
- Full Rust tests pass; response repeat and worker/logging checks are archived.
  No probability weights, sorter rules, quality gates or team/rank special cases
  were added. Setup and pilot cost proxies are not hard wall-time guarantees.

Default operation model needs no extra flags. `RUST_ODDS_DETERMINISTIC_WORK=0`
restores legacy time admission for diagnostics. Fixed HTTP seed808 remains in
force independently. `RUST_ODDS_WORK_MODEL=draws` is a diagnostic alternative.

Report: `experiments/rare_positions/2026-10-01-rust-deterministic-work.md`.
Artifacts: `experiments/rare_positions/2026-10-01-deterministic-work/`.

## R38 — certified totals before complete target paths; complete parent fallback

Status: implemented and enabled by default; included in the certified-limit release.

- Cache fully refuted target points/applicable-wins limits per request/cell before
  complete branch construction; upper/lower threshold queries share 16k nodes.
  Feasibility or quota exhaustion never removes support. Earlier pilots unchanged.
- Finish the complete primary union before optional refinement. If any split
  fails, retain the entire primary union. Refinement: 2k nodes/parent, 10k/cell.
- Transfer proof/setup/pilot/final work from existing extra confirmations; newly
  certified proposals use 25k independent main/check allocations, four cores.
- One distinct added cell: Palmeiras / 16th, group 16498, 1.68e-40 at fixed seed808.
  It gains in 3/5 distinct seeds; no selected-arm losses or proof regressions.
- Final default warm run: 56 measured pairs, 8 gained cell-runs (one cell repeated),
  0 losses. Aggregate median wall −1.03%, CPU −0.23%. Group16498 wall −4.90%.
  Per-snapshot increases explicitly remain: historical16653 wall +2.81%, CPU
  +3.78%; current16653 wall +0.40%. Held-out seed cohort wall +2.40%, CPU +0.11%.
  No added allowance assumed; aggregate production-seed budget is preserved,
  individual-input/seed latency is not guaranteed unchanged.
- Reject broad early certification (3 gains/1 loss), hybrid partial refinement
  (no gains), additive late-only work (+15.56%16498 wall), and20k batches (lose
  default-seed gain). 198 measured pairs plus28 warm-up pairs across all arms.
- 128 Rust tests pass;4 DB-dependent tests ignored. Repeat/worker/logging checks
  pass. Default16498 remaining reachable zeros: Palmeiras14/15, Flamengo12/13;
  no undecided zeros. No additional reachability/impossibility classifications.

Report: `experiments/rare_positions/2026-10-02-rust-certified-target-limits.md`.
Artifacts: `experiments/rare_positions/2026-10-02-certified-target-limits/`.
Shipping checks:128 tests pass,4 DB-dependent tests ignored; all seven seed808
responses byte-identical to the measured candidate. Seven cold pairs: wall+0.75%,
CPU+1.48%; no additional work quota. Reduced-confirmation transfer guard tested.
Next: improve the remaining complete unions and guidance without changing
previously accepted positives; remeasure latency for changed snapshots/seeds.

## R39 — Flamengo13: prune complete rank cases before setup

Status: focused offline success; expanded limits rejected for production budget.

- Group16498/team17/rank13 already has a certified cap of61 points/18 wins:
 11 target paths, necessary-event mass2.2773e-7. Current constructor skips its
 576 raw cases/path before shared-fixture checking (6,336 across11 paths).
- Generic prefix propagation completes11 paths, leaving409 non-refuted cases,
 10,251 nodes,58.83 ms wall/four cores. These are not reachability witnesses.
- A support reference safely proves two zero-mass necessary case subsets empty;
  floating zero alone never permits removal.407 complete strata remain.
- Larger offline caps (1,024 cases,100m guide values),25 pilots/stratum,40k main
  and40k independent check, four-draw floor:5/5 distinct seeds accepted, one
  distinct estimated cell. Main probabilities1.6e-34–4.7e-34; no gate relaxation.
- Single seed808 fresh process:517 ms wall,639 ms CPU,538 MiB peak RSS. Existing
  cell setup skip~1.2 ms. No full-request pair, other-group or regression claim.
  No production defaults changed; no extra allowance assumed. Offline artifacts included with the release.
- Two diagnostic prefix tests and two isolated support-reference tests pass.
- Next: lightweight prefix feasibility; discrete support refutation; reuse team
  suffix rows across cases or retain complete unsplit parent regions; transfer
  saved low-yield work and run paired full requests before enabling.

Report: `experiments/rare_positions/2026-10-02-rust-flamengo-13.md`.
Artifacts: `experiments/rare_positions/2026-10-02-flamengo-13/`.


## R40 — Resolve all remaining local experiments

Status: completed on 2026-10-02 under the user's authorization to commit or
discard each experiment. No new production method/default was enabled.

| Experiment | Final decision | Evidence |
|---|---|---|
| R33 residual loop, all five variants | Discard active implementation and runtime flag; commit negative record and archived patch | +34.81% full wall, +18.42% CPU, no newly discovered distinct cell across baseline seeds; Londrina3 still zero |
| Later gap rescue, both moved/pilot variants | Discard active implementation and flag; commit negative record | No previously missing gap estimate from either rescuer; four lost cell-runs per arm |
| High-ESS shorter point-tilt confirmations | Discard active implementation and flag | Nine-pair historical screen: no aggregate coverage improvement, worst full wall +56.2%; no validated benefit over retained policy |
| R34 complete exception branches | Commit offline drivers/evidence; retain already shipped production implementation | 7/11 accepted seeds offline; R36 later gains Londrina3 without replacing positives |
| R35 bounds and ordering | Commit offline bound audit/driver/evidence; retain shipped cheap ordering | Cheap omission −2.53% single-cell CPU; strong bounds +9.93% CPU. No omitted mass enabled in production |
| Original rare50 development arms | Commit missing generator/evidence; retain selected mechanisms already shipped; discard rejected/deferred variants as active work | Historical report gives each arm's coverage/cost decision; superseded by later defaults |
| R38/R39 unpacked raw evidence | Remove duplicates already committed in archives after byte verification | No missing evidence or code; R39 enlarged sampler stays offline |

Four modified Rust source files were restored to the released 31bc8b0a contents.
The runtime README now records the discarded residual-loop status. The unreferenced residual source was removed. The new
`branch_bounds` executable is an offline example only, not linked into server
execution. Earlier production coverage, weights, gates, deterministic budgets
and four-worker limits are preserved.

All remaining untracked odds experiment files are committed, archived or
verified duplicate copies removed. Every newly packed archive was read back and
checked byte-for-byte before deleting unpacked originals; per-file SHA256 hashes
are recorded in its manifest. No data measurements were rewritten as new runs.
Unrelated local schema/scratch files remain untouched.

Report: `experiments/rare_positions/2026-10-02-rust-experiment-decisions.md`.

R40 final validation:135 Rust tests passed (126 full-suite,9 example tests),
4 DB-dependent tests ignored. All production Rust sources match31bc8b0a;
229 archived members and22 removed duplicates verified. Offline bounds audit
retains19/22 branches with omitted mass≤4e-36. Python tools parse and historical
residual summary reproduces23 pairs/4 gains/0 losses. Logs and validation:
`experiments/rare_positions/2026-10-02-experiment-cleanup/`.

## R41 — Bounded complete constraint tree for Flamengo13

Status: complete; retain offline, reject both full-request allocation arms.
Production defaults/source unchanged. Master f0a332b8 is the Rust baseline,
four cores, saved identical requests; no new latency allowance.

Replace full rank-mask enumeration with a bounded partition of complete parent
regions. Split an unresolved rival into strict-exception and non-exception
regions; shared-fixture propagation refutes only incompatible regions. At the
rank-cardinality limit, remaining ambiguous rivals must be non-exceptions.
Retain the whole parent if either child cannot finish construction. Freeze tree
selection and allocation before fresh main/check draws; every retained leaf
has positive allocation, with the production score sorter and original P/Q.

The32-leaf trees were too coarse. A64-leaf cap under the existing4-million
guide-value/100k joint-node limits retains54 complete leaves. At25k main/check
draws it accepts3/5 seeds, main4.45e-34–6.18e-34 probability. Marginal rank-hint
priority and30k/50k final batches still accept3/5; extra tree-training adds
about90 ms without improving the initial32-leaf tree. Smaller50-draw pilots
also accept3/5 offline, with different failures. Heavy weights remain:1,452
hits can have ESS4.29 and a43% largest contribution. Almost all observed
variance comes from one already fully classified leaf, so the next target is
its joint probability proposal, rather than more rank-side splits.

Full-request500-pilot arm, five group16498 pairs: gain1/loss1, no Flamengo13
estimate, median HTTP+4.22%, CPU+1.17%, worst HTTP+34.83%. The54-leaf Flamengo
pilot pair exceeds the remaining transferred bank in all five seeds.

Full-request50-pilot arm,35 pairs/seven snapshots: gain1/loss3, no Flamengo13
estimate, no coverage change in the other six snapshots. Group16498 median
HTTP−8.52%, CPU−1.95% by reducing confirmations; worst paired HTTP increase
across the cohort+11.99%, aggregate CPU+0.57%. Gain: Palmeiras16 seed1669,
6.08e-41. Losses: Flamengo12 seeds1993/2281 and Palmeiras14 seed2293,
about1e-25–9e-25. All reachability/impossibility proofs are retained. The
operation-bank limit is preserved, but failed setup still incurs wall time.

Fresh repeated single-cell processes: median227.69 ms wall, approximately450 ms
CPU,59.56 MiB RSS; setup54.47 ms and sampling155.69 ms. This is an offline cell,
not the cost of the full endpoint. Historical407-stratum/R39 timing is not a
paired current comparison. Tests:129 Rust passed/four DB tests ignored, two
Python tests passed, three flag-off HTTP controls identical. The comparison
helper now recognizes normalized HTTP `reachable`; estimates lost by allocation
are not mislabeled as lost reachability proofs. All212 archived members verify.

Next: lower weight variance inside the dominant complete leaf, reuse setup
metadata, and fund a minimum final pair before spending transferred pilots.
Keep complete support, independent final streams and current quality gates.
Neither allocation policy is enabled or committed/pushed by this experiment.

Report: `experiments/rare_positions/2026-10-02-rust-flamengo-tree.md`.
Artifacts: `2026-10-02-flamengo-tree/` (prototype patch, compact summary,
verified raw archive/manifest and reproducible commands).

## R42 — Improve complete-branch importance proposals

Status: experiment complete; user authorized adopting the measured policy on
2026-10-02 despite CPU +2.48% and median HTTP latency +4.58%. Production
integration and validation follow below.
Current Rust baseline/four workers. User allows a
proportional increase in work measured against recovered rare cells: final
positive cells with zero initial100k-MC observations. Report net gains/losses,
CPU and full-request wall time; use the existing Rust request as denominator.
Baseline group16498 recovers82–88 rare cells across five fixed seeds, so one
additional cell alone supports only about1.2% more work. Start with R41's complete frozen tree and identify whether heavy weights
come from score tilting, rank guidance, or joint-selected fixtures which ignore
the remaining rivals' constraints. Screen alternatives with identical requests
and seeds, then measure any useful proposal by reallocating existing work.
Preserve complete event support, actual sorter checks, fresh final batches and
the existing acceptance gates. Prototype code remains isolated.

R42 results:

- Three-pass fixture/team interval messages with10% defensive original outcome
  probabilities reduce heavy weights inside the complete tree. Rebuild the
  exact joint normalizer underQ and correct every joint/guided fixture byP/Q.
  Messages are proposal heuristics, never proofs or estimates by themselves.
- A complete54-leaf tree,25 pilots/leaf and3k fresh main/check draws estimates
  Flamengo13 in10/10 seeds (five development/five holdout),4.50e-34–1.17e-33
  probability. No team/rank-specific constructor or relaxed gate was added.
-70 paired full requests/seven snapshots:10 gains/0 losses, **one distinct
  cell**, no reachability/impossibility regressions. All existing probabilities,
  standard errors and game importance are unchanged. Other snapshots gain0.
- Count against final positive cells with zero initial100k-MC observations.
  Group16498 rare cell-runs846→856 supports **1.182%** extra work. Across the
  full cohort4,319→4,329; work credit stays local to each affected request.
- Transfer/refund-only policies lost existing cell-runs. Actual pilot accounting,
  priority-prefix confirmations and conditional conservative credit preserve
  coverage. Configured credit≤0.763% fits the proportional allowance, but
  measured CPU/wall do not reliably fit it.
- Exact memoized initial-mass evaluation retains every backward-DP floating bit
  and cuts tree setup51.40→26.47 ms. All70 candidate exports match the slower
  prototype exactly. Full tables remain in use for actual conditional draws.
- Warm repeated servers: CPU31.01→31.78 seconds (+2.48%); median full HTTP
  1,249.80→1,307.07 ms (+4.58%). Four workers retained, peak RSS395.91→409.77MiB.
  Fresh-process measurements were noisy; do not treat the holdout timing drop
  as a demonstrated speedup. No additional blanket latency allowance assumed.
- Discard stronger interval powers, smaller trees, stronger/more message
  passes, the marginal cache (CPU+6.63%), and the strong point-tilt short-batch
  rule (no eligible cells). Preparation pipelines did not establish a cost win.
  Naive message reuse on Palmeiras16 with5k final batches accepted0/5.
-141 Rust tests passed/four DB tests ignored; five Python tests passed. Four
  flag-off HTTP controls exactly match baseline exports. Patch reconstructs
  the ten changed source files; raw evidence is archived with verified hashes.

Adoption: `RUST_ODDS_RARE_TAIL_TREE=1` is now the coverage default. The adopted
policy uses64 leaves,25 pilots/leaf,3k main/check allocations, three message
passes on one pilot-selected leaf,10% defensive original probabilities,
priority-prefix confirmations, unused-check refunds and conditional credit
capped at16m units. `RUST_ODDS_RARE_TAIL_TREE=0` restores the prior allocation.
Rejected preparation pipelines and prototype-only runtime tuning switches were
removed. Historical patch/raw evidence remains unchanged. Do not claim this
fits the original proportional CPU budget solely because configured credit does.
Production adoption checks:143 Rust tests pass/four DB tests ignored; five
Python tests pass.70 paired HTTP exports/seven snapshots match the validated
prototype byte-for-byte. Four flag-off controls reproduce prior production;
worker/logging/repeat invariance and reduced-budget checks pass. New warm checks
against prior production measure CPU+5.17–6.81%, HTTP+6.66–8.05%; the direct
prototype comparison measures CPU−3.39%, HTTP+0.97%. These observations exceed
the earlier+2.48%/+4.58% timing and the original proportional allowance; no
additional sampler budget was added. Report and verified adoption archive record
all measurements and commands. User authorized commit and push on2026-10-02.

Report: `experiments/rare_positions/2026-10-02-rust-flamengo-messages.md`.
Artifacts: `2026-10-02-flamengo-messages/` (patch, commands, compact summary and
verified raw log/export archive).

## R43 — Refine unstable lazy target-path proposals

Status: complete; reject all five variants (2026-10-02). Baseline is shipped Rust commit ac92a8ab,
with the complete-tree policy enabled and four workers. Prototype sources and
binary are isolated under /private/tmp/golaberto-r43; production code stays
unchanged. No additional work or latency allowance is assumed.

Test reusing cached certified target limits for unresolved lazy proposals,
then applying fixture/team messages inside pilot variance-leading target paths.
The target dynamic program retains support beyond 64 enumerated paths. Charge
reconstruction, message setup and training against the existing confirmation
allocation; fresh independent main/check batches must pass unchanged gates.
Preserve original fixture probabilities for omitted games, exact joint
normalization under Q, every active P/Q correction, actual sorter verification
and defensive support. Compare fixed requests/seeds across seven snapshots,
including coverage regressions, modeled work, full HTTP latency and CPU.

R43 checkpoint: exact small-league tests pass, including omitted fixtures,
mixture support outside a witness subset and the 186-path Flamengo12 terminal.
Five group16498 development seeds: rebuilding certified proposals plus primary
messages gains0/loses0; preserving cached witness alternatives with10% native
conditional support gains1/loses2;50% native support also gains1/loses2. Reusing
certified limits without messages gains1/loses2 (different cells). Failed
preparation is charged by shortening its original confirmation pair. These arms
are rejected; no production change. Next screen uses interval guidance within
the retuned witness proposal, matching the successful complete-tree approach.


R43 final results: the50% native/rank-guidance arm was validated on70 paired
full requests/seven snapshots/ten seeds. Gains3/losses5, two distinct gained
cells (Flamengo12, Palmeiras14). Losses include Flamengo12 and Palmeiras13
(6.85e-20 at seed1861). Flamengo12 coverage6/10→3/10. No new reachable or
impossible cells and no proof/undecided regressions. The other six snapshots
have60 byte-identical exports. Initial MC and game importance are unchanged.
Recovered rare cell-runs4,329→4,327 (16498:856→854); no proportional credit.

Current-baseline warm comparisons, four workers, two runs of10 requests per
binary/two warmups: CPU+9.75–14.67%, median HTTP+10.46–12.21%. This does not
preserve full-request cost, despite unchanged operation-bank limits. Successful
mains spend checks the baseline skipped. Every attempted refinement is charged
within its original pair;32 preparations average58.98 ms and62.22m modeled
units per affected request.146 Rust tests pass/four DB tests ignored; five
Python tests pass. Four flag-off controls and ten diagnostic exports are
byte-identical to expected outputs; enabled worker/repeat/logging invariance
passes. Production source/defaults are untouched; no commit or push.

Weight diagnostics resolve the next target: the dominant weighted mass remains
in Flamengo's60-point/18-win all-loss target path. The largest weights in
seed2281 main and seed2293 check are outside the retuned witness subset, with
score correction1 and shares45.35%/83.66%. Tightening target totals or stronger
goal tilting does not address that observed failure. Next experiment should
partition the witness subset and its complement into disjoint regions inside
the existing lazy root. Up to six interval conditions yield at most seven
ordered first-violation regions. Rebuild exact normalizers, retain the complete
parent on incomplete splitting, and replace existing work. This partition is
not implemented in R43.

Report: `experiments/rare_positions/2026-10-02-rust-lazy-messages.md`.
Artifacts: `2026-10-02-lazy-messages/` (focused prototype and diagnostic patches,
rejected-variant overlays, commands, results, verified raw archive/manifest).

## R44 — Partition lazy witness subsets and their complements

Status: complete; reject all four variants (2026-10-02). Shipped Rust ac92a8ab is the baseline;
prototype sources and binaries are isolated under /private/tmp/golaberto-r44.
Four compute workers, fixed requests/seeds, no extra work or latency allowance.

Replace a funded unstable lazy confirmation proposal with an ordered
first-violated-interval partition. Up to six witness-side conditions yield
at most seven disjoint regions. Construct shared-fixture domain restrictions
and exact joint normalizers; keep the complete parent when construction is
incomplete. Learn branch allocation from separate small pilots, retain native
support and verify ranks with the production sorter. Charge attempted setup
and training against the original pair; independent main/check gates stay
unchanged. Compare coverage, regressions, proof statuses, full-request latency
and CPU against current Rust, including seven reference snapshots if promising.

R44 checkpoint: exact partition coverage, density recovery with shared fixtures,
omissions, interval/cardinality guidance and parent fallback tests pass. First
five-pair screen with existing constructor ceilings:10 attempted partitions,
all fall back; gains0/losses0. Reallocation to at most60k additional joint nodes
and4m guide values, charged within the existing pair, completes the partitions;
parallel four-worker setup averages about10ms per attempted cell. Five pairs:
gains0/losses1 (Flamengo12/seed1669). Complement pilots usually see no hits;
the six-bound witness leaf receives nearly all learned allocation. Next screens
retain50% native support and test cardinality guidance in those broad complements.
No production change or extra latency allowance.


R44 final: V3 (50% native support) gains0/loses1 in five development pairs.
V4 (also offering cardinality guidance) gains1/loses2 in70 paired requests,
seven snapshots, ten seeds. The only changing cell is Flamengo12:
6/10→5/10; recovered seed2293 P=2.80e-26, lost seeds1669/2281. Other six
snapshots have60 byte-identical exports. Initial MC, game importance and all
proof classifications are unchanged; rare positive cell-runs4,329→4,328.
17 preparations/6 selected; construction+training14.62ms per attempted cell,
20.63m modeled units per affected request. Warm seed808: CPU+9.73%, HTTP+11.90%,
RSS+3.19MiB. A shortened retained Palmeiras15 main triggers a failed26,411-draw
check the baseline skipped, increasing actual work under the same bank limit.
146 Rust tests pass/four DB ignored; five Python tests pass; flag-off exports
and worker/repeat/logging invariance pass. All four variants rejected. Production
source/defaults untouched; no commit or push.

Report: `experiments/rare_positions/2026-10-02-rust-lazy-partitions.md`.
Artifacts: `2026-10-02-lazy-partitions/` (source/overlays, commands, paired
summaries, verified raw logs/exports/tests and hashes).

## R45 — Weighted pilot witness portfolio

Status: complete; reject (2026-10-02). Isolated prototype, production unchanged. Existing R4/R14 already use witness proposals
and rival masks. Retain a few distinct largest-weight verified seasons from
existing pilots per target path; use their different rival-side patterns as a
small frozen proposal portfolio. This tests weighted selection of multiple
observed modes, avoiding fresh pilot draws and broad zero-hit complement
branches. Correct full overlapping-mixture P/Q, defensive support, fresh main/
check batches and production sorter verification are required. Replace existing
work; do not infer spare latency from unused bank limits. Pair against current
Rust/four workers for coverage, regressions, actual CPU and request time.

R45 result: seven snapshots/five seeds/35 paired requests gain2/lose3;
recovered rare cell-runs2,166→2,165. Gains Flamengo12/seed808 and
Palmeiras13/seed2293; loses Athletico19/seed808 and Flamengo12/seeds1669,2281.
Flamengo12 coverage3/5→2/5. Learned initial-pilot modes do not cover enough
high-contribution outcomes to repay setup and mixture replay. Warm HTTP changes
−0.80%/+5.51%/−1.11% for16498/current16653/earlier16653; CPU
−3.50%/+5.68%/−2.28%. No additional time allowance or proportional work credit.
Initial MC and proof classifications are unchanged. No production change.


## R46 — Shared-fixture pair in cardinality guidance

Status: complete; reject all three revisions (2026-10-02).
Isolated prototype, production unchanged.
Choose one uncertain pair of rivals with at most two mutual residual fixtures.
Enumerate those mutual outcomes and convolve independent external tails to obtain
its joint 0/1/2-above-target count distribution. Replace the two independent
Bernoulli terms when the current fixture is disjoint from the pair. This is
proposal guidance, not a reachability certificate; actual P/Q and the sorter
remain authoritative. Charge setup, count-kernel calculations and replays to the
existing deterministic bank. Compare fixed requests/four workers with ac92a8ab.

R46 result:35 pairs per revision. Uncached guide gains0/loses18; cached guide
gains1/loses11; final allocation-bounded/appropriately metered guide also
gains1/loses11 (2,166→2,156 recovered rare cell-runs). Only gain Palmeiras13
at seed1669; losses include every Flamengo13 seed (5/5→0/5), and Flamengo12
falls3/5→1/5. Pair admission is checked before dense suffix allocation;
selection is one fixture pass with cached uncertainty and explicit work charges.
Incomplete pair preparation retains the native Pattern. No proof changes.
Final warm latency/CPU results are recorded in the report below.
Bounded revision warm HTTP changes+6.52%/−0.05%/+3.22% and CPU
+2.10%/−3.80%/+1.61% for16498/current16653/earlier16653. Reject;
no production change or proportional credit.

## R47 — Marginalize one residual fixture

Status: complete; reject (2026-10-02); isolated prototype, production unchanged.
For a cached lazy target path, select one uncertain residual fixture before
sampling. Sum its three original-probability-weighted rank contributions and
use the complete proposal's marginal density, including alternate overlap.
Other fixtures still come from the native proposal. Apply score likelihood
corrections to each alternative and keep a verified contributing witness.
Charge all density replays and sorter calls, shortening funded final batches
through current deterministic costs. No new runtime allowance or weaker gates.

R47 result:35 paired requests gain1/lose8; rare cell-runs2,166→2,159.
Only gain Palmeiras13/seed2293; seven losses in16498 and Londrina4/seed1669
in current16653. Flamengo12 coverage3/5→0/5. Warm HTTP changes
+18.27%/−0.23%/+9.35%; CPU+21.08%/+6.73%/+17.67% for
16498/current16653/earlier16653. Three corrected rank alternatives cost too
much per funded draw. No proof classification changes. Reviewed derivation,
but independent forward-Q enumeration and stochastic score-tie integration
tests would still be required before promotion. No production change.

R45–R47 report: `experiments/rare_positions/2026-10-02-rust-lazy-three.md`.
Artifacts: `2026-10-02-lazy-three/` (focused prototypes, source hashes,
commands, paired summaries, review notes, verified raw archive/manifest).

All three modes pass seven serialized worker/repeat/logging invariance exports.
Four flag-off controls per initial/final prototype are byte-identical to shipped
Rust. Final suite152 passed/four DB ignored; focused formatting passes, with
whole-crate formatting differences confined to two unchanged baseline files.
Prototypes remain isolated; no commit or push.

## R48 — Learn later retry modes from already funded long batches

Status: measured and rejected for promotion (2026-10-02).

The bounded prototype completed 35 paired requests over seven snapshots and five
seeds. Rare cell-runs changed 2,166→2,165: no gains and one loss, Flamengo/12th
at seed 1993. The union across seeds remained 503 distinct rare cells. Only two
eligible retry jobs learned modes (old 16653 Londrina/3rd and current 16653
team 125/11th); neither was a difficult Flamengo cell. Collection recorded 239
positive contributions and charged 14,151,796 operations. Thus this experiment
tests a narrow adaptation of existing ordinary Lazy retries, rather than proving
that later learning cannot help. Do not enable this arm. Timing, limitations and
reproducible evidence are in
`experiments/rare_positions/2026-10-02-rust-state-late-splitting.md`.

R43/R44 weight diagnostics and R45's single learned initial-pilot mode suggest
that short pilots miss high-contribution regions. Retain several distinct
high-contribution verified seasons from already funded main/check batches.
Freeze their modes for a later independent retry, preserving native support and
correct overlapping-mixture density. Do not reinterpret earlier samples under
the adapted proposal or weaken independent confirmation. Replace existing work;
measure global coverage and actual CPU/latency. This proposal earns no budget
credit until gains are measured against the current Rust baseline.

## R49 — Merge equivalent states in the exact joint constructor

Status: first arm measured and rejected (2026-10-02); broader admission deferred.

The fixed-selection Rust prototype completed 35 paired requests over seven
snapshots and five seeds. Rare cell-runs (zero initial MC hits, positive final
estimate) changed 2,166→2,164: three gains and five losses. No new distinct rare
cell appeared in the union across seeds; Palmeiras/14th disappeared from that
union. Proof classifications and initial MC were unchanged. Logged successful
lazy constructors reduced 92,567 history transitions to 89,910 merged-state
transitions, only 2.9%, while charging hashing, retained edges and reconstruction.
Do not enable this arm. Warm full-request latency increased 16.4% for 16498 and
4.3% for current16653. Timing and the other two experiments are recorded
in `experiments/rare_positions/2026-10-02-rust-state-late-splitting.md`.

Inspection of `NecessaryJoint::metered` in
`odds-rust/src/joint_caps/propagated.rs` finds separate internal-fixture outcome
histories retained as `(outcomes, added, mass)`. Selection limits the product
of internal outcome counts to12,000; construction also has a20,000-node limit.
The existing terminal and proposal caches do not merge these internal histories.

At the same fixture step, histories with identical accumulated packed
points/applicable wins for every selected team have the same remaining
conditioning calculation. Sum their prior masses into one state. Retain
weighted predecessor transitions so sampling reconstructs a complete outcome
history with its correct conditional probability. For example, two games
between A and B where each wins once give the same final points/wins for both
teams, regardless of which game each won. Their goals and actual fixtures must
still be reconstructed and checked through the normal score sampler/sorter.

First test state merging with unchanged selected teams and constructor quotas.
Measure history count versus unique state count, setup CPU/wall/RSS and replay
cost. Then separately test admitting more jointly constrained rivals under
the same deterministic setup/sampling bank. Count transitions, state hashing,
predecessor storage and reconstruction work. Do not assume fewer states means
lower full-request latency or allow uncapped admission after removing the old
outcome-product guard. Exhausted construction retains the complete fallback.

Exact enumerated small fixtures must match joint normalizers, reconstructed
outcome probabilities and full-rank estimates including score tiebreakers.
Never merge on the target total alone: every future-relevant selected team
must remain in the key. Subsequent frontier elimination may discard a team's
numeric state only after its remaining constraints are fully evaluated; that
is a separate experiment.

Related research: [Fichte et al. (2018), weighted model counting through dynamic
programming on small-width decompositions](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ESA.2018.28).
That paper's GPU results are not a speed claim for this native four-core Rust
adaptation.

The proposed sequence was R49 state merging, R48 modes learned from funded long
batches, and R5 discrete splitting. All three now have isolated prototypes and
paired measurements below. Discrete rank ties require an appropriate correction;
see [Walter (2015)](https://arxiv.org/abs/1507.00919). These are experimental
adaptations, with no production change.

## R5 follow-up — Fixed-level binary branching experiment

Status: implemented, measured and rejected for promotion (2026-10-02).

Both initial and corrected native prototypes completed the same 35 paired
requests. Rare cell-runs changed 2,166→2,157: two gains and 11 losses; distinct
rare union 503→502. Neither sampler reached a complete season in any of 77
pilots. The corrected version reuses RNG streams within particle segments,
initializes independent child streams only at branches, and charges actual seed
initialization work. It funded 14,908 roots and reduced summed pilot sampling
time from 9,208ms to 385ms; part of that reduction comes from funding fewer roots.
No splitting estimate was accepted. Coverage gains were native work
redistribution, not successful splitting. An accidental six-default-seed audit
(42 pairs) also found no completed seasons and regressed 11 cell-runs; it is
retained separately and is not pooled into the matched five-seed result.

Do not enable this temporal-checkpoint version. Corrected warm latency increased
10.6% for16498 and57.8% for earlier16653; current16653 was faster but lost
Londrina/4th. All final arms passed four flag-off controls and six
worker/repeat/logging invariance exports. A future arm needs native
residual guidance or stronger shared-fixture viability before branching, with
full P/Q correction and independent-root uncertainty. Timing and evidence are in
`experiments/rare_positions/2026-10-02-rust-state-late-splitting.md`.

The bounded native prototype is independent of R48/R49. Sol review specifies
three fixed residual-fixture checkpoints, binary splitting and one statistical
observation per independent root tree. One-step necessary packed-rank predicates
restrict each fixture; original P/Q and branch weights correct every leaf.
Production score sampling/sorting checks completed leaves. Worst-case cost for
each full root is reserved before drawing; admitted trees finish without
budget truncation. This replaces an existing failed-pilot retry, preserving
ordinary initial pilots and independent fixed main/check gates. Compare exact
small fixtures, coverage, root ESS, setup and actual CPU/latency against current
Rust/four workers; no additional allowance or promotion presumed.

## R50 — Learn weighted modes from complete native proposals

Status: implemented and measured (2026-10-02); both variants rejected for
production promotion. No commit or push; isolated source patches retained.

Broaden R48 to the alternate-root proposals used by the difficult cells. In
group16498 seed808, Flamengo/12th has95 ordinary main hits but ESS2.88; its longer
native confirmation has366 hits but ESS3.98. The largest weight accounts for
42% of that confirmation estimate. Earlier R48 declined alternate-root parents
and did not train this cell. These data motivate learning the high-contribution
fixture patterns already encountered, with separate outcome/goal-weight
diagnostics to identify the source of concentration.

Preserve the full native proposal, including its alternate mixture. Learn at
most a few distinct winning modes; freeze them before fresh independent fixed
main/check batches and evaluate the complete overlapping density. Use existing
confirmation funding, charging observation, setup, replay and sampling work;
the current Rust four-worker implementation is the paired baseline. Design
review selected passive capture from native first-wave mains and fresh learned
retries before second-wave admission, preserving accepted first-wave estimates.
Cases unsupported
by exact density composition must retain the native proposal. No reachability
or probability floor is inferred from a witness, and no publication gate is
weakened. Full-request latency increases must be reported explicitly.

Measure group16498 first, then the seven reference snapshots and explicit seeds
808,1669,1993,2281,2293. Track rare cell-runs and distinct cells, lost estimates,
exposure of alternate-root jobs, outcome/goal concentration, setup and complete
request timing/CPU, exact small-fixture laws and repeat/worker/flag-off invariants.

Results: two 35-pair full-request cohorts completed using four workers. The
12-ESS pilot gate admits no finals: ten fitted pilots have ESS at most2.696.
Coverage stays2,166 rare cell-runs /503 distinct identities, with0 gains/losses.
Removing the extra gate yields2 gains /1 loss,2,167 rare cell-runs /503 distinct
identities. Group16498 gains Palmeiras15/seed808 (4.19e-33 fraction) and
Flamengo12/seed2293 (9.68e-26), but loses Palmeiras14/seed2293 (baseline8.88e-25).
The lost native20k main is identical; the learned pair displaces its236.4M-unit
independent check. First-wave accepted pairs remain unchanged.

Warm group16498 latency increases1,258.95→1,515.82ms (+20.40%); CPU increases
8.80%. Gated arm gains no coverage; its warm rare-tail stage also grows slightly.
Modeled bank compliance therefore does not establish latency preservation.
Keep both disabled. Remaining reachable-zero cell-runs change16→15 in the
ungated arm, with no undecideds or proof changes. Report, reproducible patches
and raw evidence: `experiments/rare_positions/2026-10-02-rust-parent-modes.md`.

Next useful control: learn native-pattern root allocation from long native
batch draw counts and corrected second moments. In the first166 captured
Flamengo12/seed808 positives, one root accounts for99.998% of the corrected sum;
outcome ESS1.829 versus goal-ratio ESS101.810. This motivates a root-allocation
test, without proving whether the remaining variance is between or within roots.
Reserve native second-wave checks before refinements, retain complete support,
freeze allocations before independent batches, and use the new root probability
in P/Q. Replace failed confirmation work; no extra latency allowance presumed.

## R51 — Learn native root allocation from long confirmation batches

Status: implemented and measured (2026-10-02); rejected for production
promotion. Isolated source patch retained; production unchanged. No commit/push.

R50 learned extra fixture patterns but left root frequencies unchanged. Test the
smaller control: learn frequencies for the existing native roots from long
first-wave confirmation mains, with corrected contribution second moments and
per-root draw costs. Preserve the original conditional fixture/goal laws and
the 10% defensive target-root component. Compare second-moment allocation with
a cost-adjusted version, both mixed equally with the current guided allocation.
No new fixture modes, reachability rules or publication gates.

A fresh native-proposal retry control uses the same remaining-bank policy and
new independent streams. It separates root-reweighting gains from the benefit
of another attempt. All arms retain the existing estimate acceptance gates.

Protect every native second-wave admission/check before refinement. Bounded
passive observations use a separate ledger, settled after native work; this is
retrospective accounting, not strict prefunding. A ledger overdraft must be
reported, decline refinement and exclude the request from promotion evidence.
Only remaining capacity and checks actually skipped after the native second
wave can fund independent learned pilot/main/check batches. Freeze allocations
before fresh draws and include the new root-selection probability in P/Q.

Measure group 16498 first, then the seven snapshots and fixed seeds
808,1669,1993,2281,2293. Record coverage gains/losses, reachable zeros, unchanged
native results/admissions, within-root versus between-root concentration,
accounting/setup/sampling work, CPU and full-request latency using four workers.
No additional latency allowance and no production promotion assumed.

Results: native-retry, second-moment and cost-adjusted allocation each completed
35 paired full requests, plus separate development screens. All retain 2,166
rare cell-runs / 503 distinct identities, with 0 gains/losses and 16 reachable-zero
cell-runs spanning 6 identities. All 105 pairs preserve native confirmation
results; the four differing exports per arm change only `work_spent`. Eight
fresh pilots per arm lead to one native final or two reweighted finals, all
rejected. No modeled accounting overdrafts occur. Four flag-off controls and
all 18 repeat/worker/logging invariance exports pass; 88 library tests pass.

Flamengo/12th, seed 808, has 99.9755% empirical within-root variance in the captured
7,142-draw window. Its dominant root already receives 64.56% complete proposal
probability; reweighting raises this to 77.28%, predicting 16.46% lower training
second moment. The fresh main reaches ESS 6.489, but independent check ESS 2.380
rejects. Palmeiras/15th shows 99.9870% within-root variance and only 2.84% predicted
training improvement. These finite/coarsened diagnostics identify conditional
weight dispersion as the main remaining limitation; they are not population
variance guarantees.

Warm group 16498 full latency grows 12.78% for the native control, 5.86% for moment
and 5.14% for cost, without coverage gain. CPU changes are small/mixed. Keep all
arms disabled: unused modeled reservations do not imply free physical time.
Next useful research should improve residual conditioning within productive
roots and preserve native checks, rather than repeat root reallocation alone.
Report, reproducible commands, patches and evidence:
`experiments/rare_positions/2026-10-02-rust-root-reallocation.md`.

## R52 — Exact weighted counting after complete rank-case propagation

Status: completed (2026-10-03), isolated offline experiment; production unchanged.

The Flamengo/12th diagnostic has 186 target patterns and 5,884 complete rival
cases. Implemented weighted counting with forced-fixture factors, fixed-rival
removal, disconnected components, frontier memoization, exact unary gain
integration, cached terminal profiles and optimistic unary cutoff bounds.
Rank-settled and tie-sensitive mass are separate; exhausted budgets retain
explicit unresolved upper mass.

All nine single-win cases complete in 43.30 ms of counting / 1.75 ms preparation,
using 221,565 nodes. Their combined points/wins-feasible mass is
1.413430200674237e-37 (fraction), all tie-sensitive, hence negligible even if
every goal tie favored the requested rank. A full four-worker run completes
1,470/5,884 cases in 12.37 s process / 44.04 s CPU. A deeper all-loss run takes
41.25 s / 162.57 s CPU and completes only 119/531 cases. Unknown mass still
dominates, so no full Flamengo/12th probability or overall tie effect is known.
The conditional goal-simulation stage was not implemented or measured.

All 14 focused tests pass, including exhaustive small graphs and tiny cutoff
tails. Final one/four-worker numerical invariance and cached/uncached mass
parity pass. The production diagnostic baseline computes the complete matrix
in about 1.28 s; this offline counter does not fit its latency. Do not adopt
full exact counting. Next useful experiment: conditional Monte Carlo over shared
fixtures with exact unary gain weights, correct proposal likelihood ratios and
native score sampling for ties. That follow-up remains untested.

Report: `experiments/rare_positions/2026-10-02-rust-exact-rank-cases.md`.

## R53 — Best-first allocation by unresolved case mass

Status: completed (2026-10-03), isolated offline experiment; do not adopt either arm.

Replace equal node quotas in R52 with deterministic four-job scheduling waves.
Initialize conservative case bounds, prioritize the largest absolute unresolved
mass, and discard cases whose points count completes. Keep tied mass separate
from settled rank probability. Stop only when the sum of all remaining unknown
and tie-sensitive mass meets a predeclared absolute tolerance or a relative
tolerance based on a settled-rank lower bound.

The first implementation uses staged replay rather than a resumable frontier:
each larger call replaces the previous result, and every replayed visit counts
against the global node budget. Compare with equal allocation using the same
binary and actual node budget; report setup, scheduling, CPU and wall overhead
separately. Exhausted case/memory/global limits retain unresolved mass.

R52's largest 500 cases hold 99.99194% of unresolved mass, but the remaining
bound is still approximately 1e-10, far above the sampled Flamengo/12th scale.
Concentration alone does not certify a negligible remainder. No production
integration, latency increase or commit is assumed.

Results: same request and approximately 93,856,259 charged visits. The equal
control completes 1,470/5,884 cases and leaves U=1.244210916e-6. Largest-bound
staged replay completes none, leaves U=1.257819053e-6, and takes 44.13 s /
73.97 s CPU versus 11.40 s / 42.27 s CPU. About 50.52% of visits repeat prefixes;
only 108 cases receive positive-node calls. The largest case's bound barely
changes between 20,000 and 500,000 visits.

A one-pass capped proportional upper-bound comparator spends exactly the same
93,856,259 visits, completes none and leaves U=1.244211564e-6. It takes 17.22 s /
62.85 s CPU versus its fresh same-binary equal control's 10.79 s / 40.30 s CPU.
It counts 2,055 cases; the 3,829 zero-quota bounds sum to 8.965e-13. The 1e-27
stopping target is not reached. Raw bound priority does not improve the total
bound here, even after replay is removed.

All 26 tests pass. Nine-case exact profiles agree with R52/equal allocation,
and one/four-worker scheduler/share invariance checks pass. Both equal control
versions reproduce the same numerical outputs. Keep the combined-remainder
stopping rule, but leave these allocation arms offline. Next research: resumable
partial-state prioritization with tighter shared-fixture bounds and observed
bound reduction per deterministic work unit; this remains untested. Production
source/defaults, coverage and latency are unchanged.

Report: `experiments/rare_positions/2026-10-03-rust-best-first-cases.md`.

## R54 — Transfer neighboring-rank proposals into a missing rank

Status: completed (2026-10-03), isolated experiment; production unchanged.

User requested starting Flamengo/12th from the successful 11th- and 13th-place
work. Existing neighborhood season reuse and shared-constraint sampling do not
directly transfer those successful proposals: lazy seed selection filters for
the requested rank, and the complete 13th-place branches execute after initial
12th-place lazy sampling.

Capture bounded donor fixture patterns from already funded native weighted hits
without changing their RNG streams or estimates. Compare guidance from the
better neighbor, worse neighbor and both equally normalized. Rebuild the
recipient's own target law and propagated domains, retaining the native
conditional component and defensive root support. Donor observations train the
proposal only; fresh independent pilot/main/check batches provide estimates.
Preserve actual score sorting, mixture likelihood corrections and quality gates.

Replace eligible recipient additional-confirmation work within its existing
reservation, including capture, fitting, setup and pilot costs. Protect other
native jobs and report any physical latency increase even if modeled work fits.
Four workers total. First compare group16498 at fixed seeds; expand to the seven
reference snapshots if the screen supports doing so. Track rare coverage gains,
losses, remaining zeros, setup/sampling/full-request CPU and wall time, and
repeat/worker/flag-off invariants. No commit or production promotion assumed.

User steering during implementation: include a ten-fold experimental budget
to learn what the proposals can achieve. Keep current-budget measurements as
the production comparison, and compare native retry versus neighbor transfer
at the same ten-fold recipient search grant and pilot allocation. Preserve four
cores and the ordinary scout/pool. Report the additional full-request latency
and CPU explicitly; the learning arm does not change production defaults.

Results: current-budget five-seed screen finds Flamengo12 in3/5 native runs,
2/5 with11th guidance,3/5 with13th guidance and2/5 combined. No new coverage;
two lost cell-runs are the same cell/seed in different arms. At an equal10×
recipient confirmation grant, native finds5/5,11th-only5/5,13th-only4/5 and
combined4/5. Transfers add no coverage over native10 and lose two cell-runs.
The two native10 recoveries are one distinct cell (Flamengo12), with estimates
1.030–1.314e-25 across the five seeds. Seed1993 has no eligible new grant and
keeps its existing positive estimate. Targeted executed jobs settle within the
enlarged grants; acceptance gates are unchanged.

Current native medians are about1.37–1.41s wall /3.24–3.25s CPU; enlarged native
medians2.65–2.94s /4.81–5.25s. These are separate timing cohorts. The grant is10×
for the selected confirmation, rather than10× for ordinary MC or the full request.
No current-budget or enlarged transfer arm is recommended for production.
Investigate selective extra native confirmation and explicit joint rival-case
transfer next; hard rival-side transfer remains untested. Preserve the corrected
experimental patch and raw paired evidence in the R54 artifact directory.

Validation:138 Rust tests pass,3 database-dependent tests ignored; the HTTP
tests pass with localhost binding allowed. V4 disables unused late capture and
adds gate diagnostics, preserving the v3 native10/both10 seed808 exports exactly.
Six-run worker/logging and flag-off invariants pass. The failed combined seed808
main exceeds the batch-gap limit1.5; its relative SE, ESS and max-share criteria
pass. Both cohort archives and the corrected-source evidence bundle have
verified member hashes. No production code change, commit or push.

Report: `experiments/rare_positions/2026-10-03-rust-neighbor-transfer.md`.

## R55 — Isolate Flamengo's three-point contribution

Status: completed offline analysis and numerical verification (2026-10-03).
User requested breaking Flamengo12 into point totals outside the main code.
Three points divides into one win (final63 points/19 wins) and three draws
(63/18). The single strict-below grouping applies only to the one-win subset.
Its nine retained target paths share Botafogo/Vitória as the two ambiguous
strict-below rivals; three-draw paths retain337 cases across120 target patterns.

Rerunning R52's unchanged exact counter with10× node limits completes all nine
one-win cases in221,565 nodes,2.001ms preparation and38.032ms counting. Numerical
profiles agree with the prior result. Five cases have zero mass; four sum to
1.413430200674237e-37. All positive mass needs goal tiebreakers:8strictly above
and3tied, or9above and2tied. Every tied rival must beat Flamengo for rank12.
This is an upper bound on this subset's rank probability, not its estimate.

Defer its score correction: even that upper bound is roughly12 orders below
the whole-cell estimates around1e-25. An exact conditional score simulator
could estimate its correction and scale binomial bounds by the known mass;
ordinary or weighted-importance zero counts cannot use those bounds unchanged.
Previously resolved goal-independent cases provide a whole-cell lower bound
3.995833147845772e-34, so this subset's contribution is at most0.03537% of the
whole-cell probability even without the sampled1e-25 scale.
No new source, production changes, score simulation, commit or push.

Report: `experiments/rare_positions/2026-10-03-rust-flamengo-three-points.md`.

## R56 — Inspect successful Flamengo12 seasons backwards

Status: completed (2026-10-03), offline observation; production unchanged.

Replay R54's native ten-fold selected-cell confirmation at fixed seeds and
capture the successful seasons without changing proposal construction, random
draws, weighting or acceptance. Aggregate every positive observation using its
corrected importance contribution; retain a bounded list of largest individual
contributors for fixture and goal inspection. Separate pilot, main and check
streams, and distinguish actual sampled goals from goal-independent early
returns. Confirm observer-on/off export equality before the cohort replay.

Older R50 captures suggest that Flamengo losing all ten matches accounts for
more than99.8% of captured contribution across five seeds. Their low effective
sample sizes and missing rival assignments prevent treating that as a complete
description. Inspect rival above/tie/below families, point/win totals, shared
fixtures and goal corrections in the larger independent streams. Measure
capture overhead explicitly; this is an analysis tool, not extra production
budget. No production promotion, commit or push is assumed.

Results:26 captured streams across five fixed seeds reproduce their sampler
hit counts and probabilities exactly. Accepted final main/check streams contain
713,004 draws and25,593 positive observations. Main estimates remain
1.030–1.314e-25. All-loss contribution is99.683–99.999% in the main streams.
Two exact, goal-independent rival families account for77.57–92.17% of main
contribution and81.18–98.26% of checks: Corinthians/Mirassol/Vitória stay below
Flamengo, together with either Vasco or Botafogo. Grêmio/Internacional/Remo/
Chapecoense are the four other below-target teams. Eleven rivals pass Flamengo
on primary points/wins. São Paulo and Bragantino most often finish61/18;
Santos/Coritiba also concentrate near61–62 points. Actual score conditioning
is needed for only0.69–6.72% of final-stream contribution.

This exposes within-root joint rival structure that root reallocation and
neighboring-rank transfer did not solve. Both leading masks already survived
R52 pruning; their survival did not reveal their relative probability mass.
Next hypothesis, not implemented: learn rival-family strata from pilots and
try exact counting or correctly weighted conditional sampling with shared
fixture propagation for the eleven required passing rivals. Preserve support
for other families, target points and goal ties, and use fresh main/check
draws. Do not hard-prune the empirical remainder.

Validation: isolated formatting/check/release build and two-mass/two-draw-count
analyzer self-test pass; independent Sol review passes. Seed808 observer-off,
observer-on and immutable native10 exports are byte-identical. All five complete
observer-on exports byte-match their archived native10 controls. Capture is
expensive: a fresh four-worker serialized pair measures2.935s off versus6.856s
on wall time,5.121s versus9.018s total CPU. Extra CPU is largely system time.
These are offline diagnostic costs, with no production latency allowance or
source/default change. Source, raw streams and commands are preserved with
hashes in the artifact manifest. No commit or push.

Report: `experiments/rare_positions/2026-10-03-rust-hit-patterns.md`.

## R57 — Learn joint rival families and condition shared fixtures

Status: completed (2026-10-03), isolated experiment; production unchanged.

User approved testing the conditioning mechanism described after R56. Learn
target outcome roots and complete rival below/equal/above vectors from corrected
native pilot contributions. Freeze a small deterministic set of families before
final sampling. Do not load R56 final observations, reference estimates or
hardcoded team/rank families into the proposal.

Compile learned families into final point/applicable-win intervals for every
rival. Reuse joint fixture propagation, bounded joint setup and sequential
interval guidance. Keep a primary native proposal component with positive
support and preserve uncached roots. Evaluate the entire mixture density using
replay, including branch-selection probabilities and original prior factors.
Actual goal sampling, sorter and score likelihood correction remain necessary
for equal primary totals. Setup exhaustion declines a proposal and cannot
establish impossibility.

Use current Rust as the baseline. Replace selected-cell confirmation work at
the existing deterministic grant, charging collection, attempted setup,
candidate pilots and replay operations. Allow cheaper proposals more draws
within that grant, with the existing broad draw ceiling. Protect other jobs.
Compare identical full requests/seeds with four workers total and separately
measure wall time and user/system CPU. Preserve the user's ten-fold learning
arm as an equal-grant native versus family comparison; it does not change the
production allowance. Start with seed808, then expand only after correctness
tests and the initial screen pass. No commit, push or promotion is assumed.

Results: one group, 16498, and five fixed seeds: 808/1669/1993/2281/2293.
Current Rust publishes Flamengo/12th in 3/5 runs. The first family version,
learning from 500 native draws at the original grant, publishes 1/5 and loses
two existing estimates. At the selected-cell 10x grant, native/family publish
5/5 versus 4/5; family ESS improves strongly for 808/2281 but worsens for 1669,
and 2293 fails its max-share gate. Seed 1993 has no eligible replacement and
remains unchanged.

Allocation-only follow-up: use 5,000 shared native pilot draws at the original
grant, then 8% family headroom. Family publishes 4/5, versus 3/5 for both current
Rust and the matched 5k native control. Gains: 808 and 2293; loss: 1669. Net one
extra positive cell-run of the same Flamengo/12th identity; distinct cell
identities do not increase. Accepted family probabilities are 7.596e-26–1.452e-25.
Only that cell's
probability changes; all other cell estimates, MC hits/sample counts,
reachability and game importance match current Rust. Seventeen impossible zeros
and zero undecideds per matrix; reachable-zero counts change from 3/4/2/3/3 to
2/5/2/3/2. No exact whole-cell golden probability is available.

All four attempted follow-up family proposals settle within their original
grants. First-version seed 808 exceeds its grant by 3.60% and is disqualified.
The unchanged 1993 native path has an inherited discrepancy between actual-work
and nominal-grant diagnostics; no family attempt occurs. Full report separates
logical operation grants from legacy request work equivalents.

Performance: current/follow-up median full-request process wall 1.407/1.370 s,
CPU 3.373/3.212 s, four workers in every arm. Largest follow-up increase against
current Rust is 14.3 ms/+0.93% wall and +1.01% CPU (2293); against matched 5k native,
39.3 ms/+2.60% wall and +3.58% CPU. Single runs per seed/arm, not warm benchmark
replicates; failed quality checks can appear faster. First-version increases
also exist: +23.55% wall/+13.01% CPU at 1x seed 2293 and +16.29% wall/+11.90% CPU
at 10x seed 2281. No production latency allowance or speedup is inferred.

Validation: first version passes all 93 Rust tests, formatting/check/build and
independent Sol review; allocation follow-up passes six focused family/allocation
tests and formatting/check/build. Closed-form tests cover mixture densities and
an omitted family. Flag-off 808 export matches current Rust; absent allocation
overrides match version 1. Portable source archives, dependency/test fixture,
patches, raw full exports/timings and reproducible harnesses are preserved.
Production source/defaults unchanged; no DB changes, commit or push.

Recommendation: retain as an experiment; do not replace native confirmation.
Pilot learning still misses a leading family for 1669, and a main observation
contributes 75.7% of weight, causing rejection. Next hypothesis: learn bounded
family statistics from an already funded failed native main batch, preserve
accepted native estimates, and spend remaining grant on charged setup plus
fresh family main/check streams. That fallback is not implemented or measured.

Report: `experiments/rare_positions/2026-10-03-rust-family-conditioning.md`.

## R58 — Learn families after failed native confirmation

Status: completed (2026-10-03), isolated experiment; production unchanged.

User approved continuing after R57's 4/5 versus 3/5 coverage result, which
still lost seed 1669. Preserve current Rust's ordinary native confirmation
waves, accepted estimates, skipped-check refunds and second-wave admissions.
Only after those waves finish, retry an originally paired Lazy proposal whose
native main failed publication and whose cell still has no estimate.

Available work is the smaller of that cell's original grant minus native actual
work and the request bank's unallocated capacity after both ordinary waves.
Checks already financing other jobs cannot finance this fallback. Successful
native runs incur no recording work. For the first arm, replay a bounded prefix
of the failed main with its original seed to learn corrected complete rival
families. Replay is duplicated sampling work and is charged explicitly, together
with collection, cloning, attempted setup, validation and density replay.

Reuse R57's full family constraints and support-preserving mixture unchanged.
Freeze the fit before fresh validation and independent main/check streams.
Keep the current publication gates. Budget exhaustion declines the attempt;
neither missing estimates nor failed setup imply impossibility. Use deterministic
operation costs and headroom, never elapsed time, for allocation.

Start with a full-request seed 808 paired screen and flag-off export parity.
Then compare all five existing seeds with four workers and identical requests.
Report gains, losses, remaining zeros, every charged attempt, native/admission
preservation, setup costs and full-request wall/user/system CPU. Include a
separate ten-fold selected-cell learning arm if the first screen is correct;
expanded experiments do not change the production allowance. No promotion,
commit or push is assumed.

Initial R58 screen: flag-off seed 808 full export byte-matches current Rust.
The deferred fallback recovers Flamengo/12th at 3.841e-26, with independent
check 7.935e-26, main/check ESS 16.13/8.47 and 2,224 draws per stream. A replay
of 4,232 original-main draws learns four families. Native plus fallback work is
143.573M against the original 147.120M cell grant; no overrun. Existing native
streams/admissions and probabilities remain unchanged. Full request process
wall is 1.388 to 1.652 s (+19.0%), CPU 3.304 to 3.540 s (+7.1%); single-run
timings, with substantial startup variation in the flag-off control. All 96
tests pass. The result supports recovery, not production latency preservation.

Next measured arm: cap replay before sampling to leave minimum validation,
bounded setup and fresh-pair work; protect pair funding during validation.
Compare all five fixed seeds at the original grant. An overlapping schedule is
under design only: prove spare capacity before using idle time in another native
job's worker slot. No scheduling change is implemented yet.

V3 original-grant result: 5/5 Flamengo/12th runs positive versus current Rust
3/5, with no baseline-positive probability/evidence/design regressions and
identical game importance. Only request-total `work_spent` metadata changes on
other estimates. This adds two positive runs of one distinct cell in one group.
Recovered probabilities: 4.112e-26 (808) and 2.466e-26 (2293), approximately
3.2/4.8 times below R57's corresponding larger native references. Native plus
fallback actual operations remain within the original cell grants. V2 declined
808 on low validation ESS; V3 uses that independent pilot only for cost
calibration and retains every final main/check acceptance gate.

Three alternating warm timing pairs per eligible seed: mean request wall
increases 47.6 ms / 3.60% (808) and 61.7 ms / 4.22% (2293). CPU deltas are noisy.
The result improves coverage but does not preserve current request latency.
No additional production allowance is assumed. Source/patch/binary and raw
outputs are frozen separately for V1/V2/V3.

The next bounded comparison tested fresh native retry versus learned-family
fallback using identical leftover grants after unchanged native waves. Also run
a separate 10x **leftover fallback grant** arm with explicit added allowance,
native retry control, and 5k versus 50k learning caps. Charge all new/replayed
training, attempted setup and finals. Successful native estimates remain
untouched. Expanded experimental grants do not alter production budgets.

Report: `experiments/rare_positions/2026-10-03-rust-family-fallback.md`.

V4 control/learning screen complete: equal-leftover-grant native retries recover
neither 808 nor 2293; family 1x recovers both. Native 10x and family 10x with a 5k
learning cap recover both near 1e-25. Larger family learning (42,329/26,861 draws)
improves 808's main ESS to 71.69 but fails 2293: 2,591 hits, ESS 2.30, max weight share
65.1%; independent check skipped. These are quality declines, not grant
overruns. Do not adopt large learning allocation by default. All 22 main-cohort
runs preserve baseline-positive cells/game importance; 99 tests pass.

Breadth screen: 20 requests, selectors Palmeiras/14th and /15th, five seeds each,
family/native retry 1x controls. Family recovers Palmeiras/15th at 1.346e-33 on 2293;
native retries recover none. Palmeiras/14th remains zero. All baseline-positive
cells and game importance preserved; no overruns. These independently selected
screens cannot be summed into a single request: Flamengo and Palmeiras share spare
bank capacity.

V5 combined-budget screen: generic all-scope, stable native job
priority, after unchanged native waves. Sequentially reserve each originally
paired failed-main Lazy zero's current min(cell remainder, bank free), settle all
charged work and refund unused grant before next. No named-team/rank gates or
extra allowance; all-scope supports 1x only. Stop on first actual overrun and do
not claim a strict actual budget for such a run. Compare family and native-retry
five-seed full matrices, native preservation and wall/CPU; verify combined
gains directly in one export. Selected-scope defaults remain V4 unchanged.

V5 measured result: only the same two Flamengo/12th gains; no additional gain
over selected scope. On 2293, failed Palmeiras/13th learning consumes spare
bank before Palmeiras/15th, which then cannot fund a final pair. The separate
passing Flamengo and Palmeiras schedules cost 148.615M against 132.073M spare,
so they cannot simply be combined. All baseline-positive cells/game importance
are preserved and no fallback overruns occur. Mean paired warm wall increases
322.0/213.2 ms (808/2293); CPU increases 327.4/345.1 ms. Do not adopt this
all-scope allocation. The subsequent main-overrun guard passes all 101 tests;
all 13 cohort exports match the initial revision and charged component sums
are correct. Initial timing logs remain immutable and identified separately.

V6 bounded allocation screen: unchanged native waves and total
bank, all-scope 1x, three five-seed arms. Test ascending full original unused
cell grant (stable original-priority ties), learning cap 3,000, and both.
Sort before clamping grants to scarce bank capacity. Charge every failed
attempt and retain all final gates. Shorter learning can miss important
families; measure combined full matrices rather than summing separate gains.

V6 final result: ordering alone still gains only the two Flamengo/12th runs.
Cap3k raises gains to four cell-runs in each ordering arm, with no baseline
losses or overruns. Native order recovers Palmeiras/15th on 808 and /13th on
2293 (three distinct gained team/ranks including Flamengo/12th). Grant order
recovers Palmeiras/15th in both (two distinct gained team/ranks). Both produce
Flamengo and Palmeiras gains in the same requests and leave reachable zeros
`1,4,2,3,1`, versus baseline `3,4,2,3,3`. Do not combine their different
Palmeiras gains as if they coexist in a single export. Setup sorting is funded
before scanning: 16*(2J + J*ceil(log2(J+1))), 400/480 units here. All 104 tests
and ten focused tests pass; final exports/operation ledgers match the initial
screen. Three warm baseline/native-order/grant-order triplets per eligible
seed show wall increases 359.5/363.6 ms (808), 181.7/195.2 ms (2293), or
12–26%; CPU increases 501.2/565.2 ms and 297.0/283.7 ms. Spare work-bank
capacity is not free CPU or latency. Neither allocation meets the original
latency constraint, so neither is promoted.

Six 10x leftover-grant controls: Palmeiras/13th on 2293 publishes 6.451e-20
(family) and 6.655e-20 (native retry), about 9.3–9.5 times above cap3k's
6.972e-21. All four Palmeiras/15th controls fail quality. On 808, family main
has 13,063 hits but one contribution carries 99.1% of a raw 1.587e-31 mean;
native raw mean is 1.882e-31 with ESS 2.29. On 2293, family main/check
disagree ~24x (5.454e-33/1.299e-31); native raw 1.688e-31 has ESS 1.75.
These are not calibrated references, but expose a tail missed by small accepted
1e-33 batches. Do not equate the four gained nonzero runs with four validated
order-of-magnitude estimates. Every expanded attempt fits its explicit grant.

Next quality experiment (not implemented): audit Palmeiras/15th's largest
weighted seasons at the existing larger streams, retaining root, all rival
points/wins statuses, actual score/tiebreak data and density factors. Determine
whether the tail comes from unlearned roots/families or poor sampling inside a
known family. Derive generic proposal rules from the audit; learn on separate
training streams and use fresh independent confirmation. Do not reuse final
reference observations to train and validate the same reported estimate.

Next performance hypothesis (not implemented): retain a bounded number of
family observations while native confirmation is already running, avoiding the
later replay. Fund recording before knowing whether native confirmation will
fail; recording successful runs is still real work. A later skipped check is
not advance funding. Reserve only spare capacity that can be proved unavailable
to any original second-wave admission, and charge the proof/dispatch work too.
The first feasible case may be a request whose jobs were all originally paired,
so no second-wave admission can need that capacity. Bound both observed draws
and retained positive observations, keep native RNG unchanged, and subtract
recording cost from the same fallback grant. If no spare capacity is guaranteed,
retain replay or explicitly reallocate existing native work. This is a possible
way to recover latency, not a measured free optimization.

## R59 — Train fallback during preserved native confirmation

Status: isolated experiment completed (2026-10-04); not enabled in production.

User request: train the family fallback while preserving successful native
estimates. Start from frozen R58 V6, with production sources unchanged.
Record bounded corrected family contributions during existing native main draws;
fit only originally paired failed-main zero cells. No native batch, random
stream, admission or successful estimate may change. Training observations do
not enter the final reported mean; freeze the fitted proposal before fresh
validation and independent main/check streams.

Fund collection before the outcomes are known. Pay a bounded admission
certificate from audited unused capacity in the completed ordinary rare-tail
stage, including its extensions. Enumerate all possible native first-wave
publication outcomes (at most six jobs initially), simulate the original refund
and second-wave admission rules, and reserve recording only from capacity unused
in every scenario. Charge certificate work even on decline. Hold recording
reservations until the original waves finish; charge collection on native
successes as well as failures. If funding cannot be certified, retain R58's paid
replay. Cap both inspected draws and positive observations, and deduct the
failed cell's recording charge from its remaining original grant.

Measure identical full requests and fixed seeds with four workers. Compare
current Rust, R58 replay, and funded native recording. Audit native sampling and
admission parity, all successful cells, gains/losses, work settlements, and
full-request wall/user/system CPU. The operation bank is not a latency allowance;
report any increase. Default-off export parity and relevant Rust tests are
required. No commit, push or production promotion is authorized by this request.

R59 accounting review: constructor failures currently discard setup counters.
The initial strict arm withholds earlier-stage credit after any such failure;
add modeled node/guide diagnostics for failed Union construction before allowing
credit. Native recorder tests retain exact full native results and separately
bound every native-draw guard and capped positive collection. Recorded ESS is
for retained training observations, not the complete native batch.

Budget audit must distinguish reservations from actual counters. In the existing
1993 baseline, native confirmation actual work is 1,010,962,876 against capacity
1,010,986,423, despite substantially more reservation slack. Added observer work
can therefore exceed a confirmation-stage actual cap while satisfying the native
reservation model. Report each stage and the combined earlier/confirmation
capacity, including every transferred certificate fee; do not claim a universal
actual or CPU cap from preservation of reservation decisions.

R59 paired controls: strict v1 passed 110 tests and two seeds; metered v2 passed
113 tests and all five group-16498 seeds. All native metrics, admissions,
successful estimates, game importance and default-off exports are preserved.
Both arms decline collection and retain the four R58 gained cell-runs. The
constructor/Guide audit adds 54.76M previously omitted modeled setup units in
four seeds and 23.54M in seed 2293, exhausting conservative stage credit.
Next settle completed ordinary/extension final reservations to actual counters
after all their work has finished. This releases 19.42M unused units on seed
2293; other four seeds still have no credit. Also test a longer observation
prefix with the same positive-record cap, to capture late native tail seasons.
Keep all original native sampling and gates, and measure fallback regressions
as well as gains.

R59 v3: 115 tests and five-seed native preservation pass. Settled credit exists
only on 2293. Its protected confirmation residual is 3.45M units, enough for
only one 256-positive recorder, which belongs to a successful native cell.
Short/full windows produce no useful failed-cell training. K100/full window
trains failed cells but loses R58's Palmeiras/13th estimate. Six K80/K64/K32
native/grant-order controls on 2293 all preserve native estimates and ledgers,
but none improves fallback coverage; several lose Flamengo/12th. Reject these
allocations. Next explicit stage-funded arm reserves complete recorder bounds
plus dispatch from settled earlier-stage surplus before native outcomes. Keep
confirmation admissions/grants unchanged; charge success and failure collection
to that earlier stage once, and leave unused advances unspent. No new allowance.

Final R59 V5: stage funding with a 3,000-draw prefix / 256-positive cap retains
all four R58 gains on group 16498 across five seeds (three distinct cells).
Reachable-zero results fall from 15 to 11; no new proof classifications. Only
seed 2293 has audited surplus: five native MAINs record 733 observations,
costing 2,317,112 observer units plus 384 dispatch against a 10,246,960 advance.
Failed Flamengo/12 and Palmeiras/13 reuse their training; the other four seeds
retain paid replay. Longer full-native recording loses Palmeiras/13; short
recording with grant order loses the earlier grant-order Palmeiras/15. Reject
both controls. No additional coverage beyond R58's best arm.

Breadth found an inherited R58 preemption bug: group 16653/2293 immediately
published team95/3 fallback before its later native complete-branch sampler,
skipping a baseline-positive estimate. V5 holds accepted fallback results until
all native stages finish, applies only to remaining exact-zero/non-impossible
cells, and retains the full cost of superseded candidates. This restores the
native 3.884987101554226e-34 and original metadata. Group 16653 seeds 808/2293
and group 16982 seeds 808/2293 gain no cells; 16982 has no zeros or rare-tail work.
Across all three groups/nine request-seed pairs, all baseline-positive estimate
fields except total work_spent are preserved; game importance is exact, with
default-off byte parity. Seven applicable native/funding ledgers pass; the two no-work
cases are explicitly N/A. Final source review and Rust suite pass: 170 tests,
4 MySQL-gated ignored; HTTP tests pass with temporary loopback permission.

Warm four-worker full-request timing (three alternating repeats/arm, separate
warmups): group 16498 mean 1.484s current Rust vs 1.639s V5, +154ms/+10.4% wall,
+3.4% CPU time. V5 averages −2.8% wall/−4.5% CPU versus R58 replay, but its only
funded group 16498 seed 2293 is +0.83% wall/−1.54% CPU versus replay. Do not claim
unfunded-seed decreases come from reuse. Group 16653/2293 gains no cells while
adding 112ms/+9.96% wall versus current Rust; its 141.44M-unit fallback is
superseded. These local timings do not meet the production latency constraint;
keep isolated and do not enable by default. Existing actual/reservation budget
deficits are reported, not treated as extra allowance. Palmeiras/15 magnitudes
remain uncalibrated; nonzero count alone is not accuracy evidence.

Next experiment, not implemented: retain native observations/frozen original
grants and defer fallback fitting/sampling until after the final native stages,
then fit only cells still at zero. This should avoid the superseded 16653 batch
without changing native work. Charge storage/dispatch and measure full-request
timing; also continue the independent Palmeiras/15 heavy-tail audit. Full report,
commands, archives and raw paired evidence:
`experiments/rare_positions/2026-10-03-rust-family-native-training.md` and
`experiments/rare_positions/2026-10-03-family-native-training/`.

## R60 — Run family fallback only after native work finishes

2026-10-04: completed as an isolated experiment, authorized by “do the next optimization.” Preserve
R59's recorded observations, original cell grants and settled confirmation bank
in an owned handoff that borrows the original proposals. Commit unchanged native
results, complete the final native branch stage, then fit and sample fallbacks
only for cells still zero and not impossible. Native successes take precedence
before any fallback construction or sampling. No branch credits or extra budget
are introduced. Charge bounded handoff/filter work and collection even for cells
later skipped. Keep production source unchanged during this experiment.

Primary target: avoid the 141.44M-unit superseded group-16653/2293 team95/3
fallback. Paired controls: current Rust, frozen R59 V5 and R60, four workers,
group 16498's five fixed seeds plus 16653/16982 seeds 808 and 2293. Verify every
native positive, R59 coverage gains, default-off parity, once-only accounting,
full-request wall time and CPU time. Record any latency increase explicitly.

Final R60 V2 defers construction and sampling, with all nine paired requests
preserving complete R59 exports after removing only total work_spent. All four
R59 gains (three distinct cells) survive; default-off exports are byte-identical
to current Rust. No new gains or proof classifications. Group 16653/2293 skips
team95/3 before any fallback work, eliminating 141,440,918 modeled units minus
the 192-unit handoff fee. Seven applicable funding/work ledgers pass; the two
16982 cases have no rare work and are explicitly N/A.

Warm four-worker group 16653/2293: R59 1.308s / 2.975 CPU seconds versus R60
1.158s / 2.750, −11.4% wall / −7.6% CPU, with unchanged estimates. Group 16498
seeds 808/1669/2293, eight repeats across two independent blocks: R59 1.844s /
4.114 CPU seconds versus R60 1.826s / 4.224, −0.9% wall / +2.7% CPU. R60 remains
16.4% slower in wall time and 9.8% higher in CPU than current Rust for that
cohort. All 102 timed and 24 warmup exports reproduce their fixed screen hashes.
No sampler work is eliminated on 16653/808, so its observed speedup cannot be
attributed to the superseded batch. Local timings do not establish Xeon speed.

Full estimator suite passed 170 tests with four MySQL-gated tests ignored.
Sol review found a reporting-only alternate certificate-funding reservation
error; V2 corrects it and a dedicated smoke validates held versus settled
observer work. Preserve initial source/screens and final V2 separately.
Retain late scheduling for the family experiment; do not enable the whole
family arm in production yet. Next target is paid replay/fit/validation on
group 16498 and failed batches on 1669/2281, while preserving measured gains.
No commit, push, production source or DB change. Report and reproducible data:
`experiments/rare_positions/2026-10-04-rust-family-late-fallback.md` and
`experiments/rare_positions/2026-10-04-family-late-fallback/`.

### R60 CPU attribution follow-up

2026-10-04: completed in response to “Where is the extra 10% cpu going?”
Diagnostic process-CPU scopes in an isolated clone; no production edits. Five
arms compare current Rust, frozen R60 on/off, and diagnostic R60 on/off. Group
16498 seeds 808/1669/2293, four workers, five balanced rotations, separate
warmups and serialized processes. All 75 timed and 15 warmup exports match
the earlier screen hashes.

Fresh frozen-binary means: production 4.603 CPU seconds, R60 off 4.795, R60
on 4.993. Overall +390ms/+8.5% CPU and +288ms/+13.3% wall. About half of the
CPU increase is already visible with family disabled (+192ms); enabling it
adds +198ms in the same binary. The native-path cause is not isolated; do not
attribute it solely to code layout. The diagnostic build itself changes the
off-path CPU by -146ms, so its scopes cannot exactly partition the historical
9.8% increase. Pair-to-pair timing variation is substantial.

Direct diagnostic fallback CPU averages 244.8ms: MAIN/CHECK 175.8ms (71.8%),
training replay 41.3ms (16.9%), validation 15.8ms, fit 4.9ms, clone 0.4ms,
other bookkeeping/cleanup 6.7ms. Do not sum inclusive parent scopes with their
children. The failed seed-1669 Palmeiras/14 batch costs 181.7ms with no gain.
Metered setup mostly charges existing computation; source review found no
duplicated constructor. All native estimate results are preserved.

Next hypotheses, not implemented: separate the family/observer path from the
ordinary sampler hot loop and test the flag-off regression; reuse mixture
density calculations with equivalent weighting; reduce paid replay; improve
admission of low-yield fallback batches. Family fitting alone is too small to
recover the CPU increase. Diagnostic release checks/build and two helper tests
pass; harness metadata tests pass; independent Sol review confirms attribution
limits. No commit, push or production integration. Full report and evidence:
`experiments/rare_positions/2026-10-04-rust-family-cpu-attribution.md` and
`experiments/rare_positions/2026-10-04-family-cpu-attribution/`.

### R60 production adoption

2026-10-04: the user authorized adopting the measured late family fallback.
Enabled by default for coverage; `RUST_ODDS_FAMILY_FALLBACK=0` opts out and
overrides the legacy experiment master flag. Fixed all-cell, 1x, stage-funded
policy; bounded native recording and independent final checks. No additional
work allowance or cores. Native stages finish before fallback considers zeros.

Twelve paired requests across all six snapshots: all nine core defaults match
frozen R60 byte for byte, all 12 opt-outs match the old production baseline,
and existing positives/game importance are preserved. Four gained cell-runs
across three distinct cells; zero losses. Team95/3 in group16653/2293 skips
fallback after native publication. Rehashed 70 execution records and verified
native/observer/fallback ledgers after normalizing renamed log fields.

151 Rust tests pass/four MySQL-gated tests ignored; four Python helpers pass.
Warning-free release build, edited-file formatting and separate Sol review pass.
Three-seed/three-repeat local means: wall1.397→1.528s (+9.4%); CPU3.471→3.472s
(approximately unchanged in this sample). Historical frozen-R60 CPU overhead
was larger; no production Xeon measurement or speed guarantee. Rebuild/restart
the Rust service; no Rails/JavaScript build. This adoption supersedes the earlier
experimental-only recommendation. Report and compact evidence:
`experiments/rare_positions/2026-10-04-rust-family-production.md` and
`experiments/rare_positions/2026-10-04-family-production/`.

### R61: target-path overflow — experiment complete, keep default-off

2026-10-04: user requested fixing the remaining default-seed reachable zero,
Palmeiras/14th in group 16498. Baseline is master `430ac57f`, the current
production Rust family fallback; its binary is frozen before source changes.
The certificate permits 186 target paths (0–3 additional points), while both
complete enumeration and the rival partition tree stop at 64 target paths.
The dispatcher currently invokes the tree only for rival-case enumeration
overflow. The native pilot finds six events in 1,000 draws with ESS 1.389;
extra confirmation admits four of five candidates and leaves this cell unfunded.

First investigate safe joint-domain pruning before root admission and grouping
target paths without losing probability support. Compare generic bounded
experiments against identical requests, fixed seeds and four workers. Preserve
existing positive estimates and actual tiebreakers; charge setup and sampling
against deterministic work, record latency/CPU increases explicitly. Do not
substitute witnesses or a truncated union for an event estimate. Source edits
and experiment scripts are delegated to Luna; Sol owns design and review.
No commit, push or production-default change is authorized by this experiment.

Results: all 186 target paths survive joint-domain propagation. Fresh training of
the unfunded native pilot fails confirmation (1,000/2,000 draws); its source
variant was removed. Naively enlarging the complete forest misses the important
all-loss parent and produces estimates seven orders too small, even at 10x draws.
Rank-aware refinement ordering, cached marginal mass and complete parent retention
recover the relevant family. Fresh per-cell sampling namespaces and a 6,000-draw
MAIN/CHECK pair yield Palmeiras/14th `3.207e-25`, CHECK `1.238e-24`, for seed 808.

Final paired six-group default-seed runs gain **one unique cell**, with zero lost
positives, no changed positive metadata except work, unchanged game importance
and unchanged reachability. Group 16498 goes from 18 to 17 zeros, all impossible;
group 16653 retains its 48 impossible zeros; other four groups have no zeros.
Four additional group-16498 seeds preserve coverage but gain nothing: their
audited residual grants cannot fund the new path. Exhaustion never becomes a proof.

Unused measured early draw reservations provide late bank credit; setup is never
refunded and original confirmation capacity remains unchanged. Zero-root setup
skips unused training tables and guide-cache hits skip redundant span scans.
The enabled candidate still costs +9.75% mean full-process wall and +10.48% child
CPU over frozen Rust in five warmed paired rotations (four workers). Its stage
median is 101.7 ms; no current latency allowance is assumed. Keep
`RUST_ODDS_EXPERIMENT_TARGET_OVERFLOW_TREE=1` experimental and default-off.
Final Rust suite 173 passed/4 DB ignored; harness six passed; independent review
resolved RNG reuse and setup-credit bugs. No commit/push.

Next investigations: profile the disabled-candidate overhead; reuse root Domains
and avoid normalized-game allocations on marginal-cache hits; prefund complete
setup/pilot/final work before constructing a doomed tree; evaluate certified
tiny-stratum allocation or complete target-prefix grouping. Detailed assumptions,
commands, hashes, paired results and timing are in
`experiments/rare_positions/2026-10-04-rust-target-overflow.md`.

### R62: early rank-aware target-path selection — complete, keep disabled

2026-10-04: test whether exact target-path prior times a propagated marginal
rank-count heuristic identifies useful roots before expensive construction.
Compare ordering alone and ordering plus a corrected proposal tilt. Update both
fixture participants' points/applicable wins; cache rival PMFs by incident
domains, retain tied-block ranks, and count domain prior once per shared fixture.
Heuristic feasibility or small scores cannot prove reachability/impossibility.
Unselected target paths retain defensive proposal support; actual sorter and
fresh MAIN/CHECK samples determine publication.

Use the current Rust binary (R61 changes preserved, tree default-off) as baseline,
fixed snapshots/seeds and four cores. Spend only residual deterministic work;
also compare replacing R61's late tree with the same audited credit. Preserve
successful native estimates. Start with Palmeiras/14 and Flamengo/12 in 16498,
then reference groups and holdouts. Source implementation delegated to Luna;
Sol owns design/review. No commit/push/default change. Track commands and results
in `experiments/rare_positions/2026-10-04-rust-early-rank.md`.

R62 result: no endpoint gains across six default-seed groups and four 16498
holdouts; no baseline positive losses. Order and bias both fail all five
standalone MAIN/CHECK pairs for Palmeiras/14 and Flamengo/12 at 6,000 draws.
Bias identifies events at the expected scale but retains high weight variance.
At 60,000 draws/stream, Palmeiras/14 seed808 passes (`2.556e-25`, CHECK `1.249e-24`)
at approximately 1.36 seconds of serial sampling, outside the residual allowance.
Ranking costs about 14–16 ms versus 2–4 ms control setup. Keep disabled; the
same-credit complete R61 tree succeeds where ranked LazyJoint fails. Work/model
and support tests passed; final combined suite passes 184 tests with four DB tests ignored.

### R63: smaller positive leaf floors in the complete tree — experiment complete

Reallocate existing tree draws by reducing the per-leaf final minimum from ten
to two, retaining every stratum and the existing defensive variance allocation.
Test alongside smaller bound pilots; no pruning, gate relaxation or additional
bank capacity. This may preserve the successful constrained family while reducing
work on negligible leaves. The screen retained a default of ten. Compare original residual
bank and unchanged audited R61 credit, then fixed-seed holdouts/reference groups
and full-request wall/CPU timings with four cores. Report:
`experiments/rare_positions/2026-10-04-rust-tree-allocation.md`. No commit/push.

R63 result: floor2/pilot5/no-credit recovers Palmeiras/14th in 16498/808,
published `7.542e-25`, independent CHECK `6.149e-25`, using 64,791,890 of the
original 65,847,660 residual units. This is 46.3% less modeled work than R61's
successful default tree. MAIN/CHECK ESS 8.78/6.27; gates unchanged. Floor 2 alone
increases work; floor2/pilot5 with more credited draws instead fails a dominant
MAIN observation. Do not treat this as a general benefit of fewer samples.

Six snapshots and four holdouts preserve all existing positives, game importance
and proof labels; one unique default-seed cell gained, no holdout gains. Five
warmed rotations: tree-stage median 98.19→72.45 ms; lean total mean 1.969 s/3.920 s
child CPU versus frozen baseline 2.103 s/4.097 s. Timing is noisy. Enabling lean in
the same candidate adds 7.1% mean wall/0.4% CPU; the disabled binary difference
is unexplained. Six quiet same-process requests also show similar baseline/lean
latency, but do not establish production preservation. Keep opt-in; no additional
allowance presumed. Full Rust: 184 passed/four DB tests ignored; Python: nine passed.
Formatting, diff check and independent Sol review pass. No commit/push/default change.

### R63 production adoption — complete

2026-10-04: the user authorized enabling the measured lean target-overflow tree.
Enable it by default for coverage, with five pilot draws, two-draw leaf minimum,
three messages, at most 6,000 final draws and no reclaimed early draw credit.
Retain the original residual confirmation bank and four workers. Production
toggle `RUST_ODDS_TARGET_OVERFLOW_TREE=0` opts out and overrides the legacy toggle.
The unsuccessful R62 experiment remains off. Defaults reproduce the explicit R63
lean output exactly across all six references. Opt-out restores the previous
default exactly across six references and four holdouts. Existing positive
metadata, proofs and game importance are preserved; Palmeiras/14th remains the
only gain, reproduced in all five warmed repetitions.

Validation identified underfunded setup at holdout seeds 1669/1993. A generic
admission floor now requires measured per-cell root work plus the configured
node/guide quota before construction, saving 12,681,719 modeled units per case.
Holdout results differ only in work accounting; no observed tree-stage overrun
remains. The floor is an allocation policy, not a rigorous upper bound for
rank-hint/setup work. Incremental constructor budgeting remains future work.

Full Rust suite: 188 passed, four DB tests ignored. HTTP defaults, rollback
precedence, reproducibility and admission boundaries pass. Final frozen binary
SHA256: ae0e68a5d5986436656ca34aa6b6b6a9f54befeaaf96049dfbc0df96ab6bbdf1.
Five warmed pairs, four workers: new default 3.937 s wall/6.020 s CPU versus
same-binary opt-out 3.495 s/5.839 s: +12.7% wall/+3.1% CPU. Timing remains noisy
under local memory pressure; no claim of unchanged production latency or extra
budget allowance. Details and raw evidence are recorded in the R63 report and
`2026-10-04-early-rank/production-*-v2/`. The user subsequently authorized commit
and push after validation. Frozen executables and raw logs remain local artifacts.

### R64: two-result snapshot and remaining-zero diagnosis — complete

The user requested a derived group-16498 snapshot with game 364007 played at
1–2 and game 363999 played at 1–0. Preserve the frozen request's other fields,
ratings and game order. Production baseline is commit e3ce9653, binary SHA256
ae0e68a5d5986436656ca34aa6b6b6a9f54befeaaf96049dfbc0df96ab6bbdf1,
seed 808, four workers and unchanged deterministic work limits.

Initial result: 382 positive cells, 17 impossible zeros, one reachable zero
(Palmeiras/15th). The two score changes alone do not reproduce the attached
production log's three reachable zeros. Debug the derived snapshot separately;
do not infer equality of the entire production request from fixture count.

Preselected diagnostics: compare tree enabled/disabled on the identical new
request; test complete small-root partitioning offline at 6,000 draws per stream
and a tenfold diagnostic batch; then test admitting small-root residual trees
into the existing original confirmation bank, leaving production defaults and
publication gates unchanged. Record setup, pilots, messages, MAIN/CHECK,
full-request wall/CPU, remaining zeros and prior-positive preservation. No
database writes, commit or push are authorized for this experiment.

Results: the original minimum-one ordering fills Palmeiras/15th at seed 808 but
loses existing tree estimates at seeds 1669/1993; reject it. Revised ordering
preserves the original 65–256-root candidates first, then spends only leftover
work on small-root candidates. Across the five derived-fixture seeds and six
original references, it loses no estimates or non-work metadata. At seed 808 it
adds Palmeiras/15th at 2.135e-31 probability, leaving only the 17 proven
impossible zeros; holdout coverage is unchanged. Three warmed repetitions
produce identical exports. Same bank and four workers: observed mean wall
2.557→2.833 s (+10.8%), CPU 5.540→5.622 s (+1.5%). Treat local timing as noisy;
no latency preservation claim or additional allowance.

The tenfold offline batch reveals pilot underallocation: a zero-hit leaf gets
45/60,000 draws despite a material final contribution. It does not fix seed 808;
seed 1669 passes around 3.6e-31. Higher global pilots regress coverage in the
initial ordering. Recommend only the revised small-root rule as an opt-in
follow-up, not automatic production adoption. Default minimum stays 65;
`RUST_ODDS_EXPERIMENT_TARGET_TREE_MIN_ROOTS=1` enables the experiment. Full Rust
suite: 192 passed, four database tests ignored. Nine Python harness tests,
formatting, diff check and independent Sol review passed. Report:
`2026-10-04-rust-two-results.md`.

### R64 rating refresh — complete

The user updated team ratings and requested replacing the fixture's remaining
game powers. Captured the existing Rails request builder output read-only from
`GolAberto_development`, checked IDs/participants for all 101 remaining games,
and refreshed 200 means across 100 games (one game's means already match).
Preserved both requested scores, all other fields and fixture order. Current
fixture SHA256: cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee.
The previous fixture and raw DB builder output are archived locally; original
R64 results apply to its previous hash, not this refreshed revision.

Seed808/four workers: production defaults now give 380 positive cells, 17
impossible zeros and three reachable zeros (Flamengo/13th, Palmeiras/15th and
Palmeiras/16th), with no undecideds. The minimum-one experiment produces an
identical export: all three extra setup grants fall below the admission floor
after the existing search. The earlier coverage gain does not transfer to these
ratings. Production defaults remain unchanged. Targeted fixture test, formatting
and exact DB-power/diff checks pass. No DB writes, commit or push. Updated
diagnosis and reproduction commands are in the R64 report.

### R64 original-fixture comparison — complete

Compare the old 103-game fixture, the 101-game old-rating version, the updated
101-game fixture and a counterfactual 103-game updated-rating fixture with the
same frozen production binary, seed808 and four workers. All three current
reachable zeros were previously positive. Either results or ratings alone
loses Palmeiras/15th; Flamengo/13th and Palmeiras/16th are lost only in their
combination at this seed. Certified limits and the 17 impossible zeros agree.
Current Flamengo/13th lacks complete-tree pilot hits, Palmeiras/16th has many
hits but a dominant observation/low ESS, and Palmeiras/15th fails independent
confirmation. Its native MAIN passes, which excludes it from late family
fallback under the current eligibility rule. No further algorithm changes
made. The four-way probabilities, diagnostics and counterfactual hash are
recorded in the R64 report; future work should test confirmation-failure
eligibility and pilot allocation while preserving existing publication gates.

### R65: failed-confirmation families and robust pilot allocation — complete

The user requested retaining the original group-16498 fixture with no reachable
zeros at seed808 and the updated two-result/current-rating fixture as the
regression pair. Intermediate result/rating combinations remain historical local
diagnostic artifacts, not additional tracked fixtures. Use original SHA256
2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625 and updated
SHA256 cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee.
Frozen production baseline is e3ce9653, binary ae0e68a5d5986436656ca34aa6b6b6a9f54befeaaf96049dfbc0df96ab6bbdf1.

First test default-off family eligibility for failed independent confirmations,
including MAIN that passes but CHECK that fails. Preserve successful pairs,
published estimates, weighting, fresh final streams and publication gates.
Run old family candidates before extra candidates. Initially retain existing
cell/request grant limits to measure actual funding; expanding eligibility does
not create work. Any subsequent reallocation must stay inside the original
request bank and report displaced work, coverage regressions and full-request
wall/CPU explicitly. Current updated request spends approximately 108m units
on a declined family for Palmeiras/14th before its successful 120m-unit tree;
the residual bank is only 11.6m units. This is an allocation opportunity, not
evidence that those failed calculations can be identified in advance.

Then test more defensive final stratum allocation and uncertainty-aware pilot
scores. Zero pilot hits must not be equated to zero variance or impossibility.
All strata retain positive allocation and final MAIN/CHECK remain independent
of fitting/pilots. Candidate allocation heuristics are not confidence bounds.
Research context: Carpentier, Munos and Antos (2015),
[Adaptive Strategy for Stratified Monte Carlo Sampling](https://jmlr.csail.mit.edu/papers/v16/carpentier15a.html),
uses optimistic standard-deviation estimates for allocation; its sub-Gaussian
assumptions are not established for the current importance weights. Carpentier
and Munos (2013),
[Toward Optimal Stratification for Stratified Monte-Carlo Integration](https://proceedings.mlr.press/v28/carpentier13.html),
describes the tension between finer partitions and reliable allocation.

Compare identical requests/fixed seeds on four cores, serialize heavy runs,
test both regression fixtures plus the existing reference groups, and record
new estimates, lost estimates, retained reachable/impossible/undecided zeros,
actual operation accounting and timings. Adopt no tradeoff that loses prior
coverage merely to fill the updated request. No commit, push or production
default change is authorized for these experiments. Report:
`experiments/rare_positions/2026-10-04-rust-confirmation-pilots.md`.

Results: eligibility alone leaves Palmeiras/15th unfunded (2.895m residual
units). Explicit transfers and capped legacy work start family sampling, but
the measured small grants fail unchanged ESS/contribution gates. Audited unused
branch credit funds a 7,330-draw family pair, accepted at 1.741e-32 with CHECK
8.600e-33. Combining it with the pilot prior finds two updated/808 cells but
loses original/808 Palmeiras/14th; reject this configuration. A smaller complete
tree configuration preserves coverage over 15 distinct request/seed pairs and
adds four cell-runs, including two at updated/808, but costs +12.7% wall and
+14.6% CPU on that fixture. Reject automatic adoption under the latency
constraint. Shorter overflow without small-root admission still costs +27.4%
wall in its measured batch.

Recommend only pilot-prior scores plus retaining the all-zero tree-pilot skip.
It recovers Palmeiras/16th at 1.429e-40, changes updated/808 reachable zeros
3→2, and preserves original/808 zero-free coverage, all six reference groups,
and four holdouts per regression fixture. Latest warmed mean wall is −3.9%,
CPU +3.9%, with four workers; timing varies across batches, so no speed claim.
Blanket 25% uniform allocation loses two original-fixture cells; reject it.
All experiments stay default-off, with no commit/push. 74 focused tests,
formatting, whitespace and independent Sol review pass. Full Rust suite:
210 passed, four database tests requiring MYSQL_TEST_URL ignored, zero failed.
HTTP listener tests passed after retrying with localhost socket permission.
Final release binary SHA256 is
eab52f6ff920b5bcaf7e010e056eaf0cf3892f01d84d58347b5b1fd39b3612ea.
Final paired checks reproduce both frozen production exports with experiments
off and the measured recommended-profile exports with its two flags on.

### R66: reduce failed-pair family cost — proposed

The funded family only uses 26 hits from a replayed 3,000-draw MAIN prefix,
although the rejected native MAIN contains 174 hits over 20,549 draws.
Experiment with bounded observational collection of the full native MAIN,
paid from an explicit existing-bank reservation. Preserve the native stream,
weighting and original legacy-family training behavior; use the extra records
only for eligible failed-CHECK recipients. Compare fitting quality and fresh
draw requirements against the current prefix replay. Reusing data as training
still requires independent new validation and final MAIN/CHECK; the native
rejection cannot become a published estimate by itself.

Separately measure a pre-reserved parallel final pair for failed-CHECK families
on at most two of the four workers. This may reduce the sequential tail but
speculates CHECK work when family MAIN fails. Charge that work and measure its
effect on other recipients. Neither idea is implemented. Their target is
the additional Palmeiras/15th coverage with current latency, not a new global
work or time allowance. Flamengo/13th still needs a more effective proposal;
zero-hit tree finals and retries have supplied no event evidence.

### R67: extended-work allocation and proposal diagnosis — complete

The user asks whether the three lost group-16498 cells reflect random sampling
noise or overfitting to the earlier fixture, and requests extended-work runs to
discover the important outcome families. Keep only the original and updated
two-result/current-rating fixtures as this regression pair. Current endpoint
defaults and its production latency allowance remain unchanged; additional
work is explicitly an offline diagnostic.

Existing production seed controls already show instability: original
Palmeiras/15th is positive only at 808 among five measured seeds, and original
Palmeiras/16th fails at two holdouts. Updated Palmeiras/16th succeeds at all four
holdouts despite failing at 808. Updated Flamengo/13th succeeds at 1993, while
the original succeeds at every measured seed. These observations do not
separate final sampling variance from unstable pilot training and allocation.

Use complete, disjoint branch proposals with the existing production sampler,
score weighting, sorter and publication gates, four workers, no omitted
leaves, and separate pilot/final streams. Compare:

- Frozen setup and pilot seed, varying only independent final seeds.
- Tenfold final draws with unchanged pilot work.
- Tenfold pilot draws at the same final work, preserving final seeds.
- Equal stratum allocation versus empirical pilot allocation at the same
  final draw total. This is a diagnostic of missed variability, not a proposed
  blanket production policy.
- A preselected hundredfold final batch when tenfold work remains inadequate.

Record per-stratum corrected contribution, ESS, weight concentration, final
draw allocation, target outcomes, forced fixtures and rival points/wins. An
outcome-only vector does not constitute a complete goal-score witness. Use
independent high-work evidence to identify consequential families; do not
publish training results or treat accepted rough estimates as golden truth.
Report timing and actual work explicitly. Source changes are confined to an
offline example and its runner; no commit, push or default activation.

Results: 46 three-seed panels / 138 final MAIN/CHECK pairs, four workers with
heavy runs serialized. Extended high-work configurations recover all three
known-reachable cells on both fixtures: 36/36 pairs pass, including 18 fresh
validation pairs at 4093, 4099 and 4111. Mean MAIN / CHECK probabilities:
original Flamengo/13th 7.56e-34 / 7.49e-34, Palmeiras/15th 4.86e-31 / 5.18e-31,
Palmeiras/16th 3.01e-40 / 2.92e-40; updated 8.82e-34 / 8.48e-34,
5.43e-31 / 5.42e-31, and 3.01e-40 / 3.43e-40. Units are fractions. These are
provisional rough references, not certified probabilities or bounds.

The failures are not explained by benign final noise alone. Changing only
Flamengo's sparse pilot seed from 808 to 1993 recovers all three unchanged final
seeds/budgets. A 250-draw pilot trains on the one-draw branch and accepts ~1e-39;
a 2,500-draw pilot discovers the dominant all-loss branch and estimates ~9e-34.
Final-only multiplication does not reliably repair the bad training. Both final
streams can miss the same component and pass the observed diagnostics.

Original Palmeiras/15th's four learned families all require Corinthians below
the target's packed points/wins total. Complete-tree branches with Vasco or
Vitória below instead require Corinthians at or above the target; they cannot
overlap those family components. These branches contribute about 78% of measured
MAIN in original/808. Defensive native support remains, but discovery is poor.
The old published ~1e-33 is about 500 times below the larger-work evidence on
the same input. Zero-free coverage alone hid this quality problem.

Palmeiras/16th mainly needs improved allocation or additional final samples:
10× finals with its original frozen pilot recovers all three updated streams.
Blanket equal allocation is not uniformly reliable. Recommending focused
structural discovery and rival-set diversity, not automatic 100× endpoint work.
Offline panels cost 422.30 s wall / 465.40 CPU seconds in total; these exceed
the production allowance and do not establish production latency or a global
matrix regression guarantee. No production/default change, commit or push.
Four new targeted example tests, release build, formatting, runner checks,
independent Sol review and numeric legacy-example parity pass. Report:
`experiments/rare_positions/2026-10-04-rust-extended-allocation.md`.

### R68: structural discovery before family exploitation — current variants rejected

R67 shows systematic overconcentration in learned proposals: accepted nonzero
cells can still miss dominant branches, and independent confirmations can share
the same blind spot. The 36 high-work pairs are provisional rough references,
not certified probabilities or gold truth. Default-off implementation and
paired R68 full-request evaluation are complete. The feature reserves
existing work for zero-hit branches prioritized by certified constraints and
bound/rank hints, before granting message refinement solely from observed hits.
Use partial rival-status families to preserve alternative above/below sets
before filling all slots with detailed ties from one set. Maintain positive
support, corrected weights and independent validation/final streams. Neither
rank hints nor upper bounds become probability estimates.

Compare training/retuning and final work at the current production request
capacity. Score both nonzero coverage and agreement with R67's rough references,
including old Palmeiras/15th and the spurious ~1e-39 Flamengo result. Test all
reference groups and both retained regression fixtures with fixed seeds and
four workers. Measure full-request wall/CPU and actual modeled operations;
accept no implicit extra time allowance.

Implementation/experiment plan: default-off tree discovery priorities using
nominal rival interval masses, independent necessary-rank cardinality, and a
strict-side cardinality heuristic. They can select zero-hit branches for the
existing bounded message refinement before the empty-pilot skip. Scores guide
proposal work only; no constraints, probability weights or impossibility
classification follow from them. Retain the existing message count, pilot
counts, final allocation floors, request capacity and four-worker ceiling.

Separate family controls compare wildcard ties (negative control), skeletons
retaining either the above or below strict-rank side, and bounded release of one flexible
strict constraint. Non-retained statuses become unrestricted so alternative
rival sets are actually covered. Overlapping partial-family densities must sum
every matching component, with replay work and candidate generation charged.
The conservative admission bound must include native plus up to four family
evaluations; defaults keep the old disjoint full-family path and streams.

The corrected v5 evaluation completed 240 requests across both retained
fixtures, three reserved seeds and every other reference fixture, with all
eight arms, matched control and frozen production. All 24 default comparisons
match excluding `work_spent`; no corrected run violates a work grant or changes
reachability. The 171 Rust library tests pass. Every arm loses previously
positive cells on screening and independent seeds; none recovers the three
diagnostic cells. Current variants are rejected for adoption.

Tree discovery retries spend nearly 20 million units without a successful
retune on inspected retained cases: conservative guide-span bounds refuse
construction. Partial-family confirmation reserves cannot fit the current
grants on the affected failed cells. These results do not evaluate the scores'
effectiveness after successful refinement. A tighter, auditable
proposal-specific guide/density cost bound is the next prerequisite before
comparing discovery and diversity again at unchanged capacity. No commit,
push or default activation. Full results, observed timing increases, failed v4
budget cases and reproducible commands:
`experiments/rare_positions/2026-10-05-rust-structural-discovery.md`.


### R69: fresh 100× backward analysis of group 16498 — complete

Reproduced R67's three per-cell configurations on both retained snapshots,
with final seeds 60013/60017/60029 and pilots 808/60031, serialized with four
workers. Twelve panels retain all 36 MAIN/CHECK pairs: 33 pass. Every before
pair passes; updated Palmeiras15 fails one fresh-pilot pair, and updated
Palmeiras16 fails seed 60017 under both pilots (shared final streams). The
larger probability scales persist, but 100× does not guarantee precision.

Backward analysis confirms Flamengo13's dominant all-loss path, Palmeiras15's
alternative Corinthians/Vasco/Vitória below sets, and Palmeiras16's persistent
large-weight concentration. In updated Palmeiras15, the fresh pilot's three
refinement slots select a small team-584 component instead of the important
Corinthians component. A controlled fourth slot on both fixtures retains all
four alternatives and passes all six extra pairs. Updated MAIN minimum ESS
rises 5.67→121.06 and maximum weight share falls 40.88%→5.44%; before changes
little and branch-level heavy tails remain. This is exploratory evidence for
rival-set diversity, not a blanket four-slot recommendation or a proof that
extra refinement fits the production bank.

Six fresh unchanged full-request controls show zeros and substantial nonzero
underestimates on both fixtures. Target points/wins and ten remaining target
fixtures are unchanged; the after snapshot includes two results plus 200
refreshed power fields, preventing separate causal attribution. Probabilities
remain provisional. Next generic implementation needs tighter auditable guide
and mixture-density cost admission, reserved discovery of promising unseen
structural families, rival-set diversity, and separate responses to missing
families versus concentrated weights. Evaluate successful bounded refinement
before full-request adoption; validate already-positive cells as well as zeros.

Measured child-run totals: 265.76 seconds wall / 363.44 CPU seconds for twelve
primary panels, two extra-slot panels and six production controls. Four Rust
example tests and five analyzer tests pass. Production defaults unchanged; no
DB write, commit or push. Full raw commands, hashes, analysis and limitations:
`experiments/rare_positions/2026-10-05-rust-100x-backward.md`.


### R70: generic training, allocation, guidance and score controls — complete

User authorized running experiment ideas. Five policies were screened on both
16498 snapshots: P15 pilot 500→5,000 with three slots; P16 equal allocation,
pilot 5,000→50,000, always-bounds guidance, and native goal scores. Screening
uses pilot 60031, finals 60013/60017/60029 and frozen R69 example, all samples
and four-worker limits unchanged. Ten further holdout panels use fresh
pilot 60109 and finals 60101/60103/60107: three P15 training policies and selected
P16 bounds versus the adaptive baseline on both fixtures. Twenty new panels,
60 MAIN/CHECK pairs, 53 pass; all failed pairs retained.

The larger P15 pilot preserves the important Corinthians/Vasco/Vitória
refinements and passes all six screening pairs. Fresh holdout retains those
families in every policy but develops an extreme weight in the already-refined
Vasco family. Four slots fail one updated holdout, while the baseline and larger
pilot accept high concentrated estimates. The earlier fourth-slot improvement
is insufficient for adoption. The extreme rows share a final seed and are
correlated, not independent recurrences.

P16 always-bounds wins the fixed screening rule (6/6 pairs, minimum ESS 29.22,
worst share 15.05%). It then fails one original holdout with ESS 4.69/share 45.59%,
while the adaptive baseline passes all six holdouts. Equal allocation moves
failure from updated to original; more pilot work retains concentration; native
goals pass only 2/6 and worsen tails. None earns a general production switch.

Distinct families address the documented structural blind spot, but heavy
weights within covered/refined families persist. Training and final seeds
change together in holdout, so effects cannot be causally separated. Work
admission and uncertainty remain material: P15 larger-pilot after seed 60101 total
modeled charge rises 6.257B→9.325B; P16 setup/total charges remain unavailable.
No production capacity or exact-probability claim follows from fixed finals.

New runs cost 331.31 seconds wall / 551.89 CPU seconds, excluding reused controls,
analysis and idle time. Python syntax checks, twenty-panel hash/settings/draw/
probability conservation validation, and independent Sol raw-data review pass.
No new tests, Rust/default edits, DB write, commit or push. Full protocol,
selection made before holdout, raw commands/results, scripts and report:
`experiments/rare_positions/2026-10-05-rust-generic-followups.md`.

### R71: exact guide-shape admission — screen complete, no adoption

Continue the uncommitted experiments with Sol planning/review and Luna code.
Add default-off `RUST_ODDS_EXPERIMENT_TIGHT_GUIDE_BOUND=1` only to already
bounded experimental retuning. Predict both forward and reverse guide table
charges before construction, using supported forward maxima and unconditional
reverse origins, checked arithmetic and a charged estimator traversal. Keep
the legacy/default path, work grant, pilots, finals, scores and gates unchanged.

All 36 fixed screening requests complete: two retained snapshots, seeds
808/1669/1993, default control, frozen R68 v5, coarse rank, and tight
rank/interval/strict. Six default comparisons match excluding `work_spent`.
All 18 tight requests successfully refine one unseen Flamengo/13th branch
within its message grant. Tight rank spends 8.911M / 8.177M units on
original/updated instead of nearly 20M failed coarse attempts. The admission
barrier is removed, but no diagnostic zero recovers. Each tight arm gains one
cell-run and loses three against defaults; all tight exports match excluding
`work_spent`. Original/808 Palmeiras15 remains about 486 times below R67's
provisional rough mean. Updated Flamengo re-pilots or finals still lack useful
evidence or concentrate extreme weights. Construction alone is insufficient.

The stricter analyzer uncovers `branch_draw_audit_valid=false` in 30 archived
R68 screen records and 18 new screen records, including default controls.
Aggregate reserved-minus-already-released totals and explicit stage settlement
checks do not fail; individual draw-bound validity is still unproven. The older
analyzer omitted this field. No new tight guide-grant failure is observed.
The new analyzer deliberately returns status 2 with a complete preserved
report. Follow the prewritten stop rule: do not launch fresh-seed or reference
validation, increase capacity or adopt the control. Next expose and safely
bound the reservation that invalidates the draw audit, and improve discovery
of useful families before claiming request affordability.

New child runs total 58.590 s wall / 124.863 CPU seconds. Mean tight-rank wall
is +1.66% and CPU +1.27% against candidate defaults in this single screening
batch, not a latency guarantee. Thirteen focused message tests, seven Python
tests, check/format/whitespace verification pass. Optimized full Rust suite:
240 passed, four MySQL-dependent tests ignored, zero failures; all HTTP tests
pass with localhost access and optimized serial execution. Debug HTTP attempts
hit sandbox binding restrictions, then two existing 10-second read timeouts.
No production/default change, DB write, commit or push. Frozen source/binary,
protocol, raw records, failed audits and full report:
`experiments/rare_positions/2026-10-05-rust-tight-guide-bound.md`.

### October 5 results integration

The experiment results and reproducibility tools are integrated on `master`
against production baseline `e3ce9653`. Experimental controls remain opt-in;
no new policy, request capacity, acceptance gate or production default is
adopted. R65's narrow pilot-prior candidate needs renewed validation after the
later draw-audit findings. R67–R70 larger-work estimates remain provisional
diagnostics, and R68/R71 discovery variants are rejected for adoption.

A later R71 batch already present in the working tree adds 36 fresh-seed
requests. Each tight arm gains one and loses two cell-runs; 13 records have
audit or settlement failures, including aggregate draw-reservation overruns.
It does not override the original screen's continuation stop. Twelve default
comparisons across both batches match frozen R68 defaults excluding
`work_spent`. The next prerequisite remains safe per-draw admission and useful
family discovery. See `experiments/rare_positions/2026-10-05-rust-experiment-merge.md`
for integration scope, evidence handling and validation.
