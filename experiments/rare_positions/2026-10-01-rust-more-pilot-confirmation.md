# R32: spend more samples on proposals with positive pilot evidence

## Result and recommendation

Yes. Extra independent confirmation helps both proposals that were not funded
and proposals whose ordinary final batch had too few hits. Larger batches found
Londrina/4th in all five tested seeds. Some weak proposals still have highly
concentrated weights and need better proposals rather than simply more samples.

The initial recommendation was to retain this as an opt-in. After reviewing
these results, the user authorized making the larger option **the default**
and shipping it. The comparisons below describe the pre-enablement experiment:

- Cheapest already-shipped adjustment: `RUST_ODDS_RARE_TAIL_BUDGET_FRACTION=0.75`.
  On nine pairs, +11 nonzero cell-runs, zero losses, +5.95% CPU, +6.15% wall time.
  It allocates more work to the existing portfolio, not exclusively larger finals.
- New modest confirmation: `RUST_ODDS_RARE_TAIL_CONFIRM_MORE=0.25`.
  +7 cell-runs, zero losses, +3.56% CPU, +5.35% wall time on nine pairs.
- New larger confirmation: `RUST_ODDS_RARE_TAIL_CONFIRM_MORE=1.5`.
  +34 cell-runs across 23 pairs, **14 distinct cells**, zero losses,
  +27.01% CPU and +32.05% aggregate wall time. Reachable-zero cell-runs fell
  from 64 to 30. The affected groups cost substantially more than that aggregate.

Do not combine these flags based on these measurements: their combination was
not tested. The coverage profile now defaults to confirmation1.5; explicit
`RUST_ODDS_RARE_TAIL_CONFIRM_MORE=0.25` reduces the extra allowance and `0`
disables it. Rebuild and restart to use the new default. No database writes or
production access were used.

## Baseline and method

Baseline is the shipped Rust implementation at
`ee334cfb9b42075189cd22ad2fa598e94312a7ee`, default coverage profile, tail fraction
0.45, four workers. Baseline executable SHA256:
`9b76149a66bda051c1ae7dc07f4497bee1b1542cca2a59d4e1ea2863cf45aee3`.
Measurements were serialized locally on the ARM Mac, using Rust 1.93.1 release
builds. No ARM-specific optimization was added. These are saved requests, not
fresh exports of the current database.

The pilot cohort uses seeds 808, 1669 and 1993 for current 16498, current 16653,
and historical 16653. Validation uses new seeds 1847 and 1861 on all seven
reference snapshots: 15902, 16413, 16498, current/historical 16653, 16982, 16983.
The existing harness clears inherited estimator flags and alternates the order
of candidate/baseline full HTTP requests. Only one calculation runs at a time.
Wall time covers the full HTTP request; CPU is child-process user plus system
time, including process startup/shutdown. These are cold-service measurements.

Input hashes and executable hashes are in the compressed summaries. The
candidate was built from isolated baseline source plus `rare_tail.rs`; unrelated
uncommitted late-gap changes in the workspace were excluded. The archived patch
also contains a subsequent timing-log correction and retention of unfunded
plans when ordinary retries are explicitly disabled; neither changes the
benchmark's default `after` sampling path.

## Paired results

Each gain is a team/rank in one request going from zero to an accepted nonzero
estimate. Repeated gains for the same cell across seeds count separately.

| Configuration / cohort | Pairs | Gains | Distinct gained cells | Lost | CPU increase | Full wall increase |
|---|---:|---:|---:|---:|---:|---:|
| Existing tail fraction 0.75, pilot cohort | 9 | 11 | 10 | 0 | 5.95% | 6.15% |
| New confirmation 0.25, pilot cohort | 9 | 7 | 6 | 0 | 3.56% | 5.35% |
| New confirmation 1.5, pilot cohort | 9 | 20 | 12 | 0 | 35.90% | 43.71% |
| New confirmation 1.5, fresh validation | 14 | 14 | 10 | 0 | 17.72% | 20.38% |
| New confirmation 1.5, combined | 23 | 34 | 14 | 0 | 27.01% | 32.05% |

Percentages use summed costs within each cohort, not averages of percentages.
The cohorts are different, so the validation percentage must not be interpreted
as the larger option becoming cheaper. Complete groups dilute aggregate cost.

