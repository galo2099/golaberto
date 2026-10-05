use golaberto_odds::{
    api,
    model::{Key, Model, Request},
    rng::Rng,
};
fn fixture_request() -> Request {
    serde_json::from_str(include_str!("fixtures/group-17058.json")).unwrap()
}

fn team_points(model: &Model) -> Vec<(i32, i32)> {
    model.ids[..model.n]
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, model.base[i].points))
        .collect()
}

#[test]
fn fixture_reproduces_concrete_standings_that_default_to_zero() {
    let request = fixture_request();
    let model = Model::new(request.clone()).unwrap();
    assert_eq!(model.n, 4);
    assert_eq!(model.fixtures.len(), 6);
    assert_eq!(&model.keys[..2], &[Key::Pt, Key::Head]);

    // Outcomes use canonical scores: home win 1-0, draw 0-0, away win 0-1.
    // Places here are one-based, matching the odds response's position names.
    let cases: &[(i32, usize, [u8; 6], [(i32, i32); 4])] = &[
        (
            2698,
            1,
            [2, 0, 1, 0, 1, 2],
            [(2687, 8), (2701, 8), (2699, 8), (2698, 9)],
        ),
        (
            2687,
            4,
            [2, 0, 2, 0, 0, 2],
            [(2687, 6), (2701, 10), (2699, 10), (2698, 9)],
        ),
        (
            2699,
            4,
            [2, 0, 0, 0, 2, 2],
            [(2687, 12), (2701, 7), (2699, 7), (2698, 9)],
        ),
        (
            2698,
            2,
            [2, 1, 0, 0, 2, 2],
            [(2687, 12), (2701, 5), (2699, 8), (2698, 9)],
        ),
    ];
    let future_ids = [370686, 370687, 370692, 370693, 370701, 370702];

    for &(target, place, outcomes, expected_points) in cases {
        let mut request = request.clone();
        for (id, outcome) in future_ids.into_iter().zip(outcomes) {
            let game = request.games.iter_mut().find(|game| game.id == id).unwrap();
            assert!(!game.played);
            (game.home_score, game.away_score) = match outcome {
                0 => (1, 0),
                1 => (0, 0),
                2 => (0, 1),
                _ => unreachable!(),
            };
            game.played = true;
        }
        let completed = Model::new(request).unwrap();
        assert!(completed.fixtures.is_empty());
        assert_eq!(
            team_points(&completed),
            expected_points,
            "target team {target} at place {place}"
        );
        let mut order = vec![0; completed.n];
        let mut rng = Rng::new(808);
        let scores = completed.empty_scores();
        completed.standings(&mut order, &completed.base, &scores, &mut rng);
        let actual_place = order
            .iter()
            .position(|&team| completed.ids[team] == target)
            .unwrap()
            + 1;
        assert_eq!(actual_place, place, "target team {target}");
    }
}

