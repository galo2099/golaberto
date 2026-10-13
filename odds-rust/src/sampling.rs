use crate::rng::Rng;
pub struct Poisson {
    pub mean: f64,
    cdf: Vec<f64>,
    lookup: [u8; 256],
}
impl Poisson {
    pub fn new(mean: f64) -> Self {
        let mut cdf = Vec::new();
        let mut lookup = [0; 256];
        if mean <= 0. {
            cdf.push(1.);
        } else {
            let mut p = (-mean).exp();
            let mut sum = p;
            cdf.push(sum);
            for k in 1..1000 {
                if sum >= 1. - 1e-15 {
                    break;
                }
                p *= mean / k as f64;
                sum += p;
                cdf.push(sum);
            }
            *cdf.last_mut().unwrap() = 1.;
            let mut score = 0;
            for (bin, entry) in lookup.iter_mut().enumerate() {
                while cdf[score] < bin as f64 / 256. {
                    score += 1;
                }
                *entry = score.min(255) as u8;
            }
        }
        Self { mean, cdf, lookup }
    }
    /// Return the adjacent differences of the CDF used by the native sampler.
    /// The final CDF entry is forced to one during construction, so this law
    /// includes that same terminal normalization (unlike `masses(mean)`).
    /// Means above 32 use an unbounded arrival-time sampler and are unsupported.
    pub(crate) fn native_masses(&self) -> Option<Vec<f64>> {
        if !self.mean.is_finite() || self.mean < 0. || self.mean > 32. {
            return None;
        }
        if self.mean == 0. {
            return Some(vec![1.]);
        }
        let mut previous = 0.;
        let mut masses = Vec::with_capacity(self.cdf.len());
        for &cdf in &self.cdf {
            let mass = cdf - previous;
            if !mass.is_finite() || mass < 0. {
                return None;
            }
            masses.push(mass);
            previous = cdf;
        }
        Some(masses)
    }
    #[inline]
    pub fn sample(&self, rng: &mut Rng) -> i32 {
        if self.mean <= 0. {
            return 0;
        }
        if self.mean > 32. {
            let mut time = 0.;
            let mut count = 0;
            loop {
                time -= (1. - rng.float()).ln();
                if time >= self.mean {
                    return count;
                }
                count += 1;
            }
        }
        let u = rng.float();
        let mut s = self.lookup[(u * 256.) as usize] as usize;
        while self.cdf[s] < u {
            s += 1;
        }
        s as i32
    }
}
pub fn masses(mean: f64) -> Vec<f64> {
    let mut p = (-mean).exp();
    let mut sum = p;
    let mut v = vec![p];
    for k in 1..1000 {
        if 1. - sum <= 1e-15 {
            break;
        }
        p = p * mean / k as f64;
        sum += p;
        v.push(p);
    }
    v
}
pub struct Scores {
    pub prob: [f64; 3],
    values: [Vec<([i32; 2], f64)>; 3],
    lookup: Box<[[u32; 256]; 3]>,
    conditional_support_len: [usize; 3],
}
impl Scores {
    pub fn new(home: f64, away: f64) -> Self {
        let mut prob = [0.; 3];
        let mut values: [Vec<([i32; 2], f64)>; 3] = std::array::from_fn(|_| Vec::new());
        for (h, hp) in masses(home).iter().enumerate() {
            for (a, ap) in masses(away).iter().enumerate() {
                let o = if h < a {
                    0
                } else if h == a {
                    1
                } else {
                    2
                };
                let p = hp * ap;
                if p > 0. {
                    prob[o] += p;
                    values[o].push(([h as i32, a as i32], prob[o]));
                }
            }
        }
        for o in 0..3 {
            if prob[o] > 0. {
                for (_, cdf) in &mut values[o] {
                    *cdf /= prob[o];
                }
                values[o].last_mut().unwrap().1 = 1.;
            }
        }
        let total: f64 = prob.iter().sum();
        for p in &mut prob {
            *p /= total;
        }
        let lookup = std::array::from_fn(|o| {
            let v = &values[o];
            let mut i = 0;
            std::array::from_fn(|bin| {
                while i < v.len() && v[i].1 < bin as f64 / 256. {
                    i += 1;
                }
                i as u32
            })
        });
        let conditional_support_len = std::array::from_fn(|outcome| {
            values[outcome]
                .iter()
                .enumerate()
                .filter(|(index, (_, cdf))| {
                    let previous = if *index == 0 {
                        0.0
                    } else {
                        values[outcome][index - 1].1
                    };
                    *cdf > previous
                })
                .count()
        });
        Self {
            prob,
            values,
            lookup: Box::new(lookup),
            conditional_support_len,
        }
    }

