//! Multi-cell sampling from exact necessary interval events.
//! Every recipient event implies its own proposal event. Mixture likelihoods
//! include every overlapping component; scores and shared games are sampled once.
use crate::{
    conditioned::{ForwardLayer, IntMap, Result},
    joint_caps::propagated::Guide,
    logging::RequestLog,
    lookahead::RankGame,
    model::{Key, Model},
    pool::{work_per_sample, Estimate},
    rng::{derive, Rng},
    search::{apply, parallel, Cell},
};
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc, time::Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interval {
    pub team: usize,
    pub lo: i32,
    pub hi: i32,
}
struct Layout {
    stride: i32,
    base: Vec<i32>,
    min: Vec<i32>,
    max: Vec<i32>,
    pmfs: Vec<BTreeMap<i32, f64>>,
    hg: [i32; 3],
    ag: [i32; 3],
}
impl Layout {
    fn new(m: &Model) -> Option<Self> {
        if m.keys.first() != Some(&Key::Pt)
            || m.request.phase.bonus_points != 0
            || m.ids.len() != m.n
            || m.n > 32
            || m.fixtures.len() > 256
            || m.fixtures.iter().any(|g| g.home == g.away)
        {
            return None;
        }
        let c = &m.request.phase.championship;
        if (c.point_win, c.point_draw, c.point_loss) != (3, 1, 0) {
            return None;
        }
        let wins = m.keys.get(1) == Some(&Key::W);
        let mut remaining = vec![0; m.n];
        for f in &m.fixtures {
            remaining[f.home] += 1;
            remaining[f.away] += 1;
        }
        let stride = if wins {
            (0..m.n).map(|t| m.base[t].wins + remaining[t]).max()? + 1
        } else {
            1
        };
        let hg64 = [
            0,
            i64::from(stride),
            3 * i64::from(stride) + i64::from(wins),
        ];
        if stride <= 0 || hg64[2] * i64::from(*remaining.iter().max()?) > 4096 {
            return None;
        }
        let hg = hg64.map(|v| v as i32);
        let ag = [hg[2], hg[1], 0];
        let base: Option<Vec<_>> = m.base[..m.n]
            .iter()
            .map(|c| {
                i32::try_from(
                    i64::from(c.points) * i64::from(stride)
                        + if wins { i64::from(c.wins) } else { 0 },
                )
                .ok()
            })
            .collect();
        let base = base?;
        let mut pmfs = Vec::new();
        let mut min = Vec::new();
        let mut max = Vec::new();
        for t in 0..m.n {
            let mut pmf = BTreeMap::from([(0, 1.)]);
            for f in &m.fixtures {
                if f.home != t && f.away != t {
                    continue;
                }
                let gain = if f.home == t { hg } else { ag };
                let mut next = BTreeMap::new();
                for (s, p) in pmf {
                    for o in 0..3 {
                        if f.prob[o] > 0. {
                            *next.entry(s + gain[o]).or_insert(0.) += p * f.prob[o];
                        }
                    }
                }
                pmf = next;
            }
            min.push(base[t].checked_add(*pmf.keys().next()?)?);
            max.push(base[t].checked_add(*pmf.keys().next_back()?)?);
            pmfs.push(pmf);
        }
        Some(Self {
            stride,
            base,
            min,
            max,
            pmfs,
            hg,
            ag,
        })
    }
    fn mass(&self, interval: Interval) -> f64 {
        self.pmfs[interval.team]
            .iter()
            .filter_map(|(&gain, &p)| {
                let final_ = self.base[interval.team] + gain;
                (interval.lo <= final_ && final_ <= interval.hi).then_some(p)
            })
            .sum()
    }
    fn requirements(&self, m: &Model, cell: Cell) -> Option<(Interval, Vec<(Interval, u8, f64)>)> {
        let allowed: Vec<_> = self.pmfs[cell.team]
            .keys()
            .copied()
            .filter(|&gain| {
                let p = self.base[cell.team] + gain;
                let above = (0..m.n)
                    .filter(|&t| t != cell.team && self.min[t] > p)
                    .count();
                let below = (0..m.n)
                    .filter(|&t| t != cell.team && self.max[t] < p)
                    .count();
                above <= cell.rank && below <= m.n - 1 - cell.rank
            })
            .collect();
        let target = Interval {
            team: cell.team,
            lo: self.base[cell.team] + *allowed.first()?,
            hi: self.base[cell.team] + *allowed.last()?,
        };
        let mut blockers = Vec::new();
        for a in 0..m.n {
            if a == cell.team {
                continue;
            }
            let above = (0..m.n)
                .filter(|&t| t != cell.team && t != a && self.min[t] > target.hi)
                .count();
            let below = (0..m.n)
                .filter(|&t| t != cell.team && t != a && self.max[t] < target.lo)
                .count();
            let mut blocker = Interval {
                team: a,
                lo: self.min[a],
                hi: self.max[a],
            };
            let mut direction = 0;
            if above >= cell.rank && blocker.hi > target.hi {
                blocker.hi = target.hi;
                direction |= 1;
            }
            if below >= m.n - 1 - cell.rank && blocker.lo < target.lo {
                blocker.lo = target.lo;
                direction |= 2;
            }
            let mass = self.mass(blocker);
            if direction != 0 && mass > 0. && mass < 0.99 {
                blockers.push((blocker, direction, mass));
            }
        }
        blockers.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.team.cmp(&b.0.team)));
        (!blockers.is_empty()).then_some((target, blockers))
    }
}

