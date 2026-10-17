//! Bounded deterministic audit of the residual left by guided clipping.
//!
//! The search explores non-overlapping proposal prefixes.  Its unresolved
//! bounds use only the native likelihood cap and a one-team PW upper bound;
//! guided likelihood is used only at complete, feasible paths.

use super::{categorical_probabilities, outcome_gain, Edge, Forest, Interval, Law, Plan};
use golaberto_odds::model::Model;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BinaryHeap, HashMap},
    time::{Duration, Instant},
};

#[path = "residual/suffix.rs"]
mod suffix;
pub use suffix::{SuffixDiagnostics, SuffixOptions};

#[derive(Clone, Copy)]
enum Graph<'a> {
    Plan(&'a Plan),
    Forest(&'a Plan),
}

impl<'a> Graph<'a> {
    fn plan(self) -> &'a Plan {
        match self {
            Self::Plan(p) | Self::Forest(p) => p,
        }
    }
}

/// One baseline component entering the joint residual audit.
pub struct ResidualSource<'a> {
    graph: Graph<'a>,
    node: Option<usize>,
    totals: Vec<i32>,
    v_prefix: Interval,
    g_prefix: Interval,
    q0_prefix: Interval,
    zero: bool,
}

#[derive(Clone, Debug)]
pub struct ResidualSnapshot {
    pub expanded: usize,
    pub frontier: usize,
    pub expectation_upper: f64,
    pub max_residual_cap: f64,
}

#[derive(Clone, Debug)]
pub struct ResidualDepthStat {
    pub depth: usize,
    pub branches: usize,
    pub expectation_upper: f64,
}

#[derive(Clone, Debug)]
pub struct ResidualPendingBranch {
    pub source: usize,
    pub depth: usize,
    pub expectation_upper: f64,
    pub residual_cap: f64,
    pub remaining_touched_teams: usize,
}

/// Certified upper bound and deterministic work report.
#[derive(Clone, Debug)]
pub struct ResidualCertificate {
    pub expectation_upper: f64,
    pub max_residual_cap: f64,
    pub initial_bounds: f64,
    pub expanded: usize,
    pub frontier: usize,
    pub prunes: usize,
    pub leaves: usize,
    pub zero_pw_prunes: usize,
    pub zero_residual_prunes: usize,
    pub zero_residual_leaves: usize,
    pub feasible_leaves: usize,
    pub source_count: usize,
    pub frontier_min_depth: usize,
    pub frontier_max_depth: usize,
    pub elapsed: Duration,
    pub memory_limited: bool,
    pub pmf_limited: bool,
    pub complete: bool,
    pub snapshots: Vec<ResidualSnapshot>,
    pub depth_stats: Vec<ResidualDepthStat>,
    pub top_pending: Vec<ResidualPendingBranch>,
    pub suffix: SuffixDiagnostics,
}

impl Plan {
    pub fn residual_source(
        &self,
        native_factor: Interval,
        mixture_probability: Interval,
    ) -> Result<ResidualSource<'_>, String> {
        make_source(
            Graph::Plan(self),
            self.root,
            self.base.clone(),
            native_factor,
            mixture_probability,
            Interval::point(1.0),
            self.bound == 0.0,
        )
    }
}

impl Forest {
    pub fn residual_source(
        &self,
        root_index: usize,
        model: &Model,
        native_factor: Interval,
        mixture_probability: Interval,
    ) -> Result<ResidualSource<'_>, String> {
        let root = self
            .roots
            .get(root_index)
            .ok_or("forest root index out of range")?;
        let mut totals = self.core.base.clone();
        for (&fi, &o) in self.prefix_fixtures.iter().zip(&root.prefix_outcomes) {
            let (hg, ag) = outcome_gain(model, fi, o as usize, self.core.stride);
            let f = &model.fixtures[fi];
            totals[f.home] = totals[f.home].saturating_add(hg);
            totals[f.away] = totals[f.away].saturating_add(ag);
        }
        make_source(
            Graph::Forest(&self.core),
            if root.node == Some(usize::MAX) {
                None
            } else {
                root.node
            },
            totals,
            native_factor,
            mixture_probability,
            root.prefix_mass,
            root.zero,
        )
    }
}

fn make_source<'a>(
    graph: Graph<'a>,
    node: Option<usize>,
    totals: Vec<i32>,
    native_factor: Interval,
    mixture_probability: Interval,
    prefix_mass: Interval,
    zero: bool,
) -> Result<ResidualSource<'a>, String> {
    if !native_factor.lo.is_finite()
        || !native_factor.hi.is_finite()
        || !mixture_probability.lo.is_finite()
        || !mixture_probability.hi.is_finite()
        || !prefix_mass.lo.is_finite()
        || !prefix_mass.hi.is_finite()
        || native_factor.lo < 0.0
        || native_factor.lo > native_factor.hi
        || native_factor.hi > 1.0
        || mixture_probability.lo <= 0.0
        || mixture_probability.lo > mixture_probability.hi
        || mixture_probability.hi > 1.0
        || prefix_mass.lo < 0.0
        || prefix_mass.lo > prefix_mass.hi
        || prefix_mass.hi > 1.0
    {
        return Err("residual source has an invalid likelihood factor".into());
    }
    let v_prefix = widen_if_invalid(prefix_mass.mul(native_factor).div(mixture_probability));
    Ok(ResidualSource {
        graph,
        node,
        totals,
        v_prefix,
        g_prefix: Interval::point(1.0),
        q0_prefix: mixture_probability,
        zero,
    })
}

fn widen_if_invalid(x: Interval) -> Interval {
    if x.lo.is_finite() && x.hi.is_finite() && x.lo >= 0.0 && x.lo <= x.hi {
        x
    } else {
        // A lost enclosure can only make the audit looser: V gets no finite
        // upper cap and G gets no positive lower bound.
        Interval {
            lo: 0.0,
            hi: f64::INFINITY,
        }
    }
}

#[derive(Clone)]
struct Branch {
    source: usize,
    node: Option<usize>,
    pos: usize,
    totals: Vec<i32>,
    v: Interval,
    g: Interval,
    q0: Interval,
    bound: f64,
    residual_cap: f64,
    depth: usize,
    serial: u64,
    suffix_probed: bool,
}

