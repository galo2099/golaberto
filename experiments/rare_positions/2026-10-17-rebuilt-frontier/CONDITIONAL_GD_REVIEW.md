# Conditional GD mixture architecture review

Architecture GO subject to implementation review and validation. This is a new reconstruction of the prior method, not restoration of the historical binary or a guarantee of its 6.386 interval ratio.

A pivot partition must cover every event-compatible incident WDL assignment, with strict-prefix atoms sampled ordinarily and tied-prefix atoms conditioned on the necessary inclusive pair-GD comparison. Wrong-side PW atoms are excluded by proof; incomplete enumeration or an unsupported positive-mass atom rejects the whole operation. A low q cost gate is a preparation gate, never permission to omit mass. Only certified q_hi=0 permits zero omission.

For each tied atom, sample every pair-incident fixture from the same native conditional-WDL law used to compute q, then reject the whole pair score vector unless GD(positive)>=GD(negative). Shared fixtures occur once with both endpoint contributions. Accepted values have the exact conditional law; the geometric trial count is independent of the accepted value. A bounded trial count may return failure but cannot switch to unconditional scores.

On failure discard all uncommitted batch observations and terminate further sampling in that epoch/run, preserving only earlier completed prefix certificates. Never redraw an aborted batch or replace individual failed observations. This stopping policy avoids selectively conditioning the observation stream on computational success, including failures unrelated to simple geometric rejection.

Pair-fixed scorelines must override both shared score sampling and private kernel integration. Every incident endpoint receives its fixed contribution once; a fixture already supplied by the pair plan must not be sampled again or included in an independent GD kernel. Validate supplied fixture indices, WDL agreement and complete incident coverage.

Freeze the mixture and exact integer categorical probabilities after preparation. With raw WDL atom scales B_i and necessary-pair probabilities q_i, use Bmix>=max_i(B_i*q_i_hi/pi_i_lo). The observation likelihood correction is raw_weight*q_i/pi_i exactly once, with outward lower and upper scale factors. Strict atoms have q=1. Proposal selection, WDL sampling and pair-score rejection require fresh random draws. Sequential use of one RNG is valid under the existing PRNG assumption; separate namespaces are optional engineering, not a statistical requirement. The installed effective bound is Bmix; raw WDL plans remain separately available for analytic tightening.

Existing mandatory-GD analytic caps may already contain q. Do not multiply an event bound by the same factor again. Build every proposed atom freshly from the raw WDL parent constraints, without inheriting an old mixture's conditional scales. Either disallow refining an existing mixture or explicitly reconstruct from that unconditioned state.

Only a complete, supported frozen proposal with at least the required bound reduction commits. Commit a fresh sampling epoch and retain earlier absolute event intervals. Rejected setup consumes no sampling epoch or confidence observations. Setup cost shares the same 180-second request budget and must report soft deadline overruns. Cached pair probabilities and cached score kernels may change cost but not the emitted probability law.

Before measurement, tiny-model tests should verify rejection-conditional score frequencies or exact support identities, shared fixture signs, fixed-score integration against exhaustive score enumeration, full mixture expectation and support bound, nonuniform categorical correction, zero/unknown/low-q atoms, failed-batch terminal behavior and preservation of old confidence counts.

## Final source gate

GO for the planned full-budget measurement, subject to the coordinator's reported 26 Rust and 17 Python tests passing. Actual source now uses native independent-Poisson WDL rejection, model-fixture-indexed fixed score vectors, exact q/selection likelihood correction, and fixed-score overrides before private/shared integration. Raw WDL plans are retained separately and included in reported installed state counts and candidate state allowances. Fresh regime acceptance advances the epoch once; repeat rejection and zero-time rejection preserve the installed regime and raw plan.

Reviewed tests exercise a genuinely nonuniform two-atom expectation identity and actual draw support, pair/shared-fixture GD normalizers, fixed-score integration, mandatory accepted installation on the target fixture, sampling in regime mode, raw-plus-atom state counts, and rejected proposal preservation. The nonuniform mixture identity uses q=1 atoms; pair conditioning is validated separately, rather than claiming one exhaustive test covers their entire combination.

Sampling errors terminate the driver's future sampling and do not commit partial receipts. Inner WDL rejection is unbounded and pair-vector rejection has a large fixed cap; native batches have no internal wall deadline. With only a one-second cleanup reserve, a budget overrun remains possible and must be measured openly. This is a resource limitation, not permission to censor or replace draws. GO does not predict the historical 6.386 interval ratio or qualification.
