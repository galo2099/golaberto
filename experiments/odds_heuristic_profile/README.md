# Production heuristic coverage and runtime, 2026-10-18

Current master was fetched and verified as
`77a3da2e24ff0fe51282326c44471a8768776e70`. This report covers seven distinct
saved snapshots, replayed as 23 saved fixture/seed pairs: 9,048 team-position
cell-runs. Snapshots repeat under different seeds; there are 2,724 distinct
snapshot/team/position cells.

The diagnostic branch adds only wall timers around existing operations. Its
default optimized release binary produced all 23 archived responses exactly,
including byte equality. Requests and raw results remain private. No point
estimator, score model, sample budget, ordering, or acceptance gate was changed.

## What counts as solved

The adaptive statistical rule requires a valid 99% simultaneous interval with
positive lower endpoint, absolute width at most `1e-2`, and log halfwidth at
most `0.05 * max(1, -log_midpoint)`. An impossibility proof also completes a
cell. A positive point estimate, a reachability witness, empirical relative
standard error, or two agreeing sampling batches does not alone satisfy this
rule.

The statistical counts below use the certificate experiment's allocation:
0.004 to ordinary Monte Carlo, 0.002 to own-point strata, and 0.004 to fixed
conditional sampling, divided over the predeclared 9,048-cell family. These
certificates are an offline assessment of identical production outputs; the
certificate branch has not been merged into master. Production's existing
`meets_precision_goal` field uses older empirical gates and is not this rule.

Final production outputs contain 8,503 positive estimates, 540 proved-impossible
cells, and five reachable zeros. Under the supported certificates, 3,630 positive
estimates qualify. Thus **4,170/9,048 (46.09%)** are statistically complete;
4,873 positive estimates and five zeros remain incomplete.

## Main pipeline

Times below are sums of wall-clock spans across all 23 requests, with four
workers. The mean includes requests in which a stage is skipped. The replay
took 31.036 seconds of process wall time, or 1.349 seconds per request; the
endpoint's calculation timers sum to 30.219 seconds. Only 15 requests enter
the full zero-recovery pipeline; the other eight take the short refinement
path. These timings exclude certificate calculations.

"New positive" means a transition from no positive estimate to a positive
estimate at that stage. "New completed" means a new qualifying statistical
estimate or an impossibility proof. Rows sharing a timer are explicitly marked.

| Heuristic or stage | What it does | Total time | Mean/request | New positives | New completed |
| --- | --- | ---: | ---: | ---: | ---: |
| Match-importance scout | 20,000 ordinary seasons for game importance | 1.918 s | 83.4 ms | Separate output | Separate output |
| Ordinary position Monte Carlo | 100,000 unconditional seasons; raw finish counts | 2.324 s | 101.0 ms | 6,988 | 3,620 |
| Point PMFs | Convolves each team's remaining points probabilities | 0.0086 s | 0.37 ms | Preprocessing | 0 |
| Matched-point pooling and balancing | Combines same-points observations across teams, smooths and balances | 0.0357 s | 1.55 ms | 1,067 beyond raw MC | 0 additional |
| Pool jackknife | Repeats the matrix with batches omitted to report empirical errors | 0.1226 s | 5.33 ms | 0 | 0 additional |
| Point-bound screening | Rules out positions using necessary point intervals | 0.0009 s | 0.04 ms | 0 | 500 proofs |
| Early joint/rank proofs | Uses shared fixtures and collective constraints to rule out more cells | 0.0396 s | 1.72 ms | 0 | 40 proofs |
| Fixed conditional sampling and blocker construction | Builds a necessary event and samples inside it | Initial phase 0.591 s; also uses shared extra phase below | 25.7 ms initial | Part of 117 below | 10 |
| Guided conditional lookahead | Favors promising remaining WDL outcomes, with likelihood corrections | 1.361 s | 59.2 ms | 117 across initial/guided/extra ordinary paths, shared count | 0 additional certified today |
| Extra conditioning and joint-cap setup/sampling | Enlarges fixed batches and builds/samples joint rival constraints | 2.707 s, shared by these paths | 117.7 ms | 39 joint-cap publications, in addition to the 117 | Included above / uncertified joint path |
| Neighborhood and constructive witnesses, walks, deferred score completions | Finds legal seasons and seeds other proposals | 0.246 s combined | 10.7 ms | No probability estimate of its own | No additional probability completion |
| Point tilting | Biases target/rival results toward the requested rank and corrects weights | 3.717 s | 161.6 ms | 97 | 0 additional certified today |
| Peer reuse | Uses a related successful cell to construct a proposal | 0.290 s | 12.6 ms | 2 | 0 additional certified today |
| Rank-domain conditioning | Restricts rival outcome domains to compatible rank-side relationships | 0.352 s | 15.3 ms | 11 | 0 additional certified today |
| Rare-tail portfolio | Combined late recovery family detailed below | 15.421 s inclusive | 670.5 ms | 182 | 0 additional certified today |
| Exact-cap recovery | Conditions on an exact selected cap event; refines four existing estimates | 0.993 s | 43.2 ms | 0; four refinements | 0 additional certified today |
| Final reconciliation | Repairs row/column probability sums while preserving accepted support | 0.0068 s | 0.29 ms | 0 | 0 |
| Final packed witness fallback | Checks legal supported score completions for remaining candidates | 0.00010 s | 0.004 ms | No probability estimate | 0 additional probability completion |

