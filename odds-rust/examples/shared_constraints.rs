//! Offline audit; does not connect to the DB or change production defaults.
use golaberto_odds::{
    model::{Model, Request},
    search::Cell,
    shared_constraints,
};
use std::collections::BTreeMap;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let request: Request = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let m = Model::new(request).unwrap();
    let response: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    let mut cells = Vec::new();
    for (id, ranks) in response["rare_position_estimates"].as_object().unwrap() {
        let t = m.indices[&id.parse::<i32>().unwrap()];
        for (r, e) in ranks.as_object().unwrap() {
            if e["probability"].as_f64() == Some(0.)
                && !e["reachability"]
                    .as_str()
                    .unwrap_or("")
                    .starts_with("impossible")
            {
                cells.push(Cell {
                    team: t,
                    rank: r.parse().unwrap(),
                });
            }
        }
    }
    cells.sort_by_key(|c| c.index(m.n));
    let build = shared_constraints::build_config(
        &m,
        &cells,
        10.,
        args.get(4).is_some_and(|s| s == "guided"),
        args.get(5).and_then(|s| s.parse().ok()).unwrap_or(1),
        args.get(6).is_some_and(|s| s == "relative"),
    );
    let n = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(3000);
    let mut pilots = BTreeMap::new();
    for (i, g) in build.groups.iter().enumerate() {
        let start = std::time::Instant::now();
        let (sample, diagnostic) = g.sample_diagnostic(
            &m,
            n,
            808 + i as i64,
            args.get(4).is_none_or(|s| s != "blocker"),
        );
        pilots.insert(i, serde_json::json!({"proposal":g.describe(&m),"sampling_ms":start.elapsed().as_secs_f64()*1000.,"diagnostic":diagnostic,"results":sample.iter().map(|r|serde_json::json!({"probability":r.probability,"hits":r.hits,"ess":r.ess})).collect::<Vec<_>>() }));
    }
    println!(
        "{}",
        serde_json::json!({"group":m.request.id,"cells":cells.len(),"eligible":build.eligible,"setup_nodes":build.nodes,"groups":pilots})
    );
}
