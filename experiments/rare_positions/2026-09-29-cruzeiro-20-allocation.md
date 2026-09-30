# Cruzeiro / 20th: rarity and peer-search allocation (2026-09-29)

## Reproduction

Group 16498, request seed `1790746108560577000`, scout stream
`-8425532767934098155`. Saved request SHA-256:
`44eabb47ee55477c77025c3de4e839c6ea913b8569dd29694d7178bdb8bcdc89`.
Replay uses four cores, the 20,000-season scout, 100,000-season matched pool,
and production defaults with matched-point pooling enabled.

| Cell | Production probability fraction |
|---|---:|
| Cruzeiro / 19th | `1.0759e-12` |
| Cruzeiro / 20th | `0` (undecided) |
| Bahia / 19th | `3.1765e-12` |
| Bahia / 20th | `4.5290e-19` |

The peer pass logs two eligible cells and selects Cruzeiro / 19th. Its pilot
has eight hits and its confirmation has 47 hits, ESS 8.59, relative error
0.341 and batch gap 0.794. The confirmation passes, so no retry runs.
The peer pass currently confirms at most one selected target per request.
Cruzeiro / 20th is not given that confirmation slot.

## Why 20th is rarer than 19th

Cruzeiro starts on 45 points. The target-only conditioning event for 19th
includes final totals 45–53, with mass `0.0343097`. For 20th it includes
45–51, with mass `0.00910271`. These are necessary point-event probabilities,
not the probabilities of the finishing positions.

Chapecoense has 18 points and 11 remaining fixtures: its ceiling is 51.
Remo has 23 points and 10 remaining fixtures: its ceiling is 53. A 19th-place
finish can leave Chapecoense below Cruzeiro; a 20th-place finish requires
Chapecoense and all other rivals to finish ahead on points or tiebreakers.
Even if Cruzeiro loses every game and stays on 45 points, Chapecoense needs
at least 27 additional points. Those simultaneous rival results make the
20th-place probability much smaller.

Simpler necessary constraints can make a reachability witness easier to
construct. They do not make the required fixture combinations more probable
or guarantee a low-variance probability estimate.

## Diagnostic targeted searches

Using the existing sampler, target-only event, rank tilt 3 and point tilt
`-0.5`, derive the stream with label `directional-peer-rescue-15-19` from the
reported request seed. The label's rank is zero-based. No production code
changes are needed to run this diagnostic search.

| Proposal | Draws | Probability fraction | Hits | ESS | Relative error | Batch gap | Passes existing gate |
|---|---:|---:|---:|---:|---:|---:|---|
| Existing lookahead | 15,000 | `1.6601e-18` | 37 | 6.44 | 0.394 | 1.361 | No |
| Existing lookahead | 45,000 | `1.5045e-18` | 80 | 13.12 | 0.276 | 0.214 | Yes |
| Existing lookahead | 50,000 | `1.3561e-18` | 89 | 13.16 | 0.276 | 0.212 | Yes |
| Propagated domains | 45,000 | `1.5665e-18` | 83 | 12.89 | 0.278 | 0.770 | Yes |
| Propagated domains | 50,000 | `1.5152e-18` | 90 | 14.53 | 0.262 | 0.778 | Yes |

These diagnostic proposals use the same seed but different proposal paths;
they are not independent confirmations. They support an order-of-magnitude
estimate around `1e-18`, roughly 800,000 times smaller than the production
19th-place estimate. They also prove the cell reachable.

## Next allocation experiment

When the first peer confirmation succeeds, use the unused 50,000-draw retry
allowance for a second eligible peer target: 5,000 pilot draws and 45,000
fresh confirmation draws. When the first confirmation needs its retry, retain
the existing retry allocation. This would preserve the pass's maximum draw
allowance while using it more frequently.

This is a proposed experiment, not an adopted production change. It needs a
paired full-request evaluation: the fresh pilot has a separate seed, other
eligible targets may differ, and spending more of the existing maximum budget
can increase average latency. A missing estimate remains possible after finite
search even when a position is reachable.

Raw replay matrices, the temporary diagnostic harness and its log are ignored
under `experiments/rare_positions/local/2026-09-29-cruzeiro-20/`.
