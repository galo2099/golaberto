//! Matched-pool evidence conditioned on a team's own point total.
use crate::{
    confidence::{binomial_kl_interval, Evidence, Interval},
    model::{Key, Model},
    pool::Scout,
};
use std::collections::BTreeMap;

fn add(a: Interval, b: Interval) -> Interval {
    a.add(b)
}
fn mul(a: Interval, b: Interval) -> Interval {
    a.mul(b)
}

/// Produce certificate evidence from native draws conditioned on predetermined
/// contiguous bins of own points. Conditional on landing in a bin, each draw has
/// the same marginal rank probability, even though exact points are unobserved.
pub fn own_points_evidence(model: &Model, scout: &Scout) -> Vec<(usize, Evidence)> {
    if model.keys.first() != Some(&Key::Pt)
        || model.request.phase.bonus_points != 0
        || model.request.phase.championship.point_win < 0
        || model.request.phase.championship.point_draw < 0
        || model.request.phase.championship.point_loss < 0
        || model
            .fixtures
            .iter()
            .any(|f| f.home >= model.n || f.away >= model.n || f.home == f.away)
    {
        return Vec::new();
    }
    let mut fixture_wdl = Vec::with_capacity(model.fixtures.len());
    for f in &model.fixtures {
        let Some(home) = f.home_sampler.certificate_masses() else {
            return Vec::new();
        };
        let Some(away) = f.away_sampler.certificate_masses() else {
            return Vec::new();
        };
        let mut wdl = [Interval::new(0., 0.); 3];
        for (h, &hp) in home.iter().enumerate() {
            for (a, &ap) in away.iter().enumerate() {
                let outcome = if h < a {
                    0
                } else if h == a {
                    1
                } else {
                    2
                };
                wdl[outcome] = add(wdl[outcome], mul(hp, ap));
            }
        }
        fixture_wdl.push(wdl);
    }
    let rules = &model.request.phase.championship;
    let mut weights = Vec::with_capacity(model.n);
    for team in 0..model.n {
        let mut pmf = BTreeMap::from([(0i32, Interval::new(1., 1.))]);
        for (i, f) in model.fixtures.iter().enumerate() {
            let (gains, probs) = if f.home == team {
                (
                    [rules.point_loss, rules.point_draw, rules.point_win],
                    fixture_wdl[i],
                )
            } else if f.away == team {
                (
                    [rules.point_win, rules.point_draw, rules.point_loss],
                    fixture_wdl[i],
                )
            } else {
                continue;
            };
            let mut next = BTreeMap::<i32, Interval>::new();
            for (&points, &weight) in &pmf {
                for o in 0..3 {
                    let Some(next_points) = points.checked_add(gains[o]) else {
                        return Vec::new();
                    };
                    let entry = next.entry(next_points).or_insert(Interval::new(0., 0.));
                    *entry = add(*entry, mul(weight, probs[o]));
                }
            }
            pmf = next;
        }
        if pmf.is_empty() {
            return Vec::new();
        }
        let total = pmf.values().copied().fold(Interval::new(0., 0.), add);
        if total.lower > 1. || total.upper < 1. {
            return Vec::new();
        }
        weights.push(pmf);
    }

    let alpha = model.confidence.matched_alpha();
    let mut result = Vec::new();
    for team in 0..model.n {
        let points: Vec<_> = weights[team].keys().copied().collect();
        let bins = points.len().min(8);
        // Round each bin's allocation down so the union bound across bins stays
        // within the request-wide matched-family allocation.
        let alpha_each = (alpha / bins.max(1) as f64).next_down();
        let mut cell_intervals = vec![Interval::new(0., 0.); model.n];
        let mut cell_samples = vec![0usize; model.n];
        for bin in 0..bins {
            let start = bin * points.len() / bins;
            let end = (bin + 1) * points.len() / bins;
            let mut mass = Interval::new(0., 0.);
            let mut row = vec![0usize; model.n];
            for &own_points in &points[start..end] {
                mass = add(mass, weights[team][&own_points]);
                for (rank, count) in scout.row(team, own_points, model.n).iter().enumerate() {
                    row[rank] += count;
                }
            }
            let n: usize = row.iter().sum();
            for rank in 0..model.n {
                let hits = row[rank];
                // Keep the bin's full possible mass if interval construction
                // cannot establish a conditional bound; never drop a disjoint
                // contribution from the cell's upper endpoint.
                let conditional = binomial_kl_interval(hits, n, alpha_each)
                    .unwrap_or_else(|_| Interval::new(0., 1.));
                let lo = mul(
                    Interval::new(mass.lower, mass.lower),
                    Interval::new(conditional.lower, conditional.lower),
                )
                .lower
                .max(0.);
                let hi = mul(
                    Interval::new(mass.upper, mass.upper),
                    Interval::new(conditional.upper, conditional.upper),
                )
                .upper
                .min(1.);
                cell_intervals[rank] = add(cell_intervals[rank], Interval::new(lo, hi));
                cell_samples[rank] = cell_samples[rank].saturating_add(n);
            }
        }
        for rank in 0..model.n {
            let index = team * model.n + rank;
            result.push((
                index,
                Evidence {
                    interval: Interval::new(
                        cell_intervals[rank].lower.max(0.),
                        cell_intervals[rank].upper.min(1.),
                    ),
                    method: "matched_own_points_strata".into(),
                    alpha,
                    samples: cell_samples[rank],
                    contribution_cap: None,
                },
            ));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finite_native_score_enumeration_is_inside_the_matched_interval() {
        let request = serde_json::from_value(json!({
            "id": 1,
            "phase": {"sort":"pt,w,bias", "championship":{"point_win":3,"point_draw":1,"point_loss":0}},
            "team_groups":[{"team_id":1},{"team_id":2}],
            "games":[{"id":1,"home_id":1,"away_id":2,"home_power":0.8,"away_power":1.1,"played":false}]
        })).unwrap();
        let model = Model::new(request).unwrap();
        let scout = crate::pool::scout(&model, 5_000, 731);
        let evidence = own_points_evidence(&model, &scout);
        let cell = evidence.iter().find(|(index, _)| *index == 0).unwrap();

        let fixture = &model.fixtures[0];
        let home = fixture.home_sampler.certificate_masses().unwrap();
        let away = fixture.away_sampler.certificate_masses().unwrap();
        let mut wdl = [Interval::new(0.0, 0.0); 3];
        for (h, &hp) in home.iter().enumerate() {
            for (a, &ap) in away.iter().enumerate() {
                let outcome = if h < a {
                    0
                } else if h == a {
                    1
                } else {
                    2
                };
                wdl[outcome] = wdl[outcome].add(hp.mul(ap));
            }
        }
        // Stable team order resolves the final point/wins/bias tie in favor
        // of team zero, so rank zero occurs for a draw or home win.
        let exact = wdl[1].add(wdl[2]);
        assert!(cell.1.interval.lower <= exact.lower);
        assert!(cell.1.interval.upper >= exact.upper);
    }
}
