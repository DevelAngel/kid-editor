//! MCP (Model Context Protocol) server exposing text-editor tools
//! scoped to a single workspace directory.
//! All paths, relative or absolute, are resolved against the workspace root
//! and rejected if they would escape it.
mod helper;
mod tree;
mod view;

use anyhow::Result;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool_handler};

use std::path::PathBuf;

/// MCP server exposing a text-editor tool, sandboxed to one workspace root.
#[derive(Clone)]
pub struct McpService {
    workspace_root: PathBuf,
    tool_router: ToolRouter<Self>,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "kid-text-editor",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "File editor scoped to a single workspace. \
                All paths (relative or absolute) are resolved \
                against the workspace root.",
            )
    }
}

impl McpService {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            tool_router: Self::view_tool_router() + Self::tree_tool_router(),
        }
    }
}
