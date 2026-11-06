use golaberto_odds::{
    goal_completion::{self, Adjustment, Certificate},
    model::{Key, Model, Request},
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

fn run_cli(seed: i64, workers: usize, output: &Path) -> (serde_json::Value, Vec<u8>) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_golaberto-odds"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("RUST_ODDS_")
            || key.to_string_lossy().starts_with("RARE_POSITION_")
        {
            command.env_remove(key);
        }
    }
    command
        .env("RUST_ODDS_LOG", if workers == 4 { "1" } else { "0" })
        .arg("estimate")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/group-17046-2026-11-05.json"),
        )
        .arg(output)
        .arg(seed.to_string())
        .arg(workers.to_string());
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    (
        serde_json::from_slice(&fs::read(output).unwrap()).unwrap(),
        result.stderr,
    )
}

fn packed_witness_log(stderr: &[u8]) -> serde_json::Value {
    String::from_utf8_lossy(stderr)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["stage"] == "search.packed_witness")
        .expect("packed witness stage log")
}

fn snapshot_request() -> (serde_json::Value, Request) {
    let raw: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/group-17046-2026-11-05.json")).unwrap();
    let request: Request = serde_json::from_value(raw.clone()).unwrap();
    (raw, request)
}

#[test]
fn november_5_group_snapshot_has_consistent_identity_and_schedule() {
    let (raw, request) = snapshot_request();
    let model = Model::new(request.clone()).unwrap();

    assert_eq!(request.id, 17046);
    assert_eq!(raw["phase_id"], 4544);
    assert_eq!(raw["phase"]["championship_id"], 1532);
    assert_eq!(raw["phase"]["championship"]["name"], "Europa League");
    assert_eq!(request.phase.sort, "pt,gd,gf,g_away,w,name");
    assert_eq!(request.phase.bonus_points, 0);
    assert_eq!(request.phase.bonus_points_threshold, 0);
    assert_eq!(request.phase.championship.point_win, 3);
    assert_eq!(request.phase.championship.point_draw, 1);
    assert_eq!(request.phase.championship.point_loss, 0);
    assert_eq!(
        &model.keys[..5],
        &[Key::Pt, Key::Gd, Key::Gf, Key::Away, Key::W]
    );

    assert_eq!(model.n, 36);
    assert_eq!(request.team_groups.len(), 36);
    assert_eq!(request.games.len(), 144);
    assert_eq!(request.games.iter().filter(|game| game.played).count(), 72);
    assert_eq!(request.games.iter().filter(|game| !game.played).count(), 72);
    assert_eq!(model.fixtures.len(), 72);

    let team_ids: HashSet<_> = request
        .team_groups
        .iter()
        .map(|team| team.team_id)
        .collect();
    assert_eq!(team_ids.len(), 36, "team IDs must be unique");
    assert_eq!(
        model.ids,
        request
            .team_groups
            .iter()
            .map(|team| team.team_id)
            .collect::<Vec<_>>()
    );
    let game_ids: HashSet<_> = request.games.iter().map(|game| game.id).collect();
    assert_eq!(
        game_ids.len(),
        request.games.len(),
        "game IDs must be unique"
    );

    let mut appearances = vec![0; model.n];
    let mut played = vec![0; model.n];
    let mut future = vec![0; model.n];
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
        if game.played {
            played[model.indices[&game.home_id]] += 1;
            played[model.indices[&game.away_id]] += 1;
        } else {
            future[model.indices[&game.home_id]] += 1;
            future[model.indices[&game.away_id]] += 1;
            assert!(
                game.home_power.is_finite() && game.home_power > 0.0,
                "game {} home power",
                game.id
            );
            assert!(
                game.away_power.is_finite() && game.away_power > 0.0,
                "game {} away power",
                game.id
            );
        }
    }
    assert!(appearances.iter().all(|&count| count == 8));
    assert!(played.iter().all(|&count| count == 4));
    assert!(future.iter().all(|&count| count == 4));
}

