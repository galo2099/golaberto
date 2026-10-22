use super::{budget, emit, parallel, Plan};
use crate::{
    conditioned::Result, joint_caps::propagated::lazy::LazyJoint, model::Model, search::Cell,
};
use serde_json::json;
use std::time::Instant;

const PILOT_DRAWS: usize = 256;
const MIN_PILOT_DRAWS: usize = 32;
const MAX_CELLS: usize = 128;
const TRAINING_SHARE_NUMERATOR: usize = 2;
const TRAINING_SHARE_DENOMINATOR: usize = 3;

fn pilot_wave(
    costs: &[(usize, usize)],
    remaining: usize,
) -> (Vec<(usize, usize, usize)>, Vec<usize>) {
    let mut reserved = 0usize;
    let mut wave = Vec::new();
    let mut deferred = Vec::new();
    for (index, &(fixed, upper)) in costs.iter().enumerate() {
        let upper = upper.max(1);
        let available = remaining.saturating_sub(reserved);
        let draws = (available.saturating_sub(fixed) / upper).min(PILOT_DRAWS);
        if draws < MIN_PILOT_DRAWS {
            deferred.push(index);
            continue;
        }
        let grant = fixed.saturating_add(draws.saturating_mul(upper));
        reserved = reserved.saturating_add(grant);
        wave.push((index, draws, grant));
    }
    (wave, deferred)
}

pub(super) type Trained = (Cell, Plan, Result, f64, u64, f64);

