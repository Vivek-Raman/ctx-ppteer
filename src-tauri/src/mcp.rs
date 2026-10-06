use crate::{settings, status_file};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
struct WriteStatusInput {
    /// The current project's folder name, such as "ctx-ppteer".
    folder_name: String,
    /// Complete Markdown for this project's status beneath its generated title. Include the plain-text branch first, followed by activity bullets.
    status: String,
}

#[derive(Deserialize, JsonSchema)]
struct ReadStatusInput {
    /// The current project's folder name, such as "ctx-ppteer".
    folder_name: String,
}

struct StatusServer {
    tool_router: ToolRouter<Self>,
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
    #[tool(description = "Return the current ctx-ppteer status section for one project folder.")]
    async fn read_status(
        &self,
        Parameters(ReadStatusInput { folder_name }): Parameters<ReadStatusInput>,
    ) -> Result<String, McpError> {
        let path =
            settings::source_path().map_err(|error| McpError::internal_error(error, None))?;
        status_file::read_project_status(&path, &folder_name)
            .map(|status| status.unwrap_or_default())
            .map_err(|error| McpError::internal_error(error, None))
    }

    #[tool(
        description = "Create or replace this project's ctx-ppteer status section. The server generates the Title Case project heading and preserves sections for other projects."
    )]
    async fn write_status(
        &self,
        Parameters(WriteStatusInput {
            folder_name,
            status,
        }): Parameters<WriteStatusInput>,
    ) -> Result<String, McpError> {
        let path =
            settings::source_path().map_err(|error| McpError::internal_error(error, None))?;
        let bytes = status_file::write_project_status(&path, &folder_name, &status)
            .map_err(|error| McpError::internal_error(error, None))?;
        Ok(format!("Updated {folder_name} status ({bytes} bytes)."))
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
