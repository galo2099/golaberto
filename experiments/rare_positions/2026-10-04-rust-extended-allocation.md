# R67: extended-work diagnosis of the three lost estimates

## Conclusion

The loss is explained by **unstable proposal learning and allocation, amplified
by sampling randomness**. The original fixture's zero-free result at seed 808
was not a reliable accuracy benchmark. There is no evidence here of a
team-specific rule hardcoded to that fixture, but there is direct evidence that
the generic adaptive proposals overconcentrate on the few families observed
during training.

Extended runs recover all three requested cells on both retained fixtures.
Thirty-six high-work MAIN/CHECK pairs pass the existing publication gates,
including 18 pairs using fresh final seeds. These are provisional probability
references, not exact probabilities or certified bounds.

**All probabilities in this report are fractions, not percentages.** Multiply
by 100 to obtain a percentage. A production zero below means no estimate was
published; all three cells were already proven reachable.

| Cell | Original production, seed 808 | Updated production, seed 808 | Extended original: mean MAIN / CHECK | Extended updated: mean MAIN / CHECK |
|---|---:|---:|---:|---:|
| Flamengo / 13th | 1.173e-33 | 0 | 7.56e-34 / 7.49e-34 | 8.82e-34 / 8.48e-34 |
| Palmeiras / 15th | 1.000e-33 | 0 | 4.86e-31 / 5.18e-31 | 5.43e-31 / 5.42e-31 |
| Palmeiras / 16th | 1.683e-40 | 0 | 3.01e-40 / 2.92e-40 | 3.01e-40 / 3.43e-40 |

The extended columns average six independent final streams per arm, each with
an independent CHECK, under a fixed training setup. They are descriptive means,
not a new production estimate or a confidence interval. Both fixtures have
similar probability scales. The larger concern is Palmeiras/15th: its old
published value understates the extended evidence by approximately **500-fold**.

## Fixtures, executables and scope

Only these two group-16498 requests are retained as the regression pair:

| Fixture | Remaining games | SHA256 |
|---|---:|---|
| Original: `reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json` | 103 | `2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625` |
| Updated: `reference/2026-10-04-two-results/inputs/group-16498-two-results.json` | 101 | `cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee` |

The updated request includes game 364007 at 1–2, game 363999 at 1–0, and 200
refreshed team-rating-derived mean fields. The comparison does not separate
the causal effect of each result from the rating changes; it investigates the
estimator's failure on the combined request. Neither target's own starting
points, wins or ten remaining fixtures changed.

Frozen production baseline: `e3ce9653`, executable
`2026-10-04-early-rank/data/golaberto-odds-tree-production-v2`, SHA256
`ae0e68a5d5986436656ca34aa6b6b6a9f54befeaaf96049dfbc0df96ab6bbdf1`.
The offline example used here has SHA256
`6e8677056849273aced877240e29c59cdba908e5fa4011510b7b90ec19f0205f`.
The previously built main binary remains SHA256
`eab52f6ff920b5bcaf7e010e056eaf0cf3892f01d84d58347b5b1fd39b3612ea`.

New source edits in this investigation are confined to
`odds-rust/examples/branch_stratification.rs` and the offline runner
`2026-10-04-extended-allocation/scripts/run_matrix.py`. Production-source
experiments from R65 remain in the working tree, default-off. This investigation
introduces no endpoint/default change, database mutation, commit or push.

## Experimental controls

All heavy runs were serialized. Each example uses four workers total, with
the existing Rust proposal sampler, importance weights, goal-score sampling,
production sorter and publication gates. No old Go timing or one-second
allowance is used.

The runner clears inherited `TREE_*`, `RUST_ODDS_*` and `RARE_POSITION_*` controls.
It records the exact command, explicit environment, input and executable hashes,
output hashes, process wall time and child user-plus-system CPU time.

Construction stays at seed 808. `TREE_PILOT_SEED=808` freezes pilot and message
training independently of final sampling. Final seeds are 808, 1669 and 1993;
fresh validation uses 4093, 4099 and 4111. Final MAIN and CHECK streams are
independent of training and each other. Final observations never feed back
into proposal selection or allocation. The runner always sets bound mode
`off`: **no leaf is omitted and no reference-probability cutoff is used**.

