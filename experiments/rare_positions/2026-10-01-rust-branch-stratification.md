# R34: complete exception branches for Londrina/3rd

## Final disposition — 2026-10-02

Committed the offline drivers and evidence. The successful complete-branch
mechanism was subsequently integrated by R36 and remains in production. The
measurements below describe the original offline experiment.

Raw files are stored in `2026-10-01-branch-stratification/evidence.tar.gz`, with per-file
hashes in `manifest.json`. Extract that archive in its directory before using
the historical commands. Statements about flags, defaults and commit status
below refer to the experiment date. Current disposition is recorded in
[the decision report](2026-10-02-rust-experiment-decisions.md).

## Result

The proposed three branches are sound and useful. Sampling them separately
produced an accepted Londrina/3rd estimate in 2 of 11 seeds. Enumerating one
tightly constrained rival's outcome paths inside each branch, then allocating
draws from independent pilots, improved that to **7 of 11**. Accepted estimates
were **1.67e-34–7.92e-34 as fractions** (1.67e-32%–7.92e-32%). These are rough
Monte Carlo estimates, not a golden probability or upper bound.

The refined 30k variant averaged about **22.4 ms for pilots plus both final
batches**, with **7.7–12.5 ms setup** and about 1 ms model construction. It used
four workers. These are local **offline single-cell** timings, not full-request
latency or Xeon timings. CPU includes process/model/setup costs amortized over
five or six seeds; mean CPU was 49.03 ms/seed in development and 46.27 ms/seed
in fresh validation. Production does not call this new experiment.

The refined sampler missed the two seeds accepted by the simpler sampler.
Replacing the latter outright would therefore yield seven gains and two losses
over these eleven paired cell-runs. A subsequent production experiment should
retain existing accepted estimates and refine unresolved zeros, with a measured
full-request budget and all seven reference snapshots. No such integration or
full-matrix regression experiment was performed here. No commit or push.

## Why the three branches cover the event

Saved group16653 request: `reference/2026-09-30-hundredfold/inputs/group-16653-71d4fea8.json`.
SHA256: `b56f2ac8bdb017ab98814a8f653b511b54340f4437c8e890894e827ce3ee9962`.
The snapshot has 20 teams and 85 remaining fixtures; applicable sort rules are
points, wins, goal difference, goals scored, head-to-head and name.

- Londrina95 has 28 points and 7 wins, with eight games remaining. Third requires
  winning all eight: 52 points, 15 wins. Dropping any points leaves at most50,
  below Vila Nova, Fortaleza and Novorizontino. The model probability of this
  eight-win path is 1.416045169745338e-4.
- Vila Nova70 already has54 points and must finish ahead.
- Fortaleza22 and Novorizontino2064 each have51 points and14 wins. To finish
  below Londrina they can add at most one point. At52, their14 wins lose to
  Londrina's15. Their mutual fixture must be drawn if both finish below; each
  must lose every other fixture.
- Each also plays Atlético-GO279. Both being below forces two Atlético-GO wins,
  taking it from49 to55 points. Thus at least one of these three is ahead, and
  Londrina/3rd permits exactly one of them ahead besides Vila Nova.

The three surviving exception cases are therefore Novorizontino above,
Fortaleza above, and Atlético-GO above. The all-below case and cases with another
exception are removed by shared-fixture propagation. Actual full ranks are still
checked with the production sorter, including any further ties.

The original R33 expansion already generated these three cases. The new work
changes how they are sampled and allocated: independent strata ensure that a
branch does not disappear through a weak mixture allocation.

### Audited plain branches

| Additional team above | Forced fixtures | Joint fixtures | Guided fixtures |
|---|---:|---:|---:|
| Novorizontino2064 | 8 | 33 | 44 |
| Fortaleza22 | 8 | 31 | 46 |
| Atlético-GO279 | 32 | 26 | 27 |

In the Atlético-GO case, game364743 is an Atlético-GO home win over
Novorizontino; game364756 is its away win over Fortaleza; game364768 is
Novorizontino–Fortaleza drawn. The other forced results follow through propagation.

Pilot and final contributions suggest the Novorizontino case is usually the
largest, Fortaleza smaller, and Atlético-GO around e-40. This is sampling
evidence, not a proof of their relative probabilities. The most visibly
constrained branch is not the branch responsible for most of the estimated mass.

## Generic experimental algorithm

