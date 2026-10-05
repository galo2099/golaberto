# R58: Learn rival families after native confirmation fails

Date: 2026-10-03. Status: completed isolated experiments. Production source
and defaults are unchanged; no commit, push or database mutation.

## Results

Preserving native confirmation and applying a learned-family fallback recovered
Flamengo/12th in **5/5 fixed-seed runs**, versus **3/5 for current Rust**.
Every baseline-positive cell retained its probability and statistical evidence.
This is **one distinct team/rank in one saved group**, with two additional
positive cell-runs, not two new distinct cells or five groups.

The two recovered probabilities are `4.11e-26` and `2.47e-26` as fractions.
Larger native runs in R57 put this cell near `1e-25`. These fallback estimates
are useful for rough magnitude but are 3.2 and 4.8 times below the corresponding
R57 native 10x values (`1.314e-25` and `1.189e-25`). There is no exact reference
probability for the complete cell.

The original deterministic selected-cell grant funds both native work and the
fallback. That does **not** establish unchanged latency. Three alternating
warm timing pairs per eligible seed measured mean request wall increases of
**47.6 ms / 3.60%** for 808 and **61.7 ms / 4.22%** for 2293. Four Rust workers
were used. CPU deltas were noisy; this adds actual operations and is not a
demonstrated CPU optimization. No additional production allowance is assumed.

An equal-grant native retry recovers neither failed run. With tenfold leftover
grants, both native retries and a capped family learner reach values near
`1e-25`; allocating much more work to learning fails one publication gate.
The broader screen also recovers Palmeiras/15th in one run at `1.35e-33`.
The first shared-bank experiment does **not** retain that extra gain: failed
attempts consume capacity before it is considered. It preserves the two
Flamengo gains but costs substantially more than selecting Flamengo alone.
A smaller learning cap subsequently raises gains to **four cell-runs**, with
Flamengo and Palmeiras gains coexisting in the same request. Native ordering
gains three distinct team/ranks across the cohort; grant ordering gains two.
Both reduce reachable zeros from 15 to 11 across the five matrices. These are
nonzero outputs passing the current gates, not four newly proved positions.

Do **not** enable the broader version yet. Warm timings add **12–26% wall time**,
and larger runs expose instability in Palmeiras/15th's `1e-33` estimates.
The most useful next experiment is to audit its dominant weighted seasons and
identify whether the missing mass is in a root, a rival family, or scores within
an already learned family. Recording during funded native work may separately
remove replay cost, but is not implemented here.

## Baseline and experiment protocol

- Production HEAD: `4e812fd506b39a504dc3f771c26e8d027cff2311`.
- Baseline: current Rust rebuilt from clean production source; binary SHA256
  `6084a83dabd40d34c69e4b8b341b785221249986f1ec5302b097978533b39e80`.
- Request: saved group 16498, SHA256
  `2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625`.
- Twenty teams, 103 remaining fixtures, sort specification
  `pt,w,gd,gf,gp,name`. The model maps the unrecognized final `name` token to
  its existing random tie rule. Actual production goal rules/sorter are used.
- Fixed seeds: `808,1669,1993,2281,2293`. Selector `17:12` identifies the
  experimental cell; algorithm internals have no named-team/rank special case.
- Four workers, with estimator processes, builds and tests serialized.
- Clear inherited `RUST_ODDS_*` and `RARE_POSITION_*` flags before each run.
- Full matrices, game importance, stage timing, process wall and child
  user/system CPU are saved. These are full-request CLI experiments, including
  startup, decode and export, rather than direct HTTP latency measurements.
- Local platform is macOS arm64, Rust 1.93.1. Production Intel Xeon timing was
  not measured. No CPU-specific optimization was introduced.

R58 starts from the isolated R57 family implementation. With experiment flags
absent, seed 808's complete export is byte-identical to current Rust. Changes
are in a temporary crate, with portable source archives and patches saved under
`2026-10-03-family-fallback/`. Production `odds-rust/` is unchanged.

## Preserve the funded native search first

R57 replaced a native proposal before final confirmation and lost an existing
estimate on seed 1669. R58 runs the current native confirmation schedule first:

1. Keep the original first-wave pairs and speculative mains, batch sizes and
   random streams.
2. Refund skipped checks at the existing deterministic barrier.
3. Make the original second-wave admissions and run their samples.
4. After both waves finish, consider only the selected, originally paired Lazy
   proposal whose native **main** fails publication and whose cell is still zero.

A publishable main with a failed independent check is outside this experiment's
eligibility. An unfunded or speculative-only job is also ineligible. Successful
native runs incur no family recording, replay, cloning or fitting work.

