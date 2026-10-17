"""Exact binomial and layer-cake confidence arithmetic for the frontier harness."""
from __future__ import annotations
import math
from scipy.special import betaincinv, betainccinv


QUALIFICATION_POLICY = {
    "name": "absolute-and-log10-precision",
    "version": 1,
    "max_absolute_width": 0.01,
    "max_log10_half_width_fraction": 0.05,
    "log10_scale_floor": 1.0,
}


def qualification_diagnostics(lower: float, upper: float, *, conflicts: bool = False):
    """Evaluate the current interval precision policy and return auditable diagnostics."""
    result = {"policy": dict(QUALIFICATION_POLICY), "lower": lower, "upper": upper,
              "confidence_conflicts": bool(conflicts), "valid": False,
              "absolute_width": None, "log10_lower": None, "log10_upper": None,
              "log10_midpoint_scale": None, "log10_half_width": None,
              "allowed_log10_half_width": None, "qualifies": False}
    if (not isinstance(lower, (int, float)) or not isinstance(upper, (int, float))
            or not math.isfinite(lower) or not math.isfinite(upper)
            or lower < 0 or upper < lower or upper > 1):
        return result
    width = upper - lower
    result["absolute_width"] = width
    result["valid"] = True
    if lower == 0:
        return result
    log_lower, log_upper = math.log10(lower), math.log10(upper)
    scale = -(log_lower + log_upper) / 2.0
    half_width = (log_upper - log_lower) / 2.0
    allowed = QUALIFICATION_POLICY["max_log10_half_width_fraction"] * max(
        QUALIFICATION_POLICY["log10_scale_floor"], scale)
    result.update({"log10_lower": log_lower, "log10_upper": log_upper,
                   "log10_midpoint_scale": scale, "log10_half_width": half_width,
                   "allowed_log10_half_width": allowed})
    result["qualifies"] = bool(not conflicts
        and upper <= lower + QUALIFICATION_POLICY["max_absolute_width"]
        and half_width <= allowed)
    return result


def cp_one_sided(successes: int, trials: int, alpha: float, *, lower: bool) -> float:
    """Exact one-sided Clopper-Pearson endpoint with stable extreme cases."""
    if trials < 0 or not 0 <= successes <= trials or not 0.0 < alpha < 1.0:
        raise ValueError("invalid binomial interval arguments")
    if lower:
        if successes == 0: return 0.0
        value=float(betaincinv(successes, trials-successes+1, alpha))
        return max(0.0, math.nextafter(value, 0.0))
    if successes == trials: return 1.0
    if successes == 0: value=-math.expm1(math.log(alpha) / trials)
    else: value=float(betainccinv(successes+1, trials-successes, alpha))
    return min(1.0, math.nextafter(value, 1.0))


def telescoping_alpha(base: float, cohort_id: int, epoch: int, look: int, events: int) -> float:
    """Summable allocation across stable cohort IDs, epochs, looks, and events."""
    if base <= 0 or min(cohort_id, epoch, look, events) < 0:
        raise ValueError("invalid alpha allocation")
    return base / ((cohort_id+1)*(cohort_id+2) * (epoch+1)*(epoch+2)
                   * (look+1)*(look+2) * max(events, 1))


def threshold_mean_bounds(lower_counts, upper_counts, n: int, alpha: float,
                          thresholds=None):
    """Bracket means in [0,1] from CP bounds at descending dyadic thresholds."""
    if n <= 0:
        raise ValueError("n must be positive")
    if thresholds is None:
        thresholds = [2.0 ** -j for j in range(241)]
    if len(lower_counts) != len(thresholds) or len(upper_counts) != len(thresholds):
        raise ValueError("count arrays must cover all thresholds")
    if not thresholds or thresholds[0] != 1.0 or any(thresholds[i] <= thresholds[i+1] for i in range(len(thresholds)-1)):
        raise ValueError("thresholds must descend from 1")
    m=len(thresholds); a=alpha/(2*m)
    lows=[]; highs=[]
    for cl,cu in zip(lower_counts,upper_counts):
        cl=int(cl);cu=int(cu)
        if not 0 <= cl <= n or not 0 <= cu <= n: raise ValueError("invalid exceedance count")
        lows.append(cp_one_sided(cl,n,a,lower=True))
        highs.append(cp_one_sided(cu,n,a,lower=False))
    lo=0.0; hi=0.0
    # Lower rectangles use the upper endpoint of each dyadic strip.
    for j in range(m-1):
        term=math.nextafter((thresholds[j]-thresholds[j+1])*lows[j],0.0)
        lo=math.nextafter(lo+term,0.0)
    lo=math.nextafter(lo+math.nextafter(thresholds[-1]*lows[-1],0.0),0.0)
    # Upper rectangles use the lower endpoint; the unobserved bottom slab is <= t_min.
    for j in range(1,m):
        term=math.nextafter((thresholds[j-1]-thresholds[j])*highs[j],1.0)
        hi=math.nextafter(hi+term,1.0)
    hi=math.nextafter(hi+thresholds[-1],1.0)
    return max(0.0,lo), min(1.0,hi)


