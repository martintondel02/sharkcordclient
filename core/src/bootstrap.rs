//! The mandatory startup sequence: `handshake` → `joinServer`.
//!
//! Being authenticated on the WebSocket is not sufficient — `ctx.authenticated`
//! starts `false` and is only flipped by `others.joinServer`, which also
//! returns the entire initial server state in one shot.
//!
//! Flow (see `docs/api-reference.md` §2.4):
//! 1. `auth::login` → JWT.
//! 2. Open the tRPC WebSocket with connection params `{ token }`.
//! 3. `others.handshake` → `{ handshakeHash, hasPassword }`.
//! 4. If `hasPassword`, the UI collects the server password.
//! 5. `others.joinServer({ handshakeHash, password? })` →
//!    [`InitialState`]; hydrate the state store.
//!
//! _Stub: implemented in Phase 1 once the WS client is live._

use crate::error::CoreResult;
use crate::trpc::router::others::InitialState;

/// Placeholder for the connected tRPC client handle. Replaced by a real
/// `TrpcClient` type in Phase 1 once `trpc::client` is implemented.
pub struct ConnectedClient;

/// Run the bootstrap against an already-connected tRPC client.
///
/// Performs `others.handshake` then `others.joinServer` (with the
/// optional server password) and returns the initial server state to
/// hydrate [`crate::state`].
///
/// _Stub._
#[allow(clippy::unused_async)] // stub: awaits land in Phase 1
pub async fn bootstrap(
    _client: &ConnectedClient,
    _server_password: Option<&str>,
) -> CoreResult<InitialState> {
    Err(crate::CoreError::Other(
        "bootstrap::bootstrap not yet implemented (Phase 1)".to_owned(),
    ))
}
