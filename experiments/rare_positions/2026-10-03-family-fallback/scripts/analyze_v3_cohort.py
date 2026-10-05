#!/usr/bin/env python3
"""Verify preservation and summarize the saved V3 five-seed cohort."""
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]/'logs'/'v3-cohort'
OUT=Path(__file__).resolve().parents[1]/'data'/'v3-cohort-analysis.json'
SEEDS=[808,1669,1993,2281,2293]
def strip_work(v):
 if isinstance(v,dict):return {k:strip_work(x) for k,x in v.items() if k!='work_spent'}
 if isinstance(v,list):return [strip_work(x) for x in v]
 return v
def events(path):
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
  fallback=events(d/'r58-fallback.stderr.log'); changed=[]; positive_changed=[]
  for team,positions in base['rare_position_estimates'].items():
   for rank,e in positions.items():
    if strip_work(e)!=strip_work(cand['rare_position_estimates'][team][rank]):
     cell=f'{team}:{rank}';changed.append(cell)
     if e.get('probability',0)>0:positive_changed.append(cell)
  if positive_changed:raise AssertionError(f'seed{seed}: changed baseline-positive evidence {positive_changed}')
  if base['game_importance']!=cand['game_importance']:raise AssertionError(f'seed{seed}: game-importance changed')
  if seed in (1669,1993,2281) and fallback:raise AssertionError(f'seed{seed}: unexpected fallback attempt')
  runs={r['name']:r for r in json.loads((d/'summary.json').read_text())['records']}
  row={'seed':seed,'baseline_probability':base['rare_position_estimates']['17']['11']['probability'],'fallback_probability':cand['rare_position_estimates']['17']['11']['probability'],'fallback_events':fallback,'baseline_positive_cells_changed_excluding_work_spent':positive_changed,'changed_estimate_cells_excluding_work_spent':changed,'game_importance_identical':True,'full_wall_baseline_s':runs['current-rust']['wall_seconds'],'full_wall_fallback_s':runs['r58-fallback']['wall_seconds'],'cpu_baseline_s':runs['current-rust']['child_cpu_seconds'],'cpu_fallback_s':runs['r58-fallback']['child_cpu_seconds']}
  if seed==808:row['flag_off_export_byte_identical_to_current_rust']=(d/'current-rust.json').read_bytes()==(d/'r58-flag-off.json').read_bytes()
  rows.append(row)
 OUT.write_text(json.dumps({'seeds':rows},indent=2)+'\n')
if __name__=='__main__':main()
