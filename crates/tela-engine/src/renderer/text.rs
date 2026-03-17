use ratatui::layout::{Alignment, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use super::{collect_text_content, parse_style};
use crate::elements::Element;

pub fn render(frame: &mut Frame, area: Rect, element: &Element) {
    let spans = collect_text_content(element);
    let line = Line::from(spans);

    let style = parse_style(element);

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
