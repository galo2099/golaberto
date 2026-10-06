use golaberto_odds::{
    logging::RequestLog,
    model::{Model, Request},
    pool::Estimate,
    search,
};

fn fixture_request() -> Request {
    serde_json::from_str(include_str!("fixtures/group-17058.json")).unwrap()
}

fn b3_request() -> Request {
    serde_json::from_str(include_str!("fixtures/group-17064.json")).unwrap()
}

fn uniform_estimates(n: usize, design: &str) -> Vec<Estimate> {
    (0..n * n)
        .map(|_| Estimate {
            probability: 1. / n as f64,
            std_err: 0.01,
            samples: 100_000,
            hits: 0,
            ess: 0.,
            mean_weight: 1.,
            work_spent: 17,
            available: true,
            meets_precision_goal: false,
            relative_se: Some(0.02),
            max_event_weight_share: 0.25,
            zero_hit_upper_95: 0.,
            design: design.into(),
            reachability: "pool_positive".into(),
            conditional_mass: 0.4,
            conditional_samples: 123,
            conditional_hits: 0,
        })
        .collect()
}

#[test]
fn donor_only_search_is_deterministic_and_unsupported_plain_mc_stays_skipped() {
    std::env::set_var("RUST_ODDS_RARE_TAIL_BUDGET_FRACTION", "0.45");
    std::env::set_var("RUST_ODDS_DETERMINISTIC_WORK", "1");
    std::env::set_var("RUST_ODDS_LOG", "0");
    let request = fixture_request();
    let model = Model::new(request.clone()).unwrap();
    assert!(golaberto_odds::pool::point_pmfs(&model).is_some());

    let mut group_pool = golaberto_odds::pool::production(&model, 809, 1);
    assert!(group_pool.iter().all(|e| e.design == "plain_mc"));
    let original_zeros: Vec<_> = group_pool
        .iter()
        .enumerate()
        .filter_map(|(i, e)| (e.probability == 0.).then_some(i))
        .collect();
    assert_eq!(original_zeros, vec![3, 11, 12, 13]);
    let group_pool_baseline = group_pool.clone();
    let group_work = search::run(&model, 809, 1, &mut group_pool);
    assert!(
        group_work > 0,
        "group-17058 fallback zeros should be searched"
    );
    assert!(group_pool.iter().all(|e| e.probability > 0.));
    assert!(group_work <= 100_000);

    let mut mixed_zero_and_donor = group_pool_baseline;
    mixed_zero_and_donor[0].design = "matched_point_pool".into();
    mixed_zero_and_donor[0].hits = 0;
    mixed_zero_and_donor[0].conditional_mass = 0.;
    mixed_zero_and_donor[0].conditional_samples = 0;
    mixed_zero_and_donor[0].conditional_hits = 0;
    let mixed_work = search::run(&model, 809, 1, &mut mixed_zero_and_donor);
    assert!(mixed_work > 0);
    assert!(original_zeros
        .iter()
        .all(|&i| mixed_zero_and_donor[i].probability > 0.));
    assert_eq!(
        mixed_zero_and_donor[0].design,
        "matched_point_pool_rare_tail"
    );
    assert!(mixed_zero_and_donor[0].conditional_samples > 0);
    assert!(mixed_zero_and_donor[0].conditional_hits > 0);

    // This exercises the pool fallback shape without paying for a full pool run:
    // positive estimates have no own hits, so only the confirmed tail may replace one.
    let mut one_worker = uniform_estimates(model.n, "matched_point_pool");
    let delta = 0.01;
    one_worker[0].probability += delta;
    one_worker[1].probability -= delta;
    one_worker[model.n].probability -= delta;
    one_worker[model.n + 1].probability += delta;
    one_worker[0].conditional_mass = 0.;
    one_worker[0].conditional_samples = 0;
    one_worker[0].conditional_hits = 0;
    let before = serde_json::to_value(&one_worker[0]).unwrap();
    let log = RequestLog::new().calculating();
    let work = search::run_logged(&model, 809, 1, &mut one_worker, Some(&log));
    let mut four_workers = uniform_estimates(model.n, "matched_point_pool");
    four_workers[0].probability += delta;
    four_workers[1].probability -= delta;
    four_workers[model.n].probability -= delta;
    four_workers[model.n + 1].probability += delta;
    four_workers[0].conditional_mass = 0.;
    four_workers[0].conditional_samples = 0;
    four_workers[0].conditional_hits = 0;
    let parallel_log = RequestLog::new().calculating();
    let parallel_work = search::run_logged(&model, 809, 4, &mut four_workers, Some(&parallel_log));
    assert!(work > 0, "donor-only request should receive rare-tail work");
    assert_eq!(work, parallel_work);
    assert_eq!(
        serde_json::to_value(&one_worker).unwrap(),
        serde_json::to_value(&four_workers).unwrap()
    );
    if one_worker[0].probability == 1. / model.n as f64 + delta {
        assert_eq!(serde_json::to_value(&one_worker[0]).unwrap(), before);
    } else {
        assert!(one_worker[0].probability > 0.);
        assert!(one_worker[0].design.contains("rare_tail"));
    }

    // Prior conditional evidence excludes a positive cell from the donor path.
    let mut evidenced = uniform_estimates(model.n, "matched_point_pool");
    let before = serde_json::to_value(&evidenced).unwrap();
    assert_eq!(search::run(&model, 809, 1, &mut evidenced), 0);
    assert_eq!(serde_json::to_value(&evidenced).unwrap(), before);

    let mut rejected = uniform_estimates(model.n, "matched_point_pool");
    rejected[0].probability += delta;
    rejected[1].probability -= delta;
    rejected[model.n].probability -= delta;
    rejected[model.n + 1].probability += delta;
    rejected[0].conditional_mass = 0.;
    rejected[0].conditional_samples = 0;
    rejected[0].conditional_hits = 0;
    // Disable the tail stage to force an unconfirmed donor through restoration.
    let donor_before = serde_json::to_value(&rejected[0]).unwrap();
    std::env::set_var("RUST_ODDS_RARE_TAIL", "disabled");
    assert_eq!(search::run(&model, 809, 1, &mut rejected), 0);
    std::env::remove_var("RUST_ODDS_RARE_TAIL");
    assert_eq!(serde_json::to_value(&rejected[0]).unwrap(), donor_before);

    // Supported point PMFs still send zero estimates through search when the pool
    // had to use plain Monte Carlo.
    let mut fallback_zeros = uniform_estimates(model.n, "plain_mc");
    fallback_zeros[0].probability = 0.;
    fallback_zeros[0].reachability = "undecided".into();
    let fallback_log = RequestLog::new().calculating();
    let fallback_work =
        search::run_logged(&model, 809, 1, &mut fallback_zeros, Some(&fallback_log));
    assert!(
        fallback_work > 0,
        "supported point PMFs should search plain-MC zero cells"
    );
    assert!(work <= 100_000 && fallback_work <= 100_000);

    let mut unsupported = request;
    unsupported.phase.sort = "head,pt,gd,gf,w,name".into();
    let unsupported = Model::new(unsupported).unwrap();
    assert!(golaberto_odds::pool::point_pmfs(&unsupported).is_none());
    let mut plain_mc = uniform_estimates(unsupported.n, "plain_mc");
    let before = serde_json::to_value(&plain_mc).unwrap();
    assert_eq!(search::run(&unsupported, 809, 1, &mut plain_mc), 0);
    assert_eq!(serde_json::to_value(&plain_mc).unwrap(), before);

    // B3 seed 809 has a positive matched estimate for 2689 finishing first even
    // though the scout observed no such finish. It must still receive tail work.
    let b3 = Model::new(b3_request()).unwrap();
    let b3_log = RequestLog::new().calculating();
    let mut pooled = golaberto_odds::pool::production(&b3, 809, 1);
    let team = b3.ids[..b3.n].iter().position(|&id| id == 2689).unwrap();
    let index = team * b3.n;
    assert_eq!(pooled[index].design, "matched_point_pool");
    assert_eq!(pooled[index].hits, 0);
    assert!(pooled[index].probability > 0.);
    let original_probability = pooled[index].probability;
    let b3_work = search::run_logged(&b3, 809, 1, &mut pooled, Some(&b3_log));
    assert!(
        b3_work > 0,
        "positive zero-hit pool cell should receive search work"
    );
    assert!(pooled[index].probability > 0.);
    assert_ne!(pooled[index].probability, original_probability);
    assert_eq!(pooled[index].design, "matched_point_pool_rare_tail");
    assert!(pooled[index].conditional_samples > 0);
    assert!(pooled[index].conditional_hits > 0);
    assert!(b3_work <= 100_000);
}
