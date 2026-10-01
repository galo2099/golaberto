use golaberto_odds::{
    conditioned::{Event, Result},
    joint_caps,
    lookahead::{sample, Policy},
    model::{Model, Request},
    pool::Bounds,
    search::{parallel, Cell},
};
use serde_json::json;
use std::{env, fs, time::Instant};
fn info(r: &Result) -> serde_json::Value {
    json!({"samples":r.samples,"hits":r.hits,"probability":r.probability,"std_err":r.std_err,"ess":r.ess,"relative_se":if r.probability>0.{Some(r.std_err/r.probability)}else{None},"max_share":r.max_share,"batch_gap":r.batch_gap,"accepted":r.coarse()})
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let a = env::args().collect::<Vec<_>>();
    let m = Model::new(serde_json::from_slice::<Request>(&fs::read(&a[1])?)?)?;
    if a.get(2).is_some_and(|v| v == "scan") {
        for t in 0..m.n {
            for rank in 0..m.n {
                for dual in [false, true] {
                    let start = Instant::now();
                    if let Some(p) = joint_caps::JointCaps::new(&m, Cell { team: t, rank }, dual, 4)
                    {
                        println!(
                            "{}",
                            json!({"setup_ms":start.elapsed().as_secs_f64()*1000.,"setup":p.describe(&m)})
                        );
                    }
                }
            }
        }
        return Ok(());
    }
    let team = a[2].parse::<i32>()?;
    let rank = a[3].parse::<usize>()? - 1;
    let count = a[4].parse()?;
    let samples = a[5].parse()?;
    let cell = Cell {
        team: m.indices[&team],
        rank,
    };
    let dual = a.get(6).is_some_and(|s| s == "worst");
    let seeds = [801, 804, 808, 817, 818, 911];
    let setup = Instant::now();
    let broad = a.get(6).is_some_and(|s| s == "broad");
    let propagated = a.get(6).is_some_and(|s| s == "propagated");
    let p = if propagated {
        joint_caps::JointProposal::Propagated(
            joint_caps::propagated::PropagatedJoint::new(&m, cell)
                .ok_or("unsupported propagated model")?,
        )
    } else if broad {
        joint_caps::JointProposal::Broad(
            joint_caps::broad::BroadJoint::new(&m, cell).ok_or("unsupported broad model")?,
        )
    } else {
        joint_caps::JointProposal::Exact(
            joint_caps::JointCaps::new(&m, cell, dual, count)
                .ok_or("unsupported or not a forced-target cap cell")?,
        )
    };
    let setup_ms = setup.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    let results = parallel(seeds.len(), 4, |i| {
        let start = Instant::now();
        let r = p.sample(&m, samples, seeds[i], true);
        json!({"seed":seeds[i],"elapsed_ms":start.elapsed().as_secs_f64()*1000.,"result":info(&r)})
    });
    println!(
        "{}",
        json!({"group":m.request.id,"setup_ms":setup_ms,"batch_wall_ms":start.elapsed().as_secs_f64()*1000.,"setup":p.describe(&m),"runs":results})
    );
    if count == 0 && !dual && !broad && !propagated {
        let b = Bounds::new(&m);
        let e = Event::build(&m, &b, cell.team, cell.rank, &[], 20000).unwrap();
        let start = Instant::now();
        let r = parallel(seeds.len(), 4, |i| {
            let start = Instant::now();
            let r = sample(
                &m,
                &e,
                cell.team,
                cell.rank,
                &b,
                Policy {
                    samples,
                    seed: seeds[i],
                    tilt: 6.,
                    point_tilt: 0.,
                    force_points: false,
                    propagate: true,
                },
            );
            json!({"seed":seeds[i],"elapsed_ms":start.elapsed().as_secs_f64()*1000.,"result":info(&r)})
        });
        println!(
            "{}",
            json!({"variant":"existing_sampler","group":m.request.id,"batch_wall_ms":start.elapsed().as_secs_f64()*1000.,"event_mass":e.mass,"runs":r})
        );
    }
    Ok(())
}
