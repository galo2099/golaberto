# R61: target-path overflow and unfunded-pilot families

2026-10-04. Experiment complete; default adoption rejected on latency. The user asked to fix the remaining default-seed
reachable zero, Palmeiras/14th in group 16498. Production baseline is master
`430ac57fd15dc4d451be7405bcce1e5d9febafc1`, with its current family fallback.
Sol owns investigation and review; Luna owns all source and harness edits.
No commit, push, database mutation or production-default change.

## Baseline and acceptance

Frozen baseline SHA256:
`87991033bdb81854cec91f3d29c32c66f5b089f51d2530590e48b2a0efc40085`.
Use identical tracked snapshots, fixed seeds and four workers. Heavy commands
are serialized. Measure wall time and child process user+system CPU; preserve
baseline positive estimates except request-total `work_spent`, game importance,
reachability classifications and the independently confirmed family gains.
Current Rust latency is the reference. No additional time allowance is presumed.

## Experiment A: structural target-path screening

The certificate permits five packed totals: 57/16, 58/16, 59/16, 60/16 and 60/17
(points/wins). Enumerate all positive target W/D/L paths admitted by the same
necessary terminal predicate; run shared-fixture `Domains::propagate` after
fixing the target results. Only explicit relaxation infeasibility can prune.

Complete scan: **186 paths, zero refutations**. Counts by total are 1, 10, 45,
120 and 10. Enumeration takes 0.022 ms and propagation 2.203 ms in one local
diagnostic; this is setup-only, not a full-request timing. The aggregate prior
mass is `9.037255614233056e-6`, a necessary-event probability, not the rank
probability. Feasible relaxations do not establish reachability.

Moving this screen before the 64-root cap cannot fix the case. Retain it as an
offline diagnostic; do not add this unsuccessful screening work to production.

```sh
cargo build --offline --release --locked -j4 --manifest-path odds-rust/Cargo.toml --example target_path_screen
odds-rust/target/release/examples/target_path_screen \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  16 14 4096
```

## Experiment B: late family training for an unfunded pilot

The existing sampled-root LazyJoint proposal already covers all target paths,
including uncached paths. Train a family proposal only after all existing native
and originally paired family work finishes. Eligible jobs have a supported Lazy
pilot but no originally funded pair, and remain zero and non-impossible.

Prefund a bounded fresh training batch from the existing residual confirmation
bank. Fit the existing at-most-four families; retain native support and complete
mixture/score likelihood corrections. Fresh validation and independent MAIN/CHECK
streams use the existing publication gates. Training is never publication evidence.
All setup and sampling are charged to the reserved residual grant. Fresh training
and final draw costs are pilot/validation estimates, following the current native
budget policy. Their observed costs can vary; an actual overrun rejects publication
and stops fallback work. This is not a strict upper bound on CPU or operation count.

Upfront pilot recording was considered and rejected before validation: adding
its cost to native training accounting can change previous final allocations;
charging it only later cannot guarantee the recorder had sufficient funding.
Late paid training avoids that coupling and keeps native scheduling intact.

First paired seed-808 screen: **no gain and no loss**. Existing positive metadata,
reachability classifications and game importance agree exactly with baseline
(positive metadata comparison excludes `work_spent`). The residual cell grant
was 65,847,660 units.

| Fresh training cap | Outcome | Observed added work | Final MAIN / CHECK probability |
| --- | --- | ---: | --- |
| 1,000 | MAIN failed: ESS 1.805, max share 0.670 | 39,663,549 | `3.71e-25` / not run |
| 2,000 | CHECK failed: ESS 1.193; disagreement | 63,126,734 | `6.67e-29` / `4.53e-26` |
| 3,000 | Preflight could not fund training and minimum confirmation | 0 | not run |

No actual operation overruns occurred. These are failed candidate estimates,
not values to publish. Single cold timing samples vary too much for a latency
conclusion; neither funded variant recovered this cell, so neither is recommended.
The rejected variant was removed from the final source. Its measurements and
the archived `data/golaberto-odds-overflow-v1` binary retain the experiment.

