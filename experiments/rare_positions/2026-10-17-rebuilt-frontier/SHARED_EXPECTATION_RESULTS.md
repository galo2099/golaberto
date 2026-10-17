# Shared clipped suffix expectations

This experiment tests whether repeated future points/wins states can share
the analytical work used to bound the clipped residual for historical
Palmeiras finishing 16th, cohort 5. It changes the standalone recovery
experiment, with the new operation disabled by default.

For one suffix state, define

```text
F_state(t) = E_baseline[(V_future - t G_future)+ * PW_feasible]
F_state(t) = sum_supported native_probability[o]
              * F_child(t * guided_ticket_probability[o] / native_probability[o])
```

The baseline ticket probability cancels inside this recurrence. It still
determines support, the guided ticket probabilities, and the outside prefix
weight. A prefix contributes at most
`baseline_prefix_probability * V_prefix * F_state(H * G_prefix / V_prefix)`.
Interval arithmetic rounds the outside coefficient upward and the clipping
threshold downward. Different prefixes retain their separate coefficients
and clipping thresholds; averaging their likelihood ratios would be invalid.

The sparse memoization key contains the plan, suffix position, proposal node,
canonical active-team totals, and a threshold rounded down by truncating its
floating-point mantissa. The default keeps eight mantissa bits. Rounding is
conservative at every level, and its accumulated looseness is part of the
returned bound. Completed rows are conservative envelopes, not exact
residual expectations or cell probabilities. Score and ranking requirements
beyond points/wins remain bounded by one in this operation.

The initial prototype probes suffixes with at most 18 fixtures remaining.
Completed suffix bounds replace outer-tree expansion and are added to an
archived expectation sum. Their existing pointwise observation caps are
retained separately. Incomplete probes retain all unresolved native mass
and ordinary search continues. An incomplete parent bound can be lost when
the parent is replaced by independently bounded children; the prototype
does not yet maintain a weighted work queue of unfinished shared cells.

The cache shares the existing suffix-state cap, but has its own storage.
Its default time allowance is half the analytical search allowance, with
the existing per-probe visit and time limits. The original guide/joint-ratio
cache retains its separate 15% allowance. Both operations remain subject to
the same outer search deadline. A smaller outer frontier alone would not
demonstrate an improvement: cached work, absorbed bound mass, remaining
upper bounds, and total elapsed time must also be measured.

## Validation and frozen implementation

All 72 optimized standalone Rust tests and 60 Python tests pass. New cases
compare the bound with independent enumeration of native and guided path
products, including finite points/wins constraints, genuinely different
guided ticket probabilities, off-grid thresholds, canonicalized keys,
scaled prefix weights with nonzero clipping, and interrupted work. Existing
production-service code is unchanged. Independent review found no remaining
mathematical blocker.

Source is backed up in commit
`e0a85251ed018c55531e476da19e7bd5f309691e` on
`experiments/rare-odds-recovery-2026-10-17` in `galo2099/golaberto`.
The local staged tree matches GitHub tree
`3c89126a035e422e073ad16e30d1d77a4505be11`.
Frozen binary `/tmp/frontier-recovery-expectation-v9` has SHA-256
`585616bae783bfb083fd17d559db8375e4d7302fa7fe001ae8da6375051a3d57`.

The 30-second smoke allowance returns safely in 24.460 seconds with no
errors or source/binary/input changes. It has not yet reached an eligible
suffix, so it is an execution check, not evidence of cache effectiveness.
The main comparisons use the same historical request, k=9 proposal and
complete 156-root Mirassol regime, with score-GD bounds disabled. Raw JSON
stays local; only aggregate results and hashes are backed up.

An initial 180-second control with frozen v8 ran while source edits were
in progress. Its receipt correctly marks the source manifest as changed.
The executing binary stayed fixed, but this run is calibration only and
will not be used as the formal matched comparison.

## Eight-bit pilot

Under a 180-second shared allowance with a 20-second cleanup reserve, the
eight-bit experiment returns in 164.730 seconds. Its residual expectation
upper is `5.04197458042185e-38`; the pointwise residual cap is
`1.0351934325746619e-32`. There are no errors or hash changes. The tree is
incomplete, with 14204736 processed outer prefixes and 10200490 pending.

The new cache uses 1000000 cells across 29035 canonical suffix states,
averaging 34.44 clipping-threshold variants per state. Of those cells,
999960 are completed conservative envelopes and 40 are incomplete. There
are 1264810 completed-cell hits and 5260 partial-cell hits. Its recorded
work is 1005183 visits and 7397.259 ms. The cell cap is reached, while the
recorded work is well below its nominal 50% cumulative time allowance.
The time-limit diagnostic can also describe a local or final deadline;
it does not establish exhaustion of the cumulative allowance.

The operation absorbs 66456 prefixes, saving an average 17.07 remaining
fixtures per absorption. Their aggregate upper before absorption is
`2.1697820579176346e-38`, and their archived upper afterward is
`2.3774332459459755e-41`: a 912.66x local reduction. The former is a
cumulative total at different absorption moments, not a counterfactual
final control result, and must not be subtracted from a control upper.

