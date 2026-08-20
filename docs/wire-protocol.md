# tRPC-over-WebSocket wire protocol

Sharkcord exposes its entire API as a single tRPC v11 router over **one
WebSocket connection**. There is no HTTP tRPC link and no OpenAPI/JSON
schema export. This document describes the envelope shapes a client must
send and receive. It is derived from the [tRPC RPC spec][trpc-rpc] and
the upstream server's `@trpc/server/adapters/ws` usage
(`apps/server/src/utils/wss.ts`).

[trpc-rpc]: https://trpc.io/docs/v11/rpc

## Connection

- URL: `ws(s)://<host>:4991/` (same origin as the HTTP server).
- Subprotocol: none.
- **Connection params** (sent in the WS opening handshake): `{ "token": <JWT> }`.
  The server's `createContext` validates the JWT and rejects with
  `UNAUTHORIZED` if invalid/banned.
- Additionally, send a raw WS text message `{"data":{"token":"<JWT>"}}`
  immediately after connect — the server reads this once to map the
  socket to a userId for close/kick handling. (The reference client does
  this implicitly; sending both is safest.)
- **Keep-alive**: ping every 30 s, expect pong within 5 s. The server
  enforces the same. On failure, tear down and reconnect with backoff.
- **Close codes**: `1006` unexpected, `40000` kicked, `40001` banned,
  `40002` server shutdown.

All queries, mutations, and subscriptions multiplex over this single
socket, correlated by a client-chosen `id`.

## Requests (client → server)

A request is a JSON message:

```jsonc
{
  "id": "<client-chosen unique id>",
  "jsonrpc": "2.0",
  "method": "query" | "mutation" | "subscription" | "subscriptionStop",
  "params": {
    "path": "<router>.<procedure>",          // e.g. "messages.send"
    "input": <json value>                    // omitted when no input
  }
}
```

- `method`:
  - `query` — idempotent read.
  - `mutation` — write.
  - `subscription` — open a streaming subscription.
  - `subscriptionStop` — stop an open subscription; sent with the same
    `id` as the original `subscription` request. `params.path` may be
    empty for this method.
- `params.input` is the JSON-serialized value matching the procedure's
  zod schema. When the procedure takes no input, **omit** `input`
  (do not send `null`).

## Responses (server → client)

### Success — query / mutation

```jsonc
{
  "id": "<matches request>",
  "jsonrpc": "2.0",
  "result": {
    "data": <procedure output>
  }
}
```

(no `result.type` field for query/mutation.)

### Success — subscription

Subscription responses carry a `result.type` discriminant:

```jsonc
// 1. subscription accepted
{ "id": "<sub id>", "jsonrpc": "2.0", "result": { "type": "initial", "data": null } }

// 2. event
{ "id": "<sub id>", "jsonrpc": "2.0", "result": { "type": "data", "data": <event payload> } }

// 3. subscription ended cleanly
{ "id": "<sub id>", "jsonrpc": "2.0", "result": { "type": "stopped", "data": null } }
```

Events keep streaming until the client sends `subscriptionStop` or the
observable completes.

### Error

```jsonc
{
  "id": "<matches request>",
  "jsonrpc": "2.0",
  "error": {
    "json": {
      "message": "<human message>",
      "code": <json-rpc numeric code>,        // e.g. -32003
      "data": {
        "code": "<string code>",             // e.g. "FORBIDDEN"
        "httpStatus": <int>,                 // e.g. 403
        "path": "<procedure path>"           // optional
      }
    }
  }
}
```

### Error code mapping (numeric → string)

| numeric | string code               |
|--------:|---------------------------|
| -32700  | `PARSE_ERROR`             |
| -32600  | `BAD_REQUEST`             |
| -32603  | `INTERNAL_SERVER_ERROR` (also `NOT_IMPLEMENTED`, `BAD_GATEWAY`, `SERVICE_UNAVAILABLE`, `GATEWAY_TIMEOUT`) |
| -32001  | `UNAUTHORIZED`            |
| -32002  | `PAYMENT_REQUIRED`        |
| -32003  | `FORBIDDEN`               |
| -32004  | `NOT_FOUND`               |
| -32005  | `METHOD_NOT_SUPPORTED`    |
| -32008  | `TIMEOUT`                 |
| -32009  | `CONFLICT`                |
| -32012  | `PRECONDITION_FAILED`     |
| -32013  | `PAYLOAD_TOO_LARGE`       |
| -32015  | `UNSUPPORTED_MEDIA_TYPE`  |
| -32022  | `UNPROCESSABLE_CONTENT`   |
| -32028  | `PRECONDITION_REQUIRED`   |
| -32029  | `TOO_MANY_REQUESTS`       |
| -32099  | `CLIENT_CLOSED_REQUEST`   |

Unknown numeric codes fall back to `INTERNAL_SERVER_ERROR`.

## Mandatory bootstrap

Being authenticated on the WS is **not** enough — `ctx.authenticated`
starts `false` and is only set `true` by `others.joinServer`. The
required startup sequence is:

1. `POST /login` (HTTP) → JWT.
2. Open the WS with connection params `{ "token": <JWT> }` (and send the
   raw `{ "data": { "token": ... } }` first message).
3. `query` `others.handshake` (public, rate-limited) →
   `{ handshakeHash, hasPassword }`.
4. If `hasPassword`, collect the server password from the user.
5. `query` `others.joinServer({ handshakeHash, password? })` (rate-limited
   5/min, **not** a protected procedure) → flips `ctx.authenticated =
   true` and returns the **entire initial server state** (channels,
   categories, users, roles, emojis, voice map, read states, channel
   permissions, public settings). Use this as the bootstrap payload.
6. Open the subscriptions you care about (`messages.onNew`, etc.).

Skipping any of these causes `UNAUTHORIZED` on protected procedures.

## Non-tRPC HTTP endpoints

A few things live outside tRPC (plain HTTP on the same port):

| Method | Path                | Auth                | Purpose |
|--------|---------------------|---------------------|---------|
| POST   | `/login`            | none (IP rate-limited) | JWT login/register. Body `{ identity, password, invite? }` → `{ success, token }`. |
| POST   | `/upload`           | `x-token` header    | File upload. Headers `x-file-name`, `x-file-type`, `content-length`, `x-token`. Returns a temp file id. |
| GET    | `/info`             | none                | `{ serverId, version, name, description, logo, allowNewUsers }`. |
| GET    | `/healthz`          | none                | Health check. |
| GET    | `/public/<name>`    | none (signed URL token in query when enabled) | Serve a stored file. |
| GET    | `/manifest.json`    | none                | Web app manifest (not used by native clients). |

## Implementation notes for a non-TS client

- The wire format is plain JSON over WS; no TypeScript runtime is
  required. Reproduce the zod input shapes yourself (the server
  re-validates and rejects with `BAD_REQUEST`).
- For voice procedures, `dtlsParameters`, `rtpParameters`,
  `rtpCapabilities`, and `routerRtpCapabilities` are **opaque mediasoup
  JSON objects** — pass them straight through from your mediasoup
  client library.
- `messages.get` / `messages.getThread` carry `.meta({ infinite: true })`
  upstream — this is a client-side React Query hint; the server returns
  `{ messages, nextCursor }` either way. `nextCursor` is a `createdAt`
  ms epoch (or `null` when done).
- Cursor pagination uses `createdAt`, **not** message ids:
  `messages.get` uses `lt(createdAt, cursor)` (newest-first, paginate
  backward); `messages.getThread` uses `gt(createdAt, cursor)`
  (oldest-first).

The Rust implementation of this protocol lives in
[`core/src/trpc`](../core/src/trpc).