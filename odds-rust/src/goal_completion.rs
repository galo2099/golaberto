//! Constructive completions of fixed W/D/L patterns. Independent fixtures
//! outside an earlier-key tie cohort give unbounded score adjustments. Failure
//! is unknown, never an impossibility proof. Scores exist only for verification.
use crate::{
    conditioned::canonical,
    model::{Campaign, Key, Model},
    rng::Rng,
    search::Cell,
};

#[derive(Clone, Debug)]
pub struct Adjustment {
    pub fixture: usize,
    pub margin: i32,
    pub equal_goals: i32,
}
pub struct Certificate {
    pub outcomes: Vec<u8>,
    pub adjustments: Vec<Adjustment>,
    pub ranks: Vec<usize>,
}
#[derive(Clone)]
struct State<'a> {
    model: &'a Model,
    c: Vec<Campaign>,
    scores: Vec<[i32; 2]>,
    adjustments: Vec<Adjustment>,
}
impl<'a> State<'a> {
    fn new(model: &'a Model, outcomes: &[u8]) -> Option<Self> {
        if outcomes.len() != model.fixtures.len()
            || outcomes.iter().any(|o| *o > 2)
            || model.fixtures.iter().any(|f| f.home == f.away)
        {
            return None;
        }
        let mut s = Self {
            model,
            c: model.base.clone(),
            scores: model.empty_scores(),
            adjustments: Vec::new(),
        };
        for (i, f) in model.fixtures.iter().enumerate() {
            let score = canonical(outcomes[i]);
            s.scores[f.request_index] = score;
            model.add(&mut s.c, i, score);
        }
        Some(s)
    }
    fn adjust(&mut self, fixture: usize, margin: i32, equal: i32) -> Option<()> {
        if margin < 0 || equal < 0 {
            return None;
        }
        let f = self.model.fixtures.get(fixture)?;
        let score = self.scores[f.request_index];
        let (hd, ad) = match score[0].cmp(&score[1]) {
            std::cmp::Ordering::Greater => (equal.checked_add(margin)?, equal),
            std::cmp::Ordering::Less => (equal, equal.checked_add(margin)?),
            std::cmp::Ordering::Equal if margin == 0 => (equal, equal),
            _ => return None,
        };
        let new = [score[0].checked_add(hd)?, score[1].checked_add(ad)?];
        let mut h = self.c[f.home];
        let mut a = self.c[f.away];
        h.gf = h.gf.checked_add(hd)?;
        h.ga = h.ga.checked_add(ad)?;
        a.gf = a.gf.checked_add(ad)?;
        a.ga = a.ga.checked_add(hd)?;
        a.away = a.away.checked_add(ad)?;
        self.c[f.home] = h;
        self.c[f.away] = a;
        self.scores[f.request_index] = new;
        self.adjustments.push(Adjustment {
            fixture,
            margin,
            equal_goals: equal,
        });
        Some(())
    }
    fn value(&self, t: usize, key: Key) -> i64 {
        if key == Key::Gd {
            i64::from(self.c[t].gf) - i64::from(self.c[t].ga)
        } else {
            i64::from(self.c[t].gf)
        }
    }
    fn prefix(&self, t: usize, wins: bool, gd: bool) -> (i32, i32, i64) {
        (
            self.c[t].points,
            if wins { self.c[t].wins } else { 0 },
            if gd { self.value(t, Key::Gd) } else { 0 },
        )
    }
    #[cold]
    #[inline(never)]
    fn verified(&self, outcomes: &[u8], cell: Cell) -> Option<Certificate> {
        let mut order = vec![0; self.model.n];
        self.model
            .standings(&mut order, &self.c, &self.scores, &mut Rng::new(1));
        if order.get(cell.rank) != Some(&cell.team) {
            return None;
        }
        let mut ranks = vec![0; self.model.n];
        for (r, t) in order.into_iter().enumerate() {
            ranks[t] = r;
        }
        Some(Certificate {
            outcomes: outcomes.to_vec(),
            adjustments: self.adjustments.clone(),
            ranks,
        })
    }
}
#[derive(Clone, Copy)]
struct Interval {
    base: i64,
    lo: i64,
    hi: i64,
    up: Option<usize>,
    down: Option<usize>,
}
fn intervals(
    s: &State<'_>,
    cohort: &[usize],
    key: Key,
    paths: bool,
) -> (Vec<Interval>, Vec<Vec<usize>>) {
    let mut inside = vec![false; s.c.len()];
    for &t in cohort {
        inside[t] = true;
    }
    let mut values: Vec<_> = cohort
        .iter()
        .map(|&t| {
            let base = s.value(t, key);
            Interval {
                base,
                lo: base,
                hi: base,
                up: None,
                down: None,
            }
        })
        .collect();
    let mut slots = vec![usize::MAX; s.c.len()];
    for (i, &t) in cohort.iter().enumerate() {
        slots[t] = i;
    }
    for (i, f) in s.model.fixtures.iter().enumerate() {
        // Both endpoints in the cohort require coupled reasoning, not this cut.
        if inside[f.home] == inside[f.away] {
            continue;
        }
        let (t, own, other) = if inside[f.home] {
            (f.home, 0, 1)
        } else {
            (f.away, 1, 0)
        };
        let v = &mut values[slots[t]];
        let score = s.scores[f.request_index];
        if key == Key::Gf || score[own] > score[other] {
            v.up.get_or_insert(i);
            v.hi = i64::MAX / 4;
        }
        if key == Key::Gd && score[own] < score[other] {
            v.down.get_or_insert(i);
            v.lo = i64::MIN / 4;
        }
    }
    let mut routes = Vec::new();
    if paths && key == Key::Gd && inside.iter().any(|v| !*v) {
        let mut forward = vec![Vec::new(); s.c.len()];
        let mut reverse = forward.clone();
        for (i, f) in s.model.fixtures.iter().enumerate() {
            let score = s.scores[f.request_index];
            let (winner, loser) = match score[0].cmp(&score[1]) {
                std::cmp::Ordering::Greater => (f.home, f.away),
                std::cmp::Ordering::Less => (f.away, f.home),
                _ => continue,
            };
            forward[winner].push((loser, i));
            reverse[loser].push((winner, i));
        }
        for (&t, v) in cohort.iter().zip(&mut values) {
            if v.up.is_none() {
                if let Some(route) = outside_path(&forward, &inside, t) {
                    v.up = Some(s.model.fixtures.len() + routes.len());
                    v.hi = i64::MAX / 4;
                    routes.push(route);
                }
            }
            if v.down.is_none() {
                if let Some(route) = outside_path(&reverse, &inside, t) {
                    v.down = Some(s.model.fixtures.len() + routes.len());
                    v.lo = i64::MIN / 4;
                    routes.push(route);
                }
            }
        }
    }
    (values, routes)
}
// A simple directed path changes GD only at its endpoints. Its interior can
// belong to the cohort: increased goals scored AND conceded cancel their GD.
// Later goals-scored ordering must still be verified after applying all paths.
fn outside_path(
    edges: &[Vec<(usize, usize)>],
    inside: &[bool],
    start: usize,
) -> Option<Vec<usize>> {
    let mut previous = vec![None; edges.len()];
    let mut queue = vec![start];
    previous[start] = Some((start, usize::MAX));
    let mut read = 0;
    while read < queue.len() {
        let at = queue[read];
        read += 1;
        for &(next, fixture) in &edges[at] {
            if previous[next].is_some() {
                continue;
            }
            previous[next] = Some((at, fixture));
            if !inside[next] {
                let mut route = Vec::new();
                let mut cursor = next;
                while cursor != start {
                    let (parent, game) = previous[cursor]?;
                    route.push(game);
                    cursor = parent;
                }
                route.reverse();
                return Some(route);
            }
            queue.push(next);
        }
    }
    None
}
fn set_value(
    s: &mut State<'_>,
    v: Interval,
    value: i64,
    key: Key,
    routes: &[Vec<usize>],
) -> Option<()> {
    let delta = value - v.base;
    if delta == 0 {
        return Some(());
    }
    let amount = i32::try_from(delta.abs()).ok()?;
    let fixture = if delta > 0 { v.up? } else { v.down? };
    if key == Key::Gd {
        if fixture < s.model.fixtures.len() {
            s.adjust(fixture, amount, 0)
        } else {
            for &game in routes.get(fixture - s.model.fixtures.len())? {
                s.adjust(game, amount, 0)?;
            }
            Some(())
        }
    } else {
        s.adjust(fixture, 0, amount)
    }
}
// Choose strict orderings where possible; fixed equal values proceed to the
// next key / sorter. A feasible interval layout is not itself a certificate.
fn layout(values: &[Interval], target: usize, x: i64, ahead: usize) -> Option<Vec<i64>> {
    let mut assigned: Vec<_> = values.iter().map(|v| v.base).collect();
    assigned[target] = x;
    let mut forced = 0;
    let mut equal = 0;
    let mut flexible = Vec::new();
    for (i, v) in values.iter().enumerate() {
        if i == target {
            continue;
        }
        match (v.hi > x, v.lo < x) {
            (true, false) => {
                assigned[i] = v.base.max(x + 1).min(v.hi);
                forced += 1;
            }
            (false, true) => assigned[i] = v.base.min(x - 1).max(v.lo),
            (true, true) => flexible.push(i),
            (false, false) => {
                assigned[i] = x;
                equal += 1;
            }
        }
    }
    if ahead < forced || ahead > forced + flexible.len() + equal {
        return None;
    }
    flexible.sort_by_key(|i| (values[*i].base <= x, *i));
    let above = (ahead - forced).min(flexible.len());
    for (k, i) in flexible.into_iter().enumerate() {
        let v = values[i];
        assigned[i] = if k < above {
            v.base.max(x + 1).min(v.hi)
        } else {
            v.base.min(x - 1).max(v.lo)
        };
    }
    Some(assigned)
}

