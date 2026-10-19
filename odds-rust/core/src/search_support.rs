use std::sync::atomic::{AtomicUsize, Ordering};
use crate::{conditioned::Result, lookahead::Policy, model::Model, pool::Estimate, rng::derive};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Cell {
    pub team: usize,
    pub rank: usize,
}

impl Cell {
    pub fn index(self, n: usize) -> usize {
        self.team * n + self.rank
    }
}

pub fn enabled(name: &str) -> bool {
    std::env::var(name).as_deref() != Ok("0")
}

pub fn parallel<T: Send, F: Fn(usize) -> T + Sync>(count: usize, workers: usize, f: F) -> Vec<T> {
    let next = AtomicUsize::new(0);
    let mut results = std::thread::scope(|scope| {
        let f = &f;
        let next = &next;
        let mut handles = Vec::new();
        for _ in 0..workers.clamp(1, 4).min(count) {
            handles.push(scope.spawn(move || {
                let mut result = Vec::new();
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= count {
                        break;
                    }
                    result.push((i, f(i)));
                }
                result
            }));
        }
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    results.sort_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, result)| result).collect()
}

pub fn policy(
    model: &Model,
    cell: Cell,
    seed: i64,
    stream: &str,
    samples: usize,
    tilt: f64,
    point_tilt: f64,
    force_points: bool,
    propagate: bool,
) -> Policy {
    Policy {
        samples,
        seed: derive(seed, &format!("{stream}-{}-{}", model.ids[cell.team], cell.rank)),
        tilt,
        point_tilt,
        force_points,
        propagate,
    }
}

pub fn apply(est: &mut Estimate, result: &Result, design: &str) {
    est.probability = result.probability;
    est.std_err = result.std_err;
    est.relative_se = Some(result.std_err / result.probability);
    est.ess = result.ess;
    est.max_event_weight_share = result.max_share;
    est.conditional_mass = result.mass;
    est.conditional_samples = result.samples;
    est.conditional_hits = result.hits;
    est.design = design.into();
    est.meets_precision_goal = result.ess >= 10. && result.std_err / result.probability <= 0.5;
    est.reachability = "witness".into();
    est.zero_hit_upper_95 = 0.;
}

pub fn crosscheck(result: Result, check: Result) -> (Result, bool, bool) {
    if check.probability <= 0.
        || check.hits < 30
        || check.ess < 5.
        || check.std_err / check.probability > 0.6
    {
        return (result, true, false);
    }
    let ratio = result.probability / check.probability;
    if (1. / 30. ..=30.).contains(&ratio) {
        return (result, true, false);
    }
    if check.coarse() {
        (check, true, true)
    } else {
        (result, false, false)
    }
}
