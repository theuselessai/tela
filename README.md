# Tela

React Native for terminals. JSX + QuickJS + ratatui.

```
React Native:  Hermes (JS)  ↔  JSI bridge  ↔  UIKit / Android Views
Tela:          QuickJS (JS) ↔  bridge      ↔  ratatui widgets
```

Write terminal apps in JSX. Tela handles rendering, networking, and terminal I/O in Rust. Apps are bundled and run without recompilation.

## Quick Start

```sh
cargo build
cd examples/counter && npm install && npm run build
tela run examples/counter
```

## How It Works

```
index.jsx → esbuild → bundle.js → QuickJS → element tree → ratatui → terminal
```

An app exports four things:

```jsx
var initialState = { count: 0, mode: "normal" };

function reduce(state, action) {
  switch (action.type) {
    case "increment": return { ...state, count: state.count + 1 };
    case "decrement": return { ...state, count: state.count - 1 };
    default: return state;
  }
}

var keybindings = {
  normal: { j: "increment", k: "decrement", q: "quit" },
};

function view(state) {
  return (
    <box border="rounded" title="Counter">
      <text align="center" bold={true}>{"Count: " + state.count}</text>
    </box>
  );
}
```

## Native Components

| Component | Maps to | Purpose |
|-----------|---------|---------|
| `<box>` | Block | Container with borders |
| `<text>` | Paragraph | Text display with styling |
| `<span>` | Span | Inline styled text |
| `<layout>` | Layout | Flex-like containers |
| `<list>` | List | Scrollable item list |
| `<table>` | Table | Tabular data |
| `<tabs>` | Tabs | Tab navigation |
| `<input>` | Custom | Text input with cursor |
| `<gauge>` | Gauge | Progress bar |

Custom components are plain JS functions that compose these primitives.

## Engine APIs

QuickJS has no built-in I/O. The engine provides these, gated by permissions in `manifest.json`:

```javascript
// HTTP (requires "network")
var res = await fetch("https://api.example.com/data");
var data = res.json();

// WebSocket (requires "network") — multiple connections supported
var ws = new WebSocket("wss://example.com/ws");
ws.onmessage = function(e) { Tela.dispatch({ type: "msg", data: e.data }); };

// Timers (no permission needed)
setInterval(function() { Tela.dispatch({ type: "tick" }); }, 1000);

// Storage (requires "storage") — persistent key-value
storage.set("prefs", { theme: "dark" });
var prefs = storage.get("prefs");

// Clipboard (requires "clipboard")
clipboard.write("copied");
var text = clipboard.read();

// Environment (requires "env")
var key = env.get("API_KEY");

// Console (no permission needed) — outputs to stderr
console.log("debug");
```

## Package Format

```
my-app/
├── src/index.jsx       ← app code
├── bundle.js           ← built output (esbuild)
└── manifest.json       ← metadata + permissions
```

```json
{
  "name": "my-app",
  "version": "0.1.0",
  "entry": "bundle.js",
  "permissions": ["network", "storage"]
}
```

## Build

The dev toolchain is your choice. The engine only sees `bundle.js`.

```sh
esbuild src/index.jsx --bundle --jsx-factory=h --jsx-fragment=Fragment --outfile=bundle.js
```

## Examples

| Example | Demonstrates |
|---------|-------------|
| `examples/counter` | State, keybindings, box, text |
| `examples/todo` | List, table, tabs, gauge, selection |
| `examples/clock` | Timers, fetch, async dispatch |
| `examples/echo` | WebSocket, insert mode, input |
| `examples/test-all` | All components in one app |

## Docs

| Document | Covers |
|----------|--------|
| [Architecture](docs/ARCHITECTURE.md) | System design, data flow, rendering pipeline |
| [Components](docs/COMPONENTS.md) | Every native element, props, and usage |
| [Styles](docs/STYLES.md) | Colors, modifiers, alignment, borders |
| [Native API](docs/NATIVE-API.md) | fetch, WebSocket, timers, storage, clipboard, env |
| [Keyboard](docs/KEYBOARD.md) | Key handling, modes, modifiers, input system |

## Tests

```sh
cargo build
bash tests/run_all.sh
```

Requires `tmux` and `esbuild`. Tests run the examples in tmux sessions, send keys, and assert screen content.

## Crates

| Crate | Purpose |
|-------|---------|
| `tela-engine` | Core runtime: QuickJS bridge, native APIs, renderer |
| `tela-cli` | CLI: `tela run`, `tela dev`, `tela init` |