fn stage(
    s: &State<'_>,
    outcomes: &[u8],
    cell: Cell,
    key: Key,
    wins: bool,
    gf_next: bool,
    trials: &mut usize,
    paths: bool,
) -> Option<Certificate> {
    let prefix = s.prefix(cell.team, wins, key == Key::Gf && gf_next);
    let ahead = (0..s.model.n)
        .filter(|&t| s.prefix(t, wins, key == Key::Gf && gf_next) > prefix)
        .count();
    let cohort: Vec<_> = (0..s.model.n)
        .filter(|&t| s.prefix(t, wins, key == Key::Gf && gf_next) == prefix)
        .collect();
    let requested = cell.rank.checked_sub(ahead)?;
    if requested >= cohort.len() {
        return None;
    }
    let target = cohort.iter().position(|t| *t == cell.team)?;
    let (values, routes) = intervals(s, &cohort, key, paths);
    let v = values[target];
    let mut candidates = vec![v.base];
    for peer in &values {
        candidates.extend([peer.base - 1, peer.base, peer.base + 1]);
    }
    candidates.retain(|x| *x >= v.lo && *x <= v.hi);
    candidates.sort_by_key(|x| ((*x - v.base).abs(), *x));
    candidates.dedup();
    for x in candidates {
        let Some(assigned) = layout(&values, target, x, requested) else {
            continue;
        };
        // A bounded algebraic completion, not another fixture search budget.
        if *trials >= 16 {
            return None;
        }
        *trials += 1;
        let mut trial = s.clone();
        let mut legal = true;
        for (&v, &value) in values.iter().zip(&assigned) {
            if set_value(&mut trial, v, value, key, &routes).is_none() {
                legal = false;
                break;
            }
        }
        if !legal {
            continue;
        }
        if let Some(proof) = trial.verified(outcomes, cell) {
            return Some(proof);
        }
        if key == Key::Gd && gf_next {
            if let Some(proof) = stage(&trial, outcomes, cell, Key::Gf, wins, true, trials, paths) {
                return Some(proof);
            }
        }
    }
    None
}

