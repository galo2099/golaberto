//! Cheap constructive reachability witnesses for pool requests.
//!
//! This is deliberately a one-sided check: a bit is set only after the normal
//! scoring and standings implementation puts the team at that rank.
use crate::{
    model::{Key, Model},
    rng::Rng,
};

/// Find requested team/rank cells attained by complete feasible seasons.
/// `needed` is row-major (team * n + rank).
pub(super) fn find(model: &Model, needed: &[bool], seed: i64) -> Vec<bool> {
    let n = model.n;
    let len = n.saturating_mul(n);
    let mut found = vec![false; len];
    if n == 0 || needed.len() < len || !needed.iter().any(|v| *v) {
        return found;
    }
    let mut rng = Rng::new(seed);
    let mut campaign = model.base.clone();
    let mut scores = model.empty_scores();
    let mut order = vec![0; n];
    let wanted: Vec<usize> = (0..len).filter(|&i| needed[i]).collect();
    let mut remaining = wanted.len();
    let edges: Vec<_> = model.fixtures.iter().map(supported_edges).collect();
    // Fail closed if any fixture has no finite positive WDL with an actual
    // score row. This keeps every later fallback inside native support.
    if model.fixtures.iter().zip(&edges).any(|(f, edge)| {
        f.prob.iter().any(|p| !p.is_finite() || *p < 0.0)
            || !(0..3).any(|o| f.prob[o] > 0.0 && edge[o][0].is_some())
    }) {
        return found;
    }

    // Shared broad worlds amortize standings across all requested cells.
    for world in 0..96 {
        campaign.copy_from_slice(&model.base);
        for (i, f) in model.fixtures.iter().enumerate() {
            let outcome = match world % 8 {
                0 => 1, // draws
                1 => 2, // home wins
                2 => 0, // away wins
                3 => {
                    if f.home_sampler.mean >= f.away_sampler.mean {
                        2
                    } else {
                        0
                    }
                }
                4 => {
                    if f.home_sampler.mean >= f.away_sampler.mean {
                        0
                    } else {
                        2
                    }
                }
                _ => weighted_outcome(f.prob, &mut rng),
            };
            let Some(outcome) = supported_outcome(f.prob, outcome, &edges[i]) else {
                return found;
            };
            let s = if world % 8 >= 5 && world % 2 == 1 {
                edges[i][outcome][usize::from(world % 4 == 1)]
                    .unwrap_or_else(|| f.scores.sample(outcome, &mut rng))
            } else {
                f.scores.sample(outcome, &mut rng)
            };
            scores[f.request_index] = s;
            model.add(&mut campaign, i, s);
        }
        observe(
            model,
            needed,
            &mut found,
            &mut remaining,
            &mut order,
            &campaign,
            &scores,
            &mut rng,
        );
        if remaining == 0 {
            return found;
        }
    }

    // Small schedules can be covered exactly at the W/D/L level. Canonical
    // represented scores make this cheap while retaining production scoring,
    // head-to-head and all final tie-break behavior.
    let mut domains: Vec<Vec<(usize, [i32; 2])>> = Vec::with_capacity(model.fixtures.len());
    let mut path_count = 1usize;
    for f in &model.fixtures {
        let mut domain = Vec::new();
        for outcome in 0..3 {
            if f.prob[outcome] <= 0.0 {
                continue;
            }
            if let Some(score) = canonical_score(f, outcome) {
                domain.push((outcome, score));
            }
        }
        path_count = path_count.saturating_mul(domain.len().max(1));
        if domain.is_empty() || path_count > 512 {
            path_count = 0;
            break;
        }
        domains.push(domain);
    }
    if path_count > 0 {
        let paths: Vec<Vec<usize>> = (0..path_count)
            .map(|mut code| {
                (0..domains.len())
                    .map(|i| {
                        let choice = code % domains[i].len();
                        code /= domains[i].len();
                        choice
                    })
                    .collect()
            })
            .collect();
        // First cover every canonical WDL path before spending the bounded
        // margin-perturbation budget, so no path is displaced by variants.
        for path in &paths {
            campaign.copy_from_slice(&model.base);
            for (i, f) in model.fixtures.iter().enumerate() {
                let (_, score) = domains[i][path[i]];
                scores[f.request_index] = score;
                model.add(&mut campaign, i, score);
            }
            observe(
                model,
                needed,
                &mut found,
                &mut remaining,
                &mut order,
                &campaign,
                &scores,
                &mut rng,
            );
            if remaining == 0 {
                return found;
            }
        }
        let mut variants = 0usize;
        const MAX_MARGIN_VARIANTS: usize = 1024;
        'paths: for path in &paths {
            for (i, _f) in model.fixtures.iter().enumerate() {
                let (outcome, canonical) = domains[i][path[i]];
                for edge in 0..2 {
                    if variants >= MAX_MARGIN_VARIANTS {
                        break 'paths;
                    }
                    let Some(score) = edges[i][outcome][edge] else {
                        continue;
                    };
                    if score == canonical {
                        continue;
                    }
                    variants += 1;
                    campaign.copy_from_slice(&model.base);
                    let mut world = model.empty_scores();
                    for (j, fixture) in model.fixtures.iter().enumerate() {
                        let selected = if i == j { score } else { domains[j][path[j]].1 };
                        world[fixture.request_index] = selected;
                        model.add(&mut campaign, j, selected);
                    }
                    observe(
                        model,
                        needed,
                        &mut found,
                        &mut remaining,
                        &mut order,
                        &campaign,
                        &world,
                        &mut rng,
                    );
                    if remaining == 0 {
                        return found;
                    }
                }
            }
        }
    }

    // Targeted outcome paths: make each target's remaining fixtures all wins,
    // all draws, or all losses, while varying rival outcomes and score margins.
    // This exposes point totals and goal-difference extremes missed by native
    // sampling without ever inventing a score outside the fixture's support.
    let mut targets = vec![false; n];
    for &cell in &wanted {
        targets[cell / n] = true;
    }
    for target in 0..n {
        if !targets[target] {
            continue;
        }
        for style in 0..24 {
            campaign.copy_from_slice(&model.base);
            for (i, f) in model.fixtures.iter().enumerate() {
                let target_is_home = f.home == target;
                let target_is_away = f.away == target;
                let outcome = if target_is_home || target_is_away {
                    let desired = (style / 8) % 3; // target win/draw/loss
                    let home_outcome = if target_is_home {
                        [2, 1, 0][desired]
                    } else {
                        [0, 1, 2][desired]
                    };
                    if f.prob[home_outcome] > 0.0 {
                        home_outcome
                    } else {
                        weighted_outcome(f.prob, &mut rng)
                    }
                } else {
                    match style % 8 {
                        0 => 1,
                        1 => 2,
                        2 => 0,
                        3 => 1,
                        _ => weighted_outcome(f.prob, &mut rng),
                    }
                };
                let Some(outcome) = supported_outcome(f.prob, outcome, &edges[i]) else {
                    return found;
                };
                let edge = style % 8 == 3 || style % 8 == 4;
                let s = if edge {
                    edges[i][outcome][usize::from(style % 2 == 0)]
                        .unwrap_or_else(|| f.scores.sample(outcome, &mut rng))
                } else {
                    f.scores.sample(outcome, &mut rng)
                };
                scores[f.request_index] = s;
                model.add(&mut campaign, i, s);
            }
            observe(
                model,
                needed,
                &mut found,
                &mut remaining,
                &mut order,
                &campaign,
                &scores,
                &mut rng,
            );
            if remaining == 0 {
                return found;
            }
        }
    }

    // Sparse schedules leave many teams' own totals fixed. For those targets,
    // greedily build full score paths that push opponents above or below the
    // target, then replay intermediate fixture swaps through real standings.
    let mut appearances = vec![0usize; model.ids.len()];
    for f in &model.fixtures {
        if f.home < appearances.len() {
            appearances[f.home] += 1;
        }
        if f.away < appearances.len() {
            appearances[f.away] += 1;
        }
    }
    for target in 0..n {
        if remaining == 0 {
            break;
        }
        if appearances[target] > 1
            || !(0..n).any(|r| needed[target * n + r] && !found[target * n + r])
        {
            continue;
        }
        // Idle targets have one fixed path; active sparse targets get each W/D/L.
        let desired_paths: &[usize] = if appearances[target] == 0 {
            &[1]
        } else {
            &[0, 1, 2]
        };
        for &desired in desired_paths {
            let low = greedy_sparse_world(model, target, desired, false, &edges, &mut rng);
            let high = greedy_sparse_world(model, target, desired, true, &edges, &mut rng);
            // Replay up to nine evenly spaced fixture swaps. Each is a complete
            // supported season and all sorted ranks are reused.
            let checkpoints = model.fixtures.len().min(8);
            for checkpoint in 0..=checkpoints {
                let step = if checkpoints == 0 {
                    0
                } else {
                    checkpoint * model.fixtures.len() / checkpoints
                };
                campaign.copy_from_slice(&model.base);
                scores.copy_from_slice(&low);
                for (i, f) in model.fixtures.iter().enumerate() {
                    if i < step {
                        scores[f.request_index] = high[f.request_index];
                    }
                    model.add(&mut campaign, i, scores[f.request_index]);
                }
                observe(
                    model,
                    needed,
                    &mut found,
                    &mut remaining,
                    &mut order,
                    &campaign,
                    &scores,
                    &mut rng,
                );
                if remaining == 0 {
                    return found;
                }
            }
        }
    }
    found
}

