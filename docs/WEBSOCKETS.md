# Native WebSockets

Tela's QuickJS client supports text and binary WebSockets without application-specific
protocols or endpoints in the SDK. This is a small client transport, not a browser
networking compatibility layer.

```js
const ws = new WebSocket('wss://service.example/socket', ['example.v2'], {
  origin: 'https://client.example'
});
ws.binaryType = 'arraybuffer'; // the only supported mode; also the default
ws.onopen = () => {
  if (ws.protocol !== 'example.v2') { ws.close(); return; }
  ws.send(new Uint8Array([0, 1, 255]));
  ws.send(new Uint8Array([9, 8, 7, 6]).subarray(1, 3)); // sends only 8,7
  ws.send(new Uint8Array([1, 2]).buffer);
  ws.send('text remains text');
};
ws.onmessage = ({data}) => {
  if (typeof data === 'string') console.log(data);
  else console.log(new Uint8Array(data)); // actual ArrayBuffer, not JSON/base64
};
ws.onerror = ({message}) => console.error(message); // fixed, sanitized diagnostic
ws.onclose = () => console.log('closed');
```

## JavaScript contract

- `new WebSocket(url, protocols?, options?)`: URL is a string with `ws:` or
  `wss:`, a host, and no username, password, or fragment. Query strings are allowed
  but never included in diagnostics. URL input is limited to 8,192 bytes.
- Protocols are a string or array of strings. They must be nonempty ASCII HTTP
  tokens with no duplicates (case-sensitive). At most 32 tokens of 256 bytes each.
  The negotiated, validated selection is exposed as read-only `.protocol`.
  A server cannot select an unoffered protocol or multiple protocols. The current
  tungstenite backend also requires a selection when protocols were offered.
- The optional third argument accepts only `{origin: string}`. Origin must be a
  canonical serialized HTTP(S) origin (no trailing slash, path, credentials,
  query, or fragment), or `"null"`, with at most 2,048 bytes. Omission sends no
  Origin header. This nonbrowser option does not authenticate the caller.
  Arbitrary headers, cookies, TLS options, and certificate bypasses are not exposed.
- `send` accepts only a string, `ArrayBuffer`, or `Uint8Array`. Bytes are copied
  synchronously, respecting a view's offset and length. No implicit JSON encoding.
  Detached buffers and unsupported types are rejected. Text size is measured in
  UTF-8 bytes, not JavaScript code units.
- Binary messages are real ArrayBuffers. Only `binaryType = 'arraybuffer'` is
  supported; assigning another mode throws. `onopen`, `onmessage`, `onerror`, and
  `onclose` receive an event with `type` and `target`; message events have `data`,
  error events have a sanitized `message`. Handlers run with the socket as `this`.
- `CONNECTING=0`, `OPEN=1`, `CLOSING=2`, `CLOSED=3` exist on the constructor and
  prototype. `readyState`, `protocol`, and `url` are read-only public properties.
- `send` before open/after close, unsupported data, oversize data, and a full send
  queue throw synchronously. Receive overflow/oversize/transport errors produce
  `error`, then `close`. Dial failure also produces both terminal events.
- `flush()` returns a Promise that resolves only after every `send` accepted before
  that call has completed the ordered transport's `socket.send`/`flush` write. It
  rejects if the connection fails or closes first. It is an OS/TLS write-completion
  barrier, not a peer persistence or delivery acknowledgement; sends after the
  call are not included.
- `close()` is idempotent cancellation, including during DNS/TCP/TLS/HTTP dial.
  It drops the socket and any pending writes rather than waiting for an untrusted
  peer's close handshake; pending `flush()` promises reject. This limited API does
  not expose close codes/reasons, Blob, EventTarget, bufferedAmount, browser
  cookies, redirects, or reconnects.

## Bounds and ownership

Per QuickJS registration/engine registry:

| Resource | Limit |
| --- | --- |
| Connections, including connecting and undrained closed entries | 8 |
| Message and individual frame bytes | 1,048,576 (1 MiB) |
| Outbound queue | 8 messages/barriers, plus the in-flight write |
| Pending or undrained `flush()` results | 8 per connection |
| Native inbound queue | 8 events, plus 2 reserved error/close slots |
| Pending outer action notifications | One payload-free wake per connection |
| Dial deadline | 15 seconds |
| Write/flush deadline | 15 seconds, also cancellation-aware |

