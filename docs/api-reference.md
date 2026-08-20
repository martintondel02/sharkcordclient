# Sharkcord API reference (tRPC procedures)

The complete procedure surface this client targets, grouped by router.
All procedures except `others.handshake` and `others.joinServer` are
`protectedProcedure` (require a joined/authenticated session).

**Tested against:** `sharkord/sharkcord:latest` (Docker), server version
**0.0.23** (alpha). Sharkcord is alpha and ships breaking changes; see
[`core/src/compat`](../core/src/compat) for version shims. When the
upstream router changes, update this file and the Rust handles in
[`core/src/trpc/router.rs`](../core/src/trpc/router.rs).

Conventions: **Q** = query, **M** = mutation, **S** = subscription.
Input/output type names refer to the Rust structs in
[`core/src/models.rs`](../core/src/models.rs) and
[`core/src/trpc/router.rs`](../core/src/trpc/router.rs).

## Auth & bootstrap

| Path | Type | Input | Output |
| --- | --- | --- | --- |
| `others.handshake` | Q (public, RL) | none | `{ handshakeHash, hasPassword }` |
| `others.joinServer` | Q (RL 5/min, not protected) | `{ handshakeHash, password? }` | `InitialState` — full bootstrap: categories, channels, users, ownUserId, roles, emojis, voiceMap, publicSettings, channelPermissions, readStates, showWelcomeDialog |
| `others.onServerSettingsUpdate` | S | none | `TPublicServerSettings` |

