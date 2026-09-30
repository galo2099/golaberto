# Unified Rust player ratings — 2026-09-30

## Implementation

The `golaberto-odds` binary now handles `POST /player_ratings` alongside `/odds`,
`/spi`, `/eval`, and `/historic_ratings`, all on port 6577. The player endpoint
accepts an empty body, computes and persists ratings, then returns
`{"status":"ok"}`. It uses the existing native MySQL driver. No subprocess,
second server, Diesel dependency or system MySQL client library is needed by
the unified binary.

The formulas were extracted from `stats/src/main.rs` at baseline `dd61c883`
into the shared `stats/core` library. Both services now call that library.
The standalone stats server remains available on port 6578 for rollback.
Rails `PlayerRatingUpdateService` now calls 6577 and raises on failed HTTP
responses; its existing 300-second read timeout remains.

The native reader preserves the category-1, played-game, four-year filters,
historical-rating selection and appearance filters. Games are processed
chronologically. Player and appearance calculations preserve legacy `f32`
arithmetic, position weights, penalties, own goals, red-card and overtime
handling. The complete job uses one explicit UTC clock instead of reading the
clock separately for each game. Comparison runs froze both implementations
at the same clock.

Player updates use batches of 1,000, and appearance updates batches of 5,000,
inside one transaction. Only existing identities are updated; names and other
columns are preserved. A deleted record is not recreated. Both tables must use
InnoDB for rollback. The local application tables were checked to be InnoDB.
There are no schema changes or new application tables.

The existing single calculation queue also handles player jobs. There is no
additional calculation pool: odds keep four estimator workers, and player jobs
run serially. **Odds requests wait behind a player update.** Health requests
remain independent. Logs report `players.load`, `players.calculate`,
`players.persist`, plus request and queue timing.

## Calculation equivalence

Two read-only comparisons used the local application database with fixed clock
`1790812800`:

1. Native reader exports evaluated by the frozen original formulas versus the
   extracted library.
2. The original Diesel readers and original formulas versus the native readers
   and extracted library. The original reference retained its SQL loaders;
   only its clock was fixed and its persistence path replaced by a sorted
   output of raw float bits.

Both comparisons matched for **41,124 players and 823,217 appearances**, drawn
from **27,278 eligible games**. Sorted outputs include offensive rating,
defensive rating and minutes before player normalization. The compact native
and original-Diesel output files were byte-identical, with SHA-256:

```text
0476d3c698065c6be5966daa428b86bab7f35a0be7ea73d33511f8b2e2479c66
```

Application-data exports and reference executables stayed outside the repository
under `/private/tmp`. There were **zero application database writes**. A synthetic
four-game fixture and frozen original raw float bits are included with the
shared-library tests, so formula parity remains reproducible without database
access. It covers substitutions, penalties, own goals, half/end-time goals,
red cards, extra time, history boundaries and all three home-field settings.

The temporary-table adapter test verifies normalized persisted player bits and
appearance values, filters, identity preservation, empty updates, both batching
ceilings, and transaction rollback after a player update when an appearance
value is invalid. It shadows all queried tables on the same connection.

The HTTP smoke test creates a uniquely named disposable schema, calls the
actual unified binary, checks updated players and appearances, checks HTTP 500
and unchanged ratings after invalid input, and checks health afterward. It
drops only its own schema. It passed.

## Odds regression benchmark

Baseline: the current Rust binary at `dd61c883`, before this integration.
Candidate: the integrated release binary. No older Go timings were used.

Five saved requests, fixed seed 808, four estimator workers. Each snapshot had
two warmups and three timed requests per implementation. Implementations ran
sequentially with alternating request order. All **25 paired full responses**
were identical, including estimates, uncertainties, proofs and work counters.

| Request | Baseline median ms | Unified median ms | Change |
| --- | ---: | ---: | ---: |
| 16498 / 44eabb47 | 655.43 | 651.38 | −0.62% |
| 16653 / 2d1c1d6f | 637.06 | 637.38 | +0.05% |
| 16653 / 71d4fea8 | 652.66 | 643.69 | −1.37% |
| 16982 / 9327edcd | 289.26 | 290.25 | +0.34% |
| 16983 / 43969b02 | 283.30 | 282.87 | −0.15% |

