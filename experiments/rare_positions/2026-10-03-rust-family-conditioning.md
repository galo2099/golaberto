# R57: Learn rival families before conditional sampling

Date: 2026-10-03. Status: isolated experiment completed; no production changes,
commit or push.

## Result and recommendation

Learning complete rival families can estimate Flamengo/12th at approximately
`1e-25` using the existing selected-cell grant. The allocation matters: a
500-draw learning pilot performed poorly; reallocating work to a 5,000-draw
pilot recovered two previously zero runs.

Across **one group and five fixed seeds**, the revised family proposal found
Flamengo/12th in **4/5 runs**, versus **3/5 for current Rust**. There were two
zero-to-positive gains (808 and 2293) and one positive-to-zero regression
(1669). These are repeated runs of one distinct cell, not additional team/rank
identities. No other cell changed probability, MC hits/sample count,
reachability, or game importance versus current Rust.

Do **not** enable this as a replacement yet. Retain the experiment and test a
fallback that learns from an already funded, failed native batch. That should
be evaluated with fresh final/check streams and a charged setup budget; it has
not been implemented here. The current experiment establishes that the family
constraints help, but also that incomplete pilot learning can lose an estimate.

Full-request timing stayed close to the current baseline in this small screen,
but preservation is not proved. The revised candidate's largest measured
increase against current Rust was **14.3 ms / 0.93% wall time** and **1.01% CPU**
(seed 2293). Against the matched 5,000-draw native control, that run cost an
additional **39.3 ms / 2.60% wall time** and **3.58% CPU**. No extra production
allowance is assumed.

## Baseline and protocol

- Current source HEAD: `4e812fd506b39a504dc3f771c26e8d027cff2311`.
- Current Rust was rebuilt separately from clean production source. Its binary
  SHA256 is `6084a83dabd40d34c69e4b8b341b785221249986f1ec5302b097978533b39e80`.
- Request: saved group 16498, team 17/Flamengo, requested rank 12. Request SHA256:
  `2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625`.
- Twenty teams and 103 remaining fixtures. Sort specification:
  `pt,w,gd,gf,gp,name`; the model maps the unrecognized final `name` token to its
  existing random tie rule. The candidate uses the same production sorter.
- Fixed seeds: `808,1669,1993,2281,2293`. Four Rust workers. Builds, tests and
  estimator processes were serialized; there were no concurrent estimators.
- All inherited `RUST_ODDS_*` and `RARE_POSITION_*` flags were cleared by the
  harness. Explicit flags and exact command lines are preserved for each run.
- Full matrices, stage logs, process wall time, and child user/system CPU were
  captured. Wall time includes CLI startup, decode and export; these are full
  request experiments, not direct HTTP endpoint latency measurements.
- Local host: `aarch64-apple-darwin`, Rust 1.93.1. No CPU-specific optimization
  was introduced. Intel Xeon production performance was not measured.
- No database access or mutation was needed.

The experimental source starts from R54's isolated native-control harness and
adds family conditioning. With experimental flags absent, seed 808's complete
export is byte-identical to current Rust. Its native 10x seed 808 export is also
byte-identical to the archived R54 v4 control. Thus R54 supplies the experimental
grant selector, while **current Rust remains the production baseline**.

The 10x arm expands only the selected-cell confirmation grant; it does not give
the whole request ten times its production budget. Native and family arms
receive equal grants. Seed 1993 has no eligible selected replacement job and
remains unchanged in every arm; include it in coverage counts, but do not count
it as a successful family fit.

## What was implemented

### Learn from a separate native pilot

For each positive pilot observation, record:

1. The target team's complete remaining win/draw/loss sequence, called the root.
2. Every rival's final status relative to the target: below, equal or above on
   points and applicable wins.
3. The observation's corrected importance contribution.

The statuses are recomputed from the complete fixture assignment, including
target fixtures and starting standings. This avoids reading scratch point
arrays after early returns, omission or proposal-density replay.

Accumulate **sums of corrected contributions**, not the largest individual
season. Retain at most 128 root/family keys, with deterministic ordering, and
freeze the four largest pairs before final sampling. Existing retained keys
continue to accumulate if the key limit is reached. Skipped observations still
incur collection work. No R56 final samples, reference probabilities or named
team families are loaded into proposal construction.

### Compile complete families into shared fixture constraints

For a selected root, the target's final points/wins are known. Each rival gets
an interval on the existing packed points/wins total:

