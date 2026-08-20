//! The egui app: connect screen and server view (sidebar + text channel).

use std::sync::Arc;

use eframe::egui;
use eframe::egui::RichText;
use sharkcord_core::ffi::{Client, ConnectionState};
use sharkcord_core::models::{ChannelType, Message};

use crate::observer::{NotifierObserver, UiNotifier};

/// The egui app.
pub struct App {
    /// The core client (shared, cloneable).
    client: Client,
    /// UI notifier bridging observer callbacks to egui repaints.
    notifier: Arc<UiNotifier>,
    /// Connect-form state.
    form: ConnectForm,
    /// Cached snapshot rendered this frame.
    snapshot: Option<sharkcord_core::state::ServerState>,
    /// Composer text for the selected channel.
    composer: String,
    /// Whether the connect is in flight (disable the form).
    connecting: bool,
    /// Last error to show in the connect screen.
    error: Option<String>,
    /// Scroll handle for the message list.
    scroll_to_bottom: bool,
    /// Whether we're currently in a voice channel (UI-side mirror of core state).
    in_voice: bool,
}

#[derive(Clone)]
struct ConnectForm {
    origin: String,
    identity: String,
    password: String,
    invite: String,
    server_password: String,
}

impl Default for ConnectForm {
    fn default() -> Self {
        Self {
            origin: "http://localhost:4991".to_owned(),
            identity: String::new(),
            password: String::new(),
            invite: String::new(),
            server_password: String::new(),
        }
    }
}