| Cell | Offline proposal | Initial draws per final stream | Initial pilot draws per leaf | Final floor | Message setting |
|---|---|---:|---:|---:|---:|
| Flamengo / 13th | Complete tree, cap 64 leaves; 54 original / 52 updated leaves | 3,000 | 25 | 10 | 1 |
| Palmeiras / 15th | Complete late-style tree, cap 256 leaves, generic rank priority; 139 original / 138 updated leaves | 6,000 | 5 | 2 | 3 |
| Palmeiras / 16th | Complete secondary-refined branches; 18 original / 17 updated branches | 25,000 | 500 | 200 | 0 |

The Palmeiras/15th tree deliberately bypasses the production admission/bank
constraints for diagnosis. It is not equivalent to the original published
family fallback. The other two initial MAIN results reproduce their production
sampling diagnostics at seed 808. Offline CHECK also runs when production would
skip it following a rejected MAIN or empty pilot.

Compare final-only increases, larger pilots at the same final draw count, and
equal versus empirical-variability allocation with positive floors. The larger
reference configurations use:

- Flamengo/13th: 300,000 final draws and 2,500 pilot draws per leaf.
- Palmeiras/15th: 600,000 final draws and 500 pilot draws per leaf.
- Palmeiras/16th: 2,500,000 final draws and 5,000 pilot draws per branch.

Thus “10×” and “100×” refer to **per-cell final draws**, not entire endpoint
work or latency. Larger pilots are stated separately. Draws have different
costs across proposals; equal draw counts do not establish equal CPU or modeled
work. An optional prior allocator was added for reproducibility of R65, but
the panels reported here use `trained` or `equal`.

## Production seed controls: the old success was already fragile

These are existing full-request baseline exports from R65, with all experiments
off. The construction, training and final streams vary together in this table.

| Fixture / seed | Flamengo / 13th | Palmeiras / 15th | Palmeiras / 16th |
|---|---:|---:|---:|
| Original / 808 | 1.173e-33 | 1.000e-33 | 1.683e-40 |
| Original / 1669 | 8.155e-34 | 0 | 0 |
| Original / 1993 | 7.424e-34 | 0 | 2.687e-40 |
| Original / 2281 | 1.018e-33 | 0 | 0 |
| Original / 2293 | 5.792e-34 | 0 | 3.030e-40 |
| Updated / 808 | 0 | 0 | 0 |
| Updated / 1669 | 0 | 0 | 1.472e-40 |
| Updated / 1993 | 8.343e-34 | 0 | 6.186e-41 |
| Updated / 2281 | 0 | 0 | 2.445e-40 |
| Updated / 2293 | 0 | 0 | 1.775e-40 |

Original Palmeiras/15th succeeds at only one of five seeds. This is evidence
against treating its one nonzero result as a robust reference, before changing
the request at all. The experiments below isolate training from final noise.

## Flamengo / 13th: missed training dominates final noise

With updated request, frozen pilot 808 and 25 draws per leaf, all three
3,000-draw pairs fail. A tenfold final-only increase still fails all three.
Even 300,000 final draws with the same sparse pilot fails all three pairs:
occasional large weights appear, but remain unreliable.

Changing **only the pilot seed to 1993**, retaining 25 pilot draws and the same
three final seeds and 3,000-draw budget, makes all three pairs pass:
MAIN `5.873e-34`–`1.056e-33`, CHECK `7.867e-34`–`1.303e-33`. Pilot seeds 1669,
2281 and 2293 behave like 808 and fail all three. This isolates a causal
training/proposal effect; it is not merely final-stream randomness.

More training is not sufficient unless it discovers the consequential family:

| Updated, pilot seed 808 | 250 pilots / 30,000 finals | 2,500 pilots / 300,000 finals |
|---|---:|---:|
| Message-selected branch | 41 | 26 |
| Target results in selected branch | One draw against Grêmio; nine losses | Ten losses |
| Branch 26 pilot hits after training | 0 | 987 |
| Branch 26 final draw allocation | 67 | 270,118 |
| Branch 41 final draw allocation | 26,598 | 587 |
| MAIN probability, seed 808 | 9.253e-40 | 8.804e-34 |
| CHECK probability, seed 808 | 1.291e-39 | 8.167e-34 |
| Pair accepted | Yes | Yes |

The 250-pilot configuration passes two of three pairs near `1e-39`, while
effectively missing the all-loss family. The larger configuration is roughly
**950,000 times** the accepted seed-808 estimate. This is a particularly useful
counterexample: agreement and observed ESS can look adequate when both final
streams miss the same dominant component.