The archived part supplies only 0.04715% of the final upper. Pending
prefixes supply approximately `5.039597143278715e-38`, or 99.95285%; small
summation-rounding differences remain when reconstructing the reported
total from grouped depth statistics. Pending depths 73-74 alone hold
55.20% of the final upper. Thus the operation integrates the portions it
covers usefully, but capacity still limits coverage. A four-bit pilot
tests whether merging more threshold variants is worth the looser envelope.

## Matched 180-second allowances

The formal comparison uses the same frozen v9 binary with the operation
disabled or enabled. Runs are sequential, source/input/binary hashes remain
unchanged, and no build or other heavy experiment runs concurrently. Each
has a 180-second shared allowance, a 20-second cleanup reserve, the same
100M outer ceiling, and the same k=9 preparation and complete Mirassol
regime. These are individual timed runs, not a latency distribution.

| Metric | Disabled control | Eight bits | Four bits |
| --- | ---: | ---: | ---: |
| Total wall seconds | 164.229 | 164.730 | 164.930 |
| Residual expectation upper | 5.213167971e-38 | 5.041974580e-38 | 3.060367891e-38 |
| Processed outer prefixes | 14540788 | 14204736 | 14891904 |
| Pending prefixes | 10617622 | 10200490 | 9179758 |
| Absorbed prefixes | 0 | 66456 | 1190278 |
| Shared cache cells | 0 | 1000000 | 1000000 |
| Canonical states in that cache | 0 | 29035 | 82742 |
| Recorded shared-cache work, seconds | 0 | 7.397 | 8.338 |

All runs remain incomplete with zero outer leaves. The new operation's
processed-prefix count includes absorbed prefixes, and suffix-cache visits
are additional work; outer counts are not interchangeable with total CPU
cost. All three return the same pointwise residual cap,
`1.0351934325746619e-32`. No bound on individual observations is tightened
by the new mean-envelope integration.

The eight-bit upper is 3.28% below the control. The four-bit upper is 41.30%
below control, an observed 1.703x improvement, with 13.54% fewer pending
prefixes. The coarser grid's broader coverage wins despite each completed
envelope being less precise. Compared with eight bits, four bits lowers the
upper by 39.30% and absorbs about 17.91x as many prefixes. These runs do not
isolate the per-path rounding penalty: the two caches cover different sets
of prefixes and threshold states.

The four-bit cache has 999992 completed cells and eight incomplete cells,
with 2547031 completed-cell hits. It absorbs a cumulative before-bound of
`7.179949325500182e-38` into an after-bound of
`6.21515569933935e-40`, a 115.52x local reduction. Again, this before-bound
is not a counterfactual final control bound. The archived after-bound is
2.031% of the final upper; pending prefixes still account for about
`2.9982163318218353e-38`, or 97.969%. Capacity remains the demonstrated
limit on new shared cells.

For the longer test, the shared allowance increases to 540 seconds, cleanup
reserve to 40 seconds, and cache ceiling to four million cells. Keeping
four threshold bits tests whether capacity can use more of the available
analytical time. This combines a capacity increase and a longer allowance;
it is not an isolated estimate of either change's effect.

```bash
python3 residual_audit.py \
  --binary /tmp/frontier-recovery-expectation-v9 \
  --suffix-bounds --suffix-max-remaining 56 \
  --suffix-canonicalize-certain --suffix-joint-ratio \
  --shared-expectation --expectation-bits 4 \
  --suffix-state-cap 4000000 \
  --budget 540 --cleanup-reserve 40 --node-limit 100000000 \
  --output shared-expectation-bits4-cap4m-pal16-540s.json
```

## 540-second analytical result

The longer run returns in 511.254 seconds with no errors, hash changes,
reported memory/PMF limit, or total-wall overrun. Its residual upper is
`9.597486724327567e-40`, 27.88x below the retained prior 540-second
analytical upper of `2.675523916963522e-38`, a 96.41% reduction.

| Metric | Prior 540-second audit | New 540-second audit |
| --- | ---: | ---: |
| Frozen binary | v8, new operation absent | v9, four bits |
| Configured cache ceiling | 1000000 | 4000000 |
| Total wall seconds | 514.779 | 511.254 |
| Residual expectation upper | 2.675523917e-38 | 9.597486724e-40 |
| Processed outer prefixes | 37828610 | 53147439 |
| Pending prefixes | 34970893 | 14635409 |
| Absorbed prefixes | 0 | 19977587 |
| Shared expectation cells | 0 | 4000000 |
| Canonical states in the new cache | 0 | 200215 |
| Recorded new-cache work, seconds | 0 | 65.942 |

The old row is the existing v8 result documented in
[ANALYTICAL_TRIPLE_BUDGET_RESULTS.md](ANALYTICAL_TRIPLE_BUDGET_RESULTS.md),
not a fresh v9 disabled run. Both use a 540-second shared allowance,
40-second cleanup reserve, and matching accepted proposal metadata. The
new row also changes cache capacity, so this is a combined experimental
result, not an isolated latency comparison. The same-binary 180-second
comparison above supplies the direct disabled/enabled check.