struct Game {
    index: usize,
    prob: [f64; 3],
    delta: [u64; 3],
}
struct Event {
    intervals: Vec<Interval>,
    directions: Vec<u8>,
    games: Vec<Game>,
    forward: Vec<ForwardLayer>,
    terminals: Vec<(u64, f64)>,
    mass: f64,
    nodes: usize,
}
fn slot(s: u64, i: usize) -> i32 {
    ((s >> (16 * i)) & 65535) as i32
}
impl Event {
    fn build(
        m: &Model,
        layout: &Layout,
        intervals: Vec<Interval>,
        budget: &mut usize,
    ) -> Option<Self> {
        Self::build_relative(m, layout, intervals, Vec::new(), budget)
    }
    fn build_relative(
        m: &Model,
        layout: &Layout,
        intervals: Vec<Interval>,
        directions: Vec<u8>,
        budget: &mut usize,
    ) -> Option<Self> {
        if intervals.is_empty() || intervals.len() > 3 {
            return None;
        }
        let mut games = Vec::new();
        let mut lo = Vec::new();
        let mut hi = Vec::new();
        for v in &intervals {
            lo.push((v.lo - layout.base[v.team]).max(0));
            hi.push(v.hi - layout.base[v.team]);
        }
        for (index, f) in m.fixtures.iter().enumerate() {
            let mut delta = [0_u64; 3];
            let mut included = false;
            for (i, v) in intervals.iter().enumerate() {
                let gains = if f.home == v.team {
                    Some(layout.hg)
                } else if f.away == v.team {
                    Some(layout.ag)
                } else {
                    None
                };
                if let Some(gains) = gains {
                    included = true;
                    for o in 0..3 {
                        delta[o] |= (gains[o] as u64) << (16 * i);
                    }
                }
            }
            if included {
                games.push(Game {
                    index,
                    prob: f.prob,
                    delta,
                });
            }
        }
        let mut suffix_min = vec![vec![0; intervals.len()]; games.len() + 1];
        let mut suffix_max = suffix_min.clone();
        for step in (0..games.len()).rev() {
            suffix_min[step] = suffix_min[step + 1].clone();
            suffix_max[step] = suffix_max[step + 1].clone();
            for i in 0..intervals.len() {
                let gains: Vec<_> = (0..3)
                    .filter(|&o| games[step].prob[o] > 0.)
                    .map(|o| slot(games[step].delta[o], i))
                    .collect();
                suffix_min[step][i] += *gains.iter().min()?;
                suffix_max[step][i] += *gains.iter().max()?;
            }
        }
        let mut nodes = 0;
        let mut forward = vec![ForwardLayer::from_map(IntMap::from_iter([(0, 1.)]))];
        for (step, g) in games.iter().enumerate() {
            let mut next = IntMap::default();
            for (&state, &p) in forward.last()?.iter() {
                for o in 0..3 {
                    if g.prob[o] <= 0. {
                        continue;
                    }
                    if *budget == 0 {
                        return None;
                    }
                    *budget -= 1;
                    nodes += 1;
                    let new = state + g.delta[o];
                    if (0..intervals.len()).any(|i| {
                        slot(new, i) + suffix_min[step + 1][i] > hi[i]
                            || slot(new, i) + suffix_max[step + 1][i] < lo[i]
                    }) {
                        continue;
                    }
                    let target = layout.base[intervals[0].team] + slot(new, 0);
                    if directions.iter().enumerate().skip(1).any(|(i, &d)| {
                        let other = layout.base[intervals[i].team] + slot(new, i);
                        (d & 1 != 0
                            && other + suffix_min[step + 1][i] > target + suffix_max[step + 1][0])
                            || (d & 2 != 0
                                && other + suffix_max[step + 1][i]
                                    < target + suffix_min[step + 1][0])
                    }) {
                        continue;
                    }
                    *next.entry(new).or_default() += p * g.prob[o];
                    if next.len() > 20000 {
                        return None;
                    }
                }
            }
            forward.push(ForwardLayer::from_map(next));
        }
        let mut terminals: Vec<_> = forward.last()?.iter().map(|(&s, &p)| (s, p)).collect();
        terminals.sort_by_key(|(s, _)| *s);
        let mut mass = 0.;
        for (_, p) in &mut terminals {
            mass += *p;
            *p = mass;
        }
        if mass < 1e-250 || !mass.is_finite() {
            return None;
        }
        Some(Self {
            intervals,
            directions,
            games,
            forward,
            terminals,
            mass,
            nodes,
        })
    }
    fn contains(&self, points: &[i32]) -> bool {
        self.intervals
            .iter()
            .all(|v| v.lo <= points[v.team] && points[v.team] <= v.hi)
            && self.directions.iter().enumerate().skip(1).all(|(i, &d)| {
                let target = points[self.intervals[0].team];
                let other = points[self.intervals[i].team];
                (d & 1 == 0 || other <= target) && (d & 2 == 0 || other >= target)
            })
    }
}
#[derive(Clone, Copy)]
struct Transition {
    previous: [u64; 3],
    mass: [f64; 3],
}
struct Sampler<'a> {
    event: &'a Event,
    cache: Vec<IntMap<Transition>>,
}
impl<'a> Sampler<'a> {
    fn new(event: &'a Event) -> Self {
        Self {
            event,
            cache: (0..event.games.len()).map(|_| IntMap::default()).collect(),
        }
    }
    fn sample(&mut self, rng: &mut Rng, out: &mut [u8]) {
        let u = rng.float() * self.event.mass;
        let mut state = self.event.terminals[self
            .event
            .terminals
            .partition_point(|(_, c)| *c < u)
            .min(self.event.terminals.len() - 1)]
        .0;
        for step in (0..self.event.games.len()).rev() {
            let g = &self.event.games[step];
            let tr = if let Some(&tr) = self.cache[step].get(&state) {
                tr
            } else {
                let mut tr = Transition {
                    previous: [0; 3],
                    mass: [0.; 3],
                };
                for o in 0..3 {
                    if (0..self.event.intervals.len())
                        .all(|i| slot(state, i) >= slot(g.delta[o], i))
                    {
                        tr.previous[o] = state - g.delta[o];
                        tr.mass[o] = self.event.forward[step]
                            .get(&tr.previous[o])
                            .copied()
                            .unwrap_or(0.)
                            * g.prob[o];
                    }
                }
                if self.cache[step].len() < 4096 {
                    self.cache[step].insert(state, tr);
                }
                tr
            };
            let u = rng.float() * tr.mass.iter().sum::<f64>();
            let o = if u < tr.mass[0] {
                0
            } else if u < tr.mass[0] + tr.mass[1] {
                1
            } else {
                2
            };
            out[g.index] = o as u8;
            state = tr.previous[o];
        }
    }
}

