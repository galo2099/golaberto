//! Bounded late complete-target-tree fallback.
//!
//! This stage is deliberately separate from the native and existing family
//! streams. It consumes only unused confirmation-bank units after those stages
//! have completed, and a positive estimate is committed only after an
//! independent main/check pair passes the normal acceptance gate.
use super::*;
use crate::joint_caps::propagated::lazy::{combine_branches, BranchSample, BranchStrata};
use std::collections::HashMap;

const ROOT_LIMIT: usize = 256;
const LEGACY_MIN_ROOTS: usize = 65;
const DEFAULT_MIN_ROOTS: usize = 1;
const MAX_CELLS: usize = 4;
const DEFAULT_FLOOR: usize = 2;
const MAX_FLOOR: usize = 10;
const FINAL_HEADROOM_PERCENT: usize = 2;

fn bounded_setting_value(raw: Option<&str>, default: usize, min: usize, max: usize) -> usize {
    raw.and_then(|value| value.parse::<usize>().ok())
        .map_or(default, |value| value.clamp(min, max))
}

fn bounded_setting(name: &str, default: usize, min: usize, max: usize) -> usize {
    bounded_setting_value(std::env::var(name).ok().as_deref(), default, min, max)
}

fn pilot_draws() -> usize {
    bounded_setting("RUST_ODDS_EXPERIMENT_TARGET_TREE_PILOT_DRAWS", 5, 2, 50)
}

fn message_count() -> usize {
    bounded_setting("RUST_ODDS_EXPERIMENT_TARGET_TREE_MESSAGES", 3, 0, 3)
}

fn structural_discovery_enabled() -> bool {
    matches!(
        std::env::var("RUST_ODDS_EXPERIMENT_TREE_DISCOVERY").as_deref(),
        Ok("interval" | "rank" | "strict")
    )
}

fn final_draws() -> usize {
    bounded_setting(
        "RUST_ODDS_EXPERIMENT_TARGET_TREE_FINAL_DRAWS",
        6_000,
        3_000,
        6_000,
    )
}

fn final_draw_floor() -> usize {
    bounded_setting(
        "RUST_ODDS_EXPERIMENT_TARGET_TREE_DRAW_FLOOR",
        DEFAULT_FLOOR,
        2,
        MAX_FLOOR,
    )
}

fn min_roots_value(raw: Option<&str>) -> usize {
    bounded_setting_value(raw, DEFAULT_MIN_ROOTS, 1, LEGACY_MIN_ROOTS)
}

fn min_roots() -> usize {
    min_roots_value(
        std::env::var("RUST_ODDS_EXPERIMENT_TARGET_TREE_MIN_ROOTS")
            .ok()
            .as_deref(),
    )
}

fn accepted_root_count(roots: usize, minimum: usize) -> bool {
    (minimum..=ROOT_LIMIT).contains(&roots)
}

fn candidate_sort_key(candidate: Candidate, minimum: usize, n: usize) -> (usize, usize, usize) {
    let new_candidate_bucket =
        usize::from(minimum < LEGACY_MIN_ROOTS && candidate.roots < LEGACY_MIN_ROOTS);
    (
        new_candidate_bucket,
        candidate.roots,
        candidate.cell.index(n),
    )
}

pub(super) fn reclaim_enabled() -> bool {
    reclaim_value(
        std::env::var("RUST_ODDS_EXPERIMENT_TARGET_TREE_RECLAIM")
            .ok()
            .as_deref(),
    )
}

pub(super) fn requested_branch_credit(audit: super::branches::DrawAudit, transfer: usize) -> usize {
    if audit.valid {
        audit.credit.min(transfer)
    } else {
        0
    }
}

pub(super) fn applied_branch_credit(
    audit: super::branches::DrawAudit,
    transfer: usize,
    current_limit: usize,
    original_limit: usize,
    reclaim: bool,
) -> usize {
    if !reclaim {
        return 0;
    }
    requested_branch_credit(audit, transfer).min(original_limit.saturating_sub(current_limit))
}

fn reclaim_value(raw: Option<&str>) -> bool {
    raw == Some("1")
}

pub(super) fn settings() -> serde_json::Value {
    serde_json::json!({
        "enabled": super::target_overflow_tree_enabled(),
        "pilot_draws": pilot_draws(),
        "floor": final_draw_floor(),
        "messages": message_count(),
        "final_cap": final_draws(),
        "reclaim": reclaim_enabled(),
        "max_roots": ROOT_LIMIT,
        "min_roots": min_roots(),
        "max_cells": MAX_CELLS,
        "funding": "confirmation_bank",
        "late": true,
        "max_workers": 4
    })
}

