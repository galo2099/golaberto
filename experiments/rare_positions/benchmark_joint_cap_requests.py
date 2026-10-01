#!/usr/bin/env python3
"""Full-request replacement experiment, using identical seeds and four workers."""
import argparse,json,os,statistics,hashlib,resource,re
from pathlib import Path
from compare_rust_reachability import run_http,states
from benchmark_rust_service import Server

def compare(a,b):
 sa,sb=states(a),states(b)
 gains=[{'team':t,'position':r,'probability':b['rare_position_estimates'][str(t)][str(r-1)]['probability']} for (t,r),v in sa.items() if v!='positive' and sb[t,r]=='positive']
 lost=[{'team':t,'position':r} for (t,r),v in sa.items() if v=='positive' and sb[t,r]!='positive']
 false=[{'team':t,'position':r} for (t,r),v in sa.items() if v=='impossible' and sb[t,r]!='impossible']
 reachability_lost=[{'team':t,'position':r} for (t,r),v in sa.items() if v in ('positive','reachable_zero') and sb[t,r] not in ('positive','reachable_zero')]
 se_diffs=[abs(e['std_err']-b['rare_position_estimates'][t][r]['std_err']) for t,rs in a['rare_position_estimates'].items() for r,e in rs.items() if e['probability']>0]
 diffs=[abs(e['probability']-b['rare_position_estimates'][t][r]['probability']) for t,rs in a['rare_position_estimates'].items() for r,e in rs.items() if e['probability']>0]
 return dict(gains=gains,lost=lost,lost_reachability=reachability_lost,max_existing_standard_error_difference=max(se_diffs,default=0),impossible_regressions=false,max_existing_probability_difference=max(diffs,default=0),game_importance_identical=a['game_importance']==b['game_importance'])

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--baseline',type=Path,required=True);p.add_argument('--candidate',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--persistent',action='store_true');p.add_argument('--production-candidate',action='store_true');p.add_argument('--broad-candidate',action='store_true');a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
 root=Path(__file__).resolve().parents[2];inputs=root/'experiments/rare_positions/reference/2026-09-30-hundredfold/inputs';env={k:v for k,v in os.environ.items() if not k.startswith(('RARE_POSITION_','RUST_ODDS_'))};env.update(RUST_ODDS_LOG='1',TZ='UTC')
 baseline_flags={'RUST_ODDS_JOINT_CAP_BROAD':'0'} if a.broad_candidate else ({'RUST_ODDS_JOINT_CAP_CONDITIONING':'0'} if a.production_candidate else {})
 candidate_flags={} if a.production_candidate or a.broad_candidate else {'RUST_ODDS_JOINT_CAP_EXPERIMENT':'replace'}
 summary=dict(workers=4,baseline_sha256=hashlib.sha256(a.baseline.read_bytes()).hexdigest(),candidate_sha256=hashlib.sha256(a.candidate.read_bytes()).hexdigest(),baseline_flags=baseline_flags,candidate_flags=candidate_flags,pairs=[])
 # Keep the paired cohort fixed when the canonical golden set is refreshed.
 names=['group-16498-44eabb47.json','group-16653-2d1c1d6f.json','group-16653-71d4fea8.json','group-16982-9327edcd.json','group-16983-43969b02.json']
 paths=[inputs/name if (inputs/name).exists() else root/'odds-rust/tests/fixtures'/name for name in names]
 summary['inputs']={p.name:dict(path=str(p.relative_to(root)),sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in paths}
 for path in paths:
  if a.persistent:
   servers={};times={'baseline':[],'candidate':[]};response={};resources={}
   try:
    for name,binary in [('baseline',a.baseline),('candidate',a.candidate)]:
     flags=dict(env,RARE_POSITION_RANDOM_SEED='808')
     flags.update(candidate_flags if name=='candidate' else baseline_flags)
     servers[name]=Server(name,binary.resolve(),flags,a.output,path.stem)
    for i in range(10):
     for name in (['baseline','candidate'] if i%2==0 else ['candidate','baseline']):
      elapsed,r=servers[name].request('/odds',path.read_bytes());response[name]=r
      if i>=2:times[name].append(elapsed)
     assert not compare(response['baseline'],response['candidate'])['lost']
   finally:
    for name,s in servers.items():
     resources[name]=s.stop()
     raw=(a.output/f'{path.stem}-{name}-resource.txt').read_text()
     cpu=re.search(r'([\d.]+)\s+user\s+([\d.]+)\s+sys',raw)
     resources[name]['cpu_seconds']=sum(map(float,cpu.groups()))
   row=dict(input=path.name,latency_ms=times,median_ms={k:statistics.median(v) for k,v in times.items()},resources=resources,comparison=compare(response['baseline'],response['candidate']));row['change_percent']=100*(row['median_ms']['candidate']/row['median_ms']['baseline']-1);summary['pairs'].append(row);print(row,flush=True)
  else:
   for i,seed in enumerate([801,804,808,817,818,911]):
    response={};timing={}
    for name in (['baseline','candidate'] if i%2==0 else ['candidate','baseline']):
     binary=a.baseline if name=='baseline' else a.candidate;flags=baseline_flags if name=='baseline' else candidate_flags
     out=a.output/f'{path.stem}-{seed}-{name}.json'
     before=resource.getrusage(resource.RUSAGE_CHILDREN)
     elapsed,startup,lifetime,logs=run_http(binary.resolve(),path,seed,flags,out,env)
     after=resource.getrusage(resource.RUSAGE_CHILDREN)
     response[name]=json.loads(out.read_text())
     events=[json.loads(line) for line in logs.splitlines() if line.startswith('{')]
     complete=next(e for e in events if e.get('event')=='rust_odds_complete')
     timing[name]=dict(http_ms=elapsed,startup_ms=startup,cpu_ms=1000*(after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime),calculation=complete['timings'],cells=complete['cells'],stages={e['stage']:e['elapsed_ms'] for e in events if e.get('event')=='rust_odds_stage'},joint=[e for e in events if e.get('event') in ('rust_odds_joint_cap_experiment','rust_odds_joint_caps')])
    row=dict(input=path.name,seed=seed,timing=timing,comparison=compare(response['baseline'],response['candidate']));summary['pairs'].append(row);print(row,flush=True)
 (a.output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
if __name__=='__main__':main()
