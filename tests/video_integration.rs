//! Integration tests for the video frame extraction pipeline.
//!
//! These tests exercise `video::extract_frames` + `VisionClient::recognize_sequence`
//! end-to-end against a wiremock mock of the `OpenAI` API.
//!
//! Tests that require `ffmpeg` call `skip_if_no_ffmpeg()` at the top of the
//! test body and return early if the binary is absent — so the whole test suite
//! remains green on machines without `ffmpeg` installed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use serde_json::json;
use std::path::{Path, PathBuf};
use vision_recognizer::{
    openai_vision::VisionClient,
    video::{self, DEFAULT_FPS, MAX_FRAMES},
};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

// ── Helpers ────────────────────────────────────────────────────────────────

/// Returns `true` (skip) when `ffprobe` is not found in `PATH`.
///
/// Call at the start of any test that invokes ffmpeg/ffprobe so the test
/// gracefully skips on machines without ffmpeg installed.
fn skip_if_no_ffmpeg() -> bool {
    std::process::Command::new("ffprobe")
        .arg("-version")
        .output()
        .map_or(true, |o| !o.status.success())
}

/// Generate a minimal 5-second test MP4 in `dir` using `ffmpeg`.
///
/// Uses the `lavfi` virtual input device with a solid-colour source so no
/// actual media file is required as fixture.
fn make_test_mp4(dir: &Path) -> Result<PathBuf> {
    let out = dir.join("test.mp4");
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:size=640x480:rate=25",
            "-t",
            "5",
            out.to_str().expect("UTF-8 path"),
            "-y",
            "-loglevel",
            "error",
        ])
        .status()?;
    anyhow::ensure!(
        status.success(),
        "ffmpeg exited non-zero generating test mp4"
    );
    Ok(out)
}

// ── Tests ──────────────────────────────────────────────────────────────────

/// AC6: `supported_video_ext` returns `false` for `.avi`.
///
/// Does not require `ffmpeg` — pure logic test.
#[test]
fn analyze_video_unsupported_ext_returns_false() {
    assert!(
        !video::supported_video_ext("avi"),
        "avi must not be a supported extension"
    );
}

/// AC2/AC8: Happy-path — generate a 5-sec mp4, extract frames, call
/// `recognize_sequence` against a wiremock mock, assert non-empty response.
#[tokio::test]
async fn analyze_video_happy_path() -> Result<()> {
    if skip_if_no_ffmpeg() {
        eprintln!("skip: ffmpeg not found in PATH — skipping analyze_video_happy_path");
        return Ok(());
    }

    // Create a temp dir for the source mp4 (separate from the frames temp dir).
    let src_dir = tempfile::TempDir::new()?;
    let mp4_path = make_test_mp4(src_dir.path())?;

    // Start wiremock server that returns a canned OpenAI response.
    let mock_server = MockServer::start().await;
    let response_body = json!({
        "choices": [{"message": {"role": "assistant", "content": "test analysis of blue frames"}}]
    });
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    // Extract frames.
    let (_tempdir, frames) = video::extract_frames(&mp4_path, DEFAULT_FPS, MAX_FRAMES).await?;
    assert!(!frames.is_empty(), "at least one frame must be extracted");
    assert!(
        frames.len() <= MAX_FRAMES as usize,
        "frame count must not exceed MAX_FRAMES"
    );

    // Call recognize_sequence against wiremock.
    let client = VisionClient::new("sk-test", mock_server.uri())?;
    let text = client
        .recognize_sequence(&frames, "describe movement", "gpt-4o")
        .await?;

    assert!(!text.is_empty(), "response text must not be empty");
    assert_eq!(text, "test analysis of blue frames");

    // Verify wiremock received at least 1 request.
    let reqs = mock_server
        .received_requests()
        .await
        .expect("wiremock requests");
    assert!(
        !reqs.is_empty(),
        "wiremock must have received at least 1 request"
    );

    Ok(())
}
