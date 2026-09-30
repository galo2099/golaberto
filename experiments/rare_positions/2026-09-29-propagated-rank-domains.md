# Transfer reachability constraints into probability sampling (2026-09-29)

## Adopted change

The existing searches now finish with a bounded sampler that transfers
necessary points/wins constraints into fixture outcome domains. It searches
only remaining zero cells, so it preserves the existing search allocation.
There are no special team IDs, groups, or finishing positions in production.

On five fixed seeds for each of the five reference inputs, the adopted pass
found **Fortaleza / 18th in all five current group 16653 runs**. It recovered
9 zero-to-nonzero cell-runs in that snapshot and 3 in the earlier snapshot,
with no nonzero-to-zero losses in any of the 25 paired full requests.
Those 12 cell-runs represent five distinct input/team/position cells.

The median paired cost in current 16653 was **110 ms / 10.5%** on four cores.
The earlier snapshot cost 59 ms / 5.2%. Group 16498 gained no estimates from
this pass and cost 19 ms / 1.3%. Groups 16982 and 16983 had no zero targets.

The pass is enabled by default inside the matched-point-pool conditioned
zero pipeline. Existing production flags remain sufficient. Set
`RARE_POSITION_PROPAGATED_DOMAINS=0` to disable it. Startup diagnostics print
the flag and `effective_propagated_domains`; request logs print per-cell
evidence, aggregate work, and elapsed time under `rare-position-rank-domains`.
The Go service needs a rebuild and restart; no JavaScript changed.

## Why Fortaleza / 18th is a useful case

The current request has Fortaleza on 51 points, with nine fixtures left.
América-MG and Ponte Preta can reach at most 44 and 40, respectively. Both
must stay below Fortaleza, occupying both available below-rank slots for an
18th-place finish. Every other rival must finish above it.

Londrina can reach at most 52 points. Therefore Fortaleza must finish on 51
or 52: nine losses, or eight losses and one draw. Londrina must win all eight
remaining games. Its wins force losses for Avaí and Botafogo, which then need
strong results elsewhere to stay above Fortaleza. Avaí needs at least seven
wins in its other eight fixtures.

The previous probability proposal exactly conditioned on Fortaleza's own
point event, but used soft marginal guidance for its rivals. The new sampler
locks necessary rival outcomes and rejects a fixture option as soon as it
would make a required rival threshold unattainable. Locked results retain
their original probabilities in the importance weights.

## Algorithm and probability accounting

1. Sample a target fixture assignment from the existing exact point event.
   Its point-tilt likelihood ratio and event mass retain their existing role.
2. Compute each rival's attainable minimum and maximum ordered score. The
   score includes wins only when points then wins form the standings prefix
   and the existing wins-aware lookahead mode permits it.
3. Count rivals that must be strictly above or below the target. If all rank
   slots on a side are occupied, every other rival receives a necessary cap
   or floor. Equal ordered scores remain eligible for either side because
   later tiebreakers can decide their order.
4. Propagate these caps/floors through shared fixture domains to a fixed point,
   using the same minimum/maximum gain reasoning as the reachability solver.
   Remove an outcome only when it makes a required threshold impossible.
5. Precompute minimum/maximum suffix gains under the surviving domains. At
   each sampled fixture, remove options that would prevent either team from
   meeting its cap/floor even with its best remaining results.
6. Renormalize the surviving lookahead proposal and multiply the season
   weight by the original outcome probability divided by the proposal
   probability. A forced result has proposal probability 1, but still
   multiplies the weight by its original probability. Full rank evaluation
   retains the existing points/wins screen and standings sorter for ties.

Propagation is cached by the sampled selected-fixture outcomes: at most 64
patterns per sampler call, with up to 32 selected fixtures encoded in the
key. Cache overflow falls back to the original unrestricted proposal for
that pattern; it never removes support or silently declares impossibility.
An inconsistent propagated assignment contributes a zero-weight draw.

