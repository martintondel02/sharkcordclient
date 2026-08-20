//! Authentication: `POST /login` and JWT token storage.
//!
//! Sharkcord authenticates with a JWT obtained from the HTTP `/login`
//! endpoint (not tRPC). The token is then carried in the tRPC WebSocket
//! connection params (`{ token }`) and in the `x-token` header for
//! `POST /upload`. JWTs are stateless and valid for 7 days.
//!
//! This module is a stub; Phase 1 implements `login()` over HTTP and the
//! [`TokenStore`] trait backed by platform secure storage (Android
//! Keystore via FFI; OS keyring on desktop).

use crate::error::CoreResult;
use serde::{Deserialize, Serialize};

/// Request body for `POST /login`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    /// Username/identity string (>=1 char; lowercased server-side).
    pub identity: String,
    /// Password (4..128 chars).
    pub password: String,
    /// Optional invite code, required to register when `allowNewUsers`
    /// is false on the server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invite: Option<String>,
}

/// Response body from `POST /login` on success.
#[derive(Debug, Clone, Deserialize)]
pub struct LoginResponse {
    pub success: bool,
    /// The JWT to use for the WS connection params and upload header.
    pub token: String,
}

/// Abstraction over secure token storage.
///
/// Implementations:
/// - Android: JNI-backed Keystore wrapper provided by the Kotlin UI.
/// - Desktop: OS keyring (`keyring` crate) or an encrypted on-disk file.
pub trait TokenStore: Send + Sync {
    /// Persist the JWT (and the identity it belongs to).
    fn save(&self, identity: &str, token: &str) -> CoreResult<()>;

    /// Load the saved JWT for `identity`, if any.
    fn load(&self, identity: &str) -> CoreResult<Option<String>>;

    /// Delete the saved JWT.
    fn clear(&self, identity: &str) -> CoreResult<()>;
}

/// Log in to a Sharkcord server and return the JWT.
///
/// `origin` is e.g. `http://localhost:4991`. Performs `POST {origin}/login`.
/// On a `429` the response carries a `Retry-After` header (surfaced via
/// [`crate::CoreError::Other`] for now; Phase 1 will model it).
///
/// _Stub: implemented in Phase 1 alongside the HTTP client._
#[allow(clippy::unused_async)] // stub: awaits land in Phase 1
pub async fn login(_origin: &str, _req: LoginRequest) -> CoreResult<LoginResponse> {
    Err(crate::CoreError::Other(
        "auth::login not yet implemented (Phase 1)".to_owned(),
    ))
}
