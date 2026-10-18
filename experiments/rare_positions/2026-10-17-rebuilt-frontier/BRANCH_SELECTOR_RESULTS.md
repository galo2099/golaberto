# Branch-specific score-bound selector: Palmeiras 16th, cohort 9

The selector now finds relevant GD comparisons: score integration tightens
the completed envelopes 9.42 times on the same prefixes, mainly through
Mirassol. Overall performance still regresses in the matched 180-second
test: residual upper `3.6763215537891844e-35`, versus
`2.8965680615497966e-35` for the fixed selector, 26.92% higher. Fewer
envelopes finish, and unfinished branches supply 99.69% of the new upper.
The retained full-cell interval remains unchanged. This remains an opt-in
experiment; no 540-second run of this selector has been performed.

## Selection and bounds

This opt-in experiment adds `expectation_branch_selector`, requiring
`expectation_score_gd` and its existing shared-expectation/score-GD
prerequisites. The default remains the previous fixed-rival selector.
Production code is unchanged.

The previous selector chose Vitória once per source using its initial
40-goal deficit and individually reachable PW tie. Its completed residual
envelopes had no nonzero tied contribution, so Vitória's GD constraint
could not tighten their nonzero strict-PW contribution.

The new selector prepares a bank of individually tie-eligible rivals, then
chooses at each outer prefix entering shared expectation integration:

1. Reject rivals whose remaining PW support cannot reach the target tie.
2. Prefer rivals whose individually feasible completions are all tied.
3. Otherwise prefer higher native PW tie share among individually feasible
   completions.
4. Break ties by the smaller analytical GD moment bound, then team ID.

Tie share is a search-ordering heuristic. It ignores other rivals' joint PW
requirements and residual clipping. It never scales a returned probability,
prunes mass, or establishes impossibility. The shared expectation recurrence
still certifies strict and tied channels using the actual native/guided
likelihood correction and the chosen rival's conditional score moments.
If no candidate remains, integration uses the ordinary unweighted bound.

The selector cannot retain every rival's exact consumed score history on
each frontier branch: tens of millions of branches would make that memory
cost excessive. Instead, shared max-product tables recover a conservative
prefix moment envelope for each rival, indexed by consumed fixture count
and that rival's exact packed PW gain. At a rival fixture, transition the
gain and take the maximum moment product over compatible W/D/L histories.
At other fixtures, multiply by the maximum allowed conditional moment.
This includes target-only fixtures and the full shared-fixture coefficient
of magnitude two. Ignoring other past constraints enlarges the set of
histories and therefore keeps the envelope conservative.

The source's initial moment enters once outside these prefix tables, and
the future conditional moment enters through the existing cached recurrence.
Recovered moments can exceed one; they are not probabilities. The old outer
fixed-rival bound and its exact branch moments remain unchanged. No arrays
are added to the heap branch representation. Kernel/source/rival PW identity
remain part of the shared expectation cache key.

The bank shares the existing five-second score setup allowance with the
old outer bound. It has a 500000-state total prefix/suffix cap and a separate
five-million-operation setup guard. Checked rival-only gains avoid unrelated
opponent arithmetic overflow, and any saturation, missing recovery table,
deadline or capacity failure skips the candidate conservatively. Prefix
and suffix capacity counts include all positions. Tie-share calculations
use direct tied mass and tail sums to avoid upper-tail CDF cancellation.

Independent review found no mathematical blocker. All 81 Rust and 62 Python
tests pass. Tests cover actual selector switching between two branch
states, no-candidate fallback, exhaustive tiny prefix-history products in
both fixture orientations, shared and target-only fixtures, separate source
initial factors, checked oversized gains, setup deadlines/caps, score-bound
coverage against the exact finite-score oracle, and unchanged sampling caps.

## Fixture and shared budgets

The historical request has 279 played and 101 remaining matches, SHA-256
`cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee`.
The target is Palmeiras finishing 16th, cohort 9. Palmeiras must lose all
ten remaining games, finishing on 57 points and 16 wins. Corinthians is
below, Grêmio and Mirassol above; Internacional, Remo and Chapecoense are
also below.

The complete saved joint proposal has k=11, 230755 states, selected IDs
`[584,67,18,79,10,14,7,225,6,4,74]`,
`B=8.057494973466603e-35`, `H=7.86864743502598e-38`.
There are fourteen individually reachable rival PW ties in the historical
fixture. For example, Vitória needs 7W/3D, Grêmio 9W/1D, Mirassol 8W/1D/1L,
and Vasco 8W/2D/1L. Those are individual requirements, not joint reachability
certificates. Future Palmeiras losses also change each final GD gap.

