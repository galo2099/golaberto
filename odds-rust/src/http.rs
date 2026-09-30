//! HTTP replacement for the active Go application endpoints.
//! A dedicated calculation thread keeps the estimator's four-core budget.
use crate::{database, logging::RequestLog, ratings};
use serde_json::json;
use std::{
    io::Read,
    sync::{mpsc, Arc, Mutex},
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

pub const MAX_BODY: usize = 128 * 1024 * 1024;
pub const ENDPOINTS: [&str; 5] = [
    "/odds",
    "/spi",
    "/eval",
    "/historic_ratings",
    "/player_ratings",
];

#[derive(Debug)]
pub struct EndpointError {
    pub status: u16,
    pub message: String,
}
fn bad(error: impl ToString) -> EndpointError {
    EndpointError {
        status: 400,
        message: error.to_string(),
    }
}
fn database_error(error: impl std::fmt::Display, log: &RequestLog) -> EndpointError {
    log.event(
        "rust_service_database_error",
        json!({"error":error.to_string()}),
    );
    EndpointError {
        status: 500,
        message: "rating database operation failed".into(),
    }
}
pub fn execute(path: &str, body: &[u8], log: &RequestLog) -> Result<Vec<u8>, EndpointError> {
    let decode = Instant::now();
    let response = match path {
        "/odds" => {
            let request: crate::model::Request = serde_json::from_slice(body).map_err(bad)?;
            let seed = std::env::var("RARE_POSITION_RANDOM_SEED")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| {
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos() as i64
                });
            let log = log.context(request.id, seed);
            log.stage("http.decode", decode, json!({"valid":true}));
            let (response, _) =
                crate::api::calculate_logged(request, seed, 4, 20000, &log).map_err(bad)?;
            serde_json::to_value(response).map_err(bad)?
        }
        "/player_ratings" => {
            // Like the existing stats endpoint, this command has no JSON payload.
            crate::player_ratings::run(log).map_err(|e| database_error(e, log))?;
            json!({"status":"ok"})
        }
        "/spi" | "/eval" | "/historic_ratings" => {
            let request: ratings::Request = serde_json::from_slice(body).map_err(bad)?;
            request.validate().map_err(bad)?;
            log.stage(
                "http.decode",
                decode,
                json!({"valid":true,"games":request.games.len(),"teams":request.ratings.len()}),
            );
            let calculate = Instant::now();
            let value = match path {
                "/spi" => serde_json::to_value(
                    ratings::spi(&request.games, &request.initial()).map_err(bad)?,
                )
                .map_err(bad)?,
                "/eval" => {
                    serde_json::to_value(ratings::evaluate(&request).map_err(bad)?).map_err(bad)?
                }
                _ => {
                    let rows = ratings::historical(&request).map_err(bad)?;
                    if !rows.is_empty() {
                        let mut conn = database::connect().map_err(|e| database_error(e, log))?;
                        database::persist_history(&mut conn, &rows)
                            .map_err(|e| database_error(e, log))?;
                    }
                    log.event(
                        "rust_service_historical_written",
                        json!({"rows":rows.len()}),
                    );
                    // Go persists the series itself and returns empty collections.
                    json!({"ratings":{},"offense":{},"defense":{},"dates":[]})
                }
            };
            log.stage("ratings.calculate", calculate, json!({"endpoint":path}));
            value
        }
        _ => {
            return Err(EndpointError {
                status: 404,
                message: "unknown endpoint".into(),
            })
        }
    };
    let encode = Instant::now();
    let bytes = serde_json::to_vec(&response).map_err(bad)?;
    log.stage("http.encode", encode, json!({"response_bytes":bytes.len()}));
    Ok(bytes)
}
fn reply(request: Request, status: u16, body: Vec<u8>, log: &RequestLog) {
    let bytes = body.len();
    let start = Instant::now();
    let response = Response::from_data(body)
        .with_status_code(StatusCode(status))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
    if let Err(e) = request.respond(response) {
        log.event("rust_odds_http_error", json!({"error":e.to_string()}));
    }
    log.stage(
        "http.write",
        start,
        json!({"status":status,"response_bytes":bytes}),
    );
    log.event(
        "rust_odds_http_complete",
        json!({"status":status,"response_bytes":bytes,"http_total_ms":log.elapsed_ms()}),
    );
}
fn error_reply(request: Request, error: EndpointError, log: &RequestLog) {
    log.event(
        "rust_service_failed",
        json!({"status":error.status,"error":error.message}),
    );
    reply(
        request,
        error.status,
        serde_json::to_vec(&json!({"error":error.message})).unwrap(),
        log,
    );
}
type CalculationResult = Result<Vec<u8>, EndpointError>;
type Calculation = (
    String,
    Vec<u8>,
    Instant,
    RequestLog,
    mpsc::SyncSender<CalculationResult>,
);

