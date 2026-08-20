//! tRPC wire-protocol envelopes.
//!
//! tRPC v11 uses a JSON-RPC 2.0–ish envelope. Over the WebSocket link
//! (`wsLink`), requests and responses are individual JSON messages with
//! a shared `id` for correlation. The shapes below are the ones this
//! client sends and receives.
//!
//! Reference: <https://trpc.io/docs/v11/rpc> and the
//! `@trpc/server/adapters/ws` implementation that Sharkcord uses
//! (`apps/server/src/utils/wss.ts`).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Requests (client -> server)
// ---------------------------------------------------------------------------

/// One of the tRPC methods a client can invoke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrpcMethod {
    /// A query (idempotent read).
    #[serde(rename = "query")]
    Query,
    /// A mutation (write).
    #[serde(rename = "mutation")]
    Mutation,
    /// Open a subscription (streaming).
    #[serde(rename = "subscription")]
    Subscription,
    /// Stop an open subscription. Sent with the same `id` as the
    /// original `subscription` request.
    #[serde(rename = "subscriptionStop")]
    SubscriptionStop,
}

/// Parameters carried by every request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrpcRequestParams {
    /// Dotted procedure path, e.g. `messages.send`, `voice.join`.
    pub path: String,
    /// JSON-serialized input matching the procedure's zod schema. Omitted
    /// (not `null`) when the procedure takes no input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
}

/// A full tRPC request envelope sent over the WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrpcRequest {
    /// Client-chosen correlation id. Must be unique among in-flight
    /// requests on this connection.
    pub id: String,
    /// Fixed JSON-RPC version.
    pub jsonrpc: String,
    /// The method to invoke.
    pub method: TrpcMethod,
    /// The procedure path and optional input.
    pub params: TrpcRequestParams,
}

impl TrpcRequest {
    /// Build a request with a freshly-generated id.
    pub fn new(
        method: TrpcMethod,
        path: impl Into<String>,
        input: Option<serde_json::Value>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            jsonrpc: JSONRPC_VERSION.to_owned(),
            method,
            params: TrpcRequestParams {
                path: path.into(),
                input,
            },
        }
    }

    /// Build a `subscriptionStop` for the given in-flight subscription id.
    pub fn stop(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            jsonrpc: JSONRPC_VERSION.to_owned(),
            method: TrpcMethod::SubscriptionStop,
            params: TrpcRequestParams {
                path: String::new(),
                input: None,
            },
        }
    }
}

/// JSON-RPC version string.
pub const JSONRPC_VERSION: &str = "2.0";

// ---------------------------------------------------------------------------
// Responses (server -> client)
// ---------------------------------------------------------------------------

/// Discriminant for the `result.type` field on subscription responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrpcResultType {
    /// First event from a subscription (subscription accepted).
    Initial,
    /// A data event on an open subscription.
    Data,
    /// Subscription ended cleanly.
    Stopped,
}

/// The `result` object on a successful response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrpcResult {
    /// On a query/mutation this is absent (data lives in `data`). On a
    /// subscription it is one of [`TrpcResultType`].
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub result_type: Option<TrpcResultType>,
    /// The procedure output (query/mutation) or event payload
    /// (subscription `data` event).
    pub data: serde_json::Value,
}

/// The `error` object on a failed response. Mirrors tRPC's
/// JSON-RPC error envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrpcErrorBody {
    /// The JSON-RPC error object.
    pub json: TrpcErrorJson,
}

/// Inner JSON of a tRPC error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrpcErrorJson {
    /// Human-readable message.
    pub message: String,
    /// JSON-RPC numeric code (e.g. `-32001` for `UNAUTHORIZED`).
    pub code: i32,
    /// Extra tRPC metadata, including the string `code` and `path`.
    pub data: TrpcErrorData,
}

/// tRPC error metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrpcErrorData {
    /// String error code, e.g. `UNAUTHORIZED`, `FORBIDDEN`,
    /// `BAD_REQUEST`, `TOO_MANY_REQUESTS`.
    pub code: String,
    /// HTTP status the server maps this to.
    pub http_status: i32,
    /// Procedure path that failed, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// A full response envelope received over the WebSocket. A successful
/// query/mutation carries `result` (no `result_type`); a subscription
/// carries `result` with a `result_type`. A failure carries `error`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrpcResponse {
    /// Matches the request id (or subscription id for events).
    pub id: String,
    /// JSON-RPC version.
    pub jsonrpc: String,
    /// Present on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TrpcResult>,
    /// Present on failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TrpcErrorBody>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_with_input_serializes() {
        let req = TrpcRequest::new(
            TrpcMethod::Mutation,
            "messages.send",
            Some(serde_json::json!({
                "content": "hi",
                "channelId": 1,
            })),
        );
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("\"method\":\"mutation\""));
        assert!(s.contains("\"path\":\"messages.send\""));
        assert!(s.contains("\"input\""));
        assert!(s.contains("\"jsonrpc\":\"2.0\""));
    }

    #[test]
    fn request_without_input_omits_field() {
        let req = TrpcRequest::new(TrpcMethod::Query, "others.handshake", None);
        let s = serde_json::to_string(&req).unwrap();
        assert!(!s.contains("\"input\""));
    }

    #[test]
    fn subscription_stop_envelope() {
        let req = TrpcRequest::stop("abc-123");
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("\"method\":\"subscriptionStop\""));
        assert!(s.contains("\"id\":\"abc-123\""));
    }

    #[test]
    fn parses_successful_query_response() {
        let raw = serde_json::json!({
            "id": "1",
            "jsonrpc": "2.0",
            "result": { "data": { "handshakeHash": "abc", "hasPassword": false } },
        });
        let resp: TrpcResponse = serde_json::from_value(raw).unwrap();
        assert_eq!(resp.id, "1");
        let result = resp.result.unwrap();
        assert!(result.result_type.is_none());
        assert_eq!(result.data["handshakeHash"], "abc");
    }

    #[test]
    fn parses_subscription_data_event() {
        let raw = serde_json::json!({
            "id": "sub-1",
            "jsonrpc": "2.0",
            "result": { "type": "data", "data": { "channelId": 7, "userId": 2 } },
        });
        let resp: TrpcResponse = serde_json::from_value(raw).unwrap();
        let result = resp.result.unwrap();
        assert_eq!(result.result_type, Some(TrpcResultType::Data));
        assert_eq!(result.data["channelId"], 7);
    }

    #[test]
    fn parses_error_response() {
        let raw = serde_json::json!({
            "id": "1",
            "jsonrpc": "2.0",
            "error": {
                "json": {
                    "message": "nope",
                    "code": -32003,
                    "data": { "code": "FORBIDDEN", "httpStatus": 403, "path": "messages.send" },
                },
            },
        });
        let resp: TrpcResponse = serde_json::from_value(raw).unwrap();
        let err = resp.error.unwrap().json;
        assert_eq!(err.data.code, "FORBIDDEN");
        assert_eq!(err.data.path, Some("messages.send".to_owned()));
    }
}
