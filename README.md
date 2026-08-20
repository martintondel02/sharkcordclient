# sharkcordclient

An **unofficial**, open-source, **native** client for
[Sharkcord](https://github.com/Sharkcord/sharkord) — the lightweight,
self-hosted, open-source chat platform with text, voice, video, and
screen sharing.

> **Not affiliated with the Sharkord project.** See [NOTICE](NOTICE).
> Sharkcord is in alpha; this client tracks a tested server version and
> may break when the upstream protocol changes. See
> [Compatibility](#compatibility).

## Goals

Bring a real native client to Sharkcord on platforms the official web
client doesn't cover well natively:

- **Android** (Kotlin + Jetpack Compose)
- **Linux** desktop (Slint UI, PipeWire capture)
- **Windows** desktop (Slint UI, WGC/WASAPI capture)

Full text **and** voice / video / screen sharing in the first release.
No plugin support (the upstream plugin system loads JS/React bundles
in-process, which is not portable to a native client).

## Architecture

A shared **Rust core** (`sharkcord-core`) implements everything that
isn't UI:

- the **tRPC-over-WebSocket** wire protocol (Sharkcord's sole RPC
  contract — there is no REST/OpenAPI for app logic),
- JWT login + the `handshake` → `joinServer` bootstrap,
- the realtime state store fed by all tRPC subscriptions,
- file upload / signed file URLs over HTTP,
- the **mediasoup** WebRTC voice engine (signaling + media plane),
- a per-platform media-capture abstraction.

Native UIs are thin and consume the core:

| Platform | UI | Binding |
| --- | --- | --- |
| Android | Kotlin / Jetpack Compose | UniFFI (`libsharkcord_core.so`) |
| Linux / Windows | [egui](https://github.com/emilk/egui) (Rust, immediate-mode, MIT) | direct |

See [`docs/architecture.md`](docs/architecture.md) for the full design,
[`docs/api-reference.md`](docs/api-reference.md) for the complete tRPC
procedure reference this client targets, and
[`docs/wire-protocol.md`](docs/wire-protocol.md) for the tRPC-over-WS
envelope specification.

## Repository layout

```
sharkcordclient/
├─ core/       # sharkcord-core (shared Rust crate)
├─ desktop/    # egui desktop app (Linux + Windows)
├─ android/    # Kotlin/Compose Android app (planned)
├─ docs/       # protocol + design documentation
└─ scripts/    # build helpers
```

## Status

**Working (Phase 1 + 2):**

- **Core** — tRPC-over-WebSocket client (real, with keepalive and
  reconnect-friendly design), JWT login over HTTP, the mandatory
  `handshake` → `joinServer` bootstrap, the realtime state store fed by
  all text/voice-presence subscriptions, file upload + signed file
  URLs, and the full **mediasoup voice signaling** layer (every `voice.*`
  mutation/query typed; media plane abstracted behind a `MediaEngine`
  trait).
- **Desktop (Linux/Windows)** — egui connect screen, server view with
  channel sidebar + text channel (message list, composer, reactions),
  voice panel with join/leave and mic/sound/webcam/screen controls.

**Not yet wired (needs live-server validation):**

- The actual WebRTC media plane (the `voice-webrtc` feature gates
  webrtc-rs; signaling is complete, RTP send/receive needs a running
  Sharkord server to validate against).
- Platform capture (Linux PipeWire / Windows WGC/WASAPI) — Phase 3.
- Android app — Phase 4.
- HTML message rendering is currently stripped to plain text; a proper
  HTML→native renderer lands later.

See [`docs/architecture.md`](docs/architecture.md) for the milestone/
phase plan and current status.

## Compatibility

Tested against Sharkord server `sharkord/sharkcord:latest` (Docker).
Because Sharkcord is alpha, expect protocol breaks; the
[`core/src/compat`](core/src/compat) module will version the protocol
surface as upstream evolves. The tested-against server version is
recorded in [`docs/api-reference.md`](docs/api-reference.md).

## Development

### Prerequisites

- Rust ≥ 1.85 (pinned via `rust-toolchain.toml`).
- Docker, for running a local Sharkord server for integration tests
  (`sharkord/sharkord:latest`). (Not required for `cargo check`/unit
  tests.)

### Run a local server

```sh
docker run -p 4991:4991/tcp -p 40000:40000/tcp -p 40000:40000/udp \
  -v ./data:/home/bun/.config/sharkord \
  --name sharkord sharkord/sharkord:latest
```

On first launch the server prints an **owner token** to its logs —
store it securely; it grants owner access.

### Build the core

```sh
cargo check --workspace
cargo test  --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

### Run the desktop app

```sh
cargo run -p sharkcord-desktop --release
```

### Android

Android tooling and build instructions land in Phase 4.

## Contributing

Contributions are welcome. Please open an issue first for non-trivial
changes. Follow the conventions in `docs/architecture.md` and keep
`cargo clippy` / `cargo fmt` clean.

## License

MIT — see [LICENSE](LICENSE). Upstream Sharkcord is also MIT-licensed;
attribution in [NOTICE](NOTICE).