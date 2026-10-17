# Where the GD-rebuild upper bound comes from

This audit uses the saved historical Palmeiras16th run
`gd-rebuild-pal16-historical-180s.json`, SHA256
`79cdfc83ce7133e67885ffa3181b035e7c5e4fa4e87887cbf9cb2028e0eee8ac`.
No new Monte Carlo observations are added and no estimator code is changed.
The structural audit rebuilds the same proposal in an isolated scratch copy,
without changing the measured native executable.

## Global decomposition

The global upper `1.719720850889973e-37` is the outward-rounded sum of the16
cohort uppers. Uncovered mass is zero.

| Cohort | Upper endpoint | Share of global upper | Final-epoch draws | Nonzero upper-score contributions |
|---|---:|---:|---:|---:|
|5|1.5645417326444608e-37|90.976%|3995872|34|
|9|9.346141459509459e-39|5.435%|397000|10150|
|4|5.829967548308041e-39|3.390%|339000|10|
|Other13|3.418028167337079e-40|0.199%|various|various|

The top three account for99.801% of the global upper. Cohort9 provides
`3.1047963797098733e-41` of the global lower `3.105431383236009e-41`:
almost all of the statistically established lower contribution comes from it.
The nonzero counts above are weighted score contributions, not ordinary
Bernoulli counts of final-position events.

## Cohort 5: almost all of the upper is an unseen-tail allowance

Its final proposal has certified per-observation cap
`B=1.335714790055119e-32`. The final epoch has3995872 draws; the cohort has
4000000 draws across all proposal epochs. For the normalized upper score
`Y=upper_weighted_score/B`, no observation reaches1/128. The first threshold
with positive counts is1/256, with two exceedances. In absolute units, the
zero-count threshold1/128 is `1.0435271797305617e-34`.

The retained cohort upper is its last sample look: epoch6, look3995. It
recomputes exactly from the saved final threshold counts using the existing
confidence.py implementation. The family allocation is
`0.01/(6840*16)`, followed by the existing cohort/epoch/look allocation and
division across482 one-sided threshold bounds. The resulting per-threshold
tail alpha is `5.046379787669442e-21`.

Even with zero exceedances, the exact binomial upper probability is
`1-alpha_tail**(1/n) = 1.1695905807838745e-5`. This gives the characteristic
upper scale

```text
B * zero_count_CP_upper
  = 1.3357e-32 * 1.1696e-5
  ≈ 1.5622e-37
```

The exact dyadic decomposition is:

| Part of cohort5's upper calculation | Contribution | Share |
|---|---:|---:|
|Threshold strips with zero exceedances, j1 through7|1.5500344414601293e-37|99.073%|
|Threshold strips with positive exceedances|1.45072911843315e-39|0.927%|
|Bottom unresolved dyadic slab|7.559877800e-105|negligible|

The positive-count portion also contains confidence padding; it is not simply
the mass observed in simulation. The empirical upper-score mean is
`4.2137456345237857e-41`, about3713 times below the confidence upper. The
empirical lower-score mean is `3.2813000856641755e-42`, leaving an observed
lower/upper score-bracket gap of `3.885615625957368e-41`. These empirical
means are diagnostics, not qualified point estimates or confidence bounds.

Closing only that observed score-bracket gap would not remove the dominant
zero-count tail allowance. Stronger analytical integration could still help
if it lowers the certified cap B itself.

The same pattern appears in the next two cohorts. Zero-exceedance strips
account for88.217% of cohort9's upper and99.973% of cohort4's upper. The
zero-exceedance contributions from cohorts5,9,4 together account for98.316%
of the entire global upper. Both next-cohort uppers also recompute exactly
from their final epoch counts and last sample looks.

## Where cohort 5's cap comes from structurally

A deterministic scratch replay reproduces the exact saved nine-rival proposal:
joint cap `1.0140615550832632e-31`, then the team584 compact GD proposal at
`1.335714790055119e-32`,156 assignments,92975 shared suffix states and166182
total retained states. The shared forest is complete.

For assignment a, the certified mixture cap is the maximum of
`A_a/rho_a.lo`, where `A_a=P_a.hi*q_a.hi*C_suffix_a`, and rho_a is the actual
categorical proposal probability. Summing A_a gives a certified envelope for
the relaxed event: the selected joint points/wins constraints plus the
team584 GD condition when tied. The total is `1.33571479005433e-32`, within
`5.91e-13` relative of the mixture cap. It is not the probability of the full
Palmeiras16th event.

| Team584 remaining results | Assignments | Final points/wins | GD needed against Palmeiras? | Sum certified A terms | Share of relaxed envelope |
|---|---:|---|---|---:|---:|
|9W/1L|10|59/17|No|9.184534261991797e-33|68.761%|
|8W/2D|45|58/16|No|2.691334117165097e-33|20.149%|
|8W/1D/1L|90|57/16|Yes|1.0458188257985927e-33|7.830%|
|9W/1D|10|60/17|No|4.293088747593709e-34|3.214%|
|10W|1|62/18|No|6.151820828439573e-36|0.046%|

Thus92.170% of this envelope comes from strict points/wins branches. Further
GD conditioning on team584 itself addresses only the tied7.830% portion.
Mixture probabilities are proportional to each prefix's nominal joint/GD
mass, so per-draw caps are nearly equal across assignments. The literal
maximum is a tied assignment, but exceeds the strict maximum by only about
`8.1e-45`; that rounding difference is not a meaningful structural bottleneck.

The cohort requires15 rivals above Palmeiras. Nine are jointly selected:
`[67,18,584,24,14,10,7,225,6]`. The six required-above rivals outside that
selected set are `[5,4,74,15,17,8]`. Other remaining ranking restrictions
include score tie-breaks for selected rivals and the required-below teams.
The score integrator checks the full cohort event; the proposal's relaxed
envelope has not already enforced every one of those conditions.

The next useful structural target is team584's9W/1L group, then8W/2D:
identify which other joint points/wins and score-ranking restrictions can
tighten those groups. These envelope shares do not imply corresponding shares
of actual finishing probability, observed hits or the final confidence upper.
The saved run does not label observations by prefix or rejection reason, so
those attributions require additional diagnostics.

## Checks and limits

The confidence decomposition was independently recomputed using both the
existing Python confidence routines and a separate zero-count formula check.
An independent Sol review confirmed the structural cap arithmetic and the
interpretation of its group shares. The frozen native executable remains
`637b7e7726fe14e1520cae06b598141215b2111c9bbaac876c5bb030d2cf4aa1`.
Earlier per-look threshold vectors are not saved, but are unnecessary for these
three cohorts because each retained upper comes from its final look. The
existing numerical assumptions in the confidence implementation are unchanged.

Scratch audit scripts, per-prefix records and raw diagnostics remain local;
this aggregate analysis is backed up with the reconstruction branch.
