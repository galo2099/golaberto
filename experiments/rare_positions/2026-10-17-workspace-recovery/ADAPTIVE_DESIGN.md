# Adaptive estimator recovery evidence and reconstruction contract

Prepared 2026-10-17 by the read-only design/recovery reviewer. This is prose documentation, not recovered estimator source. Repository inspected: `/workspace/golaberto`, HEAD `f9a8658580b923ffaa13ee22837c5f2faf949cc4`.

## Evidence status

The previously inspected worktrees `/workspace/golaberto-adaptive-controller`, `/workspace/golaberto-adaptive-controller-budget-corrected`, `/workspace/golaberto-adaptive-sampling-20x`, and their complete October 14 artifact directories are absent from the present workspace. No exact `target_points_partition.rs`, `adaptive_controller.rs`, joint-points conditioning source, or later frontier-regimes source was found in the current checkout. The design below combines explicitly identified surviving source with earlier first-hand review retained in this chat. It must not be represented as byte-identical restoration of the latest estimator. In particular, the October 14 controller is not evidence of all changes made in the October 16 frontier/separator estimator.

Exact surviving later material:

- `experiments/rare_positions/2026-10-16-frontier-joint-growth/trusted-highs/gd-set4-split/experiment.py` and its `scripts/analyze.py`, `scripts/run_paired.py`, and `test_partition.py`.
- The experiment pins a missing native binary SHA256 `eb013be81c343762952f6a18abe7f45dda9f37a17dd33064877206c0c4fc60ab`, prescreen SHA256 `4f609a32582dd2ae24d46d74eb4384b6ee21eb2554c5f013d2ff58298fbe87df`, and GD audit SHA256 `6554a84c4b9eed28116d51f2360b2969fcd11842329cc4e4afbdc81e8c2b75a7`. These hashes identify dependencies; they do not supply their contents.
- Existing production tree machinery remains in `odds-rust/src/joint_caps/propagated/lazy/strata/tree.rs`, including guidance modules. Its comments explicitly distinguish allocation heuristics from probability bounds. It is a possible implementation dependency, not the missing frontier-regimes implementation.

## Exact fixture identity and rank indexing

Surviving group fixture: `odds-rust/tests/fixtures/palmeiras-shortlist/group-16498-uploaded-285played.json`, SHA256 `b4613707f5b2e081040e23f74cb73215b91bf90440ed2faa845ec84c0c285aa0`. It is a saved group object with 380 games, not necessarily a native odds request. `shortlist_guidance.rs` pins this same hash and identifies Palmeiras as team ID **16**, base 57 points / 16 wins, in a 20-team group. The existing shortlist itself is specific to its original target and must not silently guide a different target.

`odds-rust/src/search.rs` and `rare_tail/rank_rescue.rs` use `Cell { team, rank: 12 }` for Palmeiras **thirteenth**. Thus user-facing Palmeiras 12th normally maps to native zero-based rank **11**. Validate each CLI/request wrapper explicitly; a JSON witness may use a different convention. `odds-rust/tests/fixtures/group-16498-2026-10-10-palmeiras-13-witness.json` has SHA256 `4b15d91eb925297d4b8b9b5593f3781efd2f91d4fd475987168a18ffff390cdb`; it is a witness object, not an input request. The earlier October 14 reviewed target was team ID 11 / native rank 11 and must not be relabeled Palmeiras on this fixture. No exact historical Palmeiras-12 probability result is available from this review.

## October 14 controller: directly reviewed historical contract

The missing parent had root target-outcome paths, propagated fixture masks, and cohorts defined by root path plus rival set. Weak candidate events could overlap; accepted actual-above rival sets were checked exactly and were disjoint. Proposal prior masses were not summed as a partition. Unprepared, unsupported, deadline and truncated paths retained conservative uncovered upper mass. Integer refutation alone could zero an event.

The parent exposed `SamplingPolicy`, `LowerBoundPolicy`, `ScoreIntegrationPolicy`, `Cohort`, `RootPath`, `Moment`, `sample_stage`, `merge_stage`, `empirical_bernstein`, `expanded_plan_bounds`, `JointPointsPlan::order_opponents`, and `JointPointsPlan::build_exact_ordered`. The controller hooked in after setup completion, receiving fresh model/roots/cohorts, candidate count, uncovered mass, seed, workers and the original request clock. No saved sample data or uncharged plan reconstruction contributed to a fresh request.

