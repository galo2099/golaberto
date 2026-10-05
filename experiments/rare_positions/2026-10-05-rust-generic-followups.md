# Generic controls after the group 16498 fresh 100× experiment

October 5, 2026. Complete. User authorized running follow-up experiment ideas.
Production defaults are unchanged. No database writes, commits or pushes.

## What was run

Five policy ideas were tested on both retained before/after snapshots:

1. Palmeiras15: increase pilots from 500 to 5,000 while retaining three
   refinement slots.
2. Palmeiras16: equal rather than trained allocation.
3. Palmeiras16: increase pilots from 5,000 to 50,000.
4. Palmeiras16: always use bounds guidance rather than pilot-selected guidance.
5. Palmeiras16: native goal scores rather than conditional score importance
   sampling.

All use the frozen R69 example, construction seed 808, four workers, unchanged
gates/weights and positive sampling of every branch. Final samples remain
600,000 per MAIN/CHECK for Palmeiras15 and 2,500,000 for Palmeiras16. P15
retains complete tree cap 256, generic rank priority, production-profile pilots
and floor 2. P16 retains secondary refinement and floor 200. These are focused
offline diagnostics; equal final draw counts do not imply equal work or
production affordability.

Screening uses pilot 60031 and final seeds 60013/60017/60029. Existing R69
panels provide matched controls. Before holdout, the written selection rule
chooses the P16 variant by passing pair count, then minimum aggregate ESS across
MAIN and CHECK, then smaller worst observation share. The selected variant is
bounds guidance. Holdout uses fresh pilot 60109 and final seeds
60101/60103/60107 on both snapshots. P15 holdout compares 500/three slots,
5,000/three slots and 500/four slots. P16 holdout compares the original adaptive
policy against selected bounds guidance. Policy selection is frozen before
holdout; failures are kept and no seeds or sample counts are replaced.

Twenty new panels contain **60 MAIN/CHECK pairs: 53 pass**. All twenty child
processes exit successfully. There are five screening failures and two holdout
failures. Training is fixed within each panel; three finals do not mean three
independent proposal fits. The same final seeds across policies induce
correlation; the three policy estimates at seed 60101 are not independent
recurrences. Training and final seeds change together in holdout, so a
screen-to-holdout reversal cannot isolate their separate effects. The second training seed adds one independent
training realization. All probabilities below are fractions; means include
failed pairs.

The after fixture combines two results and 200 refreshed power fields. Target
points/wins and ten remaining target fixtures are unchanged. These fixtures
cannot isolate the effects of the results from refreshed powers.

## Palmeiras15: discovery improves, but variance moves to another family

Screening with the larger pilot selects the important Corinthians, Vasco and
Vitória alternatives on both fixtures (branches 51/53/54). All six screening
pairs pass. Before mean MAIN/CHECK is 4.545e-31 / 5.126e-31; after is
5.028e-31 / 5.166e-31. Minimum MAIN/CHECK ESS is 115.82 before and 195.21
after; worst weight shares are 5.78% and 4.28%. Thus the larger pilot repairs
the earlier fresh-pilot displacement without adding a refinement slot.

The holdout changes the conclusion about robustness:

| Fixture / policy | Passes / 3 | Mean MAIN / CHECK | Minimum MAIN/CHECK ESS | Worst weight share |
| --- | ---: | --- | ---: | ---: |
| Before / 500 pilots, three slots | 3 | 4.952e-31 / 5.191e-31 | 79.15 | 9.01% |
| Before / 5,000 pilots, three slots | 3 | 4.870e-31 / 5.262e-31 | 89.92 | 8.15% |
| Before / 500 pilots, four slots | 3 | 4.895e-31 / 5.162e-31 | 81.38 | 9.11% |
| After / 500 pilots, three slots | 3 | 6.608e-31 / 5.210e-31 | 10.85 | 25.40% |
| After / 5,000 pilots, three slots | 3 | 6.684e-31 / 5.184e-31 | 12.09 | 24.06% |
| After / 500 pilots, four slots | 2 | 6.670e-31 / 5.129e-31 | 6.36 | 39.47% |

