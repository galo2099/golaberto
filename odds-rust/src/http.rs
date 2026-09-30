//! Small local HTTP adapter for the /odds contract. Requests run serially;
//! each estimator request has at most four active CPU workers.
use crate::model::Request;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const MAX_BODY: usize = 16 * 1024 * 1024;
pub fn serve(address: &str) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(address)?;
    eprintln!("Rust odds server listening on {address}");
    for connection in listener.incoming() {
        let mut stream = connection?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;
        if let Err(e) = handle(&mut stream) {
            eprintln!("odds request failed: {e}");
        }
    }
    Ok(())
}
fn reply(stream: &mut TcpStream, status: &str, body: &[u8]) -> std::io::Result<()> {
    write!(stream,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len())?;
    stream.write_all(body)
}
fn handle(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut first = String::new();
    reader.read_line(&mut first)?;
    let fields: Vec<_> = first.split_whitespace().collect();
    if fields.len() != 3 {
        return Ok(reply(
            stream,
            "400 Bad Request",
            br#"{"error":"invalid request line"}"#,
        )?);
    }
    if fields[0] == "GET" && fields[1] == "/health" {
        return Ok(reply(stream, "200 OK", br#"{"status":"ok"}"#)?);
    }
    if fields[0] != "POST" || fields[1] != "/odds" {
        return Ok(reply(
            stream,
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
            "411 Length Required",
            br#"{"error":"content length required"}"#,
        )?);
    }
    let length = length.unwrap();
    if length > MAX_BODY {
        return Ok(reply(
            stream,
            "413 Content Too Large",
            br#"{"error":"request too large"}"#,
        )?);
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let request: Request = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            let body = serde_json::to_vec(&serde_json::json!({"error":e.to_string()}))?;
            return Ok(reply(stream, "400 Bad Request", &body)?);
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
    match crate::api::calculate(request, seed, 4, 20000) {
        Ok((response, timing)) => {
            eprintln!("rust-odds seed={seed} {}", serde_json::to_string(&timing)?);
            reply(stream, "200 OK", &serde_json::to_vec(&response)?)?;
        }
        Err(error) => {
            reply(
                stream,
                "400 Bad Request",
                &serde_json::to_vec(&serde_json::json!({"error":error}))?,
            )?;
        }
    }
    Ok(())
}
