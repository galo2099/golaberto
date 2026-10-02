# Flamengo / 13th: certified paths and complete rank cases — 2026-10-02

## Finding and recommendation

Yes: Flamengo / 13th, group 16498, has the same useful **certified target-total
limit** as the Palmeiras / 16th investigation. Its next bottleneck is larger.
The current production constructor stops before producing a complete proposal.
A focused offline prototype produces accepted order-of-magnitude estimates in
**5/5 distinct seeds**, around **1.6e-34 to 4.7e-34 probability**. These are
importance-sampling estimates, not certified probabilities or golden truth.

Do **not** enable the expanded limits in production. One fresh-process seed808
experiment costs **517 ms wall, 639 ms CPU**, with **538 MiB peak RSS**. It is
one cell, not a full request. The corresponding current full-request log skips
this cell's branch constructor in approximately **1.2 ms**. Replacing that skip
with this prototype would add substantial work unless other work is removed.
No full-request paired latency or coverage-regression comparison was performed
for this offline arm. No additional production allowance is assumed.

Production Rust source, defaults, quality gates and service binary are unchanged
in this investigation. The generic diagnostic example, this report and archived
prototype accompany the certified-limit release. The expanded construction caps
remain offline. No deployment or MySQL mutation was performed.

## Request and necessary target constraint

Request: `reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json`.
Its games, scores and powers previously matched a read-only current DB export.
SHA256: `2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625`.

Flamengo is team17, with **60 points, 18 wins, 10 matches remaining**. Wins are
the phase's second sorting key. The native certificate's upper packed total is
1815, with stride29. A total of62 points and18 wins is1816, so62 points or more
cannot produce13th. The certificate takes200 search nodes and about0.6 ms.

Therefore Flamengo must either:

- Lose every remaining match, finishing on60 points and18 wins; or
- Draw exactly one match and lose the other nine, finishing on61 points and18 wins.

There are exactly **11 target outcome paths**. Their combined prior probability
is **2.2772722599753912e-7**. This is a necessary-event mass and hence an upper
bound on13th, **not its estimate**. The individual pre-certificate points screen
admits2.722791905756469e-5. The final rank requires an extremely unlikely joint
season for the other teams too.

## Why the current proposal fails

For each of the11 paths, root propagation forces four teams strictly below
Flamengo and leaves15 rivals ambiguous in packed points/wins. At13th, seven
teams must be below it after full sorting. The necessary packed-rank model
therefore permits at most three additional strictly-below exceptions.

The existing raw mask generator tries
`C(15,0) + C(15,1) + C(15,2) + C(15,3) = 576` combinations per path, before
checking their shared fixtures. This exceeds its256-case limit. Across all
paths there are6,336 nominal combinations. The current fixed-seed808 service
log says `case enumeration budget exhausted`; it does not claim impossibility.

The complete-parent fallback cannot repair an incomplete primary union. It only
retains a primary union which was already constructed completely before optional
refinement failed.

## Focused generic experiments

All proposal code uses a requested team/rank and applicable scoring rules;
there are no Flamengo-, team- or rank-specific production conditions.

| Arm | Outcome |
| --- | --- |
| Current certified constructor | Stops at raw rank-case enumeration; no estimate |
| Incremental shared-fixture propagation at every mask prefix | 6,336 combinations reduced to409 non-refuted leaves; all11 paths complete |
| Pruning with existing guide and joint limits | Encounters a zero joint mass; after a support proof safely removes that case, stops at4-million guide-value cap after33 attempted cases |
| Offline larger setup caps plus support checks | Completes407 strata after proving two case subsets infeasible; all five seeds accepted |

Prefix pruning visited10,251 nodes and took **58.83 ms wall on four workers**;
individual path times sum to about234 ms. It currently compiles suffix and
simulation metadata at each feasible prefix even though only feasibility is
needed there. This is an optimization target.

The407 strata are a complete partition of the necessary event after two
logically impossible case subsets were excluded. They are **not407 proven
reachable cases**. Sampled scores are checked with the production sorter; the
usual goal importance weights are retained. Shared raw fixture probabilities
and joint normalizers are accounted for exactly once.

### Floating-point zero versus infeasibility

A zero joint probability can represent an empty support or numerical underflow.
The offline prototype repeats the same selected necessary rival subset with
uniform positive probabilities over its allowed fixture outcomes. It preserves
zero-probability outcomes as disallowed and relaxes other rival bounds.

For at most256 fixtures, a supported complete assignment in this uniform model
has mass at least `3^-256`, far above floating-point underflow. A completed zero
therefore proves that necessary subset incompatible. A positive result or
exhausted quota does not prove feasibility and does not permit deleting the case.
Changing the selected rivals during this check can miss the contradiction, so
selection is confined to the original necessary subset.

Tests cover a shared-game contradiction and an actual product underflow with
positive uniform support. Prefix tests enumerate all81 outcomes of a small
fixture graph and verify that every complete rank assignment survives, and that
quota exhaustion is reported as incomplete.

### Expanded offline limits and sampling

