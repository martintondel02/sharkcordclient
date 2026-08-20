//! Typed handles for every tRPC procedure this client calls.
//!
//! Each submodule corresponds to one upstream router (`others`, `messages`,
//! …). The constants are the dotted procedure paths; the input/output
//! types live alongside them so the FFI facade and the WS client stay in
//! sync with `docs/api-reference.md`.
//!
//! Procedures under the `plugins` router are intentionally absent — this
//! client does not support the upstream plugin system (it loads JS/React
//! bundles in-process, which is not portable to a native client).

// ---------------------------------------------------------------------------
// others.*
// ---------------------------------------------------------------------------

/// `others.handshake` and `others.joinServer` — the mandatory bootstrap.
pub mod others {
    use serde::{Deserialize, Serialize};

    /// `others.handshake` — public, rate-limited. No input.
    pub const HANDSHAKE: &str = "others.handshake";

    /// `others.joinServer` — establishes the authenticated session and
    /// returns the full initial server state. Not a `protectedProcedure`.
    pub const JOIN_SERVER: &str = "others.joinServer";

    /// `others.onServerSettingsUpdate` subscription.
    pub const ON_SERVER_SETTINGS_UPDATE: &str = "others.onServerSettingsUpdate";

    /// Input to [`JOIN_SERVER`].
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct JoinServerInput {
        /// Hash returned by [`HANDSHAKE`].
        pub handshake_hash: String,
        /// Server password, only when `hasPassword` was true.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub password: Option<String>,
    }

    /// Output of [`HANDSHAKE`].
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct HandshakeOutput {
        pub handshake_hash: String,
        pub has_password: bool,
    }

    /// Output of [`JOIN_SERVER`] — the entire initial server state.
    ///
    /// This is the bootstrap payload; the core hydrates its state store
    /// directly from it. Fields mirror the upstream `joinServer` return.
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct InitialState {
        pub categories: Vec<crate::models::Category>,
        pub channels: Vec<crate::models::Channel>,
        pub users: Vec<crate::models::PublicUser>,
        pub server_id: String,
        pub server_name: String,
        pub own_user_id: i64,
        pub roles: Vec<crate::models::Role>,
        /// Server-wide voice state map: `{ channelId: { users: { userId: state } } }`.
        pub voice_map: serde_json::Value,
        /// Custom emojis (joined form).
        pub emojis: serde_json::Value,
        /// Public server settings.
        pub public_settings: serde_json::Value,
        /// Per-channel permission overrides.
        pub channel_permissions: serde_json::Value,
        /// `{ channelId: lastReadMessageId }`.
        pub read_states: serde_json::Value,
        pub show_welcome_dialog: bool,
    }
}

// ---------------------------------------------------------------------------
// messages.*
// ---------------------------------------------------------------------------

/// `messages.*` — text-channel messages, reactions, threads, search.
pub mod messages {
    use serde::{Deserialize, Serialize};

    pub const GET: &str = "messages.get";
    pub const GET_ONE: &str = "messages.getOne";
    pub const GET_PINNED: &str = "messages.getPinned";
    pub const GET_THREAD: &str = "messages.getThread";
    pub const SEND: &str = "messages.send";
    pub const EDIT: &str = "messages.edit";
    pub const DELETE: &str = "messages.delete";
    pub const TOGGLE_REACTION: &str = "messages.toggleReaction";
    pub const TOGGLE_PIN: &str = "messages.togglePin";
    pub const SIGNAL_TYPING: &str = "messages.signalTyping";
    pub const SEARCH: &str = "messages.search";

    // Subscriptions
    pub const ON_NEW: &str = "messages.onNew";
    pub const ON_UPDATE: &str = "messages.onUpdate";
    pub const ON_DELETE: &str = "messages.onDelete";
    pub const ON_TYPING: &str = "messages.onTyping";
    pub const ON_THREAD_REPLY_COUNT_UPDATE: &str = "messages.onThreadReplyCountUpdate";

    /// Input to [`SEND`].
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SendInput {
        pub content: String,
        pub channel_id: i64,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        pub files: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub parent_message_id: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub reply_to_message_id: Option<i64>,
    }

    /// Input to [`GET`] (paginated, newest-first, cursor on `createdAt`).
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct GetInput {
        pub channel_id: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub cursor: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub target_message_id: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub limit: Option<u32>,
    }

    /// Output of [`GET`] / [`GET_THREAD`].
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Page {
        pub messages: Vec<crate::models::Message>,
        pub next_cursor: Option<i64>,
    }
}

