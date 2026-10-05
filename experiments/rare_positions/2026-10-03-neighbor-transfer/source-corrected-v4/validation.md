# Source-corrected v4 validation

## Seed 808 gate diagnosis

The `both10` recipient is group 16498, team ID 17, rank 12. It used 46,949 main draws and had `main_batch_gap = 1.7444975838292338`, above the unchanged publication limit of 1.5. That is the failing rule. Relative SE was `0.35823047917187983` and passed the rough-publishability limit (`rough_publishable_rules = true`); ESS `7.791339055229838` passed the minimum of 4; max event weight share `0.1894180856804311` passed the maximum of 0.35. The check stream was skipped after main publication failed. No quality gates were changed.

Raw event and export:

- `/private/tmp/golaberto-r54/v4-both10-808.time`
- `/private/tmp/golaberto-r54/v4-both10-808.json`

The both10 export SHA256 is `ba53c95bf121ade76c849210248521969194eadb957dc8bab44e87e04d7d6b46`, matching the frozen v3 both10 export byte-for-byte. The v4 native10 export also matches v3 byte-for-byte (SHA256 `9d80ba431fb2a2226f6e62a7a3c2da6c88f522ee21d6336249e991d9a35fe6c5`).

## Rust validation

Working directory: `/private/tmp/golaberto-r54/native`.

The full Rust suite was run serially with four Cargo build jobs, excluding only the two localhost HTTP tests on the first sandboxed pass:

```sh
cargo test --manifest-path Cargo.toml -j 4 -- --test-threads=1 \
  --skip active_routes_chunked_json_errors_and_health_work_over_http \
  --skip incomplete_upload_does_not_block_health_or_ready_calculations
```

Result: pass. The library suite reported 88 passed; estimator integration reported 35 passed; joint-caps reported 4 passed; player-ratings reported 2 passed and 3 database-dependent tests ignored; rank-proof reported 1 passed; ratings reported 4 passed; work-budget reported 2 passed; doc tests reported 0 tests. HTTP was filtered in this pass. The work-budget worker/logging determinism test completed successfully (the two work-budget tests took 206.80 seconds). The release build following the test run succeeded in 60 seconds.

The two HTTP tests were then run with localhost binding permitted:

```sh
cargo test --manifest-path Cargo.toml -j 4 --test http -- --test-threads=1
```

Result: 2 passed, 0 failed. No database tests were run against a database; three DB-dependent cases remained ignored as configured by the suite.

## Six-run invariants

Command:

```sh
python3 /Users/robsonaraujo/work/golaberto/experiments/rare_positions/2026-10-03-neighbor-transfer/invariants.py \
  /Users/robsonaraujo/work/golaberto/odds-rust/target/release/golaberto-odds \
  /private/tmp/golaberto-r54/candidate-target/release/golaberto-odds \
  /Users/robsonaraujo/work/golaberto/experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  --output-dir /private/tmp/golaberto-r54/v4-invariants --execute --cell-selector 17:12
```

Result: pass. Flag-off export matched production. Active exports matched across workers 1 and 4 and logging off/on. All six initial Monte Carlo hashes were identical. The complete structured result is `/private/tmp/golaberto-r54/v4-invariants/invariants.json`.

Multiplier 10 with transfer mode unset, and native10 without a selector, each exported the production baseline exactly (SHA256 `28a6d61d3466cfb706b358e09df77047bd1e8c82ed7ffc929af741eb439c1604`).

## Binary and patch

- Candidate binary SHA256: `47a4ac2fbd699a2e2673f94b1204a1e3c3a2327a68822ec357436e40f9d5c0be`
- Patch SHA256: `014aa1db4163e6abeee1ec4a0a97a9575f3be982f0292b1f3d8914e9d35e8b0e`
- Source manifest SHA256: `95f49527fd677f421843b0604abc5042574115eb8a06f37925c8721c91a61225`
