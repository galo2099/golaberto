# Rust finishing-position estimator

Native Rust implementation of the current Go **matched point pool** estimator.
It includes the initial game-importance scout, 100,000-season pool, exact point
PMFs, uncertainty estimates, cap/floor proofs, conditional sampling, rank-directed
importance sampling, neighborhood and constructive witnesses, point-tilt/gap
rescues, cross-team witness reuse, directional/constraint peer rescues, propagated
domains, reduced fixtures, and final matrix reconciliation.

The executable does not call Go and does not read the golden probabilities.
Go is used only by the offline comparison harness. Sampling budgets and
acceptance thresholds, including the 35% relative-SE gate, are preserved.

## Build and run

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

## Timing logs

Request and stage logs are enabled by default and written as JSON lines to
stderr. Each record includes a Unix timestamp in milliseconds, a request ID,
and cumulative request wall time. Estimator records also include the group
and seed, making a slow run reproducible.

The stages include setup, the initial scout, pool MC, point PMFs, the matched
matrix, jackknife uncertainty, point proofs, initial conditioning, guided and
extra search, witnesses, point tilts, neighborhood walk, peer/domain rescues,
and reconciliation. Stage records report `elapsed_ms`; enclosing `pool` and
`search` totals include their sub-stages, so do not add parent and child times.
The completion record reports positive/zero cells, certified impossible zeros,
reachable zeros without estimates, unresolved zeros, and modeled pool/search
work. Work units are not a count of ordinary MC seasons and exclude the
initial scout and constraint proof nodes.

HTTP additionally reports request reading/JSON decoding, response encoding,
writing, status, byte counts, and `http_total_ms`. This total begins when the
connection is accepted and includes reading and writing. It excludes time
waiting to be accepted while an earlier serial request runs. Estimator
`total_ms` covers calculation and response construction, excluding JSON and
network I/O. All durations are wall time, not summed CPU time across workers.

```sh
odds-rust/target/release/golaberto-odds serve 127.0.0.1:6578 \
  2> /tmp/rust-odds.log
```

Use `RUST_ODDS_LOG=0` to disable these request/stage logs for benchmarks.
The CLI's compact timing summary remains available. `RUST_ODDS_PROFILE=1`
also enables logs, including when the quiet toggle is set.

## Local HTTP adapter

```sh
RARE_POSITION_RANDOM_SEED=808 \
  odds-rust/target/release/golaberto-odds serve 127.0.0.1:6578
curl -H 'Content-Type: application/json' --data-binary @REQUEST.json \
  http://127.0.0.1:6578/odds
```

`POST /odds` preserves `team_odds`, `game_importance`, and
`rare_position_estimates`. `team_odds[*].Pos` contains percentages; estimate
probabilities contain fractions and rank keys remain zero-based. Rails null
score/bias fields are accepted. `GET /health` is available. HTTP requests run
serially with up to four estimator workers. Without a configured seed, HTTP
uses the current time and logs the seed.

The existing Rails/Go service routing is unchanged. This adapter implements
only `/odds`; Go still provides `/spi` and the rating endpoints. Route `/odds`
separately when evaluating a deployment. The adapter expects Content-Length
requests, as sent by the current Rails client.

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
