use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};

struct OutputDir(PathBuf);
impl Drop for OutputDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn clean_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_golaberto-odds"));
    for (key, _) in std::env::vars_os() {
        let key = key.to_string_lossy();
        if key.starts_with("RUST_ODDS_") || key.starts_with("RARE_POSITION_") {
            command.env_remove(key.as_ref());
        }
    }
    command
}

fn run(
    request: &PathBuf,
    output: &PathBuf,
    workers: usize,
    recovery: &str,
) -> (Vec<u8>, Vec<Value>) {
    let result = clean_command()
        .env("RUST_ODDS_LOG", "1")
        .env("RUST_ODDS_PROFILE", "1")
        .env("RUST_ODDS_DETERMINISTIC_WORK", "1")
        .env("RUST_ODDS_RARE_TAIL_POSITIVE_BRANCHES", recovery)
        .arg("estimate")
        .arg(request)
        .arg(output)
        .arg("810")
        .arg(workers.to_string())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let logs = String::from_utf8(result.stderr)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    (fs::read(output).unwrap(), logs)
}

fn assert_valid_rank_matrix(response: &Value) {
    let rows = response["rare_position_estimates"].as_object().unwrap();
    assert_eq!(rows.len(), 20);
    let mut matrix = Vec::with_capacity(400);
    for row in rows.values() {
        let ranks = row.as_object().unwrap();
        assert_eq!(ranks.len(), 20);
        for estimate in ranks.values() {
            let probability = estimate["probability"].as_f64().unwrap();
            assert!(probability.is_finite() && (0.0..=1.0).contains(&probability));
            let impossible = estimate["reachability"]
                .as_str()
                .is_some_and(|label| label.starts_with("impossible"));
            assert!(
                impossible || probability > 0.0,
                "cell must be positive or proven impossible"
            );
            matrix.push(probability);
        }
    }
    for i in 0..20 {
        let row_sum: f64 = matrix[i * 20..(i + 1) * 20].iter().sum();
        let column_sum: f64 = (0..20).map(|j| matrix[j * 20 + i]).sum();
        assert!((row_sum - 1.0).abs() <= 1e-12, "row {i}: {row_sum}");
        assert!(
            (column_sum - 1.0).abs() <= 1e-12,
            "column {i}: {column_sum}"
        );
    }
}

