# Rust odds and ratings service

Native Rust implementation of the current Go **matched point pool** estimator.
It includes the initial game-importance scout, 100,000-season pool, exact point
PMFs, uncertainty estimates, cap/floor proofs, conditional sampling, rank-directed
importance sampling, neighborhood and constructive witnesses, point-tilt/gap
rescues, cross-team witness reuse, directional/constraint peer rescues, propagated
domains, reduced fixtures, and final matrix reconciliation.

The executable does not call Go and does not read the golden probabilities.
Go is used only by the offline comparison harness. The default coverage profile
adds a bounded portfolio for remaining zero cells, targeting order-of-magnitude
estimates. Earlier sampling stages and acceptance gates are preserved; the
additional portfolio uses deterministic work limits and the documented
rough-estimate gates below.
`RUST_ODDS_RARE_TAIL=0` disables the additional portfolio.

The coverage profile also enables the bounded **family fallback** by default.
It learns rival points/wins patterns from failed native confirmation batches,
then fits proposals only for cells still at zero after all native searches.
Successful native estimates are preserved. No enabling flags are needed;
`RUST_ODDS_FAMILY_FALLBACK=0` disables this fallback. Rebuild and restart the
Rust service to apply the change.

The coverage profile enables the **lean target-overflow tree fallback** by
default. It handles remaining zeros whose target-result enumeration exceeds
the earlier 64-path limit. Its five-draw pilots and two-draw leaf minimum use
the remaining original confirmation budget, after native and family searches.
No enabling flags are needed; `RUST_ODDS_TARGET_OVERFLOW_TREE=0` disables it.
Rebuild and restart the Rust service to apply the default.

The native service also replaces the active Go `/spi`, `/eval`, and
`/historic_ratings` endpoints and integrates the active `stats` `/player_ratings`
command. It does not run or proxy Go or the stats executable. The player formulas
are shared with the standalone stats service through `stats/core`; one release
binary now serves all five application endpoints on port 6577.

## Build and run

Use a recent stable Rust toolchain. The locked ICU dependencies require Rust
**1.88 or newer**, and the service was verified with Rust/Cargo **1.93.1**.
The application uses edition 2021, but some dependencies use edition 2024.
An older Cargo can fail with `feature edition2024 is required` before compiling.

