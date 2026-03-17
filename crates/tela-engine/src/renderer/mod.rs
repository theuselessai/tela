//! Element tree -> ratatui widget rendering.
//!
//! Walks the `Element` tree and maps each node to ratatui widgets,
//! performing layout via `ratatui::layout::Layout` and drawing
//! into a `Frame`.

pub mod barchart;
pub mod box_widget;
pub mod canvas;
pub mod chart;
pub mod gauge;
pub mod input;
pub mod inputbox;
pub mod layout;
pub mod linegauge;
pub mod list;
pub mod sparkline_widget;
pub mod table;
pub mod tabs;
pub mod text;
pub mod textarea;

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::BorderType;
use ratatui::Frame;

use crate::elements::Element;

/// Render an element tree into the terminal frame.
pub fn render_element(frame: &mut Frame, area: Rect, element: &Element) {
    match element.tag.as_str() {
        "__fragment__" | "row" => render_children(frame, area, element),
        "__text__" | "span" | "dataset" => {} // consumed by parent
        "box" => box_widget::render(frame, area, element),
        "text" => text::render(frame, area, element),
        "layout" => layout::render(frame, area, element),
        "list" => list::render(frame, area, element),
        "table" => table::render(frame, area, element),
        "tabs" => tabs::render(frame, area, element),
        "input" => input::render(frame, area, element),
        "textarea" => textarea::render(frame, area, element),
        "inputbox" => inputbox::render(frame, area, element),
        "gauge" => gauge::render(frame, area, element),
        "linegauge" => linegauge::render(frame, area, element),
        "sparkline" => sparkline_widget::render(frame, area, element),
        "barchart" => barchart::render(frame, area, element),
        "chart" => chart::render(frame, area, element),
        "canvas" => canvas::render(frame, area, element),
        _ => {}
    }
}

fn render_children(frame: &mut Frame, area: Rect, element: &Element) {
    for child in &element.children {
        render_element(frame, area, child);
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

pub(crate) fn collect_text_content(element: &Element) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for child in &element.children {
        if child.is_text() {
            if let Some(content) = child.text_content() {
                spans.push(Span::raw(content.to_string()));
            }
        } else if child.tag == "span" {
            let text = collect_plain_text(child);
            let style = parse_style(child);
            spans.push(Span::styled(text, style));
        } else if child.is_fragment() {
            spans.extend(collect_text_content(child));
        }
    }
    spans
}

pub(crate) fn collect_plain_text(element: &Element) -> String {
    collect_text_content(element)
        .into_iter()
        .map(|s| s.content.into_owned())
        .collect()
}

pub(crate) fn collect_element_children(element: &Element) -> Vec<&Element> {
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

pub(crate) fn collect_row_children(element: &Element) -> Vec<&Element> {
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

pub(crate) fn parse_style(element: &Element) -> Style {
    let mut style = Style::default();

    // Read from style object first (individual props override)
    if let Some(style_obj) = element.props.get("style").and_then(|v| v.as_object()) {
        if let Some(fg) = style_obj.get("fg").and_then(|v| v.as_str()) {
            style = style.fg(parse_color(fg));
        }
        if let Some(bg) = style_obj.get("bg").and_then(|v| v.as_str()) {
            style = style.bg(parse_color(bg));
        }
        if style_obj
            .get("bold")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            style = style.add_modifier(Modifier::BOLD);
        }
        if style_obj
            .get("italic")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            style = style.add_modifier(Modifier::ITALIC);
        }
        if style_obj
            .get("underline")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        if style_obj
            .get("dim")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            style = style.add_modifier(Modifier::DIM);
        }
        if style_obj
            .get("strikethrough")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            style = style.add_modifier(Modifier::CROSSED_OUT);
        }
        if style_obj
            .get("reversed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            style = style.add_modifier(Modifier::REVERSED);
        }
    }

    // Individual props override style object
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
    if element
        .props
        .get("dim")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        style = style.add_modifier(Modifier::DIM);
    }
    if element
        .props
        .get("strikethrough")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        style = style.add_modifier(Modifier::CROSSED_OUT);
    }
    if element
        .props
        .get("reversed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        style = style.add_modifier(Modifier::REVERSED);
    }
    style
}

pub(crate) fn parse_highlight_style(element: &Element) -> Style {
    let mut style = Style::default();
    if let Some(fg) = element.props.get("highlight_fg").and_then(|v| v.as_str()) {
        style = style.fg(parse_color(fg));
    }
    if let Some(bg) = element.props.get("highlight_bg").and_then(|v| v.as_str()) {
        style = style.bg(parse_color(bg));
    }
    style
}

pub(crate) fn parse_fg_bg(element: &Element) -> (Color, Color) {
    let fg = element
        .props
        .get("fg")
        .and_then(|v| v.as_str())
        .map(parse_color)
        .unwrap_or(Color::Reset);
    let bg = element
        .props
        .get("bg")
        .and_then(|v| v.as_str())
        .map(parse_color)
        .unwrap_or(Color::Reset);
    (fg, bg)
}

