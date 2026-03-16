# Native Components

Built-in elements that map to ratatui widgets. These are the primitives available in JSX.

Custom components are plain JS functions that compose these primitives — no registration needed.

## Status Legend

- **Implemented** — available today
- **Planned** — not yet implemented, spec defined here for future work

---

## `<box>`

**Status:** Implemented

Container with optional border and title. Renders children inside the bordered area.

```jsx
<box border="rounded" title="My Panel">
  <text>Content inside the box</text>
</box>
```

### Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `border` | string | `"single"` | Border style: `"none"`, `"single"`, `"double"`, `"rounded"`, `"thick"` |
| `title` | string | none | Text displayed in the top border |

### Notes

- When `border="none"`, no border is drawn but children still render in the full area.
- The title is only visible when a border is present.
- Maps to ratatui `Block`.

---

## `<text>`

**Status:** Implemented

Text display with styling and alignment. Can contain inline `<span>` children for mixed styling.

```jsx
<text fg="cyan" bold={true} align="center">
  Hello <span fg="yellow">world</span>
</text>
```

### Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `fg` | string | terminal default | Foreground color (see [STYLES.md](./STYLES.md)) |
| `bg` | string | terminal default | Background color |
| `bold` | bool | `false` | Bold text |
| `italic` | bool | `false` | Italic text |
| `align` | string | `"left"` | Text alignment: `"left"`, `"center"`, `"right"` |

### Children

- Plain strings become `__text__` nodes (unstyled).
- `<span>` elements become styled inline segments.
- `<Fragment>` children are flattened.

### Limitations

- No text wrapping. Long text is clipped at the area boundary.
- No `underline` prop (use `<span underline={true}>` instead).
- No vertical alignment.
- Maps to ratatui `Paragraph`.

### Planned: `wrap` prop

```jsx
<text wrap={true}>Long text that should wrap at word boundaries...</text>
<text wrap="trim">Wraps and trims trailing whitespace</text>
```

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `wrap` | bool or string | `false` | `true` or `"word"` for word-wrap, `"trim"` for wrap + trim |

---

## `<span>`

**Status:** Implemented

Inline styled text. Only meaningful as a child of `<text>`.

```jsx
<text>
  <span fg="green" bold={true}>OK</span> Operation complete
</text>
```

### Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `fg` | string | inherit | Foreground color |
| `bg` | string | inherit | Background color |
| `bold` | bool | `false` | Bold text |
| `italic` | bool | `false` | Italic text |
| `underline` | bool | `false` | Underlined text |

### Notes

- `<span>` is consumed by its parent during rendering — it never renders independently.
- Maps to ratatui `Span`.

---

## `<layout>`

**Status:** Implemented

Flex-like container that splits space among children using constraints.

```jsx
<layout direction="vertical">
  <tabs height={1} />
  <list flex={1} />
  <text height={1}>Status bar</text>
</layout>
```

### Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `direction` | string | `"vertical"` | `"vertical"` (top-to-bottom) or `"horizontal"` (left-to-right) |

### Child Constraint Props

Each direct child of `<layout>` can specify how much space it needs:

| Prop | Type | Constraint | Description |
|------|------|------------|-------------|
| `height` | number | `Length(n)` | Fixed height in cells (vertical layouts) |
| `width` | number | `Length(n)` | Fixed width in cells (horizontal layouts) |
| `flex` | number | `Min(n)` | Minimum size, gets remaining space proportionally |

**Priority:** `flex` > `height`/`width` > default `Min(0)`.

If no constraint prop is set, the child gets `Min(0)` — it takes whatever space is left.

### Planned: Additional Constraints

| Prop | Type | Constraint | Description |
|------|------|------------|-------------|
| `maxHeight` | number | `Max(n)` | Maximum height in cells |
| `maxWidth` | number | `Max(n)` | Maximum width in cells |
| `percent` | number | `Percentage(n)` | Percentage of parent space (0-100) |
| `ratio` | string | `Ratio(a, b)` | Ratio-based split, e.g. `"1:3"` |

