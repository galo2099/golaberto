# Generic follow-ups after the fresh 100× campaign

October 5, 2026. Written before screening these policies. Production settings
stay unchanged. Use the previously frozen branch example and exact before/after
snapshots of group 16498. Jobs run serially, four workers; no branch omission,
weight changes, gate changes or team-specific forced selection.

## Screening

Construction seed 808, pilot seed 60031, final seeds 60013/60017/60029. Existing
primary runs are matched controls and add no new cost. Ten new panels:

| Cell | Policy change | Final draws per stream | Pilot draws | Guidance | Allocation |
| --- | --- | ---: | ---: | --- | --- |
| Palmeiras15 | Increase pilot only, keep three messages | 600,000 | 5,000 | bounds | trained |
| Palmeiras16 | Equal allocation | 2,500,000 | 5,000 | adaptive | equal |
| Palmeiras16 | Increase pilot only | 2,500,000 | 50,000 | adaptive | trained |
| Palmeiras16 | Always use bounds guidance | 2,500,000 | 5,000 | bounds | trained |
| Palmeiras16 | Native goal scores | 2,500,000 | 5,000 | adaptive | trained |

Run each policy on both snapshots. P15 keeps complete tree cap 256, generic
rank priority, production-profile pilot, floor 2 and three message slots.
P16 keeps secondary refinement and floor 200. Goals remain tilted except in
the native-score control. Equal allocation still uses pilots to choose guidance.

Increasing pilots changes fitting/selection and allocation together. Native
scores also change pilot ESS and thus guidance/allocation; it measures the
whole score-policy effect. Always-bounds changes both pilot scores used for
allocation and final guidance. No control isolates a single weight factor.

## Holdout

Freeze policy selection before holdouts. Final seeds 60101/60103/60107, fresh
pilot 60109, construction 808, same final sample counts on both snapshots.
This adds one independent training realization, not three independent fits.

For P15 run all three candidate training policies: 500 pilots/three messages,
5,000 pilots/three messages, and 500 pilots/four messages. This directly checks
whether the previous four-slot result and additional training generalize.

For P16 select at most one of the four screening variants. Rank by passing
pair count across both fixtures, then minimum aggregate ESS across all MAIN
and CHECK streams, then smaller worst aggregate maximum observation share.
These are exploratory selection criteria, not calibrated error guarantees.
Run the selected variant and the original adaptive/trained/5,000 policy on
both fixtures. If no variant improves on the original, stop screening and
report that result instead of searching more arms or seeds.

## Analysis and stop

Keep every pair, including failures; report probabilities, MAIN/CHECK
disagreement, aggregate and branch ESS, weight shares, hit counts, semantic
families, guidance choices, allocations, pilot/message/final logical charges,
and wall/CPU. New runs' costs exclude reused controls. Bounds setup and
secondary setup can have unavailable logical accounting; do not turn null into
zero or infer production affordability from final-draw equality.

Stop after these screening and holdout panels. Do not increase samples or
replace failing seeds in response to observations. These are focused offline
diagnostics, not a whole-endpoint capacity test or exact reference solution.
