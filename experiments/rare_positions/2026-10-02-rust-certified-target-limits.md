# Certified target limits and complete parent fallback — 2026-10-02

## Result and recommendation

Implemented in the current Rust coverage profile, enabled by default and included
in this release. No deployment or database mutation was performed.

The default fixed seed **808** now estimates **Palmeiras / 16th, group 16498**, at
**1.68e-40 probability** (1.68e-38%). This is **one distinct additional cell**.
It was previously proved reachable but assigned zero. The estimate is a rough
model probability, not a certified probability or an independently known truth.

The final binary's seven-snapshot warm comparison has **8 gained cell-runs, no
losses or reachability regressions**: the same Palmeiras cell in all eight
measured repeats. Aggregate median HTTP latency decreased **1.03%**; aggregate
process CPU decreased **0.23%**. Four estimator workers were used throughout.
This satisfies the measured aggregate production-seed budget without an added
allowance. It does **not** establish unchanged latency for every input or seed.
The historical 16653 snapshot increased **2.81%** in that final warm run.

On five distinct fixed seeds, the selected configuration finds Palmeiras 16th
for **808, 1993 and 2293**, and rejects it for **1669 and 2281**. No quality gate
was relaxed. Other groups gained no distinct cells. The held-out seed cohort
increased aggregate HTTP time **2.40%**, CPU **0.11%**; the initial three-seed
16498 cohort increased HTTP time **9.12%**, despite CPU falling **1.85%**. These
are explicit latency increases, not a new authorized production allowance.

Keep the bounded transferred configuration for the current fixed-seed endpoint.
Do not restore the additive late-only configuration or broadly move certification
before all existing pilots. Reevaluate latency when snapshots or the seed change.

## Baseline and measurement

- Baseline: the current Rust working implementation at master `a10f6489`, frozen
  before this change. It includes preexisting **disabled** experimental Rust code;
  those changes and unrelated working files were preserved.
- Local current-DB group 16498 export was compared read-only against the saved
  request. Games, scores and powers match exactly; only metadata differs.
- Six reference requests: groups 15902, 16413, 16498, 16653, 16982 and 16983;
  plus the historical `group-16653-2d1c1d6f.json` fixture: **seven snapshots**.
- Cold seed cohorts alternate baseline/candidate order. Warm cohorts use two
  warm-up pairs, then eight measured alternating pairs for each snapshot.
- CPU is child-process user+system time. Warm CPU includes warm-ups and startup
  for both arms. HTTP medians exclude warm-ups. Timing is diagnostic only.
- There were **198 measured pairs / 396 calculations**, plus 28 warm-up pairs.
  Repeated gains are reported as cell-runs, not distinct cells.
- No simultaneous compilation or benchmarking. Four workers per calculation;
  paired requests run sequentially. Local results are not Xeon measurements.

Final binaries:

```
baseline SHA256 bd0a717e214362d5f9394ca1de9200e21fcd12b303d779399638b6b911d12702
candidate SHA256 806f576dec367bf7b618eedd853372bd9fdb9023e8e9928ed25f023761d0e821
```

## Implementation

### 1. Certify target totals before complete target-path generation

Reuse the native explained fixture solver to ask a necessary question: can at
least the required number of rivals reach a specified target packed total?
The upper direction requires `rank - 1` rivals; the negated direction requires
`teams - rank` rivals. Search over attainable totals with a shared node quota.

In this relaxation the target's extreme outcome generously improves rivals.
A requested threshold replaces the native minimum threshold. Equality is counted
in the rivals' favor. Therefore full infeasibility certifies a target upper/lower
limit despite unrestricted goals and later tiebreakers. Feasibility, an unfinished
search or a cohort limit does not certify anything.

Points and wins are packed only if wins immediately follows points in the phase
sort order, with a stride greater than every attainable final win count. Otherwise
only points are used. Unsupported scoring, bonus points, external teams and model
size/overflow limits retain the original proposal behavior.

