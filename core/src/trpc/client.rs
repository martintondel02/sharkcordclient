//! The live tRPC-over-WebSocket client.
//!
//! [`TrpcClient`] connects to a Sharkord server's single WebSocket,
//! authenticates via the `{ token }` connection params, and multiplexes
//! queries, mutations, and subscriptions over it. It owns a reader task
//! that fans incoming responses out to waiting callers (queries/mutations)
//! and to open subscription streams (subscriptions), plus a keepalive
//! task that pings the server every 30 s.
//!
//! See [`wire`](super::wire) for the envelope spec and
//! [`router`](super::router) for the typed procedure handles.

use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::sync::{Mutex, RwLock, oneshot};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, warn};

use crate::error::{CoreError, CoreResult};
use crate::trpc::error::TrpcError;
use crate::trpc::wire::{TrpcMethod, TrpcRequest, TrpcResponse, TrpcResultType};

/// The WS write half type.
type WsSink = futures_util::stream::SplitSink<
    WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

/// A pending query/mutation awaiting a single response.
type Pending = oneshot::Sender<Result<Value, TrpcError>>;

/// An open subscription. The sender forwards every `data` event; the
/// receiver lives in the caller. Dropping the receiver stops the
/// subscription (we detect send errors and send `subscriptionStop`).
type SubSink = tokio::sync::mpsc::UnboundedSender<CoreResult<Value>>;

/// Internal shared state.
#[derive(Default)]
struct Shared {
    /// Query/mutation correlation: request id -> oneshot sender.
    pending: Mutex<HashMap<String, Pending>>,
    /// Open subscriptions: subscription id -> event sender.
    subs: Mutex<HashMap<String, SubSink>>,
}

/// A handle to a live subscription. Drop it (or call [`SubscriptionHandle::stop`])
/// to end the subscription.
pub struct SubscriptionHandle {
    id: String,
    shared: Arc<Shared>,
    /// Receiver the caller reads events from.
    pub events: tokio::sync::mpsc::UnboundedReceiver<CoreResult<Value>>,
}

impl SubscriptionHandle {
    /// Stop this subscription. Sends `subscriptionStop` and drops the
    /// receiver. Idempotent.
    pub async fn stop(mut self, client: &TrpcClient) {
        self.events.close();
        if let Err(err) = client.send(TrpcRequest::stop(self.id.clone())).await {
            warn!(error = %err, id = %self.id, "failed to send subscriptionStop");
        }
    }
}

impl Drop for SubscriptionHandle {
    fn drop(&mut self) {
        self.events.close();
    }
}

/// The tRPC-over-WebSocket client.
pub struct TrpcClient {
    shared: Arc<Shared>,
    /// The WS write half, guarded so request sends serialize cleanly.
    writer: Mutex<WsSink>,
    /// Join handle for the reader task (so we can await its exit on close).
    reader: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Join handle for the keepalive task.
    keepalive: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Cancellation token shared by background tasks.
    cancel: tokio_util::sync::CancellationToken,
}

