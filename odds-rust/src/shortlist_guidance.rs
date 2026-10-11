//! Frozen H2 top-64 / H5 shortlist scores used only to allocate rare-tail work.
use crate::model::Model;
use serde_json::Value;
use std::{collections::BTreeSet, fs};

pub(crate) const MODE_ENV: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_SHORTLIST_MODE";
pub(crate) const SOURCE_ENV: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_SHORTLIST_SOURCE";
const REQUEST_SHA256: &str = "b4613707f5b2e081040e23f74cb73215b91bf90440ed2faa845ec84c0c285aa0";

#[derive(Clone, Debug)]
pub(crate) struct Candidate {
    pub below_ids: Vec<i32>,
    pub mir_band: String,
    pub log_score: f64,
    pub weight: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct Guide {
    pub candidates: Vec<Candidate>,
    pub fallback: Option<String>,
}

fn fail(reason: &str) -> Guide {
    Guide { candidates: Vec::new(), fallback: Some(reason.into()) }
}

pub(crate) fn load(model: &Model) -> Guide {
    let mode = std::env::var(MODE_ENV).unwrap_or_default();
    if mode.is_empty() { return Guide { candidates: Vec::new(), fallback: None }; }
    let Ok(path) = std::env::var(SOURCE_ENV) else { return fail("guide_source_missing"); };
    parse_file(model, &mode, &path)
}

/// Load the frozen static shortlist for tree-frontier experiments without
/// consulting the independent final-allocation mode flag.
pub(crate) fn load_tree(model: &Model) -> (Guide, usize) {
    let scan_work = 4_550usize.saturating_mul(128).saturating_add(64usize.saturating_mul(128));
    let Ok(path) = std::env::var(SOURCE_ENV) else {
        return (fail("guide_source_missing"), scan_work);
    };
    (parse_file(model, "static", &path), scan_work)
}

fn parse_file(model: &Model, mode: &str, path: &str) -> Guide {
    if mode != "static" && mode != "pilot" { return fail("invalid_mode"); }
    let pal_index = model.indices.get(&16).copied().unwrap_or(usize::MAX);
    if model.ids.get(pal_index).copied() != Some(16)
        || !model.indices.contains_key(&584)
        || !model.indices.contains_key(&110)
        || !model.indices.contains_key(&318)
        || model.n != 20
    { return fail("request_scope_mismatch"); }
    let Ok(bytes) = fs::read(path) else { return fail("guide_read_failed"); };
    let Ok(root) = serde_json::from_slice::<Value>(&bytes) else { return fail("guide_parse_failed"); };
    let Some(metadata) = root.get("metadata") else { return fail("guide_metadata_missing"); };
    let expected_ids = [4,5,6,7,8,10,14,15,16,17,18,20,24,67,74,79,110,225,318,584];
    let actual_ids = model.ids.iter().copied().collect::<BTreeSet<_>>();
    let mir = model.indices.get(&584).copied().unwrap_or(usize::MAX);
    let teams = metadata.get("teams");
    if metadata.get("schema").and_then(Value::as_str) != Some("p14_structure_early_beam64_v1")
        || metadata.get("request_sha256").and_then(Value::as_str) != Some(REQUEST_SHA256)
        || metadata.get("pal_team_id").and_then(Value::as_i64) != Some(16)
        || teams.and_then(|v|v.get("mirassol_id")).and_then(Value::as_i64) != Some(584)
        || teams.and_then(|v| v.get("fixed_below_ids"))
            .and_then(Value::as_array).is_none_or(|a| a.iter().filter_map(Value::as_i64).collect::<BTreeSet<_>>() != [110,318].into_iter().collect())
        || teams.and_then(|v| v.get("fixed_above_ids"))
            .and_then(Value::as_array).is_none_or(|a| a.iter().filter_map(Value::as_i64).collect::<BTreeSet<_>>() != [17].into_iter().collect())
        || actual_ids != expected_ids.into_iter().collect()
        || mir >= model.base.len() || model.base[mir].points != 33 || model.base[mir].wins != 8
        || pal_index >= model.base.len() || model.base[pal_index].points != 57 || model.base[pal_index].wins != 16
        || metadata.get("beam").and_then(|v|v.get("selected_candidate_ids"))
            .and_then(Value::as_array).is_none_or(|a|a.len()!=64)
    { return fail("guide_identity_mismatch"); }
    let selected = metadata.get("beam").and_then(|v|v.get("selected_candidate_ids"))
        .and_then(Value::as_array).unwrap().iter().filter_map(Value::as_str).map(str::to_owned).collect::<BTreeSet<_>>();
    if selected.len() != 64 { return fail("guide_beam_invalid"); }
    let Some(rows) = root.get("rows").and_then(Value::as_array) else { return fail("guide_rows_missing"); };
    if rows.len() != 4550 { return fail("guide_row_count_mismatch"); }
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    for row in rows {
        let Some(id) = row.get("candidate_id").and_then(Value::as_str) else { return fail("candidate_id_invalid"); };
        if !selected.contains(id) { continue; }
        if !seen.insert(id.to_string()) { return fail("candidate_id_duplicate"); }
        let Some(score) = row.get("scores").and_then(|v|v.get("H5")).and_then(Value::as_f64).filter(|x|x.is_finite()) else { continue; };
        let Some(below) = row.get("below_ids").and_then(Value::as_array) else { return fail("below_ids_invalid"); };
        if below.len() != 4 { return fail("below_ids_invalid"); }
        let mut ids = Vec::new();
        for value in below {
            let Some(team_id) = value.as_i64().and_then(|x| i32::try_from(x).ok()) else { return fail("below_ids_invalid"); };
            if !model.indices.contains_key(&team_id) || !ids.iter().all(|x| *x != team_id) { return fail("below_ids_invalid"); }
            ids.push(team_id);
        }
        let Some(band) = row.get("mir_band").and_then(Value::as_str) else { return fail("mir_band_invalid"); };
        if !["any", "57", "58", "60"].contains(&band) { return fail("mir_band_invalid"); }
        if (band == "any") != ids.contains(&584) { return fail("mir_band_membership_mismatch"); }
        candidates.push(Candidate { below_ids: ids, mir_band: band.into(), log_score: score, weight: 0.0 });
    }
    if seen.len() != 64 || candidates.len() != 53 { return fail("guide_candidate_count_mismatch"); }
    let max = candidates.iter().map(|c|c.log_score).fold(f64::NEG_INFINITY, f64::max);
    let total = candidates.iter().map(|c|(c.log_score-max).exp()).sum::<f64>();
    if !total.is_finite() || total <= 0.0 { return fail("guide_softmax_invalid"); }
    for candidate in &mut candidates { candidate.weight = (candidate.log_score-max).exp()/total; }
    Guide { candidates, fallback: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{joint_caps::propagated::lazy::BranchStrata, model::Request, search::Cell};

    #[test]
    fn canonical_beam_maps_to_actual_pal_all_loss_leaves() {
        struct RestoreEnv(Vec<(&'static str, Option<std::ffi::OsString>)>);
        impl Drop for RestoreEnv {
            fn drop(&mut self) {
                for (key, value) in self.0.drain(..) {
                    match value {
                        Some(value) => unsafe { std::env::set_var(key, value) },
                        None => unsafe { std::env::remove_var(key) },
                    }
                }
            }
        }
        const HYBRID: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_HYBRID_TREE";
        const REPAIR: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_HYBRID_REPAIR";
        const TREE: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_SHORTLIST_TREE";
        const SOURCE: &str = "RUST_ODDS_EXPERIMENT_RANK_RESCUE_SHORTLIST_SOURCE";
        let _restore = RestoreEnv([HYBRID, REPAIR, TREE, SOURCE, MODE_ENV].into_iter()
            .map(|key| (key, std::env::var_os(key))).collect());
        unsafe {
            std::env::set_var(HYBRID, "mixed");
            std::env::set_var(REPAIR, "1");
            std::env::set_var(TREE, "mass");
            std::env::set_var(SOURCE, "/workspace/work/p14_structure_early/beam64/candidate_scores_beam64.json");
            std::env::remove_var(MODE_ENV);
        }
        let input = fs::read("/workspace/work/p14_hybrid_tree_prod/v2/inputs/group-16498-uploaded-285played.json").unwrap();
        let request: Request = serde_json::from_slice(&input).unwrap();
        let model = Model::new(request).unwrap();
        let guide = parse_file(&model, "static", "/workspace/work/p14_structure_early/beam64/candidate_scores_beam64.json");
        assert!(guide.fallback.is_none(), "{:?}", guide.fallback);
        assert_eq!(guide.candidates.len(), 53);
        let (tree_guide, scan_work) = load_tree(&model);
        assert!(tree_guide.fallback.is_none(), "{:?}", tree_guide.fallback);
        assert_eq!(tree_guide.candidates.len(), 53);
        assert_eq!(scan_work, 4_614 * 128);
        let pal = model.indices[&16];
        let cell = Cell { team: pal, rank: 13 };
        let certificate = crate::target_limits::certify(&model, cell, crate::target_limits::NODE_LIMIT);
        assert!(certificate.lower.is_some() || certificate.upper.is_some());
        let (proposal, _) = BranchStrata::with_rank_rescue_tree_experiment(
            &model, cell, 808, true, true, true, true, true, 25,
        );
        let proposal = proposal.unwrap();
        let diagnostics = proposal.setup_diagnostics();
        let hybrid_diag = &diagnostics["tree"]["hybrid_tree"];
        assert_eq!(hybrid_diag["repair_enabled"], true);
        assert_eq!(hybrid_diag["applied"], true);
        assert!(hybrid_diag["repair_hint_work"].as_u64().unwrap_or(0) > 0,
            "repair ordering hints were not accounted: {hybrid_diag}");
        assert!(hybrid_diag["work"].as_u64().unwrap() <= hybrid_diag["cap"].as_u64().unwrap());
        let tree_guide_diag = &diagnostics["tree"]["shortlist_tree_guidance"];
        assert_eq!(tree_guide_diag["requested_mode"], "mass");
        assert_eq!(tree_guide_diag["candidate_count"], 53);
        assert!(std::env::var(MODE_ENV).is_err(), "tree guidance must work without final shortlist mode");
        assert!(tree_guide_diag["frontier_passes"].as_u64().unwrap_or(0) > 0,
            "canonical tree guide did not map live frontier leaves: {tree_guide_diag}");
        assert!(tree_guide_diag["applied_selections"].as_u64().unwrap_or(0) > 0,
            "canonical tree guide did not apply frontier guidance: {tree_guide_diag}");
        let mapped = proposal.shortlist_guidance_shares(&model, &guide.candidates).unwrap();
        assert_eq!(mapped.compatible_counts.len(), 53);
        assert!(mapped.compatible_counts.iter().any(|count| *count > 0));
        assert!(mapped.shares.iter().all(|share| share.is_finite() && *share >= 0.0));
        assert!((mapped.shares.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        let all_loss = mapped.all_loss_leaf_ids.iter().copied().collect::<BTreeSet<_>>();
        assert!(mapped.shares.iter().enumerate().all(|(i, share)| all_loss.contains(&i) || *share == 0.0));
        let mixed = mapped.shares.iter().map(|share|0.95*share+0.05/mapped.shares.len() as f64).collect::<Vec<_>>();
        assert!(mixed.iter().all(|share| *share > 0.0));
        let groups = proposal.path_priority_raw_sampling_groups().unwrap();
        assert_eq!(groups.len(), 56);
        let (parents, child_shares) = proposal.rival_path_parent_map().unwrap();
        let mut totals = std::collections::BTreeMap::<usize, f64>::new();
        for (parent, share) in parents.into_iter().zip(child_shares) { *totals.entry(parent).or_default() += share; }
        assert!(!totals.is_empty());
        assert!(totals.values().all(|sum| (*sum-1.0).abs() < 1e-8));
    }
}
