"""Optional, explicitly trusted SciPy/HiGHS cohort feasibility prescreen."""
from __future__ import annotations
import time

def prescreen(model, time_limit, deadline, milp=None, node_limit=None):
    """Return a log record. Only SciPy status 2 is allowed to prune."""
    started = time.monotonic()
    rec = {"status": "unknown", "message": "", "runtime_seconds": 0.0,
           "infeasibility_assumption": "HiGHS status 2 is trusted as correct"}
    try:
        if milp is None:
            from scipy.optimize import milp
        import numpy as np
        from scipy.optimize import Bounds, LinearConstraint
        from scipy.sparse import lil_matrix
        fs, teams = model["fixtures"], model["team_ids"]
        # Three binary variables per fixture; masks select available W/D/L.
        n = 3 * len(fs); rows = len(fs) + len(teams)
        A = lil_matrix((rows, n), dtype=float)
        lo = np.full(rows, -np.inf); hi = np.full(rows, np.inf)
        c = np.zeros(n); lb = np.zeros(n); ub = np.zeros(n); integrality = np.ones(n)
        idx = {tid: i for i, tid in enumerate(teams)}
        for fi, f in enumerate(fs):
            for o in range(3):
                j = 3 * fi + o
                ub[j] = 1 if int(f["mask"]) & (1 << o) else 0
                A[fi, j] = 1
                A[len(fs) + idx[f["home_id"]], j] = f["outcomes"][o]["home"]
                A[len(fs) + idx[f["away_id"]], j] = f["outcomes"][o]["away"]
            lo[fi] = hi[fi] = 1
        for ti, packed in enumerate(model["base_packed"]):
            row = len(fs) + ti
            for fi, f in enumerate(fs):
                for o in range(3):
                    delta = (f["outcomes"][o]["home"] if idx[f["home_id"]] == ti else 0)
                    delta += (f["outcomes"][o]["away"] if idx[f["away_id"]] == ti else 0)
                    A[row, 3 * fi + o] = delta
            lo[row] = int(model["lower_packed"][ti]) - int(packed)
            hi[row] = int(model["upper_packed"][ti]) - int(packed)
        sparse_constraints = LinearConstraint(A.tocsr(), lo, hi)
        rem = deadline - time.monotonic()
        if rem <= 0:
            raise TimeoutError("shared prescreen budget exhausted")
        import warnings
        options = {"time_limit": min(float(time_limit), rem), "threads": 1}
        if node_limit is not None:
            options["node_limit"] = max(0, int(node_limit))
        # SciPy forwards HiGHS-specific options after warning that they are
        # unrecognized; keep that warning local to this optional call.
        with warnings.catch_warnings():
            warnings.filterwarnings("ignore", message="Unrecognized options detected")
            result = milp(c, integrality=integrality, bounds=Bounds(lb, ub),
                          constraints=sparse_constraints, options=options)
        rec.update(status=int(result.status), message=str(result.message),
                   infeasible=int(result.status) == 2)
        if int(result.status) == 0 and getattr(result, "x", None) is not None:
            chosen=[]
            for fi in range(len(fs)):
                chosen.append(int(np.argmax(result.x[3*fi:3*fi+3])))
            rec["feasible_witness"]={"kind":"packed_wdl_feasible","outcome_indices":chosen,"fixture_count":len(chosen)}
    except Exception as exc:
        rec.update(status="error", message=f"{type(exc).__name__}: {exc}", error=True)
    rec["runtime_seconds"] = time.monotonic() - started
    rec["pruned"] = rec.get("status") == 2
    return rec
