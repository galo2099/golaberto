//! Sample target paths instead of requiring complete enumeration. A frozen
//! cache is an importance proposal, never a truncation of event support.
use super::*;
use crate::rng::derive;

type SharedGuideKey = (
    usize,
    Vec<(usize, usize, usize, [i32; 3], [i32; 3], [u64; 3])>,
);
#[derive(Default)]
pub struct SharedGuides {
    entries: std::sync::Mutex<HashMap<SharedGuideKey, std::sync::Weak<Guide>>>,
    hits: std::sync::atomic::AtomicUsize,
}
impl SharedGuides {
    fn guide(&self, games: Vec<RankGame>, teams: usize) -> Arc<Guide> {
        let key = (
            teams,
            games
                .iter()
                .map(|g| {
                    (
                        g.index,
                        g.home,
                        g.away,
                        g.hg,
                        g.ag,
                        g.prob.map(f64::to_bits),
                    )
                })
                .collect(),
        );
        if let Some(g) = self
            .entries
            .lock()
            .unwrap()
            .get(&key)
            .and_then(|g| g.upgrade())
        {
            self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return g;
        }
        let built = Arc::new(Guide::new(games, teams));
        let mut entries = self.entries.lock().unwrap();
        if let Some(g) = entries.get(&key).and_then(|g| g.upgrade()) {
            self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            g
        } else {
            entries.insert(key, Arc::downgrade(&built));
            built
        }
    }
    pub fn describe(&self) -> serde_json::Value {
        json!({"hits":self.hits.load(std::sync::atomic::Ordering::Relaxed),"profiles":self.entries.lock().unwrap().values().filter(|g|g.strong_count()>0).count()})
    }
}