fn greedy_sparse_world(
    model: &Model,
    target: usize,
    desired: usize,
    maximize_ahead: bool,
    edges: &[[[Option<[i32; 2]>; 2]; 3]],
    rng: &mut Rng,
) -> Vec<[i32; 2]> {
    let mut campaign = model.base.clone();
    let mut scores = model.empty_scores();
    // Fix the target's own total first so every rival decision uses its final
    // points, wins, goal difference, and other supported ranking fields.
    for (i, f) in model.fixtures.iter().enumerate() {
        let (target_home, target_away) = (f.home == target, f.away == target);
        if !target_home && !target_away {
            continue;
        }
        let home_outcome = if target_home {
            [2, 1, 0][desired]
        } else {
            [0, 1, 2][desired]
        };
        let outcome = supported_outcome(f.prob, home_outcome, &edges[i])
            .expect("fixture support prevalidated");
        let favor_target_margin = if target_home {
            !maximize_ahead
        } else {
            maximize_ahead
        };
        let score = edges[i][outcome][usize::from(favor_target_margin)]
            .expect("positive outcome has represented score support");
        scores[f.request_index] = score;
        model.add(&mut campaign, i, score);
    }
    for (i, f) in model.fixtures.iter().enumerate() {
        if f.home == target || f.away == target {
            continue;
        }
        let ranked = [f.home, f.away]
            .into_iter()
            .filter(|&t| t < model.n && t != target)
            .collect::<Vec<_>>();
        let mut best: Option<([i32; 2], usize)> = None;
        for outcome in 0..3 {
            if f.prob[outcome] <= 0.0 {
                continue;
            }
            for edge in 0..2 {
                let Some(score) = edges[i][outcome][edge] else {
                    continue;
                };
                let ahead = candidate_ahead_count(model, &campaign, f, score, target, &ranked);
                let improves = best.as_ref().is_none_or(|(_, old)| {
                    if maximize_ahead {
                        ahead > *old
                    } else {
                        ahead < *old
                    }
                });
                if improves {
                    best = Some((score, ahead));
                }
            }
        }
        let score = best.map(|(score, _)| score).unwrap_or_else(|| {
            let outcome =
                supported_outcome(f.prob, 1, &edges[i]).expect("fixture support prevalidated");
            f.scores.sample(outcome, rng)
        });
        scores[f.request_index] = score;
        model.add(&mut campaign, i, score);
    }
    scores
}

