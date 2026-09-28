# Joint fixture point-cap propagation

## Hypothesis

A zero-probability cell can sometimes be proved impossible by combining a team's maximum attainable points with point caps on every other team. Shared fixtures may make those caps mutually inconsistent even when each team's individual point range appears feasible.

## Sound rule

For a target team and zero-based finishing rank `r`, let `M` be the maximum points the target can attain. At most `r` other teams can finish with **more than `M`** points. Teams already above `M` consume those slots. For any chosen set of other teams allowed above `M`, every remaining team must finish on at most `M`.

The solver starts every remaining fixture with all three win/draw/loss outcomes. It computes each constrained team's minimum possible final points from the current outcome domains and removes a fixture outcome only when that outcome plus all other fixtures' minimum gains would exceed `M`. It repeats until no domain changes. If a team exceeds its cap or a fixture loses all outcomes, that choice of above-cap teams is impossible. It tries other above-cap choices, up to 500 propagation nodes per cell and 10,000 per request. It records impossibility only when every choice has been exhausted without hitting a limit. Equal-point teams are allowed, so tiebreakers cannot make an impossibility result unsound. The rule runs only when points are the first standings criterion.

## Group 16653 result

Saved phase 4489 request, seed 808, matched point pool, four cores. After the existing sampling and neighborhood-witness stages, **23** zero cells were undecided. The point-cap pass examined all 23 and proved two impossible:

| Team | Finish | Target's maximum points | Propagation nodes |
| --- | ---: | ---: | ---: |
| Londrina (95) | 2nd | 52 | 1 |
| Team 125 | 9th | 44 | 1 |

The other **21** remain undecided. The pass used 44–86 propagation nodes in observed runs; the count varies with the internal team index order. No cell reached its 500-node limit. Each of the 21 had a consistent local point-domain relaxation, so increasing the node budget alone cannot resolve them. For Londrina, one team already has 54 points. Novorizontino and Fortaleza each have 51 and must draw their mutual fixture and lose the rest to stay at or below 52. Atlético-GO then wins both games against them, moving from 49 to 55. Two teams are strictly above Londrina's maximum, ruling out 2nd.

The focused Go benchmark, which runs the entire pass on the same 23 cells after preparing the request once, measured **148–159 µs per pass** over three 100-iteration runs on an Apple M2 Pro. Full-request five-iteration benchmarks measured 939.7 ms with the pass off and 934.3 ms with it on; the difference is within run-to-run noise. The pass contributes well below 1 ms for this group. On group 16498 it checked 31 undecided cells, proved none, and logged about 80 µs. Groups 16982, 16983, and 16986 had no zero cells under this configuration.

## Validation and limits

A hand-built fixture test reproduces the Londrina-style contradiction. A deterministic exhaustive test checks 40 small random schedules and verifies that the solver never rules out a rank attainable under any tie order. The real-group regression checks both new impossibility statuses; the full Go suite and the saved reference-group regressions pass.

This is a conservative proof pass. A nonempty set of local fixture domains is not a reachability witness, and the 500-node budget can leave a cell undecided. The pass does not change positive probability estimates or supply new odds. It is on by default; `RARE_POSITION_JOINT_POINT_CAP=0` disables it.
