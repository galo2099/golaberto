# Conditional GD bounds in the residual frontier

This experiment follows [JOINT_RATIO_RESULTS.md](JOINT_RATIO_RESULTS.md). It
adds an optional, deterministic upper bound on conditional score/ranking
probability in the standalone recovery experiment. Production and sampling
proposals are unchanged. The option is off by default.

The matched 180-second comparison shows no material overall benefit: the
residual upper is 5.019e-38 with GD bounds versus 5.005e-38 for control,
0.29% higher in this observed pair. The new bound does tighten local
branches, by up to about 30x, but activates in only 0.0021% of bound
evaluations. Keep the option experimental and off by default.

## What is bounded

The residual expectation integrates `max(V-H*G,0)*S`, where V is the native
likelihood relative to the baseline proposal, G is the guided likelihood
relative to that proposal, and S is the remaining conditional score/ranking
probability. Previous residual search bounded S by 1.

For each source, the new code selects one rival for which an exact final
points/wins tie with the target is possible. It excludes the endpoints of
the existing pair condition, screens candidates with bounded exact
single-team packed-points reachability, and prefers the most unfavorable
initial GD gap in the required ranking direction. This selection is a
heuristic for usefulness, not an impossibility proof.

The bound applies only when every feasible completion of the current branch
forces that rival to tie the target on points and wins. A possible strict
points/wins lead leaves the factor at 1. For a rival above the target, the
necessary score event is rival GD >= target GD; for a rival below, the
orientation reverses. Inclusive GD comparison conservatively includes all
GF and subsequent tiebreak cases.

For dyadic z in {9/8, 5/4, 3/2}, Markov's inequality gives
`P(D_B >= 0) <= E[z^D_B]`. The moment includes the actual existing pair
conditioning A through `E[1_A*z^D_B]/P(A)`. A weighted DP over the pair's GD
difference groups native score pairs by GD and preserves coefficients from
shared fixtures, including coefficients of magnitude 2. It uses the pair's
positive normalizer lower bound for outward division. Fixed private
fixtures outside the remaining order multiply the initial moment once;
private fixtures in the order contribute selected-prefix moments and
maximum allowed future moments. Each dyadic moment therefore upper-bounds
the required score probability uniformly over remaining W/D/L completions.
The factor is the minimum of these three moments and 1.

This factor multiplies a fresh expectation-mass bound, never a previously
scaled bound. It does not lower the per-observation residual likelihood cap.
Score-zero expectation prunes retain that cap in a separate archive. This
distinction matters because an expected score probability need not bound
each score-conditioned observation of the existing sampler.

All native finite score support is retained. Arithmetic rounds outward;
unsupported rules, invalid arithmetic, interrupted setup, or capacity limits
leave the source's factor at 1. There is no unverified numerical solver
pruning. Deterministic bounds do not consume confidence-error allocation.

## Validation and protocol

All 64 optimized Rust tests and 59 Python tests pass. Independent
mathematical review found no outstanding blockers. Tests cover enumerated
shared-score conditional moments in both orientations, the failure of
unconditional moments under conditioning, a private fixed prefix counted
once, forced versus optional points/wins ties, invalid arithmetic, expired
setup, actual forest-prefix W/D/L overrides, and expected-mass improvement
with unchanged residual observation caps.

Both timed arms use the same frozen binary and source, run serially without
competing heavy jobs, and receive one shared 180-second allowance each,
including proposal setup. Cleanup reserve is ten seconds; the outer limit
is 100M expansions. The suffix window is 56, cache target 1M states,
per-probe limits 25000 visits and 25 ms, cumulative probe allocation 15%, and
guaranteed-team keys enabled. Joint-ratio bounds are off. Only treatment
enables `--score-gd true`, with a maximum 5000 ms GD setup allowance inside
the same deadline.

