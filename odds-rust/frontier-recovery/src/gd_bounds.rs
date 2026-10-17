use crate::law::{Interval, Law};
use golaberto_odds::model::Model;
use std::{collections::BTreeMap, time::Instant};

const MAX_SUPPORT: usize = 2_000;
const MAX_WORK: usize = 2_000_000;

#[derive(Debug, Default)]
pub struct GdCache {
    max_entries: usize,
    entries: BTreeMap<(usize, usize, Vec<(usize, u8)>), Interval>,
}

impl GdCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            entries: BTreeMap::new(),
        }
    }

    /// Enclose P(GD(positive) >= GD(negative)) given fixed WDL outcomes on
    /// every fixture incident to either team. Shared fixtures are included
    /// once, with their signed goal difference doubled when both endpoints
    /// are the queried teams.
    pub fn probability(
        &mut self,
        model: &Model,
        laws: &[Law],
        masks: &[u8],
        positive: usize,
        negative: usize,
        deadline: Instant,
    ) -> Result<Option<Interval>, String> {
        if laws.len() != model.fixtures.len() || masks.len() != laws.len() {
            return Err("GD bound fixture length mismatch".into());
        }
        if positive >= model.n || negative >= model.n || positive == negative {
            return Err("GD bound team index out of range".into());
        }
        let mut key = Vec::new();
        for (i, f) in model.fixtures.iter().enumerate() {
            if f.home != positive && f.away != positive && f.home != negative && f.away != negative
            {
                continue;
            }
            let mask = masks[i];
            if mask.count_ones() != 1 {
                return Ok(None);
            }
            key.push((i, mask.trailing_zeros() as u8));
        }
        let cache_key = (positive, negative, key.clone());
        if let Some(value) = self.entries.get(&cache_key) {
            return Ok(Some(*value));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }

        let initial = model.base[positive].gf
            - model.base[positive].ga
            - (model.base[negative].gf - model.base[negative].ga);
        let mut distribution = BTreeMap::from([(initial, Interval::point(1.0))]);
        let mut work = 0usize;
        for (fixture_index, outcome) in key {
            if Instant::now() >= deadline {
                return Ok(None);
            }
            let law = &laws[fixture_index];
            let denominator = law.outcomes[outcome as usize];
            if denominator.lo <= 0.0 {
                return Ok(None);
            }
            let fixture = &model.fixtures[fixture_index];
            let mut deltas = BTreeMap::<i32, Interval>::new();
            for pair in &law.pairs[outcome as usize] {
                work = work.saturating_add(1);
                if work > MAX_WORK || (work & 255 == 0 && Instant::now() >= deadline) {
                    return Ok(None);
                }
                let gd = pair.home - pair.away;
                let mut delta = 0;
                if fixture.home == positive {
                    delta += gd;
                }
                if fixture.away == positive {
                    delta -= gd;
                }
                if fixture.home == negative {
                    delta -= gd;
                }
                if fixture.away == negative {
                    delta += gd;
                }
                let mass = pair.mass.div(denominator);
                add_mass(&mut deltas, delta, mass);
            }
            if deltas.is_empty() {
                return Ok(None);
            }
            let mut next = BTreeMap::new();
            for (&old_delta, &old_mass) in &distribution {
                for (&new_delta, &new_mass) in &deltas {
                    work = work.saturating_add(1);
                    if work > MAX_WORK || (work & 255 == 0 && Instant::now() >= deadline) {
                        return Ok(None);
                    }
                    add_mass(&mut next, old_delta + new_delta, old_mass.mul(new_mass));
                }
            }
            if next.len() > MAX_SUPPORT {
                return Ok(None);
            }
            distribution = next;
        }
        let mut result = Interval::point(0.0);
        for (&difference, &mass) in &distribution {
            if difference >= 0 {
                result = result.add(mass);
            }
        }
        let result = result.clamp_probability();
        if self.entries.len() < self.max_entries {
            self.entries.insert(cache_key, result);
        }
        Ok(Some(result))
    }
}

