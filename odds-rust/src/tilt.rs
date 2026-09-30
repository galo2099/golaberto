use crate::{
    conditioned::{stride, Event, Result},
    lookahead::{self, Policy},
    model::Model,
    pool::{Bounds, Estimate},
    search::{apply, crosscheck, enabled, parallel, policy, Cell},
};
use std::{collections::HashSet, sync::Arc};
#[derive(Clone)]
struct Candidate {
    cell: Cell,
    event: Arc<Event>,
    tilt: f64,
    point_tilt: f64,
    ess: f64,
    hits: usize,
    proved: bool,
}
struct Pilot {
    candidate: Option<Candidate>,
    witness: Option<Vec<u8>>,
    work: u64,
}
fn pilot(model: &Model, bounds: &Bounds, cell: Cell, seed: i64, proved: bool) -> Pilot {
    let mut out = Pilot {
        candidate: None,
        witness: None,
        work: 0,
    };
    let Some(event) = Event::build(model, bounds, cell.team, cell.rank, &[], 20000).map(Arc::new)
    else {
        return out;
    };
    if event.mass <= 0. {
        return out;
    }
    let direction = if cell.rank > model.n / 2 { -1. } else { 1. };
    let mut best = Candidate {
        cell,
        event: event.clone(),
        tilt: 0.,
        point_tilt: 0.,
        ess: 0.,
        hits: 0,
        proved,
    };
    for (tilt, point) in [(3., 0.5), (8., 1.), (12., 1.)] {
        let point_tilt = direction * point;
        let stream = format!(
            "conditioned-point-tilt-pilot-{}-{}-{tilt}-{point_tilt}",
            model.ids[cell.team], cell.rank
        );
        let p = Policy {
            samples: 1000,
            seed: crate::rng::derive(seed, &stream),
            tilt,
            point_tilt,
            force_points: enabled("RARE_POSITION_WIN_AWARE_STABLE_SELECTION"),
            propagate: false,
        };
        let r = lookahead::sample(model, &event, cell.team, cell.rank, bounds, p);
        out.work += r.work;
        if out.witness.is_none() {
            out.witness = r.witness.clone();
        }
        if best.hits == 0 && r.hits > 0 || r.ess >= 10. && r.ess > best.ess {
            best.tilt = tilt;
            best.point_tilt = point_tilt;
            best.ess = r.ess;
            best.hits = r.hits;
        }
    }
    if best.hits > 0 && best.ess.is_finite() {
        out.candidate = Some(best);
    }
    out
}
fn verify(
    model: &Model,
    bounds: &Bounds,
    seed: i64,
    c: &Candidate,
    mut r: Result,
    mut valid: bool,
) -> (Result, bool, bool) {
    if !valid || c.tilt < 12. || !enabled("RARE_POSITION_POINT_TILT_CROSSCHECK") {
        return (r, valid, false);
    }
    let p = policy(
        model,
        c.cell,
        seed,
        "conditioned-point-tilt-crosscheck",
        50000,
        3.,
        0.5f64.copysign(c.point_tilt),
        false,
        false,
    );
    let check = lookahead::sample(model, &c.event, c.cell.team, c.cell.rank, bounds, p);
    let work = r.work;
    let check_work = check.work;
    let (selected, ok, corrected) = crosscheck(r, check);
    r = selected;
    valid = ok;
    if corrected {
        r.work += work;
    }
    r.work += check_work;
    (r, valid, corrected)
}
pub fn run(
    model: &Model,
    cells: &[Cell],
    bounds: &Bounds,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    recycled: usize,
) -> (usize, u64) {
    if !enabled("RARE_POSITION_CONDITIONED_POINT_TILT") {
        return (0, 0);
    }
    let include = enabled("RARE_POSITION_POINT_TILT_UNDECIDED");
    let mut proved = Vec::new();
    let mut undecided = Vec::new();
    for &cell in cells {
        let e = &estimates[cell.index(model.n)];
        if e.probability != 0. {
            continue;
        }
        if e.reachability == "reachable_by_construction" {
            proved.push(cell);
        } else if include && e.reachability == "undecided" {
            undecided.push(cell);
        }
    }
    let order = |a: &Cell, b: &Cell| {
        estimates[b.index(model.n)]
            .zero_hit_upper_95
            .total_cmp(&estimates[a.index(model.n)].zero_hit_upper_95)
            .then(model.ids[a.team].cmp(&model.ids[b.team]))
            .then(a.rank.cmp(&b.rank))
    };
    proved.sort_by(order);
    undecided.sort_by(order);
    let mut pilots = proved;
    pilots.extend(undecided);
    pilots.truncate(12);
    let outputs = parallel(pilots.len(), workers, |i| {
        pilot(
            model,
            bounds,
            pilots[i],
            seed,
            estimates[pilots[i].index(model.n)].reachability == "reachable_by_construction",
        )
    });
    let mut work = outputs.iter().map(|o| o.work).sum::<u64>();
    let mut ranked: Vec<_> = outputs.iter().filter_map(|p| p.candidate.clone()).collect();
    ranked.sort_by(|a, b| {
        b.ess
            .total_cmp(&a.ess)
            .then(b.hits.cmp(&a.hits))
            .then(model.ids[a.cell.team].cmp(&model.ids[b.cell.team]))
            .then(a.cell.rank.cmp(&b.cell.rank))
    });
    let mut candidates = Vec::new();
    let mut proved = 0;
    let mut undecided = 0;
    for c in &ranked {
        if c.proved && proved < 4 {
            candidates.push(c.clone());
            proved += 1;
        } else if include && !c.proved && undecided < 3 {
            candidates.push(c.clone());
            undecided += 1;
        }
    }
    let limit = 4 + if include { 3 } else { 0 };
    for c in &ranked {
        if candidates.len() == limit {
            break;
        }
        if (c.proved || include) && !candidates.iter().any(|x| x.cell == c.cell) {
            candidates.push(c.clone());
        }
    }
    let confirmed = parallel(candidates.len(), workers, |i| {
        let c = &candidates[i];
        let samples = if c.proved && c.ess < 10. && c.hits >= 3 {
            30000
        } else {
            15000
        };
        let p = policy(
            model,
            c.cell,
            seed,
            "conditioned-point-tilt-final",
            samples,
            c.tilt,
            c.point_tilt,
            false,
            false,
        );
        let mut result = lookahead::sample(model, &c.event, c.cell.team, c.cell.rank, bounds, p);
        if !result.coarse()
            && enabled("RARE_POSITION_WIN_AWARE_FALLBACK")
            && stride(model, "lookahead") > 1
        {
            let mut point = lookahead::sample(
                model,
                &c.event,
                c.cell.team,
                c.cell.rank,
                bounds,
                Policy {
                    force_points: true,
                    ..p
                },
            );
            point.work += result.work;
            result = point;
        }
        let valid = result.coarse();
        verify(model, bounds, seed, c, result, valid)
    });
    let mut witnesses = 0;
    for (c, (r, valid, corrected)) in candidates.iter().zip(confirmed) {
        work += r.work;
        if valid {
            apply(
                &mut estimates[c.cell.index(model.n)],
                &r,
                if corrected {
                    "matched_point_pool_conditioned_point_tilt_crosschecked"
                } else {
                    "matched_point_pool_conditioned_point_tilt"
                },
            );
            witnesses += 1;
        }
    }
    if enabled("RARE_POSITION_POINT_TILT_GAP_RESCUE") {
        let mut gaps: Vec<_> = cells
            .iter()
            .copied()
            .filter(|c| {
                c.rank > 0 && c.rank + 1 < model.n && {
                    let e = &estimates[c.index(model.n)];
                    e.probability == 0.
                        && e.reachability == "reachable_by_construction"
                        && estimates[c.index(model.n) - 1].probability > 0.
                        && estimates[c.index(model.n) + 1].probability > 0.
                }
            })
            .collect();
        gaps.sort_by(|a, b| {
            estimates[b.index(model.n)]
                .zero_hit_upper_95
                .total_cmp(&estimates[a.index(model.n)].zero_hit_upper_95)
                .then(model.ids[a.team].cmp(&model.ids[b.team]))
                .then(a.rank.cmp(&b.rank))
        });
        let mut budget = 60000;
        let mut attempted = 0;
        for cell in gaps {
            if attempted >= 2 || budget < 17000 {
                break;
            }
            let Some(event) = Event::build(model, bounds, cell.team, cell.rank, &[], 20000) else {
                continue;
            };
            if event.mass <= 0. {
                continue;
            }
            let point = if cell.rank > model.n / 2 { -0.5 } else { 0.5 };
            let gentle = lookahead::sample(
                model,
                &event,
                cell.team,
                cell.rank,
                bounds,
                policy(
                    model,
                    cell,
                    seed,
                    "split-gap-gentle-pilot",
                    1000,
                    3.,
                    point,
                    enabled("RARE_POSITION_WIN_AWARE_STABLE_SELECTION"),
                    false,
                ),
            );
            let moderate = lookahead::sample(
                model,
                &event,
                cell.team,
                cell.rank,
                bounds,
                policy(
                    model,
                    cell,
                    seed,
                    "split-gap-moderate-pilot",
                    1000,
                    8.,
                    2. * point,
                    enabled("RARE_POSITION_WIN_AWARE_STABLE_SELECTION"),
                    false,
                ),
            );
            budget -= gentle.samples + moderate.samples;
            work += gentle.work + moderate.work;
            let mut configs = [("gentle", 3., point), ("moderate", 8., 2. * point)];
            if gentle.hits <= 2 && moderate.hits >= 10 && moderate.ess >= 1.5 {
                configs.swap(0, 1);
            }
            attempted += 1;
            for (index, (name, tilt, point)) in configs.into_iter().enumerate() {
                if index == 1 && budget < 15000 {
                    break;
                }
                let r = lookahead::sample(
                    model,
                    &event,
                    cell.team,
                    cell.rank,
                    bounds,
                    policy(
                        model,
                        cell,
                        seed,
                        &format!("split-gap-{name}-final"),
                        15000,
                        tilt,
                        point,
                        false,
                        false,
                    ),
                );
                budget -= r.samples;
                work += r.work;
                if r.coarse() {
                    apply(
                        &mut estimates[cell.index(model.n)],
                        &r,
                        "matched_point_pool_conditioned_point_tilt_gap",
                    );
                    witnesses += 1;
                    break;
                }
            }
        }
    }
    if include && enabled("RARE_POSITION_POINT_TILT_UNDECIDED_GAP") {
        let mut gaps: Vec<_> = cells
            .iter()
            .copied()
            .filter(|c| {
                c.rank > 0 && c.rank + 1 < model.n && {
                    let e = &estimates[c.index(model.n)];
                    e.probability == 0.
                        && e.reachability == "undecided"
                        && estimates[c.index(model.n) - 1].probability > 0.
                        && estimates[c.index(model.n) + 1].probability > 0.
                }
            })
            .collect();
        gaps.sort_by(|a, b| {
            estimates[b.index(model.n)]
                .zero_hit_upper_95
                .total_cmp(&estimates[a.index(model.n)].zero_hit_upper_95)
                .then(model.ids[a.team].cmp(&model.ids[b.team]))
                .then(a.rank.cmp(&b.rank))
        });
        for cell in gaps {
            let Some(event) = Event::build(model, bounds, cell.team, cell.rank, &[], 20000) else {
                continue;
            };
            if event.mass <= 0. {
                continue;
            }
            let point = if cell.rank > model.n / 2 { -0.5 } else { 0.5 };
            let gentle = lookahead::sample(
                model,
                &event,
                cell.team,
                cell.rank,
                bounds,
                policy(
                    model,
                    cell,
                    seed,
                    "conditioned-point-tilt-undecided-gap-gentle-pilot",
                    1000,
                    3.,
                    point,
                    enabled("RARE_POSITION_WIN_AWARE_STABLE_SELECTION"),
                    false,
                ),
            );
            let moderate = lookahead::sample(
                model,
                &event,
                cell.team,
                cell.rank,
                bounds,
                policy(
                    model,
                    cell,
                    seed,
                    "conditioned-point-tilt-undecided-gap-moderate-pilot",
                    1000,
                    8.,
                    2. * point,
                    enabled("RARE_POSITION_WIN_AWARE_STABLE_SELECTION"),
                    false,
                ),
            );
            work += gentle.work + moderate.work;
            let mut configs = [("gentle", 3., point), ("moderate", 8., 2. * point)];
            if gentle.hits <= 2 && moderate.hits >= 25 && moderate.ess >= 3. {
                configs.swap(0, 1);
            }
            for (name, tilt, point) in configs {
                let r = lookahead::sample(
                    model,
                    &event,
                    cell.team,
                    cell.rank,
                    bounds,
                    policy(
                        model,
                        cell,
                        seed,
                        &format!("conditioned-point-tilt-undecided-gap-{name}-final"),
                        15000,
                        tilt,
                        point,
                        false,
                        false,
                    ),
                );
                work += r.work;
                let valid = r.coarse()
                    && (name != "moderate"
                        || r.ess >= 50. && r.max_share <= 0.05 && r.batch_gap <= 0.5);
                if valid {
                    apply(
                        &mut estimates[cell.index(model.n)],
                        &r,
                        "matched_point_pool_conditioned_point_tilt_undecided_gap",
                    );
                    witnesses += 1;
                    break;
                }
            }
            break;
        }
    }
    if recycled > 0 {
        for c in &ranked {
            if candidates.iter().any(|x| x.cell == c.cell)
                || estimates[c.cell.index(model.n)].probability > 0.
            {
                continue;
            }
            let result = lookahead::sample(
                model,
                &c.event,
                c.cell.team,
                c.cell.rank,
                bounds,
                policy(
                    model,
                    c.cell,
                    seed,
                    "conditioned-point-tilt-final",
                    recycled,
                    c.tilt,
                    c.point_tilt,
                    false,
                    false,
                ),
            );
            let valid = result.coarse();
            let (r, valid, corrected) = verify(model, bounds, seed, c, result, valid);
            work += r.work;
            if valid {
                apply(
                    &mut estimates[c.cell.index(model.n)],
                    &r,
                    if corrected {
                        "matched_point_pool_conditioned_point_tilt_recycled_crosschecked"
                    } else {
                        "matched_point_pool_conditioned_point_tilt_recycled"
                    },
                );
                witnesses += 1;
            }
            break;
        }
    }
    if enabled("RARE_POSITION_CROSS_TEAM_WITNESS") && enabled("RARE_POSITION_NEIGHBORHOOD_SEARCH") {
        let seeds: Vec<_> = outputs.into_iter().filter_map(|p| p.witness).collect();
        let proofs = crate::neighbors::search(model, cells, &seeds, estimates, 4000);
        crate::neighbors::mark(model, &proofs, estimates);
        let selected: HashSet<_> = pilots.into_iter().collect();
        let mut eligible: Vec<_> = proofs
            .into_iter()
            .map(|p| p.cell)
            .filter(|c| estimates[c.index(model.n)].probability == 0. && !selected.contains(c))
            .collect();
        eligible.sort_by(|a, b| {
            estimates[b.index(model.n)]
                .zero_hit_upper_95
                .total_cmp(&estimates[a.index(model.n)].zero_hit_upper_95)
                .then(model.ids[a.team].cmp(&model.ids[b.team]))
                .then(a.rank.cmp(&b.rank))
        });
        if let Some(cell) = eligible.first() {
            let p = pilot(model, bounds, *cell, seed, true);
            work += p.work;
            if let Some(c) = p.candidate {
                let r = lookahead::sample(
                    model,
                    &c.event,
                    c.cell.team,
                    c.cell.rank,
                    bounds,
                    policy(
                        model,
                        c.cell,
                        seed,
                        "cross-team-final",
                        15000,
                        c.tilt,
                        c.point_tilt,
                        false,
                        false,
                    ),
                );
                let valid = r.coarse();
                let (r, valid, corrected) = verify(model, bounds, seed, &c, r, valid);
                work += r.work;
                if valid {
                    apply(
                        &mut estimates[c.cell.index(model.n)],
                        &r,
                        if corrected {
                            "matched_point_pool_conditioned_point_tilt_cross_team_crosschecked"
                        } else {
                            "matched_point_pool_conditioned_point_tilt_cross_team"
                        },
                    );
                    witnesses += 1;
                }
            }
        }
    }
    (witnesses, work)
}
