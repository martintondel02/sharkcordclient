//! Sharkcord data models, ported from `packages/shared` of the upstream
//! TypeScript codebase. These mirror the JSON shapes returned by the
//! tRPC procedures documented in `docs/api-reference.md`.
//!
//! All structs are `serde`-compatible with the server's JSON. Field
//! names use `snake_case` and are annotated with `#[serde(rename_all =
//! "camelCase")]` where the server emits camelCase.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Channel type, from `ChannelType` in `packages/shared/src/types.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ChannelType {
    /// A text channel.
    Text,
    /// A voice channel (carries voice/video/screen).
    Voice,
}

/// What a producer is sending, from `StreamKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamKind {
    /// Microphone audio.
    Audio,
    /// Webcam video.
    Video,
    /// Screen-share video.
    Screen,
    /// Screen-share system audio (paired with [`StreamKind::Screen`]).
    ScreenAudio,
    /// Plugin-produced video (external stream — not used by this client,
    /// but parsed for completeness).
    ExternalVideo,
    /// Plugin-produced audio (external stream).
    ExternalAudio,
}

/// A user's presence, from `UserStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    /// Actively connected.
    Online,
    /// Connected but idle.
    Idle,
    /// Not connected.
    Offline,
}

// ---------------------------------------------------------------------------
// Core entities (server-side joined forms, as returned by tRPC)
// ---------------------------------------------------------------------------

/// A stored file / attachment. Maps to `TFile` in `shared/src/tables.ts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    /// Stable file id.
    pub id: String,
    /// On-disk name (served at `/public/<name>`).
    pub name: String,
    /// Original user-supplied file name.
    pub original_name: String,
    /// MD5 digest.
    pub md5: String,
    /// Owning user id.
    pub user_id: i64,
    /// Size in bytes.
    pub size: i64,
    /// MIME type.
    pub mime_type: String,
    /// File extension (no dot).
    pub extension: String,
    /// Creation timestamp (ms epoch).
    pub created_at: i64,
    /// Last update timestamp (ms epoch).
    pub updated_at: i64,
    /// Signed-URL access token, present only when the server has
    /// `storageSignedUrlsEnabled`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    /// Signed-URL expiry (ms epoch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token_expires_at: Option<i64>,
}

/// A user, joined with avatar/banner files and role ids. Maps to the
/// public form `TJoinedPublicUser` returned to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicUser {
    /// Numeric user id.
    pub id: i64,
    /// Display name.
    pub name: String,
    /// Profile accent color (`#rrggbb`).
    pub profile_color: String,
    /// Free-form bio.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
    /// Avatar file id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_id: Option<String>,
    /// Banner file id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner_id: Option<String>,
    /// Whether the user is banned.
    pub banned: bool,
    /// Account creation timestamp (ms epoch).
    pub created_at: i64,
    /// Avatar file (joined), when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<File>,
    /// Banner file (joined), when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner: Option<File>,
    /// Role ids assigned to the user.
    pub role_ids: Vec<i64>,
    /// Presence, when the server includes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<UserStatus>,
}

/// A role, joined with its permissions. Maps to `TJoinedRole`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub is_persistent: bool,
    pub is_default: bool,
    pub storage_quota_override_enabled: bool,
    pub storage_space_quota: i64,
    pub created_at: i64,
    pub updated_at: i64,
    /// Permission strings (from the `Permission` enum upstream).
    pub permissions: Vec<String>,
}

/// A channel. Maps to `TChannel`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: ChannelType,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    pub private: bool,
    pub is_dm: bool,
    pub position: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A category. Maps to `TCategory`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub position: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A reaction on a message, joined with its custom-emoji file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageReaction {
    pub message_id: i64,
    pub user_id: i64,
    /// Either a unicode emoji string or a custom emoji name.
    pub emoji: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<File>,
}

/// A message, joined with files, reactions, and reply context. Maps to
/// `TJoinedMessage`. `content` is sanitized HTML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: i64,
    /// Sanitized HTML body.
    pub content: String,
    pub user_id: i64,
    /// Plugin id if this message was produced by a plugin, else null.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    pub channel_id: i64,
    /// For thread replies: the id of the parent message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_message_id: Option<i64>,
    /// For direct replies: the id of the replied-to message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_message_id: Option<i64>,
    pub editable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Vec<serde_json::Value>>,
    pub created_at: i64,
    pub updated_at: i64,
    pub pinned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_by: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited_by: Option<i64>,
    pub files: Vec<File>,
    pub reactions: Vec<MessageReaction>,
    /// Thread reply count (present on root messages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_count: Option<i64>,
}

// ---------------------------------------------------------------------------
// Voice
// ---------------------------------------------------------------------------

/// A user's voice-channel state. Maps to `TVoiceUserState`.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)] // mirrors upstream TVoiceUserState
pub struct VoiceUserState {
    /// Microphone muted.
    pub mic_muted: bool,
    /// Speakers deafened (implies mic muted upstream).
    pub sound_muted: bool,
    /// Webcam enabled.
    pub webcam_enabled: bool,
    /// Currently screen-sharing.
    pub sharing_screen: bool,
}

/// mediasoup WebRTC transport parameters returned by the server.
/// Maps to `TTransportParams`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportParams {
    /// Transport id.
    pub id: String,
    /// ICE parameters (opaque mediasoup JSON).
    pub ice_parameters: serde_json::Value,
    /// ICE candidates (opaque mediasoup JSON).
    pub ice_candidates: serde_json::Value,
    /// DTLS parameters (opaque mediasoup JSON).
    pub dtls_parameters: serde_json::Value,
}

/// Result of `voice.getProducers` — the ids of remote producers in the
/// current voice channel, grouped by kind. Maps to `TRemoteProducerIds`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProducerIds {
    pub remote_video_ids: Vec<i64>,
    pub remote_audio_ids: Vec<i64>,
    pub remote_screen_ids: Vec<i64>,
    pub remote_screen_audio_ids: Vec<i64>,
    pub remote_external_stream_ids: Vec<i64>,
}
