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
    run_seed(request, output, workers, logging, expected_cohorts, 808)
}

fn run_seed(
    request: &PathBuf,
    output: &PathBuf,
    workers: usize,
    logging: &str,
    expected_cohorts: Option<&str>,
    seed: i64,
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
        .arg(seed.to_string())
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

fn run_chapecoense(request: &PathBuf, output: &PathBuf, workers: usize) -> (Vec<u8>, Vec<Value>) {
    let mut command = clean_command();
    command
        .env("RUST_ODDS_LOG", "1")
        .env("RUST_ODDS_EXPECTED_COHORTS", "1")
        .env("RUST_ODDS_FEASIBLE_COHORTS", "1");
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
fn seed_13001_reference_cells_are_confirmed_or_conservatively_declined() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let request = root.join("tests/fixtures/group-16498-expected-cohorts.json");
    let dir = OutputDir(std::env::temp_dir().join(format!(
        "golaberto-expected-cohorts-seed-13001-{}",
        std::process::id()
    )));
    fs::create_dir(&dir.0).unwrap();

    let (bytes, logs) = run_seed(
        &request,
        &dir.0.join("seed-13001.json"),
        4,
        "1",
        None,
        13001,
    );
    let response = validate_response(&bytes);
    let published = [("16", "13", 2.746e-25), ("17", "12", 1.311e-33)]
        .into_iter()
        .filter(|(team, rank, baseline)| {
            assert_reference_result(&logs, &response, team, rank, *baseline, 50.0, 0.6)
        })
        .count();
    assert!(published > 0, "seed 13001 published no reference cells");
    let summary = logs
        .iter()
        .find(|event| event["event"] == "rust_odds_expected_cohorts_summary")
        .unwrap();
    let pairs = logs
        .iter()
        .filter(|event| event["event"] == "rust_odds_expected_cohorts_pair")
        .collect::<Vec<_>>();
    assert_eq!(summary["selected"].as_u64().unwrap() as usize, pairs.len());
    assert_eq!(summary["forecast_capacity"], summary["reserve"]);
    assert!(
        summary["actual_work"].as_u64().unwrap() <= summary["forecast_capacity"].as_u64().unwrap()
    );
    let stages = logs
        .iter()
        .filter(|event| event["event"] == "rust_odds_expected_cohorts_pass_summary")
        .filter_map(|event| event["stage"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(stages, vec!["native_recovery", "gd_refinement"]);
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

fn assert_confirmed_evidence(logs: &[Value], team: &str, rank: &str, estimate: &Value) {
    let team_id = team.parse::<u64>().unwrap();
    let log_rank = rank.parse::<u64>().unwrap() + 1;
    let estimate_probability = estimate["probability"].as_f64().unwrap();
    let evidence = logs
        .iter()
        .find(|event| {
            event["team"] == team_id
                && event["rank"] == log_rank
                && event["accepted"] == true
                && match event["event"].as_str() {
                    Some("rust_odds_rare_tail_confirm_more") => event["main_only"] == false,
                    Some("rust_odds_expected_cohorts_pair") => event["publication_gate"] == true,
                    _ => false,
                }
                && event["probability"].as_f64().is_some_and(|probability| {
                    ((probability - estimate_probability).abs() / estimate_probability) < 1e-5
                })
        })
        .unwrap_or_else(|| panic!("team {team} rank {rank} has no accepted independent evidence"));

    let main_draws = evidence["main_draws"].as_u64().unwrap();
    let check_draws = evidence["check_draws"].as_u64().unwrap();
    assert!(main_draws >= 1_000, "team {team} rank {rank}: {evidence}");
    assert!(check_draws >= 1_000, "team {team} rank {rank}: {evidence}");
    assert!(evidence["main_hits"].as_u64().unwrap() >= 30);
    assert!(evidence["check_hits"].as_u64().unwrap() >= 30);
    assert!(evidence["main_ess"].as_f64().unwrap() >= 3.0);
    assert!(evidence["check_ess"].as_f64().unwrap() >= 3.0);
}

/// Returns true when a reference cell is published. A zero estimate is allowed
/// only when a complete fixed-size independent pair failed publication or a
/// matching candidate was explicitly declined for conservative budget reasons.
fn assert_reference_result(
    logs: &[Value],
    response: &Value,
    team: &str,
    rank: &str,
    reference_probability: f64,
    reference_factor: f64,
    max_relative_se: f64,
) -> bool {
    let estimate = &response["rare_position_estimates"][team][rank];
    let probability = estimate["probability"].as_f64().unwrap();
    if probability > 0.0 {
        assert!(
            (reference_probability / reference_factor..=reference_probability * reference_factor)
                .contains(&probability),
            "team {team} rank {rank} probability {probability:e} should track reference {reference_probability:e} within {reference_factor}x"
        );
        assert!(estimate["relative_se"].as_f64().unwrap() <= max_relative_se);
        assert_confirmed_evidence(logs, team, rank, estimate);
        return true;
    }

    assert!(
        !estimate["reachability"]
            .as_str()
            .unwrap_or("")
            .starts_with("impossible"),
        "reference cell team {team} rank {rank} is known reachable"
    );
    assert_eq!(estimate["meets_precision_goal"], false);
    assert!(estimate["relative_se"].is_null());
    assert_eq!(estimate["ess"].as_f64().unwrap(), 0.0);

    let team_id = team.parse::<u64>().unwrap();
    let log_rank = rank.parse::<u64>().unwrap() + 1;
    assert!(
        !logs.iter().any(|event| {
            event["team"] == team_id
                && event["rank"] == log_rank
                && ((event["event"] == "rust_odds_expected_cohorts_pair"
                    && (event["accepted"] == true || event["publication_gate"] == true))
                    || (event["event"] == "rust_odds_rare_tail_confirm_more"
                        && event["accepted"] == true
                        && event["main_only"] == false))
        }),
        "zero reference estimate team {team} rank {rank} has an accepted publication event"
    );

    let completed_pair = logs.iter().any(|event| {
        if event["event"] != "rust_odds_expected_cohorts_pair"
            || event["team"] != team_id
            || event["rank"] != log_rank
            || event["accepted"] != false
            || event["publication_gate"] != false
        {
            return false;
        }
        let requested = event["requested_draws"].as_u64().unwrap_or(0);
        requested >= 1_000
            && event["main_draws"].as_u64() == Some(requested)
            && event["check_draws"].as_u64() == Some(requested)
    });
    let budget_decline = logs.iter().any(|event| {
        event["event"] == "rust_odds_expected_cohorts_skip"
            && event["team"] == team_id
            && event["rank"] == log_rank
            && event["reason"].as_str().is_some_and(|reason| {
                reason.contains("budget")
                    || reason.contains("cost_admission")
                    || reason.contains("grant")
            })
    });
    assert!(
        completed_pair || budget_decline,
        "zero reference estimate team {team} rank {rank} lacks a completed rejected pair or explicit budget decline"
    );
    false
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
        let requested_draws = pair["requested_draws"].as_u64().unwrap();
        let execution_cap = pair["execution_cap"].as_u64().unwrap();
        let main_draws = pair["main_draws"].as_u64().unwrap();
        let check_draws = pair["check_draws"].as_u64().unwrap();
        assert!(main_draws <= requested_draws);
        assert!(check_draws <= requested_draws);
        let actual = pair["actual_pair_work"].as_u64().unwrap();
        let forecast = pair["forecast_pair_work"].as_u64().unwrap();
        assert!(
            actual <= execution_cap,
            "actual pair work {actual} exceeded execution cap {execution_cap}"
        );
        assert_eq!(pair["overrun"], actual > forecast);
        let complete = main_draws == requested_draws && check_draws == requested_draws;
        if pair["publication_gate"] == true || pair["accepted"] == true {
            assert!(
                complete,
                "published pair did not complete its requested batch"
            );
            assert!(requested_draws >= 1_000);
        }
        if !complete {
            assert_eq!(pair["accepted"], false);
            assert_eq!(pair["publication_gate"], false);
        }
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
    let published = [
        ("16", "13", 1.45e-24),
        ("16", "14", 4.5e-31),
        ("17", "12", 9.0e-34),
    ]
    .into_iter()
    .filter(|(team, rank, reference_probability)| {
        assert_reference_result(
            &logs,
            &default_response,
            team,
            rank,
            *reference_probability,
            10.0,
            0.6,
        )
    })
    .count();
    assert!(published > 0, "default run published no reference cells");

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
}

#[test]
fn weak_expected_cohort_pilots_trigger_bounded_chapecoense_recovery_deterministically() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let request = root.join("tests/fixtures/group-16498-2026-10-18.json");
    let dir = OutputDir(std::env::temp_dir().join(format!(
        "golaberto-chapecoense-feasible-recovery-{}",
        std::process::id()
    )));
    fs::create_dir(&dir.0).unwrap();

    let (four_worker_bytes, four_worker_logs) =
        run_chapecoense(&request, &dir.0.join("four-workers.json"), 4);
    let response = validate_response(&four_worker_bytes);
    let target = &response["rare_position_estimates"]["318"]["8"];
    assert_eq!(target["reachability"], "reachable");
    let trigger = four_worker_logs
        .iter()
        .find(|event| {
            event["event"] == "rust_odds_feasible_cohort_trigger"
                && event["team"] == 318
                && event["rank"] == 9
        })
        .expect("Chapecoense weak expected-cohort pilots should trigger feasible discovery");
    assert_eq!(trigger["trigger"], "all_expected_pilots_weak");
    assert_eq!(trigger["stage"], "gd_refinement");
    assert!(trigger["existing_expected_arms"].as_u64().unwrap() > 0);
    let best_pilot = &trigger["best_pilot"];
    assert_eq!(best_pilot["healthy"], false);
    assert!(best_pilot["probability"].as_f64().unwrap().is_finite());
    assert!(
        four_worker_logs.iter().any(|event| {
            event["team"] == 318
                && event["rank"] == 9
                && (event["event"] == "rust_odds_order_fallback_arm"
                    || event["event"] == "rust_odds_order_fallback_declined")
        }),
        "feasible cohort discovery was not attempted for Chapecoense"
    );

    let summary = four_worker_logs
        .iter()
        .find(|event| event["event"] == "rust_odds_expected_cohorts_summary")
        .expect("expected-cohort summary");
    let actual_work = summary["actual_work"].as_u64().unwrap();
    let forecast_capacity = summary["forecast_capacity"].as_u64().unwrap();
    assert!(actual_work <= forecast_capacity);
    assert!(summary["candidate_cells"].as_u64().unwrap() <= 256);

    let (repeat_bytes, repeat_logs) = run_chapecoense(&request, &dir.0.join("repeat.json"), 4);
    let (one_worker_bytes, one_worker_logs) =
        run_chapecoense(&request, &dir.0.join("one-worker.json"), 1);
    assert_eq!(four_worker_bytes, repeat_bytes);
    assert_eq!(four_worker_bytes, one_worker_bytes);
    for logs in [&repeat_logs, &one_worker_logs] {
        assert!(logs.iter().any(|event| {
            event["event"] == "rust_odds_feasible_cohort_trigger"
                && event["team"] == 318
                && event["rank"] == 9
                && event["trigger"] == "all_expected_pilots_weak"
                && event["stage"] == "gd_refinement"
        }));
        let summary = logs
            .iter()
            .find(|event| event["event"] == "rust_odds_expected_cohorts_summary")
            .unwrap();
        assert!(
            summary["actual_work"].as_u64().unwrap()
                <= summary["forecast_capacity"].as_u64().unwrap()
        );
    }
}
