# Threefold analytical budget for Palmeiras 16th

The previous [540-second experiment](TRIPLE_BUDGET_RESULTS.md) increased only
sampling. Its analytical residual certificate still came from a separate
180-second audit. This experiment gives the deterministic residual search
its own 540-second shared allowance, including proposal setup.

The target remains historical Palmeiras finishing 16th, cohort 5, with
279 played and 101 remaining matches. Sampling and analytical budgets are
separate: combining a 540-second sampler and this audit is not one
540-second endpoint result. The audit adds no Monte Carlo observations.

The successful repeat returns in 514.779 seconds and lowers cohort 5's
residual upper to 2.675523916963522e-38, from 4.826735149714054e-38 in the
old audit: a 1.804x improvement. It retains 34.97 million prefixes and does
not complete the residual tree. Combined with the 540-second sampler's own
core and other cohorts, the full interval becomes approximately
`[4.618240573e-41,4.868631642e-38]`, qualifying under the adopted rule.

## Memory-only preparation

Longer search retains a much larger frontier than the 12.49 million branches
in the earlier audit. The previous final refresh moved pending branches into
a second vector, temporarily duplicating the large Branch allocation. Heap
capacity could also double at growth boundaries.

Two changes in `odds-rust/frontier-recovery/src/joint/residual.rs` address
those allocation peaks without changing the mathematical bound:

- Refresh retains and mutates branches in place, preserving survivor order.
  Unknown and deadline-limited branches retain their valid bounds. A lookup
  error preserves the current/remainder before propagating the error.
- Heap pushes reserve exact chunks of about 25% growth, with a 1024-entry
  minimum, instead of relying on automatic doubling. Push sequence, priority,
  serial tie ordering, and search rules are unchanged.

All 66 optimized Rust tests pass. Added tests check heap pop order and
capacity growth, stable in-place retention and error handling. Independent
review found no soundness blockers. The existing unknown/deadline-refresh
tests remain in the suite.

A fixture parity check with suffix proofs disabled produces identical
bounds, 7341 expansions, retained counts, prunes, depth statistics and top
branches between the old v7 and new v8 binaries. The configured 10000-entry
guard ends those small checks. A separate check with timed suffix probes
has a one-expansion difference and a roughly 2e-15 relative upper difference;
its time-bounded probes are not a deterministic work comparison. These
checks establish behavior, not comparative latency.

Obsolete debug/release build caches under `/tmp` were removed before the
timed audit, freeing about 1.6 GB of shared-memory-backed storage. Experiment
build caches, frozen binaries, source and raw results are retained. The
execution environment has a 16 GiB memory limit. Allocation use and mandatory
cleanup remain workload-dependent; the changes are not a hard memory or
wall-time guarantee.

Memory changes are backed up in source commit
`c90a2df1e8b8dcf0c6f72531e052e72999f7962c` on
`experiments/rare-odds-recovery-2026-10-17` in `galo2099/golaberto`.

## Audit protocol

Use frozen `/tmp/frontier-recovery-memory-v8`, SHA-256
`b47ac8bec6d04bfa4247edbedbc4d772a6709e17494fd9d31192b6daed7052b8`.
Keep the earlier best joint-certificate settings: 100M outer expansion
ceiling, 1M cached suffix states, a 56-fixture suffix window, 25000 visits
and 25 ms per probe, 15% cumulative probe allowance, guaranteed-team keys,
joint native/guided caps enabled, and additional score-GD bounds disabled.
Prepare k=9 with a 250000-state cap and rebuild the compact regime. The
same 1000 ms limits apply to each preparation/regime call.

The shared allowance increases from 180 to 540 seconds. The first attempt
keeps the old ten-second cleanup reserve but reaches the protocol reply
deadline before returning a certificate. It reports a TimeoutError and no
analytical result; no bound is inferred from that failed attempt.

The repeat reserves 40 seconds for cleanup within the same 540-second
allowance. After setup this leaves about 499 seconds for the native audit,
approximately three times the old 169-second native allowance. Optional
cached refresh reserves at most one second inside the native deadline;
mandatory retained-state aggregation and cleanup remain included in the
returned wall receipt. No source or mathematical change is made between
attempts.

```bash
python3 residual_audit.py \
  --binary /tmp/frontier-recovery-memory-v8 \
  --suffix-bounds --suffix-max-remaining 56 --suffix-canonicalize-certain \
  --suffix-joint-ratio --budget 540 --cleanup-reserve 40 \
  --node-limit 100000000 --output joint-ratio-memory-pal16-540s-reserve40.json
```

The old 180-second audit uses v6; the new binary includes later optional
score-GD support but leaves it disabled. Memory allocation and elapsed-time
probe completion may affect throughput and adaptive traversal. This is a
longer-budget result using the same certificates, not an isolated timing
benchmark of memory changes. Raw JSON stays local.

## Returned analytical result

| Metric | Earlier 180-second audit | Successful 540-second allowance |
| --- | ---: | ---: |
| Recorded total wall | 174.609 s | 514.779 s |
| External shell wall | Not recorded here | 515.103 s |
| Cleanup reserve | 10 s | 40 s |
| Native audit allowance | Approximately 168943 ms | 498934 ms |
| Residual expectation upper | 4.826735150e-38 | 2.675523917e-38 |
| Maximum residual observation cap | 1.035193433e-32 | 1.035193433e-32 |
| Expanded branches | 16141340 | 37828610 |
| Retained prefixes | 12494771 | 34970893 |
| Outer terminal leaves reached | 0 | 0 |
| Guide zero-residual prunes | 3347114 | 12210704 |
| Additional joint zero bounds | 0 | 0 |
| Suffix cache entries | 928273 | 928273 |
| Upper-weighted mean frontier depth | 74.05 | 75.53 |
| Upper in depths 73-74 | 54.00% | 33.17% |
| Largest retained prefix's upper | 1.632138845e-44 | 1.549378238e-45 |

