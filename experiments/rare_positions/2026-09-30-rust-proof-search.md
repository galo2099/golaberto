# Explained rank proof search in a new worktree

Date: 2026-09-30. Branch: `codex/proof-search`.
Worktree: `/Users/robsonaraujo/.codex/worktrees/proof-search/golaberto`.
The experiment was initially run without committing, pushing, mutating the
database, or modifying the original checkout. The worktree starts from the current checkout and its
uncommitted Rust changes, including enabled discrete cuts and the disabled
sampler-domain-probe experiments.

## Result and recommendation

The generic search proves **Flamengo (17) / 14th impossible** in frozen group
16498 using **367 fixture-search nodes**, versus 2,613 in the earlier offline
Rust diagnostic. This is approximately **86% fewer nodes**. The new isolated
proof takes approximately **1.1 ms**, rather than the earlier 7.4 ms diagnostic.
Those are offline proof timings, not the baseline production request cost.

The proof now fits inside the existing early-proof cell quota, without increasing
node limits, draws, neighborhood candidates, or the four-worker ceiling.

Across five snapshots and six seeds, it adds **one distinct impossibility proof**
(six cell-runs). Displayed odds, all estimate probabilities and standard errors,
and game-importance values are **exactly identical** to the frozen Rust baseline.
There are no estimate or reachability regressions and no added probability
estimates. Verified reachable zero cells remain available to the samplers.

The user subsequently authorized enabling and merging this change into `master`.
The proof is **enabled by default**; `RUST_ODDS_RANK_PROOF=0` disables it.
Coverage is a clear gain; full-request timing changes are small relative to host
variation, and the proof stage itself is measurably more expensive. Do not
advertise an overall estimator speedup. No search quotas or worker limits increase.

## Frozen baseline and protocol

- Base commit: `c1a6d3ef94dbb8e06a5735ef2d496bdaa5ad03cb`, plus the saved
  [baseline patch](results/2026-09-30-rust-proof-search/baseline.patch).
- Frozen Rust baseline executable SHA256:
  `95e61480f1870172501a398d5b446292ecf6c6e365756a34eb6c6e5e73539cf2`.
- Final candidate executable SHA256:
  `a09f6da7d275f8c59f971fea0031e5c7867bde73f3f3bc1219ff580b30cc3f7a`.
- Snapshots: 16498 `44eabb47`, earlier 16653 `2d1c1d6f`, current 16653
  `71d4fea8`, 16982 `9327edcd`, and 16983 `43969b02`.
- Identical requests; fixed seeds 801, 804, 808, 817, 818, 911.
- Two cold-server experiments with 30 paired HTTP requests each: broad
  allocation, then the final filtered allocation and propagation optimization.
- Final persistent-server comparison: seed 808, two warmups and eight timed
  request pairs per snapshot. Baseline/candidate order alternates.
- One active calculation at a time, four estimator workers total. The proof
  is serial and adds no worker. No builds or tests ran during timed comparisons.
- Setup, 20,000-season importance scout, 100,000-season pool, all remaining
  samplers/searches, response encoding and HTTP response are included.
- Rust/Cargo 1.93.1, release LTO. No Go timings or one-second allowance are used.

Machine activity, thermal state, allocation layout and cache effects can affect
these small changes. Untouched control requests also vary. Process CPU is summed
user/system time; stage timings are wall times, not CPU counters.

## Search changes

`odds-rust/src/rank_proof.rs` models one shared W/D/L outcome per fixture.
Final points and applicable wins use one lexicographic integer total. Wins are
packed only if they immediately follow points in the phase ordering. Later
criteria are deliberately relaxed; equality is generously allowed to satisfy a
required rival ordering.

For a worst-rank bound, all target fixtures are fixed to its minimum P/W results,
and every possible set of `r-1` rivals required at or above that floor is examined.
Changing a target result to a loss can only lower its P/W and raise the opponent's
P/W. Thus any actual rank-r season implies an assignment in this relaxation.
Negating the packed ordering gives the corresponding best-rank necessary bound.
This is applicable to every rank; it does not infer reachable intermediate ranks.

1. **Explained propagation.** Each removed outcome carries the fixture decisions
   and required-team constraints responsible for removing it. Only restrictions
   that lower an endpoint's maximum contribute to that bound explanation.
