# Native Rust discrete win/draw floor constraint

## Scope

This follows the [Palmeiras/17th analysis](2026-09-30-palmeiras-17.md) and adds
its missing integer win/draw condition to the current Rust estimator. The
baseline is the unified Rust service at `4333d31b`, with its existing default
goal completion enabled. These measurements use four estimator workers and
identical frozen requests; older Go timings supply no additional budget.

The implementation discovers contradictory coalitions generically. There is
no team ID, competition ID, or requested-rank lookup in the production cut.
The named Palmeiras regression is confined to tests and the offline certificate.

## Necessary inequality

For any coalition S whose members must each finish on at least F points, let:

- C be their total current points, including adjustments.
- M be the number of remaining fixtures incident to S, counting an internal
  fixture once.
- Q be the sum of `ceil(max(0, F - current_points[t]) / 3)` over its members.
- D be the number of internal fixtures ending in draws.
- `slack = C + 3*M - F*|S|`.

For each team, `3*wins + draws >= F - current_points` implies its no-draw
win requirement is at most `wins + draws`. Summing counts internal draws
twice. Every incident fixture supplies at most one coalition win; an internal
or boundary draw supplies no win. Therefore `Q <= M + D`: boundary draws
cancel out of this necessary inequality.

An internal draw also removes one point from the maximum coalition points
total. Boundary draws remove two, and boundary losses remove three. Thus
`D <= slack`. A necessary condition is consequently:

```text
Q - M <= slack
```

For the eleven-team Palmeiras certificate, `Q=87`, `M=83`, and `slack=2`.
It needs at least four internal draws but can afford at most two.

## Finding the coalition without enumerating subsets

Rearranging the violation gives:

```text
V(S) = sum(F - current[t] + ceil(max(0, F-current[t])/3)) - 4*M
```

For each fixture between two constrained teams, subtract two from each
endpoint's unary reward and add a capacity-two undirected graph edge. For a
fixture against an exempt team, subtract four from the constrained endpoint.
Positive rewards connect from the source; negative rewards connect to the
sink. Then `max V(S) = sum(positive rewards) - mincut`.

A positive maximum proves that some coalition is impossible. A zero or
negative maximum establishes no feasibility or reachability claim. A cheap
zero-positive-reward gate skips graph construction when no violation is
possible. The final graph uses fixed storage for at most 64 teams plus source
and sink, with iterative breadth-first augmenting paths.

## Integration and correctness limits

The cut runs after ordinary propagation reaches a fixed point in the existing
early joint floor proof. It uses unrestricted 3/1/0 outcomes and is disabled
for other point systems or bonus-point rules. Domain restrictions can only
make that unrestricted relaxation harder; ignoring them in the cut is safe.
Later wins, goal difference, goals scored and other tiebreakers are relaxed in
rivals' favor. No numerical score cap is used to prove impossibility.

The 500-node cell and 10,000-node direction ceilings remain unchanged.
Constructive witnesses remain at 100 nodes per cell / 3,200 total, and the
default neighborhood candidate allowance remains 10,000. Existing legacy
recycling eligibility prevents an additional impossibility proof from adding
probability draws. Exhausted budgets still return undecided.

The cut is **enabled by default**, following the user's authorization after
reviewing the CPU and stage costs. No extra flag is required;
`RUST_ODDS_DISCRETE_CUTS=0` disables it. The benchmark results below retain their
original configurations and do not establish universal latency non-increase. The existing
`RUST_ODDS_AGGREGATE_CUTS=0` also skips this strengthened early propagation.
Probability code and worker scheduling are unchanged. Aggregate-cut matrix
and team buffers are reused across propagation rounds to avoid repeated setup
allocations; equations, order and stopping conditions are unchanged.

## Paired experiments

The final result is one additional impossible cell in the frozen group 16498
request: Palmeiras (16), 17th. Its worst reachable rank remains 16th.
Group 16653 and group 16982 gain no extra proofs from this inequality on the
reference snapshots. This is a proof improvement, not an additional nonzero
probability estimate.

The measurements below compare full requests, including decoding and response
writing, against the current Rust service. Only one request is active at a
time. Paired execution alternates which binary runs first. The persistent-server
comparison uses two warmups and eight timed requests per input at seed 808;
CPU measurements include both warmups and the timed requests. The separate
six-seed comparison uses a fresh service for each request and measures listener
startup separately. No other benchmark, build or test runs concurrently.

### Failed performance alternatives

Several graph layouts and probability scheduling alternatives were tested.
These are recorded to avoid treating a favorable timing run as a universal
speedup. All preserved probabilities in their paired runs, but the following
were rejected for inconsistent or increased full-request costs:

- Heap Dinic graph: the six-seed aggregate request time increased 2.97%.
- Marking the graph routine cold: the seed-808 sum of medians increased 0.35%,
  with current 16653 increasing 1.54%.
