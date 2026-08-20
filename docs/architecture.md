# Architecture

`sharkcordclient` is an unofficial native client for
[Sharkcord](https://github.com/Sharkord/sharkcord), targeting Android,
Linux, and Windows, with full text and voice/video/screen-share.

## Design principles

1. **One core, many UIs.** A single Rust crate (`sharkcord-core`) holds
   all non-UI logic: the tRPC-over-WS protocol, auth, state, files, and
   the mediasoup voice engine. UIs are thin and consume it.
2. **Native, not web.** No WebView for app UI. Android uses Kotlin +
   Jetpack Compose; desktop uses Slint (Rust). Voice/video use native
   WebRTC and platform capture APIs.
3. **No plugins.** The upstream plugin system loads JS/React bundles
   in-process and is not portable. We don't implement the `plugins.*`
   router at all.
4. **Track upstream, don't fork.** Sharkcord is alpha and ships breaking
   changes. A `compat` module versions the protocol surface; the
   tested-against server version is recorded in
   [`api-reference.md`](api-reference.md).
5. **Safe Rust.** `#![forbid(unsafe_code)]` in the core. Any `unsafe`
   needed for FFI/platform bindings lives in well-audited dependencies.

## Layer diagram

```
 ┌─────────────┐  ┌─────────────────────┐
 │  Android UI │  │   Desktop UI (Slint)│
 │ Kotlin/Comp │  │   Linux + Windows   │
 └──────┬──────┘  └──────────┬──────────┘
        │ UniFFI (jniLibs)   │ direct
        ▼                    ▼
 ┌───────────────────────────────────────┐
 │            sharkcord-core             │
 │  ┌─────┐  ┌──────┐  ┌──────┐  ┌────┐ │
 │  │ ffi │  │trpc  │  │state │  │voice│ │
 │  │facade│ │ WS   │  │store │  │engine│ │
 │  └─────┘  └──┬───┘  └──┬───┘  └──┬──┘ │
 │             │         │          │    │
 │  ┌──────┐ ┌─┴────┐ ┌──┴────┐ ┌───┴──┐│
 │  │ auth │ │ boot │ │ files │ │platform││
 │  │strap │ │ strap│ │ (HTTP)│ │capture││
 │  └──────┘ └──────┘ └──────┘ └──────┘│
 └────────────────────┬──────────────────┘
                      │ one WebSocket (tRPC) + HTTP (/login, /upload, /public)
                      ▼
              ┌───────────────┐
              │ Sharkcord     │  ws://host:4991  (tRPC + subscriptions)
              │ server (alpha)│  udp/tcp :40000  (mediasoup WebRTC media)
              └───────────────┘
```

## Components

### `core` — `sharkcord-core` (Rust)

| Module | Responsibility |
| --- | --- |
| `trpc` | tRPC v11 wire protocol over WS (envelopes, correlation, keep-alive, error mapping); typed procedure handles grouped by router. |
| `auth` | `POST /login`, JWT handling, `TokenStore` trait (platform secure storage). |
| `bootstrap` | Mandatory `handshake` → `joinServer` sequence; hydrates initial state. |
| `state` | Realtime store mirroring the web client's `IServerState`, updated by subscriptions. |
| `files` | `POST /upload` (headers + `x-token`), signed file URLs. |
| `voice` | mediasoup signaling + WebRTC media plane (transports, producers, consumers, simulcast, quality). |
| `platform` | `PlatformCapture` / `CaptureTrack` traits: mic, webcam, screen. |
| `ffi` | High-level `Client` facade + `ClientObserver` callback interface for UIs. |
| `compat` | Protocol-version shims as upstream evolves. |
| `models` | Serde structs for all Sharkcord data shapes (channels, messages, users, voice…). |
| `error` | `CoreError` / `CoreResult`. |

Async runtime: `tokio` (multi-thread). The single WS connection
multiplexes queries, mutations, and subscriptions; the voice engine's
WebRTC IO runs on the same runtime.

### `desktop` — Slint (Linux + Windows)

A single binary per platform. Slint `.slint` files describe the UI
(connect screen, sidebar, channel view, voice grid); `src/main.rs`
wires Slint callbacks to `sharkcord-core`. Platform capture:
- **Linux**: PipeWire (screen + system audio) + PulseAudio/PipeWire mic.
- **Windows**: Windows Graphics Capture (screen) + WASAPI (mic + loopback).

Packaging: AppImage/Flatpak/deb (Linux), MSI (Windows).

### `android` — Kotlin + Jetpack Compose (Phase 4)

`sharkcord-core` cross-compiles to `libsharkcord_core.so` per ABI;
UniFFI generates Kotlin bindings. Capture via `AudioRecord`, `CameraX`,
`MediaProjection`. Notifications via local notifications (FCM optional).
Distribute: signed AAB; aim for F-Droid.

## Voice engine design

The hardest part. Port of the web client's `VoiceProvider` + transport
hooks to Rust:

1. `voice.join` → `routerRtpCapabilities` → load local mediasoup `Device`.
2. `voice.createProducerTransport` → local `WebRtcTransport` (ICE + DTLS
   + SRTP sender) via **webrtc-rs** (pure-Rust) or, if it proves
   insufficient for mediasoup's SRTP requirements, Google's libwebrtc
   via `webrtc-sys`.
3. Capture via `PlatformCapture`; `voice.produce` mic/webcam/screen with
   simulcast encodings when `webRtcSimulcastEnabled`.
4. `voice.createConsumerTransport` + `voice.getProducers` /
   `voice.onNewProducer` → `voice.consume` → spawn `Consumer`s; decoded
   frames handed to the platform renderer.
5. `voice.updateState` (mute/deafen/webcam/screenshare);
   `voice.setConsumerQuality` (simulcast layer selection).
6. Cleanup on `voice.leave` / channel change / WS close.

Audio processing uses WebRTC's built-in noise suppression / AGC / AEC
(the "standard" path the web client already supports). The
RNNoise/DTLN/AudioWorklet DSP paths are out of scope for v1.

Codecs: Opus (audio), VP8/VP9/H264/AV1 (video). Default to
non-simulcast; enable simulcast only when server setting + platform
support both confirm.

## Message rendering

Server-sanitized HTML (`sanitizeMessageHtml`) is parsed into a small
native AST in the core (text, `<b>/<i>/<s>/<code>/<pre>/<br>`,
`<a href>`, mentions, channel refs, emoji `<img>`, media, OpenGraph
previews from `message.metadata`) and serialized over FFI; each UI
renders it natively. No WebView.

## State & events

The core owns the realtime state and applies subscription events to it
serially. UIs receive **coarse** change notifications via
`ClientObserver` (not per-event IPC) and re-read snapshots — keeps IPC
chatter low across the FFI boundary on Android.

## Phasing (v1 = full feature set, internally derisked)

| Phase | Scope | Status |
| --- | --- | --- |
| 0 | Scaffolding: workspace, LICENSE/NOTICE/README, docs, CI. | **in progress** |
| 1 | Core: tRPC WS client, auth, bootstrap, state, subscriptions, files. Desktop Slint connect + text channel. Validate against local Docker server. | pending |
| 2 | Core: mediasoup voice engine (signaling + webrtc-rs transport, mic-only loopback). Biggest risk — prototype before UI work. | pending |
| 3 | Desktop voice: platform capture (mic/webcam/screen), Slint voice grid + controls. | pending |
| 4 | Android: UniFFI bindings, Compose UI (all screens), Android capture, voice UI. | pending |
| 5 | Polish + packaging: notifications, reconnect, settings, i18n, F-Droid/Flatpak/MSI. | pending |

## Risks & mitigations

| Risk | Mitigation |
| --- | --- |
| No official Rust mediasoup-client. | Implement the thin transport layer on webrtc-rs; fallback to libwebrtc via `webrtc-sys` if needed. Signaling contract is fully known. Prototype in Phase 2 before committing UI work. |
| Sharkcord alpha protocol breaks. | `compat` module versions the protocol; pin to a tested server tag; track upstream. |
| Simulcast/codec per platform. | Default non-simulcast; enable only when server + platform both support. Prefer VP8/H264. |
| Screen + system audio capture differences. | Linux: PipeWire (both). Windows: WGC + WASAPI loopback. Android: MediaProjection (system audio varies by version — document). |
| tRPC subscription backpressure. | Apply events to state serially in the core; UI gets coarse notifications, not per-event. |

## Testing

- **Unit**: wire protocol ser/de, zod-shape validation, state reducers,
  file URL building, mediasoup message shapes (in `core`).
- **Integration** (Rust): spin up `sharkcord/sharkord:latest` in Docker
  in CI; run core through login → join → send/receive → join voice →
  produce/consume against a second client.
- **E2E**: native UI smoke tests (Android emulator + Slint headless).