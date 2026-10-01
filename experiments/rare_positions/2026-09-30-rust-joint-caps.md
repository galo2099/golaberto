# Chapecoense/3rd: exact joint rival-cap conditioning

## Production integration, September 30

After the experiment and reported-seed replay, the user authorized integration.
The normal Rust library now exports `joint_caps`, and the production search
pipeline performs the measured deferred replacement. **It is enabled by
default**, with `RUST_ODDS_JOINT_CAP_CONDITIONING=0` as the disable switch. Rebuild
and restart the normal binary; no experimental enabling flag is needed.

The production implementation retains the model, random streams, acceptance
gates, existing four-candidate limit and four workers from the deferred
experiment. Its request-scoped `rust_odds_joint_caps` event contains group,
seed, request ID, setup and sampling times, main/confirmation diagnostics,
acceptance, and actual draw allocations. `rust_odds_start` reports the effective
enablement, and `search.joint_caps_publish` reports how many estimates were
published at the end. Unsupported models retain their previous ordinary batch.

The integrated binary repeated all 30 fixed-seed pairs against the preserved
pre-integration Rust executable, with no sampler enable flag. It gained the same
one distinct cell in six cell-runs, and preserved every existing positive
probability, SE, reachability proof and game-importance value exactly.

Production persistent check: seed 808, two warmups, eight timed pairs per
snapshot, alternating order, four workers and no overlapping calculations or
builds. Process CPU includes startup and warmups.

| Snapshot | Previous median ms | Integrated median ms | Wall change | CPU change |
|---|---:|---:|---:|---:|
| 16498 | 679.89 | 595.46 | −12.42% | −8.02% |
| 16653 earlier | 643.46 | 629.57 | −2.16% | −2.79% |
| 16653 current | 680.39 | 674.42 | −0.88% | **+1.12%** |
| 16982 | 296.92 | 289.25 | −2.58% | −2.12% |
| 16983 | 284.45 | 284.61 | **+0.06%** | −0.63% |

Group 16498's extra stage fell 144.62 → 92.09 ms; joint sampling including setup
took 25.15 ms, with 2.19 ms setup. Some of the whole-request difference came
from variation in other stages: point tilt measured 322.61 → 294.06 ms despite
unchanged estimates and work allocation there. The earlier experiment measured
−6.73% full-request wall and +0.93% CPU, so the integration check does not
establish a universal 12% gain or universal CPU reduction. Small control changes
are variation; cold earlier-16653 median increased **1.05%**. No additional time
allowance was used.

The sections below record the experiment as it was conducted before
integration; their statements about an isolated binary describe that historical
stage. The historical builder reconstructs the stored baseline commit and source
patch before applying either experimental hook, so its commands continue to
reproduce that comparison after integration.

Production paired results and test details are recorded under
`production_integration` in the results JSON. The integration remains uncommitted
and unpushed. No running deployment process was restarted by this task.

The full Rust release suite passed **60 tests**, with **two database tests
ignored**. The new normal-binary regression test clears inherited estimator
flags, compares the default with the disable switch at user seed
1790832522033172000, verifies Chapecoense/3rd's estimate, and checks every prior
positive probability, SE, reachability label and game-importance value. It also
checks that publication follows domain searches and that request-scoped logs
report the actual 5,000 + 2,000 + 10,000 draws. Unit tests check independent
confirmation rejection, shared-fixture exact masses, corrected weights, both
rank directions and unsupported/overflow cases. Existing HTTP route tests
required local socket access after the sandbox-only run blocked their binds.
No Rails behavior, JSON response schema, or database persistence was changed.

The final ordinary release build was also replayed across all 30 requests;
every response exactly matched its measured production candidate response. This
separate artifact check followed restoration of unrelated formatting in the
pre-existing proof source. Both measured and final binary hashes are recorded.
With all enabling flags unset, the reported user seed returns **5.107904e-24**
probability for Chapecoense/3rd, 90 conditional hits and 19.00% relative SE.

```sh
cargo build --release --locked --manifest-path odds-rust/Cargo.toml
odds-rust/target/release/golaberto-odds serve 127.0.0.1:6577
```

To compare defaults with the previous allocation using the same new executable:

