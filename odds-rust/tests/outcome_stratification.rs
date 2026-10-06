use golaberto_odds::{
    model::{Championship, Game, Model, Phase, Request, Team},
    outcome_stratification,
};

fn request(n: usize, fixtures: &[(i32, i32, f64, f64)], sort: &str) -> Request {
    Request {
        id: 7,
        zones: vec![],
        phase: Phase {
            sort: sort.into(),
            championship: Championship {
                point_win: 3,
                point_draw: 1,
                point_loss: 0,
            },
            bonus_points: 0,
            bonus_points_threshold: 0,
        },
        team_groups: (1..=n as i32)
            .map(|team_id| Team {
                team_id,
                add_sub: 0,
                bias: 0,
            })
            .collect(),
        games: fixtures
            .iter()
            .enumerate()
            .map(|(i, &(home_id, away_id, home_power, away_power))| Game {
                id: i as i32 + 1,
                home_id,
                away_id,
                home_score: 0,
                away_score: 0,
                home_power,
                away_power,
                played: false,
            })
            .collect(),
    }
}

#[test]
fn admits_more_than_six_teams_with_few_positive_fixture_paths() {
    let mut req = request(7, &[(1, 2, 0.7, 0.6), (3, 4, 0.8, 0.9)], "bias,pt,w,gd,gf");
    for (i, team) in req.team_groups.iter_mut().enumerate() {
        team.bias = i as i32;
    }
    let m = Model::new(req).unwrap();
    let (est, diag) = outcome_stratification::estimate(&m, 44).expect("bounded two-fixture plan");
    assert_eq!(diag["paths"], 9);
    assert!(
        diag["certification_and_preparation_work"].as_u64().unwrap()
            + diag["conditional_draw_work_budget"].as_u64().unwrap()
            <= diag["total_work_budget"].as_u64().unwrap()
    );
    assert!(est.iter().all(|e| e.design == "outcome_path_stratified"));
    // Independent enumeration of the nine positive WDL combinations.
    let mut exact = vec![0.; m.n * m.n];
    for a in 0..3 {
        for b in 0..3 {
            let mut c = m.base.clone();
            let mut scores = m.empty_scores();
            let mut mass = 1.;
            for (i, o) in [a, b].into_iter().enumerate() {
                let f = &m.fixtures[i];
                mass *= f.prob[o];
                let score = match o {
                    0 => [0, 1],
                    1 => [0, 0],
                    _ => [1, 0],
                };
                scores[f.request_index] = score;
                m.add(&mut c, i, score);
            }
            let mut order = vec![0; m.n];
            m.standings(
                &mut order,
                &c,
                &scores,
                &mut golaberto_odds::rng::Rng::new(0),
            );
            for (rank, team) in order.into_iter().enumerate() {
                exact[team * m.n + rank] += mass;
            }
        }
    }
    assert!(est
        .iter()
        .zip(exact)
        .all(|(actual, want)| (actual.probability - want).abs() < 1e-10));
    for t in 0..m.n {
        assert!(((0..m.n).map(|r| est[t * m.n + r].probability).sum::<f64>() - 1.).abs() < 1e-10);
    }
    for r in 0..m.n {
        assert!(((0..m.n).map(|t| est[t * m.n + r].probability).sum::<f64>() - 1.).abs() < 1e-10);
    }
}

#[test]
fn admits_more_than_eight_future_fixtures_when_only_one_outcome_path_is_possible() {
    let mut req = request(2, &[(1, 2, 0.0, 0.0); 9], "bias,pt,gd");
    req.team_groups[0].bias = 1;
    let m = Model::new(req).unwrap();
    let (_, diag) =
        outcome_stratification::estimate(&m, 12).expect("zero-support fixtures have one path");
    assert_eq!(diag["paths"], 1);
    assert_eq!(diag["sampled_strata"], 0);
}