See [`wire-protocol.md`](wire-protocol.md#mandatory-bootstrap) for the
mandatory startup sequence.

## `others.*` — server lifecycle & settings

| Path | Type | Input | Output |
| --- | --- | --- | --- |
| `others.getSettings` | Q | none | `TSettings` (secrets cleared; `MANAGE_SETTINGS`) |
| `others.updateSettings` | M | partial settings | void (`MANAGE_SETTINGS`) |
| `others.changeLogo` | M | `{ fileId? }` (temp image) | void (`MANAGE_SETTINGS`) |
| `others.getStorageSettings` | Q | none | `{ storageSettings, diskMetrics }` (`MANAGE_STORAGE`) |
| `others.getUpdate` | Q | none | `{ canUpdate, latestVersion, hasUpdate, currentVersion }` (`MANAGE_UPDATES`) |
| `others.updateServer` | M | none | void — triggers self-update (`MANAGE_UPDATES`) |
| `others.useSecretToken` | M | `{ token }` | void — grants owner role if token matches |

## `messages.*` — text channels

| Path | Type | Input | Output |
| --- | --- | --- | --- |
| `messages.get` | Q (RL) | `{ channelId, cursor?, targetMessageId?, limit?=100 }` | `{ messages, nextCursor }` — newest-first, cursor on `createdAt` |
| `messages.getOne` | Q | `{ messageId }` | `Message` |
| `messages.getPinned` | Q | `{ channelId }` | `Message[]` |
| `messages.getThread` | Q | `{ parentMessageId, cursor?, limit?=100 }` | `{ messages, nextCursor }` — oldest-first, cursor `gt(createdAt)` |
| `messages.send` | M (RL) | `{ content, channelId, files?=[], parentMessageId?, replyToMessageId? }` | `number` (new id) |
| `messages.edit` | M (RL) | `{ messageId, content }` | void |
| `messages.delete` | M | `{ messageId }` | void (owner or `MANAGE_MESSAGES`) |
| `messages.toggleReaction` | M (RL) | `{ messageId, emoji }` | void (`REACT_TO_MESSAGES`) |
| `messages.togglePin` | M | `{ messageId }` | void (`PIN_MESSAGES`; root messages only) |
| `messages.signalTyping` | M (RL) | `{ channelId, parentMessageId? }` | void (`SEND_MESSAGES`) |
| `messages.search` | Q (RL) | `{ query }` (2..24) | `{ messages, files }` (requires `enableSearch`) |
| `messages.onNew` | S | none | `Message` |
| `messages.onUpdate` | S | none | `Message` |
| `messages.onDelete` | S | none | `{ messageId, channelId }` |
| `messages.onTyping` | S | none | `{ channelId, userId, parentMessageId? }` |
| `messages.onThreadReplyCountUpdate` | S | none | `{ messageId, channelId, replyCount }` |

## `channels.*` / `categories.*` / `dms.*`

| Path | Type | Input | Output |
| --- | --- | --- | --- |
| `channels.get` | Q | `{ channelId }` | `Channel` (`MANAGE_CHANNELS`) |
| `channels.add` | M | `{ type, name, categoryId }` | `number` (`MANAGE_CHANNELS`) |
| `channels.update` | M | `{ channelId, name?, topic?, private? }` | void |
| `channels.delete` | M | `{ channelId }` | void |
| `channels.reorder` | M | `{ categoryId, channelIds }` | void |
| `channels.getPermissions` | M | `{ channelId }` | `{ rolePermissions, userPermissions }` |
| `channels.updatePermissions` | M | `{ channelId, userId?, roleId?, isCreate?, permissions }` | void |
| `channels.deletePermissions` | M | `{ channelId, userId?, roleId? }` | void |
| `channels.markAsRead` | M (RL) | `{ channelId }` | void |
| `channels.onCreate` / `onDelete` / `onUpdate` | S | none | `Channel` / `number` / `Channel` |
| `channels.onPermissionsUpdate` | S | none | channel-permissions map |
| `channels.onReadStateUpdate` | S | none | `{ channelId, count }` |
| `channels.onReadStateDelta` | S | none | `{ channelId, delta }` |
| `categories.get/add/update/delete/reorder` | Q/M | see `router.rs` | id / void |
| `categories.onCreate/onDelete/onUpdate` | S | none | `Category` / `number` / `Category` |
| `dms.get` | Q | none | `TDirectMessageConversation[]` (`directMessagesEnabled`) |
| `dms.open` | M (RL) | `{ userId }` | `{ channelId }` |
| `dms.onConversationOpen` | S | none | `{ channelId }` |

## `users.*` / `roles.*` / `emojis.*` / `invites.*` / `files.*`

| Path | Type | Input | Output |
| --- | --- | --- | --- |
| `users.getAll` | Q | none | `TUser[]` (`MANAGE_USERS`) |
| `users.getInfo` | Q | `{ userId }` | `{ user, logins, files, messages, storage }` |
| `users.update` | M | `{ name, profileColor, bio? }` | void (self) |
| `users.updatePassword` | M | `{ currentPassword, newPassword, confirmNewPassword }` | void |
| `users.changeAvatar` / `changeBanner` | M | `{ fileId? }` | void (self) |
| `users.addRole` / `removeRole` | M | `{ userId, roleId }` | void (`MANAGE_USERS`) |
| `users.kick` | M | `{ userId, reason? }` | void (closes WS `40000`) |
| `users.ban` / `unban` | M | `{ userId, reason? }` | void |
| `users.delete` | M | `{ userId, wipe? }` | void |
| `users.onJoin/onLeave/onUpdate/onCreate` | S | none | `PublicUser` / `number` / `PublicUser` / `PublicUser` |
| `users.onDelete` | S | none | `{ isWipe, userId, deletedUserId }` |
| `roles.getAll/add/update/delete/setDefault` | Q/M | see `router.rs` | id / void / `Role` |
| `roles.onCreate/onUpdate/onDelete` | S | none | `Role` / `Role` / `number` |
| `emojis.getAll/add/update/delete` | Q/M | see `router.rs` | `TJoinedEmoji[]` / void |
| `emojis.onCreate/onUpdate/onDelete` | S | none | `TJoinedEmoji` / `TJoinedEmoji` / `number` |
| `invites.getAll/add/delete` | Q/M | see `router.rs` | `TInvite[]` / `TInvite` / void |
| `files.delete` | M | `{ fileId }` | void (owner or `MANAGE_MESSAGES`) |
| `files.deleteTemporary` | M | `{ fileId }` | void (own temp file cleanup) |

## `voice.*` — voice state + mediasoup signaling

Voice-channel lifecycle (non-media):

| Path | Type | Input | Output |
| --- | --- | --- | --- |
| `voice.join` | M (RL) | `{ channelId, state }` | `{ routerRtpCapabilities }` (`JOIN_VOICE_CHANNELS`) |
| `voice.leave` | M | none | void |
| `voice.moveUser` | M (RL) | `{ userId, channelId }` | void (`MOVE_MEMBERS`) |
| `voice.updateState` | M | `{ micMuted?, soundMuted?, webcamEnabled?, sharingScreen? }` | void |
| `voice.getProducers` | Q | none | `RemoteProducerIds` |
| `voice.onJoin` / `onLeave` | S | none | `{ channelId, userId, state }` / `{ channelId, userId }` |
| `voice.onUpdateState` | S | none | `{ channelId, userId, state }` |
| `voice.onMoved` | S | none | `{ channelId, fromChannelId }` |
| `voice.onAddExternalStream` / `onUpdateExternalStream` / `onRemoveExternalStream` | S | none | external-stream events (plugin-produced; parsed but not produced by this client) |

Mediasoup signaling (opaque JSON for mediasoup params):

| Path | Type | Input | Output |
| --- | --- | --- | --- |
| `voice.createProducerTransport` | M | none | `TransportParams` |
| `voice.connectProducerTransport` | M | `{ dtlsParameters }` | void |
| `voice.produce` | M | `{ transportId, kind, rtpParameters, qualityLayers? }` | `string` (producer id) |
| `voice.closeProducer` | M | `{ kind }` | void |
| `voice.createConsumerTransport` | M | none | `TransportParams` |
| `voice.connectConsumerTransport` | M | `{ dtlsParameters }` | void |
| `voice.consume` | M | `{ kind, remoteId, rtpCapabilities }` | `{ producerId, consumerId, consumerKind, consumerRtpParameters, consumerType, qualityLayers? }` |
| `voice.setConsumerQuality` | M | `{ remoteId, kind, quality }` | void (simulcast only) |
| `voice.onNewProducer` | S | none | `{ channelId, remoteId, kind }` |
| `voice.onProducerClosed` | S | none | `{ channelId, remoteId, kind }` |

### Mediasoup signaling flow

**Produce (send):** `voice.join` → load `Device` with
`routerRtpCapabilities` → `voice.createProducerTransport` → create local
`WebRtcTransport` → on transport `connect`, call
`voice.connectProducerTransport({ dtlsParameters })` →
`transport.produce({ track })` triggers `voice.produce({ transportId,
kind, rtpParameters, qualityLayers? })` → returns `producerId`.

**Consume (recv):** `voice.createConsumerTransport` → wire `connect` →
`voice.connectConsumerTransport` → `voice.getProducers` (or
`voice.onNewProducer` events) → `voice.consume({ kind, remoteId,
rtpCapabilities })` → `transport.consume({ id: consumerId, producerId,
rtpParameters })` → play the track. On `voice.onProducerClosed`, close
the matching consumer.

**Simulcast** (`qualityLayers` on `produce`, `setConsumerQuality`) is
gated by the `webRtcSimulcastEnabled` server setting. If off, omit
`qualityLayers` and don't call `setConsumerQuality`.

Codecs configured server-side: **Opus** (audio), **VP8 / VP9 / H264 /
AV1** (video).

## `plugins.*` — intentionally unsupported

This client does **not** implement the `plugins.*` router. The upstream
plugin system dynamically `import()`s JS/React bundles and renders them
in-process, which is not portable to a native client. Plugin slash
commands and plugin UI components are both out of scope.

## Permissions (server-enforced)

Two-tier model. The client doesn't enforce, but should hide UI that
would just get `FORBIDDEN`:

- **Global** `Permission` (role-based; owner role id `1` bypasses all).
- **Per-channel** `ChannelPermission` (DMs always grant all to
  participants; private channels require explicit grants).

Full enum values live in `packages/shared/src/statics/permissions.ts`
upstream; we carry the ones we render as needed in
[`core/src/trpc/router.rs`](../core/src/trpc/router.rs).