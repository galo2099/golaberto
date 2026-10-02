# R35: ordering and pruning the 22 complete branches

## Final disposition — 2026-10-02

Committed the offline audit/driver and evidence. Cheap bound ordering is
already integrated by R36. Strong extra bounds and branch omission remain
offline because these measurements do not support a production speedup.

Raw files are stored in `2026-10-01-branch-bounds/evidence.tar.gz`, with per-file
hashes in `manifest.json`. Extract that archive in its directory before using
the historical commands. Statements about flags, defaults and commit status
below refer to the experiment date. Current disposition is recorded in
[the decision report](2026-10-02-rust-experiment-decisions.md).

## Result

Yes: compute the original-model probability of each branch's fixed fixture
outcomes, strengthen it with necessary point/win events, and process larger
bounds first. A branch can be omitted when the **sum of all omitted bounds**
fits a declared error budget. Omission is an approximation with bounded missing
mass, not an impossibility proof or an exact full-event estimate.

For the R34 Londrina/3rd experiment, using a reference fraction4e-34 and a1%
omission budget:

- Fixed priors permit omitting branch21: prior6.318e-39.
- The already-built joint normalizer tightens that bound to3.854e-43, at about
  0.013ms additional setup. Omitting it saved2.53% single-cell process CPU in
  paired cold runs, retaining exactly the same7/11 accepted seeds.
- A six-rival necessary-event DP permits omitting all three Atlético-GO branches,
  with **combined upper bound3.851e-36**. It costs about7.53ms extra setup on
  average, outweighing the saved sampling: process CPU+9.93%, complete offline
  cell wall+22.99%. Acceptance remained7/11.
- Ordering alone preserved estimates bit-for-bit but did not produce a clear
  timing improvement: CPU+0.96%, complete cell wall−1.43%.

Recommendation: reuse the cheap existing bounds; keep stronger bounding offline
or reuse it when another stage has already paid its construction cost. Continue
using pilots for allocation within the important branches. No production
integration, default changes, full-request benchmark, commit or push.

## What the bounds mean

Let branch i impose fixed W/D/L outcomes F_i. The fixed prior is
`A_i = product(P_fixture(outcome))`. All fixture outcomes in the current model
are independent before conditioning. Every season counted by the branch satisfies
F_i, so its contribution to the requested position is at most A_i. Scores and
tiebreakers can only reduce that contribution.

Each branch also has propagated fixture masks D_i and a necessary joint event
J_i for the selected rivals' final point/win intervals. The existing sampler's
normalizer is exactly the original-model probability of F_i, D_i and J_i:

`U_i = A_i × P(D_i | F_i) × P(J_i | F_i,D_i)`.

This gives a tighter upper bound without drawing any seasons. It is independent
of whether the importance sampler happens to find a matching rank. It is also
different from the allocation hint: `residual_hint` multiplies marginal rival
probabilities despite shared fixtures, and **must not be treated as a bound**.

The strengthened bound builds the same native necessary-event DP with up to six
rivals on the full remaining fixture set, under the already fixed paths and
masks. Internal games are enumerated jointly and external independent fixture
sets are convolved. At most12,000 internal states and20,000 setup nodes per call;
the builder may return fewer selected rivals. Failed/exhausted setup falls back
to the old bound. The minimum of valid necessary-event probabilities remains an
upper bound. This does not change the four-rival importance proposal.

