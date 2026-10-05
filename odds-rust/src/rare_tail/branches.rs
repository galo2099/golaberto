//! Complete branch refinement after native results are frozen.
//! Every branch retains positive final sampling allocation; bounds only order
//! work or reduce a tiny branch's draw floor, never truncate event support.
use super::*;
use crate::joint_caps::propagated::lazy::{combine_branches, BranchSample, BranchStrata};

const TOTAL: usize = 30000;
const PILOT: usize = 500;
const FLOOR: usize = 200;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct DrawAudit {
    pub credit: usize,
    pub valid: bool,
    pub reserved_draw_units: usize,
    pub actual_draw_units: usize,
    pub already_released_draw_units: usize,
}

impl DrawAudit {
    fn record(&mut self, reserved: usize, actual: usize, released: usize) {
        if released > reserved || actual > reserved.saturating_sub(released) {
            self.valid = false;
        }
        self.reserved_draw_units = self.reserved_draw_units.saturating_add(reserved);
        self.actual_draw_units = self.actual_draw_units.saturating_add(actual);
        self.already_released_draw_units =
            self.already_released_draw_units.saturating_add(released);
    }

    fn finish(&mut self) {
        self.credit = if self.valid {
            self.reserved_draw_units
                .saturating_sub(self.already_released_draw_units)
                .saturating_sub(self.actual_draw_units)
        } else {
            0
        };
    }
}

fn tree_pilot_release(pilot_cost: usize, setup_work: usize, actual_draw_units: usize) -> usize {
    pilot_cost.saturating_sub(setup_work.saturating_add(actual_draw_units))
}

fn fraction(value: &str) -> f64 {
    value
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && (0. ..=0.5).contains(v))
        .unwrap_or(0.15)
}

#[cfg(test)]
fn allocation(scores: &[f64], small: &[bool], total: usize) -> Option<Vec<usize>> {
    allocation_floor(scores, small, total, FLOOR)
}
fn allocation_floor(
    scores: &[f64],
    small: &[bool],
    total: usize,
    floor: usize,
) -> Option<Vec<usize>> {
    if scores.len() != small.len()
        || scores.is_empty()
        || scores.iter().any(|s| !s.is_finite() || *s < 0.)
    {
        return None;
    }
    let mut ns: Vec<_> = small.iter().map(|&s| if s { 2 } else { floor }).collect();
    let assigned: usize = ns.iter().sum();
    if assigned > total {
        return None;
    }
    let mut active: Vec<_> = (0..ns.len()).filter(|&i| !small[i]).collect();
    if active.is_empty() {
        active.extend(0..ns.len());
    }
    let sum: f64 = active.iter().map(|&i| scores[i]).sum();
    let left = total - assigned;
    for &i in &active {
        let share = if sum > 0. {
            0.1 / active.len() as f64 + 0.9 * scores[i] / sum
        } else {
            1. / active.len() as f64
        };
        ns[i] += (left as f64 * share).floor() as usize;
    }
    let remainder = total - ns.iter().sum::<usize>();
    for i in 0..remainder {
        ns[active[i % active.len()]] += 1;
    }
    Some(ns)
}

fn sample(
    p: &BranchStrata,
    m: &Model,
    ns: &[usize],
    guides: &[bool],
    order: &[usize],
    seed: i64,
    phase: &str,
    workers: usize,
) -> Vec<BranchSample> {
    let costs: Vec<_> = order
        .iter()
        .map(|&i| ns[i] * p.draw_work(m, i, guides[i]))
        .collect();
    let mut rows = budget::parallel(&costs, workers, |slot| {
        let i = order[slot];
        (
            i,
            p.sample(
                m,
                i,
                ns[i],
                derive(seed, &format!("branch-{phase}-{i}")),
                guides[i],
                true,
            ),
        )
    });
    rows.sort_by_key(|r| r.0);
    rows.into_iter().map(|r| r.1).collect()
}

// Early branch work is paid for by additional confirmations. A reduced
// confirmation override must also reduce the transferable branch allowance.
fn work_limit(configured: usize, available: Option<usize>) -> usize {
    available.map_or(configured, |funded| configured.min(funded))
}

