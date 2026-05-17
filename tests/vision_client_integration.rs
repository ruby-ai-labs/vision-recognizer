//! Integration tests for `VisionClient` using a `wiremock` HTTP mock server.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use rmcp::{handler::server::wrapper::Parameters, Json};
use serde_json::json;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use vision_recognizer::{
    mcp::{EstimatePortionInput, EstimatePortionOutput, VisionHandler},
    openai_vision::VisionClient,
};
use wiremock::{
    matchers::{body_string_contains, method, path},
    Mock, MockServer, ResponseTemplate,
};

/// Write minimal valid PNG bytes to a tempfile and return the path.
fn make_temp_png() -> Result<(NamedTempFile, PathBuf)> {
    // 1×1 white PNG (minimal valid file)
    let png_bytes: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature
        0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR chunk length + type
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1×1
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // 8-bit RGB
        0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, // IDAT chunk
        0x54, 0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xe2, 0x21,
        0xbc, 0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, // IEND chunk
        0x44, 0xae, 0x42, 0x60, 0x82,
    ];
    let tmp = tempfile::Builder::new().suffix(".png").tempfile()?;
    std::fs::write(tmp.path(), png_bytes)?;
    let path = tmp.path().to_path_buf();
    Ok((tmp, path))
}

/// AC6: `VisionClient::recognize` returns Err on HTTP 429.
#[tokio::test]
async fn returns_err_on_429() -> Result<()> {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
        .mount(&mock_server)
        .await;

    let (_tmp, path) = make_temp_png()?;
    let client = VisionClient::new("sk-test", mock_server.uri())?;
    let result = client.recognize(&path, "describe", "gpt-4o-mini").await;

    assert!(result.is_err(), "expected Err on 429");
    let msg = format!("{:#}", result.unwrap_err());
    assert!(
        msg.contains("429"),
        "error must mention HTTP status 429, got: {msg}"
    );
    Ok(())
}

/// AC6: `VisionClient::recognize` returns Err on HTTP 4xx generic error.
#[tokio::test]
async fn returns_err_on_4xx() -> Result<()> {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&mock_server)
        .await;

    let (_tmp, path) = make_temp_png()?;
    let client = VisionClient::new("sk-test", mock_server.uri())?;
    let result = client.recognize(&path, "describe", "gpt-4o-mini").await;

    assert!(result.is_err(), "expected Err on 401");
    let msg = format!("{:#}", result.unwrap_err());
    assert!(
        msg.contains("401"),
        "error must mention HTTP status 401, got: {msg}"
    );
    Ok(())
}

/// AC6: `VisionClient::recognize` returns Ok on successful 200 response.
#[tokio::test]
async fn returns_ok_on_200_with_valid_response() -> Result<()> {
    let mock_server = MockServer::start().await;
    let response_body = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "A white square image."
            }
        }]
    });

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let (_tmp, path) = make_temp_png()?;
    let client = VisionClient::new("sk-test", mock_server.uri())?;
    let text = client.recognize(&path, "describe", "gpt-4o-mini").await?;

    assert_eq!(text, "A white square image.");
    Ok(())
}

// ── estimate_portion integration tests (AC2, AC3, AC6) ────────────────────

/// AC2: `estimate_portion` happy path — wiremock returns valid JSON, handler
/// parses and returns `EstimatePortionOutput` with at least one item.
#[tokio::test]
async fn estimate_portion_happy_path() -> Result<()> {
    let mock_server = MockServer::start().await;
    let response_body = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "{\"items\":[{\"name\":\"плов\",\"estimated_grams\":\"250\",\"confidence\":\"high\",\"reasoning\":\"тарелка диаметром 25 см\"}]}"
            }
        }]
    });

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let (_tmp, img_path) = make_temp_png()?;
    std::env::set_var("OPENAI_API_KEY", "sk-test");
    std::env::set_var("OPENAI_BASE_URL", mock_server.uri());

    let handler = VisionHandler::new();
    let input = EstimatePortionInput {
        image_path: img_path.to_string_lossy().to_string(),
        foods_list: vec!["плов".to_owned()],
        reference: None,
        prompt: None,
        model: None,
    };
    let result: Json<EstimatePortionOutput> = handler
        .estimate_portion(Parameters(input))
        .await
        .map_err(|e| anyhow::anyhow!("{}", e.message))?;

    assert_eq!(result.0.items.len(), 1, "expected 1 item");
    assert_eq!(result.0.items[0].name, "плов");
    assert_eq!(result.0.items[0].estimated_grams, "250");
    assert_eq!(result.0.items[0].confidence, "high");
    assert!(
        !result.0.items[0].reasoning.is_empty(),
        "reasoning must not be empty"
    );

    std::env::remove_var("OPENAI_BASE_URL");
    Ok(())
}

/// AC6: `estimate_portion` strips markdown fences from LLM response and parses
/// correctly.
#[tokio::test]
async fn estimate_portion_markdown_wrap_stripped() -> Result<()> {
    let mock_server = MockServer::start().await;
    // LLM wraps its response in markdown code fences despite instructions.
    let response_body = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "```json\n{\"items\":[{\"name\":\"рис\",\"estimated_grams\":\"150\",\"confidence\":\"med\",\"reasoning\":\"стандартная порция\"}]}\n```"
            }
        }]
    });

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let (_tmp, img_path) = make_temp_png()?;
    std::env::set_var("OPENAI_API_KEY", "sk-test");
    std::env::set_var("OPENAI_BASE_URL", mock_server.uri());

    let handler = VisionHandler::new();
    let input = EstimatePortionInput {
        image_path: img_path.to_string_lossy().to_string(),
        foods_list: vec!["рис".to_owned()],
        reference: None,
        prompt: None,
        model: None,
    };
    let result: Json<EstimatePortionOutput> = handler
        .estimate_portion(Parameters(input))
        .await
        .map_err(|e| anyhow::anyhow!("{}", e.message))?;

    assert_eq!(
        result.0.items.len(),
        1,
        "expected 1 item after fence stripping"
    );
    assert_eq!(result.0.items[0].name, "рис");
    assert_eq!(result.0.items[0].estimated_grams, "150");

    std::env::remove_var("OPENAI_BASE_URL");
    Ok(())
}

/// AC3: `estimate_portion` with reference text — wiremock verifies reference
/// string appears in the request body sent to `OpenAI`.
#[tokio::test]
async fn estimate_portion_reference_in_prompt() -> Result<()> {
    let mock_server = MockServer::start().await;
    let response_body = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "{\"items\":[{\"name\":\"курица\",\"estimated_grams\":\"200\",\"confidence\":\"med\",\"reasoning\":\"ориентир — ладонь\"}]}"
            }
        }]
    });

    // Assert that the request body contains the reference text.
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_string_contains("ладонь рядом"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let (_tmp, img_path) = make_temp_png()?;
    std::env::set_var("OPENAI_API_KEY", "sk-test");
    std::env::set_var("OPENAI_BASE_URL", mock_server.uri());

    let handler = VisionHandler::new();
    let input = EstimatePortionInput {
        image_path: img_path.to_string_lossy().to_string(),
        foods_list: vec!["курица".to_owned()],
        reference: Some("ладонь рядом".to_owned()),
        prompt: None,
        model: None,
    };
    let result: Json<EstimatePortionOutput> = handler
        .estimate_portion(Parameters(input))
        .await
        .map_err(|e| anyhow::anyhow!("handler returned error: {}", e.message))?;

    assert_eq!(result.0.items.len(), 1);
    assert_eq!(result.0.items[0].name, "курица");

    std::env::remove_var("OPENAI_BASE_URL");
    Ok(())
}
