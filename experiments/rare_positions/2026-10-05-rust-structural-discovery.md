# R68: structural discovery and rival-set diversity

October 5, 2026. Paired evaluation complete. All eight current variants are
rejected for adoption; production defaults are unchanged.

## Purpose and controls

R67 found systematic concentration on incomplete outcome families. A positive
estimate can be substantially wrong, and independent confirmations can share a
proposal blind spot. Its extended estimates remain provisional references,
not exact probabilities or certified bounds.

R68 compares three structural tree priorities (interval, necessary-rank and
strict-side), three partial-family controls (wildcard ties, above/below
skeletons and release of one flexible strict constraint), and two combinations.
All controls are default off. Structural scores only select proposal work;
they do not change event definitions, importance weights or reachability.
Partial-family densities include every matching overlapping component and
retain positive native support.

Requests run serially with four workers. The same frozen candidate executable
with controls unset is the matched control. A separately frozen production
executable checks default parity. The protocol, runner, analyzer, source
archives, binary hashes and raw records are under
`2026-10-05-structural-discovery/`. Compilation finishes before measured runs.
Request capacity, acceptance gates, final floors and pilot counts remain fixed.

## Initial screen and corrections

The v4 screen completed all 60 requests: both retained group-16498 fixtures,
seeds 808, 1669 and 1993, eight experimental arms, matched control and frozen
production. All six default-control comparisons matched the frozen export
after excluding the diagnostic `work_spent` field. No reachability changes
were observed.

Counts below are cell/request comparisons against matched control, not unique
cells or estimates of accuracy.

| v4 arm | Zero to positive | Positive to zero | Changed positive | Budget failures |
| --- | ---: | ---: | ---: | --- |
| Tree interval | 1 | 4 | 3 | None |
| Tree rank | 1 | 4 | 3 | None |
| Tree strict | 1 | 4 | 3 | None |
| Family ties | 1 | 2 | 4 | None |
| Family skeleton | 1 | 2 | 3 | One fallback |
| Family relax | 1 | 3 | 2 | One message stage |
| Rank + skeleton | 1 | 8 | 3 | Same skeleton fallback |
| Strict + relax | 1 | 6 | 5 | None |

None recovered the missing diagnostic estimates or the larger provisional
Palmeiras/15th result. The three tree arms produced identical exports despite
different structural rankings: their first retune failed and consumed the only
attempt slot. The logs did not record the rejection reason.

Two concrete budget failures occurred on updated fixture / seed 1993:

- Skeleton's Flamengo/12th fallback charged 79,684,675 operations against a
  78,188,903 grant. Validation averaged about 13,691 operations/draw, while the
  final pair averaged about 15,182, exceeding its 8% headroom. The same failure
  occurred in rank + skeleton. The settlement correctly withheld publication.
- Relax's message stage charged 13,490,857 against a 9,990,015 grant. A family
  experiment changed the remaining allowance but still selected legacy
  retuning because no tree experiment was set.

The corrected frozen v5 revision sizes partial-family reserves and finals from a
conservative operation bound, activates bounded retuning for either valid
experimental control, and tries further ranked tree candidates within the
existing allowance until the configured successful message count is reached.
It reserves the first slot for structurally promising zero-hit candidates and
records actual per-attempt rejection reasons. Full-family default scheduling
and legacy retuning remain on their historical paths.

Earlier default-parity smokes caught two regressions before v4: loss of the
historical full-family top-four truncation, and changed default retuning. Both
were corrected, and those earlier artifacts remain available for audit.

The 60 v4 requests consumed 100.257 seconds of summed process wall time and
214.947 CPU seconds. These single cold observations are screening measurements,
not a latency-neutrality claim. Compilation and earlier smokes are excluded.

## Final evaluation

The corrected v5 binary completed 240 full requests, with eight experimental
arms plus control and frozen production in every cohort:

| Matrix | Fixtures | Seeds | Requests | Summed wall seconds | CPU seconds |
| --- | --- | --- | ---: | ---: | ---: |
| Screen | Original and updated 16498 | 808, 1669, 1993 | 60 | 97.767 | 210.890 |
| Reserved seeds | Original and updated 16498 | 4211, 4229, 4241 | 60 | 102.300 | 235.505 |
| References | 15902, 16413, current 16653, 16982, 16983, historical 16653 | 808, 1993 | 120 | 59.858 | 124.405 |
| **v5 total** | | | **240** | **259.926** | **570.799** |

All three analyses verified their complete manifests and recorded hashes.
All **24** default-control comparisons matched frozen production after
excluding `work_spent`. All **192** experimental requests had no recorded grant
overrun or failed within-grant settlement. No reachability changed. The 120
reference requests had no estimate changes between any arm and matched control.

