# R53: prioritize unresolved case bounds

Status: completed on 2026-10-03. Both tested allocation strategies are rejected.
Offline experiment; production source/defaults unchanged, no commit or push.

## Question

Can the R52 exact fixture counter make better progress with the same work by
discarding impossible cases, deferring small upper bounds, and repeatedly
investigating the largest unresolved absolute probability mass?

The saved group 16498 / Flamengo (team 17) / rank 12 partition has 186 target
paths and 5,884 rival cases. A case is a constrained region containing many
shared fixture assignments, not one complete season. The request, partition
and baseline are the same as
[R52](2026-10-02-rust-exact-rank-cases.md).

## Allocation and correctness contract

- Keep exact forced-fixture factors, fixed-rival reduction, independent unary
  gain integration, component convolution and production points/wins ordering.
- Initialize case bounds cheaply. After each deterministic four-job wave,
  prioritize the largest remaining absolute unknown bound, breaking ties by
  case ID. Physical worker count does not change logical scheduling.
- A larger case call replays its previous prefix. Replace its old result;
  charge every visited node, including replay, to the global budget. This is
  not a resumable search frontier.
- Cases with zero remaining points uncertainty leave that queue. A completed
  case can still have positive settled probability or unresolved goal-tie mass;
  it is not necessarily impossible.
- Recompute global positive sums in stable case order. Never subtract a large
  previous upper bound from a running total to reveal a tiny remainder.
- Track settled-rank mass `S`, tie-sensitive mass `T`, unknown mass `U`, and
  rank-impossible mass separately. Before goal sampling, the rank probability
  lies in the numerical model interval `[S, S+T+U]`.
- Stop only when `T+U`, including every deferred/unopened case, meets a
  predeclared absolute tolerance or relative tolerance based on `S>0`.
  A sampled external estimate cannot certify that relative stopping rule.
- Budget or memory exhaustion leaves a bound and an unfinished status. These
  floating-point model bounds retain R52's correctness assumptions; they are
  not directed-rounding probability certificates.

## Why concentration alone is insufficient

R52's remaining bound is 1.244210916e-6. Its 100 largest cases hold 68.565% and
its 500 largest hold 99.99194%. The tail outside those 500 still sums to about
1.00e-10, many orders above earlier estimates around 1e-25. Prioritizing the
large cases can improve the bound, but visiting most of the current bound does
not establish that the remainder is negligible.

## Measurement plan

Compare equal allocation with best-first allocation in the same executable,
using four cores and the same request/case partition. R52's equal 20,000-node
calls actually visited 93,856,259 nodes. Charge that actual total, rather than
its larger reserved quota, to the best-first comparison. Measure wall and CPU
time, preparation, scheduling/replay overhead, complete cases, resolved mass,
and the summed unknown/tie remainder. Node equality is not a latency guarantee.

Check small exhaustive graphs, replacement/conservation, replay charging,
combined small tails, tie-sensitive stopping, and one/four-worker scheduling
invariance. No production promotion, additional endpoint latency or commit is
assumed from an offline result.

## First result: staged replay

The same-binary equal control reproduces every R52 case profile and numerical
counter result. Compensated aggregate sums can differ in their last bits from
the older reduction order. With 20,000 nodes per case it visits 93,856,259 nodes.

| Metric | Equal allocation | Largest bound, staged replay |
|---|---:|---:|
| Charged node visits |93,856,259|93,856,000|
| Latest retained prefix visits |93,856,259|46,436,000|
| Complete cases |1,470|0|
| Summed unknown upper mass |1.244210916e-6|1.257819053e-6|
| Process wall seconds |11.40|44.13|
| CPU seconds |42.27|73.97|

These are one paired local run, not medians or a production endpoint cohort.
The replay arm takes 3.87 times the wall time and 1.75 times the CPU, despite
charging almost exactly the same node count. Its remainder is 1.09% larger.
It performs 971 positive-node calls in 243 logical waves; 91 cases reach their
500,000-node cap and retain their bounds. About 50.52% of visits are prefixes
repeated by a later call. Fixed four-job barriers and unequal call lengths can
also leave workers idle. The process uses about 1.68 cores on average, versus
3.71 for the equal control, without exceeding the four-worker cap.

