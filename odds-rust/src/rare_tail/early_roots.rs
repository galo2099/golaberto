//! Experimental early-rank proposal for rare zero-probability cells.
//!
//! The ranked paths only shape an importance proposal. Every published value
//! still comes from fresh `LazyJoint` main/check samples with the usual gates.
use super::*;
use crate::joint_caps::propagated::lazy::LazyJoint;

const MAX_CELLS: usize = 4;
const PILOT_DRAWS: usize = 500;
const MIN_FINAL_DRAWS: usize = 1_000;
const MAX_FINAL_DRAWS: usize = 6_000;
const FINAL_HEADROOM_PERCENT: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CellCandidate {
    index: usize,
    cell: Cell,
}

fn zero_non_impossible(estimate: &Estimate) -> bool {
    estimate.probability == 0. && !estimate.reachability.starts_with("impossible")
}

fn candidates(estimates: &[Estimate], n: usize) -> Vec<CellCandidate> {
    estimates
        .iter()
        .enumerate()
        .filter(|(_, estimate)| zero_non_impossible(estimate))
        .take(MAX_CELLS)
        .map(|(index, _)| CellCandidate {
            index,
            cell: Cell {
                team: index / n,
                rank: index % n,
            },
        })
        .collect()
}

fn reserve_all(bank: &mut budget::WorkBudget) -> Option<usize> {
    let grant = bank.limit.saturating_sub(bank.reserved);
    (grant > 0 && bank.reserve(grant)).then_some(grant)
}

fn settle(bank: &mut budget::WorkBudget, grant: usize, actual: usize) -> bool {
    if actual > grant {
        return false;
    }
    bank.release(grant - actual)
}

fn setup_limit(grant: usize) -> usize {
    grant / 2
}

fn final_draws(remaining: usize, per_draw: usize) -> usize {
    if per_draw == 0 {
        return 0;
    }
    (remaining.saturating_mul(100) / (100 + FINAL_HEADROOM_PERCENT))
        .checked_div(per_draw.saturating_mul(2))
        .unwrap_or(0)
        .min(MAX_FINAL_DRAWS)
}

fn publishable_pair(estimate: &Estimate, main: &Result, check: &Result, rough: bool) -> bool {
    zero_non_impossible(estimate) && super::accepted(main, check, rough)
}

fn phase_seed(seed: i64, team_id: i32, rank: usize, phase: &str) -> i64 {
    derive(seed, &format!("early-rank-{phase}-{team_id}-{rank}"))
}

