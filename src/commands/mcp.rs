//! MCP server command handler.
//!
//! This module implements the `tiller mcp` command which runs an MCP server
//! for AI agent integration.

use crate::commands::Out;
use crate::mcp::Io;
use crate::{mcp, Config, Mode, Result};

/// Runs the MCP server.
///
/// This launches a long-running process that communicates via JSON-RPC over stdin/stdout.
/// MCP clients (like Claude Code) launch this as a subprocess.
pub async fn mcp(config: Config, mode: Mode) -> Result<Out<()>> {
    mcp::run_server(config, mode, Io::Stdio).await?;
    Ok("Done running MCP server".into())
}
