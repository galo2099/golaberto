# Future experiments: rare-position search efficiency

Status: active measured campaign; selected configuration remains opt-in and local.
Written October 1, 2026. The original proposals below are preserved; the active
50% campaign later in this file supersedes their earlier time constraint.

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
| R5 | Fixed-level splitting / sequential Monte Carlo for a sequence of necessary rank constraints | Researched; deferred | Reliability, queue-overflow and SAT literature; discrete thresholds need explicit correction; independent root replications for uncertainty |
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
