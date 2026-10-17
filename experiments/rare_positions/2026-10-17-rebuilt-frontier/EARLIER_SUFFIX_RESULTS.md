# Moving the residual suffix proof earlier

This follows the guaranteed-team cache-key experiment in
[CERTAIN_TEAM_KEY_RESULTS.md](CERTAIN_TEAM_KEY_RESULTS.md). It tests the same
frozen implementation with a larger maximum remaining-fixture window. There
are no source changes, new proposals, production changes or new simulations.

The best returned certificate is 5.944852e-38 in 130.776 seconds: about 35000
times tighter than the preceding reduced-key result, and 11.859 times below
the historical statistical residual upper. It still falls short of the
historical full-cell qualification target. A full-length 56-fixture attempt
times out; explicit shorter search caps recover useful certificates within the
180-second allowance.

## What the previous improvement actually did

The historical Palmeiras 16th-place request has 279 played and 101 remaining
matches. The target must lose all ten remaining matches. This audit isolates
cohort 5 and keeps its existing k=9 joint points/wins proposal and compact
156-root, team-584 GD-conditioned regime (90 tied and 66 strict prefixes).
The outer search follows the regime's remaining 91-fixture W/D/L suffix.
Shared fixtures stay shared in the actual assignments and proposal likelihoods.

Let q0 be the baseline proposal and qg the sequentially guided proposal. On a
full path, including the fixed regime prefix factors, define:

```text
V = native mass / q0
G = qg / q0
H = B / 1024
residual observation = max(V - H*G, 0) * S
```

S is the remaining conditional score/ranking probability. Fixed GD regime
factors are retained in V; the certificate conservatively bounds the remaining
S by 1. The guided sampler represents the clipped core separately. A branch
with zero residual can therefore be removed from the residual search while
its probability remains represented in that core. This is not a proof that
the position is impossible.

For each outer branch the suffix solver computes a lower bound on the minimum
future G over every feasible completion of the necessary points/wins intervals.
It replays actual categorical ticket probabilities, not idealized probabilities.
The branch can be removed when:

```text
H * G_prefix_lower * G_future_min >= V_cap
V_cap = min(B, V_prefix_upper * future_native_weight_cap)
```

The threshold V_cap/H is at most 1024. A reported future G minimum alone is
not enough: the prefix G and weight cap matter too. Unknown suffixes contribute
no positive guide lower bound. Incomplete work remains in the outer frontier.

The old exact-total cache distinguished scores even after the relevant
points/wins interval was guaranteed. Its roughly 1M completed descendants did
not finish a single probe root. Canonicalizing such guaranteed totals preserves
future guide probabilities and made root proofs complete. At window 28 this
produced 607269 guide residual prunes and reduced the final expectation upper
from 4.107121e-33 to 2.080539e-33. It did not materially change the uniform
observation cap B or the statistical interval.

## Why window 28 stopped helping

The reduced-key run left only 4.245227e-37 of upper bound inside its own
28-fixture window: 0.0204% of the remaining 2.080539e-33. It had already solved
nearly all of that portion. Increasing the number of probes at the same window
would mostly miss the branches carrying the remaining bound.

The saved frontier shows where a wider window could act:

| Maximum remaining fixtures | First eligible depth | Share of window-28 remaining upper |
| --- | ---: | ---: |
| 28 | 63 | 0.0204% |
| 32 | 59 | 15.4833% |
| 36 | 55 | 23.1645% |
| 40 | 51 | 35.9845% |
| 48 | 43 | 76.8207% |
| 56 | 35 | 90.0252% |
| 64 | 27 | 99.2201% |

These are shares of conservative upper bounds, not true probability shares.
They are not ceilings on a fresh run's improvement: earlier proofs change
pruning, queue priorities, cache reuse and subsequent work allocation.

There was also spare cache capacity (248098 of a 1M target), and completed
probe work took only 0.729 seconds. This justified testing earlier proofs
before changing the proposal, clipping threshold or confidence method.

## Protocol

