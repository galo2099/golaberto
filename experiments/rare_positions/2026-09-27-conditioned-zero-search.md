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

## Deeper first-place search for Botafogo-SP and Avaí

The ordinary conditioned search left Botafogo-SP (team 457) and Avaí
(team 68) first place at zero with `undecided` reachability. Both are
reachable: complete schedules found in the earlier witness experiment
give Botafogo-SP 59 points with every rival at most 58, and Avaí 57
with every rival at most 56. The ordinary three-rival necessary events
had masses `1.13766660424e-8` and `7.13413982305e-11`, respectively.
With zero hits in 1,000 conditioned seasons, their previous 95% upper
bounds were approximately `3.40e-11` and `2.13e-13` as probabilities.

For Botafogo-SP, a million draws under the same three-rival event
produced 60 first-place finishes. For Avaí, a necessary event including
five rivals (Fortaleza, Vila Nova, Juventude, the rival with team ID
2064, and the rival with team ID 279) had exact mass
`5.81378882712e-15`. Its point-state dynamic program reached 211,680
states across 45 relevant games. A million conditioned draws produced
58 first-place finishes. The full Go standings sorter, including
scoreline tiebreakers, verified every hit.

| Team | Necessary-event mass | Hits / conditioned draws | First-place estimate (probability) | Approximate relative sampling error |
| --- | ---: | ---: | ---: | ---: |
| Botafogo-SP | `1.13766660424e-8` | 60 / 1,000,000 | `6.826e-13` | 13% |
| Avaí | `5.81378882712e-15` | 58 / 1,000,000 | `3.372e-19` | 13% |

Independent experiment seeds yielded 63 / 1,000,000 for Botafogo-SP,
and 126 / 2,000,000 plus 69 / 1,000,000 for Avaí. The Avaí estimates
from those seeds were `3.663e-19` and `4.012e-19`. This agreement
supports the order of magnitude, while the sampling uncertainty and
the underlying match-probability model still limit precision. Exact
binomial 95% sampling intervals for the independent 63-hit and 126-hit
runs are approximately `[5.51e-13, 9.17e-13]` and
`[3.05e-19, 4.36e-19]` as probabilities. These intervals do not
include uncertainty in match power estimates.

`RARE_POSITION_CONDITIONED_ZERO_DEEP=1` enables the deeper search.
It runs only after the ordinary conditioned search finds no first-place
finish. It uses one million additional draws, and when the ordinary
necessary-event mass is below `1e-9`, it tries five, then four,
blocking rivals with a 250,000-state cap. The flag is opt-in because
it adds roughly two million simulated seasons for this group.

Apple M2 Pro full-request benchmark on the same saved group 16653 JSON:

| Configuration | Time per request | Allocated bytes per request |
| --- | ---: | ---: |
| Ordinary conditioned search, deep flag off | 526.7 ms (3 runs) | 118.9 MB |
| Deep first-place search enabled | 18.46 s (1 run) | 518.3 MB |

The deep run used one million additional conditioned seasons for each
of the two previously zero first-place cells; Ceará was already found
by the ordinary search and did not get the extra draws. The sampled
season work in the request rose from 17.4 million to 227.4 million
season-work units, as reported by the Go estimator. Because the deep
benchmark has one run, treat its timing as approximate. The feature is
intended for offline or explicitly requested high-effort calculations,
not the default request path.
