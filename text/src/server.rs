use crate::mcp::{IgnorePattern, McpService, discover_recipes};

use oauth::{self, McpClientsConfig, McpOAuthStore};
use recipe::RecipeFile;

use anyhow::Result;
use axum::Router;
use axum::middleware;
use axum::routing::{get, post};
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
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(TypeStateBuilder, Debug)]
pub struct McpServer {
    #[builder(required)]
    workspace_root: PathBuf,
    #[builder(required)]
    clients: Option<McpClientsConfig>,
    #[builder(required)]
    base_url: Url,
    allowed_origins: Vec<Url>,
    ignore: Vec<IgnorePattern>,
    recipes_file: Option<PathBuf>,
}

impl McpServer {
    pub async fn serve(self, addr: SocketAddr) -> Result<()> {
        tracing::warn!("Workspace: {}", self.workspace_root.display());
        tracing::warn!(
            "Ignored patterns (invisible to all tools): {}",
            self.ignore
                .iter()
                .map(|p| p.to_string())
                .reduce(|s, name| format!("{s} | {name}"))
                .unwrap_or("none".to_owned())
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
        let ignore = self.ignore;

        let (recipes, recipe_toml_protected_path) = if let Some(recipes_file) = &self.recipes_file {
            let recipes_path = if recipes_file.is_absolute() {
                recipes_file.clone()
            } else {
                workspace_root.join(recipes_file)
            };
            if recipes_path.is_file() {
                let recipes = discover_recipes(&recipes_path);
                match recipes.iter().count() {
                    0 => tracing::warn!(
                        "recipe_run tool disabled: {} has no recipes",
                        recipes_path.display()
                    ),
                    n => {
                        tracing::warn!("recipe_run tool enabled and discovered {n} recipes");
                        recipes.iter().for_each(|(name, recipe)| {
                            tracing::info!("recipe '{name}': {}", recipe.description);
                        });
                    }
                }
                // Only a --recipes-file *inside* the workspace needs
                // hiding/write-refusal here: one outside it is already
                // unreachable through every other tool in this server,
                // regardless (see ADR 0004's "More Information"
                // amendment). Comparing canonicalized paths, not just
                // string-prefix, so a workspace_root/../sibling-style
                // --recipes-file isn't mistaken for "inside".
                let protected = recipes_path.canonicalize().ok().and_then(|canonical| {
                    canonical
                        .strip_prefix(&workspace_root)
                        .map(Path::to_path_buf)
                        .ok()
                });
                match &protected {
                    Some(relative) => tracing::warn!(
                        "{} hidden and write-protected (see ADR 0003, ADR 0004): {}",
                        recipes_path.display(),
                        relative.display()
                    ),
                    None => tracing::warn!(
                        "{} is outside the workspace; not hidden or write-protected here \
                         (already unreachable through every tool in this server)",
                        recipes_path.display()
                    ),
                }
                (recipes, protected)
            } else {
                tracing::warn!(
                    "recipe_run tool disabled: {} does not exist",
                    recipes_path.display()
                );
                (RecipeFile::default(), None)
            }
        } else {
            tracing::warn!("recipe_run tool disabled (pass --recipes-file <FILE> to enable it)");
            (RecipeFile::default(), None)
        };

        let mcp_service = StreamableHttpService::new(
            move || {
                Ok(McpService::new(
                    workspace_root.clone(),
                    ignore.clone(),
                    recipes.clone(),
                    recipe_toml_protected_path.clone(),
                ))
            },
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig::default()
                .with_allowed_origins(allowed_origins)
                .with_allowed_hosts(allowed_hosts)
                .with_cancellation_token(shutdown.child_token())
                .with_legacy_session_mode(false),
        );

        let app = if let Some(clients) = self.clients {
            tracing::info!("OAuth enabled");

            let oauth_store = Arc::new(McpOAuthStore::new(self.base_url.clone(), clients));
            tokio::spawn({
                let oauth_store = oauth_store.clone();
                let shutdown = shutdown.child_token();
                async move { oauth_store.background_cleanup(shutdown).await }
            });

            let protected_mcp_router = Router::new().nest_service("/mcp", mcp_service).layer(
                middleware::from_fn_with_state(oauth_store.clone(), oauth::validate_access_token),
            );

            let oauth_server_router = Router::new()
                .route(
                    "/.well-known/oauth-authorization-server",
                    get(oauth::auth_server).options(oauth::auth_server),
                )
                .route(
                    "/.well-known/oauth-protected-resource",
                    get(oauth::protected_resource).options(oauth::protected_resource),
                )
                .route(
                    "/.well-known/oauth-protected-resource/mcp",
                    get(oauth::protected_resource).options(oauth::protected_resource),
                )
                .route("/authorize", get(oauth::authorize))
                .route("/oauth/approve", post(oauth::approve))
                .route(
                    "/token",
                    post(oauth::gen_access_token).options(oauth::gen_access_token),
                )
                .with_state(oauth_store.clone());

            Router::new()
                .merge(protected_mcp_router)
                .merge(oauth_server_router)
                .layer(CorsLayer::permissive())
                .layer(TraceLayer::new_for_http())
        } else {
            tracing::warn!("OAuth disabled");
            let unprotected_mcp_router = Router::new().nest_service("/mcp", mcp_service);
            Router::new()
                .merge(unprotected_mcp_router)
                .layer(CorsLayer::permissive())
                .layer(TraceLayer::new_for_http())
        };

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
