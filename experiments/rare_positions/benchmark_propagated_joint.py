#!/usr/bin/env python3
"""Paired four-core full requests for propagated joint conditioning."""
import argparse, hashlib, json, os, resource, statistics, re
from pathlib import Path
from compare_rust_reachability import run_http
from benchmark_joint_cap_requests import compare
from benchmark_rust_service import Server

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--baseline',type=Path,required=True)
    p.add_argument('--candidate',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--seeds',default='801,804,808,817,818,911')
    p.add_argument('--cases',default='16498,16653,16982,16983,15902,16413')
    p.add_argument('--persistent',action='store_true')
    p.add_argument('--production-candidate',action='store_true',help='Use the normal binary without enabling flags')
    p.add_argument('--flag',action='append',default=[],help='Candidate NAME=VALUE flag; may be repeated')
    p.add_argument('--baseline-flag',action='append',default=[],help='Baseline NAME=VALUE flag; may be repeated')
    p.add_argument('--allocation',choices=['swap','transfer'],default='transfer')
    a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
    root=Path(__file__).resolve().parents[2]
    inputs=root/'experiments/rare_positions/reference/2026-09-30-hundredfold/inputs'
    selected=set(a.cases.split(','))
    paths=[q for q in sorted(inputs.glob('group-*.json')) if q.stem.split('-')[1] in selected]
    if '16653' in selected: paths.append(root/'odds-rust/tests/fixtures/group-16653-2d1c1d6f.json')
    env={k:v for k,v in os.environ.items() if not k.startswith(('RARE_POSITION_','RUST_ODDS_'))}
    flags={} if a.production_candidate else {'RUST_ODDS_JOINT_PROPAGATION':'1','RUST_ODDS_JOINT_ALLOCATION':a.allocation}
    flags.update(dict(f.split('=',1) for f in a.flag))
    baseline_flags=dict(f.split('=',1) for f in a.baseline_flag)
    summary=dict(baseline_flags=baseline_flags,workers=4,baseline_sha256=hashlib.sha256(a.baseline.read_bytes()).hexdigest(),candidate_sha256=hashlib.sha256(a.candidate.read_bytes()).hexdigest(),flags=flags,inputs={str(q.relative_to(root)):hashlib.sha256(q.read_bytes()).hexdigest() for q in paths},pairs=[])
    for path in paths:
        if a.persistent:
            servers={};times={'baseline':[],'candidate':[]};responses={};resources={};comparisons=[]
            try:
                for name,binary in [('baseline',a.baseline),('candidate',a.candidate)]:
                    servers[name]=Server(name,binary.resolve(),dict(env,RARE_POSITION_RANDOM_SEED='808',**(flags if name=='candidate' else baseline_flags)),a.output,path.stem)
                for i in range(10):
                    for name in (['baseline','candidate'] if i%2==0 else ['candidate','baseline']):
                        elapsed,response=servers[name].request('/odds',path.read_bytes());responses[name]=response
                        if i>=2:times[name].append(elapsed)
                    if i>=2:comparisons.append(compare(responses['baseline'],responses['candidate']))
            finally:
                for name,server in servers.items():
                    resources[name]=server.stop()
                    raw=server.time_path.read_text()
                    cpu=re.search(r'([\d.]+)\s+user\s+([\d.]+)\s+sys',raw)
                    resources[name]['cpu_seconds']=sum(map(float,cpu.groups()))
                    log=Path(server.log.name).read_text()
                    events=[json.loads(line) for line in log.splitlines() if line.endswith('}')]
                    resources[name]['stages']={stage:[e['elapsed_ms'] for e in events if e.get('event')=='rust_odds_stage' and e.get('stage')==stage][2:] for stage in ['scout','pool.mc','search.early_proofs','search.initial_conditioning','search.guided','search.extra','search.witnesses','search.point_tilt','search.peers','search.domains','search.rare_tail','search.late_gap_rescue','search.reconcile']}
                    resources[name]['shared']=[e for e in events if e.get('event','').startswith('rust_odds_shared_constraints')]
                    resources[name]['tail']=[e for e in events if e.get('event','').startswith('rust_odds_rare_tail')]
                    resources[name]['joint']=[e for e in events if e.get('event') in ('rust_odds_joint_caps','rust_odds_joint_planning')]
            row=dict(input=path.name,latency_ms=times,resources=resources,comparisons=comparisons,comparison=compare(responses['baseline'],responses['candidate']))
            row['median_ms']={k:statistics.median(v) for k,v in times.items()}
        else:
            for i,seed in enumerate(map(int,a.seeds.split(','))):
                responses={};timing={}
                for name in (['baseline','candidate'] if i%2==0 else ['candidate','baseline']):
                    binary=a.baseline if name=='baseline' else a.candidate
                    out=a.output/f'{path.stem}-{seed}-{name}.json'
                    before=resource.getrusage(resource.RUSAGE_CHILDREN)
                    elapsed,startup,lifetime,logs=run_http(binary.resolve(),path,seed,flags if name=='candidate' else baseline_flags,out,env)
                    after=resource.getrusage(resource.RUSAGE_CHILDREN)
                    responses[name]=json.loads(out.read_text())
                    events=[json.loads(line) for line in logs.splitlines() if line.endswith('}')]
                    complete=next(e for e in events if e.get('event')=='rust_odds_complete')
                    timing[name]=dict(shared=[e for e in events if e.get('event','').startswith('rust_odds_shared_constraints')],tail=[e for e in events if e.get('event','').startswith('rust_odds_rare_tail')],http_ms=elapsed,cpu_ms=1000*(after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime),cells=complete['cells'],stages={e['stage']:e['elapsed_ms'] for e in events if e.get('event')=='rust_odds_stage'},joint=[e for e in events if e.get('event') in ('rust_odds_joint_caps','rust_odds_joint_allocation')])
                row=dict(input=path.name,seed=seed,timing=timing,comparison=compare(responses['baseline'],responses['candidate']))
                summary['pairs'].append(row)
                print(json.dumps(dict(input=row['input'],seed=seed,baseline=timing['baseline']['cells'],candidate=timing['candidate']['cells'],gain=len(row['comparison']['gains']),lost=row['comparison']['lost'],wall_ms={k:round(v['http_ms'],2) for k,v in timing.items()})),flush=True)
            continue
        summary['pairs'].append(row);print(json.dumps(dict(input=row['input'],median_ms=row['median_ms'],cpu_seconds={k:v['cpu_seconds'] for k,v in resources.items()},comparison=row['comparison'])),flush=True)
    (a.output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
if __name__=='__main__':main()
