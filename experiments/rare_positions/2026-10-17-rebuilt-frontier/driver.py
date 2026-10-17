#!/usr/bin/env python3
"""Budgeted adaptive driver for the independently rebuilt frontier estimator."""
from __future__ import annotations
import argparse, hashlib, json, math, os, subprocess, sys, time
from pathlib import Path
IMPORT_STARTED=time.monotonic()
from confidence import telescoping_alpha, threshold_mean_bounds, scaled_absolute_bounds, intersect_bounds, apply_analytic_upper, EpochStats
from trusted_highs import prescreen as trusted_highs_prescreen
from scheduler import next_plan_request, stage_deadline, stage_timeout_ms, sampling_priority, rebuild_request

HERE = Path(__file__).resolve().parent
DEFAULT_REQUEST = HERE.parents[2] / "odds-rust/tests/fixtures/group-16498-2026-10-10.json"

def accept_regime_proposal(state, receipt):
    """Install accepted regime metadata; reset only the new proposal epoch stats."""
    if not receipt.get("accepted"):
        return False
    epoch=int(receipt.get("epoch",state["epoch"]))
    selected=receipt.get("selected_rival_ids",[])
    state["current_plan"]={"B":receipt.get("B"),"states":receipt.get("states"),
                            "k":len(selected),"selected_rival_ids":selected,
                            "complete":receipt.get("complete"),"epoch":epoch,
                            "accepted":True,"mode":receipt.get("mode","regime"),
                            "pivot_id":receipt.get("pivot_id"),"atom_count":receipt.get("atom_count")}
    state["last_attempt_k"]=len(selected)
    state["last_attempt_cap"]=int(receipt.get("states",state.get("last_attempt_cap",1)))
    state["last_attempt_accepted"]=True
    state["last_attempt_complete"]=bool(receipt.get("complete",False))
    if epoch != state["epoch"]:
        state["epoch"]=epoch
        state["look"]=0
        state["focused_looks"]=0
        state["epoch_sampling_ms"]=0.0
        state["stats"].reset(epoch)
    return True

def rebuild_after_joint_growth(engine, rid, state, state_caps, args, deadline,
                               setup_totals, now=time.monotonic):
    """Try at most one regime proposal for this accepted joint-plan epoch."""
    plan=state.get("current_plan") or {}
    parent_epoch=int(plan.get("epoch",state["epoch"]))
    attempted=state.setdefault("rebuild_parent_epochs",set())
    if (not args.regimes or not args.regime_rebuilds or not plan.get("accepted")
            or not plan.get("complete") or plan.get("mode","joint")!="joint"
            or float(plan.get("B",0.0))<=0.0 or parent_epoch in attempted):
        return None
    attempted.add(parent_epoch)
    old_states=int(state_caps.get(rid,0))
    others=sum(state_caps.values())-old_states
    cap,timeout_ms,reason=rebuild_request(args.state_cap,others,4_000_000,250_000,
                                           args.time_ms,deadline,now(),args.cleanup_reserve)
    if cap is not None and cap<=old_states:
        reason="no_growth_state_budget"
    record={"trigger":"after_joint_growth","parent_epoch":parent_epoch,
            "k":int(plan.get("k",0)),"state_cap":cap,"time_ms":timeout_ms,
            "reason":reason}
    if reason:
        record["skipped"]=True
        state["regime_attempts"].append(record)
        state["regime_status"]="skipped_"+reason
        return record
    start=now()
    try:
        receipt=engine.call(op="regime",id=rid,state_cap=cap,time_ms=timeout_ms)
    except TimeoutError as exc:
        record.update({"error":str(exc),"setup_failure":True})
        state["regime_status"]="protocol_timeout"
        state["regime_attempts"].append(record)
        elapsed=(now()-start)*1000.0
        state["setup_ms"]+=elapsed;setup_totals["setup_ms"]+=elapsed;setup_totals["budget_ms"]+=elapsed
        return record
    except (RuntimeError,ValueError) as exc:
        record.update({"error":str(exc),"setup_failure":True})
        state["regime_status"]="setup_failure"
        state["regime_attempts"].append(record)
        elapsed=(now()-start)*1000.0
        state["setup_ms"]+=elapsed;setup_totals["setup_ms"]+=elapsed;setup_totals["budget_ms"]+=elapsed
        return record
    receipt={**record,**receipt}
    state["regime_attempts"].append(receipt)
    setup=float(receipt.get("setup_ms",0.0))
    state["setup_ms"]+=setup
    setup_totals["setup_ms"]+=setup
    setup_totals["budget_ms"]+=(now()-start)*1000.0
    state["regime_status"]="accepted" if receipt.get("accepted") else ("complete_rejected" if receipt.get("complete") else "partial_rejected")
    if accept_regime_proposal(state,receipt):
        state_caps[rid]=int(receipt.get("states",old_states))
        if float(receipt.get("B",1.0))==0.0:
            state["row"]["upper"]=0.0
            state["proof_status"]="exact_zero"
            state["lo"],state["hi"],conflict=intersect_bounds(state["lo"],state["hi"],0.0,0.0)
            state["conflict"]=state.get("conflict",False) or conflict
    return receipt

