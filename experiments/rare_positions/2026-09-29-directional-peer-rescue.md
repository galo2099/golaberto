# Directional peer rescue and request-seed reuse (2026-09-29)

## Reported zero

For group 16498, a run logged scout seed `1790726765771387608` and scout
stream seed `1846960265420860113`. Bahia (team 74) had a 19th-place estimate
but Cruzeiro (team 15) appeared as zero. A fresh Rails export of the local
group request matched the saved reference in every field affecting odds:
380 fixtures, 103 unplayed, 20 teams, powers, results, and standings rules.
Only three `updated_at` timestamps differed.

The Go service listening on local port 6577 had started at 09:02, before the
16:25 commit that added the extreme-tilt cross-check. A fresh build with
`RARE_POSITION_RANDOM_SEED=1790726765771387608` returned Cruzeiro 19th at
`1.389e-12` and Bahia 19th at `3.030e-12` as probability fractions; the
returned `team_odds` matrix contained those same values multiplied by 100.

The old scout log did **not** uniquely identify the rare-search run. With an
unset seed, `calculate_odds` drew a time seed for the scout, then the rare
search drew another time seed later. The second seed was not logged in the
matched-pool branch. The code now passes one request seed through both stages
and logs its separately derived matched-pool stream. Future runs can be
replayed by setting `RARE_POSITION_RANDOM_SEED` to the logged request seed.

## Search hypothesis

When a reachable or undecided zero lies below a team's current rank, a
currently higher peer with a rare nonzero estimate at the same target rank
is evidence that the zero merits a target-specific search. For a target rank
above the team's current rank, use a currently lower peer. The peer's number
never becomes the target's probability. It only selects a candidate.

The implementation considers peers whose target-rank probability is below
`1e-6`, screens target point events below `1e-4`, and selects at most one cell
per request by existing zero-hit upper bound and then point-event mass. A
5,000-draw gentle pilot must hit the rank at least once before a fresh
50,000-draw confirmation. The confirmation uses rank tilt 3, point tilt
`+0.5` for a climb or `-0.5` for a drop, importance weights, and the existing
validity gate. The pilot never contributes to the reported estimate. Set
`RARE_POSITION_DIRECTIONAL_PEER_RESCUE=0` to disable the pass.

## Paired full-request results

Each baseline and rescue run used the same request and configured seed. For
even seeds the rescue arm ran first; for odd seeds the baseline ran first.
All runs used 20,000 initial MC seasons, the 100,000-season matched point
pool, and `GOMAXPROCS=4`. The 15 group 16498 seeds were 801–812, 817, 900,
and 911. The other four inputs used seeds 801, 802, 803, 808, and 809.

| Request | Seeds | Zero→nonzero cell-runs | Nonzero→zero | Median paired time change |
| --- | ---: | ---: | ---: | ---: |
| Group 16498 | 15 | 10 | 0 | +166 ms |
| Group 16653, current | 5 | 0 | 0 | +13 ms |
| Group 16653, earlier | 5 | 0 | 0 | +9 ms |
| Group 16982 | 5 | 0 | 0 | -4 ms |
| Group 16983 | 5 | 0 | 0 | -7 ms |

In group 16498, Cruzeiro / 19th was zero while Bahia / 19th was nonzero in
nine runs. The rescue produced an accepted Cruzeiro estimate in all nine,
from `7.36e-13` to `2.57e-12`. It also estimated Cruzeiro / 20th in seed 808
at `1.52e-18`. No previously nonzero cell was lost. The additional time was
roughly 160–210 ms when the 50,000-draw confirmation ran. The earlier group
16653 still had one five-seed run that spent the full confirmation budget
without an accepted estimate; its median overhead fell from roughly 126 ms
without the pilot to 9 ms with the pilot. Negative time differences in the
table are measurement noise, not a speedup from extra work.

The results support this narrow allocation rule for the tested requests. It
does not imply monotonic probability across current standings: Bahia's own
19th-place chance is still about 2–3 times Cruzeiro's in high-compute runs.
The rescue's additional accepted estimates have the usual weighted-sampler
uncertainty; they are order-of-magnitude estimates, not exact probabilities.

The saved group 16498 regression checks seed 804, the new nonzero cell, and
retention of all baseline nonzero cells. The seed-reuse regression checks that
an unset request seed is identical in the scout and matched-pool log lines.
`go test ./go` passes.