#[test]
fn weak_positive_complete_branch_recovery_is_bounded_and_deterministic() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let request = root.join("tests/fixtures/group-16498-2026-10-19.json");
    let dir = OutputDir(std::env::temp_dir().join(format!(
        "golaberto-positive-branch-recovery-{}",
        std::process::id()
    )));
    fs::create_dir(&dir.0).unwrap();

    let (disabled_bytes, disabled_logs) = run(&request, &dir.0.join("disabled.json"), 1, "0");
    let (one_worker_bytes, one_logs) = run(&request, &dir.0.join("one-worker.json"), 1, "1");
    let (four_worker_bytes, four_logs) = run(&request, &dir.0.join("four-workers.json"), 4, "1");
    assert_eq!(one_worker_bytes, four_worker_bytes);

    let disabled: Value = serde_json::from_slice(&disabled_bytes).unwrap();
    let recovered: Value = serde_json::from_slice(&one_worker_bytes).unwrap();
    assert_valid_rank_matrix(&disabled);
    assert_valid_rank_matrix(&recovered);

    // The older stage now already recovers this reference cell at the target
    // probability scale, so keep its order-of-magnitude check without requiring
    // positive-branch recovery to run on a cell that no longer needs it.
    let reference_old = &disabled["rare_position_estimates"]["15"]["16"];
    let reference_new = &recovered["rare_position_estimates"]["15"]["16"];
    let reference_old_probability = reference_old["probability"].as_f64().unwrap();
    let reference_new_probability = reference_new["probability"].as_f64().unwrap();
    assert!(
        (2e-23..=2e-22).contains(&reference_old_probability),
        "baseline reference estimate={reference_old_probability:e}"
    );
    assert!(
        (2e-23..=2e-22).contains(&reference_new_probability),
        "enabled reference estimate={reference_new_probability:e}"
    );

    let refined = one_logs
        .iter()
        .find(|event| {
            event["event"] == "rust_odds_rare_tail_branches"
                && event["positive_refinement"] == true
                && event["positive_accepted"] == true
        })
        .expect("accepted positive complete-branch refinement");
    assert_eq!(refined["positive_quality_improved"], true);
    let positive_team = refined["team"].as_u64().unwrap().to_string();
    let positive_rank = (refined["rank"].as_u64().unwrap() - 1).to_string();
    let new = &recovered["rare_position_estimates"][&positive_team][&positive_rank];
    let old_probability = refined["positive_old_probability"].as_f64().unwrap();
    let new_probability = new["probability"].as_f64().unwrap();
    let old_quality = &refined["positive_old_quality"];
    let old_ess = old_quality["ess"].as_f64().unwrap();
    let old_relative_se = old_quality["rse"].as_f64().unwrap();
    let old_max_share = old_quality["max_share"].as_f64().unwrap();
    let new_relative_se = new["relative_se"].as_f64().unwrap();
    assert!(old_probability > 0.0, "old={old_probability:e}");
    assert!(new_probability > 0.0, "new={new_probability:e}");
    assert!(
        (old_probability / 2.0..=old_probability * 2.0).contains(&new_probability),
        "complete-branch estimate={new_probability:e}, baseline={old_probability:e}"
    );
    let logged_new_probability = refined["positive_new_probability"].as_f64().unwrap();
    assert!((logged_new_probability - new_probability).abs() / new_probability < 1e-5);
    assert_eq!(
        new["design"],
        "matched_point_pool_complete_branches_positive"
    );
    assert_eq!(new["meets_precision_goal"], true);
    assert!(new["ess"].as_f64().unwrap() > old_ess);
    assert!(new_relative_se < old_relative_se);
    assert!(new["max_event_weight_share"].as_f64().unwrap() <= old_max_share);

    let old_rows = disabled["rare_position_estimates"].as_object().unwrap();
    let new_rows = recovered["rare_position_estimates"].as_object().unwrap();
    for (team, old_row) in old_rows {
        for (rank, old_cell) in old_row.as_object().unwrap() {
            let new_cell = &new_rows[team][rank];
            if old_cell["probability"].as_f64().unwrap_or(0.0) > 0.0 {
                assert!(
                    new_cell["probability"].as_f64().unwrap_or(0.0) > 0.0,
                    "positive cell lost: team={team}, rank={rank}"
                );
            }
            if old_cell["reachability"]
                .as_str()
                .is_some_and(|label| label.starts_with("impossible"))
            {
                assert_eq!(new_cell["reachability"], old_cell["reachability"]);
            }
        }
    }

    let summary = one_logs
        .iter()
        .find(|event| {
            event["event"] == "rust_odds_rare_tail_branches_summary"
                && event["positive_branches_enabled"] == true
        })
        .expect("complete-branch stage summary");
    let zero_limit = summary["zero_work_limit"].as_u64().unwrap();
    let positive_limit = summary["positive_work_limit"].as_u64().unwrap();
    let zero_reserved = summary["zero_reserved_work"].as_u64().unwrap();
    let positive_reserved = summary["positive_reserved_work"].as_u64().unwrap();
    assert_eq!(zero_limit, 3 * positive_limit);
    assert!(zero_reserved <= zero_limit);
    assert!(positive_reserved <= positive_limit);
    assert_eq!(
        summary["work_limit"].as_u64().unwrap(),
        zero_limit + positive_limit
    );
    assert_eq!(
        summary["reserved_work"].as_u64().unwrap(),
        zero_reserved + positive_reserved
    );
    assert!(summary["positive_eligible"].as_u64().unwrap() > 0);
    assert!(summary["positive_attempted"].as_u64().unwrap() > 0);
    assert!(summary["positive_attempted"].as_u64().unwrap() <= 2);
    assert!(summary["positive_refined"].as_u64().unwrap() > 0);
    assert!(summary["attempted"].as_u64().unwrap() <= 8);
    let reserved = summary["reserved_work"].as_u64().unwrap();
    let limit = summary["work_limit"].as_u64().unwrap();
    assert!(reserved <= limit, "reserved={reserved}, limit={limit}");

    let coverage = &refined["branch_coverage"];
    assert!(
        coverage["unsupported_upper_sum"].as_f64().unwrap()
            <= coverage["unsupported_allowance"].as_f64().unwrap()
    );
    let branches = coverage["branches"].as_array().unwrap();
    assert!(!branches.is_empty());
    for branch in branches {
        assert!(branch["allocated_draws"].as_u64().unwrap() > 0);
        assert!(branch["main"]["samples"].as_u64().unwrap() > 0);
        assert!(branch["check"]["samples"].as_u64().unwrap() > 0);
    }

    let four_summary = four_logs
        .iter()
        .find(|event| {
            event["event"] == "rust_odds_rare_tail_branches_summary"
                && event["positive_branches_enabled"] == true
        })
        .expect("four-worker complete-branch stage summary");
    assert!(four_summary["positive_attempted"].as_u64().unwrap() <= 2);
    assert!(four_summary["attempted"].as_u64().unwrap() <= 8);
    assert!(
        four_summary["reserved_work"].as_u64().unwrap()
            <= four_summary["work_limit"].as_u64().unwrap()
    );

    let disabled_summary = disabled_logs
        .iter()
        .find(|event| {
            event["event"] == "rust_odds_rare_tail_branches_summary"
                && event["positive_branches_enabled"] == false
                && event["tightened_only"] == false
                && event["tree_mode"] == false
        })
        .expect("disabled complete-branch stage summary");
    assert_eq!(disabled_summary["positive_work_limit"].as_u64().unwrap(), 0);
    assert_eq!(
        disabled_summary["positive_reserved_work"].as_u64().unwrap(),
        0
    );
    assert_eq!(
        disabled_summary["zero_work_limit"].as_u64().unwrap(),
        zero_limit
    );
    assert!(disabled_summary["zero_reserved_work"].as_u64().unwrap() <= zero_limit);
    assert_eq!(
        disabled_summary["work_limit"].as_u64().unwrap(),
        disabled_summary["zero_work_limit"].as_u64().unwrap()
    );
    assert_eq!(
        disabled_summary["reserved_work"].as_u64().unwrap(),
        disabled_summary["zero_reserved_work"].as_u64().unwrap()
    );
}
