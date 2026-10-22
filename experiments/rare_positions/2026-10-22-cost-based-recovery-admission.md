# Cost-based recovery admission

This implements the follow-up to
`2026-10-22-group-17046-estimation-gaps.md`: admit recovery work using
conservative setup and sampling costs, recycle unused grants, and retain cell
counts as secondary safeguards. Total modeled stage capacities, ranking rules,
and independent confirmation streams are retained. Existing probability quality
gates remain, with an additional native/analytic consistency rejection described
below.

## Admission policy

For the native 33–64-team path, the former default of 28 equally funded cells
is replaced by constructor quotes and proposal-specific pilot quotes, with a
128-cell safety cap. Constructors are considered in stable cost order. Training
can spend at most two thirds of the existing native stage capacity, with part
of that allowance protected for pilots before construction begins. Pilots
request 256 draws, replacing the former bounded 1,000-draw request, and
require at least 32 affordable draws. Shorter training preserves more of the
fixed stage capacity for independent finals and other candidates. Unused setup grants
return immediately; unused pilot grants return after deterministic round
barriers. Constructed proposals move into subsequent stages without redundant
cloning. The default finalist cap for this path also becomes 128; explicit
environment overrides remain effective.

The smaller-league Union/adaptive training path retains its existing proposal
builders, witness use, retry behavior, and training-prefix limit. Its constructors
do not yet expose complete conservative common-work meters. Replacing them
with the large-league constructor would change the estimator's proposal family.
The new conservative final-sampling and confirmation reservations do apply to
both native proposal types.

Each final sampler call receives its own conservative work bound. Main and
check reservations sum those two bounds, including fixed preparation separately
for each invocation. Ordinary final work settles before retry admission.
Additional confirmations repeatedly reserve affordable pairs, then speculative
mains, in stable priority order. An unaffordable job does not block a cheaper
one later in the queue. After a barrier, unused main and check work is refunded
exactly once; eligible remaining jobs can run in another round. Main lengths
and RNG streams remain fixed from independent training. A speculative main
without a completed independent check cannot publish.

Expected cohorts and late complete trees retain 128-cell safety limits rather
than truncating their candidate lists at four. Cohorts first admit a bounded
constructor/pilot envelope. Only after selecting the constructed proposal do
they require a conservative 1,000-draw final-pair floor from its sampling bound.
Training uses a work ceiling of two thirds of the current residual bank,
raised to the conservative minimum if necessary. Native pilots must complete
their fixed 1,024 draws before competing for selection; incomplete prefixes
remain diagnostic. Analytic pilots retain their separately planned prefix. Sampled finals use a work ceiling of
two fifths of capacity, similarly raised for the conservative admission floor and
bounded by residual work. Pilot costs with a 25% margin determine the fixed
requested count within that execution cap. These ceilings do not divide work
by the number of candidate cells. Before every draw, the sampler requires the
conservative next-draw allowance, retaining the full goal-preparation reserve. Both streams
must complete their fixed requested lengths to publish; a partial result is
discarded. Actual work settles before the next candidate, so unused grants can
fund later draws and cells. One later candidate's known setup/pilot floor is
protected during a pair. Analytic cohort streams retain their separate bounded
preparation/query accounting, and the former one-cell analytic truncation is
removed. No pilot average serves as a conservative work bound.

A completed native pair also supplies evidence for subsequent analytic zero
recovery. A native stream qualifies only with a finite positive probability,
finite nonnegative standard error, at least 30 hits, and ESS of at least 3. If
its probability minus two standard errors exceeds both analytic streams’
probabilities plus two standard errors, the analytic recovery is rejected.
These are approximate two-standard-error intervals, not rigorous confidence
certificates. The guard only rejects conflicting zero recovery; it does not
certify impossibility or change positive refinements.

Larger fixed batches can expose dominant importance weights that a shorter
pilot or final batch missed. Such evidence can correctly leave a reference
cell unestimated. Regression tests retain the original magnitude and quality
requirements for every published reference, and require explicit rejected-pair
or budget-decline evidence for a remaining reachable zero.

The existing deterministic draw-count diagnostic remains in draw units. The
operation bounds are used for the default modeled-operation scheduler, without
mixing these currencies.

## Root-counting defect

The late complete-tree root counter previously accepted any positive residual
grant and checked its cost only after executing. The new bounded entrypoint
first requires an allowance covering bootstrap work, optional guided-fallback
setup, and complete target-path enumeration. Unsupported inputs decline before
the legacy constructor. If the allowance cannot fit, the candidate is skipped
without consuming its grant, and other affordable candidates can be considered.

