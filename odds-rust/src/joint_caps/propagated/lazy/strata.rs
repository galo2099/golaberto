//! Offline complete case stratification. Each requested branch is a disjoint
//! target path and strict exception mask; budgets never truncate the sum.
use super::*;
mod messages;
mod tree;

struct Stratum {
    root: Vec<u8>,
    exceptions: Vec<usize>,
    above: bool,
    secondary: Option<usize>,
    pattern: Pattern,
}
pub struct BranchStrata {
    cell: Cell,
    retuned: HashMap<usize, messages::TiltedPattern>,
    message_work: usize,
    strata: Vec<Stratum>,
    goals: Option<crate::goal_tilt::GoalTilt>,
    setup_nodes: usize,
    secondary_fallbacks: usize,
    secondary_nodes: usize,
    tree_training_work: usize,
    tree_diagnostics: Option<serde_json::Value>,
}
pub struct BranchSample {
    pub result: Result,
    pub left: f64,
    pub right: f64,
}
#[derive(Clone, Copy)]
pub struct BranchBound {
    pub fixed_prior: f64,
    pub domain_prior: f64,
    pub normalizer_upper: f64,
    pub six_rival_upper: Option<f64>,
    pub read_two_upper: f64,
    pub upper_bound: f64,
}
// Probability factors are nonnegative and at most one. Flooring products is
// conservative and prevents numerical underflow from becoming a zero proof.
fn bound_product(a: f64, b: f64) -> f64 {
    (a * b).max(f64::MIN_POSITIVE)
}
/// Refine a complete exception branch by every legal outcome path of one
/// tightly capped/floored rival. Fix the path before rerunning all-team
/// propagation. Raw fixture masses enter the stratum normalizer exactly once.
fn secondary(
    builder: &mut LazyJoint,
    key: &[u8],
    points: &[i32],
    rivals: &[usize],
    initial: &Domains,
    above: bool,
    mask: u32,
    selected: u32,
) -> std::result::Result<Option<(usize, Vec<Pattern>)>, String> {
    let target = points[builder.cell.team];
    let (mut lower, mut upper) = (initial.lower.clone(), initial.upper.clone());
    for &t in rivals {
        if selected & (1 << t) == 0 {
            continue;
        }
        let strict = mask & (1 << t) != 0;
        if above == strict {
            lower[t] = lower[t].max(target + i32::from(strict));
        } else {
            upper[t] = upper[t].min(target - i32::from(strict));
        }
    }
    let d = Domains::condition(
        &builder.remaining,
        points,
        rivals,
        builder.cell.rank,
        target,
        &lower,
        &upper,
        &initial.domains,
    );
    if !d.feasible {
        return Err("parent branch propagation disagrees".into());
    }
    type Paths = Vec<(Vec<(usize, u8)>, f64)>;
    fn paths(
        gs: &[(RankGame, u8)],
        t: usize,
        i: usize,
        value: i32,
        lo: i32,
        hi: i32,
        suffix: &[(i32, i32)],
        key: &mut Vec<(usize, u8)>,
        mass: f64,
        out: &mut Paths,
    ) -> bool {
        if value + suffix[i].0 > hi || value + suffix[i].1 < lo {
            return true;
        }
        if i == gs.len() {
            out.push((key.clone(), mass));
            return out.len() <= 16;
        }
        let (g, mask) = gs[i];
        let gains = if g.home == t { g.hg } else { g.ag };
        for o in 0..3 {
            if mask & (1 << o) == 0 || g.prob[o] <= 0. {
                continue;
            }
            key.push((g.index, o as u8));
            if !paths(
                gs,
                t,
                i + 1,
                value + gains[o],
                lo,
                hi,
                suffix,
                key,
                mass * g.prob[o],
                out,
            ) {
                return false;
            }
            key.pop();
        }
        true
    }
    let mut chosen: Option<(usize, Paths, f64)> = None;
    for &t in rivals {
        let gs: Vec<_> = builder
            .remaining
            .iter()
            .zip(&d.domains)
            .filter(|(g, _)| g.home == t || g.away == t)
            .map(|(&g, &mask)| (g, mask))
            .collect();
        if gs.is_empty() || gs.iter().all(|(_, mask)| mask.count_ones() == 1) {
            continue;
        }
        let mut suffix = vec![(0, 0); gs.len() + 1];
        let mut domain_mass = 1.;
        for i in (0..gs.len()).rev() {
            let (g, mask) = gs[i];
            let gains = if g.home == t { g.hg } else { g.ag };
            let options: Vec<_> = (0..3)
                .filter(|&o| mask & (1 << o) != 0 && g.prob[o] > 0.)
                .collect();
            if options.is_empty() {
                return Err("zero numerical fixture domain".into());
            }
            suffix[i] = (
                suffix[i + 1].0 + options.iter().map(|&o| gains[o]).min().unwrap(),
                suffix[i + 1].1 + options.iter().map(|&o| gains[o]).max().unwrap(),
            );
            domain_mass *= options.iter().map(|&o| g.prob[o]).sum::<f64>();
        }
        let mut out = Vec::new();
        if !paths(
            &gs,
            t,
            0,
            points[t],
            d.lower[t],
            d.upper[t],
            &suffix,
            &mut Vec::new(),
            1.,
            &mut out,
        ) || out.len() < 2
        {
            continue;
        }
        let conditional = out.iter().map(|(_, p)| p).sum::<f64>() / domain_mass;
        if !conditional.is_finite() || conditional >= 0.9 {
            continue;
        }
        if chosen.as_ref().is_none_or(|(_, v, p)| {
            out.len() < v.len() || (out.len() == v.len() && conditional < *p)
        }) {
            chosen = Some((t, out, conditional));
        }
    }
    let Some((team, paths, _)) = chosen else {
        return Ok(None);
    };
    let original_base = builder.base.clone();
    let original_games = builder.remaining.clone();
    let mut patterns = Vec::new();
    let mut failed = false;
    for (fixed, mass) in paths {
        builder.base = original_base.clone();
        builder.remaining = original_games
            .iter()
            .copied()
            .filter(|g| !fixed.iter().any(|(i, _)| *i == g.index))
            .collect();
        for &(index, o) in &fixed {
            let g = original_games.iter().find(|g| g.index == index).unwrap();
            builder.base[g.home] += g.hg[o as usize];
            builder.base[g.away] += g.ag[o as usize];
        }
        match builder.build(key, builder.base.len(), None, Some((above, mask, selected))) {
            Ok(Some(mut p)) => {
                p.mass *= mass;
                if p.mass <= 0. || !p.mass.is_finite() {
                    failed = true;
                    break;
                }
                p.fixed.extend(fixed);
                patterns.push(p);
            }
            Ok(None) => {}
            Err(()) => {
                failed = true;
                break;
            }
        }
    }
    builder.base = original_base;
    builder.remaining = original_games;
    if failed || patterns.is_empty() {
        return Err("secondary setup incomplete; union skipped".into());
    }
    Ok(Some((team, patterns)))
}
impl BranchStrata {
    pub fn new(
        m: &Model,
        cell: Cell,
        seed: i64,
        rivals: usize,
    ) -> std::result::Result<Self, String> {
        Self::with_secondary(m, cell, seed, rivals, false)
    }
    pub fn with_secondary(
        m: &Model,
        cell: Cell,
        seed: i64,
        rivals: usize,
        refine: bool,
    ) -> std::result::Result<Self, String> {
        Self::with_secondary_quota(m, cell, seed, rivals, refine, 10000, 2000)
    }
    fn with_secondary_quota(
        m: &Model,
        cell: Cell,
        seed: i64,
        rivals: usize,
        refine: bool,
        secondary_quota: usize,
        per_parent_quota: usize,
    ) -> std::result::Result<Self, String> {
        let mut builder = LazyJoint::with_mode(m, cell, seed, &[], 0, false, false, None)
            .ok_or("unsupported model")?;
        builder.rival_limit = rivals.clamp(1, 6);
        builder.node_limit = 100000;
        // Enumerate every positive target-table path. Cached or witness paths
        // alone are insufficient to assert that strata cover the whole cell.
        fn roots(
            tab: &TerminalTable,
            t: usize,
            i: usize,
            added: usize,
            key: &mut Vec<u8>,
            out: &mut Vec<Vec<u8>>,
        ) -> std::result::Result<(), String> {
            if i == tab.games.len() {
                out.push(key.clone());
                return Ok(());
            }
            let g = tab.games[i];
            let gains = if g.home == t { g.hg } else { g.ag };
            for o in 0..3 {
                let next = added + gains[o] as usize;
                if g.prob[o] > 0. && tab.rows[i + 1].get(next).is_some_and(|&p| p > 0.) {
                    if out.len() >= 64 {
                        return Err("target path budget exhausted".into());
                    }
                    key.push(o as u8);
                    roots(tab, t, i + 1, next, key, out)?;
                    key.pop();
                }
            }
            Ok(())
        }
        let mut keys = Vec::new();
        roots(&builder.target, cell.team, 0, 0, &mut Vec::new(), &mut keys)?;
        let mut strata = Vec::new();
        let mut attempted = 0;
        for key in keys {
            let mut points = builder.base.clone();
            for (g, &o) in builder.target.games.iter().zip(&key) {
                points[g.home] += g.hg[o as usize];
                points[g.away] += g.ag[o as usize];
            }
            let target = points[cell.team];
            let ts: Vec<_> = (0..m.n).filter(|&t| t != cell.team).collect();
            let d = Domains::propagate(&builder.remaining, &points, &ts, cell.rank, target);
            if !d.feasible {
                continue;
            }
            let fixed_above = ts
                .iter()
                .filter(|&&t| points[t] + d.min[0][t] > target)
                .count();
            let fixed_below = ts
                .iter()
                .filter(|&&t| points[t] + d.max[0][t] < target)
                .count();
            let above = cell.rank - fixed_above <= m.n - 1 - cell.rank - fixed_below;
            let left = if above {
                cell.rank - fixed_above
            } else {
                m.n - 1 - cell.rank - fixed_below
            };
            let ambiguous: Vec<_> = ts
                .iter()
                .copied()
                .filter(|&t| points[t] + d.min[0][t] <= target && points[t] + d.max[0][t] >= target)
                .collect();
            fn masks(
                ts: &[usize],
                i: usize,
                left: usize,
                mask: u32,
                out: &mut Vec<u32>,
            ) -> std::result::Result<(), String> {
                if out.len() >= 256 {
                    return Err("case enumeration budget exhausted".into());
                }
                if i == ts.len() {
                    out.push(mask);
                    return Ok(());
                }
                masks(ts, i + 1, left, mask, out)?;
                if left > 0 {
                    masks(ts, i + 1, left - 1, mask | (1 << ts[i]), out)?;
                }
                Ok(())
            }
            let mut cases = Vec::new();
            masks(&ambiguous, 0, left, 0, &mut cases)?;
            let selected = ambiguous.iter().fold(0u32, |s, &t| s | (1 << t));
            for mask in cases {
                attempted += 1;
                if attempted > 256 {
                    return Err("total case budget exhausted".into());
                }
                match builder.build(&key, m.n, None, Some((above, mask, selected))) {
                    Ok(Some(pattern)) => {
                        let exceptions: Vec<_> = ambiguous
                            .iter()
                            .copied()
                            .filter(|&t| mask & (1 << t) != 0)
                            .collect();
                        strata.push(Stratum {
                            root: key.clone(),
                            exceptions,
                            above,
                            secondary: None,
                            pattern,
                        });
                    }
                    Ok(None) => {}
                    Err(()) => {
                        return Err(
                            "case setup/numerical budget exhausted; incomplete union skipped"
                                .into(),
                        )
                    }
                }
            }
        }
        if strata.is_empty() {
            return Err("no usable strata; not an impossibility proof".into());
        }
        // Finish the entire primary union before optional refinement can spend
        // its setup quota. A failed split must never erase an existing parent.
        let mut secondary_fallbacks = 0;
        let mut secondary_nodes = 0;
        if refine {
            let fallback = crate::search::enabled("RUST_ODDS_COMPLETE_PARENT_FALLBACK");
            let mut refined = Vec::new();
            for parent in &strata {
                if fallback && secondary_nodes >= secondary_quota {
                    secondary_fallbacks += 1;
                    break;
                }
                let mut points = builder.base.clone();
                for (g, &o) in builder.target.games.iter().zip(&parent.root) {
                    points[g.home] += g.hg[o as usize];
                    points[g.away] += g.ag[o as usize];
                }
                let rivals: Vec<_> = (0..m.n).filter(|&t| t != cell.team).collect();
                let d = Domains::propagate(
                    &builder.remaining,
                    &points,
                    &rivals,
                    cell.rank,
                    points[cell.team],
                );
                let mask = parent.exceptions.iter().fold(0, |v, &t| v | (1 << t));
                let target = points[cell.team];
                let selected = rivals
                    .iter()
                    .filter(|&&t| {
                        points[t] + d.min[0][t] <= target && points[t] + d.max[0][t] >= target
                    })
                    .fold(0, |v, &t| v | (1 << t));
                let limit = builder.node_limit;
                let before = builder.nodes;
                if fallback {
                    builder.node_limit =
                        limit.min(before + per_parent_quota.min(secondary_quota - secondary_nodes));
                }
                let split = secondary(
                    &mut builder,
                    &parent.root,
                    &points,
                    &rivals,
                    &d,
                    parent.above,
                    mask,
                    selected,
                );
                builder.node_limit = limit;
                secondary_nodes += builder.nodes - before;
                match split {
                    Ok(Some((team, patterns))) => {
                        for pattern in patterns {
                            refined.push(Stratum {
                                root: parent.root.clone(),
                                exceptions: parent.exceptions.clone(),
                                above: parent.above,
                                secondary: Some(team),
                                pattern,
                            });
                        }
                    }
                    Ok(None) => refined.push(Stratum {
                        root: parent.root.clone(),
                        exceptions: parent.exceptions.clone(),
                        above: parent.above,
                        secondary: None,
                        pattern: parent.pattern.clone(),
                    }),
                    Err(reason) if !fallback => return Err(reason),
                    Err(_) => {
                        secondary_fallbacks += 1;
                        break;
                    }
                }
            }
            // Keep the complete primary proposal if any optional split fails.
            // Mixing only the successful children changes allocation across
            // parents and can make an otherwise useful proposal less reliable.
            if secondary_fallbacks == 0 {
                strata = refined;
            }
        }
        Ok(Self {
            retuned: HashMap::new(),
            message_work: 0,
            cell,
            strata,
            goals: builder.goals,
            setup_nodes: builder.nodes,
            secondary_fallbacks,
            secondary_nodes,
            tree_training_work: 0,
            tree_diagnostics: None,
        })
    }
    pub fn len(&self) -> usize {
        self.strata.len()
    }
    pub fn is_empty(&self) -> bool {
        self.strata.is_empty()
    }
    pub fn setup_diagnostics(&self) -> serde_json::Value {
        json!({"nodes":self.setup_nodes,"secondary_nodes":self.secondary_nodes,"tree":self.tree_diagnostics,
            "complete_parent_fallback":self.secondary_fallbacks>0})
    }
    /// Bounds under independent original fixture outcomes, not proposal Q.
    /// Six-rival setup is offline only. Failed setup leaves the existing bound.
    pub fn bounds(&self, m: &Model, strengthen: bool) -> Vec<BranchBound> {
        self.strata
            .iter()
            .map(|s| {
                let p = &s.pattern;
                let fixed_prior: f64 = p
                    .fixed
                    .iter()
                    .map(|&(i, o)| m.fixtures[i].prob[o as usize])
                    .fold(1., bound_product);
                let games: Vec<_> = p
                    .joint
                    .games
                    .iter()
                    .chain(&p.guide.games)
                    .chain(&p.omitted)
                    .copied()
                    .collect();
                let domain_prior = games.iter().fold(fixed_prior, |mass, g| {
                    bound_product(
                        mass,
                        (0..3)
                            .filter(|&o| g.prob[o] > 0.)
                            .map(|o| m.fixtures[g.index].prob[o])
                            .sum::<f64>(),
                    )
                });
                let six_rival_upper = if strengthen {
                    NecessaryJoint::new(&games, &p.base, &p.lower, &p.upper, 6, 20000)
                        .filter(|(j, _)| j.mass > 0. && j.mass.is_finite())
                        .map(|(j, _)| bound_product(domain_prior, j.mass))
                } else {
                    None
                };
                // The joint-selected interval event and each remaining team's
                // interval form a read-2 family: a fixture affects at most two
                // events. Generalized Holder bounds the intersection by the
                // square root of the product of these event probabilities.
                // The plain residual_hint product is NOT an upper bound.
                let mut log_mass = p.joint.mass.ln();
                if strengthen {
                    for t in 0..m.n {
                        if p.joint.selected.contains(&t) {
                            continue;
                        }
                        let gs: Vec<_> = games
                            .iter()
                            .copied()
                            .filter(|g| g.home == t || g.away == t)
                            .collect();
                        let span: i32 = gs
                            .iter()
                            .map(|g| {
                                *if g.home == t { &g.hg } else { &g.ag }
                                    .iter()
                                    .max()
                                    .unwrap()
                            })
                            .sum();
                        let tab = TerminalTable::new(
                            gs,
                            t,
                            (0..=span)
                                .map(|gain| {
                                    if p.base[t] + gain >= p.lower[t]
                                        && p.base[t] + gain <= p.upper[t]
                                    {
                                        1.
                                    } else {
                                        0.
                                    }
                                })
                                .collect(),
                        );
                        let mass = tab.mass(0);
                        // A numerical zero must not become a proof; retain a
                        // looser bound on this event instead.
                        if mass > 0. && mass.is_finite() {
                            log_mass += mass.min(1.).ln();
                        }
                    }
                }
                let read_two_upper = if strengthen {
                    bound_product(domain_prior, (0.5 * log_mass).exp())
                } else {
                    1.
                };
                let raw = fixed_prior
                    .min(p.mass)
                    .min(six_rival_upper.unwrap_or(1.))
                    .min(read_two_upper);
                // Ordinary f64 DP arithmetic, not interval-certified arithmetic.
                // Retain headroom and never turn underflow into a zero proof.
                let upper_bound = if raw > 0. && raw.is_finite() {
                    (raw * (1. + 1e-9)).min(1.)
                } else {
                    fixed_prior
                };
                BranchBound {
                    fixed_prior,
                    domain_prior,
                    normalizer_upper: p.mass,
                    six_rival_upper,
                    read_two_upper,
                    upper_bound,
                }
            })
            .collect()
    }
    pub(crate) fn draw_work(&self, m: &Model, index: usize, bounds: bool) -> usize {
        let p = self
            .retuned
            .get(&index)
            .map(|q| &q.pattern)
            .unwrap_or(&self.strata[index].pattern);
        let needed = self.cell.rank.min(m.n - 1 - self.cell.rank);
        3 * usize::from(self.retuned.contains_key(&index))
            * (p.joint.games.len() + p.guide.games.len())
            + 8 * (p.omitted.len() + p.joint.games.len())
            + p.fixed.len()
            + 8 * m.n
            + p.guide.games.len()
                * (8 + 36
                    + 54
                    + if bounds {
                        0
                    } else {
                        m.n.saturating_sub(3) * (needed + 2)
                    })
            + 8 * m.fixtures.len()
            + m.n * (m.n.max(2).ilog2() as usize + 1) * m.keys.len().max(1)
    }
    pub(crate) fn setup_work(&self) -> usize {
        if let Some(tree) = &self.tree_diagnostics {
            return self.tree_training_work
                + crate::rare_tail::budget::setup_cost(
                    self.setup_nodes,
                    tree["guide_values"].as_u64().unwrap() as usize,
                );
        }
        self.tree_training_work
            + crate::rare_tail::budget::setup_cost(
                self.setup_nodes,
                self.strata
                    .iter()
                    .map(|s| {
                        s.pattern
                            .guide
                            .cdf
                            .values
                            .iter()
                            .chain(&s.pattern.guide.reverse.values)
                            .map(|v| v.len())
                            .sum::<usize>()
                    })
                    .sum(),
            )
    }
    pub fn describe(&self, m: &Model) -> serde_json::Value {
        json!({"team":m.ids[self.cell.team],"rank":self.cell.rank+1,"setup_nodes":self.setup_nodes,"tree":self.tree_diagnostics,"secondary_fallbacks":self.secondary_fallbacks,"secondary_nodes":self.secondary_nodes,"strata":self.strata.iter().map(|s|json!({
            "target_outcomes":s.root,"direction":if s.above{"above"}else{"below"},
            "secondary_team":s.secondary.map(|t|m.ids[t]),
            "exceptions":s.exceptions.iter().map(|&t|m.ids[t]).collect::<Vec<_>>(),
            "forced":s.pattern.fixed.iter().map(|&(i,o)|json!({"game":m.request.games[m.fixtures[i].request_index].id,"home":m.ids[m.fixtures[i].home],"away":m.ids[m.fixtures[i].away],"outcome":o})).collect::<Vec<_>>(),
            "forced_count":s.pattern.fixed.len(),"guided_fixtures":s.pattern.guide.games.len(),"joint_fixtures":s.pattern.joint.games.len(),
            "joint_teams":s.pattern.joint.selected.iter().map(|&t|m.ids[t]).collect::<Vec<_>>(),
            "proposal_normalizer":s.pattern.mass,"allocation_hint":s.pattern.mass*s.pattern.joint.residual_hint.max(1e-80)
        })).collect::<Vec<_>>()})
    }
    pub fn sample(
        &self,
        m: &Model,
        index: usize,
        n: usize,
        seed: i64,
        bounds: bool,
        goals: bool,
    ) -> BranchSample {
        assert!(n >= 2);
        let mut pattern = self
            .retuned
            .get(&index)
            .map(|q| &q.pattern)
            .unwrap_or(&self.strata[index].pattern)
            .clone();
        pattern.bound_guidance = bounds;

        let mut rng = Rng::new(seed);
        let mut out = vec![0; m.fixtures.len()];
        let mut points = vec![0; m.n];
        let mut scores = ScoreContext::new(m);
        let mut goal_context = crate::goal_tilt::Context::new(m);
        let mut r = Result {
            mass: pattern.mass,
            samples: n,
            ..Result::default()
        };
        let (mut sum, mut sum2, mut max, mut batches) = (0., 0., 0_f64, [0.; 2]);
        for draw in 0..n {
            let mut weight = pattern.draw(
                self.cell,
                &mut rng,
                &mut out,
                &mut points,
                true,
                true,
                false,
                0,
                &mut r.operations,
            );
            if weight > 0. {
                if let Some(q) = self.retuned.get(&index) {
                    weight *= q.correction(&out, &mut r.operations);
                }
                r.operations.rank(m);

                let (rank, ratio) = if let Some(g) = self.goals.as_ref().filter(|_| goals) {
                    g.rank(m, &out, &mut rng, &mut goal_context)
                } else {
                    (scores.rank(self.cell.team, &out, &mut rng, None), 1.)
                };
                if rank != self.cell.rank {
                    weight = 0.;
                } else {
                    weight *= ratio;
                }
            }
            if weight > 0. {
                r.hits += 1;
                if r.witness.is_none() {
                    r.witness = Some(out.clone());
                }
                sum += weight;
                sum2 += weight * weight;
                max = max.max(weight);
                batches[usize::from(draw >= n / 2)] += weight;
            }
        }
        r.work = n as u64 * crate::pool::work_per_sample(m);
        r.summarize(sum, sum2, max, batches);
        BranchSample {
            left: pattern.mass * batches[0] / (n / 2) as f64,
            right: pattern.mass * batches[1] / (n - n / 2) as f64,
            result: r,
        }
    }
}
/// Independent disjoint stratum means sum, and their estimated variances sum.
/// ESS and max share use contributions scaled by each stratum's sample count.
pub fn combine(samples: &[BranchSample]) -> Result {
    let probability: f64 = samples.iter().map(|s| s.result.probability).sum();
    let std_err = samples.iter().fold(0_f64, |v, s| v.hypot(s.result.std_err));
    let squared: f64 = if probability > 0. {
        samples
            .iter()
            .filter(|s| s.result.ess > 0.)
            .map(|s| (s.result.probability / probability).powi(2) / s.result.ess)
            .sum()
    } else {
        0.
    };
    let max_share = if probability > 0. {
        samples
            .iter()
            .map(|s| s.result.max_share * (s.result.probability / probability))
            .fold(0., f64::max)
    } else {
        0.
    };
    Result {
        mass: 1.,
        samples: samples.iter().map(|s| s.result.samples).sum(),
        hits: samples.iter().map(|s| s.result.hits).sum(),
        work: samples.iter().map(|s| s.result.work).sum(),
        operations: samples.iter().fold(
            crate::rare_tail::budget::Operations::default(),
            |mut sum, s| {
                sum.add(s.result.operations);
                sum
            },
        ),
        weighted: probability > 0.,
        probability,
        std_err,
        ess: if squared > 0. { 1. / squared } else { 0. },
        max_share,
        batch_gap: if probability > 0. {
            (samples.iter().map(|s| s.left).sum::<f64>()
                - samples.iter().map(|s| s.right).sum::<f64>())
            .abs()
                / probability
        } else {
            0.
        },
        witness: samples.iter().find_map(|s| s.result.witness.clone()),
        ..Result::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{conditioned::canonical_ranks, model::Request};
    #[test]
    fn tiny_bound_products_remain_positive() {
        assert_eq!(bound_product(1e-200, 1e-200), f64::MIN_POSITIVE);
        assert_eq!(bound_product(f64::MIN_POSITIVE, 1e-200), f64::MIN_POSITIVE);
        assert_eq!(bound_product(0.5, 0.25), 0.125);
    }
    #[test]
    fn incomplete_target_enumeration_is_an_error_and_not_a_partial_estimate() {
        let request:Request=serde_json::from_value(json!({"id":1,
            "phase":{"sort":"pt,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..8).map(|t|json!({"team_id":t,"add_sub":if t==0{0}else{100}})).collect::<Vec<_>>(),
            "games":(1..8).map(|a|json!({"id":a,"home_id":0,"away_id":a,"home_power":1.1,"away_power":0.9})).collect::<Vec<_>>() })).unwrap();
        let m = Model::new(request).unwrap();
        assert!(
            matches!(BranchStrata::new(&m,Cell{team:0,rank:7},801,4),Err(e) if e.contains("target path budget"))
        );
    }
    #[test]
    fn combining_tiny_strata_avoids_squaring_unconditional_probabilities() {
        let samples: Vec<_> = [1e-240, 2e-240]
            .iter()
            .map(|&p| BranchSample {
                result: Result {
                    probability: p,
                    std_err: p * 0.1,
                    ess: 100.,
                    max_share: 0.01,
                    samples: 100,
                    hits: 100,
                    ..Result::default()
                },
                left: p,
                right: p,
            })
            .collect();
        let r = combine(&samples);
        assert!((r.probability / 3e-240 - 1.).abs() < 1e-12);
        assert!((r.std_err / 2.23606797749979e-241 - 1.).abs() < 1e-12);
        assert!((r.ess - 180.).abs() < 1e-10);
        assert_eq!(r.batch_gap, 0.);
    }
    #[test]
    fn secondary_paths_preserve_the_complete_union_and_raw_fixture_mass() {
        let request:Request=serde_json::from_value(json!({"id":1,
            "phase":{"sort":"pt,w,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..5).map(|t|json!({"team_id":t,"add_sub":if t==1{2}else{0},"bias":if t==0{100}else{0}})).collect::<Vec<_>>(),
            "games":([(0,4),(1,2),(1,3),(2,3)]).iter().enumerate().map(|(i,(h,a))|json!({"id":i,"home_id":h,"away_id":a,"home_power":1.1,"away_power":0.9})).collect::<Vec<_>>() })).unwrap();
        let m = Model::new(request).unwrap();
        let cell = Cell { team: 0, rank: 1 };
        let mut exact = 0.;
        for mut code in 0..81 {
            let mut out = vec![0; 4];
            let mut mass = 1.;
            for (i, g) in m.fixtures.iter().enumerate() {
                let o = code % 3;
                code /= 3;
                out[i] = o as u8;
                mass *= g.prob[o];
            }
            if canonical_ranks(&m, &out)[0] == cell.rank {
                exact += mass;
            }
        }
        for (quota, parent_quota) in [(0, 0), (10000, 0), (10000, 2000)] {
            let p = BranchStrata::with_secondary_quota(&m, cell, 801, 4, true, quota, parent_quota)
                .unwrap();
            if parent_quota > 0 {
                assert!(p.strata.iter().any(|s| s.secondary.is_some()));
            } else {
                assert!(p.secondary_fallbacks > 0);
            }
            let bounds = p.bounds(&m, true);
            let mut covered = 0.;
            for (s, bound) in p.strata.iter().zip(&bounds) {
                let pattern = &s.pattern;
                let games: Vec<_> = pattern
                    .joint
                    .games
                    .iter()
                    .chain(&pattern.guide.games)
                    .collect();
                let mut seen = std::collections::HashSet::new();
                assert!(pattern.fixed.iter().all(|&(i, _)| seen.insert(i)));
                let mut branch_mass = 0.;
                for mut code in 0..81 {
                    let mut out = vec![0; 4];
                    let mut mass = 1.;
                    for (i, g) in m.fixtures.iter().enumerate() {
                        let o = code % 3;
                        code /= 3;
                        out[i] = o as u8;
                        mass *= g.prob[o];
                    }
                    if pattern.fixed.iter().any(|&(i, o)| out[i] != o)
                        || games.iter().any(|g| g.prob[out[g.index] as usize] <= 0.)
                        || canonical_ranks(&m, &out)[0] != cell.rank
                    {
                        continue;
                    }
                    let mut totals = pattern.base.clone();
                    for g in &games {
                        totals[g.home] += g.hg[out[g.index] as usize];
                        totals[g.away] += g.ag[out[g.index] as usize];
                    }
                    if (0..m.n)
                        .all(|t| totals[t] >= pattern.lower[t] && totals[t] <= pattern.upper[t])
                    {
                        branch_mass += mass;
                    }
                }
                assert!(branch_mass <= bound.upper_bound + 1e-12);
                assert!(bound.normalizer_upper <= bound.domain_prior * (1. + 1e-10));
                assert!(bound.domain_prior <= bound.fixed_prior * (1. + 1e-10));
                covered += branch_mass;
            }
            assert!((covered - exact).abs() < 1e-12);
            for bounds in [false, true] {
                let samples: Vec<_> = (0..p.len())
                    .map(|i| {
                        p.sample(
                            &m,
                            i,
                            10000 + i * 127,
                            derive(808, &format!("secondary-test-{i}")),
                            bounds,
                            false,
                        )
                    })
                    .collect();
                let r = combine(&samples);
                assert!(
                    (r.probability - exact).abs() < 6. * r.std_err + 1e-4,
                    "{} vs {exact}",
                    r.probability
                );
            }
        }
    }
    #[test]
    fn complete_case_stratification_matches_exhaustive_rank_probabilities() {
        let request:Request=serde_json::from_value(json!({"id":1,"phase":{"sort":"pt,w,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..3).map(|t|json!({"team_id":t,"add_sub":t,"bias":t})).collect::<Vec<_>>(),
            "games":(0..3).flat_map(|h|(h+1..3).map(move|a|json!({"id":h*3+a,"home_id":h,"away_id":a,"home_power":1.1,"away_power":0.9}))).collect::<Vec<_>>() })).unwrap();
        let m = Model::new(request).unwrap();
        for rank in 0..m.n {
            let mut exact = 0.;
            for mut code in 0..27 {
                let mut out = vec![0; m.fixtures.len()];
                let mut mass = 1.;
                for (i, g) in m.fixtures.iter().enumerate() {
                    let o = code % 3;
                    code /= 3;
                    out[i] = o as u8;
                    mass *= g.prob[o];
                }
                if canonical_ranks(&m, &out)[0] == rank {
                    exact += mass;
                }
            }
            let p = BranchStrata::new(&m, Cell { team: 0, rank }, 801, 4).unwrap();
            for bounds in [false, true] {
                let samples: Vec<_> = (0..p.len())
                    .map(|i| {
                        p.sample(
                            &m,
                            i,
                            10000 + i * 127,
                            derive(808, &format!("test-{i}")),
                            bounds,
                            false,
                        )
                    })
                    .collect();
                let r = combine(&samples);
                assert!(
                    (r.probability - exact).abs() < 6. * r.std_err + 1e-4,
                    "{rank}: {} vs {exact}",
                    r.probability
                );
                assert_eq!(
                    r.samples,
                    samples.iter().map(|s| s.result.samples).sum::<usize>()
                );
            }
        }
    }
}