Let `G` be the original selected-cell main/check grant, `A` its actual native
operation cost and `R` the bank's unallocated capacity after the native waves:

```text
fallback_grant B = min(max(G - A, 0), R)
```

Reserve `B` in that bank before starting the fallback. Work already used to
fund another job cannot fund this attempt. Settle unused grant afterward.
Checks skipped by the fallback can save work but do not schedule new native
jobs. This preserves current native admissions and random streams.

## Charged learning and frozen proposals

Replay an affordable prefix of the failed native main using its **original
seed**. This reproduces its outcomes without adding random draws. It still
performs duplicated sampling work, which is charged fully.

For positive observations, accumulate corrected contribution sums keyed by the
target's full outcome sequence and every rival's below/equal/above status on
points and applicable wins. Retain at most 128 keys and fit the four largest
root/family pairs. No R56 final results, reference estimates or hand-selected
Flamengo families are supplied to learning.

R57's construction and weighting remain unchanged:

- Compile complete family statuses into shared fixture constraints for all
  rivals, using points and applicable wins.
- Use 20% primary native proposal and 80% learned-family proposal for learned
  roots. Unlearned/uncached roots keep existing behavior.
- Evaluate the full mixture density by deterministic replay. Corrected weights
  include target-root selection, original prior, pattern normalizers and the
  existing goal-tilt correction.
- Keep actual goal assignment, tie rules and production sorting. A points/wins
  tie is not assigned an arbitrary rank.
- Freeze the fit before independent validation/main/check streams. Replay and
  validation observations are never included in the reported mean.

