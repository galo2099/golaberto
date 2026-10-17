# Cached guide bounds over unfinished completions

This follow-up adds an optional suffix solver to the residual certificate
audit. It targets historical Palmeiras finishing16th, cohort5. The previous
audit expanded36588101 prefixes in177.810 seconds, reached no outer complete
paths and returned residual expectation upper`3.7346398883320757e-33` with
unchanged maximum observation cap`1.335714790055119e-32`. Its guide-ratio lower
bound was zero for every unfinished branch. See
[RESIDUAL_CERTIFICATE_RESULTS.md](RESIDUAL_CERTIFICATE_RESULTS.md).

## Certificate

The suffix solver uses the actual baseline and guided integer-ticket
probabilities. For each state it bounds the minimum future ratio `qg/q0`
over completions satisfying every team's necessary points/wins interval.
It recursively takes the minimum of the current outcome's ratio times its
child's suffix minimum. A feasible terminal contributes1. A proven-impossible
child is excluded. An unresolved child contributes a lower bound of zero and
cannot be treated as impossible.

The memoization key includes immutable plan identity, fixture position,
baseline table node or sticky native fallback, and exact totals of teams with
remaining fixtures. Teams with no remaining fixtures are checked against their
points/wins constraints before their totals can leave the key. Prefix guide
ratio, native weight, mixture ticket and pair factors stay outside the cache.
Only completed suffix proofs are cached; completed descendants survive an
interrupted parent proof and can help later attempts.

The outer search retains its partition. For a prefix with guide lower bound
`G_prefix` and certified suffix minimum `G_suffix`, its residual coefficient
upper is the positive part of
`min(B, V_prefix * suffix_V_cap) - H * G_prefix * G_suffix`.
Products used for subtraction are rounded downward and the residual upper is
rounded upward. An unfinished suffix proof keeps the previous conservative
bound. Necessary PW infeasibility can also prove a branch contributes zero.
Conditional score probability remains bounded by1 in this audit.

The outer search can therefore prune an unfinished branch after completing
its suffix proof, without enumerating all its leaves in the outer frontier.
Internal suffix terminal visits are reported separately from outer leaves.
Proving zero residual does not prove the finishing position impossible:
that region's contribution remains in the hybrid's guided core.

## Budget and comparison

The feature is opt-in. Defaults budget15% of the audit's remaining
time for suffix probes, target25ms and25000 state visits per attempt,
target1000000 retained cache states, and allow at most24 remaining fixtures at a probe
root. Shared deadline and finalization reserve still apply. Child bounds use
cache lookup rather than launching more probes. Final aggregation refreshes
retained branches using completed cached bounds.
Time limits are best effort: an individual guidance operation or recursive
bookkeeping can finish after a local cutoff. The shared wall allowance includes
final aggregation and cleanup, and the harness records any overrun.

The intended comparison runs disabled and enabled modes serially on the same
frozen binary/source, each with a180-second wall allowance, ten-second
finalization reserve, and a100M outer expansion limit. Both prepare only
cohort5 at k9/250000 states and install the compact156-root regime. This is a
fixed-proposal certificate audit, not a full-request A/B timing or a new
statistical interval. There are no simulation draws in either arm.

The primary enabled arm overrides `--suffix-max-remaining 28`. This choice
uses the preceding experiment's frontier, before timing the enabled arm:
at most24 remaining fixtures covers only1.0527% of its retained upper mass,
while28 covers40.2897% and32 covers49.7667%. The28-fixture gate reaches the
largest uncertainty near suffix depths63 through66 with fewer extra fixtures
than32. All other suffix defaults remain as above; the source default stays24.

The longer finalization reserve covers refreshing millions of retained
branches from the suffix cache before returning their conservative aggregate.
Both arms use the same reserve. Compilation finishes before timing.

## Validation and frozen implementation

All51 optimized Rust tests and56 Python experiment tests pass. The new tests
compare a nonconstant guided suffix minimum against exhaustive feasible-path
replay, prove zero residual before any outer leaf, and check cache isolation,
finalized-team rejection, prefix-factor reuse, interrupted-work retention and
overflow fallback. Independent source review found no outstanding blockers.

Frozen binary SHA256:
`89318c69db2a815533642655d6bdfdb9fb9b744a5f01138b33a0137d79b31978`.
Source backup commit:
`cc4c2e2c0783c5a6da8ab3d247d6324d7ac4be85`.
Frozen source aggregate SHA256:
`139f7ce1e00d691c2c01d46858e8c41e6349543c527b0028a2e5ff31c61d10cc`.