```sh
python3 experiments/rare_positions/benchmark_joint_cap_requests.py \
  --baseline odds-rust/target/release/golaberto-odds \
  --candidate odds-rust/target/release/golaberto-odds \
  --production-candidate --output /private/tmp/joint-production-pairs
python3 experiments/rare_positions/benchmark_joint_cap_requests.py \
  --baseline odds-rust/target/release/golaberto-odds \
  --candidate odds-rust/target/release/golaberto-odds \
  --production-candidate --persistent \
  --output /private/tmp/joint-production-persistent
cargo test --release --offline --locked --manifest-path odds-rust/Cargo.toml \
  -j4 -- --test-threads=1
```

`--production-candidate` explicitly disables the feature for the baseline and
sets no sampler flag for the candidate. Inherited estimator flags are cleared.

## Result and recommendation

The experiment found a usable estimate for Chapecoense (318) finishing 3rd in group 16498 in **all six fixed seeds**, versus zero in all six baseline requests. Final probabilities were **4.40e-24 to 8.08e-24**, roughly **6e-24 probability**, or **6e-22%**.

Recommend the **deferred, budget-replacement integration**. Reject the initial integration that published estimates before existing searches: it disrupted witness reuse and lost another estimate. Production sources and defaults remain unchanged; the integration is built in an isolated temporary copy. Nothing was committed or pushed.

This adds **one distinct nonzero cell, observed in six cell-runs**. It does not resolve every reachable zero. There is no high-budget ground truth for this probability; repeated seeds, independent confirmation streams, and exhaustive small-model tests support an order-of-magnitude estimate rather than a certified numerical value.

Machine-readable measurements, input/binary hashes and additional regression checks: [results](results/2026-09-30-rust-joint-caps.json). The baseline was the existing Rust executable, with four workers, at commit `c122f2ef` plus the existing working-tree source changes. Those changes are preserved separately in the [baseline source patch](results/2026-09-30-rust-joint-caps-baseline.patch). This experiment did not edit them.

## Why the existing sampler struggles

Chapecoense starts at 18 points and three wins, with 11 matches remaining. It must win all 11 to reach 51 points and 14 wins. Replacing one win with a draw gives 49 points and 13 wins: Flamengo and Palmeiras are already above that total, and Athletico already has 49 points and 14 wins. Therefore third is impossible unless all 11 results are wins.

The model probability of all 11 wins is **4.43673276775e-8**. But that necessary event is only the first hurdle. Athletico can add at most two points; Fluminense at most three; Bahia at most five; Cruzeiro at most five, because six additional points would also give Cruzeiro too many wins. Further rivals must stay below Chapecoense's final points/wins total.

These are coupled constraints across shared fixtures. They do not fix most games individually. After Chapecoense's 11 wins, 92 matches remain: the individual domains force no additional single result, nine games have two possible outcomes, and 83 have three. Ordinary target-point conditioning still spends nearly all its effort missing the simultaneous rival restrictions.

At seed 808, the existing production extra batch used 50,000 conditional draws, got no third-place hits, and returned a conditional zero-hit upper bound of 1.375e-14. That upper bound is not the event's probability. The new sampler finds a much smaller positive estimate through a different proposal with corrected weights.

## Generic model

Implementation: [joint_caps.rs](../../odds-rust/src/joint_caps.rs), exercised through [joint_cap_conditioning.rs](../../odds-rust/examples/joint_cap_conditioning.rs). The module is deliberately not exported by the production library. The [isolated builder](build_joint_cap_experiment.py) exports and integrates it only in its temporary copy.

Eligibility is mathematical, without team or rank constants:

1. Points must be the first sorting criterion, with ordinary 3/1/0 scoring and no bonus points. Wins are packed into the ordering only when they immediately follow points.
2. Prove that the target's extreme outcome pattern is necessary: every less favorable target total already has too many guaranteed rivals ahead.
3. After fixing that pattern, the number of guaranteed-ahead rivals must equal the requested number ahead. Every other rival must therefore obey a necessary packed points/wins cap. Ties remain allowed at this screening stage.
4. Rank rivals by their exact independent cap-tail probabilities and select the tightest, up to six. These marginal probabilities select rivals; they are not multiplied as the final joint probability.
5. Split selected rivals' fixtures into matches between selected rivals and each rival's external matches. Enumerate the internal W/D/L combinations, using each shared fixture once. Weight each combination by its fixture probabilities and the exact external suffix CDFs for the remaining caps.
6. Draw internal combinations and external streams conditional on this exact joint event. Sample other fixtures with the existing style of cap-directed suffix proposal, correcting every draw by its true-probability/proposal-probability ratio.
7. Sample scores conditional on outcomes and verify the exact requested rank with the production sorter through `ScoreContext`. Only those hits contribute to the estimate.

