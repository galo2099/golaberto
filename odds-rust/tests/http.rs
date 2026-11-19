use serde_json::{json, Value};
use std::{
    fs::File,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

static SERVICE_LOG_ID: AtomicUsize = AtomicUsize::new(0);

struct Service {
    child: Child,
    address: String,
    log_path: std::path::PathBuf,
}
impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Service {
    fn start() -> Self {
        Self::start_with_family_fallback(None)
    }

    fn start_with_family_fallback(family_fallback: Option<&str>) -> Self {
        Self::start_with_settings(family_fallback, None, None, None, None)
    }

    fn start_with_settings(
        family_fallback: Option<&str>,
        target_overflow_tree: Option<&str>,
        legacy_target_overflow_tree: Option<&str>,
        expected_cohorts: Option<&str>,
        tail_algorithm: Option<&str>,
    ) -> Self {
        let log_path = std::env::temp_dir().join(format!(
            "golaberto-http-{}-{}.log",
            std::process::id(),
            SERVICE_LOG_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let socket = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = socket.local_addr().unwrap().to_string();
        drop(socket);
        let mut command = Command::new(env!("CARGO_BIN_EXE_golaberto-odds"));
        command
            .args(["serve", &address])
            .env_remove("RARE_POSITION_RANDOM_SEED");
        for (key, _) in std::env::vars() {
            if key.starts_with("RUST_ODDS_") || key.starts_with("RARE_POSITION_") {
                command.env_remove(key);
            }
        }
        command.env("RUST_ODDS_LOG", "1");
        if let Some(value) = family_fallback {
            command.env("RUST_ODDS_FAMILY_FALLBACK", value);
        }
        if let Some(value) = target_overflow_tree {
            command.env("RUST_ODDS_TARGET_OVERFLOW_TREE", value);
        }
        if let Some(value) = legacy_target_overflow_tree {
            command.env("RUST_ODDS_EXPERIMENT_TARGET_OVERFLOW_TREE", value);
        }
        if let Some(value) = expected_cohorts {
            command.env("RUST_ODDS_EXPECTED_COHORTS", value);
        }
        if let Some(value) = tail_algorithm {
            command.env("RUST_ODDS_TAIL_ALGORITHM", value);
        }
        let child = command
            .env(
                "DATABASE_URL",
                "mysql://root@127.0.0.1:1/unavailable?tcp_connect_timeout_ms=100",
            )
            .stdout(Stdio::null())
            .stderr(Stdio::from(File::create(&log_path).unwrap()))
            .spawn()
            .unwrap();
        let service = Self {
            child,
            address,
            log_path,
        };
        let start = Instant::now();
        while TcpStream::connect(&service.address).is_err() {
            assert!(start.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(10));
        }
        service
    }

    fn stop_and_read_logs(&mut self) -> String {
        let _ = self.child.kill();
        let _ = self.child.wait();
        std::fs::read_to_string(&self.log_path).unwrap()
    }
    fn request(&self, method: &str, path: &str, body: &[u8], chunked: bool) -> (u16, Vec<u8>) {
        let mut stream = TcpStream::connect(&self.address).unwrap();
        stream
            // End-to-end odds fixtures can exercise native and rare-tail
            // estimators; unrelated routes retain the short health-check timeout.
            .set_read_timeout(Some(Duration::from_secs(if path == "/odds" {
                180
            } else {
                10
            })))
            .unwrap();
        let length = if chunked {
            "Transfer-Encoding: chunked".into()
        } else {
            format!("Content-Length: {}", body.len())
        };
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n{length}\r\n\r\n"
        )
        .unwrap();
        if chunked {
            for chunk in body.chunks(7) {
                write!(stream, "{:x}\r\n", chunk.len()).unwrap();
                stream.write_all(chunk).unwrap();
                stream.write_all(b"\r\n").unwrap();
            }
            stream.write_all(b"0\r\n\r\n").unwrap();
        } else {
            stream.write_all(body).unwrap();
        }
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        let split = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let header = String::from_utf8_lossy(&bytes[..split]);
        let status = header.split_whitespace().nth(1).unwrap().parse().unwrap();
        let body = &bytes[split + 4..];
        let is_chunked = header.lines().any(|line| {
            let (name, value) = line.split_once(':').unwrap_or(("", ""));
            name.eq_ignore_ascii_case("transfer-encoding")
                && value
                    .split(',')
                    .any(|encoding| encoding.trim().eq_ignore_ascii_case("chunked"))
        });
        let body = if is_chunked {
            decode_chunked_body(body)
        } else {
            body.to_vec()
        };
        (status, body)
    }
}

fn decode_chunked_body(body: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::new();
    let mut cursor = 0;
    loop {
        let line_end = body[cursor..]
            .windows(2)
            .position(|window| window == b"\r\n")
            .map(|offset| cursor + offset)
            .expect("chunk size line should end with CRLF");
        let line = std::str::from_utf8(&body[cursor..line_end]).unwrap();
        let size = usize::from_str_radix(line.split(';').next().unwrap().trim(), 16).unwrap();
        cursor = line_end + 2;
        if size == 0 {
            if body[cursor..].starts_with(b"\r\n") {
                assert_eq!(&body[cursor..], b"\r\n");
            } else {
                let trailer_end = body[cursor..]
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .map(|offset| cursor + offset + 4)
                    .expect("chunk trailers should end with CRLFCRLF");
                assert_eq!(trailer_end, body.len());
            }
            return decoded;
        }
        let chunk_end = cursor.checked_add(size).expect("chunk size overflow");
        assert!(chunk_end + 2 <= body.len(), "chunk should fit in response");
        decoded.extend_from_slice(&body[cursor..chunk_end]);
        assert_eq!(&body[chunk_end..chunk_end + 2], b"\r\n");
        cursor = chunk_end + 2;
    }
}

fn event(logs: &str, name: &str) -> Value {
    logs.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|value| value["event"] == name)
        .unwrap_or_else(|| panic!("missing {name} event in service log"))
}

fn accepted_pair(summary: &Value, team_id: i32, rank: usize) -> Value {
    let stage = summary["sampling_stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|stage| {
            stage["stage"] == "final_pair"
                && stage["team"] == team_id
                && stage["rank"] == rank
                && stage["accepted"] == true
        })
        .unwrap_or_else(|| panic!("missing accepted pair for team={team_id} rank={rank}"))
        .clone();
    assert!(stage["main_probability"].as_f64().unwrap() > 0.0);
    assert!(stage["check_probability"].as_f64().unwrap() > 0.0);
    assert!(stage["main_hits"].as_u64().unwrap() >= 30);
    assert!(stage["check_hits"].as_u64().unwrap() >= 30);
    assert!(stage["main_ess"].as_f64().unwrap() >= 4.0);
    assert!(stage["check_ess"].as_f64().unwrap() >= 3.0);
    assert!(stage["main_relative_se"].as_f64().unwrap() <= 0.6);
    assert!(stage["check_relative_se"].as_f64().unwrap() <= 0.75);
    assert!((0.2..=5.0).contains(&stage["main_check_ratio"].as_f64().unwrap()));
    stage
}
#[test]
fn odds_http_uses_default_family_fallback_and_honors_explicit_opt_out() {
    const REQUEST: &[u8] = include_bytes!(
        "../../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json"
    );

    // Keep expected-cohort proposals out of this comparison so it isolates
    // family fallback behavior against its explicit opt-out. Keep the newer
    // tail recovery portfolio disabled so this test stays stage-specific.
    let mut default_service =
        Service::start_with_settings(None, Some("0"), None, Some("0"), Some("off"));
    let (status, default_body) = default_service.request("POST", "/odds", REQUEST, false);
    assert_eq!(status, 200);
    let default_payload: Value = serde_json::from_slice(&default_body).unwrap();

    let (repeat_status, repeat_body) = default_service.request("POST", "/odds", REQUEST, false);
    assert_eq!(repeat_status, 200);
    assert_eq!(
        default_payload,
        serde_json::from_slice::<Value>(&repeat_body).unwrap()
    );
    let default_logs = default_service.stop_and_read_logs();
    let family_summary = event(&default_logs, "rust_odds_family_fallback_late_summary");
    assert_eq!(family_summary["enabled"], true);
    assert_eq!(family_summary["attempts"], 0);
    assert_eq!(family_summary["added"], 0);

    let mut opt_out_service =
        Service::start_with_settings(Some("0"), Some("0"), None, Some("0"), Some("off"));
    let (status, opt_out_body) = opt_out_service.request("POST", "/odds", REQUEST, false);
    assert_eq!(status, 200);
    let opt_out_payload: Value = serde_json::from_slice(&opt_out_body).unwrap();
    let opt_out_logs = opt_out_service.stop_and_read_logs();
    assert_eq!(
        event(&opt_out_logs, "rust_odds_server_start")["family_fallback"]["enabled"],
        false
    );

    assert_eq!(
        default_payload["game_importance"],
        opt_out_payload["game_importance"]
    );
    let default_estimates = default_payload["rare_position_estimates"]
        .as_object()
        .unwrap();
    let opt_out_estimates = opt_out_payload["rare_position_estimates"]
        .as_object()
        .unwrap();
    for (team_id, opt_out_rows) in opt_out_estimates {
        for (rank, opt_out_estimate) in opt_out_rows.as_object().unwrap() {
            if opt_out_estimate["probability"].as_f64().unwrap() > 0.0 {
                let mut default_native = default_estimates[team_id][rank].clone();
                let mut opt_out_native = opt_out_estimate.clone();
                default_native.as_object_mut().unwrap().remove("work_spent");
                opt_out_native.as_object_mut().unwrap().remove("work_spent");
                assert_eq!(
                    default_native, opt_out_native,
                    "native estimate changed for team {team_id} rank {rank}"
                );
            }
        }
    }

    // This isolated run confirms the family's configured no-admission path.
    // The production pool may still publish an independent estimate for the
    // same cell, which must remain unchanged by the disabled stage.
    let (team_id, rank) = ("17", 11_usize);
    let default_estimate = &default_payload["rare_position_estimates"][team_id][rank.to_string()];
    let opt_out_estimate = &opt_out_payload["rare_position_estimates"][team_id][rank.to_string()];
    let mut default_target = default_estimate.clone();
    let mut opt_out_target = opt_out_estimate.clone();
    default_target.as_object_mut().unwrap().remove("work_spent");
    opt_out_target.as_object_mut().unwrap().remove("work_spent");
    assert_eq!(default_target, opt_out_target);
    assert_eq!(
        default_payload["team_odds"][team_id]["Pos"][rank],
        opt_out_payload["team_odds"][team_id]["Pos"][rank]
    );

    // The complete default pipeline independently estimates this same cell
    // with an accepted overflow-tree main/check pair.
    let mut full_service = Service::start();
    let (status, body) = full_service.request("POST", "/odds", REQUEST, false);
    assert_eq!(status, 200);
    let full_payload: Value = serde_json::from_slice(&body).unwrap();
    let full_estimate = &full_payload["rare_position_estimates"][team_id][rank.to_string()];
    assert!(full_estimate["probability"].as_f64().unwrap() > 0.0);
    assert!(full_estimate["samples"].as_u64().unwrap() > 0);
    assert!(full_estimate["relative_se"].as_f64().unwrap() <= 0.5);
    assert!(full_estimate["ess"].as_f64().unwrap() >= 4.0);
    assert_eq!(
        full_estimate["design"],
        "matched_point_pool_target_overflow_tree"
    );
    assert_eq!(full_estimate["reachability"], "reachable");
    assert_eq!(
        full_payload["team_odds"][team_id]["Pos"][rank],
        full_estimate["probability"].as_f64().unwrap() * 100.0
    );
    let full_logs = full_service.stop_and_read_logs();
    let full_overflow = event(&full_logs, "rust_odds_target_overflow_tree_summary");
    assert_eq!(full_overflow["enabled"], true);
    assert!(full_overflow["added"].as_u64().unwrap() > 0);
    let pair = accepted_pair(&full_overflow, 17, 12);
    assert_eq!(pair["team"], 17);
    assert_eq!(pair["rank"], 12);
}

