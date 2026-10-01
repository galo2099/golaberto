# Rust odds and ratings service

Native Rust implementation of the current Go **matched point pool** estimator.
It includes the initial game-importance scout, 100,000-season pool, exact point
PMFs, uncertainty estimates, cap/floor proofs, conditional sampling, rank-directed
importance sampling, neighborhood and constructive witnesses, point-tilt/gap
rescues, cross-team witness reuse, directional/constraint peer rescues, propagated
domains, reduced fixtures, and final matrix reconciliation.

The executable does not call Go and does not read the golden probabilities.
Go is used only by the offline comparison harness. Sampling budgets and
acceptance thresholds, including the 35% relative-SE gate, are preserved.

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
The four-candidate limit and four workers are unchanged. Accepted estimates
are published after existing search and witness reuse, before reconciliation.

The model supports ordinary 3/1/0 scoring with points first, and packs wins only
when wins immediately follow points. Unsupported, oversized or numerically
empty models skip this proposal without changing reachability. The logged
`target_mass` is a necessary target-event probability; `joint_normalizer` is
an expectation of the proposal's terminal weight and **is not an upper bound**.
Witnesses alone do not supply probability estimates. Finite samples and the
confirmation diagnostics do not certify tiny-event accuracy.

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
serially with up to four estimator workers. Without a configured seed, HTTP
uses the current time and logs the seed.

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
