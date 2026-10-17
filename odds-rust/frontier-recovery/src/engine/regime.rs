use super::Engine;
use crate::{
    gd_bounds::GdCache,
    joint::Plan as JointPlan,
    law::{categorical, categorical_probabilities, Interval, Law},
    pair::PairPlan,
};
use golaberto_odds::{
    domains::{DomainWork, Domains},
    lookahead::RankGame,
    model::Model,
    rng::Rng,
};
use serde_json::{json, Value};
use std::time::Instant;

pub(super) struct RegimePlan {
    pub bound: f64,
    pub states: usize,
    pub complete: bool,
    pub pivot: usize,
    pub selected: Vec<usize>,
    weights: Vec<f64>,
    atoms: Vec<Atom>,
}

struct Atom {
    plan: JointPlan,
    pair: Option<PairPlan>,
    q: Interval,
}

impl RegimePlan {
    pub fn draw(
        &self,
        model: &Model,
        laws: &[Law],
        rng: &mut Rng,
    ) -> Result<(Vec<u8>, Interval, Vec<Option<[i32; 2]>>), String> {
        if self.atoms.is_empty() {
            return Err("cannot draw from a zero-mass regime".into());
        }
        let (index, pi) = categorical(&self.weights, rng)?;
        if pi.lo <= 0.0 {
            return Err("regime mixture ticket has zero lower probability".into());
        }
        let atom = &self.atoms[index];
        let (outcomes, wdl_weight) = atom.plan.draw(laws, rng)?;
        if wdl_weight.hi == 0.0 {
            return Ok((
                outcomes,
                Interval::point(0.0),
                vec![None; model.fixtures.len()],
            ));
        }
        let fixed_scores = if let Some(pair) = &atom.pair {
            pair.draw(model, rng)?
        } else {
            vec![None; model.fixtures.len()]
        };
        let weight = wdl_weight.mul(atom.q).div(pi);
        Ok((outcomes, weight, fixed_scores))
    }
}

fn up(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        x
    } else {
        f64::from_bits(x.to_bits() + 1)
    }
}
fn mul_up(a: f64, b: f64) -> f64 {
    if a == 0.0 || b == 0.0 {
        0.0
    } else {
        let x = a * b;
        if x == 0.0 {
            f64::from_bits(1)
        } else {
            up(x)
        }
    }
}

fn make_games(model: &Model, laws: &[Law], stride: i32) -> Vec<RankGame> {
    model
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
                prob: std::array::from_fn(|o| laws[i].outcomes[o].mid()),
                hg,
                ag,
            }
        })
        .collect()
}

