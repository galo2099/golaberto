use golaberto_odds::{
    model::{Model, Request},
    pool,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, time::Instant};

const CASES: [(&str, &str); 5] = [
    ("group-99001-tight", "group-99001.reference.json"),
    ("group-99002-wide", "group-99002.reference.json"),
    ("group-99003-dominant", "group-99003.reference.json"),
    ("group-99004-weak", "group-99004.reference.json"),
    ("group-99005-blockers", "group-99005.reference.json"),
];
const SCOUT_SAMPLES: usize = 10_000;

#[derive(Deserialize)]
struct Reference {
    group: i32,
    input_sha256: String,
    samples: usize,
    seed: i64,
    cells: Vec<Cell>,
}
#[derive(Deserialize)]
struct Cell {
    team: i32,
    position: usize,
    count: usize,
    p: f64,
}

fn load(name: &str, reference_name: &str) -> (Vec<u8>, Request, Reference, Model) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/rare_position_benchmarks");
    let input = std::fs::read(dir.join(format!("{name}.json"))).unwrap();
    let request: Request = serde_json::from_slice(&input).unwrap();
    let reference: Reference =
        serde_json::from_slice(&std::fs::read(dir.join(reference_name)).unwrap()).unwrap();
    let model = Model::new(request.clone()).unwrap();
    (input, request, reference, model)
}

fn validate(name: &str, reference_name: &str) -> (Reference, Model) {
    let (input, request, reference, model) = load(name, reference_name);
    let case = format!("{} ({name})", request.id);
    let digest = format!("{:x}", Sha256::digest(&input));
    assert_eq!(reference.group, request.id, "{case}: reference group");
    assert_eq!(reference.input_sha256, digest, "{case}: input hash");
    assert_eq!(
        reference.samples, 5_000_000,
        "{case}: reference sample count"
    );
    assert_eq!(reference.seed, 72_991, "{case}: reference seed");
    assert_eq!(request.team_groups.len(), 20, "{case}: team count");
    assert_eq!(request.games.len(), 380, "{case}: schedule size");
    assert_eq!(
        request.games.iter().filter(|g| g.played).count(),
        50,
        "{case}: played games"
    );
    assert_eq!(model.fixtures.len(), 330, "{case}: remaining games");
    assert_eq!(request.phase.sort, "pt,gd,gf,bias", "{case}: tie rules");
    assert_eq!(
        request
            .team_groups
            .iter()
            .map(|t| t.team_id)
            .collect::<HashSet<_>>()
            .len(),
        20,
        "{case}: unique teams"
    );
    assert_eq!(
        request
            .team_groups
            .iter()
            .map(|team| team.bias)
            .collect::<HashSet<_>>()
            .len(),
        20,
        "{case}: unique deterministic tie biases"
    );

    let ids: HashSet<_> = request.team_groups.iter().map(|t| t.team_id).collect();
    assert_eq!(
        reference.cells.len(),
        400,
        "{case}: complete reference matrix"
    );
    let mut seen = HashSet::new();
    let mut row_sums = vec![0usize; 20];
    let mut column_sums = vec![0usize; 20];
    for cell in &reference.cells {
        assert!(
            ids.contains(&cell.team),
            "{case}: unknown reference team {}",
            cell.team
        );
        assert!(
            cell.position < 20,
            "{case}: invalid position {}",
            cell.position
        );
        assert!(
            seen.insert((cell.team, cell.position)),
            "{case}: duplicate cell team={} position={}",
            cell.team,
            cell.position
        );
        assert!(
            cell.count <= reference.samples,
            "{case}: cell count exceeds samples"
        );
        assert!(
            cell.p.is_finite() && (0.0..=1.0).contains(&cell.p),
            "{case}: invalid probability"
        );
        assert!(
            (cell.p - cell.count as f64 / reference.samples as f64).abs() < 1e-12,
            "{case}: count/probability mismatch team={} position={}",
            cell.team,
            cell.position
        );
        let row = request
            .team_groups
            .iter()
            .position(|t| t.team_id == cell.team)
            .unwrap();
        row_sums[row] += cell.count;
        column_sums[cell.position] += cell.count;
    }
    assert!(
        row_sums.iter().all(|&sum| sum == reference.samples),
        "{case}: reference row totals {row_sums:?}"
    );
    assert!(
        column_sums.iter().all(|&sum| sum == reference.samples),
        "{case}: reference column totals {column_sums:?}"
    );
    (reference, model)
}

