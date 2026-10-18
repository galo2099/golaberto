# Score-aware shared expectation: Palmeiras 16th, cohort 9

The matched 180-second test regresses: the new residual upper is
`2.915014853146674e-35`, versus `2.322453949358523e-35` for the control.
The new score calculation removes no archived residual upper. All nonzero
completed residual contribution is in the chosen rival's strict points/wins
channel, where its GD does not constrain the result. The treatment completes
fewer prefixes and represents fewer canonical PW states under the shared
allowance, consistent with the additional cache state and work.

The 540-second follow-up confirms the lack of GD tightening and returns
residual upper `2.1146629878206994e-35`. It does not improve the retained
full-cell interval `[4.618240572657253e-41, 1.802594383784308e-38]`.
The feature remains disabled by default.

## Operation and correctness

This opt-in experiment adds `expectation_score_gd`, requiring both existing
`shared_expectation` and `score_gd` options. Production code is unchanged.
The default remains the existing unweighted shared expectation operation.

For one generic rival, each cached clipped-expectation row contains five
channels: strict points/wins, tied points/wins, and three tied-channel GD
moments. The moment bases are `9/8`, `5/4`, and `3/2`. Each future W/D/L
outcome retains its actual native probability and actual guided categorical
ticket probability in the clipping-threshold transition. The GD moments
remain conditional on that same outcome; shared fixtures retain their
opposite team coefficients, including a coefficient of magnitude two when
the target plays the rival.

Only the tied channel receives the necessary GD-condition bound. Prefix
moments multiply their corresponding future moment channel outside the
cache; the three alternatives are minimized after the prefix is restored.
Strict points/wins contribution is added unchanged. GD equality is retained
conservatively, without requiring the GF tiebreaker to succeed.

The cache key retains the rival's actual packed points/wins total even if
the ordinary canonical key omits it, plus source and score-kernel identity.
Incomplete weighted channels use certified maximum future moment products;
overflow falls back conservatively. Pair-conditioned sources currently use
the plain operation. A completed envelope remains an upper bound, rather
than an exact probability. The pointwise residual sampling cap is unchanged,
including when a completed mean envelope becomes zero.

Independent review found no mathematical blocker. All 76 Rust tests and 62
Python tests pass. Tests include independent finite-score enumeration with
actual unequal proposal tickets, nonzero clipping, a consistent adverse GD
prefix, both ranking orientations, shared fixtures, scaled source factors,
strict/tie cache separation, source/kernel isolation, actual capacity and
deadline cutoffs, and preservation of the archived pointwise cap.

## Fixture, proposal, and comparison

The historical input is the 279-played/101-remaining request
`group-16498-two-results.json`, SHA-256
`cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee`.
This audit concerns Palmeiras finishing 16th, cohort 9: Corinthians below;
Grêmio and Mirassol above. Internacional, Remo, and Chapecoense are also
below. Palmeiras must lose all ten remaining games, finishing on 57 points
and 16 wins.

Both runs reproduce the saved complete joint proposal: k=11, 230755 states,
selected rival IDs `[584,67,18,79,10,14,7,225,6,4,74]`,
`B=8.057494973466603e-35`, and `H=7.86864743502598e-38`.
Both enable existing outer score-GD bounds and select Vitória, ID 67.
Thus the matched comparison isolates score integration inside shared
expectation, rather than the first application of GD information.

Vitória starts on 33 points, nine wins, GF 28, GA 42, and GD -14. Palmeiras
starts on 57 points, 16 wins, GF 47, GA 21, and GD +26. Vitória can reach
the exact points/wins tie through seven wins and three draws; eight wins
and two losses put it strictly ahead on wins. Its initial GD deficit is 40,
but Palmeiras' ten future losses change the eventual GD threshold. The
existing selector uses this adverse initial gap among rivals whose tied
packed score is individually reachable; it does not rank candidates by
their contribution to the clipped residual uncertainty.

