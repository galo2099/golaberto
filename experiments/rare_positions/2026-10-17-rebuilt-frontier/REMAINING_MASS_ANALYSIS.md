# Remaining uncertainty for historical Palmeiras 16th

The largest remaining upper contribution on the hybrid-plus-certificate
path is now cohort 9, not cohort 5. Cohorts 9, 5 and 4 account for about
96.93% of its corrected upper. Most of the statistical upper in cohorts 9
and 4 is an allowance for unseen large contributions. This is different
from knowing where the actual finishing probability lies.

This analysis recomputes saved observations and deterministic receipts. It
adds no draws, changes no native estimator, and makes no new runtime claim.
The request is the historical 279-played/101-remaining fixture, SHA-256
`cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee`.

## Corrections to the previous bounds summary

The earlier hybrid illustration 2.0358249742544096e-37 was calculated as
old global upper minus old cohort-5 residual upper plus new deterministic
residual upper. This is invalid: the old retained cohort-5 upper was the
baseline comparator, not the sum of its core and residual bounds.

The saved final-epoch threshold counts give these absolute uppers:

```text
other cohort uppers                 1.5530736543769992e-37
cohort-5 core upper                 1.1463390418238276e-39
cohort-5 statistical residual upper  7.049822405827536e-37
cohort-5 retained baseline upper     7.04990021073354e-37
best deterministic residual upper   4.826735149714054e-38
```

The valid combination replaces the residual within the actual decomposition:
`min(old_cohort_upper, core_upper + deterministic_residual_upper)`. This gives
cohort-5 upper 4.941369053896437e-38 and corrected full-cell upper
approximately 2.047210559766643e-37. Its lower remains
2.589898304031079e-41. Outward summation can change the last printed digits.
No failure-probability allocation is spent on deterministic certificates.
The sampling and certificate audit budgets are separate; this is not one
180-second full-cell result.

The same correction changes the latest score-GD illustration to
2.066470765763051e-37. It changes no standalone certificate, benchmark,
sample count or failed-qualification conclusion. The corresponding
residual-only qualification target is 3.3516004661537957e-38, leaving room
for the actual core upper. The previous 3.466234e-38 number is a whole-cohort
budget, not a residual budget.

Also, calling the hybrid bounds the best available historical interval was
incorrect. The earlier ordinary GD-rebuild run has a standalone interval
`[3.105431383236009e-41,1.719720850889973e-37]`. As already recorded in
[ADAPTIVE_QUALIFICATION.md](ADAPTIVE_QUALIFICATION.md), that interval qualifies
under the adopted relaxed rule, although its old receipt predates that rule
and stores `qualified:false`. Its separate 99% confidence statement should
not be silently merged with intervals from other runs without accounting
for their combined failure probability.

## Where the hybrid upper now sits

All these cohorts require Palmeiras to lose all ten remaining matches and
finish on 57 points and 16 wins. A cohort fixes exactly which 15 rivals finish
above and which four finish below. The top three share below teams
Internacional (20), Remo (110) and Chapecoense (318); the fourth differs.

| Cohort | Fourth team below | Current upper | Share of corrected hybrid upper | Final draws per stream | Positive core / residual |
| --- | --- | ---: | ---: | ---: | ---: |
| 9 | Corinthians (24) | 1.080793875e-37 | 52.79% | 30500 | 22729 / 815 |
| 5 | Grêmio (79) | 4.941369054e-38 | 24.14% | 875000 | 58531 / 0 |
| 4 | Mirassol (584) | 4.094595494e-38 | 20.00% | 46500 | 6295 / 2 |
| Other 13 | Other below sets | 6.282023019e-39 | 3.07% | Various | Various |

Cohort 5 uses core plus deterministic residual; cohorts 9 and 4 retain their
statistical baseline comparator upper, which is smaller than their current
core-plus-residual sum. Positive counts are weighted conditional-score
contributions, not ordinary Bernoulli rank hits. These are upper allowances,
not established shares of the actual probability.