### Example: Responsive Sidebar

```jsx
// Planned: once terminal dimensions are exposed
function view(state) {
  var cols = Tela.columns;
  if (cols > 100) {
    return (
      <layout direction="horizontal">
        <list width={30} />
        <box flex={1} />
      </layout>
    );
  }
  return <box flex={1} />;
}
```

### Notes

- Fragments are flattened — their children become direct children of the layout.
- Maps to ratatui `Layout` + `Constraint`.

---

## `<list>`

**Status:** Implemented

Scrollable item list with selection highlight.

```jsx
<list selected={state.index} highlight_fg="yellow" highlight_symbol="> ">
  {items.map(function(item, i) {
    return <text key={i}>{item.name}</text>;
  })}
</list>
```

### Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `selected` | number | none | 0-based index of highlighted item. If omitted, no highlight. |
| `highlight_symbol` | string | `"▶ "` | Prefix string for the selected item |
| `highlight_fg` | string | none | Foreground color of the highlight |
| `highlight_bg` | string | none | Background color of the highlight |

### Children

Each child should be a `<text>` element representing one list item.

### Limitations

- No scroll offset control — selection auto-scrolls to keep the selected item visible.
- No multi-select.
- Maps to ratatui `List` + `ListState`.

---

## `<table>`

**Status:** Implemented

Tabular data with optional header and row selection.

```jsx
<table header={["Name", "Status"]} widths={[20, 10]} selected={state.row}>
  {data.map(function(row, i) {
    return (
      <row key={i}>
        <text>{row.name}</text>
        <text>{row.status}</text>
      </row>
    );
  })}
</table>
```

### Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `header` | string[] | none | Column header labels (rendered bold with bottom margin) |
| `widths` | number[] | `[Min(0)]` | Column widths in cells (`Length(n)` per value) |
| `selected` | number | none | 0-based row index to highlight |
| `highlight_fg` | string | none | Row highlight foreground |
| `highlight_bg` | string | none | Row highlight background |

### Children

Children must be `<row>` elements. Each `<row>` contains `<text>` elements (one per cell).

### Notes

- Fragments inside `<table>` are flattened — only `<row>` children are collected.
- Maps to ratatui `Table` + `TableState`.

---

## `<row>`

**Status:** Implemented

Table row container. Only meaningful as a direct child of `<table>`.

```jsx
<row>
  <text>Cell 1</text>
  <text>Cell 2</text>
</row>
```

### Props

None. Layout is determined by the parent `<table>` widths.

---

## `<tabs>`

**Status:** Implemented

Tab bar for navigation.

```jsx
<tabs selected={state.activeTab} highlight_fg="cyan" divider=" | ">
  <text>Tab 1</text>
  <text>Tab 2</text>
  <text>Tab 3</text>
</tabs>
```

### Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `selected` | number | `0` | 0-based index of the active tab |
| `divider` | string | `" \| "` | Separator between tab labels |
| `highlight_fg` | string | none | Foreground color of the active tab (rendered bold) |

### Children

`<text>` elements — one per tab. The text content becomes the tab label.

### Notes

- Maps to ratatui `Tabs`.
- Active tab is always rendered bold in addition to any highlight color.

---

## `<input>`

**Status:** Implemented

Single-line text input with visible cursor.

```jsx
<input
  value={state.input}
  placeholder="Type here..."
  cursor={state.cursorPos}
  fg="white"
  bg="black"
/>
```

### Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `value` | string | `""` | Current input text |
| `placeholder` | string | `""` | Placeholder text shown when value is empty (rendered in gray) |
| `cursor` | number | end of value | Cursor position (0-based character index) |
| `fg` | string | terminal default | Text foreground color |
| `bg` | string | terminal default | Text background color |