#[test]
fn odds_http_accepts_october_11_group_16653_snapshot_for_novorizontino() {
    const REQUEST: &[u8] = include_bytes!("fixtures/group-16653-2026-10-11.json");
    let service = Service::start();
    let (status, body) = service.request("POST", "/odds", REQUEST, false);
    assert_eq!(status, 200);
    let payload: Value = serde_json::from_slice(&body).unwrap();

    let position_row = payload["team_odds"]["2064"]["Pos"]
        .as_array()
        .expect("team 2064 position row should be an array");
    assert_eq!(position_row.len(), 20);
    let estimate = &payload["rare_position_estimates"]["2064"]["14"];
    assert!(
        estimate.is_object(),
        "team 2064 fifteenth-place estimate should exist"
    );
    assert_eq!(estimate["reachability"], "reachable");
    assert!(estimate["probability"].as_f64().unwrap() > 0.0);
    assert_eq!(estimate["meets_precision_goal"], true);
    assert!(estimate["relative_se"].as_f64().unwrap() <= 0.5);
    assert!(estimate["ess"].as_f64().unwrap() >= 10.0);
    assert_eq!(
        position_row[14].as_f64().unwrap(),
        estimate["probability"].as_f64().unwrap() * 100.0
    );
    assert!(
        estimate["design"]
            .as_str()
            .unwrap()
            .contains("gd_refinement")
            || estimate["design"].as_str().unwrap().contains("gd_cohort")
    );
    assert_eq!(
        position_row[14].as_f64().unwrap(),
        estimate["probability"].as_f64().unwrap() * 100.0
    );
}