```sh
python3 experiments/rare_positions/2026-10-04-target-overflow/scripts/run_overflow.py \
  --baseline experiments/rare_positions/2026-10-04-target-overflow/data/golaberto-odds-baseline \
  --candidate experiments/rare_positions/2026-10-04-target-overflow/data/golaberto-odds-overflow-v1 \
  --output /tmp/r61-family-screen \
  --groups 16498 --seeds 808 \
  --arm off:RUST_ODDS_EXPERIMENT_PILOT_FAMILY=0 \
  --arm late1000:RUST_ODDS_EXPERIMENT_PILOT_FAMILY=1 \
  --arm late2000:RUST_ODDS_EXPERIMENT_PILOT_FAMILY=1,RUST_ODDS_EXPERIMENT_PILOT_FAMILY_TRAINING_DRAWS=2000 \
  --arm late3000:RUST_ODDS_EXPERIMENT_PILOT_FAMILY=1,RUST_ODDS_EXPERIMENT_PILOT_FAMILY_TRAINING_DRAWS=3000
```

## Experiment C: larger complete root forest, offline

Measure the cost of representing all 186 roots with a larger tree leaf budget.
Retain every supported root and stop with an error on setup exhaustion. This
does not implement partial target-prefix grouping. Initial guide/setup costs
and pilot dilution may make this unattractive.

With a 256-root/leaf limit and four million guide values, all 186 roots fit:
256 final leaves, 70 splits, six failed refinements, 3,947,190 guide values and
3,804 setup nodes. Setup took 36.997 ms in the standalone example. Five seeds
with 3,000 final draws per stream all failed independent confirmation.

At ten times the draws and a 16-million guide allowance, all five pairs passed
the example's empirical gates, with MAIN estimates from `4.37e-32` to
`1.51e-31`. However, the broad all-loss root received only 25 pilot draws and
21 MAIN plus 21 CHECK draws per seed, with no hits. Its prior mass is
`2.0019e-8`; it retained no joint-conditioned rivals. Sampling instead concentrated
on branches with small positive pilots. The accepted native seed-2293 estimate
is `8.88e-25`, not a certified golden value, but the seven-order disagreement and
unexplored broad parent are material problems. Zero empirical variance for
zero-hit strata does not quantify their unobserved contribution.

**Reject this allocation**, despite its passing empirical gates at 10x draws.
Standalone setup and sampling times are not full-request latency measurements.
Next test: order construction refinements by the existing marginal rank hint,
keeping the hint strictly an ordering heuristic. Charge its deterministic work;
retain every supported parent or its complete children and unchanged weights.

## Experiment D: rank-aware refinement ordering

The existing `next_team` routine computes independent marginal probabilities
and a rank-cardinality hint. For untrained tree construction, multiply each
leaf's current priority by that hint (positive floor `1e-80`). Cache the chosen
next rival. This controls refinement order only; it supplies neither a probability
estimate nor an impossibility proof. All outcome roots and complete parent/child
partitions retain their original weights.

With the same four-million guide budget and 3,000 draws per final stream, the
all-loss root now received 13 leaves, including jointly conditioned rival sets.
The first standalone screen produced:

| Seed | MAIN probability | CHECK probability | Passed example gates |
| --- | ---: | ---: | --- |
| 808 | `3.762e-25` | `9.808e-25` | yes |
| 1669 | `3.890e-25` | `3.290e-25` | no |
| 1993 | `1.015e-25` | `1.212e-24` | no |
| 2281 | `3.776e-25` | `7.130e-25` | yes |
| 2293 | `1.265e-24` | `6.715e-25` | no |

All five now find the same broad order of magnitude as the existing native
comparison, while empirical gates still reject inadequate batches. Setup took
71.307 ms before marginal caching and cost an additional estimated 41,372,618
hint units. That extra cost needs reduction and full-request measurement before
integration. A builder-local cache keys each marginal by team, orientation,
relative packed target threshold and incident fixture domain masks. It preserves
the marginal result and charges repeated key scans plus DP work only on misses.

Caching preserved MAIN and CHECK results exactly in all five paired standalone
runs and reduced hint work from 41,372,618 to 1,894,024 units. Total setup is
9,890,688 units. Single setup wall samples were noisy (71 vs 79 ms); this result
establishes work reuse, not a measured wall-time speedup.