fn candidate_ahead_count(
    model: &Model,
    campaign: &[crate::model::Campaign],
    fixture: &crate::model::Fixture,
    score: [i32; 2],
    target: usize,
    ranked: &[usize],
) -> usize {
    let mut home = campaign[fixture.home];
    let mut away = campaign[fixture.away];
    let rules = &model.request.phase.championship;
    home.add(score[0], score[1], true, rules);
    away.add(score[1], score[0], false, rules);
    ranked
        .iter()
        .filter(|&&team| {
            let c = if team == fixture.home {
                &home
            } else if team == fixture.away {
                &away
            } else {
                &campaign[team]
            };
            heuristic_ahead_campaign(model, c, &campaign[target])
        })
        .count()
}

fn heuristic_ahead_campaign(
    model: &Model,
    a: &crate::model::Campaign,
    b: &crate::model::Campaign,
) -> bool {
    for key in &model.keys {
        let (x, y) = match key {
            Key::Pt => (a.points, b.points),
            Key::W => (a.wins, b.wins),
            Key::Gd => (a.gf - a.ga, b.gf - b.ga),
            Key::Gf => (a.gf, b.gf),
            Key::Away => (a.away, b.away),
            Key::Bias => (a.bias, b.bias),
            Key::Average => {
                let x = a.gf as f64 / (a.ga as f64 + 1e-13);
                let y = b.gf as f64 / (b.ga as f64 + 1e-13);
                match x.total_cmp(&y) {
                    std::cmp::Ordering::Greater => return true,
                    std::cmp::Ordering::Less => return false,
                    std::cmp::Ordering::Equal => continue,
                }
            }
            Key::Head | Key::Random | Key::Name | Key::Aet | Key::Gp => continue,
        };
        if x != y {
            return x > y;
        }
    }
    false
}

