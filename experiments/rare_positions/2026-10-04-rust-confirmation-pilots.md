# R65: failed confirmations and robust pilot allocation

## Scope

Use two tracked group-16498 fixtures as the regression pair:

| Fixture | Remaining games | Ratings | SHA256 |
|---|---:|---|---|
| Original, `reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json` | 103 | Original | `2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625` |
| Updated, `reference/2026-10-04-two-results/inputs/group-16498-two-results.json` | 101 | Refreshed from the Rails request builder | `cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee` |

The updated fixture contains game 364007 at 1–2 and game 363999 at 1–0.
Intermediate result/rating combinations are historical local diagnostic files,
not additional tracked regression fixtures. Retain the other reference groups.

Baseline: production defaults at `e3ce9653`, frozen executable
`2026-10-04-early-rank/data/golaberto-odds-tree-production-v2`, SHA256
`ae0e68a5d5986436656ca34aa6b6b6a9f54befeaaf96049dfbc0df96ab6bbdf1`.
All experiments use four calculation workers and the existing deterministic
operation capacity. Wall time and CPU remain measurements, not allowances.
Experiments are default-off; no commit, push, or default change is authorized.

## Baseline diagnosis

At seed 808, the original fixture has 383 positive cells, 17 impossible zeros,
and no reachable zeros or undecideds. The updated fixture has 380 positive
cells, the same 17 impossible zeros, and three reachable zeros:

- Flamengo/13th: the complete-tree pilot has zero event hits, so final sampling
  is skipped.
- Palmeiras/15th: native MAIN passes publication checks, but the independent
  CHECK fails ESS and agreement. Existing late-family eligibility requires MAIN
  to fail, so this cell is excluded.
- Palmeiras/16th: complete branches produce many event hits, but too much
  weighted probability comes from a few observations. MAIN fails ESS, maximum
  contribution, and batch agreement checks.

See `2026-10-04-rust-two-results.md` for the fixture comparison and numerical
diagnostics. Fewer remaining games reduce some construction work, but do not
guarantee better allocation or more reliable weighted samples.

## Focused changes

### Failed independent confirmation

`RUST_ODDS_EXPERIMENT_FAMILY_FAILED_CONFIRMATION=1` extends eligibility to
originally paired Lazy proposals with a publishable MAIN, a sampled but rejected
CHECK, and a still-zero, non-impossible cell. Legacy failed-MAIN candidates run
first. Training uses the existing MAIN prefix; validation and final MAIN/CHECK
use fresh streams. Successful estimates, weighting, support, acceptance checks,
and cell/request grant limits are preserved.

Eligibility does not supply funding. Logs report the original cell grant,
native work, residual cell capacity, and available fallback grant. In the updated
baseline, a declined family for Palmeiras/14th consumes about 108 million work
units before a successful overflow search for that cell consumes about 120
million. The final residual bank is about 11.6 million. These are measured
outcomes, not a rule for identifying failed searches in advance.

### Defensive allocation

`RUST_ODDS_EXPERIMENT_BRANCH_DEFENSIVE_PERCENT` sets the uniform share of final
draws left after positive per-stratum floors. Default is 10; parsed integers are
clamped to 10–50. The remainder follows empirical pilot variability. Totals,
floors, operation reservations, final stream separation, and acceptance checks
are unchanged. This applies to complete branches and their earlier tree retry,
not the later target-overflow allocator.

### Uncertain pilot scores

`RUST_ODDS_EXPERIMENT_BRANCH_PILOT_PRIOR=1` blends empirical pilot standard
deviation with a small prior proportional to the branch's existing upper bound.
With pilot sample count `n`, empirical standard deviation `s`, maximum empirical
standard deviation `S`, bound `B`, and maximum bound `M`, the score is:

```
hypot(s * sqrt(n/(n+20)), S * (B/M) * sqrt(20/(n+20)))
```

If every empirical standard deviation is zero, positive normalized bounds
provide allocation scores. A complete tree with zero pilot hits and a positive
finite bound may proceed to its fixed final pair, within its existing bank.
Every stratum retains positive allocation. These scores are allocation
heuristics, not confidence bounds; estimates and their standard errors retain
their existing definitions. No failed pilot proves impossibility.

