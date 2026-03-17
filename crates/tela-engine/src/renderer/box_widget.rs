use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::widgets::block::Position;
use ratatui::widgets::{Block, Borders, Padding};
use ratatui::Frame;

use super::{parse_border_type, parse_color, render_element};
use crate::elements::Element;

pub fn render(frame: &mut Frame, area: Rect, element: &Element) {
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

    let block = if let Some(p) = element.props.get("padding").and_then(|v| v.as_u64()) {
        block.padding(Padding::uniform(p as u16))
    } else {
        block
    };

    let block = if let Some(c) = element.props.get("borderStyle").and_then(|v| v.as_str()) {
        block.border_style(Style::default().fg(parse_color(c)))
    } else {
        block
    };

    let block = match element.props.get("titleAlignment").and_then(|v| v.as_str()) {
        Some("center") => block.title_alignment(Alignment::Center),
        Some("right") => block.title_alignment(Alignment::Right),
        Some("left") => block.title_alignment(Alignment::Left),
        _ => block,
    };

    let block = match element.props.get("titlePosition").and_then(|v| v.as_str()) {
        Some("bottom") => block.title_position(Position::Bottom),
        Some("top") => block.title_position(Position::Top),
        _ => block,
    };

    let inner = block.inner(area);
    frame.render_widget(block, area);
    for child in &element.children {
        render_element(frame, inner, child);
    }
}
