//! Constructive completions of fixed W/D/L patterns. Independent fixtures
//! outside an earlier-key tie cohort give unbounded score adjustments. Failure
//! is unknown, never an impossibility proof. Scores exist only for verification.
use crate::{
    conditioned::canonical,
    model::{Campaign, Key, Model},
    rng::Rng,
    search::Cell,
};

#[derive(Clone, Debug)]
pub struct Adjustment {
    pub fixture: usize,
    pub margin: i32,
    pub equal_goals: i32,
}
pub struct Certificate {
    pub outcomes: Vec<u8>,
    pub adjustments: Vec<Adjustment>,
    pub ranks: Vec<usize>,
}
#[derive(Clone)]
struct State<'a> {
    model: &'a Model,
    c: Vec<Campaign>,
    scores: Vec<[i32; 2]>,
    adjustments: Vec<Adjustment>,
}
impl<'a> State<'a> {
    fn new(model: &'a Model, outcomes: &[u8]) -> Option<Self> {
        if outcomes.len() != model.fixtures.len()
            || outcomes.iter().any(|o| *o > 2)
            || model.fixtures.iter().any(|f| f.home == f.away)
        {
            return None;
        }
        let mut s = Self {
            model,
            c: model.base.clone(),
            scores: model.empty_scores(),
            adjustments: Vec::new(),
        };
        for (i, f) in model.fixtures.iter().enumerate() {
            let score = canonical(outcomes[i]);
            s.scores[f.request_index] = score;
            model.add(&mut s.c, i, score);
        }
        Some(s)
    }
    fn adjust(&mut self, fixture: usize, margin: i32, equal: i32) -> Option<()> {
        if margin < 0 || equal < 0 {
            return None;
        }
        let f = self.model.fixtures.get(fixture)?;
        let score = self.scores[f.request_index];
        let (hd, ad) = match score[0].cmp(&score[1]) {
            std::cmp::Ordering::Greater => (equal.checked_add(margin)?, equal),
            std::cmp::Ordering::Less => (equal, equal.checked_add(margin)?),
            std::cmp::Ordering::Equal if margin == 0 => (equal, equal),
            _ => return None,
        };
        let new = [score[0].checked_add(hd)?, score[1].checked_add(ad)?];
        let mut h = self.c[f.home];
        let mut a = self.c[f.away];
        h.gf = h.gf.checked_add(hd)?;
        h.ga = h.ga.checked_add(ad)?;
        a.gf = a.gf.checked_add(ad)?;
        a.ga = a.ga.checked_add(hd)?;
        a.away = a.away.checked_add(ad)?;
        self.c[f.home] = h;
        self.c[f.away] = a;
        self.scores[f.request_index] = new;
        self.adjustments.push(Adjustment {
            fixture,
            margin,
            equal_goals: equal,
        });
        Some(())
    }
    fn value(&self, t: usize, key: Key) -> i64 {
        if key == Key::Gd {
            i64::from(self.c[t].gf) - i64::from(self.c[t].ga)
        } else {
            i64::from(self.c[t].gf)
        }
    }
    fn prefix(&self, t: usize, wins: bool, gd: bool) -> (i32, i32, i64) {
        (
            self.c[t].points,
            if wins { self.c[t].wins } else { 0 },
            if gd { self.value(t, Key::Gd) } else { 0 },
        )
    }
    #[cold]
    #[inline(never)]
    fn verified(&self, outcomes: &[u8], cell: Cell) -> Option<Certificate> {
        let mut order = vec![0; self.model.n];
        self.model
            .standings(&mut order, &self.c, &self.scores, &mut Rng::new(1));
        if order.get(cell.rank) != Some(&cell.team) {
            return None;
        }
        let mut ranks = vec![0; self.model.n];
        for (r, t) in order.into_iter().enumerate() {
            ranks[t] = r;
        }
        Some(Certificate {
            outcomes: outcomes.to_vec(),
            adjustments: self.adjustments.clone(),
            ranks,
        })
    }
}
#[derive(Clone, Copy)]
struct Interval {
    base: i64,
    lo: i64,
    hi: i64,
    up: Option<usize>,
    down: Option<usize>,
}
fn intervals(
    s: &State<'_>,
    cohort: &[usize],
    key: Key,
    paths: bool,
) -> (Vec<Interval>, Vec<Vec<usize>>) {
    let mut inside = vec![false; s.c.len()];
    for &t in cohort {
        inside[t] = true;
    }
    let mut values: Vec<_> = cohort
        .iter()
        .map(|&t| {
            let base = s.value(t, key);
            Interval {
                base,
                lo: base,
                hi: base,
                up: None,
                down: None,
            }
        })
        .collect();
    let mut slots = vec![usize::MAX; s.c.len()];
    for (i, &t) in cohort.iter().enumerate() {
        slots[t] = i;
    }
    for (i, f) in s.model.fixtures.iter().enumerate() {
        // Both endpoints in the cohort require coupled reasoning, not this cut.
        if inside[f.home] == inside[f.away] {
            continue;
        }
        let (t, own, other) = if inside[f.home] {
            (f.home, 0, 1)
        } else {
            (f.away, 1, 0)
        };
        let v = &mut values[slots[t]];
        let score = s.scores[f.request_index];
        if key == Key::Gf || score[own] > score[other] {
            v.up.get_or_insert(i);
            v.hi = i64::MAX / 4;
        }
        if key == Key::Gd && score[own] < score[other] {
            v.down.get_or_insert(i);
            v.lo = i64::MIN / 4;
        }
    }
    let mut routes = Vec::new();
    if paths && key == Key::Gd && inside.iter().any(|v| !*v) {
        let mut forward = vec![Vec::new(); s.c.len()];
        let mut reverse = forward.clone();
        for (i, f) in s.model.fixtures.iter().enumerate() {
            let score = s.scores[f.request_index];
            let (winner, loser) = match score[0].cmp(&score[1]) {
                std::cmp::Ordering::Greater => (f.home, f.away),
                std::cmp::Ordering::Less => (f.away, f.home),
                _ => continue,
            };
            forward[winner].push((loser, i));
            reverse[loser].push((winner, i));
        }
        for (&t, v) in cohort.iter().zip(&mut values) {
            if v.up.is_none() {
                if let Some(route) = outside_path(&forward, &inside, t) {
                    v.up = Some(s.model.fixtures.len() + routes.len());
                    v.hi = i64::MAX / 4;
                    routes.push(route);
                }
            }
            if v.down.is_none() {
                if let Some(route) = outside_path(&reverse, &inside, t) {
                    v.down = Some(s.model.fixtures.len() + routes.len());
                    v.lo = i64::MIN / 4;
                    routes.push(route);
                }
            }
        }
    }
    (values, routes)
}
// A simple directed path changes GD only at its endpoints. Its interior can
// belong to the cohort: increased goals scored AND conceded cancel their GD.
// Later goals-scored ordering must still be verified after applying all paths.
fn outside_path(
    edges: &[Vec<(usize, usize)>],
    inside: &[bool],
    start: usize,
) -> Option<Vec<usize>> {
    let mut previous = vec![None; edges.len()];
    let mut queue = vec![start];
    previous[start] = Some((start, usize::MAX));
    let mut read = 0;
    while read < queue.len() {
        let at = queue[read];
        read += 1;
        for &(next, fixture) in &edges[at] {
            if previous[next].is_some() {
                continue;
            }
            previous[next] = Some((at, fixture));
            if !inside[next] {
                let mut route = Vec::new();
                let mut cursor = next;
                while cursor != start {
                    let (parent, game) = previous[cursor]?;
                    route.push(game);
                    cursor = parent;
                }
                route.reverse();
                return Some(route);
            }
            queue.push(next);
        }
    }
    None
}
fn set_value(
    s: &mut State<'_>,
    v: Interval,
    value: i64,
    key: Key,
    routes: &[Vec<usize>],
) -> Option<()> {
    let delta = value - v.base;
    if delta == 0 {
        return Some(());
    }
    let amount = i32::try_from(delta.abs()).ok()?;
    let fixture = if delta > 0 { v.up? } else { v.down? };
    if key == Key::Gd {
        if fixture < s.model.fixtures.len() {
            s.adjust(fixture, amount, 0)
        } else {
            for &game in routes.get(fixture - s.model.fixtures.len())? {
                s.adjust(game, amount, 0)?;
            }
            Some(())
        }
    } else {
        s.adjust(fixture, 0, amount)
    }
}
// Choose strict orderings where possible; fixed equal values proceed to the
// next key / sorter. A feasible interval layout is not itself a certificate.
fn layout(values: &[Interval], target: usize, x: i64, ahead: usize) -> Option<Vec<i64>> {
    let mut assigned: Vec<_> = values.iter().map(|v| v.base).collect();
    assigned[target] = x;
    let mut forced = 0;
    let mut equal = 0;
    let mut flexible = Vec::new();
    for (i, v) in values.iter().enumerate() {
        if i == target {
            continue;
        }
        match (v.hi > x, v.lo < x) {
            (true, false) => {
                assigned[i] = v.base.max(x + 1).min(v.hi);
                forced += 1;
            }
            (false, true) => assigned[i] = v.base.min(x - 1).max(v.lo),
            (true, true) => flexible.push(i),
            (false, false) => {
                assigned[i] = x;
                equal += 1;
            }
        }
    }
    if ahead < forced || ahead > forced + flexible.len() + equal {
        return None;
    }
    flexible.sort_by_key(|i| (values[*i].base <= x, *i));
    let above = (ahead - forced).min(flexible.len());
    for (k, i) in flexible.into_iter().enumerate() {
        let v = values[i];
        assigned[i] = if k < above {
            v.base.max(x + 1).min(v.hi)
        } else {
            v.base.min(x - 1).max(v.lo)
        };
    }
    Some(assigned)
}