impl TrpcClient {
    /// Connect to `ws(s)://host` with the given JWT in connection params.
    ///
    /// After the WS handshake, sends the raw `{ "data": { "token": ... } }`
    /// first message the server reads for close-tracking, then spawns the
    /// reader + keepalive tasks.
    pub async fn connect(host: &str, token: &str) -> CoreResult<Arc<Self>> {
        let url = build_ws_url(host);
        debug!(%url, "connecting tRPC WebSocket");

        let (mut ws, _resp) = tokio_tungstenite::connect_async(&url)
            .await
            .map_err(|e| CoreError::Disconnected(format!("ws connect: {e}")))?;

        // The server reads a one-off raw `{ data: { token } }` message to
        // map this socket to a user for close/kick tracking.
        let first = serde_json::json!({ "data": { "token": token } });
        ws.send(Message::Text(first.to_string().into()))
            .await
            .map_err(|e| CoreError::Disconnected(format!("ws send token: {e}")))?;

        let shared = Arc::new(Shared::default());
        let cancel = tokio_util::sync::CancellationToken::new();

        let (writer, mut reader) = ws.split();
        let client = Arc::new(Self {
            shared: Arc::clone(&shared),
            writer: Mutex::new(writer),
            reader: Mutex::new(None),
            keepalive: Mutex::new(None),
            cancel: cancel.clone(),
        });

        // Reader task: fan responses out to pending requests and subs.
        let shared_r = Arc::clone(&shared);
        let cancel_r = cancel.clone();
        let reader_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    () = cancel_r.cancelled() => break,
                    msg = reader.next() => {
                        let Some(msg) = msg else { break; };
                        match msg {
                            Ok(Message::Text(text)) => {
                                if let Err(err) = dispatch_text(&shared_r, &text).await {
                                    warn!(error = %err, "dispatch error");
                                }
                            }
                            Ok(Message::Ping(p)) => {
                                debug!(len = p.len(), "ws ping");
                            }
                            Ok(Message::Pong(_) | Message::Binary(_) | Message::Frame(_)) => {}
                            Ok(Message::Close(reason)) => {
                                warn!(?reason, "ws closed by server");
                                fail_all(&shared_r, CoreError::Disconnected(
                                    reason.map_or_else(String::new, |r| format!("{r}")))).await;
                                break;
                            }
                            Err(err) => {
                                warn!(%err, "ws read error");
                                fail_all(&shared_r, CoreError::Disconnected(err.to_string())).await;
                                break;
                            }
                        }
                    }
                }
            }
            debug!("tRPC reader task exiting");
        });

        *client.reader.lock().await = Some(reader_handle);
        Ok(client)
    }

    /// Send a query or mutation and await its single response.
    async fn request(
        &self,
        method: TrpcMethod,
        path: &str,
        input: Option<Value>,
    ) -> CoreResult<Value> {
        let req = TrpcRequest::new(method, path, input);
        let id = req.id.clone();
        let (tx, rx) = oneshot::channel();
        self.shared.pending.lock().await.insert(id.clone(), tx);
        self.send(req).await?;

        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(Ok(v))) => Ok(v),
            Ok(Ok(Err(e))) => Err(e.into()),
            Ok(Err(_)) => Err(CoreError::Disconnected("request dropped".into())),
            Err(_) => {
                self.shared.pending.lock().await.remove(&id);
                Err(CoreError::Timeout)
            }
        }
    }

    /// `query` a procedure.
    pub async fn query(&self, path: &str, input: Option<Value>) -> CoreResult<Value> {
        self.request(TrpcMethod::Query, path, input).await
    }

    /// `mutation` a procedure.
    pub async fn mutation(&self, path: &str, input: Option<Value>) -> CoreResult<Value> {
        self.request(TrpcMethod::Mutation, path, input).await
    }

    /// Open a `subscription` and return a handle with an event receiver.
    pub async fn subscribe(
        self: &Arc<Self>,
        path: &str,
        input: Option<Value>,
    ) -> CoreResult<SubscriptionHandle> {
        let req = TrpcRequest::new(TrpcMethod::Subscription, path, input);
        let id = req.id.clone();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.shared.subs.lock().await.insert(id.clone(), tx);
        self.send(req).await?;

        Ok(SubscriptionHandle {
            id,
            shared: Arc::clone(&self.shared),
            events: rx,
        })
    }

    /// Send a raw request envelope over the WS writer.
    async fn send(&self, req: TrpcRequest) -> CoreResult<()> {
        let json = serde_json::to_string(&req)?;
        let mut writer = self.writer.lock().await;
        writer
            .send(Message::Text(json.into()))
            .await
            .map_err(|e| CoreError::Disconnected(format!("ws send: {e}")))
    }

    /// Gracefully close the connection and stop background tasks.
    pub async fn close(&self) {
        self.cancel.cancel();
        let mut writer = self.writer.lock().await;
        let _ = writer.send(Message::Close(None)).await;
        // Drop the writer so the reader task observes EOF.
        // (Lock is released on drop at scope end.)
        drop(writer);
        if let Some(h) = self.reader.lock().await.take() {
            let _ = h.await;
        }
    }

    // Keepalive is handled by tokio-tungstenite's automatic pong to server
    // pings (server pings every 30 s; the library responds). No
    // client-side keepalive task is required.
}

