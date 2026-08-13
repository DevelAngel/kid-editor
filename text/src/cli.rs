pub use clap::Parser;
use clap::ValueHint;
use clap::{ArgGroup, Args};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use url::Url;

use crate::mcp::IgnorePattern;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

/// Builds the log filter from `--verbose`/`--quiet`. Everything outside
/// this crate (`rmcp`, `tower`, ...) is capped at `warn` even when
/// `verbosity` asks for more, so raised verbosity surfaces this crate's
/// own tool/recipe calls without drowning in dependency chatter — capped
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
    let crate_directive = format!("{crate_name}={verbosity}")
        .parse()
        .expect("crate name and level filter always form a valid directive");
    EnvFilter::default()
        .add_directive(baseline.into())
        .add_directive(crate_directive)
}

/// MCP server exposing text-editor tools
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[clap(group = ArgGroup::new("oauth").required(true).args(&["oauth-disabled", "oauth-clients-file"]))]
pub struct Cli {
    /// Directory to serve
    #[clap(value_name = "DIR", value_hint = ValueHint::DirPath)]
    pub workspace_root: PathBuf,

    /// Address to listen on
    #[clap(long = "listen", default_value_t = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9300))]
    pub addr: SocketAddr,

    /// This server's own public URL (used for OAuth)
    #[clap(long, value_name = "URL", value_hint = ValueHint::Url, default_value = "http://localhost:9300")]
    pub base_url: Url,

    /// Extra origins allowed to connect from a browser
    #[clap(long = "allowed-origin", value_name = "URL", value_hint = ValueHint::Url, value_delimiter = ',')]
    #[cfg_attr(debug_assertions, clap(default_value = "http://localhost/"))]
    pub allowed_origins: Vec<Url>,

    /// Files and folders to hide from every tool, e.g. ".git", "target"
    // The file passed via `--recipes-file` is hidden separately, not
    // through this list — see `--recipes-file` below (ADR 0004).
    #[clap(
        long,
        value_name = "PATTERN",
        value_delimiter = ',',
        value_hint = ValueHint::AnyPath,
        default_value = ".git,.hg,.svn,.jj,target,node_modules,.venv,venv,\
                          __pycache__,.mypy_cache,.pytest_cache,.ruff_cache,\
                          dist,build,.next,.idea,.vscode,.DS_Store"
    )]
    pub ignore: Vec<IgnorePattern>,

    /// Extra patterns to add to --ignore's list, instead of replacing it
    #[clap(long = "extra-ignore", value_name = "PATTERN", value_hint = ValueHint::AnyPath, value_delimiter = ',')]
    pub extra_ignore: Vec<IgnorePattern>,

    /// Let the AI run commands defined in this TOML recipe file
    // Same reasoning as --enable-just-run, for a simpler, `just`-free
    // command format (see ADR 0004). Needs both: this flag set, and the
    // file present at startup. That file also becomes hidden and
    // read-only through this server from then on. Independent of
    // --enable-just-run — either, neither, or both can be active.
    #[clap(long, value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub recipes_file: Option<PathBuf>,

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
    pub clients_file: Option<std::path::PathBuf>,
}
