use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;

use super::render_element;
use crate::elements::Element;

pub fn render(frame: &mut Frame, area: Rect, element: &Element) {
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