fn observe(
    model: &Model,
    needed: &[bool],
    found: &mut [bool],
    remaining: &mut usize,
    order: &mut [usize],
    campaign: &[crate::model::Campaign],
    scores: &[[i32; 2]],
    rng: &mut Rng,
) {
    model.standings(order, campaign, scores, rng);
    for (rank, &team) in order.iter().enumerate() {
        let cell = team * model.n + rank;
        if needed[cell] && !found[cell] {
            found[cell] = true;
            *remaining -= 1;
        }
    }
}

fn supported_outcome(
    prob: [f64; 3],
    preferred: usize,
    edges: &[[Option<[i32; 2]>; 2]; 3],
) -> Option<usize> {
    if prob[preferred] > 0.0 && edges[preferred][0].is_some() {
        return Some(preferred);
    }
    (0..3)
        .filter(|&i| prob[i].is_finite() && prob[i] > 0.0 && edges[i][0].is_some())
        .max_by(|&a, &b| prob[a].total_cmp(&prob[b]))
}

fn weighted_outcome(prob: [f64; 3], rng: &mut Rng) -> usize {
    let u = rng.float();
    let mut sum = 0.0;
    for (i, p) in prob.iter().enumerate() {
        sum += p;
        if u < sum {
            return i;
        }
    }
    prob.iter().rposition(|p| *p > 0.0).unwrap_or(1)
}

fn canonical_score(f: &crate::model::Fixture, outcome: usize) -> Option<[i32; 2]> {
    f.scores
        .conditional_scores(outcome)
        .map(|(score, _)| score)
        .min_by_key(|score| (score[0] + score[1], (score[0] - score[1]).abs()))
}

