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

fn suffix_options(command: &Value) -> Result<Option<joint::SuffixOptions>, String> {
    let Some(value) = command.get("suffix") else { return Ok(None); };
    let object = value.as_object().ok_or("suffix must be an object")?;
    let defaults = joint::SuffixOptions::default();
    let uint = |key: &str, default: u64| -> Result<usize, String> {
        match object.get(key) {
            None => Ok(default as usize),
            Some(v) => v.as_u64().map(|n| n as usize).ok_or_else(|| format!("suffix.{key} must be an unsigned integer")),
        }
    };
    let budget_fraction = match object.get("budget_fraction") {
        None => defaults.budget_fraction,
        Some(v) => v.as_f64().ok_or("suffix.budget_fraction must be a number")?,
    };
    let probe_ms = match object.get("probe_ms") {
        None => defaults.probe_ms,
        Some(v) => v.as_u64().ok_or("suffix.probe_ms must be an unsigned integer")?,
    };
    let canonicalize_certain = match object.get("canonicalize_certain") {
        None => false,
        Some(v) => v.as_bool().ok_or("suffix.canonicalize_certain must be a boolean")?,
    };
    let joint_ratio = match object.get("joint_ratio") {
        None => defaults.joint_ratio,
        Some(v) => v.as_bool().ok_or("suffix.joint_ratio must be a boolean")?,
    };
    let score_gd = match object.get("score_gd") {
        None => defaults.score_gd,
        Some(v) => v.as_bool().ok_or("suffix.score_gd must be a boolean")?,
    };
    let score_setup_ms = match object.get("score_setup_ms") {
        None => defaults.score_setup_ms,
        Some(v) => v.as_u64().filter(|n| *n > 0).ok_or("suffix.score_setup_ms must be a positive integer")?,
    };
    let shared_expectation = match object.get("shared_expectation") {
        None => defaults.shared_expectation,
        Some(v) => v.as_bool().ok_or("suffix.shared_expectation must be a boolean")?,
    };
    let expectation_bits = match object.get("expectation_bits") {
        None => defaults.expectation_bits,
        Some(v) => v.as_u64().filter(|n| (4..=20).contains(n))
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("suffix.expectation_bits must be an integer in 4..=20")?,
    };
    let expectation_max_remaining = match object.get("expectation_max_remaining") {
        None => defaults.expectation_max_remaining,
        Some(v) => v.as_u64().filter(|n| *n > 0).and_then(|n| usize::try_from(n).ok())
            .ok_or("suffix.expectation_max_remaining must be a positive integer")?,
    };
    let expectation_budget_fraction = match object.get("expectation_budget_fraction") {
        None => defaults.expectation_budget_fraction,
        Some(v) => v.as_f64().ok_or("suffix.expectation_budget_fraction must be a number")?,
    };
    let expectation_score_gd = match object.get("expectation_score_gd") {
        None => defaults.expectation_score_gd,
        Some(v) => v.as_bool().ok_or("suffix.expectation_score_gd must be a boolean")?,
    };
    if expectation_score_gd && !(shared_expectation && score_gd) {
        return Err("suffix.expectation_score_gd requires shared_expectation and score_gd".into());
    }
    if !expectation_budget_fraction.is_finite() || !(0.0..=1.0).contains(&expectation_budget_fraction)
        || expectation_budget_fraction == 0.0
    {
        return Err("suffix.expectation_budget_fraction must be finite and in (0,1]".into());
    }
    Ok(Some(joint::SuffixOptions {
        state_cap: uint("state_cap", defaults.state_cap as u64)?,
        max_remaining: uint("max_remaining", defaults.max_remaining as u64)?,
        probe_nodes: uint("probe_nodes", defaults.probe_nodes as u64)?,
        probe_ms,
        budget_fraction,
        canonicalize_certain,
        joint_ratio,
        score_gd,
        score_setup_ms,
        shared_expectation,
        expectation_bits,
        expectation_max_remaining,
        expectation_budget_fraction,
        expectation_score_gd,
    }))
}

