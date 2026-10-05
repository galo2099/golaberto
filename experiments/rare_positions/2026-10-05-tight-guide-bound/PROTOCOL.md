# R71: exact guide-shape admission before structural discovery

October 5, 2026. Protocol written before measured runs. Continue R68–R70 with
production defaults unchanged, no database writes, commits or pushes.

## Hypothesis and scope

R68's bounded retuner repeatedly refuses guide construction using a global
sum of fixture gains as every team's span. Test whether admission based on
the actual forward and reverse guide shapes permits useful refinement inside
the same work grant. This is an admission experiment, not a change to the
probability model, acceptance gates, work capacity or sampling counts.

The new `RUST_ODDS_EXPERIMENT_TIGHT_GUIDE_BOUND=1` control applies only to the
existing bounded experimental retuner. Its preflight uses checked arithmetic
to predict the logical charge of both guide tables before construction, and
charges its own traversal against the same grant. Reverse-table origins must
include zero-probability outcomes, as in `Guide::new`. The existing post-build
charge/check stays in place. Default and legacy retuning keep their streams
and behavior.

The modeled charge is a scheduling proxy, not a CPU instruction count,
physical memory bound or hard latency guarantee. A tighter precheck does not
make proposal fitting, density evaluation or final sampling free. Partial
family density bounds are outside this experiment.

For `g` remaining fixtures and `n` teams, each forward row has `F + 1`
values and each reverse row has `R + 1` values. `F` is the largest team sum
of supported gain maxima; `R` is the largest team sum of unconditional gain
maxima. Each table has `(g + 1) * n` logical rows, so the exact existing charge
is `2 * (g + 1) * n * (F + R + 2)`. Shared row storage does not reduce that
logical count. The coarse formula is not assumed to dominate this count for
every possible shape: both tables must be included even when a fixture graph
is concentrated on few teams.

## Fixed comparison

Run serialized requests with four workers and frozen executables. Clear all
inherited experiment/settings variables before setting each arm. Preserve
commands, effective flags, input/binary/export/log hashes, process exit status,
wall/CPU and raw diagnostics. Do not replace failures or increase work after
observing them.

Six arms: candidate default control; frozen R68 v5 default; coarse rank
discovery; tight rank discovery; tight interval discovery; tight strict
discovery. Compare each experimental arm with candidate control and coarse
rank, and compare candidate defaults with frozen defaults after removing only
`work_spent`. Rank versus interval/strict with the tight precheck tests proposal
selection after the admission prerequisite; it does not isolate score effects
from all subsequent pilot/allocation changes.

Screen both retained group-16498 snapshots with seeds 808, 1669 and 1993:
36 requests. Record successful and failed message attempts, zero-hit selected
branches, rejection reasons, predicted guide charges, remaining grants,
actual message work and settlement bounds. Inspect all three diagnostic cells
(Flamengo13, Palmeiras15, Palmeiras16), including already-positive estimates,
and all other matrix entries.

Continue only if a tight arm successfully refines at least one previously
zero-hit branch on a retained diagnostic cell within its grant, with no work
overrun or default-parity failure. Otherwise stop with the admission result;
do not compensate by increasing the grant. If continuation is justified,
freeze all arms and run both retained snapshots at fresh request seeds 60211,
60217 and 60223 (36 requests), plus all six other reference fixtures at 808 and
1993 (72 requests). Keep every arm in validation regardless of screening
coverage. These fresh request seeds change training and final streams together,
so they cannot separately identify training and final variance.

## Evidence and decision

Before new measured runs, the stricter analyzer was also applied to all 60
archived R68 v5 screen requests. It flags 30 records with
`branch_draw_audit_valid=false`, including some default controls. None of
these records fails the aggregate reserved-minus-already-released comparison;
the validity bit also checks individual draw reservations. The earlier R68
analyzer did not test that bit. Preserve this distinction: a failed local
draw audit is not automatically a whole-request bank overrun, but it prevents
claiming that all admission bounds are demonstrated. The R71 screen and its
stop rule stay fixed; these archived failures are a baseline limitation, not
new tight-guide observations.

Verify the predicted tight guide charge against the existing post-build
logical count in targeted tests, including asymmetric degrees, zero-mass
outcomes, empty guides, insufficient/exact grants and checked failures.
Compare ample-grant proposal probabilities and draws with the old path.
Run the Rust library suite, targeted example/fixture tests and checks; record
any unavailable database or socket requirements. Freeze the resulting source
and binary before measured runs.

Report nonzero gains/losses, changed positive estimates, reachability parity,
MAIN/CHECK diagnostics where available, attempt outcomes, modeled work and
paired timing. Agreement with R67's rough means is descriptive; they are not
exact probabilities or certified reference truth. Zero coverage is not an
impossibility proof. No production adoption follows merely from successful
construction or a passing estimate. Stop after the preselected comparisons,
document the result and leave the control default off.
