pub use clap::Parser;
use clap::ValueHint::{DirPath, FilePath};
use clap::{ArgGroup, Args};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use url::Url;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

/// MCP server exposing text-editor tools
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[clap(group = ArgGroup::new("oauth").required(true).args(&["oauth-disabled", "oauth-clients-file"]))]
pub struct Cli {
    /// Sets the workspace which will be served
    #[clap(value_name = "DIR", value_hint = DirPath)]
    pub workspace_root: PathBuf,

    /// Sets the address the MCP server listens on.
    #[clap(long = "listen", default_value_t = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9300))]
    pub addr: SocketAddr,

    /// This server's own public URL, used when issuing OAuth metadata
    /// (issuer, authorization/token endpoints), e.g. "https://mcp.example.com".
    #[clap(long, value_name = "URL", default_value = "http://localhost:9300")]
    pub base_url: Url,

    /// Origins allowed to access the MCP server cross-origin, e.g.
    /// "https://example.ai". Can be repeated or comma-separated.
    #[clap(long = "allowed-origin", value_name = "URL", value_delimiter = ',')]
    #[cfg_attr(debug_assertions, clap(default_value = "http://localhost/"))]
    pub allowed_origins: Vec<Url>,

    /// Directory/file names invisible to every tool — treated as nonexistent,
    /// not just hidden from `tree`, e.g. ".git" or "target".
    /// Can be repeated or comma-separated.
    #[clap(
        long = "ignore",
        value_name = "NAME",
        value_delimiter = ',',
        default_value = ".git,.hg,.svn,.jj,target,node_modules,.venv,venv,\
                          __pycache__,.mypy_cache,.pytest_cache,.ruff_cache,\
                          dist,build,.next,.idea,.vscode,.DS_Store"
    )]
    pub ignore: Vec<String>,

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
    #[arg(long, value_name = "FILE", value_hint = FilePath, id = "oauth-clients-file")]
    pub clients_file: Option<std::path::PathBuf>,
}
