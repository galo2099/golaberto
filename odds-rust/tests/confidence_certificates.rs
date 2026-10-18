use golaberto_odds::{
    confidence::{binomial_kl_interval, matched},
    model::{Model, Request},
    pool,
};
use serde_json::json;

fn small() -> Model {
    let request: Request = serde_json::from_value(json!({
        "id": 1,
        "phase": {"sort":"pt,w,bias", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":[{"team_id":1},{"team_id":2}],
        "games":[{"id":1,"home_id":1,"away_id":2,"home_power":0.8,"away_power":1.1,"played":false}]
    }))
    .unwrap();
    Model::new(request).unwrap()
}

#[test]
fn zero_native_hits_keep_a_positive_upper_bound_and_confidence_json_is_additive() {
    let interval = binomial_kl_interval(0, 1000, 0.001).unwrap();
    assert_eq!(interval.lower, 0.0);
    assert!(interval.upper > 0.0);
    let value = serde_json::to_value(pool::Estimate::default()).unwrap();
    assert!(value.get("confidence").is_none());
}

#[test]
fn matched_evidence_uses_own_point_rows_and_ignores_donors() {
    let model = small();
    let scout = pool::scout(&model, 20_000, 731);
    let evidence = matched::own_points_evidence(&model, &scout);
    assert!(!evidence.is_empty());
    let cell = evidence.iter().find(|(index, _)| *index == 0).unwrap();
    assert!(cell.1.samples <= scout.samples);
    assert_eq!(cell.1.method, "matched_own_points_strata");

    // Changing every other team's point/rank counts cannot change this cell's
    // certificate; no donor estimate contributes to its own-point evidence.
    let mut changed_donors = scout.clone();
    let per_team = changed_donors.span * model.n;
    for team in 1..model.n {
        let start = team * per_team;
        changed_donors.joint[start..start + per_team].fill(0);
    }
    let altered = matched::own_points_evidence(&model, &changed_donors);
    let altered_cell = altered.iter().find(|(index, _)| *index == 0).unwrap();
    assert_eq!(cell.1.interval, altered_cell.1.interval);
}

#[test]
fn unsupported_non_point_sorting_skips_matched_certificates() {
    let mut model = small();
    model.keys[0] = golaberto_odds::model::Key::W;
    let scout = pool::scout(&model, 100, 91);
    assert!(matched::own_points_evidence(&model, &scout).is_empty());
}

#[test]
fn unseen_own_point_bins_stay_in_the_matched_upper_bound() {
    let model = small();
    let scout = pool::Scout::new(&model, 0);
    let evidence = matched::own_points_evidence(&model, &scout);
    let cell = evidence.iter().find(|(index, _)| *index == 0).unwrap();
    assert_eq!(cell.1.samples, 0);
    assert_eq!(cell.1.interval.lower, 0.0);
    assert_eq!(cell.1.interval.upper, 1.0);
}

#[test]
fn partially_unseen_positive_mass_bins_remain_in_the_full_cell_bound() {
    let model = small();
    let mut scout = pool::Scout::new(&model, 100);
    // Put all observed seasons in the home-win point bin. The draw and loss
    // bins both have positive model mass and zero observations.
    let home_win = 3 - scout.offset;
    let home_row = (0 * scout.span + home_win as usize) * model.n;
    scout.joint[home_row] = 100;
    let away_loss = 0 - scout.offset;
    let away_row = (1 * scout.span + away_loss as usize) * model.n;
    scout.joint[away_row + 1] = 100;

    let evidence = matched::own_points_evidence(&model, &scout);
    let cell = evidence.iter().find(|(index, _)| *index == 0).unwrap();
    assert_eq!(cell.1.samples, 100);
    assert_eq!(cell.1.interval.upper, 1.0);
    assert!(cell.1.interval.upper > cell.1.interval.lower);
}
