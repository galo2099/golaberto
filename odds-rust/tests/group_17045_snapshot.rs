use golaberto_odds::{
    model::{Key, Model, Request},
    rank_proof::{Limits, Options, RankProof},
    search::Cell,
};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_OUTPUT: AtomicUsize = AtomicUsize::new(0);

struct OutputDir(PathBuf);

impl Drop for OutputDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_cli(enabled: bool, output: &Path) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_golaberto-odds"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("RUST_ODDS_")
            || key.to_string_lossy().starts_with("RARE_POSITION_")
        {
            command.env_remove(key);
        }
    }
    command
        .env("RUST_ODDS_LOG", "0")
        .env("RUST_ODDS_PACKED_WITNESS", if enabled { "1" } else { "0" })
        .arg("estimate")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/group-17045-2026-11-04.json"),
        )
        .arg(output)
        .arg("808")
        .arg("4");
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&fs::read(output).unwrap()).unwrap()
}

fn snapshot_request() -> Request {
    serde_json::from_str(include_str!("fixtures/group-17045-2026-11-04.json")).unwrap()
}

#[test]
fn november_4_group_snapshot_has_consistent_identity_and_schedule() {
    let request = snapshot_request();
    let model = Model::new(request.clone()).unwrap();

    assert_eq!(request.id, 17045);
    let raw: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/group-17045-2026-11-04.json")).unwrap();
    assert_eq!(raw["name"], "Only");
    assert_eq!(request.phase.sort, "pt,gd,gf,g_away,w,name");
    assert_eq!(request.phase.championship.point_win, 3);
    assert_eq!(request.phase.championship.point_draw, 1);
    assert_eq!(request.phase.championship.point_loss, 0);
    assert_eq!(
        &model.keys[..5],
        &[Key::Pt, Key::Gd, Key::Gf, Key::Away, Key::W]
    );

    assert_eq!(model.n, 36);
    assert_eq!(
        model.ids,
        request
            .team_groups
            .iter()
            .map(|team| team.team_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(request.games.len(), 144);
    assert_eq!(request.games.iter().filter(|game| game.played).count(), 72);
    assert_eq!(model.fixtures.len(), 72);

    let team_ids: HashSet<_> = model.ids.iter().copied().collect();
    let game_ids: HashSet<_> = request.games.iter().map(|game| game.id).collect();
    assert_eq!(team_ids.len(), 36);
    assert_eq!(game_ids.len(), request.games.len());

    let mut appearances = vec![0; model.n];
    for game in &request.games {
        assert!(
            team_ids.contains(&game.home_id),
            "home team {}",
            game.home_id
        );
        assert!(
            team_ids.contains(&game.away_id),
            "away team {}",
            game.away_id
        );
        assert_ne!(game.home_id, game.away_id, "game {}", game.id);
        appearances[model.indices[&game.home_id]] += 1;
        appearances[model.indices[&game.away_id]] += 1;
    }
    assert!(appearances.iter().all(|&count| count == 8));
}

#[test]
fn wider_cohort_allowance_proves_top_teams_cannot_finish_33rd_or_34th() {
    let model = Model::new(snapshot_request()).unwrap();
    let proof = RankProof::new(&model).unwrap();

    // The default 512-cohort preflight skips these; this records their reference impossibility.
    for team_id in [401, 855] {
        let team = model.indices[&team_id];
        for rank in [32, 33] {
            let report = proof.prove_floor_with_limits(
                Cell { team, rank },
                rank,
                1_000,
                Options::default(),
                None,
                Limits {
                    max_cohorts: 100_000,
                    max_depth: 512,
                },
            );
            assert!(report.impossible, "team {team_id}, rank {rank}");
            assert!(report.stats.nodes <= 1_000, "team {team_id}, rank {rank}");
        }
    }
}

#[test]
fn packed_witness_reduces_undecided_cells_without_changing_estimates() {
    let output_dir = OutputDir(std::env::temp_dir().join(format!(
        "group-17045-witness-{}-{}",
        std::process::id(),
        NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir_all(&output_dir.0).unwrap();
    let baseline = run_cli(false, &output_dir.0.join("baseline.json"));
    let enabled = run_cli(true, &output_dir.0.join("enabled.json"));

    let baseline_odds = baseline["team_odds"].as_object().unwrap();
    let enabled_odds = enabled["team_odds"].as_object().unwrap();
    let baseline_estimates = baseline["rare_position_estimates"].as_object().unwrap();
    let enabled_estimates = enabled["rare_position_estimates"].as_object().unwrap();
    let team_290_rank_32_baseline = &baseline_estimates["290"]["32"];
    let team_290_rank_32_enabled = &enabled_estimates["290"]["32"];
    assert_eq!(team_290_rank_32_baseline["reachability"], "undecided");
    assert_eq!(team_290_rank_32_enabled["reachability"], "reachable");
    assert_eq!(team_290_rank_32_enabled["probability"], 0.0);
    assert_eq!(baseline_estimates["401"]["24"]["reachability"], "undecided");
    assert_eq!(enabled_estimates["401"]["24"]["reachability"], "undecided");
    for team in ["401", "855"] {
        for rank in ["32", "33"] {
            let before = &baseline_estimates[team][rank];
            let after = &enabled_estimates[team][rank];
            assert_eq!(before["reachability"], "undecided");
            assert_eq!(after["reachability"], "impossible");
            assert_eq!(after["probability"], 0.0);
            assert_eq!(after["zero_hit_upper_95"], 0.0);
        }
    }
    assert_eq!(baseline_odds.len(), 36);
    assert_eq!(enabled_odds.len(), 36);
    assert_eq!(baseline_estimates.len(), 36);
    assert_eq!(enabled_estimates.len(), 36);
    assert_eq!(baseline["game_importance"], enabled["game_importance"]);

    let mut normalized_baseline = baseline.clone();
    let mut normalized_enabled = enabled.clone();
    for team in snapshot_request().team_groups {
        let key = team.team_id.to_string();
        let before = baseline_estimates[&key].as_object().unwrap();
        let after = enabled_estimates[&key].as_object().unwrap();
        assert_eq!(before.len(), 36);
        assert_eq!(after.len(), 36);
        for rank in 0..36 {
            let old = &before[&rank.to_string()];
            let new = &after[&rank.to_string()];
            let old_state = old["reachability"].as_str().unwrap();
            let new_state = new["reachability"].as_str().unwrap();
            let refuted = matches!((key.as_str(), rank), ("401" | "855", 32 | 33));
            assert!(
                new_state == old_state
                    || (old_state == "undecided"
                        && (new_state == "reachable" || (refuted && new_state == "impossible"))),
                "team {key}, rank {rank}: {old_state} -> {new_state}"
            );
            normalized_baseline["rare_position_estimates"][&key][&rank.to_string()]
                ["reachability"] = serde_json::Value::Null;
            normalized_enabled["rare_position_estimates"][&key][&rank.to_string()]
                ["reachability"] = serde_json::Value::Null;
            if refuted {
                normalized_baseline["rare_position_estimates"][&key][&rank.to_string()]
                    ["zero_hit_upper_95"] = serde_json::Value::Null;
                normalized_enabled["rare_position_estimates"][&key][&rank.to_string()]
                    ["zero_hit_upper_95"] = serde_json::Value::Null;
            }
        }
        let old_row = baseline_odds[&key]["Pos"].as_array().unwrap();
        let new_row = enabled_odds[&key]["Pos"].as_array().unwrap();
        assert_eq!(old_row, new_row);
        assert_eq!(old_row.len(), 36);
        assert!(old_row
            .iter()
            .all(|value| value.as_f64().unwrap().is_finite()));
        assert!((old_row.iter().map(|v| v.as_f64().unwrap()).sum::<f64>() - 100.).abs() < 1e-12);
    }
    for rank in 0..36 {
        let total: f64 = enabled_odds
            .values()
            .map(|team| team["Pos"][rank].as_f64().unwrap())
            .sum();
        assert!((total - 100.).abs() < 1e-12, "rank {rank}: {total}");
    }
    assert_eq!(normalized_baseline, normalized_enabled);
}
