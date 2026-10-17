//! Joint fixture-level importance proposals for rare packed-table events.
//!
//! A plan is deliberately immutable after construction.  In particular, an
//! edge which was unresolved during construction always enters its native
//! masked suffix, even if another path later caused a nearby state to be
//! memoized.  This is what makes the cap describe the proposal actually used
//! by `draw`.

use crate::law::{categorical, categorical_probabilities, Interval, Law};
use golaberto_odds::{model::Model, rng::Rng};
use std::{collections::HashMap, time::Instant};

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
    fallback_caps: Vec<f64>, // suffix cap indexed by order position
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

        // Place fixtures touching more selected teams first, then stable index.
        let mut order: Vec<usize> = (0..nf).collect();
        order.sort_by_key(|&i| {
            let f = &model.fixtures[i];
            let touches = usize::from(seen[f.home]) + usize::from(seen[f.away]);
            (std::cmp::Reverse(touches), i)
        });

        // The native masked suffix has a fixed proposal and therefore a fixed
        // product cap, including all fixtures after a fallback edge.
        let mut fallback_caps = vec![1.0; nf + 1];
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
        };

        // Empty selection intentionally means the native masked proposal.
        if plan.selected.is_empty() {
            plan.bound = plan.fallback_caps[0];
            return Ok(plan);
        }

        let mut memo: HashMap<(usize, Vec<i32>), usize> = HashMap::new();
        let mut limit_hit = false;
        let start_state: Vec<i32> = plan
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
                let accept = plan
                    .selected
                    .iter()
                    .enumerate()
                    .all(|(j, &t)| state[j] >= plan.lower[t] && state[j] <= plan.upper[t]);
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
                let mut feasible = suffix_possible[pos + 1];
                for (j, &t) in plan.selected.iter().enumerate() {
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
                )?;
                let child_potential = child.map_or(1.0, |id| plan.nodes[id].potential);
                // `None` is an unresolved edge, not an infeasible edge: its
                // nominal potential is one and its sampler is fixed fallback.
                let sticky_fallback = feasible && child.is_none();
                let effective_potential = if sticky_fallback {
                    1.0
                } else {
                    child_potential
                };
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
                    None => 1.0,
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
}
