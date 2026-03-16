# Native API

JavaScript APIs provided by the Tela engine. These are injected as globals into the QuickJS runtime, backed by Rust implementations.

QuickJS has no built-in I/O. Everything here is provided by the engine.

## Permissions

APIs are gated by the `permissions` array in `manifest.json`:

```json
{
  "permissions": ["network", "storage", "clipboard", "env"]
}
```

| Permission | APIs Unlocked |
|------------|---------------|
| `network` | `fetch()`, `WebSocket()` |
| `storage` | `storage.get()`, `storage.set()`, `storage.remove()` |
| `clipboard` | `clipboard.read()`, `clipboard.write()` |
| `env` | `env.get()` |
| *(none needed)* | `console.*`, `setTimeout`, `setInterval`, `clearTimeout`, `clearInterval` |

Calling a gated API without the required permission throws a runtime error.

---

## console

**Permission:** None required

Output to stderr. Does not appear in the terminal UI — only visible when running with stderr redirected (e.g. `tela run ./app 2>debug.log`).

### Methods

```javascript
console.log(...args)    // "[JS] ..." to stderr
console.error(...args)  // "[JS ERROR] ..." to stderr
console.warn(...args)   // "[JS WARN] ..." to stderr
```

### Behavior

- Strings are printed as-is.
- Objects are JSON-stringified.
- Multiple arguments are space-separated.
- Unstringifiable values print as `[unstringifiable]`.

### Example

```javascript
console.log("state:", state);
console.error("fetch failed:", e.message);
```

---

## Timers

**Permission:** None required

### `setTimeout(callback, ms)`

Executes `callback` once after `ms` milliseconds. Returns a numeric timer ID.

```javascript
var id = setTimeout(function() {
  globalThis.__tela_dispatch_queue__.push({ type: "delayed_action" });
}, 2000);
```

### `setInterval(callback, ms)`

Executes `callback` repeatedly every `ms` milliseconds. Returns a numeric timer ID.

```javascript
var id = setInterval(function() {
  globalThis.__tela_dispatch_queue__.push({ type: "tick" });
}, 1000);
```

### `clearTimeout(id)` / `clearInterval(id)`

Cancels a timer. Both functions are interchangeable — they work on either type.

```javascript
clearTimeout(id);
clearInterval(id);
```

### Implementation Details

- Minimum delay is 1ms (enforced). Passing `0` or `undefined` results in 1ms.
- `setInterval` skips the first immediate tick — the first callback fires after `ms` milliseconds.
- Timer callbacks execute in the QuickJS context. To update state, push to `__tela_dispatch_queue__`.
- Timers are backed by tokio tasks. Each timer spawns a lightweight async task.
- No limit on the number of concurrent timers.

### Limitations

- No `queueMicrotask()` or `requestAnimationFrame()`.
- Timer precision depends on the tokio runtime and system scheduler — not guaranteed to be exact.

---

## fetch()

**Permission:** `network`

HTTP client following the web Fetch API pattern. Returns a Promise.

### Signature

```javascript
var response = await fetch(url, options);
```

### Options

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `method` | string | `"GET"` | HTTP method: `"GET"`, `"POST"`, `"PUT"`, `"DELETE"`, `"PATCH"` |
| `body` | string | none | Request body. Non-string values are converted with `String()`. |
| `headers` | object | `{}` | Request headers as key-value pairs. |

### Response Object

| Property | Type | Description |
|----------|------|-------------|
| `ok` | bool | `true` if status is 200-299 |
| `status` | number | HTTP status code |
| `statusText` | string | HTTP status text (e.g. `"OK"`, `"Not Found"`) |
| `json()` | function | Parse response body as JSON. Returns parsed object (synchronous). |
| `text()` | function | Return response body as string (synchronous). |

### Example

```javascript
try {
  var res = await fetch("https://api.example.com/data", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ query: "hello" }),
  });
  if (res.ok) {
    var data = res.json();
    globalThis.__tela_dispatch_queue__.push({ type: "data_loaded", data: data });
  }
} catch (e) {
  globalThis.__tela_dispatch_queue__.push({ type: "fetch_error", error: e.message });
}
```

### Implementation Details

