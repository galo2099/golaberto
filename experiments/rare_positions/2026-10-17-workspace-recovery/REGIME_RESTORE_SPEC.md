# Recovery target: historical U/L6.386

The retained treatment interval was `[6.865802419620651e-29,
4.384498731654272e-28]`, U/L6.386, shared180 seconds for all40 cohorts. The first
reconstruction's U/L21.733 is a regression, not completed historical recovery.
This stage restores an actual GD-conditioned proposal rather than only capping
analytic event mass. Original request byte identity remains unavailable.

## Native score conditioning

PairPlan requires fixed WDL on every fixture incident to either compared team,
and uses GdCache to enclose conditional q=P(GD_positive>=GD_negative). It samples
native home/away Poisson goals conditional on each fixed WDL, and rejects the
entire incident-fixture vector unless the necessary inclusive GD condition holds.
Shared fixtures occur once with both endpoint contributions. The accepted law
is the exact native conditional law; its score likelihood correction is q once.
This first implementation uses rejection rather than the missing bridge sampler.
A pre-sampling cost gate q.lo>=0.001 avoids excessively costly supported plans.
A rejection-trial limit returns an error and discards the entire uncommitted
batch. No fallback or retry of a failed batch may silently alter the law.

`PairPlan::build(model,laws,masks,positive,negative,deadline,cache)` returns
None for unsupported preparation; certified zero q is a separate usable zero.
`draw(model,rng)` returns a vector of optional actual fixture score pairs.
`score::integrate_fixed` validates and honors every fixed result once, including
every other affected endpoint, and integrates/samples only remaining scores.
Depth-zero/full-ranking fallback must also respect these scores.

## Generic regime proposal

Each required-above pivot exhaustively partitions compatible incident WDL into
<=64 atoms, with wrong-side packed points/wins atoms excluded by proof. At least
one tied atom is needed. Each atom rebuilds integer propagation and a native
joint WDL plan preserving all selected parent constraints, including target.
Strict-PW atoms use ordinary scores; tied-PW atoms install one necessary pair
GD condition. Unsupported atoms reject the entire candidate partition. Only
certified-zero atoms may be omitted. No products of marginal rival GD events.

An atom's upper weight cap is B_WDL*q.hi for a tied atom, B_WDL otherwise. Choose
atoms by exact integer-ticket categorical weights proportional to these caps.
B_mix >= max_i cap_i/pi_i.lo. A draw's absolute likelihood is
W_WDL*q/pi_i for tied atoms or W_WDL/pi_i otherwise. Never multiply an already
GD-capped analytic event bound by q again. Fixed-score integration yields the
remaining event bracket. All retained atom states share a bounded setup budget.

Accept a complete candidate only when its B_mix is at most0.75 of the installed
proposal cap. Commit a fresh proposal epoch and reset its moments, retaining
previous absolute confidence bounds and analytic caps. Rejection leaves the
installed proposal, epoch and moments unchanged. Raw joint plans remain available
as independent WDL caps for deterministic GD analysis. Ordinary joint replacement
must compare against the effective installed cap and clear a regime only after
successful replacement. Report actual mode, selected rivals, atoms and cap.

## Measurement

Discover every live cohort before focused setup. A separate optional regime
stage uses at most20 seconds inside the same shared180-second allowance,
prioritizes absolute interval width, and gives each candidate at most1000 ms.
Retain uncovered and unsampled caps. All nested source files enter the frozen
hash manifest. Run full new tests and independent source review before a fresh
measured run. Preserve the previous reconstruction as the real runnable control;
historical numbers remain retained references. Never promise a ratio before
measuring it.