// ---------------------------------------------------------------------------
// channels.* / categories.* / dms.*
// ---------------------------------------------------------------------------

/// `channels.*` — channel CRUD, permissions, read state.
pub mod channels {
    pub const GET: &str = "channels.get";
    pub const ADD: &str = "channels.add";
    pub const UPDATE: &str = "channels.update";
    pub const DELETE: &str = "channels.delete";
    pub const REORDER: &str = "channels.reorder";
    pub const GET_PERMISSIONS: &str = "channels.getPermissions";
    pub const UPDATE_PERMISSIONS: &str = "channels.updatePermissions";
    pub const DELETE_PERMISSIONS: &str = "channels.deletePermissions";
    pub const MARK_AS_READ: &str = "channels.markAsRead";

    pub const ON_CREATE: &str = "channels.onCreate";
    pub const ON_DELETE: &str = "channels.onDelete";
    pub const ON_UPDATE: &str = "channels.onUpdate";
    pub const ON_PERMISSIONS_UPDATE: &str = "channels.onPermissionsUpdate";
    pub const ON_READ_STATE_UPDATE: &str = "channels.onReadStateUpdate";
    pub const ON_READ_STATE_DELTA: &str = "channels.onReadStateDelta";
}

/// `categories.*` — channel grouping.
pub mod categories {
    pub const GET: &str = "categories.get";
    pub const ADD: &str = "categories.add";
    pub const UPDATE: &str = "categories.update";
    pub const DELETE: &str = "categories.delete";
    pub const REORDER: &str = "categories.reorder";

    pub const ON_CREATE: &str = "categories.onCreate";
    pub const ON_DELETE: &str = "categories.onDelete";
    pub const ON_UPDATE: &str = "categories.onUpdate";
}

/// `dms.*` — direct messages.
pub mod dms {
    use serde::{Deserialize, Serialize};

    pub const GET: &str = "dms.get";
    pub const OPEN: &str = "dms.open";
    pub const ON_CONVERSATION_OPEN: &str = "dms.onConversationOpen";

    /// Input to [`OPEN`].
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct OpenInput {
        pub user_id: i64,
    }

    /// Output of [`OPEN`].
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct OpenOutput {
        pub channel_id: i64,
    }
}

// ---------------------------------------------------------------------------
// users.* / roles.* / emojis.* / invites.* / files.*
// ---------------------------------------------------------------------------

/// `users.*` — profile, admin (kick/ban/role), presence events.
pub mod users {
    pub const GET_ALL: &str = "users.getAll";
    pub const GET_INFO: &str = "users.getInfo";
    pub const UPDATE: &str = "users.update";
    pub const UPDATE_PASSWORD: &str = "users.updatePassword";
    pub const CHANGE_AVATAR: &str = "users.changeAvatar";
    pub const CHANGE_BANNER: &str = "users.changeBanner";
    pub const ADD_ROLE: &str = "users.addRole";
    pub const REMOVE_ROLE: &str = "users.removeRole";
    pub const KICK: &str = "users.kick";
    pub const BAN: &str = "users.ban";
    pub const UNBAN: &str = "users.unban";
    pub const DELETE: &str = "users.delete";

    pub const ON_JOIN: &str = "users.onJoin";
    pub const ON_LEAVE: &str = "users.onLeave";
    pub const ON_UPDATE: &str = "users.onUpdate";
    pub const ON_CREATE: &str = "users.onCreate";
    pub const ON_DELETE: &str = "users.onDelete";
}

/// `roles.*` — role CRUD and permissions.
pub mod roles {
    pub const GET_ALL: &str = "roles.getAll";
    pub const ADD: &str = "roles.add";
    pub const UPDATE: &str = "roles.update";
    pub const DELETE: &str = "roles.delete";
    pub const SET_DEFAULT: &str = "roles.setDefault";

    pub const ON_CREATE: &str = "roles.onCreate";
    pub const ON_UPDATE: &str = "roles.onUpdate";
    pub const ON_DELETE: &str = "roles.onDelete";
}

/// `emojis.*` — custom server emojis.
pub mod emojis {
    pub const GET_ALL: &str = "emojis.getAll";
    pub const ADD: &str = "emojis.add";
    pub const UPDATE: &str = "emojis.update";
    pub const DELETE: &str = "emojis.delete";

    pub const ON_CREATE: &str = "emojis.onCreate";
    pub const ON_UPDATE: &str = "emojis.onUpdate";
    pub const ON_DELETE: &str = "emojis.onDelete";
}