fn add_mass(dst: &mut BTreeMap<i32, Interval>, key: i32, mass: Interval) {
    let previous = dst
        .get(&key)
        .copied()
        .unwrap_or_else(|| Interval::point(0.0));
    dst.insert(key, previous.add(mass));
}

#[cfg(test)]
mod tests {
    use super::*;
    use golaberto_odds::model::{Model, Request};

    fn fixture_model() -> Model {
        let request: Request = serde_json::from_str(
            r#"{"id":1,"phase":{"sort":"pt,w,gd,gf,name","championship":{"point_win":3,"point_draw":1,"point_loss":0}},"games":[{"id":1,"home_id":2,"away_id":3,"home_score":2,"away_score":0,"played":true},{"id":2,"home_id":1,"away_id":2,"home_power":1.0,"away_power":1.0,"played":false},{"id":3,"home_id":1,"away_id":4,"home_power":1.0,"away_power":1.0,"played":false}],"team_groups":[{"team_id":1},{"team_id":2},{"team_id":3},{"team_id":4}]}"#,
        ).unwrap();
        Model::new(request).unwrap()
    }

    #[test]
    fn shared_and_private_fixture_enclosure_matches_exhaustive_scores() {
        let model = fixture_model();
        let laws = vec![Law::new(1.0, 1.0).unwrap(), Law::new(1.0, 1.0).unwrap()];
        let masks = [1 << 0, 1 << 2]; // shared negative win; positive private win
        let mut cache = GdCache::new(8);
        let q = cache
            .probability(
                &model,
                &laws,
                &masks,
                0,
                1,
                Instant::now() + std::time::Duration::from_secs(1),
            )
            .unwrap()
            .unwrap();

        let shared = &laws[0];
        let private = &laws[1];
        let shared_d = shared.outcomes[0];
        let private_d = private.outcomes[2];
        let mut exact = Interval::point(0.0);
        for shared_pair in &shared.pairs[0] {
            for private_pair in &private.pairs[2] {
                let diff = -2 + 2 * (shared_pair.home - shared_pair.away) + private_pair.home
                    - private_pair.away;
                if diff >= 0 {
                    exact = exact.add(
                        shared_pair
                            .mass
                            .div(shared_d)
                            .mul(private_pair.mass.div(private_d)),
                    );
                }
            }
        }
        assert!(
            q.lo <= exact.mid() && q.hi >= exact.mid(),
            "q={q:?}, exact={exact:?}"
        );
        let cached = cache
            .probability(&model, &laws, &masks, 0, 1, Instant::now())
            .unwrap()
            .unwrap();
        assert_eq!(cached, q);
        let missing = cache
            .probability(
                &model,
                &laws,
                &[u8::MAX, masks[1]],
                0,
                1,
                Instant::now() + std::time::Duration::from_secs(1),
            )
            .unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn no_partial_cache_write_on_deadline_and_uncached_capacity() {
        let model = fixture_model();
        let laws = vec![Law::new(1.0, 1.0).unwrap(), Law::new(1.0, 1.0).unwrap()];
        let masks = [1 << 0, 1 << 2];
        let mut none = GdCache::new(0);
        let q = none
            .probability(
                &model,
                &laws,
                &masks,
                0,
                1,
                Instant::now() + std::time::Duration::from_secs(1),
            )
            .unwrap()
            .unwrap();
        let mut one = GdCache::new(1);
        assert!(one
            .probability(&model, &laws, &masks, 0, 1, Instant::now())
            .unwrap()
            .is_none());
        assert_eq!(
            one.probability(
                &model,
                &laws,
                &masks,
                0,
                1,
                Instant::now() + std::time::Duration::from_secs(1)
            )
            .unwrap()
            .unwrap(),
            q
        );
    }
}
