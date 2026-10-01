# Proof-guided fixture-domain sampling experiment

Date: 2026-09-30. Current native Rust estimator, four workers. Experimental
changes remain disabled by default. No commit, push, or database write was made.

## Conclusion

Bounded conditional proofs can safely remove some fixture outcomes and compact
new forced fixtures, but these budgets did **not produce additional estimates**.
Keep production defaults unchanged.

Across five snapshots and six fixed seeds, both proof-only variants returned
responses **exactly identical** to the current Rust baseline: all probabilities,
uncertainties, team odds, game importance, and reachability labels matched.

The reallocated variant gained team **9 / position 14**, in the earlier 16653
snapshot at seeds 801 and 808. This is **one distinct cell, two cell-runs**.
An ablation using the same reduced draw counts with **no probes** produced
exactly the same responses in all 30 comparisons. Therefore these gains cannot
be attributed to the proof mechanism.

## Baseline and protocol

The baseline was saved from the current release executable before these edits,
with discrete cuts already enabled by default. It is not the historical Go
implementation or the Rust implementation before the discrete proof.

- Baseline SHA-256: `94283bee7d957029779cc148563671bdc29aa6b9251a661adb6c0e253cf559fe`.
- Candidate SHA-256: `e5e8ceaa858863f6b200928ad86f6a478bab1b789f2d6bd811dcc948aa96de4b`.
- Five frozen snapshots: 16498 `44eabb47`, earlier 16653 `2d1c1d6f`, current
  16653 `71d4fea8`, 16982 `9327edcd`, and 16983 `43969b02`.
- Fixed seeds: 801, 804, 808, 817, 818, 911.
- 30 paired complete HTTP requests per variant, 120 pairs including the ablation.
- Alternating baseline/candidate order. One active request at a time; four
  calculation workers. No tests or builds ran during the measurements.
- Each full request retains the existing 20,000-season importance scout,
  100,000-season matched pool, other rescues, proofs, and reconciliation.
- An additional persistent-server joint-probe comparison used two warmups and
  six timed requests per snapshot, seed 808.

Machine-local timing noise and compilation effects remain relevant: controls
16982 and 16983 execute no domain rescue, yet also show timing differences.
Do not interpret a small overall speedup as a saving caused by the probes.

## Implementation

Only the existing propagated-domain rescue receives the new optional probes.
Other samplers and reachability searches retain their existing budgets.
The sampler fixes the target's remaining fixture outcomes before constructing
rival domains, so its final points/wins are known for each conditional state.

1. Keep the existing min/max rank-cardinality propagation.
2. At the first uncached conditional states, order residual fixtures by number
   of allowed outcomes, then endpoint distance from the target's total.
3. Tentatively fix one allowed outcome and propagate its domains.
4. Optionally test a necessary joint cap and floor relaxation. Apply every
   singleton outcome to the base totals and remove that fixture from the
   residual proof model. Residual non-singleton masks remain restricted.
5. For wins-packed totals, also test a raw-points floor model with the discrete
   coalition cut. Recover raw points using Euclidean division, including
   negative point adjustments.
6. Remove an outcome only if a relaxation proves that conditional branch
   infeasible; propagate again and compile forced fixtures through the existing
   compact simulation loop.
7. Cache the restrictions with the existing target-fixture-pattern key and
   64-entry limit. Unsuccessful or budget-exhausted probes preserve the outcome.

Limits are **per sampler invocation**, shared across its cached states, not
per simulated season or per cache entry. A joint trial allows up to 16 exemption
search nodes **per direction/model**: packed cap, packed floor, and raw floor
when applicable, at most 48 nodes per hypothesis. Graph propagation itself is
not counted as an exemption node. Model construction is inside measured probe
time. Cache/domain setup outside the probe is captured by stage wall time.

The discrete cut still ignores residual non-singleton masks in its coalition
graph, making that part a weaker, sound relaxation. Ordinary propagation uses
those masks. It is not a complete conditional constraint solver.

| Variant | Hypotheses per call | Proof nodes per direction | Pilot / final / check draws |
|---|---:|---:|---:|
| Current baseline | 0 | 0 | 1,000 / 15,000 / 5,000 |
| Local propagation | 12 | 0 | 1,000 / 15,000 / 5,000 |
| Joint proofs | 6 | 16 | 1,000 / 15,000 / 5,000 |
| Joint + reallocation | 6 | 16 | 750 / 14,000 / 5,000 |
| Draw-only ablation | 0 | 0 | 750 / 14,000 / 5,000 |

Reallocation does not guarantee less total work: changing pilot lengths can
change which cells qualify for final sampling, and shortening a confirmation
run changes its acceptance statistics. In the earlier 16653 seed-808 case,
domain-stage wall time increased from 5.87 to 25.64 ms because the changed
allocation caused more downstream work. No additional time allowance is assumed.

## Coverage results

