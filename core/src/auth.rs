//! Authentication: `POST /login` and JWT token storage.
//!
//! Sharkcord authenticates with a JWT obtained from the HTTP `/login`
//! endpoint (not tRPC). The token is then carried in the tRPC WebSocket
//! connection params (`{ token }`) and in the `x-token` header for
//! `POST /upload`. JWTs are stateless and valid for 7 days.

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

/// Error body returned by `/login` on `400` (validation/credentials).
#[derive(Debug, Clone, Deserialize)]
pub struct LoginError {
    pub errors: serde_json::Value,
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

/// Log in to a Sharkord server and return the JWT.
///
/// `origin` is e.g. `http://localhost:4991`. Performs `POST {origin}/login`.
pub async fn login(origin: &str, req: &LoginRequest) -> CoreResult<LoginResponse> {
    let client = reqwest::Client::new();
    let url = format!("{}/login", origin.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .json(req)
        .send()
        .await
        .map_err(|e| crate::CoreError::Auth(format!("login request: {e}")))?;

    match resp.status().as_u16() {
        200 => resp
            .json::<LoginResponse>()
            .await
            .map_err(|e| crate::CoreError::Auth(format!("decode login response: {e}"))),
        400 => {
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_owned());
            Err(crate::CoreError::Auth(format!(
                "invalid credentials: {body}"
            )))
        }
        429 => Err(crate::CoreError::Auth(
            "rate limited; retry later".to_owned(),
        )),
        code => Err(crate::CoreError::Auth(format!("login failed: HTTP {code}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_request_serializes_without_invite() {
        let req = LoginRequest {
            identity: "alice".to_owned(),
            password: "hunter2".to_owned(),
            invite: None,
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("\"identity\":\"alice\""));
        assert!(!s.contains("invite"));
    }

    #[test]
    fn login_request_serializes_with_invite() {
        let req = LoginRequest {
            identity: "bob".to_owned(),
            password: "pw".to_owned(),
            invite: Some("ABC".to_owned()),
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("\"invite\":\"ABC\""));
    }
}
