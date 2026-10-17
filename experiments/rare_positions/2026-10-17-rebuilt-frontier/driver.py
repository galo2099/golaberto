#!/usr/bin/env python3
"""Budgeted adaptive driver for the independently rebuilt frontier estimator."""
from __future__ import annotations
import argparse, hashlib, json, math, os, subprocess, sys, time
from pathlib import Path
IMPORT_STARTED=time.monotonic()
from confidence import telescoping_alpha, threshold_mean_bounds, scaled_absolute_bounds, intersect_bounds, EpochStats

HERE = Path(__file__).resolve().parent
DEFAULT_REQUEST = HERE.parents[2] / "odds-rust/tests/fixtures/group-16498-2026-10-10.json"

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
    p.add_argument("--budget",type=float,default=180); p.add_argument("--seed",type=int,default=20261017); p.add_argument("--workers",type=int,default=1)
    p.add_argument("--initial-draws",type=int,default=128); p.add_argument("--max-draws",type=int,default=100000); p.add_argument("--state-cap",type=int,default=250000); p.add_argument("--time-ms",type=int,default=500); p.add_argument("--depth",type=int,default=6); p.add_argument("--initial-k",type=int,default=1); p.add_argument("--max-k",type=int,default=12); p.add_argument("--output")
    a=p.parse_args(); started=IMPORT_STARTED; deadline=started+max(1,a.budget); req=Path(a.request).resolve(); binary=Path(a.binary).resolve()
    if a.workers != 1: print("frontier-recovery uses a serial native engine; recording workers=1",file=sys.stderr)
    setup_start=time.monotonic()
    # Startup is inside the same deadline and hashes freeze the measured inputs.
    hsrc=hashlib.sha256()
    source_paths=[Path(__file__),HERE/"confidence.py",binary,HERE.parents[2]/"odds-rust/frontier-recovery/Cargo.toml"]
    crate=HERE.parents[2]/"odds-rust/frontier-recovery"
    source_paths += sorted(crate.glob("src/*.rs"))
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
        state_caps={int(r["id"]):int((r.get("plan") or {}).get("states",0)) for r in candidates}
        # Establish at least a bounded fallback plan and discovery sample for every cohort.
        for row in candidates:
            if time.monotonic()>=deadline-10: break
            rid=int(row["id"])
            try:
                others=sum(state_caps.values())-state_caps.get(rid,0)
                initial_cap=min(a.state_cap,1000,max(0,4_000_000-others))
                if initial_cap<=0: row["_first_sample"]=None; continue
                row["_last_attempt_k"],row["_last_attempt_cap"]=a.initial_k,initial_cap
                try: prep=engine.call(op="prepare",id=rid,k=a.initial_k,state_cap=initial_cap,time_ms=min(a.time_ms,100))
                except RuntimeError as exc: prep={"error":str(exc)}
                row["_prepare_setup_ms"]=prep.get("setup_ms",0)
                if prep.get("accepted"):
                    row["_plan_states"]=int(prep.get("states",0));state_caps[rid]=row["_plan_states"]
                    if float(prep.get("B",1.0))==0.0:
                        row["upper"]=0.0;row["_exact_zero"]=True;row["_first_sample"]=None
                        continue
                row["_plan_epoch"]=int(prep.get("epoch",0))
            except (TimeoutError,RuntimeError): row["_first_sample"]=None
        # Only after every reachable cohort has had its small preparation attempt,
        # collect the cheap discovery batch for all of them before focused growth.
        for row in candidates:
            if time.monotonic()>=deadline-10: break
            if row.get("_exact_zero"): continue
            rid=int(row["id"])
            try:
                first=engine.call(op="sample",id=rid,n=a.initial_draws,seed=a.seed+rid,depth=a.depth)
                row["_first_sample"]=first
                row["_plan_epoch"]=int(first.get("epoch",row.get("_plan_epoch",0)))
            except (TimeoutError,RuntimeError): row["_first_sample"]=None
        state_caps.update({int(r["id"]):int(r.get("_plan_states",state_caps.get(int(r["id"]),0))) for r in candidates})
        states={int(r["id"]):{"row":r,"epoch":int(r.get("_plan_epoch",0)),"look":0,"n":0,"stats":EpochStats(int(r.get("_plan_epoch",0))),"lo":0.,"hi":float(r.get("upper",1.)),"samples":[],"setup_ms":r.get("_prepare_setup_ms",0),"sampling_ms":0,"last_attempt_k":int(r.get("_last_attempt_k",-1)),"last_attempt_cap":int(r.get("_last_attempt_cap",-1)),"conflict":False,"sample_errors":[]} for r in candidates}
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
                    s["lo"],s["hi"],conflict=intersect_bounds(s["lo"],s["hi"],ll,ul)
                    s["conflict"]=s.get("conflict",False) or conflict
                    s["samples"].append({"batch_n":a.initial_draws,"epoch_n":s["stats"].n,"epoch":s["epoch"],"lower_absolute":ll,"upper_absolute":ul,"B":cap,"setup_ms":first.get("setup_ms",0),"elapsed_ms":first.get("elapsed_ms",0)})
                except ValueError as exc: s["sample_errors"].append(str(exc))
        # Each focused batch is incorporated only after a complete receipt arrives.
        while time.monotonic()<deadline-10 and states:
            total_lo=max(0.0,math.nextafter(math.fsum(s["lo"] for s in states.values()),0.0))
            total_hi=min(1.0,math.nextafter(math.fsum([*(s["hi"] for s in states.values()),float(desc.get("uncovered_upper",0))]),math.inf))
            if total_lo>0 and total_hi/total_lo<=10: break
            active=[]
            for item in states.items():
                rid0,s0=item
                others=sum(state_caps.values())-state_caps.get(rid0,0)
                if s0["n"]<a.max_draws and s0["hi"]>s0["lo"] and 4_000_000-others>0: active.append(item)
            if not active: break
            rid,s=max(active,key=lambda kv:(kv[1]["hi"]-kv[1]["lo"]))
            others=sum(state_caps.values())-state_caps.get(rid,0)
            allowed=max(0,4_000_000-others)
            desired=min(a.state_cap,1000*(2**min(s["look"],8)))
            plan_state_cap=min(desired,allowed)
            batch=min(1000,a.max_draws-s["n"])
            try:
                # Increasing preparation complexity is local to the current widest cohort.
                requested_k=min(a.max_k,a.initial_k+s["look"]//2)
                if (requested_k,plan_state_cap)!=(s["last_attempt_k"],s["last_attempt_cap"]):
                    s["last_attempt_k"],s["last_attempt_cap"]=requested_k,plan_state_cap
                    try:
                        prep=engine.call(op="prepare",id=rid,k=requested_k,state_cap=plan_state_cap,time_ms=min(a.time_ms,1000))
                        s["setup_ms"] += prep.get("setup_ms",0)
                    except RuntimeError as exc:
                        prep={"accepted":False,"epoch":s["epoch"]}
                        s["sample_errors"].append(str(exc))
                else: prep={"accepted":False,"epoch":s["epoch"]}
                if prep.get("accepted"):
                    state_caps[rid]=int(prep.get("states",state_caps.get(rid,0)))
                    if float(prep.get("B",1.0))==0.0:
                        s["row"]["upper"]=0.0
                        s["lo"],s["hi"],conflict=intersect_bounds(s["lo"],s["hi"],0.0,0.0)
                        s["conflict"]=s.get("conflict",False) or conflict
                        continue
                if prep.get("accepted") and int(prep.get("epoch",s["epoch"])) != s["epoch"]:
                    s["epoch"]=int(prep["epoch"]); s["look"]=0; s["stats"].reset(s["epoch"])
                rec=engine.call(op="sample",id=rid,n=batch,seed=a.seed+rid+1000003*s["epoch"]+s["look"],depth=a.depth)
            except (TimeoutError,RuntimeError): break
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
            s["n"] += batch; s["look"] += 1; s["sampling_ms"] += rec.get("elapsed_ms",0)
            s["samples"].append({"batch_n":batch,"epoch_n":epoch_n,"epoch":s["epoch"],"lower_absolute":ll,"upper_absolute":ul,"B":cap,"setup_ms":rec.get("setup_ms",0),"elapsed_ms":rec.get("elapsed_ms",0)})
        engine.close(); engine=None
        total_lo=max(0.0,math.nextafter(math.fsum(s["lo"] for s in states.values()),0.0))
        total_hi=min(1.0,math.nextafter(math.fsum([*(s["hi"] for s in states.values()),float(desc.get("uncovered_upper",0))]),math.inf))
        conflicts=any(s.get("conflict",False) for s in states.values())
        qualified=bool(not conflicts and total_lo>0 and total_hi/total_lo<=10)
        result={"provenance":"independently rebuilt estimator; not the lost binary","target":desc.get("target"),"fixture_signature":{"team_id":a.team_id,"position":a.position},"confidence_level":.99,"qualified":qualified,"estimate":(total_lo*total_hi)**.5 if qualified else None,"lower":total_lo,"upper":total_hi,"confidence_conflicts":conflicts,"elapsed_seconds":time.monotonic()-started,"setup_seconds":setup_seconds,"setup_ms_total":sum(s["setup_ms"] for s in states.values()),"sampling_ms_total":sum(s["sampling_ms"] for s in states.values()),"workers":1,"source_sha256":source_hash,"binary_sha256":binary_hash,"request_sha256":input_hash,"hashes_unchanged":sha(binary)==binary_hash and sha(req)==input_hash and all(sha(Path(path))==digest for path,digest in source_file_hashes.items()),"candidate_slots":desc.get("candidate_slots"),"uncovered_upper":desc.get("uncovered_upper"),"regions":[{"id":rid,"analytic_upper":s["row"].get("upper"),"lower":s["lo"],"upper":s["hi"],"draws":s["n"],"epoch":s["epoch"],"setup_ms":s["setup_ms"],"sampling_ms":s["sampling_ms"],"confidence_conflict":s.get("conflict",False),"proof_status":("exact" if s["row"].get("impossible") else s["row"].get("proof_status","unknown")),"plan":s["row"].get("plan"),"samples":s["samples"]} for rid,s in states.items()]}
        data=json.dumps(result,indent=2,sort_keys=True)
        if a.output: Path(a.output).write_text(data+"\n")
        print(data)
    finally:
        if engine is not None: engine.close()
if __name__=="__main__": main()
