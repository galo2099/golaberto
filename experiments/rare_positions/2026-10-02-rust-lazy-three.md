# R45–R47: three hypotheses for unresolved rare-position estimates

## Scope and decision

Baseline: shipped Rust commit `ac92a8ab88656c2e20a24b37c1f3c3d3dcfdb4cd`,
including the default complete-tree proposal. All prototypes are isolated under
`/private/tmp/golaberto-r45`; production Rust sources and its executable remain
unchanged. No commit or push.

At final verification, repository HEAD is `4e812fd5` (the AGENTS.md model-role
update). Its Rust code is unchanged from ac92a8ab. The reread instructions assign
source edits to Luna and reasoning/review to Sol; remaining corrections and
reproducibility helpers follow those roles. Pronto was not used.

Four estimator workers, serialized requests, identical snapshots and fixed seeds.
No additional time allowance. Constructor work, proposal guidance, density
replays and ranking are charged to the existing deterministic work bank.
Elapsed time is diagnostic. Keeping the bank ceiling does not guarantee equal
CPU usage or latency, so both are measured separately against current Rust.

Results reject all three hypotheses as implemented. The pair guide also
received a caching revision and a separate review of allocation limits and
operation accounting. Final corrected-revision results are recorded below.

## The three hypotheses

### R45: several weighted pilot witness modes

Existing R4/R14 already use witnesses and rival masks. The new question is
whether several **high-contribution seasons from the existing pilot** are better
anchors than the first available witness.

- Preserve the original 1,000-draw initial pilot and its RNG sequence.
- For each cached target path, retain at most three contributing seasons with
  distinct masks of rivals whose final packed points/wins are at least the
  target's total. Within a mask, retain the largest corrected contribution.
- Build witness-conditioned proposals for up to eight cached target paths.
- Allocate 50% to the native parent and 50% to the learned modes, with mode
  shares proportional to their retained pilot contributions.
- Use the density of the entire overlapping mixture. Every component capable
  of generating the season contributes to the denominator.
- Replace the existing alternate retry. Shorten its pilot to pay for attempted
  construction and up to four density evaluations. Failed refinement retains
  the parent, with setup work charged. New final batches remain independent.

Local constructor headroom can be reallocated to the learned modes; its cost is
deducted from work available to later sampling. This is not a larger request
bank. These anchors are proposal information, not probability estimates or new
impossibility certificates.

### R46: account for one shared-fixture rival pair

The current default already uses cardinality guidance. It approximates the
number of rivals on either side of the target by independent Bernoulli terms.
Two rivals playing each other are correlated; their shared outcomes can matter
when an exact number of rivals must finish ahead.

Select one uncertain rival pair with at most two remaining mutual fixtures.
Enumerate the mutual outcomes and convolve the independent external-fixture
tails to obtain the pair's joint count distribution:

```
K = [P(neither on counted side), P(exactly one), P(both)]
```

Use this three-term kernel instead of two independent Bernoulli terms when the
current fixture is disjoint from the pair. Other rival correlations remain
approximate. Ties retain the guide's half-chance convention. This guides the
proposal; actual phase sorting and complete P/Q corrections determine estimates.

The first version rebuilt external pair tables per target path and recomputed
the kernel at every eligible step. The second version shares tables for the
same residual guide/pair and reuses the kernel until a sampled fixture touches
either paired team. The separate Sol review required pre-allocation admission
under the guide cap and selection accounting proportional to its traversal.

### R47: integrate one fixture's three outcomes

For each cached target path, choose one residual fixture before sampling,
favoring fixtures whose endpoints have uncertain final rank sides. Generate
the other results with the existing proposal, then integrate all three original
fixture outcomes. Check each alternative with the production sorter and apply
its conditional-score likelihood correction.

Let `z` be the retained residual outcomes, `o` the integrated fixture outcome,
and `d_o = Q(z,o) / [P(z) P(o)]` within the target path. The collapsed conditional
weight uses:

```
numerator   = sum_o P(o) × corrected rank contribution(z,o)
denominator = sum_o P(o) × d_o
```

The ordinary target-path likelihood factor is applied outside this ratio.
Evaluate the full proposal density, including overlap with a witness alternate.
Using the old sampled weight after merely changing a fixture would be incorrect.
Use original fixture probabilities, including an outcome forbidden by one
component. Null proposal draws remain zero. Save an actually contributing
alternative as the witness, rather than the last alternative checked.