fn stage(
    s: &State<'_>,
    outcomes: &[u8],
    cell: Cell,
    key: Key,
    wins: bool,
    gf_next: bool,
    trials: &mut usize,
    paths: bool,
) -> Option<Certificate> {
    let prefix = s.prefix(cell.team, wins, key == Key::Gf && gf_next);
    let ahead = (0..s.model.n)
        .filter(|&t| s.prefix(t, wins, key == Key::Gf && gf_next) > prefix)
        .count();
    let cohort: Vec<_> = (0..s.model.n)
        .filter(|&t| s.prefix(t, wins, key == Key::Gf && gf_next) == prefix)
        .collect();
    let requested = cell.rank.checked_sub(ahead)?;
    if requested >= cohort.len() {
        return None;
    }
    let target = cohort.iter().position(|t| *t == cell.team)?;
    let (values, routes) = intervals(s, &cohort, key, paths);
    let v = values[target];
    let mut candidates = vec![v.base];
    for peer in &values {
        candidates.extend([peer.base - 1, peer.base, peer.base + 1]);
    }
    candidates.retain(|x| *x >= v.lo && *x <= v.hi);
    candidates.sort_by_key(|x| ((*x - v.base).abs(), *x));
    candidates.dedup();
    for x in candidates {
        let Some(assigned) = layout(&values, target, x, requested) else {
            continue;
        };
        // A bounded algebraic completion, not another fixture search budget.
        if *trials >= 16 {
            return None;
        }
        *trials += 1;
        let mut trial = s.clone();
        let mut legal = true;
        for (&v, &value) in values.iter().zip(&assigned) {
            if set_value(&mut trial, v, value, key, &routes).is_none() {
                legal = false;
                break;
            }
        }
        if !legal {
            continue;
        }
        if let Some(proof) = trial.verified(outcomes, cell) {
            return Some(proof);
        }
        if key == Key::Gd && gf_next {
            if let Some(proof) = stage(&trial, outcomes, cell, Key::Gf, wins, true, trials, paths) {
                return Some(proof);
            }
        }
    }
    None
}

