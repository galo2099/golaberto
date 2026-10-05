# R64: two-result snapshot and remaining-zero diagnosis

2026-10-04. Investigation complete. Production defaults and probability
publication gates remain unchanged. No database mutations, commit or push.

## Snapshot revisions

The original experiments below use the first two-result fixture, SHA256
`69151d329b594fc01b8695e788b1400724bc21d4fbb176c9f9df1dc8974a88ea`.
The user subsequently updated the database ratings and requested replacing its
remaining-game powers. The tracked fixture now has SHA256
`cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee`.
See the rating-refresh results at the end of this report. The original JSON is
archived locally as `local/2026-10-04-two-results/group-16498-two-results.original.json`;
historical reproduction commands below use that archive.

## Fixture and reproduction

Derived from `reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json`.
Only these games' scores and `played` state change:

| Game | Home team | Away team | Final score | Played |
| ---: | ---: | ---: | --- | --- |
| 364007 | 14 | 6 | 1–2 | true |
| 363999 | 4 | 225 | 1–0 | true |

Original fixture: `reference/2026-10-04-two-results/inputs/group-16498-two-results.json`
(superseded by the rating refresh).
SHA256: `69151d329b594fc01b8695e788b1400724bc21d4fbb176c9f9df1dc8974a88ea`.
It preserves all other request fields, ratings, game order and 380-game count;
remaining fixtures decrease from 103 to 101. The new `two_results_fixture`
integration test checks exact JSON derivation, the simulated fixture IDs, every
affected campaign field, and unchanged campaigns for all other teams.

Baseline: commit `e3ce9653`, executable SHA256
`ae0e68a5d5986436656ca34aa6b6b6a9f54befeaaf96049dfbc0df96ab6bbdf1`.
Fixed seed 808; four estimator workers; requests and builds run sequentially.
The paired harness clears inherited `RUST_ODDS_*` and `RARE_POSITION_*` variables.

```sh
odds-rust/target/release/golaberto-odds estimate \
  experiments/rare_positions/local/2026-10-04-two-results/group-16498-two-results.original.json \
  /tmp/two-results.json 808 4
```

## Initial production-baseline result

| Configuration | Positive | Impossible zeros | Reachable zeros | Undecided zeros | Wall s | CPU s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Current defaults | 382 | 17 | 1 | 0 | 2.113 | 5.061 |
| Same executable, overflow tree disabled | 381 | 17 | 2 | 0 | 1.870 | 4.906 |

These are single serialized measurements, not a benchmark claim. The tree adds
Palmeiras/14th at `7.100e-25` probability. Flamengo/13th is estimated by complete
branches at `7.595e-34`. Palmeiras/15th remains the only reachable zero.

This fixture does not reproduce the entire pasted production request. Both have
101 remaining fixtures and the same initial positive-cell count, but proposals
and sampling diagnostics differ. For example, Palmeiras/15th's native
conditioning mass is `0.00011168892537677116` here, versus
`0.00010963555271845533` in the pasted log. The pasted log contains neither the
complete request body nor its hash; additional data/build differences cannot be
identified conclusively from it. Its 21.25-second timing is not a paired timing
for this snapshot.

## Palmeiras/15th: verified failure mechanisms

Palmeiras starts at 57 points, 16 wins and ten remaining games. The certified
packed points/wins limit permits at most two additional points for this rank.
There are 56 target paths: ten losses; one draw and nine losses; or two draws
and eight losses. These are target combinations, not complete league seasons.

The native pilot has four hits but ESS 1.005. Additional native MAIN runs 30,000
draws, finds 227 qualifying seasons, and produces raw `5.259e-32` probability,
ESS 2.264 and maximum contribution share 0.657. It fails the existing publication
gates; CHECK is not run. This raw value is not a published estimate or bound.
The complete-branch constructor exhausts its case/setup budget and correctly
skips its incomplete union. The overflow stage counts 56 roots, excludes this
cell below its default minimum of 65, and finishes with 151,657,376 work units
unused. Thus the final zero includes both proposal weakness and an eligibility
gap, despite a verified reachability witness.

## Preselected experiments

