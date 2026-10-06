# AGENTS.md

Guidance for coding agents working in this repository.

## Model Roles and Subagent Coordination

Use **GPT-6.1 Sol (`gpt-6.1-sol`) for reasoning and coordination** and
**GPT-6 Luna (`gpt-6-luna`) for all code generation** in this repository.
This section explicitly authorizes delegation to multiple subagents for work
within the user's requested scope.

### Role Boundaries

- Run the main coordinating agent with `gpt-6.1-sol`. Sol owns investigation,
  architecture, task decomposition, debugging decisions, test strategy, code
  review, integration decisions, and communication with the user. Any additional
  agents assigned these responsibilities must also use `gpt-6.1-sol`.
- Delegate every source-code edit to a `gpt-6-luna` implementation subagent,
  including tests, migrations, scripts, configuration, refactors, and fixes
  requested during review. Sol should describe the required behavior and
  constraints rather than authoring a patch for Luna to copy. Sol may edit prose
  documentation and run read-only inspection or validation commands directly.
- Luna implements Sol's bounded specification and runs assigned checks. If a
  task requires an unresolved design choice, changes an agreed interface, or
  exposes an unexpected failure, Luna reports the evidence to Sol for a decision
  before continuing the affected work. Luna must not spawn further agents.
- This is a division of responsibilities; it cannot prevent Luna from doing
  internal reasoning while implementing code.

### Delegation Workflow

1. Sol inspects the working tree and relevant call sites, defines acceptance
   criteria, and divides implementation into bounded tasks. Even a small code
   change goes to one Luna worker. Use multiple workers when independent tasks
   can proceed safely in parallel; keep dependent changes sequential.
2. Select the model explicitly on every spawn: `gpt-6.1-sol` for investigation or
   review, `gpt-6-luna` for implementation. Use the session's subagent tools,
   rather than creating separate user-facing chats. If the spawn tool has a
   `fork_turns` option, use `"none"` with a self-contained brief or a supported
   bounded history fork when overriding the model; full-history forks may
   inherit the parent's model and reject overrides.
3. Give each worker the objective, relevant files and findings, permitted edit
   paths, interface contracts, acceptance criteria, required checks, and expected
   report. Include applicable repository instructions when context is not
   inherited. Require a report of changed files, checks and results, and remaining
   uncertainties.
4. Assign one writer per file at a time. Agents share the working tree: preserve
   existing user changes, avoid overlapping edits, and do not revert another
   worker's changes. For coupled Rails/Go JSON or Go/Rust rating changes, Sol
   establishes the common contract before workers begin. Reserve shared files
   such as routes, schema, and fixtures for a single designated worker.
5. Track task ownership, dependencies, and status. Respect the runtime's
   concurrency limit, reuse workers for follow-up fixes, and use messages and
   completion waits to coordinate. When a contract changes, inform affected
   workers before allowing dependent work to continue.
6. Sol reviews the combined diff and test evidence, sends all required code
   corrections back to Luna, and runs the relevant integration checks after
   workers finish. For larger or riskier changes, use a separate Sol reviewer
   and the full test suite. Finish with the outcome, validation, and any limits.

### Runtime Requirements

`AGENTS.md` supplies instructions, not model configuration: select GPT-6.1 Sol
for the main chat in the client or launcher. Explicit subagent model selection
must also be available in the running session. If either requested model or the
delegation tools are unavailable, report the limitation and continue only work
that can honor these roles; do not silently substitute models or generate code
with Sol. Higher-priority runtime instructions still apply.

## Project Snapshot

- Application: `Golaberto` (Ruby on Rails).
- Auxiliary service: Rust HTTP service in `odds-rust/` for odds and ratings.
- Scraping helper: Go SofaScore API fetcher in `go/sofascore_fetch/`.
- Player-rating formulas: local Rust crate in `odds-rust/player-ratings/`.
- Rails config: `config/application.rb` (app defaults are legacy-compatible).
- Database: MySQL (`mysql2` adapter in `config/database.yml`).
- Tests: Minitest with fixtures under `test/`.

## Repository Layout

- `app/models`: domain models (teams, players, games, championships, etc.).
- `app/controllers`: controller layer (legacy naming includes singular controllers like `team_controller.rb`).
- `config/routes.rb`: mixed modern + legacy routes with a catch-all route at the end.
- `odds-rust/`: unified HTTP service for championship odds and team/player ratings.
- `go/sofascore_fetch/`: standalone SofaScore API fetcher used by Rails scraping.
- `odds-rust/player-ratings/`: player-rating formulas and regression fixtures used by the unified service.
- `lib/`: important Ruby modules, helpers, and rake tasks used across the app.
- `db/`: schema and migrations.
- `test/`: `unit`, `functional`, `system`, fixtures, and test helpers.