#[test]
fn odds_http_enables_default_target_overflow_tree_and_honors_production_opt_out() {
    const REQUEST: &[u8] = include_bytes!(
        "../../experiments/rare_positions/reference/2026-09-30-hundredfold/inputs/group-16498-44eabb47.json"
    );
    // Disable the new tail portfolio as well as other late recovery stages so
    // this comparison isolates the target overflow tree and its production opt-out.
    let mut default_service =
        Service::start_with_settings(Some("0"), None, None, Some("0"), Some("off"));
    let (status, body) = default_service.request("POST", "/odds", REQUEST, false);
    assert_eq!(status, 200);
    let default_payload: Value = serde_json::from_slice(&body).unwrap();
    let (repeat_status, repeat_body) = default_service.request("POST", "/odds", REQUEST, false);
    assert_eq!(repeat_status, 200);
    assert_eq!(
        default_payload,
        serde_json::from_slice::<Value>(&repeat_body).unwrap()
    );
    let default_logs = default_service.stop_and_read_logs();
    let overflow_summary = event(&default_logs, "rust_odds_target_overflow_tree_summary");
    assert_eq!(overflow_summary["enabled"], true);
    assert!(overflow_summary["root_count_attempts"].as_u64().unwrap() > 0);
    assert!(overflow_summary["added"].as_u64().unwrap() > 0);
    let pair = accepted_pair(&overflow_summary, 17, 12);
    assert_eq!(pair["team"], 17);
    assert_eq!(pair["rank"], 12);

    let mut opt_out_service =
        Service::start_with_settings(Some("0"), Some("0"), Some("1"), Some("0"), Some("off"));
    let (status, body) = opt_out_service.request("POST", "/odds", REQUEST, false);
    assert_eq!(status, 200);
    let opt_out_payload: Value = serde_json::from_slice(&body).unwrap();
    let opt_out_logs = opt_out_service.stop_and_read_logs();
    assert_eq!(
        event(&opt_out_logs, "rust_odds_server_start")["target_overflow_tree"]["enabled"],
        false
    );

    let team = "17";
    let rank = "11";
    let position_index = rank.parse::<usize>().unwrap();
    let default_estimate = &default_payload["rare_position_estimates"][team][rank];
    let opt_out_estimate = &opt_out_payload["rare_position_estimates"][team][rank];
    assert!(default_estimate["probability"].as_f64().unwrap() > 0.0);
    assert!(default_estimate["samples"].as_u64().unwrap() > 0);
    assert!(default_estimate["relative_se"].as_f64().unwrap() <= 0.5);
    assert!(default_estimate["ess"].as_f64().unwrap() >= 4.0);
    assert_eq!(
        default_estimate["design"],
        "matched_point_pool_target_overflow_tree"
    );
    assert_eq!(default_estimate["reachability"], "reachable");
    assert_eq!(opt_out_estimate["probability"], 0.0);
    assert_ne!(opt_out_estimate["reachability"], "impossible");
    assert_eq!(
        default_payload["team_odds"][team]["Pos"][position_index],
        default_estimate["probability"].as_f64().unwrap() * 100.0
    );
    assert_eq!(
        opt_out_payload["team_odds"][team]["Pos"][position_index],
        0.0
    );

    assert_eq!(
        default_payload["game_importance"],
        opt_out_payload["game_importance"]
    );
    let default_estimates = default_payload["rare_position_estimates"]
        .as_object()
        .unwrap();
    let opt_out_estimates = opt_out_payload["rare_position_estimates"]
        .as_object()
        .unwrap();
    for (team_id, opt_out_rows) in opt_out_estimates {
        for (rank, opt_out_estimate) in opt_out_rows.as_object().unwrap() {
            if opt_out_estimate["probability"].as_f64().unwrap() > 0.0 {
                let mut default_native = default_estimates[team_id][rank].clone();
                let mut opt_out_native = opt_out_estimate.clone();
                default_native.as_object_mut().unwrap().remove("work_spent");
                opt_out_native.as_object_mut().unwrap().remove("work_spent");
                assert_eq!(
                    default_native, opt_out_native,
                    "estimate changed for team {team_id} rank {rank}"
                );
            }
        }
    }
}

