pub use clap::Parser;
use clap::ValueHint;
use clap::{ArgGroup, Args};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use url::Url;

use crate::mcp::IgnorePattern;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

/// MCP server exposing text-editor tools
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[clap(group = ArgGroup::new("oauth").required(true).args(&["oauth-disabled", "oauth-clients-file"]))]
pub struct Cli {
    /// Sets the workspace which will be served
    #[clap(value_name = "DIR", value_hint = ValueHint::DirPath)]
    pub workspace_root: PathBuf,

    /// Sets the address the MCP server listens on.
    #[clap(long = "listen", default_value_t = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9300))]
    pub addr: SocketAddr,

    /// This server's own public URL, used when issuing OAuth metadata
    /// (issuer, authorization/token endpoints), e.g. "https://mcp.example.com".
    #[clap(long, value_name = "URL", value_hint = ValueHint::Url, default_value = "http://localhost:9300")]
    pub base_url: Url,

    /// Origins allowed to access the MCP server cross-origin, e.g.
    /// "https://example.ai". Can be repeated or comma-separated.
    #[clap(long = "allowed-origin", value_name = "URL", value_hint = ValueHint::Url, value_delimiter = ',')]
    #[cfg_attr(debug_assertions, clap(default_value = "http://localhost/"))]
    pub allowed_origins: Vec<Url>,

    /// Directory/file glob patterns invisible to every tool — treated as
    /// nonexistent, not just hidden from `tree`, e.g. ".git", "target", or
    /// "*.log". Matched against every path component at any depth unless
    /// prefixed with "/", which anchors the pattern to the workspace's
    /// top level only (e.g. "/README.md" hides just the top-level file,
    /// not "sub/README.md"). Can be repeated or comma-separated.
    ///
    /// `justfile` and `*.just` are handled separately, not through this
    /// list — see `--enable-just-run` and ADR 0003.
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

    /// Additional glob patterns appended to `--ignore`'s list (default or
    /// custom), instead of replacing it. Use this to add patterns without
    /// having to repeat the whole default list. Same syntax as `--ignore`.
    /// Can be repeated or comma-separated.
    #[clap(long = "extra-ignore", value_name = "PATTERN", value_hint = ValueHint::AnyPath, value_delimiter = ',')]
    pub extra_ignore: Vec<IgnorePattern>,

    /// Enable the `just_run` tool. Off by default: a `justfile` in the
    /// workspace is not enough on its own, since discovering recipes
    /// automatically would mean trusting whatever recipes happen to be
    /// there without a human ever having looked. Passing this flag is
    /// that look — it means someone reviewed the workspace's `justfile`
    /// and decided its recipes are fine to expose. Once set, recipe
    /// discovery runs and the `just_run` tool is offered if any recipes
    /// were found; `justfile`/`*.just` also become invisible and
    /// read-only through this server, everywhere in the workspace, at
    /// the same time — see ADR 0003.
    #[clap(long)]
    pub enable_just_run: bool,

    #[command(flatten)]
    pub oauth: OauthOptions,

    #[command(flatten)]
    pub verbosity: Verbosity<InfoLevel>,
}

#[derive(Debug, Args)]
pub struct OauthOptions {
    /// Disable OAuth authorization
    #[arg(long = "disable-oauth", id = "oauth-disabled")]
    pub disabled: bool,

    /// TOML file listing MCP clients allowed to authenticate against the MCP server
    /// If unset or empty, MCP OAuth is effectively disabled:
    /// no client can complete the authorization flow.
    #[arg(long, value_name = "FILE", value_hint = ValueHint::FilePath, id = "oauth-clients-file")]
    pub clients_file: Option<std::path::PathBuf>,
}