pub struct Group {
    pub cells: Vec<Cell>,
    events: Vec<Arc<Event>>,
    common: Arc<Event>,
    pub blocker: usize,
    pub direction: u8,
    guides: Option<Vec<Guide>>,
}
pub struct Build {
    pub groups: Vec<Group>,
    pub eligible: usize,
    pub nodes: usize,
}
pub fn build(m: &Model, cells: &[Cell], ratio: f64, guided: bool) -> Build {
    build_with(m, cells, ratio, guided, 1)
}
pub fn build_with(m: &Model, cells: &[Cell], ratio: f64, guided: bool, blockers: usize) -> Build {
    build_config(m, cells, ratio, guided, blockers, false)
}
pub fn build_config(
    m: &Model,
    cells: &[Cell],
    ratio: f64,
    guided: bool,
    blockers: usize,
    relative: bool,
) -> Build {
    let mut result = Build {
        groups: Vec::new(),
        eligible: 0,
        nodes: 0,
    };
    let Some(layout) = Layout::new(m) else {
        return result;
    };
    let mut candidates: BTreeMap<(usize, u8), Vec<(Cell, Arc<Event>, Interval)>> = BTreeMap::new();
    let mut budget = 150000;
    for &cell in cells {
        if let Some((target, required)) = layout.requirements(m, cell) {
            let (blocker, direction, _) = required[0];
            let intervals = std::iter::once(target)
                .chain(required.iter().take(blockers.clamp(1, 2)).map(|p| p.0))
                .collect();
            result.eligible += 1;
            let directions = if relative {
                std::iter::once(0)
                    .chain(required.iter().take(blockers.clamp(1, 2)).map(|p| p.1))
                    .collect()
            } else {
                Vec::new()
            };
            if let Some(event) =
                Event::build_relative(m, &layout, intervals, directions, &mut budget)
            {
                candidates
                    .entry((blocker.team, direction))
                    .or_default()
                    .push((cell, Arc::new(event), blocker));
            }
        }
    }
    for ((blocker, direction), mut candidates) in candidates {
        candidates.sort_by(|a, b| {
            b.1.mass
                .total_cmp(&a.1.mass)
                .then(a.0.index(m.n).cmp(&b.0.index(m.n)))
        });
        while !candidates.is_empty() {
            let ceiling = candidates[0].1.mass;
            let count = candidates
                .iter()
                .take(4)
                .take_while(|p| p.1.mass * ratio >= ceiling)
                .count();
            let chosen: Vec<_> = candidates.drain(..count.max(1)).collect();
            if chosen.len() < 2 {
                continue;
            }
            let common = Interval {
                team: blocker,
                lo: chosen.iter().map(|p| p.2.lo).min().unwrap(),
                hi: chosen.iter().map(|p| p.2.hi).max().unwrap(),
            };
            if let Some(common) = Event::build(m, &layout, vec![common], &mut budget) {
                let guides = guided.then(|| {
                    chosen
                        .iter()
                        .map(|(_, e, _)| {
                            let games = m
                                .fixtures
                                .iter()
                                .enumerate()
                                .filter(|(i, _)| !e.games.iter().any(|g| g.index == *i))
                                .map(|(index, f)| RankGame {
                                    index,
                                    home: f.home,
                                    away: f.away,
                                    prob: f.prob,
                                    hg: layout.hg,
                                    ag: layout.ag,
                                })
                                .collect();
                            Guide::new(games, m.n)
                        })
                        .collect()
                });
                result.groups.push(Group {
                    guides,
                    cells: chosen.iter().map(|p| p.0).collect(),
                    events: chosen.into_iter().map(|p| p.1).collect(),
                    common: Arc::new(common),
                    blocker,
                    direction,
                });
            }
        }
    }
    result.nodes = 150000 - budget;
    result.groups.sort_by(|a, b| {
        b.cells
            .len()
            .cmp(&a.cells.len())
            .then(a.common.mass.total_cmp(&b.common.mass))
            .then(a.blocker.cmp(&b.blocker))
    });
    result
}
impl Group {
    // Complete-support sequential proposal. Independent suffix chances are
    // guidance only; their product is never reported as an event probability.
    fn guided_ratio(
        &self,
        m: &Model,
        layout: &Layout,
        member: usize,
        out: &mut [u8],
        rng: &mut Rng,
        replay: bool,
    ) -> f64 {
        let Some(guides) = &self.guides else {
            return 1.;
        };
        let guide = &guides[member];
        let cell = self.cells[member];
        let mut points = layout.base.clone();
        for g in &self.events[member].games {
            let f = &m.fixtures[g.index];
            let o = out[g.index] as usize;
            points[f.home] += layout.hg[o];
            points[f.away] += layout.ag[o];
        }
        let target = points[cell.team];
        let below = cell.rank > (m.n - 1) / 2;
        let needed = if below {
            m.n - 1 - cell.rank
        } else {
            cell.rank
        };
        let mut chances: Vec<_> = (0..m.n)
            .map(|t| guide.chance(0, t, target - points[t], below).clamp(0., 1.))
            .collect();
        let mut other = vec![0.; needed + 1];
        let mut ratio = 1.;
        for (step, g) in guide.games.iter().enumerate() {
            other.fill(0.);
            other[0] = 1.;
            for (t, &p) in chances.iter().enumerate() {
                if t == cell.team || t == g.home || t == g.away {
                    continue;
                }
                for j in (0..=needed).rev() {
                    other[j] = other[j] * (1. - p) + if j > 0 { other[j - 1] * p } else { 0. };
                }
            }
            let q = std::array::from_fn::<_, 3, _>(|o| {
                let h = guide
                    .chance(step + 1, g.home, target - points[g.home] - g.hg[o], below)
                    .clamp(0., 1.);
                let a = guide
                    .chance(step + 1, g.away, target - points[g.away] - g.ag[o], below)
                    .clamp(0., 1.);
                g.prob[o]
                    * (other[needed] * (1. - h) * (1. - a)
                        + if needed > 0 {
                            other[needed - 1] * (h * (1. - a) + (1. - h) * a)
                        } else {
                            0.
                        }
                        + if needed > 1 {
                            other[needed - 2] * h * a
                        } else {
                            0.
                        })
            });
            let z = q.iter().sum::<f64>();
            let q = if z > 0. {
                std::array::from_fn(|o| 0.9999 * q[o] / z + 0.0001 * g.prob[o])
            } else {
                g.prob
            };
            let o = if replay {
                out[g.index] as usize
            } else {
                let u = rng.float();
                if u < q[0] {
                    0
                } else if u < q[0] + q[1] {
                    1
                } else {
                    2
                }
            };
            if g.prob[o] <= 0. {
                return 0.;
            }
            ratio *= q[o] / g.prob[o];
            out[g.index] = o as u8;
            points[g.home] += g.hg[o];
            points[g.away] += g.ag[o];
            chances[g.home] = guide
                .chance(step + 1, g.home, target - points[g.home], below)
                .clamp(0., 1.);
            chances[g.away] = guide
                .chance(step + 1, g.away, target - points[g.away], below)
                .clamp(0., 1.);
        }
        ratio
    }
    pub fn describe(&self, m: &Model) -> serde_json::Value {
        json!({"packed_stride":Layout::new(m).unwrap().stride,"blocker":m.ids[self.blocker],"direction":self.direction,"common_mass":self.common.mass,"common_bounds":self.common.intervals.iter().map(|v|json!({"team":m.ids[v.team],"lo":v.lo,"hi":v.hi})).collect::<Vec<_>>(),"members":self.cells.iter().zip(&self.events).map(|(c,e)|json!({"team":m.ids[c.team],"rank":c.rank+1,"event_mass":e.mass,"conditional_requirement_mass":e.mass/self.common.mass,"setup_nodes":e.nodes,"relative_directions":e.directions,"bounds":e.intervals.iter().map(|v|json!({"team":m.ids[v.team],"lo":v.lo,"hi":v.hi})).collect::<Vec<_>>()})).collect::<Vec<_>>()})
    }
    pub fn sample(&self, m: &Model, n: usize, seed: i64, mixture: bool) -> Vec<Result> {
        self.sample_diagnostic(m, n, seed, mixture).0
    }
    pub fn sample_diagnostic(
        &self,
        m: &Model,
        n: usize,
        seed: i64,
        mixture: bool,
    ) -> (Vec<Result>, serde_json::Value) {
        let layout = Layout::new(m).unwrap();
        let mut samplers: Vec<_> = std::iter::once(&*self.common)
            .chain(self.events.iter().map(|p| &**p))
            .map(Sampler::new)
            .collect();
        let selected: Vec<Vec<bool>> = samplers
            .iter()
            .map(|p| {
                let mut included = vec![false; m.fixtures.len()];
                for g in &p.event.games {
                    included[g.index] = true;
                }
                included
            })
            .collect();
        let mut rng = Rng::new(seed);
        let mut out = vec![0; m.fixtures.len()];
        let mut points = layout.base.clone();
        let mut campaign = m.base.clone();
        let mut scores = m.empty_scores();
        let mut order = vec![0; m.n];
        let mut ranks = vec![0; m.n];
        let mut stats = vec![(0., 0., 0_f64, [0.; 2]); self.cells.len()];
        let mut results: Vec<_> = self
            .cells
            .iter()
            .map(|_| Result {
                mass: self.common.mass,
                samples: n,
                work: n as u64 * work_per_sample(m),
                ..Default::default()
            })
            .collect();
        let mut prefix_hits = vec![0_usize; self.cells.len()];
        let mut sorted_seasons = 0;
        let defense = if mixture { 0.1 } else { 1. };
        for draw in 0..n {
            let component = if rng.float() < defense {
                0
            } else {
                1 + (rng.float() * self.events.len() as f64) as usize
            };
            samplers[component].sample(&mut rng, &mut out);
            if component > 0 && self.guides.is_some() {
                self.guided_ratio(m, &layout, component - 1, &mut out, &mut rng, false);
            }
            points.copy_from_slice(&layout.base);
            for (i, f) in m.fixtures.iter().enumerate() {
                if !selected[component][i] && (component == 0 || self.guides.is_none()) {
                    let u = rng.float();
                    out[i] = if u < f.prob[0] {
                        0
                    } else if u < f.prob[0] + f.prob[1] {
                        1
                    } else {
                        2
                    };
                }
                points[f.home] += layout.hg[out[i] as usize];
                points[f.away] += layout.ag[out[i] as usize];
            }
            let mut possible = vec![false; self.cells.len()];
            let mut score_needed = false;
            for (i, c) in self.cells.iter().enumerate() {
                let above = (0..m.n)
                    .filter(|&t| t != c.team && points[t] > points[c.team])
                    .count();
                let tied = (0..m.n)
                    .filter(|&t| t != c.team && points[t] == points[c.team])
                    .count();
                possible[i] = above <= c.rank && c.rank <= above + tied;
                prefix_hits[i] += usize::from(possible[i]);
                score_needed |= possible[i] && tied > 0;
                ranks[c.team] = above;
            }
            if !possible.iter().any(|&v| v) {
                continue;
            }
            if score_needed {
                sorted_seasons += 1;
                campaign.copy_from_slice(&m.base);
                for (i, f) in m.fixtures.iter().enumerate() {
                    let s = f.scores.sample(out[i] as usize, &mut rng);
                    scores[f.request_index] = s;
                    m.add(&mut campaign, i, s);
                }
                m.standings(&mut order, &campaign, &scores, &mut rng);
                for (r, &t) in order.iter().enumerate() {
                    ranks[t] = r;
                }
            }
            let mut density = defense;
            for (member, e) in self.events.iter().enumerate() {
                if e.contains(&points) {
                    density += (1. - defense) / self.events.len() as f64 * self.common.mass
                        / e.mass
                        * self.guided_ratio(m, &layout, member, &mut out, &mut rng, true);
                }
            }
            let weight = 1. / density;
            for (i, c) in self.cells.iter().enumerate() {
                if possible[i] && ranks[c.team] == c.rank {
                    results[i].hits += 1;
                    if results[i].witness.is_none() {
                        results[i].witness = Some(out.clone());
                    }
                    stats[i].0 += weight;
                    stats[i].1 += weight * weight;
                    stats[i].2 = stats[i].2.max(weight);
                    stats[i].3[usize::from(draw >= n / 2)] += weight;
                }
            }
        }
        for (r, (sum, sum2, max, batches)) in results.iter_mut().zip(stats) {
            r.summarize(sum, sum2, max, batches);
        }
        (
            results,
            json!({"prefix_hits":prefix_hits,"sorted_seasons":sorted_seasons,"draws":n}),
        )
    }
}