For internal pattern `a`, external team stream `i`, and remaining cap `c_i(a)`, the exact selected-cap mass is:

```text
J = sum_a P(internal pattern a) * product_i P(external total_i <= c_i(a))
P(requested rank) = P(target pattern) * J * E_proposal[rank_hit * likelihood_ratio]
```

The external streams have disjoint fixtures after the selected-versus-selected matches are separated, so their product is valid. Their outcomes still affect unselected rivals; those totals are retained and checked. There is no multiplication of two unconditional zero-hit bounds.

The worst-rank direction uses the same mechanism with the packed order negated and per-fixture minima removed, keeping residual gains nonnegative. Both directions are tested. Cases that require several target totals or alternative exempt-rival sets are outside this experiment's eligibility gate.

### Chapecoense's six-rival plan

Selected rivals: Fluminense (8), Athletico (5), Cruzeiro (15), Bahia (74), Atlético-MG (4), São Paulo (14).

| Quantity | Value |
|---|---:|
| Target fixtures fixed | 11 |
| Shared fixtures between selected rivals | 9 |
| Internal W/D/L patterns | 19,683 |
| Selected rivals' fixtures sampled jointly | 50 |
| Residual fixtures | 42 |
| Exact joint rival-cap probability, given target wins | 2.609936e-12 |
| Target wins × joint rival caps | 1.157959e-19 |
| Product of six independent rival-cap probabilities | 6.522239e-10 |

The independent product is approximately **250 times** the correct joint cap mass. Shared fixtures matter considerably. Even the correct necessary-event mass is still much larger than the final third-place estimate, because other rivals and actual tiebreakers remain to be satisfied.

## Standalone comparison

Each rival count used 5,000 draws per seed, with seeds 801, 804, 808, 817, 818 and 911. At most four workers were active. Model construction occurred once per configuration; standalone six-seed batch timings amortize that setup and are not full-request costs.

| Selected rivals | Accepted estimates / 6 | Median effective sample size |
|---|---:|---:|
| Existing lookahead control | 0 | 0.00 |
| Target-only, new residual proposal | 0 | 2.41 |
| 1 | 0 | 2.94 |
| 2 | 0 | 4.67 |
| 3 | 0 | 3.88 |
| 4 | 4 | 13.69 |
| 5 | 6 | 19.67 |
| 6 | 6 | 31.77 |

Six rivals gave the most consistent estimates, around 5.05e-24 to 6.48e-24 in the standalone streams, with relative standard errors around 14–20%. Five also succeeded, but one result exceeded 1e-23. The six-rival setup cost about 2.0 ms standalone and 2.25–2.47 ms in cold full requests. Full-request setup plus both native sample streams cost about 23–30 ms.

The exact cap model enumerates at most 20,000 internal patterns and caps DP spans at 4,096. Six rivals are attempted first, then five if the internal graph exceeds the gate. An unsupported model, numerical underflow, or exceeded limit skips this estimator; none establishes impossibility.

## Allocation experiment and regression diagnosis

### Rejected: publish before existing searches

The first integration attempted eligible initial zero cells and immediately published successful estimates. Chapecoense/3rd appeared in all six seeds, but:

- Chapecoense/5th lost its existing nonzero estimate at seed 804.
- Chapecoense/4th and /5th lost their reachability labels in every group-16498 seed.
- Some other probabilities changed because downstream work allocation changed.
- The hook spent work on temporary zeros that later ordinary samplers already estimated.

Publishing third early removed a neighborhood query whose seasons were useful for proving fourth and fifth. This was an allocation regression, not evidence that those positions became impossible. Reject this variant.

### Recommended: replace an existing batch and publish last

Keep the existing extra-search candidate ranking and four-candidate limit. Only attempt the joint model when a cell already receives an extra 50,000-draw batch.

- Run 5,000 joint draws plus an independent 2,000-draw confirmation stream.
- If accepted, retain 10,000 ordinary draws: **17,000 actual draws instead of 50,000**.
- If rejected, run 43,000 ordinary draws: **50,000 total**, including the joint attempt.
- If unsupported, retain the original 50,000 ordinary draws.
- Hold the accepted estimate until after existing candidate searches, neighborhood searches and deferred witnesses. Publish only if the cell is still zero and not proved impossible, before matrix reconciliation.

