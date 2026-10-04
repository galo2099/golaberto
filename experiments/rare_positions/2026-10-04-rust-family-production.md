# Production adoption of the late family fallback

2026-10-04. The user authorized adopting the measured R60 family configuration
in production after reviewing its CPU cost. This supersedes the earlier R60
recommendation to keep the whole family arm experimental.

## Adopted policy

The default coverage profile enables the fallback in both CLI and `POST /odds`.
No enabling flags are needed. `RUST_ODDS_FAMILY_FALLBACK=0` disables it and takes
precedence over the legacy experiment master flag. The HTTP response contract,
fixed seed 808 and four-worker ceiling are preserved.

The production extraction keeps the measured configuration:

- All eligible unresolved cells; no team or rank selectors.
- At most 3,000 native MAIN draws observed, 256 positive observations and 128
  distinct family keys. Recording consumes no RNG and preserves native results.
- Observer work admitted before sampling against audited unused rare-tail
  operation work; unknown constructor work withholds this credit.
- Native confirmation admissions, refunds, two waves and publication gates
  remain intact. Successful estimates publish before fallback consideration.
- An owned late handoff borrows the native proposals. Fallback runs after the
  final complete-branch sampler. Newly resolved cells skip fitting, replay,
  validation and sampling.
- Original per-cell unused confirmation work and the existing confirmation
  bank fund the fallback. No extra allowance or branch-stage credit is added.
- At most four learned rival points/wins families. The proposal retains 20%
  native support and weights the complete mixture, including overlapping native
  support, with score likelihood correction and the production standings sorter.
- Fresh validation and independent MAIN/CHECK samples are required. Training
  observations or reachability witnesses alone never publish probabilities.

Experimental donor transfer, neighbor modes, retry variants, alternative funding
and budget multipliers are excluded. Startup and request logs report the fixed
effective family policy.

## Validation protocol

Build the committed original baseline from
`4e812fd506b39a504dc3f771c26e8d027cff2311` in an isolated archive checkout. Compare
identical tracked request snapshots with fixed seeds, serialized processes and
four workers. The saved R60 V2 binary is the experiment reference.

Nine core requests: group 16498 seeds 808, 1669, 1993, 2281, 2293; groups 16653
and 16982 seeds 808, 2293. Require default exports to match R60 byte for byte and
explicit opt-out exports to match original production byte for byte. Also
compare native admissions and MAIN/CHECK diagnostics, fallback work ledgers,
and the late skip of team 95/3 in group 16653/2293.

Three additional snapshots, groups 15902, 16413 and 16983 at seed 808, check
preservation of all original positive estimates except request-total
`work_spent`, exact game importance and no positive-to-zero regressions. A
separate HTTP regression exercises the real default endpoint, repeatability,
explicit opt-out and the two seed-808 family gains.

Release tests run serially after compilation with four Cargo jobs. MySQL-gated
write tests stay ignored; no database is mutated. An independent Sol review
checks the combined implementation and evidence.

## Results

All nine core default exports are byte-identical to frozen R60 V2. All 12
explicit opt-out exports are byte-identical to the rebuilt original production
baseline. Explicit production `0` also wins over the legacy experiment flags.
Across all six groups, existing positive estimate records are unchanged except
request-total `work_spent`, game importance is identical, and **no positive
estimate becomes zero**.

The four measured gains are preserved: **four cell-runs, three distinct cells**,
all in group 16498. These are raw probabilities, not percentages.

| Seed | Team | Rank | Probability |
|---|---|---|---|
| 808 | Flamengo (17) | 12 | `3.451022521859592e-26` |
| 808 | Palmeiras (16) | 15 | `9.999711245204903e-34` |
| 2293 | Flamengo (17) | 12 | `5.657356956304207e-26` |
| 2293 | Palmeiras (16) | 13 | `7.594228821382229e-21` |

There are no additional gains on the other seeds or groups, and no new
reachability proofs. Remaining reachable zeros on group 16498 are 1, 4, 2, 3, 1
for the five tested seeds, matching R60. Group 16653 retains no reachable zeros
on either tested seed, and group 16982 retains all 400 positive cells.

For group 16653/2293, the native branch stage still publishes team 95/3 at
`3.884987101554226e-34`. The late fallback logs a superseded skip and spends no
fit, replay, validation or final samples on that cell.

The release build is warning-free. **151 Rust tests pass; four MySQL-gated tests
are ignored.** The real HTTP regression checks default coverage, repeated
requests, opt-out, preservation and percent conversion. Other tests check
closed-form family mixture weights, omitted-state support, bounded recording,
goal score corrections, RNG neutrality, setup accounting, flag precedence and
worker/repeat/logging invariance. Edited Rust files pass formatting checks and
`git diff --check`. Independent Sol review found no remaining correctness
blocker; a timing scope was corrected before validation.