No replay increases a case's reported unknown bound in this run. The stopping
target is an absolute 1e-27; it is not reached. Goal-tie mass is approximately
1.442e-33 and does not drive this failure. The remaining points bound does.
Only 108 distinct cases receive positive-node calls. The largest case, ID 4190,
has an unknown bound of 2.555872809043847e-8 after 20,000 visits and
2.555872809043715e-8 after 500,000. Its bound barely improves with 25 times the
retained search depth. A large upper bound identifies possible importance;
it does not establish that extra visits will reduce that bound efficiently.

The nine-case single-win validation reproduces the exact R52 profiles and its
1.413430200674237e-37 tie-sensitive mass. One/four-worker scheduler outputs
agree. Replay charges 581,565 visits versus 221,565 for one exact pass; this
small validation stops at its category-scoped 1e-35 tolerance. It does not
certify the other categories. All 23 focused tests pass.

**Do not adopt staged replay.** The allocation idea needs a comparison without
repeated prefixes. Next measure a single pass with capped quotas proportional
to initial unresolved bounds, retaining every omitted/deferred bound and
counting preparation and allocation overhead. This separates allocation from
the current replay implementation.

## Second result: one-pass upper-bound allocation

The `upper-share` arm initializes the same root bounds and assigns deterministic
integer quotas proportional to their absolute mass. It redistributes shares
when a case hits its 500,000-node cap, then assigns integer remainders by
fractional share and case ID. Each selected case is counted once, with all
zero-quota cases retaining their root bounds. This arm checks the final
tolerance but does not implement dynamic early stopping.

| Metric | Fresh equal control | One-pass upper share |
|---|---:|---:|
| Actual node visits |93,856,259|93,856,259|
| Complete cases |1,470|0|
| Summed unknown upper mass |1.244210916e-6|1.244211564e-6|
| Process wall seconds |10.79|17.22|
| CPU seconds |40.30|62.85|

The fresh equal control uses the same final binary as upper-share and agrees
with the earlier equal control on all numerical case results and aggregates.
The one-pass arm allocates and actually spends its full budget; there are no
unused reservations in this run. It counts 2,055 cases, including 116 at the
500,000-node cap; 3,829 cases receive zero visits and keep their bounds.
Those deferred bounds sum to **8.964663218e-13**, still much larger than a
1e-27 absolute stopping target. Individual small bounds cannot be discarded
without tracking their combined contribution.

Upper-share removes repeated prefixes but takes 59.6% more wall time and 56.0%
more CPU than its fresh equal control. Its unknown upper mass is essentially
unchanged: larger by about 6.477e-13, or 0.0000521%. Preparation takes 758.84 ms,
root-bound evaluation 39.52 ms, integer quota allocation 1.60 ms, and counting
15,619.00 ms. The main expense is counting the favored cases, not scheduling.
Its average CPU usage is approximately 3.65 cores, within the four-worker cap.
The observed speed difference is not a production-Xeon benchmark or a repeated
latency cohort.

No settled-rank mass is resolved in this arm; tie-sensitive resolved mass is
3.652e-33. The final combined remainder is dominated by unknown points mass,
so the configured tolerance is not met. Small single-win validation runs agree
between one/four workers and equal/upper-share on the exact profiles and
221,565 final visits. All **26 focused tests** pass, including quota caps,
redistribution, deterministic integer remainders, zero weights, deferred bounds,
replay accounting, exhaustive fixture sums and tiny-tail arithmetic.

## Decision and next experiment

The proposed **combined-remainder stopping rule is sound within the numerical
model assumptions**. It prevents a collection of individually small cases from
silently becoming a large omission. Known goal-tie mass must remain in that
remainder until its effect is estimated or shown negligible.

