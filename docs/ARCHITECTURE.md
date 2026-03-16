# Tela — Architecture

React Native for terminals. JSX + QuickJS + ratatui.

## What is Tela?

A runtime for terminal UI apps. App developers write JSX (components, state, reducers, keybindings). Tela handles rendering, networking, and terminal I/O in Rust.

Apps are bundled and run without recompilation — like React Native, but for terminals.

```
React Native:  Hermes (JS)  ↔  JSI bridge  ↔  UIKit / Android Views
Tela:          QuickJS (JS) ↔  bridge      ↔  ratatui widgets
```

## Core Insight

A terminal is ~200x50 = 10,000 cells. A phone screen is ~3 million pixels. The terminal is 300x smaller — full re-renders are cheap. ratatui already double-buffers and only flushes changed cells. No virtual DOM needed.

## Architecture

```
┌─────────────────────────────────────┐
│  App (written by dev)               │
│                                     │
│  index.jsx → esbuild → bundle.js   │
│                                     │
│  • initialState                     │
│  • reduce(state, action)            │
│  • view(state, dispatch)            │
│  • keybindings                      │
└──────────────────┬──────────────────┘
                   │ bundle.js
                   ▼
┌─────────────────────────────────────┐
│  Rust Engine                        │
│                                     │
│  QuickJS runtime                    │
│  ├── evaluates bundle.js            │
│  ├── state lives in JS              │
│  ├── calls reduce(state, action)    │
│  ├── calls view(state, dispatch)    │
│  └── h() → element tree             │
│                                     │
│  Native functions (Rust-backed)     │
│  ├── fetch() → reqwest              │
│  ├── WebSocket() → tungstenite      │
│  ├── setTimeout/setInterval         │
│  ├── console.* → engine log         │
│  ├── storage.get/set → fs           │
│  └── clipboard → OS clipboard       │
│                                     │
│  Rendering                          │
│  ├── element tree → El enum         │
│  ├── hit testing (mouse events)     │
│  ├── focus management               │
│  └── ratatui → terminal             │
│                                     │
│  Permissions (manifest.json)        │
└─────────────────────────────────────┘
```

## Data Flow

1. Engine loads package (`bundle.js` + `manifest.json`)
2. JS exports `initialState`, `reduce`, `view`, `keybindings`
3. Engine calls `view(initialState, dispatch)` → gets element tree → renders via ratatui
4. User presses key → engine looks up keybinding → calls `reduce(state, action)` in JS
5. JS returns new state → engine calls `view(newState, dispatch)` → ratatui flushes diff
6. Side effects use async dispatch — JS calls `fetch()`, `WebSocket()` etc. directly

## Pipeline

```
key press / mouse click / timer / network event
  → Rust captures event
  → Rust calls JS: reduce(state, action)     ← JS object in, JS object out
  → Rust calls JS: view(newState, dispatch)   ← returns element tree via h()
  → Rust converts element tree → El enum
  → ratatui renders El enum → terminal buffer
  → ratatui diffs buffer → only changed cells flush
```

State never leaves JS. Rust only receives the element tree.

## Package Format

```
my-app/
├── src/
│   └── index.jsx         ← app code (JSX)
├── bundle.js             ← built output (esbuild)
└── manifest.json         ← metadata + permissions
```

The dev toolchain is the app author's choice — npm, TypeScript, JSX, whatever. The engine only sees `bundle.js`.

### manifest.json

```json
{
  "name": "chat",
  "version": "1.0.0",
  "engine": ">=0.1.0",
  "entry": "bundle.js",
  "permissions": ["network", "storage"]
}
```

Permissions gate native function access:
- `network` — `fetch()`, `WebSocket()`
- `storage` — `storage.get()`, `storage.set()`
- `clipboard` — `clipboard.read()`, `clipboard.write()`
- `env` — `env.get()`

No permission → function throws at runtime.

### App Code (JSX)

```jsx
export const initialState = {
  messages: [],
  input: "",
  mode: "normal",
};

export function reduce(state, action) {
  switch (action.type) {
    case "input_char":
      return { ...state, input: state.input + action.char };
    case "message_send":
      return {
        ...state,
        messages: [...state.messages, { role: "user", content: state.input }],
        input: "",
      };
    case "message_received":
      return {
        ...state,
        messages: [...state.messages, { role: "assistant", content: action.content }],
      };
    default:
      return state;
  }
}

export const keybindings = {
  normal: { i: "enter_insert", q: "quit", ":": "enter_command" },
  insert: { Escape: "enter_normal", Enter: "message_send" },
};

// Side effects via async dispatch
export const actions = {
  async message_send(dispatch, getState) {
    const state = getState();
    const res = await fetch("/api/chat", {
      method: "POST",
      body: JSON.stringify({ text: state.input }),
    });
    const data = await res.json();
    dispatch({ type: "message_received", content: data.content });
  },
};

// Custom component — just a function
function ChatMessage({ msg }) {
  return (
    <text fg={msg.role === "user" ? "blue" : "green"}>
      {msg.content}
    </text>
  );
}

export function view(state, dispatch) {
  return (
    <box border="single" title="Chat">
      <layout direction="vertical">
        <list flex={1}>
          {state.messages.map((msg, i) => (
            <ChatMessage key={i} msg={msg} />
          ))}
        </list>
        <input
          value={state.input}
          placeholder="Type a message..."
          onSubmit={() => dispatch({ type: "message_send" })}
        />
      </layout>
    </box>
  );
}
```

