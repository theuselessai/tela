# Terminal surface

Tela provides an engine-owned, bounded VT surface for synthetic or application-fed terminal output. It parses bytes with `vt100`; it does **not** pass ANSI through to the host terminal.

```jsx
var id = Tela.terminal.create({ cols: 80, rows: 24 });
Tela.terminal.write(id, "\x1b[31mred\x1b[m\r\n"); // UTF-8 string
Tela.terminal.write(id, [0x1b, 0x5b, 0x32, 0x4a]); // byte array
Tela.terminal.writeBase64(id, "G1szMW1yZWQbW20="); // canonical standard Base64 bytes
var modes = Tela.terminal.modes(id);
// { applicationCursor: false, applicationKeypad: false, bracketedPaste: false }
Tela.terminal.resize(id, 100, 30);
Tela.terminal.reset(id); // full VT reset
// later: Tela.terminal.dispose(id)

function view(state) {
  return <terminal id={id} focused={state.mode === "terminal"} />;
}
```

The widget paints parsed cells, colors, attributes, and its cursor into its assigned layout rectangle, clipping rather than resizing the parser grid. Handles and parser state belong to one `Engine`; no terminal state is global. OSC sequences are parsed only in memory and have no clipboard/title side effects on the host.

Limits: 16 handles per engine; 320 columns × 160 rows per handle; 2,000 scrollback rows; one write is at most 1 MiB. `writeBase64` accepts only canonical padded standard Base64 and decodes it within the same 1 MiB limit. `modes(id)` reports vt100 application cursor/keypad and bracketed-paste modes for input encoding. Invalid handle/size/write calls throw from the JavaScript API.

When `state.mode === "terminal"`, normal `keybindings.terminal` lookup (including `Ctrl+]`) happens first. If no local binding matches, the reducer receives `{ type: "terminal_key", key, ctrl, alt, shift }`. This includes Escape and Ctrl+C; outside terminal mode Ctrl+C remains the emergency exit. The surface does not encode outgoing terminal protocols, spawn a PTY, implement selection/paste, or expose title/clipboard effects.
