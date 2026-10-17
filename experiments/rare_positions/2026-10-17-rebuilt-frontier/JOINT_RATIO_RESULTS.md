# Joint native/guided likelihood certificate

This experiment follows [EARLIER_SUFFIX_RESULTS.md](EARLIER_SUFFIX_RESULTS.md).
It adds an opt-in joint suffix certificate to the standalone frontier recovery
experiment. Production and all sampling proposals remain unchanged.

The joint certificate is sound, but does not materially improve this case.
The full-budget upper is 2.44% lower, while a fixed-five-million-expansion
comparison improves by only 0.00052% and takes 4.13% longer. The complete
baseline forest makes the native-to-baseline weight nearly constant, leaving
little correlation loss for this certificate to repair. The option remains
off by default. Finalization changes allow both full-length runs to return
within the allowance.

## Certificate and guarantee

Write the native-to-baseline weight as V and the guided-to-baseline likelihood
ratio as G. The hybrid residual observation is `max(V-H*G,0)*S`, where S is the
remaining conditional score/ranking probability and is bounded by 1 here.
Fixed regime/pair-score factors remain in the prefix V. The guided core
represents the clipped part separately; pruning zero residual does not prove
the finishing position impossible.

The existing certificate combines a maximum future native-to-baseline weight
with a minimum future G. These extrema can arise from different completions.
The new cache instead also computes:

```text
C(state) = max over PW-feasible baseline-supported suffixes of product(p_i/qg_i)
K = upper(V_prefix / G_prefix) * C(state)
K <= H implies zero residual
otherwise residual <= V_cap * upper(1 - lower(H/K))
```

Here p_i is the native outcome probability and qg_i is the actual guided
integer-ticket probability, computed from the same real score totals used by
sampling. The baseline proposal probabilities cancel on each same path.
Feasible terminal C is 1; impossible children are excluded; incomplete proofs
remain unknown and uncached. A zero guided-probability lower bound, overflow
or unavailable joint cap leaves the new bound unavailable. Existing min-G
proofs remain usable. Products, divisions and subtraction round outward.
The application takes the tighter of this certificate and the old bound,
including the old global B cap.

C is cached with the existing immutable plan, fixture position, node/fallback
and guaranteed-team key. Canonicalization preserves future guidance and PW
feasibility; native fixture probabilities do not depend on accumulated totals.
Source-specific mixture, native and fixed-score prefix factors remain outside
the cache. No team independence assumption or unverified solver rejection is
introduced. The option defaults off (`--suffix-joint-ratio` requires
`--suffix-bounds`).

## Finalization changes shared by both arms

The search reserves at most one second, and at most 10% of its remaining
native allowance, for optional cached-bound refresh. Refresh performs no new
suffix DFS. Once its deadline arrives, every untouched branch retains its
existing valid bound. Mandatory aggregation still sums every pending branch.
No uncertainty is discarded to satisfy a time limit.

Finalization uses a streaming top-five diagnostic list rather than allocating
and sorting a record for every pending branch. Remaining-team scans run only
for admitted leaders. It also avoids rebuilding a heap after search is over.
Receipts report applied/skipped refresh counts, refresh milliseconds and
aggregation milliseconds. Moving/freeing retained state and mandatory O(N)
aggregation remain best effort; this is not a hard total-wall guarantee.
Both benchmark arms include these changes so the matched difference isolates
the joint certificate.

## Validation

All 57 optimized Rust tests and 58 Python experiment tests pass. Independent
source review found no outstanding blockers. The exhaustive joint test walks
all 27 paths in a three-fixture model through actual memo-node weights/edges
and sticky native fallback, with full and state-cap-1 proposals. It verifies
varied guidance, an actual fallback path, both canonicalization modes, the
joint suffix cap and enabled/disabled residual mass enclosures under two
source native-factor/mixture scalings. Other checks cover strict improvement
on separated extrema, zero/nonfinite prefix guidance, overflow, interrupted
refresh retaining all mass, and top-five tie ordering against full sorting.
JSON and Python validation verify strict booleans and default-off forwarding.

A preliminary 20-second search smoke finishes in 20.518 seconds, returns no
errors and has unchanged hashes during that run. It reports 92610 joint
bound-tightening applications, 4211 finite joint probe-root bounds and 889039
cache entries without hitting capacity. This smoke predates the final added
test/comment edits and is a functional check, not part of the frozen timing
comparison below.

## Matched protocol

