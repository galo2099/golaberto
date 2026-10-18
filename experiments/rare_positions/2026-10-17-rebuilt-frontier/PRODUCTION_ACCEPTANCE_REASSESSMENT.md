# Production acceptance reassessment — 2026-10-18

Latest GitHub master `77a3da2e24ff0fe51282326c44471a8768776e70` was built in
an isolated worktree and run against the recovered saved-request corpus.
The corpus contains **seven distinct snapshots and 23 seeded requests**, giving
9,048 cell-runs and 2,724 distinct snapshot/team/position cells. This is not
evidence of 23 distinct inputs or a 6,840-cell benchmark. The historical
6,840 denominator remains a conservative confidence-family parameter in the
recovery experiment.

No production source or endpoint behavior was changed for this assessment.

## What can be certified from production's existing observations

Each request retains 100,000 native Monte Carlo draws in its `hits` and
`samples` fields. Source inspection confirms that the matched-pool and
importance-sampling overlays preserve these original counts. Every team row
and rank column sums to 100,000 hits. All 23 requests use the pool path, so
there is no stratified-count interpretation in this corpus.

For each cell-run, construct Clopper–Pearson endpoints with per-tail allowance
`0.01 / (2 * 9048) = 5.526083112290008e-7`. This is a fixed-look allocation over
the declared family, with simultaneous 99% coverage under the native sampling
model. Apply the policy from [ADAPTIVE_QUALIFICATION.md](ADAPTIVE_QUALIFICATION.md):
positive lower endpoint, absolute width at most 0.01, and logarithmic
halfwidth at most 5% of the probability's exponent scale, floored at one.

These intervals certify the true cell probabilities using the original native
counts. They do not turn a production jackknife standard error or importance
ESS into a finite-sample guarantee. Every final production point lies inside
its corresponding native-MC interval. The adopted reporting policy would use
the interval's geometric midpoint as its estimate.

| Classification | Cell-runs |
| --- | ---: |
| Production positive estimate | 8,503 |
| Production impossibility classification | 540 |
| Production reachable zero | 5 |
| Production undecided zero | 0 |
| Positive interval meeting the adopted rule from native MC | 3,902 |
| Qualified native interval or impossibility proof | **4,442 / 9,048 (49.09%)** |
| Non-impossible cells without a qualifying native interval | 4,606 |
| Of those: absolute-width failure only | 1,310 |
| Of those: logarithmic-precision failure only | 3,296 |

There are 2,060 zero-hit native-MC cell-runs, including all 540 impossibility
classifications. Production supplies positive estimates for 1,515 of the other
zero-hit cell-runs. Those estimates need confidence certificates from their
own sampling methods before they can add to the count above. This assessment
therefore gives a conservative certifiable coverage count, not a complete
requalification of every production estimator.

## One run per snapshot

Seed 1847 occurs in all seven snapshots. Keeping the same conservative
9,048-cell family allocation avoids relaxing confidence for this subset.
No observations or intervals from different seeds are pooled.

| Snapshot | Cells | Qualified native interval | Impossible | Remaining |
| --- | ---: | ---: | ---: | ---: |
| 15902 / 31d66705 | 400 | 195 | 0 | 205 |
| 16413 / a2eef0a8 | 324 | 133 | 0 | 191 |
| 16498 / 44eabb47 | 400 | 154 | 17 | 229 |
| 16653 / 2d1c1d6f | 400 | 164 | 43 | 193 |
| 16653 / 71d4fea8 | 400 | 151 | 48 | 201 |
| 16982 / 9327edcd | 400 | 246 | 0 | 154 |
| 16983 / 43969b02 | 400 | 205 | 0 | 195 |
| **Total** | **2,724** | **1,248** | **108** | **1,368** |

This gives **1,356 / 2,724 (49.78%)** qualified-or-impossible cells. Production
returns 2,615 positives and one reachable zero in this subset. Of the 1,368
remaining cells, 377 fail absolute precision and 991 fail logarithmic precision.

## Additional production bounds

The new exact-cap stage logs an analytical lower and upper for team 125,
position 10, in snapshot 16653 / 71d4fea8: approximately
`[1.3328790480242366e-24, 1.3328790480242392e-24]`. This is one unique cell,
repeated in five seeds. The event decomposition identifies a strict
points/wins success subset and a necessary cap event.

Those endpoints currently use ordinary scalar floating arithmetic, rather
than an outward enclosure of the enumeration. They are excluded from the
primary certificate count. A numerical enclosure could add one unique cell
and five cell-runs. The production native-complement stage's own statistical
intervals are also excluded: its per-pass alpha is not the 9,048-cell family
allocation used here. A complete estimator-level requalification must expose
the necessary observations, numerical bounds and confidence allocation.

