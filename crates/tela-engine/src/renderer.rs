//! Element tree → ratatui widget rendering.
//!
//! Walks the `Element` tree and maps each node to ratatui widgets,
//! performing layout via `ratatui::layout::Layout` and drawing
//! into a `Frame`.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Gauge, List, ListItem, ListState, Paragraph, Row, Table,
    TableState, Tabs, Wrap,
};
use ratatui::Frame;

use crate::elements::Element;

/// Render an element tree into the terminal frame.
pub fn render_element(frame: &mut Frame, area: Rect, element: &Element) {
    match element.tag.as_str() {
        "__fragment__" | "row" => render_children(frame, area, element),
        "__text__" | "span" => {} // consumed by parent
        "box" => render_box(frame, area, element),
        "text" => render_text(frame, area, element),
        "layout" => render_layout(frame, area, element),
        "list" => render_list(frame, area, element),
        "table" => render_table(frame, area, element),
        "tabs" => render_tabs(frame, area, element),
        "input" => render_input(frame, area, element),
        "gauge" => render_gauge(frame, area, element),
        _ => {}
    }
}

fn render_children(frame: &mut Frame, area: Rect, element: &Element) {
    for child in &element.children {
        render_element(frame, area, child);
    }
}

fn render_box(frame: &mut Frame, area: Rect, element: &Element) {
    let border_prop = element
        .props
        .get("border")
        .and_then(|v| v.as_str())
        .unwrap_or("single");

    let block = if border_prop == "none" {
        let mut b = Block::default();
        if let Some(title) = element.props.get("title").and_then(|v| v.as_str()) {
            b = b.title(title.to_string());
        }
        b
    } else {
        let mut b = Block::default()
            .borders(Borders::ALL)
            .border_type(parse_border_type(border_prop));
        if let Some(title) = element.props.get("title").and_then(|v| v.as_str()) {
            b = b.title(title.to_string());
        }
        b
    };

    let inner = block.inner(area);
    frame.render_widget(block, area);
    render_children(frame, inner, element);
}

fn render_text(frame: &mut Frame, area: Rect, element: &Element) {
    let spans = collect_text_content(element);
    let line = Line::from(spans);

    let mut style = Style::default();
    if let Some(fg) = element.props.get("fg").and_then(|v| v.as_str()) {
        style = style.fg(parse_color(fg));
    }
    if let Some(bg) = element.props.get("bg").and_then(|v| v.as_str()) {
        style = style.bg(parse_color(bg));
    }
    if element
        .props
        .get("bold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        style = style.add_modifier(Modifier::BOLD);
    }
    if element
        .props
        .get("italic")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if element
        .props
        .get("underline")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        style = style.add_modifier(Modifier::UNDERLINED);
    }

    let alignment = match element.props.get("align").and_then(|v| v.as_str()) {
        Some("center") => Alignment::Center,
        Some("right") => Alignment::Right,
        _ => Alignment::Left,
    };

    let mut paragraph = Paragraph::new(line).style(style).alignment(alignment);
    if element.props.get("wrap").is_some() {
        let trim = element.props.get("wrap").and_then(|v| v.as_str()) == Some("trim");
        paragraph = paragraph.wrap(Wrap { trim });
    }
    if let Some(offset) = element.props.get("scroll").and_then(|v| v.as_u64()) {
        paragraph = paragraph.scroll((offset as u16, 0));
    }
    frame.render_widget(paragraph, area);
}

