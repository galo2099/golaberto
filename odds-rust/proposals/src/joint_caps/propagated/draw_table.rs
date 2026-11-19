use super::{Cell, Pattern};
use std::mem::size_of;

/// Cached information for one outcome at a fixture boundary.
#[derive(Clone, Copy, Debug)]
pub(super) struct EndpointTransition {
    pub(super) next_counts: Option<(usize, usize)>,
    pub(super) endpoint_chances: [f64; 2],
}

#[derive(Clone, Copy)]
struct PointEntry {
    // Bits 0..2: endpoint stays within its admitted point bounds by outcome.
    // Bit 3/4: current endpoint is certainly above/below the target.
    // Bits 5..7 and 8..10: next above/below indicators by outcome.
    flags: u16,
    // The selected cardinality direction is fixed for this Cell.
    chances: [f64; 3],
}

#[derive(Clone, Copy)]
struct SparseEntry {
    points: i32,
    entry: PointEntry,
}

enum PointRows {
    Dense {
        first: i32,
        entries: Box<[PointEntry]>,
    },
    Indexed {
        first: i32,
        indices: Box<[u16]>,
        entries: Box<[PointEntry]>,
    },
    Sparse(Box<[SparseEntry]>),
}
impl PointRows {
    fn get(&self, points: i32) -> Option<&PointEntry> {
        match self {
            Self::Dense { first, entries } => {
                let offset = points.checked_sub(*first)? as usize;
                entries.get(offset)
            }
            Self::Indexed {
                first,
                indices,
                entries,
            } => {
                let offset = points.checked_sub(*first)? as usize;
                let index = *indices.get(offset)?;
                (index != u16::MAX).then(|| &entries[index as usize])
            }
            Self::Sparse(entries) => entries
                .binary_search_by_key(&points, |entry| entry.points)
                .ok()
                .map(|index| &entries[index].entry),
        }
    }

    #[cfg(test)]
    fn is_sparse(&self) -> bool {
        !matches!(self, Self::Dense { .. })
    }

    #[cfg(test)]
    fn is_indexed(&self) -> bool {
        matches!(self, Self::Indexed { .. })
    }
}

struct StepRows {
    home: PointRows,
    away: PointRows,
}

/// Immutable endpoint transition cache for one admitted pattern. Reachable
/// point totals are propagated exactly through positive-probability joint and
/// guide outcomes. Small ranges use direct dense indexing, medium sparse ranges
/// use a compact direct-index map, and very wide sparse ranges use binary search.
/// Construction is
/// bounded by `max_bytes` and a separate reachable-state cap, and returns
/// `None` when either limit is exceeded.
pub(super) struct DrawTable {
    steps: Box<[StepRows]>,
    rank: usize,
    teams: usize,
    bytes: usize,
}

const MAX_REACHABLE_VALUES: usize = 1_000_000;