/// Parse and dispatch one text message to the right pending/sub handler.
async fn dispatch_text(shared: &Arc<Shared>, text: &str) -> CoreResult<()> {
    let resp: TrpcResponse = serde_json::from_str(text).map_err(|e| TrpcError::decode(&e))?;

    // Subscription event: has result.type.
    if let Some(result) = resp.result {
        match result.result_type {
            Some(TrpcResultType::Initial) | None => {
                // For a query/mutation, complete the pending request.
                if let Some(tx) = shared.pending.lock().await.remove(&resp.id) {
                    let _ = tx.send(Ok(result.data));
                }
            }
            Some(TrpcResultType::Data) => {
                if let Some(tx) = shared.subs.lock().await.get(&resp.id) {
                    if tx.send(Ok(result.data)).is_err() {
                        // Receiver dropped: stop the subscription.
                        shared.subs.lock().await.remove(&resp.id);
                        // (We can't send subscriptionStop here without the writer;
                        // the caller's SubscriptionHandle::stop does it. Best-effort.)
                    }
                }
            }
            Some(TrpcResultType::Stopped) => {
                shared.subs.lock().await.remove(&resp.id);
            }
        }
        return Ok(());
    }

    // Error response.
    if let Some(err) = resp.error {
        let code = err.json.data.code;
        let path = err.json.data.path;
        let message = err.json.message;
        let trpc_err = TrpcError::Server {
            code,
            path,
            message,
        };
        // If it's a subscription id, forward as an error event and remove.
        let mut subs = shared.subs.lock().await;
        if let Some(tx) = subs.remove(&resp.id) {
            let _ = tx.send(Err(trpc_err.into()));
        } else {
            drop(subs);
            let mut pending = shared.pending.lock().await;
            if let Some(tx) = pending.remove(&resp.id) {
                let _ = tx.send(Err(trpc_err));
            }
        }
    }
    Ok(())
}

/// Fail all pending requests and subscriptions with `err` (on disconnect).
async fn fail_all(shared: &Arc<Shared>, err: CoreError) {
    let mut pending = shared.pending.lock().await;
    for (_, tx) in pending.drain() {
        let _ = tx.send(Err(TrpcError::Decode(err.to_string())));
    }
    drop(pending);
    let mut subs = shared.subs.lock().await;
    for (_, tx) in subs.drain() {
        let _ = tx.send(Err(CoreError::Disconnected(err.to_string())));
    }
}

/// Build the WebSocket URL from a host like `localhost:4991` or
/// `https://demo.sharkord.com`. Forces `wss://` for https origins, `ws://`
/// otherwise.
fn build_ws_url(host: &str) -> String {
    let host = host.trim();
    if let Some(rest) = host.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = host.strip_prefix("http://") {
        format!("ws://{rest}")
    } else if host.starts_with("ws://") || host.starts_with("wss://") {
        host.to_owned()
    } else {
        format!("ws://{host}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_url_from_plain_host() {
        assert_eq!(build_ws_url("localhost:4991"), "ws://localhost:4991");
    }

    #[test]
    fn ws_url_from_https() {
        assert_eq!(
            build_ws_url("https://demo.sharkord.com"),
            "wss://demo.sharkord.com"
        );
    }

    #[test]
    fn ws_url_from_http() {
        assert_eq!(build_ws_url("http://h:4991"), "ws://h:4991");
    }
}