Neither measured implementation improves allocation here. Large bounds are
loose in the hard shared-fixture cases; searching their low-mass prefixes more
deeply barely tightens them. Replay adds wasted visits and barriers. One-pass
proportional allocation avoids that waste but still favors expensive states
with little bound improvement. Completed-case counts are not the objective,
but neither arm improves the relevant total bound or calculates the rank odds.

Keep both arms offline. There is no endpoint integration or coverage gain, and
the existing native full-matrix baseline remains much faster than these
single-cell counters. No additional production latency allowance is inferred.

A useful next experiment would maintain resumable state and select partial
fixture assignments by their remaining weighted bound, rather than replaying
whole cases in the current fixed outcome order. Track measured bound reduction
per deterministic unit of work to avoid repeatedly favoring unproductive cases.
The combination of a tighter shared-fixture bound and the R52 exact independent
gain weights could also feed a conditional sampler. These improvements remain
untested; the present results do not establish that they fit production latency.

## Reproduction and retained evidence

Artifacts are in `2026-10-03-best-first-cases/` beside this report. They contain
the two source versions, the final standalone-example patch, inputs, raw JSON
and timing/test output, hashes and checksum manifest. Production library files
are unchanged. The v1 binary produced the first equal/replay measurements;
v2 added only the upper-share comparator and produced the fresh equal/share
measurements. Equal-mode numerical results agree between versions.
The archive contains 33 evidence files plus its checksum manifest, with SHA-256
`5becd5c13e08c54baae45f23b3b248c5d00b301f35438f3d8b3ea2c85e304687`.

```sh
work_dir=$(mktemp -d /private/tmp/golaberto-r53-repro.XXXXXX)
artifact_dir="$PWD/experiments/rare_positions/2026-10-03-best-first-cases"
git archive 4e812fd506b39a504dc3f771c26e8d027cff2311 odds-rust stats/core AGENTS.md | tar -x -C "$work_dir"
git -C "$work_dir" apply "$artifact_dir/v2candidate.patch"
tar -xzf "$artifact_dir/raw-evidence.tar.gz" -C "$work_dir"
cargo test --locked -j4 --manifest-path "$work_dir/odds-rust/Cargo.toml" --target-dir "$work_dir/target" --example best_first_rank_cases -- --test-threads=1
cargo build --release --locked -j4 --manifest-path "$work_dir/odds-rust/Cargo.toml" --target-dir "$work_dir/target" --example best_first_rank_cases
/usr/bin/time -p "$work_dir/target/release/examples/best_first_rank_cases" "$work_dir/raw/request-16498.json" "$work_dir/raw/certified-paths.json" "$work_dir/equal.json" --allocation equal --category all --reduction fixed --unary integrate --cutoff-bound unary --nodes-per-case 20000 --nodes-total 117680000 --memo-entries 100000 --workers 4
/usr/bin/time -p "$work_dir/target/release/examples/best_first_rank_cases" "$work_dir/raw/request-16498.json" "$work_dir/raw/certified-paths.json" "$work_dir/replay.json" --allocation best-first --category all --reduction fixed --unary integrate --cutoff-bound unary --initial-nodes 1000 --nodes-per-case 500000 --nodes-total 93856259 --memo-entries 500000 --absolute-tolerance 1e-27 --workers 4
/usr/bin/time -p "$work_dir/target/release/examples/best_first_rank_cases" "$work_dir/raw/request-16498.json" "$work_dir/raw/certified-paths.json" "$work_dir/share.json" --allocation upper-share --category all --reduction fixed --unary integrate --cutoff-bound unary --nodes-per-case 500000 --nodes-total 93856259 --memo-entries 500000 --absolute-tolerance 1e-27 --workers 4
```

Use serialized commands, four Cargo jobs and at most four estimator workers.
The equal arm reserves 117,680,000 nodes to retain the original 20,000-per-case
quota; it actually visits 93,856,259. The other arms use that actual budget.
Setting the equal total directly to 93,856,259 would reduce its individual
quotas and would not reproduce the control. For the nine-case checks, select
`--category 3win`, 100,000 nodes per case and 900,000 total. Their 1e-35 tolerance
only applies to that category. No conditional goal sampler is implemented here.
