use crate::{settings, status_file};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
struct WriteStatusInput {
    /// The complete Markdown document to show in ctx-ppteer.
    markdown: String,
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
    #[tool(
        description = "Replace ctx-ppteer's configured status document with complete Markdown content."
    )]
    async fn write_status(
        &self,
        Parameters(WriteStatusInput { markdown }): Parameters<WriteStatusInput>,
    ) -> Result<String, McpError> {
        let path =
            settings::source_path().map_err(|error| McpError::internal_error(error, None))?;
        let bytes = status_file::write_markdown(&path, &markdown)
            .map_err(|error| McpError::internal_error(error, None))?;
        Ok(format!("Updated ctx-ppteer status ({bytes} bytes)."))
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
