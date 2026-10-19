//! Correctness checks for the learned root/rival-family proposal.
use super::*;
use crate::{conditioned::canonical_ranks, model::Request};
use serde_json::json;

fn three_team_model() -> Model {
    let request: Request = serde_json::from_value(json!({
        "id": 57,
        "phase": {"sort":"pt,w,bias", "championship":{
            "point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":[
            {"team_id":0,"add_sub":3,"bias":1},
            {"team_id":1,"add_sub":2,"bias":2},
            {"team_id":2,"add_sub":2,"bias":0}],
        "games":[
            {"id":10,"home_id":0,"away_id":1,"home_power":1.1,"away_power":0.9},
            {"id":11,"home_id":0,"away_id":2,"home_power":1.1,"away_power":0.9},
            {"id":12,"home_id":1,"away_id":2,"home_power":1.1,"away_power":0.9}]
    }))
    .unwrap();
    Model::new(request).unwrap()
}

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

fn goal_tilt_model() -> Model {
    let request: Request = serde_json::from_value(json!({
        "id": 58,
        "phase": {"sort":"pt,w,gd,bias", "championship":{
            "point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":(0..4).map(|t| json!({"team_id":t,"add_sub":0,"bias":if t == 0 {10} else {0}})).collect::<Vec<_>>(),
        "games":[
            {"id":1,"home_id":0,"away_id":1,"home_power":1.2,"away_power":0.8},
            {"id":2,"home_id":2,"away_id":3,"home_power":1.2,"away_power":0.8}]
    }))
    .unwrap();
    let mut model = Model::new(request).unwrap();
    model.base[0].ga = 8;
    model
}

fn assert_same_result(left: &Result, right: &Result) {
    assert_eq!(left.mass.to_bits(), right.mass.to_bits());
    assert_eq!(left.samples, right.samples);
    assert_eq!(left.hits, right.hits);
    assert_eq!(left.blockers, right.blockers);
    assert_eq!(left.work, right.work);
    assert_eq!(left.operations.fixtures, right.operations.fixtures);
    assert_eq!(left.operations.guidance, right.operations.guidance);
    assert_eq!(left.operations.ranking, right.operations.ranking);
    assert_eq!(left.weighted, right.weighted);
    assert_eq!(left.probability.to_bits(), right.probability.to_bits());
    assert_eq!(left.std_err.to_bits(), right.std_err.to_bits());
    assert_eq!(left.ess.to_bits(), right.ess.to_bits());
    assert_eq!(left.max_share.to_bits(), right.max_share.to_bits());
    assert_eq!(left.batch_gap.to_bits(), right.batch_gap.to_bits());
    assert_eq!(left.witness, right.witness);
    assert_eq!(left.omitted_draws, right.omitted_draws);
}

fn rank_state(
    plan: &LazyJoint,
    m: &Model,
    root: &[u8],
    residual_outcome: u8,
) -> (Vec<u8>, Vec<i8>) {
    let mut outcomes = vec![0; m.fixtures.len()];
    for (g, &o) in plan.target.games.iter().zip(root) {
        outcomes[g.index] = o;
    }
    for g in &plan.remaining {
        outcomes[g.index] = residual_outcome;
    }
    let totals = {
        let mut totals = plan.base.clone();
        for g in plan.target.games.iter().chain(&plan.remaining) {
            let o = outcomes[g.index] as usize;
            totals[g.home] += g.hg[o];
            totals[g.away] += g.ag[o];
        }
        totals
    };
    let target = totals[plan.cell.team];
    let state = (0..m.n)
        .filter(|&t| t != plan.cell.team)
        .map(|t| match totals[t].cmp(&target) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        })
        .collect();
    (outcomes, state)
}

#[test]
fn family_mixture_matches_closed_form_for_two_learned_and_one_omitted_state() {
    let m = four_team_one_residual_model();
    let cell = Cell { team: 0, rank: 1 };
    let mut plan = LazyJoint::with_mode(&m, cell, 1811, &[], 16, false, false, None).unwrap();
    let root = plan
        .roots
        .keys()
        .find(|root| root.as_slice() == [2])
        .unwrap()
        .clone();
    let rows: Vec<_> = (0..3).map(|o| rank_state(&plan, &m, &root, o)).collect();
    assert_eq!(plan.remaining.len(), 1);
    assert!(rows
        .iter()
        .all(|(outcomes, _)| canonical_ranks(&m, outcomes)[0] == 1));
    assert_eq!(rows[0].1, vec![-1, 1, -1]);
    assert_eq!(rows[1].1, vec![0, 0, -1]);
    assert_eq!(rows[2].1, vec![1, -1, -1]);

    let mut moments = FamilyMoments::default();
    moments
        .root_contributions
        .insert((root.clone(), rows[0].1.clone()), 2.);
    moments
        .root_contributions
        .insert((root.clone(), rows[2].1.clone()), 1.);
    let (_, _, skipped) = plan.fit_families(&moments);
    assert_eq!(skipped, 0);
    assert_eq!(plan.family_pattern_count(), 2);

    let primary = plan.roots[&root].as_ref().unwrap();
    let learned = &plan.families[&root];
    let root_probability: f64 = plan
        .target
        .games
        .iter()
        .zip(&root)
        .map(|(g, &o)| g.prob[o as usize])
        .product();
    let mut exact_event_mass = 0.;
    for mut code in 0..9usize {
        let mut outcomes = vec![0; m.fixtures.len()];
        let mut prior = 1.;
        for g in plan.target.games.iter().chain(&plan.remaining) {
            let o = code % 3;
            code /= 3;
            outcomes[g.index] = o as u8;
            prior *= g.prob[o];
        }
        if canonical_ranks(&m, &outcomes)[cell.team] == cell.rank {
            exact_event_mass += prior;
        }
    }
    assert!((exact_event_mass - root_probability).abs() < 1e-12);
    let p = plan.remaining[0].prob;
    let guide_weight = [1., 0.5, 1.];
    let guide_z: f64 = (0..3).map(|o| p[o] * guide_weight[o]).sum();
    let native_q: Vec<_> = (0..3)
        .map(|o| (1. - BASE_SHARE) * p[o] * guide_weight[o] / guide_z + BASE_SHARE * p[o])
        .collect();
    let beta = [2. / 3., 0., 1. / 3.];
    let mut prior_event_mass = 0.;
    let mut proposal_mass = 0.;

    for (index, (outcomes, state)) in rows.iter().enumerate() {
        let outcome = p[index];
        prior_event_mass += outcome;
        let mut points = vec![0; m.n];
        let mut operations = crate::rare_tail::budget::Operations::default();
        let mut rng = Rng::new(41);
        let mut replay = outcomes.clone();
        let native_ratio = primary.draw(
            cell,
            &mut rng,
            &mut replay,
            &mut points,
            true,
            true,
            true,
            0,
            &mut operations,
        );
        assert!(native_ratio > 0., "native proposal must cover {state:?}");
        let native_density = p[index] * root_probability / (primary.mass * native_ratio);
        assert!(
            (native_density - native_q[index]).abs() < 1e-8,
            "native replay density disagrees with closed form for {state:?}"
        );

        let mut family_density = 0.;
        for (family_state, pattern, weight) in learned {
            let mut replay = outcomes.clone();
            let mut points = vec![0; m.n];
            let mut operations = crate::rare_tail::budget::Operations::default();
            let mut rng = Rng::new(43);
            let ratio = pattern.draw(
                cell,
                &mut rng,
                &mut replay,
                &mut points,
                true,
                true,
                true,
                0,
                &mut operations,
            );
            if family_state == state {
                assert!(ratio > 0.);
                let component_density = p[index] * root_probability / (pattern.mass * ratio);
                assert!((component_density - 1.).abs() < 1e-10);
                family_density += weight * component_density;
            } else {
                assert_eq!(ratio, 0., "other family must reject this state");
            }
        }
        let expected_family = beta[index];
        assert!((family_density - expected_family).abs() < 1e-12);
        let expected = 0.2 * native_q[index] + 0.8 * expected_family;
        let observed = 0.2 * native_density + 0.8 * family_density;
        assert!((observed - expected).abs() < 1e-10);
        proposal_mass += observed;
        assert!(observed > 0., "proposal lost support for {state:?}");
    }
    assert!((prior_event_mass - 1.).abs() < 1e-12);
    assert!((proposal_mass - 1.).abs() < 1e-10);
    assert!(root_probability > 0.);
}

#[test]
fn empty_family_training_leaves_native_root_support_unchanged() {
    let m = three_team_model();
    let cell = Cell { team: 0, rank: 1 };
    let mut plan = LazyJoint::with_mode(&m, cell, 31, &[], 32, false, false, None).unwrap();
    let before: Vec<_> = plan
        .roots
        .iter()
        .map(|(root, pattern)| (root.clone(), pattern.as_ref().map(|p| p.mass)))
        .collect();
    assert!(!before.is_empty());
    assert_eq!(plan.fit_families(&FamilyMoments::default()), (0, 0, 0));
    assert_eq!(plan.family_pattern_count(), 0);
    let after: Vec<_> = plan
        .roots
        .iter()
        .map(|(root, pattern)| (root.clone(), pattern.as_ref().map(|p| p.mass)))
        .collect();
    assert_eq!(before, after);
    assert!(plan.sample(&m, 100, 29).probability >= 0.);
}

#[test]
fn full_family_fit_keeps_the_legacy_top_four_ranked_candidates() {
    let m = four_team_one_residual_model();
    let cell = Cell { team: 0, rank: 1 };
    let mut plan = LazyJoint::with_mode(&m, cell, 1821, &[], 16, false, false, None).unwrap();
    let root = plan.roots.keys().next().unwrap().clone();
    let mut moments = FamilyMoments::default();
    let mut ranked = Vec::new();
    for a in -1..=1 {
        for b in -1..=1 {
            for c in -1..=1 {
                let state = vec![a, b, c];
                let weight = (ranked.len() + 1) as f64;
                moments
                    .root_contributions
                    .insert((root.clone(), state.clone()), weight);
                ranked.push((root.clone(), state, weight));
            }
        }
    }
    ranked.sort_by(|(ra, sa, wa), (rb, sb, wb)| wb.total_cmp(wa).then(ra.cmp(rb)).then(sa.cmp(sb)));
    ranked.truncate(4);

    plan.fit_families_capped(&moments, 10_000, 4_000_000);

    assert!(plan.family_pattern_count() <= 4);
    let attempted: Vec<_> = plan
        .family_fit_summary()
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                serde_json::from_value::<Vec<u8>>(entry["root_outcomes"].clone()).unwrap(),
                serde_json::from_value::<Vec<i8>>(entry["rival_status"].clone()).unwrap(),
            )
        })
        .collect();
    let expected: Vec<_> = ranked
        .into_iter()
        .map(|(root, state, _)| (root, state))
        .collect();
    assert_eq!(attempted, expected);
}

