use crate::{
    conditioned::{stride, Backward, Event, Result, ScoreContext},
    model::Model,
    pool::{work_per_sample, Bounds},
    rng::Rng,
};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct RankGame {
    pub index: usize,
    pub home: usize,
    pub away: usize,
    pub prob: [f64; 3],
    pub hg: [i32; 3],
    pub ag: [i32; 3],
}
pub struct Suffix {
    pub values: Vec<Arc<[f64]>>,
    pub teams: usize,
    pub span: usize,
}
impl Suffix {
    pub fn new(games: &[RankGame], teams: usize, max_cap: i32) -> Self {
        let span = (max_cap + 1).max(1) as usize;
        let terminal: Arc<[f64]> = vec![1.; span].into();
        let mut values = vec![terminal; (games.len() + 1) * teams];
        let mut tail_starts = vec![0usize; teams];
        for step in (0..games.len()).rev() {
            let split = (step + 1) * teams;
            let (first, last) = values.split_at_mut(split);
            first[step * teams..split].clone_from_slice(&last[..teams]);
            let g = &games[step];
            let old_tails = [tail_starts[g.home], tail_starts[g.away]];
            for ((team, gains), old_tail) in
                [(g.home, g.hg), (g.away, g.ag)].into_iter().zip(old_tails)
            {
                let tail_start = if gains.iter().all(|&g| g >= 0) {
                    old_tail
                        .saturating_add(*gains.iter().max().unwrap() as usize)
                        .min(span)
                } else {
                    span
                };
                let mut cdf = vec![0.; span];
                for cap in 0..tail_start {
                    for o in 0..3 {
                        let left = cap as i32 - gains[o];
                        if left >= 0 {
                            cdf[cap] += g.prob[o] * last[team][left as usize];
                        }
                    }
                }
                if tail_start < span {
                    // Every predecessor lies in the same constant CDF tail.
                    // Evaluate it in the original outcome order, including its
                    // rounding residue, rather than assuming it equals one.
                    let mut tail = 0.;
                    for o in 0..3 {
                        tail += g.prob[o] * last[team][tail_start - gains[o] as usize];
                    }
                    cdf[tail_start..].fill(tail);
                }
                tail_starts[team] = tail_start;
                first[step * teams + team] = cdf.into();
            }
        }
        Self {
            values,
            teams,
            span,
        }
    }
    #[inline]
    pub fn cdf(&self, step: usize, team: usize, cap: i32) -> f64 {
        Self::cdf_row(&self.values[step * self.teams + team], cap)
    }
    #[inline]
    pub(crate) fn cdf_row(row: &[f64], cap: i32) -> f64 {
        if cap < 0 {
            0.
        } else if cap as usize >= row.len() {
            1.
        } else {
            row[cap as usize]
        }
    }
    #[inline]
    pub fn factor(&self, step: usize, team: usize, cap: i32, below: f64, above: f64) -> f64 {
        Self::factor_row(&self.values[step * self.teams + team], cap, below, above)
    }
    #[inline]
    pub(crate) fn factor_row(row: &[f64], cap: i32, below: f64, above: f64) -> f64 {
        if cap < 0 {
            return below * 0. + 0. + above * 1.;
        }
        if cap as usize > row.len() {
            return below * 1. + 0. + above * 0.;
        }
        let index = cap as usize;
        let lower = if index == 0 { 0. } else { row[index - 1] };
        let at = if index == row.len() { 1. } else { row[index] };
        below * lower + (at - lower) + above * (1. - at)
    }
}
#[derive(Clone, Copy)]
pub struct Policy {
    pub samples: usize,
    pub seed: i64,
    pub tilt: f64,
    pub point_tilt: f64,
    pub force_points: bool,
    pub propagate: bool,
}
pub fn sample(
    model: &Model,
    event: &Event,
    target: usize,
    rank: usize,
    bounds: &Bounds,
    policy: Policy,
) -> Result {
    sample_with_probes(
        model,
        event,
        target,
        rank,
        bounds,
        policy,
        crate::domains::ProbeConfig::default(),
    )
}
pub fn sample_with_probes(
    model: &Model,
    event: &Event,
    target: usize,
    rank: usize,
    bounds: &Bounds,
    policy: Policy,
    mut probes: crate::domains::ProbeConfig,
) -> Result {
    let rules = &model.request.phase.championship;
    if (rules.point_loss, rules.point_draw, rules.point_win) != (0, 1, 3)
        || model.request.phase.bonus_points != 0
    {
        probes = crate::domains::ProbeConfig::default();
    }
    let mut result = Result {
        mass: event.mass,
        ..Default::default()
    };
    if event.mass <= 0. || policy.samples == 0 {
        return result;
    }
    let rules = &model.request.phase.championship;
    if [rules.point_loss, rules.point_draw, rules.point_win]
        .iter()
        .any(|p| *p < 0)
    {
        return result;
    }
    let stride = if policy.force_points {
        1
    } else {
        stride(model, "lookahead")
    };
    let hg = [
        rules.point_loss * stride,
        rules.point_draw * stride,
        rules.point_win * stride + if stride > 1 { 1 } else { 0 },
    ];
    let ag = [hg[2], hg[1], hg[0]];
    let mut selected = vec![false; model.fixtures.len()];
    for g in &event.games {
        selected[g.index] = true;
    }
    let mut fixed = Vec::new();
    let mut remaining = Vec::new();
    for (i, f) in model.fixtures.iter().enumerate() {
        if f.home >= model.n
            || f.away >= model.n
            || f.home == f.away
            || (!selected[i] && (f.home == target || f.away == target))
        {
            return result;
        }
        let g = RankGame {
            index: i,
            home: f.home,
            away: f.away,
            prob: f.prob,
            hg,
            ag,
        };
        if selected[i] {
            fixed.push(g);
        } else {
            remaining.push(g);
        }
    }
    let base: Vec<_> = model
        .base
        .iter()
        .map(|c| c.points * stride + if stride > 1 { c.wins } else { 0 })
        .collect();
    let max_cap = base
        .iter()
        .map(|p| bounds.max[target] * stride + stride - 1 - p)
        .max()
        .unwrap_or(0)
        .max(0);
    if max_cap > 128 * stride + stride - 1 {
        return result;
    }
    let suffix = Suffix::new(&remaining, model.ids.len(), max_cap);
    let compact = crate::search::enabled("RARE_POSITION_COMPACT_FORCED_FIXTURES");
    let compact_zero_guide = crate::search::enabled("RARE_POSITION_COMPACT_ZERO_GUIDE");
    let mode = std::env::var("RARE_POSITION_REDUCED_SIMULATION").unwrap_or_default();
    let reduce = (mode.is_empty() || mode == "1" || mode == "all")
        && compact
        && (policy.propagate || mode == "all");
    let mut domain_cache = if policy.propagate || reduce {
        crate::domains::Cache::new(
            &remaining,
            &fixed,
            target,
            rank,
            model.n,
            reduce,
            policy.propagate,
        )
    } else {
        None
    };
    domain_cache = domain_cache.map(|c| c.with_probes(stride, probes));
    let mut points = base.clone();
    let mut outcomes = vec![0; model.fixtures.len()];
    let mut score_context = ScoreContext::new(model);
    let mut rng = Rng::new(policy.seed);
    let mut backward = Backward::new(event);
    let (cdf, ratios) = event.point_tilt(policy.point_tilt);
    let mut sum = 0.;
    let mut sum2 = 0.;
    let mut max: f64 = 0.;
    let mut batches = [0.; 2];
    for draw in 0..policy.samples {
        let mut weight = 1.;
        if policy.point_tilt == 0. {
            backward.sample(&mut rng, &mut outcomes);
        } else {
            let u = rng.float();
            let terminal = cdf.partition_point(|c| *c < u).min(cdf.len() - 1);
            backward.from_terminal(&mut rng, &mut outcomes, terminal);
            weight = ratios[terminal];
        }
        points.copy_from_slice(&base);
        for g in &fixed {
            let o = outcomes[g.index] as usize;
            points[g.home] += g.hg[o];
            points[g.away] += g.ag[o];
        }
        let target_points = points[target];
        let mut expected_above = 0.;
        let mut expected_below = 0.;
        let mut above = 0;
        let (domain_key, domains) = if let Some(cache) = &mut domain_cache {
            cache.get(&points, target_points, &outcomes)
        } else {
            (0, None)
        };
        if domains.as_ref().is_some_and(|d| !d.feasible) {
            result.samples += 1;
            continue;
        }
        for t in 0..model.n {
            if t == target {
                continue;
            }
            let cap = target_points - points[t];
            expected_above += 1. - suffix.cdf(0, t, cap);
            expected_below += suffix.cdf(0, t, cap - 1);
            above += usize::from(points[t] > target_points);
        }
        let mut above_weight = if expected_above > 0. {
            (rank as f64 / expected_above).min(1.)
        } else {
            1.
        };
        let mut below_weight = if expected_below > 0. {
            ((model.n - 1 - rank) as f64 / expected_below).min(1.)
        } else {
            1.
        };
        if policy.tilt != 1. {
            above_weight = above_weight.powf(policy.tilt);
            below_weight = below_weight.powf(policy.tilt);
        }
        if above > rank {
            weight = 0.;
        }
        let mut compact_draw = compact
            && domains
                .as_ref()
                .is_some_and(|d| !d.forced.is_empty() && weight * d.mass > 0.);
        if compact_draw && !(above_weight > 0. && below_weight > 0.) {
            compact_draw = compact_zero_guide
                && if let Some(cache) = &mut domain_cache {
                    *cache.zero_safe.entry(domain_key).or_insert_with(|| {
                        domains.as_ref().unwrap().zero_guide_safe(
                            &remaining,
                            &points,
                            target_points,
                            &suffix,
                            below_weight,
                            above_weight,
                        )
                    })
                } else {
                    false
                };
        }
        if weight > 0. {
            if compact_draw {
                let d = domains.as_ref().unwrap();
                result.omitted_draws += d.omitted_count as u64;
                points.copy_from_slice(&d.base);
                weight *= d.mass;
                for &(i, o) in &d.forced {
                    outcomes[i] = o;
                }
                let mut completed = true;
                for entry in &d.variable {
                    for _ in 0..entry.skipped {
                        rng.float();
                    }
                    let g = &remaining[entry.step];
                    let hp = points[g.home] - entry.hf;
                    let ap = points[g.away] - entry.af;
                    let hrow = &suffix.values[(entry.step + 1) * suffix.teams + g.home];
                    let arow = &suffix.values[(entry.step + 1) * suffix.teams + g.away];
                    let allowed = d.prefix_mask(entry.step, g, hp, ap);
                    let mut scores = [0.; 3];
                    let mut total = 0.;
                    for o in 0..3 {
                        if allowed & (1 << o) == 0 {
                            continue;
                        }
                        let h = Suffix::factor_row(
                            hrow,
                            target_points - hp - g.hg[o],
                            below_weight,
                            above_weight,
                        );
                        let a = Suffix::factor_row(
                            arow,
                            target_points - ap - g.ag[o],
                            below_weight,
                            above_weight,
                        );
                        scores[o] = g.prob[o] * h * a;
                        total += scores[o];
                    }
                    if total <= 0. {
                        weight = 0.;
                        completed = false;
                        break;
                    }
                    let u = rng.float() * total;
                    let o = if u < scores[0] {
                        0
                    } else if u < scores[0] + scores[1] {
                        1
                    } else {
                        2
                    };
                    let q = scores[o] / total;
                    if q <= 0. {
                        weight = 0.;
                        completed = false;
                        break;
                    }
                    weight *= g.prob[o] / q;
                    outcomes[g.index] = o as u8;
                    points[g.home] += g.hg[o];
                    points[g.away] += g.ag[o];
                }
                if completed {
                    for _ in 0..d.after {
                        rng.float();
                    }
                }
            } else {
                for (step, g) in remaining.iter().enumerate() {
                    let hp = points[g.home];
                    let ap = points[g.away];
                    let hrow = &suffix.values[(step + 1) * suffix.teams + g.home];
                    let arow = &suffix.values[(step + 1) * suffix.teams + g.away];
                    let allowed = domains
                        .as_ref()
                        .filter(|d| d.restricted)
                        .map_or(7, |d| d.prefix_mask(step, g, hp, ap));
                    let mut scores = [0.; 3];
                    let mut total = 0.;
                    for o in 0..3 {
                        if allowed & (1 << o) == 0 {
                            continue;
                        }
                        let home = Suffix::factor_row(
                            hrow,
                            target_points - hp - g.hg[o],
                            below_weight,
                            above_weight,
                        );
                        let away = Suffix::factor_row(
                            arow,
                            target_points - ap - g.ag[o],
                            below_weight,
                            above_weight,
                        );
                        scores[o] = g.prob[o] * home * away;
                        total += scores[o];
                    }
                    if total <= 0. {
                        weight = 0.;
                        break;
                    }
                    let u = rng.float() * total;
                    let o = if u < scores[0] {
                        0
                    } else if u < scores[0] + scores[1] {
                        1
                    } else {
                        2
                    };
                    let q = scores[o] / total;
                    if q <= 0. {
                        weight = 0.;
                        break;
                    }
                    weight *= g.prob[o] / q;
                    outcomes[g.index] = o as u8;
                    points[g.home] += g.hg[o];
                    points[g.away] += g.ag[o];
                }
            }
        }
        result.samples += 1;
        if weight <= 0. {
            continue;
        }
        let mut above = 0;
        let mut below = 0;
        for t in 0..model.n {
            if t == target {
                continue;
            }
            above += usize::from(points[t] > target_points);
            below += usize::from(points[t] < target_points);
        }
        if above > rank || below > model.n - 1 - rank {
            continue;
        }
        let mut hit = above == rank && above + below == model.n - 1;
        if !hit {
            hit = score_context.rank(
                target,
                &outcomes,
                &mut rng,
                domains
                    .as_ref()
                    .filter(|d| !d.omitted_scores.is_empty())
                    .map(|d| d.omitted_scores.as_slice()),
            ) == rank;
        }
        if hit {
            result.hits += 1;
            if result.witness.is_none() {
                result.witness = Some(outcomes.clone());
            }
            sum += weight;
            sum2 += weight * weight;
            max = max.max(weight);
            batches[usize::from(draw >= policy.samples / 2)] += weight;
        }
    }
    if probes.checks > 0 && std::env::var("RUST_ODDS_LOG").as_deref() != Ok("0") {
        if let Some(c) = &domain_cache {
            eprintln!(
                "{}",
                serde_json::json!({
                    "event":"rust_odds_domain_probes", "team":model.ids[target], "rank":rank+1,
                    "checks":c.stats.checks,"nodes":c.stats.nodes,"removed":c.stats.removed,
                    "forced":c.stats.forced,"infeasible":c.stats.infeasible,"elapsed_ms":c.stats.elapsed_ms
                })
            );
        }
    }
    result.work = result.samples as u64 * work_per_sample(model);
    result.summarize(sum, sum2, max, batches);
    result
}

