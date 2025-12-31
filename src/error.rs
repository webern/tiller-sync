//! Error types for the tiller application.
//!
//! This module provides:
//! - `TillerError` / `Result<T>` - structured error type for lib, pub and MCP use
//! - `Er` / `Res<T>` - anyhow-based types for internal use
//! - Trait `IntoResult<T>` for converting the internal error type to a public `Result<T>`

use serde::{Deserialize, Serialize};
use std::fmt;

/// This library's public result type.
pub type Result<T> = std::result::Result<T, TillerError>;

/// The type of error.
///
/// Errors are categorized into two groups:
/// - **Protocol errors**: JSON-RPC level failures (convert to `ErrorData`)
/// - **Tool errors**: Business logic failures (convert to `CallToolResult::error()`)
#[derive(
    Debug, Default, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "PascalCase")]
pub enum ErrorType {
    // === Protocol-level errors (become ErrorData) ===
    /// Malformed or invalid MCP request
    Request,

    /// An error from the MCP server or service, unrelated to tiller
    Service,

    // === Tool-level errors (become CallToolResult::error()) ===
    /// Unexpected internal failure, e.g. filesystem
    Internal,

    /// Sync operation failures (no backup, conflicts, formula issues)
    Sync,

    /// Authentication problems
    Auth,

    /// Configuration issues
    Config,

    /// SQLite database failures
    Database,

    /// An error whose precise type is not otherwise understood or known
    #[default]
    Other,
}

serde_plain::derive_display_from_serialize!(ErrorType);
serde_plain::derive_fromstr_from_deserialize!(ErrorType);

/// This library's public error type
#[derive(Debug)]
pub struct TillerError {
    inner: anyhow::Error,
    error_type: ErrorType,
}

impl TillerError {
    /// The `ErrorType` of the `TillerError`
    pub fn error_type(&self) -> ErrorType {
        self.error_type
    }

    /// Returns true if this should be categorized as an MCP protocol error.
    ///
    /// Protocol errors should be converted to `ErrorData` in MCP responses.
    pub fn is_protocol_error(&self) -> bool {
        match self.error_type() {
            ErrorType::Request => true,
            ErrorType::Service => true,
            ErrorType::Internal => false,
            ErrorType::Sync => false,
            ErrorType::Auth => false,
            ErrorType::Config => false,
            ErrorType::Database => false,
            ErrorType::Other => false,
        }
    }

    /// Returns true if this should be categorized as a tool error for MCP.
    ///
    /// Tool errors should be converted to `CallToolResult::error()` in MCP responses.
    pub fn is_tool_error(&self) -> bool {
        !self.is_protocol_error()
    }

    // === MCP Conversion Methods ===
    // These will be implemented when rmcp is added as a dependency.
    // For now, we provide the structure.

    // TODO: Implement when rmcp is added:
    // pub fn to_error_data(&self) -> rmcp::model::ErrorData { ... }
    // pub fn to_tool_error(&self) -> rmcp::model::CallToolResult { ... }
}

impl fmt::Display for TillerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} error: {:?}", self.error_type(), self.inner)
    }
}

impl std::error::Error for TillerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

/// A trait we can use to convert an `anyhow::Result<T>` into a public `Result<T>`
pub(crate) trait IntoResult<T> {
    fn pub_result(self, t: ErrorType) -> Result<T>;
}

/// Anyhow-based types for internal use. When constructing these, choose the name of the desired
/// public error at the start of the error message. For example `InvalidRequest: blah blah`.
pub(crate) type Er = anyhow::Error;
pub(crate) type Res<T> = std::result::Result<T, Er>;

// The implementation which makes an anyhow::Result convertible to a public Result
impl<T> IntoResult<T> for Res<T> {
    fn pub_result(self, t: ErrorType) -> Result<T> {
        match self {
            Ok(ok) => Ok(ok),
            Err(e) => Err(TillerError {
                inner: e,
                error_type: t,
            }),
        }
    }
}

#[test]
fn pub_result_test() {
    use anyhow::anyhow;
    let anyhow_error = anyhow!("MY_ERROR_MESSAGE");
    let anyhow_result: Res<()> = Err(anyhow_error);
    let result = anyhow_result.pub_result(ErrorType::Sync);
    let e = result.err().unwrap();
    let message = e.to_string().lines().next().unwrap().to_string();
    assert_eq!("Sync error: MY_ERROR_MESSAGE", message)
}