impl DrawTable {
    pub(super) fn new(pattern: &Pattern, cell: Cell, max_bytes: usize) -> Option<Self> {
        let teams = pattern.base.len();
        let target = *pattern.base.get(cell.team)?;

        // The draw's target is held fixed while guide fixtures are sampled.
        // Joint fixtures exclude it by construction; keep the fast table
        // inapplicable if a future caller violates that invariant.
        if pattern
            .joint
            .games
            .iter()
            .chain(&pattern.guide.games)
            .any(|g| g.home == cell.team || g.away == cell.team)
        {
            return None;
        }

        let games = &pattern.guide.games;
        let base_bytes = size_of::<Self>()
            .checked_add(2 * size_of::<usize>())? // Arc control block, conservatively.
            .checked_add(games.len().checked_mul(size_of::<StepRows>())?)?;
        if base_bytes > max_bytes {
            return None;
        }
        let team_headers = teams.checked_mul(size_of::<Vec<i32>>())?;
        if team_headers >= max_bytes {
            return None;
        }
        let state_limit =
            ((max_bytes - team_headers) / (2 * size_of::<i32>())).min(MAX_REACHABLE_VALUES);
        let mut live_values = 0usize;
        let mut reachable = initial_reachable(&pattern.base, &mut live_values, state_limit)?;
        for game in &pattern.joint.games {
            apply_game(game, &mut reachable, &mut live_values, state_limit)?;
        }

        // First pass measures all chosen representations without allocating
        // table rows. This reserves the complete persistent footprint before
        // any large row allocation starts.
        let mut bytes = base_bytes;
        let mut sizing_reachable = reachable;
        let mut sizing_live = live_values;
        for game in games {
            bytes = bytes.checked_add(row_layout(&sizing_reachable[game.home])?.bytes)?;
            bytes = bytes.checked_add(row_layout(&sizing_reachable[game.away])?.bytes)?;
            if bytes > max_bytes {
                return None;
            }
            apply_game(game, &mut sizing_reachable, &mut sizing_live, state_limit)?;
        }
        drop(sizing_reachable);

        let below = cell.rank > (teams - 1) / 2;
        let chance_rows = pattern.guide.prepare_chances(below);
        let mut steps = Vec::with_capacity(games.len());
        let mut build_live = 0usize;
        let mut build_reachable = initial_reachable(&pattern.base, &mut build_live, state_limit)?;
        for game in &pattern.joint.games {
            apply_game(game, &mut build_reachable, &mut build_live, state_limit)?;
        }
        for (step, game) in games.iter().enumerate() {
            let home = make_point_rows(
                &build_reachable[game.home],
                game.home,
                game.hg,
                step,
                target,
                pattern,
                chance_rows,
                below,
            )?;
            let away = make_point_rows(
                &build_reachable[game.away],
                game.away,
                game.ag,
                step,
                target,
                pattern,
                chance_rows,
                below,
            )?;
            steps.push(StepRows { home, away });
            apply_game(game, &mut build_reachable, &mut build_live, state_limit)?;
        }

        Some(Self {
            steps: steps.into_boxed_slice(),
            rank: cell.rank,
            teams,
            bytes,
        })
    }

    #[inline]
    pub(super) fn lookup(
        &self,
        step: usize,
        home_points: i32,
        away_points: i32,
        counts: (usize, usize),
    ) -> Option<[EndpointTransition; 3]> {
        let rows = self.steps.get(step)?;
        let home = rows.home.get(home_points)?;
        let away = rows.away.get(away_points)?;
        let valid = (home.flags & away.flags) & 0b111;
        if valid == 0 {
            return Some(
                [EndpointTransition {
                    next_counts: None,
                    endpoint_chances: [0.; 2],
                }; 3],
            );
        }

        // The common old endpoint indicators are removed once, rather than
        // repeating the same count updates for all three outcomes.
        let mut base_above = counts.0;
        base_above -= usize::from(home.flags & (1 << 3) != 0);
        base_above -= usize::from(away.flags & (1 << 3) != 0);
        let mut base_below = counts.1;
        base_below -= usize::from(home.flags & (1 << 4) != 0);
        base_below -= usize::from(away.flags & (1 << 4) != 0);

        Some(std::array::from_fn(|outcome| {
            let mut transition = EndpointTransition {
                next_counts: None,
                endpoint_chances: [0.; 2],
            };
            if valid & (1 << outcome) == 0 {
                return transition;
            }
            let above = base_above
                + usize::from(home.flags & (1 << (5 + outcome)) != 0)
                + usize::from(away.flags & (1 << (5 + outcome)) != 0);
            let below = base_below
                + usize::from(home.flags & (1 << (8 + outcome)) != 0)
                + usize::from(away.flags & (1 << (8 + outcome)) != 0);
            if above <= self.rank && below <= self.teams - 1 - self.rank {
                transition.next_counts = Some((above, below));
                transition.endpoint_chances = [home.chances[outcome], away.chances[outcome]];
            }
            transition
        }))
    }

    pub(super) fn memory_bytes(&self) -> usize {
        self.bytes
    }
}

