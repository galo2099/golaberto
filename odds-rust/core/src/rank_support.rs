//! Conservative discrete rank bounds from all W/D/L outcomes.
use crate::model::{Key, Model};

/// Row-major mask. `true` means the position is proven impossible.
pub fn impossible_ranks(model: &Model) -> Vec<bool> {
    let n = model.n;
    let len = n.saturating_mul(n);
    let mut mask = vec![false; len];
    if n == 0 || model.ids.len() < n || model.request.phase.bonus_points != 0 {
        return mask;
    }
    let rules = &model.request.phase.championship;
    if model.keys.first() != Some(&Key::Pt)
        || (rules.point_win, rules.point_draw, rules.point_loss) != (3, 1, 0)
    {
        return mask;
    }
    let use_wins = model.keys.get(1) == Some(&Key::W);
    let use_dynamic_gd = use_wins && model.keys.get(2) == Some(&Key::Gd);
    let mut remaining = vec![0usize; n];
    for g in &model.fixtures {
        if g.home >= model.ids.len() || g.away >= model.ids.len() || g.home == g.away {
            return mask;
        }
        if g.home < n {
            remaining[g.home] += 1;
        }
        if g.away < n {
            remaining[g.away] += 1;
        }
    }
    let mut extrema = Vec::with_capacity(n);
    let mut state_counts = Vec::with_capacity(n);
    for t in 0..n {
        let Ok(m) = i64::try_from(remaining[t]) else {
            return mask;
        };
        let base = &model.base[t];
        let Some(point_gain) = m.checked_mul(3) else {
            return mask;
        };
        let (Some(max_points), Some(max_wins)) = (
            i64::from(base.points).checked_add(point_gain),
            i64::from(base.wins).checked_add(m),
        ) else {
            return mask;
        };
        extrema.push((
            (i64::from(base.points), i64::from(base.wins)),
            (max_points, max_wins),
        ));
        let count = if use_wins {
            remaining[t]
                .checked_add(1)
                .and_then(|a| remaining[t].checked_add(2).and_then(|b| a.checked_mul(b)))
                .map(|v| v / 2)
                .unwrap_or(usize::MAX)
        } else {
            remaining[t]
                .checked_mul(3)
                .and_then(|v| v.checked_add(1))
                .unwrap_or(usize::MAX)
        };
        state_counts.push(count);
    }
    // Bound total rival/state comparisons before materializing supports. If it
    // would be excessive, use full-schedule min/max intervals below.
    const MAX_COMPARISONS: usize = 40_000_000;
    let work = state_counts.iter().try_fold(0usize, |sum, &count| {
        sum.checked_add(count.checked_mul(n.saturating_sub(1))?)
    });
    let enumerate = work.is_some_and(|w| w <= MAX_COMPARISONS);
    for team in 0..n {
        let (own_min, own_max) = extrema[team];
        let mut possible = vec![false; n];
        if enumerate {
            let Ok(m) = i64::try_from(remaining[team]) else {
                return vec![false; len];
            };
            if use_wins {
                for w in 0..=m {
                    for d in 0..=m - w {
                        let target = (
                            i64::from(model.base[team].points) + 3 * w + d,
                            i64::from(model.base[team].wins) + w,
                        );
                        let losses = m - w - d;
                        let gd_bounds = use_dynamic_gd.then(|| {
                            let base_gd =
                                i64::from(model.base[team].gf) - i64::from(model.base[team].ga);
                            if losses == 0 && w == 0 {
                                (base_gd, base_gd)
                            } else if losses == 0 {
                                (base_gd + w, i64::MAX)
                            } else if w == 0 {
                                (i64::MIN, base_gd - losses)
                            } else {
                                (i64::MIN, i64::MAX)
                            }
                        });
                        mark_interval(
                            model,
                            team,
                            target,
                            use_wins,
                            gd_bounds,
                            &extrema,
                            &remaining,
                            &mut possible,
                        );
                    }
                }
            } else {
                for p in own_min.0..=own_max.0 {
                    // With m unresolved fixtures, 3m−1 points cannot occur:
                    // the only near-maximum outcome has m−1 wins and one
                    // draw, worth 3m−2.
                    if remaining[team] > 0 && p == own_max.0 - 1 {
                        continue;
                    }
                    mark_interval(
                        model,
                        team,
                        (p, 0),
                        false,
                        None,
                        &extrema,
                        &remaining,
                        &mut possible,
                    );
                }
            }
        } else {
            // Sound full-schedule envelope fallback. It keeps all old coarse
            // proofs available without relying on a truncated enumeration.
            let target = if use_wins { own_max } else { (own_max.0, 0) };
            let mut earliest = 0;
            let mut behind = 0;
            for rival in 0..n {
                if rival == team {
                    continue;
                }
                let (mut lo, mut hi) = extrema[rival];
                let (mut tlo, mut thi) = (own_min, own_max);
                if !use_wins {
                    lo.1 = 0;
                    hi.1 = 0;
                    tlo.1 = 0;
                    thi.1 = 0;
                }
                let static_tie =
                    remaining[team] == 0 && remaining[rival] == 0 && tlo == thi && lo == hi;
                if lo > target
                    || (static_tie
                        && lo == target
                        && immutable_tiebreak_ahead(model, rival, team, use_wins))
                {
                    earliest += 1;
                }
                let low_target = if use_wins { tlo } else { (tlo.0, 0) };
                if hi < low_target
                    || (static_tie
                        && hi == low_target
                        && immutable_tiebreak_ahead(model, team, rival, use_wins))
                {
                    behind += 1;
                }
            }
            for rank in earliest..=(n - 1 - behind) {
                possible[rank] = true;
            }
        }
        for rank in 0..n {
            mask[team * n + rank] = !possible[rank];
        }
    }
    mask
}

