//! Small local HTTP adapter for the /odds contract. Requests run serially;
//! each estimator request has at most four active CPU workers.
use crate::{logging::RequestLog, model::Request};
use serde_json::json;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const MAX_BODY: usize = 16 * 1024 * 1024;
pub fn serve(address: &str) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(address)?;
    RequestLog::new().event(
        "rust_odds_server_start",
        json!({"listen":address,
        "workers":4,"scout_samples":20000,"pool_samples":100000,"pipeline":"matched_point_pool"}),
    );
    for connection in listener.incoming() {
        let mut stream = connection?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;
        let log = RequestLog::new();
        if let Err(e) = handle(&mut stream, log) {
            log.event("rust_odds_http_error", json!({"error":e.to_string()}));
        }
    }
    Ok(())
}
fn reply(
    stream: &mut TcpStream,
    log: &RequestLog,
    status: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let write_start = Instant::now();
    let status_code = status
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<u16>().ok());
    write!(stream,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len())?;
    stream.write_all(body)?;
    log.stage(
        "http.write",
        write_start,
        json!({"status":status_code,"response_bytes":body.len()}),
    );
    log.event(
        "rust_odds_http_complete",
        json!({"status":status_code,"response_bytes":body.len(),"http_total_ms":log.elapsed_ms()}),
    );
    Ok(())
}
fn handle(stream: &mut TcpStream, mut log: RequestLog) -> Result<(), Box<dyn std::error::Error>> {
    let read_start = Instant::now();
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut first = String::new();
    reader.read_line(&mut first)?;
    let fields: Vec<_> = first.split_whitespace().collect();
    if fields.len() != 3 {
        return Ok(reply(
            stream,
            &log,
            "400 Bad Request",
            br#"{"error":"invalid request line"}"#,
        )?);
    }
    log.event(
        "rust_odds_http_request",
        json!({"method":fields[0],"path":fields[1]}),
    );
    if fields[0] == "GET" && fields[1] == "/health" {
        return Ok(reply(stream, &log, "200 OK", br#"{"status":"ok"}"#)?);
    }
    if fields[0] != "POST" || fields[1] != "/odds" {
        return Ok(reply(
            stream,
            &log,
            "404 Not Found",
            br#"{"error":"expected POST /odds"}"#,
        )?);
    }
    let mut length = None;
    let mut bytes = first.len();
    let mut chunked = false;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        bytes += read;
        if read == 0 || bytes > 65536 {
            return Ok(reply(
                stream,
                &log,
                "400 Bad Request",
                br#"{"error":"invalid headers"}"#,
            )?);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                let parsed = value.trim().parse::<usize>()?;
                if length.replace(parsed).is_some() {
                    return Ok(reply(
                        stream,
                        &log,
                        "400 Bad Request",
                        br#"{"error":"duplicate content length"}"#,
                    )?);
                }
            }
            if name.eq_ignore_ascii_case("transfer-encoding") {
                chunked = true;
            }
        }
    }
    if chunked || length.is_none() {
        return Ok(reply(
            stream,
            &log,
            "411 Length Required",
            br#"{"error":"content length required"}"#,
        )?);
    }
    let length = length.unwrap();
    if length > MAX_BODY {
        return Ok(reply(
            stream,
            &log,
            "413 Content Too Large",
            br#"{"error":"request too large"}"#,
        )?);
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    log.stage("http.read", read_start, json!({"request_bytes":length}));
    let decode_start = Instant::now();
    let request: Request = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            log.stage(
                "http.decode",
                decode_start,
                json!({"valid":false,"error":e.to_string()}),
            );
            let body = serde_json::to_vec(&serde_json::json!({"error":e.to_string()}))?;
            return Ok(reply(stream, &log, "400 Bad Request", &body)?);
        }
    };
    let seed = std::env::var("RARE_POSITION_RANDOM_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as i64
        });
    log = log.context(request.id, seed);
    log.stage("http.decode", decode_start, json!({"valid":true}));
    match crate::api::calculate_logged(request, seed, 4, 20000, &log) {
        Ok((response, _timing)) => {
            let encode_start = Instant::now();
            let body = serde_json::to_vec(&response)?;
            log.stage(
                "http.encode",
                encode_start,
                json!({"response_bytes":body.len()}),
            );
            reply(stream, &log, "200 OK", &body)?;
        }
        Err(error) => {
            log.event("rust_odds_failed", json!({"error":error}));
            reply(
                stream,
                &log,
                "400 Bad Request",
                &serde_json::to_vec(&serde_json::json!({"error":error}))?,
            )?;
        }
    }
    Ok(())
}
