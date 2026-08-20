//! Protocol-version compatibility.
//!
//! Sharkcord is alpha and ships breaking changes without a formal API
//! version. This module records the protocol surface the crate was built
//! against ([`crate::TARGET_PROTOCOL_VERSION`]) and will, as upstream
//! evolves, host shims that adapt new/changed procedure shapes so the
//! rest of the core stays stable.
//!
//! The tested-against `sharkord/sharkcord:latest` Docker tag is recorded
//! in `docs/api-reference.md`.
//!
//! _Stub: no shims yet — only one target version is supported._

/// Returns the Sharkcord server protocol version this crate targets.
#[must_use]
pub fn target_version() -> &'static str {
    crate::TARGET_PROTOCOL_VERSION
}