### Affected groups: larger option across five seeds

| Snapshot | Mean CPU baseline → candidate | Mean full wall baseline → candidate | Reachable zeros baseline → candidate, by seed |
|---|---:|---:|---|
| 16498 `44eabb47` | 1,869.95 → 2,884.31 ms (+54.25%) | 681.61 → 1,016.98 ms (+49.20%) | 808: 8→5; 1669: 14→5; 1993: 7→4; 1847: 10→5; 1861: 13→6 |
| 16653 `71d4fea8` | 1,586.17 → 2,084.71 ms (+31.43%) | 614.04 → 1,002.20 ms (+63.22%) | 808: 2→1; 1669: 3→1; 1993: 3→1; 1847: 2→1; 1861: 2→1 |

Historical 16653 and the four other reference groups had no eligible remaining
zeros and no coverage changes. Their cost differences were small/noisy; for
example validation 16982 averaged 253.15→249.55 ms, with all 400 cells positive.
The extra confirmation stage does not run if the tail has no remaining cells.

### Examples: probabilities are fractions, not percentages

For group 16653, Londrina/4th received **30,000 new main draws and 30,000
independent check draws** per seed, compared with ordinary finals capped at
6,000 main draws in this profile:

| Seed | Main hits / check hits | Main estimate | Check estimate | Accepted |
|---:|---:|---:|---:|---|
| 808 | 56 / 59 | 7.12e-23 | 6.85e-23 | Yes |
| 1669 | 72 / 72 | 7.40e-23 | 1.83e-22 | Yes |
| 1993 | 63 / 67 | 7.80e-23 | 6.84e-23 | Yes |
| 1847 | 43 / 41 | 1.78e-22 | 1.54e-22 | Yes |
| 1861 | 79 / 83 | 7.88e-23 | 1.59e-22 | Yes |

This is useful order-of-magnitude agreement, not evidence of high precision.
Londrina/3rd remains zero in all five larger runs. Seed 1993 had one pilot hit,
then 13 hits in 30,000 new draws, ESS 1.25, with one sample carrying 89% of the
weight. Its independent check was **skipped after main rejection**, not sampled
and found zero. Seeds without a positive pilot do not get this extra stage.

At seed 808, Palmeiras/13th became 4.53e-20 after 12,370 main and check draws;
Flamengo/11th became 3.21e-20 after 5,404 each; Athletico/19th became 3.15e-22.
At seed 1993, a previously filtered Flamengo/12th pilot (ESS 1.32) was rescued
by 24,247 draws per batch, yielding 1.03e-25 with independent check 2.32e-25.
Palmeiras/14th and /15th, Flamengo/13th, and Londrina/3rd still have proposal
quality/allocation limitations. More draws did not reliably cure them.

All distinct paired gains in the larger cohorts:

- 16498: Athletico/17–20; Fluminense/20; Palmeiras/11–13;
  Flamengo/10–12; Chapecoense/4.
- Current 16653: América-MG/11; Londrina/4.

## Implementation and correctness

After the normal tail and its ordinary retry finish, freeze existing positive
estimates. Retain already-built plans for unfunded cells, rejected finals, and
weak positive-hit pilots. For remaining zeros with finite positive ESS, choose
both new batch sizes from the pilot:

```
n = clamp(ceil(max(120 * pilot_draws / pilot_hits,
                   32 * pilot_draws / pilot_ESS)), 2000, 30000)
```

Sort by measured forecast sampling cost, with a cell-index tie break. Fund jobs
on at most four workers from an additional allowance equal to the new flag
times measured pre-tail calculation time. Forecast uses a 1.2 safety factor.
Check allowance again before starting each job. This is approximate admission
control, **not a guaranteed deadline**. Some funded jobs can still be skipped.

Sample a fresh fixed-size main batch on a dedicated deterministic stream. Run
the independent check batch only if main passes. Pilot and earlier failed-batch
observations do not enter either new estimate. No refitting, new search, target
mean approximation, team/rank-specific code, or relaxed quality gate is added.
The existing weighted samplers and production sorter determine target hits.
Witnesses continue to establish reachability separately from probability.

