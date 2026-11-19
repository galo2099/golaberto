use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};

struct OutputDir(PathBuf);
impl Drop for OutputDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_valid_rank_matrix(response: &Value) {
    let rows = response["rare_position_estimates"].as_object().unwrap();
    assert_eq!(rows.len(), 20);
    let mut matrix = Vec::with_capacity(400);
    for (team, row) in rows {
        let ranks = row.as_object().unwrap();
        assert_eq!(ranks.len(), 20);
        for rank in 0..20 {
            let estimate = &ranks[&rank.to_string()];
            let probability = estimate["probability"].as_f64().unwrap();
            assert!(probability.is_finite() && (0.0..=1.0).contains(&probability));
            if estimate["reachability"]
                .as_str()
                .unwrap_or("")
                .starts_with("impossible")
            {
                assert_eq!(probability, 0.0, "impossible team={team} rank={rank}");
            }
            assert_eq!(
                response["team_odds"][team]["Pos"][rank],
                probability * 100.0,
                "team odds disagree for team={team} rank={rank}"
            );
            matrix.push(probability);
        }
    }
    for team in 0..20 {
        let row_sum: f64 = matrix[team * 20..(team + 1) * 20].iter().sum();
        let column_sum: f64 = (0..20).map(|other| matrix[other * 20 + team]).sum();
        assert!((row_sum - 1.0).abs() <= 1e-12, "team row {team}: {row_sum}");
        assert!(
            (column_sum - 1.0).abs() <= 1e-12,
            "rank column {team}: {column_sum}"
        );
    }
}

#[test]
fn default_coverage_runs_shared_sampling_and_zero_disables_the_portfolio() {
    let request = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json",
    );
    let dir = OutputDir(
        std::env::temp_dir().join(format!("golaberto-coverage-test-{}", std::process::id())),
    );
    fs::create_dir(&dir.0).unwrap();
    let run = |disabled: bool| {
        let output = dir.0.join(format!("{disabled}.json"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_golaberto-odds"));
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy();
            if name.starts_with("RUST_ODDS_") || name.starts_with("RARE_POSITION_") {
                command.env_remove(key);
            }
        }
        command.env("RUST_ODDS_LOG", "1");
        if disabled {
            command.env("RUST_ODDS_RARE_TAIL", "0");
        }
        let result = command
            .arg("estimate")
            .arg(&request)
            .arg(&output)
            .arg("808")
            .arg("4")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let response: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        let logs: Vec<Value> = String::from_utf8(result.stderr)
            .unwrap()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        (response, logs)
    };
    let (coverage, logs) = run(false);
    let start = logs
        .iter()
        .find(|e| e["event"] == "rust_odds_start")
        .unwrap();
    assert_eq!(start["rare_tail_profile"], "coverage");
    assert_eq!(start["rare_tail_extension"], "after");
    assert_eq!(start["rare_tail_confirm_more"], "1.5");
    assert_eq!(start["rare_tail_branches"], true);
    assert_eq!(start["rare_tail_branch_bound_floor"], false);
    let branches = logs
        .iter()
        .find(|e| e["event"] == "rust_odds_rare_tail_branches_summary")
        .unwrap();
    assert_eq!(branches["preserved_existing_estimates"], true);
    let confirmation = logs
        .iter()
        .find(|e| e["event"] == "rust_odds_rare_tail_confirm_more_summary")
        .unwrap();
    assert_eq!(confirmation["fraction"], 1.5);
    assert_eq!(confirmation["preserved_existing_estimates"], true);
    let extension = logs
        .iter()
        .find(|e| e["event"] == "rust_odds_rare_tail_extension_summary")
        .unwrap();
    assert_eq!(extension["mode"], "after");
    assert_eq!(start["shared_constraints"], "guided");
    assert_eq!(start["shared_constraints_blockers"], "2");
    assert_eq!(start["shared_constraints_relative"], true);
    assert_eq!(start["shared_constraints_confirmation"], "parallel");
    assert_eq!(start["shared_constraints_fraction"], "0.5");
    let shared = logs
        .iter()
        .find(|e| e["event"] == "rust_odds_shared_constraints_summary")
        .unwrap();
    assert_eq!(shared["mode"], "guided");
    assert_eq!(shared["concurrent_confirmation"], true);
    let (native, logs) = run(true);
    let start = logs
        .iter()
        .find(|e| e["event"] == "rust_odds_start")
        .unwrap();
    assert_eq!(start["rare_tail_profile"], "0");
    assert_eq!(start["rare_tail_branches"], false);
    assert!(!logs
        .iter()
        .any(|e| e["event"] == "rust_odds_rare_tail_branches_summary"));
    assert_eq!(start["rare_tail_extension"], "");
    assert_eq!(start["rare_tail_confirm_more"], "");
    assert!(!logs.iter().any(|e| e["event"].as_str().is_some_and(|s| {
        s.starts_with("rust_odds_shared_constraints") || s.starts_with("rust_odds_rare_tail")
    })));
    assert_eq!(coverage["game_importance"], native["game_importance"]);
    for (team, ranks) in native["rare_position_estimates"].as_object().unwrap() {
        for (rank, cell) in ranks.as_object().unwrap() {
            if cell["reachability"]
                .as_str()
                .unwrap_or("")
                .starts_with("impossible")
            {
                assert_eq!(
                    cell["reachability"],
                    coverage["rare_position_estimates"][team][rank]["reachability"]
                );
            }
        }
    }
}

