# Threefold sampling budget for Palmeiras 16th

This experiment follows [REMAINING_MASS_ANALYSIS.md](REMAINING_MASS_ANALYSIS.md).
It tests whether more sampling reveals a different pattern in the remaining
hybrid uncertainty, using a 540-second shared allowance instead of 180 seconds.
No estimator or production source is changed.

The run finishes in 539.205 seconds and tightens the sampling-only interval
to `[4.618240572657253e-41,3.4129620365771417e-37]`, still narrowly
unqualified. Cohorts 9 and 4 improve about sevenfold. Cohort 5 reaches its
four-million physical-draw cap, still observes zero residual contributions,
and now accounts for 93.78% of the raw upper. The dominant pattern remains
an unseen-tail allowance rather than newly discovered residual mass.

## Protocol

Use the same frozen lazy-hybrid binary as the saved 180-second comparison,
`/tmp/frontier-recovery-hybrid-lazy-v2`, SHA-256
`369eae5388513cb8ed1e56aca99731c2c31dfa9e7111e1e0eb9a1c37f9c52768`.
Later GD/residual-certificate experiments are separate paths and are not
invoked by this sampling run. The historical request has 279 played and
101 remaining matches, SHA-256
`cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee`.

Use one worker, seed 20261017, 99% simultaneous confidence, depth 9, 128
initial physical draws, initial k=7/1000 states, maximum k=11/250000 states,
1000 ms per setup attempt, conditional regimes and rebuilds enabled, HiGHS
pruning disabled, and clip fraction 1/1024. Retain the maximum 4000000
physical draws per cohort. Proposal growth and confidence accounting are
unchanged. Setup, sampling and confidence calculations are charged inside
the same 540-second allowance, with a one-second cleanup reserve.

The new run adds `--run-full-budget` so qualification cannot stop the budget
experiment early. The historical hybrid did not qualify and consumed its
usable allowance, so this flag does not explain its earlier endpoint.
The adaptive scheduler may choose different setup attempts and sample
allocations because timing affects proposal completion and cohort priority.
The new run is not assumed to contain exactly the old run's sample paths.

```bash
python3 driver.py \
  --binary /tmp/frontier-recovery-hybrid-lazy-v2 \
  --request ../reference/2026-10-04-two-results/inputs/group-16498-two-results.json \
  --team-id 16 --position 16 --budget 540 --cleanup-reserve 1 \
  --seed 20261017 --workers 1 --initial-draws 128 --max-draws 4000000 \
  --initial-state-cap 1000 --state-cap 250000 --time-ms 1000 \
  --depth 9 --initial-k 7 --max-k 11 --hybrid \
  --hybrid-clip-fraction 0.0009765625 --run-full-budget \
  --output lazy-hybrid-pal16-540s.json
```

The raw JSON remains local. The report records aggregate results and hashes.
No earlier statistical confidence intervals are intersected across separate
runs. Any reuse of an earlier deterministic certificate must preserve its
proposal and residual decomposition, and is reported separately from the
measured 540-second sampling interval.

## Measured sampling comparison

| Metric | Saved 180-second run | 540-second run |
| --- | ---: | ---: |
| Recorded run seconds | 179.026 | 539.205 |
| Physical draws across all cohorts | 1951048 | 5306920 |
| Simultaneous 99% lower | 2.589898304e-41 | 4.618240573e-41 |
| Simultaneous 99% upper | 8.602973865e-37 | 3.412962037e-37 |
| Upper/lower ratio | 33217.42 | 7390.18 |
| Exponent halfwidth | 2.260683 | 1.934327 |
| Allowed exponent halfwidth | 1.916302 | 1.920060 |
| Qualified | No | No |

The new upper is 60.33% lower (2.521x improvement); the lower rises 78.32%.
Physical draws increase 2.720x for three times the allowance. This is one
adaptive seed/request comparison, not a statistical performance guarantee.
The raw upper is 6.46% above the maximum permitted with its fixed lower.

Both runs have unchanged hashes within their respective measurements, no
sample errors, no confidence conflicts, zero uncovered mass, and no trusted
HiGHS dependence. The top-three installed proposal dictionaries match
exactly between runs, including complete tables, selected rivals, states,
pivot/atom counts and certified caps. Their observation caps do not improve.
The gain comes from more sampling under those proposals.

