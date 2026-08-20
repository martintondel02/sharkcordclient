//! The FFI facade exposed to the Kotlin (UniFFI) and egui front-ends.
//!
//! This is the high-level surface the UIs call: `connect`, `send_message`,
//! `join_voice`, `toggle_mic`, etc. It owns the core's async runtime,
//! the tRPC client, the state store, and the voice engine, and emits
//! change notifications back to the UI via a callback interface.
//!
//! For Phase 1 (text), the facade exposes connect/disconnect, message
//! listing/sending/editing/deleting, reactions, typing, and channel
//! selection. Phase 2 adds voice.

use std::sync::Arc;

use serde_json::json;
use tokio::sync::RwLock;

use crate::auth::{self, LoginRequest};
use crate::bootstrap;
use crate::error::CoreResult;
use crate::state::{ChangeSet, ConnectionState, ServerState, SharedState};
use crate::subscriptions::Subscriptions;
use crate::trpc::TrpcClient;
use crate::trpc::router;

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

    /// A new message arrived (best-effort hint; the UI re-reads state).
    fn on_message_new(&self, _channel_id: i64) {}

    /// A voice participant state changed in `channel_id`.
    fn on_voice_state(&self, _channel_id: i64) {}
}

/// The high-level client facade.
///
/// Clone cheaply (all fields are `Arc`).
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    /// Dedicated multi-threaded runtime so UIs don't need to bring tokio.
    rt: tokio::runtime::Runtime,
    trpc: RwLock<Option<Arc<TrpcClient>>>,
    state: SharedState,
    subs: RwLock<Option<Subscriptions>>,
    observer: RwLock<Option<Arc<dyn ClientObserver>>>,
    /// The origin the client connected to (for file URLs/uploads).
    origin: RwLock<String>,
    /// The current JWT (for uploads).
    token: RwLock<String>,
}

