"""Plan-growth decisions for the adaptive frontier runner."""
import math


def sampling_priority(lo, hi, n, cap, lower_counts, upper_counts, elapsed_ms,
                      batch_size=1000):
    """Estimate reducible CI width per batch cost, without changing any bound."""
    n=int(n);batch_size=max(1,int(batch_size));cap=max(0.0,float(cap or 0.0))
    width=max(0.0,float(hi)-float(lo))
    if n<=0:
        gain=width
        cost=max(1e-9,float(batch_size))
        return gain/cost
    if len(lower_counts)!=241 or len(upper_counts)!=241:
        raise ValueError("priority needs all 241 dyadic threshold counts")
    lower=[int(x) for x in lower_counts];upper=[int(x) for x in upper_counts]
    if any(x<0 or x>n for x in lower+upper):
        raise ValueError("priority threshold count outside current epoch sample size")
    tmin=2.0**-240
    empirical_lower=math.fsum((2.0**-j-2.0**-(j+1))*lower[j]/n for j in range(240)) + tmin*lower[240]/n
    empirical_upper=math.fsum((2.0**-(j-1)-2.0**-j)*upper[j]/n for j in range(1,241)) + tmin
    intrinsic=max(0.0,cap*(empirical_upper-empirical_lower))
    statistical=max(0.0,width-intrinsic)
    gain=statistical*(1.0-math.sqrt(n/(n+batch_size)))
    cost=max(1e-9,float(elapsed_ms)/n*batch_size)
    if statistical==0.0:
        gain=max(gain,width*1e-6/(n+1))
    return gain/cost


def next_plan_request(plan, last_k, last_cap, max_k, max_cap, focused_looks,
                      last_attempt_accepted=False, last_attempt_complete=None):
    """Return monotone (k, state_cap), completing the latest candidate before widening."""
    last_k=max(1,int(last_k)); last_cap=max(1,int(last_cap)); max_k=max(1,int(max_k)); max_cap=max(1,int(max_cap))
    plan=plan or {}
    plan_k=max(1,int(plan.get("k",0) or 1))
    plan_states=max(1,int(plan.get("states",0) or 1))
    complete=bool(plan.get("complete",False))
    k=max(last_k,plan_k)
    cap=max(last_cap,plan_states)
    # A rejected partial candidate does not replace the installed plan. Its
    # own completeness still controls the next attempt: grow that candidate's
    # table before considering a wider proposal.
    candidate_incomplete = last_attempt_complete is False
    if candidate_incomplete:
        at_cap=last_cap>=max_cap
        if at_cap and not last_attempt_accepted and k<max_k:
            return min(max_k,k+1),max_cap
        grown=min(max_cap,max(cap+1,last_cap*2,plan_states*2))
        return min(k,max_k),grown
    if not complete:
        at_cap=last_cap>=max_cap
        if at_cap and not last_attempt_accepted and k<max_k:
            return min(max_k,k+1),max_cap
        grown=min(max_cap,max(cap+1,last_cap*2,plan_states*2))
        return min(k,max_k),grown
    if focused_looks>=1 and k<max_k:
        return min(max_k,k+1),min(max_cap,cap)
    return min(k,max_k),min(max_cap,cap)


def stage_deadline(run_deadline, now, allowance_seconds, cleanup_reserve_seconds):
    return min(run_deadline-cleanup_reserve_seconds,now+allowance_seconds)


def stage_timeout_ms(stage_deadline_at, now, per_call_ms):
    return max(0,int(min(per_call_ms,(stage_deadline_at-now)*1000.0)))
