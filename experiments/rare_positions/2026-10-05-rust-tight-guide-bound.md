# R71: tight guide admission enables refinement but fails the adoption screen

October 5, 2026. Screening complete. Production defaults are unchanged. No
database writes, commits or pushes.

The exact guide-shape precheck removes R68's construction barrier: all 18
tight-arm requests successfully refine a previously zero-hit Flamengo/13th
branch within the existing message allowance. This does not repair the
probability estimates. Each tight arm gains one cell-run and loses three
previously positive cell-runs against defaults; none recovers a diagnostic
zero. The stricter audit also flags 18 requests, including default controls.
The prewritten continuation rule therefore stops the campaign after its
36-request screen. Fresh-seed and other-reference validation were not run.

## Implementation and fixed protocol

R68's bounded retuner used the sum of possible gains across remaining fixtures
as every team's guide span. `RUST_ODDS_EXPERIMENT_TIGHT_GUIDE_BOUND=1` now
predicts the existing logical charge from the actual forward and reverse
table shapes before construction. It applies only to already-bounded
experimental retuning. The legacy/default retuner, discovery scores,
acceptance gates, node cap, pilot/final counts and request bank are unchanged.

For `g` remaining games and `n` teams, the forward span `F` is the largest
team sum of supported gain maxima. The reverse span `R` uses unconditional
gain maxima, including zero-probability outcomes, to match `Guide::new`'s
origins. Each table has `(g + 1) * n` logical rows. The existing exact charge
is `2 * (g + 1) * n * (F + R + 2)`. Shared physical row storage does not reduce
this scheduling charge. Checked arithmetic rejects invalid or overflowing
inputs before construction. The estimator charges a proxy of `g + 4*n`
units to the same grant before its traversal. The post-build charge/check
remains, with a debug assertion comparing the saved prediction to the actual
logical count. Attempt logs retain predictions, rejection reasons and the
remaining grant after each attempt.

The coarse expression is not a universal upper bound on both tables: an
exact calculation can be larger on concentrated fixture graphs. The new
precheck follows the table shape rather than assuming every new bound is
smaller. These are modeled scheduling units, not exact CPU instructions,
physical memory consumption or a wall-time guarantee. Family density
admission is outside this change.

The protocol was written before measured runs in
`2026-10-05-tight-guide-bound/PROTOCOL.md`. Requests run serially with four
workers, both retained group-16498 snapshots and seeds 808, 1669 and 1993.
Six arms use frozen binaries: candidate default control, frozen R68 v5
default, coarse rank discovery, and tight rank/interval/strict discovery.
Inherited controls are cleared; commands, effective flags, hashes, exits,
exports, raw logs and wall/CPU costs are retained. All 36 child processes
exit successfully, and the analyzer verifies the complete manifest and hashes.
All six candidate-default comparisons match frozen defaults after excluding
only `work_spent`.

## Construction succeeds inside the message grant

Every tight arm selects one unseen Flamengo/13th branch on its first attempt.
The original snapshot selects index 18 in all three modes. Updated rank and
strict select index 24; interval selects index 29. These are construction
successes, not event evidence or probability certificates.

| Fixture / mode | Coarse rank attempts / successes | Tight attempts / successes | Tight message work | Predicted guide charge |
| --- | --- | --- | ---: | ---: |
| Original / rank | 14 / 0 | 1 / 1 | 8,911,071 | 5,286,000 |
| Updated / rank | 15 / 0 | 1 / 1 | 8,176,674 | 5,074,560 |

These rows hold for each screening seed. Coarse rank spends 19,999,775 units
on the original snapshot and 19,999,671 on the updated snapshot. Its attempts
end with 13 or 14 `guide_bound` refusals followed by `work_allowance`.
Tight rank leaves 11,088,929 and 11,823,326 units of its 20-million message
grant. The interval/strict tight message charges range from 8,159,163 to
8,909,402 units. Failed construction is no longer the reason these runs lack
a useful refinement.

## Coverage and estimate quality still fail

