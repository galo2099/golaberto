# Shared suffix bounds for Palmeiras 16th, cohorts 9 and 4

Cohort 4's full upper improves 7.60x to `7.38e-40`. Cohort 9's original
18-fixture gate barely activates; widening it to 36 fixtures improves the
analytical certificate 6.04x, but still does not beat the saved sampling
bound. Retaining that bound gives full-cell interval
`[4.62e-41,1.80e-38]`, a 21.25% reduction in the upper. Three separate
540-second analytical allowances are used, including the cohort-9 retry.

This applies the existing shared clipped-expectation operation to the two
cohorts that supplied most of the full-cell upper after cohort 5 improved.
The historical input, native estimator binary and sampling observations
remain unchanged. Each new cohort audit receives its own 540-second shared
allowance, including setup, with a 40-second cleanup reserve. The audits
run sequentially without another heavy experiment or build running.

The operation retains canonical future points/wins state and a separate
rounded-down clipping threshold. Four mantissa bits, a four-million-cell
ceiling and an 18-fixture integration window match the longer cohort-5
experiment. The original suffix guide/joint-ratio cache remains enabled;
additional score-GD bounds remain disabled. A completed cached envelope is
a conservative expectation upper, not an exact probability.

## Reproduce the saved proposals

The residual decomposition depends on the actual proposal. A different
proposal cannot be paired with the saved sampled core merely because it
has the same target cell. Short smoke audits verify the installed proposals
before the long runs.

| Property | Cohort 9 | Cohort 4 |
| --- | --- | --- |
| Team below Palmeiras among the varying three | Corinthians | Mirassol |
| Other two above Palmeiras | Grêmio, Mirassol | Grêmio, Corinthians |
| Proposal mode | Joint | Regime |
| Selected team count | 11 | 9 |
| Proposal states | 230755 | 125672 |
| Regime pivot / atoms | None | Grêmio 79 / 11 |
| B | 8.057494973466603e-35 | 4.839011239011391e-35 |
| H = B / 1024 | 7.86864743502598e-38 | 4.725596913097061e-38 |

Both cohorts also place Internacional, Remo and Chapecoense below Palmeiras.
Palmeiras must lose all ten remaining games, finishing on 57 points and
16 wins. The above-team sets and selected-rival order match the saved
sampler's final proposals.

The driver adds `--proposal-mode joint`, which skips regime construction
and directly audits its accepted prepared joint plan. Regime remains the
default. This avoids replacing cohort 9's saved joint proposal while
attempting a new partition. No native Rust changes or rebuild are needed.
All 62 Python tests pass, including the new joint-mode setup/budget cases.
The unchanged estimator's 72 Rust tests passed in the preceding experiment.

Driver source and tests are backed up in commit
`623fd3ef0faba6ffa7a6ff6c01d79e455fd4be36` on
`experiments/rare-odds-recovery-2026-10-17`, with tree
`3c90ae43dc26e419e6b9de9fdc0cd9623de9b24a`.
Frozen `/tmp/frontier-recovery-expectation-v9` has SHA-256
`585616bae783bfb083fd17d559db8375e4d7302fa7fe001ae8da6375051a3d57`.

## Confidence and combination

Reuse only the existing 540-second sampler in
`lazy-hybrid-pal16-540s.json`, SHA-256
`9a4cc6173966a6a386684278c30682b840686f6ec28b1a78725fc479014301b0`.
The saved threshold counts yield these sampled core uppers:

| Cohort | Saved epoch / look | Core draws | Core upper |
| --- | --- | ---: | ---: |
| 9 | 7 / 493 | 247000 | 3.0299741691865805e-41 |
| 4 | 4 / 751 | 376000 | 1.3169602726480017e-41 |
| 5, validation | 6 / 3995 | 1997936 | 6.970578528214142e-40 |

The calculation uses the existing base error allocation
`0.01 / (6840 * 16)`, the cohort/epoch/look telescoping allocation, division
among three streams, and division among the 482 threshold endpoints.
Multiply the normalized core bound by H and round upward. Recomputing
cohort 5 reproduces its previously used core upper exactly.

For each new audit, the usable cohort upper is
`min(retained_full_upper, outward_sum(core_upper, residual_upper))`.
The retained uppers for cohorts 9 and 4 are baseline-stream comparators:
`1.5176801956762062e-38` and `5.6025180799188936e-39`, respectively.
Subtracting a residual upper from either comparator would be invalid.

Retain cohort 5's existing combined upper `1.6568065252541712e-39`.
The remaining thirteen cohort uppers have an accurately summed value of
approximately `4.546993577551541e-40`; sum all final contributions outward.
The full lower remains `4.618240572657253e-41`. No new Monte Carlo samples
or confidence-error allocation are added by the deterministic audits.

These are separate budgets: the existing sampler had 540 seconds, cohort
5's prior analytical audit had 540 seconds, and each new analytical audit
has another 540 seconds. The combined interval is not one 540-second
endpoint result. Smoke checks and setup verification add task work too.

## Cohort 9: first pass exposes an eligibility limit

