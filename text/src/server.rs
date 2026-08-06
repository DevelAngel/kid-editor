use crate::mcp::{IgnorePattern, McpService, RecipeName};
use crate::oauth::{self, McpClientsConfig, McpOAuthStore};

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

use std::collections::BTreeMap;
use std::fs;
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
    enable_just_run: bool,
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

        let just_recipes = if self.enable_just_run {
            let just_recipes = RecipeName::discover(&workspace_root);
            let justfiles = find_justfiles(&workspace_root, &ignore);
            match just_recipes.len() {
                0 => tracing::warn!("just_run tool disabled: no justfile or no recipes found"),
                n => {
                    tracing::warn!("just_run tool enabled and discovered {n} recipes");
                    just_recipes.iter().for_each(|(name, info)| {
                        tracing::info!("just recipe '{name}': {}", info.desc());
                    });
                }
            }
            // Listed regardless of whether any recipes were found: an
            // empty justfile, or a `*.just` nobody imports, still becomes
            // invisible and read-only the moment --enable-just-run is
            // set (see ADR 0003) — worth surfacing even then, so the
            // person who set the flag can check it against what they
            // actually reviewed.
            tracing::warn!(
                "justfile/*.just hidden and write-protected (see ADR 0003): {}",
                justfiles
                    .iter()
                    .map(|p| p.display().to_string())
                    .reduce(|s, p| format!("{s} | {p}"))
                    .unwrap_or("none found".to_owned())
            );
            just_recipes
        } else {
            tracing::warn!(
                "just_run tool disabled (pass --enable-just-run to enable it); \
                 justfile/*.just are ordinary, writable files through this server"
            );
            BTreeMap::new()
        };

        let mcp_service = StreamableHttpService::new(
            move || {
                Ok(McpService::new(
                    workspace_root.clone(),
                    ignore.clone(),
                    just_recipes.clone(),
                ))
            },
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig::default()
                .with_allowed_origins(allowed_origins)
                .with_allowed_hosts(allowed_hosts)
                .with_cancellation_token(shutdown.child_token()),
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

/// Walks `root` breadth-first, collecting every `justfile`/`*.just` found
/// — purely for the startup log in [`McpServer::serve`], so the person
/// who set `--enable-just-run` can cross-check what's about to become
/// hidden and read-only against what they actually reviewed, rather than
/// trusting that the two match.
///
/// Directories matching `ignore` (the plain `--ignore`/`--extra-ignore`
/// list, *not* yet including the justfile patterns — those are added by
/// [`McpService::new`], after this runs) are skipped, mainly so this
/// doesn't wander into `node_modules` or `target` for no reason. This
/// walk has no bearing on which files actually end up protected — that's
/// decided per-request by `WorkspacePath`/`IgnorePattern`, independent of
/// this function entirely — so a directory this skips is a directory
/// this log stays quiet about, not a directory the server stops
/// protecting.
fn find_justfiles(root: &Path, ignore: &[IgnorePattern]) -> Vec<PathBuf> {
    let justfile_patterns = IgnorePattern::justfile_patterns();
    let mut found = Vec::new();
    let mut queue = std::collections::VecDeque::from([(root.to_path_buf(), 0usize)]);
    while let Some((dir, depth)) = queue.pop_front() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if ignore
                .iter()
                .any(|pattern| pattern.matches_name_at_depth(&name, depth))
            {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                queue.push_back((path, depth + 1));
            } else if justfile_patterns
                .iter()
                .any(|pattern| pattern.matches_name_at_depth(&name, depth))
            {
                found.push(
                    path.strip_prefix(root)
                        .map(Path::to_path_buf)
                        .unwrap_or(path),
                );
            }
        }
    }
    found.sort();
    found
}
