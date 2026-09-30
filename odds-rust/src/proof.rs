use crate::{
    conditioned::canonical_ranks,
    model::{Key, Model},
    pool::Estimate,
    search::Cell,
};
use std::collections::HashSet;
#[derive(Clone)]
pub struct Game {
    pub home: usize,
    pub away: usize,
    pub hg: [i32; 3],
    pub ag: [i32; 3],
    pub prob: [f64; 3],
}
pub struct Problem {
    pub base: Vec<i32>,
    pub ranked: Vec<bool>,
    pub games: Vec<Game>,
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
    pub fn negated(&self) -> Self {
        Self {
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
        let mut lower = self.base.clone();
        let mut hm = vec![0; self.games.len()];
        let mut am = hm.clone();
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
                return Some(lower);
            }
        }
    }
    pub fn impossible(&self, t: usize, rank: usize, cap: i32, budget: usize) -> (bool, usize) {
        let mut exempt = 1 << t;
        let mut mandatory = 0;
        let mut min = self.base.clone();
        for g in &self.games {
            min[g.home] += min_gain(7, g.hg);
            min[g.away] += min_gain(7, g.ag);
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
        ) -> u8 {
            if !seen.insert(mask) {
                return 2;
            }
            if *nodes >= budget {
                return 0;
            }
            *nodes += 1;
            if p.propagate(cap, mask, &mut vec![7; p.games.len()])
                .is_some()
            {
                return 1;
            }
            if slots == 0 {
                return 2;
            }
            for i in 0..p.base.len() {
                if mask & (1 << i) != 0 || !p.ranked[i] {
                    continue;
                }
                let result = visit(p, cap, mask | (1 << i), slots - 1, budget, nodes, seen);
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
        );
        (outcome == 2, nodes)
    }
}
pub fn early(model: &Model, cells: &[Cell], estimates: &mut [Estimate]) -> usize {
    if model.keys.first() != Some(&Key::Pt) {
        return 0;
    }
    let Some(p) = Problem::new(model) else {
        return 0;
    };
    let mut proofs = 0;
    for dual in [true, false] {
        if !crate::search::enabled(if dual {
            "RARE_POSITION_JOINT_POINT_FLOOR"
        } else {
            "RARE_POSITION_JOINT_POINT_CAP"
        }) {
            continue;
        }
        let negated = p.negated();
        let problem = if dual { &negated } else { &p };
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
            let (impossible, spent) =
                problem.impossible(cell.team, rank, cap, 500.min(10000 - nodes));
            nodes += spent;
            if impossible {
                est.reachability = "impossible_by_joint_points".into();
                est.zero_hit_upper_95 = 0.;
                proofs += 1;
            }
        }
    }
    proofs
}
struct Witness<'a> {
    problem: &'a Problem,
    model: &'a Model,
    cell: Cell,
    cap: i32,
    nodes: usize,
    budget: usize,
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
            return if canonical_ranks(self.model, &outcomes)[self.cell.team] == self.cell.rank {
                Some(outcomes)
            } else {
                None
            };
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
        let mut mandatory = 1 << t;
        let mut count = 0;
        for (i, p) in self.problem.base.iter().enumerate() {
            if !self.problem.ranked[i] {
                mandatory |= 1 << i;
            } else if i != t && *p > self.cap {
                mandatory |= 1 << i;
                count += 1;
            }
        }
        if count > self.cell.rank {
            return None;
        }
        let spare = self.cell.rank - count;
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
        let mut candidates: Vec<_> = (0..self.problem.base.len())
            .filter(|i| mandatory & (1 << i) == 0 && self.problem.ranked[*i])
            .collect();
        candidates.sort_by_key(|i| (-self.problem.base[*i], *i));
        for extra in 0..=spare.min(candidates.len()) {
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
pub fn witnesses(model: &Model, cells: &[Cell], estimates: &mut [Estimate]) -> Vec<Vec<u8>> {
    if !crate::search::enabled("RARE_POSITION_JOINT_POINT_WITNESS")
        || model.keys.first() != Some(&Key::Pt)
    {
        return Vec::new();
    }
    let Some(problem) = Problem::new(model) else {
        return Vec::new();
    };
    let mut nodes = 0;
    let mut found = Vec::new();
    for &cell in cells {
        let est = &mut estimates[cell.index(model.n)];
        if est.probability != 0. || est.reachability != "undecided" || nodes >= 3200 {
            continue;
        }
        let mut search = Witness {
            problem: &problem,
            model,
            cell,
            cap: problem.target_max(cell.team),
            nodes: 0,
            budget: 100.min(3200 - nodes),
        };
        if let Some(o) = search.find() {
            est.reachability = "reachable_by_construction".into();
            found.push(o);
        }
        nodes += search.nodes;
    }
    found
}