See R57's derivation and tests in
`2026-10-03-rust-family-conditioning.md`. Learning mixture rates is also studied
by [He and Owen, *Optimal mixture weights in multiple importance sampling*](https://arxiv.org/abs/1411.3954).
R58 uses contribution proportions, not their weight-optimization procedure.

## Allocation revisions

All replay operations, observation collection, cloning, attempted family setup,
validation, final sampling and proposal-density replay are charged. Setup is
bounded by 10,000 additional nodes and four million guide values; its charge is
16 units/node plus two units/guide value. A partial fit retains completed
components and renormalizes their rates. Exhaustion does not prove impossibility.

The cost model is `8 × fixtures + guidance + ranking`. These are logical work
units, not CPU cycles. The legacy request-wide `work_spent` field uses a
different convention, so this report compares explicit operation counters and
grants rather than treating that field as interchangeable with them.

- **V1:** up to 5,000 replay draws, spending at most half of the available grant;
  validation did not protect the minimum final-pair reserve.
- **V2:** cap replay before sampling to leave clone, bounded setup, at least 100
  validation draws and at least 1,000 final draws per stream. Protect that final
  reserve while choosing validation length. Validation ESS had to reach 1.5.
- **V3:** same V2 allocation, but validation measures draw cost only. It still
  needs at least 100 draws and must stay within the conservative operation bound.
  Its noisy ESS no longer blocks a valid constructed proposal. **Independent
  final publication gates are unchanged.**

The conservative validation bound is 110,746 units/draw for this request.
Final lengths use validation's measured draw cost, 8% headroom and a 100,000
draw ceiling per stream. Publication requires actual work within the settled
grant as well as the native main/check quality gates.

Measured average costs and headroom do not provide a hard cap on actual work.
An overrun rejects publication but cannot undo spent CPU. None of the V3
attempts exceeded their grant. This remains an experimental allocation rule.

## Paired V3 results at the original grant

Probabilities below are fractions; rank numbers are one-based.

| Seed | Current Rust, Flamengo/12th | Deferred family fallback | Result |
| ---: | ---: | ---: | --- |
| 808 | 0 | 4.11210e-26 | Recovered |
| 1669 | 1.61028e-25 | 1.61028e-25 | Native retained; no fallback work |
| 1993 | 1.03005e-25 | 1.03005e-25 | Native retained; no fallback work |
| 2281 | 1.10005e-25 | 1.10005e-25 | Native retained; no fallback work |
| 2293 | 0 | 2.46575e-26 | Recovered |

Every baseline-positive estimate's probability, evidence, uncertainty, design
and reachability were preserved. Only `work_spent`, which is copied from the
request total into each estimate, changes when the fallback spends work. Game
importance is identical. Only Flamengo/12th changes other estimate fields on
808 and 2293. Flag-off seed 808 is byte-identical to current Rust.

| Seed | Replay draws / hits / ESS | Built families | Validation draws / ESS | Main/check draws each | Main/check hits | Main/check ESS | Main/check probability |
| ---: | --- | ---: | --- | ---: | --- | --- | --- |
| 808 | 4,232 / 178 / 4.68 | 4 | 166 / 1.17 | 2,058 | 217 / 192 | 15.83 / 7.39 | 4.11210e-26 / 7.25786e-26 |
| 2293 | 1,988 / 78 / 5.69 | 3 | 106 / 2.68 | 1,751 | 177 / 176 | 21.55 / 10.69 | 2.46575e-26 / 4.23453e-26 |

V2 declined seed 808 solely because the 166-draw validation had ESS 1.17.
V3 uses it for cost calibration and the fresh final pair passes unchanged gates.
For 2293, setup declines the fourth family after its budget is exhausted;
the three completed components remain usable.

| Seed | Native actual | Fallback grant | Fallback actual | Native + fallback actual | Original cell grant | Overrun |
| ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 808 | 75.051M | 72.070M | 65.255M | 140.306M | 147.120M | No |
| 2293 | 45.342M | 47.057M | 43.365M | 88.707M | 92.399M | No |

The complete matrices have 17 impossible zeros and no undecided cells in each
run. Remaining reachable zeros change from `3,4,2,3,3` to `2,4,2,3,2` in seed
order. These are probability-estimation gains; reachability itself does not
change. Across 2,000 matrix cell-runs, positives increase from 1,900 to 1,902.

## Timing: original-grant fallback

Initial cold runs varied enough to obscure the small added stage. V1's first
seed-808 request measured +19.0% wall and +7.1% CPU; later V2 had an even larger
startup outlier. The flag-off export remained identical. These measurements
are saved, not discarded or treated as a production budget.

For V3, run three alternating baseline/fallback pairs per eligible seed and
verify every complete export hash against the corresponding saved V3 result.

| Seed | Mean baseline wall | Mean fallback wall | Mean wall increase | Median paired increase | Mean CPU delta | Median CPU delta |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 808 | 1.3233 s | 1.3708 s | 47.6 ms / 3.60% | 35.7 ms | −26.8 ms | −63.0 ms |
| 2293 | 1.4604 s | 1.5220 s | 61.7 ms / 4.22% | 31.1 ms | +58.1 ms | −11.3 ms |

The added fallback stage itself takes approximately 61 ms for 808 and 41 ms
for 2293; replay accounts for about 34 and 17 ms. Individual CPU deltas change
sign and the sample is small. There is no evidence that fallback reduces CPU.
It adds a serialized tail after native waves, so request latency preservation
has **not** been achieved.

## V4: native retry and tenfold learning controls

The native retry uses the same frozen native plan with fresh independent
main/check labels. It needs no replay, clone, fit or new validation: its native
main already supplies a measured draw cost. Give it the same leftover grant
`B`, allocate 92% of it to a pair using that cost, and retain the same quality
and actual-settlement gates. This is an end-to-end allocation comparison, not
equal final draw counts.

The separate 10x experiment expands **only the leftover fallback grant**:

```text
expanded fallback grant = 10 B
isolated added allowance = 9 B
experimental cell limit = A + 10 B
experimental bank limit = original bank limit + 9 B
```

Add that allowance only after native waves finish. Thus native admissions and
successful estimates remain unchanged. This is not tenfold request work, or
ten times the original cell pair, and is not a production allowance. Overflow
declines the expansion before bank mutation or sampling.

The family 10x control caps learning at 5,000 draws. Its larger-learning arm
allows up to 50,000 draws, still limited by protected setup/validation/final
reserves. Training can extend the original main stream; report its replayed
prefix separately from genuinely new training draws. Both are fully charged.
Fit and final draw ceilings are unchanged.

The 22 serialized V4 runs comprise four arms over all five seeds and two
family-1x parity repetitions. No arm changes the three successful native runs.
Every attempt settles within its grant; all baseline-positive estimate fields
apart from request-total `work_spent`, and game importance, are preserved.

| Arm | 808 probability | 2293 probability | Flamengo/12th coverage across five seeds |
| --- | ---: | ---: | ---: |
| Current Rust | 0 | 0 | 3/5 |
| Native retry, 1x leftover | 0 | 0 | 3/5 |
| Family fallback, 1x leftover | 4.11210e-26 | 2.46575e-26 | 5/5 |
| Native retry, 10x leftover | 1.39488e-25 | 1.52002e-25 | 5/5 |
| Family 10x, 5k learning cap | 1.11254e-25 | 1.53872e-25 | 5/5 |
| Family 10x, 50k learning cap | 1.11604e-25 | 0 | 4/5 |

Both 1x native retries fail main quality: ESS 2.61/2.23, maximum contribution
share 60.4%/65.5%, relative SE 61.9%/66.9%. Their checks are skipped. These
are **quality failures, not budget overruns**. Family conditioning materially
helps within the same available grant.

| Arm | Seed | Learning draws | Final draws/stream | Actual fallback work | Unused grant | Full wall | User + system CPU |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Native retry 1x | 808 | 0 | 3,894 | 32.439M | 39.631M | 1.889 s | 3.481 s |
| Native retry 1x | 2293 | 0 | 2,471 | 21.732M | 25.325M | 1.566 s | 3.658 s |
| Family 1x | 808 | 4,232 | 2,058 | 65.255M | 6.815M | 1.392 s | 3.222 s |
| Family 1x | 2293 | 1,988 | 1,751 | 43.365M | 3.692M | 1.601 s | 3.708 s |
| Native retry 10x | 808 | 0 | 38,942 | 653.751M | 66.948M | 1.991 s | 3.836 s |
| Native retry 10x | 2293 | 0 | 24,712 | 430.999M | 39.569M | 1.919 s | 3.824 s |
| Family 10x, cap5k | 808 | 5,000 | 44,709 | 662.303M | 58.396M | 1.958 s | 3.925 s |
| Family 10x, cap5k | 2293 | 5,000 | 25,095 | 420.967M | 49.600M | 1.840 s | 3.724 s |
| Family 10x, cap50k | 808 | 42,329 | 12,160 | 682.626M | 38.073M | 1.976 s | 3.945 s |
| Family 10x, cap50k | 2293 | 26,861 | 9,641 main; check skipped | 351.375M | 119.192M | 1.834 s | 3.853 s |

These are single process timings with substantial startup/host variability;
the paired warm V3 repetitions above are the better original-grant latency
measurement. Expanded arms add hundreds of milliseconds of fallback work.
They do not establish unchanged production latency.

### What the larger learner teaches

For 808, learning expands to 42,329 draws: 8,817 replayed and 33,512 new.
It learns several material families rather than concentrating approximately
99% on one. Main ESS improves to 71.69, relative SE to 11.8%; check ESS is
18.75. This is evidence that learning more joint structure can help.

For 2293, 26,861 learning draws comprise 5,177 replayed and 21,684 new.
The final main has **2,591 hits but ESS only 2.30**. One observation carries
65.1% of its contribution and relative SE reaches 65.9%. The raw mean is
`4.119e-25`; it fails the existing gate, so the check is skipped and no estimate
is published. All four fitted components were built and work remained within
grant. Learning consumed work that could otherwise fund final draws, while
the fitted proposal still had a material weighted tail. Do not adopt the
larger learning allocation by default.

## Breadth: two other baseline-zero cells

Select Palmeiras/team 16 at ranks 14 and 15 from baseline failed-main zeros,
before examining their family results. Test family/native retry at 1x for
both selectors over all five seeds: 20 serialized full requests.

| Selector | Baseline zero runs | Eligible attempts per mode | Family gains | Native retry gains |
| --- | ---: | ---: | ---: | ---: |
| Palmeiras/14th | 4/5 | 3 | 0 | 0 |
| Palmeiras/15th | 5/5 | 2 | 1 | 0 |

The new Palmeiras/15th estimate is `1.34571e-33` on seed 2293. Its native
actual cost is 115.379M, leftover grant 113.673M, fallback actual 105.250M,
and original cell grant 229.051M. It uses 5,000 training draws and 2,549 final
draws per stream. There is no larger-run reference here establishing its
accuracy. Every breadth attempt settles within grant, all baseline-positive
cells are preserved, and game importance is identical.

For Palmeiras/14th, seed 1993 has only 11.262M free bank capacity, so both
methods decline before sampling. The other eligible runs fail final quality.
An absent fallback event means there was no eligible selected originally
paired failed-main zero proposal; it does not establish impossibility.

These individual screens yield three additional positive **cell-runs across
two distinct identities** when combined as a list of experimental outcomes.
They do **not** show both estimates in one request: Flamengo and Palmeiras
compete for the same spare bank. The all-scope screen below measures that
competition rather than assembling a synthetic matrix from separate runs.

## V5: all eligible cells sharing one bank

After unchanged native waves, visit all originally paired, failed-main Lazy
zeros in their existing job priority. Reserve each cell's current
`min(G - A, bank_free)`, settle every charged attempt, and refund unused work
before the next cell. No extra allowance is added. All-scope rejects a 10x
multiplier rather than granting expanded work. Selected-scope remains the V4
default, with exact export parity.

The family arm recovers only the same two Flamengo/12th runs as V3. The native
retry arm recovers none. No baseline-positive estimate or game importance
changes, and no measured fallback overruns occur. Remaining reachable zeros
stay `2,4,2,3,2` for the family arm. **Palmeiras/15th is not recovered in the
combined seed-2293 export.**

| Seed | All-family added actual work | New positive cell-runs | Native confirmation plus fallback actual | Native bank limit |
| ---: | ---: | ---: | ---: | ---: |
| 808 | 273.449M | 1 | 955.352M | 1,019.220M |
| 1669 | 102.523M | 0 | 1,054.222M | 1,096.321M |
| 1993 | 0 | 0 | 1,010.963M | 1,010.986M |
| 2281 | 77.149M | 0 | 928.711M | 1,012.221M |
| 2293 | 104.513M | 1 | 981.925M | 1,015.053M |

These are confirmation-bank operations, not all request operations. Actual
native costs already differ from native reservations in current Rust; the
experiment retains that accounting. All five measured actual confirmation
totals fit the bank limit, but a reservation is not a universal hard cap.

Seed 2293 illustrates the allocation problem:

1. Flamengo/12th receives 47.057M and spends 43.365M successfully.
2. Palmeiras/13th receives the remaining 88.708M. Learning, fit and validation
   spend 48.857M, but cannot fund the minimum fresh pair.
3. Palmeiras/15th receives only 39.851M. It spends 12.291M and also declines
   before final sampling. Its independent selected-cell experiment had
   113.673M available and spent 105.250M successfully.

The two previously passing schedules would spend 148.615M together, exceeding
the 132.073M spare bank. Reordering alone cannot reproduce both schedules;
it must find a cheaper passing allocation or trade one gain for another.

Three alternating warm timing pairs per seed measure mean all-scope wall
increases of **322.0 ms / 23.48%** for 808 and **213.2 ms / 14.46%** for 2293,
with mean CPU increases of 327.4 and 345.1 ms. Mean full-request wall changes
from 1.3711 to 1.6931 s and 1.4742 to 1.6874 s. This has no coverage advantage
over the selected-cell fallback. **Do not adopt V5's allocation.**

Those timing logs refer to the immutable initial V5 revision. A subsequent
guard charges each final main immediately and skips its check if the main has
already exceeded its grant; an overrun rejects publication and stops further
all-scope attempts. All 13 guarded cohort exports exactly match that initial
revision, and every attempt's recorded component costs sum to its charged
total. The guard cannot undo the cost of an overrun main.

## V6: separate learning from ordering

Keep the native schedule and total bank unchanged. Compare three all-scope
1x arms over the same five seeds:

- Order eligible fallbacks by full original unused cell grant, ascending, with
  stable original-priority ties; retain the 5,000-draw learning cap.
- Keep native fallback priority but cap learning at 3,000 draws.
- Combine that ordering and learning cap.

Order before clamping grants to current bank capacity. Using clamped grants
would make competing proposals appear equally cheap when the bank is scarce.
This changes fallback admissions only; probability weighting and fresh final
publication gates remain unchanged. Fund the ordering scan/sort in advance
from the same spare bank, charging
`16 × (2 × J + J × ceil(log2(J + 1)))` for `J` original jobs. It is a logical
setup model, not a CPU-cycle bound. Setup costs 400 or 480 units in this cohort.
The native-order path has no sorting fee; minor scheduling scaffolding is
included in measured latency.

The final 15-run screen has no overruns, correct component/bank ledgers,
identical game importance and no baseline-positive estimate changes apart
from request-total `work_spent`. Its complete exports and operation ledgers
match the initial screen. Native-order/cap5k exports and native counters match
guarded V5. V6 also corrects a diagnostic reservation snapshot: the native
reservation after both waves is distinguished from the current reservation
immediately before each fallback. This does not change grants or estimates.

| All-scope arm | New cell-runs vs current Rust | Distinct gained team/ranks | 808 gains | 2293 gains |
| --- | ---: | ---: | --- | --- |
| Native order, cap5k (V5) | 2 | 1 | Flamengo/12 | Flamengo/12 |
| Grant order, cap5k | 2 | 1 | Flamengo/12 | Flamengo/12 |
| Native order, cap3k | 4 | 3 | Flamengo/12, Palmeiras/15 | Flamengo/12, Palmeiras/13 |
| Grant order, cap3k | 4 | 2 | Flamengo/12, Palmeiras/15 | Flamengo/12, Palmeiras/15 |

Seeds 1669, 1993 and 2281 gain no cells. Both cap3k arms change remaining
reachable zeros to `1,4,2,3,1`, and matrix positives increase from 1,900 to
1,904 across 2,000 cell-runs. Each arm is a complete five-request cohort;
their different Palmeiras gains must not be added into a synthetic matrix.

| Arm | Seed | New probability (fraction) | Total added fallback operations |
| --- | ---: | --- | ---: |
| Native order, cap3k | 808 | Flamengo/12 `3.45102e-26`; Palmeiras/15 `9.99971e-34` | 274.847M |
| Native order, cap3k | 2293 | Flamengo/12 `2.46575e-26`; Palmeiras/13 `6.97249e-21` | 128.751M |
| Grant order, cap3k | 808 | Flamengo/12 `3.45102e-26`; Palmeiras/15 `9.99971e-34` | 274.847M |
| Grant order, cap3k | 2293 | Flamengo/12 `2.46575e-26`; Palmeiras/15 `1.26556e-33` | 123.769M |

In the joint cap/order arm on 2293, Flamengo still spends 43.365M. Palmeiras/15
then learns from 3,000 draws, uses 2,243 independent draws per final stream,
and spends 80.404M. Main/check ESS are 6.96/4.04, relative SE 37.9%/49.7%,
with probabilities `1.266e-33`/`9.682e-34`. Both estimates coexist within the
132.073M spare bank. Palmeiras/13 declines before work when only 8.304M remains.
This actually supplies the cheaper passing allocation that V5 lacked.

Ordering alone supplies no extra coverage. Cutting learning shifts work toward
fresh final sampling and sometimes also changes the learned mixture. It does
not uniformly save operations: seed 1669 spends more final work and still
fails quality. The larger controls below test Palmeiras/15's rough magnitude
and Palmeiras/13 on 2293. The latter's new `6.972e-21` is 6–10 times below
other seeds' accepted native values (`4.534e-20`, `6.917e-20`, `7.105e-20`);
those are comparison estimates, not exact references.

### Warm full-request timing

Run three alternating baseline/native-order/grant-order triplets per seed,
18 processes total, all serialized with four workers. Every export matches
the frozen corresponding cohort hash.

| Seed | Arm | Mean full wall | Increase vs current Rust | Mean CPU increase |
| ---: | --- | ---: | ---: | ---: |
| 808 | Current Rust | 1.4086 s | — | — |
| 808 | Native order, cap3k | 1.7681 s | 359.5 ms / 25.52% | 501.2 ms / 15.09% |
| 808 | Grant order, cap3k | 1.7722 s | 363.6 ms / 25.81% | 565.2 ms / 17.02% |
| 2293 | Current Rust | 1.5495 s | — | — |
| 2293 | Native order, cap3k | 1.7312 s | 181.7 ms / 11.73% | 297.0 ms / 8.26% |
| 2293 | Grant order, cap3k | 1.7447 s | 195.2 ms / 12.60% | 283.7 ms / 7.89% |

The spare deterministic bank limits are respected in the measured attempts,
but current Rust leaves that capacity unused. Spending it adds actual CPU and
a serialized tail. Neither cap3k arm satisfies the latency-preservation
constraint. Small timing differences between their orderings are not reliable
enough to choose one for speed.

### Six selected 10x controls: coverage is not calibration

These expand only the selected cell's leftover grant after unchanged native
waves. They are separate learning experiments, with explicit added allowance,
not candidate production budgets. All six settle within their expanded grants.

| Cell | Seed | Mode | Final draws/stream | Raw main / check probability | Main / check ESS | Published |
| --- | ---: | --- | ---: | --- | --- | --- |
| Palmeiras/15 | 808 | Family, cap3k | 95,537 main; check skipped | 1.587e-31 / — | 1.02 / — | No |
| Palmeiras/15 | 808 | Native retry | 100,000 main; check skipped | 1.882e-31 / — | 2.29 / — | No |
| Palmeiras/15 | 2293 | Family, cap3k | 44,403 each | 5.454e-33 / 1.299e-31 | 14.33 / 2.90 | No |
| Palmeiras/15 | 2293 | Native retry | 60,429 main; check skipped | 1.688e-31 / — | 1.75 / — | No |
| Palmeiras/13 | 2293 | Family, cap3k | 27,859 each | 6.451e-20 / 6.020e-20 | 7.54 / 14.04 | Yes |
| Palmeiras/13 | 2293 | Native retry | 40,990 each | 6.655e-20 / 5.512e-20 | 24.69 / 62.27 | Yes |

Palmeiras/13's two accepted larger means agree near `6.5e-20`, making the
small-budget `6.972e-21` about 9.3–9.5 times lower. This supports an extremely
small probability, with a substantial small-budget underestimation.

Palmeiras/15 is more concerning. The larger 808 family main has 13,063 hits,
but **one contribution accounts for 99.1%** of its mean. Native retry's maximum
share is 65.3%. On 2293, family main/check disagree by about 24 times, and the
native main's maximum share is 75.1%. All four controls fail quality.

These are not reliable `1e-31` reference estimates and do not prove the true
probability is 100 times larger. They do show a weighted tail that the smaller
accepted `1e-33` batches missed. **Palmeiras/15's order of magnitude remains
unvalidated.** Increasing samples can expose that tail and turn an accepted
small-batch output back into zero. Thus the four additional nonzero cell-runs
should not be presented as four validated probability improvements.

## Correctness and remaining limits

- Relaxation feasibility and learned family construction are proposals, not
  reachability proofs. Every probability observation uses actual sorting.
- Unknown/omitted families remain supported by the primary native component.
  Positive support alone does not ensure low variance or adequate discovery.
- Failed setup, missing pilot hits and budget exhaustion leave estimation
  unresolved; they do not turn a reachable cell into an impossible one.
- Learning is conditioned on native failure. With a frozen fit and independent
  final streams the usual importance identity still applies. Data-dependent
  publication gates do not make published endpoint outputs unbiased, and a
  reported standard error cannot certify the mass of an unseen family.
- Both successful V3 fits mostly learn one leading family. The larger native
  experiment sees another material family. This explains why the new estimate
  can have acceptable observed diagnostics while remaining below the larger-run
  reference. This experiment demonstrates recovery and rough magnitude.
- The Palmeiras/15 controls show a stronger version of that limitation. No
  season-level audit here identifies the dominant weight's cause, so attributing
  it specifically to an omitted family would be an untested inference.
- Eligibility is deliberately narrow: it cannot help a cell lacking an
  originally paired failed native main, or one whose main/check disagree.
- No conclusion about coverage across groups 16653 or 16982 follows from this
  single-group follow-up. Their production behavior is unchanged.

## Artifacts and checks

Artifact root: `experiments/rare_positions/2026-10-03-family-fallback/`.

- `logs/v1-seed808`, `logs/v2-cohort`, `logs/v3-cohort`: raw full exports, stage
  logs, command/environment records, process wall/user/system CPU and hashes.
- `logs/v3-timing`, `data/v3-timing-analysis.json`: paired warm repetitions.
- `logs/v5-screen`, `logs/v5-timing`: immutable initial V5 measurements;
  `logs/v5-final-screen` records the guard correction and exact parity.
- `logs/v6-screen`, `logs/v6-timing`, `logs/v6-reference-controls`,
  `data/v6-screen-analysis.json`, `data/v6-followup-analysis.json`: final
  allocation cohort, 18 warm timings and six expanded reference controls.
  Initial V6 measurements are preserved under `logs/v6-screen-pre-final`.
- `data/v3-cohort-analysis.json`, `data/v4-cohort-analysis.json` and
  `data/v4-breadth-analysis.json`: matrix preservation, full diagnostics and
  grant settlement.
- `data/r58-source-v1.tar.gz`, `data/r58-source-v2.tar.gz`,
  `data/r58-source-v3.tar.gz`, `data/r58-source-v4.tar.gz`: immutable portable
  source snapshots, including
  the sibling rating core and required compile-time test fixture.
- `data/r58-source-v5-final.tar.gz`, `data/r58-source-v6.tar.gz`: guarded V5
  and final V6 source/harness snapshots; earlier V5 remains separately frozen.
- `patch/r58-v*.patch`, `data/provenance-v*.json`, source manifests and versioned
  binaries: source/build/input identities for each measured revision.
- `scripts/run_screen.py`, `run_cohort.py`, `run_v3_cohort.py`,
  `run_timing_repeats.py` and analyzers: reproducible serialized experiment harnesses.

V1 passed all 96 Rust tests. V3 passed formatting/check/release build and four
focused fallback tests, including replay/funding guards, weak cost-validation
admission with unchanged final rejection, and validation operation overrun
rejection. V4 passes all 99 tests, formatting/check/release build and flag-off
parity; family1x matches V3 exactly for both recovered seeds. Independent Sol
review found no material weighting/gating blocker. V4 source archive SHA256 is
`2950df49b84218145965fdfffec6411e3a944615970d338d426c11e57b60a034`;
all 64 member sizes/hashes are verified. Its patch is incremental from R57's
reallocated isolated source, not directly from production HEAD. Use the full
portable archive to reproduce the measured code.

Final V6 passes all **104 Rust tests**, ten focused fallback tests,
formatting/check and release build. Independent Sol review finds no material
weighting, admission or settlement blocker. Source archive SHA256:
`2f1ef12463f6d75053268010183620e4f50f88ce321f5140f6d4489b42187b63`;
binary SHA256:
`93d24dde2a152f1f4bfa9209013c262fa8082c61983df800f4d6423ccef9f5ee`.
All 158 archive member hashes, the source file and saved binary match the
manifest/provenance. Baseline timing rows use the current Rust binary;
experimental timing/reference rows use this V6 binary.

## Reproduce

Use Rust 1.93.1 and run from the repository root. Extract the portable source
into a fresh temporary directory. Saved controls remain read-only; harnesses
refuse to overwrite an existing result directory.
The source builds independently, but analyzers use recorded comparator paths
in this checkout. Relocating the evidence requires rebasing those paths.

```sh
cd /Users/robsonaraujo/work/golaberto
artifact="$PWD/experiments/rare_positions/2026-10-03-family-fallback"
repro_dir=$(mktemp -d /tmp/golaberto-r58-repro.XXXXXX)
tar -xzf "$artifact/data/r58-source-v6.tar.gz" -C "$repro_dir"
mkdir -p "$repro_dir/current"
git archive 4e812fd506b39a504dc3f771c26e8d027cff2311 odds-rust stats/core \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  | tar -x -C "$repro_dir/current"
cargo build --release --locked -j4 --manifest-path "$repro_dir/current/odds-rust/Cargo.toml"
cargo build --release --locked -j4 --manifest-path "$repro_dir/odds-rust/Cargo.toml"
cargo test --release --locked -j4 --manifest-path "$repro_dir/odds-rust/Cargo.toml" -- --test-threads=1

mkdir -p "$repro_dir/experiment-harness/logs" "$repro_dir/experiment-harness/data"
ln -s "$artifact/logs/v3-cohort" "$repro_dir/experiment-harness/logs/v3-cohort"
ln -s "$artifact/logs/v4-breadth" "$repro_dir/experiment-harness/logs/v4-breadth"
ln -s "$artifact/logs/v5-final-screen" "$repro_dir/experiment-harness/logs/v5-final-screen"

python3 "$repro_dir/experiment-harness/scripts/run_v6_screen.py" \
  "$repro_dir/odds-rust/target/release/golaberto-odds" \
  "$repro_dir/current/odds-rust/target/release/golaberto-odds" \
  "$repro_dir/experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json" \
  "$artifact/logs/v3-cohort" "$artifact/logs/v5-final-screen"
python3 "$repro_dir/experiment-harness/scripts/analyze_v6_screen.py"

python3 "$repro_dir/experiment-harness/scripts/run_v6_timing.py" \
  "$repro_dir/odds-rust/target/release/golaberto-odds" \
  "$repro_dir/current/odds-rust/target/release/golaberto-odds" \
  "$repro_dir/experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json" \
  "$repro_dir/experiment-harness/logs/v6-screen"
python3 "$repro_dir/experiment-harness/scripts/run_v6_reference_controls.py" \
  "$repro_dir/odds-rust/target/release/golaberto-odds" \
  "$repro_dir/experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json" \
  "$repro_dir/experiment-harness/logs/v6-screen"
python3 "$repro_dir/experiment-harness/scripts/analyze_v6_followups.py"
```

Each harness clears inherited odds flags and runs
`golaberto-odds estimate REQUEST EXPORT SEED 4`. The capped native-order arm uses:

```text
RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK=1
RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE=all
RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE=family
RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP=3000
```

Grant ordering adds `RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_ORDER=grant`.
Selected controls use `RUST_ODDS_EXPERIMENT_NEIGHBOR_TRANSFER=native`,
`RUST_ODDS_EXPERIMENT_NEIGHBOR_CELL=16:15` or `16:13`, and multiplier 10.
These are archived experiment flags, not current production options.

## Recommendations

| Strategy | Decision |
| --- | --- |
| Deferred family fallback after unchanged native confirmation | Retain as the strongest proposal direction: Flamengo/12th improves from 3/5 to 5/5 with no native losses. Solve its 3.6–4.2% latency increase before production integration. |
| Equal-grant native retry | Reject for these failed runs: no coverage gain. |
| Much larger learning allocation | Reject as a default: spends final-sampling capacity and loses one Flamengo run. |
| Native-priority all-scope/cap5k | Reject: no extra coverage over selected fallback, substantially more latency. |
| Grant ordering alone | Reject: no extra coverage. |
| Cap3k with either ordering | Retain for further experiments: four nonzero cell-runs, but 12–26% added wall and unvalidated Palmeiras/15 magnitude prevent production adoption. Native order covers more distinct identities in this cohort. |

Next, audit the largest Palmeiras/15 weights with root, full rival status,
actual score/tiebreak data and proposal-density components. Determine whether
the tail needs another family or better sampling within a known one. Use that
audit to design generic rules, then train on separate samples and confirm with
fresh streams. Do not feed these final reference observations into a purportedly
independent estimate. Track this and the funded-recording performance hypothesis
in `FUTURE_EXPERIMENTS.md`.