/// Independent outside-cohort goal adjustments, followed by actual sorting.
/// Only supported score-key prefixes are tried. None is always "unknown".
#[cold]
#[inline(never)]
pub fn complete(model: &Model, outcomes: &[u8], cell: Cell) -> Option<Certificate> {
    complete_with_paths(
        model,
        outcomes,
        cell,
        std::env::var("RUST_ODDS_GOAL_PATHS").as_deref() == Ok("1"),
    )
}
/// Extend independent GD adjustments through decisive paths, within the same
/// sixteen layout attempts. Paths are witnesses, never impossibility proofs.
pub fn complete_with_paths(
    model: &Model,
    outcomes: &[u8],
    cell: Cell,
    paths: bool,
) -> Option<Certificate> {
    if cell.team >= model.n || cell.rank >= model.n || model.request.phase.bonus_points != 0 {
        return None;
    }
    let wins = model.keys.get(1) == Some(&Key::W);
    let offset = 1 + usize::from(wins);
    if model.keys.first() != Some(&Key::Pt) {
        return None;
    }
    let key = *model.keys.get(offset)?;
    if !matches!(key, Key::Gd | Key::Gf) {
        return None;
    }
    let gf_next = key == Key::Gd && model.keys.get(offset + 1) == Some(&Key::Gf);
    let state = State::new(model, outcomes)?;
    stage(&state, outcomes, cell, key, wins, gf_next, &mut 0, paths)
}

/// Reconstruct compact adjustments and verify their complete season on demand.
pub fn verify(model: &Model, proof: &Certificate, cell: Cell) -> bool {
    let Some(mut s) = State::new(model, &proof.outcomes) else {
        return false;
    };
    for a in &proof.adjustments {
        if s.adjust(a.fixture, a.margin, a.equal_goals).is_none() {
            return false;
        }
    }
    s.verified(&proof.outcomes, cell)
        .is_some_and(|actual| actual.ranks == proof.ranks)
}
