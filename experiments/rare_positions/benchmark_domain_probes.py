#!/usr/bin/env python3
"""Alternating persistent /odds comparisons; four workers, no database access."""
import argparse
import json
import os
from pathlib import Path
import re
import statistics
from benchmark_rust_service import Server
from compare_rust_reachability import VARIANTS, states


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--baseline',required=True,type=Path)
    p.add_argument('--candidate',required=True,type=Path)
    p.add_argument('--variant',default='probe_joint',choices=VARIANTS)
    p.add_argument('--iterations',type=int,default=8)
    p.add_argument('--seed',type=int,default=808)
    p.add_argument('--input',action='append',type=Path)
    p.add_argument('--output',required=True,type=Path)
    args=p.parse_args(); assert args.iterations>0
    inputs=args.input or sorted((Path(__file__).parent/'reference/2026-09-30-hundredfold/inputs').glob('*.json'))
    out=args.output.resolve();out.mkdir(parents=True,exist_ok=True)
    env={k:v for k,v in os.environ.items() if not k.startswith(('RARE_POSITION_','RUST_ODDS_'))}
    env.update(RUST_ODDS_LOG='1',RUST_ODDS_PROFILE='0',RARE_POSITION_RANDOM_SEED=str(args.seed))
    summary=dict(seed=args.seed,workers=4,warmups=2,iterations=args.iterations,
                 variant=args.variant,flags=VARIANTS[args.variant],scenarios=[])
    for path in inputs:
        servers={};resources={};times={k:[] for k in ['baseline','candidate']};responses={}
        try:
            for name,binary in [('baseline',args.baseline),('candidate',args.candidate)]:
                servers[name]=Server(name,binary.resolve(),dict(env,**(VARIANTS[args.variant] if name=='candidate' else {})),out,path.stem)
            for i in range(args.iterations+2):
                for name in (['baseline','candidate'] if i%2==0 else ['candidate','baseline']):
                    elapsed,response=servers[name].request('/odds',path.read_bytes())
                    if i>=2:times[name].append(elapsed)
                    if i==0:responses[name]=response
                    else:assert response==responses[name],(path.name,name,'response changed with fixed seed')
                assert responses['baseline']['game_importance']==responses['candidate']['game_importance']
        finally:
            for name,server in servers.items():resources[name]=server.stop()
        stages={};probes={}
        for name in resources:
            raw=(out/f'{path.stem}-{name}-resource.txt').read_text()
            cpu=re.search(r'([\d.]+)\s+user\s+([\d.]+)\s+sys',raw);assert cpu
            resources[name]['cpu_seconds']=sum(map(float,cpu.groups()))
            events=[json.loads(line) for line in (out/f'{path.stem}-{name}.log').read_text().splitlines(keepends=True) if line.startswith('{') and line.endswith('\n')]
            stages[name]={s:[e['elapsed_ms'] for e in events if e.get('stage')==s] for s in sorted({e['stage'] for e in events if e.get('event')=='rust_odds_stage'})}
            probes[name]=[e for e in events if e.get('event')=='rust_odds_domain_probes']
        a,b=map(states,[responses['baseline'],responses['candidate']])
        row=dict(input=path.name,latency_ms=times,median_ms={k:statistics.median(v) for k,v in times.items()},resources=resources,stages=stages,probes=probes,
                 gained_nonzero=[dict(team=t,position=r) for (t,r),v in a.items() if v!='positive' and b[t,r]=='positive'],
                 lost_nonzero=[dict(team=t,position=r) for (t,r),v in a.items() if v=='positive' and b[t,r]!='positive'])
        row['median_change_percent']=100*(row['median_ms']['candidate']/row['median_ms']['baseline']-1)
        summary['scenarios'].append(row)
        (out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
        print(path.name, row['median_ms'],round(row['median_change_percent'],2),'%',flush=True)
    print('summed medians',100*(sum(r['median_ms']['candidate'] for r in summary['scenarios'])/sum(r['median_ms']['baseline'] for r in summary['scenarios'])-1),flush=True)


if __name__=='__main__':main()
