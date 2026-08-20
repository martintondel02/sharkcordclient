//! The FFI facade exposed to the Kotlin (UniFFI) and Slint front-ends.
//!
//! This is the high-level surface the UIs call: `connect`, `send_message`,
//! `join_voice`, `toggle_mic`, etc. It owns the core's async runtime,
//! the tRPC client, the state store, and the voice engine, and emits
//! change notifications back to the UI via a callback interface.
//!
//! UniFFI bindings (`uniffi` crate) and the Slint-facing API object are
//! wired in Phase 1 (text) and Phase 2 (voice). _Stub._

// Stub methods are `async` to fix their final signatures now, but don't
// await anything yet. Remove this allow as Phase 1/2 fills them in.
#![allow(clippy::unused_async)]

use crate::error::CoreResult;

/// Callbacks the UI implements to receive state-change notifications.
/// Each method is fired on a core-owned thread; the UI must marshal back
/// to its own UI thread.
pub trait ClientObserver: Send + Sync {
    /// Fired once the session is established and the initial state is
    /// hydrated. The UI can now render the server view.
    fn on_connected(&self) {}

    /// Fired when the WS drops. `code` is the close code (e.g. `1006`,
    /// `40000` kicked, `40001` banned, `40002` server shutdown).
    fn on_disconnected(&self, _code: u16, _reason: String) {}

    /// Coarse "something in the state changed" notification. The UI
    /// re-reads what it needs from the exposed snapshot. Kept coarse to
    /// avoid per-event IPC chatter.
    fn on_state_changed(&self) {}

    /// A new message arrived in `channel_id`.
    fn on_message_new(&self, _channel_id: i64) {}

    /// A voice participant state changed in `channel_id`.
    fn on_voice_state(&self, _channel_id: i64) {}
}

/// The high-level client facade. _Stub — methods land in Phase 1/2._
pub struct Client;

impl Client {
    /// Connect to a Sharkcord server and run the bootstrap.
    /// _Stub._
    pub async fn connect(
        &self,
        _origin: &str,
        _identity: &str,
        _password: &str,
        _invite: Option<&str>,
        _server_password: Option<&str>,
        _observer: Box<dyn ClientObserver>,
    ) -> CoreResult<()> {
        Err(crate::CoreError::Other(
            "Client::connect not yet implemented (Phase 1)".to_owned(),
        ))
    }

    /// Disconnect and release all resources. _Stub._
    pub async fn disconnect(&self) -> CoreResult<()> {
        Ok(())
    }
}
