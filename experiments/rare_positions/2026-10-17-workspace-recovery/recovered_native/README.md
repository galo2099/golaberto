# Recovered native-source records

## Scope and provenance

This directory contains only the exact successful `apply_patch` records available in the retained conversation for the adaptive controller I previously edited. They target the former isolated checkout path `/workspace/golaberto-adaptive-controller` and preserve their source-relative location under `patches/odds-rust/src/target_points_partition/`.

The first record changes scheduler ranking/decision linkage and attaches a monotone event serial to incomplete-stage decisions. The second record removes an extra event-serial increment so each incomplete stage receipt and its decision share the same serial. These are literal patch inputs, not complete module sources. They were copied as patches rather than applied because the base controller file is absent.

## Missing complete source

The complete `odds-rust/src/target_points_partition/adaptive_controller.rs`, the parent hook changes in `odds-rust/src/target_points_partition.rs`, and the frontier estimator modules are not present in the restored checkout, retained workspaces, scratch directories, or Git object database examined for this recovery. A final v1 candidate-source manifest fingerprint was retained in the earlier session:

- adaptive controller module SHA-256: `7ace82af4851703266f1bbb28ddf1be1efa85c19013c8c1de0bb4fbf1a5d48fb`
- parent hook SHA-256: `5880ce6f532cfafdaafae2c80832e4974b9d5d8f20771bd6bfca37ba8c4dc8d9`
- example SHA-256: `b9eba788ac603a9a94aa6b0c541ef3fcd58ca80a41e8e2204f137908ba1430a1`

These hashes identify missing content; they do not permit reconstruction or establish that a matching source file exists here.

## Recovered interface notes

The controller child was called after fresh parent setup by the parent `calculate_impl_config` path. The retained interface notes say the child received the model, target index and target, prepared root paths and mutable cohorts, candidate-slot count, uncovered-mass/root-cap values, seed, workers, score policy, request deadline/start time, and budget. It returned cohort rows, sampled lower mass, remaining upper mass, and an adaptive-controller receipt. The parent hook selected the new `AdaptiveController` sampling policy and replaced fixed-look confidence metadata in the response.

The full dependency contracts are missing. In particular, this recovery does not contain the exact Rust type definitions/signatures for the missing frontier estimator, its joint-growth/conditioning state, parent setup hook, or binary CLI; it also lacks the exact interfaces to the native joint-point plan, threshold-bound helpers, stage sampler, and cache-transaction failure types as they existed in the missing worktree. Recreating those files would require a new implementation, not recovery.

## Native sampling-grid caveat

The restored base still has `Poisson::native_masses()` in `odds-rust/src/sampling.rs`. It returns adjacent differences of the sampler CDF, including the sampler's forced terminal normalization. That helper is useful for the continuous CDF law but does **not** encode the exact accepted `Rng::float()` grid. The restored RNG creates a float from `int63() as f64 / 2^63` and rejects a rounded result of exactly `1.0`; those discrete rounding bins and rejection affect the native accepted-grid probabilities at CDF cutpoints. A missing frontier implementation reportedly had an exact accepted-grid path. No source or patch for that path was recovered, so `native_masses()` must not be presented as its replacement.
