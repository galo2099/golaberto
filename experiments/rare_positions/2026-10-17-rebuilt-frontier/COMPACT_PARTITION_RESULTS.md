# Compact WDL partitions with shared joint suffix tables

The experimental native estimator now supports a required-above rival with
more than 64 compatible incident WDL assignments. It retains lightweight
assignment records and one shared suffix forest, rather than rebuilding a
complete joint conditioning table for every assignment. The production odds
endpoint is unchanged.

## Construction and statistical contract

All fixed pivot-incident outcomes contribute their native probability once.
Each tied points/wins assignment retains its own conditional GD normalizer and
native conditional score plan. Strictly ahead assignments remain in the
partition with GD factor one. Unknown normalizers, expensive rejection plans,
unsupported mixture tickets and interrupted assignment enumeration reject the
candidate without replacing the installed proposal.

The suffix uses one fixture order, one set of masks and one memo keyed by
fixture position and the remaining selected teams' packed points/wins state.
Assignments reaching equal states share suffix nodes; their distinct score
laws and GD factors remain separate. Per-assignment domain propagation is
omitted so the common suffix masks remain valid. Selected parent constraints
are preserved and the pivot is included when necessary. Unfinished suffix
edges retain the immutable native-mask fallback and its certified cap.

For assignment a, let P_a be its native prefix probability, q_a its GD factor,
H_a the suffix proposal potential, C_a the suffix contribution cap, and rho_a
the actual integer-ticket selection probability. Nominal mixture weights are
P_a.mid * q_a.mid * H_a. The observation weight is
`(P_a / rho_a) * suffix_weight * q_a`; the certified cap is the outward-rounded
maximum of `P_a.hi * q_a.hi * C_a / rho_a.lo`. Assignment probability and GD
normalization therefore enter once each. The confidence calculation is unchanged.

Compact enumeration is bounded at 4096 lightweight assignments. Unique suffix
states, prefix records and pair-plan fixture records are charged against the
candidate state allowance. Unsupported categorical totals reject a candidate;
they do not truncate the partition. The existing per-assignment implementation
remains available for partitions with at most 64 assignments.

## Historical Palmeiras 16th, cohort 5

Input canonical identifier `40b8ab70b9fc`, the 279-played-game request documented
in PAL16_RESULTS.md. Cohort numbers refer to the reconstructed partition.

The real-fixture setup accepts team 584's full 156-assignment partition:
90 tied points/wins assignments and 66 strictly ahead assignments. All 90 GD
plans complete; their normalizers range from approximately 0.0077876 to
0.0133816. With the initial seven-rival parent, the shared suffix uses 12648
states and the accepted contribution cap is `1.3942847810974012e-28`.
In the frozen shared-budget run, team 584's GD-plan preparation took 93.1 ms.
The complete candidate search used its 1000 ms allowance while trying later
pivots. Team 24's 127-assignment partition also completes.

An isolated check first completes the nine-rival joint parent at 69691 states
and cap `1.0140615550832632e-31`, then attempts the compact proposal with the
same total 250000-state / 1000 ms allowance. Team 584's proposal completes at
92975 unique suffix states and 166182 total retained states including the
parent and metadata. Its cap is `1.335714790055119e-32`, a factor 7.59 reduction.
The full candidate search took 1001.2 ms; later candidates consumed the rest
of that allowance after the successful proposals had completed.

Team 67 remains limited by the rejection-cost gate: a tied assignment has GD
probability about `3.13e-5`, below the existing 0.001 floor. That candidate is
rejected as a whole. This is a setup limitation, not an impossibility proof.

## Same shared 180-second budget

The Python controller, confidence allocation, seed, request and budget settings
are unchanged. Both runs use one worker, depth 9, initial seven-rival/1000-state
plans, growth through eleven rivals and 250000 states, 128 discovery draws,
4000000 total draws per cohort, a one-second output reserve, and full-budget
sampling. Source changes and builds finish before the clock.

| Run | External wall | Draws | Lower | Upper | U/L | Qualified |
|---|---:|---:|---:|---:|---:|---|
| Previous reconstruction |179.241s|7165920|4.1384946899147247e-41|1.2085487829286124e-36|29202.6|No|
| Compact WDL partition |179.249s|7297920|4.2494598410268494e-41|1.184683963327682e-36|27878.5|No|

The observed ratio improves only about 4.5%; this single adaptive run does not
establish a general performance improvement. Fifteen regime proposals were
installed initially, including cohort 5, but later joint-only refinements
replaced all of them. The controller in this run attempted GD regime setup only
during its initial stage. It did not rebuild a compact proposal after the
selected joint group grows. Thus the isolated nine-rival cap reduction above
was not used by this measured controller run.

Cohort 5 ends with its original nine-rival joint cap `1.0140615550832632e-31`,
4000000 total draws, and interval
`[4.5835494545051e-44,1.1709157088914786e-36]`. Its final epoch has 3994872
draws and 33 nonzero upper-score contributions. These are weighted score
contributions, not ordinary event hits. It still supplies 98.84% of the global
upper bound. One cohort is proved zero; all 15 live cohorts receive samples.

Reported regime setup increases from 189.9 ms to 4049.3 ms; all preparation
increases from 5444.4 ms to 8796.5 ms. Native sampling takes 141456.3 ms.
There are no sample errors, confidence conflicts or uncovered mass, and all
measured hashes remain unchanged. No qualified point estimate is reported.
The subsequent scheduling experiment rebuilds GD conditioning after accepted
joint growth; see GD_REBUILD_RESULTS.md. The fixed-plan cap diagnostic above
does not measure that scheduling policy's budget impact or confidence interval.

## Validation

31 optimized Rust tests and 20 Python tests pass. New tests cover distinct
prefixes sharing a suffix state, exhaustive native event mass and actual
categorical likelihood correction under internal partial-state caps 1 through
3, multi-root amortization, and the real 156-assignment compact mixture with
both tied and strict support. Fixed conditional scores agree with the sampled
WDL outcomes. Independent Sol review found no statistical or support blockers.

The 128-root amortization stress test uses repeated prefixes to isolate memo
reuse; a separate test covers distinct assignments, and the real-fixture test
covers 156 unique assignments. The SciPy beta-inverse floating-point assumption
documented in the existing confidence review still applies.

Frozen binary SHA256:
`637b7e7726fe14e1520cae06b598141215b2111c9bbaac876c5bb030d2cf4aa1`.
Frozen joint.rs SHA256:
`aad3cdf3140b848956a9dfbdaaacae12ccb60889a8a266e5775470ca5a356cd4`.
Frozen engine/regime.rs SHA256:
`f84926088431499f7d03fee58937ce2532fe1464b5d5e40b39d174601519fdf9`.
Measured source aggregate SHA256:
`dbd5a5c4ae249b32b9ae3ed7fdea13f845e23b4412477a4778c02d536066476f`.

Raw receipts remain local. Aggregate results and source are backed up to the
existing recovery branch; LOCAL_ARTIFACTS.md records raw artifact identities.
