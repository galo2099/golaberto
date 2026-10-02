//! Factor-message proposal. Messages guide Q only. They neither prove
//! infeasibility nor replace the exact joint normalizer and final P/Q correction.
use super::*;

pub(super) struct TiltedPattern {
    pub pattern: Pattern,
    pub nominal: Arc<Vec<[f64; 3]>>,
}
impl TiltedPattern {
    pub fn correction(&self, out: &[u8], ops: &mut crate::rare_tail::budget::Operations) -> f64 {
        self.pattern
            .joint
            .games
            .iter()
            .chain(&self.pattern.guide.games)
            .fold(1., |v, g| {
                ops.guidance += 3;
                let o = out[g.index] as usize;
                v * self.nominal[g.index][o] / g.prob[o]
            })
    }
}

fn normalized(mut row: [f64; 3]) -> Option<[f64; 3]> {
    let z = row.iter().sum::<f64>();
    if z <= 0. || !z.is_finite() {
        return None;
    }
    for p in &mut row {
        *p /= z;
    }
    Some(row)
}
fn proposal_rows(p: &Pattern, iterations: usize, work: &mut usize) -> Option<Vec<RankGame>> {
    let mut games: Vec<_> = p
        .joint
        .games
        .iter()
        .chain(&p.guide.games)
        .copied()
        .collect();
    games.sort_by_key(|g| g.index);
    let mut incident = vec![Vec::new(); p.base.len()];
    for (i, g) in games.iter().enumerate() {
        incident[g.home].push((i, 0, g.hg));
        incident[g.away].push((i, 1, g.ag));
    }
    let mut messages = vec![[[1.; 3]; 2]; games.len()];
    for _ in 0..iterations {
        let mut next = messages.clone();
        for (t, edges) in incident.iter().enumerate() {
            for &(excluded, side, gains) in edges {
                let mut distribution = vec![1.];
                for &(i, s, gain) in edges {
                    if i == excluded {
                        continue;
                    }
                    let row = normalized(std::array::from_fn(|o| {
                        games[i].prob[o] * messages[i][1 - s][o]
                    }))?;
                    let mut added =
                        vec![0.; distribution.len() + *gain.iter().max().unwrap() as usize];
                    for (v, &mass) in distribution.iter().enumerate() {
                        if mass == 0. {
                            continue;
                        }
                        for o in 0..3 {
                            if row[o] > 0. {
                                *work += 1;
                                if *work > 20_000_000 {
                                    return None;
                                }
                                added[v + gain[o] as usize] += mass * row[o];
                            }
                        }
                    }
                    distribution = added;
                }
                let mut row = [0.; 3];
                for o in 0..3 {
                    let lo = (p.lower[t] - p.base[t] - gains[o]).max(0) as usize;
                    let hi = p.upper[t] - p.base[t] - gains[o];
                    if hi >= 0 && lo < distribution.len() {
                        let hi = (hi as usize).min(distribution.len() - 1);
                        if lo <= hi {
                            row[o] = distribution[lo..=hi].iter().sum();
                            *work += hi + 1 - lo;
                        }
                    }
                }
                let max = row.iter().copied().fold(0_f64, f64::max);
                if max > 0. && max.is_finite() {
                    for o in 0..3 {
                        next[excluded][side][o] =
                            0.5 * messages[excluded][side][o] + 0.5 * row[o] / max;
                    }
                }
                // A vanished floating point message is not an impossibility proof.
            }
        }
        messages = next;
    }
    let share = 0.9; // Defensive original probability preserves event support.
    for (i, g) in games.iter_mut().enumerate() {
        let tilted = normalized(std::array::from_fn(|o| {
            g.prob[o] * messages[i][0][o] * messages[i][1][o]
        }));
        if let Some(q) = tilted {
            g.prob = std::array::from_fn(|o| (1. - share) * g.prob[o] + share * q[o]);
        }
    }
    Some(games)
}
fn retune(
    p: &Pattern,
    fixture_count: usize,
    iterations: usize,
    work: &mut usize,
) -> Option<TiltedPattern> {
    if !p.omitted.is_empty() {
        return None;
    }
    let games = proposal_rows(p, iterations, work)?;
    let mut nominal = vec![[0.; 3]; fixture_count];
    for g in p.joint.games.iter().chain(&p.guide.games) {
        nominal[g.index] = g.prob;
    }
    let mut nodes = 10_000;
    let (joint, remaining) =
        NecessaryJoint::metered(&games, &p.base, &p.lower, &p.upper, 4, &mut nodes)?;
    *work += 16 * (10_000 - nodes);
    if joint.mass <= 0. || !joint.mass.is_finite() {
        return None;
    }
    let guide = Guide::new(remaining, p.base.len());
    *work += 2 * guide
        .cdf
        .values
        .iter()
        .chain(&guide.reverse.values)
        .map(|v| v.len())
        .sum::<usize>();
    let mut q = p.clone();
    q.mass = p.mass / p.joint.mass * joint.mass;
    if q.mass <= 0. || !q.mass.is_finite() {
        return None;
    }
    q.joint = Arc::new(joint);
    q.guide = Arc::new(guide);
    Some(TiltedPattern {
        pattern: q,
        nominal: Arc::new(nominal),
    })
}
impl BranchStrata {
    pub fn clear_messages(&mut self) {
        self.retuned.clear();
        self.message_work = 0;
    }
    pub fn prepare_messages(
        &mut self,
        m: &Model,
        pilots: &[Result],
        count: usize,
    ) -> serde_json::Value {
        self.clear_messages();
        let iterations = 3;
        let mut order: Vec<_> = (0..self.strata.len())
            .filter(|&i| pilots[i].probability > 0.)
            .collect();
        order.sort_by(|&a, &b| {
            pilots[b]
                .std_err
                .total_cmp(&pilots[a].std_err)
                .then(a.cmp(&b))
        });
        for &i in order.iter().take(count) {
            if let Some(q) = retune(
                &self.strata[i].pattern,
                m.fixtures.len(),
                iterations,
                &mut self.message_work,
            ) {
                self.retuned.insert(i, q);
            }
        }
        let mut selected: Vec<_> = self.retuned.keys().copied().collect();
        selected.sort_unstable();
        json!({"selected":selected,"iterations":iterations,"work":self.message_work,"patterns":selected.iter().map(|&i|json!({"index":i,"mass":self.retuned[&i].pattern.mass,"joint_teams":self.retuned[&i].pattern.joint.selected.iter().map(|&t|m.ids[t]).collect::<Vec<_>>(),"guided":self.retuned[&i].pattern.guide.games.len()})).collect::<Vec<_>>()})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{conditioned::canonical_ranks, model::Request};
    fn model() -> Model {
        let r:Request=serde_json::from_value(json!({"id":1,"phase":{"sort":"pt,w,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..4).map(|t|json!({"team_id":t,"add_sub":t,"bias":t})).collect::<Vec<_>>(),
            "games": ([(0,1),(0,2),(1,3),(2,3)]).iter().enumerate().map(|(i,&(h,a))|json!({"id":i,"home_id":h,"away_id":a,"home_power":1.1,"away_power":0.9})).collect::<Vec<_>>() })).unwrap();
        Model::new(r).unwrap()
    }
    #[test]
    fn tilted_joint_and_guide_exactly_restore_original_rank_mass() {
        let m = model();
        for rank in 0..m.n {
            let cell = Cell { team: 0, rank };
            let p = BranchStrata::with_tree(&m, cell, 808, 1, 32, 0, 4_000_000).unwrap();
            let mut expected = 0.;
            for mut code in 0..3usize.pow(m.fixtures.len() as u32) {
                let mut out = vec![0; m.fixtures.len()];
                let mut raw = 1.;
                for (i, g) in m.fixtures.iter().enumerate() {
                    let o = code % 3;
                    code /= 3;
                    out[i] = o as u8;
                    raw *= g.prob[o];
                }
                if canonical_ranks(&m, &out)[cell.team] == rank {
                    expected += raw;
                }
            }
            for bounds in [false, true] {
                let mut estimate = 0.;
                for s in &p.strata {
                    let mut work = 0;
                    let mut q = retune(&s.pattern, m.fixtures.len(), 6, &mut work).unwrap();
                    q.pattern.bound_guidance = bounds;
                    let uncorrected = q.pattern.clone();
                    for g in q.pattern.joint.games.iter().chain(&q.pattern.guide.games) {
                        for o in 0..3 {
                            assert!(g.prob[o] >= 0.099999 * q.nominal[g.index][o]);
                        }
                    }
                    for mut code in 0..3usize.pow(m.fixtures.len() as u32) {
                        let mut out = vec![0; m.fixtures.len()];
                        let mut raw = 1.;
                        for (i, g) in m.fixtures.iter().enumerate() {
                            let o = code % 3;
                            code /= 3;
                            out[i] = o as u8;
                            raw *= g.prob[o];
                        }
                        if canonical_ranks(&m, &out)[cell.team] != rank {
                            continue;
                        }
                        let mut points = vec![0; m.n];
                        let mut rng = Rng::new(123);
                        let w = q.pattern.draw(
                            cell,
                            &mut rng,
                            &mut out,
                            &mut points,
                            true,
                            true,
                            true,
                            0,
                            &mut Default::default(),
                        );
                        if w == 0. {
                            continue;
                        }
                        assert!(w.is_finite());
                        let w = w * q.correction(&out, &mut Default::default());
                        assert!(w.is_finite());
                        let uncorrected_weight = uncorrected.draw(
                            cell,
                            &mut rng,
                            &mut out,
                            &mut points,
                            true,
                            true,
                            true,
                            0,
                            &mut Default::default(),
                        );
                        let density = q
                            .pattern
                            .joint
                            .games
                            .iter()
                            .chain(&q.pattern.guide.games)
                            .fold(1., |v, g| v * g.prob[out[g.index] as usize])
                            / q.pattern.joint.mass
                            / uncorrected_weight;
                        estimate += density * w * q.pattern.mass;
                        let _ = raw;
                    }
                }
                assert!(
                    (estimate - expected).abs() < 1e-11,
                    "rank {rank} bounds {bounds}: {estimate} vs {expected}"
                );
            }
        }
    }
    #[test]
    fn message_work_exhaustion_retains_original_pattern() {
        let m = model();
        let cell = Cell { team: 0, rank: 1 };
        let p = BranchStrata::with_tree(&m, cell, 808, 1, 32, 0, 4_000_000).unwrap();
        let original = p
            .strata
            .iter()
            .find(|s| !s.pattern.guide.games.is_empty())
            .unwrap();
        let mut work = 20_000_000;
        assert!(retune(&original.pattern, m.fixtures.len(), 6, &mut work).is_none());
        assert!(!original.pattern.guide.games.is_empty());
    }
}