Research context:
[Carpentier, Munos and Antos (2015)](https://jmlr.csail.mit.edu/papers/v16/carpentier15a.html)
uses optimistic variability estimates for stratified sampling, while
[Carpentier and Munos (2013)](https://proceedings.mlr.press/v28/carpentier13.html)
describes the allocation difficulty created by finer partitions. Their formal
assumptions are not established for our importance weights; this experiment
does not inherit their performance guarantees.

## Initial paired screens

Candidate v1 SHA256:
`1e768ac301b1be7788ec441f7217f12937a065d4bcc792089fea2a44d6b6d898`.
The default-off candidate and frozen baseline produce identical updated-fixture
exports at seed 808.

| Change, updated fixture/808 | Added cells | Lost cells | Diagnosis |
|---|---:|---:|---|
| Failed-confirmation eligibility only | 0 | 0 | Palmeiras/15th becomes eligible but has only 2,894,621 residual work units |
| Pilot prior, 10% defensive share | 1 | 0 | Palmeiras/16th passes its original 25,000-draw pair |
| 25% defensive share | 1 | 0 | Palmeiras/16th requires the existing 75,000-draw retry |
| 50% defensive share | 0 | 0 | Palmeiras/16th still fails confirmation |
| Pilot prior plus 25% defensive share | 1 | 0 | Palmeiras/16th passes its original pair |

Palmeiras/16th with the prior: probability `1.4288e-40`, independent CHECK
`2.9397e-40`, MAIN ESS 10.46, CHECK ESS 5.08, maximum MAIN contribution 0.190,
and batch gap 0.267. Baseline MAIN ESS is 2.286, maximum contribution 0.640,
and batch gap 1.715. The final exported probability differs negligibly from
the raw estimate due to matrix reconciliation. Units here are fractions;
multiply by 100 for percentage values.

The prior still finds no Flamengo/13th event in its admitted 3,000-draw pair
or existing 9,000-draw retry. Spending final draws on every zero-hit pilot is
therefore not automatically useful.

Six original reference groups at seed 808 preserve coverage with the prior and
with eligibility only. **Reject the blanket 25% defensive share:** on the
original group-16498 fixture it loses Palmeiras/14th and Palmeiras/16th.
Improvement on the updated fixture alone would hide this regression.

The prior also preserves coverage on seeds 1669, 1993, 2281 and 2293 for both
fixtures, with no additional gains on those seeds. These checks establish
preservation on the measured requests, not a guarantee for arbitrary seeds.

Three warmed, rotated paired repetitions at seed 808:

| Fixture | Mean request wall, baseline → prior | Mean child CPU, baseline → prior |
|---|---|---|
| Original | 2.070 → 2.005 s (−3.1%) | 4.159 → 4.077 s (−2.0%) |
| Updated | 2.036 → 1.954 s (−4.0%) | 4.447 → 4.352 s (−2.1%) |

All three exports for each arm/fixture agree. This small local timing sample
shows no increase; it is insufficient to claim a reliable speed improvement.
The CLI measures complete estimator requests, including model construction and
export, with four workers. It does not include Rails request-building time.

The failed-confirmation grant is the immediate funding constraint:
`349,908,372 original − 347,013,751 native = 2,894,621 residual`.
Training does not start. Extending eligibility without allocating recycled work
cannot repair this failure.

Raw v1 logs/exports are under
`local/2026-10-04-confirmation-pilots/{family-updated,pilot-updated,reference-screen}`.
Single-run timing is noisy and is not used as a performance conclusion.

## Failed-confirmation funding experiments

Candidate v2 SHA256:
`86e6dbe81dd97bbf5acd701cd4569c41b2b149b2b769c4b5c2cf1b19e201f2f8`.
Its opt-out export matches the production baseline exactly at updated/808.

The additional controls are default-off:

- `RUST_ODDS_EXPERIMENT_FAMILY_FAILED_CONFIRMATION_TRANSFER_PERCENT`: default
  0, clamp 0–50. Allows an explicit shared-bank transfer capped by this fraction
  of the original cell grant. An already-overrun native cell cannot receive it.
- `RUST_ODDS_EXPERIMENT_FAMILY_LEGACY_GRANT_PERCENT`: default 100, clamp 0–100.
  Caps legacy failed-MAIN family residual grants only when an eligible
  failed-CHECK recipient exists and transfer mode is active.
- `RUST_ODDS_EXPERIMENT_FAMILY_RESERVE_FAILED_CONFIRMATION=1`: holds recipient
  grants across overflow, in stable order, within half the entry free bank.
  Superseded or aborted candidates return their promises. Without it, overflow
  runs before extra families without an advance reservation.
- `RUST_ODDS_EXPERIMENT_BRANCH_SKIP_EMPTY_PILOT_FINAL=1`: retains the legacy
  skip for an entirely zero-hit tree pilot while keeping the prior scores for
  zero-hit strata within a nonempty pilot. The admitted empty-pilot finals
  found no additional cells in these checks.

Funding records account for requested, reserved, consumed and released work
after sampling. Publication checks both the fresh pair and the original grant
plus explicit transfer and any separately audited observer allowance.

Updated fixture/808, transfer percentage / legacy grant percentage:

| Funding mode | Family grant for Palmeiras/15th | Fresh MAIN draws | Result |
|---|---:|---:|---|
| After overflow, 20 / 50 | 36,809,986 | 0 | Insufficient final funding |
| After overflow, 40 / 25 | 94,649,424 | 3,501 | MAIN ESS 3.455, max share 0.388; declined |
| Advance reserve, 20 / 50 | 72,876,295 | 2,350 | MAIN ESS 2.382, max share 0.502; declined |
| Advance reserve, 40 / 25 | 120,993,934 | 4,561 | MAIN ESS 3.521, max share 0.385; declined |
| Reserve 40 / 25 plus pilot prior and empty-pilot skip | 96,759,808 | 3,585 | MAIN ESS 3.463, max share 0.388; declined |

All preserve existing positives on this request and remain within their family
grant. Training yields 26 hits and four families at the 3,000-draw cap for the
larger grants. More funding begins useful sampling, but a few observations still
dominate. Acceptance gates were not relaxed.

### Reuse of unused branch reservations

The existing `RUST_ODDS_EXPERIMENT_TARGET_TREE_RECLAIM=1` returns audited unused
branch draw capacity to the confirmation bank. Credit is
`reserved − already released − actual draw work`, capped by the original branch
transfer and the remaining original confirmation capacity. An invalid audit
returns zero credit. This reuses existing capacity and can increase actual work
and runtime relative to a baseline that left it unused.

With this credit, a 50% transfer permits a 177,848,807-unit Palmeiras/15th family
grant. It spends 166,796,264 units and runs 7,330 fresh draws per stream:

| Metric | MAIN | CHECK |
|---|---:|---:|
| Probability | 1.74147e-32 | 8.59968e-33 |
| Hits | 811 | 762 |
| ESS | 5.780 | 3.430 |
| Relative standard error | 0.416 | 0.540 |

MAIN maximum contribution is 0.276. The pair passes existing order-of-magnitude
gates. Combining this with the pilot prior finds Palmeiras/15th and /16th on the
updated fixture, but **loses Palmeiras/14th on the original fixture at 808**.
Reject that configuration.

### Smaller complete trees

A further existing-knob configuration uses minimum one target root and 3,000
final tree draws, with credit, prior scores, the empty-pilot skip, and reserved
50 / 25 family funding. It fills Palmeiras/15th through the tree before family
replay. Its tree MAIN is `3.49138e-31`, CHECK `8.98774e-32`, with ESS 5.971 and
10.596. The formerly failed family candidate is then skipped.

Across the seven snapshots at 808 and four additional seeds for each regression
fixture (15 distinct request/seed pairs), this configuration has four gained
cell-runs and no lost positives:

| Fixture / seed | Additional nonzero cells |
|---|---|
| Updated / 808 | Palmeiras/15th, Palmeiras/16th |
| Original / 1669 | Palmeiras/14th |
| Updated / 2293 | Paysandu/4th, team 318 |

The two Palmeiras/15th proposals differ by about 20-fold despite independent
checks passing. There is no high-precision golden probability for this updated
fixture. Treat these as rough estimates; full support and a passing diagnostic
do not certify precision for such tiny probabilities.

At updated/808 this configuration has 382 positives, 17 impossible zeros, one
reachable zero (Flamengo/13th), and no undecideds. The original/808 remains
zero-free among reachable cells. However, its warmed cost violates the latency
constraint, as shown below.

## Paired full-request timing

Three measured repetitions after one warm-up; serialized requests, rotated arm
order, four workers. Times include CLI model construction, all estimator stages
and export. No Rails request-building work is included. Different batches show
local variability; the same capacity ceiling is not evidence of equal runtime.

| Configuration / fixture | Mean wall, baseline → candidate | Mean child CPU, baseline → candidate |
|---|---|---|
| v2 defaults / updated | 2.069 → 1.853 s (−10.4%) | 4.232 → 4.244 s (+0.3%) |
| Prior with empty-pilot finals / updated | 2.069 → 2.186 s (+5.7%) | 4.232 → 4.430 s (+4.7%) |
| **Prior with empty-pilot skip / updated** | **2.069 → 1.988 s (−3.9%)** | **4.232 → 4.399 s (+3.9%)** |
| Small trees plus reserved families/credit / original | 1.843 → 1.898 s (+3.0%) | 3.936 → 4.153 s (+5.5%) |
| Small trees plus reserved families/credit / updated | 1.918 → 2.163 s (+12.7%) | 4.074 → 4.670 s (+14.6%) |
| 3,000-draw overflow plus family/credit, without small-root admission / updated | 1.751 → 2.230 s (+27.4%) | 3.936 → 4.579 s (+16.3%) |

Repeated exports are identical per arm/request. The baseline wall time varies
enough that these small samples do not establish a reliable speed improvement.
CPU increases for the more extensive configurations are material and are
reported explicitly. No extra time allowance is assumed.

In the broad small-tree updated/808 timing record, legacy family work decreases
from 107,978,996 to 24,953,266 units. Overflow work increases from 120,002,122 to
149,049,562 units, and wall time from 120.4 to 226.8 ms in that repetition.
Credit returns 154,375,639 unused units; its bank limit becomes 1,230,145,941,
within the original confirmation ceiling. Setup and sampling costs differ
between proposal types; modeled units alone did not predict the full CPU cost.
Invalid draw audits occur on some baseline and candidate holdouts; none receives
credit. All recorded family settlements remain within their grants, with no
family overrun.

## Recommendation

1. **Recommend the pilot prior with the empty-pilot final skip.** It recovers
   Palmeiras/16th at `1.42882e-40` on the updated default seed, changing reachable
   zeros from three to two. It loses no positives on all six reference groups
   at 808 and four holdouts for both regression fixtures. The original/808 keeps
   zero reachable zeros. Latest paired wall time does not increase; CPU rises
   3.9%, with the same four workers. Production defaults remain unchanged.
2. Keep failed-confirmation eligibility and transfer/reservation controls as
   default-off experiments. Eligibility alone cannot fund the current failure;
   sufficient funding can rescue it. The broader measured configurations
   either lose coverage or increase latency. Do not enable them as a package
   under the current performance constraint.
3. Reject the blanket 25% or 50% uniform allocation changes. The 25% variant
   trades away two original-fixture estimates, and 50% adds no updated/808 cell.
4. Flamengo/13th needs a better proposal. Its prior-admitted finals and larger
   retry still find zero events; the extra work has not produced an estimate.

No new reachability proof mechanism was introduced. The remaining zero
classifications come from the existing production sorter and proof stages.
All weighting, support, fixed streams and publication gates are retained. This
task does not commit, push, or change defaults.

## Reproduction

Build/check:

```bash
cargo build --offline --locked --release -j1 --manifest-path odds-rust/Cargo.toml --bin golaberto-odds
cargo test --offline --locked --release -j1 --manifest-path odds-rust/Cargo.toml -- --test-threads=1
```

Recommended narrow experiment on the updated fixture:

```bash
python3 experiments/rare_positions/2026-10-04-target-overflow/scripts/run_overflow.py \
  --baseline experiments/rare_positions/2026-10-04-early-rank/data/golaberto-odds-tree-production-v2 \
  --baseline-sha256 ae0e68a5d5986436656ca34aa6b6b6a9f54befeaaf96049dfbc0df96ab6bbdf1 \
  --candidate odds-rust/target/release/golaberto-odds \
  --reference-dir experiments/rare_positions/reference/2026-10-04-two-results/inputs \
  --output /tmp/r65-updated-prior-skip \
  --groups 16498 --seeds 808,1669,1993,2281,2293 \
  --arm priorSkip:RUST_ODDS_EXPERIMENT_BRANCH_PILOT_PRIOR=1,RUST_ODDS_EXPERIMENT_BRANCH_SKIP_EMPTY_PILOT_FINAL=1
```

For original reference controls, omit `--reference-dir` and use
`--groups 15902,16413,16498,16653,16982,16983 --seeds 808`.
For timing use `--seeds 808 --repeats 3 --warmups 1`. The harness clears inherited
`RUST_ODDS_*` and `RARE_POSITION_*` variables and records all explicit flags,
input/executable/export hashes, events, CPU and wall time. Output must be empty.

The broader diagnostic profile is the same command with this arm:

```text
smalltreescredit:RUST_ODDS_EXPERIMENT_FAMILY_FAILED_CONFIRMATION=1,RUST_ODDS_EXPERIMENT_FAMILY_FAILED_CONFIRMATION_TRANSFER_PERCENT=50,RUST_ODDS_EXPERIMENT_FAMILY_LEGACY_GRANT_PERCENT=25,RUST_ODDS_EXPERIMENT_FAMILY_RESERVE_FAILED_CONFIRMATION=1,RUST_ODDS_EXPERIMENT_BRANCH_PILOT_PRIOR=1,RUST_ODDS_EXPERIMENT_BRANCH_SKIP_EMPTY_PILOT_FINAL=1,RUST_ODDS_EXPERIMENT_TARGET_TREE_RECLAIM=1,RUST_ODDS_EXPERIMENT_TARGET_TREE_MIN_ROOTS=1,RUST_ODDS_EXPERIMENT_TARGET_TREE_FINAL_DRAWS=3000
```

Raw results are retained locally under `local/2026-10-04-confirmation-pilots/`.
Key batches: `funding-updated`, `reclaimed-screen`, `funded-reference-screen`,
`funded-holdouts-{original,updated}`, `selected-timing-{original,updated}`,
`default-prior-timing`, `minimal-reference-screen` and
`minimal-holdouts-{original,updated}`. Frozen v1/v2 candidate executables are in
its `binaries` directory. The final build includes only a test-only compilation
cleanup after measured v2: a legacy allocation wrapper is gated with
`#[cfg(test)]`. Final executable SHA256:
`eab52f6ff920b5bcaf7e010e056eaf0cf3892f01d84d58347b5b1fd39b3612ea`.

Final artifact checks are in `final-artifact-{original,updated}`. At seed 808,
default-off final exports exactly match the frozen production baseline on both
fixtures, and final recommended-profile exports exactly match the measured v2
records. Original coverage remains 383 positive cells with no reachable zeros;
updated coverage is 381 positive cells with two reachable zeros. Both have
17 impossible zeros and no undecideds.

| Fixture | Default export SHA256 | Recommended export SHA256 |
|---|---|---|
| Original | `d916a38398554b138e4f042fa6bfd3c9dae300c98743b057a65f475bcd907be5` | `d7a0edc0a1c5bdc13731cb6c091c5b63db5f956d0fbcc6c8da5d5c4dc9c0f4a7` |
| Updated | `69649ae089d0d50d8e1a09611468f789bf4893b26ce44760d4124b080d3779eb` | `53a1337c7aee110484515e9225a93da3728736e601ef09a54ba82bc263a7b9e9` |

## Validation

- Independent Sol review of support, independence, grant accounting and credit.
- 74 focused rare-tail tests pass, including actual low-ESS/disagreeing CHECK
  cases, fixed legacy allocation vectors, tiny-score arithmetic, transfer
  limits, native overruns, promise release and settlement accounting.
- Formatting and diff checks pass.
- Full Rust suite: 210 passed, four ignored, zero failed. The ignored tests
  require `MYSQL_TEST_URL` and cover database operations using temporary tables.
  The initial sandboxed attempt could not bind HTTP test listeners; the exact
  rerun with localhost socket permission passed.
- Final release main build succeeds; both fixture exports reproduce the frozen
  baseline with experiments off and the measured recommended configuration
  with its two flags on.
