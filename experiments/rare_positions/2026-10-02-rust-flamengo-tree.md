# Flamengo / 13th: bounded complete constraint tree — 2026-10-02

## Result and decision

Tested Flamengo/team17 finishing13th in group16498 against the current Rust
baseline. A generic bounded tree can construct a complete proposal within the
existing setup caps, and obtains accepted offline estimates in **3/5 seeds**,
with main probabilities **4.45e-34–6.18e-34**. All numbers here are probabilities;
multiply by100 for percentages. These are rough estimates, not golden truth.

**Do not adopt either full-request allocation arm.** The smaller-pilot arm
was tested in35 pairs across seven snapshots. It produced **no accepted
Flamengo13 estimate**, gained one Palmeiras16 cell-run and lost three existing
cell-runs. The other six snapshots had no coverage change. Reachability and
impossibility classifications were preserved throughout.

The smaller-pilot arm's group16498 median full HTTP wall fell8.52%, from
1,391.27 to1,272.70 ms, because confirmation work was removed. Its worst paired
full-request increase was **11.99%**, and several other snapshots had higher
median latency/CPU. The original500-pilot arm increased group16498 median
latency4.22%, with a34.83% worst pair. Neither result meets the existing
coverage/performance requirement. No additional latency allowance is assumed.

Production Rust sources and defaults remain unchanged. The prototype is an
archived patch applied to an isolated source copy; no commit, push, deployment
or DB mutation was performed. The only active tool correction recognizes the
public HTTP `reachable` label in the offline comparison harness.

## Baseline and case

- Baseline: master `f0a332b8`; its production Rust sources match the released
  `31bc8b0a`. The later commit contains offline tools and evidence only.
- Binary SHA256:
  `f8db89014089e9db7f06a1f9ba15f4d48e4fc1a68c28127c42b6c64a4c75c1cb`.
- Identical saved request:
  `reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json`.
  SHA256 `2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625`.
- Four estimator workers total; serialized, alternating Rust/Rust HTTP pairs.
  Local arm64 macOS, Rust1.93.1. Production Xeon timing was not measured; the
  experiment introduces no CPU-specific instructions.
- Fixed seed roster:808,1669,1993,2281,2293. The full smaller-pilot cohort also
  includes both group16653 snapshots and groups16982,16983,15902,16413.

Flamengo starts on60 points/18 wins with10 remaining fixtures. The existing
certificate limits it to at most61 points/18 wins for13th. Consequently there
are11 target paths: all losses, or one draw/nine losses. Their total prior mass
is2.2772722599753912e-7, a necessary-event **upper bound**, not the rank estimate.
See [the preceding case analysis](2026-10-02-rust-flamengo-13.md).

On each path, packed points/wins propagation forces four teams below and leaves
15 rivals ambiguous. At most three additional rivals can be strictly below.
The released constructor tries576 masks per path, exceeding its256-case cap.
It skips the incomplete union, taking roughly1.2 ms in these paired logs.
The earlier enlarged offline constructor retained407 complete strata but used
a100-million guide cap. This experiment replaces that full enumeration.

## Complete tree construction

The prototype has no team- or position-specific conditions. It accepts a cell
and phase rules, and performs these steps:

1. Apply the existing certified target limit and enumerate every supported
   target path, retaining the64-path cap. More paths cause an incomplete-setup
   error; a partial union is never returned.
2. Construct a complete parent region for each non-refuted path.
3. Choose a promising parent and split an unresolved rival into disjoint
   strict-exception/non-exception regions. A non-exception includes a packed
   tie; goals and later keys are still checked by the production sorter.
4. Propagate shared fixture domains in each child. Once the allowed strict
   exception count is reached, all remaining ambiguous rivals must be
   non-exceptions. This removes irrelevant descendants cheaply.
5. Replace a parent only when both children finish construction or are refuted
   by necessary constraints. A numerical zero or exhausted setup quota retains
   the whole parent. Unsplit parents cover all their remaining descendants.
6. Freeze the partition. Fresh rank/interval pilots choose each leaf's guide
   and variance allocation; independent main/check batches estimate the sum
   over the retained disjoint regions.

The primary priority is parent mass times the existing residual hint. Rival
selection prefers the smallest approximate marginal chance of the required
non-exception side under allowed fixture outcomes. A second priority arm used
an independent-marginal rank-cardinality hint. Both are ordering heuristics,
not estimates, probability bounds or infeasibility tests.

