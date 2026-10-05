# Group 16498: fresh 100× runs and backward family analysis

October 5, 2026. Evaluation complete. Production defaults are unchanged.

## Scope and design

This reproduces the R67 100× per-cell diagnostic on Flamengo/13th and
Palmeiras/15th–16th before and after the two games. It does not multiply every
cell's full endpoint budget. Probabilities are fractions.

The original snapshot has 103 remaining games; the refreshed after snapshot
has 101. The latter includes game 364007 at 1–2, game 363999 at 1–0 and 200
updated power fields. Both target teams retain the same initial points/wins
and ten remaining fixtures. Results and refreshed powers change together;
this pair cannot isolate their separate causal effects. Exact input copies
are retained as `2026-10-05-100x-backward/data/before.json` and `after.json`.

| Cell | Final draws in each MAIN/CHECK | Pilot draws per branch | Proposal |
| --- | ---: | ---: | --- |
| Flamengo13 | 300,000 | 2,500 | Complete tree, 64-leaf cap, one message |
| Palmeiras15 | 600,000 | 500 | Complete tree, 256-leaf cap, three messages, generic rank priority |
| Palmeiras16 | 2,500,000 | 5,000 | Complete secondary-refined branches |

Final seeds are 60013, 60017 and 60029. Construction stays at 808; each of the
six fixture/cell combinations runs with pilot seed 808 and fresh pilot seed
60031. The same final seeds across the pilot controls isolate learning and
allocation changes; they do not provide six independent final streams per
cell. Some controls can produce identical estimates when they choose the same
proposal and allocation. MAIN and CHECK streams remain distinct.

All heavy jobs run serially with four workers. The R67 runner clears inherited
experiment controls and stores commands, environment, hashes, raw diagnostics,
wall time and child CPU time. Branch omission is off; every stratum retains
positive sampling work. The offline example bypasses production admission
constraints, particularly for Palmeiras15. Its costs do not describe a 100×
production endpoint or justify activating these settings automatically.

## Results

All twelve primary panels completed: **33/36 MAIN/CHECK pairs pass** the
unchanged gates. Every before pair passes. After Palmeiras15 with pilot 60031
fails seed 60029; after Palmeiras16 fails seed 60017 under both pilots.
The latter two failures share final streams and are correlated. These fresh
results supersede any inference that the earlier R67 36/36 pass count guarantees
reliable confirmation at 100×.

Means across the three fresh final seeds, **including failed pairs**, with
construction/pilot seed 808:

| Cell | Before MAIN / CHECK | After MAIN / CHECK | Passing pairs before / after |
| --- | --- | --- | --- |
| Flamengo13 | 7.709e-34 / 7.636e-34 | 8.577e-34 / 8.753e-34 | 3/3 / 3/3 |
| Palmeiras15 | 4.788e-31 / 4.608e-31 | 5.386e-31 / 5.295e-31 | 3/3 / 3/3 |
| Palmeiras16 | 2.830e-40 / 3.155e-40 | 3.901e-40 / 3.150e-40 | 3/3 / 2/3 |

Pilot seed 60031, reusing the same final seeds:

| Cell | Before MAIN / CHECK | After MAIN / CHECK | Passing pairs before / after |
| --- | --- | --- | --- |
| Flamengo13 | 7.709e-34 / 7.636e-34 | 8.577e-34 / 8.753e-34 | 3/3 / 3/3 |
| Palmeiras15 | 4.538e-31 / 5.176e-31 | 6.113e-31 / 5.981e-31 | 3/3 / 2/3 |
| Palmeiras16 | 2.891e-40 / 3.215e-40 | 4.084e-40 / 3.277e-40 | 3/3 / 2/3 |

These are descriptive Monte Carlo means, not exact probabilities or confidence
intervals. Failed concentrated MAIN rows raise the after Palmeiras16 mean;
CHECK and most individual rows remain near 3e-40. The evidence supports the
previous larger probability scales, while their precision remains provisional.
After Flamengo is identical across pilots; those controls add no independent
final evidence.

### What the unchanged production algorithm publishes

Six additional full-request controls use the same final seeds, four workers,
and the frozen R68 v5 binary with no experiment flags. The recorded seed is
shared, but the production and focused example streams/proposals differ; this
is not a common-random-number comparison. Values below are `Pos[rank-1]/100`;
rare diagnostics use zero-based rank keys and fractions.