#[test]
fn impossible_family_bounds_decline_without_changing_native_root_support() {
    let m = four_team_one_residual_model();
    let cell = Cell { team: 0, rank: 1 };
    let mut plan = LazyJoint::with_mode(&m, cell, 37, &[], 16, false, false, None).unwrap();
    let root = plan
        .roots
        .keys()
        .find(|root| root.as_slice() == [2])
        .unwrap()
        .clone();
    let before: Vec<_> = plan
        .roots
        .iter()
        .map(|(key, pattern)| (key.clone(), pattern.as_ref().map(|p| p.mass)))
        .collect();
    let mut moments = FamilyMoments::default();
    // The dummy is fixed at zero, so this full packed status cannot occur.
    moments
        .root_contributions
        .insert((root.clone(), vec![1, 1, 1]), 1.);
    let (_, _, skipped) = plan.fit_families(&moments);
    assert_eq!(skipped, 1);
    assert_eq!(plan.family_pattern_count(), 0);
    let after: Vec<_> = plan
        .roots
        .iter()
        .map(|(key, pattern)| (key.clone(), pattern.as_ref().map(|p| p.mass)))
        .collect();
    assert_eq!(before, after);
    assert!(plan.roots[&root].as_ref().unwrap().mass > 0.);
}

#[test]
fn native_family_observer_is_bounded_deterministic_and_rng_neutral() {
    let m = three_team_model();
    let cell = Cell { team: 0, rank: 1 };
    let plan = LazyJoint::with_mode(&m, cell, 817, &[], 24, false, false, None).unwrap();
    let samples = 5_000;
    let baseline = plan.sample(&m, samples, 921);
    let (observed, moments) = plan.sample_family_native_training(&m, samples, 921, 1_000, 25);
    assert_same_result(&baseline, &observed);
    assert_eq!(moments.observed_prefix_draws, 1_000);
    assert_eq!(moments.positive_observations_considered, 25);
    assert!(moments.recorded_positive_observations > 0);
    assert!(moments.recorded_positive_observations <= 25);
    assert!(moments.root_contributions.len() <= 128);
    assert!(moments.contributions.len() <= 128);
    assert!(moments.recorded_observation_ess > 0.);
    assert!(moments.collection_work > 0);
    let bound = LazyJoint::native_family_recording_bound(&m, samples, 1_000, 25).unwrap();
    assert!(moments.collection_work <= bound);

    let (_, repeated) = plan.sample_family_native_training(&m, samples, 921, 1_000, 25);
    assert_eq!(
        moments.observed_prefix_draws,
        repeated.observed_prefix_draws
    );
    assert_eq!(
        moments.positive_observations_considered,
        repeated.positive_observations_considered
    );
    assert_eq!(
        moments.recorded_positive_observations,
        repeated.recorded_positive_observations
    );
    assert_eq!(
        moments.skipped_key_observations,
        repeated.skipped_key_observations
    );
    assert_eq!(moments.collection_work, repeated.collection_work);
    assert_eq!(
        moments.recorded_observation_ess.to_bits(),
        repeated.recorded_observation_ess.to_bits()
    );
    assert_eq!(moments.root_contributions, repeated.root_contributions);
    assert_eq!(moments.contributions, repeated.contributions);
}

