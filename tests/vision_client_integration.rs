//! Integration tests for `VisionClient` using a `wiremock` HTTP mock server.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use serde_json::json;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use vision_recognizer::openai_vision::VisionClient;
use wiremock::{
    matchers::{method, path},
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
