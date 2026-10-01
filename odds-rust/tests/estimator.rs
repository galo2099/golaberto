use golaberto_odds::{
    conditioned::{self, Event, Result},
    domains::Domains,
    lookahead::{self, Policy, RankGame},
    model::{Model, Request},
    pool::{self, Bounds},
    proof::Problem,
    rng::Rng,
};
use serde_json::{json, Value};
fn tiny() -> Model {
    let mut games = Vec::new();
    for h in 0..4 {
        for a in h + 1..4 {
            games.push(json!({"id":games.len(),"home_id":h,"away_id":a,"home_score":null,"away_score":null,"home_power":1.3,"away_power":0.9,"played":false}));
        }
    }
    let request:Request=serde_json::from_value(json!({"id":1,"phase":{"sort":"pt,w,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":(0..4).map(|t|json!({"team_id":t,"bias":t,"add_sub":0})).collect::<Vec<_>>(),"games":games})).unwrap();
    Model::new(request).unwrap()
}
fn enumerate(model: &Model, mut visit: impl FnMut(&[u8], f64)) {
    let mut outcomes = vec![0; model.fixtures.len()];
    for mut code in 0..3usize.pow(outcomes.len() as u32) {
        let mut p = 1.;
        for (i, o) in outcomes.iter_mut().enumerate() {
            *o = (code % 3) as u8;
            code /= 3;
            p *= model.fixtures[i].prob[*o as usize];
        }
        visit(&outcomes, p);
    }
}
fn reference_model() -> Model {
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16653-71d4fea8.json");
    Model::new(serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()).unwrap()
}
#[test]
fn go_rng_and_scout_are_identical() {
    let model = reference_model();
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/go-scout-16653-808.json")).unwrap();
    let mut rng = Rng::new(808);
    for v in expected["floats"].as_array().unwrap() {
        assert_eq!(rng.float(), v.as_f64().unwrap());
    }
    let scout = pool::scout(&model, 1000, 808);
    for t in 0..model.n {
        let expected = &expected["scout"][model.ids[t].to_string()];
        assert_eq!(
            serde_json::to_value(&scout.ranks[t * model.n..(t + 1) * model.n]).unwrap(),
            expected["rank_counts"]
        );
        for (added, row) in expected["point_rank_counts"].as_object().unwrap() {
            assert_eq!(
                serde_json::to_value(scout.row(t, added.parse().unwrap(), model.n)).unwrap(),
                *row
            );
        }
    }
}
#[test]
fn conditional_event_contains_every_rank_hit_and_has_exact_mass() {
    let model = tiny();
    let bounds = Bounds::new(&model);
    for target in 0..4 {
        for rank in 0..4 {
            let blockers: Vec<_> = (0..4).filter(|t| *t != target).take(2).collect();
            let event = Event::build(&model, &bounds, target, rank, &blockers, 20000).unwrap();
            let mut mass = 0.;
            enumerate(&model, |outcomes, p| {
                let mut added = [0; 6];
                let rules = &model.request.phase.championship;
                for (i, f) in model.fixtures.iter().enumerate() {
                    let o = outcomes[i] as usize;
                    for (slot, &t) in event.teams.iter().enumerate() {
                        if t == f.home {
                            added[slot] += [rules.point_loss, rules.point_draw, rules.point_win][o];
                        }
                        if t == f.away {
                            added[slot] += [rules.point_win, rules.point_draw, rules.point_loss][o];
                        }
                    }
                }
                let state = added
                    .iter()
                    .enumerate()
                    .fold(0u64, |s, (i, p)| s | ((*p as u64) << (8 * i)));
                let included = event.terminals.iter().any(|(s, _)| *s == state);
                if conditioned::canonical_ranks(&model, outcomes)[target] == rank {
                    assert!(included, "lost feasible outcome {outcomes:?}");
                }
                if included {
                    mass += p;
                }
            });
            assert!((mass - event.mass).abs() < 1e-12);
        }
    }
}
#[test]
fn cap_and_floor_proofs_never_exclude_a_feasible_relaxation() {
    let model = tiny();
    let problem = Problem::new(&model).unwrap().with_discrete_cuts(true);
    for dual in [false, true] {
        let negative = problem.negated();
        let problem = if dual { &negative } else { &problem };
        for target in 0..4 {
            for rank in 0..4 {
                for cap in -10..=10 {
                    let mut feasible = false;
                    enumerate(&model, |o, _| {
                        let mut points = problem.base.clone();
                        for (i, g) in problem.games.iter().enumerate() {
                            points[g.home] += g.hg[o[i] as usize];
                            points[g.away] += g.ag[o[i] as usize];
                        }
                        if (0..4).filter(|t| *t != target && points[*t] > cap).count() <= rank {
                            feasible = true;
                        }
                    });
                    let (impossible, _) = problem.impossible(target, rank, cap, 10000);
                    if feasible {
                        assert!(
                            !impossible,
                            "false proof target={target} rank={rank} cap={cap} dual={dual}"
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn propagated_domains_preserve_all_rank_hits() {
    let model = tiny();
    let rules = &model.request.phase.championship;
    let stride = conditioned::stride(&model, "lookahead");
    let hg = [
        rules.point_loss * stride,
        rules.point_draw * stride,
        rules.point_win * stride + 1,
    ];
    let ag = [hg[2], hg[1], hg[0]];
    for target in 0..4 {
        let remaining: Vec<_> = model
            .fixtures
            .iter()
            .enumerate()
            .filter(|(_, f)| f.home != target && f.away != target)
            .map(|(i, f)| RankGame {
                index: i,
                home: f.home,
                away: f.away,
                hg,
                ag,
                prob: f.prob,
            })
            .collect();
        let rivals: Vec<_> = (0..4).filter(|t| *t != target).collect();
        enumerate(&model, |o, _| {
            let rank = conditioned::canonical_ranks(&model, o)[target];
            let mut points = vec![0; 4];
            for (i, f) in model.fixtures.iter().enumerate() {
                if f.home == target || f.away == target {
                    points[f.home] += hg[o[i] as usize];
                    points[f.away] += ag[o[i] as usize];
                }
            }
            let domains = Domains::propagate(&remaining, &points, &rivals, rank, points[target]);
            assert!(domains.feasible);
            for (step, g) in remaining.iter().enumerate() {
                let outcome = o[g.index] as usize;
                assert!(domains.allows(step, g, outcome, &points));
                points[g.home] += g.hg[outcome];
                points[g.away] += g.ag[outcome];
            }
        });
    }
}
#[test]
fn weighted_sampler_agrees_with_exhaustive_probability() {
    let model = tiny();
    let bounds = Bounds::new(&model);
    let mut exact = [0.; 4];
    enumerate(&model, |o, p| {
        let rank = conditioned::canonical_ranks(&model, o)[0];
        exact[rank] += p;
    });
    for rank in 0..4 {
        let event = Event::build(&model, &bounds, 0, rank, &[], 20000).unwrap();
        for propagate in [false, true] {
            let result = lookahead::sample(
                &model,
                &event,
                0,
                rank,
                &bounds,
                Policy {
                    samples: 30000,
                    seed: 808,
                    tilt: 3.,
                    point_tilt: 0.5,
                    force_points: false,
                    propagate,
                },
            );
            assert!(result.weighted);
            assert!(
                (result.probability - exact[rank]).abs() < 6. * result.std_err + 1e-6,
                "rank={rank} propagate={propagate} estimate={} exact={} se={}",
                result.probability,
                exact[rank],
                result.std_err
            );
        }
    }
}
#[test]
fn proof_guided_sampler_agrees_with_exhaustive_probability() {
    let model = tiny();
    let bounds = Bounds::new(&model);
    let mut exact = [0.; 4];
    enumerate(&model, |o, p| {
        exact[conditioned::canonical_ranks(&model, o)[0]] += p
    });
    for rank in 0..4 {
        let event = Event::build(&model, &bounds, 0, rank, &[], 20000).unwrap();
        let result = lookahead::sample_with_probes(
            &model,
            &event,
            0,
            rank,
            &bounds,
            Policy {
                samples: 30000,
                seed: 808,
                tilt: 3.,
                point_tilt: 0.5,
                force_points: false,
                propagate: true,
            },
            golaberto_odds::domains::ProbeConfig {
                checks: 24,
                nodes: 32,
            },
        );
        assert!(result.weighted);
        assert!(
            (result.probability - exact[rank]).abs() < 6. * result.std_err + 1e-6,
            "rank={rank} estimate={} exact={}",
            result.probability,
            exact[rank]
        );
    }
}

#[test]
fn production_acceptance_gate_is_unchanged() {
    let mut result = Result {
        weighted: true,
        probability: 1e-12,
        std_err: 0.349e-12,
        hits: 30,
        ess: 8.,
        max_share: 0.25,
        batch_gap: 1.,
        ..Default::default()
    };
    assert!(result.coarse());
    result.std_err = 0.351e-12;
    assert!(!result.coarse());
    result.std_err = 0.2e-12;
    result.ess = 7.99;
    assert!(!result.coarse());
}
#[test]
fn matrix_balances_with_structural_zeros() {
    let mut matrix = vec![0.7, 0.3, 0., 0.2, 0.5, 0.3, 0., 0.2, 0.8];
    assert!(pool::balance(&mut matrix, 3));
    assert_eq!(matrix[2], 0.);
    assert_eq!(matrix[6], 0.);
    assert!(pool::valid(&matrix, 3));
}
#[test]
fn rails_nulls_and_extra_fields_are_accepted() {
    let request:Request=serde_json::from_value(json!({"id":1,"name":"test","phase":{"sort":"pt,w","bonus_points":null,"championship":{"point_win":3,"point_draw":1,"point_loss":null}},
        "team_groups":[{"team_id":1,"add_sub":null,"bias":null},{"team_id":2}],"games":[{"id":1,"home_id":1,"away_id":2,"home_score":null,"away_score":null,"home_power":1.,"away_power":1.,"played":false}]})).unwrap();
    assert_eq!(request.games[0].home_score, 0);
    assert_eq!(request.team_groups[0].bias, 0);
    assert!(Model::new(request).is_ok());
}

#[test]
fn non_points_standings_keep_plain_mc_uncertainty_and_skip_rare_search() {
    let mut request = tiny().request;
    request.phase.sort = "w,pt,bias".into();
    let model = Model::new(request).unwrap();
    let mut estimates = pool::production(&model, 808, 4);
    assert!(estimates.iter().all(|e| e.design == "plain_mc"));
    for e in &estimates {
        assert_eq!(e.ess, e.hits as f64);
        assert!((e.std_err.powi(2) - e.probability * (1. - e.probability) / 99999.).abs() < 1e-18);
        if e.hits > 0 {
            assert_eq!(e.max_event_weight_share, 1. / e.hits as f64);
        }
    }
    assert_eq!(
        golaberto_odds::search::run(&model, 808, 4, &mut estimates),
        0
    );
}

#[test]
fn packed_point_overflow_skips_conditioning_instead_of_losing_support() {
    let mut request = tiny().request;
    request.phase.championship.point_win = 100;
    let model = Model::new(request).unwrap();
    assert!(Event::build(&model, &Bounds::new(&model), 0, 0, &[], 20000).is_none());
}

#[test]
fn joint_rank_propagation_preserves_every_enumerated_witness() {
    for sort in ["pt,w,bias", "pt,gd,w,bias"] {
        let mut request = tiny().request;
        request.phase.sort = sort.into();
        let model = Model::new(request).unwrap();
        let problem = golaberto_odds::reachability::JointProblem::new(&model).unwrap();
        enumerate(&model, |outcomes, _| {
            let ranks = conditioned::canonical_ranks(&model, outcomes);
            for target in 0..4 {
                for prefix in [0, 2, 4, 6] {
                    let mut domains = problem.initial_domains();
                    for i in 0..prefix {
                        domains[i] = 1 << outcomes[i];
                    }
                    assert!(
                        problem
                            .propagate(target, ranks[target], &mut domains)
                            .is_some(),
                        "{sort} lost {outcomes:?}, target {target}, prefix {prefix}"
                    );
                    for i in 0..outcomes.len() {
                        assert_ne!(domains[i] & (1 << outcomes[i]), 0);
                    }
                }
            }
        });
    }
}

#[test]
fn joint_score_failure_and_budget_exhaustion_remain_undecided() {
    let request:Request=serde_json::from_value(json!({"id":1,"phase":{"sort":"pt,w,gd,gf","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":[{"team_id":1},{"team_id":2}],"games":[
        {"id":0,"home_id":2,"away_id":1,"home_score":5,"away_score":0,"played":true},
        {"id":1,"home_id":1,"away_id":2,"home_power":1.,"away_power":1.,"played":false}]})).unwrap();
    let model = Model::new(request).unwrap();
    let cell = golaberto_odds::search::Cell { team: 0, rank: 0 };
    let mut estimates = vec![
        pool::Estimate {
            reachability: "undecided".into(),
            ..Default::default()
        };
        4
    ];
    let run = golaberto_odds::reachability::run(&model, &[cell], &mut estimates, 100, 3200);
    assert_eq!(run.stats.impossible, 0);
    assert_eq!(estimates[0].reachability, "undecided");
    // The same WDL pattern really is reachable with different goals.
    let mut campaign = model.base.clone();
    let mut scores = model.empty_scores();
    scores[1] = [6, 0];
    model.add(&mut campaign, 0, [6, 0]);
    let mut order = vec![0; 2];
    model.standings(&mut order, &campaign, &scores, &mut Rng::new(1));
    assert_eq!(order[0], 0);
    let mut estimates = vec![
        pool::Estimate {
            reachability: "undecided".into(),
            ..Default::default()
        };
        4
    ];
    let run = golaberto_odds::reachability::run(&model, &[cell], &mut estimates, 0, 0);
    assert_eq!(run.stats.impossible, 0);
    assert_eq!(estimates[0].reachability, "undecided");
}

#[test]
fn joint_verified_seasons_are_reused_and_never_become_estimates() {
    let model = tiny();
    let cells: Vec<_> = (0..4)
        .flat_map(|team| (0..4).map(move |rank| golaberto_odds::search::Cell { team, rank }))
        .collect();
    let mut estimates = vec![
        pool::Estimate {
            reachability: "undecided".into(),
            ..Default::default()
        };
        16
    ];
    let result =
        golaberto_odds::reachability::run_ordered(&model, &cells, &mut estimates, 100, 3200, true);
    assert!(result.stats.reachable > 0);
    assert!(result.stats.nodes <= 3200);
    for (i, e) in estimates.iter().enumerate() {
        assert_eq!(e.probability, 0.);
        if e.reachability == "reachable_by_construction" {
            assert!(result
                .outcomes
                .iter()
                .any(|o| conditioned::canonical_ranks(&model, o)[i / 4] == i % 4));
        }
    }
}

#[test]
fn aggregate_fixture_cut_preserves_all_feasible_caps_and_floors() {
    let model = tiny();
    let positive = Problem::new(&model).unwrap();
    let negative = positive.negated();
    for problem in [&positive, &negative] {
        enumerate(&model, |outcomes, _| {
            let mut points = problem.base.clone();
            for (i, g) in problem.games.iter().enumerate() {
                points[g.home] += g.hg[outcomes[i] as usize];
                points[g.away] += g.ag[outcomes[i] as usize];
            }
            for mask in 0..16 {
                for cap in -9..=9 {
                    if (0..4).all(|t| mask & (1 << t) != 0 || points[t] <= cap) {
                        let mut domains = vec![7; problem.games.len()];
                        assert!(problem
                            .propagate_policy(cap, mask, &mut domains, true)
                            .is_some());
                        for i in 0..outcomes.len() {
                            assert_ne!(domains[i] & (1 << outcomes[i]), 0);
                        }
                    }
                }
            }
        });
    }
}

#[test]
fn aggregate_cut_detects_a_shared_fixture_contradiction_marginals_miss() {
    let mut request = tiny().request;
    request.team_groups.push(request.team_groups[0].clone());
    request.team_groups[4].team_id = 4;
    for t in 0..4 {
        let mut g = request.games[0].clone();
        g.home_id = t;
        g.away_id = 4;
        g.id = request.games.len() as i32;
        request.games.push(g);
    }
    let model = Model::new(request).unwrap();
    let problem = Problem::new(&model).unwrap();
    assert!(problem
        .propagate_policy(3, 0, &mut vec![7; 10], false)
        .is_some());
    assert!(problem
        .propagate_policy(3, 0, &mut vec![7; 10], true)
        .is_none());
}

#[test]
fn discrete_floor_proves_palmeiras_seventeenth_and_preserves_reachable_ranks() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json",
    );
    let request: Request = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    for reverse in [false, true] {
        let mut request = request.clone();
        if reverse {
            request.team_groups.reverse();
            request.games.reverse();
        }
        let model = Model::new(request).unwrap();
        let team = model.indices[&16];
        let problem = Problem::new(&model).unwrap().negated();
        let cap = problem.target_max(team);
        assert_eq!(cap, -57);
        let discrete = problem.clone().with_discrete_cuts(true);
        let legacy = problem.with_discrete_cuts(false);
        // Enable the existing aggregate stage independently of environment flags.
        // impossible() uses that stage when RUST_ODDS_AGGREGATE_CUTS=1; test the
        // explicit point-domain policy first for each required exemption set.
        let mandatory = [16, 110, 318].map(|id| model.indices[&id]);
        for additional in [20, 79] {
            let mask = mandatory.into_iter().fold(0, |m, t| m | (1 << t))
                | (1 << model.indices[&additional]);
            assert!(discrete
                .propagate_policy(cap, mask, &mut vec![7; model.fixtures.len()], true)
                .is_none());
            if additional == 20 {
                assert!(legacy
                    .propagate_policy(cap, mask, &mut vec![7; model.fixtures.len()], true)
                    .is_some());
            }
        }
        // The complete verified 15th-place witness must retain a feasible floor.
        let certificate: Value = serde_json::from_str(include_str!(
            "../../experiments/rare_positions/results/2026-09-30-palmeiras-15-goal-witness.json"
        ))
        .unwrap();
        let mut campaigns = model.base.clone();
        let mut full_scores = model.empty_scores();
        for (i, g) in model.fixtures.iter().enumerate() {
            let id = model.request.games[g.request_index].id;
            let score = certificate["remaining_fixture_assignment"]
                .as_array()
                .unwrap()
                .iter()
                .find(|a| a["game_id"] == id)
                .unwrap()["score"]
                .as_array()
                .unwrap();
            let score = [
                score[0].as_i64().unwrap() as i32,
                score[1].as_i64().unwrap() as i32,
            ];
            model.add(&mut campaigns, i, score);
            full_scores[g.request_index] = score;
        }
        let mut order = vec![0; model.n];
        model.standings(&mut order, &campaigns, &full_scores, &mut Rng::new(1));
        assert_eq!(order[14], team);
        let mask = (0..model.ids.len())
            .filter(|t| *t == team || campaigns[*t].points < 57)
            .fold(0, |m, t| m | (1 << t));
        assert!(discrete
            .propagate_policy(cap, mask, &mut vec![7; model.fixtures.len()], true)
            .is_some());
    }
}

#[test]
fn discrete_floor_retains_feasible_restricted_domains_with_point_adjustments() {
    for adjustments in [[0, 1, 2, 4], [-2, 0, 3, 7]] {
        let mut request = tiny().request;
        for (team, adjustment) in request.team_groups.iter_mut().zip(adjustments) {
            team.add_sub = adjustment;
        }
        let model = Model::new(request).unwrap();
        let problem = Problem::new(&model)
            .unwrap()
            .negated()
            .with_discrete_cuts(true);
        enumerate(&model, |outcomes, _| {
            let mut points = problem.base.clone();
            for (g, &o) in problem.games.iter().zip(outcomes) {
                points[g.home] += g.hg[o as usize];
                points[g.away] += g.ag[o as usize];
            }
            for mask in 0..16 {
                for cap in -9..=2 {
                    if (0..4).all(|t| mask & (1 << t) != 0 || points[t] <= cap) {
                        let mut domains: Vec<_> = outcomes.iter().map(|o| (1 << o) | 2).collect();
                        assert!(problem
                            .propagate_policy(cap, mask, &mut domains, true)
                            .is_some());
                        for (d, o) in domains.into_iter().zip(outcomes) {
                            assert_ne!(d & (1 << o), 0);
                        }
                    }
                }
            }
        });
    }
}

#[test]
fn discrete_cut_is_disabled_for_other_scoring_and_bonus_rules() {
    for bonus in [false, true] {
        let mut request = tiny().request;
        if bonus {
            request.phase.bonus_points = 1;
        } else {
            request.phase.championship.point_win = 2;
        }
        let model = Model::new(request).unwrap();
        let problem = Problem::new(&model).unwrap().negated();
        for mask in 0..16 {
            for cap in -9..=3 {
                let mut on = vec![7; problem.games.len()];
                let mut off = on.clone();
                assert_eq!(
                    problem
                        .clone()
                        .with_discrete_cuts(true)
                        .propagate_policy(cap, mask, &mut on, true),
                    problem
                        .clone()
                        .with_discrete_cuts(false)
                        .propagate_policy(cap, mask, &mut off, true)
                );
                assert_eq!(on, off);
            }
        }
    }
}

#[test]
fn reordered_neighbors_return_only_production_sorted_witnesses() {
    let mut model = tiny();
    model.keys = vec![
        golaberto_odds::model::Key::Pt,
        golaberto_odds::model::Key::Gd,
        golaberto_odds::model::Key::W,
        golaberto_odds::model::Key::Bias,
    ];
    let cells: Vec<_> = (0..model.n)
        .flat_map(|team| (0..model.n).map(move |rank| golaberto_odds::search::Cell { team, rank }))
        .collect();
    let estimates = vec![
        pool::Estimate {
            reachability: "undecided".into(),
            ..Default::default()
        };
        model.n * model.n
    ];
    for order in ["breadth", "spread"] {
        let proofs = golaberto_odds::neighbors::search_order(
            &model,
            &cells,
            &[vec![0; model.fixtures.len()], vec![2; model.fixtures.len()]],
            &estimates,
            100,
            order,
        );
        assert!(!proofs.is_empty());
        for proof in proofs {
            assert_eq!(
                conditioned::canonical_ranks(&model, &proof.outcomes)[proof.cell.team],
                proof.cell.rank
            );
        }
    }
}

#[test]
fn constructive_reuse_marks_only_ranks_in_verified_seasons() {
    let model = tiny();
    let cells: Vec<_> = (0..model.n)
        .flat_map(|team| (0..model.n).map(move |rank| golaberto_odds::search::Cell { team, rank }))
        .collect();
    let mut estimates = vec![
        pool::Estimate {
            reachability: "undecided".into(),
            ..Default::default()
        };
        model.n * model.n
    ];
    let seasons = golaberto_odds::proof::witnesses_mode(&model, &cells, &mut estimates, "reuse");
    for (index, estimate) in estimates.iter().enumerate() {
        assert_eq!(estimate.probability, 0.);
        if estimate.reachability == "reachable_by_construction" {
            assert!(seasons
                .iter()
                .any(
                    |o| conditioned::canonical_ranks(&model, o)[index / model.n] == index % model.n
                ));
        }
    }
}

#[test]
fn extreme_rank_screen_never_rejects_a_matching_complete_season() {
    for sort in ["pt,w,bias", "pt,gd,w,bias"] {
        for draw_points in [1, 3] {
            let mut request = tiny().request;
            request.phase.sort = sort.into();
            request.phase.championship.point_draw = draw_points;
            let model = Model::new(request).unwrap();
            let screen = golaberto_odds::reachability::JointProblem::new(&model).unwrap();
            enumerate(&model, |outcomes, _| {
                let ranks = conditioned::canonical_ranks(&model, outcomes);
                for team in 0..model.n {
                    for maximum in [true, false] {
                        let mut extreme = true;
                        for (i, g) in model.fixtures.iter().enumerate() {
                            let points = if g.home == team {
                                Some([0, draw_points, 3])
                            } else if g.away == team {
                                Some([3, draw_points, 0])
                            } else {
                                None
                            };
                            if let Some(points) = points {
                                let v = if maximum {
                                    *points.iter().max().unwrap()
                                } else {
                                    *points.iter().min().unwrap()
                                };
                                extreme &= points[outcomes[i] as usize] == v;
                            }
                        }
                        if extreme {
                            assert!(screen.extreme_possible(
                                &model,
                                golaberto_odds::search::Cell {
                                    team,
                                    rank: ranks[team]
                                },
                                maximum
                            ));
                        }
                    }
                }
            });
        }
    }
}

#[test]
fn numeric_zero_outcome_mass_cannot_prove_unrestricted_impossibility() {
    let request: Request = serde_json::from_value(json!({"id":1,
        "phase":{"sort":"pt,w,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":[{"team_id":1},{"team_id":2,"add_sub":2}],
        "games":[{"id":1,"home_id":1,"away_id":2,"home_power":1e-20,"away_power":1e-20,"played":false}]})).unwrap();
    let model = Model::new(request).unwrap();
    assert_eq!(model.fixtures[0].prob, [0., 1., 0.]);
    let cell = golaberto_odds::search::Cell { team: 0, rank: 0 };
    let mut estimates = vec![
        pool::Estimate {
            reachability: "undecided".into(),
            ..Default::default()
        };
        4
    ];
    let run = golaberto_odds::reachability::run(&model, &[cell], &mut estimates, 100, 3200);
    assert_eq!(run.stats.impossible, 0);
    assert_eq!(estimates[0].reachability, "reachable_by_construction");
    assert!(run
        .outcomes
        .iter()
        .any(|o| conditioned::canonical_ranks(&model, o)[0] == 0));
    assert_eq!(estimates[0].probability, 0.);
}

#[test]
fn reference_joint_domains_match_the_measured_positive_outcome_masks() {
    let inputs = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs");
    for entry in std::fs::read_dir(inputs).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|s| s != "json") {
            continue;
        }
        let model =
            Model::new(serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()).unwrap();
        let problem = golaberto_odds::reachability::JointProblem::new(&model).unwrap();
        for (domain, g) in problem.initial_domains().iter().zip(&model.fixtures) {
            let old_mask = (0..3).fold(0, |d, o| if g.prob[o] > 0. { d | (1 << o) } else { d });
            assert_eq!(*domain, old_mask);
        }
    }
}

#[test]
fn minimum_witness_uses_final_bounds_and_defers_new_proofs() {
    let request: Request = serde_json::from_value(json!({"id":1,
        "phase":{"sort":"pt,w,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":[{"team_id":1,"add_sub":3},{"team_id":2}],
        "games":[{"id":1,"home_id":1,"away_id":2,"home_power":1.,"away_power":1.,"played":false}]}))
    .unwrap();
    let model = Model::new(request).unwrap();
    let cell = golaberto_odds::search::Cell { team: 0, rank: 1 };
    let mut estimates = vec![
        pool::Estimate {
            reachability: "undecided".into(),
            ..Default::default()
        };
        4
    ];
    let result =
        golaberto_odds::proof::witnesses_report(&model, &[cell], &mut estimates, "deferred");
    // Target loses, opponent catches its three points and wins the wins tie.
    // The opponent's current zero points must not force it below the target.
    assert_eq!(result.deferred.len(), 1);
    assert!(result.outcomes.is_empty());
    assert!(result.nodes <= 100);
    assert_eq!(
        conditioned::canonical_ranks(&model, &result.deferred[0])[0],
        1
    );
    assert!(estimates
        .iter()
        .all(|e| e.probability == 0. && e.reachability == "undecided"));
}

#[test]
fn goal_completion_resolves_palmeiras_ties_with_verified_compact_certificates() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../experiments/rare_positions");
    let model = Model::new(
        serde_json::from_slice(
            &std::fs::read(
                root.join("reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json"),
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let saved: Value = serde_json::from_slice(
        &std::fs::read(root.join("results/2026-09-30-palmeiras-15-goal-witness.json")).unwrap(),
    )
    .unwrap();
    let outcomes: Vec<_> = saved["remaining_fixture_assignment"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            match g["score"][0]
                .as_i64()
                .unwrap()
                .cmp(&g["score"][1].as_i64().unwrap())
            {
                std::cmp::Ordering::Less => 0,
                std::cmp::Ordering::Equal => 1,
                std::cmp::Ordering::Greater => 2,
            }
        })
        .collect();
    let team = model.indices[&16];
    assert_eq!(conditioned::canonical_ranks(&model, &outcomes)[team], 12);
    for rank in [13, 14] {
        let cell = golaberto_odds::search::Cell { team, rank };
        let proof = golaberto_odds::goal_completion::complete(&model, &outcomes, cell).unwrap();
        assert!(golaberto_odds::goal_completion::verify(
            &model, &proof, cell
        ));
        assert_eq!(proof.ranks[team], rank);
        assert!(!proof.adjustments.is_empty());
    }
    // This particular pattern has only fourteen other teams at or above the
    // target's points/wins. Changing goals cannot put fifteen ahead.
    assert!(golaberto_odds::goal_completion::complete(
        &model,
        &outcomes,
        golaberto_odds::search::Cell { team, rank: 15 }
    )
    .is_none());
}

#[test]
fn equal_goals_on_a_draw_break_a_goals_scored_tie_without_changing_gd() {
    let request: Request = serde_json::from_value(json!({"id":1,
        "phase":{"sort":"pt,w,gd,gf,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":[{"team_id":1,"add_sub":10},{"team_id":2,"add_sub":11,"bias":1},{"team_id":3}],
        "games":[{"id":1,"home_id":1,"away_id":3,"home_power":1.,"away_power":1.,"played":false}]})).unwrap();
    let model = Model::new(request).unwrap();
    let cell = golaberto_odds::search::Cell { team: 0, rank: 0 };
    assert_eq!(conditioned::canonical_ranks(&model, &[1])[0], 1);
    let proof = golaberto_odds::goal_completion::complete(&model, &[1], cell).unwrap();
    assert!(proof.adjustments.iter().all(|a| a.margin == 0));
    assert!(proof.adjustments.iter().any(|a| a.equal_goals > 0));
    assert!(golaberto_odds::goal_completion::verify(
        &model, &proof, cell
    ));
}

#[test]
fn shared_equal_goal_adjustments_do_not_independently_order_tied_endpoints() {
    let request: Request = serde_json::from_value(json!({"id":1,
        "phase":{"sort":"pt,w,gd,gf,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":[{"team_id":1},{"team_id":2,"bias":1}],
        "games":[{"id":1,"home_id":1,"away_id":2,"home_power":1.,"away_power":1.,"played":false}]})).unwrap();
    let model = Model::new(request).unwrap();
    assert!(golaberto_odds::goal_completion::complete(
        &model,
        &[1],
        golaberto_odds::search::Cell { team: 0, rank: 0 }
    )
    .is_none());
}

#[test]
fn every_goal_completion_in_the_enumerated_league_is_a_real_sorted_season() {
    for sort in ["pt,w,gd,gf,bias", "pt,gd,gf,w,bias", "pt,w,gf,bias"] {
        let mut request = tiny().request;
        request.phase.sort = sort.into();
        let model = Model::new(request).unwrap();
        let mut completed = 0;
        enumerate(&model, |outcomes, _| {
            for team in 0..model.n {
                for rank in 0..model.n {
                    let cell = golaberto_odds::search::Cell { team, rank };
                    if let Some(proof) = golaberto_odds::goal_completion::complete_with_paths(
                        &model, outcomes, cell, true,
                    ) {
                        assert!(golaberto_odds::goal_completion::verify(
                            &model, &proof, cell
                        ));
                        // Replay independently through Model::add to check the
                        // helper's incremental campaign updates as well.
                        let mut scores: Vec<_> = outcomes
                            .iter()
                            .map(|o| conditioned::canonical(*o))
                            .collect();
                        for a in &proof.adjustments {
                            let score = &mut scores[a.fixture];
                            score[0] += a.equal_goals;
                            score[1] += a.equal_goals;
                            let winner = if score[0] > score[1] { 0 } else { 1 };
                            assert!(a.margin == 0 || score[0] != score[1]);
                            score[winner] += a.margin;
                        }
                        let mut campaigns = model.base.clone();
                        let mut full_scores = model.empty_scores();
                        for (i, f) in model.fixtures.iter().enumerate() {
                            model.add(&mut campaigns, i, scores[i]);
                            full_scores[f.request_index] = scores[i];
                        }
                        let mut order = vec![0; model.n];
                        model.standings(&mut order, &campaigns, &full_scores, &mut Rng::new(1));
                        let mut ranks = vec![0; model.n];
                        for (rank, team) in order.into_iter().enumerate() {
                            ranks[team] = rank;
                        }
                        assert_eq!(ranks, proof.ranks);
                        completed += 1;
                    }
                }
            }
        });
        assert!(completed > 0);
    }
}

#[test]
fn goal_certificates_are_deferred_without_changing_probabilities_or_early_labels() {
    let request: Request = serde_json::from_value(json!({"id":1,
        "phase":{"sort":"pt,w,gd,gf,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":[{"team_id":1},{"team_id":2},{"team_id":3,"add_sub":6},{"team_id":4}],
        "games":[{"id":1,"home_id":1,"away_id":4,"home_score":5,"away_score":0,"played":true},
                 {"id":2,"home_id":2,"away_id":4,"home_score":1,"away_score":0,"played":true},
                 {"id":3,"home_id":1,"away_id":3,"home_power":1.,"away_power":1.,"played":false}]})).unwrap();
    let model = Model::new(request).unwrap();
    let cell = golaberto_odds::search::Cell { team: 0, rank: 2 };
    assert_eq!(conditioned::canonical_ranks(&model, &[0])[0], 1);
    let mut estimates = vec![
        pool::Estimate {
            reachability: "undecided".into(),
            ..Default::default()
        };
        16
    ];
    let report = golaberto_odds::proof::witnesses_report_with_goals(
        &model,
        &[cell],
        &mut estimates,
        "deferred",
        true,
    );
    assert_eq!(report.goal_witnesses.len(), 1);
    assert!(report.outcomes.is_empty());
    assert!(report.nodes < 100);
    assert!(estimates
        .iter()
        .all(|e| e.probability == 0. && e.reachability == "undecided"));
    assert!(golaberto_odds::goal_completion::verify(
        &model,
        &report.goal_witnesses[0],
        cell
    ));
}

#[test]
fn adjacent_cdf_factor_preserves_the_original_weight_bits_at_boundaries() {
    let games = [lookahead::RankGame {
        index: 0,
        home: 0,
        away: 1,
        prob: [0.2, 0.3, 0.5],
        hg: [0, 1, 3],
        ag: [3, 1, 0],
    }];
    for cap_max in [0, 1, 3, 8] {
        let suffix = lookahead::Suffix::new(&games, 2, cap_max);
        for step in 0..=1 {
            for team in 0..2 {
                for cap in -5..=cap_max + 5 {
                    for (below, above) in [(0., 1.), (0.03125, 18.), (1., 1.), (123.456, 0.789)] {
                        let lower = suffix.cdf(step, team, cap - 1);
                        let at = suffix.cdf(step, team, cap);
                        let original = below * lower + (at - lower) + above * (1. - at);
                        assert_eq!(
                            suffix.factor(step, team, cap, below, above).to_bits(),
                            original.to_bits()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn proof_probes_preserve_every_enumerated_rank_hit_with_wins_and_negative_points() {
    use golaberto_odds::domains::{Cache, ProbeConfig};
    let mut request = tiny().request.clone();
    request.team_groups[0].add_sub = -3;
    let model = Model::new(request).unwrap();
    let mut checks = 0;
    let mut removals = 0;
    for stride in [1, 8] {
        for target in 0..model.n {
            let mut selected = Vec::new();
            let mut remaining = Vec::new();
            for (index, f) in model.fixtures.iter().enumerate() {
                let g = RankGame {
                    index,
                    home: f.home,
                    away: f.away,
                    prob: f.prob,
                    hg: [0, stride, 3 * stride + i32::from(stride > 1)],
                    ag: [3 * stride + i32::from(stride > 1), stride, 0],
                };
                if g.home == target || g.away == target {
                    selected.push(g)
                } else {
                    remaining.push(g)
                }
            }
            for rank in 0..model.n {
                enumerate(&model, |outcomes, _| {
                    if conditioned::canonical_ranks(&model, outcomes)[target] != rank {
                        return;
                    }
                    let mut points: Vec<_> = model
                        .base
                        .iter()
                        .map(|b| b.points * stride + if stride > 1 { b.wins } else { 0 })
                        .collect();
                    for g in &selected {
                        let o = outcomes[g.index] as usize;
                        points[g.home] += g.hg[o];
                        points[g.away] += g.ag[o];
                    }
                    // Fresh budgets exercise each possible conditional target pattern.
                    let mut cache =
                        Cache::new(&remaining, &selected, target, rank, model.n, false, true)
                            .unwrap()
                            .with_probes(
                                stride,
                                ProbeConfig {
                                    checks: 12,
                                    nodes: 32,
                                },
                            );
                    let (_, domains) = cache.get(&points, points[target], outcomes);
                    if let Some(d) = domains {
                        assert!(d.feasible, "lost rank hit {target}/{rank} {outcomes:?}");
                        for (g, mask) in remaining.iter().zip(&d.domains) {
                            assert_ne!(*mask & (1 << outcomes[g.index]), 0, "pruned valid outcome");
                        }
                    }
                    checks += cache.stats.checks;
                    removals += cache.stats.removed;
                    assert!(cache.stats.checks <= 12);
                });
            }
        }
    }
    assert!(checks > 0);
    assert!(removals > 0);
}

#[test]
fn decisive_path_completion_changes_endpoint_gd_and_preserves_intermediate_gd() {
    let request:Request=serde_json::from_value(json!({"id":1,
        "phase":{"sort":"pt,w,gd,gf,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":[{"team_id":0},{"team_id":1,"bias":1},{"team_id":2,"add_sub":-10}],
        "games":[{"id":0,"home_id":0,"away_id":2,"home_score":0,"away_score":10,"played":true},
                 {"id":1,"home_id":0,"away_id":1,"played":false},
                 {"id":2,"home_id":1,"away_id":2,"played":false}]})).unwrap();
    let model = Model::new(request).unwrap();
    let outcomes = [2, 2];
    let cell = golaberto_odds::search::Cell { team: 0, rank: 0 };
    assert_eq!(conditioned::canonical_ranks(&model, &outcomes)[0], 1);
    assert!(
        golaberto_odds::goal_completion::complete_with_paths(&model, &outcomes, cell, false)
            .is_none()
    );
    let proof = golaberto_odds::goal_completion::complete_with_paths(&model, &outcomes, cell, true)
        .unwrap();
    assert!(golaberto_odds::goal_completion::verify(
        &model, &proof, cell
    ));
    let mut campaigns = model.base.clone();
    for (i, f) in model.fixtures.iter().enumerate() {
        let mut score = conditioned::canonical(outcomes[i]);
        for a in &proof.adjustments {
            if a.fixture == i {
                assert_eq!(a.equal_goals, 0);
                score[0] += a.margin;
            }
        }
        model.add(&mut campaigns, i, score);
        assert_eq!(f.home, i);
    }
    assert_eq!(campaigns[1].gf - campaigns[1].ga, 0);
    assert!(campaigns[1].gf > 1 && campaigns[1].ga > 1);
    assert!(proof.adjustments.len() >= 2);
}

#[test]
fn reverse_decisive_path_can_lower_target_gd_without_lowering_middle_team_gd() {
    let request:Request=serde_json::from_value(json!({"id":1,
        "phase":{"sort":"pt,w,gd,gf,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":[{"team_id":0},{"team_id":1},{"team_id":2,"add_sub":100},{"team_id":3,"add_sub":-100}],
        "games":[{"id":0,"home_id":0,"away_id":3,"home_score":10,"away_score":0,"played":true},
                 {"id":1,"home_id":1,"away_id":0,"played":false},
                 {"id":2,"home_id":2,"away_id":1,"played":false}]})).unwrap();
    let model = Model::new(request).unwrap();
    let outcomes = [2, 2];
    let cell = golaberto_odds::search::Cell { team: 0, rank: 2 };
    assert_eq!(conditioned::canonical_ranks(&model, &outcomes)[0], 1);
    assert!(
        golaberto_odds::goal_completion::complete_with_paths(&model, &outcomes, cell, false)
            .is_none()
    );
    let proof = golaberto_odds::goal_completion::complete_with_paths(&model, &outcomes, cell, true)
        .unwrap();
    assert!(golaberto_odds::goal_completion::verify(
        &model, &proof, cell
    ));
}

#[test]
fn fixture_mask_matches_individual_prefix_checks() {
    let games = vec![
        RankGame {
            index: 0,
            home: 0,
            away: 1,
            prob: [0.4, 0.2, 0.4],
            hg: [0, 1, 3],
            ag: [3, 1, 0],
        },
        RankGame {
            index: 1,
            home: 0,
            away: 2,
            prob: [0.4, 0.2, 0.4],
            hg: [0, 1, 3],
            ag: [3, 1, 0],
        },
        RankGame {
            index: 2,
            home: 1,
            away: 2,
            prob: [0.4, 0.2, 0.4],
            hg: [0, 1, 3],
            ag: [3, 1, 0],
        },
    ];
    for rank in 0..3 {
        let d = Domains::propagate(&games, &[0, 2, 4], &[1, 2], rank, 6);
        for (step, g) in games.iter().enumerate() {
            for hp in -2..12 {
                for ap in -2..12 {
                    let mask = d.prefix_mask(step, g, hp, ap);
                    for o in 0..3 {
                        assert_eq!(mask & (1 << o) != 0, d.allows_prefix(step, g, o, hp, ap));
                    }
                }
            }
        }
    }
}
