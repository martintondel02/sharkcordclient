//! `sharkcord-desktop` — egui desktop app (Linux + Windows) for the
//! unofficial sharkcordclient.
//!
//! Phase 1: connect screen + server view (channel sidebar + text channel
//! with message list and composer). Voice (Phase 2) is wired next.

use eframe::egui;
use sharkcord_core::ffi::Client;

mod observer;
mod ui;

/// Entry point.
fn main() -> Result<(), eframe::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([640.0, 420.0])
            .with_title("sharkcordclient"),
        ..Default::default()
    };

    eframe::run_native(
        "sharkcordclient",
        options,
        Box::new(|cc| Ok(Box::new(ui::App::new(cc)))),
    )
}

/// Convenience alias used by the UI module.
pub(crate) type DesktopClient = Client;
