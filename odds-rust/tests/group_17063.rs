use golaberto_odds::{
    api,
    model::{Campaign, Model, Request, Team},
    rng::Rng,
};

const TEAM_2700: i32 = 2700;
const TEAM_1013: i32 = 1013;
const TEAM_2691: i32 = 2691;
const TEAM_257: i32 = 257;
const FUTURE_GAMES: [i32; 4] = [370742, 370743, 370749, 370750];

fn fixture_request() -> Request {
    serde_json::from_str(include_str!("fixtures/group-17063.json")).unwrap()
}

fn team_index(model: &Model, id: i32) -> usize {
    model.ids[..model.n]
        .iter()
        .position(|&candidate| candidate == id)
        .unwrap()
}

fn completed_request(outcomes: [usize; 4], scores: [[i32; 2]; 4]) -> Request {
    let mut request = fixture_request();
    for ((game_id, outcome), score) in FUTURE_GAMES.into_iter().zip(outcomes).zip(scores) {
        let game = request
            .games
            .iter_mut()
            .find(|game| game.id == game_id)
            .unwrap();
        let expected_outcome = match (score[0].cmp(&score[1]), outcome) {
            (std::cmp::Ordering::Less, 0)
            | (std::cmp::Ordering::Equal, 1)
            | (std::cmp::Ordering::Greater, 2) => true,
            _ => false,
        };
        assert!(
            expected_outcome,
            "score {score:?} does not match outcome {outcome}"
        );
        (game.home_score, game.away_score, game.played) = (score[0], score[1], true);
    }
    request
}

fn standings(request: Request) -> (Model, Vec<i32>, Vec<Campaign>) {
    let model = Model::new(request).unwrap();
    let mut order = vec![0; model.n];
    model.standings(
        &mut order,
        &model.base,
        &model.empty_scores(),
        &mut Rng::new(808),
    );
    let ids = order.iter().map(|&team| model.ids[team]).collect();
    let campaigns = model.base.clone();
    (model, ids, campaigns)
}

#[test]
fn only_one_future_wdl_path_can_put_every_rival_at_or_above_1013_points() {
    let mut qualifying_paths = Vec::new();
    for mut code in 0..3usize.pow(4) {
        let mut outcomes = [0; 4];
        let mut scores = [[0; 2]; 4];
        for index in 0..4 {
            outcomes[index] = code % 3;
            code /= 3;
            scores[index] = match outcomes[index] {
                0 => [0, 1], // away win
                1 => [0, 0], // draw
                _ => [1, 0], // home win
            };
        }

        let request = completed_request(outcomes, scores);
        let (model, _, campaigns) = standings(request);
        let target_points = campaigns[team_index(&model, TEAM_1013)].points;
        let all_rivals_at_or_above = [TEAM_2700, TEAM_2691, TEAM_257]
            .into_iter()
            .all(|team| campaigns[team_index(&model, team)].points >= target_points);
        if all_rivals_at_or_above {
            qualifying_paths.push(outcomes);
        }
    }
    assert_eq!(qualifying_paths, vec![[2, 0, 0, 0]]);
}

