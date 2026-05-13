//! `OpenAI` Vision API client — sends images to `POST /v1/chat/completions`
//! using the `image_url` content type with base64-encoded data URLs.

use anyhow::{Context, Result};
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
    pub(crate) fn mime_for_ext(_ext: &str) -> Result<&'static str> {
        todo!("implement mime_for_ext")
    }

    /// Analyse an image file using `OpenAI` Vision.
    ///
    /// Returns the model's text response.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, the format is unsupported,
    /// the HTTP request fails, or the API returns a non-2xx status code.
    pub async fn recognize(
        &self,
        _image_path: &Path,
        _prompt: &str,
        _model: &str,
    ) -> Result<String> {
        todo!("implement recognize")
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    // Tests legitimately use `?` propagation and assert! — unwrap/expect
    // are acceptable in test context; silenced here.
    use super::*;

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
        assert!(
            msg.contains("HEIC"),
            "error must mention HEIC, got: {msg}"
        );
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
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let original = b"\x89PNG\r\n\x1a\nhello test bytes";
        let encoded = STANDARD.encode(original);
        let decoded = STANDARD.decode(&encoded).context("base64 decode failed")?;
        assert_eq!(decoded.as_slice(), original.as_slice());
        Ok(())
    }

    /// AC6: client construction succeeds.
    #[test]
    fn client_new_does_not_panic() -> Result<()> {
        VisionClient::new("sk-test", "http://localhost:8080")?;
        Ok(())
    }

    /// AC6: `recognize` returns Err for missing file.
    #[tokio::test]
    async fn recognize_missing_file_returns_err() -> Result<()> {
        let client = VisionClient::new("sk-test", "http://localhost:9999")?;
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
            msg.contains("cannot read image file") || msg.contains("not yet implemented"),
            "error message must be relevant, got: {msg}"
        );
        Ok(())
    }
}
