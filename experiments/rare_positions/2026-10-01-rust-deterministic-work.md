# R37: deterministic proposal work budgets

## Decision

Enable deterministic operation budgets by default in the current Rust estimator.
The selected two-wave confirmation allocator gained **7 nonzero cell-runs across
4 distinct cells**, with **0 lost estimates or proof regressions**, in 98 paired
full requests. CPU increased **1.62%** and complete HTTP latency **2.27%**.
The user explicitly accepted the approximately 2.3% latency increase. This is an
aggregate increase; group 16498 increased **6.36%**. Production was not measured. The experiment was uncommitted at the time of
measurement; the user authorized integration into master on 2026-10-02. Rebuild
and restart the server to deploy the new defaults.

Fixed seeds alone were insufficient: the frozen baseline produced different
complete response bytes in 3 of 14 repeated HTTP pairs, despite identical input,
seed and worker count. The additional coverage stages previously admitted work
using elapsed time, including time affected by logging and worker scheduling.

## Deterministic cost model

```
estimated work = setup units + draws × estimated cost per draw
setup units = 16 × enumerated nodes + 2 × guide values
estimated cost per draw = ceil(pilot operation units / pilot draws)
pilot operation units = 8 × fixture operations + guidance + ranking allowance
reference cost = 64 × remaining fixtures + 32 × teams + 1
```

Counters follow active joint, guided and omitted fixtures, domain checks, actual
optimized cardinality DP support, and mixture-density replays. Early exits are
counted. Ranking allows for conditional score generation, campaign updates, and
comparison sorting using the phase's number of keys. A common reference cost
converts each stage's quota to units; it does not force all proposals to have the
same draw cost. No elapsed time enters the default admission or retry decisions.

These coefficients are calibrated scheduling proxies, **not CPU cycles or hard
wall deadlines**. Successful setup and every completed training pilot are charged
once, including discarded alternatives. Failed constructors remain bounded by
existing node/guide limits and deterministic cell prefixes; their internal failed
setup work is not fully metered. Training precedes final admission, so unusually
expensive training can exhaust or overrun its quota; finals then receive no
remaining units. Summary logs expose nominal limits and reservation totals.
No setup overrun occurred in the selected 98-request cohort (largest individual
setup/pilot share was 68.2%). Costs inferred from pilots can also mispredict final
execution costs. Actual timing remains diagnostic.

| Stage | Default reference draws | Policy |
| --- | ---: | --- |
| Shared setup/pilots/finals | 22,500 | One quota; up to 8 group pilots |
| Individual setup/pilots/finals/extensions | 90,000 | 28-cell training prefix; one final quota |
| Additional independent confirmations | 200,000 | Reuse training; two deterministic waves |
| Complete branch setup/pilots/finals/retries | 90,000 | At most 8 setup attempts; complete support |

Multiply reference draws by the request's reference cost. Existing fractions
remain 0.45 (tail), 0.5 (shared), 1.5 (confirm-more), and 0.15 (branches).
Constructor node/guide ceilings are preserved. The optional residual-loop and
late-gap experiments are excluded from this integration.
Native scout, pool, proofs, neighborhood and earlier samplers keep their existing
fixed work limits; this change targets the time-driven additional portfolio.

### Allocation and reproducibility

1. Independent pilots determine draw costs and proposal quality. Confirmation
   priority is forecast ESS per work, multiplied by `hits / (hits + 30)` to avoid
   giving a cheap but unreliable pilot priority over established evidence.
2. Ordinary main/check pairs reserve their complete modeled cost. Ordinary finals
   finish at a barrier before extensions are allocated. Their skipped checks are
   not refunded.
3. Additional confirmations reserve ordinary pairs first, then use spare units
   for pending main batches. After all first-wave work completes, skipped checks
   release their reserved units. In a second stable allocation pass, those units
   fund independent checks of publishable pending mains or complete pending pairs.
   There is no third wave and a main alone cannot publish.
4. Cheap complete-branch proposals first run the original 30k main/check design
   and stream labels. After failure, spare work can fund a **fresh** independent
   retry up to 90k. Every enumerated branch receives positive allocation.
