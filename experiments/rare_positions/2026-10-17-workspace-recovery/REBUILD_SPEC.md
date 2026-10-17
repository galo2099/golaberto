# Rebuilt estimator contract (new implementation)

Reconstruct an offline estimator for the historical Palmeiras 12th cell; do not
claim byte identity with lost code or historical request bytes. Use surviving
`odds-rust/tests/fixtures/group-16498-2026-10-10.json`, team ID16, one-based
position12 (Cell.rank11). Signature:60 points,17 wins,GD27,nine games left;
team18:35 points,9 wins,nine games left. Keep production endpoint unchanged.

## Layout and ownership

New standalone Cargo package `odds-rust/frontier-recovery` depends on the
surviving `golaberto-odds` path library, serde and serde_json. Rebuilding this
small crate must not recompile the production library after every edit.

- Native law/score worker owns `src/law.rs`, `src/score.rs` only.
- Region/DP worker owns `src/engine.rs` only.
- Additional joint-DP worker owns `src/joint.rs` only; engine owner integrates
  this module rather than duplicating its implementation inside engine.rs.
- Harness worker owns Cargo manifest/lock, `src/main.rs`, Python driver/tests
  under `experiments/rare_positions/2026-10-17-rebuilt-frontier`.
- Coordinator owns this prose specification and result/recovery reports.
- One writer per file. All source edits by Luna. No nested delegation.

## Common types and APIs

`law.rs`: public `Interval { pub lo:f64, pub hi:f64 }`, Copy/Clone/Debug,
`Interval::point(f64)`, `.add(Interval)`, `.mul(Interval)`, `.div(Interval)`,
`.mid()`, `.clamp_probability()`. Positive directed arithmetic, exact zeros;
division requires positive denominator lower. Implement `Law { pub outcomes:
[Interval;3], pub pairs:[Vec<Pair>;3] }`, `Pair { pub home:i32,pub away:i32,
pub mass:Interval }`; mass is unconditional joint score probability.
`Law::new(home_mean:f64,away_mean:f64)->Result<Law,String>`.
`categorical(weights:&[f64],rng:&mut golaberto_odds::rng::Rng)
->Result<(usize,Interval),String>` returns selected index and its ACTUAL rational
probability interval. Also expose `categorical_probabilities(weights:&[f64])
->Result<Vec<Interval>,String>` computing the same ticket allocation without RNG.
Use positive integer tickets ceil(weight/max_weight * 2^52), at least1 for each
positive weight, exact int63 rejection modulo total. Zero weights receive0.

Law must reproduce Poisson::new CDF and the accepted int63->f64 grid used by
Rng::float(), not continuous Poisson masses nor adjacent floating CDF differences.
For each CDF threshold binary-search number of accepted integer draws with
(draw as f64)/2^63 <= threshold. Accepted count is 2^63-512; derive/test it.
Construct probability enclosures from integer counts using outward conversion
and division; score pairs are products of independent native goal probabilities.
Outcome encoding0=awaywin,1=draw,2=homewin. Reject unsupported means>32.

`score.rs`: `ScoreCache::new(max_states:usize)`, `.states()->usize`;
`integrate(model:&Model,laws:&[Law],outcomes:&[u8],above:&[usize],target:usize,
depth:usize,rng:&mut Rng,cache:&mut ScoreCache)->Result<Interval,String>`.
Return a conditional lower/upper probability for the exact ABOVE SET event.
First check all points/wins comparisons: wrong strict side implies exact zero.
Group target with every rival tied in points/wins. Sample fixtures touching
two tied teams once. For a fixture touching just one tied team, other endpoint
strict prefix relative to target, integrate up to depth private fixtures per
tied team using cached conditional GD kernels; sample others from native
Poisson samplers by rejection until WDL matches. Independent private kernels
and shared sampled goals preserve fixture correlations. Use positive GD
convolutions, target endpoint sum and products of rival CDF range masses.
Strict GD side products supply a lower; inclusive products supply an upper
covering GD ties/GF/random sort. Do not pretend GD equality is impossible.
If depth0/no integration, use model.standings to resolve the sampled exact
above set including native tie RNG. Cache cap declines integration safely.
All kernel arithmetic outward; score intervals clipped only to proven [0,1].
No score likelihood correction: native rejection gives exact conditional scores.

### Joint plan module split

`joint.rs` exports `pub struct Plan` with public `bound:f64`, `states:usize`,
`complete:bool`, `selected:Vec<usize>`.
`Plan::build(model:&Model,laws:&[Law],masks:&[u8],lower:&[i32],upper:&[i32],
selected:&[usize],stride:i32,state_cap:usize,deadline:Instant)
->Result<Plan,String>` owns all required immutable sampling data.
`Plan::draw(&self,laws:&[Law],rng:&mut Rng)
->Result<(Vec<u8>,Interval),String>` returns WDL outcomes and the enclosed
likelihood ratio of the COMPLETE original WDL law to its actual proposal.
Its bound includes all masked fixtures, including root-fixed fixtures, exactly
once. Engine MUST NOT multiply fixed/root probability again. Plans with no
selected teams provide a native-mask proposal with product p/q bound and zero
memo states. Lower/upper arrays are absolute packed final totals; selected
constraints use model.base packed totals to derive remaining gains. All WDL
increments are [0,stride,3stride+1] home and reverse away.