The input is historical Palmeiras finishing 16th, cohort 5: 279 played,
101 remaining matches. Both arms prepare k=9 at a 250000-state cap, install
the same team-584 compact regime with 156 roots (90 tied, 66 strict), and
search the remaining 91-fixture W/D/L suffix. The baseline forest is
complete. This is a residual-certificate audit: it runs no simulations and
produces no new confidence interval or complete cell estimate.

## Measurements

| Metric | Control | GD bounds enabled |
| --- | ---: | ---: |
| Total wall time | 174.290 s | 174.731 s |
| Total-wall overrun | 0 s | 0 s |
| Residual expectation upper | 5.004862589e-38 | 5.019337210e-38 |
| Maximum residual observation cap | 1.035193433e-32 | 1.035193433e-32 |
| Outer expansions | 15359814 | 15296388 |
| Pending branches | 11585835 | 11511029 |
| Outer PW-impossible prunes | 4032244 | 4003762 |
| Guide zero-residual prunes | 3056379 | 3033199 |
| Outer terminal leaves reached | 0 | 0 |
| GD setup | 0 s | 0.446 s |
| Prepared GD sources | 0 | 156 / 156 |
| GD bound evaluations | disabled | 39645580 |
| Forced-tie evaluations / local tightenings | 0 | 818 / 818 |
| Smallest applied GD factor | unavailable | 0.03340645422 |
| Score-zero expectation prunes | 0 | 0 |
| Suffix cache entries | 928272 | 928273 |
| Positive / impossible / unknown probe roots | 5926 / 5785 / 23 | 5920 / 5785 / 18 |
| Suffix probe time | 2.901 s | 2.636 s |
| Final refresh time | 1.721 s | 1.821 s |
| Mandatory aggregation time | 0.285 s | 0.349 s |

Both runs report incomplete, no errors, unchanged request/source/binary
hashes, and no outer-memory, PMF-cache or suffix-capacity limit. The prepared
table has 69691 states; the regime has 166182 states and a complete
92975-state accepted forest. Source counts, selected rivals, regime atoms,
`B=1.335714790055119e-32`, and `H=1.304408974663202e-35` match.
Treatment setup uses 14260992 work units, with no timeout, invalid source,
support limit or work limit. Thus missing setup is not the cause of the lack
of overall improvement.

The preliminary 20-second-search smoke returned in 20.550 seconds with no
errors. It prepared all sources in 0.529 seconds but found no forced-tie
applications. Its source-hash stability check is false because test-only
source fixes were still being completed during the smoke. It is a functional
check, excluded from the matched comparison. Both full runs use the final
unchanged source and binary.

## Interpretation and next experiment

The selector chooses team 67 for all 156 sources. It starts with 33 points,
9 wins and GD -14, versus Palmeiras' fixed final 57 points, 16 wins and
initial GD 26. Its exact tie requires seven wins and three draws in its ten
remaining matches. It has other W/D/L patterns that put it strictly ahead on
points or wins, where a GD comparison is unnecessary. Ranking candidates
by the initial GD deficit favors a potentially strong bound, without
measuring how much unresolved residual mass actually requires that bound.

There are 818 forced-tie evaluations among 39645580 bound evaluations,
0.002063%. Every such evaluation tightens its local score upper; the
strongest is about 29.93x. These are applications, not unique branches, and
the receipt does not measure the residual mass removed by each application.
The small activation fraction alone does not prove a small mass contribution.
The measured full-budget result nevertheless shows no overall gain.
Treatment expands 0.413% fewer outer nodes and returns a 0.289% higher
upper. This single wall-time pair does not isolate mathematical gain from
setup, per-branch overhead, queue ordering, probe timing or normal throughput
variation. It does not establish a meaningful regression either.

