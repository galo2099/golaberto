//! Cost-bounded WDL outcome path stratification with conditional Poisson scores.
use crate::{
    head_buckets::{self, FixtureBuckets},
    model::{Key, Model},
    pool::Estimate,
    rng::{derive, Rng},
};
use std::{cmp::Ordering, collections::HashSet};

const ORDINARY_SEASONS: u64 = 100_000;
const MAX_STRATA: usize = 50_000;
const MAX_DRAWS: usize = 100_000;
const DRAWS_PER_STRATUM: usize = 128;

#[derive(Clone)]
struct Leaf {
    outcomes: Vec<usize>,
    bucket_ids: Vec<Option<usize>>,
    mass: f64,
    exact: bool,
}

fn eligible(m: &Model) -> bool {
    m.request.phase.bonus_points == 0
        && m.fixtures
            .iter()
            .all(|f| f.home != f.away && f.home_sampler.mean <= 32. && f.away_sampler.mean <= 32.)
}

fn head_optimization_safe(m: &Model) -> bool {
    m.keys.get(0) == Some(&Key::Pt)
        && m.keys.get(1) == Some(&Key::Head)
        && m.fixtures.iter().all(|f| f.home < m.n && f.away < m.n)
        && {
            let mut pairs = HashSet::new();
            m.fixtures
                .iter()
                .all(|f| pairs.insert((f.home.min(f.away), f.home.max(f.away))))
        }
}

fn plan(m: &Model, head: bool) -> Option<(usize, u64, u64, usize, usize)> {
    let paths = m.fixtures.iter().try_fold(1usize, |v, f| {
        v.checked_mul(f.prob.iter().filter(|p| **p > 0.).count())
    })?;
    if paths == 0 || paths > MAX_STRATA {
        return None;
    }
    // Normalize against this model's ordinary season cost, conservatively
    // charging campaign updates, all pair comparisons, key scans, and head recursion.
    let n = m.ids.len() as u64;
    let f = m.fixtures.len() as u64;
    let games = m.request.games.len() as u64;
    let comparator = m.keys.len() as u64 + 1;
    let mut ordinary = f
        .checked_mul(n)?
        .checked_add(n.checked_mul(n)?.checked_mul(comparator)?)?
        .checked_add(games)?
        .checked_add(n)?
        .checked_mul(comparator)?
        .max(1);
    if m.keys.contains(&Key::Head) {
        ordinary = ordinary.checked_add(
            n.checked_mul(n)?
                .checked_mul(comparator)?
                .checked_mul(games.checked_add(n)?.checked_add(comparator)?)?,
        )?;
    }
    let pair_cert = n
        .checked_mul(n)?
        .checked_mul(comparator)?
        .checked_add(f.checked_mul(n)?)?
        .max(1);
    let prep = if head {
        m.fixtures.iter().try_fold(0u64, |sum, fx| {
            let h = crate::sampling::masses(fx.home_sampler.mean).len() as u64;
            let a = crate::sampling::masses(fx.away_sampler.mean).len() as u64;
            sum.checked_add(
                h.checked_mul(a)?
                    .checked_mul(games.checked_add(n)?.checked_add(comparator)?)?,
            )
        })?
    } else {
        0
    };
    let path_work = (paths as u64).checked_mul(pair_cert)?.checked_add(prep)?;
    let budget = ORDINARY_SEASONS.checked_mul(ordinary)?;
    if path_work >= budget {
        return None;
    }
    let draw_budget = ((budget - path_work) / ordinary).min(MAX_DRAWS as u64);
    // Actual unresolved mass/leaf count is determined by the bounded path
    // certificate below; do not reject an all-exact plan on a sampling floor.
    Some((
        paths,
        path_work,
        budget,
        usize::try_from(ordinary).ok()?,
        draw_budget as usize,
    ))
}

fn leaf_work_unit(m: &Model, head: bool) -> Option<u64> {
    let n = m.ids.len() as u64;
    let f = m.fixtures.len() as u64;
    let mut cost = n
        .checked_mul(n)?
        .checked_mul(m.keys.len() as u64 + 1)?
        .checked_add(f.checked_mul(n)?)?
        // Each leaf constructs canonical campaign/scores more than once;
        // charge three full request-game score-vector copies conservatively.
        .checked_add((m.request.games.len() as u64).checked_mul(3)?)?;
    if head {
        cost = cost.checked_add(
            n.checked_mul(n)?.checked_mul(
                (m.request.games.len() as u64)
                    .checked_add(n)?
                    .checked_add(m.keys.len() as u64 + 1)?,
            )?,
        )?;
    }
    Some(cost.max(1))
}

