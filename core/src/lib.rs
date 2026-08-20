//! `sharkcord-core` — the shared, UI-less heart of sharkcordclient.
//!
//! This crate implements everything a native client needs to talk to a
//! [Sharkcord](https://github.com/Sharkord/sharkcord) server, without
//! pulling in any UI framework:
//!
//! - [`trpc`] — a client for Sharkcord's tRPC-over-WebSocket protocol
//!   (queries, mutations, and live subscriptions), with JWT auth in the
//!   WS connection params and a keep-alive matching the server
//!   (ping 30 s / pong 5 s).
//! - [`auth`] — the `POST /login` flow and secure token storage
//!   abstraction.
//! - [`bootstrap`] — the mandatory `others.handshake` →
//!   `others.joinServer` startup sequence that hydrates the entire
//!   initial server state.
//! - [`state`] — the realtime store mirroring the official web client's
//!   `IServerState`, fed by tRPC subscriptions.
//! - [`files`] — the `POST /upload` HTTP flow and signed file URLs.
//! - [`voice`] — the mediasoup WebRTC voice engine: signaling against
//!   the `voice` tRPC router plus the media plane (transports, producers,
//!   consumers, simulcast, quality layers).
//! - [`platform`] — a capture abstraction (mic / webcam / screen) that
//!   each UI target implements (Android `MediaProjection`/`CameraX`,
//!   `Linux` `PipeWire`, `Windows` `WGC`/`WASAPI`).
//! - [`ffi`] — the high-level facade exposed to the Kotlin (`UniFFI`) and
//!   `Slint` front-ends via callback interfaces.
//!
//! The crate is `#![deny(unsafe_code)]` and targets `no_std`-friendly IO
//! where practical; it carries no UI dependencies so it can be embedded
//! in a native Android `.so` or a desktop binary unchanged.
//!
//! See `docs/api-reference.md` for the full tRPC procedure surface this
//! crate targets, and `docs/wire-protocol.md` for the envelope spec.

#![forbid(unsafe_code)]

/// tRPC-over-WebSocket client (wire protocol, router handles, errors).
pub mod trpc;

/// `POST /login`, JWT handling, and the secure token-storage trait.
pub mod auth;

/// The `handshake` → `joinServer` bootstrap that establishes a session
/// and returns the full initial server state.
pub mod bootstrap;

/// Protocol-version compatibility shims — keeps the crate working across
/// Sharkcord server alpha releases.
pub mod compat;

/// File upload (`POST /upload`) and signed file URLs.
pub mod files;

/// The FFI facade exposed to Kotlin (UniFFI) and Slint.
pub mod ffi;

/// Platform capture abstraction: mic, webcam, and screen.
pub mod platform;

/// The realtime state store, fed by tRPC subscriptions.
pub mod state;

/// The mediasoup WebRTC voice engine.
pub mod voice;

/// Re-exported Sharkcord data models (channels, messages, users, …).
pub mod models;

/// Shared error type.
pub mod error;

pub use error::{CoreError, CoreResult};

/// Semantic version of the Sharkcord server protocol this crate was
/// built and tested against. Used by [`compat`] to negotiate.
///
/// Sharkcord is alpha and ships breaking changes; bump this and add a
/// compat shim when the upstream router changes. The tested-against
/// `sharkord/sharkord:latest` Docker tag is recorded in
/// `docs/api-reference.md`.
pub const TARGET_PROTOCOL_VERSION: &str = "0.0.23";
