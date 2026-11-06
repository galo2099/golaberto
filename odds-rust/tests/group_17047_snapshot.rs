use golaberto_odds::{
    goal_completion::{self, Adjustment, Certificate},
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

fn run_cli(workers: usize, output: &Path) -> (serde_json::Value, Vec<u8>) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_golaberto-odds"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("RUST_ODDS_")
            || key.to_string_lossy().starts_with("RARE_POSITION_")
        {
            command.env_remove(key);
        }
    }
    let result = command
        .env("RUST_ODDS_LOG", if workers == 4 { "1" } else { "0" })
        .arg("estimate")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/group-17047-2026-11-06.json"),
        )
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
    (
        serde_json::from_slice(&fs::read(output).unwrap()).unwrap(),
        result.stderr,
    )
}

fn snapshot_request() -> (serde_json::Value, Request) {
    let raw: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/group-17047-2026-11-06.json")).unwrap();
    let request: Request = serde_json::from_value(raw.clone()).unwrap();
    (raw, request)
}

#[test]
fn november_6_group_snapshot_has_consistent_identity_and_schedule() {
    let (raw, request) = snapshot_request();
    let model = Model::new(request.clone()).unwrap();

    assert_eq!(request.id, 17047);
    assert_eq!(raw["phase_id"], 4545);
    assert_eq!(raw["phase"]["championship_id"], 1533);
    assert_eq!(raw["phase"]["championship"]["name"], "Conference League");
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
    assert_eq!(request.games.len(), 108);
    assert_eq!(request.games.iter().filter(|game| game.played).count(), 54);
    assert_eq!(request.games.iter().filter(|game| !game.played).count(), 54);
    assert_eq!(model.fixtures.len(), 54);

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
    assert!(appearances.iter().all(|&count| count == 6));
    assert!(played.iter().all(|&count| count == 3));
    assert!(future.iter().all(|&count| count == 3));
}

#[test]
fn november_6_rank_29_witness_certificates_replay() {
    let request: Request =
        serde_json::from_str(include_str!("fixtures/group-17047-2026-11-06.json")).unwrap();
    let model = Model::new(request).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/group-17047-2026-11-06-witnesses.json"
    ))
    .unwrap();
    assert_eq!(
        fixture["provenance"]["request_sha256"],
        "1defeed977e556966ac928a287e23b492c4d4729a6dbffcd07992d08ad849d70"
    );
    assert_eq!(fixture["provenance"]["base_commit"], "6d7b7da0");
    let mut identities = HashSet::new();
    for case in fixture["cases"].as_array().unwrap() {
        let team_id = case["team_id"].as_i64().unwrap() as i32;
        let position = case["position"].as_u64().unwrap() as usize;
        assert!(identities.insert((team_id, position)));
        assert_eq!(position, 29);
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
        assert_eq!(certificate.outcomes.len(), 54);
        assert_eq!(certificate.ranks.len(), 36);
        let target = model.indices[&team_id];
        assert_eq!(certificate.ranks[target], position - 1);
        assert!(goal_completion::verify(
            &model,
            &certificate,
            Cell {
                team: target,
                rank: position - 1
            }
        ));
        assert!(goal_completion::has_native_score_support(
            &model,
            &certificate
        ));
    }
    assert_eq!(
        identities,
        HashSet::from([(367, 29), (809, 29), (848, 29), (1223, 29)])
    );
}

#[test]
fn november_6_rank_30_and_31_refutations_with_enlarged_diagnostic_budget() {
    // The 2,000,000-cohort budget is an offline diagnostic limit used to fully
    // resolve cells left undecided by the production default budget.
    let request: Request =
        serde_json::from_str(include_str!("fixtures/group-17047-2026-11-06.json")).unwrap();
    let model = Model::new(request).unwrap();
    let proof = RankProof::new(&model).unwrap();
    for team_id in [367, 809, 848, 1223] {
        for position in [30, 31] {
            let result = proof.feasible_domains_with_limits(
                Cell {
                    team: model.indices[&team_id],
                    rank: position - 1,
                },
                position - 1,
                1_000,
                Options::default(),
                Limits {
                    max_cohorts: 2_000_000,
                    max_depth: 512,
                },
            );
            assert!(result.impossible, "team {team_id}, position {position}");
            assert!(result.domains.is_none());
            assert!(result.stats.nodes <= 1_000);
        }
    }
}

#[test]
fn november_6_default_production_cli_characterizes_complete_rank_grid() {
    let output_dir = OutputDir(std::env::temp_dir().join(format!(
        "group-17047-default-{}-{}",
        std::process::id(),
        NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir_all(&output_dir.0).unwrap();

    let (parallel, stderr) = run_cli(4, &output_dir.0.join("workers-4.json"));
    let (single, _) = run_cli(1, &output_dir.0.join("workers-1.json"));
    assert_eq!(
        parallel, single,
        "worker count must not change the response"
    );

    let packed = String::from_utf8_lossy(&stderr)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["stage"] == "search.packed_witness")
        .expect("packed witness pass log");
    assert_eq!(packed["supported"], true, "{packed}");
    let nodes = packed["nodes"].as_u64().expect("packed witness node count");
    assert!(nodes > 0 && nodes <= 6_000, "packed witness nodes: {nodes}");
    let work = packed["cohort_work"].as_u64().expect("packed witness work");
    let work_limit = packed["request_work_limit"].as_u64().unwrap();
    assert!(work > 0 && work <= work_limit, "{packed}");
    assert!(
        packed["cohort_branch_visits"].as_u64().unwrap() > 0,
        "{packed}"
    );
    assert!(
        packed["cohort_pruned_branches"].as_u64().unwrap() > 0,
        "{packed}"
    );

    let team_ids = snapshot_request().0["team_groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|team| team["team_id"].as_i64().unwrap())
        .collect::<Vec<_>>();
    let estimates = parallel["rare_position_estimates"].as_object().unwrap();
    assert_eq!(estimates.len(), 36);
    let mut reachable = 0;
    let mut impossible = 0;
    let mut undecided = HashSet::new();
    for team_id in team_ids {
        let row = estimates[&team_id.to_string()].as_object().unwrap();
        assert_eq!(row.len(), 36, "team {team_id}");
        for rank in 0..36 {
            let estimate = &row[&rank.to_string()];
            let probability = estimate["probability"].as_f64().unwrap();
            assert!(probability.is_finite() && probability >= 0.0);
            match estimate["reachability"].as_str().unwrap() {
                "reachable" => reachable += 1,
                "impossible" => impossible += 1,
                "undecided" => {
                    undecided.insert((team_id, rank + 1));
                }
                label => panic!("unexpected reachability label {label}"),
            }
        }
    }
    assert_eq!(reachable, 1_262);
    assert_eq!(impossible, 34);
    assert!(undecided.is_empty());
    for team_id in [367, 809, 848, 1223] {
        let row = estimates[&team_id.to_string()].as_object().unwrap();
        assert_eq!(
            row["28"]["reachability"], "reachable",
            "team {team_id}, rank 29"
        );
        for rank in [29, 30] {
            assert_eq!(
                row[&rank.to_string()]["reachability"],
                "impossible",
                "team {team_id}, rank {}",
                rank + 1
            );
        }
    }
}
