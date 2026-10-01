//! Offline worst-rank proof diagnostic. No probability or production-label changes.
use golaberto_odds::{
    model::{Key, Model, Request},
    proof::{max_gain, min_gain, Problem},
};
use serde_json::json;
use std::{env, fs, time::Instant};

#[derive(Debug)]
enum Answer {
    Impossible,
    Feasible,
    Unknown,
}
struct Solver {
    nodes: usize,
    budget: usize,
}
impl Solver {
    fn visit(&mut self, p: &Problem, cap: i32, exempt: u64, mut domains: Vec<u8>) -> Answer {
        if self.nodes >= self.budget {
            return Answer::Unknown;
        }
        self.nodes += 1;
        let Some(lower) = p.propagate_policy(cap, exempt, &mut domains, true) else {
            return Answer::Impossible;
        };
        let branch = p
            .games
            .iter()
            .enumerate()
            .filter(|(i, _)| domains[*i].count_ones() > 1)
            .min_by_key(|(i, g)| {
                (
                    domains[*i].count_ones(),
                    [g.home, g.away]
                        .into_iter()
                        .filter(|t| exempt & (1 << t) == 0)
                        .map(|t| cap - lower[t])
                        .min()
                        .unwrap_or(i32::MAX),
                    *i,
                )
            })
            .map(|(i, _)| i);
        let Some(i) = branch else {
            return Answer::Feasible;
        };
        let g = &p.games[i];
        let mut options: Vec<_> = (0..3).filter(|o| domains[i] & (1 << o) != 0).collect();
        options.sort_by_key(|o| {
            (if exempt & (1 << g.home) == 0 {
                g.hg[*o]
            } else {
                0
            }) + (if exempt & (1 << g.away) == 0 {
                g.ag[*o]
            } else {
                0
            })
        });
        for o in options {
            let mut next = domains.clone();
            next[i] = 1 << o;
            match self.visit(p, cap, exempt, next) {
                Answer::Impossible => {}
                other => return other,
            }
        }
        Answer::Impossible
    }
}
fn combinations(
    candidates: &[usize],
    left: usize,
    start: usize,
    selected: &mut Vec<usize>,
    out: &mut Vec<Vec<usize>>,
) {
    if left == 0 {
        out.push(selected.clone());
        return;
    }
    for i in start..=candidates.len() - left {
        selected.push(candidates[i]);
        combinations(candidates, left - 1, i + 1, selected, out);
        selected.pop();
    }
}
fn analyze(
    request: Request,
    team: i32,
    rank: usize,
    wins: bool,
    budget: usize,
) -> Result<serde_json::Value, String> {
    let started = Instant::now();
    let model = Model::new(request)?;
    let target = *model.indices.get(&team).ok_or("unknown team")?;
    if rank == 0 || rank > model.n || target >= model.n {
        return Err("invalid rank or unranked target".into());
    }
    let rules = &model.request.phase.championship;
    if model.keys.first() != Some(&Key::Pt)
        || (rules.point_win, rules.point_draw, rules.point_loss) != (3, 1, 0)
        || model.request.phase.bonus_points != 0
    {
        return Err("requires standard 3/1/0 scoring, no bonus points, points first".into());
    }
    if wins && model.keys.get(1) != Some(&Key::W) {
        return Err("wins must immediately follow points".into());
    }
    let mut remaining = vec![0; model.ids.len()];
    for g in &model.fixtures {
        remaining[g.home] += 1;
        remaining[g.away] += 1;
    }
    let stride = if wins {
        model
            .base
            .iter()
            .enumerate()
            .map(|(t, c)| c.wins + remaining[t])
            .max()
            .unwrap()
            + 1
    } else {
        1
    };
    let setup = Instant::now();
    let mut p = Problem::new(&model).ok_or("unsupported model")?;
    if wins {
        // The production discrete cut currently assumes raw 3/1/0 gains;
        // disable it for this lexicographic point/win encoding.
        p = p.with_discrete_cuts(false);
        p.base = model
            .base
            .iter()
            .map(|c| {
                c.points
                    .checked_mul(stride)
                    .and_then(|v| v.checked_add(c.wins))
                    .ok_or("encoded points overflow")
            })
            .collect::<Result<_, _>>()?;
        let win = 3_i32
            .checked_mul(stride)
            .and_then(|v| v.checked_add(1))
            .ok_or("encoded gain overflow")?;
        for g in &mut p.games {
            g.hg = [0, stride, win];
            g.ag = [win, stride, 0];
        }
    }
    p = p.negated();
    let cap = p.target_max(target);
    let mut domains = vec![7; p.games.len()];
    // Necessary worst-rank relaxation: turn every target result into a loss.
    // Its P/W decrease and each affected rival's P/W increase. Consequently
    // every rival ahead before the change remains >= the target afterward.
    // Equality is generously counted as ahead, without restricting any goals.
    for (i, g) in p.games.iter().enumerate() {
        let gain = if g.home == target {
            Some(g.hg)
        } else if g.away == target {
            Some(g.ag)
        } else {
            None
        };
        if let Some(gain) = gain {
            let smallest_actual = max_gain(7, gain);
            domains[i] = (0..3).fold(0, |d, o| {
                if gain[o] == smallest_actual {
                    d | (1 << o)
                } else {
                    d
                }
            });
        }
    }
    let mut actual_max = p.base.clone();
    for (g, d) in p.games.iter().zip(&domains) {
        actual_max[g.home] += min_gain(*d, g.hg);
        actual_max[g.away] += min_gain(*d, g.ag);
    }
    let eligible: Vec<_> = (0..model.n)
        .filter(|t| *t != target && actual_max[*t] <= cap)
        .collect();
    let mut cohorts = Vec::new();
    if eligible.len() >= rank - 1 {
        combinations(&eligible, rank - 1, 0, &mut Vec::new(), &mut cohorts);
    }
    let setup_ms = setup.elapsed().as_secs_f64() * 1000.;
    let search = Instant::now();
    let all = if model.ids.len() == 64 {
        u64::MAX
    } else {
        (1_u64 << model.ids.len()) - 1
    };
    let mut solver = Solver { nodes: 0, budget };
    let mut records = Vec::new();
    let mut impossible = true;
    for cohort in &cohorts {
        let exempt = cohort.iter().fold(all, |m, t| m & !(1 << t));
        let before = solver.nodes;
        let answer = solver.visit(&p, cap, exempt, domains.clone());
        records.push(json!({"required":cohort.iter().map(|t|model.ids[*t]).collect::<Vec<_>>(),"status":format!("{answer:?}"),"nodes":solver.nodes-before}));
        if !matches!(answer, Answer::Impossible) {
            impossible = false;
            break;
        }
    }
    Ok(
        json!({"group":model.request.id,"team":team,"rank":rank,"wins_aware":wins,"stride":stride,"points":model.base[target].points,"wins":model.base[target].wins,"eligible":eligible.iter().map(|t|model.ids[*t]).collect::<Vec<_>>(),"cohorts_total":cohorts.len(),"cohorts_tested":records.len(),"nodes":solver.nodes,"node_budget":budget,"impossible_points_wins_relaxation":impossible,"records":records,"search_ms":search.elapsed().as_secs_f64()*1000.,"proof_setup_ms":setup_ms,"total_ms":started.elapsed().as_secs_f64()*1000.}),
    )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 4 {
        return Err(
            "usage: rank_floor_analysis REQUEST TEAM RANK [NODE_BUDGET] [points-only]".into(),
        );
    }
    let request: Request = serde_json::from_slice(&fs::read(&args[1])?)?;
    let result = analyze(
        request,
        args[2].parse()?,
        args[3].parse()?,
        args.get(5).is_none_or(|v| v != "points-only"),
        args.get(4)
            .map(|v| v.parse())
            .transpose()?
            .unwrap_or(200000),
    )?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Request {
        serde_json::from_str(include_str!("../../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json")).unwrap()
    }
    #[test]
    fn bounded_wins_proof_refutes_all_cohorts() {
        let r = analyze(request(), 17, 14, true, 200000).unwrap();
        assert_eq!(r["cohorts_total"], 105);
        assert_eq!(r["cohorts_tested"], 105);
        assert_eq!(r["impossible_points_wins_relaxation"], true);
    }
    #[test]
    fn exhausted_budget_is_unknown() {
        let r = analyze(request(), 17, 14, true, 0).unwrap();
        assert_eq!(r["impossible_points_wins_relaxation"], false);
        assert_eq!(r["records"][0]["status"], "Unknown");
    }
    #[test]
    fn matches_exhaustive_small_leagues_including_all_target_results() {
        for adjustment in [0, 1, 3, 6] {
            let request: Request = serde_json::from_value(json!({
                "id":1,"zones":[],
                "phase":{"sort":"pt,w,gd,gf", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
                "team_groups":[{"team_id":1,"add_sub":adjustment},{"team_id":2},{"team_id":3},{"team_id":4}],
                "games":[
                    {"id":1,"home_id":1,"away_id":2,"played":true,"home_score":1,"away_score":0},
                    {"id":2,"home_id":1,"away_id":3},
                    {"id":3,"home_id":2,"away_id":4},
                    {"id":4,"home_id":2,"away_id":3},
                    {"id":5,"home_id":3,"away_id":4}
                ]
            })).unwrap();
            let model = Model::new(request.clone()).unwrap();
            for wins in [false, true] {
                let mut max_ahead = vec![0; model.n];
                for mut code in 0..3_usize.pow(model.fixtures.len() as u32) {
                    let mut points: Vec<_> = model.base.iter().map(|c| c.points).collect();
                    let mut victories: Vec<_> = model.base.iter().map(|c| c.wins).collect();
                    for g in &model.fixtures {
                        let o = code % 3;
                        code /= 3;
                        points[g.home] += [0, 1, 3][o];
                        points[g.away] += [3, 1, 0][o];
                        victories[g.home] += i32::from(o == 2);
                        victories[g.away] += i32::from(o == 0);
                    }
                    for t in 0..model.n {
                        let ahead = (0..model.n)
                            .filter(|&other| {
                                other != t
                                    && if wins {
                                        (points[other], victories[other])
                                            >= (points[t], victories[t])
                                    } else {
                                        points[other] >= points[t]
                                    }
                            })
                            .count();
                        max_ahead[t] = max_ahead[t].max(ahead);
                    }
                }
                for (t, &ahead) in max_ahead.iter().enumerate() {
                    for rank in 1..=model.n {
                        let result =
                            analyze(request.clone(), model.ids[t], rank, wins, 200000).unwrap();
                        assert_eq!(
                            result["impossible_points_wins_relaxation"],
                            ahead < rank - 1,
                            "adjustment={adjustment} wins={wins} team={t} rank={rank}"
                        );
                    }
                }
            }
        }
    }
}
