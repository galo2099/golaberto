# Tenfold work budget for remaining zero cells (2026-09-29)

## Summary

Increasing the modeled simulation work to ten times the current full request
found a median **6 additional estimates in group 16498, 9 in current group
16653, and 8 in the earlier group 16653 snapshot**. Across five seeds per
input, the respective ranges were 5–8, 8–12, and 5–11. Groups 16982 and
16983 already had estimates in all 400 cells and needed no extra search.

These are paired zero-to-nonzero gains within each run. Across the seeds,
there were 118 such cell-runs, representing 37 distinct input/team/position
cells. Nineteen of those 37 cells were zero in **every** baseline seed;
the other 18 already had an estimate in at least one baseline seed.
The two group 16653 snapshots are counted separately because their fixtures
differ. There were no nonzero-to-zero regressions: this experiment retains
every existing estimate by construction.

The added estimates are mainly extremely small tails, rather than missed
events around `1e-6`. Median new probabilities were about `2.65e-15` in
16498, `6.35e-14` in current 16653, and `4.34e-15` in earlier 16653.
The lowest accepted result was `4.11e-24`. Independent proposal checks
supported the order of magnitude of six selected cells, including that one.
This is evidence for useful tail recovery, not an accuracy guarantee for
every newly filled cell.

**All probabilities in this report are fractions, not percentages.**

## Experiment design

The baseline is the production implementation at commit `4f15cbbb`, with
matched point pooling, conditioned zero search, lookahead, point tilt,
undecided-cell search, extreme-tilt cross-check, and directional peer rescue
enabled. It uses 20,000 initial MC seasons and the 100,000-season matched
point pool. CEM importance sampling is disabled, matching the recent
production experiment configuration. Four cores are used throughout.

For each input and each seed 801, 804, 808, 817, and 911:

1. Run the full baseline request and retain its estimates.
2. Select every remaining zero that has not been proved impossible.
3. Build the target team's attainable-point event for each selected rank.
4. Pilot six proposals: rank tilts 3, 8, and 12, each with positive and
   negative point tilt. Each pilot gets up to 5,000 draws. Pilot selection
   uses the existing points-only screen; final sampling uses the wins-aware
   screen where the phase permits wins.
5. Select the proposal by pilot effective sample size, with first-hit
   fallback. Split the remaining budget equally across eligible cells.
   Use two thirds of each cell's final draws for the selected proposal and
   one third for a fresh gentle proposal with rank tilt 3 and point tilt
   magnitude 0.5. Pilot draws never enter the reported probability.
6. Apply the production acceptance gate: at least 30 hits, ESS at least 8,
   relative standard error at most 0.35, largest weight share at most 0.25,
   and batch gap at most 1. Prefer the gentle result when it passes. A
   conclusive gentle check that disagrees by more than 30 times prevents
   retaining the stronger result. An inconclusive check cannot certify
   the stronger result's accuracy.
7. Preserve baseline nonzero entries, add accepted estimates, and separately
   record search witnesses that did not produce an accepted estimate.

The baseline work is its shared `WorkSpent` plus the initial 20,000 MC
seasons' work. Each simulated season is charged `unplayed fixtures + teams`
work units, as in the existing estimator. Additional work is capped at nine
baseline budgets. All 15 requests with zeros consumed effectively the full
10× total budget; integer rounding leaves a few draws unused. Depending on
input and baseline search work, each unresolved cell received approximately
199,000–650,000 final draws, plus its pilots.

This tests **a larger budget directed at remaining zeros**, not ten times
every loop of the existing algorithm. It is an offline extension and does
not change service behavior or production defaults. It appends rare-cell
estimates without rerunning matrix reconciliation; the largest added entry
is only `1.07e-10`, so the resulting marginal changes are tiny.

The work model counts simulated fixtures and sorting work. It is not a CPU
instruction count and does not price proofs, preprocessing, or proposal
construction precisely. Therefore 10× modeled work is not 10× elapsed time.

## Paired full-request results

| Input | Baseline nonzero cells, median (range) | Additional estimates per run, median (range) | Extended nonzero cells, median (range) | Distinct cells gaining an estimate across seeds | Absent in every baseline seed |
| --- | ---: | ---: | ---: | ---: | ---: |
| 16498 | 362 (359–364) | 6 (5–8) | 368 (365–369) | 12 | 5 |
| 16653, current / 85 unplayed | 338 (335–338) | 9 (8–12) | 347 (346–348) | 13 | 8 |
| 16653, earlier / 90 unplayed | 345 (344–349) | 8 (5–11) | 354 (352–355) | 12 | 6 |
| 16982 | 400 | 0 | 400 | 0 | 0 |
| 16983 | 400 | 0 | 400 | 0 | 0 |

Medians are computed independently by column. A distinct cell gain means
it changed from zero within at least one paired run; it does not mean that
cell was missing from all baseline runs.