pub fn run(
    m: &Model,
    cells: &[Cell],
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    log: Option<&RequestLog>,
    budget_ms: f64,
    rough: bool,
) -> (usize, u64) {
    let mode = crate::rare_tail::value("RUST_ODDS_SHARED_CONSTRAINTS");
    if !["mixture", "blocker", "guided"].contains(&mode.as_str()) {
        return (0, 0);
    }
    let start = Instant::now();
    let ratio = crate::rare_tail::value("RUST_ODDS_SHARED_CONSTRAINTS_RATIO")
        .parse::<f64>()
        .ok()
        .filter(|&v| v.is_finite() && v >= 1.)
        .unwrap_or(10.);
    let blockers = crate::rare_tail::value("RUST_ODDS_SHARED_CONSTRAINTS_BLOCKERS")
        .parse::<usize>()
        .ok()
        .unwrap_or(1)
        .clamp(1, 2);
    let setup = build_config(
        m,
        cells,
        ratio,
        mode == "guided",
        blockers,
        crate::rare_tail::value("RUST_ODDS_SHARED_CONSTRAINTS_RELATIVE") == "1",
    );
    let setup_ms = start.elapsed().as_secs_f64() * 1000.;
    let mixture = mode != "blocker";
    let concurrent =
        crate::rare_tail::value("RUST_ODDS_SHARED_CONSTRAINTS_CONFIRMATION") == "parallel";
    let pilots = parallel(setup.groups.len().min(8), workers, |i| {
        if start.elapsed().as_secs_f64() * 1000. > budget_ms * 0.6 {
            return None;
        }
        let g = &setup.groups[i];
        let clock = Instant::now();
        let (pilot, diagnostic) =
            g.sample_diagnostic(m, 750, derive(seed, &format!("shared-pilot-{i}")), mixture);
        let ms = clock.elapsed().as_secs_f64() * 1000.;
        if let Some(l) = log {
            l.event("rust_odds_shared_constraints_pilot", json!({"group_index":i,"proposal":g.describe(m),"sampling_ms":ms,"diagnostic":diagnostic,"cells":pilot.iter().map(|p|json!({"hits":p.hits,"ess":p.ess,"probability":p.probability})).collect::<Vec<_>>()}));
        }
        Some((i, pilot, ms))
    });
    let mut work = 0;
    let mut candidates = Vec::new();
    for (i, pilot, ms) in pilots.into_iter().flatten() {
        work += pilot[0].work;
        let useful: Vec<_> = pilot
            .iter()
            .filter(|r| r.hits >= 3 && r.ess >= 2.)
            .collect();
        if useful.len() < 2 {
            continue;
        }
        let n = useful
            .iter()
            .map(|r| {
                ((if concurrent { 80. } else { 40. }) * r.samples as f64 / r.hits as f64)
                    .max(10. * r.samples as f64 / r.ess)
                    .ceil() as usize
            })
            .max()
            .unwrap()
            .clamp(1000, 5000);
        let check = (n / 2).max(1000);
        let cost = 1.2 * ms / 750. * if concurrent { n } else { n + check } as f64;
        candidates.push((i, n, check, cost, useful.len() as f64 / cost.max(0.01)));
    }
    candidates.sort_by(|a, b| b.4.total_cmp(&a.4));
    let remaining = (budget_ms - start.elapsed().as_secs_f64() * 1000.).max(0.);
    let mut bins = vec![0_f64; workers.clamp(1, 4)];
    candidates.retain(|p| {
        let bin = (0..bins.len())
            .min_by(|&a, &b| bins[a].total_cmp(&bins[b]))
            .unwrap();
        if !concurrent {
            if bins[bin] + p.3 <= remaining {
                bins[bin] += p.3;
                true
            } else {
                false
            }
        } else {
            let mut proposed = bins.clone();
            proposed[bin] += p.3;
            let second = (0..proposed.len())
                .min_by(|&a, &b| proposed[a].total_cmp(&proposed[b]))
                .unwrap();
            proposed[second] += p.3 * p.2 as f64 / p.1 as f64;
            if proposed.iter().all(|&v| v <= remaining) {
                bins = proposed;
                true
            } else {
                false
            }
        }
    });
    let finals = if concurrent {
        let draws = parallel(candidates.len() * 2, workers, |j| {
            let (i, n, check, _, _) = candidates[j / 2];
            let g = &setup.groups[i];
            g.sample(
                m,
                if j % 2 == 0 { n } else { check },
                derive(
                    seed,
                    &format!("shared-{}-{i}", if j % 2 == 0 { "main" } else { "check" }),
                ),
                mixture,
            )
        });
        let mut draws = draws.into_iter();
        candidates
            .iter()
            .map(|p| (p.0, draws.next().unwrap(), draws.next().unwrap()))
            .collect::<Vec<_>>()
    } else {
        parallel(candidates.len(), workers, |j| {
            let (i, n, check, _, _) = candidates[j];
            let g = &setup.groups[i];
            let main = g.sample(m, n, derive(seed, &format!("shared-main-{i}")), mixture);
            let check = g.sample(
                m,
                check,
                derive(seed, &format!("shared-check-{i}")),
                mixture,
            );
            (i, main, check)
        })
    };
    let mut found = 0;
    for (i, main, check) in finals {
        work += main[0].work + check[0].work;
        for ((&cell, main), check) in setup.groups[i].cells.iter().zip(main).zip(check) {
            let accepted = crate::rare_tail::accepted(&main, &check, rough);
            if let Some(l) = log {
                l.event("rust_odds_shared_constraints_final", json!({"group_index":i,"team":m.ids[cell.team],"rank":cell.rank+1,"main_draws":main.samples,"check_draws":check.samples,"probability":main.probability,"hits":main.hits,"ess":main.ess,"relative_se":if main.probability>0.{Some(main.std_err/main.probability)}else{None},"check_probability":check.probability,"check_hits":check.hits,"check_ess":check.ess,"accepted":accepted}));
            }
            if accepted && estimates[cell.index(m.n)].probability == 0. {
                apply(
                    &mut estimates[cell.index(m.n)],
                    &main,
                    "matched_point_pool_shared_constraints",
                );
                found += 1;
            }
        }
    }
    if let Some(l) = log {
        l.event("rust_odds_shared_constraints_summary", json!({"mode":mode,"concurrent_confirmation":concurrent,"eligible":setup.eligible,"groups":setup.groups.len(),"setup_nodes":setup.nodes,"setup_ms":setup_ms,"funded":candidates.len(),"accepted":found,"budget_ms":budget_ms,"elapsed_ms":start.elapsed().as_secs_f64()*1000.,"work":work}));
    }
    (found, work)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Championship, Phase, Request, Team};
    fn toy(points: [i32; 4], wins: bool) -> Model {
        let games = (0..4)
            .flat_map(|a| {
                ((a + 1)..4).map(move |b| crate::model::Game {
                    id: (a * 4 + b) as i32,
                    home_id: a,
                    away_id: b,
                    home_score: 0,
                    away_score: 0,
                    home_power: 0.7 + a as f64 * 0.2,
                    away_power: 0.8 + b as f64 * 0.15,
                    played: false,
                })
            })
            .collect();
        Model::new(Request {
            id: 0,
            zones: vec![],
            phase: Phase {
                sort: if wins { "pt,w,bias" } else { "pt,bias" }.into(),
                championship: Championship {
                    point_win: 3,
                    point_draw: 1,
                    point_loss: 0,
                },
                bonus_points: 0,
                bonus_points_threshold: 0,
            },
            games,
            team_groups: points
                .iter()
                .enumerate()
                .map(|(t, &p)| Team {
                    team_id: t as i32,
                    add_sub: p,
                    bias: t as i32,
                })
                .collect(),
        })
        .unwrap()
    }
    fn enumerate(m: &Model, mut f: impl FnMut(&[i32], &[usize], &[u8], f64)) {
        let l = Layout::new(m).unwrap();
        for mut code in 0..3_usize.pow(m.fixtures.len() as u32) {
            let mut p = 1.;
            let mut outcomes = vec![0; m.fixtures.len()];
            let mut points = l.base.clone();
            let mut campaign = m.base.clone();
            let mut scores = m.empty_scores();
            for (i, g) in m.fixtures.iter().enumerate() {
                let o = code % 3;
                outcomes[i] = o as u8;
                code /= 3;
                p *= g.prob[o];
                points[g.home] += l.hg[o];
                points[g.away] += l.ag[o];
                let s = [[0, 1], [0, 0], [1, 0]][o];
                scores[g.request_index] = s;
                m.add(&mut campaign, i, s);
            }
            let mut order = vec![0; m.n];
            m.standings(&mut order, &campaign, &scores, &mut Rng::new(42));
            let mut ranks = vec![0; m.n];
            for (r, &t) in order.iter().enumerate() {
                ranks[t] = r;
            }
            f(&points, &ranks, &outcomes, p);
        }
    }
    #[test]
    fn requirements_are_necessary_for_every_rank_in_both_directions() {
        for points in [[14, 7, 6, 5], [0, 7, 6, 5], [12, 0, 4, 8], [3, 3, 3, 3]] {
            for wins in [false, true] {
                let m = toy(points, wins);
                let l = Layout::new(&m).unwrap();
                enumerate(&m, |points, ranks, _, _| {
                    for t in 0..m.n {
                        for rank in 0..m.n {
                            if ranks[t] != rank {
                                continue;
                            }
                            if let Some((a, bs)) = l.requirements(&m, Cell { team: t, rank }) {
                                for &(b, d, _) in &bs {
                                    assert!(d & 1 == 0 || points[b.team] <= points[a.team]);
                                    assert!(d & 2 == 0 || points[b.team] >= points[a.team]);
                                }
                                for v in std::iter::once(a).chain(bs.iter().map(|p| p.0)) {
                                    assert!(
                                        v.lo <= points[v.team] && points[v.team] <= v.hi,
                                        "{t}/{rank}: {v:?} {points:?}"
                                    );
                                }
                            }
                        }
                    }
                });
            }
        }
    }
    #[test]
    fn exact_joint_normalizers_mixture_and_sampler_match_enumeration() {
        let m = toy([14, 7, 6, 5], true);
        let cells: Vec<_> = (1..4).map(|team| Cell { team, rank: 0 }).collect();
        let built = build(&m, &cells, 1e12, false);
        assert!(!built.groups.is_empty());
        let g = &built.groups[0];
        assert!(g.cells.len() >= 2);
        let mut masses = vec![0.; g.events.len()];
        let mut common = 0.;
        let mut probabilities = vec![0.; g.cells.len()];
        let mut qmass = 0.;
        enumerate(&m, |points, ranks, _, p| {
            for (i, e) in g.events.iter().enumerate() {
                if e.contains(points) {
                    masses[i] += p;
                }
            }
            if g.common.contains(points) {
                common += p;
                let density = 0.1 / g.common.mass
                    + 0.9 / g.events.len() as f64
                        * g.events
                            .iter()
                            .filter(|e| e.contains(points))
                            .map(|e| 1. / e.mass)
                            .sum::<f64>();
                qmass += p * density;
                for (i, c) in g.cells.iter().enumerate() {
                    if ranks[c.team] == c.rank {
                        assert!(g.events[i].contains(points));
                        probabilities[i] += p;
                    }
                }
            } else {
                for c in &g.cells {
                    assert_ne!(ranks[c.team], c.rank);
                }
            }
        });
        assert!((qmass - 1.).abs() < 1e-12);
        assert!((common - g.common.mass).abs() < 1e-12);
        for (p, e) in masses.iter().zip(&g.events) {
            assert!((p - e.mass).abs() < 1e-12);
        }
        for (guided, blockers, relative) in [
            (false, 1, false),
            (true, 1, false),
            (true, 2, false),
            (true, 2, true),
        ] {
            let built = build_config(&m, &cells, 1e12, guided, blockers, relative);
            let g = &built.groups[0];
            let layout = Layout::new(&m).unwrap();
            let mut qtotal = 0.;
            let mut weighted = vec![0.; g.cells.len()];
            enumerate(&m, |points, ranks, out, p| {
                if !g.common.contains(points) {
                    return;
                }
                let mut density = 0.1 / g.common.mass;
                for (member, e) in g.events.iter().enumerate() {
                    if e.contains(points) {
                        density += 0.9 / g.events.len() as f64 / e.mass
                            * g.guided_ratio(
                                &m,
                                &layout,
                                member,
                                &mut out.to_vec(),
                                &mut Rng::new(0),
                                true,
                            );
                    }
                }
                qtotal += p * density;
                let conditional_weight = 1. / (density * g.common.mass);
                for (i, c) in g.cells.iter().enumerate() {
                    if ranks[c.team] == c.rank {
                        weighted[i] += p * density * g.common.mass * conditional_weight;
                    }
                }
            });
            assert!((qtotal - 1.).abs() < 1e-12, "guided={guided} mass={qtotal}");
            for (actual, expected) in weighted.iter().zip(&probabilities) {
                assert!((actual - expected).abs() < 1e-12);
            }
            for mixture in [false, true] {
                for (i, r) in g.sample(&m, 60000, 818, mixture).iter().enumerate() {
                    assert!(r.hits > 0);
                    assert!(
                        (r.probability - probabilities[i]).abs() < 6. * r.std_err + 1e-8,
                        "{mixture}/{i}: {} vs {} SE {}",
                        r.probability,
                        probabilities[i],
                        r.std_err
                    );
                }
            }
        }
    }
    #[test]
    fn shared_sampler_uses_real_goal_tiebreakers_after_equal_final_points() {
        let mut request = toy([0; 4], false).request;
        request.team_groups.truncate(2);
        request.phase.sort = "pt,gd,gf,bias".into();
        let mut played = request.games[0].clone();
        played.home_score = 3;
        played.away_score = 0;
        played.played = true;
        let mut remaining = played.clone();
        remaining.id = 42;
        remaining.played = false;
        remaining.home_power = 0.8;
        remaining.away_power = 1.3;
        request.games = vec![played, remaining];
        let m = Model::new(request).unwrap();
        let layout = Layout::new(&m).unwrap();
        let event = Arc::new(
            Event::build(
                &m,
                &layout,
                vec![Interval {
                    team: 0,
                    lo: 3,
                    hi: 3,
                }],
                &mut 150000,
            )
            .unwrap(),
        );
        let g = Group {
            cells: vec![Cell { team: 1, rank: 0 }],
            events: vec![event.clone()],
            common: event,
            blocker: 0,
            direction: 1,
            guides: None,
        };
        let mut expected = 0.;
        let mut normalizer = 0.;
        for (h, hp) in crate::sampling::masses(0.8).iter().enumerate() {
            for (a, ap) in crate::sampling::masses(1.3).iter().enumerate() {
                let mut campaign = m.base.clone();
                let mut scores = m.empty_scores();
                let score = [h as i32, a as i32];
                m.add(&mut campaign, 0, score);
                scores[m.fixtures[0].request_index] = score;
                let mut order = vec![0; m.n];
                m.standings(&mut order, &campaign, &scores, &mut Rng::new(42));
                normalizer += hp * ap;
                if order[0] == 1 {
                    expected += hp * ap;
                }
            }
        }
        expected /= normalizer;
        let (r, diagnostic) = g.sample_diagnostic(&m, 60000, 1213, true);
        assert_eq!(diagnostic["sorted_seasons"], 60000);
        assert!(r[0].hits > 100);
        assert!((r[0].probability - expected).abs() < 6. * r[0].std_err);
        assert!(r[0].probability < r[0].mass * 0.3);
    }
    #[test]
    fn nine_remaining_games_two_point_cap_has_exactly_46_weighted_paths() {
        let initial = toy([27, 1, 2, 0], false);
        let mut request = initial.request.clone();
        let template = request.games[0].clone();
        request.games = (0..9)
            .map(|i| {
                let mut g = template.clone();
                g.id = i;
                g.home_id = 0;
                g.away_id = 1 + i % 3;
                g.home_power = 0.7 + 0.1 * i as f64;
                g
            })
            .collect();
        let m = Model::new(request).unwrap();
        let l = Layout::new(&m).unwrap();
        let interval = Interval {
            team: 0,
            lo: 27,
            hi: 29,
        };
        let e = Event::build(&m, &l, vec![interval], &mut 150000).unwrap();
        let mut paths = 0;
        let mut exact = 0.;
        for mut code in 0..3_usize.pow(9) {
            let mut gain = 0;
            let mut mass = 1.;
            for f in &m.fixtures {
                let o = code % 3;
                code /= 3;
                gain += l.hg[o];
                mass *= f.prob[o];
            }
            if gain <= 2 {
                paths += 1;
                exact += mass;
            }
        }
        assert_eq!(paths, 46);
        assert!((exact - e.mass).abs() < 1e-14);
        let mut out = vec![0; 9];
        let mut sampler = Sampler::new(&e);
        let mut rng = Rng::new(123);
        for _ in 0..1000 {
            sampler.sample(&mut rng, &mut out);
            assert!(out.iter().map(|&o| l.hg[o as usize]).sum::<i32>() <= 2);
        }
    }
    #[test]
    fn exhausted_setup_and_unsupported_rules_do_not_claim_proofs() {
        let mut m = toy([14, 7, 6, 5], false);
        let l = Layout::new(&m).unwrap();
        assert!(Event::build(
            &m,
            &l,
            vec![Interval {
                team: 0,
                lo: 14,
                hi: 15
            }],
            &mut 0
        )
        .is_none());
        m.fixtures[0].away = m.fixtures[0].home;
        assert!(Layout::new(&m).is_none());
        m.fixtures[0].away = 1;
        m.request.phase.bonus_points = 1;
        assert_eq!(
            build(&m, &[Cell { team: 1, rank: 0 }], 10., false)
                .groups
                .len(),
            0
        );
        m.request.phase.bonus_points = 0;
        m.request.phase.championship.point_win = 2;
        assert!(Layout::new(&m).is_none());
    }
}