This replaces final draws through measured deterministic operation costs. It
does not count the three alternatives as three independent hits.

## Correctness and review

- Necessary domain infeasibility remains the only proof-based rejection.
  Approximate cardinality chances do not prove reachability or impossibility.
- Native conditional support and the existing 10% full target-path component
  remain available. No intermediate-rank inference or team/rank constants.
- Witness and rank contributions use the production sorter, with applicable
  wins and score likelihood corrections. No arbitrary score cap establishes
  unrestricted impossibility.
- Acceptance and independent-confirmation gates are unchanged.
- Existing proof limits are unchanged: constructive search 100 nodes/cell and
  3,200 total; joint cap/floor proofs 500/cell and 10,000/direction; neighborhood
  default 10,000 candidates. This work changes proposals after those stages.

The separate Sol review found no new likelihood bias. It requested three
implementation corrections: pair-table allocation admission before allocation,
linear traversal/appropriate work charges during pair selection, and rollback
of both root allocation and learned portfolios when a combined retry fails.
The last combination requires an extra experimental roots flag and was absent
from the measured default arms. Corrections and their validation are included
in the final prototype.

Validation limits: the marginal-density test partly checks an algebraic
cancellation, rather than independently enumerating forward proposal Q.
The small-league probability comparisons exercise deterministic points/wins/bias
sorting, not stochastic score-tie integration. The likelihood derivation was
reviewed, but those additional tests would be required before promotion. None
of these rare-cell estimates has a validated golden probability here.

## Paired experiment

Seven snapshots, including two versions of group16653:

- `group-16498-44eabb47.json`
- `group-16653-71d4fea8.json`
- `group-16653-2d1c1d6f.json`
- `group-16982-9327edcd.json`
- `group-16983-43969b02.json`
- `group-15902-31d66705.json`
- `group-16413-a2eef0a8.json`

Seeds: `808,1669,1993,2281,2293`. Every arm has 35 paired complete requests.
The ordinary 20,000-season scout and 100,000-season pool are retained. Initial
MC hit/sample counts and game importance are unchanged. All reachability
classifications are unchanged: no new reachable proofs, impossible proofs or
undecided regressions.

Count rare cells consistently with earlier experiments:

```
recovered rare cell = initial MC hits == 0 AND final probability > 0
```

These are **cell-runs**, not distinct new positions. Each repeated recovery in
another seed counts separately for robustness. All arms have net losses, so
none earns proportional additional work credit.

| Hypothesis / revision | Gained cell-runs | Lost cell-runs | Recovered rare cell-runs | Decision |
|---|---:|---:|---:|---|
| R45 weighted pilot witnesses | 2 | 3 | 2,166 → 2,165 | Reject |
| R46 first pair guide | 0 | 18 | 2,166 → 2,148 | Reject |
| R46 cached pair guide | 1 | 11 | 2,166 → 2,156 | Reject |
| R46 bounded, metered cached guide | 1 | 11 | 2,166 → 2,156 | Reject |
| R47 fixture integration | 1 | 8 | 2,166 → 2,159 | Reject |

The corrected R46 allocation/accounting revision has the same gained and lost
cell-runs as the cached version. It does not reverse the coverage regression.

### Which cells changed?

R45 gains Flamengo/12 at seed808 (`P=7.8813e-28`) and Palmeiras/13 at seed2293
(`P=7.6954e-20`). It loses Athletico/19 at seed808 and Flamengo/12 at seeds1669
and2281. Flamengo/12 coverage falls **3/5 → 2/5**. Its new seed808 value is much
smaller than the successful baseline values at other seeds, roughly `1e-25`;
that is an uncertainty signal, not evidence that the smaller value is accurate.

Cached R46 gains Palmeiras/13 at seed1669 (`P=1.0262e-19`). Its eleven losses
include Palmeiras/13 and16, Flamengo/12 and13. Flamengo/12 falls **3/5 → 1/5**;
Flamengo/13 falls **5/5 → 0/5**. All changes occur in group16498.

R47 gains Palmeiras/13 at seed2293 (`P=8.6164e-20`), but loses seven cell-runs
in group16498 and Londrina/4 at seed1669 in current group16653
(baseline `P=7.4038e-23`). Flamengo/12 falls **3/5 → 0/5**.