#[test]
fn propagated_default_preserves_old_work_and_rejects_unconfirmed_estimates() {
    let request = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json",
    );
    let dir = OutputDir(
        std::env::temp_dir().join(format!("golaberto-propagated-test-{}", std::process::id())),
    );
    fs::create_dir(&dir.0).unwrap();
    for seed in [808, 818] {
        let run = |enabled: bool| {
            let output = dir.0.join(format!("{seed}-{enabled}.json"));
            let mut command = Command::new(env!("CARGO_BIN_EXE_golaberto-odds"));
            for (key, _) in std::env::vars_os() {
                let name = key.to_string_lossy();
                if name.starts_with("RUST_ODDS_") || name.starts_with("RARE_POSITION_") {
                    command.env_remove(key);
                }
            }
            command.env("RUST_ODDS_LOG", "1");
            // Isolate joint allocation from the independent coverage portfolio.
            command.env("RUST_ODDS_RARE_TAIL", "0");
            if !enabled {
                command.env("RUST_ODDS_JOINT_PROPAGATION", "0");
            }
            let result = command
                .arg("estimate")
                .arg(&request)
                .arg(&output)
                .arg(seed.to_string())
                .arg("4")
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let response: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
            let logs: Vec<Value> = String::from_utf8(result.stderr)
                .unwrap()
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect();
            (response, logs)
        };
        let (before, _) = run(false);
        let (after, logs) = run(true);
        for (team, ranks) in before["rare_position_estimates"].as_object().unwrap() {
            for (rank, old) in ranks.as_object().unwrap() {
                let new = &after["rare_position_estimates"][team][rank];
                if old["probability"].as_f64().unwrap() > 0. {
                    assert_eq!(
                        old["probability"], new["probability"],
                        "{seed} {team}/{rank}"
                    );
                    assert_eq!(old["std_err"], new["std_err"], "{seed} {team}/{rank}");
                }
                if old["reachability"]
                    .as_str()
                    .unwrap_or("")
                    .starts_with("impossible")
                {
                    assert_eq!(old["reachability"], new["reachability"]);
                }
            }
        }
        assert_eq!(before["game_importance"], after["game_importance"]);
        let cell = &after["rare_position_estimates"]["5"]["19"];
        assert_eq!(cell["reachability"], "reachable");
        let joint = logs
            .iter()
            .find(|e| {
                e["event"] == "rust_odds_joint_caps"
                    && e["setup"]["mode"] == "propagated_target_union"
            })
            .unwrap();
        assert_eq!(joint["setup"]["target_patterns"], 56);
        assert_eq!(joint["setup"]["feasible_patterns"], 9);
        assert_eq!(joint["setup"]["forced_min"], 28);
        assert_eq!(joint["joint_draws"], 5000);
        assert_eq!(joint["check_draws"], 3000);
        assert!(joint["saved_draws"].as_u64().unwrap() >= 8000);
        assert_eq!(joint["ordinary_draws"], 0);
        assert_eq!(joint["accepted"], seed == 808);
        if seed == 808 {
            assert!(cell["probability"].as_f64().unwrap() > 1e-35);
            assert!(cell["probability"].as_f64().unwrap() < 1e-33);
        } else {
            assert_eq!(cell["probability"], 0.);
            assert_eq!(
                cell["zero_hit_upper_95"],
                joint["setup"]["conditioning_mass"]
            );
            assert!(cell["zero_hit_upper_95"].as_f64().unwrap() < 2e-25);
        }
    }
}