DP recursively follows all fixture steps in a fixed deterministic order. Integer
suffix min/max prune only impossible selected constraints. Clamp a satisfied
lower requirement to zero. Store nominal potential and certified actual-q
continuation cap per memo node. Unknown successors use their native-mask suffix
bound and nominal potential1, and enter sticky fallback for the entire suffix.
Record that fallback choice on the edge; later memo entries reached through a
different branch must never change this edge's sampler. Deadline/state cap
causes conservative unknown edges, never an unsupported zero or batch rejection.
`bound==0` may mean a completed exact selected-constraint refutation; numerical
underflow must not manufacture it. Weight/C comparisons must tolerate only
provable interval enclosure overlap; any lower likelihood exceeding the cap
is an error. Tests must exercise complete versus partial table identities,
shared fixtures, ticket likelihood correction, state_cap0 and atomic build.

`engine.rs`: own Model,Vec<Law>,Vec<Region>, per-region plans, and ScoreCache.
`Engine::new(request:Request,team_id:i32,position:usize)->Result<Self,String>`.
`Engine::describe(&self)->serde_json::Value` returns target/fixture/root/region
inventory, candidate_slots, uncovered_upper, region rows with `id`, `root_id`,
`opponent_ids`, `impossible`, analytic `upper`, and current plan metadata.
`Engine::prepare(&mut self,id:usize,k:usize,state_cap:usize,time_ms:u64)
->Result<serde_json::Value,String>` builds candidate off to side, commits only
full support and improved B; receipt includes `accepted`, `B`, `states`,
`selected_rival_ids`, `setup_ms`, `complete`, and `epoch`.
`Engine::sample(&mut self,id:usize,n:usize,seed:i64,depth:usize)
->Result<serde_json::Value,String>` returns EXACT completed fixed sample count
`n`, current `epoch`, weight cap `B`, `sum_lower`, `sum_upper`, squared sums,
and exceedance integer counts for PREDECLARED thresholds `[2^-j:j=0..240]`
for both normalized score streams. Arrays are in j ascending order. Lower
counts mean Y_lower>=t; upper counts mean Y_upper>=t, conservatively usable
as an upper for P(Y>t). Also return positive draw count and elapsed_ms.
No outcome-dependent sample truncation. Invalid/overweight observations error
out; never normalize/clamp an uncertified weight to hide a bound failure.

## Region preparation

Support ordinary3/1/0, no bonuses, ranking `pt,w,gd,gf,gp,name` initially, in-group
fixtures only. Use public target_limits::certify and its proven packed total
limits to enumerate target WDL roots. Exact native-law masses, no heuristics
to discard roots. Enumerate all compatible target paths (nine games=19683).
Keep only sound elementary rank relaxations and certificate-allowed totals.
If quota truncates anything, keep conservative uncovered mass; no false zero.

For each surviving target root, enumerate every exact set of `position-1`
above opponents consistent with packed PW min/max. Certified-always-above
must be included, certified-always-below excluded. Outcomes sharing fixtures
are propagated jointly via surviving Domains::condition/condition_bounded,
or equivalent integer fixed-point propagation. Required-above gets lower
PW>=target; required-below gets upper PW<=target. Equality is permitted on
both sides because GD remains unresolved. Only completed exact propagation
refutations can remove events. Preserve root mass exactly once. Every candidate
cohort is a disjoint exact final above-set event, even though its weak PW
conditioning superset overlaps other cohorts. Do not sum weak event masses as
if they were disjoint. Retain analytic bounds and all unsampled cohorts.

Joint conditional DP selects up to k rare required prefix constraints (normally
above rivals), solves them over the SAME shared fixture outcomes with masks.
Use remaining required packed gains capped at0 for lower-bound constraints;
prune only integer suffix min/max impossibility. Process fixed fixtures ahead,
including their prior mass once and campaign gains. Memoize by step and state;
factor fixtures not touching selected teams. Order fixtures deterministically
to expose constrained shared games early. Every unresolved state returns a
native-mask fallback rather than rejecting/truncating support. Sampling that
enters a missing/unprepared state MUST stay in fallback thereafter; otherwise
preparation caps may no longer describe the sampler.

At prepared state choose outcomes proportional to p_nominal * child potential,
using exact ticket categorical. Compute a certified continuation weight cap
C(state)=max_o[p_upper(o)/q_lower(o)*C(child)] with positive directed arithmetic;
terminal accepted C=1, rejected C=0. Unknown suffix cap derives from the exact
native-mask fallback proposal. This bounds weights even with partial tables,
rounding, tiny ticket allocations and early sampling. Roots include fixed
fixture mass once. Sampling multiplies interval p/q and score interval,
normalizes by the declared B upper; reject any truly overweight result.
Unknown is never infeasible; all rejected plans leave previous plan intact.

