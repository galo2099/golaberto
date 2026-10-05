# R60 CPU attribution

2026-10-04. Diagnostic experiment completed; production source is unchanged.

## Question and protocol

Where does R60's previously measured 9.8% increase in CPU time go? That result
was group 16498, seeds 808/1669/2293: current Rust 3.848 CPU seconds versus
R60 4.224, an additional 376ms per request. CPU time sums work across threads;
it is distinct from request wall time and CPU utilization.

Compare five arms: frozen production baseline, frozen R60 with family flags
off/on, and an instrumented R60 with those flags off/on. Use the identical saved
request and three seeds, four workers, five balanced order rotations, separate
warmups, and serialized estimator processes. No builds or tests overlap runs.
All **75 timed and 15 warmup exports** match the earlier fixed screen hashes.
There are no changed estimates, coverage counts or classifications.

The diagnostic clone adds opt-in `getrusage(RUSAGE_SELF)` readings at stage
boundaries, covering process user and system CPU, including worker threads.
There are no timing syscalls inside draws. Parallel confirmation waves are
bracketed as whole waves; wave 1 includes both MAIN and conditional CHECK work.
Nested scopes are inclusive: never sum a parent and its children. Late fallback
is sequential, so its per-cell phase scopes are disjoint and can be summed.

## Full-request controls

Means over 15 timed requests per arm:

| Arm | CPU seconds | Wall seconds |
|---|---:|---:|
| Current production Rust | 4.603 | 2.167 |
| Frozen R60, family off | 4.795 | 2.225 |
| Frozen R60, family on | 4.993 | 2.454 |
| Diagnostic R60, family off | 4.649 | 2.138 |
| Diagnostic R60, family on | 4.862 | 2.387 |

The new frozen-binary comparison is **+390ms CPU / +8.5%** and
**+288ms wall / +13.3%** versus production. It is consistent with a material
increase, but the earlier 9.8% is not a fixed cost independent of local runtime
conditions.

Two paired contrasts explain the observed increase at a coarse level:

- Frozen R60 with family disabled versus production: **+192ms CPU**, or 4.2%
  of production CPU. This is an observed experimental-build regression in
  existing work; it occurs without fallback replay, fitting or final sampling.
- Enabling family work in that same frozen R60 binary: **+198ms CPU**, or 4.3%
  of production CPU (4.1% relative to its own flag-off control).

Thus roughly half of the measured increase appears without enabling the new
family work. Its cause is not isolated. Source review found no duplicated
constructor search. A larger native sampler with optional family/recorder
branches and compiler code layout are hypotheses, not established causes.

The diagnostic build's flag-off CPU is 146ms lower than frozen R60. Adding
instrumentation changes the compiled program, beyond the boundary syscall
overhead. Its enabled/disabled delta is 213ms, versus 198ms in frozen R60.
Use its scope readings to locate family computation, not to assign an exact
portion of the historical 376ms increase to every function.

Timing variation is substantial: the paired CPU contrast standard deviations
are 205ms for frozen-off versus production and 357ms for frozen-on versus
frozen-off. Across seed means, the first contrast is +143 to +232ms; the second
is +63 to +296ms. These local arm64 measurements do not establish Xeon timings.

## Where family computation goes

Mean process CPU in the diagnostic family-on arm:

| Disjoint fallback task | CPU ms/request | Share of fallback CPU |
|---|---:|---:|
| Fresh MAIN and CHECK | 175.8 | 71.8% |
| Replay training draws | 41.3 | 16.9% |
| Fresh validation | 15.8 | 6.4% |
| Family fitting | 4.9 | 2.0% |
| Clone | 0.4 | 0.2% |
| Other fallback bookkeeping, logging and cleanup | 6.7 | 2.7% |
| **Inclusive fallback total** | **244.8** | **100%** |

MAIN is 86.0ms; CHECK is 89.7ms. Each family draw reconstructs complete outcome
totals and evaluates the native and learned proposal densities to obtain the
correct mixture weight. This can replay an alternative pattern's probability
calculation. It is more work than a plain native draw; removing those density
calculations without an equivalent weighting method would change the estimate.

Replay repeats a bounded native prefix when its observations could not be
funded and retained. Seeds 808 and 1669 replay 3,000 draws per attempted cell;
2293 reuses native observations and has effectively zero replay CPU. Native
observation collection itself checks draws and aggregates bounded positive-hit
families; it is paid even for a later superseded cell.

