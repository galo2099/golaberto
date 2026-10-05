//! Production policy for the bounded rare-tail family fallback.

/// Resolve an explicit setting and the coverage profile default without
/// reading or changing process environment state.
fn resolve(raw: Option<&str>, coverage: bool) -> bool {
    match raw {
        Some("1") => true,
        Some("0") | Some(_) => false,
        None => coverage,
    }
}

fn resolve_with_legacy(production: Option<&str>, legacy: Option<&str>, coverage: bool) -> bool {
    resolve(production.or(legacy), coverage)
}

pub fn enabled() -> bool {
    let production = std::env::var("RUST_ODDS_FAMILY_FALLBACK").ok();
    let legacy = std::env::var("RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK").ok();
    resolve_with_legacy(
        production.as_deref(),
        legacy.as_deref(),
        super::profile() == "coverage",
    )
}

pub fn settings() -> serde_json::Value {
    serde_json::json!({
        "enabled": enabled(),
        "mode": "family",
        "scope": "all",
        "multiplier": 1,
        "training_cap": 3000,
        "native_prefix": 3000,
        "native_positive_observation_cap": 256,
        "native_key_cap": 128,
        "funding": "stage",
        "late": true,
        "max_workers": 4
    })
}

/// Default-off proposal structure experiments. Invalid values preserve the
/// historical full-status family fitting path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FamilyStructure {
    Full,
    Ties,
    Skeleton,
    Relax,
}

pub(crate) fn structure_from(raw: Option<&str>) -> FamilyStructure {
    match raw {
        Some("ties") => FamilyStructure::Ties,
        Some("skeleton") => FamilyStructure::Skeleton,
        Some("relax") => FamilyStructure::Relax,
        _ => FamilyStructure::Full,
    }
}

pub(crate) fn structure() -> FamilyStructure {
    structure_from(
        std::env::var("RUST_ODDS_EXPERIMENT_FAMILY_STRUCTURE")
            .ok()
            .as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::resolve;
    use super::resolve_with_legacy;
    use super::structure_from;
    use super::FamilyStructure;

    #[test]
    fn setting_resolution_respects_explicit_values_and_profile() {
        assert!(!resolve(None, false));
        assert!(resolve(None, true));
        assert!(!resolve(Some("0"), true));
        assert!(resolve(Some("1"), false));
        assert!(!resolve(Some("invalid"), true));
        assert!(!resolve(Some(""), false));
    }

    #[test]
    fn production_setting_takes_precedence_over_legacy_setting() {
        assert!(!resolve_with_legacy(Some("0"), Some("1"), true));
        assert!(!resolve_with_legacy(Some("bad"), Some("1"), true));
        assert!(resolve_with_legacy(None, Some("1"), false));
        assert!(!resolve_with_legacy(None, Some("bad"), true));
    }

    #[test]
    fn partial_family_structure_is_explicit_and_defaults_to_full() {
        assert_eq!(structure_from(None), FamilyStructure::Full);
        assert_eq!(structure_from(Some("skeleton")), FamilyStructure::Skeleton);
        assert_eq!(structure_from(Some("ties")), FamilyStructure::Ties);
        assert_eq!(structure_from(Some("relax")), FamilyStructure::Relax);
        assert_eq!(structure_from(Some("invalid")), FamilyStructure::Full);
    }
}