## Setup and Run

Use project scripts first:

```bash
bin/setup
```

If needed, run commands individually:

```bash
bundle install
bin/rails db:prepare
bin/rails server
```

Run the unified Rust service for odds and player ratings:

```bash
cargo build --release --locked --manifest-path odds-rust/Cargo.toml
odds-rust/target/release/golaberto-odds serve
```

Player-rating updates use `POST /player_ratings` on port 6577 and require
`DATABASE_URL` (MySQL) in the service environment.

## Test Commands

Run targeted tests for the files you changed:

```bash
bin/rails test test/unit/player_test.rb
```

Run full suite before finalizing larger or riskier changes:

```bash
bin/rails test
```

Legacy rake tasks are also available:

```bash
bin/rake test
```

## Coding Conventions for This Repo

- Keep changes focused and avoid unrelated refactors.
- Follow existing Ruby/Rails style in surrounding files (including older syntax where already used).
- Preserve legacy routing behavior:
  - Add new routes above the final catch-all route in `config/routes.rb`.
- Keep controller/model naming consistent with existing patterns in this codebase.
- Respect localization defaults (`pt-BR` with fallback locales) when adding user-facing strings.
- Add or update tests when behavior changes.
- Do not commit secrets, credentials, or environment-specific files.

## Rust Odds/Rating Service Notes

- Service entrypoint: `odds-rust/src/main.rs`.
- Build with `cargo build --release --locked --manifest-path odds-rust/Cargo.toml`.
- Run `odds-rust/target/release/golaberto-odds serve`.
- The service listens on `localhost:6577`.
- Key endpoints used by Rails:
  - `POST /odds` (championship odds simulation)
  - `POST /spi` (team power/rating calculations)
  - Also exposed: `/eval`, `/historic_ratings`, `/player_ratings`
- Rails has direct call sites to this service (for example in `app/models/group.rb` and controllers like `team_controller.rb` and `championship_controller.rb`).
- If you change request/response JSON shapes in the Rust service, update all Ruby call sites in the same change.
- Run `cargo test --release --locked --manifest-path odds-rust/Cargo.toml` for service changes.
- The former Go odds/ratings service has been removed. Saved Go oracle fixtures remain for regression tests; historical comparison tools require an earlier checkout or a frozen Go test binary.
- The separate Go SofaScore fetcher remains in `go/sofascore_fetch/`, with its own module. `bin/setup` and deployment build it into `bin/sofascore_fetch`; retain its browser TLS profile when changing transport behavior.

## Rust Player Ratings Notes

- Formulas: `odds-rust/player-ratings/src/lib.rs`.
- Database adapter and persistence: `odds-rust/src/player_ratings.rs`.
- The standalone Diesel `stats` service has been removed; the unified service uses its native MySQL driver.
- Reads match/player/goal/historical rating data and upserts computed ratings into:
  - `players` (`off_rating`, `def_rating`, `rating`)
  - `player_games` (`off_rating`, `def_rating`)
- Uses `DATABASE_URL`; do not commit local database credentials.
- Preserve the player formulas' legacy float arithmetic and verify the frozen output fixture when changing formulas.
- Run `cargo test --release --locked --manifest-path odds-rust/player-ratings/Cargo.toml` for formula changes, plus the unified service tests for integration changes.

## Important Ruby Files in `lib/`

Treat `lib/` as production code in this repository. Key files include:

- `lib/poisson.rb`: Poisson distribution utility used by models.
- `lib/lttb.rb`: timeseries downsampling utility (covered by `test/unit/lttb_test.rb`).
- `lib/authenticated_system.rb`: authentication mixin required by `ApplicationController`.
- `lib/geo_clusterer.rb` and `lib/google_url_signer.rb`: geospatial/map helpers used by views.
- `lib/image_upload.rb` and `lib/paperclip_processors/logo.rb`: image upload and processing logic.
- `lib/scrape.rb`: external data ingestion/scraping and normalization routines.
- `lib/tasks/*.rake`: operational rake tasks (for example `odds_history:backfill`).

When editing `lib/` files:

- Search for all call sites (`require` and method/module usage) and update them together.
- Add/update tests when possible; at minimum run affected tests/tasks.
- Be careful with encoding and locale-sensitive strings in scraping/normalization code.

## Data and Schema Changes

When changing persistence behavior:

- Add a migration in `db/migrate`.
- Keep migrations reversible when practical.
- Update tests/fixtures affected by schema or data-shape changes.

## Pre-merge Checklist

- [ ] Relevant tests were added or updated.
- [ ] Targeted tests pass locally.
- [ ] Full test suite run for high-impact changes.
- [ ] Routes and legacy endpoints were reviewed for regressions.
- [ ] No secrets or local artifacts were introduced.