Counts are cell/request comparisons across six cohorts, not distinct cells
or accuracy guarantees. Changed-positive counts exclude `work_spent`.

| Arm versus candidate defaults | Zero to positive | Positive to zero | Changed positive | Reachability changes |
| --- | ---: | ---: | ---: | ---: |
| Coarse rank | 1 | 4 | 3 | 0 |
| Tight rank | 1 | 3 | 4 | 0 |
| Tight interval | 1 | 3 | 4 | 0 |
| Tight strict | 1 | 3 | 4 | 0 |

The three tight-arm exports are identical after excluding `work_spent`, even
though updated interval selects a different unseen branch. Against coarse
rank, each tight arm regains original/808 Palmeiras/14th at 4.498e-25, with
aggregate ESS 5.91 and maximum observation share 31.09%; it adds no losses.
This is a rough estimate with `meets_precision_goal=false`. Against defaults,
the gain is original/1993 Palmeiras/14th at 1.088e-26. The lost cell-runs are
Flamengo/13th at original/1993 and updated/1993, plus Palmeiras/14th at
updated/808. No loss becomes an impossibility classification.

The three diagnostic cells show no useful recovery:

- Flamengo/13th stays zero at original/1993 and every updated seed. Original
  808 and 1669 publish 3.835e-34 and 2.512e-34, versus default 1.173e-33 and
  8.155e-34. R67's original provisional mean is 7.56e-34; it is descriptive
  high-work evidence, not exact truth.
- Palmeiras/15th is unchanged: original/808 remains 1.000e-33, approximately
  486 times below R67's original rough mean of 4.86e-31. The other five
  screening estimates remain zero.
- Palmeiras/16th is unchanged in all six cohorts, including its zeros.

Successful construction does not supply successful exploration. Updated
808 and 1669 still skip Flamengo/13th finals for lack of event evidence after
the structural re-pilot. Updated 1993 runs finals but rejects them: MAIN ESS
1.001, maximum share 99.941%, MAIN 4.267e-33 versus CHECK 1.180e-34.
Original/1669 accepts a rough pair with MAIN ESS 5.23 and CHECK ESS 7.60,
yet its published estimate remains below the provisional high-work scale.
Covered and refined proposals can retain the heavy-weight problem seen in
R69–R70. All probabilities in this report are fractions.

## The stricter audit prevents a capacity claim

Before new runs, the new analyzer rechecked all 60 archived R68 v5 screening
requests. It flagged 30 records with `branch_draw_audit_valid=false`, including
some defaults. The older analyzer did not inspect this field.

The new screen flags 18/36 records: three per arm, including control and
frozen. The failed field is exclusively `branch_draw_audit_valid`; no recorded
aggregate reserved-minus-already-released comparison fails, and no explicit
stage settlement exceeds its grant. `DrawAudit::record` also tests individual
draw reservations, so aggregate headroom does not establish valid local
bounds. Tight arms have the same flagged cohorts as coarse rank. No new tight
guide-grant failure is observed, but the wider request admission contract is
not demonstrated. The analyzer preserves this result and intentionally exits
with status 2 rather than marking the matrix valid.

The protocol requires no work-bound failure before continuation. Accordingly,
no 60211/60217/60223 holdouts or six-other-fixture validation were launched.
The coverage losses independently reject adoption. The next prerequisite is
to expose the stage/draw reservation that invalidates the audit and obtain a
safe bound for it, while improving discovery of useful families rather than
treating construction success as evidence. These results do not justify more
request capacity or relaxed acceptance.

## Cost, validation and reproduction

The 36 new child runs total **58.590 seconds wall / 124.863 CPU seconds**.
Compilation, tests, archive analysis and idle time are excluded. Mean paired
changes against candidate controls are:

| Arm | Mean wall change | Mean CPU change |
| --- | ---: | ---: |
| Coarse rank | +4.10% | +2.34% |
| Tight rank | +1.66% | +1.27% |
| Tight interval | +5.37% | +3.48% |
| Tight strict | +2.05% | +2.57% |