    /// Number of positive-mass rows in the native conditional CDF for one WDL.
    /// This lets bounded backtracking account for score-support scans without
    /// enumerating the score rows during every query.
    pub(crate) fn conditional_support_len(&self, outcome: usize) -> usize {
        self.conditional_support_len[outcome.min(2)]
    }

    /// Enumerate the conditional masses represented by the sampler's CDF.
    /// Adjacent differences reproduce the native conditional score choices,
    /// including its forced terminal CDF value.
    pub(crate) fn conditional_scores(
        &self,
        outcome: usize,
    ) -> impl Iterator<Item = ([i32; 2], f64)> + '_ {
        let values = &self.values[outcome];
        values
            .iter()
            .enumerate()
            .filter_map(move |(index, (score, cdf))| {
                let previous = if index == 0 { 0.0 } else { values[index - 1].1 };
                let mass = *cdf - previous;
                (mass > 0.0).then_some((*score, mass))
            })
    }

    #[inline]
    pub fn sample(&self, outcome: usize, rng: &mut Rng) -> [i32; 2] {
        let v = &self.values[outcome];
        assert!(!v.is_empty(), "zero mass outcome");
        let u = rng.float();
        let mut i = self.lookup[outcome][(u * 256.) as usize] as usize;
        while i < v.len() && v[i].1 < u {
            i += 1;
        }
        v[i.min(v.len() - 1)].0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_poisson_masses_match_the_sampler_cdf_law() {
        let zero = Poisson::new(0.);
        assert_eq!(zero.native_masses().unwrap(), vec![1.]);
        assert!(Poisson::new(32.0001).native_masses().is_none());

        for mean in [0.01, 0.7, 5.3, 32.] {
            let poisson = Poisson::new(mean);
            let actual = poisson.native_masses().unwrap();
            assert_eq!(actual.len(), poisson.cdf.len());
            assert!((actual.iter().sum::<f64>() - 1.).abs() < 1e-15);
            let mut previous = 0.;
            for (&mass, &cdf) in actual.iter().zip(&poisson.cdf) {
                assert_eq!(mass.to_bits(), (cdf - previous).to_bits());
                previous = cdf;
            }
        }

        let poisson = Poisson::new(1.);
        let native = poisson.native_masses().unwrap();
        let raw = masses(1.);
        assert_eq!(native.len(), raw.len());
        assert_ne!(
            native.last().unwrap().to_bits(),
            raw.last().unwrap().to_bits()
        );
    }
    #[test]
    fn indexed_scores_match_binary_search_at_boundaries_and_fixed_rng() {
        for (h, a) in [(0., 0.), (0., 1.2), (1.1, 0.), (1.4, 0.9), (7.3, 12.6)] {
            let scores = Scores::new(h, a);
            for outcome in 0..3 {
                let v = &scores.values[outcome];
                if v.is_empty() {
                    continue;
                }
                for u in (0..256)
                    .map(|b| b as f64 / 256.)
                    .chain(v.iter().flat_map(|(_, c)| [*c, c.next_down(), c.next_up()]))
                    .filter(|&u| (0. ..1.).contains(&u))
                {
                    let expected = v.partition_point(|(_, c)| *c < u).min(v.len() - 1);
                    let mut i = scores.lookup[outcome][(u * 256.) as usize] as usize;
                    while i < v.len() && v[i].1 < u {
                        i += 1;
                    }
                    assert_eq!(i.min(v.len() - 1), expected, "{h}/{a}, {outcome}, {u}");
                }
                let mut old_rng = Rng::new(808);
                let mut new_rng = Rng::new(808);
                for _ in 0..10000 {
                    let u = old_rng.float();
                    let i = v.partition_point(|(_, c)| *c < u).min(v.len() - 1);
                    assert_eq!(scores.sample(outcome, &mut new_rng), v[i].0);
                }
                assert_eq!(old_rng.float(), new_rng.float());
            }
        }
    }
}