| Fixture / seed | Flamengo13 | Palmeiras15 | Palmeiras16 |
| --- | ---: | ---: | ---: |
| Before / 60013 | 8.836e-34 | 0 | 8.505e-41 |
| Before / 60017 | 6.902e-34 | 0 | 0 |
| Before / 60029 | 1.127e-33 | 1.114e-31 | 0 |
| After / 60013 | 0 | 0 | 1.812e-40 |
| After / 60017 | 0 | 0 | 1.798e-40 |
| After / 60029 | 6.840e-34 | 0 | 1.761e-40 |

Zeros here are published estimator outcomes, not proof of impossibility. Both
fixtures show quality failures; useful nonzero outputs can also substantially
underestimate a cell. For before seed 60013, Palmeiras16 publishes 8.505e-41 and marks its precision
goal met, although the high-work evidence is near 3e-40. That flag describes
observed estimator diagnostics, not accuracy against the broader proposal
evidence. Logs and exports are under `production-fresh/`.

## Backward interpretation

Branch probabilities sum directly to the aggregate estimate. For each stream,
measured contribution is `branch probability / aggregate probability`; it is
not multiplied by allocation or proposal-normalizer share. Branch indices
are joined to their setup metadata, then compared using target fixture/outcome
paths and selected/strict rival constraints. First-hit witnesses retain
phase-derived points/wins and fixture W/D/L outcomes, but not actual goals;
they illustrate successful samples and cannot independently replay goal ties.

### Flamengo13: find the dominant target path before exploitation

Both pilot-808 panels recover probability near 8e-34. Their dominant target
path is ten losses, ending at 60 points and 18 wins. Original and updated
branch indices differ despite the same semantic path. The first fresh-seed
pair assigns about 90% of final draws to that branch and measures nearly all
probability there. The old sparse pilot could instead concentrate on one draw
and nine losses, producing convincing MAIN/CHECK agreement near 1e-39 while
missing the dominant component. Merely extending those frozen finals is an
inefficient remedy.

### Palmeiras15: preserve different rival sets

The first high-work pairs are near 5e-31. They again include the all-loss path
ending at 57 points and 16 wins, with Chapecoense, Grêmio, Internacional and
Remo below in illustrated hits. Corinthians, Vasco or Vitória supplies the
additional below rival in different components. These are different structural
families, not detailed tie variations of one family. The old four learned
families all required Corinthians below. Tree components requiring Corinthians
on the nonstrict opposite side cannot overlap those learned components.
For seed 60013/pilot808 before, Vasco and Vitória components supply 81.1% of
MAIN and 71.2% of CHECK; after, they supply 64.1% and 66.7%. Their changing
shares show why a single rival-set winner is insufficient on either fixture.

A fresh pilot reveals a second selection problem. Before it refines branches
51, 53 and 54 (Corinthians, Vasco and Vitória alternatives); after it selects
52, 53 and 54, so the smaller team-584 branch displaces Corinthians. Branch 51
receives only 40,351 of 600,000 draws. In after seed 60029 it supplies 60.80%
of MAIN but 21.57% of CHECK; MAIN ESS is 5.67 and one observation supplies
40.88%. In seed 60017 it supplies 12.80% of MAIN and 61.01% of CHECK. That pair
passes despite CHECK ESS 9.95 and a 29.00% maximum share. Passing the current
gates does not establish tight precision.

### Controlled fourth-slot check

After observing that displacement, run exactly one extra policy on **both**
fixtures: four message slots instead of three, holding frozen binary,
construction seed, pilot seed 60031, 500 pilot draws, allocation floor, guides,
600,000 final draws and all three final seeds fixed. This is an exploratory
counterfactual, recorded separately from the twelve primary panels. It changes
proposal refinement, replacement pilots, allocation and final draws together;
it cannot isolate the benefit of allocation alone.

Both fixtures select branches 51, 52, 53 and 54. All six extra pairs pass.

| Fixture / slots | Mean MAIN / CHECK | Minimum aggregate MAIN ESS | Largest aggregate MAIN weight share |
| --- | --- | ---: | ---: |
| Before / 3 | 4.538e-31 / 5.176e-31 | 133.95 | 7.89% |
| Before / 4 | 4.550e-31 / 5.181e-31 | 134.92 | 7.83% |
| After / 3 | 6.113e-31 / 5.981e-31 | 5.67 | 40.88% |
| After / 4 | 5.331e-31 / 5.390e-31 | 121.06 | 5.44% |

After, Corinthians contributions become 25.6–33.4% of MAIN and 28.8–36.4%
of CHECK. Before, the extra slot changes little and branch-level heavy tails
persist: seed 60013 Corinthians CHECK ESS is still 5.24. This supports preserving
distinct rival skeletons before detailed exploitation; it does not justify a
blanket increase in slots or prove stability for other seeds.

