# R60: Defer family fallback construction and sampling

2026-10-04. Status: isolated implementation and paired experiment completed.

## Objective

Move R59's family fallback fitting, replay, validation and fresh sampling after
all native stages. Preserve original native admissions, draws, random streams,
publication gates and successful estimates. Run fallback only for remaining
zero cells that are not proven impossible. A later native success should avoid
fallback work entirely.

R59 deferred only fallback publication. In group 16653, seed 2293, team 95's
third-place fallback consumed 141,440,918 modeled units before a later native
complete-branch estimate superseded it. Avoiding that batch is the primary
optimization target. The earlier observer cost is still incurred and charged.

## Design

Move native jobs, results, recorded family observations and the settled
confirmation bank into an owned handoff that borrows the original proposals.
Retain original cell grants and configuration. Commit native confirmation
results, run the original final branch stage, then consume the handoff. Do not
clone large native plans or results. Do not credit branch work to the fallback
bank. Charge a bounded handoff and eligibility scan from existing free capacity;
failure to fund that overhead declines fallback without changing native work.

Native training observations remain training only. A proposal is frozen before
fresh validation and independent MAIN/CHECK streams. Existing mixture-density
correction and publication gates remain in force. A reachability witness is not
a probability estimate, and a small accepted importance batch does not establish
precise magnitude. R59's Palmeiras/15th calibration limitation still applies.

## Protocol

- Current Rust: production HEAD `4e812fd506b39a504dc3f771c26e8d027cff2311`.
- Parent: frozen R59 V5 source and binary.
- Group 16498: identical saved request, seeds 808, 1669, 1993, 2281 and 2293.
- Groups 16653 and 16982: saved reference requests, seeds 808 and 2293.
- Four workers per estimator; builds, tests and estimator runs serialized.
- Compare all baseline-positive estimate fields except request-total
  `work_spent`, exact game importance, native admission and sampling metrics,
  parent coverage, classifications and remaining zeros.
- Audit handoff/filter fees, observer costs, inherited bank limits and late
  sampling work separately. Count each cost once.
- Measure full-request wall and child user/system CPU after separate warmups,
  with alternating arm order. Verify timed export hashes against paired screens.
- Production source is unchanged during this experiment. No commit, push or
  database mutation is authorized by this request.

The deterministic operation model schedules work; it is not a wall-time or CPU
guarantee. Compare to current Rust and R59, and report every latency increase.
Local macOS arm64 timings do not establish production Xeon speed.

## Evidence and result

The final R60 V2 paired screen passes across all nine requests. All four R59
gains survive, including their estimate fields apart from request-total work.
All baseline positives, native confirmations, game importance and default-off
exports are preserved. There are no new reachability proofs or additional
zero-to-positive results relative to R59.

| Group | Seeds | Baseline reachable zeros | R59 reachable zeros | R60 reachable zeros | Undecideds |
|---|---|---|---|---|---|
| 16498 | 808, 1669, 1993, 2281, 2293 | 3, 4, 2, 3, 3 | 1, 4, 2, 3, 1 | 1, 4, 2, 3, 1 | 0 |
| 16653 | 808, 2293 | 0, 0 | 0, 0 | 0, 0 | 0 |
| 16982 | 808, 2293 | 0, 0 | 0, 0 | 0, 0 | 0 |

Group 16498 retains 17 impossible zeros per run; group 16653 retains 48.
Group 16982 has all 400 cells positive and no native/fallback stage. Its funding
and admission comparisons are explicitly not applicable, supported by the
empty search logs and full export.

For group 16653/2293, the later native complete-branch sampler still publishes
team 95/3 at `3.884987101554226e-34` (raw probability). R60 then logs
`superseded_by_later_native_stage` and performs no fallback attempt, replay,
fit, validation, MAIN or CHECK for that cell. R59 spent 141,440,918 modeled
units; R60 incurs only its 192-unit request handoff fee, for a net reduction of
141,440,726 units. The previously paid observer and dispatch work remain charged.

On the five group-16498 runs there are no later-native skips, and the late
fallback charges exactly the same work as R59. Handoff adds 320, 384, 320, 320
and 384 units respectively. Group 16653/808 adds 64 units without fallback work.
These are deterministic operation charges, not measured CPU cycles.

Source review found no sampling or native preservation defect. It found two
late summary fields using a pre-observer-settlement snapshot in the alternate
confirmation-certificate funding mode. V2 corrects those diagnostics to the
settled reservation before and after the handoff fee. The measured stage-funded
arm is unaffected. Initial and final screens and source versions are preserved.
V2 repeats all nine paired comparisons with identical results.

## Warm full-request timing

