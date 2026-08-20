//! The realtime state store.
//!
//! Mirrors the official web client's `IServerState` Redux slice: a
//! single in-memory model of the server (channels, categories, users,
//! roles, emojis, messages per channel, voice map, read states, public
//! settings) kept in sync by the tRPC subscriptions.
//!
//! UI layers don't read this directly; instead the core emits coarse
//! change notifications through the FFI callback interface so each
//! platform can drive its own reactive UI (Compose state, Slint props).
//!
//! _Stub: the store struct and apply-event methods land in Phase 1._

use crate::models::{Category, Channel, Message, PublicUser, Role, VoiceUserState};
use std::collections::HashMap;

/// The realtime server state, hydrated from `others.joinServer` and
/// updated by subscriptions.
#[derive(Debug, Clone, Default)]
pub struct ServerState {
    pub server_id: String,
    pub server_name: String,
    pub own_user_id: i64,
    pub categories: Vec<Category>,
    pub channels: Vec<Channel>,
    pub users: Vec<PublicUser>,
    pub roles: Vec<Role>,
    /// Messages per channel (newest-first, matching `messages.get`).
    pub messages_by_channel: HashMap<i64, Vec<Message>>,
    /// Currently selected channel (UI hint; not protocol state).
    pub selected_channel_id: Option<i64>,
    /// Voice state per channel: `channelId -> (userId -> state)`.
    pub voice_map: HashMap<i64, HashMap<i64, VoiceUserState>>,
    /// Last-read message id per channel.
    pub read_states: HashMap<i64, i64>,
    /// Public server settings (opaque for now; typed in Phase 1).
    pub public_settings: Option<serde_json::Value>,
}

impl ServerState {
    /// Find a channel by id.
    #[must_use]
    pub fn channel(&self, id: i64) -> Option<&Channel> {
        self.channels.iter().find(|c| c.id == id)
    }

    /// Find a user by id.
    #[must_use]
    pub fn user(&self, id: i64) -> Option<&PublicUser> {
        self.users.iter().find(|u| u.id == id)
    }
}
