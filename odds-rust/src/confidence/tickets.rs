//! Exact categorical probabilities induced by the finite `Rng::float` ticket set.
use super::Interval;

const DOMAIN: u64 = 1u64 << 63;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    Less,
    LessEqual,
}

fn ticket_float(k: u64) -> f64 {
    k as f64 / 9223372036854775808.0
}

fn accepted_tickets() -> u64 {
    // `DOMAIN` is the exclusive search sentinel and maps to 1.0.
    let (mut lo, mut hi) = (0u64, DOMAIN);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if ticket_float(mid) < 1.0 {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

fn boundary_count(threshold: f64, scale: f64, comparison: Comparison, accepted: u64) -> u64 {
    let matches = |k: u64| {
        let value = ticket_float(k) * scale;
        match comparison {
            Comparison::Less => value < threshold,
            Comparison::LessEqual => value <= threshold,
        }
    };
    let (mut lo, mut hi) = (0u64, accepted);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if matches(mid) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

fn enclosing_float(integer: u64) -> (f64, f64) {
    if integer == 0 {
        return (0.0, 0.0);
    }
    let rounded = integer as f64;
    (rounded.next_down().max(0.0), rounded.next_up())
}

fn ratio_interval(numerator: u64, denominator: u64) -> Interval {
    if numerator == 0 {
        return Interval::new(0.0, 0.0);
    }
    if numerator == denominator {
        return Interval::new(1.0, 1.0);
    }
    let (num_lo, num_hi) = enclosing_float(numerator);
    let (den_lo, den_hi) = enclosing_float(denominator);
    Interval::new(
        (num_lo / den_hi).next_down().max(0.0),
        (num_hi / den_lo).next_up().min(1.0),
    )
}

/// Convert ordered CDF boundaries to probability enclosures for the sampler's
/// finite floating-point ticket distribution. The final category is the exact
/// remainder after all supplied thresholds.
pub fn categorical_masses(
    thresholds: &[f64],
    scale: f64,
    comparison: Comparison,
) -> Option<Vec<Interval>> {
    if !scale.is_finite() || scale < 0.0 || thresholds.iter().any(|t| !t.is_finite()) {
        return None;
    }
    if thresholds.windows(2).any(|w| w[0] > w[1]) {
        return None;
    }
    let accepted = accepted_tickets();
    if accepted == 0 {
        return None;
    }
    let mut masses = Vec::with_capacity(thresholds.len() + 1);
    let mut previous = 0u64;
    for &threshold in thresholds {
        let cumulative = boundary_count(threshold, scale, comparison, accepted);
        if cumulative < previous {
            return None;
        }
        masses.push(ratio_interval(cumulative - previous, accepted));
        previous = cumulative;
    }
    masses.push(ratio_interval(accepted - previous, accepted));
    Some(masses)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_boundaries_match_strict_and_inclusive_comparisons() {
        let strict = categorical_masses(&[0.0], 1.0, Comparison::Less).unwrap();
        let inclusive = categorical_masses(&[0.0], 1.0, Comparison::LessEqual).unwrap();
        assert_eq!(strict[0], Interval::new(0.0, 0.0));
        assert!(inclusive[0].lower > 0.0);
        assert_eq!(strict[0].upper, 0.0);
    }

    #[test]
    fn equality_threshold_and_last_remainder_are_conserved() {
        let threshold = ticket_float(1u64 << 62);
        let accepted = accepted_tickets();
        let less = boundary_count(threshold, 1.0, Comparison::Less, accepted);
        let less_equal = boundary_count(threshold, 1.0, Comparison::LessEqual, accepted);
        assert!(less_equal > less);
        let masses = categorical_masses(&[threshold], 1.0, Comparison::LessEqual).unwrap();
        assert!(masses[0].lower <= 0.5 && masses[0].upper >= 0.5);
        let total = masses
            .iter()
            .copied()
            .fold(Interval::new(0.0, 0.0), |sum, mass| sum.add(mass));
        assert!(total.lower <= 1.0 && total.upper >= 1.0);
    }
}
