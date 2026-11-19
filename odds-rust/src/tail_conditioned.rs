//! Pilot-sized native conditional proposals for persistent zero cells.

use crate::{
    conditioned::Result,
    joint_caps::propagated::lazy::LazyJoint,
    logging::RequestLog,
    model::Model,
    pool::Estimate,
    rng::derive,
    search::{self, Cell},
};
use serde_json::json;

const PILOT_DRAWS: usize = 4096;
const TARGET_ESS: f64 = 128.0;
const MIN_DRAWS: usize = 16_384;
const MAX_DRAWS: usize = 524_288;

struct ProposalRun {
    pilot: Result,
    final_result: Result,
    gate: serde_json::Value,
}

struct CellOutcome {
    cell: Cell,
    main: ProposalRun,
    check: ProposalRun,
    accepted: bool,
}

fn adaptive_draws(pilot_samples: usize, pilot_ess: f64) -> usize {
    let effective_rate = if pilot_samples > 0 && pilot_ess.is_finite() && pilot_ess > 0.0 {
        pilot_ess / pilot_samples as f64
    } else {
        0.0
    };
    if effective_rate <= 0.0 || !effective_rate.is_finite() {
        return MAX_DRAWS;
    }
    let needed = (TARGET_ESS / effective_rate).ceil();
    if !needed.is_finite() || needed >= MAX_DRAWS as f64 {
        return MAX_DRAWS;
    }
    (needed as usize)
        .max(1)
        .next_power_of_two()
        .clamp(MIN_DRAWS, MAX_DRAWS)
}

fn run_proposal(
    model: &Model,
    cell: Cell,
    setup_seed: i64,
    pilot_seed: i64,
    final_seed: i64,
) -> ProposalRun {
    let allowance = LazyJoint::order_constructor_allowance(model, cell);
    let (plan, _setup_charge, gate) = LazyJoint::new_order_base(model, cell, setup_seed, allowance);
    let Some(plan) = plan else {
        return ProposalRun {
            pilot: Result::default(),
            final_result: Result::default(),
            gate,
        };
    };
    let pilot = plan.sample_order_pilot(model, PILOT_DRAWS, pilot_seed, usize::MAX);
    let draws = adaptive_draws(pilot.samples, pilot.ess);
    let final_result = plan.sample(model, draws, final_seed);
    ProposalRun {
        pilot,
        final_result,
        gate,
    }
}

fn relative_se(result: &Result) -> Option<f64> {
    (result.probability > 0.0).then_some(result.std_err / result.probability)
}

fn nominal_work(run: &ProposalRun) -> u64 {
    run.pilot.work.saturating_add(run.final_result.work)
}

fn proposal_diagnostic(run: &ProposalRun) -> serde_json::Value {
    json!({
        "gate": run.gate,
        "pilot_samples": run.pilot.samples,
        "pilot_hits": run.pilot.hits,
        "pilot_ess": run.pilot.ess,
        "pilot_operations": run.pilot.operations.units(),
        "final_samples": run.final_result.samples,
        "final_hits": run.final_result.hits,
        "probability": run.final_result.probability,
        "ess": run.final_result.ess,
        "relative_se": relative_se(&run.final_result),
        "max_share": run.final_result.max_share,
        "batch_gap": run.final_result.batch_gap,
        "final_operations": run.final_result.operations.units(),
        "nominal_result_work": nominal_work(run)
    })
}

