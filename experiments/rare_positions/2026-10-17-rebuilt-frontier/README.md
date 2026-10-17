# Reconstructed rare-position estimator

This is a new implementation reconstructed from retained review history and
surviving production model code after the experimental worktrees disappeared.
It is not a byte-for-byte restoration of the former `frontier-regimes` binary.
The historical design and recovery evidence are in
`../2026-10-17-workspace-recovery/`.

The offline native package lives at `odds-rust/frontier-recovery`. It depends
on the surviving model library while keeping the frequently edited estimator
in a small separate crate. No production endpoint or default is changed.

## Fixture and recovered structure

Use `odds-rust/tests/fixtures/group-16498-2026-10-10.json`, SHA256
`b8d78c18e6ab00b20888dbb36f5f80004879549bd5307fbd8146bdc4c44cccaa`.
Target: team16 (Palmeiras), position12, zero-based rank11. Palmeiras starts
on60 points,17 wins,GD27, with nine matches left. Rival18 starts35/9 with
nine matches left. These match the retained historical case; the original
experimental request's hash is unavailable, so exact old input identity has
not been established.

The surviving exact target-total prover and reconstructed integer propagation
reproduce one eligible final target points/wins total (60/17), force all nine
target losses, and retain40 candidate exact above sets from286 raw groups.
This is a useful structural cross-check, not independent proof of old input
byte identity. Grouped points/wins roots replace the lost fixed-WDL root
representation; the sampler enforces each grouped root exactly.

## Implemented mechanisms

- Native finite goal laws account for the actual accepted int63-to-f64 grid,
  including rejection of values rounding to1. These are not substituted
  continuous Poisson probabilities or the legacy conditional-score CDF law.
- Every cohort is an exact final above-set event. Weak points/wins constraints
  can overlap, but their relaxed probabilities are never added as disjoint
  event estimates. Every fixture contributes to its likelihood exactly once.
- Selected rival constraints are conditioned jointly over shared fixtures.
  Unfinished DP branches keep a frozen native-mask fallback and a valid cap.
  Actual integer-ticket selection probabilities enter the likelihood ratio.
- Private score GD kernels are integrated independently only after shared
  tied-team matches are sampled once. Strict/inclusive GD products bracket
  GF and later tie decisions. Cache capacity only affects retention, preserving
  the same observation law within a proposal epoch.
- Adaptive sampling preserves unsampled analytic upper bounds, uses fresh
  statistics after changed proposals, and retains valid absolute bounds from
  previous epochs. Cumulative predeclared threshold counts use one-sided
  Clopper-Pearson bounds with summable cohort/epoch/look confidence allocation.
  The beta inverses use SciPy double precision with outward adjustments;
  numerical tests are not a formal proof of the special-function implementation.
- GD proposals are rebuilt once after accepting a completed positive joint
  plan during focused growth, within the remaining shared time/state budget.
  Rejected rebuilds retain the joint plan; accepted rebuilds start a fresh
  proposal epoch while preserving earlier absolute confidence bounds.

The reconstruction uses one native worker. It now also tightens analytic event
upper bounds using mandatory GD comparisons and exhaustive, bounded pivot WDL
partitions. These bounds preserve the existing sampling proposal and statistics.
The next recovery stage restores actual score-conditioned regime proposals
with native rejection sampling, fresh joint WDL atom plans and exact integer
mixture selection. Fixed scorelines are honored throughout remaining integration.
The historical bridge sampler and full GD-equality residual integrator remain
unrestored. Variable elimination replaces the
old explicit separator representation. Historical runtime and interval quality
cannot be assumed to transfer.

Large pivot partitions now have a compact path: lightweight incident WDL
assignments retain separate GD factors while sharing one remaining-fixture
joint DP forest. The existing per-assignment path remains for partitions of at
most 64 assignments. See COMPACT_PARTITION_RESULTS.md for the statistical
contract, real 156-assignment regression, and Palmeiras16th measurements.
GD_REBUILD_RESULTS.md records the subsequent controller transition and its
shared-budget comparison. Use `--no-regime-rebuilds` to retain initial GD setup
while disabling after-growth rebuilds for a control run.
UPPER_BOUND_ANALYSIS.md breaks down the retained confidence upper and the
remaining structural envelope for cohort5.
An optional `--trusted-highs` cohort prescreen trusts numerical infeasibility
without certificate verification. TRUSTED_HIGHS_RESULTS.md records its
same-budget Palmeiras16th comparison and the conditional confidence claim.

## Build and run

```bash
PATH=/workspace/onboarding/cargo/bin:$PATH \
CARGO_HOME=/workspace/onboarding/cargo \
RUSTUP_HOME=/workspace/onboarding/rustup \
CARGO_TARGET_DIR=/tmp/frontier-recovery-target \
cargo build --profile experiment --locked --offline \
  --manifest-path odds-rust/frontier-recovery/Cargo.toml

python3 experiments/rare_positions/2026-10-17-rebuilt-frontier/driver.py \
  --binary /tmp/frontier-recovery-target/experiment/frontier-recovery \
  --budget 180 --cleanup-reserve 1 --run-full-budget \
  --initial-k 7 --max-k 11 --depth 9 \
  --initial-state-cap 1000 --initial-draws 128 --max-draws 4000000 \
  --state-cap 250000 --time-ms 1000 --workers 1 \
  --output experiments/rare_positions/2026-10-17-rebuilt-frontier/pal12.json
```

The shared wall budget includes Python startup, native initialization, setup,
sampling and cleanup, best effort. Builds finish before measurement. Hashes
freeze request, native source, driver and binary. The target is simultaneous99%
confidence with an order-of-magnitude interval (`upper/lower <=10`); a midpoint
is reported as qualified only when that criterion passes without a conflict.

## Validation checkpoint

At the GD-rebuild controller freeze,30 Python tests pass; the unchanged native
code remains at its31-test optimized Rust validation checkpoint. Rust tests
cover exact shared-fixture proposal mass, actual proposal
caps, interior partial-table fallback at state caps1–3, satisfied-lower state
clamping, accepted RNG-grid boundaries, integer conversion enclosures,
conditional score GD brackets, cache invariance and the real fixture partition.
Python tests cover binomial tails including alpha1e-20/n1,000,000, layer-cake
bounds, normalized weight scaling, transactional epoch accumulation, retained
intervals and summable confidence allocation.

No historical samples are reused. Fresh measured results are recorded
separately; the original experimental measurements cannot validate this binary.
The first primary reconstruction run is preserved in `primary-pal12-180s.json`:
170.3 seconds,1,000,096 draws,eight exact zero cohorts and32 sampled cohorts;
99% simultaneous interval `[4.648436909477114e-29,1.3232625740437402e-27]`,
ratio28.47. It did not meet the target. It predates the deterministic GD caps.
The source manifest in that run omitted scheduler.py; its separately recorded
SHA256 was checked unchanged before and after measurement. Later manifests
include all Python sources. See `RESULTS.md` for the GD-cap measurements, and `REGIME_RESULTS.md` for
the subsequent conditional-proposal recovery.

## Latest recovery result

The restored conditional proposals and cost-aware scheduler reach
`[6.147750115263105e-29,3.1342415010044333e-28]`, U/L5.098, in179.152 seconds
on Palmeiras12th. This meets the simultaneous99% order-of-magnitude target
and improves on the retained historical6.386 result for the signature-matching
surviving fixture. See REGIME_RESULTS.md for comparison, validation and limits.