Branch 26's original joint teams are Vasco, Coritiba, Bragantino and São Paulo.
The successful message retuning uses Vasco, Coritiba, Botafogo and São Paulo.
The three branch exceptions are Corinthians, team 584 and Vitória. Flamengo
ends at 60 points and 18 wins. The sampler still checks actual ranks, including
goal tiebreakers; these point/win constraints alone are not the event.

At the high configuration, all six pairs per fixture pass. Updated MAIN is
`8.523e-34`–`9.418e-34`; MAIN ESS is 305–826 and CHECK ESS 581–980. Original
MAIN is `7.101e-34`–`8.040e-34`. Equal allocation with the same larger pilot also
passes all three original-seed updated pairs, with MAIN near `1e-33`.

**Overconcentration:** scarce message refinement and almost all final draws
follow the first sampled family; zero-hit but consequential branches receive
little discovery work. Final-only multiplication does not reliably repair this.

## Palmeiras / 15th: the old learned families omit important alternatives

The original production fallback trains on 19 hits, training ESS 3.006, and
builds four full rival-status patterns. All four require Corinthians to finish
**strictly below Palmeiras in the packed points/wins total**. They share the
strict-below set Chapecoense, Corinthians, Grêmio, Internacional and Remo;
their other packed ties differ. This learned event is narrower than “Palmeiras
finishes 15th.”

The complete tree exposes alternatives even at the nominal 6,000-draw budget.
At the larger pilot/final configuration, three all-loss strata dominate:

| Original request, final seed 808 | Variable team below Palmeiras, alongside the common below teams | MAIN contribution | CHECK contribution |
|---|---|---:|---:|
| Branch 51 | Corinthians | 9.960e-32 | 1.069e-31 |
| Branch 53 | Vasco | 1.635e-31 | 2.150e-31 |
| Branch 54 | Vitória | 1.999e-31 | 2.065e-31 |

Illustrative first-hit outcome vectors have Chapecoense, Grêmio, Internacional
and Remo below Palmeiras, with Corinthians, Vasco or Vitória supplying the
fifth team. Palmeiras loses all ten games and ends at 57 points, 16 wins.
Several other teams tie that packed total, so sampled goals and the production
sorter remain essential.

The missing learned-component coverage is structurally verifiable. In branches
53/54, Corinthians is selected on the non-strict side of the `below` split:

```text
Tree branches 53/54: Corinthians packed total >= Palmeiras packed total
All four old families: Corinthians packed total <= Palmeiras packed total - 1
```

These conditions cannot overlap. The same metadata occurs on both fixtures.
Branches 53 and 54 contribute about **78% of measured MAIN** and **79% of
measured CHECK** in original/808. Those proportions are sampled contributions,
not exact fractions of true probability. Branch 51 may overlap the learned
families and requires further tie-pattern analysis.

This proves missing coverage in the learned family components, **not missing
overall proposal support**: the defensive native mixture can still sample the
other cases, inefficiently. Full support does not ensure practical discovery
within a finite budget.

All twelve high-work pairs across the two fixtures pass. Original MAIN is
`4.305e-31`–`5.160e-31`; updated MAIN is `4.595e-31`–`6.284e-31`. Equal allocation
with the same larger pilots independently corroborates the `1e-31` scale,
although one original pair fails and updated weights are less stable. This is
much more informative than retaining the old `1e-33` merely because it is
nonzero. It is still not a certified high-precision reference.

**Overconcentration:** a small training prefix, top-four full status patterns,
and a cached-root construction gate concentrate learning on one rival set.
The old zero-free fixture concealed the same problem already present before
the two new results.

## Palmeiras / 16th: final variance and allocation are the main issue

The complete proposal already exists. Updated/808's 25,000-draw MAIN has 1,159
hits, but ESS 2.286; one observation contributes 64.0% of its estimate and the
batch gap is 1.715. It correctly fails publication. More hits alone would be a
poor metric for this case.

Freeze the 500-draw pilot, increase only finals to 250,000, and all three
updated pairs pass. MAIN is `2.219e-40`–`3.477e-40`, CHECK `1.692e-40`–`2.994e-40`.
Unlike Flamengo, final-only work already helps here.

With 5,000-draw pilots and 2.5 million finals, all six pairs per fixture pass:
original MAIN `2.562e-40`–`3.653e-40`, updated `2.708e-40`–`3.605e-40`. Updated
MAIN ESS is 33–215 and CHECK ESS 16–188. The existing R65 allocation-prior
experiment's `1.429e-40` has the same order of magnitude, albeit below these
larger-work means. Equal allocation is not uniformly safer: updated/1993 at
250,000 finals produces a concentrated CHECK of `3.816e-39` and fails.

