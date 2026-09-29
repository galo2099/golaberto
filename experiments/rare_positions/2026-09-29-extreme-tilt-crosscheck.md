# Independent check for extreme point-tilt estimates (2026-09-29)

## Problem

The point-tilt search sometimes accepted a proposal with rank tilt 12 after
15,000 draws. A proposal can record hundreds of finishing-rank hits yet miss
rare outcomes carrying most of the importance weight. Observed effective
sample size and weight concentration cannot detect weight that was never
sampled. In group 16498, seed 808, this produced `1.63e-15` for Cruzeiro
(team 15) finishing 19th.

Two independent one-million-draw proposals instead gave `1.65e-12` and
`1.61e-12`. Bahia (team 74) finishing 19th gave `4.03e-12` and `3.88e-12`.
These are probability fractions. The target-only point event was exact under
the fixture model: Cruzeiro had a 3.43% chance of finishing with at most 53
points, versus Bahia's 11.43%. Their expected final points were 60.79 and
58.24. The teams' true 19th-place probabilities are in the same order of
magnitude; the production discrepancy was a proposal/validation problem.

The earlier group 16653 request showed the same failure for Avaí (team 68).
One-million-draw gentle searches estimated second at `1.25e-13` and third at
`1.05e-10`. A stronger proposal had many more raw hits but poor effective
sample size and did not provide a reliable estimate.

## Variants

1. **Strict agreement:** discard every accepted rank-tilt-12 result unless an
   independent gentle proposal also passes the full quality gate and agrees.
   This removed 23 existing nonzero cell-runs in 15 group 16498 requests;
   most events were too rare for the gentle check to resolve. Rejected.
2. **Evidence-based check:** for accepted rank-tilt-12 results, draw 50,000
   fresh samples with rank tilt 3 and a modest point tilt of magnitude 0.5.
   If the check has at least 30 hits, effective sample size at least 5, and
   relative standard error at most 0.6, compare the estimates. A ratio above
   30 is a contradiction. Replace the original estimate when the gentle one
   meets the normal validity gate; otherwise suppress the contradicted
   estimate. An inconclusive check preserves the original result. This check
   also covers the recycled and cross-team confirmation paths.

The check uses an independent random stream. Its 50,000 draws and work are
included in the reported search work. `RARE_POSITION_POINT_TILT_CROSSCHECK=0`
disables it; the evidence-based check is enabled by default.

## Full-request comparison

The same seed and saved request were run with the check off and on. All runs
used a 20,000-season scout and the 100,000-season matched
point pool. The first three inputs used seeds 801–812, 817, 900, and 911;
groups 16982 and 16983 used seeds 801, 802, 803, 808, and 809. The inputs
are saved outside the repository because they came from local database data.

| Input | Requests | Nonzero cells lost | Values changed by >1% | Median baseline / checked time |
| --- | ---: | ---: | ---: | ---: |
| Group 16498 | 15 | 0 | 1 | 1,023 / 1,103 ms |
| Group 16653, September 28 | 15 | 0 | 0 | 921 / 993 ms |
| Group 16653, earlier | 15 | 0 | 2 | 982 / 989 ms |
| Group 16982 | 5 | 0 | 0 | 866 / 871 ms |
| Group 16983 | 5 | 0 | 0 | 862 / 866 ms |

Across 55 fixed-seed request comparisons, the median seed-matched time
difference was +44 ms. These runs were not interleaved, so timing differences
include machine noise; the 50,000-draw check is a real additional cost when
triggered. There is still no hard one-second guarantee.

The three material corrections were:

| Request and cell | Before | After check | Independent million-draw reference |
| --- | ---: | ---: | ---: |
| 16498, Cruzeiro / 19th, seed 808 | `1.63e-15` | `1.03e-12` | `1.61–1.65e-12` |
| 16653 earlier, Avaí / 2nd, seed 807 | `3.68e-15` | `1.16e-13` | `1.25e-13` |
| 16653 earlier, Avaí / 3rd, seed 812 | `1.26e-13` | `1.02e-10` | `1.05e-10` |

The reference values are themselves weighted Monte Carlo estimates, not
exact probabilities. The check does not certify every extreme-tilt result:
when the gentle search has too little evidence, it leaves the original
estimate available. This avoids turning many rare but estimated cells back
to zero. The previous guardrail still applies to all accepted results.

`go test ./go` passes. The request and per-seed comparison JSON remain under
`/private/tmp/` and are not committed.
