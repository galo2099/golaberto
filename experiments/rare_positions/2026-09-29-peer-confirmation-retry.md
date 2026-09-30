# Cruzeiro / 19th: retry a noisy peer confirmation (2026-09-29)

## Reproduction and cause

Reported request seed: `1790744713633556000`, group 16498. The replay emits
the same scout stream seed, `-6282549715285062840`. Both compacted and original
fixture loops reproduce Bahia / 19th at `4.7976e-12` and Cruzeiro / 19th at zero.
All probabilities in this report are fractions, not percentages.

A fresh Rails export from the local database matches the saved request's
fixture order, team order, results, powers and standings rules. Only timestamps
and the unused `odds_progress` field differ. There are 380 fixtures, 103 unplayed.
Saved input SHA-256:
`44eabb47ee55477c77025c3de4e839c6ea913b8569dd29694d7178bdb8bcdc89`.

The directional peer rescue correctly selected Cruzeiro / 19th. Its pilot
found nine finishes there. Its fresh 50,000-draw confirmation found 38,
estimating `1.1357e-12`. The evidence passed every acceptance condition except
the two-batch stability check:

| Metric | First confirmation | Required |
|---|---:|---:|
| Hits | 38 | At least 30 |
| ESS | 8.54 | At least 8 |
| Relative standard error | 0.342 | At most 0.35 |
| Maximum event weight share | 0.197 | At most 0.25 |
| Batch gap | **1.257** | At most 1 |

The two half-sample estimates differ by roughly 4.38 times. The code rejected
the estimate and also left the reachability label `undecided`, despite finding
valid finishes in the pilot and confirmation. The displayed zero was a rejected
probability estimate, not evidence of impossibility.

## Adopted fix

1. Preserve the pilot's verified reachability independently of whether a
   probability estimate passes validation. An undecided cell becomes
   `reachable_by_construction` as soon as the peer pilot hits it.
2. Retry only a confirmation that fails the batch check while passing every
   other existing acceptance condition. The pass already selects at most one
   target per request, so there is at most one retry.
3. Run a fresh 50,000-draw confirmation with the same proposal: rank tilt 3,
   direction-selected point tilt, and the existing wins-aware rank guidance.
   Use a separately derived seed; neither the pilot nor the failed confirmation
   contributes to the reported probability.
4. Require the retry to pass every unchanged acceptance gate and agree within
   the existing 30-fold cross-check tolerance with the informative first
   confirmation. Otherwise retain zero and the known reachability proof.
5. Log the estimated probability, relative error, largest weight share, batch
   gap, first batch gap and retry draws so a rejected result is diagnosable.

The retry is generic across groups, teams, ranks and directions. It is enabled
by default. `RARE_POSITION_DIRECTIONAL_PEER_RETRY=0` disables it; existing
production flags remain sufficient. Rebuild and restart the Go service.

## Alternatives tested on the reported cell

- Points-only proposal at rank tilt 3: 1 hit in 15,000 draws and 2 hits in
  50,000. Estimates failed the unchanged gate. Not adopted.
- Fresh wins-aware 15,000 draws at tilts 2, 3, 4 and 6: all failed a gate.
  Increasing tilt did not reliably increase effective evidence. Not adopted.
- Recomputing the original stream at 75,000 and 100,000 draws passed, with
  estimates `1.0040e-12` and `9.4063e-13`. This would repeat the original work
  without adding continuation support to the sampler. Not adopted.
- Fresh wins-aware 50,000 draws at tilt 3: **61 hits, ESS 13.63, relative
  error 0.271, max share 0.108, batch gap 0.635**, probability `1.3506e-12`.
  Adopted as the narrowly triggered independent confirmation.

## Full-request coverage and cost

Six seeds on each of five reference inputs:
`801,804,808,817,911,1790744713633556000`. Inputs are current 16498,
current and earlier 16653, current 16982, and reference 16983. Each pair uses
four cores, a 20,000-season scout, the 100,000-season matched pool, and the
existing production searches. Only the retry flag differs; arm order alternates.

Across **30 paired requests**, the reported 16498 seed gained Cruzeiro / 19th.
There were **zero lost nonzero cell-runs**. The other 29 requests used exactly
the same simulation work with the retry enabled and disabled.

The affected request changed from **1.294 s to 1.478 s: +185 ms / +14.3%**.
Total modeled work rose from 74,907,000 to 81,180,000 units: 6,150,000 for
the fresh confirmation plus 123,000 from a newly available downstream domain
pilot slot. No additional estimates were found in the other reference pairs.
Their timing differences are measurement noise, not speed improvements.

Final reported-seed estimates after matrix reconciliation:

| Team / rank | Probability fraction |
|---|---:|
| Cruzeiro 15 / 19 | `1.3506e-12` |
| Bahia 74 / 19 | `4.7976e-12` |

The fresh database export also reproduces these estimates. These are weighted
order-of-magnitude estimates; finding a witness and passing the sample checks
does not provide an exhaustive probability reference or guarantee every seed
will produce an accepted estimate.

## Verification and saved artifacts

- Full Go suite passed.
- Regression test replays the reported seed with retry disabled/enabled,
  checks that rejected estimates retain reachability, verifies the recovered
  estimate and retains every previously nonzero cell.
- Eligibility test rejects retries for low hits, low ESS, excess relative
  error, excess weight concentration, and already-valid batch checks.
- Targeted race check includes the reported-seed regression and eligibility
  and direction checks.
- Raw matrices are ignored under
  `experiments/rare_positions/local/2026-09-29-peer-retry/full/`.
  The fresh database replay is in `fresh-db/`.

Run the regression:

```sh
GOCACHE=/private/tmp/golaberto-go-cache GOMAXPROCS=4 \
RARE_POSITION_BENCHMARK_GROUP_JSON=/private/tmp/golaberto-group-16498-current.json \
go test ./go -run '^TestDirectionalPeerRetryReportedSeed$' -count=1 -v
```

For paired experiments, `TestDirectionalPeerRetryFullRequestExperiment` reads
`RARE_POSITION_PEER_RETRY_EXPERIMENT_REQUESTS` (comma-separated request files),
`RARE_POSITION_PEER_RETRY_EXPERIMENT_SEEDS` (comma-separated seeds), and
`RARE_POSITION_PEER_RETRY_EXPERIMENT_OUTPUT` (absolute output directory).
