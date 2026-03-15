# Tela — Architecture

JS-scriptable TUI engine. Templates + state bridge + ratatui rendering.

## What is Tela?

A runtime for terminal UI apps. App developers write JS (state, reducers, keybindings) and Handlebars templates. Tela handles rendering, networking, and terminal I/O in Rust.

Apps are downloaded and run without recompilation — like React Native, but for terminals.

```
React Native:  Hermes (JS)  ↔  JSI bridge  ↔  UIKit / Android Views
Tela:          QuickJS (JS) ↔  bridge      ↔  ratatui widgets
```

## Core Insight

A terminal is ~200x50 = 10,000 cells. A phone screen is ~3 million pixels. The terminal is 300x smaller — full re-renders are cheap. ratatui already double-buffers and only flushes changed cells. No virtual DOM needed.

## Architecture

```
┌──────────────────────────────────┐
│  Package (downloaded at runtime) │
│  ├── index.js                    │
│  ├── templates/*.hbs             │
│  └── manifest.json               │
└──────────────┬───────────────────┘
               │
               ▼
┌──────────────────────────────────┐
│  JS Runtime (QuickJS / rquickjs) │
│                                  │
│  • State (plain JSON objects)    │
│  • Reducers (pure functions)     │
│  • Keybindings (mapping object)  │
│  • Effects (declarative)         │
│                                  │
│  No npm. No node_modules.        │
│  Engine provides: fetch, ws, log │
└──────────────┬───────────────────┘
               │ JSON state
               ▼
┌──────────────────────────────────┐
│  Rust Engine                     │
│                                  │
│  state JSON                      │
│    ↓                             │
│  Handlebars(template, state)     │
│    ↓                             │
│  XML string                      │
│    ↓                             │
│  parse → El tree                 │
│    ↓                             │
│  render → ratatui buffer         │
│    ↓                             │
│  ratatui diffs buffer →          │
│    only changed cells flush      │
└──────────────────────────────────┘
```

## Data Flow

1. Engine loads package (`index.js` + templates)
2. JS exports `initialState`, `reduce`, `keybindings`, `effects`
3. Engine calls `initialState` → gets JSON → renders templates → draws terminal
4. User presses key → engine looks up keybinding → calls `reduce(state, action)` in JS
5. JS returns new state → engine re-renders templates → ratatui flushes diff
6. If reducer returns an effect → engine executes it natively (HTTP, WebSocket)

## Package Format

```
my-app/
├── index.js              ← app logic (vanilla ES2023, no build step)
├── templates/
│   ├── app.hbs           ← root layout
│   ├── chat.hbs          ← chat view
│   └── status_bar.hbs    ← status bar
└── manifest.json         ← metadata + permissions
```

### manifest.json

```json
{
  "name": "chat",
  "version": "1.0.0",
  "engine": ">=0.1.0",
  "entry": "index.js",
  "permissions": ["network", "auth"]
}
```

### index.js

```javascript
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
    case "chat_response":
      return {
        ...state,
        messages: [...state.messages, { role: action.role, content: action.content }],
      };
    default:
      return state;
  }
}

export const keybindings = {
  normal: { i: "enter_insert", q: "quit", ":": "enter_command" },
  insert: { Escape: "enter_normal", Enter: "message_send" },
};

export const effects = {
  message_send: (state) => ({
    type: "http_post",
    url: `/api/v1/workflows/${state.agent}/chat/`,
    body: { text: state.input },
  }),
};
```

## Engine-Provided APIs

JS code has access to globals injected by the Rust engine:

```javascript
// HTTP (executed natively in Rust via reqwest)
const data = await engine.fetch("/api/v1/workflows/");

// WebSocket subscriptions
engine.subscribe("ws:chat_message", (msg) => { ... });

// Logging
engine.log("debug info");

// Auth (reads from plit auth login)
const { token, url } = engine.auth;
```

No Node.js APIs. No `fs`, no `http`, no `child_process`. The engine provides a sandboxed, capability-based API controlled by `manifest.json` permissions.

## Why No Virtual DOM?

| | Mobile (React Native) | Terminal (Tela) |
|---|---|---|
| Render surface | ~3M pixels | ~10K cells |
| Render cost | Expensive (GPU) | Cheap (stdout) |
| Diffing | Required (Fabric) | Free (ratatui double-buffer) |
| Virtual DOM | Required | Not needed |

ratatui already keeps two buffers and only flushes the delta to the terminal. A "full re-render" in Tela means re-evaluating templates and rebuilding the El tree — but only changed terminal cells actually get written. The framework gets diffing for free.

## Why No npm / node_modules?

The JS layer is intentionally thin — just state logic. All heavy lifting (HTTP, WebSocket, file I/O, rendering) is in Rust. There's nothing to `npm install`.

If an app author wants a library, they bundle it before publishing:
```sh
esbuild index.js --bundle --outfile=bundle.js
```

Same as React Native — Metro bundles everything before it hits Hermes.

## Proof of Concept: plit-tui

[plit-tui](https://github.com/theuselessai/plit-tui) is the proof of concept. It already has:

- Handlebars templates → XML → ratatui widget pipeline
- Redux-like actions/reducers/middleware
- Custom components (message_list, input_box, activity_bar)
- WebSocket real-time updates
- Auth integration (plit auth login)

The only missing pieces to make it a Tela app:
1. Embed QuickJS (rquickjs crate)
2. Bridge: Rust calls JS with events, JS returns new state
3. Load templates from package directory instead of embedded
4. Package loader + manifest parser

## Components

| Crate | Purpose |
|---|---|
| `tela-engine` | Core runtime: JS bridge, template renderer, widget registry |
| `tela-cli` | CLI: `tela run ./my-app`, `tela dev` (hot reload), `tela init` |
| `tela-devtools` | State inspector, template viewer, event log |

## Prior Art

| Project | Approach | Takeaway |
|---|---|---|
| React Native | JS + native bridge | Architecture model (but we skip the virtual DOM) |
| iocraft | React-like Rust TUI with hooks | Component model reference |
| reratui | Fiber-based Rust TUI | Reconciliation reference (if we ever need it) |
| Ink (JS) | React for CLI | DX reference (how app code should feel) |

## Open Questions

- [ ] Hot reload: can QuickJS re-evaluate index.js without restarting?
- [ ] Devtools: inline state inspector? separate terminal? web UI?
- [ ] Package registry: where do apps live? git repos? central registry?
- [ ] Permissions model: what capabilities can JS request?
- [ ] Template hot reload: watch templates/ dir, re-register on change?
- [ ] Error boundaries: what happens when JS throws? graceful fallback?
