use crate::commands::Out;
use rmcp::model::{CallToolResult, Content};
use rmcp::ErrorData;
use serde::Serialize;
use std::fmt::Debug;
use tracing::error;

pub(super) fn to_content<T>(out: Out<T>) -> Vec<Content>
where
    T: Debug + Clone + Serialize,
{
    let mut content = vec![Content::text(out.message())];
    if let Some(object) = out.structure() {
        match Content::json(object) {
            Ok(json) => content.push(json),
            Err(e) => error!("Unable to serialize JSON output: {e}"),
        };
    }
    content
}

pub(super) fn tool_result<T>(result: crate::Result<Out<T>>) -> Result<CallToolResult, ErrorData>
where
    T: Debug + Clone + Serialize,
{
    Ok(match result {
        Ok(out) => CallToolResult::success(to_content(out)),
        Err(e) => CallToolResult::error(vec![Content::text(e.to_string())]),
    })
}
