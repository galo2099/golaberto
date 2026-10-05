import json,statistics,hashlib
from pathlib import Path
import sys
root=Path(sys.argv[1]);s=json.loads((root/'summary.json').read_text());rows=s['pairs']
out={'pairs':len(rows),'gains':[],'losses':[],'proof_regressions':[],'groups':{},'preparations':[],'identical_exports':0}
for row in rows:
 name=row['input'];seed=row.get('seed');g=out['groups'].setdefault(name,{'gains':[],'losses':[],'cpu_ms':{'baseline':0,'candidate':0},'http_ms':{'baseline':[],'candidate':[]},'tail_ms':{'baseline':[],'candidate':[]}})
 for kind in ['gains','lost']:
  key='gains' if kind=='gains' else 'losses'
  for x in row['comparison'][kind]:
   item=dict(input=name,seed=seed,**x);out[key].append(item);g[key].append(item)
 out['proof_regressions'] += row['comparison']['lost_reachability']+row['comparison']['impossible_regressions']
 if 'timing' in row:
  for arm in ['baseline','candidate']:
   t=row['timing'][arm];g['cpu_ms'][arm]+=t['cpu_ms'];g['http_ms'][arm].append(t['http_ms']);g['tail_ms'][arm].append(t['stages']['search.rare_tail'])
  a=root/f'{Path(name).stem}-{seed}-baseline.json';b=root/f'{Path(name).stem}-{seed}-candidate.json'
  out['identical_exports']+=a.read_bytes()==b.read_bytes()
for p in root.glob('*candidate.log'):
 for line in p.read_text().splitlines():
  if line.startswith('{'):
   e=json.loads(line)
   if e.get('event')=='rust_odds_lazy_partitions':out['preparations'].append(dict(file=p.name,**e))
for g in out['groups'].values():
 if g['http_ms']['baseline']:
  g['median_http_ms']={a:statistics.median(g['http_ms'][a])for a in ['baseline','candidate']}
  g['median_tail_ms']={a:statistics.median(g['tail_ms'][a])for a in ['baseline','candidate']}
  g['cpu_change_pct']=100*(g['cpu_ms']['candidate']/g['cpu_ms']['baseline']-1)
  g['http_change_pct']=100*(g['median_http_ms']['candidate']/g['median_http_ms']['baseline']-1)
(root/'analysis.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({k:v for k,v in out.items() if k not in ['preparations','groups']},indent=2))
for name,g in out['groups'].items():print(name,json.dumps(g))
print('preparations',len(out['preparations']),'selected',sum(e['details']['selected']for e in out['preparations']))
for e in out['preparations']:print(e['seed'],e['team'],e['rank'],{k:v for k,v in e['details'].items()if k!='regions'})