def scaled_absolute_bounds(lower: float, upper: float, cap: float, analytic_upper: float):
    """Convert normalized W/B bounds to absolute contribution bounds."""
    if cap < 0 or analytic_upper < 0 or lower < 0 or upper < lower:
        raise ValueError("invalid bound scaling")
    return max(0.0, math.nextafter(cap*lower, 0.0)), min(analytic_upper, math.nextafter(cap*upper, 1.0))


def intersect_bounds(old_lower: float, old_upper: float, new_lower: float, new_upper: float):
    """Intersect same-event bounds, returning a conservative hull on conflict."""
    lo=max(old_lower,new_lower); hi=min(old_upper,new_upper)
    if lo > hi:
        return min(old_lower,new_lower), max(old_upper,new_upper), True
    return lo,hi,False


def apply_analytic_upper(old_lower: float, old_upper: float, event_upper: float):
    """Intersect a retained absolute confidence interval with a proof upper bound."""
    if not all(math.isfinite(x) for x in (old_lower,old_upper,event_upper)) or not 0 <= old_lower <= old_upper <= 1 or not 0 <= event_upper <= 1:
        raise ValueError("invalid analytic or confidence bound")
    lo,hi,conflict=intersect_bounds(old_lower,old_upper,0.0,event_upper)
    return lo,hi,conflict


def add_epoch_counts(old_lower, old_upper, old_n: int, batch_lower, batch_upper, batch_n: int):
    """Validate and stage a complete batch before mutating epoch statistics."""
    if batch_n <= 0 or old_n < 0 or len(old_lower) != len(old_upper):
        raise ValueError("invalid current confidence state")
    if len(batch_lower) != len(old_lower) or len(batch_upper) != len(old_upper):
        raise ValueError("incomplete threshold count arrays")
    lows=[int(x) for x in batch_lower]; highs=[int(x) for x in batch_upper]
    if any(x < 0 or x > batch_n for x in lows+highs):
        raise ValueError("batch count outside completed sample size")
    return ([int(x)+y for x,y in zip(old_lower,lows)],
            [int(x)+y for x,y in zip(old_upper,highs)], old_n+batch_n)


class EpochStats:
    """Transactional cumulative threshold counts for one fixed proposal epoch."""
    def __init__(self, epoch: int):
        self.epoch=int(epoch); self.n=0; self.cap=None
        self.sum_lower=0.0; self.sum_upper=0.0; self.positive_draws=0
        self.sum_lower_sq=0.0; self.sum_upper_sq=0.0
        self.lower=[0]*241; self.upper=[0]*241

    def reset(self, epoch: int):
        self.__init__(epoch)

    def consume(self, receipt, expected_n: int):
        if int(receipt.get("n",-1)) != expected_n or expected_n <= 0:
            raise ValueError("sample receipt is incomplete")
        if int(receipt.get("epoch",-1)) != self.epoch:
            raise ValueError("sample belongs to another proposal epoch")
        cap=float(receipt.get("B",receipt.get("weight_cap",float("nan"))))
        if not math.isfinite(cap) or cap <= 0:
            raise ValueError("sample receipt has no valid weight cap")
        if self.cap is not None and cap != self.cap:
            raise ValueError("weight cap changed within one proposal epoch")
        sl=float(receipt.get("sum_lower",float("nan"))); su=float(receipt.get("sum_upper",float("nan")))
        ps=int(receipt.get("positive_draws",-1))
        sls=float(receipt.get("sum_lower_sq",float("nan"))); sus=float(receipt.get("sum_upper_sq",float("nan")))
        if not all(math.isfinite(x) for x in (sl,su,sls,sus)) or not 0 <= sl <= su <= expected_n or not 0 <= ps <= expected_n or not 0 <= sls <= sus <= expected_n:
            raise ValueError("sample receipt has invalid raw sums")
        next_lower,next_upper,next_n=add_epoch_counts(
            self.lower,self.upper,self.n,receipt.get("lower_exceedances",[]),
            receipt.get("upper_exceedances",[]),expected_n)
        self.lower,self.upper,self.n,self.cap=next_lower,next_upper,next_n,cap
        self.sum_lower+=sl;self.sum_upper+=su;self.positive_draws+=ps
        self.sum_lower_sq+=sls;self.sum_upper_sq+=sus
        return self.n