The control's largest pending branch contributes only 8.658117385e-45 to
its 5.004862589e-38 upper, about 1.73e-7 of the total. Its main frontier is
around depths 72-77. This is a broad residual tail, so a useful score rule
must apply across many surviving branches. We have not established whether
team 67's tied branches are mostly removed by PW infeasibility, covered by
the guided core, or simply not selected for expansion before the deadline.
No claim that its last fixture occurs late is supported by these receipts.

The next bounded experiment should prepare two or three candidate rival
models per source within the existing setup allowance, then take the minimum
applicable score bound. Their bounds must not be multiplied: their score
events share target matches and existing pair conditioning. Record residual
bound mass classified as strict, tied or undecided for each rival, and the
mass removed by tightening, so candidate selection can follow coverage as
well as potential GD strength.

A later alternative is an explicit strict-PW/tied-PW partition:
`U <= min(old_U, U_strict + GD_factor*U_tie)`. Each term needs a valid bound
on the weighted residual in that partition. A nominal probability of a tie
cannot scale the old bound without accounting for likelihood correlation.

## Relation to the historical full-cell interval

Holding the old lower endpoint, other cohort uppers and saved final-look core
upper fixed, the residual upper would need to be at most
3.3516004661537957e-38 to satisfy the adopted adaptive qualification rule.
Treatment is still about 1.4976x above that residual threshold. Adding its
deterministic residual bound to the actual core upper
1.1463390418238276e-39 and other cohort uppers 1.5530736543769992e-37 gives
a full upper of approximately 2.066470765763051e-37, above the required
1.899697091410617e-37.
This is a post-hoc accounting comparison, not a new 180-second full-cell
estimate: the historical sampling and this separate audit consume different
time budgets. No additional cell has been demonstrated qualified.

Accounting correction: an earlier version reported 2.055085180250817e-37
by subtracting a residual upper from a full-cohort upper supplied by the
baseline comparator. These were separate bounds, so that subtraction was
invalid. The source, standalone deterministic certificate and matched
measurements are unchanged. The corrected combination uses core plus
residual explicitly; see [REMAINING_MASS_ANALYSIS.md](REMAINING_MASS_ANALYSIS.md).

## Reproducibility and backup

Source is backed up in commit
`6a41a17a47ba524189c0ef05e281821c6fc7d31f` on
`experiments/rare-odds-recovery-2026-10-17` in `galo2099/golaberto`.
Its Git tree is `2e3525449ed179baf0f9756b152d8fad81f809fa`, verified equal
to the local staged tree before this report was added. Raw JSON stays local.

| Artifact | SHA-256 |
| --- | --- |
| Historical request | `cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee` |
| Canonical request | `40b8ab70b9fc464d154e5123d527a0b62f31d4a47b8e71a464c8b51c1dfe4544` |
| Frozen binary `/tmp/frontier-recovery-score-v7` | `1992292bac6977b6f8414857df0f7359b0dfea32bf52c93597a42b1f06b8aac5` |
| Aggregate source/binary manifest | `15b95cd3ac107c515306e898c2070668ab6f5ac7bb3ba9eafaa23a8d7fa6810c` |
| `score-gd-control-pal16-180s.json` | `570361c74f8cc32e6b9fc84e8295c2e91e74123ca8719438e9296e34bb77d5af` |
| `score-gd-pal16-180s.json` | `921e83df5b926deae2de7f832e6845b5bab231aa8190bedade7526bd6d7b4ab8` |
| Preliminary `score-gd-smoke-pal16.json` | `71b5a042d32acb3a1c498031e9a8b0cab05d074d9b0d1318891b0998f3d74c8f` |

The aggregate manifest includes the binary path and all experiment Python,
Rust, Cargo.toml and Cargo.lock contents. The full manifests are in the local
receipts. To reproduce the two benchmark arms, use `residual_audit.py` with
the frozen binary, `--suffix-bounds --suffix-max-remaining 56
--suffix-canonicalize-certain --budget 180 --cleanup-reserve 10
--node-limit 100000000`, adding `--score-gd true` for treatment.