- Fixed-storage Dinic graph: the sum of medians increased 6.35%.
- Reusing proposal CDF rows: sum of medians increased 2.33%; current 16653
  increased 7.99%. This sampler edit was reverted.
- Static confirmation cost ordering: the persistent run improved 5.89%, but
  the six-seed run increased 0.87%, including 2.41% and 3.35% increases for
  the two 16653 snapshots. This scheduling edit was reverted.
- Pilot-time confirmation ordering: persistent median totals improved 0.78%,
  but current 16653 increased 4.46%; the six-seed aggregate increased 0.66%
  and CPU time increased 2.72%. This scheduling edit was reverted.

Sampler-stage timing varied much more than the graph routine itself, despite
identical samples and estimates. The experiments do not establish which
combination of code layout, allocation and worker scheduling caused that
variation.

The final implementation retains the simpler fixed-storage breadth-first
augmenting-path graph and reuses existing aggregate propagation buffers.
There are no probability-sampler or worker-scheduling changes.

### Final comparison

The benchmark executable used the explicit opt-in flag. Its hashes, per-pair
counts, timings, CPU data, setup cost and earlier trials are saved in
[`results/2026-09-30-rust-discrete-cut.json`](results/2026-09-30-rust-discrete-cut.json).

Persistent server, seed 808, eight timed pairs per input (milliseconds):

| Snapshot | Baseline full request | Candidate full request | Change |
| --- | ---: | ---: | ---: |
| `group-16498-44eabb47.json` | 744.114 | 680.349 | -8.57% |
| `group-16653-2d1c1d6f.json` | 665.770 | 662.872 | -0.44% |
| `group-16653-71d4fea8.json` | 688.031 | 678.781 | -1.34% |
| `group-16982-9327edcd.json` | 310.172 | 307.914 | -0.73% |
| `group-16983-43969b02.json` | 301.227 | 304.813 | +1.19% |

The sum of these medians improved **2.75%**. The dense 16983 control increased
**1.19%**. These are measured paired differences, not a guarantee that every
request becomes faster. Across six fresh-server seeds per input, total client
request time improved **0.83%**, and CPU time improved **1.86%**. Some
individual seed pairs were slower, including 16498/911 at +7.9%.

Persistent early-proof stage, including native graph setup (milliseconds):

| Snapshot | Baseline median | Candidate median | Added time |
| --- | ---: | ---: | ---: |
| `group-16498-44eabb47.json` | 0.4085 | 0.6400 | +0.2315 |
| `group-16653-2d1c1d6f.json` | 0.1940 | 0.2365 | +0.0425 |
| `group-16653-71d4fea8.json` | 0.3070 | 0.3540 | +0.0470 |
| `group-16982-9327edcd.json` | 0.0070 | 0.0080 | +0.0010 |
| `group-16983-43969b02.json` | 0.0085 | 0.0110 | +0.0025 |

The graph's practical added proof cost is about **0.05–0.23 ms** on the three
20-team snapshots. No external solver/model process is started. Existing
estimator model setup in the six-seed run averages 0.67–2.67 ms; per-input
baseline/candidate values are retained in the JSON artifact.

CPU seconds over two warmups plus eight requests, and service peak RSS in MiB:

| Snapshot | CPU baseline / candidate | RSS baseline / candidate |
| --- | ---: | ---: |
| `group-16498-44eabb47.json` | 20.74 / 18.22 | 160.67 / 168.22 |
| `group-16653-2d1c1d6f.json` | 14.81 / 15.04 | 164.45 / 161.09 |
| `group-16653-71d4fea8.json` | 19.89 / 19.56 | 163.92 / 169.86 |
| `group-16982-9327edcd.json` | 6.59 / 6.54 | 24.28 / 24.28 |
| `group-16983-43969b02.json` | 6.38 / 6.42 | 23.27 / 23.23 |

Total persistent CPU time improved **3.84%** in this final run. Four estimator
workers remain the ceiling; no additional sampling, candidate or proof-node
allowance was granted. In the six-seed run, total CPU divided by request wall
time corresponds to approximately 2.43 / 2.40 cores of work (including the
small service startup CPU cost), below the four-worker ceiling.

### Coverage and remaining undecideds

- Six seeds × five requests × 400 cells = **12,000 cell-runs**. Exactly six
  change label: the same Palmeiras/17th cell on each seed. This is **one unique
  cell**, not six different positions.
- Additional reachable cells: **0**. Additional impossible cell-runs: **6**.
- Changed probability cells: **0**. Every seed-808 persistent pair also asserts
  exact equality of uncertainties, displayed team-odds matrix and game importance.
- Lost nonzero estimates, lost reachability labels, proof conflicts: **0**.
- Resolved cell-runs rise **11,987 → 11,993**. In the final six-seed timing run,
  that is approximately **706 → 712 resolved cell-runs per second**. Most of
  this throughput difference is timing variation, not new coverage.
