use golaberto_odds::{
    api,
    model::{Campaign, Key, Model, Request},
    rng::Rng,
    sampling::masses,
};

const A: i32 = 2676;
const B: i32 = 2688;
const C: i32 = 2694;
const D: i32 = 2696;

fn fixture_request() -> Request {
    serde_json::from_str(include_str!("fixtures/group-17059-head-gd.json")).unwrap()
}

fn request_with_margins(x: i32, y: i32) -> Request {
    let mut request = fixture_request();
    for game in &mut request.games {
        match game.id {
            370691 => {
                (game.home_score, game.away_score, game.played) = (x, 0, true);
            }
            370700 => {
                (game.home_score, game.away_score, game.played) = (y, 0, true);
            }
            _ => {}
        }
    }
    request
}

fn request_with_spectators() -> Request {
    let mut request = fixture_request();
    for i in 0..16 {
        request.team_groups.push(golaberto_odds::model::Team {
            team_id: 20_000 + i,
            add_sub: 100 + 10 * i,
            bias: 0,
        });
    }
    request
}

fn independent_c_second_probability(request: &Request) -> (f64, f64, f64) {
    let a_b = request.games.iter().find(|game| game.id == 370691).unwrap();
    let c_a = request.games.iter().find(|game| game.id == 370700).unwrap();
    let a_home = masses(a_b.home_power);
    let b_away = masses(a_b.away_power);
    let c_home = masses(c_a.home_power);
    let a_away = masses(c_a.away_power);
    let mut full = 0.;
    let mut strict_gd = 0.;
    let mut tied_gd_gf = 0.;
    for (h1, &p_h1) in a_home.iter().enumerate() {
        for (a1, &p_a1) in b_away.iter().enumerate() {
            let x = h1 as i32 - a1 as i32;
            if x <= 0 {
                continue;
            }
            for (h2, &p_h2) in c_home.iter().enumerate() {
                for (a2, &p_a2) in a_away.iter().enumerate() {
                    let y = h2 as i32 - a2 as i32;
                    if y <= 0 || 2 * y - x <= 9 {
                        continue;
                    }
                    let mass = p_h1 * p_a1 * p_h2 * p_a2;
                    if x + y > 18 {
                        full += mass;
                        strict_gd += mass;
                    } else if x + y == 18 && 1 + h2 as i32 > 10 + a1 as i32 {
                        full += mass;
                        tied_gd_gf += mass;
                    }
                }
            }
        }
    }
    (full, strict_gd, tied_gd_gf)
}

