"""Plan-growth decisions for the adaptive frontier runner."""


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