| Cohort | Final draws per stream, 180s -> 540s | Retained upper, 180s -> 540s | Upper improvement | Share of raw 540s upper |
| --- | --- | --- | ---: | ---: |
| 5 | 875000 -> 1997936 | 7.049900211e-37 -> 3.200621843e-37 | 2.203x | 93.78% |
| 9 | 30500 -> 247000 | 1.080793875e-37 -> 1.517680196e-38 | 7.121x | 4.45% |
| 4 | 46500 -> 376000 | 4.094595494e-38 -> 5.602518080e-39 | 7.308x | 1.64% |
| Other 13 | Various | Various -> 4.546993578e-40 total | Various | 0.13% |

Each core/residual pair costs two physical draws; the baseline comparator
reuses the residual campaign. Cohort 5 uses 4000000 physical draws including
earlier epochs, reaching the unchanged cap. Cohorts 9 and 4 get about 8.10x
their old final-epoch stream draws even though wall allowance rises only 3x.
The final distribution is consistent with the adaptive scheduler moving
work toward other cohorts once cohort 5 is capped. Receipts do not record
global per-batch wall timestamps, so they do not locate the exact moment
of that transition or first qualification under a hypothetical policy.

The native sampling timer totals 463.001 seconds, with 375.439 seconds in
guided core and 86.973 seconds in baseline/residual sampling. Replay takes
0.406 seconds and is already included in baseline time. The core uses
81.19% of measured core-plus-baseline phase time. Setup totals 17.278 seconds;
confidence calculations and orchestration consume additional wall time.

## What emerges in the observations

| Cohort | Core positives, 180s -> 540s | Residual positives | Baseline positives |
| --- | --- | --- | --- |
| 5 | 58531 -> 133599 | 0 -> 0 | 5 -> 18 |
| 9 | 22729 -> 184923 | 815 -> 6393 | 815 -> 6393 |
| 4 | 6295 -> 49962 | 2 -> 2 | 3 -> 10 |

These are nonzero weighted conditional-score contributions, not ordinary
Bernoulli final-position hits. In cohort 5, all 18 observed positive baseline
contributions still have zero residual contribution: they do not expose the
overflow outside the clipped core. More sampling has not established that
overflow is zero.

The guided score-stream means remain similar:

| Diagnostic stream | Empirical lower/upper means, 180s | Empirical lower/upper means, 540s |
| --- | --- | --- |
| Cohort 5 core | [6.0141e-41, 1.6456e-40] | [5.9713e-41, 1.6006e-40] |
| Cohort 9 residual | [7.2030e-41, 1.9648e-40] | [8.1281e-41, 2.1515e-40] |
| Cohort 4 core | [1.0945e-42, 3.0817e-42] | [1.2081e-42, 3.0649e-42] |

These are diagnostics, not confidence intervals or qualified point estimates.
Cohort 5's baseline upper-score mean changes much more, from 1.8034e-42 to
5.0785e-41, despite its stable guided-core mean. Its sparse baseline remains
high variance. The two empirical streams need not agree at finite sample
sizes, and their difference does not measure true residual mass.

## The upper is still dominated by unseen tails

Recomputing the saved final-look threshold bounds gives:

| Binding baseline bound | Zero-count strip share, 180s | Zero-count strip share, 540s |
| --- | ---: | ---: |
| Cohort 5 | 99.98669% | 99.13457% |
| Cohort 9 | 98.64064% | 93.42055% |
| Cohort 4 | 99.99665% | 99.97317% |

Cohort 5's residual has no positive observations; essentially its entire
3.197906296e-37 statistical residual upper is zero-count tail allowance.
Cohort 4's residual has the same two positive observations and its zero-count
strip share remains about 99.99958%. Cohort 9 has many more residual
observations, yet unseen high-weight ranges still dominate its upper.

The approximately inverse-draw tail reduction persists. Cohorts 9 and 4
receive about eight times the final-epoch draws and improve about sevenfold;
confidence allocation across additional looks prevents exact 1/n scaling.
Cohort 5 receives only 2.283x more final-epoch draws before its cap and improves
2.203x. There is no new proposal-cap reduction or newly observed cohort-5
residual mass explaining the gain.