**Overconcentration:** noisy pilot variability influences the allocation and
large corrected weights dominate short finals. This cell supports targeted
additional final work or better allocation, rather than replacing its whole
proposal.

## Cost and accounting

There are 46 three-seed panels, 138 MAIN/CHECK pairs, plus a separate legacy
parity check. Panel process totals are **422.30 seconds wall and 465.40 CPU
seconds**. These are offline diagnostic costs, not an endpoint latency claim.
The fresh high-work panels have the following costs; each includes one shared
construction plus three repeated pilot/training/final pairs:

| Fixture / cell | Panel wall | Panel CPU | Average busy cores, CPU / wall |
|---|---:|---:|---:|
| Original / Flamengo 13 | 11.190 s | 11.723 s | 1.05 |
| Updated / Flamengo 13 | 10.742 s | 11.034 s | 1.03 |
| Original / Palmeiras 15 | 18.305 s | 24.216 s | 1.32 |
| Updated / Palmeiras 15 | 10.707 s | 18.894 s | 1.76 |
| Original / Palmeiras 16 | 25.897 s | 41.033 s | 1.58 |
| Updated / Palmeiras 16 | 48.668 s | 51.722 s | 1.06 |

Four workers are used; allocation imbalance and local scheduling prevent
continuous utilization of all four. A dominant stratum runs on one worker.
Costs vary across batches: the earlier updated Palmeiras/16th high panel takes
85.669 s wall / 65.089 s CPU. These single panels are not a production-speed
benchmark and must not be extrapolated to the production Xeon.

Illustrative updated/4093 modeled work per high-work pair:

| Cell | Setup | Pilot sampling | Messages | MAIN + CHECK | Accounted total |
|---|---:|---:|---:|---:|---:|
| Flamengo 13 | 7,993,048 | 185,939,474 | 4,644,948 | 3,731,693,964 | 3,930,271,434 |
| Palmeiras 15 | 9,294,922 | 199,051,219 | 13,749,832 | 6,129,012,094 | 6,351,108,067 |
| Palmeiras 16 | Not instrumented | 164,213,349 | 0 | 14,796,247,140 | Not available |

The secondary-branch constructor exposes no setup work counter; its measured
setup wall time is included in process timing. Do not label its partial counts
as full cost. Optional prior-bound computation also exposes timing but no work
counter; it was not used in these panels. Diagnostic serialization of first-hit
outcomes is included in wall/CPU, not the sampler's operation counters.

For comparison only, the preceding production full-request R65 updated baseline
averages 2.036 s wall / 4.447 s CPU in one paired batch. Those measurements use
the current Rust implementation, not Go. **No extended configuration is proposed
for automatic integration at its measured cost.** A focused extension is not a
50% full-request allowance or a claim of equal latency. The production endpoint
was not rerun with these per-cell offline settings.

## Recommendations and next experiment

1. Allocate **discovery/retuning before exploitation** for high-priority branches
   with zero pilot hits. Use certified constraints and existing bound/rank hints
   to identify candidates; a large upper bound is not a probability estimate.
   Reserve this work from the existing bank and measure the complete endpoint.
   Flamengo shows that multiplying final draws is often the wrong expense.
2. Preserve structural diversity in family learning: cover alternative
   below/above rival sets before spending all four learned slots on detailed
   tie variations of the same rival set. Start with partial status families
   and compare their weights/support against the current full status patterns.
   Palmeiras/15th supplies concrete missing alternatives for that experiment.
3. Continue to validate nonzero rare cells. An accepted pair can miss material
   components; zero count alone would have rewarded the old Palmeiras/15th
   underestimate and the accepted `1e-39` Flamengo failure. Compare independent
   proposal families as well as independent random streams.
4. Keep cell-specific work decisions generic: discovered hits with concentrated
   weights can justify more final draws; no-hit promising branches need proposal
   work. Do not apply a blanket equal-allocation or hundredfold policy.

These are experimentally motivated next steps, not production changes in this
task. There are no additional reachable/impossible classifications or resolved
undecideds: three already-reachable cells get offline estimates. Other matrix
cells are not resimulated by this focused example, so it establishes no global
coverage-regression guarantee. R65's full-request controls remain the evidence
for its separate narrow allocation recommendation.

## Reproduction

From the repository root, build and run the targeted example tests:

```bash
cargo build --offline --locked --release -j1 --manifest-path odds-rust/Cargo.toml --example branch_stratification
cargo test --offline --locked --release -j1 --manifest-path odds-rust/Cargo.toml --example branch_stratification -- --test-threads=1
```

