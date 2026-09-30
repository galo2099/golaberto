# Native Rust position reachability: research and paired experiments

## Result and recommendation

The best measured configuration replaces unproductive maximum-points constructive
queries with minimum-points queries and publishes their verified seasons **after
probability estimation**. A shared-fixture capacity cut also strengthens early
impossibility proofs. All improvements are generic across teams and ranks.

Over ten fixed seeds and five snapshots it found **186 additional reachable
cell-runs**, covering **26 distinct snapshot/team/rank cells** (24 when deduplicated
by group rather than snapshot), and **10 additional impossible cell-runs**, all
for group 16498 / team 17 / position 15. There were **no lost known reachability
results, no lost nonzero cells, and no changed probabilities** in the final 50
paired requests. Witnesses were not used as probability estimates.

Total client HTTP request time fell **2.00%**, from 27.247 s to 26.703 s. Total
native CPU time fell **2.59%**. These are aggregate measurements, not a guarantee
for every request. Dense control group 16983 increased **0.434% in median paired
latency** and **0.510% in aggregate**; this increase is explicitly reported.
No additional latency allowance was assumed. Four workers and the existing
node/candidate/sampling ceilings were retained.

**Recommendation:** prefer the `deferred` experimental configuration over the
joint replacement, immediate witness publication, and neighborhood reordering.
It materially improves reachability while preserving probability coverage and
meeting the aggregate latency constraint on these inputs. Keep the default
unchanged for this research change; the small dense-control increase and
remaining host-specific variance deserve deployment evaluation. No commit or
push was performed.

This resolves uncertainty about reachability, **not the remaining zero estimates**.
The count of reachable cells without estimates rises because previously undecided
zeros now have verified witnesses. Turning these witnesses into new probabilities
requires a separate sampler experiment within the same full-request budget.

## Baseline and measurement protocol

Baseline: current Rust commit `5821a358002718b0979a3ebd76e9ec5345ebb46e`,
branch `codex/rare-position-rust`, frozen before edits. No Go timing or historical
one-second budget was used. Required README/report/modules, production sorter,
and witness/probability integration were read before implementation. AGENTS
instructions were followed. This task changed no Rails code or database data.

| Frozen reference input | Remaining fixtures | Role |
|---|---:|---|
| 16498 / 44eabb47 | 103 | Many unresolved rare ranks |
| 16653 / earlier 2d1c1d6f | 90 | Sparse rare ranks |
| 16653 / current 71d4fea8 | 85 | Sparse rare ranks |
| 16982 / 9327edcd | 330 | Dense control |
| 16983 / 43969b02 | 311 | Additional dense control |

These are **five snapshots of four groups**, each with 20 teams / 400 cells.
Requests come from `reference/2026-09-30-hundredfold/inputs`; every paired
measurement records its semantic SHA-256 fingerprint. Dense controls already
have all 400 probabilities positive, so they cannot gain rare-rank coverage.
The golden reference is partial, combining MC/IS and certified impossibilities;
provisional/undecided entries are excluded from probability scoring.

Environment: ARM64 macOS, Rust/Cargo 1.93.1, existing thin-LTO release profile,
locked dependencies, no new solver dependency. The harness clears inherited
`RARE_POSITION_*` and `RUST_ODDS_*` flags, uses identical requests/seeds, alternates
run order, and runs requests **sequentially with four workers total**. No concurrent
baseline/candidate estimates, extra search pool, or concurrent compile during
measurement. Final ten seeds: 801,804,808,817,911,802,805,809,818,912.

In total: **450 paired requests across 22 configuration names**, plus 25 initial
baseline profiles: **925 complete calculations**. Experimental code evolved
between batches. Earlier minimum-direction batches had an overpruning bug and
are explicitly preliminary; the decisive result is the corrected `deferred-http`
batch. The packaged JSON records each batch's implementation version.

CLI timings include process launch, input reading, calculation, encoding and
output. Final latency measurements use the complete production HTTP adapter:
POST submission to full response receipt, normal logging, experiment tracing off.
Listener startup is separate (5.0–11.0 ms in the final batch). A new listener is
used for each paired calculation; repeated requests on a warm long-lived server
were not benchmarked. Diagnostic stage/node profiling is separate from this gate.

