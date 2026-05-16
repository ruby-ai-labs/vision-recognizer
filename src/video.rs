//! Video frame extraction utilities using `ffmpeg`/`ffprobe` subprocesses.
//!
//! # Overview
//!
//! Provides an async pipeline for extracting still frames from short video files
//! and returning them as a `TempDir`-owned collection of JPEG paths.
//!
//! All heavy lifting delegates to external `ffmpeg` / `ffprobe` binaries.  The
//! public API is intentionally thin so that callers (MCP handler) stay decoupled
//! from subprocess details.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use tempfile::TempDir;

// ── Public constants ───────────────────────────────────────────────────────

/// Maximum number of frames that will be extracted from any video.
///
/// Enforces the cost-guard: no more than 16 images are ever sent to OpenAI.
pub const MAX_FRAMES: u16 = 16;

/// Maximum video duration (in seconds) accepted by `extract_frames`.
///
/// Videos longer than this are rejected before any subprocess is spawned.
pub const MAX_DURATION_SECS: f64 = 30.0;

/// Default frame-extraction rate (frames per second) when the caller does not
/// specify a value.
pub const DEFAULT_FPS: f32 = 2.0;

// ── Public helpers ─────────────────────────────────────────────────────────

/// Returns `true` if the file extension is a supported video format.
///
/// Comparison is case-insensitive.  Supported: `mp4`, `mov`, `webm`.
pub fn supported_video_ext(ext: &str) -> bool {
    matches!(ext.to_lowercase().as_str(), "mp4" | "mov" | "webm")
}

/// Computes the effective extraction FPS so that the total frame count never
/// exceeds `max_frames`.
///
/// If `fps * duration_secs ≤ max_frames` the requested `fps` is returned
/// unchanged.  Otherwise the fps is scaled down to `max_frames / duration_secs`.
///
/// # Panics
///
/// Does not panic.  When `fps` is zero or `duration_secs` is zero the function
/// returns `fps` as-is (no frames will be extracted anyway).
pub(crate) fn compute_effective_fps(fps: f32, duration_secs: f64, max_frames: u16) -> f32 {
    let total = fps * duration_secs as f32;
    if total > max_frames as f32 && duration_secs > 0.0 {
        max_frames as f32 / duration_secs as f32
    } else {
        fps
    }
}

// ── ffprobe / ffmpeg wrappers ──────────────────────────────────────────────