Each matched run has a separate 180-second shared allowance including
setup, with a 20-second cleanup reserve. Both use the same binary, four
threshold bits, a four-million-row ceiling, 36-fixture expectation window,
56-fixture suffix window, canonicalization, and joint-ratio bounds. The
five-channel rows consume more bytes than the plain rows; this is the same
row ceiling, not an equal-memory comparison. Runs are sequential, without
concurrent builds or heavy experiments.

| Metric | Plain shared expectation + outer GD | Score-aware shared expectation |
| --- | ---: | ---: |
| Wall seconds | 168.957 | 169.404 |
| Residual upper | 2.322453949358523e-35 | 2.915014853146674e-35 |
| Archived upper | 1.4837373891797906e-36 | 1.3027932976228964e-36 |
| Pending upper, summed by depth | 2.1740802089277563e-35 | 2.784735520657947e-35 |
| Pending share of upper | 93.61% | 95.53% |
| Outer prefixes processed | 8422524 | 7928523 |
| Completed prefixes absorbed | 882505 | 588653 |
| Pending prefixes | 7190907 | 7595940 |
| Cache rows | 4000000 | 4000000 |
| Completed cache rows | 3999992 | 3999950 |
| Canonical PW states represented | 462310 | 343960 |
| Recorded expectation-cache seconds | 17.795 | 19.156 |

Both receipts have unchanged source/input/binary hashes, no errors, no
wall-budget overrun, and no reported memory or marginal-PMF guard. Both
reach the shared expectation row ceiling. Neither finishes the frontier.

The treatment's same-prefix diagnostic gives identical plain and
score-adjusted archived uppers, `1.3027932976228964e-36`. Its raw strict
channel contributes approximately that entire sum; its raw tied and
adjusted tied channels are zero. These are upper-envelope diagnostics, not
observed probabilities or final-rank hit counts. Zero clipped tied
contribution does not prove the underlying points/wins tie unreachable:
it could also be absent from these completed prefixes or clipped away by
`(V-H*G)+`. Pending tied possibilities have not all been resolved.

The smaller treatment archive cannot be credited to GD tightening: it
absorbs fewer prefixes. Its total upper is 25.51% higher, despite a 12.20%
smaller archived sum. It processes 5.87% fewer outer prefixes and absorbs
33.30% fewer completed prefixes. Depths 63-65 retain much of the pending
upper, just before or at the 36-fixture integration gate in a 101-fixture
plan. A globally selected rival with a large initial GD deficit need not
be the useful rival for the completed residual paths.

## Existing-budget follow-up: 540 seconds

The same treatment receives a separate 540-second allowance, including
setup, with a 40-second cleanup reserve. It returns in 516.861 seconds
with unchanged hashes, matching proposal, no errors, no wall overrun, no
moment overflow, and no reported memory or marginal-PMF limit. It reaches
the four-million-row ceiling and leaves the frontier incomplete.

| Metric | Score-aware, 540-second allowance |
| --- | ---: |
| Residual upper | 2.1146629878206994e-35 |
| Archived upper | 1.5134753638178368e-36 |
| Pending upper, summed by depth | 1.9633154457613387e-35 |
| Outer prefixes processed | 25769753 |
| Completed prefixes absorbed | 1941016 |
| Pending prefixes | 26473352 |
| Cache rows / complete rows | 4000000 / 3999863 |
| Canonical PW states represented | 343970 |
| Recorded expectation-cache seconds | 30.989 |
| Raw tied / adjusted tied archived upper | 0 / 0 |

The same-prefix plain and score-adjusted archived uppers remain equal.
All its nonzero completed clipped residual upper is again in Vitória's
strict PW channel. Its archived upper alone is 99.72 times the retained
full-cohort upper. More expansion of the pending frontier cannot erase
that fixed archive. Additional time increases the archive and reduces the
pending upper, but does not make this selected GD condition useful.

The upper is 61.90% higher than the historical plain shared-F 540-second
result, `1.306127933964181e-35`, whose frozen receipt is
`shared-expectation-cohort9-window36-bits4-cap4m-pal16-540s.json`, SHA-256
`78d879bdd46636bbb89aeec54d09f78b04ec6f8854af1359559b4b38fa4ab669`.
That historical run disabled the outer score-GD option, so it is a budget
reference rather than an isolated test of score integration. The matched
180-second pair above supplies the isolated comparison.