## Bounded production allocation

The adopted pass leaves all earlier proposal pilots, confirmations, peer
searches, and their random streams intact. After they finish:

- Consider only remaining zeros not proved impossible, prioritizing by the
  existing upper bound, then deterministic team/rank tie-breaking.
- Inspect at most 24 cells. Admit a cell when one side's rank slots are
  necessarily occupied throughout **all** its attainable target totals.
  The eligibility screen uses points/wins envelopes when wins are permitted.
- Run a fresh 1,000-draw constrained pilot at rank tilt 6. Point tilt is
  +0.5 for a climb from current standing and -0.5 for a drop.
- Require at least three pilot hits and ESS at least 3. Select at most three
  finalists by pilot ESS. Pilots never contribute to reported probabilities.
- Confirm each finalist with 15,000 fresh draws at tilt 6 and independently
  check it with 5,000 fresh draws at tilt 3, using the same necessary domains.
- Prefer a check that passes the existing production gate. Otherwise require
  a valid main confirmation and apply the existing 30-fold discrepancy check
  when independent evidence is conclusive. An inconclusive check cannot make
  an invalid main estimate acceptable.
- Apply the existing gate: weighted positive estimate, at least 30 hits, ESS
  at least 8, relative SE at most 0.35, largest weight share at most 0.25,
  and batch gap at most 1. Reconcile through the existing matrix balancing.

The maximum added simulation budget is 84,000 season draws per request,
including pilots and independent checks, with at most four workers. Setup
and propagation have additional costs measured by full-request timing.
Pilot rank hits also provide reachability evidence for cells whose final
probability estimate is not accepted.

## Experiments and rejected alternatives

### Replacing all lookahead proposals

The first prototype applied constraints to every existing lookahead sampler.
It found some new cells, including Fortaleza / 18th, but changed pilot
selection and confirmation allocation enough to lose other estimates:

| Input | Zero→nonzero cell-runs | Nonzero→zero cell-runs | Median paired extra time |
| --- | ---: | ---: | ---: |
| 16498 | 4 | 6 | +235 ms |
| 16653, current | 13 | 11 | +247 ms |
| 16653, earlier | 2 | 3 | +9 ms |
| 16982 / 16983 | 0 | 0 | Timing noise |

This replacement was rejected. The final implementation calls the new
sampler explicitly from the added pass; earlier samplers retain their
original behavior.

### Short confirmations over every remaining zero

An unrestricted rescue prototype also exposed a missed dominant mode:
current América-MG / 13th repeatedly produced an apparently stable moderate
tilt estimate near `3e-16`, while the previous deeper rank-tilt-12 runs
produced `1.01e-10`–`1.07e-10` with ESS around 2,000. This cell's target point
event spans constrained lower-total assignments and less constrained
higher-total assignments. Short moderate proposals can find the former
without discovering the latter, which dominate its probability.

That result was not adopted. The final bounded pass requires a uniformly
occupied rank side across attainable target totals. América-MG / 13th
fails that admission rule and stays with the existing general searches.
The final acceptance helper also explicitly rejects invalid confirmations
even when their independent check is inconclusive.

### Adopted paired full requests

Each pair uses the same saved input and seed. Even seeds run the constrained
arm first; odd seeds run the baseline first. Seeds are 801, 804, 808, 817,
and 911. Baselines use 20,000 initial MC seasons, the 100,000-season matched
point pool, existing conditioned searches, cross-checks, and peer rescue.
CEM importance sampling is disabled. All runs use four cores.

