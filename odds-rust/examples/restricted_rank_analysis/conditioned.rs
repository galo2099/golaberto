use super::{SharedEdge, WeightedLeaf};
use golaberto_odds::rng::Rng;

#[derive(Clone)]
pub(super) struct UnaryChoice {
    pub fixture_index: usize,
    pub mask: u8,
    pub probability: [f64; 3],
    pub gain: [i32; 3],
}

struct UnaryTeam {
    choices: Vec<UnaryChoice>,
    suffix_cdf: Vec<Vec<f64>>,
}

pub(super) struct Sampler {
    leaves: Vec<WeightedLeaf>,
    tree: Vec<f64>,
    tree_base: usize,
    shared_fixture_indices: Vec<usize>,
    edges: Vec<SharedEdge>,
    unary: Vec<UnaryTeam>,
    irrelevant: Vec<(usize, u8, [f64; 3])>,
    target_fixed: Vec<(usize, u8)>,
    fixture_count: usize,
}

impl Sampler {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        leaves: Vec<WeightedLeaf>,
        shared_fixture_indices: Vec<usize>,
        edges: &[SharedEdge],
        unary_choices: Vec<Vec<UnaryChoice>>,
        max_remaining: &[i32],
        irrelevant: Vec<(usize, u8, [f64; 3])>,
        target_fixed: Vec<(usize, u8)>,
        fixture_count: usize,
    ) -> Result<Self, String> {
        if leaves.is_empty() || edges.len() != shared_fixture_indices.len() {
            return Err("empty or inconsistent conditioned support".into());
        }
        let tree_base = leaves.len().next_power_of_two();
        let mut tree = vec![0.0; tree_base * 2];
        for (i, leaf) in leaves.iter().enumerate() {
            tree[tree_base + i] = leaf.weight;
        }
        for i in (1..tree_base).rev() {
            tree[i] = tree[i * 2] + tree[i * 2 + 1];
        }
        if !tree[1].is_finite() || tree[1] <= 0.0 {
            return Err("conditioned sampler has no finite positive mass".into());
        }
        if unary_choices.len() != max_remaining.len() {
            return Err("unary team count does not match cap state".into());
        }

        let unary = unary_choices
            .into_iter()
            .zip(max_remaining)
            .map(|(choices, &max_gain)| {
                let limit = max_gain.max(0) as usize;
                let mut suffix_cdf = vec![vec![0.0; limit + 1]; choices.len() + 1];
                let mut suffix_pmf = vec![vec![0.0; limit + 1]; choices.len() + 1];
                suffix_pmf[choices.len()][0] = 1.0;
                for i in (0..choices.len()).rev() {
                    for suffix_gain in 0..=limit {
                        for outcome in 0..3 {
                            if choices[i].mask & (1 << outcome) == 0 {
                                continue;
                            }
                            let total_gain = suffix_gain + choices[i].gain[outcome] as usize;
                            if total_gain <= limit {
                                suffix_pmf[i][total_gain] += choices[i].probability[outcome]
                                    * suffix_pmf[i + 1][suffix_gain];
                            }
                        }
                    }
                    let mut running = 0.0;
                    for cap in 0..=limit {
                        running += suffix_pmf[i][cap];
                        suffix_cdf[i][cap] = running;
                    }
                }
                suffix_cdf[choices.len()] = (0..=limit).map(|_| 1.0).collect();
                UnaryTeam {
                    choices,
                    suffix_cdf,
                }
            })
            .collect();
        Ok(Self {
            leaves,
            tree,
            tree_base,
            shared_fixture_indices,
            edges: edges.to_vec(),
            unary,
            irrelevant,
            target_fixed,
            fixture_count,
        })
    }

    pub(super) fn total_mass(&self) -> f64 {
        self.tree[1]
    }

    pub(super) fn sample(&self, rng: &mut Rng) -> Result<Vec<u8>, String> {
        let mut outcomes = vec![u8::MAX; self.fixture_count];
        let mut threshold = rng.float() * self.tree[1];
        let mut node = 1;
        while node < self.tree_base {
            let left = self.tree[node * 2];
            if threshold < left {
                node *= 2;
            } else {
                threshold -= left;
                node = node * 2 + 1;
            }
        }
        let leaf_index = node - self.tree_base;
        let leaf = self
            .leaves
            .get(leaf_index)
            .ok_or_else(|| "sum tree selected padding leaf".to_string())?;
        for (i, &fixture_index) in self.shared_fixture_indices.iter().enumerate() {
            let outcome = ((leaf.packed_outcomes >> (2 * i)) & 3) as usize;
            let edge = &self.edges[i];
            if edge.mask & (1 << outcome) == 0 || edge.probability[outcome] <= 0.0 {
                return Err("leaf contains a forbidden shared-edge outcome".into());
            }
            outcomes[fixture_index] = outcome as u8;
        }
        for &(fixture_index, outcome) in &self.target_fixed {
            outcomes[fixture_index] = outcome;
        }
        for (team, data) in self.unary.iter().enumerate() {
            let mut remaining = leaf.remaining[team];
            for (i, choice) in data.choices.iter().enumerate() {
                let mut masses = [0.0; 3];
                let mut total = 0.0;
                for outcome in 0..3 {
                    let gain = choice.gain[outcome];
                    if choice.mask & (1 << outcome) == 0 || gain > remaining {
                        continue;
                    }
                    let rem = (remaining - gain) as usize;
                    let suffix = data.suffix_cdf[i + 1].get(rem).copied().unwrap_or(0.0);
                    masses[outcome] = choice.probability[outcome] * suffix;
                    total += masses[outcome];
                }
                if !total.is_finite() || total <= 0.0 {
                    return Err("unary conditioned suffix lost positive support".into());
                }
                let mut draw = rng.float() * total;
                let mut selected = None;
                for outcome in 0..3 {
                    if draw < masses[outcome] {
                        selected = Some(outcome);
                        break;
                    }
                    draw -= masses[outcome];
                }
                let outcome = selected
                    .or_else(|| (0..3).rev().find(|&o| masses[o] > 0.0))
                    .ok_or_else(|| "unary conditional draw had no outcome".to_string())?;
                outcomes[choice.fixture_index] = outcome as u8;
                remaining -= choice.gain[outcome];
            }
            if remaining < 0 {
                return Err("conditioned unary draw exceeded packed cap".into());
            }
        }
        for &(fixture_index, mask, probability) in &self.irrelevant {
            let total = (0..3)
                .filter(|&o| mask & (1 << o) != 0)
                .map(|o| probability[o])
                .sum::<f64>();
            if total <= 0.0 || !total.is_finite() {
                return Err("irrelevant fixture has no allowed WDL mass".into());
            }
            let mut draw = rng.float() * total;
            for outcome in 0..3 {
                if mask & (1 << outcome) == 0 {
                    continue;
                }
                if draw < probability[outcome] {
                    outcomes[fixture_index] = outcome as u8;
                    break;
                }
                draw -= probability[outcome];
            }
            if outcomes[fixture_index] == u8::MAX {
                outcomes[fixture_index] = (0..3)
                    .rev()
                    .find(|&o| mask & (1 << o) != 0 && probability[o] > 0.0)
                    .ok_or_else(|| "irrelevant fixture draw had no outcome".to_string())?
                    as u8;
            }
        }
        if outcomes.iter().any(|&o| o > 2) {
            return Err("conditioned sampler left an outcome unset".into());
        }
        Ok(outcomes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_leaves_and_suffix_conditioning_match_hand_computation() {
        let edge = SharedEdge {
            a: 0,
            b: 1,
            mask: 0b101,
            probability: [0.2, 0.0, 0.8],
            gain_a: [0, 0, 0],
            gain_b: [0, 0, 0],
        };
        let sampler = Sampler::new(
            vec![
                WeightedLeaf {
                    packed_outcomes: 0,
                    remaining: vec![28, 28],
                    weight: 0.1,
                },
                WeightedLeaf {
                    packed_outcomes: 2,
                    remaining: vec![28, 28],
                    weight: 0.4,
                },
            ],
            vec![0],
            std::slice::from_ref(&edge),
            vec![
                vec![UnaryChoice {
                    fixture_index: 1,
                    mask: 0b111,
                    probability: [0.2, 0.3, 0.5],
                    gain: [0, 28, 85],
                }],
                vec![],
            ],
            &[28, 28],
            vec![],
            vec![],
            2,
        )
        .unwrap();
        assert!((sampler.total_mass() - 0.5).abs() < 1e-15);
        let mut rng = Rng::new(808);
        let samples = 20_000usize;
        let mut shared_wins = 0;
        let mut unary_draws = 0;
        for _ in 0..samples {
            let outcomes = sampler.sample(&mut rng).unwrap();
            assert!(edge.mask & (1 << outcomes[0]) != 0);
            assert_ne!(outcomes[1], 2, "85-point win cannot fit 28-point residual");
            shared_wins += usize::from(outcomes[0] == 2);
            unary_draws += usize::from(outcomes[1] == 1);
        }
        let shared_rate = shared_wins as f64 / samples as f64;
        let unary_draw_rate = unary_draws as f64 / samples as f64;
        assert!((shared_rate - 0.8).abs() < 0.02);
        assert!((unary_draw_rate - 0.6).abs() < 0.02);
    }
}