Both successful runs are incomplete and have no reported errors, hash
changes, memory limit, PMF-cache limit, suffix-capacity limit, or total-wall
overrun. The new receipt has zero outer leaves and 14 unknown suffix probe
roots, alongside 5921 positive and 5785 impossible probe roots. The PW
suffix-feasibility prune counter reaches 13157272. These are residual-search
counters, not new cell impossibility proofs or observations.

The accepted proposal is identical on its semantic metadata: historical
request, target/root/above set, selected nine rivals, pivot Mirassol 584,
156 roots (90 tied and 66 strict), complete 92975-state forest, 166182 total
states, pair-normalizer range, B=1.335714790055119e-32, and
H=1.304408974663202e-35. The memory changes preserve the guided proposal
and residual identity. Equality of B/H alone would not establish this match.

The native elapsed timer reads 500.789 seconds, including 1.944 seconds in
final refresh and 0.912 seconds in aggregation. The certificate call takes
513.549 seconds. Approximately 12.760 seconds lie outside that internal
elapsed timer, consistent with automatic retained-state destruction and
receipt construction before returning from the call. This phase is not
separately profiled. The successful receipt shows that work outside this
timer alone can exceed ten seconds at this frontier size; it does not
separately establish the failed attempt's exact cause. The successful run
returns within the larger 40-second reserve. Optional refresh's one-second lookup reserve does
not bound mandatory traversal or destruction of all retained state.

The failed first attempt records 540.216 seconds total, a TimeoutError and
no certificate; external shell wall is 540.525 seconds. It is retained for
provenance and excluded from all bound calculations. The total work spent on
this task therefore includes that failed attempt as well as the successful
repeat; no combined interval is claimed to fit that cumulative task time
inside one 540-second request.

## Pattern revealed by more analytical work

The upper falls 44.57%, while expansions increase 2.344x and pending prefixes
increase 2.799x. Thus more analytical time helps, with sublinear bound
improvement in this observed pair. It does not collapse the problem into a
few dominant branches or close the tree.

The frontier moves modestly deeper. Depths 74-77 now carry 72.44% of its
upper, with 14-17 fixtures still unassigned in the 91-fixture suffix.
Depths 70-79 carry 94.74%; the depth-80-or-later share increases from 1.68%
to 5.26%. The largest individual prefix contributes only 5.79e-8 of the
new upper. The remaining allowance is even more diffuse than before.

The maximum residual observation cap is unchanged apart from rounding.
The gain tightens the sum of expected residual contributions across the
partition, not the cap on every possible future observation. The suffix
cache remains at 928273 entries below its 1M target; increasing that cache
limit has no demonstrated benefit here. There are still no additional joint
zero proofs. These results support further expectation-bound integration,
and do not establish that GD or PW feasibility is the dominant remaining
loss. The source/rival-weighted audit proposed in
[REMAINING_MASS_ANALYSIS.md](REMAINING_MASS_ANALYSIS.md) is still needed
for that attribution.

## Combining with sampling, with explicit budgets

Use only the 540-second sampling run's existing statistics:

```text
cohort-5 core upper                 6.970578528214142e-40
new deterministic residual upper   2.675523916963522e-38
new combined cohort-5 upper        2.745229702245664e-38
other cohort uppers                2.123401939443611e-38
full upper, outward rounded        4.868631641689276e-38
full lower, unchanged              4.618240572657253e-41
```

The interval qualifies under the adaptive rule: exponent halfwidth 1.511465
versus allowance 1.941203. It uses a 540-second sampling allowance and a
separate 540-second analytical allowance, not one 540-second endpoint run.
The failed analytical attempt adds further task work. No old statistical
interval is combined with the new run. The deterministic bound consumes
no additional failure-probability allocation.

Using the earlier analytical certificate with the same 540-second samples
gave upper 7.019842874439809e-38. Increasing analytical work reduces that
combined full upper by about 30.64%, beyond the sampling-budget improvement.
The full-cell upper is now about 1054.22 times its lower. Qualification is
under the adopted adaptive logarithmic rule, not a factor-two guarantee.

Separately, combining the new certificate with only the old 180-second
sampler's core and other cohort bounds gives
`[2.589898304031079e-41,1.8320894364915898e-37]`, also qualifying under
the adaptive rule. This is another independently stated combination with
separate sampling/analytical budgets, not an intersection of old and new
statistical intervals. It illustrates that analytical time also improves
the earlier sampling result.

## Reproducibility

Two independent reviews verify semantic proposal compatibility and explicit
core-plus-residual arithmetic. Raw receipts remain local and unchanged.

| Artifact | SHA-256 |
| --- | --- |
| Successful `joint-ratio-memory-pal16-540s-reserve40.json` | `baf21dfa6b0c313f2f47b19d6a96f696d53d662680c3d2b5caca62d2d64eb312` |
| Failed `joint-ratio-memory-pal16-540s.json` | `81ec5bab489c19eda62248c767285fc38bca6e85e35afca50c44d5f74d4fbca7` |
| New aggregate source/binary manifest | `3ccde011845b6ab8b0d5e63dca201ffc4190854fcd370e4d51e7e79762b7bb89` |
| Old `joint-ratio-pal16-180s.json` | `5badc49fd8a559660198d38db024322de687c71972d6b1b8871dba181df60136` |
| `lazy-hybrid-pal16-540s.json` samples | `9a4cc6173966a6a386684278c30682b840686f6ec28b1a78725fc479014301b0` |

The source commit above and this aggregate report are backed up on the
existing experiment branch. Production endpoint code is unchanged.
