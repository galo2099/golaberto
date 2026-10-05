#!/usr/bin/env python3
"""Run serialized seed-808 current-vs-R58 parity and fallback screen."""
from __future__ import annotations
import hashlib, json, os, resource, subprocess, sys, time
from pathlib import Path

def sha(path: Path) -> str:
    h=hashlib.sha256()
    with path.open('rb') as f:
        for b in iter(lambda:f.read(1<<20),b''): h.update(b)
    return h.hexdigest()

def run(name: str, binary: Path, request: Path, out: Path, flags: dict[str,str]) -> dict:
    export=out/f'{name}.json'; stdout=out/f'{name}.stdout.log'; stderr=out/f'{name}.stderr.log'
    env={k:v for k,v in os.environ.items() if not k.startswith(('RUST_ODDS_','RARE_POSITION_'))}
    env.update(flags); env['RUST_ODDS_LOG']='1'
    command=[str(binary),'estimate',str(request),str(export),'808','4']
    before=resource.getrusage(resource.RUSAGE_CHILDREN); start=time.perf_counter()
    with stdout.open('wb') as so,stderr.open('wb') as se: rc=subprocess.run(command,env=env,stdout=so,stderr=se).returncode
    wall=time.perf_counter()-start; after=resource.getrusage(resource.RUSAGE_CHILDREN)
    if rc or not export.is_file(): raise RuntimeError(f'{name} failed; inspect {stderr}')
    events=[]
    for line in stderr.read_text(errors='replace').splitlines():
        try: v=json.loads(line)
        except json.JSONDecodeError: continue
        if isinstance(v,dict) and isinstance(v.get('event'),str): events.append(v)
    stages={e.get('stage'):e.get('elapsed_ms') for e in events if e.get('event')=='rust_odds_stage'}
    return {'name':name,'seed':808,'workers':4,'binary':str(binary),'binary_sha256':sha(binary),'request':str(request),'request_sha256':sha(request),'argv':command,'explicit_environment':{**flags,'RUST_ODDS_LOG':'1'},'returncode':rc,'export':export.name,'export_sha256':sha(export),'stdout_log':stdout.name,'stdout_sha256':sha(stdout),'stderr_log':stderr.name,'stderr_sha256':sha(stderr),'wall_seconds':wall,'child_user_seconds':after.ru_utime-before.ru_utime,'child_system_seconds':after.ru_stime-before.ru_stime,'child_cpu_seconds':after.ru_utime-before.ru_utime+after.ru_stime-before.ru_stime,'stages_ms':stages}

def main():
    if len(sys.argv)!=4: raise SystemExit('usage: run_screen.py BASELINE_BINARY R58_BINARY REQUEST')
    baseline,r58,request=(Path(x).resolve(strict=True) for x in sys.argv[1:])
    out=Path(__file__).resolve().parents[1]/'logs'/'seed808'; out.mkdir(parents=True,exist_ok=False)
    records=[]
    records.append(run('current-rust',baseline,request,out,{}))
    records.append(run('r58-flag-off',r58,request,out,{}))
    parity=(out/'current-rust.json').read_bytes()==(out/'r58-flag-off.json').read_bytes()
    flags={'RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK':'1','RUST_ODDS_EXPERIMENT_NEIGHBOR_TRANSFER':'native','RUST_ODDS_EXPERIMENT_NEIGHBOR_CELL':'17:12','RUST_ODDS_EXPERIMENT_NEIGHBOR_BUDGET_MULTIPLIER':'1'}
    records.append(run('r58-fallback',r58,request,out,flags))
    payload={'seed':808,'serialized_processes':True,'flag_off_export_byte_identical_to_current_rust':parity,'records':records}
    (out/'summary.json').write_text(json.dumps(payload,indent=2)+'\n')
    if not parity: raise RuntimeError('R58 flag-off export differs from clean current Rust')
if __name__=='__main__': main()
