# R56: work backwards from successful Flamengo / 12th seasons

## Question and result

The tiny estimate has a repeatable structure. In the five accepted main batches,
Flamengo losing all ten remaining matches carries **99.683–99.999% of corrected
importance contribution**. Two rival families carry **77.57–92.17%** of that
batch's total contribution, including only seasons with zero additional Flamengo
points and no primary point/win ties:

| Family | Additional rivals below Flamengo | Tight rival that passes Flamengo |
|---|---|---|
| A | Corinthians, Mirassol, Vitória, Vasco | Botafogo |
| B | Corinthians, Mirassol, Vitória, Botafogo | Vasco |

Grêmio, Internacional, Remo and Chapecoense finish below Flamengo in both
families. Their maximum possible points are respectively59,58,53 and51, below
Flamengo's starting60. The other eleven rivals finish strictly above Flamengo
on points/wins. These are observed families of many complete fixture assignments;
they do not describe every possible way Flamengo can finish12th.

The important concentration is in **rival identities and near-threshold totals**.
Most non-target fixtures are not individually fixed across the successful
seasons. Concentrating on Flamengo's own results alone leaves this joint rival
problem largely intact.

## Data and method

Use the saved group16498 request, team17, rank12:

`experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json`

Request SHA256:
`2e08f162592ded09393f0438686a60690dda6e85cd54da5dc0d20224c81aa625`.

The request sorts by `pt,w,gd,gf,gp,name`. All statistics come from this request;
the local database was read only to obtain team names. No database mutations.

Replay R54's immutable native10 Rust control at seeds808,1669,1993,2281,2293,
with four workers and no concurrent heavy benchmark/build. This grants ten-fold
selected-cell additional confirmation, rather than ten-fold ordinary Monte
Carlo or full-request work. Seed1993 receives no expanded grant because its
native experimental admission is ineligible; retain it as recorded.

The observer lives in an isolated crate at
`/private/tmp/golaberto-r56/odds-rust`. It records every positive lazy sampler
observation, plus full assignments for the largest32 corrected weights per
stream. It changes no random calls, proposals, weighting, sample counts or
acceptance gates. The production Rust tree and defaults remain unchanged.

For each stream, group contributions are computed as:

```
contribution = target_condition_mass × sum(corrected_hit_weights) / all_draws
```

The denominator includes failed draws. Raw hit counts are retained separately.
Pilot, ordinary final, additional-confirmation main and check streams stay
separate. The endpoint publishes the accepted **main** estimate; its independent
check validates that estimate and is not averaged into it.

All fixture marginals and team-total histograms include every positive hit.
Largest-weight examples are used only for assignment inspection, not as a
substitute for those marginals. Caller stream labels are recovered from the
existing deterministic FNV-derived seed tags.

## Paired final-batch observations

Shares below are corrected contribution shares within each separate stream.
Families A/B require exactly the stated eight strictly-below rivals, no equal
point/win rivals, and zero additional Flamengo points.

| Request seed | Main/check draws each | Main estimate, probability | Main all-loss share | Main A+B share | Check A+B share | Main/check ESS |
|---|---:|---:|---:|---:|---:|---:|
| 808 | 83,951 | 1.314e-25 | 99.9986% | 77.57% | 86.94% | 23.13 /29.80 |
| 1669 | 100,000 | 1.037e-25 | 99.6832% | 92.17% | 81.18% | 47.38 /29.09 |
| 1993 | 24,247 | 1.030e-25 | 99.9965% | 85.16% | 96.69% | 7.97 /5.93 |
| 2281 | 100,000 | 1.049e-25 | 99.9671% | 87.05% | 94.67% | 35.02 /19.28 |
| 2293 | 48,304 | 1.189e-25 | 99.9835% | 85.71% | 98.26% | 12.46 /16.84 |

These probabilities precede final matrix reconciliation, which makes tiny
numerical adjustments. There are356,502 draws and12,707 positive hits in the
five main streams, with the same number of draws and12,886 hits in their checks.
This is evidence for a useful proposal structure, not a certified posterior
decomposition. Effective sample sizes remain modest despite the larger raw
batches. Seed1993's check estimate2.318e-25 is more than twice its main estimate.

### The rivals usually pass by a small margin