These single serialized screening measurements do not establish production
latency or a speed improvement. Fitting and final streams change together
with each request seed; this screen does not isolate their variance.
The updated snapshot combines two results with refreshed power fields, so
it cannot separately attribute their effects.

Validation: 13 focused message tests and seven Python analyzer tests pass;
`cargo check --locked`, owned-file formatting and whitespace checks pass.
The full optimized Rust suite passes **240 tests, zero failures**, with four
database tests ignored because they require `MYSQL_TEST_URL`. The initial
debug attempt was blocked on localhost binding; with localhost access, two
debug HTTP tests hit their existing 10-second read timeout. The optimized,
serialized full suite includes all four HTTP tests and passes without changing
their timeout. All check logs are retained. No database tests were activated.

New runtime edits are confined to `odds-rust/src/joint_caps/propagated/lazy/strata/messages.rs`;
new experiment scripts and reports live under this campaign. Existing
uncommitted work is preserved. The frozen source archive includes the existing
Rust project and its local player-rating dependency. Source SHA-256:
`a4b9550cc0419ef0a06b1502e3537f27b77e5436eb817644e47a5c1d74a063f5`.
Binary SHA-256:
`c5b299ee903ebdec0697aa6fe09fcdc4dc04c3dc502ff98b6c77ee84ac8c12c2`.
Hashes, source diff against R68, archived baseline audit and raw screen results
are in `2026-10-05-tight-guide-bound/`.

From the repository root, reproduce into a new destination:

```bash
python3 experiments/rare_positions/2026-10-05-tight-guide-bound/scripts/run_matrix.py \
  --candidate-binary experiments/rare_positions/2026-10-05-tight-guide-bound/data/golaberto-odds-r71-v1 \
  --frozen-production-binary experiments/rare_positions/2026-10-05-structural-discovery/data/golaberto-odds-r68-v5 \
  --out-dir /tmp/r71-screen \
  --fixtures original-16498,updated-16498 --seeds 808,1669,1993 \
  --arms control,frozen,coarse-rank,tight-rank,tight-interval,tight-strict
python3 experiments/rare_positions/2026-10-05-tight-guide-bound/scripts/analyze_matrix.py \
  --run-dir /tmp/r71-screen
```

An audit-failing reproduction returns analyzer status 2 and retains its report.
Keep the tight guide control default off; no production switch is recommended.

## Later batch reconciled during results integration

The working tree also contains a separate 36-request batch under
`fresh-validation/`, using seeds 60211, 60217 and 60223 and the same six frozen
arms, fixture hashes and four-worker limit. This was not part of the completed
screen described above; its presence does not satisfy or supersede the
screen's stop rule. The merge preserves it as additional diagnostic evidence.

All six default comparisons again match frozen defaults excluding
`work_spent`. Each tight arm gains one cell-run and loses two versus defaults;
coarse rank gains none and loses two. Tight rank loses original/60217
Palmeiras/15th and updated/60211 Palmeiras/14th; it adds updated/60211
Flamengo/12th (team 17). Reachability does not change. Combined with the
screen, each tight arm has two gains and five losses over twelve matched
fixture/seed cohorts.

The analyzer flags 13 records. All arms have invalid branch draw audits;
some also exceed aggregate `reserved - already_released` draw reservations.
The coarse-rank arm additionally reports a target-overflow settlement failure.
These failures strengthen the need to repair draw admission before claiming
capacity preservation. They do not justify enabling the tight guide control.
Other-reference validation remains absent. The raw manifest, records, exports
and analyzer output are preserved with the other experiment evidence.

The integration source has a later coarse-precheck correction: its conservative
charge multiplies the row-span bound by four to cover both tables and both
slots, rather than the historical factor of two. A concentrated-fixture test
demonstrates the old underestimate. This correction affects only bounded
experimental retuning; it was not used by the frozen screen or later batch.
Their recorded costs must not be attributed to the corrected source.