## Harness, confidence, scheduling

Rust binary arguments REQUEST TEAM_ID POSITION; stdin newline JSON commands
`{"op":"describe"}`, `{"op":"prepare","id":0,"k":7,"state_cap":250000,
"time_ms":500}`, `{"op":"sample","id":0,"n":128,"seed":...,
"depth":6}`, `{"op":"quit"}`. Stdout one JSON receipt/line only, errors in a
structured receipt with no mutation. Diagnostics stderr. Keep binary and all
source frozen during measured run. Python includes startup/setup and all
actions in ONE shared180s budget, best effort; reserve10s cleanup. Initial
prepare small plans for ALL nonzero cohorts, then sample128/1000 discovery
draws before expensive setup. Rerank by absolute uncertainty width. Grow k or
state capacity for widest unresolved events only, keep old absolute confidence
interval after valid same-event proposal replacement; fresh epoch samples.

Simultaneous99%: allocate A=0.01/(6840*max(candidate_slots,1)); cohort IDs use
summable 1/((id+1)(id+2)); epoch and look use respective telescoping allocations.
Allocate each look across lower/upper threshold events with a union bound.
Use exact one-sided Clopper-Pearson via scipy.special betaincinv/betainccinv;
no Gaussian interval or observed-max-as-bound. Threshold lower bound max(t*CPlo)
or positive layercake; upper min(t+(1-t)*CPup) or positive layercake. Count t=0
as well for upper tail; otherwise 2^-240 is a valid omitted lower slab upper.
Confidence must cover adaptive cohort/epoch/look choices. Outward absolute
aggregation, retain unsampled analytic uppers and uncovered mass. Do not sum
parent/child same-event estimates; intersect their absolute intervals. Report
confidence conflicts conservatively, never clip lower to upper silently.

Stop only after a complete update when global L>0,U/L<=10 (user target), or
budget exhausted. Report geometric midpoint only when qualified, otherwise
separate empirical estimate(s) and certified interval. Native score equality
brackets may leave irreducible width: expose it rather than call it sampling
variance. Record actual wall, setup/sampling, per-region draws/bounds, source,
binary/input hashes, exact proof vs unknown, and reconstructed provenance.

## Validation

Tiny-model exhaustive score enumeration must bracket score integrator output
and total conditional expectation, preserving shared fixtures. Native accepted
grid tests atCDFboundary0,1,powersoftwo and terminal rounding. Categorical
ticket support and rational bounds. Exhaustive joint DP probability identities,
sample weight cap, partial DP/full-support fallback, zero-state-cap fallback,
failed candidate atomicity and depth0 native ranking. Confidence tests against
direct binomial tails, no-hit/exact-hit extremes, telescoping budgets, epoch
reset, incomplete-batch preservation and global residual coverage.
Run all new crate tests and changed scope checks, plus fixture signature and
target-limit proof. A reconstruction run must never be labeled the old binary.

## Next historical layer: deterministic GD event caps

`GdCache::probability(model,laws,masks,positive,negative,deadline)` encloses
P(GD_positive >= GD_negative | every incident WDL fixed). It uses native pair
masses, directed positive convolution, and includes shared fixtures once with
the correct signed coefficient (two for a direct meeting). Unfixed outcomes,
capacity exhaustion, and deadlines return no cap. Cache only completed results.

`Engine::tighten(id,state_cap,time_ms)` changes only the analytic event upper
bound, preserving proposal, normalization B, and epoch. For a required-above
rival whose fixed final points AND wins equal the target, its GD comparison is
necessary. Multiply the ORIGINAL WDL plan bound by this conditional GD bound;
never multiply two marginal GD probabilities or multiply an already GD-capped
bound again. A rival strictly above on points/wins needs no GD comparison.

The generic pivot operation exhaustively enumerates compatible incident WDL
patterns, pruning patterns that cannot reach the target's packed points/wins.
Accept at most 64 surviving atoms, with at least one tied atom; abandon an
incomplete partition. Each atom fixes all pivot incident outcomes, propagates
all cohort constraints, and builds a fresh joint WDL cap retaining parent
selected constraints. Strict atoms keep this cap; tied atoms multiply by their
conditional pair-GD bound. Sum disjoint atom caps outward. These child caps
already include the native root probability once. Missing any required cap
abandons that partition, preserving prior completed bounds. Try other pivots
when a candidate fails. No target-team or pivot IDs may be hardcoded.

The harness tries these analytic caps after broad discovery, prioritizing
large current upper bounds, with a shared allowance of at most 20 seconds
inside the existing 180-second request budget. Retain existing samples and
intersect absolute confidence intervals; do not substitute analytic event U
for importance-weight normalization B. Conflicts remain visible and disable
qualification. Record every cap receipt and include scheduler.py in the source
hash manifest. Compare a fresh frozen run against the recorded primary run;
no source edits during measurement.
