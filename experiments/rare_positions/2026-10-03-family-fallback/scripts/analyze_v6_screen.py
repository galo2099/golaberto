"""Summarize V6 matrix, work, setup and preservation results."""
import json
from pathlib import Path
ART = Path(__file__).resolve().parents[1]
ROOT = ART / "logs" / "v6-screen"
V3 = ART / "logs" / "v3-cohort"
V4 = ART / "logs" / "v4-breadth"
V5 = ART / "logs" / "v5-final-screen"
OUT = ART / "data" / "v6-screen-analysis.json"
SEEDS = [808, 1669, 1993, 2281, 2293]
ARMS = ["grant-order-cap5000", "native-order-cap3000", "grant-order-cap3000"]

def strip_work(x):
    if isinstance(x, dict): return {k: strip_work(v) for k,v in x.items() if k != "work_spent"}
    if isinstance(x, list): return [strip_work(v) for v in x]
    return x

def matrix(x): return x["rare_position_estimates"]
def gains(base, cand):
    out=[]
    for team,ranks in matrix(base).items():
        for rank,old in ranks.items():
            new=matrix(cand)[team][rank]
            if old.get("probability",0)==0 and new.get("probability",0)>0:
                out.append({"team_id":team,"rank_index":rank,"probability":new["probability"],"design":new.get("design")})
    return out

def v4_gains():
    out=set()
    for seed in SEEDS:
        base=json.loads((V3/f"seed{seed}"/"current-rust.json").read_text())
        for selector in ("16-14","16-15"):
            path=V4/selector/"family-1x"/f"seed{seed}"/"export.json"
            cand=json.loads(path.read_text())
            out.update((seed,g["team_id"],g["rank_index"]) for g in gains(base,cand))
    return out

def main():
    rows=[]; preserved=True; all_v4=v4_gains(); all_gains_by_arm={}
    for arm in ARMS:
        arm_gains=set()
        for seed in SEEDS:
            d=ROOT/arm/f"seed{seed}"; record=json.loads((d/"record.json").read_text())
            cand=json.loads((d/"export.json").read_text()); base=json.loads((V3/f"seed{seed}"/"current-rust.json").read_text())
            changed=[]; positive_changed=[]
            for team,ranks in matrix(base).items():
                for rank,old in ranks.items():
                    new=matrix(cand)[team][rank]
                    if strip_work(old)!=strip_work(new):
                        changed.append(f"{team}:{rank}")
                        if old.get("probability",0)>0: positive_changed.append(f"{team}:{rank}")
            same_importance=base["game_importance"]==cand["game_importance"]
            if positive_changed or not same_importance: preserved=False
            cell_gains=gains(base,cand)
            keys={(seed,g["team_id"],g["rank_index"]) for g in cell_gains}; arm_gains |= keys
            summary=record["confirmation_summaries"][0] if record["confirmation_summaries"] else {}
            events=[e["fallback"] for e in record["fallback_events"]]
            stages={}
            for event in events:
                for name,ms in event.get("stage_ms",{}).items(): stages[name]=stages.get(name,0)+ms
            order_work=summary.get("family_fallback_order_setup_work",0)
            attempt_work=sum(e.get("added_actual_work",0) for e in events)
            reported_work=summary.get("family_fallback_added_work",0)
            rows.append({"arm":arm,"seed":seed,"export_sha256":record["export_sha256"],"wall_seconds":record["wall_seconds"],"child_user_seconds":record["child_user_seconds"],"child_system_seconds":record["child_system_seconds"],"child_cpu_seconds":record["child_cpu_seconds"],"new_positive_cells":cell_gains,"gains_already_seen_v4":[g for g in cell_gains if (seed,g["team_id"],g["rank_index"]) in all_v4],"baseline_positive_cells_changed_excluding_work_spent":positive_changed,"changed_estimate_cells_excluding_work_spent":changed,"game_importance_identical":same_importance,"remaining_reachable_zeros":[{"team_id":t,"rank_index":r} for t,rs in matrix(cand).items() for r,e in rs.items() if e.get("probability",0)==0 and e.get("reachability")=="reachable"],"order_setup_work":order_work,"attempted_charge_sum":attempt_work,"reported_total_fallback_work":reported_work,"work_ledger_matches":order_work+attempt_work==reported_work,"attempts":summary.get("family_fallback_attempts"),"quality_declines":summary.get("family_fallback_quality_declines"),"budget_declines":summary.get("family_fallback_budget_declines"),"published_charged_work":summary.get("family_fallback_published_charged_work"),"initial_post_native_bank_free":summary.get("family_fallback_initial_post_native_bank_free"),"final_bank_free":summary.get("family_fallback_final_bank_free"),"overrun_count":sum(bool(e.get("overrun")) for e in events),"stopped_after_overrun":summary.get("family_fallback_stopped_after_overrun"),"order_events":record["order_events"],"order_declines":record["order_declines"],"fallback_events":events,"fallback_stage_ms_sum":stages})
        all_gains_by_arm[arm]=[list(x) for x in sorted(arm_gains)]
    control=[]
    for seed in SEEDS:
        path=V5/"all-family-1x"/f"seed{seed}"/"export.json"
        cand=json.loads(path.read_text()); base=json.loads((V3/f"seed{seed}"/"current-rust.json").read_text())
        control.extend((seed,g["team_id"],g["rank_index"]) for g in gains(base,cand))
    overlaps={arm: [x for x in all_gains_by_arm[arm] if tuple(x) in set(tuple(y) for y in control)] for arm in ARMS}
    OUT.write_text(json.dumps({"baseline_positive_and_game_importance_preserved_all_runs":preserved,"all_fallback_work_ledgers_match":all(r["work_ledger_matches"] for r in rows),"zero_overruns":all(r["overrun_count"]==0 for r in rows),"v5_native_order_cap5000_control_gains":[list(x) for x in sorted(control)],"gains_by_arm":all_gains_by_arm,"gains_already_seen_in_v5_control":overlaps,"runs":rows},indent=2)+"\n")
if __name__=="__main__": main()
