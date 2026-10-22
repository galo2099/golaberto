use golaberto_odds::{
    api,
    model::{Model, Request},
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

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

fn fixture_request() -> (Value, Request) {
    let raw: Value =
        serde_json::from_str(include_str!("fixtures/group-17046-2026-10-22.json")).unwrap();
    let request: Request = serde_json::from_value(raw.clone()).unwrap();
    (raw, request)
}

#[test]
fn fixture_identity_schedule_and_model_are_consistent() {
    let (raw, request) = fixture_request();
    let model = Model::new(request.clone()).unwrap();

    assert_eq!(request.id, 17046);
    assert_eq!(raw["phase_id"], 4544);
    assert_eq!(raw["phase"]["championship_id"], 1532);
    assert_eq!(request.phase.sort, "pt,gd,gf,g_away,w,name");
    assert_eq!(request.team_groups.len(), 36);
    assert_eq!(request.games.len(), 144);
    assert_eq!(request.games.iter().filter(|game| game.played).count(), 54);
    assert_eq!(request.games.iter().filter(|game| !game.played).count(), 90);
    assert_eq!(request.zones.len(), 5);

    let team_ids: HashSet<_> = request
        .team_groups
        .iter()
        .map(|team| team.team_id)
        .collect();
    assert_eq!(team_ids.len(), 36, "team IDs must be unique");
    let model_ids: HashSet<_> = model.ids.iter().copied().collect();
    assert_eq!(model.n, 36);
    assert_eq!(model.fixtures.len(), 90);
    assert_eq!(model_ids, team_ids);

    let mut game_ids = HashSet::new();
    let mut game_pairs = HashSet::new();
    let mut total_games = HashMap::new();
    let mut played_games = HashMap::new();
    let mut remaining_games = HashMap::new();
    for game in &request.games {
        assert!(team_ids.contains(&game.home_id));
        assert!(team_ids.contains(&game.away_id));
        assert_ne!(game.home_id, game.away_id);
        assert!(game_ids.insert(game.id), "duplicate game ID {}", game.id);
        let pair = if game.home_id < game.away_id {
            (game.home_id, game.away_id)
        } else {
            (game.away_id, game.home_id)
        };
        assert!(
            game_pairs.insert(pair),
            "duplicate fixture for pair {pair:?}"
        );

        *total_games.entry(game.home_id).or_insert(0) += 1;
        *total_games.entry(game.away_id).or_insert(0) += 1;
        if game.played {
            *played_games.entry(game.home_id).or_insert(0) += 1;
            *played_games.entry(game.away_id).or_insert(0) += 1;
        } else {
            *remaining_games.entry(game.home_id).or_insert(0) += 1;
            *remaining_games.entry(game.away_id).or_insert(0) += 1;
        }
    }
    for team_id in team_ids {
        assert_eq!(total_games.get(&team_id), Some(&8), "team {team_id}");
        assert_eq!(played_games.get(&team_id), Some(&3), "team {team_id}");
        assert_eq!(remaining_games.get(&team_id), Some(&5), "team {team_id}");
    }
}

#[test]
fn seed_808_returns_a_complete_finite_reachable_matrix() {
    let (_, request) = fixture_request();
    // The scout sample count only drives game-importance output; estimation
    // still uses the production pool size.
    let response = api::calculate(request, 808, 4, 100).unwrap().0;

    assert_eq!(response.team_odds.len(), 36);
    assert_eq!(response.rare_position_estimates.len(), 36);
    let mut positive_cells = 0;
    for (&team_id, odds) in &response.team_odds {
        assert_eq!(odds.pos.len(), 36, "team {team_id}");
        assert!(odds
            .pos
            .iter()
            .all(|p| p.is_finite() && (0. ..=100.).contains(p)));
        assert!((odds.pos.iter().sum::<f64>() - 100.).abs() < 1e-9);

        let ranks = &response.rare_position_estimates[&team_id];
        assert_eq!(ranks.len(), 36, "team {team_id}");
        for (&rank, estimate) in ranks {
            let public_probability = odds.pos[rank] / 100.;
            assert_eq!(
                estimate.reachability, "reachable",
                "team {team_id}, rank {rank}"
            );
            assert!(estimate.probability.is_finite() && (0. ..=1.).contains(&estimate.probability));
            assert!(
                (public_probability - estimate.probability).abs() < 1e-12,
                "team {team_id}, rank {rank}: odds={} estimate={}",
                odds.pos[rank],
                estimate.probability
            );
            if estimate.probability == 0. {
                assert!(
                    !estimate.meets_precision_goal,
                    "team {team_id}, rank {rank}"
                );
                assert!(
                    estimate.relative_se.is_none(),
                    "team {team_id}, rank {rank}"
                );
                assert!(
                    estimate.zero_hit_upper_95 > 0.,
                    "team {team_id}, rank {rank}"
                );
            } else {
                positive_cells += 1;
            }
        }
    }

    for rank in 0..36 {
        let total: f64 = response.team_odds.values().map(|odds| odds.pos[rank]).sum();
        assert!((total - 100.).abs() < 1e-9, "rank {}: {total}", rank + 1);
    }
    assert!(positive_cells >= 1273, "positive cells: {positive_cells}");
}

#[test]
fn cli_admission_audit_covers_overflow_seeds_and_recycles_unused_grants() {
    let request = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/group-17046-2026-10-22.json");
    let dir = OutputDir(
        std::env::temp_dir().join(format!("golaberto-group-17046-cli-{}", std::process::id())),
    );
    fs::create_dir_all(&dir.0).unwrap();

    let (seed_808_bytes, seed_808_logs) = run_cli(
        &request,
        &dir.0.join("seed-808-workers-4.json"),
        808,
        4,
        "1",
    );
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

    for seed in 808..=812 {
        let logs = if seed == 808 {
            seed_808_logs.clone()
        } else {
            let (_, logs) = run_cli(
                &request,
                &dir.0.join(format!("seed-{seed}-workers-4.json")),
                seed,
                4,
                "1",
            );
            logs
        };
        let overflow = logs
            .iter()
            .find(|event| event["event"] == "rust_odds_target_overflow_tree_summary")
            .unwrap();
        let bank_before = overflow["bank_free_before"].as_u64().unwrap();
        let actual_work = overflow["actual_work"].as_u64().unwrap();
        let bank_after = overflow["bank_free_after"].as_u64().unwrap();
        assert!(
            actual_work <= bank_before,
            "seed {seed}: overflow exceeded free bank"
        );
        assert_eq!(bank_before - actual_work, bank_after, "seed {seed}");

        let root_setup_stages: Vec<_> = overflow["setup_stages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|stage| stage["stage"] == "root_count")
            .collect();
        assert!(
            !root_setup_stages.is_empty(),
            "seed {seed}: missing root-count setup audit"
        );
        for stage in root_setup_stages {
            assert_eq!(stage["within_grant"], true, "seed {seed}: {stage}");
            assert!(
                stage["actual"].as_u64().unwrap() <= stage["grant"].as_u64().unwrap(),
                "seed {seed}: root-count setup exceeded its grant"
            );
        }
        for stage in overflow["sampling_stages"].as_array().unwrap() {
            if stage["stage"] == "root_count" {
                assert_eq!(stage["within_grant"], true, "seed {seed}: {stage}");
                assert!(
                    stage["actual"].as_u64().unwrap() <= stage["grant"].as_u64().unwrap(),
                    "seed {seed}: root-count sampling exceeded its grant"
                );
            }
        }

        let training = logs
            .iter()
            .find(|event| event["event"] == "rust_odds_large_training_summary")
            .unwrap();
        let capacity = training["capacity"].as_u64().unwrap();
        let stage_capacity = training["stage_capacity"].as_u64().unwrap();
        assert_eq!(training["overrun"], false);
        assert_eq!(training["recycled_grants"], true);
        assert!(capacity <= stage_capacity);
        if seed == 808 {
            assert!(training["completed"].as_u64().unwrap() > 28);
        }
        let jobs = training["jobs"].as_array().unwrap();
        let mut actual_total = 0u64;
        let mut cumulative_grants = 0u64;
        let mut refunded = 0u64;
        for job in jobs {
            let setup_grant = job["setup_grant"].as_u64().unwrap();
            let pilot_grant = job["pilot_grant"].as_u64().unwrap();
            let setup = job["setup_work"].as_u64().unwrap();
            let pilot = job["pilot_work"].as_u64().unwrap();
            assert!(setup <= setup_grant, "seed {seed}: setup over grant");
            assert!(pilot <= pilot_grant, "seed {seed}: pilot over grant");
            assert_eq!(setup_grant - setup, job["setup_refund"].as_u64().unwrap());
            assert_eq!(pilot_grant - pilot, job["pilot_refund"].as_u64().unwrap());
            assert_eq!(setup_grant + pilot_grant, job["grant"].as_u64().unwrap());
            actual_total += setup + pilot;
            cumulative_grants += job["grant"].as_u64().unwrap();
            refunded +=
                job["setup_refund"].as_u64().unwrap() + job["pilot_refund"].as_u64().unwrap();
        }
        assert_eq!(actual_total, training["actual_charged"].as_u64().unwrap());
        assert_eq!(
            actual_total,
            training["setup_charged"].as_u64().unwrap()
                + training["pilot_charged"].as_u64().unwrap()
        );
        assert_eq!(
            cumulative_grants,
            training["cumulative_quoted_work"].as_u64().unwrap()
        );
        assert_eq!(refunded, cumulative_grants - actual_total);
        assert!(refunded > 0, "seed {seed}: no unused grant was refunded");
        assert!(actual_total <= capacity);

        let rare_tail = logs
            .iter()
            .find(|event| event["event"] == "rust_odds_rare_tail_summary")
            .unwrap();
        let native = &rare_tail["native_family_recording_stage"];
        assert_eq!(native["admission_valid"], true);
        let audited_training = native["audited_training_work"].as_u64().unwrap();
        let final_operations = native["actual_final_operations"].as_u64().unwrap();
        let native_capacity = native["capacity"].as_u64().unwrap();
        assert_eq!(audited_training, actual_total);
        assert_eq!(native_capacity, stage_capacity);
        assert!(audited_training + final_operations <= native_capacity);
    }
}
