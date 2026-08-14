mod cli;
mod mcp;
mod server;

use crate::cli::Cli;
use crate::server::McpServer;

use oauth::McpClientsConfig;

use anyhow::{Context, Result};
use clap::Parser;

use std::io;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(logging::env_filter(&cli.verbosity, cli.log_baseline))
        .with_writer(io::stderr)
        .init();

    let clients = if cli.oauth.disabled {
        None
    } else {
        let Some(clients_file) = cli.oauth.clients_file else {
            unreachable!("either oauth is disabled or clients file is set")
        };
        let clients = McpClientsConfig::load(&clients_file)
            .context("failed to load OAuth clients configuration")?;
        Some(clients)
    };

    McpServer::builder()
        .base_url(cli.base_url)
        .allowed_origins(cli.allowed_origins)
        .workspace_root(cli.workspace_root)
        .ignore(cli.ignore.into_iter().chain(cli.extra_ignore).collect())
        .recipes_file(cli.recipes_file)
        .clients(clients)
        .build()
        .serve(cli.addr)
        .await
        .context("failed to serve MCP service")?;
    Ok(())
}