A second diagnostic uses a read-2 bound: the original joint interval event plus
the remaining one-team interval events depend on independent fixture variables,
each used by at most two events. Generalized Holder/Shearer bounds their
intersection by the square root of the product of their marginal probabilities.
The arbitrary-marginal case follows from Corollary2.8 with all events required;
see [Gavinsky, Lovett, Saks and Srinivasan](https://arxiv.org/pdf/1205.1478).
It improved none of these22 six-rival bounds and is not recommended as additional
production work based on this cohort.

These are analytical bounds under the current discretized Poisson model,
computed with ordinary f64 DPs. They are **not interval-arithmetic certificates**.
The experiment adds1e-9 relative headroom, skips numerical-zero DP components,
floors product underflow at the smallest positive normal f64 value, and never
uses a numerical zero to prove impossibility. No arbitrary goal cap, rank
interpolation or feasibility relaxation supplies a reachability claim.

## All 22 branches

All values are fractions, not percentages. IDs are R34 zero-based branch indices;
the parent identifies the sole additional team above Londrina besides Vila Nova.
Sorted by the strengthened upper bound, largest first. Complete fixture lists
are in the R34 audit, indexed identically. Columns are rounded for display;
decisions use full-precision values. Reference4e-34, total omission budget4e-36.

| Branch | Parent above | Fixed prior | Existing joint upper | Strong upper | Decision |
|---:|---|---:|---:|---:|---|
| 6 | Novorizontino | 4.900e-10 | 9.674e-21 | 3.032e-24 | Keep |
| 1 | Novorizontino | 2.785e-14 | 7.598e-25 | 1.040e-27 | Keep |
| 12 | Fortaleza | 7.951e-14 | 5.114e-25 | 6.959e-28 | Keep |
| 16 | Fortaleza | 1.932e-13 | 1.215e-24 | 6.358e-28 | Keep |
| 7 | Novorizontino | 4.257e-14 | 3.220e-25 | 5.837e-28 | Keep |
| 9 | Novorizontino | 4.165e-14 | 1.131e-26 | 6.720e-29 | Keep |
| 2 | Novorizontino | 5.497e-14 | 1.493e-26 | 5.507e-29 | Keep |
| 5 | Novorizontino | 4.306e-14 | 1.170e-26 | 4.315e-29 | Keep |
| 8 | Novorizontino | 4.294e-14 | 1.166e-26 | 4.302e-29 | Keep |
| 0 | Novorizontino | 3.816e-14 | 1.037e-26 | 3.824e-29 | Keep |
| 4 | Novorizontino | 3.609e-14 | 9.802e-27 | 3.616e-29 | Keep |
| 3 | Novorizontino | 2.210e-14 | 6.002e-27 | 2.214e-29 | Keep |
| 18 | Fortaleza | 5.221e-18 | 3.233e-28 | 1.750e-30 | Keep |
| 13 | Fortaleza | 1.103e-17 | 1.337e-27 | 1.199e-30 | Keep |
| 17 | Fortaleza | 1.010e-17 | 5.597e-29 | 3.981e-31 | Keep |
| 15 | Fortaleza | 3.742e-18 | 2.075e-29 | 1.475e-31 | Keep |
| 14 | Fortaleza | 3.431e-18 | 1.902e-29 | 1.353e-31 | Keep |
| 11 | Fortaleza | 2.924e-18 | 1.621e-29 | 1.153e-31 | Keep |
| 10 | Fortaleza | 2.920e-18 | 1.619e-29 | 1.151e-31 | Keep |
| 19 | Atlético-GO | 7.153e-22 | 1.054e-32 | 3.723e-36 | Skip |
| 20 | Atlético-GO | 1.190e-21 | 9.972e-34 | 1.276e-37 | Skip |
| 21 | Atlético-GO | 6.318e-39 | 3.854e-43 | 3.431e-43 | Skip |

The other19 branches remain too loosely bounded to discard, even where pilots
observed no hits. A high bound is room for probability, not evidence of a large
actual contribution. R34 observations suggest branches6,12,16 dominate, but the
table alone does not prove that. Ordering and bound-based pruning do not replace
importance sampling.

## Safe omission and scheduling

1. Compute cheap priors and reuse existing joint bounds. Sort descending, and
   run at most four branches concurrently.
2. Use an existing accepted estimate, or independent training batches, to set a
   scale and choose a tolerance appropriate for rough estimates.
3. Select the smallest-bound tail only while its **summed** bound is below that
   tolerance times the reference. An individual1% threshold on each of22
   branches would permit excessive total omitted mass.
4. Freeze the retained branch set and allocations before fresh main/check draws.
   Keep existing accepted cell estimates. Record the retained estimate separately
   from `omitted_upper`. If a justified retained confidence interval is[L,U], the
   full-event interval can include the omission as[L,U+omitted_upper].

The relative budget is relative to the supplied estimate, not a proven lower
bound on the true event probability. The absolute omitted bound is meaningful
even if the reference estimate is inaccurate. For example3.851e-36 is0.96% of
4e-34, but2.31% of the smallest R34 accepted estimate1.669e-34. It cannot be
presented as a guaranteed1% relative error on the true probability.

This experiment freezes the set from a fixed4e-34 reference before any pilots.
It preserves all R34 pilots and draw allocations, then skips the selected main
and check batches. This isolates ordering/pruning effects and keeps retained
branch draws bit-for-bit identical. It does not reallocate the saved draws and
does not test a production controller that stops adaptively during final draws.

## Paired measurements

Baseline: isolated Rust2290fe04 plus the R33/R34 focused patch; the frozen R34
example executable is the sampling baseline. Unrelated late-gap workspace
changes are excluded. Identical group16653 historical snapshot, fixed model
means, seeds808,1669,1993,1847,1861,2111,2129,2141,2161,2179,2197. Eleven seeds,
three repeats, four arms:132 single-cell invocations. Arm order reverses in the
middle repeat. Fresh model/proposal/bounds setup per invocation, four workers,
all invocations/builds/tests serialized. Local ARM Mac, Rust1.93.1; no Xeon or
full-request measurement.

| Arm | Skipped branches | Final draws per batch | Mean CPU ms | Mean stage ms | Model+setup+bounds+stage ms |
|---|---:|---:|---:|---:|---:|
| R34 baseline | 0 | 30,000 | 61.18 | 22.90 | 31.78 |
| Fixed-prior order only | 0 | 30,000 | 61.77 | 22.50 | 31.32 |
| Existing joint bounds + omission | 1 | 29,684 | 59.64 | 22.20 | 31.07 |
| Strong bounds + omission | 3 | 29,052 | 67.26 | 22.42 | 39.09 |

CPU is complete child user+system time. Complete cell wall sums instrumented
stages; process wall including launch/output was39.72,39.29,38.87,47.09ms,
respectively. Differences are small, so these are directional measurements,
not a promised production speedup. All arms accepted the same7/11 seeds in every
repeat (21/33 accepted invocations, seven distinct accepted seeds).

Ordering-only main/check values are bit-for-bit unchanged. Cheap omission changes
main estimates by at most1.99e-9 relative; strong omission by at most1.30e-6
relative. These are observed estimate differences, distinct from the analytical
omission bounds. Existing acceptance decisions were unchanged; estimates are
now for the retained union and carry a separate omitted-mass bound.

A preliminary amortized run reused setup across eleven seeds per process.
It misleadingly makes strong bounding look cheaper: CPU47.85→45.04ms/seed,
compared with cold61.18→67.26ms. Production cannot assume that setup amortization
across different requests. Both cohorts are archived; use the cold cohort for
the recommendation.

## Tests, code and reproduction

The complete-branch exhaustive fixture test now checks every branch's fixed,
domain and strengthened bounds against its exact contribution and verifies
coverage of the complete event. A new underflow test prevents products becoming
zero impossibility claims. No HTTP JSON or production search behavior changes.
All61 library tests passed with four test threads after the final guard.

```sh
cargo build --release --locked -j4 --manifest-path odds-rust/Cargo.toml \
  --example branch_bounds --example branch_stratification
cargo test --locked -j4 --manifest-path odds-rust/Cargo.toml --lib -- --test-threads=4

odds-rust/target/release/examples/branch_bounds \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16653-71d4fea8.json \
  95 3 4e-34 0.01 strong

python3 experiments/rare_positions/experiment_branch_bounds.py \
  odds-rust/target/release/examples/branch_stratification \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16653-71d4fea8.json \
  /private/tmp/r35-cold
```

The driver defaults to the new example's unchanged `off` mode as baseline.
Pass `--baseline PATH` to use the frozen R34 binary. `--amortize` reproduces the
preliminary cohort; it is not the realistic single-request setup comparison.
The branch sampler's new trailing options are
`off|prior|joint|strong REFERENCE_PROBABILITY RELATIVE_TOLERANCE`.
For order-only use `prior 4e-34 0`; for cheap omission use `joint 4e-34 .01`.
Prior mode with a positive tolerance also supports fixed-prior-only pruning.

`2026-10-01-branch-bounds/` archives audits, cold and amortized raw JSONL,
commands, hashes, CPU/wall summaries, tests and a focused patch against2290fe04
including R33/R34 dependencies. Measured binaries preceded the final underflow
guard; the final smoke checks that these non-underflowing branch values remain
unchanged. Current production defaults are unchanged.