1. Offline complete small-root tree, 6,000 MAIN/CHECK draws, five pilot draws
   per leaf, two-draw allocation floor, three message refinements; seeds 808 and
   1669. Repeat with tenfold final draws to diagnose unseen contribution tails.
2. Default-off `RUST_ODDS_EXPERIMENT_TARGET_TREE_MIN_ROOTS=1`, using the same
   original confirmation bank and publication gates. Check complete request
   output parity without the flag and compare gains/losses with the flag.
3. Stronger pilots within the same bank, if the first two experiments show
   material underallocation to useful leaves. Final samples remain independent
   of training. Do not select a shorter final batch after observing its result.

Raw runs are local under `local/2026-10-04-two-results/`; frozen executables and
raw exports are excluded from the proposed fixture/documentation changes.

## Offline small-root tree: allocation diagnosis

Use the existing `branch_stratification` example, complete roots, 256-leaf cap,
zero constructor training, rank priority, four rivals, four workers, three
message refinements, five bound pilots per leaf and final floor two. Construction
produces 138 leaves from 56 roots. This example's setup accounting excludes
bootstrap/root-enumeration work; its operation totals are not evidence that a
proposal fits the endpoint bank. Final rows retain every leaf, exact weights,
score sampling, production sorting and independent streams.

| Final draws per stream | Seed | MAIN probability | CHECK probability | MAIN ESS | CHECK ESS | Accepted |
| ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 6,000 | 808 | 3.065e-31 | 1.040e-30 | 6.463 | 1.573 | no |
| 6,000 | 1669 | 1.721e-31 | 1.629e-31 | 6.235 | 14.917 | no |
| 60,000 | 808 | 1.341e-30 | 3.616e-31 | 3.347 | 10.679 | no |
| 60,000 | 1669 | 3.614e-31 | 3.977e-31 | 18.525 | 10.066 | yes |

The preselected tenfold batch does not repair seed 808. Its five-draw pilots
allocate 52,792 of 60,000 MAIN draws to leaf 53 and only 45 to leaf 51. Leaf 51
then contributes `6.323e-31`, with ESS 1.019 and maximum share 0.991. Leaves
51–54 share the same all-loss target path but different rival status constraints.
The unseen variance in a pilot is not zero variance in the true distribution.
The final floor preserves support but supplies too little information here.
Seed 1669 distributes work differently and supplies independent evidence for
the order of magnitude, not a certified reference probability.

```sh
env TREE_MODE=1 TREE_TRAINING=0 TREE_LEAVES=256 TREE_GUIDE_VALUES=4000000 \
  TREE_PRODUCTION_PROFILE=1 TREE_MESSAGES=3 \
  RUST_ODDS_EXPERIMENT_TREE_RANK_PRIORITY=1 \
  odds-rust/target/release/examples/branch_stratification \
  experiments/rare_positions/local/2026-10-04-two-results/group-16498-two-results.original.json \
  16 15 6000 808,1669 trained bounds 4 tilt plain 5 2 off
```

Repeat with 60,000 instead of 6,000. Clear inherited estimator/`TREE_*` settings
first. Offline draws have their own independent namespace; passing one offline
seed does not imply that the endpoint's streams will pass.

## Same-bank eligibility screen and regression correction

An experimental minimum-root flag defaults to 65 and permits a lower value for
diagnosis. All other limits, grants, weighting and publication gates remain
unchanged. Initial minimum-one ordering by root count recovers Palmeiras/15th at
seed 808 (`2.135e-31`), preserving all baseline estimates exactly. However, it
loses Palmeiras/14th at seed 1669 and trades Flamengo/12th for Palmeiras/16th at
seed 1993. Fifty-draw pilots also produce losses. These variants are rejected
for production adoption.

Cause: newly admitted small-root trees ran before the original overflow trees
and spent their work. The revised experiment orders original 65–256-root
candidates first, retaining their original root-count/cell-index order and
four-cell priority; only afterward may 1–64-root candidates use leftovers.
Root counting already visits all residual cells in the default, so lowering the
minimum adds no counting work before the original candidates. Independent review
found no support or default-order change; paired validation follows.

## Revised priority: paired full requests