CPU is child user+system time from `getrusage`, including worker threads. HTTP
CPU/lifetime includes listener startup, health and shutdown. Median average CPU
usage was about 2.1–2.7 cores. This is average utilization, not a claim of CPU
pinning; the implementation's four-worker ceiling is unchanged.

[Packaged paired measurements](results/2026-09-30-rust-reachability.json) contain
flags, seeds, fingerprints, binary hashes, classifications, times, CPU and partial
reference comparisons. Raw responses/logs remain at
`/private/tmp/golaberto-rust-reachability-2026-09-30/`.

## Existing limits and measured costs

| Work | Current default ceiling | Existing mechanism |
|---|---|---|
| Constructive witnesses | 100 nodes/cell, 3,200 total | Maximum target points, exempt rivals, domain/slack branching, actual sorter |
| Joint cap proof | 500 nodes/cell, 10,000 total | Shared fixtures, propagation, exemption recursion and query-local cache |
| Joint floor proof | Same, separately per direction | Negated point model |
| Initial neighborhood | 10,000 candidates | Up to two changed fixtures |
| Later walks | Up to two × 4,000 candidates | Reuse verified season outcomes |
| Point-tilt reuse | Another 4,000 candidates | Cross-team pilot witnesses and selected proposals |

Initial 25-request Rust profiling medians, milliseconds:

| Input | Estimator | Early proofs | Neighborhood + constructor | Walk | Point tilt | Extra conditional | Domain rescue |
|---|---:|---:|---:|---:|---:|---:|---:|
| 16498 | 899.7 | 0.335 | 13.581 | 6.880 | 363.028 | 159.142 | 4.495 |
| 16653 earlier | 762.5 | 0.151 | 7.165 | 1.848 | 261.226 | 205.305 | 24.566 |
| 16653 current | 737.2 | 0.188 | 11.629 | 2.457 | 217.572 | 185.805 | 51.522 |
| 16982 | 315.1 | 0.012 | 0 | 0 | 0 | 0 | 0 |
| 16983 | 302.7 | 0.013 | 0 | 0 | 0 | 0 | 0 |

Most cost is probability estimation. Initial setup medians were 0.7–0.9 ms for
sparse inputs and 2.6–2.7 ms for controls. Parent stages include child stages;
never add both. Independent medians need not sum to full-request medians.

`domains.rs` already applies fixed-target cardinality propagation, applicable
wins, selected-pattern caching, forced-fixture compaction and strict-prefix
fixture omission. Witness reuse already exists across cells and teams. The gap
was constructive direction and reuse timing, not absence of propagation.
Most stored witnesses retain W/D/L outcomes rather than actual goal margins;
canonical goal reconstruction can lose ranks that require particular margins.

## Successful native changes

### 1. Necessary rank screen and minimum-points construction

Fix only the existing maximum-points target pattern, then compute necessary rank
bounds under shared legal fixture outcomes. Count rivals definitely above and
below the target. For zero-based rank `r`, require:

```
definitely_ahead <= r
definitely_below <= n - 1 - r
```

Points and wins are packed into an order-preserving prefix **only when wins
immediately follows points** in the phase order. With `pt,gd,w,...`, relax on
points alone; the production sorter checks all later keys.

If the maximum pattern cannot realize the rank, spend that query's existing
quota on a minimum-points pattern that survives the screen. Otherwise retain
the original maximum search. If both patterns fail, stay undecided: intermediate
point totals might work. No particular team or position is special-cased.

For the minimum search, negated current points are insufficient for selecting
mandatory exemptions: future results can lower that negated total. Use attainable
**final** bounds, omit rivals that can never exceed the cap, and try the full
permitted exemption cardinality first. This branch ordering produced substantially
more witnesses within the same 100-node query budget.

Corrected diagnostic seed-808 constructor costs, including the screen:

| Input | Nodes | Deferred verified seasons | Constructor ms |
|---|---:|---:|---:|
| 16498 | 2,573 | 17 | 4.228 |
| 16653 earlier | 1,211 | 8 | 2.028 |
| 16653 current | 1,751 | 11 | 3.819 |