5. Expensive admitted jobs run first on at most four workers. Results return in
   stable admission order. The allocator depends on completed pilot evidence and
   fixed barriers, not which worker happened to finish first.

For Londrina/3rd, seed 808, the 22 branch pilot costs ranged from 329 to 1,841
units/draw, versus a 6,081-unit request reference. A successful original 30k pair
was preserved. Across other seeds, cheap full-support retries recovered three
baseline-zero runs. A larger batch is not guaranteed to pass the same ESS/weight
share gates, which is why the original independent pair runs first.

## Paired full-request results

Baseline: actual current Rust binary frozen before changes, HEAD `7b36a823`
**plus the pending adopted R36 and existing experiment prerequisites**. It is not
an older Go baseline. SHA256:
`61abf11d493efb61fb17cfe6c90ae9945bf31e7cc8e38d14c53694e0225eccd4`.
Selected measured candidate SHA256:
`cfee52292a55c7f94f1afe5ce9fac4969c0b45464b34bc47839a23f5551696b2`.

Seven saved snapshots, six groups, 14 seeds, serial paired calculations. The
current and historical group 16653 snapshots are separate requests. Each request
uses four estimator workers. Fresh loopback servers are alternated baseline-first
and candidate-first; no candidate-specific probability flags were set. HTTP wall
excludes startup. CPU is child user+system time including process lifecycle.
No calculations overlap and no production hardware assumptions replace local
measurements. Exact input and binary hashes are in each archived summary.

| Snapshot | Mean baseline HTTP ms | Mean selected HTTP ms | Wall change | CPU change | Gains / losses |
| --- | ---: | ---: | ---: | ---: | ---: |
| 15902 | 169.02 | 164.51 | −2.67% | −1.41% | 0 / 0 |
| 16413 | 150.00 | 150.16 | +0.11% | +0.57% | 0 / 0 |
| 16498 | 1,201.23 | 1,277.62 | +6.36% | +3.57% | 4 / 0 |
| 16653 historical | 693.04 | 699.73 | +0.96% | +1.11% | 0 / 0 |
| 16653 current | 1,082.56 | 1,095.49 | +1.19% | +1.00% | 3 / 0 |
| 16982 | 256.32 | 254.05 | −0.88% | −1.26% | 0 / 0 |
| 16983 | 255.09 | 252.04 | −1.20% | −0.50% | 0 / 0 |
| All 98 | 543.90 | 556.23 | **+2.27%** | **+1.62%** | **7 / 0** |

Total CPU: 125,987 → 128,032 ms. Reachable-zero cell-runs: **72 → 65**;
positive cell-runs: **36,552 → 36,559**. Impossible zeros stay at 1,512 and
undecided zeros stay at zero in this cohort. Coverage per CPU-second changes from
290.13 to 285.55 positive cell-runs; the benefit is reproducibility and targeted
zero coverage, not an overall CPU reduction. Coverage is finite-cohort evidence,
not a guarantee for every request/seed. Fixed-seed wall baseline variation is
another source of noise in exact gain counts.

| Group | Team / rank | Seeds gaining an estimate | Estimated probability (fraction) |
| --- | --- | --- | --- |
| 16498 | 16 / 13 | 2459, 9749 | 6.20e−20, 6.39e−20 |
| 16498 | 16 / 15 | 1861 | 4.01e−32 |
| 16498 | 318 / 4 | 1861 | 2.67e−19 |
| 16653 current | 95 / 3 | 1993, 2039, 7919 | 3.69e−34, 4.50e−34, 5.78e−34 |

These are rough accepted importance estimates, not ground-truth probabilities.
Cell-runs count one team/rank/seed/snapshot; seven gains mean four distinct cells,
not seven new cells in one table. At seed 808 neither group gained a new cell
relative to its paired baseline; deterministic allocation mainly stabilizes that
request and improves some other fixed seeds.

### Where time went

Mean stage wall time across 98 requests; parent/child rows overlap and must not
be added together. Rare-tail includes shared proposals and confirmations.

