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
#[derive(Clone)]
pub struct Problem {
    pub base: Vec<i32>,
    pub ranked: Vec<bool>,
    pub games: Vec<Game>,
    aggregate_cuts: bool,
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
            aggregate_cuts: self.aggregate_cuts,
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
        self.propagate_policy(cap, exempt, domains, self.aggregate_cuts)
    }
    pub fn propagate_policy(
        &self,
        cap: i32,
        exempt: u64,
        domains: &mut [u8],
        cuts: bool,
    ) -> Option<Vec<i32>> {
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
            if cuts {
                let n = self.base.len();
                let mut weights = vec![0; n * n];
                for (i, g) in self.games.iter().enumerate() {
                    if exempt & (1 << g.home) != 0 || exempt & (1 << g.away) != 0 {
                        continue;
                    }
                    let together = min_gain(domains[i], std::array::from_fn(|o| g.hg[o] + g.ag[o]));
                    let extra = together - hm[i] - am[i];
                    weights[g.home * n + g.away] += extra;
                    weights[g.away * n + g.home] += extra;
                }
                let mut teams: Vec<_> = (0..n).filter(|t| exempt & (1 << t) == 0).collect();
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
            legacy
        });
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
            let (impossible, spent) = problem.impossible(cell.team, rank, cap, allowance);
            nodes += spent;
            if impossible {
                est.reachability = "impossible_by_joint_points".into();
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
                    recycle_credit += 1;
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
