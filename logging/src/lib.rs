use clap_verbosity_flag::{InfoLevel, Verbosity};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

/// This repository's crate names, in `tracing` target form (underscores,
/// not hyphens). Kept as one list so `--verbose`/`--quiet` raises or
/// lowers logging for the whole repository, not just the crate behind
/// the binary currently running — e.g. running `kid-text-editor
/// --verbose` also surfaces `kid-recipe`'s debug logs.
const REPO_CRATE_NAMES: [&str; 4] = [
    "kid_text_editor",
    "kid_mcp_gateway",
    "kid_recipe",
    "kid_oauth",
];

/// Builds the log filter from `--verbose`/`--quiet` and `--log-baseline`.
/// Everything outside this repository (`rmcp`, `tower`, ...) sits at
/// `log_baseline` (default `warn`), capped at `verbosity` itself so
/// `--quiet` (error level) quiets dependencies down to `error` too, not
/// just this repository's crates. `RUST_LOG`, if set, wins outright.
pub fn env_filter(verbosity: &Verbosity<InfoLevel>, log_baseline: LevelFilter) -> EnvFilter {
    if let Ok(filter) = EnvFilter::try_from_default_env() {
        return filter;
    }
    let verbosity: LevelFilter = verbosity.tracing_level_filter();
    let baseline = std::cmp::min(verbosity, log_baseline);
    REPO_CRATE_NAMES.iter().fold(
        EnvFilter::default().add_directive(baseline.into()),
        |filter, crate_name| {
            let directive = format!("{crate_name}={verbosity}")
                .parse()
                .expect("crate name and level filter always form a valid directive");
            filter.add_directive(directive)
        },
    )
}