The tiny joint-publication timer (0.000103 seconds) is not its sampling cost;
sampling occurs in the shared extra-conditioning phase. The fixed, guided,
and extra phases together cost 4.659 seconds. Exact-cap reconciliation and
other small controller work are included in the outer search total of 25.745
seconds, rather than individually assigned to estimators.

Ordinary MC, matched pooling, and the rescue stages reconcile exactly:

`6,988 + 1,067 + 117 + 97 + 2 + 11 + 39 + 182 = 8,503 positive estimates`.

Own-team matched-strata certificates separately qualify 906 cell-runs, but all
906 already qualify through ordinary MC. This does not certify production's
cross-team matching heuristic. Fixed conditional sampling qualifies ten
cell-runs that raw MC does not. Other guided paths currently lack their own
complete confidence certificates; zero additional certified cells does not
establish that their point estimates are wrong.

## Rare-tail subphases

All rows below are inside the 15.421-second rare-tail parent above. Do not add
them to that parent again. They are serial controller phase wall times, not
sums of parallel worker sampling times.

| Heuristic | Total time | Mean/request | New positive estimates | Refinements |
| --- | ---: | ---: | ---: | ---: |
| Shared-constraint mixture | 0.501 s | 21.8 ms | 13 | 0 |
| Ordinary union/lazy proposal construction and training | 1.599 s | 69.5 ms | Prepares the next row | 0 |
| Main/check sampling plus after-extension | 1.429 s | 62.1 ms | 115 ordinary + seven extensions | 0 |
| Complete branches and trees, including retry | 1.931 s | 83.9 ms | 12 ordinary + one retry | 0 |
| Additional independent confirmation | 5.031 s | 218.7 ms | 24 | 0 |
| Late family fallback | 0.706 s | 30.7 ms | 2 | 0 |
| Target overflow-tree fallback | 0.281 s | 12.2 ms | 3 | 0 |
| Expected/feasible cohort recovery with sampled scores | 2.155 s | 93.7 ms | 5 | 0 |
| Cohort GD integration/refinement | 1.609 s | 70.0 ms | 0 | 5 |

The two cohort pass times are children of a 3.766-second cohort parent. Do not
add the parent again. The main/check timer already includes after-extension;
extension worker durations must not be added to it. Construction/training
excludes optional proposal reuse, which is off in this default run. The
remaining roughly 0.18 seconds of the outer rare-tail span cover handoff,
selection, bookkeeping, guide/controller work, logging and small uncovered
spans.

The new-positive column sums to 182. The rare-tail summary's 187 accepted
updates includes the five GD refinements. Four later exact-cap refinements
also overwrite previous publishers without adding positive cells.

## Every surviving production publisher

Final labels identify the last surviving estimator, not its first discoverer.
This table is therefore distinct from the incremental counts above. All labels
except `exact_caps_conditioned` retain a `matched_point_pool` prefix in code.

| Final estimator | Positive cell-runs | Cell-runs with a qualifying supported interval |
| --- | ---: | ---: |
| Matched-point pool, without a rescue suffix | 8,055 | 3,620 |
| Ordinary rare-tail portfolio | 107 | 0 |
| Fixed conditioned | 99 | 10 |
| Point tilt | 87 | 0 |
| Joint caps | 39 | 0 |
| Additional confirmation | 24 | 0 |
| Conditioned lookahead | 18 | 0 |
| Shared constraints | 13 | 0 |
| Complete branches | 12 | 0 |
| Rank domains | 11 | 0 |
| Rare-tail extension | 6 | 0 |
| Point-tilt undecided-gap variant | 5 | 0 |
| Expected cohorts | 5 | 0 |
| Exact-cap conditioned | 4 | 0 |
| Expected-cohort GD refinement | 4 | 0 |
| Cross-team point tilt | 4 | 0 |
| Target overflow tree | 3 | 0 |
| Peer point tilt | 2 | 0 |
| Family fallback | 2 | 0 |
| Crosschecked point tilt | 1 | 0 |
| Feasible-cohort GD refinement | 1 | 0 |
| Complete-branch retry | 1 | 0 |
| **Total positive estimates** | **8,503** | **3,630** |