fn standings_for_margins(x: i32, y: i32) -> (Model, Vec<i32>, Vec<i32>, Vec<i32>) {
    let request = request_with_margins(x, y);
    let mut mini = vec![Campaign::default(); 3];
    let team_index = |id| {
        [A, B, C]
            .iter()
            .position(|candidate| *candidate == id)
            .unwrap()
    };
    for game in request.games.iter().filter(|game| {
        game.played && [A, B, C].contains(&game.home_id) && [A, B, C].contains(&game.away_id)
    }) {
        let home = team_index(game.home_id);
        let away = team_index(game.away_id);
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
    let mini_points = mini.iter().map(|campaign| campaign.points).collect();
    let mini_goal_difference = mini
        .iter()
        .map(|campaign| campaign.gf - campaign.ga)
        .collect();

    let model = Model::new(request).unwrap();
    let mut order = vec![0; model.n];
    model.standings(
        &mut order,
        &model.base,
        &model.empty_scores(),
        &mut Rng::new(808),
    );
    let ids = order.into_iter().map(|index| model.ids[index]).collect();
    (model, mini_points, mini_goal_difference, ids)
}

#[test]
fn fixture_metadata_and_base_points_match_synthetic_group() {
    let request = fixture_request();
    let metadata: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/group-17059-head-gd.json")).unwrap();
    let model = Model::new(request.clone()).unwrap();

    assert_eq!(request.id, 990059);
    assert_eq!(
        metadata["name"],
        "Synthetic C2: head-to-head goal difference"
    );
    assert_eq!(request.phase.sort, "pt,head,gd,gf,g_away,w,name");
    assert_eq!(&model.keys[..3], &[Key::Pt, Key::Head, Key::Gd]);
    assert_eq!(model.n, 4);
    assert_eq!(model.fixtures.len(), 2);
    assert_eq!(request.games.iter().filter(|game| game.played).count(), 10);
    assert_eq!(request.team_groups.len(), 4);

    let base_points: Vec<_> = model.base[..model.n]
        .iter()
        .map(|campaign| campaign.points)
        .collect();
    assert_eq!(base_points, vec![4, 7, 4, 12]);
}

#[test]
fn mini_table_goal_difference_can_put_c_second() {
    let cases = [
        (1, 1, vec![D, B, A, C], vec![0, 8, -8]),
        (1, 10, vec![D, B, C, A], vec![-9, 8, 1]),
        (10, 1, vec![D, A, B, C], vec![9, -1, -8]),
        (10, 10, vec![D, C, A, B], vec![0, -1, 1]),
        // The GD tie at 8-0 / 10-0 is resolved by C's higher mini-table GF.
        (8, 10, vec![D, C, B, A], vec![-2, 1, 1]),
    ];
    for (x, y, expected_order, expected_gd) in cases {
        let (_model, mini_points, mini_gd, actual_order) = standings_for_margins(x, y);
        assert_eq!(mini_points, vec![6, 6, 6], "margins {x}/{y}");
        assert_eq!(mini_gd, expected_gd, "margins {x}/{y}");
        assert_eq!(actual_order, expected_order, "margins {x}/{y}");
    }
}

#[test]
fn certificate_and_api_keep_c_second_reachable_despite_no_canonical_witness() {
    let request = fixture_request();
    let model = Model::new(request.clone()).unwrap();
    let outcomes = [2, 2]; // A beats B at home; C beats A at home.
    let ci = c_index(&model, C);
    let limited = model.discrete_reachability(&outcomes, 64);
    assert!(limited.possible[ci * model.n + 1]);
    if !limited.completed {
        assert!(limited.possible.iter().all(|possible| *possible));
    }
    let certificate = model.discrete_reachability(&outcomes, 1_024);
    assert!(certificate.completed);
    println!(
        "synthetic C2 discrete certificate replays={}",
        certificate.replays
    );
    assert!(certificate.replays > 1);
    assert!(certificate.possible[ci * model.n + 1]);
    assert_eq!(standings_for_margins(1, 1).3[1], B);
    assert_eq!(standings_for_margins(10, 10).3[1], C);

    // Independent product-Poisson probability for C beating both A and B on
    // mini-table GD. With X=A's margin over B and Y=C's margin over A,
    // C's GD is strictly higher when X+Y>18 and 2Y-X>9.
    let a_b = request.games.iter().find(|game| game.id == 370691).unwrap();
    let c_a = request.games.iter().find(|game| game.id == 370700).unwrap();
    let a_home = masses(a_b.home_power);
    let b_away = masses(a_b.away_power);
    let c_home = masses(c_a.home_power);
    let a_away = masses(c_a.away_power);
    let (oracle, lower_bound, tied_gd_gf) = independent_c_second_probability(&request);
    let explicit_10_0_mass = a_home[10] * b_away[0] * c_home[10] * a_away[0];
    assert!(explicit_10_0_mass > 0.);
    assert!(lower_bound >= explicit_10_0_mass);
    println!("synthetic C2 C-second independent GD lower_bound={lower_bound:e} 10-0 witness_mass={explicit_10_0_mass:e}");

    assert!((oracle - 9.865986065702399e-12).abs() < 1e-22);
    assert!((lower_bound - 1.871250324473612e-12).abs() < 1e-22);
    assert!((tied_gd_gf - 7.994735741228787e-12).abs() < 1e-22);
    assert!((oracle - lower_bound - tied_gd_gf).abs() < 1e-22);
    let explicit_8_0_10_0_mass = a_home[8] * b_away[0] * c_home[10] * a_away[0];
    assert!(explicit_8_0_10_0_mass > 0.);
    assert!(tied_gd_gf >= explicit_8_0_10_0_mass);
    println!("synthetic C2 independent full_oracle={oracle:e} strict_gd={lower_bound:e} tied_gd_gf={tied_gd_gf:e}");

    for seed in [808, 809] {
        let (one_worker, _) = api::calculate(request.clone(), seed, 1, 100).unwrap();
        let (four_workers, _) = api::calculate(request.clone(), seed, 4, 100).unwrap();
        let estimate = &one_worker.rare_position_estimates[&C][&1];
        let parallel = &four_workers.rare_position_estimates[&C][&1];
        assert_eq!(
            serde_json::to_value(estimate).unwrap(),
            serde_json::to_value(parallel).unwrap()
        );
        assert_eq!(estimate.reachability, "reachable");
        assert!(estimate.probability.is_finite() && estimate.probability > 0.);
        assert!(estimate.std_err.is_finite() && estimate.std_err > 0.);
        assert!((estimate.probability - oracle).abs() <= 6. * estimate.std_err);
        assert!(
            estimate.relative_se.unwrap() <= 0.4,
            "seed={seed} relative_se={:?}",
            estimate.relative_se
        );
        println!(
            "synthetic C2 seed={seed} workers=1 C/2 probability={} std_err={} relative_se={:?} reachability={}",
            estimate.probability,
            estimate.std_err,
            estimate.relative_se,
            estimate.reachability
        );
        assert_eq!(parallel.reachability, "reachable");
        println!(
            "synthetic C2 seed={seed} workers=4 C/2 probability={} std_err={} relative_se={:?} reachability={}",
            parallel.probability,
            parallel.std_err,
            parallel.relative_se,
            parallel.reachability
        );
    }

    let expanded = request_with_spectators();
    let expanded_oracle = independent_c_second_probability(&expanded).0;
    assert!((expanded_oracle - oracle).abs() < 1e-22);
    let expanded_model = Model::new(expanded.clone()).unwrap();
    assert_eq!(expanded_model.n, 20);
    let mut witness_scores = expanded_model.empty_scores();
    let mut witness_campaigns = expanded_model.base.clone();
    for (fixture_index, fixture) in expanded_model.fixtures.iter().enumerate() {
        let request_index = fixture.request_index;
        let game = &expanded.games[request_index];
        if game.id == 370691 || game.id == 370700 {
            witness_scores[request_index] = [10, 0];
            expanded_model.add(&mut witness_campaigns, fixture_index, [10, 0]);
        }
    }
    let mut witness_order = vec![0; expanded_model.n];
    expanded_model.standings(
        &mut witness_order,
        &witness_campaigns,
        &witness_scores,
        &mut Rng::new(808),
    );
    assert_eq!(expanded_model.ids[witness_order[17]], C);
    for seed in [808, 809] {
        let (one_worker, _) = api::calculate(expanded.clone(), seed, 1, 100).unwrap();
        let (four_workers, _) = api::calculate(expanded.clone(), seed, 4, 100).unwrap();
        let estimate = &one_worker.rare_position_estimates[&C][&17];
        let parallel = &four_workers.rare_position_estimates[&C][&17];
        assert_eq!(
            serde_json::to_value(estimate).unwrap(),
            serde_json::to_value(parallel).unwrap()
        );
        assert_eq!(estimate.reachability, "reachable");
        assert!(estimate.probability.is_finite() && estimate.probability > 0.);
        assert!(estimate.std_err.is_finite() && estimate.std_err > 0.);
        assert!((estimate.probability - expanded_oracle).abs() <= 6. * estimate.std_err);
        assert!(
            estimate.relative_se.unwrap() <= 0.4,
            "expanded seed={seed} relative_se={:?}",
            estimate.relative_se
        );
        assert_eq!(parallel.reachability, "reachable");
    }
}

fn c_index(model: &Model, team: i32) -> usize {
    model.ids.iter().position(|id| *id == team).unwrap()
}
