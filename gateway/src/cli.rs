pub use clap::Parser;
use clap::ValueHint;
use clap::{ArgGroup, Args};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use url::Url;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

/// This repository's crate names, in `tracing` target form (underscores,
/// not hyphens). Kept as one list so `--verbose`/`--quiet` raises or
/// lowers logging for the whole repository, not just the crate behind
/// the binary currently running — e.g. running `kid-mcp-gateway
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

/// MCP gateway aggregating tools from multiple upstream MCP servers
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[clap(group = ArgGroup::new("oauth").required(true).args(&["oauth-disabled", "oauth-clients-file"]))]
pub struct Cli {
    /// TOML file listing the upstream MCP servers to aggregate
    #[clap(value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub upstreams_file: PathBuf,

    /// Address to listen on
    #[clap(long = "listen", default_value_t = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9310))]
    pub addr: SocketAddr,

    /// This server's own public URL (used for OAuth)
    #[clap(long, value_name = "URL", value_hint = ValueHint::Url, default_value = "http://localhost:9310")]
    pub base_url: Url,

    /// Extra origins allowed to connect from a browser
    #[clap(long = "allowed-origin", value_name = "URL", value_hint = ValueHint::Url, value_delimiter = ',')]
    #[cfg_attr(debug_assertions, clap(default_value = "http://localhost/"))]
    pub allowed_origins: Vec<Url>,

    #[command(flatten)]
    pub oauth: OauthOptions,

    #[command(flatten)]
    pub verbosity: Verbosity<InfoLevel>,

    /// Log level for dependencies outside this repository's own crates
    #[clap(long, default_value = "warn")]
    pub log_baseline: LevelFilter,
}

#[derive(Debug, Args)]
pub struct OauthOptions {
    /// Skip login — anyone who can reach this server can use it
    #[arg(long = "disable-oauth", id = "oauth-disabled")]
    pub disabled: bool,

    /// File listing which apps are allowed to log in
    // If unset or empty, login is effectively disabled: no client can
    // complete the authorization flow.
    #[arg(long, value_name = "FILE", value_hint = ValueHint::FilePath, id = "oauth-clients-file")]
    pub clients_file: Option<PathBuf>,
}