impl PartialEq for Branch {
    fn eq(&self, rhs: &Self) -> bool {
        self.serial == rhs.serial
    }
}
impl Eq for Branch {}
impl PartialOrd for Branch {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}
impl Ord for Branch {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.bound
            .total_cmp(&rhs.bound)
            .then_with(|| rhs.serial.cmp(&self.serial))
    }
}

fn up(x: f64) -> f64 {
    if x <= 0.0 || !x.is_finite() {
        x.max(0.0)
    } else {
        f64::from_bits(x.to_bits() + 1)
    }
}

fn clipped_guide_lower(prefix: Interval, suffix: f64, clip_h: f64) -> f64 {
    let guide = widen_if_invalid(prefix.mul(Interval::point(suffix)));
    widen_if_invalid(Interval::point(clip_h).mul(Interval::point(guide.lo))).lo
}

#[derive(Clone)]
struct TeamSuffix {
    pmf: Vec<(i32, f64)>,
    unrelated: f64,
}

#[derive(Default)]
struct NativePmfCache {
    rows: HashMap<(usize, usize, usize), Option<TeamSuffix>>,
    states: usize,
    limited: bool,
}

impl NativePmfCache {
    fn team_suffix(
        &mut self,
        plan: &Plan,
        model: &Model,
        laws: &[Law],
        team: usize,
        from: usize,
        deadline: Instant,
    ) -> Option<TeamSuffix> {
        let key = (plan as *const Plan as usize, team, from);
        if let Some(row) = self.rows.get(&key) {
            return row.clone();
        }
        let mut pmf = BTreeMap::<i32, f64>::from([(0, 1.0)]);
        let mut unrelated = Interval::point(1.0);
        for pos in from..plan.order.len() {
            if Instant::now() >= deadline {
                self.limited = true;
                self.rows.insert(key, None);
                return None;
            }
            let fi = plan.order[pos];
            let f = &model.fixtures[fi];
            if f.home != team && f.away != team {
                let mut m = Interval::point(0.0);
                for o in 0..3 {
                    if plan.masks[fi] & (1 << o) != 0 {
                        m = m.add(laws[fi].outcomes[o]);
                    }
                }
                unrelated = unrelated.mul(Interval {
                    lo: 0.0,
                    hi: m.hi.min(1.0),
                });
                continue;
            }
            let mut next = BTreeMap::<i32, f64>::new();
            for (&old, &mass) in &pmf {
                for o in 0..3 {
                    if plan.masks[fi] & (1 << o) == 0 {
                        continue;
                    }
                    let p = laws[fi].outcomes[o].hi;
                    if p <= 0.0 {
                        continue;
                    }
                    let (hg, ag) = outcome_gain(model, fi, o, plan.stride);
                    let gain = if f.home == team { hg } else { ag };
                    let value = Interval::point(mass)
                        .mul(Interval::point(p))
                        .add(Interval::point(
                            *next.get(&old.saturating_add(gain)).unwrap_or(&0.0),
                        ))
                        .hi;
                    next.insert(old.saturating_add(gain), value);
                }
            }
            if next.len() > 100_000 {
                self.limited = true;
                self.rows.insert(key, None);
                return None;
            }
            pmf = next;
        }
        if self.states.saturating_add(pmf.len()) > 500_000 {
            self.limited = true;
            self.rows.insert(key, None);
            return None;
        }
        self.states += pmf.len();
        let row = TeamSuffix {
            pmf: pmf.into_iter().collect(),
            unrelated: unrelated.hi,
        };
        self.rows.insert(key, Some(row.clone()));
        Some(row)
    }
}

fn team_event_upper(
    plan: &Plan,
    model: &Model,
    laws: &[Law],
    team: usize,
    from: usize,
    score: i32,
    cache: &mut NativePmfCache,
    deadline: Instant,
) -> Result<f64, String> {
    let Some(row) = cache.team_suffix(plan, model, laws, team, from, deadline) else {
        return Ok(1.0);
    };
    let allowed = plan.lower[team]..=plan.upper[team];
    let event = row
        .pmf
        .iter()
        .filter(|(gain, _)| allowed.contains(&score.saturating_add(*gain)))
        .fold(0.0, |s, (_, p)| {
            Interval::point(s).add(Interval::point(*p)).hi
        });
    Ok(Interval::point(row.unrelated)
        .mul(Interval::point(event))
        .hi
        .min(1.0))
}

fn suffix_event_upper(
    plan: &Plan,
    model: &Model,
    laws: &[Law],
    totals: &[i32],
    from: usize,
    cache: &mut NativePmfCache,
    deadline: Instant,
) -> Result<f64, String> {
    // Cheap all-team extrema prove impossible PW intervals even if the exact
    // marginal PMF cache later hits its work or memory guard.
    let mut min_gain = vec![0i32; model.n];
    let mut max_gain = vec![0i32; model.n];
    for pos in from..plan.order.len() {
        let fi = plan.order[pos];
        let f = &model.fixtures[fi];
        for team in [f.home, f.away] {
            let mut lo = i32::MAX;
            let mut hi = i32::MIN;
            for o in 0..3 {
                if plan.masks[fi] & (1 << o) == 0 || laws[fi].outcomes[o].hi <= 0.0 {
                    continue;
                }
                let (hg, ag) = outcome_gain(model, fi, o, plan.stride);
                let gain = if team == f.home { hg } else { ag };
                lo = lo.min(gain);
                hi = hi.max(gain);
            }
            if lo != i32::MAX {
                min_gain[team] = min_gain[team].saturating_add(lo);
                max_gain[team] = max_gain[team].saturating_add(hi);
            }
        }
    }
    for team in 0..model.n {
        if totals[team].saturating_add(min_gain[team]) > plan.upper[team]
            || totals[team].saturating_add(max_gain[team]) < plan.lower[team]
        {
            return Ok(0.0);
        }
    }
    let mut upper = 1.0f64;
    for team in 0..model.n {
        if plan.lower[team] > plan.upper[team] {
            return Ok(0.0);
        }
        upper = upper.min(team_event_upper(
            plan,
            model,
            laws,
            team,
            from,
            totals[team],
            cache,
            deadline,
        )?);
        if upper == 0.0 {
            break;
        }
    }
    Ok(upper)
}

