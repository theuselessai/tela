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

    let width = area.width as usize;
    if width == 0 {
        return;
    }

    if value.is_empty() && !placeholder.is_empty() {
        let mut lines: Vec<Line> = vec![Line::from(vec![
            Span::styled(placeholder.to_string(), Style::default().fg(Color::Gray)),
            Span::styled(" ".to_string(), Style::default().fg(bg).bg(fg)),
        ])];
        for _ in 1..area.height {
            lines.push(Line::from(""));
        }
        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, area);
        return;
    }

    let normal_style = Style::default().fg(fg).bg(bg);

    let raw_lines: Vec<&str> = value.split('\n').collect();
    let mut display_lines: Vec<Vec<char>> = Vec::new();
    let mut line_map: Vec<(usize, usize)> = Vec::new();

    let should_wrap = element
        .props
        .get("wrap")
        .map(|v| v.as_bool().unwrap_or(true))
        .unwrap_or(true);

    for (raw_idx, raw_line) in raw_lines.iter().enumerate() {
        let chars: Vec<char> = raw_line.chars().collect();
        if chars.is_empty() {
            display_lines.push(Vec::new());
            line_map.push((raw_idx, 0));
        } else if !should_wrap || chars.len() <= width {
            display_lines.push(chars);
            line_map.push((raw_idx, 0));
        } else {
            let mut offset = 0;
            while offset < chars.len() {
                let end = (offset + width).min(chars.len());
                display_lines.push(chars[offset..end].to_vec());
                line_map.push((raw_idx, offset));
                offset = end;
            }
        }
    }

    let cursor_pos = cursor.min(value.len());
    let mut flat_idx = 0;
    let mut cursor_display_line = 0;
    let mut cursor_display_col = 0;

    for (raw_idx, raw_line) in raw_lines.iter().enumerate() {
        let line_len = raw_line.len();
        if flat_idx + line_len >= cursor_pos || raw_idx == raw_lines.len() - 1 {
            let offset_in_line = cursor_pos - flat_idx;
            let char_offset = raw_line
                .char_indices()
                .enumerate()
                .find(|(i, _)| *i >= offset_in_line)
                .map(|(i, _)| i)
                .unwrap_or(raw_line.chars().count());

            for (dl_idx, (map_raw, map_start)) in line_map.iter().enumerate() {
                if *map_raw == raw_idx {
                    let dl_len = display_lines[dl_idx].len();
                    if char_offset >= *map_start && char_offset <= *map_start + dl_len {
                        cursor_display_line = dl_idx;
                        cursor_display_col = char_offset - map_start;
                        break;
                    }
                    if char_offset < *map_start {
                        cursor_display_line = dl_idx;
                        cursor_display_col = 0;
                        break;
                    }
                    cursor_display_line = dl_idx;
                    cursor_display_col = dl_len;
                }
            }
            break;
        }
        flat_idx += line_len + 1;
    }

    let visible_height = area.height as usize;
    let scroll_offset = if cursor_display_line >= visible_height {
        cursor_display_line - visible_height + 1
    } else {
        0
    };

    let mut rendered_lines: Vec<Line> = Vec::new();
    for dl_idx in scroll_offset..(scroll_offset + visible_height).min(display_lines.len()) {
        let chars = &display_lines[dl_idx];

        if dl_idx == cursor_display_line {
            let spans = render_cursor_spans(chars, cursor_display_col, fg, bg);
            rendered_lines.push(Line::from(spans));
        } else {
            let text: String = chars.iter().collect();
            rendered_lines.push(Line::from(Span::styled(text, normal_style)));
        }
    }

    for _ in rendered_lines.len()..visible_height {
        rendered_lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(rendered_lines);
    frame.render_widget(paragraph, area);
}