The ordinary GD-rebuild run has a different allocation and bottleneck:
cohort 5 supplies 90.98% of its upper, cohort 9 5.43%, cohort 4 3.39%.
It gives cohort 9 397000 final-epoch ordinary draws and obtains 10150 positive
contributions, compared with the hybrid's 30500 baseline/residual draws and
815 positives. Thus the latest hybrid problem should not be generalized to
every saved estimator version.

## Cohort 9: observed residual, large unseen-tail allowance

Cohort 9 requires Grêmio and Mirassol above Palmeiras, with Corinthians
below. Grêmio begins on 29 points and seven wins; to finish above 57/16 it
needs ten wins (59/17, strictly ahead) or nine wins and one draw (57/16,
requiring the score tiebreak). The complete k=11 joint points/wins proposal
selects rivals `[584,67,18,79,10,14,7,225,6,4,74]` and uses 230755 states.
It already preserves shared fixtures among those constraints.

The final-epoch core interval is approximately
`[1.1864e-42,1.2262e-40]`. The residual interval is
`[1.0181e-41,1.0807e-37]`, so core precision is not the leading upper problem.
The fixed clip threshold is B/1024. Empirical score-stream means place about
96% of the observed core-plus-residual contribution in residual. This is a
diagnostic of this proposal/clip choice, not a certified probability share.
The 22729 positive core observations do not certify the separate residual.

The residual has cap B=8.057494973466603e-35, n=30500 and 815 positive
contributions. Despite these observations, zero-exceedance dyadic strips
j=1 through 7 contribute 1.066102026576091e-37: 98.65% of its residual upper
and 98.64% of the binding baseline upper. The first populated upper threshold
is below those large-weight strips.

For a normalized observation X/B in [0,1], a zero-count threshold still has
Clopper-Pearson probability upper `1-alpha_tail^(1/n)`, approximately
`-log(alpha_tail)/n`. At this saved look, B times that zero-count probability
is approximately 1.074496531e-37, close to the entire retained upper.
The baseline empirical upper-score mean is only about 2.0174e-40.
Observed positives therefore do not remove the allowed unseen high-weight
tail. The confidence cap and draw allocation drive most of the gap.

Holding other corrected contributions and the historical lower fixed,
cohort 9 needs an upper at most 9.332804064e-38 to qualify the combined
hybrid interval: a 13.65% reduction from its current upper. The dominant-tail
inverse-draw approximation suggests increasing final residual draws from
30500 to roughly 35000-36000. This is a planning calculation, not a guarantee:
new high-weight observations, look allocation and other endpoints can change.

## Cohort 4: the core finds contributions, the residual is unexcluded

Cohort 4 instead puts Grêmio above and Mirassol below. Its complete k=9
regime proposal conditions pivot 79, with 11 atoms, 125672 states and
B=4.839011239011391e-35. It uses 46500 draws in each stream.

The core has 6295 positive contributions and upper 5.134171069e-41.
Residual has only two positives and upper 4.094584604e-38. The observed
core means dominate the diagnostic core-plus-residual means (over 99.9%),
but this cannot justify discarding an unobserved residual tail.

Zero-count strips account for 99.99958% of the residual upper and 99.99665%
of the binding baseline upper. Here the large uncertainty is not an observed
large residual contribution. It is failure to exclude such a contribution.
A residual-specific cap or deterministic expectation certificate is a more
direct structural target than tightening the already small core interval.

## Cohort 5: millions of unresolved prefix bounds

The best deterministic residual upper is 4.826735150e-38. Its frontier has
12494771 pending prefixes; 98.28% of the upper is at depths 70-79 of the
91-fixture suffix, with 12-21 fixtures still unassigned. Depths 73-74 alone
contribute 54.00%. The largest retained prefix contributes 1.632138845e-44,
only about 3.38e-7 of the total. There is no single dominant remaining prefix.

