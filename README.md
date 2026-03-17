# Tela

React Native for terminals. Write terminal apps in JSX — Tela renders them with [ratatui](https://github.com/ratatui/ratatui).

```
React Native:  Hermes (JS)  <->  JSI bridge  <->  UIKit / Android Views
Tela:          QuickJS (JS) <->  bridge      <->  ratatui widgets
```

Apps are bundled JavaScript. No recompilation needed — just ship `bundle.js`.

## Quick Start

```sh
cargo install --path crates/tela-cli
cd examples/counter && npm install && npm run build
tela run examples/counter
```

## How It Works

```
index.jsx  ->  esbuild  ->  bundle.js  ->  QuickJS  ->  element tree  ->  ratatui  ->  terminal
```

An app exports state, a reducer, keybindings, and a view:

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
| `<canvas>` | Canvas | Braille/dot drawing surface |

Custom components are plain JS functions that compose these primitives.

## Engine APIs

QuickJS has no built-in I/O. The engine provides these, gated by permissions in `manifest.json`:

```javascript
// HTTP (requires "network")
var res = await fetch("https://api.example.com/data");
var data = res.json();

// WebSocket (requires "network")
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
```

No Node.js APIs. No `fs`, no `http`, no `child_process`. Sandboxed by default.

## Package Format

```
my-app/
├── src/index.jsx       <- app code
├── bundle.js           <- built output (esbuild)
└── manifest.json       <- metadata + permissions
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
| `counter` | State, keybindings, box, text |
| `todo` | List, table, tabs, gauge, selection |
| `clock` | Timers, fetch, async dispatch |
| `echo` | WebSocket, insert mode, input |
| `demo` | Multiple components composed together |
| `graph-boxes` | Workflow graph with box-drawing characters |
| `graph-canvas` | Workflow graph with braille canvas |
| `plit-tui` | Real-world chat client ([Pipelit](https://github.com/theuselessai/pipelit)) |
| `test-all` | All components in one app |

```sh
cd examples/counter && npm install && npm run build
tela run examples/counter
```

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

Requires `tmux` and `esbuild`. Tests launch examples in tmux sessions, send keys, and assert screen content.

## Crates

| Crate | Purpose |
|-------|---------|
| `tela-engine` | Core runtime: QuickJS bridge, native APIs, renderer |
| `tela-cli` | CLI: `tela run`, `tela dev`, `tela init` |

## Why No Virtual DOM?

A terminal is ~200x50 = 10,000 cells. A phone screen is ~3 million pixels. ratatui already double-buffers and only flushes changed cells — a full re-render means rebuilding the element tree, but only changed cells actually hit the terminal. No virtual DOM needed.

## Contributing

Tela is early-stage. Feedback, bug reports, and PRs are welcome.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
