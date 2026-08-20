//! tRPC-over-WebSocket client for Sharkcord.
//!
//! Sharkcord exposes its entire API as a single tRPC router over one
//! WebSocket connection (`ws(s)://host:4991`). Queries, mutations, and
//! subscriptions all multiplex over that socket; auth is a JWT passed in
//! the WebSocket connection params (`{ token }`). There is no HTTP tRPC
//! link and no OpenAPI/JSON-schema export — this module reimplements the
//! tRPC v11 wire protocol by hand.
//!
//! See `docs/wire-protocol.md` for the envelope specification and
//! `docs/api-reference.md` for the procedure reference.

pub mod error;
pub mod router;
pub mod wire;

pub use error::TrpcError;