Sampling began with a broad 1,000-draw look for live cohorts. Later cumulative targets per unchanged proposal epoch were `[1000,5000,20000,40000,80000,160000,320000,640000,1280000]`. Rerank by absolute interval width after completed sampling or conditioning, with deterministic ties. Admission used 1.25 times the largest observed raw milliseconds per draw and skipped a next stage that would not fit; a skipped stage did not erase its previous CI. Partial stages discarded all their moments/counts and preserved completed CIs. A phase-interrupted stage could not retry the same partial seed.

Deeper conditioning was limited to one executed attempt per cohort, among the four currently widest, after at least 5,000 epoch draws and when the empirical-Bernstein **range term**, rather than its variance term, contributed at least half the current width. Build exactly k+1 ordered rivals; commit only a valid proposal with strictly smaller weight upper bound. Keep old proposal live until candidate validation. Caps: 8,000,000 retained states, 2,000,000 per candidate, 10,000,000 peak. Candidate allowance is bounded by `min(2M,8M-(retained-old_states),10M-retained)`. Peak reporting must use conservative `retained_before + candidate_state_limit`, not old after-states substituted for failed candidate states.

On commit increment proposal epoch, reset all normalized moments and CP threshold counts, derive fresh seeds including cohort/epoch/look, and retain the **absolute** old CI through intersection with new deterministic and sampling bounds. Never pool normalized moments across changed proposals. Failed attempts preserve old plan and CI. If deterministic impossibility conflicts with a positive retained lower, retain evidence, mark a sticky interval conflict and disable qualification; do not silently overwrite the lower with zero.

The corrected timing proposal reserved 10 seconds for response/cleanup inside a 180-second external request: work ends at 170 seconds; final draw-only window is 110–170 seconds; conditioning soft deadlines stop at 110 seconds. Each solver deadline is at most 3 seconds, explicitly soft because DP cleanup is not interruptible. Actual conditioning charge includes cleanup, with a 20-second pool and 1.5-second cleanup headroom before admitting further attempts. Release the final old-plan Arc **before** finishing the attempt timing after a replacement. External process wall time, including response output and final destruction, is the acceptance gate. The corrected run outcome was not available in this review's retained context.

## Confidence and score integration

Historical controller family denominator was frozen before pruning: `A = 0.01 / (6840 * max(candidate_slots,1))`. For proposal epoch e and fixed cumulative look l (both zero-based), allocate downward-rounded `A / ((e+1)(e+2)(l+1)(l+2))`. The two telescoping sums are each one, supporting adaptive scheduling and replacements. Spend 7/8 on EB, split between lower/upper score streams when needed, and 1/8 across 25 one-sided CP threshold bounds at dyadic thresholds `2^-24,...,1`. The old helper assuming four looks could not be reused unchanged.

Validity relies on proposal selection conditional on preceding data and fresh future sampling. Within an epoch each completed n is predeclared; do not turn outcome-dependent partial batches into completed looks. EB/CP estimates operate on normalized bounded observations. Lower scores and upper scores must represent valid conditional bounds on the same event, with their own sums and squared sums. Exact residual/private-fixture GD integration was supported under policies including native, GD-prefix and GD-residual modes; the exact missing integration algorithm is **not** reconstructed here. Keep the surviving native score law as ground truth until an independently checked integrator is implemented. Native W/D/L and full-score laws must not be replaced with heuristic score completion.

Apply epoch-specific lower/upper weight scales to normalized endpoints, intersect with certified event priors, and outward-round absolute arithmetic and global sums. Historical EB numerical protection included gamma-based accumulation/variance padding and directed endpoint steps, not merely a fixed ULP tolerance. Independent high-precision EB and direct binomial-tail audits are required; physical absolute tolerances such as 1e-12 are meaningless for probabilities near 1e-27. Global lower sums disjoint accepted events; upper includes uncovered/unvisited mass and a certified root mass cap. Stop only if `L>0`, `L<=U`, `U<=4L` and no sticky conflict, immediately after every completed update. This interval yields a factor-two estimate via its geometric midpoint.

## Later frontier/separator evidence and safe reconstruction boundary