#[test]
fn normal_binary_enables_joint_caps_and_preserves_existing_estimates_and_proofs() {
    let request = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json",
    );
    let dir = OutputDir(
        std::env::temp_dir().join(format!("golaberto-joint-test-{}", std::process::id())),
    );
    fs::create_dir(&dir.0).unwrap();
    let run = |name: &str, disabled: bool| {
        let output = dir.0.join(format!("{name}.json"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_golaberto-odds"));
        // Child processes isolate toggles from the other tests and from local
        // benchmark settings. The joint sampler uses its own default settings.
        for (key, _) in std::env::vars_os() {
            let key_name = key.to_string_lossy();
            if key_name.starts_with("RUST_ODDS_") || key_name.starts_with("RARE_POSITION_") {
                command.env_remove(key);
            }
        }
        command.env("RUST_ODDS_LOG", "1");
        command.env("RUST_ODDS_RARE_TAIL", "0");
        // Isolate the exact fast path regression from the broad replacement.
        command.env("RUST_ODDS_JOINT_CAP_BROAD", "0");
        if disabled {
            command.env("RUST_ODDS_JOINT_CAP_CONDITIONING", "0");
        }
        let result = command
            .arg("estimate")
            .arg(&request)
            .arg(&output)
            .arg("1790832522033172000")
            .arg("4")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let response: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        let events: Vec<Value> = String::from_utf8(result.stderr)
            .unwrap()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        (response, events)
    };
    let (baseline, disabled_logs) = run("disabled", true);
    let (candidate, logs) = run("default", false);
    let old = &baseline["rare_position_estimates"];
    let new = &candidate["rare_position_estimates"];
    assert_eq!(old["318"]["2"]["probability"], 0.);
    let cell = &new["318"]["2"];
    let probability = cell["probability"].as_f64().unwrap();
    assert!((probability / 5.1079036442014455e-24 - 1.).abs() < 0.01);
    assert_eq!(cell["design"], "matched_point_pool_joint_caps");
    assert_eq!(cell["conditional_samples"], 5000);
    assert_eq!(cell["conditional_hits"], 90);
    assert!(candidate["rare_position_estimates"]["318"]["3"]["reachability"] != "undecided");

    for (team, ranks) in old.as_object().unwrap() {
        for (rank, before) in ranks.as_object().unwrap() {
            let after = &new[team][rank];
            if before["probability"].as_f64().unwrap() > 0. {
                assert_eq!(
                    before["probability"], after["probability"],
                    "team={team} rank={rank}"
                );
                assert_eq!(
                    before["std_err"], after["std_err"],
                    "team={team} rank={rank}"
                );
            }
            match before["reachability"].as_str().unwrap_or("") {
                "reachable" => {
                    assert!(
                        after["probability"].as_f64().unwrap() > 0.
                            || matches!(after["reachability"].as_str(), Some("reachable")),
                        "lost reachable team={team} rank={rank}"
                    );
                }
                label if label.starts_with("impossible") => {
                    assert_eq!(before["reachability"], after["reachability"])
                }
                _ => {}
            }
        }
    }
    assert_eq!(baseline["game_importance"], candidate["game_importance"]);
    assert!(!disabled_logs
        .iter()
        .any(|v| v["event"] == "rust_odds_joint_caps"));
    let joint = logs
        .iter()
        .find(|v| {
            v["event"] == "rust_odds_joint_caps"
                && v["setup"]["team"] == 318
                && v["setup"]["rank"] == 3
        })
        .unwrap();
    assert_eq!(joint["accepted"], true);
    assert_eq!(joint["seed"], 1790832522033172000_i64);
    assert_eq!(joint["joint_draws"], 5000);
    assert_eq!(joint["check_draws"], 2000);
    assert_eq!(joint["ordinary_draws"], 10000);
    assert!(joint["setup_ms"].as_f64().unwrap() >= 0.);
    let stages: Vec<_> = logs.iter().filter_map(|v| v["stage"].as_str()).collect();
    let publish = stages
        .iter()
        .position(|v| *v == "search.joint_caps_publish")
        .unwrap();
    assert!(stages.iter().position(|v| *v == "search.domains").unwrap() < publish);
    assert!(
        publish
            < stages
                .iter()
                .position(|v| *v == "search.reconcile")
                .unwrap()
    );
}

#[test]
fn broad_replacement_preserves_existing_estimates_and_confirms_rare_gains() {
    let dir = OutputDir(
        std::env::temp_dir().join(format!("golaberto-broad-test-{}", std::process::id())),
    );
    fs::create_dir(&dir.0).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs");
    for (request, seed) in [
        (
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/group-16653-2d1c1d6f.json"),
            808,
        ),
        (root.join("group-16653-71d4fea8.json"), 818),
    ] {
        let run = |broad: bool| {
            let output = dir.0.join(format!("{seed}-{broad}.json"));
            let mut command = Command::new(env!("CARGO_BIN_EXE_golaberto-odds"));
            for (key, _) in std::env::vars_os() {
                let name = key.to_string_lossy();
                if name.starts_with("RUST_ODDS_") || name.starts_with("RARE_POSITION_") {
                    command.env_remove(key);
                }
            }
            command.env("RUST_ODDS_LOG", "1");
            command.env("RUST_ODDS_RARE_TAIL", "0");
            // Isolate the previously shipped broad allocation from the new
            // funded propagated candidate tested separately above.
            command.env("RUST_ODDS_JOINT_PROPAGATION", "0");
            if !broad {
                command.env("RUST_ODDS_JOINT_CAP_BROAD", "0");
            }
            let result = command
                .arg("estimate")
                .arg(&request)
                .arg(&output)
                .arg(seed.to_string())
                .arg("4")
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let response: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
            let events: Vec<Value> = String::from_utf8(result.stderr)
                .unwrap()
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect();
            (response, events)
        };
        let (before, _) = run(false);
        let (after, logs) = run(true);
        let old = &before["rare_position_estimates"];
        let new = &after["rare_position_estimates"];
        assert_valid_rank_matrix(&before);
        assert_valid_rank_matrix(&after);
        let old_positive_count = old
            .as_object()
            .unwrap()
            .values()
            .flat_map(|ranks| ranks.as_object().unwrap().values())
            .filter(|cell| cell["probability"].as_f64().unwrap() > 0.0)
            .count();
        assert!(old_positive_count > 0, "baseline positive preservation set");

        let accepted_evidence = logs
            .iter()
            .filter(|event| {
                event["event"] == "rust_odds_joint_caps"
                    && event["setup"]["mode"] == "target_total_mixture"
                    && event["accepted"] == true
            })
            .collect::<Vec<_>>();
        assert!(
            !accepted_evidence.is_empty(),
            "accepted joint evidence for seed={seed}"
        );
        let mut newly_gained = Vec::new();
        for evidence in &accepted_evidence {
            assert_eq!(evidence["joint_draws"], 1_500);
            assert_eq!(evidence["check_draws"], 1_500);
            assert!(evidence["hits"].as_u64().unwrap() >= 30);
            assert!(evidence["check_hits"].as_u64().unwrap() >= 30);
            assert!(evidence["ess"].as_f64().unwrap() >= 3.0);
            assert!(evidence["check_ess"].as_f64().unwrap() >= 3.0);
            assert!(evidence["probability"].as_f64().unwrap() > 0.0);
            assert!(evidence["check_probability"].as_f64().unwrap() > 0.0);
            assert!(evidence["relative_se"].as_f64().unwrap() <= 0.75);
            let pair_ratio = evidence["probability"].as_f64().unwrap()
                / evidence["check_probability"].as_f64().unwrap();
            assert!((0.2..=5.0).contains(&pair_ratio));

            let team = evidence["setup"]["team"].as_u64().unwrap().to_string();
            let rank = (evidence["setup"]["rank"].as_u64().unwrap() - 1).to_string();
            let old_probability = old[&team][&rank]["probability"].as_f64().unwrap();
            let new_probability = new[&team][&rank]["probability"].as_f64().unwrap();
            if old_probability == 0.0 {
                assert!(
                    new_probability > 0.0,
                    "accepted zero cell not published: {team}/{rank}"
                );
                assert_eq!(new[&team][&rank]["design"], "matched_point_pool_joint_caps");
                assert_eq!(new[&team][&rank]["reachability"], "reachable");
                let evidence_probability = evidence["probability"].as_f64().unwrap();
                assert!(
                    (new_probability / evidence_probability - 1.0).abs() < 1e-3,
                    "team={team} rank={rank}: response={new_probability:e}, evidence={evidence_probability:e}"
                );
                newly_gained.push((team, rank));
            }
        }
        assert!(
            !newly_gained.is_empty(),
            "broad replacement should publish at least one independently accepted new cell for seed={seed}"
        );
        for (team, ranks) in new.as_object().unwrap() {
            for (rank, cell) in ranks.as_object().unwrap() {
                if old[team][rank]["probability"].as_f64().unwrap() == 0.0
                    && cell["probability"].as_f64().unwrap() > 0.0
                {
                    assert!(
                        newly_gained.contains(&(team.clone(), rank.clone())),
                        "unattributed new cell team={team} rank={rank}"
                    );
                }
            }
        }
        for (team, ranks) in old.as_object().unwrap() {
            for (rank, e) in ranks.as_object().unwrap() {
                let n = &new[team][rank];
                let p = e["probability"].as_f64().unwrap();
                let q = n["probability"].as_f64().unwrap();
                if p > 0. {
                    assert!(
                        q > 0.,
                        "lost {request:?} seed={seed} team={team} rank={rank}"
                    );
                    let uncertainty =
                        e["std_err"].as_f64().unwrap() + n["std_err"].as_f64().unwrap();
                    assert!(
                        (p - q).abs() <= (5. * uncertainty).max(p * 0.2),
                        "changed estimate {request:?} team={team} rank={rank}: {p} -> {q}"
                    );
                }
                let label = e["reachability"].as_str().unwrap_or("");
                if label.starts_with("impossible") {
                    assert_eq!(e["reachability"], n["reachability"]);
                } else if matches!(label, "reachable") {
                    assert!(q > 0. || matches!(n["reachability"].as_str(), Some("reachable")));
                }
            }
        }
        assert_eq!(before["game_importance"], after["game_importance"]);
        let joint = logs
            .iter()
            .filter(|e| {
                e["event"] == "rust_odds_joint_caps" && e["setup"]["mode"] == "target_total_mixture"
            })
            .collect::<Vec<_>>();
        assert!(!joint.is_empty());
        assert!(joint.iter().any(|e| e["accepted"] == true));
        assert!(joint.iter().any(|e| e["ordinary_draws"] == 35000));
        assert!(joint.iter().any(|e| e["ordinary_draws"] == 43000));
        for e in joint {
            assert!(matches!(e["ordinary_draws"].as_u64(), Some(35000 | 43000)));
            assert_eq!(e["joint_draws"], 1500);
            assert_eq!(e["check_draws"], 1500);
        }
        let start = logs
            .iter()
            .find(|e| e["event"] == "rust_odds_start")
            .unwrap();
        assert_eq!(start["joint_cap_broad"], true);
    }
}