fn render_layout(frame: &mut Frame, area: Rect, element: &Element) {
    let direction = match element.props.get("direction").and_then(|v| v.as_str()) {
        Some("horizontal") => Direction::Horizontal,
        _ => Direction::Vertical,
    };

    let constraints: Vec<Constraint> = element
        .children
        .iter()
        .map(|child| {
            if let Some(p) = child.props.get("percent").and_then(|v| v.as_u64()) {
                Constraint::Percentage(p as u16)
            } else if let Some(m) = child.props.get("maxHeight").and_then(|v| v.as_u64()) {
                Constraint::Max(m as u16)
            } else if let Some(m) = child.props.get("maxWidth").and_then(|v| v.as_u64()) {
                Constraint::Max(m as u16)
            } else if let Some(flex) = child.props.get("flex").and_then(|v| v.as_u64()) {
                Constraint::Min(flex as u16)
            } else if let Some(h) = child.props.get("height").and_then(|v| v.as_u64()) {
                Constraint::Length(h as u16)
            } else if let Some(w) = child.props.get("width").and_then(|v| v.as_u64()) {
                Constraint::Length(w as u16)
            } else {
                Constraint::Min(0)
            }
        })
        .collect();

    let chunks = Layout::default()
        .direction(direction)
        .constraints(constraints)
        .split(area);

    for (i, child) in element.children.iter().enumerate() {
        if i < chunks.len() {
            render_element(frame, chunks[i], child);
        }
    }
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

fn render_list(frame: &mut Frame, area: Rect, element: &Element) {
    let children = collect_element_children(element);
    let items: Vec<ListItem> = children
        .iter()
        .map(|child| ListItem::new(Line::from(collect_text_content(child))))
        .collect();

    let highlight_symbol = element
        .props
        .get("highlight_symbol")
        .and_then(|v| v.as_str())
        .unwrap_or("▶ ");

    let mut highlight_style = Style::default();
    if let Some(fg) = element.props.get("highlight_fg").and_then(|v| v.as_str()) {
        highlight_style = highlight_style.fg(parse_color(fg));
    }
    if let Some(bg) = element.props.get("highlight_bg").and_then(|v| v.as_str()) {
        highlight_style = highlight_style.bg(parse_color(bg));
    }

    let list = List::new(items)
        .highlight_style(highlight_style)
        .highlight_symbol(highlight_symbol);

    if let Some(selected) = element.props.get("selected").and_then(|v| v.as_u64()) {
        let mut state = ListState::default();
        state.select(Some(selected as usize));
        frame.render_stateful_widget(list, area, &mut state);
    } else {
        frame.render_widget(list, area);
    }
}

// ---------------------------------------------------------------------------
// Table
// ---------------------------------------------------------------------------

fn render_table(frame: &mut Frame, area: Rect, element: &Element) {
    let row_elements = collect_row_children(element);
    let rows: Vec<Row> = row_elements
        .iter()
        .map(|row_el| {
            let cell_children = collect_element_children(row_el);
            let cells: Vec<Cell> = cell_children
                .iter()
                .map(|c| Cell::from(Line::from(collect_text_content(c))))
                .collect();
            Row::new(cells)
        })
        .collect();

    let widths: Vec<Constraint> = element
        .props
        .get("widths")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| Constraint::Length(v.as_u64().unwrap_or(10) as u16))
                .collect()
        })
        .unwrap_or_else(|| vec![Constraint::Min(0)]);

    let mut table = Table::new(rows, &widths);

    if let Some(header_arr) = element.props.get("header").and_then(|v| v.as_array()) {
        let header_cells: Vec<Cell> = header_arr
            .iter()
            .map(|v| Cell::from(v.as_str().unwrap_or("").to_string()))
            .collect();
        table = table.header(
            Row::new(header_cells)
                .style(Style::default().add_modifier(Modifier::BOLD))
                .bottom_margin(1),
        );
    }

    let mut highlight_style = Style::default();
    if let Some(fg) = element.props.get("highlight_fg").and_then(|v| v.as_str()) {
        highlight_style = highlight_style.fg(parse_color(fg));
    }
    if let Some(bg) = element.props.get("highlight_bg").and_then(|v| v.as_str()) {
        highlight_style = highlight_style.bg(parse_color(bg));
    }
    table = table.row_highlight_style(highlight_style);

    if let Some(selected) = element.props.get("selected").and_then(|v| v.as_u64()) {
        let mut state = TableState::default();
        state.select(Some(selected as usize));
        frame.render_stateful_widget(table, area, &mut state);
    } else {
        frame.render_widget(table, area);
    }
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

