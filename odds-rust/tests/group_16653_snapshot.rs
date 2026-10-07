use golaberto_odds::{
    api,
    model::Model,
    pool::Estimate,
    proof::Problem,
    rank_proof::{Options, RankProof},
    search::Cell,
};

fn snapshot_request() -> golaberto_odds::model::Request {
    serde_json::from_str(include_str!("fixtures/group-16653-2026-10-07.json")).unwrap()
}

#[test]
fn october_7_group_snapshot_preserves_model_and_team_550_state() {
    let request = snapshot_request();
    let model = Model::new(request.clone()).unwrap();

    assert_eq!(request.id, 16653);
    assert_eq!(request.phase.sort, "pt,w,gd,gf,head,name");
    assert_eq!(request.phase.championship.point_win, 3);
    assert_eq!(request.phase.championship.point_draw, 1);
    assert_eq!(request.phase.championship.point_loss, 0);
    assert_eq!(model.n, 20);
    assert_eq!(
        model.ids,
        request
            .team_groups
            .iter()
            .map(|team| team.team_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(request.games.len(), 380);
    assert_eq!(request.games.iter().filter(|game| game.played).count(), 313);
    assert_eq!(model.fixtures.len(), 67);

    let team_index = model.indices[&550];
    let campaign = model.base[team_index];
    assert_eq!(campaign.points, 42);
    assert_eq!(
        (campaign.wins, campaign.draws, campaign.losses),
        (10, 12, 10)
    );
    assert_eq!((campaign.gf, campaign.ga, campaign.away), (38, 37, 19));

    let remaining_opponents = request
        .games
        .iter()
        .filter(|game| !game.played && (game.home_id == 550 || game.away_id == 550))
        .map(|game| {
            if game.home_id == 550 {
                game.away_id
            } else {
                game.home_id
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(remaining_opponents, vec![95, 68, 588, 69, 279, 429]);
}

#[test]
fn packed_rank_proof_resolves_team_550_first_place_and_early_report_uses_it() {
    let model = Model::new(snapshot_request()).unwrap();
    let team = model.indices[&550];
    let cell = Cell { team, rank: 0 };
    let points = Problem::new(&model).unwrap();
    let points_result = points.impossible(team, 0, points.target_max(team), 500);
    assert_eq!(points_result, (false, 1));
    let packed = RankProof::new(&model).unwrap().negated();
    let result = packed.prove(
        cell,
        model.n - 1,
        500,
        Options {
            require_root_pressure: true,
            ..Options::default()
        },
    );
    assert!(result.impossible, "{:#?}", result.stats);
    assert!(result.stats.nodes <= 500);

    let mut estimates = vec![Estimate::default(); model.n * model.n];
    let estimate = &mut estimates[cell.index(model.n)];
    estimate.reachability = "undecided".into();
    estimate.zero_hit_upper_95 = 1.;
    let report = golaberto_odds::proof::early_report(&model, &[cell], &mut estimates);
    assert_eq!(report.proofs, 1);
    assert_eq!(report.recycle_credit, 0);
    let estimate = &estimates[cell.index(model.n)];
    assert_eq!(estimate.reachability, "impossible_by_joint_rank");
    assert_eq!(estimate.zero_hit_upper_95, 0.);
}

#[test]
fn api_reports_team_550_first_place_as_impossible_deterministically() {
    let request = snapshot_request();
    let (one_worker, _) = api::calculate(request.clone(), 808, 1, 100).unwrap();
    let (four_workers, _) = api::calculate(request, 808, 4, 100).unwrap();

    for response in [&one_worker, &four_workers] {
        let estimate = &response.rare_position_estimates[&550][&0];
        assert_eq!(estimate.reachability, "impossible");
        assert_eq!(estimate.probability, 0.);
        assert_eq!(estimate.zero_hit_upper_95, 0.);
    }
    assert_eq!(
        one_worker.rare_position_estimates[&550][&0].reachability,
        four_workers.rare_position_estimates[&550][&0].reachability
    );
    assert_eq!(
        one_worker.team_odds[&550].pos[0],
        four_workers.team_odds[&550].pos[0]
    );
}