impl App {
    /// Construct the app and wire the observer.
    pub(crate) fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let client = Client::new();
        let notifier = UiNotifier::new();
        let observer = Arc::new(NotifierObserver::new(Arc::clone(&notifier)));
        client.set_observer(observer);
        Self {
            client,
            notifier,
            form: ConnectForm::default(),
            snapshot: None,
            composer: String::new(),
            connecting: false,
            error: None,
            scroll_to_bottom: false,
            in_voice: false,
        }
    }

    /// Kick off the connect on the core's runtime.
    fn do_connect(&mut self) {
        self.connecting = true;
        self.error = None;
        let client = self.client.clone();
        let form = self.form.clone();
        let server_password = if form.server_password.is_empty() {
            None
        } else {
            Some(form.server_password.as_str())
        };
        let invite = if form.invite.is_empty() {
            None
        } else {
            Some(form.invite.as_str())
        };
        let result = self.client.block_on(async move {
            client
                .connect(
                    &form.origin,
                    &form.identity,
                    &form.password,
                    invite,
                    server_password,
                )
                .await
        });
        match result {
            Ok(()) => {
                self.connecting = false;
            }
            Err(err) => {
                self.connecting = false;
                self.error = Some(format!("{err}"));
            }
        }
    }

    fn do_disconnect(&mut self) {
        self.client.block_on(self.client.disconnect());
        self.in_voice = false;
    }

    /// Render the connect screen.
    fn view_connect(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.heading("sharkcordclient");
                ui.label("Unofficial native client for Sharkcord");
                ui.add_space(20.0);
            });

            egui::Grid::new("connect_form")
                .num_columns(2)
                .spacing([8.0, 8.0])
                .min_col_width(120.0)
                .show(ui, |ui| {
                    ui.label("Server");
                    ui.text_edit_singleline(&mut self.form.origin);
                    ui.end_row();

                    ui.label("Username");
                    ui.text_edit_singleline(&mut self.form.identity);
                    ui.end_row();

                    ui.label("Password");
                    ui.text_edit_singleline(&mut self.form.password);
                    ui.end_row();

                    ui.label("Invite (optional)");
                    ui.text_edit_singleline(&mut self.form.invite);
                    ui.end_row();

                    ui.label("Server password (optional)");
                    ui.text_edit_singleline(&mut self.form.server_password);
                    ui.end_row();
                });

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let enabled = !self.connecting
                    && !self.form.origin.is_empty()
                    && !self.form.identity.is_empty()
                    && !self.form.password.is_empty();
                if ui.add_enabled(enabled, egui::Button::new("Connect")).clicked() {
                    self.do_connect();
                }
                if self.connecting {
                    ui.spinner();
                    ui.label("Connecting…");
                }
            });

            if let Some(err) = &self.error {
                ui.add_space(8.0);
                ui.colored_label(egui::Color32::from_rgb(220, 60, 60), format!("Error: {err}"));
            }

            ui.add_space(24.0);
            ui.label(
                RichText::new(
                    "Start a local server with: docker run -p 4991:4991/tcp -p 40000:40000/tcp -p 40000:40000/udp -v ./data:/home/bun/.config/sharkord --name sharkord sharkord/sharkcord:latest",
                )
                .small()
                .color(egui::Color32::from_gray(120)),
            );
        });
    }

    /// Render the main server view: sidebar (categories/channels) + text channel.
    #[allow(clippy::too_many_lines)] // UI render functions are naturally long
    fn view_server(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let snapshot = self.snapshot.clone().unwrap_or_default();
        let own_id = snapshot.own_user_id;

        // Left sidebar: server name + channels.
        egui::SidePanel::left("sidebar")
            .min_width(200.0)
            .default_width(240.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.heading(&snapshot.server_name);
                ui.label(
                    RichText::new(format!("{} channels", snapshot.channels.len()))
                        .small()
                        .color(egui::Color32::from_gray(140)),
                );
                ui.separator();

                // Categories (or uncategorized channels).
                let channels = snapshot.channels.clone();
                let categories = snapshot.categories.clone();
                let previous = snapshot.selected_channel_id;
                let mut selected = previous;

                if categories.is_empty() {
                    for ch in &channels {
                        if ch.kind == ChannelType::Text {
                            selected = channel_button(ui, ch.id, &ch.name, selected);
                        }
                    }
                } else {
                    for cat in &categories {
                        ui.collapsing(&cat.name, |ui| {
                            for ch in channels.iter().filter(|c| {
                                c.category_id == Some(cat.id) && c.kind == ChannelType::Text
                            }) {
                                selected = channel_button(ui, ch.id, &ch.name, selected);
                            }
                        });
                    }
                    // Uncategorized.
                    let uncategorized: Vec<_> = channels
                        .iter()
                        .filter(|c| c.category_id.is_none() && c.kind == ChannelType::Text)
                        .collect();
                    if !uncategorized.is_empty() {
                        ui.collapsing("Uncategorized", |ui| {
                            for ch in uncategorized {
                                selected = channel_button(ui, ch.id, &ch.name, selected);
                            }
                        });
                    }
                }

                // If the user clicked a different channel, tell the core to
                // select + load messages.
                if selected != previous {
                    let id = selected.unwrap();
                    let client = self.client.clone();
                    self.client.block_on(async move {
                        let _ = client.select_channel(id).await;
                    });
                    self.scroll_to_bottom = true;
                }

                ui.separator();
                ui.label(
                    RichText::new(format!("{} users", snapshot.users.len()))
                        .small()
                        .color(egui::Color32::from_gray(140)),
                );

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    if ui.button("Disconnect").clicked() {
                        self.do_disconnect();
                    }
                });
            });

        // Right: voice channels + voice controls.
        egui::SidePanel::right("voice")
            .min_width(140.0)
            .default_width(180.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Voice");
                ui.separator();

                // Voice controls (only meaningful when connected).
                if self.in_voice {
                    ui.horizontal(|ui| {
                        if ui.button("🎤 Mic").clicked() {
                            let _ = self.client.block_on(self.client.toggle_mic());
                        }
                        if ui.button("🔊 Sound").clicked() {
                            let _ = self.client.block_on(self.client.toggle_sound());
                        }
                        if ui.button("🎥 Webcam").clicked() {
                            let _ = self.client.block_on(self.client.toggle_webcam());
                        }
                        if ui.button("🖥 Screen").clicked() {
                            let _ = self.client.block_on(self.client.toggle_screen_share());
                        }
                    });
                    if ui.button("Leave voice").clicked() {
                        let _ = self.client.block_on(self.client.leave_voice());
                        self.in_voice = false;
                    }
                    ui.separator();
                } else if let Some(channel_id) = snapshot.selected_channel_id {
                    let is_voice_channel = snapshot
                        .channel(channel_id)
                        .is_some_and(|c| c.kind == ChannelType::Voice);
                    if is_voice_channel && ui.button("Join voice channel").clicked() {
                        let result = self.client.block_on(self.client.join_voice(channel_id));
                        if result.is_ok() {
                            self.in_voice = true;
                        } else if let Err(err) = result {
                            self.error = Some(format!("join voice failed: {err}"));
                        }
                    }
                }

                if snapshot.voice_map.is_empty() {
                    ui.label(
                        RichText::new("No active voice channels")
                            .small()
                            .color(egui::Color32::from_gray(140)),
                    );
                } else {
                    for (channel_id, users) in &snapshot.voice_map {
                        ui.label(format!("Channel {channel_id}"));
                        for (user_id, state) in users {
                            let name = snapshot
                                .user(*user_id)
                                .map_or_else(|| format!("user {user_id}"), |u| u.name.clone());
                            let icon = if state.mic_muted { "🔇" } else { "🎤" };
                            ui.label(format!("{icon} {name}"));
                        }
                        ui.separator();
                    }
                }
            });

        // Central: the selected text channel.
        egui::CentralPanel::default().show(ctx, |ui| {
            let channel_id = snapshot.selected_channel_id;
            let channel_name = channel_id.map_or_else(
                || "Select a channel".to_owned(),
                |id| {
                    snapshot
                        .channel(id)
                        .map(|c| c.name.clone())
                        .unwrap_or_default()
                },
            );

            ui.heading(&channel_name);
            ui.separator();

            let messages = channel_id
                .and_then(|id| snapshot.messages(id))
                .unwrap_or(&[]);

            // Message list (scroll area).
            let scroll_id = format!("messages_{channel_id:?}");
            egui::ScrollArea::vertical()
                .id_source(scroll_id)
                .stick_to_bottom(self.scroll_to_bottom)
                .show(ui, |ui| {
                    if messages.is_empty() {
                        ui.label(
                            RichText::new("No messages yet").color(egui::Color32::from_gray(140)),
                        );
                    } else {
                        // Render oldest-last (messages are newest-first in state).
                        for msg in messages.iter().rev() {
                            render_message(ui, msg, &snapshot, own_id);
                            ui.separator();
                        }
                    }
                });
            self.scroll_to_bottom = false;

            // Composer.
            ui.separator();
            ui.horizontal(|ui| {
                let can_send = channel_id.is_some() && !self.composer.is_empty();
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.composer)
                        .hint_text("Message #channel")
                        .desired_width(ui.available_width() - 80.0)
                        .interactive(can_send),
                );
                let enter_pressed =
                    response.lost_focus() && ui.input(|r| r.key_pressed(egui::Key::Enter));
                if (ui
                    .add_enabled(can_send, egui::Button::new("Send"))
                    .clicked()
                    || enter_pressed)
                    && can_send
                {
                    let channel_id = channel_id.unwrap();
                    let text = std::mem::take(&mut self.composer);
                    let client = self.client.clone();
                    let result = self.client.block_on(async move {
                        client.send_message(channel_id, &text, None).await
                    });
                    if let Err(err) = result {
                        self.error = Some(format!("send failed: {err}"));
                    }
                    self.scroll_to_bottom = true;
                }
            });
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Register the egui context with the notifier so async callbacks
        // can request repaints.
        if self.snapshot.is_none() {
            let notifier = Arc::clone(&self.notifier);
            let ctx_clone = ctx.clone();
            self.client.block_on(async move {
                notifier.set_ctx(ctx_clone).await;
            });
        }

        // Poll the changed flag and refresh the snapshot if needed.
        if self
            .notifier
            .changed
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            self.snapshot = Some(self.client.block_on(self.client.snapshot()));
        }

        // Determine which view to show.
        let conn = self
            .snapshot
            .as_ref()
            .map_or(ConnectionState::Disconnected, |s| s.connection);

        match conn {
            ConnectionState::Connected => self.view_server(ctx, frame),
            ConnectionState::Connecting => {
                self.view_connect(ctx, frame);
            }
            ConnectionState::Disconnected => self.view_connect(ctx, frame),
        }

        // Keep requesting repaints while connecting so the spinner updates.
        if self.connecting {
            ctx.request_repaint();
        }
    }
}

