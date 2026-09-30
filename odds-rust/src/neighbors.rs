use crate::{
    conditioned::canonical_ranks,
    model::{Key, Model},
    pool::Estimate,
    search::{enabled, Cell},
};
use std::collections::HashSet;
#[derive(Clone)]
pub struct Proof {
    pub cell: Cell,
    pub outcomes: Vec<u8>,
}
struct Checker<'a> {
    model: &'a Model,
    unresolved: Vec<Cell>,
    proofs: Vec<Proof>,
    points: Vec<i32>,
    wins: Vec<i32>,
    candidate: Vec<u8>,
    attempts: usize,
    ties: usize,
    total_ties: usize,
    tie_limit: usize,
    use_wins: bool,
    verify_all: bool,
    sorter_checks: usize,
}
impl Checker<'_> {
    fn check(&mut self) {
        self.attempts += 1;
        let mut remaining = Vec::new();
        for &cell in &self.unresolved {
            let p = self.points[cell.team];
            let w = self.wins[cell.team];
            let mut above = 0;
            let mut tied = 0;
            for t in 0..self.model.n {
                if t == cell.team {
                    continue;
                }
                if self.points[t] > p || self.use_wins && self.points[t] == p && self.wins[t] > w {
                    above += 1;
                } else if self.points[t] == p && (!self.use_wins || self.wins[t] == w) {
                    tied += 1;
                }
            }
            if cell.rank < above || cell.rank > above + tied {
                remaining.push(cell);
                continue;
            }
            let mut verified = tied == 0 && above == cell.rank;
            if verified && self.verify_all {
                self.sorter_checks += 1;
                verified = canonical_ranks(self.model, &self.candidate)[cell.team] == cell.rank;
            }
            if !verified && self.ties < self.tie_limit && self.total_ties < 500 {
                self.ties += 1;
                self.total_ties += 1;
                self.sorter_checks += 1;
                verified = canonical_ranks(self.model, &self.candidate)[cell.team] == cell.rank;
            }
            if verified {
                self.proofs.push(Proof {
                    cell,
                    outcomes: self.candidate.clone(),
                });
            } else {
                remaining.push(cell);
            }
        }
        self.unresolved = remaining;
    }
    fn apply(&mut self, i: usize, next: u8) {
        let f = &self.model.fixtures[i];
        let old = self.candidate[i] as usize;
        let next = next as usize;
        let c = &self.model.request.phase.championship;
        let hp = [c.point_loss, c.point_draw, c.point_win];
        let ap = [c.point_win, c.point_draw, c.point_loss];
        self.points[f.home] += hp[next] - hp[old];
        self.points[f.away] += ap[next] - ap[old];
        self.wins[f.home] += i32::from(next == 2) - i32::from(old == 2);
        self.wins[f.away] += i32::from(next == 0) - i32::from(old == 0);
        self.candidate[i] = next as u8;
    }
}
pub fn search(
    model: &Model,
    cells: &[Cell],
    assignments: &[Vec<u8>],
    estimates: &[Estimate],
    budget: usize,
) -> Vec<Proof> {
    let order = std::env::var("RUST_ODDS_NEIGHBOR_ORDER").unwrap_or_default();
    search_order(model, cells, assignments, estimates, budget, &order)
}
pub fn search_order(
    model: &Model,
    cells: &[Cell],
    assignments: &[Vec<u8>],
    estimates: &[Estimate],
    budget: usize,
    order: &str,
) -> Vec<Proof> {
    if !enabled("RARE_POSITION_NEIGHBORHOOD_SEARCH")
        || model.keys.first() != Some(&Key::Pt)
        || budget == 0
        || model.fixtures.is_empty()
        || model
            .fixtures
            .iter()
            .any(|f| f.home >= model.n || f.away >= model.n || f.home == f.away)
    {
        return Vec::new();
    }
    let unresolved: Vec<_> = cells
        .iter()
        .copied()
        .filter(|c| {
            let e = &estimates[c.index(model.n)];
            e.probability == 0. && e.reachability == "undecided"
        })
        .collect();
    if unresolved.is_empty() {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    let seeds: Vec<_> = assignments
        .iter()
        .filter(|a| a.len() == model.fixtures.len() && seen.insert((*a).clone()))
        .collect();
    if seeds.is_empty() {
        return Vec::new();
    }
    let per_seed = (budget / seeds.len()).max(1);
    let tie_limit = (500 / seeds.len()).max(1);
    let mut check = Checker {
        model,
        unresolved,
        proofs: Vec::new(),
        points: vec![0; model.ids.len()],
        wins: vec![0; model.ids.len()],
        candidate: Vec::new(),
        attempts: 0,
        ties: 0,
        total_ties: 0,
        tie_limit,
        use_wins: model.keys.get(1) == Some(&Key::W),
        verify_all: matches!(order, "breadth" | "spread"),
        sorter_checks: 0,
    };
    let c = &model.request.phase.championship;
    let hp = [c.point_loss, c.point_draw, c.point_win];
    let ap = [c.point_win, c.point_draw, c.point_loss];
    let breadth = order == "breadth" || order == "spread";
    let start = std::time::Instant::now();
    for seed in seeds {
        if check.attempts >= budget || check.unresolved.is_empty() {
            break;
        }
        let limit = (check.attempts + per_seed).min(budget);
        for (t, c) in model.base.iter().enumerate() {
            check.points[t] = c.points;
            check.wins[t] = c.wins;
        }
        let mut valid = true;
        for (i, f) in model.fixtures.iter().enumerate() {
            let o = seed[i] as usize;
            if o > 2 || f.prob[o] <= 0. {
                valid = false;
                break;
            }
            check.points[f.home] += hp[o];
            check.points[f.away] += ap[o];
            check.wins[f.home] += i32::from(o == 2);
            check.wins[f.away] += i32::from(o == 0);
        }
        if !valid {
            continue;
        }
        check.candidate = seed.clone();
        check.ties = 0;
        check.check();
        if breadth {
            // Spend each seed's existing quota across every single-fixture
            // mutation before enumerating pairs. No extra candidates.
            for i in 0..model.fixtures.len() {
                let old = check.candidate[i];
                for next in 0..3 {
                    if check.attempts >= limit || check.unresolved.is_empty() {
                        break;
                    }
                    if next == old || model.fixtures[i].prob[next as usize] <= 0. {
                        continue;
                    }
                    check.apply(i, next);
                    check.check();
                    check.apply(i, old);
                }
            }
        }
        if order == "spread" {
            // Visit pairs across the fixture list rather than exhausting all
            // pairs involving its first fixture. Keep the same candidate quota.
            let m = model.fixtures.len();
            'pairs: for distance in 1..m {
                for i in 0..m - distance {
                    if check.attempts >= limit || check.unresolved.is_empty() {
                        break 'pairs;
                    }
                    let j = i + distance;
                    let old = check.candidate[i];
                    let other_old = check.candidate[j];
                    for next in 0..3 {
                        if next == old || model.fixtures[i].prob[next as usize] <= 0. {
                            continue;
                        }
                        check.apply(i, next);
                        for other_next in 0..3 {
                            if check.attempts >= limit || check.unresolved.is_empty() {
                                break;
                            }
                            if other_next == other_old
                                || model.fixtures[j].prob[other_next as usize] <= 0.
                            {
                                continue;
                            }
                            check.apply(j, other_next);
                            check.check();
                            check.apply(j, other_old);
                        }
                        check.apply(i, old);
                    }
                }
            }
            continue;
        }
        for i in 0..model.fixtures.len() {
            if check.attempts >= limit || check.unresolved.is_empty() {
                break;
            }
            let old = check.candidate[i];
            for next in 0..if breadth { 0 } else { 3 } {
                if check.attempts >= limit {
                    break;
                }
                if next == old || model.fixtures[i].prob[next as usize] <= 0. {
                    continue;
                }
                check.apply(i, next);
                check.check();
                if check.unresolved.is_empty() {
                    return check.proofs;
                }
                check.apply(i, old);
            }
            for j in i + 1..model.fixtures.len() {
                if check.attempts >= limit || check.unresolved.is_empty() {
                    break;
                }
                let other_old = check.candidate[j];
                for next in 0..3 {
                    if next == old || model.fixtures[i].prob[next as usize] <= 0. {
                        continue;
                    }
                    check.apply(i, next);
                    for other_next in 0..3 {
                        if other_next == other_old
                            || model.fixtures[j].prob[other_next as usize] <= 0.
                        {
                            continue;
                        }
                        check.apply(j, other_next);
                        check.check();
                        check.apply(j, other_old);
                        if check.attempts >= limit || check.unresolved.is_empty() {
                            break;
                        }
                    }
                    check.apply(i, old);
                    if check.attempts >= limit || check.unresolved.is_empty() {
                        break;
                    }
                }
            }
        }
    }
    if std::env::var("RUST_ODDS_REACHABILITY_TRACE").as_deref() == Ok("1") {
        eprintln!(
            "{}",
            serde_json::json!({"event":"rust_reachability_neighborhood","group":model.request.id,
            "order":order,"breadth":breadth,"attempts":check.attempts,"sorter_checks":check.sorter_checks,
            "tie_sorter_checks":check.total_ties,
            "found":check.proofs.len(),"budget":budget,"elapsed_ms":start.elapsed().as_secs_f64()*1000.})
        );
    }
    check.proofs
}
pub fn mark(model: &Model, proofs: &[Proof], estimates: &mut [Estimate]) {
    for p in proofs {
        estimates[p.cell.index(model.n)].reachability = "reachable_by_construction".into();
    }
}
pub fn walk(model: &Model, cells: &[Cell], assignments: &[Vec<u8>], estimates: &mut [Estimate]) {
    walk_passes(model, cells, assignments, estimates, 2);
}
pub fn walk_passes(
    model: &Model,
    cells: &[Cell],
    assignments: &[Vec<u8>],
    estimates: &mut [Estimate],
    passes: usize,
) {
    if !enabled("RARE_POSITION_NEIGHBORHOOD_WALK") {
        return;
    }
    let mut seeds = assignments.to_vec();
    for _ in 0..passes {
        if seeds.is_empty() {
            break;
        }
        let proofs = search(model, cells, &seeds, estimates, 4000);
        mark(model, &proofs, estimates);
        seeds = proofs.into_iter().map(|p| p.outcomes).collect();
    }
}