Both arms run serially, with no competing heavy jobs, on the final frozen
binary and source. Each has a 180-second shared allowance, including setup,
with a ten-second cleanup reserve and a 100M outer expansion limit. The suffix
window is 56 fixtures, cache target 1M states, per-probe limits 25000 visits
and 25 ms, cumulative probe allocation 15%, and guaranteed-team keys enabled.
The only arm-specific setting is `--suffix-joint-ratio`.

The fixture is historical Palmeiras finishing 16th, cohort 5, with 279 played
and 101 remaining matches. Both prepare k=9 at 250000 states and install the
same compact team-584 regime with 156 roots (90 tied, 66 strict). The prepared
joint table has 69691 states and the regime 166182. The audit explores the
remaining 91-fixture W/D/L suffix. There are no simulations or new confidence
intervals in either arm.

| Metric | Existing certificate | Joint certificate enabled |
| --- | ---: | ---: |
| Total wall time | 174.131 s | 174.609 s |
| Total-wall overrun | 0 s | 0 s |
| Residual expectation upper | 4.947368143e-38 | 4.826735150e-38 |
| Maximum residual weight cap | 1.035193433e-32 | 1.035193433e-32 |
| Outer expansions | 15603682 | 16141340 |
| Pending branches | 11871499 | 12494771 |
| Outer PW-impossible prunes | 4144187 | 4395449 |
| Guide zero-residual prunes | 3146456 | 3347114 |
| Additional joint zero bounds | 0 | 0 |
| Joint cap-tightening applications | 0 | 32990401 |
| Cache entries | 928273 | 928273 |
| Positive / impossible / unknown probe roots | 5920 / 5785 / 14 | 5920 / 5785 / 14 |
| Finite joint probe-root caps | 0 | 5920 |
| Cumulative probe time | 2.746 s | 2.426 s |
| Final refresh time | 1.661 s | 1.734 s |
| Mandatory aggregation time | 0.280 s | 0.319 s |
| Refresh applied / skipped | 1188079 / 10683420 | 1163835 / 11330936 |

Both arms report `incomplete`, no errors, unchanged request/source/binary
hashes, no total-wall overrun, no outer memory limit, no PMF cache limit and
no suffix capacity limit. Their regime atoms, prepared table and source count,
B=1.335714790055119e-32 and H=B/1024 match. The accepted pivot-584 candidate
explicitly reports `forest_complete: true`, not merely regime completeness.
No outer leaves are reached.

The joint-enabled upper is 2.4383% lower. It also completes 3.4457% more outer
expansions within the allowance, so this one wall-time pair does not isolate
the mathematical gain from throughput and queue-order differences. The
maximum cap changes only at rounding scale. The roughly 33M tightening count
records applications, which can repeat on a branch; it is not a count of
unique branches, materially tighter expectation bounds or new zero proofs.
The expectation bound also takes a minimum with the native PW marginal bound,
which can remain limiting even when a residual cap decreases.

Refresh skipped counts combine unavailable cached bounds and deadline skips;
they do not count deadline skips alone. Mandatory movement of all retained
branches remains in the refresh timer, so the phase can exceed the one-second
lookup reserve. Every retained branch is still included in final aggregation.
The control's 0.280-second aggregation covers all 11.9M pending branches.
This full-length control returned successfully where the preceding version's
full-length 56-fixture attempt had timed out. This is one observed comparison,
not a general latency guarantee or an isolated measurement of every change.

## Fixed-expansion diagnostic

To separate time-limited work completion from certificate strength, repeat
both arms serially with `--node-limit 5000000`. All other settings and the
180-second shared allowance stay the same. Both terminate at exactly 5M
expansions without reaching the time or frontier-memory limit.

| Metric | Existing certificate | Joint certificate enabled |
| --- | ---: | ---: |
| Total wall time | 37.188 s | 38.724 s |
| Expansions | 5000000 | 5000000 |
| Residual expectation upper | 4.7353724990384123e-36 | 4.735347850506292e-36 |
| Pending branches | 358152 | 358151 |
| Total outer prunes | 1313198 | 1313198 |
| Cache entries | 925865 | 925865 |
| Joint tightening applications | 0 | 391467 |
| Additional joint zero bounds | 0 | 0 |

The relative upper reduction is 5.205194e-6, or 0.0005205%. Measured wall time
is 4.131% higher with the option enabled. This is a single diagnostic pair,
not a statistically established runtime regression. It demonstrates that the
certificate adds negligible strength at this search effort despite hundreds
of thousands of positive tightening counters. Search priorities may differ
between the arms; equal expansion counts do not imply identical visited
branch sets. All hashes match and neither run reports errors or overruns.

