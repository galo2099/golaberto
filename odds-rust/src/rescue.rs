use crate::{
    conditioned::{stride, Backward, Event, Result},
    domains::Domains,
    lookahead::{self, RankGame},
    model::{Key, Model},
    pool::{Bounds, Estimate},
    rng::{derive, Rng},
    search::{apply, crosscheck, enabled, parallel, policy, Cell},
};
use std::{collections::BTreeMap, sync::Arc};
fn current_ranks(model: &Model) -> (Vec<usize>, Vec<usize>) {
    let mut order = vec![0; model.n];
    model.standings(
        &mut order,
        &model.base,
        &model.empty_scores(),
        &mut Rng::new(1),
    );
    let mut rank = vec![0; model.n];
    for (r, &t) in order.iter().enumerate() {
        rank[t] = r;
    }
    (order, rank)
}
pub fn uniform(model: &Model, event: &Event, cell: Cell, bounds: &Bounds) -> bool {
    let stride = stride(model, "lookahead");
    let base_wins = if stride > 1 {
        model.base[cell.team].wins
    } else {
        0
    };
    let mut ranges = BTreeMap::from([(0, (0, 0))]);
    if stride > 1 {
        for g in &event.games {
            let mut next = BTreeMap::new();
            let f = &model.fixtures[g.index];
            for (points, (lo, hi)) in ranges {
                for o in 0..3 {
                    if g.prob[o] <= 0. {
                        continue;
                    }
                    let win =
                        i32::from(f.home == cell.team && o == 2 || f.away == cell.team && o == 0);
                    let added = points + (g.delta[o] & 255) as i32;
                    let entry = next.entry(added).or_insert((lo + win, hi + win));
                    entry.0 = entry.0.min(lo + win);
                    entry.1 = entry.1.max(hi + win);
                }
            }
            ranges = next;
        }
    }
    let mut min = i32::MAX;
    let mut max = i32::MIN;
    for &(state, _) in &event.terminals {
        let added = (state & 255) as i32;
        let (lo, hi) = if stride > 1 { ranges[&added] } else { (0, 0) };
        let p = (bounds.current[cell.team] + added) * stride + base_wins;
        min = min.min(p + lo);
        max = max.max(p + hi);
    }
    let mut left = vec![0; model.ids.len()];
    for f in &model.fixtures {
        left[f.home] += 1;
        left[f.away] += 1;
    }
    let mut above = 0;
    let mut below = 0;
    for t in 0..model.n {
        if t == cell.team {
            continue;
        }
        let mut lo = bounds.min[t] * stride;
        let mut hi = bounds.max[t] * stride;
        if stride > 1 {
            lo += model.base[t].wins;
            hi += model.base[t].wins + left[t];
        }
        above += usize::from(lo > max);
        below += usize::from(hi < min);
    }
    above == cell.rank || below == model.n - 1 - cell.rank
}
#[derive(Clone)]
struct Candidate {
    cell: Cell,
    event: Arc<Event>,
    point: f64,
    pilot: Result,
    upper: f64,
}
pub fn domains(
    model: &Model,
    bounds: &Bounds,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
) -> (usize, u64) {
    if !enabled("RARE_POSITION_PROPAGATED_DOMAINS") || model.keys.first() != Some(&Key::Pt) {
        return (0, 0);
    }
    let probes = crate::domains::ProbeConfig {
        checks: std::env::var("RUST_ODDS_DOMAIN_PROBES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        nodes: std::env::var("RUST_ODDS_DOMAIN_PROBE_NODES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(16),
    };
    let reallocate = std::env::var("RUST_ODDS_DOMAIN_PROBE_REALLOCATE").as_deref() == Ok("1");
    let (_, current) = current_ranks(model);
    let mut cells = Vec::new();
    for t in 0..model.n {
        for r in 0..model.n {
            let e = &estimates[t * model.n + r];
            if e.probability == 0. && !e.reachability.starts_with("impossible") {
                cells.push(Cell { team: t, rank: r });
            }
        }
    }
    cells.sort_by(|a, b| {
        estimates[b.index(model.n)]
            .zero_hit_upper_95
            .total_cmp(&estimates[a.index(model.n)].zero_hit_upper_95)
            .then(model.ids[a.team].cmp(&model.ids[b.team]))
            .then(a.rank.cmp(&b.rank))
    });
    cells.truncate(24);
    let pilots = parallel(cells.len(), workers, |i| {
        let cell = cells[i];
        let event = Arc::new(Event::build(
            model,
            bounds,
            cell.team,
            cell.rank,
            &[],
            20000,
        )?);
        if event.mass <= 0. || !uniform(model, &event, cell, bounds) {
            return None;
        }
        let point = if cell.rank > current[cell.team] {
            -0.5
        } else {
            0.5
        };
        let pilot = lookahead::sample_with_probes(
            model,
            &event,
            cell.team,
            cell.rank,
            bounds,
            policy(
                model,
                cell,
                seed,
                "rank-domain-pilot",
                if reallocate { 750 } else { 1000 },
                6.,
                point,
                false,
                true,
            ),
            probes,
        );
        Some(Candidate {
            cell,
            event,
            point,
            pilot,
            upper: 0.,
        })
    });
    let mut work = 0;
    let mut finalists = Vec::new();
    for c in pilots.into_iter().flatten() {
        work += c.pilot.work;
        if c.pilot.hits > 0 && estimates[c.cell.index(model.n)].reachability == "undecided" {
            estimates[c.cell.index(model.n)].reachability = "reachable_by_construction".into();
        }
        if c.pilot.hits >= 3 && c.pilot.ess >= 3. {
            finalists.push(c);
        }
    }
    finalists.sort_by(|a, b| {
        b.pilot
            .ess
            .total_cmp(&a.pilot.ess)
            .then(model.ids[a.cell.team].cmp(&model.ids[b.cell.team]))
            .then(a.cell.rank.cmp(&b.cell.rank))
    });
    finalists.truncate(3);
    let confirmed = parallel(finalists.len(), workers, |i| {
        let c = &finalists[i];
        let r = lookahead::sample_with_probes(
            model,
            &c.event,
            c.cell.team,
            c.cell.rank,
            bounds,
            policy(
                model,
                c.cell,
                seed,
                "rank-domain-final",
                if reallocate { 14000 } else { 15000 },
                6.,
                c.point,
                false,
                true,
            ),
            probes,
        );
        let check = lookahead::sample_with_probes(
            model,
            &c.event,
            c.cell.team,
            c.cell.rank,
            bounds,
            policy(
                model,
                c.cell,
                seed,
                "rank-domain-check",
                5000,
                3.,
                c.point,
                false,
                true,
            ),
            probes,
        );
        (r, check)
    });
    let mut accepted = 0;
    for (c, (r, check)) in finalists.iter().zip(confirmed) {
        work += r.work + check.work;
        let (selected, valid) = if check.coarse() {
            (check, true)
        } else if !r.coarse() {
            (r, false)
        } else {
            let (r, v, _) = crosscheck(r, check);
            (r, v)
        };
        if valid {
            apply(
                &mut estimates[c.cell.index(model.n)],
                &selected,
                "matched_point_pool_conditioned_rank_domains",
            );
            accepted += 1;
        }
    }
    (accepted, work)
}
fn score(model: &Model, event: &Event, cell: Cell, seed: i64) -> (f64, u64) {
    let stride = stride(model, "lookahead");
    let rules = &model.request.phase.championship;
    let hg = [
        rules.point_loss * stride,
        rules.point_draw * stride,
        rules.point_win * stride + if stride > 1 { 1 } else { 0 },
    ];
    let ag = [hg[2], hg[1], hg[0]];
    let base: Vec<_> = model
        .base
        .iter()
        .map(|c| c.points * stride + if stride > 1 { c.wins } else { 0 })
        .collect();
    let mut selected = Vec::new();
    let mut remaining = Vec::new();
    let mut options = 0;
    for (i, f) in model.fixtures.iter().enumerate() {
        let g = RankGame {
            index: i,
            home: f.home,
            away: f.away,
            hg,
            ag,
            prob: f.prob,
        };
        if f.home == cell.team || f.away == cell.team {
            selected.push(g);
        } else {
            options += g.prob.iter().filter(|p| **p > 0.).count();
            remaining.push(g);
        }
    }
    if options == 0 {
        return (0., 0);
    }
    let rivals: Vec<_> = (0..model.n).filter(|t| *t != cell.team).collect();
    let mut backwards = Backward::new(event);
    let mut rng = Rng::new(seed);
    let mut outcomes = vec![0; model.fixtures.len()];
    let mut points = base.clone();
    let mut removed = 0;
    for _ in 0..32 {
        backwards.sample(&mut rng, &mut outcomes);
        points.copy_from_slice(&base);
        for g in &selected {
            let o = outcomes[g.index] as usize;
            points[g.home] += g.hg[o];
            points[g.away] += g.ag[o];
        }
        let domains =
            Domains::propagate(&remaining, &points, &rivals, cell.rank, points[cell.team]);
        if !domains.feasible {
            continue;
        }
        let retained: usize = domains
            .domains
            .iter()
            .map(|d| d.count_ones() as usize)
            .sum();
        removed += options - retained;
    }
    (
        removed as f64 / (options * 32) as f64,
        (model.fixtures.len() * 32) as u64,
    )
}
fn constraint(
    model: &Model,
    bounds: &Bounds,
    seed: i64,
    current: &[usize],
    mut targets: Vec<Candidate>,
    estimates: &mut [Estimate],
) -> (usize, u64) {
    targets.sort_by(|a, b| {
        b.upper
            .total_cmp(&a.upper)
            .then(b.event.mass.total_cmp(&a.event.mass))
            .then(model.ids[a.cell.team].cmp(&model.ids[b.cell.team]))
            .then(a.cell.rank.cmp(&b.cell.rank))
    });
    let mut best = None;
    let mut best_score = 0.;
    let mut work = 0;
    let mut screened = 0;
    for c in targets {
        if estimates[c.cell.index(model.n)].probability > 0.
            || !uniform(model, &c.event, c.cell, bounds)
        {
            continue;
        }
        if screened == 8 {
            break;
        }
        screened += 1;
        let stream = derive(
            seed,
            &format!(
                "constraint-peer-probe-{}-{}",
                model.ids[c.cell.team], c.cell.rank
            ),
        );
        let (score, spent) = score(model, &c.event, c.cell, stream);
        work += spent;
        if score > best_score {
            best = Some(c);
            best_score = score;
        }
    }
    let Some(mut c) = best else {
        return (0, work);
    };
    c.point = if c.cell.rank > current[c.cell.team] {
        -0.5
    } else {
        0.5
    };
    let pilot = lookahead::sample(
        model,
        &c.event,
        c.cell.team,
        c.cell.rank,
        bounds,
        policy(
            model,
            c.cell,
            seed,
            "directional-peer-pilot",
            5000,
            3.,
            c.point,
            false,
            true,
        ),
    );
    work += pilot.work;
    if pilot.hits == 0 {
        return (0, work);
    }
    let est = &mut estimates[c.cell.index(model.n)];
    if est.reachability == "undecided" || est.reachability.is_empty() {
        est.reachability = "reachable_by_construction".into();
    }
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
            "directional-peer-rescue",
            45000,
            3.,
            c.point,
            false,
            true,
        ),
    );
    work += r.work;
    if r.coarse() {
        apply(
            &mut estimates[c.cell.index(model.n)],
            &r,
            "matched_point_pool_conditioned_constraint_peer",
        );
        (1, work)
    } else {
        (0, work)
    }
}
pub fn peers(
    model: &Model,
    bounds: &Bounds,
    seed: i64,
    _workers: usize,
    estimates: &mut [Estimate],
) -> (usize, u64) {
    if !enabled("RARE_POSITION_DIRECTIONAL_PEER_RESCUE") {
        return (0, 0);
    }
    let (standing, current) = current_ranks(model);
    let mut targets = Vec::new();
    for &t in &standing {
        let from = current[t];
        for r in 0..model.n {
            let e = &estimates[t * model.n + r];
            if r == from || e.probability > 0. || e.reachability.starts_with("impossible") {
                continue;
            }
            let peer = standing.iter().any(|o| {
                let p = estimates[o * model.n + r].probability;
                p > 0.
                    && p < 1e-6
                    && (r > from && current[*o] < from || r < from && current[*o] > from)
            });
            if !peer {
                continue;
            }
            let Some(event) = Event::build(model, bounds, t, r, &[], 20000) else {
                continue;
            };
            if event.mass < 1e-4 {
                continue;
            }
            targets.push(Candidate {
                cell: Cell { team: t, rank: r },
                event: Arc::new(event),
                point: 0.,
                pilot: Result::default(),
                upper: e.zero_hit_upper_95,
            });
        }
    }
    let best = targets
        .iter()
        .max_by(|a, b| {
            a.upper
                .total_cmp(&b.upper)
                .then(a.event.mass.total_cmp(&b.event.mass))
                .then(model.ids[b.cell.team].cmp(&model.ids[a.cell.team]))
                .then(b.cell.rank.cmp(&a.cell.rank))
        })
        .cloned();
    let Some(mut c) = best else {
        return (0, 0);
    };
    c.point = if c.cell.rank > current[c.cell.team] {
        -0.5
    } else {
        0.5
    };
    let pilot = lookahead::sample(
        model,
        &c.event,
        c.cell.team,
        c.cell.rank,
        bounds,
        policy(
            model,
            c.cell,
            seed,
            "directional-peer-pilot",
            5000,
            3.,
            c.point,
            false,
            false,
        ),
    );
    let mut work = pilot.work;
    if pilot.hits == 0 {
        return (0, work);
    }
    let est = &mut estimates[c.cell.index(model.n)];
    if est.reachability == "undecided" || est.reachability.is_empty() {
        est.reachability = "reachable_by_construction".into();
    }
    let mut result = lookahead::sample(
        model,
        &c.event,
        c.cell.team,
        c.cell.rank,
        bounds,
        policy(
            model,
            c.cell,
            seed,
            "directional-peer-rescue",
            50000,
            3.,
            c.point,
            false,
            false,
        ),
    );
    work += result.work;
    let mut accepted = result.coarse();
    let mut retried = false;
    let mut without_gap = result.clone();
    without_gap.batch_gap = 0.;
    if !accepted
        && result.batch_gap > 1.
        && without_gap.coarse()
        && enabled("RARE_POSITION_DIRECTIONAL_PEER_RETRY")
    {
        let retry = lookahead::sample(
            model,
            &c.event,
            c.cell.team,
            c.cell.rank,
            bounds,
            policy(
                model,
                c.cell,
                seed,
                "directional-peer-fallback",
                50000,
                3.,
                c.point,
                false,
                false,
            ),
        );
        work += retry.work;
        retried = true;
        if retry.coarse() {
            let (r, valid, _) = crosscheck(retry, result);
            result = r;
            accepted = valid;
        }
    }
    if accepted {
        apply(
            &mut estimates[c.cell.index(model.n)],
            &result,
            "matched_point_pool_conditioned_point_tilt_peer",
        );
        if !retried && enabled("RARE_POSITION_CONSTRAINT_PEER_RESCUE") {
            let (found, spent) = constraint(model, bounds, seed, &current, targets, estimates);
            return (1 + found, work + spent);
        }
        (1, work)
    } else {
        (0, work)
    }
}
