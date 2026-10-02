//! Offline deterministic branch-bound audit, no estimator or server defaults.
use golaberto_odds::{
    joint_caps::propagated::lazy::BranchStrata,
    model::{Model, Request},
    search::Cell,
};
use serde_json::json;
use std::{env, fs, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 4 {
        return Err(
            "REQUEST TEAM RANK [REFERENCE_PROBABILITY] [RELATIVE_TOLERANCE] [plain|strong]".into(),
        );
    }
    let start = Instant::now();
    let m = Model::new(serde_json::from_slice::<Request>(&fs::read(&args[1])?)?)?;
    let cell = Cell {
        team: m.indices[&args[2].parse()?],
        rank: args[3].parse::<usize>()? - 1,
    };
    let reference = args.get(4).map_or(Ok(4e-34), |s| s.parse::<f64>())?;
    let tolerance = args.get(5).map_or(Ok(0.01), |s| s.parse::<f64>())?;
    if reference <= 0. || !reference.is_finite() || tolerance < 0. || !tolerance.is_finite() {
        return Err("reference must be finite/positive and tolerance finite/nonnegative".into());
    }
    let model_ms = start.elapsed().as_secs_f64() * 1000.;
    let setup = Instant::now();
    let p = BranchStrata::with_secondary(&m, cell, 808, 4, true)?;
    let setup_ms = setup.elapsed().as_secs_f64() * 1000.;
    let clock = Instant::now();
    let bounds = p.bounds(&m, args.get(6).is_some_and(|s| s == "strong"));
    let bounds_ms = clock.elapsed().as_secs_f64() * 1000.;
    let description = p.describe(&m);
    let mut order: Vec<_> = (0..p.len()).collect();
    order.sort_by(|&a, &b| {
        bounds[b]
            .upper_bound
            .total_cmp(&bounds[a].upper_bound)
            .then(a.cmp(&b))
    });
    // Only discard an ascending tail whose TOTAL bound fits the error budget.
    let upper: Vec<_> = bounds.iter().map(|b| b.upper_bound).collect();
    let (omitted_upper, skipped) = omissions(&upper, &order, reference * tolerance);
    println!(
        "{}",
        json!({"model_ms":model_ms,"setup_ms":setup_ms,"bounds_ms":bounds_ms,"workers_max":4,
        "reference_probability":reference,"relative_tolerance":tolerance,"omitted_upper":omitted_upper,
        "retained":skipped.iter().filter(|&&s|!s).count(),"skipped":skipped.iter().filter(|&&s|s).count(),
        "branches":order.iter().map(|&i|{
            let b=bounds[i]; let s=&description["strata"][i];
            json!({"index":i,"exceptions":s["exceptions"],"secondary_team":s["secondary_team"],"forced_count":s["forced_count"],
                "fixed_prior":b.fixed_prior,"domain_prior":b.domain_prior,"normalizer_upper":b.normalizer_upper,
                "six_rival_upper":b.six_rival_upper,"read_two_upper":b.read_two_upper,"upper_bound":b.upper_bound,"skipped":skipped[i]})
        }).collect::<Vec<_>>() })
    );
    Ok(())
}

fn omissions(upper: &[f64], descending: &[usize], budget: f64) -> (f64, Vec<bool>) {
    let mut total = 0.;
    let mut skipped = vec![false; upper.len()];
    for &i in descending.iter().rev() {
        let next = total + upper[i];
        if next > budget {
            break;
        }
        total = next;
        skipped[i] = true;
    }
    (total, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omission_budget_is_for_the_sum_of_branches() {
        let (total, skipped) = omissions(&[0.002, 0.003, 0.5], &[2, 1, 0], 0.004);
        assert_eq!(total, 0.002);
        assert_eq!(skipped, vec![true, false, false]);
    }

    #[test]
    fn zero_budget_retains_every_positive_branch() {
        let (total, skipped) = omissions(&[1e-40, 1e-30], &[1, 0], 0.);
        assert_eq!(total, 0.);
        assert_eq!(skipped, vec![false, false]);
    }
}