The initial 18-fixture window returns in 514.350 seconds, with unchanged
hashes, no errors or reported memory/PMF limit, and matching proposal B/H.
Its residual upper is `7.887931826568143e-35`, too loose to improve the
retained full-cohort upper `1.5176801956762062e-38`. The latter remains in
the full-cell calculation. An analytical run producing a looser bound is
not evidence that the cohort has more actual probability.

The operation barely activates: it absorbs 785 prefixes and creates only
20006 completed cells across 1426 canonical states, with 68.977 ms of
recorded expectation-cache work and no capacity limit. It processes
24458914 outer prefixes but retains 34182022. The expectation upper is
almost entirely in the pending frontier; the small archived contribution
is `2.2356548319092124e-40`.

The joint plan orders 101 fixtures. An 18-fixture window starts integration
at depth 83, while the pending upper-weighted mean depth is 68.97, leaving
about 32 fixtures. The first run's pending depth distribution gives:

| Maximum remaining fixtures | Share of pending upper eligible |
| --- | ---: |
| 18 | 0.0094% |
| 24 | 6.41% |
| 28 | 27.39% |
| 32 | 59.60% |
| 36 | 81.09% |

This identifies an outer-search/eligibility bottleneck. It does not yet
show whether GD integration or threshold-envelope tightness would limit
the operation after it reaches the relevant states. After the cohort-4
run, cohort 9 is retried with a 36-fixture window under another separate
540-second allowance, keeping the four-million-cell ceiling and other
settings fixed. The broader gate may spend more time on interrupted
recursion; completed weighted coverage must be measured, not assumed.

## Cohort 4: useful tightening, with substantial pending work

The 18-fixture audit returns in 517.394 seconds. Its proposal matches the
saved sample stream: complete 125672-state regime, Grêmio pivot 79,
11 atoms, selected-rival order, B and H. It reports no errors, hash changes,
memory limit or total-wall overrun. The native marginal-PMF cache guard
does activate, retaining conservative fallback bounds; this is not a
completed analysis, and the guard's effect cannot be isolated from this
single run.

```text
sampled core upper                 1.3169602726480017e-41
new residual upper                 7.244663953451916e-40
new full-cohort upper, rounded up   7.376359980716718e-40
retained previous full upper       5.6025180799188936e-39
```

The full-cohort upper improves by 7.60x, or 86.83%. The operation absorbs
3277896 prefixes while processing 35010986 outer prefixes, retaining
29055449. The expectation cache reaches four million cells across 377983
canonical states, with 3999950 completed rows, 50 incomplete rows and
7822674 completed-cell hits. Its recorded work takes 54.981 seconds. The
separate guide/joint-ratio cache uses 3726166 rows and does not hit its cap.

The archived mean upper is `7.944139508917855e-41`, or 10.97% of the new
residual upper. Pending depth bounds sum to approximately
`6.450249986174489e-40`, or 89.03%. Their upper-weighted mean depth is
87.92 of 101 fixtures; depths 86-90 supply 82.33% of the residual upper.
Unlike cohort 5's fixed-envelope floor, pending uncertainty remains
substantial here and lies inside the shared integration window. New-cell
capacity is reached, and the PMF guard can also loosen the remaining
bounds. This run does not establish that GD is the dominant remaining loss.

Before cohort 9's wider-window retry, replacing only cohort 4's full upper
lowers the full-cell upper from `2.2890825919690283e-38` to approximately
`1.802594383784307e-38`, a 21.25% reduction. Cohort 9 still supplies about
84.19% of that interim upper. The full lower and sampling data are unchanged.

## Cohort 9: wider window enables integration but is still too loose

The 36-fixture retry returns in 517.155 seconds, with unchanged hashes,
matching complete joint proposal, and no errors, reported memory/PMF
limit or wall overrun. It absorbs 3265329 prefixes, versus 785 with the
18-fixture gate. The cache reaches four million cells across 461935
canonical states, with 3999992 completed cells, eight incomplete cells,
6213107 completed-cell hits and 26.079 seconds of recorded cache work.
It processes 28962412 outer prefixes and retains 24430079.

| Cohort-9 result | Upper |
| --- | ---: |
| Residual certificate, window 18 | 7.887931826568143e-35 |
| Residual certificate, window 36 | 1.306127933964181e-35 |
| Saved core plus window-36 residual, rounded up | 1.3061309639383506e-35 |
| Retained sampled full-cohort upper | 1.5176801956762062e-38 |

The wider gate improves the analytical residual upper 6.04x, but that
upper still exceeds the saved full-cohort upper 860.61x. The latter remains
the usable bound. Eligibility was a real first-pass limit, but fixing it
does not make the current envelopes competitive.

The archived upper is `1.758829701208506e-36`, or 13.47% of the new
certificate. Pending depth bounds sum to approximately
`1.1302449601422379e-35`, or 86.53%. Their upper-weighted mean depth shifts
from 68.97 to 62.71; depth 64 or shallower now carries 54.64% of the whole
certificate upper. Integrating deeper branches pushes remaining search
uncertainty toward earlier states, while the cache ceiling is reached.

