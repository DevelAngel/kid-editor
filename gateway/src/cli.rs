pub use clap::Parser;
use clap::ValueHint;
use clap::{ArgGroup, Args};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use url::Url;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

/// Builds the log filter from `--verbose`/`--quiet`. Everything outside
/// this crate (`rmcp`, `tower`, ...) is capped at `warn` even when
/// `verbosity` asks for more, so raised verbosity surfaces this crate's
/// own tool-routing logs without drowning in dependency chatter — capped
/// at `verbosity` itself too, so `--quiet` (error level) quiets
/// dependencies down to `error` as well, not just this crate.
/// `RUST_LOG`, if set, wins outright.
pub fn env_filter(verbosity: &Verbosity<InfoLevel>) -> EnvFilter {
    if let Ok(filter) = EnvFilter::try_from_default_env() {
        return filter;
    }
    let verbosity: LevelFilter = verbosity.tracing_level_filter();
    let baseline = std::cmp::min(verbosity, LevelFilter::WARN);
    let crate_name = env!("CARGO_PKG_NAME").replace('-', "_");
    EnvFilter::new(format!("{baseline},{crate_name}={verbosity}"))
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
