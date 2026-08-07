mod cli;
mod mcp;
mod oauth;
mod server;

use crate::server::McpServer;
use crate::{cli::Cli, oauth::McpClientsConfig};

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
        .enable_just_run(cli.enable_just_run)
        .recipes_file(cli.recipes_file)
        .clients(clients)
        .build()
        .serve(cli.addr)
        .await
        .context("failed to serve MCP service")?;
    Ok(())
}
