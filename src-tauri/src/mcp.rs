use crate::{
    paths,
    status::domain::Point,
    status::store::{StatusError, StatusStore},
};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct WriteStatusInput {
    /// The current project's folder name, such as "ctx-ppteer".
    folder_name: String,
    /// Your stable session ID from the harness.
    session_id: String,
    /// Currently checked out branch.
    branch: String,
    /// Complete revised point list; omitted points are removed.
    points: Vec<Point>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadStatusInput {
    /// The current project's folder name, such as "ctx-ppteer".
    folder_name: String,
    /// Your stable session ID from the harness.
    session_id: String,
}

struct StatusServer {
    tool_router: ToolRouter<Self>,
}

fn map_status_error(error: StatusError) -> McpError {
    match error {
        StatusError::InvalidInput(message) => McpError::invalid_params(message, None),
        StatusError::Conflict(message) => McpError::invalid_params(message, None),
        StatusError::InvalidDocument(message) | StatusError::Io(message) => {
            McpError::internal_error(message, None)
        }
    }
}

impl StatusServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router(router = tool_router)]
impl StatusServer {
    #[tool(
        description = "Read your submitted points for this project and session before revising them."
    )]
    async fn read_status(
        &self,
        Parameters(input): Parameters<ReadStatusInput>,
    ) -> Result<String, McpError> {
        let path = paths::status_path()
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        let entries = StatusStore::new(path)
            .read_project(&input.folder_name, &input.session_id)
            .map_err(map_status_error)?;
        serde_json::to_string(&entries)
            .map_err(|error| McpError::internal_error(error.to_string(), None))
    }

    #[tool(
        description = "Save your revised points for this project and session. Replaces the previous point list; omitted points are removed."
    )]
    async fn write_status(
        &self,
        Parameters(input): Parameters<WriteStatusInput>,
    ) -> Result<String, McpError> {
        let path = paths::status_path().map_err(|error| McpError::internal_error(error, None))?;
        let bytes = StatusStore::new(path)
            .upsert(
                input.folder_name.clone(),
                input.session_id,
                input.branch,
                input.points,
            )
            .map_err(|error| map_status_error(error))?;
        Ok(format!(
            "Updated {} status ({bytes} bytes).",
            input.folder_name
        ))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for StatusServer {}

pub fn run() -> Result<(), String> {
    tokio::runtime::Runtime::new()
        .map_err(|error| error.to_string())?
        .block_on(async {
            StatusServer::new()
                .serve(rmcp::transport::stdio())
                .await
                .map_err(|error| error.to_string())?
                .waiting()
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_tools_require_a_string_session_id() {
        for value in [
            serde_json::json!({"folder_name": "project"}),
            serde_json::json!({"folder_name": "project", "session_id": null}),
        ] {
            assert!(serde_json::from_value::<ReadStatusInput>(value.clone()).is_err());
            let mut write = value;
            write["branch"] = serde_json::json!("main");
            write["points"] = serde_json::json!([{"text": "activity"}]);
            assert!(serde_json::from_value::<WriteStatusInput>(write).is_err());
        }
        for schema in [
            schemars::schema_for!(ReadStatusInput),
            schemars::schema_for!(WriteStatusInput),
        ] {
            let value = serde_json::to_value(schema).unwrap();
            assert!(value["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field == "session_id"));
            assert_eq!(value["properties"]["session_id"]["type"], "string");
        }
    }
}
