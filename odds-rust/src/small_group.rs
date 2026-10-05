//! Outcome path stratification for small groups whose first two ranking keys
//! are points and head-to-head. WDL path priors are exact; score ties inside a
//! path are sampled from conditional Poisson score buckets.
use crate::{
    head_buckets::{self, FixtureBuckets},
    model::{Key, Model},
    pool::Estimate,
    rng::{derive, Rng},
};
use std::{cmp::Ordering, collections::HashSet};

const MAX_TEAMS: usize = 6;
const MAX_FUTURE: usize = 8;
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
    let keys = &m.keys;
    m.n <= MAX_TEAMS
        && m.fixtures.len() <= MAX_FUTURE
        && keys.get(0) == Some(&Key::Pt)
        && keys.get(1) == Some(&Key::Head)
        && m.request.phase.bonus_points == 0
        && m.ids.len() == m.n
        && m.fixtures.iter().all(|f| {
            f.home < m.n
                && f.away < m.n
                && f.home != f.away
                && f.home_sampler.mean <= 32.
                && f.away_sampler.mean <= 32.
        })
        && {
            let mut pairs = HashSet::new();
            m.fixtures
                .iter()
                .all(|f| pairs.insert((f.home.min(f.away), f.home.max(f.away))))
        }
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

fn combinations(
    m: &Model,
    outcomes: &[usize],
    mass: f64,
    buckets: &[FixtureBuckets],
    leaves: &mut Vec<Leaf>,
) -> bool {
    let (c, _) = path_scores(m, outcomes);
    let mut relevant = Vec::new();
    for a in 0..m.n {
        for b in a + 1..m.n {
            if c[a].points == c[b].points {
                if let Some((i, _)) = m
                    .fixtures
                    .iter()
                    .enumerate()
                    .find(|(_, f)| (f.home == a && f.away == b) || (f.home == b && f.away == a))
                {
                    relevant.push(i);
                }
            }
        }
    }
    let mut selected = vec![None; m.fixtures.len()];
    fn visit(
        m: &Model,
        outcomes: &[usize],
        path_mass: f64,
        buckets: &[FixtureBuckets],
        relevant: &[usize],
        depth: usize,
        selected: &mut [Option<usize>],
        leaves: &mut Vec<Leaf>,
    ) -> bool {
        if depth == relevant.len() {
            if leaves.len() >= MAX_STRATA {
                return false;
            }
            let mut mass = path_mass;
            for &i in relevant {
                mass *= buckets[i].head[outcomes[i]][selected[i].unwrap()].1.mass;
            }
            let mut scores = path_scores(m, outcomes).1;
            for &i in relevant {
                let bi = selected[i].unwrap();
                let bucket = &buckets[i].head[outcomes[i]][bi].1;
                scores[m.fixtures[i].request_index] = bucket.representative;
            }
            let (c, _) = path_scores(m, outcomes);
            let mut exact = true;
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
        for bi in 0..buckets[i].head[outcomes[i]].len() {
            if buckets[i].head[outcomes[i]][bi].1.mass > 0. {
                selected[i] = Some(bi);
                if !visit(
                    m,
                    outcomes,
                    path_mass,
                    buckets,
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
    let buckets: Vec<_> = (0..m.fixtures.len())
        .map(|i| head_buckets::build(m, i))
        .collect();
    for (i, f) in m.fixtures.iter().enumerate() {
        for o in 0..3 {
            if f.prob[o] > 0. && (buckets[i].outcomes[o].is_none() || buckets[i].head[o].is_empty())
            {
                return None;
            }
        }
    }
    let mut leaves = Vec::new();
    let mut outcomes = vec![0; m.fixtures.len()];
    fn paths(
        m: &Model,
        buckets: &[FixtureBuckets],
        outcomes: &mut [usize],
        i: usize,
        prior: f64,
        leaves: &mut Vec<Leaf>,
    ) -> bool {
        if i == outcomes.len() {
            return combinations(m, outcomes, prior, buckets, leaves);
        }
        for o in 0..3 {
            let p = m.fixtures[i].prob[o];
            if p > 0. {
                outcomes[i] = o;
                if !paths(m, buckets, outcomes, i + 1, prior * p, leaves) {
                    return false;
                }
            }
        }
        true
    }
    if !paths(m, &buckets, &mut outcomes, 0, 1., &mut leaves) {
        return None;
    }
    let unresolved = leaves.iter().filter(|l| !l.exact).count();
    if unresolved.saturating_mul(DRAWS_PER_STRATUM) > MAX_DRAWS {
        return None;
    }
    let per = if unresolved == 0 {
        0
    } else {
        DRAWS_PER_STRATUM
    };
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
            for (i, bid) in leaf.bucket_ids.iter().enumerate() {
                if let Some(b) = bid {
                    scores[m.fixtures[i].request_index] =
                        buckets[i].head[leaf.outcomes[i]][*b].1.representative;
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
                    let s = if let Some(b) = leaf.bucket_ids[i] {
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
        serde_json::json!({"paths":3usize.pow(m.fixtures.len() as u32),"strata":leaves.len(),"exact_strata":leaves.len()-unresolved,
        "sampled_strata":unresolved,"conditional_draws":draws,"draws_per_unresolved_stratum":per,"score_buckets":"H2H comparator less/equal/greater"}),
    ))
}
