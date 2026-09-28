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

## Faster first-place search prototype (not shipped)

The deep implementation simulates scorelines and sorts the standings
for every conditioned season. A first-place result can be screened by
points first: a rival above the target makes the draw a certain miss;
a strict target lead makes it a certain hit; only a points tie requires
scorelines and the full sorter. In one million draws for each team,
the points-only prototype found just 61 Botafogo-SP and 54 Avaí draws
that could still finish first. All 115 were points ties. The two
million points-only draws took 8.2 seconds in the prototype, compared
with 18.5 seconds for a full deep request. The comparison includes
different surrounding work, so it is an indication of savings, not a
full-request benchmark.

A second prototype sampled each remaining game's win/draw/loss outcome
only among outcomes that kept both teams at or below the target's final
points. It multiplied each path by the product of the probabilities of
the allowed outcomes. That weight makes the estimate unbiased under
the same match model. The full Go sorter ran only for paths that passed
the points check. With 100,000 weighted draws per team and the same
three-rival and five-rival conditioning events, respectively, it gave:

| Team | Weighted first-place estimate | Within-run estimated relative SE | Draw time after event construction |
| --- | ---: | ---: | ---: |
| Botafogo-SP | `5.71e-13` | 9.7% | 0.33 s |
| Avaí | `3.07e-19` | 6.5% | 0.73 s |

The two event constructions and weighted runs together took 2.5
seconds in the prototype. Independent 100,000-draw Avaí seeds gave
`3.68e-19` and `2.87e-19`; the within-run error estimates did not
fully capture this spread. Some paths carried 1.6–2.9% of the total
hit weight. Using four rather than five rivals reduced dynamic-program
size, but made the weight tail worse: estimates were `2.46e-19` at
100,000 draws, `2.98e-19` at 500,000 draws, and `3.68e-19` in a
separate million-draw run. The largest path supplied 6.9% of hit
weight in that last run. These results are exploratory; no weighted
search code was added to production.

The next candidate is a sequential proposal that looks ahead to each
rival's remaining fixtures, or a particle sampler that resamples when
weights concentrate. It should be checked across independent seeds
against the full conditioned-search reference. A points-first filter
can be added independently because it does not change the probability
estimator. Early rejection assumes nonnegative points earned per game;
other scoring rules need a remaining-points bound instead.

## Exact points screen and lookahead experiment

The points screen is now used by the million-draw deep first-place
search. After drawing win/draw/loss outcomes, it rejects a season as
soon as a rival exceeds the target's fixed final points. A strict
points lead is counted immediately; a points tie goes through the
existing conditional scoreline sampler and Go standings sorter. The
screen falls back to the original full-season simulation when points
per game can be negative or fixtures include teams outside the group.
It preserves the same probability distribution and does not change
the ordinary request path.

The weighted lookahead is enabled by default after the ordinary search
has no first-place hits. Set `RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD=0`
to disable it. It
conditions on three blocking rivals, or four when the three-rival
event mass is below `1e-9`. For each remaining game, the proposal
scores each win/draw/loss outcome using the rivals' marginal chances
of staying below the target's final points over their remaining
fixtures. The sampled path receives the exact product of original
outcome probability divided by proposal probability. Scorelines are
sampled conditional on the outcomes only when teams tie on points.
Thus the proposal changes variance and cost, not the expected estimate.

The production mode uses 20,000 weighted pilot draws per zero
first-place cell.
It accepts the proposal only with at least 100 finishing paths,
effective sample size at least 200, estimated relative standard error
at most 10%, largest hit weight at most 3% of the total, and a gap of
at most 20% between the two half-sample means. If accepted, a separate
independent 20,000 draws produce the estimate. Separating the pilot
prevents selection on the estimate's own sampling noise. A rejected
pilot falls back to the points-screened million-draw search. The
million-draw fallback remains opt-in with
`RARE_POSITION_CONDITIONED_ZERO_DEEP=1`; the ordinary request now uses
lookahead automatically when conditioned zero search is enabled.