## Why the improvement is small

The accepted baseline forest is complete for its selected PW event. In ideal
arithmetic its probability is native probability conditioned on that event.
From any represented suffix state s, let Z(s) be its native event mass. Then:

```text
q0(path | s) = native(path) / Z(s)
A(path) = native(path) / q0(path | s) = Z(s)
C(s) = max(A(path) / G_future(path)) = Z(s) / min(G_future)
```

A is constant across feasible paths from that state. The allegedly separate
extrema therefore have essentially no correlation gap: substituting C into
the joint residual bound recovers the old min-G expression. Floating proposal
construction, interval enclosures and integer tickets create small differences,
but this structure explains why the new formula can produce many tiny cap
reductions without any new zero proof. Masked native fallback suffixes have
a similar constant-weight property, apart from ticket rounding.

This does not establish that the feature is useless for incomplete proposals
or other fixtures where baseline weights vary substantially. Those cases
were not benchmarked here. It does establish that it is not a productive
default change for this complete-forest case.

## Statistical and next-step interpretation

The joint-enabled deterministic residual upper is 14.606 times below the
historical statistical residual upper of 7.049822405827536e-37. Recombining
it with the saved final-look core upper of 1.1463390418238276e-39 and the
other cohort uppers gives a full-cell upper of approximately
2.047210559766643e-37. This remains above the approximately 1.899697e-37
qualification threshold for the historical lower endpoint. The residual
upper is 1.4401 times the corresponding residual-only target of
3.3516004661537957e-38. This combines separate audits and historical samples;
it does not establish a qualifying new 180-second estimate.

Accounting correction: an earlier version reported 2.0358249742544096e-37
by subtracting the statistical residual upper from the retained full-cohort
upper. The latter was the baseline comparator, not the core-plus-residual
sum; subtracting those separate upper bounds is invalid. The standalone
deterministic audit and benchmark measurements are unchanged. See
[REMAINING_MASS_ANALYSIS.md](REMAINING_MASS_ANALYSIS.md).

The historical statistical interval and confidence allocation are unchanged.
All current operations are deterministic, conservative certificates, not
empirical mean/variance confidence heuristics or new simulations.

The finalization changes are useful and retained in the experiment. The joint
certificate remains opt-in. The next substantive target is a nontrivial upper
bound on the remaining score/ranking factor S in the late branches, preserving
the fixed GD-conditioned regime and shared-score dependencies. The present
audit takes S<=1, and its output does not quantify separately how much of the
remaining gap comes from score/ranking bounds versus unresolved joint PW
constraints. Increasing cache capacity has no demonstrated benefit here: both
full-budget runs retain 928273 rows below the 1M target.

## Frozen implementation and reproduction

Binary SHA256:
`849f6d54e19dc7597a36e21f5716d488e64df8d8086bc3f719b5fa1c85baad76`.
Source aggregate SHA256:
`ea2f675a98516a97e6cc30bb95cef0be73bbfd88717c3582e39f30d85cf3df50`.
Source backup commit:
`96ff2dec89195a68b7c830a4b6c087574784543c`.
Request SHA256:
`cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee`.
Canonical request SHA256:
`40b8ab70b9fc464d154e5123d527a0b62f31d4a47b8e71a464c8b51c1dfe4544`.

From this directory, run the control and then repeat with
`--suffix-joint-ratio` and a different output filename:

```bash
python3 residual_audit.py \
  --binary /tmp/frontier-recovery-joint-v6 \
  --suffix-bounds --suffix-max-remaining 56 --suffix-canonicalize-certain \
  --budget 180 --cleanup-reserve 10 --node-limit 100000000 \
  --output joint-ratio-control-pal16-180s.json
```

Raw JSON stays local; source and this aggregate report are backed up.

| Local raw artifact | SHA256 |
| --- | --- |
| `joint-ratio-control-pal16-180s.json` | `f2f8752ba6a3ffb6c4ddd74cfeaedcdfc5473f2f2be832d0b3ea3b603ba7f522` |
| `joint-ratio-pal16-180s.json` | `5badc49fd8a559660198d38db024322de687c71972d6b1b8871dba181df60136` |
| `joint-ratio-fixed5m-control-pal16.json` | `b67c79fd500440fdad8fdc171e72831b2cf832f064a79e0cb2a35014df8745af` |
| `joint-ratio-fixed5m-pal16.json` | `d1a841d82f0e8c3486b9a2bce6bea5ca22d6971b1a2bdb733310475d95e32a61` |
