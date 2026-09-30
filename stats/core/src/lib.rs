//! Shared player-rating formulas from the legacy stats service.
//! Database access and persistence belong to the calling service.
use chrono::{NaiveDate, NaiveDateTime};
use smallvec::SmallVec;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

const AVG_BASE: f32 = 1.335_025_8;
const HOME_ADV: f32 = 0.161_336_76;

#[derive(Clone, Debug)]
pub struct Game {
    pub id: i32,
    pub home_id: i32,
    pub away_id: i32,
    pub date: NaiveDateTime,
    pub home_field: i32,
    pub home_aet: Option<i32>,
}
#[derive(Clone, Debug)]
pub struct Goal {
    pub player_id: i32,
    pub team_id: i32,
    pub time: i32,
    pub penalty: bool,
    pub own_goal: bool,
}
#[derive(Clone, Debug)]
pub struct HistoricalRating {
    pub team_id: i32,
    pub measure_date: NaiveDate,
    pub off_rating: f32,
    pub def_rating: f32,
}
#[derive(Clone, Debug)]
pub struct PlayerGame {
    pub id: i32,
    pub game_id: i32,
    pub player_id: i32,
    pub team_id: i32,
    pub on: i32,
    pub off: i32,
    pub red: bool,
}
#[derive(Clone, Debug)]
pub struct PlayerGamePos {
    pub pg: PlayerGame,
    pub pos: String,
}
#[derive(Clone, Debug)]
pub struct PlayerRating {
    pub off: f32,
    pub def: f32,
    pub minutes: f32,
}
#[derive(Default, Debug)]
pub struct Computed {
    pub player_ratings: HashMap<i32, PlayerRating>,
    pub player_game_ratings: HashMap<(i32, i32), PlayerRating>,
}

