use golaberto_odds::{
    api,
    joint_caps::propagated::lazy::{BranchStrata, LazyJoint},
    model::{Campaign, Model, Request},
    rng::Rng,
    search::Cell,
    target_limits,
};
use std::collections::{HashMap, HashSet};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::Value;

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

fn run_cli(
    request: &Path,
    output: &Path,
    seed: i64,
    workers: usize,
    logging: &str,
) -> (Vec<u8>, Vec<Value>) {
    let result = clean_command()
        .env("RUST_ODDS_LOG", logging)
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

fn fixture_request() -> Request {
    serde_json::from_str(include_str!("fixtures/group-17045-2026-10-20.json")).unwrap()
}

fn validate_cli_matrix(bytes: &[u8]) -> Value {
    let response: Value = serde_json::from_slice(bytes).unwrap();
    let odds = response["team_odds"].as_object().unwrap();
    let estimates = response["rare_position_estimates"].as_object().unwrap();
    assert_eq!(odds.len(), 36);
    assert_eq!(estimates.len(), 36);

    for team in fixture_request().team_groups {
        let team_id = team.team_id;
        let key = team_id.to_string();
        let positions = odds[&key]["Pos"].as_array().unwrap();
        assert_eq!(positions.len(), 36, "team {team_id}");
        let row: Vec<f64> = positions
            .iter()
            .map(|position| position.as_f64().unwrap())
            .collect();
        assert!(row.iter().all(|probability| probability.is_finite()));
        assert!((row.iter().sum::<f64>() - 100.).abs() < 1e-12);

        let ranks = estimates[&key].as_object().unwrap();
        assert_eq!(ranks.len(), 36, "team {team_id}");
        for rank in 0..36 {
            let estimate = &ranks[&rank.to_string()];
            assert_eq!(
                estimate["reachability"], "reachable",
                "team {team_id}, rank {rank}"
            );
            let probability = estimate["probability"].as_f64().unwrap();
            assert!(
                probability.is_finite() && probability >= 0.,
                "team {team_id}, rank {rank}: {probability}"
            );
            if probability == 0. {
                assert!(!estimate["meets_precision_goal"].as_bool().unwrap());
            }
        }
    }

    for rank in 0..36 {
        let total: f64 = odds
            .values()
            .map(|team| team["Pos"][rank].as_f64().unwrap())
            .sum();
        assert!((total - 100.).abs() < 1e-12, "rank {}: {total}", rank + 1);
    }

    let positive = estimates
        .values()
        .flat_map(|ranks| ranks.as_object().unwrap().values())
        .filter(|estimate| estimate["probability"].as_f64().unwrap() > 0.)
        .count();
    assert!(positive >= 1290, "positive cells: {positive}");
    response
}

fn assert_confirmed_pair_quality(event: &Value) {
    assert!(event["main_hits"].as_u64().unwrap() >= 30);
    assert!(event["check_hits"].as_u64().unwrap() >= 30);
    assert!(event["main_ess"].as_f64().unwrap() >= 4.);
    assert!(event["check_ess"].as_f64().unwrap() >= 3.);
    let main_probability = event
        .get("main_probability")
        .or_else(|| event.get("probability"))
        .unwrap()
        .as_f64()
        .unwrap();
    let check_probability = event["check_probability"].as_f64().unwrap();
    assert!(main_probability > 0. && check_probability > 0.);
    let ratio = main_probability / check_probability;
    assert!(
        (0.2..=5.).contains(&ratio),
        "main/check probability ratio: {ratio}"
    );
}

#[test]
fn fixture_identity_and_league_phase_schedule_are_consistent() {
    let raw: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/group-17045-2026-10-20.json")).unwrap();
    let request = fixture_request();
    let model = Model::new(request.clone()).unwrap();

    assert_eq!(request.id, 17045);
    assert_eq!(raw["phase_id"], 4543);
    assert_eq!(raw["phase"]["championship_id"], 1531);
    assert_eq!(request.phase.sort, "pt,gd,gf,g_away,w,name");
    assert_eq!(request.team_groups.len(), 36);
    assert_eq!(request.games.len(), 144);
    assert_eq!(request.games.iter().filter(|game| game.played).count(), 45);
    assert_eq!(request.games.iter().filter(|game| !game.played).count(), 99);
    assert_eq!(request.zones.len(), 5);

    let team_ids: HashSet<_> = request
        .team_groups
        .iter()
        .map(|team| team.team_id)
        .collect();
    let model_ids: HashSet<_> = model.ids.iter().copied().collect();
    assert_eq!(model.n, 36);
    assert_eq!(model_ids, team_ids);

    let scheduled_ids: HashSet<_> = request
        .games
        .iter()
        .flat_map(|game| [game.home_id, game.away_id])
        .collect();
    assert_eq!(scheduled_ids, team_ids);

    let mut total_games = HashMap::new();
    let mut remaining_games = HashMap::new();
    for game in &request.games {
        *total_games.entry(game.home_id).or_insert(0) += 1;
        *total_games.entry(game.away_id).or_insert(0) += 1;
        if !game.played {
            *remaining_games.entry(game.home_id).or_insert(0) += 1;
            *remaining_games.entry(game.away_id).or_insert(0) += 1;
        }
    }
    for team_id in team_ids {
        assert_eq!(total_games.get(&team_id), Some(&8), "team {team_id}");
        assert!(
            matches!(remaining_games.get(&team_id), Some(5 | 6)),
            "team {team_id} has {:?} remaining games",
            remaining_games.get(&team_id)
        );
    }
}

#[test]
fn large_group_tail_support_is_cost_sensitive_and_bounded() {
    let request = fixture_request();
    let model = Model::new(request).unwrap();
    let cell = Cell {
        team: model.indices[&36],
        rank: 35,
    };

    assert_eq!(model.n, 36);
    // Bootstrap admission is cost-sensitive, so construction may be declined
    // even when the league size itself is supported.
    let _lazy = LazyJoint::new(&model, cell, 808, &[], 32);

    let (overflow_roots, root_work) = BranchStrata::overflow_root_count(&model, cell, 808, 32);
    assert!(root_work > 0);
    if let Err(error) = overflow_roots {
        assert_ne!(error, "unsupported tree model");
        assert!(
            error.contains("complete tree target-path budget exhausted"),
            "unexpected root enumeration error: {error}"
        );
    }

    let limits = target_limits::certify(&model, cell, target_limits::NODE_LIMIT);
    assert!(limits.nodes <= target_limits::NODE_LIMIT);
    if let (Some(lower), Some(upper)) = (limits.lower, limits.upper) {
        assert!(lower <= upper);
    }
}

#[test]
fn production_standings_keep_goal_keys_before_away_and_wins() {
    let request: Request = serde_json::from_value(serde_json::json!({
        "id": 1,
        "phase": {
            "sort": "pt,gd,gf,g_away,w,name",
            "championship": {"point_win": 3, "point_draw": 1, "point_loss": 0}
        },
        "games": [],
        "team_groups": [
            {"team_id": 10}, {"team_id": 11},
            {"team_id": 20}, {"team_id": 21},
            {"team_id": 30}, {"team_id": 31}
        ]
    }))
    .unwrap();
    let model = Model::new(request).unwrap();
    let mut campaigns = vec![Campaign::default(); model.n];
    let index = |id| model.indices[&id];

    // Equal points, goal difference, and goals for: away goals beat wins.
    campaigns[index(10)] = Campaign {
        points: 6,
        wins: 1,
        gf: 4,
        ga: 2,
        away: 3,
        ..Campaign::default()
    };
    campaigns[index(11)] = Campaign {
        points: 6,
        wins: 2,
        gf: 4,
        ga: 2,
        away: 1,
        ..Campaign::default()
    };

    // Equal points and goals for: goal difference beats the opposing win total.
    campaigns[index(20)] = Campaign {
        points: 3,
        wins: 1,
        gf: 4,
        ga: 1,
        ..Campaign::default()
    };
    campaigns[index(21)] = Campaign {
        points: 3,
        wins: 2,
        gf: 4,
        ga: 2,
        ..Campaign::default()
    };

    // With all earlier keys tied, wins decides the order.
    campaigns[index(30)] = Campaign {
        points: 0,
        wins: 2,
        gf: 2,
        ga: 2,
        away: 1,
        ..Campaign::default()
    };
    campaigns[index(31)] = Campaign {
        points: 0,
        wins: 1,
        gf: 2,
        ga: 2,
        away: 1,
        ..Campaign::default()
    };

    let mut order = vec![0; model.n];
    model.standings(
        &mut order,
        &campaigns,
        &model.empty_scores(),
        &mut Rng::new(808),
    );
    let ids: Vec<_> = order.iter().map(|&team| model.ids[team]).collect();
    assert!(ids.iter().position(|&id| id == 10) < ids.iter().position(|&id| id == 11));
    assert!(ids.iter().position(|&id| id == 20) < ids.iter().position(|&id| id == 21));
    assert!(ids.iter().position(|&id| id == 30) < ids.iter().position(|&id| id == 31));
}

#[test]
fn seed_808_returns_a_complete_finite_reachable_matrix() {
    let response = api::calculate(fixture_request(), 808, 4, 100).unwrap().0;

    assert_eq!(response.team_odds.len(), 36);
    assert_eq!(response.rare_position_estimates.len(), 36);
    for (&team_id, odds) in &response.team_odds {
        assert_eq!(odds.pos.len(), 36, "team {team_id}");
        assert!(odds.pos.iter().all(|probability| probability.is_finite()));
        assert!((odds.pos.iter().sum::<f64>() - 100.).abs() < 1e-12);
    }
    for rank in 0..36 {
        let total: f64 = response.team_odds.values().map(|odds| odds.pos[rank]).sum();
        assert!((total - 100.).abs() < 1e-12, "rank {}: {total}", rank + 1);
    }

    let mut positive_cells = 0;
    for (&team_id, ranks) in &response.rare_position_estimates {
        assert_eq!(ranks.len(), 36, "team {team_id}");
        for (&rank, estimate) in ranks {
            assert_eq!(
                estimate.reachability, "reachable",
                "team {team_id}, rank {rank}"
            );
            assert!(
                estimate.probability.is_finite() && estimate.probability >= 0.,
                "team {team_id}, rank {rank}: {}",
                estimate.probability
            );
            if estimate.probability == 0. {
                assert!(
                    !estimate.meets_precision_goal,
                    "team {team_id}, rank {rank}"
                );
            } else {
                positive_cells += 1;
            }
        }
    }
    // Keep the previously established positive-cell coverage as a floor while
    // allowing new support to recover additional cells.
    assert!(positive_cells >= 1290, "positive cells: {positive_cells}");
}

#[test]
fn cli_recovers_group_17045_tail_cells_with_audited_costs_and_stable_output() {
    let request = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/group-17045-2026-10-20.json");
    let dir = OutputDir(
        std::env::temp_dir().join(format!("golaberto-group-17045-cli-{}", std::process::id())),
    );
    fs::create_dir_all(&dir.0).unwrap();

    let (seed_808_bytes, seed_808_logs) = run_cli(
        &request,
        &dir.0.join("seed-808-workers-4.json"),
        808,
        4,
        "1",
    );
    let seed_808 = validate_cli_matrix(&seed_808_bytes);
    let (seed_809_bytes, seed_809_logs) = run_cli(
        &request,
        &dir.0.join("seed-809-workers-4.json"),
        809,
        4,
        "1",
    );
    let seed_809 = validate_cli_matrix(&seed_809_bytes);
    let (seed_808_single_worker, _) = run_cli(
        &request,
        &dir.0.join("seed-808-workers-1.json"),
        808,
        1,
        "0",
    );
    assert_eq!(
        seed_808_bytes, seed_808_single_worker,
        "seed 808 response changed with worker count or logging"
    );

    for (seed, response) in [(808, &seed_808), (809, &seed_809)] {
        for team in ["36", "855"] {
            let estimate = &response["rare_position_estimates"][team]["35"];
            let probability = estimate["probability"].as_f64().unwrap();
            let relative_se = estimate["relative_se"].as_f64().unwrap();
            let ess = estimate["ess"].as_f64().unwrap();
            assert!(probability > 0., "seed {seed}, team {team}, rank 36");
            assert!(
                relative_se <= 0.6,
                "seed {seed}, team {team}: {relative_se}"
            );
            assert!(ess >= 4., "seed {seed}, team {team}: {ess}");
        }
    }

    for (seed, logs) in [(808, &seed_808_logs), (809, &seed_809_logs)] {
        let start = logs
            .iter()
            .find(|event| event["event"] == "rust_odds_start")
            .unwrap();
        assert_eq!(start["seed"], seed);
        assert_eq!(start["workers"], 4);
        assert_eq!(start["scout_samples"], 20000);

        let overflow = logs
            .iter()
            .find(|event| event["event"] == "rust_odds_target_overflow_tree_summary")
            .unwrap();
        let bank_before = overflow["bank_free_before"].as_u64().unwrap();
        let actual_work = overflow["actual_work"].as_u64().unwrap();
        let bank_after = overflow["bank_free_after"].as_u64().unwrap();
        assert!(
            bank_before >= actual_work,
            "seed {seed}: target-tree bank overdrawn"
        );
        assert_eq!(bank_after, bank_before - actual_work, "seed {seed}");
        assert_eq!(overflow["branch_draw_audit_valid"], true);
        for setup in overflow["setup_stages"].as_array().unwrap() {
            if setup["stage"] == "root_count" {
                if let Some(error) = setup["error"].as_str() {
                    assert!(
                        !error.contains("unsupported tree model"),
                        "seed {seed}: {error}"
                    );
                }
                assert!(
                    setup["actual"].as_u64().unwrap() <= setup["grant"].as_u64().unwrap(),
                    "seed {seed}: root-count setup exceeded its grant"
                );
            }
        }
        for pair in overflow["sampling_stages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["stage"] == "final_pair" && event["accepted"] == true)
        {
            assert!(pair["total_draws"].as_u64().unwrap() > 0);
            assert!(pair["main_hits"].as_u64().unwrap() >= 30);
            assert!(pair["check_hits"].as_u64().unwrap() >= 30);
            assert!(pair["main_ess"].as_f64().unwrap() >= 4.);
            assert!(pair["check_ess"].as_f64().unwrap() >= 3.);
            assert!(pair["main_max_share"].as_f64().unwrap() <= 0.35);
            assert_confirmed_pair_quality(pair);
        }

        for event in logs.iter().filter(|event| {
            event["event"] == "rust_odds_rare_tail_confirm_more" && event["accepted"] == true
        }) {
            assert_eq!(event["main_only"], false);
            assert!(event["main_draws"].as_u64().unwrap() > 0);
            assert!(event["check_draws"].as_u64().unwrap() > 0);
            assert!(event["main_max_share"].as_f64().unwrap() <= 0.35);
            assert_confirmed_pair_quality(event);
        }
        let confirm_summary = logs
            .iter()
            .find(|event| event["event"] == "rust_odds_rare_tail_confirm_more_summary")
            .unwrap();
        assert!(
            confirm_summary["reserved_work"].as_u64().unwrap()
                <= confirm_summary["work_limit"].as_u64().unwrap()
        );

        for event in logs.iter().filter(|event| {
            event["event"] == "rust_odds_expected_cohorts_pair" && event["accepted"] == true
        }) {
            assert_eq!(event["publication_gate"], true);
            assert_eq!(event["overrun"], false);
            assert!(event["main_draws"].as_u64().unwrap() > 0);
            assert!(event["check_draws"].as_u64().unwrap() > 0);
            assert!(
                event["actual_pair_work"].as_u64().unwrap()
                    <= event["forecast_pair_work"].as_u64().unwrap()
            );
            assert_confirmed_pair_quality(event);
        }
        let cohorts = logs
            .iter()
            .find(|event| event["event"] == "rust_odds_expected_cohorts_summary")
            .unwrap();
        assert!(
            cohorts["actual_work"].as_u64().unwrap()
                <= cohorts["forecast_capacity"].as_u64().unwrap()
        );
        assert_eq!(cohorts["overrun"], false);
        assert_eq!(cohorts["capacity_exceeded"], false);

        let exact_caps = logs
            .iter()
            .find(|event| event["event"] == "rust_odds_exact_caps_summary")
            .unwrap();
        assert!(
            exact_caps["actual_work"].as_u64().unwrap() <= exact_caps["capacity"].as_u64().unwrap()
        );
        assert_eq!(exact_caps["overrun"], false);

        let large_training = logs
            .iter()
            .find(|event| event["event"] == "rust_odds_large_training_summary")
            .unwrap();
        let training_capacity = large_training["capacity"].as_u64().unwrap();
        let training_stage_capacity = large_training["stage_capacity"].as_u64().unwrap();
        let training_actual = large_training["actual_charged"].as_u64().unwrap();
        assert_eq!(large_training["overrun"], false);
        assert_eq!(large_training["worker_independent_admission"], true);
        assert_eq!(large_training["recycled_grants"], true);
        assert!(training_actual <= training_capacity);
        assert!(training_capacity <= training_stage_capacity);

        let jobs = large_training["jobs"].as_array().unwrap();
        assert!(
            !jobs.is_empty(),
            "seed {seed}: expected admitted training jobs"
        );
        {
            let mut job_grants = 0u64;
            let mut job_charged = 0u64;
            let mut setup_charged = 0u64;
            let mut pilot_charged = 0u64;
            let mut setup_refunded = 0u64;
            let mut pilot_refunded = 0u64;
            for job in jobs {
                let grant = job["grant"].as_u64().unwrap();
                let setup_grant = job["setup_grant"].as_u64().unwrap();
                let pilot_grant = job["pilot_grant"].as_u64().unwrap();
                let setup = job["setup_work"].as_u64().unwrap();
                let pilot = job["pilot_work"].as_u64().unwrap();
                let charged = setup + pilot;
                assert!(
                    charged <= grant,
                    "seed {seed}: large-training job exceeded grant"
                );
                assert!(
                    job["constructor"]["work"].as_u64().unwrap() <= setup,
                    "seed {seed}: constructor work exceeds setup charge"
                );
                assert!(setup <= setup_grant, "seed {seed}: setup exceeded grant");
                assert!(pilot <= pilot_grant, "seed {seed}: pilot exceeded grant");
                assert_eq!(setup_grant - setup, job["setup_refund"].as_u64().unwrap());
                assert_eq!(pilot_grant - pilot, job["pilot_refund"].as_u64().unwrap());
                assert_eq!(setup_grant + pilot_grant, grant);
                job_grants += grant;
                job_charged += charged;
                setup_charged += setup;
                pilot_charged += pilot;
                setup_refunded += job["setup_refund"].as_u64().unwrap();
                pilot_refunded += job["pilot_refund"].as_u64().unwrap();
            }
            assert_eq!(
                job_grants,
                large_training["cumulative_quoted_work"].as_u64().unwrap()
            );
            assert_eq!(job_charged, training_actual);
            assert_eq!(job_charged, setup_charged + pilot_charged);
            assert_eq!(
                setup_charged,
                large_training["setup_charged"].as_u64().unwrap()
            );
            assert_eq!(
                pilot_charged,
                large_training["pilot_charged"].as_u64().unwrap()
            );
            assert_eq!(
                setup_refunded + pilot_refunded,
                job_grants - job_charged,
                "seed {seed}: unused setup and pilot grants were not refunded"
            );
            assert!(
                setup_refunded + pilot_refunded > 0,
                "seed {seed}: no unused grant was refunded"
            );
            assert!(job_charged <= training_capacity);
        }

        let rare_tail = logs
            .iter()
            .find(|event| event["event"] == "rust_odds_rare_tail_summary")
            .unwrap();
        let family_stage = &rare_tail["native_family_recording_stage"];
        assert_eq!(family_stage["admission_valid"], true);
        assert_eq!(family_stage["unknown_constructor_failures"], 0);
        assert_eq!(family_stage["setup_audit_extra_work"], 0);
        let audited_training = family_stage["audited_training_work"].as_u64().unwrap();
        let final_operations = family_stage["actual_final_operations"].as_u64().unwrap();
        let reserved_final = family_stage["reserved_final_work"].as_u64().unwrap();
        let native_capacity = family_stage["capacity"].as_u64().unwrap();
        assert_eq!(audited_training, training_actual);
        assert!(audited_training + final_operations <= native_capacity);
        if family_stage["admission_valid"] == true {
            assert!(audited_training + reserved_final <= native_capacity);
        }
    }
}
