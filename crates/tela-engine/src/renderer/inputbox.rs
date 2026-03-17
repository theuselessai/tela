use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::parse_fg_bg;
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
    let prefix = element
        .props
        .get("prefix")
        .and_then(|v| v.as_str())
        .unwrap_or("  \u{258E} ");
    let (fg, bg) = parse_fg_bg(element);

    let prefix_style = Style::default().fg(Color::Gray);
    let normal_style = Style::default().fg(fg).bg(bg);
    let cursor_style = Style::default().add_modifier(Modifier::UNDERLINED);

    // Placeholder when empty and not focused
    if value.is_empty() && !placeholder.is_empty() {
        let show_cursor = element
            .props
            .get("focused")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let mut lines = vec![Line::from("")];
        if show_cursor {
            lines.push(Line::from(vec![
                Span::styled(prefix.to_string(), prefix_style),
                Span::styled(" ", cursor_style),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(prefix.to_string(), prefix_style),
                Span::styled(
                    placeholder.to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, area);
        return;
    }

    let input_lines: Vec<&str> = if value.is_empty() {
        vec![""]
    } else {
        value.split('\n').collect()
    };
    let mut lines: Vec<Line> = vec![Line::from("")]; // top spacing
    let mut char_offset: usize = 0;

    for input_line in &input_lines {
        let line_len = input_line.len();
        let line_start = char_offset;
        let line_end = char_offset + line_len;

        let bar = Span::styled(prefix.to_string(), prefix_style);

        if cursor >= line_start && cursor <= line_end {
            let pos_in_line = cursor - line_start;
            let before = &input_line[..pos_in_line];

            if pos_in_line < line_len {
                let cursor_char = &input_line[pos_in_line..pos_in_line + 1];
                let rest = &input_line[pos_in_line + 1..];
                lines.push(Line::from(vec![
                    bar,
                    Span::styled(before.to_string(), normal_style),
                    Span::styled(cursor_char.to_string(), cursor_style),
                    Span::styled(rest.to_string(), normal_style),
                ]));
            } else {
                lines.push(Line::from(vec![
                    bar,
                    Span::styled(before.to_string(), normal_style),
                    Span::styled("_", Style::default().fg(Color::DarkGray)),
                ]));
            }
        } else {
            lines.push(Line::from(vec![
                bar,
                Span::styled(input_line.to_string(), normal_style),
            ]));
        }

        char_offset = line_end + 1; // +1 for \n
    }

    // Scroll if too many lines
    let max_lines = area.height as usize;
    if lines.len() > max_lines {
        let skip = lines.len() - max_lines;
        lines = lines.into_iter().skip(skip).collect();
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}
