//! Stable, fair ordering for candidate work grouped by team.
use crate::search::Cell;

/// Interleave candidates by team, keeping each team's incoming priority order.
/// The starting team order is derived from stable IDs, request seed, and stage.
pub(crate) fn interleave(cells: Vec<Cell>, ids: &[i32], seed: i64, stage: &str) -> Vec<Cell> {
    if cells.is_empty() || ids.is_empty() {
        return cells;
    }
    let team_count = ids.len();
    let mut buckets = vec![Vec::new(); team_count];
    for cell in cells {
        if cell.team < team_count {
            buckets[cell.team].push(cell);
        }
    }
    let mut order: Vec<usize> = (0..team_count)
        .filter(|&team| !buckets[team].is_empty())
        .collect();
    order.sort_by_key(|&team| (team_hash(seed, stage, ids[team]), ids[team], team));

    let mut output = Vec::new();
    let max_depth = order
        .iter()
        .map(|&team| buckets[team].len())
        .max()
        .unwrap_or(0);
    for depth in 0..max_depth {
        for &team in &order {
            if let Some(cell) = buckets[team].get(depth) {
                output.push(*cell);
            }
        }
    }
    output
}

fn team_hash(seed: i64, stage: &str, team_id: i32) -> u64 {
    // FNV-1a avoids randomized std hashers, so ordering is stable across runs.
    let mut hash = 0xcbf29ce484222325u64;
    for byte in seed
        .to_le_bytes()
        .iter()
        .chain(stage.as_bytes())
        .chain(team_id.to_le_bytes().iter())
    {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interleaves_teams_without_changing_team_priority_and_is_repeatable() {
        let input = vec![
            Cell { team: 0, rank: 0 },
            Cell { team: 0, rank: 1 },
            Cell { team: 0, rank: 2 },
            Cell { team: 1, rank: 0 },
            Cell { team: 1, rank: 1 },
            Cell { team: 2, rank: 0 },
        ];
        let first = interleave(input.clone(), &[31, 7, 19], 42, "pilot");
        assert_eq!(first, interleave(input, &[31, 7, 19], 42, "pilot"));
        let permuted = vec![
            Cell { team: 2, rank: 0 },
            Cell { team: 1, rank: 0 },
            Cell { team: 0, rank: 0 },
            Cell { team: 1, rank: 1 },
            Cell { team: 0, rank: 1 },
            Cell { team: 0, rank: 2 },
        ];
        assert_eq!(first, interleave(permuted, &[31, 7, 19], 42, "pilot"));
        let first_round: Vec<_> = first.iter().take(3).map(|cell| cell.team).collect();
        assert_eq!(first_round.len(), 3);
        assert_eq!(
            first_round
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            3
        );
        for team in 0..3 {
            let ranks: Vec<_> = first
                .iter()
                .filter(|cell| cell.team == team)
                .map(|cell| cell.rank)
                .collect();
            assert!(ranks.windows(2).all(|pair| pair[0] < pair[1]));
        }
    }

    #[test]
    fn handles_empty_teams_and_empty_input() {
        let cells = vec![Cell { team: 2, rank: 4 }, Cell { team: 2, rank: 8 }];
        let ordered = interleave(cells, &[1, 2, 3, 4], 9, "shared");
        assert_eq!(
            ordered,
            vec![Cell { team: 2, rank: 4 }, Cell { team: 2, rank: 8 }]
        );
        assert!(interleave(Vec::new(), &[1], 9, "shared").is_empty());
    }
}
