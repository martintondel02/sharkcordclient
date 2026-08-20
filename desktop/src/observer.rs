//! Bridge from the core's async `ClientObserver` callbacks to egui.
//!
//! The core fires observer methods on its own tokio threads. We marshal
//! them into a shared state struct that the UI polls each frame, and
//! request a repaint so the UI updates without busy-polling.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::Context;
use sharkcord_core::ffi::ClientObserver;
use tokio::sync::Mutex;

/// Shared UI-side state written by the observer and read by egui each frame.
#[derive(Default)]
pub struct UiNotifier {
    /// "Something changed" flag — the UI re-reads the snapshot.
    pub changed: AtomicBool,
    /// Whether we are connected.
    pub connected: AtomicBool,
    /// Last disconnect reason, if any.
    pub disconnect_reason: Mutex<Option<String>>,
    /// The egui context, used to request repaints from async threads.
    pub ctx: Mutex<Option<Context>>,
}

impl UiNotifier {
    /// Create a new notifier. The egui context is set by the app on first frame.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Set the egui context (so observer callbacks can request repaints).
    pub async fn set_ctx(&self, ctx: Context) {
        *self.ctx.lock().await = Some(ctx);
    }

    fn flag_changed(&self) {
        self.changed.store(true, Ordering::Relaxed);
        if let Ok(ctx) = self.ctx.try_lock() {
            if let Some(ctx) = ctx.as_ref() {
                ctx.request_repaint();
            }
        }
    }
}

/// The `ClientObserver` impl that forwards into a [`UiNotifier`].
pub struct NotifierObserver {
    notifier: Arc<UiNotifier>,
}

impl NotifierObserver {
    #[must_use]
    pub fn new(notifier: Arc<UiNotifier>) -> Self {
        Self { notifier }
    }
}

impl ClientObserver for NotifierObserver {
    fn on_connected(&self) {
        self.notifier.connected.store(true, Ordering::Relaxed);
        self.notifier.flag_changed();
    }

    fn on_disconnected(&self, code: u16, reason: String) {
        self.notifier.connected.store(false, Ordering::Relaxed);
        // Best-effort store of the reason.
        if let Ok(mut guard) = self.notifier.disconnect_reason.try_lock() {
            *guard = Some(format!("code {code}: {reason}"));
        }
        self.notifier.flag_changed();
    }

    fn on_state_changed(&self) {
        self.notifier.flag_changed();
    }
}