Candidate executable SHA256:
`0fe57de92cb4c9753ed60debbe6d973301d503b457371dc9edb98c68bac127ed`.
Set only `RUST_ODDS_EXPERIMENT_TARGET_TREE_MIN_ROOTS=1`. Original overflow trees
run first; small-root trees receive only their residual work.

| New fixture seed | Baseline reachable zeros | Revised reachable zeros | New estimates | Lost estimates |
| ---: | ---: | ---: | ---: | ---: |
| 808 | 1 | **0** | 1 | 0 |
| 1669 | 3 | 3 | 0 | 0 |
| 1993 | 4 | 4 | 0 | 0 |
| 2281 | 3 | 3 | 0 | 0 |
| 2293 | 1 | 1 | 0 | 0 |

Every existing positive and its non-work metadata is preserved across all five
paired requests. Game importance and reachability classifications agree except
for the new accepted probability. All six original reference snapshots at seed
808 also preserve their probabilities, metadata, proofs and game importance.
The unflagged candidate in the initial parity screen reproduces the frozen
default exactly. The priority helper's default remains the original order.

At seed 808, the revised tree stage adds both Palmeiras/14th and Palmeiras/15th.
The former retains its exact baseline estimate. The new /15th publication is
`2.1351748813839996e-31` probability (`2.135e-29%`), raw MAIN
`2.13517481300886e-31`, CHECK `1.1515667250825103e-31`, ESS 5.995/10.821 and
MAIN maximum share 0.337. There are 6,000 final draws per stream. These pass the
existing rough-estimate gates; the probability is not a certified interval.

Both cells fit within the original 265,894,075-unit residual bank. Total tree
work is 215,200,049 units, leaving 50,694,026. Compared with the default tree's
114,236,699 units, this uses 100,963,350 previously unused units. Bank capacity
and four-worker usage do not increase. Three warmed repeats reproduce the same
published estimate and export hash.

The 17 zeros still present at seed 808 are proven impossible:

| Team | Impossible ranks |
| --- | --- |
| Palmeiras (16) | 17–20 |
| Flamengo (17) | 14–20 |
| Team 20 | 1 |
| Team 79 | 1 |
| Team 110 | 1–2 |
| Team 318 | 1–2 |

Other seeds still leave reachable cells zero. The experiment supplies no
universal completeness guarantee and earns no additional work allowance.

## Latency and recommendation

Three warmed, serialized pairs rotate arm order, with one warmup per arm:

| Arm | Mean full-process wall s | Mean child CPU s |
| --- | ---: | ---: |
| Frozen production baseline | 2.557 | 5.540 |
| Revised small-root experiment | 2.833 | 5.622 |

The observed increase is **0.276 s wall (+10.8%) and 0.082 s CPU (+1.5%)**.
Wall samples span 2.30–3.07 seconds. These are local four-worker measurements,
not a production Xeon benchmark; do not infer unchanged latency from unchanged
work capacity. The original reference group timing differences are noisy and
are not attributed to the strategy.

Recommend the revised minimum-one rule as a focused opt-in follow-up. Reject
the original small-root-first ordering and the fifty-pilot global tuning.
Keep production defaults unchanged pending adoption authorization. This fixes
the eligibility gap on the requested default-seed fixture and preserves the
existing searches, but does not resolve every holdout zero or establish an
accurate golden probability.

```sh
RUST_ODDS_EXPERIMENT_TARGET_TREE_MIN_ROOTS=1 \
  odds-rust/target/release/golaberto-odds estimate \
  experiments/rare_positions/local/2026-10-04-two-results/group-16498-two-results.original.json \
  /tmp/two-results-small-roots.json 808 4
```

For paired reproduction, use `2026-10-04-target-overflow/scripts/run_overflow.py`
with the frozen baseline path/hash, candidate executable, the original archive's
parent directory as `--reference-dir`, `--groups 16498`, `--seeds 808,1669,1993,2281,2293`
and `--arm small:RUST_ODDS_EXPERIMENT_TARGET_TREE_MIN_ROOTS=1`. Raw evidence:
`preserved-priority/`, `reference-controls-v2/`, `timing-v2/`. The rejected initial
ordering is archived separately under `same-bank-screen/` and `holdouts/`.