### Cursor Rendering

The cursor is rendered as an inverted cell (fg and bg swapped) at the cursor position. If the cursor is past the end of the text, a space is rendered with inverted colors.

### Limitations

- Single-line only — no line wrapping or vertical scrolling.
- No selection/highlighting of text ranges.
- Input handling is not built-in — the app must handle `input_char`, `input_backspace`, `input_submit` actions via the keybinding system and reducer.

---

## `<gauge>`

**Status:** Implemented

Horizontal progress bar.

```jsx
<gauge percent={75} label="75% complete" fg="green" />
```

### Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `percent` | number | `0` | Progress value, 0-100 (clamped) |
| `label` | string | none | Text label displayed on the gauge |
| `fg` | string | terminal default | Gauge foreground color |
| `bg` | string | terminal default | Gauge background color |

### Notes

- Maps to ratatui `Gauge`.

---

## `<Fragment>`

**Status:** Implemented

Virtual container that renders its children directly into the parent. Used for grouping without adding a DOM node.

```jsx
function StatusIcons({ items }) {
  return (
    <>
      {items.map(function(item, i) {
        return <text key={i}>{item.icon}</text>;
      })}
    </>
  );
}
```

### Notes

- Fragments are flattened during rendering.
- JSX `<>...</>` syntax compiles to `Fragment` via `--jsx-fragment=Fragment`.

---

## Planned: `<scroll>` (Scrollable Container)

**Status:** Planned

A scrollable container that wraps content taller than its allocated area. Manages viewport offset and provides scroll indicators.

```jsx
<scroll offset={state.scrollOffset} sticky="bottom">
  {messages.map(function(msg, i) {
    return <text key={i}>{msg.content}</text>;
  })}
</scroll>
```

### Proposed Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `offset` | number | `0` | Scroll offset in lines from top |
| `sticky` | string | none | `"bottom"` to auto-scroll when new content is added |

### Design Notes

This would be a convenience wrapper. The app still manages `scrollOffset` in state and responds to scroll actions — the component handles viewport clipping and (optionally) rendering a scroll indicator.

---

## Planned: `<textarea>` (Multi-Line Input)

**Status:** Planned

Multi-line text input with cursor tracking, line wrapping, and vertical scrolling.

```jsx
<textarea
  value={state.input}
  placeholder="Type a message..."
  cursor={state.cursorPos}
  maxHeight={5}
  fg="white"
/>
```

### Proposed Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `value` | string | `""` | Current input text (may contain newlines) |
| `placeholder` | string | `""` | Placeholder text when empty |
| `cursor` | number | end of value | Cursor position (character index in full text) |
| `maxHeight` | number | none | Maximum height in lines before scrolling |
| `fg` | string | terminal default | Text foreground |
| `bg` | string | terminal default | Text background |
| `wrap` | bool | `true` | Word-wrap long lines |

### Design Notes

- Like `<input>` but supports newlines and vertical scrolling.
- Cursor rendered as inverted cell, same as `<input>`.
- When content exceeds `maxHeight`, the viewport scrolls to keep the cursor visible.
- Needs corresponding keyboard actions: `input_newline` (e.g. Ctrl+J or Shift+Enter) in addition to the existing `input_char`, `input_backspace`, `input_submit`.

---

## `<sparkline>`

**Status:** Implemented

Compact bar graph for visualizing data series.

```jsx
<sparkline data={[10, 20, 50, 30, 80, 60]} fg="green" max={100} />
```

### Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `data` | number[] | `[]` | Data values to plot |
| `max` | number | auto | Maximum value for scaling. If omitted, uses the max in the data. |
| `fg` | string | terminal default | Bar color |
| `bg` | string | terminal default | Background color |

### Notes

- Uses 9-level Unicode block characters for resolution.
- Maps to ratatui `Sparkline`.

---

## `<barchart>`

**Status:** Implemented

