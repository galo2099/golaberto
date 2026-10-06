# Rare-position experiment archive

`local/` holds compressed per-run JSON, exported real-group benchmarks, and
rejected proposal patches. It is ignored by Git because runs may contain data
derived from the local database. Keep each hypothesis in its own dated
directory. Aggregate comparison JSON lives beside the raw runs.

The benchmark and comparison commands are documented in
`doc/rare_position_experiments.md`.

The Go odds/ratings service has been removed. Historical Go/Rust comparison
tools in this archive require a frozen Go test binary; the Go experiment runner
requires earlier source checkouts containing the retired service. Saved oracle
fixtures and experiment evidence remain available without Go. Current Rust
service commands are documented in `odds-rust/README.md`. The separate
SofaScore fetcher in `go/sofascore_fetch/` is still used by Rails scraping.

The standalone `stats` service has also been removed. Its active formula
library moved from `stats/core` to `odds-rust/player-ratings`. References to
`stats/core` in dated reports and commands describe the historical source
layout; archives restored from those revisions still use that layout.