- Below: upper bound less than the target total.
- Equal: lower and upper bound equal to the target total.
- Above: lower bound greater than the target total.

These constraints cover **all rivals**, rather than a subset of favorable
anchors. The implementation reuses the shared fixture domain propagation,
bounded necessary joint model and sequential interval guidance. Every fixture
still has one shared outcome affecting both teams. Applicable wins remain part
of the primary constraints.

Each family fit is a sampling proposal, not an exact enumeration or a
reachability certificate. Necessary-constraint feasibility does not establish
reachability. Failed setup declines that component without marking a root or
cell impossible. Partially completed components survive later setup exhaustion
and their mixture probabilities are renormalized for their root.

### Preserve support and use the complete mixture density

For a learned root, use 20% of the existing **primary native pattern** and 80%
of the frozen family mixture. The primary native component preserves support
for omitted families. Unlearned cached roots and the defensive uncached-root
path retain their existing behavior. This replaces the legacy alternate/case
mixture for learned roots; it is not a 20% share of that entire legacy mixture.

Conceptually, for target root `a` and remaining assignment `y`:

```text
q(a,y) = q_root(a) × [0.2 q_native(y|a)
                      + 0.8 Σ beta_f q_family_f(y|a)]
weight = p_original(a,y) / q(a,y)
```

The implementation evaluates component densities by deterministic replay,
including proposal root selection, cached pattern normalizers and the original
prior factors. Complete ternary families are disjoint, so at most one family
component can contribute density for an assignment. The native density still
contributes even when a family generated it. Weighting by only the chosen
component would be incorrect.

Actual goals, applicable goal tie rules, the production sorter and the existing
GoalTilt likelihood correction remain in use. Equal primary totals are not
treated as arbitrary ranks, and witnesses are not substituted for probability
estimates. The target event indicator is evaluated independently of whether an
assignment belongs to a learned family.

