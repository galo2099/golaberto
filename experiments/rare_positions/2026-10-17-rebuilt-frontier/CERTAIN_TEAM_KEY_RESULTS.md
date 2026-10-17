# Canonical cache keys for guaranteed points/wins requirements

Reducing the suffix cache key cuts the deterministic cohort-5 residual upper
bound by 49.34% in the matched 180-second audit: 4.107121e-33 becomes
2.080539e-33. Both arms finish within the allowance. This improves the search,
but does not yet improve the existing statistical estimate or qualify the cell.

## Sound reduction

An active team can have remaining fixtures while its necessary points/wins
requirement is already guaranteed. The new optional key represents such a
team with a distinct `Certain` tag; uncertain teams retain exact packed
points/wins totals. Tags preserve each team's position in the key. The actual
recursion keeps every team's real total.

Checked integer arithmetic computes the maximum remaining packed gain from
all three outcomes of every remaining fixture, ignoring outcome restrictions
and allowing zero gain. All gains must be nonnegative and representable;
unsupported arithmetic or self-fixtures disable the reduction. A team is
certain only if its current total is at least its lower bound and its current
total plus the maximum remaining gain is at most its upper bound. Additional
range checks prevent saturation.

This property persists after every allowed outcome. Every future endpoint
guidance calculation for that team selects its entire remaining marginal row.
Different certain totals therefore produce identical floating sums, clamps,
guidance weights and actual categorical ticket probabilities. The code
preserves the existing sum; it does not replace it with 1.

The key still includes plan identity, fixture position and the selected-table
node or sticky fallback. Finalized teams are validated before cache lookup.
Teams with negative gains, overflow risks or uncertain requirements keep exact
totals. No independence between teams is assumed.

Certainty refers only to the necessary points/wins interval used by guidance.
It does not prove final ranking after GD/GF ties. The audit continues to bound
conditional score probability by 1. A zero residual branch still has its guided
core represented in the hybrid; it is not a proof the cell is impossible.

## Matched comparison

The fixture is the historical Palmeiras 16th-place request with 279 played and
101 remaining matches, team ID 16, cohort 5. Its input is
`../reference/2026-10-04-two-results/inputs/group-16498-two-results.json`.
Request SHA256: `cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee`.
Canonical request SHA256: `40b8ab70b9fc464d154e5123d527a0b62f31d4a47b8e71a464c8b51c1dfe4544`.

Exact-total and reduced keys run serially on one frozen binary/source, with
the suffix solver enabled in both. Each gets a 180-second wall allowance,
including setup and finalization, a ten-second cleanup reserve, and a 100M
outer expansion limit. Both prepare cohort 5 at k=9/250000 states and install
the same compact 156-root regime. Their selected rivals, regime atoms, B and H
match. Suffix settings are 28 remaining fixtures, a 1M cache-state target,
25000 new visits and 25 ms per probe, and a 15% cumulative probe-time budget.
Limits are best effort; completed proof unwinding can slightly exceed the
cache target. Actual timings and overruns are recorded below.

The only intended difference is `--suffix-canonicalize-certain`. The optional
flag defaults off. Production and sampling proposals are unchanged. These are
deterministic certificate audits: neither arm runs simulations or produces a
new confidence interval. This fresh same-binary control is the comparison for
this experiment, rather than controls from earlier binaries.

| Metric | Exact totals | Guaranteed-team tags |
| --- | ---: | ---: |
| Total wall time | 173.274 s | 172.218 s |
| Wall allowance overrun | 0 s | 0 s |
| Residual expectation upper | 4.107121e-33 | 2.080539e-33 |
| Maximum residual cap | 1.335715e-32 | 1.335715e-32 |
| Outer expansions | 33556592 | 35253699 |
| Pending branches | 5147144 | 3580256 |
| Outer PW-impossible prunes | 5335 | 1344619 |
| Outer guide zero-residual prunes | 0 | 607269 |
| Further bound tightenings | 0 | 54 |
| Outer leaves | 0 | 0 |
| Suffix probe roots | 2666102 | 1248 |
| Positive / impossible / unknown probe roots | 0 / 0 / 2666102 | 663 / 578 / 7 |
| Completed cache entries | 1000001 | 248098 |
| Cache hits | 1128960 | 2285919 |
| Cumulative probe time | 5.138 s | 0.729 s |
| Cache capacity guard hit | yes | no |