New code: `odds-rust/src/joint_caps/propagated/lazy/strata.rs`, exercised by
`odds-rust/examples/branch_stratification.rs`. No group, team or rank is hardcoded.

1. Enumerate every positive supported target W/D/L path in the existing target
   terminal table. Up to64 paths; if enumeration is incomplete, reject the
   proposal rather than report a partial sum.
2. Propagate shared fixture constraints, using points and applicable wins.
   Choose the direction with fewer possible exceptions (above or below).
   Enumerate all strict exception masks within rank cardinality, up to256
   attempted cases in total. Inclusive complements preserve ties; final score
   sorting filters the remaining false positives.
3. Construct each complete case with all-team propagation, forced fixture
   compaction, the existing exact joint block (four rivals here), and the
   existing guided sampler. Offline construction allows100k joint setup nodes
   per build and retains the existing four-million guide-value limit. This
   setup allowance is not a production budget recommendation.
4. In refined mode, select one rival whose propagated interval leaves2–16
   complete legal outcome paths and interval mass below0.9 conditional on
   fixture masks. Prefer fewer paths, then smaller mass. Enumerate all paths,
   fix their fixtures for both endpoints, and rerun propagation before building
   the joint proposal. Multiply each path's original fixture probability exactly
   once into its normalizer. Setup exhaustion or numerical failure rejects the
   entire union; relaxation-infeasible subcases may be omitted.
5. Independent pilots compare rank guidance and propagated interval guidance;
   choose the higher pilot ESS. Trained allocation uses pilot standard
   deviations: a draw floor per stratum, followed by10% equal allocation and90%
   proportional to contribution standard deviation. Freeze this before drawing
   independent main and check batches. Pilot values are excluded from estimates.
6. Sum unconditional branch contributions, not branch averages. Sum independent
   variances, and compute combined ESS, maximum weight share and half-batch
   disagreement with each branch's actual draw count. Correct P/Q weights include
   fixed paths, domain masks, joint conditioning, guided outcomes and score tilt.
7. Apply the unchanged production tail acceptance gates via `rare_tail::accepted`.
   All candidate seasons use the production sorter. For diagnosis the offline
   example runs a check even when the main fails; production can skip that work.

The refinement produced22 disjoint subbranches:

| Parent exception | Refined rival | Complete paths |
|---|---|---:|
| Novorizontino above | Fortaleza22 | 10 |
| Fortaleza above | Novorizontino2064 | 9 |
| Atlético-GO above | CRB77 | 3 |

## Development results

Seeds808,1669,1993,1847,1861. Draw budgets are **per whole cell per final batch**,
not per branch:30k means30k main plus30k check. Plain pilots use2k per branch
per guidance; refined pilots use500. Plain allocation floor2k, refined floor200.
Adaptive guidance runs both pilots; fixed rank uses one. All runs are serial
and each arm uses at most four workers. Goal tilt is enabled except native-score
diagnostic arms. Wall times below exclude model/setup and include pilots/main/check.

| Strata | Allocation/guidance | Draws/batch | Accepted/5 | Mean stage ms | CPU ms/seed |
|---|---|---:|---:|---:|---:|
| 3 | Equal, rank | 30k | 1 | 15.91 | 37.17 |
| 3 | Equal, interval | 30k | 0 | 12.80 | 33.91 |
| 3 | Trained, adaptive | 30k | 1 | 24.05 | 35.96 |
| 3 | Equal, rank | 100k | 1 | 46.05 | 106.12 |
| 3 | Trained, adaptive | 100k | 2 | 82.24 | 106.48 |
| 3 | Equal, rank, native scores | 100k | 2 | 51.31 | 115.08 |
| 22 | Equal, rank | 30k | 1 | 17.17 | 50.34 |
| 22 | Trained, adaptive | 30k | **4** | **22.35** | **49.03** |
| 22 | Equal, rank | 100k | 0 | 52.54 | 151.63 |
| 22 | Trained, adaptive | 100k | 4 | 66.27 | 120.66 |
| 22 | Equal, rank, native scores | 100k | 2 | 47.95 | 154.23 |

Plain setup was2.05–2.32ms; refined setup7.69–8.46ms in these arms, with a cold
refined audit of12.49ms. Larger batches did not monotonically improve acceptance:
occasional large weights can expose instability that a smaller batch misses.

## Fresh validation and regressions

