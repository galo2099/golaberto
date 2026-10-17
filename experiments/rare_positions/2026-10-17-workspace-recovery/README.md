# Rare-position experiment recovery

## Status

The managed workspace became accessible again on 2026-10-17. The accessible
checkout was a clean `f9a8658580b923ffaa13ee22837c5f2faf949cc4` (PR #111).
The recent experimental worktree and its artifact directory were absent.
`git fsck --full --no-reflogs --unreachable` found no unreachable objects.
The scratch and shared download directories contained no recovery archives.

This branch preserves the recovered checkout and information retained in the
conversation. It must not be presented as a backup of all the recent source.
The original implementation agents recovered the four Set 4 Python drafts
from their retained successful patch records. Those drafts are included under
`../2026-10-16-frontier-joint-growth/trusted-highs/gd-set4-split/`.
They are incomplete and cannot run without the missing frozen dependencies.
All four pass Python syntax compilation; functional validation has not run.
The latest benchmark described below never ran. Numerical results below are
transcribed from the retained session summary; their raw outputs were not
available for fresh verification after recovery.

Missing paths include:

- `/workspace/golaberto-frontier-joint-growth`
- `/workspace/golaberto-frontier-separator-rebuild`
- `experiments/rare_positions/2026-10-16-frontier-joint-growth`
- The associated `/tmp/frontier-joint-growth-runs` and
  `/tmp/frontier-trusted-highs-runs` outputs and binaries.

The missing experiment directory previously held source and result archives,
including `validation/source-primary-headroom.tar.gz`,
`validation/primary-2350-raw.tar.gz`, and
`trusted-highs/validation/executed-wrapper-source.tar.gz`.
These archives were not found in the restored filesystem. Their prior presence
does not establish that they survived outside this restored snapshot.

## Last completed experiment

The latest completed comparison used an adaptive joint-conditioning frontier
with optional trusted HiGHS prescreening, targeting Palmeiras finishing 16th.
The user explicitly authorized trusting HiGHS numerical infeasibility without
exact certificate verification. Any guarantee therefore depends on that solver
assumption, in addition to the statistical procedure's other assumptions.

Seed: 2360. Workers: 4. Shared wall allowance: 180 seconds per arm, including
prescreen/setup, best effort.

| Arm | Reported lower bound | Reported upper bound | Draws | Total wall time |
| --- | --- | --- | --- | --- |
| Adaptive joint growth | 7.844225251337929e-61 | 2.294377864105339e-30 | 4,000,000 | 180.140 s |
| Trusted HiGHS + same estimator | 7.844225251337929e-61 | 5.605805720031053e-31 | 1,295,616 | 174.859 s |

The treatment prescreen took 0.923248 seconds and classified 22 of 32 unresolved
branches infeasible, leaving 10. Together with 76 previously certified zeros,
this yielded 98 zeros and 10 live cohorts among 108. The resulting interval did
not meet the order-of-magnitude target: upper/lower was approximately 7.14641e29.

The comparison had a recorded provenance exception: the prescreen and its test
changed after the plan was written but before treatment started. The retained
summary says the edits affected comments and inventory validation rather than
the algorithm. Do not characterize this as a clean frozen-source comparison.
The original provenance audit itself is among the missing artifacts.

Set 4 dominated the treatment upper bound: 3.984692913711509e-31, about 71.08%
of the total, after 746,112 draws. Three cohorts without a mandatory GD
comparison (0, 3, 4) accounted for approximately 76.42% of the upper bound.
The other seven live cohorts required a GD tie comparison collectively.

## Pending Set 4 partition experiment

The next experiment was intended to split Set 4 by team 79's remaining outcomes
so that the existing native pair-GD integration could tighten the tied branches
before simulation. This experiment was not run.

For this cohort Palmeiras finishes with 57 points and 16 wins after 10 losses.
Team 79 starts with 29 points, 7 wins, and 10 remaining games. The relevant
remaining outcome patterns for team 79 are:

- 10 wins: finishes on 59 points and 17 wins; no tie with Palmeiras.
- 9 wins and 1 draw: finishes on 57 points and 16 wins; GD can decide the order.

Both Palmeiras/team-79 meetings are already played, a correction to an earlier
draft: game 363886 was Palmeiras 2-1 team 79; game 364073 was team 79 0-0
Palmeiras. There are therefore TEN possible draw locations, not nine.

The exhaustive split has 11 disjoint children. Keep original cohort index 4 as
the 10W child and append the ten 9W1D children at indices 108 through 117.
Keep every other original cohort index stable. Expected inventory: 118 cohorts,
98 existing zeros, 20 initially live. Root 0's prepared count increases from
16 to 26. Root count 486 and candidate-slot count 2425 remain unchanged.

Team 79's remaining fixture IDs:

`364086, 364094, 364105, 364110, 364124, 364133, 364143, 364158, 364162, 364173`.

Each child must intersect its fixed outcomes with the parent's fixture masks,
retain the same 15 above-Palmeiras opponents, preserve the original seven-rival
selection and `joint_points_diagnostic.separator_certificate`, and discard
historical Monte Carlo moments/bounds. The original analytic parent cap
`cohort_event_prior_upper = 2.7967748868729874e-27` remains valid for every
child, but summing that inherited cap independently can be loose. Root-path
probability is already included and must not be multiplied again.

For each 9W1D child, all future fixture outcomes incident to the Palmeiras/79
pair are fixed (20 incident fixtures, none shared). The intended native method
then integrates `P(GD79 >= GDPalmeiras | fixed incident W/D/L)`; equality remains
in the upper bound because goals scored may decide it. Multiply that conditional
factor by the corresponding joint W/D/L cap, respecting shared fixture effects.
This first-moment bound does not by itself create a better score proposal.

Planned control/treatment: seed 2370, four workers, 180000 ms per arm including
all wrapper preparation, serial arms, same binary. Both arms rerun the original
32-cohort weak HiGHS screen, keeping timeouts/errors/unknowns. Do not screen the
new children with an extra solver pass. The native budget is the remaining
allowance after preparation. Preserve raw native output and record the actual
passed budget; final diagnostics may exceed the allowance.

Expected native CLI contract from the missing experiment:

```text
frontier-regimes REQUEST FILTERED OUTPUT SEED WORKERS REMAINING_MS --regimes-only
```

The partition prototype is flat: it does not retain an active parent sampler or
parent/child scheduling tree. A grouped Set 4 upper bound can use the minimum
of its original analytic cap and the outward sum of child upper bounds. Do not
intersect with a separate control run's sampled interval without proper joint
confidence allocation.

Before the outage, drafts of `experiment.py`, `test_partition.py`,
`scripts/run_paired.py`, and `scripts/analyze.py` had been written. A subsequent
partition patch preserving diagnostic hints and recording explicit treatment
indices FAILED while the environment was offline. Do not assume those fixes
are present in any recovered draft. Revalidation, a short native probe, the
full paired run, and an independent audit were still pending.

## Recorded fingerprints of missing artifacts

These fingerprints identify prior artifacts if the original volume is restored;
they do not imply that the artifacts are present on this branch.

| Artifact | SHA256 |
| --- | --- |
| `inputs/request.json` | `538859da91dc7825ab412a2da44893592f46d83feb1ee70351b49b199372cefa` |
| `inputs/target-16-rank-15-separator-saved.json` | `ef80774b597cd60fbde8bffce51f10209631fc8db06254ffa78a04ea92aacb9c` |
| Native adaptive-joint-growth `frontier-regimes` | `eb013be81c343762952f6a18abe7f45dda9f37a17dd33064877206c0c4fc60ab` |
| Executed trusted-HiGHS `prescreen.py` | `4f609a32582dd2ae24d46d74eb4384b6ee21eb2554c5f013d2ff58298fbe87df` |
| `pal16-gd-branch-analysis.py` | `6554a84c4b9eed28116d51f2360b2969fcd11842329cc4e4afbdc81e8c2b75a7` |

Recovered draft SHA256 fingerprints:

| File | SHA256 |
| --- | --- |
| `experiment.py` | `b2357997f937f9b456cbeb9faa6cb76756e0f9c04d7d093239654c6777c3a372` |
| `test_partition.py` | `a46d15f55dab4533a099d380abf3c745f874e2546982d44c45caf9c5fc71fc31` |
| `scripts/run_paired.py` | `77cb29e81a3309b8c8da28be7011cea82915c5a16588cf51019d0bcd8b367cc0` |
| `scripts/analyze.py` | `c75ccbf7585763cc05f47ca79607f5c91462b37b2c446f8d6d374634e3fe131c` |