The metered constructor calls the constructor once. Its extra modeled setup
units mostly account for pre-existing work. Actual additions include node
counters and scans of guide row lengths; modeled units are not CPU cycles.
The diagnostic ordinary setup/pilot delta is only +1.1ms, and the aggregate
native confirmation delta is -3.6ms in this sample. The latter is a measured
offset, not evidence that recording speeds confirmation.

| Seed | Inclusive fallback CPU ms | Outcome |
|---|---:|---|
| 808 | 386.8 | Flamengo/12 and Palmeiras/15 accepted |
| 1669 | 181.7 | Palmeiras/14 rejected; no additional estimate |
| 2293 | 165.8 | Flamengo/12 and Palmeiras/13 accepted; Palmeiras/15 declined before sampling |

The 1669 failed batch accounts for about one quarter of mean fallback CPU in
this cohort. Fallback time is larger than its net on/off CPU difference because
existing-stage timings also vary. In the diagnostic pair, the search delta is
212.3ms, the rare-tail delta 253.3ms, and the measured pre-tail offset -41.0ms.
Do not reinterpret this offset as work deliberately saved by family fallback.

## Recommendation

Investigate the native-path performance regression before merging family code.
Separating the family sampler/observer from the ordinary hot loop is a useful
hypothesis to test with identical outputs. For enabled work, fresh MAIN/CHECK
dominates; reuse probability calculations while preserving the full mixture
density, then reduce paid replay. Better admission of low-yield fallbacks is
another target: the failed Palmeiras/14 batch costs 182ms here. Fitting alone
is too small to recover the overall CPU increase.

No optimization, production integration, commit or push is performed by this
attribution experiment. Existing production and user changes are preserved.

## Validation and reproduction

Diagnostic formatting, offline release check/build and two CPU helper tests
pass. The harness passes three binary metadata tests, byte compilation and
CLI checks. Independent Sol review confirms the arithmetic, scope nesting and
attribution limits. All 63 regular source archive members match their manifest;
the patch applies cleanly against frozen R60 V2.

Raw evidence is under `2026-10-04-family-cpu-attribution/logs/`:
`cpu-attribution-runs/summary.json`, per-run records/exports/stderr, and
`analysis.json`. The scripts record request, binary, stdout, stderr and export
hashes, explicit flags and child user/system CPU.

```bash
python3 experiments/rare_positions/2026-10-04-family-cpu-attribution/scripts/run_cpu_attribution.py \
  --baseline /private/tmp/golaberto-r57/baseline/odds-rust/target/release/golaberto-odds \
  --frozen-r60 /private/tmp/golaberto-r60-v2-late-fallback/odds-rust/target/release/golaberto-odds \
  --diagnostic-r60 /private/tmp/golaberto-r60-cpu-attribution/odds-rust/target/release/golaberto-odds \
  --request experiments/rare_positions/2026-10-03-family-conditioning/data/group-16498-44eabb47.json \
  --reference-screen experiments/rare_positions/2026-10-04-family-late-fallback/logs/v2-screen-16498 \
  --out /private/tmp/r60-cpu-new-runs --workers 4 --repeats 5

python3 experiments/rare_positions/2026-10-04-family-cpu-attribution/scripts/analyze_cpu_attribution.py \
  --runs /private/tmp/r60-cpu-new-runs --out /private/tmp/r60-cpu-new-analysis.json
```

Use fresh output paths. Family flags match R60's stage-funded all-scope
configuration, with a training cap of 3,000; the diagnostic arms additionally
set `RUST_ODDS_DIAGNOSTIC_CPU=1`. Inherited estimator flags are cleared.

Saved artifacts under `2026-10-04-family-cpu-attribution/data/`:

- Diagnostic binary SHA256: `39a02a9ef6656ae2281a231357a7d5dea9c5139fd8230fc713d1ad6c9f2d9c0b`.
- Source archive: `r60-cpu-attribution-source.tar.gz`, SHA256
  `9f638ac83acb1ec00746258be58e16b0f3073ea123140ce828b8dc5417fae4be`.
- Member manifest SHA256:
  `cdaf731223f2c2385f1a0f67a47560bfad332fcfa972580617b6e9df56bf8502`.
- V2-relative patch: `r60-cpu-attribution-vs-r60-v2.patch`, SHA256
  `cb536920323dc7e99827a004056cb390881a9bb9264e24dd1c27b3f623c1cdb5`.

The source archive restores `odds-rust/`, `stats/core/` and required reference
inputs. Rebuild with `cargo build --release -j 4 --offline` from its `odds-rust`
directory using the existing cached dependencies. Changes are diagnostic only:
Cargo manifests, API/search/rare-tail scope wrappers and the new CPU helper.