| Input | Additional estimates per run | Cell-run gains | Distinct gained cells | Lost cells | Median paired extra time | Median paired time change |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 16498 | 0, 0, 0, 0, 0 | 0 | 0 | 0 | +19 ms | +1.3% |
| 16653, current | 2, 2, 2, 1, 2 | 9 | 4 | 0 | +110 ms | +10.5% |
| 16653, earlier | 0, 1, 0, 1, 1 | 3 | 1 | 0 | +59 ms | +5.2% |
| 16982 | 0, 0, 0, 0, 0 | 0 | 0 | 0 | +2 ms | +0.2% |
| 16983 | 0, 0, 0, 0, 0 | 0 | 0 | 0 | -4 ms | -0.5% |

Negative differences are timing noise. Per-cell probabilities that were
already nonzero remained unchanged within `1e-8` relative tolerance in these
pairs; matrix balancing can make tiny changes to their last digits.

All probabilities below are fractions, not percentages:

| Newly filled cell | Runs gaining estimate | Probability range |
| --- | ---: | ---: |
| Current 16653 / Fortaleza (22) / 18 | 5 | `8.45e-19`–`2.33e-18` |
| Current 16653 / Sport (77) / 19 | 2 | `7.83e-13`–`7.89e-13` |
| Current 16653 / Juventude (12) / 18 | 1 | `3.58e-15` |
| Current 16653 / Atlético-GO (279) / 18 | 1 | `1.54e-12` |
| Earlier 16653 / Ponte Preta (9) / 14 | 3 | `4.91e-15`–`7.74e-15` |

Fortaleza's independent deeper checks were approximately `2.2e-18`.
The new 21,000-draw pilot/confirmation/check budget per selected cell recovers
the same rough magnitude with much less work than the earlier 10× experiment.
The variability in the gentle short estimates remains visible; this change
is intended to recover order-of-magnitude estimates, not exact probabilities.
Neither reachability nor a low-variance sampled estimate proves that every
important probability mode has been found.

## Verification and reproduction

- Exhaustive small coupled-fixture tests check every assignment and every
  possible rank, both with and without encoded wins. They verify that
  propagation and sequential pruning retain every assignment that could
  finish at the requested rank, including ties.
- Independent plain conditional MC and the weighted constrained sampler
  agree on the small synthetic group within their sampling uncertainty.
- A saved current-16653 regression proves that the propagated domains lock
  all eight Londrina wins, admits Fortaleza / 18th, excludes the mixed-mode
  América-MG cell, finds a default-enabled estimate in seed 808, and retains
  every baseline nonzero cell.
- Confirmation tests cover invalid selected estimates, independent valid
  checks, and contradictory unstable evidence.
- `go test ./go` passes. `go test -race ./go -run 'TestRankDomain'` also passes
  with the saved current request supplied for the regression test.

Input hashes and source request paths match the
[tenfold work report](2026-09-29-tenfold-work-budget.md). Raw results remain
under ignored `experiments/rare_positions/local/2026-09-29-propagated-domains/`:
`replacement/` contains the rejected replacement prototype; `uniform/`
contains the adopted paired measurements. The intermediate `rescue/`
prototype was not adopted. Request exports are not committed.

Run the paired experiment from the repository root:

```sh
GOCACHE=/private/tmp/golaberto-go-cache GOMAXPROCS=4 \
RARE_POSITION_DOMAIN_EXPERIMENT_REQUESTS=/private/tmp/golaberto-group-16498-current.json,/private/tmp/group16653-live-20260927.json,/private/tmp/golaberto-real-fixtures/group-16653.json,/private/tmp/golaberto-group-16982-current.json,/private/tmp/golaberto-real-fixtures/group-16983.json \
RARE_POSITION_DOMAIN_EXPERIMENT_OUTPUT="$PWD/experiments/rare_positions/local/2026-09-29-propagated-domains/verification" \
go test ./go -run '^TestPropagatedDomainsFullRequestExperiment$' -count=1 -v -timeout 10m
```

The adopted measurements preceded the final pilot-witness metadata update;
that update records reachability evidence without changing proposal selection
or probability estimates. The saved default-enabled seed-808 regression was
run after that update. No probability floors or interpolation were added.
