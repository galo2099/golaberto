use crate::{
    conditioned::canonical_ranks,
    model::{Key, Model},
    pool::Estimate,
    search::Cell,
};
use std::collections::HashSet;
#[derive(Clone, Copy, PartialEq)]
enum DiscreteDirection {
    Unsupported,
    Cap,
    Floor,
}
#[derive(Clone)]
pub struct Game {
    pub home: usize,
    pub away: usize,
    pub hg: [i32; 3],
    pub ag: [i32; 3],
    pub prob: [f64; 3],
}
#[derive(Clone)]
pub struct Problem {
    pub base: Vec<i32>,
    pub ranked: Vec<bool>,
    pub games: Vec<Game>,
    aggregate_cuts: bool,
    discrete_direction: DiscreteDirection,
    discrete_cuts: bool,
    coalition_branching: bool,
}

// Problem::new limits the model to 64 teams; source and sink add two nodes.
// Fixed storage avoids changing the sampler's heap layout after an early proof.
struct Flow {
    capacity: [[i64; 66]; 66],
    n: usize,
}
impl Flow {
    fn new(n: usize) -> Self {
        Self {
            capacity: [[0; 66]; 66],
            n,
        }
    }
    fn add(&mut self, from: usize, to: usize, capacity: i64) {
        self.capacity[from][to] += capacity;
    }
    fn maximum(&mut self, source: usize, sink: usize, limit: i64) -> i64 {
        // Small, integer-capacity proof graphs need only a few augmentations.
        // An iterative breadth-first augmenting path avoids recursive frames
        // and keeps graph setup to one fixed matrix.
        let mut total = 0;
        while total < limit {
            let mut parent = [usize::MAX; 66];
            parent[source] = source;
            let mut queue = [0; 66];
            queue[0] = source;
            let mut read = 0;
            let mut write = 1;
            while read < write && parent[sink] == usize::MAX {
                let at = queue[read];
                read += 1;
                for to in 0..self.n {
                    if self.capacity[at][to] > 0 && parent[to] == usize::MAX {
                        parent[to] = at;
                        queue[write] = to;
                        write += 1;
                    }
                }
            }
            if parent[sink] == usize::MAX {
                break;
            }
            let mut sent = limit - total;
            let mut at = sink;
            while at != source {
                let from = parent[at];
                sent = sent.min(self.capacity[from][at]);
                at = from;
            }
            at = sink;
            while at != source {
                let from = parent[at];
                self.capacity[from][at] -= sent;
                self.capacity[at][from] += sent;
                at = from;
            }
            total += sent;
        }
        total
    }
}
pub fn min_gain(domain: u8, gain: [i32; 3]) -> i32 {
    (0..3)
        .filter(|o| domain & (1 << o) != 0)
        .map(|o| gain[o])
        .min()
        .unwrap_or(i32::MAX / 4)
}
pub fn max_gain(domain: u8, gain: [i32; 3]) -> i32 {
    (0..3)
        .filter(|o| domain & (1 << o) != 0)
        .map(|o| gain[o])
        .max()
        .unwrap_or(i32::MIN / 4)
}
impl Problem {
    pub fn new(model: &Model) -> Option<Self> {
        if model.ids.len() > 64 || model.fixtures.iter().any(|f| f.home == f.away) {
            return None;
        }
        let c = &model.request.phase.championship;
        let hg = [c.point_loss, c.point_draw, c.point_win];
        let ag = [c.point_win, c.point_draw, c.point_loss];
        Some(Self {
            aggregate_cuts: std::env::var("RUST_ODDS_AGGREGATE_CUTS").as_deref() == Ok("1"),
            discrete_direction: if (c.point_win, c.point_draw, c.point_loss) == (3, 1, 0)
                && model.request.phase.bonus_points == 0
            {
                DiscreteDirection::Cap
            } else {
                DiscreteDirection::Unsupported
            },
            discrete_cuts: crate::search::enabled("RUST_ODDS_DISCRETE_CUTS"),
            coalition_branching: std::env::var("RUST_ODDS_COALITION_BRANCHING").as_deref()
                == Ok("1"),
            base: model.base.iter().map(|c| c.points).collect(),
            ranked: (0..model.ids.len()).map(|t| t < model.n).collect(),
            games: model
                .fixtures
                .iter()
                .map(|f| Game {
                    home: f.home,
                    away: f.away,
                    hg,
                    ag,
                    prob: f.prob,
                })
                .collect(),
        })
    }
    /// Conditional sampler relaxation: fixed outcomes have already been applied
    /// to base and removed from games. This does not publish reachability.
    pub fn conditional(base: Vec<i32>, games: Vec<Game>) -> Option<Self> {
        if base.len() > 64
            || games
                .iter()
                .any(|g| g.home == g.away || g.home >= base.len() || g.away >= base.len())
        {
            return None;
        }
        let standard = games.iter().all(|g| g.hg == [0, 1, 3] && g.ag == [3, 1, 0]);
        Some(Self {
            ranked: vec![true; base.len()],
            base,
            games,
            aggregate_cuts: true,
            discrete_direction: if standard {
                DiscreteDirection::Cap
            } else {
                DiscreteDirection::Unsupported
            },
            discrete_cuts: crate::search::enabled("RUST_ODDS_DISCRETE_CUTS"),
            coalition_branching: std::env::var("RUST_ODDS_COALITION_BRANCHING").as_deref()
                == Ok("1"),
        })
    }
    pub fn negated(&self) -> Self {
        Self {
            aggregate_cuts: self.aggregate_cuts,
            discrete_direction: match self.discrete_direction {
                DiscreteDirection::Cap => DiscreteDirection::Floor,
                DiscreteDirection::Floor => DiscreteDirection::Cap,
                DiscreteDirection::Unsupported => DiscreteDirection::Unsupported,
            },
            discrete_cuts: self.discrete_cuts,
            coalition_branching: self.coalition_branching,
            base: self.base.iter().map(|p| -p).collect(),
            ranked: self.ranked.clone(),
            games: self
                .games
                .iter()
                .map(|g| Game {
                    hg: g.hg.map(|p| -p),
                    ag: g.ag.map(|p| -p),
                    ..g.clone()
                })
                .collect(),
        }
    }
    /// Explicit control for comparisons/tests without changing global flags.
    pub fn with_discrete_cuts(mut self, enabled: bool) -> Self {
        self.discrete_cuts = enabled;
        self
    }
    pub fn with_coalition_branching(mut self, enabled: bool) -> Self {
        self.coalition_branching = enabled;
        self
    }
    /// Check every coalition's discrete floor cut with one small graph cut.
    /// This uses unrestricted W/D/L outcomes, so later goal tiebreakers and
    /// numerical score-table truncation cannot create a false impossibility.
    #[cold]
    #[inline(never)]
    #[cfg(test)]
    fn discrete_floor_impossible(&self, cap: i32, exempt: u64) -> bool {
        self.discrete_floor_cut(cap, exempt, false).is_some()
    }
    #[cold]
    #[inline(never)]
    fn discrete_floor_cut(&self, cap: i32, exempt: u64, certificate: bool) -> Option<u64> {
        let n = self.base.len();
        let source = n;
        let sink = n + 1;
        let mut reward = [0_i64; 64];
        for (t, value) in reward[..n].iter_mut().enumerate() {
            if exempt & (1 << t) == 0 {
                // The problem is negated: deficit = floor - current points.
                let deficit = i64::from(self.base[t]) - i64::from(cap);
                *value = deficit + (deficit.max(0) + 2) / 3;
            }
        }
        for g in &self.games {
            let h = exempt & (1 << g.home) == 0;
            let a = exempt & (1 << g.away) == 0;
            match (h, a) {
                (true, true) => {
                    reward[g.home] -= 2;
                    reward[g.away] -= 2;
                }
                (true, false) => reward[g.home] -= 4,
                (false, true) => reward[g.away] -= 4,
                _ => {}
            }
        }
        let positive: i64 = reward[..n].iter().filter(|v| **v > 0).sum();
        // With no positive unary reward every coalition has nonpositive
        // violation. Avoid constructing a graph for these common easy floors.
        if positive == 0 {
            return None;
        }
        let mut flow = Flow::new(n + 2);
        for g in &self.games {
            if exempt & ((1 << g.home) | (1 << g.away)) == 0 {
                flow.add(g.home, g.away, 2);
                flow.add(g.away, g.home, 2);
            }
        }
        for (t, value) in reward[..n].iter().copied().enumerate() {
            if value > 0 {
                flow.add(source, t, value);
            } else if value < 0 {
                flow.add(t, sink, -value);
            }
        }
        // For S: Q=sum ceil(max(0,floor-current)/3), M=incident games,
        // slack=sum(current)+3M-floor*|S|. Necessary: Q-M <= slack.
        // Violation = sum(deficit+ceil(deficit/3)) - 4M > 0.
        // Each internal fixture contributes -2 at both endpoints and a
        // capacity-2 undirected cut edge; each boundary fixture contributes
        // -4. Maximum coalition violation = positive source reward - mincut.
        if positive <= flow.maximum(source, sink, positive) {
            return None;
        }
        if !certificate {
            return Some(0);
        }
        let mut seen = [false; 66];
        let mut queue = [0; 66];
        seen[source] = true;
        queue[0] = source;
        let mut read = 0;
        let mut write = 1;
        while read < write {
            let at = queue[read];
            read += 1;
            for next in 0..flow.n {
                if !seen[next] && flow.capacity[at][next] > 0 {
                    seen[next] = true;
                    queue[write] = next;
                    write += 1;
                }
            }
        }
        Some(
            (0..n)
                .filter(|t| seen[*t] && exempt & (1 << t) == 0)
                .fold(0, |mask, t| mask | (1 << t)),
        )
    }
    pub fn target_max(&self, t: usize) -> i32 {
        self.base[t]
            + self
                .games
                .iter()
                .map(|g| {
                    if g.home == t {
                        max_gain(7, g.hg)
                    } else if g.away == t {
                        max_gain(7, g.ag)
                    } else {
                        0
                    }
                })
                .sum::<i32>()
    }
    pub fn propagate(&self, cap: i32, exempt: u64, domains: &mut [u8]) -> Option<Vec<i32>> {
        self.propagate_policy(cap, exempt, domains, self.aggregate_cuts)
    }
    pub fn propagate_policy(
        &self,
        cap: i32,
        exempt: u64,
        domains: &mut [u8],
        cuts: bool,
    ) -> Option<Vec<i32>> {
        self.propagate_conflict(cap, exempt, domains, cuts, &mut 0)
    }
    fn propagate_conflict(
        &self,
        cap: i32,
        exempt: u64,
        domains: &mut [u8],
        cuts: bool,
        conflict: &mut u64,
    ) -> Option<Vec<i32>> {
        let mut lower = self.base.clone();
        let mut hm = vec![0; self.games.len()];
        let mut am = hm.clone();
        // Reuse aggregate-cut scratch storage across propagation rounds. This
        // offsets part of the discrete cut cost without changing any search.
        let mut weights = Vec::new();
        let mut teams = Vec::new();
        loop {
            lower.copy_from_slice(&self.base);
            for (i, g) in self.games.iter().enumerate() {
                if domains[i] == 0 {
                    return None;
                }
                hm[i] = min_gain(domains[i], g.hg);
                am[i] = min_gain(domains[i], g.ag);
                lower[g.home] += hm[i];
                lower[g.away] += am[i];
            }
            if lower
                .iter()
                .enumerate()
                .any(|(t, p)| exempt & (1 << t) == 0 && *p > cap)
            {
                return None;
            }
            if cuts {
                let n = self.base.len();
                if weights.is_empty() {
                    weights.resize(n * n, 0);
                    teams.extend((0..n).filter(|t| exempt & (1 << t) == 0));
                } else {
                    weights.fill(0);
                }
                for (i, g) in self.games.iter().enumerate() {
                    if exempt & (1 << g.home) != 0 || exempt & (1 << g.away) != 0 {
                        continue;
                    }
                    let together = min_gain(domains[i], std::array::from_fn(|o| g.hg[o] + g.ag[o]));
                    let extra = together - hm[i] - am[i];
                    weights[g.home * n + g.away] += extra;
                    weights[g.away * n + g.home] += extra;
                }
                teams.sort_by_key(|t| (cap - lower[*t], *t));
                let mut capacity = 0;
                let mut demand = 0;
                for (index, &t) in teams.iter().enumerate() {
                    capacity += cap - lower[t];
                    for &other in &teams[..index] {
                        demand += weights[t * n + other];
                    }
                    if demand > capacity {
                        return None;
                    }
                }
            }
            let mut changed = false;
            for (i, g) in self.games.iter().enumerate() {
                let mut domain = domains[i];
                for o in 0..3 {
                    let bit = 1 << o;
                    if domain & bit == 0 {
                        continue;
                    }
                    if (exempt & (1 << g.home) == 0 && lower[g.home] - hm[i] + g.hg[o] > cap)
                        || (exempt & (1 << g.away) == 0 && lower[g.away] - am[i] + g.ag[o] > cap)
                    {
                        domain &= !bit;
                        changed = true;
                    }
                }
                if domain == 0 {
                    return None;
                }
                domains[i] = domain;
            }
            if !changed {
                if cuts && self.discrete_cuts && self.discrete_direction == DiscreteDirection::Floor
                {
                    if let Some(mask) =
                        self.discrete_floor_cut(cap, exempt, self.coalition_branching)
                    {
                        *conflict = mask;
                        return None;
                    }
                }
                return Some(lower);
            }
        }
    }
    pub fn impossible(&self, t: usize, rank: usize, cap: i32, budget: usize) -> (bool, usize) {
        self.impossible_domains(t, rank, cap, &vec![7; self.games.len()], budget)
    }
    pub fn impossible_domains(
        &self,
        t: usize,
        rank: usize,
        cap: i32,
        domains: &[u8],
        budget: usize,
    ) -> (bool, usize) {
        assert_eq!(domains.len(), self.games.len());
        if domains.iter().any(|d| *d == 0) {
            return (true, 0);
        }
        let mut exempt = 1 << t;
        let mut mandatory = 0;
        let mut min = self.base.clone();
        for (g, d) in self.games.iter().zip(domains) {
            min[g.home] += min_gain(*d, g.hg);
            min[g.away] += min_gain(*d, g.ag);
        }
        for (i, p) in min.iter().enumerate() {
            if !self.ranked[i] {
                exempt |= 1 << i;
            } else if i != t && *p > cap {
                exempt |= 1 << i;
                mandatory += 1;
            }
        }
        if mandatory > rank {
            return (true, 0);
        }
        fn visit(
            p: &Problem,
            cap: i32,
            mask: u64,
            slots: usize,
            budget: usize,
            nodes: &mut usize,
            seen: &mut HashSet<u64>,
            domains: &[u8],
        ) -> u8 {
            if !seen.insert(mask) {
                return 2;
            }
            if *nodes >= budget {
                return 0;
            }
            *nodes += 1;
            let mut conflict = 0;
            if p.propagate_conflict(
                cap,
                mask,
                &mut domains.to_vec(),
                p.aggregate_cuts,
                &mut conflict,
            )
            .is_some()
            {
                return 1;
            }
            if slots == 0 {
                return 2;
            }
            // Every repair must exempt at least one member of this violated
            // coalition. Exempting an outside team cannot change its incident
            // fixture total or its floor requirement. Branch on this hitting set.
            let coalition = if p.coalition_branching
                && p.aggregate_cuts
                && p.discrete_cuts
                && p.discrete_direction == DiscreteDirection::Floor
            {
                if conflict != 0 {
                    conflict
                } else {
                    // Ordinary propagation can fail before running the floor
                    // cut. In that case seek a certificate once, not twice.
                    p.discrete_floor_cut(cap, mask, true).unwrap_or(0)
                }
            } else {
                0
            };
            for i in 0..p.base.len() {
                if mask & (1 << i) != 0
                    || !p.ranked[i]
                    || coalition != 0 && coalition & (1 << i) == 0
                {
                    continue;
                }
                let result = visit(
                    p,
                    cap,
                    mask | (1 << i),
                    slots - 1,
                    budget,
                    nodes,
                    seen,
                    domains,
                );
                if result != 2 {
                    return result;
                }
            }
            2
        }
        let mut nodes = 0;
        let outcome = visit(
            self,
            cap,
            exempt,
            rank - mandatory,
            budget,
            &mut nodes,
            &mut HashSet::new(),
            domains,
        );
        (outcome == 2, nodes)
    }
}
pub fn early(model: &Model, cells: &[Cell], estimates: &mut [Estimate]) -> usize {
    early_report(model, cells, estimates).proofs
}
#[derive(Default)]
pub struct EarlyReport {
    pub proofs: usize,
    pub recycle_credit: usize,
}
pub fn early_report(model: &Model, cells: &[Cell], estimates: &mut [Estimate]) -> EarlyReport {
    if model.keys.first() != Some(&Key::Pt) {
        return EarlyReport::default();
    }
    let Some(mut p) = Problem::new(model) else {
        return EarlyReport::default();
    };
    if std::env::var("RUST_ODDS_AGGREGATE_CUTS").unwrap_or_else(|_| "early".into()) == "early" {
        p.aggregate_cuts = true;
    }
    let rank_proof = (!cells.is_empty()
        && matches!(std::env::var("RUST_ODDS_RANK_PROOF").as_deref(), Err(_) | Ok("1")))
    .then(|| crate::rank_proof::RankProof::new(model))
    .flatten();
    let mut proofs = 0;
    let mut recycle_credit = 0;
    let preserve_credit = p.aggregate_cuts
        && std::env::var("RUST_ODDS_PROOF_RECYCLE_CREDIT").unwrap_or_else(|_| "legacy".into())
            == "legacy";
    for dual in [true, false] {
        if !crate::search::enabled(if dual {
            "RARE_POSITION_JOINT_POINT_FLOOR"
        } else {
            "RARE_POSITION_JOINT_POINT_CAP"
        }) {
            continue;
        }
        let start = std::time::Instant::now();
        let before = proofs;
        let negated = p.negated();
        let problem = if dual { &negated } else { &p };
        let credit_problem = preserve_credit.then(|| {
            let mut legacy = problem.clone();
            legacy.aggregate_cuts = false;
            legacy.discrete_cuts = false;
            legacy
        });
        let packed_negative = (!dual)
            .then(|| rank_proof.as_ref().map(|p| p.negated()))
            .flatten();
        let packed = if dual {
            rank_proof.as_ref()
        } else {
            packed_negative.as_ref()
        };
        let mut nodes = 0;
        for &cell in cells {
            let index = cell.index(model.n);
            let est = &mut estimates[index];
            if est.probability != 0. || est.reachability != "undecided" || nodes >= 10000 {
                continue;
            }
            let rank = if dual {
                model.n - 1 - cell.rank
            } else {
                cell.rank
            };
            let cap = problem.target_max(cell.team);
            let allowance = 500.min(10000 - nodes);
            let (mut impossible, mut spent) = problem.impossible(cell.team, rank, cap, allowance);
            let mut rank_impossible = false;
            // Spend fixture-search work only where the existing cohort proof
            // already encountered a conflict. A one-node consistent relaxation
            // is a weak priority signal, and retains its existing undecided state.
            if !impossible && spent > 1 && spent < allowance {
                if let Some(packed) = packed {
                    let timer = std::time::Instant::now();
                    let result = packed.prove(
                        cell,
                        if dual {
                            cell.rank
                        } else {
                            model.n - 1 - cell.rank
                        },
                        allowance - spent,
                        crate::rank_proof::Options {
                            require_root_pressure: true,
                            ..crate::rank_proof::Options::default()
                        },
                    );
                    spent += result.stats.nodes;
                    rank_impossible = result.impossible;
                    impossible |= rank_impossible;
                    if std::env::var("RUST_ODDS_REACHABILITY_TRACE").as_deref() == Ok("1") {
                        eprintln!(
                            "{}",
                            serde_json::json!({"event":"rust_reachability_rank_proof","group":model.request.id,
                            "team":model.ids[cell.team],"rank":cell.rank+1,"direction":if dual {"floor"}else{"cap"},
                            "impossible":rank_impossible,"stats":result.stats,"elapsed_ms":timer.elapsed().as_secs_f64()*1000.})
                        );
                    }
                }
            }
            nodes += spent;
            if impossible {
                est.reachability = if rank_impossible {
                    "impossible_by_joint_rank"
                } else {
                    "impossible_by_joint_points"
                }
                .into();
                est.zero_hit_upper_95 = 0.;
                proofs += 1;
                if preserve_credit {
                    // New cuts may remove a zero, but do not automatically add
                    // new probability draws. Check credit with the old relaxation
                    // inside the same cell and direction node limits.
                    let (old_proof, credit_nodes) = credit_problem.as_ref().unwrap().impossible(
                        cell.team,
                        rank,
                        cap,
                        allowance - spent,
                    );
                    nodes += credit_nodes;
                    recycle_credit += usize::from(old_proof);
                } else {
                    // A new fixture proof does not mint extra probability draws.
                    recycle_credit += usize::from(!rank_impossible);
                }
            }
        }
        if std::env::var("RUST_ODDS_REACHABILITY_TRACE").as_deref() == Ok("1") {
            eprintln!(
                "{}",
                serde_json::json!({"event":"rust_reachability_early","group":model.request.id,
                "direction":if dual{"floor"}else{"cap"},"nodes":nodes,"found":proofs-before,
                "node_limit":10000,"cell_node_limit":500,"elapsed_ms":start.elapsed().as_secs_f64()*1000.})
            );
        }
    }
    EarlyReport {
        proofs,
        recycle_credit,
    }
}
struct Witness<'a> {
    problem: &'a Problem,
    model: &'a Model,
    cell: Cell,
    constraint_rank: usize,
    minimum: bool,
    cap: i32,
    nodes: usize,
    budget: usize,
    goal_completion: bool,
    goal_attempts: usize,
    goal_ms: f64,
    goal_witness: Option<crate::goal_completion::Certificate>,
}
impl Witness<'_> {
    fn assign(&mut self, mut domains: Vec<u8>, exempt: u64) -> Option<Vec<u8>> {
        if self.nodes >= self.budget {
            return None;
        }
        self.nodes += 1;
        let lower = self.problem.propagate(self.cap, exempt, &mut domains)?;
        let mut best = None;
        let mut size = 4;
        let mut slack = i32::MAX;
        for (i, g) in self.problem.games.iter().enumerate() {
            let count = domains[i].count_ones();
            if count <= 1 {
                continue;
            }
            let hs = if exempt & (1 << g.home) == 0 {
                self.cap - lower[g.home]
            } else {
                1000000
            };
            let as_ = if exempt & (1 << g.away) == 0 {
                self.cap - lower[g.away]
            } else {
                1000000
            };
            let s = hs.min(as_);
            if count < size || count == size && s < slack {
                best = Some(i);
                size = count;
                slack = s;
            }
        }
        let Some(best) = best else {
            let outcomes: Vec<_> = domains.iter().map(|d| d.trailing_zeros() as u8).collect();
            if canonical_ranks(self.model, &outcomes)[self.cell.team] == self.cell.rank {
                return Some(outcomes);
            }
            if self.goal_completion && self.goal_attempts < 2 && self.goal_witness.is_none() {
                self.goal_attempts += 1;
                let start = std::time::Instant::now();
                self.goal_witness =
                    crate::goal_completion::complete(self.model, &outcomes, self.cell);
                self.goal_ms += start.elapsed().as_secs_f64() * 1000.;
                // Minimum-path witnesses are deferred already. Stop an otherwise
                // wasted branch. Maximum-path seeds retain their original search.
                if self.minimum && self.goal_witness.is_some() {
                    return Some(outcomes);
                }
            }
            return None;
        };
        let g = &self.problem.games[best];
        let penalty = |o: usize| {
            let home = if exempt & (1 << g.home) == 0 {
                g.hg[o] as f64 / (self.cap - lower[g.home] + 1).max(1) as f64
            } else {
                0.
            };
            let away = if exempt & (1 << g.away) == 0 {
                g.ag[o] as f64 / (self.cap - lower[g.away] + 1).max(1) as f64
            } else {
                0.
            };
            home + away
        };
        let mut options: Vec<_> = (0..3).filter(|o| domains[best] & (1 << o) != 0).collect();
        options.sort_by(|a, b| penalty(*a).total_cmp(&penalty(*b)));
        for o in options {
            if self.nodes >= self.budget {
                break;
            }
            let mut next = domains.clone();
            next[best] = 1 << o;
            if let Some(outcomes) = self.assign(next, exempt) {
                return Some(outcomes);
            }
        }
        None
    }
    fn choose(
        &mut self,
        initial: &[u8],
        candidates: &[usize],
        mask: u64,
        start: usize,
        left: usize,
    ) -> Option<Vec<u8>> {
        if self.nodes >= self.budget {
            return None;
        }
        if left == 0 {
            return self.assign(initial.to_vec(), mask);
        }
        for i in start..=candidates.len() - left {
            if let Some(o) = self.choose(
                initial,
                candidates,
                mask | (1 << candidates[i]),
                i + 1,
                left - 1,
            ) {
                return Some(o);
            }
            if self.nodes >= self.budget {
                break;
            }
        }
        None
    }
    fn find(&mut self) -> Option<Vec<u8>> {
        let t = self.cell.team;
        let mut initial = Vec::new();
        for g in &self.problem.games {
            let gains = if g.home == t {
                Some(g.hg)
            } else if g.away == t {
                Some(g.ag)
            } else {
                None
            };
            let best = gains.map(|gain| {
                (0..3)
                    .filter(|o| g.prob[*o] > 0.)
                    .map(|o| gain[o])
                    .max()
                    .unwrap_or(i32::MIN)
            });
            let mut domain = 0;
            for o in 0..3 {
                if g.prob[o] > 0. && gains.is_none_or(|gain| Some(gain[o]) == best) {
                    domain |= 1 << o;
                }
            }
            if domain == 0 {
                return None;
            }
            initial.push(domain);
        }
        let mut lower = self.problem.base.clone();
        let mut upper = self.minimum.then(|| self.problem.base.clone());
        if self.minimum {
            // Negated gains can lower a rival below its current value. Current
            // points alone do not make that rival a mandatory exemption.
            for (g, &d) in self.problem.games.iter().zip(&initial) {
                lower[g.home] += min_gain(d, g.hg);
                lower[g.away] += min_gain(d, g.ag);
                let u = upper.as_mut().unwrap();
                u[g.home] += max_gain(d, g.hg);
                u[g.away] += max_gain(d, g.ag);
            }
        }
        let mut mandatory = 1 << t;
        let mut count = 0;
        for (i, p) in lower.iter().enumerate() {
            if !self.problem.ranked[i] {
                mandatory |= 1 << i;
            } else if i != t && *p > self.cap {
                mandatory |= 1 << i;
                count += 1;
            }
        }
        if count > self.constraint_rank {
            return None;
        }
        let spare = self.constraint_rank - count;
        let mut candidates: Vec<_> = (0..self.problem.base.len())
            .filter(|i| {
                mandatory & (1 << i) == 0
                    && self.problem.ranked[*i]
                    && upper.as_ref().is_none_or(|u| u[*i] >= self.cap)
            })
            .collect();
        candidates.sort_by_key(|i| (-lower[*i], *i));
        let count = spare.min(candidates.len());
        for step in 0..=count {
            let extra = if self.minimum { count - step } else { step };
            if let Some(o) = self.choose(&initial, &candidates, mandatory, 0, extra) {
                return Some(o);
            }
            if self.nodes >= self.budget {
                break;
            }
        }
        None
    }
}
/// Deferred completion preserves the sampler's original proposal priorities.
pub fn witness_mode() -> String {
    std::env::var("RUST_ODDS_WITNESS_MODE").unwrap_or_else(|_| "deferred".into())
}
pub fn witnesses(model: &Model, cells: &[Cell], estimates: &mut [Estimate]) -> Vec<Vec<u8>> {
    let mode = witness_mode();
    witnesses_mode(model, cells, estimates, &mode)
}
pub fn witnesses_mode(
    model: &Model,
    cells: &[Cell],
    estimates: &mut [Estimate],
    mode: &str,
) -> Vec<Vec<u8>> {
    witnesses_report(model, cells, estimates, mode).outcomes
}
#[derive(Default)]
pub struct WitnessReport {
    pub outcomes: Vec<Vec<u8>>,
    pub deferred: Vec<Vec<u8>>,
    pub nodes: usize,
    pub goal_witnesses: Vec<crate::goal_completion::Certificate>,
}
pub fn witnesses_report(
    model: &Model,
    cells: &[Cell],
    estimates: &mut [Estimate],
    mode: &str,
) -> WitnessReport {
    let goals = crate::search::enabled("RUST_ODDS_GOAL_COMPLETION");
    witnesses_report_with_goals(model, cells, estimates, mode, goals)
}
/// Explicit options also allow tests to exercise deferred integration without
/// changing process-wide environment while other tests are running.
pub fn witnesses_report_with_goals(
    model: &Model,
    cells: &[Cell],
    estimates: &mut [Estimate],
    mode: &str,
    goals: bool,
) -> WitnessReport {
    if !crate::search::enabled("RARE_POSITION_JOINT_POINT_WITNESS")
        || model.keys.first() != Some(&Key::Pt)
    {
        return WitnessReport::default();
    }
    if mode == "joint" {
        let result = crate::reachability::run(model, cells, estimates, 100, 3200);
        return WitnessReport {
            outcomes: result.outcomes,
            nodes: result.stats.nodes,
            deferred: Vec::new(),
            goal_witnesses: Vec::new(),
        };
    }
    let Some(problem) = Problem::new(model) else {
        return WitnessReport::default();
    };
    let negated = if matches!(mode, "dual" | "adaptive" | "deferred") {
        Some(problem.negated())
    } else {
        None
    };
    let start = std::time::Instant::now();
    let rank_screen = if matches!(mode, "adaptive" | "deferred" | "skip") {
        crate::reachability::JointProblem::new(model)
    } else {
        None
    };
    let mut skipped_maximum = 0;
    let mut skipped_both = 0;
    let mut nodes = 0;
    let mut found = Vec::new();
    let mut deferred = Vec::new();
    let mut goal_witnesses = Vec::new();
    let goal_completion = mode == "deferred" && goals;
    let mut goal_attempts = 0;
    let mut goal_ms = 0.;
    for &cell in cells {
        let est = &mut estimates[cell.index(model.n)];
        if est.probability != 0. || est.reachability != "undecided" || nodes >= 3200 {
            continue;
        }
        let screened_maximum = rank_screen
            .as_ref()
            .is_some_and(|p| !p.extreme_possible(model, cell, true));
        if screened_maximum {
            skipped_maximum += 1;
            if mode == "skip" {
                continue;
            }
            if !rank_screen
                .as_ref()
                .unwrap()
                .extreme_possible(model, cell, false)
            {
                skipped_both += 1;
                continue;
            }
        }
        let dual = (mode == "dual" && cell.rank > model.n - 1 - cell.rank) || screened_maximum;
        let p = if dual {
            negated.as_ref().unwrap()
        } else {
            &problem
        };
        let mut search = Witness {
            problem: p,
            model,
            cell,
            constraint_rank: if dual {
                model.n - 1 - cell.rank
            } else {
                cell.rank
            },
            minimum: dual,
            cap: p.target_max(cell.team),
            nodes: 0,
            budget: 100.min(3200 - nodes),
            goal_completion,
            goal_attempts: 0,
            goal_ms: 0.,
            goal_witness: None,
        };
        if let Some(o) = search.find() {
            if mode == "deferred" && dual {
                deferred.push(o);
            } else {
                est.reachability = "reachable_by_construction".into();
                if mode == "dual" || mode == "reuse" || mode == "adaptive" {
                    for (team, rank) in canonical_ranks(model, &o).into_iter().enumerate() {
                        let e = &mut estimates[team * model.n + rank];
                        if e.probability == 0. && e.reachability == "undecided" {
                            e.reachability = "reachable_by_construction".into();
                        }
                    }
                }
                found.push(o);
            }
        }
        if let Some(proof) = search.goal_witness.take() {
            goal_witnesses.push(proof);
        }
        nodes += search.nodes;
        goal_attempts += search.goal_attempts;
        goal_ms += search.goal_ms;
    }
    if std::env::var("RUST_ODDS_REACHABILITY_TRACE").as_deref() == Ok("1") {
        eprintln!(
            "{}",
            serde_json::json!({"event":"rust_reachability_constructive","group":model.request.id,
            "mode":if mode.is_empty(){"legacy"}else{mode},"nodes":nodes,"found":found.len(),
            "skipped_maximum":skipped_maximum,"skipped_both":skipped_both,
            "deferred":deferred.len(),
            "goal_witnesses":goal_witnesses.len(),
            "goal_attempts":goal_attempts,"goal_ms":goal_ms,
            "elapsed_ms":start.elapsed().as_secs_f64()*1000.,"node_limit":3200,"cell_node_limit":100})
        );
    }
    WitnessReport {
        outcomes: found,
        deferred,
        nodes,
        goal_witnesses,
    }
}

