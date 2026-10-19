/// Coverage is the default profile, with explicit environment overrides.
pub fn profile() -> String {
    resolve_profile(std::env::var("RUST_ODDS_RARE_TAIL").ok())
}

pub fn value(name: &str) -> String {
    resolve_value(name, std::env::var(name).ok(), profile() == "coverage")
}

pub fn resolve_profile(explicit: Option<String>) -> String {
    explicit.unwrap_or_else(|| "coverage".into())
}

pub fn resolve_value(name: &str, explicit: Option<String>, coverage: bool) -> String {
    explicit.unwrap_or_else(|| {
        if coverage {
            profile_default(name).into()
        } else {
            String::new()
        }
    })
}

pub fn profile_default(name: &str) -> &'static str {
    match name {
        "RUST_ODDS_LAZY_GUIDE" => "cardinality",
        "RUST_ODDS_LAZY_GOALS"
        | "RUST_ODDS_LAZY_OMIT"
        | "RUST_ODDS_RARE_TAIL_SHARE"
        | "RUST_ODDS_RARE_TAIL_BRANCHES"
        | "RUST_ODDS_RARE_TAIL_POSITIVE_BRANCHES"
        | "RUST_ODDS_RARE_TAIL_TREE" => "1",
        "RUST_ODDS_LAZY_SUBSET" => "adaptive",
        "RUST_ODDS_RARE_TAIL_FINALISTS" => "16",
        "RUST_ODDS_LAZY_RIVALS" => "4",
        "RUST_ODDS_RARE_TAIL_QUALITY" => "order",
        "RUST_ODDS_RARE_TAIL_BUDGET_FRACTION" => "0.45",
        "RUST_ODDS_RARE_TAIL_CONFIRM_MORE" => "1.5",
        "RUST_ODDS_RARE_TAIL_EXTENSION" => "after",
        "RUST_ODDS_RARE_TAIL_UNION_PATTERNS" => "256",
        "RUST_ODDS_RARE_TAIL_RETRY" | "RUST_ODDS_RARE_TAIL_BATCH_ALLOCATION" => "0",
        "RUST_ODDS_SHARED_CONSTRAINTS" => "guided",
        "RUST_ODDS_SHARED_CONSTRAINTS_RATIO" => "0.35",
        "RUST_ODDS_SHARED_CONSTRAINTS_BLOCKERS" => "2",
        "RUST_ODDS_SHARED_CONSTRAINTS_RELATIVE" => "1",
        "RUST_ODDS_SHARED_CONSTRAINTS_CONFIRMATION" => "parallel",
        "RUST_ODDS_SHARED_CONSTRAINTS_FRACTION" => "0.5",
        _ => "",
    }
}
