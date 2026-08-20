//! The mediasoup signaling client.
//!
//! Thin typed wrappers over the `voice.*` tRPC router that the engine
//! calls. Each method maps to one tRPC mutation/query and returns the
//! decoded response. The opaque mediasoup JSON (DTLS/RTP parameters,
//! capabilities) is passed through as `serde_json::Value`.

use std::sync::Arc;

use serde_json::{Value, json};

use crate::error::CoreResult;
use crate::models::{RemoteProducerIds, StreamKind, TransportParams};
use crate::trpc::TrpcClient;
use crate::trpc::router;

/// Which transport a signaling call targets (producer/send or consumer/recv).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    /// Send side (`createProducerTransport`/`connectProducerTransport`).
    Producer,
    /// Recv side (`createConsumerTransport`/`connectConsumerTransport`).
    Consumer,
}

/// A typed mediasoup signaling client bound to a [`TrpcClient`].
pub struct SignalingClient {
    trpc: Arc<TrpcClient>,
}

impl SignalingClient {
    #[must_use]
    pub fn new(trpc: Arc<TrpcClient>) -> Self {
        Self { trpc }
    }

    /// `voice.join`: join a voice channel and get the router RTP capabilities.
    pub async fn join(
        &self,
        channel_id: i64,
        state: crate::models::VoiceUserState,
    ) -> CoreResult<Value> {
        self.trpc
            .mutation(
                router::voice::JOIN,
                Some(json!({ "channelId": channel_id, "state": state })),
            )
            .await
    }

    /// `voice.leave`: leave the current voice channel.
    pub async fn leave(&self) -> CoreResult<()> {
        self.trpc.mutation(router::voice::LEAVE, None).await?;
        Ok(())
    }

    /// `voice.updateState`: update mic/sound/webcam/screenshare state.
    pub async fn update_state(
        &self,
        mic_muted: Option<bool>,
        sound_muted: Option<bool>,
        webcam_enabled: Option<bool>,
        sharing_screen: Option<bool>,
    ) -> CoreResult<()> {
        self.trpc
            .mutation(
                router::voice::UPDATE_STATE,
                Some(json!({
                    "micMuted": mic_muted,
                    "soundMuted": sound_muted,
                    "webcamEnabled": webcam_enabled,
                    "sharingScreen": sharing_screen,
                })),
            )
            .await?;
        Ok(())
    }

    /// `voice.getProducers`: list remote producers in the channel.
    pub async fn get_producers(&self) -> CoreResult<RemoteProducerIds> {
        let v = self.trpc.query(router::voice::GET_PRODUCERS, None).await?;
        Ok(serde_json::from_value(v)?)
    }

    /// `voice.createProducerTransport` / `createConsumerTransport`.
    pub async fn create_transport(&self, kind: TransportKind) -> CoreResult<TransportParams> {
        let path = match kind {
            TransportKind::Producer => router::voice::CREATE_PRODUCER_TRANSPORT,
            TransportKind::Consumer => router::voice::CREATE_CONSUMER_TRANSPORT,
        };
        let v = self.trpc.mutation(path, None).await?;
        Ok(serde_json::from_value(v)?)
    }

    /// `voice.connectProducerTransport` / `connectConsumerTransport`.
    pub async fn connect_transport(
        &self,
        kind: TransportKind,
        dtls_parameters: Value,
    ) -> CoreResult<()> {
        let path = match kind {
            TransportKind::Producer => router::voice::CONNECT_PRODUCER_TRANSPORT,
            TransportKind::Consumer => router::voice::CONNECT_CONSUMER_TRANSPORT,
        };
        self.trpc
            .mutation(path, Some(json!({ "dtlsParameters": dtls_parameters })))
            .await?;
        Ok(())
    }

    /// `voice.produce`: produce a local track on the producer transport.
    pub async fn produce(
        &self,
        transport_id: &str,
        kind: StreamKind,
        rtp_parameters: Value,
        quality_layers: Option<Vec<Value>>,
    ) -> CoreResult<String> {
        let mut input = json!({
            "transportId": transport_id,
            "kind": kind,
            "rtpParameters": rtp_parameters,
        });
        if let Some(layers) = quality_layers {
            input["qualityLayers"] = Value::Array(layers);
        }
        let v = self
            .trpc
            .mutation(router::voice::PRODUCE, Some(input))
            .await?;
        Ok(serde_json::from_value::<String>(v).unwrap_or_default())
    }

    /// `voice.closeProducer`: stop producing a kind.
    pub async fn close_producer(&self, kind: StreamKind) -> CoreResult<()> {
        self.trpc
            .mutation(router::voice::CLOSE_PRODUCER, Some(json!({ "kind": kind })))
            .await?;
        Ok(())
    }

    /// `voice.consume`: consume a remote producer.
    pub async fn consume(
        &self,
        kind: StreamKind,
        remote_id: i64,
        rtp_capabilities: Value,
    ) -> CoreResult<router::voice::ConsumeOutput> {
        let v = self
            .trpc
            .mutation(
                router::voice::CONSUME,
                Some(json!({
                    "kind": kind,
                    "remoteId": remote_id,
                    "rtpCapabilities": rtp_capabilities,
                })),
            )
            .await?;
        Ok(serde_json::from_value(v)?)
    }

    /// `voice.setConsumerQuality`: pick a simulcast layer (simulcast only).
    pub async fn set_consumer_quality(
        &self,
        remote_id: i64,
        kind: StreamKind,
        quality: Value,
    ) -> CoreResult<()> {
        self.trpc
            .mutation(
                router::voice::SET_CONSUMER_QUALITY,
                Some(json!({
                    "remoteId": remote_id,
                    "kind": kind,
                    "quality": quality,
                })),
            )
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_kind_is_copy() {
        let a = TransportKind::Producer;
        let b = a;
        assert_eq!(a, b);
    }
}
