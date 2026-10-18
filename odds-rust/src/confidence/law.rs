//! Change-of-law and finite-kernel enclosures for conditioned fast sampling.
use crate::{
    conditioned::Event,
    confidence::{
        tickets::{categorical_masses, Comparison},
        Interval,
    },
    model::{Key, Model},
};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) struct Bridge {
    ratio: Interval,
    score_tv: f64,
}

impl Bridge {
    pub(crate) fn build(model: &Model) -> Option<Self> {
        let mut ratio = Interval::new(1.0, 1.0);
        let mut score_tv = 0.0f64;
        for f in &model.fixtures {
            let model_wdl = normalized_wdl(f.prob)?;
            let native_home = f.home_sampler.certificate_masses()?;
            let native_away = f.away_sampler.certificate_masses()?;
            let mut native_wdl = [Interval::new(0.0, 0.0); 3];
            for (h, &hp) in native_home.iter().enumerate() {
                for (a, &ap) in native_away.iter().enumerate() {
                    let o = outcome(h, a);
                    native_wdl[o] = native_wdl[o].add(hp.mul(ap));
                }
            }

            let mut r_lo = f64::INFINITY;
            let mut r_hi: f64 = 0.0;
            let mut fixture_score_tv = 0.0f64;
            for o in 0..3 {
                if model_wdl[o].lower <= 0.0 || native_wdl[o].lower <= 0.0 {
                    return None;
                }
                let lr = divide(native_wdl[o], model_wdl[o])?;
                r_lo = r_lo.min(lr.lower);
                r_hi = r_hi.max(lr.upper);

                let native_conditional =
                    native_conditional_scores(&native_home, &native_away, native_wdl[o], o)?;
                let model_conditional = f.scores.certificate_conditional_scores(o)?;
                let tv = score_distribution_tv(&native_conditional, &model_conditional);
                fixture_score_tv = fixture_score_tv.max(tv);
            }
            ratio = ratio.mul(Interval::new(r_lo, r_hi));
            score_tv = (score_tv + fixture_score_tv).next_up().min(1.0);
        }
        Some(Self {
            ratio,
            score_tv: score_tv.min(1.0),
        })
    }

    pub(crate) fn convert(&self, model_joint: Interval, region_mass: Interval) -> Option<Interval> {
        if !valid(model_joint) || !valid(region_mass) {
            return None;
        }
        let shift = region_mass
            .upper
            .mul_add(self.score_tv, 0.0)
            .next_up()
            .min(1.0);
        let conditional = Interval::new(
            (model_joint.lower - shift).next_down().max(0.0),
            (model_joint.upper + shift).next_up().min(1.0),
        );
        Some(clamp01(self.ratio.mul(conditional)))
    }
}

/// Return the ideal normalized-model event mass and a bound on the entire
/// event sampler's total-variation error. The bound includes terminal ticket
/// selection, each backward kernel's worst reachable-state ticket error, and
/// the finite-ticket errors in every unconditioned fixture draw.
pub(crate) fn conditioned_law(
    model: &Model,
    event: &Event,
    target: usize,
    rank: usize,
) -> Option<(Interval, f64)> {
    // This counter includes each call's retained-state precheck/integrity
    // validation and any first-time cached law construction.
    let started = std::time::Instant::now();
    let result = conditioned_law_inner(model, event, target, rank);
    model
        .confidence
        .record_setup_ns(started.elapsed().as_nanos().min(u64::MAX as u128) as u64);
    result
}

fn conditioned_law_inner(
    model: &Model,
    event: &Event,
    target: usize,
    rank: usize,
) -> Option<(Interval, f64)> {
    if event.certificate_cell != Some((target, rank))
        || event.teams.first() != Some(&target)
        || model.keys.first() != Some(&Key::Pt)
        || model.request.phase.bonus_points != 0
        || event.certificate_state_count() > Event::CERTIFICATE_STATE_LIMIT
        || !event.certificate_intact_for(model)
    {
        return None;
    }
    event.cached_certificate_law(|| compute_conditioned_law(model, event, target, rank))
}