const DEFENSIVE: f64 = 0.1;
type JointKey = (Vec<(usize, [u64; 3])>, Vec<(i32, i32)>);
struct Cases {
    above: bool,
    patterns: Vec<(u32, Pattern, f64)>,
    mass: f64,
    complete: bool,
}
#[derive(Default)]
pub(crate) struct RootMoments {
    squared: HashMap<Vec<u8>, f64>,
}
pub struct LazyJoint {
    cell: Cell,
    subset: bool,
    shared: Option<Arc<SharedGuides>>,
    omit: bool,
    goals: Option<crate::goal_tilt::GoalTilt>,
    base: Vec<i32>,
    target: TerminalTable,
    remaining: Vec<RankGame>,
    roots: HashMap<Vec<u8>, Option<Pattern>>,
    alternates: HashMap<Vec<u8>, Pattern>,
    cases: HashMap<Vec<u8>, Cases>,
    biased: HashMap<Vec<u8>, Pattern>,
    mixture: Vec<(Vec<u8>, f64)>,
    mixture_mass: f64,
    root_allocation: HashMap<Vec<u8>, f64>,
    nodes: usize,
    node_limit: usize,
    joint_cache: HashMap<JointKey, (Arc<NecessaryJoint>, Vec<RankGame>)>,
    fallback: Option<(Arc<NecessaryJoint>, Arc<Guide>)>,
    guide_values: usize,
    guides: HashMap<Vec<(usize, [u64; 3])>, Arc<Guide>>,
}
impl LazyJoint {
    pub fn new(
        m: &Model,
        cell: Cell,
        seed: i64,
        witnesses: &[Vec<u8>],
        roots: usize,
    ) -> Option<Self> {
        Self::new_shared(m, cell, seed, witnesses, roots, None)
    }
    pub fn new_shared(
        m: &Model,
        cell: Cell,
        seed: i64,
        witnesses: &[Vec<u8>],
        roots: usize,
        shared: Option<Arc<SharedGuides>>,
    ) -> Option<Self> {
        Self::with_mode(
            m,
            cell,
            seed,
            witnesses,
            roots,
            crate::rare_tail::value("RUST_ODDS_LAZY_SUBSET") == "1",
            crate::rare_tail::value("RUST_ODDS_LAZY_OMIT") == "1",
            shared,
        )
    }
    pub fn from_seasons(
        m: &Model,
        cell: Cell,
        seed: i64,
        witnesses: &[Vec<u8>],
        roots: usize,
    ) -> Option<Self> {
        Self::with_mode(
            m,
            cell,
            seed,
            witnesses,
            roots,
            true,
            crate::rare_tail::value("RUST_ODDS_LAZY_OMIT") == "1",
            None,
        )
    }
    fn with_mode(
        m: &Model,
        cell: Cell,
        seed: i64,
        witnesses: &[Vec<u8>],
        roots: usize,
        subset: bool,
        omit: bool,
        shared: Option<Arc<SharedGuides>>,
    ) -> Option<Self> {
        let rules = &m.request.phase.championship;
        if m.keys.first() != Some(&Key::Pt)
            || m.request.phase.bonus_points != 0
            || (rules.point_win, rules.point_draw, rules.point_loss) != (3, 1, 0)
            || m.ids.len() != m.n
            || m.n > 32
            || m.fixtures.len() > 256
            || m.fixtures.iter().any(|g| g.home == g.away)
        {
            return None;
        }
        let mut left = vec![0; m.n];
        for g in &m.fixtures {
            left[g.home] += 1;
            left[g.away] += 1;
        }
        let wins = m.keys.get(1) == Some(&Key::W);
        let stride = if wins {
            m.base.iter().zip(&left).map(|(b, l)| b.wins + l).max()? + 1
        } else {
            1
        };
        if stride <= 0
            || left
                .iter()
                .any(|&n| (3 * stride + i32::from(wins)) * n > 4096)
            || left[cell.team] == 0
        {
            return None;
        }
        if m.base.iter().any(|c| {
            (i64::from(c.points).abs() + 3 * m.fixtures.len() as i64 + 1) * i64::from(stride)
                + i64::from(c.wins)
                >= i64::from(i32::MAX / 4)
        }) {
            return None;
        }
        let hg = [0, stride, 3 * stride + i32::from(wins)];
        let base: Vec<_> = m
            .base
            .iter()
            .map(|b| b.points * stride + if wins { b.wins } else { 0 })
            .collect();
        let all: Vec<_> = m
            .fixtures
            .iter()
            .enumerate()
            .map(|(index, g)| RankGame {
                index,
                home: g.home,
                away: g.away,
                prob: g.prob,
                hg,
                ag: [hg[2], hg[1], hg[0]],
            })
            .collect();
        let (target, remaining): (Vec<_>, Vec<_>) = all
            .iter()
            .copied()
            .partition(|g| g.home == cell.team || g.away == cell.team);
        let span = (hg[2] * left[cell.team]) as usize + 1;
        let terminal: Vec<_> = (0..span)
            .map(|gain| {
                let total = base[cell.team] + gain as i32;
                let above = (0..m.n)
                    .filter(|&t| t != cell.team && base[t] > total)
                    .count();
                let below = (0..m.n)
                    .filter(|&t| t != cell.team && base[t] + hg[2] * left[t] < total)
                    .count();
                if above <= cell.rank && below <= m.n - 1 - cell.rank {
                    1.
                } else {
                    0.
                }
            })
            .collect();
        let table = TerminalTable::new(target.clone(), cell.team, terminal.clone());
        if table.mass(0) <= 0. {
            return None;
        }
        // Independent rival marginals guide training only. They are not
        // probabilities of the true shared-fixture rank event.
        let pmfs: Vec<_> = (0..m.n)
            .map(|t| {
                let mut p = vec![1.];
                for g in all.iter().filter(|g| g.home == t || g.away == t) {
                    let gain = if g.home == t { g.hg } else { g.ag };
                    let mut next = vec![0.; p.len() + hg[2] as usize];
                    for (a, &v) in p.iter().enumerate() {
                        for o in 0..3 {
                            next[a + gain[o] as usize] += v * g.prob[o];
                        }
                    }
                    p = next;
                }
                p
            })
            .collect();
        let hint: Vec<_> = terminal
            .iter()
            .enumerate()
            .map(|(gain, &allowed)| {
                if allowed == 0. || pmfs[cell.team].get(gain).copied().unwrap_or(0.) == 0. {
                    return 0.;
                }
                let total = base[cell.team] + gain as i32;
                let mut dp = vec![vec![0.; m.n]; m.n];
                dp[0][0] = 1.;
                let mut count = 0;
                for t in 0..m.n {
                    if t == cell.team {
                        continue;
                    }
                    let (mut lo, mut eq, mut hi) = (0., 0., 0.);
                    for (a, &p) in pmfs[t].iter().enumerate() {
                        match (base[t] + a as i32).cmp(&total) {
                            std::cmp::Ordering::Less => lo += p,
                            std::cmp::Ordering::Equal => eq += p,
                            std::cmp::Ordering::Greater => hi += p,
                        }
                    }
                    let mut next = vec![vec![0.; m.n]; m.n];
                    for a in 0..=count {
                        for tie in 0..=count - a {
                            let p = dp[a][tie];
                            next[a][tie] += p * lo;
                            next[a + 1][tie] += p * hi;
                            next[a][tie + 1] += p * eq;
                        }
                    }
                    dp = next;
                    count += 1;
                }
                (0..=cell.rank)
                    .map(|a| (cell.rank - a..m.n - a).map(|tie| dp[a][tie]).sum::<f64>())
                    .sum::<f64>()
            })
            .collect();
        let max = hint.iter().copied().fold(0., f64::max);
        let tilted = TerminalTable::new(
            target.clone(),
            cell.team,
            hint.iter()
                .zip(&terminal)
                .map(|(&h, &a)| a * if max > 0. { (h / max).max(1e-6) } else { 1. })
                .collect(),
        );
        let mut result = Self {
            cell,
            subset,
            shared,
            omit,
            goals: if crate::rare_tail::value("RUST_ODDS_LAZY_GOALS") == "1" {
                crate::goal_tilt::GoalTilt::new(m, cell)
            } else {
                None
            },
            base,
            target: table,
            remaining,
            roots: HashMap::new(),
            alternates: HashMap::new(),
            cases: HashMap::new(),
            biased: HashMap::new(),
            mixture: Vec::new(),
            mixture_mass: 0.,
            root_allocation: HashMap::new(),
            nodes: 0,
            node_limit: 60000,
            joint_cache: HashMap::new(),
            fallback: None,
            guide_values: 0,
            guides: HashMap::new(),
        };
        let mut seen = std::collections::HashSet::new();
        let mut keys = Vec::new();
        for out in witnesses {
            if out.len() != m.fixtures.len() || out.iter().any(|&o| o > 2) {
                continue;
            }
            let key: Vec<_> = target.iter().map(|g| out[g.index]).collect();
            let gain: i32 = target
                .iter()
                .zip(&key)
                .map(|(g, &o)| {
                    if g.home == cell.team {
                        g.hg[o as usize]
                    } else {
                        g.ag[o as usize]
                    }
                })
                .sum();
            if terminal.get(gain as usize) == Some(&1.) && seen.insert(key.clone()) {
                keys.push(key);
            }
        }
        let mut rng = Rng::new(derive(seed, "lazy-root-training"));
        let mut out = vec![0; m.fixtures.len()];
        for i in 0..roots * 4 {
            if keys.len() >= roots {
                break;
            }
            if i % 8 == 0 {
                result.target.sample(0, &mut rng, &mut out);
            } else {
                tilted.sample(0, &mut rng, &mut out);
            }
            let key: Vec<_> = target.iter().map(|g| out[g.index]).collect();
            if seen.insert(key.clone()) {
                keys.push(key);
            }
        }
        keys.truncate(roots);
        for key in keys {
            let anchor = witnesses
                .iter()
                .find(|out| {
                    out.len() == m.fixtures.len()
                        && target.iter().zip(&key).all(|(g, &o)| out[g.index] == o)
                })
                .or(witnesses.first());
            let subset = result.subset;
            result.subset = false;
            let native = result.build(&key, m.n, None, None);
            result.subset = subset;
            let alternate =
                if subset && result.alternates.len() < 8 && matches!(&native, Ok(Some(_))) {
                    result
                        .build(&key, m.n, anchor.map(|v| v.as_slice()), None)
                        .ok()
                        .flatten()
                } else {
                    None
                };
            if let Some(p) = alternate {
                result.alternates.insert(key.clone(), p);
            }
            match native {
                Ok(Some(mut p)) => {
                    let gain:usize=target.iter().zip(&key).map(|(g,&o)|if g.home==cell.team{g.hg[o as usize]}else{g.ag[o as usize]} as usize).sum();
                    p.tilt = if max > 0. {
                        (hint[gain] / max).max(1e-20)
                    } else {
                        1.
                    };
                    let score = p.mass * p.joint.residual_hint.max(1e-80) * p.tilt;
                    result.mixture_mass += score;
                    result.mixture.push((key.clone(), result.mixture_mass));
                    result.roots.insert(key, Some(p));
                }
                Ok(None) => {
                    result.roots.insert(key, None);
                }
                Err(()) => {} // setup exhaustion uses the full-support fallback
            }
        }
        if std::env::var("RUST_ODDS_LAZY_FALLBACK").as_deref() == Ok("guided") {
            let gs = result.remaining.clone();
            let key: Vec<_> = gs
                .iter()
                .map(|g| (g.index, g.prob.map(f64::to_bits)))
                .collect();
            let guide = if let Some(g) = result.guides.get(&key) {
                g.clone()
            } else {
                let span = (0..m.n)
                    .map(|t| {
                        gs.iter()
                            .filter(|g| g.home == t || g.away == t)
                            .map(|g| {
                                *if g.home == t { &g.hg } else { &g.ag }
                                    .iter()
                                    .max()
                                    .unwrap()
                            })
                            .sum::<i32>()
                    })
                    .max()
                    .unwrap_or(0) as usize
                    + 1;
                let values = (2 * gs.len() + 1) * span * 2;
                if result.guide_values + values > 4000000 {
                    return Some(result);
                }
                result.guide_values += values;
                if let Some(s) = &result.shared {
                    s.guide(gs, m.n)
                } else {
                    Arc::new(Guide::new(gs, m.n))
                }
            };
            let (joint, _) = NecessaryJoint::new(
                &[],
                &result.base,
                &vec![i32::MIN / 4; m.n],
                &vec![i32::MAX / 4; m.n],
                0,
                0,
            )?;
            result.fallback = Some((Arc::new(joint), guide));
        }
        Some(result)
    }
    fn build(
        &mut self,
        key: &[u8],
        teams: usize,
        anchor: Option<&[u8]>,
        case: Option<(bool, u32)>,
    ) -> std::result::Result<Option<Pattern>, ()> {
        let mut points = self.base.clone();
        let mut fixed = Vec::new();
        let mut weight = 1.;
        for (g, &o) in self.target.games.iter().zip(key) {
            let o = o as usize;
            if o > 2 || g.prob[o] <= 0. {
                return Ok(None);
            }
            fixed.push((g.index, o as u8));
            weight *= g.prob[o];
            points[g.home] += g.hg[o];
            points[g.away] += g.ag[o];
        }
        let rivals: Vec<_> = (0..teams).filter(|&t| t != self.cell.team).collect();
        let mut d = Domains::propagate(
            &self.remaining,
            &points,
            &rivals,
            self.cell.rank,
            points[self.cell.team],
        );
        if !d.feasible {
            return Ok(None);
        }
        if let Some((above, mask)) = case {
            let target = points[self.cell.team];
            for t in 0..teams {
                if t == self.cell.team {
                    continue;
                }
                let strict = mask & (1 << t) != 0;
                if above == strict {
                    d.lower[t] = d.lower[t].max(target + i32::from(strict));
                } else {
                    d.upper[t] = d.upper[t].min(target - i32::from(strict));
                }
            }
            d = Domains::condition(
                &self.remaining,
                &points,
                &rivals,
                self.cell.rank,
                target,
                &d.lower,
                &d.upper,
                &d.domains,
            );
            if !d.feasible {
                return Ok(None);
            }
        }
        let mut variables = Vec::new();
        for (g, &mask) in self.remaining.iter().zip(&d.domains) {
            let total: f64 = (0..3)
                .filter(|o| mask & (1 << o) != 0)
                .map(|o| g.prob[o])
                .sum();
            if total <= 0. {
                return Ok(None);
            }
            weight *= total;
            if mask.count_ones() == 1 {
                fixed.push((g.index, mask.trailing_zeros() as u8));
            } else {
                let mut g = *g;
                g.prob = std::array::from_fn(|o| {
                    if mask & (1 << o) != 0 {
                        g.prob[o] / total
                    } else {
                        0.
                    }
                });
                variables.push(g);
            }
        }
        let mut omitted = Vec::new();
        let mut base = if d.base.is_empty() {
            points.clone()
        } else {
            d.base.clone()
        };
        if self.omit {
            let target = points[self.cell.team];
            let irrelevant = |t: usize| {
                let lo = points[t] + d.min[0][t];
                let hi = points[t] + d.max[0][t];
                (lo > target || hi < target) && d.lower[t] <= lo && d.upper[t] >= hi
            };
            variables.retain(|g| {
                if irrelevant(g.home) && irrelevant(g.away) {
                    let o = g.prob.iter().position(|&p| p > 0.).unwrap();
                    base[g.home] += g.hg[o];
                    base[g.away] += g.ag[o];
                    omitted.push(*g);
                    false
                } else {
                    true
                }
            });
        }
        if self.subset {
            if let Some(anchor) = anchor.filter(|o| {
                o.len() == self.target.games.len() + self.remaining.len()
                    && o.iter().all(|&v| v < 3)
            }) {
                let mut final_points = self.base.clone();
                for g in self.target.games.iter().chain(&self.remaining) {
                    let o = anchor[g.index] as usize;
                    final_points[g.home] += g.hg[o];
                    final_points[g.away] += g.ag[o];
                }
                let total = base[self.cell.team];
                let mut sides = Vec::new();
                for t in 0..teams {
                    if t == self.cell.team {
                        continue;
                    }
                    let ahead = final_points[t] >= final_points[self.cell.team];
                    let gs: Vec<_> = variables
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
                    let table = TerminalTable::new(
                        gs,
                        t,
                        (0..=span)
                            .map(|a| {
                                if if ahead {
                                    base[t] + a >= total
                                } else {
                                    base[t] + a <= total
                                } {
                                    1.
                                } else {
                                    0.
                                }
                            })
                            .collect(),
                    );
                    let mass = table.mass(0);
                    if mass > 0. && mass < 0.9 {
                        sides.push((t, ahead, mass));
                    }
                }
                sides.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)));
                for (t, ahead, _) in sides.into_iter().take(6) {
                    if ahead {
                        d.lower[t] = d.lower[t].max(total);
                    } else {
                        d.upper[t] = d.upper[t].min(total);
                    }
                }
            }
        }
        let key: JointKey = (
            variables
                .iter()
                .map(|g| (g.index, g.prob.map(f64::to_bits)))
                .collect(),
            (0..teams)
                .map(|t| {
                    let span: i32 = variables
                        .iter()
                        .filter(|g| g.home == t || g.away == t)
                        .map(|g| {
                            *if g.home == t { &g.hg } else { &g.ag }
                                .iter()
                                .max()
                                .unwrap()
                        })
                        .sum();
                    (
                        (d.lower[t] - base[t]).clamp(0, span + 1),
                        (d.upper[t] - base[t]).clamp(-1, span),
                    )
                })
                .collect(),
        );
        let (joint, mut variables) = if let Some(pair) = self.joint_cache.get(&key) {
            pair.clone()
        } else {
            if self.nodes >= self.node_limit {
                return Err(());
            }
            let mut budget = self.node_limit - self.nodes;
            let joint = NecessaryJoint::metered(
                &variables,
                &base,
                &d.lower,
                &d.upper,
                Some(crate::rare_tail::value("RUST_ODDS_LAZY_RIVALS"))
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(6)
                    .clamp(1, 6),
                &mut budget,
            );
            self.nodes = self.node_limit - budget;
            let (joint, variables) = joint.ok_or(())?;
            let pair = (Arc::new(joint), variables);
            self.joint_cache.insert(key, pair.clone());
            pair
        };
        if joint.mass <= 0. {
            return if self.subset { Err(()) } else { Ok(None) };
        }
        let order = std::env::var("RUST_ODDS_LAZY_ORDER").unwrap_or_default();
        if order == "rare" || order == "uncertain" {
            let ahead = self.cell.rank > (teams - 1) / 2;
            let target = base[self.cell.team];
            let chance: Vec<_> = (0..teams)
                .map(|t| {
                    let gs: Vec<_> = variables
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
                    TerminalTable::new(
                        gs,
                        t,
                        (0..=span)
                            .map(|a| {
                                if if ahead {
                                    base[t] + a >= target
                                } else {
                                    base[t] + a <= target
                                } {
                                    1.
                                } else {
                                    0.
                                }
                            })
                            .collect(),
                    )
                    .mass(0)
                })
                .collect();
            let score = |g: &RankGame| {
                let measure = |t: usize| {
                    let p = chance[t];
                    if order == "uncertain" {
                        -p * (1. - p)
                    } else if p > 0. && p < 1. {
                        p
                    } else {
                        1.
                    }
                };
                measure(g.home) + measure(g.away)
            };
            variables.sort_by(|a, b| score(a).total_cmp(&score(b)).then(a.index.cmp(&b.index)));
        }
        let span = (0..teams)
            .map(|t| {
                variables
                    .iter()
                    .filter(|g| g.home == t || g.away == t)
                    .map(|g| {
                        *if g.home == t { &g.hg } else { &g.ag }
                            .iter()
                            .max()
                            .unwrap()
                    })
                    .sum::<i32>()
            })
            .max()
            .unwrap_or(0) as usize
            + 1;
        let values = (2 * variables.len() + 1) * span * 2;
        let guide_key: Vec<_> = variables
            .iter()
            .map(|g| (g.index, g.prob.map(f64::to_bits)))
            .collect();
        let guide = if let Some(g) = self.guides.get(&guide_key) {
            g.clone()
        } else {
            if self.guide_values + values > 4000000 {
                return Err(());
            }
            self.guide_values += values;
            let g = if let Some(s) = &self.shared {
                s.guide(variables, teams)
            } else {
                Arc::new(Guide::new(variables, teams))
            };
            self.guides.insert(guide_key, g.clone());
            g
        };
        weight *= joint.mass;
        Ok(Some(Pattern {
            bias: None,
            omitted,
            fixed,
            base,
            lower: d.lower,
            upper: d.upper,
            guide,
            joint,
            mass: weight,
            tilt: 1.,
            cumulative: 0.,
        }))
    }
    pub fn add_alternates(&mut self, m: &Model, witnesses: &[Vec<u8>]) {
        self.subset = true;
        let keys: Vec<_> = self
            .mixture
            .iter()
            .map(|(k, _)| k.clone())
            .take(8)
            .collect();
        for key in keys {
            let anchor = witnesses
                .iter()
                .find(|o| {
                    o.len() == m.fixtures.len()
                        && self
                            .target
                            .games
                            .iter()
                            .zip(&key)
                            .all(|(g, &v)| o[g.index] == v)
                })
                .or(witnesses.first());
            if let Some(anchor) = anchor {
                if let Ok(Some(p)) = self.build(&key, m.n, Some(anchor), None) {
                    self.alternates.insert(key, p);
                }
            }
        }
    }
    pub fn add_cases(&mut self, m: &Model, witnesses: &[Vec<u8>]) {
        if m.n > 32 {
            return;
        }
        let hard = std::env::var("RUST_ODDS_LAZY_CARDINALITY_CASES").as_deref() == Ok("hard");
        let limit = if hard { 8 } else { 32 };
        if hard {
            self.node_limit = self.nodes + 20000;
        }
        let old = self.subset;
        self.subset = false;
        let keys: Vec<_> = self
            .mixture
            .iter()
            .map(|(k, _)| k.clone())
            .take(if hard { 2 } else { 8 })
            .collect();
        let mut built = 0;
        for key in keys {
            let mut points = self.base.clone();
            for (g, &o) in self.target.games.iter().zip(&key) {
                points[g.home] += g.hg[o as usize];
                points[g.away] += g.ag[o as usize];
            }
            let target = points[self.cell.team];
            let rivals: Vec<_> = (0..m.n).filter(|&t| t != self.cell.team).collect();
            let d = Domains::propagate(&self.remaining, &points, &rivals, self.cell.rank, target);
            if !d.feasible {
                continue;
            }
            let fixed_above = rivals
                .iter()
                .filter(|&&t| points[t] + d.min[0][t] > target)
                .count();
            let fixed_below = rivals
                .iter()
                .filter(|&&t| points[t] + d.max[0][t] < target)
                .count();
            let above = self.cell.rank.saturating_sub(fixed_above)
                <= (m.n - 1 - self.cell.rank).saturating_sub(fixed_below);
            let allowed = if above {
                self.cell.rank
            } else {
                m.n - 1 - self.cell.rank
            };
            let mut mandatory = 0u32;
            let mut ambiguous = Vec::new();
            for &t in &rivals {
                let lo = points[t] + d.min[0][t];
                let hi = points[t] + d.max[0][t];
                if if above { lo > target } else { hi < target } {
                    mandatory |= 1 << t;
                } else if lo <= target && hi >= target {
                    ambiguous.push(t);
                }
            }
            let k = allowed.saturating_sub(mandatory.count_ones() as usize);
            let mut counts = vec![0usize; k + 1];
            counts[0] = 1;
            for _ in &ambiguous {
                for j in (1..=k).rev() {
                    counts[j] = (counts[j] + counts[j - 1]).min(33);
                }
            }
            let complete = counts.iter().sum::<usize>() <= limit;
            let mut masks = Vec::new();
            if complete {
                fn enumerate(ts: &[usize], i: usize, left: usize, mask: u32, out: &mut Vec<u32>) {
                    if i == ts.len() {
                        out.push(mask);
                        return;
                    }
                    enumerate(ts, i + 1, left, mask, out);
                    if left > 0 {
                        enumerate(ts, i + 1, left - 1, mask | (1 << ts[i]), out);
                    }
                }
                enumerate(&ambiguous, 0, k, mandatory, &mut masks);
            } else {
                for out in witnesses.iter().filter(|o| {
                    o.len() == m.fixtures.len()
                        && self
                            .target
                            .games
                            .iter()
                            .zip(&key)
                            .all(|(g, &v)| o[g.index] == v)
                }) {
                    let mut p = self.base.clone();
                    for g in self.target.games.iter().chain(&self.remaining) {
                        let o = out[g.index] as usize;
                        p[g.home] += g.hg[o];
                        p[g.away] += g.ag[o];
                    }
                    let mask = rivals
                        .iter()
                        .filter(|&&t| {
                            if above {
                                p[t] > p[self.cell.team]
                            } else {
                                p[t] < p[self.cell.team]
                            }
                        })
                        .fold(0u32, |s, &t| s | (1 << t));
                    if mask.count_ones() as usize <= allowed && !masks.contains(&mask) {
                        masks.push(mask);
                    }
                }
            }
            let mut cases = Cases {
                above,
                patterns: Vec::new(),
                mass: 0.,
                complete,
            };
            for mask in masks {
                if built >= limit {
                    cases.complete = false;
                    break;
                }
                match self.build(&key, m.n, None, Some((above, mask))) {
                    Ok(Some(p)) => {
                        let score = p.mass * p.joint.residual_hint.max(1e-80);
                        cases.mass += score;
                        cases.patterns.push((mask, p, cases.mass));
                        built += 1;
                    }
                    Ok(None) => {}
                    Err(()) => cases.complete = false,
                }
            }
            if !cases.patterns.is_empty() {
                self.cases.insert(key, cases);
            }
        }
        self.subset = old;
    }
    pub fn add_fixture_bias(&mut self, m: &Model, witnesses: &[Vec<u8>]) {
        let seasons: Vec<_> = witnesses
            .iter()
            .filter(|o| o.len() == m.fixtures.len() && o.iter().all(|&v| v < 3))
            .collect();
        if seasons.is_empty() {
            return;
        }
        // Smoothed empirical outcome frequencies affect the proposal only.
        // Both native and biased path densities are replayed in the mixture.
        let bias: Arc<Vec<[f64; 3]>> = Arc::new(
            m.fixtures
                .iter()
                .enumerate()
                .map(|(i, g)| {
                    let counts: [f64; 3] = std::array::from_fn(|o| {
                        seasons.iter().filter(|s| s[i] == o as u8).count() as f64
                    });
                    std::array::from_fn(|o| {
                        if g.prob[o] > 0. {
                            (counts[o] + g.prob[o]) / (seasons.len() as f64 + 1.) / g.prob[o]
                        } else {
                            1.
                        }
                    })
                })
                .collect(),
        );
        for (key, p) in &self.roots {
            if let Some(p) = p {
                let mut p = p.clone();
                p.bias = Some(bias.clone());
                self.biased.insert(key.clone(), p);
            }
        }
    }
    pub fn clear_bias(&mut self) {
        self.biased.clear();
    }
    pub fn clear_cases(&mut self) {
        self.cases.clear();
    }
    pub fn clear_alternates(&mut self) {
        self.alternates.clear();
    }
    pub fn describe(&self, m: &Model) -> serde_json::Value {
        json!({"mode":"sampled_target_paths","team":m.ids[self.cell.team],"rank":self.cell.rank+1,"roots":self.roots.len(),"alternate_roots":self.alternates.len(),"biased_roots":self.biased.len(),"omitted_fixtures":self.roots.values().filter_map(|p|p.as_ref()).map(|p|p.omitted.len()).sum::<usize>(),"cardinality_roots":self.cases.len(),"cardinality_cases":self.cases.values().map(|c|c.patterns.len()).sum::<usize>(),"feasible_roots":self.mixture.len(),"conditioning_mass":self.target.mass(0),"cache_mass":self.mixture_mass,"setup_nodes":self.nodes,"guide_values":self.guide_values,"joint_profiles":self.joint_cache.len(),"guided_fallback":self.fallback.is_some(),"root_allocation_fitted":!self.root_allocation.is_empty()})
    }
    /// Learn branch frequencies from training only. Cached branches are disjoint
    /// by target outcomes. The 10% full-support component remains unchanged.
    pub(crate) fn fit_root_allocation(&mut self, moments: &RootMoments) -> bool {
        if self.mixture.len() < 2 || self.mixture_mass <= 0. {
            return false;
        }
        let mass = self.target.mass(0);
        let scores: Vec<_> = self
            .mixture
            .iter()
            .map(|(key, _)| {
                let p = self.roots[key].as_ref().unwrap();
                let old = p.mass * p.joint.residual_hint.max(1e-80) * p.tilt / self.mixture_mass;
                let root: f64 = self
                    .target
                    .games
                    .iter()
                    .zip(key)
                    .map(|(g, &o)| g.prob[o as usize])
                    .product();
                let q = DEFENSIVE * root / mass + (1. - DEFENSIVE) * old;
                let second = moments.squared.get(key).copied().unwrap_or(0.);
                (key.clone(), old, (q * second).sqrt())
            })
            .collect();
        let total: f64 = scores.iter().map(|s| s.2).sum();
        if !total.is_finite() || total <= 0. {
            return false;
        }
        self.root_allocation = scores
            .into_iter()
            .map(|(key, old, learned)| (key, 0.5 * old + 0.5 * learned / total))
            .collect();
        self.rebuild_root_cdf();
        true
    }
    pub(crate) fn clear_root_allocation(&mut self) {
        self.root_allocation.clear();
        self.rebuild_root_cdf();
    }
    fn rebuild_root_cdf(&mut self) {
        let mut sum = 0.;
        for (key, cumulative) in &mut self.mixture {
            let p = self.roots[key].as_ref().unwrap();
            sum += self.root_allocation.get(key).copied().map_or_else(
                || p.mass * p.joint.residual_hint.max(1e-80) * p.tilt,
                |q| q * self.mixture_mass,
            );
            *cumulative = sum;
        }
        // Absorb only floating-point accumulation error in the final interval.
        if let Some((_, cumulative)) = self.mixture.last_mut() {
            *cumulative = self.mixture_mass;
        }
    }
    pub(crate) fn sample_training(
        &self,
        m: &Model,
        samples: usize,
        seed: i64,
    ) -> (Result, RootMoments) {
        let mut moments = RootMoments::default();
        let result = self.sample_internal(m, samples, seed, Some(&mut moments));
        (result, moments)
    }
    pub fn sample(&self, m: &Model, samples: usize, seed: i64) -> Result {
        self.sample_internal(m, samples, seed, None)
    }
    fn sample_internal(
        &self,
        m: &Model,
        samples: usize,
        seed: i64,
        mut moments: Option<&mut RootMoments>,
    ) -> Result {
        let mass = self.target.mass(0);
        let alpha = if self.mixture_mass > 0. {
            DEFENSIVE
        } else {
            1.
        };
        let mut result = Result {
            mass,
            ..Default::default()
        };
        let mut rng = Rng::new(seed);
        let mut scores = ScoreContext::new(m);
        let mut goal_context = self
            .goals
            .as_ref()
            .map(|_| crate::goal_tilt::Context::new(m));
        let mut out = vec![0; m.fixtures.len()];
        let cardinality = crate::rare_tail::value("RUST_ODDS_LAZY_GUIDE") == "cardinality";
        let dynamic = std::env::var("RUST_ODDS_LAZY_PROPAGATE")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let mut points = vec![0; m.n];
        let (mut sum, mut sum2, mut max, mut batches) = (0., 0., 0_f64, [0.; 2]);
        for draw in 0..samples {
            let mut fresh_key = smallvec::SmallVec::<[u8; 16]>::new();
            let key: &[u8] = if rng.float() < alpha {
                self.target.sample(0, &mut rng, &mut out);
                fresh_key.extend(self.target.games.iter().map(|g| out[g.index]));
                &fresh_key
            } else {
                let u = rng.float() * self.mixture_mass;
                &self.mixture[self
                    .mixture
                    .partition_point(|(_, v)| *v < u)
                    .min(self.mixture.len() - 1)]
                .0
            };
            result.samples += 1;
            let root_prob: f64 = self
                .target
                .games
                .iter()
                .zip(key)
                .map(|(g, &o)| g.prob[o as usize])
                .product();
            let mut weight = match self.roots.get(key) {
                Some(Some(p)) => {
                    let q = if let Some(&allocation) = self.root_allocation.get(key) {
                        alpha * root_prob / mass + (1. - alpha) * allocation
                    } else {
                        // Preserve the pre-experiment floating-point operation
                        // order when no learned allocation is active.
                        alpha * root_prob / mass
                            + (1. - alpha) * p.mass * p.joint.residual_hint.max(1e-80) * p.tilt
                                / self.mixture_mass
                    };
                    if let Some(cases) = self.cases.get(key) {
                        // Retain the primary guided mode even for an enumerated union.
                        // Numerical zero mass is not an unrestricted infeasibility proof.
                        let alpha = if cases.complete { 0.1 } else { 0.5 };
                        let use_case = rng.float() >= alpha;
                        let mut case_index = None;
                        let drawn = if use_case {
                            let u = rng.float() * cases.mass;
                            let ix = cases
                                .patterns
                                .partition_point(|(_, _, c)| *c < u)
                                .min(cases.patterns.len() - 1);
                            case_index = Some(ix);
                            &cases.patterns[ix].1
                        } else {
                            p
                        };
                        let ratio = drawn.draw(
                            self.cell,
                            &mut rng,
                            &mut out,
                            &mut points,
                            true,
                            cardinality,
                            false,
                            dynamic,
                        );
                        if ratio <= 0. {
                            continue;
                        }
                        let native_density = if alpha > 0. {
                            let ratio = if !use_case {
                                ratio
                            } else {
                                p.draw(
                                    self.cell,
                                    &mut rng,
                                    &mut out,
                                    &mut points,
                                    true,
                                    cardinality,
                                    true,
                                    dynamic,
                                )
                            };
                            if ratio > 0. {
                                root_prob / (p.mass * ratio)
                            } else {
                                0.
                            }
                        } else {
                            0.
                        };
                        let mut case_density = 0.;
                        if !use_case {
                            points.copy_from_slice(&self.base);
                            for g in self.target.games.iter().chain(&self.remaining) {
                                let o = out[g.index] as usize;
                                points[g.home] += g.hg[o];
                                points[g.away] += g.ag[o];
                            }
                            let mask = (0..m.n)
                                .filter(|&t| {
                                    t != self.cell.team
                                        && if cases.above {
                                            points[t] > points[self.cell.team]
                                        } else {
                                            points[t] < points[self.cell.team]
                                        }
                                })
                                .fold(0u32, |s, t| s | (1 << t));
                            case_index = cases.patterns.iter().position(|(s, _, _)| *s == mask);
                        }
                        if let Some(ix) = case_index {
                            let (_, cp, cum) = &cases.patterns[ix];
                            let previous = if ix > 0 { cases.patterns[ix - 1].2 } else { 0. };
                            let ratio = if use_case {
                                ratio
                            } else {
                                cp.draw(
                                    self.cell,
                                    &mut rng,
                                    &mut out,
                                    &mut points,
                                    true,
                                    cardinality,
                                    true,
                                    dynamic,
                                )
                            };
                            if ratio > 0. {
                                case_density =
                                    (cum - previous) / cases.mass * root_prob / (cp.mass * ratio);
                            }
                        }
                        root_prob
                            / (mass * q)
                            / (alpha * native_density + (1. - alpha) * case_density)
                    } else if let Some(alt) =
                        self.biased.get(key).or_else(|| self.alternates.get(key))
                    {
                        let draw_alt = rng.float() < 0.5;
                        let drawn = if draw_alt { alt } else { p };
                        let other = if draw_alt { p } else { alt };
                        let ratio = drawn.draw(
                            self.cell,
                            &mut rng,
                            &mut out,
                            &mut points,
                            true,
                            cardinality,
                            false,
                            dynamic,
                        );
                        if ratio <= 0. {
                            continue;
                        }
                        let density = root_prob / (drawn.mass * ratio);
                        let other_ratio = other.draw(
                            self.cell,
                            &mut rng,
                            &mut out,
                            &mut points,
                            true,
                            cardinality,
                            true,
                            dynamic,
                        );
                        let other_density = if other_ratio > 0. {
                            root_prob / (other.mass * other_ratio)
                        } else {
                            0.
                        };
                        root_prob / (mass * q) / (0.5 * density + 0.5 * other_density)
                    } else {
                        p.mass / (mass * q)
                            * p.draw(
                                self.cell,
                                &mut rng,
                                &mut out,
                                &mut points,
                                true,
                                cardinality,
                                false,
                                dynamic,
                            )
                    }
                }
                Some(None) => 0., // only sound necessary-event infeasibility
                None => {
                    for (g, &o) in self.target.games.iter().zip(key) {
                        out[g.index] = o;
                    }
                    if let Some((joint, guide)) = &self.fallback {
                        let mut base = self.base.clone();
                        for g in &self.target.games {
                            let o = out[g.index] as usize;
                            base[g.home] += g.hg[o];
                            base[g.away] += g.ag[o];
                        }
                        let p = Pattern {
                            bias: None,
                            omitted: Vec::new(),
                            fixed: Vec::new(),
                            base,
                            lower: vec![i32::MIN / 4; m.n],
                            upper: vec![i32::MAX / 4; m.n],
                            guide: guide.clone(),
                            joint: joint.clone(),
                            mass: 1.,
                            tilt: 1.,
                            cumulative: 1.,
                        };
                        p.draw(
                            self.cell,
                            &mut rng,
                            &mut out,
                            &mut points,
                            true,
                            cardinality,
                            false,
                            dynamic,
                        ) / alpha
                    } else {
                        for g in &self.remaining {
                            out[g.index] = choose(g.prob, g.prob.iter().sum(), &mut rng) as u8;
                        }
                        1. / alpha
                    }
                }
            };
            if weight > 0. {
                let (rank, ratio) = if let Some(goals) = &self.goals {
                    goals.rank(m, &out, &mut rng, goal_context.as_mut().unwrap())
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
                result.hits += 1;
                if let Some(moments) = moments.as_deref_mut() {
                    if let Some(sum) = moments.squared.get_mut(key) {
                        *sum += weight * weight;
                    } else {
                        moments.squared.insert(key.to_vec(), weight * weight);
                    }
                }
                if result.witness.is_none() {
                    result.witness = Some(out.clone());
                }
                sum += weight;
                sum2 += weight * weight;
                max = max.max(weight);
                batches[usize::from(draw >= samples / 2)] += weight;
            }
        }
        result.work = result.samples as u64 * work_per_sample(m);
        result.summarize(sum, sum2, max, batches);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{conditioned::canonical_ranks, model::Request};
    #[test]
    fn pilot_root_fit_preserves_weighted_ranks_defensive_support_and_rollback() {
        let request: Request = serde_json::from_value(json!({"id":1,
            "phase":{"sort":"pt,w,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..3).map(|t|json!({"team_id":t,"add_sub":t,"bias":t})).collect::<Vec<_>>(),
            "games":(0..3).flat_map(|h|(h+1..3).map(move|a|json!({"id":h*3+a,"home_id":h,"away_id":a,"home_power":1.1,"away_power":0.9}))).collect::<Vec<_>>() })).unwrap();
        let m = Model::new(request).unwrap();
        for rank in 0..m.n {
            let mut expected = 0.;
            for mut code in 0..3usize.pow(m.fixtures.len() as u32) {
                let mut out = vec![0; m.fixtures.len()];
                let mut mass = 1.;
                for (i, g) in m.fixtures.iter().enumerate() {
                    let o = code % 3;
                    code /= 3;
                    out[i] = o as u8;
                    mass *= g.prob[o];
                }
                if canonical_ranks(&m, &out)[0] == rank {
                    expected += mass;
                }
            }
            for roots in [2, 32] {
                let mut p = LazyJoint::with_mode(
                    &m,
                    Cell { team: 0, rank },
                    801,
                    &[],
                    roots,
                    false,
                    false,
                    None,
                )
                .unwrap();
                let before = p.mixture.clone();
                let ordinary = p.sample(&m, 2000, 911);
                let (pilot, moments) = p.sample_training(&m, 2000, 911);
                assert_eq!(ordinary.probability.to_bits(), pilot.probability.to_bits());
                assert_eq!(ordinary.ess.to_bits(), pilot.ess.to_bits());
                assert!(!p.fit_root_allocation(&RootMoments::default()));
                if p.fit_root_allocation(&moments) {
                    let q: f64 = p.root_allocation.values().sum();
                    assert!((q - 1.).abs() < 1e-12);
                    assert!(p.root_allocation.values().all(|&v| v.is_finite() && v > 0.));
                    for seed in [817, 1229] {
                        let result = p.sample(&m, 30000, seed);
                        assert!(
                            (result.probability - expected).abs() < 6. * result.std_err + 1e-4,
                            "rank={rank} roots={roots}: {} vs {expected}",
                            result.probability
                        );
                    }
                    p.clear_root_allocation();
                    assert_eq!(p.mixture, before);
                    let restored = p.sample(&m, 2000, 911);
                    assert_eq!(
                        restored.probability.to_bits(),
                        ordinary.probability.to_bits()
                    );
                }
            }
        }
    }
    #[test]
    fn cached_and_uncached_roots_match_exhaustive_shared_fixture_rank_probabilities() {
        let request:Request=serde_json::from_value(json!({"id":1,
            "phase":{"sort":"pt,w,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..3).map(|t|json!({"team_id":t,"add_sub":t,"bias":t})).collect::<Vec<_>>(),
            "games":(0..3).flat_map(|h|(h+1..3).map(move|a|json!({"id":h*3+a,"home_id":h,"away_id":a,"home_power":1.1,"away_power":0.9}))).collect::<Vec<_>>() })).unwrap();
        let m = Model::new(request).unwrap();
        for rank in 0..m.n {
            let mut expected = 0.;
            for mut code in 0..3usize.pow(m.fixtures.len() as u32) {
                let mut out = vec![0; m.fixtures.len()];
                let mut mass = 1.;
                for (i, g) in m.fixtures.iter().enumerate() {
                    let o = code % 3;
                    code /= 3;
                    out[i] = o as u8;
                    mass *= g.prob[o];
                }
                if canonical_ranks(&m, &out)[0] == rank {
                    expected += mass;
                }
            }
            for (roots, subset, cases) in [
                (0, false, 0),
                (1, false, 0),
                (32, false, 0),
                (1, true, 0),
                (32, true, 0),
                (32, false, 1),
                (32, false, 2),
                (32, false, 3),
            ] {
                let mut p = LazyJoint::with_mode(
                    &m,
                    Cell { team: 0, rank },
                    801,
                    &[vec![0, 0, 0]],
                    roots,
                    subset,
                    false,
                    None,
                )
                .unwrap();
                if cases == 3 {
                    p.add_fixture_bias(&m, &[vec![0, 0, 0]]);
                    assert!(!p.biased.is_empty());
                }
                if cases > 0 && cases < 3 {
                    p.add_cases(&m, &[vec![0, 0, 0]]);
                    assert!(!p.cases.is_empty());
                    if cases == 2 {
                        // An intentionally incomplete union must still estimate
                        // the entire event through the primary defensive mode.
                        for c in p.cases.values_mut() {
                            c.patterns.truncate(1);
                            c.mass = c.patterns[0].2;
                            c.complete = false;
                        }
                    }
                }
                let r = p.sample(&m, 30000, 808);
                assert!(
                    (r.probability - expected).abs() < 6. * r.std_err + 1e-4,
                    "roots={roots} rank={rank}: {} vs {expected}",
                    r.probability
                );
            }
        }
    }
    #[test]
    fn omitting_strict_slack_rivals_preserves_full_rank_probabilities_and_mixture() {
        let request:Request=serde_json::from_value(json!({"id":1,
            "phase":{"sort":"pt,w,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..5).map(|t|json!({"team_id":t,"add_sub":if t==1||t==2{100}else{3},"bias":t})).collect::<Vec<_>>(),
            "games":([(0,3),(0,4),(1,2),(1,3)]).iter().enumerate().map(|(i,(h,a))|json!({"id":i,"home_id":h,"away_id":a,"home_power":1.1,"away_power":0.9})).collect::<Vec<_>>() })).unwrap();
        let m = Model::new(request).unwrap();
        let mut expected = vec![0.; m.n];
        for mut code in 0..3usize.pow(m.fixtures.len() as u32) {
            let mut out = vec![0; m.fixtures.len()];
            let mut mass = 1.;
            for (i, g) in m.fixtures.iter().enumerate() {
                let o = code % 3;
                code /= 3;
                out[i] = o as u8;
                mass *= g.prob[o];
            }
            expected[canonical_ranks(&m, &out)[0]] += mass;
        }
        let mut omitted = false;
        for (rank, &exact) in expected.iter().enumerate().skip(2) {
            for bias in [false, true] {
                let mut p = LazyJoint::with_mode(
                    &m,
                    Cell { team: 0, rank },
                    801,
                    &[],
                    32,
                    false,
                    true,
                    None,
                )
                .unwrap();
                omitted |= p.roots.values().flatten().any(|p| !p.omitted.is_empty());
                if bias {
                    p.add_fixture_bias(&m, &[vec![0, 0, 0, 0]]);
                }
                let r = p.sample(&m, 30000, 808);
                assert!(
                    (r.probability - exact).abs() < 6. * r.std_err + 1e-4,
                    "rank={rank}, bias={bias}: {} vs {exact}",
                    r.probability
                );
            }
        }
        assert!(omitted);
    }
    #[test]
    fn sharing_checks_exact_fixture_probabilities_and_preserves_guide_bits() {
        let shared = SharedGuides::default();
        let g = RankGame {
            index: 0,
            home: 0,
            away: 1,
            hg: [0, 1, 3],
            ag: [3, 1, 0],
            prob: [0.2, 0.3, 0.5],
        };
        let a = shared.guide(vec![g], 2);
        let b = shared.guide(vec![g], 2);
        assert!(Arc::ptr_eq(&a, &b));
        let ordinary = Guide::new(vec![g], 2);
        for cap in -2..8 {
            for t in 0..2 {
                assert_eq!(
                    a.cdf.cdf(0, t, cap).to_bits(),
                    ordinary.cdf.cdf(0, t, cap).to_bits()
                );
            }
        }
        let mut changed = g;
        changed.prob = [0.3, 0.2, 0.5];
        let c = shared.guide(vec![changed], 2);
        assert!(!Arc::ptr_eq(&a, &c));
        changed = g;
        changed.hg = [0, 1, 4];
        let c = shared.guide(vec![changed], 2);
        assert!(!Arc::ptr_eq(&a, &c));
    }
}