Certificates are cached per immutable request and cell. Both exhaustive and lazy
joint target-table constructors use the same certified terminal predicate. Cached
setup is charged only once. The certificate is calculated before the **complete
branch** constructor, after ordinary portfolio estimates are frozen. Existing
training proposals retain their original support and streams.

The initial individual terminal must admit more than 64 target paths to trigger
this additional proof work. Queries share at most **16,000 nodes per cell**,
bounded further by remaining modeled work. Each branch stage examines at most
eight unresolved cells. Original constructive limits (100/cell, 3,200 total),
cap/floor proof limits (500/cell, 10,000/direction) and neighborhood default
(10,000 candidates) are unchanged.

### 2. Complete parents survive failed secondary refinement

Construct **all primary target-path/strict-rank strata first**. If primary
construction fails, skip the proposal; a partial primary union cannot estimate
the whole event. Each primary branch is disjoint and retains its raw fixture
probabilities and normalizers.

Optional refinement then has 2,000 additional nodes per parent, 10,000 per cell.
If any split fails, underflows or exhausts that quota, stop and discard the
optional children. Retain the **entire complete primary union**. This avoids both
missing probability mass and the poorer allocation observed when only successful
children replaced their parents. No incomplete child union is published.

### 3. Reallocate existing confirmation work

Run newly tightened complete proposals after ordinary finals and before extra
confirmations. Independent pilots choose guidance and allocation. The selected
new proposal uses a fixed **25,000-draw main and independent check** allocation;
all strata receive positive draws. The ordinary branch configuration remains
30,000. A failed main is not published without a qualifying independent check.

Proof setup, proposal setup, pilots and reserved main/check work are deducted
from the existing additional-confirmation quota. Handled cells are excluded from
late branch retries. The sum of modeled stage allowances does not increase.
Duration never controls default admission. Earlier positive estimates are frozen.

The portable proof charge is `nodes × (8 × fixtures + 4 × teams) + 16 × fixtures`.
It is a scheduling estimate, not an instruction count. Actual builder nodes,
including failed refinements, enter proposal setup accounting. Small unsuccessful
constructors remain bounded by their existing explicit node/cell limits.

## Palmeiras 16th

Palmeiras starts with 57 points, 16 wins and ten remaining matches. The individual
rank screen permits zero through five extra points: 1,098 W/D/L target paths,
with aggregate mass **1.1168892538e-4**. Complete enumeration previously stopped
at the 64-path limit.

The new shared-fixture relaxation proves that even one additional point cannot
support 15 teams ahead. It consumes **10,683 nodes**, roughly **22–25 ms** locally
across the threshold queries. With stride 29, its packed upper limit is 1697;
57 points / 16 wins is 1669, while one draw would produce 1698 and is excluded.

There is now one target path: ten losses, prior **2.0019063432e-8**. This is a
certified necessary event, not the probability of finishing 16th. The other teams'
points, wins and actual goal tiebreakers make the complete event much rarer.

The complete parent union has **18 strata**. Optional refinement fails after
1,197 extra builder nodes, so the entire 18-parent proposal survives. In the final
default request, setup is approximately 50 ms, including threshold proof and the
failed refinement. The default paired estimate is **1.68e-40**; other accepted
seeds give **2.69e-40** and **3.03e-40**. They support a rough 1e-40 scale, not a
precise calibrated estimate. Existing sorter and score sampling remain in force.

For seed 808, group 16498 changes from 22 to 21 zeros: 17 impossible and four
reachable without estimates. Remaining reachable zeros are **Palmeiras 14th/15th
and Flamengo 12th/13th**. No unresolved reachability statuses remain in these
reference snapshots before or after this change.

## Paired experiment sequence

