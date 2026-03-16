# Keyboard Input

How Tela handles keyboard events, the keybinding system, mode switching, and input handling.

## Status Legend

- **Implemented** — available today
- **Planned** — not yet implemented, spec defined here for future work

---

## Overview

Tela uses a declarative keybinding system. The app exports a `keybindings` object that maps modes to key-action pairs. The engine captures keyboard events, looks up the keybinding, and dispatches the corresponding action through `reduce()`.

```javascript
var keybindings = {
  normal: {
    j: "move_down",
    k: "move_up",
    q: "quit",
  },
  insert: {
    // empty — unbound keys handled as text input
  },
};
```

---

## Mode System

**Status:** Implemented

The engine reads `state.mode` to determine which keybinding set is active. Modes are strings — any value is valid.

### Built-in Behavior per Mode

The engine has special handling when `state.mode === "insert"`:

- **Unbound character keys** dispatch `{ type: "input_char", char: "x" }`.
- **Backspace** dispatches `{ type: "input_backspace" }`.
- **Enter** dispatches `{ type: "input_submit" }`.
- **Escape** dispatches `{ type: "enter_normal" }`.

These only fire for keys that are **not** mapped in the current mode's keybindings. Explicit bindings always take priority.

### Mode Switching

Mode switching is done via the reducer. A typical pattern:

```javascript
var keybindings = {
  normal: { i: "enter_insert", ":": "enter_command" },
  insert: {},
  command: {},
};

function reduce(state, action) {
  switch (action.type) {
    case "enter_insert":
      return Object.assign({}, state, { mode: "insert" });
    case "enter_command":
      return Object.assign({}, state, { mode: "command" });
    case "enter_normal":
      return Object.assign({}, state, { mode: "normal" });
    default:
      return state;
  }
}
```

You can define any number of modes. The engine simply looks up `keybindings[state.mode]`.

### Default Mode

If `state.mode` is `undefined`, the engine looks up `keybindings.normal`. Define `mode: "normal"` in your `initialState`.

---

## Supported Keys

**Status:** Implemented

### Character Keys

Any printable character. The key name is the character itself.

```javascript
{
  "a": "action_a",
  "A": "action_shift_a",   // uppercase = Shift held
  "1": "action_one",
  " ": "action_space",
  "/": "action_slash",
}
```

### Special Keys

| Key Name | Key | Status |
|----------|-----|--------|
| `"Enter"` | Enter / Return | Implemented |
| `"Escape"` | Escape | Implemented |
| `"Backspace"` | Backspace / Delete | Implemented |
| `"Tab"` | Tab | Implemented |

### Keys Not Yet Supported

| Key | Status | Notes |
|-----|--------|-------|
| Arrow keys (Up, Down, Left, Right) | Planned | crossterm captures these, engine doesn't map them |
| Function keys (F1-F12) | Planned | |
| Home, End, PageUp, PageDown | Planned | |
| Insert, Delete (forward) | Planned | |

---

## Key Modifiers

### Ctrl+C

**Status:** Implemented (hardcoded)

Always exits the app. Cannot be rebound. This is the emergency exit.

### Other Ctrl Combinations

**Status:** Planned

Currently, `Ctrl+key` combinations (other than Ctrl+C) are not dispatched to JS. crossterm captures the modifier, but the engine only checks for Ctrl+C.

#### Planned Key Name Format

```javascript
{
  "Ctrl+j": "input_newline",
  "Ctrl+s": "save",
  "Ctrl+d": "scroll_half_page_down",
  "Ctrl+u": "scroll_half_page_up",
}
```

### Shift

**Status:** Partial

For character keys, Shift is implicit — `"A"` means Shift+a. For special keys, Shift combinations are not yet supported.

#### Planned

```javascript
{
  "Shift+Tab": "focus_previous",
  "Shift+Enter": "input_newline",
}
```

### Alt / Meta

**Status:** Planned

```javascript
{
  "Alt+j": "move_pane_down",
  "Alt+k": "move_pane_up",
}
```

---

## Key Dispatch Flow