Inbound overflow terminates the connection; it never silently drops a message and
continues. Reserved terminal slots ensure overflow can still release the connection.
Closed entries occupy a connection slot until JS drains their terminal events,
preventing connect/close churn from filling the unbounded outer action channel.
Network tasks never retain JS values. A single task owns each socket; cancellation
drops that task's transport future, including any pending write. There are no
separate detached reader/writer tasks. Finalizing the registration's QuickJS
native-function owner cancels all its connections as a fallback. QuickJS may retain
those functions until runtime destruction, even after its Context handle is dropped.
Explicit `shutdown_ws` on engine/context disposal is therefore mandatory; it works
while the QuickJS runtime is still alive.

## Exact engine integration seam

All APIs are in `tela_engine::native::websocket` (the module is already public).

```rust
use tela_engine::native::websocket::{
    register_websocket, register_websocket_with_config, shutdown_ws, WsConfig, WsSenders,
};

// Existing initialization and registration remain source-compatible:
let registry: WsSenders = std::sync::Arc::new(std::sync::Mutex::new(
    std::collections::HashMap::new(),
));
// register_websocket(&ctx, action_tx, registry.clone(), runtime_handle)?;

// On orderly shutdown, error exits, and Engine::drop:
shutdown_ws(&registry);
```

`WsSenders` now maps IDs to opaque `Arc<WsConnection>` handles, not string senders.
The engine must not write or delete individual registry entries itself. Re-export
`shutdown_ws`, `WsConfig`, and `register_websocket_with_config` from `native/mod.rs`
if convenient; no dependency changes are needed for this provider.

**The existing action dispatcher needs no change.** Each native action is:

```json
{"type":"__tela_ws__","id":1,"event":"drain","data":""}
```

Continue calling `__tela_ws_event__(id, event, dataString)` on the owning QuickJS
context. The bootstrap calls native `__tela_ws_pull__(id)` itself, returning a
bounded array of events with direct ArrayBuffer payloads. Acknowledgement and
queue transfer occur atomically; later arrivals schedule a fresh wake. Dispatch
and handler invocation remain on the engine thread. Even if a handler throws,
remaining events in that batch (especially close) are processed before rethrowing.
Do not manufacture one outer action per incoming payload or serialize bytes into
`dataString`. The internal connect/send/pull functions are implementation details.

For a Rust-host-only test/private CA configuration, use:

```rust,ignore
let config = WsConfig {
    connector: Some(tokio_tungstenite::Connector::Rustls(std::sync::Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(test_roots)
            .with_no_client_auth(),
    ))),
};
register_websocket_with_config(&ctx, action_tx, registry.clone(), handle, config)?;
```

The host supplies `test_roots` and must keep normal certificate/hostname validation.
Default registration uses the backend's public WebPKI roots. `Connector::Plain`
is rejected at registration to prevent accidental `wss` downgrade. There is no JS
TLS switch. Hosts constructing a Rustls config need a direct, matching Rustls
0.23 dependency; this provider does not add or refresh manifests/lockfiles. Main
can thread an optional `WsConfig` through engine construction and use a locally
issued localhost/SAN certificate for its owned TLS integration fixture.

## Verification

From `tela/`:

```sh
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX15.4.sdk \
  cargo test --offline --locked -p tela-engine websocket
```

`native/websocket_tests.rs` uses only ephemeral loopback listeners and owned
sockets. Tests exercise QuickJS binary/text roundtrips (including a 262,168-byte
payload), flush-before-close write completion and rejection/bounds, view offsets,
protocol negotiation, Origin, invalid constructors,
message/queue/connection caps, wake coalescing, handler exceptions, redacted
failures, handshake cancellation, explicit shutdown, and JS runtime destruction.
They also verify empty/detached buffers and safe finalization of detached received
buffers (which use QuickJS-owned copies to avoid an rquickjs 0.9 Vec-finalizer bug).
The Rust host's private-CA **TLS fixture is an integration responsibility**; these
provider tests do not contact a live relay or use credentials.
