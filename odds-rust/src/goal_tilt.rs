//! Conditional score importance proposals on the existing model's score
//! support. Outcome probabilities are unchanged; all goal tilts use P/Q.
use crate::{
    conditioned::canonical,
    model::{Campaign, Key, Model},
    rng::Rng,
    sampling::masses,
    search::Cell,
};
const BETAS: [f64; 9] = [-3., -2., -1., -0.5, 0., 0.5, 1., 2., 3.];
struct Table {
    values: [Vec<([i32; 2], f64)>; 3],
    log_z: [f64; 3],
    mean: [f64; 3],
}
pub struct Context {
    campaign: Vec<Campaign>,
    scores: Vec<[i32; 2]>,
    order: Vec<usize>,
    ties: Vec<i32>,
}
impl Context {
    pub fn new(m: &Model) -> Self {
        Self {
            campaign: m.base.clone(),
            scores: m.empty_scores(),
            order: vec![0; m.n],
            ties: Vec::with_capacity(m.n),
        }
    }
}
pub struct GoalTilt {
    cell: Cell,
    tables: Vec<Vec<Table>>,
}
impl GoalTilt {
    pub fn new(m: &Model, cell: Cell) -> Option<Self> {
        let gd = if m.keys.get(1) == Some(&Key::W) { 2 } else { 1 };
        if m.keys.first() != Some(&Key::Pt) || m.keys.get(gd) != Some(&Key::Gd) {
            return None;
        }
        if m.fixtures
            .iter()
            .any(|f| f.home_sampler.mean > 32. || f.away_sampler.mean > 32.)
        {
            return None;
        }
        let tables = m
            .fixtures
            .iter()
            .filter(|f| f.home == cell.team || f.away == cell.team)
            .map(|f| {
                let h = masses(f.home_sampler.mean);
                let a = masses(f.away_sampler.mean);
                let sign = if f.home == cell.team { 1. } else { -1. };
                BETAS
                    .iter()
                    .map(|&beta| {
                        let mut values: [Vec<([i32; 2], f64)>; 3] =
                            std::array::from_fn(|_| Vec::new());
                        let (mut raw, mut z, mut mean) = ([0.; 3], [0.; 3], [0.; 3]);
                        for (hs, &hp) in h.iter().enumerate() {
                            for (as_, &ap) in a.iter().enumerate() {
                                let o = if hs < as_ {
                                    0
                                } else if hs == as_ {
                                    1
                                } else {
                                    2
                                };
                                let p = hp * ap;
                                let d = sign * (hs as f64 - as_ as f64);
                                let q = p * (beta * d).exp();
                                raw[o] += p;
                                z[o] += q;
                                mean[o] += q * d;
                                values[o].push(([hs as i32, as_ as i32], z[o]));
                            }
                        }
                        let log_z = std::array::from_fn(|o| {
                            if raw[o] > 0. {
                                (z[o] / raw[o]).ln()
                            } else {
                                0.
                            }
                        });
                        for o in 0..3 {
                            if z[o] > 0. {
                                mean[o] /= z[o];
                                for (_, c) in &mut values[o] {
                                    *c /= z[o];
                                }
                                values[o].last_mut().unwrap().1 = 1.;
                            }
                        }
                        Table {
                            values,
                            log_z,
                            mean,
                        }
                    })
                    .collect()
            })
            .collect();
        Some(Self { cell, tables })
    }
    pub fn rank(
        &self,
        m: &Model,
        out: &[u8],
        rng: &mut Rng,
        context: &mut Context,
    ) -> (usize, f64) {
        let c = &mut context.campaign;
        c.copy_from_slice(&m.base);
        let scores = &mut context.scores;
        let order = &mut context.order;
        for (i, _) in m.fixtures.iter().enumerate() {
            m.add(c, i, canonical(out[i]));
        }
        let wins = m.keys.get(1) == Some(&Key::W);
        let target = self.cell.team;
        let compare = |a: &Campaign, b: &Campaign| {
            a.points.cmp(&b.points).then_with(|| {
                if wins {
                    a.wins.cmp(&b.wins)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
        };
        let above = (0..m.n)
            .filter(|&t| t != target && compare(&c[t], &c[target]).is_gt())
            .count();
        let ties = &mut context.ties;
        ties.clear();
        ties.extend(
            (0..m.n)
                .filter(|&t| t != target && compare(&c[t], &c[target]).is_eq())
                .map(|t| c[t].gf - c[t].ga),
        );
        if ties.is_empty() {
            return (above, 1.);
        }
        if self.cell.rank < above || self.cell.rank > above + ties.len() {
            return (above, 1.);
        }
        ties.sort_by(|a, b| b.cmp(a));
        let k = self.cell.rank - above;
        let desired = if k == 0 {
            ties[0] as f64 + 1.
        } else if k == ties.len() {
            ties[k - 1] as f64 - 1.
        } else {
            (ties[k - 1] + ties[k]) as f64 * 0.5
        };
        let mut means = [(m.base[target].gf - m.base[target].ga) as f64; 9];
        let mut ix = 0;
        for (i, f) in m.fixtures.iter().enumerate() {
            if f.home == target || f.away == target {
                for b in 0..9 {
                    means[b] += self.tables[ix][b].mean[out[i] as usize];
                }
                ix += 1;
            }
        }
        let chosen = (0..9)
            .min_by(|&a, &b| {
                (means[a] - desired)
                    .abs()
                    .total_cmp(&(means[b] - desired).abs())
            })
            .unwrap();
        let drawn = if rng.float() < 0.1 { 4 } else { chosen };
        c.copy_from_slice(&m.base);
        ix = 0;
        let (mut log_z, mut gain) = (0., 0.);
        for (i, f) in m.fixtures.iter().enumerate() {
            let o = out[i] as usize;
            let score = if f.home == target || f.away == target {
                let table = &self.tables[ix][drawn];
                let v = &table.values[o];
                let u = rng.float();
                let score = v[v.partition_point(|(_, p)| *p < u).min(v.len() - 1)].0;
                log_z += self.tables[ix][chosen].log_z[o];
                gain += if f.home == target {
                    score[0] - score[1]
                } else {
                    score[1] - score[0]
                } as f64;
                ix += 1;
                score
            } else {
                f.scores.sample(o, rng)
            };
            scores[f.request_index] = score;
            m.add(c, i, score);
        }
        m.standings(order, c, scores, rng);
        let density = (BETAS[chosen] * gain - log_z).exp();
        (
            order.iter().position(|&t| t == target).unwrap(),
            1. / (0.1 + 0.9 * density),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Request;
    use serde_json::json;
    #[test]
    fn tilted_goal_mixture_matches_exact_conditional_rare_goal_difference_probability() {
        let request:Request=serde_json::from_value(json!({"id":1,"phase":{"sort":"pt,w,gd,bias","championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":(0..4).map(|t|json!({"team_id":t,"add_sub":0,"bias":if t==0{10}else{0}})).collect::<Vec<_>>(),
            "games":[{"id":1,"home_id":0,"away_id":1,"home_power":1.2,"away_power":0.8},{"id":2,"home_id":2,"away_id":3,"home_power":1.2,"away_power":0.8}]})).unwrap();
        let mut m = Model::new(request).unwrap();
        m.base[0].ga = 8;
        let mut d = vec![0.; 40];
        let mut z = 0.;
        for (h, &hp) in masses(1.2).iter().enumerate() {
            for (a, &ap) in masses(0.8).iter().enumerate() {
                if h > a {
                    d[h - a] += hp * ap;
                    z += hp * ap;
                }
            }
        }
        for p in &mut d {
            *p /= z;
        }
        let mut expected = 0.;
        for h in 1..d.len() {
            for a in 1..d.len() {
                if h as i32 - 8 >= a as i32 {
                    expected += d[h] * d[a];
                }
            }
        }
        assert!(expected > 0. && expected < 1e-5);
        let p = GoalTilt::new(&m, Cell { team: 0, rank: 0 }).unwrap();
        let mut rng = Rng::new(808);
        let mut context = Context::new(&m);
        let (mut sum, mut sum2) = (0., 0.);
        let n = 40000;
        for _ in 0..n {
            let (rank, w) = p.rank(&m, &[2, 2], &mut rng, &mut context);
            if rank == 0 {
                sum += w;
                sum2 += w * w;
            }
        }
        let mean = sum / n as f64;
        let se = ((sum2 / n as f64 - mean * mean) / (n - 1) as f64).sqrt();
        assert!(
            (mean - expected).abs() < 6. * se,
            "{mean} vs {expected}, se={se}"
        );
    }
}
