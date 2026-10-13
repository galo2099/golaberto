//! Bounded extreme-rank relaxation with explained fixture conflicts.
//! A feasible P/W assignment is not a reachability witness. Only exhaustion of
//! every required-rival cohort can prove impossibility, without restricting goals.
use crate::{
    model::{Key, Model},
    search::Cell,
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Reason<const WORDS: usize> {
    decisions: [u64; WORDS],
    teams: u64,
}
impl<const WORDS: usize> Default for Reason<WORDS> {
    fn default() -> Self {
        Self {
            decisions: [0; WORDS],
            teams: 0,
        }
    }
}
impl<const WORDS: usize> Reason<WORDS> {
    fn insert(&mut self, bit: usize) {
        self.decisions[bit / 64] |= 1_u64 << (bit % 64);
    }
    fn contains(&self, bit: usize) -> bool {
        self.decisions[bit / 64] & (1_u64 << (bit % 64)) != 0
    }
    fn remove(&mut self, bit: usize) {
        self.decisions[bit / 64] &= !(1_u64 << (bit % 64));
    }
    fn is_empty(&self) -> bool {
        self.decisions.iter().all(|word| *word == 0)
    }
    fn merge(&mut self, other: Self) {
        for (left, right) in self.decisions.iter_mut().zip(other.decisions) {
            *left |= right;
        }
        self.teams |= other.teams;
    }
}
struct Game {
    home: usize,
    away: usize,
    gain: [[i64; 3]; 2],
    maximum: [[i64; 8]; 2],
}
pub struct RankProof {
    base: Vec<i64>,
    games: Vec<Game>,
    adjacent: Vec<Vec<(usize, usize)>>,
    ranked: usize,
}
#[derive(Default, Debug, Serialize)]
pub struct Stats {
    pub nodes: usize,
    pub cohorts: usize,
    pub cohort_cache_hits: usize,
    pub backjumps: usize,
}
#[derive(Clone, Copy)]
pub struct Options {
    pub explain: bool,
    pub internal_order: bool,
    pub reuse_cohorts: bool,
    /// Avoid deep search unless an eligible rival has a tight fixture outcome.
    pub require_root_pressure: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_cohorts: usize,
    pub max_depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_cohorts: 512,
            max_depth: 128,
        }
    }
}
impl Default for Options {
    fn default() -> Self {
        Self {
            explain: true,
            internal_order: true,
            reuse_cohorts: true,
            require_root_pressure: false,
        }
    }
}
#[derive(Debug, PartialEq)]
enum Answer<const WORDS: usize> {
    Conflict(Reason<WORDS>),
    Feasible,
    Unknown,
}
#[derive(Default, Serialize)]
pub struct Report {
    pub impossible: bool,
    pub stats: Stats,
}
#[derive(Default)]
pub struct FeasibleDomains {
    pub domains: Option<Vec<u8>>,
    pub stats: Stats,
}
impl RankProof {
    pub fn new(model: &Model) -> Option<Self> {
        let c = &model.request.phase.championship;
        if model.ids.len() > 64
            || model.keys.first() != Some(&Key::Pt)
            || model.request.phase.bonus_points != 0
            || (c.point_win, c.point_draw, c.point_loss) != (3, 1, 0)
            || model.fixtures.iter().any(|g| g.home == g.away)
        {
            return None;
        }
        let wins = model.keys.get(1) == Some(&Key::W);
        let mut remaining = vec![0_i64; model.ids.len()];
        for g in &model.fixtures {
            remaining[g.home] += 1;
            remaining[g.away] += 1;
        }
        let stride = if wins {
            model
                .base
                .iter()
                .enumerate()
                .map(|(t, c)| i64::from(c.wins) + remaining[t])
                .max()?
                + 1
        } else {
            1
        };
        let base = model
            .base
            .iter()
            .map(|c| {
                i64::from(c.points)
                    .checked_mul(stride)?
                    .checked_add(if wins { i64::from(c.wins) } else { 0 })
            })
            .collect::<Option<Vec<_>>>()?;
        let win = 3 * stride + i64::from(wins);
        let mut adjacent = vec![Vec::new(); model.ids.len()];
        let games = model
            .fixtures
            .iter()
            .enumerate()
            .map(|(i, g)| {
                adjacent[g.home].push((i, 0));
                adjacent[g.away].push((i, 1));
                let gain = [[0, stride, win], [win, stride, 0]];
                let maximum = std::array::from_fn(|end| {
                    std::array::from_fn(|d| {
                        (0..3)
                            .filter(|o| d & (1 << o) != 0)
                            .map(|o| gain[end][o])
                            .max()
                            .unwrap_or(i64::MIN / 4)
                    })
                });
                Game {
                    home: g.home,
                    away: g.away,
                    gain,
                    maximum,
                }
            })
            .collect();
        Some(Self {
            base,
            games,
            adjacent,
            ranked: model.n,
        })
    }
    /// Negating the packed ordering provides the corresponding best-rank proof.
    pub fn negated(&self) -> Self {
        let games = self
            .games
            .iter()
            .map(|g| {
                let gain = g.gain.map(|v| v.map(|x| -x));
                let maximum = std::array::from_fn(|end| {
                    std::array::from_fn(|d| {
                        (0..3)
                            .filter(|o| d & (1 << o) != 0)
                            .map(|o| gain[end][o])
                            .max()
                            .unwrap_or(i64::MIN / 4)
                    })
                });
                Game {
                    home: g.home,
                    away: g.away,
                    gain,
                    maximum,
                }
            })
            .collect();
        Self {
            base: self.base.iter().map(|v| -v).collect(),
            games,
            adjacent: self.adjacent.clone(),
            ranked: self.ranked,
        }
    }
    pub fn prove(
        &self,
        cell: Cell,
        required_count: usize,
        budget: usize,
        options: Options,
    ) -> Report {
        self.prove_floor(cell, required_count, budget, options, None)
    }
    /// Necessary relaxation for seasons whose target packed total is at least
    /// `floor` in this proof's orientation. Extreme target results generously
    /// improve every rival; equality is counted in the rivals' favor. A false
    /// result, including quota exhaustion, supplies no restriction.
    pub fn prove_floor(
        &self,
        cell: Cell,
        required_count: usize,
        budget: usize,
        options: Options,
        requested_floor: Option<i64>,
    ) -> Report {
        self.prove_floor_with_limits(
            cell,
            required_count,
            budget,
            options,
            requested_floor,
            Limits::default(),
        )
    }
    pub fn prove_floor_with_limits(
        &self,
        cell: Cell,
        required_count: usize,
        budget: usize,
        options: Options,
        requested_floor: Option<i64>,
        limits: Limits,
    ) -> Report {
        if cell.team >= self.ranked || required_count >= self.ranked {
            return Report::default();
        }
        let mut initial = vec![7_u8; self.games.len()];
        let mut floor = self.base[cell.team];
        for (i, end) in &self.adjacent[cell.team] {
            let gain = self.games[*i].gain[*end];
            let minimum = *gain.iter().min().unwrap();
            initial[*i] = (0..3).fold(0, |d, o| if gain[o] == minimum { d | (1 << o) } else { d });
            floor += minimum;
        }
        if let Some(requested) = requested_floor {
            floor = floor.max(requested);
        }
        let mut maximum = self.base.clone();
        for (i, g) in self.games.iter().enumerate() {
            maximum[g.home] += g.maximum[0][initial[i] as usize];
            maximum[g.away] += g.maximum[1][initial[i] as usize];
        }
        let eligible: Vec<_> = (0..self.ranked)
            .filter(|t| *t != cell.team && maximum[*t] >= floor)
            .collect();
        if eligible.len() < required_count {
            return Report {
                impossible: true,
                stats: Stats::default(),
            };
        }
        // Difficulty signal only: an individually tight outcome indicates that
        // shared assignments may matter. Absence of this signal merely skips the
        // fallback; it does not establish feasibility or impossibility.
        if options.require_root_pressure
            && !eligible.iter().any(|&t| {
                self.adjacent[t].iter().any(|&(i, end)| {
                    let g = &self.games[i];
                    let minimum = (0..3)
                        .filter(|o| initial[i] & (1 << o) != 0)
                        .map(|o| g.gain[end][o])
                        .min()
                        .unwrap();
                    maximum[t] - g.maximum[end][initial[i] as usize] + minimum < floor
                })
            })
        {
            return Report::default();
        }
        // Bound cohort enumeration independently of the fixture-node budget.
        let k = required_count.min(eligible.len() - required_count);
        let mut count = 1_usize;
        for i in 0..k {
            count = match count.checked_mul(eligible.len() - i) {
                Some(v) => v / (i + 1),
                None => return Report::default(),
            };
            if count > limits.max_cohorts {
                return Report::default();
            }
        }
        if count > limits.max_cohorts {
            return Report::default();
        }
        let max_depth = limits.max_depth.min(512);
        if max_depth <= 128 {
            let mut solver: Solver<'_, 2> = Solver {
                p: self,
                floor,
                initial,
                initial_maximum: maximum,
                budget,
                options,
                max_depth,
                stats: Stats::default(),
                cores: Vec::new(),
                capture_feasible_domains: false,
                feasible_domains: None,
            };
            let impossible = solver.cohorts(&eligible, 0, required_count, 0);
            return Report {
                impossible,
                stats: solver.stats,
            };
        }
        let mut solver: Solver<'_, 8> = Solver {
            p: self,
            floor,
            initial,
            initial_maximum: maximum,
            budget,
            options,
            max_depth,
            stats: Stats::default(),
            cores: Vec::new(),
            capture_feasible_domains: false,
            feasible_domains: None,
        };
        let impossible = solver.cohorts(&eligible, 0, required_count, 0);
        Report {
            impossible,
            stats: solver.stats,
        }
    }

    /// Find one feasible point/win relaxation assignment for witness
    /// construction. Feasibility is only a candidate: callers must complete
    /// scores and verify the actual standings before publishing it.
    pub fn feasible_domains(
        &self,
        cell: Cell,
        required_count: usize,
        budget: usize,
        options: Options,
    ) -> FeasibleDomains {
        self.feasible_domains_with_limits(
            cell,
            required_count,
            budget,
            options,
            Limits::default(),
        )
    }
    pub fn feasible_domains_with_limits(
        &self,
        cell: Cell,
        required_count: usize,
        budget: usize,
        options: Options,
        limits: Limits,
    ) -> FeasibleDomains {
        if cell.team >= self.ranked || required_count >= self.ranked || budget == 0 {
            return FeasibleDomains::default();
        }
        let mut initial = vec![7_u8; self.games.len()];
        let mut floor = self.base[cell.team];
        for (i, end) in &self.adjacent[cell.team] {
            let gain = self.games[*i].gain[*end];
            let minimum = *gain.iter().min().unwrap();
            initial[*i] = (0..3).fold(0, |d, o| if gain[o] == minimum { d | (1 << o) } else { d });
            floor += minimum;
        }
        let mut maximum = self.base.clone();
        for (i, g) in self.games.iter().enumerate() {
            maximum[g.home] += g.maximum[0][initial[i] as usize];
            maximum[g.away] += g.maximum[1][initial[i] as usize];
        }
        let eligible: Vec<_> = (0..self.ranked)
            .filter(|t| *t != cell.team && maximum[*t] >= floor)
            .collect();
        if eligible.len() < required_count {
            return FeasibleDomains::default();
        }
        if options.require_root_pressure
            && !eligible.iter().any(|&t| {
                self.adjacent[t].iter().any(|&(i, end)| {
                    let g = &self.games[i];
                    let minimum = (0..3)
                        .filter(|o| initial[i] & (1 << o) != 0)
                        .map(|o| g.gain[end][o])
                        .min()
                        .unwrap();
                    maximum[t] - g.maximum[end][initial[i] as usize] + minimum < floor
                })
            })
        {
            return FeasibleDomains::default();
        }
        let k = required_count.min(eligible.len() - required_count);
        let mut count = 1_usize;
        for i in 0..k {
            count = match count.checked_mul(eligible.len() - i) {
                Some(v) => v / (i + 1),
                None => return FeasibleDomains::default(),
            };
            if count > limits.max_cohorts {
                return FeasibleDomains::default();
            }
        }
        if count > limits.max_cohorts {
            return FeasibleDomains::default();
        }
        let max_depth = limits.max_depth.min(512);
        if max_depth <= 128 {
            let mut solver: Solver<'_, 2> = Solver {
                p: self,
                floor,
                initial,
                initial_maximum: maximum,
                budget,
                options,
                max_depth,
                stats: Stats::default(),
                cores: Vec::new(),
                capture_feasible_domains: true,
                feasible_domains: None,
            };
            solver.cohorts(&eligible, 0, required_count, 0);
            return FeasibleDomains {
                domains: solver.feasible_domains,
                stats: solver.stats,
            };
        }
        let mut solver: Solver<'_, 8> = Solver {
            p: self,
            floor,
            initial,
            initial_maximum: maximum,
            budget,
            options,
            max_depth,
            stats: Stats::default(),
            cores: Vec::new(),
            capture_feasible_domains: true,
            feasible_domains: None,
        };
        solver.cohorts(&eligible, 0, required_count, 0);
        FeasibleDomains {
            domains: solver.feasible_domains,
            stats: solver.stats,
        }
    }
}
struct Solver<'a, const WORDS: usize> {
    p: &'a RankProof,
    floor: i64,
    initial: Vec<u8>,
    initial_maximum: Vec<i64>,
    budget: usize,
    options: Options,
    max_depth: usize,
    stats: Stats,
    cores: Vec<u64>,
    capture_feasible_domains: bool,
    feasible_domains: Option<Vec<u8>>,
}
fn gcd(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}
impl<const WORDS: usize> Solver<'_, WORDS> {
    /// Project one required endpoint after removing optional draws from all
    /// other fixtures. A fixed draw remains allowed. This score guides capture
    /// branch order only; it never removes an outcome from a domain.
    fn no_optional_draw_demand(
        &self,
        team: usize,
        focus: Option<(usize, usize)>,
        ds: &[u8],
    ) -> Option<i128> {
        let mut minimum_total = i128::from(self.p.base[team]);
        let mut maximum_total = i128::from(self.p.base[team]);
        let mut step = 0_i128;
        for &(i, end) in &self.p.adjacent[team] {
            let domain = if focus.is_some_and(|(game, _)| game == i) {
                1 << focus.unwrap().1
            } else if ds[i] & 0b101 != 0 {
                ds[i] & 0b101
            } else {
                ds[i]
            };
            if domain == 0 {
                return None;
            }
            let mut minimum: Option<i128> = None;
            for outcome in 0..3 {
                if domain & (1 << outcome) == 0 {
                    continue;
                }
                let gain = i128::from(self.p.games[i].gain[end][outcome]);
                minimum = Some(minimum.map_or(gain, |v| v.min(gain)));
            }
            let minimum = minimum?;
            minimum_total = minimum_total.checked_add(minimum)?;
            let maximum = (0..3)
                .filter(|outcome| domain & (1 << outcome) != 0)
                .map(|outcome| i128::from(self.p.games[i].gain[end][outcome]))
                .max()?;
            maximum_total = maximum_total.checked_add(maximum)?;
            for outcome in 0..3 {
                if domain & (1 << outcome) != 0 {
                    step = gcd(
                        step,
                        i128::from(self.p.games[i].gain[end][outcome]) - minimum,
                    );
                }
            }
        }
        let floor = i128::from(self.floor);
        if maximum_total < floor {
            return None;
        }
        if minimum_total >= floor {
            Some(minimum_total)
        } else if step == 0 {
            None
        } else {
            let gap = floor.checked_sub(minimum_total)?;
            let steps = gap.checked_add(step - 1)?.checked_div(step)?;
            minimum_total.checked_add(steps.checked_mul(step)?)
        }
    }
    // Only restrictions that lower this endpoint's maximum can contribute to
    // its bound. A hypothetical outcome replaces its own fixture, so omit that
    // fixture's restriction when explaining why the outcome was removed.
    fn explanation(
        &self,
        t: usize,
        ds: &[u8],
        reasons: &[Reason<WORDS>],
        exclude: usize,
    ) -> Reason<WORDS> {
        let mut r = Reason::<WORDS> {
            decisions: [0; WORDS],
            teams: 1 << t,
        };
        for &(i, end) in &self.p.adjacent[t] {
            if i != exclude
                && self.p.games[i].maximum[end][ds[i] as usize]
                    < self.p.games[i].maximum[end][self.initial[i] as usize]
            {
                r.merge(reasons[i]);
            }
        }
        r
    }
    /// A cohort must fit all required teams' cheapest attainable packed totals
    /// into the packed points available from their shared fixtures. The gcd
    /// residue bound keeps each individual demand conservative when outcomes
    /// have gaps. Restricted-domain explanations are all dependencies of this
    /// collective bound, even when a restriction did not lower an endpoint's
    /// individual maximum.
    fn collective_conflict(
        &self,
        required: u64,
        ds: &[u8],
        reasons: &[Reason<WORDS>],
    ) -> Option<Reason<WORDS>> {
        let mut capacity = 0_i128;
        for (team, &base) in self.p.base.iter().enumerate() {
            if required & (1 << team) != 0 {
                capacity = capacity.checked_add(i128::from(base))?;
            }
        }
        for (i, game) in self.p.games.iter().enumerate() {
            let home_required = required & (1 << game.home) != 0;
            let away_required = required & (1 << game.away) != 0;
            if !home_required && !away_required {
                continue;
            }
            if ds[i] == 0 {
                let mut conflict = Reason::<WORDS> {
                    decisions: [0; WORDS],
                    teams: required,
                };
                conflict.merge(reasons[i]);
                return Some(conflict);
            }
            let mut best: Option<i128> = None;
            for outcome in 0..3 {
                if ds[i] & (1 << outcome) == 0 {
                    continue;
                }
                let mut gain = 0_i128;
                if home_required {
                    gain = gain.checked_add(i128::from(game.gain[0][outcome]))?;
                }
                if away_required {
                    gain = gain.checked_add(i128::from(game.gain[1][outcome]))?;
                }
                best = Some(best.map_or(gain, |v| v.max(gain)));
            }
            capacity = capacity.checked_add(best?)?;
        }

        let mut demand = 0_i128;
        for team in 0..self.p.base.len() {
            if required & (1 << team) == 0 {
                continue;
            }
            let mut minimum_total = i128::from(self.p.base[team]);
            let mut step = 0_i128;
            for &(i, end) in &self.p.adjacent[team] {
                let mut minimum: Option<i128> = None;
                for outcome in 0..3 {
                    if ds[i] & (1 << outcome) == 0 {
                        continue;
                    }
                    let gain = i128::from(self.p.games[i].gain[end][outcome]);
                    minimum = Some(minimum.map_or(gain, |v| v.min(gain)));
                }
                let minimum = minimum?;
                minimum_total = minimum_total.checked_add(minimum)?;
                for outcome in 0..3 {
                    if ds[i] & (1 << outcome) != 0 {
                        step = gcd(
                            step,
                            i128::from(self.p.games[i].gain[end][outcome]) - minimum,
                        );
                    }
                }
            }
            let floor = i128::from(self.floor);
            if step == 0 && minimum_total < floor {
                let mut conflict = Reason::<WORDS> {
                    decisions: [0; WORDS],
                    teams: 1 << team,
                };
                for &(i, _) in &self.p.adjacent[team] {
                    if ds[i] != self.initial[i] {
                        conflict.merge(reasons[i]);
                    }
                }
                return Some(conflict);
            }
            let required_total = if minimum_total >= floor {
                minimum_total
            } else {
                let gap = floor.checked_sub(minimum_total)?;
                let steps = gap.checked_add(step - 1)?.checked_div(step)?;
                minimum_total.checked_add(steps.checked_mul(step)?)?
            };
            demand = demand.checked_add(required_total)?;
        }
        if demand <= capacity {
            return None;
        }

        let mut conflict = Reason::<WORDS> {
            decisions: [0; WORDS],
            teams: required,
        };
        for (i, game) in self.p.games.iter().enumerate() {
            if ds[i] != self.initial[i] && (required & ((1 << game.home) | (1 << game.away))) != 0 {
                conflict.merge(reasons[i]);
            }
        }
        Some(conflict)
    }
    fn propagate(
        &self,
        required: u64,
        ds: &mut [u8],
        reasons: &mut [Reason<WORDS>],
    ) -> Result<Vec<i64>, Reason<WORDS>> {
        let mut maximum = self.initial_maximum.clone();
        loop {
            maximum.copy_from_slice(&self.initial_maximum);
            // Root maxima include forced target results. Unchanged fixture
            // domains need no score-table loads or endpoint updates.
            for (i, g) in self.p.games.iter().enumerate() {
                if ds[i] == self.initial[i] {
                    continue;
                }
                maximum[g.home] +=
                    g.maximum[0][ds[i] as usize] - g.maximum[0][self.initial[i] as usize];
                maximum[g.away] +=
                    g.maximum[1][ds[i] as usize] - g.maximum[1][self.initial[i] as usize];
            }
            for (t, &v) in maximum.iter().enumerate() {
                if required & (1 << t) != 0 && v < self.floor {
                    return Err(self.explanation(t, ds, reasons, usize::MAX));
                }
            }
            let mut changed = false;
            for (i, g) in self.p.games.iter().enumerate() {
                let domain = ds[i];
                // Singleton support is checked by the endpoint maxima above.
                // Games outside the required cohort cannot constrain its floor.
                if domain.is_power_of_two() || required & ((1 << g.home) | (1 << g.away)) == 0 {
                    continue;
                }
                for o in 0..3 {
                    if domain & (1 << o) == 0 {
                        continue;
                    }
                    for (t, end) in [(g.home, 0), (g.away, 1)] {
                        if required & (1 << t) != 0
                            && maximum[t] - g.maximum[end][domain as usize] + g.gain[end][o]
                                < self.floor
                        {
                            let reason = self.explanation(t, ds, reasons, i);
                            reasons[i].merge(reason);
                            ds[i] &= !(1 << o);
                            changed = true;
                            break;
                        }
                    }
                }
                if ds[i] == 0 {
                    return Err(reasons[i]);
                }
            }
            if !changed {
                if self.capture_feasible_domains {
                    if let Some(conflict) = self.collective_conflict(required, ds, reasons) {
                        return Err(conflict);
                    }
                }
                return Ok(maximum);
            }
        }
    }
    fn visit(
        &mut self,
        required: u64,
        mut ds: Vec<u8>,
        mut reasons: Vec<Reason<WORDS>>,
        depth: usize,
    ) -> Answer<WORDS> {
        if self.stats.nodes >= self.budget || depth >= self.max_depth {
            return Answer::Unknown;
        }
        self.stats.nodes += 1;
        let maximum = match self.propagate(required, &mut ds, &mut reasons) {
            Ok(v) => v,
            Err(r) => return Answer::Conflict(r),
        };
        let old_order_key = |(i, g): (usize, &Game)| {
            let h = required & (1 << g.home) != 0;
            let a = required & (1 << g.away) != 0;
            let sum = if h { maximum[g.home] - self.floor } else { 0 }
                + if a { maximum[g.away] - self.floor } else { 0 };
            let slack = [(g.home, h), (g.away, a)]
                .into_iter()
                .filter(|(_, r)| *r)
                .map(|(t, _)| maximum[t] - self.floor)
                .min()
                .unwrap();
            if self.options.internal_order {
                (
                    ds[i].count_ones(),
                    2 - usize::from(h) - usize::from(a),
                    sum,
                    i,
                )
            } else {
                (ds[i].count_ones(), 0, slack, i)
            }
        };
        let best = if self.capture_feasible_domains && self.options.internal_order {
            let candidates: Vec<_> = self
                .p
                .games
                .iter()
                .enumerate()
                .filter(|(i, g)| {
                    ds[*i].count_ones() > 1 && required & ((1 << g.home) | (1 << g.away)) != 0
                })
                .collect();
            let mut current_projected = vec![None; self.p.base.len()];
            for team in 0..self.p.base.len() {
                if required & (1 << team) != 0 {
                    current_projected[team] = self.no_optional_draw_demand(team, None, &ds);
                }
            }
            let mut effects = Vec::with_capacity(candidates.len());
            for &(i, game) in &candidates {
                let endpoints = [(game.home, 0), (game.away, 1)];
                let mut current_demand = 0_i128;
                let mut demand_valid = true;
                let mut current_capacity: Option<i128> = None;
                for outcome in 0..3 {
                    if ds[i] & (1 << outcome) == 0 {
                        continue;
                    }
                    let mut gain = 0_i128;
                    for &(team, end) in &endpoints {
                        if required & (1 << team) != 0 {
                            gain = match gain.checked_add(i128::from(game.gain[end][outcome])) {
                                Some(value) => value,
                                None => {
                                    demand_valid = false;
                                    gain
                                }
                            };
                        }
                    }
                    current_capacity = Some(current_capacity.map_or(gain, |value| value.max(gain)));
                }
                for &(team, _) in &endpoints {
                    if required & (1 << team) == 0 {
                        continue;
                    }
                    if let Some(projected) = current_projected[team] {
                        current_demand = match current_demand.checked_add(projected) {
                            Some(value) => value,
                            None => {
                                demand_valid = false;
                                current_demand
                            }
                        };
                    } else {
                        demand_valid = false;
                    }
                }
                let mut best_effect: Option<i128> = None;
                if demand_valid {
                    if let Some(current_capacity) = current_capacity {
                        for outcome in 0..3 {
                            if ds[i] & (1 << outcome) == 0 {
                                continue;
                            }
                            let mut new_demand = 0_i128;
                            let mut chosen_gain = 0_i128;
                            let mut valid = true;
                            for &(team, end) in &endpoints {
                                if required & (1 << team) == 0 {
                                    continue;
                                }
                                match self.no_optional_draw_demand(team, Some((i, outcome)), &ds) {
                                    Some(value) => match new_demand.checked_add(value) {
                                        Some(sum) => new_demand = sum,
                                        None => valid = false,
                                    },
                                    None => valid = false,
                                }
                                match chosen_gain.checked_add(i128::from(game.gain[end][outcome])) {
                                    Some(sum) => chosen_gain = sum,
                                    None => valid = false,
                                }
                            }
                            if valid {
                                let effect =
                                    new_demand.checked_sub(current_demand).and_then(|value| {
                                        value.checked_add(current_capacity - chosen_gain)
                                    });
                                if let Some(effect) = effect {
                                    best_effect =
                                        Some(best_effect.map_or(effect, |best| best.min(effect)));
                                }
                            }
                        }
                    }
                }
                effects.push((i, game, best_effect));
            }
            if effects.iter().any(|(_, _, effect)| effect.is_some()) {
                effects
                    .into_iter()
                    .filter_map(|(i, game, effect)| effect.map(|effect| (i, game, effect)))
                    .min_by(|(i_a, g_a, effect_a), (i_b, g_b, effect_b)| {
                        ds[*i_a]
                            .count_ones()
                            .cmp(&ds[*i_b].count_ones())
                            .then_with(|| effect_a.cmp(effect_b))
                            .then_with(|| {
                                old_order_key((*i_a, g_a)).cmp(&old_order_key((*i_b, g_b)))
                            })
                    })
                    .map(|(i, _, _)| i)
            } else {
                self.p
                    .games
                    .iter()
                    .enumerate()
                    .filter(|(i, g)| {
                        ds[*i].count_ones() > 1 && required & ((1 << g.home) | (1 << g.away)) != 0
                    })
                    .min_by_key(|candidate| old_order_key(*candidate))
                    .map(|(i, _)| i)
            }
        } else {
            self.p
                .games
                .iter()
                .enumerate()
                .filter(|(i, g)| {
                    ds[*i].count_ones() > 1 && required & ((1 << g.home) | (1 << g.away)) != 0
                })
                .min_by_key(|candidate| old_order_key(*candidate))
                .map(|(i, _)| i)
        };
        let Some(i) = best else {
            if self.capture_feasible_domains {
                self.feasible_domains = Some(ds);
            }
            return Answer::Feasible;
        };
        let g = &self.p.games[i];
        let mut options: Vec<_> = (0..3).filter(|o| ds[i] & (1 << o) != 0).collect();
        if self.capture_feasible_domains && self.options.internal_order {
            let mut guaranteed = vec![0_i128; self.p.base.len()];
            let mut flexible = vec![0_usize; self.p.base.len()];
            for (team, total) in guaranteed.iter_mut().enumerate() {
                if required & (1 << team) == 0 {
                    continue;
                }
                *total = i128::from(self.p.base[team]);
                for &(game, end) in &self.p.adjacent[team] {
                    let minimum = (0..3)
                        .filter(|outcome| ds[game] & (1 << outcome) != 0)
                        .map(|outcome| self.p.games[game].gain[end][outcome])
                        .min()
                        .unwrap_or(0);
                    *total += i128::from(minimum);
                    if ds[game].count_ones() > 1 {
                        flexible[team] += 1;
                    }
                }
            }
            let endpoint_minimum = [
                (required & (1 << g.home) != 0).then(|| {
                    (0..3)
                        .filter(|outcome| ds[i] & (1 << outcome) != 0)
                        .map(|outcome| g.gain[0][outcome])
                        .min()
                        .unwrap_or(0)
                }),
                (required & (1 << g.away) != 0).then(|| {
                    (0..3)
                        .filter(|outcome| ds[i] & (1 << outcome) != 0)
                        .map(|outcome| g.gain[1][outcome])
                        .min()
                        .unwrap_or(0)
                }),
            ];
            let endpoints = [(g.home, 0), (g.away, 1)];
            let floor = i128::from(self.floor);
            let score = |outcome: usize| {
                let mut projected_demand = 0_i128;
                let mut projected_valid = true;
                let mut actual_gain = 0_i128;
                for &(team, end) in &endpoints {
                    if required & (1 << team) == 0 {
                        continue;
                    }
                    match self.no_optional_draw_demand(team, Some((i, outcome)), &ds) {
                        Some(demand) => match projected_demand.checked_add(demand) {
                            Some(sum) => projected_demand = sum,
                            None => projected_valid = false,
                        },
                        None => projected_valid = false,
                    }
                    match actual_gain.checked_add(i128::from(g.gain[end][outcome])) {
                        Some(sum) => actual_gain = sum,
                        None => projected_valid = false,
                    }
                }
                let projected_score = projected_valid
                    .then(|| projected_demand.checked_sub(actual_gain))
                    .flatten();
                let mut ratios = [0.0_f64; 2];
                let mut residual_sum = 0_i128;
                let mut count = 0;
                for (slot, &(team, end)) in endpoints.iter().enumerate() {
                    if required & (1 << team) == 0 {
                        continue;
                    }
                    let delta = i128::from(g.gain[end][outcome])
                        - i128::from(endpoint_minimum[slot].unwrap());
                    let residual = (floor - guaranteed[team] - delta).max(0);
                    residual_sum += residual;
                    let remaining = flexible[team] - usize::from(ds[i].count_ones() > 1);
                    ratios[count] = if residual == 0 {
                        0.0
                    } else if remaining == 0 {
                        f64::INFINITY
                    } else {
                        residual as f64 / remaining as f64
                    };
                    count += 1;
                }
                let first = ratios[0];
                let second = if count > 1 { ratios[1] } else { 0.0 };
                (
                    projected_score,
                    first.max(second),
                    (first - second).abs(),
                    first + second,
                    residual_sum,
                    outcome,
                )
            };
            options.sort_by(|a, b| {
                let a = score(*a);
                let b = score(*b);
                let projected = match (a.0, b.0) {
                    (Some(left), Some(right)) => left.cmp(&right),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                };
                projected
                    .then_with(|| a.1.total_cmp(&b.1))
                    .then_with(|| a.2.total_cmp(&b.2))
                    .then_with(|| a.3.total_cmp(&b.3))
                    .then_with(|| a.4.cmp(&b.4))
                    .then_with(|| a.5.cmp(&b.5))
            });
        } else {
            options.sort_by_key(|o| {
                -(if required & (1 << g.home) != 0 {
                    g.gain[0][*o]
                } else {
                    0
                } + if required & (1 << g.away) != 0 {
                    g.gain[1][*o]
                } else {
                    0
                })
            });
        }
        // The parent domain may already exclude values. Those explanations must
        // be retained when resolving child conflicts over only its allowed values.
        let mut union = reasons[i];
        for o in options {
            let mut next = ds.clone();
            next[i] = 1 << o;
            let mut why = reasons.clone();
            why[i].insert(depth);
            match self.visit(required, next, why, depth + 1) {
                Answer::Conflict(r) => {
                    if self.options.explain && !r.contains(depth) {
                        self.stats.backjumps += 1;
                        return Answer::Conflict(r);
                    }
                    union.merge(r);
                }
                other => return other,
            }
        }
        union.remove(depth);
        Answer::Conflict(union)
    }
    fn cohorts(&mut self, candidates: &[usize], start: usize, left: usize, required: u64) -> bool {
        if left == 0 {
            self.stats.cohorts += 1;
            if self.options.reuse_cohorts && self.cores.iter().any(|core| core & required == *core)
            {
                self.stats.cohort_cache_hits += 1;
                return true;
            }
            if self.stats.nodes >= self.budget {
                return false;
            }
            match self.visit(
                required,
                self.initial.clone(),
                vec![Reason::<WORDS>::default(); self.p.games.len()],
                0,
            ) {
                Answer::Conflict(r) => {
                    debug_assert!(r.is_empty());
                    debug_assert_eq!(r.teams & required, r.teams);
                    if self.options.reuse_cohorts
                        && !self.cores.iter().any(|core| core & r.teams == *core)
                    {
                        self.cores.retain(|core| r.teams & *core != r.teams);
                        self.cores.push(r.teams);
                    }
                    true
                }
                _ => false,
            }
        } else {
            for i in start..=candidates.len() - left {
                if !self.cohorts(candidates, i + 1, left - 1, required | (1 << candidates[i])) {
                    return false;
                }
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Request;
    use serde_json::json;
    #[test]
    fn decision_reasons_merge_and_clear_across_the_full_depth_range() {
        let mut a = Reason::<8>::default();
        for bit in [127, 128, 255, 511] {
            a.insert(bit);
            assert!(a.contains(bit));
        }
        let mut b = Reason::<8>::default();
        b.insert(128);
        b.insert(300);
        a.merge(b);
        assert!(a.contains(127) && a.contains(128) && a.contains(255));
        assert!(a.contains(300) && a.contains(511));
        for bit in [127, 128, 255, 300, 511] {
            a.remove(bit);
        }
        assert!(a.is_empty());
    }
    fn snapshot() -> Model {
        let request:Request=serde_json::from_str(include_str!("../../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json")).unwrap();
        Model::new(request).unwrap()
    }
    #[test]
    fn threshold_refutations_never_remove_a_legal_outcome_in_either_direction() {
        for sort in ["pt,w,gd,gf", "pt,gd,gf,w"] {
            let request:Request=serde_json::from_value(json!({"id":1,"phase":{"sort":sort,"championship":{"point_win":3,"point_draw":1,"point_loss":0}},
                "team_groups":[{"team_id":1,"add_sub":2},{"team_id":2},{"team_id":3},{"team_id":4}],
                "games":[{"id":1,"home_id":1,"away_id":2,"played":true,"home_score":1,"away_score":0},
                {"id":2,"home_id":1,"away_id":3},{"id":3,"home_id":2,"away_id":4},{"id":4,"home_id":2,"away_id":3},{"id":5,"home_id":3,"away_id":4}]})).unwrap();
            let m = Model::new(request).unwrap();
            let p = RankProof::new(&m).unwrap();
            let neg = p.negated();
            for proof in [&p, &neg] {
                let seasons: Vec<_> = (0..81)
                    .map(|mut code| {
                        let mut totals = proof.base.clone();
                        for g in &proof.games {
                            let o = code % 3;
                            code /= 3;
                            totals[g.home] += g.gain[0][o];
                            totals[g.away] += g.gain[1][o];
                        }
                        totals
                    })
                    .collect();
                for team in 0..m.n {
                    let floors: std::collections::BTreeSet<_> = seasons
                        .iter()
                        .flat_map(|s| [s[team], s[team] + 1])
                        .collect();
                    for floor in floors {
                        let ahead = seasons
                            .iter()
                            .filter(|s| s[team] >= floor)
                            .map(|s| (0..m.n).filter(|&t| t != team && s[t] >= s[team]).count())
                            .max();
                        for rank in 0..m.n {
                            let r = proof.prove_floor(
                                Cell { team, rank },
                                rank,
                                10000,
                                Options::default(),
                                Some(floor),
                            );
                            assert!(!r.impossible || ahead.is_none_or(|a|a<rank),"false refutation: {sort}, target {team}, floor {floor}, rank {rank}");
                            let expanded = proof.prove_floor_with_limits(
                                Cell { team, rank },
                                rank,
                                10000,
                                Options::default(),
                                Some(floor),
                                Limits { max_cohorts: 100_000, max_depth: 512 },
                            );
                            assert!(
                                !expanded.impossible || ahead.is_none_or(|a| a < rank),
                                "false expanded refutation: {sort}, target {team}, floor {floor}, rank {rank}"
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn explained_search_refutes_snapshot_with_existing_cell_quota() {
        let m = snapshot();
        let p = RankProof::new(&m).unwrap();
        let cell = Cell {
            team: m.indices[&17],
            rank: 13,
        };
        let result = p.prove(cell, 13, 500, Options::default());
        assert!(result.impossible);
        assert!(result.stats.nodes <= 500);
        assert!(result.stats.cohort_cache_hits > 0);
        assert!(result.stats.backjumps > 0);
        let budgeted = p.prove(
            cell,
            13,
            500,
            Options {
                require_root_pressure: true,
                ..Options::default()
            },
        );
        assert!(budgeted.impossible);
        let short = p.prove(cell, 13, 100, Options::default());
        assert!(!short.impossible);
        let empty = p.prove(cell, 13, 0, Options::default());
        assert!(!empty.impossible);
        assert_eq!(empty.stats.nodes, 0);
    }
    #[test]
    fn feasible_domain_capture_finds_palmeiras_thirteenth_within_small_budget() {
        let request: Request = serde_json::from_str(include_str!(
            "../tests/fixtures/group-16498-2026-10-10.json"
        ))
        .unwrap();
        let model = Model::new(request).unwrap();
        let proof = RankProof::new(&model).unwrap();
        let cell = Cell {
            team: model.indices[&16],
            rank: 12,
        };
        let found = proof.feasible_domains(cell, 12, 100, Options::default());
        assert!(found.domains.is_some(), "nodes={}", found.stats.nodes);
        assert!(found.stats.nodes <= 100);
        assert!(found.stats.cohorts > 0);
        assert!(found
            .domains
            .unwrap()
            .iter()
            .all(|domain| (1..=7).contains(domain)));

        let short = proof.feasible_domains(cell, 12, 1, Options::default());
        assert!(short.domains.is_none());
        assert!(short.stats.nodes <= 1);
        let empty = proof.feasible_domains(cell, 12, 0, Options::default());
        assert!(empty.domains.is_none());
        assert_eq!(empty.stats.nodes, 0);
    }
    #[test]
    fn capture_collective_bound_keeps_draw_dependency_when_maxima_do_not_change() {
        let request: Request = serde_json::from_value(json!({
            "id": 1,
            "phase": {"sort":"pt,gd,gf", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups": [
                {"team_id":1,"add_sub":0},
                {"team_id":2,"add_sub":0},
                {"team_id":3,"add_sub":0}
            ],
            "games": [{"id":1,"home_id":2,"away_id":3}]
        })).unwrap();
        let model = Model::new(request).unwrap();
        let mut proof = RankProof::new(&model).unwrap();
        // With one point already banked, either decisive result leaves one
        // required team below two points. The draw is the only shared support.
        proof.base[1] = 1;
        proof.base[2] = 1;
        let solver: Solver<'_, 8> = Solver {
            p: &proof,
            floor: 2,
            initial: vec![7],
            initial_maximum: vec![0; proof.base.len()],
            budget: 100,
            options: Options::default(),
            max_depth: 512,
            stats: Stats::default(),
            cores: Vec::new(),
            capture_feasible_domains: true,
            feasible_domains: None,
        };
        let required = (1 << 1) | (1 << 2);
        assert!(solver
            .collective_conflict(required, &[7], &[Reason::default()])
            .is_none());

        // Removing the draw keeps both individual maxima at three, so the
        // old endpoint-only explanation would omit this dependency. Its gcd
        // residue raises each demand to four while shared capacity remains five.
        let mut reason = Reason::<8>::default();
        reason.insert(128);
        let conflict = solver
            .collective_conflict(required, &[0b101], &[reason])
            .expect("decisive-only domains cannot satisfy both teams");
        assert!(conflict.contains(128));
        let fixed_total_conflict = solver
            .collective_conflict(1 << 1, &[0b001], &[reason])
            .expect("a fixed total below the floor is impossible");
        assert_eq!(fixed_total_conflict.teams, 1 << 1);
        assert!(fixed_total_conflict.contains(128));
        for team in [1, 2] {
            assert_eq!(proof.games[0].maximum[usize::from(team == 2)][0b101], 3);
        }
    }
    #[test]
    fn expanded_depth_finds_a_feasible_domain_after_128_decisions() {
        let request = json!({
            "id": 99005,
            "phase": {"sort":"pt,gd,gf,bias", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups": (0..3).map(|team| json!({"team_id":team,"add_sub":0,"bias":team})).collect::<Vec<_>>(),
            "games": (0..160).map(|id| json!({"id":id+1,"home_id":1,"away_id":2,"home_power":1.0,"away_power":1.0})).collect::<Vec<_>>()
        });
        let model = Model::new(serde_json::from_value(request).unwrap()).unwrap();
        let proof = RankProof::new(&model).unwrap();
        let cell = Cell { team: 0, rank: 1 };
        let default = proof.feasible_domains(cell, 1, 1000, Options::default());
        assert!(default.domains.is_none());
        let depth_limited = proof.feasible_domains_with_limits(
            cell,
            1,
            1000,
            Options::default(),
            Limits {
                max_cohorts: 100_000,
                max_depth: 1,
            },
        );
        assert!(depth_limited.domains.is_none());
        assert!(depth_limited.stats.nodes <= 1);

        let expanded = proof.feasible_domains_with_limits(
            cell,
            1,
            1000,
            Options::default(),
            Limits {
                max_cohorts: 100_000,
                max_depth: 512,
            },
        );
        assert!(expanded.domains.is_some());
        assert!(expanded.stats.nodes > 128, "{:?}", expanded.stats);
        assert!(expanded.stats.nodes <= 1000);
    }
    #[test]
    fn learned_conflicts_and_both_directions_match_exhaustive_relaxation() {
        for adjustment in [0, 1, 3, 6] {
            for sort in ["pt,w,gd,gf", "pt,gd,gf,w"] {
                let request:Request=serde_json::from_value(json!({"id":1,"phase":{"sort":sort,"championship":{"point_win":3,"point_draw":1,"point_loss":0}},
                "team_groups":[{"team_id":1,"add_sub":adjustment},{"team_id":2},{"team_id":3},{"team_id":4}],
                "games":[{"id":1,"home_id":1,"away_id":2,"played":true,"home_score":1,"away_score":0},
                {"id":2,"home_id":1,"away_id":3},{"id":3,"home_id":2,"away_id":4},{"id":4,"home_id":2,"away_id":3},{"id":5,"home_id":3,"away_id":4}]})).unwrap();
                let m = Model::new(request).unwrap();
                let p = RankProof::new(&m).unwrap();
                let neg = p.negated();
                for (proof, direction) in [(&p, 1), (&neg, -1)] {
                    let mut maximum_ahead = vec![0; m.n];
                    for mut code in 0..3_usize.pow(m.fixtures.len() as u32) {
                        let mut totals = proof.base.clone();
                        for g in &proof.games {
                            let o = code % 3;
                            code /= 3;
                            totals[g.home] += g.gain[0][o];
                            totals[g.away] += g.gain[1][o];
                        }
                        for t in 0..m.n {
                            maximum_ahead[t] = maximum_ahead[t].max(
                                (0..m.n)
                                    .filter(|other| *other != t && totals[*other] >= totals[t])
                                    .count(),
                            );
                        }
                    }
                    for (team, &ahead) in maximum_ahead.iter().enumerate() {
                        for rank in 0..m.n {
                            let mut initial = vec![7_u8; proof.games.len()];
                            let mut floor = proof.base[team];
                            for &(i, end) in &proof.adjacent[team] {
                                let minimum = proof.games[i].gain[end].into_iter().min().unwrap();
                                initial[i] = (0..3).fold(0, |domain, outcome| {
                                    if proof.games[i].gain[end][outcome] == minimum {
                                        domain | (1 << outcome)
                                    } else {
                                        domain
                                    }
                                });
                                floor += minimum;
                            }
                            let mut exact_capture_exists = false;
                            for mut code in 0..3_usize.pow(proof.games.len() as u32) {
                                let mut totals = proof.base.clone();
                                let mut allowed = true;
                                for (i, game) in proof.games.iter().enumerate() {
                                    let outcome = code % 3;
                                    code /= 3;
                                    if initial[i] & (1 << outcome) == 0 {
                                        allowed = false;
                                        break;
                                    }
                                    totals[game.home] += game.gain[0][outcome];
                                    totals[game.away] += game.gain[1][outcome];
                                }
                                if allowed
                                    && (0..m.n)
                                        .filter(|other| *other != team && totals[*other] >= floor)
                                        .count()
                                        >= rank
                                {
                                    exact_capture_exists = true;
                                    break;
                                }
                            }
                            let captured = proof.feasible_domains_with_limits(
                                Cell { team, rank },
                                rank,
                                10_000,
                                Options::default(),
                                Limits {
                                    max_cohorts: 100_000,
                                    max_depth: 512,
                                },
                            );
                            assert_eq!(
                                captured.domains.is_some(),
                                exact_capture_exists,
                                "capture mismatch: adjustment={adjustment} sort={sort} direction={direction} team={team} required={rank}"
                            );
                            if let Some(domains) = captured.domains {
                                for mut code in 0..3_usize.pow(proof.games.len() as u32) {
                                    let mut totals = proof.base.clone();
                                    let mut allowed = true;
                                    for (i, game) in proof.games.iter().enumerate() {
                                        let outcome = code % 3;
                                        code /= 3;
                                        if domains[i] & (1 << outcome) == 0 {
                                            allowed = false;
                                            break;
                                        }
                                        totals[game.home] += game.gain[0][outcome];
                                        totals[game.away] += game.gain[1][outcome];
                                    }
                                    if allowed {
                                        assert!(
                                            (0..m.n)
                                                .filter(|other| *other != team && totals[*other] >= floor)
                                                .count()
                                                >= rank,
                                            "invalid captured completion: adjustment={adjustment} sort={sort} direction={direction} team={team} required={rank}"
                                        );
                                    }
                                }
                            }
                            let capture_without_helpers = proof.feasible_domains_with_limits(
                                Cell { team, rank },
                                rank,
                                10_000,
                                Options {
                                    explain: false,
                                    internal_order: false,
                                    reuse_cohorts: false,
                                    require_root_pressure: false,
                                },
                                Limits {
                                    max_cohorts: 100_000,
                                    max_depth: 512,
                                },
                            );
                            assert_eq!(
                                capture_without_helpers.domains.is_some(),
                                exact_capture_exists,
                                "unoptimized capture mismatch: adjustment={adjustment} sort={sort} direction={direction} team={team} required={rank}"
                            );
                            if let Some(domains) = capture_without_helpers.domains {
                                for mut code in 0..3_usize.pow(proof.games.len() as u32) {
                                    let mut totals = proof.base.clone();
                                    let mut allowed = true;
                                    for (i, game) in proof.games.iter().enumerate() {
                                        let outcome = code % 3;
                                        code /= 3;
                                        if domains[i] & (1 << outcome) == 0 {
                                            allowed = false;
                                            break;
                                        }
                                        totals[game.home] += game.gain[0][outcome];
                                        totals[game.away] += game.gain[1][outcome];
                                    }
                                    if allowed {
                                        assert!(
                                            (0..m.n)
                                                .filter(|other| *other != team && totals[*other] >= floor)
                                                .count()
                                                >= rank,
                                            "invalid unoptimized captured completion: adjustment={adjustment} sort={sort} direction={direction} team={team} required={rank}"
                                        );
                                    }
                                }
                            }
                            for options in [
                                Options {
                                    explain: false,
                                    internal_order: false,
                                    reuse_cohorts: false,
                                    require_root_pressure: false,
                                },
                                Options::default(),
                            ] {
                                let r = proof.prove(Cell { team, rank }, rank, 100000, options);
                                assert_eq!(r.impossible,ahead<rank,"adjustment={adjustment} sort={sort} direction={direction} team={team} required={rank}");
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn rejects_unsupported_scoring_and_bonus_points() {
        let m = snapshot();
        let mut r = m.request.clone();
        r.phase.bonus_points = 1;
        assert!(RankProof::new(&Model::new(r).unwrap()).is_none());
        let mut r = m.request.clone();
        r.phase.championship.point_win = 2;
        assert!(RankProof::new(&Model::new(r).unwrap()).is_none());
    }
}