fn read_body(mut request: Request, calculator: &mpsc::SyncSender<Calculation>, log: RequestLog) {
    let read = Instant::now();
    if request.body_length().is_some_and(|size| size > MAX_BODY) {
        error_reply(
            request,
            EndpointError {
                status: 413,
                message: "request too large".into(),
            },
            &log,
        );
        return;
    }
    let mut body = Vec::new();
    if let Err(e) = request
        .as_reader()
        .take((MAX_BODY + 1) as u64)
        .read_to_end(&mut body)
    {
        error_reply(request, bad(e), &log);
        return;
    }
    if body.len() > MAX_BODY {
        error_reply(
            request,
            EndpointError {
                status: 413,
                message: "request too large".into(),
            },
            &log,
        );
        return;
    }
    log.stage("http.read", read, json!({"request_bytes":body.len()}));
    let wait = Instant::now();
    // Keep slow uploads on the readers. One calculation thread reuses its heap
    // across requests instead of retaining separate working sets in each reader.
    let path = request.url().split('?').next().unwrap_or("").to_string();
    let (response, completed) = mpsc::sync_channel(1);
    let result = if calculator.send((path, body, wait, log, response)).is_ok() {
        completed.recv().unwrap_or_else(|_| {
            Err(EndpointError {
                status: 503,
                message: "calculation worker unavailable".into(),
            })
        })
    } else {
        Err(EndpointError {
            status: 503,
            message: "calculation worker unavailable".into(),
        })
    };
    // Response I/O stays on the reader, so a slow client cannot hold the
    // calculation worker after its result is ready.
    match result {
        Ok(bytes) => reply(request, 200, bytes, &log),
        Err(error) => error_reply(request, error, &log),
    }
}

fn calculate(path: &str, body: Vec<u8>, wait: Instant, log: RequestLog) -> CalculationResult {
    log.stage("http.queue", wait, json!({}));
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| execute(path, &body, &log)));
    drop(body);
    match result {
        Ok(result) => result,
        Err(_) => Err(EndpointError {
            status: 500,
            message: "calculation failed".into(),
        }),
    }
}
pub fn serve(address: &str) -> Result<(), Box<dyn std::error::Error>> {
    let server = Server::http(address).map_err(|e| -> Box<dyn std::error::Error> { e })?;
    RequestLog::new().event("rust_odds_server_start", json!({"listen":address,"workers":4,
        "body_readers":4,"calculation_workers":1,
        "endpoints":ENDPOINTS,"scout_samples":20000,"pool_samples":100000,"pipeline":"matched_point_pool"}));
    let (sender, receiver) = mpsc::sync_channel::<(Request, RequestLog)>(16);
    let receiver = Arc::new(Mutex::new(receiver));
    // A rendezvous channel keeps prepared bodies bounded by the reader count.
    let (calculator, ready) = mpsc::sync_channel::<Calculation>(0);
    std::thread::scope(|scope| {
        scope.spawn(move || {
            for (path, body, wait, log, response) in ready {
                let result = calculate(&path, body, wait, log);
                let _ = response.send(result);
            }
        });
        for _ in 0..4 {
            let calculator = calculator.clone();
            let receiver = receiver.clone();
            scope.spawn(move || loop {
                let next = receiver.lock().unwrap().recv();
                let Ok((request, log)) = next else {
                    break;
                };
                read_body(request, &calculator, log);
            });
        }
        for request in server.incoming_requests() {
            let log = RequestLog::new();
            log.event(
                "rust_odds_http_request",
                json!({"method":request.method().as_str(),"path":request.url()}),
            );
            let path = request.url().split('?').next().unwrap_or("");
            if request.method() == &Method::Get && path == "/health" {
                reply(request, 200, br#"{"status":"ok"}"#.to_vec(), &log);
            } else if !ENDPOINTS.contains(&path) {
                error_reply(
                    request,
                    EndpointError {
                        status: 404,
                        message: "unknown endpoint".into(),
                    },
                    &log,
                );
            } else if request.method() != &Method::Post {
                error_reply(
                    request,
                    EndpointError {
                        status: 405,
                        message: "expected POST".into(),
                    },
                    &log,
                );
            } else if let Err(e) = sender.try_send((request, log)) {
                let (request, log) = match e {
                    mpsc::TrySendError::Full(value) | mpsc::TrySendError::Disconnected(value) => {
                        value
                    }
                };
                error_reply(
                    request,
                    EndpointError {
                        status: 503,
                        message: "request queue full".into(),
                    },
                    &log,
                );
            }
        }
        drop(sender);
        drop(calculator);
    });
    Ok(())
}