At 10x final draws, four of five pairs passed; the rejected seed-1993 MAIN was
dominated by one high-weight observation. This retains the reason to require an
independent check rather than accepting every positive candidate.

The production-style profile uses 25 bound pilots per branch, retunes only the
selected message branches and repilots only those branches. It avoids the offline
adaptive example's discarded rank pilots and full-tree repilot. Three message
refinements and 4,000 draws per stream passed in **all five standalone seeds**:

| Seed | MAIN | CHECK | Total modeled work |
| --- | ---: | ---: | ---: |
| 808 | `4.041e-25` | `6.957e-25` | 86,609,079 |
| 1669 | `4.889e-25` | `1.414e-24` | 86,752,947 |
| 1993 | `2.349e-25` | `3.133e-25` | 86,696,646 |
| 2281 | `4.111e-25` | `5.946e-25` | 86,256,801 |
| 2293 | `9.947e-25` | `8.756e-25` | 86,683,583 |

The baseline seed-808 request leaves **65,847,660** confirmation units after its
successful family work (`1,019,219,717 - 953,372,057`). An initial reading of
86,486,095 was wrong: that figure included a rejected experimental grant, rather
than measuring remaining bank capacity. The successful four-thousand-draw profile
does not fit the current residual bank and also needs separate late counting work.
A full-request experiment must charge all setup/counting, size its final pair
from the actual residual grant, and reject any observed overrun. Investigate
late-only refunds of fully measured unused draw reservations from completed branch
searches; preserve all original scheduling before any such refund. Passing the
standalone test alone does not authorize integration or a budget increase.

## Experiment E: late complete tree in the full request

Default-off `RUST_ODDS_EXPERIMENT_TARGET_OVERFLOW_TREE=1` runs after native and
existing family work. It counts complete target paths for remaining non-impossible
zeros and accepts 65–256 roots, keeping a maximum of four cells. Setup uses the
rank-aware ordering and cached marginal calculations above, 256 leaves, four
conditioned rivals and four million guide values. Each supported stratum keeps
a positive draw allocation; failed refinement retains its complete parent.

The existing confirmation bank pays for counting, construction, pilots, message
setup and independent MAIN/CHECK streams. Measured unused **draw** reservations
from completed early branch work may be credited only at this late stage, after
the old scheduling has finished. Setup work is never refunded. Credit is bounded
by the old transfer and original confirmation capacity; any invalid measured
draw audit disables the credit. Observed stage overruns stop the fallback and
prevent publication. Model units are deterministic accounting proxies, not CPU
or elapsed-time guarantees.

All sampling uses fresh per-cell overflow stream namespaces. Pilot/training
observations are excluded from final estimates. Publication retains the existing
ESS, weight concentration, batch stability and independent check gates. Existing
positive estimates remain untouched.

Initial full-request screen: the 4,000-draw pair declined MAIN despite a positive
`5.24e-25` value (maximum contribution share 0.442). The lean no-refund profile
also declined, after fitting down to 3,025 draws. Increasing the bounded nominal
allocation to 6,000 draws recovered the requested cell:

- Raw MAIN `3.2073921144540935e-25`; CHECK `1.2376726230851375e-24`.
- MAIN ESS 8.184, maximum share 0.316; CHECK ESS 4.787.
- Published, after matrix reconciliation: `3.207392222892081e-25` probability.
- Complete 186 roots, 256 leaves; no truncated outcome union.
- Modeled actual work 130,822,198, within the 219,361,856 late free bank.
- Late draw credit 153,514,196; original confirmation capacity was not increased.

The final experiment defaults to 6,000 nominal draws when explicitly enabled;
the actual pair is sized to the residual bank. The independent check is roughly
3.9 times MAIN, consistent with the requested rough scale goal, rather than a
precision estimate or a certified probability interval.

All six seed-808 snapshots preserved existing positive metadata exactly apart
from `work_spent`, and preserved game importance and reachability classifications:

