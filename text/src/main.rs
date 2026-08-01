mod cli;
mod mcp;
mod server;

use crate::cli::Cli;
use crate::server::McpServer;

use anyhow::{Context, Result};
use clap::Parser;

use std::io;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_max_level(cli.verbosity)
        .with_writer(io::stderr)
        .init();

    McpServer::builder()
        .base_url(cli.base_url)
        .allowed_origins(cli.allowed_origins)
        .workspace_root(cli.workspace_root)
        .build()
        .serve(cli.addr)
        .await
        .context("failed to serve MCP service")?;
    Ok(())
}
