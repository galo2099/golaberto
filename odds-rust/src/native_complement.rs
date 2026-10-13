//! Bounded native full-season complement sampling for saturated title cells.
use crate::{
    conditioned::{Backward, Event},
    model::Model,
    pool::{Bounds, Estimate},
    rng::{derive, Rng},
};

const DRAWS: usize = 20_000;
const EVENT_STATES: usize = 20_000;
const TARGET_LIMIT: usize = 2;
const ALPHA: f64 = 0.01;
const PREP_WORK: usize = 24_000_000;
const TABLE_ENTRIES: usize = 500_000;

pub(crate) struct PassResult {
    pub components: usize,
    pub work: u64,
    pub intervals: Vec<(usize, f64, f64)>,
    pub reason: &'static str,
    pub accepted_targets: usize,
    pub reserved_prep: usize,
    pub epsilon_interval: Option<(f64, f64)>,
}

struct NativeFixture {
    rows: [Vec<([i32; 2], f64)>; 3],
}

fn native_fixture(
    f: &crate::model::Fixture,
    entry_budget: usize,
) -> Option<(NativeFixture, [f64; 3])> {
    if !f.home_sampler.mean.is_finite()
        || !f.away_sampler.mean.is_finite()
        || f.home_sampler.mean > 32.
        || f.away_sampler.mean > 32.
        || f.home_sampler.mean < 0.
        || f.away_sampler.mean < 0.
    {
        return None;
    }
    let hm = f.home_sampler.native_masses()?;
    let am = f.away_sampler.native_masses()?;
    if hm.len().saturating_mul(am.len()) > entry_budget {
        return None;
    }
    let mut rows: [Vec<([i32; 2], f64)>; 3] = std::array::from_fn(|_| Vec::new());
    let mut probs = [0.; 3];
    for (h, hp) in hm.iter().enumerate() {
        for (a, ap) in am.iter().enumerate() {
            let o = if h < a {
                0
            } else if h == a {
                1
            } else {
                2
            };
            let mass = hp * ap;
            if mass > 0. {
                rows[o].push(([h as i32, a as i32], mass));
                probs[o] += mass;
            }
        }
    }
    let total: f64 = probs.iter().sum();
    if !(total > 0.) {
        return None;
    }
    for p in &mut probs {
        *p /= total;
    }
    Some((NativeFixture { rows }, probs))
}

fn choose_score(rows: &[([i32; 2], f64)], rng: &mut Rng) -> [i32; 2] {
    let total: f64 = rows.iter().map(|x| x.1).sum();
    let mut x = rng.float() * total;
    for &(score, p) in rows {
        if x < p {
            return score;
        }
        x -= p;
    }
    rows.last().unwrap().0
}