The 540 proofs and five remaining reachable zeros retain the original matched
design label. Proof attribution is 500 point-screen proofs and 40 early proofs;
they must not be counted as 540 extra matched-pool successes. The 3,620 qualified
matched-pool cells have supporting ordinary-MC intervals, rather than a proven
cross-team-pooling error law.

## Shared proposal techniques and paths without final publications

The following are internal components, with time and discoveries charged to
their enclosing stages rather than separately counted:

- Target WDL path conditioning; blocker/rival selection; joint points/wins
  profiles that preserve shared-fixture constraints.
- Outcome-domain propagation and fixed outcomes; omission of fixtures proved
  irrelevant to a comparison; cached suffix distributions of points/wins.
- Cardinality guidance for counts above/tied/below; sequential guidance with
  likelihood corrections; target-root and conditional-score tilts.
- Successful-witness reuse during proposal construction, alternate result
  modes, shared guide caches, overlapping union mixtures, and defensive
  support outside learned regions.
- Pilot comparison of union/lazy proposals followed by fresh main/check
  streams; modeled setup/draw-cost allocation; complete branch coverage and
  transfers of unused budget.
- Expected and feasible rival cohorts; exact admitted private/shared GD
  integration; ordinary score sampling when integration is not admitted.

Cardinality/expected-points approximations can guide a corrected sampler; they
are not independent reachability proofs. Exact integration of a subproblem
does not by itself provide a confidence interval for the whole cell estimate.

Other paths checked or available in current production:

| Path | Status on this corpus |
| --- | --- |
| Plain-MC fallback for failed/unsupported matched matrices | No final use; all 23 runs admitted matched pooling |
| Exact outcome-path stratification for small schedules | No admitted replacement of the pool on these inputs |
| Saturated-cell point-certainty proof | Checked after search; no final `certain_by_points` publisher |
| Native full-world complement | Checked; zero accepted components and zero final publications |
| Packed-witness fallback | No new probability estimates; reachability-only operation |
| Experimental rank rescue and its stronger structural/root modes | Disabled by clean production defaults |
| Explicit fixture-bias retries, cardinality-case retries, fitted-root retries, optional guided fallback | Opt-in experiments; not part of this default run |

The complement and saturated-certainty checks do not have their own complete
wall timers in the archived or diagnostic code; their small cost remains in
the search controller residual. There is no honest independent runtime to
assign to a disabled path or a helper shared by several samplers.

## Reproduction and limits

The diagnostic-only Rust diff adds `training_phase_ms` to the existing rare-tail
summary and `elapsed_ms` to existing cohort run/pass summaries. The runner is
`run_profile.py --binary PATH --run`; it is inert without `--run`. It clears
inherited experimental settings, uses four workers, excludes a warmup, verifies
private input hashes, and compares every full response with the archive.

- Default optimized build: passed, using the locked offline dependency set.
- Successful measured runs: 23/23; structural JSON equality: 23/23; byte equality:
  23/23. Source, binary, manifest and input hashes remained unchanged.
- Inspected sample/work/publication counts matched the archive. One shared-guide
  cache-hit diagnostic differed (three versus four); no response bytes differed.
- Profile binary SHA-256:
  `bc193cec1a43d3720506c3ef8c22452920386e307f280ecea90af4086871be02`.
- Profile production-source SHA-256:
  `a4faabc20d061dc74208ddb33542d7b232346f1994c84750ce7b93f914d77876`.
- Private profile artifacts:
  `/workspace/rare-odds-heuristic-breakdown-2026-10-18`.
- Certificate data comes from the earlier verified same-output comparison in
  `/workspace/rare-odds-certificates-2026-10-18/paired`.

The earlier native-only Clopper–Pearson assessment completed 4,442 cells using
the full 0.01 error allowance for native evidence. The present 4,170 uses a
split allowance and conservative binomial-tail enclosures. Mixing those counts
would misstate the contribution of the additional heuristics. These are saved
fixture measurements, not a measurement of live service traffic.