pub(super) fn run(
    m: &Model,
    seed: i64,
    estimates: &mut [Estimate],
    bank: &mut budget::WorkBudget,
    log: Option<&crate::logging::RequestLog>,
    rough: bool,
    branch_draw_audit: super::branches::DrawAudit,
    branch_draw_credit_requested: usize,
    bank_limit_before_credit: usize,
    bank_limit_after_credit: usize,
) -> (usize, u64) {
    let started = Instant::now();
    let mode = super::experimental_early_rank_mode();
    let roots = super::experimental_early_rank_roots();
    let before: Vec<_> = estimates
        .iter()
        .enumerate()
        .filter(|(_, estimate)| estimate.probability > 0.)
        .map(|(index, estimate)| (index, serde_json::to_value(estimate).unwrap()))
        .collect();
    let selected = candidates(estimates, m.n);
    let mut added = 0usize;
    let mut charged_total = 0usize;
    let mut granted_total = 0usize;
    let mut stopped = false;
    let mut stop_reason = None;
    let mut rows = Vec::new();

    if !budget::modeled() || mode.is_none() {
        emit(
            log,
            "rust_odds_early_rank_root_summary",
            json!({"enabled":mode.is_some(),"status":"skipped","reason":if !budget::modeled() {"operation work model required"} else {"flag unset or invalid"},"roots":roots,"attempted":0,"added":0,"actual_work":0,"preserved_existing_estimates":true,"elapsed_ms":started.elapsed().as_secs_f64()*1000.}),
        );
        return (0, 0);
    }
    let bias = mode.unwrap();

    for candidate_cell in selected {
        let index = candidate_cell.index;
        let cell = candidate_cell.cell;
        if !zero_non_impossible(&estimates[index]) {
            continue;
        }
        if stopped {
            break;
        }
        let cell_started = Instant::now();
        let free_before = bank.limit.saturating_sub(bank.reserved);
        let Some(setup_grant) = reserve_all(bank) else {
            stop_reason = Some("no residual modeled work for constructor");
            break;
        };
        let setup_budget = setup_limit(setup_grant);
        let construct_seed = phase_seed(seed, m.ids[cell.team], cell.rank, "construct");
        let setup_started = Instant::now();
        let (proposal_result, setup_work) =
            LazyJoint::with_early_rank(m, cell, construct_seed, roots, bias, setup_budget);
        let setup_ms = setup_started.elapsed().as_secs_f64() * 1000.;
        granted_total = granted_total.saturating_add(setup_grant);
        charged_total = charged_total.saturating_add(setup_work);
        let settled = settle(bank, setup_grant, setup_work);
        let mut row = json!({
            "team_id":m.ids[cell.team],"team_index":cell.team,"rank":cell.rank+1,
            "roots":roots,"bias":bias,"bank_free_before":free_before,
            "constructor_grant":setup_grant,"constructor_budget":setup_budget,
            "constructor_work":setup_work,"constructor_settled":settled,"constructor_ms":setup_ms,
            "elapsed_ms":cell_started.elapsed().as_secs_f64()*1000.
        });
        let proposal = match proposal_result {
            Ok(proposal) if settled && setup_work <= setup_budget => proposal,
            Ok(_) => {
                stopped = true;
                stop_reason = Some("constructor work exceeded its setup grant");
                row["status"] = json!("overrun");
                rows.push(row);
                break;
            }
            Err(error) => {
                row["status"] = json!("constructor_failed");
                row["error"] = json!(error);
                row["work_charged"] = json!(setup_work);
                rows.push(row);
                if !settled || setup_work > setup_budget {
                    stopped = true;
                    stop_reason = Some("failed constructor work exceeded its setup grant");
                    break;
                }
                continue;
            }
        };

        let diagnostics = proposal.early_rank_diagnostics();
        let pilot_grant = match reserve_all(bank) {
            Some(grant) => grant,
            None => {
                row["status"] = json!("no_pilot_bank");
                rows.push(row);
                break;
            }
        };
        let pilot_preflight = budget::reference_cost(m).saturating_mul(PILOT_DRAWS);
        if pilot_preflight > pilot_grant {
            assert!(bank.release(pilot_grant));
            row["status"] = json!("pilot_below_deterministic_preflight");
            row["pilot_preflight_work"] = json!(pilot_preflight);
            rows.push(row);
            continue;
        }
        let pilot_seed = phase_seed(seed, m.ids[cell.team], cell.rank, "pilot");
        let pilot_started = Instant::now();
        let pilot = proposal.sample(m, PILOT_DRAWS, pilot_seed);
        let pilot_ms = pilot_started.elapsed().as_secs_f64() * 1000.;
        let pilot_work = pilot.operations.units();
        granted_total = granted_total.saturating_add(pilot_grant);
        charged_total = charged_total.saturating_add(pilot_work);
        let pilot_settled = settle(bank, pilot_grant, pilot_work);
        row["pilot"] = json!({"draws":pilot.samples,"hits":pilot.hits,"ess":pilot.ess,
            "max_share":pilot.max_share,"batch_gap":pilot.batch_gap,
            "relative_se":if pilot.probability>0.{Some(pilot.std_err/pilot.probability)}else{None},
            "work":pilot_work,"grant":pilot_grant,"settled":pilot_settled,
            "elapsed_ms":pilot_ms});
        if !pilot_settled {
            row["status"] = json!("pilot_overrun");
            rows.push(row);
            stopped = true;
            stop_reason = Some("pilot work exceeded reserved bank");
            break;
        }

        // The pilot-measured cost is an admission proxy. The final batch size
        // is fixed before the independent main/check streams begin.
        let pilot_per_draw = budget::per_draw(&pilot).max(1);
        let sampling_grant = match reserve_all(bank) {
            Some(grant) => grant,
            None => {
                row["status"] = json!("no_final_bank");
                rows.push(row);
                break;
            }
        };
        let final_n = final_draws(sampling_grant, pilot_per_draw);
        row["preflight"] = json!({"pilot_per_draw":pilot_per_draw,"final_draws_per_stream":final_n,
            "sampling_grant":sampling_grant});
        if final_n < MIN_FINAL_DRAWS {
            assert!(bank.release(sampling_grant));
            row["status"] = json!("final_batch_below_minimum");
            row["work_charged"] = json!(setup_work.saturating_add(pilot_work));
            rows.push(row);
            continue;
        }

        let pair_started = Instant::now();
        let main = proposal.sample(
            m,
            final_n,
            phase_seed(seed, m.ids[cell.team], cell.rank, "main"),
        );
        let main_work = main.operations.units();
        let check = if publishable(&main, rough) && main_work <= sampling_grant {
            proposal.sample(
                m,
                final_n,
                phase_seed(seed, m.ids[cell.team], cell.rank, "check"),
            )
        } else {
            Result::default()
        };
        let check_work = check.operations.units();
        let actual = main_work.saturating_add(check_work);
        granted_total = granted_total.saturating_add(sampling_grant);
        charged_total = charged_total.saturating_add(actual);
        let pair_settled = settle(bank, sampling_grant, actual);
        let accepted = pair_settled && publishable_pair(&estimates[index], &main, &check, rough);
        if accepted && commit_estimate(&mut estimates[index], &main, true) {
            estimates[index].design = "matched_point_pool_early_rank_roots".into();
            added += 1;
        }
        row["status"] = json!(if accepted {
            "accepted"
        } else if pair_settled {
            "declined"
        } else {
            "overrun"
        });
        row["early_rank"] = diagnostics.clone();
        row["pilot"] = json!({"draws":pilot.samples,"hits":pilot.hits,"ess":pilot.ess,
            "max_share":pilot.max_share,"batch_gap":pilot.batch_gap,
            "relative_se":if pilot.probability>0.{Some(pilot.std_err/pilot.probability)}else{None},
            "work":pilot_work,"grant":pilot_grant,"settled":pilot_settled});
        row["main_check"] = json!({"draws_per_stream":final_n,"main_hits":main.hits,"main_ess":main.ess,
            "main_max_share":main.max_share,"main_batch_gap":main.batch_gap,
            "main_relative_se":if main.probability>0.{Some(main.std_err/main.probability)}else{None},
            "check_hits":check.hits,"check_ess":check.ess,"check_max_share":check.max_share,
            "check_batch_gap":check.batch_gap,
            "check_relative_se":if check.probability>0.{Some(check.std_err/check.probability)}else{None},
            "main_check_ratio":if check.probability>0.{Some(main.probability/check.probability)}else{None},
            "main_work":main_work,"check_work":check_work,"grant":sampling_grant,"actual":actual,
            "settled":pair_settled,"accepted":accepted,"elapsed_ms":pair_started.elapsed().as_secs_f64()*1000.});
        row["score_concentration"] = diagnostics
            .get("by_target_gain")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        row["top_paths"] = diagnostics
            .get("selected_paths")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        row["elapsed_ms"] = json!(cell_started.elapsed().as_secs_f64() * 1000.);
        rows.push(row);
        if !pair_settled {
            stopped = true;
            stop_reason = Some("main/check work exceeded reserved bank");
        }
    }

    let preserved = before
        .iter()
        .all(|(index, estimate)| *estimate == serde_json::to_value(&estimates[*index]).unwrap());
    for row in &rows {
        emit(
            log,
            "rust_odds_early_rank_root",
            json!({"group":m.request.id,"root":row}),
        );
    }
    emit(
        log,
        "rust_odds_early_rank_root_summary",
        json!({"enabled":true,"mode":if bias {"bias"}else{"order"},"roots":roots,
            "max_cells":MAX_CELLS,"pilot_draws":PILOT_DRAWS,"final_draw_cap":MAX_FINAL_DRAWS,
            "final_headroom_percent":FINAL_HEADROOM_PERCENT,"bank_limit":bank.limit,
            "bank_free_after":bank.limit.saturating_sub(bank.reserved),"attempted":rows.len(),
            "branch_draw_credit_requested":branch_draw_credit_requested,
            "branch_draw_credit_computed":branch_draw_audit.credit,
            "branch_draw_audit_valid":branch_draw_audit.valid,
            "branch_draw_reserved_units":branch_draw_audit.reserved_draw_units,
            "branch_draw_actual_units":branch_draw_audit.actual_draw_units,
            "branch_draw_already_released_units":branch_draw_audit.already_released_draw_units,
            "bank_limit_before_credit":bank_limit_before_credit,
            "bank_limit_after_credit":bank_limit_after_credit,
            "branch_draw_credit_applied":bank_limit_after_credit.saturating_sub(bank_limit_before_credit),
            "added":added,"cumulative_stage_grants_nonadditive":granted_total,"actual_work":charged_total,
            "stopped":stopped,"stop_reason":stop_reason,"cells":rows,
            "preserved_existing_estimates":preserved,"elapsed_ms":started.elapsed().as_secs_f64()*1000.}),
    );
    (added, charged_total as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_are_stable_bounded_and_exclude_nonzero_or_impossible() {
        let estimates = vec![Estimate::default(); 8];
        let selected = candidates(&estimates, 4);
        assert_eq!(selected.len(), MAX_CELLS);
        assert_eq!(
            selected.iter().map(|item| item.index).collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        let mut estimates = estimates;
        estimates[0].probability = 0.01;
        estimates[1].reachability = "impossible:proof".into();
        assert_eq!(candidates(&estimates, 4)[0].index, 2);
    }

    #[test]
    fn setup_and_pair_settlement_preserve_only_unused_bank() {
        let mut bank = budget::WorkBudget::new(10_000);
        let grant = reserve_all(&mut bank).unwrap();
        assert_eq!(setup_limit(grant), 5_000);
        assert!(settle(&mut bank, grant, 1_000));
        assert_eq!(bank.reserved, 1_000);
        let pair_grant = reserve_all(&mut bank).unwrap();
        assert!(settle(&mut bank, pair_grant, 3_000));
        assert_eq!(bank.reserved, 4_000);
    }

    #[test]
    fn constructor_failure_still_settles_returned_setup_work() {
        let mut bank = budget::WorkBudget::new(8_000);
        let grant = reserve_all(&mut bank).unwrap();
        let failed_setup_work = 1_250;
        assert!(settle(&mut bank, grant, failed_setup_work));
        assert_eq!(bank.reserved, failed_setup_work);
    }

    #[test]
    fn early_pair_uses_normal_acceptance_and_cannot_replace_positive_estimate() {
        let main = Result {
            weighted: true,
            probability: 0.01,
            std_err: 0.002,
            hits: 40,
            ess: 20.,
            max_share: 0.1,
            batch_gap: 1.,
            ..Default::default()
        };
        let check = main.clone();
        let empty = Estimate::default();
        assert!(publishable_pair(&empty, &main, &check, true));
        let positive = Estimate {
            probability: 0.02,
            design: "existing".into(),
            ..Estimate::default()
        };
        assert!(!publishable_pair(&positive, &main, &check, true));
        let impossible = Estimate {
            reachability: "impossible:certified".into(),
            ..Estimate::default()
        };
        assert!(!publishable_pair(&impossible, &main, &check, true));
    }

    #[test]
    fn final_batch_respects_headroom_and_caps() {
        assert_eq!(final_draws(21_600, 1), 6_000);
        assert_eq!(final_draws(21_600, 9), 1_111);
        assert_eq!(final_draws(1_000, 1), 462);
    }
}
