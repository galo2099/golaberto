# Faster Rails /odds request construction

## Findings and implementation

`Group#odds` previously serialized 380 games with `home_power` and `away_power`.
Those methods resolve owner-dependent `home_rating` and `away_rating` scopes.
Preloading `home` and `away` does not preload these historical ratings: each game
still performs two rating SELECTs. Every reference request issued **767 SELECTs,
including 760 rating queries**. Initial runs took 2.9–3.5 seconds locally.

`OddsRequestBuilder` now fetches:

1. The persisted group fixtures using the existing group filter and ordering.
2. The latest rating date for each involved team before the first required date.
3. Those earlier ratings plus ratings inside the fixture date interval, retrieving
   only team ID, date, attack and defense scalars with `pluck`.

Sorted per-team arrays provide binary-search lookups for each game. Both power
formulas still use the original `Game#home_power` and `Game#away_power`; no math
or estimator request fields changed. This uses the existing unique
`(team_id, measure_date)` index, with no migration or persistent cache.
The request exporter uses the same builder, including its optional game dates.

A supplied `games_json` array bypasses all game/rating construction, preserving
history-backfill behavior. The builder fetches a fresh fixture relation even if
the group's games association was previously loaded.

## Measurements

Three alternating-order paired runs per group, Ruby 3.2.9 / Rails 8.1.3 and local
MySQL 9.4.0. Every request contains 380 games. The development database contains
8,181,985 historical rating rows at measurement time. Rails query caching was
explicitly disabled and schemas/autoloading warmed before measurement. Each pair
uses a read-only repeatable-read transaction so concurrent scraping/odds updates
cannot change one builder's input between comparisons. Building includes
`Group.find` through final `to_json`; it excludes HTTP and persistence.

| Group | Old median ms | New median ms | Speedup | Old / new SELECTs | Old / new rating SELECTs |
| --- | ---: | ---: | ---: | ---: | ---: |
| 16498 | 819.76 | 85.98 | 9.53× | 767 / 7 | 760 / 2 |
| 16653 | 879.11 | 50.06 | 17.56× | 767 / 7 | 760 / 2 |
| 16982 | 937.45 | 41.27 | 22.72× | 767 / 7 | 760 / 2 |

All nine pairs produced **byte-identical complete JSON**, including every game
power and fixture order. Median allocations decreased from about 378k objects to
57k, 53k and 41k respectively (85–89% reduction). Median database time fell from
590–657 ms to 20–26 ms. Both SQL work and Ruby object churn contribute.

Three final verification pairs also produced identical JSON. Their timings were
noisier (old 2.13–2.79 s, new 65–90 ms); they ran while the separate test suite was
finishing and are a correctness check, not the main performance comparison.
Local cache state and other DB activity affect timing. These are request-builder
measurements, not Rust-estimator or production Xeon measurements.

### Rejected implementation

A single game query with two correlated as-of rating joins also reduced round
trips, but took roughly 1.7–2.5 seconds uncached. `EXPLAIN ANALYZE` showed repeated
rating-history scans and sorts; comparing DATE rating columns with DATETIME game
columns introduced casts, and correlated lookups still performed poorly after
changing the date predicate. Query count alone was misleading. The bounded bulk
load removed this implementation; it does not load all historical ratings.

An earlier cached run looked much faster than the real DB work. Final measurements
therefore use `ActiveRecord::Base.uncached`. Concurrent updates changed group
`updated_at` fields during initial checks, motivating the paired consistent
snapshot. The benchmark itself makes no development DB writes and never calls
`/odds`.

## Correctness and limits

- Historical ratings use DATE, but fixtures can include a time. At exactly DB
  midnight, that day's rating is excluded. After midnight it is eligible. The
  builder always converts timestamps to UTC before deriving the exclusive DATE
  boundary. Rating selection is independent of the user/app timezone and the
  database timezone setting. This preserves the existing UTC strict-before rule.
- Missing ratings yield the same nil powers, and opponents outside the group
  are included in rating lookup when their fixtures involve a group member.
- All serialized metadata and game fields remain present. No odds, reachability
  logic, response handling, or persistence code changed.
- Memory scales with teams and the fixture date interval. Very long phases can
  load more rating rows; no full-history or cross-request cache is introduced.
- Production no longer makes hundreds of individual rating queries. Exact latency
  depends on DB distance, buffer cache and rating distribution.

## Verification and reproduction

Rails suite: **128 runs, 422 assertions, zero failures/errors, one existing skip**.
New tests exercise timestamp boundaries, application time zones, all three home
fields, played/unplayed games, missing ratings, external opponents, supplied
snapshot games, already-loaded fixtures, optional dates, and bounded query count.
Tests write only the separate `golaberto_test` database.

```sh
bin/rails runner script/benchmark_odds_request.rb 3 16498 16653 16982
bin/rails test test/unit/odds_request_builder_test.rb test/unit/group_test.rb \
  test/unit/odds_history_backfill_service_test.rb
bin/rails test
```

The benchmark's legacy path retains the original serializer for comparison. Its
JSON lines include SQL time, query counts, allocations and payload SHA256.
Results, final smoke checks, source hashes and test logs are in
[odds-request-builder/](odds-request-builder/).

Implementation: [builder](../app/services/odds_request_builder.rb),
[caller](../app/models/group.rb), [exporter](../script/export_rare_position_groups.rb),
[benchmark](../script/benchmark_odds_request.rb),
[regression tests](../test/unit/odds_request_builder_test.rb).

Enabled in `Group#odds` without extra flags. Apply the Ruby changes and restart
Rails; no Rust, JavaScript or asset build is required.