2. **Backjumping.** A contradiction independent of the latest fixture decision
   skips the remaining alternatives at that decision. When all allowed values
   fail, their reasons are combined, including reasons for already excluded
   values of the parent domain.
3. **Coupled fixture ordering.** Prefer smaller domains, then fixtures between
   two constrained rivals, then the smaller combined endpoint slack. This reduces
   exploration of easy games involving an unconstrained opponent.
4. **Conflicting-cohort reuse.** An exhaustive root proof produces a smaller set
   of required rivals that is already inconsistent. Any later cohort containing
   that set is refuted without replaying fixture DFS. This cache is scoped to
   one target/floor query; it is not reused across unrelated teams or thresholds.
5. **Propagation overhead.** Reuse initial maxima and update changed domains;
   singleton support and fixtures outside the required cohort are skipped.
   Games with both endpoints unconstrained are never branched on.

This is a small native proof engine, not CP-SAT or SCIP. Explanation-based
propagation and learning are established techniques; see Perron, Didier and Gay,
[The CP-SAT-LP Solver](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.CP.2023.3).
The measured claims here come from this implementation's own tests and requests.

## Allocation experiments

The first integration attempted fixture proofs after every unresolved point-cohort
query. It resolved the desired cell but wasted work on easy consistent queries.
For 16498 its median early-proof stage grew from 0.632 to 3.286 ms. Earlier/current
16653 grew from 0.253/0.360 to 0.949/1.535 ms, with no added coverage there.
The broad experiment's median paired HTTP change was +0.13%; sum of HTTP times
changed -0.27%, and summed process CPU -0.75%. These variations do not establish
that the broad allocation is free. Reject that allocation.

The final integration prioritizes queries where the original point proof visited
more than one cohort-search node, indicating an encountered conflict. It also
requires an individually tight fixture outcome among eligible rivals before
spending fixture-branch work. These are workload gates; a skipped query remains
undecided. They are not team- or position-specific exceptions.

The fixture search uses only the **remaining** cell quota after the existing point
proof. Raw proof nodes, new fixture nodes and legacy recycling checks share the
same 500-node cell / 10,000-node enabled-direction limits. New proofs grant no
additional probability draws. Resolving Flamengo early removes its impossible
cell from downstream candidate work; measured modeled search work at seed 808
falls from 93,676,800 to 93,406,200 fixture-work units. Proof nodes are not included
in that work metric and are reported separately.

A final traced request used 681 nodes for the entire early floor direction and
35 for the cap direction, within the unchanged ceilings. The new Flamengo query
used 367 nodes, 18 backjumps, and 93 conflicting-cohort cache hits across 105 sets.

## Isolated solver ablation

Nine sequential runs per variant; median search wall time. These compare variants
of the new engine at a generous offline 5,000-node quota so each can finish.
The production integration continues to share the existing 500-node quota.

| Variant | Nodes | Cohorts reused | Backjumps | Search ms |
|---|---:|---:|---:|---:|
| Plain fixture DFS, no aggregate cuts | 2,703 | 0 | 0 | 6.013 |
| Explained backjumping | 1,024 | 0 | 302 | 2.211 |
| Backjumping + coupled ordering | 496 | 0 | 18 | 1.411 |
| Above + conflicting-cohort reuse | 367 | 93 | 18 | 1.149 |
| Above + production pressure gate | 367 | 93 | 18 | 1.099 |

The earlier 2,613-node diagnostic used production aggregate cuts; that explains
its different node count from the 2,703-node plain ablation. No claim is made that
367 nodes are a theoretical minimum. Model setup is approximately 0.010 ms,
excluding ordinary request decoding and `Model` construction.

## Final coverage

The sole new classification is Flamengo/14th in 16498, from `undecided` to
`impossible_by_joint_rank`, on all six seeds. It remains a zero probability.
No nonzero cells were added or lost. No probability or standard error changed.

At seed 808:

| Snapshot | Nonzero | Impossible zero, before → after | Reachable zero | Undecided, before → after |
|---|---:|---:|---:|---:|
| 16498 | 362 | 16 → 17 | 21 | 1 → 0 |
| Earlier 16653 | 344 | 43 → 43 | 13 | 0 → 0 |
| Current 16653 | 338 | 48 → 48 | 14 | 0 → 0 |
| 16982 | 400 | 0 → 0 | 0 | 0 → 0 |
| 16983 | 400 | 0 → 0 | 0 | 0 → 0 |