| Input | Median baseline time | Median extra time | Median total time | Median paired elapsed-time factor | Median non-impossible zeros remaining |
| --- | ---: | ---: | ---: | ---: | ---: |
| 16498 | 1.257 s | 5.305 s | 6.614 s | 5.27× | 18 |
| 16653, current | 1.028 s | 4.397 s | 5.412 s | 5.29× | 5 |
| 16653, earlier | 1.060 s | 4.089 s | 5.110 s | 4.83× | 3 |
| 16982 | 0.892 s | 0.003 s | 0.894 s | about 1× | 0 |
| 16983 | 0.886 s | 0.003 s | 0.889 s | about 1× | 0 |

The last two inputs used approximately 1× work because there were no zeros
to search. The full 25-run experiment took 95 seconds. Timing is a single
measurement per input/seed on this host, without concurrent experiment jobs.

For seed 808, the remaining non-impossible zeros break down as follows:

| Input | Undecided | Reachable by construction, no estimate | New sampling witness, no accepted estimate | Proved impossible zeros |
| --- | ---: | ---: | ---: | ---: |
| 16498 | 15 | 1 | 1 | 14 |
| 16653, current | 1 | 2 | 2 | 48 |
| 16653, earlier | 1 | 3 | 1 | 43 |

Extra simulation does not itself prove impossibility. A witness also does
not automatically meet the weighted probability estimator's acceptance gate.

## Probability magnitudes and examples

Across the baseline cells already below `1e-6`, median nonzero probabilities
were `3.28e-9`, `3.07e-9`, and `1.68e-9` respectively for 16498, current
16653, and earlier 16653. Median added probabilities are roughly five to
six orders of magnitude smaller. These are distribution comparisons, not
claims that every newly found cell is smaller than every existing cell.

| Input / team / position | Accepted added probability range | Paired or nearby existing estimate |
| --- | ---: | --- |
| 16498 / Athletico-PR (5) / 17 | `1.11e-11`–`2.76e-11` | 16th was `3.12e-9` in seed 801 |
| 16498 / Cruzeiro (15) / 19 | `1.76e-12`–`1.92e-12` | Other baseline seeds already gave `1.03e-12`–`1.88e-12` |
| 16498 / Chapecoense (318) / 4 | `2.70e-19`–`3.79e-19` | 6th was `4.49e-13` in seed 801; newly found 5th was about `2.5e-15` |
| 16498 / Chapecoense (318) / 3 | `4.11e-24` in one run | The independent check gave `5.23e-24` |
| Current 16653 / Fortaleza (22) / 17 | `3.34e-12`–`3.44e-12` | 16th was `2.52e-9` in seed 808 |
| Current 16653 / Fortaleza (22) / 18 | `2.21e-18`–`2.42e-18` | Newly estimated 17th is about a million times larger |
| Current 16653 / Londrina (95) / 4 | `6.91e-23`–`1.25e-22` | New 5th about `9.5e-17`; new 6th about `8.7e-13` |
| Current 16653 / América-MG (125) / 12 | `5.73e-14`–`6.35e-14` | 13th estimates are around `1.0e-10` |
| Current 16653 / Novorizontino (2064) / 18 | `7.27e-20`–`1.45e-19` | New 17th is about `6.9e-13` |
| Earlier 16653 / Londrina (95) / 2 | `4.90e-20`–`7.27e-20` | Newly found 3rd is `3.73e-15`–`4.58e-15` |

Team names were read from the local database with `mysql -u root`.

## Independent proposal checks

Only 20 of the 118 accepted additions also had a conclusive gentle check
inside the tenfold experiment. Most of the remaining additions used stronger
proposals. Stable repetition across seeds is useful evidence, but can miss
systematic proposal problems.

To investigate that risk, six selected cells received three independent
300,000-draw checks at rank tilts 4, 6, and 10, with point-tilt magnitudes
0.5, 0.75, and 1. These use seed 1211 with separately derived streams.
These **5.4 million diagnostic draws are outside the primary 10× budget**
and took another 5.34 seconds. They did not alter the coverage matrix.

| Cell | Tilt 4 probability | Tilt 6 probability | Tilt 10 probability |
| --- | ---: | ---: | ---: |
| 16498 / Chapecoense / 3 | `7.17e-24` (fails gate) | `5.05e-24` (fails gate) | **`5.23e-24`** |
| 16498 / Chapecoense / 4 | `7.09e-19` (fails gate) | **`3.71e-19`** | `3.92e-19` (fails gate) |
| Current 16653 / Fortaleza / 17 | **`3.29e-12`** | **`3.40e-12`** | **`3.18e-12`** |
| Current 16653 / Fortaleza / 18 | `1.52e-18` (fails gate) | **`2.20e-18`** | **`2.19e-18`** |
| Current 16653 / Londrina / 4 | `2.74e-22` (fails gate) | **`1.22e-22`** | `1.63e-22` (fails gate) |
| Current 16653 / Novorizontino / 18 | `0` (fails gate) | `5.63e-20` (fails gate) | **`1.19e-19`** |

