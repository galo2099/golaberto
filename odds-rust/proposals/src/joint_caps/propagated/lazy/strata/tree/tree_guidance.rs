//! Opt-in allocation guidance from the frozen static rank shortlist.
use super::Leaf;
#[cfg(any(test, feature = "test-support"))]
use crate::test_env as experiment_env;
#[cfg(not(any(test, feature = "test-support")))]
use std::env as experiment_env;
use crate::{
    model::{Key, Model},
    shortlist_guidance::{self, Candidate},
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};

pub(super) const FLAG: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_SHORTLIST_TREE";
const REPAIR_FLAG: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_HYBRID_REPAIR";
const NORMAL_CAP: usize = 8_000_000;
const LARGE_CAP: usize = 64_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Mass,
    Entropy,
}

#[derive(Hash, PartialEq, Eq)]
struct BoundsKey {
    root: Vec<u8>,
    lower: Vec<i32>,
    upper: Vec<i32>,
}

pub(super) struct State {
    mode: Option<Mode>,
    candidates: Vec<Candidate>,
    reason: Option<String>,
    cap: usize,
    pub(super) work: usize,
    cache: HashMap<u64, Vec<(BoundsKey, Vec<bool>)>>,
    cache_hits: usize,
    cache_misses: usize,
    frontier_passes: usize,
    applied_selections: usize,
    entropy_selections: usize,
    unmatched_weight: f64,
    no_compatible_candidates: usize,
    no_live_compatible_candidates: usize,
    stride: Option<i32>,
    pal: Option<usize>,
    mir: Option<usize>,
    target_shape: Option<Vec<(usize, usize)>>,
}

impl State {
    pub(super) fn new(model: &Model, active: bool, large: bool) -> Self {
        let cap = if large { LARGE_CAP } else { NORMAL_CAP };
        let requested = experiment_env::var(FLAG).unwrap_or_default();
        let mode = match requested.as_str() {
            "mass" => Some(Mode::Mass),
            "entropy" => Some(Mode::Entropy),
            "" => None,
            _ => None,
        };
        let mut state = Self {
            mode,
            candidates: Vec::new(),
            reason: None,
            cap,
            work: 0,
            cache: HashMap::new(),
            cache_hits: 0,
            cache_misses: 0,
            frontier_passes: 0,
            applied_selections: 0,
            entropy_selections: 0,
            unmatched_weight: 0.0,
            no_compatible_candidates: 0,
            no_live_compatible_candidates: 0,
            stride: None,
            pal: None,
            mir: None,
            target_shape: None,
        };
        if requested.is_empty() {
            return state;
        }
        if requested != "mass" && requested != "entropy" {
            state.mode = None;
            state.reason = Some("invalid_mode".into());
            return state;
        }
        if experiment_env::var(REPAIR_FLAG).as_deref() != Ok("1")
            || !active
            || model.keys.get(1) != Some(&Key::W)
        {
            state.mode = None;
            state.reason = Some("requires_repair_flag_and_scoped_hybrid_tree".into());
            return state;
        }
        let (guide, scan_work) = shortlist_guidance::load_tree(model);
        let mut left = vec![0i32; model.n];
        for fixture in &model.fixtures {
            left[fixture.home] += 1;
            left[fixture.away] += 1;
        }
        let stride = model
            .base
            .iter()
            .zip(left)
            .map(|(base, remaining)| base.wins + remaining)
            .max()
            .map(|value| value.saturating_add(1));
        state.stride = stride;
        state.pal = model.indices.get(&16).copied();
        state.mir = model.indices.get(&584).copied();
        state.work = scan_work
            .saturating_add(model.fixtures.len().saturating_mul(2))
            .saturating_add(model.n);
        if scan_work > cap {
            state.mode = None;
            state.reason = Some("work_budget_exhausted".into());
            return state;
        }
        if let Some(reason) = guide.fallback {
            state.mode = None;
            state.reason = Some(reason);
            return state;
        }
        state.candidates = guide.candidates;
        state
    }

