use super::*;
use crate::{conditioned::canonical_ranks, model::Request, rng::Rng};
use serde_json::json;

fn four_team_one_residual_model() -> Model {
    let request: Request = serde_json::from_value(json!({
        "id": 59,
        "phase": {"sort":"pt,bias", "championship":{
            "point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":[
            {"team_id":0,"add_sub":0,"bias":2},
            {"team_id":1,"add_sub":2,"bias":3},
            {"team_id":2,"add_sub":2,"bias":1},
            {"team_id":3,"add_sub":0,"bias":0}],
        "games":[
            {"id":20,"home_id":0,"away_id":3,"home_power":1.1,"away_power":0.9},
            {"id":21,"home_id":1,"away_id":2,"home_power":1.1,"away_power":0.9}]
    }))
    .unwrap();
    Model::new(request).unwrap()
}

fn four_team_goal_model() -> Model {
    let request: Request = serde_json::from_value(json!({
        "id": 60,
        "phase": {"sort":"pt,gd,bias", "championship":{
            "point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":[
            {"team_id":0,"add_sub":0,"bias":2},
            {"team_id":1,"add_sub":2,"bias":3},
            {"team_id":2,"add_sub":2,"bias":1},
            {"team_id":3,"add_sub":0,"bias":0}],
        "games":[
            {"id":20,"home_id":0,"away_id":3,"home_power":1.1,"away_power":0.9},
            {"id":21,"home_id":1,"away_id":2,"home_power":1.1,"away_power":0.9}]
    }))
    .unwrap();
    Model::new(request).unwrap()
}

fn outcomes_for(m: &Model, plan: &LazyJoint, root: &[u8], residual: u8) -> Vec<u8> {
    let mut outcomes = vec![0; m.fixtures.len()];
    for (game, &outcome) in plan.target.games.iter().zip(root) {
        outcomes[game.index] = outcome;
    }
    for game in &plan.remaining {
        outcomes[game.index] = residual;
    }
    outcomes
}

fn joint_probability(m: &Model, outcomes: &[u8]) -> f64 {
    m.fixtures
        .iter()
        .enumerate()
        .map(|(index, game)| game.prob[outcomes[index] as usize])
        .product()
}

fn replay_density(pattern: &Pattern, plan: &LazyJoint, m: &Model, outcomes: &[u8]) -> f64 {
    let mut replay = outcomes.to_vec();
    let mut points = vec![0; m.n];
    let mut operations = crate::rare_tail::budget::Operations::default();
    let mut rng = Rng::new(991);
    let ratio = pattern.draw(
        plan.cell,
        &mut rng,
        &mut replay,
        &mut points,
        true,
        true,
        true,
        0,
        &mut operations,
    );
    if ratio <= 0. {
        0.
    } else {
        joint_probability(m, outcomes) / (pattern.mass * ratio)
    }
}

fn exact_rank_probability(m: &Model, cell: Cell) -> f64 {
    let mut probability = 0.;
    for a in 0..3u8 {
        for b in 0..3u8 {
            let outcomes = [a, b];
            if canonical_ranks(m, &outcomes)[cell.team] == cell.rank {
                probability += joint_probability(m, &outcomes);
            }
        }
    }
    probability
}

#[test]
fn expected_cohorts_cover_both_sides_of_a_rank_tie_and_form_a_normalized_proposal() {
    let m = four_team_one_residual_model();
    let cell = Cell { team: 0, rank: 1 };
    let mut plan = LazyJoint::with_mode(&m, cell, 609031, &[], 32, false, false, None).unwrap();
    let report = plan.add_expected_cohorts(&m, 4, 60_000, 4_000_000, 2_000_000);
    assert_eq!(report["status"], "added");
    assert!(report["cohorts_added"].as_u64().unwrap() >= 2);

    let root = vec![2u8];
    let paths = report["root_paths"].as_array().unwrap();
    let above_sets: Vec<Vec<i64>> = paths
        .iter()
        .filter(|path| path["root"] == json!(root))
        .map(|path| serde_json::from_value(path["above_team_ids"].clone()).unwrap())
        .collect();
    assert!(above_sets.iter().any(|ids| ids == &[1]));
    assert!(above_sets.iter().any(|ids| ids == &[2]));

    let native = plan.roots[&root].as_ref().unwrap();
    let proposals = &plan.families[&root];
    assert_eq!(proposals.len(), 2);
    let beta_sum: f64 = proposals.iter().map(|(_, _, beta)| *beta).sum();
    assert!((beta_sum - 1.).abs() < 1e-12);

    let mut native_mass = 0.;
    let mut family_masses = vec![0.; proposals.len()];
    let mut mixed_mass = 0.;
    let mut tied_family_density = vec![0.; proposals.len()];
    for residual in 0..3u8 {
        let outcomes = outcomes_for(&m, &plan, &root, residual);
        let prior = joint_probability(&m, &outcomes);
        let native_density = replay_density(native, &plan, &m, &outcomes);
        native_mass += native_density;
        let mut family_density = 0.;
        for (i, (_, pattern, beta)) in proposals.iter().enumerate() {
            let density = replay_density(pattern, &plan, &m, &outcomes);
            family_masses[i] += density;
            if residual == 1 {
                tied_family_density[i] = density;
            }
            family_density += beta * density;
        }
        let mixture_density = 0.2 * native_density + 0.8 * family_density;
        mixed_mass += mixture_density;
        assert!(
            mixture_density > 0.,
            "proposal lost residual outcome {residual}"
        );
        assert!(prior > 0.);
    }
    assert!(
        (native_mass - 1.).abs() < 1e-9,
        "native Q mass: {native_mass}"
    );
    for mass in family_masses {
        assert!((mass - 1.).abs() < 1e-9, "family Q mass: {mass}");
    }
    assert!(
        tied_family_density.iter().all(|density| *density > 0.),
        "the tied residual outcome must replay in both inclusive cohorts: {tied_family_density:?}"
    );
    assert!((mixed_mass - 1.).abs() < 1e-9, "mixed Q mass: {mixed_mass}");

    let exact = exact_rank_probability(&m, cell);
    let estimate = plan.sample(&m, 50_000, 7919);
    assert!(
        (estimate.probability - exact).abs() <= 6. * estimate.std_err + exact * 0.005,
        "sampled {} vs exhaustive {}, se {}",
        estimate.probability,
        exact,
        estimate.std_err
    );
}

#[test]
fn expected_order_uses_away_points_and_setup_caps_charge_a_decline() {
    let m = four_team_one_residual_model();
    let cell = Cell { team: 0, rank: 1 };
    let mut plan = LazyJoint::with_mode(&m, cell, 609031, &[], 32, false, false, None).unwrap();
    let before_roots: Vec<_> = plan
        .roots
        .iter()
        .map(|(root, pattern)| (root.clone(), pattern.as_ref().map(|p| p.mass)))
        .collect();
    let before_nodes = plan.nodes;
    let before_guides = plan.guide_values;
    let before_work = plan.order_cohort_setup_work();

    let declined = plan.add_expected_cohorts(&m, 4, 60_000, 4_000_000, 1);
    assert_eq!(declined["status"], "declined");
    assert_eq!(declined["reason"], "selection_budget");
    assert_eq!(plan.order_cohort_setup_work(), before_work + 1);
    assert_eq!(plan.nodes, before_nodes);
    assert_eq!(plan.guide_values, before_guides);
    assert_eq!(plan.family_pattern_count(), 0);
    let after_roots: Vec<_> = plan
        .roots
        .iter()
        .map(|(root, pattern)| (root.clone(), pattern.as_ref().map(|p| p.mass)))
        .collect();
    assert_eq!(before_roots, after_roots);

    let mut expected_plan =
        LazyJoint::with_mode(&m, cell, 609031, &[], 32, false, false, None).unwrap();
    let expected = expected_plan.add_expected_cohorts(&m, 4, 60_000, 4_000_000, 2_000_000);
    let root = vec![2u8];
    let residual = &expected_plan.remaining[0];
    let home_win = residual.prob[2];
    let draw = residual.prob[1];
    let away_win = residual.prob[0];
    let expected_scores = super::expected_raw_points(
        &m,
        &expected_plan.target.games,
        &expected_plan.remaining,
        &root,
    );
    let expected_team_1 = 2. + draw + 3. * home_win;
    let expected_team_2 = 2. + draw + 3. * away_win;
    assert!((expected_scores[1] - expected_team_1).abs() < 1e-12);
    assert!((expected_scores[2] - expected_team_2).abs() < 1e-12);
    assert!(expected_team_1 > expected_team_2);
    let primary_above: Vec<i64> = serde_json::from_value(
        expected["root_paths"]
            .as_array()
            .unwrap()
            .iter()
            .find(|path| path["root"] == json!(root))
            .unwrap()["above_team_ids"]
            .clone(),
    )
    .unwrap();
    assert_eq!(
        primary_above,
        vec![1],
        "home team 1 has greater expected points than away team 2"
    );
}

#[test]
fn native_base_is_explicitly_full_support_and_pilot_respects_its_work_cap() {
    let m = four_team_one_residual_model();
    let cell = Cell { team: 0, rank: 1 };
    let (plan, charged, diagnostics) = LazyJoint::new_order_base(&m, cell, 609031, usize::MAX);
    let plan = plan.expect("the small supported model should build");
    assert!(charged > 0);
    assert_eq!(diagnostics["native_width"], json!(4));
    assert!(!plan.omit);
    assert!(!plan.subset);
    assert_eq!(plan.rival_limit, 4);

    for limit in [0, 1] {
        let capped = plan.sample_order_pilot(&m, 512, 991, limit);
        assert_eq!(capped.samples, 0);
        assert!(capped.operations.units() <= limit);
    }
    let (declined, charged, decline_report) = LazyJoint::new_order_base(&m, cell, 609031, 0);
    assert!(declined.is_none());
    assert_eq!(charged, 0);
    assert!(decline_report["requested_allowance"].as_u64().unwrap() > 0);
}

#[test]
fn overlapping_expected_cohorts_can_be_sampled_within_the_pilot_grant() {
    let m = four_team_one_residual_model();
    let cell = Cell { team: 0, rank: 1 };
    let mut plan = LazyJoint::with_mode(&m, cell, 609031, &[], 32, false, false, None).unwrap();
    let report = plan.add_expected_cohorts(&m, 4, 60_000, 4_000_000, 2_000_000);
    assert_eq!(report["status"], "added");
    assert!(plan.families.values().any(|families| families.len() > 1));

    let requested = 12;
    let grant = crate::rare_tail::budget::ordered_draw_upper_cost(&m);
    let pilot = plan.sample_order_pilot(&m, requested, 991, grant);
    assert!(pilot.samples > 0);
    assert!(
        pilot.samples < requested,
        "grant should stop the pilot early"
    );
    assert!(pilot.operations.units() <= grant);
}

#[test]
fn cold_goal_tilt_preparation_is_reserved_inside_the_pilot_grant() {
    let m = four_team_goal_model();
    let cell = Cell { team: 0, rank: 1 };
    let mut plan = LazyJoint::with_mode(&m, cell, 609031, &[], 32, false, false, None).unwrap();
    let goals = crate::goal_tilt::GoalTilt::new(&m, cell).expect("pt/gd model is guidable");

    // Replaying one root outcome twice in a shared context makes the first
    // call pay for a cold joint proposal and the second use the cached proposal.
    let outcome = vec![0; m.fixtures.len()];
    let mut context = crate::goal_tilt::Context::with_sample_budget(&m, 8);
    let mut rng = Rng::new(7331);
    let mut cold_operations = crate::rare_tail::budget::Operations::default();
    goals.rank_with_operations(&m, &outcome, &mut rng, &mut context, &mut cold_operations);
    let mut warm_operations = crate::rare_tail::budget::Operations::default();
    goals.rank_with_operations(&m, &outcome, &mut rng, &mut context, &mut warm_operations);
    assert!(
        cold_operations.units() > warm_operations.units(),
        "first GoalTilt call should include joint-proposal preparation work: cold={}, warm={}",
        cold_operations.units(),
        warm_operations.units()
    );
    plan.goals = Some(goals);

    let requested = 8;
    let grant = requested * crate::goal_tilt::JOINT_PREPARATION_ALLOWANCE_PER_DRAW
        + crate::rare_tail::budget::ordered_draw_upper_cost(&m);
    let pilot = plan.sample_order_pilot(&m, requested, 991, grant);
    assert!(pilot.samples > 0);
    assert!(pilot.operations.units() <= grant);
}

#[test]
fn dynamic_propagation_pilot_counts_rejections_and_stays_within_its_grant() {
    const CHILD: &str = "RUST_ODDS_ORDER_COHORT_PROPAGATION_CHILD";
    if std::env::var_os(CHILD).as_deref() != Some(std::ffi::OsStr::new("1")) {
        let test_module = module_path!()
            .split_once("::")
            .map_or(module_path!(), |(_, module)| module);
        let test_name = format!(
            "{test_module}::dynamic_propagation_pilot_counts_rejections_and_stays_within_its_grant"
        );
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(test_name)
            .arg("--nocapture")
            .arg("--test-threads=1")
            .env(CHILD, "1")
            .env("RUST_ODDS_LAZY_PROPAGATE", "1")
            .output()
            .expect("launch isolated dynamic propagation test process");
        assert!(
            output.status.success(),
            "isolated dynamic propagation test failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("running 1 test"),
            "child test binary did not run exactly one test:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let m = four_team_one_residual_model();
    let cell = Cell { team: 0, rank: 0 };
    let plan = LazyJoint::with_mode(&m, cell, 609031, &[], 32, false, false, None).unwrap();
    let samples = 64;
    let grant = samples * crate::rare_tail::budget::ordered_draw_upper_cost(&m);
    let pilot = plan.sample_order_pilot(&m, samples, 991, grant);

    assert!(pilot.samples > 0);
    assert!(pilot.operations.units() <= grant);
    assert!(
        pilot.samples > pilot.hits,
        "rank-rejected draws must count as samples"
    );
}
