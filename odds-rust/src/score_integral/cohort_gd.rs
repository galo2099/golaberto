//! Positive-mass order statistics for an arbitrary tied cohort's goal
//! differences. This module is deliberately independent of the score sampler;
//! it provides the exact coefficient and first-equal weights used by the
//! conditional cohort projection.

use super::EndpointIndex;
use std::collections::BTreeMap;

/// One team's bounded private-fixture projection. The kernel sequence retains
/// fixture order so a caller can use the same sequence to backtrack original
/// scorelines after conditioning on a terminal GD.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GdProjection {
    pub mass_by_gd: BTreeMap<i32, f64>,
    pub kernels: Vec<Vec<(i32, f64)>>,
}

impl GdProjection {
    pub(crate) fn new() -> Self {
        Self {
            mass_by_gd: BTreeMap::from([(0, 1.0)]),
            kernels: Vec::new(),
        }
    }

    /// Add one independent fixture's conditional GD contribution. Repeated
    /// contributions are combined with positive summation.
    pub(crate) fn push_kernel(&mut self, kernel: Vec<(i32, f64)>) {
        let mut grouped = BTreeMap::<i32, f64>::new();
        for (gd, mass) in &kernel {
            if *mass > 0.0 {
                *grouped.entry(*gd).or_default() += *mass;
            }
        }
        let mut next = BTreeMap::<i32, f64>::new();
        for (&prefix_gd, &prefix_mass) in &self.mass_by_gd {
            for (&fixture_gd, &fixture_mass) in &grouped {
                *next.entry(prefix_gd + fixture_gd).or_default() += prefix_mass * fixture_mass;
            }
        }
        self.mass_by_gd = next;
        self.kernels.push(kernel);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RankMass {
    pub strict: f64,
    pub residual: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct IndexedRankProjection {
    pub strict: f64,
    pub residual: f64,
    pub residual_choices: Vec<(i32, usize, f64)>,
    pub query_work: usize,
}

/// Combine strict-rank and first-equal residual weights from cached 1D
/// endpoints. Rival entries contain `(sampled_base_gd, private_endpoint_index)`
/// in stable cohort order excluding the target. This shares interval queries
/// between strict and residual calculations and avoids rebuilding marginals.
pub(crate) fn project_indexed(
    target_layer: &BTreeMap<(i32, i32), f64>,
    target_base_gd: i32,
    rivals: &[(i32, &EndpointIndex)],
    strictly_greater: usize,
) -> IndexedRankProjection {
    let mut projection = IndexedRankProjection::default();
    for (&(target_delta_gd, _), &target_mass) in target_layer {
        if target_mass <= 0.0 {
            continue;
        }
        let target_total_gd = target_base_gd + target_delta_gd;
        let mut classes = Vec::with_capacity(rivals.len());
        for &(rival_base_gd, index) in rivals {
            let threshold = target_total_gd - rival_base_gd;
            let (class, work) = classify_indexed_with_work(index, threshold);
            projection.query_work = projection.query_work.saturating_add(work);
            classes.push(class);
        }
        let (strict_mass, coefficient_work) =
            strict_rank_mass_with_work(&classes, strictly_greater);
        projection.query_work = projection
            .query_work
            .saturating_add(coefficient_work)
            .saturating_add(classes.len().saturating_mul(3));
        projection.strict += target_mass * strict_mass;
        let mut prior_unequal = 1.0;
        for (rival, class) in classes.iter().enumerate() {
            let weight = target_mass * prior_unequal * class.equal;
            projection.query_work = projection.query_work.saturating_add(3);
            if weight > 0.0 {
                projection.residual += weight;
                projection
                    .residual_choices
                    .push((target_total_gd, rival, weight));
            }
            prior_unequal *= class.lower + class.greater;
        }
    }
    projection
}

/// Integrate GD-only strict rank and the union of configurations where at
/// least one cohort rival equals target GD. `rivals` must be in stable model
/// team order, and `strictly_greater` is the target's rank inside the tied
/// points/wins cohort.
pub(crate) fn rank_mass(
    target: &BTreeMap<i32, f64>,
    rivals: &[BTreeMap<i32, f64>],
    strictly_greater: usize,
) -> RankMass {
    rank_mass_with_work(target, rivals, strictly_greater).0
}

pub(crate) fn rank_mass_with_work(
    target: &BTreeMap<i32, f64>,
    rivals: &[BTreeMap<i32, f64>],
    strictly_greater: usize,
) -> (RankMass, usize) {
    let rival_indexes: Option<Vec<_>> = rivals
        .iter()
        .map(|distribution| {
            let layer = distribution
                .iter()
                .map(|(&gd, &mass)| ((gd, 0), mass))
                .collect::<BTreeMap<_, _>>();
            EndpointIndex::build(&layer)
        })
        .collect();
    let Some(rival_indexes) = rival_indexes else {
        return (RankMass::default(), 0);
    };
    let mut work: usize = rival_indexes
        .iter()
        .map(|(_, build_work)| *build_work)
        .sum();
    let mut out = RankMass::default();
    for (&target_gd, &target_mass) in target {
        if target_mass <= 0.0 {
            continue;
        }
        let mut classify_work = 0usize;
        let classes: Vec<_> = rival_indexes
            .iter()
            .map(|(index, _)| {
                let (class, measured) = classify_indexed_with_work(index, target_gd);
                classify_work = classify_work.saturating_add(measured);
                class
            })
            .collect();
        let (strict, coefficient_work) = strict_rank_mass_with_work(&classes, strictly_greater);
        out.strict += target_mass * strict;
        let equal_union = first_equal_masses(&classes).into_iter().sum::<f64>();
        out.residual += target_mass * equal_union;
        work = work
            .saturating_add(classify_work)
            .saturating_add(coefficient_work)
            .saturating_add(classes.len().saturating_mul(3));
    }
    (out, work)
}

pub(crate) fn residual_choices(
    target: &BTreeMap<i32, f64>,
    rivals: &[BTreeMap<i32, f64>],
) -> (f64, Vec<(i32, usize, f64)>) {
    let (total, choices, _) = residual_choices_with_work(target, rivals);
    (total, choices)
}

pub(crate) fn residual_choices_with_work(
    target: &BTreeMap<i32, f64>,
    rivals: &[BTreeMap<i32, f64>],
) -> (f64, Vec<(i32, usize, f64)>, usize) {
    let rival_indexes: Option<Vec<_>> = rivals
        .iter()
        .map(|distribution| {
            let layer = distribution
                .iter()
                .map(|(&gd, &mass)| ((gd, 0), mass))
                .collect::<BTreeMap<_, _>>();
            EndpointIndex::build(&layer)
        })
        .collect();
    let Some(rival_indexes) = rival_indexes else {
        return (0.0, Vec::new(), 0);
    };
    let mut work: usize = rival_indexes
        .iter()
        .map(|(_, build_work)| *build_work)
        .sum();
    let mut total = 0.0;
    let mut choices = Vec::new();
    for (&target_gd, &target_mass) in target {
        if target_mass <= 0.0 {
            continue;
        }
        let mut classify_work = 0usize;
        let classes: Vec<_> = rival_indexes
            .iter()
            .map(|(index, _)| {
                let (class, measured) = classify_indexed_with_work(index, target_gd);
                classify_work = classify_work.saturating_add(measured);
                class
            })
            .collect();
        work = work
            .saturating_add(classify_work)
            .saturating_add(classes.len().saturating_mul(3));
        for (rival, first_equal_mass) in first_equal_masses(&classes).into_iter().enumerate() {
            let mass = target_mass * first_equal_mass;
            work = work.saturating_add(2);
            if mass > 0.0 {
                total += mass;
                choices.push((target_gd, rival, mass));
            }
        }
    }
    (total, choices, work)
}

/// Rival mass relative to one target goal difference.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CompareMass {
    pub lower: f64,
    pub equal: f64,
    pub greater: f64,
}

/// Return the coefficient for exactly `strictly_greater` rivals having a
/// greater GD. All arithmetic is a positive sum; no CDF subtraction is used.
pub(crate) fn strict_rank_mass(rivals: &[CompareMass], strictly_greater: usize) -> f64 {
    strict_rank_mass_with_work(rivals, strictly_greater).0
}

pub(crate) fn strict_rank_mass_with_work(
    rivals: &[CompareMass],
    strictly_greater: usize,
) -> (f64, usize) {
    if strictly_greater > rivals.len() {
        return (0.0, 0);
    }
    // Count the rarer side of the rank. The recurrence stays positive and
    // uses O(min(r, m-r)) memory for targets near either end of the table.
    let count_lower = rivals.len() - strictly_greater;
    let reverse = count_lower < strictly_greater;
    let degree = count_lower.min(strictly_greater);
    let mut coefficients = vec![0.0; degree + 1];
    coefficients[0] = 1.0;
    let mut seen = 0usize;
    let mut work = 0usize;
    for rival in rivals {
        let upper = degree.min(seen + 1);
        for rank in (0..=upper).rev() {
            // A strict GD ordering excludes every equality. Reverse counting
            // swaps which strict side contributes the coefficient variable.
            let (without_mass, with_mass) = if reverse {
                (rival.greater, rival.lower)
            } else {
                (rival.lower, rival.greater)
            };
            let without = coefficients[rank] * without_mass;
            let with = if rank > 0 {
                coefficients[rank - 1] * with_mass
            } else {
                0.0
            };
            coefficients[rank] = without + with;
            work = work.saturating_add(3);
        }
        seen += 1;
    }
    (coefficients[degree], work)
}

/// Classify with positive segment-tree interval sums. This avoids scanning a
/// rival's entire GD support for every target endpoint.
pub(crate) fn classify_indexed(index: &EndpointIndex, target_gd: i32) -> CompareMass {
    classify_indexed_with_work(index, target_gd).0
}

fn classify_indexed_with_work(index: &EndpointIndex, target_gd: i32) -> (CompareMass, usize) {
    let target = i64::from(target_gd);
    let (lower_end, lower_bound_work) = index.lower_bound((target, i64::MIN));
    let (equal_end, upper_bound_work) = index.upper_bound((target, i64::MAX));
    let (lower, lower_range_work) = index.range_mass(0, lower_end);
    let (equal, equal_range_work) = index.range_mass(lower_end, equal_end);
    let (greater, greater_range_work) = index.range_mass(equal_end, usize::MAX);
    (
        CompareMass {
            lower,
            equal,
            greater,
        },
        lower_bound_work
            .saturating_add(upper_bound_work)
            .saturating_add(lower_range_work)
            .saturating_add(equal_range_work)
            .saturating_add(greater_range_work),
    )
}

/// Probability that the first GD-equal rival in stable rival order is at
/// index `j`. Prior rivals must be unequal; later rivals are unrestricted.
/// Every term is a product/sum of nonnegative masses. Later rival marginals
/// are unrestricted and therefore must be normalized.
pub(crate) fn first_equal_masses(rivals: &[CompareMass]) -> Vec<f64> {
    let mut prior_unequal = 1.0;
    let mut out = Vec::with_capacity(rivals.len());
    for rival in rivals {
        out.push(prior_unequal * rival.equal);
        prior_unequal *= rival.lower + rival.greater;
    }
    out
}

/// Classify a discrete rival GD distribution against a target GD. The input
/// The distribution must be normalized before use in `first_equal_masses`.
pub(crate) fn classify<I>(distribution: I, target_gd: i32) -> CompareMass
where
    I: IntoIterator<Item = (i32, f64)>,
{
    let mut out = CompareMass::default();
    for (gd, mass) in distribution {
        if mass <= 0.0 {
            continue;
        }
        if gd < target_gd {
            out.lower += mass;
        } else if gd == target_gd {
            out.equal += mass;
        } else {
            out.greater += mass;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-14, "{a} != {b}");
    }

    #[test]
    fn strict_coefficients_match_exhaustive_outcomes_for_cohort_sizes_two_to_four() {
        let cases = [
            vec![CompareMass {
                lower: 0.2,
                equal: 0.3,
                greater: 0.5,
            }],
            vec![
                CompareMass {
                    lower: 0.2,
                    equal: 0.3,
                    greater: 0.5,
                },
                CompareMass {
                    lower: 0.4,
                    equal: 0.1,
                    greater: 0.5,
                },
            ],
            vec![
                CompareMass {
                    lower: 0.2,
                    equal: 0.3,
                    greater: 0.5,
                },
                CompareMass {
                    lower: 0.4,
                    equal: 0.1,
                    greater: 0.5,
                },
                CompareMass {
                    lower: 0.1,
                    equal: 0.6,
                    greater: 0.3,
                },
            ],
        ];
        for rivals in cases {
            for wanted in 0..=rivals.len() {
                let mut exact = 0.0;
                let mut states = vec![(0usize, false, 1.0)];
                for rival in &rivals {
                    let mut next = Vec::new();
                    for (count, has_equal, mass) in states {
                        next.push((count, has_equal, mass * rival.lower));
                        next.push((count, true, mass * rival.equal));
                        next.push((count + 1, has_equal, mass * rival.greater));
                    }
                    states = next;
                }
                for (count, has_equal, mass) in states {
                    if count == wanted && !has_equal {
                        exact += mass;
                    }
                }
                close(strict_rank_mass(&rivals, wanted), exact);
            }
        }
    }

    #[test]
    fn first_equal_weights_are_disjoint_and_positive_at_tiny_tails() {
        let tiny = 1e-280;
        let rivals = [
            CompareMass {
                lower: 0.5,
                equal: tiny,
                greater: 0.5,
            },
            CompareMass {
                lower: 0.25,
                equal: 2.0 * tiny,
                greater: 0.75,
            },
        ];
        let weights = first_equal_masses(&rivals);
        assert!(weights[0] > 0.0);
        assert!(weights[1] > 0.0);
        assert!((weights[0] / tiny - 1.0).abs() < 1e-14);
        assert!((weights[1] / (2.0 * tiny) - 1.0).abs() < 1e-14);
    }

    #[test]
    fn classify_uses_positive_interval_masses_and_ignores_zero_support() {
        let got = classify([(0, 0.2), (1, 0.3), (2, 0.5), (1, 0.0)], 1);
        close(got.lower, 0.2);
        close(got.equal, 0.3);
        close(got.greater, 0.5);
    }

    #[test]
    fn projection_convolves_fixture_gd_masses_and_keeps_backtracking_kernels() {
        let mut projection = GdProjection::new();
        projection.push_kernel(vec![(-1, 0.25), (1, 0.75)]);
        projection.push_kernel(vec![(0, 0.5), (2, 0.5)]);
        close(projection.mass_by_gd[&-1], 0.125);
        close(projection.mass_by_gd[&1], 0.5);
        close(projection.mass_by_gd[&3], 0.375);
        assert_eq!(projection.kernels.len(), 2);
    }

    #[test]
    fn rank_mass_matches_brute_force_with_shared_points_and_gd_ties() {
        let target = BTreeMap::from([(-1, 0.25), (0, 0.75)]);
        let rivals = vec![
            BTreeMap::from([(-1, 0.25), (0, 0.75)]),
            BTreeMap::from([(0, 0.5), (1, 0.5)]),
        ];
        let actual = rank_mass(&target, &rivals, 1);
        let mut strict = 0.0;
        let mut residual = 0.0;
        for (&td, &tm) in &target {
            for (&a, &am) in &rivals[0] {
                for (&b, &bm) in &rivals[1] {
                    let mass = tm * am * bm;
                    let greater = usize::from(a > td) + usize::from(b > td);
                    if greater == 1 && a != td && b != td {
                        strict += mass;
                    }
                    if a == td || b == td {
                        residual += mass;
                    }
                }
            }
        }
        close(actual.strict, strict);
        close(actual.residual, residual);
    }
}
