use golaberto_odds::{
    api,
    model::{Key, Model, Request},
    rng::Rng,
};

fn fixture_request() -> Request {
    serde_json::from_str(include_str!("fixtures/group-17064.json")).unwrap()
}

#[test]
fn fixture_metadata_matches_group_b3() {
    let request = fixture_request();
    let model = Model::new(request.clone()).unwrap();

    assert_eq!(request.id, 17064);
    assert_eq!(request.phase.sort, "pt,head,gd,gf,g_away,w,name");
    assert_eq!(request.phase.championship.point_win, 3);
    assert_eq!(model.n, 4);
    assert_eq!(model.fixtures.len(), 4);
    assert_eq!(&model.keys[..2], &[Key::Pt, Key::Head]);
    assert_eq!(request.games.iter().filter(|game| game.played).count(), 8);
}

#[test]
fn all_team_points_tie_skips_head_and_uses_overall_goal_difference() {
    let mut request = fixture_request();
    let scores = [(0, 1), (0, 1), (0, 1), (10, 0)];
    for (game_id, (home, away)) in [370741, 370744, 370751, 370752].into_iter().zip(scores) {
        let game = request
            .games
            .iter_mut()
            .find(|game| game.id == game_id)
            .unwrap();
        (game.home_score, game.away_score) = (home, away);
        game.played = true;
    }

    let completed = Model::new(request).unwrap();
    assert_eq!(
        completed.ids[..completed.n]
            .iter()
            .enumerate()
            .map(|(index, &team)| (team, completed.base[index].points))
            .collect::<Vec<_>>(),
        vec![(2678, 8), (2679, 8), (2689, 8), (2702, 8)]
    );
    let mut order = vec![0; completed.n];
    let mut rng = Rng::new(808);
    let scores = completed.empty_scores();
    completed.standings(&mut order, &completed.base, &scores, &mut rng);
    let ordered_ids: Vec<_> = order.iter().map(|&team| completed.ids[team]).collect();
    assert_eq!(ordered_ids, vec![2689, 2679, 2702, 2678]);
}

#[test]
fn estimates_reachable_positions_for_ruby_standings() {
    let request = fixture_request();
    let run = |seed, workers| {
        api::calculate(request.clone(), seed, workers, 100)
            .unwrap()
            .0
    };
    let response = run(808, 1);
    let parallel = run(808, 4);

    assert_eq!(
        serde_json::to_value(&response.rare_position_estimates).unwrap(),
        serde_json::to_value(&parallel.rare_position_estimates).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&response.team_odds).unwrap(),
        serde_json::to_value(&parallel.team_odds).unwrap()
    );
    assert_eq!(response.rare_position_estimates.len(), 4);
    assert!(response.rare_position_estimates.values().all(|places| {
        places.len() == 4
            && places.values().all(|estimate| {
                estimate.design == "outcome_path_stratified" && estimate.samples > 0
            })
    }));
    for (&team, odds) in &response.team_odds {
        assert_eq!(odds.pos.len(), 4, "team {team}");
        assert!(odds.pos.iter().all(|p| p.is_finite() && *p >= 0.));
        assert!((odds.pos.iter().sum::<f64>() - 100.).abs() < 1e-12);
    }
    for place in 0..4 {
        let total: f64 = response
            .team_odds
            .values()
            .map(|odds| odds.pos[place])
            .sum();
        assert!((total - 100.).abs() < 1e-12, "place {place}: {total}");
    }

    let mut zero_cells = Vec::new();
    let mut positive_cells = 0;
    for (&team, places) in &response.rare_position_estimates {
        for (&place, estimate) in places {
            if estimate.probability > 0. {
                positive_cells += 1;
                assert_eq!(
                    estimate.reachability, "reachable",
                    "team {team}, place {place}"
                );
            } else {
                assert_eq!(estimate.probability, 0.);
                assert_eq!(
                    estimate.reachability, "undecided",
                    "team {team}, place {place}"
                );
                zero_cells.push((team, place));
            }
        }
    }
    assert_eq!(positive_cells, 16);
    assert_eq!(zero_cells.len(), 0);

    // Exhaust canonical outcomes as an independent reachability witness.
    let future_ids: Vec<_> = fixture_request()
        .games
        .iter()
        .filter(|game| !game.played)
        .map(|game| game.id)
        .collect();
    let mut seen = std::collections::HashSet::new();
    for mut code in 0..3usize.pow(future_ids.len() as u32) {
        let mut completed_request = fixture_request();
        for id in &future_ids {
            let outcome = code % 3;
            code /= 3;
            let game = completed_request
                .games
                .iter_mut()
                .find(|game| game.id == *id)
                .unwrap();
            (game.home_score, game.away_score) = match outcome {
                0 => (1, 0),
                1 => (0, 0),
                _ => (0, 1),
            };
            game.played = true;
        }
        let completed = Model::new(completed_request).unwrap();
        let mut order = vec![0; completed.n];
        let mut rng = Rng::new(808);
        let scores = completed.empty_scores();
        completed.standings(&mut order, &completed.base, &scores, &mut rng);
        for (place, &team_index) in order.iter().enumerate() {
            seen.insert((completed.ids[team_index], place + 1));
        }
    }
    // Canonical 0-1/0-0/1-0 scores witness 14 cells. The remaining two
    // positions require the explicit 10-0 scoreline covered above.
    let mut extreme = fixture_request();
    for (id, (home, away)) in
        [370741, 370744, 370751, 370752]
            .into_iter()
            .zip([(0, 1), (0, 1), (0, 1), (10, 0)])
    {
        let game = extreme.games.iter_mut().find(|game| game.id == id).unwrap();
        (game.home_score, game.away_score, game.played) = (home, away, true);
    }
    let completed = Model::new(extreme).unwrap();
    let mut order = vec![0; completed.n];
    completed.standings(
        &mut order,
        &completed.base,
        &completed.empty_scores(),
        &mut Rng::new(808),
    );
    for (place, team) in order.into_iter().enumerate() {
        seen.insert((completed.ids[team], place + 1));
    }
    for team in [2678, 2679, 2689, 2702] {
        for place in 1..=4 {
            assert!(
                seen.contains(&(team, place)),
                "missing witness {team}/{place}"
            );
        }
    }
    let other_seed = run(809, 1);
    assert!(other_seed.rare_position_estimates.values().all(|places| {
        places
            .values()
            .all(|estimate| estimate.probability > 0. && estimate.reachability == "reachable")
    }));
}