| Arm | Measured pairs | Gained / lost cell-runs | HTTP delta | CPU delta | Decision |
| --- | ---: | ---: | ---: | ---: | --- |
| Late certification; mixed successful children + failed parents | 21 | 0 / 0 | +3.19% | +3.01% | Reject hybrid allocation |
| Late certification; complete primary fallback, 16498 only | 3 | 2 / 0 | +15.56% | +9.36% | Useful proposal, additive cost rejected |
| Certification before all individual pilots | 21 | 3 / 1 | +6.71% | +6.25% | Reject; loses Flamengo 12th at seed 1669 |
| Transfer existing confirmation work, 30k | 21 | 2 / 0 | +3.33% | −1.46% | Tune batch size |
| Transferred 20k, 16498 only | 3 | 1 / 0 | +5.81% | −1.40% | Reject weaker default-seed coverage |
| Transferred 25k, 16498 only | 3 | 2 / 0 | +9.12% | −1.85% | Selected; custom-seed latency caveat |
| Selected warm seed 808, explicit flags | 56 | 8 / 0 | −1.74% | −1.83% | Production-seed validation |
| Selected held-out seeds 2281/2293 | 14 | 1 / 0 | +2.40% | +0.11% | Retain; report latency increase |
| Final rebuilt default binary, warm seed 808, no flags | 56 | 8 / 0 | −1.03% | −0.23% | Default validation |

HTTP aggregates use ratios of summed arm times, not averages of per-pair ratios.
For warm runs, sum per-snapshot medians. Repeated deterministic responses are
identical across all warm comparisons. Final per-snapshot warm timings:

| Snapshot | Baseline HTTP ms | Candidate HTTP ms | HTTP delta | CPU delta |
| --- | ---: | ---: | ---: | ---: |
| 15902 | 152.45 | 150.48 | −1.29% | −1.25% |
| 16413 | 144.69 | 143.77 | −0.63% | 0.00% |
| 16498 | 1252.93 | 1191.59 | −4.90% | −3.57% |
| 16653 current | 1186.48 | 1191.19 | +0.40% | +1.43% |
| 16982 | 240.95 | 241.82 | +0.36% | 0.00% |
| 16983 | 237.33 | 236.89 | −0.18% | +0.20% |
| 16653 historical | 676.74 | 695.74 | +2.81% | +3.78% |

Small unaffected-group differences include build/run noise. They are reported,
not assumed away. The baseline-relative default matrix gains one cell while
using approximately the same aggregate CPU and less aggregate wall time.

## Tests and correctness boundaries

**128 Rust tests pass; four database-dependent tests remain ignored.** The initial
sandbox run passed library/estimator tests but could not bind HTTP sockets; the
complete suite passed with approved local socket access. No DB-write test ran.

Relevant checks:

- Exhaustively enumerate all outcome assignments in small leagues: a threshold
  refutation must never remove a legal assignment, in either orientation, with
  wins second or later in the phase order.
- Exhaustively check complete primary/secondary/fallback event mass and disjoint
  strata; compare the weighted sampler against exact small-league probabilities.
- Verify cached certificates charge once, quota exhaustion supplies no false
  bound, and both target builders apply the terminal restriction.
- Group 16498 and 16653 responses are byte-identical across repeated four-worker
  runs, one worker and logging disabled. Every logged reservation is within quota.
- Paired comparisons check lost positives, lost reachability, impossible-status
  regressions, existing odds/error changes and game-importance changes.

No goal cap establishes impossibility. A points/wins feasible relaxation does
not establish reachability. No intermediate ranks are inferred from extreme
ranks. Complete seasons are ranked by the production sorter. A witness, necessary
event probability or zero-hit confidence bound is never substituted for a
probability estimate.

## Reproduction and artifacts

Build and test the candidate from this working tree:

```sh
cargo build --release --offline --locked -j4 --manifest-path odds-rust/Cargo.toml --bin golaberto-odds
RUST_TEST_THREADS=1 cargo test --release --offline --locked -j4 --manifest-path odds-rust/Cargo.toml
```

