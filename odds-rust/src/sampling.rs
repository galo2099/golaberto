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
        Self { prob, values }
    }
    #[inline]
    pub fn sample(&self, outcome: usize, rng: &mut Rng) -> [i32; 2] {
        let v = &self.values[outcome];
        assert!(!v.is_empty(), "zero mass outcome");
        let u = rng.float();
        let i = v.partition_point(|(_, c)| *c < u);
        v[i.min(v.len() - 1)].0
    }
}