The three new runs have separate allowances of 180, 180, and 540 seconds;
their measured wall times sum to 855.222 seconds. This is additional
analytical work beyond the saved sampling and earlier analytical runs.

## Confidence and retained bounds

These audits are deterministic upper-bound calculations and add no new
sampling observations or confidence allocation. Reuse only the saved
540-second sampler in `lazy-hybrid-pal16-540s.json`, SHA-256
`9a4cc6173966a6a386684278c30682b840686f6ec28b1a78725fc479014301b0`.
Its cohort-9 core upper is `3.0299741691865805e-41`, from epoch 7, look 493,
247000 draws and the existing 99% simultaneous confidence allocation.
The proposal, actual ticket law, B, H and clipping definition match.

The candidate cohort upper is the outward sum of that actual core upper
and the new residual upper. Take its minimum with the retained full-cohort
upper `1.5176801956762062e-38`. Both 180-second candidates are much looser
than the retained full bound. No residual upper is subtracted from a
full-cohort comparator.

The 540-second treatment's core-plus-residual candidate is
`2.1146660177948688e-35`, also much looser than the retained comparator.

With cohort 4, cohort 5 and the other thirteen cohort contributions retained
as in `SHARED_EXPECTATION_COHORTS_9_4_RESULTS.md`, the full-cell interval
therefore remains `[4.618240572657253e-41, 1.802594383784308e-38]` after
the matched 180-second comparison. This is a retained combination of
separate sampling and analytical budgets, not the output of one request
within a single 180- or 540-second allowance.

The longer follow-up leaves that interval unchanged too. The useful next
investigation is to identify which rivals actually require tied-score
comparisons on the strict-Vitória residual paths represented in the archive, while also
checking clipping-grid slack. A future selector should use that conditional
structure rather than only the largest initial GD gap. Where a rival is
already guaranteed strict, the plain cache can avoid score-state overhead.
Multiple GD constraints must preserve shared fixture dependence; multiplying
independent rival probabilities would be invalid.

## Reproduction and artifacts

Source is backed up in commit
`4a128d2eaeb644d15ab3b6c9d109d2af0393703d` on
`experiments/rare-odds-recovery-2026-10-17`, tree
`ee191dfb5a5634201727ccaae70d1bece701c27c`.
Frozen `/tmp/frontier-recovery-score-expectation-v10` has SHA-256
`5f99fcaa07a51770e650ec8f196ad456fca0a2582353d0c42759bcc7bdee6e14`.
Raw requests and receipts remain local; only source, aggregates and hashes
are backed up.

| Local receipt | SHA-256 |
| --- | --- |
| score-expectation-cohort9-control-pal16-180s.json | b0679cc9e5ed4f6e544c8fbcb385a850c35daed3ca99ca4c56271059027f0eb3 |
| score-expectation-cohort9-pal16-180s.json | bdc79b5b6c9d2af758688ae856a9121523fd2816d635324ce28631b8d7348d6c |
| score-expectation-cohort9-pal16-540s.json | 5a2967bfe25de43b9dd0008d6f1d92a8f91fdceb42dfee9b54ca070160c1a8bd |

Run from this experiment directory:

```bash
python3 residual_audit.py \
  --binary /tmp/frontier-recovery-score-expectation-v10 \
  --region-id 9 --k 11 --proposal-mode joint \
  --suffix-bounds --suffix-state-cap 4000000 --suffix-max-remaining 56 \
  --suffix-canonicalize-certain --suffix-joint-ratio \
  --shared-expectation --expectation-bits 4 --expectation-max-remaining 36 \
  --score-gd true --expectation-score-gd \
  --budget 180 --cleanup-reserve 20 --node-limit 100000000 \
  --output score-expectation-cohort9-pal16-180s.json
```

Omit `--expectation-score-gd` for the matched control.
Use `--budget 540 --cleanup-reserve 40` and the corresponding output name
for the longer treatment.
