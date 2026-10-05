#!/usr/bin/env python3
"""Run the authorized five fixed-seed paired screen serially at four workers."""
from __future__ import annotations
import hashlib,json,os,resource,subprocess,sys,time
from pathlib import Path

def sha(p:Path)->str:
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1<<20),b''):h.update(b)
 return h.hexdigest()

def run(name,binary,request,out,seed,flags):
 export=out/f'{name}.json'; stdout=out/f'{name}.stdout.log'; stderr=out/f'{name}.stderr.log'
 env={k:v for k,v in os.environ.items() if not k.startswith(('RUST_ODDS_','RARE_POSITION_'))}; env.update(flags); env['RUST_ODDS_LOG']='1'
 argv=[str(binary),'estimate',str(request),str(export),str(seed),'4']
 before=resource.getrusage(resource.RUSAGE_CHILDREN); start=time.perf_counter()
 with stdout.open('wb') as so,stderr.open('wb') as se: rc=subprocess.run(argv,env=env,stdout=so,stderr=se).returncode
 wall=time.perf_counter()-start; after=resource.getrusage(resource.RUSAGE_CHILDREN)
 if rc or not export.is_file(): raise RuntimeError(f'{name}/seed{seed} failed; inspect {stderr}')
 events=[]
 for line in stderr.read_text(errors='replace').splitlines():
  try:e=json.loads(line)
  except json.JSONDecodeError:continue
  if isinstance(e,dict) and isinstance(e.get('event'),str):events.append(e)
 stages={e.get('stage'):e.get('elapsed_ms') for e in events if e.get('event')=='rust_odds_stage'}
 return {'name':name,'seed':seed,'workers':4,'binary':str(binary),'binary_sha256':sha(binary),'request':str(request),'request_sha256':sha(request),'argv':argv,'explicit_environment':{**flags,'RUST_ODDS_LOG':'1'},'returncode':rc,'export':export.name,'export_sha256':sha(export),'stdout_log':stdout.name,'stdout_sha256':sha(stdout),'stderr_log':stderr.name,'stderr_sha256':sha(stderr),'wall_seconds':wall,'child_user_seconds':after.ru_utime-before.ru_utime,'child_system_seconds':after.ru_stime-before.ru_stime,'child_cpu_seconds':after.ru_utime-before.ru_utime+after.ru_stime-before.ru_stime,'stages_ms':stages}

def main():
 if len(sys.argv)!=4:raise SystemExit('usage: run_cohort.py CURRENT_RUST_BINARY R58_V2_BINARY REQUEST')
 base,r58,request=(Path(x).resolve(strict=True) for x in sys.argv[1:])
 root=Path(__file__).resolve().parents[1]/'logs'/'v3-cohort'; root.mkdir(parents=True,exist_ok=False)
 seeds=[808,1669,1993,2281,2293]; all_records=[]
 fallback_flags={'RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK':'1','RUST_ODDS_EXPERIMENT_NEIGHBOR_TRANSFER':'native','RUST_ODDS_EXPERIMENT_NEIGHBOR_CELL':'17:12','RUST_ODDS_EXPERIMENT_NEIGHBOR_BUDGET_MULTIPLIER':'1'}
 for seed in seeds:
  out=root/f'seed{seed}';out.mkdir()
  records=[run('current-rust',base,request,out,seed,{})]
  if seed==808:
   records.append(run('r58-flag-off',r58,request,out,seed,{}))
  records.append(run('r58-fallback',r58,request,out,seed,fallback_flags))
  if seed==808 and (out/'current-rust.json').read_bytes()!=(out/'r58-flag-off.json').read_bytes():
   raise RuntimeError('V3 R58 flag-off export differs from current Rust')
  (out/'summary.json').write_text(json.dumps({'seed':seed,'serialized_processes':True,'records':records},indent=2)+'\n')
  all_records.extend(records)
 (root/'summary.json').write_text(json.dumps({'seeds':seeds,'workers':4,'serialized_processes':True,'seed808_flag_off_export_byte_identical_to_current_rust':True,'records':all_records},indent=2)+'\n')
if __name__=='__main__':main()
