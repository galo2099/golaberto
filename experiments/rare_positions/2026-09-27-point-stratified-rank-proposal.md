# Point-stratified rank proposal experiment

This experiment uses a saved group 16653 / phase 4489 database request
captured on 2026-09-27 and Avaí (team 68). The raw request remains under
the ignored `local/` directory, as described in this archive's README.
It reproduces the existing seed-808 regression. The objective is
to find Avaí's reachable but currently zero second- and third-place
cells without extrapolating their odds from first or fourth place.

## Method

For a target final point total, the point-outcome dynamic program
selects exactly the target's match outcomes producing that total. Its
event mass is known. The rank-general lookahead then samples remaining
match outcomes with a proposal directed toward the requested rank and
multiplies each hit by the original/proposal likelihood ratio. Thus the
reported value estimates `P(final points = p AND finish rank = r)`.
Different point totals are disjoint and could be summed for a complete
estimate. This test measures their contributions separately; it does
not label an incomplete sum as the full rank probability.

The existing proposal gives rival outcomes on the wrong side of the
target's points a weight between zero and one. The experiment raises
that weight to a positive *tilt* exponent. Tilt 1 is current production
behavior; larger values push harder while retaining likelihood-ratio
correction. The effective sample size (ESS), maximum hit weight share,
relative standard error, and half-batch consistency gates remain the
same. Tilt 8 demonstrates that more hits alone do not mean a more
reliable estimate.

## Avaí at 57 points

The exact probability of Avaí ending on 57 points is `6.77633444e-5`.
For each rank and tilt, three independent 100,000-draw estimates gave:

| Rank | Tilt | Hits per run | ESS per run | Estimated 57-point contribution | Precision gate |
| --- | ---: | ---: | ---: | ---: | --- |
| 2 | 0.5 | 0 | 0 | 0 | rejected |
| 2 | 1 | 2–3 | 1.8–3.0 | unstable, about `2.4–3.5e-14` | rejected |
| 2 | 2 | 891–965 | 563–600 | `1.51–1.65e-14` | 3/3 accepted |
| 2 | 4 | 8,524–8,686 | 520–624 | `1.60–1.73e-14` | 3/3 accepted |
| 2 | 8 | 8,240–8,457 | 167–171 | `0.69–1.00e-15` | rejected |
| 3 | 0.5 | 0–3 | 0–3 | unstable | rejected |
| 3 | 1 | 17–20 | 17–20 | `2.98–3.48e-11` | rejected |
| 3 | 2 | 1,106–1,151 | 956–990 | `2.79–2.84e-11` | 3/3 accepted |
| 3 | 4 | 32,603–32,870 | 7,430–7,570 | `2.81–2.87e-11` | 3/3 accepted |
| 3 | 8 | 37,877–38,073 | 3–8 | `1.13–2.42e-11` | rejected |

The moderate tilts agree across seeds and with each other. The extreme
tilt produces many hits but a few large weights dominate the estimate,
so its lower numbers should not be used.

With a smaller, five-seed budget, tilt 2 at rank 2 with 50,000 draws
produced 438–497 hits, ESS 269–319, and accepted estimates
`1.45–1.77e-14`. Tilt 4 at rank 3 with 20,000 draws produced
6,446–6,607 hits, ESS 1,440–1,540, and accepted estimates
`2.74–2.90e-11`. Sampling alone took 155–179 ms and 101–102 ms per
cell, respectively, on an Apple M2 Pro with four Go cores. Event
construction and request setup are excluded from those timings.

## Coverage beyond 57 points

A 20,000-draw pass over every rank-feasible point total with the same
tilts found:

| Rank | 57 points | 55 points | 54 points | 53, 52, 51 points |
| --- | ---: | ---: | ---: | --- |
| 2 | 200 hits; ESS 125 | 0 | 0 | 0 |
| 3 | 6,552 hits; ESS 1,480 | 568 hits; `1.59e-15`, ESS 207 | 33 hits; `1.71e-19`, ESS 11 | 0 |

The measured 55-point third-place contribution is about four orders
of magnitude below the 57-point contribution. Zeros at other totals
are still undecided: this pass does not establish useful upper bounds
for all of them. Second place has a robust *57-point contribution*,
not a complete probability estimate. The earlier 57-point third-place
estimate with tilt 1 and 45 hits in 200,000 draws was noisier; its
`3.99e-11 ± 6.0e-12` result is compatible with the better sampled
moderate-tilt values.

## Cost and production decision

The unchanged production request benchmark took 0.913–0.954 s over
five runs (mean 0.930 s) with four cores. Adding the two budget-sized
point-stratum samplers sequentially would add roughly 0.26 s before
setup, exceeding the under-one-second target. These draws should
replace work selected by the existing zero-cell scheduler, not simply
run afterward. Other groups and cells also need validation.

The experiment is intentionally **not enabled in production**. A
production version should pilot feasible point strata and modest tilt
values for every unresolved cell, allocate independent estimate draws
by expected precision gain per unit time, and combine disjoint strata.
It must keep an explicit unresolved bound or partial-estimate label
where point-total coverage is incomplete. An observed witness can mark
reachability without claiming that the partial probability is the
complete rank odds.

The synthetic rank-general comparison test now checks tilts 1, 2, and
4 against direct conditional sampling. The experimental tests are
disabled unless `RARE_POSITION_POINT_STRATA_EXPERIMENT=1` is set.
Export the current group request from the repository root, then run
the tests from `go/`:

```sh
bin/rails runner script/export_rare_position_groups.rb experiments/rare_positions/local 16653
cd go
GOMAXPROCS=4 RARE_POSITION_POINT_STRATA_EXPERIMENT=1 \
RARE_POSITION_BENCHMARK_GROUP_JSON=../experiments/rare_positions/local/group-16653.json \
go test -run 'TestPointStratified(RankProposal|Coverage|Budget)Experiment' -count=1 -v
```

If the group has changed since 2026-09-27, the numerical results will
change with the new fixtures and powers.