#[test]
fn incomplete_upload_does_not_block_health_or_ready_calculations() {
    let service = Service::start();
    let mut upload = TcpStream::connect(&service.address).unwrap();
    write!(
        upload,
        "POST /odds HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1024\r\n\r\n{{"
    )
    .unwrap();
    upload.flush().unwrap();
    assert_eq!(service.request("GET", "/health", &[], false).0, 200);
    let (status, body) = service.request(
        "POST",
        "/spi",
        br#"{"games":[],"ratings":[{"id":1}]}"#,
        false,
    );
    assert_eq!(status, 200);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"1":null})
    );
    drop(upload);
}

#[test]
fn active_routes_chunked_json_errors_and_health_work_over_http() {
    let service = Service::start();
    assert_eq!(service.request("GET", "/health", &[], false).0, 200);
    assert_eq!(
        service.request("POST", "/player_ratings", &[], false).0,
        500
    );
    assert_eq!(service.request("GET", "/spi", &[], false).0, 405);
    assert_eq!(service.request("POST", "/spi", b"invalid", false).0, 400);
    let request = br#"{"games":[],"ratings":[{"id":1,"offense":null,"defense":null}]}"#;
    let (status, body) = service.request("POST", "/spi?test=1", request, true);
    assert_eq!(status, 200);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"1":null})
    );
    assert_eq!(service.request("POST", "/eval", request, false).0, 400);
    let (status, body) = service.request("POST", "/historic_ratings", request, false);
    assert_eq!(status, 200);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"ratings":{},"offense":{},"defense":{},"dates":[]})
    );
    let request = br#"{"games":[{"home_id":1,"away_id":2,"home_score":1,"away_score":0,"timestamp":1700000000,"length":1}],"ratings":[{"id":1},{"id":2}]}"#;
    assert_eq!(service.request("POST", "/spi", request, false).0, 200);
    let evaluation = br#"{"games":[{"phase_id":1,"home_id":1,"away_id":2,"home_score":1,"away_score":0,"timestamp":1700000000,"length":1},{"phase_id":2,"home_id":2,"away_id":1,"home_score":0,"away_score":0,"timestamp":1700086400,"length":1}],"ratings":[{"id":1},{"id":2}],"phases_to_eval":[2]}"#;
    let (status, body) = service.request("POST", "/eval", evaluation, false);
    assert_eq!(status, 200);
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert!((0. ..=1.).contains(&value["rps"].as_f64().unwrap()));
    assert_eq!(value["team_rps"].as_object().unwrap().len(), 2);
    let (status, body) = service.request("POST", "/historic_ratings", request, false);
    assert_eq!(status, 500);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["error"],
        "rating database operation failed"
    );
    assert_eq!(service.request("GET", "/health", &[], false).0, 200);
}