/// `invites.*` — invite codes.
pub mod invites {
    pub const GET_ALL: &str = "invites.getAll";
    pub const ADD: &str = "invites.add";
    pub const DELETE: &str = "invites.delete";
}

/// `files.*` — delete an attached message file or an unattached temp file.
pub mod files {
    use serde::{Deserialize, Serialize};

    pub const DELETE: &str = "files.delete";
    pub const DELETE_TEMPORARY: &str = "files.deleteTemporary";

    /// Input to [`DELETE_TEMPORARY`].
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct DeleteTemporaryInput {
        pub file_id: String,
    }
}

// ---------------------------------------------------------------------------
// voice.*
// ---------------------------------------------------------------------------

/// `voice.*` — voice-channel state and mediasoup WebRTC signaling.
///
/// The non-media procedures (`join`, `leave`, `moveUser`, `updateState`,
/// `getProducers`) plus the subscription events drive the
/// [`crate::voice`] engine. The signaling procedures
/// (`createProducerTransport`, `connectProducerTransport`, `produce`,
/// `closeProducer`, `createConsumerTransport`, `connectConsumerTransport`,
/// `consume`, `setConsumerQuality`) carry opaque mediasoup JSON.
pub mod voice {
    use serde::{Deserialize, Serialize};

    // Non-media
    pub const JOIN: &str = "voice.join";
    pub const LEAVE: &str = "voice.leave";
    pub const MOVE_USER: &str = "voice.moveUser";
    pub const UPDATE_STATE: &str = "voice.updateState";
    pub const GET_PRODUCERS: &str = "voice.getProducers";

    // Signaling (producer / send side)
    pub const CREATE_PRODUCER_TRANSPORT: &str = "voice.createProducerTransport";
    pub const CONNECT_PRODUCER_TRANSPORT: &str = "voice.connectProducerTransport";
    pub const PRODUCE: &str = "voice.produce";
    pub const CLOSE_PRODUCER: &str = "voice.closeProducer";

    // Signaling (consumer / recv side)
    pub const CREATE_CONSUMER_TRANSPORT: &str = "voice.createConsumerTransport";
    pub const CONNECT_CONSUMER_TRANSPORT: &str = "voice.connectConsumerTransport";
    pub const CONSUME: &str = "voice.consume";
    pub const SET_CONSUMER_QUALITY: &str = "voice.setConsumerQuality";

    // Subscriptions — voice-channel state
    pub const ON_JOIN: &str = "voice.onJoin";
    pub const ON_LEAVE: &str = "voice.onLeave";
    pub const ON_UPDATE_STATE: &str = "voice.onUpdateState";
    pub const ON_MOVED: &str = "voice.onMoved";
    pub const ON_ADD_EXTERNAL_STREAM: &str = "voice.onAddExternalStream";
    pub const ON_UPDATE_EXTERNAL_STREAM: &str = "voice.onUpdateExternalStream";
    pub const ON_REMOVE_EXTERNAL_STREAM: &str = "voice.onRemoveExternalStream";

    // Subscriptions — mediasoup producers
    pub const ON_NEW_PRODUCER: &str = "voice.onNewProducer";
    pub const ON_PRODUCER_CLOSED: &str = "voice.onProducerClosed";

    /// Input to [`JOIN`].
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct JoinInput {
        pub channel_id: i64,
        pub state: crate::models::VoiceUserState,
    }

    /// Output of [`JOIN`] — the mediasoup router RTP capabilities, fed
    /// into the local `Device.load`.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct JoinOutput {
        pub router_rtp_capabilities: serde_json::Value,
    }

    /// Input to [`PRODUCE`].
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ProduceInput {
        pub transport_id: String,
        pub kind: crate::models::StreamKind,
        /// Opaque mediasoup RTP parameters from the local producer.
        pub rtp_parameters: serde_json::Value,
        /// Simulcast layers, video-only and only when the server has
        /// `webRtcSimulcastEnabled`.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub quality_layers: Option<Vec<serde_json::Value>>,
    }

    /// Input to [`CONSUME`].
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ConsumeInput {
        pub kind: crate::models::StreamKind,
        /// Remote producer id.
        pub remote_id: i64,
        /// Local device RTP capabilities.
        pub rtp_capabilities: serde_json::Value,
    }

    /// Output of [`CONSUME`].
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ConsumeOutput {
        pub producer_id: String,
        pub consumer_id: String,
        pub consumer_kind: crate::models::StreamKind,
        pub consumer_rtp_parameters: serde_json::Value,
        pub consumer_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub quality_layers: Option<serde_json::Value>,
    }
}
