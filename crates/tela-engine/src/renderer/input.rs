use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::{parse_fg_bg, render_cursor_spans};
use crate::elements::Element;

pub fn render(frame: &mut Frame, area: Rect, element: &Element) {
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

    let (fg, bg) = parse_fg_bg(element);

    let spans = if value.is_empty() && !placeholder.is_empty() {
        vec![
            Span::styled(placeholder.to_string(), Style::default().fg(Color::Gray)),
            Span::styled(" ".to_string(), Style::default().fg(bg).bg(fg)),
        ]
    } else {
        let chars: Vec<char> = value.chars().collect();
        render_cursor_spans(&chars, cursor, fg, bg)
    };

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
