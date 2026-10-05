# R68 paired evaluation protocol

Written October 5, 2026 before the R68 screen.

The candidate executable and source archive are frozen under `data/`. All
requests run serially with four workers. Compilation finishes before timing.
Inherited `TREE_*`, `RUST_ODDS_*` and `RARE_POSITION_*` settings are cleared.
Each run records the exact request, executable, flags, hashes, wall time,
child process CPU time, export and diagnostic log.

The same executable with experimental controls unset is the matched control.
The frozen R67 production executable supplies a separate default parity check.
Initial arms isolate interval, necessary-rank and strict-side discovery;
wildcard ties, partial skeletons and relaxed family constraints; and two
preselected combinations (rank + skeleton, strict + relax).

Screen both retained group-16498 fixtures at seeds 808, 1669 and 1993. Select
arms using estimate quality and coverage jointly. Then evaluate selected arms
at reserved seeds 4211, 4229 and 4241 and on all other reference groups,
including historical group 16653, at seeds 808 and 1993. Rotate execution order
within pairs. For an arm considered for adoption, measure at least eight
timed pairs after two warmups on each affected retained fixture.

Inspect nonzero estimates as well as zeros. The R67 diagnostic means for
Flamengo/13th, Palmeiras/15th and Palmeiras/16th are provisional rough
references in fractions. They guide comparisons but do not certify accuracy.
Record raw MAIN/CHECK diagnostics, accepted estimates, concentration, family
and discovery selections, gains, losses and reachability changes. Report
charged work and grant violations separately from measured CPU and wall time.

Production request capacity, acceptance gates, final floors, pilot counts and
worker limit remain fixed. Reject candidates with unexplained losses or
budget violations; report every observed time increase. Code remains
experimental unless independent evidence supports adoption. No commit, push
or default activation is part of this evaluation.
