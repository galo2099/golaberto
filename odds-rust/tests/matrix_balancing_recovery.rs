use golaberto_odds::{
    api,
    model::{Model, Request},
    pool,
};
use serde_json::Value;

fn new_request_model() -> Model {
    let request: Request =
        serde_json::from_str(include_str!("fixtures/group-16498-2026-10-19.json")).unwrap();
    Model::new(request).unwrap()
}

#[test]
fn new_fixture_keeps_matched_pool_design_for_seeds_800_through_819() {
    let model = new_request_model();
    for seed in 800..=819 {
        let estimates = pool::production(&model, seed, 4);
        assert_eq!(estimates.len(), model.n * model.n);
        let matrix: Vec<_> = estimates
            .iter()
            .map(|estimate| estimate.probability)
            .collect();
        assert!(
            pool::valid(&matrix, model.n),
            "seed {seed} produced an invalid rank matrix"
        );
        assert!(
            estimates.iter().all(|estimate| {
                estimate.design == "matched_point_pool"
                    && estimate.probability.is_finite()
                    && (0.0..=1.0).contains(&estimate.probability)
            }),
            "seed {seed} fell back from the matched design"
        );
    }
}

#[test]
fn repaired_matrix_locks_structural_zeros_and_tiny_cells() {
    let tiny = vec![
        1. - 3e-6,
        3e-6,
        1e-40,
        1e-6,
        1. - 4e-6,
        3e-6,
        2e-6,
        2e-6,
        1. - 3e-6,
    ];
    let mut matrix = tiny.clone();
    assert!(pool::balance(&mut matrix, 3));
    assert!(pool::valid(&matrix, 3));
    assert!(matrix[2] > 0.0);

    let mut structural = vec![
        1. - 3e-6,
        3e-6,
        0.,
        1e-6,
        1. - 4e-6,
        3e-6,
        2e-6,
        2e-6,
        1. - 3e-6,
    ];
    assert!(pool::balance(&mut structural, 3));
    assert_eq!(structural[2].to_bits(), 0.0f64.to_bits());
    assert!(pool::valid(&structural, 3));
}

#[test]
fn production_recovery_is_worker_deterministic_and_preserves_all_positions() {
    let request: Request =
        serde_json::from_str(include_str!("fixtures/group-16498-2026-10-19.json")).unwrap();
    let (one_worker, _) = api::calculate(request.clone(), 808, 1, 20_000).unwrap();
    let (four_workers, _) = api::calculate(request, 808, 4, 20_000).unwrap();
    let one: Value = serde_json::to_value(one_worker).unwrap();
    let four: Value = serde_json::to_value(four_workers).unwrap();
    assert_eq!(
        one, four,
        "worker count must not change the recovered response"
    );

    let estimates = one["rare_position_estimates"].as_object().unwrap();
    assert_eq!(estimates.len(), 20);
    let mut matrix = Vec::with_capacity(400);
    for row in estimates.values() {
        let ranks = row.as_object().unwrap();
        assert_eq!(ranks.len(), 20);
        for estimate in ranks.values() {
            let probability = estimate["probability"].as_f64().unwrap();
            assert!(probability.is_finite() && (0.0..=1.0).contains(&probability));
            let impossible = estimate["reachability"]
                .as_str()
                .is_some_and(|label| label.starts_with("impossible"));
            assert!(
                impossible || probability > 0.0,
                "every position must be positive or proven impossible"
            );
            matrix.push(probability);
        }
    }
    assert!(pool::valid(&matrix, 20));

    let ninth = &one["rare_position_estimates"]["318"]["8"];
    assert_eq!(ninth["probability"].as_f64().unwrap(), 0.0);
    assert!(ninth["reachability"]
        .as_str()
        .unwrap()
        .starts_with("impossible"));
    let tenth = &one["rare_position_estimates"]["318"]["9"];
    assert!(tenth["probability"].as_f64().unwrap() > 0.0);
}
