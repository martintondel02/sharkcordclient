//! `sharkcord-desktop` — Slint desktop app (Linux + Windows) for the
//! unofficial sharkcordclient.
//!
//! Phase 0: a placeholder binary that boots the async runtime and prints
//! the target protocol version, proving the workspace builds end-to-end.
//! Phase 1 replaces this with the Slint window + connect screen.

use sharkcord_core::compat;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!(
        target_protocol_version = compat::target_version(),
        "sharkcord-desktop starting (Phase 0 placeholder; Slint UI lands in Phase 1)"
    );
    tracing::info!("Run a Sharkcord server locally with:");
    tracing::info!(
        "  docker run -p 4991:4991/tcp -p 40000:40000/tcp -p 40000:40000/udp -v ./data:/home/bun/.config/sharkord --name sharkord sharkord/sharkcord:latest"
    );
}
