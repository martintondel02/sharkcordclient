//! The mandatory startup sequence: `handshake` → `joinServer`.
//!
//! Being authenticated on the WebSocket is not sufficient — `ctx.authenticated`
//! starts `false` and is only flipped by `others.joinServer`, which also
//! returns the entire initial server state in one shot.
//!
//! Flow (see `docs/api-reference.md` §Auth & bootstrap):
//! 1. `auth::login` → JWT.
//! 2. Open the tRPC WebSocket with connection params `{ token }`.
//! 3. `others.handshake` → `{ handshakeHash, hasPassword }`.
//! 4. If `hasPassword`, the UI collects the server password.
//! 5. `others.joinServer({ handshakeHash, password? })` →
//!    [`InitialState`]; hydrate the state store.

use crate::error::CoreResult;
use crate::trpc::router::others::{HandshakeOutput, InitialState, JoinServerInput};
use crate::trpc::{TrpcClient, router};
use serde_json::json;
use std::sync::Arc;

/// Run `others.handshake` to learn the server's handshake hash and whether
/// a server password is required.
pub async fn handshake(client: &Arc<TrpcClient>) -> CoreResult<HandshakeOutput> {
    let v = client.query(router::others::HANDSHAKE, None).await?;
    Ok(serde_json::from_value(v)?)
}

/// Run `others.joinServer` with the handshake hash (and the optional
/// server password). Establishes the authenticated session and returns
/// the full initial server state to hydrate [`crate::state`].
pub async fn join_server(
    client: &Arc<TrpcClient>,
    handshake_hash: &str,
    server_password: Option<&str>,
) -> CoreResult<InitialState> {
    let input = JoinServerInput {
        handshake_hash: handshake_hash.to_owned(),
        password: server_password.map(str::to_owned),
    };
    let v = client
        .query(router::others::JOIN_SERVER, Some(json!(input)))
        .await?;
    Ok(serde_json::from_value(v)?)
}

/// Convenience: run the full bootstrap (handshake + joinServer) when no
/// server password is needed. Returns the initial state.
pub async fn bootstrap(
    client: &Arc<TrpcClient>,
    server_password: Option<&str>,
) -> CoreResult<InitialState> {
    let hs = handshake(client).await?;
    if hs.has_password && server_password.is_none() {
        return Err(crate::CoreError::ServerGate(
            "server requires a password but none was provided".to_owned(),
        ));
    }
    join_server(client, &hs.handshake_hash, server_password).await
}