| Group | Baseline zeros | Candidate zeros | Remaining classification |
| --- | ---: | ---: | --- |
| 15902 | 0 | 0 | none |
| 16413 | 0 | 0 | none |
| 16498 | 18 | 17 | 17 proven impossible |
| 16653 | 48 | 48 | 48 proven impossible |
| 16982 | 0 | 0 | none |
| 16983 | 0 | 0 | none |

Group-16498 seed sensitivity: only 808 gained a cell; 1669, 1993, 2281 and 2293
preserved coverage but had insufficient audited residual work. The conservative
audit disables credit if any measured draw phase exceeds its own reservation.
In the first version, some root-count calls exceeded their residual modeled grant.
After the setup optimization, seeds 1669 and 1993 instead reached construction
and exceeded their setup grants; 2281 could not pre-fund the complete pilots.
They stopped without publication. This is a remaining admission limitation;
it does not imply impossibility or publish a partial estimate.

## Exact setup optimizations

For `LazyJoint::with_mode(... roots=0 ...)`, the root keys are truncated to zero
and its root sampling loop is empty. Skip independent-team training PMFs, their
rank-hint DP and the tilted target table in this path. Keep target terminal tables,
goals, guided fallback and all actual proposal construction. Populated native
root training retains its original floating-point operation order.

Move guide span/value calculation inside the existing guide-cache miss branch.
Hits previously recomputed those values and then discarded them. Misses retain
their original order, cost counter, budget limit and guide data.

The measured zero-root bootstrap proxy charges the remaining scans, single target
table and optional goal setup. This changes neither outcome support nor likelihood
weights. Final all-six-group paired requests preserve existing positive metadata,
game importance and reachability exactly. The recovered Palmeiras probability is
unchanged. Its charged new work decreases to **120,572,748** units: counting
2,791,031, tree construction 12,681,719, then pilots/messages/final pair. The
recorded modeled reduction is not itself evidence of a CPU speedup.

## Final paired timing

Four workers, serialized processes, one warm-up rotation and five measured
rotations; arm order rotates each repetition. Measure CLI `estimate` process wall
time and child user+system CPU, including startup and JSON export. These are
paired full-estimator runs, not production-host measurements or HTTP queue times.
Earlier samples were more variable; the final baseline wall range was
1.776–1.810 seconds, with candidate tree range 1.881–2.002 seconds.

| Arm | Mean wall | Mean child CPU | Wall vs baseline | CPU vs baseline |
| --- | ---: | ---: | ---: | ---: |
| Frozen Rust baseline | 1.792 s | 3.822 s | — | — |
| Candidate, tree disabled | 1.852 s | 4.072 s | +3.38% | +6.53% |
| Candidate, tree enabled | 1.966 s | 4.222 s | **+9.75%** | **+10.48%** |

The tree stage median is **101.7 ms**. Comparing enabled to the same candidate
with the tree disabled adds 114 ms mean wall and 151 ms mean CPU. The disabled
candidate also measured slower than the frozen baseline, despite identical
outputs. The exact setup optimizations did not establish an overall performance
win; further profiling is required to distinguish code-generation effects from
remaining overhead. Do not claim that unused modeled reservations provide free
CPU or that the current latency is preserved.

Every measured seed-808 repeat recovered the same unique cell; repeated runs of
one fixed seed are reproducibility checks, not additional independent probability
observations or five distinct gains.

## Validation and reproduction

Final candidate binary SHA256:
`09da95531b96998a5de5da7659729b5bfbe31b36648e6eac95abdf2a71f1dbf8`.
Archived pre-optimization candidate SHA256:
`505101d368f8a41696faa55e493e31764a240a256e937cef02880cf9e9bf0e99`.
Each harness record includes input, executable, export and log hashes, explicit
environment and argv. The harness clears inherited `RUST_ODDS_*` and
`RARE_POSITION_*` settings. Seeds are passed directly, with four workers.