Earlier 16653 retains team 95 / 5th as undecided at seed 818. That remains the
only undecided cell-run across these final 30 requests. A feasible relaxed P/W
assignment does not resolve an actual tied rank.

## Final latency and CPU

Across the final 30 cold HTTP pairs:

- Median paired HTTP change: **-0.55%**; estimator calculation change: -0.51%.
- Change in sum of full HTTP times: **-0.91%**.
- Change in summed process CPU: **-0.77%**.
- Individual HTTP pairs range from approximately **-10.3% to +12.0%**.
  Latency preservation for every request is not established by this sample.

Persistent HTTP results, eight timed pairs per row after two warmups:

| Snapshot | Baseline median ms | Candidate median ms | Change | Early-proof stage ms, before → after | Process CPU change |
|---|---:|---:|---:|---:|---:|
| 16498 | 685.44 | 683.90 | -0.22% | 0.665 → 2.067 | **+0.72%** |
| Earlier 16653 | 676.25 | 671.41 | -0.72% | 0.252 → 0.429 | 0.00% |
| Current 16653 | 722.28 | 673.47 | -6.76% | 0.362 → 0.679 | -7.47% |
| 16982 control | 307.51 | 303.31 | -1.37% | 0.010 → 0.010 | -0.91% |
| 16983 control | 290.31 | 296.35 | **+2.08%** | 0.007 → 0.008 | **+0.16%** |

Stage medians above exclude warmups. Process CPU includes startup and all ten
requests per server. The sum of persistent medians changed -1.99%; summed CPU
changed -2.20%. Current 16653's large improvement occurs without a new proof and
should not be attributed to the proof algorithm. The proof-stage increases of
1.40/0.18/0.32 ms in the first three rows are real measured costs; the full-request
changes include avoided work and wider measurement variation.

At the observed approximately 1.1 ms proof cost, coverage yield is one additional
impossible cell per completed relevant query. There is no increase in estimated
reachable cells per unit of time. The strongest finding is the node reduction
that makes this proof fit the current ceilings, with unchanged estimate quality.

## Correctness, tests and limits

- Relaxation infeasibility alone proves impossibility. Feasible leaves are never
  labeled reachable or converted into probability estimates.
- Points/wins equality counts as favorable; arbitrary legal goals remain allowed.
  No failed goal assignment, score cap or numerical PMF truncation refutes a pattern.
- Every rival in the required set must use the same fixture assignment. Games
  cannot contribute wins simultaneously to both endpoints.
- Team-conflict caches are valid only for the current target, floor and initial
  target fixture restrictions. Superset reuse is sound within that query.
- Budget exhaustion, skipped workload gates, more than 512 candidate cohorts,
  unsupported scoring/bonus rules, and depth 128 remain unproved. These are search
  limits, not impossibility assumptions. Only up to 64 fixture participants are
  supported by this proof engine.
- Both directions and optimized/plain search match exhaustive P/W relaxation
  enumeration on small leagues, including negative point adjustments, played
  wins, and phases with wins after goal criteria (points-only proof there).
- Twenty additional five-team leagues enumerate all 729 W/D/L seasons each,
  checking every team and required-rival count in both directions, all optimizations
  and the conservative pressure gate.
- **Full Rust suite: 51 passed, 2 optional database tests ignored.** No application
  database is needed. Existing HTTP endpoints, sorter, sampler, acceptance-gate
  and discrete-proof tests pass. Rails/JavaScript code is unchanged by this task.

## Reproduce

All paths below are relative to this worktree. Build the frozen baseline in a
separate directory to avoid overwriting the candidate:

```sh
git worktree add --detach /tmp/odds-proof-baseline c1a6d3ef94dbb8e06a5735ef2d496bdaa5ad03cb
git -C /tmp/odds-proof-baseline apply "$PWD/experiments/rare_positions/results/2026-09-30-rust-proof-search/baseline.patch"
cargo build --release --offline --locked --manifest-path /tmp/odds-proof-baseline/odds-rust/Cargo.toml -j 4
cargo build --release --offline --locked --manifest-path odds-rust/Cargo.toml -j 4
cargo build --release --offline --locked --manifest-path odds-rust/Cargo.toml --example rank_proof_search -j 4
cargo test --offline --locked --manifest-path odds-rust/Cargo.toml -j 4 -- --test-threads=1

odds-rust/target/release/examples/rank_proof_search \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  17 14 500 learn

RUST_ODDS_RANK_PROOF=1 odds-rust/target/release/golaberto-odds estimate \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  /tmp/improved-odds.json 808 4

python3 experiments/rare_positions/compare_rust_reachability.py \
  --baseline /tmp/odds-proof-baseline/odds-rust/target/release/golaberto-odds \
  --candidate odds-rust/target/release/golaberto-odds \
  --reference experiments/rare_positions/reference/2026-09-30-hundredfold/reference.json \
  --output /tmp/proof-search-cold --baseline-variant defaults --variants rank_proof \
  --seeds 801,804,808,817,818,911 --http --no-trace

python3 experiments/rare_positions/benchmark_rank_proof.py \
  --baseline /tmp/odds-proof-baseline/odds-rust/target/release/golaberto-odds \
  --candidate odds-rust/target/release/golaberto-odds \
  --iterations 8 --output /tmp/proof-search-persistent
```

Use `plain`, `backjump`, `order`, `learn`, or `budgeted` in the diagnostic for
ablations; append `best` to select the negated direction. The production toggle
applies both directions. The benchmark helpers clear unrelated estimator flags.

Recorded commands used `CARGO_TARGET_DIR=/private/tmp/golaberto-proof-search-target`
for the candidate and the frozen binary `/private/tmp/golaberto-proof-search-baseline`.
The SHA256 values and per-query nodes, paired stage times, CPU, RSS, input hashes,
quality comparisons and coverage deltas are in
[results.json](results/2026-09-30-rust-proof-search/results.json).

## Default enablement validation

After user authorization, the default was changed to enabled (unset or `1`);
`RUST_ODDS_RANK_PROOF=0` disables it. The earlier paired results describe the
opt-in build; the following checks cover the final default-enabled build.

A first rebuild using the shared `search::enabled` helper showed repeatable
full-request increases of about 6–9% on the dense snapshots. Most of the
extra time appeared in point-tilt sampling, rather than in the proof stage.
That build was rejected. The final implementation uses a local environment
match for default enablement; this does not establish why binary performance
changed.

Final build SHA256: `9e99c789db2f6084f52ed3582b1c655e43301948be558fac0ed244cba33dbda5`.
The final binary was compared with itself, proof disabled versus default enabled,
using four workers, seed 808, two warmups and four timed pairs per snapshot:

| Snapshot | Disabled median ms | Enabled median ms | Wall change | CPU change |
|---|---:|---:|---:|---:|
| group-16498-44eabb47.json | 691.94 | 689.46 | -0.36% | +1.10% |
| group-16653-2d1c1d6f.json | 668.78 | 675.96 | +1.07% | -1.96% |
| group-16653-71d4fea8.json | 678.61 | 709.59 | +4.56% | +1.24% |
| group-16982-9327edcd.json | 321.67 | 312.98 | -2.70% | +0.25% |
| group-16983-43969b02.json | 310.94 | 308.53 | -0.78% | -1.74% |

The current 16653 snapshot therefore still shows a **4.56% wall-time increase**
in this check; latency non-increase is not established. Its early-proof stage
adds roughly 0.3 ms, while other stage times vary more. The final frozen-baseline
subset check showed −0.86% for 16498 and +3.46% for current 16653. These increases
are reported, not treated as an additional work allowance. All original search
quotas and the four-worker ceiling remain in place.

All five snapshots retained exactly identical probabilities, standard errors and
game-importance values, with the additional Flamengo/14th impossibility proof
and no lost reachability. The full Rust suite passed with default enablement
(51 tests passed; two optional database tests ignored). Raw validation results
are included in `results.json`. The final persistent harness explicitly disables
the proof for its baseline and leaves the candidate flag unset.

Only the rank-proof changes, focused tests, harness and report are included in
this commit. Earlier discrete-proof and sampler-domain experiments inherited
from the checkout remain separate uncommitted work. The frozen baseline patch
is retained as historical experiment data.

The exact staged source was also exported and tested independently of inherited
uncommitted changes: 44 tests passed, with the same two optional DB tests ignored.