São Paulo and Bragantino's most common weighted final point/win bin is61/18 in
every main stream. Santos and Coritiba concentrate on61/17 and62/18, with some
63-point contributions. For example, at seed1669 the share finishing on61 or62
points is88.74% for São Paulo,90.36% for Bragantino,93.68% for Santos and88.34%
for Coritiba. These shares vary across seeds; Bragantino's61/62 share is only
58.24% at seed2293, where63/19 also carries substantial weight.

These totals correspond to tight remaining records:

| Rival | Current points/wins | Example final points/wins | Required additional record |
|---|---:|---:|---|
| São Paulo | 36/10 | 61/18 | 8wins,1draw,2losses |
| Bragantino | 36/10 | 61/18 | 8wins,1draw,2losses |
| Santos | 38/10 | 61/17 | 7wins,2draws,2losses |
| Coritiba | 38/10 | 61/17 | 7wins,2draws,1loss |
| Botafogo | 35/9 | 61/17 | 8wins,2draws,0losses |
| Vasco | 31/8 | 61/18 | 10wins,0draws,1loss |

They cannot be conditioned independently: their shared matches must allocate
the same three points consistently to both teams.

### Goals are usually already irrelevant to the rank

Only1.28–4.46% of main contribution, and0.69–6.72% of check contribution,
requires the sampler's actual conditional score draw. The rest has the rank
already fixed by points/wins. No canonical1–0 score was presented as an actual
sampled score. The observer marks score validity afresh on every rank call and
retains real goals only after the existing score sampler and sorter ran.

A concrete example is the largest contributor in the seed808 check. Flamengo
finishes60/18. The eleven rivals above it finish:

| Rival | Points/wins | Rival | Points/wins |
|---|---:|---|---:|
| Athletico-PR | 61/17 | Atlético-MG | 62/18 |
| Bahia | 62/17 | Bragantino | 62/18 |
| Coritiba | 62/18 | Cruzeiro | 61/18 |
| Fluminense | 61/17 | Palmeiras | 71/20 |
| Santos | 63/18 | São Paulo | 62/18 |
| Vasco | 61/18 | | |

All eleven have more than60 points, so this rank needs no goal tiebreaker.
The retained outcome vector covers every remaining fixture. Its individual
full W/D/L assignment has prior probability7.255e-51; its importance-sampling
contribution to the estimate is1.078e-26. Those quantities have different
meanings: the event estimate sums contributions for many assignments.

## Why the rival graph creates these families

Corinthians and Mirassol each start on32points/8wins with ten games remaining.
To pass60/18, either must win all ten:9wins and1draw gives60/17 and loses on
wins. Their remaining shared fixture364121 prevents both from doing that.

If Corinthians passes, its required wins also beat Vasco, Vitória and
Botafogo. Vasco and Vitória cannot afford another nonwin and still pass
Flamengo, but play one another in fixture364178. Botafogo cannot take a second
loss and pass60, but plays Vasco in fixture364080. These conflicts force
additional teams below Flamengo in the Corinthians-above branch.

The leading sampled families take a different, repeatable route: **both
Corinthians and Mirassol stay below**, along with Vitória and either Vasco or
Botafogo. That frees enough wins for the other tight rivals. The same outcome
assignment supplies all their final totals.

Both leading masks already occur in R52's all-loss pruning-surviving partition:
zero-based case ordinals51 and234 within its531-case all-loss row. Their prior
survival was not a probability estimate. The fresh captures now show where
much of the weighted contribution lies.

The exact target all-loss prior is1.501974029360509e-8 in this model. Dividing
the main all-loss contributions by that prior gives an estimated conditional
rank probability around6.9–8.7e-18. Thus the main remaining difficulty is the
joint rivals, even after Flamengo's own outcomes are fixed.

## Recommendation

Next experiment: partition the rival problem by these learned below-team
families and apply shared fixture propagation to the eleven teams that must
pass Flamengo. Try exact counting or correctly weighted conditional sampling
inside each family, starting with the0-point target root. Learn the families
from pilot data; validate with fresh independent main/check draws.

Keep a supported component for other rival families, target point totals and
goal ties. The two leading families omit roughly8–22% of observed main
contribution. A rare but legitimate Mirassol-above family accounts for12.76%
of the seed808 main estimate through a single observation; it is not a sound
reason to exclude other families or force Mirassol below unconditionally.