Initial validation exposed two test-helper issues: a recorder test exceeded
its observation cap, and the HTTP helper did not decode large chunked
responses. Both are fixed; production sampling was unchanged. The first paired
analysis also used historical log field names. Its 22 failures are diagnostic
schema comparisons, while every full-export hash already matched. Preserve
that initial summary and raw evidence, then validate renamed fields against
the same hashed records with the corrected analysis. This revalidation passes:
all 70 run records and their artifacts are rehashed, native diagnostics and
fallback ledgers match R60, each work component reconciles once, and all 22
alias-only failures clear. Four Python helper tests pass. The compact committed
result is
[`v1-validated-analysis-v2.json`](2026-10-04-family-production/logs/v1-validated-analysis-v2.json).

### Warm full-request timing

Three repeats per arm and seed after separate warmups, rotating baseline/default
order. The 18 timed and six warmup exports match their paired-screen hashes.
All processes are serialized and use four workers.

| Group 16498 seed | Original wall / CPU seconds | Adopted wall / CPU seconds |
|---|---|---|
| 808 | 1.363 / 3.268 | 1.585 / 3.381 |
| 1669 | 1.228 / 3.358 | 1.327 / 3.385 |
| 2293 | 1.602 / 3.786 | 1.673 / 3.651 |
| Equal-weight mean | **1.397 / 3.471** | **1.528 / 3.472** |

Mean wall time increases **131ms / 9.4%**. CPU time is approximately unchanged
in this sample (the point estimate is +0.05%). Three repeats do not establish
that production CPU overhead vanished; retain the larger historical frozen-R60
measurements below as context. No nominal work budget or worker count was raised.

## Performance interpretation

The prior frozen R60 experiment measured group-16498 CPU increases of 8.5–9.8%
and wall-time increases of 13–16% against then-current Rust. Those observations
motivated the CPU attribution study; about half the newer measured CPU increase
was present with family sampling disabled. That native-path difference was not
isolated. The diagnostic build itself affected timing, so its phase scopes do
not exactly partition the frozen binary's overhead.

Diagnostic fallback CPU was approximately 72% MAIN/CHECK, 17% replay, and 11%
validation, fitting and other work. Moving the fallback late saves work when a
native stage resolves a candidate, but it does not make all remaining fallback
batches successful. Local timings do not establish the same speed on the
production Intel Xeon E5-2697. No live production server was accessed.

## Build and run

From the repository root:

```sh
cargo build --release --locked -j4 --manifest-path odds-rust/Cargo.toml
odds-rust/target/release/golaberto-odds serve 127.0.0.1:6577
```

Restart the existing Rust process with its current database environment. No
Rails or JavaScript build is required. To disable only this fallback:

```sh
RUST_ODDS_FAMILY_FALLBACK=0 odds-rust/target/release/golaberto-odds serve 127.0.0.1:6577
```

The paired driver is
`2026-10-04-family-production/scripts/run_adoption.py`; helper unit tests are in
the same directory. Full historical R60 comparisons require the preserved local
R60 binary and screen artifacts. Ordinary default-versus-opt-out reproduction
uses the tracked snapshots and current production binary alone.

Full local comparison command:

```sh
python3 experiments/rare_positions/2026-10-04-family-production/scripts/run_adoption.py \
  --production odds-rust/target/release/golaberto-odds \
  --old-production /private/tmp/golaberto-family-production-baseline/odds-rust/target/release/golaberto-odds \
  --frozen-r60 experiments/rare_positions/2026-10-04-family-late-fallback/data/golaberto-odds-r60-v2-late-fallback \
  --out /tmp/family-production-validation --timing-repeats 3
cargo test --release --locked --offline -j4 --manifest-path odds-rust/Cargo.toml -- --test-threads=1
```

To repeat the read-only validation of preserved local evidence:

```sh
python3 experiments/rare_positions/2026-10-04-family-production/scripts/validate_existing_run.py \
  --summary experiments/rare_positions/2026-10-04-family-production/logs/adoption-v1/summary.json \
  --out /tmp/family-production-revalidated.json
python3 -m unittest discover -s experiments/rare_positions/2026-10-04-family-production/scripts -p 'test_*.py'
```

Baseline reconstruction uses `git archive` of the baseline commit's
`odds-rust`, `stats/core` and tracked reference inputs, extracted outside the
working tree and built with `cargo build --release --locked --offline -j4`.
This avoids changing the live checkout. Tests require local loopback access.

## Provenance

| Binary | SHA256 |
|---|---|
| Rebuilt original production | `2466d4045ce8c71e48835cdb295143942fa6118f1688d71d6c4ff59aad50321f` |
| Adopted production | `87991033bdb81854cec91f3d29c32c66f5b089f51d2530590e48b2a0efc40085` |
| Frozen R60 V2 | `627726438d3d7c995397f45b77b9c487558aaa29a55f87e665106fe206b736e6` |

Local raw evidence is under `2026-10-04-family-production/logs/adoption-v1/`:
46 screen executions, six warmups and 18 timed executions. Each record retains
request, binary, export, stdout and stderr hashes, explicit environment, argv,
worker count and wall/process-CPU measurements. Large binaries and raw historical
experiment artifacts are kept locally; the adoption commit includes a compact
validation summary and reproducible scripts.
