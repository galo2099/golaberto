//! Conservative confidence intervals and request-wide evidence accounting.
pub mod law;
pub mod matched;
pub mod tickets;
const PI_LOWER: f64 = 3.1415926535897927;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[cfg(test)]
fn certificates_enabled_from_env() -> bool {
    crate::test_env::var("RUST_ODDS_CERTIFICATES").as_deref() != Ok("0")
}
#[cfg(not(test))]
fn certificates_enabled_from_env() -> bool {
    std::env::var("RUST_ODDS_CERTIFICATES").as_deref() != Ok("0")
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Interval {
    pub lower: f64,
    pub upper: f64,
}

impl Interval {
    pub fn new(lower: f64, upper: f64) -> Self {
        Self { lower, upper }
    }
    pub fn add(self, rhs: Self) -> Self {
        Self {
            lower: down(self.lower + rhs.lower).max(0.0),
            upper: up(self.upper + rhs.upper),
        }
    }
    pub fn mul(self, rhs: Self) -> Self {
        Self {
            lower: down(self.lower * rhs.lower).max(0.0),
            upper: up(self.upper * rhs.upper),
        }
    }
    pub fn sub(self, rhs: Self) -> Self {
        Self {
            lower: down(self.lower - rhs.upper).max(0.0),
            upper: up(self.upper - rhs.lower).max(0.0),
        }
    }
}
fn down(x: f64) -> f64 {
    if x.is_nan() || x == f64::NEG_INFINITY {
        x
    } else if x == 0.0 {
        -f64::from_bits(1)
    } else {
        x.next_down()
    }
}
fn up(x: f64) -> f64 {
    if x.is_nan() || x == f64::INFINITY {
        x
    } else if x == 0.0 {
        f64::from_bits(1)
    } else {
        x.next_up()
    }
}

/// The adopted qualification rule. Geometric midpoint is computed in log space.
pub fn qualification(i: Interval) -> bool {
    if !valid_interval(i) || i.lower <= 0.0 {
        return false;
    }
    let log_lower = i.lower.log10();
    let log_upper = i.upper.log10();
    let log_guard = 1.0e-12 * log_lower.abs().max(log_upper.abs()).max(1.0);
    let mid_log_hi = up((log_lower + log_upper) * 0.5 + log_guard);
    let half_log_hi = up((log_upper - log_lower) * 0.5 + log_guard);
    let relative_scale_lo = (-mid_log_hi).max(1.0);
    let relative_bound_lo = down(0.05 * relative_scale_lo - log_guard).max(0.0);
    let width_hi = up(i.upper - i.lower);
    // A small absolute guard makes values sitting on either decimal boundary
    // fail closed when libm or subtraction rounding obscures which side they lie.
    width_hi <= 0.01 - 1.0e-14 && half_log_hi <= relative_bound_lo
}
fn valid_interval(i: Interval) -> bool {
    i.lower.is_finite()
        && i.upper.is_finite()
        && i.lower >= 0.0
        && i.upper <= 1.0
        && i.lower <= i.upper
}

/// Conservative binomial-tail inversion using a certified KL lower bound,
/// Robbins PMF bound, geometric tail ratio, and Chernoff fallback. This is not
/// an exact Clopper-Pearson interval; exclusion requires an upper tail bound
/// strictly below the allocated tail probability.
pub fn binomial_kl_interval(
    hits: usize,
    n: usize,
    alpha_two_sided: f64,
) -> Result<Interval, String> {
    if !alpha_two_sided.is_finite() || alpha_two_sided <= 0.0 || alpha_two_sided >= 1.0 {
        return Err("invalid alpha".into());
    }
    if hits > n {
        return Err("hits exceed sample count".into());
    }
    if n == 0 {
        return Ok(Interval::new(0.0, 1.0));
    }
    if hits > n - hits {
        let reflected = binomial_kl_interval(n - hits, n, alpha_two_sided)?;
        return Ok(Interval::new(
            down(1.0 - reflected.upper).max(0.0),
            up(1.0 - reflected.lower).min(1.0),
        ));
    }
    let p = observed_fraction(hits, n);
    let tail_alpha = down(alpha_two_sided * 0.5).max(0.0);
    let lower = if hits == 0 {
        0.0
    } else {
        bisect_binomial(hits, n, p, tail_alpha, false)
    };
    let upper = if hits == n {
        1.0
    } else {
        bisect_binomial(hits, n, p, tail_alpha, true)
    };
    Ok(Interval::new(down(lower).max(0.0), up(upper).min(1.0)))
}
fn observed_fraction(hits: usize, n: usize) -> Interval {
    if hits == 0 {
        return Interval::new(0.0, 0.0);
    }
    if hits == n {
        return Interval::new(1.0, 1.0);
    }
    let hit_lo = down(hits as f64).max(0.0);
    let hit_hi = up(hits as f64);
    let n_lo = down(n as f64);
    let n_hi = up(n as f64);
    Interval::new(down(hit_lo / n_hi).max(0.0), up(hit_hi / n_lo).min(1.0))
}

/// Lower enclosure of D(p||q), using the integral of |t-p|/[t(1-t)].
/// p and q are floating endpoints; partition endpoints are shared f64 values.
fn kl_lower(p: Interval, q: f64) -> f64 {
    let (p_star, lo, hi) = if q < p.lower {
        (p.lower, q, p.lower)
    } else if q > p.upper {
        (p.upper, p.upper, q)
    } else {
        return 0.0;
    };
    let span = hi - lo;
    if span <= 0.0 {
        return 0.0;
    }
    let mut total = 0.0;
    let mut left = lo;
    // Geometric partitions resolve the KL singularity at q=0 or q=1 while
    // keeping exactly sixteen deterministic segments.
    let near_lo = lo < span;
    let near_hi = 1.0 - hi < span;
    let cluster_lo = near_lo && (!near_hi || lo <= 1.0 - hi);
    let cluster_hi = near_hi && (!near_lo || 1.0 - hi < lo);
    for part in 1..=16 {
        let right = if part == 16 {
            hi
        } else {
            let fraction = if cluster_lo {
                2.0_f64.powi(part as i32 - 16)
            } else if cluster_hi {
                1.0 - 2.0_f64.powi(-(part as i32))
            } else {
                part as f64 / 16.0
            };
            (lo + span * fraction).max(left).min(hi)
        };
        if right > left {
            let width_lo = down(right - left).max(0.0);
            let distance_lo = if p_star <= left {
                down(left - p_star).max(0.0)
            } else {
                down(p_star - right).max(0.0)
            };
            let midpoint_distance_lo = down(distance_lo + down(width_lo * 0.5)).max(0.0);
            let area_lo = down(width_lo * midpoint_distance_lo).max(0.0);
            let x = if left <= 0.5 && right >= 0.5 {
                0.5
            } else if right < 0.5 {
                right
            } else {
                left
            };
            let one_minus_x_hi = up(1.0 - x).min(1.0);
            let denominator_hi = up(x * one_minus_x_hi);
            if denominator_hi > 0.0 {
                let part_lo = down(area_lo / denominator_hi).max(0.0);
                total = down(total + part_lo).max(0.0);
            }
        }
        left = right;
    }
    total
}

fn binomial_pmf_factor_upper(hits: usize, n: usize) -> f64 {
    let k_lo = down(hits as f64).max(0.0);
    let m_lo = down((n - hits) as f64).max(0.0);
    let n_hi = up(n as f64);
    let two_pi_lo = down(2.0 * PI_LOWER).max(0.0);
    let denom_lo = down(down(two_pi_lo * k_lo) * m_lo).max(0.0);
    if denom_lo == 0.0 {
        return 1.0;
    }
    let ratio_hi = up(n_hi / denom_lo);
    let root_hi = up(ratio_hi.sqrt());
    let n_lo = down(n as f64);
    let factor_hi = up(1.0 + up(1.0 / n_lo));
    up(factor_hi * root_hi).min(1.0)
}

/// Upper tail probability from a certified lower KL bound and a geometric
/// ratio bound on successive binomial PMFs. Robbins factorial bounds give the
/// PMF-at-k factor; all floating operations here widen outward.
fn binomial_tail_upper(hits: usize, n: usize, p: Interval, q: f64) -> f64 {
    if q <= 0.0 {
        return if hits == 0 { 1.0 } else { 0.0 };
    }
    if q >= 1.0 {
        return if hits == n { 1.0 } else { 0.0 };
    }
    if hits == 0 {
        return pow_upper(up(1.0 - q).min(1.0), n);
    }
    if hits == n {
        return pow_upper(q, n);
    }
    let (ratio_factor_hi, q_factor_hi) = if q < p.lower {
        let numerator_hi = up((n - hits) as f64);
        let denominator_lo = down((hits + 1) as f64);
        let ratio_hi = up(numerator_hi / denominator_lo);
        let q_ratio_hi = up(q / down(1.0 - q));
        (ratio_hi, q_ratio_hi)
    } else if q > p.upper {
        let numerator_hi = up(hits as f64);
        let denominator_lo = down((n - hits + 1) as f64);
        let ratio_hi = up(numerator_hi / denominator_lo);
        let q_ratio_hi = up(up(1.0 - q) / down(q));
        (ratio_hi, q_ratio_hi)
    } else {
        return 1.0;
    };
    let r_hi = up(ratio_factor_hi * q_factor_hi);
    let prefactor_hi = if r_hi >= 1.0 {
        1.0
    } else {
        let one_minus_r_lo = down(1.0 - r_hi).max(0.0);
        if one_minus_r_lo == 0.0 {
            1.0
        } else {
            up(binomial_pmf_factor_upper(hits, n) / one_minus_r_lo).min(1.0)
        }
    };
    let d_lo = kl_lower(p, q);
    let n_lo = down(n as f64);
    let exponent_lo = down(n_lo * d_lo).max(0.0);
    up(prefactor_hi * exp_negative_upper(exponent_lo)).min(1.0)
}

fn pow_upper(mut base: f64, mut exponent: usize) -> f64 {
    if base <= 0.0 {
        return 0.0;
    }
    base = up(base).min(1.0);
    let mut result = 1.0;
    while exponent > 0 {
        if exponent & 1 == 1 {
            result = up(result * base).min(1.0);
        }
        exponent >>= 1;
        if exponent > 0 {
            base = up(base * base).min(1.0);
        }
    }
    result
}

/// exp(-t) <= (1+t/65536)^(-65536); repeated outward squaring evaluates
/// the reciprocal power without relying on a libm exponential.
fn exp_negative_upper(t: f64) -> f64 {
    if t <= 0.0 {
        return 1.0;
    }
    let step_lo = down(t / 65536.0).max(0.0);
    let base_lo = down(1.0 + step_lo).max(1.0);
    let mut value = up(1.0 / base_lo).min(1.0);
    for _ in 0..16 {
        value = up(value * value).min(1.0);
    }
    value
}

fn bisect_binomial(hits: usize, n: usize, p: Interval, alpha: f64, upper: bool) -> f64 {
    let (mut lo, mut hi) = if upper {
        (p.upper, 1.0)
    } else {
        (0.0, p.lower)
    };
    for _ in 0..80 {
        let m = lo + (hi - lo) * 0.5;
        if m == lo || m == hi {
            break;
        }
        let tail_upper = binomial_tail_upper(hits, n, p, m);
        let excluded = tail_upper < alpha;
        if upper {
            if excluded {
                hi = m;
            } else {
                lo = m;
            }
        } else if excluded {
            lo = m;
        } else {
            hi = m;
        }
    }
    if upper {
        hi
    } else {
        lo
    }
}

/// Empirical Bernstein bound for a fixed-n IID variable in [0, cap].
/// Uses unbiased sample variance and a Hoeffding fallback, with a union bound
/// over the variance and mean deviations (Maurer-Pontil style constants).
pub fn bounded_mean_interval(
    n: usize,
    sum: f64,
    sum2: f64,
    cap: f64,
    alpha: f64,
) -> Result<Interval, String> {
    if n < 2
        || !cap.is_finite()
        || cap <= 0.0
        || !alpha.is_finite()
        || alpha <= 0.0
        || alpha >= 1.0
        || !sum.is_finite()
        || !sum2.is_finite()
    {
        return Err("invalid bounded-mean inputs".into());
    }
    let cap2 = up(cap * cap);
    let max_sum = up(n as f64 * cap);
    let max_sum2 = up(n as f64 * cap2);
    if !cap2.is_finite() || cap2 == 0.0 || !max_sum.is_finite() || !max_sum2.is_finite() {
        return Err("bounded-mean cap overflows or underflows".into());
    }
    if sum < 0.0 || sum > max_sum || sum2 < 0.0 || sum2 > max_sum2 {
        return Err("bounded-mean range violation".into());
    }
    // Enclose each nonnegative accumulated sum for at most n rounded adds.
    let neps = n as f64 * f64::EPSILON;
    if neps >= 1.0 {
        return Err("sample count too large for rounding enclosure".into());
    }
    let gamma = neps / (1.0 - neps);
    let sum_lo = (down(sum * (1.0 - gamma))).max(0.0);
    let sum_hi = up(sum * (1.0 + gamma)).min(max_sum);
    let sum2_lo = down(sum2 * (1.0 - gamma)).max(0.0);
    let sum2_hi = up(sum2 * (1.0 + gamma)).min(max_sum2);
    let mean_lo = down(sum_lo / up(n as f64)).max(0.0);
    let mean_hi = up(sum_hi / down(n as f64)).min(cap);
    if sum_lo * sum_lo / n as f64 > sum2_hi || sum2_lo > up(cap * sum_hi) {
        return Err("inconsistent bounded-mean moments".into());
    }
    let sq_lo = down(sum_lo * sum_lo).max(0.0);
    let mean_sq_lo = down(sq_lo / n as f64).max(0.0);
    let var_num_hi = up(sum2_hi - mean_sq_lo).max(0.0);
    let var = up(var_num_hi / down((n - 1) as f64)).max(0.0);
    let l = (8.0 / alpha).ln();
    let eb_sq = up(up(up(2.0 * var) * l) / down(n as f64)).max(0.0);
    let eb_var = up(eb_sq.sqrt());
    let eb_bias = up(up(up(7.0 * cap) * l) / down(3.0 * (n - 1) as f64));
    let eb = up(eb_var + eb_bias);
    let hoeff_l = (4.0 / alpha).ln();
    let hoeff = up(cap * up((up(hoeff_l / down(2.0 * n as f64)).max(0.0)).sqrt()));
    let radius = eb.min(hoeff);
    Ok(Interval::new(
        down(mean_lo - radius).max(0.0),
        up(mean_hi + radius).min(cap),
    ))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    pub interval: Interval,
    pub method: String,
    pub alpha: f64,
    pub samples: usize,
    pub contribution_cap: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Certificate {
    pub status: String,
    pub lower: f64,
    pub upper: f64,
    pub confidence_level: f64,
    pub scope: String,
    pub family_cells: usize,
    pub methods: Vec<String>,
    /// Per-source counts can reuse underlying draws (native and matched do).
    pub source_bounds: Vec<SourceBound>,
    pub evidence_samples: usize,
    /// Alpha attached to accepted evidence records.
    pub alpha_spent: f64,
    /// Per-cell allocations held aside, including unused IS tickets.
    pub alpha_reserved: f64,
    pub interval_qualifies: bool,
    pub certified_estimate: Option<f64>,
    pub reported_estimate_inside: bool,
    pub conflicts: Vec<String>,
    pub reason: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceBound {
    pub method: String,
    pub lower: f64,
    pub upper: f64,
    pub sample_uses: usize,
    pub alpha_spent: f64,
    pub record_count: usize,
}
impl Default for Certificate {
    fn default() -> Self {
        Self {
            status: "unavailable".into(),
            lower: 0.0,
            upper: 1.0,
            confidence_level: 0.99,
            scope: "declared_family_simultaneous".into(),
            family_cells: 1,
            methods: vec![],
            source_bounds: vec![],
            evidence_samples: 0,
            alpha_spent: 0.0,
            alpha_reserved: 0.0,
            interval_qualifies: false,
            certified_estimate: None,
            reported_estimate_inside: false,
            conflicts: vec![],
            reason: Some("no_evidence".into()),
        }
    }
}

#[derive(Default)]
struct Ledger {
    evidence: Vec<Evidence>,
    methods: BTreeSet<String>,
    conflicts: BTreeSet<String>,
    invalid: bool,
    native_records: usize,
    matched_records: usize,
    is_records: usize,
}
pub struct Budget {
    enabled: bool,
    family_cells: usize,
    native: f64,
    matched: f64,
    is: f64,
    ledgers: Vec<Mutex<Ledger>>,
    tickets: Vec<Mutex<usize>>,
    setup_ns: AtomicU64,
}
impl Budget {
    pub fn new(cells: usize) -> Self {
        let enabled = certificates_enabled_from_env();
        let env = std::env::var_os("RUST_ODDS_CERTIFICATE_FAMILY_CELLS");
        let declared = match env {
            Some(s) => s
                .to_str()
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(usize::MAX),
            None => 0,
        };
        let family_cells = cells.max(declared).max(1);
        let cells_f = up(family_cells as f64);
        let native = down(0.004 / cells_f).max(0.0);
        let matched = down(0.002 / cells_f).max(0.0);
        let is = down(down(0.004 / cells_f) / 1024.0).max(0.0);
        Self {
            enabled,
            family_cells,
            native,
            matched,
            is,
            ledgers: if enabled {
                (0..cells).map(|_| Mutex::new(Ledger::default())).collect()
            } else {
                Vec::new()
            },
            tickets: if enabled {
                (0..cells).map(|_| Mutex::new(0)).collect()
            } else {
                Vec::new()
            },
            setup_ns: AtomicU64::new(0),
        }
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn record_setup_ns(&self, ns: u64) {
        self.setup_ns.fetch_add(ns, Ordering::Relaxed);
    }
    pub fn setup_ns(&self) -> u64 {
        self.setup_ns.load(Ordering::Relaxed)
    }
    pub fn native_alpha(&self) -> f64 {
        self.native
    }
    pub fn matched_alpha(&self) -> f64 {
        self.matched
    }
    pub fn claim_is(&self, index: usize) -> Option<f64> {
        if !self.enabled {
            return None;
        }
        let t = self.tickets.get(index)?;
        let mut x = t.lock().ok()?;
        if *x >= 1024 {
            return None;
        }
        *x += 1;
        Some(self.is)
    }
    pub fn record(&self, index: usize, e: Evidence) {
        if !self.enabled {
            return;
        }
        if let Some(l) = self.ledgers.get(index) {
            if let Ok(mut l) = l.lock() {
                l.methods.insert(e.method.clone());
                let source_valid = if e.method == "native_mc" {
                    l.native_records == 0 && e.alpha.to_bits() == self.native.to_bits()
                } else if e.method == "matched_own_points_strata" {
                    l.matched_records == 0 && e.alpha.to_bits() == self.matched.to_bits()
                } else {
                    let claimed = self
                        .tickets
                        .get(index)
                        .and_then(|t| t.lock().ok().map(|v| *v))
                        .unwrap_or(0);
                    l.is_records < claimed && e.alpha.to_bits() == self.is.to_bits()
                };
                if !source_valid {
                    l.invalid = true;
                    l.conflicts.insert("alpha_budget_exceeded".into());
                } else if !valid_interval(e.interval)
                    || !e.alpha.is_finite()
                    || e.alpha <= 0.0
                    || e.alpha >= 1.0
                    || e.contribution_cap
                        .is_some_and(|x| !x.is_finite() || x <= 0.0)
                {
                    l.invalid = true;
                    l.conflicts.insert("invalid_evidence".into());
                } else {
                    if e.method == "native_mc" {
                        l.native_records += 1;
                    } else if e.method == "matched_own_points_strata" {
                        l.matched_records += 1;
                    } else {
                        l.is_records += 1;
                    }
                    l.evidence.push(e);
                }
            }
        }
    }
    pub fn finish(
        &self,
        index: usize,
        reported_probability: f64,
        proved_impossible: bool,
    ) -> Certificate {
        let mut c = Certificate {
            family_cells: self.family_cells,
            ..Certificate::default()
        };
        let Some(ledger) = self.ledgers.get(index) else {
            c.reason = Some("invalid_cell_index".into());
            return c;
        };
        let Ok(l) = ledger.lock() else {
            c.reason = Some("ledger_unavailable".into());
            return c;
        };
        c.methods = l.methods.iter().cloned().collect();
        c.conflicts = l.conflicts.iter().cloned().collect();
        // Counts evidence sample uses; independent sources may reuse the same
        // underlying draws, so this is not necessarily a count of unique draws.
        c.evidence_samples = l
            .evidence
            .iter()
            .fold(0usize, |a, e| a.saturating_add(e.samples));
        c.alpha_spent = l.evidence.iter().fold(0.0, |a, e| up(a + e.alpha));
        let is_claims = self
            .tickets
            .get(index)
            .and_then(|t| t.lock().ok().map(|v| *v))
            .unwrap_or(0);
        c.alpha_reserved = up(up(self.native + self.matched) + up(self.is * is_claims as f64));
        let mut by_method = BTreeMap::<String, Vec<&Evidence>>::new();
        for evidence in &l.evidence {
            by_method
                .entry(evidence.method.clone())
                .or_default()
                .push(evidence);
        }
        for (method, evidence) in by_method {
            let mut bounds = Interval::new(0.0, 1.0);
            let mut method_conflict = false;
            for e in &evidence {
                let lo = bounds.lower.max(e.interval.lower);
                let hi = bounds.upper.min(e.interval.upper);
                if lo > hi {
                    method_conflict = true;
                    break;
                }
                bounds = Interval::new(lo, hi);
            }
            if method_conflict {
                bounds = Interval::new(
                    evidence
                        .iter()
                        .map(|e| e.interval.lower)
                        .fold(f64::INFINITY, f64::min),
                    evidence
                        .iter()
                        .map(|e| e.interval.upper)
                        .fold(f64::NEG_INFINITY, f64::max),
                );
                c.conflicts.push(format!("source_conflict:{method}"));
            }
            c.source_bounds.push(SourceBound {
                method,
                lower: bounds.lower,
                upper: bounds.upper,
                sample_uses: evidence
                    .iter()
                    .fold(0usize, |a, e| a.saturating_add(e.samples)),
                alpha_spent: evidence.iter().fold(0.0, |a, e| up(a + e.alpha)),
                record_count: evidence.len(),
            });
        }
        let mut interval = Interval::new(0.0, 1.0);
        let mut conflict = false;
        for e in &l.evidence {
            let lo = interval.lower.max(e.interval.lower);
            let hi = interval.upper.min(e.interval.upper);
            if lo > hi {
                conflict = true;
                break;
            }
            interval = Interval::new(lo, hi);
        }
        if conflict {
            c.conflicts.push("confidence_conflict".into());
            interval = Interval::new(
                l.evidence
                    .iter()
                    .map(|e| e.interval.lower)
                    .fold(f64::INFINITY, f64::min),
                l.evidence
                    .iter()
                    .map(|e| e.interval.upper)
                    .fold(f64::NEG_INFINITY, f64::max),
            );
        }
        if proved_impossible {
            if l.evidence.iter().any(|e| e.interval.lower > 0.0) {
                c.conflicts.push("proof_conflict".into());
                c.status = "conflict".into();
                c.reason = Some("proof_contradicts_positive_lower_bound".into());
                c.lower = interval.lower;
                c.upper = interval.upper;
                return c;
            }
            interval = Interval::new(0.0, 0.0);
            c.status = "proved_impossible".into();
        } else if l.evidence.is_empty() || l.invalid {
            c.reason = Some(
                if l.invalid {
                    "invalid_evidence"
                } else {
                    "no_evidence"
                }
                .into(),
            );
            return c;
        } else if !c.conflicts.is_empty() {
            c.status = "conflict".into();
            c.reason = Some(
                if c.conflicts.iter().any(|x| x == "confidence_conflict") {
                    "confidence_conflict"
                } else {
                    "source_conflict"
                }
                .into(),
            );
        } else {
            c.status = "bounded".into();
        }
        c.lower = interval.lower;
        c.upper = interval.upper;
        c.interval_qualifies = !l.invalid && c.conflicts.is_empty() && qualification(interval);
        let mid = if interval.lower > 0.0 {
            Some(((interval.lower.ln() + interval.upper.ln()) * 0.5).exp())
        } else {
            None
        };
        c.certified_estimate = if c.interval_qualifies { mid } else { None };
        c.reported_estimate_inside = reported_probability.is_finite()
            && reported_probability >= interval.lower
            && reported_probability <= interval.upper;
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn enabled_budget(cells: usize) -> (crate::test_env::OverrideGuard, Budget) {
        let env = crate::test_env::scoped(&[("RUST_ODDS_CERTIFICATES", Some("1"))]);
        (env, Budget::new(cells))
    }
    fn evidence(interval: Interval, method: &str, alpha: f64) -> Evidence {
        Evidence {
            interval,
            method: method.into(),
            alpha,
            samples: 2,
            contribution_cap: None,
        }
    }
    #[test]
    fn binomial_edges_and_validation() {
        let a = binomial_kl_interval(0, 10, 0.05).unwrap();
        assert_eq!(a.lower, 0.0);
        assert!(a.upper > 0.0 && a.upper < 1.0);
        let b = binomial_kl_interval(10, 10, 0.05).unwrap();
        assert_eq!(b.upper, 1.0);
        assert!(b.lower > 0.0);
        assert_eq!(
            binomial_kl_interval(0, 0, 0.05).unwrap(),
            Interval::new(0.0, 1.0)
        );
        assert!(binomial_kl_interval(2, 1, 0.05).is_err());
        let rare = binomial_kl_interval(0, 1_000_000_000_000, 0.05).unwrap();
        assert!(rare.upper > 0.0 && rare.upper < 1.0);
        let near_one = binomial_kl_interval(999_999, 1_000_000, 0.05).unwrap();
        assert!(near_one.lower > 0.99 && near_one.upper < 1.0);
        let subnormal_alpha = binomial_kl_interval(5, 10, f64::from_bits(1)).unwrap();
        assert_eq!(subnormal_alpha, Interval::new(0.0, 1.0));
        for n in 2..=20 {
            for hits in 1..n {
                let a = binomial_kl_interval(hits, n, 0.1).unwrap();
                let b = binomial_kl_interval(n - hits, n, 0.1).unwrap();
                assert!(
                    (a.lower - (1.0 - b.upper)).abs() < 1e-12,
                    "n={n} k={hits} a={a:?} reflected={:?}",
                    Interval::new(1.0 - b.upper, 1.0 - b.lower)
                );
                assert!(
                    (a.upper - (1.0 - b.lower)).abs() < 1e-12,
                    "n={n} k={hits} a={a:?} reflected={:?}",
                    Interval::new(1.0 - b.upper, 1.0 - b.lower)
                );
            }
        }
    }
    #[test]
    fn integral_kl_lower_encloses_reference_grid() {
        let ps: [f64; 8] = [0.0, 1e-4, 0.1, 0.3, 0.5, 0.9, 0.9999, 1.0];
        let qs: [f64; 9] = [
            1e-40,
            1e-12,
            1e-4,
            0.02,
            0.2,
            0.5001,
            0.8,
            0.98,
            1.0 - 1e-12,
        ];
        for p in ps {
            for q in qs {
                if (p - q).abs() < 1e-8 {
                    continue;
                }
                let lower = kl_lower(Interval::new(p, p), q);
                let mut reference = 0.0;
                if p > 0.0 {
                    reference += p * (p / q).ln();
                }
                if p < 1.0 {
                    reference += (1.0 - p) * ((1.0 - p) / (1.0 - q)).ln();
                }
                assert!(
                    lower <= reference,
                    "p={p} q={q}: lower={lower} ref={reference}"
                );
                assert!(lower >= 0.0);
            }
        }
    }
    #[test]
    fn geometric_binomial_tail_bounds_cover_enumerated_cdfs() {
        for n in 2..=16 {
            for hits in 1..n {
                let p = observed_fraction(hits, n);
                for qi in 0..=100 {
                    let q = qi as f64 / 100.0;
                    let mut pmfs = vec![0.0; n + 1];
                    for j in 0..=n {
                        let mut choose = 1.0;
                        for r in 0..j {
                            choose *= (n - r) as f64 / (r + 1) as f64;
                        }
                        pmfs[j] = choose * q.powi(j as i32) * (1.0 - q).powi((n - j) as i32);
                    }
                    if q < p.lower {
                        let exact_tail: f64 = pmfs[hits..].iter().sum();
                        let upper = binomial_tail_upper(hits, n, p, q);
                        assert!(
                            upper + 1e-13 >= exact_tail,
                            "n={n} k={hits} q={q}: {upper} < {exact_tail}"
                        );
                    } else if q > p.upper {
                        let exact_tail: f64 = pmfs[..=hits].iter().sum();
                        let upper = binomial_tail_upper(hits, n, p, q);
                        assert!(
                            upper + 1e-13 >= exact_tail,
                            "n={n} k={hits} q={q}: {upper} < {exact_tail}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn binomial_tiny_n_coverage() {
        for n in 1..=8 {
            for j in 0..=100 {
                let p = j as f64 / 100.0;
                let mut covered = 0.0;
                for k in 0..=n {
                    let i = binomial_kl_interval(k, n, 0.1).unwrap();
                    if i.lower <= p && p <= i.upper {
                        let mut choose = 1.0;
                        for r in 0..k {
                            choose *= ((n - r) as f64) / ((r + 1) as f64);
                        }
                        covered += choose * p.powi(k as i32) * (1.0 - p).powi((n - k) as i32);
                    }
                }
                assert!(covered >= 0.899999, "n={n} p={p} coverage={covered}");
            }
        }
    }
    #[test]
    fn bounded_mean_checks_ranges() {
        assert!(bounded_mean_interval(4, 0.0, 0.0, 1.0, 0.05).unwrap().upper > 0.0);
        assert!(bounded_mean_interval(4, 5.0, 0.0, 1.0, 0.05).is_err());
    }
    #[test]
    fn certificate_disable_flag_avoids_ledger_allocations() {
        let _env = crate::test_env::scoped(&[("RUST_ODDS_CERTIFICATES", Some("0"))]);
        let b = Budget::new(8);
        assert!(!b.enabled());
        assert!(b.ledgers.is_empty() && b.tickets.is_empty());
        assert!(b.claim_is(0).is_none());
        b.record(
            0,
            evidence(Interval::new(0.2, 0.3), "native_mc", b.native_alpha()),
        );
        assert_eq!(b.finish(0, 0.25, false).status, "unavailable");
    }
    #[test]
    fn ledger_conflict_and_point_outside() {
        let (_env, b) = enabled_budget(1);
        b.record(
            0,
            evidence(Interval::new(0.2, 0.3), "native_mc", b.native_alpha()),
        );
        b.record(
            0,
            evidence(
                Interval::new(0.4, 0.5),
                "matched_own_points_strata",
                b.matched_alpha(),
            ),
        );
        let c = b.finish(0, 0.0, false);
        assert_eq!(c.status, "conflict");
        assert_eq!(c.lower, 0.2);
        assert_eq!(c.upper, 0.5);
        assert!(!c.reported_estimate_inside);
    }
    #[test]
    fn proof_contradiction_retains_positive_lower() {
        let (_env, b) = enabled_budget(1);
        b.record(
            0,
            evidence(Interval::new(0.1, 0.2), "native_mc", b.native_alpha()),
        );
        let c = b.finish(0, 0.0, true);
        assert_eq!(c.status, "conflict");
    }
    #[test]
    fn proof_conflict_sees_positive_source_even_when_hull_includes_zero() {
        let (_env, b) = enabled_budget(1);
        b.record(
            0,
            evidence(Interval::new(0.2, 0.4), "native_mc", b.native_alpha()),
        );
        b.record(
            0,
            evidence(
                Interval::new(0.0, 0.1),
                "matched_own_points_strata",
                b.matched_alpha(),
            ),
        );
        let c = b.finish(0, 0.0, true);
        assert_eq!(c.status, "conflict");
        assert_eq!(
            c.reason.as_deref(),
            Some("proof_contradicts_positive_lower_bound")
        );
        assert_eq!(c.lower, 0.0);
        assert_eq!(c.upper, 0.4);
    }
    #[test]
    fn is_tickets_are_never_reclaimed() {
        let (_env, b) = enabled_budget(1);
        let mut last = 0.0;
        for _ in 0..1024 {
            last = b.claim_is(0).unwrap();
        }
        assert!(b.claim_is(0).is_none());
        let c = b.finish(0, 0.0, false);
        assert!(c.alpha_reserved >= b.native_alpha() + b.matched_alpha() + last * 1024.0);
    }
    #[test]
    fn ledger_rejects_reused_and_overallocated_sources() {
        let (_env, b) = enabled_budget(1);
        let n = b.native_alpha();
        b.record(0, evidence(Interval::new(0.1, 0.3), "native_mc", n));
        b.record(0, evidence(Interval::new(0.1, 0.3), "native_mc", n));
        assert!(b
            .finish(0, 0.2, false)
            .conflicts
            .iter()
            .any(|c| c == "alpha_budget_exceeded"));

        let (_env, b) = enabled_budget(1);
        b.record(0, evidence(Interval::new(0.1, 0.3), "native_mc", n * 2.0));
        assert!(b
            .finish(0, 0.2, false)
            .conflicts
            .iter()
            .any(|c| c == "alpha_budget_exceeded"));

        let (_env, b) = enabled_budget(1);
        let alpha = b.claim_is(0).unwrap();
        b.record(
            0,
            evidence(Interval::new(0.1, 0.3), "is_fresh_batch", alpha),
        );
        b.record(
            0,
            evidence(Interval::new(0.1, 0.3), "is_fresh_batch", alpha),
        );
        assert!(b
            .finish(0, 0.2, false)
            .conflicts
            .iter()
            .any(|c| c == "alpha_budget_exceeded"));
    }
    #[test]
    fn certificate_source_bounds_are_sorted_and_hull_conflicts() {
        let (_env, b) = enabled_budget(1);
        b.record(
            0,
            evidence(Interval::new(0.1, 0.5), "native_mc", b.native_alpha()),
        );
        b.record(
            0,
            evidence(
                Interval::new(0.2, 0.4),
                "matched_own_points_strata",
                b.matched_alpha(),
            ),
        );
        let alpha = b.claim_is(0).unwrap();
        b.claim_is(0).unwrap();
        b.record(0, evidence(Interval::new(0.1, 0.2), "is_test", alpha));
        b.record(0, evidence(Interval::new(0.3, 0.4), "is_test", alpha));
        let c = b.finish(0, 0.25, false);
        assert_eq!(
            c.source_bounds
                .iter()
                .map(|s| s.method.as_str())
                .collect::<Vec<_>>(),
            ["is_test", "matched_own_points_strata", "native_mc"]
        );
        let is = &c.source_bounds[0];
        assert_eq!(
            (is.lower, is.upper, is.sample_uses, is.record_count),
            (0.1, 0.4, 4, 2)
        );
        assert!(c.conflicts.iter().any(|x| x == "source_conflict:is_test"));
    }
    #[test]
    fn qualification_handles_tiny_values() {
        assert!(qualification(Interval::new(1e-250, 1.00001e-250)));
        assert!(!qualification(Interval::new(0.0, 0.001)));
        assert!(!qualification(Interval::new(1e-210, 1e-190)));
    }
}