Vertical bar chart with labels and values.

```jsx
<barchart
  data={[["Mon", 12], ["Tue", 28], ["Wed", 15], ["Thu", 42]]}
  barWidth={5}
  barGap={2}
  fg="cyan"
  valueFg="white"
  labelFg="gray"
/>
```

### Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `data` | [string, number][] | `[]` | Array of `[label, value]` pairs |
| `barWidth` | number | auto | Width of each bar in cells |
| `barGap` | number | auto | Gap between bars in cells |
| `max` | number | auto | Maximum value for scaling |
| `fg` | string | terminal default | Bar fill color |
| `valueFg` | string | none | Value label color |
| `labelFg` | string | none | Bottom label color |

### Notes

- Maps to ratatui `BarChart`.

---

## `<linegauge>`

**Status:** Implemented

Thin horizontal line progress bar.

```jsx
<linegauge ratio={0.65} label="65%" fg="magenta" />
```

### Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `ratio` | number | `0.0` | Progress value, 0.0-1.0 (clamped) |
| `label` | string | none | Text label displayed on the gauge |
| `fg` | string | terminal default | Filled portion color |
| `bg` | string | terminal default | Unfilled portion color |
| `lineSet` | string | `"normal"` | Line character set: `"normal"`, `"thick"`, `"double"` |

### Notes

- Maps to ratatui `LineGauge`.

---

## `<chart>`

**Status:** Implemented

X-Y line chart with axes and multiple datasets. Contains `<dataset>` children.

```jsx
<chart
  xBounds={[0, 100]}
  yBounds={[-20, 20]}
  xLabels={["0", "50", "100"]}
  yLabels={["-20", "0", "20"]}
  xTitle="Time"
  yTitle="Value"
>
  <dataset name="sin" data={[[0, 0], [10, 15], [20, 5]]} fg="cyan" marker="braille" />
  <dataset name="cos" data={[[0, 10], [10, -5], [20, 8]]} fg="yellow" marker="dot" />
</chart>
```

### Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `xBounds` | [number, number] | `[0, 100]` | X-axis min and max |
| `yBounds` | [number, number] | `[0, 100]` | Y-axis min and max |
| `xLabels` | string[] | none | Labels along X-axis |
| `yLabels` | string[] | none | Labels along Y-axis |
| `xTitle` | string | none | X-axis title |
| `yTitle` | string | none | Y-axis title |

### `<dataset>` (child of `<chart>`)

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `name` | string | `""` | Legend label |
| `data` | [number, number][] | `[]` | Array of `[x, y]` data points |
| `fg` | string | terminal default | Line/point color |
| `marker` | string | `"dot"` | Point style: `"dot"`, `"braille"`, `"block"`, `"bar"` |

### Notes

- Maps to ratatui `Chart` with `Dataset`, `Axis`.
- `<dataset>` elements are only meaningful as children of `<chart>`.

---

## `<canvas>`

**Status:** Planned

Drawing surface with SVG-compatible shape elements. Uses a coordinate system mapped to terminal cells via braille characters.

```jsx
<canvas viewBox="-180 -90 360 180" marker="braille">
  <map resolution="high" color="white" />
  <rect x={0} y={30} width={10} height={10} color="yellow" />
  <circle cx={103.86} cy={1.35} r={10} color="green" />
  <line x1={-74} y1={40.71} x2={2.35} y2={48.85} color="yellow" />
  <text x={-74} y={40.71} color="green">X</text>
  <polyline points={[[0,0], [10,10], [20,0]]} color="cyan" />
  <polygon points={[[0,0], [10,10], [20,0]]} color="magenta" />
</canvas>
```

### Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `viewBox` | string | none | SVG-style viewBox: `"minX minY width height"`. Alternative to `xBounds`/`yBounds`. |
| `xBounds` | [number, number] | `[-180, 180]` | X-axis coordinate range |
| `yBounds` | [number, number] | `[-90, 90]` | Y-axis coordinate range |
| `marker` | string | `"braille"` | Rendering mode: `"braille"`, `"dot"`, `"block"` |