#[test]
fn november_5_group_packed_witness_certificates_verify() {
    let (_, request) = snapshot_request();
    let model = Model::new(request).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/group-17046-2026-11-05-witnesses.json"
    ))
    .unwrap();
    assert_eq!(
        fixture["provenance"]["request_sha256"],
        raw_request_sha256()
    );

    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 4);
    let mut identities = HashSet::new();
    for case in cases {
        let team_id = case["team_id"].as_i64().unwrap() as i32;
        let position = case["position"].as_u64().unwrap() as usize;
        assert!(
            identities.insert((team_id, position)),
            "duplicate witness case"
        );
        let encoded = &case["certificate"];
        let certificate = Certificate {
            outcomes: serde_json::from_value(encoded["outcomes"].clone()).unwrap(),
            adjustments: encoded["adjustments"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| Adjustment {
                    fixture: value["fixture"].as_u64().unwrap() as usize,
                    margin: value["margin"].as_i64().unwrap() as i32,
                    equal_goals: value["equal_goals"].as_i64().unwrap() as i32,
                })
                .collect(),
            ranks: serde_json::from_value(encoded["ranks"].clone()).unwrap(),
        };
        assert_eq!(certificate.outcomes.len(), 72);
        assert_eq!(certificate.ranks.len(), 36);
        let target = model.indices[&team_id];
        assert_eq!(certificate.ranks[target], position - 1);
        assert!(goal_completion::verify(
            &model,
            &certificate,
            Cell {
                team: target,
                rank: position - 1,
            }
        ));
    }
    assert_eq!(
        identities,
        HashSet::from([(3169, 30), (402, 30), (402, 31), (2436, 6)])
    );
}

fn raw_request_sha256() -> String {
    use sha2::Digest;
    use std::fmt::Write;
    let bytes = include_bytes!("fixtures/group-17046-2026-11-05.json");
    let digest = sha2::Sha256::digest(bytes);
    digest.iter().fold(String::new(), |mut output, byte| {
        write!(&mut output, "{byte:02x}").unwrap();
        output
    })
}

#[test]
fn november_5_default_production_cli_resolves_the_complete_rank_grid() {
    let output_dir = OutputDir(std::env::temp_dir().join(format!(
        "group-17046-default-{}-{}",
        std::process::id(),
        NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir_all(&output_dir.0).unwrap();

    let cases = [(808, 4), (809, 4), (1669, 4), (2293, 4), (808, 1)];
    let mut results = Vec::new();
    for (seed, workers) in cases {
        let (result, stderr) = run_cli(
            seed,
            workers,
            &output_dir
                .0
                .join(format!("seed-{seed}-workers-{workers}.json")),
        );
        if workers == 4 {
            let packed = packed_witness_log(&stderr);
            let nodes = packed["nodes"].as_u64().expect("packed witness node count");
            assert!(nodes > 0 && nodes <= 6_000, "packed witness nodes: {nodes}");
            assert!(packed["candidates"].as_u64().unwrap() > 0, "{packed}");
            assert!(packed["cohorts"].as_u64().unwrap() > 0, "{packed}");
        }
        results.push(result);
    }

    let request = snapshot_request().0;
    let team_ids = request["team_groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|team| team["team_id"].as_i64().unwrap())
        .collect::<Vec<_>>();
    for (case_index, result) in results.iter().enumerate() {
        let estimates = result["rare_position_estimates"].as_object().unwrap();
        assert_eq!(estimates.len(), 36);
        let mut reachable = 0;
        let mut impossible = 0;
        let mut undecided = 0;
        for team_id in &team_ids {
            let row = estimates[&team_id.to_string()].as_object().unwrap();
            assert_eq!(row.len(), 36, "team {team_id}");
            for rank in 0..36 {
                let estimate = &row[&rank.to_string()];
                let label = estimate["reachability"].as_str().unwrap();
                if label.starts_with("reachable") {
                    reachable += 1;
                } else if label.starts_with("impossible") {
                    impossible += 1;
                } else if label == "undecided" {
                    undecided += 1;
                } else {
                    panic!("unexpected reachability label {label}");
                }
                assert!(estimate["probability"].as_f64().unwrap().is_finite());
            }
        }
        assert_eq!(reachable, 1_286, "case {case_index}");
        assert_eq!(impossible, 10, "case {case_index}");
        assert_eq!(undecided, 0, "case {case_index}");
        for (team, rank) in [(3169, 30), (402, 30), (402, 31), (2436, 6)] {
            assert_eq!(
                estimates[&team.to_string()][&(rank - 1).to_string()]["reachability"],
                "reachable",
                "team {team}, position {rank}, case {case_index}"
            );
        }
    }
    for key in ["team_odds", "rare_position_estimates", "game_importance"] {
        assert_eq!(
            results[0][key], results[4][key],
            "workers must not change {key}"
        );
    }
}