#[test]
fn unique_path_prevents_1013_from_finishing_fourth_across_future_goal_margins() {
    let margins = [
        [[1, 0], [0, 1], [0, 1], [0, 1]],
        [[25, 0], [0, 40], [0, 31], [0, 22]],
        [[8, 0], [0, 3], [0, 50], [0, 4]],
    ];
    for scores in margins {
        let request = completed_request([2, 0, 0, 0], scores);
        let (model, actual_order, campaigns) = standings(request);
        assert_eq!(
            actual_order,
            vec![TEAM_2691, TEAM_2700, TEAM_1013, TEAM_257],
            "future scores {scores:?}"
        );

        let index_1013 = team_index(&model, TEAM_1013);
        let index_257 = team_index(&model, TEAM_257);
        let mut mini = vec![Campaign::default(); model.n];
        for game in fixture_request().games.iter().filter(|game| {
            game.played
                && [TEAM_1013, TEAM_257].contains(&game.home_id)
                && [TEAM_1013, TEAM_257].contains(&game.away_id)
        }) {
            let home = team_index(&model, game.home_id);
            let away = team_index(&model, game.away_id);
            mini[home].add(
                game.home_score,
                game.away_score,
                true,
                &model.request.phase.championship,
            );
            mini[away].add(
                game.away_score,
                game.home_score,
                false,
                &model.request.phase.championship,
            );
        }
        assert_eq!(campaigns[index_1013].points, 7);
        assert_eq!(campaigns[index_257].points, 7);
        assert_eq!(campaigns[team_index(&model, TEAM_2700)].points, 8);
        assert_eq!(campaigns[team_index(&model, TEAM_2691)].points, 11);
        assert_eq!(mini[index_1013].points, 3);
        assert_eq!(mini[index_257].points, 3);
        assert_eq!(mini[index_1013].gf - mini[index_1013].ga, 0);
        assert_eq!(mini[index_257].gf - mini[index_257].ga, 0);
        assert_eq!(mini[index_1013].gf, 2);
        assert_eq!(mini[index_257].gf, 2);
        assert_eq!(mini[index_1013].away, 2);
        assert_eq!(mini[index_257].away, 1);
    }
}

#[test]
fn discrete_certificate_excludes_team_1013_from_fourth_place() {
    let model = Model::new(fixture_request()).unwrap();
    let outcomes = [2, 0, 0, 0];
    let target = team_index(&model, TEAM_1013);
    for replay_limit in [64, 4_096] {
        let certificate = model.discrete_reachability(&outcomes, replay_limit);
        let possible = certificate.possible[target * model.n + 3];
        println!(
            "limit={replay_limit} completed={} replays={} team_1013_rank4_possible={possible}",
            certificate.completed, certificate.replays
        );
        assert!(certificate.completed);
        assert!(!possible, "team 1013 cannot finish fourth on this WDL path");
    }
}

#[test]
fn api_certifies_group_b2_rank_four_as_impossible() {
    let request = fixture_request();
    for (seed, workers) in [(808, 1), (808, 4), (809, 1), (809, 4)] {
        let (response, _) = api::calculate(request.clone(), seed, workers, 100).unwrap();
        let estimate = &response.rare_position_estimates[&TEAM_1013][&3];
        assert_eq!(estimate.probability, 0., "seed={seed} workers={workers}");
        assert_eq!(estimate.zero_hit_upper_95, 0.);
        assert_eq!(estimate.reachability, "impossible");
        let positive = response
            .rare_position_estimates
            .values()
            .flat_map(|places| places.values())
            .filter(|estimate| estimate.probability > 0.)
            .count();
        let impossible = response
            .rare_position_estimates
            .values()
            .flat_map(|places| places.values())
            .filter(|estimate| estimate.reachability == "impossible")
            .count();
        assert_eq!(positive, 13, "seed={seed} workers={workers}");
        assert_eq!(impossible, 3, "seed={seed} workers={workers}");
    }
}

#[test]
fn group_b2_certificate_and_api_cover_twenty_team_spectators() {
    let mut request = fixture_request();
    for index in 0..16 {
        request.team_groups.push(Team {
            team_id: 3000 + index,
            add_sub: 100 + index * 10,
            bias: 0,
        });
    }
    let model = Model::new(request.clone()).unwrap();
    let outcomes = [2, 0, 0, 0];
    let certificate = model.discrete_reachability(&outcomes, 4_096);
    let target = team_index(&model, TEAM_1013);
    assert!(certificate.completed);
    assert!(!certificate.possible[target * model.n + 19]);

    for (seed, workers) in [(808, 1), (808, 4), (809, 1), (809, 4)] {
        let (response, _) = api::calculate(request.clone(), seed, workers, 100).unwrap();
        let estimate = &response.rare_position_estimates[&TEAM_1013][&19];
        assert_eq!(estimate.probability, 0., "seed={seed} workers={workers}");
        assert_eq!(estimate.zero_hit_upper_95, 0.);
        assert_eq!(estimate.reachability, "impossible");
    }
}
