#!/usr/bin/env python3
"""Offline paired controls for exact rival-cap conditioning; four workers total."""
import argparse,json,os,subprocess,time,hashlib
from pathlib import Path

def main():
 p=argparse.ArgumentParser(description=__doc__)
 p.add_argument('--example',type=Path,required=True)
 p.add_argument('--baseline',type=Path,required=True)
 p.add_argument('--output',type=Path,required=True)
 p.add_argument('--samples',type=int,default=5000)
 a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
 root=Path(__file__).resolve().parents[2]
 inputs=root/'experiments/rare_positions/reference/2026-09-30-hundredfold/inputs'
 env={k:v for k,v in os.environ.items() if not k.startswith(('RUST_ODDS_','RARE_POSITION_'))};env['RUST_ODDS_LOG']='0'
 all_results={'samples':a.samples,'workers':4,'baseline_sha256':hashlib.sha256(a.baseline.read_bytes()).hexdigest(),'example_sha256':hashlib.sha256(a.example.read_bytes()).hexdigest(),'groups':[]}
 # No simultaneous requests or builds during timings.
 path=inputs/'group-16498-44eabb47.json'
 for k in range(7):
  t=time.perf_counter();r=subprocess.run([str(a.example),'%s'%path,'318','3',str(k),str(a.samples)],env=env,capture_output=True,text=True)
  (a.output/f'chapecoense-k{k}.jsonl').write_text(r.stdout);(a.output/f'chapecoense-k{k}.stderr').write_text(r.stderr)
  if r.returncode: print({'k':k,'skipped':r.stderr});continue
  rows=[json.loads(l) for l in r.stdout.splitlines()];all_results.setdefault('chapecoense',[]).extend(rows);print({'k':k,'runs':[(v['seed'],v['result']['probability'],v['result']['ess'],v['result']['accepted']) for v in rows[0]['runs']],'setup_ms':rows[0]['setup_ms'],'elapsed_s':time.perf_counter()-t},flush=True)
 for path in sorted(inputs.glob('*.json')):
  scan=subprocess.run([str(a.example),str(path),'scan'],env=env,capture_output=True,text=True,check=True)
  (a.output/(path.stem+'-scan.jsonl')).write_text(scan.stdout)
  candidates=[json.loads(l) for l in scan.stdout.splitlines()];rows=[]
  for seed in [801,804,808,817,818,911]:
   out=a.output/f'{path.stem}-{seed}-baseline.json';r=subprocess.run([str(a.baseline),'estimate',str(path),str(out),str(seed),'4'],env=env,capture_output=True,text=True,check=True)
   (out.with_suffix('.log')).write_text(r.stderr);response=json.loads(out.read_text());timing=json.loads(r.stderr.splitlines()[-1]);eligible=[]
   for c in candidates:
    st=c['setup'];e=response['rare_position_estimates'][str(st['team'])][str(st['rank']-1)]
    if e['probability']==0 and not e.get('reachability','').startswith('impossible'):eligible.append(st)
   rows.append({'seed':seed,'timing':timing,'eligible_zero_cells':eligible,'zero_counts':sum(e['probability']==0 for ranks in response['rare_position_estimates'].values() for e in ranks.values())})
  all_results['groups'].append({'input':path.name,'candidates':candidates,'baseline':rows});print({'input':path.name,'eligible':[(r['seed'],[(v['team'],v['rank'],v['dual']) for v in r['eligible_zero_cells']]) for r in rows]},flush=True)
 (a.output/'summary.json').write_text(json.dumps(all_results,indent=2)+'\n')
if __name__=='__main__':main()