#[derive(Clone, Copy)]
struct Candidate {
    cell: Cell,
    roots: usize,
}

fn zero_non_impossible(estimate: &Estimate) -> bool {
    estimate.probability == 0. && !estimate.reachability.starts_with("impossible")
}

fn pair_is_publishable(estimate: &Estimate, main: &Result, check: &Result) -> bool {
    zero_non_impossible(estimate) && accepted(main, check, true)
}

fn sampling_seed(seed: i64, team_id: i32, rank: usize) -> i64 {
    derive(seed, &format!("target-overflow-sampling-{team_id}-{rank}"))
}

fn settle_stage(bank: &mut WorkBudget, grant: usize, actual: usize) -> bool {
    if actual > grant {
        return false;
    }
    bank.release(grant - actual)
}

fn reserve_all(bank: &mut WorkBudget) -> Option<usize> {
    let grant = bank.limit.saturating_sub(bank.reserved);
    (grant > 0 && bank.reserve(grant)).then_some(grant)
}

fn setup_grant_admitted(grant: usize, admission_floor: usize) -> bool {
    grant >= admission_floor
}

fn allocate(scores: &[f64], total: usize, floor: usize) -> Option<Vec<usize>> {
    if floor < 2 || scores.is_empty() || scores.iter().any(|x| !x.is_finite() || *x < 0.) {
        return None;
    }
    let mut counts = vec![floor; scores.len()];
    let base = floor.checked_mul(scores.len())?;
    if base > total {
        return None;
    }
    let sum: f64 = scores.iter().sum();
    let remaining = total - base;
    for (i, score) in scores.iter().enumerate() {
        let share = if sum > 0. {
            0.1 / scores.len() as f64 + 0.9 * *score / sum
        } else {
            1. / scores.len() as f64
        };
        counts[i] += (remaining as f64 * share).floor() as usize;
    }
    let remainder = total - counts.iter().sum::<usize>();
    let len = counts.len();
    for i in 0..remainder {
        counts[i % len] += 1;
    }
    Some(counts)
}

fn sample_stage(
    p: &BranchStrata,
    m: &Model,
    ns: &[usize],
    seed: i64,
    phase: &'static str,
    workers: usize,
) -> Vec<BranchSample> {
    let costs: Vec<_> = (0..p.len())
        .map(|i| ns[i] * p.draw_work(m, i, true))
        .collect();
    let mut rows = budget::parallel(&costs, workers, |i| {
        let sample = p.sample(
            m,
            i,
            ns[i],
            derive(seed, &format!("branch-{phase}-{i}")),
            true,
            true,
        );
        (i, sample)
    });
    rows.sort_by_key(|row| row.0);
    rows.into_iter().map(|row| row.1).collect()
}

fn sample_selected(
    p: &BranchStrata,
    m: &Model,
    selected: &[usize],
    n: usize,
    seed: i64,
    workers: usize,
) -> Vec<(usize, BranchSample)> {
    let costs: Vec<_> = selected
        .iter()
        .map(|&i| n * p.draw_work(m, i, true))
        .collect();
    let mut rows = budget::parallel(&costs, workers, |slot| {
        let i = selected[slot];
        let sample = p.sample(
            m,
            i,
            n,
            derive(seed, &format!("message-pilot-{i}")),
            true,
            true,
        );
        (i, sample)
    });
    rows.sort_by_key(|row| row.0);
    rows
}

fn stage_charge(samples: &[BranchSample]) -> usize {
    samples
        .iter()
        .map(|sample| {
            if budget::modeled() {
                sample.result.operations.units()
            } else {
                sample.result.samples
            }
        })
        .sum()
}

fn work_cost_from_estimates(p: &BranchStrata, m: &Model, ns: &[usize], paired: bool) -> usize {
    ns.iter()
        .enumerate()
        .map(|(i, &n)| {
            let per_draw = p.draw_work(m, i, true);
            n.saturating_mul(per_draw)
                .saturating_mul(if paired { 2 } else { 1 })
        })
        .sum()
}

