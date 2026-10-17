//! Joint fixture-level importance proposals for rare packed-table events.
//!
//! A plan is deliberately immutable after construction.  In particular, an
//! edge which was unresolved during construction always enters its native
//! masked suffix, even if another path later caused a nearby state to be
//! memoized.  This is what makes the cap describe the proposal actually used
//! by `draw`.

use crate::law::{categorical, categorical_probabilities, Interval, Law};
use golaberto_odds::{model::Model, rng::Rng};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::Instant,
};

#[path = "joint/residual.rs"]
mod residual;
pub use residual::{residual_certificate, residual_certificate_with_options, ResidualCertificate, ResidualSource, SuffixDiagnostics, SuffixOptions};

#[derive(Clone, Debug)]
struct Edge {
    outcome: u8,
    child: Option<usize>, // None is a sticky native-mask suffix fallback.
}

#[derive(Clone, Debug)]
struct Node {
    weights: [f64; 3],
    edges: Vec<Edge>,
    potential: f64,
    cap: f64,
    terminal: Option<bool>,
}

/// An immutable proposal for the selected packed-table constraints.
pub struct Plan {
    pub bound: f64,
    pub states: usize,
    pub complete: bool,
    pub selected: Vec<usize>,
    masks: Vec<u8>,
    order: Vec<usize>,
    nodes: Vec<Node>,
    root: Option<usize>,
    stride: i32,
    lower: Vec<i32>,
    upper: Vec<i32>,
    base: Vec<i32>,
    clamp_lower: Vec<bool>,
    fallback_caps: Vec<f64>,       // suffix cap indexed by order position
    fallback_potentials: Vec<f64>, // nominal native-mask suffix mass
    guide: OnceLock<HybridGuide>,
}

/// Sparse endpoint gain rows are built only when a hybrid draw is requested.
/// Forest roots share this cache through their common core Plan.
#[derive(Default)]
struct HybridGuide {
    rows: Mutex<HashMap<(usize, usize), Option<Arc<Vec<(i32, f64)>>>>>,
    rows_built: AtomicUsize,
    states_built: AtomicUsize,
}

impl HybridGuide {
    fn publish(
        &self,
        key: (usize, usize),
        row: Option<Arc<Vec<(i32, f64)>>>,
    ) -> Result<Option<Arc<Vec<(i32, f64)>>>, String> {
        let mut rows = self
            .rows
            .lock()
            .map_err(|_| "hybrid guide cache poisoned")?;
        if let Some(existing) = rows.get(&key) {
            return Ok(existing.clone());
        }
        self.rows_built.fetch_add(1, Ordering::Relaxed);
        self.states_built
            .fetch_add(row.as_ref().map_or(0, |r| r.len()), Ordering::Relaxed);
        rows.insert(key, row.clone());
        Ok(row)
    }

    fn stats(&self) -> (usize, usize) {
        (
            self.rows_built.load(Ordering::Relaxed),
            self.states_built.load(Ordering::Relaxed),
        )
    }
}

/// Importance weights returned by a native guided draw.
#[derive(Clone, Debug)]
pub struct HybridDraw {
    pub outcomes: Vec<u8>,
    pub baseline_weight: Interval,
    pub guide_over_baseline: Interval,
}

/// Metadata for one fixed light-prefix assignment in a shared suffix forest.
pub struct ForestRoot {
    /// Native probability of the fixed prefix under the unconditional WDL law.
    pub prefix_mass: Interval,
    /// Native masked suffix event mass from this prefix state.
    pub potential: f64,
    /// Enclosed maximum suffix likelihood ratio for this root's proposal.
    pub cap: f64,
    /// Whether this root was fully represented by the memoized suffix table.
    pub complete: bool,
    /// True only when the event was certified impossible for this prefix.
    pub zero: bool,
    prefix_outcomes: Vec<u8>,
    node: Option<usize>,
}

/// A collection of fixed prefixes sharing one immutable suffix DP and fallback.
pub struct Forest {
    pub states: usize,
    pub complete: bool,
    pub roots: Vec<ForestRoot>,
    core: Plan,
    prefix_fixtures: Vec<usize>,
}

fn up(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        x
    } else {
        f64::from_bits(x.to_bits() + 1)
    }
}
fn ratio(p: Interval, q: Interval) -> Result<f64, String> {
    if q.lo <= 0.0 {
        return Err("proposal categorical has no positive lower probability".into());
    }
    if p.hi == 0.0 {
        return Ok(0.0);
    }
    let raw = p.div(q).hi;
    let r = if raw == 0.0 { f64::from_bits(1) } else { raw };
    if r.is_finite() {
        Ok(r)
    } else {
        Err("non-finite likelihood cap".into())
    }
}
fn mul_cap(a: f64, b: f64) -> f64 {
    if a == 0.0 || b == 0.0 {
        0.0
    } else {
        let raw = a * b;
        if raw == 0.0 {
            f64::from_bits(1)
        } else {
            up(raw)
        }
    }
}
fn outcome_gain(model: &Model, _fixture: usize, outcome: usize, stride: i32) -> (i32, i32) {
    // outcome: 0 away win, 1 draw, 2 home win.
    let points = model.request.phase.championship.point_win;
    let draw = model.request.phase.championship.point_draw;
    let loss = model.request.phase.championship.point_loss;
    let (hp, ap, hw, aw) = match outcome {
        0 => (loss, points, false, true),
        1 => (draw, draw, false, false),
        _ => (points, loss, true, false),
    };
    (hp * stride + i32::from(hw), ap * stride + i32::from(aw))
}

impl Plan {
    fn suffix_gain_row(
        &self,
        model: &Model,
        laws: &[Law],
        team: usize,
        from: usize,
    ) -> Result<Option<Arc<Vec<(i32, f64)>>>, String> {
        let cache = self.guide.get_or_init(HybridGuide::default);
        if let Some(row) = cache
            .rows
            .lock()
            .map_err(|_| "hybrid guide cache poisoned")?
            .get(&(team, from))
            .cloned()
        {
            return Ok(row);
        }
        // Ordered accumulation keeps guidance values independent of HashMap
        // iteration order when a suffix row is first built on different runs.
        let mut pmf = BTreeMap::from([(0i32, 1.0f64)]);
        for pos in from..self.order.len() {
            let fi = self.order[pos];
            let f = &model.fixtures[fi];
            if f.home != team && f.away != team {
                continue;
            }
            let mut w = [0.0; 3];
            for o in 0..3 {
                if self.masks[fi] & (1 << o) != 0 {
                    w[o] = laws[fi].outcomes[o].mid();
                }
            }
            if w.iter().all(|x| *x == 0.0) {
                for o in 0..3 {
                    if self.masks[fi] & (1 << o) != 0 && laws[fi].outcomes[o].hi > 0.0 {
                        w[o] = 1.0;
                    }
                }
            }
            let probs = match categorical_probabilities(&w) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let mut next = BTreeMap::<i32, f64>::new();
            for (&gain, &mass) in &pmf {
                for o in 0..3 {
                    let p = probs[o].mid();
                    if p <= 0.0 {
                        continue;
                    }
                    let (hg, ag) = outcome_gain(model, fi, o, self.stride);
                    let add = if f.home == team { hg } else { ag };
                    *next.entry(gain.saturating_add(add)).or_default() += mass * p;
                }
            }
            // Very large rows fall back to a neutral endpoint score. This is
            // only guidance; it never changes the native likelihood bound.
            if next.len() > 250_000 {
                return cache.publish((team, from), None);
            }
            pmf = next;
        }
        let row: Vec<_> = pmf.into_iter().collect();
        let row = Some(Arc::new(row));
        cache.publish((team, from), row)
    }

    /// Number of cached suffix rows and total sparse PMF states they contain.
    pub fn hybrid_guide_cache_stats(&self) -> (usize, usize) {
        self.guide.get_or_init(HybridGuide::default).stats()
    }

    fn endpoint_chance(
        &self,
        model: &Model,
        laws: &[Law],
        team: usize,
        from: usize,
        score: i32,
    ) -> Result<f64, String> {
        let Some(row) = self.suffix_gain_row(model, laws, team, from)? else {
            return Ok(1.0);
        };
        let lo = self.lower[team].saturating_sub(score);
        let hi = self.upper[team].saturating_sub(score);
        if lo > hi {
            return Ok(0.0);
        }
        Ok(row
            .iter()
            .filter(|(g, _)| *g >= lo && *g <= hi)
            .map(|(_, p)| *p)
            .sum::<f64>()
            .clamp(0.0, 1.0))
    }

