//! tRPC error type and numeric-code mapping.
//!
//! See `docs/wire-protocol.md` and
//! <https://trpc.io/docs/v11/rpc#error-codes---json-rpc-20-error-codes>.

use crate::error::CoreError;
use thiserror::Error;

/// A typed tRPC error raised by the client when a procedure fails.
#[derive(Debug, Error)]
pub enum TrpcError {
    /// The server returned a tRPC error envelope with one of the known
    /// string codes.
    #[error("trpc {code}{path}: {message}", path = .path.as_ref().map(|p| format!(" at {p}")).unwrap_or_default())]
    Server {
        /// String code, e.g. `UNAUTHORIZED`, `FORBIDDEN`, `BAD_REQUEST`,
        /// `TOO_MANY_REQUESTS`, `NOT_FOUND`, `CONFLICT`,
        /// `INTERNAL_SERVER_ERROR`.
        code: String,
        /// Procedure path, when known.
        path: Option<String>,
        /// Server message.
        message: String,
    },

    /// The response could not be deserialized as a tRPC envelope.
    #[error("trpc decode error: {0}")]
    Decode(String),
}

impl From<TrpcError> for CoreError {
    fn from(err: TrpcError) -> Self {
        match err {
            TrpcError::Server {
                code,
                path,
                message,
            } => CoreError::Trpc {
                code,
                message,
                path,
            },
            TrpcError::Decode(msg) => CoreError::Other(format!("trpc decode error: {msg}")),
        }
    }
}

/// Helper to convert a [`serde_json::Error`] into a [`TrpcError::Decode`].
impl TrpcError {
    #[allow(dead_code)] // used by the WS client
    pub(crate) fn decode(err: &serde_json::Error) -> Self {
        Self::Decode(err.to_string())
    }
}

/// JSON-RPC 2.0 numeric error codes used by tRPC, mapped to their string
/// equivalents. Copied from `@trpc/server` `rpc/codes.ts`.
///
/// Used to translate the `error.json.code` number on the wire into the
/// friendlier string code we surface in [`TrpcError::Server`].
pub mod codes {
    /// Parse the tRPC string code from the numeric JSON-RPC code.
    ///
    /// tRPC encodes 4xx HTTP statuses as `-320NN` where `NN` are the last
    /// two digits of the status; a handful of 5xx codes all collapse to
    /// `INTERNAL_SERVER_ERROR` (`-32603`). Unknown codes map to
    /// `INTERNAL_SERVER_ERROR`.
    #[must_use]
    pub fn string_from_numeric(num: i32) -> &'static str {
        match num {
            -32700 => "PARSE_ERROR",
            -32600 => "BAD_REQUEST",
            -32603 => "INTERNAL_SERVER_ERROR",
            -32001 => "UNAUTHORIZED",
            -32002 => "PAYMENT_REQUIRED",
            -32003 => "FORBIDDEN",
            -32004 => "NOT_FOUND",
            -32005 => "METHOD_NOT_SUPPORTED",
            -32008 => "TIMEOUT",
            -32009 => "CONFLICT",
            -32012 => "PRECONDITION_FAILED",
            -32013 => "PAYLOAD_TOO_LARGE",
            -32015 => "UNSUPPORTED_MEDIA_TYPE",
            -32022 => "UNPROCESSABLE_CONTENT",
            -32028 => "PRECONDITION_REQUIRED",
            -32029 => "TOO_MANY_REQUESTS",
            -32099 => "CLIENT_CLOSED_REQUEST",
            _ => "INTERNAL_SERVER_ERROR",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_numeric_codes() {
        assert_eq!(codes::string_from_numeric(-32001), "UNAUTHORIZED");
        assert_eq!(codes::string_from_numeric(-32003), "FORBIDDEN");
        assert_eq!(codes::string_from_numeric(-32029), "TOO_MANY_REQUESTS");
        assert_eq!(codes::string_from_numeric(-32603), "INTERNAL_SERVER_ERROR");
        assert_eq!(codes::string_from_numeric(-32700), "PARSE_ERROR");
    }

    #[test]
    fn unknown_code_falls_back() {
        assert_eq!(codes::string_from_numeric(-99999), "INTERNAL_SERVER_ERROR");
    }

    #[test]
    fn trpc_error_converts_to_core() {
        let err = TrpcError::Server {
            code: "FORBIDDEN".to_owned(),
            path: Some("messages.send".to_owned()),
            message: "no".to_owned(),
        };
        let core: CoreError = err.into();
        match core {
            CoreError::Trpc { code, path, .. } => {
                assert_eq!(code, "FORBIDDEN");
                assert_eq!(path.as_deref(), Some("messages.send"));
            }
            other => panic!("expected Trpc, got {other:?}"),
        }
    }
}