The largest measured increase was 0.99 ms (+0.34%). Three timed observations
per snapshot do not establish a small speedup or slowdown. This check supports
preserved odds behavior and similar standalone odds latency. Peak process RSS
ranged from 21.91–168.88 MiB for the baseline and 22.03–169.45 MiB for the
candidate; per-snapshot readings are in the accompanying JSON. No claim is made
that memory changed systematically.

Full-size player persistence, database lock contention and the resulting wait
for queued odds requests **were not benchmarked on application data**. The
read-only parity runs are not a full player-job latency measurement. Deployments
should schedule the player update with this queue behavior in mind and inspect
the new persistence/queue timing logs.

## Verification results

- Unified Rust: 40 tests passed; two explicit database tests ignored by default.
- Shared player library: 7 tests passed.
- Standalone stats: `cargo check`, build, formatting and 4 tests passed.
- Explicit player temporary-table test: passed.
- Disposable-schema HTTP smoke test: passed.
- Full Rails suite: 120 runs, 349 assertions, 0 failures, 0 errors, 1 skip.

Raw benchmark timings and sanitized verification metadata are in
[2026-09-30-unified-player-ratings.json](2026-09-30-unified-player-ratings.json).

## Reproducible commands

Run from the repository root with Rust 1.88 or newer (tested with 1.93.1):

```sh
cargo build --release --locked --manifest-path odds-rust/Cargo.toml
cargo test --release --locked --manifest-path odds-rust/Cargo.toml
cargo test --release --locked --manifest-path stats/core/Cargo.toml
cargo check --locked --manifest-path stats/Cargo.toml
cargo test --locked --manifest-path stats/Cargo.toml
cargo fmt --manifest-path odds-rust/Cargo.toml -- --check
cargo fmt --manifest-path stats/core/Cargo.toml -- --check
cargo fmt --manifest-path stats/Cargo.toml -- --check
bin/rails test

# Connection-local temporary tables; no application updates.
MYSQL_TEST_URL=mysql://root@127.0.0.1:3306/GolAberto_development \
  cargo test --release --locked --manifest-path odds-rust/Cargo.toml \
  --test player_ratings -- --ignored

# Requires mysql -u root; writes only a fresh disposable schema, then drops it.
PYTHONDONTWRITEBYTECODE=1 python3 experiments/rare_positions/test_unified_player_service.py \
  --binary odds-rust/target/release/golaberto-odds \
  --output /tmp/unified-player-http.json

# Read-only application calculation; output and optional input export stay local.
DATABASE_URL='mysql://USER:PASSWORD@127.0.0.1:3306/DATABASE' \
  cargo run --release --locked --manifest-path odds-rust/Cargo.toml \
  --example player_ratings_dry_run -- /tmp/player-bits.json 1790812800 \
  /tmp/player-input.json
```

For the odds benchmark, build `dd61c883` in a separate checkout and copy its
release binary to `/tmp/golaberto-before-unified-players`. Then run:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 experiments/rare_positions/benchmark_odds_memory.py \
  --baseline /tmp/golaberto-before-unified-players \
  --candidate odds-rust/target/release/golaberto-odds \
  --iterations 3 --output /tmp/unified-player-odds
```

## Deployment

Build from the complete repository: `odds-rust` has a path dependency on
`stats/core`. Deploy the Rails port change together with the unified binary.
Start the binary with the application's MySQL `DATABASE_URL`:

```sh
DATABASE_URL='mysql://USER:PASSWORD@127.0.0.1:3306/DATABASE' \
  odds-rust/target/release/golaberto-odds serve 127.0.0.1:6577
```

Restart the Rails processes so the service port change is loaded. Once the
unified service and Rails caller are deployed, stop the separate stats server
on 6578. No production service was restarted during this implementation.

Recommendation: use the unified binary with this coordinated deployment. The
calculation parity and persistence tests pass; the material operational change
is serialized player updates sharing the calculation queue with odds.
