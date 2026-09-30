# Forced-fixture boundary compaction and constrained peer allocation

## Decision

Enable both changes by default in the matched-point production path:

1. Safely compact forced fixtures when a proposal guide coefficient is zero.
2. When the first directional peer confirmation succeeds without a retry,
   use its unused retry allowance for one remaining constrained peer cell.

The second change estimates Cruzeiro/20th for the reported group 16498 seed.
The first provides a small execution saving; it does not itself find new cells.
These are generic rules, with no production team, group or finishing-rank list.

## Implementation

### Compaction with a zero guide

The original compaction requires both guide coefficients positive. A coefficient
can be zero when the requested count of rivals on that side is zero. Skipping a
forced fixture then requires checking that the original proposal would not
reject it with zero score.

For each cached exact target-fixture assignment, check every possible scalar
points/wins prefix interval at each forced fixture. Evaluate the original suffix
CDF and multiplication order. If any forced score is zero, nonpositive or
underflows, retain the original loop. Otherwise preapply its points/wins and
original probability factor. Keep the original CDFs, future-point offsets and
RNG draws, so later proposals and seeded hits remain the same.

The interval check is conservative: a prefix included in the interval may itself
be unreachable. That can prevent an optimization, but cannot remove support.
The cache is local to one fixed-tilt sampler call, so its assignment key also
fixes the guide coefficients being checked.

### Allocation to a second constrained peer

Keep the primary peer selection, 5,000-draw pilot, 50,000-draw confirmation and
optional 50,000-draw retry unchanged. If that first confirmation succeeds
without retrying:

- Consider remaining zero cells already flagged by directional peer evidence.
- Require the existing necessary uniform-rank-side eligibility check across
  attainable target totals. This excludes the partly constrained modes for which
  prior short experiments missed dominant probability contributions.
- Probe up to eight eligible candidates, drawing 32 target-only assignments
  each. Propagate necessary points/wins domains and score the fraction of rival
  outcome options removed. Infeasible probe assignments contribute zero to the
  scheduling score. This is neither a probability estimate nor a proof that an
  unobserved candidate is impossible.
- Choose the highest positive score, breaking ties by the existing upper-bound,
  point-mass, team-ID and rank ordering.
- Run a 5,000-draw propagated-domain pilot at rank tilt 3 and point tilt
  +/-0.5. If it finds no hits, stop. Otherwise record its reachability witness.
- Use a different seed for 45,000 fresh confirmation draws. Report an estimate
  only if it passes the existing full validity gate: at least 30 hits, ESS >=8,
  relative SE <=0.35, largest weight share <=0.25 and batch gap <=1.

The pilot does not contribute to the reported probability. No validity threshold
was relaxed. The pass still draws at most **105,000 full fixture assignments**:
either 5k+50k+50k for the primary, or 5k+50k+5k+45k for two cells. Additional
preprocessing is bounded by eight times 32 target-only assignments and domain
propagation. Average work can rise because an allowance previously left unused
is now spent.

## Experiment design

Four cores (`GOMAXPROCS=4`), 20,000 scout seasons and the existing 100,000-season
matched-point pool. Explicit matched-pool/conditioned search features on and
legacy importance sampling off, as in the preceding production experiments.
Independent timing experiments ran sequentially; no competing benchmark jobs.

Five saved reference inputs, seven seeds each:

| Group / snapshot | Input SHA-256 prefix | Unplayed fixtures |
|---|---|---:|
| 16498 current | `44eabb47` | 103 |
| 16653 current | `71d4fea8` | 85 |
| 16653 earlier | `2d1c1d6f` | 90 |
| 16982 current | `9327edcd` | 330 |
| 16983 saved | `43969b02` | 311 |

Seeds: `801,804,808,817,911,1790744713633556000,1790746108560577000`.
Paired arm order alternates by seed parity. Every input has 400 cells, giving
14,000 cell-run comparisons per ablation. The two group 16653 snapshots are
separate inputs, not separate groups.

### Allocation ablation: 35 paired requests

Compaction enabled in both arms; toggle `RARE_POSITION_CONSTRAINT_PEER_RESCUE`.

| Input | Mean baseline ms | Mean allocation ms | Total time change |
|---|---:|---:|---:|
| 16498 current | 1,333.5 | 1,363.5 | +2.25% |
| 16653 current | 1,120.8 | 1,145.9 | +2.24% |
| 16653 earlier | 1,120.1 | 1,122.8 | +0.24% |
| 16982 current | 905.8 | 906.7 | +0.10% |
| 16983 saved | 914.1 | 906.9 | -0.79% |
| All requests | | | **+0.95%** |

