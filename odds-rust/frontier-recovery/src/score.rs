use crate::law::{Interval, Law};
use golaberto_odds::{model::Model, rng::Rng};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct ScoreCache {
    max_states: usize,
    states: usize,
    kernels: BTreeMap<Vec<(usize, u8, bool)>, BTreeMap<i32, Interval>>,
}
impl ScoreCache {
    pub fn new(max_states: usize) -> Self {
        Self {
            max_states,
            states: 0,
            kernels: BTreeMap::new(),
        }
    }
    pub fn states(&self) -> usize {
        self.states
    }
}
fn add_mass(dst: &mut BTreeMap<i32, Interval>, key: i32, mass: Interval) {
    let old = dst
        .get(&key)
        .copied()
        .unwrap_or_else(|| Interval::point(0.));
    dst.insert(key, old.add(mass));
}
fn convolve(
    a: &BTreeMap<i32, Interval>,
    b: &[(i32, Interval)],
    cap: usize,
) -> Option<BTreeMap<i32, Interval>> {
    let mut out = BTreeMap::new();
    for (&x, &px) in a {
        for &(y, py) in b {
            add_mass(&mut out, x + y, px.mul(py));
            if out.len() > cap {
                return None;
            }
        }
    }
    Some(out)
}
fn sample_conditional(model: &Model, index: usize, outcome: u8, rng: &mut Rng) -> [i32; 2] {
    let f = &model.fixtures[index];
    loop {
        let h = f.home_sampler.sample(rng);
        let a = f.away_sampler.sample(rng);
        let o = if h < a {
            0
        } else if h == a {
            1
        } else {
            2
        };
        if o == outcome {
            return [h, a];
        }
    }
}
fn gd_change(fixture: &golaberto_odds::model::Fixture, team: usize, score: [i32; 2]) -> i32 {
    if fixture.home == team {
        score[0] - score[1]
    } else {
        score[1] - score[0]
    }
}
fn kernel(law: &Law, outcome: u8, home_is_team: bool) -> Option<Vec<(i32, Interval)>> {
    let denom = law.outcomes[outcome as usize];
    if denom.lo <= 0.0 {
        return Some(Vec::new());
    }
    let mut out = BTreeMap::new();
    for p in &law.pairs[outcome as usize] {
        let gd = if home_is_team {
            p.home - p.away
        } else {
            p.away - p.home
        };
        add_mass(&mut out, gd, p.mass.div(denom));
    }
    Some(out.into_iter().collect())
}
fn exact_standings(
    model: &Model,
    outcomes: &[u8],
    want: &[bool],
    target: usize,
    rng: &mut Rng,
    fixed_scores: &[Option<[i32; 2]>],
) -> Result<Interval, String> {
    let mut campaigns = model.base.clone();
    let mut scores = model.empty_scores();
    for (i, &o) in outcomes.iter().enumerate() {
        let score = fixed_scores[i].unwrap_or_else(|| sample_conditional(model, i, o, rng));
        scores[model.fixtures[i].request_index] = score;
        model.add(&mut campaigns, i, score);
    }
    let mut order = vec![0; model.n];
    model.standings(&mut order, &campaigns, &scores, rng);
    let rank = order
        .iter()
        .position(|&team| team == target)
        .ok_or("target absent from standings")?;
    let event = order
        .iter()
        .enumerate()
        .all(|(i, &team)| want[team] == (i < rank));
    Ok(Interval::point(if event { 1. } else { 0. }))
}

/// Conditional probability bounds for one exact above-set event. Shared score
/// pairs are drawn once; private goal-difference kernels remain independent.
pub fn integrate(
    model: &Model,
    laws: &[Law],
    outcomes: &[u8],
    above: &[usize],
    target: usize,
    depth: usize,
    rng: &mut Rng,
    cache: &mut ScoreCache,
) -> Result<Interval, String> {
    let fixed_scores = vec![None; outcomes.len()];
    integrate_fixed(
        model,
        laws,
        outcomes,
        above,
        target,
        depth,
        rng,
        cache,
        &fixed_scores,
    )
}

