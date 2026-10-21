use super::{budget, emit, parallel, Plan};
use crate::{
    conditioned::Result, joint_caps::propagated::lazy::LazyJoint, model::Model, search::Cell,
};
use serde_json::json;
use std::time::Instant;

pub(super) type Trained = (Cell, Plan, Result, f64, u64, f64);

/// Train the initial candidates for large leagues under a fixed, deterministic
/// work budget. Every job receives its quota before any constructor runs.
pub(super) fn train(
    m: &Model,
    cells: &[Cell],
    training_limit: usize,
    workers: usize,
    seed: i64,
    capacity: usize,
    log: Option<&crate::logging::RequestLog>,
) -> (Vec<Trained>, usize, bool) {
    let count = cells.len().min(training_limit);
    let quotient = if count == 0 { 0 } else { capacity / count };
    let remainder = if count == 0 { 0 } else { capacity % count };
    let outcomes = parallel(count, workers, |index| {
        let cell = cells[index];
        let grant = quotient + usize::from(index < remainder);
        let started = Instant::now();
        let setup_seed = super::derive(
            seed,
            &format!("rare-tail-setup-{}-{}", m.ids[cell.team], cell.rank),
        );
        let (proposal, setup_work, diagnostics) =
            LazyJoint::new_order_base(m, cell, setup_seed, grant);
        let mut pilot_work = 0usize;
        let mut legacy_pilot_work = 0u64;
        let mut pilot_ms = 0.0;
        let mut candidate = None;
        let mut job_overrun = setup_work > grant;
        if let Some(proposal) = proposal {
            let remaining = grant.saturating_sub(setup_work);
            if remaining > 0 {
                let pilot_started = Instant::now();
                let result = proposal.sample_order_pilot(
                    m,
                    1000,
                    super::derive(
                        super::derive(
                            seed,
                            &format!("rare-tail-pilot-{}-{}", m.ids[cell.team], cell.rank),
                        ),
                        "lazy",
                    ),
                    remaining,
                );
                pilot_ms = pilot_started.elapsed().as_secs_f64() * 1000.0;
                pilot_work = result.operations.units();
                legacy_pilot_work = result.work;
                if pilot_work > remaining {
                    job_overrun = true;
                } else if result.samples > 0 {
                    candidate = Some((Plan::Lazy(proposal), result));
                }
            }
        }
        let charged = setup_work.saturating_add(pilot_work);
        if charged > grant {
            job_overrun = true;
        }
        (
            cell,
            grant,
            setup_work,
            pilot_work,
            legacy_pilot_work,
            pilot_ms,
            started.elapsed().as_secs_f64() * 1000.0,
            charged,
            job_overrun,
            diagnostics,
            candidate,
        )
    });
    let mut charged_total = 0usize;
    let mut completed = Vec::new();
    let mut overrun = false;
    let mut per_job = Vec::with_capacity(count);

    for (
        cell,
        grant,
        setup_work,
        pilot_work,
        legacy_pilot_work,
        pilot_ms,
        elapsed,
        charged,
        job_overrun,
        diagnostics,
        candidate,
    ) in outcomes
    {
        overrun |= job_overrun;
        charged_total = charged_total.saturating_add(charged);
        per_job.push(json!({
            "team":m.ids[cell.team], "rank":cell.rank+1, "grant":grant,
            "setup_work":setup_work, "pilot_work":pilot_work,
            "status":if candidate.is_some(){"completed"}else{"declined"},
            "constructor":diagnostics
        }));
        emit(
            log,
            "rust_odds_rare_tail_training",
            json!({"group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,
            "training_ms":elapsed,"pilot_work":legacy_pilot_work,
            "training_work_units":charged,"pilot":candidate.as_ref().map(|(p,r)|json!({
                "hits":r.hits,"ess":r.ess,"sampling_ms":pilot_ms,
                "setup_work":p.setup_work(),"cost_per_draw":budget::per_draw(r),
                "operations":{"fixtures":r.operations.fixtures,"guidance":r.operations.guidance,"ranking":r.operations.ranking},
                "plan":p.describe(m)
            }))}),
        );
        if let Some((plan, result)) = candidate {
            completed.push((cell, plan, result, elapsed, legacy_pilot_work, pilot_ms));
        }
    }
    emit(
        log,
        "rust_odds_large_training_summary",
        json!({"group":m.request.id,"capacity":capacity,"granted":capacity.min(quotient.saturating_mul(count).saturating_add(remainder)),
            "actual_charged":charged_total,"completed":completed.len(),"declined":count.saturating_sub(completed.len()),
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
