# Styles

Styling reference for Tela components. All styling is done via props on elements — there is no CSS, no style sheets, no class names.

## Status Legend

- **Implemented** — available today
- **Planned** — not yet implemented, spec defined here for future work

---

## Colors

### Named Colors

**Status:** Implemented

| Name | Aliases | Preview |
|------|---------|---------|
| `"red"` | | Standard red |
| `"green"` | | Standard green |
| `"blue"` | | Standard blue |
| `"yellow"` | | Standard yellow |
| `"cyan"` | | Standard cyan |
| `"magenta"` | | Standard magenta |
| `"white"` | | Standard white |
| `"black"` | | Standard black |
| `"gray"` | `"grey"` | Standard gray |

Colors are case-insensitive. An unrecognized color name falls back to `Reset` (terminal default).

```jsx
<text fg="cyan" bg="black">Styled text</text>
```

### Planned: Additional Named Colors

| Name | Description |
|------|-------------|
| `"darkgray"` / `"darkgrey"` | Dark gray (ratatui `Color::DarkGray`) |
| `"lightred"` | Light red |
| `"lightgreen"` | Light green |
| `"lightblue"` | Light blue |
| `"lightyellow"` | Light yellow |
| `"lightcyan"` | Light cyan |
| `"lightmagenta"` | Light magenta |

### Planned: Hex Colors

```jsx
<text fg="#ff5500">Orange text</text>
<text bg="#1a1a2e">Dark background</text>
```

Hex colors (`#RRGGBB`) map to ratatui `Color::Rgb(r, g, b)`. Requires a terminal that supports truecolor (most modern terminals do).

### Planned: 256-Color Palette

```jsx
<text fg="color(196)">Red from 256 palette</text>
```

Indexed color using `color(N)` syntax where N is 0-255. Maps to ratatui `Color::Indexed(n)`.

### Color Support Detection

Terminal color support varies:

| Terminal | Colors |
|----------|--------|
| Most modern terminals (iTerm2, Kitty, Alacritty, WezTerm, Windows Terminal) | Truecolor (16M) |
| xterm-256color | 256 colors |
| Basic terminals | 16 colors |

When a color isn't supported, the terminal approximates to the nearest available color. This is handled by the terminal emulator, not Tela.

---

## Text Modifiers

**Status:** Implemented

| Prop | Type | Effect | Supported On |
|------|------|--------|--------------|
| `bold` | bool | **Bold text** | `<text>`, `<span>` |
| `italic` | bool | *Italic text* | `<text>`, `<span>` |
| `underline` | bool | Underlined text | `<span>` only |

### Planned: Additional Modifiers

| Prop | Type | Effect |
|------|------|--------|
| `dim` | bool | Dimmed/faint text |
| `strikethrough` | bool | ~~Strikethrough text~~ |
| `blink` | bool | Blinking text (limited terminal support) |
| `underline` | bool | Extend to `<text>` (currently `<span>` only) |

### Example

```jsx
<text bold={true} italic={true} fg="yellow">
  Warning: <span underline={true}>important</span>
</text>
```

---

## Text Alignment

**Status:** Implemented

| Value | Description |
|-------|-------------|
| `"left"` | Left-aligned (default) |
| `"center"` | Centered |
| `"right"` | Right-aligned |

```jsx
<text align="center" fg="gray">Centered status text</text>
```

Supported on: `<text>`.

---

## Border Styles

**Status:** Implemented

Used on `<box>` via the `border` prop.

| Value | Appearance |
|-------|------------|
| `"single"` | `┌─┐│ │└─┘` (default) |
| `"double"` | `╔═╗║ ║╚═╝` |
| `"rounded"` | `╭─╮│ │╰─╯` |
| `"thick"` | `┏━┓┃ ┃┗━┛` |
| `"none"` | No border drawn |

```jsx
<box border="rounded" title="Panel">
  <text>Content</text>
</box>
```

---

## Style Composition

There is no `style` object prop today. Styles are set via individual props.

### Current Pattern

```jsx
<text fg="red" bold={true}>Error</text>
<text fg="green">Success</text>
```

### Pattern: Reusable Styles via JS

You can create style "presets" as plain objects and spread them:

```jsx
var styles = {
  error: { fg: "red", bold: true },
  success: { fg: "green", bold: false },
  muted: { fg: "gray", italic: true },
};

// Use via spread in h() calls
function StyledText({ style, children }) {
  return h("text", style, children);
}

// In JSX
<StyledText style={styles.error}>Something went wrong</StyledText>
```

### Planned: `style` Object Prop

```jsx
var theme = {
  heading: { fg: "cyan", bold: true },
  muted: { fg: "gray", italic: true },
};

<text style={theme.heading}>Title</text>
<text style={{ ...theme.muted, align: "center" }}>Subtitle</text>
```

The `style` prop would be a shorthand — individual props override style object values.

---

## Highlight Styles

Some components have separate highlight styling props for their selected/active state.

### List Highlight

```jsx
<list highlight_fg="yellow" highlight_bg="black" highlight_symbol="> " selected={0}>
  <text>Item 1</text>
</list>
```

### Table Highlight

```jsx
<table highlight_fg="cyan" highlight_bg="black" selected={0}>
  ...
</table>
```

### Tabs Highlight

```jsx
<tabs highlight_fg="cyan" selected={0}>
  <text>Tab 1</text>
</tabs>
```

The active tab is always rendered bold in addition to the highlight color.

---

## Summary: What's Available Today

| Feature | Status | Notes |
|---------|--------|-------|
| Named colors (9) | Implemented | red, green, blue, yellow, cyan, magenta, white, black, gray |
| Hex colors (`#RRGGBB`) | Planned | Truecolor support |
| 256-color palette | Planned | `color(N)` syntax |
| `bold` | Implemented | `<text>`, `<span>` |
| `italic` | Implemented | `<text>`, `<span>` |
| `underline` | Implemented | `<span>` only |
| `dim`, `strikethrough` | Planned | |
| `align` | Implemented | left, center, right |
| Border styles | Implemented | none, single, double, rounded, thick |
| Highlight styles | Implemented | list, table, tabs |
| `style` object prop | Planned | Shorthand for style composition |