The importance-sampling identity applies conditional on the frozen pilot fit,
with independent final streams. It does not make accepted endpoint outputs an
unbiased estimator after data-dependent publication gates, or establish that
their reported standard errors capture every unobserved tail. Mixture proposal
rates can be learned from samples, but rate optimization is a separate problem;
R57 uses contribution proportions rather than the optimization or control
variates in [He and Owen, *Optimal mixture weights in multiple importance
sampling*](https://arxiv.org/abs/1411.3954).

## Work allocation

Charge native learning draws, bounded observation collection, plan cloning,
attempted family setup, candidate validation, final sampling and density replay
to the same selected-cell deterministic grant. Setup is bounded by 10,000
additional nodes and four million additional guide values; setup charges are
16 units per node plus two per guide value. The fit reserve and a minimum
validation reserve are checked before admitting the clone/fit.

Validation uses up to 1,000 draws, capped by remaining work and a conservative
per-draw operations bound. For this request that bound is 110,746. Final draws
are allocated from remaining work using validation's measured operation count
per draw, divided between independent main/check streams, with a 100,000 draw
ceiling per stream. Operation counts, rather than elapsed time, determine draw
allocation. Final actual work is checked before a replacement can publish.

Other original job reservations are protected. Refunds from replaced jobs are
quarantined, so they cannot silently finance another replacement or second-wave
admission. The complete output comparison checks the consequences for other
cells.

Two revisions were measured:

| Revision | Grant | Shared native pilot | Family final-work headroom |
| --- | --- | ---: | ---: |
| V1 current budget | Original 1x | 500 draws | 2% |
| V1 learning arm | Selected-cell 10x | 5,000 draws | 2% |
| V2 reallocation | Original 1x | 5,000 draws | 8% |

V2 changes only pilot allocation and headroom. Its matched native control also
uses 5,000 pilot draws at the unchanged grant, with the original native final
allocation rule. Absent overrides preserve V1 output.

This is modeled work, not a hard runtime cap. V1 seed 808 demonstrates that an
average pilot cost plus 2% headroom can underpredict the final cost. Its family
attempt spent 152.414 million units against a 147.120 million-unit grant
(**3.60% over**) and was disqualified. All four attempted V2 family proposals
settled within their grants.

## Paired coverage results

All probabilities below are fractions, **not percentages**. A zero means the
endpoint did not publish an estimate; positive observations can still have
failed the quality gates.

| Seed | Current Rust | V1 native 1x | V1 family 1x | Native 5k, 1x | V2 family 5k, 1x | V1 native 10x | V1 family 10x |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 808 | 0 | 0 | 0 | 0 | 1.452e-25 | 1.314e-25 | 1.236e-25 |
| 1669 | 1.610e-25 | 1.631e-25 | 0 | 1.627e-25 | 0 | 1.037e-25 | 9.694e-26 |
| 1993 | 1.030e-25 | 1.030e-25 | 1.030e-25 | 1.030e-25 | 1.030e-25 | 1.030e-25 | 1.030e-25 |
| 2281 | 1.100e-25 | 1.111e-25 | 0 | 1.163e-25 | 1.097e-25 | 1.049e-25 | 1.274e-25 |
| 2293 | 0 | 0 | 0 | 0 | 7.596e-26 | 1.189e-25 | 0 |
| Positive runs | **3/5** | **3/5** | **1/5** | **3/5** | **4/5** | **5/5** | **4/5** |

There are 400 cells per matrix. Total positive **cell-runs** across the five
matrices are respectively 1900, 1900, 1898, 1900, 1901, 1902 and 1901. The only
identity changing coverage is Flamengo/12th. Every matrix has 17 proven
impossible zeros and no undecided cells. V2's remaining reachable zeros by seed
are `2,5,2,3,2`, versus current Rust's `3,4,2,3,3`.

### V2 gates and work

| Seed | Main/check draws each | Main ESS / relative SE | Independent check probability | Actual / grant, million units | Result |
| ---: | ---: | ---: | ---: | ---: | --- |
| 808 | 4,230 | 20.47 / 22.1% | 1.565e-25 | 139.143 / 147.120 | Accepted |
| 1669 | 11,249 main; no check | 1.733 / 76.0% | — | 132.616 / 229.244 | Quality failure |
| 1993 | Existing native path | 7.971 / 35.4% | 2.318e-25 | See inherited accounting note | Unchanged |
| 2281 | 16,155 | 91.01 / 10.5% | 1.347e-25 | 403.377 / 421.331 | Accepted |
| 2293 | 1,529 | 29.94 / 18.1% | 1.179e-25 | 88.956 / 92.399 | Accepted |

The comparison is about coverage and variance indicators. There is no exact
whole-cell golden probability. The larger independent native estimates support
an order around `1e-25`; neither agreement nor ESS proves exact accuracy.

### Why learning and allocation matter

Seed 808's 500-draw native pilot contains 16 positive observations but has ESS
only 1.008. It assigns 99.997% of family weight to one family and misses the
other leading goal-independent family. Its family main batch looks usable
(ESS 33.95), but the check has ESS 1.327 and estimates over five times the main.
That attempt also exceeds its grant.

At 5,000 pilot draws, seed 808 has 163 hits and ESS 5.128. It learns both leading
families with shares 64.8% and 34.5%, plus two small components. V2 can then
publish using about 4,200 final draws per stream at the original grant. The
matched native 5k control still fails its main quality gate. Thus the gain comes
from the learned proposal, not merely increasing training draws.

Seed 1669 shows the remaining weakness. Its 5,000-draw pilot has ESS 4.520 and
assigns 95.96% of family weight to one component. The other leading
goal-independent family is absent from the selected four. The native support
component preserves omitted outcomes, but one main observation contributes
75.7% of the total weight. Main ESS collapses to 1.733, so no check runs and no
estimate publishes. This is a variance failure, not an impossibility result.

At 10x, family conditioning substantially improves seed 808 and 2281 ESS, but
does not improve every seed. Seed 1669's native/family ESS changes from 47.38 to
18.42. Seed 2293's family main has max-share 35.96%, exceeding the unchanged
35% publication gate; native 10x passes. More positive draws alone are not
sufficient.

## Full-request performance

These are single fixed-seed runs per arm, not repeated warm timing benchmarks.
Different arms ran sequentially, and V2 followed the V1 cohort. Timing noise and
the absence of a check batch after quality failure affect comparisons.

| Arm | Median wall, seconds | Median user + system CPU, seconds |
| --- | ---: | ---: |
| Current Rust | 1.407 | 3.373 |
| V1 native 1x | 1.443 | 3.311 |
| V1 family 1x | 1.363 | 3.285 |
| Native 5k, 1x | 1.414 | 3.270 |
| V2 family 5k, 1x | 1.370 | 3.212 |
| V1 native 10x | 2.459 | 4.567 |
| V1 family 10x | 2.325 | 4.506 |

V2 versus current Rust, per seed:

| Seed | Wall change, ms | Wall change | Total CPU change |
| ---: | ---: | ---: | ---: |
| 808 | -16.8 | -1.21% | -0.20% |
| 1669 | -100.2 | -7.51% | -8.28% |
| 1993 | -39.0 | -2.65% | -0.55% |
| 2281 | -73.8 | -5.24% | -7.61% |
| 2293 | **+14.3** | **+0.93%** | **+1.01%** |

Median paired wall/CPU ratios against current Rust are -2.65%/-0.55%; against
the matched 5k native control they are +0.18%/-1.77%. These results do not
establish a production speedup. Seed 1669 is faster partly because it loses
coverage and does not run a check. Seed 1993 is unchanged mathematically, so its
timing difference demonstrates measurement variability.

Report the V1 increases too: family 1x seed 2293 adds **355.5 ms / 23.55% wall**
and **13.01% CPU** against native 1x. Family 10x seed 2281 adds **444.6 ms /
16.29% wall** and **11.90% CPU** against native 10x. Median improvements do not
erase these increases.

In V2's attempted fits, shared native training takes 39.4–42.6 ms, family setup
1.5–2.2 ms, and validation 4.8–9.9 ms. Final sampling diagnostics range
36.3–320.1 ms, including main/check where run. The full rare-tail stage and
other stages are preserved in each command record. Four workers were configured
in every arm; average utilized cores are lower because the pipeline includes
serial work.

## Validation and limitations

- V1: formatting, `cargo check -j4`, five focused family tests, all 93 tests,
  and release build passed. Independent Sol review found no correctness blocker
  for the modeled-operation experiment.
- V2: formatting, check, six focused family/allocation tests and release build
  passed. Root reviewed the allocation-only diff. Default/flag-off seed 808
  output matches V1 byte-for-byte.
- The closed-form test checks two learned components, an omitted equal family,
  exact densities and normalization independently of the replay implementation.
  Tests also cover empty training, inconsistent bounds, deterministic training
  with unchanged native RNG/results, and a finite nonunit GoalTilt correction.
  The standalone GoalTilt test is not a full family/goal integration oracle.
- Root compared complete V2 matrices against current Rust by team/rank key:
  only team 17/rank 12 probability changes. MC counts, reachability and game
  importance match. Archive/member hashes were verified.
- Source and defaults in production `odds-rust/` and `stats/core/` are unchanged.
  Existing user changes, including `db/schema.rb`, were preserved.
- The request-level `work_spent` field uses legacy fixture/team work equivalents
  (123 per broad draw here), not the logical operation units in selected-cell
  grants. Do not compare those units directly.
- Seed 1993's unchanged plain native path reports actual operation work
  426.758M against a nominal 415.060M grant, while its legacy `within_grant` field
  is true. No family attempt occurs; this inherited metric/gate discrepancy was
  not changed. The claim that V2 stays within grant applies to its four attempted
  family replacements.
- Some inherited diagnostic fields are stale: the native-control event reports
  the hypothetical native allocation even when a family is selected, and
  `final_draw_cost_headroom_percent` remains 2 in V2. Use
  `family_headroom_fraction`, `final_plan_*` and final confirmation fields.
  The `clone` timer starts after cloning, so it times the cost calculation,
  not the clone itself; cloning is charged deterministically and included in
  full-request wall/CPU time.
- Outside modeled-work mode, family validation compares raw operation counts
  against a per-draw bound of one and declines the proposal. Only modeled-work
  behavior was screened; this artifact is not ready for generic promotion.
- First seed-808 logs from before diagnostic instrumentation were overwritten.
  Deterministic exports were unchanged; final diagnostic logs and the complete
  subsequent cohort are preserved. All timing claims here use preserved runs.
- Five seeds of one cell are a screening result. Groups 16653 and 16982 were
  not run for this selected-cell experiment. Broad admission, quality and
  performance effects across groups remain unmeasured.

## Reproduction and artifacts

Artifacts: [2026-10-03-family-conditioning](2026-10-03-family-conditioning/).
The portable archives include each complete crate, the sibling `stats/core`
dependency and the required `include_str!` request fixture. They do not require
the temporary R54 checkout. SHA256 and per-member hashes are in
`data/source-archive-manifest.json`; exact source/binary/request provenance is
in `data/provenance.json` and `data/reallocation-provenance.json`.

Run from the repository root with a fresh temporary output directory:

```sh
artifact="$PWD/experiments/rare_positions/2026-10-03-family-conditioning"
repro_dir=$(mktemp -d /tmp/golaberto-r57-repro.XXXXXX)
tar -xzf "$artifact/data/r57-v1-source.tar.gz" -C "$repro_dir"
tar -xzf "$artifact/data/r57-reallocated-source.tar.gz" -C "$repro_dir"
cargo build --release -j4 --manifest-path "$repro_dir/odds-rust/Cargo.toml"
cargo build --release -j4 --manifest-path "$repro_dir/reallocated/odds-rust/Cargo.toml"
cargo test -j4 --manifest-path "$repro_dir/reallocated/odds-rust/Cargo.toml" family_ -- --test-threads=1
```

Rebuild the baseline from the recorded HEAD if desired:

```sh
mkdir -p "$repro_dir/current"
git archive 4e812fd506b39a504dc3f771c26e8d027cff2311 odds-rust stats/core \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  | tar -x -C "$repro_dir/current"
cargo build --release -j4 --manifest-path "$repro_dir/current/odds-rust/Cargo.toml"
python3 "$artifact/scripts/run_v1_cohort.py" \
  "$repro_dir/current/odds-rust/target/release/golaberto-odds" \
  "$repro_dir/odds-rust/target/release/golaberto-odds" \
  "$artifact/data/group-16498-44eabb47.json" 808,1669,1993,2281,2293
```

The V1 runner writes to `logs/v1-replay`, preserving the original V1 cohort.
The reallocation runner writes to `logs/reallocation/cohort`; copy the artifact
directory before rerunning it to preserve the recorded V2 measurements:

```sh
cp -R "$artifact" "$repro_dir/replay-artifacts"
python3 "$repro_dir/replay-artifacts/scripts/run_reallocation.py" \
  "$repro_dir/reallocated/odds-rust/target/release/golaberto-odds" \
  "$artifact/data/group-16498-44eabb47.json" 808,1669,1993,2281,2293
python3 "$repro_dir/replay-artifacts/scripts/analyze_reallocation.py"
```

The harness clears inherited odds flags, sets `RUST_ODDS_LOG=1`, then runs:

```text
golaberto-odds estimate REQUEST EXPORT SEED 4
```

V1 native/family flags:

```text
RUST_ODDS_EXPERIMENT_NEIGHBOR_TRANSFER=native
RUST_ODDS_EXPERIMENT_NEIGHBOR_CELL=17:12
RUST_ODDS_EXPERIMENT_NEIGHBOR_BUDGET_MULTIPLIER=1   # or 10
RUST_ODDS_EXPERIMENT_FAMILY=1                    # family arm only
```

V2 adds `RUST_ODDS_EXPERIMENT_FAMILY_PILOT_DRAWS=5000` in both paired arms and
`RUST_ODDS_EXPERIMENT_FAMILY_HEADROOM=0.08` in the family arm. The selector is
an offline experiment control; the family model itself has no named-team or
rank-specific rule. These flags exist only in the archived experimental build.

Raw V1 data are in `logs/seed808` and `logs/cohort`; V2 data are in
`logs/reallocation/cohort`. JSON summaries contain full estimates, gates,
operation accounting, stage timing, user/system CPU, commands and output hashes.
The original V1 and allocation-only changes are separately saved under `patch/`.

## Sources and next hypothesis

- [R54 neighbor-transfer experiments](2026-10-03-rust-neighbor-transfer.md):
  native controls and selected-cell grant expansion.
- [R56 successful-season audit](2026-10-03-rust-hit-patterns.md): two dominant
  rival families and the unmodeled remainder. Used to motivate R57, not as its
  training input.
- [He and Owen (2014)](https://arxiv.org/abs/1411.3954): mixture importance
  sampling and learning proposal rates. R57's complete mixture correction is
  supported independently by its closed-form tests; no claim is made to have
  implemented that paper's optimized estimator.

Next experiment, **not implemented**: preserve an accepted native estimate,
and use bounded corrected observations from an already funded failed native
main batch to fit family proposals for a later attempt. Reserve and charge new
setup and fresh independent main/check streams within that cell's remaining
grant. This could learn from more informative observations without spending a
separate 5,000-draw pilot on every candidate. It must still be tested for full
matrix coverage, admission/refund effects and latency; preserving support alone
does not prevent high variance.
