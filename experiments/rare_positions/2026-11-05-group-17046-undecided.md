# Europa League group 17046: November 5 reachability analysis

The [production fix](2026-11-05-group-17046-cohort-fix.md) now resolves these
cells under the original limits. The analysis below records the production
baseline before that change.

Production leaves between one and three cells undecided on the new request,
depending on the seed. There are four distinct affected cells. Every one is
reported reachable in another production run. The direct late witness search
skips these cells at its cohort-count preflight, before spending search nodes.

## Production baseline and fixture

The base is freshly fetched `origin/master`, commit
`91b1226c2d7f9c03301f946c6cd05d2bac2db8ad`, in a clean checkout. Earlier
uncommitted training experiments are preserved in the separate original
checkout. No experimental flags or larger budgets are enabled here.

The uploaded request is stored unchanged in
[group-17046-2026-11-05.json](../../odds-rust/tests/fixtures/group-17046-2026-11-05.json).
Its SHA-256 is
`b3440005e6633db297252c25d9dbb941d7730667cdaac4f5e1aac73a3c4c0f1a`.
It has 36 teams, 144 matches, 72 played and 72 remaining, with four played and
four remaining matches for each team. The sort is
`pt,gd,gf,g_away,w,name`, with 3/1/0 scoring and no bonus points. All future
Poisson means are finite and positive.

The freshly built production executable SHA-256 is
`a367e66936e7435966fc7955693f4ac55b1b442c88f70bdca5fa084f8e512556`.
Runs clear inherited `RUST_ODDS_*` and `RARE_POSITION_*` variables, then enable
only logging. They use four workers and seeds 808, 809, 1669 and 2293. Seed
808 also runs with one worker and produces a byte-identical response.

| Seed | Positive estimates | Proven reachable | Proven impossible | Undecided | Unestimated |
|---|---:|---:|---:|---:|---:|
| 808 | 1,214 | 1,285 | 10 | 1 | 72 |
| 809 | 1,214 | 1,284 | 10 | 2 | 72 |
| 1669 | 1,210 | 1,284 | 10 | 2 | 76 |
| 2293 | 1,208 | 1,283 | 10 | 3 | 78 |

Each response contains all 1,296 cells. Unestimated means zero probability
without an impossibility certificate, including undecided cells. Proven
reachable cells can still lack an accepted probability estimate.

## The four cells

Positions below are one-based. Team IDs are the identifiers supplied by the
request.

| Team | Position | Seeds leaving it undecided | Current points | Current GD |
|---|---:|---|---:|---:|
| 3169 | 30 | 808 | 10 | +8 |
| 402 | 30 | 809, 1669, 2293 | 10 | +3 |
| 402 | 31 | 809, 1669, 2293 | 10 | +3 |
| 2436 | 6 | 2293 | 1 | −8 |

Team 3169's thirtieth-place probability is estimated at seed 2293; team 2436's
sixth-place probability is positive at the other three seeds. Team 402's two
positions are proved reachable at seed 808, although their probabilities stay
unestimated. No cell remains undecided in all four runs.

## Why direct witness search skips them

The late packed search has two orientations. One fixes the target to its
worst outcomes and asks for enough rivals above it. The negated orientation
fixes its best outcomes and asks for enough rivals below it. Here these are
necessary points-prefix relaxations: wins comes after the goal keys, so it is
not part of the prefix. A feasible assignment still needs score completion
and verification through the actual sorter before publication.

For all four cells, all 35 rivals remain eligible in both orientations.
The cohort preflight therefore computes:

| Position | Cohorts in either orientation | Production limit |
|---|---:|---:|
| 30 | `C(35,6) = 1,623,160` | 100,000 |
| 31 or 6 | `C(35,5) = 324,632` | 100,000 |

[The caller](../../odds-rust/src/search.rs:1306) supplies `max_cohorts: 100_000`.
[The preflight](../../odds-rust/core/src/rank_proof.rs:414) returns an unknown
result as soon as the combinatorial count exceeds that cap. Both directions
return before searching fixtures, and no candidate reaches goal completion.

This is not exhaustion of the 1,000-node per-cell allowance or the 6,000-node
global allowance. The entire packed pass uses only 740, 845, 898 and 843 nodes
across the four seeds, respectively. Raising the node allowance alone cannot
remove this preflight rejection.