## What the measured experiments imply for an offline job

All three distinct target/input combinations tested by the rebuilt frontier
have a qualifying standalone 180-second run under the adopted policy, without
trusted-HiGHS pruning or separate analytical audits:

| Target/input | Simultaneous 99% interval | External wall seconds |
| --- | --- | ---: |
| Palmeiras 12th, 290 played | [6.147750115263105e-29, 3.1342415010044333e-28] | 179.152 |
| Palmeiras 16th, 285 played | [5.9453740344990614e-43, 8.844105786639036e-42] | 179.185 |
| Palmeiras 16th, historical 279 played | [3.105431383236009e-41, 1.719720850889973e-37] | 179.180 |

These are three target/input combinations, not a whole-corpus completion run.
They use different request snapshots; their probabilities must not be compared
as though they were the same cell. The saved full-budget runs do not measure
the first time that an adaptive job could have stopped.

At the production reassessment's confidence allocation, 100,000 native draws
qualify exactly at hit counts 276–11,937 or 88,063–100,000. The middle range
fails the 0.01 absolute-width requirement. Approximately 237,544 fixed draws
remove the worst-case absolute-width barrier. This is a draw-count planning
calculation, not an adaptive stopping guarantee or a runtime benchmark.

Shared native simulation should handle ordinary cells before individual rare
cell work. A season supplies observations for the whole matrix. Historical
reference runs produced five million seasons in about 19–20 seconds per
snapshot on a different ARM host; current timings must be measured separately.

For a concrete campaign allowance, giving each of the seed-1847 subset's 991
logarithmic failures the full 180 seconds reserves **49.55 worker-hours** for
one targeted sweep. Many cells could finish earlier, and shared proposals or
analytical bounds could reduce that cost. Conversely, some cells could remain
unqualified or need additional ranking-rule support. This arithmetic is a
budget for one pass, not an estimate or upper bound on time to finish every cell.

There is no measured whole-corpus completion ETA yet. In particular, more
draws cannot remove an irreducible analytical score bracket, and bounded proof
searches can remain unfinished. A completion estimate needs an end-to-end
offline benchmark with stopping times and unresolved cells recorded. The
appropriate job structure is shared MC, proof and analytical bounds, followed
by targeted estimation and refinement for the remaining uncertainty.

The stronger retained historical Palmeiras-16 interval
`[4.618240572657253e-41, 1.802594383784308e-38]` used separately budgeted
sampling and multiple analytical audits. It is not necessary to obtain an
initial adaptive qualification and is not a single 180- or 540-second run.

## Reproduction and validation

- Source: latest master `77a3da2e24ff0fe51282326c44471a8768776e70`.
- Source fingerprint: `2157814a8ecefb0c32a7c801fd853ffd6a6732db098e2723dc3b3535f3afa3f9`.
- Release binary: `0c78fb79e9536fb0d0e737933da2a9228610b52ef1084ff0bed7dd7d01138e86`.
- Build: `cargo build --release --locked --offline --manifest-path odds-rust/Cargo.toml`.
- Host/toolchain: x86_64 Linux, Rust 1.99.0; four production workers.
- CLI: `golaberto-odds estimate INPUT OUTPUT SEED 4`.
- All inherited `RUST_ODDS_*` and `RARE_POSITION_*` variables cleared.
- Seeds 808, 1669, 1993 on snapshots 16498 and both 16653 inputs; seeds
  1847 and 1861 on all seven inputs.
- Input bytes match the archived request hashes. Builds preceded serial timing.
- Total external request wall: **31.44555 seconds**, range **0.225–2.894 seconds**.
- Source and binary unchanged across runs. No benchmark ran concurrently.
- An independent reviewer recomputed counts and qualification from all 23 raw
  exports, checked row/column hit totals, and confirmed matching native counts
  against the preceding checkout. Production point estimates can differ.

Raw requests, exports, logs, intervals, manifests and provenance remain local
at `/workspace/rare-odds-production-2026-10-18`; they are not uploaded. The
aggregate report alone is suitable for the experiment branch.

Relevant experiment reports: [adaptive qualification](ADAPTIVE_QUALIFICATION.md),
[GD rebuilding](GD_REBUILD_RESULTS.md),
[conditional regimes](REGIME_RESULTS.md), and
[separate-budget retained bounds](SHARED_EXPECTATION_COHORTS_9_4_RESULTS.md).
