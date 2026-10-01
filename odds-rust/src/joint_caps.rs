//! Exact necessary caps when proved; otherwise a broad defensive joint proposal.
//! No arbitrary fixture or score truncation establishes impossibility.
use crate::{
    conditioned::{Result, ScoreContext},
    lookahead::{RankGame, Suffix},
    model::{Key, Model},
    pool::work_per_sample,
    rng::Rng,
    search::Cell,
};
use serde_json::json;

pub mod broad;
pub mod propagated;

pub enum JointProposal {
    Exact(JointCaps),
    Broad(broad::BroadJoint),
    Propagated(propagated::PropagatedJoint),
}
impl JointProposal {
    pub fn new(model: &Model, cell: Cell) -> Option<Self> {
        if let Some(plan) = [false, true].into_iter().find_map(|dual| {
            [6, 5]
                .into_iter()
                .find_map(|count| JointCaps::new(model, cell, dual, count))
        }) {
            return Some(Self::Exact(plan));
        }
        if !crate::search::enabled("RUST_ODDS_JOINT_CAP_BROAD") {
            return None;
        }
        broad::BroadJoint::new(model, cell).map(Self::Broad)
    }
    pub fn describe(&self, model: &Model) -> serde_json::Value {
        match self {
            Self::Exact(plan) => {
                let mut value = plan.describe(model);
                value["mode"] = json!("exact_extreme");
                value
            }
            Self::Broad(plan) => plan.describe(model),
            Self::Propagated(plan) => plan.describe(model),
        }
    }
    pub fn sample(&self, model: &Model, samples: usize, seed: i64, guided: bool) -> Result {
        match self {
            Self::Exact(plan) => plan.sample(model, samples, seed, guided),
            Self::Broad(plan) => plan.sample(model, samples, seed, guided),
            Self::Propagated(plan) => plan.sample(model, samples, seed, guided),
        }
    }
}

/// Independent confirmation for a coarse weighted estimate. Failure leaves the
/// cell to its existing estimators; it does not establish impossibility.
pub fn confirmed(result: &Result, check: &Result) -> bool {
    result.coarse()
        && check.hits >= 30
        && check.ess >= 5.
        && check.probability > 0.
        && check.std_err / check.probability <= 0.6
        && (0.1..=10.).contains(&(result.probability / check.probability))
}

