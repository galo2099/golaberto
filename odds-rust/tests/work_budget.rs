use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};

struct OutputDir(PathBuf);
impl Drop for OutputDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn rare_position_responses_are_identical_across_workers_repeats_and_logging() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = OutputDir(
        std::env::temp_dir().join(format!("golaberto-work-budget-test-{}", std::process::id())),
    );
    fs::create_dir(&dir.0).unwrap();
    for fixture in ["group-16498-44eabb47.json", "group-16653-71d4fea8.json"] {
        let request = root
            .join("../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs")
            .join(fixture);
        let mut expected = None;
        // One core changes wall time substantially; disabling logging previously
        // also changed the time allowance. Neither may change admission now.
        for (i, (workers, logging)) in [(4, "1"), (4, "1"), (1, "0")].into_iter().enumerate() {
            let output = dir.0.join(format!("{fixture}-{i}.json"));
            let mut command = Command::new(env!("CARGO_BIN_EXE_golaberto-odds"));
            for (key, _) in std::env::vars_os() {
                if key.to_string_lossy().starts_with("RUST_ODDS_")
                    || key.to_string_lossy().starts_with("RARE_POSITION_")
                {
                    command.env_remove(key);
                }
            }
            let result = command
                .env("RUST_ODDS_LOG", logging)
                .arg("estimate")
                .arg(&request)
                .arg(&output)
                .arg("808")
                .arg(workers.to_string())
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let bytes = fs::read(&output).unwrap();
            if fixture.starts_with("group-16498-") {
                let response: Value = serde_json::from_slice(&bytes).unwrap();
                assert!(
                    response["rare_position_estimates"]["17"]["12"]["probability"]
                        .as_f64()
                        .unwrap()
                        > 0.,
                    "default must estimate Flamengo/13th"
                );
            }
            if let Some(expected) = &expected {
                assert_eq!(
                    &bytes, expected,
                    "response changed for {fixture}, workers={workers}, logging={logging}"
                );
            } else {
                expected = Some(bytes);
            }
            if logging == "1" {
                let logs: Vec<Value> = String::from_utf8(result.stderr)
                    .unwrap()
                    .lines()
                    .filter_map(|line| serde_json::from_str(line).ok())
                    .collect();
                let start = logs
                    .iter()
                    .find(|e| e["event"] == "rust_odds_start")
                    .unwrap();
                assert_eq!(start["deterministic_work_budget"], true);
                assert_eq!(start["certified_target_limits"], true);
                assert_eq!(start["complete_parent_fallback"], true);
                assert_eq!(start["certified_branch_transfer"], true);
                assert_eq!(start["rare_tail_tree"], true);
                assert_eq!(start["rare_tail_tree_pilot_draws"], 25);
                for event in [
                    "rust_odds_shared_constraints_summary",
                    "rust_odds_rare_tail_summary",
                    "rust_odds_rare_tail_confirm_more_summary",
                    "rust_odds_rare_tail_branches_summary",
                ] {
                    let budgets: Vec<_> = logs.iter().filter(|e| e["event"] == event).collect();
                    assert!(!budgets.is_empty());
                    for budget in budgets {
                        assert_eq!(budget["budget_mode"], "operations");
                        if let Some(reserved) = budget["reserved_work"].as_u64() {
                            assert!(reserved <= budget["work_limit"].as_u64().unwrap());
                        }
                    }
                }
                for credit in logs
                    .iter()
                    .filter(|e| e["event"] == "rust_odds_rare_tail_tree_credit")
                {
                    let units = credit["credit_units"].as_u64().unwrap();
                    assert!(units <= 16_000_000);
                    assert!(units <= credit["tree_reserved"].as_u64().unwrap());
                    assert!(units <= credit["proportional_credit_cap"].as_u64().unwrap());
                    if credit["additional"] == 0 {
                        assert_eq!(units, 0);
                    }
                }
            }
        }
    }
}

#[test]
fn early_branches_respect_a_reduced_confirmation_allowance() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = OutputDir(std::env::temp_dir().join(format!(
        "golaberto-transfer-budget-test-{}",
        std::process::id()
    )));
    fs::create_dir(&dir.0).unwrap();
    let request = root.join(
        "../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json",
    );
    let model = golaberto_odds::model::Model::new(
        serde_json::from_slice(&fs::read(&request).unwrap()).unwrap(),
    )
    .unwrap();
    let configured = (200000_f64 * 0.25 / 1.5).floor() as u64
        * (64 * model.fixtures.len() + 32 * model.n + 1) as u64;
    let mut command = Command::new(env!("CARGO_BIN_EXE_golaberto-odds"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("RUST_ODDS_")
            || key.to_string_lossy().starts_with("RARE_POSITION_")
        {
            command.env_remove(key);
        }
    }
    let result = command
        .env("RUST_ODDS_LOG", "1")
        .env("RUST_ODDS_RARE_TAIL_CONFIRM_MORE", "0.25")
        .arg("estimate")
        .arg(&request)
        .arg(dir.0.join("result.json"))
        .arg("808")
        .arg("4")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let logs: Vec<Value> = String::from_utf8(result.stderr)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let early = logs
        .iter()
        .find(|e| {
            e["event"] == "rust_odds_rare_tail_branches_summary" && e["tightened_only"] == true
        })
        .unwrap();
    let extra = logs
        .iter()
        .find(|e| e["event"] == "rust_odds_rare_tail_confirm_more_summary")
        .unwrap();
    let transferred = extra["transferred_branch_work"].as_u64().unwrap();
    assert!(early["work_limit"].as_u64().unwrap() <= configured);
    let tree = logs
        .iter()
        .find(|e| e["event"] == "rust_odds_rare_tail_tree_credit")
        .unwrap();
    assert_eq!(
        transferred,
        early["reserved_work"].as_u64().unwrap() + tree["tree_reserved"].as_u64().unwrap()
            - tree["credit_units"].as_u64().unwrap()
    );
    assert!(transferred <= configured);
    assert_eq!(
        extra["work_limit"].as_u64().unwrap() + transferred,
        configured
    );
}
