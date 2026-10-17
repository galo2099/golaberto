# Bounded residual certificate audit

This experiment tests whether deterministic search can tighten the hybrid
sampler's unseen residual tail for historical Palmeiras finishing16th,
cohort5. It is a nonmutating audit of the installed proposal. The ordinary
sampler and production endpoint do not invoke it.

For baseline proposal `q0`, guided proposal `qg`, native importance weight
`V=p/q0`, guide ratio `G=qg/q0`, certified cap `B`, clip threshold `H=B/1024`
and conditional score probability `S`, the residual is
`max(V-H*G,0)*S`. A complete path with a certified `H*G >= V` has zero
residual. Observed guide ratios cannot certify unobserved paths.

The search partitions the actual proposal's W/D/L paths. Every unfinished
branch retains a bound; timeout and capacity limits never discard its mass.
Full team totals are carried with each prefix, since the selected-team joint
table can merge prefixes with different guidance states. Fixed forest prefixes,
actual mixture ticket probabilities and score-conditioning factors are included.
An unresolved proposal edge keeps its original masked native suffix.

The initial prototype uses `G_min=0` for unfinished branches. At complete paths,
it evaluates the actual guided ratio using integer-ticket probabilities. This
can prove zero residual at leaves; it does not yet provide a strong guide-ratio
lower bound over an unfinished subtree.

A branch also has a native event-mass bound. Independent fixture laws provide
interval bounds on each team's possible final points/wins. The minimum team
completion bound is a valid upper bound for their joint event; multiplying the
team probabilities would assume independence despite shared fixtures. Goal
difference and later ranking criteria are conservatively bounded by `S <= 1`.

The audit reports both the largest possible residual observation and the sum
of possible residual contributions over the remaining partition. These are
deterministic upper bounds, not Monte Carlo estimates or additional confidence
intervals. A useful deterministic bound could intersect a simultaneous99%
statistical interval without spending additional failure probability.

## Decision threshold

The previous lazy hybrid run ended at
`[2.589898304031079e-41,8.60297386511054e-37]`, with cohort5 upper
`7.04990021073354e-37`. Holding that lower bound and all other cohort uppers
fixed, the adopted adaptive qualification rule requires total upper at most
`1.899697091410617e-37`, leaving cohort5 at most
`3.46623437033617e-38`: a20.34x reduction. This is a conditional target,
not a prediction of a new request run.

## Measurement

The native suite passes45 tests, including five focused certificate tests for
exhaustive tiny-path residual sums, retaining all sources at an expired
deadline, partial-table fallback retention, unselected-team feasibility, and
fixed forest-prefix totals. All54 Python experiment tests pass. An independent
source review found no remaining soundness blockers. The search deadline is
best effort during an individual guidance/PMF operation and final aggregation;
the harness reserves time for the result receipt and records any wall overrun.

The audit uses the historical279-played/101-remaining request, SHA256
`cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee`.
It prepares cohort5 at k9 with250000 states, then rebuilds the compact regime
with250000 total retained states and1000ms per setup attempt. The installed
proposal has156 roots and the same cap as the prior lazy hybrid:
`B=1.335714790055119e-32`, `H=1.304408974663202e-35`.

Only cohort5 is prepared and searched. This gives it nearly the entire request
budget; it is not a full-request A/B timing or a new statistical interval.
There are no simulation draws in this audit. Compilation finishes before
timing. No HiGHS pruning is used.

Frozen binary SHA256:
`9b6fe0d5b9d398ba8c858525d0f1f22aaa4eb01e49c3e7dbffc0bca71bdea825`.
Implementation backup commit:
`1c01f9f95688fa4fa803f3b67d9a18ee0841f7d3`.

The short smoke reached its10000 pending-branch capacity guard after7341
expansions, in1.156 seconds including setup. It returned a conservative
unfinished result with no errors and unchanged source/input/binary hashes.

The first larger audit used a180-second allowance but stopped at2M expansions
in14.805 seconds, with1040666 pending branches and no completed paths. Its
deterministic residual expectation upper fell7.41% to
`1.2367087965402353e-32`; the maximum residual observation cap remained
`1.335714790055119e-32`. No PW-impossible or zero-residual branches were pruned.
The PMF cache and pending-branch capacity guards were not hit. Pending suffix
depths ranged0 through67; fixed forest-prefix fixtures are excluded from this
depth. The largest pending branch still involved13 teams with remaining
fixtures. This result justified raising the expansion limit to30M for a timed
180-second audit, with a five-second finalization reserve.

