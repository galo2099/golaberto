use golaberto_odds::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub lo: f64,
    pub hi: f64,
}

fn down(x: f64) -> f64 {
    if x <= 0.0 || !x.is_finite() {
        x.max(0.0)
    } else {
        f64::from_bits(x.to_bits() - 1)
    }
}
fn up(x: f64) -> f64 {
    if x == 0.0 {
        f64::from_bits(1)
    } else if !x.is_finite() {
        x
    } else {
        f64::from_bits(x.to_bits() + 1)
    }
}
impl Interval {
    pub fn point(x: f64) -> Self {
        Self { lo: x, hi: x }
    }
    pub fn add(self, b: Self) -> Self {
        if self.hi == 0.0 && b.hi == 0.0 {
            return Self::point(0.0);
        }
        Self {
            lo: down(self.lo + b.lo),
            hi: up(self.hi + b.hi),
        }
    }
    pub fn mul(self, b: Self) -> Self {
        if (self.lo == 0.0 && self.hi == 0.0) || (b.lo == 0.0 && b.hi == 0.0) {
            return Self::point(0.0);
        }
        let v = [
            self.lo * b.lo,
            self.lo * b.hi,
            self.hi * b.lo,
            self.hi * b.hi,
        ];
        Self {
            lo: down(v.iter().copied().fold(f64::INFINITY, f64::min)),
            hi: up(v.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
        }
    }
    pub fn div(self, b: Self) -> Self {
        assert!(b.lo > 0.0);
        if self.lo == 0.0 && self.hi == 0.0 {
            return Self::point(0.0);
        }
        self.mul(Interval {
            lo: down(1.0 / b.hi),
            hi: up(1.0 / b.lo),
        })
    }
    pub fn mid(self) -> f64 {
        self.lo + (self.hi - self.lo) * 0.5
    }
    pub fn clamp_probability(self) -> Self {
        Self {
            lo: self.lo.max(0.0),
            hi: self.hi.min(1.0),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Pair {
    pub home: i32,
    pub away: i32,
    pub mass: Interval,
}
#[derive(Clone, Debug)]
pub struct Law {
    pub outcomes: [Interval; 3],
    pub pairs: [Vec<Pair>; 3],
}

fn accepted_count() -> u64 {
    let mut lo = 0u64;
    let mut hi = 1u64 << 63;
    while lo < hi {
        let m = lo + (hi - lo) / 2;
        let f = (m as f64) / 9223372036854775808.0;
        if f == 1.0 {
            hi = m
        } else {
            lo = m + 1
        }
    }
    lo
}
fn count_le(cdf: f64, n: u64) -> u64 {
    if cdf < 0.0 {
        return 0;
    }
    if cdf >= 1.0 {
        return n;
    }
    let mut lo = 0u64;
    let mut hi = n;
    while lo < hi {
        let m = lo + (hi - lo) / 2;
        let f = (m as f64) / 9223372036854775808.0;
        if f <= cdf {
            lo = m + 1
        } else {
            hi = m
        }
    }
    lo
}
fn frac(x: u64, n: u64) -> Interval {
    if x == 0 {
        return Interval::point(0.0);
    }
    if x == n {
        return Interval::point(1.0);
    }
    fn cast_bounds(v: u64) -> Interval {
        let f = v as f64;
        if f as u128 == v as u128 {
            Interval::point(f)
        } else if (f as u128) < (v as u128) {
            Interval {
                lo: f,
                hi: f64::from_bits(f.to_bits() + 1),
            }
        } else {
            Interval {
                lo: f64::from_bits(f.to_bits() - 1),
                hi: f,
            }
        }
    }
    let a = cast_bounds(x);
    let b = cast_bounds(n);
    a.div(b).clamp_probability()
}
fn native_masses(mean: f64, n: u64) -> Result<Vec<Interval>, String> {
    if !mean.is_finite() || mean < 0.0 || mean > 32.0 {
        return Err("unsupported Poisson mean".into());
    }
    if mean == 0.0 {
        return Ok(vec![Interval::point(1.0)]);
    }
    let mut p = (-mean).exp();
    let mut sum = p;
    let mut cdf = vec![sum];
    for k in 1..1000 {
        if sum >= 1.0 - 1e-15 {
            break;
        }
        p *= mean / k as f64;
        sum += p;
        cdf.push(sum);
    }
    *cdf.last_mut().unwrap() = 1.0;
    let mut previous = 0u64;
    let mut out = Vec::with_capacity(cdf.len());
    for x in cdf {
        let now = count_le(x, n);
        out.push(frac(now.saturating_sub(previous), n));
        previous = now;
    }
    Ok(out)
}
impl Law {
    pub fn new(home_mean: f64, away_mean: f64) -> Result<Self, String> {
        let n = accepted_count();
        let h = native_masses(home_mean, n)?;
        let a = native_masses(away_mean, n)?;
        let mut pairs: [Vec<Pair>; 3] = std::array::from_fn(|_| Vec::new());
        let mut outcomes = [Interval::point(0.0); 3];
        for (hi, hm) in h.iter().enumerate() {
            for (ai, am) in a.iter().enumerate() {
                let o = if hi < ai {
                    0
                } else if hi == ai {
                    1
                } else {
                    2
                };
                let mass = hm.mul(*am);
                if mass.hi > 0.0 {
                    pairs[o].push(Pair {
                        home: hi as i32,
                        away: ai as i32,
                        mass,
                    });
                    outcomes[o] = outcomes[o].add(mass);
                }
            }
        }
        Ok(Self { outcomes, pairs })
    }
}

fn tickets(weights: &[f64]) -> Result<Vec<u64>, String> {
    if weights.is_empty() || weights.iter().any(|x| !x.is_finite() || *x < 0.0) {
        return Err("invalid categorical weights".into());
    }
    let max = weights.iter().copied().fold(0.0, f64::max);
    if max == 0.0 {
        return Err("zero categorical mass".into());
    }
    let scale = (1u64 << 52) as f64;
    Ok(weights
        .iter()
        .map(|w| {
            if *w == 0.0 {
                0
            } else {
                ((*w / max * scale).ceil() as u64).max(1)
            }
        })
        .collect())
}
fn ticket_total(tickets: &[u64]) -> Result<u64, String> {
    tickets
        .iter()
        .try_fold(0u64, |sum, &x| sum.checked_add(x))
        .ok_or_else(|| "categorical ticket total overflow".into())
}
pub fn categorical_probabilities(weights: &[f64]) -> Result<Vec<Interval>, String> {
    let t = tickets(weights)?;
    let total = ticket_total(&t)?;
    if total == 0 || total > i64::MAX as u64 {
        return Err("categorical ticket total unsupported".into());
    }
    Ok(t.into_iter()
        .map(|x| {
            if x == 0 {
                Interval::point(0.0)
            } else {
                frac(x, total)
            }
        })
        .collect())
}
pub fn categorical(weights: &[f64], rng: &mut Rng) -> Result<(usize, Interval), String> {
    let t = tickets(weights)?;
    let total = ticket_total(&t)?;
    let n = 1u64 << 63;
    if total == 0 || total > i64::MAX as u64 {
        return Err("categorical ticket total unsupported".into());
    }
    let reject = n % total;
    let x = loop {
        let v = rng.int63() as u64;
        if v >= reject {
            break v % total;
        }
    };
    let mut acc = 0u64;
    let mut chosen = None;
    for (i, w) in t.iter().enumerate() {
        acc += *w;
        if x < acc {
            chosen = Some(i);
            break;
        }
    }
    let i = chosen.ok_or("categorical selection failed")?;
    let probs = categorical_probabilities(weights)?;
    Ok((i, probs[i]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepted_grid_and_boundaries() {
        let n = accepted_count();
        assert_eq!(n, (1u64 << 63) - 512);
        for c in [0.0, 0.5, 0.75, 1.0] {
            let k = count_le(c, n);
            assert!(k <= n);
            if k < n {
                assert!(((k as f64) / 9223372036854775808.0) > c)
            }
        }
    }
    #[test]
    fn category_bounds_support_and_rng() {
        let w = [0.0, 1.0, 2.0];
        let p = categorical_probabilities(&w).unwrap();
        assert_eq!(p[0], Interval::point(0.));
        assert!(p[1].lo <= 1. / 3. && p[1].hi >= 1. / 3.);
        let (i, q) = categorical(&w, &mut Rng::new(9)).unwrap();
        assert!(i > 0 && q.lo > 0.);
        assert!(categorical(&[0., 0.], &mut Rng::new(1)).is_err())
    }
    #[test]
    fn rational_bounds_cover_casts_that_round_up_and_down() {
        let cast_bounds = |v: u64| {
            let f = v as f64;
            if f as u128 == v as u128 {
                Interval::point(f)
            } else if (f as u128) < (v as u128) {
                Interval {
                    lo: f,
                    hi: f64::from_bits(f.to_bits() + 1),
                }
            } else {
                Interval {
                    lo: f64::from_bits(f.to_bits() - 1),
                    hi: f,
                }
            }
        };
        let odd = (1u64 << 53) + 1;
        assert_eq!(cast_bounds(odd).lo, (1u64 << 53) as f64);
        let rounds_up = (1u64 << 53) + 3;
        assert_eq!(cast_bounds(rounds_up).hi, rounds_up as f64);
        let x = 3 * (1u64 << 61) + 1;
        let n = (1u64 << 63) - 512;
        let q = frac(x, n);
        fn scaled53(v: f64) -> u64 {
            let bits = v.to_bits();
            let exp = ((bits >> 52) & 0x7ff) as i32;
            let mant = (bits & ((1u64 << 52) - 1)) | (1u64 << 52);
            assert_eq!(exp, 1022);
            mant
        }
        // Both endpoints are near 1/2 and represented as integer / 2^53.
        let lo_num = scaled53(q.lo);
        let hi_num = scaled53(q.hi);
        assert!((lo_num as u128) * (n as u128) <= (x as u128) * (1u128 << 53));
        assert!((hi_num as u128) * (n as u128) >= (x as u128) * (1u128 << 53));
    }
    #[test]
    fn law_is_joint_and_normalized() {
        let l = Law::new(1.1, 0.7).unwrap();
        assert!(l.outcomes.iter().map(|x| x.mid()).sum::<f64>() > 0.999999999999);
        assert!(l.pairs.iter().flatten().all(|p| p.mass.lo >= 0.));
        assert!(Law::new(33., 1.).is_err())
    }
}