With [rustup](https://rust-lang.org/tools/install), update the stable toolchain
and make its Cargo available in the current shell:

```sh
rustup update stable
source "$HOME/.cargo/env"
cargo +stable build --release --locked --manifest-path odds-rust/Cargo.toml
```

`+stable` explicitly selects the updated toolchain if a directory override or
older default selects another version. If rustup is not installed, install it
using the linked Rust instructions first. Nightly is not required.

From the repository root:

```sh
cargo build --release --locked --manifest-path odds-rust/Cargo.toml
odds-rust/target/release/golaberto-odds estimate \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16653-71d4fea8.json \
  /tmp/rust-odds.json 808 4
```

Arguments are request path, output path, seed, and worker count (1–4).
The default seed is 808 for reproducible CLI experiments. Every full request
includes the 20,000-season game-importance scout and 100,000-season matched pool.
Timings go to stderr; the response goes to the output file.

```sh
odds-rust/target/release/golaberto-odds bench REQUEST.json 10
RUST_ODDS_PROFILE=1 odds-rust/target/release/golaberto-odds estimate \
  REQUEST.json /tmp/odds.json 808 4
```

`bench` repeats complete requests and reports a median.

## Odds memory

Completed conditioning DP layers use compact immutable storage with their
original iteration order. One-team layers have direct point-total lookups;
larger layers use a compact index. Obsolete events are released before deeper
search allocates its replacements. Sampling budgets and estimates are unchanged.

The HTTP service uses one calculation worker with four estimator cores, four
body readers and a bounded request queue. Response I/O stays on the readers,
so slow uploads or downloads do not occupy the calculator.

For an allocation trace of a full four-worker calculation:

```sh
RUST_ODDS_PROFILE=1 cargo run --release --locked \
  --manifest-path odds-rust/Cargo.toml --example profile_memory -- REQUEST.json \
  > /tmp/odds-heap.csv 2> /tmp/odds-heap-stages.log
```

The optional diagnostic counts requested live/peak Rust heap bytes and emits a
CSV trace. It adds allocation-counter overhead; benchmark the ordinary service
binary for latency and process RSS. Measurements and reproduction are documented
in `experiments/rare_positions/2026-09-30-rust-odds-memory.md`.

## Timing logs

Request and stage logs are enabled by default and written as JSON lines to
stderr. Each record includes a Unix timestamp in milliseconds, a request ID,
and cumulative request wall time. Estimator records also include the group
and seed, making a slow run reproducible.

The stages include setup, the initial scout, pool MC, point PMFs, the matched
matrix, jackknife uncertainty, point proofs, initial conditioning, guided and
extra search, witnesses, point tilts, neighborhood walk, peer/domain rescues,
deferred joint-cap publication, and reconciliation. `rust_odds_joint_caps`
records include the seed, setup/sampling times, acceptance and confirmation
diagnostics, and joint/confirmation/ordinary draw counts. `rust_odds_start`
reports whether joint-cap conditioning and its broader fallback are enabled. Stage records report `elapsed_ms`; enclosing `pool` and
`search` totals include their sub-stages, so do not add parent and child times.
The completion record reports positive/zero cells, certified impossible zeros,
reachable zeros without estimates, unresolved zeros, and modeled pool/search
work. Work units are not a count of ordinary MC seasons and exclude the
initial scout and constraint proof nodes.

HTTP additionally reports request reading/JSON decoding, response encoding,
writing, status, byte counts, and `http_total_ms`. This total begins when the
HTTP server dispatches the parsed request and includes body reading, calculation
queue time, and response writing. It excludes earlier TCP/header parsing time. Estimator
`total_ms` covers calculation and response construction, excluding JSON and
network I/O. All durations are wall time, not summed CPU time across workers.

```sh
odds-rust/target/release/golaberto-odds serve 127.0.0.1:6577 \
  2> /tmp/rust-odds.log
```

Use `RUST_ODDS_LOG=0` to disable these request/stage logs for benchmarks.
The CLI's compact timing summary remains available. `RUST_ODDS_PROFILE=1`
also enables logs, including when the quiet toggle is set.

## HTTP service and Go replacement

### Late family fallback

The family fallback runs after the existing native searches, including complete
branch sampling. It can use at most four learned families per proposal, with a
20% native component preserving support. Training records a prefix of at most
3,000 native MAIN draws, at most 256 positive observations and 128 distinct
family keys. Recording preserves the native random stream and results. When
this prefix supplies no useful family, a separately funded replay can train
the fallback. Fresh validation and independent MAIN/CHECK streams are required
before publication; training hits alone do not become estimates.

Observer work uses audited unused work from the existing rare-tail stage.
Late fitting and sampling use the remaining original confirmation reservation,
without an additional work allowance. Admission and draw counts depend on
deterministic operation costs. A later native estimate or impossibility proof
skips the fallback before fitting, replay or sampling. Every rank uses the same
policy, and the JSON response contract is unchanged.

Startup and `rust_odds_start` logs include the effective `family_fallback`
settings. Detailed confirmation/fallback records report training, admissions,
modeled work, fitting, sample diagnostics and elapsed time. This improves
coverage of tiny events, with the same order-of-magnitude publication gates;
it does not certify their precision. The adopted build measured about 9.4%
more wall time and approximately unchanged CPU time on group 16498 in a short
paired check. Earlier frozen builds measured 8.5–9.8% more CPU and 13–16% more
wall time; these local checks do not establish the same overhead on production.
The user authorized adoption after reviewing that cost. The
[production adoption report](../experiments/rare_positions/2026-10-04-rust-family-production.md)
records the final paired validation and local timing.

`RUST_ODDS_FAMILY_FALLBACK=0` explicitly disables this work, even if the legacy
`RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK=1` is present. The legacy master flag is
accepted only when the production flag is unset. Other family experiment
tuning flags do not change the fixed production policy. The fallback requires
the deterministic operation work model; legacy draw or wall budgets skip it.

### Lean target-overflow tree fallback

The final tree fallback is enabled by default in the coverage profile. It
considers at most four remaining non-impossible zeros with 65–256 target paths.
It retains the complete outcome partition, failed-refinement parents and actual
score tiebreakers. It starts with five pilot draws per leaf, refines three message
proposals, and sizes independent MAIN/CHECK batches to the residual confirmation
bank, with a nominal maximum of 6,000 draws and a minimum of two per leaf.
Successful earlier estimates and the existing publication gates are preserved.

The default uses no reclaimed early draw credit and does not increase the
original confirmation capacity. Startup and request logs expose the effective
`target_overflow_tree` settings; the stage summary reports setup, draw work,
acceptance, budget settlement and timing.

Tree construction requires a residual grant covering its measured root-count
work plus the configured node and guide quotas. Smaller grants skip construction.
This is an admission floor, not a strict setup upper bound: rank-hint work is
measured afterward, and an overrun stops the stage and declines publication.

`RUST_ODDS_TARGET_OVERFLOW_TREE=0` opts out and takes precedence over the legacy
`RUST_ODDS_EXPERIMENT_TARGET_OVERFLOW_TREE` toggle. Existing experimental tuning
knobs remain available for diagnostics; remove them to use the measured defaults.
The rejected early-rank experiment remains disabled by default.

In group 16498/seed 808, this recovered Palmeiras/14th at `7.54e-25` probability
with an independent check of `6.15e-25`, using 46.3% less modeled work than the
earlier complete tree. Six snapshots and four holdout seeds preserved existing
positive estimates; only the default seed gained this cell. Median tree-stage
wall time decreased from 98 to 72 ms in the original experiment. Full-request
measurements remain noisy: the original comparison added 7.1% mean wall/0.4%
CPU; the adoption comparison under local memory pressure added 12.7%/3.1%.
The user authorized default enablement. No extra bank capacity is assumed.
See the [allocation and adoption report](../experiments/rare_positions/2026-10-04-rust-tree-allocation.md).

### Joint rival conditioning

Joint conditioning and its broader fallback are **enabled by default** in the
normal binary. Build and restart the Rust service; no enabling flag is needed.
`RUST_ODDS_JOINT_CAP_CONDITIONING=0` disables both, while
`RUST_ODDS_JOINT_CAP_BROAD=0` retains only the previous exact extreme fast path.

For zeros already selected for an extra batch, the exact fast path first checks
whether extreme target results and a particular ahead set are necessary. When
that gate is not proved, the broader sampler covers **all attainable target
points/wins totals allowed by safe rank bounds**. It favors promising totals
and joint rival caps/floors through a defensive proposal mixture: 10% target-only
conditioning and 90% tilted joint conditioning. Rival caps/floors guide sampling;
they are not treated as requirements of the requested rank.

Shared fixtures are modeled once. Internal outcomes are enumerated within a
20,000-state setup limit, and external streams use exact backward DP masses.
Weights use the density of the **whole mixture**, including its overlap and
soft target tilt, followed by corrected residual-fixture proposal weights. A
small base-distribution component in the residual proposal preserves outcome
support even when a guide rounds to zero.
Scores and actual phase tiebreakers are checked with the production sorter.
The same model handles every rank and both directions without team constants.

Broader attempts replace work in an existing 50,000-draw batch: 1,500 main
plus an independent 1,500-draw confirmation. The two streams forecast how
many ordinary hits the replaced 50,000-draw batch might produce. If the larger
forecast is at least 0.01, retain 43,000 ordinary draws for witness reuse;
otherwise retain 35,000 (46,000 or 38,000 total). This forecast guides allocation
and is logged as `ordinary_hit_forecast`; it is not a probability bound. The exact fast path retains its previous 5,000
main plus 2,000 confirmation draws, with 10,000 ordinary draws when accepted
and 43,000 when rejected. Unsupported models retain 50,000 ordinary draws.
The four ordinary candidates and four workers are retained. Accepted estimates
are published after existing search and witness reuse, before reconciliation.

The model supports ordinary 3/1/0 scoring with points first, and packs wins only
when wins immediately follow points. Unsupported, oversized or numerically
empty models skip this proposal without changing reachability. The logged
`target_mass` is a necessary target-event probability; `joint_normalizer` is
an expectation of the proposal's terminal weight and **is not an upper bound**.
Witnesses alone do not supply probability estimates. Finite samples and the
confirmation diagnostics do not certify tiny-event accuracy.

### Propagated constraints and work transfer

Propagated joint conditioning is enabled by default. It can consider a zero
below the previous `1e-11` allocation cutoff when its complete target outcome
union fits a bounded model. Propagation supplies necessary rival points/wins
bounds and allowed fixture outcomes; forced fixtures are compacted out of the
sampling loop. Ties remain allowed until the production sorter checks scores.
Target cases and shared rival fixtures have exact probability normalizers;
soft case tilts and residual proposals use corrected importance weights.

The setup retains every supported target case or skips the model: at most 64
target patterns, 16 feasible propagated cases, six jointly conditioned rivals,
and 60,000 internal enumeration nodes including retries. Forward/reverse guides
have a four-million-value allocation limit. Every feasible case must force
at least four non-target fixtures. Unsupported or exhausted models retain the
existing search path and cannot establish impossibility.

One extra propagated candidate can be funded from the existing four candidates.
Broad attempts with an ordinary-hit forecast below 0.01 retain 25,000 ordinary
draws instead of 35,000, saving 10,000 per such candidate. Once at least 8,000
draws are saved, allocate 5,000 main and 3,000 independent confirmation draws to
the propagated candidate. Preserve the other candidates' joint attempts and
43,000 ordinary draws for promising candidates. This reduces total draws; setup
and sample costs are measured separately. Long batches run first within each
phase, using at most four workers.

Acceptance gates are unchanged. If confirmation fails, keep any verified
reachability witness and tighten the upper bound to the necessary conditioning
event mass, while retaining a zero estimate. Do not apply binomial zero-hit
bounds to importance samples. This mass is an upper bound, not an estimate.

`RUST_ODDS_JOINT_PROPAGATION=0` or `RUST_ODDS_JOINT_ALLOCATION=legacy` restores
the previous allocation. `rust_odds_start` reports both settings; planning,
allocation and sampler logs include actual setup time, forced fixtures, case
counts, saved draws and independent-check diagnostics. The
[propagated conditioning report](../experiments/rare_positions/2026-10-01-rust-propagated-joint.md)
records the paired results and rejected allocation variants.

The [broad conditioning report](../experiments/rare_positions/2026-09-30-rust-broad-joint.md)
records paired coverage, regressions fixed during tuning, request/stage timings,
CPU usage, correctness tests and reproduction commands. The
[earlier exact-cap report](../experiments/rare_positions/2026-09-30-rust-joint-caps.md)
records the previous Chapecoense/3rd integration.

```sh
RARE_POSITION_RANDOM_SEED=808 \
  odds-rust/target/release/golaberto-odds serve 127.0.0.1:6577
curl -H 'Content-Type: application/json' --data-binary @REQUEST.json \
  http://127.0.0.1:6577/odds
```

`POST /odds` preserves `team_odds`, `game_importance`, and
`rare_position_estimates`. `team_odds[*].Pos` contains percentages; estimate
probabilities contain fractions and rank keys remain zero-based. Rails null
score/bias fields are accepted. `GET /health` is available. HTTP requests run
serially with up to four estimator workers. HTTP defaults to fixed seed `808`,
matching the CLI. `RARE_POSITION_RANDOM_SEED` overrides it; unset, empty, or
invalid values use `808`. The effective seed is logged. Deterministic work budgets are enabled by default, so measured wall time
does not control search admission. The default profile is verified reproducible for the same input, seed, settings,
and binary, including one versus four workers; this does not eliminate sampling
error.

Running the executable without arguments also starts the service at
`127.0.0.1:6577`. For a deployment trial, use an explicit unused port, then
replace the Go process on port 6577 after validation. Do not run both processes
on the same address. The Rails odds, SPI and evaluation call sites already use
6577 and require no request/response changes. The updated Rails player-rating
caller also uses 6577. Deploy the Rails change with the unified service; after
that deployment the separate `stats` process on 6578 can be stopped.

| Endpoint | Request | Response and side effects |
|---|---|---|
| `POST /odds` | Existing group JSON | Existing odds, importance and rare-position metadata |
| `POST /spi` | `games`, `ratings` | Team ID map with `Id`, `Offense`, `Defense`, `Team`; inactive teams are `null` |
| `POST /eval` | `games`, `ratings`, `phases_to_eval` | `rps`, `team_rps`; no database writes |
| `POST /historic_ratings` | `games`, `ratings` | Upserts `historical_ratings`, then returns `ratings`, `offense`, `defense` as empty objects and `dates` as an empty array, matching Go |
| `POST /player_ratings` | No body required | Reads the same four-year category-1 data as stats; atomically updates existing player and appearance ratings, then returns `{"status":"ok"}` |
| `GET /health` | None | `{"status":"ok"}` without a database connection |

Rating requests accept the Rails lowercase field names, Go field names, and
null rating numbers. Games must be chronological and every participating team
must appear in `ratings`. Empty SPI requests return null ratings; evaluation
with no selected games returns HTTP 400. Invalid input returns JSON HTTP 400;
database failures return HTTP 500 and are logged without exposing connection
configuration to the caller. Unknown paths return 404; non-POST methods on
application endpoints return 405.

Content-Length and chunked request bodies are supported, including rating
histories larger than the old adapter's 16 MiB limit. The new body limit is
128 MiB. Four HTTP workers and a bounded queue accept requests; calculation
requests run one at a time with up to four estimator CPU workers. Health checks
are served independently. A full queue returns 503. HTTP totals include queue
time; CLI calculation timing does not.

`/historic_ratings` and `/player_ratings` require MySQL. Set `DATABASE_URL` to a `mysql://` or
Rails-compatible `mysql2://` URL pointing to the same database as Rails.
If unset, the legacy development default
is `mysql://root@127.0.0.1:3306/GolAberto_development`. Production must set this
explicitly. Database credentials belong in the process environment, not the
repository. Historical updates use parameterized batches and a transaction;
the table must use a transactional engine such as InnoDB. Numeric persistence
retains Go's six fractional digits. Player updates retain the existing stats
`f32` formulas, weights, interval boundaries and normalization. The `players`
and `player_games` tables must also use InnoDB for their joint transaction.

`TeamController#historic_ratings` recognizes the service-persisted empty series
and redirects without attempting a second, empty SQL insert. It still accepts
nonempty series from older implementations and checks HTTP failures before
decoding the response.

For read-only comparisons and offline computation:

```sh
odds-rust/target/release/golaberto-odds spi RATINGS_REQUEST.json /tmp/spi.json
odds-rust/target/release/golaberto-odds eval RATINGS_REQUEST.json /tmp/eval.json
odds-rust/target/release/golaberto-odds historic RATINGS_REQUEST.json /tmp/history.json
```

The `historic` CLI exports computed rows and does **not** write MySQL.
The implementation preserves the legacy fitting formulas, convergence policy,
evaluation advantage convention, day-of-month refit policy, four-year history
window and historical sampling schedule.

## Configuration and scope

The Rust executable selects the matched-pool pipeline directly; it does not
require `RARE_POSITION_MATCHED_POINT_POOL=1`. Current default search stages
are enabled. Their existing disable toggles are honored, including
`RARE_POSITION_CONDITIONED_ZERO=0`,
`RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD=0`,
`RARE_POSITION_CONDITIONED_POINT_TILT=0`,
`RARE_POSITION_PROPAGATED_DOMAINS=0`, and the peer/neighborhood/proof toggles.
`RARE_POSITION_WIN_AWARE_RANK` and
`RARE_POSITION_REDUCED_SIMULATION` retain their current meanings.

Historical CEM/diversified-only pipelines and optional million-draw deep
experiments are outside this port. Worker and iteration controls come from
the Rust interface, rather than Go's benchmark-only environment variables.
The four-core benchmark uses the same ten fixed pool batches as parallel Go.
Rust also retains these batch streams with one worker; Go's serial pool uses
a different stream layout. Non-points standings or unsupported point PMFs
fall back to plain MC.

### Reachability defaults and experiments

The native Rust experiments and paired results are documented in
[`2026-09-30-rust-reachability.md`](../experiments/rare_positions/2026-09-30-rust-reachability.md).
The measured deferred configuration and verified goal completion are enabled by
default. No extra flags are required. Sampling acceptance gates are unchanged.

| Configuration flag | Values / effect |
|---|---|
| `RUST_ODDS_REACHABILITY_TRACE=1` | Actual proof nodes, candidate counts, sorter calls and stage cost on stderr |
| `RUST_ODDS_WITNESS_MODE` (default `deferred`) | `deferred`: screen maximum-points patterns, replace rejected queries with minimum-points construction, and publish new witnesses after probability sampling; `adaptive`: publish those witnesses immediately; `skip`: only skip screened maximum patterns; `reuse`: reuse all constructive ranks; `dual`: choose direction by rank cardinality; `joint`: bounded shared-fixture solver |
| `RUST_ODDS_NEIGHBOR_ORDER` | `breadth`: all single changes before pairs; `spread`: also distribute pairs across fixture indices |
| `RUST_ODDS_AGGREGATE_CUTS` (default `early`) | `early`: shared-fixture subset capacity cuts in early proofs; `1`: also use them in construction |
| `RUST_ODDS_DISCRETE_CUTS` (default enabled; `0` disables) | In early floor proofs for standard 3/1/0 scoring without bonus points, check every team coalition for incompatible integer win/draw requirements using one graph cut |
| `RUST_ODDS_RANK_PROOF` (default enabled; `0` disables) | After a conflicting but unresolved point-cohort proof, use the remaining shared 500-node cell / 10,000-node direction quota for packed points/wins fixture branching, explained backjumping, and conflicting-cohort reuse |
| `RUST_ODDS_PROOF_RECYCLE_CREDIT` (default `legacy`) | For strengthened early proofs, check recycling eligibility using the original relaxation, within the remaining cell/direction node budget |
| `RUST_ODDS_GOAL_COMPLETION` (default enabled; `0` disables) | With `deferred` mode, complete failed canonical seasons using independent outside-cohort winning margins and equal-goal additions; verify with the production sorter and publish compact certificates after sampling |
| `RUST_ODDS_REACHABILITY_ALLOCATION` | `replace_neighborhood`, `replace_walk`, `split_walk`, `adaptive_tail`: experimental replacements described in the report |
| `RUST_ODDS_JOINT_ROOT_SWEEP=1` | Visit each unresolved joint query's root before spending the remaining joint node budget |

Alternative witness modes, neighborhood ordering and work reallocations remain
experimental; the report distinguishes useful cuts from rejected reallocations. A
verified witness changes reachability metadata, never assigns a probability.
Reachability tracing also splits neighborhood and constructive stage timing;
normal logs retain the combined `search.witnesses` stage. The comparison harness
supports `--no-trace` for production timing and `--http` to separate complete
HTTP request latency from server startup.

The default measured combination is `RUST_ODDS_WITNESS_MODE=deferred`,
`RUST_ODDS_AGGREGATE_CUTS=early`, and
`RUST_ODDS_PROOF_RECYCLE_CREDIT=legacy`. It preserves the original node,
candidate, sampling and worker ceilings. Minimum construction uses attainable
final bounds in the negated point model. Additional complete seasons are reused
after sampling so they cannot alter proposal priorities. Immediate publication
and joint replacement lost estimates in experiments; see the report for paired
coverage gains, latency increases in the dense control, and remaining undecideds.

The default discrete win/draw proof and paired request measurements are documented in
[`2026-09-30-rust-discrete-cut.md`](../experiments/rare_positions/2026-09-30-rust-discrete-cut.md).
It adds a necessary coalition point-floor inequality to the existing early
joint proof stage and retains its 500-node cell / 10,000-node direction limits.
Passing this relaxation does not establish reachability. The new cut proves
Palmeiras/17th impossible in the frozen group 16498 request, independently of
score margins and later tiebreakers. Legacy recycling eligibility is retained,
so stronger proofs do not add probability draws. Full-request latency variation
and the small measured proof-stage overhead are reported explicitly. The user
authorized enabling it after reviewing the CPU and stage costs. No additional
flag is required; set `RUST_ODDS_DISCRETE_CUTS=0` to disable it.

The default generic explained rank proof is documented in
[`2026-09-30-rust-proof-search.md`](../experiments/rare_positions/2026-09-30-rust-proof-search.md).
It resolves Flamengo/14th in frozen group 16498 within the existing proof
ceilings. It supports both rank-bound directions and uses wins only immediately
after points in the phase ordering. No score caps or numerical outcome-mass
filters are used. Relaxation feasibility and budget exhaustion remain undecided.
No additional flag is required; set `RUST_ODDS_RANK_PROOF=0` to disable it.
Probabilities and sampler acceptance gates are unchanged. Full-request latency
and CPU increases observed in the paired controls are reported explicitly.

The goal-difference / goals-scored completion experiment is documented in
[`2026-09-30-rust-goal-completion.md`](../experiments/rare_positions/2026-09-30-rust-goal-completion.md).
It keeps the existing fixture-search ceilings, tries at most two completions per
cell and sixteen algebraic layouts per completion, and never turns a failed
score completion into an impossibility proof. Stored certificates contain outcome
assignments, fixture adjustments and verified ranks; full score arrays are only
temporary verification data. The comparison harness accepts
`--baseline-variant deferred` to compare against the previous best Rust algorithm.
The final full-request comparison measured +0.24% wall time and +1.25% CPU time;
strict latency non-increase is not established. The user authorized enabling this
measured tradeoff. Set `RUST_ODDS_GOAL_COMPLETION=0` to disable score completion.
To restore the original pre-research pipeline, also set
`RUST_ODDS_WITNESS_MODE=legacy`, `RUST_ODDS_AGGREGATE_CUTS=0` and
`RUST_ODDS_PROOF_RECYCLE_CREDIT=0`.
The proposal's adjacent CDF lookup also shares its row lookup while preserving
the original weight arithmetic and fixed-seed probability values.

Go's random-comparison tiebreaker requires its original sorter: Rust includes
a translated PDQsort rather than passing that comparator to Rust's standard
sort. The Go RNG, sorter, and associated constants retain their BSD attribution
in `third_party/GO_LICENSE`. Floating-point library and map accumulation order
can still change rare proposal trajectories; identical seeds do not guarantee
identical full matrices across languages.

## Verification and comparison

```sh
cargo test --release --locked --manifest-path odds-rust/Cargo.toml
cargo check --locked --manifest-path odds-rust/Cargo.toml
cargo fmt --manifest-path odds-rust/Cargo.toml -- --check

# Explicit DB check: all writes use a connection-local temporary table.
MYSQL_TEST_URL=mysql://root@127.0.0.1:3306/GolAberto_development \
  cargo test --release --locked --manifest-path odds-rust/Cargo.toml \
  --test database -- --ignored

(cd go && GOMAXPROCS=4 go test -c -o /tmp/golaberto-go-baseline.test .)
python3 experiments/rare_positions/compare_rust.py \
  --go /tmp/golaberto-go-baseline.test \
  --rust odds-rust/target/release/golaberto-odds \
  --reference experiments/rare_positions/reference/2026-09-30-hundredfold/reference.json \
  --output /tmp/rust-paired --seeds 801,804,808,817,911
```

The comparison runs Go and Rust sequentially, alternates their order, clears
inherited rare-estimator flags, and scores both against the same partial golden
set. Raw matrices and logs remain in the requested output directory.

The `oracle` CLI mode and `TestRustEstimatorOracle` Go test compare intermediate
RNG, MC, point-PMF, pool, and conditional-kernel results. See the experiment
report under `experiments/rare_positions/2026-09-30-rust.md` for measured results
and the limits of the quality comparison.

The active endpoint replacement has a Go differential oracle and paired
comparison harness. See `experiments/rare_positions/2026-09-30-rust-service.md`
for results and commands. The oracle's historical writes also use a temporary
table; neither comparison modifies application rating data.

The service branch was merged with the enabled reachability improvements;
[merge verification](../experiments/rare_positions/2026-09-30-rust-service-merge.md)
records response equivalence, memory reductions and measured latency increases
against that current Rust baseline.

## Unified player ratings

Build from the complete repository checkout: `odds-rust` depends on the shared
library in `stats/core`. The unified binary uses its existing native MySQL driver
and does not need Diesel or a system MySQL client library. Its `DATABASE_URL`
must point to the same database as Rails and permit player/appearance updates.

```sh
DATABASE_URL='mysql://USER:PASSWORD@127.0.0.1:3306/DATABASE' \
  odds-rust/target/release/golaberto-odds serve 127.0.0.1:6577
curl -X POST http://127.0.0.1:6577/player_ratings
```

Calling the endpoint updates player data; it is not a preview. Calculation and
persistence finish before `{"status":"ok"}` is returned. A failed input or write
returns HTTP 500, and the Rails caller raises on failed HTTP responses.
The player and appearance updates are batched into one transaction. Names,
appearance identities and other columns remain intact; deleted records are not
recreated. One timestamp controls the complete job's time-decay weights.

The existing calculation queue serializes player updates with odds and team
ratings. This preserves four estimator cores without running a competing player
calculation. Odds requests queue behind a player job; `/health` remains separate.
Player stage logs report `players.load`, `players.calculate`, `players.persist`.
The Rails caller retains its 300-second read timeout.

Player refreshes read independent inputs concurrently using at most four DB
connections, decode native dates, and read each player position once. Every
request still recalculates the complete ratings with one fresh UTC timestamp;
all player totals are written so their date weights stay current. Appearance
updates are omitted only when the newly computed offensive and defensive float
bits both match the stored values. Edited match, player, or team-history data
therefore changes the next calculation and is persisted.

Writes use a connection-local MEMORY staging table holding at most 5,000 rows,
then primary-key joins, replacing the large CASE expressions. The service DB
account needs `CREATE TEMPORARY TABLES`; no persistent schema migration is needed.
Both target tables still commit together. Staging cleanup uses DELETE and DROP
TEMPORARY TABLE, which preserve the target transaction; TRUNCATE would commit
it ([MySQL transaction rules](https://dev.mysql.com/doc/refman/8.4/en/implicit-commit.html)).

The [player performance measurements](../experiments/player_ratings/2026-10-02-performance.json)
show **92.8% lower median HTTP latency for a routine refresh**: 33.25 seconds to
2.39 seconds, recalculating 41,111 players and 821,930 appearances. Three timed
requests per implementation alternated order after warmups on the same
disposable copy of application data, with normal InnoDB destination tables.
The application database was only read. A separate fixed-clock replay where
almost all stored appearances needed changes improved by 78.4%; the 90% result
is for routine refreshes, not every possible full rewrite. All 823,217 raw
appearance outputs and 41,124 player outputs matched the frozen baseline bits.

Reproduce the isolated HTTP comparison with an original service binary and the
new release build (requires local mysql CLI access as root):

```sh
python3 script/performance/benchmark_player_ratings.py \
  --baseline /tmp/golaberto-player-before \
  --candidate odds-rust/target/release/golaberto-odds \
  --iterations 3 --output /tmp/player-http.json
```

The benchmark creates a uniquely named disposable schema, copies eligible
source data, warms the ratings, alternates HTTP requests, and drops its schema.
Use `--source-database NAME` to choose the read-only source. The fixed-clock
stage replay below writes only connection-local temporary destinations and
checks every stored float. `stored` copies the source's current ratings;
`empty` forces every appearance to update; `unchanged` models already-rated
history. Keep derived data exports and detailed logs outside the repository.

```sh
cargo run --release --locked --manifest-path odds-rust/Cargo.toml \
  --example player_ratings_benchmark -- /tmp/player-stages.json \
  1790812800 3 stored
```

For a read-only check on application data:

```sh
DATABASE_URL='mysql://USER:PASSWORD@127.0.0.1:3306/DATABASE' \
  cargo run --release --locked --manifest-path odds-rust/Cargo.toml \
  --example player_ratings_dry_run -- /tmp/player-ratings.json
```

This example writes an output file with IDs and raw float bits; it performs no
DB updates. An optional fixed timestamp and input-export filename allow offline
formula comparisons. Keep database-derived exports outside the repository.

Tests of the shared library run separately:

```sh
cargo test --release --locked --manifest-path stats/core/Cargo.toml
MYSQL_TEST_URL=mysql://root@127.0.0.1:3306/GolAberto_development \
  cargo test --release --locked --manifest-path odds-rust/Cargo.toml \
  --test player_ratings -- --ignored
```

The optional DB test shadows every queried table with connection-local temporary
tables. [Player integration verification](../experiments/rare_positions/2026-09-30-unified-player-ratings.md)
documents the real-data comparison and disposable-schema HTTP check.

### Experimental conditional fixture probes

`RUST_ODDS_DOMAIN_PROBES` (default `0`) enables a bounded number of hypothetical
outcome checks per propagated-domain sampler call. `RUST_ODDS_DOMAIN_PROBE_NODES`
(default `16`) limits exemption-search nodes per proof direction; use `0` for
local propagation only. `RUST_ODDS_DOMAIN_PROBE_REALLOCATE=1` changes domain-rescue
pilot/final/check draws from 1,000/15,000/5,000 to 750/14,000/5,000.

These options remain experimental and disabled by default. Six-seed comparisons
found no additional estimates attributable to the probes. See the
[paired experiment report](../experiments/rare_positions/2026-09-30-rust-domain-probes.md)
for measurements, correctness assumptions, and the draw-only ablation.

The [larger-budget follow-up](../experiments/rare_positions/2026-09-30-rust-domain-probes-larger.md)
also tested 24, 96, and 384 hypotheses per call. More outcomes were pruned, but
no additional estimates were found; these budgets are not recommended defaults.

### Experimental coalition branching and directed goal paths

`RUST_ODDS_COALITION_BRANCHING=1` restricts early proof exemption branching to
members of a violated discrete coalition. `RUST_ODDS_GOAL_PATHS=1` allows
symbolic GD completion along decisive winner-to-loser paths, with full sorter
verification. Both default to disabled. They added no reference coverage in
90 paired comparisons; the combined persistent benchmark increased current
16653 latency by 3.18%. See the [experiment report](../experiments/rare_positions/2026-09-30-rust-path-coalitions.md).

The `residual_components` example audits small residual point-cap components
offline; it does not change production reachability labels.


### Default rare-position coverage

The default profile below funds a final weighted portfolio for cells still zero
after the existing pipeline, with at most four workers. It preserves the
scout, pooled MC, proofs and earlier positive estimates.

```sh
./odds-rust/target/release/golaberto-odds serve
```

Use the server arguments documented above for your port/database. An unset
`RUST_ODDS_RARE_TAIL` selects `coverage`; explicitly setting it to `0` keeps the
previous pipeline. Explicit `coverage` remains supported. The profile combines full
support sampled target paths, four-rival exact joint blocks, Poisson-binomial
rank guidance, conditional goal tilts, adaptive witness proposals and omission
of strictly irrelevant fixtures. Frozen suffix probability guides are shared
across cells only when fixture order, gains and probabilities match exactly.
Final main/check batches are fresh and fixed;
proposals are frozen after training. Remaining zeros are not assigned witnesses
or probability bounds as estimates.

It targets order-of-magnitude coverage: final estimates may have relative
standard error up to 60%, with a separate check up to 75% and agreement within
a factor of 5. `meets_precision_goal` still reports the existing stricter criterion.
Set `RUST_ODDS_RARE_TAIL_QUALITY=current` to retain the previous final acceptance
checks; coverage will be lower. Explicit experimental flags override profile
defaults. Case enumeration, learned fixture bias, inside-draw propagation and
point-tilt reallocation remain disabled in this profile.

The tail allocation experiment remains opt-in after production regression
reports. Default coverage uses the previous complete-union cap of 256,
existing witness/alternate retries, and ordinary final-batch allocation.

To explicitly enable the original allocation experiment, first disable the
sequential extension with `RUST_ODDS_RARE_TAIL_EXTENSION=0`, then set
`RUST_ODDS_RARE_TAIL_UNION_PATTERNS=64`, `RUST_ODDS_RARE_TAIL_RETRY=roots`, and
`RUST_ODDS_RARE_TAIL_BATCH_ALLOCATION=reserve`. It learns target-path allocation
from pilot second moments and enlarges weak-pilot main/check batches from spare
capacity. Its published estimates retain the existing quality and confirmation
gates, but altered candidate selection can displace earlier successful cells.
Restore defaults individually with `256`, `0`, and `0`, respectively.

The [tail allocation experiment](../experiments/rare_positions/2026-10-01-rust-tail-retry.md)
records the initial gains/losses. The
[regression follow-up](../experiments/rare_positions/2026-10-01-rust-tail-regressions.md)
records the decision to restore the previous default configuration.

The portfolio uses deterministic operation and setup limits by default (see the
budget table below). Elapsed time is diagnostic only. Set
`RUST_ODDS_DETERMINISTIC_WORK=0` to restore the legacy controller: 45% of elapsed
native calculation time, a 65% training cutoff, and measured pilot costs. Neither
controller is a hard latency guarantee. See the [campaign report](../experiments/rare_positions/2026-10-01-rust-rare50.md)
and [progress ledger](../FUTURE_EXPERIMENTS.md) for paired latency, CPU, memory,
regressions, seeds and independent-reference comparisons.

### Retries after ordinary estimates

The default coverage profile uses `RUST_ODDS_RARE_TAIL_EXTENSION=after`:
ordinary coverage pilots, funding and final batches run first. Only remaining
zeros can receive a fresh fixed main batch and independent confirmation, reusing
an already trained proposal. Accepted ordinary estimates and their metadata are
preserved before matrix reconciliation. The retry retains the same quality gates
and four workers, and receives only the unused part of the existing allowance.
It can increase actual request latency; unused allowance is not free wall time.

`overlap` is a separate experiment using smaller batches on idle workers within
the predicted ordinary final span. With legacy wall budgets it skips retries whose ordinary result is
still in flight. Deterministic budgets always insert a barrier before retry
admission, including with `overlap`, to avoid dependence on worker scheduling. Sequential `after` is enabled by default; `overlap` remains
experimental. Both select the restored ordinary settings (`256` patterns, existing witness/alternate retries, ordinary
allocation), even if the three R26 experimental settings above are also set.
Other explicit profile settings still apply. Use the normal coverage profile.

```sh
odds-rust/target/release/golaberto-odds serve
```

Set `RUST_ODDS_RARE_TAIL_EXTENSION=0` to disable retries while retaining ordinary
coverage. Unsetting the flag uses the default sequential retry. No extra flags
are required after rebuilding and restarting the server.

The preserved-result guarantee is within a request: the ordinary allocator still
uses elapsed time, so separate runs can fund different ordinary cells even with
a fixed seed. The [retry allocation report](../experiments/rare_positions/2026-10-01-rust-tail-preservation.md)
records paired coverage, remaining losses and latency increases. New log events
`rust_odds_rare_tail_extension` and `rust_odds_rare_tail_extension_summary` record
source, batch sizes, evidence, acceptance and time; start/summary logs identify
the selected mode and effective ordinary settings.

### Additional confirmation for positive-hit pilots

The default coverage profile uses `RUST_ODDS_RARE_TAIL_CONFIRM_MORE=1.5` for
fresh confirmation of already-built proposals, including weak positive-hit
pilots. No additional flag is required after rebuilding and restarting.
Use `0.25` for a smaller allowance or `0` to disable this stage.
With deterministic budgets the value scales `200000 × reference_draw_cost ×
value / 1.5` work units across main/check pairs. The reference cost depends on
remaining fixtures and team count; actual proposal draw costs come from pilots.
With legacy wall budgets it is
a fraction of measured pre-tail time. Neither specifies a request latency
increase.
Only remaining zeros are eligible. Batch sizes derive from pilot hits/ESS and
are fixed before fresh main/check sampling; training observations never enter
the estimate. Existing quality gates and the four-worker limit are retained.
Zero-hit pilots are excluded. Each independent batch has 2,000–30,000 samples.

The larger setting gained 34 nonzero cell-runs across 23 paired saved requests,
with no losses, at 27% more CPU and 32% more aggregate wall time. The affected
groups averaged approximately 0.33–0.39 seconds extra. Local timings do not
establish a production deadline. See the
[confirmation experiment](../experiments/rare_positions/2026-10-01-rust-more-pilot-confirmation.md)
for per-group costs, seeds, and limitations. Logs
`rust_odds_rare_tail_confirm_more` and `rust_odds_rare_tail_confirm_more_summary`
record evidence, acceptance, work and additional time.

### Shared blocker conditioning in the coverage profile

The default coverage profile enables the strongest tested
multi-cell sampler automatically: cardinality guidance, two blockers constrained
against each recipient's actual final points/wins, concurrent independent
main/check batches, and a separate operation allowance scaled by 50% of the
ordinary tail reference allowance (default: 22,500 reference draws, including
setup and pilots; these are work units, not a fixed actual draw count).
Four-worker usage and acceptance gates are unchanged. The legacy wall controller
uses a relative tail allowance of 45% after native CPU optimization: faster
earlier stages otherwise shrink search funding.
This reallocates part of their measured savings; total request CPU and latency
must be compared with the previous 35% profile. No extra shared-sampler flags
are needed.

Set `RUST_ODDS_SHARED_CONSTRAINTS=0` to disable shared sampling while retaining
the individual coverage portfolio. Explicit settings override profile defaults:
`RUST_ODDS_SHARED_CONSTRAINTS=blocker|mixture|guided`,
`RUST_ODDS_SHARED_CONSTRAINTS_BLOCKERS`,
`RUST_ODDS_SHARED_CONSTRAINTS_RELATIVE`,
`RUST_ODDS_SHARED_CONSTRAINTS_CONFIRMATION`, and
`RUST_ODDS_SHARED_CONSTRAINTS_FRACTION`. The native pipeline still applies when
`RUST_ODDS_RARE_TAIL=0` is set.

Across 39 paired requests the selected configuration gained seven cell-runs and
lost one; eight warm comparisons had no net coverage gain. Warm median latency
was 830.18 → 825.83ms; the largest cold-pair increase was 6.70%. This is a measured
coverage tradeoff, not a guarantee that every previous estimate survives
allocation changes. Flags, weighting, timing, regressions and reproduction
commands are in
[the shared-constraint report](../experiments/rare_positions/2026-10-01-rust-shared-constraints.md).

### Portable CPU optimizations

Native release builds retain the same simulation counts, random streams and
probability gates. Fixture bounds are loaded once, constant CDF tails retain
their original floating-point residues, and deterministic rank terms skip
redundant arithmetic. Score lookup and leading integer standings comparisons
preserve the original samples and sorting rules. Prepared rank guidance rows,
bounded rank DP, small dense point-state accumulation and borrowed root keys
remove repeated work. The paired local measurements show 20.22% less CPU in cold
requests and 22.01% less in warm servers, with no coverage losses in those cohorts.
Poisson mean rounding/table sharing was tested and remains experimental.

The paired CPU experiment uses four workers and process user+system time.
Portable local gains are expected to apply to the Xeon; exact percentages may
vary. Production access is not required for this work. See the
[CPU report](../experiments/rare_positions/2026-10-01-rust-cpu.md) for source
patches, all seven snapshots, coverage regressions and reproducible local
commands. Optional profile-guided builds must generate their profiles on the
intended architecture; a normal `cargo build --release --locked` needs no PGO
tools or runtime optimization flags.

The `/odds` response always includes `rare_position_estimates[team_id][zero_based_rank].reachability` as `impossible`, `reachable`, or `undecided`. A reachable cell can still have zero probability when no acceptable estimate was found. Detailed proof labels remain internal and in diagnostic logs.

### Complete-branch refinement

The default coverage profile includes a generic complete-branch stage after
existing coverage estimates are frozen. It can fill remaining zero cells with
few attainable target paths and few complete exception cases. All branches
retain positive sampling allocation; cheap original-model bounds order the
work, independent pilots choose guidance/allocation, and fresh main/check
batches use the existing rough-estimate acceptance gates. There is no omitted
probability or team/position hardcoding. The stage uses the same four workers.

Set `RUST_ODDS_RARE_TAIL_BRANCHES=0` to disable this stage explicitly. Other
profiles can enable it with `RUST_ODDS_RARE_TAIL_BRANCHES=1`.
Its default operation quota uses 90,000 reference draws: a 600,000-draw reference multiplied by
`RUST_ODDS_RARE_TAIL_BRANCH_BUDGET_FRACTION` (default 0.15, range 0–0.5). Zero
disables its work. Both pilots and full final pairs are reserved before sampling.
With legacy wall budgets, this parameter remains a fraction of preceding
calculation wall time and admitted fixed batches finish even if the forecast
overruns the allowance.
Construction remains bounded by64 target paths,256 attempted exception cases,
100k joint setup nodes and the existing four-million guide-value cap. At most
eight remaining zero cells are considered. Incomplete enumeration skips the
proposal and does not prove impossibility.

`RUST_ODDS_RARE_TAIL_BRANCH_BOUND_FLOOR=1` experimentally reduces negligible
branches to two final draws and reallocates the freed draws while retaining
full support. It is not required for the measured main configuration. Logs
`rust_odds_rare_tail_branches`, `_skip` and `_summary` record stage timings,
evidence, setup failures, budget and preservation of earlier estimates.
See [the production-path experiment](../experiments/rare_positions/2026-10-01-rust-production-branches.md)
for paired full-request coverage, latency, CPU and limitations.

### Deterministic work budgets

Enabled by default. `RUST_ODDS_DETERMINISTIC_WORK=0` restores elapsed-time
admission for comparison. The fixed seed default is independent of this flag.
`RUST_ODDS_WORK_MODEL=draws` is an experimental draw-count diagnostic; use the
archived prototype binary to reproduce its measured trial.

The production coverage allocator models:

```
estimated work = setup units + draws × pilot cost per draw
reference_draw_cost = 64 × remaining fixtures + 32 × teams + 1
pilot cost per draw = ceil((8 × fixture operations + guidance units
                           + ranking allowance) / pilot draws)
setup units = 16 × enumerated nodes + 2 × guide values
```

Fixture counters follow active guided, joint, and omitted fixture sampling.
Guidance counts domain checks, optimized cardinality DP support, and additional
mixture-density replays, including early exits. Ranking has an allowance for
conditional score sampling, campaign updates, sorting, and the actual phase's
number of sort keys. These are portable scheduling proxies, not measured CPU
cycles. No duration enters admission in the default mode.

| Stage | Default reference allocation | Scaling |
| --- | ---: | --- |
| Shared setup, pilots and finals | 22,500 | `100000 × tail_fraction × shared_fraction` |
| Individual training prefix | 28 cells | `floor(64 × tail_fraction)`, maximum 128 |
| Individual setup, pilots, finals and retries | 90,000 | `200000 × tail_fraction` |
| Additional confirmations, reusing trained proposals | 200,000 | `200000 × confirm_more / 1.5` |
| Complete branches, including setup and pilots | 90,000 | `600000 × branch_fraction` |

Reference allocations multiply by `reference_draw_cost` to obtain work units.
Fractions retain defaults 0.45, 0.5, 1.5, and 0.15. Individual setup and pilots
are charged once; confirmation reuses them without charging setup again.
Constructor cell/node/guide limits additionally bound setup attempts, including
unsuccessful construction. The model estimates proposal costs; it does not
establish an exact instruction count or hard wall deadline.

Ordinary main/check pairs are reserved together in stable order. Ordinary finals
finish before retries are admitted, irrespective of worker count. Their skipped
checks are not refunded. Additional confirmation uses two fixed waves: reserve
normal pairs first, use spare units for pending mains, then reclaim checks that
were skipped after every first-wave job finishes. Only then reserve fresh checks
for publishable pending mains or complete pairs for unstarted candidates. A main
without an independent check never publishes. Neither wave changes main lengths
or proposal costs based on its own observations. The total modeled quota stays
fixed; second-wave savings are not recycled into another wave.

Confirmation priority uses forecast ESS per work unit, smoothed by pilot hit
reliability `hits / (hits + 30)`. The 30-hit acceptance requirement itself is
unchanged. Running expensive admitted jobs first reduces scheduling tails;
results are returned in stable admission order.

Complete branches retain the original 30,000-draw allocation and streams first.
On failure, spare modeled work may fund a fresh, independent retry of up to
90,000 draws for a cheap proposal. Every supported branch retains a positive
allocation. Earlier positive estimates are preserved. The residual-loop
experiment was discarded from active code; its results and source patch are
retained in the experiment archives.

Summary logs include `budget_mode`, `work_limit`, `reserved_work`, setup/pilot
work, and proposal draw costs. Historical `work` retains its nominal season
work meaning. Timing logs remain diagnostic. See the
[paired experiment](../experiments/rare_positions/2026-10-01-rust-deterministic-work.md)
for coverage, latency, reproducibility checks, and practical limits.

### Certified totals and complete parent fallback

The coverage profile now certifies target-total limits before constructing the
complete branch proposal. This runs after ordinary portfolio results are frozen
and before additional confirmations. Earlier pilots retain their original
proposals. Both joint target tables consult the same per-request certificate.
Only a fully refuted points/applicable-wins relaxation removes a total; feasibility
or quota exhaustion supplies no restriction. There is no team or rank whitelist.

The certification screen runs only when the existing terminal admits more than
64 target paths. Its upper/lower queries share at most 16,000 fixture nodes per
cell, and at most eight unresolved cells enter the branch stage. Points/wins are
packed only when wins is the second phase sort key. Actual goals and remaining
sort keys are still checked by the production sorter during probability sampling.

All primary strata must finish before optional secondary refinement. Refinement
has 2,000 extra nodes per parent and 10,000 per cell. Any failed or unfinished
split discards the optional children and retains the entire complete primary
proposal. Incomplete primary enumeration still skips the cell. Every supported
stratum keeps a positive final allocation and its original prior factors.

Newly certified proposals use independent pilots and 25,000-draw main/check
allocations. Their reserved setup/pilot/final work is deducted from the existing
additional confirmation quota; handled cells are not sampled again at the late
branch stage. Earlier positive estimates and acceptance gates remain unchanged.
Certificates charge setup once. Transferred work is also capped by the configured
confirmation allowance, including reduced diagnostic overrides. Full-request
timing is still diagnostic.

Defaults require no extra flags. Diagnostic overrides:

| Flag | Effect |
| --- | --- |
| `RUST_ODDS_CERTIFIED_TARGET_LIMITS=0` | Disable the new total certification |
| `RUST_ODDS_COMPLETE_PARENT_FALLBACK=0` | Abort when secondary refinement fails |
| `RUST_ODDS_CERTIFIED_BRANCH_TRANSFER=0` | Restore late branch placement; this can cost more time |
| `RUST_ODDS_CERTIFIED_BRANCH_DRAWS` | Newly certified batch allocation, 10,000–30,000; default 25,000 |

Logs expose certified packed limits, proof nodes, charged work, primary fallback,
secondary setup nodes and confirmation work transferred to branches. See
[the paired experiment](../experiments/rare_positions/2026-10-02-rust-certified-target-limits.md)
for coverage, timing by seed, assumptions and remaining zeros. Local timings do
not establish identical speed on the production Xeon.

### Complete-tree fallback and constraint messages

The coverage profile enables `RUST_ODDS_RARE_TAIL_TREE=1` by default. After
ordinary results are frozen, a certified cell whose complete exception-mask
enumeration exhausts its budget can use a bounded partition tree instead.
It keeps every supported target path and retains the entire parent whenever a
split cannot finish. No team or rank is singled out.

Production settings are 64 leaves, 100,000 joint construction nodes and
4,000,000 cumulative guide values, including failed refinements. Each leaf gets
25 pilot draws. The variance-leading positive pilot leaf receives three passes
of fixture/team interval messages, mixed with 10% original outcome probabilities.
The joint normalizer is rebuilt under the proposal, and every sampled joint or
guided fixture receives the original/proposal probability correction. Messages
guide sampling; they do not prove reachability or impossibility.

Fresh independent main/check allocations use up to 3,000 draws each, with a
positive floor for every leaf. Pilots are excluded from reported estimates and
the existing acceptance gates still apply. Setup, pilots and final draws debit
the existing confirmation allowance. Unused native checks are refunded and
confirmations fund a stable priority prefix. An accepted additional cell can
return at most 16,000,000 operation units to confirmations, further capped by
the tree's reservation and a conservative proportion of initial-MC zero cells.
A request with no accepted additional cell earns zero credit. Admission remains
deterministic.

Across seven reference snapshots and ten seeds, the experiment gained one
distinct cell (Flamengo/13th, 10/10 runs), with no lost estimates or changed
proof classifications. Repeated warm group16498 requests measured **CPU +2.48%** and
**median HTTP latency +4.58%** on the local machine, using four workers. The
user approved this tradeoff; it exceeds the 1.18% recovered rare-cell gain.
Production adoption checks reproduce all 70 prototype exports byte-for-byte.
Two subsequent warm checks against prior production observed higher costs:
**CPU +5.17–6.81%**, **median HTTP +6.66–8.05%**. A direct prototype comparison
measured CPU −3.39% and median HTTP +0.97%. These are local observations with
timing variability, not a guarantee for the production Xeon.

No additional flags are needed. Set `RUST_ODDS_RARE_TAIL_TREE=0` to disable the
fallback, refunds, priority-prefix allocation and conditional credit together.
The exact memoized setup calculation remains enabled. Rebuild and restart the
Rust server to use the changed defaults.

`rust_odds_start` reports the tree switch and settings. Branch logs include tree
construction, message work, independent batch diagnostics and accepted results;
`rust_odds_rare_tail_tree_credit` records bounded additional work credit.
See [the experiment and adoption results](../experiments/rare_positions/2026-10-02-rust-flamengo-messages.md).
