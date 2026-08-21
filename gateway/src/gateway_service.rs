use crate::config::{UpstreamConfig, UpstreamName};

use anyhow::{Context, Result};
use indexmap::IndexMap;
use rmcp::ServiceExt;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ContentBlock, ErrorData as McpError, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RunningService};
use rmcp::transport::auth::AuthClient;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use rmcp::{RoleClient, RoleServer, ServerHandler};

use std::sync::Arc;

/// One connected upstream MCP server: its live client connection plus its
/// (already-prefixed) tool list. Keeping tools here, per upstream, instead
/// of in a separate flat list keeps "which tools exist" and "which
/// upstream serves them" as a single source of truth - there's no second
/// collection that could drift out of sync with the routing table.
struct Upstream {
    prefix: String,
    client: RunningService<RoleClient, ()>,
    tools: Vec<Tool>,
}

/// MCP server that aggregates tools from multiple upstream MCP servers
/// behind a single endpoint, prefixing each upstream's tool names
/// (`editor_*`, `tasks_*`, ...) to avoid collisions and routing
/// `call_tool` back to the matching upstream by stripping that prefix.
#[derive(Clone)]
pub struct GatewayService {
    upstreams: Arc<IndexMap<UpstreamName, Upstream>>,
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
        let tools = self
            .upstreams
            .values()
            .flat_map(|upstream| upstream.tools.iter().cloned())
            .collect();
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let prefixed_tool_name = request.name.to_string();

        let (upstream, tool_name) = self
            .upstreams
            .values()
            .find_map(|upstream| {
                request
                    .name
                    .strip_prefix(upstream.prefix.as_str())
                    .map(|stripped| (upstream, stripped.to_owned()))
            })
            .ok_or_else(|| {
                McpError::invalid_params(format!("unknown tool: {}", request.name), None)
            })?;

        tracing::debug!(tool = %prefixed_tool_name, arguments = ?request.arguments, "tool called");
        tracing::debug!(tool = %prefixed_tool_name, arguments = ?request.arguments, "tool called");
        let input_chars = request
            .arguments
            .as_ref()
            .and_then(|args| serde_json::to_string(args).ok())
            .map(|s| s.chars().count())
            .unwrap_or(0);

        let result: Result<CallToolResponse, McpError> = upstream
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
            });

        match &result {
            Ok(response) => {
                let output = response.output_text();
                tracing::info!(tool = %prefixed_tool_name, input_chars, output_chars = output.chars().count(), "tool succeeded");
                tracing::debug!(tool = %prefixed_tool_name, output = %output, "tool output");
            }
            Err(error) => tracing::info!(tool = %prefixed_tool_name, %error, "tool failed"),
        }

        result
    }
}

/// Extends the foreign `CallToolResponse` with text rendering for
/// logging, instead of a free function operating on it from outside —
/// same policy as `kid-text-editor`'s own tool-call logging.
trait ResponseTextExt {
    fn output_text(&self) -> String;
}

impl ResponseTextExt for CallToolResponse {
    fn output_text(&self) -> String {
        let CallToolResponse::Complete(result) = self else {
            return format!("{self:?}");
        };
        result
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl GatewayService {
    /// Connects to every configured upstream and caches its (prefixed)
    /// tool list. An upstream that fails to connect or list its tools
    /// aborts startup entirely - a gateway silently missing an upstream
    /// would be worse than one that fails fast.
    pub async fn connect(upstream_configs: IndexMap<UpstreamName, UpstreamConfig>) -> Result<Self> {
        let mut upstreams = IndexMap::with_capacity(upstream_configs.len());

        for (name, config) in upstream_configs {
            let auth_client =
                oauth::authenticated_client(&config.mcp_url, &config.client_id, &config.secret)
                    .await
                    .with_context(|| format!("failed to authenticate against upstream '{name}'"))?;

            let client = connect_upstream(auth_client, &config.mcp_url)
                .await
                .with_context(|| format!("failed to connect to upstream '{name}'"))?;

            let prefix = format!("{name}_");
            let tools = client
                .list_all_tools()
                .await
                .with_context(|| format!("failed to list tools from upstream '{name}'"))?
                .into_iter()
                .map(|tool| prefixed(tool, &prefix))
                .collect();

            upstreams.insert(
                name,
                Upstream {
                    prefix,
                    client,
                    tools,
                },
            );
        }

        Ok(Self {
            upstreams: Arc::new(upstreams),
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

    #[test]
    fn prefixed_preserves_annotations() {
        let mut annotations = rmcp::model::ToolAnnotations::default();
        annotations.read_only_hint = Some(true);
        annotations.title = Some("View File or Dir".to_owned());

        let tool = Tool::new("view", "view a file", Arc::new(Default::default()))
            .with_annotations(annotations);

        let result = prefixed(tool, "editor_");
        let annotations = result
            .annotations
            .expect("annotations should survive prefixing");
        assert_eq!(annotations.read_only_hint, Some(true));
        assert_eq!(annotations.title.as_deref(), Some("View File or Dir"));
    }
}