**One nonzero cell-run gained, zero lost.** That is one distinct cell:
Cruzeiro/20th at seed `1790746108560577000`. No coverage improvement was observed
on the other four inputs. The timing variations on those inputs do not establish
an execution change: additional sampler work was allocated in only two requests,
both group 16498. Seed 911 spent extra work without a valid new estimate.

For the reported seed:

| Quantity | Before | After |
|---|---:|---:|
| Cruzeiro/20th probability fraction | 0 | `1.566508e-18` |
| Cruzeiro/20th probability in percent | 0 | `1.566508e-16%` |
| Reachability | undecided | witness |
| Confirmation draws | | 45,000 |
| Confirmation hits | | 83 |
| ESS | | 12.894 |
| Relative SE | | 0.2785 |
| Largest weight share | | 0.1707 |
| Batch gap | | 0.7701 |
| Full request runtime | 1,302.8 ms | 1,464.4 ms |

The request gained 161.6 ms (**12.4%**) and 6,030,296 conservative modeled fixture
work units (**7.31%**). The secondary search itself used 6,150,000 units plus
3,296 probe units; finding the cell eliminated a later 123,000-unit domain pilot.
Cruzeiro/19th remained positive at about `1.075907e-12` probability. The 20th
estimate is about 687,000 times smaller; present standing does not imply that
20th is more probable than 19th.

This remains a finite-sample importance estimate, useful for order of magnitude.
It does not guarantee every seed will resolve the cell or certify its exact
probability.

### Compaction ablation

Allocation enabled in both arms; toggle `RARE_POSITION_COMPACT_ZERO_GUIDE`.
Another **35 paired full requests** preserved all hit/sample counts, work counts,
designs, reachability labels and acceptance decisions. Largest relative
probability difference: `1.75e-14`, from floating-point accumulation order.
Total runtime differed by -0.09%, which is too small to claim a whole-request
speedup from these measurements.

Isolated boundary kernel benchmark: five repeats of five complete 45,000-draw
calls, rank tilt 3, point tilt -0.5, reported request seed and the existing
directional confirmation label. Medians:

| Cell | Original loop ms | Safe compact loop ms | Reduction |
|---|---:|---:|---:|
| Cruzeiro/20th | 116.404 | 115.778 | 0.54% |
| Bahia/20th | 126.252 | 121.711 | 3.60% |

The 18 forced fixtures previously observed at Cruzeiro's feasible 50-point
assignments are not fixed in every conditioning case. Many draws use other
totals or fall outside the bounded domain cache. Therefore the average benefit
is much smaller than removing 18 of 93 fixtures from every draw would suggest.

## Verification and reproduction

Tests cover zero-score rejection, numeric underflow, allowed wins ordering,
exhaustive coupled-prefix safety, seeded boundary equivalence, full-request
no-loss regression for the reported seed, and preservation of the earlier
Cruzeiro/19th retry regression. Full Go suite and targeted race tests pass.

Use the existing production flags, including `RARE_POSITION_MATCHED_POINT_POOL=1`.
No additional enabling flag is required. Rebuild/restart the Go service.
Separate opt-outs for diagnosis:

```sh
RARE_POSITION_COMPACT_ZERO_GUIDE=0
RARE_POSITION_CONSTRAINT_PEER_RESCUE=0
```

Paired experiment harnesses:

```sh
GOCACHE=/private/tmp/golaberto-go-cache GOMAXPROCS=4 \
RARE_POSITION_CONSTRAINT_EXPERIMENT_REQUESTS='<comma-separated absolute JSON paths>' \
RARE_POSITION_CONSTRAINT_EXPERIMENT_OUTPUT='<absolute output directory>' \
RARE_POSITION_CONSTRAINT_EXPERIMENT_SEEDS='801,804,808,817,911,1790744713633556000,1790746108560577000' \
go test ./go -run '^TestConstraintPeerFullRequestExperiment$' -count=1 -v
```

For compaction, use the corresponding `RARE_POSITION_COMPACT_EXPERIMENT_*`
variables and `TestCompactZeroGuideFullRequestExperiment`. For the kernel use
`RARE_POSITION_BENCHMARK_GROUP_JSON` and `BenchmarkCompactZeroGuide`.

Raw paired matrices and benchmark/test logs remain ignored under
`experiments/rare_positions/local/2026-09-29-constraint-peer/` and
`experiments/rare_positions/local/2026-09-29-zero-guide/`. Saved requests remain
local; no database contents are committed.
