//! Crate-wide error type.

use thiserror::Error;

/// Errors returned by the core.
#[derive(Debug, Error)]
pub enum CoreError {
    /// A tRPC procedure returned an error response.
    #[error("trpc error ({code}){path}: {message}", path = .path.as_ref().map(|p| format!(" at {p}")).unwrap_or_default())]
    Trpc {
        /// tRPC error code string (e.g. `UNAUTHORIZED`, `FORBIDDEN`).
        code: String,
        /// Human-readable message from the server.
        message: String,
        /// Procedure path that failed (e.g. `messages.send`), when known.
        path: Option<String>,
    },

    /// The WebSocket was closed or is not connected.
    #[error("connection closed{0}")]
    Disconnected(String),

    /// A request timed out waiting for a response.
    #[error("request timed out")]
    Timeout,

    /// Authentication failed (bad credentials, expired/revoked token).
    #[error("authentication failed: {0}")]
    Auth(String),

    /// The server rejected the handshake/server-password gate.
    #[error("server rejected: {0}")]
    ServerGate(String),

    /// Serialization / deserialization of a tRPC envelope or payload.
    #[error("codec error: {0}")]
    Codec(#[from] serde_json::Error),

    /// Low-level IO failure (socket, file).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// The server's protocol response did not match what we expect for
    /// the target `TARGET_PROTOCOL_VERSION`. Raised by [`crate::compat`].
    #[error("protocol mismatch: {0}")]
    ProtocolMismatch(String),

    /// The voice engine could not complete an operation (mediasoup
    /// signaling, transport, producer/consumer).
    #[error("voice error: {0}")]
    Voice(String),

    /// A platform capture source was unavailable or denied.
    #[error("capture error: {0}")]
    Capture(String),

    /// Anything not covered above.
    #[error("{0}")]
    Other(String),
}

/// Convenience alias.
pub type CoreResult<T> = Result<T, CoreError>;
