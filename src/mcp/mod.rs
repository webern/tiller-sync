//! MCP (Model Context Protocol) server implementation.
//!
//! This module provides an MCP server that exposes tiller functionality as tools
//! for AI agent integration. The server communicates via JSON-RPC over stdio.

/// Checks if the server has been initialized and returns an error if not.
macro_rules! require_init {
    ($self:expr) => {
        if !$self.check_initialized().await {
            return Self::uninitialized();
        }
    };
}

mod sync;

use crate::{Config, Mode};
use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{
    CallToolResult, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::transport::stdio;
use rmcp::ErrorData as McpError;
use rmcp::{tool_handler, ServerHandler, ServiceExt};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

/// The tiller MCP server.
///
/// This server exposes tiller sync functionality as MCP tools.
#[derive(Debug, Clone)]
pub struct TillerServer {
    initialized: Arc<Mutex<bool>>,
    mode: Mode,
    #[allow(dead_code)]
    config: Arc<Config>,
    tool_router: ToolRouter<TillerServer>,
}

impl TillerServer {
    /// Creates a new TillerServer with the given configuration.
    pub fn new(config: Config, mode: Mode) -> Self {
        Self {
            initialized: Arc::new(Mutex::new(false)),
            mode,
            config: Arc::new(config),
            tool_router: Self::tool_router(),
        }
    }

    async fn check_initialized(&self) -> bool {
        *self.initialized.lock().await
    }

    fn uninitialized() -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::error(vec![rmcp::model::Content::text(
            "You have not yet initialized the service. Please call __initialize_service__ first.",
        )]))
    }
}

#[tool_handler]
impl ServerHandler for TillerServer {
    /// Returns server information sent to the MCP client during initialization.
    ///
    /// The `instructions` field is intended by the specification to be the primary way to
    /// communicate the server's purpose and usage to AI agents like Claude Code. This text is shown
    /// to the AI to help it understand when and how to use this server's tools. However, it has
    /// been noted that agents tend to consider this reading as optional. We have solved this
    /// problem by requiring agents to call an `__initialize_service__` tool before anything else.
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "tiller".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                ..Default::default()
            },
            instructions: Some(include_str!("docs/INTRO.md").into()),
        }
    }
}

/// Transport type for the MCP server.
#[derive(Debug, Default)]
pub(crate) enum Io {
    #[default]
    Stdio,
    /// Mock transport for testing - holds one end of a duplex channel.
    #[cfg(test)]
    Mock(tokio::io::DuplexStream),
}

/// Runs the MCP server with stdio transport or mock transport. This function starts the MCP server
/// and blocks until the client disconnects or an error occurs.
///
/// # Arguments
/// - `config`: The `Config` object
/// - `mode`: Whether we are running with a live Google sheet or with a test sheet
/// - `io`: Whether we are using stdio as the transport or using mock io for testing
///
pub(crate) async fn run_server(config: Config, mode: Mode, io: Io) -> crate::Result<()> {
    use crate::error::{ErrorType, IntoResult};
    let server = TillerServer::new(config, mode);
    info!("Starting MCP server...");

    let service = match io {
        Io::Stdio => server
            .serve(stdio())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to start MCP server: {e}"))
            .pub_result(ErrorType::Service)?,
        #[cfg(test)]
        Io::Mock(stream) => server
            .serve(stream)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to start MCP server: {e}"))
            .pub_result(ErrorType::Service)?,
    };

    info!("MCP server running, waiting for requests...");

    // Wait for the server to complete (client disconnects or error)
    service
        .waiting()
        .await
        .map_err(|e| anyhow::anyhow!("MCP server error: {e}"))
        .pub_result(ErrorType::Service)?;

    info!("MCP server shut down");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;
    use rmcp::ServiceExt;
    use tempfile::TempDir;
    use tokio::io::duplex;

    /// Create a test Config with temporary directories.
    async fn create_test_config() -> (Config, TempDir) {
        let dir = TempDir::new().unwrap();
        let home_dir = dir.path().join("tiller_home");
        let secret_source_file = dir.path().join("client_secret.json");

        // Write a minimal client_secret.json
        let secret_content = r#"{"installed":{"client_id":"test","client_secret":"test","redirect_uris":["http://localhost"]}}"#;
        crate::utils::write(&secret_source_file, secret_content)
            .await
            .unwrap();

        let sheet_url = "https://docs.google.com/spreadsheets/d/TestSheetId/edit";

        let config = Config::create(&home_dir, &secret_source_file, sheet_url)
            .await
            .unwrap();

        (config, dir)
    }

    /// Integration test for the MCP server using an in-memory transport.
    /// Tests initialize_service, sync_down, and sync_up tools.
    #[tokio::test]
    async fn test_mcp_server_integration() {
        // Create duplex channel - one end for server, one for client
        let (client_io, server_io) = duplex(4096);

        // Create test config (keep TempDir alive for duration of test)
        let (config, _temp_dir) = create_test_config().await;

        // Spawn server in background task
        let server_handle =
            tokio::spawn(
                async move { run_server(config, Mode::Testing, Io::Mock(server_io)).await },
            );

        // Create MCP client connected to the other end
        let client = ().serve(client_io).await.expect("Failed to create client");

        // Test 1: Call initialize_service tool
        let init_result = client
            .call_tool(rmcp::model::CallToolRequestParam {
                name: "initialize_service".into(),
                arguments: None,
            })
            .await
            .expect("initialize_service call failed");

        assert!(
            !init_result.is_error.unwrap_or(false),
            "initialize_service returned error: {:?}",
            init_result.content
        );

        // Test 2: Call sync_down tool
        let sync_down_result = client
            .call_tool(rmcp::model::CallToolRequestParam {
                name: "sync_down".into(),
                arguments: None,
            })
            .await
            .expect("sync_down call failed");

        assert!(
            !sync_down_result.is_error.unwrap_or(false),
            "sync_down returned error: {:?}",
            sync_down_result.content
        );

        // Test 3: Call sync_up tool with force and formulas params
        let mut args = serde_json::Map::new();
        args.insert("force".into(), serde_json::Value::Bool(true));
        args.insert(
            "formulas".into(),
            serde_json::Value::String("ignore".into()),
        );

        let sync_up_result = client
            .call_tool(rmcp::model::CallToolRequestParam {
                name: "sync_up".into(),
                arguments: Some(args),
            })
            .await
            .expect("sync_up call failed");

        assert!(
            !sync_up_result.is_error.unwrap_or(false),
            "sync_up returned error: {:?}",
            sync_up_result.content
        );

        // Drop client to trigger server shutdown
        drop(client);

        // Wait for server to finish (with timeout)
        let server_result = tokio::time::timeout(std::time::Duration::from_secs(5), server_handle)
            .await
            .expect("Server timed out")
            .expect("Server task panicked");

        assert!(
            server_result.is_ok(),
            "Server returned error: {:?}",
            server_result
        );
    }
}