Every arm starts from the same request and regime, runs serially on the same
frozen binary/source, and has a separate 180-second total allowance. Setup and
finalization are included, with a ten-second cleanup reserve and 100M outer
expansion limit. Compilation is excluded because the binary is unchanged.
There are no competing benchmark processes.

All arms enable certain-team canonicalization. The only experimental setting
is maximum remaining fixtures: the preceding 28-fixture result is the control;
this follow-up measures 36, 48 and 56. The cache target stays 1M, local probes
stay at 25000 new visits/25 ms, and the cumulative probe allocation stays 15%.
The limits are best effort. Diagnostics and actual wall times are retained.

The 36-fixture result was inspected before proceeding to 48. Its continuing
cache headroom and useful proofs justified the second arm. The 48-fixture
result then justified testing 56, which reaches the newly dominant depths.
This is an adaptive tuning experiment on one fixture, not a general coverage
or latency benchmark.

## Results

| Search window | Native search cap | Total wall | Residual expectation upper | Cache entries | Guide zero-residual prunes |
| --- | ---: | ---: | ---: | ---: | ---: |
| 28, preceding control | about 169 s | 172.218 s | 2.080539e-33 | 248098 | 607269 |
| 36 | about 169 s | 172.884 s | 9.561634e-34 | 415142 | 785069 |
| 48 | about 169 s | 171.176 s | 1.113673e-35 | 739170 | 2111145 |
| 56 | about 169 s | 180.135 s | **No certificate: harness timeout** | unavailable | unavailable |
| 56, diagnostic | 60 s | 62.343 s | 1.094869e-37 | 928272 | 642736 |
| 56, capped follow-up | 120 s | 130.776 s | 5.944852e-38 | 928273 | 1977804 |

Window 36 improves the upper by 2.176 times relative to 28. Window 48 improves
another 85.857 times, or 186.818 times overall. Both remain within the shared
allowance and have no errors or hash changes. They finish almost all probe
roots and leave the cache below its 1M target.

At window 36, 3248 probe roots return positive guide minima, 2480 are proven
PW-impossible, and 9 remain unknown. Probe work totals 1.155 seconds. At window
48, those counts are 15407, 12783 and 15, with 2.090 seconds of probe work.
The 48-fixture search performs 4956181 outer PW-impossible prunes and 2383
additional branch-bound tightenings. These counters are distinct from
internal suffix rejects and completed outer leaves (zero in both runs).

The full-length window-56 run reports `TimeoutError: shared run budget
exhausted`, no certificate, and a 0.135-second harness overrun. Its request,
source and binary hashes remain unchanged. Without a receipt, neither its
bound nor its cache occupancy or exact timeout cause can be inferred.

To recover diagnostics, a fresh window-56 run uses an explicit 60-second
native search cap inside the same 180-second shared allowance. It returns
successfully after 62.343 seconds with no errors or hash changes. It retains
928272 completed cache rows, makes 7190128 cache hits, and completes 5923
positive and 5785 impossible probe roots, leaving 19 unknown. Probe work takes
2.987 seconds; the capacity guard is not hit. Its outer search expands
7431380 branches, retains 837107, and makes 1375472 PW-impossible prunes.

The short diagnostic is already 101.717 times tighter than the completed
window-48 result and 19002.630 times tighter than window 28. Its bound is
6.439 times smaller than the historical residual statistical upper. This is
a real deterministic certificate within the allowance, even though the
longer window-56 attempt failed to return one.

It also shifts the bottleneck: 99.8423% of its remaining bound is inside the
old 28-fixture window, with the largest contributions at depths 70--74.
That differs sharply from the earlier result, where virtually all uncertainty
lay outside that window. Extending the eligibility window again is therefore
not directly supported by this latest frontier. These diagnostics do not show
how much of the late tail is genuine residual probability, separate-extrema
slack, incomplete joint PW information, or the remaining score/ranking factor.

The 120-second follow-up keeps the 180-second shared deadline but deliberately
ends native search earlier, leaving time to retrieve the certificate. It is
a separate scheduling treatment, not a replay or continuation of the failed
run. Its final outcome is recorded below.

