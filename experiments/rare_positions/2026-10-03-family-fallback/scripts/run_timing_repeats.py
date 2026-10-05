#!/usr/bin/env python3
"""Run three alternating serialized timing pairs for seeds 808 and 2293."""
import hashlib,json,os,resource,subprocess,time
from pathlib import Path
ART=Path(__file__).resolve().parents[1]
REQ=Path(__file__).resolve().parents[4]/'experiments'/'rare_positions'/'2026-10-03-family-conditioning'/'data'/'group-16498-44eabb47.json'
BASE=Path('/private/tmp/golaberto-r57/baseline/odds-rust/target/release/golaberto-odds')
R58=Path('/private/tmp/golaberto-r58/odds-rust/target/release/golaberto-odds')
FLAGS={'RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK':'1','RUST_ODDS_EXPERIMENT_NEIGHBOR_TRANSFER':'native','RUST_ODDS_EXPERIMENT_NEIGHBOR_CELL':'17:12','RUST_ODDS_EXPERIMENT_NEIGHBOR_BUDGET_MULTIPLIER':'1'}
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1<<20),b''):h.update(b)
 return h.hexdigest()
def parse(path):
 out=[]
 for line in path.read_text(errors='replace').splitlines():
  try:x=json.loads(line)
  except json.JSONDecodeError:continue
  if isinstance(x,dict) and isinstance(x.get('event'),str):out.append(x)
 return out
def run(seed,rep,arm,out):
 name='baseline' if arm=='baseline' else 'fallback'; binary=BASE if arm=='baseline' else R58
 folder=out/f'seed{seed}';folder.mkdir(exist_ok=True)
 stem=f'{name}-r{rep}'; export=folder/(stem+'.json'); stdout=folder/(stem+'.stdout.log'); stderr=folder/(stem+'.stderr.log')
 env={k:v for k,v in os.environ.items() if not k.startswith(('RUST_ODDS_','RARE_POSITION_'))}
 if arm=='fallback':env.update(FLAGS)
 env['RUST_ODDS_LOG']='1';argv=[str(binary),'estimate',str(REQ),str(export),str(seed),'4']
 before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.perf_counter()
 with stdout.open('wb') as so,stderr.open('wb') as se:rc=subprocess.run(argv,env=env,stdout=so,stderr=se).returncode
 wall=time.perf_counter()-start;after=resource.getrusage(resource.RUSAGE_CHILDREN)
 if rc or not export.is_file():raise RuntimeError(f'{stem} failed')
 expected=ART/'logs'/'v3-cohort'/f'seed{seed}'/('current-rust.json' if arm=='baseline' else 'r58-fallback.json')
 if sha(export)!=sha(expected):raise RuntimeError(f'{stem} export differs from V3: {sha(export)} != {sha(expected)}')
 es=parse(stderr); stages={e.get('stage'):e.get('elapsed_ms') for e in es if e.get('event')=='rust_odds_stage'}
 fallback=[e['fallback'] for e in es if e.get('event')=='rust_odds_family_fallback']
 return {'arm':arm,'seed':seed,'repetition':rep,'binary':str(binary),'binary_sha256':sha(binary),'request':str(REQ),'request_sha256':sha(REQ),'argv':argv,'explicit_environment':{**(FLAGS if arm=='fallback' else {}),'RUST_ODDS_LOG':'1'},'export_sha256':sha(export),'expected_v3_export_sha256':sha(expected),'stderr_sha256':sha(stderr),'stdout_sha256':sha(stdout),'returncode':rc,'wall_seconds':wall,'child_user_seconds':after.ru_utime-before.ru_utime,'child_system_seconds':after.ru_stime-before.ru_stime,'child_cpu_seconds':after.ru_utime-before.ru_utime+after.ru_stime-before.ru_stime,'stages_ms':stages,'fallback_stage_ms':fallback[0].get('stage_ms') if fallback else None,'fallback_work':fallback[0] if fallback else None}
def main():
 out=ART/'logs'/'v3-timing';out.mkdir(parents=True,exist_ok=False); all=[]
 for seed in (808,2293):
  for rep in (1,2,3):
   order=('baseline','fallback') if rep%2 else ('fallback','baseline')
   pair=[run(seed,rep,arm,out) for arm in order]
   (out/f'seed{seed}'/f'pair-r{rep}.json').write_text(json.dumps({'seed':seed,'repetition':rep,'order':order,'runs':pair},indent=2)+'\n')
   all.extend(pair)
 (out/'summary.json').write_text(json.dumps({'seeds':[808,2293],'repetitions_per_seed':3,'serialized':True,'alternating_arm_order':True,'all_exports_match_v3':True,'runs':all},indent=2)+'\n')
if __name__=='__main__':main()
