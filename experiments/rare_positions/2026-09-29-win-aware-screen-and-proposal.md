# Wins-aware rank screen and importance proposal, follow-up (2026-09-29)

## Question and setup

Does using wins after points in `pt,w,...` phases save screen time or discover
more unresolved rank cells without displacing estimates the point-only path
already finds? Compare complete 4-core odds requests with identical seeds.
Use the matched point pool, conditioned zero search, lookahead, point tilt, and
undecided point tilt on both sides. The experiment initially kept the
wins-aware path opt-in.

Inputs: group 16653 current snapshot (85 unplayed, SHA-256 `b87207393a283ae4b7cc986c49a60fdd0943057295872ddcda27926d9d6597dd`),
group 16653 older reference snapshot (SHA-256 `2d1c1d6f70a64d5b86f5129b32cbf77bedc4c42e464cd849011ca0b3018b3d87`),
and group 16498 reference snapshot (103 unplayed, SHA-256 `b37c82f317d5f0e412b2642101929cca778857c2792dc89c0fbb830e702e701e`).
The two older inputs have independent 5-million-season references.

## Isolating the screen

The fast screen previously used the same RNG for fixture W/D/L outcomes and
conditional scorelines. Skipping scorelines with a `(points,wins)` screen
therefore changed later fixture outcomes even when it classified the same
season. Experimental `RARE_POSITION_PAIRED_SCREEN_RNG=1` separates those
streams and reseeds scoreline sampling per draw. A toy `pt,w,...` group had
exactly matching rank-hit counts for every tested cell with point and
point/win screens (5,000 paired draws per cell).

Across seeds 808, 811, and 812, the paired full requests had no changed
nonzero cells on either group 16653 current or group 16498. The screen saved
about 2.7% wall time on group 16653 current and 0.7% on group 16498. These
are short local timings, not evidence of a dependable production speedup.
Enabling paired RNG changes the random stream relative to today's default.

## Keeping selection stable while testing the proposal

With the original wins-aware lookahead, group 16653 seed 811 lost Avaí's
first-place estimate. A trace showed the guided search did select Avaí under
the original point-based pilot. Its wins-aware confirmation had 9,149 hits,
ESS 991 and an estimate of `3.335e-19`, but failed the existing two-half
consistency gate (gap 0.2125; limit 0.2). The point-based confirmation had
9,088 hits, ESS 706 and estimate `3.361e-19`; its gap was 0.0594 and passed.
The loss therefore did not mean the wins-aware proposal could not find Avaí.

The follow-up candidate keeps all guided, point-tilt, and gap pilots on the
point-only proposal, preserving their slot ordering. A rejected wins-aware
guided or main point-tilt confirmation gets a fresh point-only confirmation
with the same seed, and its work is counted. Accepted wins-aware confirmations
keep their own likelihood-ratio estimate. The fallback is limited to those
two confirmation paths; the gap confirmation path still uses its existing
two-proposal sequence.

Full-request pairs for seeds 808–822:

| Input | Newly nonzero cell-runs | Lost nonzero cell-runs |
| --- | ---: | ---: |
| 16653 current | 0 | 0 |
| 16653 older reference | 6 | 0 |
| 16498 older reference | 15 | 0 |

These are **cell-runs**, not 21 distinct team/position cells. In the old
16653 snapshot, examples include Avaí 2nd (`2.22e-15`) and team 70 17th
(`3.80e-11`) at seed 811. Group 16498 team 8's 18th-place estimate was
typically around `2e-12` to `4e-12`. Some gains are weak: team 5's 17th-place
estimate ranged from `1.52e-13` to `4.08e-11` across observed seeds, often
with ESS near 10. Treat these as discovery or rough estimates, not calibrated
precision at that scale.

The 5-million-season references cannot validate probabilities this small.
For the 11 reference cells near `1e-6` in group 16498 and 10 in old group
16653, baseline and candidate RMSE were numerically indistinguishable across
seeds 808–812. The candidate offers no measured improvement at `1e-6`.

In those same five seeds, mean modeled work rose 6.4% for group 16498 and
4.7% for old group 16653 because rejected confirmations were retried. Mean
wall time changed from 1.322 to 1.315 seconds for 16498 and 1.250 to 1.274
seconds for old 16653. Local scheduler variation is larger than the apparent
speed difference. The candidate did not show a robust runtime win.

## Combined screen and proposal

With both wins-aware stages enabled, paired RNG, stable selection, and
fallback, seeds 808–812 yielded 3 gained/0 lost cell-runs on group 16498 and
5 gained/0 lost on old group 16653. The `1e-6` reference RMSE stayed
effectively the same. The screen's small saving did not clearly offset the
extra confirmation work.

## Decision and default

After review against the project's stated goal of replacing reachable zeros
with rough rare-position estimates within a 20% time allowance, enable the
stable-selection/fallback **importance proposal** by default for `pt,w,...`
phases. It improved coverage in 21 of 45 paired cell-runs, lost none, and
raised modeled work by about 5–6% in the measured reference requests. It did
not improve the `1e-6` band, and the newly found smaller estimates remain
uncalibrated against an independent deep reference. The rollout can be
reversed with `RARE_POSITION_WIN_AWARE_RANK=0`.

Keep the **fast rank screen** opt-in. Its 0.7–2.7% local speed saving does not
yet justify changing the random stream of direct conditional searches by
default. `RARE_POSITION_WIN_AWARE_RANK=1` plus
`RARE_POSITION_PAIRED_SCREEN_RNG=1` enables both stages. The wins-aware
proposal's point-based pilot selection and fallback are now automatic;
`RARE_POSITION_WIN_AWARE_STABLE_SELECTION=0` or
`RARE_POSITION_WIN_AWARE_FALLBACK=0` disable those protections for experiments.

A saved group 16653 full request produced the same matrix and work under the
new defaults and the equivalent explicit flags, within floating-point
rounding. The Go test suite passed after this default change.

Reproduce one full-request comparison:

```bash
RARE_POSITION_BENCHMARK_GROUP_JSON=/path/to/group.json \
RARE_POSITION_BENCHMARK_REFERENCE_JSON=/path/to/group.reference.json \
RARE_POSITION_RANDOM_SEED=811 \
RARE_POSITION_WIN_AWARE_CANDIDATE=lookahead \
RARE_POSITION_STABLE_SELECTION_CANDIDATE=1 \
RARE_POSITION_WIN_FALLBACK_CANDIDATE=1 \
GOMAXPROCS=4 go test ./... -run '^TestWinAwareRankFullRequestExperiment$' -count=1 -v
```

The default proposal requires no wins-aware flags. The separate screen
experiment uses `RARE_POSITION_WIN_AWARE_RANK=1` and
`RARE_POSITION_PAIRED_SCREEN_RNG=1`. Correctness checks and
`go test ./... -count=1` passed.