Freeze the prior Rust working implementation as `/tmp/odds-baseline`. For source
reconstruction, start at `a10f6489`, apply the archived `baseline-source.patch`,
and copy `baseline-residual.rs` to
`odds-rust/src/joint_caps/propagated/lazy/residual.rs` before building. The final
focused `source.patch` applies on that baseline source. The rejected broad early
placement is captured separately in `early-placement.patch`.

```sh
python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline /tmp/odds-baseline --candidate odds-rust/target/release/golaberto-odds \
  --production-candidate --persistent --output /tmp/certified-warm

python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline /tmp/odds-baseline --candidate odds-rust/target/release/golaberto-odds \
  --production-candidate --seeds 808,1669,1993,2281,2293 \
  --output /tmp/certified-seeds
```

The final defaults require no extra flags. See README for diagnostic rollback
and draw-count flags. Explicitly transferring work with 25k reproduces the same
selected estimator configuration on the earlier experimental binary.

Artifacts: [`2026-10-02-certified-target-limits/`](2026-10-02-certified-target-limits/).
Each summary records request and binary hashes, seeds, four-worker timing, stage
costs, reservation totals and comparisons. Compressed raw archives contain
responses/logs and the individual trial summaries. `tests.log` records the suite.

## Scientific context

Football elimination under 3/1/0 is NP-complete even with three remaining matches
per team. This motivates bounded certificates whose exhaustion stays unknown,
rather than assuming every request can be fully solved inside a fixed budget.
[Bernholt et al., 1999](https://image.informatik.htw-aalen.de/~thierauf/TI2/Netz/football.pdf).

The rank-bounding IP work and complementary soccer CP/MIP experiments motivate
shared fixture assignments and explicit infeasibility proofs. Their runtimes are
not imported as a full-matrix production allowance, and no external solver was
introduced here. [Gotzes & Hoppmann](https://link.springer.com/article/10.1007/s12351-020-00546-w),
[Duque, Arbelaez & Díaz](https://www.aimsciences.org/article/doi/10.3934/jimo.2018109).

The existing native explanation/cohort-cache solver remains the implementation;
CP-SAT's solver architecture is background for future offline comparisons.
[Perron, Didier & Gay](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.CP.2023.3).
The staged outcome/tiebreaker/goal treatment in NHL clinching is relevant context,
while this change supplies only necessary packed-total restrictions and retains
actual goal checks. [Rosenberg et al.](https://arxiv.org/html/2605.13142v2).

## Shipping verification

The release is built from committed master `a10f6489` plus the focused certified-
limit patch, excluding preexisting residual-loop, late-gap and point-tilt
experiments. The diagnostic examples and Flamengo13 offline artifacts accompany
the release; the expanded Flamengo construction caps remain offline.

Fresh Rust tests and fixed-seed response comparisons against the previously
measured candidate are recorded in `2026-10-02-certified-target-limits/shipping/`.
The earlier performance tables remain the original measurements, not a claim
that compilation without disabled experiments has an identical binary hash.

Shipping review also bounds early branch work by the configured additional-
confirmation allowance, so reducing that diagnostic fraction cannot overfund
the transfer. The default capacity and allocation are unchanged; a unit test
and a reduced-allowance full-request regression test cover this guard.

Final shipping checks:128 tests passed (126 full-suite tests plus two diagnostic
example tests);4 database-dependent tests remain ignored. Worker/logging
reproducibility and the reduced-confirmation-budget integration test pass.
All seven seed808 reference responses are byte-for-byte identical to the
previously measured candidate, including Palmeiras16th. Seven cold HTTP pairs
measured aggregate wall+0.75%, CPU+1.48% relative to that candidate; these
shipping checks are not a replacement for the earlier warm performance cohort.
No extra algorithmic work quota was added. Expanded Flamengo13 caps and the
proposed adaptive constraint tree remain offline/unimplemented, respectively.

The full JSON summaries, source patches and test logs are packaged in
`2026-10-02-certified-target-limits/text-evidence.tar.gz` under their original
relative names. Extract that archive in its directory before using the historical
reproduction paths above. `shipping/summary.json` is the concise release check.