/// Run independently trained main/check proposals for every unresolved cell.
pub(crate) fn run(
    model: &Model,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    log: Option<&RequestLog>,
) -> (usize, u64) {
    if model.n == 0 || estimates.len() != model.n.saturating_mul(model.n) {
        return (0, 0);
    }
    let cells: Vec<_> = estimates
        .iter()
        .enumerate()
        .filter_map(|(index, estimate)| {
            (index / model.n < model.request.team_groups.len()
                && estimate.probability == 0.0
                && !estimate.reachability.starts_with("impossible"))
            .then_some(Cell {
                team: index / model.n,
                rank: index % model.n,
            })
        })
        .collect();
    if cells.is_empty() {
        return (0, 0);
    }
    let rough = crate::rare_tail::value("RUST_ODDS_RARE_TAIL_QUALITY") == "order";
    // Main and check proposals have independent seeds and no data dependency.
    // Schedule them separately so a worker can start another proposal while a
    // slow proposal's paired run is still in flight.
    let proposal_runs = search::parallel(cells.len().saturating_mul(2), workers, |job| {
        let index = job / 2;
        let main_proposal = job % 2 == 0;
        let cell = cells[index];
        let base_seed = derive(
            seed,
            &format!("conditioned-tail-cell-{}-{}", cell.team, cell.rank),
        );
        let stream = if main_proposal { "main" } else { "check" };
        run_proposal(
            model,
            cell,
            derive(base_seed, &format!("{stream}-setup")),
            derive(base_seed, &format!("{stream}-pilot")),
            derive(base_seed, &format!("{stream}-final")),
        )
    });
    let mut proposal_runs = proposal_runs.into_iter();
    let outcomes = cells
        .into_iter()
        .map(|cell| {
            let main = proposal_runs.next().unwrap();
            let check = proposal_runs.next().unwrap();
            let accepted =
                crate::rare_tail::accepted(&main.final_result, &check.final_result, rough);
            CellOutcome {
                cell,
                main,
                check,
                accepted,
            }
        })
        .collect::<Vec<_>>();

    let mut accepted_count = 0;
    let mut public_work = 0u64;
    for outcome in outcomes {
        public_work = public_work
            .saturating_add(nominal_work(&outcome.main))
            .saturating_add(nominal_work(&outcome.check));
        if outcome.accepted {
            let index = outcome.cell.index(model.n);
            search::apply(
                &mut estimates[index],
                &outcome.main.final_result,
                "pilot_sized_lazy_conditioned",
            );
            accepted_count += 1;
        }
        if let Some(log) = log {
            log.event(
                "rust_odds_tail_conditioned_cell",
                json!({
                    "group": model.request.id,
                    "team": model.ids[outcome.cell.team],
                    "rank": outcome.cell.rank + 1,
                    "quality": if rough {"order"} else {"current"},
                    "main": proposal_diagnostic(&outcome.main),
                    "check": proposal_diagnostic(&outcome.check),
                    "accepted": outcome.accepted,
                    "work_spent": nominal_work(&outcome.main).saturating_add(nominal_work(&outcome.check))
                }),
            );
        }
    }
    (accepted_count, public_work)
}

#[cfg(test)]
mod tests {
    use super::{adaptive_draws, MAX_DRAWS, MIN_DRAWS, PILOT_DRAWS};
    use crate::{
        model::{Model, Request},
        pool::Estimate,
    };
    use serde_json::json;

    fn one_fixture_large_model() -> Model {
        let teams: Vec<_> = (0..33)
            .map(|team| {
                json!({
                    "team_id": team,
                    "add_sub": if team == 1 { 1 } else if team == 0 { 0 } else { -100 }
                })
            })
            .collect();
        let request: Request = serde_json::from_value(json!({
            "id": 9801,
            "phase": {"sort":"pt", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups": teams,
            "games": [{"id":1,"home_id":0,"away_id":1,"home_power":1.0,"away_power":0.8}]
        }))
        .unwrap();
        Model::new(request).unwrap()
    }

    #[test]
    fn adaptive_draws_rounds_and_respects_bounds() {
        assert_eq!(adaptive_draws(PILOT_DRAWS, 0.0), MAX_DRAWS);
        assert_eq!(adaptive_draws(PILOT_DRAWS, f64::NAN), MAX_DRAWS);
        assert_eq!(adaptive_draws(PILOT_DRAWS, 32.0), MIN_DRAWS);
        assert_eq!(adaptive_draws(PILOT_DRAWS, 16.0), 32_768);
        assert_eq!(adaptive_draws(PILOT_DRAWS, 12.0), 65_536);
        assert_eq!(adaptive_draws(PILOT_DRAWS, 8.0), 65_536);
        assert_eq!(adaptive_draws(PILOT_DRAWS, 4.0), 131_072);
        assert_eq!(adaptive_draws(PILOT_DRAWS, 1.0), MAX_DRAWS);
    }

    #[test]
    fn conditioned_recovery_matches_exact_top_rank_and_preserves_other_cells() {
        let model = one_fixture_large_model();
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        for estimate in &mut estimates {
            estimate.probability = 0.1;
        }
        let target = 0;
        let rank = 0;
        let target_index = target * model.n + rank;
        estimates[target_index].probability = 0.0;
        estimates[target_index].reachability = "undecided".into();
        let positive_index = 1;
        let positive_before = estimates[positive_index].probability;
        let certified_index = model.n * model.n - 1;
        estimates[certified_index].probability = 0.0;
        estimates[certified_index].reachability = "impossible:test".into();

        let expected = model.fixtures[0].prob[2];
        let (accepted, _) = super::run(&model, 90421, 1, &mut estimates, None);

        assert_eq!(accepted, 1);
        assert!(estimates[target_index].probability > 0.0);
        assert!(
            (estimates[target_index].probability - expected).abs()
                <= 6.0 * estimates[target_index].std_err + 0.02,
            "estimated {} vs exact {}, se {}",
            estimates[target_index].probability,
            expected,
            estimates[target_index].std_err
        );
        assert_eq!(estimates[positive_index].probability, positive_before);
        assert_eq!(estimates[certified_index].probability, 0.0);
        assert_eq!(estimates[certified_index].reachability, "impossible:test");
    }
}