These are diagnostic measurements, not the final paired latency gate. No
constructor exceeds the existing 3,200-node ceiling.

### 2. Publish additional witnesses after probability estimation

A proof label affects proved/undecided proposal priorities. With the corrected
minimum search, immediate publication at seed 808 lost two nonzero estimates
in 16498, two in earlier 16653, and one in current 16653, despite some gains.
That variant is rejected.

`deferred` retains original maximum-path witness publication and seeds. It holds
new minimum-path seasons until point tilt, walks, peers and domain sampling finish,
then verifies/reuses their complete ranks with the production sorter. It only
marks zero, undecided cells as `reachable_by_construction`; it assigns no mass.
Thus new reachability information does not reshuffle existing probability work.
The final 50 paired probability matrices are bit-identical to baseline.

### 3. Shared-fixture capacity cut and conservative work recycling

For each capped team, let `L_i` be its independent attainable minimum and
`s_i = cap - L_i`. For an internal fixture, shared outcomes incur:

```
d_g = min(home_gain + away_gain) - min(home_gain) - min(away_gain)
```

For any bounded subset `S`, necessarily `sum(s_i) >= sum(d_g)` over its internal
fixtures. Violation proves infeasibility. Check every prefix after ordering teams
by slack; this is necessary subset separation, not all-subset enumeration.
For ordinary points, both teams cannot receive zero from a free fixture, creating
two points of joint minimum demand. The same argument works with negated gains
for floor proofs.

`AGGREGATE_CUTS=early` proves 16498 / team 17 / position 15 impossible. Applying
the cut throughout construction added cost without useful coverage.

A new proof would normally grant 1,000 extra recycled probability draws. The
`PROOF_RECYCLE_CREDIT=legacy` setting instead checks eligibility using the original
relaxation **within the remaining 500-node cell / 10,000-node direction allowance**.
A stronger new proof does not automatically increase sampler work. Final recycled
draw counts and point-tilt work are identical to baseline in all 50 pairs.

## Alternatives and rejected reallocations

A generic native joint solver was implemented with one shared W/D/L variable per
fixture, variable target points/wins, rank-cardinality propagation, minimum-domain
and target-incident branching, query-local conflict caching, and strict-prefix
fixture decomposition. Complete seasons require the production sorter.
Only infeasible relaxation branches may become no-goods; failed goal assignments
and exhausted budgets remain unknown.

At 16498 / seed 808, full joint construction took **34.307 ms**, with 2,427 nodes,
348 sorter checks, **0.003 ms setup** and **zero cache hits**. Setup was cheap;
branching/propagation/verification was costly. Replacing the original constructor
lost existing results and probability estimates, so it is not recommended.

Initial seed-808 screen, totals across five snapshots:

| Strategy | New reachable cell-runs | New impossible | Lost known reachability | Nonzero gains / losses | Decision |
|---|---:|---:|---:|---:|---|
| Reuse constructive seasons only | 0 | 0 | 0 | 0 / 0 | No added coverage |
| Dual direction by rank cardinality | 1 | 0 | 0 | 1 / 1 | Reject regression |
| Singles before pairs | 1 | 0 | 0 | 0 / 0 | Follow-up regressed |
| Spread fixture pairs | 1 | 0 | 0 | 0 / 0 | No established improvement |
| Capacity cut throughout | 0 | 1 | 0 | 0 / 0 | Restrict to early proofs |
| Full joint constructor | 9 | 0 | 8 | 4 / 7 | Reject replacement |
| Joint replaces initial neighborhood | 10 | 0 | 9 | 3 / 6 | Reject replacement |
| Dual replaces both walks | 0 | 0 | 10 | 0 / 0 | Reject replacement |
| One walk + small joint tail | 0 | 0 | 3 | 0 / 0 | Reject replacement |
| Early capacity cut + breadth | 1 | 1 | 0 | 0 / 0 | Follow-up loses a witness |

Five-seed breadth follow-up gained two witness cell-runs and lost one: team 5 /
position 17 in 16498 gained at 808 and 911, lost at 804. Small 200/400-node joint
tails and a root-first query sweep found no further cells; replacing walks lost
proofs. These prototypes are not wall-time guarantees and are not recommended.

