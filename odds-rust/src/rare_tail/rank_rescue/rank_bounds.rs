//! Small, optimistic W/D/L bounds used to certify rank-rescue roots.
//!
//! This module only reasons about points and wins. Callers must supply the
//! standings after applying the candidate root and the remaining fixtures.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rival {
    pub(crate) points: i32,
    pub(crate) wins: usize,
    /// Fixtures whose outcomes are not jointly enumerated below. Each is
    /// optimistically treated as a win for this rival.
    pub(crate) outside_fixtures: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Game {
    pub(crate) a: usize,
    pub(crate) b: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WorkCap;

const WDL: [(i32, usize); 3] = [(3, 1), (1, 0), (0, 0)];

/// Return an upper bound on the number of rivals that can finish at least as
/// well as the target under points, then wins. Equal tuples count as possible
/// outrankers because later tie breakers are deliberately left optimistic.
///
/// `None` means validation failed or the bounded enumeration was exhausted;
/// callers must retain the root in either case.
pub(crate) fn maximum_possible_above(
    rivals: &[Rival],
    games: &[Game],
    target_points: i32,
    target_wins: usize,
    node_cap: usize,
) -> Result<(usize, usize), WorkCap> {
    if rivals.is_empty()
        || node_cap == 0
        || games.iter().any(|g| g.a >= rivals.len() || g.b >= rivals.len() || g.a == g.b)
    {
        return Err(WorkCap);
    }
    // Keep only vertices that can reach the target threshold even if every
    // outside fixture is a win. Everyone else cannot outrank the target.
    let mut incident = vec![0usize; rivals.len()];
    for game in games {
        incident[game.a] = incident[game.a].saturating_add(1);
        incident[game.b] = incident[game.b].saturating_add(1);
    }
    let eligible: Vec<usize> = rivals
        .iter()
        .enumerate()
        .filter_map(|(i, rival)| {
            let optimistic_fixtures = rival.outside_fixtures.saturating_add(incident[i]);
            let points = rival.points.saturating_add(3i32.saturating_mul(optimistic_fixtures as i32));
            let wins = rival.wins.saturating_add(optimistic_fixtures);
            ((points, wins) >= (target_points, target_wins)).then_some(i)
        })
        .collect();
    if eligible.is_empty() {
        return Ok((0, 1));
    }
    let mut eligible_index = vec![None; rivals.len()];
    for (index, &original) in eligible.iter().enumerate() {
        eligible_index[original] = Some(index);
    }
    let internal: Vec<(usize, usize)> = games
        .iter()
        .filter_map(|game| Some((eligible_index[game.a]?, eligible_index[game.b]?)))
        .collect();
    let internal_by_original: Vec<usize> = (0..rivals.len())
        .map(|i| {
            games
                .iter()
                .filter(|game| {
                    (game.a == i && eligible_index[game.b].is_none())
                        || (game.b == i && eligible_index[game.a].is_none())
                })
                .count()
        })
        .collect();
    let mut visited = 0usize;
    let mut maximum = 0usize;

    fn visit(
        edge: usize,
        edges: &[(usize, usize)],
        scores: &mut [(i32, usize)],
        target: (i32, usize),
        visited: &mut usize,
        cap: usize,
        maximum: &mut usize,
    ) -> Result<(), WorkCap> {
        *visited = visited.checked_add(1).ok_or(WorkCap)?;
        if *visited > cap {
            return Err(WorkCap);
        }
        if edge == edges.len() {
            *maximum = (*maximum).max(scores.iter().filter(|score| **score >= target).count());
            return Ok(());
        }
        let (a, b) = edges[edge];
        // Fixture increments are from a's perspective. A draw is shared;
        // a win for either side is a loss for the other.
        for outcome in 0..3 {
            let (points, wins) = WDL[outcome];
            let (other_points, other_wins) = WDL[2 - outcome];
            scores[a].0 = scores[a].0.saturating_add(points - 3);
            scores[a].1 = scores[a].1.saturating_add(wins).saturating_sub(1);
            scores[b].0 = scores[b].0.saturating_add(other_points - 3);
            scores[b].1 = scores[b].1.saturating_add(other_wins).saturating_sub(1);
            visit(edge + 1, edges, scores, target, visited, cap, maximum)?;
            scores[a].0 = scores[a].0.saturating_sub(points - 3);
            scores[a].1 = scores[a].1.saturating_sub(wins).saturating_add(1);
            scores[b].0 = scores[b].0.saturating_sub(other_points - 3);
            scores[b].1 = scores[b].1.saturating_sub(other_wins).saturating_add(1);
        }
        Ok(())
    }

    // The per-rival outside maximum included every enumerated game as a win.
    // Replace the two independent wins with the shared WDL outcome.
    let mut base = Vec::with_capacity(eligible.len());
    for (index, &original) in eligible.iter().enumerate() {
        let r = rivals[original];
        let shared = internal.iter().filter(|(a, b)| *a == index || *b == index).count();
        let outside = r.outside_fixtures.saturating_add(internal_by_original[original]);
        base.push((
            r.points
                .saturating_add(3i32.saturating_mul(outside.saturating_add(shared) as i32)),
            r.wins.saturating_add(outside).saturating_add(shared),
        ));
    }
    let mut scores = base;
    visit(
        0,
        &internal,
        &mut scores,
        (target_points, target_wins),
        &mut visited,
        node_cap,
        &mut maximum,
    )?;
    Ok((maximum, visited))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brute_force(rivals: &[Rival], games: &[Game], target: (i32, usize)) -> usize {
        let states = 3usize.pow(games.len() as u32);
        (0..states)
            .map(|encoded| {
                let mut scores: Vec<_> = rivals
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        let incident = games.iter().filter(|g| g.a == i || g.b == i).count();
                        (
                            r.points + 3 * r.outside_fixtures as i32,
                            r.wins + r.outside_fixtures,
                        )
                    })
                    .collect();
                let mut value = encoded;
                for game in games {
                    let outcome = value % 3;
                    value /= 3;
                    let (points_a, wins_a) = WDL[outcome];
                    let (points_b, wins_b) = WDL[2 - outcome];
                    scores[game.a].0 += points_a;
                    scores[game.a].1 += wins_a;
                    scores[game.b].0 += points_b;
                    scores[game.b].1 += wins_b;
                }
                scores.iter().filter(|score| **score >= target).count()
            })
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn bound_dominates_exhaustive_small_wdl_models() {
        for seed in 0..81usize {
            let rivals = (0..4)
                .map(|i| Rival {
                    points: ((seed >> (i * 2)) % 7) as i32,
                    wins: (seed >> (i * 2 + 1)) % 3,
                    outside_fixtures: (i + seed) % 2,
                })
                .collect::<Vec<_>>();
            let games = [Game { a: 0, b: 1 }, Game { a: 1, b: 2 }, Game { a: 2, b: 3 }];
            let target = ((seed % 12) as i32, seed % 4);
            let exact = brute_force(&rivals, &games, target);
            let (bound, _) = maximum_possible_above(&rivals, &games, target.0, target.1, 1_000)
                .expect("small model fits cap");
            assert!(bound >= exact, "seed={seed}: bound={bound}, exact={exact}");
        }
    }

    #[test]
    fn cap_exhaustion_returns_unknown() {
        let rivals = vec![Rival { points: 0, wins: 0, outside_fixtures: 1 }; 2];
        let games = [Game { a: 0, b: 1 }];
        assert_eq!(maximum_possible_above(&rivals, &games, 1, 0, 1), Err(WorkCap));
    }

    #[test]
    fn a_selected_game_can_supply_the_only_possible_rival_win() {
        let rivals = vec![Rival { points: 0, wins: 0, outside_fixtures: 0 }; 2];
        let games = [Game { a: 0, b: 1 }];
        assert_eq!(
            maximum_possible_above(&rivals, &games, 3, 1, 100),
            Ok((1, 4))
        );
    }
}
