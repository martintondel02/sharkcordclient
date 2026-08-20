//! The realtime state store.
//!
//! Mirrors the official web client's `IServerState` Redux slice: a
//! single in-memory model of the server (channels, categories, users,
//! roles, emojis, messages per channel, voice map, read states, public
//! settings) kept in sync by the tRPC subscriptions.
//!
//! UI layers don't read this directly; instead the core emits coarse
//! change notifications through the FFI callback interface so each
//! platform can drive its own reactive UI (Compose state, egui state).
//!
//! The store is `Arc<RwLock<ServerState>>` so the FFI facade and the
//! subscription runner can share it. Apply methods take `&mut self`.

use crate::models::{Category, Channel, Message, PublicUser, Role, VoiceUserState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// A shared, lock-protected server state.
pub type SharedState = Arc<RwLock<ServerState>>;

use std::sync::Arc;

/// The realtime server state, hydrated from `others.joinServer` and
/// updated by subscriptions.
#[derive(Debug, Clone, Default)]
pub struct ServerState {
    /// Server id (stable across reconnects).
    pub server_id: String,
    /// Server display name.
    pub server_name: String,
    /// The current user's id.
    pub own_user_id: i64,
    /// Channel categories, ordered by position.
    pub categories: Vec<Category>,
    /// All channels (text + voice), ordered by position.
    pub channels: Vec<Channel>,
    /// All known users.
    pub users: Vec<PublicUser>,
    /// All roles.
    pub roles: Vec<Role>,
    /// Messages per channel (newest-first, matching `messages.get`).
    pub messages_by_channel: HashMap<i64, Vec<Message>>,
    /// Currently selected channel (UI hint; not protocol state).
    pub selected_channel_id: Option<i64>,
    /// Voice state per channel: `channelId -> (userId -> state)`.
    pub voice_map: HashMap<i64, HashMap<i64, VoiceUserState>>,
    /// Last-read message id per channel.
    pub read_states: HashMap<i64, i64>,
    /// Public server settings (opaque for now; typed as needed).
    pub public_settings: Option<serde_json::Value>,
    /// Currently typing users per channel: `channelId -> (userId -> deadline)`.
    pub typing: HashMap<i64, HashMap<i64, std::time::Instant>>,
    /// The connection state for UI rendering.
    pub connection: ConnectionState,
}

/// Coarse connection state surfaced to the UI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected.
    #[default]
    Disconnected,
    /// Connecting / authenticating.
    Connecting,
    /// Connected and joined.
    Connected,
}

/// The set of high-level "what changed" hints the store emits so the UI
/// can decide what to re-render without re-reading everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)] // intentional bitfield of what changed
pub struct ChangeSet {
    pub channels: bool,
    pub users: bool,
    pub roles: bool,
    pub categories: bool,
    pub messages: bool,
    pub voice: bool,
    pub read_states: bool,
    pub settings: bool,
    pub connection: bool,
}

impl ChangeSet {
    /// Merge two changesets (OR the flags).
    pub fn merge(&mut self, other: ChangeSet) {
        self.channels |= other.channels;
        self.users |= other.users;
        self.roles |= other.roles;
        self.categories |= other.categories;
        self.messages |= other.messages;
        self.voice |= other.voice;
        self.read_states |= other.read_states;
        self.settings |= other.settings;
        self.connection |= other.connection;
    }
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

    /// Messages in a channel (newest-first), if loaded.
    #[must_use]
    pub fn messages(&self, channel_id: i64) -> Option<&[Message]> {
        self.messages_by_channel.get(&channel_id).map(Vec::as_slice)
    }

    // -----------------------------------------------------------------
    // Apply methods (called by the subscription runner). Each returns a
    // ChangeSet describing what changed.
    // -----------------------------------------------------------------

    /// Hydrate from `others.joinServer`.
    pub fn apply_initial(&mut self, init: crate::trpc::router::others::InitialState) -> ChangeSet {
        self.server_id = init.server_id;
        self.server_name = init.server_name;
        self.own_user_id = init.own_user_id;
        self.categories = init.categories;
        self.channels = init.channels;
        self.users = init.users;
        self.roles = init.roles;
        self.public_settings = Some(init.public_settings);
        self.connection = ConnectionState::Connected;
        ChangeSet {
            channels: true,
            users: true,
            roles: true,
            categories: true,
            settings: true,
            connection: true,
            ..Default::default()
        }
    }

    /// A new message arrived (or was sent by us).
    pub fn apply_message_new(&mut self, msg: Message) -> ChangeSet {
        let msgs = self.messages_by_channel.entry(msg.channel_id).or_default();
        // Insert newest-first; dedupe by id.
        if !msgs.iter().any(|m| m.id == msg.id) {
            msgs.insert(0, msg);
        }
        ChangeSet {
            messages: true,
            ..Default::default()
        }
    }

    /// A message was edited.
    pub fn apply_message_update(&mut self, msg: Message) -> ChangeSet {
        if let Some(msgs) = self.messages_by_channel.get_mut(&msg.channel_id) {
            if let Some(existing) = msgs.iter_mut().find(|m| m.id == msg.id) {
                *existing = msg;
                return ChangeSet {
                    messages: true,
                    ..Default::default()
                };
            }
        }
        ChangeSet::default()
    }

    /// A message was deleted.
    pub fn apply_message_delete(&mut self, channel_id: i64, message_id: i64) -> ChangeSet {
        if let Some(msgs) = self.messages_by_channel.get_mut(&channel_id) {
            let before = msgs.len();
            msgs.retain(|m| m.id != message_id);
            if msgs.len() != before {
                return ChangeSet {
                    messages: true,
                    ..Default::default()
                };
            }
        }
        ChangeSet::default()
    }