#[test]
fn native_family_observer_zero_caps_are_plain_sampling_and_overflow_is_declined() {
    let m = four_team_one_residual_model();
    let cell = Cell { team: 0, rank: 1 };
    let plan = LazyJoint::with_mode(&m, cell, 37, &[], 16, false, false, None).unwrap();
    let baseline = plan.sample(&m, 128, 729);
    for (draw_cap, positive_cap) in [(0, 10), (10, 0)] {
        let (observed, moments) =
            plan.sample_family_native_training(&m, 128, 729, draw_cap, positive_cap);
        assert_same_result(&baseline, &observed);
        assert_eq!(moments.observed_prefix_draws, 0);
        assert_eq!(moments.positive_observations_considered, 0);
        assert_eq!(moments.recorded_positive_observations, 0);
        assert_eq!(moments.recorded_observation_ess, 0.);
        assert_eq!(moments.collection_work, 0);
        assert!(moments.root_contributions.is_empty());
    }
    assert_eq!(
        LazyJoint::native_family_recording_bound(&m, 128, 0, 10),
        Some(0)
    );
    assert_eq!(
        LazyJoint::native_family_recording_bound(&m, 128, 10, 0),
        Some(0)
    );
    assert_eq!(
        LazyJoint::native_family_recording_bound(&m, usize::MAX, 10, 10),
        None
    );
    let (oversized, moments) =
        plan.sample_family_native_training(&m, MAX_NATIVE_FAMILY_SAMPLES + 1, 729, 10, 10);
    assert_eq!(oversized.samples, MAX_NATIVE_FAMILY_SAMPLES + 1);
    assert_eq!(moments.collection_work, 0);
    assert!(moments.root_contributions.is_empty());
}