fn supported_edges(f: &crate::model::Fixture) -> [[Option<[i32; 2]>; 2]; 3] {
    std::array::from_fn(|outcome| {
        let mut low: Option<[i32; 2]> = None;
        let mut high: Option<[i32; 2]> = None;
        for (score, _) in f.scores.conditional_scores(outcome) {
            if low.is_none_or(|old| score[0] - score[1] < old[0] - old[1]) {
                low = Some(score);
            }
            if high.is_none_or(|old| score[0] - score[1] > old[0] - old[1]) {
                high = Some(score);
            }
        }
        [low, high]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Championship, Game, Phase, Request, Team};

    fn model(points: &[i32], games: Vec<Game>, sort: &str) -> Model {
        Model::new(Request {
            id: 1,
            zones: vec![],
            phase: Phase {
                sort: sort.into(),
                championship: Championship {
                    point_win: 3,
                    point_draw: 1,
                    point_loss: 0,
                },
                bonus_points: 0,
                bonus_points_threshold: 0,
            },
            games,
            team_groups: points
                .iter()
                .enumerate()
                .map(|(i, p)| Team {
                    team_id: i as i32 + 1,
                    add_sub: *p,
                    bias: 0,
                })
                .collect(),
        })
        .unwrap()
    }
    fn game(h: i32, a: i32, hm: f64, am: f64) -> Game {
        Game {
            id: 1,
            home_id: h,
            away_id: a,
            home_score: 0,
            away_score: 0,
            home_power: hm,
            away_power: am,
            played: false,
        }
    }

    #[test]
    fn zero_rate_fixture_is_scored_and_unreachable_cells_stay_false() {
        // Only teams 1 and 2 play. Their deterministic draw awards one point
        // each and moves both above outsider 3, whose old total was tied with
        // theirs. A skipped fixture would leave all three tied and can falsely
        // witness the outsider at the top.
        let m = model(&[0, 0, 0], vec![game(1, 2, 0.0, 0.0)], "pt,gd,random");
        let mut needed = vec![false; 9];
        needed[6] = true; // outsider 3 first is impossible
        needed[0] = true; // team 1 first is feasible
        let found = find(&m, &needed, 43);
        assert!(!found[6]);
        assert!(found[0]);
    }

    #[test]
    fn sparse_sweep_handles_unranked_external_opponent() {
        let m = model(&[0, 0], vec![game(1, 3, 0.0, 0.0)], "pt,gd,random");
        assert_eq!(m.n, 2);
        assert!(m.ids.len() > m.n);
        let mut needed = vec![false; 4];
        needed[0] = true;
        needed[2] = true;
        let found = find(&m, &needed, 211);
        assert!(found[0]);
        assert!(!found[2]);
    }

    #[test]
    fn fixed_points_and_static_head_to_head_tie_are_respected() {
        let m = model(
            &[30, 20, 10],
            vec![Game {
                id: 1,
                home_id: 2,
                away_id: 3,
                home_score: 1,
                away_score: 0,
                home_power: 0.0,
                away_power: 0.0,
                played: true,
            }],
            "pt,gd,head",
        );
        let mut needed = vec![false; 9];
        needed[1] = true;
        needed[3] = true;
        needed[7] = true;
        let found = find(&m, &needed, 4);
        assert_eq!(found, vec![false; 9]);
    }

    #[test]
    fn degenerate_zero_mean_draw_has_only_zero_zero_support() {
        let m = model(&[0, 0], vec![game(1, 2, 0.0, 0.0)], "pt,gd,random");
        let f = &m.fixtures[0];
        let edges = supported_edges(f);
        assert_eq!(edges[1], [Some([0, 0]), Some([0, 0])]);
        assert_eq!(edges[0], [None, None]);
        let mut needed = vec![false; 4];
        needed[0] = true;
        needed[3] = true;
        let found = find(&m, &needed, 9);
        assert!(found[0] && found[3]);
    }
}