The surviving October 16 analyzer expects schema `target_points_frontier_regimes_v1`, optional outer `regimes`, canonical cohorts, and per-cohort `nodes`. Node receipts expose `node_id`, `parent_node_id`, `depth`, `proposal_kind`, `status`, `lower`, `upper`, `completed_draws`, `prepare_receipt`, and `batch_receipts`. Preparation fields include `plan_states`, `proposal_weight_cap`, `alpha_node`, `setup_ms`, `selected_rival_ids`, and `joint_status`. Arm fields include `global_interval`, `first_qualification_ms`, growth attempts/acceptances/setup, and retained state final/peak counts. These exact surviving names constrain compatibility; they do not establish the missing allocator or node-alpha formula.

The surviving partition experiment loads a missing `target-16-rank-15-separator-saved.json`; it is a Palmeiras **16th** experiment (rank 15), not Palmeiras 12th. Its fixed input expects 108 cohorts, 486 root paths, 2,425 pre-pruning candidate slots, and original cohort 4 under root 0 containing team 79. At target pack 57 points / 16 wins, team 79 has ten remaining incident fixtures. Enumerating allowed masks produces exactly eleven points/wins-valid patterns: one 10W and ten 9W1D. Replacing the parent by these eleven disjoint children gives 118 cohorts and changes root-0 prepared count from 16 to 26. Every other cohort stays JSON-equivalent. Old parent Monte Carlo moments and CI do **not** transfer to children; children retain only certified deterministic upper diagnostics. Native outcomes use home perspective: 0 away win, 1 draw, 2 home win. All assertions are fixture-specific and must be rederived for Palmeiras 12th.

The prescreen/partition marks results `conditional_on_highs_correct: true`. Preserve that conditional status; a solver's unsupported/unknown result is not a certified zero. The exact separator selection, frontier early-sampling policy, growth trigger and node confidence spending are unavailable. A reconstruction must label its own algorithm and prospectively specify them rather than invent historical behavior.

A safe proposed separator interface is a complete disjoint partition of a parent event, with explicit mask subset/coverage evidence, stable parent/child IDs, and conservative residual mass for omitted or unfinished children. Adaptive child creation needs a summable node allocation in addition to epoch/look spending; the controller's cohort formula alone does not automatically cover an unbounded new node family. Parent bounds may safely intersect the sum of simultaneously valid child bounds for an exact partition. Parent normalized samples must not be distributed into children by heuristics. These are reconstruction requirements, not recovered source.

## Dependencies before a rebuilt Palmeiras-12 run

1. Freeze the chosen surviving input bytes, explicit group-to-native conversion, team ID 16 and native target rank 11; verify all fixture IDs/outcome conventions and unchanged native scoring/RNG.
2. Select whether to rebuild the directly reviewed controller or a newly specified frontier/separator estimator. Document the latter's node family, alpha budget and partition proof before confirmation draws.
3. Have the implementation owner restore missing model, propagation, plan-builder and score integration interfaces or provide tested equivalents. This note supplies contracts, not their implementation.
4. Verify support preservation, exact tiny-model mass identities, disjoint coverage, deadline/failure atomicity, epoch resets, numeric direction, whole-cell stop and memory headroom. Freeze source/binary/request/plan hashes and use fresh samples only.
5. Report measured external wall time and explicitly distinguish valid confidence bounds, timing validity and factor-two qualification. A numerical gain alone is not a completed fixed-budget result.

Historical diagnostic only: October 14 seed-1300 controller used 923,000 completed draws over 73 stages and committed one k7→k8 refinement. Its bounds were approximately `[7.19994e-29,7.51063e-28]`, U/L≈10.43, versus control `[3.85252e-29,2.68728e-27]`. The controller took 185.696 seconds externally and failed its declared 180-second budget and conditioning limits. Those samples cannot seed a rebuilt run or substantiate any Palmeiras-12 result on the surviving fixture.

## Later input identification correction

For the historical Palmeiras16th experiment, use the279-played request
`experiments/rare_positions/reference/2026-10-04-two-results/inputs/group-16498-two-results.json`
with canonical SHA256 `40b8ab70b9fc464d154e5123d527a0b62f31d4a47b8e71a464c8b51c1dfe4544`.
The285-played shortlist fixture described above has the same Palmeiras base
points/wins but a different team79 state and is a separate case. The corrected
measurement is documented in `../2026-10-17-rebuilt-frontier/PAL16_RESULTS.md`.
