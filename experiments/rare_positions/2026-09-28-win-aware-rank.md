# Win-aware rank screening and lookahead, 2026-09-28

## Question

For phases ordered by points, then wins, can using wins alongside points in the
conditioned probability search find more rare finishing positions or reduce
request time?

## Candidate

`RARE_POSITION_WIN_AWARE_RANK=screen` encodes `(points, wins)` as one ordered
value while screening completed W/D/L samples. It only samples scorelines when
the target's rank still depends on a tie in both points and wins.

`RARE_POSITION_WIN_AWARE_RANK=lookahead` also uses the encoded value in the
remaining-fixture marginal suffix distributions and in the importance proposal.
The usual likelihood ratio still corrects for the proposal, so this does not
change the target probability. `RARE_POSITION_WIN_AWARE_RANK=1` enables both.
The experiment is opt-in; the default estimator remains unchanged.

The encoding is used only when the standings sort starts with `pt,w`. For other
sort orders, a win difference cannot safely decide a rank before the preceding
tiebreakers. The point-conditioned event, matched point pool, and hard point
bounds are still point-based. Changing those requires a joint point/win mass
calculation; simply discarding samples would invalidate the estimates or upper
bounds.

## Fixed-seed full-request results

Four cores, production-sized request with matched point pool and conditioned
search enabled. The saved group 16653 request had 85 unplayed games (SHA-256
`b87207393a283ae4b7cc986c49a60fdd0943057295872ddcda27926d9d6597dd`);
the group 16498 request had 103 unplayed games (SHA-256
`b37c82f317d5f0e412b2642101929cca778857c2792dc89c0fbb830e702e701e`).
Both phases sort `pt,w,...`. Each row compares the same seeds, 808–812, with
the default estimator. Times are the mean of one baseline and one candidate
request per seed on the local machine; they are indicative, not a stable
benchmark.

| Group | Mode | Newly nonzero cell-runs | Lost nonzero cell-runs | Mean baseline | Mean candidate |
| --- | --- | ---: | ---: | ---: | ---: |
| 16653 | screen + lookahead | 0 | 2 | 1.236 s | 1.207 s |
| 16498 | screen + lookahead | 7 | 0 | 1.318 s | 1.377 s |

The newly nonzero values for group 16498 were all approximately `2e-18` to
`4e-12`. The two lost values for group 16653 were approximately `3e-19` and
`1e-12`. These are below the resolution of the existing 5-million-season
reference, so this experiment cannot establish which proposal is more accurate
for those cells. It found no consistent gain near the `1e-6` probability level.

### Why the two group 16653 cells disappeared

At seed 811, Avaí (team 68) in 1st place had a baseline estimate of
`3.361e-19` from 9,088 weighted hits in 20,000 guided draws (ESS 706). The
win-aware full request left that cell at zero after its initial 1,000 draws.
The isolated win-aware pilot gave 108 hits (versus 90 with points alone),
then 9,149 confirmation hits, estimate `3.335e-19`, and ESS 991. This
initially suggested a budget-selection effect. A subsequent full-request trace
with the original point-based pilot retained for selection showed Avaí was
selected for confirmation, but the win-aware confirmation failed the existing
batch-consistency gate: its two-half estimate gap was 0.2125 against the 0.2
limit. The point-based confirmation passed at 0.0594. A point-based fallback
restored Avaí's estimate. The original proposal's candidate ordering may
also have changed, but the earlier displacement claim was not established.

At seed 812, team 77 in 19th place had one hit in the baseline's 1,000 direct
conditioned draws, giving `1.349e-12` with nearly 100% relative standard error.
The win-aware fast screen used the same seed but consumed fewer scoreline random
draws, so subsequent fixture outcomes differed and it saw zero hits. Its
reported 95% upper bound was `4.035e-12`; zero here is no proof of
unreachability. Matrix reconciliation did not remove either positive result.

Separating the two changes on seeds 808, 811, and 812 showed that the screen
alone gave group 16498 no additional cells and was about 3% faster there. The
lookahead proposal found five additional group 16498 cell-runs but lost three
and gained one in group 16653. Group 16982 sorts `pt,gd,...`, so the candidate
left its result unchanged in the tested seed.

## Decision

Keep this as an opt-in experiment. The proposal changes which very rare cells
are seen, with a measurable cost in group 16498, but does not reliably improve
coverage across groups under the current shared search budget. A next test
should preserve previously selected guided candidates while using the
win-aware proposal for confirmation, then measure the combined coverage and
cost. The screen is a small speed opportunity; it should be benchmarked
separately with more repetitions before enabling it by default.

Correctness check: the win-aware fast and importance samplers agreed with full
scoreline sampling on a small `pt,w,...` group, and `go test ./...` passed.

Reproduce a paired full request with:

```bash
RARE_POSITION_BENCHMARK_GROUP_JSON=/path/to/group.json \
RARE_POSITION_RANDOM_SEED=808 GOMAXPROCS=4 \
go test ./... -run '^TestWinAwareRankFullRequestExperiment$' -count=1 -v
```