The residual upper improves by a factor of 1.974, or 49.343%. The cache now
completes nearly all probe roots instead of filling before any root completes.
It can reuse those proofs to reject PW-impossible branches and branches whose
residual is already covered by the guided core. The outer search also expands
5.06% more nodes and leaves 30.44% fewer pending branches.

The reduced run reports at most 11 active team components and at most 6 exact
components per queried key. It records 12887833 certain components, 14342990
exact components, 2545473 key calls containing certainty, and 2285917 hits on
keys containing certainty. These are query counters, not unique-state or
unique-alias counts. No plan falls back due to unsupported arithmetic.

Both runs report `incomplete`, no errors, unchanged source/binary hashes, no
outer memory limit, and no PMF cache limit. The suffix time flag is set in both;
it can indicate a local 25 ms probe interruption and does not mean the full
15% cumulative allocation was consumed. The single suffix terminal entry
counts a cached terminal state, not a sampled campaign or an outer leaf.

## Remaining limitation

The maximum residual cap remains B=1.335714790055119e-32, with
H=B/1024=1.304408974663202e-35. Thus the uniform residual weight bound used by
the statistical sampler has not improved. The new deterministic residual
expectation upper is still about 2951 times the historical residual statistical
upper of 7.049822405827536e-37. It therefore cannot tighten that existing
interval. The earlier full-cell interval remains unqualified; this audit has
no new samples and cannot establish a coverage gain.

Much of the remaining bound is outside the 28-fixture suffix gate: the largest
remaining depth contributions are depth 43 (3.196878e-34), depth 62
(2.730888e-34), depth 49 (2.150919e-34), and depth 36 (1.179592e-34), in a
91-fixture suffix. The gate begins at depth 63. The compact cache removes a
previous obstacle to probing earlier branches, but this experiment does not
test a larger gate or include GD/GF restrictions in the suffix bound.

## Validation and reproducibility

All 54 optimized Rust tests and 57 Python experiment tests pass. Tests cover
different certain totals sharing a solved key, positional tag isolation after
seeding a cache entry, nonconstant-guidance exact/reduced minimum agreement,
identical categorical probabilities, finalized-invalid rejection, boundary
states and unsupported arithmetic. Independent source review found no
outstanding blockers.

Frozen binary SHA256:
`a9a86294eaff54e91d2ec09fb55c9409fd474b27b25984d36c8ceda725860268`.
Source aggregate SHA256:
`903b4ff4a15418108526af554ad8202227a846b2b483355ba939a4fdaddd3a88`.
Source backup commit:
`ba6f22e2643e208b4c7f53fe7761aad22e397d3c`.

From this directory, run the control and then repeat with
`--suffix-canonicalize-certain` and a different output filename:

```bash
python3 residual_audit.py \
  --binary /tmp/frontier-recovery-certain-v5 \
  --suffix-bounds --suffix-max-remaining 28 \
  --budget 180 --cleanup-reserve 10 --node-limit 100000000 \
  --output certain-team-key-exact-pal16-180s.json
```

The smoke uses a 30-second allowance, five-second reserve and explicit
`--search-ms 20000`. It finishes in 21.900 seconds, retains 202468 cache states,
and performs 46295 guide zero-residual prunes. It reports no errors or hash
changes. The matched pair above provides the full-budget outcome.

Raw JSON files remain local. Their SHA256 receipts are:

| File | SHA256 |
| --- | --- |
| `certain-team-key-exact-pal16-180s.json` | `f6bdcd5b81ea29a540bb894e986ce33d3179b8d670efd7a37b01bcf6b26d7c52` |
| `certain-team-key-pal16-180s.json` | `18a1db9ccef26927d5fed851aa3024804f724263e6fd116c13e717b08f51e046` |
| `certain-team-key-smoke-pal16.json` | `f4da6a5c974cd82332f8e21324f0ee2ea469a2c2975a6e423473de5f1c220737` |
