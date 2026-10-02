# Disposition of remaining Rust experiments — 2026-10-02

The user authorized deciding whether to commit or discard each remaining local
experiment. Baseline is the shipped `master` commit `31bc8b0a`. These decisions
close out the local work; they do not introduce another production search stage.

## Decisions

| Experiment | Decision | Reason |
|---|---|---|
| R33 residual conditioning loop, five variants | Discard active code and runtime flag; commit the negative results and archival patch | Final 23 pairs gained four cell-runs, but no newly discovered distinct cell across the baseline seeds. Full wall +34.81%, CPU +18.42%; Londrina/3 remained zero in every seed. |
| Late gap rescue: unconditional move and pilot guard | Discard active code and runtime flag; commit the negative results | Neither rescuer supplied a previously missing estimate. Each nine-pair arm lost four cell-runs; aggregate gains came from other stages' timing-dependent allocation. |
| High-ESS shorter point-tilt confirmations | Discard the `RUST_ODDS_POINT_TILT_FINAL=pilot_budget` option | Its historical nine-pair screen had no aggregate improvement over the retained policy, with worst full wall +56.2%. There is no validated reason to retain a second policy. |
| R34 complete exception branches | Commit offline drivers and historical evidence; retain the already shipped production implementation | Refined sampling accepted 7/11 distinct seeds; R36 subsequently measured the integration on full requests. The active server already has the useful method. |
| R35 fixed priors, bounds and ordering | Commit the offline audit, driver and evidence; retain shipped cheap ordering | Cheap bound omission saved 2.53% single-cell CPU; strong extra bounds increased CPU 9.93%. Strong bounds and omission remain offline; production gives all supported branches positive allocation. |
| Original rare50 development variants | Commit the missing evidence and report generator; retain previously shipped selected mechanisms | The existing report evaluates every arm. Rejected/deferred arms remain archival evidence, with no pending active implementation. |
| R38 certified limits and R39 enlarged Flamengo/13 prototype | Remove unpacked copies already committed in evidence archives | R38 remains enabled. R39 remains offline: its 517 ms single-cell wall and 538 MiB RSS do not justify production integration. No new decision to enable it is made. |

The observations are the original historical cohorts, not fresh performance
measurements against today's server. Negative results are retained so future
experiments need not repeat them.

## Implementation cleanup

Restored these four production source files to their released contents:

- `odds-rust/src/joint_caps/propagated/lazy.rs`
- `odds-rust/src/rare_tail.rs`
- `odds-rust/src/search.rs`
- `odds-rust/src/tilt.rs`

Removed the unreferenced active `lazy/residual.rs` source. Removed the late-gap
runtime documentation and clarified the residual loop's archival status. No
production weights, sorting rules, acceptance gates, budgets or four-worker
limits change relative to `31bc8b0a`.

The new `odds-rust/examples/branch_bounds.rs` is an offline executable. It is not
invoked by the HTTP service. Its tests check that an omission allowance applies
to the sum of omitted branch bounds and that a zero allowance retains every
positive branch. The omission allowance is relative to a supplied estimate,
not a proven true-probability lower bound. Floating point bounds are not
interval-arithmetic certificates or impossibility proofs.

## Evidence handling

Packed the remaining historical evidence under its original relative filenames:

- `2026-10-01-branch-bounds/evidence.tar.gz`
- `2026-10-01-branch-stratification/evidence.tar.gz`
- `2026-10-01-late-gap/evidence.tar.gz`
- `2026-10-01-rare50/evidence.tar.gz`
- `2026-10-01-residual-loop/evidence.tar.gz`

Each directory includes a README and a manifest with the archive hash and every
member's SHA256. Every archive was read back and verified before removing its
unpacked originals. The rejected residual loop remains reproducible from its
historical patch. The late-gap archive additionally preserves the local cleanup
snapshot against `31bc8b0a`; that snapshot is explicitly distinguished from the
earlier measured executable.

Also removed 21 unpacked R38/R39 files and the shared-constraint baseline patch
after checking they match already committed compressed evidence byte-for-byte.
No experiment evidence was lost. The unrelated `db/schema.rb` edit and local
scripts/assets are outside these decisions and remain untouched.

## Verification

- Full Rust suite: **126 passed**, four database-dependent tests ignored.
- All example tests: **9 passed**, including two new bound-selection tests.
- Total: **135 passed**, no final failures. HTTP tests passed after permitting
  local listeners; no ignored database test was enabled.
- Offline bound audit on historical group16653/Londrina3: 22 branches, 19
  retained, three omitted with total bound `3.8509489576483436e-36` under a
  `4e-36` allowance. This is an offline diagnostic, not a production estimate.
- Python tools parse; both driver CLIs load; the residual summarizer reproduces
  the historical 23 pairs, four gained cell-runs and zero losses.
- All five new archives and all 229 members verify against their manifests.
  All 22 removed duplicate files matched existing committed compressed evidence.
- All tracked production Rust source files match `31bc8b0a` byte-for-byte.
  No new timing experiment was necessary because no production code changed.

Test logs, audit output and the validation summary are committed in
`2026-10-02-experiment-cleanup/`.

Reproduction from the repository root:

```sh
cargo test --offline --locked -j4 --manifest-path odds-rust/Cargo.toml -- --test-threads=1
cargo test --offline --locked -j4 --manifest-path odds-rust/Cargo.toml --examples -- --test-threads=1
cargo run --offline --locked -j4 --manifest-path odds-rust/Cargo.toml \
  --example branch_bounds -- \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16653-71d4fea8.json \
  95 3 4e-34 0.01 strong
```

To inspect a historical evidence archive, extract it in a temporary directory,
or in its corresponding experiment directory before following that report's
commands. Historical binaries, flags and timings are not current defaults.
