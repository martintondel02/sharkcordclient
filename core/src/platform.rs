//! Platform media capture abstraction.
//!
//! The voice engine ([`crate::voice`]) needs mic, webcam, and
//! screen-share media. Capture is inherently platform-specific, so this
//! module defines a trait the core consumes, and each UI target provides
//! an implementation:
//!
//! - **Android** — `AudioRecord` (mic), `CameraX` (webcam),
//!   `MediaProjection` (screen +, on newer Android, system audio).
//! - **Linux** — PipeWire (screen + system audio) and PulseAudio/PipeWire
//!   (mic).
//! - **Windows** — Windows Graphics Capture (screen) and WASAPI
//!   (mic + system-audio loopback).
//!
//! Implementations feed raw frames to the core via this trait; the
//! voice engine packs them into RTP and sends through the mediasoup
//! transport. _Stub; trait finalized in Phase 2._

use crate::error::CoreResult;
use crate::models::StreamKind;
use async_trait::async_trait;

/// A live captured media track the voice engine can produce.
#[async_trait]
pub trait CaptureTrack: Send {
    /// The stream kind this track carries.
    fn kind(&self) -> StreamKind;

    /// Pull the next raw frame (e.g. Opus packet, or raw video frame).
    /// Blocking; returns `None` when the track ended.
    async fn next_frame(&mut self) -> CoreResult<Option<Vec<u8>>>;

    /// Stop capture and release platform resources.
    async fn stop(&mut self) -> CoreResult<()>;

    /// Whether this track is currently muted/disabled (no frames
    /// emitted). Toggled by the voice controls.
    fn muted(&self) -> bool;
    /// Mute or unmute.
    fn set_muted(&mut self, muted: bool);
}

/// Factory for platform capture sources.
#[async_trait]
pub trait PlatformCapture: Send + Sync {
    /// Open the default (or configured) microphone.
    async fn open_mic(&self) -> CoreResult<Box<dyn CaptureTrack>>;

    /// Open the configured webcam.
    async fn open_webcam(&self) -> CoreResult<Box<dyn CaptureTrack>>;

    /// Open screen-share video. On desktop this prompts the user to pick
    /// a display/window; on Android it launches `MediaProjection`.
    async fn open_screen(&self) -> CoreResult<Box<dyn CaptureTrack>>;

    /// Open system-audio capture for screen-share, when supported.
    /// Returns `Err` on platforms/versions without system-audio capture.
    async fn open_screen_audio(&self) -> CoreResult<Box<dyn CaptureTrack>>;
}