The main stream must pass the existing coarse gate: at least 30 hits, ESS at least eight, relative SE at most 35%, maximum weight share at most 25%, and acceptable batch agreement. Confirmation requires at least 30 hits, ESS at least five, relative SE at most 60%, and agreement within a factor of ten. This confirmation is an empirical safeguard, not a mathematical guarantee.

No new thread, candidate, proof-node or neighborhood quotas were allocated. Ordinary zero-hit statistics use the number of ordinary draws actually performed. Joint successes carry their own conditional mass, samples, hits and weighted SE. Witnesses do not become probabilities merely by being found.

## Paired full-request quality

Thirty pairs used identical frozen requests and seeds: group 16498, earlier and current snapshots of 16653, 16982 and 16983. Requests were sequential, with four workers per request, and baseline/candidate order alternated.

| Seed | Chapecoense/3rd baseline | Deferred candidate probability |
|---|---:|---:|
| 801 | 0 | 6.093511e-24 |
| 804 | 0 | 4.395774e-24 |
| 808 | 0 | 5.399591e-24 |
| 817 | 0 | 7.257309e-24 |
| 818 | 0 | 8.080630e-24 |
| 911 | 0 | 6.452574e-24 |

Across all 30 pairs:

- One distinct additional nonzero cell; six additional nonzero cell-runs.
- Zero lost nonzero cells; every existing positive probability and standard error stayed exactly equal.
- Zero lost reachability labels and zero impossible-cell regressions.
- Game-importance values stayed exactly equal.
- No additional impossible or newly reachable cells: Chapecoense/3rd was already proved reachable.
- No changes to coverage in the other snapshots. Current 16653's seed-818 undecided Londrina/5th remained undecided.

At seed 808, group 16498 changed from **38 zeros to 37**: 17 impossible zeros, 20 reachable zeros and zero undecided cells. Across the completed baseline zero cells, only Chapecoense/3rd met this experiment's eligibility gate in these reference requests. General implementation does not imply broad present-day coverage.

## Current Rust latency and CPU

### User-reported seed: 1790832522033172000

Replayed the reported seed against the same frozen group-16498 request. The ordinary production executable returned zero for Chapecoense/3rd, with reachability `reachable_by_construction`. It contains no joint-cap integration. The isolated deferred candidate, with `RUST_ODDS_JOINT_CAP_EXPERIMENT=replace`, returned **5.107904e-24** probability: 90 main-stream hits, ESS 27.55, relative SE 19.00%, and 36 confirmation hits. No existing positive probability, SE, game importance, or reachability proof was lost or changed.

The submitted `http.decode` line identifies the seed and request decoding only; it does not establish that the joint estimator ran. Its identifying candidate log is `rust_odds_joint_cap_experiment` with `accepted: true`. The production module is still unexported, so setting that flag on an ordinary production build does not enable it. This confirms a build/integration difference for the saved snapshot, not the current database request's exact contents. The replay and hashes are recorded under `user_seed_check` in the results JSON.

### Persistent and cold measurements

A persistent-server check used seed 808, two warmups and eight timed requests per side, alternating order. No concurrent builds or other benchmark requests ran. Wall times are complete HTTP requests after server startup, not only sampler loops.

| Snapshot | Baseline median ms | Deferred median ms | Wall change | Process CPU change |
|---|---:|---:|---:|---:|
| 16498 | 643.37 | 600.05 | **−6.73%** | **+0.93%** |
| 16653 earlier | 642.29 | 634.50 | −1.21% | −4.04% |
| 16653 current | 668.89 | 661.62 | −1.09% | −1.29% |
| 16982 | 295.78 | 294.02 | −0.59% | −0.15% |
| 16983 | 283.43 | 284.99 | **+0.55%** | 0.00% |

Group 16498's extra-search stage median fell from **141.41 to 89.55 ms**. The new setup and native sampler are inside that stage. Reducing an expensive job on the critical path lowered wall time, while the more elaborate proposal slightly increased aggregate CPU. Peak RSS rose from 186.42 to 191.78 MiB.

The six-seed cold-request medians also decreased for 16498, from 849.81 to 797.37 ms (−6.17%). Earlier 16653 increased 0.47% in that comparison. These small control changes should be treated as measurement variation, not benefits of an inactive estimator. There is no claim of strict latency preservation on every request or arbitrary phase: **the measured increases are reported**, and no additional allowance was assumed. Neither the historical Go times nor one second served as a budget.

