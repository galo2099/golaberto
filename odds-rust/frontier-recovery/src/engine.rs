use crate::{
    gd_bounds::GdCache,
    joint::Plan as JointPlan,
    law::{Interval, Law},
    score::{self, ScoreCache},
};
use golaberto_odds::{
    domains::{DomainWork, Domains},
    lookahead::RankGame,
    model::{Model, Request},
    rng::Rng,
    search::Cell,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Instant;

mod regime;
use regime::RegimePlan;
mod hybrid;

/// A support-preserving recovery engine. Every region starts with its native
/// law conditioned only on the safe unary WDL masks; joint plans refine that
/// proposal while retaining a sticky masked-native suffix for unresolved states.
pub struct Engine {
    model: Model,
    laws: Vec<Law>,
    target: usize,
    rank: usize,
    regions: Vec<Region>,
    score_cache: ScoreCache,
    gd_cache: GdCache,
}

struct Region {
    root_id: usize,
    target_points: i32,
    target_wins: i32,
    above: Vec<usize>,
    upper: f64,
    impossible: bool,
    masks: Vec<u8>,
    lower: Vec<i32>,
    upper_bounds: Vec<i32>,
    plan: Option<JointPlan>,
    regime: Option<RegimePlan>,
    optimized: bool,
    epoch: u64,
}

fn up(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        x
    } else {
        f64::from_bits(x.to_bits() + 1)
    }
}
fn mul_up_positive(a: f64, b: f64) -> f64 {
    let x = a * b;
    if x == 0.0 && a > 0.0 && b > 0.0 {
        f64::from_bits(1)
    } else {
        up(x)
    }
}
fn mask_mass_upper(laws: &[Law], masks: &[u8]) -> f64 {
    let mut mass = 1.0;
    for (i, law) in laws.iter().enumerate() {
        let p = (0..3)
            .filter(|&o| masks[i] & (1 << o) != 0)
            .map(|o| law.outcomes[o].hi)
            .sum::<f64>();
        mass = mul_up_positive(mass, p);
    }
    mass.min(1.0)
}

