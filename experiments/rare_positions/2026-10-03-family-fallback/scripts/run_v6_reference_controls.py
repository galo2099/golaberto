"""Run only authorized selected-cell 10x reference controls on frozen V6 binary."""
import hashlib,json,os,resource,subprocess,sys,time
from pathlib import Path

def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def run(binary,request,root,seed,cell,mode):
    arm=f"{cell.replace(':','-')}-{mode}-10x-cap3000"; out=root/arm/f"seed{seed}";out.mkdir(parents=True)
    export,stdout,stderr=out/'export.json',out/'stdout.log',out/'stderr.log'
    flags={'RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK':'1','RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE':mode,'RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MULTIPLIER':'10','RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP':'3000','RUST_ODDS_EXPERIMENT_NEIGHBOR_TRANSFER':'native','RUST_ODDS_EXPERIMENT_NEIGHBOR_CELL':cell}
    env={k:v for k,v in os.environ.items() if not k.startswith(('RUST_ODDS_','RARE_POSITION_'))};env.update(flags);env['RUST_ODDS_LOG']='1'
    argv=[str(binary),'estimate',str(request),str(export),str(seed),'4'];before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.perf_counter()
    with stdout.open('wb') as so,stderr.open('wb') as se: code=subprocess.run(argv,env=env,stdout=so,stderr=se).returncode
    wall=time.perf_counter()-start;after=resource.getrusage(resource.RUSAGE_CHILDREN)
    if code or not export.exists(): raise RuntimeError(f'{arm} seed{seed} failed')
    events=[]
    for line in stderr.read_text(errors='replace').splitlines():
        try:v=json.loads(line)
        except json.JSONDecodeError:continue
        if isinstance(v,dict) and isinstance(v.get('event'),str):events.append(v)
    return {'arm':arm,'seed':seed,'cell':cell,'mode':mode,'multiplier':10,'training_cap':3000,'workers':4,'binary':str(binary),'binary_sha256':sha(binary),'request':str(request),'request_sha256':sha(request),'argv':argv,'explicit_environment':{**flags,'RUST_ODDS_LOG':'1'},'export':str(export),'export_sha256':sha(export),'stdout':str(stdout),'stdout_sha256':sha(stdout),'stderr':str(stderr),'stderr_sha256':sha(stderr),'wall_seconds':wall,'child_user_seconds':after.ru_utime-before.ru_utime,'child_system_seconds':after.ru_stime-before.ru_stime,'child_cpu_seconds':after.ru_utime-before.ru_utime+after.ru_stime-before.ru_stime,'fallback_events':[e for e in events if e.get('event')=='rust_odds_family_fallback'],'confirmation_summaries':[e for e in events if e.get('event')=='rust_odds_rare_tail_confirm_more_summary'],'stages':[e for e in events if e.get('event')=='rust_odds_stage']}

def main():
    if len(sys.argv)!=4:raise SystemExit('usage: run_v6_reference_controls.py V6_BINARY REQUEST V6_SCREEN_ROOT')
    binary,request,screen=(Path(x).resolve(strict=True) for x in sys.argv[1:]);root=Path(__file__).resolve().parents[1]/'logs'/'v6-reference-controls';root.mkdir(parents=True,exist_ok=False)
    specs=[(808,'16:15','family'),(808,'16:15','native_retry'),(2293,'16:15','family'),(2293,'16:15','native_retry'),(2293,'16:13','family'),(2293,'16:13','native_retry')]
    rows=[run(binary,request,root,*spec) for spec in specs]
    for row in rows:(root/row['arm']/f"seed{row['seed']}"/'record.json').write_text(json.dumps(row,indent=2)+'\n')
    (root/'summary.json').write_text(json.dumps({'binary':str(binary),'binary_sha256':sha(binary),'request':str(request),'request_sha256':sha(request),'workers':4,'serialized_processes':True,'records':rows},indent=2)+'\n')
if __name__=='__main__':main()