Crucially, the archived upper alone is 115.89x larger than the retained
sampling bound. Even replacing every pending branch by zero would leave
this certificate too loose while the archived envelopes stay fixed.
More outer expansion or a still-wider gate alone cannot close that gap.

The archive is an upper envelope, not measured probability mass. The
operation retains grid rounding and bounds score/ranking acceptance by one
inside its cached integration. The result does not isolate how much comes
from each relaxation, or prove that actual GD probability dominates.
The next useful comparison should refine or incorporate score/ranking
bounds in the completed envelopes. Preserving weighted archived groups
would allow such refinement without discarding the search already done.

## Final retained full-cell result

Use the saved sampled upper for cohort 9, the new core-plus-residual upper
for cohort 4, the previously improved cohort-5 upper, and the saved bounds
for every other cohort. Add all sixteen raw cohort contributions outward
rather than subtracting a residual bound from a baseline comparator.

| Contribution | Retained upper | Share of full upper |
| --- | ---: | ---: |
| Cohort 9 | 1.517680196e-38 | 84.19% |
| Cohort 5 | 1.656806525e-39 | 9.19% |
| Cohort 4 | 7.376359981e-40 | 4.09% |
| Other thirteen, approximate sum | 4.546993578e-40 | 2.52% |

```text
full lower, unchanged             4.618240572657253e-41
full upper, outward sum           1.802594383784308e-38
previous full upper               2.2890825919690283e-38
upper/lower ratio                 390.3205897191119
```

The upper falls 21.25%. The interval qualifies under the adopted adaptive
logarithmic rule: exponent halfwidth 1.295711 versus allowance 1.951991.
It is not a factor-two estimate. The 99% simultaneous confidence allocation
is unchanged; deterministic audits add no error budget or observations.

The final result uses the existing 540-second sampler, the earlier separate
540-second cohort-5 analytical audit, and three new analytical allowances
of 540 seconds each. The three new runs take approximately 1548.899 seconds
in total, before smoke checks and other task work. This is not one request
that fits all sampling and analysis inside 540 seconds.

For cohort 4, both the reached integration-cache cap and the marginal-PMF
fallback are useful next experiments to isolate; most of its upper is still
pending. For cohort 9, the fixed completed-envelope floor must also be
addressed. Increasing cache time alone cannot grow either full cache.

## Receipts and review

All raw JSON remains local. Source and this aggregate report are backed
up on the existing experiment branch. Independent review verifies matching
proposal metadata, the saved core confidence calculations, all three run
results, receipt hashes, the sixteen-cohort outward sum, and the explicit
separate-budget interpretation.

| Receipt | SHA-256 |
| --- | --- |
| `shared-expectation-cohort9-proposal-smoke.json` | `8f87cf2810a5cbd3cef4339f6461ccacd8004bf38a3959ea91077e9b5bcb0021` |
| `shared-expectation-cohort4-proposal-smoke.json` | `23fd1880b8385505b7bc35fda1d705b3dac52bb990e245fad631e3213ac9b90d` |
| `shared-expectation-cohort9-bits4-cap4m-pal16-540s.json` | `aa679fa97eb1942b296243846fdeb44469c4fe30cf5cf1a712ada71b4c092726` |
| `shared-expectation-cohort4-bits4-cap4m-pal16-540s.json` | `fb01da745ede672bb63f91db8801b138e80ab82a65a44d3431539911e5af4546` |
| `shared-expectation-cohort9-window36-bits4-cap4m-pal16-540s.json` | `78d879bdd46636bbb89aeec54d09f78b04ec6f8854af1359559b4b38fa4ab669` |

## Commands

```bash
python3 residual_audit.py \
  --binary /tmp/frontier-recovery-expectation-v9 \
  --region-id 9 --k 11 --proposal-mode joint \
  --suffix-bounds --suffix-state-cap 4000000 --suffix-max-remaining 56 \
  --suffix-canonicalize-certain --suffix-joint-ratio \
  --shared-expectation --expectation-bits 4 \
  --budget 540 --cleanup-reserve 40 --node-limit 100000000 \
  --output shared-expectation-cohort9-bits4-cap4m-pal16-540s.json

python3 residual_audit.py \
  --binary /tmp/frontier-recovery-expectation-v9 \
  --region-id 4 --k 9 --proposal-mode regime \
  --suffix-bounds --suffix-state-cap 4000000 --suffix-max-remaining 56 \
  --suffix-canonicalize-certain --suffix-joint-ratio \
  --shared-expectation --expectation-bits 4 \
  --budget 540 --cleanup-reserve 40 --node-limit 100000000 \
  --output shared-expectation-cohort4-bits4-cap4m-pal16-540s.json

python3 residual_audit.py \
  --binary /tmp/frontier-recovery-expectation-v9 \
  --region-id 9 --k 11 --proposal-mode joint \
  --suffix-bounds --suffix-state-cap 4000000 --suffix-max-remaining 56 \
  --suffix-canonicalize-certain --suffix-joint-ratio \
  --shared-expectation --expectation-bits 4 --expectation-max-remaining 36 \
  --budget 540 --cleanup-reserve 40 --node-limit 100000000 \
  --output shared-expectation-cohort9-window36-bits4-cap4m-pal16-540s.json
```
