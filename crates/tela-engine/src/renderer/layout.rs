use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;

use super::render_element;
use crate::elements::Element;

pub fn render(frame: &mut Frame, area: Rect, element: &Element) {
    let position = element
        .props
        .get("position")
        .and_then(|v| v.as_str())
        .unwrap_or("relative");

    if position == "absolute" {
        render_absolute(frame, area, element);
        return;
    }

    let direction = match element.props.get("direction").and_then(|v| v.as_str()) {
        Some("horizontal") => Direction::Horizontal,
        _ => Direction::Vertical,
    };

    let constraints: Vec<Constraint> = element
        .children
        .iter()
        .map(|child| {
            if let Some(h) = child.props.get("height").and_then(|v| v.as_u64()) {
                Constraint::Length(h as u16)
            } else if let Some(w) = child.props.get("width").and_then(|v| v.as_u64()) {
                Constraint::Length(w as u16)
            } else if let Some(flex) = child.props.get("flex").and_then(|v| v.as_u64()) {
                Constraint::Min(flex as u16)
            } else if let Some(m) = child.props.get("maxHeight").and_then(|v| v.as_u64()) {
                Constraint::Max(m as u16)
            } else if let Some(m) = child.props.get("maxWidth").and_then(|v| v.as_u64()) {
                Constraint::Max(m as u16)
            } else if let Some(p) = child.props.get("layoutPercent").and_then(|v| v.as_u64()) {
                Constraint::Percentage(p as u16)
            } else {
                Constraint::Min(0)
            }
        })
        .collect();

    let mut layout = Layout::default()
        .direction(direction)
        .constraints(constraints);

    if let Some(s) = element.props.get("spacing").and_then(|v| v.as_u64()) {
        layout = layout.spacing(s as u16);
    }

    let chunks = layout.split(area);

    for (i, child) in element.children.iter().enumerate() {
        if i < chunks.len() {
            render_element(frame, chunks[i], child);
        }
    }
}

/// Position children at absolute x,y coordinates within the parent area.
fn render_absolute(frame: &mut Frame, area: Rect, element: &Element) {
    for child in &element.children {
        if child.is_text() {
            continue;
        }
        if child.is_fragment() {
            render_absolute(frame, area, child);
            continue;
        }

        let x = child.props.get("x").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
        let y = child.props.get("y").and_then(|v| v.as_u64()).unwrap_or(0) as u16;

        let abs_x = area.x.saturating_add(x);
        let abs_y = area.y.saturating_add(y);

        if abs_x >= area.x + area.width || abs_y >= area.y + area.height {
            continue;
        }

        let max_w = (area.x + area.width).saturating_sub(abs_x);
        let max_h = (area.y + area.height).saturating_sub(abs_y);

        let w = child
            .props
            .get("width")
            .and_then(|v| v.as_u64())
            .map(|v| (v as u16).min(max_w))
            .unwrap_or(max_w);
        let h = child
            .props
            .get("height")
            .and_then(|v| v.as_u64())
            .map(|v| (v as u16).min(max_h))
            .unwrap_or(max_h);

        if w == 0 || h == 0 {
            continue;
        }

        let child_area = Rect::new(abs_x, abs_y, w, h);
        render_element(frame, child_area, child);
    }
}
