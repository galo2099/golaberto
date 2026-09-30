use golaberto_odds::{
    model::{Model, Request},
    pool,
    rng::Rng,
};
use serde_json::json;
use std::{collections::BTreeMap, env, fs, time::Instant};
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() == 1 || args.get(1).is_some_and(|a| a == "serve") {
        let address = args.get(2).map(String::as_str).unwrap_or("127.0.0.1:6577");
        return golaberto_odds::http::serve(address);
    }
    if args.len() < 3 {
        return Err(
            "usage: golaberto-odds serve [ADDRESS] | estimate|oracle|bench|spi|eval|historic REQUEST.json [OUTPUT.json] [SEED] [WORKERS]"
                .into(),
        );
    }
    if ["spi", "eval", "historic"].contains(&args[1].as_str()) {
        let request: golaberto_odds::ratings::Request =
            serde_json::from_slice(&fs::read(&args[2])?)?;
        request.validate()?;
        let result = match args[1].as_str() {
            "spi" => serde_json::to_value(golaberto_odds::ratings::spi(
                &request.games,
                &request.initial(),
            )?)?,
            "eval" => serde_json::to_value(golaberto_odds::ratings::evaluate(&request)?)?,
            _ => serde_json::to_value(golaberto_odds::ratings::historical(&request)?)?,
        };
        let encoded = serde_json::to_vec(&result)?;
        if let Some(path) = args.get(3) {
            fs::write(path, encoded)?;
        } else {
            println!("{}", String::from_utf8(encoded)?);
        }
        return Ok(());
    }
    let request: Request = serde_json::from_slice(&fs::read(&args[2])?)?;
    if args[1] == "estimate" {
        let seed = args.get(4).map(|s| s.parse()).transpose()?.unwrap_or(808);
        let workers = args.get(5).map(|s| s.parse()).transpose()?.unwrap_or(4);
        let (result, timing) = golaberto_odds::api::calculate(request, seed, workers, 20000)?;
        eprintln!("{}", serde_json::to_string(&timing)?);
        let encoded = serde_json::to_vec(&result)?;
        if let Some(path) = args.get(3) {
            fs::write(path, encoded)?;
        } else {
            println!("{}", String::from_utf8(encoded)?);
        }
        return Ok(());
    }
    if args[1] == "bench" {
        let count = args
            .get(3)
            .map(|s| s.parse::<usize>())
            .transpose()?
            .unwrap_or(10);
        if count == 0 {
            return Err("benchmark iterations must be positive".into());
        }
        let mut durations = Vec::new();
        for i in 0..count {
            let (_, timing) = golaberto_odds::api::calculate(request.clone(), 808, 4, 20000)?;
            durations.push(timing.total_ms);
            eprintln!("iteration={i} {}", serde_json::to_string(&timing)?);
        }
        durations.sort_by(f64::total_cmp);
        println!("median_ms={}", durations[durations.len() / 2]);
        return Ok(());
    }
    let model = Model::new(request)?;
    let start = Instant::now();
    if args[1] != "oracle" {
        return Err(
            "unknown mode; expected estimate, bench, oracle, spi, eval, historic, or serve".into(),
        );
    }
    let mut rng = Rng::new(808);
    let floats: Vec<_> = (0..1000).map(|_| rng.float()).collect();
    let scout = pool::scout(&model, 1000, 808);
    let pmfs = pool::point_pmfs(&model).unwrap();
    let estimates = pool::production(&model, 808, 4);
    let mut scouts = BTreeMap::new();
    let mut marginal = BTreeMap::new();
    let mut result = BTreeMap::new();
    for t in 0..model.n {
        let id = model.ids[t];
        let mut counts = BTreeMap::new();
        let mut joint = BTreeMap::new();
        for p in 0..scout.span {
            let added = p as i32 + scout.offset;
            let row = scout.row(t, added, model.n);
            let count: usize = row.iter().sum();
            if count > 0 {
                counts.insert(added, count);
                joint.insert(added, row);
            }
        }
        scouts.insert(id,json!({"samples":1000,"rank_counts":scout.ranks[t*model.n..(t+1)*model.n],"point_counts":counts,"point_rank_counts":joint}));
        marginal.insert(id, pmfs[t].iter().copied().collect::<BTreeMap<_, _>>());
        result.insert(
            id,
            estimates[t * model.n..(t + 1) * model.n]
                .iter()
                .enumerate()
                .collect::<BTreeMap<_, _>>(),
        );
    }
    let bounds = pool::Bounds::new(&model);
    let mut kernels = BTreeMap::new();
    for t in 0..model.n {
        for rank in [0, 1, 2, 17, 18, 19] {
            if rank >= model.n {
                continue;
            }
            if let Some(event) =
                golaberto_odds::conditioned::Event::build(&model, &bounds, t, rank, &[], 20000)
            {
                if event.mass <= 0. {
                    continue;
                }
                for force_points in [false, true] {
                    for propagate in [false, true] {
                        let r = golaberto_odds::lookahead::sample(
                            &model,
                            &event,
                            t,
                            rank,
                            &bounds,
                            golaberto_odds::lookahead::Policy {
                                samples: 200,
                                seed: 808,
                                tilt: 3.,
                                point_tilt: 0.5,
                                force_points,
                                propagate,
                            },
                        );
                        kernels.insert(format!("{}-{rank}-{force_points}-{propagate}",model.ids[t]),json!({"mass":r.mass,"hits":r.hits,"samples":r.samples,"probability":r.probability,"std_err":r.std_err,"ess":r.ess,"max_share":r.max_share,"batch_gap":r.batch_gap,"omitted_draws":r.omitted_draws}));
                    }
                }
            }
        }
    }
    let output = json!({"floats":floats,"scout":scouts,"pmfs":marginal,"rare_position_estimates":result,"kernels":kernels});
    eprintln!("elapsed_ms={:.3}", start.elapsed().as_secs_f64() * 1000.);
    let encoded = serde_json::to_vec(&output)?;
    if let Some(path) = args.get(3) {
        fs::write(path, encoded)?;
    } else {
        println!("{}", String::from_utf8(encoded)?);
    }
    Ok(())
}
