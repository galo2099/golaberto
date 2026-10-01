# Rust reachability: directed GD paths, coalition branching, residual components

## Decision

Keep both new production options disabled. Directed paths broaden score construction on synthetic cases, but neither option resolved additional reference cells or changed estimates. The residual audit gives no justification for adding a production component solver on these requests.

Baseline is the current Rust estimator, including the default discrete win/draw cut, with four workers. No additional simulation allowance was assumed. No commits, pushes, or database mutations were performed.

Machine-readable paired results, input hashes, binary hashes, stages, CPU and memory measurements: [results](results/2026-09-30-rust-path-coalitions.json).

## Literature and applicability

[Bernholt et al., §4](https://image.informatik.htw-aalen.de/~thierauf/TI2/Netz/football.pdf) gives polynomial elimination for at most two remaining games per team, decomposing paths and cycles. Its champion decision permits point ties; it does not establish exact ranks under wins, GD and GF.

[Schlotter and Cechlárová, Theorem 4](https://real.mtak.hu/61819/1/mbc.pdf) gives an O(r²n) algorithm for MinStanding on directed trees under well-based scoring, with forest and degree-two extensions. MinStanding limits the number of teams strictly ahead on points. Applying that result directly to exact ranks and production tiebreakers would be unsound.

These are useful structural directions, but their assumptions must be checked after reduction.

## Implementations

### Directed-path GD completion

`RUST_ODDS_GOAL_PATHS=1` extends the existing symbolic completion. A decisive game points from winner to loser. When a tied-cohort team lacks a direct adjustable fixture against an outside-cohort team, BFS finds a forward or reverse decisive path to such an opponent.

For A → B → C, adding δ goals to each winner changes:

| Team | GD | GF | GA |
|---|---:|---:|---:|
| A | +δ | +δ | 0 |
| B | 0 | +δ | +δ |
| C | −δ | 0 | +δ |

Thus intermediate GD cancels, while GF does not. The implementation updates every affected score and verifies the complete season with the production sorter. Existing GF completion can subsequently resolve a residual tie. Away goals and other sorting effects are verified as well.

This is a constructive sufficient test, not a complete score solver. It does not solve every coupled cohort-to-cohort adjustment. Failure, integer overflow, or exhausted layouts remain unknown. There is no arbitrary margin cap used to prove impossibility. Verified seasons enter the existing witness machinery; a witness itself is not a probability estimate.

### Contradictory-coalition branching

`RUST_ODDS_COALITION_BRANCHING=1` extracts the violating team set from the discrete floor cut's residual min-cut graph. A repair must exempt at least one member of that set: exempting only other teams cannot repair its violated inequality. The early exemption search therefore branches inside the coalition.

The final implementation reuses the certificate from propagation. If ordinary propagation failed before reaching the cut, it attempts certificate extraction once as a fallback. This changes early proof branching only; constructive fixture ordering is unchanged. The cut remains points-only, so wins-based conflicts are a separate future improvement.

Limits remain 500 nodes per cell and 10,000 per enabled early direction; constructive witnesses retain 100 per cell and 3,200 total, and neighborhood search retains 10,000 candidates. No sampler draws were added or reallocated.

### Small residual components: offline audit

The new `residual_components` example fixes target extreme outcomes, propagates fixed exemption choices, removes forced fixtures, and inspects remaining constrained-team components. Components with at most eight edges are enumerated exactly against their fixed caps.

This audit does not change production labels. Feasible point-cap assignments do not establish exact-rank reachability. A contradiction under one exemption choice does not prove all choices impossible.

Across the five inputs, 98 nontrivial component instances were visited: one forest, no simple cycles, and two components with at most eight edges. Four enumerated assignments found no missed fixed-exemption contradiction. Most components were large and cyclic. Counts are repeated query components, not distinct fixtures. The audit covers visited extreme-target relaxations, not every possible search state.

## Quality and stage results

Five snapshots (16498, earlier and current 16653, 16982, 16983), six seeds (801, 804, 808, 817, 818, 911), three variants: **90 paired comparisons**. All candidate full responses were exactly equal to their paired baseline responses:

- Additional reachable cells: 0.
- Additional impossible cells: 0.
- Changes in undecided cells: 0.
- Additional or lost nonzero estimates: 0.

Synthetic forward and reverse paths succeed where direct-fixture completion fails. The exhaustive small-model test replays certificates through the production sorter.

Traced diagnostic runs, before the final certificate-reuse refinement, reduced group 16498's floor nodes from 225 to 132 at seed 808, and 232 to 136 at seed 818 (about 41%). Other active snapshots' node counts and real-data goal-completion successes were unchanged. These node reductions do not imply equivalent full-request savings: early proofs occupy milliseconds or less, whereas sampling dominates.

## Full-request performance

Final paired HTTP request wall-time sums and process CPU sums:

| Variant | Wall-time change | CPU change |
|---|---:|---:|
| Paths | +0.80% | +1.74% |
| Coalition branching | +0.59% | −1.55% |
| Both | −0.35% | −1.23% |

Small changes at this scale should not be interpreted as reliable speedups. A second persistent-server comparison used seed 808, two warmups and six timed requests per side:

| Snapshot | Baseline median ms | Both median ms | Change |
|---|---:|---:|---:|
| 16498 | 722.43 | 682.24 | −5.56% |
| 16653 earlier | 689.33 | 698.48 | +1.33% |
| 16653 current | 709.61 | 732.18 | **+3.18%** |
| 16982 | 321.82 | 319.69 | −0.66% |
| 16983 | 310.69 | 313.53 | +0.91% |

Sum of these medians changed −0.28%. There is explicitly no demonstrated latency preservation for each input. Persistent CPU includes startup and warmups; paired CPU measurements and peak RSS are in the JSON.

Persistent early-proof medians increased from 0.672 to 2.320 ms for 16498, 0.262 to 0.425 ms for earlier 16653, and 0.376 to 0.692 ms for current 16653. Extracting useful certificates can cost more than the saved nodes. Witness-stage medians decreased in these samples, but produced no extra witnesses and are insufficient evidence of a causal speedup. BFS and graph setup are included in the measured stages; neither approach starts an external solver.

## Reproduction

Preserve the current baseline executable before editing/building; the stored JSON identifies the exact baseline and candidate hashes. Temporary binaries are local experiment artifacts, not portable repository dependencies.

```sh
cp odds-rust/target/release/golaberto-odds /private/tmp/golaberto-path-baseline
cargo build --offline --locked --release --manifest-path odds-rust/Cargo.toml
python3 experiments/rare_positions/compare_rust_reachability.py \
  --baseline /private/tmp/golaberto-path-baseline \
  --candidate odds-rust/target/release/golaberto-odds \
  --reference experiments/rare_positions/reference/2026-09-30-hundredfold/reference.json \
  --output /private/tmp/golaberto-path-coalition-final \
  --baseline-variant defaults --variants goal_paths,coalition_branching,paths_coalitions \
  --seeds 801,804,808,817,818,911 --http --no-trace
python3 experiments/rare_positions/benchmark_domain_probes.py \
  --baseline /private/tmp/golaberto-path-baseline \
  --candidate odds-rust/target/release/golaberto-odds \
  --variant paths_coalitions --iterations 6 --seed 808 \
  --output /private/tmp/golaberto-path-coalition-persistent
cargo build --offline --locked --release --manifest-path odds-rust/Cargo.toml \
  --example residual_components
odds-rust/target/release/examples/residual_components REQUEST.json BASELINE-EXPORT.json
cargo test --offline --locked --manifest-path odds-rust/Cargo.toml
```

The HTTP harness runs four workers and clears inherited estimator flags. Full tests: **50 passed, two database tests ignored**. New tests cover forward/reverse paths, production-sorter replay, cut certificate validity, and coalition-search equivalence with exhaustive small outcomes.

## Recommendations

1. Retain directed paths as an opt-in constructive capability; do not enable on current coverage evidence.
2. Retain coalition branching as an opt-in experiment. It saves nodes but certificate overhead and unchanged coverage do not justify adoption.
3. Do not add a production tree/cycle solver from this audit. Revisit when reductions produce useful sparse components, and include actual rank/tiebreak validation.
4. Investigate wins-aware conflicts before spending more on GD completion for unresolved cases whose obstruction lies in points and wins. The existing offline Flamengo analysis is an example; this experiment does not integrate its proof.

Verification scope: measurements and the 50-test result apply to the hashed experiment executable/source at build time. The final workspace formatting check subsequently found an unrelated `RUST_ODDS_RANK_PROOF` expression in `proof.rs` requiring rustfmt wrapping; that separate change was left untouched. `git diff --check` passed.
