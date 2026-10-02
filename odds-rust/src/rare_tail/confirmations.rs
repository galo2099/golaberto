//! Two deterministic waves reuse skipped checks without a time-dependent race.
//! Main lengths remain fixed from independent training. Speculative mains never
//! publish alone; all reported estimates require a fresh independent check.
use super::*;

type Proposal = (Cell, Plan, Result, f64, u64, f64);
struct Job<'a> {
    p: &'a Proposal,
    n: usize,
    draw_cost: usize,
}
impl Job<'_> {
    fn main(&self, m: &Model, seed: i64) -> Result {
        self.p.1.sample(
            m,
            self.n,
            derive(
                seed,
                &format!(
                    "rare-tail-confirm-more-main-{}-{}",
                    m.ids[self.p.0.team], self.p.0.rank
                ),
            ),
        )
    }
    fn check(&self, m: &Model, seed: i64, main: &Result, rough: bool) -> Result {
        if !publishable(main, rough) {
            return Result::default();
        }
        self.p.1.sample(
            m,
            self.n,
            derive(
                seed,
                &format!(
                    "rare-tail-confirm-more-check-{}-{}",
                    m.ids[self.p.0.team], self.p.0.rank
                ),
            ),
        )
    }
}

pub(super) fn run(
    m: &Model,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    proposals: &[&Proposal],
    rough: bool,
    fraction: f64,
    log: Option<&crate::logging::RequestLog>,
) -> (usize, u64) {
    let clock = Instant::now();
    let before: Vec<_> = estimates
        .iter()
        .enumerate()
        .filter(|(_, e)| e.probability > 0.)
        .map(|(i, e)| (i, serde_json::to_value(e).unwrap()))
        .collect();
    let mut jobs: Vec<_> = proposals
        .iter()
        .filter_map(|p| {
            confirmation_batches(&p.2).map(|n| Job {
                p,
                n,
                draw_cost: budget::draw_cost(&p.2),
            })
        })
        .collect();
    jobs.sort_by(|a, b| {
        confirmation_priority(&a.p.2, a.n, (2 * a.n * a.draw_cost) as f64)
            .total_cmp(&confirmation_priority(
                &b.p.2,
                b.n,
                (2 * b.n * b.draw_cost) as f64,
            ))
            .then(a.p.0.index(m.n).cmp(&b.p.0.index(m.n)))
    });
    let mut bank = WorkBudget::new(budget::capacity(m, budget::confirmation_limit(fraction)));
    let mut paired = vec![false; jobs.len()];
    let mut started = vec![false; jobs.len()];
    // Reserve normal independent pairs first in stable priority order.
    for (i, j) in jobs.iter().enumerate() {
        paired[i] = bank.reserve(2 * j.n * j.draw_cost);
        started[i] = paired[i];
    }
    // Spare units can start a pending main while other workers run pairs.
    // Its check is not promised, and it cannot be published on its own.
    for (i, j) in jobs.iter().enumerate() {
        if !started[i] {
            started[i] = bank.reserve(j.n * j.draw_cost);
        }
    }
    let first: Vec<_> = (0..jobs.len()).filter(|&i| started[i]).collect();
    let costs: Vec<_> = first
        .iter()
        .map(|&i| jobs[i].n * jobs[i].draw_cost * if paired[i] { 2 } else { 1 })
        .collect();
    let rows = budget::parallel(&costs, workers, |slot| {
        let i = first[slot];
        let j = &jobs[i];
        let start = Instant::now();
        let main = j.main(m, seed);
        let check = if paired[i] {
            j.check(m, seed, &main, rough)
        } else {
            Result::default()
        };
        (i, main, check, start.elapsed().as_secs_f64() * 1000.)
    });
    let mut results = vec![None; jobs.len()];
    let mut reclaimed = 0;
    for (i, main, check, ms) in rows {
        if paired[i] && check.samples == 0 {
            let unused = jobs[i].n * jobs[i].draw_cost;
            assert!(bank.release(unused));
            reclaimed += unused;
        }
        results[i] = Some((main, check, ms));
    }
    // Barrier: all first-wave results are known. Allocation cannot depend on
    // which worker finished first, nor on elapsed duration.
    let mut second = Vec::new();
    for (i, j) in jobs.iter().enumerate() {
        if paired[i] {
            continue;
        }
        let cost = if started[i] {
            if !publishable(&results[i].as_ref().unwrap().0, rough) {
                continue;
            }
            j.n * j.draw_cost
        } else {
            2 * j.n * j.draw_cost
        };
        if bank.reserve(cost) {
            second.push((i, cost));
        }
    }
    let costs: Vec<_> = second.iter().map(|r| r.1).collect();
    let rows = budget::parallel(&costs, workers, |slot| {
        let i = second[slot].0;
        let j = &jobs[i];
        let start = Instant::now();
        let main = if started[i] {
            None
        } else {
            Some(j.main(m, seed))
        };
        let existing = results[i].as_ref().map(|r| &r.0);
        let check = j.check(m, seed, main.as_ref().or(existing).unwrap(), rough);
        (i, main, check, start.elapsed().as_secs_f64() * 1000.)
    });
    for (i, main, check, ms) in rows {
        let old = results[i].take();
        results[i] = Some((
            main.unwrap_or_else(|| old.as_ref().unwrap().0.clone()),
            check,
            ms + old.map_or(0., |r| r.2),
        ));
        paired[i] = true;
    }
    let mut work = 0;
    let mut added = 0;
    let mut completed = 0;
    let mut main_only = 0;
    for (i, row) in results.into_iter().enumerate() {
        let Some((main, check, ms)) = row else {
            continue;
        };
        work += main.work + check.work;
        let j = &jobs[i];
        let cell = j.p.0;
        let ok = paired[i] && accepted(&main, &check, rough);
        if paired[i] {
            completed += 1;
        } else {
            main_only += 1;
        }
        if ok && commit_estimate(&mut estimates[cell.index(m.n)], &main, true) {
            estimates[cell.index(m.n)].design = "matched_point_pool_rare_tail_confirm_more".into();
            added += 1;
        }
        emit(
            log,
            "rust_odds_rare_tail_confirm_more",
            json!({
                "group":m.request.id,"team":m.ids[cell.team],"rank":cell.rank+1,
                "pilot_hits":j.p.2.hits,"pilot_ess":j.p.2.ess,"main_draws":main.samples,
                "main_hits":main.hits,"main_ess":main.ess,"main_max_share":main.max_share,
                "probability":main.probability,"check_draws":check.samples,"check_hits":check.hits,
                "check_ess":check.ess,"check_probability":check.probability,"accepted":ok,
                "cost_per_draw":j.draw_cost,"modeled_main_work":main.samples*j.draw_cost,
                "modeled_check_work":check.samples*j.draw_cost,"sampling_ms":ms,"main_only":!paired[i]
            }),
        );
    }
    emit(
        log,
        "rust_odds_rare_tail_confirm_more_summary",
        json!({
            "group":m.request.id,"fraction":fraction,"budget_mode":budget::mode(),
            "work_limit":bank.limit,"reserved_work":bank.reserved,"reclaimed_check_work":reclaimed,
            "eligible":jobs.len(),"funded":paired.iter().filter(|&&p|p).count(),
            "completed":completed,"accepted":added,"main_only":main_only,"waves":2,
            "work":work,"elapsed_ms":clock.elapsed().as_secs_f64()*1000.,
            "preserved_existing_estimates":before.iter().all(|(i,e)|
                *e==serde_json::to_value(&estimates[*i]).unwrap())
        }),
    );
    (added, work)
}
