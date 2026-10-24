use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use super::{collect_text_content, parse_style};
use crate::elements::Element;

// A Line containing newline characters does not become multiple display rows.
// Split explicitly, retaining span styles and empty/trailing lines.
fn text_lines(spans: Vec<Span<'static>>) -> Vec<Line<'static>> {
    let mut lines = vec![Line::default()];
    for span in spans {
        for (index, part) in span.content.split('\n').enumerate() {
            if index > 0 { lines.push(Line::default()); }
            lines.last_mut().unwrap().spans.push(Span::styled(part.to_owned(), span.style));
        }
    }
    lines
}

#[test]
fn multiline_spans_preserve_breaks_and_styles() {
    use ratatui::style::{Color, Style};
    let style = Style::default().fg(Color::Cyan);
    let lines = text_lines(vec![Span::raw("one\n\n"), Span::styled("two\nthree", style), Span::raw("!\n")]);
    let plain: Vec<String> = lines.iter().map(|line| line.spans.iter().map(|span| span.content.as_ref()).collect()).collect();
    assert_eq!(plain, ["one", "", "two", "three!", ""]);
    assert_eq!(lines[2].spans.last().unwrap().style, style);
    assert_eq!(lines[3].spans[0].style, style);
}

pub fn render(frame: &mut Frame, area: Rect, element: &Element) {
    let spans = collect_text_content(element);
    let lines = text_lines(spans);

    let style = parse_style(element);

    let alignment = match element.props.get("align").and_then(|v| v.as_str()) {
        Some("center") => Alignment::Center,
        Some("right") => Alignment::Right,
        _ => Alignment::Left,
    };

    let mut paragraph = Paragraph::new(lines).style(style).alignment(alignment);
    if element.props.get("wrap").is_some() {
        let trim = element.props.get("wrap").and_then(|v| v.as_str()) == Some("trim");
        paragraph = paragraph.wrap(Wrap { trim });
    }
    if let Some(offset) = element.props.get("scroll").and_then(|v| v.as_u64()) {
        paragraph = paragraph.scroll((offset as u16, 0));
    }
    frame.render_widget(paragraph, area);
}
