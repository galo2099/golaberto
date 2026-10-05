# R62: rank target paths before proposal construction

2026-10-04. Experiment complete; early ranking is not recommended for adoption.
Follow-up to the target-path overflow experiment R61.
The current Rust baseline includes the uncommitted R61 changes, with its new
tree disabled. Preserve those changes and the native estimates. No commit,
push, database mutation or additional time allowance.

## Hypothesis and safeguards

The existing sampler tilts target point totals. Paths to the same target total
can award different points and wins to opponents. Before expensive joint root
construction, enumerate a bounded complete set of target paths, update both
participants, propagate shared fixture domains, and score each path using its
exact prior multiplied by a marginal rank-count heuristic. Test ordering alone
and ordering plus an importance tilt.

The marginal rank score is an allocation heuristic, never an estimate or proof.
Shared-fixture domain prior is counted once per fixture. Ties remain possible;
the heuristic includes ranks within the tied block. Production sorting and goal
sampling verify actual finishes. A defensive target-conditioned component keeps
support for paths outside the shortlist. Fresh independent MAIN/CHECK streams
and existing publication gates are required.

Run the new stage after all existing native/family work, using the original
residual confirmation bank. A separate arm replaces R61's complete late tree
and uses the same audited unused draw credit. Charge setup and draws; elapsed
time remains diagnostic. Exhausted work leaves a cell unresolved. Preserve all
existing positive estimates.

## Baseline

HEAD: `430ac57fd15dc4d451be7405bcce1e5d9febafc1` plus preserved R61 source.
Frozen binary:
`2026-10-04-early-rank/data/golaberto-odds-r61-baseline`.
SHA256: `09da95531b96998a5de5da7659729b5bfbe31b36648e6eac95abdf2a71f1dbf8`.
Use identical snapshot JSON, fixed seeds and four workers; serialize heavy
builds and experiments. Start with group 16498, Palmeiras/14th and Flamengo/12th,
then the six reference snapshots and seed holdouts.

## Progress

- Sol reviewed current total-level scoring, target-path overflow and residual
  bank accounting. Both diagnostic cells admit 186 positive target paths.
- Luna workers own the scorer, late-stage integration, diagnostic example and
  reproducible paired harness. The experiment remains disabled by default.
- Focused Rust tests pass. Independent Sol review found no support/density
  defect and identified setup-accounting gaps; Luna corrected those gaps.

## R62 results

Frozen candidate SHA256:
`a5aae435a90a34f8ad2a605674e6849884453d2d54f70ac51b2a4198723984ed`.
`data/golaberto-odds-early-rank-v1` retains the tested binary. Timing is diagnostic;
the initial sequential screen was cold and is unsuitable for speedup claims.

### Standalone matched-seed proposals

Each seed uses a 500-draw pilot and independent 6,000-draw MAIN/CHECK streams.
The comparison is the existing 32-root LazyJoint proposal, not an entire request.
Seeds: 808, 1669, 1993, 2281, 2293. Construction receives an offline 100-million-unit
cap; final draws are not constrained to the endpoint residual bank in this probe.

| Cell | Existing 32-root accepted pairs | Ranked order | Ranked bias |
| --- | ---: | ---: | ---: |
| Palmeiras/14 | 0/5 | 0/5 | 0/5 |
| Flamengo/12 | 0/5 | 0/5 | 0/5 |

The existing production family fallback already estimates Flamengo/12 for seed
808. These standalone failures do not replace that successful estimate.

Ranking constructs in about 14–16 ms after warmup, versus approximately 2–4 ms
for the sampled-root control. Modeled setup is 18,701,863 units for Palmeiras and
18,660,346 for Flamengo. Both enumerate 186 paths. Palmeiras records 3,515 PMF
cache hits and 19 misses. All-loss is correctly ranked first: exact target prior
`2.0019063431519247e-8`, rank hint `1.5704324343440124e-9`, combined score
`3.143858651804794e-17`. This score is **not** a rank probability or upper bound.
Its independent rival/tie relaxation is much looser than the confirmed event.

Ordering alone also removes the existing target-total tilt in the experimental
cached mixture (`tilt=1`), so it does not isolate just a sorting instruction.
It gets only 0–1 MAIN hits per seed. Bias restores concentration: Palmeiras MAIN
has 19–27 hits, but ESS only 2.17–5.26; dominant contributions still fail a gate.
Selecting the right target path does not identify the requisite joint rival and
score families.