fn candidate_for_pivot(
    model: &Model,
    laws: &[Law],
    gd_cache: &mut GdCache,
    masks: &[u8],
    lower: &[i32],
    upper: &[i32],
    rivals: &[usize],
    rank: usize,
    target: usize,
    target_packed: i32,
    pivot: usize,
    selected: &[usize],
    games: &[RankGame],
    packed_base: &[i32],
    stride: i32,
    state_cap: usize,
    deadline: Instant,
) -> Result<Option<RegimePlan>, String> {
    let incident: Vec<usize> = model
        .fixtures
        .iter()
        .enumerate()
        .filter_map(|(i, f)| (f.home == pivot || f.away == pivot).then_some(i))
        .collect();
    let options: Vec<Vec<u8>> = incident
        .iter()
        .map(|&fi| {
            (0..3)
                .filter(|&o| masks[fi] & (1 << o) != 0 && laws[fi].outcomes[o].hi > 0.0)
                .map(|o| o as u8)
                .collect()
        })
        .collect();
    if options.iter().any(Vec::is_empty) {
        return Ok(None);
    }
    let mut suffix_max = vec![0i32; incident.len() + 1];
    for pos in (0..incident.len()).rev() {
        let fi = incident[pos];
        let f = &model.fixtures[fi];
        suffix_max[pos] = suffix_max[pos + 1]
            + options[pos]
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
    }
    fn visit(
        pos: usize,
        value: i32,
        pivot: usize,
        incident: &[usize],
        options: &[Vec<u8>],
        games: &[RankGame],
        suffix: &[i32],
        target: i32,
        prefix: &mut Vec<u8>,
        atoms: &mut Vec<(Vec<u8>, i32)>,
        overflow: &mut bool,
        deadline: Instant,
    ) {
        if *overflow || Instant::now() >= deadline {
            *overflow = true;
            return;
        }
        if value + suffix[pos] < target {
            return;
        }
        if pos == incident.len() {
            atoms.push((prefix.clone(), value));
            if atoms.len() > 64 {
                *overflow = true;
            }
            return;
        }
        let f = &games[incident[pos]];
        for &o in &options[pos] {
            prefix.push(o);
            let gain = if f.home == pivot {
                f.hg[o as usize]
            } else {
                f.ag[o as usize]
            };
            visit(
                pos + 1,
                value + gain,
                pivot,
                incident,
                options,
                games,
                suffix,
                target,
                prefix,
                atoms,
                overflow,
                deadline,
            );
            prefix.pop();
            if *overflow {
                return;
            }
        }
    }
    let mut atoms = Vec::new();
    let mut overflow = false;
    visit(
        0,
        packed_base[pivot],
        pivot,
        &incident,
        &options,
        games,
        &suffix_max,
        target_packed,
        &mut Vec::new(),
        &mut atoms,
        &mut overflow,
        deadline,
    );
    if overflow || atoms.is_empty() || !atoms.iter().any(|(_, pw)| *pw == target_packed) {
        return Ok(None);
    }

    let mut retained = Vec::<Atom>::new();
    let mut scales = Vec::<f64>::new();
    let mut states = 0usize;
    for (atom, pw) in atoms {
        if Instant::now() >= deadline {
            return Ok(None);
        }
        let mut child_masks = masks.to_vec();
        for (&fi, &o) in incident.iter().zip(&atom) {
            child_masks[fi] = 1 << o;
        }
        let mut work = DomainWork::new(state_cap.saturating_sub(states));
        let Some(domains) = Domains::condition_bounded(
            games,
            packed_base,
            rivals,
            rank,
            target_packed,
            lower,
            upper,
            &child_masks,
            &mut work,
        ) else {
            return Ok(None);
        };
        if !domains.feasible {
            continue;
        }
        child_masks = domains.domains;
        let remaining = state_cap.saturating_sub(states);
        let child = JointPlan::build(
            model,
            laws,
            &child_masks,
            lower,
            upper,
            selected,
            stride,
            remaining,
            deadline,
        )?;
        if child.bound == 0.0 {
            if child.complete {
                continue;
            }
            return Ok(None);
        }
        states = states.saturating_add(child.states);
        if states > state_cap {
            return Ok(None);
        }
        let (pair, q, scale) = if pw == target_packed {
            let Some(pair) =
                PairPlan::build(model, laws, &child_masks, pivot, target, deadline, gd_cache)?
            else {
                return Ok(None);
            };
            let q = pair.normalizer;
            if q.hi == 0.0 {
                continue;
            }
            if q.lo < 0.001 {
                return Ok(None);
            }
            (Some(pair), q, mul_up(child.bound, q.hi))
        } else {
            (None, Interval::point(1.0), child.bound)
        };
        if scale == 0.0 {
            continue;
        }
        retained.push(Atom {
            plan: child,
            pair,
            q,
        });
        scales.push(scale);
    }
    if retained.is_empty() {
        return Ok(Some(RegimePlan {
            bound: 0.0,
            states,
            complete: true,
            pivot,
            selected: selected.to_vec(),
            weights: Vec::new(),
            atoms: retained,
        }));
    }
    let probabilities = categorical_probabilities(&scales)?;
    let mut bound = 0.0f64;
    for (scale, pi) in scales.iter().zip(&probabilities) {
        if pi.lo <= 0.0 {
            return Ok(None);
        }
        let atom_cap = up(*scale / pi.lo);
        bound = bound.max(atom_cap);
    }
    Ok(Some(RegimePlan {
        bound,
        states,
        complete: true,
        pivot,
        selected: selected.to_vec(),
        weights: scales,
        atoms: retained,
    }))
}