/// Train large-league candidates under a deterministic, cost-based work cap.
/// Each phase reserves conservative quotes and returns unused grants at its
/// barrier so later candidates can use them.
pub(super) fn train(
    m: &Model,
    cells: &[Cell],
    training_limit: usize,
    workers: usize,
    seed: i64,
    capacity: usize,
    log: Option<&crate::logging::RequestLog>,
) -> (Vec<Trained>, usize, bool) {
    let count = cells.len().min(training_limit).min(MAX_CELLS);
    let training_capacity =
        capacity.saturating_mul(TRAINING_SHARE_NUMERATOR) / TRAINING_SHARE_DENOMINATOR;
    let mut cells: Vec<_> = cells.iter().copied().take(count).collect();
    cells.sort_by_key(|cell| {
        (
            LazyJoint::order_constructor_allowance(m, *cell),
            m.ids[cell.team],
            cell.rank,
        )
    });

    // Setup quotes are available before construction. Run cheapest constructors
    // first and release any difference between quote and actual charge.
    // Keep a third of the training pool for pilot work before constructors run.
    let pilot_reserve = training_capacity / 3;
    let mut remaining = training_capacity;
    let mut setup_remaining = training_capacity.saturating_sub(pilot_reserve);
    let mut setup_charged = 0usize;
    let mut setup_granted = 0usize;
    let mut proposals = Vec::new();
    let mut per_job = Vec::with_capacity(count);
    for cell in cells {
        let grant = LazyJoint::order_constructor_allowance(m, cell);
        if grant > setup_remaining {
            per_job.push(json!({"team":m.ids[cell.team],"rank":cell.rank+1,"setup_quote":grant,"grant":0,"status":"setup_budget"}));
            continue;
        }
        setup_granted = setup_granted.saturating_add(grant);
        let started = Instant::now();
        let setup_seed = super::derive(
            seed,
            &format!("rare-tail-setup-{}-{}", m.ids[cell.team], cell.rank),
        );
        let (proposal, setup_work, diagnostics) =
            LazyJoint::new_order_base(m, cell, setup_seed, grant);
        if setup_work > grant {
            per_job.push(json!({"team":m.ids[cell.team],"rank":cell.rank+1,"grant":grant,"setup_work":setup_work,"status":"overrun","constructor":diagnostics}));
            return (Vec::new(), setup_charged.saturating_add(setup_work), false);
        }
        setup_remaining = setup_remaining.saturating_sub(setup_work);
        remaining = remaining.saturating_sub(setup_work);
        setup_charged = setup_charged.saturating_add(setup_work);
        if let Some(proposal) = proposal {
            let draw_upper = proposal.sample_work_upper_bound(m, 1).max(1);
            proposals.push((
                cell,
                proposal,
                grant,
                setup_work,
                draw_upper,
                started.elapsed().as_secs_f64() * 1000.0,
                diagnostics,
            ));
        } else {
            per_job.push(json!({"team":m.ids[cell.team],"rank":cell.rank+1,"grant":grant,"setup_work":setup_work,"status":"declined","constructor":diagnostics}));
        }
    }

    // Pilot costs vary by proposal. Build deterministic waves by minimum useful
    // quote; each wave is parallel, then its unused reservation is recycled.
    proposals.sort_by_key(|(cell, _, _, _, upper, _, _)| {
        (
            upper.saturating_mul(MIN_PILOT_DRAWS),
            m.ids[cell.team],
            cell.rank,
        )
    });
    let mut pilot_charged = 0usize;
    let mut pilot_granted = 0usize;
    let mut completed = Vec::new();
    let mut overrun = false;
    let mut pending = proposals;
    while !pending.is_empty() && remaining > 0 {
        let costs: Vec<_> = pending.iter().map(|item| (0, item.4)).collect();
        let (selected, deferred_indices) = pilot_wave(&costs, remaining);
        let mut reservations = vec![None; pending.len()];
        for (index, draws, grant) in selected {
            reservations[index] = Some((draws, grant));
        }
        let mut wave = Vec::new();
        let mut deferred = Vec::new();
        for (index, item) in pending.into_iter().enumerate() {
            if let Some((draws, grant)) = reservations[index] {
                wave.push((item, draws, grant));
            } else if deferred_indices.contains(&index) {
                deferred.push(item);
            }
        }
        if wave.is_empty() {
            pending = deferred;
            break;
        }
        let reserved: usize = wave.iter().map(|(_, _, grant)| *grant).sum();
        pilot_granted = pilot_granted.saturating_add(reserved);
        remaining = remaining.saturating_sub(reserved);
        let outcomes = parallel(wave.len(), workers, |i| {
            let (
                (cell, proposal, setup_grant, setup_work, upper, setup_ms, diagnostics),
                draws,
                grant,
            ) = &wave[i];
            let started = Instant::now();
            let result = proposal.sample_order_pilot(
                m,
                *draws,
                super::derive(
                    super::derive(
                        seed,
                        &format!("rare-tail-pilot-{}-{}", m.ids[cell.team], cell.rank),
                    ),
                    "lazy",
                ),
                *grant,
            );
            let work = result.operations.units();
            let overrun = work > *grant;
            let pilot_ms = started.elapsed().as_secs_f64() * 1000.0;
            let _ = (cell, setup_grant, setup_work, upper, setup_ms, diagnostics);
            (result, work, overrun, pilot_ms)
        });
        for ((item, _draws, grant), (result, work, job_overrun, pilot_ms)) in
            wave.into_iter().zip(outcomes)
        {
            let (cell, proposal, setup_grant, setup_work, upper, setup_ms, diagnostics) = item;
            let legacy_work = result.work;
            let elapsed = setup_ms + pilot_ms;
            let candidate = (!job_overrun && result.samples >= MIN_PILOT_DRAWS)
                .then(|| (Plan::Lazy(proposal), result));
            overrun |= job_overrun;
            pilot_charged = pilot_charged.saturating_add(work);
            remaining = remaining.saturating_add(grant.saturating_sub(work));
            let setup_refund = setup_grant.saturating_sub(setup_work);
            let pilot_refund = grant.saturating_sub(work);
            let total_grant = setup_grant.saturating_add(grant);
            let total_work = setup_work.saturating_add(work);
            per_job.push(json!({"team":m.ids[cell.team],"rank":cell.rank+1,"grant":total_grant,"setup_grant":setup_grant,"pilot_grant":grant,"setup_work":setup_work,"pilot_work":work,"charged_work":total_work,"setup_refund":setup_refund,"pilot_refund":pilot_refund,"draw_upper_work":upper,"status":if candidate.is_some(){"completed"}else{"declined"},"constructor":diagnostics}));
            emit(
                log,
                "rust_odds_rare_tail_training",
                json!({"group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,"training_ms":elapsed,"pilot_work":legacy_work,"grant":total_grant,"setup_grant":setup_grant,"pilot_grant":grant,"setup_work":setup_work,"pilot_work_units":work,"setup_refund":setup_refund,"pilot_refund":pilot_refund,"training_work_units":total_work,"pilot":candidate.as_ref().map(|(p,r)|json!({"hits":r.hits,"ess":r.ess,"setup_work":p.setup_work(),"cost_per_draw":budget::per_draw(r),"operations":{"fixtures":r.operations.fixtures,"guidance":r.operations.guidance,"ranking":r.operations.ranking},"plan":p.describe(m)}))}),
            );
            if let Some((plan, result)) = candidate {
                completed.push((cell, plan, result, elapsed, legacy_work, pilot_ms));
            }
        }
        pending = deferred;
    }
    for (cell, _, setup_grant, setup_work, upper, _, diagnostics) in pending {
        per_job.push(json!({"team":m.ids[cell.team],"rank":cell.rank+1,"grant":setup_grant,"setup_grant":setup_grant,"pilot_grant":0,"setup_work":setup_work,"pilot_work":0,"setup_refund":setup_grant.saturating_sub(setup_work),"pilot_refund":0,"draw_upper_work":upper,"status":"pilot_budget","constructor":diagnostics}));
    }
    let charged_total = setup_charged.saturating_add(pilot_charged);
    if charged_total > training_capacity {
        overrun = true;
    }
    emit(
        log,
        "rust_odds_large_training_summary",
        json!({"group":m.request.id,"capacity":training_capacity,"stage_capacity":capacity,"setup_charged":setup_charged,"pilot_charged":pilot_charged,"cumulative_quoted_work":setup_granted.saturating_add(pilot_granted),"recycled_grants":true,
            "actual_charged":charged_total,"completed":completed.len(),"declined":count.saturating_sub(completed.len()),
            "training_share":"2/3","pilot_draws":PILOT_DRAWS,"minimum_pilot_draws":MIN_PILOT_DRAWS,
            "worker_independent_admission":true,"overrun":overrun,"jobs":per_job}),
    );
    (completed, charged_total, !overrun)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Request;
    use serde_json::json;

    fn large_model() -> Model {
        let request: Request = serde_json::from_value(json!({
            "id": 81,
            "phase": {"sort":"pt", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups": (0..36).map(|team| json!({"team_id":team,"add_sub":0})).collect::<Vec<_>>(),
            "games": [{"id":1,"home_id":0,"away_id":1,"home_power":1.1,"away_power":0.9}]
        })).unwrap();
        Model::new(request).unwrap()
    }

    #[test]
    fn cost_admission_skips_expensive_candidate_and_admits_later_cheap_one() {
        let (wave, deferred) = pilot_wave(&[(0, 100), (0, 2)], 64);
        assert_eq!(wave, vec![(1, 32, 64)]);
        assert_eq!(deferred, vec![0]);
    }

    #[test]
    fn cost_admission_can_cover_more_than_old_cell_limit() {
        let costs = vec![(0, 10); 40];
        let (wave, deferred) = pilot_wave(&costs, 40 * PILOT_DRAWS * 10);
        assert_eq!(wave.len(), 40);
        assert!(deferred.is_empty());
        assert!(wave
            .iter()
            .all(|(_, draws, grant)| { *draws == PILOT_DRAWS && *grant == PILOT_DRAWS * 10 }));
    }

    #[test]
    fn unused_grants_return_at_barrier_and_tiny_budget_declines_cleanly() {
        let (wave, deferred) = pilot_wave(&[(0, 1), (0, 2)], 32 + 64);
        assert_eq!(wave, vec![(0, 96, 96)]);
        assert_eq!(deferred, vec![1]);
        let refunded = 96 - 32;
        let (next_wave, next_deferred) = pilot_wave(&[(0, 2)], refunded);
        assert_eq!(next_wave, vec![(0, 32, 64)]);
        assert!(next_deferred.is_empty());
        let (empty, deferred) = pilot_wave(&[(0, 1), (0, 10)], 31);
        assert!(empty.is_empty());
        assert_eq!(deferred, vec![0, 1]);
    }

    #[test]
    fn small_constructor_grant_is_declined_and_fully_accounted() {
        let model = large_model();
        let cell = Cell { team: 0, rank: 0 };
        let (_, charged, report) = LazyJoint::new_order_base(&model, cell, 808, 0);
        assert_eq!(charged, 0);
        assert_eq!(report["status"], "declined");
        assert_eq!(report["reason"], "constructor_budget");
    }

    #[test]
    fn bounded_pilot_is_worker_independent_and_charged_within_grant() {
        let model = large_model();
        let cells = [Cell { team: 0, rank: 0 }, Cell { team: 1, rank: 0 }];
        let capacity = 40_000_000;
        let (one_worker, one_charge, one_valid) =
            train(&model, &cells, cells.len(), 1, 808, capacity, None);
        let (four_workers, four_charge, four_valid) =
            train(&model, &cells, cells.len(), 4, 808, capacity, None);
        assert!(one_valid && four_valid);
        assert!(one_charge <= capacity);
        assert!(four_charge <= capacity);
        assert_eq!(one_charge, four_charge);
        assert!(!one_worker.is_empty(), "finite grants should admit a pilot");
        assert_eq!(one_worker.len(), four_workers.len());
        for (left, right) in one_worker.iter().zip(&four_workers) {
            assert_eq!(left.0, right.0);
            assert_eq!(left.2.samples, right.2.samples);
            assert_eq!(left.2.hits, right.2.hits);
            assert_eq!(left.2.operations.units(), right.2.operations.units());
        }
    }
}
