# R52: exact weighted counting of propagated rival cases

Status: completed on 2026-10-03. Limited success: the nine single-win cases are
fully counted and negligible, but the complete partition is not calculated.
Production source and defaults unchanged; no commit or push authorized.

## Question and baseline

Can Flamengo/12th in the saved group 16498 request be calculated by summing
the complete points/wins case partition, then simulating only the probability
mass whose rank depends on goal tiebreakers?

Baseline HEAD: `4e812fd506b39a504dc3f771c26e8d027cff2311`; production Rust source
revision `ac92a8ab88656c2e20a24b37c1f3c3d3dcfdb4cd`. Request:
`reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json`, SHA-256
`2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625`.
The existing certified-path diagnostic enumerates 186 complete target patterns
and 5,884 rival cases: 531 at 60 points, 2,231 at 61, 2,776 at 62, 337 at
63 points / 18 wins and 9 at 63 points / 19 wins. There are 103 unplayed
fixtures, including 10 for Flamengo.

A case assigns every ambiguous rival to strictly below the target packed
points/wins total or at least that total. It contains many shared fixture
assignments and does not itself certify the final rank. Separate target
patterns and strict-below masks form disjoint regions.

## Proposed exact calculation

Recompute each target pattern and propagate each case using the production
domains. Multiply original probabilities of forced fixtures once. Factor
disconnected residual fixture components. Memoize partial totals only for
teams that still have unfinished fixtures, checking their case bounds and
recording their rank comparison before dropping their state.

An exact weighted outcome sum provides the mass of each `(a,e)` combination,
where `a` rivals are strictly ahead in points/wins and `e` are equal. For rank 12:

- `a=11,e=0`: the rank is settled before goal tiebreakers.
- `e>0` and `a<=11<=a+e`: goal tiebreakers can affect the requested rank.
- Otherwise the requested rank is impossible for that outcome assignment.

Fixed node and memory limits leave explicitly unresolved upper mass. A cutoff
does not establish zero probability or infeasibility. Results are numerical
weighted sums within the production fixture-probability model, with floating
point error, rather than rational-arithmetic certificates.

If the exact tie-sensitive mass is material, the intended next stage samples
outcomes from that exact conditional law, then samples the original conditional
scores and verifies rank with the production sorter. Goal limits do not prove
unrestricted impossibility. Reachability witnesses never become probabilities.

## Related methods