pub(crate) fn parse_bounds(
    element: &Element,
    x_key: &str,
    y_key: &str,
    x_default: [f64; 2],
    y_default: [f64; 2],
) -> ([f64; 2], [f64; 2]) {
    let x_bounds = element
        .props
        .get(x_key)
        .and_then(|v| v.as_array())
        .map(|a| {
            [
                a.first().and_then(|v| v.as_f64()).unwrap_or(x_default[0]),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(x_default[1]),
            ]
        })
        .unwrap_or(x_default);
    let y_bounds = element
        .props
        .get(y_key)
        .and_then(|v| v.as_array())
        .map(|a| {
            [
                a.first().and_then(|v| v.as_f64()).unwrap_or(y_default[0]),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(y_default[1]),
            ]
        })
        .unwrap_or(y_default);
    (x_bounds, y_bounds)
}

pub(crate) fn render_cursor_spans(
    chars: &[char],
    cursor_pos: usize,
    fg: Color,
    bg: Color,
) -> Vec<Span<'static>> {
    let pos = cursor_pos.min(chars.len());
    let before: String = chars[..pos].iter().collect();
    let cursor_char = if pos < chars.len() {
        chars[pos].to_string()
    } else {
        " ".to_string()
    };
    let after: String = if pos < chars.len() {
        chars[pos + 1..].iter().collect()
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
}

pub(crate) fn parse_color(name: &str) -> Color {
    let lower = name.to_lowercase();
    if lower.starts_with('#') && lower.len() == 7 {
        let r = u8::from_str_radix(&lower[1..3], 16).unwrap_or(0);
        let g = u8::from_str_radix(&lower[3..5], 16).unwrap_or(0);
        let b = u8::from_str_radix(&lower[5..7], 16).unwrap_or(0);
        return Color::Rgb(r, g, b);
    }
    if lower.starts_with("color(") && lower.ends_with(')') {
        if let Ok(n) = lower[6..lower.len() - 1].parse::<u8>() {
            return Color::Indexed(n);
        }
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

pub(crate) fn parse_border_type(name: &str) -> BorderType {
    match name.to_lowercase().as_str() {
        "double" => BorderType::Double,
        "rounded" => BorderType::Rounded,
        "thick" => BorderType::Thick,
        _ => BorderType::Plain,
    }
}

pub(crate) enum CanvasShape {
    MapShape {
        resolution: ratatui::widgets::canvas::MapResolution,
        color: Color,
    },
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: Color,
    },
    CircleShape {
        cx: f64,
        cy: f64,
        r: f64,
        color: Color,
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: Color,
    },
    Text {
        x: f64,
        y: f64,
        color: Color,
        content: String,
    },
    Lines {
        segments: Vec<(f64, f64, f64, f64)>,
        color: Color,
    },
}

pub(crate) fn collect_canvas_shapes(element: &Element) -> Vec<CanvasShape> {
    use ratatui::widgets::canvas::MapResolution;

    let mut shapes = Vec::new();
    for child in &element.children {
        if child.is_text() || child.is_fragment() {
            continue;
        }
        let color = child
            .props
            .get("color")
            .and_then(|v| v.as_str())
            .map(|c| parse_color(c))
            .unwrap_or(Color::White);

        match child.tag.as_str() {
            "map" => {
                let res = match child.props.get("resolution").and_then(|v| v.as_str()) {
                    Some("high") => MapResolution::High,
                    _ => MapResolution::Low,
                };
                shapes.push(CanvasShape::MapShape {
                    resolution: res,
                    color,
                });
            }
            "rect" => {
                let x = child.props.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let y = child.props.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let w = child
                    .props
                    .get("width")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let h = child
                    .props
                    .get("height")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                shapes.push(CanvasShape::Rect {
                    x,
                    y,
                    width: w,
                    height: h,
                    color,
                });
            }
            "circle" => {
                let cx = child
                    .props
                    .get("cx")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let cy = child
                    .props
                    .get("cy")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let r = child.props.get("r").and_then(|v| v.as_f64()).unwrap_or(0.0);
                shapes.push(CanvasShape::CircleShape { cx, cy, r, color });
            }
            "line" => {
                let x1 = child
                    .props
                    .get("x1")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let y1 = child
                    .props
                    .get("y1")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let x2 = child
                    .props
                    .get("x2")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let y2 = child
                    .props
                    .get("y2")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                shapes.push(CanvasShape::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    color,
                });
            }
            "text" => {
                let x = child.props.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let y = child.props.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let content = collect_plain_text(child);
                shapes.push(CanvasShape::Text {
                    x,
                    y,
                    color,
                    content,
                });
            }
            "polyline" | "polygon" => {
                let pts: Vec<(f64, f64)> = child
                    .props
                    .get("points")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|p| {
                                let pair = p.as_array()?;
                                Some((pair.first()?.as_f64()?, pair.get(1)?.as_f64()?))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let mut segments = Vec::new();
                for i in 0..pts.len().saturating_sub(1) {
                    segments.push((pts[i].0, pts[i].1, pts[i + 1].0, pts[i + 1].1));
                }
                if child.tag == "polygon" && pts.len() >= 3 {
                    let last = pts.len() - 1;
                    segments.push((pts[last].0, pts[last].1, pts[0].0, pts[0].1));
                }
                shapes.push(CanvasShape::Lines { segments, color });
            }
            _ => {}
        }
    }
    shapes
}