impl Engine {
    pub fn regime(&mut self, id: usize, state_cap: usize, time_ms: u64) -> Result<Value, String> {
        let start = Instant::now();
        if id >= self.regions.len() {
            return Err("unknown region id".into());
        }
        if state_cap == 0 {
            return Err("state_cap must be positive".into());
        }
        let deadline = start + std::time::Duration::from_millis(time_ms);
        let effective_before = {
            let r = &self.regions[id];
            r.regime
                .as_ref()
                .map_or_else(|| r.plan.as_ref().map_or(1.0, |p| p.bound), |p| p.bound)
        };
        let raw_states = self.regions[id].plan.as_ref().map_or(0, |p| p.states);
        if raw_states > state_cap {
            return Ok(
                json!({"accepted":false,"B":effective_before,"candidate_B":null,"states":raw_states,
                "atoms":0,"atom_count":0,"pivot_id":null,"selected_rival_ids":[],"mode":"joint",
                "complete":false,"setup_ms":start.elapsed().as_secs_f64()*1000.0,"epoch":self.regions[id].epoch,"candidates":[]}),
            );
        }
        let child_state_cap = state_cap.saturating_sub(raw_states);
        let (candidate, candidate_receipts) = {
            let region = &self.regions[id];
            let model = &self.model;
            let laws = &self.laws;
            let stride = self.packed_stride();
            let packed_base: Vec<i32> = model
                .base
                .iter()
                .map(|c| c.points * stride + c.wins)
                .collect();
            let games = make_games(model, laws, stride);
            let rivals: Vec<usize> = (0..model.n).filter(|&t| t != self.target).collect();
            let selected = region
                .plan
                .as_ref()
                .map(|p| p.selected.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| vec![self.target]);
            let target_packed = region.lower[self.target];
            let mut best: Option<RegimePlan> = None;
            let mut receipts = Vec::new();
            for &pivot in &region.above {
                if Instant::now() >= deadline {
                    break;
                }
                let attempt = candidate_for_pivot(
                    model,
                    laws,
                    &mut self.gd_cache,
                    &region.masks,
                    &region.lower,
                    &region.upper_bounds,
                    &rivals,
                    self.rank,
                    self.target,
                    target_packed,
                    pivot,
                    &selected,
                    &games,
                    &packed_base,
                    stride,
                    child_state_cap,
                    deadline,
                );
                match attempt {
                    Ok(Some(plan)) => {
                        receipts.push(json!({"pivot_id":model.ids[pivot],"B":plan.bound,
                            "states":raw_states.saturating_add(plan.states),"atom_count":plan.atoms.len(),"complete":plan.complete}));
                        if best.as_ref().map_or(true, |b| plan.bound < b.bound) {
                            best = Some(plan);
                        }
                    }
                    Ok(None) => {}
                    Err(e) => return Err(e),
                }
            }
            (best, receipts)
        };
        let Some(candidate) = candidate else {
            return Ok(
                json!({"accepted":false,"B":effective_before,"candidate_B":null,"states":raw_states,"atoms":0,"atom_count":0,"pivot_id":null,"selected_rival_ids":[],"mode":if self.regions[id].regime.is_some(){"regime"}else{"joint"},"complete":false,"setup_ms":start.elapsed().as_secs_f64()*1000.0,"epoch":self.regions[id].epoch,"candidates":candidate_receipts}),
            );
        };
        let improved = candidate.bound <= 0.75 * effective_before;
        let states = raw_states.saturating_add(candidate.states);
        let atoms = candidate.atoms.len();
        let pivot_id = self.model.ids[candidate.pivot];
        let selected_rival_ids = candidate
            .selected
            .iter()
            .filter(|&&t| t != self.target)
            .map(|&t| self.model.ids[t])
            .collect::<Vec<_>>();
        if improved {
            self.regions[id].epoch = self.regions[id].epoch.saturating_add(1);
            if candidate.bound == 0.0 {
                self.regions[id].impossible = true;
                self.regions[id].upper = 0.0;
            }
            self.regions[id].regime = Some(candidate);
        }
        Ok(
            json!({"accepted":improved,"B":if improved {self.regions[id].regime.as_ref().unwrap().bound} else {effective_before},
            "candidate_B":candidate_receipts.iter().map(|c|c["B"].as_f64().unwrap_or(f64::INFINITY)).fold(f64::INFINITY,f64::min),
            "states":states,"atoms":atoms,"atom_count":atoms,"pivot_id":pivot_id,"selected_rival_ids":selected_rival_ids,
            "mode":if improved {"regime"}else if self.regions[id].regime.is_some(){"regime"}else{"joint"},
            "complete":true,"setup_ms":start.elapsed().as_secs_f64()*1000.0,"epoch":self.regions[id].epoch,"candidates":candidate_receipts}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use golaberto_odds::model::{Model, Request};
    use std::time::Duration;

    fn tiny() -> Model {
        let request: Request = serde_json::from_str(
            r#"{"id":1,"phase":{"sort":"pt,w,gd,gf,name","championship":{"point_win":3,"point_draw":1,"point_loss":0}},"games":[{"id":1,"home_id":1,"away_id":2,"home_power":1.0,"away_power":1.0,"played":false}],"team_groups":[{"team_id":1},{"team_id":2}]}"#,
        ).unwrap();
        Model::new(request).unwrap()
    }

    #[test]
    fn nonuniform_atom_mixture_identity_and_cap() {
        let model = tiny();
        let law = Law::new(1.0, 1.0).unwrap();
        let laws = vec![law.clone()];
        let lower = vec![i32::MIN / 4; model.n];
        let upper = vec![i32::MAX / 4; model.n];
        let stride = 16;
        let deadline = Instant::now() + Duration::from_secs(1);
        let left = JointPlan::build(
            &model,
            &laws,
            &[1 << 1],
            &lower,
            &upper,
            &[],
            stride,
            0,
            deadline,
        )
        .unwrap();
        let right = JointPlan::build(
            &model,
            &laws,
            &[1 << 2],
            &lower,
            &upper,
            &[],
            stride,
            0,
            deadline,
        )
        .unwrap();
        let scales = vec![left.bound, right.bound];
        let pi = categorical_probabilities(&scales).unwrap();
        assert!(
            pi[0].mid() != pi[1].mid(),
            "the mixture fixture must be nonuniform"
        );
        let bound = up((0..2).map(|i| scales[i] / pi[i].lo).fold(0.0, f64::max));
        assert!(scales[0] / pi[0].lo <= bound && scales[1] / pi[1].lo <= bound);
        let plans = RegimePlan {
            bound,
            states: 0,
            complete: true,
            pivot: 0,
            selected: Vec::new(),
            weights: scales.clone(),
            atoms: vec![
                Atom {
                    plan: left,
                    pair: None,
                    q: Interval::point(1.0),
                },
                Atom {
                    plan: right,
                    pair: None,
                    q: Interval::point(1.0),
                },
            ],
        };
        // Exact mixture expectation for the union of the two atom events
        // recovers their native-law mass, with the ticket-probability interval
        // canceling between mixture selection and likelihood correction.
        let expected = pi[0]
            .mul(law.outcomes[1].div(pi[0]))
            .add(pi[1].mul(law.outcomes[2].div(pi[1])));
        let native = law.outcomes[1].add(law.outcomes[2]);
        assert!(expected.lo <= native.lo && expected.hi >= native.hi);
        let mut rng = Rng::new(701);
        for _ in 0..32 {
            let (outcomes, weight, scores) = plans.draw(&model, &laws, &mut rng).unwrap();
            assert!(weight.lo >= 0.0 && weight.lo <= bound);
            assert!(scores.iter().all(Option::is_none));
            assert!(outcomes[0] == 1 || outcomes[0] == 2);
        }
    }
}