Five rotating-order repeats per arm after separate warmups. Group 16498's CPU
difference warranted a second, balanced three-repeat block. The table combines
all eight repeats for its three timing seeds, with equal weight per seed. The
group-16653 rows use five repeats each. No simultaneous estimator, test or build
processes; every estimator uses four workers. All **102 timed and 24 warmup
exports** match their fixed paired-screen hashes.

| Request | Current Rust wall / CPU seconds | R59 wall / CPU seconds | R60 wall / CPU seconds | R60 vs R59 wall / CPU |
|---|---|---|---|---|
| 16498, seeds 808/1669/2293, eight repeats | 1.569 / 3.848 | 1.844 / 4.114 | 1.826 / 4.224 | −0.9% / **+2.7%** |
| 16653, seed 808, five repeats | 1.653 / 3.375 | 1.655 / 3.393 | 1.478 / 2.990 | −10.7% / −11.9% |
| 16653, seed 2293, five repeats | 1.182 / 2.840 | 1.308 / 2.975 | 1.158 / 2.750 | **−11.4% / −7.6%** |

The target request, 16653/2293, saves 149ms against R59. It is 24ms/2.0% faster
than current Rust, with 3.2% lower CPU time in this local sample. Across both
16653 seeds, R60 averages 1.318s versus R59's 1.481s, −11.0% wall and −9.9% CPU.

Group 16498 has no eliminated fallback sampling. Its added handoff fees are
tiny, but measured CPU time does not improve. The first five-repeat block is
R60 +0.2% wall/+3.7% CPU versus R59; the independent three-repeat block is
−3.1% wall/+0.6% CPU. The combined CPU increase is 110ms. The whole R60 family
arm remains **257ms/16.4% slower than current Rust**, with **9.8% more CPU time**.
This does not satisfy the production latency constraint for that group.

There are also no eliminated samples on 16653/808. Its observed speedup cannot
be attributed to skipping team95/3. Code layout, allocation lifetime and local
scheduling may contribute; this experiment does not isolate those effects.
Treat the exact removed work and preserved exports as established evidence,
and the local speedup as measured rather than a fleet-wide or Xeon guarantee.
Group 16982 was screened, with no fallback/native work; it was not warm-timed.

## Validation and accounting

- Full R60 suite before the reporting-only V2 correction: **170 passed,
  four MySQL-gated ignored**. The initial sandbox run could not bind the two
  local HTTP test ports; the rerun with temporary loopback permission passed.
- Final V2 formatting, release check and release build pass. It retains one
  unused raw snapshot binding warning; the raw diagnostic is already emitted
  in the native summary. No production source or database was changed.
- Separate Sol source review: native work and gates preserved; V2 closes the
  alternate funding diagnostic finding. Only the two planned source files
  change relative to R59.
- All seven applicable final stage ledgers pass. The two no-work group-16982
  cases are explicitly N/A. Native recorder outputs and original MAIN/CHECK
  metrics remain identical; the handoff fee adds no native admissions.
- Every final complete export is identical to R59 after removing only
  `work_spent` from estimate records. Default-off exports are byte-identical
  to current Rust. No regressions, additional proofs or new gains versus R59.
- Per-stage and combined work checks count the native work, observer, dispatch,
  handoff and fallback exactly once. Final branches contribute no credit to
  the frozen fallback bank. Historical `work` includes legacy draw work;
  distinguish it from `actual_operation_work` and late modeled operation costs.
  The inherited reservation/projection limitations described in R59 persist.

The alternate certificate-funding smoke run uses group 16498/2293. Its raw
pre-settlement reservation is 885,011,408, including a held observer bound of
2,031,424. Actual observer collection is 771,904. The settled pre-handoff
reservation is 883,751,888; adding the 384 fee gives a late-entry reservation of
883,752,272. Its 127,784,374 late units yield final reservation 1,011,536,646.
V2 reports these quantities separately and does not double-charge the observer.

## Recommendation

Retain R60 as the successor to R59's family-fallback experiment. Running fallback
after native completion avoids work that is known to be unnecessary and
preserves all measured coverage and estimated values. **Do not enable the whole
family approach in production yet:** group 16498 still exceeds current latency
and its CPU difference merits further work. R60 introduces no new precision
evidence for the tiny Palmeiras estimates.

The next useful performance target is the remaining paid replay and family
fit/validation cost on group 16498, together with the failed batches on seeds
1669 and 2281. Reallocate that work while preserving successful native estimates
and the four measured gains. Avoid increasing the nominal allowance to hide
latency. No commit, push or production promotion occurred.

## Reproduction and artifacts

