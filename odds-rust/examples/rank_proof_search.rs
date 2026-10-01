//! Offline ablation of generic explained points/wins rank proofs.
use golaberto_odds::{
    model::{Model, Request},
    rank_proof::{Options, RankProof},
    search::Cell,
};
use std::{env, fs, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    if a.len() < 4 {
        return Err("usage: rank_proof_search REQUEST TEAM RANK [NODES] [plain|backjump|order|learn] [best]".into());
    }
    let started = Instant::now();
    let request: Request = serde_json::from_slice(&fs::read(&a[1])?)?;
    let model = Model::new(request)?;
    let team = model.indices[&a[2].parse::<i32>()?];
    let rank = a[3].parse::<usize>()? - 1;
    if rank >= model.n {
        return Err("invalid rank".into());
    }
    let setup = Instant::now();
    let p = RankProof::new(&model).ok_or("unsupported scoring or phase")?;
    let best = a.get(6).is_some_and(|v| v == "best");
    let p = if best { p.negated() } else { p };
    let setup_ms = setup.elapsed().as_secs_f64() * 1000.;
    let mode = a.get(5).map(String::as_str).unwrap_or("learn");
    let options = match mode {
        "plain" => Options {
            explain: false,
            internal_order: false,
            reuse_cohorts: false,
            require_root_pressure: false,
        },
        "backjump" => Options {
            explain: true,
            internal_order: false,
            reuse_cohorts: false,
            require_root_pressure: false,
        },
        "order" => Options {
            explain: true,
            internal_order: true,
            reuse_cohorts: false,
            require_root_pressure: false,
        },
        "learn" => Options::default(),
        "budgeted" => Options {
            require_root_pressure: true,
            ..Options::default()
        },
        _ => return Err("unknown mode".into()),
    };
    let search = Instant::now();
    let result = p.prove(
        Cell { team, rank },
        if best { model.n - 1 - rank } else { rank },
        a.get(4).map(|v| v.parse()).transpose()?.unwrap_or(500),
        options,
    );
    println!(
        "{}",
        serde_json::to_string(
            &serde_json::json!({"group":model.request.id,"team":model.ids[team],"rank":rank+1,"mode":mode,"direction":if best {"best"}else{"worst"},"result":result,"setup_ms":setup_ms,"search_ms":search.elapsed().as_secs_f64()*1000.,"total_ms":started.elapsed().as_secs_f64()*1000.})
        )?
    );
    Ok(())
}