fn compute_conditioned_law(
    model: &Model,
    event: &Event,
    target: usize,
    rank: usize,
) -> Option<(Interval, f64)> {
    // The bounded, full integrity and model-identity check is performed
    // before cached lookup in `conditioned_law_inner`.
    if event.certificate_cell != Some((target, rank)) || event.teams.first() != Some(&target) {
        return None;
    }
    let states = event.certificate_state_count();
    if states > Event::CERTIFICATE_STATE_LIMIT {
        return None;
    }
    if event.forward.len() != event.games.len() + 1
        || event.forward.first()?.len() != 1
        || event.forward.first()?.get(&0).is_none()
    {
        return None;
    }
    if event
        .games
        .iter()
        .any(|g| model.fixtures.get(g.index).is_none_or(|f| g.prob != f.prob))
    {
        return None;
    }
    let mut selected = vec![false; model.fixtures.len()];
    for g in &event.games {
        if std::mem::replace(&mut selected[g.index], true) {
            return None;
        }
    }
    let teams = &event.teams;
    for (i, f) in model.fixtures.iter().enumerate() {
        let expected = teams.contains(&f.home) || teams.contains(&f.away);
        if selected[i] != expected {
            return None;
        }
    }

    let probabilities: Vec<_> = model
        .fixtures
        .iter()
        .map(|f| normalized_wdl(f.prob))
        .collect::<Option<_>>()?;
    let layers = directed_layers(event, &probabilities)?;
    let mut region = Interval::new(0.0, 0.0);
    for (state, _) in &event.terminals {
        region = region.add(*layers.last()?.get(state)?);
    }
    if region.upper <= 0.0 {
        return None;
    }

    let mut tv = terminal_tv(event, layers.last()?)?;
    for step in 0..event.games.len() {
        let g = &event.games[step];
        let mut worst: f64 = 0.0;
        for (&state, _) in event.forward[step + 1].iter() {
            if let Some(intended) =
                transition_intended(event, &layers, step, state, probabilities[g.index])
            {
                if let Some(actual) = transition_actual(event, step, state, g) {
                    worst = worst.max(distribution_tv(&intended, &actual));
                } else {
                    worst = 1.0;
                }
            } else {
                worst = 1.0;
            }
        }
        tv = (tv + worst).next_up().min(1.0);
    }
    for (i, fixture) in model.fixtures.iter().enumerate() {
        if selected[i] {
            continue;
        }
        let intended = probabilities[i];
        let actual = wdl_ticket_masses(fixture.prob)?;
        tv = (tv + distribution_tv(&intended, &actual))
            .next_up()
            .min(1.0);
    }
    Some((clamp01(region), tv))
}

fn directed_layers(
    event: &Event,
    probabilities: &[[Interval; 3]],
) -> Option<Vec<HashMap<u64, Interval>>> {
    let mut layers = Vec::with_capacity(event.forward.len());
    let mut first = HashMap::new();
    first.insert(0, Interval::new(1.0, 1.0));
    layers.push(first);
    for (step, game) in event.games.iter().enumerate() {
        let previous = layers.last()?;
        let mut next = HashMap::new();
        for (&state, _) in event.forward[step + 1].iter() {
            let mut total = Interval::new(0.0, 0.0);
            for o in 0..3 {
                if !can_subtract(state, game.delta[o], event.teams.len()) {
                    continue;
                }
                let predecessor = state - game.delta[o];
                if let Some(mass) = previous.get(&predecessor) {
                    total = total.add(mass.mul(probabilities[game.index][o]));
                }
            }
            next.insert(state, total);
        }
        // A zero upper bound for every retained state indicates a missing or
        // underflowed positive path; reject rather than certify lost support.
        if next.values().all(|p| p.upper == 0.0) {
            return None;
        }
        layers.push(next);
    }
    Some(layers)
}

fn terminal_tv(event: &Event, final_layer: &HashMap<u64, Interval>) -> Option<f64> {
    let ideal_raw: Vec<_> = event
        .terminals
        .iter()
        .map(|(s, _)| final_layer.get(s).copied())
        .collect::<Option<_>>()?;
    let total = sum_intervals(&ideal_raw);
    if total.lower <= 0.0 {
        return None;
    }
    let ideal: Vec<_> = ideal_raw
        .into_iter()
        .map(|p| divide(p, total))
        .collect::<Option<_>>()?;
    let thresholds: Vec<_> = event
        .terminals
        .iter()
        .take(event.terminals.len().saturating_sub(1))
        .map(|(_, c)| *c)
        .collect();
    let actual = categorical_masses(&thresholds, event.mass, Comparison::LessEqual)?;
    Some(distribution_tv(&ideal, &actual))
}