    fn reserve(&mut self, fee: usize) -> bool {
        match self.work.checked_add(fee) {
            Some(next) if next <= self.cap => {
                self.work = next;
                true
            }
            _ => {
                self.mode = None;
                self.reason = Some("work_budget_exhausted".into());
                false
            }
        }
    }

    fn compatible_vector(
        &mut self,
        model: &Model,
        target_games: &[(usize, usize)],
        leaf: &Leaf,
    ) -> Option<Vec<bool>> {
        let key_fee = leaf
            .root
            .len()
            .saturating_add(2usize.saturating_mul(leaf.pattern.lower.len()))
            .saturating_add(self.candidates.len());
        if !self.reserve(key_fee) {
            return None;
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        leaf.root.hash(&mut hasher);
        leaf.pattern.lower.hash(&mut hasher);
        leaf.pattern.upper.hash(&mut hasher);
        let hash = hasher.finish();
        if let Some(value) = self
            .cache
            .get(&hash)
            .and_then(|bucket| {
                bucket.iter().find(|(key, _)| {
                    key.root == leaf.root
                        && key.lower == leaf.pattern.lower
                        && key.upper == leaf.pattern.upper
                })
            })
            .map(|(_, value)| value.clone())
        {
            self.cache_hits += 1;
            return Some(value);
        }
        let n = model.n;
        let bound_fee = self.candidates.len().saturating_mul(n).saturating_mul(6);
        if !self.reserve(bound_fee) {
            return None;
        }
        self.cache_misses += 1;
        if self.target_shape.is_none() {
            if !self.reserve(target_games.len()) {
                return None;
            }
            self.target_shape = Some(target_games.to_vec());
        }
        let pal = self.pal?;
        let mir = self.mir?;
        let stride = self.stride?;
        let shape = self.target_shape.as_ref()?;
        let all_loss = leaf.root.len() == shape.len()
            && leaf
                .root
                .iter()
                .zip(shape)
                .all(|(&outcome, &(home, away))| {
                    if home == pal {
                        outcome == 0
                    } else if away == pal {
                        outcome == 2
                    } else {
                        true
                    }
                });
        let packed_target = 57 * stride + 16;
        let result = self
            .candidates
            .iter()
            .map(|candidate| {
                if !all_loss {
                    return false;
                }
                let mut below = candidate
                    .below_ids
                    .iter()
                    .filter_map(|id| model.indices.get(id).copied())
                    .collect::<Vec<_>>();
                let Some(&fixed_110) = model.indices.get(&110) else {
                    return false;
                };
                let Some(&fixed_318) = model.indices.get(&318) else {
                    return false;
                };
                below.extend([fixed_110, fixed_318]);
                if candidate.mir_band == "any" && !below.contains(&mir) {
                    return false;
                }
                for team in 0..n {
                    if team == pal {
                        continue;
                    }
                    let (Some(&lo), Some(&hi)) =
                        (leaf.pattern.lower.get(team), leaf.pattern.upper.get(team))
                    else {
                        return false;
                    };
                    if below.contains(&team) {
                        if lo > packed_target {
                            return false;
                        }
                    } else if hi < packed_target {
                        return false;
                    }
                }
                let band = packed_band_target(&candidate.mir_band, stride);
                band.is_none_or(|packed| {
                    leaf.pattern.lower.get(mir).is_some_and(|lo| *lo <= packed)
                        && leaf.pattern.upper.get(mir).is_some_and(|hi| packed <= *hi)
                })
            })
            .collect::<Vec<_>>();
        self.cache.entry(hash).or_default().push((
            BoundsKey {
                root: leaf.root.clone(),
                lower: leaf.pattern.lower.clone(),
                upper: leaf.pattern.upper.clone(),
            },
            result.clone(),
        ));
        Some(result)
    }

    pub(super) fn frontier_weights(
        &mut self,
        model: &Model,
        target_games: &[(usize, usize)],
        leaves: &[Leaf],
        baseline: &[f64],
    ) -> Option<Vec<f64>> {
        if self.mode.is_none() {
            return None;
        }
        let arithmetic_fee = self
            .candidates
            .len()
            .saturating_mul(leaves.len())
            .saturating_mul(3);
        if !self.reserve(arithmetic_fee) {
            return None;
        }
        self.frontier_passes += 1;
        let mut compatible = Vec::with_capacity(leaves.len());
        for leaf in leaves {
            compatible.push(self.compatible_vector(model, target_games, leaf)?);
        }
        let candidate_weights = self
            .candidates
            .iter()
            .map(|candidate| candidate.weight)
            .collect::<Vec<_>>();
        let scores = leaves.iter().map(|leaf| leaf.score).collect::<Vec<_>>();
        let proxies = leaves
            .iter()
            .map(|leaf| leaf.pattern.mass * leaf.pattern.joint.residual_hint)
            .collect::<Vec<_>>();
        let frozen = leaves.iter().map(|leaf| leaf.frozen).collect::<Vec<_>>();
        let (per_candidate, no_compatible, no_live, unmatched) =
            distribute_candidates(&candidate_weights, &compatible, &scores, &proxies, &frozen);
        self.no_compatible_candidates += no_compatible;
        self.no_live_compatible_candidates += no_live;
        self.unmatched_weight = unmatched;
        let mut guided = vec![0.0; leaves.len()];
        for distribution in per_candidate {
            for (share, weight) in guided.iter_mut().zip(distribution) {
                *share += weight;
            }
        }
        let guide_total = guided.iter().sum::<f64>();
        if !guide_total.is_finite() || guide_total <= 0.0 {
            return None;
        }
        for weight in &mut guided {
            *weight /= guide_total;
        }
        let base = if baseline.len() == leaves.len() {
            normalized(baseline)
        } else {
            normalized(
                &leaves
                    .iter()
                    .map(|leaf| leaf.score.max(0.0))
                    .collect::<Vec<_>>(),
            )
        };
        self.applied_selections += 1;
        Some(mix_frontier(&guided, &base, &frozen))
    }

    pub(super) fn entropy_team(
        &mut self,
        model: &Model,
        leaf: &Leaf,
        target_team: usize,
        compatible: &[bool],
        ids: &[i32],
    ) -> Option<usize> {
        if self.mode != Some(Mode::Entropy) {
            return None;
        }
        let fee = self
            .candidates
            .len()
            .saturating_mul(model.n)
            .saturating_mul(6);
        if !self.reserve(fee) {
            return None;
        }
        let best = label_entropy_team(
            &self.candidates,
            compatible,
            target_team,
            leaf.selected,
            leaf.ambiguous,
            ids,
        );
        if best.is_some() {
            self.entropy_selections += 1;
        }
        best
    }

    pub(super) fn compatible_for_leaf(&mut self, leaf: &Leaf) -> Option<Vec<bool>> {
        let fee = leaf
            .root
            .len()
            .saturating_add(2usize.saturating_mul(leaf.pattern.lower.len()))
            .saturating_add(self.candidates.len());
        if !self.reserve(fee) {
            return None;
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        leaf.root.hash(&mut hasher);
        leaf.pattern.lower.hash(&mut hasher);
        leaf.pattern.upper.hash(&mut hasher);
        self.cache
            .get(&hasher.finish())?
            .iter()
            .find(|(key, _)| {
                key.root == leaf.root
                    && key.lower == leaf.pattern.lower
                    && key.upper == leaf.pattern.upper
            })
            .map(|(_, value)| value.clone())
    }

    pub(super) fn diagnostics(&self, requested: &str) -> Value {
        json!({"requested_mode":requested,"enabled":self.mode.is_some(),"reason":self.reason,
            "work":self.work,"work_cap":self.cap,"prepaid_scan_work":4550usize*128+64usize*128,
            "compatibility_cache_hits":self.cache_hits,"compatibility_cache_misses":self.cache_misses,
            "frontier_passes":self.frontier_passes,"applied_selections":self.applied_selections,
            "entropy_selections":self.entropy_selections,"unmatched_candidate_weight":self.unmatched_weight,
            "no_compatible_candidates":self.no_compatible_candidates,
            "no_live_compatible_candidates":self.no_live_compatible_candidates,
            "candidate_count":self.candidates.len()})
    }
}

fn normalized(values: &[f64]) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    let sum = values
        .iter()
        .map(|x| if x.is_finite() { x.max(0.0) } else { 0.0 })
        .sum::<f64>();
    if !sum.is_finite() || sum <= 0.0 {
        return vec![1.0 / values.len() as f64; values.len()];
    }
    values
        .iter()
        .map(|x| if x.is_finite() { x.max(0.0) / sum } else { 0.0 })
        .collect()
}

fn packed_band_target(band: &str, stride: i32) -> Option<i32> {
    match band {
        "57" => Some(57 * stride + 16),
        "58" => Some(58 * stride + 16),
        "60" => Some(60 * stride + 17),
        _ => None,
    }
}

fn label_entropy_team(
    candidates: &[Candidate],
    compatible: &[bool],
    target_team: usize,
    selected: u64,
    ambiguous: u64,
    ids: &[i32],
) -> Option<usize> {
    let selected_weight = candidates
        .iter()
        .zip(compatible)
        .filter(|(_, ok)| **ok)
        .map(|(candidate, _)| candidate.weight)
        .sum::<f64>();
    if !selected_weight.is_finite() || selected_weight <= 0.0 {
        return None;
    }
    let mut best: Option<(usize, f64)> = None;
    for team in 0..ids.len() {
        if team == target_team || selected & (1 << team) != 0 || ambiguous & (1 << team) == 0 {
            continue;
        }
        let id = ids[team];
        let below_weight = candidates
            .iter()
            .zip(compatible)
            .filter(|(_, ok)| **ok)
            .filter(|(candidate, _)| candidate.below_ids.contains(&id) || id == 110 || id == 318)
            .map(|(candidate, _)| candidate.weight)
            .sum::<f64>()
            / selected_weight;
        let p = below_weight.clamp(0.0, 1.0);
        let entropy = if p == 0.0 || p == 1.0 {
            0.0
        } else {
            -p * p.ln() - (1.0 - p) * (1.0 - p).ln()
        };
        if entropy > 0.0
            && best.is_none_or(|(old, score)| {
                entropy > score || (entropy == score && ids[team] < ids[old])
            })
        {
            best = Some((team, entropy));
        }
    }
    best.map(|(team, _)| team)
}

fn distribute_candidates(
    candidate_weights: &[f64],
    compatibility: &[Vec<bool>],
    scores: &[f64],
    proxies: &[f64],
    frozen: &[bool],
) -> (Vec<Vec<f64>>, usize, usize, f64) {
    let mut distributions = vec![vec![0.0; compatibility.len()]; candidate_weights.len()];
    let mut no_compatible = 0;
    let mut no_live = 0;
    let mut unmatched = 0.0;
    for (candidate, &candidate_weight) in candidate_weights.iter().enumerate() {
        let all = compatibility
            .iter()
            .enumerate()
            .filter_map(|(leaf, row)| row.get(candidate).copied().unwrap_or(false).then_some(leaf))
            .collect::<Vec<_>>();
        if all.is_empty() {
            no_compatible += 1;
            unmatched += candidate_weight;
            continue;
        }
        let live = all
            .iter()
            .copied()
            .filter(|&leaf| !frozen.get(leaf).copied().unwrap_or(true))
            .collect::<Vec<_>>();
        if live.is_empty() {
            no_live += 1;
            unmatched += candidate_weight;
            continue;
        }
        let weights = live
            .iter()
            .map(|&leaf| {
                let score = scores.get(leaf).copied().unwrap_or(0.0);
                if score.is_finite() && score > 0.0 {
                    score
                } else {
                    proxies.get(leaf).copied().unwrap_or(0.0).max(0.0)
                }
            })
            .collect::<Vec<_>>();
        let total = weights.iter().sum::<f64>();
        if total.is_finite() && total > 0.0 {
            for (&leaf, weight) in live.iter().zip(weights) {
                distributions[candidate][leaf] = candidate_weight * weight / total;
            }
        } else {
            for leaf in &live {
                distributions[candidate][*leaf] = candidate_weight / live.len() as f64;
            }
        }
    }
    (distributions, no_compatible, no_live, unmatched)
}

fn mix_frontier(guided: &[f64], baseline: &[f64], frozen: &[bool]) -> Vec<f64> {
    let base = normalized(baseline);
    let live = frozen.iter().filter(|&&value| !value).count();
    guided
        .iter()
        .zip(base)
        .enumerate()
        .map(|(i, (guide, base))| {
            0.85 * guide
                + 0.10 * base
                + if frozen.get(i).copied().unwrap_or(true) || live == 0 {
                    0.0
                } else {
                    0.05 / live as f64
                }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{distribute_candidates, label_entropy_team, mix_frontier, packed_band_target};
    use crate::shortlist_guidance::Candidate;

    #[test]
    fn candidate_weights_are_distributed_once_over_live_compatible_leaves() {
        let compatibility = vec![
            vec![true, false, false, false],
            vec![true, true, false, false],
            vec![false, false, false, false],
            vec![false, true, true, false],
        ];
        let (mapped, no_compatible, no_live, unmatched) = distribute_candidates(
            &[0.7, 0.15, 0.1, 0.05],
            &compatibility,
            &[1.0, 3.0, 0.0, 2.0],
            &[0.0; 4],
            &[false, false, false, true],
        );
        assert_eq!(no_compatible, 1);
        assert_eq!(no_live, 1);
        assert!((unmatched - 0.15).abs() < 1e-12);
        assert!((mapped[0].iter().sum::<f64>() - 0.7).abs() < 1e-12);
        assert!((mapped[1].iter().sum::<f64>() - 0.15).abs() < 1e-12);
        assert_eq!(
            mapped[1][3], 0.0,
            "frozen compatible leaves receive no refinement mass"
        );
    }

    #[test]
    fn frontier_mix_keeps_baseline_and_uniform_live_exploration() {
        let mixed = mix_frontier(&[1.0, 0.0, 0.0], &[0.0, 0.0, 1.0], &[false, false, true]);
        assert!((mixed.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(
            mixed[1] > 0.0,
            "live noncandidate leaves retain uniform exploration"
        );
        assert!(
            mixed[2] > 0.0,
            "existing baseline priorities remain represented"
        );
    }

    #[test]
    fn entropy_team_choice_is_deterministic_and_bands_use_dynamic_stride() {
        let candidates = vec![
            Candidate {
                below_ids: vec![5],
                mir_band: "57".into(),
                log_score: 0.0,
                weight: 0.5,
            },
            Candidate {
                below_ids: vec![4],
                mir_band: "58".into(),
                log_score: 0.0,
                weight: 0.5,
            },
        ];
        let first = label_entropy_team(&candidates, &[true, true], 2, 0, 0b011, &[5, 4, 9]);
        let second = label_entropy_team(&candidates, &[true, true], 2, 0, 0b011, &[5, 4, 9]);
        assert_eq!(
            first,
            Some(1),
            "equal entropy ties use stable team-id order"
        );
        assert_eq!(first, second);
        assert_eq!(packed_band_target("57", 10), Some(586));
        assert_eq!(packed_band_target("57", 11), Some(643));
        let skewed = vec![
            Candidate {
                below_ids: vec![5],
                mir_band: "57".into(),
                log_score: 0.0,
                weight: 0.8,
            },
            Candidate {
                below_ids: vec![4],
                mir_band: "58".into(),
                log_score: 0.0,
                weight: 0.1,
            },
            Candidate {
                below_ids: vec![9],
                mir_band: "60".into(),
                log_score: 0.0,
                weight: 0.1,
            },
        ];
        assert_eq!(
            label_entropy_team(&skewed, &[true, true, true], 2, 0, 0b011, &[5, 4, 9]),
            Some(0)
        );
    }
}
