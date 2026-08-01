pub use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use url::Url;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

#[cfg(debug_assertions)]
/// Origin of the MCP Inspector
const MCP_INSPECTOR_ORIGIN: &str = "http://localhost:6247/";

/// MCP server exposing text-editor tools
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Sets the workspace which will be served
    #[clap(value_name = "PATH")]
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
    #[cfg_attr(debug_assertions, clap(default_value = MCP_INSPECTOR_ORIGIN))]
    pub allowed_origins: Vec<Url>,

    #[command(flatten)]
    pub verbosity: Verbosity<InfoLevel>,
}