The 120-second search returns a stronger bound in 130.776 seconds, with no
errors, hash changes or total-wall overrun. It improves the 60-second result
by 1.842 times, the window-28 result by 34997.318 times, and the historical
statistical residual upper by 11.859 times. The maximum residual observation
cap also drops meaningfully, by 22.499%, from B=1.335714790055119e-32 to
1.0351934325747053e-32. This cap improvement is distinct from the much larger
improvement in the expectation upper.

The cache retains 928273 rows, essentially unchanged from the 60-second run,
and its capacity guard remains clear. There are 5923 positive, 5785 impossible
and 18 unknown probe roots. Probe work totals 2.746 seconds. The outer search
expands 12301217 branches, performs 2734072 PW-impossible prunes and 1977804
guide zero-residual prunes, and retains 7846678 pending branches. No outer leaf
is reached. More cache capacity is not the demonstrated bottleneck here.

The frontier grows more than ninefold relative to the 60-second diagnostic,
while the upper improves by only 1.842 times. At the final snapshot, 99.99915%
of the bound is already inside the old 28-fixture window; depths 72--74 carry
72.391%. The remaining task is to tighten this large collection of late
branches, rather than merely enabling earlier suffix probes.

The requested search cap is 120 seconds. The inner certificate records
127.297 seconds, the native audit records 128.281 seconds, and the complete
protocol command takes 129.649 seconds. These timers cover different layers,
including unbudgeted final processing and cleanup; there is no separate timer
for the exact finalization cost. This result demonstrates why leaving more
headroom can matter. It does not identify the precise cause of the failed
full-length run.

The full timeout may involve post-search work: the current implementation
refreshes cached bounds across the entire retained frontier, rebuilds the heap,
collects per-branch diagnostics and sorts them before keeping only the top
five. These operations and destruction of retained state are not hard-budgeted.
A larger frontier could make the ten-second reserve insufficient. This is a
source-level possibility, not an established diagnosis of the failed run.

## What remains loose

The outer native-event bound uses the minimum of necessary single-team
marginals rather than multiplying them. This is sound despite shared matches,
but it can be loose because no one marginal captures every joint requirement.
Suffix proofs recover some of that lost joint feasibility by exploring actual
shared assignments. The remaining score/ranking factor is still bounded by 1.
These are separate sources of conservatism; this audit does not measure how
much of the gap each contributes.

The suffix guarantee is a worst-case guarantee over all feasible completions.
A rare completion with a low guide likelihood can keep the minimum low. It
cannot be discarded based on rarity alone without separately accounting for
its contribution. A larger window is useful only when these complete proofs
remain tractable and strong enough to tighten branches.

The certificate also combines separate extrema: a large native weight and a
small guide ratio can occur on different completions. A possible subsequent
experiment would cache a joint future cap:

```text
C(state) = max over feasible completions of product(p_i / qg_i)
K = upper(V_prefix / G_prefix) * C(state)
K <= H implies zero residual
```

Here p_i is native W/D/L probability and qg_i is the actual guided ticket
probability. The q0 factors cancel on the same path. This preserves correlation
between V and G that separate extrema lose. The canonical keys remain valid
because they preserve future guidance and PW feasibility. Feasible terminals
have C=1; impossible children can be excluded; unresolved children require an
upper bound such as infinity, never zero. Divisions and products must round
outward, and a zero prefix-G lower bound prevents division. Retain the existing
B-based certificate and take the tighter valid result; unconditional numerical
dominance is not guaranteed. This joint-cap idea was mathematically reviewed
but is not implemented or measured in the present experiment.

## Statistical interpretation

The historical lazy-hybrid residual interval upper for this cohort is
7.049822405827536e-37. The earlier full-cell interval was
[2.589898304031079e-41, 8.60297386511054e-37], unqualified under the adopted
adaptive log-width rule. The no-hit residual tail still uses the roughly
1.335715e-32 uniform observation cap.