- Backed by `reqwest` (Rust HTTP client).
- Each `fetch()` call spawns a new tokio task — requests run concurrently.
- No limit on concurrent requests.
- The response is delivered back to JS via the engine's action channel.

### Limitations

- No streaming responses (`res.body` / ReadableStream).
- No `res.headers` access.
- No `res.blob()` or `res.arrayBuffer()`.
- No `AbortController` / request cancellation.
- `json()` and `text()` are synchronous (the body is already fully received).
- No cookie jar / automatic cookie handling.
- No redirect control — follows redirects automatically (reqwest default).

---

## WebSocket

**Permission:** `network`

WebSocket client following the browser WebSocket API pattern.

### Constructor

```javascript
var ws = new WebSocket(url);
```

Creates a new WebSocket connection. Connection happens asynchronously — the constructor returns immediately.

### Event Handlers

| Handler | Event Object | Description |
|---------|-------------|-------------|
| `ws.onopen` | none | Connection established |
| `ws.onmessage` | `{ data: string }` | Message received |
| `ws.onclose` | none | Connection closed |
| `ws.onerror` | `{ message: string }` | Error occurred |

### Methods

| Method | Description |
|--------|-------------|
| `ws.send(data)` | Send a message. Non-string values are JSON-stringified. |
| `ws.close()` | Close the connection. |

### Properties

| Property | Type | Description |
|----------|------|-------------|
| `ws.readyState` | number | `0` = connecting, `1` = open, `3` = closed |
| `ws.url` | string | The connection URL |

### Example

```javascript
var ws = new WebSocket("wss://echo.example.com");

ws.onopen = function() {
  globalThis.__tela_dispatch_queue__.push({ type: "ws_connected" });
  ws.send(JSON.stringify({ subscribe: "channel-1" }));
};

ws.onmessage = function(event) {
  var data = JSON.parse(event.data);
  globalThis.__tela_dispatch_queue__.push({ type: "ws_message", payload: data });
};

ws.onclose = function() {
  globalThis.__tela_dispatch_queue__.push({ type: "ws_disconnected" });
};

ws.onerror = function(event) {
  console.error("WebSocket error:", event.message);
};
```

### Multiple Connections

Yes, you can open multiple WebSocket connections simultaneously. Each `new WebSocket(url)` creates an independent connection with its own ID, event handlers, and send channel.

```javascript
var controlWs = new WebSocket("wss://api.example.com/control");
var dataWs = new WebSocket("wss://api.example.com/stream");

controlWs.onmessage = function(e) { /* handle control messages */ };
dataWs.onmessage = function(e) { /* handle data stream */ };
```

Each connection is backed by a separate tokio task with independent read/write channels.

### Implementation Details

- Backed by `tokio-tungstenite`.
- Each connection spawns two tokio tasks (read loop + write loop).
- Connections are tracked by numeric ID in a shared `HashMap<u64, Sender>`.
- When `ws.close()` is called, the send channel is dropped, which terminates the write task. The read task detects the close and sends the `close` event.
- Events are delivered via the engine's action channel, then routed to the correct JS handler by ID.

### Limitations

- Text messages only — binary messages (ArrayBuffer, Blob) are not supported.
- No sub-protocol negotiation (`new WebSocket(url, protocols)` ignores protocols).
- No custom headers on the handshake.
- No ping/pong API — the underlying library handles protocol-level pings automatically, but there's no JS-level heartbeat. Implement application-level heartbeat with `setInterval` if needed.
- No auto-reconnect — implement in JS:

```javascript
function connectWithRetry(url, delay) {
  var ws = new WebSocket(url);
  ws.onclose = function() {
    setTimeout(function() {
      connectWithRetry(url, Math.min(delay * 2, 30000));
    }, delay);
  };
  ws.onopen = function() {
    // reset delay on success
  };
  return ws;
}
```

- No `addEventListener` — use `onopen`/`onmessage`/`onclose`/`onerror` only (single handler per event).

---

## storage

**Permission:** `storage`

Persistent key-value storage. Data survives app restarts.

### Methods

```javascript
storage.set(key, value)     // Store a value
storage.get(key)            // Retrieve a value (or null)
storage.remove(key)         // Delete a key
```

### Behavior