The30M audit also stopped on its expansion limit, in147.644 seconds including
setup. Its upper fell65.62% to`4.591985643975442e-33`, with4772880 pending
branches at depths15 through69,3021 PW-impossible prunes and no complete paths.
Neither capacity guard was hit. The maximum residual cap was unchanged. This
upper still exceeds the previous residual statistical upper by about6514x,
so it provides no statistical interval or sampling-cap improvement.

The final180-second run raises the expansion limit to100M, retaining the
five-second finalization reserve. It stops on time with36588101 expansions,
in177.810 seconds including setup and finalization. It reports no overrun,
no errors and unchanged input/source/binary hashes. Neither capacity guard is
hit. Source aggregate SHA256:
`e04a7549133f401aa86c6fc2ed6701b910653835121d59248565a564791eac19`.

| Audit | Wall seconds including setup | Expansions | Retained branches | Deterministic residual expectation upper | Maximum residual observation cap |
|---|---:|---:|---:|---:|---:|
|2M limit|14.805|2000000|1040666|1.2367087965402353e-32|1.335714790055119e-32|
|30M limit|147.644|30000000|4772880|4.591985643975442e-33|1.335714790055119e-32|
|180-second allowance|177.810|36588101|5481807|3.7346398883320757e-33|1.335714790055119e-32|

The final expectation upper is72.04% below the initial deterministic envelope,
but remains5297x above the previous residual statistical upper. The maximum
residual observation cap is unchanged. Consequently neither the deterministic
intersection nor a smaller sampling cap can improve the existing statistical
interval. There is no qualification or coverage gain demonstrated here, and
the audit is not enabled in the request sampler.

The final run proves8221 prefixes PW-impossible, reaches no complete W/D/L
paths, and therefore applies no guide-based zero-residual pruning. Pending
depths range15 through69 of91 suffix fixtures; the10 fixed pivot-prefix
fixtures are outside that depth. The largest pending branch has upper
`1.3647967704908615e-39`, depth65 and11 teams with remaining fixtures. Millions
of such small branch bounds still sum to a large remainder. Depths62 through66
account for about47.24% of the final deterministic upper; depth43 adds9.12%.

The search call requests173933ms after setup. Its reported native elapsed is
175032ms; final aggregation and freeing the retained frontier add work beyond
the search deadline. The engine receipt arrives in176712ms, and the harness
finishes inside the full180-second allowance. The five-second reserve works
for this run; this is best-effort budget behavior, not a hard latency guarantee.

The2M certificate is about17542x above the prior residual statistical upper
`7.049822405827536e-37`. Intersecting those two bounds leaves the statistical
interval unchanged. The7.41% reduction is relative to the initial deterministic
envelope, not a7.41% improvement in the Monte Carlo interval.

Reproduction, from this directory:

```bash
python3 residual_audit.py \
  --binary /tmp/frontier-recovery-residual-v3 \
  --budget 180 --cleanup-reserve 5 --node-limit 100000000 \
  --output residual-certificate-pal16-180s.json
```

The harness defaults to this historical fixture and cohort5. The native
certificate operation is generic over installed joint/regime proposals and
does not hardcode Palmeiras, team IDs, a rank, or the number of forest roots.

## What this does and does not test

The retained partition and its bounds are valid, but this first implementation
is not a strong unfinished-subtree guide certificate: its `G_min` remains zero
until a complete path. Best-first expansion can spend the entire budget
widening the frontier. No leaves therefore does not establish intrinsic
infeasibility, inefficient guidance on feasible paths, or failure of all
possible residual-certification algorithms.

The next useful operation would compute a cached suffix guide-ratio minimum
using the actual base node/fallback state and exact totals of teams with
remaining fixtures. A bounded depth-first completion search can produce those
bounds while best-first search still decides which uncertainty to investigate.
Finalized teams can leave the cache key after their necessary PW conditions
are checked. In a native masked suffix, disconnected remaining fixture
components can be solved separately after verifying the proposal factorizes.
Unfinished cache entries must retain a zero lower bound. These changes are
proposals, not functionality implemented or measured in this experiment.

## Local raw artifacts

Raw JSON stays local. Source and this aggregate report are backed up.

```text
residual-certificate-smoke-pal16.json  fee813269d755f9d26f9ab734f1661cbc31aa2c475a828aa47b8aebe6e33e0d9
residual-certificate-pal16-2m.json     c7a861cfe3232d2723115bc58d84e0bd7c07adf1141275d4ae4bfcdcaa8214c8
residual-certificate-pal16-30m.json    c906fe1d251ec71fda2491ef842304dcce6b0392b972e922841be0415468272b
residual-certificate-pal16-180s.json   da1c9b7ec12b5be78d95e1993728e5e7166484dcdede6c73a30c58859592b3b8
```
