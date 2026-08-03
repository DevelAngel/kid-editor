//! MCP (Model Context Protocol) server exposing text-editor tools
//! scoped to a single workspace directory.
//! All paths, relative or absolute, are resolved against the workspace root
//! and rejected if they would escape it.
mod create;
mod insert;
mod just_run;
mod str_replace;
mod tree;
mod view;
mod workspace_path;

pub(crate) use just_run::RecipeName;

use anyhow::Result;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool_handler};

use std::collections::HashSet;
use std::path::PathBuf;

/// MCP server exposing a text-editor tool, sandboxed to one workspace root.
#[derive(Clone)]
pub struct McpService {
    workspace_root: PathBuf,
    /// Names treated as nonexistent by every tool, e.g. ".git" or "target" —
    /// not just hidden from `tree`, but unreadable, unwritable, and invisible
    /// everywhere a path is resolved.
    ignore: Vec<String>,
    /// Just Recipes, discovered once at construction time.
    /// Empty if the workspace has no `justfile`.
    just_recipes: HashSet<RecipeName>,
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
    /// `just_recipes` is discovered once by the caller (see
    /// `McpServer::serve`), not per instance — `McpService::new` runs
    /// once per client session, and re-running `just --summary` on every
    /// new session would be wasted work for a result that can't change
    /// mid-process.
    pub fn new(
        workspace_root: PathBuf,
        ignore: Vec<String>,
        just_recipes: HashSet<RecipeName>,
    ) -> Self {
        let mut tool_router = Self::create_tool_router()
            + Self::insert_tool_router()
            + Self::str_replace_tool_router()
            + Self::tree_tool_router()
            + Self::view_tool_router();
        if !just_recipes.is_empty() {
            tool_router += Self::just_run_tool_router();
        }

        Self {
            workspace_root,
            ignore,
            just_recipes,
            tool_router,
        }
    }
}
