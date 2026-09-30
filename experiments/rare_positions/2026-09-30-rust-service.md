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

## Paired HTTP speed and memory benchmark

Measured all four active endpoints on the local macOS machine after the
replacement was completed, using the release Rust executable at `69f820a5`
and a compiled Go test host that calls the original production HTTP handlers.
Two persistent servers run on unused loopback ports. Requests alternate between
implementations, with two warmups followed by seven timed requests per endpoint.
Only one request calculates at a time. Go has `GOMAXPROCS=4`; Rust keeps its
existing four estimator workers. Request logs are disabled for both, and seed
808 is fixed. Latency includes HTTP upload, calculation and response transfer.

Peak memory is the **whole server process maximum resident set size** reported
by macOS `/usr/bin/time -l` over all nine requests, not a per-request allocation
count. Each scenario uses fresh processes. The Go benchmark host includes the
test runtime; the production handlers themselves are unchanged. The ratings
payload contains 27,465 real matches and 3,425 teams, spanning approximately four
years. Evaluation selects the last 40 matches across eight UTC dates, retaining
the preceding matches for training; synthetic phase tags select those matches.
Historical requests include real MySQL upserts in a disposable InnoDB schema
with the Rails FLOAT column types and unique team/date key. That schema was
dropped after the run; application tables were not changed.

| Endpoint / workload | Go median | Rust median | Speedup | Go peak RSS | Rust peak RSS |
|---|---:|---:|---:|---:|---:|
| `/odds`, group 16653 | 1,157.5 ms | 658.7 ms | 1.76x | 293.3 MiB | 430.9 MiB |
| `/odds`, group 16982 | 873.8 ms | 293.2 ms | 2.98x | 41.8 MiB | 25.7 MiB |
| `/spi`, database export | 108.9 ms | 37.0 ms | 2.94x | 39.5 MiB | 67.4 MiB |
| `/eval`, last 40 matches | 395.3 ms | 97.7 ms | 4.05x | 39.8 MiB | 65.6 MiB |
| `/historic_ratings`, database export | 351.3 ms | 111.7 ms | 3.14x | 41.5 MiB | 72.0 MiB |

All tested requests are faster in Rust. **Peak memory is higher for four of
the five workloads**, by approximately 47–74%; group 16982 uses about 39% less.
Startup idle RSS is lower in Rust, approximately 6.4 MiB versus 11.8–12.2 MiB,
but that does not predict peak memory under repeated requests. The high-water
measurement includes allocator-retained memory; it does not establish a leak
or identify which allocations should be reduced. Memory profiling remains a
useful next investigation. These local measurements do not establish Linux
memory consumption or throughput under concurrent requests. The Rust server's
calculation gate intentionally serializes simultaneous calculation requests.

This is not a speedup from reducing the measured odds budget. Both languages
report identical work: 78,708,000 fixture-work units and 394,000 conditional
draws for 16653, and 35,000,000 units with no conditional draws for 16982.
Both use the 20,000 initial scout and 100,000 matched-pool seasons. Per-cell
sample counts, conditional draws, estimator designs and reachability statuses
match. Maximum absolute position probability differences are 1.3500e-13
percentage points for 16653 and 3.5527e-14 for 16982. The initial game-importance
scout differs between the implementations, so the benchmark does not assert
identical game-importance output. No CEM configuration is compared here:
Go uses `RARE_POSITION_MATCHED_POINT_POOL=1`,
`RARE_POSITION_IMPORTANCE_SAMPLING=0`, with remaining search flags at defaults.

The SPI response differs by at most 1.1369e-13 and evaluation by 4.4409e-16.
Historical HTTP responses and all 2,230 stored historical rows match exactly.
All per-request timings and process resource logs are saved under
`/private/tmp/golaberto-rust-service/http-performance`, with the timing summary
also retained beside this report as `2026-09-30-rust-service-http-benchmark.json`.
Private database-derived input and response bodies remain outside the repository.

Reproduce on macOS with a local MySQL root account:

```sh
(cd go && go test -mod=mod -c -o /tmp/golaberto-service-benchmark.test .)
python3 experiments/rare_positions/benchmark_rust_service.py \
  --rust odds-rust/target/release/golaberto-odds \
  --go /tmp/golaberto-service-benchmark.test \
  --ratings-request /path/to/read-only-ratings-export.json \
  --output /tmp/golaberto-http-performance --iterations 7
```

The benchmark creates a randomly named `golaberto_rust_bench_` schema and drops
it in cleanup. The Go host refuses historical benchmarking against a schema
without that prefix. It is opt-in and does not change the production Go server.

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
