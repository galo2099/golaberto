# Conditioned search for zero finishing-position estimates

## Method

With `RARE_POSITION_MATCHED_POINT_POOL=1`, the existing 100,000-season
scout and matched point pool run first. The new search examines every cell
whose reconciled estimate is zero. The simple point bounds still identify
positions that are impossible. For each remaining zero, it constructs a
necessary event involving the target's final points. For first and last
place it also tracks three likely blocking rivals; for second and
penultimate place it tracks two. Other ranks use the target's possible
point totals. All outcomes that put the target in the requested position
are inside the event, including positions decided by tiebreakers.

A dynamic program calculates the event's probability from the exact
win/draw/loss probabilities of the relevant fixtures. The search draws
win/draw/loss outcomes conditional on the event, samples Poisson
scorelines conditional on those outcomes, simulates every other fixture
normally, and applies the existing Go standings sort. If `h` of `n`
conditioned seasons finish at the requested rank, the estimate is
`P(event) * h/n`. Sample counts are fixed before these draws: 1,000,
2,000, or 20,000 depending on event mass and rank. A full finishing
season is a reachability witness. Zero hits retain a zero estimate and
receive the 95% upper bound `P(event) * (1 - 0.05^(1/n))` when it improves
the earlier bound. The resulting probability matrix is balanced again.

The search is enabled by default with the matched point pool. Set
`RARE_POSITION_CONDITIONED_ZERO=0` to disable it. At most four workers
search cells concurrently. A 20,000-state cap on the dynamic program
falls back to fewer rivals if needed.

## Group 16653, phase 4489

Input: saved group request `/tmp/phase4489-group16653-request.json`.
Seed: `RARE_POSITION_RANDOM_SEED=808`. Go benchmark used seven full
`calculate_odds` operations on Apple M2 Pro (arm64).

| Configuration | Time per full request | Allocated bytes per request |
| --- | ---: | ---: |
| Matched point pool only | 345.0 ms | 25.0 MB |
| Matched point pool plus conditioned search | 547.2 ms | 118.9 MB |

Increment: 202.2 ms, or 59%. The new search used 66,000 conditioned
seasons across 35 zero cells; its dynamic programs add work beyond those
season counts. Of 81 initial zero cells, 46 were ruled out by point
bounds, 35 were searched, and eight got a finishing witness and nonzero
estimate. The other 27 remain undecided rather than receiving an
unsupported positive number.

Ceará (team 69) first place: the exact necessary event involving
Fortaleza, Vila Nova, and Juventude had probability
`5.14342797285148e-7`. Four of 20,000 conditioned seasons put Ceará
first. The reconciled estimate was `1.02868561160287e-10` probability,
or `1.02868561160287e-8%`. The relative sampling error is about 50%,
so this is an order-of-magnitude estimate. The necessary-event mass is
also a model-based upper bound on Ceará's first-place probability.

## Group 16982

Input: saved group request
`/private/tmp/golaberto-real-fixtures/group-16982.json`. The matched
pool had no zero cells, so the new search did no conditioned draws.

| Configuration | Time per full request | Allocated bytes per request |
| --- | ---: | ---: |
| Matched point pool only | 920.8 ms | 47.9 MB |
| With conditioned search | 910.8 ms | 47.9 MB |

The time difference is benchmark noise; there is no measurable extra
work for this input.

## Validation and limits

The small-group test enumerates all 27 win/draw/loss patterns and checks
the dynamic-program mass, the necessary-event property, and the
conditional draw distribution. The real
group test checks Ceará's event mass, witness, nonzero estimate, and
matrix column sums. The Go package tests, `go vet`, and the race test on
group 16653 passed. The Rails suite passed (110 runs, 275 assertions,
one skipped test).

Conditioned search does not prove reachability for every remaining
zero within a fixed compute budget. A cell with no finishing witness is
reported as `undecided` with its upper bound. The point estimates are
unbiased before matrix reconciliation under the Poisson model, but the
few-hit estimates have substantial sampling uncertainty. The extra
allocated memory, about 94 MB per group 16653 operation, is the main
resource cost to monitor in production.