/// Independent outside-cohort goal adjustments, followed by actual sorting.
/// Only supported score-key prefixes are tried. None is always "unknown".
#[cold]
#[inline(never)]
pub fn complete(model: &Model, outcomes: &[u8], cell: Cell) -> Option<Certificate> {
    complete_with_paths(
        model,
        outcomes,
        cell,
        std::env::var("RUST_ODDS_GOAL_PATHS").as_deref() == Ok("1"),
    )
}
/// Extend independent GD adjustments through decisive paths, within the same
/// sixteen layout attempts. Paths are witnesses, never impossibility proofs.
pub fn complete_with_paths(
    model: &Model,
    outcomes: &[u8],
    cell: Cell,
    paths: bool,
) -> Option<Certificate> {
    if cell.team >= model.n || cell.rank >= model.n || model.request.phase.bonus_points != 0 {
        return None;
    }
    let wins = model.keys.get(1) == Some(&Key::W);
    let offset = 1 + usize::from(wins);
    if model.keys.first() != Some(&Key::Pt) {
        return None;
    }
    let key = *model.keys.get(offset)?;
    if !matches!(key, Key::Gd | Key::Gf) {
        return None;
    }
    let gf_next = key == Key::Gd && model.keys.get(offset + 1) == Some(&Key::Gf);
    let state = State::new(model, outcomes)?;
    stage(&state, outcomes, cell, key, wins, gf_next, &mut 0, paths)
}