This differs from the earlier root-only reallocation, which left within-root
rival variability unchanged, and the unsuccessful neighboring-rank transfer.
It is a hypothesis for a focused next experiment; no new probability proposal
or production change was adopted here.

## Reproduction and validation

The self-contained isolated source is archived at
`experiments/rare_positions/2026-10-03-hit-patterns/source-archive.tar.gz`,
SHA256 `eb0e5ef1006135299dc3d6419af951fdd016582cd46d4acee0fc902bb5b53c86`.
It includes the observer crate and its required sibling `stats/core` source;
compiled targets and local environment files are excluded. Rebuild it with:

```sh
mkdir -p /private/tmp/flamengo-pattern-source
tar -xzf experiments/rare_positions/2026-10-03-hit-patterns/source-archive.tar.gz \
  -C /private/tmp/flamengo-pattern-source
cargo build --release -j4 \
  --manifest-path /private/tmp/flamengo-pattern-source/odds-rust/Cargo.toml
```

The diagnostic selector and output directory are opt-in flags in the isolated
observer source. A representative invocation is:

```sh
env RUST_ODDS_LOG=1 \
  RUST_ODDS_EXPERIMENT_NEIGHBOR_TRANSFER=native \
  RUST_ODDS_EXPERIMENT_NEIGHBOR_CELL=17:12 \
  RUST_ODDS_EXPERIMENT_NEIGHBOR_BUDGET_MULTIPLIER=10 \
  RUST_ODDS_LAZY_HIT_AUDIT_CELL=17:12 \
  RUST_ODDS_LAZY_HIT_AUDIT_DIR=/private/tmp/flamengo-hit-audit \
  /private/tmp/flamengo-pattern-source/odds-rust/target/release/golaberto-odds \
  estimate \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  /private/tmp/flamengo-hit-audit-result.json 808 4

python3 experiments/rare_positions/2026-10-03-hit-patterns/analyze_hit_audit.py \
  experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json \
  /private/tmp/flamengo-hit-audit /private/tmp/flamengo-hit-analysis.json

python3 experiments/rare_positions/2026-10-03-hit-patterns/analyze_hit_audit.py --self-test
```

`cargo fmt --check`, `cargo check --release -j4` and
`cargo build --release -j4` pass in the isolated observer crate. All26 captured
streams exactly reproduce their sampler hit counts and probabilities.
Observer-off/on seed808 exports also byte-match the immutable R54v4 native10
export, including every other cell. All five complete observer-on exports
byte-match their archived R54 native10 controls. The packaging summary records
the source-member hashes and every control comparison; the coordinator also
verified all26 captured stream hashes and result checks.

The self-test uses two different masses and draw counts and separate rival
statuses. It verifies contribution normalization and keeps stream reports
separate. An independent Sol review found no material issue in observer RNG,
weighting, score-validity handling, or full-assignment replay.

### Diagnostic overhead

A fresh serialized seed808 pair used the same native10 grant and four workers:

| Measurement | Observer off | Observer on |
|---|---:|---:|
| Process wall time | 2.935s | 6.856s |
| User CPU | 4.930s | 5.033s |
| System CPU | 0.190s | 3.985s |
| Total CPU | 5.121s | 9.018s |

Capture adds3.921s wall time (about134%) and3.897s CPU (about76%) in this single
pair. Most additional CPU is system time. These measurements include diagnostic
capture/output and are not a production latency allowance or an optimization
claim. The two numerical exports are identical. R54 separately measures the
cost of the selected-cell ten-fold learning grant. No observer code or new
search work was integrated into production.

Raw stream captures, analyses, commands, source provenance and check evidence
are kept in `experiments/rare_positions/2026-10-03-hit-patterns/`.
Exact timing argv/environment and export/log hashes are in
`seed808-pair-resource-usage.json`; `manifest.json` identifies each stream and
separates pre-final schema artifacts. No commit or push.

Related evidence: [R54 neighboring-rank transfer](2026-10-03-rust-neighbor-transfer.md),
[R55 three-point isolation](2026-10-03-rust-flamengo-three-points.md), and
[R52 exact rank cases](2026-10-02-rust-exact-rank-cases.md). R52's preserved
`2026-10-02-exact-rank-cases/raw-evidence.tar.gz` contains `certified-paths.json`
with the all-loss531-case partition used above.