#[cfg(test)]
mod discrete_tests {
    use super::*;
    use crate::model::Request;

    #[test]
    fn graph_cut_matches_exhaustive_coalition_certificates() {
        let request: Request = serde_json::from_value(serde_json::json!({
            "id":1,"phase":{"sort":"pt","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":[{"team_id":0,"add_sub":-3},{"team_id":1,"add_sub":1},{"team_id":2,"add_sub":4},{"team_id":3,"add_sub":8},{"team_id":4,"add_sub":2}],
            "games":[{"id":0,"home_id":0,"away_id":1},{"id":1,"home_id":0,"away_id":2},{"id":2,"home_id":1,"away_id":2},{"id":3,"home_id":1,"away_id":3},{"id":4,"home_id":2,"away_id":4},{"id":5,"home_id":3,"away_id":4},{"id":6,"home_id":0,"away_id":4}]
        })).unwrap();
        let model = Model::new(request).unwrap();
        let p = Problem::new(&model).unwrap().negated();
        for floor in -3..=15 {
            for exempt in 0..32_u64 {
                let mut expected = false;
                for subset in 1..32_u64 {
                    if subset & exempt != 0 {
                        continue;
                    }
                    let mut current = 0_i64;
                    let mut needed = 0_i64;
                    let mut count = 0;
                    for t in 0..5 {
                        if subset & (1 << t) != 0 {
                            let points = i64::from(model.base[t].points);
                            current += points;
                            needed += ((i64::from(floor) - points).max(0) + 2) / 3;
                            count += 1;
                        }
                    }
                    let incident = p
                        .games
                        .iter()
                        .filter(|g| subset & ((1 << g.home) | (1 << g.away)) != 0)
                        .count() as i64;
                    let slack = current + 3 * incident - i64::from(floor) * count;
                    expected |= needed - incident > slack;
                }
                let certificate = p.discrete_floor_cut(-floor, exempt, true);
                assert_eq!(certificate.is_some(), expected);
                if let Some(coalition) = certificate {
                    assert_ne!(coalition, 0);
                    assert_eq!(coalition & exempt, 0);
                    let requirement: i64 = (0..5)
                        .filter(|t| coalition & (1 << t) != 0)
                        .map(|t| {
                            let deficit = i64::from(floor - model.base[t].points);
                            deficit + (deficit.max(0) + 2) / 3
                        })
                        .sum();
                    let incident = p
                        .games
                        .iter()
                        .filter(|g| coalition & ((1 << g.home) | (1 << g.away)) != 0)
                        .count() as i64;
                    assert!(requirement > 4 * incident);
                }
                assert_eq!(
                    p.discrete_floor_impossible(-floor, exempt),
                    expected,
                    "floor={floor} exempt={exempt}"
                );
            }
        }
    }

    #[test]
    fn coalition_branching_preserves_full_budget_proof_answers() {
        let request:Request=serde_json::from_value(serde_json::json!({
            "id":1,"phase":{"sort":"pt","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":[{"team_id":0,"add_sub":0},{"team_id":1,"add_sub":1},{"team_id":2,"add_sub":4},{"team_id":3,"add_sub":8},{"team_id":4,"add_sub":2}],
            "games":[{"id":0,"home_id":0,"away_id":1},{"id":1,"home_id":0,"away_id":2},{"id":2,"home_id":1,"away_id":2},{"id":3,"home_id":1,"away_id":3},{"id":4,"home_id":2,"away_id":4},{"id":5,"home_id":3,"away_id":4}]
        })).unwrap();
        let model = Model::new(request).unwrap();
        let mut p = Problem::new(&model)
            .unwrap()
            .negated()
            .with_discrete_cuts(true);
        p.aggregate_cuts = true;
        let q = p.clone().with_coalition_branching(true);
        let mut best = vec![5; 20];
        for mut code in 0..3usize.pow(p.games.len() as u32) {
            let mut points = model.base.iter().map(|b| b.points).collect::<Vec<_>>();
            for g in &p.games {
                let o = code % 3;
                code /= 3;
                points[g.home] -= g.hg[o];
                points[g.away] -= g.ag[o];
            }
            for floor in 0..20 {
                best[floor] =
                    best[floor].min((0..4).filter(|t| points[*t] < (floor as i32)).count());
            }
        }
        for floor in 0..20 {
            for rank in 0..5 {
                let a = p.impossible(4, rank, -(floor as i32), 10000);
                let b = q.impossible(4, rank, -(floor as i32), 10000);
                assert_eq!(a.0, b.0, "floor={floor} rank={rank}");
                if b.0 {
                    assert!(best[floor] > rank, "false impossibility");
                }
            }
        }
    }
    #[test]
    fn complete_early_floor_proof_resolves_seventeenth_with_existing_budget() {
        let request: Request = serde_json::from_str(include_str!(
            "../../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json"
        )).unwrap();
        let model = Model::new(request).unwrap();
        let t = model.indices[&16];
        let mut p = Problem::new(&model)
            .unwrap()
            .negated()
            .with_discrete_cuts(true);
        p.aggregate_cuts = true;
        let cap = p.target_max(t);
        let (proof, nodes) = p.impossible(t, 3, cap, 500);
        assert!(proof);
        assert!(nodes <= 500);
        for below in [4, 5] {
            assert!(!p.impossible(t, below, cap, 500).0);
        }
        p.discrete_cuts = false;
        assert!(!p.impossible(t, 3, cap, 500).0);
    }
}