/// Complete a W/D/L assignment with a small, native-supported set of winning
/// margins. This is a bounded reachability witness search: failure is unknown
/// and never means that the requested position is impossible.
pub(crate) fn complete_with_supported_margins(
    model: &Model,
    outcomes: &[u8],
    cell: Cell,
) -> Option<Certificate> {
    if cell.team >= model.n || cell.rank >= model.n || model.request.phase.bonus_points != 0 {
        return None;
    }
    let wins = model.keys.get(1) == Some(&Key::W);
    let offset = 1 + usize::from(wins);
    if model.keys.first() != Some(&Key::Pt) || model.keys.get(offset) != Some(&Key::Gd) {
        return None;
    }

    // Cache the exact native CDF masses once for every future fixture. The
    // independent sampler supports, rather than the conditioned outcome
    // sampler's retained support, determine whether a score can occur.
    let masses: Vec<_> = model
        .fixtures
        .iter()
        .map(|fixture| {
            Some((
                fixture.home_sampler.native_masses()?,
                fixture.away_sampler.native_masses()?,
            ))
        })
        .collect::<Option<_>>()?;
    let mut minimum_winner_scores = Vec::with_capacity(outcomes.len());
    for (i, &outcome) in outcomes.iter().enumerate() {
        let (home_masses, away_masses) = masses.get(i)?;
        let positive = |law: &[f64], score: usize| law.get(score).is_some_and(|mass| *mass > 0.);
        let winner_scores = match outcome {
            0 => {
                if !positive(home_masses, 0) {
                    return None;
                }
                away_masses
            }
            1 => {
                if !positive(home_masses, 0) || !positive(away_masses, 0) {
                    return None;
                }
                minimum_winner_scores.push(None);
                continue;
            }
            2 => {
                if !positive(away_masses, 0) {
                    return None;
                }
                home_masses
            }
            _ => return None,
        };
        let minimum = winner_scores
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(score, mass)| (*mass > 0.).then_some(score))?;
        minimum_winner_scores.push(Some(minimum));
    }

    for level in 1..=16usize {
        let mut trial = State::new(model, outcomes)?;
        for (i, &outcome) in outcomes.iter().enumerate() {
            if outcome == 1 {
                continue;
            }
            let (home_masses, away_masses) = masses.get(i)?;
            let winner_scores = if outcome == 0 {
                away_masses
            } else {
                home_masses
            };
            let minimum = minimum_winner_scores.get(i).copied().flatten()?;
            let focal_fixture = {
                let fixture = model.fixtures.get(i)?;
                fixture.home == cell.team || fixture.away == cell.team
            };
            let winner_score = if focal_fixture {
                winner_scores
                    .iter()
                    .enumerate()
                    .skip(1)
                    .filter(|(score, mass)| *score <= level && **mass > 0.)
                    .map(|(score, _)| score)
                    .last()
                    .unwrap_or(minimum)
            } else {
                minimum
            };
            let margin = i32::try_from(winner_score.checked_sub(1)?).ok()?;
            if margin > 0 {
                trial.adjust(i, margin, 0)?;
            }
        }
        if let Some(certificate) = trial.verified(outcomes, cell) {
            if has_native_score_support(model, &certificate) {
                return Some(certificate);
            }
        }
    }
    None
}

/// Reconstruct compact adjustments and verify their complete season on demand.
pub fn verify(model: &Model, proof: &Certificate, cell: Cell) -> bool {
    let Some(mut s) = State::new(model, &proof.outcomes) else {
        return false;
    };
    for a in &proof.adjustments {
        if s.adjust(a.fixture, a.margin, a.equal_goals).is_none() {
            return false;
        }
    }
    s.verified(&proof.outcomes, cell)
        .is_some_and(|actual| actual.ranks == proof.ranks)
}

/// Check that every completed fixture score is in the support of the native
/// home and away score samplers. Goal adjustments are unbounded, so the
/// ranking certificate alone does not establish that the completed season has
/// positive probability.
pub(crate) fn has_native_score_support(model: &Model, proof: &Certificate) -> bool {
    let Some(mut s) = State::new(model, &proof.outcomes) else {
        return false;
    };
    for a in &proof.adjustments {
        if s.adjust(a.fixture, a.margin, a.equal_goals).is_none() {
            return false;
        }
    }
    model.fixtures.iter().all(|fixture| {
        let score = s.scores[fixture.request_index];
        let Some(home) = fixture.home_sampler.native_masses() else {
            return false;
        };
        let Some(away) = fixture.away_sampler.native_masses() else {
            return false;
        };
        let (Ok(home_score), Ok(away_score)) =
            (usize::try_from(score[0]), usize::try_from(score[1]))
        else {
            return false;
        };
        home.get(home_score).is_some_and(|mass| *mass > 0.)
            && away.get(away_score).is_some_and(|mass| *mass > 0.)
    })
}

