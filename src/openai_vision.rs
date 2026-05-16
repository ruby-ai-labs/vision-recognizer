//! `OpenAI` Vision API client — sends images to `POST /v1/chat/completions`
//! using the `image_url` content type with base64-encoded data URLs.

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;
use std::path::Path;

/// Thin wrapper around the `OpenAI` Chat Completions endpoint for vision.
pub struct VisionClient {
    api_key: String,
    client: reqwest::Client,
    base_url: String,
}

impl VisionClient {
    /// Create a new client.
    ///
    /// `base_url` is normally `"https://api.openai.com"`. Tests can supply a
    /// mock URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying TLS stack cannot be initialised.
    pub fn new(api_key: impl Into<String>, base_url: impl Into<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_mins(1))
            .build()
            .context("failed to initialise HTTP client (TLS)")?;
        Ok(Self {
            api_key: api_key.into(),
            client,
            base_url: base_url.into(),
        })
    }

    /// Return the MIME type for a supported image extension.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported or HEIC formats.
    pub(crate) fn mime_for_ext(ext: &str) -> Result<&'static str> {
        match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" => Ok("image/jpeg"),
            "png" => Ok("image/png"),
            "webp" => Ok("image/webp"),
            "gif" => Ok("image/gif"),
            "heic" | "heif" => bail!(
                "HEIC format is not supported by OpenAI Vision API. \
                 Send JPEG or PNG instead."
            ),
            other => bail!("unsupported image format: .{other} — use jpeg, png, webp, or gif"),
        }
    }

    /// Analyse a sequence of image frames using `OpenAI` Vision.
    ///
    /// Builds a single `messages[0].content` array with one `image_url` item per
    /// frame followed by one `text` item containing `prompt`.  All frames are
    /// assumed to be JPEG (as produced by `video::extract_frames`).
    ///
    /// # Errors
    ///
    /// Returns an error if `frames` is empty, any frame file cannot be read, the
    /// HTTP request fails, or the API returns a non-2xx status code.
    pub async fn recognize_sequence(
        &self,
        frames: &[std::path::PathBuf],
        prompt: &str,
        model: &str,
    ) -> Result<String> {
        if frames.is_empty() {
            anyhow::bail!("frames slice is empty — at least one frame is required");
        }

        let mut content: Vec<serde_json::Value> = Vec::with_capacity(frames.len() + 1);

        for frame_path in frames {
            let item = Self::build_image_item(frame_path).await?;
            content.push(item);
        }
        content.push(serde_json::json!({ "type": "text", "text": prompt }));

        let payload = serde_json::json!({
            "model": model,
            "messages": [{ "role": "user", "content": content }],
            "max_tokens": 1024
        });

        let url = format!("{}/v1/chat/completions", self.base_url);
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await
            .context("HTTP request to OpenAI Vision API failed")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("failed to read OpenAI Vision API response body")?;

        if !status.is_success() {
            anyhow::bail!("OpenAI Vision API error {status}: {body}");
        }

        let parsed: ChatResponse =
            serde_json::from_str(&body).context("failed to parse OpenAI Vision API response")?;

        parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .context("OpenAI Vision API response contained no content")
    }

    /// Build a single `image_url` content item from a local image file.
    ///
    /// Reads the file, base64-encodes it, and wraps it in the `OpenAI`
    /// `image_url` object format.  Uses `mime_for_ext` for the MIME type; if
    /// the extension is missing falls back to `image/jpeg` (as produced by
    /// `extract_frames`).
    async fn build_image_item(path: &Path) -> Result<serde_json::Value> {
        let bytes = tokio::fs::read(path)
            .await
            .with_context(|| format!("cannot read frame file: {}", path.display()))?;

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
        let mime = Self::mime_for_ext(ext).unwrap_or("image/jpeg");

        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let data_url = format!("data:{mime};base64,{encoded}");

        Ok(serde_json::json!({
            "type": "image_url",
            "image_url": { "url": data_url, "detail": "auto" }
        }))
    }

    /// Analyse an image file using `OpenAI` Vision.
    ///
    /// Returns the model's text response.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, the format is unsupported,
    /// the HTTP request fails, or the API returns a non-2xx status code.
    pub async fn recognize(&self, image_path: &Path, prompt: &str, model: &str) -> Result<String> {
        let bytes = tokio::fs::read(image_path)
            .await
            .with_context(|| format!("cannot read image file: {}", image_path.display()))?;

        let ext = image_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let mime = Self::mime_for_ext(ext)?;
        let encoded = STANDARD.encode(&bytes);
        let data_url = format!("data:{mime};base64,{encoded}");

        let payload = serde_json::json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": data_url,
                            "detail": "auto"
                        }
                    },
                    {
                        "type": "text",
                        "text": prompt
                    }
                ]
            }],
            "max_tokens": 1024
        });

        let url = format!("{}/v1/chat/completions", self.base_url);
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await
            .context("HTTP request to OpenAI Vision API failed")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("failed to read OpenAI Vision API response body")?;

        if !status.is_success() {
            bail!("OpenAI Vision API error {status}: {body}");
        }

        // Extract choices[0].message.content
        let parsed: ChatResponse =
            serde_json::from_str(&body).context("failed to parse OpenAI Vision API response")?;

        parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .context("OpenAI Vision API response contained no content")
    }
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: Option<String>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    // Tests legitimately use `?` propagation and assert! — unwrap/expect
    // are acceptable in test context; silenced here.
    use super::*;
    use serde_json::Value;
    use tempfile::NamedTempFile;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// AC6: `mime_for_ext` accepts jpeg and png extensions.
    #[test]
    fn mime_for_jpeg_returns_image_jpeg() -> Result<()> {
        assert_eq!(VisionClient::mime_for_ext("jpeg")?, "image/jpeg");
        assert_eq!(VisionClient::mime_for_ext("jpg")?, "image/jpeg");
        Ok(())
    }

    #[test]
    fn mime_for_png_returns_image_png() -> Result<()> {
        assert_eq!(VisionClient::mime_for_ext("png")?, "image/png");
        Ok(())
    }

    #[test]
    fn mime_for_webp_returns_image_webp() -> Result<()> {
        assert_eq!(VisionClient::mime_for_ext("webp")?, "image/webp");
        Ok(())
    }

    #[test]
    fn mime_for_gif_returns_image_gif() -> Result<()> {
        assert_eq!(VisionClient::mime_for_ext("gif")?, "image/gif");
        Ok(())
    }

    /// AC6: HEIC returns explicit error message.
    #[test]
    fn heic_extension_returns_unsupported_err() {
        let result = VisionClient::mime_for_ext("heic");
        assert!(result.is_err(), "heic must return Err");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("HEIC"), "error must mention HEIC, got: {msg}");
        assert!(
            msg.contains("JPEG") || msg.contains("PNG"),
            "error must suggest JPEG or PNG, got: {msg}"
        );
    }

    #[test]
    fn unsupported_extension_returns_err() {
        let result = VisionClient::mime_for_ext("bmp");
        assert!(result.is_err(), "bmp must return Err");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("unsupported"),
            "error must say unsupported, got: {msg}"
        );
    }

    /// AC6: base64 round-trip — encode then decode returns original bytes.
    #[test]
    fn base64_roundtrip_small_png() -> Result<()> {
        let original = b"\x89PNG\r\n\x1a\nhello test bytes";
        let encoded = STANDARD.encode(original);
        let decoded = STANDARD.decode(&encoded).context("base64 decode failed")?;
        assert_eq!(decoded.as_slice(), original.as_slice());
        Ok(())
    }

    /// AC6: client construction succeeds.
    #[test]
    fn client_new_does_not_panic() -> Result<()> {
        VisionClient::new("dummy-api-key", "http://localhost:8080")?;
        Ok(())
    }

    // ── recognize_sequence tests (AC2, AC7) ───────────────────────────────

    /// AC2/AC7: `recognize_sequence` builds a payload with N `image_url` items
    /// (one per frame) followed by one `text` item; uses model `gpt-4o`.
    #[tokio::test]
    async fn recognize_sequence_builds_payload() -> Result<()> {
        let mock_server = MockServer::start().await;

        // Capture the request body so we can assert on payload shape.
        let response_body = serde_json::json!({
            "choices": [{"message": {"role": "assistant", "content": "test analysis"}}]
        });
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        // Create 3 minimal PNG temp files.
        let png_bytes: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xe2, 0x21, 0xbc,
            0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];

        let mut tmp_files: Vec<NamedTempFile> = Vec::new();
        let mut frame_paths: Vec<std::path::PathBuf> = Vec::new();
        for _ in 0..3 {
            let tmp = tempfile::Builder::new().suffix(".png").tempfile()?;
            std::fs::write(tmp.path(), png_bytes)?;
            frame_paths.push(tmp.path().to_path_buf());
            tmp_files.push(tmp);
        }

        let client = VisionClient::new("sk-test", mock_server.uri())?;
        let text = client
            .recognize_sequence(&frame_paths, "describe movement", "gpt-4o")
            .await?;

        assert_eq!(text, "test analysis");

        // Inspect the captured request to verify payload shape.
        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1, "expected exactly 1 request");

        let body: Value = serde_json::from_slice(&requests[0].body)?;
        assert_eq!(body["model"], "gpt-4o", "model must be gpt-4o");

        let content = body["messages"][0]["content"]
            .as_array()
            .expect("content must be array");

        // Should be 3 image_url items + 1 text item = 4 total
        assert_eq!(
            content.len(),
            4,
            "expected 4 content items (3 images + 1 text)"
        );

        let image_items: Vec<&Value> = content
            .iter()
            .filter(|item| item["type"] == "image_url")
            .collect();
        assert_eq!(image_items.len(), 3, "expected 3 image_url items");

        let text_items: Vec<&Value> = content
            .iter()
            .filter(|item| item["type"] == "text")
            .collect();
        assert_eq!(text_items.len(), 1, "expected 1 text item");
        assert_eq!(text_items[0]["text"], "describe movement");

        Ok(())
    }

    /// AC7 edge case: `recognize_sequence` with empty frames slice returns Err.
    #[tokio::test]
    async fn recognize_sequence_empty_frames() -> Result<()> {
        let client = VisionClient::new("sk-test", "http://localhost:19999")?;
        let result = client
            .recognize_sequence(&[], "describe movement", "gpt-4o")
            .await;
        assert!(result.is_err(), "empty frames slice must return Err");
        let msg = format!("{:#}", result.unwrap_err());
        assert!(
            msg.contains("frame") || msg.contains("empty"),
            "error must mention frames or empty, got: {msg}"
        );
        Ok(())
    }

    /// AC6: `recognize` returns Err for missing file.
    #[tokio::test]
    async fn recognize_missing_file_returns_err() -> Result<()> {
        let client = VisionClient::new("dummy-api-key", "http://localhost:9999")?;
        let result = client
            .recognize(
                Path::new("/tmp/this-file-does-not-exist-vision-recognizer.png"),
                "describe",
                "gpt-4o-mini",
            )
            .await;
        assert!(result.is_err(), "expected Err for missing file");
        let msg = format!("{:#}", result.unwrap_err());
        assert!(
            msg.contains("cannot read image file"),
            "error message must mention 'cannot read image file', got: {msg}"
        );
        Ok(())
    }
}