```
1. User presses key
2. crossterm captures KeyEvent { code, modifiers, kind: Press }
3. Engine checks: Ctrl+C? → exit
4. Engine resolves key name (e.g. KeyCode::Char('j') → "j")
5. Engine looks up: keybindings[state.mode][keyName]
6. If found:
   a. If action is "quit" → exit
   b. Otherwise → reduce(state, { type: actionName })
7. If not found AND state.mode === "insert":
   a. Char key → reduce(state, { type: "input_char", char: "j" })
   b. Backspace → reduce(state, { type: "input_backspace" })
   c. Enter → reduce(state, { type: "input_submit" })
   d. Escape → reduce(state, { type: "enter_normal" })
8. If not found AND state.mode !== "insert":
   a. Key is ignored
```

### Important: Keybindings Override Insert Behavior

If you bind a key in insert mode, it takes that binding instead of the default insert behavior:

```javascript
var keybindings = {
  insert: {
    Escape: "enter_normal",       // this would fire anyway (default insert)
    Tab: "autocomplete",          // overrides default (Tab would be ignored)
    Enter: "submit_and_clear",    // overrides default "input_submit"
  },
};
```

---

## Action Payload

Actions dispatched from keybindings are minimal:

```javascript
// From a keybinding like { "j": "move_down" }
{ type: "move_down" }

// From insert mode character input
{ type: "input_char", char: "a" }

// From insert mode special keys
{ type: "input_backspace" }
{ type: "input_submit" }
{ type: "enter_normal" }
```

Keybinding actions only carry `type`. If you need richer payloads, use the reducer to look up context from state:

```javascript
function reduce(state, action) {
  switch (action.type) {
    case "delete_selected":
      // action only has { type: "delete_selected" }
      // but we know what's selected from state
      var items = state.items.filter(function(_, i) { return i !== state.selectedIndex; });
      return Object.assign({}, state, { items: items });
  }
}
```

---

## Planned: Multi-Key Sequences

Leader-key / chord sequences like vim's `gg`, `dd`, `dw`:

```javascript
var keybindings = {
  normal: {
    "g g": "scroll_to_top",
    "d d": "delete_line",
    "d w": "delete_word",
  },
};
```

### Design Notes

This would require the engine to buffer key presses and wait for a timeout before deciding if a key is a single press or the start of a sequence. Typical timeout: 500ms.

---

## Planned: Terminal Dimensions

Expose terminal size to JS so apps can make responsive layout decisions:

```javascript
// Read-only globals, updated on resize
Tela.columns  // number of columns (width)
Tela.rows     // number of rows (height)
```

Usage in view:

```javascript
function view(state) {
  if (Tela.columns > 100) {
    return wideLayout(state);
  }
  return narrowLayout(state);
}
```

---

## Planned: Resize Event

Dispatch an action when the terminal is resized:

```javascript
// Engine automatically dispatches on resize:
{ type: "__tela_resize__", columns: 120, rows: 40 }
```

The app can handle this in its reducer if needed, or ignore it (the view re-renders automatically on resize regardless).

---

## Limitations

| Limitation | Description |
|------------|-------------|
| No mouse events | crossterm captures mouse events but they are not dispatched to JS |
| No key repeat detection | Held keys fire repeated Press events — no way to distinguish hold from rapid press |
| No focus system | No `autoFocus` prop or imperative focus API |
| Ctrl+C cannot be rebound | Always exits the app |
| Single handler per key | No key event bubbling or capture phase |

---

## Quick Reference

### Minimal Keybinding Setup

```javascript
var keybindings = {
  normal: {
    q: "quit",
  },
};
```

### Vim-style Navigation with Insert Mode

```javascript
var keybindings = {
  normal: {
    j: "move_down",
    k: "move_up",
    i: "enter_insert",
    Escape: "enter_normal",
    q: "quit",
  },
  insert: {},
};
```

### Multi-mode with Command Bar

```javascript
var keybindings = {
  normal: {
    j: "move_down",
    k: "move_up",
    i: "enter_insert",
    ":": "enter_command",
    q: "quit",
  },
  insert: {},
  command: {
    Escape: "enter_normal",
    Enter: "execute_command",
    Backspace: "command_backspace",
  },
};
```