fn transition_intended(
    event: &Event,
    layers: &[HashMap<u64, Interval>],
    step: usize,
    state: u64,
    p: [Interval; 3],
) -> Option<Vec<Interval>> {
    let game = &event.games[step];
    let previous = &layers[step];
    let mut weights = [Interval::new(0.0, 0.0); 3];
    for o in 0..3 {
        if can_subtract(state, game.delta[o], event.teams.len()) {
            if let Some(mass) = previous.get(&(state - game.delta[o])) {
                weights[o] = mass.mul(p[o]);
            }
        }
    }
    let total = sum_intervals(&weights);
    if total.lower <= 0.0 {
        return None;
    }
    weights.into_iter().map(|w| divide(w, total)).collect()
}

fn transition_actual(
    event: &Event,
    step: usize,
    state: u64,
    game: &crate::conditioned::PointGame,
) -> Option<Vec<Interval>> {
    let mut weights = [0.0; 3];
    for o in 0..3 {
        if can_subtract(state, game.delta[o], event.teams.len()) {
            weights[o] = event.forward[step]
                .get(&(state - game.delta[o]))
                .copied()
                .unwrap_or(0.0)
                * game.prob[o];
        }
    }
    let total: f64 = weights.iter().sum();
    if !total.is_finite() || total <= 0.0 {
        return None;
    }
    let boundaries = [weights[0], weights[0] + weights[1]];
    categorical_masses(&boundaries, total, Comparison::Less)
}

fn wdl_ticket_masses(p: [f64; 3]) -> Option<Vec<Interval>> {
    if p.iter().any(|x| !x.is_finite() || *x < 0.0) {
        return None;
    }
    categorical_masses(&[p[0], p[0] + p[1]], 1.0, Comparison::Less)
}

fn normalized_wdl(p: [f64; 3]) -> Option<[Interval; 3]> {
    if p.iter().any(|x| !x.is_finite() || *x < 0.0) {
        return None;
    }
    let raw = p.map(|x| Interval::new(x, x));
    let total = sum_intervals(&raw);
    if total.lower <= 0.0 {
        return None;
    }
    Some(
        raw.map(|x| divide(x, total))
            .into_iter()
            .collect::<Option<Vec<_>>>()?
            .try_into()
            .ok()?,
    )
}

fn native_conditional_scores(
    home: &[Interval],
    away: &[Interval],
    wdl: Interval,
    o: usize,
) -> Option<Vec<([i32; 2], Interval)>> {
    let mut scores = Vec::new();
    for (h, &hp) in home.iter().enumerate() {
        for (a, &ap) in away.iter().enumerate() {
            if outcome(h, a) == o {
                scores.push(([h as i32, a as i32], divide(hp.mul(ap), wdl)?));
            }
        }
    }
    Some(scores)
}

fn distribution_tv(a: &[Interval], b: &[Interval]) -> f64 {
    let n = a.len().max(b.len());
    let mut l1 = 0.0;
    for i in 0..n {
        let x = a.get(i).copied().unwrap_or(Interval::new(0.0, 0.0));
        let y = b.get(i).copied().unwrap_or(Interval::new(0.0, 0.0));
        let upper = (x.upper - y.lower)
            .abs()
            .max((y.upper - x.lower).abs())
            .next_up();
        l1 = l1.add_nonnegative(upper);
    }
    (l1 * 0.5).next_up().min(1.0)
}

fn score_distribution_tv<K: Eq + std::hash::Hash>(a: &[(K, Interval)], b: &[(K, Interval)]) -> f64 {
    let mut values = HashMap::<&K, (Interval, Interval)>::new();
    for (k, v) in a {
        values
            .entry(k)
            .or_insert((Interval::new(0.0, 0.0), Interval::new(0.0, 0.0)))
            .0 = *v;
    }
    for (k, v) in b {
        values
            .entry(k)
            .or_insert((Interval::new(0.0, 0.0), Interval::new(0.0, 0.0)))
            .1 = *v;
    }
    let mut l1 = 0.0f64;
    for (_, (x, y)) in values {
        l1 = l1.add_nonnegative(
            (x.upper - y.lower)
                .abs()
                .max((y.upper - x.lower).abs())
                .next_up(),
        );
    }
    (l1 * 0.5).next_up().min(1.0)
}