**Preliminary bug:** earlier `adaptive`, tail and `budgeted` batches used negated
current points for mandatory exemptions. This overpruned feasible minimum
patterns, so their low node counts and near-flat timings do not describe the
corrected algorithm. It produced missed witnesses, not false impossibility
certificates. Final-bound and cardinality corrections are covered by a regression
test. `minimum-corrected` compares corrected immediate, deferred and skip-only
versions. Only `deferred-http` is the final adoption evidence.

Across all measured variants, no previously positive/known-reachable cell became
certified impossible. Snapshot checks and exhaustive tests supplement the proof
arguments; they do not establish safety for every arbitrary input.

## Final paired results

### Reachability gains

Ten seeds per snapshot. A cell-run counts a team/rank result once per seed;
distinct snapshot cells do not count repeats.

| Input | New reachable cell-runs | Distinct snapshot cells | New impossible cell-runs |
|---|---:|---:|---:|
| 16498-44eabb47 | 134 | 17 | 10 |
| 16653-2d1c1d6f | 27 | 6 | 0 |
| 16653-71d4fea8 | 25 | 3 | 0 |
| 16982-9327edcd | 0 | 0 | 0 |
| 16983-43969b02 | 0 | 0 | 0 |

All 50 pairs: zero lost known proofs, zero nonzero gains/losses, zero probability
differences, zero proof conflicts. The partial golden reference's RMSE and
probability coverage are unchanged because the probability matrix is unchanged.

New reachable cells across the ten seeds:

- 16498: team 5 / 17–20; team 8 / 18–20; team 15 / 20; team 16 / 11–14;
  team 17 / 9–13.
- 16653 earlier: team 12 / 18; team 22 / 17–18; team 70 / 18; team 2064 / 17–18.
- 16653 current: team 22 / 17; team 70 / 16; team 2064 / 18.

### Complete HTTP request latency

Milliseconds, normal logging. Paired change is the median of individual ratios,
not the ratio of the two medians. Summed change compares aggregate time per input.

| Input | Baseline median | Candidate median | Median paired change | Summed change |
|---|---:|---:|---:|---:|
| 16498-44eabb47 | 826.128 | 830.862 | -2.257% | -2.624% |
| 16653-2d1c1d6f | 656.853 | 640.599 | -2.053% | -2.156% |
| 16653-71d4fea8 | 676.312 | 668.742 | -3.454% | -2.845% |
| 16982-9327edcd | 289.955 | 289.981 | -0.391% | -0.375% |
| 16983-43969b02 | 284.354 | 286.435 | +0.434% | +0.510% |

The control increase is retained: 16983 **+0.434% paired / +0.510% summed**.
Individual paired changes ranged from −9.57% to +3.14%; those increases
are not treated as an available allowance. Summed full-request time
is **−2.00% overall**; this supports the candidate on these inputs without
assuming any new allowance. It does not establish a host-independent non-increase
for every group or a tail-latency guarantee.

### Reachability cost, setup and CPU

Reachability stage cost below sums early proofs, initial neighborhood/construction,
later walk and **deferred witness publication**, then takes the median. Embedded
point-tilt reuse stays inside the unchanged point-tilt stage. No omitted deferred
work and no double-counting of parent/child stages.

| Input | Baseline reachability ms | Candidate reachability ms |
|---|---:|---:|
| 16498-44eabb47 | 19.957 | 19.772 |
| 16653-2d1c1d6f | 8.636 | 8.084 |
| 16653-71d4fea8 | 13.343 | 12.265 |
| 16982-9327edcd | 0.007 | 0.014 |
| 16983-43969b02 | 0.011 | 0.013 |

Final sparse model-setup medians: 0.67–0.81 ms in both implementations; controls
2.44–2.58 ms. This includes the normal model setup, not external solver startup.
The corrected screen and deferred verification fit by replacing wasted queries,
not by shrinking sample ceilings or claiming an additional millisecond budget.

| Input | Baseline median CPU ms | Candidate median CPU ms |
|---|---:|---:|
| 16498-44eabb47 | 1943.3 | 1878.1 |
| 16653-2d1c1d6f | 1575.8 | 1505.9 |
| 16653-71d4fea8 | 1842.1 | 1799.9 |
| 16982-9327edcd | 636.1 | 638.5 |
| 16983-43969b02 | 622.5 | 633.4 |