All holdout policies retain the important alternatives. In after seed 60101,
the large weight comes from **Vasco branch 53**, which is refined in every
policy. Its MAIN contribution is about 63–64%, with branch ESS 4.45, 4.89
and 2.58 respectively. This differs from the earlier unrefined Corinthians
failure. Covered, refined families can still have heavy weights within them.

The three-slot baseline accepts MAIN 9.026e-31 versus CHECK 5.488e-31, despite
MAIN ESS 10.85 and a 25.40% maximum share. The larger pilot accepts
8.570e-31 versus 4.998e-31, ESS 12.09 and maximum share 24.06%. Four slots
reject 8.456e-31 versus 5.527e-31, ESS 6.36 and maximum share 39.47%. The
earlier R69 four-slot improvement does not generalize uniformly. Gate
acceptance is an observed diagnostic, not evidence of a tightly determined
probability. Do not publish the elevated MAIN mean as a new reference truth.

The larger pilot also costs more. For after seed 60101, measured modeled pilot
work rises from 198,954,939 to 1,991,966,338 units. Total paired modeled work
rises from 6,256,628,213 to 9,325,170,966, while final sample counts stay fixed.
Different pilots change refinement selection and allocation jointly; this is
a training policy comparison, not an isolated allocation intervention.

## Palmeiras16: the screening winner regresses on holdout

Screening totals across both fixtures include MAIN and CHECK diagnostics:

| Policy | Passes / 6 | Minimum ESS | Worst weight share |
| --- | ---: | ---: | ---: |
| Reused adaptive baseline | 5 | 5.20 | 42.55% |
| Equal allocation | 5 | 2.76 | 58.48% |
| Pilot 50,000 | 6 | 11.70 | 28.17% |
| Bounds guidance | 6 | 29.22 | 15.05% |
| Native goals | 2 | 1.02 | 99.25% |

Equal allocation fixes the after failure but introduces a before failure. The
larger pilot passes all six screening pairs but retains substantial
concentration. Bounds guidance wins the predeclared screening rule.

| Holdout fixture / policy | Passes / 3 | Mean MAIN / CHECK | Minimum MAIN/CHECK ESS | Worst weight share |
| --- | ---: | --- | ---: | ---: |
| Before / adaptive baseline | 3 | 2.993e-40 / 2.737e-40 | 63.53 | 8.78% |
| Before / bounds guidance | 2 | 3.746e-40 / 3.100e-40 | 4.69 | 45.59% |
| After / adaptive baseline | 3 | 2.984e-40 / 2.836e-40 | 21.89 | 19.06% |
| After / bounds guidance | 3 | 2.962e-40 / 2.983e-40 | 21.38 | 19.31% |

Before seed 60107 with bounds guidance fails: MAIN 5.266e-40 versus CHECK
2.886e-40; MAIN ESS 4.69 and maximum share 45.59%. Branch 11 supplies 60.85%
of MAIN with branch ESS 1.78 and a 74.92% maximum share. The original adaptive
holdout passes all six pairs. In the before fixture, the guide intervention
also raises branch 11 allocation from 133,186 to 308,337 draws. This does not
prevent its heavy tail. The first before holdout seed spends about 25% fewer
observed final MAIN operation units under bounds, illustrating that variance
and cost must be considered together; this is not a guaranteed cost reserve. Always choosing bounds is therefore rejected as
a general replacement for adaptive guidance on this evidence.

Guidance selection and allocation interact. Adaptive samples both rank and
bounds pilots and selects the bounds pilot only when its observed ESS is
larger. Equal allocation still uses those pilots for guidance. Always-bounds
also changes the empirical allocation scores, so its effect is not confined
to the final draw kernel. The larger pilot arm changes both decisions.

For the first before screening seed, pilot 50,000 uses 3,073,107,944 modeled
pilot units, versus 307,796,827 for the 5,000-pilot bounds arm. P16 setup and
total modeled accounting are unavailable in this example. Reported pilot and
final operation counters are partial observed costs; null setup/total is not
zero and does not establish a safe production reserve.

### Native scores are a poor replacement

Native scores pass only two of six screening pairs and have a worst weight
share of 99.25%. An accepted before seed 60029 pair estimates MAIN 2.157e-41
and CHECK 8.354e-42, far below the earlier high-work scale. Agreement alone
can coexist with insufficient exploration.

