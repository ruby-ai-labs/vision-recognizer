//! MCP stdio server for vision-recognizer.
//!
//! Exposes one tool: `vision.recognize_image` — analyses an image file
//! via the `OpenAI` Vision API.

use anyhow::Result;
use rmcp::{
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router, ErrorData, Json, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::openai_vision::VisionClient;

// ── Input / Output types ───────────────────────────────────────────────────

/// Input schema for `vision.recognize_image`.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct RecognizeImageInput {
    /// Absolute path to the image file (jpeg, png, webp, gif).
    pub image_path: String,

    /// Natural language prompt for the vision model.
    pub prompt: String,

    /// `OpenAI` model override (optional, default: `gpt-4o-mini`).
    pub model: Option<String>,
}

/// Output wrapper for `vision.recognize_image`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub(crate) struct RecognizeImageOutput {
    /// Vision model text response.
    pub text: String,
}

// ── Handler ────────────────────────────────────────────────────────────────

/// MCP handler that exposes vision recognition tools.
#[derive(Clone)]
pub(crate) struct VisionHandler {
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl VisionHandler {
    /// Create a new handler.
    pub(crate) fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// `vision.recognize_image` — analyse an image via `OpenAI` Vision API.
    #[tool(
        name = "vision.recognize_image",
        description = "Analyze an image file using OpenAI Vision API (gpt-4o-mini by default). \
                       Reads OPENAI_API_KEY from environment. \
                       Returns the model's text description of the image. \
                       USE WHEN: user sends a Telegram photo (image_path attribute present); \
                       user asks to identify food on a photo; \
                       user needs nutritional analysis from a food image; \
                       user wants to know what is depicted in an image. \
                       DO NOT USE for audio files, plain text, or PDF documents \
                       — this tool only handles image content (jpeg, png, webp, gif)."
    )]
    pub async fn recognize_image(
        &self,
        Parameters(input): Parameters<RecognizeImageInput>,
    ) -> Result<Json<RecognizeImageOutput>, ErrorData> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| ErrorData::internal_error("OPENAI_API_KEY is not set".to_owned(), None))?;

        let client = VisionClient::new(api_key, "https://api.openai.com")
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let model = input.model.as_deref().unwrap_or("gpt-4o-mini");
        let path = std::path::PathBuf::from(&input.image_path);

        let text = client
            .recognize(&path, &input.prompt, model)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(RecognizeImageOutput { text }))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for VisionHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new(
                "vision-recognizer-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Vision recognition tools powered by OpenAI Vision API. \
                 Use vision.recognize_image to analyze images.",
            )
    }
}

// ── Entry point ────────────────────────────────────────────────────────────

/// Start the MCP stdio server and block until the client disconnects.
///
/// # Errors
///
/// Returns an error if the stdio transport or the underlying service fails.
pub async fn run() -> Result<()> {
    tracing::info!("starting vision-recognizer MCP server on stdio");
    let handler = VisionHandler::new();
    let transport = rmcp::transport::stdio();
    let service = handler.serve(transport).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC6: `VisionHandler` must expose a tool named `vision.recognize_image`.
    #[test]
    fn tool_router_lists_recognize_image() {
        let handler = VisionHandler::new();
        let tools = handler.tool_router.list_all();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(
            names.contains(&"vision.recognize_image"),
            "tool list must contain vision.recognize_image, got: {names:?}"
        );
    }

    /// AC1: `VisionHandler` must expose a tool named `vision.analyze_video`.
    #[test]
    fn tool_router_lists_analyze_video() {
        let handler = VisionHandler::new();
        let tools = handler.tool_router.list_all();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(
            names.contains(&"vision.analyze_video"),
            "tool list must contain vision.analyze_video, got: {names:?}"
        );
    }

    /// AC6: tool description contains USE WHEN / DO NOT USE guidance.
    #[test]
    fn tool_description_contains_when_to_use() {
        let handler = VisionHandler::new();
        let tools = handler.tool_router.list_all();
        let maybe_tool = tools
            .iter()
            .find(|t| t.name.as_ref() == "vision.recognize_image");
        assert!(
            maybe_tool.is_some(),
            "vision.recognize_image must be registered"
        );
        let desc = maybe_tool
            .and_then(|t| t.description.as_deref())
            .unwrap_or("");
        assert!(
            desc.contains("USE WHEN"),
            "description must contain 'USE WHEN', got: {desc}"
        );
        assert!(
            desc.contains("DO NOT USE"),
            "description must contain 'DO NOT USE', got: {desc}"
        );
    }

    /// AC6: missing `OPENAI_API_KEY` returns MCP `ErrorData`.
    #[tokio::test]
    async fn recognize_image_missing_key_returns_mcp_error() {
        // Remove key to ensure it's absent for this test
        std::env::remove_var("OPENAI_API_KEY");
        let handler = VisionHandler::new();
        let input = RecognizeImageInput {
            image_path: "/tmp/test.png".to_owned(),
            prompt: "describe".to_owned(),
            model: None,
        };
        let result = handler.recognize_image(Parameters(input)).await;
        assert!(
            result.is_err(),
            "must return Err when OPENAI_API_KEY is not set"
        );
    }
}