Current order-quality main gates remain: ≥30 hits, ESS≥4, relative SE≤0.6,
maximum weight share≤0.35, batch gap≤1.5, and positive weighted probability.
Independent check requires ≥30 hits, ESS≥3, relative SE≤0.75, and the existing
main/check consistency interval. Selection/publication gates still introduce
selection effects; independent confirmation does not establish unbiasedness or
replace comparison to a high-work probability reference.

The new stage only fills zeros. All 23 larger requests logged
`preserved_existing_estimates=true` before reconciliation. It accepted 36 cells
within its own ordinary prefixes, versus 34 net paired gains. Those quantities
differ because wall-clock funding can alter ordinary prefixes between separate
requests even at fixed seeds. No paired positive cell, reachability status, or
impossibility proof was lost; game importance was identical. Some existing
probabilities differed between paired requests (maximum absolute difference
4.72e-12), so do not claim separate requests were byte-identical.

## Reproduction and artifacts

Preserve a baseline binary from the specified commit, then build the candidate:

```sh
cargo build --release --locked -j4 --manifest-path odds-rust/Cargo.toml
cp odds-rust/target/release/golaberto-odds /tmp/r32-candidate
python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline /tmp/r32-baseline --candidate /tmp/r32-candidate \
  --production-candidate --flag RUST_ODDS_RARE_TAIL_CONFIRM_MORE=1.5 \
  --cases 16498,16653 --seeds 808,1669,1993 --output /tmp/r32-pilot
python3 experiments/rare_positions/benchmark_propagated_joint.py \
  --baseline /tmp/r32-baseline --candidate /tmp/r32-candidate \
  --production-candidate --flag RUST_ODDS_RARE_TAIL_CONFIRM_MORE=1.5 \
  --seeds 1847,1861 --output /tmp/r32-validation
```

For the smaller confirmation comparison, replace the flag value with `0.25`.
For the existing-portfolio comparison use the **baseline binary for both arms**
and `--flag RUST_ODDS_RARE_TAIL_BUDGET_FRACTION=0.75`.
Do not run these benchmarks concurrently with builds or tests.

Artifacts are in `2026-10-01-more-pilot-confirmation/`: compressed JSON summaries,
paired response/log archives and `candidate.patch.gz`. Tests cover invalid
allowances, zero/invalid pilots, hit/ESS-driven sizing, capped batch sizes,
independent confirmation gates and preservation of existing metadata.

The full Rust suite on isolated candidate source passed **99 tests**, with the
two DB-write tests left ignored; no failures. This includes the HTTP endpoints,
rating oracles, proof and estimator regressions. `rustfmt --check` passed for
the changed Rust file. Full output is archived as `tests.log`.
The final rebuild after the timing-log correction was checked on three seed-808
pairs: four gains, no losses, Londrina/4th still accepted. This follow-up is
archived separately as `final-check` and excluded from the cohort totals above.
`metadata.json` records both candidate executable hashes.

### Default enablement

After user authorization, the coverage profile defaults to confirmation1.5.
Startup logs identify `rare_tail_confirm_more`; explicit0 disables just this
stage, explicit0.25 reduces its allowance, and disabling the coverage portfolio
continues to disable it. No endpoint/response shape or four-core limit changed.
The full Rust suite passed100 tests, with2 DB-write tests ignored. Added tests
verify default activation, explicit overrides and runtime preservation.
`default-tests.log` and `shipment.patch.gz` record these release checks and changes.
The rebuilt release was then compared without candidate flags on all seven
saved snapshots at seed808: four gained cell-runs, zero losses, no proof
regressions and identical game importance. Londrina/4th was accepted with the
default setting. The affected requests took679.39→1013.76ms (16498) and
682.43→1093.10ms (current16653). `default-check` archives these seven follow-up
pairs separately; they do not change the earlier cohort totals.

## Next useful work

The larger option is now the default following explicit user authorization.
Use the 0.25 override for inexpensive coverage when the approximately one-second
local full requests of the larger option are undesirable. Neither option removes every reachable zero.
For the remaining concentrated-weight pilots, improve rival constraints or
proposal mixtures before multiplying sample counts again. Preserve independent
confirmation and the already-published estimates when evaluating that work.
