//! The mediasoup WebRTC voice engine.
//!
//! Sharkcord runs one mediasoup `Router` per voice channel; signaling
//! rides the `voice.*` tRPC router over the same WebSocket as everything
//! else, and the media plane is ICE/DTLS/SRTP on a separate UDP/TCP port
//! (default `40000`). This module ports the official web client's
//! `VoiceProvider` + `use-transports` logic to Rust:
//!
//! 1. `voice.join` → `routerRtpCapabilities`; load a local `Device`.
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
//! Audio processing uses WebRTC's built-in noise suppression / AGC / AEC
//! (the "standard" path the web client already supports).
//!
//! The signaling layer is fully implemented and testable against the
//! typed `voice` tRPC router. The media plane is abstracted behind the
//! [`MediaEngine`] trait so it can be backed by `webrtc` (webrtc-rs) or
//! stubbed when building without the `voice-webrtc` feature.

pub mod engine;
pub mod signaling;

pub use engine::{VoiceConfig, VoiceEngine};
pub use signaling::{SignalingClient, TransportKind};

use crate::error::CoreResult;
use crate::models::{StreamKind, TransportParams};

/// Which side a mediasoup transport serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportSide {
    /// Sending media to the server (producer).
    Send,
    /// Receiving media from the server (consumer).
    Recv,
}

/// A mediasoup device the media engine loads with the server's router
/// RTP capabilities, then uses to create send/recv transports.
pub trait MediasoupDevice: Send + Sync {
    /// Load the router RTP capabilities returned by `voice.join`. Returns
    /// the local receive RTP capabilities to use in `voice.consume`.
    fn load(&self, router_rtp_capabilities: &serde_json::Value) -> CoreResult<serde_json::Value>;

    /// Whether the device can produce the given kind.
    fn can_produce(&self, kind: StreamKind) -> bool;
}

/// A mediasoup WebRTC transport the media engine creates from server
/// `TransportParams` and uses to produce/consume media.
pub trait MediasoupTransport: Send {
    /// The server-assigned transport id.
    fn id(&self) -> &str;

    /// Wire the DTLS connect callback: when ICE connects, send our local
    /// DTLS parameters to the server via `connectProducerTransport`/
    /// `connectConsumerTransport`.
    fn on_connect(&mut self, callback: Box<dyn Fn(serde_json::Value) + Send + 'static>);

    /// The side this transport serves.
    fn side(&self) -> TransportSide;
}

/// The media-plane abstraction: creates mediasoup devices and transports.
/// Backed by `webrtc` (webrtc-rs) when the `voice-webrtc` feature is on;
/// stubbed otherwise (for text-only builds).
pub trait MediaEngine: Send + Sync {
    /// Create a fresh mediasoup device.
    fn new_device(&self) -> Box<dyn MediasoupDevice>;

    /// Create a `WebRtcTransport` from the server's `TransportParams`.
    fn create_transport(
        &self,
        params: TransportParams,
        side: TransportSide,
    ) -> CoreResult<Box<dyn MediasoupTransport>>;
}

/// A no-op media engine used when the `voice-webrtc` feature is off (or
/// for tests). All operations return errors; the signaling layer still
/// works but no media flows.
pub struct StubMediaEngine;

impl MediaEngine for StubMediaEngine {
    fn new_device(&self) -> Box<dyn MediasoupDevice> {
        Box::new(StubDevice)
    }
    fn create_transport(
        &self,
        _params: TransportParams,
        _side: TransportSide,
    ) -> CoreResult<Box<dyn MediasoupTransport>> {
        Err(crate::CoreError::Voice(
            "media engine is stubbed (build without voice-webrtc feature or no MediaEngine set)"
                .into(),
        ))
    }
}

/// A no-op mediasoup device (stub).
struct StubDevice;

impl MediasoupDevice for StubDevice {
    fn load(&self, _caps: &serde_json::Value) -> CoreResult<serde_json::Value> {
        Err(crate::CoreError::Voice("stub device cannot load".into()))
    }
    fn can_produce(&self, _kind: StreamKind) -> bool {
        false
    }
}

/// Re-exported voice user state for convenience.
pub use crate::models::VoiceUserState;
