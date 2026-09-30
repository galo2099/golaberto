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
    let problem = Problem::new(&model).unwrap();
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
