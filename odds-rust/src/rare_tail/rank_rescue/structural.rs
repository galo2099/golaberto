//! Repartition existing pilot quotas within complete root cohorts.

use super::{normalize_weights, quota_counts};

/// Reallocate each root's existing pilot budget using comparable leaf shares.
///
/// `root_groups` contains disjoint lists of leaf indices. Every leaf must occur
/// exactly once; invalid or incomplete mappings leave the caller's quotas
/// untouched by returning `None`.
pub(super) fn repartition_pilot(
    root_groups: &[(Vec<u8>, Vec<usize>)],
    shares: &[f64],
    original_counts: &[usize],
) -> Option<Vec<usize>> {
    if shares.is_empty() || shares.len() != original_counts.len() || root_groups.is_empty() {
        return None;
    }

    let mut seen = vec![false; shares.len()];
    let mut output = vec![0usize; original_counts.len()];
    for (_, indices) in root_groups {
        if indices.is_empty() {
            return None;
        }
        let mut total = 0usize;
        let mut local_shares = Vec::with_capacity(indices.len());
        for &index in indices {
            if index >= shares.len() || seen[index] {
                return None;
            }
            seen[index] = true;
            total = total.checked_add(original_counts[index])?;
            local_shares.push(shares[index]);
        }
        let weights = normalize_weights(&local_shares)?;
        let allocated = quota_counts(&weights, total, 5)?;
        for (&index, count) in indices.iter().zip(allocated) {
            output[index] = count;
        }
    }
    if seen.iter().any(|covered| !covered) {
        return None;
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::repartition_pilot;

    #[test]
    fn keeps_independent_root_totals() {
        let roots = vec![(vec![0], vec![0, 1]), (vec![1], vec![2, 3])];
        let result = repartition_pilot(&roots, &[0.9, 0.1, 0.2, 0.8], &[20, 30, 40, 60]).unwrap();
        assert_eq!(result[0] + result[1], 50);
        assert_eq!(result[2] + result[3], 100);
        assert!(result.iter().all(|count| *count >= 5));
    }

    #[test]
    fn tempers_partial_leaf_while_preserving_root_quota_and_floor() {
        let roots = vec![(vec![0], vec![0, 1, 2, 3])];
        let original = [12, 5, 14, 69];
        let result = repartition_pilot(&roots, &[0.295, 0.295, 0.295, 0.115], &original).unwrap();
        assert_eq!(result.iter().sum::<usize>(), 100);
        assert!(result.iter().all(|count| *count >= 5));
        assert!(
            result[3] <= 20,
            "partial leaf retained {} pilots",
            result[3]
        );
        assert!(result[0] + result[1] + result[2] >= 80);
        assert!(result[0] > original[0]);
        assert!(result[1] > original[1]);
        assert!(result[2] > original[2]);
    }

    #[test]
    fn incomplete_or_invalid_mapping_falls_back() {
        assert_eq!(
            repartition_pilot(&[(vec![0], vec![0])], &[1.0, 1.0], &[5, 5]),
            None
        );
        assert_eq!(
            repartition_pilot(&[(vec![0], vec![0, 0])], &[1.0], &[10]),
            None
        );
        assert_eq!(
            repartition_pilot(&[(vec![0], vec![0, 2])], &[1.0, 1.0], &[5, 5]),
            None
        );
        assert_eq!(
            repartition_pilot(&[(vec![0], vec![0, 1])], &[1.0, f64::NAN], &[5, 5]),
            None
        );
    }

    #[test]
    fn underfloor_root_falls_back() {
        assert_eq!(
            repartition_pilot(&[(vec![0], vec![0, 1])], &[0.5, 0.5], &[4, 5]),
            None
        );
    }
}
