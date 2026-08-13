//! MCP (Model Context Protocol) server implementation.
//!
//! This module provides an MCP server that exposes tiller functionality as tools
//! for AI agent integration. The server communicates via JSON-RPC over stdio.

mod mcp_utils;
mod tools;

use crate::{Config, Mode};
use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::transport::stdio;
use rmcp::{tool_handler, ServerHandler, ServiceExt};
use std::sync::Arc;
use tracing::info;

/// The tiller MCP server.
///
/// This server exposes tiller sync functionality as MCP tools.
#[derive(Debug, Clone)]
pub struct TillerServer {
    mode: Mode,
    config: Arc<Config>,
    tool_router: ToolRouter<TillerServer>,
}

impl TillerServer {
    /// Creates a new TillerServer with the given configuration.
    pub fn new(config: Config, mode: Mode) -> Self {
        Self {
            mode,
            config: Arc::new(config),
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TillerServer {
    /// Returns server information sent to the MCP client during initialization.
    ///
    /// The `instructions` field is the specification's way of telling an AI client what this
    /// server is for. It is deliberately short: the client pays for it in context on every session,
    /// whether or not tiller ends up being used. The in-depth guide lives behind the `instructions`
    /// tool, which the agent can call when it wants the detail.
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(include_str!("docs/INTRO.md"));
        info.protocol_version = ProtocolVersion::V_2024_11_05;
        info.server_info = Implementation::new("tiller", env!("CARGO_PKG_VERSION"));
        info
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
    use crate::args::UpdateTransactionsArgs;
    use crate::test::TestEnv;
    use rmcp::ServiceExt;
    use tokio::io::duplex;

    /// Integration test for the MCP server using an in-memory transport.
    /// Tests the sync_down, sync_up and update_transactions tools.
    #[tokio::test]
    async fn test_mcp_server_integration() {
        // Create duplex channel - one end for server, one for client
        let (client_io, server_io) = duplex(4096);

        // Create test environment (holds TempDir alive for duration of test)
        let env = TestEnv::new().await;
        let config = env.config();

        // Spawn server in background task
        let server_handle =
            tokio::spawn(
                async move { run_server(config, Mode::Testing, Io::Mock(server_io)).await },
            );

        // Create MCP client connected to the other end
        let client = ().serve(client_io).await.expect("Failed to create client");

        // Test 1: sync_down works without any prior "initialization" call. The server used to
        // reject every tool until `initialize_service` had been called at least once.
        let sync_down_result = client
            .call_tool(rmcp::model::CallToolRequestParams::new("sync_down"))
            .await
            .expect("sync_down call failed");

        assert!(
            !sync_down_result.is_error.unwrap_or(false),
            "sync_down returned error: {:?}",
            sync_down_result.content
        );

        // Test 2: Call sync_up tool with force and formulas params
        let mut args = serde_json::Map::new();
        args.insert("force".into(), serde_json::Value::Bool(true));
        args.insert(
            "formulas".into(),
            serde_json::Value::String("ignore".into()),
        );

        let sync_up_result = client
            .call_tool(rmcp::model::CallToolRequestParams::new("sync_up").with_arguments(args))
            .await
            .expect("sync_up call failed");

        assert!(
            !sync_up_result.is_error.unwrap_or(false),
            "sync_up returned error: {:?}",
            sync_up_result.content
        );

        // Test 3: Call update_transaction tool
        // After sync_down, we have transactions in the database. Get one to update.
        let tiller_data = env.config().db().get_tiller_data().await.unwrap();
        let first_txn = &tiller_data.transactions.data()[0];
        let txn_id = first_txn.transaction_id.clone();
        let updates = crate::model::TransactionUpdates {
            note: Some("Updated via MCP".to_string()),
            ..Default::default()
        };
        let updates = UpdateTransactionsArgs::new(vec![txn_id], updates).unwrap();
        let updates_json = serde_json::to_value(&updates)
            .unwrap()
            .as_object()
            .unwrap()
            .clone();

        let update_result = client
            .call_tool(
                rmcp::model::CallToolRequestParams::new("update_transactions")
                    .with_arguments(updates_json),
            )
            .await
            .expect("update_transactions call failed");

        assert!(
            !update_result.is_error.unwrap_or(false),
            "update_transactions returned error: {:?}",
            update_result.content
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

    /// The server used to reject every tool call until `initialize_service` had been called,
    /// which forced a help lookup before any real work could start. Tools are now callable
    /// straight away, like any other MCP server.
    #[tokio::test]
    async fn test_tools_need_no_initialization_call() {
        let (client_io, server_io) = duplex(4096);
        let env = TestEnv::new().await;
        let config = env.config();

        let _server_handle =
            tokio::spawn(
                async move { run_server(config, Mode::Testing, Io::Mock(server_io)).await },
            );
        let client = ().serve(client_io).await.expect("Failed to create client");

        let tools = client
            .list_tools(Default::default())
            .await
            .expect("Failed to list tools");
        let names: Vec<&str> = tools.tools.iter().map(|t| t.name.as_ref()).collect();

        assert!(
            !names.contains(&"initialize_service"),
            "the initialization tool should be gone, found: {names:?}"
        );
        assert!(
            names.contains(&"instructions"),
            "the in-depth guide should still be reachable, found: {names:?}"
        );

        // Every tool must work as the very first call of the session. `schema` is the cheapest one
        // that touches real state.
        let result = client
            .call_tool(rmcp::model::CallToolRequestParams::new("schema"))
            .await
            .expect("schema call failed");

        assert!(
            !result.is_error.unwrap_or(false),
            "a tool called before anything else should succeed, got: {:?}",
            result.content
        );
    }

    /// Queries MCP tool definitions and writes them to `.ignore/mcp_tools.txt`.
    /// This provides a human-readable dump of the tool schemas for inspection.
    #[tokio::test]
    async fn write_mcp_tools_to_file() {
        use std::fs::{self, File};
        use std::io::Write;
        use std::path::PathBuf;

        fn project_root() -> PathBuf {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        }

        // Create duplex channel
        let (client_io, server_io) = duplex(4096);

        // Create test environment
        let env = TestEnv::new().await;
        let config = env.config();

        // Spawn server in background
        let _server_handle =
            tokio::spawn(
                async move { run_server(config, Mode::Testing, Io::Mock(server_io)).await },
            );

        // Create MCP client
        let client = ().serve(client_io).await.expect("Failed to create client");

        // Get the list of tools
        let tools_response = client
            .list_tools(Default::default())
            .await
            .expect("Failed to list tools");

        // Build output string
        let mut output = String::new();
        output.push_str(&format!(
            "=== MCP Tools ({} total) ===\n\n",
            tools_response.tools.len()
        ));

        for tool in &tools_response.tools {
            output.push_str(
                "────────────────────────────────────────────────────────────────────────────────\n",
            );
            output.push_str(&format!("TOOL: {}\n", tool.name));
            output.push_str(
                "────────────────────────────────────────────────────────────────────────────────\n",
            );
            output.push_str("\nDescription:\n");
            if let Some(desc) = &tool.description {
                for desc_line in desc.lines() {
                    output.push_str(&format!("  {}\n", desc_line));
                }
            }
            output.push_str("\nInput Schema:\n");
            output.push_str(&serde_json::to_string_pretty(&tool.input_schema).unwrap());
            output.push_str("\n\n");
        }

        // Write to .ignore/mcp_tools.txt
        let ignore_dir = project_root().join(".ignore");
        fs::create_dir_all(&ignore_dir).expect("Failed to create .ignore directory");

        let output_path = ignore_dir.join("mcp_tools.txt");
        let mut file = File::create(&output_path).expect("Failed to create output file");
        file.write_all(output.as_bytes())
            .expect("Failed to write output");
    }
}
