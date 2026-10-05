//! Two deterministic waves reuse skipped checks without a time-dependent race.
//! Main lengths remain fixed from independent training. Speculative mains never
//! publish alone; all reported estimates require a fresh independent check.
use super::*;

type Proposal = (Cell, Plan, Result, f64, u64, f64);
const FAMILY_TRAINING_CAP: usize = 3_000;
const NATIVE_PREFIX_CAP: usize = 3_000;
const NATIVE_OBSERVATION_CAP: usize = 256;
const FAMILY_MAX_NODES: usize = 10_000;
const FAMILY_MAX_GUIDE_VALUES: usize = 4_000_000;

struct Job<'a> {
    p: &'a Proposal,
    n: usize,
    draw_cost: usize,
    original_grant: usize,
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
    fn main_with_native_training(
        &self,
        m: &Model,
        seed: i64,
    ) -> Option<(Result, crate::joint_caps::propagated::lazy::FamilyMoments)> {
        self.p.1.sample_family_native_training(
            m,
            self.n,
            derive(
                seed,
                &format!(
                    "rare-tail-confirm-more-main-{}-{}",
                    m.ids[self.p.0.team], self.p.0.rank
                ),
            ),
            NATIVE_PREFIX_CAP,
            NATIVE_OBSERVATION_CAP,
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

pub(super) struct RunOutcome<'a> {
    pub added: usize,
    pub charged_work: u64,
    pub fallback: Option<FallbackState<'a>>,
}

impl<'a> RunOutcome<'a> {
    pub(super) fn account_work(self, work: &mut u64) -> (usize, Option<FallbackState<'a>>) {
        *work = work.saturating_add(self.charged_work);
        (self.added, self.fallback)
    }
}

pub(super) struct FallbackState<'a> {
    jobs: Vec<Job<'a>>,
    results: Vec<Option<(Result, Result, f64)>>,
    paired: Vec<bool>,
    moments: Vec<Option<crate::joint_caps::propagated::lazy::FamilyMoments>>,
    bank: WorkBudget,
    seed: i64,
    rough: bool,
    handoff_fee: usize,
    observer_funded: bool,
    observer_work: usize,
    observer_bounds: Vec<Option<usize>>,
    before: Vec<(usize, serde_json::Value)>,
    family_enabled: bool,
    workers: usize,
    branch_draw_audit: super::branches::DrawAudit,
    original_confirmation_capacity: usize,
    branch_transfer: usize,
    failed_confirmation_experiment: bool,
}

