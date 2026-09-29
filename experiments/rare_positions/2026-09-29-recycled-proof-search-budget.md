# Reusing work saved by early impossibility proofs

## Question

The joint minimum and maximum points proofs now run before conditional searches. Could a small part of the saved search budget turn an otherwise zero, reachable position into a usable estimate without materially increasing request time?

## Experiment

An early proof removes a zero cell before its direct conditional search. That search normally begins with at least 2,000 draws. The new path grants 1,000 confirmation draws per early proof, capped at 4,000 per request. It chooses the highest ranked point-tilt pilot candidate that missed the regular final slots and remains zero. The pilot must have observed the rank, so it already supplies a reachability witness. A fresh confirmation uses the pilot-selected proposal and the existing weighted-estimate acceptance checks; failed confirmations leave the cell at zero. Regular final slots and gap rescues run first. `RARE_POSITION_RECYCLE_PROOF_WORK=0` disables this path.

Before implementing it, I tried adding 1,000 or 3,000 pilot draws for each of the proved-reachable zeros team 318/3rd in group 16498 and teams 125/10th and 95/3rd in group 16653. None of the three tilt configurations observed the target rank. Spending the saved work on those cells did not help.

## Paired fixed-seed results

Both sides used the same request JSON, seed, four cores, 20,000 scout draws, 100,000 matched-pool draws, and the current production estimator. The only changed input was the recycle flag. Probabilities below are raw fractions, not percentages.

| Request | Seeds | New nonzero cell-runs | Lost nonzero cell-runs | Added work when used |
| --- | --- | --- | --- | --- |
| Group 16653, live September 28 snapshot | 801, 808, 811, 817, 900 | 4 | 0 | 210,000 work units (2,000 draws × 105 units) |
| Group 16498, September 24 snapshot | 801, 808, 811, 817, 900 | 0 | 0 | 0 or 246,000 units (2,000 draws × 123 units) |
| Group 16982, September 24 snapshot | 801, 817, 900 | 0 | 0 | 0 |
| Group 16653, older September 24 snapshot | 808, 811, 817 | 0 | 0 | 0 or 110,000 units |

The four group 16653 gains were team 12/17th at seeds 808 and 900 (3.48e-10 and 3.87e-10), and team 279/18th at seeds 801 and 817 (7.42e-13 and 7.13e-13). Seed 811 spent the confirmation without a new accepted estimate. The matching values across seeds are a useful stability check, though they do not establish precision at those tiny probabilities.

The current group 16653's two early cap proofs had previously saved about 3,400 plain-season-equivalent draws. The added 2,000 draws use less work than that measured saving. Group 16498's early floor proofs had saved about 4,400 draws at seed 808; at that seed, no unused pilot-positive candidate needed confirmation, so no work was added.

## Runtime

`BenchmarkMatchedPointPoolFullRequest`, `GOMAXPROCS=4`, five requests per measurement and three measurements per variant:

| Request | Recycle off, median | Recycle on, median | Change |
| --- | ---: | ---: | ---: |
| Group 16653 live | 0.932 s | 0.945 s | +1.4% |
| Group 16498 | 0.917 s | 0.918 s | +0.2% |

Both medians remained below one second. These timings include the full request and are noisy at the millisecond level. The search work accounting is more stable than the wall-time difference.

## Decision and limits

Keep the recycled confirmation enabled by default because it recovers rare cells on the live group 16653 snapshot with no positive-to-zero regression across the paired seeds and a small time cost. The proof count provides a conservative draw allowance, not an exact per-request runtime guarantee. Some seeds and groups spend it without gaining a cell. The estimate is reported only after a fresh held-out confirmation passes the same ESS, relative error, weight concentration, and batch agreement gates as other point-tilt estimates.

`go test ./...` passed with no external fixture and with the live group 16653 request. Several optional assertions against the older group 16653 fixture still fail on current estimator behavior; for example, `TestConditionedZeroRealGroup` fails with recycling disabled as well. The paired snapshots above compare that fixture with and without this change directly.