Group 16653, phase 4489, seed 808, full matrix after reconciliation:

| Team | First-place probability | Weighted finishing paths / draws | ESS | Largest hit-weight share |
| --- | ---: | ---: | ---: | ---: |
| Botafogo-SP | `6.412e-13` | 7,770 / 20,000 | 1,422 | 0.51% |
| Avaí | `3.383e-19` | 9,279 / 20,000 | 1,010 | 0.74% |

In four independent 20,000-draw prototype seeds, Botafogo-SP ranged
from `5.88e-13` to `6.21e-13`, and Avaí from `3.21e-19` to
`3.37e-19`. These estimates are consistent in order of magnitude
with the earlier million-draw conditioned search. Between-seed
agreement and weight concentration matter more here than the
within-run standard error alone.

Apple M2 Pro full-request benchmarks on the same saved input:

| Configuration | Time per request | Allocated bytes per request |
| --- | ---: | ---: |
| Ordinary request | 539.3 ms (3 runs) | 118.9 MB |
| Million-draw deep search with exact points screen | 9.79 s (1 run) | 385.6 MB |
| Lookahead, 20,000 pilot plus 20,000 estimate draws per zero first-place cell | 1.568 s (3 runs) | 152.6 MB |

Lookahead took 2.91 times the ordinary request time and was 11.8
times faster than the earlier 18.46-second deep search. Group 16982
still has no zero cells, so the lookahead mode does no extra search
there. The Go package tests, `go vet`, and the saved group 16653 and
16982 regressions passed.

## Second-place zero in group 16653

The first-place lookahead found Botafogo-SP (team 457) at `6.412e-13`,
while its second-place cell stayed zero and `undecided`. This was a
sampling miss, not a reachability result. In a saved first-place finish,
Botafogo-SP has 59 points and every rival has at most 58. Changing
game 364696 (team 22 versus team 550) from a draw to a home win gives
team 22 60 points and leaves Botafogo-SP second on 59. The change has
positive probability under the game model.

The ordinary second-place search conditions on two likely blocking
rivals and draws 2,000 seasons. Its necessary points event has mass
`2.823e-5`; no second-place finish appeared. A third blocker shrinks
that event to mass `1.340e-6`. Production now tries this three-blocker
event, up to 250,000 dynamic-program states and 50,000 conditional
draws, only when a second-place or second-to-last zero remains and the
original event mass is at most `1e-3`. The event is still a necessary
condition, so `event mass × conditional finish frequency` is an
unbiased estimate. If the deeper event cannot be built, the original
result remains.

On the same saved group 16653 request and seed 808, the full estimator
found Botafogo-SP second 10 times in 50,000 conditional draws. Its
reconciled probability is `2.680e-10`, with estimated standard error
`8.47e-11` (31.6% relative); first place remained `6.412e-13`.
The total recorded work rose from 25.83 million to 41.58 million work
units (1.61×), and the test run rose from about 1.6 to 2.95 seconds.
The same search found no second-place finish for teams 68 and 95, so
those cells remain undecided with tighter 95% upper limits of
`3.48e-12` and `9.81e-17`, respectively.

## Difficulty-based extra-search allocation

The fixed 50,000-draw escalation for second and penultimate place is
replaced by a request-wide budget of 200,000 conditional draws. Every
zero cell not ruled out by points can compete for one 50,000-draw
batch. For each cell, the scheduler measures its current 95% zero-hit
upper bound and the upper bound achievable by a fresh batch. When
three tracked rivals can constrain the requested rank, it builds a
more selective points event (up to 120,000 states) and uses its exact
mass. A cell's priority is the log reduction in its upper bound,
capped at a `1e-12` probability floor. A cell already within ten times
that floor receives no extra search. The top four candidates get
independent conditional draws. Only those fresh draws supply a new
point estimate, avoiding selection of a lucky pilot hit.
Position number does not enter the priority score or the draw budget.
The three-rival event is only attempted where three rivals can
mathematically exclude the rank by points; other ranks remain eligible
under their target-points event. First-place weighted lookahead remains
a separate rank-specific proposal because it relies on the condition
that no rival can finish above the target. A rejected lookahead pilot
now returns to the ordinary zero-cell result unless the million-draw
deep-search flag is explicitly enabled.