struct Table {
    games: Vec<(usize, [i32; 3], [f64; 3])>,
    cdf: Vec<Vec<f64>>,
    cap: usize,
}
impl Table {
    fn new(games: Vec<(usize, [i32; 3], [f64; 3])>, cap: i32) -> Option<Self> {
        if !(0..=4096).contains(&cap) {
            return None;
        }
        let cap = cap as usize;
        let mut cdf = vec![vec![1.; cap + 1]; games.len() + 1];
        for i in (0..games.len()).rev() {
            for c in 0..=cap {
                cdf[i][c] = (0..3)
                    .filter(|&o| games[i].1[o] <= c as i32)
                    .map(|o| games[i].2[o] * cdf[i + 1][c - games[i].1[o] as usize])
                    .sum();
            }
        }
        Some(Self { games, cdf, cap })
    }
    fn mass(&self, c: i32) -> f64 {
        if c < 0 {
            0.
        } else {
            self.cdf[0][(c as usize).min(self.cap)]
        }
    }
    fn sample(&self, mut cap: i32, rng: &mut Rng, out: &mut [u8]) {
        for (i, (index, gain, p)) in self.games.iter().enumerate() {
            let weights = std::array::from_fn::<_, 3, _>(|o| {
                if gain[o] <= cap {
                    p[o] * self.cdf[i + 1][cap as usize - gain[o] as usize]
                } else {
                    0.
                }
            });
            let total: f64 = weights.iter().sum();
            assert!(total > 0.);
            let o = choose(weights, total, rng);
            out[*index] = o as u8;
            cap -= gain[o];
        }
    }
}
struct Pattern {
    code: Vec<u8>,
    left: Vec<i32>,
    cumulative: f64,
}
pub struct JointCaps {
    pub target: usize,
    pub rank: usize,
    pub dual: bool,
    pub stride: i32,
    pub cap: i32,
    pub target_mass: f64,
    pub joint_mass: f64,
    pub selected: Vec<usize>,
    pub individual_masses: Vec<f64>,
    pub states: usize,
    pub selected_games: usize,
    fixed: Vec<(usize, u8)>,
    base: Vec<i32>,
    constrained: Vec<bool>,
    internal: Vec<(RankGame, usize, usize)>,
    tables: Vec<Table>,
    patterns: Vec<Pattern>,
    selected_fixtures: Vec<RankGame>,
    remaining: Vec<RankGame>,
    suffix: Suffix,
}
fn choose(weights: [f64; 3], total: f64, rng: &mut Rng) -> usize {
    let u = rng.float() * total;
    if u < weights[0] {
        0
    } else if u < weights[0] + weights[1] {
        1
    } else {
        2
    }
}
impl JointCaps {
    /// Supported only if the best target pattern is necessary and the number of
    /// unavoidable teams ahead equals the requested transformed rank. This gate
    /// makes every remaining rival cap necessary; it is independent of identities.
    pub fn new(m: &Model, cell: Cell, dual: bool, count: usize) -> Option<Self> {
        let c = &m.request.phase.championship;
        if m.keys.first() != Some(&Key::Pt)
            || m.request.phase.bonus_points != 0
            || (c.point_win, c.point_draw, c.point_loss) != (3, 1, 0)
            || m.ids.len() != m.n
            || cell.team >= m.n
            || cell.rank >= m.n
            || m.fixtures.iter().any(|g| g.home == g.away)
        {
            return None;
        }
        // Keep packed arithmetic safely inside i32, including negated floors.
        let max_wins = m.base.iter().map(|v| i64::from(v.wins)).max()? + m.fixtures.len() as i64;
        let safe_stride = max_wins + 1;
        if m.fixtures.len() > 4096
            || m.base.iter().any(|v| {
                (i64::from(v.points).abs() + 3 * m.fixtures.len() as i64 + 1) * safe_stride
                    + max_wins
                    >= i64::from(i32::MAX / 4)
            })
        {
            return None;
        }
        let wins = m.keys.get(1) == Some(&Key::W);
        let mut left = vec![0; m.n];
        for g in &m.fixtures {
            left[g.home] += 1;
            left[g.away] += 1;
        }
        let stride = if wins {
            (0..m.n).map(|t| m.base[t].wins + left[t]).max()? + 1
        } else {
            1
        };
        let sign = if dual { -1 } else { 1 };
        let hg = [0, stride, 3 * stride + i32::from(wins)].map(|v| v * sign);
        let ag = [hg[2], hg[1], hg[0]];
        let target = cell.team;
        let rank = if dual { m.n - 1 - cell.rank } else { cell.rank };
        let mut base = m
            .base
            .iter()
            .map(|c| (c.points * stride + if wins { c.wins } else { 0 }) * sign)
            .collect::<Vec<_>>();
        let mut lower = base.clone();
        for g in &m.fixtures {
            lower[g.home] += *hg.iter().min()?;
            lower[g.away] += *ag.iter().min()?;
        }
        let mut cap = base[target];
        let mut fixed = Vec::new();
        let mut target_mass = 1.;
        let mut smallest_loss = i32::MAX;
        for (i, g) in m.fixtures.iter().enumerate() {
            if g.home != target && g.away != target {
                continue;
            }
            let gain = if g.home == target { hg } else { ag };
            let maximum = *gain.iter().max()?;
            let o = gain.iter().position(|v| *v == maximum)?;
            smallest_loss =
                smallest_loss.min(maximum - *gain.iter().filter(|&&v| v < maximum).max()?);
            cap += maximum;
            fixed.push((i, o as u8));
            target_mass *= g.prob[o];
            base[g.home] += hg[o];
            base[g.away] += ag[o];
        }
        if fixed.is_empty()
            || target_mass <= 0.
            || lower
                .iter()
                .enumerate()
                .filter(|(t, p)| *t != target && **p > cap - smallest_loss)
                .count()
                <= rank
        {
            return None;
        }
        let mut all = Vec::new();
        for (i, g) in m.fixtures.iter().enumerate() {
            if g.home == target || g.away == target {
                continue;
            }
            let hm = *hg.iter().min()?;
            let am = *ag.iter().min()?;
            base[g.home] += hm;
            base[g.away] += am;
            all.push(RankGame {
                index: i,
                home: g.home,
                away: g.away,
                prob: g.prob,
                hg: hg.map(|v| v - hm),
                ag: ag.map(|v| v - am),
            });
        }
        if base
            .iter()
            .enumerate()
            .filter(|(t, p)| *t != target && **p > cap)
            .count()
            != rank
        {
            return None;
        }
        let constrained = (0..m.n)
            .map(|t| t != target && base[t] <= cap)
            .collect::<Vec<_>>();
        let mut candidates = Vec::new();
        for t in 0..m.n {
            if !constrained[t] {
                continue;
            }
            let games = all
                .iter()
                .filter(|g| g.home == t || g.away == t)
                .map(|g| (g.index, if g.home == t { g.hg } else { g.ag }, g.prob))
                .collect();
            let table = Table::new(games, cap - base[t])?;
            let mass = table.mass(cap - base[t]);
            if mass <= 0. {
                return None;
            }
            candidates.push((t, mass));
        }
        candidates.sort_by(|a, b| a.1.total_cmp(&b.1).then(m.ids[a.0].cmp(&m.ids[b.0])));
        let selected = candidates
            .iter()
            .take(count.min(6))
            .map(|v| v.0)
            .collect::<Vec<_>>();
        let individual_masses = candidates
            .iter()
            .take(selected.len())
            .map(|v| v.1)
            .collect();
        let mut internal = Vec::new();
        let mut ext = vec![Vec::new(); selected.len()];
        let mut selected_fixtures = Vec::new();
        let mut remaining = Vec::new();
        for g in all {
            let h = selected.iter().position(|v| *v == g.home);
            let a = selected.iter().position(|v| *v == g.away);
            match (h, a) {
                (Some(h), Some(a)) => {
                    internal.push((g, h, a));
                    selected_fixtures.push(g)
                }
                (Some(h), None) => {
                    ext[h].push((g.index, g.hg, g.prob));
                    selected_fixtures.push(g)
                }
                (None, Some(a)) => {
                    ext[a].push((g.index, g.ag, g.prob));
                    selected_fixtures.push(g)
                }
                _ => remaining.push(g),
            }
        }
        let states = 3usize.checked_pow(internal.len() as u32)?;
        if states > 20000 {
            return None;
        }
        let caps = selected.iter().map(|&t| cap - base[t]).collect::<Vec<_>>();
        let tables = ext
            .into_iter()
            .zip(&caps)
            .map(|(gs, &c)| Table::new(gs, c))
            .collect::<Option<Vec<_>>>()?;
        let mut patterns = Vec::new();
        let mut joint_mass = 0.;
        for mut code in 0..states {
            let mut left = caps.clone();
            let mut outcomes = Vec::new();
            let mut weight = 1.;
            for (g, h, a) in &internal {
                let o = code % 3;
                code /= 3;
                left[*h] -= g.hg[o];
                left[*a] -= g.ag[o];
                weight *= g.prob[o];
                outcomes.push(o as u8);
            }
            for (table, &c) in tables.iter().zip(&left) {
                weight *= table.mass(c);
            }
            if weight > 0. {
                joint_mass += weight;
                patterns.push(Pattern {
                    code: outcomes,
                    left,
                    cumulative: joint_mass,
                });
            }
        }
        if joint_mass <= 0. {
            return None;
        }
        let max_cap = base.iter().map(|p| cap - p).max()?.max(0);
        if max_cap > 4096 {
            return None;
        }
        let suffix = Suffix::new(&remaining, m.n, max_cap);
        let selected_games = selected_fixtures.len();
        Some(Self {
            target,
            rank: cell.rank,
            dual,
            stride,
            cap,
            target_mass,
            joint_mass,
            selected,
            individual_masses,
            states,
            selected_games,
            fixed,
            base,
            constrained,
            internal,
            tables,
            patterns,
            selected_fixtures,
            remaining,
            suffix,
        })
    }
    pub fn describe(&self, m: &Model) -> serde_json::Value {
        json!({"team":m.ids[self.target],"rank":self.rank+1,"stride":self.stride,"dual":self.dual,"target_fixed":self.fixed.len(),"target_mass":self.target_mass,"rivals":self.selected.iter().map(|&t|m.ids[t]).collect::<Vec<_>>(),"individual_cap_masses":self.individual_masses,"internal_games":self.internal.len(),"enumerated_states":self.states,"joint_cap_mass":self.joint_mass,"selected_games":self.selected_games,"residual_games":self.remaining.len(),"conditioning_mass":self.target_mass*self.joint_mass})
    }
    pub fn sample(&self, m: &Model, samples: usize, seed: i64, guided: bool) -> Result {
        let mut r = Result {
            mass: self.target_mass * self.joint_mass,
            ..Default::default()
        };
        let mut rng = Rng::new(seed);
        let mut scores = ScoreContext::new(m);
        let mut out = vec![0; m.fixtures.len()];
        let mut points = self.base.clone();
        let (mut sum, mut sum2, mut max, mut batches) = (0., 0., 0_f64, [0.; 2]);
        for draw in 0..samples {
            for &(i, o) in &self.fixed {
                out[i] = o
            }
            let u = rng.float() * self.joint_mass;
            let p = &self.patterns[self
                .patterns
                .partition_point(|v| v.cumulative < u)
                .min(self.patterns.len() - 1)];
            for ((g, _, _), &o) in self.internal.iter().zip(&p.code) {
                out[g.index] = o
            }
            for (table, &cap) in self.tables.iter().zip(&p.left) {
                table.sample(cap, &mut rng, &mut out)
            }
            points.copy_from_slice(&self.base);
            for g in &self.selected_fixtures {
                let o = out[g.index] as usize;
                points[g.home] += g.hg[o];
                points[g.away] += g.ag[o];
            }
            let mut weight = 1.;
            for (step, g) in self.remaining.iter().enumerate() {
                let mut weights = [0.; 3];
                for o in 0..3 {
                    let h = if guided && self.constrained[g.home] {
                        self.suffix
                            .cdf(step + 1, g.home, self.cap - points[g.home] - g.hg[o])
                    } else {
                        1.
                    };
                    let a = if guided && self.constrained[g.away] {
                        self.suffix
                            .cdf(step + 1, g.away, self.cap - points[g.away] - g.ag[o])
                    } else {
                        1.
                    };
                    weights[o] = g.prob[o] * h * a;
                }
                let total: f64 = weights.iter().sum();
                if total <= 0. {
                    weight = 0.;
                    break;
                }
                let o = choose(weights, total, &mut rng);
                weight *= g.prob[o] * total / weights[o];
                out[g.index] = o as u8;
                points[g.home] += g.hg[o];
                points[g.away] += g.ag[o];
            }
            r.samples += 1;
            if weight <= 0.
                || self
                    .constrained
                    .iter()
                    .enumerate()
                    .any(|(t, &on)| on && points[t] > self.cap)
            {
                continue;
            }
            if scores.rank(self.target, &out, &mut rng, None) != self.rank {
                continue;
            }
            r.hits += 1;
            if r.witness.is_none() {
                r.witness = Some(out.clone())
            }
            sum += weight;
            sum2 += weight * weight;
            max = max.max(weight);
            batches[usize::from(draw >= samples / 2)] += weight;
        }
        r.work = r.samples as u64 * work_per_sample(m);
        r.summarize(sum, sum2, max, batches);
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Request;
    #[test]
    fn confirmation_rejects_missing_or_inconsistent_independent_evidence() {
        let result = Result {
            weighted: true,
            probability: 1e-24,
            std_err: 2e-25,
            hits: 90,
            ess: 25.,
            max_share: 0.08,
            batch_gap: 0.1,
            ..Default::default()
        };
        let mut check = Result {
            probability: 1.2e-24,
            std_err: 3e-25,
            hits: 36,
            ess: 15.,
            ..Default::default()
        };
        assert!(confirmed(&result, &check));
        assert!(!confirmed(&result, &Result::default()));
        check.probability = 1e-26;
        assert!(!confirmed(&result, &check));
        check.probability = 1.2e-24;
        check.hits = 29;
        assert!(!confirmed(&result, &check));
        check.hits = 36;
        check.std_err = f64::NAN;
        assert!(!confirmed(&result, &check));
        assert!(!confirmed(&Result::default(), &check));
    }
    fn league(points: [i32; 3]) -> Model {
        let r:Request=serde_json::from_value(json!({"id":1,"phase":{"sort":"pt,w,gd,gf","championship":{"point_win":3,"point_draw":1,"point_loss":0}},"team_groups":(0..3).map(|t|json!({"team_id":t,"add_sub":points[t]})).collect::<Vec<_>>(),"games":[{"id":1,"home_id":0,"away_id":1,"home_power":1.2,"away_power":0.9},{"id":2,"home_id":0,"away_id":2,"home_power":1.1,"away_power":1.3},{"id":3,"home_id":1,"away_id":2,"home_power":0.8,"away_power":1.4}]})).unwrap();
        Model::new(r).unwrap()
    }
    fn enumerate(m: &Model, p: &JointCaps) -> (f64, f64) {
        let mut event = 0.;
        let mut rank_mass = 0.;
        let sign = if p.dual { -1 } else { 1 };
        for mut code in 0..3usize.pow(m.fixtures.len() as u32) {
            let mut out = Vec::new();
            let mut pts = m
                .base
                .iter()
                .map(|c| (c.points * p.stride + c.wins) * sign)
                .collect::<Vec<_>>();
            let mut weight = 1.;
            for g in &m.fixtures {
                let o = code % 3;
                code /= 3;
                out.push(o as u8);
                pts[g.home] += [0, p.stride, 3 * p.stride + 1][o] * sign;
                pts[g.away] += [3 * p.stride + 1, p.stride, 0][o] * sign;
                weight *= g.prob[o];
            }
            if p.fixed.iter().all(|&(i, o)| out[i] == o)
                && p.selected.iter().all(|&t| pts[t] <= p.cap)
            {
                event += weight;
            }
            let original = pts.iter().map(|v| v * sign).collect::<Vec<_>>();
            let above = original
                .iter()
                .enumerate()
                .filter(|(t, v)| *t != p.target && **v > original[p.target])
                .count();
            let tied = original
                .iter()
                .enumerate()
                .filter(|(t, v)| *t != p.target && **v == original[p.target])
                .count();
            if above <= p.rank && p.rank <= above + tied {
                assert!(p.fixed.iter().all(|&(i, o)| out[i] == o));
                assert!(p.selected.iter().all(|&t| pts[t] <= p.cap));
            }
            if above == p.rank && tied == 0 {
                rank_mass += weight;
            }
        }
        (event, rank_mass)
    }
    #[test]
    fn exact_joint_mass_matches_all_shared_fixture_assignments() {
        for (points, rank, dual) in [([0, 5, 7], 1, false), ([10, 4, 7], 2, true)] {
            let m = league(points);
            for k in 0..=2 {
                let p = JointCaps::new(&m, Cell { team: 0, rank }, dual, k).unwrap();
                let (e, _) = enumerate(&m, &p);
                assert!((e - p.target_mass * p.joint_mass).abs() < 1e-13);
            }
        }
    }
    #[test]
    fn conditional_draws_and_importance_weights_match_exact_probability() {
        for (points, rank, dual) in [([0, 5, 7], 1, false), ([10, 4, 7], 2, true)] {
            let m = league(points);
            for k in 0..=2 {
                let p = JointCaps::new(&m, Cell { team: 0, rank }, dual, k).unwrap();
                let (_, exact) = enumerate(&m, &p);
                let r = p.sample(&m, 5000, 808, true);
                assert!(r.hits > 0);
                assert_eq!(r.work, r.samples as u64 * work_per_sample(&m));
                assert!(
                    (r.probability - exact).abs() < 6. * r.std_err + 1e-12,
                    "k={k} dual={dual} p={} exact={exact}",
                    r.probability
                );
            }
        }
    }
    #[test]
    fn four_rival_internal_graph_matches_exhaustive_joint_mass() {
        let teams = [45, 43, 48, 47, 42]
            .into_iter()
            .enumerate()
            .map(|(t, p)| json!({"team_id":t,"add_sub":p}))
            .collect::<Vec<_>>();
        let mut games = vec![
            json!({"id":1,"home_id":0,"away_id":1,"home_power":1.2,"away_power":0.9}),
            json!({"id":2,"home_id":0,"away_id":2,"home_power":1.1,"away_power":1.3}),
        ];
        for h in 1..5 {
            for a in h + 1..5 {
                games.push(json!({"id":games.len()+1,"home_id":h,"away_id":a,"home_power":1.0,"away_power":1.1}));
            }
        }
        for id in [20, 21] {
            games.push(json!({"id":id,"home_id":1,"away_id":4,"played":true,"home_score":1,"away_score":0}));
        }
        let request:Request=serde_json::from_value(json!({"id":1,"phase":{"sort":"pt,w,gd,gf","championship":{"point_win":3,"point_draw":1,"point_loss":0}},"team_groups":teams,"games":games})).unwrap();
        let m = Model::new(request).unwrap();
        for count in 0..=4 {
            let p = JointCaps::new(&m, Cell { team: 0, rank: 0 }, false, count).unwrap();
            let (event, _) = enumerate(&m, &p);
            assert!(
                (event - p.target_mass * p.joint_mass).abs() < 1e-13,
                "count={count}"
            );
        }
    }

    #[test]
    fn unsupported_cells_are_skipped_without_probability_or_proof() {
        let mut large = league([0, 5, 7]);
        large.base[0].points = i32::MAX / 8;
        assert!(JointCaps::new(&large, Cell { team: 0, rank: 1 }, false, 2).is_none());
        let m = league([0, 0, 0]);
        assert!(JointCaps::new(&m, Cell { team: 0, rank: 1 }, false, 2).is_none());
        assert!(JointCaps::new(&m, Cell { team: 0, rank: m.n }, true, 2).is_none());
        let mut request = m.request.clone();
        request.phase.sort = "gd,pt".into();
        let m = Model::new(request).unwrap();
        assert!(JointCaps::new(&m, Cell { team: 0, rank: 1 }, false, 2).is_none());
    }
}