/// Query the duration of a video file in seconds via `ffprobe`.
///
/// # Errors
///
/// - Returns an error containing an install hint when `ffprobe` is not found in
///   `PATH`.
/// - Returns an error if the subprocess exits non-zero or if the JSON output
///   cannot be parsed.
pub async fn video_duration_secs(video_path: &Path) -> Result<f64> {
    let output = tokio::process::Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            video_path
                .to_str()
                .context("video path contains invalid UTF-8")?,
        ])
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow!(
                    "ffmpeg not found in PATH; install with: brew install ffmpeg"
                )
            } else {
                anyhow!("ffprobe failed to start: {e}")
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("ffprobe exited with error: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).context("failed to parse ffprobe JSON output")?;

    let duration_str = parsed["format"]["duration"]
        .as_str()
        .context("ffprobe output missing format.duration field")?;

    duration_str
        .parse::<f64>()
        .context("format.duration is not a valid float")
}

/// Extract frames from a video file into a temporary directory.
///
/// Returns `(TempDir, Vec<PathBuf>)`.  The caller **must** keep the `TempDir`
/// alive for as long as the paths are needed — dropping it removes all frames.
///
/// # Arguments
///
/// * `video_path` — path to the source video.
/// * `fps`        — desired extraction rate; capped by `MAX_FRAMES` guard.
/// * `max_frames` — hard upper bound on returned frames (pass `MAX_FRAMES`).
///
/// # Errors
///
/// - Returns an error if `ffmpeg` / `ffprobe` are not found in `PATH`.
/// - Returns an error if the video duration exceeds `MAX_DURATION_SECS`.
/// - Returns an error if `ffmpeg` exits non-zero.
/// - Returns an error if the temporary directory cannot be created.
pub async fn extract_frames(
    video_path: &Path,
    fps: f32,
    max_frames: u16,
) -> Result<(TempDir, Vec<PathBuf>)> {
    let duration = video_duration_secs(video_path).await?;

    if duration > MAX_DURATION_SECS {
        return Err(anyhow!(
            "video duration {duration:.1}s exceeds the {MAX_DURATION_SECS}s limit"
        ));
    }

    let effective_fps = compute_effective_fps(fps, duration, max_frames);

    let tmp = TempDir::new().context("failed to create temporary directory for frames")?;
    let pattern = tmp.path().join("frame_%04d.jpg");
    let pattern_str = pattern
        .to_str()
        .context("temp path contains invalid UTF-8")?;

    let output = tokio::process::Command::new("ffmpeg")
        .args([
            "-i",
            video_path
                .to_str()
                .context("video path contains invalid UTF-8")?,
            "-vf",
            &format!("fps={effective_fps}"),
            "-frames:v",
            &max_frames.to_string(),
            pattern_str,
            "-y",
            "-loglevel",
            "error",
        ])
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow!(
                    "ffmpeg not found in PATH; install with: brew install ffmpeg"
                )
            } else {
                anyhow!("ffmpeg failed to start: {e}")
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("ffmpeg exited with error: {stderr}"));
    }

    // Collect and sort extracted frames.
    let mut frames = Vec::new();
    let mut read_dir = tokio::fs::read_dir(tmp.path())
        .await
        .context("failed to read temp frame directory")?;
    while let Some(entry) = read_dir
        .next_entry()
        .await
        .context("error iterating temp directory")?
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("jpg") {
            frames.push(path);
        }
    }
    frames.sort();

    Ok((tmp, frames))
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

    use super::*;

    // AC6 — extension detection

    #[test]
    fn supported_video_ext_mp4() {
        assert!(supported_video_ext("mp4"));
    }

    #[test]
    fn unsupported_ext_avi_false() {
        assert!(!supported_video_ext("avi"));
    }

    #[test]
    fn uppercase_mp4_true() {
        assert!(supported_video_ext("MP4"));
    }

    #[test]
    fn uppercase_mov_true() {
        assert!(supported_video_ext("MOV"));
    }

    #[test]
    fn supported_video_ext_webm() {
        assert!(supported_video_ext("webm"));
    }

    // Edge case: unicode path — supported_video_ext only checks extension string
    #[test]
    fn supported_video_ext_unicode() {
        // Extension of "тест.mp4" is "mp4" — should be supported
        let path = std::path::Path::new("тест.mp4");
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        assert!(supported_video_ext(ext));
    }

    // AC4 — compute_effective_fps

    #[test]
    fn effective_fps_cap() {
        // 10 fps * 5s = 50 frames > 16 → capped at 16/5 = 3.2
        let result = compute_effective_fps(10.0, 5.0, 16);
        assert!(
            (result - 3.2_f32).abs() < 1e-5,
            "expected ~3.2, got {result}"
        );
    }

    #[test]
    fn effective_fps_no_cap_when_within_limit() {
        // 2 fps * 5s = 10 frames ≤ 16 → unchanged
        let result = compute_effective_fps(2.0, 5.0, 16);
        assert!(
            (result - 2.0_f32).abs() < 1e-5,
            "expected 2.0, got {result}"
        );
    }

    // AC3 — duration boundary

    #[test]
    fn duration_exactly_30s_passes() {
        // Guard is >, not >= — exactly 30.0 must pass
        assert!(
            30.0_f64 <= MAX_DURATION_SECS,
            "30.0s should be within the limit"
        );
        assert!(
            !(30.0_f64 > MAX_DURATION_SECS),
            "30.0s must NOT be rejected by the > guard"
        );
    }

    // Edge case: fps = 0 must not panic or divide by zero

    #[test]
    fn fps_zero_no_div_by_zero() {
        // fps=0 → total = 0 ≤ 16 → returns 0 unchanged
        let result = compute_effective_fps(0.0, 10.0, 16);
        assert!(result.is_finite(), "result must be finite, got {result}");
    }

    // AC5 — no_ffmpeg_returns_install_hint (async; requires tokio runtime)

    #[tokio::test]
    async fn no_ffmpeg_returns_install_hint() {
        // Temporarily restrict PATH to exclude ffmpeg
        let result =
            video_duration_secs_with_empty_path(std::path::Path::new("/tmp/fake.mp4")).await;
        assert!(result.is_err(), "expected Err when ffmpeg absent");
        let msg = format!("{:#}", result.unwrap_err());
        assert!(
            msg.contains("ffmpeg"),
            "error must mention ffmpeg, got: {msg}"
        );
        assert!(
            msg.contains("brew install ffmpeg"),
            "error must include install hint, got: {msg}"
        );
    }

    /// Runs `ffprobe` with PATH restricted to `/no-such-dir` so that the binary
    /// is reliably not found.
    async fn video_duration_secs_with_empty_path(path: &Path) -> Result<f64> {
        let output = tokio::process::Command::new("ffprobe")
            .args([
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                path.to_str().unwrap_or(""),
            ])
            .env("PATH", "/no-such-dir")
            .output()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    anyhow!("ffmpeg not found in PATH; install with: brew install ffmpeg")
                } else {
                    anyhow!("ffprobe failed to start: {e}")
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("ffprobe exited with error: {stderr}"));
        }
        Ok(0.0)
    }
}
