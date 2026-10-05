"""Analyze authorized V6 timing repetitions and selected 10x references."""
import json, statistics
from pathlib import Path
ART=Path(__file__).resolve().parents[1]
LOG=ART/'logs'; V3=LOG/'v3-cohort'; CORE=LOG/'v6-screen'; TIME=LOG/'v6-timing'; REF=LOG/'v6-reference-controls'
def mean(values): return statistics.mean(values) if values else 0.0
def main():
    td=json.loads((TIME/'summary.json').read_text())
    timing=[]
    for arm in ('baseline','native-order-cap3000','grant-order-cap3000'):
        for seed in (808,2293):
            rr=[r for r in td['records'] if r['arm']==arm and r['seed']==seed]
            timing.append({'arm':arm,'seed':seed,'repeat_count':len(rr),'wall_ms_mean':mean([r['wall_seconds']*1000 for r in rr]),'wall_ms_median':statistics.median([r['wall_seconds']*1000 for r in rr]),'cpu_ms_mean':mean([r['child_cpu_seconds']*1000 for r in rr]),'system_cpu_ms_mean':mean([r['child_system_seconds']*1000 for r in rr]),'stage_ms_mean':{stage:mean([r['full_pipeline_stage_ms'].get(stage,0) for r in rr]) for stage in sorted({s for r in rr for s in r['full_pipeline_stage_ms']})},'all_exports_match_frozen_screen':all(r['actual_export_sha256']==r['expected_export_sha256'] for r in rr)})
    timing_index={(r['arm'],r['seed']):r for r in timing}
    for r in timing:
        if r['arm']!='baseline':
            b=timing_index[('baseline',r['seed'])]
            r['wall_ms_delta_vs_baseline']=r['wall_ms_mean']-b['wall_ms_mean']
            r['cpu_ms_delta_vs_baseline']=r['cpu_ms_mean']-b['cpu_ms_mean']
    controls=json.loads((REF/'summary.json').read_text()); ref_rows=[]
    for r in controls['records']:
        target_team='16'; rank_index=str(int(r['cell'].split(':')[1])-1)
        export=json.loads(Path(r['export']).read_text())
        base=json.loads((V3/f"seed{r['seed']}"/'current-rust.json').read_text())
        target=export['rare_position_estimates'][target_team][rank_index]
        base_target=base['rare_position_estimates'][target_team][rank_index]
        event=r['fallback_events'][0]['fallback'] if r['fallback_events'] else {}
        fs=event.get('family_summary',{}).get('final',{})
        ref_rows.append({'seed':r['seed'],'cell':r['cell'],'team_id':target_team,'rank_index':rank_index,'mode':r['mode'],'multiplier':10,'baseline_probability':base_target.get('probability'),'candidate_probability':target.get('probability'),'candidate_design':target.get('design'),'published':event.get('published',False),'settlement_within_grant':event.get('settlement_within_grant'),'base_fallback_grant':event.get('base_fallback_grant'),'expanded_fallback_grant':event.get('fallback_grant'),'added_actual_work':event.get('added_actual_work'),'replay_work':event.get('training_operation_work',0),'collection_work':event.get('family_collection_work',0),'clone_work':event.get('clone_work',0),'fit_work':event.get('fit_work',0),'validation_work':event.get('validation_work',0),'main_check_work':event.get('final_work',0),'ledger_matches':sum(event.get(k,0) for k in ('training_operation_work','family_collection_work','clone_work','fit_work','validation_work','final_work'))==event.get('added_actual_work'),'reason':event.get('reason'),'quality':fs,'wall_seconds':r['wall_seconds'],'child_cpu_seconds':r['child_cpu_seconds']})
    parity=[]
    def clean_summary(x):
        x=dict(x)
        for k in ('elapsed_ms','timestamp_unix_ms','request_elapsed_ms','family_fallback_order','family_fallback_order_active','family_fallback_order_setup_work'):x.pop(k,None)
        return x
    def clean_events(rec):
        result=[]
        for e in rec['fallback_events']:
            x=dict(e['fallback'])
            for k in ('stage_ms','attempt_order_index','full_original_unused_grant','ordering_metric','original_priority','starting_bank_free','bank_reserved_after_native','bank_reserved_before_fallback'):x.pop(k,None)
            result.append(x)
        return result
    for seed in (808,2293):
        a=json.loads((CORE/'native-order-v5-parity'/f'seed{seed}'/'record.json').read_text())
        b=json.loads((LOG/'v5-final-screen'/'all-family-1x'/f'seed{seed}'/'record.json').read_text())
        parity.append({'seed':seed,'export_exact':Path(a['export']).read_bytes()==Path(b['export']).read_bytes(),'native_counters_equal':clean_summary(a['confirmation_summaries'][0])==clean_summary(b['confirmation_summaries'][0]),'fallback_operation_records_equal':clean_events(a)==clean_events(b)})
    result={'timing_records':timing,'timing_all_frozen_exports_match':all(r['all_exports_match_frozen_screen'] for r in timing),'native_order_v5_parity':parity,'reference_controls':ref_rows,'reference_all_ledgers_match':all(r['ledger_matches'] for r in ref_rows),'reference_overruns':sum(not r['settlement_within_grant'] for r in ref_rows)}
    (ART/'data'/'v6-followup-analysis.json').write_text(json.dumps(result,indent=2)+'\n')
if __name__=='__main__':main()