With the new lower fixed, the raw total upper must be at most
3.2059369423503933e-37 to qualify. Holding other cohorts fixed requires
cohort-5 upper at most 2.993596748406032e-37, approximately 6.47% below its
current value. A further wall-budget increase cannot add cohort-5 draws
under the unchanged four-million cap. Further work on other cohorts or an
improved lower could still change qualification, so this is a conditional
diagnostic, not an impossibility claim.

## Separate reuse of the existing deterministic certificate

The cohort-5 accepted proposal matches the old certificate's complete
Mirassol-584 forest: selected nine rivals, 156 roots (90 tied/66 strict),
92975 forest states, 166182 total states, pair-normalizer range, B, H,
root partition and native guide semantics. The unchanged frozen v2 sampler
and the later v6 certificate implement the same residual decomposition.
The exact proposal and ranking match supports reusing the deterministic
certificate; equality of B/H alone would not be sufficient.

Using only statistics from the new 540-second run:

```text
cohort-5 core upper                 6.970578528214142e-40
old deterministic residual upper   4.826735149714054e-38
new combined cohort-5 upper        4.896440934996197e-38
other cohort uppers                2.123401939443611e-38
post-hoc full upper                7.019842874439809e-38
full lower, unchanged              4.618240572657253e-41
```

The illustrative interval `[4.618240573e-41,7.019842874e-38]` qualifies:
exponent halfwidth 1.590925, allowance 1.937230. The earlier deterministic
audit consumes a separate budget; this is not the raw measured 540-second
interval and does not establish that the current endpoint returns this
interval in 540 seconds. No old statistical interval is intersected with
the new run. The existing report's baseline-versus-core accounting
correction is preserved.

The raw bottleneck has returned to cohort 5. Even after applying the old
certificate, cohort 5 contributes about 69.75% of the illustrative upper,
cohort 9 21.62%, cohort 4 7.98%. The broader pattern is stable observed
guided contributions plus a large, unobserved residual allowance.

## Implications

The extra budget confirms that more samples help cohorts 9 and 4 substantially,
without changing their proposals. The next useful change is to let the
scheduler use a compatible residual expectation certificate, so it can
allocate work according to the remaining bounded uncertainty. Simply
extending wall time while leaving cohort 5 capped cannot add evidence there.
Raising that draw cap modestly would test the near qualification threshold,
but would not address the structural gap between the unseen-tail allowance
and observed contributions.

For structural progress on cohort 5, the source/rival-weighted frontier audit
identified in REMAINING_MASS_ANALYSIS remains relevant. This run does not
establish that GD or PW feasibility is the dominant missing integration.

## Provenance and checks

No source-code edits or new tests are needed for this budget-only run. The
same native binary is used as the saved lazy 180-second comparison. The four
runtime Python modules `driver.py`, `hybrid.py`, `confidence.py` and
`scheduler.py` were compared byte-for-byte with source backup commit
`87667bce6880c55178490a1291cdea754ac6be99` and match. The aggregate source
hash differs because the manifest also includes later experiment files,
tests and Rust sources; it must not be described as equal between runs.
The new run's inputs and measured source hashes stay unchanged throughout.

An independent numerical review recomputed the per-stream confidence
bounds and unseen-tail decomposition. A separate review confirmed proposal
compatibility and the explicit core-plus-residual arithmetic.

| Artifact | SHA-256 |
| --- | --- |
| New `lazy-hybrid-pal16-540s.json` | `9a4cc6173966a6a386684278c30682b840686f6ec28b1a78725fc479014301b0` |
| Saved `lazy-paired-pal16-180s.json` | `6e6434b9e89217c70eb6e70a74371e9f5981370a20767204591f6905fe983573` |
| New aggregate source/binary manifest | `f2f2c7fdec772366a179d7239f2773e07fb58a6750379fb8fea814e342c5a75c` |
| Existing `joint-ratio-pal16-180s.json` certificate | `5badc49fd8a559660198d38db024322de687c71972d6b1b8871dba181df60136` |

Aggregate results are backed up on `experiments/rare-odds-recovery-2026-10-17`
in `galo2099/golaberto`. Raw JSON remains local and unchanged.