The measured executable initially counted native work as fixture units only, whereas the established diagnostic counts fixtures plus teams per draw. The final source corrects that accounting, and a seed-808 check preserved every probability and SE exactly. Normalized total modeled work was **93,406,200 → 89,347,200**; the reduction of 4,059,000 equals 33,000 draws × (103 fixtures + 20 teams). This counter is not measured CPU time. The paired timing executable's original SHA and the final accounting-check SHA are both in the results JSON.

## Correctness and limits

- Every actual rank hit is checked with the production sorter and conditional goal sampler. Relaxed cap feasibility alone never proves rank reachability.
- Necessary caps permit point/win ties; the final score sample determines their actual order.
- Every shared fixture is assigned once. Independent cap-tail products are used only where fixture streams are genuinely disjoint.
- The residual directed proposal keeps positive support for cap-feasible outcomes, and its likelihood ratio is included. Zero proposal continuations correspond to violated necessary remaining caps.
- No arbitrary score cap establishes unrestricted impossibility. Failed score/rank samples are misses, not no-good constraints over their whole outcome patterns.
- Bounds, unsupported scoring, overflow and numeric underflow skip this method. They never change a reachability classification by themselves.
- Repeated-seed agreement does not rule out rare high-weight contributions. Published SE is a sampling diagnostic, not a proven error bound. The result is useful for the requested order of magnitude, with validation limited to this model and these snapshots.
- Replacement reduces the ordinary witness-search budget for an accepted cell. Deferred publication eliminated observed regressions, but unseen requests may still depend on seasons the discarded 40,000 ordinary draws would have found. Broader paired coverage should precede a default rollout.

Primary implementation references are the repository's [conditioned sampler and score verification](../../odds-rust/src/conditioned.rs), [suffix proposal](../../odds-rust/src/lookahead.rs), [allocation and reconciliation](../../odds-rust/src/search.rs), [work and point pool](../../odds-rust/src/pool.rs), and [production sorter](../../odds-rust/src/sort.rs). This experiment derives the joint factorization directly; it does not rely on external solver timings or a solver's feasible relaxation as a witness.

## Reproduction

Preserve the current baseline executable before building. Hashes in the results JSON identify the measured baseline and frozen requests; if the repository has changed, reconstruct the baseline from the recorded commit and source patch in a separate checkout.

```sh
cp odds-rust/target/release/golaberto-odds /private/tmp/joint-cap-baseline
CARGO_TARGET_DIR=/private/tmp/chapecoense-joint-target \
  cargo build --release --offline --locked \
  --manifest-path odds-rust/Cargo.toml --example joint_cap_conditioning -j4
python3 experiments/rare_positions/benchmark_joint_caps.py \
  --example /private/tmp/chapecoense-joint-target/release/examples/joint_cap_conditioning \
  --baseline /private/tmp/joint-cap-baseline \
  --output /private/tmp/chapecoense-joint-experiment --samples 5000
python3 experiments/rare_positions/build_joint_cap_experiment.py \
  --allocation deferred --directory /private/tmp/golaberto-joint-cap-deferred \
  --target /private/tmp/chapecoense-joint-deferred-target
python3 experiments/rare_positions/benchmark_joint_cap_requests.py \
  --baseline /private/tmp/joint-cap-baseline \
  --candidate /private/tmp/chapecoense-joint-deferred-target/release/golaberto-odds \
  --output /private/tmp/chapecoense-joint-deferred-pairs
python3 experiments/rare_positions/benchmark_joint_cap_requests.py \
  --baseline /private/tmp/joint-cap-baseline \
  --candidate /private/tmp/chapecoense-joint-deferred-target/release/golaberto-odds \
  --persistent --output /private/tmp/chapecoense-joint-deferred-persistent
CARGO_TARGET_DIR=/private/tmp/chapecoense-joint-target \
  cargo test --offline --locked --manifest-path odds-rust/Cargo.toml \
  --example joint_cap_conditioning -j4 -- --test-threads=1
```

The harness clears inherited estimator flags and enables `RUST_ODDS_JOINT_CAP_EXPERIMENT=replace` only for its candidate copy. This flag has no effect on the current production executable. Use `--allocation early` and a different temporary directory to reproduce the rejected variant.

**Four targeted tests passed**: exact joint mass against exhaustive shared-fixture enumeration in both directions; weighted rank estimates against exact probabilities; a four-rival internal graph against exhaustive assignments; and unsupported/overflow cases skipping the model. The weighted test also verifies normalized work accounting. Python entrypoints compile successfully. No Rails or database behavior was changed, and no database mutations were performed.