fn branch_bounds(
    branch: &Branch,
    source: &ResidualSource<'_>,
    model: &Model,
    laws: &[Law],
    b: f64,
    clip_h: f64,
    cache: &mut NativePmfCache,
    suffix_cache: Option<&mut suffix::Cache>,
    deadline: Instant,
) -> Result<(f64, f64, bool), String> {
    let plan = source.graph.plan();
    let suffix_cap = branch
        .node
        .map_or_else(|| plan.fallback_caps[branch.pos], |id| plan.nodes[id].cap);
    let v_cap = branch.v.mul(Interval::point(suffix_cap)).hi.min(b).max(0.0);
    let mut residual_cap = v_cap;
    let mut suffix_cache = suffix_cache;
    if let Some(cache) = suffix_cache.as_deref_mut() {
        match suffix::cached(
            source,
            branch.pos,
            branch.node,
            &branch.totals,
            model,
            laws,
            cache,
        )? {
            suffix::Bound::Impossible => return Ok((0.0, 0.0, true)),
            suffix::Bound::Feasible(g_future) => {
                let clipped = clipped_guide_lower(branch.g, g_future, clip_h);
                residual_cap = if v_cap <= clipped {
                    0.0
                } else {
                    up(v_cap - clipped).min(b)
                };
                if residual_cap == 0.0 {
                    cache.record_guide_zero();
                    cache.record_guide_prune();
                }
            }
            suffix::Bound::Unknown => {}
        }
    }
    let event = if Instant::now() >= deadline {
        1.0
    } else {
        suffix_event_upper(
            plan,
            model,
            laws,
            &branch.totals,
            branch.pos,
            cache,
            deadline,
        )?
    };
    let native = branch.q0.mul(branch.v).mul(Interval::point(event)).hi;
    let mass_bound = native.min(branch.q0.mul(Interval::point(residual_cap)).hi);
    Ok((mass_bound.max(0.0), residual_cap, event == 0.0))
}

fn selected_feasible(plan: &Plan, totals: &[i32]) -> bool {
    (0..plan.base.len()).all(|t| totals[t] >= plan.lower[t] && totals[t] <= plan.upper[t])
}