The smoke uses a20-second search allowance inside a30-second wall allowance.
It finishes in21.801 seconds, with2987182 outer expansions, no errors and
unchanged hashes. It reaches depth67 among pending prefixes but does not yet
pop an eligible suffix root, so its suffix probe count is zero. This validates
dispatch and budget behavior, not a real-fixture suffix-bound improvement.

## Matched180-second results

| Mode | Wall seconds including setup | Outer expansions | Pending branches | Deterministic residual expectation upper | Maximum residual observation cap |
|---|---:|---:|---:|---:|---:|
|Disabled control|172.592|33945113|5189477|4.057532709537863e-33|1.335714790055119e-32|
|Cached suffix,28 fixtures|173.270|33388748|5128974|4.1287142721020636e-33|1.335714790055119e-32|

Both runs finish inside their180-second allowance, return conservative
unfinished results, and report unchanged input/source/binary hashes and no
errors. They install the same156-root proposal with the same`B` and`H`.
The fresh control is the appropriate comparison; the preceding audit used a
shorter finalization reserve and had more search time.

The enabled upper is1.754% worse than the control, with1.639% fewer outer
expansions. Its maximum residual observation cap does not improve. The upper
remains about5856x above the prior residual statistical upper
`7.049822405827536e-37`; intersecting these bounds would leave the statistical
interval unchanged. There is no qualification or coverage gain demonstrated.
The feature is not enabled in ordinary sampling, the hybrid request sampler,
or production.

## What the cache accomplished

| Suffix diagnostic | Enabled result |
|---|---:|
|Probe calls|2648289|
|Calls returning an unresolved root|2648289|
|New state visits|1001606|
|Completed cached states|1000000|
|Cache hits, including recursive hits|1122715|
|Cumulative probe seconds|4.087|
|Positive lower bounds returned for probe roots|0|
|Outer branch bounds tightened by suffix proofs|0|
|Guide-based zero-residual prunes|0|

The cache reaches its1M-state capacity. Completed descendants are reused, but
none of the actual probe calls finishes its root proof. Every unresolved root
therefore retains a zero guide-ratio lower bound. Cached descendants yield no
measured outer-bound tightening or guide prune. Later eligible probes mostly
return unresolved immediately because the cache is full. The configured15%
work allowance is not exhausted: only4.087 seconds are recorded inside probes.
That timer excludes ordinary cache lookup and final refresh overhead.

The aggregate time-limit flag is true because individual25ms attempts can
expire; it does not establish exhaustion of the cumulative work allowance.
The per-probe25000-new-state limit is not hit. The separate native marginal
PMF and outer-frontier capacity limits are not hit either.

The solver records878114 internal PW-feasibility rejections. These do not
represent additional outer branches pruned or additional cells proved zero.
It creates one distinct cached feasible terminal state; terminal cache hits
can represent many completions, so this counter is not a count of complete
campaigns visited. Both outer searches still reach zero outer leaves. The
enabled outer search records5218 PW-impossible prunes versus5608 in the control.

This is a negative result for this configuration, not a rejection of guide
certificates in general. The small tests demonstrate the mechanism, while
the real fixture exposes the state-capacity cost of exact active-team keys.

## Next state reduction to test

A team can still have remaining fixtures even when its PW requirement is
guaranteed under every remaining allowed outcome. Its exact points/wins then
may distinguish cache keys without changing future guidance. A useful next
experiment would canonicalize such teams to a separate "PW certain" state.
It must prove certainty over the endpoint PMF support using safe integer
arithmetic, preserve actual full-row floating sums instead of substituting1,
and keep plan/node/fallback identities. This optimization is not implemented
or measured here.

## Reproduction and local artifacts

From this directory, after building the frozen implementation:

```bash
python3 residual_audit.py --binary /tmp/frontier-recovery-suffix-v4 \
  --budget 180 --cleanup-reserve 10 --node-limit 100000000 \
  --output suffix-bound-control-pal16-180s.json
python3 residual_audit.py --binary /tmp/frontier-recovery-suffix-v4 \
  --suffix-bounds --suffix-max-remaining 28 \
  --budget 180 --cleanup-reserve 10 --node-limit 100000000 \
  --output suffix-bound-pal16-180s.json
```

Raw JSON stays local; source and this aggregate report are backed up.

```text
suffix-bound-smoke-pal16.json         26aa64a369bbf509fe31c6287fee386b9f5d5a34283cc5efecd28c75f1a4d47c
suffix-bound-control-pal16-180s.json   ecc8c1e8a96bbfee4b99408083bcd7680813f3d75b902fdb6981215c72f6ee27
suffix-bound-pal16-180s.json           32f9ce2ad3818d7b857f1da9cda7869b4db2a7cde72957411f7028433867fe19
```