Total CPU: **66.710 → 64.983 seconds (−2.59%)**, unchanged four-worker ceiling.
Control 16983's CPU median also increased; no uniform per-group speedup is claimed.

### Remaining zeros and coverage per time

Mean counts per request over ten seeds. Zero probability counts remain unchanged.

| Input | Impossible zeros | Reachable zeros without estimates | Undecided zeros |
|---|---:|---:|---:|
| 16498-44eabb47 | 14 → 15 | 5.4 → 18.8 | 18.4 → 4.0 |
| 16653-2d1c1d6f | 43 → 43 | 8.7 → 11.4 | 2.8 → 0.1 |
| 16653-71d4fea8 | 48 → 48 | 8.6 → 11.1 | 3.5 → 1.0 |
| 16982-9327edcd | 0 → 0 | 0.0 → 0.0 | 0.0 → 0.0 |
| 16983-43969b02 | 0 → 0 | 0.0 → 0.0 | 0.0 → 0.0 |

16498 finishes with exactly four undecideds in every seed: team 16 / positions
15,16,17 and team 17 / position 14. Current 16653 finishes with one: team 70 /
position 17. Earlier 16653 has none in nine seeds and team 95 / position 5 in seed 818. These
are bounded-search outcomes, not claims that those cells are impossible.

| Input | Baseline known cells / request-second | Candidate known cells / request-second |
|---|---:|---:|
| 16498-44eabb47 | 475.8 | 507.1 |
| 16653-2d1c1d6f | 605.1 | 622.7 |
| 16653-71d4fea8 | 575.2 | 595.8 |
| 16982-9327edcd | 1374.4 | 1379.6 |
| 16983-43969b02 | 1398.6 | 1391.5 |

This metric counts `400 - undecided` divided by summed complete-request seconds,
including already positive/certified cells. Additionally, 196 new certificate
cell-runs over 26.703 candidate request-seconds give **7.34 additional certificates
per full-request second** across the final suite. It is not a probability-estimate
coverage metric and not a rate for an isolated solver call.

## Correctness assumptions and tests

- Infeasible shared-fixture relaxations prove impossibility; feasible intervals
  or models do not prove reachability.
- One W/D/L outcome belongs to each remaining game and simultaneously determines
  both teams' points and applicable wins. No independent incompatible rival
  results are accepted.
- Actual phase sorting rules verify complete witnesses, including later
  tiebreakers and the existing translated sorter/random comparison semantics.
- Canonical 1–0/0–0 goals are representative completions, not a goal cap. A failed
  completion stays unknown. A team losing 0–5 can win the return 6–0; failure of
  its canonical 1–0 return does not exclude the win pattern.
- Numeric zero mass in a truncated score PMF cannot refute unrestricted soccer
  outcomes. The new joint proof solver starts with all three W/D/L values. Its
  proposal-only extreme screen matches existing numerical support and only
  skips that proposal; it never proves the whole rank impossible.
- Arbitrary score caps, failed goal assignment, or exhausted budgets never
  produce impossibility. No intermediate ranks are inferred from endpoints.
- No-goods are query-local and learned only from infeasible relaxations. No
  unrestricted claim is made about finite representative goal searches.
- Deferred seasons establish reachability only; they do not assign probabilities,
  replace zeros with upper bounds, or change estimator acceptance gates.

**23 Rust tests passed (2 unit + 21 integration).** Added tests exhaustively check
729 complete four-team W/D/L assignments against rank propagation; preserve
feasible cap/floor assignments; verify wins ordering, aggregate cuts, reordered
neighborhoods and season reuse; and exercise goal failure, budget exhaustion,
minimum final-bound selection/defer behavior and tiny-PMF numerical zeros.
All reference fixtures' original numerical outcome masks equal the unrestricted
joint masks, verifying that the support correction does not alter those measured
inputs. Formatting, `cargo check` and `git diff --check` passed.

## Scientific context


