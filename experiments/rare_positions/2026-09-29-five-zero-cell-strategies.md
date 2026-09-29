# Five more zero-cell search experiments (2026-09-29)

## Objective and setup

Test five changes to the rare-position search with one objective: turn more reachable or undecided zero cells into independently confirmed nonzero estimates. This follows [the heuristic time-allocation profile](2026-09-29-heuristic-time-allocation.md). The previous coverage-first reference skips the 200,000-draw extra zero-hit extension, pilots up to 24 cells, and permits up to five proved-cell and seven undecided-cell confirmations. It found 41 additional nonzero **cell-runs** and no losses versus current production in 15 paired requests, but weakened upper bounds for some remaining zeros. It was an experimental reference, not a production setting.

The five experiments below changed one search dimension at a time relative to that reference. Each ran a complete `calculate_odds` request on the saved current group 16653 request (85 unplayed fixtures), group 16498 request (103), and earlier group 16653 request (90), with seeds 808, 811, 817, 900, and 911. `GOMAXPROCS=4`, 20,000 initial Monte Carlo seasons, and the 100,000-season matched pool were held constant. Each variant used the same seed and fixture as its reference. Times are in-process full-request medians of the five seeds for that input; process startup is excluded. The scorer's existing fresh confirmation and validity gates were unchanged. Temporary experimental flags, timers, and the probe test were removed after measurement.

Fixture SHA-256 hashes for identifying the snapshots:

- 16653 current: `b87207393a283ae4b7cc986c49a60fdd0943057295872ddcda27926d9d6597dd`
- 16498: `44eabb47ee55477c77025c3de4e839c6ea913b8569dd29694d7178bdb8bcdc89`
- 16653 earlier: `2d1c1d6f70a64d5b86f5129b32cbf77bedc4c42e464cd849011ca0b3018b3d87`

The coverage-first reference's nonzero counts over the five seeds were 339/341/340/340/340 for current 16653, 364/361/361/362/363 for 16498, and 347/348/347/348/348 for earlier 16653. Its median times were 818, 838, and 743 ms, respectively.

## Five experiments

`+` and `−` below count cells changing zero→nonzero and nonzero→zero across all 15 paired requests, relative to the coverage-first reference. These are **cell-runs**, so a cell appearing under multiple seeds is counted multiple times. A gain does not imply known accuracy against an exhaustive reference probability.

| Strategy | Change | + / − cell-runs | Median times: 16653 current / 16498 / 16653 earlier | Recommendation |
| --- | --- | ---: | --- | --- |
| 1. More pilots | 24 → 32 cells, 1,000 draws per proposal | +2 / −2 | 801 / 878 / 763 ms | **Do not adopt.** One 16498 cell was gained, but two current-16653 cells were displaced in another run; no net coverage gain. |
| 2. Wider, shallower pilots | Up to 48 cells, 500 instead of 1,000 draws per proposal | +7 / −10 | 747 / 819 / 715 ms | **Do not adopt.** The saved time came with a net loss of three nonzero cell-runs. A thin pilot missed useful proposal evidence. |
| 3. Team round robin | Pilot one cell per team before taking second cells for the same team | +2 / −2 | 771 / 841 / 757 ms | **Do not adopt.** It diversified teams but exchanged two previously estimated cells for two others, with no net gain. |
| 4. Stronger proposal tilts | Try `(8,1)`, `(12,1)`, `(16,1.5)` instead of `(3,0.5)`, `(8,1)`, `(12,1)` | +8 / −10 | 897 / 825 / 918 ms | **Do not adopt.** The aggressive proposals found a few new very rare finishes but lost more existing estimates, particularly on earlier 16653, and cost more. |
| 5. Flexible confirmations | Select the best 12 pilot candidates by effective sample size, rather than reserving five proved and seven undecided slots | +6 / −1 | 821 / 848 / 773 ms | **Do not adopt unchanged.** It increased net coverage, but lost the earlier-16653 team 68 / 1st estimate in one run. Keep minimum category quotas. |

The first four strategies did not improve net zero coverage robustly. More pilots can also change which proposal receives an existing confirmation slot, so a gain and a loss may appear even when total confirmation work is similar. These results explain why the earlier 24-cell pilot cap was a useful allocation on these inputs.

## Safer refinement of strategy 5

I tested a quota-preserving spillover: fill the existing proved and undecided confirmation quotas first, then give any *unused* slots to the best remaining pilot candidates. This keeps the original selected candidates while allowing useful work when one category has fewer candidates. Relative to the coverage-first reference, it gained three cell-runs and lost none across the original 15 requests. The extra estimates were current-16653 team 95 / 5th under two seeds and earlier-16653 team 68 / 1st under one seed. Their fresh confirmations passed the existing precision gates; for example, team 95 / 5th had 201–215 conditional hits and effective sample size 52–64.

Because coverage-first weakens some upper bounds, I also ran the confirmation variants with the **unchanged production zero-hit extension**. I used 15 seeds (801–812, 817, 900, 911) on each of the three saved inputs: 45 paired full requests per variant.

| Production allocation | Gained / lost cell-runs vs production | Distinct gained cells | Median paired wall-time change | Remaining-zero upper bounds |
| --- | ---: | ---: | ---: | --- |
| Unrestricted flexible confirmations | +14 / −1 | 8 | +1.4% | Materially unchanged |
| Quota-preserving spillover | **+8 / −0** | 6 | **+1.0%** | Materially unchanged |

The unrestricted version lost earlier-16653 team 95 / 4th at seed 809: production estimated `1.12e-11`, while the variant left it zero. Spillover preserved it. Spillover's paired 90th-percentile time increase was 3.5%; it adds only confirmations for candidates already piloted. The input-specific median full-request times were production → spillover: 916 → 922 ms for current 16653, 968 → 969 ms for 16498, and 943 → 941 ms for earlier 16653. Some requests remain above one second under both allocations, so the one-second goal is not a hard guarantee on this machine.

As a cost cross-check, keeping production's full upper-bound extension while also using the entire coverage-first 24-pilot/12-confirmation allocation gained 41 and lost one cell-run over the original 15 requests. Its overall median was about 1,016 ms, and nine of 15 requests exceeded one second. This is not a good way to preserve both full bound search and wider pilots under the current time target.

## Recommendation

Adopt **only quota-preserving confirmation spillover** as a small production allocation change, after adding a targeted regression test for the selector. It added eight nonzero cell-runs with no losses across 45 paired production requests, retained the existing upper-bound work, and cost about 1% median wall time. Do not adopt the other four strategies or unrestricted flexible selection based on these results. The coverage-first reallocation remains a separate decision because its stronger coverage comes with weaker upper bounds for some cells still at zero.

These experiments measure observed coverage and service cost on saved requests. They do not establish the true probabilities of the new e-18 to e-11 estimates; the existing fresh importance-sampling acceptance checks are the evidence supporting those estimates. More seasons or an independent reference would be needed for calibration at that scale.
