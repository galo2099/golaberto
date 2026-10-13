use serde_json::Value;
use sha2::{Digest, Sha256};

const REQUEST: &[u8] = include_bytes!("fixtures/group-16687-2026-10-12.json");
const REQUEST_SHA256: &str = "ea8375fb04530496e9981c8fd27037b5de6acc47774ae7c56dc7efbee16dfaed";
const TEAM_COUNT: usize = 218;
const GAME_COUNT: usize = 263;
const PLAYED_GAMES: usize = 243;
const UNPLAYED_GAMES: usize = 20;

#[test]
fn request_fixture_has_expected_identity_and_dimensions() {
    let digest = format!("{:x}", Sha256::digest(REQUEST));
    assert_eq!(digest, REQUEST_SHA256);

    let request: Value = serde_json::from_slice(REQUEST).expect("parse request fixture");
    assert_eq!(request["id"].as_i64(), Some(16687));
    assert_eq!(
        request["phase"]["sort"].as_str(),
        Some("pt, w, gd, gf, gp, g_away, name")
    );
    assert_eq!(request["zones"].as_array().map(Vec::len), Some(0));

    let teams = request["team_groups"]
        .as_array()
        .expect("team_groups array");
    assert_eq!(teams.len(), TEAM_COUNT);
    let games = request["games"].as_array().expect("games array");
    assert_eq!(games.len(), GAME_COUNT);
    let played = games
        .iter()
        .filter(|game| game["played"].as_bool() == Some(true))
        .count();
    assert_eq!(played, PLAYED_GAMES);
    assert_eq!(games.len() - played, UNPLAYED_GAMES);
}

#[cfg(not(debug_assertions))]
mod release_performance {
    use super::{REQUEST, TEAM_COUNT, UNPLAYED_GAMES};
    use serde_json::Value;
    use std::{
        collections::BTreeSet,
        env,
        ffi::OsStr,
        fs,
        path::{Path, PathBuf},
        process::{Command, Output, Stdio},
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    const ESTIMATE_SAMPLES: u64 = 100_000;
    const CHILD_TIMEOUT: Duration = Duration::from_secs(90);

    struct OutputDir(PathBuf);

    impl OutputDir {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos();
            let path = env::temp_dir().join(format!(
                "golaberto-request-performance-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create performance output directory");
            Self(path)
        }
    }

    impl Drop for OutputDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    #[ignore = "manual performance measurement"]
    fn release_request_performance_measurement_preserves_output_quality() {
        let binary = env::var_os("ODDS_PERFORMANCE_BINARY")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_golaberto-odds")));
        let output_dir = OutputDir::new();
        let warm_output_path = output_dir.0.join("warm.json");

        let mut warm_command = base_command(&binary);
        warm_command
            .arg("estimate")
            .arg(fixture_path())
            .arg(&warm_output_path)
            .arg("808")
            .arg("4");
        let warm_output = run_child(warm_command, CHILD_TIMEOUT);
        assert_success(&warm_output, "warm-up estimate");
        let warm_response: Value =
            serde_json::from_slice(&fs::read(&warm_output_path).expect("read warm-up response"))
                .expect("parse warm-up response");
        validate_response(&warm_response);

        let mut bench_command = base_command(&binary);
        bench_command.arg("bench").arg(fixture_path()).arg("7");
        let bench_output = run_child(bench_command, CHILD_TIMEOUT);
        assert_success(&bench_output, "benchmark");
        let reported_median_ms = parse_reported_median(&bench_output.stdout);
        let samples_ms = parse_iteration_timings(&bench_output.stderr);
        assert_eq!(
            samples_ms.len(),
            7,
            "expected exactly seven benchmark iterations"
        );
        let mut sorted_samples = samples_ms.clone();
        sorted_samples.sort_by(f64::total_cmp);
        let median_ms = sorted_samples[3];
        assert_eq!(reported_median_ms.to_bits(), median_ms.to_bits());

        println!("request performance: samples_ms={samples_ms:?}, median_ms={median_ms:.3}");
    }

    fn fixture_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/group-16687-2026-10-12.json")
    }

    fn base_command(binary: &Path) -> Command {
        let mut command = Command::new(binary);
        for (key, _) in env::vars_os() {
            if starts_with(&key, "RUST_ODDS_") || starts_with(&key, "RARE_POSITION_") {
                command.env_remove(key);
            }
        }
        command.env("RUST_ODDS_LOG", "0");
        command
    }

    fn starts_with(value: &OsStr, prefix: &str) -> bool {
        value.to_string_lossy().starts_with(prefix)
    }