### Build Step

```sh
esbuild src/index.jsx --bundle --jsx-factory=h --jsx-fragment=Fragment --outfile=bundle.js
```

The engine provides `h()` and `Fragment` as globals. JSX compiles to `h()` calls. The engine receives a plain element tree — no JSX parsing at runtime.

## Native Components

Primitives that map to ratatui widgets:

| Component | Maps to | Purpose |
|---|---|---|
| `<box>` | Block | Container, borders, title |
| `<text>` | Paragraph | Text display, wrapping |
| `<span>` | Span | Inline styled text |
| `<list>` | List | Scrollable item list |
| `<table>` | Table | Tabular data |
| `<tabs>` | Tabs | Tab navigation |
| `<input>` | Custom | Text input with cursor |
| `<gauge>` | Gauge | Progress bar |
| `<layout>` | Layout + Constraint | Flex-like containers |

Custom components are plain JS functions that compose primitives. No registration needed.

## Engine-Provided APIs

QuickJS has no built-in I/O. The engine injects these as globals, backed by Rust:

```javascript
// HTTP — follows web fetch API (requires "network" permission)
const res = await fetch("/api/data");
const data = await res.json();

// WebSocket — follows browser WebSocket API (requires "network" permission)
const ws = new WebSocket("wss://example.com/ws");
ws.onmessage = (msg) => dispatch({ type: "ws_message", data: msg.data });

// Timers
const id = setTimeout(() => dispatch({ type: "tick" }), 1000);
clearTimeout(id);

// Logging
console.log("debug info");
console.error("something broke");

// Persistent storage (requires "storage" permission)
await storage.set("key", value);
const val = await storage.get("key");

// Clipboard (requires "clipboard" permission)
await clipboard.write("copied text");
const text = await clipboard.read();
```

No Node.js APIs. No `fs`, no `http`, no `child_process`. Sandboxed, capability-based.

## Mouse & Events

crossterm captures mouse events. The engine does hit testing:

1. Each component gets a `Rect` (x, y, width, height) after layout
2. Mouse event at (col, row) → walk element tree → find deepest component containing point
3. Fire handler on target component

```jsx
<box onClick={() => dispatch({ type: "select_item", id: item.id })}>
  <text>Click me</text>
</box>
```

## Error Handling

- JS throws during `reduce()` → engine keeps previous state, logs error
- JS throws during `view()` → engine shows last good frame + error overlay
- Native function fails (network error) → Promise rejects, JS handles via try/catch
- Out of memory → engine kills JS context, shows error, allows restart

## Hot Reload

rquickjs supports re-evaluating scripts in an existing context. `tela dev` watches the source directory, re-runs the build step, and reloads `bundle.js` — preserving app state across reloads via `Persistent<T>`.

## Why No Virtual DOM?

| | Mobile (React Native) | Terminal (Tela) |
|---|---|---|
| Render surface | ~3M pixels | ~10K cells |
| Render cost | Expensive (GPU) | Cheap (stdout) |
| Diffing | Required (Fabric) | Free (ratatui double-buffer) |
| Virtual DOM | Required | Not needed |

ratatui already keeps two buffers and only flushes the delta to the terminal. A "full re-render" in Tela means calling `view()` and rebuilding the El tree — but only changed terminal cells actually get written.

## Build Toolchain is App Author's Choice

The engine runs `bundle.js`. How you produce it is up to you:

```sh
# Minimal — just JSX transform
esbuild src/index.jsx --bundle --jsx-factory=h --outfile=bundle.js

# TypeScript
esbuild src/index.tsx --bundle --jsx-factory=h --outfile=bundle.js

# npm dependencies, the works
npm install && esbuild src/index.jsx --bundle --jsx-factory=h --outfile=bundle.js
```

No npm at runtime. No node_modules shipped. Just `bundle.js`.

## Crates

| Crate | Purpose |
|---|---|
| `tela-engine` | Core runtime: QuickJS bridge, native functions, element tree, widget rendering |
| `tela-cli` | CLI: `tela run ./app`, `tela dev` (watch + hot reload), `tela init` |

## Prior Art

| Project | Approach | Takeaway |
|---|---|---|
| React Native | JS + native bridge | Architecture model (skip the virtual DOM) |
| plit-tui | Handlebars → XML → ratatui | Proof of concept (El enum, rendering pipeline) |
| iocraft | React-like Rust TUI with hooks | Component trait patterns |
| Ink (JS) | React for CLI | DX reference (how app code should feel) |

## Open Questions

- [ ] Devtools: inline state inspector? separate terminal? web UI?
- [ ] Package registry: where do apps live? git repos? central registry?
- [ ] Error boundaries: granular per-component or global only?
- [ ] Focus system: declarative (autoFocus prop) or imperative (focus API)?
- [ ] Accessibility: screen reader support in terminal?
