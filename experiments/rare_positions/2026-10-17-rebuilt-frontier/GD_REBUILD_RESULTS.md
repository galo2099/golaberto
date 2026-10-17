# Rebuild GD proposals after joint conditioning grows

The experimental controller now attempts GD conditioning once after each
accepted, completed, positive joint proposal in the focused sampling loop.
Previously GD setup ran only during the initial stage, so later joint plans
could replace a GD proposal without rebuilding it against the stronger joint
constraints. The native estimator and production endpoint are unchanged.

## Controller transition

After accepting a new joint plan, the controller installs its metadata and
resets statistics to its native proposal epoch. Before sampling it requests a
GD rebuild, retaining the new joint proposal if the rebuild is rejected or
fails. An accepted GD proposal installs its own fresh epoch and statistics.
Previously established absolute confidence intervals, total draw counts,
analytic upper bounds and cumulative sampling time remain intact. No samples
from different proposal laws are pooled into an epoch's threshold counts.

Rebuilds are generic across cohorts. Rejected, unfinished and zero joint
proposals do not trigger them. Each eligible parent epoch receives at most one
rebuild decision, including recorded skips when resources are unavailable.
The initial GD stage remains available. `--no-regimes` disables both stages;
`--no-regime-rebuilds` preserves initial GD setup while disabling the new
after-growth attempts for control runs.

Each request is bounded by the remaining shared wall time before the cleanup
reserve, the user setup allowance, and 1000 ms. Retained states must fit the
user limit, the 250000-state cohort policy and the shared 4000000-state limit.
No rebuild is requested when the state allowance leaves no room beyond the
new joint parent. Accepted and rejected setup costs are recorded; errors
without a native timing receipt are charged their observed elapsed time.
Protocol timeouts stop further estimation calls. A new zero proof stops
sampling that cohort, with its proposal epoch updated before returning.

## Validation

Thirty Python tests and `py_compile` pass, with independent Sol review. New
tests exercise the real main loop through a fake engine protocol, checking
joint growth followed by GD rebuilding and a sample using the new epoch/seed.
They also cover joint and GD zero proofs without further sampling, fatal GD
protocol timeouts, rejected rebuild preservation, retained absolute bounds,
one attempt per parent epoch, deadline/state skips and setup accounting.
The native code and executable remain at the prior 31-test validation checkpoint.
The existing SciPy beta-inverse numerical assumption is unchanged.

## Same shared 180-second budget

Historical Palmeiras16th request, canonical identifier `40b8ab70b9fc`, with
279 played games. The native executable, request, seed, one-worker policy,
depth9, initial seven-rival/1000-state setup, maximum eleven rivals/250000
states, 128 discovery draws, 4000000 total draws per cohort, and one-second
cleanup reserve match the previous compact-partition run. Both runs continue
sampling for the full budget. There is no native build during measurement.

| Controller | External wall | Draws | Lower | Upper | U/L | Qualified |
|---|---:|---:|---:|---:|---:|---|
| Initial GD setup only |179.249s|7297920|4.2494598410268494e-41|1.184683963327682e-36|27878.5|No|
| Rebuild GD after joint growth |179.180s|4789920|3.105431383236009e-41|1.719720850889973e-37|5537.8|No|

The global upper endpoint falls by a factor6.89 and the interval ratio improves
by a factor5.03. Neither run qualifies for the requested simultaneous99%
order-of-magnitude interval. No qualified point estimate is reported.
These are single-seed adaptive runs on one saved request, not a performance
guarantee across all saved fixtures.

The new controller makes26 after-growth rebuild attempts and accepts25. Fourteen
of the15 live cohorts retain GD proposals at the end. Cohort9 keeps its joint
proposal because the attempted GD candidate has a worse cap. One cohort is
proved zero; all15 live cohorts receive samples. No sample errors, confidence
conflicts or uncovered mass are reported, and all measured hashes remain
unchanged.

### Cohort 5 now retains the stronger joint GD proposal

The eight-rival rebuild is accepted with cap `4.992168707386364e-31` after
183.4 ms setup. The nine-rival rebuild is accepted with cap
`1.335714790055119e-32` after320.1 ms setup. It retains the same nine jointly
conditioned rivals plus team584's156-assignment GD mixture (90 tied and66
strict assignments), at166182 retained states. The final proposal is regime
epoch6, rather than a joint-only proposal.

Cohort5 still reaches4000000 total draws. Its final epoch has3995872 draws and
34 nonzero upper-score contributions; those are weighted contributions, not
ordinary event hits. Its interval is
`[3.834775699752718e-45,1.5645417326444608e-37]`, and it supplies90.98% of the
global upper endpoint. That upper endpoint is about7.48 times smaller than
before rebuilding, close to the7.59-fold reduction in its contribution cap.
The nonzero count changes little from the control's33: the main observed gain
is the tighter certified cap, rather than substantially more nonzero draws.

### Cost

After-growth GD setup costs8014.9 ms across all cohorts; the two accepted
cohort5 rebuilds account for503.5 ms. Total reported GD setup is12109.1 ms,
and reported joint/regime preparation totals15362.1 ms. Native sampling takes
143517.4 ms. Cohort5 sampling increases from66.187 seconds to115.176 seconds
for the same4000000 total draws. GD conditioning reduces the cap while making
each draw more expensive, and other cohorts receive fewer draws within the
same shared wall allowance. The comparison includes those costs.

Measured source aggregate SHA256:
`29ebfa2517ff497229623c55baaa8406162e2192f96d42484fc55a97e1ae7e66`.

Frozen native executable SHA256:
`637b7e7726fe14e1520cae06b598141215b2111c9bbaac876c5bb030d2cf4aa1`.
Frozen driver.py SHA256:
`5584a293538ef6f77a1d3c2533602cab2e926e0288485f6b7e211ebae46950c6`.
Frozen scheduler.py SHA256:
`8ced6ad4f64108f9d0bcda2d2aaf27e48628da95f5b31fe79676346f64b00975`.
Frozen test_driver.py SHA256:
`ce93d22edf78f9446de2ed4093583cebdf0c18fc7a8ebad8dc6fc5a1ef4ade7a`.

Raw receipts stay local. Source and aggregate results are backed up to the
existing recovery branch; LOCAL_ARTIFACTS.md records artifact identities.