#[test]
fn goal_tilt_uses_production_rank_and_has_nontrivial_score_likelihood_ratio() {
    let m = goal_tilt_model();
    let tilt = crate::goal_tilt::GoalTilt::new(&m, Cell { team: 0, rank: 0 }).unwrap();
    let outcomes = vec![2, 2];
    let mut context = crate::goal_tilt::Context::new(&m);
    let mut found = None;
    for seed in 0..32 {
        let (rank, ratio) = tilt.rank(&m, &outcomes, &mut Rng::new(seed), &mut context);
        assert!(rank < m.n);
        if (ratio - 1.).abs() > 1e-8 {
            found = Some((rank, ratio));
            break;
        }
    }
    let (rank, ratio) = found.expect("score tilt should produce a non-unit P/Q ratio");
    assert!(rank <= 1, "goal path should retain a plausible target rank");
    assert!(ratio.is_finite() && ratio > 0.);
}
#[test]
fn goal_tilt_family_recorder_preserves_results_and_collects_corrected_weights() {
    let m = goal_tilt_model();
    let cell = Cell { team: 0, rank: 0 };
    let mut plan = LazyJoint::with_mode(&m, cell, 1819, &[], 24, false, false, None).unwrap();
    plan.goals = Some(crate::goal_tilt::GoalTilt::new(&m, cell).unwrap());

    let samples = 128;
    let seed = 2_927;
    let baseline = plan.sample(&m, samples, seed);
    let (observed, moments) = plan.sample_family_native_training(&m, samples, seed, samples, 128);
    assert_same_result(&baseline, &observed);
    assert!(
        baseline.hits > 0,
        "the goal-aware target event should be sampled"
    );
    assert_eq!(moments.observed_prefix_draws, samples);
    assert_eq!(moments.positive_observations_considered, baseline.hits);
    assert_eq!(moments.recorded_positive_observations, baseline.hits);
    assert_eq!(moments.skipped_key_observations, 0);
    assert!(moments.root_contributions.len() <= 128);

    let corrected_weight: f64 = moments.root_contributions.values().sum();
    let expected_weight = baseline.probability * samples as f64;
    assert!(
        (corrected_weight - expected_weight).abs() <= 1e-10 * expected_weight.abs().max(1.),
        "recorded goal-corrected contributions should match the native result: {corrected_weight} vs {expected_weight}"
    );
    assert!(
        (corrected_weight - baseline.hits as f64).abs() > 1e-8,
        "goal score likelihood ratios should affect the recorded contributions"
    );
}
