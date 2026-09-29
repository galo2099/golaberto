# Conditioned rare-position search performance, 2026-09-29

## Setup

Measured full `/odds` calculations using `BenchmarkMatchedPointPoolFullRequest`,
saved requests for groups 16653, 16498, and 16982, `GOMAXPROCS=4`, a 20,000
season scout, and fixed random seed 808. Each timing below is the mean of
contemporaneous five-request benchmark runs on an Apple M2 Pro. The baseline
was the preceding commit; the optimized version uses the same requested work.

| Group | Baseline | Optimized | Wall-time reduction | Allocated bytes per request |
| --- | ---: | ---: | ---: | ---: |
| 16653 | 1.328 s | 1.035 s | 22% | 443 MB → 388 MB |
| 16498 | 1.368 s | 1.071 s | 22% | 468 MB → 367 MB |
| 16982 | 1.000 s | 1.000 s | ~0% | 48 MB → 48 MB |

Group 16982 spends little time in the conditioned searches changed here, so
this optimization has little effect on its full request time.

## Changes

- Pack each six-byte conditioned-point state into a `uint64` map key and add
  packed game deltas directly. This reduces forward-map memory and allocation
  overhead. The per-team overflow checks remain in place.
- Cache backward transition weights while drawing repeatedly from a point
  event. The cache is local to a sampling call and bounded to 4,096 states per
  game stage.
- Hoist repeated CDF lookups and per-fixture values out of the lookahead
  outcome loop.
- Run independent point-tilt pilots on up to four workers. Results are
  collected in original cell order before selecting production candidates.

CPU profiles showed the conditioned event builder, backward sampling, and
lookahead proposal among the largest costs. We also tried larger state-map
reservations, larger and smaller backward caches, an overflow-check shortcut,
and partial point-range pruning. Those were slower, neutral, or changed which
rare cells received estimates, so they were discarded.

## Correctness check

The full Go test suite passed. A new test compares cached and uncached
backward draws with identical random streams. Fixed-seed full-request snapshots
at seed 811 had the same nonzero cells and reported work before and after:

| Group | Nonzero cells | Lost | Gained | Maximum relative change on common nonzero cells |
| --- | ---: | ---: | ---: | ---: |
| 16653 | 337 / 400 | 0 | 0 | 0.0067% |
| 16498 | 360 / 400 | 0 | 0 | 0.0035% |
| 16982 | 400 / 400 | 0 | 0 | below 0.000001% |

Small numerical differences come from changed floating-point addition order
in the packed maps and parallel pilots. The candidate order and simulation
budgets remain unchanged.