pub fn integrate_fixed(
    model: &Model,
    laws: &[Law],
    outcomes: &[u8],
    above: &[usize],
    target: usize,
    depth: usize,
    rng: &mut Rng,
    cache: &mut ScoreCache,
    fixed_scores: &[Option<[i32; 2]>],
) -> Result<Interval, String> {
    if laws.len() != model.fixtures.len() || outcomes.len() != laws.len() {
        return Err("score integrator fixture length mismatch".into());
    }
    if fixed_scores.len() != laws.len() {
        return Err("fixed score fixture length mismatch".into());
    }
    if target >= model.n || above.iter().any(|&x| x >= model.n || x == target) {
        return Err("score integrator team index out of range".into());
    }
    let mut want = vec![false; model.n];
    for &x in above {
        if want[x] {
            return Err("duplicate above-set team".into());
        }
        want[x] = true;
    }
    let mut campaigns = model.base.clone();
    for (i, &o) in outcomes.iter().enumerate() {
        if o > 2 {
            return Err("invalid WDL outcome".into());
        }
        if laws[i].outcomes[o as usize].hi <= 0.0 {
            if fixed_scores[i].is_some() {
                return Err("fixed score has unsupported WDL outcome".into());
            }
            return Ok(Interval::point(0.));
        }
        if let Some(score) = fixed_scores[i] {
            if score[0] < 0 || score[1] < 0 {
                return Err("fixed score goals must be nonnegative".into());
            }
            let actual = if score[0] < score[1] {
                0
            } else if score[0] == score[1] {
                1
            } else {
                2
            };
            if actual != o {
                return Err("fixed score does not match WDL outcome".into());
            }
            if !laws[i].pairs[o as usize]
                .iter()
                .any(|pair| pair.home == score[0] && pair.away == score[1] && pair.mass.hi > 0.0)
            {
                return Err("fixed score is outside native score support".into());
            }
        }
        model.add(
            &mut campaigns,
            i,
            match o {
                0 => [0, 1],
                1 => [0, 0],
                _ => [1, 0],
            },
        );
    }
    if depth == 0 {
        return exact_standings(model, outcomes, &want, target, rng, fixed_scores);
    }
    let mut tied = vec![target];
    for team in 0..model.n {
        if team == target {
            continue;
        }
        let c = campaigns[team]
            .points
            .cmp(&campaigns[target].points)
            .then(campaigns[team].wins.cmp(&campaigns[target].wins));
        if c == std::cmp::Ordering::Equal {
            tied.push(team)
        } else {
            let should_above = c == std::cmp::Ordering::Greater;
            if want[team] != should_above {
                return Ok(Interval::point(0.));
            }
        }
    }
    let tied_mask: Vec<bool> = (0..model.n).map(|i| tied.contains(&i)).collect();
    let mut gd: Vec<BTreeMap<i32, Interval>> = (0..model.n).map(|_| BTreeMap::new()).collect();
    for &team in &tied {
        gd[team].insert(
            model.base[team].gf - model.base[team].ga,
            Interval::point(1.),
        );
    }
    let mut used_depth = vec![0usize; model.n];
    let mut selected = vec![Vec::<usize>::new(); model.n];
    let mut shared = Vec::<usize>::new();
    let mut sampled = vec![Vec::<usize>::new(); model.n];
    // Fixed fixture goals have already been drawn by PairPlan. Apply each
    // incident fixture once per tied endpoint and omit it from further sampling.
    for (i, f) in model.fixtures.iter().enumerate() {
        let ht = f.home < model.n && tied_mask[f.home];
        let at = f.away < model.n && tied_mask[f.away];
        if !ht && !at {
            continue;
        }
        if let Some(score) = fixed_scores[i] {
            if ht {
                let shift = gd_change(f, f.home, score);
                for (g, mass) in std::mem::take(&mut gd[f.home]) {
                    gd[f.home].insert(g + shift, mass);
                }
            }
            if at {
                let shift = gd_change(f, f.away, score);
                for (g, mass) in std::mem::take(&mut gd[f.away]) {
                    gd[f.away].insert(g + shift, mass);
                }
            }
            continue;
        }
        if ht && at {
            shared.push(i);
        } else {
            let team = if ht { f.home } else { f.away };
            if used_depth[team] < depth {
                selected[team].push(i);
                used_depth[team] += 1;
            } else {
                sampled[team].push(i);
            }
        }
    }
    if !selected.iter().any(|v| !v.is_empty()) {
        return exact_standings(model, outcomes, &want, target, rng, fixed_scores);
    }
    for i in shared {
        let f = &model.fixtures[i];
        let s = sample_conditional(model, i, outcomes[i], rng);
        let h = f.home;
        let a = f.away;
        for (k, v) in std::mem::take(&mut gd[h]) {
            gd[h].insert(k + s[0] - s[1], v);
        }
        for (k, v) in std::mem::take(&mut gd[a]) {
            gd[a].insert(k + s[1] - s[0], v);
        }
    }
    for &team in &tied {
        for &i in &sampled[team] {
            let s = sample_conditional(model, i, outcomes[i], rng);
            let shift = gd_change(&model.fixtures[i], team, s);
            for (k, v) in std::mem::take(&mut gd[team]) {
                gd[team].insert(k + shift, v);
            }
        }
        let key: Vec<_> = selected[team]
            .iter()
            .map(|&i| (i, outcomes[i], model.fixtures[i].home == team))
            .collect();
        if !key.is_empty() {
            let rows = if let Some(v) = cache.kernels.get(&key) {
                Some(v.clone())
            } else {
                let mut combined = BTreeMap::from([(0, Interval::point(1.))]);
                for &(i, outcome, home) in &key {
                    let Some(marginal) = kernel(&laws[i], outcome, home) else {
                        return Err("invalid private GD kernel".into());
                    };
                    if marginal.is_empty() {
                        return Ok(Interval::point(0.));
                    }
                    combined = convolve(&combined, &marginal, usize::MAX)
                        .ok_or("private GD kernel convolution failed")?;
                }
                if cache.states.saturating_add(combined.len()) <= cache.max_states {
                    cache.states += combined.len();
                    cache.kernels.insert(key.clone(), combined.clone());
                }
                Some(combined)
            };
            if let Some(rows) = rows {
                if rows.is_empty() {
                    return Ok(Interval::point(0.));
                }
                let offset = *gd[team].keys().next().unwrap();
                gd[team].clear();
                for (g, p) in rows {
                    gd[team].insert(offset + g, p);
                }
            }
        }
    }
    let mut lower = Interval::point(0.);
    let mut upper = Interval::point(0.);
    for (&target_gd, &tmass) in &gd[target] {
        let mut pl = Interval::point(1.);
        let mut pu = Interval::point(1.);
        for &rival in tied.iter().filter(|&&x| x != target) {
            let want_above = want[rival];
            let mut strict = Interval::point(0.);
            let mut inclusive = Interval::point(0.);
            for (&g, &p) in &gd[rival] {
                let qualifies_strict = if want_above {
                    g > target_gd
                } else {
                    g < target_gd
                };
                let qualifies_inclusive = if want_above {
                    g >= target_gd
                } else {
                    g <= target_gd
                };
                if qualifies_strict {
                    strict = strict.add(p)
                }
                if qualifies_inclusive {
                    inclusive = inclusive.add(p)
                }
            }
            pl = pl.mul(strict.clamp_probability());
            pu = pu.mul(inclusive.clamp_probability());
        }
        lower = lower.add(tmass.mul(pl));
        upper = upper.add(tmass.mul(pu));
    }
    if upper.hi < lower.lo {
        return Err("score probability enclosure conflict".into());
    }
    Ok(Interval {
        lo: lower.lo.max(0.).min(1.),
        hi: upper.hi.max(lower.lo).min(1.),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::law::Law;
    use golaberto_odds::model::{Model, Request};
    fn tiny() -> Model {
        let req:Request=serde_json::from_str(r#"{"id":1,"phase":{"sort":"pt,w,gd,gf,name","championship":{"point_win":3,"point_draw":1,"point_loss":0}},"games":[{"id":1,"home_id":1,"away_id":2,"home_power":0.0,"away_power":0.0,"played":false}],"team_groups":[{"team_id":1},{"team_id":2}]}"#).unwrap();
        Model::new(req).unwrap()
    }
    fn private_fixture_model() -> Model {
        let req: Request = serde_json::from_str(r#"{"id":2,"phase":{"sort":"pt,w,gd,gf,name","championship":{"point_win":0,"point_draw":0,"point_loss":0}},"games":[{"id":1,"home_id":2,"away_id":3,"home_score":1,"away_score":0,"played":true},{"id":2,"home_id":1,"away_id":3,"home_power":1.0,"away_power":1.0,"played":false}],"team_groups":[{"team_id":1},{"team_id":2},{"team_id":3,"add_sub":10}]}"#).unwrap();
        Model::new(req).unwrap()
    }
    fn fixed_pair_model() -> Model {
        let req: Request = serde_json::from_str(
            r#"{"id":3,"phase":{"sort":"pt,w,gd,name","championship":{"point_win":0,"point_draw":0,"point_loss":0}},"games":[{"id":1,"home_id":2,"away_id":3,"home_score":1,"away_score":0,"played":true},{"id":2,"home_id":1,"away_id":2,"home_power":1.0,"away_power":1.0,"played":false},{"id":3,"home_id":1,"away_id":4,"home_power":1.0,"away_power":1.0,"played":false},{"id":4,"home_id":1,"away_id":3,"home_power":2.0,"away_power":2.0,"played":false}],"team_groups":[{"team_id":1},{"team_id":2},{"team_id":3,"add_sub":20},{"team_id":4,"add_sub":10}]}"#,
        ).unwrap();
        Model::new(req).unwrap()
    }
    #[test]
    fn shared_score_equality_is_bracketed_and_pw_refuted() {
        let m = tiny();
        let l = vec![Law::new(0., 0.).unwrap()];
        let mut c = ScoreCache::new(100);
        let p = integrate(&m, &l, &[1], &[1], 0, 1, &mut Rng::new(2), &mut c).unwrap();
        assert_eq!(p.lo, p.hi);
        assert!(p.lo == 0. || p.lo == 1.);
        let p = integrate(&m, &l, &[0], &[], 0, 1, &mut Rng::new(2), &mut c).unwrap();
        assert_eq!(p, Interval::point(0.));
    }
    #[test]
    fn shared_tie_remains_bracketed() {
        let m = tiny();
        let l = vec![Law::new(1., 1.).unwrap()];
        let mut c = ScoreCache::new(10000);
        let p = integrate(&m, &l, &[1], &[1], 0, 1, &mut Rng::new(7), &mut c).unwrap();
        assert!(p.lo <= p.hi && p.hi <= 1.);
    }
    #[test]
    fn private_kernel_and_cap_fallback() {
        let m = private_fixture_model();
        let law = Law::new(1., 1.).unwrap();
        let laws = vec![law.clone()];
        let mut cache = ScoreCache::new(10000);
        let p = integrate(&m, &laws, &[2], &[2], 0, 1, &mut Rng::new(13), &mut cache).unwrap();
        assert!(p.lo <= p.hi && p.hi <= 1.);
        assert!(cache.states() > 0);
        let total = law.outcomes[2];
        let mut strict = Interval::point(0.);
        let mut inclusive = Interval::point(0.);
        for pair in &law.pairs[2] {
            let mass = pair.mass.div(total);
            if pair.home - pair.away > 1 {
                strict = strict.add(mass);
            }
            if pair.home - pair.away >= 1 {
                inclusive = inclusive.add(mass);
            }
        }
        assert!(p.lo <= strict.hi && p.hi >= inclusive.lo);
        let retained = cache.states();
        let again = integrate(&m, &laws, &[2], &[2], 0, 1, &mut Rng::new(29), &mut cache).unwrap();
        assert_eq!(again, p);
        assert_eq!(cache.states(), retained);
        let mut capped = ScoreCache::new(0);
        let q = integrate(&m, &laws, &[2], &[2], 0, 1, &mut Rng::new(13), &mut capped).unwrap();
        assert_eq!(q, p);
        assert_eq!(capped.states(), 0);

        let all_none = vec![None; laws.len()];
        let mut legacy_cache = ScoreCache::new(10000);
        let mut fixed_cache = ScoreCache::new(10000);
        let legacy = integrate(
            &m,
            &laws,
            &[2],
            &[2],
            0,
            1,
            &mut Rng::new(71),
            &mut legacy_cache,
        )
        .unwrap();
        let explicit_none = integrate_fixed(
            &m,
            &laws,
            &[2],
            &[2],
            0,
            1,
            &mut Rng::new(71),
            &mut fixed_cache,
            &all_none,
        )
        .unwrap();
        assert_eq!(legacy, explicit_none);
        assert_eq!(legacy_cache.states(), fixed_cache.states());
    }

    #[test]
    fn fixed_shared_and_private_scores_are_honored_in_integrated_and_exact_paths() {
        let model = fixed_pair_model();
        let laws = vec![
            Law::new(1., 1.).unwrap(),
            Law::new(1., 1.).unwrap(),
            Law::new(2., 2.).unwrap(),
        ];
        let outcomes = [2, 1, 0];
        let fixed = [Some([2, 0]), Some([1, 1]), None];
        let mut cache = ScoreCache::new(10000);
        let integrated = integrate_fixed(
            &model,
            &laws,
            &outcomes,
            &[1, 2, 3],
            0,
            1,
            &mut Rng::new(5),
            &mut cache,
            &fixed,
        )
        .unwrap();
        let last_law = &laws[2];
        let mut exact_mass = Interval::point(0.);
        let mut scores = [Some([2, 0]), Some([1, 1]), None];
        let mut fixed_event = None;
        for pair in &last_law.pairs[outcomes[2] as usize] {
            scores[2] = Some([pair.home, pair.away]);
            let mut campaigns = model.base.clone();
            let mut score_rows = model.empty_scores();
            for (i, score) in scores.iter().enumerate() {
                let score = score.unwrap();
                score_rows[model.fixtures[i].request_index] = score;
                model.add(&mut campaigns, i, score);
            }
            let mut order = vec![0; model.n];
            model.standings(&mut order, &campaigns, &score_rows, &mut Rng::new(91));
            let rank = order.iter().position(|&team| team == 0).unwrap();
            let event = order
                .iter()
                .enumerate()
                .all(|(position, &team)| ([1, 2, 3].contains(&team)) == (position < rank));
            fixed_event.get_or_insert((scores[2].unwrap(), event));
            if event {
                exact_mass = exact_mass.add(pair.mass.div(last_law.outcomes[0]));
            }
        }
        assert!(integrated.lo <= exact_mass.hi && integrated.hi >= exact_mass.lo);
        assert!(exact_mass.lo > 0. && exact_mass.hi < 1.);
        let (last_score, event) = fixed_event.unwrap();
        let all_fixed = [Some([2, 0]), Some([1, 1]), Some(last_score)];
        let exact_interval = integrate_fixed(
            &model,
            &laws,
            &outcomes,
            &[1, 2, 3],
            0,
            0,
            &mut Rng::new(5),
            &mut ScoreCache::new(0),
            &all_fixed,
        )
        .unwrap();
        assert_eq!(exact_interval, Interval::point(if event { 1. } else { 0. }));
        assert!(integrate_fixed(
            &model,
            &laws,
            &outcomes,
            &[1, 2, 3],
            0,
            1,
            &mut Rng::new(5),
            &mut ScoreCache::new(0),
            &[Some([2, 0]), Some([1, 0]), None]
        )
        .is_err());
        assert!(integrate_fixed(
            &model,
            &laws,
            &outcomes,
            &[1, 2, 3],
            0,
            1,
            &mut Rng::new(5),
            &mut ScoreCache::new(0),
            &[Some([-1, 3]), Some([1, 1]), None]
        )
        .is_err());
    }
}
