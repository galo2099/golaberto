use golaberto_odds::{
    api,
    model::{Key, Model, Request},
    rng::Rng,
};

fn fixture_request() -> Request {
    serde_json::from_str(include_str!("fixtures/group-17059.json")).unwrap()
}

#[test]
fn fixture_metadata_matches_group_c2() {
    let request = fixture_request();
    let model = Model::new(request.clone()).unwrap();

    assert_eq!(request.id, 17059);
    let metadata: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/group-17059.json")).unwrap();
    assert_eq!(metadata["name"], "Group C2");
    assert_eq!(request.phase.sort, "pt,head,gd,gf,g_away,w,name");
    assert_eq!(request.phase.championship.point_win, 3);
    assert_eq!(model.n, 4);
    assert_eq!(model.fixtures.len(), 4);
    assert_eq!(&model.keys[..2], &[Key::Pt, Key::Head]);
    assert_eq!(request.games.iter().filter(|game| game.played).count(), 8);
}

#[test]
fn discrete_standings_certificate_transfers_to_twenty_team_groups() {
    let mut request = fixture_request();
    for index in 0..16 {
        request.team_groups.push(golaberto_odds::model::Team {
            team_id: 3000 + index,
            add_sub: 100 + index * 10,
            bias: 0,
        });
    }
    let model = Model::new(request).unwrap();
    assert_eq!(model.n, 20);
    let (estimates, diagnostic) =
        golaberto_odds::outcome_stratification::estimate(&model, 808).unwrap();
    assert_eq!(diagnostic["discrete_reachability_coverage_enabled"], true);
    for (team, group_rank) in [
        (2676, 0),
        (2688, 3),
        (2694, 0),
        (2694, 1),
        (2696, 2),
        (2696, 3),
    ] {
        let team_index = model.ids.iter().position(|id| *id == team).unwrap();
        let estimate = &estimates[team_index * model.n + group_rank + 16];
        assert_eq!(estimate.probability, 0., "team {team} rank {group_rank}");
        assert_eq!(estimate.zero_hit_upper_95, 0.);
        assert_eq!(estimate.reachability, "impossible_by_discrete_standings");
    }
}

#[test]
fn certifies_group_c2_zero_cells_and_preserves_scoreline_witnesses() {
    let request = fixture_request();
    let run = |seed, workers| {
        api::calculate(request.clone(), seed, workers, 100)
            .unwrap()
            .0
    };
    let response_808 = run(808, 1);
    let response_808_parallel = run(808, 4);
    let response_809 = run(809, 1);
    assert_eq!(
        serde_json::to_value(&response_808.rare_position_estimates).unwrap(),
        serde_json::to_value(&response_808_parallel.rare_position_estimates).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&response_808.team_odds).unwrap(),
        serde_json::to_value(&response_808_parallel.team_odds).unwrap()
    );
    let responses = [(808, response_808), (809, response_809)];
    let expected_zeros = [
        (2676, 0),
        (2688, 3),
        (2694, 0),
        (2694, 1),
        (2696, 2),
        (2696, 3),
    ];

    for (seed, response) in &responses {
        let zero_cells: Vec<_> = response
            .rare_position_estimates
            .iter()
            .flat_map(|(&team, places)| {
                places.iter().filter_map(move |(&place, estimate)| {
                    (estimate.probability == 0.).then_some((team, place, estimate))
                })
            })
            .collect();
        println!("C2 seed={seed} zero_cells={}", zero_cells.len());
        let actual_zeros: Vec<_> = zero_cells
            .iter()
            .map(|(team, place, _)| (*team, *place))
            .collect();
        assert_eq!(actual_zeros, expected_zeros);
        for (team, place, estimate) in zero_cells {
            assert_eq!(estimate.probability, 0.);
            assert_eq!(estimate.zero_hit_upper_95, 0.);
            assert_eq!(estimate.reachability, "impossible");
            println!(
                "C2 seed={seed} zero team={team} place={} probability={} std_err={} relative_se={:?} reachability={}",
                place + 1,
                estimate.probability,
                estimate.std_err,
                estimate.relative_se,
                estimate.reachability
            );
        }
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
        assert_eq!(
            response
                .rare_position_estimates
                .values()
                .flat_map(|places| places.values())
                .filter(|estimate| estimate.probability > 0.)
                .count(),
            10
        );
    }

    let future_ids: Vec<_> = request
        .games
        .iter()
        .filter(|game| !game.played)
        .map(|game| game.id)
        .collect();
    let mut witnesses = std::collections::HashSet::new();
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
        completed.standings(
            &mut order,
            &completed.base,
            &completed.empty_scores(),
            &mut Rng::new(808),
        );
        for (place, &team_index) in order.iter().enumerate() {
            witnesses.insert((completed.ids[team_index], place));
        }
    }

    for (seed, response) in &responses {
        for (&team, places) in &response.rare_position_estimates {
            for (&place, estimate) in places {
                let witnessed = witnesses.contains(&(team, place));
                if estimate.probability > 0. {
                    assert!(
                        witnessed,
                        "positive estimate has no WDL witness: {team}/{}",
                        place + 1
                    );
                    assert_eq!(estimate.reachability, "reachable");
                } else {
                    assert!(
                        !witnessed,
                        "zero estimate has a canonical WDL witness: {team}/{}",
                        place + 1
                    );
                    assert_eq!(estimate.reachability, "impossible");
                }
                println!(
                    "C2 seed={seed} team={team} place={} probability={} std_err={} relative_se={:?} reachability={} witness={witnessed}",
                    place + 1,
                    estimate.probability,
                    estimate.std_err,
                    estimate.relative_se,
                    estimate.reachability
                );
            }
        }
    }
}