class Protocol:
    def __init__(self, binary, request, team, position, deadline):
        self.proc = subprocess.Popen([str(binary), str(request), str(team), str(position)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
        self.deadline=deadline
    def call(self, **obj):
        if time.monotonic() >= self.deadline: raise TimeoutError("shared run budget exhausted")
        self.proc.stdin.write(json.dumps(obj,separators=(",",":"))+"\n"); self.proc.stdin.flush()
        remaining=max(.001,self.deadline-time.monotonic())
        import selectors
        sel=selectors.DefaultSelector(); sel.register(self.proc.stdout,selectors.EVENT_READ)
        if not sel.select(remaining): raise TimeoutError("shared run budget exhausted")
        line=self.proc.stdout.readline()
        if not line: raise RuntimeError("estimator exited without a receipt")
        rec=json.loads(line)
        if "error" in rec: raise RuntimeError(rec["error"])
        return rec
    def close(self):
        try: self.call(op="quit")
        except Exception: pass
        self.proc.terminate(); self.proc.wait(timeout=2)

def sha(path):
    h=hashlib.sha256()
    with open(path,"rb") as f:
        for b in iter(lambda:f.read(1<<20),b""): h.update(b)
    return h.hexdigest()

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--binary",required=True); p.add_argument("--request",default=str(DEFAULT_REQUEST)); p.add_argument("--team-id",type=int,default=16); p.add_argument("--position",type=int,default=12)
    p.add_argument("--budget",type=float,default=180); p.add_argument("--cleanup-reserve",type=float,default=10.0,help="seconds reserved for shutdown and result output (default: 10)"); p.add_argument("--seed",type=int,default=20261017); p.add_argument("--workers",type=int,default=1)
    p.add_argument("--initial-draws",type=int,default=128); p.add_argument("--max-draws",type=int,default=100000); p.add_argument("--initial-state-cap",type=int,default=1000); p.add_argument("--state-cap",type=int,default=250000); p.add_argument("--time-ms",type=int,default=500); p.add_argument("--depth",type=int,default=6); p.add_argument("--initial-k",type=int,default=1); p.add_argument("--max-k",type=int,default=12); p.add_argument("--output")
    p.add_argument("--gd-bounds",dest="gd_bounds",action="store_true",default=True); p.add_argument("--no-gd-bounds",dest="gd_bounds",action="store_false")
    p.add_argument("--regimes",dest="regimes",action="store_true",default=True); p.add_argument("--no-regimes",dest="regimes",action="store_false")
    p.add_argument("--no-regime-rebuilds",dest="regime_rebuilds",action="store_false",default=True,help="disable regime rebuilds after joint plan growth")
    p.add_argument("--trusted-highs",action="store_true",help="trust HiGHS status 2 to prescreen infeasible cohorts")
    p.add_argument("--highs-budget",type=float,default=10.0,help="shared HiGHS setup and solve budget in seconds")
    p.add_argument("--highs-time-limit",type=float,default=.25,help="per-cohort HiGHS time limit in seconds")
    p.add_argument("--highs-node-limit",type=int,help="optional HiGHS node limit")
    p.add_argument("--run-full-budget",action="store_true",help="continue focused sampling after the target ratio is reached")
    a=p.parse_args()
    if not 0 <= a.cleanup_reserve < a.budget: p.error("--cleanup-reserve must be at least 0 and less than --budget")
    if not math.isfinite(a.highs_budget) or a.highs_budget < 0: p.error("--highs-budget must be finite and nonnegative")
    if not math.isfinite(a.highs_time_limit) or a.highs_time_limit <= 0: p.error("--highs-time-limit must be finite and positive")
    if a.highs_node_limit is not None and a.highs_node_limit < 0: p.error("--highs-node-limit must be nonnegative")
    started=IMPORT_STARTED; deadline=started+a.budget; req=Path(a.request).resolve(); binary=Path(a.binary).resolve()
    if a.workers != 1: print("frontier-recovery uses a serial native engine; recording workers=1",file=sys.stderr)
    setup_start=time.monotonic()
    # Startup is inside the same deadline and hashes freeze the measured inputs.
    hsrc=hashlib.sha256()
    source_paths=sorted(HERE.glob("*.py"))+[binary,HERE.parents[2]/"odds-rust/frontier-recovery/Cargo.toml"]
    crate=HERE.parents[2]/"odds-rust/frontier-recovery"
    source_paths += sorted(crate.rglob("*.rs"))
    if (crate/"Cargo.lock").exists(): source_paths.append(crate/"Cargo.lock")
    for path in sorted(source_paths):
        if path.exists(): hsrc.update(str(path).encode()); hsrc.update(bytes.fromhex(sha(path)))
    source_hash=hsrc.hexdigest(); binary_hash=sha(binary)
    source_file_hashes={str(path):sha(path) for path in source_paths if path.exists()}
    input_hash=sha(req); engine=Protocol(binary,req,a.team_id,a.position,deadline)
    try:
        desc=engine.call(op="describe")
        setup_seconds=time.monotonic()-setup_start
        rows=desc.get("regions",[]); candidates=[r for r in rows if not r.get("impossible",False) and float(r.get("upper",0))>0]
        highs_logs=[]; highs_pruned=set(); prescreen_protocol_failed=False; highs_started=time.monotonic(); highs_deadline=min(deadline-a.cleanup_reserve,highs_started+a.highs_budget)
        if a.trusted_highs:
            for row in candidates:
                if time.monotonic() >= highs_deadline: break
                rid=int(row["id"])
                prescreen_call_started=time.monotonic()
                try:
                    model=engine.call(op="feasibility_model",id=rid)
                    rec=trusted_highs_prescreen(model,a.highs_time_limit,highs_deadline,node_limit=a.highs_node_limit)
                except Exception as exc:
                    rec={"status":"error","message":f"{type(exc).__name__}: {exc}","runtime_seconds":time.monotonic()-prescreen_call_started,"infeasibility_assumption":"HiGHS status 2 trusted as correct","pruned":False}
                    if isinstance(exc, TimeoutError):
                        prescreen_protocol_failed=True
                rec["id"]=rid; highs_logs.append(rec)
                if prescreen_protocol_failed: break
                if rec.get("pruned") is True: highs_pruned.add(rid)
            for row in candidates:
                if int(row["id"]) in highs_pruned:
                    row["upper"]=0.0; row["_exact_zero"]=True; row["_proof_status"]="trusted_highs_infeasible"; row["_trusted_highs_log"]=next(x for x in highs_logs if x["id"]==int(row["id"]))
        highs_elapsed=time.monotonic()-highs_started if a.trusted_highs else 0.0
        state_caps={int(r["id"]):int((r.get("plan") or {}).get("states",0)) for r in candidates}
        # Establish at least a bounded fallback plan and discovery sample for every cohort.
        for row in candidates:
            if prescreen_protocol_failed: break
            if time.monotonic()>=deadline-a.cleanup_reserve: break
            if row.get("_exact_zero"): continue
            rid=int(row["id"])
            try:
                others=sum(state_caps.values())-state_caps.get(rid,0)
                initial_cap=min(a.state_cap,a.initial_state_cap,max(0,4_000_000-others))
                if initial_cap<=0: row["_first_sample"]=None; continue
                row["_last_attempt_k"],row["_last_attempt_cap"]=a.initial_k,initial_cap
                try: prep=engine.call(op="prepare",id=rid,k=a.initial_k,state_cap=initial_cap,time_ms=min(a.time_ms,100))
                except RuntimeError as exc:
                    prep={"error":str(exc)}
                    row.setdefault("_prepare_attempts",[]).append({"k":a.initial_k,"state_cap":initial_cap,"accepted":False,"error":str(exc)})
                else:
                    row.setdefault("_prepare_attempts",[]).append({"k":a.initial_k,"state_cap":initial_cap,**prep})
                row["_prepare_setup_ms"]=prep.get("setup_ms",0)
                if prep.get("accepted"):
                    row["_plan_states"]=int(prep.get("states",0));state_caps[rid]=row["_plan_states"]
                    row["_current_plan"]={"B":prep.get("B"),"states":prep.get("states"),"k":len(prep.get("selected_rival_ids",[])),"selected_rival_ids":prep.get("selected_rival_ids",[]),"complete":prep.get("complete"),"epoch":prep.get("epoch"),"accepted":True,"mode":prep.get("mode","joint")}
                    if float(prep.get("B",1.0))==0.0:
                        row["upper"]=0.0;row["_exact_zero"]=True;row["_proof_status"]="exact_zero";row["_first_sample"]=None
                        continue
                row["_plan_epoch"]=int(prep.get("epoch",0))
            except TimeoutError as exc:
                row["_first_sample"]=None;row.setdefault("_prepare_attempts",[]).append({"k":a.initial_k,"state_cap":initial_cap,"accepted":False,"timeout":str(exc)})
            except RuntimeError as exc:
                row["_first_sample"]=None;row.setdefault("_prepare_attempts",[]).append({"k":a.initial_k,"state_cap":initial_cap,"accepted":False,"error":str(exc)})
        # Only after every reachable cohort has had its small preparation attempt,
        # collect the cheap discovery batch for all of them before focused growth.
        for row in candidates:
            if prescreen_protocol_failed: break
            if time.monotonic()>=deadline-a.cleanup_reserve: break
            if row.get("_exact_zero"): continue
            rid=int(row["id"])
            try:
                first=engine.call(op="sample",id=rid,n=a.initial_draws,seed=a.seed+rid,depth=a.depth)
                row["_first_sample"]=first
                row["_plan_epoch"]=int(first.get("epoch",row.get("_plan_epoch",0)))
            except (TimeoutError,RuntimeError) as exc:
                row["_first_sample"]=None;row.setdefault("_sample_errors",[]).append(str(exc))
        state_caps.update({int(r["id"]):int(r.get("_plan_states",state_caps.get(int(r["id"]),0))) for r in candidates})
        states={int(r["id"]):{"row":r,"epoch":int(r.get("_plan_epoch",0)),"look":0,"focused_looks":0,"n":0,"stats":EpochStats(int(r.get("_plan_epoch",0))),"lo":0.,"hi":float(r.get("upper",1.)),"samples":[],"setup_ms":r.get("_prepare_setup_ms",0),"sampling_ms":0,"epoch_sampling_ms":0.0,"last_attempt_k":int(r.get("_last_attempt_k",-1)),"last_attempt_cap":int(r.get("_last_attempt_cap",-1)),"last_attempt_accepted":bool((r.get("_prepare_attempts") or [{}])[-1].get("accepted",False)),"last_attempt_complete":(bool((r.get("_prepare_attempts") or [{}])[-1]["complete"]) if "complete" in (r.get("_prepare_attempts") or [{}])[-1] else None),"conflict":False,"sample_errors":list(r.get("_sample_errors",[])),"prepare_attempts":list(r.get("_prepare_attempts",[])),"current_plan":r.get("_current_plan",r.get("plan")),"proof_status":r.get("_proof_status","unknown"),"gd_bounds":None,"gd_bounds_status":"pending" if a.gd_bounds else "disabled","gd_bounds_attempts":[],"regime_attempts":[],"regime_status":"pending" if a.regimes else "disabled","rebuild_parent_epochs":set()} for r in candidates}
        for rid,s in states.items():
            first=s["row"].get("_first_sample")
            if first and int(first.get("n",-1))==a.initial_draws:
                try:
                    s["stats"].consume(first,a.initial_draws)
                    alpha=telescoping_alpha(.01/(6840*max(int(desc.get("candidate_slots",1)),1)),rid,s["epoch"],s["look"],1)
                    ll,ul=threshold_mean_bounds(s["stats"].lower,s["stats"].upper,s["stats"].n,alpha)
                    cap=s["stats"].cap
                    ll,ul=scaled_absolute_bounds(ll,ul,cap,float(s["row"].get("upper",1.)))
                    s["n"]+=a.initial_draws;s["look"]+=1;s["sampling_ms"]+=first.get("elapsed_ms",0)
                    s["epoch_sampling_ms"]+=first.get("elapsed_ms",0)
                    s["lo"],s["hi"],conflict=intersect_bounds(s["lo"],s["hi"],ll,ul)
                    s["conflict"]=s.get("conflict",False) or conflict
                    s["samples"].append({"batch_n":a.initial_draws,"epoch_n":s["stats"].n,"epoch":s["epoch"],"mode":first.get("mode",(s.get("current_plan") or {}).get("mode")),"sum_lower":first["sum_lower"],"sum_upper":first["sum_upper"],"positive_draws":first["positive_draws"],"sum_lower_sq":first["sum_lower_sq"],"sum_upper_sq":first["sum_upper_sq"],"lower_absolute":ll,"upper_absolute":ul,"B":cap,"setup_ms":first.get("setup_ms",0),"elapsed_ms":first.get("elapsed_ms",0)})
                except ValueError as exc: s["sample_errors"].append(str(exc))
        gd_started=time.monotonic();gd_setup_ms=0.0;gd_budget_ms=0.0
        if a.gd_bounds and not prescreen_protocol_failed:
            gd_deadline=stage_deadline(deadline,gd_started,20.0,a.cleanup_reserve)
            for rid,s in sorted(states.items(),key=lambda kv:float(kv[1]["row"].get("upper",0.0)),reverse=True):
                if float(s["row"].get("upper",0.0))<=0:
                    s["gd_bounds_status"]="already_zero"
                    continue
                remaining=(gd_deadline-time.monotonic())*1000.0
                if remaining<=0: break
                call_started=time.monotonic()
                request_ms=stage_timeout_ms(gd_deadline,call_started,min(a.time_ms,500))
                if request_ms<=0: break
                try:
                    receipt=engine.call(op="tighten",id=rid,state_cap=min(a.state_cap,250_000),time_ms=request_ms)
                    if "event_upper" not in receipt: raise ValueError("tighten receipt missing event_upper")
                    cap=float(receipt["event_upper"])
                    if not math.isfinite(cap) or not 0<=cap<=1: raise ValueError("tighten receipt has invalid event_upper")
                    old_upper=float(s["row"].get("upper",1.0));new_upper=min(old_upper,cap)
                    lo,hi,conflict=apply_analytic_upper(s["lo"],s["hi"],new_upper)
                    s["row"]["upper"]=new_upper;s["lo"],s["hi"]=lo,hi
                    s["conflict"]=s.get("conflict",False) or conflict
                    s["gd_bounds"]=receipt
                    s["gd_bounds_status"]="complete" if receipt.get("complete") else "partial"
                    if new_upper==0.0: s["proof_status"]="exact_zero"
                except (RuntimeError,ValueError,TimeoutError) as exc:
                    receipt={"error":str(exc),"state_cap":min(a.state_cap,250_000),"time_ms":request_ms}
                    s["gd_bounds_status"]="error"
                    s["gd_bounds_attempts"].append(receipt)
                else:
                    s["gd_bounds_attempts"].append(receipt)
                    gd_setup_ms+=float(receipt.get("setup_ms",0.0))
                gd_budget_ms+=(time.monotonic()-call_started)*1000.0
        regime_started=time.monotonic();regime_setup_ms=0.0;regime_budget_ms=0.0
        rebuild_totals={"setup_ms":0.0,"budget_ms":0.0};protocol_failed=prescreen_protocol_failed
        if a.regimes and not protocol_failed:
            regime_deadline=stage_deadline(deadline,regime_started,20.0,a.cleanup_reserve)
            for rid,s in sorted(states.items(),key=lambda kv:kv[1]["hi"]-kv[1]["lo"],reverse=True):
                if s["proof_status"]=="exact_zero" or float(s["row"].get("upper",0.0))<=0:
                    s["regime_status"]="skipped_zero"
                    continue
                remaining=(regime_deadline-time.monotonic())*1000.0
                if remaining<=0: break
                others=sum(state_caps.values())-state_caps.get(rid,0)
                policy_cap=max(0,min(a.state_cap,250_000,4_000_000-others))
                if policy_cap<=0:
                    s["regime_status"]="skipped_state_budget"
                    continue
                request_ms=min(1000,max(0,int(remaining)))
                call_started=time.monotonic()
                try:
                    receipt=engine.call(op="regime",id=rid,state_cap=policy_cap,time_ms=request_ms)
                    s["regime_attempts"].append(receipt)
                    s["regime_status"]="accepted" if receipt.get("accepted") else ("complete_rejected" if receipt.get("complete") else "partial_rejected")
                    regime_setup_ms+=float(receipt.get("setup_ms",0.0))
                    if accept_regime_proposal(s,receipt):
                        state_caps[rid]=int(receipt.get("states",state_caps.get(rid,0)))
                        if float(receipt.get("B",1.0))==0.0:
                            s["row"]["upper"]=0.0
                            s["proof_status"]="exact_zero"
                            s["lo"],s["hi"],conflict=intersect_bounds(s["lo"],s["hi"],0.0,0.0)
                            s["conflict"]=s.get("conflict",False) or conflict
                    s["setup_ms"]+=float(receipt.get("setup_ms",0.0))
                except (RuntimeError,TimeoutError,ValueError) as exc:
                    s["regime_status"]="error"
                    s["regime_attempts"].append({"error":str(exc),"state_cap":policy_cap,"time_ms":request_ms})
                    elapsed=(time.monotonic()-call_started)*1000.0
                    s["setup_ms"]+=elapsed;regime_setup_ms+=elapsed
                    if isinstance(exc, TimeoutError):
                        regime_budget_ms+=elapsed
                        protocol_failed=True
                        break
                regime_budget_ms+=(time.monotonic()-call_started)*1000.0
        # Each focused batch is incorporated only after a complete receipt arrives.
        while not protocol_failed and time.monotonic()<deadline-a.cleanup_reserve and states:
            total_lo=max(0.0,math.nextafter(math.fsum(s["lo"] for s in states.values()),0.0))
            raw_hi=math.fsum([*(s["hi"] for s in states.values()),float(desc.get("uncovered_upper",0))])
            total_hi=min(1.0,0.0 if raw_hi==0.0 else math.nextafter(raw_hi,math.inf))
            if not a.run_full_budget and total_lo>0 and total_hi/total_lo<=10: break
            active=[]
            for item in states.items():
                rid0,s0=item
                others=sum(state_caps.values())-state_caps.get(rid0,0)
                available=min(a.state_cap,4_000_000-others)
                if s0["proof_status"]!="exact_zero" and s0["n"]<a.max_draws and s0["hi"]>s0["lo"] and available>0 and available>=s0["last_attempt_cap"]: active.append(item)
            if not active: break
            def priority(item):
                _,candidate=item
                next_batch=min(1000,max(1,a.max_draws-candidate["n"]))
                return sampling_priority(candidate["lo"],candidate["hi"],candidate["stats"].n,
                                         candidate["stats"].cap,candidate["stats"].lower,
                                         candidate["stats"].upper,candidate["epoch_sampling_ms"],next_batch)
            rid,s=max(active,key=lambda kv:(priority(kv),kv[1]["hi"]-kv[1]["lo"]))
            others=sum(state_caps.values())-state_caps.get(rid,0)
            allowed=max(0,4_000_000-others)
            max_plan_cap=min(a.state_cap,allowed)
            requested_k,plan_state_cap=next_plan_request(s.get("current_plan"),s["last_attempt_k"],s["last_attempt_cap"],a.max_k,max_plan_cap,s["focused_looks"],s["last_attempt_accepted"],s["last_attempt_complete"])
            batch=min(1000,a.max_draws-s["n"])
            phase="prepare"
            try:
                # Increasing preparation complexity is local to the current widest cohort.
                if (requested_k,plan_state_cap)!=(s["last_attempt_k"],s["last_attempt_cap"]):
                    s["last_attempt_k"],s["last_attempt_cap"]=requested_k,plan_state_cap
                    try:
                        prep=engine.call(op="prepare",id=rid,k=requested_k,state_cap=plan_state_cap,time_ms=min(a.time_ms,1000))
                        s["last_attempt_accepted"]=bool(prep.get("accepted",False))
                        s["last_attempt_complete"]=bool(prep.get("complete",False))
                        s["setup_ms"] += prep.get("setup_ms",0)
                        s["prepare_attempts"].append({"k":requested_k,"state_cap":plan_state_cap,**prep})
                    except RuntimeError as exc:
                        prep={"accepted":False,"epoch":s["epoch"]}
                        s["last_attempt_accepted"]=False
                        s["last_attempt_complete"]=False
                        s["sample_errors"].append(str(exc))
                        s["prepare_attempts"].append({"k":requested_k,"state_cap":plan_state_cap,"accepted":False,"error":str(exc)})
                else: prep={"accepted":False,"epoch":s["epoch"]}
                if prep.get("accepted"):
                    state_caps[rid]=int(prep.get("states",state_caps.get(rid,0)))
                    s["current_plan"]={"B":prep.get("B"),"states":prep.get("states"),"k":len(prep.get("selected_rival_ids",[])),"selected_rival_ids":prep.get("selected_rival_ids",[]),"complete":prep.get("complete"),"epoch":prep.get("epoch"),"accepted":True,"mode":prep.get("mode","joint")}
                    if int(prep.get("epoch",s["epoch"])) != s["epoch"]:
                        s["epoch"]=int(prep["epoch"]); s["look"]=0; s["focused_looks"]=0; s["epoch_sampling_ms"]=0.0; s["stats"].reset(s["epoch"])
                    if float(prep.get("B",1.0))==0.0:
                        s["row"]["upper"]=0.0
                        s["proof_status"]="exact_zero"
                        s["lo"],s["hi"],conflict=intersect_bounds(s["lo"],s["hi"],0.0,0.0)
                        s["conflict"]=s.get("conflict",False) or conflict
                        continue
                if prep.get("accepted"):
                    rebuild_after_joint_growth(engine,rid,s,state_caps,a,deadline,rebuild_totals)
                    if s.get("regime_status")=="protocol_timeout":
                        protocol_failed=True
                        break
                    if s.get("proof_status")=="exact_zero":
                        continue
                if time.monotonic()>=deadline-a.cleanup_reserve:
                    break
                phase="sample"
                rec=engine.call(op="sample",id=rid,n=batch,seed=a.seed+rid+1000003*s["epoch"]+s["look"],depth=a.depth)
            except TimeoutError as exc:
                s["sample_errors"].append(str(exc))
                if phase=="prepare":
                    s["last_attempt_accepted"]=False;s["last_attempt_complete"]=False
                    s["prepare_attempts"].append({"k":requested_k,"state_cap":plan_state_cap,"accepted":False,"complete":False,"timeout":str(exc)})
                break
            except RuntimeError as exc:
                s["sample_errors"].append(str(exc));break
            if int(rec.get("n",-1)) != batch: break
            # Receipts expose normalized score streams and threshold counts.
            try: epoch_n=s["stats"].consume(rec,batch)
            except ValueError as exc:
                s["sample_errors"].append(str(exc));s["conflict"]=True;break
            alpha=telescoping_alpha(.01/(6840*max(int(desc.get("candidate_slots",1)),1)),rid,s["epoch"],s["look"],1)
            ll,ul=threshold_mean_bounds(s["stats"].lower,s["stats"].upper,epoch_n,alpha)
            cap=s["stats"].cap
            ll,ul=scaled_absolute_bounds(ll,ul,cap,float(s["row"].get("upper",1.)))
            # Intersect valid same-event intervals; preserve their hull on conflict.
            s["lo"],s["hi"],conflict=intersect_bounds(s["lo"],s["hi"],ll,ul)
            s["conflict"]=s.get("conflict",False) or conflict
            s["n"] += batch; s["look"] += 1; s["focused_looks"] += 1; s["sampling_ms"] += rec.get("elapsed_ms",0); s["epoch_sampling_ms"] += rec.get("elapsed_ms",0)
            s["samples"].append({"batch_n":batch,"epoch_n":epoch_n,"epoch":s["epoch"],"mode":rec.get("mode",(s.get("current_plan") or {}).get("mode")),"sum_lower":rec["sum_lower"],"sum_upper":rec["sum_upper"],"positive_draws":rec["positive_draws"],"sum_lower_sq":rec["sum_lower_sq"],"sum_upper_sq":rec["sum_upper_sq"],"lower_absolute":ll,"upper_absolute":ul,"B":cap,"setup_ms":rec.get("setup_ms",0),"elapsed_ms":rec.get("elapsed_ms",0)})
        engine.close(); engine=None
        total_lo=max(0.0,math.nextafter(math.fsum(s["lo"] for s in states.values()),0.0))
        raw_hi=math.fsum([*(s["hi"] for s in states.values()),float(desc.get("uncovered_upper",0))])
        total_hi=min(1.0,0.0 if raw_hi==0.0 else math.nextafter(raw_hi,math.inf))
        conflicts=any(s.get("conflict",False) for s in states.values())
        qualified=bool(not conflicts and total_lo>0 and total_hi/total_lo<=10)
        region_results=[]
        for rid,s in states.items():
            stats=s["stats"]
            empirical=None if stats.n==0 or stats.cap is None else {
                "absolute_lower_mean":stats.cap*stats.sum_lower/stats.n,
                "absolute_upper_mean":stats.cap*stats.sum_upper/stats.n,
                "interpretation":"separate empirical means of the lower and upper score streams; not a single point estimate",
            }
            region_results.append({"id":rid,"root_id":s["row"].get("root_id"),"target_points":s["row"].get("target_points"),"target_wins":s["row"].get("target_wins"),"opponent_ids":s["row"].get("opponent_ids",[]),"analytic_upper":s["row"].get("upper"),"lower":s["lo"],"upper":s["hi"],"draws":s["n"],"epoch":s["epoch"],"setup_ms":s["setup_ms"],"sampling_ms":s["sampling_ms"],"confidence_conflict":s.get("conflict",False),"proof_status":s.get("proof_status","unknown"),"trusted_highs":s["row"].get("_trusted_highs_log"),"plan":s.get("current_plan"),"prepare_attempts":s.get("prepare_attempts",[]),"sample_errors":s.get("sample_errors",[]),"gd_bounds_status":s.get("gd_bounds_status"),"gd_bounds":s.get("gd_bounds"),"gd_bounds_attempts":s.get("gd_bounds_attempts",[]),"regime_status":s.get("regime_status"),"regime_attempts":s.get("regime_attempts",[]),"empirical_raw_score_streams":empirical,"current_epoch":{"n":stats.n,"B":stats.cap,"sum_lower":stats.sum_lower,"sum_upper":stats.sum_upper,"sum_lower_sq":stats.sum_lower_sq,"sum_upper_sq":stats.sum_upper_sq,"positive_draws":stats.positive_draws,"lower_exceedances":stats.lower,"upper_exceedances":stats.upper},"samples":s["samples"]})
        relied=bool(highs_pruned)
        result={"provenance":"independently rebuilt estimator; not the lost binary","target":desc.get("target"),"fixture_signature":{"team_id":a.team_id,"position":a.position},"confidence_level":.99,"qualified":qualified,"estimate":(total_lo*total_hi)**.5 if qualified else None,"lower":total_lo,"upper":total_hi,"confidence_conflicts":conflicts,"elapsed_seconds":time.monotonic()-started,"setup_seconds":setup_seconds,"setup_ms_total":sum(s["setup_ms"] for s in states.values()),"gd_bounds_enabled":a.gd_bounds,"gd_bounds_elapsed_ms":gd_budget_ms,"gd_bounds_setup_ms":gd_setup_ms,"regimes_enabled":a.regimes,"regime_rebuilds_enabled":bool(a.regimes and a.regime_rebuilds),"regime_elapsed_ms":regime_budget_ms+rebuild_totals["budget_ms"],"regime_setup_ms":regime_setup_ms+rebuild_totals["setup_ms"],"sampling_ms_total":sum(s["sampling_ms"] for s in states.values()),"trusted_highs_enabled":a.trusted_highs,"conditional_on_highs_correct":relied,"trusted_highs_budget_seconds":a.highs_budget,"trusted_highs_elapsed_seconds":highs_elapsed,"trusted_highs_solver_runtime_seconds":sum(float(x.get("runtime_seconds",0)) for x in highs_logs),"trusted_highs_calls":highs_logs,"workers":1,"source_sha256":source_hash,"binary_sha256":binary_hash,"request_sha256":input_hash,"hashes_unchanged":sha(binary)==binary_hash and sha(req)==input_hash and all(sha(Path(path))==digest for path,digest in source_file_hashes.items()),"candidate_slots":desc.get("candidate_slots"),"uncovered_upper":desc.get("uncovered_upper"),"regions":region_results}
        data=json.dumps(result,indent=2,sort_keys=True)
        if a.output: Path(a.output).write_text(data+"\n")
        print(data)
    finally:
        if engine is not None: engine.close()
if __name__=="__main__": main()
