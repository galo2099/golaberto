use golaberto_odds::{
    joint_caps::propagated::{Limits, PropagatedJoint},
    model::{Model, Request},
    search::{parallel, Cell},
};
use serde_json::{json, Value};
use std::{env, fs, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    let m = Model::new(serde_json::from_slice::<Request>(&fs::read(&a[1])?)?)?;
    let response: Value = serde_json::from_slice(&fs::read(&a[2])?)?;
    let mut cells = Vec::new();
    for (t, rs) in response["rare_position_estimates"].as_object().unwrap() {
        for (r, e) in rs.as_object().unwrap() {
            if e["probability"] == 0.
                && !e["reachability"]
                    .as_str()
                    .unwrap_or("")
                    .starts_with("impossible")
            {
                cells.push((
                    Cell {
                        team: m.indices[&t.parse::<i32>()?],
                        rank: r.parse::<usize>()?,
                    },
                    e.clone(),
                ));
            }
        }
    }
    let sample_count: usize = a.get(3).map(|v| v.parse()).transpose()?.unwrap_or(0);
    let results = parallel(cells.len(), 4, |i| {
        let (cell, e) = &cells[i];
        let mut variants = Vec::new();
        if sample_count > 0 {
            let start = Instant::now();
            if let Some(p) = golaberto_odds::joint_caps::propagated::lazy::LazyJoint::new(
                &m,
                *cell,
                808,
                &[],
                32,
            ) {
                let setup_ms = start.elapsed().as_secs_f64() * 1000.;
                let start = Instant::now();
                let r = p.sample(&m, sample_count, 808);
                variants.push(json!({"name":"lazy","setup_ms":setup_ms,"setup":p.describe(&m),"result":{"samples":r.samples,"hits":r.hits,"probability":r.probability,"ess":r.ess,"relative_se":if r.probability>0.{Some(r.std_err/r.probability)}else{None},"max_share":r.max_share,"accepted":r.coarse(),"sampling_ms":start.elapsed().as_secs_f64()*1000.}}));
            }
        }
        for (name, limits) in [
            ("default", Limits::default()),
            (
                "unforced",
                Limits {
                    minimum_forced: 0,
                    feasible_cases: 64,
                    ..Limits::default()
                },
            ),
            (
                "expanded",
                Limits {
                    minimum_forced: 0,
                    target_patterns: 256,
                    feasible_cases: 128,
                    ..Limits::default()
                },
            ),
        ] {
            let start = Instant::now();
            let p = PropagatedJoint::configured(&m, *cell, limits);
            let setup_ms = start.elapsed().as_secs_f64() * 1000.;
            match p {
                Ok(p) => {
                    let result = if sample_count > 0 {
                        let start = Instant::now();
                        let r = p.sample(&m, sample_count, 808, true);
                        Some(
                            json!({"samples":r.samples,"hits":r.hits,"probability":r.probability,"ess":r.ess,"relative_se":if r.probability>0.{Some(r.std_err/r.probability)}else{None},"max_share":r.max_share,"accepted":r.coarse(),"sampling_ms":start.elapsed().as_secs_f64()*1000.}),
                        )
                    } else {
                        None
                    };
                    variants.push(json!({"name":name,"setup_ms":setup_ms,"setup":p.describe(&m),"result":result}));
                }
                Err(reason) => {
                    variants.push(json!({"name":name,"reason":reason,"setup_ms":setup_ms}))
                }
            }
        }
        json!({"team":m.ids[cell.team],"rank":cell.rank+1,"reachability":e["reachability"],"upper":e["zero_hit_upper_95"],"variants":variants})
    });
    println!(
        "{}",
        json!({"group":m.request.id,"input":a[1],"sample_count":sample_count,"cells":results})
    );
    Ok(())
}
