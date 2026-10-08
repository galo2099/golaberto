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
        if key.to_string_lossy().starts_with("RUST_ODDS_")
            || key.to_string_lossy().starts_with("RARE_POSITION_")
        {
            command.env_remove(key);
        }
    }
    command
}

fn run(
    request: &PathBuf,
    output: &PathBuf,
    workers: usize,
    logging: &str,
    expected_cohorts: Option<&str>,
) -> (Vec<u8>, Vec<Value>) {
    let mut command = clean_command();
    command.env("RUST_ODDS_LOG", logging);
    if let Some(setting) = expected_cohorts {
        command.env("RUST_ODDS_EXPECTED_COHORTS", setting);
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

fn validate_response(bytes: &[u8]) -> Value {
    let response: Value = serde_json::from_slice(bytes).unwrap();
    let estimates = response["rare_position_estimates"].as_object().unwrap();
    assert_eq!(estimates.len(), 20);
    for row in estimates.values() {
        let row = row.as_object().unwrap();
        assert_eq!(row.len(), 20);
        for estimate in row.values() {
            let probability = estimate["probability"].as_f64().unwrap();
            assert!(probability.is_finite() && (0.0..=1.0).contains(&probability));
            if estimate["reachability"]
                .as_str()
                .unwrap_or("")
                .starts_with("impossible")
            {
                assert_eq!(probability, 0.0);
            }
        }
    }
    response
}

#[test]
fn expected_cohorts_default_is_bounded_and_deterministic_and_can_be_disabled() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let request = root.join("tests/fixtures/group-16498-expected-cohorts.json");
    let dir = OutputDir(std::env::temp_dir().join(format!(
        "golaberto-expected-cohorts-test-{}",
        std::process::id()
    )));
    fs::create_dir(&dir.0).unwrap();

    // The first invocation exercises the default coverage behavior. Repeating
    // with another worker count and with logging disabled must preserve bytes.
    let (default_bytes, logs) = run(&request, &dir.0.join("default.json"), 4, "1", None);
    let default_response = validate_response(&default_bytes);
    let start = logs
        .iter()
        .find(|event| event["event"] == "rust_odds_start")
        .unwrap();
    assert_eq!(start["rare_tail_profile"], "coverage");
    assert_eq!(start["deterministic_work_budget"], true);
    let summary = logs
        .iter()
        .find(|event| event["event"] == "rust_odds_expected_cohorts_summary")
        .expect("expected-cohort stage summary");
    assert_eq!(summary["enabled"], true);
    assert_eq!(summary["budget_mode"], "operations");
    assert!(summary["candidate_cells"].as_u64().unwrap() > 0);
    assert_eq!(summary["forecast_capacity"], summary["reserve"]);

    let arms: Vec<_> = logs
        .iter()
        .filter(|event| event["event"] == "rust_odds_expected_cohorts_arm")
        .collect();
    assert!(!arms.is_empty(), "eligible candidates should be evaluated");
    for arm in &arms {
        let status = arm["status"].as_str().unwrap_or("");
        assert_ne!(status, "training_overrun");
        assert_ne!(status, "pilot_training_overrun");
        if let Some(grant) = arm["arm_grant"].as_u64() {
            let setup = arm["setup_work"].as_u64().unwrap_or(0);
            let pilot = arm["pilot_work"].as_u64().unwrap_or(0);
            assert!(setup.saturating_add(pilot) <= grant);
        }
    }
    let pairs: Vec<_> = logs
        .iter()
        .filter(|event| event["event"] == "rust_odds_expected_cohorts_pair")
        .collect();
    assert!(
        !pairs.is_empty(),
        "expected an ordered proposal to be evaluated"
    );
    assert_eq!(summary["selected"].as_u64().unwrap() as usize, pairs.len());
    assert_eq!(
        summary["accepted"].as_u64().unwrap() as usize,
        pairs.iter().filter(|pair| pair["accepted"] == true).count()
    );
    let mut actual_final_work = 0u64;
    let mut forecast_pair_work = 0u64;
    for pair in &pairs {
        let main_draws = pair["main_draws"].as_u64().unwrap();
        let check_draws = pair["check_draws"].as_u64().unwrap();
        assert_eq!(main_draws, check_draws, "main/check draw counts must match");
        assert!(
            main_draws >= 1_000,
            "each selected pair must complete its fixed batch"
        );
        let actual = pair["actual_pair_work"].as_u64().unwrap();
        let forecast = pair["forecast_pair_work"].as_u64().unwrap();
        assert_eq!(pair["overrun"], actual > forecast);
        actual_final_work = actual_final_work.saturating_add(actual);
        forecast_pair_work = forecast_pair_work.saturating_add(forecast);
    }
    assert_eq!(
        summary["actual_final_work"].as_u64().unwrap(),
        actual_final_work
    );
    assert_eq!(
        summary["forecast_pair_work"].as_u64().unwrap(),
        forecast_pair_work
    );
    assert_eq!(
        summary["actual_work"].as_u64().unwrap(),
        summary["setup_pilot_work"].as_u64().unwrap() + actual_final_work
    );
    assert_eq!(
        summary["capacity_exceeded"],
        summary["actual_work"].as_u64().unwrap() > summary["forecast_capacity"].as_u64().unwrap()
    );
    for (team, rank, reference_probability) in [
        ("16", "13", 1.45e-24),
        ("16", "14", 4.5e-31),
        ("17", "12", 9.0e-34),
    ] {
        let estimate = &default_response["rare_position_estimates"][team][rank];
        let probability = estimate["probability"].as_f64().unwrap();
        assert!(
            probability > 0.0,
            "team {team} rank {rank} should be recovered"
        );
        assert!(
            (reference_probability / 10.0..=reference_probability * 10.0)
                .contains(&probability),
            "team {team} rank {rank} probability {probability:e} should track the reference order of magnitude"
        );
        assert!(estimate["design"]
            .as_str()
            .unwrap()
            .contains("expected_cohorts"));
    }

    let (repeat_bytes, _) = run(&request, &dir.0.join("repeat.json"), 4, "1", None);
    let (single_worker_bytes, _) = run(&request, &dir.0.join("single-worker.json"), 1, "0", None);
    assert_eq!(default_bytes, repeat_bytes);
    assert_eq!(default_bytes, single_worker_bytes);

    let (disabled_bytes, disabled_logs) =
        run(&request, &dir.0.join("disabled.json"), 4, "1", Some("0"));
    let disabled_response = validate_response(&disabled_bytes);
    let disabled = disabled_logs
        .iter()
        .find(|event| event["event"] == "rust_odds_expected_cohorts_summary")
        .expect("disabled expected-cohort stage summary");
    assert_eq!(disabled["enabled"], false);
    assert_eq!(disabled["status"], "disabled");
    assert!(disabled_logs
        .iter()
        .all(|event| event["event"] != "rust_odds_expected_cohorts_pair"));
    for (team, disabled_row) in disabled_response["rare_position_estimates"]
        .as_object()
        .unwrap()
    {
        for (rank, disabled_estimate) in disabled_row.as_object().unwrap() {
            let default_estimate = &default_response["rare_position_estimates"][team][rank];
            if disabled_estimate["probability"].as_f64().unwrap() > 0.0 {
                assert!(
                    default_estimate["probability"].as_f64().unwrap() > 0.0,
                    "default lost an existing positive estimate team={team} rank={rank}"
                );
            }
        }
    }
    for (team, rank) in [("16", "13"), ("16", "14"), ("17", "12")] {
        assert_eq!(
            disabled_response["rare_position_estimates"][team][rank]["probability"], 0.0,
            "opt-out should retain the baseline zero for team {team} rank {rank}"
        );
    }
}