Fresh high-work Flamengo/13th validation on the updated request:

```bash
python3 experiments/rare_positions/2026-10-04-extended-allocation/scripts/run_matrix.py \
  --binary odds-rust/target/release/examples/branch_stratification \
  --request experiments/rare_positions/reference/2026-10-04-two-results/inputs/group-16498-two-results.json \
  --output /tmp/r67-updated-flam13 \
  --team 17 --rank 13 --draws 300000 --seeds 4093,4099,4111 \
  --allocation trained --guide bounds --pilot-draws 2500 --floor 10 \
  --pilot-seed 808 --tree-leaves 64 --messages 1 --production-profile
```

For Palmeiras/15th change the output directory and use:

```text
--team 16 --rank 15 --draws 600000 --seeds 4093,4099,4111
--allocation trained --guide bounds --pilot-draws 500 --floor 2
--pilot-seed 808 --tree-leaves 256 --messages 3 --production-profile --rank-priority
```

For Palmeiras/16th change the output directory and use these arguments, omitting
tree, message, production-profile and rank-priority flags:

```text
--team 16 --rank 16 --draws 2500000 --seeds 4093,4099,4111
--allocation trained --guide adaptive --pilot-draws 5000 --floor 200
--pilot-seed 808 --refine
```

Repeat with the original request path for paired inputs. Use seeds
`808,1669,1993` for the first high panels. To isolate final work, retain each
cell's initial pilot count from the design table; to compare allocation, replace
`trained` with `equal` at the same pilot and final draw settings. For the
Flamengo pilot-seed control use 3,000 finals / 25 pilots and change only
`--pilot-seed` to 1993. The output directory must be new or empty.

## Validation, correctness and evidence

- Four targeted example tests pass: stable tiny prior scores, invalid-input and
  zero-hit fallback handling, allocation floors/totals, and pilot-seed parsing.
- Release example build, formatting, runner syntax/help and whitespace checks
  pass. Independent Sol review checked stream separation, full support,
  weighting scope, outcome diagnostics and accounting limitations.
- Frozen pre-diagnostic versus new example parity on original Flamengo/13th,
  seed 808, without pilot override: zero numeric/allocation mismatches after
  excluding timings and new diagnostic fields. MAIN `1.172984726e-33`, CHECK
  `4.499105530e-34`, identical modeled total 50,473,186 units.
- Outcome diagnostics retain fixture IDs and win/draw/loss assignments plus
  phase-derived points/wins. **Actual sampled goal scores are not retained**.
  These records illustrate verified sampler hits, but cannot independently
  replay a goal-tiebreak witness from the log. They are not new solver proofs.
- Training hits, bounds, native estimates rejected by CHECK, and reachability
  witnesses are never published as probability estimates by this experiment.
  No acceptance gate was relaxed, no failed score assignment became an
  impossibility proof, and no arbitrary score cap was introduced.
- The preceding core build's full Rust suite passed 210 tests with four DB
  tests ignored; this investigation changes no core/runtime source and adds
  targeted offline-example tests.

Local raw evidence is in `local/2026-10-04-extended-allocation/`: each named panel
contains `record.json`, `summary.json`, `stdout.jsonl` and `stderr.txt`. Key
panels are `{original,updated}-{flam13,pal15,pal16}-fresh-validation`,
`updated-flam13-{1x-frozen,1x-pilotseed1993,10x-pilots-finals,100x-pilots-finals}`,
`{original,updated}-pal15-100x-pilots-finals`, and
`updated-pal16-{1x-frozen,10x-finals,100x-finals-10x-pilots}`.
Legacy parity evidence is `parity-comparison.json` and `parity-{frozen,current}`.
Raw files are local diagnostic artifacts; commands and results above make the
report self-contained and reproducible without committing large logs.

Primary repository sources:

- [Fixture and production failure comparison](2026-10-04-rust-two-results.md).
- [R65 full-request seed controls and timing](2026-10-04-rust-confirmation-pilots.md).
- [Tree selected-side constraints](../../odds-rust/src/joint_caps/propagated/lazy/strata/tree.rs), `next_team_with_work`.
- [Learned full family constraints](../../odds-rust/src/joint_caps/propagated/lazy.rs), `build_family_pattern`, and defensive mixture weighting in `sample_internal`.
- [Offline example](../../odds-rust/examples/branch_stratification.rs) and
  [reproducible runner](2026-10-04-extended-allocation/scripts/run_matrix.py).
