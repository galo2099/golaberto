# Independent rebuild contract review

Pre-implementation review: the statistical architecture is viable; this is not approval of unwritten implementations. No source edits or delegation by this reviewer.

The most important soundness properties in REBUILD_SPEC are correct: final exact above-set events are disjoint even though weak points/wins conditioning supersets overlap; unknown preparation retains full support; the certified continuation cap uses actual categorical probability intervals; entering an unprepared state permanently selects fallback; changed proposals require fresh observations; old absolute event intervals may be intersected; and confidence budgets cover adaptive region, epoch and look choices.

## Conditions requiring implementation checks

1. Use a single declared probability model throughout. The specified accepted-grid law reproduces independent Poisson::sample draws and rejection conditional on WDL. Surviving Scores::new instead constructs normalized conditional score CDFs from masses(); it is not automatically identical on the accepted finite RNG grid. Never combine law.rs outcome masses with model.fixture.prob or Scores::sample unless equivalence or a valid likelihood correction is proved. A coherent Poisson-based reconstruction is acceptable, but must be labeled rather than claimed identical to the lost estimator.

2. The target fixed-root probability and each fixed fixture probability enter exactly once. Conditional joint DP must not multiply them again through both initial root mass and fixture traversal. Fixture outcomes forbidden by a mask have exact zero proposal probability; every positive target-law outcome permitted by a fallback domain needs positive tickets. Zero potential can remove an outcome only after exact impossibility or a certified zero mass.

3. Partial DP fallback potentials and continuation caps must describe the sampler actually used. Once fallback starts it cannot re-enter a memoized conditional table. The fallback cap must include all remaining native-mask importance factors, while unselected fixtures are independently accounted for. Midpoints may choose proposals but never certify event mass or upper support. Ticket integer overflow and vanishing probability lower bounds must reject preparation, not truncate support.

4. Score integration must preserve shared fixture dependence. Private fixture sets are disjoint by construction; fixtures joining two target-PW-tied teams are sampled once. Strict GD comparisons bound the exact event below, inclusive comparisons bound it above; equality remains for GF and later keys. If integration is declined by a cache cap, fresh native conditional sampling is safe. Never suppress a large observed weight by clipping to one without a proven support cap.

5. Fixed sample requests commit all n observations or none. Errors or overweight values invalidate the whole uncommitted stage; dropping, replacing or retrying only inconvenient observations biases the sample. Adaptive scheduling is safe when the look/epoch allocation is fixed and summable and each new proposal uses fresh future draws. Per-node IDs must not be recycled.

6. One-sided Clopper–Pearson is an exact statistical construction, but scipy.special inverse beta functions are floating-point numerical implementations. nextafter alone is not a proof that library approximation error is enclosed. Use an explicitly stated numerical assumption or validated conservative guard rather than claiming formal numerical exactness. Upper/lower beta-tail calls must avoid subtraction cancellation, handle zero/all hits, and allocate alpha across every threshold and both tails. Omitted threshold slabs must contribute to the upper bound.

7. Analytic upper bounds and sum/aggregation require positive outward arithmetic, preserving exact zero and guarding underflow. Lower and upper streams use separate confidence events. A confidence conflict must preserve a conservative earlier interval or revert analytically; silently enforcing L<=U would conceal invalid evidence.

8. A shared 180-second wall allowance includes initial region enumeration, planning, sampling and response construction. Native/solver checks may be soft; report observed overruns. Keep all unsampled regions and any quota-truncated uncovered mass. Root enumeration must not treat target-limits unknown as impossible.

## Confidence and event contracts

The documented alpha family divided by 6840 and the original positive candidate-slot denominator, then summable region/epoch/look allocations and a per-look union bound across thresholds/tails, is conservative if implemented consistently. Additional proposal epochs need not change the original candidate count. Intersect retained absolute confidence intervals for the same event; sum only disjoint event partitions. The score bracket can have irreducible equality width and should not be described entirely as sampling variance.

A successful implementation should demonstrate tiny-model exhaustive identities for complete and partial DP, actual ticket selection, fixed-root accounting, mixed shared/private score kernels, and fresh epoch confidence state. Until source and test review, these are requirements rather than verified properties.
