# Rare finishing-position experiments

The diversified estimator has a fixed 35,000,000 nominal-work limit. The
offline reference is separate and must never be passed to the estimator.

## Inputs

`RARE_POSITION_BENCHMARK_FIXTURES` is a comma-separated list of serialized
`GroupType` request JSON files. To select and export real groups from the local
development database, without updating odds or other records:

```sh
bin/rails runner script/select_rare_position_benchmark_groups.rb /tmp/rare-groups.json
bin/rails runner script/export_rare_position_groups.rb /tmp/real-fixtures 16982 16986 16498 16983 16653
```

The selector reports group IDs with varied standings spread and remaining-game
counts. Recheck its output if the local database changes. Keep the exported
requests and references local; they are not committed to this repository.

For a fixed synthetic suite that can be shared in the repository:

```sh
python3 script/generate_rare_position_benchmarks.py --output /tmp/rare-fixtures
```

The generator creates five 20-team groups (`tight`, `wide`, `dominant`, `weak`,
and `blockers`), each with 330 unplayed games and 50 deterministic played
games. A checked-in copy and its 5M-simulation reference matrices live in
`go/testdata/rare_position_benchmarks`. These are controlled scenarios, not a
substitute for real-group validation.

## Build references

```sh
RARE_POSITION_BENCHMARK_FIXTURES=/tmp/rare-fixtures/group-99001-tight.json \
RARE_POSITION_BENCHMARK_OUTPUT=/tmp/rare-reference \
RARE_POSITION_REFERENCE_SAMPLES=5000000 \
go test ./go -run '^TestDiversifiedBuildReference$' -count=1 -timeout=60m
```

Use a comma-separated fixture list to build several references in one run.
Each `group-ID.reference.json` stores all team-position probabilities, counts,
the sample count, the reference seed, and the SHA-256 of the request. The
benchmark rejects stale references and references below 5M samples. Zero
reference hits do not establish impossibility. Calibration statistics use only
reference cells with at least 25 hits.

## Run a matched-work arm

```sh
RARE_POSITION_BENCHMARK_FIXTURES=/tmp/rare-fixtures/group-99001-tight.json \
RARE_POSITION_BENCHMARK_REFERENCE_DIR=/tmp/rare-reference \
RARE_POSITION_BENCHMARK_OUTPUT=/tmp/rare-candidate \
RARE_POSITION_BENCHMARK_SEEDS=5 \
go test ./go -run '^TestDiversifiedMatchedWorkBenchmark$' -count=1 -timeout=60m
```

Use distinct output directories for baseline and candidate. Each run appends
one JSON object for the diversified estimator and one for plain MC to
`runs.jsonl`. Remove that file before rerunning the same arm; the comparison
script rejects duplicate group/seed/method keys. Each method has the same
35M-work limit and the same master seeds. The JSON includes phase work,
proposal shortlist, validation samples and cell ESS, frozen production
allocation, quality, all estimated cells, and error/calibration by reference
probability band. Proposal discovery and validation data are reported but
never used in the official estimates.

## Compare and record

The one-hypothesis runner executes candidate unit tests, both matched-work
arms, and the paired comparison, then stores logs, JSONL runs, aggregate
metrics, and a decision in a new experiment directory:

```sh
python3 script/run_rare_position_experiment.py \
  --baseline /path/to/baseline-checkout \
  --candidate /path/to/candidate-checkout \
  --fixtures /tmp/real-fixtures/group-16982.json \
  --reference-dir /tmp/rare-reference \
  --output /tmp/rare-experiments/hypothesis-001 \
  --hypothesis 'Redistribute KL to frontier blockers' \
  --seeds 5
```

For a promising result, rerun in a new directory with `--seeds 20` or more.
The runner checks the 35M-work cap and marks failed correctness tests or
invalid output as rejected. It does not alter either source checkout or
promote a candidate commit.

To compare existing JSONL files directly:

```sh
python3 script/compare_rare_position_benchmarks.py \
  /tmp/rare-baseline/runs.jsonl /tmp/rare-candidate/runs.jsonl \
  --hypothesis 'Intended-cell eligibility gates proposal allocation' \
  --decision inconclusive \
  --output /tmp/rare-experiments/intended-gate.json
```

The report records paired candidate-minus-baseline differences with mean,
median, standard deviation, p10, and p90 for every whole-table metric and
reference band. Keep the JSONL files and experiment report for each hypothesis.
Use five matched seeds while iterating and at least 20 before accepting a
change. In addition to the paired result, compare each diversified arm with
its equal-work plain MC arm. Treat common-cell variance and calibration as
guards against a tail-only gain.

For local Go tests where the normal cache is not writable, set
`GOCACHE=/tmp/golaberto-go-cache`.