pub(super) fn run(
    m: &Model,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    log: Option<&crate::logging::RequestLog>,
    preceding_ms: f64,
    tightened_only: bool,
    excluded: &[Cell],
    available_work: Option<usize>,
    tree_mode: bool,
) -> (usize, u64, usize, Vec<Cell>, Vec<Cell>, DrawAudit) {
    let floor = if tree_mode { 10 } else { FLOOR };
    let nominal_total = if tree_mode {
        3000
    } else if tightened_only {
        std::env::var("RUST_ODDS_CERTIFIED_BRANCH_DRAWS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|n| (10000..=TOTAL).contains(n))
            .unwrap_or(25000)
    } else {
        TOTAL
    };
    let pilot_draws = if tree_mode { 25 } else { PILOT };
    let start = Instant::now();
    let deterministic = budget::deterministic();
    let share = fraction(&value("RUST_ODDS_RARE_TAIL_BRANCH_BUDGET_FRACTION"));
    let allowance = preceding_ms * share;
    let mut draw_budget = WorkBudget::new(work_limit(
        budget::capacity(m, scaled(6 * REFERENCE_DRAWS, share)),
        available_work,
    ));
    let audit_enabled = super::target_overflow_tree_enabled() && budget::modeled();
    let mut audit = DrawAudit {
        valid: audit_enabled,
        ..DrawAudit::default()
    };
    let mut cells: Vec<_> = estimates
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            (e.probability == 0. && !e.reachability.starts_with("impossible")).then_some(Cell {
                team: i / m.n,
                rank: i % m.n,
            })
        })
        .filter(|c| !excluded.contains(c))
        .collect();
    if tree_mode {
        let mut counted: Vec<_> = cells
            .into_iter()
            .map(|c| (BranchStrata::tree_root_count(m, c, seed), c))
            .collect();
        counted.sort_by_key(|&(n, c)| (n, c.index(m.n)));
        cells = counted.into_iter().map(|(_, c)| c).collect();
    }
    let before: Vec<_> = estimates
        .iter()
        .enumerate()
        .filter(|(_, e)| e.probability > 0.)
        .map(|(i, e)| (i, serde_json::to_value(e).unwrap()))
        .collect();
    let reduce_floor = value("RUST_ODDS_RARE_TAIL_BRANCH_BOUND_FLOOR") == "1";
    let (mut found, mut work, mut attempted, mut skipped, mut setups_ms) = (0, 0, 0, 0, 0.);
    let refund_checks = tree_mode || tree_enabled();
    let mut reclaimed_check_work = 0;
    let mut handled = Vec::new();
    let mut tree_eligible = Vec::new();
    for cell in cells.iter().take(8).copied() {
        if (deterministic && draw_budget.reserved >= draw_budget.limit)
            || (!deterministic && start.elapsed().as_secs_f64() * 1000. >= allowance)
        {
            break;
        }
        attempted += 1;
        let setup = Instant::now();
        if crate::search::enabled("RUST_ODDS_CERTIFIED_TARGET_LIMITS") {
            let cost = 8 * m.fixtures.len() + 4 * m.n;
            let remaining = draw_budget.limit.saturating_sub(draw_budget.reserved);
            let node_budget = if budget::modeled() {
                remaining.saturating_sub(16 * m.fixtures.len()) / cost.max(1)
            } else {
                crate::target_limits::NODE_LIMIT
            }
            .min(crate::target_limits::NODE_LIMIT);
            let (c, setup_work) = crate::target_limits::certify_with_work(m, cell, node_budget);
            let charge = if budget::modeled() {
                setup_work
            } else {
                setup_work.div_ceil(budget::reference_cost(m))
            };
            if deterministic && !draw_budget.reserve(charge) {
                break;
            }
            emit(
                log,
                "rust_odds_certified_target_limits",
                json!({"team":m.ids[cell.team],"rank":cell.rank+1,"lower_packed":c.lower,"upper_packed":c.upper,"nodes":c.nodes,"reserved_work":charge,"elapsed_ms":setup.elapsed().as_secs_f64()*1000.}),
            );
            if tightened_only && c.lower.is_none() && c.upper.is_none() {
                continue;
            }
        }
        handled.push(cell);
        // Deterministic complete enumeration; no cached target path is omitted.
        let proposal = if tree_mode {
            BranchStrata::with_tree(m, cell, seed, 4, 64, 0, 4_000_000)
        } else {
            BranchStrata::with_secondary(m, cell, seed, 4, true)
        };
        let setup_ms = setup.elapsed().as_secs_f64() * 1000.;
        setups_ms += setup_ms;
        let mut p = match proposal {
            Ok(p) if p.len() * floor <= nominal_total => p,
            result => {
                skipped += 1;
                let reason = match result {
                    Err(reason) => {
                        if reason == "case enumeration budget exhausted" {
                            tree_eligible.push(cell);
                        }
                        reason
                    }
                    Ok(_) => "branch allocation floor exceeds draw budget".into(),
                };
                emit(
                    log,
                    "rust_odds_rare_tail_branches_skip",
                    json!({"group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,"setup_ms":setup_ms,"reason":reason}),
                );
                continue;
            }
        };
        if !deterministic && start.elapsed().as_secs_f64() * 1000. >= allowance {
            break;
        }
        // Reserve both pilots and complete main/check batches before sampling.
        // Branch count comes from complete enumeration, never pilot outcomes.
        let pilot_cost = if budget::modeled() {
            p.setup_work()
                + (0..p.len())
                    .map(|i| {
                        pilot_draws
                            * (if tree_mode {
                                p.draw_work(m, i, true)
                            } else {
                                p.draw_work(m, i, false) + p.draw_work(m, i, true)
                            })
                    })
                    .sum::<usize>()
        } else {
            2 * p.len() * pilot_draws + 2 * nominal_total
        };
        if deterministic && !draw_budget.reserve(pilot_cost) {
            skipped += 1;
            emit(
                log,
                "rust_odds_rare_tail_branches_skip",
                json!({
                    "group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,
                    "setup_ms":setup_ms,"branches":p.len(),
                    "requested_draws":2 * p.len() * pilot_draws + 2 * nominal_total,
                    "reason":"complete pilot pair exceeds remaining work budget"
                }),
            );
            continue;
        }
        let bound_clock = Instant::now();
        let bounds = p.bounds(m, false);
        let bounds_ms = bound_clock.elapsed().as_secs_f64() * 1000.;
        let mut order: Vec<_> = (0..p.len()).collect();
        order.sort_by(|&a, &b| {
            bounds[b]
                .upper_bound
                .total_cmp(&bounds[a].upper_bound)
                .then(a.cmp(&b))
        });
        let pilot_clock = Instant::now();
        let mut pilots = parallel(p.len(), workers, |i| {
            if tree_mode {
                let interval = p.sample(
                    m,
                    i,
                    pilot_draws,
                    derive(seed, &format!("branch-bound-pilot-{i}")),
                    true,
                    true,
                );
                let work = interval.result.work;
                let draw_units = interval.result.operations.units();
                return (interval, true, work, draw_units);
            }
            let rank = p.sample(
                m,
                i,
                pilot_draws,
                derive(seed, &format!("branch-pilot-{i}")),
                false,
                true,
            );
            let interval = p.sample(
                m,
                i,
                pilot_draws,
                derive(seed, &format!("branch-bound-pilot-{i}")),
                true,
                true,
            );
            let use_interval = interval.result.ess > rank.result.ess;
            let work = rank.result.work + interval.result.work;
            let draw_units = rank
                .result
                .operations
                .units()
                .saturating_add(interval.result.operations.units());
            (
                if use_interval { interval } else { rank },
                use_interval,
                work,
                draw_units,
            )
        });
        work += pilots.iter().map(|r| r.2).sum::<u64>();
        let pilot_actual = pilots.iter().fold(0usize, |sum, r| sum.saturating_add(r.3));
        let pilot_release = if tree_mode && budget::modeled() {
            tree_pilot_release(pilot_cost, p.setup_work(), pilot_actual)
        } else {
            0
        };
        if tree_mode && budget::modeled() {
            let actual = p.setup_work().saturating_add(pilot_actual);
            if actual < pilot_cost {
                assert!(draw_budget.release(pilot_cost - actual));
            }
        }
        if audit_enabled {
            audit.record(
                pilot_cost.saturating_sub(p.setup_work()),
                pilot_actual,
                pilot_release,
            );
        }
        if tree_mode && pilots.iter().all(|r| r.0.result.probability <= 0.) {
            skipped += 1;
            emit(
                log,
                "rust_odds_rare_tail_branches_skip",
                json!({"team":m.ids[cell.team],"rank":cell.rank+1,"reason":"no event evidence in complete tree pilots","pilot_draws":pilot_draws*p.len()}),
            );
            continue;
        }
        let mut message_info = serde_json::Value::Null;
        if tree_mode {
            let rs = pilots
                .iter()
                .map(|r| r.0.result.clone())
                .collect::<Vec<_>>();
            message_info = p.prepare_messages(m, &rs, 1);
            let message_work = message_info["work"].as_u64().unwrap() as usize;
            let selected: Vec<_> = message_info["selected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as usize)
                .collect();
            let re_pilot_work = selected
                .iter()
                .map(|&i| pilot_draws * p.draw_work(m, i, true))
                .sum::<usize>();
            if !draw_budget.reserve(message_work + re_pilot_work) {
                skipped += 1;
                continue;
            }
            let revised = parallel(selected.len(), workers, |slot| {
                let i = selected[slot];
                let r = p.sample(
                    m,
                    i,
                    pilot_draws,
                    derive(seed, &format!("message-pilot-{i}")),
                    true,
                    true,
                );
                let work = r.result.work;
                let draw_units = r.result.operations.units();
                (i, (r, true, work, draw_units))
            });
            let revised_actual = revised
                .iter()
                .fold(0usize, |sum, row| sum.saturating_add(row.1 .3));
            if audit_enabled {
                audit.record(re_pilot_work, revised_actual, 0);
            }
            for (i, row) in revised {
                work += row.2;
                pilots[i] = row;
            }
        }
        let pilot_ms = pilot_clock.elapsed().as_secs_f64() * 1000.;
        let pilot_probability: f64 = pilots.iter().map(|r| r.0.result.probability).sum();
        let small: Vec<_> = bounds
            .iter()
            .map(|b| {
                reduce_floor
                    && pilot_probability > 0.
                    && b.upper_bound <= pilot_probability * 0.001 / p.len() as f64
            })
            .collect();
        let scores: Vec<_> = pilots
            .iter()
            .map(|r| r.0.result.std_err * (r.0.result.samples as f64).sqrt())
            .collect();
        let mut total = nominal_total;
        let costs: Vec<_> = pilots
            .iter()
            .map(|r| budget::draw_cost(&r.0.result))
            .collect();
        if budget::modeled() {
            // Preserve the ordinary allocation and streams before spending
            // remaining units on a larger independent retry for cheap failures.
            while total >= p.len() * floor {
                let Some(ns) = allocation_floor(&scores, &small, total, floor) else {
                    break;
                };
                let cost: usize = ns.iter().zip(&costs).map(|(n, c)| 2 * n * c).sum();
                if draw_budget.reserve(cost) {
                    break;
                }
                total = total.saturating_sub(1000);
            }
            if total < p.len() * floor {
                skipped += 1;
                continue;
            }
        }
        let Some(ns) = allocation_floor(&scores, &small, total, floor) else {
            continue;
        };
        let guides: Vec<_> = pilots.iter().map(|r| r.1).collect();
        // Soft admission deadline; the entire fixed main/check pair runs after
        // admission. Never stop a reported batch based on its own results.
        let forecast =
            1.3 * pilot_ms * (2 * nominal_total) as f64 / (2 * p.len() * pilot_draws) as f64;
        if !deterministic && start.elapsed().as_secs_f64() * 1000. + forecast > allowance {
            emit(
                log,
                "rust_odds_rare_tail_branches_skip",
                json!({"group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,"setup_ms":setup_ms,"pilot_ms":pilot_ms,"forecast_ms":if deterministic {None}else{Some(forecast)},"reason":"final pair exceeds remaining admission allowance"}),
            );
            skipped += 1;
            continue;
        }
        let (main, check, main_ms, check_ms) = if tree_mode {
            let clock = Instant::now();
            let costs: Vec<_> = (0..2 * p.len())
                .map(|slot| {
                    let i = order[slot % p.len()];
                    ns[i] * p.draw_work(m, i, guides[i])
                })
                .collect();
            let mut rows = budget::parallel(&costs, workers, |slot| {
                let phase = if slot < p.len() { "main" } else { "check" };
                let i = order[slot % p.len()];
                (
                    slot / p.len(),
                    i,
                    p.sample(
                        m,
                        i,
                        ns[i],
                        derive(seed, &format!("branch-{phase}-{i}")),
                        guides[i],
                        true,
                    ),
                )
            });
            rows.sort_by_key(|r| (r.0, r.1));
            let mut rows: Vec<_> = rows.into_iter().map(|r| r.2).collect();
            let checks = rows.split_off(p.len());
            (
                combine_branches(&rows),
                combine_branches(&checks),
                clock.elapsed().as_secs_f64() * 1000.,
                0.,
            )
        } else {
            let main_clock = Instant::now();
            let mains = sample(&p, m, &ns, &guides, &order, seed, "main", workers);
            let main_ms = main_clock.elapsed().as_secs_f64() * 1000.;
            let main = combine_branches(&mains);
            let check_clock = Instant::now();
            let check = if publishable(&main, true) {
                combine_branches(&sample(&p, m, &ns, &guides, &order, seed, "check", workers))
            } else {
                Result::default()
            };
            let check_ms = check_clock.elapsed().as_secs_f64() * 1000.;
            if refund_checks && check.samples == 0 {
                let unused = ns.iter().zip(&costs).map(|(n, c)| n * c).sum();
                assert!(draw_budget.release(unused));
                reclaimed_check_work += unused;
            }
            if audit_enabled {
                let reserved = ns.iter().zip(&costs).fold(0usize, |sum, (n, c)| {
                    sum.saturating_add(2usize.saturating_mul(*n).saturating_mul(*c))
                });
                let released = if refund_checks && check.samples == 0 {
                    ns.iter().zip(&costs).fold(0usize, |sum, (n, c)| {
                        sum.saturating_add(n.saturating_mul(*c))
                    })
                } else {
                    0
                };
                audit.record(
                    reserved,
                    main.operations
                        .units()
                        .saturating_add(check.operations.units()),
                    released,
                );
            }
            (main, check, main_ms, check_ms)
        };
        if tree_mode && audit_enabled {
            let reserved = ns.iter().zip(&costs).fold(0usize, |sum, (n, c)| {
                sum.saturating_add(2usize.saturating_mul(*n).saturating_mul(*c))
            });
            audit.record(
                reserved,
                main.operations
                    .units()
                    .saturating_add(check.operations.units()),
                0,
            );
        }
        work += main.work + check.work;
        let ok = accepted(&main, &check, true);
        if ok && commit_estimate(&mut estimates[cell.index(m.n)], &main, true) {
            estimates[cell.index(m.n)].design = "matched_point_pool_complete_branches".into();
            estimates[cell.index(m.n)].work_spent +=
                pilots.iter().map(|r| r.2).sum::<u64>() + main.work + check.work;
            found += 1;
        }
        emit(
            log,
            "rust_odds_rare_tail_branches",
            json!({"group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,"branches":p.len(),"setup":p.setup_diagnostics(),"parallel_pair":tree_mode,"setup_ms":setup_ms,"bounds_ms":bounds_ms,"messages":message_info,"pilot_ms":pilot_ms,"main_ms":main_ms,"check_ms":check_ms,"pilot_probability":pilot_probability,"small_branch_floors":small.iter().filter(|&&s|s).count(),"allocated_main_draws":total,"estimated_cost_per_draw":costs,"main_draws":main.samples,"main_hits":main.hits,"main_ess":main.ess,"main_max_share":main.max_share,"main_batch_gap":main.batch_gap,"probability":main.probability,"check_probability":check.probability,"check_ess":check.ess,"accepted":ok,"full_support":true,"omitted_probability":0,"forecast_ms":if deterministic {None}else{Some(forecast)}}),
        );
        if !ok && budget::modeled() {
            let reference = budget::reference_cost(m);
            let average = costs.iter().sum::<usize>() / costs.len().max(1);
            let retry_total = (nominal_total * reference / average.max(reference / 3).max(1))
                .clamp(nominal_total, 3 * nominal_total);
            if let Some(retry_ns) = allocation_floor(&scores, &small, retry_total, floor) {
                let retry_cost: usize = retry_ns.iter().zip(&costs).map(|(n, c)| 2 * n * c).sum();
                if draw_budget.reserve(retry_cost) {
                    let clock = Instant::now();
                    let retry = combine_branches(&sample(
                        &p,
                        m,
                        &retry_ns,
                        &guides,
                        &order,
                        seed,
                        "retry-main",
                        workers,
                    ));
                    let check = if publishable(&retry, true) {
                        combine_branches(&sample(
                            &p,
                            m,
                            &retry_ns,
                            &guides,
                            &order,
                            seed,
                            "retry-check",
                            workers,
                        ))
                    } else {
                        Result::default()
                    };
                    let released = if refund_checks && check.samples == 0 {
                        retry_cost / 2
                    } else {
                        0
                    };
                    if refund_checks && check.samples == 0 {
                        assert!(draw_budget.release(released));
                        reclaimed_check_work += retry_cost / 2;
                    }
                    if audit_enabled {
                        audit.record(
                            retry_cost,
                            retry
                                .operations
                                .units()
                                .saturating_add(check.operations.units()),
                            released,
                        );
                    }
                    work += retry.work + check.work;
                    let accepted = accepted(&retry, &check, true);
                    if accepted && commit_estimate(&mut estimates[cell.index(m.n)], &retry, true) {
                        estimates[cell.index(m.n)].design =
                            "matched_point_pool_complete_branches_retry".into();
                        found += 1;
                    }
                    emit(
                        log,
                        "rust_odds_rare_tail_branches_retry",
                        json!({
                            "group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,
                            "allocated_main_draws":retry_total,"main_draws":retry.samples,
                            "check_draws":check.samples,"probability":retry.probability,
                            "check_probability":check.probability,"accepted":accepted,
                            "reserved_work":retry_cost,"elapsed_ms":clock.elapsed().as_secs_f64()*1000.
                        }),
                    );
                }
            }
        }
    }
    emit(
        log,
        "rust_odds_rare_tail_branches_summary",
        json!({"group":m.request.id,"tree_mode":tree_mode,"pilot_draws_per_branch":pilot_draws,"tightened_only":tightened_only,"eligible":cells.len(),"attempted":attempted,"skipped":skipped,"accepted":found,"elapsed_ms":start.elapsed().as_secs_f64()*1000.,"setup_ms":setups_ms,"allowance_ms":if deterministic {None}else{Some(allowance)},"budget_mode":budget::mode(),"work_limit":if deterministic {Some(draw_budget.limit)}else{None},"reclaimed_check_work":reclaimed_check_work,"reserved_work":if deterministic {Some(draw_budget.reserved)}else{None},"work":work,"bound_floor":reduce_floor,"preserved_existing_estimates":before.iter().all(|(i, e)| *e == serde_json::to_value(&estimates[*i]).unwrap())}),
    );
    if audit_enabled {
        audit.finish();
    }
    (
        found,
        work,
        draw_budget.reserved,
        handled,
        tree_eligible,
        audit,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draw_audit_credits_discarded_pilot_surplus() {
        let mut audit = DrawAudit {
            valid: true,
            ..DrawAudit::default()
        };
        audit.record(100, 70, 0);
        audit.finish();
        assert_eq!(audit.credit, 30);
        assert!(audit.valid);
    }
    #[test]
    fn tree_pilot_release_subtracts_setup_and_actual_draw_work() {
        assert_eq!(tree_pilot_release(150, 20, 100), 30);
        assert_eq!(tree_pilot_release(150, 20, 140), 0);
    }
    #[test]
    fn draw_audit_subtracts_skipped_check_release_once() {
        let mut audit = DrawAudit {
            valid: true,
            ..DrawAudit::default()
        };
        audit.record(200, 100, 100);
        audit.finish();
        assert_eq!(audit.credit, 0);
        assert_eq!(audit.already_released_draw_units, 100);
    }
    #[test]
    fn settled_tree_pilot_has_no_surplus_credit() {
        let mut audit = DrawAudit {
            valid: true,
            ..DrawAudit::default()
        };
        // The tree runner has already released its pilot surplus before this
        // phase is audited, so the effective reservation equals actual work.
        audit.record(80, 80, 0);
        audit.finish();
        assert_eq!(audit.credit, 0);
    }
    #[test]
    fn draw_audit_overrun_invalidates_all_credit() {
        let mut audit = DrawAudit {
            valid: true,
            ..DrawAudit::default()
        };
        audit.record(50, 60, 0);
        audit.record(100, 50, 0);
        audit.finish();
        assert!(!audit.valid);
        assert_eq!(audit.credit, 0);
    }
    #[test]
    fn released_reservation_cannot_mask_phase_overrun() {
        let mut audit = DrawAudit {
            valid: true,
            ..DrawAudit::default()
        };
        audit.record(100, 60, 50);
        audit.record(200, 100, 0);
        audit.finish();
        assert!(!audit.valid);
        assert_eq!(audit.credit, 0);
    }
    #[test]
    fn failed_setup_without_measured_phase_has_no_credit() {
        let mut audit = DrawAudit {
            valid: true,
            ..DrawAudit::default()
        };
        audit.finish();
        assert_eq!(audit.credit, 0);
        assert_eq!(audit.reserved_draw_units, 0);
    }

    #[test]
    fn transferred_work_cannot_exceed_its_funding() {
        assert_eq!(work_limit(90000, Some(30000)), 30000);
        assert_eq!(work_limit(90000, Some(200000)), 90000);
        assert_eq!(work_limit(90000, Some(0)), 0);
        assert_eq!(work_limit(90000, None), 90000);
    }
    #[test]
    fn allocation_preserves_every_branch_and_fixed_budget() {
        let ns = allocation(&[1., 2., 3.], &[false, false, false], 1000).unwrap();
        assert_eq!(ns.iter().sum::<usize>(), 1000);
        assert!(ns.iter().all(|&n| n >= FLOOR));
        let ns = allocation(&[1., 2., 3.], &[true, false, false], 1000).unwrap();
        assert_eq!(ns[0], 2);
        assert_eq!(ns.iter().sum::<usize>(), 1000);
        assert!(allocation(&[1., 2.], &[false, false], 399).is_none());
        assert!(allocation(&[], &[], 1000).is_none());
        assert!(allocation(&[1.], &[false, true], 1000).is_none());
        let ns = allocation(&[1., 2.], &[true, true], 1000).unwrap();
        assert_eq!(ns.iter().sum::<usize>(), 1000);
        assert!(ns.iter().all(|&n| n >= 2));
        assert!(allocation(&[f64::NAN], &[false], 1000).is_none());
        assert!(allocation(&[-1.], &[false], 1000).is_none());
    }
    #[test]
    fn bounded_fraction_has_explicit_zero_rollback() {
        assert_eq!(fraction("0"), 0.);
        assert_eq!(fraction("0.25"), 0.25);
        for value in ["", "NaN", "inf", "-1", "0.6"] {
            assert_eq!(fraction(value), 0.15);
        }
    }
    #[test]
    fn production_branch_stage_defaults_to_coverage_with_explicit_overrides() {
        assert_eq!(
            resolve_value("RUST_ODDS_RARE_TAIL_BRANCHES", None, true),
            "1"
        );
        assert_eq!(
            resolve_value("RUST_ODDS_RARE_TAIL_BRANCHES", Some("1".into()), true),
            "1"
        );
        assert_eq!(
            resolve_value("RUST_ODDS_RARE_TAIL_BRANCHES", Some("0".into()), true),
            "0"
        );
        assert_eq!(
            resolve_value("RUST_ODDS_RARE_TAIL_BRANCHES", None, false),
            ""
        );
        assert_eq!(
            resolve_value("RUST_ODDS_RARE_TAIL_BRANCH_BOUND_FLOOR", None, true),
            ""
        );
    }
}