impl Client {
    /// Create a new client with no connection. The UI calls
    /// [`connect`](Self::connect) to attach to a server.
    #[must_use]
    pub fn new() -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");
        Self {
            inner: Arc::new(Inner {
                rt,
                trpc: RwLock::new(None),
                state: Arc::new(RwLock::new(ServerState::default())),
                subs: RwLock::new(None),
                observer: RwLock::new(None),
                origin: RwLock::new(String::new()),
                token: RwLock::new(String::new()),
            }),
        }
    }

    /// Set the observer that receives change notifications.
    pub fn set_observer(&self, observer: Arc<dyn ClientObserver>) {
        let inner = &self.inner;
        inner
            .rt
            .block_on(async { *inner.observer.write().await = Some(observer) });
    }

    /// Connect to a Sharkcord server, log in, run the bootstrap, and
    /// start subscriptions. `origin` is e.g. `http://localhost:4991`.
    ///
    /// If the server requires a password, pass it via `server_password`.
    pub async fn connect(
        &self,
        origin: &str,
        identity: &str,
        password: &str,
        invite: Option<&str>,
        server_password: Option<&str>,
    ) -> CoreResult<()> {
        let inner = &self.inner;
        *inner.origin.write().await = origin.to_owned();
        inner
            .state
            .write()
            .await
            .set_connection(ConnectionState::Connecting);

        // 1. Login over HTTP.
        let req = LoginRequest {
            identity: identity.to_owned(),
            password: password.to_owned(),
            invite: invite.map(str::to_owned),
        };
        let login_resp = auth::login(origin, &req).await?;
        *inner.token.write().await = login_resp.token.clone();

        // 2. Open the tRPC WebSocket.
        let client = TrpcClient::connect(origin, &login_resp.token).await?;
        *inner.trpc.write().await = Some(Arc::clone(&client));

        // 3. Bootstrap (handshake + joinServer).
        let init = bootstrap::bootstrap(&client, server_password).await?;
        let mut change = inner.state.write().await;
        change.apply_initial(init);
        let own_user_id = change.own_user_id;
        drop(change);

        // 4. Start subscriptions.
        let observer = inner.observer.read().await.clone();
        let observer = observer.unwrap_or_else(|| Arc::new(NoopObserver));
        let subs = Subscriptions::start(&client, &inner.state, &observer);
        *inner.subs.write().await = Some(subs);

        if let Some(observer) = inner.observer.read().await.clone() {
            observer.on_connected();
            observer.on_message_new(0); // hint UI to refresh
            let _ = own_user_id;
        }
        Ok(())
    }

    /// Disconnect: stop subscriptions, close the WS, reset state.
    pub async fn disconnect(&self) -> CoreResult<()> {
        let inner = &self.inner;
        if let Some(mut subs) = inner.subs.write().await.take() {
            subs.stop();
        }
        if let Some(client) = inner.trpc.write().await.take() {
            client.close().await;
        }
        inner
            .state
            .write()
            .await
            .set_connection(ConnectionState::Disconnected);
        if let Some(observer) = inner.observer.read().await.clone() {
            observer.on_disconnected(1000, "disconnected".to_owned());
        }
        Ok(())
    }

    /// Get a consistent snapshot of the server state for rendering.
    pub async fn snapshot(&self) -> ServerState {
        self.inner.state.read().await.clone()
    }

    /// The current connection state.
    pub async fn connection_state(&self) -> ConnectionState {
        self.inner.state.read().await.connection
    }

    /// Select a channel (UI hint). Also loads messages if not loaded.
    pub async fn select_channel(&self, channel_id: i64) -> CoreResult<()> {
        let inner = &self.inner;
        inner
            .state
            .write()
            .await
            .set_selected_channel(Some(channel_id));
        // Load messages if we don't have them yet.
        let needs_load = inner.state.read().await.messages(channel_id).is_none();
        if needs_load {
            self.load_messages(channel_id, None).await?;
        }
        if let Some(observer) = inner.observer.read().await.clone() {
            observer.on_state_changed();
        }
        Ok(())
    }

    /// Load (or paginate) messages for a channel via `messages.get`.
    pub async fn load_messages(
        &self,
        channel_id: i64,
        cursor: Option<i64>,
    ) -> CoreResult<crate::trpc::router::messages::Page> {
        let inner = &self.inner;
        let client = inner
            .trpc
            .read()
            .await
            .clone()
            .ok_or_else(|| crate::CoreError::Disconnected("not connected".into()))?;
        let input = json!({
            "channelId": channel_id,
            "cursor": cursor,
            "limit": 100,
        });
        let v = client.query(router::messages::GET, Some(input)).await?;
        let page: crate::trpc::router::messages::Page = serde_json::from_value(v)?;
        // Merge into state (newest-first). For pagination we prepend older messages.
        let mut s = inner.state.write().await;
        let msgs = s.messages_by_channel.entry(channel_id).or_default();
        if cursor.is_none() {
            // First load: replace.
            msgs.clone_from(&page.messages);
        } else {
            // Pagination: append older messages at the end.
            for m in &page.messages {
                if !msgs.iter().any(|x| x.id == m.id) {
                    msgs.push(m.clone());
                }
            }
        }
        drop(s);
        if let Some(observer) = inner.observer.read().await.clone() {
            observer.on_state_changed();
        }
        Ok(page)
    }

    /// Send a message to a channel.
    pub async fn send_message(
        &self,
        channel_id: i64,
        content: &str,
        reply_to: Option<i64>,
    ) -> CoreResult<i64> {
        let inner = &self.inner;
        let client = inner
            .trpc
            .read()
            .await
            .clone()
            .ok_or_else(|| crate::CoreError::Disconnected("not connected".into()))?;
        let input = json!({
            "content": content,
            "channelId": channel_id,
            "replyToMessageId": reply_to,
        });
        let v = client.mutation(router::messages::SEND, Some(input)).await?;
        Ok(serde_json::from_value::<i64>(v).unwrap_or(0))
    }

    /// Edit a message.
    pub async fn edit_message(&self, message_id: i64, content: &str) -> CoreResult<()> {
        let inner = &self.inner;
        let client = inner
            .trpc
            .read()
            .await
            .clone()
            .ok_or_else(|| crate::CoreError::Disconnected("not connected".into()))?;
        let input = json!({ "messageId": message_id, "content": content });
        client.mutation(router::messages::EDIT, Some(input)).await?;
        Ok(())
    }

    /// Delete a message.
    pub async fn delete_message(&self, message_id: i64) -> CoreResult<()> {
        let inner = &self.inner;
        let client = inner
            .trpc
            .read()
            .await
            .clone()
            .ok_or_else(|| crate::CoreError::Disconnected("not connected".into()))?;
        let input = json!({ "messageId": message_id });
        client
            .mutation(router::messages::DELETE, Some(input))
            .await?;
        Ok(())
    }

    /// Toggle a reaction on a message.
    pub async fn toggle_reaction(&self, message_id: i64, emoji: &str) -> CoreResult<()> {
        let inner = &self.inner;
        let client = inner
            .trpc
            .read()
            .await
            .clone()
            .ok_or_else(|| crate::CoreError::Disconnected("not connected".into()))?;
        let input = json!({ "messageId": message_id, "emoji": emoji });
        client
            .mutation(router::messages::TOGGLE_REACTION, Some(input))
            .await?;
        Ok(())
    }

    /// Signal typing in a channel.
    pub async fn signal_typing(&self, channel_id: i64) -> CoreResult<()> {
        let inner = &self.inner;
        let client = inner
            .trpc
            .read()
            .await
            .clone()
            .ok_or_else(|| crate::CoreError::Disconnected("not connected".into()))?;
        let input = json!({ "channelId": channel_id });
        client
            .mutation(router::messages::SIGNAL_TYPING, Some(input))
            .await?;
        Ok(())
    }

    /// Mark a channel as read.
    pub async fn mark_channel_read(&self, channel_id: i64) -> CoreResult<()> {
        let inner = &self.inner;
        let client = inner
            .trpc
            .read()
            .await
            .clone()
            .ok_or_else(|| crate::CoreError::Disconnected("not connected".into()))?;
        let input = json!({ "channelId": channel_id });
        client
            .mutation(router::channels::MARK_AS_READ, Some(input))
            .await?;
        Ok(())
    }

    /// Upload a file and return the temp file id (for `send_message`'s `files`).
    pub async fn upload_file(
        &self,
        file_name: &str,
        mime_type: &str,
        bytes: Vec<u8>,
    ) -> CoreResult<String> {
        let inner = &self.inner;
        let origin = inner.origin.read().await.clone();
        let token = inner.token.read().await.clone();
        let temp = crate::files::upload(&origin, &token, file_name, mime_type, bytes).await?;
        Ok(temp.id)
    }

    /// Build a file URL for rendering (uses the connected origin).
    pub async fn file_url(
        &self,
        name: &str,
        access_token: Option<&str>,
        expires: Option<i64>,
    ) -> String {
        let origin = self.inner.origin.read().await.clone();
        crate::files::file_url(&origin, name, access_token, expires)
    }

    /// Block on a future using the client's runtime. Convenience for UIs
    /// that aren't async.
    pub fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        self.inner.rt.block_on(fut)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

/// A no-op observer used when none is set.
struct NoopObserver;

impl ClientObserver for NoopObserver {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_constructs() {
        let _client = Client::new();
    }
}
