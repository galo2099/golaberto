//! Offline necessary-point-floor certificate using the production model.
//! Does not alter reachability labels, sample probabilities or access the DB.
use golaberto_odds::model::{Key, Model, Request};
use serde_json::json;
use std::{collections::BTreeSet, env, fs, time::Instant};

#[derive(Debug)]
struct Cut {
    current: i64,
    required: i64,
    internal: i64,
    boundary: i64,
    wins_without_draws: i64,
}
impl Cut {
    fn incident(&self) -> i64 {
        self.internal + self.boundary
    }
    fn slack(&self) -> i64 {
        self.current + 3 * self.incident() - self.required
    }
    fn minimum_internal_draws(&self) -> i64 {
        (self.wins_without_draws - self.incident()).max(0)
    }
    fn impossible(&self) -> bool {
        // Let d be internal draws, e boundary draws and l boundary losses.
        // Total points require d + 2e + 3l <= slack.
        // Each draw saves at most one required win per participating team:
        // required wins >= wins_without_draws - 2d - e.
        // Available wins <= incident - d - e - l. Hence
        // d >= wins_without_draws - incident + l.
        self.slack() < 0 || self.minimum_internal_draws() > self.slack()
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err(
            "usage: point_floor_certificate REQUEST.json POINT_FLOOR TEAM_IDS_COMMA_SEPARATED"
                .into(),
        );
    }
    let start = Instant::now();
    let request: Request = serde_json::from_slice(&fs::read(&args[1])?)?;
    let rules = &request.phase.championship;
    if (rules.point_win, rules.point_draw, rules.point_loss) != (3, 1, 0)
        || request.phase.bonus_points != 0
    {
        return Err("certificate requires standard 3/1/0 points and no bonus points".into());
    }
    let model = Model::new(request)?;
    if model.keys.first() != Some(&Key::Pt) {
        return Err("certificate requires points as the first sorting criterion".into());
    }
    let floor: i32 = args[2].parse()?;
    let ids: BTreeSet<i32> = args[3]
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    let teams: BTreeSet<usize> = ids
        .iter()
        .map(|id| model.indices.get(id).copied().ok_or("unknown team ID"))
        .collect::<Result<_, _>>()?;
    if teams.is_empty() || teams.iter().any(|t| *t >= model.n) {
        return Err("certificate requires a nonempty set of ranked teams".into());
    }
    let mut cut = Cut {
        current: 0,
        required: i64::from(floor) * teams.len() as i64,
        internal: 0,
        boundary: 0,
        wins_without_draws: 0,
    };
    let mut rows = Vec::new();
    for &t in &teams {
        let c = model.base[t];
        let wins = ((i64::from(floor) - i64::from(c.points)).max(0) + 2) / 3;
        cut.current += i64::from(c.points);
        cut.wins_without_draws += wins;
        let left = model
            .fixtures
            .iter()
            .filter(|g| g.home == t || g.away == t)
            .count();
        rows.push(json!({"team":model.ids[t],"points":c.points,"wins":c.wins,"remaining":left,"wins_needed_without_draws":wins}));
    }
    for g in &model.fixtures {
        match (teams.contains(&g.home), teams.contains(&g.away)) {
            (true, true) => cut.internal += 1,
            (true, false) | (false, true) => cut.boundary += 1,
            _ => {}
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "group":model.request.id,"point_floor":floor,"teams":rows,
            "current_points":cut.current,"required_final_points":cut.required,
            "internal_fixtures":cut.internal,"boundary_fixtures":cut.boundary,
            "incident_fixtures":cut.incident(),
            "maximum_final_points":cut.current+3*cut.incident(),
            "point_slack":cut.slack(),"wins_needed_without_draws":cut.wins_without_draws,
            "minimum_internal_draws":cut.minimum_internal_draws(),
            "all_teams_reaching_floor_impossible":cut.impossible(),
            "meaning":"Infeasibility is a certificate; passing this screen proves neither joint feasibility nor rank reachability.",
            "setup_and_certificate_ms":start.elapsed().as_secs_f64()*1000.
        }))?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eleven_team_certificate_has_contradictory_draw_limits() {
        let cut = Cut {
            current: 380,
            required: 627,
            internal: 32,
            boundary: 51,
            wins_without_draws: 87,
        };
        assert_eq!(cut.slack(), 2);
        assert_eq!(cut.minimum_internal_draws(), 4);
        assert!(cut.impossible());
    }
    #[test]
    fn cut_never_rejects_small_exhaustively_feasible_tournaments() {
        // Teams 0..2 form the coalition; team 3 is outside. Test every outcome
        // of a complete four-team remaining round robin, varied starting points
        // and point floors. This includes internal and boundary draws/losses.
        let games = [(0, 1), (0, 2), (1, 2), (0, 3), (1, 3), (2, 3)];
        for base in [[0, 0, 0], [1, 2, 4], [3, 5, 7], [9, 0, 2]] {
            for floor in 0..=12 {
                let cut = Cut {
                    current: base.iter().sum(),
                    required: floor * 3,
                    internal: 3,
                    boundary: 3,
                    wins_without_draws: base
                        .iter()
                        .map(|p| (floor - p).max(0) + 2)
                        .map(|p| p / 3)
                        .sum(),
                };
                for code in 0..729 {
                    let mut code = code;
                    let mut points = [base[0], base[1], base[2], 0];
                    for &(h, a) in &games {
                        let o = code % 3;
                        code /= 3;
                        points[h] += [0, 1, 3][o];
                        points[a] += [3, 1, 0][o];
                    }
                    if points[..3].iter().all(|p| *p >= floor) {
                        assert!(!cut.impossible(), "base={base:?} floor={floor}");
                    }
                }
            }
        }
    }
}