Added modeled message charge per seed is 4,788,337 units before and 4,468,211
after; added pilot charge is 3,499,246 and 3,318,966. For seed 60013, added
MAIN/CHECK charges are 4,189,013 / 4,224,388 before and 27,204,806 / 27,344,049
after. Total paired modeled charge rises about 0.21% before and 0.92% after.
These final charges are observed diagnostics, not guaranteed reserves. The
legacy message path does not enforce the supplied 20-million-unit limit;
this check cannot establish feasibility inside the production bank. Timing
is a single serialized observation, not a latency benchmark.

### Palmeiras16: large weights survive 100× sampling

The original first-pilot panel passes all three pairs near 3e-40. The updated
panel passes two; final seed 60017 fails despite 2.5 million MAIN draws and
113,032 hits. MAIN is 6.0556e-40, ESS 5.5486 and maximum observation share
41.13%; CHECK is 2.9996e-40, ESS 133.666 and maximum share 4.79%. Two branches
of the same all-loss target path account for about 99.1% of MAIN. The largest
has branch ESS 2.973 and a 57.84% maximum share. This is a concentrated-weight
problem; a nonzero answer or many hits does not establish reliable precision.

## A generic algorithm design

Use a hierarchy shared by every team/rank cell:

1. **Target paths:** enumerate feasible remaining outcomes under certified
   points/wins limits, retaining complete parent regions when splitting cannot
   finish. Compare semantic paths, not branch IDs or team-specific exceptions.
2. **Rival skeletons:** represent alternative strict-above/strict-below sets
   before spending learned slots on detailed ties. Reserve discovery work for
   promising unseen skeletons and retain defensive native support.
3. **Affordable refinement:** admit setup and confirmations using the actual
   proposal structure and existing bank. Charge failed attempts. Use discovery training,
   freeze the proposal, then use separate allocation pilots, MAIN and CHECK
   streams.
4. **Allocation by failure evidence:** a promising zero-hit family needs
   discovery/refinement; many hits with low ESS and large maximum weights need
   variance reduction or additional affordable final work. Apply these rules
   through measured diagnostics for every cell.
5. **Coverage validation:** compare separately trained proposals and structural
   families, including already-positive cells. Independent final streams drawn
   from the same incomplete learned proposal can share a blind spot.

R68 did reserve discovery, but its current guide admission bound blocked attempted
retunes and prevented evaluating their usefulness, and its blanket partial-family confirmation reserve did not fit the
affected grants. Correcting that cost model is a prerequisite to testing the
hierarchy at production capacity.

For the current guide representation, its logical operation charge can be
calculated before constructing it:

```text
guide_charge = 2 * (remaining_games + 1) * teams
                 * (forward_cap + reverse_cap + 2)
```

Each cap is the maximum across teams of summed incident-game gain maxima.
Forward maxima consider outcomes with positive probability; reverse maxima
consider all outcomes. The representation gives every logical row the global
cap, so a sum of individual team spans would undercount. Use checked arithmetic
and charge the scan itself. This follows `Guide::new` and the stored suffix
row dimensions and matches current logical charging; it is not a CPU guarantee.

For overlapping learned families, derive a safe density evaluation bound from
each actual pattern's fixed, omitted, joint and guided fixtures, joint-team
count and cardinality/dynamic work. Sum native plus fitted-family evaluations
per root, then bound the possible roots and add status/root/ranking work.
Reuse the selected component's already-computed ratio. Reserve the final pair
at fixed sizes before sampling; empirical pilot averages alone cannot certify
the worst cost. Every matching family must remain in the mixture denominator.

This is an implementation design, not a demonstrated production improvement.
Its first acceptance condition is successful refinement inside the existing
allowance, followed by paired whole-endpoint evaluation on both snapshots and
independent fixtures. The fresh 100× panels diagnose families and variance;
they cannot certify global matrix coverage or a production latency budget.

## Evidence and reproduction

Raw records, analyzer, protocol and frozen artifacts are under
`2026-10-05-100x-backward/`. The only Rust edit is the offline example's call
to the new message API, passing its historical 20-million-unit allowance.
The example's four targeted tests pass. Production source and settings are
unchanged; no database write, commit or push.

The twelve primary panels cost **229.51 seconds wall / 299.84 CPU seconds**.
The two fourth-slot panels add **26.04 seconds wall / 41.44 CPU seconds**;
six full-request controls add **10.20 / 22.16 seconds**. Combined measured
experiment cost is **265.76 seconds wall / 363.44 CPU seconds**, excluding
builds, tests, analysis, inspection and idle time. These are sums of recorded
child runs, not elapsed time for the entire task. All 20 child processes exit
successfully. Four Rust example tests and the analyzer's focused tests pass.