fn mark_interval(
    model: &Model,
    team: usize,
    target: (i64, i64),
    use_wins: bool,
    gd_bounds: Option<(i64, i64)>,
    extrema: &[((i64, i64), (i64, i64))],
    remaining: &[usize],
    possible: &mut [bool],
) {
    let n = possible.len();
    let target = if use_wins { target } else { (target.0, 0) };
    let (target_lo, target_hi) = extrema[team];
    let target_fixed = remaining[team] == 0 && target_lo == target_hi;
    let mut earliest = 0usize;
    let mut behind = 0usize;
    for rival in 0..n {
        if rival == team {
            continue;
        }
        let (mut lo, mut hi) = extrema[rival];
        if !use_wins {
            lo.1 = 0;
            hi.1 = 0;
        }
        let static_tie = target_fixed && remaining[rival] == 0 && lo == hi && lo == target;
        let fixed_score_tie =
            gd_bounds.is_some() && remaining[rival] == 0 && lo == target && hi == target;
        let gd_order = if fixed_score_tie {
            gd_bounds.and_then(|(gd_min, gd_max)| {
                let rival_gd = i64::from(model.base[rival].gf) - i64::from(model.base[rival].ga);
                if gd_min > rival_gd {
                    Some(std::cmp::Ordering::Greater)
                } else if gd_max < rival_gd {
                    Some(std::cmp::Ordering::Less)
                } else {
                    None
                }
            })
        } else {
            None
        };
        if lo > target
            || gd_order == Some(std::cmp::Ordering::Less)
            || (static_tie && immutable_tiebreak_ahead(model, rival, team, use_wins))
        {
            earliest += 1;
        }
        if hi < target
            || gd_order == Some(std::cmp::Ordering::Greater)
            || (static_tie && immutable_tiebreak_ahead(model, team, rival, use_wins))
        {
            behind += 1;
        }
    }
    let latest = n - 1 - behind;
    for rank in earliest..=latest {
        possible[rank] = true;
    }
}

