mod cli;
mod config;
mod gateway_service;
mod server;

use crate::cli::Cli;
use crate::config::UpstreamsConfig;
use crate::server::GatewayServer;

use oauth::McpClientsConfig;

use anyhow::{Context, Result};
use clap::Parser;

use std::io;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(cli::env_filter(&cli.verbosity))
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

    let upstreams = UpstreamsConfig::load(&cli.upstreams_file)
        .context("failed to load upstreams configuration")?;

    GatewayServer::builder()
        .base_url(cli.base_url)
        .allowed_origins(cli.allowed_origins)
        .upstreams(upstreams)
        .clients(clients)
        .build()
        .serve(cli.addr)
        .await
        .context("failed to serve MCP gateway")?;
    Ok(())
}