| Variant | Added nonzero cell-runs | Lost nonzero cell-runs | New reachable / impossible labels | Recommendation |
|---|---:|---:|---:|---|
| Local propagation | 0 | 0 | 0 / 0 | Do not enable |
| Joint proofs | 0 | 0 | 0 / 0 | Do not enable |
| Joint + reallocation | 2 | 0 | 0 / 0 | Do not credit probes; do not enable from this experiment |
| Draw-only ablation | 2 | 0 | 0 / 0 | Interesting allocation effect; needs independent seeds before adoption |

All newly pruned outcomes occur in the earlier 16653 snapshot. Counting
repeated conditional states/calls/seeds, rather than distinct fixture outcomes:

| Variant | Hypotheses | Exemption nodes | Removed outcomes | Newly forced fixtures | Entire conditional states rejected |
|---|---:|---:|---:|---:|---:|
| Local propagation | 1,440 | 0 | 35 | 19 | 4 |
| Joint proofs | 720 | 3,026 | 22 | 10 | 5 |
| Joint + reallocation | 744 | 3,098 | 22 | 10 | 5 |

Local and joint variants have different hypothesis budgets; these counts alone
cannot establish which propagation method is stronger at equal work.
The rejected states are conditional target-outcome patterns, not newly proved
impossible team/rank cells. They must not be added to the global impossibility
count.

Baseline seed-808 matrix status, also unchanged by the proof-only variants:

| Snapshot | Nonzero | Impossible zeros | Reachable zeros | Undecided zeros |
|---|---:|---:|---:|---:|
| 16498 | 362 | 16 | 21 | 1 |
| Earlier 16653 | 344 | 43 | 13 | 0 |
| Current 16653 | 338 | 48 | 14 | 0 |
| 16982 | 400 | 0 | 0 | 0 |
| 16983 | 400 | 0 | 0 | 0 |

At seed 818, earlier 16653 has 12 reachable zeros and one undecided zero.
The proof probes leave that undecided unchanged. Group 16498 retains one
undecided across all six tested seeds.

The draw-allocation gains have probability, expressed as fractions:

- Team 9 / rank 14, earlier 16653, seed 801: `1.3546e-14`, relative SE 16.9%.
- Same cell, seed 808: `1.1296e-14`, relative SE 21.6%.

These are sampler estimates passing the existing acceptance checks, not exact
probabilities. They are far below the 1e-6 event scale. Matrix reconciliation
can change other nonzero entries after accepting a new estimate.

## Timing and CPU

Cold-server request wall-time summary across all 30 pairs per variant:

| Variant | Median paired change | Change in sum of request times | Change in summed process CPU |
| Local propagation | -1.69% | -1.46% | -0.75% |
| Joint proofs | -1.85% | -1.68% | -1.53% |
| Joint + reallocation | -0.03% | +0.64% | -0.33% |
| Draw-only ablation | -1.60% | -1.99% | -2.43% |

The reallocated variant's summed full-request latency **increased 0.64%**.
Its group-level median increases were 2.54% for 16498 and 2.44% for current
16653; some individual pairs increased more. Cold comparisons contain noise;
this is not evidence that the reallocated variant preserves latency everywhere.

Persistent-server joint-probe measurements at seed 808:

| Snapshot | Baseline full HTTP median ms | Joint median ms | Change | Baseline domain stage ms | Joint domain stage ms |
| 16498 | 759.73 | 708.27 | -6.77% | 4.55 | 4.45 |
| Earlier 16653 | 692.72 | 690.36 | -0.34% | 4.77 | 5.42 |
| Current 16653 | 736.78 | 739.12 | +0.32% | 54.82 | 57.22 |
| 16982 | 318.29 | 326.28 | +2.51% | 0.00 | 0.00 |
| 16983 | 316.28 | 311.33 | -1.57% | 0.00 | 0.00 |

Zero stage entries mean no domain rescue ran. The current 16653 domain stage
increased **2.40 ms**, even though full-request median increased only 0.32%.
The joint variant's summed persistent HTTP medians decreased 1.72%, and total
process CPU decreased 2.27%; these results include substantial unchanged work.
They do not justify an optimization claim when coverage is identical and the
no-probe control groups also vary. CPU measurements include startup and all
eight requests per server; timed wall medians exclude the two warmups.

Probe/model work summed over parallel sampler invocations has median per
request of approximately 1.14 ms for 16498, 1.77 ms for earlier 16653, and
2.32 ms for current 16653 in the joint variant. This is summed probe wall time,
**not** separately measured CPU time or critical-path request overhead.
The result JSON includes process CPU, peak RSS, per-stage wall times, and each
probe event. Additional estimated cells attributable to probes per unit of time
are zero in this experiment.

## Correctness and limitations

- Probes operate on a necessary relaxation of rank; equality in points/wins
  leaves later goal and other tiebreakers free. Relaxation feasibility is not
  accepted as a witness or estimate.
- The raw discrete model is supported only for ordinary 3/1/0 scoring with no
  bonus points. The sampler disables probes for other scoring/bonus rules.
- All target fixtures are already selected/fixed before probing. Wins remain
  packed within the stride; final wins are below the stride by its construction.