```sh
cargo test --offline --locked --release -j4 --manifest-path odds-rust/Cargo.toml
cargo build --offline --locked --release -j4 --manifest-path odds-rust/Cargo.toml --bin golaberto-odds
python3 -m unittest discover \
  -s experiments/rare_positions/2026-10-04-target-overflow/scripts -p test_run_overflow.py

python3 experiments/rare_positions/2026-10-04-target-overflow/scripts/run_overflow.py \
  --baseline experiments/rare_positions/2026-10-04-target-overflow/data/golaberto-odds-baseline \
  --candidate odds-rust/target/release/golaberto-odds --output /tmp/r61-final-six \
  --groups 15902,16413,16498,16653,16982,16983 --seeds 808 \
  --arm off:RUST_ODDS_EXPERIMENT_TARGET_OVERFLOW_TREE=0 \
  --arm tree:RUST_ODDS_EXPERIMENT_TARGET_OVERFLOW_TREE=1

python3 experiments/rare_positions/2026-10-04-target-overflow/scripts/run_overflow.py \
  --baseline experiments/rare_positions/2026-10-04-target-overflow/data/golaberto-odds-baseline \
  --candidate odds-rust/target/release/golaberto-odds --output /tmp/r61-final-extra-seeds \
  --groups 16498 --seeds 1669,1993,2281,2293 \
  --arm tree:RUST_ODDS_EXPERIMENT_TARGET_OVERFLOW_TREE=1

python3 experiments/rare_positions/2026-10-04-target-overflow/scripts/run_overflow.py \
  --baseline experiments/rare_positions/2026-10-04-target-overflow/data/golaberto-odds-baseline \
  --candidate odds-rust/target/release/golaberto-odds --output /tmp/r61-final-timing \
  --groups 16498 --seeds 808 --warmups 1 --repeats 5 \
  --arm off:RUST_ODDS_EXPERIMENT_TARGET_OVERFLOW_TREE=0 \
  --arm tree:RUST_ODDS_EXPERIMENT_TARGET_OVERFLOW_TREE=1
```

Final Rust suite: **173 passed, 4 database tests ignored**, no failures. HTTP
tests required local socket permission; the initial sandbox run failed to bind,
and the final suite with socket access passed. No database writes were performed.
Harness tests: six passed. `rustfmt` and `git diff --check` pass. Independent Sol
review found no remaining correctness issue after fixing RNG namespace reuse and
the charged-handoff refund. Relevant existing exhaustive tests check full branch
partition mass, actual rank sorting, conditional weights and retained support
after setup/refinement exhaustion.

Evidence directories under `2026-10-04-target-overflow/data/`:
`full-screen808-v1`, `full-sixk-five-seeds-v1`, `full-six-groups-v1`, `warm808-v1`,
`final-six-groups-v2`, `final-four-extra-seeds-v2`, `final-warm808-v2`.

## Implementation references

- `odds-rust/src/joint_caps/propagated/lazy/strata/tree.rs`: complete target roots,
  rank-aware ordering, bounded marginal cache, retained parent partitions and
  explicit measured overflow constructor.
- `odds-rust/src/rare_tail/branches.rs`: measured draw reservation audit, including
  discarded pilots and already released check/pilot work.
- `odds-rust/src/rare_tail/confirmations.rs`: late-only bank credit and dispatch.
- `odds-rust/src/rare_tail/overflow_trees.rs`: complete pilots, fixed allocations,
  fresh MAIN/CHECK streams, gates, settlement and diagnostics.
- `odds-rust/src/joint_caps/propagated/lazy.rs`: zero-root and guide-cache setup
  optimizations.

## Results and recommendation

**The experiment fixes Palmeiras/14th at seed 808**, generically, with no lost
positive cells across six snapshots and the extra seed comparisons. It leaves
only proven-impossible zeros in all six default-seed matrices.

**Keep it default-off.** It does not meet the current full-request latency
constraint. Opt-in for experimentation is
`RUST_ODDS_EXPERIMENT_TARGET_OVERFLOW_TREE=1`; no other new flags are required.
The ordinary production estimator remains the baseline. No commit or push.

Next work should preserve the complete support but fund it through actual savings:
profile the disabled-candidate overhead, reuse canonical root/domain setup, build
marginal-cache keys before allocating normalized games, and avoid constructing a
tree when its complete pilot/final grant cannot fit. Root grouping or certified
tiny-branch allocation can reduce the 186-root sampling floor, but require new
weight/support tests rather than truncation. Independently confirmed rough scale
estimates remain the goal; this experiment supplies neither exact probabilities
nor stronger impossibility proofs.