At **10x final draws**, seed-808 ranked bias passes: MAIN `2.555799276876936e-25`,
CHECK `1.2489648281278725e-24`, ESS 12.76/4.50. The existing sampled-root control
still fails its MAIN gate. This diagnostic uses 60,000 draws per stream and
approximately 1.36 seconds of serial sampling, well beyond the current residual
allowance. It is evidence of variance reduction, not an endpoint-ready solution
or a precise probability interval.

### Full requests and unchanged budgets

The pure arms use the 65,847,660-unit residual bank. The replacement arms use
the same R61 audited late draw credit (153,514,196 units), giving 219,361,856
free units without raising the original confirmation capacity.

| 16498/808 arm | Added cells | Actual additional work | Stage wall ms | Final draws/stream |
| --- | ---: | ---: | ---: | ---: |
| Ranked order | 0 | 43,633,879 | 60.1 | 1,960 |
| Ranked bias | 0 | 43,339,784 | 61.7 | 1,583 |
| Order replacing R61 tree, same credit | 0 | 84,060,804 | 99.2 | 6,000 |
| Bias replacing R61 tree, same credit | 0 | 95,444,747 | 110.5 | 6,000 |
| Existing R61 complete tree | 1 | 120,572,748 | see paired evidence | 6,000 |

Failed MAIN gates skip CHECK. No observed operation overruns occurred in these
seed-808 early-rank arms. The complete tree again publishes Palmeiras/14
`3.207392222892081e-25`; its independently measured cost remains material.
Replacing this successful tree with ranked LazyJoint would lose this gain.

Six reference groups at seed 808 (15902, 16413, 16498, 16653, 16982, 16983), plus
four 16498 seed holdouts, find **zero** early-rank gains and **zero** lost baseline
positives. Existing positive metadata (apart from total work), game importance
and reachability classifications agree. R61 alone gains only the default-seed
Palmeiras cell. Do not interpret repeat runs as additional unique cells.

## Commands and evidence

```sh
cargo build --offline --locked --release -j4 \
  --manifest-path odds-rust/Cargo.toml --bin golaberto-odds --example early_rank_roots
odds-rust/target/release/examples/early_rank_roots \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  16 14 bias 32 6000 808,1669,1993,2281,2293 100000000
python3 experiments/rare_positions/2026-10-04-target-overflow/scripts/run_overflow.py \
  --baseline experiments/rare_positions/2026-10-04-early-rank/data/golaberto-odds-r61-baseline \
  --baseline-sha256 09da95531b96998a5de5da7659729b5bfbe31b36648e6eac95abdf2a71f1dbf8 \
  --candidate experiments/rare_positions/2026-10-04-early-rank/data/golaberto-odds-early-rank-v1 \
  --output /tmp/early-rank-reproduction --groups 16498 --seeds 808 \
  --arm bias:RUST_ODDS_EXPERIMENT_EARLY_RANK=bias \
  --arm tree:RUST_ODDS_EXPERIMENT_TARGET_OVERFLOW_TREE=1 \
  --arm replacement:RUST_ODDS_EXPERIMENT_TARGET_OVERFLOW_TREE=1,RUST_ODDS_EXPERIMENT_EARLY_RANK=bias
```

Change team/rank to `17 12` for Flamengo; use 60,000 draws for the offline 10x
diagnostic. Raw evidence is under `2026-10-04-early-rank/probes/`, `screen/`,
`reference/` and `holdouts/`. Harness summaries include hashes, exact commands,
four-worker count, child CPU time and export comparisons. Heavy runs were serial.

Implementation sources: `odds-rust/src/joint_caps/propagated/lazy/early_rank.rs`,
the existing mixture weighting in `lazy.rs`, shared `domains.rs`, production
`sort.rs`, and late admission in `rare_tail/early_roots.rs`. R61's report records
the earlier complete-tree experiments and source context. Setup counts and pilot
costs are deterministic accounting proxies; actual sample overruns reject
publication and stop late work. They do not certify elapsed-time bounds.

## Recommendation

Keep early ranking disabled. It has no measured endpoint gain and cannot
replace the stronger complete-tree refinement. A further experiment reallocates
the complete tree's uniform leaf draw minimum; see
`2026-10-04-rust-tree-allocation.md`. The combined final Rust suite passes 184
tests (four database-gated tests skipped); Python helpers pass nine tests,
edited-file formatting and diff checks pass. No commit or push.