The root-path limit remains 256. Enumeration returns a complete cover or an
error; a work decline never establishes impossibility or publishes a partial
cover. Full default-tree construction also preflights bootstrap, enumeration,
and node/guide quotas. Each rank hint then needs its conservative allowance from
the residual grant. An unaffordable hint freezes its leaf without removing that
outcome region. This bounds setup while retaining a complete cover. Final tree
pairs use conservative per-draw costs for all modeled team counts. Refunds
reconcile the original bank with actual modeled operations.

## Regression coverage

Unit checks cover affordable admission beyond the old count limits, skipping
expensive candidates without blocking cheaper ones, refunds across deterministic
barriers, tiny budgets, overrun accounting, and preservation of draw-mode units.
Existing exhaustive sampler tests also verify that measured operations stay
within the new proposal-specific bounds, including cached/uncached roots,
omitted rivals, bias/case variants, and analytic score preparation.

The group-17046 CLI regression audits seeds 808–812, including the three seeds
that previously exceeded their root-count grants. It checks original native
capacity, individual setup and pilot grants, exact refunds, late-tree bank
conservation, and identical seed-808 responses with one/four workers and logging
off/on. The group-17045 regression retains its coverage and known tail-cell
quality requirements while replacing equal-quota assertions with cost and
refund accounting.

The comparisons use the same saved request bytes, seed, worker count, and
20,000-draw game-importance scout. The before binary was saved from rebuilt
source before the admission changes. Probability estimates remain approximate;
improved nonzero coverage does not establish accuracy for the hardest tails.

## Final snapshot measurements

The uploaded group-17046 request uses the original stage capacities. Every
pre-tail zero candidate receives a completed native pilot in these five runs.

| Seed | Baseline final zeros | New final zeros | Native candidates trained | Actual training work | Late-tree actual / available work |
| --- | ---: | ---: | ---: | ---: | ---: |
| 808 | 23 | 20 | 43 | 292,408,960 | 31,068,468 / 38,514,725 |
| 809 | 19 | 13 | 38 | 252,010,474 | 34,094,172 / 41,876,054 |
| 810 | 22 | 16 | 45 | 300,150,877 | 29,892,236 / 40,334,839 |
| 811 | 18 | 16 | 37 | 245,557,910 | 48,861,485 / 55,945,068 |
| 812 | 24 | 19 | 46 | 311,616,092 | 28,223,865 / 28,748,165 |

Training capacity is 414,780,000 operations within the unchanged native stage
capacity of 622,170,000. Seed 808 trains 43 candidates instead of 28. Its
cumulative constructor/pilot quotes total 793,032,620, while actual charges
total 292,408,960: returned grants allow more candidates to run without raising
capacity. All five native, cohort, and late-tree audits stay within their
respective capacities; late-tree refunds reconcile exactly. Maximum row/column
normalization error is 5.7e-14 percentage points. No remaining zero is certified
impossible.

Total final zeros fall from 106 to 84 (20.8%). Across individual cells and seeds,
33 previously zero estimates become positive and 11 previously positive
estimates become zero. Altered work allocation and independently rejected
final pairs explain why coverage is not monotonic for every cell. The
Champions League comparison, group 17045, changes final zeros from
`[2, 1, 2, 3, 0]` to `[2, 1, 0, 2, 1]` for the same seeds. These measurements
characterize coverage and budget accounting, not true tail probabilities.

## Final validation

The complete Rust release workspace run passed all 418 checks outside the
proposal unit target, with five existing database/manual/performance checks
ignored. The new bounded-tree unit test initially contained two assertion
mistakes: treating the exact rank-event mass as total mass one, and looking
for a tree diagnostic outside its `tree` object. Both were corrected in test
code only. Fresh full proposal-unit reruns passed all 141 tests with both
default features and `test-support` enabled, including
exhaustive event coverage and conservative-work checks. The latest target
results therefore total 559 passing tests and five ignored tests. The initial
full command reported that stale proposal-test failure; no production code
changed after the successful service/integration run.

```sh
cargo test --release --locked --offline --manifest-path odds-rust/Cargo.toml --no-fail-fast
cargo test --release --locked --offline --manifest-path odds-rust/Cargo.toml -p golaberto-odds-proposals --lib
cargo test --release --locked --offline --manifest-path odds-rust/Cargo.toml -p golaberto-odds-proposals --lib --features test-support
```

The targeted service release checks also pass: 184 unit tests, expected cohorts,
groups 17045 and 17046, HTTP, and positive-branch recovery. Existing compiler
warnings remain. The uploaded fixture SHA-256 is
`02f09e5fe88acb7b8d39aa47d358eef245091b005bdb3c4f489756de53083f23`.