struct RowLayout {
    first: i32,
    last: i32,
    kind: RowKind,
    bytes: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowKind {
    Dense,
    Indexed,
    Sparse,
}

const MAX_DIRECT_DENSE_WIDTH: usize = 32;
const MAX_INDEXED_WIDTH: usize = 4_096;

fn row_layout(points: &[i32]) -> Option<RowLayout> {
    let first = *points.first()?;
    let last = *points.last()?;
    let width = point_width(first, last)?;
    let dense_bytes = width.checked_mul(size_of::<PointEntry>())?;
    let sparse_bytes = points.len().checked_mul(size_of::<SparseEntry>())?;
    let indexed_bytes = width
        .checked_mul(size_of::<u16>())?
        .checked_add(points.len().checked_mul(size_of::<PointEntry>())?)?;
    let kind = if width <= MAX_DIRECT_DENSE_WIDTH {
        RowKind::Dense
    } else if width <= MAX_INDEXED_WIDTH
        && width >= points.len().checked_mul(2)?
        && points.len() <= u16::MAX as usize
        && indexed_bytes < dense_bytes
    {
        RowKind::Indexed
    } else if width > MAX_INDEXED_WIDTH && sparse_bytes < dense_bytes {
        RowKind::Sparse
    } else {
        RowKind::Dense
    };
    let bytes = match kind {
        RowKind::Dense => dense_bytes,
        RowKind::Indexed => indexed_bytes,
        RowKind::Sparse => sparse_bytes,
    };
    Some(RowLayout {
        first,
        last,
        kind,
        bytes,
    })
}

fn point_width(min: i32, max: i32) -> Option<usize> {
    usize::try_from(i64::from(max) - i64::from(min) + 1).ok()
}

fn initial_reachable(
    base: &[i32],
    live_values: &mut usize,
    state_limit: usize,
) -> Option<Vec<Vec<i32>>> {
    if base.len() > state_limit {
        return None;
    }
    *live_values = base.len();
    Some(base.iter().map(|&points| vec![points]).collect())
}

fn apply_game(
    game: &super::RankGame,
    reachable: &mut [Vec<i32>],
    live_values: &mut usize,
    state_limit: usize,
) -> Option<()> {
    let home = add_gains(
        &reachable[game.home],
        game.hg,
        game.prob,
        *live_values,
        state_limit,
    )?;
    *live_values = (*live_values)
        .checked_sub(reachable[game.home].capacity())?
        .checked_add(home.capacity())?;
    reachable[game.home] = home;
    let away = add_gains(
        &reachable[game.away],
        game.ag,
        game.prob,
        *live_values,
        state_limit,
    )?;
    *live_values = (*live_values)
        .checked_sub(reachable[game.away].capacity())?
        .checked_add(away.capacity())?;
    reachable[game.away] = away;
    Some(())
}

fn add_gains(
    current: &[i32],
    gains: [i32; 3],
    prob: [f64; 3],
    live_values: usize,
    state_limit: usize,
) -> Option<Vec<i32>> {
    let mut unique = [0; 3];
    let mut unique_len = 0usize;
    for outcome in 0..3 {
        if prob[outcome] <= 0. || unique[..unique_len].contains(&gains[outcome]) {
            continue;
        }
        unique[unique_len] = gains[outcome];
        unique_len += 1;
    }
    if unique_len == 0 {
        return None;
    }
    let capacity = current.len().checked_mul(unique_len)?;
    if live_values.checked_add(capacity)? > state_limit {
        return None;
    }
    let mut next = Vec::new();
    next.try_reserve_exact(capacity).ok()?;
    for &gain in &unique[..unique_len] {
        for &points in current {
            next.push(points.checked_add(gain)?);
        }
    }
    next.sort_unstable();
    next.dedup();
    Some(next)
}

fn make_point_rows(
    reachable: &[i32],
    team: usize,
    gains: [i32; 3],
    step: usize,
    target: i32,
    pattern: &Pattern,
    chance_rows: &[std::sync::Arc<[f64]>],
    below: bool,
) -> Option<PointRows> {
    let layout = row_layout(reachable)?;
    match layout.kind {
        RowKind::Sparse => {
            let mut entries = Vec::new();
            entries.try_reserve_exact(reachable.len()).ok()?;
            for &points in reachable {
                entries.push(SparseEntry {
                    points,
                    entry: make_point_entry(
                        points,
                        team,
                        gains,
                        step,
                        target,
                        pattern,
                        chance_rows,
                        below,
                    ),
                });
            }
            Some(PointRows::Sparse(entries.into_boxed_slice()))
        }
        RowKind::Indexed => {
            let width = point_width(layout.first, layout.last)?;
            let mut indices = Vec::new();
            indices.try_reserve_exact(width).ok()?;
            indices.resize(width, u16::MAX);
            let mut entries = Vec::new();
            entries.try_reserve_exact(reachable.len()).ok()?;
            for (index, &points) in reachable.iter().enumerate() {
                let offset = usize::try_from(i64::from(points) - i64::from(layout.first)).ok()?;
                indices[offset] = u16::try_from(index).ok()?;
                entries.push(make_point_entry(
                    points,
                    team,
                    gains,
                    step,
                    target,
                    pattern,
                    chance_rows,
                    below,
                ));
            }
            Some(PointRows::Indexed {
                first: layout.first,
                indices: indices.into_boxed_slice(),
                entries: entries.into_boxed_slice(),
            })
        }
        RowKind::Dense => {
            let width = point_width(layout.first, layout.last)?;
            let mut entries = Vec::new();
            entries.try_reserve_exact(width).ok()?;
            for offset in 0..width {
                let points = layout.first.checked_add(i32::try_from(offset).ok()?)?;
                entries.push(make_point_entry(
                    points,
                    team,
                    gains,
                    step,
                    target,
                    pattern,
                    chance_rows,
                    below,
                ));
            }
            Some(PointRows::Dense {
                first: layout.first,
                entries: entries.into_boxed_slice(),
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn make_point_entry(
    points: i32,
    team: usize,
    gains: [i32; 3],
    step: usize,
    target: i32,
    pattern: &Pattern,
    chance_rows: &[std::sync::Arc<[f64]>],
    below: bool,
) -> PointEntry {
    let old_above = points + pattern.guide.min[step][team] > target;
    let old_below = points + pattern.guide.max[step][team] < target;
    let mut flags = (u16::from(old_above) << 3) | (u16::from(old_below) << 4);
    let mut chances = [0.; 3];
    for outcome in 0..3 {
        let lo = points + gains[outcome] + pattern.guide.min[step + 1][team];
        let hi = points + gains[outcome] + pattern.guide.max[step + 1][team];
        if lo <= pattern.upper[team] && hi >= pattern.lower[team] {
            flags |= 1 << outcome;
            flags |= u16::from(lo > target) << (5 + outcome);
            flags |= u16::from(hi < target) << (8 + outcome);
        }
        chances[outcome] = pattern
            .guide
            .cached_chance(
                chance_rows,
                step + 1,
                team,
                target - (points + gains[outcome]),
                below,
            )
            .clamp(0., 1.);
    }
    PointEntry { flags, chances }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lookahead::RankGame;
    use std::sync::Arc;

    fn game(index: usize, home: usize, away: usize) -> RankGame {
        RankGame {
            index,
            home,
            away,
            prob: [0.2, 0.3, 0.5],
            hg: [0, 1, 3],
            ag: [3, 1, 0],
        }
    }

    fn pattern() -> Pattern {
        let base = vec![8, 5, 6, 4];
        let lower = vec![0; 4];
        let upper = vec![40; 4];
        let joint_game = game(10, 2, 3);
        let joint = super::super::NecessaryJoint {
            games: vec![joint_game],
            internal: Vec::new(),
            tables: Vec::new(),
            patterns: Vec::new(),
            mass: 1.,
            selected: Vec::new(),
            states: 0,
            residual_hint: 0.,
        };
        Pattern {
            fixed: Vec::new(),
            bias: None,
            omitted: Vec::new(),
            base,
            lower,
            upper,
            guide: Arc::new(super::super::Guide::new(
                vec![game(0, 1, 2), game(1, 2, 3), game(2, 1, 3)],
                4,
            )),
            joint: Arc::new(joint),
            mass: 1.,
            tilt: 1.,
            cumulative: 1.,
            bound_guidance: false,
        }
    }

    #[test]
    fn cached_transitions_match_scalar_counts_and_chances_for_reachable_states() {
        let pattern = pattern();
        let cell = Cell { team: 0, rank: 2 };
        let cap = 1 << 20;
        let table = DrawTable::new(&pattern, cell, cap).expect("small table fits");
        assert!(table.memory_bytes() <= cap);
        assert!(!table.steps[0].home.is_sparse(), "small ranges stay dense");
        assert!(DrawTable::new(&pattern, cell, table.memory_bytes() - 1).is_none());
        let below = cell.rank > (pattern.base.len() - 1) / 2;
        let chance_rows = pattern.guide.prepare_chances(below);
        let target = pattern.base[cell.team];
        let mut states = vec![pattern.base.clone()];

        for game in &pattern.joint.games {
            states = states
                .into_iter()
                .flat_map(|points| {
                    (0..3).filter_map(move |outcome| {
                        if game.prob[outcome] <= 0. {
                            return None;
                        }
                        let mut next = points.clone();
                        next[game.home] += game.hg[outcome];
                        next[game.away] += game.ag[outcome];
                        Some(next)
                    })
                })
                .collect();
        }

        for (step, game) in pattern.guide.games.iter().enumerate() {
            let current = states.clone();
            for points in &current {
                let Some(counts) = pattern.counts(step, points, cell) else {
                    continue;
                };
                let cached = table
                    .lookup(step, points[game.home], points[game.away], counts)
                    .expect("reachable endpoint values are covered");
                for outcome in 0..3 {
                    let expected = pattern.next_counts(step, points, cell, outcome, counts);
                    assert_eq!(cached[outcome].next_counts, expected);
                    if expected.is_some() {
                        let home_chance = pattern
                            .guide
                            .cached_chance(
                                chance_rows,
                                step + 1,
                                game.home,
                                target - (points[game.home] + game.hg[outcome]),
                                below,
                            )
                            .clamp(0., 1.);
                        let away_chance = pattern
                            .guide
                            .cached_chance(
                                chance_rows,
                                step + 1,
                                game.away,
                                target - (points[game.away] + game.ag[outcome]),
                                below,
                            )
                            .clamp(0., 1.);
                        assert_eq!(
                            cached[outcome].endpoint_chances[0].to_bits(),
                            home_chance.to_bits()
                        );
                        assert_eq!(
                            cached[outcome].endpoint_chances[1].to_bits(),
                            away_chance.to_bits()
                        );
                    }
                }
            }
            states = current
                .into_iter()
                .flat_map(|points| {
                    (0..3).filter_map(move |outcome| {
                        if game.prob[outcome] <= 0. {
                            return None;
                        }
                        let mut next = points.clone();
                        next[game.home] += game.hg[outcome];
                        next[game.away] += game.ag[outcome];
                        Some(next)
                    })
                })
                .collect();
        }

        assert!(table.lookup(0, i32::MIN, pattern.base[2], (0, 0)).is_none());
        assert!(DrawTable::new(&pattern, cell, 0).is_none());
    }
    fn packed_game(index: usize, home: usize, away: usize, stride: i32) -> RankGame {
        RankGame {
            index,
            home,
            away,
            prob: [0.2, 0.3, 0.5],
            hg: [0, stride, 3 * stride + 1],
            ag: [3 * stride + 1, stride, 0],
        }
    }

    fn negative_packed_game(index: usize, home: usize, away: usize, stride: i32) -> RankGame {
        RankGame {
            index,
            home,
            away,
            prob: [0.2, 0.3, 0.5],
            hg: [-3 * stride - 1, -stride, 0],
            ag: [0, -stride, -3 * stride - 1],
        }
    }

    #[test]
    fn sparse_packed_points_wins_rows_cover_reachable_and_reject_absent_totals() {
        let teams = 20;
        let stride = 29;
        let base = (0..teams)
            .map(|team| {
                if team == 0 {
                    10_000
                } else if team == 1 {
                    120
                } else {
                    20
                }
            })
            .collect::<Vec<_>>();
        let games_per_team = 11;
        let mut joint_games = (0..games_per_team)
            .map(|i| packed_game(i, 1, i + 2, stride))
            .collect::<Vec<_>>();
        joint_games.extend(
            (0..games_per_team)
                .map(|i| negative_packed_game(i + games_per_team, 16, i + 2, stride)),
        );
        let guide_games = vec![
            packed_game(30, 1, 15, stride),
            packed_game(31, 1, 17, stride),
            packed_game(32, 1, 18, stride),
            packed_game(33, 16, 19, stride),
        ];
        let pattern = Pattern {
            fixed: Vec::new(),
            bias: None,
            omitted: Vec::new(),
            lower: vec![-10_000; teams],
            upper: vec![10_000; teams],
            base: base.clone(),
            guide: Arc::new(super::super::Guide::new(guide_games, teams)),
            joint: Arc::new(super::super::NecessaryJoint {
                games: joint_games,
                internal: Vec::new(),
                tables: Vec::new(),
                patterns: Vec::new(),
                mass: 1.,
                selected: Vec::new(),
                states: 0,
                residual_hint: 0.,
            }),
            mass: 1.,
            tilt: 1.,
            cumulative: 1.,
            bound_guidance: false,
        };
        let cell = Cell { team: 0, rank: 0 };
        let cap = 1 << 20;
        let table = DrawTable::new(&pattern, cell, cap).expect("bounded sparse table fits");
        let points = &table.steps[0].home;
        assert!(
            points.is_sparse(),
            "packed point totals should not allocate the span"
        );

        let mut reachable = Vec::new();
        for wins in 0..=games_per_team {
            for draws in 0..=games_per_team - wins {
                reachable.push(base[1] + wins as i32 * (3 * stride + 1) + draws as i32 * stride);
            }
        }
        reachable.sort_unstable();
        reachable.dedup();
        assert_eq!(
            reachable.len(),
            (games_per_team + 1) * (games_per_team + 2) / 2
        );
        assert!(
            points.is_indexed(),
            "realistic packed span should use O(1) compact lookup"
        );
        for &score in &reachable {
            assert!(
                points.get(score).is_some(),
                "missing reachable score {score}"
            );
        }
        assert!(
            points.get(base[1] + 1).is_none(),
            "interior gap should stay absent"
        );
        assert!(table.lookup(0, base[1] + 1, base[15], (0, 19)).is_none());
        let negative_points = &table.steps[3].home;
        assert!(
            negative_points.is_sparse(),
            "mirrored negative totals should stay sparse"
        );
        for wins in 0..=games_per_team {
            for draws in 0..=games_per_team - wins {
                let score = base[16] - wins as i32 * (3 * stride + 1) - draws as i32 * stride;
                assert!(
                    negative_points.get(score).is_some(),
                    "missing negative reachable score {score}"
                );
                let mut state = base.clone();
                state[16] = score;
                let game = &pattern.guide.games[3];
                let counts = pattern.counts(3, &state, cell).expect("wide rank bounds");
                let cached = table
                    .lookup(3, score, state[game.away], counts)
                    .expect("reachable negative endpoint total");
                for outcome in 0..3 {
                    assert_eq!(
                        cached[outcome].next_counts,
                        pattern.next_counts(3, &state, cell, outcome, counts)
                    );
                }
            }
        }
        assert!(negative_points.get(base[16] - 1).is_none());
        assert!(table.lookup(3, base[16] - 1, base[19], (0, 19)).is_none());

        let chance_rows = pattern.guide.prepare_chances(false);
        let target = base[cell.team];
        for score in reachable {
            let mut state = base.clone();
            state[1] = score;
            let game = &pattern.guide.games[0];
            let counts = pattern.counts(0, &state, cell).expect("wide rank bounds");
            let cached = table
                .lookup(0, score, state[game.away], counts)
                .expect("reachable home and away totals");
            for outcome in 0..3 {
                assert_eq!(
                    cached[outcome].next_counts,
                    pattern.next_counts(0, &state, cell, outcome, counts)
                );
                if cached[outcome].next_counts.is_some() {
                    for (endpoint, team, gain) in [
                        (0, game.home, game.hg[outcome]),
                        (1, game.away, game.ag[outcome]),
                    ] {
                        let expected = pattern
                            .guide
                            .cached_chance(
                                chance_rows,
                                1,
                                team,
                                target - (state[team] + gain),
                                false,
                            )
                            .clamp(0., 1.);
                        assert_eq!(
                            cached[outcome].endpoint_chances[endpoint].to_bits(),
                            expected.to_bits()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn target_team_in_guide_or_joint_declines_fixed_target_table() {
        let mut with_guide_target = pattern();
        with_guide_target.guide = Arc::new(super::super::Guide::new(vec![game(0, 0, 1)], 4));
        assert!(DrawTable::new(&with_guide_target, Cell { team: 0, rank: 2 }, 1 << 20).is_none());

        let mut with_joint_target = pattern();
        with_joint_target.joint = Arc::new(super::super::NecessaryJoint {
            games: vec![game(1, 0, 2)],
            internal: Vec::new(),
            tables: Vec::new(),
            patterns: Vec::new(),
            mass: 1.,
            selected: Vec::new(),
            states: 0,
            residual_hint: 0.,
        });
        assert!(DrawTable::new(&with_joint_target, Cell { team: 0, rank: 2 }, 1 << 20).is_none());
    }
}
