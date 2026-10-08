# Separate measurements of the two recovery changes

This experiment measures each proposed change against an actual clean committed build. The baseline is commit `4200a3e8150cffec8003f579be7c7f1e206e64b3`, compiled from an isolated `git archive` checkout. This identifies a committed source baseline; it does not assert which binary is deployed on the service host.

The prior experiment used the current working-tree binary in every arm and disabled cohorts with a runtime flag in its no-cohort controls. Its four-way comparison was useful for interaction, but the headline baseline included the uncommitted cohort implementation. This experiment removes that ambiguity by using separate source builds.

## Source and feature separation

| Arm | Compiled source | Expected cohorts | Tree minimum |
| --- | --- | --- | ---: |
| Clean baseline | Committed HEAD | Absent from source | 65 |
| Cohorts only | Current working tree | Enabled | 65 |
| Smaller trees only | Committed HEAD | Absent from source | 1 |
| Both | Current working tree | Enabled | 1 |

The experimental tree-minimum flag and overflow-first ordering already exist in committed HEAD, so the smaller-tree arm needs no cohort source. The working-tree build contains the uncommitted cohort implementation and its supporting sampler/budget plumbing. No changes to application source, production defaults, or existing uncommitted work are made for this experiment.

Both individual-change arms are compared directly against the clean baseline. The combined arm is a secondary comparison that shows whether the changes duplicate work or complement each other. Report the marginal effect of adding cohorts after smaller trees, and of adding smaller trees after cohorts, separately from their standalone effects.

## Measurement design

Use the same 12 fixtures and seeds as the immediately preceding experiment: the 11 established fixtures, plus the motivating latest95 fixture; seeds 808, 13001, 13003, 13007, and 13009. Each of four arms uses four workers and two measured repetitions: 480 measured requests, plus four discarded warmups. Requests run serially with rotated arm order and reversed order for the second repetition. Builds and heavy checks finish before timing or run afterward.

The runner chooses a recorded binary for each arm, clears inherited `RUST_ODDS_` and `RARE_POSITION_` overrides, and records wall time and child user plus system CPU. Each fixture/arm timing is the mean across seeds of its two-repeat median. Corpus totals sum per-fixture repeat medians within each seed, then average across seeds. The 11 established fixtures and latest95 are reported separately, with an additional all-12 total.

Cell coverage counts are deduplicated by fixture, seed, team, and position, rather than counting timing repetitions twice. Both gains and losses are evaluated against the clean baseline. Check logged settings, repeated exports, input hashes, valid probabilities, impossible-cell contradictions, estimator changes, and disjoint tree/cohort work ledgers. The clean source has no expected-cohort log event; its absence is expected rather than a failed flag check.

The existing limitations remain relevant: extreme-tail estimates have no exact oracle for latest95; a prior Palmeiras 15th family estimate differs substantially from a tree estimate in another seed; and latest95 seed 13009 previously exceeded the residual grant during root counting and safely stopped. Record these results rather than treating positive coverage or repeat stability as accuracy guarantees.

## Results

All 480 measured requests and four discarded warmups completed successfully. Each row below compares directly with the **clean committed baseline**, rather than with the uncommitted cohort implementation.

| Change applied alone or together | CPU change, all 12 fixtures | Wall change, all 12 fixtures | Added positive cell/seed outcomes | Lost positive outcomes |
| --- | ---: | ---: | ---: | ---: |
| Expected cohorts only | +9.45% | +14.60% | 22 | 0 |
| Smaller-tree eligibility only | +1.06% | +2.18% | 7 | 0 |
| Both changes | +9.55% | +14.89% | 24 | 0 |

The clean baseline averages 1.473 CPU seconds and 0.656 wall seconds per request with equal fixture weight. These percentages divide each arm's aggregate timing by the baseline aggregate, rather than averaging per-seed percentages. Cell/seed outcomes count each fixture, seed, team, and position once; the two timing repetitions do not double the coverage counts.

Separating the motivating fixture from the established corpus gives:

| Change | Established 11: CPU / wall change | Established 11: added / lost | Latest95: CPU / wall change | Latest95: added / lost |
| --- | ---: | ---: | ---: | ---: |
| Expected cohorts only | +8.42% / +12.63% | 13 / 0 | +13.32% / +22.15% | 9 / 0 |
| Smaller-tree eligibility only | +0.67% / +1.21% | 0 / 0 | +2.50% / +5.88% | 7 / 0 |
| Both changes | +9.13% / +13.52% | 13 / 0 | +11.10% / +20.16% | 11 / 0 |

The smaller-tree rule is the cheaper standalone change, with gains confined to latest95 in this sample. Cohorts cost substantially more and improve coverage on the established fixtures as well. Their gains overlap on latest95, so adding the standalone gains would overstate the combined benefit.

## Latest fixture in absolute terms

| Arm | CPU/request | Wall/request | Added outcomes versus clean | Undecided zeros across five seeds |
| --- | ---: | ---: | ---: | ---: |
| Clean baseline | 3.738 s | 1.626 s | 0 | 5 |
| Expected cohorts only | 4.236 s | 1.986 s | 9 | 1 |
| Smaller trees only | 3.831 s | 1.721 s | 7 | 1 |
| Both changes | 4.153 s | 1.954 s | 11 | 0 |

With both changes, earlier tree recovery reduces mean cohort work on latest95 from approximately 467.46 million to 291.30 million modeled units per request, a 37.7% reduction. The tree itself spends additional work. Combined CPU remains approximately the same as cohorts alone across the complete corpus, while gaining two more cell/seed estimates. The small timing differences between those two arms do not establish a general speed improvement.

## Source and output verification

The clean binary was compiled from the archived source at HEAD and has SHA-256 `ff48a783754ea49a95c36b6aeb05db950fe0bc50541b676c970cb7aee227e594`. An independent audit found all 109 archived Rust files byte-identical to HEAD and no cohort paths. The working-tree binary has SHA-256 `794677a295bb7a784f7625d9cd33dc3f86b6368ebd0621c8abf721af6b7661ab` and contains the previously uncommitted cohort implementation.

All 480 measured exports match the corresponding feature combinations in the preceding benchmark byte for byte, including clean-source outputs versus the prior runtime-disabled cohort controls. Every repeated export also matches. Thus the earlier feature-off control isolated behavior correctly, while this new benchmark measures each individual change against an actual separate committed build.

The runner's post-run comparison selector initially retained its previous baseline name. Measurement commands, binaries, environment, CPU and wall records were unaffected. Paired summaries were regenerated from the same raw records using the corrected clean-baseline selector; no measured requests were rerun to select outcomes.

## Interpretation

The two changes should be evaluated separately: expanded tree eligibility offers a small, inexpensive coverage improvement here; expected cohorts offer broader coverage at a larger cost. They also complement each other, since their combination preserves all tested positive outcomes and recovers more cells at approximately the standalone cohort cost.

These are coverage and cost measurements, not an exact accuracy oracle for the extreme tail. The previously observed approximately 117-fold cross-seed/method disagreement for Palmeiras 15th remains present. Known residual root-count admission overruns on latest95 seed 13009 are shared by all arms and safely stop the stage; they do not explain the treatment gains. No production defaults were changed.