The complete baseline jointly conditions points/wins for Vitória, Vasco,
Mirassol, Corinthians, São Paulo, Coritiba, Botafogo, Bragantino and Santos.
Additional nonautomatic above restrictions include Athletico-PR, Atlético-MG,
Bahia, Cruzeiro and Fluminense. Flamengo is already above on points.
Internacional and Grêmio must remain below; Remo and Chapecoense cannot
reach Palmeiras' points total. Other GD/GF restrictions also remain.

Completed suffix proofs enforce necessary PW feasibility and bound extrema
of guide likelihood over feasible completions. They do not compute the exact
weighted residual sum. A low-guide-likelihood feasible completion can keep
an entire prefix cap high even if that completion has tiny conditional mass.
The native event bound combines team-wise marginal upper bounds through a
minimum, rather than multiplying independent rival probabilities or exactly
integrating their shared-fixture joint event. This is conservative but loose.
The tree must partition further before those maxima cease to cover so much
prefix mass. Joint native/guided caps add little because native importance
weights are nearly constant inside the complete baseline forest.

The remaining score factor is generally bounded by 1. Mirassol's GD event is
already conditioned in its 90 tied sources; its other 66 sources are strict
on points/wins. The latest additional Vitória bound applies in only 818 of
39645580 bound evaluations. It can improve a local factor by about 30x,
but did not improve the full-budget upper materially.

Neither the old relaxed envelope's 92.17% strict-Mirassol share nor the new
818 applications determines the mass distribution of the retained frontier.
Current receipts do not attribute its upper by source result group, binding
PW bound versus residual cap, or each rival's remaining GD requirement.
Therefore we cannot establish that GD or shared PW feasibility is the largest
remaining structural loss. A source/rival-weighted frontier audit is needed
to resolve that narrower question; increasing GD depth blindly is unsupported.

## Priorities supported by this analysis

1. Rebalance work toward cohort 9 on the hybrid path. Its binding tail has
   few draws and its observed contribution mostly remains in residual. Test
   more baseline/residual draws, ordinary sampling for that cohort, or a
   cohort-specific fixed clip choice, with valid epoch/confidence accounting.
2. Apply residual-specific expectation/cap analysis to cohort 4 rather than
   spending more draws merely improving its small guided-core interval.
3. For cohort 5, measure source-group and rival-weighted frontier upper,
   including which bound is limiting. Extrema need to be replaced or refined
   by weighted low-guide-tail analysis; the present cache-size limit is not
   being hit. Multiple rival GD bounds may help coverage, but are not yet
   shown to target the dominant retained upper.

Most of the remaining upper is an unseen-tail or worst-completion allowance.
The observed score-stream means suggest contributions around 1e-40 in
cohorts 5 and 9, but they do not establish qualified per-cohort point estimates
or permit that allowance to be discarded.

## Reproducible evidence

The confidence decomposition uses `confidence.py`: base allocation
`0.01/(6840*16)`, cohort/epoch/look telescoping allocation, then division
across three streams and 482 one-sided threshold endpoints. Final-look
stream bounds are recomputed from saved cumulative 241-threshold vectors.
All three relevant retained cohort uppers come from the baseline comparator;
only cohort 5 is subsequently replaced through the explicit decomposition.
No historical threshold counts or raw receipts were modified.

| Local receipt | SHA-256 |
| --- | --- |
| `lazy-paired-pal16-180s.json` | `6e6434b9e89217c70eb6e70a74371e9f5981370a20767204591f6905fe983573` |
| `joint-ratio-pal16-180s.json` | `5badc49fd8a559660198d38db024322de687c71972d6b1b8871dba181df60136` |
| `score-gd-pal16-180s.json` | `921e83df5b926deae2de7f832e6845b5bab231aa8190bedade7526bd6d7b4ab8` |
| `gd-rebuild-pal16-historical-180s.json` | `79cdfc83ce7133e67885ffa3181b035e7c5e4fa4e87887cbf9cb2028e0eee8ac` |

Raw JSON stays local. Source and aggregate reports are backed up on
`experiments/rare-odds-recovery-2026-10-17` in `galo2099/golaberto`.
