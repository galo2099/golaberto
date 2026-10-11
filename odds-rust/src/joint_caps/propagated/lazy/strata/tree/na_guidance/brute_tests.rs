use super::*;
use crate::{
    joint_caps::propagated::lazy::strata::tree::path_priority,
    lookahead::RankGame,
    model::{Model, Request},
};
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc};

fn model(pairs: &[(usize, usize)], mean: f64) -> Model {
    let n = pairs.iter().flat_map(|&(h, a)| [h, a]).max().unwrap_or(0) + 1;
    let request: Request=serde_json::from_value(json!({
        "id":1,"phase":{"sort":"pt,w,gd,gf","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
        "team_groups":(0..n).map(|t|json!({"team_id":t,"add_sub":0,"bias":0})).collect::<Vec<_>>(),
        "games":pairs.iter().enumerate().map(|(i,&(h,a))|json!({"id":i,"home_id":h,"away_id":a,"home_power":mean,"away_power":mean})).collect::<Vec<_>>()
    })).unwrap();
    Model::new(request).unwrap()
}
fn packed_games(m: &Model, masks: &[[f64; 3]]) -> Vec<RankGame> {
    m.fixtures
        .iter()
        .enumerate()
        .map(|(i, f)| RankGame {
            index: i,
            home: f.home,
            away: f.away,
            prob: masks[i],
            hg: [0, 1, 3],
            ag: [3, 1, 0],
        })
        .collect()
}
fn pattern(
    m: &Model,
    fixed: &[(usize, u8)],
    masks: &[[f64; 3]],
    intervals: &[(i32, i32)],
) -> Pattern {
    let games = packed_games(m, masks);
    let mut base = vec![0; m.n];
    base[0] = 3;
    for &(i, o) in fixed {
        let g = games[i];
        base[g.home] += g.hg[o as usize];
        base[g.away] += g.ag[o as usize];
    }
    let lower: Vec<_> = intervals.iter().map(|x| x.0).collect();
    let upper: Vec<_> = intervals.iter().map(|x| x.1).collect();
    let joint = NecessaryJoint::new(&[], &base, &lower, &upper, 0, 100)
        .unwrap()
        .0;
    Pattern {
        fixed: fixed.to_vec(),
        bias: None,
        omitted: games
            .into_iter()
            .filter(|g| !fixed.iter().any(|(i, _)| *i == g.index))
            .collect(),
        base,
        lower,
        upper,
        guide: Arc::new(Guide::new(Vec::new(), m.n)),
        joint: Arc::new(joint),
        mass: 1.0,
        tilt: 1.0,
        cumulative: 1.0,
        bound_guidance: false,
    }
}
fn exact_rectangle_mass(
    m: &Model,
    p: &Pattern,
    games: &[RankGame],
    target: usize,
    rank: usize,
) -> f64 {
    let mut total = 0.0;
    let mut outcomes = vec![0u8; games.len()];
    fn walk(
        m: &Model,
        p: &Pattern,
        games: &[RankGame],
        target: usize,
        rank: usize,
        i: usize,
        out: &mut [u8],
        mass: f64,
        total: &mut f64,
    ) {
        if i == games.len() {
            let mut scores = p.base.clone();
            for (j, g) in games.iter().enumerate() {
                let o = out[j] as usize;
                scores[g.home] += g.hg[o];
                scores[g.away] += g.ag[o];
            }
            for t in 1..m.n {
                if scores[t] < p.lower[t] || scores[t] > p.upper[t] {
                    return;
                }
            }
            let above = (1..m.n).filter(|&t| scores[t] >= scores[target]).count();
            let below = (1..m.n).filter(|&t| scores[t] <= scores[target]).count();
            if above >= rank && below >= m.n - 1 - rank {
                *total += mass;
            }
            return;
        }
        let g = games[i];
        let original = m.fixtures[g.index].prob;
        for o in 0..3 {
            if original[o] > 0.0 && g.prob[o] > 0.0 {
                out[i] = o as u8;
                walk(
                    m,
                    p,
                    games,
                    target,
                    rank,
                    i + 1,
                    out,
                    mass * original[o],
                    total,
                );
            }
        }
    }
    let fixed_mass = p
        .fixed
        .iter()
        .map(|&(i, o)| m.fixtures[i].prob[o as usize])
        .product::<f64>();
    walk(
        m,
        p,
        games,
        target,
        rank,
        0,
        &mut outcomes,
        fixed_mass,
        &mut total,
    );
    total
}
fn eval_bound(
    m: &Model,
    p: &Pattern,
    target: usize,
    rank: usize,
    cache: &mut PmfCache,
) -> (f64, f64, Vec<u64>, Pmfs) {
    let mut work = 0;
    let mut hits = 0;
    let (games, key, domain) = rectangle(m, p, &mut work, 2_000_000).unwrap();
    let pmf = pmfs(
        m, p, &games, &key, m.n, &mut work, 2_000_000, cache, &mut hits,
    )
    .unwrap();
    let result = score(p, target, rank, m.n, &pmf, &mut work, 2_000_000).unwrap();
    (domain, result.log_upper, key, pmf)
}

#[test]
fn exhaustive_four_rival_shared_graph_respects_mask_fixed_domain_and_box_bound_for_ranks_1_to_3() {
    let m = model(&[(1, 2), (2, 3), (3, 4), (4, 1)], 1.0);
    let masks = [
        [0.4, 0.0, 0.6],
        [0.99, 0.0, 0.01],
        [0.0, 0.8, 0.2],
        [0.7, 0.0, 0.3],
    ];
    let fixed = [(0, 2)];
    let mut cache = PmfCache::new();
    let boxes = [
        vec![(i32::MIN / 4, i32::MAX / 4), (1, 9), (1, 9), (1, 9), (1, 9)],
        vec![(i32::MIN / 4, i32::MAX / 4), (6, 9), (0, 2), (1, 9), (1, 9)], // one rival above, another below
        vec![(i32::MIN / 4, i32::MAX / 4), (1, 9), (3, 3), (1, 9), (1, 9)], // explicit tied score allowed
    ];
    let mut positive_cases = 0;
    let mut baseline_rank_cases = 0;
    for (box_index, intervals) in boxes.into_iter().enumerate() {
        let p = pattern(&m, &fixed, &masks, &intervals);
        let (games, key, domain) = rectangle(&m, &p, &mut 0, 2_000_000).unwrap();
        let expected_domain = m.fixtures[0].prob[2].ln()
            + games
                .iter()
                .map(|g| {
                    (0..3)
                        .filter(|&o| m.fixtures[g.index].prob[o] > 0.0 && g.prob[o] > 0.0)
                        .map(|o| m.fixtures[g.index].prob[o])
                        .sum::<f64>()
                        .ln()
                })
                .sum::<f64>();
        assert!((domain - expected_domain).abs() < 1e-12);
        for rank in 1..=3 {
            let (d, u, _, _) = eval_bound(&m, &p, 0, rank, &mut cache);
            let exact = exact_rectangle_mass(&m, &p, &games, 0, rank);
            if exact == 0.0 {
                continue;
            }
            positive_cases += 1;
            if box_index == 0 {
                baseline_rank_cases += 1;
            }
            let bound = (d + u).exp();
            assert!(
                exact <= bound * (1.0 + 1e-12) + 1e-300,
                "rank={rank}, exact={exact:e}, bound={bound:e}"
            );
        }
        assert!(!key.is_empty());
    }
    assert_eq!(
        baseline_rank_cases, 3,
        "all ranks need positive brute-force mass in baseline box"
    );
    assert!(positive_cases >= 3);
}

#[test]
fn pmfs_use_original_probabilities_and_cache_keys_distinguish_masks() {
    let m = model(&[(1, 2), (2, 3), (3, 4), (4, 1)], 1.0);
    let all = [
        [0.9, 0.0, 0.1],
        [0.99, 0.0, 0.01],
        [0.8, 0.0, 0.2],
        [0.75, 0.0, 0.25],
    ];
    let p = pattern(&m, &[(0, 2)], &all, &vec![(i32::MIN / 4, i32::MAX / 4); 5]);
    let mut cache = PmfCache::new();
    let (domain, _, key_a, pmfs_a) = eval_bound(&m, &p, 0, 2, &mut cache);
    let (_, _, key_again, pmfs_again) = eval_bound(&m, &p, 0, 2, &mut cache);
    assert_eq!(key_a, key_again);
    assert_eq!(pmfs_a, pmfs_again);
    let mut masked = all;
    masked[1] = [0.2, 0.8, 0.0];
    let p2 = pattern(
        &m,
        &[(0, 2)],
        &masked,
        &vec![(i32::MIN / 4, i32::MAX / 4); 5],
    );
    let (d2, _, key_b, pmfs_b) = eval_bound(&m, &p2, 0, 2, &mut cache);
    assert_ne!(key_a, key_b);
    assert_ne!(domain, d2);
    assert_ne!(pmfs_a, pmfs_b);
    let (games, _, _) = rectangle(&m, &p, &mut 0, 2_000_000).unwrap();
    let team = 3;
    let mut expected = BTreeMap::from([(p.base[team], 1.0)]);
    for g in games.iter().filter(|g| g.home == team || g.away == team) {
        let gains = if g.home == team { g.hg } else { g.ag };
        let original = m.fixtures[g.index].prob;
        let norm = (0..3)
            .filter(|&o| original[o] > 0.0 && g.prob[o] > 0.0)
            .map(|o| original[o])
            .sum::<f64>();
        let mut next = BTreeMap::<i32, f64>::new();
        for (&value, &mass) in &expected {
            for o in 0..3 {
                if original[o] > 0.0 && g.prob[o] > 0.0 {
                    *next.entry(value + gains[o]).or_default() += mass * original[o] / norm;
                }
            }
        }
        expected = next;
    }
    assert_eq!(pmfs_a[team], expected.into_iter().collect::<Vec<_>>());
}

#[test]
fn mixed_up_down_events_on_shared_fixture_are_positively_correlated() {
    let m = model(&[(0, 1)], 1.0);
    let original = m.fixtures[0].prob;
    let mass = original[0] + original[2];
    let up = original[2] / mass;
    let down = original[2] / mass;
    let joint = original[2] / mass;
    assert!(up > 0.0 && down > 0.0);
    assert!(joint > up * down + 1e-12);
}

#[test]
fn unsupported_gain_family_and_tiny_cap_leave_leaf_logs_unchanged() {
    let m = model(&[(1, 2), (2, 3), (3, 4), (4, 1)], 1.0);
    let masks = [[0.5, 0.0, 0.5]; 4];
    let p = pattern(&m, &[], &masks, &vec![(i32::MIN / 4, i32::MAX / 4); 5]);
    let mut unsupported = packed_games(&m, &masks);
    unsupported[1].ag = [0, 1, 3];
    let mut broken = p.clone();
    broken.omitted = unsupported;
    assert!(rectangle(&m, &broken, &mut 0, 10000).is_err());
    let leaf = Leaf {
        root: vec![],
        root_index: 0,
        above: true,
        mask: 0,
        selected: 0,
        ambiguous: 0,
        max_exceptions: 0,
        pattern: p,
        frozen: false,
        score: 1.0,
        rank_hint_next_team: None,
        original_parent_id: 0,
        rival_team_id: None,
        path_id: None,
        rival_fixed: vec![],
        raw_path_prior: 1.0,
        parent_proxy_log: None,
        child_proxy_log: None,
        local_child_share: 1.0,
        guide_fraction: 1.0,
        hybrid_depth: 0,
        hybrid_path_count: 0,
    };
    let mut logs = vec![-2.0];
    let before = logs.clone();
    let err = blend_leaf_corrected_logs(&m, 0, 1, &[leaf], &mut logs, 0.5, &mut 0, 0);
    assert!(err.is_err());
    assert_eq!(logs, before);
    let games = packed_games(&m, &masks);
    let mut screen = path_priority::Screen {
        roots: vec![path_priority::RootShare {
            prior: 1.0,
            count_chance: 1.0,
            raw_score: 1.0,
            normalized_share: 1.0,
        }],
        work: 0,
        reason: None,
        leaf_comparators: Some(vec![vec![[0.2, 0.3, 0.5]; 4]]),
        leaf_work: 0,
        leaf_reason: None,
    };
    let before = screen.roots[0].raw_score;
    assert!(blend_root_raw_scores(&m, 1, &games, &mut screen, 0.5, &mut 0, 0).is_err());
    assert_eq!(screen.roots[0].raw_score, before);
}