[**Bernholt et al. (1999)**](https://image.informatik.htw-aalen.de/~thierauf/TI2/Netz/football.pdf)
prove football champion elimination NP-complete under the three-point rule,
even with three remaining games per team. This motivates bounded propagation
and reuse, not a promise of resolving every matrix cell within a fixed budget.
It does not imply that each particular unresolved cell is difficult.

[**Gotzes & Hoppmann (2020 online / 2022 journal)**](https://link.springer.com/article/10.1007/s12351-020-00546-w)
report strong practical SCIP results: 14,688 query instances, mean about 0.024 s,
maximum 11.84 s, and 99.11% solved at the root. Their tied-on-points ordering is
arbitrary. Their hardware, per-query optimization, and relaxed tie rules are not
this full 400-cell request with the production sorter; the timings cannot be
used as an available production budget.

[**Duque, Arbelaez & Díaz (2019)**](https://www.aimsciences.org/article/doi/10.3934/jimo.2018109)
compare CP/MIP soccer models and complementary solver performance. Their results
support testing both approaches on the actual problem; they establish no
universal best solver for our fixture/tiebreaker model.

[**Perron, Didier & Gay (2023)**](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.CP.2023.3)
describe CP-SAT's propagation, clause learning, LP relaxation/cuts and portfolio
search. A small native query-local cache implements only a small part of that
architecture; it should not be represented as a CP-SAT substitute.

[**Rosenberg et al. (2026), NHL clinching**](https://arxiv.org/html/2605.13142v2)
provide a useful separation of outcomes, tiebreaker checks, goal assignment and
sound no-goods. Their practical goal cap of 99 is not an unrestricted soccer
impossibility proof. Our representative-goal failures stay undecided instead.

No external reference solver was installed or launched. A future offline solver
could identify useful conflicts and goal witnesses; production startup/model
construction/repeated calls would need separate measurement. Further native work could preserve actual goal seasons for reuse or separate
stronger cheap subset cuts. These
next ideas were not tested here and are not adoption recommendations.


## Reproduction

Build the frozen baseline without changing the working checkout or branch:

```sh
reach_base=$(mktemp -d /tmp/golaberto-reach-base.XXXXXX)
git archive 5821a358002718b0979a3ebd76e9ec5345ebb46e odds-rust | tar -x -C "$reach_base"
cargo build --release --locked --manifest-path "$reach_base/odds-rust/Cargo.toml"
cargo build --release --locked --manifest-path odds-rust/Cargo.toml
```

Final normal-logging HTTP comparison; ephemeral localhost listeners are stopped
by the harness. Use `--http --no-trace` for full-request latency, omit `--no-trace`
for diagnostic node/stage counts, omit `--http` for CLI process-lifetime timings.

```sh
python3 experiments/rare_positions/compare_rust_reachability.py \
  --baseline "$reach_base/odds-rust/target/release/golaberto-odds" \
  --candidate odds-rust/target/release/golaberto-odds \
  --reference experiments/rare_positions/reference/2026-09-30-hundredfold/reference.json \
  --output /tmp/rust-reach-http \
  --seeds 801,804,808,817,911,802,805,809,818,912 \
  --variants deferred --http --no-trace
```

Corrected direction/publication screen:

```sh
python3 experiments/rare_positions/compare_rust_reachability.py \
  --baseline "$reach_base/odds-rust/target/release/golaberto-odds" \
  --candidate odds-rust/target/release/golaberto-odds \
  --reference experiments/rare_positions/reference/2026-09-30-hundredfold/reference.json \
  --output /tmp/rust-reach-directions --seeds 808 \
  --variants deferred,skip,budgeted
```

The harness `VARIANTS` lists other experiments; packaged rows supply every tested
setting/seed. Earlier screening can change with the documented bug fix; its saved
measurements are preliminary, not reproducible claims about the corrected code.

Best candidate directly (additional experimental flags, default rare pipeline):

```sh
RUST_ODDS_WITNESS_MODE=deferred \
RUST_ODDS_AGGREGATE_CUTS=early \
RUST_ODDS_PROOF_RECYCLE_CREDIT=legacy \
odds-rust/target/release/golaberto-odds estimate \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  /tmp/rust-reach-16498.json 808 4
```

Do not enable neighbor-order, joint allocation or tracing flags for normal timing.
Rust already uses the matched-point pipeline. Production defaults are unchanged.

```sh
cargo test --offline --release --manifest-path odds-rust/Cargo.toml
cargo check --offline --manifest-path odds-rust/Cargo.toml
cargo fmt --manifest-path odds-rust/Cargo.toml -- --check
git diff --check
```

Implementation: `odds-rust/src/proof.rs`, `reachability.rs`, `neighbors.rs`,
`search.rs`, integration tests and the Python comparison harness. JSON response
shapes, Rails callers, database and sampler acceptance gates are unchanged.

## Follow-up: four unresolved 16498 cells

Read-only DB lookup identifies team 16 as Palmeiras-SP and team 17 as Flamengo-RJ.
In the frozen snapshot Palmeiras is second with 57 points / 16 wins; Flamengo
is first with 60 points / 18 wins. Both have ten remaining fixtures.

Isolated diagnostics of the unchanged constructor, with 100 nodes per query:

| Cell | Required rivals ahead | Minimum-pattern nodes | Complete canonical seasons | Actual ranks found |
|---|---:|---:|---:|---|
| Palmeiras / 15 | 14 | 100 | 12 | 13 |
| Palmeiras / 16 | 15 | 100 | 36 | 10 |
| Palmeiras / 17 | 16 | 100 | 0 | None; 46 propagated contradictions |
| Flamengo / 14 | 13 | 100 | 27 | 8 |

Maximum-point patterns fail the necessary screen; minimum-point patterns pass.
The full seed-808 constructor used 2,573 nodes, below its total 3,200 ceiling.

Palmeiras/15's first season had 12 rivals strictly above 57 points and two tied
on 57 points / 16 wins (Botafogo and Santos). Palmeiras/16's first had nine
strictly above and six tied on both points and wins. Canonical goal margins left
Palmeiras ahead of those tied rivals. Failed canonical scores cannot refute their
W/D/L patterns; the constructor does not solve goal assignment.

Flamengo/14's first season had seven strictly above 60 points and six tied on 60.
Three tied rivals had 17 wins and three had 18, versus Flamengo's 18. The inner
constructor propagates points caps/floors, so those ties pass its relaxation;
wins already prevent three overtaking Flamengo, and canonical later tiebreakers
left all six below. The outer extreme-pattern screen uses applicable wins, but
its necessary bounds do not guarantee a complete exact-rank assignment.

Early floor proofs returned feasible after 13 nodes for each Palmeiras query,
21 for Flamengo; cap proofs returned feasible after one node. They did not exhaust
500 nodes: propagation merely failed to refute an exempt-rival configuration.
Raising that node limit alone would not strengthen these feasible relaxations.
All four remain undecided. These diagnostics establish neither unrestricted
impossibility nor witnesses for the requested ranks. The instrumented copy and
outputs are in the temporary experiment directory; production code is unchanged.

### Goal-difference completion proves Palmeiras / 15 reachable

The user identified the missing completion: a projected win/loss can have an
arbitrarily large legal margin without changing points or wins. Reusing the
first minimum-points W/D/L assignment above, change only game 364084,
Palmeiras–Bahia, from 0–1 to 0–100. Palmeiras's final goal difference changes
from +16 to −83; its 57 points and 16 wins are unchanged. Santos (+3) and
Botafogo (+1), also with 57 points and 16 wins, both move ahead. Bahia already
has more points (58), and the twelve strictly higher-point teams remain ahead.

All final campaigns were independently rebuilt from the complete fixture scores
and checked with `Model::standings` / the production sorter: **Palmeiras is 15th**.
Every W/D/L outcome is unchanged. No extra seasons were sampled and no probability
was assigned. The single changed game's winner gains exactly the goal difference
lost by Palmeiras, so both endpoints are accounted for.

[Complete verified fixture assignment and final standings](results/2026-09-30-palmeiras-15-goal-witness.json).
This supplies an offline reachability certificate for one of the four cells that
the current estimator leaves undecided; the production algorithm and measured
paired results above are unchanged. The 100-goal score is a witness choice, not
an impossibility cap or a probability-estimation proposal.