The variants still failed the quality screen and its independent-seed check:

| v5 arm | Screen gains / losses | Reserved-seed gains / losses | Changed positives, total | Reference gains / losses |
| --- | ---: | ---: | ---: | ---: |
| Tree interval | 1 / 4 | 0 / 5 | 4 | 0 / 0 |
| Tree rank | 1 / 4 | 0 / 5 | 4 | 0 / 0 |
| Tree strict | 1 / 4 | 0 / 5 | 4 | 0 / 0 |
| Family ties | 0 / 2 | 2 / 2 | 8 | 0 / 0 |
| Family skeleton | 0 / 2 | 2 / 2 | 8 | 0 / 0 |
| Family relax | 0 / 2 | 2 / 2 | 8 | 0 / 0 |
| Rank + skeleton | 0 / 6 | 1 / 5 | 8 | 0 / 0 |
| Strict + relax | 0 / 6 | 1 / 5 | 8 | 0 / 0 |

Each of the three tree modes produced identical exports excluding `work_spent`;
the three family modes also matched each other, as did the two combinations.
The equal outputs do not establish equal score quality: no inspected tree
retune reached a refined proposal on the difficult retained cases.

### Diagnostic estimates

The following counts include all twelve retained-fixture/seed cohorts.
All positive Palmeiras/16th values remained identical to control. None of the
eight arms recovered a diagnostic zero.

| Arm group | Positive Flamengo/13th | Positive Palmeiras/15th | Positive Palmeiras/16th |
| --- | ---: | ---: | ---: |
| Control | 6 / 12 | 2 / 12 | 10 / 12 |
| Any tree mode | 3 / 12 | 1 / 12 | 10 / 12 |
| Any family mode | 6 / 12 | 0 / 12 | 10 / 12 |
| Either combination | 3 / 12 | 0 / 12 | 10 / 12 |

Selected fractions illustrate that nonzero coverage alone is insufficient:

| Fixture / seed / cell | Control | Tree rank | Family skeleton | R67 provisional mean |
| --- | ---: | ---: | ---: | ---: |
| Original / 808 / Flamengo13 | 1.173e-33 | 3.843e-34 | 1.173e-33 | 7.56e-34 |
| Original / 1669 / Flamengo13 | 8.155e-34 | 2.512e-34 | 8.155e-34 | 7.56e-34 |
| Original / 1993 / Flamengo13 | 7.424e-34 | 0 | 7.424e-34 | 7.56e-34 |
| Original / 808 / Palmeiras15 | 1.000e-33 | 1.000e-33 | 0 | 4.86e-31 |
| Updated / 4241 / Palmeiras15 | 2.732e-34 | 0 | 0 | 5.43e-31 |
| Original / 808 / Palmeiras16 | 1.683e-40 | 1.683e-40 | 1.683e-40 | 3.01e-40 |

The tree mode retains the old original/808 Palmeiras15 underestimate,
approximately 486 times below the provisional reference. It loses the updated
fresh-seed positive result, already approximately 1,988 times below its rough
reference. The family modes lose both positive Palmeiras15 estimates.
These ratios describe agreement with R67; they are not certified accuracy
measurements. Zero remains an unobserved estimate, not an impossibility proof.

### Why the current implementation fails

Representative original/808 tree-rank diagnostics spend 19,999,775 scheduling
units on fourteen unseen attempts: thirteen `guide_bound` failures, followed
by `work_allowance`, with no selected proposal. Updated/1993 interval similarly
spends 20 million units on fifteen unsuccessful unseen attempts. Failed
proposal-row and joint setup work is charged; guide construction is refused
before execution. Retrying fixes the one-attempt defect but consumes work that
otherwise supported later sampling.

The guide bound applies the league-wide sum of possible gains as every team's
span. It is conservative and substantially looser than the actual per-team
guide spans. Therefore this result rejects the current realization and work
allocation; it does not establish that the three structural scores cannot
identify useful unseen families.

For partial-family fallbacks, the conservative native-plus-four-component
confirmation reserve cannot fit the current grants on the affected failed
cells. Updated/1993 skeleton and relax now decline with zero family fitting or
final work rather than executing an underfunded pair. This fixes the overrun
but supplies no evidence that broader rival families improve those estimates
at the current capacity. Positive native support alone does not make that
proposal affordable.

The next investigation should obtain a tighter, auditable proposal-specific
guide/density cost bound and show useful refinements completing inside the
existing grants before comparing structural scores or rival diversity again.
Adding sampling or relaxing acceptance would change the experiment's capacity
contract. No such change was made here.