Frozen example SHA-256:
`1c191c1fe9a7d6a07a652748545443f6cb8a4c61e49781fc9a15292ed27ec4f1`.
Source archive SHA-256:
`ee636b4c22485d5e71938bb2a94c7a437ff150aa15c5898eb349d882a95f5c79`.
The archive and diff preserve the broader existing experimental working tree;
this campaign changes only the example's API call and adds analysis artifacts.
Production control binary SHA-256:
`6317b78f47e355f2b6310cd05ffcf9eb62d3c3ef37c32db2988c6af6ad3b89be`.
`data/v1.sha256` records the frozen example, source archive and runner. Input
hashes are recorded separately in every run record. Each run's
`record.json` contains its exact command, environment, input/output/binary
hashes, exit status and wall/CPU costs. `analysis-100x.json` validates the
complete primary campaign and optionally summarizes the fourth-slot check.

From the repository root, reproduce panels into a new output root (the runner
refuses existing destinations). The full frozen commands also live in each
record. `before.json` and `after.json` are retained exact snapshots.

```bash
RUNNER=experiments/rare_positions/2026-10-04-extended-allocation/scripts/run_matrix.py
BINARY=experiments/rare_positions/2026-10-05-100x-backward/data/branch-stratification-v1
INPUTS=experiments/rare_positions/2026-10-05-100x-backward/data
OUT=/tmp/group-16498-100x-reproduction

for pilot in 808 60031; do
  suffix=''
  if [ "$pilot" = 60031 ]; then suffix=-pilot60031; fi
  for phase in before after; do
    python3 "$RUNNER" --binary "$BINARY" --request "$INPUTS/$phase.json" \
      --output "$OUT/$phase-flam13-100x$suffix" --team 17 --rank 13 \
      --draws 300000 --seeds 60013,60017,60029 --allocation trained \
      --guide bounds --pilot-draws 2500 --floor 10 --pilot-seed "$pilot" \
      --tree-leaves 64 --messages 1 --production-profile
    python3 "$RUNNER" --binary "$BINARY" --request "$INPUTS/$phase.json" \
      --output "$OUT/$phase-pal15-100x$suffix" --team 16 --rank 15 \
      --draws 600000 --seeds 60013,60017,60029 --allocation trained \
      --guide bounds --pilot-draws 500 --floor 2 --pilot-seed "$pilot" \
      --tree-leaves 256 --messages 3 --production-profile --rank-priority
    python3 "$RUNNER" --binary "$BINARY" --request "$INPUTS/$phase.json" \
      --output "$OUT/$phase-pal16-100x$suffix" --team 16 --rank 16 \
      --draws 2500000 --seeds 60013,60017,60029 --allocation trained \
      --guide adaptive --pilot-draws 5000 --floor 200 --pilot-seed "$pilot" --refine
  done
done
for phase in before after; do
  python3 "$RUNNER" --binary "$BINARY" --request "$INPUTS/$phase.json" \
    --output "$OUT/$phase-pal15-100x-pilot60031-messages4" --team 16 --rank 15 \
    --draws 600000 --seeds 60013,60017,60029 --allocation trained \
    --guide bounds --pilot-draws 500 --floor 2 --pilot-seed 60031 \
    --tree-leaves 256 --messages 4 --production-profile --rank-priority
done
python3 experiments/rare_positions/2026-10-05-100x-backward/scripts/analyze_100x.py \
  --run-dir "$OUT"
```

Fresh full-request controls:

```bash
python3 experiments/rare_positions/2026-10-05-structural-discovery/scripts/run_matrix.py \
  --candidate-binary experiments/rare_positions/2026-10-05-structural-discovery/data/golaberto-odds-r68-v5 \
  --out-dir /tmp/group-16498-production-fresh \
  --fixtures original-16498,updated-16498 --seeds 60013,60017,60029 \
  --arms control --repeats 1 --warmups 0
```

Analyzer validation:

```bash
python3 -m unittest discover \
  -s experiments/rare_positions/2026-10-05-100x-backward/scripts \
  -p 'test_analyze_100x.py'
```

No default activation, database write, commit or push.


## Subsequent fresh-training follow-up

The generic controls and additional holdouts are complete in
`2026-10-05-rust-generic-followups.md`. The earlier fourth-slot gain does not
uniformly generalize: a fresh training/final realization develops large
weights inside the refined Vasco family. Always-bounds guidance also regresses
on an original-fixture holdout. Preserve these later failures when interpreting
this report's exploratory success; no tested setting change earns adoption.