    /// A user started typing in a channel.
    pub fn apply_typing(&mut self, channel_id: i64, user_id: i64) -> ChangeSet {
        self.typing
            .entry(channel_id)
            .or_default()
            .insert(user_id, std::time::Instant::now());
        ChangeSet {
            messages: true,
            ..Default::default()
        }
    }

    /// A channel was created/updated/deleted.
    pub fn apply_channel_create(&mut self, ch: Channel) -> ChangeSet {
        if self.channels.iter().all(|c| c.id != ch.id) {
            self.channels.push(ch);
            ChangeSet {
                channels: true,
                ..Default::default()
            }
        } else {
            ChangeSet::default()
        }
    }
    pub fn apply_channel_update(&mut self, ch: Channel) -> ChangeSet {
        if let Some(existing) = self.channels.iter_mut().find(|c| c.id == ch.id) {
            *existing = ch;
            return ChangeSet {
                channels: true,
                ..Default::default()
            };
        }
        ChangeSet::default()
    }
    pub fn apply_channel_delete(&mut self, id: i64) -> ChangeSet {
        let before = self.channels.len();
        self.channels.retain(|c| c.id != id);
        self.messages_by_channel.remove(&id);
        if self.channels.len() == before {
            ChangeSet::default()
        } else {
            ChangeSet {
                channels: true,
                messages: true,
                ..Default::default()
            }
        }
    }

    /// A user joined/was created/updated/left.
    pub fn apply_user_join(&mut self, user: PublicUser) -> ChangeSet {
        if self.users.iter().all(|u| u.id != user.id) {
            self.users.push(user);
            ChangeSet {
                users: true,
                ..Default::default()
            }
        } else {
            ChangeSet::default()
        }
    }
    pub fn apply_user_update(&mut self, user: PublicUser) -> ChangeSet {
        if let Some(existing) = self.users.iter_mut().find(|u| u.id == user.id) {
            *existing = user;
            return ChangeSet {
                users: true,
                ..Default::default()
            };
        }
        ChangeSet::default()
    }
    pub fn apply_user_leave(&mut self, id: i64) -> ChangeSet {
        // The server reports leave but doesn't delete; we keep the user.
        ChangeSet::default()
    }
    pub fn apply_user_create(&mut self, user: PublicUser) -> ChangeSet {
        self.apply_user_join(user)
    }
    pub fn apply_user_delete(&mut self, user_id: i64) -> ChangeSet {
        let before = self.users.len();
        self.users.retain(|u| u.id != user_id);
        if self.users.len() == before {
            ChangeSet::default()
        } else {
            ChangeSet {
                users: true,
                ..Default::default()
            }
        }
    }

    /// Voice channel state events.
    pub fn apply_voice_join(
        &mut self,
        channel_id: i64,
        user_id: i64,
        state: VoiceUserState,
    ) -> ChangeSet {
        self.voice_map
            .entry(channel_id)
            .or_default()
            .insert(user_id, state);
        ChangeSet {
            voice: true,
            ..Default::default()
        }
    }
    pub fn apply_voice_leave(&mut self, channel_id: i64, user_id: i64) -> ChangeSet {
        if let Some(ch) = self.voice_map.get_mut(&channel_id) {
            ch.remove(&user_id);
            if ch.is_empty() {
                self.voice_map.remove(&channel_id);
            }
        }
        ChangeSet {
            voice: true,
            ..Default::default()
        }
    }
    pub fn apply_voice_state(
        &mut self,
        channel_id: i64,
        user_id: i64,
        state: VoiceUserState,
    ) -> ChangeSet {
        self.voice_map
            .entry(channel_id)
            .or_default()
            .insert(user_id, state);
        ChangeSet {
            voice: true,
            ..Default::default()
        }
    }

    /// Set the selected channel (UI hint).
    pub fn set_selected_channel(&mut self, id: Option<i64>) -> ChangeSet {
        self.selected_channel_id = id;
        ChangeSet::default()
    }

    /// Set connection state.
    pub fn set_connection(&mut self, state: ConnectionState) -> ChangeSet {
        self.connection = state;
        ChangeSet {
            connection: true,
            ..Default::default()
        }
    }

    /// Prune stale typing indicators (older than 5s).
    pub fn prune_typing(&mut self) -> ChangeSet {
        let now = std::time::Instant::now();
        let mut changed = false;
        for ch in self.typing.values_mut() {
            let before = ch.len();
            ch.retain(|_, deadline| now.duration_since(*deadline).as_secs() < 5);
            if ch.len() != before {
                changed = true;
            }
        }
        self.typing.retain(|_, ch| !ch.is_empty());
        if changed {
            ChangeSet {
                messages: true,
                ..Default::default()
            }
        } else {
            ChangeSet::default()
        }
    }
}

/// A message-deletion event payload from `messages.onDelete`.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageDeleteEvent {
    pub message_id: i64,
    pub channel_id: i64,
}

/// A typing event payload from `messages.onTyping`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypingEvent {
    pub channel_id: i64,
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_message_id: Option<i64>,
}

/// A voice-state event payload from `voice.onJoin`/`onUpdateState`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStateEvent {
    pub channel_id: i64,
    pub user_id: i64,
    pub state: VoiceUserState,
}

/// A voice-leave event payload from `voice.onLeave`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceLeaveEvent {
    pub channel_id: i64,
    pub user_id: i64,
}