### Shape Elements (children of `<canvas>`)

#### `<rect>` — Rectangle

| Prop | Type | Description |
|------|------|-------------|
| `x` | number | Left edge X coordinate |
| `y` | number | Bottom edge Y coordinate |
| `width` | number | Width in coordinate units |
| `height` | number | Height in coordinate units |
| `color` | string | Shape color |

#### `<circle>` — Circle

| Prop | Type | Description |
|------|------|-------------|
| `cx` | number | Center X coordinate |
| `cy` | number | Center Y coordinate |
| `r` | number | Radius in coordinate units |
| `color` | string | Shape color |

#### `<line>` — Line segment

| Prop | Type | Description |
|------|------|-------------|
| `x1` | number | Start X |
| `y1` | number | Start Y |
| `x2` | number | End X |
| `y2` | number | End Y |
| `color` | string | Shape color |

#### `<text>` — Positioned text

| Prop | Type | Description |
|------|------|-------------|
| `x` | number | X coordinate |
| `y` | number | Y coordinate |
| `color` | string | Text color |

Text content is the element's children (string).

#### `<polyline>` — Connected line segments

| Prop | Type | Description |
|------|------|-------------|
| `points` | [number, number][] | Array of `[x, y]` vertices. Segments connect consecutive points. |
| `color` | string | Line color |

#### `<polygon>` — Closed polyline

| Prop | Type | Description |
|------|------|-------------|
| `points` | [number, number][] | Array of `[x, y]` vertices. Last point auto-connects to first. |
| `color` | string | Shape color |

#### `<map>` — World map outline (TUI-only, not SVG)

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `resolution` | string | `"low"` | `"low"` or `"high"` |
| `color` | string | `"white"` | Map outline color |

### SVG Compatibility

The API uses SVG element names and attribute conventions where possible:

| SVG | Tela | Difference |
|-----|------|------------|
| `<svg viewBox="...">` | `<canvas viewBox="...">` | Container name differs |
| `fill`, `stroke` | `color` | Single color per shape (terminal limitation) |
| `stroke-width` | — | Not supported (always 1 braille dot) |
| `opacity` | — | Not supported (no alpha in terminal) |
| `transform` | — | Not supported (no rotate/scale/translate) |
| `<path d="...">` | — | Not supported (no bezier curves) |
| `<ellipse>` | — | Not supported (use `<circle>` only) |
| `<g>` | — | Not supported (no grouping/transforms) |

### Notes

- Maps to ratatui `Canvas` with shape primitives.
- The `viewBox` string is parsed into `xBounds` and `yBounds`: `"minX minY width height"` → `xBounds=[minX, minX+width]`, `yBounds=[minY, minY+height]`.
- Shape elements (`<rect>`, `<circle>`, etc.) are only interpreted inside `<canvas>`. Outside a canvas, they are ignored.
- `<text>` inside `<canvas>` is positioned text on the drawing surface, not the paragraph `<text>` element.
- `<map>` is TUI-specific with no SVG equivalent.

---

## Custom Components

Custom components are plain JS functions. No registration, no lifecycle hooks.

```jsx
function ChatMessage({ msg }) {
  return (
    <text fg={msg.role === "user" ? "blue" : "green"}>
      <span bold={true}>{msg.role}: </span>
      <span>{msg.content}</span>
    </text>
  );
}

function view(state) {
  return (
    <layout direction="vertical">
      {state.messages.map(function(msg, i) {
        return <ChatMessage key={i} msg={msg} />;
      })}
    </layout>
  );
}
```

When JSX encounters a function as a tag, `h()` calls it with `{ ...props, children }` and uses the returned element tree. This is resolved at render time — there's no component instance or state.
