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
    logging: bool,
    exact_setting: Option<&str>,
) -> (Vec<u8>, Vec<Value>) {
    let mut command = clean_command();
    command.env("RUST_ODDS_LOG", if logging { "1" } else { "0" });
    if let Some(value) = exact_setting {
        command.env("RUST_ODDS_EXACT_CAP_RECOVERY", value);
    }
    let result = command
        .arg("estimate")
        .arg(request)
        .arg(output)
        .arg("808")
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

#[test]
fn chapecoense_recovery_is_worker_and_logging_stable_and_preserves_prior_claims() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let request = root.join("tests/fixtures/group-16498-2026-10-18.json");
    let dir = OutputDir(
        std::env::temp_dir().join(format!("golaberto-exact-caps-test-{}", std::process::id())),
    );
    fs::create_dir(&dir.0).unwrap();

    let (disabled_bytes, _) = run(&request, &dir.0.join("disabled.json"), 1, false, Some("0"));
    let (one_worker_bytes, _) = run(&request, &dir.0.join("one-worker.json"), 1, false, None);
    let (four_worker_bytes, logs) = run(&request, &dir.0.join("four-workers.json"), 4, true, None);

    assert_eq!(one_worker_bytes, four_worker_bytes);
    let disabled: Value = serde_json::from_slice(&disabled_bytes).unwrap();
    let one_worker: Value = serde_json::from_slice(&one_worker_bytes).unwrap();
    let old_cells = disabled["rare_position_estimates"].as_object().unwrap();
    let new_cells = one_worker["rare_position_estimates"].as_object().unwrap();
    for (team, old_row) in old_cells {
        let new_row = &new_cells[team];
        for (rank, old) in old_row.as_object().unwrap() {
            let new = &new_row[rank];
            if old["probability"].as_f64().unwrap_or(0.0) > 0.0 {
                assert!(
                    new["probability"].as_f64().unwrap_or(0.0) > 0.0,
                    "exact caps must preserve prior positive cell team={team}, rank={rank}"
                );
            }
            if old["reachability"] == "impossible" {
                assert_eq!(new["reachability"], "impossible");
            }
        }
    }
    let target = &one_worker["rare_position_estimates"]["318"]["8"];
    let probability = target["probability"].as_f64().unwrap();
    assert!(
        (5.0e-24..=1.5e-23).contains(&probability),
        "Chapecoense rank 9 estimate was {probability:e}"
    );
    assert_eq!(target["design"], "exact_caps_conditioned");
    assert_eq!(target["meets_precision_goal"], true);

    let summary = logs
        .iter()
        .find(|event| event["event"] == "rust_odds_exact_caps_summary")
        .expect("exact-caps stage summary");
    assert_eq!(summary["enabled"], true);
    assert_eq!(summary["overrun"], false);
    let capacity = summary["capacity"].as_u64().unwrap();
    let scan = summary["scan_work"].as_u64().unwrap();
    let setup = summary["setup_work"].as_u64().unwrap();
    let final_work = summary["final_work"].as_u64().unwrap();
    assert!(scan + setup + final_work <= capacity);
    assert!(summary["reserved_final_work"].as_u64().unwrap() >= final_work);
    let published = summary["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["status"] == "published")
        .expect("published exact-caps event");
    let draws = published["draws_per_stream"].as_u64().unwrap();
    assert_eq!(published["main_samples"].as_u64().unwrap(), draws);
    assert_eq!(published["check_samples"].as_u64().unwrap(), draws);
    assert!(published["actual_within_reserve"].as_bool().unwrap());
    assert!(published["bounds_ok"].as_bool().unwrap());
    let reconcile = logs
        .iter()
        .find(|event| event["event"] == "rust_odds_exact_caps_reconcile")
        .expect("exact-caps reconciliation");
    assert_eq!(reconcile["accepted"], 1);
    assert_eq!(reconcile["valid_matrix"], true);
    assert_eq!(reconcile["bounds_preserved"], true);
}