The matched fixed-selector and branch-selector runs use the same frozen
binary and have separate 180-second shared allowances including setup,
with a 20-second cleanup reserve. Both enable score-aware shared expectation,
outer score-GD bounds, four threshold bits, four million shared cache rows,
a 36-fixture expectation window, a 56-fixture suffix window, canonicalization,
and joint-ratio bounds. They run sequentially without concurrent builds or
heavy jobs. Selector preparation and selection consume the treatment's
existing allowance.

The fresh fixed-selector control returns in 169.392 seconds with residual
upper `2.8965680615497966e-35`, matching the proposal and retaining its
entire nonzero archived contribution in Vitória's strict PW channel.
The prior fixed-selector result `2.915014853146674e-35` is close, but the
fresh same-binary result is the matched comparator.

## Matched 180-second results

| Metric | Fixed Vitória selector | Branch-specific selector |
| --- | ---: | ---: |
| Wall seconds | 169.392 | 168.813 |
| Residual upper | 2.8965680615497966e-35 | 3.6763215537891844e-35 |
| Archived upper | 1.307412325854095e-36 | 1.1296523521955655e-37 |
| Pending upper, summed by depth | 2.7658268261845486e-35 | 3.6650250271551396e-35 |
| Pending share of total upper | 95.49% | 99.69% |
| Outer prefixes processed | 8098953 | 7294583 |
| Completed prefixes absorbed | 601614 | 353439 |
| Pending prefixes | 7769491 | 7731526 |
| Shared cache rows / completed rows | 4000000 / 3999917 | 4000000 / 3999779 |
| Canonical PW states represented | 343970 | 260701 |
| Recorded expectation-cache seconds | 19.086 | 18.744 |
| Incomplete queries that tightened outer bounds | 0 | 0 |

Both receipts have matching proposal B/H, identical source manifests and
binary, unchanged hashes, no errors or wall-budget overrun, and no reported
memory or marginal-PMF limit. Both reach the shared expectation row ceiling
and leave the frontier incomplete. There are no moment overflows or invalid
expectation states.

The branch selector prepares all fourteen eligible rivals in 15.182 ms,
using 33790 prefix states and 40473 suffix states, 74263 total; no setup
capacity, work or time guard is hit. Setup is therefore small in this run.
It makes 1458608 queries and selects a rival forced into a PW tie on
1453362 of them, 99.64%. Every query finds a candidate. These are query
counts and individual-support facts, not probability-mass shares or joint
feasibility certificates.

| Selected rival | Query count | Completed prefixes | Same-prefix plain upper | GD-adjusted upper |
| --- | ---: | ---: | ---: | ---: |
| Mirassol 584 | 1241490 | 330200 | 9.832542030387858e-37 | 8.193748289174408e-38 |
| Grêmio 79 | 77838 | 15474 | 6.299848092579972e-38 | 2.7680922836888333e-38 |
| Vasco 18 | 51974 | 5171 | 1.2102923720195274e-38 | 2.479702276420276e-39 |
| Santos 6 | 81910 | 2161 | 3.242226111476143e-39 | 4.027370713893165e-40 |
| Coritiba 10 | 3855 | 283 | 1.2839806524225152e-39 | 1.771459002722193e-40 |
| Botafogo 7 | 1535 | 150 | 7.383078020424066e-40 | 2.8724424009408237e-40 |

Fluminense 8 and Atlético-MG 4 receive four and two queries, respectively,
with no completed absorbed prefix. Vitória is never selected by the new
selector in this run. Mirassol receives 85.11% of queries. Its completed
contribution is entirely PW-tied and tightens twelve times through GD.

Across all absorbed prefixes, the same-prefix plain upper is
`1.0636201222559408e-36`, and the score-adjusted upper is
`1.1296523521955655e-37`, a 9.41546-fold tightening. Raw strict contribution
is approximately `3.11064668765405e-40`; raw tied contribution is
`1.0633090575871786e-36`, reduced to `1.126541705507855e-37`.
These channels are upper-envelope diagnostics, not observed probabilities.
Per-rival comparisons take the minimum with the same pre-existing outer
bound on both sides, so they do not credit the selector for earlier outer
bound tightening.