#[test]
fn frozen_go_rare_position_references_are_complete_and_scout_regresses_them() {
    let started = Instant::now();
    for (name, reference_name) in CASES {
        let (reference, model) = validate(name, reference_name);
        // Keep the scout stream separate from the Go oracle seed.
        let scout = pool::scout(&model, SCOUT_SAMPLES, 808 + model.request.id as i64);
        let mut rows = vec![0usize; model.n];
        let mut columns = vec![0usize; model.n];
        for team in 0..model.n {
            for position in 0..model.n {
                let count = scout.ranks[team * model.n + position];
                rows[team] += count;
                columns[position] += count;
                let team_id = model.ids[team];
                let baseline = reference
                    .cells
                    .iter()
                    .find(|cell| cell.team == team_id && cell.position == position)
                    .unwrap();
                let p = baseline.p;
                // Independent Go (5M) and Rust (10k) Monte Carlo samples have binomial uncertainty.
                // Six combined standard errors plus a small finite-sample floor gives a conservative gate.
                let allowance = 6.0
                    * (p * (1.0 - p)
                        * (1.0 / SCOUT_SAMPLES as f64 + 1.0 / reference.samples as f64))
                        .sqrt()
                    + 3.0 / SCOUT_SAMPLES as f64;
                let actual = count as f64 / SCOUT_SAMPLES as f64;
                assert!((actual - p).abs() <= allowance,
                    "{} ({name}): team={team_id} rank={position} count={count}/{SCOUT_SAMPLES}, Go={}/{}, delta={:.6}, allowance={allowance:.6}",
                    model.request.id, baseline.count, reference.samples, (actual-p).abs());
            }
        }
        assert!(
            rows.iter().all(|&n| n == SCOUT_SAMPLES),
            "{} ({name}): scout row totals {rows:?}",
            model.request.id
        );
        assert!(
            columns.iter().all(|&n| n == SCOUT_SAMPLES),
            "{} ({name}): scout column totals {columns:?}",
            model.request.id
        );
    }
    eprintln!(
        "rare-position scout benchmark elapsed: {:.2?}",
        started.elapsed()
    );
}

#[test]
fn production_pool_is_valid_and_tracks_frozen_reference() {
    // Empirical regression gate on these fixed synthetic cases, not a guarantee that
    // the jackknife error captures pooling or shrinkage bias. Oracle zeros are not
    // treated as proof that an event is impossible.
    const SAMPLES: usize = 100_000;
    let mut designs = std::collections::BTreeMap::<String, usize>::new();
    for (name, reference_name) in CASES {
        let (reference, model) = validate(name, reference_name);
        let estimates = pool::production(&model, 808, 4);
        assert_eq!(
            estimates.len(),
            model.n * model.n,
            "{} ({name}): estimate matrix",
            model.request.id
        );
        let mut rows = vec![0.0; model.n];
        let mut columns = vec![0.0; model.n];
        for team in 0..model.n {
            for position in 0..model.n {
                let estimate = &estimates[team * model.n + position];
                assert!(
                    estimate.probability.is_finite() && (0.0..=1.0).contains(&estimate.probability),
                    "{} ({name}): invalid team={} rank={position}",
                    model.request.id,
                    model.ids[team]
                );
                assert!(
                    estimate.std_err.is_finite() && estimate.std_err >= 0.0,
                    "{} ({name}): invalid standard error",
                    model.request.id
                );
                *designs.entry(estimate.design.clone()).or_default() += 1;
                rows[team] += estimate.probability;
                columns[position] += estimate.probability;
                let baseline = reference
                    .cells
                    .iter()
                    .find(|cell| cell.team == model.ids[team] && cell.position == position)
                    .unwrap();
                if baseline.count >= 25 {
                    let allowance = 6.0
                        * (estimate.std_err.powi(2)
                            + baseline.p * (1.0 - baseline.p) / reference.samples as f64)
                            .sqrt()
                        + 3.0 / SAMPLES as f64;
                    assert!(
                        (estimate.probability - baseline.p).abs() <= allowance,
                        "{} ({name}): team={} rank={position} production={:.6} ± {:.6}, Go={:.6}, allowance={allowance:.6}",
                        model.request.id,
                        model.ids[team],
                        estimate.probability,
                        estimate.std_err,
                        baseline.p
                    );
                }
            }
        }
        for total in rows.iter().chain(&columns) {
            assert!(
                (*total - 1.0).abs() <= 1e-8,
                "{} ({name}): production marginal {total}",
                model.request.id
            );
        }
    }
    eprintln!("rare-position production designs: {designs:?}");
}
