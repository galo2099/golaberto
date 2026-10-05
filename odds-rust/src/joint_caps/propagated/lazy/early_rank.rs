use super::*;

#[derive(Clone)]
struct Candidate {
    key: Vec<u8>,
    target_gain: i32,
    target_total: i32,
    point_totals: Vec<i32>,
    win_totals: Vec<i32>,
    packed_totals: Vec<i32>,
    prior: f64,
    domain_prior: f64,
    hint: f64,
    score: f64,
    propagation_refuted: bool,
}

pub(super) fn with_early_rank(
    m: &Model,
    cell: Cell,
    seed: i64,
    roots: usize,
    bias: bool,
    work_limit: usize,
) -> (std::result::Result<LazyJoint, String>, usize) {
    let mut work = 0usize;
    if cell.team >= m.n || cell.rank >= m.n {
        return (Err("early ranking cell is outside the model".into()), work);
    }
    if !(1..=32).contains(&roots) {
        return (Err("early ranking requires 1 to 32 roots".into()), work);
    }
    if work_limit == 0 {
        return (Err("early ranking work limit is zero".into()), work);
    }
    let bootstrap_estimate = bootstrap_work_estimate(
        m,
        cell,
        crate::rare_tail::value("RUST_ODDS_LAZY_GOALS") == "1",
    );
    if bootstrap_estimate > work_limit {
        let preflight_cost = m.fixtures.len().saturating_add(m.n).max(1);
        return (
            Err("early ranking bootstrap exceeds work limit".into()),
            preflight_cost.min(work_limit),
        );
    }
    work = bootstrap_estimate;
    let mut plan = match LazyJoint::with_mode(m, cell, seed, &[], 0, false, false, None) {
        Some(plan) => plan,
        None => {
            return (
                Err("lazy proposal setup is unsupported for this model".into()),
                work,
            )
        }
    };
    let target = plan.target.games.clone();
    let mut candidates = Vec::new();
    let mut key = Vec::with_capacity(target.len());
    let mut points = plan.base.clone();
    let mut visits = 0usize;
    let mut capped = false;
    fn enumerate(
        i: usize,
        games: &[RankGame],
        cell: Cell,
        suffix_rows: &[Vec<f64>],
        packed_gain: usize,
        points: &mut [i32],
        key: &mut Vec<u8>,
        prior: f64,
        out: &mut Vec<Candidate>,
        visits: &mut usize,
        capped: &mut bool,
        work: &mut usize,
        work_limit: usize,
    ) {
        if *work >= work_limit {
            *capped = true;
            return;
        }
        *work += 1;
        *visits += 1;
        if i == games.len() {
            if suffix_rows[i].get(packed_gain).copied().unwrap_or(0.) > 0. {
                if out.len() == 512 {
                    *capped = true;
                    return;
                }
                if prior.is_finite() && prior > 0. {
                    let allocation_cost = key.len().saturating_add(points.len().saturating_mul(3));
                    if work.saturating_add(allocation_cost) > work_limit {
                        *capped = true;
                        return;
                    }
                    *work += allocation_cost;
                    out.push(Candidate {
                        key: key.clone(),
                        target_gain: packed_gain as i32,
                        target_total: points[cell.team],
                        packed_totals: points.to_vec(),
                        point_totals: Vec::new(),
                        win_totals: Vec::new(),
                        prior,
                        domain_prior: 1.,
                        hint: 0.,
                        score: 0.,
                        propagation_refuted: false,
                    });
                }
            }
            return;
        }
        let g = &games[i];
        for o in 0..3 {
            if g.prob[o] <= 0. {
                continue;
            }
            let own_gain = if g.home == cell.team {
                g.hg[o]
            } else {
                g.ag[o]
            };
            let next_gain = packed_gain.saturating_add(own_gain.max(0) as usize);
            if suffix_rows[i + 1].get(next_gain).copied().unwrap_or(0.) <= 0. {
                continue;
            }
            key.push(o as u8);
            points[g.home] += g.hg[o];
            points[g.away] += g.ag[o];
            enumerate(
                i + 1,
                games,
                cell,
                suffix_rows,
                next_gain,
                points,
                key,
                prior * g.prob[o],
                out,
                visits,
                capped,
                work,
                work_limit,
            );
            points[g.home] -= g.hg[o];
            points[g.away] -= g.ag[o];
            key.pop();
            if *capped {
                return;
            }
        }
    }
    enumerate(
        0,
        &target,
        cell,
        &plan.target.rows,
        0,
        &mut points,
        &mut key,
        1.,
        &mut candidates,
        &mut visits,
        &mut capped,
        &mut work,
        work_limit,
    );
    if capped {
        return (
            Err("early ranking enumeration/work cap exhausted".into()),
            work,
        );
    }
    if candidates.is_empty() {
        return (
            Err("early ranking found no admitted target paths".into()),
            work,
        );
    }
    for candidate in &mut candidates {
        let mut points: Vec<_> = m.base.iter().map(|campaign| campaign.points).collect();
        let mut wins: Vec<_> = m.base.iter().map(|campaign| campaign.wins).collect();
        for (game, &outcome) in target.iter().zip(&candidate.key) {
            let fixture = &m.fixtures[game.index];
            match outcome {
                0 => wins[fixture.away] += 1,
                2 => wins[fixture.home] += 1,
                _ => {}
            }
            let (home, away) = match outcome {
                0 => (0, 3),
                1 => (1, 1),
                _ => (3, 0),
            };
            points[fixture.home] += home;
            points[fixture.away] += away;
        }
        candidate.point_totals = points;
        candidate.win_totals = wins;
    }
    let mut pmf_cache: HashMap<(usize, Vec<(usize, u8)>), Vec<f64>> = HashMap::new();
    let mut hits = 0usize;
    let mut misses = 0usize;
    let mut dp_transitions = 0usize;
    let mut propagations = 0usize;
    let mut refuted_paths = 0usize;
    let rivals: Vec<_> = (0..m.n).filter(|&t| t != cell.team).collect();
    for candidate in &mut candidates {
        if work >= work_limit {
            return (Err("early ranking work cap exhausted".into()), work);
        }
        let propagation_cost = plan
            .remaining
            .len()
            .saturating_mul(m.n)
            .saturating_mul(16)
            .saturating_add(candidate.key.len())
            .saturating_add(m.n.saturating_mul(3));
        if work.saturating_add(propagation_cost) > work_limit {
            return (
                Err("early ranking domain propagation exceeds work limit".into()),
                work,
            );
        }
        work += propagation_cost;
        let mut packed = plan.base.clone();
        for (g, &o) in target.iter().zip(&candidate.key) {
            packed[g.home] += g.hg[o as usize];
            packed[g.away] += g.ag[o as usize];
        }
        let d = Domains::propagate(
            &plan.remaining,
            &packed,
            &rivals,
            cell.rank,
            packed[cell.team],
        );
        propagations += 1;
        if !d.feasible {
            candidate.score = 0.;
            candidate.domain_prior = 0.;
            candidate.propagation_refuted = true;
            refuted_paths += 1;
            continue;
        }
        let mut domain_prior = 1.;
        if work.saturating_add(plan.remaining.len()) > work_limit {
            return (
                Err("early ranking domain prior exceeds work limit".into()),
                work,
            );
        }
        for (g, &mask) in plan.remaining.iter().zip(&d.domains) {
            let p: f64 = (0..3)
                .filter(|&o| mask & (1 << o) != 0)
                .map(|o| g.prob[o])
                .sum();
            domain_prior *= p;
            work += 1;
        }
        candidate.domain_prior = domain_prior;
        if !domain_prior.is_finite() || domain_prior <= 0. {
            candidate.score = 0.;
            continue;
        }
        let mut dp = vec![vec![0.0f64; m.n + 1]; m.n + 1];
        dp[0][0] = 1.;
        let mut processed = 0usize;
        for &team in &rivals {
            if work.saturating_add(plan.remaining.len()) > work_limit {
                return (
                    Err("early ranking PMF key scan exceeds work limit".into()),
                    work,
                );
            }
            work += plan.remaining.len();
            let cache_key = (
                team,
                plan.remaining
                    .iter()
                    .zip(&d.domains)
                    .filter(|(g, _)| g.home == team || g.away == team)
                    .map(|(g, &mask)| (g.index, mask))
                    .collect::<Vec<_>>(),
            );
            let pmf = if let Some(pmf) = pmf_cache.get(&cache_key) {
                let clone_cost = pmf.len();
                if work.saturating_add(clone_cost) > work_limit {
                    return (
                        Err("early ranking cached PMF copy exceeds work limit".into()),
                        work,
                    );
                }
                hits += 1;
                work += clone_cost;
                pmf.clone()
            } else {
                misses += 1;
                let mut span = 1usize;
                let mut pmf_cost = 0usize;
                for (g, _) in plan.remaining.iter().zip(&d.domains) {
                    if g.home == team || g.away == team {
                        let gain = if g.home == team { g.hg } else { g.ag };
                        span =
                            span.saturating_add(
                                gain.iter().copied().max().unwrap_or(0).max(0) as usize
                            );
                        pmf_cost = pmf_cost.saturating_add(span.saturating_mul(3));
                    }
                }
                if work.saturating_add(pmf_cost) > work_limit {
                    return (
                        Err("early ranking PMF construction exceeds work limit".into()),
                        work,
                    );
                }
                work += pmf_cost;
                let mut values = vec![1.];
                for (g, &mask) in plan.remaining.iter().zip(&d.domains) {
                    if g.home != team && g.away != team {
                        continue;
                    }
                    let allowed_mass: f64 = (0..3)
                        .filter(|&o| mask & (1 << o) != 0)
                        .map(|o| g.prob[o])
                        .sum();
                    if allowed_mass <= 0. {
                        continue;
                    }
                    let gain = if g.home == team { g.hg } else { g.ag };
                    let mut next =
                        vec![0.; values.len() + gain.iter().copied().max().unwrap_or(0) as usize];
                    for (a, &v) in values.iter().enumerate() {
                        for o in 0..3 {
                            if mask & (1 << o) != 0 {
                                next[a + gain[o] as usize] += v * g.prob[o] / allowed_mass;
                            }
                        }
                    }
                    values = next;
                }
                if pmf_cache.len() < 4096 {
                    pmf_cache.insert(cache_key, values.clone());
                }
                values
            };
            if work.saturating_add(pmf.len()) > work_limit {
                return (
                    Err("early ranking PMF score scan exceeds work limit".into()),
                    work,
                );
            }
            work += pmf.len();
            let (mut lo, mut eq, mut hi) = (0., 0., 0.);
            for (gain, &p) in pmf.iter().enumerate() {
                let score = packed[team] + gain as i32;
                if score < d.lower[team] || score > d.upper[team] {
                    continue;
                }
                if score < packed[cell.team] {
                    lo += p;
                } else if score == packed[cell.team] {
                    eq += p;
                } else {
                    hi += p;
                }
            }
            let allocation_cost = (m.n + 1).saturating_mul(m.n + 1);
            if work.saturating_add(allocation_cost) > work_limit {
                return (
                    Err("early ranking tie-DP allocation exceeds work limit".into()),
                    work,
                );
            }
            work += allocation_cost;
            let transition_cost = (0..=processed)
                .map(|ahead| {
                    (0..=processed - ahead)
                        .filter(|&ties| dp[ahead][ties] != 0.)
                        .count()
                        .saturating_mul(3)
                })
                .sum::<usize>();
            if work.saturating_add(transition_cost) > work_limit {
                return (
                    Err("early ranking tie-DP transitions exceed work limit".into()),
                    work,
                );
            }
            let mut next = vec![vec![0.0f64; m.n + 1]; m.n + 1];
            for ahead in 0..=processed {
                for ties in 0..=processed - ahead {
                    let mass = dp[ahead][ties];
                    if mass == 0. {
                        continue;
                    }
                    next[ahead][ties] += mass * lo;
                    next[ahead][ties + 1] += mass * eq;
                    next[ahead + 1][ties] += mass * hi;
                    dp_transitions += 3;
                }
            }
            dp = next;
            work += transition_cost;
            processed += 1;
        }
        let hint: f64 = (0..=cell.rank.min(m.n))
            .map(|ahead| {
                (cell.rank - ahead..=m.n - 1 - ahead)
                    .map(|ties| dp[ahead][ties])
                    .sum::<f64>()
            })
            .sum();
        candidate.hint = hint;
        candidate.score = candidate.prior * domain_prior * hint;
    }
    let max_hint = candidates
        .iter()
        .map(|c| c.hint)
        .filter(|h| h.is_finite())
        .fold(0.0, f64::max);
    let mut by_target_gain: std::collections::BTreeMap<(i32, i32), serde_json::Value> =
        std::collections::BTreeMap::new();
    for candidate in &candidates {
        let entry = by_target_gain.entry((candidate.target_gain, candidate.target_total)).or_insert_with(|| json!({"candidates":0,"sum_prior":0.0,"sum_domain_prior":0.0,"sum_score":0.0,"refuted":0}));
        let count = entry["candidates"].as_u64().unwrap_or(0) + 1;
        let sum_prior = entry["sum_prior"].as_f64().unwrap_or(0.) + candidate.prior;
        let sum_domain_prior =
            entry["sum_domain_prior"].as_f64().unwrap_or(0.) + candidate.domain_prior;
        let sum_score = entry["sum_score"].as_f64().unwrap_or(0.) + candidate.score;
        let refuted = entry["refuted"].as_u64().unwrap_or(0)
            + if candidate.propagation_refuted { 1 } else { 0 };
        entry["candidates"] = json!(count);
        entry["sum_prior"] = json!(sum_prior);
        entry["sum_domain_prior"] = json!(sum_domain_prior);
        entry["sum_score"] = json!(sum_score);
        entry["refuted"] = json!(refuted);
    }
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.key.cmp(&b.key)));
    let admitted_keys = candidates.len();
    let zero_rank_hints = candidates
        .iter()
        .filter(|candidate| candidate.hint <= 0.)
        .count();
    let selected: Vec<_> = candidates.into_iter().take(roots).collect();
    let mut built = 0usize;
    let mut selected_paths = Vec::new();
    let mut builder_exhausted = false;
    for candidate in selected {
        if work >= work_limit {
            selected_paths.push(json!({"key":candidate.key,"construction":"work_limit"}));
            builder_exhausted = true;
            break;
        }
        let node_start = plan.nodes;
        let guide_start = plan.guide_values;
        let old_limit = plan.node_limit;
        let old_guide_limit = plan.guide_limit;
        let scaffold_cost = plan
            .remaining
            .len()
            .saturating_mul(m.n)
            .saturating_mul(16)
            .saturating_add(candidate.key.len())
            .saturating_add(m.n.saturating_mul(10));
        if work.saturating_add(scaffold_cost) > work_limit {
            selected_paths.push(json!({"key":candidate.key,"construction":"work_limit"}));
            builder_exhausted = true;
            break;
        }
        work += scaffold_cost;
        let remaining = work_limit.saturating_sub(work);
        let node_budget = (remaining / 32).min(old_limit.saturating_sub(node_start));
        let guide_budget = (remaining / 4).min(old_guide_limit.saturating_sub(guide_start));
        plan.node_limit = node_start.saturating_add(node_budget);
        plan.guide_limit = guide_start.saturating_add(guide_budget);
        let result = plan.build(&candidate.key, m.n, None, None);
        plan.node_limit = old_limit;
        plan.guide_limit = old_guide_limit;
        work = work.saturating_add(
            (plan.nodes - node_start)
                .saturating_mul(16)
                .saturating_add((plan.guide_values - guide_start).saturating_mul(2)),
        );
        match result {
            Ok(Some(mut pattern)) if pattern.mass.is_finite() && pattern.mass > 0. => {
                pattern.tilt = if bias && max_hint > 0. {
                    (candidate.hint / max_hint).max(1e-80)
                } else {
                    1.
                };
                let proposal_score =
                    pattern.mass * pattern.joint.residual_hint.max(1e-80) * pattern.tilt;
                if proposal_score.is_finite() && proposal_score > 0. {
                    selected_paths.push(json!({"key":candidate.key.clone(),"target_gain":candidate.target_gain,"target_packed_total":candidate.target_total,"packed_totals":candidate.packed_totals,"point_totals":candidate.point_totals,"win_totals":candidate.win_totals,"exact_prior":candidate.prior,"global_domain_mass":candidate.domain_prior,"rank_hint":candidate.hint,"combined_score":candidate.score,"construction":"built"}));
                    plan.mixture_mass += proposal_score;
                    plan.mixture
                        .push((candidate.key.clone(), plan.mixture_mass));
                    plan.roots.insert(candidate.key, Some(pattern));
                    built += 1;
                }
            }
            Ok(None) => {
                selected_paths.push(json!({"key":candidate.key.clone(),"target_gain":candidate.target_gain,"target_packed_total":candidate.target_total,"packed_totals":candidate.packed_totals,"point_totals":candidate.point_totals,"win_totals":candidate.win_totals,"exact_prior":candidate.prior,"global_domain_mass":candidate.domain_prior,"rank_hint":candidate.hint,"combined_score":candidate.score,"construction":"no_pattern"}));
                plan.roots.insert(candidate.key, None);
            }
            Err(()) => {
                selected_paths.push(json!({"key":candidate.key,"target_gain":candidate.target_gain,"target_packed_total":candidate.target_total,"packed_totals":candidate.packed_totals,"point_totals":candidate.point_totals,"win_totals":candidate.win_totals,"exact_prior":candidate.prior,"global_domain_mass":candidate.domain_prior,"rank_hint":candidate.hint,"combined_score":candidate.score,"construction":"setup_exhausted"}));
                builder_exhausted = true;
                break;
            }
            _ => {}
        }
    }
    if built == 0 {
        return (
            Err("early ranking produced no usable patterns".into()),
            work,
        );
    }
    selected_paths.truncate(32);
    let by_target_gain: Vec<_> = by_target_gain.into_iter().map(|((gain, total), aggregate)| {
        json!({"target_gain":gain,"target_packed_total":total,"aggregate":aggregate})
    }).collect();
    plan.early_rank_diagnostics = Some(json!({
        "admitted_keys": admitted_keys,
        "enumeration_visits": visits,
        "domain_propagations": propagations,
        "pmf_cache_hits": hits,
        "pmf_cache_misses": misses,
        "dp_transitions": dp_transitions,
        "zero_rank_hints": zero_rank_hints,
        "refuted_paths": refuted_paths,
        "consumed_work": work,
        "declined": if builder_exhausted { json!("builder_budget_exhausted") } else { serde_json::Value::Null },
        "selected_paths": selected_paths,
        "by_target_gain": by_target_gain
    }));
    (Ok(plan), work)
}