Bold entries pass the production validity gate. All six have an accepted
independent result at the original order of magnitude. Results failing the
gate are diagnostic values, not accepted estimates.

There was also one useful warning within the primary experiment: earlier
16653 / Londrina / 2 in seed 911 had a selected-proposal result of
`2.43e-22`, but its gentle result was `5.01e-20`, roughly 206 times larger.
The gentle result passed the gate and replaced the selected result. This
supports retaining verification budget even when precision is not the goal.

## What this teaches us

1. **The current budget misses genuine tail events.** Several cells that
   were zero in every baseline run gain consistent estimates under extra
   work. Current Fortaleza / 17 and / 18 each gain estimates in all five
   runs, with narrow ranges and independent proposal agreement.
2. **An adjacent position can be dramatically rarer.** The millionfold drop
   between Fortaleza's 17th and 18th, and larger drops for other teams,
   argue against copying/interpolating a neighbor's probability. Neighbor
   information remains useful for candidate selection and witness reuse.
3. **Proposal strength matters as much as raw sample count.** In the checks,
   tilt 4 often failed to get enough useful samples, while tilt 6 or 10
   succeeded. Stronger was not uniformly better: Londrina / 4 passed at
   tilt 6 but failed at 10. Proposal diversity plus independent confirmation
   is a better tuning target than always increasing the strongest tilt.
4. **There is still a difficult tail after 10× work.** The median remaining
   non-impossible zeros are 18, 5, and 3. Some are proved reachable but lack
   an accepted probability estimate; others remain undecided. More sampling
   alone does not close every cell.
5. **Extra budget has uneven value across groups.** There is no coverage
   gain for the two early-season reference groups that already have all
   cells nonzero. A future larger-budget mode should stop when no useful
   zero targets remain.

Recommendation: keep this as an offline or optional deeper pass. The observed
5–7 second total runtime does not meet the previous production latency goal.
The most useful next production experiment is selective allocation to cells
with successful pilots and competitive weighted ESS, together with an
intermediate-strength independent check. The present experiment did not
measure a 20% overhead version, so it does not establish how much of this
coverage can be retained under that constraint.

## Inputs and reproduction

| Input | SHA256 |
| --- | --- |
| 16498 | `44eabb47ee55477c77025c3de4e839c6ea913b8569dd29694d7178bdb8bcdc89` |
| 16653, current | `71d4fea8502c3086b8787c764a94b5f8a5caa6549300f2a7687ead8897e11aec` |
| 16653, earlier | `2d1c1d6f70a64d5b86f5129b32cbf77bedc4c42e464cd849011ca0b3018b3d87` |
| 16982 | `9327edcdfcaa5cdc0f975771e9150b2436750231ea4b417a279fb372564e2aeb` |
| 16983 | `43969b02fe7138b991b9d74215223efbc9fc16bc8f4ebe6d0a5e274126f8af36` |

Run from the repository root (saved request files are local, not committed):

```sh
GOCACHE=/private/tmp/golaberto-go-cache GOMAXPROCS=4 \
RARE_POSITION_BUDGET_EXPERIMENT_REQUESTS=/private/tmp/golaberto-group-16498-current.json,/private/tmp/group16653-live-20260927.json,/private/tmp/golaberto-real-fixtures/group-16653.json,/private/tmp/golaberto-group-16982-current.json,/private/tmp/golaberto-real-fixtures/group-16983.json \
RARE_POSITION_BUDGET_EXPERIMENT_OUTPUT="$PWD/experiments/rare_positions/local/2026-09-29-tenfold-work" \
go test ./go -run '^TestRarePositionTenfoldWorkExperiment$' -count=1 -v -timeout 20m
```

For the separate diagnostic checks, use only the 16498 and current 16653
request paths above, set
`RARE_POSITION_BUDGET_CHECK_CELLS=16498:318:3,16498:318:4,16653:22:17,16653:22:18,16653:95:4,16653:2064:18`,
and run `TestRarePositionBudgetProposalCheck` instead. Both optional tests
skip when their required environment variables are absent. The coverage
test asserts the work cap and preservation of existing nonzero estimates.
An additional current-16653 seed-808 rerun reproduced coverage, reachability,
and hit counts; probabilities agreed within `1e-12` relative tolerance.
Floating-point accumulation order produces tiny differences between runs.

Raw per-run outputs remain in ignored
`experiments/rare_positions/local/2026-09-29-tenfold-work/`. The committed
test harness and this report contain no request exports. `go test ./go`
passes with the experiment environment unset.
