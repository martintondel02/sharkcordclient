//! The mediasoup WebRTC voice engine.
//!
//! Sharkcord runs one mediasoup `Router` per voice channel; signaling
//! rides the `voice.*` tRPC router over the same WebSocket as everything
//! else, and the media plane is ICE/DTLS/SRTP on a separate UDP/TCP port
//! (default `40000`). This module ports the official web client's
//! `VoiceProvider` + `use-transports` logic to Rust:
//!
//! 1. `voice.join` → `routerRtpCapabilities`; load a local mediasoup
//!    `Device`.
//! 2. `voice.createProducerTransport` → server `TransportParams`; build a
//!    local `WebRtcTransport` (ICE + DTLS + SRTP sender).
//! 3. Capture local tracks via [`crate::platform::PlatformCapture`] and
//!    `voice.produce` them (mic, webcam, screen, screen-audio), with
//!    simulcast encodings for video when enabled.
//! 4. `voice.createConsumerTransport` + `voice.getProducers` /
//!    `voice.onNewProducer` → `voice.consume` → spawn `Consumer`s and
//!    hand decoded frames to the platform renderer.
//! 5. `voice.updateState` for mute/deafen/webcam/screenshare;
//!    `voice.setConsumerQuality` for simulcast layer selection.
//! 6. Cleanup on `voice.leave` / channel change / WS close.
//!
//! Audio processing relies on WebRTC's built-in noise suppression / AGC /
//! AEC (the "standard" path the web client already supports). The
//! RNNoise/DTLN/AudioWorklet DSP paths are out of scope for v1.
//!
//! _Stub: the engine struct and signaling flow land in Phase 2._

// Stub methods are `async` to fix their final signatures now, but don't
// await anything yet. Remove this allow as Phase 2 fills them in.
#![allow(clippy::unused_async)]

use crate::error::CoreResult;
use crate::models::{StreamKind, TransportParams, VoiceUserState};
use crate::platform::PlatformCapture;

/// A joined voice channel session.
pub struct VoiceEngine {
    /// The channel this engine is bound to.
    pub channel_id: i64,
    // Phase 2 fields: mediasoup Device, producer/consumer transports,
    // local producers, remote consumers, platform capture handles.
}

/// Configuration for a new voice session.
pub struct VoiceConfig {
    /// Whether the server has `webRtcSimulcastEnabled` (controls whether
    /// we send `qualityLayers` on `produce` and call `setConsumerQuality`).
    pub simulcast_enabled: bool,
    /// Preferred codecs (subset of Opus, VP8, VP9, H264, AV1).
    pub preferred_video_codecs: Vec<String>,
    /// Platform capture source for mic/webcam/screen.
    pub capture: Box<dyn PlatformCapture>,
}

impl VoiceEngine {
    /// Join a voice channel. Calls `voice.join`, loads the mediasoup
    /// device, creates producer + consumer transports, starts mic
    /// capture, and consumes existing producers.
    ///
    /// _Stub: implemented in Phase 2._
    pub async fn join(
        _channel_id: i64,
        _state: VoiceUserState,
        _cfg: VoiceConfig,
    ) -> CoreResult<Self> {
        Err(crate::CoreError::Voice(
            "VoiceEngine::join not yet implemented (Phase 2)".to_owned(),
        ))
    }

    /// Leave the channel and tear down all transports/producers/consumers.
    /// _Stub._
    pub async fn leave(self) -> CoreResult<()> {
        Err(crate::CoreError::Voice(
            "VoiceEngine::leave not yet implemented (Phase 2)".to_owned(),
        ))
    }

    /// Toggle mic mute (also sends `voice.updateState`).
    /// _Stub._
    pub async fn toggle_mic(&mut self) -> CoreResult<()> {
        Err(crate::CoreError::Voice(
            "toggle_mic not yet implemented (Phase 2)".to_owned(),
        ))
    }

    /// Start or stop webcam producer.
    /// _Stub._
    pub async fn toggle_webcam(&mut self) -> CoreResult<()> {
        Err(crate::CoreError::Voice(
            "toggle_webcam not yet implemented (Phase 2)".to_owned(),
        ))
    }

    /// Start or stop screen-share (video + optional system audio).
    /// _Stub._
    pub async fn toggle_screen_share(&mut self) -> CoreResult<()> {
        Err(crate::CoreError::Voice(
            "toggle_screen_share not yet implemented (Phase 2)".to_owned(),
        ))
    }

    /// Set the simulcast quality layer for a remote video/screen consumer.
    /// _Stub._
    pub async fn set_consumer_quality(
        &mut self,
        _remote_id: i64,
        _kind: StreamKind,
        _spatial_layer: Option<u8>,
    ) -> CoreResult<()> {
        Err(crate::CoreError::Voice(
            "set_consumer_quality not yet implemented (Phase 2)".to_owned(),
        ))
    }

    /// Opaque handle to the server's transport params (Phase 2).
    #[must_use]
    pub fn placeholder() -> TransportParams {
        TransportParams {
            id: String::new(),
            ice_parameters: serde_json::Value::Null,
            ice_candidates: serde_json::Value::Null,
            dtls_parameters: serde_json::Value::Null,
        }
    }
}