The conditional score proposal has a 10% native mixture, so its score
importance ratio is at most 10 (`goal_tilt.rs`, mixture and final ratio).
Removing it worsens observed concentration. Thus giant weights cannot be
explained solely by an unbounded score correction. Outcome proposal ratios,
allocation and rarity within the structural family remain relevant. This
arm retrains guidance/allocation too, so it measures the whole native-score
policy. Actual score and weight-factor traces are absent; these results cannot
assign every extreme observation to one factor.

## What the experiments establish

The experiments support three separate mechanisms: sparse pilots can omit an
important refinement; empirical allocation/guidance choices can be unstable;
and covered, refined families can retain heavy internal weights. Preserving
distinct families addresses the documented structural blind spot but does not
control heavy weights within a family.

None of the tested setting changes has earned production adoption. The
strongest screening gains regress on fresh training/final seeds. The generic
algorithm must preserve structural coverage, account for work before fitting,
and expose uncertainty from concentrated weights even when a cell is nonzero
or its current gate passes. This campaign does not certify exact probabilities,
global matrix coverage or feasibility inside the existing production bank.

## Artifacts, cost and reproduction

Protocol, pre-holdout selection, scripts, machine analysis and raw records are
in `2026-10-05-generic-followups/`. Each panel stores its exact command,
environment, frozen binary/input/output hashes, exit code and wall/CPU costs.
The Rust binary and source archive are unchanged from R69. Binary SHA-256:
`1c191c1fe9a7d6a07a652748545443f6cb8a4c61e49781fc9a15292ed27ec4f1`.
Source archive SHA-256:
`ee636b4c22485d5e71938bb2a94c7a437ff150aa15c5898eb349d882a95f5c79`.
The new runner is a copy of the R67 runner with only an optional goals argument;
its default remains tilted scores. New source edits are confined to experiment
tooling. No Rust or production source was edited or rebuilt for these runs.

| New measured runs | Wall seconds | CPU seconds |
| --- | ---: | ---: |
| Ten screening panels | 171.66 | 304.86 |
| Ten holdout panels | 159.65 | 247.03 |
| Total | **331.31** | **551.89** |

These sums exclude reused R69 controls, analysis, syntax checks, agent work and
idle time. Single serialized timings are not a controlled latency benchmark.
Python syntax checks pass. Analyzer validation confirms hashes, fixture/cell
identity, settings, seeds, four workers, draw accounting and branch probability
conservation for all twenty panels. Independent Sol review compares the
reported screening/holdout statistics with raw logs. No new tests were added
or run.

Run from the repository root. This representative screening command keeps the
same frozen input and samples; use a new output destination:

```bash
python3 experiments/rare_positions/2026-10-05-generic-followups/scripts/run_panel.py \
  --binary experiments/rare_positions/2026-10-05-100x-backward/data/branch-stratification-v1 \
  --request experiments/rare_positions/2026-10-05-100x-backward/data/after.json \
  --output /tmp/group-16498-p15-pilot5000 \
  --team 16 --rank 15 --draws 600000 --seeds 60013,60017,60029 \
  --allocation trained --guide bounds --pilot-draws 5000 --floor 2 \
  --pilot-seed 60031 --tree-leaves 256 --messages 3 \
  --production-profile --rank-priority
```

The protocol table defines every other policy, and each `record.json` retains
the exact invocation/environment. Holdouts change pilot to 60109 and finals
to 60101,60103,60107. P16 uses rank 16, draws 2,500,000, floor 200 and
`--refine`, with no tree/profile/rank-priority flags. Only native uses
`--goals native`. Analyze retained runs with:

```bash
python3 experiments/rare_positions/2026-10-05-generic-followups/scripts/analyze_followups.py \
  --run-dir experiments/rare_positions/2026-10-05-generic-followups \
  --baseline-dir experiments/rare_positions/2026-10-05-100x-backward \
  --expected-prefix '' --expected-panels 20 \
  --output /tmp/group-16498-generic-analysis.json
```

All larger probability estimates remain provisional. All planned screening
and holdout panels are complete; no pending adoption or experiment is implied.
