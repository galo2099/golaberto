"""Run three serialized alternating warm baseline/native/grant timing triplets."""
import hashlib, json, os, resource, subprocess, sys, time
from pathlib import Path

def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def run(binary, request, root, seed, repeat, arm, flags, expected):
    out=root/arm/f"seed{seed}"/f"repeat{repeat}"
    out.mkdir(parents=True)
    export, stdout, stderr=out/'export.json',out/'stdout.log',out/'stderr.log'
    env={k:v for k,v in os.environ.items() if not k.startswith(("RUST_ODDS_","RARE_POSITION_"))}
    env.update(flags); env['RUST_ODDS_LOG']='1'
    argv=[str(binary),'estimate',str(request),str(export),str(seed),'4']
    before=resource.getrusage(resource.RUSAGE_CHILDREN); start=time.perf_counter()
    with stdout.open('wb') as so,stderr.open('wb') as se: code=subprocess.run(argv,env=env,stdout=so,stderr=se).returncode
    wall=time.perf_counter()-start; after=resource.getrusage(resource.RUSAGE_CHILDREN)
    if code or sha(export)!=expected: raise RuntimeError(f"{arm} seed {seed} repeat {repeat}: output hash mismatch or command failure")
    events=[]
    for line in stderr.read_text(errors='replace').splitlines():
        try: v=json.loads(line)
        except json.JSONDecodeError: continue
        if isinstance(v,dict) and isinstance(v.get('event'),str): events.append(v)
    stage={e.get('stage'):e.get('elapsed_ms') for e in events if e.get('event')=='rust_odds_stage'}
    confirms=[e for e in events if e.get('event')=='rust_odds_rare_tail_confirm_more_summary']
    fallback=[e.get('fallback',{}) for e in events if e.get('event')=='rust_odds_family_fallback']
    return {'arm':arm,'seed':seed,'repeat':repeat,'workers':4,'binary':str(binary),'binary_sha256':sha(binary),'request':str(request),'request_sha256':sha(request),'argv':argv,'explicit_environment':{**flags,'RUST_ODDS_LOG':'1'},'expected_export_sha256':expected,'actual_export_sha256':sha(export),'wall_seconds':wall,'child_user_seconds':after.ru_utime-before.ru_utime,'child_system_seconds':after.ru_stime-before.ru_stime,'child_cpu_seconds':after.ru_utime-before.ru_utime+after.ru_stime-before.ru_stime,'full_pipeline_stage_ms':stage,'confirmation_summary':confirms[0] if confirms else {},'fallback_events':fallback}

def main():
    if len(sys.argv)!=5: raise SystemExit('usage: run_v6_timing.py V6_BINARY CURRENT_RUST_BINARY REQUEST V6_SCREEN_ROOT')
    binary,current,request,screen=(Path(x).resolve(strict=True) for x in sys.argv[1:])
    root=Path(__file__).resolve().parents[1]/'logs'/'v6-timing'; root.mkdir(parents=True,exist_ok=False)
    specs={'baseline':(current,{}),'native-order-cap3000':(binary,{'RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK':'1','RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE':'all','RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE':'family','RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP':'3000'}),'grant-order-cap3000':(binary,{'RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK':'1','RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE':'all','RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE':'family','RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_ORDER':'grant','RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP':'3000'})}
    rows=[]
    for seed in (808,2293):
        for repeat in range(3):
            order=list(specs) if repeat%2==0 else list(reversed(specs))
            for arm in order:
                expected=(screen/arm/f'seed{seed}'/'export.json')
                if arm=='baseline': expected=screen/'flag-off'/'seed808'/'export.json' if seed==808 else Path('experiments/rare_positions/2026-10-03-family-fallback/logs/v3-cohort/seed2293/current-rust.json').resolve()
                binary_for_arm,flags=specs[arm]
                rows.append(run(binary_for_arm,request,root,seed,repeat,arm,flags,sha(expected)))
    (root/'summary.json').write_text(json.dumps({'binary':str(binary),'binary_sha256':sha(binary),'seeds':[808,2293],'repeats':3,'workers':4,'serialized_processes':True,'warm_order':'baseline/native/grant on even repeats; reverse on odd repeats','records':rows},indent=2)+'\n')
    for row in rows: (root/row['arm']/f"seed{row['seed']}"/f"repeat{row['repeat']}.json").write_text(json.dumps(row,indent=2)+'\n')
if __name__=='__main__': main()
