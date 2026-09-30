# Native Rust replacement for active Go endpoints

## Scope and isolation

Implemented on `codex/rust-service-replacement` in the managed worktree
`/Users/robsonaraujo/.codex/worktrees/rust-service-replacement/golaberto`,
starting from `5821a358002718b0979a3ebd76e9ec5345ebb46e`.

The service covers `/odds`, `/spi`, `/eval`, and `/historic_ratings` without
calling Go. The user explicitly excluded the deprecated Go `/player_ratings`
endpoint. The separate `stats` Rust service and Rails player-rating route on
6578 are unchanged. No live Go process was stopped or replaced.

## Implementation

- Ported SPI fitting, team-strength conversion, ranked probability scoring,
  rolling training histories, and historical snapshots to native `f64` Rust.
- Preserved uppercase SPI response fields and null ratings for inactive teams.
  Rails lowercase inputs, original Go names, and null numeric ratings are accepted.
- Preserved the legacy convergence limit/criterion, evaluation advantage
  convention, day-of-month-only refit trigger, four-year training window,
  and historical snapshot schedule. This is a compatibility port, not a new
  ratings model.
- Historical HTTP requests compute the series and upsert it to MySQL before
  returning the same empty collections as Go. Parameterized batches retain
  six-decimal persistence, and one transaction prevents partial updates.
- Corrected the Rails historical action to recognize a service-persisted
  empty series and avoid a second invalid SQL insert. HTTP failures are checked
  before response decoding. Nonempty legacy series are still accepted.
- Replaced the limited handwritten odds HTTP parser with `tiny_http`, including
  chunked bodies, keep-alive handling, a 128 MiB body limit, bounded queue,
  four HTTP workers, and an independent health route. Calculation requests
  share a gate to preserve four estimator workers rather than multiply workers
  by concurrent requests. Queue saturation returns 503.
- The executable's default address is `127.0.0.1:6577`, including when started
  without arguments. CLI `spi`, `eval`, and `historic` modes permit read-only
  comparison; `historic` exports rows rather than updating MySQL.
- MySQL is required only for nonempty historical writes. `DATABASE_URL`
  selects the Rails database; the unset fallback matches Go's development
  database. Production must set this explicitly. Connect/read/write timeouts
  and HTTP 500 errors replace a process crash on database failure.

## Differential results

The synthetic fixture has 360 matches spanning almost five years, three active
teams, an inactive fourth team, null warm-start ratings, extra-time lengths,
home/neutral/away advantages, and a held-out evaluation phase. The historical
comparison covers 204 stored snapshots and exercises history-window expiry.

| Comparison | Maximum absolute difference |
|---|---:|
| Synthetic SPI | 4.9738e-14 |
| Synthetic evaluation RPS | 5.5511e-17 |
| Synthetic historical rows versus Go SQL values | 4.9939e-7 |
| Read-only DB export SPI: 27,465 games / 3,425 teams | 1.1369e-13 |

The historical discrepancy is the expected rounding of Go's six-decimal SQL
values versus the unrounded read-only Rust CLI output. Rust persistence applies
the same six-decimal rounding. Both implementations return 1,115 non-null ratings
for the database export. Database-derived requests and team ratings remain in
`/private/tmp/golaberto-rust-service`; they are not committed.

The repeated harness run measured 164.6 ms for the Go SPI oracle process and
43.6 ms for the Rust SPI CLI on that export. These are single process-level
observations with different logging/output costs, not a controlled speedup claim.

## Existing odds regression check

Built the starting Rust revision independently and ran three alternating paired
full CLI requests per snapshot, seed 808, four workers. All **15 pairs have
identical parsed responses**, including probabilities and reachability metadata.

| Snapshot | Starting Rust median | Replacement median |
|---|---:|---:|
| 16498 / 44eabb47 | 729.1 ms | 676.7 ms |
| 16653 / earlier 2d1c1d6f | 684.8 ms | 665.0 ms |
| 16653 / current 71d4fea8 | 729.1 ms | 683.5 ms |
| 16982 / 9327edcd | 294.1 ms | 288.1 ms |
| 16983 / 43969b02 | 284.3 ms | 285.2 ms |

These short local measurements show no material regression; they do not prove
a speedup. The estimator implementation and sample budgets are unchanged.
The current 16653 full HTTP request also matches the CLI exactly, with 737.8 ms
observed client wall time. Its wall time includes HTTP work unlike CLI estimator
time. Timing depends on other load on this shared machine.

## Verification

- Rust release suite: 19 passing tests; one MySQL test is opt-in.
- Final release run with `--include-ignored`: all 20 Rust tests pass.
- Explicit MySQL test: passes six-decimal values, duplicate-row updates, empty
  batches and rollback when a later batch fails. All writes use a connection-local
  temporary table shadowing the real table.
- HTTP tests cover health, active routing, query strings, chunked JSON,
  malformed JSON, wrong methods, inactive SPI teams, evaluation without selected
  games, historical empty responses, database failures and continued health.
- Full Rails suite: 117 tests, 314 assertions, zero failures/errors, one existing
  skip. Run against a dedicated scratch database to avoid other agents' test data.
  Includes the persisted empty response, nonempty legacy response and HTTP errors.
- Full Go suite: passes; the new differential oracle skips unless enabled.
- `cargo fmt` and `git diff --check`: pass.
- Strict Clippy is blocked by 18 existing warnings in unchanged estimator files;
  it reported no warning in the new endpoint modules before stopping.

The full historical HTTP check against the actual Rails schema stored 204 rows,
matching Go exactly after MySQL FLOAT storage. A repeated request updated those
same rows without duplicates. A health check completed in 0.75 ms while a full
16653 odds request was still calculating. These writes used synthetic rows in
the isolated scratch database; application data was untouched.

## Reproduction

```sh
cargo build --release --locked --manifest-path odds-rust/Cargo.toml
cargo test --release --locked --manifest-path odds-rust/Cargo.toml
MYSQL_TEST_URL=mysql://root@127.0.0.1:3306/GolAberto_development \
  cargo test --release --locked --manifest-path odds-rust/Cargo.toml \
  --test database -- --ignored

(cd go && go test -mod=mod -c -o /tmp/golaberto-ratings-go.test .)
python3 experiments/rare_positions/compare_rust_service.py \
  --rust odds-rust/target/release/golaberto-odds \
  --go /tmp/golaberto-ratings-go.test \
  --baseline /path/to/starting-rust-binary \
  --output /tmp/golaberto-service-paired --iterations 3
```

The oracle's historical path uses a temporary table, not persistent writes.
Add `--db-request /path/to/read-only-export.json` to compare a real SPI request.
The raw paired run and summary are in
`/private/tmp/golaberto-rust-service/paired` for this session.

## Deployment

Build the Rust executable and configure its `DATABASE_URL` for the Rails
database. Validate on an unused local port first. When ready, replace the Go
process on 6577 with:

```sh
odds-rust/target/release/golaberto-odds serve 127.0.0.1:6577
```

Keep the existing `stats` service on 6578. Go remains in the repository as a
reference; the native replacement does not invoke it. No deployment, process
restart, merge, or push was performed by this task.