A deterministic expectation upper can tighten that residual upper only when
it is smaller. The 36- and 48-fixture results remain coarser and do not help
that interval. Both successful 56-fixture runs are smaller and could tighten
it if their certificates were combined with the matching historical hybrid
decomposition. Deterministic bounds do not spend additional confidence error.
Reducing aggregate branch mass is distinct from reducing the uniform
observation cap in its binomial no-hit confidence calculation.

Holding the earlier lower endpoint and other contributions fixed, cohort 5
would need an upper of about 3.466234e-38 to qualify that historical full-cell
interval. This is a diagnostic target, not a new confidence calculation. The
old 179-second sampling run cannot be combined with this additional audit and
called a 180-second end-to-end result. Keeping its other contributions fixed,
substituting the best certificate would produce an illustrative full upper
of 2.1476366612658992e-37, still above the approximately 1.899697e-37 allowed
with that historical lower endpoint. The new residual bound is 1.715 times
above the 3.466234e-38 diagnostic target. This is progress on the actual tail
certificate, not a new qualifying 180-second estimate. None of the audits
samples campaigns or establishes new cell coverage.

The next justified implementation experiment is to tighten the late-branch
certificate using the joint V/G cap described above, while retaining the
existing bounds. Budget-aware finalization should accompany further long
runs: retain only five diagnostic leaders during aggregation rather than
sorting every branch, and allow optional cache refresh to stop safely while
retaining the old bounds. A larger suffix window or cache is not supported
as the first next change by the best run's occupancy and depth breakdown.
Bounding the remaining score/ranking factor is another possible improvement,
but the current output cannot quantify its benefit separately.

## Frozen artifacts and reproduction

Binary SHA256:
`a9a86294eaff54e91d2ec09fb55c9409fd474b27b25984d36c8ceda725860268`.
Source aggregate SHA256:
`903b4ff4a15418108526af554ad8202227a846b2b483355ba939a4fdaddd3a88`.
Source backup commit:
`ba6f22e2643e208b4c7f53fe7761aad22e397d3c`.
Request SHA256:
`cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee`.
Canonical request SHA256:
`40b8ab70b9fc464d154e5123d527a0b62f31d4a47b8e71a464c8b51c1dfe4544`.

The unchanged source previously passed all 54 optimized Rust and 57 Python
experiment tests. No new tests were needed for these CLI setting changes.
Independent review checked the source reasoning and measured outputs.

From this directory, repeat for windows 36, 48 and 56 by changing only
`--suffix-max-remaining` and the output name:

```bash
python3 residual_audit.py \
  --binary /tmp/frontier-recovery-certain-v5 \
  --suffix-bounds --suffix-max-remaining 36 --suffix-canonicalize-certain \
  --budget 180 --cleanup-reserve 10 --node-limit 100000000 \
  --output certain-team-key-gate36-pal16-180s.json
```

Raw JSON remains local; source and aggregate reports are backed up. Receipts:

| File | SHA256 |
| --- | --- |
| `certain-team-key-pal16-180s.json` | `18a1db9ccef26927d5fed851aa3024804f724263e6fd116c13e717b08f51e046` |
| `certain-team-key-gate36-pal16-180s.json` | `b972fe07f685f240ac8dc6e602331466fab113b8a70dfb8bed62e703c971e3d2` |
| `certain-team-key-gate48-pal16-180s.json` | `f4c4c6214357744914db7a2d6778e2cd3c45a35cf1b50039fe548eb0cb4491e8` |
| `certain-team-key-gate56-pal16-180s.json` | `9eb5f090381692f69e17322a8d8a9de9b02661434e7b6ac58ad4a1a50c7edfc1` |
| `certain-team-key-gate56-diagnostic60s-pal16.json` | `8ea558c06123e9801916de2ef2bc507c29e3a4ec0fb3dd601ddad0cefedc7497` |
| `certain-team-key-gate56-capped120s-pal16-180s.json` | `1ccbc2427b81c33580ee54858b2f33739f0fa1bd35068c385ef026a73f10b332` |
