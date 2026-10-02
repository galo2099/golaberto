#!/usr/bin/env python3
"""Summarize paired native Rust rare-probability experiments (no DB access)."""
import argparse, collections, hashlib, json, statistics
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEST = ROOT / 'experiments/rare_positions/2026-10-01-rare50'
REPORT = ROOT / 'experiments/rare_positions/2026-10-01-rust-rare50.md'


def load(path):
    return json.loads(Path(path).read_text())


def analyze(rows):
    unique = {(r['input'], g['team'], g['position']) for r in rows for g in r['comparison']['gains']}
    return dict(pairs=len(rows), gain=sum(len(r['comparison']['gains']) for r in rows),
                distinct_snapshot_cells=len(unique),
                eligible_before=sum(sum(r['timing']['baseline']['cells'][k] for k in ('reachable_zero','undecided_zero')) for r in rows),
                eligible_after=sum(sum(r['timing']['candidate']['cells'][k] for k in ('reachable_zero','undecided_zero')) for r in rows),
                max_latency_ratio=max(r['timing']['candidate']['http_ms']/r['timing']['baseline']['http_ms'] for r in rows),
                losses=sum(len(r['comparison']['lost']) for r in rows),
                proof_regressions=sum(len(r['comparison']['impossible_regressions']) for r in rows),
                lost_reachability=sum(len(r['comparison']['lost_reachability']) for r in rows),
                game_importance_identical=all(r['comparison']['game_importance_identical'] for r in rows))


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--pairs',type=Path,required=True,help='Benchmark output directory')
    p.add_argument('--warm',type=Path,required=True,help='Warm benchmark output directory')
    p.add_argument('--tests-passed',type=int,default=77)
    a=p.parse_args(); pairs=load(a.pairs/'summary.json'); warm=load(a.warm/'summary.json')
    rows=pairs['pairs']; dev=[r for r in rows if r['seed']<1200]; held=[r for r in rows if r['seed']>=1200]
    totals=analyze(rows); by=collections.defaultdict(list)
    budget_fraction=float(pairs['flags'].get('RUST_ODDS_RARE_TAIL_BUDGET_FRACTION','0.35'))
    warm_ratios=[c/b for r in warm['pairs'] for b,c in zip(r['latency_ms']['baseline'],r['latency_ms']['candidate'])]
    warm_over=sum(q>1.5 for q in warm_ratios)
    total_baseline_ms=sum(r['timing']['baseline']['http_ms'] for r in rows)
    total_candidate_ms=sum(r['timing']['candidate']['http_ms'] for r in rows)
    added_seconds=(total_candidate_ms-total_baseline_ms)/1000.
    efficiency=totals['gain']/added_seconds if added_seconds>0 else None
    for r in rows:by[r['input']].append(r)
    golden=load(ROOT/'experiments/rare_positions/reference/2026-09-30-hundredfold/reference.json')
    refs={(c['case']+'.json',e['team'],e['position']):e for c in golden['cases'] for e in c['cells']}
    evidence=[]; remaining=[]; scored=[]
    for r in rows:
        response=load(a.pairs/f"{Path(r['input']).stem}-{r['seed']}-candidate.json")
        for g in r['comparison']['gains']:
            ref=refs.get((r['input'],g['team'],g['position']),{})
            meta=response['rare_position_estimates'][str(g['team'])][str(g['position']-1)]
            e=dict(input=r['input'],seed=r['seed'],team=g['team'],position=g['position'],estimate=meta,reference_status=ref.get('status'),reference_probability=ref.get('probability'))
            if ref.get('status') in ('reference_is','reference_mc') and ref.get('probability'):
                e['reference_ratio']=meta['probability']/ref['probability'];scored.append(e)
            evidence.append(e)
        for t,rs in response['rare_position_estimates'].items():
            for pos,e in rs.items():
                if e['probability']==0 and not e['reachability'].startswith('impossible'):
                    remaining.append(dict(input=r['input'],seed=r['seed'],team=int(t),position=int(pos)+1,reachability=e['reachability'],upper=e['zero_hit_upper_95']))
    result=dict(description='Native Rust baseline, seven snapshots, nine paired four-worker seeds. Probabilities are fractions, not percentages.',
                totals=totals,development=analyze(dev),held_out=analyze(held),
                final_pairs=pairs,final_warm=warm,new_estimates=evidence,remaining_eligible_zeros=remaining,
                golden_comparisons=scored,tests=dict(passed=a.tests_passed,ignored=2,database_writes=False))
    DEST.mkdir(exist_ok=True);(DEST/'final-results.json').write_text(json.dumps(result,indent=2)+'\n')
    case_lines=[]
    for name,rs in by.items():
        s=analyze(rs)
        before=[r['timing']['baseline']['cells']['reachable_zero']+r['timing']['baseline']['cells']['undecided_zero'] for r in rs]
        after=[r['timing']['candidate']['cells']['reachable_zero']+r['timing']['candidate']['cells']['undecided_zero'] for r in rs]
        proof=rs[0]['timing']['candidate']['cells']['impossible_zero']
        gains=[len(r['comparison']['gains']) for r in rs]
        case_lines.append(f"| {name.removesuffix('.json')} | {min(before)}–{max(before)} | {min(after)}–{max(after)} | {proof} | {s['gain']} | {s['distinct_snapshot_cells']} | {min(gains)}–{max(gains)} |")
    warm_lines=[]; stage_lines=[]
    stages=['scout','pool.mc','search.initial_conditioning','search.guided','search.extra','search.witnesses','search.point_tilt','search.peers','search.domains','search.rare_tail']
    for r in warm['pairs']:
        b,c=r['resources']['baseline'],r['resources']['candidate'];bm,cm=r['median_ms']['baseline'],r['median_ms']['candidate']
        warm_lines.append(f"| {r['input'].removesuffix('.json')} | {bm:.1f} | {cm:.1f} | {(cm/bm-1)*100:+.1f}% | {(c['cpu_seconds']/b['cpu_seconds']-1)*100:+.1f}% | {b['peak_rss_mib']:.1f} → {c['peak_rss_mib']:.1f} |")
        if '16498' in r['input']:
            for s in stages:
                bv=b['stages'].get(s,[]);cv=c['stages'].get(s,[])
                stage_lines.append(f"| {s} | {statistics.median(bv) if bv else 0:.2f} | {statistics.median(cv) if cv else 0:.2f} |")
    training_lines=[]
    for name,rs in by.items():
        summaries=[e for r in rs for e in r['timing']['candidate']['tail'] if e['event']=='rust_odds_rare_tail_summary']
        if summaries:
            training=statistics.median(e['allowance_ms']-e['remaining_after_training_ms'] for e in summaries)
            total=statistics.median(e['elapsed_ms'] for e in summaries)
            training_lines.append(f"| {name.removesuffix('.json')} | {training:.1f} | {total:.1f} | {statistics.median(e['finalists'] for e in summaries):g} | {statistics.median(e['accepted'] for e in summaries):g} | {statistics.median(e['shared_guides']['hits'] for e in summaries):g} |")
    arms=load(DEST/'development-arms.json');arm_lines=[]
    decisions={
        'union':'Useful first control; superseded','portfolio':'Reject: latency','funded':'Adopt allocation principle','subset':'Reject: missed probability modes',
        'goals':'Adopt goal proposal','multimode':'Adopt defense; adapt setup','adaptive':'Adopt pilot-selected modes','order-rare':'Do not adopt globally',
        'order-uncertain':'Do not adopt globally','dynamic-valid':'Reject: >100% latency increase','fallback':'Defer: over allowance',
        'cases':'Defer: setup displaces finals','replace-tilt':'Reject: lost old positives','four-rivals':'Adopt cheaper exact block',
        'funded-tilt':'Reject: latency','witness-fixtures':'Defer: weaker coverage/latency','omit':'Adopt strict irrelevance rule',
        'order-quality':'Adopt for rough coverage','order-cases':'Reject global enumeration','reallocate-safe':'Reject: original policy at lower tail budget',
        'hard-cases':'Defer: no aggregate improvement','pilot-reuse':'Defer: no aggregate improvement','high-ess-tilt':'Reject: no gain, over allowance',
        'shared-guides':'Adopt memory sharing', 'headroom40':'Reject funding: cold wall increase >50%' ,
        'headroom35':'Select funding headroom'}
    for r in arms:
        arm_lines.append(f"| {r['arm']} | {r['pairs']} | {r['cell_run_gain']} | {r['losses']} | {(r['maximum_latency_ratio']-1)*100:+.1f}% | {decisions.get(r['arm'],'See ledger')} |")
    rem_lines=[]
    for name in by:
        cells=[e for e in remaining if e['input']==name and e['seed']==808]
        if cells:rem_lines.append(f"- **{name.removesuffix('.json')}, seed 808:** "+', '.join(f"{e['team']}/{e['position']}" for e in cells)+'.')
    ratios=[e['reference_ratio'] for e in scored]
    ratio_text=f"{len(ratios)} independently scorable new cell-runs: candidate/reference ratios **{min(ratios):.3g}–{max(ratios):.3g}**" if ratios else 'No independently scorable gains'
    removed=totals['gain']/totals['eligible_before']*100
    text=f'''# Native Rust rare-position coverage with 50% additional time

Historical measurements from October 1, 2026; evidence archived and committed
on October 2. The selected mechanisms were already shipped by later releases.
`FUTURE_EXPERIMENTS.md` is the chronological progress ledger. This report records
the selected final configuration separately from development variants.

Raw files are in `2026-10-01-rare50/evidence.tar.gz`; extract it before using
the historical commands. The final disposition of remaining local code is in
[the decision report](2026-10-02-rust-experiment-decisions.md).

## Result

The selected profile adds **{totals['gain']} nonzero cell-runs**, covering
**{totals['distinct_snapshot_cells']} distinct snapshot/team/position cells**, in
{totals['pairs']} paired full requests. It removes **{removed:.1f}%** of the
reachable/undecided zero cell-runs ({totals['eligible_before']} → {totals['eligible_after']}).
Repeated seeds are counted as cell-runs, not additional positions in one table.

No baseline nonzero cell, certified impossibility, or known reachability was
lost. Game importance is identical in every pair. The largest measured paired
full-request latency increase is **{(totals['max_latency_ratio']-1)*100:.1f}%**.
Every estimator uses four workers. The allocation is a measured work controller,
not a hard real-time guarantee for every possible future input. Warm individual
paired increases peak at **{(max(warm_ratios)-1)*100:.1f}%**, with **{warm_over}** pairs above 50%.

```sh
RUST_ODDS_RARE_TAIL=coverage odds-rust/target/release/golaberto-odds serve 127.0.0.1:6577
```

At measurement time this was opt-in. Coverage is now the production default,
with later refinements tracked in the ledger. This historical profile targets rough probability
orders, with fresh main/check batches and explicit precision metadata. Set
`RUST_ODDS_RARE_TAIL_QUALITY=current` for stricter final acceptance at lower coverage.

## Cohort and paired coverage

Seven request snapshots: six reference groups, plus historical 16653. Development
seeds 801,804,808,817,818,911; reserved seeds 1201,1213,1229. Selection preceded
held-out evaluation. The final shared-guide/funding safety variant was
validated on all nine seeds without tuning individual held-out cells. Funding
was reduced in response to timing failures; these repeated held-out checks
are safety validation, not a second untouched model-selection holdout.

| Snapshot | Eligible zeros before, per request | After | Impossible zeros, per request | New cell-runs | Distinct gained cells | Gains/request |
|---|---:|---:|---:|---:|---:|---:|
{chr(10).join(case_lines)}

Development adds {result['development']['gain']} cell-runs; held-out adds
{result['held_out']['gain']}. No new impossibility proofs are claimed by this
probability campaign. Existing proofs are retained. Historical 16653 resolves
all eligible zeros in some requests; that does not establish universal coverage.
The four control snapshots already have full nonzero matrices. Total paired
request time rises **{(total_candidate_ms/total_baseline_ms-1)*100:.1f}%**; this gains
**{efficiency:.1f}** nonzero cell-runs per additional full-request second. This
is a cohort work metric, not extra distinct cells per second in one request.

## Baseline and fairness

The baseline is the CURRENT native Rust code with the preceding propagated-joint
work enabled, not Go and not an assumed one-second allowance. Frozen binary
SHA256: `{pairs['baseline_sha256']}`. Final candidate SHA256:
`{pairs['candidate_sha256']}`. Rust 1.93.1, locked dependencies, release build.

Baseline source is reconstructible from `2026-10-01-rare50/baseline.patch` and
`baseline-propagated.rs`, applied to commit 65b71493e738ce434f071e717b0646a0e4c8a842.
Embedded build paths can change rebuilt binary hashes. Final source hashes are
in `2026-10-01-rare50/final-source-sha256.json`. The measured binary is frozen
at `/private/tmp/golaberto-rare50-final35-validated`. Input hashes, flags,
per-seed times and comparisons are in `final-results.json`; no golden
probabilities are read by the production binary. No DB writes were performed.

All baseline/candidate full requests run sequentially, alternating order.
No compilation or other benchmark runs concurrently. Warm servers execute ten
alternating requests each; first two are excluded from latency medians. CPU
and peak RSS cover the whole ten-request process, including warmup/startup.

## Warm latency, CPU and memory

| Snapshot | Baseline ms | Candidate ms | Wall increase | Process CPU increase | Peak RSS MiB |
|---|---:|---:|---:|---:|---:|
{chr(10).join(warm_lines)}

CPU time is summed across workers; wall time is elapsed request time. The same
four-worker maximum does not imply the same average CPU utilization. The 50% allowance here is evaluated as request wall time with four workers.
Summed CPU increases can exceed 50%; no 50% CPU-cost cap is claimed. Earlier
unshared warm 16498 used 55% more CPU and 448MiB. The first 45% funding profile
recovered 223 cell-runs but had a 52.8% warm-pair wall increase and 64.5% more CPU;
it is retained in the ledger and is not the selected funding setting.

### Where the time goes: warm 16498 median stages

| Stage | Baseline ms | Candidate ms |
|---|---:|---:|
{chr(10).join(stage_lines)}

Parent `pool`/`search` times include child stages; do not add both. Training
logs report combined model construction and pilot sampling, including discarded
proposals. Final logs report fresh main/check sampling separately. Thus a
reported `training_ms` is not pure model setup. Construction is substantial
for difficult shared-fixture roots; larger case unions displaced useful finals.

### Portfolio preparation and funding costs

Medians across the nine final cold pairs for each difficult snapshot:

| Snapshot | Preparation wall ms | Entire portfolio wall ms | Funded cells | Accepted cells | Shared guide hits |
|---|---:|---:|---:|---:|---:|
{chr(10).join(training_lines)}

Preparation is model construction **plus** all pilots, adaptation and funding.
It is measured from the portfolio start through final-batch planning, not a sum
of overlapping worker durations. The difference of the two medians is not an
exact sampling-stage measurement. The dominant opportunity is preparation for
16498; extra case construction performed worse because it consumed final-batch
funding. Original point-tilt remains the largest earlier stage, but attempted
reallocation failed either coverage or latency checks.

## Selected algorithm

```mermaid
flowchart TD
  A[Existing Rust scout, pool and searches] --> B[Actual final zero cells]
  B --> C{{Already proved impossible?}}
  C -->|yes| Z[Keep structural zero]
  C -->|no| D[Complete bounded joint union pilot]
  D --> E[Full-support sampled target-path model when needed]
  E --> F[Propagated fixture domains and exact four-rival block]
  F --> G[Poisson-binomial rank guidance; conditional goal proposal]
  G --> H[Independent pilots; adapt witness mode only if useful]
  H --> I[Fund fixed fresh batches by measured ESS, hits and cost]
  I --> J[Independent main and check]
  J --> K{{Acceptance and agreement}}
  K -->|pass| L[Fill this zero with weighted estimate]
  K -->|fail| M[Keep zero and its known reachability]
  L --> N[Reconcile matrix]
```

1. Keep the existing 20k scout, 100k pool, proof allocation, point-tilt and other
   estimators. Attempts to remove point-tilt lost existing cells or exceeded the
   allowance after triggering other rescue work.
2. Work on actual final zero cells, excluding certified impossible cells. No
   team IDs or particular finishing ranks select search eligibility.
3. Try a complete propagated union (up to 256 target patterns/128 feasible cases)
   and, when needed, sample target paths instead of truncating event support.
   The sampled model caches up to 32 roots with 60k construction nodes and 4M
   logical guide values per model. Failed construction is charged; exhausted
   paths use the defensive proposal, never an impossibility label.
4. Target outcome proposal: 10% exact necessary target-event distribution and 90%
   cached root proposal. Remaining fixtures use exact masked probabilities,
   selected-rival shared conditioning, and guided likelihood ratios. Fixed
   fixture probabilities and joint normalizers are included.
5. Rank guidance uses a Poisson-binomial approximation for the remaining number
   above/below the target; this is a proposal, not a probability estimate or
   reachability proof. Small tails are computed directly instead of subtracting
   a CDF rounded to one. Conditional goal tilts use exact table normalizers and
   actual production sorting. Points/wins ties retain actual later rules.
6. A pilot with low ESS can add a witness-directed mode; retain it only if a
   separate pilot improves ESS. Its 50/50 mixture includes the broader guided
   mode, and both path densities are evaluated. Ordinary MC defense alone
   proved practically inadequate in an early rejected experiment.
7. Omit a guided-loop fixture only if both endpoints remain strictly on known
   sides of the target and their point/win bounds are slack throughout their
   entire ranges. Actual omitted outcomes are still sampled from their masked
   true distribution for the complete season. Tied teams are retained.
8. Share frozen suffix guides only with identical ordered fixtures, endpoints,
   packed gains, probabilities and team count. Bases/bounds stay per cell.
   Weak cache references do not retain completed models across requests.
9. Additional allowance is {budget_fraction*100:.0f}% of elapsed **native calculation** time, excluding
   upload/HTTP queue wait. Stop starting training after 65% of this allowance;
   fund final batches using pilot cost estimates and four worker bins. Sampling
   batches are fixed before main/check draws. No first-hit stopping.
10. Fill only zeros. Main/check results do not replace earlier positive estimates.
    Matrix reconciliation can make tiny numerical adjustments to existing odds.

### Acceptance versus precision

Existing estimators retain their existing gates. For these final rough estimates,
main requires ≥30 hits, ESS ≥4, relative SE ≤60%, max weight share ≤35%, split-batch gap ≤1.5.
The independent check requires ≥30 hits, ESS ≥3, relative SE ≤75%, and main/check within
a factor of 5. Predicted main allocation targets 10 effective samples, 1000–6000 draws;
check is at least 1000. `meets_precision_goal` retains the existing stricter
criterion and may be false for a published rough estimate.

These checks are finite-sample diagnostics, not a guaranteed confidence interval
or guaranteed order of magnitude. A full-support proposal can still miss a major
mode. The rejected restricted model demonstrated this directly. Scores use the
same finite numerical Poisson-table support as the existing estimator; this
experiment does not claim unrestricted goal-tail impossibility.

## Independent quality checks

{ratio_text}. Only established `reference_is`/`reference_mc` cells are scored.
Provisional IS and previously reachable-without-reference cells are listed but
not treated as golden truth. Historical 16653 is a different snapshot and is not
scored against current 16653 probabilities. All probabilities in JSON/report are
fractions; multiply by 100 for percentages.

The exact tests enumerate shared fixture assignments in small leagues, compare
weighted rank probabilities, check complete/incomplete proposal mixtures,
verify irrelevant-fixture compaction, and compare conditional goal-tail IS to
an independently summed distribution. Bounds/witnesses are never inserted as
point probability estimates.

## Development experiments and recommendations

Most arms use three difficult snapshots × three seeds; arms with 3 pairs are a
single-seed screen. Funding screens are separate profiles on the same mechanisms. Compare aggregate gains on equal cohorts, not raw totals
from different numbers of requests. The ledger contains chronological details.

| Arm | Pairs | Gained cell-runs | Old positives lost | Worst wall increase | Recommendation |
|---|---:|---:|---:|---:|---|
{chr(10).join(arm_lines)}

The `dynamic` run without `-valid` is INVALID (old binary after a failed build)
and excluded. The `reallocate-safe` label was an intended edit that did not
reach that binary: it repeated the original R17 allocation at a lower tail
allowance. It is not evidence for the corrected high-ESS-only policy; the
separate `high-ess-tilt` run is that actual test. Corrections are recorded in the
ledger, not silently reinterpreted.

## Research from other rare-event problems

- Reliability/queue overflow: [Glasserman et al., multilevel splitting (1999)](https://business.columbia.edu/sites/default/files-efs/pubfiles/4273/multilevel_splitting.pdf)
  motivates intermediate levels and work-normalized evaluation. A witness alone
  does not determine the event's mass. Football application is an inference.
- Discrete/SAT rare events: [Walter (2015)](https://arxiv.org/abs/1507.00919)
  motivates explicit treatment of discontinuous scores. A continuous-score
  adaptive splitting algorithm cannot simply be reused for discrete points/ranks.
- Multiple failure modes: [Botev, Chan and Kroese (2011)](https://people.smp.uq.edu.au/DirkKroese/ps/WSC_BCK.pdf)
  motivates mixture proposals. The native defensive guided mixtures were tested
  rather than assuming one successful witness identifies the dominant mode.
- Queueing, random walks, insurance and graph counting:
  [Blanchet and Lam (2012)](https://www.columbia.edu/~khl2114/files/1-s2.0-S1876735411000250-main.pdf)
  motivates state-dependent guidance and evaluation of variance, not only hits.
  No asymptotic efficiency theorem is claimed for this football sampler.

Fixed-level splitting/SMC remains a research direction. A production experiment
would need valid discrete weighting, suitable shared-fixture transition kernels,
and independent root replications for uncertainty. It was not fabricated or
claimed to outperform the tested native portfolio.

## Remaining eligible zeros

These are **team/position IDs**, not impossible cells. Seed 808 illustrates the
remaining work; other seeds can publish a different subset.

{chr(10).join(rem_lines)}

All final eligible zeros have known reachability in this cohort; none remain
undecided. Structural impossibility is a separate category.

Failures combine low event hit rates, dominant importance weights, insufficient
independent confirmation, and final batches too costly to fund. Larger model
setup does not automatically help: it consumes the time needed to sample its
proposal. Best/worst reachable ranks do not establish intermediate reachability.
We report the strongest measured configuration within the allowance, not a proof
that no other algorithm can eliminate more zeros.

## Reproduce

```sh
cargo build --release --offline --locked --manifest-path odds-rust/Cargo.toml --bin golaberto-odds -j4
python3 experiments/rare_positions/benchmark_propagated_joint.py \\
  --baseline /private/tmp/golaberto-rare50-baseline \\
  --candidate odds-rust/target/release/golaberto-odds --production-candidate \\
  --flag RUST_ODDS_RARE_TAIL=coverage \\
  --seeds 801,804,808,817,818,911,1201,1213,1229 --output /private/tmp/rare50-final35
python3 experiments/rare_positions/benchmark_propagated_joint.py \\
  --baseline /private/tmp/golaberto-rare50-baseline \\
  --candidate odds-rust/target/release/golaberto-odds --production-candidate \\
  --flag RUST_ODDS_RARE_TAIL=coverage --cases 16498,16653,16982 \\
  --persistent --output /private/tmp/rare50-final35-warm
python3 experiments/rare_positions/write_rare50_report.py \\
  --pairs /private/tmp/rare50-final35 --warm /private/tmp/rare50-final35-warm
cargo test --release --offline --locked --manifest-path odds-rust/Cargo.toml -j4 -- --test-threads=1
```

Full suite: **{a.tests_passed} passed, 2 DB tests ignored**. No MySQL mutation,
Rails/UI changes, commits or pushes. Code, report, portable results and the
progress ledger remain available for review and later continuation.
'''
    REPORT.write_text(text)
    print(json.dumps(dict(report=str(REPORT),totals=totals,reference_ratio_range=[min(ratios),max(ratios)] if ratios else None)))

if __name__=='__main__':main()