- No score cap, team-specific rule, or rank-specific search was introduced.
- Remaining target-pattern probability, forced-fixture masses, proposal
  normalization, and original/proposal likelihood ratios use the existing
  importance-sampling implementation. Infeasible conditional patterns retain a
  zero contribution in the total sample denominator.
- A returned witness still comes from a complete season checked by the
  production standings sorter. Removed branches do not publish global proofs.
- A finite probing budget examines only early cached states and a few fixture
  outcomes. These results reject these bounded variants for production; they
  do not show that stronger or differently allocated conditional propagation
  can never help.
- The newly forced states did not change any published proof-only response.
  This suggests the removed work was not a limiting factor for accepted rare
  estimates under these budgets; it does not prove that every later proposal
  would encounter the same constraints.

## Tests

`cargo test --offline --locked --manifest-path odds-rust/Cargo.toml`:
**47 passed; two database tests ignored**. Local HTTP tests needed execution
outside the listener-blocking sandbox; the suite then passed. No database writes
were performed.

Two focused tests were added:

- Enumerate all 729 outcome assignments of a four-team league; for every actual
  rank hit, preserve every fixture outcome after probing its fixed target
  pattern. Exercise points only, wins packing, negative point adjustments,
  actual removals, and per-call budget enforcement.
- Compare the proof-guided importance sampler against exact enumerated rank
  probabilities, using the existing statistical tolerance. This checks forced
  outcomes and conditional pruning through the weighted sampling path.

## Reproduce

Save the **current default Rust executable** before applying experimental source
changes; do not use the older Go or pre-discrete Rust baseline:

```sh
cp odds-rust/target/release/golaberto-odds /private/tmp/golaberto-domain-probe-baseline
cargo build --release --offline --locked --manifest-path odds-rust/Cargo.toml

python3 experiments/rare_positions/compare_rust_reachability.py \
  --baseline /private/tmp/golaberto-domain-probe-baseline \
  --candidate odds-rust/target/release/golaberto-odds \
  --reference experiments/rare_positions/reference/2026-09-30-hundredfold/reference.json \
  --output /private/tmp/domain-probe-pairs \
  --baseline-variant defaults \
  --variants probe_local,probe_joint,probe_realloc,probe_draws_only \
  --seeds 801,804,808,817,818,911 --http --no-trace

python3 experiments/rare_positions/benchmark_domain_probes.py \
  --baseline /private/tmp/golaberto-domain-probe-baseline \
  --candidate odds-rust/target/release/golaberto-odds \
  --variant probe_joint --iterations 6 --seed 808 \
  --output /private/tmp/domain-probe-persistent
```

The actual paired run was split into seeds `808,818` and
`801,804,817,911`, followed by the six-seed draw-only ablation. Results are
combined in [the saved JSON](results/2026-09-30-rust-domain-probes.json).

Experimental flags, all defaulting to no probes/no draw reallocation:

```sh
RUST_ODDS_DOMAIN_PROBES=6 RUST_ODDS_DOMAIN_PROBE_NODES=16 \
  odds-rust/target/release/golaberto-odds estimate REQUEST.json OUTPUT.json 808 4
# Add RUST_ODDS_DOMAIN_PROBE_REALLOCATE=1 only to reproduce the reallocated test.
# Local-only variant: PROBES=12 and PROBE_NODES=0.
```

`RUST_ODDS_DOMAIN_PROBE_NODES` defaults to 16 but does nothing unless the
hypothesis budget is positive. Probe telemetry is emitted with ordinary odds
logging enabled. The standalone benchmark uses only `/odds`; it imports the
persistent-server helper without invoking its separate ratings/database harness.

## Recommendation

Keep these probes as an opt-in experiment. They add verified pruning but no
measured probability coverage. Do not enable a larger budget on the assumption
that a one-second production allowance exists.

If continuing this line of investigation, first measure how frequently each
conditional state recurs and whether rejected branches dominate failed
confirmation runs. That would give a reason to move a bounded proof budget to
specific expensive states. Separately validate the draw-only allocation on
held-out seeds: the current gains are tiny and can result from pilot selection
or acceptance changes, not a generally better proposal.

Related proof derivation and prior measurements:
[discrete-cut report](2026-09-30-rust-discrete-cut.md).
Scientific background supplied for this investigation:
[Bernholt et al.](https://image.informatik.htw-aalen.de/~thierauf/TI2/Netz/football.pdf),
[Gotzes and Hoppmann](https://link.springer.com/article/10.1007/s12351-020-00546-w),
[Duque et al.](https://www.aimsciences.org/article/doi/10.3934/jimo.2018109),
[Perron et al.](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.CP.2023.3),
and [Rosenberg et al.](https://arxiv.org/html/2605.13142v2).
No external CP-SAT/SCIP runtime was integrated in this experiment.


## Follow-up

The [larger-budget experiment](2026-09-30-rust-domain-probes-larger.md) tested
24, 96, and 384 hypotheses per call across the same snapshots and six seeds.
It found more pruning but no additional estimates attributable to probes.