fn observer_plan(
    jobs: &[Job<'_>],
    paired: &[bool],
    m: &Model,
) -> Option<(usize, Vec<Option<usize>>)> {
    let mut total = 0usize;
    let mut bounds = vec![None; jobs.len()];
    let mut found = false;
    for (i, job) in jobs.iter().enumerate() {
        if !paired[i] {
            continue;
        }
        if let Some(bound) = job.p.1.native_family_recording_bound(
            m,
            job.n,
            NATIVE_PREFIX_CAP,
            NATIVE_OBSERVATION_CAP,
        ) {
            found = true;
            total = total.checked_add(bound)?;
            bounds[i] = Some(bound);
        }
    }
    found.then_some((total, bounds))
}

fn observer_dispatch_fee(job_count: usize) -> Option<usize> {
    job_count.checked_mul(64)
}

pub(super) fn native_training_stage_credit(
    capacity: usize,
    audited_training: usize,
    actual_final: usize,
    reserved_final: usize,
    modeled: bool,
    audit_complete: bool,
    unmetered_constructor_failures: usize,
) -> Option<(usize, usize, usize, usize)> {
    if !modeled || !audit_complete || unmetered_constructor_failures != 0 {
        return None;
    }
    let settled_used = audited_training.checked_add(actual_final)?;
    let conservative_used = audited_training.checked_add(actual_final.max(reserved_final))?;
    Some((
        settled_used,
        conservative_used,
        capacity.saturating_sub(settled_used),
        capacity.saturating_sub(conservative_used),
    ))
}

fn late_eligible(
    originally_paired: bool,
    native_main: &Result,
    estimate: &Estimate,
    rough: bool,
    supported: bool,
) -> bool {
    originally_paired
        && !publishable(native_main, rough)
        && supported
        && estimate.probability == 0.0
        && !estimate.reachability.starts_with("impossible")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FallbackEligibility {
    LegacyFailedMain,
    FailedConfirmation,
}

fn failed_confirmation_experiment_enabled(raw: Option<&str>) -> bool {
    raw == Some("1")
}

fn failed_confirmation_eligible(
    originally_paired: bool,
    lazy_supported: bool,
    estimate: &Estimate,
    main_publishable: bool,
    check_samples: usize,
    pair_accepted: bool,
) -> bool {
    originally_paired
        && lazy_supported
        && estimate.probability == 0.0
        && !estimate.reachability.starts_with("impossible")
        && main_publishable
        && check_samples > 0
        && !pair_accepted
}

fn eligibility_order(categories: &[Option<FallbackEligibility>], enabled: bool) -> Vec<usize> {
    let mut order: Vec<_> = (0..categories.len()).collect();
    if enabled {
        order.sort_by_key(|&index| match categories[index] {
            Some(FallbackEligibility::LegacyFailedMain) => 0,
            Some(FallbackEligibility::FailedConfirmation) => 1,
            None => 2,
        });
    }
    order
}

fn fallback_training_draws(
    grant: usize,
    clone_work: usize,
    validation_upper: usize,
    native_cost: usize,
    pair_cost: usize,
    sample_limit: usize,
) -> usize {
    let reserve = clone_work
        .saturating_add(100usize.saturating_mul(validation_upper))
        .saturating_add(fallback_pair_reserve(pair_cost))
        .saturating_add(1_000_000);
    let replay_budget = (grant / 2).min(grant.saturating_sub(reserve));
    (replay_budget / native_cost.max(1))
        .min(FAMILY_TRAINING_CAP)
        .min(sample_limit)
}

fn fallback_validation_draws(remaining: usize, pair_reserve: usize, upper: usize) -> usize {
    1_000usize.min(remaining.saturating_sub(pair_reserve) / upper.max(1))
}

fn validation_cost(validation: &Result, upper: usize) -> Option<usize> {
    if validation.samples < 100
        || validation.operations.units() > upper.max(1).saturating_mul(validation.samples)
    {
        return None;
    }
    let cost = budget::per_draw(validation);
    (cost > 0).then_some(cost)
}

fn fallback_pair_cost(
    structure: crate::rare_tail::family_config::FamilyStructure,
    native_cost: usize,
    validation_upper: usize,
) -> usize {
    if structure == crate::rare_tail::family_config::FamilyStructure::Full {
        native_cost.max(1)
    } else {
        validation_upper.max(1)
    }
}

fn fallback_pair_reserve(pair_cost: usize) -> usize {
    2_000usize.saturating_mul(pair_cost.max(1))
}

#[cfg(test)]
fn fallback_grant(cell_grant: usize, native_work: usize, bank_free: usize) -> usize {
    cell_grant.saturating_sub(native_work).min(bank_free)
}

fn family_settlement_within_grant(
    native_work: usize,
    observer_work: usize,
    family_work: usize,
    original_cell_grant: usize,
    transfer_work: usize,
    observer_bound: usize,
    overrun: bool,
) -> bool {
    !overrun
        && native_work
            .saturating_add(observer_work)
            .saturating_add(family_work)
            <= original_cell_grant
                .saturating_add(transfer_work)
                .saturating_add(observer_bound)
}

fn bounded_percent(raw: Option<&str>, default: usize, maximum: usize) -> usize {
    raw.and_then(|value| value.parse::<i128>().ok())
        .map(|value| value.clamp(0, maximum as i128) as usize)
        .unwrap_or(default.min(maximum))
}

fn percent_of(amount: usize, percent: usize) -> usize {
    (amount / 100)
        .saturating_mul(percent)
        .saturating_add((amount % 100).saturating_mul(percent) / 100)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FallbackGrant {
    requested: usize,
    capped: usize,
    available: usize,
    base: usize,
    transferred: usize,
    transfer_suppressed_native_overrun: bool,
}

fn fallback_grant_plan(
    original_cell_grant: usize,
    native_work: usize,
    bank_free: usize,
    legacy_percent: usize,
    transfer_percent: usize,
    failed_confirmation: bool,
) -> FallbackGrant {
    let residual = original_cell_grant.saturating_sub(native_work);
    let transfer_suppressed_native_overrun =
        failed_confirmation && native_work > original_cell_grant;
    let requested_transfer = if failed_confirmation && !transfer_suppressed_native_overrun {
        percent_of(original_cell_grant, transfer_percent)
    } else {
        0
    };
    let requested = residual.saturating_add(requested_transfer);
    let capped = if failed_confirmation {
        requested
    } else {
        percent_of(residual, legacy_percent)
    };
    let available = capped.min(bank_free);
    let base = available.min(residual);
    FallbackGrant {
        requested,
        capped,
        available,
        base,
        transferred: available.saturating_sub(base),
        transfer_suppressed_native_overrun,
    }
}

fn failed_confirmation_requested_grant(
    original_cell_grant: usize,
    native_work: usize,
    transfer_percent: usize,
) -> usize {
    fallback_grant_plan(
        original_cell_grant,
        native_work,
        usize::MAX,
        100,
        transfer_percent,
        true,
    )
    .requested
}

fn family_transfer_experiment_enabled(failed_confirmation: bool, transfer_percent: usize) -> bool {
    failed_confirmation && transfer_percent > 0
}

fn transfer_percent(raw: Option<&str>) -> usize {
    bounded_percent(raw, 0, 50)
}

fn legacy_grant_percent(raw: Option<&str>) -> usize {
    bounded_percent(raw, 100, 100)
}

fn reserve_failed_confirmation_enabled(raw: Option<&str>) -> bool {
    raw == Some("1")
}

fn grant_promises(requested: &[usize], entry_free: usize) -> Vec<usize> {
    let mut remaining = entry_free / 2;
    requested
        .iter()
        .map(|&request| {
            let promise = request.min(remaining);
            remaining -= promise;
            promise
        })
        .collect()
}

fn release_candidate_promise(bank: &mut WorkBudget, promises: &mut [usize], index: usize) -> usize {
    let held = promises[index];
    if held > 0 {
        assert!(bank.release(held));
        promises[index] = 0;
    }
    held
}

fn run_overflow_stage(
    state: &mut FallbackState<'_>,
    m: &Model,
    estimates: &mut [Estimate],
    log: Option<&crate::logging::RequestLog>,
    stopped_after_overrun: bool,
) -> (usize, u64) {
    let overflow_enabled = super::target_overflow_tree_enabled();
    let early_rank_enabled = super::experimental_early_rank_mode().is_some();
    let mut added = 0usize;
    let mut work = 0u64;
    if (overflow_enabled || early_rank_enabled) && !stopped_after_overrun && budget::modeled() {
        let bank_limit_before_credit = state.bank.limit;
        let requested_credit = if overflow_enabled {
            let requested_credit = super::overflow_trees::requested_branch_credit(
                state.branch_draw_audit,
                state.branch_transfer,
            );
            let applied_credit = super::overflow_trees::applied_branch_credit(
                state.branch_draw_audit,
                state.branch_transfer,
                state.bank.limit,
                state.original_confirmation_capacity,
                super::overflow_trees::reclaim_enabled(),
            );
            state.bank.limit = state
                .bank
                .limit
                .checked_add(applied_credit)
                .unwrap_or(state.original_confirmation_capacity)
                .min(state.original_confirmation_capacity);
            requested_credit
        } else {
            0
        };
        let bank_limit_after_credit = state.bank.limit;
        if early_rank_enabled {
            (added, work) = super::early_roots::run(
                m,
                state.seed,
                estimates,
                &mut state.bank,
                log,
                state.rough,
                state.branch_draw_audit,
                requested_credit,
                bank_limit_before_credit,
                bank_limit_after_credit,
            );
        } else if overflow_enabled {
            (added, work) = super::overflow_trees::run(
                m,
                state.seed,
                state.workers,
                estimates,
                &mut state.bank,
                log,
                state.branch_draw_audit,
                requested_credit,
                bank_limit_before_credit,
                bank_limit_after_credit,
            );
        }
    }
    (added, work)
}

pub(super) fn run<'a>(
    m: &Model,
    seed: i64,
    workers: usize,
    estimates: &mut [Estimate],
    proposals: &[&'a Proposal],
    rough: bool,
    fraction: f64,
    log: Option<&crate::logging::RequestLog>,
    transferred_work: usize,
    stage_spare: Option<usize>,
    stage_audit_valid: bool,
    branch_draw_audit: super::branches::DrawAudit,
    original_confirmation_capacity: usize,
) -> RunOutcome<'a> {
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
                original_grant: 2 * n * budget::draw_cost(&p.2),
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
    let mut bank = WorkBudget::new(
        budget::capacity(m, budget::confirmation_limit(fraction)).saturating_sub(transferred_work),
    );
    let mut paired = vec![false; jobs.len()];
    let mut started = vec![false; jobs.len()];
    let prefix = tree_enabled();
    if prefix {
        // A lower-priority full pair must not steal a higher-priority main.
        // Complete the first priority prefix, then fund one speculative main.
        for (i, j) in jobs.iter().enumerate() {
            if bank.reserve(2 * j.n * j.draw_cost) {
                paired[i] = true;
                started[i] = true;
            } else {
                started[i] = bank.reserve(j.n * j.draw_cost);
                break;
            }
        }
    } else {
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
    }
    let originally_paired = paired.clone();
    let family_enabled = super::family_fallback_enabled() && budget::modeled();
    let failed_confirmation_experiment = failed_confirmation_experiment_enabled(
        std::env::var("RUST_ODDS_EXPERIMENT_FAMILY_FAILED_CONFIRMATION")
            .ok()
            .as_deref(),
    );
    let mut observer_bounds = vec![None; jobs.len()];
    let mut observer_fee = 0usize;
    let mut observer_funded = false;
    if family_enabled && stage_audit_valid {
        let fee = observer_dispatch_fee(jobs.len());
        if let (Some(stage_spare), Some(fee)) = (stage_spare, fee) {
            if fee > 0 && fee <= stage_spare {
                observer_fee = fee;
                if let Some((bound, bounds)) = observer_plan(&jobs, &originally_paired, m) {
                    if bound > 0
                        && fee
                            .checked_add(bound)
                            .is_some_and(|need| need <= stage_spare)
                    {
                        observer_bounds = bounds;
                        observer_funded = true;
                    }
                }
            }
        }
    }
    let mut moments: Vec<Option<crate::joint_caps::propagated::lazy::FamilyMoments>> =
        (0..jobs.len()).map(|_| None).collect();
    let first: Vec<_> = (0..jobs.len()).filter(|&i| started[i]).collect();
    let costs: Vec<_> = first
        .iter()
        .map(|&i| jobs[i].n * jobs[i].draw_cost * if paired[i] { 2 } else { 1 })
        .collect();
    let rows = budget::parallel(&costs, workers, |slot| {
        let i = first[slot];
        let j = &jobs[i];
        let start = Instant::now();
        let (main, family_moments) = if observer_funded && observer_bounds[i].is_some() {
            j.main_with_native_training(m, seed)
                .map(|(main, moments)| (main, Some(moments)))
                .unwrap_or_else(|| (j.main(m, seed), None))
        } else {
            (j.main(m, seed), None)
        };
        let check = if paired[i] {
            j.check(m, seed, &main, rough)
        } else {
            Result::default()
        };
        (
            i,
            main,
            check,
            family_moments,
            start.elapsed().as_secs_f64() * 1000.,
        )
    });
    let mut results = vec![None; jobs.len()];
    let mut reclaimed = 0;
    for (i, main, check, family_moments, ms) in rows {
        if paired[i] && check.samples == 0 {
            let unused = jobs[i].n * jobs[i].draw_cost;
            assert!(bank.release(unused));
            reclaimed += unused;
        }
        moments[i] = family_moments;
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
    let observer_actual: usize = moments
        .iter()
        .flatten()
        .map(|moments| moments.collection_work)
        .sum();
    let observer_bound: usize = observer_bounds.iter().flatten().sum();
    let observer_within_bound = !observer_funded || observer_actual <= observer_bound;
    let mut work = 0;
    let mut added = 0;
    let mut completed = 0;
    let mut main_only = 0;
    for (i, row) in results.iter().enumerate() {
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
    work = work.saturating_add(observer_fee as u64 + observer_actual as u64);
    let required_handoff_fee = if family_enabled {
        observer_dispatch_fee(jobs.len())
    } else {
        Some(0)
    };
    let handoff_fee = if family_enabled {
        required_handoff_fee
            .filter(|&fee| bank.reserve(fee))
            .unwrap_or(0)
    } else {
        0
    };
    let handoff_funded = !family_enabled || required_handoff_fee == Some(handoff_fee);
    work = work.saturating_add(handoff_fee as u64);
    let mut summary = json!({
        "group":m.request.id,"fraction":fraction,"budget_mode":budget::mode(),
        "transferred_branch_work":transferred_work,
        "work_limit":bank.limit,"reserved_work":bank.reserved,"reclaimed_check_work":reclaimed,
        "eligible":jobs.len(),"funded":paired.iter().filter(|&&p|p).count(),
        "completed":completed,"accepted":added,"main_only":main_only,"waves":2,
        "family_fallback_enabled":family_enabled,"family_fallback_handoff_fee_required":required_handoff_fee,
        "family_fallback_handoff_fee":handoff_fee,"family_fallback_handoff_fee_funded":handoff_funded,
        "native_family_observer_funded":observer_funded,"native_family_observer_bound":observer_bound,
        "native_family_observer_work":observer_actual,"native_family_observer_within_bound":observer_within_bound,
        "work":work,"elapsed_ms":clock.elapsed().as_secs_f64()*1000.,
        "preserved_existing_estimates":before.iter().all(|(i,e)|
            *e==serde_json::to_value(&estimates[*i]).unwrap())
    });
    if failed_confirmation_experiment {
        summary["family_failed_confirmation_experiment"] = json!(true);
    }
    emit(log, "rust_odds_rare_tail_confirm_more_summary", summary);
    let overflow_enabled = super::target_overflow_tree_enabled();
    let family_fallback_available = family_enabled && handoff_funded && observer_within_bound;
    let early_rank_available = super::experimental_early_rank_mode().is_some() && budget::modeled();
    let fallback = if family_fallback_available
        || ((overflow_enabled || early_rank_available) && budget::modeled())
    {
        Some(FallbackState {
            jobs,
            results,
            paired: originally_paired,
            moments,
            bank,
            seed,
            rough,
            handoff_fee,
            observer_funded,
            observer_work: observer_actual,
            observer_bounds,
            before,
            family_enabled: family_fallback_available,
            workers,
            branch_draw_audit,
            original_confirmation_capacity,
            branch_transfer: transferred_work,
            failed_confirmation_experiment,
        })
    } else {
        None
    };
    RunOutcome {
        added,
        charged_work: work,
        fallback,
    }
}

pub(super) fn finish_fallback<'a>(
    mut state: FallbackState<'a>,
    m: &Model,
    estimates: &mut [Estimate],
    log: Option<&crate::logging::RequestLog>,
) -> (usize, u64) {
    let started = Instant::now();
    let transfer_percent = transfer_percent(
        std::env::var("RUST_ODDS_EXPERIMENT_FAMILY_FAILED_CONFIRMATION_TRANSFER_PERCENT")
            .ok()
            .as_deref(),
    );
    let configured_legacy_grant_percent = legacy_grant_percent(
        std::env::var("RUST_ODDS_EXPERIMENT_FAMILY_LEGACY_GRANT_PERCENT")
            .ok()
            .as_deref(),
    );
    let reserve_failed_confirmation = reserve_failed_confirmation_enabled(
        std::env::var("RUST_ODDS_EXPERIMENT_FAMILY_RESERVE_FAILED_CONFIRMATION")
            .ok()
            .as_deref(),
    );
    let transfer_requested =
        family_transfer_experiment_enabled(state.failed_confirmation_experiment, transfer_percent);
    let bank_free_before = state.bank.limit.saturating_sub(state.bank.reserved);
    let bank_reserved_before = state.bank.reserved;
    let mut added = 0usize;
    let mut charged_total = 0usize;
    let mut granted_total = 0usize;
    let mut settled_total = 0usize;
    let mut published_charged = 0usize;
    let mut attempts = 0usize;
    let mut skips = 0usize;
    let mut stopped_after_overrun = false;
    let mut overflow_stage_ran = false;
    let mut overflow_stage_ms = 0.0;
    let mut overflow_stage_added = 0usize;
    let mut overflow_stage_work = 0u64;
    let mut transfer_requested_total = 0usize;
    let mut transfer_reserved_total = 0usize;
    let mut transfer_consumed_total = 0usize;
    let mut transfer_released_total = 0usize;
    let mut legacy_requested_total = 0usize;
    let mut legacy_capped_total = 0usize;
    let mut legacy_actual_total = 0usize;
    let mut legacy_released_total = 0usize;
    let categories: Vec<_> = state
        .jobs
        .iter()
        .enumerate()
        .map(|(i, job)| {
            if !state.family_enabled || !state.paired[i] {
                return None;
            }
            let Some((native_main, native_check, _)) = state.results[i].as_ref() else {
                return None;
            };
            let index = job.p.0.index(m.n);
            if late_eligible(
                state.paired[i],
                native_main,
                &estimates[index],
                state.rough,
                matches!(&job.p.1, Plan::Lazy(_)),
            ) {
                Some(FallbackEligibility::LegacyFailedMain)
            } else if state.failed_confirmation_experiment
                && failed_confirmation_eligible(
                    state.paired[i],
                    matches!(&job.p.1, Plan::Lazy(_)),
                    &estimates[index],
                    publishable(native_main, state.rough),
                    native_check.samples,
                    accepted(native_main, native_check, state.rough),
                )
            {
                Some(FallbackEligibility::FailedConfirmation)
            } else {
                None
            }
        })
        .collect();
    let has_failed_confirmation_candidates = categories
        .iter()
        .any(|category| *category == Some(FallbackEligibility::FailedConfirmation));
    let transfer_mode = transfer_requested && has_failed_confirmation_candidates;
    let reserve_mode = transfer_mode && reserve_failed_confirmation;
    let legacy_grant_percent = if transfer_mode {
        configured_legacy_grant_percent
    } else {
        100
    };
    let entry_bank_free = state.bank.limit.saturating_sub(state.bank.reserved);
    let mut held_promises = vec![0usize; state.jobs.len()];
    let mut promised_total = 0usize;
    if reserve_mode {
        let candidate_requests: Vec<_> = categories
            .iter()
            .enumerate()
            .filter_map(|(i, category)| {
                if *category != Some(FallbackEligibility::FailedConfirmation) {
                    return None;
                }
                let job = &state.jobs[i];
                let (native_main, native_check, _) = state.results[i].as_ref()?;
                let native_work = native_main
                    .operations
                    .units()
                    .saturating_add(native_check.operations.units());
                Some((
                    i,
                    failed_confirmation_requested_grant(
                        job.original_grant,
                        native_work,
                        transfer_percent,
                    ),
                ))
            })
            .collect();
        let requested: Vec<_> = candidate_requests.iter().map(|(_, grant)| *grant).collect();
        let promises = grant_promises(&requested, entry_bank_free);
        for ((index, _), promise) in candidate_requests.into_iter().zip(promises) {
            if promise > 0 && state.bank.reserve(promise) {
                held_promises[index] = promise;
                promised_total = promised_total.saturating_add(promise);
            }
        }
        emit(
            log,
            "rust_odds_family_fallback_reservation_summary",
            json!({
                "group":m.request.id,"enabled":true,"entry_bank_free":entry_bank_free,
                "aggregate_envelope":entry_bank_free/2,"recipient_count":held_promises.iter().filter(|&&v|v>0).count(),
                "promised_work":promised_total,"overflow_draws_reallocated":true
            }),
        );
    }
    let order = eligibility_order(&categories, state.failed_confirmation_experiment);
    for i in order {
        if stopped_after_overrun {
            break;
        }
        if transfer_mode
            && categories[i] == Some(FallbackEligibility::FailedConfirmation)
            && !overflow_stage_ran
        {
            let bank_free = state.bank.limit.saturating_sub(state.bank.reserved);
            for (candidate_index, category) in categories.iter().enumerate() {
                if *category != Some(FallbackEligibility::FailedConfirmation) {
                    continue;
                }
                let job = &state.jobs[candidate_index];
                let Some((native_main, native_check, _)) = state.results[candidate_index].as_ref()
                else {
                    continue;
                };
                let original_grant = job.original_grant;
                let native_work = native_main
                    .operations
                    .units()
                    .saturating_add(native_check.operations.units());
                let residual = original_grant.saturating_sub(native_work);
                let requested = failed_confirmation_requested_grant(
                    original_grant,
                    native_work,
                    transfer_percent,
                );
                let held_promise = held_promises[candidate_index];
                let pre_overflow_grant = requested.min(bank_free.saturating_add(held_promise));
                let training_draws = fallback_training_draws(
                    pre_overflow_grant,
                    job.p.1.clone_work().unwrap_or(0),
                    budget::family_validation_upper_cost(m).max(1),
                    budget::per_draw(native_main).max(1),
                    fallback_pair_cost(
                        crate::rare_tail::family_structure(),
                        budget::per_draw(native_main).max(1),
                        budget::family_validation_upper_cost(m).max(1),
                    ),
                    native_main.samples,
                );
                emit(
                    log,
                    "rust_odds_family_fallback_deferred",
                    json!({
                        "group":m.request.id,"team_id":m.ids[job.p.0.team],"rank":job.p.0.rank+1,
                        "eligibility_category":"failed_confirmation","deferred_until_after_overflow":true,
                        "original_cell_grant":original_grant,"native_actual_work":native_work,
                        "original_residual_grant":residual,"transfer_percent":transfer_percent,
                        "legacy_grant_percent":legacy_grant_percent,"pre_overflow_unreserved_bank_free":bank_free,
                        "held_grant_promise":held_promise,
                        "pre_overflow_requested_grant":requested,"pre_overflow_available_grant":pre_overflow_grant,
                        "pre_overflow_training_draws":training_draws,"pre_overflow_training_sufficient":training_draws>=100
                    }),
                );
            }
            let overflow_started = Instant::now();
            (overflow_stage_added, overflow_stage_work) =
                run_overflow_stage(&mut state, m, estimates, log, stopped_after_overrun);
            overflow_stage_ms += overflow_started.elapsed().as_secs_f64() * 1000.0;
            overflow_stage_ran = true;
        }
        let job = &state.jobs[i];
        if !state.family_enabled {
            continue;
        }
        let Some((native_main, native_check, _)) = state.results[i].as_ref() else {
            continue;
        };
        let cell = job.p.0;
        let index = cell.index(m.n);
        let lazy_supported = matches!(&job.p.1, Plan::Lazy(_));
        let category = categories[i];
        let eligible_now = match category {
            Some(FallbackEligibility::LegacyFailedMain) => late_eligible(
                state.paired[i],
                native_main,
                &estimates[index],
                state.rough,
                lazy_supported,
            ),
            Some(FallbackEligibility::FailedConfirmation) => failed_confirmation_eligible(
                state.paired[i],
                lazy_supported,
                &estimates[index],
                publishable(native_main, state.rough),
                native_check.samples,
                accepted(native_main, native_check, state.rough),
            ),
            None => false,
        };
        if !eligible_now {
            let released_promise =
                release_candidate_promise(&mut state.bank, &mut held_promises, i);
            if released_promise > 0 {
                emit(
                    log,
                    "rust_odds_family_fallback_reservation_release",
                    json!({
                        "group":m.request.id,"team_id":m.ids[cell.team],"rank":cell.rank+1,
                        "released_work":released_promise,"reason":"candidate_superseded_or_failed_recheck"
                    }),
                );
            }
            if state.paired[i]
                && !publishable(native_main, state.rough)
                && estimates[index].probability > 0.
            {
                skips += 1;
                emit(
                    log,
                    "rust_odds_family_fallback_skip",
                    json!({"group":m.request.id,"team_id":m.ids[cell.team],"team_index":cell.team,"rank":cell.rank+1,"reason":"superseded_by_later_native_stage"}),
                );
            } else if state.paired[i]
                && !publishable(native_main, state.rough)
                && estimates[index].reachability.starts_with("impossible")
            {
                skips += 1;
                emit(
                    log,
                    "rust_odds_family_fallback_skip",
                    json!({"group":m.request.id,"team_id":m.ids[cell.team],"team_index":cell.team,"rank":cell.rank+1,"reason":"became_impossible"}),
                );
            }
            continue;
        }
        attempts += 1;
        let original_cell_grant = job.original_grant;
        let native_work = native_main
            .operations
            .units()
            .saturating_add(native_check.operations.units());
        let recorded = state.moments[i].take();
        let observer_work = recorded
            .as_ref()
            .map_or(0, |moments| moments.collection_work);
        let observer_bound = state.observer_bounds[i].unwrap_or(0);
        let held_promise = release_candidate_promise(&mut state.bank, &mut held_promises, i);
        let bank_free = state.bank.limit.saturating_sub(state.bank.reserved);
        let grant_plan = fallback_grant_plan(
            original_cell_grant,
            native_work,
            bank_free,
            legacy_grant_percent,
            if transfer_mode { transfer_percent } else { 0 },
            category == Some(FallbackEligibility::FailedConfirmation),
        );
        let grant = grant_plan.available;
        let mut charged = 0usize;
        let mut training_draws = 0usize;
        let mut replayed_training_draws = 0usize;
        let mut new_training_draws = 0usize;
        let mut training_hits = 0usize;
        let mut training_ess = 0.0;
        let mut training_operation_work = 0usize;
        let mut collection_work = 0usize;
        let mut clone_work = 0usize;
        let clone_estimate_work = job.p.1.clone_work().unwrap_or(0);
        let mut fit_work = 0usize;
        let mut validation_work = 0usize;
        let mut validation_draws = 0usize;
        let mut validation_hits = 0usize;
        let mut validation_ess = 0.0;
        let mut final_work = 0usize;
        let mut final_draws = 0usize;
        let mut family_summary = json!([]);
        let mut family_count = 0usize;
        let mut final_metrics = json!({});
        let mut diagnostic = serde_json::Map::new();
        let mut stage_ms = serde_json::Map::new();
        let mut published = false;
        let mut grant_reserved = false;
        let mut pair_cost_bound = 0usize;
        let partial_family_structure = crate::rare_tail::family_structure()
            != crate::rare_tail::family_config::FamilyStructure::Full;
        let overrun;

        if grant == 0 || !state.bank.reserve(grant) {
            overrun = false;
        } else {
            grant_reserved = true;
            let native_cost = budget::per_draw(native_main).max(1);
            let upper = budget::family_validation_upper_cost(m).max(1);
            let pair_cost =
                fallback_pair_cost(crate::rare_tail::family_structure(), native_cost, upper);
            pair_cost_bound = pair_cost;
            training_draws = recorded.as_ref().map_or_else(
                || {
                    fallback_training_draws(
                        grant,
                        clone_estimate_work,
                        upper,
                        native_cost,
                        pair_cost,
                        native_main.samples,
                    )
                },
                |moments| moments.observed_prefix_draws,
            );
            let training_started = Instant::now();
            if training_draws >= 100 {
                let training = if let Some(moments) = recorded.clone() {
                    training_hits = moments.recorded_positive_observations;
                    training_ess = moments.recorded_observation_ess;
                    Some(moments)
                } else {
                    job.p
                        .1
                        .sample_family_training(
                            m,
                            training_draws,
                            derive(
                                state.seed,
                                &format!(
                                    "rare-tail-confirm-more-main-{}-{}",
                                    m.ids[cell.team], cell.rank
                                ),
                            ),
                        )
                        .map(|(training, moments)| {
                            training_draws = training.samples;
                            training_hits = training.hits;
                            training_ess = training.ess;
                            training_operation_work = training.operations.units();
                            collection_work = moments.collection_work;
                            let (replayed, new) =
                                training_draw_accounting(training.samples, native_main.samples);
                            replayed_training_draws = replayed;
                            new_training_draws = new;
                            moments
                        })
                };
                stage_ms.insert(
                    "replay".into(),
                    json!(training_started.elapsed().as_secs_f64() * 1000.0),
                );
                if let Some(moments) = training {
                    let train_charge = training_operation_work.saturating_add(collection_work);
                    charged = charged.saturating_add(train_charge);
                    let min_validation = 100usize.saturating_mul(upper);
                    let min_pair = fallback_pair_reserve(pair_cost);
                    if charged
                        .saturating_add(clone_estimate_work)
                        .saturating_add(min_validation)
                        .saturating_add(min_pair)
                        <= grant
                    {
                        let clone_started = Instant::now();
                        let clone_candidate = job.p.1.clone_lazy();
                        stage_ms.insert(
                            "clone".into(),
                            json!(clone_started.elapsed().as_secs_f64() * 1000.0),
                        );
                        if let Some(mut candidate) = clone_candidate {
                            clone_work = clone_estimate_work;
                            charged = charged.saturating_add(clone_work);
                            let fit_budget = grant
                                .saturating_sub(charged)
                                .saturating_sub(min_validation)
                                .saturating_sub(min_pair);
                            let fit_started = Instant::now();
                            let (nodes, guides, _) = candidate.fit_families_capped(
                                &moments,
                                FAMILY_MAX_NODES.min(fit_budget / 32),
                                FAMILY_MAX_GUIDE_VALUES.min(fit_budget / 4),
                            );
                            fit_work = budget::setup_cost(nodes, guides)
                                .saturating_add(candidate.family_selection_work());
                            charged = charged.saturating_add(fit_work);
                            family_count = candidate.family_pattern_count();
                            family_summary = candidate.family_fit_summary();
                            stage_ms.insert(
                                "family_fit".into(),
                                json!(fit_started.elapsed().as_secs_f64() * 1000.0),
                            );
                            let validation_n = fallback_validation_draws(
                                grant.saturating_sub(charged),
                                min_pair,
                                upper,
                            );
                            if validation_n >= 100 {
                                let validation_started = Instant::now();
                                let validation = candidate.sample(
                                    m,
                                    validation_n,
                                    derive(
                                        state.seed,
                                        &format!(
                                            "rare-tail-family-fallback-validation-{}-{}",
                                            m.ids[cell.team], cell.rank
                                        ),
                                    ),
                                );
                                validation_draws = validation.samples;
                                validation_hits = validation.hits;
                                validation_ess = validation.ess;
                                validation_work = validation.operations.units();
                                charged = charged.saturating_add(validation_work);
                                stage_ms.insert(
                                    "validation".into(),
                                    json!(validation_started.elapsed().as_secs_f64() * 1000.0),
                                );
                                let per_draw = validation_cost(&validation, upper);
                                let final_n = per_draw.map_or(0, |cost| {
                                    let cost = if partial_family_structure {
                                        cost.max(pair_cost)
                                    } else {
                                        cost
                                    };
                                    ((grant.saturating_sub(charged).saturating_mul(92) / 100)
                                        / cost.saturating_mul(2).max(1))
                                    .min(100_000)
                                });
                                if family_count > 0
                                    && per_draw.is_some()
                                    && charged <= grant
                                    && final_n >= 1_000
                                {
                                    let main_started = Instant::now();
                                    let main = candidate.sample(
                                        m,
                                        final_n,
                                        derive(
                                            state.seed,
                                            &format!(
                                                "rare-tail-family-fallback-main-{}-{}",
                                                m.ids[cell.team], cell.rank
                                            ),
                                        ),
                                    );
                                    stage_ms.insert(
                                        "main".into(),
                                        json!(main_started.elapsed().as_secs_f64() * 1000.0),
                                    );
                                    let main_work = main.operations.units();
                                    charged = charged.saturating_add(main_work);
                                    let check_started = Instant::now();
                                    let check =
                                        if charged <= grant && publishable(&main, state.rough) {
                                            candidate.sample(
                                                m,
                                                final_n,
                                                derive(
                                                    state.seed,
                                                    &format!(
                                                        "rare-tail-family-fallback-check-{}-{}",
                                                        m.ids[cell.team], cell.rank
                                                    ),
                                                ),
                                            )
                                        } else {
                                            Result::default()
                                        };
                                    stage_ms.insert(
                                        "check".into(),
                                        json!(check_started.elapsed().as_secs_f64() * 1000.0),
                                    );
                                    let check_work = check.operations.units();
                                    final_draws = final_n;
                                    final_work = main_work.saturating_add(check_work);
                                    charged = charged.saturating_add(check_work);
                                    published = charged <= grant
                                        && family_settlement_within_grant(
                                            native_work,
                                            observer_work,
                                            charged,
                                            original_cell_grant,
                                            grant_plan.transferred,
                                            observer_bound,
                                            charged > grant,
                                        )
                                        && accepted(&main, &check, state.rough)
                                        && estimates[index].probability == 0.0;
                                    final_metrics = json!({
                                        "main_probability":main.probability,"main_ess":main.ess,"main_hits":main.hits,
                                        "main_relative_se":if main.probability>0.{Some(main.std_err/main.probability)}else{None},
                                        "main_max_share":main.max_share,"main_publishable":publishable(&main,state.rough),
                                        "check_probability":check.probability,"check_ess":check.ess,"check_hits":check.hits,
                                        "check_relative_se":if check.probability>0.{Some(check.std_err/check.probability)}else{None},
                                        "accepted":accepted(&main,&check,state.rough)
                                    });
                                    if published {
                                        commit_estimate(&mut estimates[index], &main, true);
                                        estimates[index].design =
                                            "matched_point_pool_rare_tail_family_fallback".into();
                                        added += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            overrun = charged > grant;
            if !overrun {
                assert!(state.bank.release(grant - charged));
            }
        }
        if transfer_mode {
            let category_label = match category {
                Some(FallbackEligibility::LegacyFailedMain) => "legacy_failed_main",
                Some(FallbackEligibility::FailedConfirmation) => "failed_confirmation",
                None => "ineligible",
            };
            let reserved_transfer = if grant_reserved {
                grant_plan.transferred
            } else {
                0
            };
            let consumed_transfer = charged
                .saturating_sub(grant_plan.base)
                .min(reserved_transfer);
            let original_residual = original_cell_grant.saturating_sub(native_work);
            let requested_transfer = grant_plan.requested.saturating_sub(original_residual);
            transfer_requested_total = transfer_requested_total.saturating_add(requested_transfer);
            transfer_reserved_total = transfer_reserved_total.saturating_add(reserved_transfer);
            transfer_consumed_total = transfer_consumed_total.saturating_add(consumed_transfer);
            transfer_released_total = transfer_released_total
                .saturating_add(reserved_transfer.saturating_sub(consumed_transfer));
            if category == Some(FallbackEligibility::LegacyFailedMain) {
                legacy_requested_total =
                    legacy_requested_total.saturating_add(grant_plan.requested);
                legacy_capped_total = legacy_capped_total.saturating_add(grant_plan.capped);
                legacy_actual_total = legacy_actual_total.saturating_add(grant);
                legacy_released_total =
                    legacy_released_total.saturating_add(grant.saturating_sub(charged.min(grant)));
            }
            diagnostic.extend(json!({
                "eligibility_category":category_label,
                "original_cell_grant":original_cell_grant,
                "native_actual_work":native_work,
                "original_residual_grant":original_residual,
                "requested_fallback_grant":grant_plan.requested,
                "capped_fallback_grant":grant_plan.capped,
                "reserved_fallback_grant":if grant_reserved {grant}else{0},
                "consumed_fallback_grant":charged,
                "released_fallback_grant":if grant_reserved {grant.saturating_sub(charged.min(grant))}else{0},
                "transfer_percent":transfer_percent,
                "transfer_suppressed_native_overrun":grant_plan.transfer_suppressed_native_overrun,
                "transfer_requested_work":requested_transfer,
                "transfer_reserved_work":reserved_transfer,
                "transfer_consumed_work":consumed_transfer,
                "transfer_released_work":reserved_transfer.saturating_sub(consumed_transfer),
                "legacy_grant_percent":legacy_grant_percent,
                "legacy_requested_residual":if category_label=="legacy_failed_main" {grant_plan.requested}else{0},
                "legacy_capped_grant":if category_label=="legacy_failed_main" {grant_plan.capped}else{0},
                "legacy_actual_grant":if category_label=="legacy_failed_main" && grant_reserved {grant}else{0},
                "legacy_released_grant":if category_label=="legacy_failed_main" && grant_reserved {grant.saturating_sub(charged.min(grant))}else{0},
                "grant_sufficient_for_training":training_draws>=100,
                "held_grant_promise":held_promise,
                "released_grant_promise":held_promise
            }).as_object().unwrap().clone());
        } else if category == Some(FallbackEligibility::FailedConfirmation) {
            diagnostic.insert("eligibility_category".into(), json!("failed_confirmation"));
            diagnostic.insert("original_cell_grant".into(), json!(original_cell_grant));
            diagnostic.insert("native_actual_work".into(), json!(native_work));
            diagnostic.insert(
                "cell_residual_after_native_work".into(),
                json!(original_cell_grant.saturating_sub(native_work)),
            );
            diagnostic.insert("available_fallback_grant".into(), json!(grant));
        }
        let reason = if grant == 0 {
            "no cell grant remaining after native actual work"
        } else if overrun {
            "fallback actual work exceeded its grant"
        } else if published {
            "family fallback published after independent validation and pair"
        } else {
            "family training, fit, validation or fresh pair did not pass gates"
        };
        charged_total = charged_total.saturating_add(charged);
        granted_total = granted_total.saturating_add(grant);
        settled_total = settled_total.saturating_add(charged.min(grant));
        if published {
            published_charged = published_charged.saturating_add(charged);
        }
        if overrun {
            stopped_after_overrun = true;
        }
        let mut fallback_record = json!({
                "status":if published {"published"} else {"declined"},"reason":reason,
                "native_training_funding":"stage","native_training_observer_bound":observer_bound,
                "native_training_observer_work":observer_work,
                "native_training_reused_draws":recorded.as_ref().map_or(0, |v|v.observed_prefix_draws),
                "native_training_recorded_observations":recorded.as_ref().map_or(0, |v|v.recorded_positive_observations),
                "native_training_recorded_ess":recorded.as_ref().map_or(0., |v|v.recorded_observation_ess),
                "base_fallback_grant":grant,"fallback_grant":grant,"unused_fallback_grant":grant.saturating_sub(charged),
                "added_actual_work":charged,"added_actual_work_including_stage_observer":charged.saturating_add(observer_work),
                "native_plus_added_work":native_work.saturating_add(observer_work).saturating_add(charged),
                "settlement_within_grant":family_settlement_within_grant(native_work,observer_work,charged,original_cell_grant,grant_plan.transferred,observer_bound,overrun),
                "overrun":overrun,"published":published,
                "training_draws":training_draws,"training_replayed_draws":replayed_training_draws,"training_new_draws":new_training_draws,
                "training_cap":FAMILY_TRAINING_CAP,"training_hits":training_hits,"training_ess":training_ess,
                "training_operation_work":training_operation_work,"family_collection_work":collection_work,
                "clone_estimate_work":clone_estimate_work,"clone_work":clone_work,"fit_work":fit_work,
                "validation_draws":validation_draws,"validation_hits":validation_hits,"validation_ess":validation_ess,"validation_work":validation_work
        });
        fallback_record.as_object_mut().unwrap().extend(json!({
                "family_count":family_count,"family_summary":family_summary,"final_draws_per_stream":final_draws,"final_work":final_work,"final":final_metrics,"charged_work":charged,
                "stage_ms":stage_ms
        }).as_object().unwrap().clone());
        if partial_family_structure {
            fallback_record.as_object_mut().unwrap().extend(json!({
                "partial_family_validation_upper_cost":budget::family_validation_upper_cost(m).max(1),
                "partial_family_pair_cost_bound":pair_cost_bound
            }).as_object().unwrap().clone());
        }
        fallback_record.as_object_mut().unwrap().extend(diagnostic);
        emit(
            log,
            "rust_odds_family_fallback",
            json!({"group":m.request.id,"team_id":m.ids[cell.team],"team_index":cell.team,"rank":cell.rank+1,"fallback":fallback_record}),
        );
    }
    for i in 0..held_promises.len() {
        let released = release_candidate_promise(&mut state.bank, &mut held_promises, i);
        if released > 0 {
            let job = &state.jobs[i];
            emit(
                log,
                "rust_odds_family_fallback_reservation_release",
                json!({
                    "group":m.request.id,"team_id":m.ids[job.p.0.team],"rank":job.p.0.rank+1,
                    "released_work":released,
                    "reason":if stopped_after_overrun {"aborted_after_overrun"} else {"candidate_not_attempted"}
                }),
            );
        }
    }
    if transfer_mode && !overflow_stage_ran {
        let overflow_started = Instant::now();
        (overflow_stage_added, overflow_stage_work) =
            run_overflow_stage(&mut state, m, estimates, log, stopped_after_overrun);
        overflow_stage_ms += overflow_started.elapsed().as_secs_f64() * 1000.0;
    }
    let preserved_existing_estimates = state
        .before
        .iter()
        .all(|(i, estimate)| *estimate == serde_json::to_value(&estimates[*i]).unwrap());
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let family_elapsed_ms = if transfer_mode {
        (elapsed_ms - overflow_stage_ms).max(0.0)
    } else {
        elapsed_ms
    };
    let mut summary = json!({
    "group":m.request.id,"enabled":state.family_enabled,"mode":"family","scope":"all","multiplier":1,
    "training_cap":FAMILY_TRAINING_CAP,"native_prefix_cap":NATIVE_PREFIX_CAP,"native_observation_cap":NATIVE_OBSERVATION_CAP,
    "handoff_fee":state.handoff_fee,"observer_funded":state.observer_funded,"observer_work":state.observer_work,
    "bank_limit":state.bank.limit,"bank_reserved_before_late_work":bank_reserved_before,
    "bank_free_before_handoff":bank_free_before,"bank_reserved_after_late_work":state.bank.reserved,
    "attempts":attempts,"skips":skips,"added_actual_work":charged_total,"granted_work":granted_total,
    "settled_fallback_work":settled_total,"unused_grant_work":granted_total.saturating_sub(settled_total),
    "published_charged_work":published_charged,"added":added,"stopped_after_overrun":stopped_after_overrun,
        "preserved_existing_estimates":preserved_existing_estimates,"elapsed_ms":family_elapsed_ms
    });
    if state.failed_confirmation_experiment {
        summary["family_failed_confirmation_experiment"] = json!(true);
        summary["family_transfer_reallocation"] = json!({
            "transfer_percent":transfer_percent,
            "configured_legacy_grant_percent":configured_legacy_grant_percent,
            "effective_legacy_grant_percent":legacy_grant_percent,
            "has_failed_confirmation_candidates":has_failed_confirmation_candidates,
            "reallocation_active":transfer_mode,
            "reserve_failed_confirmation_requested":reserve_failed_confirmation,
            "reserve_failed_confirmation_active":reserve_mode,
            "reservation_entry_bank_free":if reserve_mode {Some(entry_bank_free)}else{None},
            "reservation_aggregate_envelope":if reserve_mode {Some(entry_bank_free/2)}else{None},
            "reservation_promised_work":promised_total,
            "overflow_draws_reallocated":reserve_mode,
            "transfer_requested_work":transfer_requested_total,
            "transfer_reserved_work":transfer_reserved_total,
            "transfer_consumed_work":transfer_consumed_total,
            "transfer_released_work":transfer_released_total,
            "legacy_requested_residual":legacy_requested_total,
            "legacy_capped_grant":legacy_capped_total,
            "legacy_actual_grant":legacy_actual_total,
            "legacy_released_grant":legacy_released_total
        });
        if transfer_mode {
            summary["overflow_stage_ms"] = json!(overflow_stage_ms);
            summary["includes_overflow_stage_time"] = json!(false);
        }
    }
    emit(log, "rust_odds_family_fallback_late_summary", summary);
    if !transfer_mode {
        (overflow_stage_added, overflow_stage_work) =
            run_overflow_stage(&mut state, m, estimates, log, stopped_after_overrun);
    }
    (
        added.saturating_add(overflow_stage_added),
        (charged_total as u64).saturating_add(overflow_stage_work),
    )
}

fn training_draw_accounting(total: usize, native_samples: usize) -> (usize, usize) {
    (
        total.min(native_samples),
        total.saturating_sub(native_samples),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_pair_floor_uses_conservative_bound_only_for_partial_structures() {
        use crate::rare_tail::family_config::FamilyStructure::{Full, Relax, Skeleton, Ties};
        let native = 17;
        let upper = 101;
        assert_eq!(fallback_pair_cost(Full, native, upper), native);
        assert_eq!(
            fallback_pair_reserve(fallback_pair_cost(Full, native, upper)),
            34_000
        );
        for structure in [Ties, Skeleton, Relax] {
            let cost = fallback_pair_cost(structure, native, upper);
            assert_eq!(cost, upper);
            assert_eq!(fallback_pair_reserve(cost), 202_000);
        }
        assert_eq!(fallback_validation_draws(201_999, 202_000, upper), 0);
    }

    #[test]
    fn stage_credit_requires_complete_audited_modeled_work() {
        assert_eq!(
            native_training_stage_credit(1000, 100, 200, 300, true, true, 0),
            Some((300, 400, 700, 600))
        );
        assert_eq!(
            native_training_stage_credit(1000, 100, 200, 300, true, false, 0),
            None
        );
        assert_eq!(
            native_training_stage_credit(1000, 100, 200, 300, true, true, 1),
            None
        );
        assert_eq!(
            native_training_stage_credit(1000, 100, 200, 300, false, true, 0),
            None
        );
    }

    #[test]
    fn late_eligibility_needs_failed_paired_main_and_current_zero() {
        let main = Result::default();
        let mut estimate = Estimate::default();
        assert!(late_eligible(true, &main, &estimate, true, true));
        assert!(!late_eligible(false, &main, &estimate, true, true));
        assert!(!late_eligible(true, &main, &estimate, true, false));
        estimate.probability = 0.1;
        assert!(!late_eligible(true, &main, &estimate, true, true));
        estimate.probability = 0.0;
        estimate.reachability = "impossible:test".into();
        assert!(!late_eligible(true, &main, &estimate, true, true));
    }

    #[test]
    fn failed_confirmation_experiment_is_opt_in() {
        assert!(!failed_confirmation_experiment_enabled(None));
        assert!(!failed_confirmation_experiment_enabled(Some("0")));
        assert!(!failed_confirmation_experiment_enabled(Some("true")));
        assert!(failed_confirmation_experiment_enabled(Some("1")));
    }

    #[test]
    fn failed_confirmation_accepts_only_failed_supported_zero_pairs() {
        let mut estimate = Estimate::default();
        assert!(failed_confirmation_eligible(
            true, true, &estimate, true, 200, false
        )); // A sampled CHECK with poor ESS or disagreement failed acceptance.
        assert!(!failed_confirmation_eligible(
            true, true, &estimate, true, 200, true
        )); // A good pair stays published by its native path.
        assert!(!failed_confirmation_eligible(
            true, true, &estimate, true, 0, false
        )); // No CHECK work was completed.
        assert!(!failed_confirmation_eligible(
            false, true, &estimate, true, 200, false
        )); // The native pair was not originally funded.
        assert!(!failed_confirmation_eligible(
            true, false, &estimate, true, 200, false
        )); // Union plans cannot train lazy families.
        assert!(!failed_confirmation_eligible(
            true, true, &estimate, false, 200, false
        )); // The native MAIN must be publishable.
        estimate.probability = 0.01;
        assert!(!failed_confirmation_eligible(
            true, true, &estimate, true, 200, false
        )); // Existing positive estimates are preserved.
        estimate.probability = 0.0;
        estimate.reachability = "impossible:test".into();
        assert!(!failed_confirmation_eligible(
            true, true, &estimate, true, 200, false
        )); // Impossible cells are excluded.
    }

    #[test]
    fn publishable_main_with_low_ess_or_disagreeing_check_is_a_candidate() {
        let estimate = Estimate::default();
        let main = Result {
            weighted: true,
            samples: 500,
            probability: 1.0e-6,
            std_err: 1.0e-7,
            hits: 40,
            ess: 15.0,
            max_share: 0.05,
            batch_gap: 0.1,
            ..Result::default()
        };
        assert!(publishable(&main, false));

        let low_ess_check = Result {
            samples: 500,
            probability: 1.0e-6,
            hits: 40,
            ess: 4.0,
            ..Result::default()
        };
        assert!(!accepted(&main, &low_ess_check, false));
        assert!(failed_confirmation_eligible(
            true,
            true,
            &estimate,
            publishable(&main, false),
            low_ess_check.samples,
            accepted(&main, &low_ess_check, false),
        ));

        let disagreeing_check = Result {
            samples: 500,
            probability: 1.0e-8,
            hits: 40,
            ess: 10.0,
            ..Result::default()
        };
        assert!(!accepted(&main, &disagreeing_check, false));
        assert!(failed_confirmation_eligible(
            true,
            true,
            &estimate,
            publishable(&main, false),
            disagreeing_check.samples,
            accepted(&main, &disagreeing_check, false),
        ));
    }

    #[test]
    fn rough_mode_main_with_low_check_ess_or_disagreement_is_a_candidate() {
        let estimate = Estimate::default();
        let main = Result {
            weighted: true,
            samples: 500,
            probability: 1.0,
            std_err: 0.1,
            hits: 174,
            ess: 7.9,
            max_share: 0.22,
            batch_gap: 1.0,
            ..Result::default()
        };
        assert!(publishable(&main, true));

        let low_ess = Result {
            samples: 500,
            probability: 1.0,
            hits: 40,
            ess: 2.6,
            std_err: 0.2,
            ..Result::default()
        };
        assert!(!accepted(&main, &low_ess, true));
        assert!(failed_confirmation_eligible(
            true,
            true,
            &estimate,
            publishable(&main, true),
            low_ess.samples,
            accepted(&main, &low_ess, true),
        ));

        let disagreement = Result {
            samples: 500,
            probability: 0.15,
            hits: 40,
            ess: 3.2,
            std_err: 0.02,
            ..Result::default()
        };
        assert!(!accepted(&main, &disagreement, true));
        assert!(failed_confirmation_eligible(
            true,
            true,
            &estimate,
            publishable(&main, true),
            disagreement.samples,
            accepted(&main, &disagreement, true),
        ));
    }

    #[test]
    fn experimental_fallback_order_keeps_legacy_jobs_first_and_stable() {
        let categories = [
            Some(FallbackEligibility::FailedConfirmation),
            None,
            Some(FallbackEligibility::LegacyFailedMain),
            Some(FallbackEligibility::FailedConfirmation),
            Some(FallbackEligibility::LegacyFailedMain),
        ];
        assert_eq!(eligibility_order(&categories, true), vec![2, 4, 0, 3, 1]);
        assert_eq!(eligibility_order(&categories, false), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn fallback_grant_is_limited_by_cell_and_request_bank_remainders() {
        assert_eq!(fallback_grant(100, 40, 30), 30);
        assert_eq!(fallback_grant(100, 140, 30), 0);
        assert_eq!(fallback_grant(100, 40, 100), 60);
    }

    #[test]
    fn shared_bank_grant_controls_default_and_clamp() {
        assert_eq!(transfer_percent(None), 0);
        assert_eq!(transfer_percent(Some("bad")), 0);
        assert_eq!(transfer_percent(Some("20")), 20);
        assert_eq!(transfer_percent(Some("80")), 50);
        assert_eq!(transfer_percent(Some("-5")), 0);
        assert_eq!(legacy_grant_percent(None), 100);
        assert_eq!(legacy_grant_percent(Some("bad")), 100);
        assert_eq!(legacy_grant_percent(Some("40")), 40);
        assert_eq!(legacy_grant_percent(Some("120")), 100);
        assert_eq!(legacy_grant_percent(Some("-5")), 0);
        assert!(family_transfer_experiment_enabled(true, 1));
        assert!(!family_transfer_experiment_enabled(false, 50));
        assert!(!family_transfer_experiment_enabled(true, 0));
    }

    #[test]
    fn legacy_grant_cap_preserves_full_grant_and_cuts_requested_shares() {
        assert_eq!(
            fallback_grant_plan(1000, 400, 900, 100, 25, false),
            FallbackGrant {
                requested: 600,
                capped: 600,
                available: 600,
                base: 600,
                transferred: 0,
                transfer_suppressed_native_overrun: false,
            }
        );
        assert_eq!(
            fallback_grant_plan(1000, 400, 900, 50, 25, false).available,
            300
        );
        assert_eq!(
            fallback_grant_plan(1000, 400, 900, 0, 25, false).available,
            0
        );
    }

    #[test]
    fn failed_confirmation_transfer_is_fractional_bank_limited_and_nonduplicating() {
        let plan = fallback_grant_plan(1000, 400, 700, 100, 25, true);
        assert_eq!(plan.requested, 850); // 600 residual + 250 donor grant.
        assert_eq!(plan.available, 700);
        assert_eq!(plan.base, 600);
        assert_eq!(plan.transferred, 100);
        assert!(plan.transferred <= percent_of(1000, 25));
        assert!(plan.available <= 700);
        let full = fallback_grant_plan(1000, 400, 900, 100, 25, true);
        assert_eq!(full.available, 850);
        assert_eq!(full.transferred, 250);
        assert!(full.available <= full.requested);
        let overrun = fallback_grant_plan(1000, 1100, 900, 100, 50, true);
        assert_eq!(overrun.available, 0);
        assert!(overrun.transfer_suppressed_native_overrun);
    }

    #[test]
    fn transfer_percentage_math_saturates_safely_at_usize_max() {
        assert_eq!(percent_of(usize::MAX, 100), usize::MAX);
        assert!(percent_of(usize::MAX, 50) <= usize::MAX / 2 + 1);
    }

    #[test]
    fn reserved_failed_confirmation_promises_share_half_the_entry_bank() {
        let promises = grant_promises(&[80, 90, 20], 200);
        assert_eq!(promises, vec![80, 20, 0]);
        assert_eq!(promises.iter().sum::<usize>(), 100);
        assert_eq!(grant_promises(&[], 200), Vec::<usize>::new());
        assert_eq!(grant_promises(&[10, 20], 0), vec![0, 0]);
        assert!(
            grant_promises(&[usize::MAX, usize::MAX], usize::MAX)
                .iter()
                .sum::<usize>()
                <= usize::MAX / 2
        );
    }

    #[test]
    fn held_promises_release_on_supersede_or_stop_without_charging_work() {
        let mut bank = WorkBudget::new(1000);
        assert!(bank.reserve(300));
        let mut promises = vec![300, 0];
        assert_eq!(release_candidate_promise(&mut bank, &mut promises, 0), 300);
        assert_eq!(bank.reserved, 0);
        assert_eq!(promises, vec![0, 0]);
        assert_eq!(release_candidate_promise(&mut bank, &mut promises, 1), 0);
    }

    #[test]
    fn transfer_settlement_allows_only_explicit_transfer_and_audited_observer_bound() {
        assert!(family_settlement_within_grant(
            900, 20, 180, 1000, 100, 30, false
        ));
        assert!(!family_settlement_within_grant(
            900, 20, 211, 1000, 100, 30, false
        ));
        assert!(!family_settlement_within_grant(
            900, 20, 180, 1000, 100, 30, true
        ));
        assert!(!family_settlement_within_grant(
            900, 51, 180, 1000, 100, 30, false
        ));
    }
}