    fn run_child(mut command: Command, timeout: Duration) -> Output {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.spawn().expect("spawn odds CLI");
        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return child.wait_with_output().expect("collect odds CLI output"),
                Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(20)),
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait_with_output();
                    panic!("odds CLI exceeded {timeout:?} timeout");
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait_with_output();
                    panic!("failed waiting for odds CLI: {error}");
                }
            }
        }
    }

    fn assert_success(output: &Output, label: &str) {
        assert!(
            output.status.success(),
            "{label} failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn validate_response(response: &Value) {
        let fixture: Value = serde_json::from_slice(REQUEST).expect("parse fixture team ids");
        let expected_teams: BTreeSet<String> = fixture["team_groups"]
            .as_array()
            .expect("fixture team_groups")
            .iter()
            .map(|team| {
                team["team_id"]
                    .as_i64()
                    .expect("fixture team_id")
                    .to_string()
            })
            .collect();
        let expected_ranks: BTreeSet<String> =
            (0..TEAM_COUNT).map(|rank| rank.to_string()).collect();
        let team_odds = response["team_odds"].as_object().expect("team_odds object");
        assert_eq!(team_odds.len(), TEAM_COUNT);
        assert_eq!(
            team_odds.keys().cloned().collect::<BTreeSet<_>>(),
            expected_teams.clone(),
            "team_odds team ids"
        );
        let mut columns = vec![0.; TEAM_COUNT];
        for (team_id, odds) in team_odds {
            let positions = odds["Pos"].as_array().unwrap_or_else(|| {
                panic!("team {team_id} has no Pos array");
            });
            assert_eq!(positions.len(), TEAM_COUNT, "team {team_id} positions");
            let mut row_sum = 0.;
            for (rank, value) in positions.iter().enumerate() {
                let probability = value
                    .as_f64()
                    .unwrap_or_else(|| panic!("team {team_id} rank {rank} is not numeric"));
                assert!(
                    probability.is_finite() && (0. ..=100.).contains(&probability),
                    "team {team_id} rank {rank} has invalid Pos value {probability}"
                );
                row_sum += probability;
                columns[rank] += probability;
            }
            assert!(
                (row_sum - 100.).abs() <= 1e-8,
                "team {team_id} Pos sums to {row_sum}"
            );
        }
        for (rank, sum) in columns.into_iter().enumerate() {
            assert!((sum - 100.).abs() <= 1e-8, "rank {rank} sums to {sum}");
        }

        let estimates = response["rare_position_estimates"]
            .as_object()
            .expect("rare_position_estimates object");
        assert_eq!(estimates.len(), TEAM_COUNT);
        assert_eq!(
            estimates.keys().cloned().collect::<BTreeSet<_>>(),
            expected_teams,
            "estimate team ids"
        );
        for (team_id, ranks) in estimates {
            let ranks = ranks
                .as_object()
                .unwrap_or_else(|| panic!("team {team_id} estimates are not an object"));
            assert_eq!(ranks.len(), TEAM_COUNT, "team {team_id} estimates");
            assert_eq!(
                ranks.keys().cloned().collect::<BTreeSet<_>>(),
                expected_ranks,
                "team {team_id} rank keys"
            );
            let positions = team_odds
                .get(team_id.as_str())
                .expect("corresponding team odds")["Pos"]
                .as_array()
                .expect("corresponding Pos array");
            for (rank, estimate) in ranks {
                assert_eq!(
                    estimate["samples"].as_u64(),
                    Some(ESTIMATE_SAMPLES),
                    "team {team_id} rank {rank} sample count"
                );
                let standard_error = estimate["std_err"]
                    .as_f64()
                    .unwrap_or_else(|| panic!("team {team_id} rank {rank} has no std_err"));
                assert!(
                    standard_error.is_finite() && standard_error >= 0.,
                    "team {team_id} rank {rank} has invalid std_err {standard_error}"
                );
                let probability = estimate["probability"]
                    .as_f64()
                    .unwrap_or_else(|| panic!("team {team_id} rank {rank} has no probability"));
                assert!(
                    probability.is_finite() && (0. ..=1.).contains(&probability),
                    "team {team_id} rank {rank} has invalid probability {probability}"
                );
                let position_index = rank.parse::<usize>().expect("numeric rank key");
                let position = positions[position_index]
                    .as_f64()
                    .expect("numeric Pos value");
                assert!(
                    (probability * 100. - position).abs() <= 1e-10,
                    "team {team_id} rank {rank} estimate {probability} disagrees with Pos {position}"
                );
            }
        }
        for (team_id, rank) in [("1538", "107"), ("2895", "120")] {
            let probability = response["rare_position_estimates"][team_id][rank]["probability"]
                .as_f64()
                .unwrap_or_else(|| panic!("team {team_id} rank {rank} has no probability"));
            assert!(
                probability > 0.,
                "team {team_id} rank {rank} lost its recovered positive estimate"
            );
        }

        let importance = response["game_importance"]
            .as_object()
            .expect("game_importance object");
        assert_eq!(importance.len(), UNPLAYED_GAMES);
        for (game_id, values) in importance {
            let values = values
                .as_array()
                .unwrap_or_else(|| panic!("game {game_id} importance is not an array"));
            assert_eq!(values.len(), 2, "game {game_id} importance sides");
            assert!(values.iter().all(|value| value.as_f64() == Some(0.)));
        }
    }

    fn parse_reported_median(stdout: &[u8]) -> f64 {
        let stdout = std::str::from_utf8(stdout).expect("benchmark stdout UTF-8");
        let mut medians = stdout
            .lines()
            .filter_map(|line| line.strip_prefix("median_ms="))
            .map(|value| value.parse::<f64>().expect("parse reported median"));
        let median = medians.next().expect("benchmark reported median_ms");
        assert!(medians.next().is_none(), "multiple reported medians");
        assert!(
            median.is_finite() && median > 0.,
            "invalid reported median {median}"
        );
        median
    }

    fn parse_iteration_timings(stderr: &[u8]) -> Vec<f64> {
        let stderr = std::str::from_utf8(stderr).expect("benchmark stderr UTF-8");
        let mut timings = Vec::new();
        for line in stderr.lines() {
            let Some((iteration, payload)) = line
                .strip_prefix("iteration=")
                .and_then(|line| line.split_once(' '))
            else {
                continue;
            };
            let iteration = iteration.parse::<usize>().expect("parse iteration index");
            assert_eq!(iteration, timings.len(), "benchmark iteration order");
            let timing: Value = serde_json::from_str(payload).expect("parse iteration timing JSON");
            let total_ms = timing["total_ms"]
                .as_f64()
                .expect("iteration total_ms number");
            assert!(
                total_ms.is_finite() && total_ms > 0.,
                "invalid total_ms {total_ms}"
            );
            timings.push(total_ms);
        }
        timings
    }
}