fn binomial_interval(hits: usize, samples: usize, alpha: f64) -> (f64, f64) {
    if samples == 0 {
        return (0., 1.);
    }
    let n = samples as f64;
    let k = hits.min(samples) as f64;
    let observed = k / n;
    let cutoff = (2. / alpha).ln();
    let kl = |p: f64, q: f64| {
        let mut value = 0.;
        if p > 0. {
            value += p * (p / q.max(f64::MIN_POSITIVE)).ln();
        }
        if p < 1. {
            value += (1. - p) * ((1. - p) / (1. - q).max(f64::MIN_POSITIVE)).ln();
        }
        value
    };
    let lower = if hits == 0 {
        0.
    } else {
        let (mut lo, mut hi) = (0., observed);
        for _ in 0..64 {
            let mid = (lo + hi) / 2.;
            if n * kl(observed, mid) > cutoff {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    };
    let upper = if hits == samples {
        1.
    } else {
        let (mut lo, mut hi) = (observed, 1.);
        for _ in 0..64 {
            let mid = (lo + hi) / 2.;
            if n * kl(observed, mid) > cutoff {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
    };
    (lower, upper)
}

#[cfg(test)]
mod tests {
    use super::{native_fixture, run};
    use crate::{
        model::{Fixture, Model, Request},
        pool::{Bounds, Estimate},
        rng::Rng,
        sampling::{Poisson, Scores},
    };

    fn fixture(home: f64, away: f64) -> Fixture {
        Fixture {
            home: 0,
            away: 1,
            request_index: 0,
            home_sampler: Poisson::new(home),
            away_sampler: Poisson::new(away),
            prob: [0.2, 0.3, 0.5],
            scores: Scores::new(home, away),
        }
    }

    #[test]
    fn native_conditional_score_rows_reconstruct_fixture_wdl_law() {
        for (home, away) in [(0., 0.), (0., 1.3), (1.3, 0.), (1.2, 0.8)] {
            let f = fixture(home, away);
            let (table, probabilities) = native_fixture(&f, 20_000).unwrap();
            let row_masses: [f64; 3] =
                std::array::from_fn(|o| table.rows[o].iter().map(|(_, p)| *p).sum());
            let total: f64 = row_masses.iter().sum();
            for outcome in 0..3 {
                assert!((row_masses[outcome] / total - probabilities[outcome]).abs() < 1e-14);
                if table.rows[outcome].is_empty() {
                    assert_eq!(probabilities[outcome], 0.);
                    continue;
                }
                let mut rng = Rng::new(91 + outcome as i64);
                for _ in 0..1000 {
                    let score = super::choose_score(&table.rows[outcome], &mut rng);
                    let observed = if score[0] < score[1] {
                        0
                    } else if score[0] == score[1] {
                        1
                    } else {
                        2
                    };
                    assert_eq!(observed, outcome);
                }
            }
        }
    }

    #[test]
    fn native_score_support_rejects_means_outside_bounded_design() {
        assert!(native_fixture(&fixture(33., 1.), 20_000).is_none());
        assert!(native_fixture(&fixture(1., -1.), 20_000).is_none());
    }

    fn tiny_model(power: f64) -> Model {
        Model::new(serde_json::from_value::<Request>(serde_json::json!({
            "id": 91,
            "phase": {"sort":"pt,gd,gf,bias", "championship":{"point_loss":0,"point_draw":1,"point_win":3}},
            "team_groups":[{"team_id":1,"bias":20},{"team_id":2,"bias":10},{"team_id":3,"bias":0}],
            "games":[
                {"id":1,"home_id":1,"away_id":2,"home_power":power,"away_power":power},
                {"id":2,"home_id":1,"away_id":3,"home_power":power,"away_power":power}
            ]
        })).unwrap()).unwrap()
    }

    fn enumerate(
        model: &Model,
        tables: &[super::NativeFixture],
    ) -> (Vec<f64>, Vec<f64>, Vec<f64>, f64) {
        fn visit(
            model: &Model,
            tables: &[super::NativeFixture],
            fixture: usize,
            scores: &mut Vec<[i32; 2]>,
            mass: f64,
            focal: usize,
            rank_mass: &mut [f64],
            event_mass: &mut [f64],
            joint: &mut [f64],
            title_mass: &mut [f64],
            overlap: &mut f64,
        ) {
            if fixture < tables.len() {
                for row in &tables[fixture].rows {
                    for &(score, p) in row {
                        scores.push(score);
                        visit(
                            model,
                            tables,
                            fixture + 1,
                            scores,
                            mass * p,
                            focal,
                            rank_mass,
                            event_mass,
                            joint,
                            title_mass,
                            overlap,
                        );
                        scores.pop();
                    }
                }
                return;
            }
            let mut campaign = model.base.clone();
            let mut score_vec = model.empty_scores();
            for (i, f) in model.fixtures.iter().enumerate() {
                score_vec[f.request_index] = scores[i];
                model.add(&mut campaign, i, scores[i]);
            }
            let mut order = vec![0; model.n];
            model.standings(
                &mut order,
                &campaign,
                &score_vec,
                &mut crate::rng::Rng::new(3),
            );
            let focal_rank = order.iter().position(|&t| t == focal).unwrap();
            rank_mass[focal_rank] += mass;
            if order[0] != focal {
                joint[focal_rank] += mass;
            }
            let bounds = Bounds::new(model);
            let mut member = vec![false; model.n];
            for rival in 0..model.n {
                if rival != focal {
                    let rival_added = campaign[rival].points - model.base[rival].points;
                    member[rival] = campaign[rival].points >= campaign[focal].points
                        && bounds.allowed(rival, 0, rival_added);
                    if member[rival] {
                        event_mass[rival] += mass;
                    }
                    if order[0] == rival {
                        title_mass[rival] += mass;
                    }
                }
            }
            if member.iter().filter(|&&b| b).count() >= 2 {
                *overlap += mass;
            }
            if order[0] != focal {
                let winner = order[0];
                assert!(
                    member[winner],
                    "the actual rival winner must lie in its necessary event"
                );
            }
        }
        let mut rank_mass = vec![0.; model.n];
        let mut event_mass = vec![0.; model.n];
        let mut joint = vec![0.; model.n];
        let mut title_mass = vec![0.; model.n];
        let mut overlap = 0.;
        visit(
            model,
            tables,
            0,
            &mut Vec::new(),
            1.,
            0,
            &mut rank_mass,
            &mut event_mass,
            &mut joint,
            &mut title_mass,
            &mut overlap,
        );
        (rank_mass, joint, title_mass, overlap)
    }

    #[test]
    fn complete_native_full_world_indicators_reduce_saturated_title() {
        let model = tiny_model(0.8);
        let tables: Vec<_> = model
            .fixtures
            .iter()
            .map(|f| native_fixture(f, 20_000).unwrap().0)
            .collect();
        let (_, exact_complement, exact_title, overlap) = enumerate(&model, &tables);
        assert!(
            overlap > 0.,
            "necessary rival events overlap in complete seasons"
        );
        let exact_epsilon: f64 = exact_complement.iter().skip(1).sum();
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        estimates[0].probability = 1.;
        estimates[0].samples = 100_000;
        let pass = run(&model, 812, &mut estimates);
        assert_eq!(pass.components, 2);
        assert!(estimates[0].probability < 1.);
        assert!(estimates[1].probability > 0. || estimates[2].probability > 0.);
        for rival in 1..model.n {
            assert!(estimates[rival * model.n].meets_precision_goal);
        }
        let (_, first_lo, first_hi) = pass.intervals.iter().find(|x| x.0 == 0).unwrap();
        assert!(estimates[0].probability >= *first_lo && estimates[0].probability <= *first_hi);
        assert!((estimates[0].probability - (1. - exact_epsilon)).abs() <= first_hi - first_lo);
        for r in 1..model.n {
            let (idx, lo, hi) = pass.intervals.iter().find(|x| x.0 == r).unwrap();
            assert_eq!(*idx, r);
            assert!(estimates[r].probability >= *lo && estimates[r].probability <= *hi);
            assert!(exact_complement[r] >= *lo && exact_complement[r] <= *hi);
            let title_index = r * model.n;
            let (_, title_lo, title_hi) =
                pass.intervals.iter().find(|x| x.0 == title_index).unwrap();
            assert!(
                estimates[title_index].probability >= *title_lo
                    && estimates[title_index].probability <= *title_hi
            );
            assert!(exact_title[r] >= *title_lo && exact_title[r] <= *title_hi);
        }
        let sum: f64 = estimates[..model.n].iter().map(|e| e.probability).sum();
        assert!((sum - 1.).abs() < 1e-12);

        let mut repeated = vec![Estimate::default(); model.n * model.n];
        repeated[0].probability = 1.;
        repeated[0].samples = 100_000;
        let again = run(&model, 812, &mut repeated);
        assert_eq!(again.components, pass.components);
        assert_eq!(
            estimates
                .iter()
                .map(|e| e.probability.to_bits())
                .collect::<Vec<_>>(),
            repeated
                .iter()
                .map(|e| e.probability.to_bits())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn unsupported_native_mean_leaves_the_estimate_atomic() {
        let model = tiny_model(33.);
        let mut estimates = vec![Estimate::default(); model.n * model.n];
        estimates[0].probability = 1.;
        let before = estimates.iter().map(|e| e.probability).collect::<Vec<_>>();
        let pass = run(&model, 9, &mut estimates);
        assert_eq!(pass.components, 0);
        assert_eq!(
            before,
            estimates.iter().map(|e| e.probability).collect::<Vec<_>>()
        );
    }
}

/// Complete a bounded title complement pass. A failure leaves all numbers intact.
pub(crate) fn run(model: &Model, seed: i64, estimates: &mut [Estimate]) -> PassResult {
    let mut last_epsilon_interval = None;
    let fail = |work, reason, accepted_targets, reserved_prep, epsilon_interval| PassResult {
        components: 0,
        work,
        intervals: Vec::new(),
        reason,
        accepted_targets,
        reserved_prep,
        epsilon_interval,
    };
    if model.n < 2
        || model.n > 64
        || model.request.phase.bonus_points != 0
        || model.keys.first() != Some(&crate::model::Key::Pt)
        || model.request.phase.championship.point_win < 0
        || model.request.phase.championship.point_draw < 0
        || model.request.phase.championship.point_loss < 0
        || model.fixtures.iter().any(|f| f.home == f.away)
    {
        return fail(0, "unsupported_model", 0, 0, None);
    }
    let targets: Vec<_> = (0..model.n)
        .filter(|&t| {
            estimates[t * model.n].probability == 1.
                && estimates[t * model.n].design != "certain_by_points"
        })
        .collect();
    if targets.is_empty() || targets.len() > TARGET_LIMIT {
        return fail(0, "no_saturated_title_or_target_cap", 0, 0, None);
    }
    let mut tables = Vec::with_capacity(model.fixtures.len());
    let mut native_probs = Vec::with_capacity(model.fixtures.len());
    let mut table_entries = 0usize;
    for f in &model.fixtures {
        let Some((table, p)) = native_fixture(f, TABLE_ENTRIES.saturating_sub(table_entries))
        else {
            return fail(
                table_entries as u64,
                "native_score_support_or_table_cap",
                targets.len(),
                table_entries,
                None,
            );
        };
        table_entries =
            table_entries.saturating_add(table.rows.iter().map(Vec::len).sum::<usize>());
        if table_entries > TABLE_ENTRIES {
            return fail(
                table_entries as u64,
                "native_table_cap",
                targets.len(),
                table_entries,
                None,
            );
        }
        tables.push(table);
        native_probs.push(p);
    }
    let components = targets.len().saturating_mul(model.n - 1);
    let draws = DRAWS / components;
    if draws == 0 {
        return fail(
            table_entries as u64,
            "draw_budget",
            targets.len(),
            table_entries,
            None,
        );
    }
    let bounds = Bounds::new(model);
    let rules = &model.request.phase.championship;
    let max_gain = rules.point_win.max(rules.point_draw).max(rules.point_loss) as usize;
    let incident = |team: usize| {
        model
            .fixtures
            .iter()
            .filter(|f| f.home == team || f.away == team)
            .count()
    };
    let mut reserved_prep = table_entries;
    for &focal in &targets {
        for rival in 0..model.n {
            if rival == focal {
                continue;
            }
            let games = model
                .fixtures
                .iter()
                .filter(|f| [focal, rival].contains(&f.home) || [focal, rival].contains(&f.away))
                .count();
            let width_f = incident(focal).saturating_mul(max_gain).saturating_add(1);
            let width_r = incident(rival).saturating_mul(max_gain).saturating_add(1);
            let states = width_f.saturating_mul(width_r).min(EVENT_STATES + 1);
            let nodes = games.saturating_add(1).saturating_mul(states);
            let reserve = 3usize.saturating_mul(nodes);
            if nodes > 1_000_000 || reserved_prep.saturating_add(reserve) > PREP_WORK {
                return fail(
                    reserved_prep as u64,
                    "dp_preflight_cap",
                    targets.len(),
                    reserved_prep,
                    None,
                );
            }
            reserved_prep += reserve;
        }
    }
    let mut proposed = estimates.to_vec();
    let mut work = 0u64;
    let mut count = 0usize;
    let mut all_intervals = Vec::new();
    for &focal in &targets {
        let mut other = vec![0f64; model.n];
        let mut epsilon = 0.;
        let mut epsilon_lower = 0.;
        let mut epsilon_upper = 0.;
        let mut epsilon_variance = 0.;
        let mut epsilon_weighted_hits = 0.;
        let mut epsilon_weighted_sq = 0.;
        let mut epsilon_max_contribution = 0.0f64;
        let mut conditional_hits = 0usize;
        let mut marginals = vec![0.; model.n * model.n];
        let mut marginal_lower = vec![0.; model.n * model.n];
        let mut marginal_upper = vec![0.; model.n * model.n];
        let mut marginal_variance = vec![0.; model.n * model.n];
        let mut marginal_hits = vec![0usize; model.n * model.n];
        let mut marginal_weighted_hits = vec![0.; model.n * model.n];
        let mut marginal_weighted_sq = vec![0.; model.n * model.n];
        let mut marginal_max_contribution = vec![0.0f64; model.n * model.n];
        let mut marginal_upper95 = vec![0.; model.n * model.n];
        let mut total_samples = 0usize;
        for rival in 0..model.n {
            if rival == focal {
                continue;
            }
            let Some(event) = Event::build_with_probabilities(
                model,
                &bounds,
                rival,
                0,
                &[focal],
                EVENT_STATES,
                &native_probs,
            ) else {
                return fail(
                    work.saturating_add(reserved_prep as u64),
                    "event_build_failed",
                    count,
                    reserved_prep,
                    last_epsilon_interval,
                );
            };
            if event.mass <= 0. {
                if !event.terminals.is_empty() {
                    return fail(
                        work.saturating_add(reserved_prep as u64),
                        "zero_mass_nonempty_event",
                        count,
                        reserved_prep,
                        last_epsilon_interval,
                    );
                }
                continue;
            }
            let mut backward = Backward::new(&event);
            let mut rng = Rng::new(derive(
                seed,
                &format!("native-complement-{focal}-{rival}-0"),
            ));
            let selected: Vec<bool> = (0..model.fixtures.len())
                .map(|i| event.games.iter().any(|g| g.index == i))
                .collect();
            let mut outcomes = vec![0u8; model.fixtures.len()];
            let mut scores = model.empty_scores();
            let mut campaign = model.base.clone();
            let mut order = vec![0usize; model.n];
            let mut hits = vec![0usize; model.n];
            let mut occupied = vec![vec![0usize; model.n]; model.n];
            for _ in 0..draws {
                backward.sample(&mut rng, &mut outcomes);
                campaign.copy_from_slice(&model.base);
                for (i, f) in model.fixtures.iter().enumerate() {
                    let sc = if selected[i] {
                        choose_score(&tables[i].rows[outcomes[i] as usize], &mut rng)
                    } else {
                        [
                            f.home_sampler.sample(&mut rng),
                            f.away_sampler.sample(&mut rng),
                        ]
                    };
                    scores[f.request_index] = sc;
                    model.add(&mut campaign, i, sc);
                }
                model.standings(&mut order, &campaign, &scores, &mut rng);
                if order[0] == rival {
                    let r = order.iter().position(|&t| t == focal).unwrap();
                    hits[r] += 1;
                    for (rank, &team) in order.iter().enumerate() {
                        occupied[team][rank] += 1;
                    }
                }
            }
            let mass = event.mass;
            let any_hits = hits.iter().sum::<usize>();
            let any = any_hits as f64 / draws as f64;
            epsilon += any * mass;
            epsilon_variance += mass * mass * any * (1. - any) / draws as f64;
            epsilon_weighted_hits += mass * any_hits as f64;
            epsilon_weighted_sq += mass * mass * any_hits as f64;
            if any_hits > 0 {
                epsilon_max_contribution = epsilon_max_contribution.max(mass);
            }
            let alpha = ALPHA / (components as f64 * (model.n * model.n + 1) as f64);
            let (any_lo, any_hi) = binomial_interval(any_hits, draws, alpha);
            epsilon_lower += mass * any_lo;
            epsilon_upper += mass * any_hi;
            conditional_hits += any_hits;
            total_samples += draws;
            for r in 1..model.n {
                other[r] += hits[r] as f64 / draws as f64 * mass;
            }
            for t in 0..model.n {
                for r in 0..model.n {
                    let i = t * model.n + r;
                    let cell_hits = occupied[t][r];
                    let q = cell_hits as f64 / draws as f64;
                    marginals[i] += q * mass;
                    // Since the sampled full standings has `rival` at rank
                    // zero, every other team's rank-zero indicator is exactly
                    // zero, even when its necessary point event overlaps.
                    let structurally_zero = r == 0 && t != rival;
                    if structurally_zero && cell_hits != 0 {
                        return fail(
                            work.saturating_add(reserved_prep as u64),
                            "winner_partition_contradiction",
                            count,
                            reserved_prep,
                            last_epsilon_interval,
                        );
                    }
                    let (cell_lo, cell_hi) = if structurally_zero {
                        (0., 0.)
                    } else {
                        binomial_interval(cell_hits, draws, alpha)
                    };
                    marginal_lower[i] += cell_lo * mass;
                    marginal_upper[i] += cell_hi * mass;
                    marginal_variance[i] += mass * mass * q * (1. - q) / draws as f64;
                    marginal_hits[i] += cell_hits;
                    marginal_weighted_hits[i] += mass * cell_hits as f64;
                    marginal_weighted_sq[i] += mass * mass * cell_hits as f64;
                    if cell_hits > 0 {
                        marginal_max_contribution[i] = marginal_max_contribution[i].max(mass);
                    }
                    if cell_hits == 0 && !structurally_zero {
                        marginal_upper95[i] += mass * cell_hi;
                    }
                }
            }
            work = work.saturating_add(draws as u64 * crate::pool::work_per_sample(model));
            count += 1;
        }
        let eps_lo = epsilon_lower.max(0.);
        let eps_hi = epsilon_upper.min(1.);
        last_epsilon_interval = Some((eps_lo, eps_hi));
        if eps_lo <= 64. * f64::EPSILON || eps_hi > 2. * eps_lo {
            return fail(
                work.saturating_add(reserved_prep as u64),
                "complement_ci_unqualified",
                count,
                reserved_prep,
                last_epsilon_interval,
            );
        }
        let e = &mut proposed[focal * model.n];
        e.probability = (1. - epsilon).clamp(0., 1.);
        e.design = "native_full_world_complement".into();
        e.conditional_mass = 0.;
        e.samples = estimates[focal * model.n].samples;
        e.conditional_samples = total_samples;
        e.conditional_hits = conditional_hits;
        e.ess = if epsilon_weighted_sq > 0. {
            epsilon_weighted_hits.powi(2) / epsilon_weighted_sq
        } else {
            0.
        };
        e.max_event_weight_share = if epsilon_weighted_hits > 0. {
            epsilon_max_contribution / epsilon_weighted_hits
        } else {
            0.
        };
        e.zero_hit_upper_95 = 0.;
        e.std_err = epsilon_variance.sqrt();
        e.relative_se = Some(e.std_err / e.probability.max(f64::MIN_POSITIVE));
        e.meets_precision_goal = (1. - eps_hi) > 0. && (1. - eps_lo) <= 2. * (1. - eps_hi);
        all_intervals.push((
            focal * model.n,
            (1. - eps_hi).max(0.),
            (1. - eps_lo).min(1.),
        ));
        // Rival title cells and the focal team's remaining ranks are all
        // recovered from the same disjoint full-world indicators.
        for r in 1..model.n {
            let p = other[r];
            let i = focal * model.n + r;
            if proposed[i].reachability.starts_with("impossible") {
                if p > 0. {
                    return fail(
                        work.saturating_add(reserved_prep as u64),
                        "impossible_focal_cell_observed",
                        count,
                        reserved_prep,
                        last_epsilon_interval,
                    );
                }
                continue;
            }
            proposed[i].probability = p;
            proposed[i].design = "native_full_world_complement".into();
            proposed[i].conditional_mass = 0.;
            proposed[i].std_err = marginal_variance[i].sqrt();
            proposed[i].relative_se = Some(proposed[i].std_err / p.max(f64::MIN_POSITIVE));
            proposed[i].meets_precision_goal =
                p > 0. && marginal_upper[i] <= 2. * marginal_lower[i];
            proposed[i].conditional_samples = total_samples;
            proposed[i].conditional_hits = marginal_hits[i];
            proposed[i].ess = if marginal_weighted_sq[i] > 0. {
                marginal_weighted_hits[i].powi(2) / marginal_weighted_sq[i]
            } else {
                0.
            };
            proposed[i].max_event_weight_share = if marginal_weighted_hits[i] > 0. {
                marginal_max_contribution[i] / marginal_weighted_hits[i]
            } else {
                0.
            };
            proposed[i].zero_hit_upper_95 = if marginal_hits[i] == 0 {
                marginal_upper95[i]
            } else {
                0.
            };
            if marginal_hits[i] > 0 {
                proposed[i].reachability = "witness".into();
            }
            all_intervals.push((
                focal * model.n + r,
                marginal_lower[focal * model.n + r],
                marginal_upper[focal * model.n + r].min(1.),
            ));
        }
        for rival in 0..model.n {
            if rival != focal {
                let i = rival * model.n;
                if proposed[i].reachability.starts_with("impossible") {
                    if marginals[i] > 0. {
                        return fail(
                            work.saturating_add(reserved_prep as u64),
                            "impossible_rival_cell_observed",
                            count,
                            reserved_prep,
                            last_epsilon_interval,
                        );
                    }
                    continue;
                }
                if proposed[i].probability == 0. || proposed[i].probability == 1. {
                    proposed[i].probability = marginals[i];
                    proposed[i].design = "native_full_world_complement".into();
                    proposed[i].conditional_mass = 0.;
                    proposed[i].std_err = marginal_variance[i].sqrt();
                    proposed[i].relative_se =
                        Some(proposed[i].std_err / marginals[i].max(f64::MIN_POSITIVE));
                    proposed[i].meets_precision_goal =
                        marginals[i] > 0. && marginal_upper[i] <= 2. * marginal_lower[i];
                    proposed[i].conditional_samples = total_samples;
                    proposed[i].conditional_hits = marginal_hits[i];
                    proposed[i].ess = if marginal_weighted_sq[i] > 0. {
                        marginal_weighted_hits[i].powi(2) / marginal_weighted_sq[i]
                    } else {
                        0.
                    };
                    proposed[i].max_event_weight_share = if marginal_weighted_hits[i] > 0. {
                        marginal_max_contribution[i] / marginal_weighted_hits[i]
                    } else {
                        0.
                    };
                    proposed[i].zero_hit_upper_95 = if marginal_hits[i] == 0 {
                        marginal_upper95[i]
                    } else {
                        0.
                    };
                    if marginal_hits[i] > 0 {
                        proposed[i].reachability = "witness".into();
                    }
                    all_intervals.push((i, marginal_lower[i], marginal_upper[i].min(1.)));
                }
            }
        }
    }
    estimates.clone_from_slice(&proposed);
    PassResult {
        components: count,
        work: work.saturating_add(reserved_prep as u64),
        intervals: all_intervals,
        reason: "qualified",
        accepted_targets: targets.len(),
        reserved_prep,
        epsilon_interval: last_epsilon_interval,
    }
}