#[test]
fn small_league_estimates_reachable_zero_cells_with_stratified_paths() {
    let request = fixture_request();
    let model = Model::new(request.clone()).unwrap();
    let run = |seed, workers| {
        api::calculate(request.clone(), seed, workers, 100)
            .unwrap()
            .0
    };
    let response = run(808, 1);
    let parallel = run(808, 4);

    // The small-group design allocates work around outcome paths. Its answer and
    // uncertainty metadata must not depend on the number of worker threads.
    assert_eq!(
        serde_json::to_value(&response.rare_position_estimates).unwrap(),
        serde_json::to_value(&parallel.rare_position_estimates).unwrap()
    );
    assert!(response
        .rare_position_estimates
        .values()
        .flat_map(|ranks| ranks.values())
        .all(|e| { e.design == "outcome_path_stratified" && e.samples < 100_000 }));

    for (&team, odds) in &response.team_odds {
        assert_eq!(odds.pos.len(), 4);
        assert!(
            odds.pos.iter().all(|p| p.is_finite() && *p > 0.),
            "team {team}"
        );
        assert!(
            (odds.pos.iter().sum::<f64>() - 100.).abs() < 1e-12,
            "team {team}"
        );
    }
    for place in 0..4 {
        let total: f64 = response.team_odds.values().map(|o| o.pos[place]).sum();
        assert!((total - 100.).abs() < 1e-12, "place {place}: {total}");
    }

    let rare = &response.rare_position_estimates;
    let expected_rare = [(2698, 0), (2698, 1), (2687, 3), (2699, 3)];
    for (team, place) in expected_rare {
        let estimate = &rare[&team][&place];
        assert!(estimate.probability.is_finite() && estimate.probability > 0.);
        assert!(estimate.std_err.is_finite() && estimate.std_err >= 0.);
        assert_eq!(
            estimate.reachability, "reachable",
            "team {team}, place {place}"
        );
    }

    // One explicit six-result path proves a lower bound for 2698 finishing
    // first: away, home, draw, home, draw, away in fixture order.
    // Scores::prob indices are away, draw, home.
    let path = [0, 2, 1, 2, 1, 0];
    let prior: f64 = model
        .fixtures
        .iter()
        .zip(path)
        .map(|(fixture, outcome)| fixture.prob[outcome])
        .product();
    assert!(prior > 0.);
    assert!(rare[&2698][&0].probability + 1e-15 >= prior);

    // A different seed may change the estimate, but its movement should fit
    // within a conservative six-standard-error band from both runs.
    let other = run(809, 1);
    for (team, place) in expected_rare {
        let a = &rare[&team][&place];
        let b = &other.rare_position_estimates[&team][&place];
        let allowance = 6. * (a.std_err * a.std_err + b.std_err * b.std_err).sqrt() + 1e-12;
        assert!(
            (a.probability - b.probability).abs() <= allowance,
            "team {team}, place {place}: {} vs {}, allowance {allowance}",
            a.probability,
            b.probability
        );
    }
}

#[test]
fn two_team_head_to_head_bucket_probability_matches_poisson_reference() {
    let request: Request = serde_json::from_str(
        r#"{
          "id": 1,
          "zones": [],
          "phase": {"sort":"pt,head,gd,gf,g_away,w,name", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
          "team_groups": [{"team_id":1,"add_sub":0,"bias":0},{"team_id":2,"add_sub":0,"bias":0}],
          "games": [
            {"id":1,"home_id":1,"away_id":2,"home_score":3,"away_score":0,"played":true,"home_power":1.0,"away_power":0.5},
            {"id":2,"home_id":2,"away_id":1,"home_score":0,"away_score":0,"played":false,"home_power":1.4,"away_power":0.9}
          ]
        }"#,
    )
    .unwrap();

    // Let h/a be the future B-home/A-away score. A has three points of
    // historical goal difference. At equal final goal difference, equal GF
    // occurs only for (h,a)=(3,0); that exact residual tie is split evenly.
    let home = golaberto_odds::sampling::masses(1.4);
    let away = golaberto_odds::sampling::masses(0.9);
    let mut favorable = 0.;
    let mut total = 0.;
    for (h, &ph) in home.iter().enumerate() {
        for (a, &pa) in away.iter().enumerate() {
            let mass = ph * pa;
            total += mass;
            match h as i32 - a as i32 {
                d if d < 3 => favorable += mass,
                3 if a > 0 => favorable += mass,
                3 if a == 0 => favorable += mass * 0.5,
                _ => {}
            }
        }
    }
    let expected = favorable / total;
    let (response, _) = api::calculate(request, 808, 1, 100).unwrap();
    let estimate = &response.rare_position_estimates[&1][&0];
    assert_eq!(estimate.design, "outcome_path_stratified");
    assert!(estimate.samples > 0);
    assert!(estimate.std_err.is_finite() && estimate.std_err > 0.);
    assert!(
        (estimate.probability - expected).abs() <= 5. * estimate.std_err + 1e-12,
        "estimate {}, exact reference {expected}, se {}",
        estimate.probability,
        estimate.std_err
    );

    for odds in response.team_odds.values() {
        assert_eq!(odds.pos.len(), 2);
        assert!(odds.pos.iter().all(|p| p.is_finite() && *p > 0.));
        assert!((odds.pos.iter().sum::<f64>() - 100.).abs() < 1e-12);
    }
    for place in 0..2 {
        let total: f64 = response
            .team_odds
            .values()
            .map(|odds| odds.pos[place])
            .sum();
        assert!((total - 100.).abs() < 1e-12);
    }
}
