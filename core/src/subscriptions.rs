//! The subscription runner.
//!
//! Subscribes to all the realtime tRPC subscriptions the UI needs for
//! text/voice presence and applies their events to the shared state
//! store. Emits coarse [`state::ChangeSet`] notifications through the
//! provided callback so the UI can re-render.
//!
//! Each subscription runs in its own task; the runner keeps handles to
//! all of them and aborts on [`Subscriptions::stop`].

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::error::CoreResult;
use crate::ffi::ClientObserver;
use crate::state::{ChangeSet, SharedState};
use crate::trpc::TrpcClient;
use crate::trpc::router;

/// Owns all live subscription tasks.
pub struct Subscriptions {
    handles: Vec<JoinHandle<()>>,
}

impl Subscriptions {
    /// Start all subscriptions against `client`, applying events to
    /// `state` and notifying `observer` on change. Returns a handle whose
    /// [`stop`](Self::stop) aborts all of them.
    pub fn start(
        client: &Arc<TrpcClient>,
        state: &SharedState,
        observer: &Arc<dyn ClientObserver>,
    ) -> Self {
        let mut handles = Vec::new();

        // Helper to spawn a subscription task.
        macro_rules! spawn_sub {
            ($path:expr) => {{
                let client = Arc::clone(client);
                let state = Arc::clone(state);
                let observer = Arc::clone(observer);
                handles.push(tokio::spawn(async move {
                    run_sub(&client, &state, &observer, $path).await;
                }));
            }};
        }

        spawn_sub!(router::messages::ON_NEW);
        spawn_sub!(router::messages::ON_UPDATE);
        spawn_sub!(router::messages::ON_DELETE);
        spawn_sub!(router::messages::ON_TYPING);
        spawn_sub!(router::channels::ON_CREATE);
        spawn_sub!(router::channels::ON_UPDATE);
        spawn_sub!(router::channels::ON_DELETE);
        spawn_sub!(router::users::ON_JOIN);
        spawn_sub!(router::users::ON_LEAVE);
        spawn_sub!(router::users::ON_UPDATE);
        spawn_sub!(router::users::ON_CREATE);
        spawn_sub!(router::users::ON_DELETE);
        spawn_sub!(router::voice::ON_JOIN);
        spawn_sub!(router::voice::ON_LEAVE);
        spawn_sub!(router::voice::ON_UPDATE_STATE);

        Self { handles }
    }

    /// Stop all subscriptions (aborts the tasks).
    pub fn stop(&mut self) {
        for h in self.handles.drain(..) {
            h.abort();
        }
    }
}

/// Run one subscription forever: (re)subscribe, apply events to the
/// state, notify the observer. Reconnects on error after a short delay.
async fn run_sub(
    client: &Arc<TrpcClient>,
    state: &SharedState,
    observer: &Arc<dyn ClientObserver>,
    path: &'static str,
) {
    loop {
        match client.subscribe(path, None).await {
            Ok(mut sub) => {
                while let Some(ev) = sub.events.recv().await {
                    match ev {
                        Ok(data) => {
                            let change = apply_event(state, path, data).await;
                            if !is_empty(change) {
                                emit(observer, change, path);
                            }
                        }
                        Err(err) => {
                            warn!(%path, %err, "subscription event error");
                        }
                    }
                }
                // Channel closed: subscription ended. Reconnect.
                warn!(%path, "subscription closed; reconnecting");
            }
            Err(err) => {
                warn!(%path, %err, "subscribe failed; retrying");
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

/// Apply one event payload to the state, returning what changed.
async fn apply_event(state: &SharedState, path: &str, data: serde_json::Value) -> ChangeSet {
    let mut s = state.write().await;
    match path {
        router::messages::ON_NEW => decode_or(&data, path, |m: crate::models::Message| {
            s.apply_message_new(m)
        }),
        router::messages::ON_UPDATE => decode_or(&data, path, |m: crate::models::Message| {
            s.apply_message_update(m)
        }),
        router::messages::ON_DELETE => {
            decode_or(&data, path, |e: crate::state::MessageDeleteEvent| {
                s.apply_message_delete(e.channel_id, e.message_id)
            })
        }
        router::messages::ON_TYPING => decode_or(&data, path, |e: crate::state::TypingEvent| {
            s.apply_typing(e.channel_id, e.user_id)
        }),
        router::channels::ON_CREATE => decode_or(&data, path, |c: crate::models::Channel| {
            s.apply_channel_create(c)
        }),
        router::channels::ON_UPDATE => decode_or(&data, path, |c: crate::models::Channel| {
            s.apply_channel_update(c)
        }),
        router::channels::ON_DELETE => decode_or(&data, path, |id: i64| s.apply_channel_delete(id)),
        router::users::ON_JOIN | router::users::ON_CREATE => {
            decode_or(&data, path, |u: crate::models::PublicUser| {
                s.apply_user_join(u)
            })
        }
        router::users::ON_UPDATE => decode_or(&data, path, |u: crate::models::PublicUser| {
            s.apply_user_update(u)
        }),
        router::users::ON_LEAVE => decode_or(&data, path, |id: i64| s.apply_user_leave(id)),
        router::users::ON_DELETE => decode_or(&data, path, |u: crate::models::PublicUser| {
            s.apply_user_delete(u.id)
        }),
        router::voice::ON_JOIN => decode_or(&data, path, |e: crate::state::VoiceStateEvent| {
            s.apply_voice_join(e.channel_id, e.user_id, e.state)
        }),
        router::voice::ON_LEAVE => decode_or(&data, path, |e: crate::state::VoiceLeaveEvent| {
            s.apply_voice_leave(e.channel_id, e.user_id)
        }),
        router::voice::ON_UPDATE_STATE => {
            decode_or(&data, path, |e: crate::state::VoiceStateEvent| {
                s.apply_voice_state(e.channel_id, e.user_id, e.state)
            })
        }
        _ => ChangeSet::default(),
    }
}

/// Decode `data` as `T`; on success call `f` to produce a `ChangeSet`, on
/// failure log and return an empty `ChangeSet`.
fn decode_or<T: serde::de::DeserializeOwned>(
    data: &serde_json::Value,
    path: &str,
    f: impl FnOnce(T) -> ChangeSet,
) -> ChangeSet {
    match serde_json::from_value::<T>(data.clone()) {
        Ok(v) => f(v),
        Err(e) => {
            warn!(%path, %e, "decode event");
            ChangeSet::default()
        }
    }
}
/// Emit a change notification to the observer.
fn emit(observer: &Arc<dyn ClientObserver>, change: ChangeSet, path: &str) {
    if change.messages {
        // Determine the channel id lazily from the state — but we already
        // dropped the lock. The UI re-reads what it needs; per-channel
        // granularity isn't required for v1.
        observer.on_state_changed();
        if path.starts_with("messages.on") {
            // Best-effort: tell the UI messages changed somewhere.
            // (A full impl would carry the channel id through the event.)
        }
    }
    if change.channels || change.users || change.roles || change.voice || change.settings {
        observer.on_state_changed();
    }
}

fn is_empty(c: ChangeSet) -> bool {
    !(c.channels
        || c.users
        || c.roles
        || c.categories
        || c.messages
        || c.voice
        || c.read_states
        || c.settings
        || c.connection)
}

#[allow(clippy::needless_pass_by_value)] // mpsc::Sender kept for future per-sub backpressure
fn _unused(_tx: mpsc::Sender<()>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changeset_empty() {
        assert!(is_empty(ChangeSet::default()));
        assert!(!is_empty(ChangeSet {
            messages: true,
            ..Default::default()
        }));
    }
}