/// Use one explicit clock for the entire calculation, including reproducible
/// comparisons. Preserve the legacy f32 arithmetic and chronological accumulation.
#[allow(deprecated)] // NaiveDateTime::timestamp also supports stats' locked chrono 0.4.23.
pub fn calculate(
    games: &[Game],
    goals: &HashMap<i32, SmallVec<[Goal; 4]>>,
    mut ratings: HashMap<i32, VecDeque<HistoricalRating>>,
    players: &HashMap<i32, Vec<PlayerGamePos>>,
    now: i64,
) -> Result<Computed, String> {
    if games.windows(2).any(|p| p[0].date > p[1].date) {
        return Err("player-rating games must be chronological".into());
    }
    let mut checked_histories = HashSet::new();
    for game in games {
        let Some(entries) = players.get(&game.id) else {
            continue;
        };
        if !(0..=2).contains(&game.home_field) {
            return Err(format!("invalid home field for game {}", game.id));
        }
        for id in [game.home_id, game.away_id] {
            if !checked_histories.insert(id) {
                continue;
            }
            let Some(history) = ratings.get(&id) else {
                return Err(format!("missing rating history for team {id}"));
            };
            if history.is_empty()
                || history
                    .iter()
                    .zip(history.iter().skip(1))
                    .any(|(a, b)| a.measure_date > b.measure_date)
            {
                return Err(format!("invalid rating history for team {id}"));
            }
        }
        for entry in entries {
            if !["g", "dc", "dl", "dr", "cm", "fw", ""].contains(&entry.pos.as_str()) {
                return Err(format!("invalid player appearance {}", entry.pg.id));
            }
        }
    }
    let mut player_ratings = HashMap::<i32, PlayerRating>::new();
    let mut player_game_ratings = HashMap::<(i32, i32), PlayerRating>::new();
    for game in games.iter() {
        if players.get(&game.id).is_none() {
            continue;
        }

        let home_adv: f32 = match game.home_field {
            0 => HOME_ADV,
            1 => 0.0,
            2 => -HOME_ADV,
            _ => panic!("invalid home_adv"),
        };

        let weight = squash_date(game.date.timestamp(), now);

        let length = match game.home_aet {
            Some(_) => 120.0,
            None => 90.0,
        };

        get_rating(&mut ratings, game.date.date(), game.home_id);
        get_rating(&mut ratings, game.date.date(), game.away_id);

        let home_rating: &HistoricalRating = ratings[&game.home_id].front().unwrap();
        let away_rating: &HistoricalRating = ratings[&game.away_id].front().unwrap();

        let home_for_zero_per90 = -(away_rating.def_rating + home_adv)
            / ((away_rating.def_rating + home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548)
            / length
            / 11.0;
        let home_for_goal_weight = 1.0 / ((away_rating.def_rating + home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548);
        let home_agg_zero_per90 = (away_rating.off_rating - home_adv)
            / ((away_rating.off_rating - home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548)
            / length
            / 11.0;
        let home_agg_goal_weight = -1.0 / ((away_rating.off_rating - home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548);

        let away_for_zero_per90 = -(home_rating.def_rating - home_adv)
            / ((home_rating.def_rating - home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548)
            / length
            / 11.0;
        let away_for_goal_weight = 1.0 / ((home_rating.def_rating - home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548);
        let away_agg_zero_per90 = (home_rating.off_rating + home_adv)
            / ((home_rating.off_rating + home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548)
            / length
            / 11.0;
        let away_agg_goal_weight = -1.0 / ((home_rating.off_rating + home_adv) * 0.424 + 0.548)
            * (AVG_BASE * 0.424 + 0.548);

        let home_players = players[&game.id]
            .iter()
            .filter(|x| x.pg.team_id == game.home_id)
            .collect::<Vec<_>>();
        let away_players = players[&game.id]
            .iter()
            .filter(|x| x.pg.team_id == game.away_id)
            .collect::<Vec<_>>();

        let empty_vec = SmallVec::<[_; 4]>::new();
        let home_goals = goals
            .get(&game.id)
            .unwrap_or(&empty_vec)
            .iter()
            .filter(|&goal| {
                (goal.team_id == game.away_id && goal.own_goal)
                    || (goal.team_id == game.home_id && !goal.own_goal)
            })
            .collect::<SmallVec<[_; 4]>>();
        let away_goals = goals
            .get(&game.id)
            .unwrap_or(&empty_vec)
            .iter()
            .filter(|&goal| {
                (goal.team_id == game.home_id && goal.own_goal)
                    || (goal.team_id == game.away_id && !goal.own_goal)
            })
            .collect::<SmallVec<[_; 4]>>();

        let intervals = home_players
            .iter()
            .flat_map(|x| [x.pg.on, x.pg.off])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut off_penalty = 0.0;
        let mut def_penalty = 0.0;
        for pair in intervals.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            let off_penalty_interval = off_penalty;
            let def_penalty_interval = def_penalty;
            let hp = home_players
                .iter()
                .copied()
                .filter(|x| std::cmp::max(from, x.pg.on) < std::cmp::min(to, x.pg.off))
                .collect::<Vec<_>>();
            let pos = hp.iter().fold(
                HashMap::from(["g", "dc", "cm", "fw", ""].map(|x| (x, 0.0))),
                |mut h, x| {
                    *h.entry(&x.pos).or_default() += 1.0;
                    h
                },
            );
            let off_windividual = hp.len() as f32
                / (pos["g"] * 0.3 + pos["dc"] * 0.7 + pos["cm"] + pos["fw"] * 1.0 + pos[""]);
            let off_w = HashMap::from([
                ("g", off_windividual * 0.3),
                ("dc", off_windividual * 0.7),
                ("dl", off_windividual * 0.7),
                ("dr", off_windividual * 0.7),
                ("cm", off_windividual),
                ("fw", off_windividual * 1.0),
                ("", off_windividual),
            ]);
            let def_windividual = hp.len() as f32
                / (pos["g"] * 4.0 + pos["dc"] * 2.0 + pos["cm"] + pos["fw"] * 0.5 + pos[""]);
            let def_w = HashMap::from([
                ("g", def_windividual * 4.0),
                ("dc", def_windividual * 2.0),
                ("dr", def_windividual * 2.0),
                ("dl", def_windividual * 2.0),
                ("cm", def_windividual),
                ("fw", def_windividual * 0.5),
                ("", def_windividual),
            ]);

            let home_goals_interval = home_goals
                .iter()
                .copied()
                .filter(|g| goal_interval_filter(g, from, to))
                .collect::<SmallVec<[_; 4]>>();
            let away_goals_interval = away_goals
                .iter()
                .filter(|g| goal_interval_filter(g, from, to))
                .count();
            let home_goals_own = home_goals_interval.iter().filter(|g| g.own_goal).count();
            let home_goals_regular = home_goals_interval
                .iter()
                .copied()
                .filter(|g| !g.own_goal && !g.penalty)
                .collect::<SmallVec<[_; 4]>>();
            let home_goals_penalty = home_goals_interval
                .iter()
                .copied()
                .filter(|g| !g.own_goal && g.penalty)
                .collect::<SmallVec<[_; 4]>>();

            for &v in &hp {
                let player_rating = player_ratings
                    .entry(v.pg.player_id)
                    .or_insert(PlayerRating {
                        off: 0.0,
                        def: 0.0,
                        minutes: 0.0,
                    });
                let player_game_rating = player_game_ratings
                    .entry((v.pg.id, v.pg.game_id))
                    .or_insert(PlayerRating {
                        off: 0.0,
                        def: 0.0,
                        minutes: 0.0,
                    });
                if v.pg.off == to && v.pg.red {
                    let red_penalty_per90 = red_card_penalty_remaining_per90(length, v.pg.off);
                    player_game_rating.off -= 0.3 * red_penalty_per90;
                    player_game_rating.def -= 0.5 * red_penalty_per90;
                    player_rating.off -= 0.3 * red_penalty_per90 * weight;
                    player_rating.def -= 0.5 * red_penalty_per90 * weight;
                    off_penalty += 0.3 / 90.0;
                    def_penalty += 0.5 / 90.0;
                }
                let minutes = (to - from) as f32;
                player_rating.minutes += minutes * weight;
                player_game_rating.minutes += minutes;
                let off_player_weight = off_w[&*v.pos];
                let regular_goals = home_goals_regular
                    .iter()
                    .filter(|x| x.player_id == v.pg.player_id)
                    .count();
                let penalty_goals = home_goals_penalty
                    .iter()
                    .filter(|x| x.player_id == v.pg.player_id)
                    .count();

                let off = off_penalty_interval * minutes / (hp.len() as f32)
                    + minutes * home_for_zero_per90 * off_player_weight
                    + home_goals_own as f32 * home_for_goal_weight * off_player_weight
                        / (hp.len() as f32)
                    + (home_goals_regular.len() as f32) * home_for_goal_weight * off_player_weight
                        / (hp.len() as f32)
                        / 4.0
                        * 3.0
                    + (home_goals_penalty.len() as f32) * home_for_goal_weight * off_player_weight
                        / (hp.len() as f32)
                        / 6.0
                        * 5.0
                    + regular_goals as f32 * home_for_goal_weight / 4.0
                    + penalty_goals as f32 * home_for_goal_weight / 6.0;
                let def = def_penalty_interval * minutes / (hp.len() as f32)
                    + (minutes * home_agg_zero_per90
                        + away_goals_interval as f32 * home_agg_goal_weight / (hp.len() as f32))
                        * def_w[&*v.pos];
                player_rating.off += off * weight;
                player_rating.def += def * weight;
                player_game_rating.off += off;
                player_game_rating.def += def;
            }
        }

        let intervals = away_players
            .iter()
            .flat_map(|x| [x.pg.on, x.pg.off])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut off_penalty = 0.0;
        let mut def_penalty = 0.0;
        for pair in intervals.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            let off_penalty_interval = off_penalty;
            let def_penalty_interval = def_penalty;
            let ap = away_players
                .iter()
                .copied()
                .filter(|x| std::cmp::max(from, x.pg.on) < std::cmp::min(to, x.pg.off))
                .collect::<Vec<_>>();
            let pos = ap.iter().fold(
                HashMap::from(["g", "dc", "cm", "fw", ""].map(|x| (x, 0.0))),
                |mut h, x| {
                    *h.entry(&x.pos).or_default() += 1.0;
                    h
                },
            );
            let off_windividual = ap.len() as f32
                / (pos["g"] * 0.3 + pos["dc"] * 0.7 + pos["cm"] + pos["fw"] * 1.0 + pos[""]);
            let off_w = HashMap::from([
                ("g", off_windividual * 0.3),
                ("dc", off_windividual * 0.7),
                ("dl", off_windividual * 0.7),
                ("dr", off_windividual * 0.7),
                ("cm", off_windividual),
                ("fw", off_windividual * 1.0),
                ("", off_windividual),
            ]);
            let def_windividual = ap.len() as f32
                / (pos["g"] * 4.0 + pos["dc"] * 2.0 + pos["cm"] + pos["fw"] * 0.5 + pos[""]);
            let def_w = HashMap::from([
                ("g", def_windividual * 4.0),
                ("dc", def_windividual * 2.0),
                ("dr", def_windividual * 2.0),
                ("dl", def_windividual * 2.0),
                ("cm", def_windividual),
                ("fw", def_windividual * 0.5),
                ("", def_windividual),
            ]);

            let away_goals_interval = away_goals
                .iter()
                .copied()
                .filter(|g| goal_interval_filter(g, from, to))
                .collect::<SmallVec<[_; 4]>>();
            let home_goals_interval = home_goals
                .iter()
                .filter(|g| goal_interval_filter(g, from, to))
                .count();
            let away_goals_own = away_goals_interval.iter().filter(|g| g.own_goal).count();
            let away_goals_regular = away_goals_interval
                .iter()
                .copied()
                .filter(|g| !g.own_goal && !g.penalty)
                .collect::<SmallVec<[_; 4]>>();
            let away_goals_penalty = away_goals_interval
                .iter()
                .copied()
                .filter(|g| !g.own_goal && g.penalty)
                .collect::<SmallVec<[_; 4]>>();

            for &v in &ap {
                let player_rating = player_ratings
                    .entry(v.pg.player_id)
                    .or_insert(PlayerRating {
                        off: 0.0,
                        def: 0.0,
                        minutes: 0.0,
                    });
                let player_game_rating = player_game_ratings
                    .entry((v.pg.id, v.pg.game_id))
                    .or_insert(PlayerRating {
                        off: 0.0,
                        def: 0.0,
                        minutes: 0.0,
                    });
                if v.pg.off == to && v.pg.red {
                    let red_penalty_per90 = red_card_penalty_remaining_per90(length, v.pg.off);
                    player_game_rating.off -= 0.3 * red_penalty_per90;
                    player_game_rating.def -= 0.5 * red_penalty_per90;
                    player_rating.off -= 0.3 * red_penalty_per90 * weight;
                    player_rating.def -= 0.5 * red_penalty_per90 * weight;
                    off_penalty += 0.3 / 90.0;
                    def_penalty += 0.5 / 90.0;
                }
                let minutes = (to - from) as f32;
                player_rating.minutes += minutes * weight;
                player_game_rating.minutes += minutes;
                let off_player_weight = off_w[&*v.pos];
                let regular_goals = away_goals_regular
                    .iter()
                    .filter(|x| x.player_id == v.pg.player_id)
                    .count();
                let penalty_goals = away_goals_penalty
                    .iter()
                    .filter(|x| x.player_id == v.pg.player_id)
                    .count();

                let off = off_penalty_interval * minutes / (ap.len() as f32)
                    + minutes * away_for_zero_per90 * off_player_weight
                    + away_goals_own as f32 * away_for_goal_weight * off_player_weight
                        / (ap.len() as f32)
                    + (away_goals_regular.len() as f32) * away_for_goal_weight * off_player_weight
                        / (ap.len() as f32)
                        / 4.0
                        * 3.0
                    + (away_goals_penalty.len() as f32) * away_for_goal_weight * off_player_weight
                        / (ap.len() as f32)
                        / 6.0
                        * 5.0
                    + regular_goals as f32 * away_for_goal_weight / 4.0
                    + penalty_goals as f32 * away_for_goal_weight / 6.0;
                let def = def_penalty_interval * minutes / (ap.len() as f32)
                    + (minutes * away_agg_zero_per90
                        + home_goals_interval as f32 * away_agg_goal_weight / (ap.len() as f32))
                        * def_w[&*v.pos];
                player_rating.off += off * weight;
                player_rating.def += def * weight;
                player_game_rating.off += off;
                player_game_rating.def += def;
            }
        }
    }

    for value in player_ratings.values().chain(player_game_ratings.values()) {
        if value.minutes <= 0.
            || !value.minutes.is_finite()
            || !value.off.is_finite()
            || !value.def.is_finite()
        {
            return Err("non-finite player rating or zero playing time".into());
        }
    }
    Ok(Computed {
        player_ratings,
        player_game_ratings,
    })
}

fn squash_date(timestamp: i64, now: i64) -> f32 {
    use std::f32::consts::E;
    let x = (timestamp - now) as f32 / (730.0 * 24.0 * 60.0 * 60.0);
    1.0 + (E.powf(x) - E.powf(-x)) / (E.powf(x) + E.powf(-x))
}

pub fn squash_rating(minutes: f32) -> f32 {
    use std::f32::consts::E;
    1.0 / (1.0 + E.powf(-(minutes - 2000.0) / 400.0))
}

fn goal_interval_filter(g: &Goal, from: i32, to: i32) -> bool {
    g.time >= from && (g.time < to || (g.time == 90 && to == 90) || (g.time == 45 && to == 45))
}

pub fn red_card_penalty_remaining_per90(game_length: f32, player_off: i32) -> f32 {
    // Red-card penalties are calibrated per 90 minutes, not per match. An overtime
    // match can therefore charge more than a normal match's full-game penalty when
    // the player misses more than 90 minutes.
    ((game_length - player_off as f32) / 90.0).max(0.0)
}

fn get_rating(ratings: &mut HashMap<i32, VecDeque<HistoricalRating>>, date: NaiveDate, id: i32) {
    let r = ratings.get_mut(&id).expect("");
    while r.len() > 1 && date > r[1].measure_date {
        r.pop_front();
    }
}

#[cfg(test)]
mod tests {
    use super::red_card_penalty_remaining_per90;

    #[test]
    fn red_card_penalty_uses_regular_time_remaining_per90() {
        assert_eq!(red_card_penalty_remaining_per90(90.0, 60), 30.0 / 90.0);
    }

    #[test]
    fn red_card_penalty_includes_overtime_remaining_per90() {
        assert_eq!(red_card_penalty_remaining_per90(120.0, 60), 60.0 / 90.0);
    }

    #[test]
    fn red_card_penalty_handles_cards_during_overtime_per90() {
        assert_eq!(red_card_penalty_remaining_per90(120.0, 105), 15.0 / 90.0);
    }

    #[test]
    fn red_card_penalty_per90_never_rewards_late_cards() {
        assert_eq!(red_card_penalty_remaining_per90(90.0, 105), 0.0);
    }
}