The new cache has 3999993 completed cells and seven incomplete cells,
25500310 completed-cell hits, 4000348 visits, and a reached cell cap.
The original guide/joint-ratio cache still contains 928273 rows, as in
the old audit; raising the shared configuration ceiling did not expand
that original cache. No new score-GD bounds are enabled.

Almost 20 million outer prefixes are replaced by completed conservative
suffix envelopes. Their cumulative before-bound is
`1.0525761795144662e-37`; their archived after-bound is
`9.597459727745078e-40`. These local diagnostic sums do not define a
counterfactual disabled final result.

The archive now supplies 99.99972% of the reported upper. Grouped pending
depth bounds sum to about `2.697270485080664e-45`; subtracting the archived
upper from the sequentially rounded total gives `2.6996582488666792e-45`.
The small difference reflects accumulation rounding. Although 14.64 million
prefixes remain, their possible contribution is only about 0.00028% of the
total. With these archived envelopes fixed, finishing the remaining outer
tree could scarcely change this certificate. Improving the completed
envelopes is now more useful than adding outer expansions.

The tree remains incomplete with zero outer leaves. The archived value is
a conservative upper that includes grid rounding and relaxed score/ranking
requirements, not an estimate of the actual residual probability. Its
pointwise observation cap remains approximately `1.035193433e-32`.

## Full-cell interval using the existing samples

Keep only the existing 540-second sampler's statistics from
`lazy-hybrid-pal16-540s.json`; no new Monte Carlo observations are added.
The accepted proposal's target, selected rivals, complete regime, B and H
match the new certificate. Outward addition gives:

```text
cohort-5 core upper                 6.970578528214142e-40
new deterministic residual upper   9.597486724327567e-40
combined cohort-5 upper            1.6568065252541712e-39
other cohort uppers                2.123401939443611e-38
full cell upper                    2.2890825919690283e-38
full cell lower, unchanged         4.618240572657253e-41
```

The full upper is 52.98% below the previous combined
`4.868631641689276e-38`. Other cohorts now supply about 92.76% of the full
upper. The full upper/lower ratio is 495.66, and it qualifies under the
adopted adaptive logarithmic rule: exponent halfwidth 1.347592 versus
allowance 1.949397. This is not a factor-two estimate.

The existing 99% simultaneous confidence allocation is unchanged. The
deterministic certificate spends no additional failure probability.
Sampling and analysis use separate 540-second allowances; the combination
is not one 540-second endpoint run. The 180-second pilots and calibration
also consume additional experimental work. No statistical intervals from
different sampling runs are intersected.

## What the cache pattern tells us

Independent comparison confirms identical prepared-plan states, selected
rivals, accepted regime pivot/roots, bound B and clipping H in the three
180-second runs. The operation changes the deterministic residual audit,
not the proposal or sampling distribution.

The four-bit 180-second pilot nearly eliminates the final upper carried by depth-80 or
later prefixes: their share falls to about 0.00064%, versus 1.38% with the
operation disabled. Depths 73-74 now hold 62.55% of its final upper. The
cache therefore performs useful integration at the deep end, while many
earlier entrances to that integration remain uncovered.

A further capacity improvement can exploit the shape of the same function.
For fixed future PW acceptance and guided tickets, F(t) is decreasing and
convex. Two certified knot uppers at thresholds a and b give a conservative
upper chord between them; a lower-threshold knot also gives a valid
monotonic upper. A bounded set of knots per canonical state could answer
many thresholds without allocating one cell for every bin. Interpolation
must round outward, and a zero endpoint requires a separate certified
native/guided ratio cap. This is a proposed follow-up, not part of the
measured code.

The longer run also shows why an adaptive version should retain weighted
groups for completed envelopes. This prototype archives their scalar
upper sum permanently. Refining those bounds later requires retaining
enough state, threshold and prefix-weight information to revisit them;
further outer search alone cannot update the archive.

## Local receipts

| Artifact | SHA-256 |
| --- | --- |
| `shared-expectation-smoke-pal16-30s.json` | `6beadeed498de160c42db015cfe21bd93fd99d7c9b09405980796eb9445b810a` |
| Calibration-only `shared-expectation-control-pal16-180s.json` | `bc250e5d53701f9ea349656c25d21a1612176af21e9916be59ac3bd330461f04` |
| `shared-expectation-disabled-pal16-180s.json` | `54353fdcd5cd1a794feb7e490eb74c3b364193e06e3a1d5b9e0fb0c60d7518ae` |
| `shared-expectation-pal16-180s.json` | `008e2014eac6d27943ef2dedd253d65bfbaa2cfe5892554382b26f424137797f` |
| `shared-expectation-bits4-pal16-180s.json` | `e294412fee8c0558bb47af37e1a1c149163b02997564f55ea352d10acebf2b42` |
| `shared-expectation-bits4-cap4m-pal16-540s.json` | `04cff700b0621d4518312668a58b266ee507997df72656d40accbe0d0d2a894c` |

Independent numerical review verifies matching proposal metadata, receipt
hashes, the core-plus-residual arithmetic, and the separate-budget
interpretation. Source and this aggregate report are backed up on the
existing experiment branch; raw JSON remains local.