Caps remain **100,000 joint setup nodes and4,000,000 guide values**. A64-leaf
cap completes54 leaves, using43 successful splits,43 failed splits,776 nodes
and3,918,630 charged guide values. Failed splits retain their parents. No
cell-level reachability or impossibility result is added by this tree.

## Correct weighting and validation

For a frozen disjoint partition, the estimate is the sum of each leaf's mean
`indicator(actual rank) * original_probability / proposal_probability`.
Every leaf receives a positive final allocation. Existing target, shared
fixture, joint-normalizer, guidance and conditional goal weights remain in
use; no supported event region is omitted.

This relies on the usual importance-sampling support and likelihood-ratio
conditions. Uneven weights can make many hits ineffective, and estimated ESS
or variance cannot prove that every important region was sampled.
[Owen, *Monte Carlo theory, methods and examples*, chapter9, §§9.1/9.3](https://artowen.su.domains/mc/Ch-var-is.pdf).

Correctness assumptions and limits:

- Packed points/wins screens are necessary conditions using the applicable
  phase rules. Relaxation feasibility is not a reachability proof.
- Exact final ranks are checked with the production sorter and sampled scores.
  No arbitrary score cap is used to establish cell impossibility. Score
  sampling retains the existing model's numerical support.
- An incompatible child may be removed only by necessary domain/cardinality
  propagation. A failed numerical construction is not an impossibility proof.
- Cardinality only bounds the number of strict exceptions; packed ties are
  retained for score-based sorting. Intermediate ranks are not inferred.
- Search pilots and optional tree-training draws are excluded from reported
  main/check estimates. Final batches never stop according to their own hits.
- The tree's setup charge includes cumulative guide construction, including
  failed refinements. It is still an operation-cost approximation.

Three new Rust tests enumerate all81 outcomes of a four-fixture small league,
check disjointness/coverage for every requested rank, compare weighted estimates
with exact probabilities under both guides, verify complete-parent fallback
after quota exhaustion, and reject insufficient root budgets.
Full Rust tests: **129 passed, four DB-dependent tests ignored**. Two Python
comparison tests pass. Three flag-off HTTP control pairs return identical
estimates, standard errors, game importance and reachability classifications.
The initial sandbox HTTP-test failure was a denied localhost listener; the
permitted rerun passed. No database test was enabled.

## Offline quality screens

Unless stated otherwise: setup seed808, frozen tree, four workers,25k draws in
each main/check batch,200 fresh pilots per leaf for each guide,200-draw floor.
No branch-bound omission or reduced floor is enabled. Existing rough-quality
gates are unchanged, including main ESS≥4 and largest contribution≤35%.

| Arm | Leaves | Additional tree training | Accepted seed pairs | Interpretation |
|---|---:|---:|---:|---|
|32-leaf cap|32|0|0/1|Too coarse; weighted estimates around1e-39 fail quality |
|32-leaf cap, training priority|32|21,200 draws|0/1|Same tree/result; about90 ms additional training |
|64-leaf cap, simple priority|54|0|3/5|Useful offline complete proposal |
|64-leaf cap, marginal rank hint|54|0|3/5|No coverage gain; more setup work |
|Same simple tree,30k main/check|54|0|3/5|No acceptance improvement |
|Same simple tree,50k main/check|54|0|3/5|Different failing seeds; more draws alone do not solve variance |
|Same simple tree,50 pilots/guide|54|0|3/5|Different failing seeds; pilot guide choice remains important |

The batch/pilot screens reuse the same five seeds and are correlated. They are
not15 or20 additional independent successful validations.

Base25k simple-tree results:

| Seed | Main probability | Check probability | Main ESS | Check ESS | Accepted |
|---:|---:|---:|---:|---:|:---:|
|808|1.2875e-33|2.7590e-34|4.29|24.38|No|
|1669|6.1793e-34|9.9940e-34|13.37|10.71|Yes|
|1993|4.4459e-34|6.1375e-34|18.14|7.12|Yes|
|2281|6.1642e-34|1.1035e-33|14.76|9.23|Yes|
|2293|2.0893e-33|5.4931e-34|2.54|10.96|No|

All five main/check comparisons are within a factor5, but the two failed main
batches have concentrated weights. Seed808 has1,452 hits and ESS4.29; one
season supplies43.15% of the weighted total, exceeding the35% gate. Seed2293
has1,611 hits, ESS2.54 and a61.34% largest contribution.

Almost all observed contribution and variance come from leaf28: Flamengo loses
all10 fixtures, strict exceptions are teams24/584/67, and all15 initially
ambiguous rivals are classified. It receives about13k of the25k draws. This
leaf already has its complete rank-side constraints; adding more rival-side
tree splits cannot improve it. The current sampler jointly conditions a
bounded subset of rivals and guides the rest. Improving that within-leaf
proposal is the next useful target. The observed dominance is not a proof
that other leaf contributions are negligible.

### Setup and process cost

The base64-leaf screen measured50.93 ms tree/setup and99.73 ms pilot/main/check
sampling. Full-request tree setups are usually about51–70 ms; selection and
failed constructions for other cells add overhead.

Three repeated fresh seed1669 processes measured median **227.69 ms wall,
approximately450 ms CPU and59.56 MiB peak RSS**, with median54.47 ms setup and
155.69 ms sampling. macOS `time` CPU values are rounded to0.01 seconds. The
first separate invocation took540 ms real; all raw measurements are retained.

The earlier407-stratum prototype's517 ms wall/538 MiB observation was a
historical single-cell measurement, not a paired current benchmark. This tree
uses much less memory in the measured process, but has lower acceptance and
does not establish a production speedup. Some offline multi-seed processes
also show scheduling variation; instrumented times and cold process wall
are recorded separately.

## Full-request work transfer

Both arms run the existing native branches first. The generic tree stage then
orders remaining zeros by complete target-path count, with stable cell order
for ties. It examines at most eight cells, retaining the existing certified
limits and branch acceptance gates. No Flamengo-only allocation is used.

Tree work is transferred from the existing additional-confirmation bank. The
remaining allowance is the minimum of the unused branch quota and remaining
confirmation funding. Combined native/tree branch reserve stays within the
original90k reference-draw branch allowance; the confirmation bank is reduced
by that reserve. Its nominal200k-reference funding does not increase. Actual
draw counts use existing deterministic operation estimates.

This preserves modeled bank limits, **not automatically actual wall time**.
Root-count ordering and proposals which fail admission still consume setup.
The current branch runner may also spend pilots before discovering that no
complete final allocation fits. That can remove useful confirmation work
without producing an accepted replacement.

### Arm A: retain500 pilots per guide

Five group16498 pairs: gain1/loss1, **Flamengo13 accepted0/5**. All54-leaf
Flamengo proposals fail pilot-pair admission. Median HTTP1,346.74→1,403.63 ms
(+4.22%); median CPU3,218.04→3,255.76 ms (+1.17%). Median rare-tail stage
790.84→883.95 ms. Worst full wall pair+34.83%. The gain is Palmeiras16,
seed1669,8.83e-41; the loss is Flamengo12, seed2281, about1.10e-25.

### Arm B:50 pilots per guide

| Snapshot | Gain/loss cell-runs | Baseline/candidate median HTTP ms | Wall change | Baseline/candidate median CPU ms | CPU change |
|---|---:|---:|---:|---:|---:|
|15902|0/0|162.54 /168.88|+3.90%|338.82 /361.49|+6.69%|
|16413|0/0|159.91 /158.21|−1.06%|330.94 /338.34|+2.24%|
|16498|1/3|1,391.27 /1,272.70|−8.52%|3,317.02 /3,252.49|−1.95%|
|16653 current,71d4fea8|0/0|1,138.30 /1,159.59|+1.87%|2,575.20 /2,711.72|+5.30%|
|16982|0/0|261.60 /265.58|+1.52%|537.11 /568.59|+5.86%|
|16983|0/0|265.18 /277.20|+4.53%|551.02 /588.30|+6.76%|
|16653 historical,2d1c1d6f|0/0|668.66 /695.66|+4.04%|1,625.04 /1,694.31|+4.26%|

Flamengo13 results:

| Seed | Main draws | Main probability | Main ESS | Outcome |
|---:|---:|---:|---:|---|
|808|20,000|1.3125e-33|2.30|Rejected; check skipped after main quality failure |
|1669|0|—|—|Pilot pair exceeds remaining bank |
|1993|12,000|4.9555e-35|1.21|Rejected; no check |
|2281|0|—|—|Pilot pair exceeds remaining bank |
|2293|17,000|4.7577e-34|5.05|Rejected by other main-weight gates; no check |

The new estimate is Palmeiras16, seed1669,6.08e-41. Lost estimates:

- Flamengo12, seed1993:1.0301e-25.
- Flamengo12, seed2281:1.1001e-25.
- Palmeiras14, seed2293:8.8791e-25.

These are **one gain and three losses across repeated cell-runs**, involving
one gained distinct cell and two lost distinct cells. No new impossible cell,
no newly reachable cell, no lost reachability proof, no impossible-to-positive
regression. All baseline snapshots have zero undecideds; remaining unestimated
non-impossible cells are already reachable. Game importance is identical.

Across35 pairs, median paired HTTP change+1.30%, total measured CPU+0.57%,
worst HTTP increase+11.99%. Timings include snapshots where no tree draws run;
these short cohorts cannot separate local process variation from compiler/code
layout effects. Coverage losses are deterministic for the fixed seeds and
directly associated with reduced confirmation allocations.

The old comparison helper reported the three lost estimates as lost proofs
because it recognized internal `witness` labels but not normalized HTTP
`reachable`. This is fixed and covered by two tests. Both original and
recomputed comparisons are archived; measured responses/timings are unchanged.

## Recommendation

Keep the tree as an offline experiment. It solves the incomplete-union setup
problem and reduces memory, but the tested funding policies do not improve
the full endpoint's zero coverage.

Next test: improve the joint importance proposal **inside the complete leaf
with largest pilot variance**, while preserving every other leaf's support and
fresh main/check streams. Reuse parent/child suffix metadata to lower setup;
reserve a minimum full final pair before spending transferred pilots. Evaluate
any such change by replacing existing work and comparing all seven snapshots.
Do not drop parent regions, loosen gates merely to publish the failed estimates,
or assume the offline dominant leaf is the only contributing region.

## Reproduction and evidence

Files in `2026-10-02-flamengo-tree/`:

- `prototype.patch`: isolated prototype against `f0a332b8`; includes two
  formatting-only hunks in proof/search produced by `cargo fmt`.
- `summary.json`: compact quality, timing, source/binary identity and test results.
- `evidence.tar.gz`:212 raw files, including all40 flagged HTTP pairs,
  three flag-off controls, standalone screens, resources and test logs.
- `manifest.json`: archive/member SHA256 hashes; all members read back and verified.

No prototype binary is enabled in the checkout. To replay in a temporary copy:

```sh
experiment_dir=$(mktemp -d /tmp/flamengo-tree.XXXXXX)
git archive f0a332b8 odds-rust stats/core | tar -x -C "$experiment_dir"
ln -s "$PWD/experiments" "$experiment_dir/experiments"
CARGO_TARGET_DIR="$experiment_dir/target-baseline" cargo build --release \
  --offline --locked -j4 --manifest-path "$experiment_dir/odds-rust/Cargo.toml" \
  --bin golaberto-odds
patch -d "$experiment_dir" -p1 < \
  experiments/rare_positions/2026-10-02-flamengo-tree/prototype.patch
CARGO_TARGET_DIR="$experiment_dir/target-candidate" cargo build --release \
  --offline --locked -j4 --manifest-path "$experiment_dir/odds-rust/Cargo.toml" \
  --bin golaberto-odds --example branch_stratification

TREE_MODE=1 TREE_LEAVES=64 TREE_TRAINING=0 TREE_GUIDE_VALUES=4000000 \
  "$experiment_dir/target-candidate/release/examples/branch_stratification" \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  17 13 25000 808,1669,1993,2281,2293 trained adaptive 4 tilt plain 200 200

python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline "$experiment_dir/target-baseline/release/golaberto-odds" \
  --candidate "$experiment_dir/target-candidate/release/golaberto-odds" \
  --production-candidate --flag RUST_ODDS_EXPERIMENT_TREE=1 \
  --flag RUST_ODDS_EXPERIMENT_TREE_PILOT=50 \
  --cases 16498,16653,16982,16983,15902,16413 \
  --seeds 808,1669,1993,2281,2293 --output "$experiment_dir/full-pilot50"

CARGO_TARGET_DIR="$experiment_dir/target-candidate" cargo test --release \
  --offline --locked -j4 --manifest-path "$experiment_dir/odds-rust/Cargo.toml" \
  -- --test-threads=1
python3 -m unittest discover -s experiments/rare_positions \
  -p test_compare_rust_reachability.py
```

For the marginal rank-hint arm add `TREE_PRIORITY=rank_hint` to the standalone
command. For optional tree-training use `TREE_TRAINING=200`; for the32-leaf
screen use `TREE_LEAVES=32`. Main/check total and final pilot/floor arguments
control the other standalone screens. For Arm A restrict HTTP cases to16498
and use `RUST_ODDS_EXPERIMENT_TREE_PILOT=500`. The archived old executable's
SHA differs from the final patch build, but this override reproduces its pilot
policy. Binary hashes, actual commands and detailed proposal diagnostics are
recorded with the raw evidence; wall/CPU observations may vary on replay.