## CEM bank pilot

The 2026-09-25 attack/concession CEM pilot is recorded in
`experiments/rare_positions/2026-09-25-cem-summary.json`. It used the same
five real groups, five matched seeds, 5M references, and 35M work limit. A
stronger KL warm start allowed some CEM proposals to pass independent
validation and slightly improved the current diversified estimator, but it
did not outperform equal-work plain MC. The candidate source and raw runs are
archived under the ignored `experiments/rare_positions/local/2026-09-25/`.
Three follow-up CEM tuning screens are recorded in
`experiments/rare_positions/2026-09-25-cem-tuning-summary.json`. Lower KL,
broader one-step adaptation, and deeper four-proposal validation all lost
whole-table coverage against the original CEM pilot. The most revealing arm
admitted more CEM production work while losing ESS>=10 cells, so tuning
admission alone is not a reliable path to a better estimator.

## Group 16653 multi-cell CEM trial

`experiments/rare_positions/2026-09-25-group-16653-multicell-cem.json`
records a focused 20-seed comparison. A CEM update trained on a window of
adjacent rare positions activated nine times, but did not improve the ESS>=10
or ESS>=25 cell counts. Allowing the adjacent low-count observed cell to admit
the proposal improved the five-seed screen modestly, but the 20-seed result
remained below equal-work plain MC. The candidate code, comparisons, per-run
JSONL, and logs are archived in the ignored local experiment directory.

A further five-seed smooth-rank CEM check is recorded in
`experiments/rare_positions/2026-09-25-group-16653-smooth-cem.json`.
Both a broad and a narrow rank-distance kernel activated CEM updates, but
neither improved ESS>=10 coverage. The broad kernel reduced targeted
production work, while the narrow kernel lost ESS>=25 coverage. Both remained
below equal-work plain MC. The experimental CEM code was removed from the
production path; its source and results are preserved in the ignored archive.

The next focused allocation check raised the frozen production utility target
from ESS 10 to 50 for independently admitted CEM proposals. It allocated more
targeted work in two of five runs but added no ESS>=10 cells, lost ESS>=25
coverage, and remained below equal-work plain MC. The result is recorded in
`experiments/rare_positions/2026-09-25-group-16653-allocation.json`; the
temporary allocation parameter was removed.

## Team 125 in group 16653: points-stratum hybrid

`experiments/rare_positions/2026-09-25-group-16653-point-stratum.json`
records the 20-seed result. Team 125 has 17 points and nine unplayed games.
Finishing 16th or better requires at least 15 more points, so its probability
cannot exceed the fixture model's exact `P(additional points >= 15) = 0.0763297485`.
The production stratum uses the narrower event `A = additional points >= 21`,
whose exact probability is `0.00154226665`.

The implementation computes the points-tail probability with a small dynamic
program over each remaining fixture's Poisson win/draw/loss probabilities.
Conditional samples draw a result pattern from that exact conditional law,
then draw goal scores from the original Poisson law conditional on each result.
Every other fixture remains under the original model. For team 125 positions
1–16, the fresh-production estimate is

```text
P(rank, outside A) from plain MC + P(A) × P(rank | A) from conditional MC
```

All other team-position cells use fresh plain MC. The scout, pure-Q discovery,
and independent validation are design-only; they never enter official
estimates. Production counts are frozen before either stream begins. The
candidate stays within 35M nominal work and uses 2% of production work on the
conditional stream after validation admits it.

The 20-seed candidate gained 0.85 whole-table coverage-score points over the
current diversified estimator and 2.25 ESS>=25 cells. It was essentially tied
with equal-work plain MC on whole-table coverage (mean difference -0.014),
while team 125's 15th-place mean ESS rose from 0.8 to 5.0 and its 16th-place
mean ESS rose from 17.0 to 98.6. Common-cell estimated variance was about
3.1% above equal-work plain MC. The option is therefore kept explicit for this
group and team:

```sh
RARE_POSITION_DIVERSIFIED_IS=1 \
RARE_POSITION_POINT_HYBRID_GROUP=16653 \
RARE_POSITION_POINT_HYBRID_TEAM=125 \
RARE_POSITION_POINT_HYBRID_MAX_RANK=15 \
RARE_POSITION_POINT_HYBRID_Q_PERCENT=2
```

The group guard keeps the general estimator unchanged. If the group fixture
changes, rerun the matched-work benchmark before using the option again.

The corresponding 20-seed group 16982 trial, both groups' full results,
limitations, and reproduction settings are in
[`experiments/rare_positions/2026-09-25-point-stratum-hybrid.md`](../experiments/rare_positions/2026-09-25-point-stratum-hybrid.md).