#[cfg(test)]
mod native_support_tests {
    use super::*;
    use crate::model::{Championship, Game, Phase, Request, Team};

    fn model() -> Model {
        let request: Request = serde_json::from_str(include_str!(
            "../tests/fixtures/group-16498-2026-10-10.json"
        ))
        .unwrap();
        Model::new(request).unwrap()
    }

    fn supported_margin_model(
        team2_points: i32,
        team3_points: i32,
        home_mean: f64,
        away_mean: f64,
    ) -> Model {
        let mut games = vec![Game {
            id: 1,
            home_id: 1,
            away_id: 2,
            home_score: 58,
            away_score: 1,
            home_power: 1.,
            away_power: 1.,
            played: true,
        }];
        for id in 0..12 {
            games.push(Game {
                id: id + 2,
                home_id: 2,
                away_id: 1,
                home_score: 0,
                away_score: 0,
                home_power: home_mean,
                away_power: away_mean,
                played: false,
            });
        }
        Model::new(Request {
            id: 1,
            zones: Vec::new(),
            phase: Phase {
                sort: "pt,gd,gf,bias,name".into(),
                championship: Championship {
                    point_win: 3,
                    point_draw: 1,
                    point_loss: 0,
                },
                bonus_points: 0,
                bonus_points_threshold: 0,
            },
            games,
            team_groups: vec![
                Team {
                    team_id: 1,
                    add_sub: 0,
                    bias: 0,
                },
                Team {
                    team_id: 2,
                    add_sub: team2_points,
                    bias: 0,
                },
                Team {
                    team_id: 3,
                    add_sub: team3_points,
                    bias: 0,
                },
            ],
        })
        .unwrap()
    }

    #[test]
    fn native_support_accepts_positive_masses_and_rejects_completed_scores_outside_them() {
        let model = model();
        let outcomes = vec![2; model.fixtures.len()];
        let supported = Certificate {
            outcomes: outcomes.clone(),
            adjustments: Vec::new(),
            ranks: Vec::new(),
        };
        assert!(has_native_score_support(&model, &supported));

        let unsupported = Certificate {
            outcomes,
            adjustments: vec![Adjustment {
                fixture: 0,
                margin: 10_000,
                equal_goals: 0,
            }],
            ranks: Vec::new(),
        };
        assert!(!has_native_score_support(&model, &unsupported));
    }

    #[test]
    fn supported_margins_distribute_goals_across_focal_losses() {
        let model = supported_margin_model(-33, 3, 1., 1.);
        let outcomes = vec![2; model.fixtures.len()];
        let cell = Cell { team: 0, rank: 2 };

        let proof = complete_with_supported_margins(&model, &outcomes, cell).unwrap();

        assert_eq!(proof.outcomes, outcomes);
        assert_eq!(proof.ranks[0], cell.rank);
        assert!(verify(&model, &proof, cell));
        assert!(has_native_score_support(&model, &proof));
        assert_eq!(proof.adjustments.len(), 12);
        assert!(proof.adjustments.iter().filter(|a| a.margin > 0).count() >= 6);
    }

    #[test]
    fn supported_margins_decline_positions_blocked_by_points() {
        let model = supported_margin_model(100, 50, 1., 1.);
        let outcomes = vec![2; model.fixtures.len()];

        assert!(
            complete_with_supported_margins(&model, &outcomes, Cell { team: 0, rank: 0 },)
                .is_none()
        );
    }

    #[test]
    fn supported_margins_decline_unsupported_native_laws() {
        let outcomes = vec![2; 12];
        let cell = Cell { team: 0, rank: 2 };
        let unsupported_mean = supported_margin_model(-33, 0, 33., 1.);
        let unsupported_winner_score = supported_margin_model(-33, 0, 0., 1.);

        assert!(complete_with_supported_margins(&unsupported_mean, &outcomes, cell).is_none());
        assert!(
            complete_with_supported_margins(&unsupported_winner_score, &outcomes, cell).is_none()
        );
    }
}
