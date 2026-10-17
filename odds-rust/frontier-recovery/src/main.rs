mod engine;
mod gd_bounds;
mod joint;
mod law;
mod pair;
mod score;

use engine::Engine;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::time::Instant;

fn receipt(engine: &mut Engine, command: &Value) -> Result<Value, String> {
    match command.get("op").and_then(Value::as_str).unwrap_or("") {
        "describe" => Ok(engine.describe()),
        "prepare" => engine.prepare(
            command.get("id").and_then(Value::as_u64).ok_or("missing id")? as usize,
            command.get("k").and_then(Value::as_u64).unwrap_or(7) as usize,
            command.get("state_cap").and_then(Value::as_u64).unwrap_or(250_000) as usize,
            command.get("time_ms").and_then(Value::as_u64).unwrap_or(500),
        ),
        "tighten" => engine.tighten(
            command.get("id").and_then(Value::as_u64).ok_or("missing id")? as usize,
            command.get("state_cap").and_then(Value::as_u64).unwrap_or(250_000) as usize,
            command.get("time_ms").and_then(Value::as_u64).unwrap_or(500),
        ),
        "regime" => engine.regime(
            command.get("id").and_then(Value::as_u64).ok_or("missing id")? as usize,
            command.get("state_cap").and_then(Value::as_u64).unwrap_or(250_000) as usize,
            command.get("time_ms").and_then(Value::as_u64).unwrap_or(1_000),
        ),
        "sample" => engine.sample(
            command.get("id").and_then(Value::as_u64).ok_or("missing id")? as usize,
            command.get("n").and_then(Value::as_u64).unwrap_or(128) as usize,
            command.get("seed").and_then(Value::as_i64).unwrap_or(1),
            command.get("depth").and_then(Value::as_u64).unwrap_or(6) as usize,
        ),
        "quit" => Ok(json!({"quit": true})),
        _ => Err("unknown operation".into()),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = match args.next() { Some(v) => v, None => { eprintln!("usage: frontier-recovery REQUEST TEAM_ID POSITION"); std::process::exit(2) } };
    let team_id: i32 = args.next().and_then(|x| x.parse().ok()).unwrap_or_else(|| { eprintln!("invalid TEAM_ID"); std::process::exit(2) });
    let position: usize = args.next().and_then(|x| x.parse().ok()).unwrap_or_else(|| { eprintln!("invalid POSITION"); std::process::exit(2) });
    let request: golaberto_odds::model::Request = match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|s| serde_json::from_slice(&s).map_err(|e| e.to_string())) {
        Ok(r) => r, Err(e) => { eprintln!("request: {e}"); std::process::exit(2) }
    };
    let mut engine = match Engine::new(request, team_id, position) { Ok(e) => e, Err(e) => { eprintln!("engine: {e}"); std::process::exit(2) } };
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    for line in stdin.lock().lines() {
        let start = Instant::now();
        let result = line.map_err(|e| e.to_string()).and_then(|s| serde_json::from_str::<Value>(&s).map_err(|e| e.to_string())).and_then(|v| {
            if v.get("op").and_then(Value::as_str) == Some("quit") { Ok((json!({"quit":true}), true)) }
            else { receipt(&mut engine, &v).map(|r| (r, false)) }
        });
        let value = match result { Ok((mut v, _)) => { if let Some(obj)=v.as_object_mut() { obj.entry("command_elapsed_ms").or_insert(json!(start.elapsed().as_secs_f64()*1000.0)); } v }, Err(e) => json!({"error":e}) };
        if writeln!(stdout, "{}", value).is_err() { break; }
        let _ = stdout.flush();
        if value.get("quit").and_then(Value::as_bool)==Some(true) { break; }
    }
}
