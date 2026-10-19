//! Bounded exact score-difference kernels for conditional score integration.
use std::collections::BTreeMap;

pub const MAX_COHORT_GD_PATHS: usize = 256;
/// Per-team terminal support cap for the experimental arbitrary-cohort GD
/// projection. Prefix DP stops before exceeding this layer size.
pub(crate) const MAX_COHORT_GD_ENDPOINTS: usize = 2_000;
pub(crate) const MAX_LAYER_CELLS: usize = 50_000;
pub const MAX_TOTAL_CELLS: usize = 300_000;
pub const MAX_WORK: usize = 8_000_000;
pub(crate) const MAX_KERNELS: usize = 256;
pub(crate) const MAX_KERNEL_SUPPORT: usize = 2_000;
pub(crate) mod cohort_gd;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct BuildCaps {
    pub work: usize,
    pub layer_cells: usize,
    pub total_cells: usize,
    pub convolution: usize,
}
pub const DEFAULT_BUILD_CAPS: BuildCaps = BuildCaps {
    work: MAX_WORK,
    layer_cells: MAX_LAYER_CELLS,
    total_cells: MAX_TOTAL_CELLS,
    convolution: 2_000_000,
};

/// Conservative query plus residual/backtrack work for the endpoint supports
/// that were actually admitted by prefix preparation. The fixture and
/// bookkeeping terms cover native draws, score-path construction, residual
/// reconstruction, and the O(fixtures + cohort) admission pass.
pub(crate) fn cohort_gd_prepared_query_work_upper_bound(
    target_support: usize,
    rival_supports: &[usize],
    fixture_count: usize,
    integrated_kernel_costs: &[usize],
    bookkeeping_work: usize,
) -> usize {
    let target_support = target_support.max(1);
    let mut interval_work = 0usize;
    let mut residual_scans = 0usize;
    for &support in rival_supports {
        let support = support.max(1);
        let endpoint_log = support.max(2).next_power_of_two().ilog2() as usize + 1;
        interval_work = interval_work.saturating_add(
            target_support.saturating_mul(endpoint_log.saturating_mul(14).saturating_add(24)),
        );
        residual_scans = residual_scans.saturating_add(support.saturating_mul(2));
    }
    let rival_count = rival_supports.len();
    let coefficient_work = target_support
        .saturating_mul(rival_count)
        .saturating_mul(rival_count / 2 + 1)
        .saturating_mul(3);
    let residual_choices = target_support.saturating_mul(rival_count);
    let backtrack = integrated_kernel_costs
        .iter()
        .copied()
        .fold(0usize, usize::saturating_add);
    interval_work
        .saturating_add(coefficient_work)
        .saturating_add(residual_choices)
        .saturating_add(residual_scans)
        .saturating_add(backtrack)
        // One fixture-sized score-path pass, the possible residual rebuild,
        // and the Operations.units() cost (8 units) for each native score draw.
        .saturating_add(fixture_count.saturating_mul(10))
        .saturating_add(3) // the one-cell identity EndpointIndex
        .saturating_add(bookkeeping_work)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScorePair {
    pub gd: i32,
    pub gf: i32,
    pub mass: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct JointKernel {
    pub fixture: usize,
    pub values: Vec<ScorePair>,
}

#[derive(Clone, Debug)]
pub(crate) struct PrefixDp {
    /// Prefix layers include the empty prefix at index zero.
    pub layers: Vec<BTreeMap<(i32, i32), f64>>,
    pub work: usize,
    pub integrated: usize,
}

/// Immutable lexicographic endpoint masses with a positive-sum range index.
/// The tree stores sums only, so queries never subtract nearly equal CDFs.
#[derive(Clone, Debug)]
pub(crate) struct EndpointIndex {
    keys: Vec<(i32, i32)>,
    tree: Vec<f64>,
    base: usize,
}

impl EndpointIndex {
    pub(crate) fn build(layer: &BTreeMap<(i32, i32), f64>) -> Option<(Self, usize)> {
        let base = layer.len().checked_next_power_of_two()?.max(1);
        let tree_len = base.checked_mul(2)?;
        let mut keys = Vec::with_capacity(layer.len());
        let mut tree = vec![0.0; tree_len];
        for (slot, (&key, &mass)) in layer.iter().enumerate() {
            keys.push(key);
            tree[base + slot] = mass;
        }
        for node in (1..base).rev() {
            tree[node] = tree[node * 2] + tree[node * 2 + 1];
        }
        let work = keys.len().saturating_add(tree_len);
        Some((Self { keys, tree, base }, work))
    }

    pub(crate) fn retained_cells(&self) -> usize {
        self.keys.len().saturating_add(self.tree.len())
    }

    pub(crate) fn len(&self) -> usize {
        self.keys.len()
    }

    fn bound(&self, key: (i64, i64), upper: bool) -> (usize, usize) {
        let (mut low, mut high, mut work) = (0, self.keys.len(), 0);
        while low < high {
            let mid = low + (high - low) / 2;
            work += 1;
            let candidate = (i64::from(self.keys[mid].0), i64::from(self.keys[mid].1));
            if candidate < key || (upper && candidate == key) {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        (low, work)
    }

    pub(crate) fn lower_bound(&self, key: (i64, i64)) -> (usize, usize) {
        self.bound(key, false)
    }

    pub(crate) fn upper_bound(&self, key: (i64, i64)) -> (usize, usize) {
        self.bound(key, true)
    }

    pub(crate) fn exact_mass(&self, key: (i32, i32)) -> (f64, usize) {
        let (index, work) = self.lower_bound((i64::from(key.0), i64::from(key.1)));
        let mass = self
            .keys
            .get(index)
            .filter(|&&candidate| candidate == key)
            .map_or(0.0, |_| self.tree[self.base + index]);
        (mass, work.saturating_add(1))
    }

    pub(crate) fn range_mass(&self, mut start: usize, mut end: usize) -> (f64, usize) {
        start = start.min(self.keys.len());
        end = end.min(self.keys.len());
        if start >= end {
            return (0.0, 0);
        }
        let (mut left, mut right) = (start + self.base, end + self.base);
        let (mut left_sum, mut right_sum, mut work) = (0.0, 0.0, 0);
        while left < right {
            if left & 1 == 1 {
                left_sum += self.tree[left];
                left += 1;
                work += 1;
            }
            if right & 1 == 1 {
                right -= 1;
                right_sum += self.tree[right];
                work += 1;
            }
            left >>= 1;
            right >>= 1;
            work += 2;
        }
        (left_sum + right_sum, work)
    }
}

impl PrefixDp {
    pub(crate) fn build(kernels: &[JointKernel], work_limit: usize) -> Option<Self> {
        Self::build_bounded(kernels, work_limit, MAX_TOTAL_CELLS)
    }

    pub(crate) fn build_bounded(
        kernels: &[JointKernel],
        work_limit: usize,
        total_cell_limit: usize,
    ) -> Option<Self> {
        Self::build_with_caps(
            kernels,
            work_limit.min(MAX_WORK),
            total_cell_limit.min(MAX_TOTAL_CELLS),
            MAX_LAYER_CELLS,
            DEFAULT_BUILD_CAPS.convolution,
        )
    }

    pub(crate) fn build_with_caps(
        kernels: &[JointKernel],
        work_limit: usize,
        total_cell_limit: usize,
        layer_cell_limit: usize,
        convolution_limit: usize,
    ) -> Option<Self> {
        if kernels.len() > MAX_KERNELS {
            return None;
        }
        let mut layers = vec![BTreeMap::from([((0i32, 0i32), 1.0)])];
        let mut work = 0usize;
        let mut total_cells = 1usize;
        'kernels: for kernel in kernels {
            let active = layers.last()?;
            let estimated = active.len().checked_mul(kernel.values.len())?;
            if estimated > convolution_limit || work.checked_add(estimated)? > work_limit {
                break;
            }
            let mut next = BTreeMap::<(i32, i32), f64>::new();
            let mut layer_overflow = false;
            for (&(gd, gf), &mass) in active {
                for value in &kernel.values {
                    let key = (gd.checked_add(value.gd)?, gf.checked_add(value.gf)?);
                    if next.len() >= layer_cell_limit && !next.contains_key(&key) {
                        work += 1;
                        layer_overflow = true;
                        break;
                    }
                    *next.entry(key).or_default() += mass * value.mass;
                    work += 1;
                }
                if layer_overflow {
                    break;
                }
            }
            if layer_overflow {
                break 'kernels;
            }
            total_cells = total_cells.checked_add(next.len())?;
            if total_cells > total_cell_limit {
                break;
            }
            layers.push(next);
        }
        Some(Self {
            integrated: layers.len() - 1,
            layers,
            work,
        })
    }

    pub(crate) fn mass(&self, gd: i32, gf: i32) -> f64 {
        self.layers
            .last()
            .and_then(|layer| layer.get(&(gd, gf)))
            .copied()
            .unwrap_or(0.0)
    }

    /// Sample an ordered vector of score differences conditional on its exact
    /// joint endpoint. Each backward choice is weighted by prefix mass times
    /// the current fixture's exact kernel mass.
    pub(crate) fn sample_path(
        &self,
        kernels: &[JointKernel],
        endpoint: (i32, i32),
        rng: &mut crate::rng::Rng,
    ) -> Option<Vec<(i32, i32)>> {
        if kernels.len() + 1 != self.layers.len() || self.mass(endpoint.0, endpoint.1) <= 0.0 {
            return None;
        }
        let mut residual = endpoint;
        let mut path = vec![(0, 0); kernels.len()];
        for index in (0..kernels.len()).rev() {
            let previous = &self.layers[index];
            let kernel = &kernels[index];
            let mut choices = Vec::new();
            let mut total = 0.0;
            for value in &kernel.values {
                let key = (
                    residual.0.checked_sub(value.gd)?,
                    residual.1.checked_sub(value.gf)?,
                );
                let mass = previous.get(&key).copied().unwrap_or(0.0) * value.mass;
                if mass > 0.0 {
                    total += mass;
                    choices.push((key, (value.gd, value.gf), mass));
                }
            }
            if total <= 0.0 {
                return None;
            }
            let mut draw = rng.float() * total;
            let chosen = choices
                .iter()
                .find(|(_, _, mass)| {
                    draw -= *mass;
                    draw < 0.0
                })
                .or_else(|| choices.last())?;
            residual = chosen.0;
            path[index] = chosen.1;
        }
        (residual == (0, 0)).then_some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joint_dp_preserves_exact_joint_mass_and_conditional_paths() {
        let kernels = vec![
            JointKernel {
                fixture: 0,
                values: vec![
                    ScorePair {
                        gd: 1,
                        gf: 1,
                        mass: 0.25,
                    },
                    ScorePair {
                        gd: 0,
                        gf: 2,
                        mass: 0.75,
                    },
                ],
            },
            JointKernel {
                fixture: 1,
                values: vec![
                    ScorePair {
                        gd: -1,
                        gf: 0,
                        mass: 0.4,
                    },
                    ScorePair {
                        gd: 0,
                        gf: 1,
                        mass: 0.6,
                    },
                ],
            },
        ];
        let dp = PrefixDp::build(&kernels, 100).unwrap();
        assert!((dp.mass(0, 1) - 0.1).abs() < 1e-15);
        assert!((dp.mass(0, 3) - 0.45).abs() < 1e-15);
        let mut rng = crate::rng::Rng::new(8);
        for _ in 0..100 {
            let path = dp.sample_path(&kernels, (0, 1), &mut rng).unwrap();
            assert_eq!(path.iter().map(|v| v.0).sum::<i32>(), 0);
            assert_eq!(path.iter().map(|v| v.1).sum::<i32>(), 1);
        }
    }

    #[test]
    fn bounded_dp_declines_without_truncating_support() {
        let kernel = JointKernel {
            fixture: 0,
            values: (0..10)
                .map(|i| ScorePair {
                    gd: i,
                    gf: i,
                    mass: 0.1,
                })
                .collect(),
        };
        let dp = PrefixDp::build(&vec![kernel], 5).unwrap();
        assert_eq!(dp.integrated, 0);
        assert_eq!(dp.work, 0);
    }
}

/// Eligibility guard for the exact arbitrary-cohort goal-difference projection.
/// Unsupported standings keys stay on the native score path.
pub fn supported(model: &crate::model::Model) -> bool {
    use crate::model::Key;
    if model.request.phase.bonus_points != 0
        || model
            .fixtures
            .iter()
            .any(|fixture| fixture.home == fixture.away)
    {
        return false;
    }
    let Some(gd_at) = model.keys.iter().position(|key| *key == Key::Gd) else {
        return false;
    };
    if model.keys.get(gd_at + 1) != Some(&Key::Gf) {
        return false;
    }
    let prefix = &model.keys[..gd_at];
    if prefix != [Key::Pt] && prefix != [Key::Pt, Key::W] {
        return false;
    }
    model.keys[gd_at + 2..].iter().all(|key| {
        matches!(
            key,
            Key::Gp | Key::Aet | Key::Bias | Key::Name | Key::Random | Key::Head
        )
    })
}