impl Engine {
    fn packed_stride(&self) -> i32 {
        (0..self.model.n)
            .map(|t| {
                self.model.base[t].wins
                    + self
                        .model
                        .fixtures
                        .iter()
                        .filter(|f| f.home == t || f.away == t)
                        .count() as i32
            })
            .max()
            .unwrap_or(0)
            + 1
    }
    pub fn new(request: Request, team_id: i32, position: usize) -> Result<Self, String> {
        if position == 0 {
            return Err("position must be one-based and positive".into());
        }
        if request.phase.bonus_points != 0 || request.phase.bonus_points_threshold != 0 {
            return Err("recovery engine supports no bonus points".into());
        }
        let model = Model::new(request)?;
        if model.request.phase.championship.point_win != 3
            || model.request.phase.championship.point_draw != 1
            || model.request.phase.championship.point_loss != 0
        {
            return Err("recovery engine requires ordinary 3/1/0 scoring".into());
        }
        let supported = [
            golaberto_odds::model::Key::Pt,
            golaberto_odds::model::Key::W,
            golaberto_odds::model::Key::Gd,
            golaberto_odds::model::Key::Gf,
            golaberto_odds::model::Key::Gp,
            golaberto_odds::model::Key::Name,
        ];
        if model.keys.as_slice() != supported {
            return Err("recovery engine requires ranking pt,w,gd,gf,gp,name".into());
        }
        let target = *model
            .indices
            .get(&team_id)
            .ok_or("target team is not in group")?;
        let rank = position - 1;
        if rank >= model.n {
            return Err("position exceeds group size".into());
        }
        let mut laws = Vec::with_capacity(model.fixtures.len());
        for f in &model.fixtures {
            laws.push(Law::new(f.home_sampler.mean, f.away_sampler.mean)?);
        }
        let stride = (0..model.n)
            .map(|t| {
                model.base[t].wins
                    + model
                        .fixtures
                        .iter()
                        .filter(|f| f.home == t || f.away == t)
                        .count() as i32
            })
            .max()
            .unwrap_or(0)
            + 1;
        let initial: Vec<u8> = laws
            .iter()
            .map(|l| {
                (0..3).fold(0, |m, o| {
                    if l.outcomes[o].hi > 0.0 {
                        m | (1 << o)
                    } else {
                        m
                    }
                })
            })
            .collect();
        let mut target_fixtures = Vec::new();
        for (i, f) in model.fixtures.iter().enumerate() {
            if f.home == target || f.away == target {
                target_fixtures.push(i)
            }
        }
        let mut roots = BTreeMap::<(i32, i32), f64>::new();
        fn enumerate_roots(
            pos: usize,
            games: &[usize],
            model: &Model,
            laws: &[Law],
            target: usize,
            stride: i32,
            points: i32,
            wins: i32,
            mass: f64,
            roots: &mut BTreeMap<(i32, i32), f64>,
        ) {
            if pos == games.len() {
                let slot = roots.entry((points, wins)).or_default();
                *slot = up(*slot + mass);
                return;
            }
            let fi = games[pos];
            let f = &model.fixtures[fi];
            for o in 0..3 {
                let p = laws[fi].outcomes[o].hi;
                if p <= 0.0 {
                    continue;
                }
                let own_win = if f.home == target { o == 2 } else { o == 0 };
                let draw = o == 1;
                let pg = if own_win {
                    model.request.phase.championship.point_win
                } else if draw {
                    model.request.phase.championship.point_draw
                } else {
                    model.request.phase.championship.point_loss
                };
                enumerate_roots(
                    pos + 1,
                    games,
                    model,
                    laws,
                    target,
                    stride,
                    points + pg,
                    wins + i32::from(own_win),
                    mul_up_positive(mass, p),
                    roots,
                );
            }
            let _ = stride;
        }
        enumerate_roots(
            0,
            &target_fixtures,
            &model,
            &laws,
            target,
            stride,
            model.base[target].points,
            model.base[target].wins,
            1.0,
            &mut roots,
        );
        let cert =
            golaberto_odds::target_limits::certify(&model, Cell { team: target, rank }, 16_000);
        let left: Vec<i32> = (0..model.n)
            .map(|t| {
                model
                    .fixtures
                    .iter()
                    .filter(|f| f.home == t || f.away == t)
                    .count() as i32
            })
            .collect();
        let packed_base: Vec<i32> = model
            .base
            .iter()
            .map(|c| c.points * stride + c.wins)
            .collect();
        let games: Vec<RankGame> = model
            .fixtures
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let mut hg = [0; 3];
                let mut ag = [0; 3];
                for o in 0..3 {
                    let (h, a) = if o == 0 {
                        (
                            model.request.phase.championship.point_loss,
                            model.request.phase.championship.point_win,
                        )
                    } else if o == 1 {
                        (
                            model.request.phase.championship.point_draw,
                            model.request.phase.championship.point_draw,
                        )
                    } else {
                        (
                            model.request.phase.championship.point_win,
                            model.request.phase.championship.point_loss,
                        )
                    };
                    hg[o] = h * stride + i32::from(o == 2);
                    ag[o] = a * stride + i32::from(o == 0);
                }
                RankGame {
                    index: i,
                    home: f.home,
                    away: f.away,
                    prob: std::array::from_fn(|o| laws[i].outcomes[o].mid()),
                    hg,
                    ag,
                }
            })
            .collect();
        let mut regions = Vec::new();
        let mut root_id = 0usize;
        for (&(target_points, target_wins), &root_mass) in &roots {
            let packed = target_points * stride + target_wins;
            if !cert.allows(i64::from(packed)) {
                continue;
            }
            let mut required = Vec::new();
            let mut eligible = Vec::new();
            for t in 0..model.n {
                if t == target {
                    continue;
                }
                let lo = packed_base[t];
                let hi = lo + left[t] * (3 * stride + 1);
                if lo > packed {
                    required.push(t);
                    eligible.push(t)
                } else if hi >= packed {
                    eligible.push(t)
                }
            }
            if required.len() > rank || eligible.len() < rank {
                root_id += 1;
                continue;
            }
            let mut choices = Vec::new();
            fn choose(
                at: usize,
                need: usize,
                items: &[usize],
                current: &mut Vec<usize>,
                out: &mut Vec<Vec<usize>>,
            ) {
                if need == 0 {
                    out.push(current.clone());
                    return;
                }
                if items.len().saturating_sub(at) < need {
                    return;
                }
                for i in at..items.len() {
                    current.push(items[i]);
                    choose(i + 1, need - 1, items, current, out);
                    current.pop();
                }
            }
            let optional: Vec<_> = eligible
                .iter()
                .copied()
                .filter(|t| !required.contains(t))
                .collect();
            choose(
                0,
                rank - required.len(),
                &optional,
                &mut Vec::new(),
                &mut choices,
            );
            let masks = initial.clone();
            for above in choices {
                let mut exact = required.clone();
                exact.extend(above);
                exact.sort_unstable();
                let mut lower = vec![i32::MIN / 4; model.n];
                let mut upper_bounds = vec![i32::MAX / 4; model.n];
                lower[target] = packed;
                upper_bounds[target] = packed;
                for &t in &exact {
                    lower[t] = packed
                }
                for t in 0..model.n {
                    if t != target && !exact.contains(&t) {
                        upper_bounds[t] = packed
                    }
                }
                let mut work = DomainWork::new(250_000);
                let final_masks = if let Some(bounds) = Domains::condition_bounds_only_bounded(
                    &games,
                    &packed_base,
                    &(0..model.n).filter(|&t| t != target).collect::<Vec<_>>(),
                    rank,
                    packed,
                    &lower,
                    &upper_bounds,
                    &masks,
                    &mut work,
                ) {
                    if !bounds.feasible {
                        continue;
                    }
                    bounds.domains
                } else {
                    masks.clone()
                };
                // The score-ranked above set is checked against the full native
                // standings at draw time, including GD ties and random sorters.
                let region_upper = root_mass.min(mask_mass_upper(&laws, &final_masks));
                let baseline = JointPlan::build(
                    &model,
                    &laws,
                    &final_masks,
                    &lower,
                    &upper_bounds,
                    &[],
                    stride,
                    0,
                    Instant::now(),
                )?;
                regions.push(Region {
                    root_id,
                    target_points,
                    target_wins,
                    above: exact,
                    upper: region_upper,
                    impossible: false,
                    masks: final_masks,
                    lower,
                    upper_bounds,
                    plan: Some(baseline),
                    regime: None,
                    optimized: false,
                    epoch: 0,
                });
            }
            root_id += 1;
        }
        if regions.is_empty() {
            return Err("no target roots/cohorts survived conservative enumeration".into());
        }
        Ok(Self {
            model,
            laws,
            target,
            rank,
            regions,
            score_cache: ScoreCache::new(2_000_000),
            gd_cache: GdCache::new(100_000),
        })
    }

    pub fn describe(&self) -> Value {
        json!({
            "target":{"team_id":self.model.ids[self.target],"position":self.rank+1,"rank":self.rank},
            "fixture_count":self.model.fixtures.len(),
            "root_count":self.regions.iter().map(|r|r.root_id).max().unwrap_or(0)+1,
            "candidate_slots":self.regions.len(),
            "uncovered_upper":0.0,
            "regions":self.regions.iter().enumerate().map(|(id,r)|{let raw_b=r.plan.as_ref().map_or(1.0,|p|p.bound);let b=r.regime.as_ref().map_or(raw_b,|p|p.bound);json!({"id":id,"root_id":r.root_id,"target_points":r.target_points,"target_wins":r.target_wins,"opponent_ids":r.above.iter().map(|&t|self.model.ids[t]).collect::<Vec<_>>(),"impossible":r.impossible,"upper":r.upper,
                "plan":{"prepared":r.optimized || r.regime.is_some(),"B":b,"raw_B":raw_b,"mode":if r.regime.is_some(){"regime"}else{"joint"},"states":r.plan.as_ref().map_or(0,|p|p.states).saturating_add(r.regime.as_ref().map_or(0,|p|p.states)),
                    "k":r.regime.as_ref().map_or_else(||r.plan.as_ref().map_or(0,|p|p.selected.iter().filter(|&&t|t!=self.target).count()),|p|p.selected.iter().filter(|&&t|t!=self.target).count()),"complete":r.regime.as_ref().map_or_else(||r.plan.as_ref().is_some_and(|p|p.complete),|p|p.complete),"epoch":r.epoch}})}).collect::<Vec<_>>()
        })
    }

    /// Native packed-total feasibility model for an optional external prescreen.
    pub fn feasibility_model(&self, id: usize) -> Result<Value, String> {
        let r = self.regions.get(id).ok_or("unknown region id")?;
        let stride = self.packed_stride();
        let base: Vec<i32> = self
            .model
            .base
            .iter()
            .map(|c| c.points * stride + c.wins)
            .collect();
        let fixtures: Vec<Value> = self.model.fixtures.iter().enumerate().map(|(i, f)| {
            let outcomes = (0..3).map(|o| {
                let hp = if o == 2 { 3 } else if o == 1 { 1 } else { 0 };
                let ap = if o == 0 { 3 } else if o == 1 { 1 } else { 0 };
                json!({"home": hp * stride + i32::from(o == 2), "away": ap * stride + i32::from(o == 0)})
            }).collect::<Vec<_>>();
            json!({"home_id":self.model.ids[f.home],"away_id":self.model.ids[f.away],"mask":r.masks[i],"outcomes":outcomes})
        }).collect();
        Ok(
            json!({"id":id,"stride":stride,"team_ids":self.model.ids,"base_packed":base,
            "lower_packed":r.lower,"upper_packed":r.upper_bounds,"fixtures":fixtures}),
        )
    }

    /// Tighten a cohort's analytic event cap without changing its sampling
    /// proposal or epoch. Pairwise GD bounds apply only when all incident WDL
    /// outcomes are fixed; the pivot pass partitions the parent's masked
    /// support into disjoint WDL atoms and sums certified child caps.
    pub fn tighten(&mut self, id: usize, state_cap: usize, time_ms: u64) -> Result<Value, String> {
        let started = Instant::now();
        if id >= self.regions.len() {
            return Err("unknown region id".into());
        }
        if state_cap == 0 {
            return Err("state_cap must be positive".into());
        }
        let deadline = started + std::time::Duration::from_millis(time_ms);
        let parent_upper = self.regions[id].upper;
        if self.regions[id].impossible || parent_upper == 0.0 {
            return Ok(
                json!({"event_upper":0.0,"improved":false,"parent_upper_before":parent_upper,
                "mandatory_pair_bounds":[],"regime_pivot_id":null,"atom_count":0,
                "strict_upper":0.0,"tied_upper":0.0,"setup_ms":started.elapsed().as_secs_f64()*1000.0,"complete":true}),
            );
        }
        let region = &self.regions[id];
        let mut cap = parent_upper;
        let plan_bound = region
            .plan
            .as_ref()
            .ok_or("cannot tighten a region without its WDL proposal cap")?
            .bound;
        let mut mandatory = Vec::new();
        for &rival in &region.above {
            if Instant::now() >= deadline {
                break;
            }
            let mut rival_packed =
                self.model.base[rival].points * self.packed_stride() + self.model.base[rival].wins;
            let mut rival_fixed = true;
            for (fi, fixture) in self.model.fixtures.iter().enumerate() {
                if fixture.home != rival && fixture.away != rival {
                    continue;
                }
                let mask = region.masks[fi];
                if mask.count_ones() != 1 {
                    rival_fixed = false;
                    break;
                }
                let outcome = mask.trailing_zeros() as usize;
                let (points, wins) = if fixture.home == rival {
                    let (p, w) = match outcome {
                        0 => (0, 0),
                        1 => (1, 0),
                        _ => (3, 1),
                    };
                    (p, w)
                } else {
                    let (p, w) = match outcome {
                        0 => (3, 1),
                        1 => (1, 0),
                        _ => (0, 0),
                    };
                    (p, w)
                };
                rival_packed += points * self.packed_stride() + wins;
            }
            // GD only decides the rank when points and wins are equal. A
            // strict packed-PW lead can rank above regardless of GD.
            if !rival_fixed || rival_packed != region.lower[self.target] {
                continue;
            }
            if let Some(q) = self.gd_cache.probability(
                &self.model,
                &self.laws,
                &region.masks,
                rival,
                self.target,
                deadline,
            )? {
                let pair_cap = mul_up_positive(plan_bound, q.hi).min(1.0);
                cap = cap.min(pair_cap);
                mandatory.push(json!({"team_id":self.model.ids[rival],"gd_probability":{"lo":q.lo,"hi":q.hi},"event_upper":pair_cap}));
            }
        }

        let stride = self.packed_stride();
        let packed_base: Vec<i32> = self
            .model
            .base
            .iter()
            .map(|c| c.points * stride + c.wins)
            .collect();
        let games: Vec<RankGame> = self
            .model
            .fixtures
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let mut hg = [0; 3];
                let mut ag = [0; 3];
                for o in 0..3 {
                    let (hp, ap) = match o {
                        0 => (0, 3),
                        1 => (1, 1),
                        _ => (3, 0),
                    };
                    hg[o] = hp * stride + i32::from(o == 2);
                    ag[o] = ap * stride + i32::from(o == 0);
                }
                RankGame {
                    index: i,
                    home: f.home,
                    away: f.away,
                    prob: std::array::from_fn(|o| self.laws[i].outcomes[o].mid()),
                    hg,
                    ag,
                }
            })
            .collect();
        let rivals: Vec<usize> = (0..self.model.n).filter(|&t| t != self.target).collect();
        let target_packed = region.lower[self.target];
        let selected = region
            .plan
            .as_ref()
            .map(|p| p.selected.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| vec![self.target]);
        let mut pivot_id = None;
        let mut atom_count = 0usize;
        let mut strict_upper = 0.0;
        let mut tied_upper = 0.0;
        let mut regime_complete = false;
        let mut regime_candidates = Vec::new();

        // Try each required-above team as a pivot. Each atom fixes every
        // incident WDL result; exact propagation then conditions the entire
        // cohort, while the joint cap retains the parent selected constraints.
        for &pivot in &region.above {
            if Instant::now() >= deadline {
                break;
            }
            let incident: Vec<usize> = self
                .model
                .fixtures
                .iter()
                .enumerate()
                .filter_map(|(i, f)| (f.home == pivot || f.away == pivot).then_some(i))
                .collect();
            let options: Vec<Vec<u8>> = incident
                .iter()
                .map(|&fi| {
                    (0..3)
                        .filter(|&o| {
                            region.masks[fi] & (1 << o) != 0 && self.laws[fi].outcomes[o].hi > 0.0
                        })
                        .map(|o| o as u8)
                        .collect()
                })
                .collect();
            if options.iter().any(Vec::is_empty) {
                continue;
            }
            // Prune partial WDL assignments as soon as even their maximum
            // remaining packed PW total falls below the required rank cell.
            // This keeps the atom cap on compatible assignments, rather than
            // rejecting a pivot based on its unpruned Cartesian product.
            let mut suffix_max = vec![0i32; incident.len() + 1];
            for pos in (0..incident.len()).rev() {
                let fi = incident[pos];
                let f = &self.model.fixtures[fi];
                let gain = options[pos]
                    .iter()
                    .map(|&o| {
                        if f.home == pivot {
                            games[fi].hg[o as usize]
                        } else {
                            games[fi].ag[o as usize]
                        }
                    })
                    .max()
                    .unwrap_or(0);
                suffix_max[pos] = suffix_max[pos + 1] + gain;
            }
            fn enumerate_atoms(
                pos: usize,
                value: i32,
                pivot: usize,
                incident: &[usize],
                options: &[Vec<u8>],
                games: &[RankGame],
                suffix_max: &[i32],
                target: i32,
                prefix: &mut Vec<u8>,
                out: &mut Vec<(Vec<u8>, i32)>,
                too_many: &mut bool,
                deadline: Instant,
            ) {
                if *too_many || Instant::now() >= deadline {
                    *too_many = true;
                    return;
                }
                if value + suffix_max[pos] < target {
                    return;
                }
                if pos == incident.len() {
                    out.push((prefix.clone(), value));
                    if out.len() > 64 {
                        *too_many = true;
                    }
                    return;
                }
                let fi = incident[pos];
                let f = &games[fi];
                for &o in &options[pos] {
                    prefix.push(o);
                    let gain = if f.home == pivot {
                        f.hg[o as usize]
                    } else {
                        f.ag[o as usize]
                    };
                    enumerate_atoms(
                        pos + 1,
                        value + gain,
                        pivot,
                        incident,
                        options,
                        games,
                        suffix_max,
                        target,
                        prefix,
                        out,
                        too_many,
                        deadline,
                    );
                    prefix.pop();
                    if *too_many {
                        return;
                    }
                }
            }
            let mut atoms = Vec::new();
            let mut too_many = false;
            enumerate_atoms(
                0,
                packed_base[pivot],
                pivot,
                &incident,
                &options,
                &games,
                &suffix_max,
                target_packed,
                &mut Vec::new(),
                &mut atoms,
                &mut too_many,
                deadline,
            );
            if too_many || atoms.is_empty() || !atoms.iter().any(|(_, pw)| *pw == target_packed) {
                continue;
            }
            let mut strict = 0.0;
            let mut tied = 0.0;
            let mut failed = false;
            for (atom, packed) in &atoms {
                if Instant::now() >= deadline {
                    failed = true;
                    break;
                }
                let mut child_masks = region.masks.clone();
                for (&fi, &o) in incident.iter().zip(atom) {
                    child_masks[fi] = 1 << o;
                }
                let mut work = DomainWork::new(state_cap);
                let Some(domains) = Domains::condition_bounded(
                    &games,
                    &packed_base,
                    &rivals,
                    self.rank,
                    target_packed,
                    &region.lower,
                    &region.upper_bounds,
                    &child_masks,
                    &mut work,
                ) else {
                    failed = true;
                    break;
                };
                if !domains.feasible {
                    continue;
                }
                child_masks = domains.domains;
                let child = match JointPlan::build(
                    &self.model,
                    &self.laws,
                    &child_masks,
                    &region.lower,
                    &region.upper_bounds,
                    &selected,
                    stride,
                    state_cap,
                    deadline,
                ) {
                    Ok(p) => p,
                    Err(_) => {
                        failed = true;
                        break;
                    }
                };
                let atom_cap = if *packed == target_packed {
                    match self.gd_cache.probability(
                        &self.model,
                        &self.laws,
                        &child_masks,
                        pivot,
                        self.target,
                        deadline,
                    )? {
                        Some(q) => mul_up_positive(child.bound, q.hi).min(1.0),
                        None => {
                            failed = true;
                            break;
                        }
                    }
                } else {
                    child.bound
                };
                if *packed == target_packed {
                    tied = up(tied + atom_cap);
                } else {
                    strict = up(strict + atom_cap);
                }
            }
            if failed {
                continue;
            }
            let candidate = up(strict + tied).min(parent_upper);
            regime_candidates.push(json!({"pivot_id":self.model.ids[pivot],
                "event_upper":candidate,"atom_count":atoms.len(),
                "strict_upper":strict,"tied_upper":tied}));
            regime_complete = true;
            if candidate < cap {
                cap = candidate;
                pivot_id = Some(self.model.ids[pivot]);
                atom_count = atoms.len();
                strict_upper = strict;
                tied_upper = tied;
            }
            // Evaluate every eligible pivot within the shared deadline and
            // retain the strongest certified cap rather than stopping at the
            // first improvement.
        }
        let improved = cap < parent_upper;
        if improved {
            self.regions[id].upper = cap;
            if cap == 0.0 {
                self.regions[id].impossible = true;
            }
        }
        Ok(
            json!({"event_upper":cap,"improved":improved,"parent_upper_before":parent_upper,
            "mandatory_pair_bounds":mandatory,"regime_pivot_id":pivot_id,"atom_count":atom_count,
            "strict_upper":strict_upper,"tied_upper":tied_upper,"regime_candidates":regime_candidates,
            "setup_ms":started.elapsed().as_secs_f64()*1000.0,"complete":regime_complete || Instant::now() < deadline}),
        )
    }

    /// Stage a full-support native-law proposal. Preparation is transactional:
    /// all validation precedes replacing the current plan.
    pub fn prepare(
        &mut self,
        id: usize,
        k: usize,
        state_cap: usize,
        time_ms: u64,
    ) -> Result<Value, String> {
        let start = Instant::now();
        if id >= self.regions.len() {
            return Err("unknown region id".into());
        }
        if state_cap == 0 {
            return Err("state_cap must be positive".into());
        }
        if k > self.model.n.saturating_sub(1) {
            return Err("k exceeds rival count".into());
        }
        let region = &self.regions[id];
        let stride = self.packed_stride();
        let mut candidates = region.above.clone();
        let target_packed = region.lower[self.target];
        let rarity = |team: usize| {
            let mut dist = BTreeMap::from([(
                self.model.base[team].points * stride + self.model.base[team].wins,
                1.0f64,
            )]);
            for (i, f) in self.model.fixtures.iter().enumerate() {
                if f.home != team && f.away != team {
                    continue;
                }
                let raw: Vec<f64> = (0..3)
                    .map(|o| {
                        if region.masks[i] & (1 << o) != 0 {
                            self.laws[i].outcomes[o].mid()
                        } else {
                            0.0
                        }
                    })
                    .collect();
                let z = raw.iter().sum::<f64>();
                if z <= 0.0 {
                    return 0.0;
                }
                let mut next = BTreeMap::new();
                for (&value, &mass) in &dist {
                    for o in 0..3 {
                        let q = raw[o] / z;
                        if q == 0.0 {
                            continue;
                        }
                        let gain = if f.home == team {
                            if o == 0 {
                                0
                            } else if o == 1 {
                                stride
                            } else {
                                3 * stride + 1
                            }
                        } else if o == 0 {
                            3 * stride + 1
                        } else if o == 1 {
                            stride
                        } else {
                            0
                        };
                        *next.entry(value + gain).or_insert(0.0) += mass * q;
                    }
                }
                dist = next;
            }
            dist.iter()
                .filter(|(score, _)| **score >= target_packed)
                .map(|(_, p)| *p)
                .sum::<f64>()
        };
        let mut ranked: Vec<_> = candidates
            .drain(..)
            .map(|team| (team, rarity(team)))
            .collect();
        ranked.sort_by(|a, b| a.1.total_cmp(&b.1));
        let mut candidates: Vec<_> = ranked.into_iter().take(k).map(|(team, _)| team).collect();
        // The target's exact packed root total is part of every region event;
        // include it in the joint state so its root mass is sampled and capped
        // once along with the selected rival prefix constraints.
        candidates.push(self.target);
        let deadline = Instant::now() + std::time::Duration::from_millis(time_ms);
        let candidate = JointPlan::build(
            &self.model,
            &self.laws,
            &region.masks,
            &region.lower,
            &region.upper_bounds,
            &candidates,
            (0..self.model.n)
                .map(|t| {
                    self.model.base[t].wins
                        + self
                            .model
                            .fixtures
                            .iter()
                            .filter(|f| f.home == t || f.away == t)
                            .count() as i32
                })
                .max()
                .unwrap_or(0)
                + 1,
            state_cap,
            deadline,
        )?;
        if !candidate.bound.is_finite() || candidate.bound < 0.0 {
            return Err("candidate has an invalid certified bound".into());
        }
        if candidate.bound == 0.0 {
            if !candidate.complete {
                return Err("incomplete candidate cannot certify a zero event".into());
            }
            self.regions[id].epoch = self.regions[id].epoch.saturating_add(1);
            self.regions[id].impossible = true;
            self.regions[id].upper = 0.0;
            self.regions[id].regime = None;
            let receipt = json!({"accepted":true,"B":0.0,"states":candidate.states,"selected_rival_ids":candidate.selected.iter().filter(|&&t|t!=self.target).map(|&t|self.model.ids[t]).collect::<Vec<_>>(),"setup_ms":start.elapsed().as_secs_f64()*1000.0,"complete":true,"epoch":self.regions[id].epoch,"impossible":true});
            self.regions[id].plan = Some(candidate);
            self.regions[id].optimized = true;
            return Ok(receipt);
        }
        let effective_before = self.regions[id].regime.as_ref().map_or_else(
            || self.regions[id].plan.as_ref().map_or(1.0, |old| old.bound),
            |old| old.bound,
        );
        if candidate.bound >= effective_before * (1.0 - 1e-9) {
            let old = self.regions[id].plan.as_ref();
            return Ok(
                json!({"accepted":false,"B":effective_before,"candidate_B":candidate.bound,"states":candidate.states,"selected_rival_ids":old.map(|p|p.selected.iter().filter(|&&t|t!=self.target).map(|&t|self.model.ids[t]).collect::<Vec<_>>()).unwrap_or_default(),"setup_ms":start.elapsed().as_secs_f64()*1000.0,"complete":candidate.complete,"epoch":self.regions[id].epoch}),
            );
        }
        self.regions[id].epoch = self.regions[id].epoch.saturating_add(1);
        let selected_rival_ids = candidate
            .selected
            .iter()
            .filter(|&&t| t != self.target)
            .map(|&t| self.model.ids[t])
            .collect::<Vec<_>>();
        let receipt = json!({"accepted":true,"B":candidate.bound,"states":candidate.states,
            "selected_rival_ids":selected_rival_ids,"setup_ms":start.elapsed().as_secs_f64()*1000.0,
            "complete":candidate.complete,"epoch":self.regions[id].epoch});
        self.regions[id].plan = Some(candidate);
        self.regions[id].regime = None;
        self.regions[id].optimized = true;
        Ok(receipt)
    }

    /// Draw exactly n complete joint proposals and integrate their native
    /// conditional score event. Each stream estimates the absolute cohort mass.
    pub fn sample(
        &mut self,
        id: usize,
        n: usize,
        seed: i64,
        depth: usize,
    ) -> Result<Value, String> {
        let start = Instant::now();
        if id >= self.regions.len() {
            return Err("unknown region id".into());
        }
        if n == 0 {
            return Err("n must be positive".into());
        }
        if self.regions[id].impossible {
            return Ok(
                json!({"id":id,"n":n,"epoch":self.regions[id].epoch,"B":0.0,"mode":if self.regions[id].regime.is_some(){"regime"}else{"joint"},"states":0,"sum_lower":0.0,"sum_upper":0.0,"sum_lower_sq":0.0,"sum_upper_sq":0.0,"lower_exceedances":vec![0u64;241],"upper_exceedances":vec![0u64;241],"positive_draws":0,"elapsed_ms":start.elapsed().as_secs_f64()*1000.0,"normalized":true,"absolute_stream":false}),
            );
        }
        if self.regions[id].plan.is_none() {
            let region = &self.regions[id];
            let stride = self.packed_stride();
            let deadline = Instant::now();
            self.regions[id].plan = Some(JointPlan::build(
                &self.model,
                &self.laws,
                &region.masks,
                &region.lower,
                &region.upper_bounds,
                &[self.target],
                stride,
                0,
                deadline,
            )?);
        }
        let plan = self.regions[id].plan.as_ref().unwrap();
        let regime_plan = self.regions[id].regime.as_ref();
        let b = regime_plan.map_or(plan.bound, |p| p.bound);
        if b <= 0.0 || !b.is_finite() {
            return Err("installed proposal has an invalid weight cap".into());
        }
        let mut rng = Rng::new(seed);
        let mut sum_lower = 0.0;
        let mut sum_upper = 0.0;
        let mut sum_lower_sq = 0.0;
        let mut sum_upper_sq = 0.0;
        let mut lower_counts = vec![0u64; 241];
        let mut upper_counts = vec![0u64; 241];
        let mut positives = 0u64;
        for _ in 0..n {
            let region = &self.regions[id];
            let (outcomes, weight, fixed_scores) = if let Some(regime) = regime_plan {
                regime.draw(&self.model, &self.laws, &mut rng)?
            } else {
                let (outcomes, weight) = plan.draw(&self.laws, &mut rng)?;
                (outcomes, weight, vec![None; self.model.fixtures.len()])
            };
            if weight.lo < 0.0 || !weight.hi.is_finite() {
                return Err("proposal returned an invalid likelihood interval".into());
            }
            let mut root_campaigns = self.model.base.clone();
            for (i, &o) in outcomes.iter().enumerate() {
                self.model.add(
                    &mut root_campaigns,
                    i,
                    match o {
                        0 => [0, 1],
                        1 => [0, 0],
                        2 => [1, 0],
                        _ => return Err("proposal returned invalid outcome".into()),
                    },
                );
            }
            let target_campaign = &root_campaigns[self.target];
            let root_matches = target_campaign.points == region.target_points
                && target_campaign.wins == region.target_wins;
            let score = if weight.hi == 0.0 || !root_matches {
                Interval::point(0.0)
            } else {
                score::integrate_fixed(
                    &self.model,
                    &self.laws,
                    &outcomes,
                    &region.above,
                    self.target,
                    depth,
                    &mut rng,
                    &mut self.score_cache,
                    &fixed_scores,
                )?
            };
            let normalized = weight.div(Interval::point(b));
            if weight.lo > b {
                return Err(format!(
                    "observed likelihood lower bound exceeds certified cap: {} > {b}",
                    weight.lo
                ));
            }
            // `weight.hi` may exceed B by a rounding ulp because the sample's
            // interval product is wider than the direct recurrence cap. Since
            // the cap certifies the true ratio and weight.lo<=B, tighten only
            // that enclosure to the proven [0,1] normalized range.
            let normalized = Interval {
                lo: normalized.lo.max(0.0),
                hi: normalized.hi.min(1.0),
            };
            let y = normalized.mul(score).clamp_probability();
            sum_lower += y.lo;
            sum_upper += y.hi;
            sum_lower_sq += y.lo * y.lo;
            sum_upper_sq += y.hi * y.hi;
            if y.hi > 0.0 {
                positives += 1
            }
            for j in 0..=240 {
                let threshold = 2f64.powi(-(j as i32));
                if y.lo >= threshold {
                    lower_counts[j] += 1
                }
                if y.hi >= threshold {
                    upper_counts[j] += 1
                }
            }
        }
        Ok(json!({"id":id,"n":n,"epoch":self.regions[id].epoch,"B":b,
            "mode":if regime_plan.is_some(){"regime"}else{"joint"},"states":plan.states.saturating_add(regime_plan.map_or(0,|p|p.states)),
            "sum_lower":sum_lower,"sum_upper":sum_upper,"sum_lower_sq":sum_lower_sq,"sum_upper_sq":sum_upper_sq,
            "lower_exceedances":lower_counts,"upper_exceedances":upper_counts,"positive_draws":positives,
            "score_cache_states":self.score_cache.states(),"event_lower":null,"absolute_stream":false,"normalized":true,
            "elapsed_ms":start.elapsed().as_secs_f64()*1000.0}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pal12_fixture_certifies_one_loss_root_and_exact_cohorts() {
        let request: Request = serde_json::from_str(include_str!(
            "../../tests/fixtures/group-16498-2026-10-10.json"
        ))
        .unwrap();
        let engine = Engine::new(request, 16, 12).unwrap();
        assert_eq!(engine.regions.len(), 40);
        assert!(engine
            .regions
            .iter()
            .all(|r| r.root_id == 0 && r.target_points == 60 && r.target_wins == 17));
        assert!(engine.regions.iter().all(|r| {
            !r.optimized
                && r.plan
                    .as_ref()
                    .is_some_and(|p| p.bound.is_finite() && p.bound > 0.0)
        }));
        let target_fixtures: Vec<_> = engine
            .model
            .fixtures
            .iter()
            .enumerate()
            .filter(|(_, f)| f.home == engine.target || f.away == engine.target)
            .collect();
        assert_eq!(target_fixtures.len(), 9);
        for region in &engine.regions {
            for &(i, f) in &target_fixtures {
                let loss = if f.home == engine.target { 0 } else { 2 };
                assert_eq!(
                    region.masks[i],
                    1 << loss,
                    "target-root propagation must force each loss"
                );
            }
        }
    }

    #[test]
    fn tighten_changes_only_analytic_cap_not_plan_or_epoch() {
        let request: Request = serde_json::from_str(include_str!(
            "../../tests/fixtures/group-16498-2026-10-10.json"
        ))
        .unwrap();
        let mut engine = Engine::new(request, 16, 12).unwrap();
        let before_plan = engine.regions[0].plan.as_ref().unwrap().bound;
        let before_epoch = engine.regions[0].epoch;
        let before_masks = engine.regions[0].masks.clone();
        let before_upper = engine.regions[0].upper;
        let receipt = engine.tighten(0, 250_000, 250).unwrap();
        assert_eq!(engine.regions[0].plan.as_ref().unwrap().bound, before_plan);
        assert_eq!(engine.regions[0].epoch, before_epoch);
        assert_eq!(engine.regions[0].masks, before_masks);
        assert!(receipt["event_upper"].as_f64().unwrap() <= before_upper);
        assert_eq!(
            receipt["parent_upper_before"].as_f64().unwrap(),
            before_upper
        );
    }

    #[test]
    fn strict_packed_pw_leader_does_not_receive_a_gd_only_cap() {
        let request: Request = serde_json::from_str(include_str!(
            "../../tests/fixtures/group-16498-2026-10-10.json"
        ))
        .unwrap();
        let mut engine = Engine::new(request, 16, 12).unwrap();
        let stride = engine.packed_stride();
        let mut totals: Vec<i32> = engine
            .model
            .base
            .iter()
            .map(|c| c.points * stride + c.wins)
            .collect();
        for (fi, fixture) in engine.model.fixtures.iter().enumerate() {
            engine.regions[0].masks[fi] = 1 << 2;
            totals[fixture.home] += 3 * stride + 1;
        }
        let rival = (0..engine.model.n)
            .find(|&t| t != engine.target && totals[t] > totals[engine.target])
            .expect("fixture has a strict-PW leader under fixed home wins");
        engine.regions[0].above = vec![rival];
        engine.regions[0].lower[rival] = totals[engine.target];
        engine.regions[0].upper_bounds[rival] = i32::MAX / 4;
        engine.regions[0].upper = 1.0;
        let receipt = engine.tighten(0, 250_000, 100).unwrap();
        assert!(receipt["mandatory_pair_bounds"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn team18_pivot_prunes_to_ten_compatible_wdl_atoms() {
        let request: Request = serde_json::from_str(include_str!(
            "../../tests/fixtures/group-16498-2026-10-10.json"
        ))
        .unwrap();
        let mut engine = Engine::new(request, 16, 12).unwrap();
        let rival = *engine.model.indices.get(&18).unwrap();
        let id = engine
            .regions
            .iter()
            .position(|r| r.above.contains(&rival))
            .unwrap();
        engine.regions[id].above = vec![rival];
        let receipt = engine.tighten(id, 250_000, 2_000).unwrap();
        assert_eq!(receipt["regime_pivot_id"].as_i64(), Some(18));
        assert_eq!(receipt["atom_count"].as_u64(), Some(10));
    }

    #[test]
    fn tighten_keeps_best_of_all_completed_pivots() {
        let request: Request = serde_json::from_str(include_str!(
            "../../tests/fixtures/group-16498-2026-10-10.json"
        ))
        .unwrap();
        let mut engine = Engine::new(request, 16, 12).unwrap();
        let rival = *engine.model.indices.get(&18).unwrap();
        let id = engine
            .regions
            .iter()
            .position(|r| r.above.contains(&rival))
            .unwrap();
        // Repeating the same valid pivot here exercises the all-candidates
        // reduction deterministically without changing its atom law.
        engine.regions[id].above = vec![rival, rival];
        let plan_bound = engine.regions[id].plan.as_ref().unwrap().bound;
        let epoch = engine.regions[id].epoch;
        let parent_upper = engine.regions[id].upper;
        let receipt = engine.tighten(id, 250_000, 3_000).unwrap();
        let candidates = receipt["regime_candidates"].as_array().unwrap();
        assert_eq!(candidates.len(), 2);
        let candidate_min = candidates
            .iter()
            .map(|c| c["event_upper"].as_f64().unwrap())
            .fold(f64::INFINITY, f64::min);
        assert!(receipt["event_upper"].as_f64().unwrap() <= candidate_min);
        assert!(receipt["event_upper"].as_f64().unwrap() <= parent_upper);
        assert_eq!(engine.regions[id].plan.as_ref().unwrap().bound, plan_bound);
        assert_eq!(engine.regions[id].epoch, epoch);
    }

    #[test]
    fn regime_candidate_preserves_support_and_commits_atomically() {
        let request: Request = serde_json::from_str(include_str!(
            "../../tests/fixtures/group-16498-2026-10-10.json"
        ))
        .unwrap();
        let mut engine = Engine::new(request, 16, 12).unwrap();
        let rival = *engine.model.indices.get(&18).unwrap();
        let id = engine
            .regions
            .iter()
            .position(|r| r.above.contains(&rival))
            .unwrap();
        let _raw_prepare = engine.prepare(id, 1, 250_000, 500).unwrap();
        let before_plan = engine.regions[id].plan.as_ref().unwrap().bound;
        let before_epoch = engine.regions[id].epoch;
        let before_regime = engine.regions[id].regime.as_ref().map(|p| p.bound);
        let receipt = engine.regime(id, 250_000, 1_500).unwrap();
        assert_eq!(
            receipt["accepted"], true,
            "Pal12 regime candidate: {receipt}"
        );
        let b = receipt["B"].as_f64().unwrap();
        assert!(b <= 0.75 * before_regime.unwrap_or(before_plan));
        assert!(receipt["atom_count"].as_u64().unwrap() > 0);
        assert_eq!(engine.regions[id].epoch, before_epoch + 1);
        assert_eq!(engine.regions[id].plan.as_ref().unwrap().bound, before_plan);
        let sample = engine.sample(id, 2, 81, 2).unwrap();
        assert_eq!(sample["mode"], "regime");
        assert_eq!(sample["n"], 2);
        assert_eq!(
            sample["states"].as_u64(),
            Some(
                engine.regions[id].plan.as_ref().unwrap().states as u64
                    + engine.regions[id].regime.as_ref().unwrap().states as u64
            )
        );
        let installed_b = engine.regions[id].regime.as_ref().unwrap().bound;
        let installed_epoch = engine.regions[id].epoch;
        let rejected = engine.regime(id, 250_000, 1_500).unwrap();
        assert_eq!(rejected["accepted"], false);
        assert_eq!(
            engine.regions[id].regime.as_ref().unwrap().bound,
            installed_b
        );
        assert_eq!(engine.regions[id].epoch, installed_epoch);
        assert_eq!(engine.regions[id].plan.as_ref().unwrap().bound, before_plan);
        let failed = engine.regime(id, 250_000, 0).unwrap();
        assert_eq!(failed["accepted"], false);
        assert_eq!(
            engine.regions[id].regime.as_ref().unwrap().bound,
            installed_b
        );
        assert_eq!(engine.regions[id].epoch, installed_epoch);
    }
}