fn path_scores(m: &Model, outcomes: &[usize]) -> (Vec<crate::model::Campaign>, Vec<[i32; 2]>) {
    let mut c = m.base.clone();
    let mut scores = m.empty_scores();
    for (i, (&o, f)) in outcomes.iter().zip(&m.fixtures).enumerate() {
        let s = match o {
            0 => [0, 1],
            1 => [0, 0],
            _ => [1, 0],
        };
        scores[f.request_index] = s;
        m.add(&mut c, i, s);
    }
    (c, scores)
}

fn score_independent_exact(m: &Model, c: &[crate::model::Campaign]) -> bool {
    for a in 0..m.n {
        for b in a + 1..m.n {
            let mut decided = false;
            for key in &m.keys {
                let x = match key {
                    Key::Pt => c[a].points,
                    Key::W => c[a].wins,
                    Key::Bias => c[a].bias,
                    Key::Aet | Key::Gp => 0,
                    _ => return false,
                };
                let y = match key {
                    Key::Pt => c[b].points,
                    Key::W => c[b].wins,
                    Key::Bias => c[b].bias,
                    Key::Aet | Key::Gp => 0,
                    _ => return false,
                };
                if x != y {
                    decided = true;
                    break;
                }
            }
            if !decided {
                return false;
            }
        }
    }
    true
}

fn combinations(
    m: &Model,
    outcomes: &[usize],
    mass: f64,
    buckets: Option<&[FixtureBuckets]>,
    leaf_limit: usize,
    leaves: &mut Vec<Leaf>,
) -> bool {
    let (c, _) = path_scores(m, outcomes);
    let mut relevant = Vec::new();
    if buckets.is_some() {
        for a in 0..m.n {
            for b in a + 1..m.n {
                if c[a].points == c[b].points {
                    if let Some((i, _)) =
                        m.fixtures.iter().enumerate().find(|(_, f)| {
                            (f.home == a && f.away == b) || (f.home == b && f.away == a)
                        })
                    {
                        relevant.push(i);
                    }
                }
            }
        }
    }
    let mut selected = vec![None; m.fixtures.len()];
    fn visit(
        m: &Model,
        outcomes: &[usize],
        path_mass: f64,
        buckets: Option<&[FixtureBuckets]>,
        leaf_limit: usize,
        relevant: &[usize],
        depth: usize,
        selected: &mut [Option<usize>],
        leaves: &mut Vec<Leaf>,
    ) -> bool {
        if depth == relevant.len() {
            if leaves.len() >= leaf_limit {
                return false;
            }
            let mut mass = path_mass;
            if let Some(buckets) = buckets {
                for &i in relevant {
                    mass *= buckets[i].head[outcomes[i]][selected[i].unwrap()].1.mass;
                }
            }
            let mut scores = path_scores(m, outcomes).1;
            if let Some(buckets) = buckets {
                for &i in relevant {
                    let bi = selected[i].unwrap();
                    let bucket = &buckets[i].head[outcomes[i]][bi].1;
                    scores[m.fixtures[i].request_index] = bucket.representative;
                }
            }
            let (c, _) = path_scores(m, outcomes);
            let mut exact = if buckets.is_some() {
                true
            } else {
                score_independent_exact(m, &c)
            };
            if buckets.is_some()
                && m.n <= 2
                && (0..m.n).any(|a| (0..m.n).any(|b| b != a && c[a].points == c[b].points))
            {
                exact = false;
            }
            if buckets.is_some()
                && (0..m.n).any(|a| (0..m.n).filter(|&b| c[a].points == c[b].points).count() > 2)
            {
                exact = false;
            }
            if buckets.is_some() && exact {
                let mut cert_rng = Rng::new(0);
                for a in 0..m.n {
                    for b in a + 1..m.n {
                        if c[a].points == c[b].points
                            && m.head_order(a, b, &scores, &mut cert_rng) == Ordering::Equal
                        {
                            exact = false;
                        }
                    }
                }
            }
            let mut ids = vec![None; m.fixtures.len()];
            ids.copy_from_slice(selected);
            leaves.push(Leaf {
                outcomes: outcomes.to_vec(),
                bucket_ids: ids,
                mass,
                exact,
            });
            return true;
        }
        let i = relevant[depth];
        let Some(buckets) = buckets else {
            return true;
        };
        for bi in 0..buckets[i].head[outcomes[i]].len() {
            if buckets[i].head[outcomes[i]][bi].1.mass > 0. {
                selected[i] = Some(bi);
                if !visit(
                    m,
                    outcomes,
                    path_mass,
                    Some(buckets),
                    leaf_limit,
                    relevant,
                    depth + 1,
                    selected,
                    leaves,
                ) {
                    return false;
                }
            }
        }
        selected[i] = None;
        true
    }
    visit(
        m,
        outcomes,
        mass,
        buckets,
        leaf_limit,
        &relevant,
        0,
        &mut selected,
        leaves,
    )
}

