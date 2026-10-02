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
                for event in [
                    "rust_odds_shared_constraints_summary",
                    "rust_odds_rare_tail_summary",
                    "rust_odds_rare_tail_confirm_more_summary",
                    "rust_odds_rare_tail_branches_summary",
                ] {
                    let budget = logs.iter().find(|e| e["event"] == event).unwrap();
                    assert_eq!(budget["budget_mode"], "operations");
                    if let Some(reserved) = budget["reserved_work"].as_u64() {
                        assert!(reserved <= budget["work_limit"].as_u64().unwrap());
                    }
                }
            }
        }
    }
}
