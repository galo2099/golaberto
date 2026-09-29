# Cross-team witness reuse and prioritization (2026-09-29)

## Question

Can a full fixture assignment found while estimating one team identify useful
unresolved positions for other teams? Maximum attainable points or wins alone
cannot transfer an exact-position probability. A complete assignment can,
however, be checked against every team's standings rank and can seed a small
neighborhood search. Any resulting probability still requires a target-specific
estimator with full support and fresh confirmation draws.

## Variants

The existing neighborhood search runs before the point-tilt pilots. It reuses
witnesses from earlier conditional searches, but not from the later pilots.
Each point-tilt pilot already retains one complete win/draw/loss assignment
when it hits its target cell. I tested two ways to reuse these assignments:

1. **Reallocate:** run eight of the 12 normal point-tilt pilots, search the
   cross-team neighborhoods of their assignments, then choose the final four
   pilots from the updated proved/undecided list. The total pilot count stays
   at 12.
2. **Additive:** leave all 12 normal pilots and their confirmations unchanged.
   Search the cross-team neighborhoods of their assignments with a 4,000
   candidate cap. Mark verified positions reachable. Choose the newly proved,
   previously unpiloted cell with the largest existing 95% zero-hit upper
   bound. Give it up to three 1,000-draw proposal pilots, then at most one
   independent 15,000-draw confirmation. The ordinary validity gates still
   require effective hits, reasonable weight concentration, and agreement
   between sample halves. A reachability proof alone never becomes an odds
   estimate.

I also compared one versus two extra additive pilots. Both gained 15
nonzero cell-runs without losses in the broader seed set; one pilot reached
seven distinct cells, versus six with two pilots. The selected one-pilot
version costs less and preserves the normal pilot choices.

## Paired full-request results

The final one-pilot additive version was compared with the original algorithm
on the same request and random seed. For even seeds the additive request ran
first; for odd seeds the baseline ran first. There were 15 seeds per input:
801–812, 817, 900, 911. The service used four Go cores, 20,000 initial MC
seasons, and the 100,000-season matched point pool. Timings exclude compilation
and process startup. The saved requests were:

| Input | SHA-256 | Remaining fixtures |
| --- | --- | ---: |
| Group 16653, September 28 | `b87207393a283ae4b7cc986c49a60fdd0943057295872ddcda27926d9d6597dd` | 85 |
| Group 16498 | `44eabb47ee55477c77025c3de4e839c6ea913b8569dd29694d7178bdb8bcdc89` | 103 |
| Group 16653, earlier | `2d1c1d6f70a64d5b86f5129b32cbf77bedc4c42e464cd849011ca0b3018b3d87` | 90 |

| Input | Zero→nonzero cell-runs | Nonzero→zero | Distinct gained cells | Additional remaining-zero reachability proofs | Median paired time change |
| --- | ---: | ---: | ---: | ---: | ---: |
| 16653 current | 1 | 0 | 1 | 13 | +4.0 ms |
| 16498 | 6 | 0 | 4 | 32 | +22.7 ms |
| 16653 earlier | 8 | 0 | 2 | 2 | +31.2 ms |
| **Total** | **15** | **0** | **7** | **47** | **+13.4 ms overall** |

The seven distinct newly estimated cells were current-16653 team 95 / 6th;
16498 teams 110 / 4th, 318 / 5th, 15 / 20th, and 74 / 20th; and
earlier-16653 team 95 / 4th and 5th. The same cell under several seeds counts
several cell-runs. The additional proofs count cells that remained zero but
changed from undecided to reachable-by-construction in a request.

Across all 45 pairs, the median time change was +13.4 ms (+1.3%). The 90th
percentile was +66.3 ms (+6.9%). Nine baseline requests and 14 additive
requests exceeded one second on this machine. This is a modest but real cost;
the algorithm still has no hard one-second guarantee.

On the first five-seed-per-input comparison, reallocating four existing pilot
slots gained six nonzero cell-runs but lost seven. It changed which rare cells
received confirmation and was not retained. The additive version preserved
all baseline nonzero cells in the 45 paired requests.

## Decision and limits

Enable the one-pilot additive pass by default. Set
`RARE_POSITION_CROSS_TEAM_WITNESS=0` to disable it. It respects
`RARE_POSITION_NEIGHBORHOOD_SEARCH=0` and uses the same points/wins-aware
reachability verifier as the earlier neighborhood stage. The new target is
prioritized by its existing zero-hit upper bound, but its estimate comes only
from the target's own full-support point event and fresh weighted samples.

The newly estimated probabilities are mostly far below what a five-million-
season plain MC reference can resolve. These experiments establish extra
coverage and measured runtime, not ground-truth accuracy at `1e-11` or lower.
The existing importance-sampling validity gates are the basis for accepting
the new estimates. A saved-group regression checks one gained cell and that
the baseline's nonzero cells remain positive. `go test ./go` passes.
`go test ./...` still fails because archived local experiment directories
under `experiments/rare_positions/local/` contain standalone Go fragments
that do not build as separate packages; this change does not touch them.