[Dechter's bucket-elimination paper](https://ics.uci.edu/~dechter/publications/bucket-elimination.pdf)
describes exact probabilistic elimination and how graph structure controls
cost. [Chavira and Darwiche (2008)](https://escholarship.org/uc/item/7916m4fn)
describe probability as a sum of weights over satisfying assignments. R52 uses
a native fixture model with frontier memoization; it does not import a SAT
compiler or claim that their published timings apply here.

## Measurement contract

Run an all-case propagation/component census, then count the nine single-win
cases before expanding to the other categories. Use at most four workers,
serialize builds and heavy runs, and record setup/census, counting wall/CPU,
node counts, memo size, frontier width, solved cases and unresolved mass.
Compare against current Rust timing; unused modeled work is not a latency
allowance. No one-second or historical Go budget is assumed.

Validate against exhaustive small fixture graphs, including shared games,
forced-result probabilities, component convolution, live-state reduction,
all rank/tie classes and cutoff bounds. Production integration requires separate
paired full-request evidence and is not presumed from an offline success.

## Initial measurements

The unchanged production binary, seed 808/four workers, spent 1,275.19 ms in the
estimator for this request (process 1.29 s, user 2.96 s, system 0.11 s). Its rare-tail
stage took 764.83 ms. This is one diagnostic baseline run, not a paired latency
cohort. It returned 380 positive cells, 17 impossible zeros and 3 reachable zeros;
Flamengo/12th was one of the zeros.

The first counter retains every nonsingleton rival fixture and orders them by
greedy frontier width. Nine single-win cases, each limited to 100,000 nodes,
exhausted all 900,000 nodes without resolving a complete profile. They retain
74–83 variable fixtures and frontier widths 12–14. Setup/propagation took 8.83 ms,
counting 140.93 ms; process 0.50 s, CPU 0.44 s. The remaining upper mass was 1.184e-11,
far too loose to estimate the roughly 1e-25 event suggested by earlier sampling.

An all-case census took 2,365.89 ms for preparation alone. The 5,884 cases retain
24–93 variable fixtures, with frontier widths 6–14. With 20,000 nodes per case,
the counter visited 117,066,509 nodes and completed 42 cases. Preparation took
2,315.04 ms and counting 8,817.04 ms (process 11.19 s, CPU 37.53 s). Its resolved
tie-sensitive mass was about 1.0505e-40, while unresolved upper mass remained
4.887e-6. These are partial weighted sums and bounds, not an estimate of the
requested rank. The remaining mass dominates them.

Next measure a sound reduction: remove a rival from the constraint state only
when its entire residual interval satisfies its case bounds and fixes its
comparison with the target. A fixture with two removed endpoints contributes
the sum of its allowed original probabilities. A fixture with one retained
endpoint becomes a unary factor; removed teams must not connect otherwise
independent constraint components. Keep the outcome law for later score work.

## Fixed-status reduction

The reduced nine-case run, using the same 100,000-node quotas, completed 1/9
cases. It branches over 57–66 retained fixtures: 22–27 shared fixtures and 35–39
unary fixtures. Nine or ten rivals leave the constraint state. Counting falls
140.93→76.82 ms, but the unresolved upper mass is still 7.310e-12.

An all-case comparison using the same revised binary and 20,000-node quotas:

| Metric | Retain all teams | Remove fixed statuses |
|---|---:|---:|
| Preparation ms |2,710.16|2,233.47|
| Counting ms |9,186.22|7,350.34|
| Process seconds |12.03|9.70|
| CPU seconds |39.38|31.64|
| Nodes |117,066,509|107,354,619|
| Complete cases |42|672|

The reduction uses 19.7% less CPU and completes 630 additional cases. All 42
cases completed by both modes agree. Of the 672 completed reduced cases, 470
have positive probability mass. Nevertheless, the full partition remains
dominated by unknown mass. The aggregate resolved tie-sensitive mass is only
1.772e-32 and unresolved upper mass is 4.852e-6 (uncapped sum of case bounds).
No production integration is supported by this result.

Next integrate unary fixtures per rival with an exact weighted gain DP. Search
only shared active-rival fixtures. At each rival's final shared fixture, sum its
unary gain distribution inside the case interval and record below/equal/above
mass. Preserve original probabilities and carry unclosed unary mass into every
cutoff bound. This is a further offline experiment, not an additional endpoint
work allowance.

## Unary integration and tighter bounds

Unary integration completes all nine single-win cases with 221,565 nodes and
43.30 ms of counting. Preparation takes 1.75 ms. Their exact numerical
points/wins-feasible mass is **1.413430200674237e-37** (fraction), all
tie-sensitive. Five cases have zero mass and four have positive mass. Actual
Flamengo/12th probability in this category cannot exceed that number, regardless
of goal tiebreakers. Its maximum contribution is about twelve orders below the
roughly 1e-25 results from earlier importance sampling. Goal simulation here
cannot materially change that comparison and is unnecessary.

At 20,000 nodes per case across the complete partition:

| Metric | Unary integration | Integration + cache/tighter cutoff |
|---|---:|---:|
| Preparation ms |1,030.70|729.29|
| Counting ms |11,755.16|11,059.84|
| Process seconds |12.93|12.37|
| CPU seconds |46.32|44.04|
| Nodes |93,856,259|93,856,259|
| Complete cases |1,470|1,470|
| Unfinished cases |4,414|4,414|

Integration reduces branching to 1–57 shared fixtures per case, rather than
24–93 original variable fixtures. It completes more cases but makes each
closure more expensive. Node counts alone do not measure computational cost.

The cached version records 385,926 terminal-profile misses and 103,040,926 hits.
Both modes have identical resolved masses. The tighter cutoff replaces full
unary allowed mass with its exact coefficient sum over the optimistic interval
`[lower-current-max_remaining_shared, upper-current-min_remaining_shared]`.
Shared extremes are optimistic and need not occur together. Every valid
completion lies inside those intervals; unary fixture sets are independent,
so multiplying their interval masses and remaining shared allowed mass gives
a safe numerical upper bound. Closed unary factors remain at ancestors and
enter once.

Results of the tighter-cutoff run, expressed as probability fractions:

| Target added points | Settled-rank resolved mass | Tie-sensitive resolved mass | Unresolved upper mass |
|---|---:|---:|---:|
|0|3.312e-35|2.487e-31|2.460e-7|
|1|3.665e-34|8.408e-31|5.363e-7|
|2|0|1.478e-31|4.610e-7|
|3, three draws|5.202e-39|1.169e-35|9.339e-10|
|3, one win|0|5.250e-38|1.964e-15|

The last row is partial at 20,000 nodes; the separate 100,000-node run above
fully resolves that category. These upper masses are uncapped sums of case
bounds and can be tightened by the total prior of the distinct target paths.
Even that cap remains far too loose. Tie-sensitive resolved mass is not an
actual rank probability: native goal sampling/sorting is still needed within
those seasons.

A larger all-loss-only run requests 1,000,000 nodes per case, with 500,000 memo
entries per case. Memo exhaustion often stops it at 500,000 visited nodes.
It visits 226,423,433 nodes, completes 119/531 cases, and takes 41.25 s process /
162.57 s CPU (approximately 3.94 active cores). Counting takes 41,130.91 ms and
preparation 84.35 ms. Its settled-rank resolved mass is 1.981e-30 and tie-sensitive
resolved mass 5.751e-29. The remaining raw upper mass is 1.187e-6. Increasing
work substantially still fails to calculate the dominant event mass.

## Ties, correctness and recommendation

Flamengo begins with goal difference+32 in this snapshot. We cannot assume
that matching its final points/wins implies finishing above it. The proposed
conditional-score stage would sample the exact conditional outcome law, then
use `ScoreContext` and the production sorter. **That stage was not implemented
or measured in R52.** The dominant point-case mass remains unresolved, so the
experiment cannot determine the overall influence of ties or a full Flamengo/12th
probability. The fully counted single-win category can instead be discarded as
negligible using its upper bound, without a score simulation.

All 14 focused tests pass. They check original fixture weighting, forced
probability factors, restrictive final bounds, fixed rivals and target
exclusion, zero-based rank indexing, component convolution, shared/unary
integration against exhaustive enumeration, pure-unary components, cutoff
containment, packed gain gaps, caching parity, and a 1e-20 unresolved tail next
to known mass near one. Interval masses sum positive coefficients directly;
nearly-equal CDF subtraction and subtraction of known mass from an upper mass
are avoided. Independent Sol review found no remaining blocker for the bounded
measurements. No production library code changed; the standalone example tests
were the relevant gate. The final nine-case run has identical numerical output
with one and four workers, and its resolved mass agrees between the uncached
and cached versions.

The input partition is trusted to the existing complete `certified_paths`
diagnostic, tied to the recorded request/census hashes. The counter validates
target paths, priors, points/wins, stride, duplicate paths/masks, and selected
rival partitions. It does not independently certify the whole diagnostic
partition. The prototype supports standard 3/1/0, no bonus points, points/wins
as the first two keys, and the diagnostic's strict-below orientation; unsupported
inputs are rejected. Floating-point sums are numerical results in the native
fixture-probability model, not arbitrary-precision probability certificates.

**Recommendation:** retain exact unary integration as useful research, but do
not integrate full case counting into production. The current baseline computes
the complete matrix in about 1.28 s for this diagnostic run; R52 spends 12–41 s on
one cell without calculating its total. No additional production latency
allowance is assumed, and no candidate full-request endpoint cohort was run.
Production latency/coverage are unchanged because the experiment is isolated.

The most useful next experiment is conditional Monte Carlo over shared fixtures
using the exact unary weights, with correct proposal likelihood ratios and
native score sampling for ties. That could exploit this reduction without
exhaustively counting a large frontier. It remains untested; it must be compared
with the current native estimator and fresh checks before integration.

## Reproduction and retained evidence

Artifacts are in `2026-10-02-exact-rank-cases/` beside this report: the final
offline example patch, versioned sources, validation/timing output and request /
census provenance. The raw archive contains 48 evidence files plus its checksum
manifest, including the original request and final invariance checks. Archive
SHA-256: `4fb82b2ffe22963fc1a1177f63dcddaf88fb4c0fd32e3589578b8758b7d9047a`.
Use a fresh scratch directory; do not modify the production
checkout. Serialize heavy commands and use four workers / four Cargo jobs.

```sh
work_dir=$(mktemp -d /private/tmp/golaberto-r52-repro.XXXXXX)
artifact_dir="$PWD/experiments/rare_positions/2026-10-02-exact-rank-cases"
request_file="$PWD/experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json"
git archive 4e812fd506b39a504dc3f771c26e8d027cff2311 odds-rust stats/core AGENTS.md | tar -x -C "$work_dir"
git -C "$work_dir" apply "$artifact_dir/v4candidate.patch"
cargo test --locked -j4 --manifest-path "$work_dir/odds-rust/Cargo.toml" --target-dir "$work_dir/target" --example exact_rank_cases -- --test-threads=1
cargo build --release --locked -j4 --manifest-path "$work_dir/odds-rust/Cargo.toml" --target-dir "$work_dir/target" --example certified_paths --example exact_rank_cases
"$work_dir/target/release/examples/certified_paths" "$request_file" 17 12 10000 > "$work_dir/cases.json"
/usr/bin/time -p "$work_dir/target/release/examples/exact_rank_cases" "$request_file" "$work_dir/cases.json" "$work_dir/result.json" --category all --reduction fixed --unary integrate --cutoff-bound unary --nodes-per-case 20000 --nodes-total 117680000 --memo-entries 100000 --workers 4
```

For the fully counted single-win run, use `--category 3win`, 100,000 nodes per
case and 900,000 total. For the deep all-loss run use `--category 0`, 1,000,000 /
531,000,000 nodes and 500,000 memo entries. Rebuild the retained v3 source for
its original uncached deep timing. Historical v1 source was not retained;
the paired v2 `--reduction none|fixed` comparison is reproducible from its
retained source. Timing measurements are local and include no production-Xeon
claim. Process RSS could not be read under the sandbox; CPU and wall time were
available.