- All methods are **synchronous**.
- Values are JSON-serialized on write and JSON-deserialized on read.
- If a stored value can't be parsed as JSON, it's returned as a raw string.
- Returns `null` if the key doesn't exist.

### Example

```javascript
// Store
storage.set("user_prefs", { theme: "dark", fontSize: 14 });
storage.set("last_seen", Date.now());

// Retrieve
var prefs = storage.get("user_prefs");  // { theme: "dark", fontSize: 14 }
var missing = storage.get("nonexistent");  // null

// Delete
storage.remove("last_seen");
```

### Implementation Details

- Storage location: `~/.local/share/tela/{app_name}/` (Linux) or platform equivalent via `dirs_next::data_local_dir()`.
- Each key is a separate file on disk.
- Key sanitization: only alphanumeric characters, `_`, and `-` are allowed. All other characters are replaced with `_`.
  - `"my-key"` → file `my-key`
  - `"user.prefs"` → file `user_prefs`
  - `"foo/bar"` → file `foo_bar`
- No size limits enforced (bounded by disk space).
- No atomic writes — a crash during `set()` could corrupt the file.

### Limitations

- No `storage.keys()` or `storage.clear()`.
- No expiry / TTL.
- No cross-app storage — each app name gets its own directory.
- Synchronous I/O — large values may block the render loop briefly.

---

## clipboard

**Permission:** `clipboard`

System clipboard access.

### Methods

```javascript
clipboard.write(text)    // Copy text to clipboard
var text = clipboard.read()  // Read text from clipboard (or null)
```

### Behavior

- Both methods are **synchronous**.
- Reads/writes the system clipboard (the same one used by Ctrl+C/Ctrl+V).

### Example

```javascript
clipboard.write("copied from Tela app");
var content = clipboard.read();
```

### Implementation Details

- **Linux:** Uses `xclip -selection clipboard` (must be installed).
- **macOS:** Uses `pbcopy` / `pbpaste`.
- **Windows:** Not implemented — `clipboard.read()` returns `null`, `clipboard.write()` is a no-op.

### Limitations

- Text only — no images or rich content.
- Requires `xclip` on Linux (common but not always pre-installed).
- No Wayland-specific support (uses X11 clipboard via xclip).
- Synchronous — spawns a subprocess, which may have minor latency.

---

## env

**Permission:** `env`

Read-only access to environment variables.

### Methods

```javascript
var value = env.get(key)  // Returns string or null
```

### Example

```javascript
var apiUrl = env.get("API_URL") || "http://localhost:8080";
var debug = env.get("DEBUG") === "true";
```

### Implementation Details

- Reads from the process environment (`std::env::var`).
- Returns `null` if the variable is not set.

### Limitations

- Read-only — no `env.set()`.
- Reads from the process that launched `tela`, not from a `.env` file.

---

## Dispatch Queue

Not an API per se, but the mechanism for async code to update state.

### Pattern

```javascript
globalThis.__tela_dispatch_queue__.push({ type: "my_action", data: "..." });
```

The engine drains this queue after every render cycle and after handling each action. Each item is passed through `reduce(state, action)`.

### When to Use

- Inside timer callbacks
- Inside WebSocket event handlers
- Inside fetch `.then()` / async handlers
- Any place where you don't have direct access to `dispatch`

### Example

```javascript
setInterval(function() {
  globalThis.__tela_dispatch_queue__.push({ type: "tick" });
}, 1000);

ws.onmessage = function(event) {
  globalThis.__tela_dispatch_queue__.push({
    type: "message_received",
    data: event.data,
  });
};
```

### Note

Inside `view(state, dispatch)`, you have access to `dispatch` directly — but calling it during render pushes to the same queue. Dispatched actions are processed after the current render completes.

---

## Summary

| API | Permission | Sync/Async | Multiple Instances |
|-----|------------|------------|-------------------|
| `console.*` | none | sync | N/A |
| `setTimeout` / `setInterval` | none | async | unlimited |
| `fetch()` | `network` | async (Promise) | unlimited concurrent |
| `WebSocket()` | `network` | async (events) | unlimited concurrent |
| `storage.*` | `storage` | sync | N/A |
| `clipboard.*` | `clipboard` | sync | N/A |
| `env.get()` | `env` | sync | N/A |