/// Search the largest residual branches first, retaining every unresolved
/// branch when either the time or expansion budget is exhausted.
pub fn residual_certificate(
    sources: &[ResidualSource<'_>],
    model: &Model,
    laws: &[Law],
    cap_b: f64,
    clip_h: f64,
    node_limit: usize,
    deadline: Instant,
) -> Result<ResidualCertificate, String> {
    residual_certificate_with_options(
        sources, model, laws, cap_b, clip_h, node_limit, deadline, None,
    )
}

pub fn residual_certificate_with_options(
    sources: &[ResidualSource<'_>],
    model: &Model,
    laws: &[Law],
    cap_b: f64,
    clip_h: f64,
    node_limit: usize,
    deadline: Instant,
    suffix_options: Option<SuffixOptions>,
) -> Result<ResidualCertificate, String> {
    let started = Instant::now();
    if laws.len() != model.fixtures.len()
        || !cap_b.is_finite()
        || cap_b <= 0.0
        || !clip_h.is_finite()
        || clip_h < 0.0
    {
        return Err("invalid residual certificate dimensions or caps".into());
    }
    let mut heap = BinaryHeap::new();
    let mut serial = 0u64;
    let mut initial_bounds = 0.0;
    let mut initial_max: f64 = 0.0;
    let mut snapshots = Vec::new();
    let mut pmf_cache = NativePmfCache::default();
    let mut suffix_cache = suffix_options
        .map(|options| suffix::Cache::new(options, started, deadline))
        .transpose()?;
    let mut prunes = 0usize;
    let mut zero_pw_prunes = 0usize;
    let mut zero_residual_prunes = 0usize;
    for (si, source) in sources.iter().enumerate() {
        let plan = source.graph.plan();
        if plan.masks.len() != laws.len() || plan.base.len() != model.n {
            return Err("residual source dimensions do not match".into());
        }
        if source.zero {
            continue;
        }
        let mut br = Branch {
            source: si,
            node: source.node,
            pos: 0,
            totals: source.totals.clone(),
            v: source.v_prefix,
            g: source.g_prefix,
            q0: source.q0_prefix,
            bound: 0.0,
            residual_cap: 0.0,
            depth: 0,
            serial,
            suffix_probed: false,
        };
        let (bound, residual_cap, pwzero) = branch_bounds(
            &br,
            source,
            model,
            laws,
            cap_b,
            clip_h,
            &mut pmf_cache,
            suffix_cache.as_mut(),
            deadline,
        )?;
        br.bound = bound;
        br.residual_cap = residual_cap;
        initial_bounds = up(initial_bounds + bound);
        initial_max = initial_max.max(residual_cap);
        if bound > 0.0 {
            heap.push(br);
        } else {
            prunes += 1;
            if pwzero {
                zero_pw_prunes += 1;
            } else if residual_cap == 0.0 {
                zero_residual_prunes += 1;
            }
        }
        serial += 1;
    }
    let initial_bounds = initial_bounds.min(cap_b).max(0.0);
    let mut leaves = 0usize;
    let mut feasible_leaves = 0usize;
    let mut zero_residual_leaves = 0usize;
    let mut expanded = 0usize;
    let mut memory_limited = false;
    // A finite guard protects memory even when a high-fanout tree defeats the
    // expansion budget's usual relationship to frontier size.
    let frontier_limit = node_limit.max(sources.len()).max(1);
    let mut completed_bound = 0.0f64;
    let mut completed_max = 0.0f64;
    while let Some(branch) = heap.pop() {
        if expanded >= node_limit || Instant::now() >= deadline {
            heap.push(branch);
            break;
        }
        let source = &sources[branch.source];
        let plan = source.graph.plan();
        let mut branch = branch;
        if !branch.suffix_probed {
            if let Some(cache) = suffix_cache.as_mut() {
                let remaining = plan.order.len().saturating_sub(branch.pos);
                if remaining <= cache.max_remaining() {
                    let bound = suffix::minimum(
                        source,
                        branch.pos,
                        branch.node,
                        &branch.totals,
                        model,
                        laws,
                        cache,
                        deadline,
                    )?;
                    branch.suffix_probed = true;
                    if !matches!(bound, suffix::Bound::Unknown) {
                        let (new_bound, residual_cap, pwzero) = branch_bounds(
                            &branch,
                            source,
                            model,
                            laws,
                            cap_b,
                            clip_h,
                            &mut pmf_cache,
                            Some(cache),
                            deadline,
                        )?;
                        if new_bound == 0.0 {
                            prunes += 1;
                            if pwzero {
                                zero_pw_prunes += 1;
                            } else if residual_cap == 0.0 {
                                zero_residual_prunes += 1;
                            }
                            expanded += 1;
                            continue;
                        }
                        if new_bound < branch.bound {
                            cache.record_tightened();
                            branch.bound = new_bound;
                            branch.residual_cap = branch.residual_cap.min(residual_cap);
                            heap.push(branch);
                            continue;
                        }
                        branch.bound = branch.bound.min(new_bound);
                        branch.residual_cap = branch.residual_cap.min(residual_cap);
                    }
                }
            }
        }
        if branch.pos == plan.order.len() {
            leaves += 1;
            if selected_feasible(plan, &branch.totals) {
                feasible_leaves += 1;
                let hg_lower = Interval::point(clip_h).mul(Interval::point(branch.g.lo)).lo;
                let residual = if branch.v.hi <= hg_lower {
                    0.0
                } else {
                    up(branch.v.hi - hg_lower).min(cap_b)
                };
                if residual == 0.0 {
                    zero_residual_leaves += 1;
                }
                completed_bound = Interval::point(completed_bound)
                    .add(Interval::point(branch.q0.hi).mul(Interval::point(residual)))
                    .hi;
                completed_max = completed_max.max(residual);
            } else {
                zero_pw_prunes += 1;
                prunes += 1;
            }
            expanded += 1;
            continue;
        }
        let fi = plan.order[branch.pos];
        let (base, edges): (Vec<f64>, Vec<Edge>) = if let Some(id) = branch.node {
            let node = plan.nodes.get(id).ok_or("residual search node missing")?;
            (node.weights.to_vec(), node.edges.clone())
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
                Vec::new(),
            )
        };
        let q = categorical_probabilities(&base)?;
        let guided = plan.guided_weights(model, laws, fi, branch.pos, &branch.totals, &base)?;
        let gq = categorical_probabilities(&guided)?;
        let mut children = Vec::new();
        for o in 0..3 {
            if q[o].hi <= 0.0 || laws[fi].outcomes[o].hi <= 0.0 {
                continue;
            }
            if branch.node.is_some() && !edges.iter().any(|e| e.outcome as usize == o) {
                continue;
            }
            let edge = edges.iter().find(|e| e.outcome as usize == o);
            let (hg, ag) = outcome_gain(model, fi, o, plan.stride);
            let f = &model.fixtures[fi];
            let mut totals = branch.totals.clone();
            totals[f.home] = totals[f.home].saturating_add(hg);
            totals[f.away] = totals[f.away].saturating_add(ag);
            let mut child = Branch {
                source: branch.source,
                node: if branch.node.is_some() {
                    edge.and_then(|e| e.child)
                } else {
                    None
                },
                pos: branch.pos + 1,
                totals,
                v: widen_if_invalid(branch.v.mul(laws[fi].outcomes[o].div(q[o]))),
                g: widen_if_invalid(branch.g.mul(gq[o].div(q[o]))),
                q0: branch.q0.mul(q[o]),
                bound: 0.0,
                residual_cap: 0.0,
                depth: branch.depth + 1,
                serial,
                suffix_probed: false,
            };
            serial += 1;
            let (bound, residual_cap, pwzero) = branch_bounds(
                &child,
                source,
                model,
                laws,
                cap_b,
                clip_h,
                &mut pmf_cache,
                suffix_cache.as_mut(),
                deadline,
            )?;
            child.bound = bound;
            child.residual_cap = residual_cap;
            if bound == 0.0 {
                prunes += 1;
                if pwzero {
                    zero_pw_prunes += 1;
                } else if residual_cap == 0.0 {
                    zero_residual_prunes += 1;
                }
            } else {
                children.push(child);
            }
        }
        // Commit children as one operation, so the popped branch is never
        // lost from the retained frontier if interruption is observed.
        for child in children {
            heap.push(child);
        }
        expanded += 1;
        if heap.len() > frontier_limit {
            memory_limited = true;
            break;
        }
        if expanded == 1_000 || expanded == 10_000 || expanded == 100_000 {
            let mut frontier_bound = completed_bound;
            for pending in &heap {
                frontier_bound = up(frontier_bound + pending.bound);
            }
            snapshots.push(ResidualSnapshot {
                expanded,
                frontier: heap.len(),
                expectation_upper: frontier_bound.min(initial_bounds),
                max_residual_cap: heap
                    .iter()
                    .map(|x| x.residual_cap)
                    .fold(completed_max, f64::max),
            });
        }
    }
    if let Some(cache) = suffix_cache.as_mut() {
        let mut refreshed = Vec::with_capacity(heap.len());
        for mut branch in std::mem::take(&mut heap).into_vec() {
            let source = &sources[branch.source];
            match suffix::cached(
                source,
                branch.pos,
                branch.node,
                &branch.totals,
                model,
                laws,
                cache,
            )? {
                suffix::Bound::Impossible => {
                    prunes += 1;
                    zero_pw_prunes += 1;
                    continue;
                }
                suffix::Bound::Feasible(g_future) => {
                    let plan = source.graph.plan();
                    let suffix_cap = branch
                        .node
                        .map_or_else(|| plan.fallback_caps[branch.pos], |id| plan.nodes[id].cap);
                    let v_cap = branch
                        .v
                        .mul(Interval::point(suffix_cap))
                        .hi
                        .min(cap_b)
                        .max(0.0);
                    let clipped = clipped_guide_lower(branch.g, g_future, clip_h);
                    let residual_cap = if v_cap <= clipped {
                        0.0
                    } else {
                        up(v_cap - clipped).min(cap_b)
                    };
                    if residual_cap == 0.0 {
                        cache.record_guide_zero();
                        cache.record_guide_prune();
                    }
                    branch.bound = branch
                        .bound
                        .min(branch.q0.mul(Interval::point(residual_cap)).hi);
                    branch.residual_cap = branch.residual_cap.min(residual_cap);
                }
                suffix::Bound::Unknown => {}
            }
            if branch.bound == 0.0 {
                prunes += 1;
                if branch.residual_cap == 0.0 {
                    zero_residual_prunes += 1;
                }
            } else {
                refreshed.push(branch);
            }
        }
        heap = BinaryHeap::from(refreshed);
    }
    let mut expectation_upper = completed_bound;
    let mut max_residual_cap = completed_max;
    let mut min_depth = usize::MAX;
    let mut max_depth = 0usize;
    let mut depths = BTreeMap::<usize, (usize, f64)>::new();
    let mut top_pending = Vec::new();
    for branch in &heap {
        expectation_upper = up(expectation_upper + branch.bound);
        max_residual_cap = max_residual_cap.max(branch.residual_cap);
        min_depth = min_depth.min(branch.depth);
        max_depth = max_depth.max(branch.depth);
        let entry = depths.entry(branch.depth).or_default();
        entry.0 += 1;
        entry.1 = up(entry.1 + branch.bound);
        let plan = sources[branch.source].graph.plan();
        let mut touched = vec![false; model.n];
        for &fi in plan.order.iter().skip(branch.pos) {
            touched[model.fixtures[fi].home] = true;
            touched[model.fixtures[fi].away] = true;
        }
        top_pending.push(ResidualPendingBranch {
            source: branch.source,
            depth: branch.depth,
            expectation_upper: branch.bound,
            residual_cap: branch.residual_cap,
            remaining_touched_teams: touched.iter().filter(|&&v| v).count(),
        });
    }
    top_pending.sort_by(|a, b| {
        b.expectation_upper
            .total_cmp(&a.expectation_upper)
            .then_with(|| a.source.cmp(&b.source))
            .then_with(|| a.depth.cmp(&b.depth))
    });
    top_pending.truncate(5);
    expectation_upper = expectation_upper.min(initial_bounds).min(cap_b).max(0.0);
    max_residual_cap = max_residual_cap.min(initial_max).min(cap_b);
    let complete = heap.is_empty();
    let suffix_diagnostics = suffix_cache
        .as_mut()
        .map(|c| {
            c.finish();
            c.diagnostics()
        })
        .unwrap_or_default();
    Ok(ResidualCertificate {
        expectation_upper,
        max_residual_cap,
        initial_bounds,
        expanded,
        frontier: heap.len(),
        prunes,
        leaves,
        zero_pw_prunes,
        zero_residual_prunes,
        zero_residual_leaves,
        feasible_leaves,
        source_count: sources.len(),
        frontier_min_depth: if heap.is_empty() { 0 } else { min_depth },
        frontier_max_depth: max_depth,
        elapsed: started.elapsed(),
        memory_limited,
        pmf_limited: pmf_cache.limited,
        complete,
        snapshots,
        depth_stats: depths
            .into_iter()
            .map(|(depth, (branches, expectation_upper))| ResidualDepthStat {
                depth,
                branches,
                expectation_upper,
            })
            .collect(),
        top_pending,
        suffix: suffix_diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use golaberto_odds::model::Request;
    use serde_json::json;

    fn tiny(games: usize) -> (Model, Vec<Law>) {
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

    fn broad_bounds() -> (Vec<i32>, Vec<i32>) {
        (vec![i32::MIN / 4; 2], vec![i32::MAX / 4; 2])
    }

    #[test]
    fn complete_search_encloses_exhaustive_tiny_residual_sum() {
        let (model, laws) = tiny(2);
        let (lower, upper) = broad_bounds();
        let plan = Plan::build(
            &model,
            &laws,
            &[7, 7],
            &lower,
            &upper,
            &[],
            4,
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let source = plan
            .residual_source(Interval::point(1.0), Interval::point(1.0))
            .unwrap();
        let cert = residual_certificate(
            &[source],
            &model,
            &laws,
            10.0,
            0.4,
            10_000,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        assert!(cert.complete);
        let mut exact_upper = 0.0;
        for a in 0..3 {
            for b in 0..3 {
                let mut q0 = 1.0;
                let mut v = Interval::point(1.0);
                for (fi, o) in [(0, a), (1, b)] {
                    let w: Vec<f64> = (0..3).map(|x| laws[fi].outcomes[x].mid()).collect();
                    let q = categorical_probabilities(&w).unwrap()[o];
                    q0 *= q.mid();
                    v = v.mul(laws[fi].outcomes[o].div(q));
                }
                let replay = plan
                    .guide_ratio(&model, &laws, &[a as u8, b as u8])
                    .unwrap();
                let residual =
                    (v.hi - Interval::point(0.4).mul(Interval::point(replay.lo)).lo).max(0.0);
                exact_upper += q0 * residual;
            }
        }
        assert!(
            cert.expectation_upper + 1e-12 >= exact_upper,
            "certificate {} below enumerated upper {exact_upper}",
            cert.expectation_upper
        );
        assert!(cert.expectation_upper - exact_upper < 1e-10);
        assert_eq!(cert.leaves, 9);
    }

    #[test]
    fn zero_deadline_retains_every_source_and_node_cap_is_bounded() {
        let (model, laws) = tiny(2);
        let (lower, upper) = broad_bounds();
        let plan = Plan::build(
            &model,
            &laws,
            &[7, 7],
            &lower,
            &upper,
            &[],
            4,
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let a = plan
            .residual_source(Interval::point(1.0), Interval::point(0.4))
            .unwrap();
        let b = plan
            .residual_source(Interval::point(1.0), Interval::point(0.6))
            .unwrap();
        let cert = residual_certificate(
            &[a, b],
            &model,
            &laws,
            10.0,
            0.4,
            100,
            Instant::now() - Duration::from_secs(1),
        )
        .unwrap();
        assert!(!cert.complete);
        assert_eq!(cert.frontier, 2);
        assert_eq!(cert.expanded, 0);
        let a = plan
            .residual_source(Interval::point(1.0), Interval::point(0.4))
            .unwrap();
        let b = plan
            .residual_source(Interval::point(1.0), Interval::point(0.6))
            .unwrap();
        let capped = residual_certificate(
            &[a, b],
            &model,
            &laws,
            10.0,
            0.4,
            0,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(capped.expanded, 0);
        assert_eq!(capped.frontier, 2);
        assert!(capped.expectation_upper <= capped.initial_bounds);
    }

    #[test]
    fn unselected_team_interval_is_checked_using_current_totals() {
        let (model, laws) = tiny(2);
        let lower = [i32::MIN / 4, 100_000];
        let upper = [i32::MAX / 4, 100_001];
        let plan = Plan::build(
            &model,
            &laws,
            &[7, 7],
            &lower,
            &upper,
            &[0],
            4,
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let source = plan
            .residual_source(Interval::point(1.0), Interval::point(1.0))
            .unwrap();
        let cert = residual_certificate(
            &[source],
            &model,
            &laws,
            10.0,
            0.4,
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(cert.expectation_upper, 0.0);
        assert!(cert.zero_pw_prunes > 0);
    }

    #[test]
    fn forest_prefix_score_is_carried_into_unselected_team_bound() {
        let (model, laws) = tiny(2);
        let lower = [i32::MIN / 4, 26];
        let upper = [i32::MAX / 4, 26];
        let forest = Forest::build(
            &model,
            &laws,
            &[7, 7],
            &lower,
            &upper,
            &[0],
            4,
            &[0],
            &[vec![0]],
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        assert!(!forest.roots[0].zero);
        let source = forest
            .residual_source(0, &model, Interval::point(1.0), Interval::point(1.0))
            .unwrap();
        let cert = residual_certificate(
            &[source],
            &model,
            &laws,
            10.0,
            1e-6,
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        assert!(cert.expectation_upper > 0.0);
        assert!(cert.feasible_leaves > 0);
        assert_eq!(cert.source_count, 1);
    }

    #[test]
    fn partial_dag_fallback_bounds_are_retained_after_one_expansion() {
        let (model, laws) = tiny(3);
        let lower = [13, i32::MIN / 4];
        let upper = [i32::MAX / 4; 2];
        let plan = Plan::build(
            &model,
            &laws,
            &[7; 3],
            &lower,
            &upper,
            &[0],
            4,
            1,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        assert!(!plan.complete);
        let source = plan
            .residual_source(Interval::point(1.0), Interval::point(1.0))
            .unwrap();
        let cert = residual_certificate(
            &[source],
            &model,
            &laws,
            10.0,
            0.4,
            1,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        assert!(!cert.complete);
        assert_eq!(cert.expanded, 1);
        assert!(cert.frontier > 0);
        assert!(cert.expectation_upper <= cert.initial_bounds);
    }

    #[test]
    fn cached_suffix_minimum_can_prove_zero_before_outer_leaf() {
        let (model, laws) = tiny(1);
        let (lower, upper) = (vec![13, i32::MIN / 4], vec![i32::MAX / 4; 2]);
        let plan = Plan::build(
            &model,
            &laws,
            &[7],
            &lower,
            &upper,
            &[],
            4,
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let source = plan
            .residual_source(Interval::point(0.3), Interval::point(1.0))
            .unwrap();
        let mut options = SuffixOptions::default();
        options.max_remaining = 3;
        let cert = residual_certificate_with_options(
            &[source],
            &model,
            &laws,
            1.0,
            0.5,
            100,
            Instant::now() + Duration::from_secs(2),
            Some(options),
        )
        .unwrap();
        assert_eq!(cert.leaves, 0);
        assert_eq!(cert.expectation_upper, 0.0);
        assert!(cert.suffix.completed_rows > 0);
        assert!(cert.suffix.guide_zero_prunes > 0);
    }

    #[test]
    fn suffix_probe_timeout_keeps_unknown_branch_mass() {
        let (model, laws) = tiny(2);
        let (lower, upper) = broad_bounds();
        let plan = Plan::build(
            &model,
            &laws,
            &[7; 2],
            &lower,
            &upper,
            &[],
            4,
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let source = plan
            .residual_source(Interval::point(1.0), Interval::point(1.0))
            .unwrap();
        let mut options = SuffixOptions::default();
        options.max_remaining = 3;
        options.probe_nodes = 1;
        let cert = residual_certificate_with_options(
            &[source],
            &model,
            &laws,
            2.0,
            0.9,
            1,
            Instant::now() + Duration::from_secs(2),
            Some(options),
        )
        .unwrap();
        assert!(cert.expectation_upper > 0.0);
        assert!(cert.frontier > 0);
        assert!(cert.suffix.probe_node_limit_hit || cert.suffix.unknown > 0);
    }

    #[test]
    fn conditioned_suffix_minimum_is_below_exhaustive_replay_minimum() {
        let (model, laws) = tiny(2);
        let (lower, upper) = (vec![13, i32::MIN / 4], vec![i32::MAX / 4; 2]);
        let plan = Plan::build(
            &model,
            &laws,
            &[7; 2],
            &lower,
            &upper,
            &[1],
            4,
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let source = plan
            .residual_source(Interval::point(0.7), Interval::point(0.4))
            .unwrap();
        let now = Instant::now();
        let mut cache =
            suffix::Cache::new(SuffixOptions::default(), now, now + Duration::from_secs(2))
                .unwrap();
        let observed = suffix::minimum(
            &source,
            0,
            source.node,
            &source.totals,
            &model,
            &laws,
            &mut cache,
            now + Duration::from_secs(2),
        )
        .unwrap();
        let suffix::Bound::Feasible(lower_g) = observed else {
            panic!("expected solved feasible suffix")
        };
        let mut exact_min = f64::INFINITY;
        let mut replay_ratios = Vec::new();
        for a in 0..3 {
            for b in 0..3 {
                let outcomes = [a as u8, b as u8];
                let mut totals = plan.base.clone();
                for (fi, &o) in outcomes.iter().enumerate() {
                    let (hg, ag) = outcome_gain(&model, fi, o as usize, plan.stride);
                    let f = &model.fixtures[fi];
                    totals[f.home] += hg;
                    totals[f.away] += ag;
                }
                if !selected_feasible(&plan, &totals) {
                    continue;
                }
                let ratio = plan.guide_ratio(&model, &laws, &outcomes).unwrap().lo;
                exact_min = exact_min.min(ratio);
                replay_ratios.push(ratio);
            }
        }
        assert!(exact_min.is_finite());
        let ratio_min = replay_ratios.iter().copied().fold(f64::INFINITY, f64::min);
        let ratio_max = replay_ratios.iter().copied().fold(0.0f64, f64::max);
        assert!(
            ratio_max - ratio_min > 1e-6,
            "fixture must exercise nontrivial guidance: {replay_ratios:?}"
        );
        assert!(lower_g <= exact_min);
        assert!(
            exact_min - lower_g < 1e-10,
            "cached {lower_g}, replay {exact_min}"
        );

        // Reuse the solved suffix with a different atom's prefix factors, then
        // ensure a finalized-invalid state is still rejected before cache hit.
        let other = plan
            .residual_source(Interval::point(0.9), Interval::point(0.6))
            .unwrap();
        assert!(
            matches!(suffix::cached(&other, 0, other.node, &other.totals, &model, &laws, &mut cache).unwrap(), suffix::Bound::Feasible(g) if g == lower_g)
        );
        assert!(matches!(
            suffix::cached(&source, 0, None, &source.totals, &model, &laws, &mut cache).unwrap(),
            suffix::Bound::Unknown
        ));
        let second_plan = Plan::build(
            &model,
            &laws,
            &[7; 2],
            &lower,
            &upper,
            &[1],
            4,
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let second_source = second_plan
            .residual_source(Interval::point(0.7), Interval::point(0.4))
            .unwrap();
        assert!(matches!(
            suffix::cached(
                &second_source,
                0,
                second_source.node,
                &second_source.totals,
                &model,
                &laws,
                &mut cache
            )
            .unwrap(),
            suffix::Bound::Unknown
        ));
        let mut invalid = source.totals.clone();
        invalid[0] = i32::MIN / 4;
        let valid_completion = [2u8, 0u8];
        let mut valid_totals = plan.base.clone();
        for (fi, &o) in valid_completion.iter().enumerate() {
            let (hg, ag) = outcome_gain(&model, fi, o as usize, plan.stride);
            let f = &model.fixtures[fi];
            valid_totals[f.home] += hg;
            valid_totals[f.away] += ag;
        }
        assert!(matches!(
            suffix::minimum(
                &source,
                plan.order.len(),
                None,
                &valid_totals,
                &model,
                &laws,
                &mut cache,
                now + Duration::from_secs(2)
            )
            .unwrap(),
            suffix::Bound::Feasible(1.0)
        ));
        let bad = suffix::cached(
            &source,
            plan.order.len(),
            None,
            &invalid,
            &model,
            &laws,
            &mut cache,
        )
        .unwrap();
        assert!(matches!(bad, suffix::Bound::Impossible));
    }

    #[test]
    fn overflowing_guide_intermediate_never_proves_zero_residual() {
        let v = clipped_guide_lower(Interval::point(1e160), 1e150, 1e-320);
        assert_eq!(v, 0.0);
    }

    #[test]
    fn certain_totals_share_keys_without_changing_guidance_or_suffix_minimum() {
        let (model, laws) = tiny(2);
        let (lower, upper) = broad_bounds();
        let plan = Plan::build(
            &model,
            &laws,
            &[7; 2],
            &lower,
            &upper,
            &[0],
            4,
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let source = plan
            .residual_source(Interval::point(0.7), Interval::point(0.4))
            .unwrap();
        let now = Instant::now();
        let mut options = SuffixOptions::default();
        options.canonicalize_certain = true;
        let mut reduced = suffix::Cache::new(options, now, now + Duration::from_secs(2)).unwrap();
        let first = suffix::minimum(
            &source,
            0,
            source.node,
            &source.totals,
            &model,
            &laws,
            &mut reduced,
            now + Duration::from_secs(2),
        )
        .unwrap();
        assert!(matches!(&first, suffix::Bound::Feasible(_)));
        let mut second_totals = source.totals.clone();
        second_totals[1] += 1;
        let fi = plan.order[0];
        let base: Vec<f64> = (0..3).map(|o| laws[fi].outcomes[o].mid()).collect();
        assert_eq!(
            plan.guided_weights(&model, &laws, fi, 0, &source.totals, &base)
                .unwrap(),
            plan.guided_weights(&model, &laws, fi, 0, &second_totals, &base)
                .unwrap(),
        );
        let first_tickets = categorical_probabilities(
            &plan
                .guided_weights(&model, &laws, fi, 0, &source.totals, &base)
                .unwrap(),
        )
        .unwrap();
        let second_tickets = categorical_probabilities(
            &plan
                .guided_weights(&model, &laws, fi, 0, &second_totals, &base)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(first_tickets, second_tickets);
        let second = suffix::minimum(
            &source,
            0,
            source.node,
            &second_totals,
            &model,
            &laws,
            &mut reduced,
            now + Duration::from_secs(2),
        )
        .unwrap();
        assert!(
            matches!((first, second), (suffix::Bound::Feasible(a), suffix::Bound::Feasible(b)) if a == b)
        );
        let diag = reduced.diagnostics();
        assert!(diag.canonicalized_components > 0);
        assert!(diag.canonicalized_cache_hits > 0);

        let valid_completion = [2u8, 0u8];
        let mut valid_totals = plan.base.clone();
        for (fixture, &outcome) in valid_completion.iter().enumerate() {
            let (hg, ag) = outcome_gain(&model, fixture, outcome as usize, plan.stride);
            let f = &model.fixtures[fixture];
            valid_totals[f.home] += hg;
            valid_totals[f.away] += ag;
        }
        assert!(matches!(
            suffix::minimum(
                &source,
                plan.order.len(),
                None,
                &valid_totals,
                &model,
                &laws,
                &mut reduced,
                now + Duration::from_secs(2)
            )
            .unwrap(),
            suffix::Bound::Feasible(1.0)
        ));
        let mut invalid_totals = valid_totals;
        invalid_totals[0] = i32::MIN;
        assert!(matches!(
            suffix::cached(
                &source,
                plan.order.len(),
                None,
                &invalid_totals,
                &model,
                &laws,
                &mut reduced
            )
            .unwrap(),
            suffix::Bound::Impossible
        ));

        let mut exact =
            suffix::Cache::new(SuffixOptions::default(), now, now + Duration::from_secs(2))
                .unwrap();
        let _ = suffix::minimum(
            &source,
            0,
            source.node,
            &source.totals,
            &model,
            &laws,
            &mut exact,
            now + Duration::from_secs(2),
        )
        .unwrap();
        assert!(matches!(
            suffix::cached(
                &source,
                0,
                source.node,
                &second_totals,
                &model,
                &laws,
                &mut exact
            )
            .unwrap(),
            suffix::Bound::Unknown
        ));
    }

    #[test]
    fn certain_keys_keep_uncertain_components_exact_near_boundary() {
        let (model, laws) = tiny(2);
        let (lower, upper) = (vec![1, 8], vec![i32::MAX / 4; 2]);
        let plan = Plan::build(
            &model,
            &laws,
            &[7; 2],
            &lower,
            &upper,
            &[0],
            4,
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let source = plan
            .residual_source(Interval::point(1.0), Interval::point(1.0))
            .unwrap();
        let now = Instant::now();
        let mut options = SuffixOptions::default();
        options.canonicalize_certain = true;
        let mut cache = suffix::Cache::new(options, now, now + Duration::from_secs(2)).unwrap();
        let _ = suffix::minimum(
            &source,
            0,
            source.node,
            &source.totals,
            &model,
            &laws,
            &mut cache,
            now + Duration::from_secs(2),
        )
        .unwrap();
        let diagnostics = cache.diagnostics();
        assert!(diagnostics.canonicalized_components > 0);
        assert!(diagnostics.exact_components > 0);
        let mut exact_cache =
            suffix::Cache::new(SuffixOptions::default(), now, now + Duration::from_secs(2))
                .unwrap();
        let exact_min = suffix::minimum(
            &source,
            0,
            source.node,
            &source.totals,
            &model,
            &laws,
            &mut exact_cache,
            now + Duration::from_secs(2),
        )
        .unwrap();
        let reduced_min = suffix::minimum(
            &source,
            0,
            source.node,
            &source.totals,
            &model,
            &laws,
            &mut cache,
            now + Duration::from_secs(2),
        )
        .unwrap();
        assert!(
            matches!((exact_min, reduced_min), (suffix::Bound::Feasible(a), suffix::Bound::Feasible(b)) if a == b)
        );
        let mut at_lower = source.totals.clone();
        at_lower[1] = 8;
        assert!(matches!(
            suffix::cached(
                &source,
                0,
                source.node,
                &at_lower,
                &model,
                &laws,
                &mut cache
            )
            .unwrap(),
            suffix::Bound::Unknown | suffix::Bound::Impossible
        ));
        let mut marker_a = source.totals.clone();
        marker_a[1] = 8;
        let mut marker_b = source.totals.clone();
        marker_b[0] = 1;
        assert!(matches!(
            suffix::minimum(
                &source,
                0,
                source.node,
                &marker_a,
                &model,
                &laws,
                &mut cache,
                now + Duration::from_secs(2)
            )
            .unwrap(),
            suffix::Bound::Feasible(_)
        ));
        assert!(matches!(
            suffix::cached(
                &source,
                0,
                source.node,
                &marker_b,
                &model,
                &laws,
                &mut cache
            )
            .unwrap(),
            suffix::Bound::Unknown
        ));
    }

    #[test]
    fn unsupported_signed_and_large_packed_gains_fall_back_to_exact_keys() {
        let (mut model, laws) = tiny(1);
        let (lower, upper) = broad_bounds();
        model.request.phase.championship.point_loss = -1;
        let plan = Plan::build(
            &model,
            &laws,
            &[7],
            &lower,
            &upper,
            &[0],
            4,
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let source = plan
            .residual_source(Interval::point(1.0), Interval::point(1.0))
            .unwrap();
        let now = Instant::now();
        let mut options = SuffixOptions::default();
        options.canonicalize_certain = true;
        let mut cache = suffix::Cache::new(options, now, now + Duration::from_secs(2)).unwrap();
        let _ = suffix::minimum(
            &source,
            0,
            source.node,
            &source.totals,
            &model,
            &laws,
            &mut cache,
            now + Duration::from_secs(2),
        )
        .unwrap();
        let diagnostics = cache.diagnostics();
        assert_eq!(diagnostics.unsupported_plans, 1);
        assert_eq!(diagnostics.canonicalized_components, 0);
        let (home, away) = suffix::checked_packed_gains(&model, 2, i32::MAX).unwrap();
        assert!(home > i32::MAX as i64 || away > i32::MAX as i64);

        let (model, laws) = tiny(2);
        let (lower, upper) = broad_bounds();
        let large_plan = Plan::build(
            &model,
            &laws,
            &[7; 2],
            &lower,
            &upper,
            &[0],
            600_000_000,
            100,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let large_source = large_plan
            .residual_source(Interval::point(1.0), Interval::point(1.0))
            .unwrap();
        let mut large_cache = suffix::Cache::new(
            SuffixOptions {
                canonicalize_certain: true,
                ..Default::default()
            },
            now,
            now + Duration::from_secs(2),
        )
        .unwrap();
        let _ = suffix::minimum(
            &large_source,
            0,
            large_source.node,
            &large_source.totals,
            &model,
            &laws,
            &mut large_cache,
            now + Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(large_cache.diagnostics().unsupported_plans, 1);
    }
}