### Timing and decision

The table reports mean paired changes and the largest observed increase over
the twelve retained-fixture cohorts. Every raw observation, including reference
timing, is retained. These are cold screening measurements with one repeat;
no latency-neutrality or speedup claim follows from them.

| Arm | Mean wall delta, s | Mean CPU delta, s | Largest wall increase, s | Largest CPU increase, s |
| --- | ---: | ---: | ---: | ---: |
| Tree interval | +0.0285 | +0.0004 | +0.1480 | +0.1848 |
| Tree rank | +0.0078 | +0.0073 | +0.1347 | +0.2195 |
| Tree strict | +0.0312 | +0.0487 | +0.3472 | +0.3963 |
| Family ties | -0.1293 | -0.0957 | +0.0898 | +0.2015 |
| Family skeleton | -0.1181 | -0.0807 | +0.1092 | +0.2154 |
| Family relax | -0.1368 | -0.1122 | +0.0469 | +0.1144 |
| Rank + skeleton | -0.0449 | -0.0727 | +0.1067 | +0.1354 |
| Strict + relax | -0.0618 | -0.0830 | +0.1260 | +0.1311 |

All eight variants are rejected for adoption because of losses on screening
and independent seeds and failure to recover the diagnostic estimates.
The protocol's additional warmed eight-pair timing gate applies only to an
adoption candidate; none reached it. Safety and default parity pass after the
corrections. Defaults remain unchanged; nothing was committed or pushed.

Together, the retained v4 and v5 matrices contain **300 requests**, consuming
**360.183 seconds** of summed process wall time and **785.746 CPU seconds**.
This excludes compilation, tests, analysis, pauses and earlier parity smokes.
Modeled work is a scheduling measure; nested stage counters are not additive
totals and should not be summed across duplicated diagnostics.

## Validation and reproduction

The independent source review cleared the corrected budget, retry and default
contracts. `cargo check` passed, and the final
`cargo test --offline --locked --lib` passed **171 tests** with no failures.
Four focused tests also passed, one test each:

- `family_pair_floor_uses_conservative_bound_only_for_partial_structures`
- `bounded_candidate_retry_fills_success_count_without_overspending`
- `prepared_messages_spend_only_reserved_work_and_report_partial_attempts`
- `legacy_retuner_matches_the_grant_bounded_retuner_with_ample_budget`

The prepared-message test exercises allowances of one and zero and verifies
that setup charging stays inside the allowance. The final targeted test build
compiled the updated ordering code after `cargo check`. Release compilation
used `cargo build --offline --locked --release -j1 --bin golaberto-odds`.
`git diff --check` passed for the touched files. Formatting checks found
differences throughout the already-dirty tree; no broad formatting changes
were applied.

Run from the repository root. The frozen production SHA-256 is
`ae0e68a5d5986436656ca34aa6b6b6a9f54befeaaf96049dfbc0df96ab6bbdf1`.
The candidate, source archive and runner/analyzer hashes are in `data/v5.sha256`.

```sh
python3 experiments/rare_positions/2026-10-05-structural-discovery/scripts/run_matrix.py \
  --candidate-binary experiments/rare_positions/2026-10-05-structural-discovery/data/golaberto-odds-r68-v5 \
  --frozen-production-binary experiments/rare_positions/2026-10-04-early-rank/data/golaberto-odds-tree-production-v2 \
  --frozen-production-sha256 ae0e68a5d5986436656ca34aa6b6b6a9f54befeaaf96049dfbc0df96ab6bbdf1 \
  --out-dir /tmp/r68-screen-reproduction \
  --fixtures original-16498,updated-16498 --seeds 808,1669,1993 \
  --arms control,frozen,tree-interval,tree-rank,tree-strict,family-ties,family-skeleton,family-relax,combined-rank-skeleton,combined-strict-relax \
  --repeats 1 --warmups 0
python3 experiments/rare_positions/2026-10-05-structural-discovery/scripts/analyze_matrix.py \
  --run-dir /tmp/r68-screen-reproduction
```

For the reserved-seed run, use seeds `4211,4229,4241` and a fresh output
directory. For the reference run, use fixtures
`ref-15902,ref-16413,ref-16653-current,ref-16982,ref-16983,historical-16653`,
seeds `808,1993`, and another fresh directory. All other arguments are identical.
`--resume` reuses completed records only after checking the identical manifest
and all recorded hashes. Raw logs are authoritative; analysis verifies those
hashes and matrix completeness before comparisons.
