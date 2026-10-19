pub use golaberto_odds_core::{
    conditioned, config, domains, goal_completion, goal_tilt, head_buckets, logging, lookahead,
    model, native_complement, neighbors, outcome_stratification, pilot_allocation, pool, proof,
    rank_proof, reachability, rng, sampling, score_integral, sort, target_limits, tilt,
};
pub mod shortlist_guidance;
pub use golaberto_odds_core::search_support as search;
pub use golaberto_odds_core::{budget, family_config};
pub mod joint_caps;

/// Compatibility paths used by the proposal implementation. These contain
/// only shared configuration and accounting, never scheduler code.
pub mod rare_tail {
    pub use golaberto_odds_core::budget;
    pub use golaberto_odds_core::config::value;
    pub use golaberto_odds_core::family_config;
    pub use golaberto_odds_core::family_config::{structure as family_structure, FamilyStructure};
}

#[cfg(any(test, feature = "test-support"))]
pub mod test_env {
    pub use golaberto_odds_core::test_env::*;
}

#[cfg(test)]
mod tests {
    use crate::{
        joint_caps::propagated::lazy::BranchStrata,
        model::{Model, Request},
        search::Cell,
    };
    use sha2::{Digest, Sha256};
    use std::collections::BTreeSet;

    #[test]
    fn canonical_beam_maps_to_actual_pal_all_loss_leaves() {
        const HYBRID: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_HYBRID_TREE";
        const REPAIR: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_HYBRID_REPAIR";
        const TREE: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_SHORTLIST_TREE";
        const SOURCE: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_SHORTLIST_SOURCE";
        const INPUT: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/palmeiras-shortlist/group-16498-uploaded-285played.json"
        ));
        const GUIDE: &str = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/palmeiras-shortlist/candidate_scores_beam64.json"
        );
        let _env = crate::test_env::scoped(&[
            (HYBRID, Some("mixed")),
            (REPAIR, Some("1")),
            (TREE, Some("mass")),
            (SOURCE, Some(GUIDE)),
            (crate::shortlist_guidance::MODE_ENV, None),
        ]);
        assert_eq!(
            format!("{:x}", Sha256::digest(INPUT)),
            crate::shortlist_guidance::REQUEST_SHA256
        );
        let request: Request = serde_json::from_slice(INPUT).unwrap();
        let model = Model::new(request).unwrap();
        let guide = crate::shortlist_guidance::parse_file(&model, "static", GUIDE);
        assert!(guide.fallback.is_none(), "{:?}", guide.fallback);
        assert_eq!(guide.candidates.len(), 53);
        let (tree_guide, scan_work) = crate::shortlist_guidance::load_tree(&model);
        assert!(tree_guide.fallback.is_none(), "{:?}", tree_guide.fallback);
        assert_eq!(tree_guide.candidates.len(), 53);
        assert_eq!(scan_work, 4_614 * 128);
        let pal = model.indices[&16];
        let cell = Cell {
            team: pal,
            rank: 13,
        };
        let certificate =
            crate::target_limits::certify(&model, cell, crate::target_limits::NODE_LIMIT);
        assert!(certificate.lower.is_some() || certificate.upper.is_some());
        let (proposal, _) = BranchStrata::with_rank_rescue_tree_experiment(
            &model, cell, 808, true, true, true, true, true, 25,
        );
        let proposal = proposal.unwrap();
        let diagnostics = proposal.setup_diagnostics();
        let hybrid_diag = &diagnostics["tree"]["hybrid_tree"];
        assert_eq!(hybrid_diag["repair_enabled"], true);
        assert_eq!(hybrid_diag["applied"], true);
        assert!(
            hybrid_diag["repair_hint_work"].as_u64().unwrap_or(0) > 0,
            "repair ordering hints were not accounted: {hybrid_diag}"
        );
        assert!(hybrid_diag["work"].as_u64().unwrap() <= hybrid_diag["cap"].as_u64().unwrap());
        let tree_guide_diag = &diagnostics["tree"]["shortlist_tree_guidance"];
        assert_eq!(tree_guide_diag["requested_mode"], "mass");
        assert_eq!(tree_guide_diag["candidate_count"], 53);
        assert!(
            crate::test_env::var(crate::shortlist_guidance::MODE_ENV).is_err(),
            "tree guidance must work without final shortlist mode"
        );
        assert!(
            tree_guide_diag["frontier_passes"].as_u64().unwrap_or(0) > 0,
            "canonical tree guide did not map live frontier leaves: {tree_guide_diag}"
        );
        assert!(
            tree_guide_diag["applied_selections"].as_u64().unwrap_or(0) > 0,
            "canonical tree guide did not apply frontier guidance: {tree_guide_diag}"
        );
        let mapped = proposal
            .shortlist_guidance_shares(&model, &guide.candidates)
            .unwrap();
        assert_eq!(mapped.compatible_counts.len(), 53);
        assert!(mapped.compatible_counts.iter().any(|count| *count > 0));
        assert!(mapped
            .shares
            .iter()
            .all(|share| share.is_finite() && *share >= 0.0));
        assert!((mapped.shares.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        let all_loss = mapped
            .all_loss_leaf_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert!(mapped
            .shares
            .iter()
            .enumerate()
            .all(|(i, share)| all_loss.contains(&i) || *share == 0.0));
        let mixed = mapped
            .shares
            .iter()
            .map(|share| 0.95 * share + 0.05 / mapped.shares.len() as f64)
            .collect::<Vec<_>>();
        assert!(mixed.iter().all(|share| *share > 0.0));
        let groups = proposal.path_priority_raw_sampling_groups().unwrap();
        assert_eq!(groups.len(), 56);
        let (parents, child_shares) = proposal.rival_path_parent_map().unwrap();
        let mut totals = std::collections::BTreeMap::<usize, f64>::new();
        for (parent, share) in parents.into_iter().zip(child_shares) {
            *totals.entry(parent).or_default() += share;
        }
        assert!(!totals.is_empty());
        assert!(totals.values().all(|sum| (*sum - 1.0).abs() < 1e-8));
    }

    #[test]
    fn certified_zero_point_path_filters_both_builders() {
        use crate::model::Request;
        let m = crate::model::Model::new(
            serde_json::from_str::<Request>(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json"
            )))
            .unwrap(),
        )
        .unwrap();
        let cell = crate::search::Cell {
            team: m.indices[&16],
            rank: 15,
        };
        let c = crate::target_limits::certify(&m, cell, 16000);
        assert!(c.upper.is_some());
        assert!(c.nodes <= 16000);
        let again = crate::target_limits::certify(&m, cell, 1);
        assert_eq!(again.nodes, c.nodes);
        assert_eq!(again.upper, c.upper);
        assert_eq!(
            crate::target_limits::certify_with_work(&m, cell, 16000).1,
            0
        );
        let p =
            crate::joint_caps::propagated::lazy::LazyJoint::new(&m, cell, 808, &[], 32).unwrap();
        assert!(
            (p.describe(&m)["conditioning_mass"].as_f64().unwrap() / 2.0019063431519247e-8 - 1.)
                .abs()
                < 1e-10
        );
        let p = crate::joint_caps::propagated::PropagatedJoint::configured(
            &m,
            cell,
            crate::joint_caps::propagated::Limits {
                minimum_forced: 0,
                rivals: 4,
                target_patterns: 1,
                feasible_cases: 128,
            },
        );
        assert!(!matches!(p, Err("target_pattern_limit")));
    }
}
