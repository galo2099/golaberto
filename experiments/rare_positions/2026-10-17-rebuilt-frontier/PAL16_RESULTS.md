# Palmeiras finishing16th: unchanged-estimator check

The requested historical case is the279-played-game request at
`experiments/rare_positions/reference/2026-10-04-two-results/inputs/group-16498-two-results.json`.
Its canonical JSON SHA256 begins with the retained case identifier `40b8ab70b9fc`:
`40b8ab70b9fc464d154e5123d527a0b62f31d4a47b8e71a464c8b51c1dfe4544`.
Raw file SHA256:
`cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee`.
Palmeiras starts57 points/16 wins with10 matches remaining. Team79 starts29
points/7 wins with10 remaining fixtures, matching the historical branch analysis.

## Result

Same frozen estimator as the successful Palmeiras12th run: one native worker,
shared180-second wall budget, one second output reserve, full-budget sampling,
99% simultaneous confidence, depth9, initial k7/1000 states, maximum k11 and
250000 states,128 discovery draws and4000000 total draws per cohort. Setup,
sampling and confidence work are charged. No HiGHS prescreen is used.

| Case | External wall | Draws | Lower | Upper | U/L | Qualified |
|---|---:|---:|---:|---:|---:|---|
| Historical Palmeiras16th request |179.241s|7165920|4.1384946899147247e-41|1.2085487829286124e-36|29202.6|No|
| Palmeiras12th reference run |179.152s|1768096|6.147750115263105e-29|3.1342415010044333e-28|5.098|Yes|

There is no qualified order-of-magnitude estimate for Palmeiras16th. The
separate empirical lower/upper score-stream means were
`[1.3755662264228564e-40,3.6592966635647608e-40]`; they are not a confidence
interval or a single statistically qualified estimate.

Integer bounds retain16 cohorts, all with target final57 points/16 wins
(ten target losses). One cohort was proved impossible by the end;15 remained
live. Fourteen regime proposals were initially installed, one proposal setup
was rejected and one cohort became a certified zero. Later stronger joint-only
plans replaced the installed regimes. These are regenerated cohorts, so their
indices/counts must not be equated with the old saved108-cohort representation.

No sample errors, confidence conflicts or uncovered mass were reported. Frozen
source, input and binary hashes remained unchanged. Initial engine setup took
0.016 seconds. GD analysis took0.257 seconds, regime setup0.194 seconds, and
reported joint/regime preparation totaled5.444 seconds. Native sampling took
143.374 seconds; confidence calculations and orchestration also consume budget.

## Concentrated unresolved contribution

Current cohort5 supplies98.795% of the total upper bound:
`[2.4669719412757e-44,1.1939902422279762e-36]`.
It has required-above team IDs
`[5,4,74,7,225,24,10,15,17,8,584,6,14,18,67]`.
This numeric cohort index is local to the reconstructed partition.

It reached the4000000-draw cap and spent70.598 seconds sampling. Its final
proposal epoch had3991872 draws with32 nonzero upper-score contributions.
The builder did not complete a supported conditional regime for this cohort.
Joint-only refinements at k10 and k11 remained incomplete at the250000-state
limit; the retained complete plan selected nine rivals and had cap
`1.0140615550832632e-31`. These are the concrete setup/variance obstacles;
reaching a positive score contribution does not by itself tighten the unseen
contribution enough to satisfy the requested precision.

## Compact conditional WDL follow-up

The shared suffix forest now completes team 584's 156-assignment conditional
proposal (90 tied and 66 strict assignments) without a full joint table per
assignment. An isolated nine-rival check reduces the certified contribution
cap from `1.0140615550832632e-31` to `1.335714790055119e-32`, a factor 7.59.

The unchanged controller's shared 180-second run nevertheless remains
unqualified: interval `[4.2494598410268494e-41,1.184683963327682e-36]`, ratio
27878.5, 7297920 draws and 179.249 seconds external wall time. It replaces the
initial compact proposal with a stronger joint-only plan and never rebuilds
GD conditioning after that growth. The fixed-plan gain therefore does not
transfer to the final proposal. See COMPACT_PARTITION_RESULTS.md for the
construction, statistical review, tests and complete comparison.

## Initial snapshot mismatch

An initial run used the285-played-game uploaded snapshot, which also has
Palmeiras57/16 but has team79 at30 points with nine games left. It is a
different case. That run produced `[5.9453740344990614e-43,
8.844105786639036e-42]`, ratio14.876, in179.185 seconds and486128 draws.
Its files were renamed to `regime-cost-pal16-285played-180s*` after execution;
the stored command retains the original output filename. Those observations
were not reused in the corrected historical-case run.

## Provenance

Native binary SHA256:
`61ec2e29f5c28d035251e9e7a2050dc8b2649a760886ae89fe128d4eb99d0b13`.
Source aggregate SHA256:
`62383b24cd078c08d5a08bb61f7a3f1c832dde983329f0c192a89a0edf3d462a`.
No source changes were made for this check. The existing26 Rust and20 Python
tests remain the validation checkpoint. Raw receipts/timings stay local;
this aggregate report and artifact identities are included in the branch backup.
