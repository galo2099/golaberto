#!/usr/bin/env python3
"""Verify preservation and summarize the saved five-seed R58 cohort."""
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]/'logs'/'v2-cohort'
OUT=Path(__file__).resolve().parents[1]/'data'/'v2-cohort-analysis.json'
SEEDS=[808,1669,1993,2281,2293]
def strip_work(v):
 if isinstance(v,dict):return {k:strip_work(x) for k,x in v.items() if k!='work_spent'}
 if isinstance(v,list):return [strip_work(x) for x in v]
 return v
def parse_events(path):
 rows=[]
 for line in path.read_text(errors='replace').splitlines():
  try:x=json.loads(line)
  except json.JSONDecodeError:continue
  if x.get('event')=='rust_odds_family_fallback':rows.append(x['fallback'])
 return rows
def main():
 rows=[]
 for seed in SEEDS:
  d=ROOT/f'seed{seed}'; base=json.loads((d/'current-rust.json').read_text()); cand=json.loads((d/'r58-fallback.json').read_text())
  events=parse_events(d/'r58-fallback.stderr.log')
  target0=base['rare_position_estimates']['17']['11']; target1=cand['rare_position_estimates']['17']['11']
  positive_changed=[]; changed=[]
  for team,positions in base['rare_position_estimates'].items():
   for rank,e in positions.items():
    other=cand['rare_position_estimates'][team][rank]
    if strip_work(e)!=strip_work(other):
     key=f'{team}:{rank}';changed.append(key)
     if e.get('probability',0)>0:positive_changed.append(key)
  if positive_changed:raise AssertionError(f'seed {seed} changed baseline-positive evidence: {positive_changed}')
  if base['game_importance']!=cand['game_importance']:raise AssertionError(f'seed {seed} game importance changed')
  if seed in (1669,1993,2281) and events:raise AssertionError(f'seed {seed} unexpectedly attempted fallback')
  records={r['name']:r for r in json.loads((d/'summary.json').read_text())['records']}
  rows.append({'seed':seed,'baseline_probability':target0['probability'],'fallback_probability':target1['probability'],'fallback_events':events,'baseline_positive_cells_changed_excluding_work_spent':positive_changed,'changed_estimate_cells_excluding_work_spent':changed,'game_importance_identical':True,'full_wall_baseline_s':records['current-rust']['wall_seconds'],'full_wall_fallback_s':records['r58-fallback']['wall_seconds'],'cpu_baseline_s':records['current-rust']['child_cpu_seconds'],'cpu_fallback_s':records['r58-fallback']['child_cpu_seconds']})
 OUT.write_text(json.dumps({'seeds':rows},indent=2)+'\n')
if __name__=='__main__':main()
