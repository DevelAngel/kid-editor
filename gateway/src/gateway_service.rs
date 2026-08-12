use crate::config::UpstreamConfig;

use anyhow::{Context, Result};
use rmcp::ServiceExt;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ErrorData as McpError, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RunningService};
use rmcp::transport::auth::AuthClient;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use rmcp::{RoleClient, RoleServer, ServerHandler};

use std::sync::Arc;

/// One connected upstream MCP server: its live client connection plus the
/// tool-name prefix (`{name}_`) this gateway exposes its tools under.
struct Upstream {
    prefix: String,
    client: RunningService<RoleClient, ()>,
}

/// MCP server that aggregates tools from multiple upstream MCP servers
/// behind a single endpoint, prefixing each upstream's tool names
/// (`editor_*`, `tasks_*`, ...) to avoid collisions and routing
/// `call_tool` back to the matching upstream by stripping that prefix.
#[derive(Clone)]
pub struct GatewayService {
    upstreams: Arc<Vec<Upstream>>,
    tools: Arc<Vec<Tool>>,
}

impl ServerHandler for GatewayService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "kid-mcp-gateway",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Aggregates tools from multiple upstream MCP servers. \
                 Each tool name is prefixed with its upstream's name, \
                 e.g. `editor_view`.",
            )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items((*self.tools).clone()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let (upstream, tool_name) = self
            .upstreams
            .iter()
            .find_map(|upstream| {
                request
                    .name
                    .strip_prefix(upstream.prefix.as_str())
                    .map(|stripped| (upstream, stripped.to_owned()))
            })
            .ok_or_else(|| {
                McpError::invalid_params(format!("unknown tool: {}", request.name), None)
            })?;

        upstream
            .client
            .call_tool({
                let mut params = CallToolRequestParams::new(tool_name);
                params.arguments = request.arguments;
                params
            })
            .await
            .map(Into::into)
            .map_err(|error| {
                McpError::internal_error(
                    format!(
                        "upstream '{}' failed to run tool: {error}",
                        upstream.prefix.trim_end_matches('_')
                    ),
                    None,
                )
            })
    }
}

impl GatewayService {
    /// Connects to every configured upstream and caches its (prefixed)
    /// tool list. An upstream that fails to connect or list its tools
    /// aborts startup entirely - a gateway silently missing an upstream
    /// would be worse than one that fails fast.
    pub async fn connect(upstream_configs: Vec<UpstreamConfig>) -> Result<Self> {
        let mut upstreams = Vec::with_capacity(upstream_configs.len());
        let mut tools = Vec::new();

        for config in upstream_configs {
            let auth_client =
                oauth::authenticated_client(&config.mcp_url, &config.client_id, &config.secret)
                    .await
                    .with_context(|| {
                        format!("failed to authenticate against upstream '{}'", config.name)
                    })?;

            let client = connect_upstream(auth_client, &config.mcp_url)
                .await
                .with_context(|| format!("failed to connect to upstream '{}'", config.name))?;

            let upstream_tools = client
                .list_all_tools()
                .await
                .with_context(|| format!("failed to list tools from upstream '{}'", config.name))?;

            let prefix = format!("{}_", config.name);
            tools.extend(
                upstream_tools
                    .into_iter()
                    .map(|tool| prefixed(tool, &prefix)),
            );

            upstreams.push(Upstream { prefix, client });
        }

        Ok(Self {
            upstreams: Arc::new(upstreams),
            tools: Arc::new(tools),
        })
    }
}

async fn connect_upstream(
    auth_client: AuthClient<reqwest::Client>,
    mcp_url: &url::Url,
) -> Result<RunningService<RoleClient, ()>> {
    let transport = StreamableHttpClientTransport::with_client(
        auth_client,
        StreamableHttpClientTransportConfig::with_uri(mcp_url.as_str().to_owned()),
    );
    let client = ().serve(transport).await?;
    Ok(client)
}

fn prefixed(mut tool: Tool, prefix: &str) -> Tool {
    tool.name = format!("{prefix}{}", tool.name).into();
    tool
}

/// Extracted for a unit test that doesn't need a live upstream connection.
#[cfg(test)]
fn route<'a>(prefixes: &'a [&'a str], tool_name: &str) -> Option<(&'a str, String)> {
    prefixes.iter().find_map(|prefix| {
        tool_name
            .strip_prefix(prefix)
            .map(|stripped| (*prefix, stripped.to_owned()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_matching_prefix() {
        let prefixes = ["editor_", "tasks_"];
        assert_eq!(
            route(&prefixes, "editor_view"),
            Some(("editor_", "view".to_owned()))
        );
        assert_eq!(
            route(&prefixes, "tasks_list"),
            Some(("tasks_", "list".to_owned()))
        );
    }

    #[test]
    fn rejects_unknown_prefix() {
        let prefixes = ["editor_", "tasks_"];
        assert_eq!(route(&prefixes, "unknown_tool"), None);
    }
}
