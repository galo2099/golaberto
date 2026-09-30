use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Service {
    child: Child,
    address: String,
}
impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Service {
    fn start() -> Self {
        let socket = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = socket.local_addr().unwrap().to_string();
        drop(socket);
        let child = Command::new(env!("CARGO_BIN_EXE_golaberto-odds"))
            .args(["serve", &address])
            .env("RUST_ODDS_LOG", "0")
            .env("RARE_POSITION_RANDOM_SEED", "808")
            .env(
                "DATABASE_URL",
                "mysql://root@127.0.0.1:1/unavailable?tcp_connect_timeout_ms=100",
            )
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let service = Self { child, address };
        let start = Instant::now();
        while TcpStream::connect(&service.address).is_err() {
            assert!(start.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(10));
        }
        service
    }
    fn request(&self, method: &str, path: &str, body: &[u8], chunked: bool) -> (u16, Vec<u8>) {
        let mut stream = TcpStream::connect(&self.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
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
        (status, bytes[split + 4..].to_vec())
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