- Prefix enumeration:20,000 nodes shared across the cell.
- Case caps:1,024 per target path and1,024 total, raised from256.
- Guide cap:100 million values, raised from4 million. Construction had already
  reached34.5 million charged guide values before its370th case.
- Joint-node limit:100,000, unchanged; completed setup used17,296 nodes including
  the support reference checks.
- No optional secondary refinement.
-25 pilot draws per stratum for each of rank and bound guidance; freeze the
  selected guidance and variance-based allocation before independent main/check.
-40,000 draws in **each** main/check sum, four cores total. Every stratum receives
  at least four draws. No branch probability mass is omitted.
- Existing aggregate main/check quality gates are unchanged. Pilot and floor
  settings differ from production's500 pilots and200-draw ordinary floor. A
 407-stratum union cannot fit production's25k final total with that ordinary floor.

## Probability and timing results

One frozen proposal, five distinct independent seed pairs:

| Seed | Main probability | Check probability | Main ESS | Check ESS | Accepted | Sampling wall ms |
| ---: | ---: | ---: | ---: | ---: | :---: | ---: |
|808|1.9584e-34|2.2692e-34|33.33|40.86|Yes|137.92|
|1669|4.6727e-34|2.7919e-34|7.60|19.24|Yes|150.31|
|1993|2.9941e-34|7.9498e-34|60.30|9.86|Yes|75.45|
|2281|3.2164e-34|3.7558e-34|45.59|19.16|Yes|76.46|
|2293|1.6084e-34|1.7246e-34|58.43|89.57|Yes|167.17|

Setup wall time was346.17 ms for that five-seed process, excluding the separately
measured approximately0.6 ms certification and1.4 ms Model construction. End-to-
end five-seed process:1,016 ms wall,1,434 ms CPU,546 MiB peak RSS. One separately
measured fresh seed808 process:517 ms wall,583.47 ms user +55.84 ms system CPU,
538 MiB peak RSS. These are local-machine observations, not production Xeon
measurements or a proposed allowed latency increase.

Only one cell was investigated. No additional reachability/impossibility cell
classification was obtained: Flamengo13th was already proved reachable. The two
new infeasibility results eliminate **case subsets**, not entire team/rank cells.
Five accepted runs represent **one distinct offline estimated cell**, not five
new cells. No other-group quality claim follows from this experiment.

## Next production candidates

1. Avoid compiling simulation suffix metadata while checking rank-case prefixes.
2. Add an explicit discrete support check so an impossible necessary case can be
   removed without treating every floating-point zero as a proof.
3. Reuse individual-team suffix rows across similar cases. Full-guide caching
   already exists; it does not share a complete guide when a case changes its
   variable fixture set or conditioned probabilities.
4. Alternatively partition rank cases incrementally, preserving complete parent
   regions when further splits exceed setup quotas. A parent must include every
   unsplit descendant; a selected subset of completed leaves is insufficient.
5. Measure those candidates against the current Rust full endpoint, fixed seeds
   and four cores, with work transferred from existing low-yield confirmations.
   Require coverage comparisons across all seven snapshots before changing defaults.

The confirmed bottleneck is **setup**, not too few target-season draws. Increasing
pilot sizes before fixing setup would not make the production proposal admissible.

## Reproduction and files

Current working implementation is the baseline from
[the certified-limit report](2026-10-02-rust-certified-target-limits.md), with
service binary SHA256
`806f576dec367bf7b618eedd853372bd9fdb9023e8e9928ed25f023761d0e821`.

Generic diagnostic, no server or DB access:

```bash
cargo run --release --offline --locked -j4 \
  --manifest-path odds-rust/Cargo.toml --example certified_paths -- \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  17 13 10000
cargo test --offline --locked -j4 \
  --manifest-path odds-rust/Cargo.toml --example certified_paths
```

To reproduce the enlarged offline arm, copy the current `odds-rust/Cargo.toml`,
`Cargo.lock`, `src/`, `examples/`, `stats/core/`, and reference request into the
same relative layout in a temporary directory. Do not copy build directories.
Apply `2026-10-02-flamengo-13/offline-prototype.patch` to that copy with `patch -p1`.
The patch is relative to this turn's working source, including the preceding
certified-limit implementation, rather than the older committed master.

```bash
cargo run --release --offline --locked -j4 \
  --manifest-path /private/tmp/flamengo13-replay/odds-rust/Cargo.toml \
  --example branch_stratification -- \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  17 13 40000 808,1669,1993,2281,2293 trained adaptive 4 tilt plain 25 4
cargo test --offline --locked -j4 \
  --manifest-path /private/tmp/flamengo13-replay/odds-rust/Cargo.toml \
  --lib uniform_support_reference_tests
```

Artifacts: `2026-10-02-flamengo-13/summary.json`, `pruned-cases.json`,
`offline-prototype.patch`, `offline-raw.tar.gz`, and targeted test logs.

Before applying the offline prototype patch or reading the targeted test logs,
extract `2026-10-02-flamengo-13/text-evidence.tar.gz` in that directory. Those
files are archived under the relative filenames used in the commands above.