- Frozen 16498 retains one undecided cell on each seed: Flamengo (17), 14th.
  Its reachable cells without estimates are unchanged. The earlier 16653
  snapshot retains Londrina (95), 5th on seed 818; the current 16653 snapshot
  and the 16982/16983 controls have no remaining undecideds in these runs.

### Recommendation

The mathematical constraint is useful and sound. It was initially kept opt-in
because robust latency non-increase had not been established. The final
aggregate measurements are favorable, but the persistent control increase and
larger regressions in earlier builds remain part of the evidence. There is no
assumed one-second or 20% allowance.

The user subsequently authorized **enabling the discrete proof by default**
after reviewing the CPU and stage-cost breakdown. The node limits, probability
sampling and four-worker ceiling remain unchanged. Palmeiras/17th now becomes
`impossible_by_joint_points` without extra flags. `RUST_ODDS_DISCRETE_CUTS=0`
restores its prior `undecided` label on this snapshot. Source is uncommitted and
has not been pushed or deployed. The release binary was rebuilt after enabling
the default. A seed-808 replay verifies both the enabled default and the `0`
override, preserves the 16th-place witness, and leaves team odds and game
importance identical. All 39 library/estimator tests pass with the enabled
default.

## Tests and reproducible commands

Rust: **45 full-suite tests passed**, two optional DB tests ignored, plus two
offline-certificate tests passed. The final opt-in/default gate was followed
by all 39 library/estimator tests passing. Rails: **120 tests / 349 assertions
passed**, one skip. Formatting and `git diff --check` pass.

New tests cover every coalition in a five-team graph across all exemption
masks and varied floors; every W/D/L season of a small league with point
adjustments and restricted domains; unsupported scoring/bonus rules; both
Palmeiras exemption cases in reversed team/fixture order; the production-sorted
15th-place complete certificate; and the full bounded 17th-place proof while
preserving the feasibility of the 15th/16th relaxations.

Build and test with Rust/Cargo 1.93.1 on macOS arm64 (the declared minimum Rust
version remains unchanged):

```bash
cargo test --offline --locked --manifest-path odds-rust/Cargo.toml
cargo test --offline --locked --manifest-path odds-rust/Cargo.toml --example point_floor_certificate
cargo fmt --manifest-path odds-rust/Cargo.toml --check
cargo build --release --offline --locked --manifest-path odds-rust/Cargo.toml
bin/rails test
```

Rebuild the baseline in a separate directory, without changing the checkout:

```bash
mkdir -p /private/tmp/golaberto-discrete-baseline
git archive 4333d31b odds-rust stats/core | tar -x -C /private/tmp/golaberto-discrete-baseline
cargo build --release --offline --locked --manifest-path /private/tmp/golaberto-discrete-baseline/odds-rust/Cargo.toml
```

The recorded baseline binary was built at that commit before source edits;
compiler, machine and allocation effects can change exact timings on rebuild.

Persistent and six-seed request comparisons, run sequentially:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 experiments/rare_positions/benchmark_discrete_cut.py \
  --baseline /private/tmp/golaberto-discrete-baseline/odds-rust/target/release/golaberto-odds \
  --candidate odds-rust/target/release/golaberto-odds \
  --iterations 8 --output /private/tmp/discrete-http

PYTHONDONTWRITEBYTECODE=1 python3 experiments/rare_positions/compare_rust_reachability.py \
  --baseline /private/tmp/golaberto-discrete-baseline/odds-rust/target/release/golaberto-odds \
  --candidate odds-rust/target/release/golaberto-odds \
  --reference experiments/rare_positions/reference/2026-09-30-hundredfold/reference.json \
  --output /private/tmp/discrete-paired --variants discrete --baseline-variant defaults \
  --seeds 801,804,808,817,818,911 --http --no-trace
```

The persistent harness explicitly enables the candidate cut; `discrete` in the
six-seed harness enables only this flag on top of existing Rust defaults.
Both clear inherited rare-position settings. The snapshots are unchanged;
no application database writes or external solver installations were performed.
The Rails suite uses its ordinary test database.

To run the enhanced estimator once using the enabled default:

```bash
odds-rust/target/release/golaberto-odds estimate \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  /private/tmp/discrete-16498.json 808 4
```

## Sources and implementation

The inequality above is derived directly from the standard 3/1/0 fixture
model, not from an arbitrary score bound or a feasible LP relaxation. Evidence
comes from the unchanged reference request, the production `Model` and sorter,
and exhaustive tests:

- [Native propagation and discrete graph cut](../../odds-rust/src/proof.rs).
- [Model points, scoring and production sorting](../../odds-rust/src/model.rs).
- [Estimator regression tests](../../odds-rust/tests/estimator.rs).
- [Offline coalition certificate checker](../../odds-rust/examples/point_floor_certificate.rs).
- [Original Palmeiras certificate](2026-09-30-palmeiras-17.md).
- [Prior Rust reachability research and scientific sources](2026-09-30-rust-reachability.md).