Artifact root: `experiments/rare_positions/2026-10-04-family-late-fallback/`.
Final paired evidence: `logs/v2-screen-{16498,16653,16982}` and the matching
`logs/v2-analysis-*.json`. Warm evidence: `logs/v2-timing-16498`,
`logs/v2-timing-16498-repeat`, `logs/v2-timing-16653`. Initial `screen-*` artifacts
are the pre-correction version, retained for provenance.

Baseline binary SHA256:
`6084a83dabd40d34c69e4b8b341b785221249986f1ec5302b097978533b39e80`.
Parent R59 binary SHA256:
`1098fa98e657023af7a3df7d1dbb64b3b453f8c2dfdf038130e440f52d1246e0`.
Final R60 V2 binary SHA256:
`627726438d3d7c995397f45b77b9c487558aaa29a55f87e665106fe206b736e6`.

Saved under `data/`:

| Artifact | SHA256 |
|---|---|
| `r60-v2-late-fallback-source.tar.gz` | `d3284e5af67bd5e47972891159f7c836b1a6ebf4da221cdc602544e393366637` |
| `r60-v2-late-fallback-source.members.sha256` | `6c9729aa5f906832c15c4328af96e6c097dfcb0f5a993a9c4a4566fb7683301d` |
| `r60-v2-late-fallback-vs-r59-v5.patch` | `24ab1c1ba3555d4b335c2ef71fdeb106c1c716994496dd4fc5fa0017798be5e9` |
| `r60-v2-late-fallback-provenance.json` | `94c9dfb5e244f20bb99dbac31cf95be31604e6563c96b2bd9d43fbc1d52ae52c` |

The saved binary is `golaberto-odds-r60-v2-late-fallback`, with the binary hash
above. All 62 regular archive members verify against the manifest. The archive
contains standalone Rust source, unchanged integration tests/fixtures, the
`stats/core` dependency and required reference inputs; it excludes build caches.
The patch changes only `odds-rust/src/rare_tail.rs` and
`odds-rust/src/rare_tail/confirmations.rs` and applies cleanly to frozen R59 V5.
Use the complete source archive for recovery; the delta is not a production-HEAD
patch. Provenance records the toolchain and the exact source revision of each
validation check.

To restore source and rebuild:

```bash
mkdir -p /private/tmp/r60-restored
tar -xzf experiments/rare_positions/2026-10-04-family-late-fallback/data/r60-v2-late-fallback-source.tar.gz \
  -C /private/tmp/r60-restored
cd /private/tmp/r60-restored/odds-rust
cargo build --release -j 4
```

Example commands from the repository root; use fresh output paths:

```bash
task_baseline=/private/tmp/golaberto-r57/baseline/odds-rust/target/release/golaberto-odds
task_parent=/private/tmp/golaberto-r59-v5-deferred/odds-rust/target/release/golaberto-odds
task_candidate=/private/tmp/golaberto-r60-v2-late-fallback/odds-rust/target/release/golaberto-odds
task_scripts=experiments/rare_positions/2026-10-04-family-late-fallback/scripts
task_request=experiments/rare_positions/2026-10-03-family-conditioning/data/group-16498-44eabb47.json

python3 "$task_scripts/run_screen.py" \
  --baseline "$task_baseline" --parent "$task_parent" --candidate "$task_candidate" \
  --request "$task_request" --out /private/tmp/r60-repro-screen
python3 "$task_scripts/analyze_screen.py" \
  --screen /private/tmp/r60-repro-screen --out /private/tmp/r60-repro-analysis.json
python3 "$task_scripts/run_timing.py" \
  --baseline "$task_baseline" --parent "$task_parent" --candidate "$task_candidate" \
  --request "$task_request" --screen /private/tmp/r60-repro-screen \
  --out /private/tmp/r60-repro-timing --repeats 5
```

For breadth, substitute the saved group 16653 or 16982 reference request and
pass `--seed-list 808,2293` to the screen. The harness clears inherited
`RUST_ODDS_*` and `RARE_POSITION_*` settings, uses four workers, records exact
commands/environment/hashes, and refuses existing output directories.

Manual alternate-funding smoke command (stage-funding flag deliberately unset):

```bash
env -i RUST_ODDS_LOG=1 \
  RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK=1 \
  RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE=all \
  RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE=family \
  RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP=3000 \
  RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING=1 \
  "$task_candidate" estimate "$task_request" /private/tmp/r60-certificate.json 2293 4
```

Parent design, statistical background, measurements and reproducible archives:
[R59](2026-10-03-rust-family-native-training.md). This scheduling change uses
the same probability estimator; it introduces no new statistical assumption or
solver proof rule.