/// Returns `None` without touching any output when the bounded path plan is
/// unsupported or exceeds its deterministic stratum cap.
pub fn estimate(m: &Model, seed: i64) -> Option<(Vec<Estimate>, serde_json::Value)> {
    if !eligible(m) {
        return None;
    }
    let mut use_head = head_optimization_safe(m);
    let planned = plan(m, use_head).or_else(|| {
        if use_head {
            use_head = false;
            plan(m, false)
        } else {
            None
        }
    });
    let (path_count, path_work, budget, ordinary_work, draw_budget) = planned?;
    let leaf_unit = leaf_work_unit(m, use_head)?;
    let leaf_limit = ((budget - path_work) / leaf_unit).min(MAX_STRATA as u64) as usize;
    if leaf_limit < path_count {
        return None;
    }
    let buckets: Vec<_> = if use_head {
        (0..m.fixtures.len())
            .map(|i| head_buckets::build(m, i))
            .collect()
    } else {
        Vec::new()
    };
    for (i, f) in m.fixtures.iter().enumerate() {
        if use_head {
            for o in 0..3 {
                if f.prob[o] > 0.
                    && (buckets[i].outcomes[o].is_none() || buckets[i].head[o].is_empty())
                {
                    return None;
                }
            }
        }
    }
    let bucket_ref = if use_head {
        Some(buckets.as_slice())
    } else {
        None
    };
    let mut leaves = Vec::new();
    let mut outcomes = vec![0; m.fixtures.len()];
    fn paths(
        m: &Model,
        buckets: Option<&[FixtureBuckets]>,
        leaf_limit: usize,
        outcomes: &mut [usize],
        i: usize,
        prior: f64,
        leaves: &mut Vec<Leaf>,
    ) -> bool {
        if i == outcomes.len() {
            return combinations(m, outcomes, prior, buckets, leaf_limit, leaves);
        }
        for o in 0..3 {
            let p = m.fixtures[i].prob[o];
            if p > 0. {
                outcomes[i] = o;
                if !paths(m, buckets, leaf_limit, outcomes, i + 1, prior * p, leaves) {
                    return false;
                }
            }
        }
        true
    }
    if !paths(m, bucket_ref, leaf_limit, &mut outcomes, 0, 1., &mut leaves) {
        return None;
    }
    let unresolved = leaves.iter().filter(|l| !l.exact).count();
    let leaf_cert = (leaves.len() as u64).checked_mul(leaf_unit)?;
    let exact_rank = ((leaves.len() - unresolved) as u64).checked_mul(ordinary_work as u64)?;
    let total_plan_work = path_work.checked_add(leaf_cert)?.checked_add(exact_rank)?;
    if total_plan_work >= budget {
        return None;
    }
    let remaining_draws = ((budget - total_plan_work) / (ordinary_work as u64))
        .min(draw_budget as u64)
        .min(MAX_DRAWS as u64) as usize;
    let per = if unresolved == 0 {
        0
    } else {
        (remaining_draws / unresolved).min(MAX_DRAWS / unresolved)
    };
    if unresolved > MAX_STRATA || (unresolved > 0 && per < DRAWS_PER_STRATUM) {
        return None;
    }
    let mut p = vec![0.; m.n * m.n];
    let mut variance = vec![0.; m.n * m.n];
    let mut hits = vec![0usize; m.n * m.n];
    let mut draws = 0usize;
    let mut event_weight = vec![0.; m.n * m.n];
    let mut event_weight_sq = vec![0.; m.n * m.n];
    let mut max_event: Vec<f64> = vec![0.; m.n * m.n];
    for (li, leaf) in leaves.iter().enumerate() {
        if leaf.exact {
            let mut scores = path_scores(m, &leaf.outcomes).1;
            if let Some(buckets) = bucket_ref {
                for (i, bid) in leaf.bucket_ids.iter().enumerate() {
                    if let Some(b) = bid {
                        scores[m.fixtures[i].request_index] =
                            buckets[i].head[leaf.outcomes[i]][*b].1.representative;
                    }
                }
            }
            let mut c = m.base.clone();
            for (i, f) in m.fixtures.iter().enumerate() {
                m.add(&mut c, i, scores[f.request_index]);
            }
            let mut order = vec![0; m.n];
            m.standings(&mut order, &c, &scores, &mut Rng::new(0));
            for (rank, t) in order.into_iter().enumerate() {
                p[t * m.n + rank] += leaf.mass;
            }
        } else if per > 0 {
            let mut counts = vec![0usize; m.n * m.n];
            let mut rng = Rng::new(derive(seed, &format!("small-group-{li}")));
            for _ in 0..per {
                let mut scores = path_scores(m, &leaf.outcomes).1;
                for (i, f) in m.fixtures.iter().enumerate() {
                    let s = if let (Some(b), Some(buckets)) = (leaf.bucket_ids[i], bucket_ref) {
                        buckets[i].head[leaf.outcomes[i]][b].1.sample(&mut rng)
                    } else {
                        m.fixtures[i].scores.sample(leaf.outcomes[i], &mut rng)
                    };
                    scores[f.request_index] = s;
                }
                let mut c = m.base.clone();
                for (i, f) in m.fixtures.iter().enumerate() {
                    m.add(&mut c, i, scores[f.request_index]);
                }
                let mut order = vec![0; m.n];
                m.standings(&mut order, &c, &scores, &mut rng);
                for (rank, t) in order.into_iter().enumerate() {
                    counts[t * m.n + rank] += 1;
                }
            }
            draws += per;
            for j in 0..p.len() {
                let q = counts[j] as f64 / per as f64;
                p[j] += leaf.mass * q;
                hits[j] += counts[j];
                let w = leaf.mass / per as f64;
                event_weight[j] += w * counts[j] as f64;
                event_weight_sq[j] += w * w * counts[j] as f64;
                if counts[j] > 0 {
                    max_event[j] = max_event[j].max(w);
                }
                variance[j] +=
                    leaf.mass * leaf.mass * q * (1. - q) / (per.saturating_sub(1).max(1) as f64);
            }
        }
    }
    let unresolved_mass = leaves
        .iter()
        .filter(|l| !l.exact)
        .map(|l| l.mass)
        .sum::<f64>();
    let result = (0..p.len())
        .map(|j| {
            let se = variance[j].sqrt();
            let rel = if p[j] > 0. { Some(se / p[j]) } else { None };
            Estimate {
                probability: p[j],
                std_err: se,
                samples: draws,
                hits: hits[j],
                ess: if event_weight_sq[j] > 0. {
                    event_weight[j] * event_weight[j] / event_weight_sq[j]
                } else {
                    0.
                },
                // The sampling component carries only unresolved leaf mass;
                // exact leaf contributions are analytic and are not observations.
                mean_weight: if draws > 0 {
                    unresolved_mass / draws as f64
                } else {
                    0.
                },
                available: true,
                work_spent: draws as u64,
                meets_precision_goal: p[j] > 0.
                    && se / p[j] <= 0.5
                    && unresolved_mass <= 0.5 * p[j],
                relative_se: rel,
                max_event_weight_share: if p[j] > 0. { max_event[j] / p[j] } else { 0. },
                zero_hit_upper_95: if p[j] == 0. {
                    (unresolved_mass + 1e-15).min(1.)
                } else {
                    0.
                },
                design: "outcome_path_stratified".into(),
                reachability: if p[j] > 0. {
                    "reachable_by_construction".into()
                } else {
                    "undecided".into()
                },
                ..Default::default()
            }
        })
        .collect();
    Some((
        result,
        serde_json::json!({"paths":path_count,"strata":leaves.len(),"exact_strata":leaves.len()-unresolved,
        "sampled_strata":unresolved,"conditional_draws":draws,"draws_per_unresolved_stratum":per,
        "cost_budget_ordinary_seasons":ORDINARY_SEASONS,"ordinary_season_work":ordinary_work,
        "certification_and_preparation_work":total_plan_work,"total_work_budget":budget,"conditional_draw_work_budget":remaining_draws as u64 * ordinary_work as u64,
        "strategy":"positive-probability WDL paths; optional safe two-team H2H score buckets"}),
    ))
}