Validation: fixture and parser/priority unit tests added; independent Sol review
found no actionable correctness issue. Full release Rust suite passed: 192 tests,
with four database tests ignored because `MYSQL_TEST_URL` was not configured.
Nine Python harness tests, formatting and `git diff --check` also passed.

```sh
cargo test --offline --locked --release -j1 \
  --manifest-path odds-rust/Cargo.toml -- --test-threads=1
```

## Rating refresh requested by the user

After updating team ratings, the user requested replacing the remaining-game
powers in the tracked two-result fixture. Capture used the existing Rails
request builder against `GolAberto_development`:

```sh
bin/rails runner -e development \
  'require "json"; group = Group.find(16498); print JSON.generate(OddsRequestBuilder.new(group).build)'
```

The command only reads database data; it does not invoke `Group#odds` or persist
odds, ratings or results. The raw builder output is archived locally as
`local/2026-10-04-two-results/updated-builder-request.json`, SHA256
`d381e6f0f0773ab0d73c71323ee4786de31dd66bfcf9469a56fbbed8365f7f20`.
Map its powers by game ID, checking that both participant IDs match before
replacing the 101 remaining fixtures' `home_power` and `away_power` values.
All captured means are finite and positive. Values change for 100 games
(200 mean fields); one game's means already agree. Every other JSON field and
the original fixture order remain unchanged, including played game 364007 at
1–2 and played game 363999 at 1–0. Played games' powers are retained.

Updated fixture SHA256:
`cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee`.
The offline test now permits only the two requested results and remaining-game
power refresh, retains the standings/fixture-ID assertions, and passes after
the refresh. Formatting and exact field-level checks pass as well.

### Refreshed request: seed 808, four workers

The same frozen production executable and revised experimental executable used
above ran sequentially against the refreshed input, clearing inherited flags.
Raw evidence: `local/2026-10-04-two-results/refreshed-ratings-seed808/`.

| Arm | Positive | Impossible zeros | Reachable zeros | Undecided zeros | Wall s | CPU s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Production defaults | 380 | 17 | 3 | 0 | 1.958 | 4.259 |
| Experimental minimum roots = 1 | 380 | 17 | 3 | 0 | 2.135 | 3.903 |

These are single measurements, not a timing recommendation. The response
exports are byte-for-byte identical between the two arms. Palmeiras/14th is
estimated at `3.696e-25` probability. The reachable zeros are Flamengo/13th,
Palmeiras/15th and Palmeiras/16th. All 17 impossible zeros retain the same
classification listed above.

The native Palmeiras/15th MAIN estimate is `2.163e-32`, CHECK `1.422e-31`;
their ratio is approximately 0.152 and fails the existing 0.2–5 agreement gate.
These raw diagnostics are not published probabilities. Flamengo/13th has no
event evidence in its complete-tree pilots. The late stage counts 1, 11 and 56
roots for Palmeiras/16th, Flamengo/13th and Palmeiras/15th respectively. The
smaller-root experiment admits them as candidates, but all three construction
grants are below the setup admission floor: 11,623,694 available units versus
approximately 12,389,000 required. It skips them without spending additional
tree work. The prior minimum-one coverage gain therefore does not transfer to
this refreshed request; keep production defaults unchanged.

The updated probabilities also differ from the earlier pasted production log:
Palmeiras/15th conditioning mass is now `0.00011228443082242327`, compared with
`0.00010963555271845533` in that log. This refresh uses the newly updated local
ratings, not a recovered historical production request body. The same count
of three reachable zeros does not mean the same cells or complete inputs.

```sh
odds-rust/target/release/golaberto-odds estimate \
  experiments/rare_positions/reference/2026-10-04-two-results/inputs/group-16498-two-results.json \
  /tmp/two-results-refreshed.json 808 4

cargo test --offline --locked --release -j1 \
  --manifest-path odds-rust/Cargo.toml --test two_results_fixture -- --test-threads=1
```

Validation for this refresh: one targeted fixture integration test passed,
`rustfmt --check` passed, and independent inspection confirmed exact parity of
all 101 remaining fixtures' powers with the captured builder output. No
production-code change, database mutation, commit or push.

## Comparison with the original 103-remaining-game fixture

