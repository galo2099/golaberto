//! Request-scoped JSON lines on stderr; no work inside simulation loops.
use crate::pool::Estimate;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy)]
pub struct RequestLog {
    request_id: u64,
    group: Option<i32>,
    seed: Option<i64>,
    start: Instant,
    calculation_start: Instant,
    enabled: bool,
}

impl Default for RequestLog {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestLog {
    pub fn new() -> Self {
        let start = Instant::now();
        Self {
            request_id: NEXT_REQUEST.fetch_add(1, Ordering::Relaxed),
            group: None,
            seed: None,
            start,
            calculation_start: start,
            enabled: std::env::var("RUST_ODDS_LOG").as_deref() != Ok("0")
                || std::env::var("RUST_ODDS_PROFILE").as_deref() == Ok("1"),
        }
    }

    pub fn context(self, group: i32, seed: i64) -> Self {
        Self {
            group: Some(group),
            seed: Some(seed),
            ..self
        }
    }

    /// Calculation budgets exclude uploads and time waiting in the HTTP queue.
    pub fn calculating(mut self) -> Self {
        self.calculation_start = Instant::now();
        self
    }
    pub fn calculation_elapsed_ms(&self) -> f64 {
        millis(self.calculation_start)
    }
    pub fn elapsed_ms(&self) -> f64 {
        millis(self.start)
    }

    fn record(&self, event: &str, fields: Value) -> Value {
        let mut record = fields.as_object().cloned().unwrap_or_default();
        record.insert("event".into(), json!(event));
        record.insert("request_id".into(), json!(self.request_id));
        record.insert("timestamp_unix_ms".into(), json!(unix_ms()));
        record.insert("request_elapsed_ms".into(), json!(self.elapsed_ms()));
        if let Some(group) = self.group {
            record.insert("group".into(), json!(group));
        }
        if let Some(seed) = self.seed {
            record.insert("seed".into(), json!(seed));
        }
        Value::Object(record)
    }

    pub fn event(&self, event: &str, fields: Value) {
        if self.enabled {
            eprintln!("{}", self.record(event, fields));
        }
    }

    pub fn stage(&self, stage: &str, start: Instant, fields: Value) {
        if self.enabled {
            let mut fields = fields.as_object().cloned().unwrap_or_default();
            fields.insert("stage".into(), json!(stage));
            fields.insert("elapsed_ms".into(), json!(millis(start)));
            self.event("rust_odds_stage", Value::Object(fields));
        }
    }
}

pub fn millis(start: Instant) -> f64 {
    (start.elapsed().as_secs_f64() * 1_000_000.).round() / 1000.
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[derive(Default, Debug, Serialize)]
pub struct CellCounts {
    pub positive: usize,
    pub zeros: usize,
    pub impossible_zero: usize,
    pub reachable_zero: usize,
    pub undecided_zero: usize,
}

pub fn cell_counts(estimates: &[Estimate]) -> CellCounts {
    let mut counts = CellCounts::default();
    for e in estimates {
        if e.probability > 0. {
            counts.positive += 1;
        } else {
            counts.zeros += 1;
            if e.reachability.starts_with("impossible") {
                counts.impossible_zero += 1;
            } else if e.reachability == "witness" || e.reachability == "reachable_by_construction" {
                counts.reachable_zero += 1;
            } else {
                counts.undecided_zero += 1;
            }
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculation_budget_excludes_queue_time_without_losing_request_context() {
        let mut log = RequestLog::new().context(1, 808);
        log.start -= std::time::Duration::from_secs(1);
        let scoped = log.calculating();
        assert!(scoped.elapsed_ms() >= 1000.);
        assert!(scoped.calculation_elapsed_ms() < 100.);
        assert_eq!(scoped.request_id, log.request_id);
        assert_eq!(scoped.group, log.group);
        assert_eq!(scoped.seed, log.seed);
    }
    #[test]
    fn request_context_survives_each_stage_and_fields_are_structured() {
        let log = RequestLog::new().context(16653, 808);
        let first = log.record("start", json!({"teams":20}));
        let second = log.record("complete", json!({"message":"bad\nrequest"}));
        assert_eq!(first["request_id"], second["request_id"]);
        assert_eq!(second["group"], 16653);
        assert_eq!(second["seed"], 808);
        assert!(second["request_elapsed_ms"].as_f64().unwrap() >= 0.);
        assert!(second["timestamp_unix_ms"].as_u64().unwrap() > 0);
        assert!(!second.to_string().contains('\n'));
    }

    #[test]
    fn zero_summary_distinguishes_proofs_from_estimates() {
        let estimates: Vec<_> = [
            (0.1, ""),
            (0., "impossible_by_joint_points"),
            (0., "reachable_by_construction"),
            (0., "undecided"),
            (0., ""),
        ]
        .into_iter()
        .map(|(probability, reachability)| Estimate {
            probability,
            reachability: reachability.into(),
            ..Default::default()
        })
        .collect();
        let c = cell_counts(&estimates);
        assert_eq!((c.positive, c.zeros), (1, 4));
        assert_eq!(
            (c.impossible_zero, c.reachable_zero, c.undecided_zero),
            (1, 1, 2)
        );
    }
}