| Stage | Baseline ms | Selected ms |
| --- | ---: | ---: |
| Setup | 1.75 | 1.75 |
| Scout | 65.96 | 64.68 |
| Pool total | 81.31 | 80.69 |
| Initial conditioning | 13.36 | 13.33 |
| Guided search | 28.76 | 28.50 |
| Extra search | 57.82 | 57.54 |
| Point tilt | 80.58 | 80.45 |
| Domains | 6.81 | 6.90 |
| Rare-tail portfolio | 188.34 | 203.15 |
| All search | 391.36 | 405.60 |

Most added work is in the changed portfolio. The increase is explicit and was
accepted by the user, rather than being charged against an assumed one-second
allowance. No CPU-specific instructions were introduced; Xeon results may differ.

## Trials and rejected alternatives

| Trial | Pairs | Gains / losses | CPU change | HTTP change | Decision |
| --- | ---: | ---: | ---: | ---: | --- |
| Draw-only fixed quotas | 77 | 8 / 0 | +1.94% | +3.12% | Superseded: treats cheap/expensive draws equally |
| Operations v1 | 16 | 0 / 2 | +0.11% | +2.06% | Reject larger-first branches and cheapest weak pilot priority |
| Operations v2 + validation | 77 | 3 / 1 | −2.26% | +0.60% | Add pilot hit reliability |
| Operations v3 | 77 | 3 / 0 | −0.41% | +1.52% | Retain stable quality/work priority |
| Scheduled v4, including fresh seeds | 98 | 5 / 1 | +0.34% | +0.14% | Fresh seed exposed skipped-check waste |
| Two-wave check recycling v5 | 98 | **7 / 0** | **+1.62%** | **+2.27%** | **Selected; user accepts measured cost** |
| All mains before checks | 98 | 5 / 5 | +1.89% | +1.67% | Reject: main batches starve independent checks |
| Compile counters out of final draws | 98 | 6 / 0 | +4.30% | +5.57% | Reject: slower despite 98/98 responses matching v5 |
| Confirm quota 1.2 instead of 1.5 | 98 | 4 / 2 | −1.61% | −0.38% | Reject: loses baseline estimates |
| Confirm quota 1.35 | 98 | 5 / 0 | +0.87% | +2.93% | Reject: delayed second-wave pairs, worse latency/coverage |
| Complete-union setup cap 128 vs 256 | 98 | 6 / 0 | +0.22% | +0.90% | Keep 256 for seventh gained run under accepted allowance |

The union cap trial falls back to lazy complete-support proposals when a full
union cannot be built, never a truncated prefix. Reducing to 128 gave up the
16498 team16/rank13 seed2459 gain. The override is now honored even with extension
enabled, but the default remains 256.

Eleven development seeds: 808,1669,1993,2293,1847,1861,2281,2137,2357,2039,2459.
Three seeds initially held out: 7919,9749,10007. After using their regressions to
revise the allocator, the final 98 are a regression cohort, not untouched
statistical validation. Baseline vs itself had no zero coverage change but three
complete-response differences. Timing percentages are noisy local measurements;
the 128-union alternative was run later, not concurrently against v5.

## Correctness and limits

- Probability weights, actual phase sorter, reachability proof semantics and
  acceptance gates are unchanged. Work units never enter probability weights.
- Setup/pilot decisions are independent of final samples. Ordinary draws have
  prechosen lengths; extra confirmation uses the already-required publication
  screen to decide whether an independent check is useful. Main-only results are
  discarded. Screening/acceptance already makes this a filtered rough-estimate
  procedure; deterministic budgeting does not promise an unbiased unconditional
  published sample or a formal confidence bound for each accepted estimate.
- Branch retries use fresh streams and retain every supported branch. Original
  pairs are preserved before a retry. Previous positive estimates cannot be
  overwritten by a later failure.
- Budget exhaustion means unresolved estimation work, not impossibility. Witnesses
  remain reachability evidence, not probability estimates. No team/rank hardcoding.