    fn guided_weights(
        &self,
        model: &Model,
        laws: &[Law],
        fi: usize,
        pos: usize,
        totals: &[i32],
        base: &[f64],
    ) -> Result<Vec<f64>, String> {
        let f = &model.fixtures[fi];
        let mut guide = [0.0; 3];
        for o in 0..3 {
            if base[o] <= 0.0 {
                continue;
            }
            let (hg, ag) = outcome_gain(model, fi, o, self.stride);
            let mut h = 1.0;
            if !self.selected.contains(&f.home) {
                h *= self.endpoint_chance(
                    model,
                    laws,
                    f.home,
                    pos + 1,
                    totals[f.home].saturating_add(hg),
                )?;
            }
            if !self.selected.contains(&f.away) {
                h *= self.endpoint_chance(
                    model,
                    laws,
                    f.away,
                    pos + 1,
                    totals[f.away].saturating_add(ag),
                )?;
            }
            guide[o] = h;
        }
        let bp = categorical_probabilities(base)?;
        let guided_mass = (0..3).map(|o| bp[o].mid() * guide[o]).sum::<f64>();
        if guided_mass <= 0.0 {
            return Ok(base.to_vec());
        }
        Ok((0..3)
            .map(|o| 0.05 * bp[o].mid() + 0.95 * bp[o].mid() * guide[o] / guided_mass)
            .collect())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build(
        model: &Model,
        laws: &[Law],
        masks: &[u8],
        lower: &[i32],
        upper: &[i32],
        selected: &[usize],
        stride: i32,
        state_cap: usize,
        deadline: Instant,
    ) -> Result<Self, String> {
        let nf = model.fixtures.len();
        if laws.len() != nf || masks.len() != nf {
            return Err("joint plan fixture dimensions do not match".into());
        }
        if lower.len() != model.n || upper.len() != model.n {
            return Err("joint plan team dimensions do not match".into());
        }
        if stride <= 0 {
            return Err("joint plan stride must be positive".into());
        }
        let mut seen = vec![false; model.n];
        for &t in selected {
            if t >= model.n || seen[t] {
                return Err("selected team index is invalid or repeated".into());
            }
            seen[t] = true;
        }
        if masks.iter().any(|m| m & !7 != 0) {
            return Err("joint plan mask has invalid outcome bits".into());
        }
        let selected = selected.to_vec();
        let packed = |t: usize| {
            model.base[t]
                .points
                .checked_mul(stride)
                .and_then(|x| x.checked_add(model.base[t].wins))
                .ok_or("packed base overflow")
        };
        let base = (0..model.n).map(packed).collect::<Result<Vec<_>, _>>()?;
        let mut max_final = base.clone();
        for (fi, f) in model.fixtures.iter().enumerate() {
            for t in [f.home, f.away] {
                let mut max_gain = 0i32;
                for o in 0..3 {
                    if masks[fi] & (1 << o) == 0 || laws[fi].outcomes[o].hi <= 0.0 {
                        continue;
                    }
                    let (hg, ag) = outcome_gain(model, fi, o, stride);
                    max_gain = max_gain.max(if t == f.home { hg } else { ag });
                }
                max_final[t] = max_final[t].saturating_add(max_gain);
            }
        }
        // Cap state at a one-sided lower threshold only when the upper bound
        // is proven vacuous throughout the masked support. Exact root bounds
        // and any other finite two-sided interval retain their full state.
        let clamp_lower: Vec<bool> = selected
            .iter()
            .map(|&t| upper[t] > lower[t] && upper[t] >= max_final[t])
            .collect();

        // Fixed single-support fixtures expose their known packed gains first.
        // Variable fixtures are then eliminated team by team using a
        // deterministic min-degree/min-fill order on the selected-team graph.
        let mut fixed = Vec::new();
        let mut variable = Vec::new();
        for fi in 0..nf {
            let support = (0..3)
                .filter(|&o| masks[fi] & (1 << o) != 0 && laws[fi].outcomes[o].hi > 0.0)
                .count();
            if support <= 1 {
                fixed.push(fi);
            } else {
                variable.push(fi);
            }
        }
        fixed.sort_unstable();
        variable.sort_unstable();
        let mut order = fixed;
        let mut remaining = vec![false; nf];
        for &fi in &variable {
            remaining[fi] = true;
        }
        let mut active = seen.clone();
        while active.iter().any(|&x| x) {
            if Instant::now() >= deadline {
                break;
            }
            let mut best: Option<(usize, usize, usize, i32, usize)> = None;
            for t in 0..model.n {
                if !active[t] {
                    continue;
                }
                let mut neighbors = vec![false; model.n];
                let mut remaining_games = 0usize;
                for &fi in &variable {
                    if !remaining[fi] {
                        continue;
                    }
                    let f = &model.fixtures[fi];
                    if f.home == t || f.away == t {
                        remaining_games += 1;
                        let other = if f.home == t { f.away } else { f.home };
                        if active[other] && other != t {
                            neighbors[other] = true;
                        }
                    }
                }
                let degree = neighbors.iter().filter(|&&x| x).count();
                let mut fill = 0usize;
                for a in 0..model.n {
                    if !neighbors[a] {
                        continue;
                    }
                    for b in a + 1..model.n {
                        if !neighbors[b] {
                            continue;
                        }
                        let adjacent = variable.iter().any(|&fi| {
                            remaining[fi]
                                && ((model.fixtures[fi].home == a && model.fixtures[fi].away == b)
                                    || (model.fixtures[fi].home == b
                                        && model.fixtures[fi].away == a))
                        });
                        if !adjacent {
                            fill += 1;
                        }
                    }
                }
                let key = (degree, fill, remaining_games, model.ids[t], t);
                if best.map_or(true, |current| key < current) {
                    best = Some(key);
                }
            }
            let team = best.ok_or("selected fixture elimination failed")?.4;
            for &fi in &variable {
                let f = &model.fixtures[fi];
                if remaining[fi] && (f.home == team || f.away == team) {
                    order.push(fi);
                    remaining[fi] = false;
                }
            }
            active[team] = false;
        }
        for fi in variable {
            if remaining[fi] {
                order.push(fi);
            }
        }
        if order.len() != nf {
            return Err("joint fixture ordering omitted a fixture".into());
        }

        // The native masked suffix has a fixed proposal and therefore a fixed
        // product cap, including all fixtures after a fallback edge.
        let mut fallback_caps = vec![1.0; nf + 1];
        let mut fallback_potentials = vec![1.0; nf + 1];
        for pos in (0..nf).rev() {
            let fi = order[pos];
            let w: Vec<f64> = (0..3)
                .map(|o| {
                    if masks[fi] & (1 << o) != 0 {
                        laws[fi].outcomes[o].mid()
                    } else {
                        0.0
                    }
                })
                .collect();
            let suffix_mass = w.iter().sum::<f64>().min(1.0);
            let raw_potential = suffix_mass * fallback_potentials[pos + 1];
            fallback_potentials[pos] = if raw_potential == 0.0 && suffix_mass > 0.0 {
                f64::from_bits(1)
            } else {
                raw_potential
            };
            let probs = if w.iter().any(|x| *x > 0.0) {
                categorical_probabilities(&w)?
            } else {
                fallback_caps[pos] = 0.0;
                continue;
            };
            let mut step = 0.0f64;
            for o in 0..3 {
                if w[o] == 0.0 {
                    continue;
                }
                step = step.max(ratio(laws[fi].outcomes[o], probs[o])?);
            }
            fallback_caps[pos] = mul_cap(step, fallback_caps[pos + 1]);
        }

        let mut plan = Self {
            bound: 1.0,
            states: 0,
            complete: true,
            selected,
            masks: masks.to_vec(),
            order,
            nodes: Vec::new(),
            root: None,
            stride,
            lower: lower.to_vec(),
            upper: upper.to_vec(),
            base,
            clamp_lower,
            fallback_caps,
            fallback_potentials,
            guide: OnceLock::new(),
        };

        // Empty selection intentionally means the native masked proposal.
        if plan.selected.is_empty() {
            plan.bound = plan.fallback_caps[0];
            return Ok(plan);
        }

        let mut memo: HashMap<(usize, Vec<i32>), usize> = HashMap::new();
        let mut limit_hit = false;
        let mut start_state: Vec<i32> = plan
            .selected
            .iter()
            .enumerate()
            .map(|(j, &t)| {
                if plan.clamp_lower[j] {
                    plan.base[t].min(plan.lower[t])
                } else {
                    plan.base[t]
                }
            })
            .collect();
        let mut last_touch = vec![None; plan.selected.len()];
        for (pos, &fi) in plan.order.iter().enumerate() {
            let f = &model.fixtures[fi];
            for (j, &t) in plan.selected.iter().enumerate() {
                if t == f.home || t == f.away {
                    last_touch[j] = Some(pos);
                }
            }
        }
        for (j, &t) in plan.selected.iter().enumerate() {
            if last_touch[j].is_none() {
                if plan.base[t] < plan.lower[t] || plan.base[t] > plan.upper[t] {
                    plan.bound = 0.0;
                    return Ok(plan);
                }
                start_state[j] = 0;
            }
        }
        // Cache independent integer suffix extrema. Recomputing these for
        // every node would multiply a large DP by the fixture count twice.
        let mut suffix_min = vec![vec![0i32; plan.selected.len()]; nf + 1];
        let mut suffix_max = vec![vec![0i32; plan.selected.len()]; nf + 1];
        let mut suffix_possible = vec![true; nf + 1];
        for pos in (0..nf).rev() {
            suffix_min[pos] = suffix_min[pos + 1].clone();
            suffix_max[pos] = suffix_max[pos + 1].clone();
            suffix_possible[pos] = suffix_possible[pos + 1];
            let fi = plan.order[pos];
            let f = &model.fixtures[fi];
            for (j, &t) in plan.selected.iter().enumerate() {
                if t != f.home && t != f.away {
                    continue;
                }
                let mut lo = i32::MAX;
                let mut hi = i32::MIN;
                for o in 0..3 {
                    if plan.masks[fi] & (1 << o) == 0 || laws[fi].outcomes[o].hi <= 0.0 {
                        continue;
                    }
                    let (hg, ag) = outcome_gain(model, fi, o, plan.stride);
                    let gain = if t == f.home { hg } else { ag };
                    lo = lo.min(gain);
                    hi = hi.max(gain);
                }
                if lo == i32::MAX {
                    suffix_possible[pos] = false;
                } else {
                    suffix_min[pos][j] = suffix_min[pos][j].saturating_add(lo);
                    suffix_max[pos][j] = suffix_max[pos][j].saturating_add(hi);
                }
            }
        }
        fn build_node(
            plan: &mut Plan,
            model: &Model,
            laws: &[Law],
            state: Vec<i32>,
            pos: usize,
            memo: &mut HashMap<(usize, Vec<i32>), usize>,
            cap: usize,
            deadline: Instant,
            limit_hit: &mut bool,
            suffix_min: &[Vec<i32>],
            suffix_max: &[Vec<i32>],
            suffix_possible: &[bool],
            last_touch: &[Option<usize>],
        ) -> Result<Option<usize>, String> {
            if let Some(&id) = memo.get(&(pos, state.clone())) {
                return Ok(Some(id));
            }
            if *limit_hit || plan.states >= cap || Instant::now() >= deadline {
                *limit_hit = true;
                plan.complete = false;
                return Ok(None);
            }
            plan.states += 1;
            if pos == plan.order.len() {
                let accept = plan.selected.iter().enumerate().all(|(j, &t)| {
                    last_touch[j].is_none()
                        || state[j] == 0
                        || (state[j] >= plan.lower[t] && state[j] <= plan.upper[t])
                });
                let id = plan.nodes.len();
                plan.nodes.push(Node {
                    weights: [0.0; 3],
                    edges: Vec::new(),
                    potential: if accept { 1.0 } else { 0.0 },
                    cap: if accept { 1.0 } else { 0.0 },
                    terminal: Some(accept),
                });
                memo.insert((pos, state), id);
                return Ok(Some(id));
            }

            let fi = plan.order[pos];
            let f = &model.fixtures[fi];
            let mut weights = [0.0; 3];
            let mut edges = Vec::new();
            let mut potential = 0.0;
            for o in 0..3 {
                if plan.masks[fi] & (1 << o) == 0 {
                    continue;
                }
                let p = laws[fi].outcomes[o];
                if p.hi <= 0.0 {
                    continue;
                }
                let (hg, ag) = outcome_gain(model, fi, o, plan.stride);
                let mut next = state.clone();
                for (j, &t) in plan.selected.iter().enumerate() {
                    if t == f.home {
                        next[j] = next[j].checked_add(hg).ok_or("packed state overflow")?;
                        if plan.clamp_lower[j] {
                            next[j] = next[j].min(plan.lower[t]);
                        }
                    }
                    if t == f.away {
                        next[j] = next[j].checked_add(ag).ok_or("packed state overflow")?;
                        if plan.clamp_lower[j] {
                            next[j] = next[j].min(plan.lower[t]);
                        }
                    }
                }
                // Safe selected-only integer suffix range test. It may remove
                // only states that cannot reach the requested packed interval.
                let mut feasible = true;
                for (j, &t) in plan.selected.iter().enumerate() {
                    if last_touch[j] == Some(pos) {
                        if next[j] < plan.lower[t] || next[j] > plan.upper[t] {
                            feasible = false;
                            break;
                        }
                        next[j] = 0; // constraint closed; forget its packed total
                    }
                }
                if !suffix_possible[pos + 1] {
                    feasible = false;
                }
                for (j, &t) in plan.selected.iter().enumerate() {
                    if !last_touch[j].is_some_and(|last| last > pos) {
                        continue;
                    }
                    if !feasible
                        || next[j].saturating_add(suffix_max[pos + 1][j]) < plan.lower[t]
                        || next[j].saturating_add(suffix_min[pos + 1][j]) > plan.upper[t]
                    {
                        feasible = false;
                        break;
                    }
                }
                if !feasible {
                    continue;
                }
                let child = build_node(
                    plan,
                    model,
                    laws,
                    next,
                    pos + 1,
                    memo,
                    cap,
                    deadline,
                    limit_hit,
                    suffix_min,
                    suffix_max,
                    suffix_possible,
                    last_touch,
                )?;
                let child_potential = child.map_or(plan.fallback_potentials[pos + 1], |id| {
                    plan.nodes[id].potential
                });
                // `None` is an unresolved edge, not an infeasible edge. Its
                // nominal potential is the native masked suffix mass, matching
                // the fixed fallback sampler used after this edge.
                let sticky_fallback = feasible && child.is_none();
                let effective_potential = child_potential;
                let raw_weight = p.mid() * effective_potential;
                let w = if raw_weight == 0.0 && p.mid() > 0.0 && effective_potential > 0.0 {
                    f64::from_bits(1)
                } else {
                    raw_weight
                };
                if w <= 0.0 {
                    continue;
                }
                weights[o] = w;
                edges.push(Edge {
                    outcome: o as u8,
                    child: if sticky_fallback { None } else { child },
                });
            }
            if !weights.iter().any(|x| *x > 0.0) {
                let id = plan.nodes.len();
                plan.nodes.push(Node {
                    weights,
                    edges,
                    potential: 0.0,
                    cap: 0.0,
                    terminal: None,
                });
                memo.insert((pos, state), id);
                return Ok(Some(id));
            }
            let q = categorical_probabilities(&weights)?;
            let mut qbound = 0.0f64;
            for edge in &edges {
                let o = edge.outcome as usize;
                let child_cap = match edge.child {
                    Some(id) => plan.nodes[id].cap,
                    None => plan.fallback_caps[pos + 1],
                };
                qbound = qbound.max(mul_cap(ratio(laws[fi].outcomes[o], q[o])?, child_cap));
                let child_potential = match edge.child {
                    Some(id) => plan.nodes[id].potential,
                    None => plan.fallback_potentials[pos + 1],
                };
                let term = laws[fi].outcomes[o].mid() * child_potential;
                let term =
                    if term == 0.0 && laws[fi].outcomes[o].mid() > 0.0 && child_potential > 0.0 {
                        f64::from_bits(1)
                    } else {
                        term
                    };
                potential = up(potential + term);
            }
            let id = plan.nodes.len();
            plan.nodes.push(Node {
                weights,
                edges,
                potential: potential.min(1.0),
                cap: qbound,
                terminal: None,
            });
            memo.insert((pos, state), id);
            Ok(Some(id))
        }

        plan.root = build_node(
            &mut plan,
            model,
            laws,
            start_state,
            0,
            &mut memo,
            state_cap,
            deadline,
            &mut limit_hit,
            &suffix_min,
            &suffix_max,
            &suffix_possible,
            &last_touch,
        )?;
        if let Some(root) = plan.root {
            plan.bound = plan.nodes[root].cap;
        } else {
            plan.bound = plan.fallback_caps[0];
            plan.complete = false;
        }
        if !plan.bound.is_finite() {
            return Err("joint plan produced a non-finite cap".into());
        }
        Ok(plan)
    }

    /// Draw one campaign's three-valued outcomes and its enclosed full-law / proposal ratio.
    pub fn draw(&self, laws: &[Law], rng: &mut Rng) -> Result<(Vec<u8>, Interval), String> {
        if laws.len() != self.masks.len() {
            return Err("draw law dimensions do not match plan".into());
        }
        let mut outcomes = vec![0u8; laws.len()];
        let mut weight = Interval::point(1.0);
        if self.selected.is_empty() || self.root.is_none() {
            self.draw_fallback(0, laws, rng, &mut outcomes, &mut weight)?;
            if weight.lo > self.bound {
                return Err("draw likelihood lower bound exceeds plan cap".into());
            }
            return Ok((outcomes, weight));
        }
        let mut id = self.root;
        let mut pos = 0usize;
        while let Some(node_id) = id {
            let node = &self.nodes[node_id];
            if let Some(accept) = node.terminal {
                if !accept {
                    return Err("planner reached a zero-potential terminal".into());
                }
                break;
            }
            let (o, q) = categorical(&node.weights, rng)?;
            let edge = node
                .edges
                .iter()
                .find(|e| e.outcome as usize == o)
                .ok_or("joint plan edge missing")?;
            let fi = self.order[pos];
            outcomes[fi] = o as u8;
            weight = weight.mul(laws[fi].outcomes[o].div(q));
            pos += 1;
            id = edge.child;
            if id.is_none() {
                self.draw_fallback(pos, laws, rng, &mut outcomes, &mut weight)?;
                break;
            }
        }
        if weight.lo > self.bound {
            return Err("draw likelihood lower bound exceeds plan cap".into());
        }
        Ok((outcomes, weight))
    }

    /// Draw while evaluating both the native proposal likelihood and the
    /// guided-to-native proposal ratio on the same path.
    pub fn draw_hybrid(
        &self,
        model: &Model,
        laws: &[Law],
        rng: &mut Rng,
        guided: bool,
    ) -> Result<HybridDraw, String> {
        if laws.len() != self.masks.len()
            || model.fixtures.len() != laws.len()
            || model.n != self.base.len()
        {
            return Err("draw law dimensions do not match plan".into());
        }
        let mut outcomes = vec![0u8; laws.len()];
        let mut totals = self.base.clone();
        let mut baseline_weight = Interval::point(1.0);
        let mut guide_over_baseline = Interval::point(1.0);
        let mut id = if self.selected.is_empty() {
            None
        } else {
            self.root
        };
        let mut pos = 0usize;
        while pos < self.order.len() {
            let fi = self.order[pos];
            let (base, child) = if let Some(node_id) = id {
                let node = &self.nodes[node_id];
                if let Some(accept) = node.terminal {
                    if !accept {
                        return Err("planner reached a zero-potential terminal".into());
                    }
                    break;
                }
                (node.weights.to_vec(), node.edges.clone())
            } else {
                let w = (0..3)
                    .map(|o| {
                        if self.masks[fi] & (1 << o) != 0 {
                            laws[fi].outcomes[o].mid()
                        } else {
                            0.0
                        }
                    })
                    .collect();
                (w, Vec::new())
            };
            let guided_weights = self.guided_weights(model, laws, fi, pos, &totals, &base)?;
            let base_probs = categorical_probabilities(&base)?;
            let guide_probs = categorical_probabilities(&guided_weights)?;
            let chosen = if guided {
                categorical(&guided_weights, rng)?.0
            } else {
                categorical(&base, rng)?.0
            };
            if id.is_some() && !child.iter().any(|e| e.outcome as usize == chosen) {
                return Err("joint plan edge missing".into());
            }
            outcomes[fi] = chosen as u8;
            baseline_weight =
                baseline_weight.mul(laws[fi].outcomes[chosen].div(base_probs[chosen]));
            guide_over_baseline =
                guide_over_baseline.mul(guide_probs[chosen].div(base_probs[chosen]));
            let (hg, ag) = outcome_gain(model, fi, chosen, self.stride);
            let f = &model.fixtures[fi];
            totals[f.home] = totals[f.home].saturating_add(hg);
            totals[f.away] = totals[f.away].saturating_add(ag);
            pos += 1;
            id = if id.is_some() {
                child
                    .iter()
                    .find(|e| e.outcome as usize == chosen)
                    .and_then(|e| e.child)
            } else {
                None
            };
        }
        if baseline_weight.lo > self.bound {
            return Err("draw likelihood lower bound exceeds plan cap".into());
        }
        Ok(HybridDraw {
            outcomes,
            baseline_weight,
            guide_over_baseline,
        })
    }

    /// Replay the guided-to-native likelihood ratio for a supplied path.
    /// This follows the same memoized edges and sticky native suffix used by
    /// `draw_hybrid`, without sampling or consuming random state.
    pub fn guide_ratio(
        &self,
        model: &Model,
        laws: &[Law],
        outcomes: &[u8],
    ) -> Result<Interval, String> {
        if laws.len() != self.masks.len()
            || model.fixtures.len() != laws.len()
            || outcomes.len() != laws.len()
            || model.n != self.base.len()
        {
            return Err("replay dimensions do not match plan".into());
        }
        if outcomes.iter().any(|&o| o > 2) {
            return Err("replay outcome is outside the categorical domain".into());
        }
        let mut totals = self.base.clone();
        let mut replay_ratio = Interval::point(1.0);
        let mut id = if self.selected.is_empty() { None } else { self.root };
        for (pos, &fi) in self.order.iter().enumerate() {
            let chosen = outcomes[fi] as usize;
            let (base, edges) = if let Some(node_id) = id {
                let node = self.nodes.get(node_id).ok_or("replay node is missing")?;
                (node.weights.to_vec(), Some(&node.edges[..]))
            } else {
                ((0..3).map(|o| if self.masks[fi] & (1 << o) != 0 {
                    laws[fi].outcomes[o].mid()
                } else { 0.0 }).collect(), None)
            };
            if let Some(edges) = edges {
                if !edges.iter().any(|e| e.outcome as usize == chosen) {
                    return Err("replay path has no matching plan edge".into());
                }
            }
            let base_probs = categorical_probabilities(&base)?;
            let guided = self.guided_weights(model, laws, fi, pos, &totals, &base)?;
            let guide_probs = categorical_probabilities(&guided)?;
            if base_probs[chosen].lo <= 0.0 || guide_probs[chosen].lo <= 0.0 {
                return Err("replay outcome has zero proposal support".into());
            }
            replay_ratio = replay_ratio.mul(guide_probs[chosen].div(base_probs[chosen]));
            let (hg, ag) = outcome_gain(model, fi, chosen, self.stride);
            let f = &model.fixtures[fi];
            totals[f.home] = totals[f.home].saturating_add(hg);
            totals[f.away] = totals[f.away].saturating_add(ag);
            id = if let Some(edges) = edges {
                edges.iter().find(|e| e.outcome as usize == chosen).and_then(|e| e.child)
            } else { None };
        }
        Ok(replay_ratio)
    }

    fn draw_fallback(
        &self,
        from: usize,
        laws: &[Law],
        rng: &mut Rng,
        outcomes: &mut [u8],
        weight: &mut Interval,
    ) -> Result<(), String> {
        for pos in from..self.order.len() {
            let fi = self.order[pos];
            let w: Vec<f64> = (0..3)
                .map(|o| {
                    if self.masks[fi] & (1 << o) != 0 {
                        laws[fi].outcomes[o].mid()
                    } else {
                        0.0
                    }
                })
                .collect();
            let (o, q) = categorical(&w, rng)?;
            outcomes[fi] = o as u8;
            *weight = weight.mul(laws[fi].outcomes[o].div(q));
        }
        Ok(())
    }
}

impl Forest {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        model: &Model,
        laws: &[Law],
        masks: &[u8],
        lower: &[i32],
        upper: &[i32],
        selected: &[usize],
        stride: i32,
        prefix_fixtures: &[usize],
        prefixes: &[Vec<u8>],
        state_cap: usize,
        deadline: Instant,
    ) -> Result<Self, String> {
        let nf = model.fixtures.len();
        if laws.len() != nf || masks.len() != nf {
            return Err("joint forest fixture dimensions do not match".into());
        }
        if lower.len() != model.n || upper.len() != model.n {
            return Err("joint forest team dimensions do not match".into());
        }
        if stride <= 0 {
            return Err("joint forest stride must be positive".into());
        }
        if masks.iter().any(|m| m & !7 != 0) {
            return Err("joint forest mask has invalid outcome bits".into());
        }
        if prefixes.iter().any(|p| p.len() != prefix_fixtures.len()) {
            return Err("joint forest prefix dimensions do not match".into());
        }
        let mut is_prefix = vec![false; nf];
        for &fi in prefix_fixtures {
            if fi >= nf || is_prefix[fi] {
                return Err("joint forest prefix fixture is invalid or repeated".into());
            }
            is_prefix[fi] = true;
        }
        let mut seen = vec![false; model.n];
        for &t in selected {
            if t >= model.n || seen[t] {
                return Err("selected team index is invalid or repeated".into());
            }
            seen[t] = true;
        }
        let selected = selected.to_vec();
        let base = (0..model.n)
            .map(|t| {
                model.base[t]
                    .points
                    .checked_mul(stride)
                    .and_then(|x| x.checked_add(model.base[t].wins))
                    .ok_or("packed base overflow")
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Fixed single-support suffix fixtures are exposed first. The
        // remaining variable fixture order uses the same deterministic
        // selected-team min-degree/min-fill elimination as Plan::build.
        let mut fixed = Vec::new();
        let mut variable = Vec::new();
        for fi in 0..nf {
            if is_prefix[fi] {
                continue;
            }
            let support = (0..3)
                .filter(|&o| masks[fi] & (1 << o) != 0 && laws[fi].outcomes[o].hi > 0.0)
                .count();
            if support <= 1 {
                fixed.push(fi);
            } else {
                variable.push(fi);
            }
        }
        fixed.sort_unstable();
        variable.sort_unstable();
        let mut order = fixed;
        let mut remaining = vec![false; nf];
        for &fi in &variable {
            remaining[fi] = true;
        }
        let mut active = seen.clone();
        while active.iter().any(|&x| x) {
            if Instant::now() >= deadline {
                break;
            }
            let mut best: Option<(usize, usize, usize, i32, usize)> = None;
            for t in 0..model.n {
                if !active[t] {
                    continue;
                }
                let mut neighbors = vec![false; model.n];
                let mut remaining_games = 0usize;
                for &fi in &variable {
                    if !remaining[fi] {
                        continue;
                    }
                    let f = &model.fixtures[fi];
                    if f.home == t || f.away == t {
                        remaining_games += 1;
                        let other = if f.home == t { f.away } else { f.home };
                        if active[other] && other != t {
                            neighbors[other] = true;
                        }
                    }
                }
                let degree = neighbors.iter().filter(|&&x| x).count();
                let mut fill = 0usize;
                for a in 0..model.n {
                    if !neighbors[a] {
                        continue;
                    }
                    for b in a + 1..model.n {
                        if !neighbors[b] {
                            continue;
                        }
                        let adjacent = variable.iter().any(|&fi| {
                            remaining[fi]
                                && ((model.fixtures[fi].home == a && model.fixtures[fi].away == b)
                                    || (model.fixtures[fi].home == b
                                        && model.fixtures[fi].away == a))
                        });
                        if !adjacent {
                            fill += 1;
                        }
                    }
                }
                let key = (degree, fill, remaining_games, model.ids[t], t);
                if best.map_or(true, |current| key < current) {
                    best = Some(key);
                }
            }
            let team = best.ok_or("selected fixture elimination failed")?.4;
            for &fi in &variable {
                let f = &model.fixtures[fi];
                if remaining[fi] && (f.home == team || f.away == team) {
                    order.push(fi);
                    remaining[fi] = false;
                }
            }
            active[team] = false;
        }
        for fi in variable {
            if remaining[fi] {
                order.push(fi);
            }
        }
        if order.len() + prefix_fixtures.len() != nf {
            return Err("joint forest fixture ordering omitted a fixture".into());
        }

        // Clamp a one-sided lower state only when every outcome, including
        // every possible prefix assignment, is already proven below upper.
        let mut max_final = base.clone();
        for (fi, f) in model.fixtures.iter().enumerate() {
            for t in [f.home, f.away] {
                let mut max_gain = 0;
                for o in 0..3 {
                    if masks[fi] & (1 << o) == 0 || laws[fi].outcomes[o].hi <= 0.0 {
                        continue;
                    }
                    let (hg, ag) = outcome_gain(model, fi, o, stride);
                    max_gain = max_gain.max(if t == f.home { hg } else { ag });
                }
                max_final[t] = max_final[t].saturating_add(max_gain);
            }
        }
        let clamp_lower: Vec<bool> = selected
            .iter()
            .map(|&t| upper[t] > lower[t] && upper[t] >= max_final[t])
            .collect();

        // All proposal suffix factors are computed once, over the common
        // suffix only; prefix mass and prefix mixture tickets are caller-owned.
        let ns = order.len();
        let mut fallback_caps = vec![1.0; ns + 1];
        let mut fallback_potentials = vec![1.0; ns + 1];
        for pos in (0..ns).rev() {
            let fi = order[pos];
            let w: Vec<f64> = (0..3)
                .map(|o| {
                    if masks[fi] & (1 << o) != 0 {
                        laws[fi].outcomes[o].mid()
                    } else {
                        0.0
                    }
                })
                .collect();
            let mass = w.iter().sum::<f64>().min(1.0);
            let raw = mass * fallback_potentials[pos + 1];
            fallback_potentials[pos] = if raw == 0.0 && mass > 0.0 {
                f64::from_bits(1)
            } else {
                raw
            };
            if !w.iter().any(|x| *x > 0.0) {
                fallback_caps[pos] = 0.0;
                continue;
            }
            let q = categorical_probabilities(&w)?;
            let mut step = 0.0f64;
            for o in 0..3 {
                if w[o] > 0.0 {
                    step = step.max(ratio(laws[fi].outcomes[o], q[o])?);
                }
            }
            fallback_caps[pos] = mul_cap(step, fallback_caps[pos + 1]);
        }

        let mut core = Plan {
            bound: fallback_caps[0],
            states: 0,
            complete: true,
            selected,
            masks: masks.to_vec(),
            order,
            nodes: Vec::new(),
            root: None,
            stride,
            lower: lower.to_vec(),
            upper: upper.to_vec(),
            base,
            clamp_lower,
            fallback_caps,
            fallback_potentials,
            guide: OnceLock::new(),
        };
        let ns = core.order.len();
        let mut last_touch = vec![None; core.selected.len()];
        for (pos, &fi) in core.order.iter().enumerate() {
            let f = &model.fixtures[fi];
            for (j, &t) in core.selected.iter().enumerate() {
                if t == f.home || t == f.away {
                    last_touch[j] = Some(pos);
                }
            }
        }
        let mut suffix_min = vec![vec![0i32; core.selected.len()]; ns + 1];
        let mut suffix_max = vec![vec![0i32; core.selected.len()]; ns + 1];
        let mut suffix_possible = vec![true; ns + 1];
        for pos in (0..ns).rev() {
            suffix_min[pos] = suffix_min[pos + 1].clone();
            suffix_max[pos] = suffix_max[pos + 1].clone();
            suffix_possible[pos] = suffix_possible[pos + 1];
            let fi = core.order[pos];
            let f = &model.fixtures[fi];
            for (j, &t) in core.selected.iter().enumerate() {
                if t != f.home && t != f.away {
                    continue;
                }
                let mut lo = i32::MAX;
                let mut hi = i32::MIN;
                for o in 0..3 {
                    if core.masks[fi] & (1 << o) == 0 || laws[fi].outcomes[o].hi <= 0.0 {
                        continue;
                    }
                    let (hg, ag) = outcome_gain(model, fi, o, stride);
                    let g = if t == f.home { hg } else { ag };
                    lo = lo.min(g);
                    hi = hi.max(g);
                }
                if lo == i32::MAX {
                    suffix_possible[pos] = false;
                } else {
                    suffix_min[pos][j] = suffix_min[pos][j].saturating_add(lo);
                    suffix_max[pos][j] = suffix_max[pos][j].saturating_add(hi);
                }
            }
        }

        fn build_node(
            plan: &mut Plan,
            model: &Model,
            laws: &[Law],
            state: Vec<i32>,
            pos: usize,
            memo: &mut HashMap<(usize, Vec<i32>), usize>,
            cap: usize,
            deadline: Instant,
            limit_hit: &mut bool,
            suffix_min: &[Vec<i32>],
            suffix_max: &[Vec<i32>],
            suffix_possible: &[bool],
            last_touch: &[Option<usize>],
        ) -> Result<Option<usize>, String> {
            if let Some(&id) = memo.get(&(pos, state.clone())) {
                return Ok(Some(id));
            }
            if *limit_hit || plan.states >= cap || Instant::now() >= deadline {
                *limit_hit = true;
                plan.complete = false;
                return Ok(None);
            }
            plan.states += 1;
            if pos == plan.order.len() {
                let accept = plan.selected.iter().enumerate().all(|(j, &t)| {
                    last_touch[j].is_none()
                        || state[j] == 0
                        || (state[j] >= plan.lower[t] && state[j] <= plan.upper[t])
                });
                let id = plan.nodes.len();
                plan.nodes.push(Node {
                    weights: [0.; 3],
                    edges: Vec::new(),
                    potential: if accept { 1. } else { 0. },
                    cap: if accept { 1. } else { 0. },
                    terminal: Some(accept),
                });
                memo.insert((pos, state), id);
                return Ok(Some(id));
            }
            let fi = plan.order[pos];
            let f = &model.fixtures[fi];
            let mut weights = [0.; 3];
            let mut edges = Vec::new();
            let mut potential = 0.;
            for o in 0..3 {
                if plan.masks[fi] & (1 << o) == 0 {
                    continue;
                }
                let p = laws[fi].outcomes[o];
                if p.hi <= 0. {
                    continue;
                }
                let (hg, ag) = outcome_gain(model, fi, o, plan.stride);
                let mut next = state.clone();
                for (j, &t) in plan.selected.iter().enumerate() {
                    if t == f.home {
                        next[j] = next[j].checked_add(hg).ok_or("packed state overflow")?;
                        if plan.clamp_lower[j] {
                            next[j] = next[j].min(plan.lower[t]);
                        }
                    }
                    if t == f.away {
                        next[j] = next[j].checked_add(ag).ok_or("packed state overflow")?;
                        if plan.clamp_lower[j] {
                            next[j] = next[j].min(plan.lower[t]);
                        }
                    }
                }
                let mut feasible = true;
                for (j, &t) in plan.selected.iter().enumerate() {
                    if last_touch[j] == Some(pos) {
                        if next[j] < plan.lower[t] || next[j] > plan.upper[t] {
                            feasible = false;
                            break;
                        }
                        next[j] = 0;
                    }
                }
                if !suffix_possible[pos + 1] {
                    feasible = false;
                }
                for (j, &t) in plan.selected.iter().enumerate() {
                    if !last_touch[j].is_some_and(|last| last > pos) {
                        continue;
                    }
                    if !feasible
                        || next[j].saturating_add(suffix_max[pos + 1][j]) < plan.lower[t]
                        || next[j].saturating_add(suffix_min[pos + 1][j]) > plan.upper[t]
                    {
                        feasible = false;
                        break;
                    }
                }
                if !feasible {
                    continue;
                }
                let child = build_node(
                    plan,
                    model,
                    laws,
                    next,
                    pos + 1,
                    memo,
                    cap,
                    deadline,
                    limit_hit,
                    suffix_min,
                    suffix_max,
                    suffix_possible,
                    last_touch,
                )?;
                let child_potential = child.map_or(plan.fallback_potentials[pos + 1], |id| {
                    plan.nodes[id].potential
                });
                let raw = p.mid() * child_potential;
                let w = if raw == 0. && p.mid() > 0. && child_potential > 0. {
                    f64::from_bits(1)
                } else {
                    raw
                };
                if w <= 0. {
                    continue;
                }
                weights[o] = w;
                edges.push(Edge {
                    outcome: o as u8,
                    child: child,
                });
            }
            if !weights.iter().any(|x| *x > 0.) {
                let id = plan.nodes.len();
                plan.nodes.push(Node {
                    weights,
                    edges,
                    potential: 0.,
                    cap: 0.,
                    terminal: None,
                });
                memo.insert((pos, state), id);
                return Ok(Some(id));
            }
            let q = categorical_probabilities(&weights)?;
            let mut qbound = 0.0f64;
            for edge in &edges {
                let o = edge.outcome as usize;
                let child_cap = edge
                    .child
                    .map_or(plan.fallback_caps[pos + 1], |id| plan.nodes[id].cap);
                qbound = qbound.max(mul_cap(ratio(laws[fi].outcomes[o], q[o])?, child_cap));
                let child_pot = edge.child.map_or(plan.fallback_potentials[pos + 1], |id| {
                    plan.nodes[id].potential
                });
                let term = laws[fi].outcomes[o].mid() * child_pot;
                potential = up(potential
                    + if term == 0. && laws[fi].outcomes[o].mid() > 0. && child_pot > 0. {
                        f64::from_bits(1)
                    } else {
                        term
                    });
            }
            let id = plan.nodes.len();
            plan.nodes.push(Node {
                weights,
                edges,
                potential: potential.min(1.),
                cap: qbound,
                terminal: None,
            });
            memo.insert((pos, state), id);
            Ok(Some(id))
        }

        let mut memo = HashMap::new();
        let mut completeness_memo = HashMap::new();
        let mut limit_hit = false;
        let mut roots = Vec::with_capacity(prefixes.len());
        for prefix in prefixes {
            let mut prefix_mass = Interval::point(1.);
            let mut state = core
                .selected
                .iter()
                .map(|&t| core.base[t])
                .collect::<Vec<_>>();
            let mut possible = true;
            for (&fi, &outcome) in prefix_fixtures.iter().zip(prefix) {
                if outcome > 2 || masks[fi] & (1 << outcome) == 0 {
                    return Err("joint forest prefix outcome outside mask".into());
                }
                let p = laws[fi].outcomes[outcome as usize];
                prefix_mass = prefix_mass.mul(p);
                if p.hi <= 0. {
                    possible = false;
                    break;
                }
                let f = &model.fixtures[fi];
                let (hg, ag) = outcome_gain(model, fi, outcome as usize, stride);
                for (j, &t) in core.selected.iter().enumerate() {
                    if t == f.home {
                        state[j] = state[j].checked_add(hg).ok_or("packed prefix overflow")?;
                    }
                    if t == f.away {
                        state[j] = state[j].checked_add(ag).ok_or("packed prefix overflow")?;
                    }
                }
            }
            if !possible || prefix_mass.hi == 0. {
                roots.push(ForestRoot {
                    prefix_mass,
                    potential: 0.,
                    cap: 0.,
                    complete: true,
                    zero: true,
                    prefix_outcomes: prefix.clone(),
                    node: None,
                });
                continue;
            }
            let mut closed_bad = false;
            for (j, &t) in core.selected.iter().enumerate() {
                if core.clamp_lower[j] {
                    state[j] = state[j].min(core.lower[t]);
                }
                if last_touch[j].is_none() {
                    if state[j] < core.lower[t] || state[j] > core.upper[t] {
                        closed_bad = true;
                    }
                    state[j] = 0;
                }
            }
            if Instant::now() >= deadline {
                limit_hit = true;
                core.complete = false;
            }
            let node = if closed_bad || limit_hit {
                None
            } else {
                build_node(
                    &mut core,
                    model,
                    laws,
                    state,
                    0,
                    &mut memo,
                    state_cap,
                    deadline,
                    &mut limit_hit,
                    &suffix_min,
                    &suffix_max,
                    &suffix_possible,
                    &last_touch,
                )?
            };
            let unresolved = node.is_none() && !closed_bad;
            let potential = if closed_bad {
                0.
            } else {
                node.map_or(core.fallback_potentials[0], |id| core.nodes[id].potential)
            };
            let cap = if closed_bad {
                0.
            } else {
                node.map_or(core.fallback_caps[0], |id| core.nodes[id].cap)
            };
            let complete = closed_bad
                || node.is_some_and(|id| node_complete(id, &core.nodes, &mut completeness_memo));
            roots.push(ForestRoot {
                prefix_mass,
                potential,
                cap,
                complete,
                zero: complete && potential == 0.,
                prefix_outcomes: prefix.clone(),
                node: if closed_bad { Some(usize::MAX) } else { node },
            });
            if unresolved {
                core.complete = false;
            }
        }
        let complete = roots.iter().all(|r| r.complete);
        Ok(Self {
            states: core.states,
            complete,
            roots,
            core,
            prefix_fixtures: prefix_fixtures.to_vec(),
        })
    }

    /// Draw from one root's suffix proposal. Prefix outcomes are included in
    /// the returned fixture-indexed vector but contribute no likelihood factor.
    pub fn draw_root(
        &self,
        root_index: usize,
        laws: &[Law],
        rng: &mut Rng,
    ) -> Result<(Vec<u8>, Interval), String> {
        if laws.len() != self.core.masks.len() {
            return Err("draw law dimensions do not match forest".into());
        }
        let root = self
            .roots
            .get(root_index)
            .ok_or("forest root index out of range")?;
        if root.zero {
            return Err("cannot draw from certified zero forest root".into());
        }
        let mut outcomes = vec![0u8; laws.len()];
        for (&fi, &o) in self.prefix_fixtures.iter().zip(&root.prefix_outcomes) {
            outcomes[fi] = o;
        }
        let mut weight = Interval::point(1.);
        let mut id = if root.node == Some(usize::MAX) {
            None
        } else {
            root.node
        };
        let mut pos = 0;
        if id.is_none() {
            self.core
                .draw_fallback(0, laws, rng, &mut outcomes, &mut weight)?;
        }
        while let Some(node_id) = id {
            let node = &self.core.nodes[node_id];
            if let Some(accept) = node.terminal {
                if !accept {
                    return Err("forest reached a zero-potential terminal".into());
                }
                break;
            }
            let (o, q) = categorical(&node.weights, rng)?;
            let edge = node
                .edges
                .iter()
                .find(|e| e.outcome as usize == o)
                .ok_or("forest edge missing")?;
            let fi = self.core.order[pos];
            outcomes[fi] = o as u8;
            weight = weight.mul(laws[fi].outcomes[o].div(q));
            pos += 1;
            id = edge.child;
            if id.is_none() {
                self.core
                    .draw_fallback(pos, laws, rng, &mut outcomes, &mut weight)?;
                break;
            }
        }
        if weight.lo > root.cap {
            return Err("draw likelihood lower bound exceeds forest root cap".into());
        }
        Ok((outcomes, weight))
    }

    /// Hybrid counterpart to `draw_root`; prefix fixtures are fixed and carry
    /// no likelihood factor, but their gains are included in endpoint scores.
    pub fn draw_root_hybrid(
        &self,
        root_index: usize,
        model: &Model,
        laws: &[Law],
        rng: &mut Rng,
        guided: bool,
    ) -> Result<HybridDraw, String> {
        if laws.len() != self.core.masks.len()
            || model.fixtures.len() != laws.len()
            || model.n != self.core.base.len()
        {
            return Err("draw law dimensions do not match forest".into());
        }
        let root = self
            .roots
            .get(root_index)
            .ok_or("forest root index out of range")?;
        if root.zero {
            return Err("cannot draw from certified zero forest root".into());
        }
        let mut outcomes = vec![0u8; laws.len()];
        let mut totals = self.core.base.clone();
        for (&fi, &o) in self.prefix_fixtures.iter().zip(&root.prefix_outcomes) {
            outcomes[fi] = o;
            let (hg, ag) = outcome_gain(model, fi, o as usize, self.core.stride);
            let f = &model.fixtures[fi];
            totals[f.home] = totals[f.home].saturating_add(hg);
            totals[f.away] = totals[f.away].saturating_add(ag);
        }
        let mut baseline_weight = Interval::point(1.0);
        let mut guide_over_baseline = Interval::point(1.0);
        let mut id = if root.node == Some(usize::MAX) {
            None
        } else {
            root.node
        };
        for pos in 0..self.core.order.len() {
            let fi = self.core.order[pos];
            let (base, edges) = if let Some(node_id) = id {
                let node = &self.core.nodes[node_id];
                if let Some(accept) = node.terminal {
                    if !accept {
                        return Err("forest reached a zero-potential terminal".into());
                    }
                    break;
                }
                (node.weights.to_vec(), node.edges.clone())
            } else {
                (
                    (0..3)
                        .map(|o| {
                            if self.core.masks[fi] & (1 << o) != 0 {
                                laws[fi].outcomes[o].mid()
                            } else {
                                0.0
                            }
                        })
                        .collect(),
                    Vec::new(),
                )
            };
            let guided_weights = self
                .core
                .guided_weights(model, laws, fi, pos, &totals, &base)?;
            let base_probs = categorical_probabilities(&base)?;
            let guide_probs = categorical_probabilities(&guided_weights)?;
            let chosen = if guided {
                categorical(&guided_weights, rng)?.0
            } else {
                categorical(&base, rng)?.0
            };
            if id.is_some() && !edges.iter().any(|e| e.outcome as usize == chosen) {
                return Err("forest edge missing".into());
            }
            outcomes[fi] = chosen as u8;
            baseline_weight =
                baseline_weight.mul(laws[fi].outcomes[chosen].div(base_probs[chosen]));
            guide_over_baseline =
                guide_over_baseline.mul(guide_probs[chosen].div(base_probs[chosen]));
            let (hg, ag) = outcome_gain(model, fi, chosen, self.core.stride);
            let f = &model.fixtures[fi];
            totals[f.home] = totals[f.home].saturating_add(hg);
            totals[f.away] = totals[f.away].saturating_add(ag);
            id = if id.is_some() {
                edges
                    .iter()
                    .find(|e| e.outcome as usize == chosen)
                    .and_then(|e| e.child)
            } else {
                None
            };
        }
        if baseline_weight.lo > root.cap {
            return Err("draw likelihood lower bound exceeds forest root cap".into());
        }
        Ok(HybridDraw {
            outcomes,
            baseline_weight,
            guide_over_baseline,
        })
    }

    /// Replay a root's guided-to-native likelihood ratio for a supplied full
    /// fixture assignment. Fixed prefix fixtures are checked and contribute
    /// gains to guidance, while carrying no suffix likelihood factor.
    pub fn root_guide_ratio(
        &self,
        root_index: usize,
        model: &Model,
        laws: &[Law],
        outcomes: &[u8],
    ) -> Result<Interval, String> {
        if laws.len() != self.core.masks.len()
            || model.fixtures.len() != laws.len()
            || outcomes.len() != laws.len()
            || model.n != self.core.base.len()
        {
            return Err("replay dimensions do not match forest".into());
        }
        if outcomes.iter().any(|&o| o > 2) {
            return Err("replay outcome is outside the categorical domain".into());
        }
        let root = self.roots.get(root_index).ok_or("forest root index out of range")?;
        if root.zero {
            return Err("cannot replay a certified zero forest root".into());
        }
        for (&fi, &fixed) in self.prefix_fixtures.iter().zip(&root.prefix_outcomes) {
            if outcomes[fi] != fixed {
                return Err("replay prefix contradicts selected forest root".into());
            }
        }
        let mut totals = self.core.base.clone();
        for (&fi, &o) in self.prefix_fixtures.iter().zip(&root.prefix_outcomes) {
            let (hg, ag) = outcome_gain(model, fi, o as usize, self.core.stride);
            let f = &model.fixtures[fi];
            totals[f.home] = totals[f.home].saturating_add(hg);
            totals[f.away] = totals[f.away].saturating_add(ag);
        }
        let mut replay_ratio = Interval::point(1.0);
        let mut id = if root.node == Some(usize::MAX) { None } else { root.node };
        for (pos, &fi) in self.core.order.iter().enumerate() {
            let chosen = outcomes[fi] as usize;
            let (base, edges) = if let Some(node_id) = id {
                let node = self.core.nodes.get(node_id).ok_or("replay node is missing")?;
                (node.weights.to_vec(), Some(&node.edges[..]))
            } else {
                ((0..3).map(|o| if self.core.masks[fi] & (1 << o) != 0 {
                    laws[fi].outcomes[o].mid()
                } else { 0.0 }).collect(), None)
            };
            if let Some(edges) = edges {
                if !edges.iter().any(|e| e.outcome as usize == chosen) {
                    return Err("replay path has no matching forest edge".into());
                }
            }
            let base_probs = categorical_probabilities(&base)?;
            let guided = self.core.guided_weights(model, laws, fi, pos, &totals, &base)?;
            let guide_probs = categorical_probabilities(&guided)?;
            if base_probs[chosen].lo <= 0.0 || guide_probs[chosen].lo <= 0.0 {
                return Err("replay outcome has zero proposal support".into());
            }
            replay_ratio = replay_ratio.mul(guide_probs[chosen].div(base_probs[chosen]));
            let (hg, ag) = outcome_gain(model, fi, chosen, self.core.stride);
            let f = &model.fixtures[fi];
            totals[f.home] = totals[f.home].saturating_add(hg);
            totals[f.away] = totals[f.away].saturating_add(ag);
            id = if let Some(edges) = edges {
                edges.iter().find(|e| e.outcome as usize == chosen).and_then(|e| e.child)
            } else { None };
        }
        Ok(replay_ratio)
    }

    /// Aggregate lazy guide-cache size shared by all roots in this forest.
    pub fn hybrid_guide_cache_stats(&self) -> (usize, usize) {
        self.core.hybrid_guide_cache_stats()
    }
}

fn node_complete(id: usize, nodes: &[Node], memo: &mut HashMap<usize, bool>) -> bool {
    if let Some(&v) = memo.get(&id) {
        return v;
    }
    let complete = nodes[id].edges.iter().all(|e| {
        e.child
            .is_some_and(|child| node_complete(child, nodes, memo))
    });
    memo.insert(id, complete);
    complete
}

#[cfg(test)]
mod tests {
    use super::*;
    use golaberto_odds::model::Request;
    use serde_json::json;

    fn tiny_model(games: usize) -> (Model, Vec<Law>) {
        let request: Request = serde_json::from_value(json!({
            "id": 1,
            "phase": {"sort":"pt,w,gd,gf,gp,name", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":[{"team_id":10},{"team_id":20}],
            "games":(0..games).map(|i| json!({"id":i+1,"home_id":10,"away_id":20,"home_power":1.1,"away_power":0.8,"played":false})).collect::<Vec<_>>()
        })).unwrap();
        let model = Model::new(request).unwrap();
        let laws = model
            .fixtures
            .iter()
            .map(|f| Law::new(f.home_sampler.mean, f.away_sampler.mean).unwrap())
            .collect();
        (model, laws)
    }

    #[test]
    fn shared_fixture_joint_event_uses_one_fixture_mass() {
        let (model, laws) = tiny_model(1);
        let all = [7u8];
        let stride = 4;
        let mut lower = vec![i32::MIN / 4; 2];
        let mut upper = vec![i32::MAX / 4; 2];
        // Home must win and away must lose. Both conditions refer to the same
        // WDL draw, so the joint probability is p(home win), not p(home win)^2.
        lower[0] = 13;
        upper[0] = 13;
        lower[1] = 0;
        upper[1] = 0;
        let plan = Plan::build(
            &model,
            &laws,
            &all,
            &lower,
            &upper,
            &[0, 1],
            stride,
            100,
            Instant::now() + std::time::Duration::from_secs(5),
        )
        .unwrap();
        assert!(plan.complete);
        assert_eq!(plan.states, 2);
        assert!(plan.nodes[plan.root.unwrap()].potential >= laws[0].outcomes[2].lo);
        assert!(plan.nodes[plan.root.unwrap()].potential <= laws[0].outcomes[2].hi + 1e-12);
        assert!(plan.bound >= laws[0].outcomes[2].lo);
        let (outcomes, weight) = plan.draw(&laws, &mut Rng::new(37)).unwrap();
        assert_eq!(outcomes, vec![2]);
        assert!(weight.lo <= laws[0].outcomes[2].hi && weight.hi >= laws[0].outcomes[2].lo);
        assert!(weight.hi <= plan.bound * (1.0 + 1e-12));
    }

    #[test]
    fn contradictory_shared_constraints_are_exactly_refuted() {
        let (model, laws) = tiny_model(1);
        let mut lower = vec![0; 2];
        let mut upper = vec![i32::MAX / 4; 2];
        lower[0] = 13;
        upper[0] = 13;
        lower[1] = 13;
        upper[1] = 13;
        let plan = Plan::build(
            &model,
            &laws,
            &[7],
            &lower,
            &upper,
            &[0, 1],
            4,
            100,
            Instant::now() + std::time::Duration::from_secs(5),
        )
        .unwrap();
        assert!(plan.complete);
        assert_eq!(plan.bound, 0.0);
    }

    #[test]
    fn partial_table_uses_native_mask_fallback_and_cap() {
        let (model, laws) = tiny_model(2);
        let masks = [7, 7];
        let mut lower = vec![0; 2];
        let upper = vec![i32::MAX / 4; 2];
        lower[0] = 26;
        let plan = Plan::build(
            &model,
            &laws,
            &masks,
            &lower,
            &upper,
            &[0],
            4,
            0,
            Instant::now() + std::time::Duration::from_secs(5),
        )
        .unwrap();
        assert!(!plan.complete);
        assert_eq!(plan.states, 0);
        assert!(plan.bound >= 1.0);
        for seed in 0..128 {
            let (_, w) = plan.draw(&laws, &mut Rng::new(seed)).unwrap();
            assert!(
                w.hi <= plan.bound * (1.0 + 1e-12),
                "weight {:?} exceeds {}",
                w,
                plan.bound
            );
        }
        let empty = Plan::build(
            &model,
            &laws,
            &masks,
            &lower,
            &upper,
            &[],
            4,
            10,
            Instant::now() + std::time::Duration::from_secs(5),
        )
        .unwrap();
        assert!(empty.complete);
        assert_eq!(empty.states, 0);
    }

    #[test]
    fn hybrid_baseline_keeps_rng_path_and_cap_and_guided_keeps_support() {
        let (model, laws) = tiny_model(3);
        let masks = [7, 7, 7];
        let lower = vec![39, 0];
        let upper = vec![39, 18];
        let plan = Plan::build(
            &model,
            &laws,
            &masks,
            &lower,
            &upper,
            &[0],
            4,
            2,
            Instant::now() + std::time::Duration::from_secs(5),
        )
        .unwrap();
        // Independently initialized lazy PMF caches must produce the same
        // guided path and ratios for the same RNG seed.
        let fresh_plan = Plan::build(
            &model,
            &laws,
            &masks,
            &lower,
            &upper,
            &[0],
            4,
            2,
            Instant::now() + std::time::Duration::from_secs(5),
        )
        .unwrap();
        for seed in 0..128 {
            let expected = plan.draw(&laws, &mut Rng::new(seed)).unwrap();
            let got = plan
                .draw_hybrid(&model, &laws, &mut Rng::new(seed), false)
                .unwrap();
            assert_eq!(got.outcomes, expected.0);
            assert_eq!(got.baseline_weight, expected.1);
            assert!(got.guide_over_baseline.lo > 0.0);
            assert!(got.baseline_weight.lo <= plan.bound);
            let guided = plan
                .draw_hybrid(&model, &laws, &mut Rng::new(seed), true)
                .unwrap();
            let fresh_guided = fresh_plan
                .draw_hybrid(&model, &laws, &mut Rng::new(seed), true)
                .unwrap();
            assert_eq!(guided.outcomes, fresh_guided.outcomes);
            assert_eq!(guided.baseline_weight, fresh_guided.baseline_weight);
            assert_eq!(guided.guide_over_baseline, fresh_guided.guide_over_baseline);
            assert!(guided.baseline_weight.lo <= plan.bound);
            for (fi, outcome) in guided.outcomes.iter().enumerate() {
                assert_ne!(masks[fi] & (1 << *outcome), 0);
            }
        }
        assert_eq!(
            plan.hybrid_guide_cache_stats(),
            fresh_plan.hybrid_guide_cache_stats()
        );
        assert!(plan.hybrid_guide_cache_stats().0 > 0);
    }

    #[test]
    fn hybrid_actual_path_ratios_normalize_for_one_fixture() {
        let (model, laws) = tiny_model(1);
        let masks = [7];
        let lower = [0, 0];
        let upper = [1000, 0];
        let plan = Plan::build(
            &model,
            &laws,
            &masks,
            &lower,
            &upper,
            &[],
            4,
            100,
            Instant::now() + std::time::Duration::from_secs(5),
        )
        .unwrap();
        let mut qg_by_outcome = [None; 3];
        for seed in 0..4096 {
            let draw = plan
                .draw_hybrid(&model, &laws, &mut Rng::new(seed), true)
                .unwrap();
            let o = draw.outcomes[0] as usize;
            let qbase = laws[0].outcomes[o].mid() / draw.baseline_weight.mid();
            qg_by_outcome[o] = Some(draw.guide_over_baseline.mid() * qbase);
            if qg_by_outcome.iter().all(Option::is_some) {
                break;
            }
        }
        let total: f64 = qg_by_outcome.into_iter().map(Option::unwrap).sum();
        assert!(
            (total - 1.0).abs() < 1e-12,
            "guided probabilities sum to {total}"
        );
    }

    #[test]
    fn replay_matches_hybrid_paths_for_full_and_partial_plans() {
        let (model, laws) = tiny_model(3);
        let masks = [7, 5, 3];
        let lower = [0, 0];
        let upper = [1000, 1000];
        for cap in [1, 2, 3, 100] {
            let plan = Plan::build(
                &model, &laws, &masks, &lower, &upper, &[0], 4, cap,
                Instant::now() + std::time::Duration::from_secs(5),
            ).unwrap();
            for seed in 0..32 {
                let hybrid = plan.draw_hybrid(&model, &laws, &mut Rng::new(seed), false).unwrap();
                let replay = plan.guide_ratio(&model, &laws, &hybrid.outcomes).unwrap();
                assert!((replay.mid() - hybrid.guide_over_baseline.mid()).abs() < 1e-12);
                let mut a = Rng::new(seed);
                let (outcomes, weight) = plan.draw(&laws, &mut a).unwrap();
                assert_eq!(outcomes, hybrid.outcomes);
                assert!((weight.mid() - hybrid.baseline_weight.mid()).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn partial_sticky_edges_preserve_joint_mass_for_state_caps_one_to_three() {
        let (model, laws) = tiny_model(3);
        let masks = [7, 7, 7];
        let stride = 4;
        let lower = vec![39, 0];
        let upper = vec![39, 0];
        fn finish_path(
            plan: &Plan,
            laws: &[Law],
            pos: usize,
            node_id: Option<usize>,
            outcomes: &mut [u8],
            q_path: Interval,
            w_path: Interval,
            event_mass: &mut Interval,
            q_total: &mut Interval,
            max_upper: &mut f64,
        ) -> Result<(), String> {
            if pos == plan.order.len() {
                *q_total = q_total.add(q_path);
                if outcomes.iter().all(|&o| o == 2) {
                    *event_mass = event_mass.add(q_path.mul(w_path));
                }
                *max_upper = max_upper.max(w_path.hi);
                if w_path.lo > plan.bound {
                    return Err("enumerated path lower weight exceeds cap".into());
                }
                return Ok(());
            }
            let fi = plan.order[pos];
            let (weights, node_edges) = if let Some(id) = node_id {
                let node = &plan.nodes[id];
                if let Some(accept) = node.terminal {
                    if accept {
                        *q_total = q_total.add(q_path);
                    }
                    return Ok(());
                }
                (node.weights.to_vec(), Some(&node.edges))
            } else {
                (
                    (0..3)
                        .map(|o| {
                            if plan.masks[fi] & (1 << o) != 0 {
                                laws[fi].outcomes[o].mid()
                            } else {
                                0.0
                            }
                        })
                        .collect(),
                    None,
                )
            };
            let probs = categorical_probabilities(&weights)?;
            for o in 0..3 {
                if probs[o].hi == 0.0 {
                    continue;
                }
                let child = node_edges
                    .and_then(|edges| edges.iter().find(|e| e.outcome as usize == o))
                    .and_then(|e| e.child);
                outcomes[fi] = o as u8;
                finish_path(
                    plan,
                    laws,
                    pos + 1,
                    child,
                    outcomes,
                    q_path.mul(probs[o]),
                    w_path.mul(laws[fi].outcomes[o].div(probs[o])),
                    event_mass,
                    q_total,
                    max_upper,
                )?;
            }
            Ok(())
        }
        for state_cap in 1..=3 {
            let plan = Plan::build(
                &model,
                &laws,
                &masks,
                &lower,
                &upper,
                &[0, 1],
                stride,
                state_cap,
                Instant::now() + std::time::Duration::from_secs(5),
            )
            .unwrap();
            assert!(
                !plan.complete,
                "cap {state_cap} should leave at least one sticky fallback edge"
            );
            let mut event_mass = Interval::point(0.0);
            let mut q_total = Interval::point(0.0);
            let mut max_upper = 0.0f64;
            finish_path(
                &plan,
                &laws,
                0,
                plan.root,
                &mut vec![0u8; 3],
                Interval::point(1.0),
                Interval::point(1.0),
                &mut event_mass,
                &mut q_total,
                &mut max_upper,
            )
            .unwrap();
            let native_event = laws[0].outcomes[2]
                .mul(laws[1].outcomes[2])
                .mul(laws[2].outcomes[2]);
            assert!(
                event_mass.lo <= native_event.hi && event_mass.hi >= native_event.lo,
                "cap {state_cap}: event mass {:?} does not enclose native {:?}",
                event_mass,
                native_event
            );
            assert!(
                q_total.lo <= 1.0 && q_total.hi >= 1.0,
                "proposal mass {:?}",
                q_total
            );
            assert!(
                max_upper <= plan.bound * (1.0 + 1e-12),
                "cap {state_cap}: {max_upper} > {}",
                plan.bound
            );
        }
    }

    #[test]
    fn lower_state_clamp_applies_only_to_proven_one_sided_bounds() {
        let (model, laws) = tiny_model(2);
        let masks = [7, 7];
        let lower = [13, i32::MIN / 4];
        let upper = [i32::MAX / 4, i32::MAX / 4];
        let lower_only = Plan::build(
            &model,
            &laws,
            &masks,
            &lower,
            &upper,
            &[0],
            4,
            100,
            Instant::now() + std::time::Duration::from_secs(5),
        )
        .unwrap();
        assert!(lower_only.complete);
        assert!(lower_only.clamp_lower[0]);
        for seed in 0..64 {
            let (outcomes, _) = lower_only.draw(&laws, &mut Rng::new(seed)).unwrap();
            assert!(outcomes.iter().any(|&o| o == 2));
        }

        let exact_upper = [13, i32::MAX / 4];
        let exact = Plan::build(
            &model,
            &laws,
            &masks,
            &lower,
            &exact_upper,
            &[0],
            4,
            100,
            Instant::now() + std::time::Duration::from_secs(5),
        )
        .unwrap();
        assert!(!exact.clamp_lower[0]);
        for seed in 0..64 {
            let (outcomes, _) = exact.draw(&laws, &mut Rng::new(seed)).unwrap();
            assert_eq!(outcomes.iter().filter(|&&o| o == 2).count(), 1);
        }
    }

    #[test]
    fn unknown_edge_potential_includes_rare_native_mask_suffix_mass() {
        let (model, mut laws) = tiny_model(3);
        for law in &mut laws {
            law.outcomes = [
                Interval::point(1e-20),
                Interval::point(0.5),
                Interval::point(0.5),
            ];
        }
        let masks = [1, 1, 7];
        let lower = vec![i32::MIN / 4; 2];
        let upper = vec![i32::MAX / 4; 2];
        let plan = Plan::build(
            &model,
            &laws,
            &masks,
            &lower,
            &upper,
            &[0],
            4,
            1,
            Instant::now() + std::time::Duration::from_secs(5),
        )
        .unwrap();
        assert!(!plan.complete);
        assert_eq!(plan.states, 1);
        assert_eq!(&plan.order[..2], &[0, 1]);
        let suffix_prior = plan.fallback_potentials[1];
        assert!(suffix_prior > 0.0 && suffix_prior <= 1.1e-20);
        let root = &plan.nodes[plan.root.unwrap()];
        for o in 0..3 {
            let expected = laws[plan.order[0]].outcomes[o].mid() * suffix_prior;
            if o == 0 {
                assert!(root.weights[o] > 0.0);
                assert!((root.weights[o] / expected - 1.0).abs() < 1e-12);
            } else {
                assert_eq!(root.weights[o], 0.0);
            }
        }
        // With both suffix fixtures fixed to a 1e-20 outcome, the unresolved
        // proposal cap should retain that small native mass rather than
        // treating the unknown suffix as unit potential.
        assert!(
            plan.bound < 1e-30,
            "unexpectedly inflated cap {}",
            plan.bound
        );
        for seed in 0..64 {
            let (_, weight) = plan.draw(&laws, &mut Rng::new(seed)).unwrap();
            assert!(weight.lo <= plan.bound);
        }
    }

    #[test]
    fn fixed_fixture_first_order_preserves_masked_event_mass_and_support() {
        let (model, laws) = tiny_model(3);
        let masks = [7, 4, 7]; // fixture 1 is fixed to a home win
        let lower = [26, i32::MIN / 4];
        let upper = [26, i32::MAX / 4];
        let plan = Plan::build(
            &model,
            &laws,
            &masks,
            &lower,
            &upper,
            &[0],
            4,
            0,
            Instant::now() + std::time::Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(
            plan.order[0], 1,
            "single-support fixture must be processed first"
        );
        let mut estimate = Interval::point(0.0);
        let mut native = Interval::point(0.0);
        let mut q: Vec<[Interval; 3]> = Vec::new();
        for fi in 0..3 {
            let weights: Vec<f64> = (0..3)
                .map(|o| {
                    if masks[fi] & (1 << o) != 0 {
                        laws[fi].outcomes[o].mid()
                    } else {
                        0.0
                    }
                })
                .collect();
            q.push(
                categorical_probabilities(&weights)
                    .unwrap()
                    .try_into()
                    .unwrap(),
            );
        }
        for a in 0..3 {
            for b in 0..3 {
                for c in 0..3 {
                    let os = [a, b, c];
                    let mut q_path = Interval::point(1.0);
                    let mut w_path = Interval::point(1.0);
                    let mut p_path = Interval::point(1.0);
                    let mut valid = true;
                    for fi in 0..3 {
                        let o = os[fi];
                        if masks[fi] & (1 << o) == 0 {
                            valid = false;
                            break;
                        }
                        q_path = q_path.mul(q[fi][o]);
                        p_path = p_path.mul(laws[fi].outcomes[o]);
                        w_path = w_path.mul(laws[fi].outcomes[o].div(q[fi][o]));
                    }
                    if valid && os.iter().filter(|&&o| o == 2).count() == 2 {
                        estimate = estimate.add(q_path.mul(w_path));
                        native = native.add(p_path);
                    }
                }
            }
        }
        assert!(
            estimate.lo <= native.hi && estimate.hi >= native.lo,
            "reordered proposal event mass {:?} does not enclose native {:?}",
            estimate,
            native
        );
        for seed in 0..128 {
            let (outcomes, weight) = plan.draw(&laws, &mut Rng::new(seed)).unwrap();
            assert_eq!(outcomes[1], 2);
            assert!(weight.lo <= plan.bound);
        }
    }

    fn forest_event_mass(
        model: &Model,
        laws: &[Law],
        prefix_fixtures: &[usize],
        prefix: &[u8],
        suffix: &[usize],
        lower: &[i32],
        upper: &[i32],
        selected: &[usize],
        stride: i32,
    ) -> f64 {
        let mut mass = 0.0;
        let mut state = (0..model.n)
            .map(|t| model.base[t].points * stride + model.base[t].wins)
            .collect::<Vec<_>>();
        for (&fi, &o) in prefix_fixtures.iter().zip(prefix) {
            let (hg, ag) = outcome_gain(model, fi, o as usize, stride);
            let f = &model.fixtures[fi];
            for &t in selected {
                if t == f.home {
                    state[t] += hg;
                }
                if t == f.away {
                    state[t] += ag;
                }
            }
        }
        fn rec(
            model: &Model,
            laws: &[Law],
            suffix: &[usize],
            at: usize,
            state: &mut [i32],
            p: f64,
            mass: &mut f64,
            lower: &[i32],
            upper: &[i32],
            selected: &[usize],
            stride: i32,
        ) {
            if at == suffix.len() {
                if selected
                    .iter()
                    .all(|&t| state[t] >= lower[t] && state[t] <= upper[t])
                {
                    *mass += p;
                }
                return;
            }
            let fi = suffix[at];
            let f = &model.fixtures[fi];
            for o in 0..3 {
                let probability = laws[fi].outcomes[o].mid();
                if probability == 0.0 {
                    continue;
                }
                let (hg, ag) = outcome_gain(model, fi, o, stride);
                if selected.contains(&f.home) {
                    state[f.home] += hg;
                }
                if selected.contains(&f.away) {
                    state[f.away] += ag;
                }
                rec(
                    model,
                    laws,
                    suffix,
                    at + 1,
                    state,
                    p * probability,
                    mass,
                    lower,
                    upper,
                    selected,
                    stride,
                );
                if selected.contains(&f.home) {
                    state[f.home] -= hg;
                }
                if selected.contains(&f.away) {
                    state[f.away] -= ag;
                }
            }
        }
        rec(
            model, laws, suffix, 0, &mut state, 1.0, &mut mass, lower, upper, selected, stride,
        );
        mass
    }

    #[test]
    fn forest_roots_share_suffix_states_and_preserve_native_event_mass() {
        let (model, laws) = tiny_model(3);
        let masks = [7; 3];
        let lower = [39, 0];
        let upper = [39, 0];
        let prefixes = vec![vec![2], vec![1], vec![0], vec![2]];
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        let forest = Forest::build(
            &model,
            &laws,
            &masks,
            &lower,
            &upper,
            &[0, 1],
            4,
            &[0],
            &prefixes,
            1000,
            deadline,
        )
        .unwrap();
        assert!(forest.complete);
        assert_eq!(forest.roots.len(), 4);
        let suffix = [1usize, 2];
        for (i, prefix) in prefixes.iter().enumerate() {
            let root = &forest.roots[i];
            let native_prefix = laws[0].outcomes[prefix[0] as usize].mid();
            assert!(root.prefix_mass.lo <= native_prefix && root.prefix_mass.hi >= native_prefix);
            let exact = forest_event_mass(
                &model,
                &laws,
                &[0],
                prefix,
                &suffix,
                &lower,
                &upper,
                &[0, 1],
                4,
            );
            assert!(
                root.potential <= exact + 1e-12 && root.potential + 1e-12 >= exact,
                "root {i}: {} vs {exact}",
                root.potential
            );
            assert_eq!(root.zero, exact == 0.0);
            if !root.zero {
                let (outcomes, weight) = forest
                    .draw_root(i, &laws, &mut Rng::new(61 + i as i64))
                    .unwrap();
                assert_eq!(outcomes[0], prefix[0]);
                assert!(weight.lo <= root.cap);
                assert_eq!(outcomes[1..], [2, 2]);
                let hybrid = forest
                    .draw_root_hybrid(i, &model, &laws, &mut Rng::new(900 + i as i64), true)
                    .unwrap();
                assert_eq!(hybrid.outcomes[0], prefix[0]);
                assert!(hybrid.baseline_weight.lo <= root.cap);
                let baseline = forest
                    .draw_root_hybrid(i, &model, &laws, &mut Rng::new(1200 + i as i64), false)
                    .unwrap();
                let replay = forest.root_guide_ratio(i, &model, &laws, &baseline.outcomes).unwrap();
                assert!((replay.mid() - baseline.guide_over_baseline.mid()).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn forest_shares_memo_across_many_prefix_roots_and_partial_caps_hold() {
        let (model, laws) = tiny_model(4);
        let masks = [7; 4];
        let lower = [52, 0];
        let upper = [52, 0];
        let prefixes = vec![vec![2]; 128];
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        let shared = Forest::build(
            &model,
            &laws,
            &masks,
            &lower,
            &upper,
            &[0, 1],
            4,
            &[0],
            &prefixes,
            5000,
            deadline,
        )
        .unwrap();
        let one = Forest::build(
            &model,
            &laws,
            &masks,
            &lower,
            &upper,
            &[0, 1],
            4,
            &[0],
            &[vec![2]],
            5000,
            deadline,
        )
        .unwrap();
        assert!(shared.complete && one.complete);
        assert_eq!(
            shared.states, one.states,
            "duplicate prefixes should reuse the same suffix nodes"
        );
        assert!(
            shared.states * 64 < one.states * prefixes.len(),
            "forest did not amortize prefix roots"
        );

        let partial = Forest::build(
            &model,
            &laws,
            &masks,
            &lower,
            &upper,
            &[0, 1],
            4,
            &[0],
            &[vec![2]],
            0,
            deadline,
        )
        .unwrap();
        assert!(!partial.complete);
        let root = &partial.roots[0];
        assert!(!root.complete && !root.zero);
        let suffix = [1usize, 2, 3];
        let expected = forest_event_mass(
            &model,
            &laws,
            &[0],
            &[2],
            &suffix,
            &lower,
            &upper,
            &[0, 1],
            4,
        );
        let mut estimate = 0.0;
        for a in 0..3 {
            for b in 0..3 {
                for c in 0..3 {
                    let os = [a, b, c];
                    let mut q = 1.0;
                    let mut w = 1.0;
                    let mut valid = true;
                    for (j, &fi) in suffix.iter().enumerate() {
                        if masks[fi] & (1 << os[j]) == 0 {
                            valid = false;
                            break;
                        }
                        let z: f64 = (0..3).map(|o| laws[fi].outcomes[o].mid()).sum();
                        q *= laws[fi].outcomes[os[j]].mid() / z;
                        w *= z;
                    }
                    if valid && os.iter().all(|&o| o == 2) {
                        estimate += q * w;
                    }
                    if valid {
                        assert!(w <= root.cap * (1.0 + 1e-12));
                    }
                }
            }
        }
        assert!(
            (estimate - expected).abs() < 1e-12,
            "QW={estimate}, native={expected}"
        );
    }

    #[test]
    fn distinct_prefixes_with_equal_selected_state_share_suffix_root() {
        let (model, laws) = tiny_model(4);
        let masks = [7; 4];
        let lower = [26, 26];
        let upper = [26, 26];
        let prefixes = vec![vec![2, 0], vec![0, 2]];
        let forest = Forest::build(
            &model,
            &laws,
            &masks,
            &lower,
            &upper,
            &[0, 1],
            4,
            &[0, 1],
            &prefixes,
            1000,
            Instant::now() + std::time::Duration::from_secs(5),
        )
        .unwrap();
        assert!(forest.complete);
        assert_ne!(
            forest.roots[0].prefix_outcomes,
            forest.roots[1].prefix_outcomes
        );
        assert_eq!(forest.roots[0].node, forest.roots[1].node);
        assert_eq!(forest.roots[0].potential, forest.roots[1].potential);
        for i in 0..2 {
            let exact = forest_event_mass(
                &model,
                &laws,
                &[0, 1],
                &prefixes[i],
                &[2, 3],
                &lower,
                &upper,
                &[0, 1],
                4,
            );
            assert!((forest.roots[i].potential - exact).abs() < 1e-12);
            let (outcomes, weight) = forest
                .draw_root(i, &laws, &mut Rng::new(i as i64 + 3))
                .unwrap();
            assert_eq!(&outcomes[0..2], &prefixes[i]);
            assert!(weight.lo <= forest.roots[i].cap);
        }
    }

    #[test]
    fn partial_forest_internal_nodes_preserve_mass_and_root_caps() {
        fn walk(
            forest: &Forest,
            laws: &[Law],
            pos: usize,
            node_id: Option<usize>,
            outcomes: &mut [u8],
            q_path: Interval,
            w_path: Interval,
            event_qw: &mut Interval,
            max_weight: &mut f64,
        ) -> Result<(), String> {
            let plan = &forest.core;
            if pos == plan.order.len() {
                *max_weight = max_weight.max(w_path.hi);
                if plan.order.iter().all(|&fi| outcomes[fi] == 2) {
                    *event_qw = event_qw.add(q_path.mul(w_path));
                }
                return Ok(());
            }
            let fi = plan.order[pos];
            if let Some(id) = node_id {
                let node = &plan.nodes[id];
                if node.terminal.is_some() {
                    return Ok(());
                }
                if !node.weights.iter().any(|x| *x > 0.) {
                    return Ok(());
                }
                let q = categorical_probabilities(&node.weights)?;
                for edge in &node.edges {
                    let o = edge.outcome as usize;
                    outcomes[fi] = o as u8;
                    let ratio = laws[fi].outcomes[o].div(q[o]);
                    walk(
                        forest,
                        laws,
                        pos + 1,
                        edge.child,
                        outcomes,
                        q_path.mul(q[o]),
                        w_path.mul(ratio),
                        event_qw,
                        max_weight,
                    )?;
                }
            } else {
                let weights: Vec<f64> = (0..3)
                    .map(|o| {
                        if plan.masks[fi] & (1 << o) != 0 {
                            laws[fi].outcomes[o].mid()
                        } else {
                            0.
                        }
                    })
                    .collect();
                let q = categorical_probabilities(&weights)?;
                for o in 0..3 {
                    if weights[o] == 0. {
                        continue;
                    }
                    outcomes[fi] = o as u8;
                    let ratio = laws[fi].outcomes[o].div(q[o]);
                    walk(
                        forest,
                        laws,
                        pos + 1,
                        None,
                        outcomes,
                        q_path.mul(q[o]),
                        w_path.mul(ratio),
                        event_qw,
                        max_weight,
                    )?;
                }
            }
            Ok(())
        }

        let (model, laws) = tiny_model(4);
        let masks = [7; 4];
        let lower = [52, 0];
        let upper = [52, 0];
        let prefix = vec![2];
        let expected = forest_event_mass(
            &model,
            &laws,
            &[0],
            &prefix,
            &[1, 2, 3],
            &lower,
            &upper,
            &[0, 1],
            4,
        );
        for state_cap in 1..=3 {
            let forest = Forest::build(
                &model,
                &laws,
                &masks,
                &lower,
                &upper,
                &[0, 1],
                4,
                &[0],
                &[prefix.clone()],
                state_cap,
                Instant::now() + std::time::Duration::from_secs(5),
            )
            .unwrap();
            let root = &forest.roots[0];
            let mut outcomes = vec![0; laws.len()];
            outcomes[0] = 2;
            let mut event_qw = Interval::point(0.);
            let mut max_weight = 0.;
            walk(
                &forest,
                &laws,
                0,
                root.node,
                &mut outcomes,
                Interval::point(1.),
                Interval::point(1.),
                &mut event_qw,
                &mut max_weight,
            )
            .unwrap();
            assert!(
                event_qw.lo <= expected && event_qw.hi >= expected,
                "cap={state_cap}, QW={event_qw:?}, native={expected}"
            );
            assert!(
                max_weight <= root.cap * (1.0 + 1e-12),
                "cap={state_cap}, max={max_weight}, B={}",
                root.cap
            );
            for seed in 0..16 {
                let (_, weight) = forest.draw_root(0, &laws, &mut Rng::new(seed)).unwrap();
                assert!(weight.lo <= root.cap);
            }
        }
    }
}