fn receipt(engine: &mut Engine, command: &Value) -> Result<Value, String> {
    match command.get("op").and_then(Value::as_str).unwrap_or("") {
        "describe" => Ok(engine.describe()),
        "feasibility_model" => engine.feasibility_model(
            command
                .get("id")
                .and_then(Value::as_u64)
                .ok_or("missing id")? as usize,
        ),
        "prepare" => engine.prepare(
            command
                .get("id")
                .and_then(Value::as_u64)
                .ok_or("missing id")? as usize,
            command.get("k").and_then(Value::as_u64).unwrap_or(7) as usize,
            command
                .get("state_cap")
                .and_then(Value::as_u64)
                .unwrap_or(250_000) as usize,
            command
                .get("time_ms")
                .and_then(Value::as_u64)
                .unwrap_or(500),
        ),
        "tighten" => engine.tighten(
            command
                .get("id")
                .and_then(Value::as_u64)
                .ok_or("missing id")? as usize,
            command
                .get("state_cap")
                .and_then(Value::as_u64)
                .unwrap_or(250_000) as usize,
            command
                .get("time_ms")
                .and_then(Value::as_u64)
                .unwrap_or(500),
        ),
        "regime" => engine.regime(
            command
                .get("id")
                .and_then(Value::as_u64)
                .ok_or("missing id")? as usize,
            command
                .get("state_cap")
                .and_then(Value::as_u64)
                .unwrap_or(250_000) as usize,
            command
                .get("time_ms")
                .and_then(Value::as_u64)
                .unwrap_or(1_000),
        ),
        "residual_certificate" => {
            let id = command.get("id").and_then(Value::as_u64).ok_or("missing id")? as usize;
            let clip_fraction = command.get("clip_fraction").and_then(Value::as_f64).unwrap_or(1.0 / 1024.0);
            let node_limit = command.get("node_limit").and_then(Value::as_u64).unwrap_or(250_000) as usize;
            let time_ms = command.get("time_ms").and_then(Value::as_u64).unwrap_or(1_000);
            match suffix_options(command)? {
                Some(options) => engine.residual_certificate_with_options(
                    id, clip_fraction, node_limit, time_ms, Some(options),
                ),
                None => engine.residual_certificate(id, clip_fraction, node_limit, time_ms),
            }
        }
        "sample" => engine.sample(
            command
                .get("id")
                .and_then(Value::as_u64)
                .ok_or("missing id")? as usize,
            command.get("n").and_then(Value::as_u64).unwrap_or(128) as usize,
            command.get("seed").and_then(Value::as_i64).unwrap_or(1),
            command.get("depth").and_then(Value::as_u64).unwrap_or(6) as usize,
        ),
        "sample_hybrid" => engine.sample_hybrid_with_options(
            command
                .get("id")
                .and_then(Value::as_u64)
                .ok_or("missing id")? as usize,
            command.get("n").and_then(Value::as_u64).unwrap_or(128) as usize,
            command.get("seed").and_then(Value::as_i64).unwrap_or(1),
            command.get("depth").and_then(Value::as_u64).unwrap_or(6) as usize,
            command
                .get("clip_fraction")
                .and_then(Value::as_f64)
                .unwrap_or(1.0 / 1024.0),
            command.get("lazy_residual").and_then(Value::as_bool).unwrap_or(true),
        ),
        "quit" => Ok(json!({"quit": true})),
        _ => Err("unknown operation".into()),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = match args.next() {
        Some(v) => v,
        None => {
            eprintln!("usage: frontier-recovery REQUEST TEAM_ID POSITION");
            std::process::exit(2)
        }
    };
    let team_id: i32 = args.next().and_then(|x| x.parse().ok()).unwrap_or_else(|| {
        eprintln!("invalid TEAM_ID");
        std::process::exit(2)
    });
    let position: usize = args.next().and_then(|x| x.parse().ok()).unwrap_or_else(|| {
        eprintln!("invalid POSITION");
        std::process::exit(2)
    });
    let request: golaberto_odds::model::Request = match std::fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|s| serde_json::from_slice(&s).map_err(|e| e.to_string()))
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("request: {e}");
            std::process::exit(2)
        }
    };
    let mut engine = match Engine::new(request, team_id, position) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("engine: {e}");
            std::process::exit(2)
        }
    };
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    for line in stdin.lock().lines() {
        let start = Instant::now();
        let result = line
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<Value>(&s).map_err(|e| e.to_string()))
            .and_then(|v| {
                if v.get("op").and_then(Value::as_str) == Some("quit") {
                    Ok((json!({"quit":true}), true))
                } else {
                    receipt(&mut engine, &v).map(|r| (r, false))
                }
            });
        let value = match result {
            Ok((mut v, _)) => {
                if let Some(obj) = v.as_object_mut() {
                    obj.entry("command_elapsed_ms")
                        .or_insert(json!(start.elapsed().as_secs_f64() * 1000.0));
                }
                v
            }
            Err(e) => json!({"error":e}),
        };
        if writeln!(stdout, "{}", value).is_err() {
            break;
        }
        let _ = stdout.flush();
        if value.get("quit").and_then(Value::as_bool) == Some(true) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffix_option_parser_keeps_absent_mode_disabled_and_uses_defaults() {
        assert!(suffix_options(&json!({})).unwrap().is_none());
        let options = suffix_options(&json!({"suffix":{}})).unwrap().unwrap();
        assert_eq!(options.state_cap, 1_000_000);
        assert_eq!(options.max_remaining, 24);
        assert_eq!(options.probe_nodes, 25_000);
        assert_eq!(options.probe_ms, 25);
        assert_eq!(options.budget_fraction, 0.15);
        assert!(!options.canonicalize_certain);
        assert!(!options.joint_ratio);
        assert!(!options.score_gd);
        assert_eq!(options.score_setup_ms, 5_000);
        assert!(!options.shared_expectation);
        assert_eq!(options.expectation_bits, 8);
        assert_eq!(options.expectation_max_remaining, 18);
        assert_eq!(options.expectation_budget_fraction, 0.5);
        assert!(!options.expectation_score_gd);
        assert!(suffix_options(&json!({"suffix":{"joint_ratio":true}})).unwrap().unwrap().joint_ratio);
        assert!(suffix_options(&json!({"suffix":{"canonicalize_certain":true}})).unwrap().unwrap().canonicalize_certain);
    }

    #[test]
    fn suffix_option_parser_rejects_nonobjects_and_invalid_types() {
        assert!(suffix_options(&json!({"suffix":false})).is_err());
        assert!(suffix_options(&json!({"suffix":{"probe_nodes":-1}})).is_err());
        assert!(suffix_options(&json!({"suffix":{"budget_fraction":"small"}})).is_err());
        assert!(suffix_options(&json!({"suffix":{"canonicalize_certain":"true"}})).is_err());
        assert!(suffix_options(&json!({"suffix":{"joint_ratio":"true"}})).is_err());
        assert!(suffix_options(&json!({"suffix":{"score_gd":"true"}})).is_err());
        assert!(suffix_options(&json!({"suffix":{"score_setup_ms":0}})).is_err());
        assert!(suffix_options(&json!({"suffix":{"shared_expectation":"true"}})).is_err());
        assert!(suffix_options(&json!({"suffix":{"expectation_bits":21}})).is_err());
        assert!(suffix_options(&json!({"suffix":{"expectation_max_remaining":0}})).is_err());
        assert!(suffix_options(&json!({"suffix":{"expectation_budget_fraction":0}})).is_err());
        assert!(suffix_options(&json!({"suffix":{"expectation_score_gd":"true"}})).is_err());
        assert!(suffix_options(&json!({"suffix":{"expectation_score_gd":true}})).is_err());
        assert!(suffix_options(&json!({"suffix":{"shared_expectation":true,"expectation_score_gd":true}})).is_err());
        assert!(suffix_options(&json!({"suffix":{"score_gd":true,"expectation_score_gd":true}})).is_err());
        assert!(suffix_options(&json!({"suffix":{"shared_expectation":true,"score_gd":true,"expectation_score_gd":true}})).unwrap().unwrap().expectation_score_gd);
    }
}
