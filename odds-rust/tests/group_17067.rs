use golaberto_odds::{
    api,
    model::{Campaign, Key, Model, Request},
};

fn fixture_request() -> Request {
    serde_json::from_str(include_str!("fixtures/group-17067.json")).unwrap()
}

#[test]
fn fixture_metadata_matches_group_a2() {
    let request = fixture_request();
    let model = Model::new(request.clone()).unwrap();
    let metadata: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/group-17067.json")).unwrap();

    assert_eq!(request.id, 17067);
    assert_eq!(metadata["phase"]["id"], 4553);
    assert_eq!(metadata["phase"]["name"], "League A");
    assert_eq!(request.phase.sort, "pt,head,gd,gf,g_away,w,name");
    assert_eq!(request.phase.championship.point_win, 3);
    assert_eq!(model.n, 4);
    assert_eq!(model.fixtures.len(), 2);
    assert_eq!(&model.keys[..2], &[Key::Pt, Key::Head]);
    assert_eq!(request.games.iter().filter(|game| game.played).count(), 10);
}

#[test]
fn api_certifies_1035_first_and_preserves_246_third() {
    let request = fixture_request();
    let mut baselines = [None, None];
    for (seed, workers) in [(808, 1), (808, 4), (809, 1), (809, 4)] {
        let (response, _) = api::calculate(request.clone(), seed, workers, 100).unwrap();
        assert_eq!(response.rare_position_estimates.len(), 4);
        for (&team, places) in &response.rare_position_estimates {
            assert_eq!(places.len(), 4, "team {team}");
            for (&place, estimate) in places {
                println!(
                    "A2 seed={seed} workers={workers} team={team} place={} probability={} reachability={} design={} std_err={} relative_se={:?}",
                    place + 1,
                    estimate.probability,
                    estimate.reachability,
                    estimate.design,
                    estimate.std_err,
                    estimate.relative_se
                );
                assert!(
                    estimate.probability.is_finite() && (0.0..=1.0).contains(&estimate.probability)
                );
                assert!(estimate.std_err.is_finite() && estimate.std_err >= 0.0);
            }
        }
        let positive = response
            .rare_position_estimates
            .values()
            .flat_map(|places| places.values())
            .filter(|estimate| estimate.probability > 0.0)
            .count();
        let impossible = response
            .rare_position_estimates
            .values()
            .flat_map(|places| places.values())
            .filter(|estimate| estimate.reachability == "impossible")
            .count();
        assert_eq!(positive, 9, "seed={seed} workers={workers}");
        assert_eq!(impossible, 7, "seed={seed} workers={workers}");
        let first = &response.rare_position_estimates[&1035][&0];
        assert_eq!(first.probability, 0.0);
        assert_eq!(first.zero_hit_upper_95, 0.0);
        assert_eq!(first.reachability, "impossible");
        let team_246_third = &response.rare_position_estimates[&246][&2];
        assert!(team_246_third.probability > 0.0);
        assert_eq!(team_246_third.reachability, "reachable");
        for place in 0..4 {
            let total: f64 = response
                .team_odds
                .values()
                .map(|odds| odds.pos[place])
                .sum();
            assert!((total - 100.0).abs() < 1e-12, "place {place}: {total}");
        }
        for (&team, odds) in &response.team_odds {
            assert_eq!(odds.pos.len(), 4, "team {team}");
            assert!((odds.pos.iter().sum::<f64>() - 100.0).abs() < 1e-12);
        }
        let value =
            serde_json::to_value((&response.rare_position_estimates, &response.team_odds)).unwrap();
        let seed_index = usize::from(seed == 809);
        if workers == 1 {
            baselines[seed_index] = Some(value);
        } else {
            assert_eq!(
                Some(&value),
                baselines[seed_index].as_ref(),
                "seed {seed} worker-count determinism"
            );
        }
    }
}

#[test]
fn every_wdl_path_certificate_excludes_1035_from_first() {
    let model = Model::new(fixture_request()).unwrap();
    let first = model.ids[..model.n]
        .iter()
        .position(|&team| team == 1035)
        .unwrap();
    for mut code in 0..3usize.pow(model.fixtures.len() as u32) {
        let mut outcomes = vec![0; model.fixtures.len()];
        for outcome in &mut outcomes {
            *outcome = code % 3;
            code /= 3;
        }
        let certificate = model.discrete_reachability(&outcomes, 4096);
        assert!(certificate.completed, "outcomes={outcomes:?}");
        assert!(
            !certificate.possible[first * model.n],
            "outcomes={outcomes:?} replays={}",
            certificate.replays
        );
    }
}

#[test]
fn team_1035_stays_below_first_for_small_and_large_238_goal_margins() {
    for (x, expected) in [
        (1, [246, 238, 1035, 1039]),
        (13, [238, 246, 1035, 1039]),
        (14, [238, 1035, 246, 1039]),
        (15, [238, 1035, 246, 1039]),
    ] {
        let mut request = fixture_request();
        let first = request
            .games
            .iter_mut()
            .find(|game| game.id == 370799)
            .unwrap();
        (first.home_score, first.away_score, first.played) = (x, 0, true);
        let second = request
            .games
            .iter_mut()
            .find(|game| game.id == 370800)
            .unwrap();
        (second.home_score, second.away_score, second.played) = (1, 0, true);

        let mini_teams = [238, 246, 1035];
        let mut mini = [Campaign::default(); 3];
        for game in request.games.iter().filter(|game| game.played) {
            let Some(home) = mini_teams.iter().position(|&team| team == game.home_id) else {
                continue;
            };
            let Some(away) = mini_teams.iter().position(|&team| team == game.away_id) else {
                continue;
            };
            mini[home].add(
                game.home_score,
                game.away_score,
                true,
                &request.phase.championship,
            );
            mini[away].add(
                game.away_score,
                game.home_score,
                false,
                &request.phase.championship,
            );
        }
        let mini_points: Vec<_> = mini.iter().map(|team| team.points).collect();
        let mini_gd: Vec<_> = mini.iter().map(|team| team.gf - team.ga).collect();
        assert_eq!(mini_points, vec![5, 5, 5], "x={x}");
        assert_eq!(mini_gd, vec![x - 1, 7 - x, -6], "x={x}");

        let model = Model::new(request).unwrap();
        let mut order = vec![0; model.n];
        model.standings(
            &mut order,
            &model.base,
            &model.empty_scores(),
            &mut golaberto_odds::rng::Rng::new(808),
        );
        let actual: Vec<_> = order.iter().map(|&index| model.ids[index]).collect();
        assert_eq!(actual, expected, "x={x}");
    }
}

#[test]
fn certificate_excludes_1035_from_seventeenth_with_twenty_teams() {
    let mut request = fixture_request();
    for index in 0..16 {
        request.team_groups.push(golaberto_odds::model::Team {
            team_id: 3000 + index,
            add_sub: 100 + index * 10,
            bias: 0,
        });
    }
    let model = Model::new(request).unwrap();
    let target = model.ids[..model.n]
        .iter()
        .position(|&team| team == 1035)
        .unwrap();
    let certificate = model.discrete_reachability(&[2, 2], 4096);
    println!(
        "A2 twenty-team certificate completed={} replays={} target_global_rank17_possible={}",
        certificate.completed,
        certificate.replays,
        certificate.possible[target * model.n + 16]
    );
    assert!(certificate.completed);
    assert!(!certificate.possible[target * model.n + 16]);
}