/// Spend the residual confirmation bank on up to four complete target trees.
/// The caller invokes this only after all native and family fallback work.
pub(super) fn run(
    m: &Model,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    bank: &mut WorkBudget,
    log: Option<&crate::logging::RequestLog>,
    branch_draw_audit: super::branches::DrawAudit,
    branch_draw_credit_requested: usize,
    bank_limit_before_credit: usize,
    bank_limit_after_credit: usize,
) -> (usize, u64) {
    let started = Instant::now();
    let min_roots = min_roots();
    let pilot_draws = pilot_draws();
    let messages = message_count();
    let nominal_final_draws = final_draws();
    let draw_floor = final_draw_floor();
    let bank_before = bank.limit.saturating_sub(bank.reserved);
    let mut charged = 0usize;
    let mut granted = 0usize;
    let mut added = 0usize;
    let mut root_attempts = 0usize;
    let mut roots_counted = 0usize;
    let mut candidates = Vec::new();
    let mut root_count_work = HashMap::new();
    let mut setup_rows = Vec::new();
    let mut stage_rows = Vec::new();
    let mut stopped = false;
    let mut stop_reason = None;
    let before: Vec<_> = estimates
        .iter()
        .enumerate()
        .filter(|(_, estimate)| estimate.probability > 0.)
        .map(|(i, estimate)| (i, serde_json::to_value(estimate).unwrap()))
        .collect();

    // The tree work counters are modeled operation units. Skip in modes whose
    // confirmation bank uses a different unit, rather than mixing currencies.
    if !budget::modeled() {
        emit(
            log,
            "rust_odds_target_overflow_tree_summary",
            json!({
            "enabled": true, "status": "skipped", "reason": "operation work model required",
            "pilot_draws_per_branch":pilot_draws,"message_count":messages,
            "nominal_final_draws":nominal_final_draws,"final_draw_floor":draw_floor,"reclaim_enabled":reclaim_enabled(),
            "bank_free_before": bank_before, "added": 0,
            "branch_draw_credit_requested":branch_draw_credit_requested,"branch_draw_credit_computed":branch_draw_audit.credit,
            "branch_draw_audit_valid":branch_draw_audit.valid,"branch_draw_reserved_units":branch_draw_audit.reserved_draw_units,
            "branch_draw_actual_units":branch_draw_audit.actual_draw_units,
            "branch_draw_already_released_units":branch_draw_audit.already_released_draw_units,
            "bank_limit_before_credit":bank_limit_before_credit,"bank_limit_after_credit":bank_limit_after_credit,
                "preserved_existing_estimates": before.iter().all(|(i,e)| *e == serde_json::to_value(&estimates[*i]).unwrap()),
                "elapsed_ms": started.elapsed().as_secs_f64() * 1000.0,
            }),
        );
        return (0, 0);
    }

    // Count complete roots for every still-zero cell. Each count call reserves
    // the whole current residual bank before entering setup/enumeration.
    for index in 0..estimates.len() {
        let estimate = &estimates[index];
        if !zero_non_impossible(estimate) {
            continue;
        }
        if stopped {
            break;
        }
        let grant = match reserve_all(bank) {
            Some(grant) => grant,
            None => {
                stop_reason = Some("no residual bank for root counting");
                break;
            }
        };
        root_attempts += 1;
        let cell = Cell {
            team: index / m.n,
            rank: index % m.n,
        };
        let (result, count_work) = BranchStrata::overflow_root_count(
            m,
            cell,
            derive(
                seed,
                &format!(
                    "target-overflow-root-count-{}-{}",
                    m.ids[cell.team], cell.rank
                ),
            ),
            ROOT_LIMIT,
        );
        granted = granted.saturating_add(grant);
        charged = charged.saturating_add(count_work);
        let settled = settle_stage(bank, grant, count_work);
        setup_rows.push(json!({
            "stage":"root_count", "team":m.ids[cell.team], "rank":cell.rank+1,
            "grant":grant,"actual":count_work,"within_grant":settled,
            "complete_roots":result.as_ref().ok(),"error":result.as_ref().err(),
        }));
        if !settled {
            stopped = true;
            stop_reason = Some("root-count work exceeded residual-bank grant");
            break;
        }
        if let Ok(roots) = result {
            roots_counted += 1;
            if accepted_root_count(roots, min_roots) {
                root_count_work.insert(cell.index(m.n), count_work);
                candidates.push(Candidate { cell, roots });
            }
        }
    }
    candidates.sort_by_key(|&candidate| candidate_sort_key(candidate, min_roots, m.n));
    candidates.truncate(MAX_CELLS);

    for candidate in candidates.iter().copied() {
        if stopped {
            break;
        }
        let cell = candidate.cell;
        let index = cell.index(m.n);
        let mut cell_actual_work = root_count_work.get(&index).copied().unwrap_or(0);
        if !zero_non_impossible(&estimates[index]) {
            continue;
        }

        let setup_grant = match reserve_all(bank) {
            Some(grant) => grant,
            None => {
                stop_reason = Some("no residual bank for tree setup");
                break;
            }
        };
        let admission_floor = BranchStrata::overflow_tree_setup_admission_floor(cell_actual_work);
        if !setup_grant_admitted(setup_grant, admission_floor) {
            assert!(bank.release(setup_grant));
            granted = granted.saturating_add(setup_grant);
            setup_rows.push(json!({
                "stage":"tree_setup", "team":m.ids[cell.team], "rank":cell.rank+1,
                "roots":candidate.roots,"grant":setup_grant,"actual":0,
                "within_grant":true,"admission_floor":admission_floor,
                "status":"skipped","reason":"residual setup grant below admission floor",
                "elapsed_ms":0.0,
            }));
            continue;
        }
        let setup_started = Instant::now();
        let (proposal, setup_work) = BranchStrata::with_overflow_tree(
            m,
            cell,
            derive(
                seed,
                &format!("target-overflow-tree-{}-{}", m.ids[cell.team], cell.rank),
            ),
        );
        granted = granted.saturating_add(setup_grant);
        charged = charged.saturating_add(setup_work);
        cell_actual_work = cell_actual_work.saturating_add(setup_work);
        let setup_settled = settle_stage(bank, setup_grant, setup_work);
        setup_rows.push(json!({
            "stage":"tree_setup", "team":m.ids[cell.team], "rank":cell.rank+1,
            "roots":candidate.roots,"grant":setup_grant,"actual":setup_work,
            "within_grant":setup_settled,"elapsed_ms":setup_started.elapsed().as_secs_f64()*1000.0,
            "diagnostics":proposal.as_ref().ok().map(|p| p.setup_diagnostics()),
            "error":proposal.as_ref().err(),
        }));
        if !setup_settled {
            stopped = true;
            stop_reason = Some("tree setup work exceeded residual-bank grant");
            break;
        }
        let mut p = match proposal {
            Ok(p) => p,
            Err(_) => continue,
        };
        if p.len() * draw_floor > nominal_final_draws {
            stage_rows.push(json!({"stage":"final_pair","team":m.ids[cell.team],"rank":cell.rank+1,
                "status":"skipped","reason":"leaf floor exceeds nominal final draw count","leaves":p.len(),
                "floor":draw_floor,"nominal_final_draws":nominal_final_draws}));
            continue;
        }
        let cell_seed = sampling_seed(seed, m.ids[cell.team], cell.rank);

        let pilot_ns = vec![pilot_draws; p.len()];
        let pilot_estimated = work_cost_from_estimates(&p, m, &pilot_ns, false);
        let pilot_grant = match reserve_all(bank) {
            Some(grant) if pilot_estimated <= grant => grant,
            Some(grant) => {
                assert!(bank.release(grant));
                stop_reason = Some("residual bank cannot fund complete tree pilots");
                break;
            }
            None => {
                stop_reason = Some("no residual bank for complete tree pilots");
                break;
            }
        };
        let pilot_started = Instant::now();
        let mut pilots = sample_stage(&p, m, &pilot_ns, cell_seed, "bound-pilot", workers);
        let pilot_work = stage_charge(&pilots);
        granted = granted.saturating_add(pilot_grant);
        charged = charged.saturating_add(pilot_work);
        cell_actual_work = cell_actual_work.saturating_add(pilot_work);
        let pilot_settled = settle_stage(bank, pilot_grant, pilot_work);
        stage_rows.push(json!({"stage":"bound_pilot","team":m.ids[cell.team],"rank":cell.rank+1,
            "branches":p.len(),"draws":pilot_draws*p.len(),"grant":pilot_grant,"estimated":pilot_estimated,
            "actual":pilot_work,"within_grant":pilot_settled,"elapsed_ms":pilot_started.elapsed().as_secs_f64()*1000.0,
            "hits":pilots.iter().map(|s|s.result.hits).sum::<usize>(),
            "probability":combine_branches(&pilots).probability}));
        if !pilot_settled {
            stopped = true;
            stop_reason = Some("pilot work exceeded residual-bank grant");
            break;
        }
        let defer_empty_skip = structural_discovery_enabled();
        if pilots.iter().all(|sample| sample.result.probability <= 0.0) && !defer_empty_skip {
            continue;
        }

        let message_grant = match reserve_all(bank) {
            Some(grant) => grant,
            None => {
                stop_reason = Some("no residual bank for branch messages");
                break;
            }
        };
        let pilot_results: Vec<_> = pilots.iter().map(|sample| sample.result.clone()).collect();
        let message_started = Instant::now();
        let message_info = p.prepare_messages(m, &pilot_results, messages, message_grant);
        let selected: Vec<usize> = message_info["selected"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_u64().map(|n| n as usize))
            .collect();
        let message_work = message_info["work"].as_u64().unwrap_or(0) as usize;
        let repilot_estimated: usize = selected
            .iter()
            .map(|&i| pilot_draws * p.draw_work(m, i, true))
            .sum();
        let message_preflight = message_work.saturating_add(repilot_estimated) <= message_grant;
        let repilots = if message_preflight {
            sample_selected(&p, m, &selected, pilot_draws, cell_seed, workers)
        } else {
            p.clear_messages();
            Vec::new()
        };
        let repilot_actual = repilots
            .iter()
            .map(|(_, sample)| {
                if budget::modeled() {
                    sample.result.operations.units()
                } else {
                    sample.result.samples
                }
            })
            .sum::<usize>();
        let message_actual = message_work.saturating_add(repilot_actual);
        for (branch, sample) in repilots {
            pilots[branch] = sample;
        }
        granted = granted.saturating_add(message_grant);
        charged = charged.saturating_add(message_actual);
        cell_actual_work = cell_actual_work.saturating_add(message_actual);
        let message_settled = settle_stage(bank, message_grant, message_actual);
        stage_rows.push(json!({"stage":"message_repilot","team":m.ids[cell.team],"rank":cell.rank+1,
            "selected":selected,"message_work":message_work,"grant":message_grant,"estimated_repilot":repilot_estimated,
            "preflight_fit":message_preflight,"actual":message_actual,"within_grant":message_settled,
            "elapsed_ms":message_started.elapsed().as_secs_f64()*1000.0}));
        if !message_settled {
            stopped = true;
            stop_reason = Some("message/re-pilot work exceeded residual-bank grant");
            break;
        }
        if !message_preflight {
            // Retuning was paid for, but its repilot did not fit. Clear the
            // speculative patterns and continue with the complete original
            // pilot if the residual bank still funds a final pair.
        }
        if defer_empty_skip && pilots.iter().all(|sample| sample.result.probability <= 0.0) {
            stage_rows.push(json!({"stage":"structural_discovery_empty_skip","team":m.ids[cell.team],"rank":cell.rank+1,"reason":"no event evidence after structural discovery re-pilot","messages":message_info}));
            continue;
        }

        let scores: Vec<_> = pilots
            .iter()
            .map(|sample| sample.result.std_err * (sample.result.samples as f64).sqrt())
            .collect();
        let costs: Vec<_> = pilots
            .iter()
            .map(|sample| budget::per_draw(&sample.result))
            .collect();
        let mut total = nominal_final_draws;
        let pair_bank_free = bank.limit.saturating_sub(bank.reserved);
        let (counts, pair_grant, pair_estimate) = loop {
            if total < p.len() * draw_floor {
                break (None, 0, 0);
            }
            let Some(counts) = allocate(&scores, total, draw_floor) else {
                break (None, 0, 0);
            };
            let estimate = counts
                .iter()
                .zip(&costs)
                .map(|(n, c)| n.saturating_mul(*c).saturating_mul(2))
                .sum::<usize>();
            let admission = estimate.saturating_add(
                estimate
                    .saturating_mul(FINAL_HEADROOM_PERCENT)
                    .div_ceil(100),
            );
            if admission <= pair_bank_free {
                if bank.reserve(pair_bank_free) {
                    break (Some(counts), pair_bank_free, estimate);
                }
            }
            total = total.saturating_sub(25);
        };
        let Some(counts) = counts else {
            stage_rows.push(json!({"stage":"final_pair","team":m.ids[cell.team],"rank":cell.rank+1,
                "status":"skipped","reason":"no pair allocation fits residual bank","floor":draw_floor,"bank_free":bank.limit.saturating_sub(bank.reserved)}));
            continue;
        };
        let pair_started = Instant::now();
        let main_rows = sample_stage(&p, m, &counts, cell_seed, "main", workers);
        let main = combine_branches(&main_rows);
        let main_charge = stage_charge(&main_rows);
        let check_rows = if main_charge <= pair_grant && publishable(&main, true) {
            sample_stage(&p, m, &counts, cell_seed, "check", workers)
        } else {
            Vec::new()
        };
        let check = if check_rows.is_empty() {
            Result::default()
        } else {
            combine_branches(&check_rows)
        };
        let actual = main_charge.saturating_add(stage_charge(&check_rows));
        granted = granted.saturating_add(pair_grant);
        charged = charged.saturating_add(actual);
        cell_actual_work = cell_actual_work.saturating_add(actual);
        let within = actual <= pair_grant;
        if within {
            assert!(bank.release(pair_grant - actual));
        }
        let accepted_pair = within && pair_is_publishable(&estimates[index], &main, &check);
        if accepted_pair && commit_estimate(&mut estimates[index], &main, true) {
            estimates[index].design = "matched_point_pool_target_overflow_tree".into();
            estimates[index].work_spent = estimates[index]
                .work_spent
                .saturating_add(cell_actual_work as u64);
            added += 1;
        }
        stage_rows.push(json!({"stage":"final_pair","team":m.ids[cell.team],"rank":cell.rank+1,
            "status":if accepted_pair {"accepted"} else {"declined"},"floor":draw_floor,"draws_per_branch":counts,
            "total_draws":counts.iter().sum::<usize>(),"pair_grant":pair_grant,"pair_estimate":pair_estimate,
            "main_work":main_charge,"check_work":stage_charge(&check_rows),"actual":actual,"within_grant":within,
            "main_publishable":publishable(&main,true),
            "main_probability":main.probability,"main_hits":main.hits,"main_ess":main.ess,
            "main_relative_se":if main.probability>0.{Some(main.std_err/main.probability)}else{None},
            "main_max_share":main.max_share,"main_batch_gap":main.batch_gap,
            "check_probability":check.probability,"check_hits":check.hits,"check_ess":check.ess,
            "check_relative_se":if check.probability>0.{Some(check.std_err/check.probability)}else{None},
            "main_check_ratio":if check.probability>0.{Some(main.probability/check.probability)}else{None},
            "accepted":accepted_pair,"elapsed_ms":pair_started.elapsed().as_secs_f64()*1000.0}));
        if !within {
            stopped = true;
            stop_reason = Some("final pair work exceeded residual-bank grant");
            break;
        }
    }

    let preserved = before
        .iter()
        .all(|(i, estimate)| *estimate == serde_json::to_value(&estimates[*i]).unwrap());
    emit(
        log,
        "rust_odds_target_overflow_tree_summary",
        json!({
            "enabled":true,"mode":"late_complete_target_tree","training_excluded":true,
            "target_path_limit":ROOT_LIMIT,"accepted_root_range":[min_roots,ROOT_LIMIT],"max_cells":MAX_CELLS,
            "pilot_draws_per_branch":pilot_draws,"message_count":messages,"final_draw_floor":draw_floor,
            "nominal_final_draws":nominal_final_draws,"final_headroom_percent":FINAL_HEADROOM_PERCENT,
            "reclaim_enabled":reclaim_enabled(),
            "bank_limit":bank.limit,"bank_free_before":bank_before,"bank_free_after":bank.limit.saturating_sub(bank.reserved),
            "branch_draw_credit_requested":branch_draw_credit_requested,"branch_draw_credit_computed":branch_draw_audit.credit,
            "branch_draw_audit_valid":branch_draw_audit.valid,"branch_draw_reserved_units":branch_draw_audit.reserved_draw_units,
            "branch_draw_actual_units":branch_draw_audit.actual_draw_units,
            "branch_draw_already_released_units":branch_draw_audit.already_released_draw_units,
            "bank_limit_before_credit":bank_limit_before_credit,"bank_limit_after_credit":bank_limit_after_credit,
            "root_count_attempts":root_attempts,"supported_complete_root_counts":roots_counted,
            "candidate_roots":candidates.iter().map(|c|json!({"team":m.ids[c.cell.team],"rank":c.cell.rank+1,"roots":c.roots})).collect::<Vec<_>>(),
            "setup_stages":setup_rows,"sampling_stages":stage_rows,"cumulative_stage_grants_nonadditive":granted,"actual_work":charged,
            "branch_draw_credit_applied":bank_limit_after_credit.saturating_sub(bank_limit_before_credit),
            "added":added,"stopped":stopped,"stop_reason":stop_reason,"preserved_existing_estimates":preserved,
            "elapsed_ms":started.elapsed().as_secs_f64()*1000.0
        }),
    );
    (added, charged as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_candidate_filter_excludes_positive_and_certified_impossible_cells() {
        let mut estimate = Estimate::default();
        assert!(zero_non_impossible(&estimate));
        estimate.probability = 1e-30;
        assert!(!zero_non_impossible(&estimate));
        estimate.probability = 0.0;
        estimate.reachability = "impossible:certified".into();
        assert!(!zero_non_impossible(&estimate));
    }

    #[test]
    fn overflow_pair_requires_zero_cell_and_normal_independent_gate() {
        let main = Result {
            weighted: true,
            probability: 1e-25,
            std_err: 2e-26,
            hits: 100,
            ess: 20.,
            max_share: 0.1,
            batch_gap: 0.2,
            ..Result::default()
        };
        let check = main.clone();
        let mut estimate = Estimate::default();
        assert!(pair_is_publishable(&estimate, &main, &check));
        estimate.probability = 1e-24;
        assert!(!pair_is_publishable(&estimate, &main, &check));
        estimate.probability = 0.0;
        estimate.reachability = "impossible:certified".into();
        assert!(!pair_is_publishable(&estimate, &main, &check));
    }

    #[test]
    fn overflow_allocation_preserves_explicit_ten_draw_floor_and_ninety_ten_split() {
        let counts = allocate(&[9.0, 1.0], 100, 10).unwrap();
        assert_eq!(counts.iter().sum::<usize>(), 100);
        assert_eq!(counts, vec![79, 21]);
        assert!(counts.iter().all(|&n| n >= 10));
        assert!(counts[0] > counts[1]);
        assert!(allocate(&[1.0; 601], 6_000, 10).is_none());
    }

    #[test]
    fn overflow_allocation_configurable_floor_conserves_total_and_keeps_each_leaf_positive() {
        let scores = [7.0, 2.0, 1.0];
        let counts = allocate(&scores, 37, 2).unwrap();
        assert_eq!(counts.iter().sum::<usize>(), 37);
        assert!(counts.iter().all(|&count| count >= 2));
        let pure_default = bounded_setting_value(None, DEFAULT_FLOOR, 2, DEFAULT_FLOOR);
        let same_default = allocate(&scores, 37, pure_default).unwrap();
        assert_eq!(same_default, allocate(&scores, 37, DEFAULT_FLOOR).unwrap());
        assert!(allocate(&[1.0; 601], 6_000, 2).is_some());
        assert!(allocate(&scores, 37, 1).is_none());
    }

    #[test]
    fn overflow_draw_floor_setting_defaults_to_two_and_is_bounded_to_ten() {
        assert_eq!(bounded_setting_value(None, DEFAULT_FLOOR, 2, MAX_FLOOR), 2);
        assert_eq!(
            bounded_setting_value(Some("1"), DEFAULT_FLOOR, 2, MAX_FLOOR),
            2
        );
        assert_eq!(
            bounded_setting_value(Some("7"), DEFAULT_FLOOR, 2, MAX_FLOOR),
            7
        );
        assert_eq!(
            bounded_setting_value(Some("99"), DEFAULT_FLOOR, 2, MAX_FLOOR),
            10
        );
        assert_eq!(
            bounded_setting_value(Some("bad"), DEFAULT_FLOOR, 2, MAX_FLOOR),
            2
        );
    }

    #[test]
    fn overflow_diagnostic_settings_are_bounded_and_reclaim_defaults_off() {
        assert_eq!(bounded_setting_value(None, 5, 2, 50), 5);
        assert_eq!(bounded_setting_value(Some("1"), 5, 2, 50), 2);
        assert_eq!(bounded_setting_value(Some("500"), 5, 2, 50), 50);
        assert_eq!(bounded_setting_value(Some("invalid"), 5, 2, 50), 5);
        assert_eq!(bounded_setting_value(Some("0"), 3, 0, 3), 0);
        assert_eq!(bounded_setting_value(None, 6_000, 3_000, 6_000), 6_000);
        assert_eq!(
            bounded_setting_value(Some("4000"), 6_000, 3_000, 6_000),
            4_000
        );
        assert_eq!(
            bounded_setting_value(Some("6001"), 6_000, 3_000, 6_000),
            6_000
        );
        assert!(!reclaim_value(None));
        assert!(!reclaim_value(Some("0")));
        assert!(reclaim_value(Some("1")));
    }

    #[test]
    fn target_tree_min_roots_parser_defaults_clamps_and_rejects_invalid_values() {
        assert_eq!(min_roots_value(None), 1);
        assert_eq!(min_roots_value(Some("1")), 1);
        assert_eq!(min_roots_value(Some("56")), 56);
        assert_eq!(min_roots_value(Some("0")), 1);
        assert_eq!(min_roots_value(Some("65")), 65);
        assert_eq!(min_roots_value(Some("100")), 65);
        assert_eq!(min_roots_value(Some("invalid")), 1);
        assert_eq!(min_roots_value(Some("-1")), 1);
    }

    #[test]
    fn target_tree_root_candidate_eligibility_uses_configured_lower_bound() {
        assert!(!accepted_root_count(56, 65));
        assert!(accepted_root_count(56, 1));
        assert!(!accepted_root_count(0, 1));
        assert!(accepted_root_count(1, 1));
        assert!(accepted_root_count(ROOT_LIMIT, 1));
        assert!(!accepted_root_count(ROOT_LIMIT + 1, 1));
    }

    #[test]
    fn lowered_minimum_keeps_legacy_roots_ahead_and_preserves_legacy_order() {
        let mut candidates = vec![
            Candidate {
                cell: Cell { team: 0, rank: 4 },
                roots: 186,
            },
            Candidate {
                cell: Cell { team: 0, rank: 1 },
                roots: 11,
            },
            Candidate {
                cell: Cell { team: 0, rank: 3 },
                roots: 100,
            },
            Candidate {
                cell: Cell { team: 0, rank: 2 },
                roots: 186,
            },
            Candidate {
                cell: Cell { team: 0, rank: 0 },
                roots: 56,
            },
        ];

        candidates.sort_by_key(|&candidate| candidate_sort_key(candidate, 1, 5));
        let ordered: Vec<_> = candidates
            .iter()
            .map(|candidate| (candidate.roots, candidate.cell.index(5)))
            .collect();
        assert_eq!(
            ordered,
            vec![(100, 3), (186, 2), (186, 4), (11, 1), (56, 0)]
        );

        let mut default_candidates = candidates.clone();
        default_candidates
            .sort_by_key(|&candidate| candidate_sort_key(candidate, DEFAULT_MIN_ROOTS, 5));
        let default_order: Vec<_> = default_candidates
            .iter()
            .map(|candidate| (candidate.roots, candidate.cell.index(5)))
            .collect();
        assert_eq!(
            default_order,
            vec![(100, 3), (186, 2), (186, 4), (11, 1), (56, 0)]
        );

        let mut legacy_candidates = candidates.clone();
        legacy_candidates.sort_by_key(|&candidate| candidate_sort_key(candidate, 65, 5));
        let legacy_order: Vec<_> = legacy_candidates
            .iter()
            .map(|candidate| (candidate.roots, candidate.cell.index(5)))
            .collect();
        assert_eq!(
            legacy_order,
            vec![(11, 1), (56, 0), (100, 3), (186, 2), (186, 4)]
        );
    }

    #[test]
    fn overflow_stream_namespace_is_per_cell_and_phase_independent() {
        let seed = 17;
        let cell_seed = sampling_seed(seed, 16, 13);
        let other_cell = sampling_seed(seed, 16, 14);
        assert_ne!(cell_seed, other_cell);
        let old_early = derive(seed, "branch-bound-pilot-0");
        let overflow_pilot = derive(cell_seed, "branch-bound-pilot-0");
        let overflow_message = derive(cell_seed, "message-pilot-0");
        let overflow_main = derive(cell_seed, "branch-main-0");
        let overflow_check = derive(cell_seed, "branch-check-0");
        assert_ne!(old_early, overflow_pilot);
        assert_ne!(overflow_pilot, overflow_message);
        assert_ne!(overflow_pilot, overflow_main);
        assert_ne!(overflow_main, overflow_check);
        assert_ne!(overflow_pilot, derive(other_cell, "branch-bound-pilot-0"));
    }

    #[test]
    fn stage_settlement_releases_unused_but_never_hides_overrun() {
        let mut bank = WorkBudget::new(100);
        assert!(bank.reserve(100));
        assert!(settle_stage(&mut bank, 100, 75));
        assert_eq!(bank.reserved, 75);
        assert!(!settle_stage(&mut bank, 75, 76));
        assert_eq!(bank.reserved, 75);
    }

    #[test]
    fn setup_grant_admission_includes_boundary_and_rejects_shortfall() {
        assert!(!setup_grant_admitted(99, 100));
        assert!(setup_grant_admitted(100, 100));
        assert!(setup_grant_admitted(101, 100));
    }

    #[test]
    fn draw_credit_requires_valid_audit_transfer_and_original_capacity() {
        let audit = super::super::branches::DrawAudit {
            credit: 1_000,
            valid: true,
            ..Default::default()
        };
        assert_eq!(requested_branch_credit(audit, 500), 500);
        assert_eq!(applied_branch_credit(audit, 500, 900, 1_100, true), 200);
        assert_eq!(applied_branch_credit(audit, 500, 900, 1_100, false), 0);
        assert_eq!(applied_branch_credit(audit, 500, 1_100, 1_100, true), 0);
        let invalid = super::super::branches::DrawAudit {
            valid: false,
            ..audit
        };
        assert_eq!(applied_branch_credit(invalid, 500, 0, 1_100, true), 0);
    }
}