fn render_tabs(frame: &mut Frame, area: Rect, element: &Element) {
    let children = collect_element_children(element);
    let titles: Vec<String> = children
        .iter()
        .map(|child| collect_plain_text(child))
        .collect();

    let divider = element
        .props
        .get("divider")
        .and_then(|v| v.as_str())
        .unwrap_or(" | ")
        .to_string();

    let mut highlight_style = Style::default().add_modifier(Modifier::BOLD);
    if let Some(fg) = element.props.get("highlight_fg").and_then(|v| v.as_str()) {
        highlight_style = highlight_style.fg(parse_color(fg));
    }

    let selected = element
        .props
        .get("selected")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let tabs = Tabs::new(titles)
        .select(selected)
        .highlight_style(highlight_style)
        .divider(divider);

    frame.render_widget(tabs, area);
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

fn render_input(frame: &mut Frame, area: Rect, element: &Element) {
    let value = element
        .props
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let placeholder = element
        .props
        .get("placeholder")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let cursor = element
        .props
        .get("cursor")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(value.len());

    let mut fg = Color::Reset;
    if let Some(f) = element.props.get("fg").and_then(|v| v.as_str()) {
        fg = parse_color(f);
    }
    let mut bg = Color::Reset;
    if let Some(b) = element.props.get("bg").and_then(|v| v.as_str()) {
        bg = parse_color(b);
    }

    let spans = if value.is_empty() && !placeholder.is_empty() {
        vec![
            Span::styled(placeholder.to_string(), Style::default().fg(Color::Gray)),
            Span::styled(" ".to_string(), Style::default().fg(bg).bg(fg)),
        ]
    } else {
        let chars: Vec<char> = value.chars().collect();
        let cursor_pos = cursor.min(chars.len());
        let before: String = chars[..cursor_pos].iter().collect();
        let cursor_char = if cursor_pos < chars.len() {
            chars[cursor_pos].to_string()
        } else {
            " ".to_string()
        };
        let after: String = if cursor_pos < chars.len() {
            chars[cursor_pos + 1..].iter().collect()
        } else {
            String::new()
        };

        let normal_style = Style::default().fg(fg).bg(bg);
        let cursor_style = Style::default().fg(bg).bg(fg);

        let mut result = vec![
            Span::styled(before, normal_style),
            Span::styled(cursor_char, cursor_style),
        ];
        if !after.is_empty() {
            result.push(Span::styled(after, normal_style));
        }
        result
    };

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Gauge
// ---------------------------------------------------------------------------

fn render_gauge(frame: &mut Frame, area: Rect, element: &Element) {
    let percent = element
        .props
        .get("percent")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        .min(100) as u16;

    let mut gauge = Gauge::default().percent(percent);

    if let Some(label) = element.props.get("label").and_then(|v| v.as_str()) {
        gauge = gauge.label(label.to_string());
    }

    let mut style = Style::default();
    if let Some(fg) = element.props.get("fg").and_then(|v| v.as_str()) {
        style = style.fg(parse_color(fg));
    }
    if let Some(bg) = element.props.get("bg").and_then(|v| v.as_str()) {
        style = style.bg(parse_color(bg));
    }
    gauge = gauge.gauge_style(style);

    frame.render_widget(gauge, area);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Collect styled spans from an element's children.
/// `__text__` children become unstyled spans, `<span>` children become styled
/// spans, and fragments are recursively flattened.
fn collect_text_content(element: &Element) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for child in &element.children {
        if child.is_text() {
            if let Some(content) = child.text_content() {
                spans.push(Span::raw(content.to_string()));
            }
        } else if child.tag == "span" {
            let text = collect_plain_text(child);
            let style = parse_span_style(child);
            spans.push(Span::styled(text, style));
        } else if child.is_fragment() {
            spans.extend(collect_text_content(child));
        }
    }
    spans
}

/// Collect plain text from an element's children (discarding styles).
fn collect_plain_text(element: &Element) -> String {
    collect_text_content(element)
        .into_iter()
        .map(|s| s.content.into_owned())
        .collect()
}

/// Collect non-text children, flattening fragments.
fn collect_element_children(element: &Element) -> Vec<&Element> {
    let mut result = Vec::new();
    for child in &element.children {
        if child.is_fragment() {
            result.extend(collect_element_children(child));
        } else if !child.is_text() {
            result.push(child);
        }
    }
    result
}

/// Collect `<row>` children for table rendering, flattening fragments.
fn collect_row_children(element: &Element) -> Vec<&Element> {
    let mut result = Vec::new();
    for child in &element.children {
        if child.tag == "row" {
            result.push(child);
        } else if child.is_fragment() {
            result.extend(collect_row_children(child));
        }
    }
    result
}

/// Parse style props from a `<span>` element.
fn parse_span_style(element: &Element) -> Style {
    let mut style = Style::default();
    if let Some(fg) = element.props.get("fg").and_then(|v| v.as_str()) {
        style = style.fg(parse_color(fg));
    }
    if let Some(bg) = element.props.get("bg").and_then(|v| v.as_str()) {
        style = style.bg(parse_color(bg));
    }
    if element
        .props
        .get("bold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        style = style.add_modifier(Modifier::BOLD);
    }
    if element
        .props
        .get("italic")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if element
        .props
        .get("underline")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

fn parse_color(name: &str) -> Color {
    let lower = name.to_lowercase();
    if lower.starts_with('#') && lower.len() == 7 {
        let r = u8::from_str_radix(&lower[1..3], 16).unwrap_or(0);
        let g = u8::from_str_radix(&lower[3..5], 16).unwrap_or(0);
        let b = u8::from_str_radix(&lower[5..7], 16).unwrap_or(0);
        return Color::Rgb(r, g, b);
    }
    match lower.as_str() {
        "red" => Color::Red,
        "green" => Color::Green,
        "blue" => Color::Blue,
        "yellow" => Color::Yellow,
        "cyan" => Color::Cyan,
        "magenta" => Color::Magenta,
        "white" => Color::White,
        "black" => Color::Black,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightblue" => Color::LightBlue,
        "lightyellow" => Color::LightYellow,
        "lightcyan" => Color::LightCyan,
        "lightmagenta" => Color::LightMagenta,
        _ => Color::Reset,
    }
}

fn parse_border_type(name: &str) -> BorderType {
    match name.to_lowercase().as_str() {
        "double" => BorderType::Double,
        "rounded" => BorderType::Rounded,
        "thick" => BorderType::Thick,
        _ => BorderType::Plain,
    }
}