Fresh seeds2111,2129,2141,2161,2179,2197, same request and30k final batches.
Plain equal accepted1/6, refined trained/adaptive3/6. Mean stage wall16.36→22.40ms;
CPU38.13→46.27ms/seed; setup2.21→7.98ms. This is **+36.9% stage wall** and
**+21.3% amortized CPU** relative to the offline plain sampler, not the Rust
full-request baseline.

Combined eleven seeds:

| Seed | Plain equal accepted | Refined adaptive accepted | Refined main fraction, if accepted |
|---:|:---:|:---:|---:|
| 808 | No | Yes | 4.94e-34 |
| 1669 | No | Yes | 5.24e-34 |
| 1993 | Yes | No | — |
| 1847 | No | Yes | 3.35e-34 |
| 1861 | No | Yes | 3.40e-34 |
| 2111 | Yes | No | — |
| 2129 | No | Yes | 3.00e-34 |
| 2141 | No | Yes | 7.92e-34 |
| 2161 | No | No | — |
| 2179 | No | Yes | 1.67e-34 |
| 2197 | No | No | — |

The refined main at2161 fails the half-batch disagreement gate (1.526>1.5).
At2197 the check ESS2.479 fails the3 threshold. At1993 and2111 large weights
cause main ESS below4. These are rejected estimates, not impossibility proofs.
Keeping either sampler's accepted estimate would cover9/11 empirically, but this
portfolio has **not** been integrated, timed or validated as a production method.

R32 and R33 full requests previously left Londrina/3 zero on all five development
seeds; those paired requests are in the R33 archive/report. We did not rerun R32
on the six fresh seeds here. Other groups, reachable/impossible counts and
full-matrix regressions were not measured by this single-cell experiment. It
adds probability candidates, not a new reachability proof algorithm.

## Correctness checks and limitations

- All60 library tests passed after the final numerical guard, with four test
 threads. The four new stratum tests check complete-case probabilities
 against exhaustive W/D/L enumeration, secondary shared-fixture mass accounting,
 incomplete target enumeration rejection, and stable combination at e-240.
- A zero floating-point normalizer is a setup failure, not an impossibility proof.
 Complete assignments are sorted with actual scores; no score cap proves
 infeasibility and no best/worst rank interpolation supplies a missing rank.
- Accepted e-34 values satisfy existing finite-batch gates. Gates and ESS do not
 certify accuracy; residual weight concentration remains, and there is no
 independently established golden probability at this scale.
- The measured source excludes unrelated late-gap workspace experiments. Baseline
 is Rust2290fe044a86fed57c20a5c586ca1ffe87e74e23; R33 dependency changes are included
 but its optional HTTP stage remains disabled. Local platform is ARM Mac,
 Rust1.93.1; production uses four Xeon cores. No production timings are claimed.

## Reproduction

Run from the repository root, with a clean baseline plus the archived focused
patch. Build/tests/measurements must be serialized to preserve four-core usage.

```sh
cargo build --release --locked -j4 --manifest-path odds-rust/Cargo.toml --example branch_stratification
cargo test --locked -j4 --manifest-path odds-rust/Cargo.toml --lib strata::

odds-rust/target/release/examples/branch_stratification \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16653-71d4fea8.json \
  95 3 30000 808 trained adaptive 4 tilt refine 500 200

python3 experiments/rare_positions/experiment_branch_stratification.py \
  odds-rust/target/release/examples/branch_stratification \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16653-71d4fea8.json \
  /private/tmp/r34-validation --cohort validation \
  --seeds 2111,2129,2141,2161,2179,2197
```

Use `--cohort plain` and `--cohort refined` for the development arms. Output
directories must be new. `TOTAL_DRAWS=0` audits branches without sampling.
Archives in `2026-10-01-branch-stratification/` contain compressed raw JSONL,
commands, measured executable/request hashes, CPU/wall summaries, branch audits,
test logs and a patch against2290fe04. Development binaries preceded the final
logging/floor options and numerical guard; final smoke reproduces seed808's
probability exactly. Rebuilding can change timings without changing this seed's
estimate. The archived summaries retain the original measured hashes.

## Recommendation

Continue with complete-case refinement and pilot allocation as an opt-in
full-request experiment. Preserve existing accepted estimates, prioritize weak
zero cells with few complete exception cases and a rival having few outcome
paths, and reallocate existing tail work before adding a stage. Measure all
seven snapshots at fixed seeds, retaining any latency increase explicitly.
Increasing every batch to100k or replacing all existing proposals is not
supported by these results. Current production defaults are unchanged.