- Reproducibility is for identical requests, flags, seed and the same build/runtime
  math. Cross-architecture floating-point bit identity is not established.

## Reproduce

From repository root, using the same locked dependencies and stable toolchain:

```sh
cargo build --release --locked -j4 --manifest-path odds-rust/Cargo.toml
# No extra runtime flags are required for default deterministic operation quotas.
odds-rust/target/release/golaberto-odds estimate \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16653-71d4fea8.json \
  /tmp/deterministic.json 808 4

# Policy comparison after rebuilding (wall behavior, not the frozen historical binary):
RUST_ODDS_DETERMINISTIC_WORK=0 odds-rust/target/release/golaberto-odds estimate \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16653-71d4fea8.json \
  /tmp/wall-budget.json 808 4

# The exact recorded comparison used the preserved pre-change binary:
python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline /private/tmp/golaberto-before-deterministic \
  --candidate odds-rust/target/release/golaberto-odds --production-candidate \
  --seeds 808,1669,1993,2293,1847,1861,2281,2137,2357,2039,2459,7919,9749,10007 \
  --output /tmp/deterministic-paired
python3 experiments/rare_positions/summarize_work_budget.py \
  /tmp/deterministic-paired/summary.json

cargo test --release --locked -j4 --manifest-path odds-rust/Cargo.toml \
  --tests -- --test-threads=1
```

The temporary frozen binary is local, not committed; rebuilding current code with
the wall flag reproduces the old policy but not its exact historical machine code.
The archive preserves its hash, full responses/logs and recorded paired metrics.
A scoped candidate source patch against HEAD also includes pending Rust experiment
prerequisites; it excludes unrelated Rails/database/player-ratings edits.

## Tests and sources

**123 Rust tests passed; 3 database-write tests were ignored.** All 14 full HTTP
repeat pairs (seven snapshots × seeds 808/2293) had identical complete response
bytes. The final build also matched the selected measured v5 response bytes in
all 14 cases. Its SHA256 is
`e130cfe57b94268793b86f2f849502f0d4b1901497bb0b8a5cad0451a67242c5`.
Only diagnostics, test wiring, formatting, and honoring the nondefault union-cap
override changed after the selected benchmark.

Final test results and HTTP repeat verification are recorded in `metadata.json`
and `tests.log` in the [artifact directory](2026-10-01-deterministic-work/).
The integration test runs identical CLI responses twice with four workers and
once with one worker/logging disabled, checking bytes and budget logs. Unit tests
cover weighted cost arithmetic, bounded reservations/refunds, cheap proposal draw
capacity and stable execution result order. Existing exhaustive mixture-density
and branch-support tests exercise the unchanged probability mathematics.

Implementation sources: [budget model](../../odds-rust/src/rare_tail/budget.rs),
[two-wave confirmations](../../odds-rust/src/rare_tail/confirmations.rs),
[portfolio](../../odds-rust/src/rare_tail.rs),
[complete branches](../../odds-rust/src/rare_tail/branches.rs),
[shared conditioning](../../odds-rust/src/shared_constraints.rs),
[guided proposals](../../odds-rust/src/joint_caps/propagated.rs),
[lazy joint proposals](../../odds-rust/src/joint_caps/propagated/lazy.rs), and
[repeat test](../../odds-rust/tests/work_budget.rs).

## Integration verification — 2026-10-02

The release includes deterministic accounting and the already adopted R36
complete-branch stage required by the measured baseline. Separate Rails request
construction, player-rating performance, residual-loop and late-gap changes are
excluded. The scoped release tree is tested against the committed dependency
lockfile before pushing. Experimental source patches in the archive preserve the
full measured workspace and are not the release patch.

Scoped release verification: **119 Rust tests passed**, 2 DB-write tests ignored.
The local DB request at production seed `1790924698729174282` finds
Fluminense/20th at `5.252665895092511e-26` (fraction), with independent
confirmation; its complete response matches the measured working implementation
byte for byte. The production request body was not supplied, so this is a local
DB replay at the same seed, not an exact production-body replay.