The archived comparison between the two runs alone would overstate the GD
gain, because the treatment absorbs 41.25% fewer prefixes. The same-prefix
comparison above isolates the score operation on its completed prefix set.
The treatment also processes 9.93% fewer outer prefixes and represents
fewer canonical PW states under the four-million-row ceiling. Rival-specific
cache identities and different search trajectories affect coverage; this
single comparison does not isolate their separate CPU or reuse costs.

## Remaining uncertainty and confidence

The smaller completed upper does not compensate for the pending frontier:
`3.6650250271551396e-35`, or 99.69% of the total. About 58.91% of that
pending upper lies before depth 65 and therefore outside the 36-fixture
integration window of a 101-fixture plan. The remaining 41.09% is inside
the window. Depth 64 is the largest pending bucket, `7.456125832189579e-36`.
Neither run obtains a smaller outer bound from an incomplete expectation
query. Thus relevant GD conditions have been identified, but their bounds
are useful here only after an envelope finishes; unfinished work and cache
coverage still dominate.

Even removing the entire pending upper would leave this run's fixed archive
at `1.1296523521955655e-37`, 7.44 times the retained cohort-9 full upper
`1.5176801956762062e-38`. Further tightening must address the completed
envelopes too, rather than only expanding the pending frontier.

No new Monte Carlo samples or confidence allocation are added. Reuse only
the saved 540-second sampler in `lazy-hybrid-pal16-540s.json`, SHA-256
`9a4cc6173966a6a386684278c30682b840686f6ec28b1a78725fc479014301b0`.
Its cohort-9 sampled core upper is `3.0299741691865805e-41`, from epoch 7,
look 493, 247000 draws and the existing 99% simultaneous allocation.
The clipping definition and actual proposal match.

The new outward core-plus-residual candidate is
`3.676324583763354e-35`. Taking its minimum with the retained full-cohort
upper keeps `1.5176801956762062e-38`. No residual upper is subtracted from
a full-cohort comparator. Retaining the other fifteen cohort contributions
leaves the full-cell interval
`[4.618240572657253e-41, 1.802594383784308e-38]` unchanged.
That interval combines earlier separate sampling and analytical budgets;
it is not the result of one new 180-second run.

The two new allowances total 360 seconds; measured wall time totals
338.204 seconds. The earlier unweighted 180-second comparator had upper
`2.322453949358523e-35`, and the earlier unweighted 540-second audit had
upper `1.306127933964181e-35`. These are historical references, rather than
additional matched treatments of the new selector. The new selector has
only been measured at 180 seconds.

A useful next experiment is to preserve the selected rival's forced-tie
knowledge inside unfinished subproblems, so a conservative unknown state
need not retain spurious strict-PW contribution. This would require a
certified support classification at each relevant cache state. It may let
score bounds tighten before every continuation is integrated. The archive
also needs stronger joint ranking conditions or a check of clipping-grid
slack; additional conditions must retain shared-fixture dependence.

## Source backup and reproduction

Source is backed up on `experiments/rare-odds-recovery-2026-10-17` in commit
`2596fc75f9f9526178140b3a1aa1ad2452362f6d`, tree
`dbc9d0e642853f082c99f994530f02cea42d434c`.
Frozen `/tmp/frontier-recovery-branch-selector-v11` has SHA-256
`da9a7495f859a70a5b9e8b8fa3960e54f61b14a669e9b9994c22cade34f924b9`.
Raw requests and receipts remain local; source, aggregates and hashes are
backed up.

| Local receipt | SHA-256 |
| --- | --- |
| branch-selector-cohort9-control-pal16-180s.json | 0414f85d08c1c2573006da5e919d05b21a39732d7eaeff7778d40968e854c526 |
| branch-selector-cohort9-pal16-180s.json | dcd617f350fdda188d212fa4b3eac658dd86101fcb5d16a8db50f2b3c38dcfeb |

Run from this directory:

```bash
python3 residual_audit.py \
  --binary /tmp/frontier-recovery-branch-selector-v11 \
  --region-id 9 --k 11 --proposal-mode joint \
  --suffix-bounds --suffix-state-cap 4000000 --suffix-max-remaining 56 \
  --suffix-canonicalize-certain --suffix-joint-ratio \
  --shared-expectation --expectation-bits 4 --expectation-max-remaining 36 \
  --score-gd true --expectation-score-gd --expectation-branch-selector \
  --budget 180 --cleanup-reserve 20 --node-limit 100000000 \
  --output branch-selector-cohort9-pal16-180s.json
```

Omit `--expectation-branch-selector` for the matched fixed-selector control.
