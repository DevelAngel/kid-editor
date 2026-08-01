use crate::mcp::McpService;

use anyhow::Result;
use axum::Router;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tokio::net::TcpListener;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use type_state_builder::TypeStateBuilder;
use url::Url;

use std::iter;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(TypeStateBuilder, Debug)]
pub struct McpServer {
    #[builder(required)]
    workspace_root: PathBuf,
    #[builder(required)]
    base_url: Url,
    allowed_origins: Vec<Url>,
}

impl McpServer {
    pub async fn serve(self, addr: SocketAddr) -> Result<()> {
        tracing::warn!(
            "MCP server will serve workspace: {}",
            self.workspace_root.display(),
        );

        let all_origins: Vec<Url> = iter::once(self.base_url.clone())
            .chain(self.allowed_origins)
            .collect();
        tracing::info!(
            "Allowed origins: {}",
            all_origins
                .iter()
                .map(|url| url.to_string())
                .reduce(|s, url| format!("{s} | {url}"))
                .unwrap_or("no origins configured".to_owned())
        );

        let allowed_hosts: Vec<_> = all_origins.iter().filter_map(host_from_url).collect();
        let allowed_origins: Vec<_> = all_origins
            .into_iter()
            .map(|url| url.origin().ascii_serialization())
            .collect();

        let shutdown = CancellationToken::new();
        let workspace_root = self.workspace_root.canonicalize()?;
        let mcp_service = StreamableHttpService::new(
            move || Ok(McpService::new(workspace_root.clone())),
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig::default()
                .with_allowed_origins(allowed_origins)
                .with_allowed_hosts(allowed_hosts)
                .with_cancellation_token(shutdown.child_token()),
        );

        let mcp_router = Router::new().nest_service("/mcp", mcp_service);

        let app = Router::new()
            .merge(mcp_router)
            .layer(CorsLayer::permissive())
            .layer(TraceLayer::new_for_http());

        let listener = TcpListener::bind(addr).await?;
        tracing::info!(
            "MCP server listening on: http://{} (public: {})",
            listener.local_addr().unwrap(),
            self.base_url,
        );

        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                signal::ctrl_c().await.unwrap();
                tracing::warn!("Server shutting down");
                shutdown.cancel();
            })
            .await;
        Ok(())
    }
}

/// Derives a `Host` header value (`host` or `host:port`) from a configured
/// allowed origin, for `StreamableHttpServerConfig::with_allowed_hosts`.
fn host_from_url(url: &Url) -> Option<String> {
    let host = url.host_str()?;
    Some(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    })
}
