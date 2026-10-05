# Rare-position experiment results integration — October 5, 2026

Integrate the remaining local experiment reports, tools, regression fixture
and default-off Rust controls into `master`, whose production baseline is
`e3ce9653`. This records the findings without adopting another sampling policy.
Previously approved complete-tree, late-family and lean target-overflow
defaults remain in place. No database writes or service restart are involved.

## Disposition

| Experiments | Integration decision | Evidence and consequence |
| --- | --- | --- |
| R43–R56 lazy conditioning, partitions, rank cases and witness diagnostics | Preserve historical reports, tools and evidence; retain already shipped behavior | Rejected/deferred variants do not justify additional production stages. These reports predate the current production baseline. |
| R57–R60 family conditioning, training, late placement and CPU attribution | Preserve missing historical evidence and reproduction tools | The selected late-family fallback is already enabled in `430ac57f`; this integration does not repeat its adoption. |
| R61–R64 tree allocation and two-result fixture | Preserve remaining evidence and add the regression fixture/test | Lean target-overflow fallback is already enabled in `e3ce9653`. Small-root eligibility remains opt-in. The updated fixture combines two results with refreshed powers, so their individual effects are not isolated. |
| R65 pilot prior and failed-confirmation funding | Retain opt-in controls; defer default enablement | The narrow prior/empty-pilot-skip screen recovered updated Palmeiras/16th without measured coverage loss, but later draw-audit evidence requires a safe bound and renewed validation. Broader funding/allocation packages lose coverage or exceed the measured latency constraint. |
| R66 full native observation and parallel family finals | Remain proposed | No implementation or measured adoption result exists. |
| R67–R70 extended-work, backward analysis and generic guidance controls | Preserve offline example, drivers and all reported outcomes | High-work means are provisional. Sparse training can miss dominant rival families; large weights persist inside covered/refined families. More pilot work, an extra message slot, equal allocation, bound guidance and native goal scores do not earn a general production switch. |
| R68 structural discovery and partial families | Retain disabled controls for reproducibility; reject measured policies for adoption | The corrected 240-request campaign loses previous positives. The older analyzer missed invalid branch draw audits. |
| R71 tight guide-shape admission | Retain disabled control and stricter audit tooling; reject adoption | All 18 tight screening requests refine within the message grant, but each arm loses three cell-runs. A later 36-request batch adds two losses per tight arm. Draw-audit and settlement failures prevent a request-capacity claim. |

Negative results are useful evidence. Retaining opt-in research controls is
not default enablement, and successful construction is not successful event
discovery. Zero-free output and passing rough MAIN/CHECK gates do not certify
accuracy for the tiniest probabilities. No larger production work allowance
or relaxed acceptance criterion follows from this integration.

## Runtime and interface scope

The remaining Rust changes contain default-off failed-confirmation eligibility,
bank transfer/reservation, pilot allocation, small-root tree eligibility,
structural discovery, partial-family mixture correction and tighter guide
admission. Default full-family behavior and the legacy message path remain
available; invalid/unset experimental selectors preserve them. The offline
`branch_stratification` example adds separate pilot seeds, allocation controls
and diagnostics. It is not invoked by the HTTP service.

Rails/Go request and response shapes, routes, schema, ratings formulas,
publication gates and four-worker limits are unchanged by this integration.
The existing unrelated `db/schema.rb` edit and local scripts/assets are outside
the commit scope.

## Evidence and reproduction

Historical reports retain their original dates, measured binaries and claims.
Statements that no commit was authorized describe those experiment turns;
the subsequent user request to merge results authorizes this local integration.
The R71 report has a separate addendum for the fresh-seed batch found during
integration, rather than rewriting the stopped screening protocol as a passed
validation campaign.

Large raw artifacts are packed per campaign with relative member paths and
SHA256 manifests. Scripts, protocols, Markdown reports and reference fixture
JSON remain directly readable. Archives are read back and checked against
their originals before any duplicate is removed. The packaging inventory and
verification summary live in `2026-10-05-experiment-merge/`. To reproduce a
historical command, extract the corresponding `merge-evidence.tar.gz` in its
campaign directory first. Existing `raw-evidence.tar.gz` archives retain their
own original extraction instructions. Frozen executable hashes identify
historical measurements, not the currently running service.

The verified snapshot packs **6,728 files / 1,117,316,832 original bytes** into
23 new archives totaling **198,961,154 bytes**. The largest is 39,333,117 bytes.
Every persisted archive and member was checked twice against its original
before cleanup. The 218 directly readable candidates and the updated reference
fixture are retained. The packaging/prune safety suite passes all nine tests,
including changed originals, tampered archives, unsafe members, symlink
ancestors, changed deletion scope and files added after the snapshot.

Ignored `experiments/rare_positions/local/` artifacts are not added to Git.
Reports that reference them continue to identify them as local evidence;
this integration does not claim to have made every historical batch portable.

## Validation and remaining work

The frozen R71 measured Rust source passes the full optimized suite: **240 passed,
zero failed, four MySQL-dependent tests ignored**. This includes HTTP tests
with localhost access and serial execution. The 13 focused message tests and
seven strict analyzer tests pass. The six screening and six later default
comparisons match frozen R68 defaults excluding `work_spent`; the preceding
R65 final artifacts match the production baseline on both regression fixtures.

The integration source also contains a later correction to the coarse guide
precheck and formatting changes. Its source differs from the
frozen R71 executable; the historical screen is not a measurement of those
corrections. Final integration validation is recorded separately below.

Final integration checks pass: offline locked `cargo check`, formatting of the
11 changed Rust files, and the full serialized optimized all-target suite with
localhost access: **241 passed, zero failed, four database tests ignored** over
26 targets. The initial sandbox run had four HTTP bind denials; the permitted
rerun passes all four HTTP tests. No ignored database test was enabled.

The explicitly rebuilt executable has SHA256
`0e7d57adb68ac5d199d77e8e4e2d9c9be7ba7d014059439f0ca67d30b13ca34e`.
Both seed-808 default exports match the historical R71 controls byte-for-byte,
including `work_spent`. The current 84-file source snapshot has SHA256
`a383751cf4e3f7a963345ebcad63fe5ba8d1135a372978a7d1c0bff9e4de7443`.
Logs, source member hashes and export comparisons are in the merge directory's
`validation/`. The measured Rust source was unchanged during these checks.

The R71, R69, CPU-attribution, family-adoption and overflow Python test modules
also pass: 28 tests in total. The compact R71 matrix summary preserves both
invalid batches and returns status 2 as expected.

These passing checks do not resolve the recorded budget failures. The stricter
R71 analyzer intentionally returns status 2 for invalid matrices, while
preserving their records. Before another adoption campaign, expose and bound
the individual draw reservation that invalidates the audit, then evaluate
useful family discovery at unchanged capacity with fresh seeds and the other
reference fixtures. Validate already-positive estimates as well as zeros.