To separate the new results from rating changes, compare all four combinations
at seed 808 with four workers and the same frozen production executable. A, B
and C use the recorded runs above. D is a new local counterfactual: start with
the original request, update only powers for all 103 unplayed games from the
captured builder output, and leave both specified games unplayed at 0–0.
Its SHA256 is `efaff6932ec6f4ce32344a8c766dc476fb5127fed4d97fd20b2b59589e62fd24`;
raw evidence is under `local/2026-10-04-two-results/counterfactual-ratings/experiment/`.
The two reverted games' captured powers already match their original powers,
so D differs from A in the same 200 means across 100 games as C differs from B.

All probabilities below are fractions, not percentages, and are rough published
estimates rather than exact reference probabilities. Zero means that no
probability estimate passed the publication gates.

| Cell | A: 103 remaining, old ratings | B: 101 remaining, old ratings | D: 103 remaining, updated ratings | C: 101 remaining, updated ratings |
| --- | ---: | ---: | ---: | ---: |
| Flamengo/13th | 1.173e-33 | 7.595e-34 | 1.196e-33 | 0 |
| Palmeiras/15th | 1.000e-33 | 0 | 0 | 0 |
| Palmeiras/16th | 1.683e-40 | 2.490e-40 | 1.227e-40 | 0 |
| All positive cells | 383 | 382 | 382 | 380 |
| Reachable zeros | 0 | 1 | 1 | 3 |

Every arm has the same 17 proven impossible zeros and no undecided zeros. The
certified upper packed limits for the three cells are unchanged. Flamengo and
Palmeiras retain their own points, wins and remaining fixtures: Flamengo has
60 points/18 wins/10 games; Palmeiras has 57 points/16 wins/10 games. The new
results add three points and one win to teams 4 and 6, and update goals and
remaining games for teams 4, 6, 14 and 225.

These controlled outcomes show an interaction: either ratings or results alone
loses Palmeiras/15th; Flamengo/13th and Palmeiras/16th disappear only when both
changes are applied. This is specific to the tested seed and request. It does
not prove that the true probabilities decreased or became zero.

### Which calculation fails

- **Flamengo/13th:** A publishes through a 54-branch complete tree, B through
  51 branches, and D through 54 branches. Their MAIN/CHECK ESS values are
  16.78/10.66, 18.96/14.01 and 10.35/9.55. C instead finds no target-rank event
  in its 1,300 complete-tree pilot draws and skips its final pair. The native
  pilot also has no hits. An outcome being reachable does not ensure a pilot
  will find the required goals and rival standings.
- **Palmeiras/16th:** the complete construction exists in every arm, with 18
  branches in A/D and 17 in B/C. A and D use 25,000 final MAIN draws, and B
  uses 30,000. C uses 25,000, finds 1,159 qualifying seasons, but one contributes
  64.05% of the estimated probability. Its ESS is only 2.286, below the required
  4; maximum contribution share exceeds 0.35 and batch gap 1.715 exceeds 1.5.
  CHECK is skipped. B had ESS 13.59 and a largest contribution of 16.64%, and
  passed its independent check. C's rejected diagnostic `3.242e-40` is still at
  the earlier estimates' scale; the issue is evidence quality, not zero hits.
- **Palmeiras/15th:** A's family fallback publishes after MAIN/CHECK ESS
  6.86/16.67 and probabilities near `1.000e-33`/`5.302e-34`. B and D do not
  publish this fallback for the cell. C's native MAIN passes its own gate, but
  CHECK fails both ESS and agreement, as detailed above. The late family
  eligibility helper requires an originally paired job whose native MAIN
  itself is not publishable; it therefore does not rescue confirmation-only
  failures. This is a specific allocation/eligibility limitation. Construction
  and overflow fallback limits also prevent rescue, as described above.

Two fewer simulated games change random-number consumption and rival constraints.
Updated means change proposal weights, pilot hits, learned families and budget
allocation. A fixed seed guarantees reproducibility of an identical request;
it does not guarantee preservation of estimates after changing that request.
Smaller fixture counts therefore do not guarantee better coverage from this
finite adaptive portfolio. Improving confirmation-failure eligibility and
pilot allocation is a more direct response than treating these zeros as new
impossibility proofs or publishing the rejected raw estimates.