/// Render a channel select button; returns the newly selected id if clicked.
fn channel_button(ui: &mut egui::Ui, id: i64, name: &str, selected: Option<i64>) -> Option<i64> {
    let is_selected = selected == Some(id);
    let button = if is_selected {
        egui::Button::new(format!("# {name}")).fill(egui::Color32::from_gray(50))
    } else {
        egui::Button::new(format!("# {name}"))
    };
    if ui.add(button).clicked() {
        Some(id)
    } else {
        selected
    }
}

/// Render one message.
fn render_message(
    ui: &mut egui::Ui,
    msg: &Message,
    state: &sharkcord_core::state::ServerState,
    _own_id: i64,
) {
    let author = state
        .user(msg.user_id)
        .map_or_else(|| format!("user {}", msg.user_id), |u| u.name.clone());
    let time = format_timestamp(msg.created_at);
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(&author)
                .strong()
                .color(egui::Color32::from_rgb(120, 180, 255)),
        );
        ui.label(
            RichText::new(time)
                .small()
                .color(egui::Color32::from_gray(140)),
        );
    });
    // Content is sanitized HTML; we strip tags to plain text for v1.
    let plain = strip_html(&msg.content);
    ui.label(&plain);
    if !msg.reactions.is_empty() {
        ui.horizontal(|ui| {
            for r in &msg.reactions {
                ui.label(&r.emoji);
            }
        });
    }
}

/// Strip HTML tags to plain text (v1; a real HTML renderer lands later).
fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    // Decode a few common entities.
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
}

/// Format a ms-epoch timestamp as HH:MM.
fn format_timestamp(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let secs = ms / 1000;
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    format!("{h:02}:{m:02}")
}
