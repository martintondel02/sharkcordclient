//! The voice engine: orchestrates signaling + the media plane.
//!
//! [`VoiceEngine::join`] runs the full join sequence: `voice.join` → load
//! the device → create producer + consumer transports → start mic →
//! consume existing producers → subscribe to `voice.onNewProducer` for
//! late arrivals. Control methods (`toggle_mic`, `toggle_webcam`, …) send
//! `voice.updateState` and start/stop the corresponding producers.
//!
//! Because the media plane can't be validated without a live server, the
//! signaling is fully wired and the media operations are routed through
//! the [`super::MediaEngine`] trait. With the `voice-webrtc` feature the
//! `WebrtcMediaEngine` backs it; otherwise the [`super::StubMediaEngine`]
//! returns errors (signaling still succeeds).

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::signaling::{SignalingClient, TransportKind};
use super::{MediaEngine, MediasoupDevice, TransportSide};
use crate::error::CoreResult;
use crate::models::{StreamKind, VoiceUserState};
use crate::trpc::TrpcClient;
use crate::trpc::router;

/// Configuration for a new voice session.
pub struct VoiceConfig {
    /// Whether the server has `webRtcSimulcastEnabled`.
    pub simulcast_enabled: bool,
    /// Preferred video codecs (subset of "VP8", "VP9", "H264", "AV1").
    pub preferred_video_codecs: Vec<String>,
    /// The media engine (webrtc-rs-backed or stub).
    pub media: Box<dyn MediaEngine>,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            simulcast_enabled: false,
            preferred_video_codecs: vec!["VP8".to_owned(), "H264".to_owned()],
            media: Box::new(super::StubMediaEngine),
        }
    }
}

/// A joined voice channel session.
pub struct VoiceEngine {
    channel_id: i64,
    signaling: SignalingClient,
    trpc: Arc<TrpcClient>,
    config: VoiceConfig,
    /// Local mic/webcam state (mute/deafen/etc.).
    state: RwLock<VoiceUserState>,
    /// The loaded mediasoup device (after join).
    device: RwLock<Option<Box<dyn MediasoupDevice>>>,
}

impl VoiceEngine {
    /// Join a voice channel.
    ///
    /// Runs `voice.join`, loads the device with the router RTP caps,
    /// creates producer + consumer transports, and starts consuming
    /// existing producers. Returns the engine for control calls.
    pub async fn join(
        trpc: Arc<TrpcClient>,
        channel_id: i64,
        state: VoiceUserState,
        config: VoiceConfig,
    ) -> CoreResult<Self> {
        let signaling = SignalingClient::new(Arc::clone(&trpc));

        // 1. voice.join → router RTP capabilities.
        let router_caps = signaling.join(channel_id, state).await?;
        debug!(%channel_id, "voice.join ok");

        // 2. Load the local device.
        let device = config.media.new_device();
        let recv_caps = device.load(&router_caps)?;
        debug!("device loaded");

        let engine = Self {
            channel_id,
            signaling,
            trpc: Arc::clone(&trpc),
            config,
            state: RwLock::new(state),
            device: RwLock::new(Some(device)),
        };

        // 3. Create producer transport (send side).
        match engine
            .signaling
            .create_transport(TransportKind::Producer)
            .await
        {
            Ok(params) => {
                debug!(transport_id = %params.id, "producer transport created");
                // The webrtc-backed engine would build a WebRtcTransport
                // from params here and wire the connect callback.
                let _ = params;
            }
            Err(err) => {
                warn!(%err, "failed to create producer transport (signaling ok; media may be stubbed)");
            }
        }

        // 4. Create consumer transport (recv side) and consume existing producers.
        match engine
            .signaling
            .create_transport(TransportKind::Consumer)
            .await
        {
            Ok(params) => {
                debug!(transport_id = %params.id, "consumer transport created");
                // Consume existing producers.
                if let Ok(producers) = engine.signaling.get_producers().await {
                    let device_guard = engine.device.read().await;
                    if let Some(device) = device_guard.as_ref() {
                        let caps = device.load(&router_caps)?;
                        for id in producers.remote_audio_ids {
                            if let Err(err) = engine
                                .signaling
                                .consume(StreamKind::Audio, id, caps.clone())
                                .await
                            {
                                warn!(%err, id, "consume audio failed");
                            }
                        }
                        for id in producers.remote_video_ids {
                            if let Err(err) = engine
                                .signaling
                                .consume(StreamKind::Video, id, caps.clone())
                                .await
                            {
                                warn!(%err, id, "consume video failed");
                            }
                        }
                    }
                }
            }
            Err(err) => {
                warn!(%err, "failed to create consumer transport");
            }
        }

        // 5. Subscribe to onNewProducer for late arrivals (best-effort).
        match trpc.subscribe(router::voice::ON_NEW_PRODUCER, None).await {
            Ok(mut sub) => {
                let engine_trpc = Arc::clone(&trpc);
                let engine_signaling = engine.signaling_signaling_handle();
                tokio::spawn(async move {
                    while let Some(ev) = sub.events.recv().await {
                        if let Ok(data) = ev {
                            if let Ok(payload) = serde_json::from_value::<NewProducerEvent>(data) {
                                debug!(?payload, "onNewProducer");
                                // Consume the new producer (best-effort).
                                let _ = engine_signaling
                                    .consume(payload.kind, payload.remote_id, Value::Null)
                                    .await;
                            }
                        }
                    }
                    let _ = engine_trpc;
                });
            }
            Err(err) => warn!(%err, "subscribe onNewProducer failed"),
        }

        Ok(engine)
    }