Full individual gains/losses, probabilities and paired request data are saved
with the artifacts. Current group16982 remains completely nonzero; it supplies
a regression/control check, rather than an opportunity to recover a zero.

## Timing method

Fresh-process paired runs are retained, but warm measurements are preferable
for HTTP deployment costs. Each warm comparison starts one baseline and one
candidate server, sends ten requests to each, excludes the first two requests
from wall/stage medians, and alternates which binary goes first. Only one
calculation is active at a time. CPU totals include all ten requests and startup.

Local ARM/macOS measurements cannot quantify performance on the production
four-core Xeon. No CPU-specific optimization is proposed, and no older Go
timings or one-second budget enter the comparison.

Completed warm measurements for all measured prototypes:

| Arm | Snapshot | Median HTTP ms, baseline → candidate | HTTP change | CPU change | Rare-tail median ms, baseline → candidate |
|---|---|---:|---:|---:|---:|
| R45 | 16498 | 1,333.81 → 1,323.17 | −0.80% | −3.50% | 808.45 → 750.11 |
| R45 | 16653 current | 1,338.25 → 1,411.93 | +5.51% | +5.68% | 732.52 → 785.30 |
| R45 | 16653 earlier | 751.09 → 742.72 | −1.11% | −2.28% | 175.76 → 175.47 |
| R46 cached, before review fixes | 16498 | 1,530.65 → 1,690.13 | +10.42% | +0.99% | 894.59 → 1,079.65 |
| R46 cached, before review fixes | 16653 current | 1,622.30 → 1,623.57 | +0.08% | −0.63% | 797.64 → 906.69 |
| R46 cached, before review fixes | 16653 earlier | 883.84 → 845.23 | −4.37% | −4.55% | 197.44 → 208.27 |
| R46 bounded, metered cached guide | 16498 | 1,375.27 → 1,464.87 | +6.52% | +2.10% | 809.43 → 942.07 |
| R46 bounded, metered cached guide | 16653 current | 1,429.15 → 1,428.47 | −0.05% | −3.80% | 763.06 → 818.98 |
| R46 bounded, metered cached guide | 16653 earlier | 775.65 → 800.62 | +3.22% | +1.61% | 181.50 → 188.84 |
| R47 | 16498 | 1,560.29 → 1,845.31 | +18.27% | +21.08% | 881.17 → 1,246.00 |
| R47 | 16653 current | 1,426.15 → 1,422.82 | −0.23% | +6.73% | 789.41 → 808.36 |
| R47 | 16653 earlier | 946.11 → 1,034.60 | +9.35% | +17.67% | 229.25 → 376.44 |

Compare each candidate with its own paired baseline. Different batches have
different absolute timings; they cannot be combined into a cross-arm speed
ranking. The observed increases are explicit violations of the desired cost
preservation, not an assumed additional allowance.

The bounded pair guide's group16498 peak RSS rises from 410.20 to 429.59 MiB
in this warm batch. Its guide cap prevents unbounded table admission, but cached
pair tables still have a measurable memory cost.

### Why better local guidance did not improve coverage

For Flamengo/12 at seed 808, the original selected pilot has ESS 3.63 and an
estimated 8,343 operations/draw. R45's shortened learned pilot has ESS 4.67 but
costs 14,977/draw; it retains only one witness mode for that target. Its apparent
pilot improvement does not establish that all high-contribution modes have
been discovered.

Cached R46 gives almost the same pilot evidence, ESS 3.65, at 9,554/draw. R47
increases ESS to 4.22 at 12,351/draw. Both use 1,000 draws in these pilot comparisons.
Their ESS per operation is approximately 88% and 79% of baseline respectively.
The improvement in rank guidance or per-draw variance is insufficient to repay
the work it replaces.

Construction and altered pilot costs also change admission of later complete
tree proposals and independent checks. For example, first-version R46 seed808
loses Flamengo/13 in the later tree stage after a shortened1,125-draw pilot sees
no event evidence; baseline publishes that cell from its complete tree. The
tree likelihood itself was not changed. These effects explain how an unbiased
proposal refinement can still reduce full-request coverage.

## Validation

The first prototype passes 146 Rust tests; the cached revision passes 148;
the final bounded revision passes 152. Four database integration tests are
ignored in each suite. The final release build completes successfully before
its binary is frozen. The reconstructed standalone first prototype also passes
`cargo check --release --locked --lib`.