## Why reuse does not finish the job

The packed fallback also tries to adapt verified W/D/L seasons from its own
successful constructions. It first checks the points-prefix rank band of a
seed season and calls goal completion only for eligible targets. These seeds
come from newly generated certificates in that packed pass; they do not form
a bank of every earlier verified season.

Reuse makes 24, 22, 25 and 22 attempts across the four seeds, and every attempted
completion succeeds. Neither reuse quota is reached. Thus the final undecided
cells have no usable prefix rank band among the available seeds. Their failure
is not a score-completion rejection after an attempted reuse.

Earlier sampling and recovery change which cells still need the packed pass,
and therefore which primary searches produce reusable seasons. That explains
why a deterministic late search can finish with different unresolved cells for
different request seeds. Existing proven cells are preserved; these are
failures to discover certificates, not certificates being cleared.

At seed 808, the pipeline progresses from 116 initial undecided cells to 109
after early proofs, 44 after deferred witnesses, 41 after rare-tail recovery,
and one after packed fallback. The packed pass supplies three refutations and
37 new construction labels without changing probability estimates.

## Offline verification and checks

The [diagnostic probe](../../odds-rust/examples/packed_witness_probe.rs)
reproduces production's preferred/alternate orientations, outcome selection,
shared 1,000-node per-cell allowance, 512-depth limit, score completion and
exact verification. With the production 100,000-cohort cap, both orientations
return unknown with zero nodes and zero visited cohorts for every cell.
Changing only the cohort cap to two million gives:

| Team | Position | Actual cohorts visited | Nodes | Verified scored witness |
|---|---:|---:|---:|---|
| 3169 | 30 | 1 | 58 | Yes |
| 402 | 30 | 1 | 58 | Yes |
| 402 | 31 | 1 | 56 | Yes |
| 2436 | 6 | 1 | 60 | Yes |

All four positions are therefore mathematically reachable. The preflight's
worst-case count is a poor predictor of the work needed to find their first
witness. This probe evaluates individual cells, not a changed full production
pipeline, so it does not establish the resulting request-wide counts or
latency after a production change.

The witnesses for teams 3169 and 2436 include scores outside the native finite
score-table support. This does not invalidate reachability: every finite
nonnegative integer score has positive mathematical probability under the
positive finite Poisson means in this request. Score completion plus the
production sorter verifies the exact target positions. It does not provide an
accepted probability estimate for those positions.

A suitable production improvement would bound **actually visited cohorts**,
alongside the existing node and depth limits, rather than rejecting the search
solely because its hypothetical total cohort count is large. Cache-hit cohorts
must count toward that separate budget: cached traversal consumes CPU without
consuming fixture nodes. Any interrupted search must remain unknown; only
exhaustive refutation may establish impossibility. Raising the static cap alone
would expand worst-case work and needs separate validation before promotion.

Broader reuse of earlier verified construction seasons is a complementary
option. It still needs deterministic ordering, bounded deduplication,
points-prefix screening, score completion and exact verification. A compatible
prefix or a W/D/L vector alone is insufficient.

The [snapshot tests](../../odds-rust/tests/group_17046_snapshot.rs) check request
identity and schedule consistency and replay the four exact scored witnesses
from [their fixture](../../odds-rust/tests/fixtures/group-17046-2026-11-05-witnesses.json).
The replay checks request provenance, the exact four target cells, and every
certificate through the production sorter, without rerunning enlarged searches.

The new tests and the three existing Europa League regressions pass:
**five passed, zero failed**. The command is
`cargo test --release --locked --offline --manifest-path odds-rust/Cargo.toml --test group_17046_snapshot --test group_17046`.
Formatting and whitespace checks pass. All five baseline native-stage audits
pass, with complete setup audits, no extra setup work, no unknown constructor
failures and no training overruns. Production solver behavior is unchanged.

The [machine-readable results](results/2026-11-05-group-17046-undecided.json)
retain per-seed undecided cells, their estimate metadata, request standings and
opponents, construction diagnostics, stage budget audits and hashes. Raw
exports and logs are under `/workspace/work/production-17046-2026-11-05/`.
