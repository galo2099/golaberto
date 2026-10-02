//! Offline diagnostic for certified target paths and shared-fixture case pruning.
//! No production flags, response changes or database access.
use golaberto_odds::{
    domains::Domains,
    lookahead::RankGame,
    model::{Key, Model, Request},
    search::{parallel, Cell},
    target_limits,
};
use serde_json::json;
use std::{env, fs, time::Instant};
fn masks(
    games: &[RankGame],
    points: &[i32],
    rivals: &[usize],
    rank: usize,
    target: i32,
    ambiguous: &[usize],
    above: bool,
    i: usize,
    left: usize,
    mask: u32,
    initial: &Domains,
    nodes: &mut usize,
    pruned: &mut usize,
    out: &mut Vec<u32>,
    quota: usize,
) -> bool {
    if *nodes >= quota {
        return false;
    }
    *nodes += 1;
    if i == ambiguous.len() {
        out.push(mask);
        return true;
    }
    let t = ambiguous[i];
    for strict in [false, true] {
        if strict && left == 0 {
            continue;
        }
        let mut lower = initial.lower.clone();
        let mut upper = initial.upper.clone();
        if above == strict {
            lower[t] = lower[t].max(target + i32::from(strict));
        } else {
            upper[t] = upper[t].min(target - i32::from(strict));
        }
        let d = Domains::condition(
            games,
            points,
            rivals,
            rank,
            target,
            &lower,
            &upper,
            &initial.domains,
        );
        if !d.feasible {
            *pruned += 1;
            continue;
        }
        if !masks(
            games,
            points,
            rivals,
            rank,
            target,
            ambiguous,
            above,
            i + 1,
            left - usize::from(strict),
            mask | if strict { 1 << t } else { 0 },
            &d,
            nodes,
            pruned,
            out,
            quota,
        ) {
            return false;
        }
    }
    true
}
fn combinations(n: usize, k: usize) -> usize {
    let mut c = 1;
    let mut sum = 1;
    for j in 1..=k.min(n) {
        c = c * (n - j + 1) / j;
        sum += c;
    }
    sum
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    if a.len() < 4 {
        return Err("certified_paths REQUEST TEAM RANK [NODES_PER_PATH]".into());
    }
    let m = Model::new(serde_json::from_slice::<Request>(&fs::read(&a[1])?)?)?;
    let rules = &m.request.phase.championship;
    if m.keys.first() != Some(&Key::Pt)
        || (rules.point_win, rules.point_draw, rules.point_loss) != (3, 1, 0)
        || m.request.phase.bonus_points != 0
        || m.ids.len() != m.n
        || m.n > 32
    {
        return Err("diagnostic supports points-first 3/1/0 phases, at most 32 teams".into());
    }
    let cell = Cell {
        team: m.indices[&a[2].parse()?],
        rank: a[3].parse::<usize>()? - 1,
    };
    let quota = a.get(4).map(|s| s.parse()).transpose()?.unwrap_or(10000);
    let start = Instant::now();
    let c = target_limits::certify(&m, cell, 16000);
    let proof_ms = start.elapsed().as_secs_f64() * 1000.;
    let mut left = vec![0; m.n];
    for g in &m.fixtures {
        left[g.home] += 1;
        left[g.away] += 1;
    }
    let wins = m.keys.get(1) == Some(&Key::W);
    let stride = if wins {
        m.base
            .iter()
            .zip(&left)
            .map(|(c, l)| c.wins + l)
            .max()
            .unwrap()
            + 1
    } else {
        1
    };
    let base: Vec<_> = m
        .base
        .iter()
        .map(|c| c.points * stride + if wins { c.wins } else { 0 })
        .collect();
    let win = 3 * stride + i32::from(wins);
    let all: Vec<_> = m
        .fixtures
        .iter()
        .enumerate()
        .map(|(index, g)| RankGame {
            index,
            home: g.home,
            away: g.away,
            prob: g.prob,
            hg: [0, stride, win],
            ag: [win, stride, 0],
        })
        .collect();
    let (target, remaining): (Vec<_>, Vec<_>) = all
        .into_iter()
        .partition(|g| g.home == cell.team || g.away == cell.team);
    let mut paths = vec![(base.clone(), Vec::new(), 1.)];
    for g in &target {
        let mut next = Vec::new();
        for (points, key, mass) in paths {
            for o in 0..3 {
                let mut p = points.clone();
                p[g.home] += g.hg[o];
                p[g.away] += g.ag[o];
                if !c.upper.is_none_or(|u| i64::from(p[cell.team]) <= u) || g.prob[o] == 0. {
                    continue;
                }
                let mut k = key.clone();
                k.push(o as u8);
                next.push((p, k, mass * g.prob[o]));
            }
        }
        paths = next;
        if paths.len() > 100000 {
            return Err("offline target path budget".into());
        }
    }
    paths.retain(|(p, _, _)| c.allows(i64::from(p[cell.team])));
    let start = Instant::now();
    let rows = parallel(paths.len(), 4, |i| {
        let clock = Instant::now();
        let (points, key, mass) = &paths[i];
        let total = points[cell.team];
        let ts: Vec<_> = (0..m.n).filter(|&t| t != cell.team).collect();
        let d = Domains::propagate(&remaining, points, &ts, cell.rank, total);
        if !d.feasible {
            return json!({"path":key,"prior":mass,"feasible":false});
        }
        let fixed_above = ts
            .iter()
            .filter(|&&t| points[t] + d.min[0][t] > total)
            .count();
        let fixed_below = ts
            .iter()
            .filter(|&&t| points[t] + d.max[0][t] < total)
            .count();
        let above = cell.rank - fixed_above <= m.n - 1 - cell.rank - fixed_below;
        let left = if above {
            cell.rank - fixed_above
        } else {
            m.n - 1 - cell.rank - fixed_below
        };
        let ambiguous: Vec<_> = ts
            .iter()
            .copied()
            .filter(|&t| points[t] + d.min[0][t] <= total && points[t] + d.max[0][t] >= total)
            .collect();
        let (mut nodes, mut pruned, mut cases) = (0, 0, Vec::new());
        let complete = masks(
            &remaining,
            points,
            &ts,
            cell.rank,
            total,
            &ambiguous,
            above,
            0,
            left,
            0,
            &d,
            &mut nodes,
            &mut pruned,
            &mut cases,
            quota,
        );
        json!({"path":key,"target_points":total/stride,"target_wins":total%stride,"prior":mass,"feasible":true,"fixed_above":fixed_above,"fixed_below":fixed_below,"above":above,"exceptions":left,"ambiguous":ambiguous.iter().map(|&t|m.ids[t]).collect::<Vec<_>>(),"raw_cases":combinations(ambiguous.len(),left),"complete":complete,"nodes":nodes,"pruned_subtrees":pruned,"cases":cases.iter().map(|mask|ambiguous.iter().filter(|&&t|mask&(1<<t)!=0).map(|&t|m.ids[t]).collect::<Vec<_>>()).collect::<Vec<_>>(),"elapsed_ms":clock.elapsed().as_secs_f64()*1000.})
    });
    println!(
        "{}",
        json!({"group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,"points":m.base[cell.team].points,"wins":m.base[cell.team].wins,"remaining":target.len(),"stride":stride,"certified":{"lower":c.lower,"upper":c.upper,"nodes":c.nodes,"elapsed_ms":proof_ms},"paths":paths.len(),"target_mass":paths.iter().map(|p|p.2).sum::<f64>(),"pruning_ms":start.elapsed().as_secs_f64()*1000.,"workers":4,"rows":rows})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_pruning_preserves_every_complete_rank_assignment() {
        // Target team zero has fixed five points. Enumerate all 81 outcomes
        // among the four rivals, including ties, and check every rank.
        let points = [5, 2, 3, 4, 5];
        let rivals = [1, 2, 3, 4];
        let games: Vec<_> = [(1, 2), (2, 3), (3, 4), (4, 1)]
            .into_iter()
            .enumerate()
            .map(|(index, (home, away))| RankGame {
                index,
                home,
                away,
                prob: [0.4, 0.2, 0.4],
                hg: [0, 1, 3],
                ag: [3, 1, 0],
            })
            .collect();
        for rank in 0..points.len() {
            let d = Domains::propagate(&games, &points, &rivals, rank, points[0]);
            let mut cases = Vec::new();
            let mut ambiguous = Vec::new();
            let mut above = false;
            if d.feasible {
                let fixed_above = rivals
                    .iter()
                    .filter(|&&t| points[t] + d.min[0][t] > points[0])
                    .count();
                let fixed_below = rivals
                    .iter()
                    .filter(|&&t| points[t] + d.max[0][t] < points[0])
                    .count();
                above = rank - fixed_above <= rivals.len() - rank - fixed_below;
                let left = if above {
                    rank - fixed_above
                } else {
                    rivals.len() - rank - fixed_below
                };
                ambiguous = rivals
                    .iter()
                    .copied()
                    .filter(|&t| {
                        points[t] + d.min[0][t] <= points[0] && points[t] + d.max[0][t] >= points[0]
                    })
                    .collect();
                assert!(masks(
                    &games, &points, &rivals, rank, points[0], &ambiguous, above, 0, left, 0, &d,
                    &mut 0, &mut 0, &mut cases, 10000,
                ));
            }
            for code in 0..3usize.pow(games.len() as u32) {
                let mut code = code;
                let mut final_points = points;
                for g in &games {
                    let o = code % 3;
                    code /= 3;
                    final_points[g.home] += g.hg[o];
                    final_points[g.away] += g.ag[o];
                }
                let mut order: Vec<_> = (0..points.len()).collect();
                order.sort_by_key(|&t| (-final_points[t], t));
                if order[rank] != 0 {
                    continue;
                }
                let mask = ambiguous.iter().fold(0, |mask, &t| {
                    let strict = if above {
                        final_points[t] > final_points[0]
                    } else {
                        final_points[t] < final_points[0]
                    };
                    mask | if strict { 1 << t } else { 0 }
                });
                assert!(d.feasible && cases.contains(&mask), "lost rank {rank}");
            }
        }
    }

    #[test]
    fn exhausted_prefix_budget_is_reported_incomplete() {
        let g = RankGame {
            index: 0,
            home: 1,
            away: 2,
            prob: [0.4, 0.2, 0.4],
            hg: [0, 1, 3],
            ag: [3, 1, 0],
        };
        let points = [5, 3, 3];
        let d = Domains::propagate(&[g], &points, &[1, 2], 1, 5);
        assert!(d.feasible);
        let mut cases = Vec::new();
        assert!(!masks(
            &[g],
            &points,
            &[1, 2],
            1,
            5,
            &[1, 2],
            true,
            0,
            1,
            0,
            &d,
            &mut 0,
            &mut 0,
            &mut cases,
            1,
        ));
        assert!(cases.is_empty());
    }
}