/// Resolve lower keys only when both campaigns are fully immutable.
fn immutable_tiebreak_ahead(model: &Model, a: usize, b: usize, use_wins: bool) -> bool {
    let (ca, cb) = (&model.base[a], &model.base[b]);
    let lower_keys = if use_wins { 2 } else { 1 };
    for key in model.keys.iter().skip(lower_keys) {
        let ord = match key {
            Key::W => ca.wins.cmp(&cb.wins),
            Key::Gd => (ca.gf - ca.ga).cmp(&(cb.gf - cb.ga)),
            Key::Gf => ca.gf.cmp(&cb.gf),
            Key::Average => {
                let x = ca.gf as f64 / (ca.ga as f64 + 1e-13);
                let y = cb.gf as f64 / (cb.ga as f64 + 1e-13);
                x.total_cmp(&y)
            }
            Key::Away => ca.away.cmp(&cb.away),
            Key::Bias => ca.bias.cmp(&cb.bias),
            Key::Gp | Key::Aet => std::cmp::Ordering::Equal,
            Key::Pt => ca.points.cmp(&cb.points),
            Key::Head | Key::Random | Key::Name => return false,
        };
        if ord != std::cmp::Ordering::Equal {
            return ord == std::cmp::Ordering::Greater;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Championship, Game, Phase, Request, Team};

    fn model(sort: &str, points: &[i32], games: Vec<Game>) -> Model {
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
                    team_id: i as i32,
                    add_sub: *p,
                    bias: 0,
                })
                .collect(),
        })
        .unwrap()
    }
    fn game(h: i32, a: i32, played: bool, hs: i32, as_: i32) -> Game {
        Game {
            id: 0,
            home_id: h,
            away_id: a,
            home_score: hs,
            away_score: as_,
            home_power: 1.,
            away_power: 1.,
            played,
        }
    }
    #[test]
    fn proves_only_strictly_separated_points() {
        let m = model("pt,gd", &[12, 0, 0], vec![game(1, 2, false, 0, 0)]);
        let p = impossible_ranks(&m);
        assert!(p[1 * 3]); // both opponents must remain below team 0
        assert!(!p[0 * 3]);
    }
    #[test]
    fn leaves_equal_points_open_to_tiebreaks() {
        let m = model("pt,gd", &[3, 3], vec![]);
        assert_eq!(impossible_ranks(&m), vec![false; 4]);
    }
    #[test]
    fn discrete_points_keep_a_gap_with_tail_name() {
        let m = model("pt,gd,name", &[0, 2, 2], vec![game(0, 99, false, 0, 0)]);
        let p = impossible_ranks(&m);
        assert!(p[1]); // target can score 0, 1, or 3 points, never 2
        assert!(!p[0]);
        assert!(!p[2]);
    }

    #[test]
    fn draw_only_points_wins_state_can_be_ordered_by_fixed_gd() {
        let mut m = model("pt,w,gd,name", &[0, 1, 2], vec![game(0, 99, false, 0, 0)]);
        m.base[1].gf = 2;
        m.base[1].ga = 1;
        let p = impossible_ranks(&m);
        assert!(p[1]);
    }

    #[test]
    fn winning_score_margin_can_reverse_a_fixed_rivals_gd() {
        let mut m = model("pt,w,gd,name", &[0, 3], vec![game(0, 99, false, 0, 0)]);
        m.base[1].wins = 1;
        m.base[1].gf = 5;
        assert!(!impossible_ranks(&m)[0]);
    }

    #[test]
    fn static_goal_difference_can_resolve_fixed_points() {
        let m = model("pt,gd,name", &[0, 3, 0], vec![game(0, 2, true, 1, 0)]);
        let p = impossible_ranks(&m);
        assert!(p[3]);
        assert!(!p[0]);
    }

    #[test]
    fn real_218_team_fixture_discrete_gap_for_1270() {
        let request: crate::model::Request = serde_json::from_str(include_str!(
            "../../tests/fixtures/group-16687-2026-10-12.json"
        ))
        .unwrap();
        let m = Model::new(request).unwrap();
        assert_eq!(m.n, 218);
        let team = *m.indices.get(&1270).unwrap();
        let mask = impossible_ranks(&m);
        for rank in 75..=103 {
            assert!(mask[team * m.n + rank], "rank {rank}");
        }
        // These positions had positive baseline probability in the fixture.
        for rank in [55usize, 69, 106, 120, 131] {
            assert!(!mask[team * m.n + rank], "feasible rank {rank}");
        }
    }

    #[test]
    fn unsupported_order_and_scoring_are_open() {
        let mut m = model("gd,pt", &[99, 0], vec![]);
        assert_eq!(impossible_ranks(&m), vec![false; 4]);
        m.request.phase.championship.point_win = 2;
        assert_eq!(impossible_ranks(&m), vec![false; 4]);
    }
}