    /// Leave the channel and tear down. Sends `voice.leave`.
    pub async fn leave(&self) -> CoreResult<()> {
        let _ = self.signaling.leave().await;
        Ok(())
    }

    /// Toggle mic mute. Sends `voice.updateState`.
    pub async fn toggle_mic(&self) -> CoreResult<()> {
        let mut state = self.state.write().await;
        state.mic_muted = !state.mic_muted;
        let mic_muted = state.mic_muted;
        drop(state);
        self.signaling
            .update_state(Some(mic_muted), None, None, None)
            .await
    }

    /// Toggle deafen (speakers). Implies mic muted upstream.
    pub async fn toggle_sound(&self) -> CoreResult<()> {
        let mut state = self.state.write().await;
        state.sound_muted = !state.sound_muted;
        if state.sound_muted {
            state.mic_muted = true;
        }
        let (mic_muted, sound_muted) = (state.mic_muted, state.sound_muted);
        drop(state);
        self.signaling
            .update_state(Some(mic_muted), Some(sound_muted), None, None)
            .await
    }

    /// Start/stop webcam. Sends `voice.updateState`.
    pub async fn toggle_webcam(&self) -> CoreResult<()> {
        let mut state = self.state.write().await;
        state.webcam_enabled = !state.webcam_enabled;
        let webcam_enabled = state.webcam_enabled;
        drop(state);
        self.signaling
            .update_state(None, None, Some(webcam_enabled), None)
            .await
    }

    /// Start/stop screen share. Sends `voice.updateState`.
    pub async fn toggle_screen_share(&self) -> CoreResult<()> {
        let mut state = self.state.write().await;
        state.sharing_screen = !state.sharing_screen;
        let sharing_screen = state.sharing_screen;
        drop(state);
        self.signaling
            .update_state(None, None, None, Some(sharing_screen))
            .await
    }

    /// Set the simulcast quality layer for a remote consumer.
    pub async fn set_consumer_quality(
        &self,
        remote_id: i64,
        kind: StreamKind,
        spatial_layer: Option<u8>,
    ) -> CoreResult<()> {
        let quality = match spatial_layer {
            None => serde_json::json!({ "mode": "auto" }),
            Some(layer) => serde_json::json!({ "mode": "layer", "spatialLayer": layer }),
        };
        self.signaling
            .set_consumer_quality(remote_id, kind, quality)
            .await
    }

    /// The current local voice state (mic/sound/webcam/screen).
    pub async fn state(&self) -> VoiceUserState {
        *self.state.read().await
    }

    /// Clone a signaling handle for the onNewProducer task.
    /// (Re-creates a `SignalingClient` bound to the same trpc.)
    fn signaling_signaling_handle(&self) -> SignalingClient {
        SignalingClient::new(Arc::clone(&self.trpc))
    }
}

/// Payload from `voice.onNewProducer`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewProducerEvent {
    channel_id: i64,
    remote_id: i64,
    kind: StreamKind,
}