fn bootstrap_work_estimate(m: &Model, cell: Cell, goal_tilt_requested: bool) -> usize {
    let teams = m.n;
    let fixtures = m.fixtures.len();
    let mut degree = vec![0usize; teams];
    for game in &m.fixtures {
        degree[game.home] = degree[game.home].saturating_add(1);
        degree[game.away] = degree[game.away].saturating_add(1);
    }
    let has_wins = m.keys.get(1) == Some(&Key::W);
    let stride = if has_wins {
        m.base
            .iter()
            .zip(&degree)
            .map(|(base, &left)| base.wins.saturating_add(left as i32))
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    } else {
        1
    };
    let max_gain = (3 * stride + i32::from(has_wins)).max(1) as usize;
    let target_degree = degree.get(cell.team).copied().unwrap_or(0);
    let target_span = target_degree.saturating_mul(max_gain).saturating_add(1);
    let mut work = fixtures
        .saturating_mul(teams.saturating_add(4))
        .saturating_add(teams.saturating_mul(teams.saturating_add(1)));
    work = work.saturating_add(
        target_degree
            .saturating_add(1)
            .saturating_mul(target_span)
            .saturating_add(target_degree.saturating_mul(target_span).saturating_mul(3)),
    );
    if goal_tilt_requested {
        let gd_index = if has_wins { 2 } else { 1 };
        let supported = m.keys.get(gd_index) == Some(&Key::Gd)
            && !m
                .fixtures
                .iter()
                .any(|fixture| fixture.home_sampler.mean > 32. || fixture.away_sampler.mean > 32.);
        if supported {
            for fixture in m
                .fixtures
                .iter()
                .filter(|fixture| fixture.home == cell.team || fixture.away == cell.team)
            {
                let support = |mean: f64| {
                    if mean <= 0. {
                        1
                    } else {
                        ((mean.ceil() as usize).saturating_mul(4).saturating_add(64)).min(1000)
                    }
                };
                work = work.saturating_add(
                    9usize
                        .saturating_mul(support(fixture.home_sampler.mean))
                        .saturating_mul(support(fixture.away_sampler.mean))
                        .saturating_mul(6),
                );
            }
        } else {
            work = work.saturating_add(fixtures.saturating_add(teams).max(1));
        }
    }
    work
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{conditioned::canonical_ranks, model::Request};
    use serde_json::json;

    fn tiny_model(sort: &str) -> Model {
        let request: Request = serde_json::from_value(json!({
            "id": 901,
            "phase": {"sort":sort,"championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":[
                {"team_id":0,"add_sub":1,"bias":2},
                {"team_id":1,"add_sub":2,"bias":0},
                {"team_id":2,"add_sub":0,"bias":1}],
            "games":[
                {"id":1,"home_id":0,"away_id":1,"home_power":1.4,"away_power":0.7},
                {"id":2,"home_id":0,"away_id":2,"home_power":0.8,"away_power":1.2},
                {"id":3,"home_id":1,"away_id":2,"home_power":1.1,"away_power":0.9}]
        }))
        .unwrap();
        Model::new(request).unwrap()
    }

    fn exact_rank_mass(m: &Model, cell: Cell) -> f64 {
        fn visit(m: &Model, cell: Cell, i: usize, outcomes: &mut [u8], mass: f64, total: &mut f64) {
            if i == m.fixtures.len() {
                if canonical_ranks(m, outcomes)[cell.team] == cell.rank {
                    *total += mass;
                }
                return;
            }
            for outcome in 0..3 {
                outcomes[i] = outcome;
                visit(
                    m,
                    cell,
                    i + 1,
                    outcomes,
                    mass * m.fixtures[i].prob[outcome as usize],
                    total,
                );
            }
        }
        let mut total = 0.;
        visit(m, cell, 0, &mut vec![0; m.fixtures.len()], 1., &mut total);
        total
    }

    #[test]
    fn scored_root_order_keeps_defensive_support_and_corrects_to_exact_rank_mass() {
        let m = tiny_model("pt,bias");
        let cell = Cell { team: 0, rank: 1 };
        let (built, _) = LazyJoint::with_early_rank(&m, cell, 77, 1, false, 2_800_000);
        let plan = built.unwrap();
        let diagnostics = plan.early_rank_diagnostics();
        let paths = diagnostics["selected_paths"].as_array().unwrap();
        assert_eq!(paths.len(), 1);
        assert!(plan.roots.len() < diagnostics["admitted_keys"].as_u64().unwrap() as usize);
        let root = plan.roots.keys().next().unwrap();
        let mut packed = plan.base.clone();
        for (game, &outcome) in plan.target.games.iter().zip(root) {
            packed[game.home] += game.hg[outcome as usize];
            packed[game.away] += game.ag[outcome as usize];
        }
        let rivals: Vec<_> = (0..m.n).filter(|&team| team != cell.team).collect();
        let domains = Domains::propagate(
            &plan.remaining,
            &packed,
            &rivals,
            cell.rank,
            packed[cell.team],
        );
        let expected_domain_mass: f64 = plan
            .remaining
            .iter()
            .zip(&domains.domains)
            .map(|(game, &mask)| {
                (0..3)
                    .filter(|&outcome| mask & (1 << outcome) != 0)
                    .map(|outcome| game.prob[outcome])
                    .sum::<f64>()
            })
            .product();
        assert_eq!(
            paths[0]["global_domain_mass"].as_f64().unwrap().to_bits(),
            expected_domain_mass.to_bits()
        );
        let exact = exact_rank_mass(&m, cell);
        let estimated = plan.sample(&m, 20_000, 78);
        assert!((estimated.probability - exact).abs() <= estimated.std_err * 5. + 1e-3);
        let (biased, _) = LazyJoint::with_early_rank(&m, cell, 77, 1, true, 2_800_000);
        let biased = biased.unwrap();
        assert_eq!(biased.roots.keys().next(), plan.roots.keys().next());
        let biased_estimate = biased.sample(&m, 20_000, 78);
        assert!((biased_estimate.probability - exact).abs() <= biased_estimate.std_err * 5. + 1e-3);
    }

    #[test]
    fn same_target_gain_can_have_different_opponent_rank_hints() {
        let m = tiny_model("pt,bias");
        let cell = Cell { team: 0, rank: 1 };
        let (built, _) = LazyJoint::with_early_rank(&m, cell, 79, 32, false, 2_800_000);
        let plan = built.unwrap();
        let paths = plan.early_rank_diagnostics()["selected_paths"]
            .as_array()
            .unwrap()
            .clone();
        let mut hints: HashMap<i64, Vec<u64>> = HashMap::new();
        for path in paths {
            let gain = path["target_gain"].as_i64().unwrap();
            hints
                .entry(gain)
                .or_default()
                .push(path["rank_hint"].as_f64().unwrap().to_bits());
        }
        assert!(hints
            .values()
            .any(|values| values.len() > 1 && values.windows(2).any(|pair| pair[0] != pair[1])));
    }

    #[test]
    fn wins_tiebreak_packed_totals_match_reported_points_and_wins() {
        let m = tiny_model("pt,w,bias");
        let cell = Cell { team: 0, rank: 1 };
        let (built, _) = LazyJoint::with_early_rank(&m, cell, 80, 1, true, 2_800_000);
        let plan = built.unwrap();
        let diag = plan.early_rank_diagnostics();
        let path = diag["selected_paths"].as_array().unwrap().first().unwrap();
        let points = path["point_totals"].as_array().unwrap();
        let wins = path["win_totals"].as_array().unwrap();
        let packed = path["packed_totals"].as_array().unwrap();
        let stride = (0..m.n)
            .map(|team| {
                m.base[team].wins
                    + m.fixtures
                        .iter()
                        .filter(|g| g.home == team || g.away == team)
                        .count() as i32
            })
            .max()
            .unwrap()
            + 1;
        for team in 0..m.n {
            assert_eq!(
                packed[team].as_i64().unwrap(),
                points[team].as_i64().unwrap() * i64::from(stride) + wins[team].as_i64().unwrap()
            );
        }
        let exact = exact_rank_mass(&m, cell);
        let estimated = plan.sample(&m, 10_000, 81);
        assert!((estimated.probability - exact).abs() <= estimated.std_err * 5. + 1e-3);
    }
}