Tests cover all three experiment modes against exhaustively enumerated small
league probabilities, native support, omitted fixtures, corrected contributing
witnesses, pair kernels, cache invalidation, pre-allocation admission, original
pair-selection equivalence and combined retry rollback. The sampler comparison
uses a statistical tolerance; it is not a proof of exact probability equality.
The R47 limitations described above remain.

The late exact-tie fixture initially failed because its chance calculations
did not yield equal candidate scores. Binary-exact fixture probabilities repair
the test while preserving its exact equality and independent old-selector
assertions. Both failed and passing test logs are retained.

Formatting passes for the four changed files. Whole-crate `cargo fmt --check`
reports existing formatting differences in unchanged `proof.rs` and `search.rs`,
both byte-identical to the baseline; those files were not changed.

Four flag-off full-request controls for each of V1 and V3 are byte-identical
to the shipped baseline. Each enabled mode passes seven serialized CLI exports
covering four workers, one worker, repeated execution and quiet logging:
portfolio/marginal on V1 and pair on V3. Parsed outputs match within each arm.
Their raw commands, JSON exports and canonical hashes are retained.

Release SHA256:

- Baseline/production: `010a91e9d5f9432f4ddfdd165d13df3bfefed6de502732aa0e3f99d9cc6fbd46`
- V1: `e85dc46045147b487446bed23964933b6a4e51f51d840c3fdcc71fb766d706dd`
- V2: `f29c935cd1f3ab2d678959770fb094da1f39614f4b9c8725c8e48659d1d9c136`
- V3: `39adb8036c0c3b1386e0e2f4a5cef3854a326fa1343aced62801e5fd9af295fe`

## Recommendation

Reject all three implementations and retain shipped ac92a8ab. They gain a few
individual estimates but lose more repeated recoveries, with explicit latency
or CPU increases on reference snapshots. No arm meets the coverage and cost
requirements. No new proof classifications are produced.

A next hypothesis is to learn modes from high-contribution seasons encountered
in already funded long main/check batches, then freeze those modes for a later
independent retry. The short initial pilot supplies too few modes for R45.
Learning from later batches must not reuse their samples as estimates under
the adapted proposal. This is proposed follow-up work, not an implemented or
measured result here.

## Sources and interpretation

[Owen, *Monte Carlo theory, methods and examples*, chapter9](https://artowen.su.domains/mc/Ch-var-is.pdf)
describes importance likelihoods, mixture proposals and integrating a variable
conditional on the others. [Veach and Guibas (1995)](https://graphics.stanford.edu/papers/combine/)
motivate combining sampling techniques through their proposal densities.
Our soccer-specific witness masks, pair kernel and fixture integration are
derived experimental designs, not conclusions established by those sources.

## Reproduction and artifacts

See [`2026-10-02-lazy-three/commands.txt`](2026-10-02-lazy-three/commands.txt),
the focused prototype patches, source hashes, compact summaries and verified raw
evidence archive. Apply a patch to a fresh archive of the baseline, build with
`cargo build --release --locked -j4`, then freeze the successfully completed
binary before benchmarking. Never copy a binary while compilation is running.

Select each hypothesis separately with:

```
RUST_ODDS_EXPERIMENT_LAZY=portfolio
RUST_ODDS_EXPERIMENT_LAZY=pair
RUST_ODDS_EXPERIMENT_LAZY=marginal
```

These are experimental flags for the isolated prototype. They are not new
production options or recommendations.

Machine-readable results: [`final-summary.json`](2026-10-02-lazy-three/final-summary.json).
Independent review: [`sol-review.md`](2026-10-02-lazy-three/sol-review.md).
The archive manifest hashes original selected files before archiving and checks
every archived member against those hashes. Exact requests, logs, exports,
failed/passing test runs, prototype patches and helper scripts are included;
executables and Cargo target directories are excluded.

Archive verification passes for 937 files (16,380,591 compressed bytes).
See [`raw-evidence.manifest.json`](2026-10-02-lazy-three/raw-evidence.manifest.json)
and [`raw-evidence.tar.gz`](2026-10-02-lazy-three/raw-evidence.tar.gz).
Archive SHA256: `2d9a5ca110d92d6a0b32fff97ec87ff699092bdc94caedd5cead9348779388d2`.