#[cfg(test)]
mod suffix_tail_tests {
    use super::*;
    #[test]
    fn constant_tails_match_the_full_cdf_in_every_bit() {
        for stride in [1, 13, 31] {
            let hg = [0, stride, 3 * stride + i32::from(stride > 1)];
            let games = (0..18)
                .map(|i| RankGame {
                    index: i,
                    home: i % 4,
                    away: (i + 1) % 4,
                    prob: if i % 3 == 0 {
                        [0.9999999999999999, 0., 1e-20]
                    } else {
                        [0.31, 0.27, 0.42]
                    },
                    hg,
                    ag: [hg[2], hg[1], hg[0]],
                })
                .collect::<Vec<_>>();
            let suffix = Suffix::new(&games, 4, 1000);
            let mut original = vec![vec![1.; suffix.span]; (games.len() + 1) * 4];
            for step in (0..games.len()).rev() {
                for t in 0..4 {
                    original[step * 4 + t] = original[(step + 1) * 4 + t].clone();
                }
                let g = &games[step];
                for (t, gains) in [(g.home, g.hg), (g.away, g.ag)] {
                    let mut row = vec![0.; suffix.span];
                    for cap in 0..suffix.span {
                        for o in 0..3 {
                            let left = cap as i32 - gains[o];
                            if left >= 0 {
                                row[cap] += g.prob[o] * original[(step + 1) * 4 + t][left as usize];
                            }
                        }
                    }
                    original[step * 4 + t] = row;
                }
            }
            for (new, old) in suffix.values.iter().zip(original) {
                assert_eq!(
                    new.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    old.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                );
            }
        }
    }
}
