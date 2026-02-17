# AGENTS.md

Guidance for coding agents working in this repository.

## Project Snapshot

- Application: `Golaberto` (Ruby on Rails).
- Rails config: `config/application.rb` (app defaults are legacy-compatible).
- Database: MySQL (`mysql2` adapter in `config/database.yml`).
- Tests: Minitest with fixtures under `test/`.

## Repository Layout

- `app/models`: domain models (teams, players, games, championships, etc.).
- `app/controllers`: controller layer (legacy naming includes singular controllers like `team_controller.rb`).
- `config/routes.rb`: mixed modern + legacy routes with a catch-all route at the end.
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