On the saved group 16653 request with seed 808, the selected extra
cells were Botafogo-SP 2nd, Avaí 2nd, team 95 3rd, and team 22 17th.
Botafogo-SP 2nd was observed 9 times in 50,000 draws under event mass
`1.340e-6`, giving a reconciled estimate of `2.412e-10` and a relative
standard error near 33%. Team 95 3rd stayed zero but its 95% upper
bound fell to `3.47e-12`; team 22 17th stayed zero with upper bound
`5.20e-7`. The request recorded 46.83 million work units, 12.6% above
the fixed-position escalation's 41.58 million. A small-group
regression confirms that the scheduler can select the middle rank when
it offers the best gain.

Direct three-run full-request benchmarks on Apple M2 Pro, comparing
commit `acaeb6ce` (fixed-position extra search) with `5a899e42`
(difficulty-based allocation), gave:

| Group | Previous time | New time | Time change | Previous allocated bytes | New allocated bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| 16653 | 3.005 s | 3.704 s | +23.3% | 244 MB | 366 MB |
| 16982 | 0.954 s | 0.959 s | +0.5% | 48.0 MB | 48.0 MB |

Allocated bytes are cumulative per-request allocations, not peak
resident memory. Group 16982 had no zero cells and spent no extra
draws; its small time difference is benchmark noise. Candidate point
event construction adds cost beyond the recorded simulation-work units.

## Four-core latency optimization

The same saved group 16653 request and seed 808 previously took 3.704 s
per full request. CPU sampling showed substantial time in points-event
maps, scoreline simulation, standings sorting, and memory management.
The first-place lookahead setting also forced the initial zero-cell
search onto one worker, even when other cells were independent.

The zero-cell search now uses at most four workers. Each conditional
draw first computes every team's final points from win/draw/loss
outcomes. It generates full scorelines and applies the existing
standings tiebreakers only when teams tied on points can affect the
target rank. This preserves the point-event mass, draw count, and
conditional estimator; it changes the random stream and therefore
individual Monte Carlo hit counts. Candidate event refinement and the
four selected 50,000-draw extra batches also run across four workers.
The points dynamic program caches the rival starting points and target
maximum outside its inner state loop.

On Apple M2 Pro with `GOMAXPROCS=4`, the five-run full-request benchmark
for group 16653 averaged **0.925 s/request**, with 389 MB cumulative
allocations. The saved-request regression completed in 0.947 s, and
five independent single-request measurements ranged from 0.924 to
0.957 s. The scout remains at 100,000 seasons, the extra-search budget remains
200,000 conditional draws, and recorded work remains 46.83 million
units. Botafogo-SP first and Avaí first retained their lookahead
estimates (`6.412e-13` and `3.383e-19`); Botafogo-SP second was found
7 times in its 50,000-draw batch, with estimate `1.876e-10`. This is
consistent with the prior 9-hit estimate given its sampling error.
For group 16982, which has no zero cells, the three-run benchmark was
0.868 s/request versus the earlier 0.959 s measurement.

The full Go suite, the saved group regressions, and the race detector
on the parallel zero-cell paths passed. The new all-rank points-screen
test compares its conditional hit rates with the original full
scoreline sampler.

## Rank-general guided search

The previous search gave first place a 20,000-draw guided pilot and a
fresh 20,000-draw estimate, while initial draw counts and blocker
counts had explicit first/last and second/penultimate branches. The
production search now uses the requested rank as a numerical constraint
throughout, with no branch naming a particular position:

- The narrower side of the rank determines whether a small set of
  tracked rivals can constrain it. Initial point events use at most
  three blockers; the adaptive extra-search planner can build a deeper
  event if it measurably reduces the upper bound.
- Initial conditional draws use the same event-mass rule for every
  cell: 2,000 draws for event mass in `[1e-7, 1e-3]`, otherwise 1,000.
- Every still-unobserved cell gets a 200-draw guided pilot. A pilot
  with at least 20 hits can compete for a fresh 20,000-draw estimate.
  At most three estimates run per request, selected by pilot hit count
  with effective sample size as a tie breaker. The pilots choose cells;
  only fresh draws that pass the precision checks supply the reported
  estimate.
- The optional deep mode uses the same pilot evidence to choose up to
  two cells for one million ordinary conditional draws each. It no
  longer chooses by position number.

The guided proposal forecasts each rival's chance of finishing above,
equal to, or below the target's final points from the remaining
fixtures. It weights the three possible outcomes of the next match by
those forecasts and by how many rivals the requested rank permits on
each side. At rank 1, the allowed number above is zero, recovering the
earlier first-place proposal. At the last rank, the allowed number
below is zero. For intermediate ranks, both sides contribute. These
marginal forecasts are only proposal heuristics; the product of the
original-outcome/proposal-outcome ratios corrects their approximation.
Full scoreline tiebreakers still decide point ties.

An all-rank comparison test found and fixed one incorrect shortcut:
teams currently below the target may still gain enough points to catch
up, so only a rival already above a target with fixed final points can
be pruned early. A separate scheduler test confirms that a middle-rank
cell can receive a guided estimate.

On saved group 16653 with seed 808, the generalized search retained
nine conditional witnesses. Botafogo-SP first was `6.019e-13`, Avaí
first `3.257e-19`, and Botafogo-SP second `1.876e-10` (7 hits in
50,000 extra draws). Ceará first was also witnessed by the guided
search. The scout remains 100,000 seasons and the extra-search budget
remains 200,000 draws. Recorded work was 43.47 million units, versus
46.83 million before this change. With four cores, five independent
full-request measurements ranged from **0.947 to 0.981 s**; a five-run
benchmark averaged **0.949 s/request**. Group 16498 averaged 0.875 s
over three requests. The saved group regressions, full Go suite, and
race detector passed.

### Production flag and seed regression

The matched-point pool path takes precedence over the legacy importance
sampling path. Consequently, with `RARE_POSITION_MATCHED_POINT_POOL=1`,
the importance sampling, diversified IS, CEM parameterization, sequential
pruning, and minimum interesting probability flags do not affect this
estimator. Conditioned zero search and its guided lookahead are enabled
by default, unless their flags are explicitly set to `0`.

The first 200-draw guided pilot was initially using effective sample size
at least 10 and a maximum single weight share of 20% as promotion gates.
Across seeds 801–812 on the saved group 16653 request, those gates
sometimes prevented Botafogo-SP or Avaí from receiving the independent
20,000-draw estimate, despite dozens of pilot hits. With the user's exact
flag combination, Botafogo-SP first remained zero for four of the 12
seeds and Avaí first for four. The short pilot now requires at least
20 hits, ranking eligible cells by hit count; only the independent
20,000-draw estimate must pass the existing precision checks. With the
same 12 seeds, Botafogo-SP, Avaí, and Ceará each received a positive
first-place estimate in all 12 requests. Seeds 804, 805, and 811 now
have an environment-gated regression test because they exposed the
original failure. The four-core, five-request benchmark measured
0.932–0.979 s/request, averaging 0.947 s, versus 0.949 s before the
fix. This improves seed robustness for this saved request;
it does not guarantee that every future draw or changed fixture set
will find every extremely rare event.