fn sum_intervals(xs: &[Interval]) -> Interval {
    xs.iter()
        .copied()
        .fold(Interval::new(0.0, 0.0), |a, b| a.add(b))
}
trait NonnegativeAdd {
    fn add_nonnegative(self, rhs: f64) -> f64;
}
impl NonnegativeAdd for f64 {
    fn add_nonnegative(self, rhs: f64) -> f64 {
        (self + rhs).next_up()
    }
}
fn clamp01(i: Interval) -> Interval {
    Interval::new(i.lower.max(0.0), i.upper.min(1.0))
}
fn divide(a: Interval, b: Interval) -> Option<Interval> {
    if b.lower <= 0.0 {
        return None;
    }
    Some(Interval::new(
        (a.lower / b.upper).next_down().max(0.0),
        (a.upper / b.lower).next_up(),
    ))
}
fn outcome(h: usize, a: usize) -> usize {
    if h < a {
        0
    } else if h == a {
        1
    } else {
        2
    }
}
fn can_subtract(state: u64, delta: u64, slots: usize) -> bool {
    (0..slots).all(|slot| ((state >> (8 * slot)) & 255) >= ((delta >> (8 * slot)) & 255))
}
fn valid(i: Interval) -> bool {
    i.lower.is_finite()
        && i.upper.is_finite()
        && i.lower >= 0.0
        && i.upper <= 1.0
        && i.lower <= i.upper
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        conditioned::Event,
        model::{Championship, Game, Model, Phase, Request, Team},
        pool::Bounds,
    };

    fn tiny_model(home_power: f64, away_power: f64, sort: &str) -> Model {
        Model::new(Request {
            id: 10,
            zones: vec![],
            phase: Phase {
                sort: sort.into(),
                championship: Championship {
                    point_win: 3,
                    point_draw: 1,
                    point_loss: 0,
                },
                bonus_points: 0,
                bonus_points_threshold: 0,
            },
            games: vec![Game {
                id: 1,
                home_id: 1,
                away_id: 2,
                home_score: 0,
                away_score: 0,
                home_power,
                away_power,
                played: false,
            }],
            team_groups: vec![
                Team {
                    team_id: 1,
                    add_sub: 0,
                    bias: 0,
                },
                Team {
                    team_id: 2,
                    add_sub: 0,
                    bias: 0,
                },
            ],
        })
        .unwrap()
    }

    #[test]
    fn tiny_event_bridge_contains_enumerated_native_mass() {
        let model = tiny_model(0.8, 0.5, "pt");
        assert!(Bridge::build(&model).unwrap().score_tv > 0.0);
        let event = Event::build(&model, &Bounds::new(&model), 0, 0, &[1], 100).unwrap();
        let (region, tv) = conditioned_law(&model, &event, 0, 0).unwrap();
        let bridge = Bridge::build(&model).unwrap();
        let certified = bridge.convert(region, region).unwrap();
        let native_home = model.fixtures[0].home_sampler.certificate_masses().unwrap();
        let native_away = model.fixtures[0].away_sampler.certificate_masses().unwrap();
        let terminal_states: std::collections::HashSet<_> =
            event.terminals.iter().map(|(s, _)| *s).collect();
        let mut native_event = Interval::new(0.0, 0.0);
        for (h, hp) in native_home.iter().enumerate() {
            for (a, ap) in native_away.iter().enumerate() {
                let o = outcome(h, a);
                let state = event.games[0].delta[o];
                if terminal_states.contains(&state) {
                    native_event = native_event.add(hp.mul(*ap));
                }
            }
        }
        assert!(
            certified.lower <= native_event.lower && certified.upper >= native_event.upper,
            "certified={certified:?} native={native_event:?} region={region:?} tv={tv}"
        );
        assert!(tv >= 0.0 && tv <= 1.0);
    }

    #[test]
    fn bridge_fails_closed_when_wdl_support_is_missing() {
        let model = tiny_model(0.0, 0.0, "pt");
        assert!(Bridge::build(&model).is_none());
    }

    #[test]
    fn conditioned_fast_records_only_a_complete_fixed_batch() {
        let model = tiny_model(0.8, 0.5, "pt");
        let event = Event::build(&model, &Bounds::new(&model), 0, 0, &[1], 100).unwrap();
        crate::conditioned::fast(&model, &event, 0, 0, 32, 987);
        let certificate = model.confidence.finish(0, 0.5, false);
        assert_eq!(certificate.status, "bounded");
        assert_eq!(certificate.evidence_samples, 32);
        assert!(certificate
            .methods
            .iter()
            .any(|m| m == "conditioned_fast_is_native_bridge"));
    }

    #[test]
    fn custom_event_has_no_fast_certificate() {
        let model = tiny_model(0.8, 0.5, "pt");
        let probabilities = vec![[0.0, 1.0, 0.0]];
        let event = Event::build_with_probabilities(
            &model,
            &Bounds::new(&model),
            0,
            0,
            &[1],
            100,
            &probabilities,
        )
        .unwrap();
        crate::conditioned::fast(&model, &event, 0, 0, 8, 121);
        let certificate = model.confidence.finish(0, 0.5, false);
        assert_eq!(certificate.reason.as_deref(), Some("no_evidence"));
    }

    #[test]
    fn non_points_primary_event_has_no_certificate() {
        let model = tiny_model(0.8, 0.5, "gd,pt");
        let event = Event::build(&model, &Bounds::new(&model), 0, 0, &[1], 100).unwrap();
        crate::conditioned::fast(&model, &event, 0, 0, 8, 123);
        assert_eq!(
            model.confidence.finish(0, 0.5, false).reason.as_deref(),
            Some("no_evidence")
        );
    }

    #[test]
    fn fast_scope_rejects_custom_bounds_and_bonus_rules() {
        let mut model = tiny_model(0.8, 0.5, "pt");
        let mut bounds = Bounds::new(&model);
        bounds.max[0] += 1;
        let event = Event::build(&model, &bounds, 0, 0, &[1], 100).unwrap();
        crate::conditioned::fast(&model, &event, 0, 0, 8, 125);
        assert_eq!(
            model.confidence.finish(0, 0.5, false).reason.as_deref(),
            Some("no_evidence")
        );

        model.request.phase.bonus_points = 1;
        let bounds = Bounds::new(&model);
        let event = Event::build(&model, &bounds, 0, 0, &[1], 100).unwrap();
        crate::conditioned::fast(&model, &event, 0, 0, 8, 126);
        assert_eq!(
            model.confidence.finish(0, 0.5, false).reason.as_deref(),
            Some("no_evidence")
        );
    }

    #[test]
    fn cached_law_rejects_model_replacement_and_event_mutation() {
        let model = tiny_model(0.8, 0.5, "pt");
        let event = Event::build(&model, &Bounds::new(&model), 0, 0, &[1], 100).unwrap();
        assert!(conditioned_law(&model, &event, 0, 0).is_some());
        let replacement = tiny_model(0.8, 0.5, "pt");
        assert!(conditioned_law(&replacement, &event, 0, 0).is_none());
        let mut changed = Event::build(&model, &Bounds::new(&model), 0, 0, &[1], 100).unwrap();
        changed.mass = f64::from_bits(changed.mass.to_bits() + 1);
        assert!(conditioned_law(&model, &changed, 0, 0).is_none());
    }

    #[test]
    fn certificates_do_not_change_fast_sampling_results() {
        let enabled_result = {
            let _env = crate::test_env::scoped(&[("RUST_ODDS_CERTIFICATES", Some("1"))]);
            let model = tiny_model(0.8, 0.5, "pt");
            let event = Event::build(&model, &Bounds::new(&model), 0, 0, &[1], 100).unwrap();
            let result = crate::conditioned::fast(&model, &event, 0, 0, 128, 991);
            let ledger = model.confidence.finish(0, 0.5, false);
            assert_eq!(ledger.status, "bounded");
            assert_eq!(ledger.evidence_samples, 128);
            assert!(model.confidence.setup_ns() > 0);
            result
        };
        let disabled_result = {
            let _env = crate::test_env::scoped(&[("RUST_ODDS_CERTIFICATES", Some("0"))]);
            let model = tiny_model(0.8, 0.5, "pt");
            let event = Event::build(&model, &Bounds::new(&model), 0, 0, &[1], 100).unwrap();
            assert!(!model.confidence.enabled());
            let result = crate::conditioned::fast(&model, &event, 0, 0, 128, 991);
            assert!(model.confidence.claim_is(0).is_none());
            assert_eq!(model.confidence.setup_ns(), 0);
            result
        };
        assert_eq!(
            format!("{enabled_result:?}"),
            format!("{disabled_result:?}")
        );
    }
}