#[test]
fn non_head_stratification_matches_independent_poisson_enumeration() {
    let m = Model::new(request(2, &[(1, 2, 1.1, 0.8)], "pt,gd,gf,bias")).unwrap();
    let (est, diag) =
        outcome_stratification::estimate(&m, 210).expect("non-head score-dependent plan");
    assert_eq!(diag["paths"], 3);
    let home = golaberto_odds::sampling::masses(1.1);
    let away = golaberto_odds::sampling::masses(0.8);
    let mut exact = [0.; 4];
    let mut total = 0.;
    for (h, &ph) in home.iter().enumerate() {
        for (a, &pa) in away.iter().enumerate() {
            let mass = ph * pa;
            total += mass;
            let mut c = m.base.clone();
            let mut scores = m.empty_scores();
            let score = [h as i32, a as i32];
            scores[m.fixtures[0].request_index] = score;
            m.add(&mut c, 0, score);
            let mut order = vec![0; 2];
            m.standings(
                &mut order,
                &c,
                &scores,
                &mut golaberto_odds::rng::Rng::new(0),
            );
            for (rank, t) in order.into_iter().enumerate() {
                exact[t * 2 + rank] += mass;
            }
        }
    }
    for e in &mut exact {
        *e /= total;
    }
    for (e, want) in est.iter().zip(exact) {
        assert!(
            (e.probability - want).abs() <= 6. * e.std_err + 1e-12,
            "got {} expected {want} se {}",
            e.probability,
            e.std_err
        );
    }
}

#[test]
fn repeated_pairs_and_external_opponents_use_general_path_sampler() {
    let req = request(
        3,
        &[(1, 2, 0.8, 0.7), (1, 2, 0.9, 0.8), (3, 99, 0.7, 0.6)],
        "pt,gd,gf",
    );
    let m = Model::new(req).unwrap();
    let (a, diag) = outcome_stratification::estimate(&m, 55).expect("general WDL plan");
    let (b, _) = outcome_stratification::estimate(&m, 55).expect("repeatable plan");
    assert_eq!(diag["paths"], 27);
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(&b).unwrap()
    );
    for r in 0..m.n {
        assert!(((0..m.n).map(|t| a[t * m.n + r].probability).sum::<f64>() - 1.).abs() < 1e-10);
    }
}

#[test]
fn random_fallback_is_sampled_and_seeded() {
    let m = Model::new(request(2, &[(1, 2, 0.0, 0.0)], "pt,random")).unwrap();
    let (a, diag) = outcome_stratification::estimate(&m, 81).unwrap();
    let (same, _) = outcome_stratification::estimate(&m, 81).unwrap();
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(&same).unwrap()
    );
    assert_eq!(diag["paths"], 1);
    let draws = diag["conditional_draws"].as_u64().unwrap();
    assert!((90_000..=100_000).contains(&draws));
    assert_eq!(
        diag["draws_per_stratum"].as_array().unwrap()[0]
            .as_u64()
            .unwrap(),
        draws
    );
    assert_eq!(
        diag["actual_conditional_draw_work"].as_u64().unwrap(),
        draws * diag["ordinary_season_work"].as_u64().unwrap()
    );
    assert_eq!(diag["conditional_floor"], 1_024);
    assert_eq!(diag["floor_draw_budget"], 1_024);
    assert!(
        diag["actual_conditional_draw_work"].as_u64().unwrap()
            <= diag["conditional_draw_work_budget"].as_u64().unwrap()
    );
    assert!(a
        .iter()
        .all(|e| e.samples as u64 == draws && e.std_err > 0.));
    assert!((a[0].probability - 0.5).abs() <= 6. * a[0].std_err + 1e-12);
}

#[test]
fn zero_probability_branches_are_not_counted_and_overflow_fails_before_enumeration() {
    let only_draw = Model::new(request(2, &[(1, 2, 0.0, 0.0)], "pt,w,gd")).unwrap();
    let (_, diag) = outcome_stratification::estimate(&only_draw, 2).unwrap();
    assert_eq!(diag["paths"], 1);
    let many: Vec<_> = (0..40)
        .map(|i| (((i % 2) + 1) as i32, ((i % 2) + 2) as i32, 1.0, 1.0))
        .collect();
    let m = Model::new(request(3, &many, "pt,w,gd")).unwrap();
    assert!(outcome_stratification::estimate(&m, 3).is_none());
}
